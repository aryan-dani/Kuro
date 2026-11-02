"""GPU-Accelerated Text-to-Speech (TTS) engine for Kuro using Coqui XTTS-v2.

Hardware target: NVIDIA RTX 5070 (CUDA 13.0, sm_120) with CPU fallback.
Features:
- Near-instant GPU speech inference via PyTorch on CUDA.
- Verifies `torch.cuda.is_available()`; falls back to CPU with warning if unavailable.
- Voice Cloning: Uses `config.XTTS_SPEAKER_WAV` or `voices/kuro_voice.wav` if provided,
  or falls back to the default clear reference audio in `models/xtts_v2/samples/en_sample.wav`.
- Caches speaker conditioning latents so generation is near-instant (<1s on RTX 5070).
- Plays audio directly via sounddevice (24kHz).
"""

import os
import io
import time
import logging
import threading
import numpy as np
import sounddevice as sd
import soundfile as sf
import torch
import torchaudio

import compute_policy
import config

logger = logging.getLogger(__name__)

_piper_voice = None
_piper_lock = threading.Lock()

# Agreement for Coqui Public Model License
os.environ["COQUI_TOS_AGREED"] = "1"

# 1. Compatibility patch: PyTorch 2.6+ weights_only default
_orig_load = torch.load
def _compat_load(*args, **kwargs):
    if "weights_only" not in kwargs:
        kwargs["weights_only"] = False
    return _orig_load(*args, **kwargs)
torch.load = _compat_load

# 2. Compatibility patch: Pure soundfile loader for audio files
import TTS.tts.models.xtts as xtts_mod
def _custom_load_audio(audiopath: str, load_sr: int):
    data, sr = sf.read(audiopath)
    tensor = torch.tensor(data, dtype=torch.float32)
    if tensor.ndim == 1:
        tensor = tensor.unsqueeze(0)
    else:
        tensor = tensor.t()
    if sr != load_sr:
        resampler = torchaudio.transforms.Resample(sr, load_sr)
        tensor = resampler(tensor)
    return tensor
xtts_mod.load_audio = _custom_load_audio

# Model and conditioning state
_model = None
_model_lock = threading.Lock()
_cached_speaker_wav = None
_cached_latents = None
_device = None
_speak_lock = threading.Lock()
_last_spoken = ""
_last_spoken_at = 0.0

def get_device() -> str:
    """Verifies CUDA availability on the RTX 5070, falling back to CPU with warning."""
    global _device
    if _device is not None:
        return _device

    try:
        if torch.cuda.is_available():
            gpu_name = torch.cuda.get_device_name(0)
            print(f"[Kuro TTS] CUDA available! Accelerating XTTS-v2 on GPU: {gpu_name}")
            _device = "cuda"
        else:
            print("[Kuro TTS] WARNING: torch.cuda.is_available() is False. Falling back to CPU.")
            _device = "cpu"
    except Exception as e:
        print(f"[Kuro TTS] WARNING: Could not check CUDA status ({e}). Falling back to CPU.")
        _device = "cpu"

    return _device

def get_speaker_wav() -> str:
    """Finds the speaker reference WAV file for voice cloning."""
    # 1. Check .env config
    if config.XTTS_SPEAKER_WAV and os.path.exists(config.XTTS_SPEAKER_WAV):
        return os.path.abspath(config.XTTS_SPEAKER_WAV)

    # 2. Check voices/ folder
    custom_voice = os.path.abspath("voices/kuro_voice.wav")
    if os.path.exists(custom_voice):
        return custom_voice

    # 3. Default built-in sample reference
    default_sample = os.path.abspath("models/xtts_v2/samples/en_sample.wav")
    if os.path.exists(default_sample):
        return default_sample

    raise FileNotFoundError("Could not find reference voice sample for XTTS-v2.")

def get_model():
    """Loads and caches the XTTS-v2 model on GPU."""
    global _model
    if _model is not None:
        return _model

    with _model_lock:
        if _model is not None:
            return _model

        from TTS.tts.configs.xtts_config import XttsConfig
        from TTS.tts.models.xtts import Xtts

        checkpoint_dir = os.path.abspath("models/xtts_v2")
        config_path = os.path.join(checkpoint_dir, "config.json")
        if not os.path.exists(config_path):
            raise FileNotFoundError(f"XTTS checkpoint config not found at: {config_path}")

        print(f"[Kuro TTS] Initializing Coqui XTTS-v2 from {checkpoint_dir}...")
        start_time = time.perf_counter()

        model_config = XttsConfig()
        model_config.load_json(config_path)

        model = Xtts.init_from_config(model_config)
        model.load_checkpoint(model_config, checkpoint_dir=checkpoint_dir, eval=True)

        device = get_device()
        model.to(device)

        elapsed = time.perf_counter() - start_time
        print(f"[Kuro TTS] Model loaded on {device.upper()} in {elapsed:.2f}s.")

        _model = model
        return _model

def unload_model():
    """Releases XTTS-v2 from GPU memory and flushes CUDA cache, returning the GPU to sleep."""
    global _model, _cached_speaker_wav, _cached_latents
    with _model_lock:
        if _model is not None:
            print("[Kuro TTS] Unloading XTTS-v2 from GPU VRAM...")
            del _model
            _model = None
            _cached_speaker_wav = None
            _cached_latents = None
            if torch.cuda.is_available():
                torch.cuda.empty_cache()
            print("[Kuro TTS] XTTS-v2 unloaded. GPU VRAM returned to system.")

def get_conditioning(speaker_wav: str):
    """Caches speaker conditioning latents so synthesis starts instantly."""
    global _cached_speaker_wav, _cached_latents
    if _cached_speaker_wav == speaker_wav and _cached_latents is not None:
        return _cached_latents

    model = get_model()
    print(f"[Kuro TTS] Computing voice cloning embedding from: {os.path.basename(speaker_wav)}")
    gpt_cond_latent, speaker_embedding = model.get_conditioning_latents(audio_path=[speaker_wav])
    _cached_speaker_wav = speaker_wav
    _cached_latents = (gpt_cond_latent, speaker_embedding)
    return _cached_latents

def is_ready() -> bool:
    """True once the natural XTTS voice is loaded on GPU."""
    return _model is not None and _cached_latents is not None


def speak(text: str, *, fast: bool = False):
    """
    Speak text aloud. Prefers the natural XTTS voice once preloaded.
    Serialized + de-duped so concurrent/double instances of the same line don't stack.
    """
    global _last_spoken, _last_spoken_at

    if not text or not text.strip():
        return

    cleaned_text = text.strip()
    if len(cleaned_text) > 320:
        cleaned_text = cleaned_text[:317] + "..."

    with _speak_lock:
        now = time.time()
        if cleaned_text == _last_spoken and (now - _last_spoken_at) < 2.5:
            logger.info("Skipping duplicate speak within 2.5s: %r", cleaned_text)
            return
        _last_spoken = cleaned_text
        _last_spoken_at = now

        print(f'\n[Kuro speaking]: "{cleaned_text}"')
        logger.info('Speaking: "%s" (fast=%s xtts_ready=%s)', cleaned_text, fast, is_ready())

        try:
            import music as music_mod

            music_mod.duck_for_speech()
        except Exception:
            pass
        try:
            sd.stop()
        except Exception:
            pass

        engine = compute_policy.prefer_tts(fast=fast)
        logger.info("TTS engine=%s mode=%s", engine, compute_policy.mode())

        if engine == "piper":
            try:
                _speak_piper(cleaned_text)
                return
            except Exception as e:
                logger.warning("Piper failed (%s); falling back", e)
                engine = "xtts" if compute_policy.on_ac() and not fast else "pyttsx3"

        if engine == "xtts":
            if is_ready():
                try:
                    _speak_xtts(cleaned_text)
                    return
                except Exception as e:
                    logger.warning("XTTS failed (%s); falling back to pyttsx3", e)
            elif not fast and compute_policy.should_preload_xtts():
                try:
                    _warm_xtts(blocking=True)
                    _speak_xtts(cleaned_text)
                    return
                except Exception as e:
                    logger.warning("XTTS warm/speak failed (%s)", e)

        _speak_pyttsx3(cleaned_text)
        if engine == "xtts" and not is_ready() and compute_policy.should_preload_xtts():
            threading.Thread(
                target=lambda: _warm_xtts(blocking=True), daemon=True, name="KuroXTTSWarm"
            ).start()


def _warm_xtts(blocking: bool = True) -> None:
    try:
        get_model()
        get_conditioning(get_speaker_wav())
        # Tiny silent synthesis so first real utterance isn't a cold CUDA spike
        try:
            model = get_model()
            gpt_cond_latent, speaker_embedding = get_conditioning(get_speaker_wav())
            model.inference(
                text="Ready.",
                language=config.XTTS_LANGUAGE or "en",
                gpt_cond_latent=gpt_cond_latent,
                speaker_embedding=speaker_embedding,
                temperature=0.65,
                speed=1.1,
            )
        except Exception as e:
            logger.debug("XTTS synth warm skipped: %s", e)
        logger.info("XTTS warmed and ready on %s (%s)", get_device(), get_speaker_wav())
    except Exception:
        logger.exception("XTTS warm-up failed")


def _get_piper_voice():
    global _piper_voice
    with _piper_lock:
        if _piper_voice is not None:
            return _piper_voice
        from piper import PiperVoice

        model = compute_policy.piper_model_path()
        if not model:
            raise FileNotFoundError("No Piper model (set KURO_PIPER_MODEL)")
        logger.info("Loading Piper voice from %s", model)
        _piper_voice = PiperVoice.load(model)
        return _piper_voice


def _speak_piper(cleaned_text: str) -> None:
    """CPU Piper TTS — fast first audio for battery / thin-client mode."""
    voice = _get_piper_voice()
    start = time.perf_counter()
    chunks: list[np.ndarray] = []
    sample_rate = 22050
    for chunk in voice.synthesize(cleaned_text):
        # piper chunk: AudioChunk with audio_int16_array / sample_rate
        sr = getattr(chunk, "sample_rate", None) or sample_rate
        sample_rate = int(sr)
        arr = getattr(chunk, "audio_int16_array", None)
        if arr is None:
            arr = getattr(chunk, "audio_float_array", None)
            if arr is None:
                continue
            audio = np.asarray(arr, dtype=np.float32)
        else:
            audio = np.asarray(arr, dtype=np.float32) / 32768.0
        chunks.append(audio)
    if not chunks:
        raise RuntimeError("Piper produced no audio")
    audio_data = np.concatenate(chunks)
    peak = float(np.max(np.abs(audio_data))) + 1e-8
    audio_data = np.clip(audio_data * (0.9 / peak), -1.0, 1.0)
    logger.info("Piper synthesis %.2fs", time.perf_counter() - start)
    sd.play(audio_data, samplerate=sample_rate)
    sd.wait()


def _speak_xtts(cleaned_text: str) -> None:
    model = get_model()
    speaker_wav = get_speaker_wav()
    gpt_cond_latent, speaker_embedding = get_conditioning(speaker_wav)
    lang = config.XTTS_LANGUAGE or "en"

    start_time = time.perf_counter()
    out = model.inference(
        text=cleaned_text,
        language=lang,
        gpt_cond_latent=gpt_cond_latent,
        speaker_embedding=speaker_embedding,
        temperature=0.65,
        speed=1.1,
    )
    inference_time = time.perf_counter() - start_time
    logger.info("XTTS synthesis %.2fs on %s", inference_time, get_device())

    audio_data = np.array(out["wav"], dtype=np.float32)
    # Audibility boost
    peak = float(np.max(np.abs(audio_data))) + 1e-8
    audio_data = np.clip(audio_data * (0.9 / peak), -1.0, 1.0)
    sd.play(audio_data, samplerate=24000)
    sd.wait()


def _speak_pyttsx3(cleaned_text: str) -> None:
    """Windows SAPI fallback — prefer a natural female voice to match XTTS."""
    try:
        import pyttsx3

        engine = pyttsx3.init()
        engine.setProperty("rate", 185)
        engine.setProperty("volume", 1.0)
        try:
            for v in engine.getProperty("voices"):
                name = (v.name or "").lower()
                if any(k in name for k in ("zira", "hazel", "susan", "samantha", "female")):
                    engine.setProperty("voice", v.id)
                    break
        except Exception:
            pass
        engine.say(cleaned_text)
        engine.runAndWait()
        try:
            engine.stop()
        except Exception:
            pass
        logger.info("pyttsx3 speech done (fallback)")
    except Exception as e:
        logger.error("pyttsx3 failed: %s", e)
        try:
            import winsound

            winsound.MessageBeep(winsound.MB_ICONASTERISK)
        except Exception:
            pass


def preload() -> None:
    """Load the natural XTTS voice onto GPU at boot (blocking in worker thread)."""
    threading.Thread(target=lambda: _warm_xtts(blocking=True), daemon=True, name="KuroXTTSWarm").start()
