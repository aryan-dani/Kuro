"""Kuro: Personal Jarvis-style AI Assistant.

- Push-to-talk: hold Ctrl+Alt+Space to record, release to send
- Overlay HUD + system tray
- Fast phrase paths (e.g. daddy's home) skip the LLM
"""

from __future__ import annotations

import logging
import os
import sys
import tempfile
import threading
import time
from collections.abc import Callable
from logging.handlers import RotatingFileHandler

import numpy as np
import sounddevice as sd
import soundfile as sf

import config
import hub_client
import media_hold
import phrases
import timing
from overlay import HeadlessOverlay, OverlayManager, OverlayState, voice_level
from tray import KuroState, TrayManager

SAMPLE_RATE = 16000
CHANNELS = 1
# Tray-menu fallback only (push-to-talk has no fixed length)
TRAY_RECORD_SECONDS = 4
MIN_PTT_SECONDS = 0.35
MAX_PTT_SECONDS = 30.0
_MIC_DEVICE: int | None = None
_MIC_RATE = SAMPLE_RATE
_MIC_CHANNELS = CHANNELS


def _host_name(device: dict) -> str:
    try:
        return str(sd.query_hostapis(int(device["hostapi"]))["name"]).lower()
    except Exception:
        return ""


def _mic_candidates() -> list[tuple[int, str]]:
    """WASAPI shared first, then the MME Realtek array. Skip headsets and WDM-KS."""
    devices = sd.query_devices()
    ranked: list[tuple[int, int, str]] = []
    for i, d in enumerate(devices):
        if d.get("max_input_channels", 0) <= 0:
            continue
        name = d.get("name") or ""
        low = name.lower()
        if "headset" in low or "hands-free" in low or "bthhfenum" in low:
            continue
        host = _host_name(d)
        if "wdm" in host:
            continue
        if "mme" in host and "realtek" in low and "mic" in low:
            rank = 0
        elif "wasapi" in host and "microphone" in low:
            rank = 1
        elif "directsound" in host and "microphone" in low:
            rank = 2
        elif "microphone" in low:
            rank = 3
        else:
            continue
        ranked.append((rank, i, name))
    ranked.sort()
    ordered = [(i, name) for _, i, name in ranked]
    if _MIC_DEVICE is not None:
        ordered.sort(key=lambda item: 0 if item[0] == _MIC_DEVICE else 1)
    return ordered


def _open_ptt_stream():
    """Open the mic at its native rate. WASAPI stays in shared mode."""
    global _MIC_DEVICE, _MIC_RATE, _MIC_CHANNELS
    last_error: Exception | None = None
    for idx, name in _mic_candidates():
        info = sd.query_devices(idx)
        rate = int(info.get("default_samplerate") or SAMPLE_RATE)
        channels = 2 if int(info.get("max_input_channels") or 1) >= 2 else 1
        kwargs = dict(
            samplerate=rate,
            channels=channels,
            dtype="int16",
            callback=_ptt_callback,
            blocksize=1024,
            device=idx,
        )
        if "wasapi" in _host_name(info):
            kwargs["extra_settings"] = sd.WasapiSettings(exclusive=False)
        stream = None
        try:
            stream = sd.InputStream(**kwargs)
            stream.start()
        except Exception as exc:
            last_error = exc
            logger.warning("Mic [%s] %s did not open (%s)", idx, name, exc)
            if stream is not None:
                try:
                    stream.close()
                except Exception:
                    pass
            continue
        _MIC_DEVICE = idx
        _MIC_RATE = rate
        _MIC_CHANNELS = channels
        logger.info("Mic open: [%s] %s at %s Hz, %s ch", idx, name, rate, channels)
        return stream
    if last_error is not None:
        raise last_error
    raise RuntimeError("no microphone")


def _as_16k_mono(audio: np.ndarray) -> np.ndarray:
    """Downmix and resample a native-rate capture to 16 kHz mono."""
    if audio.size == 0:
        return np.zeros(0, dtype=np.int16)
    if audio.ndim == 2:
        audio = audio.mean(axis=1)
    samples = np.asarray(audio, dtype=np.float32).reshape(-1)
    rate = _MIC_RATE or SAMPLE_RATE
    if rate != SAMPLE_RATE and samples.size > 1:
        target = max(1, int(round(samples.size * SAMPLE_RATE / float(rate))))
        source_x = np.linspace(0.0, 1.0, num=samples.size, endpoint=False)
        target_x = np.linspace(0.0, 1.0, num=target, endpoint=False)
        samples = np.interp(target_x, source_x, samples).astype(np.float32)
    return np.clip(np.rint(samples), -32768, 32767).astype(np.int16)


is_processing = False
_processing_started_at = 0.0
process_lock = threading.Lock()
tray_manager: TrayManager | None = None
overlay_manager: OverlayManager | None = None
stop_event = threading.Event()
logger = logging.getLogger("kuro")

# Push-to-talk recorder state
_ptt_lock = threading.Lock()
_ptt_active = False
_followup = False
_ptt_frames: list[np.ndarray] = []
_ptt_stream: sd.InputStream | None = None
_ptt_started_at = 0.0
_ptt_partial_stop = threading.Event()
_ptt_level_tick = 0
_live_transcript = ""
_cancel_speech = threading.Event()
_early_home_fired = False
BUSY_STALE_SECONDS = 20.0


def _force_unblock(reason: str = "reset") -> None:
    """Clear stuck busy/PTT state so the hotkey works again."""
    global is_processing, _ptt_active, _ptt_stream, _ptt_frames, _live_transcript, _early_home_fired
    global _processing_started_at

    logger.warning("Force unblock (%s)", reason)
    try:
        import cancel

        cancel.event.set()
    except Exception:
        pass
    _ptt_partial_stop.set()
    _cancel_speech.set()
    with _ptt_lock:
        _ptt_active = False
        stream = _ptt_stream
        _ptt_stream = None
        _ptt_frames = []
    if stream is not None:
        try:
            stream.stop()
            stream.close()
        except Exception:
            pass
    try:
        import sounddevice as _sd

        _sd.stop()
    except Exception:
        pass
    with process_lock:
        is_processing = False
        _processing_started_at = 0.0
    _live_transcript = ""
    _early_home_fired = False
    if tray_manager:
        try:
            tray_manager.set_state(KuroState.IDLE)
        except Exception:
            pass


def _busy_is_stale() -> bool:
    if not is_processing:
        return False
    if _processing_started_at <= 0:
        return True
    return (time.time() - _processing_started_at) > BUSY_STALE_SECONDS


def _setup_logging() -> None:
    log_dir = os.path.join(os.environ.get("LOCALAPPDATA", tempfile.gettempdir()), "Kuro")
    os.makedirs(log_dir, exist_ok=True)
    log_path = os.path.join(log_dir, "kuro.log")

    root = logging.getLogger()
    root.setLevel(logging.INFO)
    fmt = logging.Formatter("%(asctime)s [%(levelname)s] %(message)s")

    fh = RotatingFileHandler(log_path, maxBytes=1_000_000, backupCount=3, encoding="utf-8")
    fh.setFormatter(fmt)
    root.addHandler(fh)

    if sys.stdout and sys.stdout.isatty():
        sh = logging.StreamHandler(sys.stdout)
        sh.setFormatter(fmt)
        root.addHandler(sh)

    logger.info("Logging to %s", log_path)


def _speak(text: str, *, fast: bool = False, on_audio_start=None) -> None:
    import tts

    _cancel_speech.clear()
    # Serialize speech so overlapping calls can't double-play
    tts.speak(text, fast=fast, on_audio_start=on_audio_start)


def _mark_speaking() -> None:
    """Flip the HUD to Speaking only when audio is about to play."""
    if tray_manager:
        tray_manager.set_state(KuroState.SPEAKING)
    if overlay_manager:
        overlay_manager.set_state(OverlayState.SPEAKING, subtitle="Speaking...")


def _transcribe(audio_path: str, partial: bool = False) -> str:
    """STT with a hard timeout so a hung GPU decode can't lock the hotkey forever."""
    from stt import transcribe

    result: dict[str, str] = {"text": ""}
    error: list[BaseException] = []

    def _run():
        try:
            result["text"] = transcribe(audio_path, partial=partial) or ""
        except BaseException as e:  # noqa: BLE001
            error.append(e)

    t = threading.Thread(target=_run, daemon=True, name="KuroSTTJob")
    t.start()
    t.join(timeout=8.0 if partial else 12.0)
    if t.is_alive():
        logger.error("STT timed out (partial=%s) — abandoning decode", partial)
        return ""
    if error:
        logger.error("STT error: %s", error[0])
        return ""
    return result["text"]


def _save_wav(audio: np.ndarray) -> str:
    temp_file = os.path.join(tempfile.gettempdir(), "kuro_temp_audio.wav")
    if audio.ndim == 1:
        audio = audio.reshape(-1, 1)
    sf.write(temp_file, audio, SAMPLE_RATE)
    return temp_file


def _ptt_callback(indata, frames, time_info, status) -> None:  # noqa: ARG001
    global _ptt_level_tick
    if status:
        logger.debug("InputStream status: %s", status)
    with _ptt_lock:
        if not _ptt_active:
            return
        _ptt_frames.append(indata.copy())

    # Live waveform level (0..1). Raw laptop speech is too small to see.
    try:
        level = voice_level(indata)
        if overlay_manager:
            overlay_manager.push_level(level)
            _ptt_level_tick += 1
            if _ptt_level_tick % 8 == 0:
                overlay_manager.set_rec_seconds(time.time() - _ptt_started_at)
    except Exception:
        pass


def _partial_transcribe_loop() -> None:
    """While holding PTT, caption live — and early-fire daddy's home the instant it matches."""
    global _live_transcript, _early_home_fired
    last_text = ""
    while not _ptt_partial_stop.wait(0.45):
        with _ptt_lock:
            if not _ptt_active or len(_ptt_frames) < 8:
                continue
            frames = list(_ptt_frames)
        try:
            audio = _as_16k_mono(np.concatenate(frames, axis=0))
            if audio.shape[0] < int(SAMPLE_RATE * 0.8):
                continue
            path = os.path.join(tempfile.gettempdir(), "kuro_partial.wav")
            sf.write(path, audio, SAMPLE_RATE)
            text = _transcribe(path, partial=True)
            if text and text.strip() and text.strip() != last_text:
                last_text = text.strip()
                _live_transcript = last_text
                logger.info('Live hearing: "%s"', last_text)
                if overlay_manager:
                    overlay_manager.set_transcript(you=last_text)

                # EARLY TRIGGER: don't wait for key release
                if (not _early_home_fired) and phrases.looks_like_daddys_home(last_text):
                    _early_home_fired = True
                    logger.info("Early daddy's home match — firing now")
                    threading.Thread(
                        target=_early_hub_or_local_home,
                        args=(last_text,),
                        daemon=True,
                        name="KuroEarlyHome",
                    ).start()
        except Exception as e:
            logger.debug("Partial STT skipped: %s", e)


def _early_hub_or_local_home(text: str) -> None:
    """Fire daddy's home via hub (preferred) or local plug+song fallback."""
    if hub_client.configured():
        result = hub_client.command(text, early=True)
        if result is None:
            _speak("I can't reach the hub.")
            return
        if result.get("duplicate"):
            return
        if result.get("handled"):
            reply = result.get("reply") or "Welcome home, sir."
            if overlay_manager:
                overlay_manager.set_transcript(kuro=reply)
                overlay_manager.set_state(OverlayState.THINKING, subtitle="On it...")
            hub_client.apply_client_actions(result.get("client_actions") or [])
            _speak(reply, on_audio_start=_mark_speaking)
            return

    if overlay_manager:
        overlay_manager.set_transcript(kuro="Welcome home, sir.")
        overlay_manager.set_state(OverlayState.THINKING, subtitle="On it...")
    if phrases.fire_daddys_home_parallel():
        _speak("Welcome home, sir.", on_audio_start=_mark_speaking)


def start_push_to_talk() -> None:
    """Begin recording while the hotkey is held."""
    global _ptt_active, _ptt_frames, _ptt_stream, _ptt_started_at, _ptt_level_tick, _live_transcript, _early_home_fired

    import engine_api

    if engine_api.is_paused():
        logger.info("Listening paused — ignore PTT start.")
        return

    with process_lock:
        if is_processing:
            if _busy_is_stale():
                logger.warning("Busy state stale — auto-resetting so hotkey works")
            else:
                logger.info("Busy — ignore PTT start.")
                if overlay_manager:
                    overlay_manager.set_state(
                        OverlayState.ERROR,
                        subtitle="Busy — press Esc, then try again",
                        hold_ms=1800,
                    )
                return

    # Reset outside the lock if stale
    if is_processing and _busy_is_stale():
        _force_unblock("stale-busy")

    with _ptt_lock:
        if _ptt_active:
            return
        _ptt_active = True
        _ptt_frames = []
        _ptt_started_at = time.time()
        _ptt_level_tick = 0
        _live_transcript = ""
        _early_home_fired = False

    # Stop Spotify and anything else before the mic opens, or the music is transcribed.
    timing.begin("voice")
    timing.mark("hotkey_down")
    media_hold.hold()

    with _ptt_lock:
        try:
            _ptt_stream = _open_ptt_stream()
            timing.mark("recording_start")
        except Exception:
            _ptt_active = False
            _ptt_stream = None
            logger.exception("Failed to open microphone stream")
            timing.finish()
            media_hold.release()
            if overlay_manager:
                overlay_manager.set_state(OverlayState.ERROR, subtitle="Mic failed to open", hold_ms=1600)
            threading.Thread(
                target=lambda: _speak("I could not open the microphone.", fast=True),
                daemon=True,
                name="KuroMicFail",
            ).start()
            return

    _ptt_partial_stop.clear()
    threading.Thread(target=_partial_transcribe_loop, daemon=True, name="KuroPartialSTT").start()

    logger.info("PTT recording started.")
    if tray_manager:
        tray_manager.set_state(KuroState.LISTENING)
    if overlay_manager:
        overlay_manager.clear_levels()
        overlay_manager.set_transcript(you="", kuro="")
        overlay_manager.set_state(OverlayState.LISTENING, subtitle="Recording")


def stop_push_to_talk() -> None:
    """Stop recording on key release and process the clip."""
    global _ptt_active, _ptt_stream, _ptt_frames

    with _ptt_lock:
        if not _ptt_active:
            return
        _ptt_active = False
        stream = _ptt_stream
        _ptt_stream = None
        frames = list(_ptt_frames)
        _ptt_frames = []
        started = _ptt_started_at

    _ptt_partial_stop.set()

    try:
        if stream is not None:
            stream.stop()
            stream.close()
    except Exception:
        logger.exception("Error closing mic stream")

    timing.mark("hotkey_up")
    elapsed = time.time() - started
    audio = np.zeros(0, dtype=np.int16)
    if frames:
        try:
            audio = _as_16k_mono(np.concatenate(frames, axis=0))
        except Exception:
            logger.exception("Mic convert failed")
            audio = np.zeros(0, dtype=np.int16)
    peak = int(np.max(np.abs(audio))) if audio.size else 0
    logger.info(
        "PTT released after %.2fs (%d chunks, peak=%s, rate=%s)",
        elapsed,
        len(frames),
        peak,
        _MIC_RATE,
    )

    if peak < 80:
        logger.warning("Mic peak very low (%s) — check Windows mic privacy / input device", peak)
        if _followup:
            if overlay_manager:
                overlay_manager.set_open_mic(False)
                overlay_manager.set_state(OverlayState.DONE, subtitle="Done", hold_ms=800)
            if tray_manager:
                tray_manager.set_state(KuroState.IDLE)
            timing.finish()
            media_hold.release()
            return
        if overlay_manager:
            overlay_manager.clear_levels()
            overlay_manager.set_transcript(you="", kuro="")
            # Stay up long enough to open the chat chevron; auto-hides if they do not.
            overlay_manager.set_state(
                OverlayState.ERROR,
                subtitle="Mic too quiet",
                hold_ms=6000,
            )
        if tray_manager:
            tray_manager.set_state(KuroState.IDLE)
        timing.finish()
        media_hold.release()
        return

    if elapsed < MIN_PTT_SECONDS or not frames:
        logger.info("Clip too short — ignored.")
        if _followup:
            if overlay_manager:
                overlay_manager.set_open_mic(False)
                overlay_manager.set_state(OverlayState.DONE, subtitle="Done", hold_ms=800)
            if tray_manager:
                tray_manager.set_state(KuroState.IDLE)
            timing.finish()
            media_hold.release()
            return
        if overlay_manager:
            overlay_manager.clear_levels()
            overlay_manager.set_transcript(you="", kuro="")
            overlay_manager.set_state(
                OverlayState.ERROR,
                subtitle="Too short — keep holding",
                hold_ms=6000,
            )
        if tray_manager:
            tray_manager.set_state(KuroState.IDLE)
        timing.finish()
        media_hold.release()
        return

    max_samples = int(MAX_PTT_SECONDS * SAMPLE_RATE)
    if audio.shape[0] > max_samples:
        audio = audio[:max_samples]

    path = _save_wav(audio)
    timing.mark("audio_saved")
    threading.Thread(
        target=process_audio_file,
        args=(path,),
        daemon=True,
        name="KuroProcess",
    ).start()


def record_fixed(duration: float) -> str:
    """Fixed-length capture for tray menu trigger."""
    logger.info("Fixed record for %.1fs (tray)", duration)
    media_hold.hold()
    try:
        return _record_fixed(duration)
    except Exception:
        media_hold.release()
        raise


def _record_fixed(duration: float) -> str:
    recording = sd.rec(
        int(duration * SAMPLE_RATE),
        samplerate=SAMPLE_RATE,
        channels=CHANNELS,
        dtype=np.int16,
    )
    sd.wait()
    return _save_wav(recording)


def _is_music_only_phrase(text: str) -> bool:
    t = phrases._normalize(text)
    return phrases._is_pause(t) or phrases._is_resume(t) or phrases._is_stop_music(t)


def _is_hub_desk_phrase(text: str) -> bool:
    t = phrases._normalize(text)
    return any(
        (
            phrases._is_daddys_home(t),
            phrases._is_goodnight(t),
            phrases._is_power_on(t),
            phrases._is_power_off(t),
            phrases._is_news(t),
        )
    )


def _route_command(user_prompt: str) -> tuple[str, bool, Callable | None, bool]:
    """Hub first for desk rituals. Meet, WhatsApp, workflows, and inbox stay on this laptop."""
    # Anything phrases can answer stays on this laptop — never waits on hub or Ollama.
    local_first = phrases.try_handle(user_prompt)
    if local_first is not None:
        if overlay_manager:
            overlay_manager.set_state(OverlayState.THINKING, subtitle="On it...")
        return (
            local_first.reply,
            local_first.skip_tts,
            local_first.after_speak,
            local_first.force_fast_tts,
        )

    if hub_client.configured():
        result = hub_client.command(user_prompt, early=False)
        if result is None:
            if _is_music_only_phrase(user_prompt):
                local = phrases.try_handle(user_prompt)
                if local is not None:
                    if overlay_manager:
                        overlay_manager.set_state(OverlayState.THINKING, subtitle="On it...")
                    return local.reply, local.skip_tts, local.after_speak, local.force_fast_tts
            if _is_hub_desk_phrase(user_prompt):
                if overlay_manager:
                    overlay_manager.set_state(OverlayState.THINKING, subtitle="Hub offline")
                return "I can't reach the hub.", False, None, False
            # fall through to inbox / LLM
        elif result.get("duplicate"):
            if overlay_manager:
                overlay_manager.set_state(OverlayState.DONE, subtitle="Already handled", hold_ms=800)
            return "", True, None, False
        elif result.get("handled"):
            if overlay_manager:
                overlay_manager.set_state(OverlayState.THINKING, subtitle="On it...")
            hub_client.apply_client_actions(result.get("client_actions") or [])
            return result.get("reply") or "", False, None, False

    # Local inbox + any remaining local phrases, then LLM as last resort only.
    handled = phrases.try_handle(user_prompt)
    if handled is not None:
        if overlay_manager:
            overlay_manager.set_state(OverlayState.THINKING, subtitle="On it...")
        return handled.reply, handled.skip_tts, handled.after_speak, handled.force_fast_tts

    mode = config.KURO_LLM_MODE
    if mode == "off":
        if overlay_manager:
            overlay_manager.set_state(OverlayState.THINKING, subtitle="No match")
        return (
            "I don't have a phrase for that yet. Desk, news, music, meet, WhatsApp, and workflows work offline.",
            False,
            None,
            True,
        )

    return _llm_last_resort(user_prompt)


def _llm_last_resort(user_prompt: str) -> tuple[str, bool, Callable | None, bool]:
    """Only path that may touch Ollama. Speaks a wait line, starts serve, then answers."""
    import brain
    import ollama_runtime

    if overlay_manager:
        overlay_manager.set_state(OverlayState.THINKING, subtitle="Thinking...")

    if ollama_runtime.is_up():
        return brain.think(user_prompt), False, None, False

    prompt = user_prompt

    def _start_then_answer() -> None:
        if overlay_manager:
            overlay_manager.set_state(OverlayState.THINKING, subtitle="Starting neural engine")
        ok = ollama_runtime.ensure_ready(timeout=45.0)
        if not ok:
            _speak(
                "I could not start the neural engine.",
                fast=True,
                on_audio_start=_mark_speaking,
            )
            return
        if overlay_manager:
            overlay_manager.set_state(OverlayState.THINKING, subtitle="Thinking...")
        try:
            answer = brain.think(prompt)
        except Exception:
            logger.exception("LLM after Ollama start failed")
            answer = "The neural engine started, but the reply failed."
        _speak(answer, on_audio_start=_mark_speaking)

    return (
        "Neural engine is off. Please wait while I start it.",
        False,
        _start_then_answer,
        True,
    )


def _follow_up_listen() -> None:
    """After a question, keep the mic open so the answer is the next turn."""
    global _followup
    time.sleep(0.4)
    _followup = True
    try:
        start_push_to_talk()
        if not _ptt_active:
            return
        if overlay_manager:
            overlay_manager.set_open_mic(True)
            overlay_manager.set_state(OverlayState.LISTENING, subtitle="Go on")
        time.sleep(7.0)
        if _ptt_active:
            stop_push_to_talk()
    finally:
        _followup = False
        if overlay_manager:
            overlay_manager.set_open_mic(False)


def _keep_music_stopped(text: str) -> bool:
    t = phrases._normalize(text)
    return phrases._is_pause(t) or phrases._is_stop_music(t) or phrases._is_goodnight(t)


def process_audio_file(audio_file: str) -> None:
    """Transcribe -> hub / phrases / brain -> speak."""
    global is_processing, _live_transcript, _early_home_fired, _processing_started_at
    asked = False
    user_prompt = ""
    with process_lock:
        if is_processing:
            logger.info("Already processing — drop clip.")
            return
        is_processing = True
        _processing_started_at = time.time()

    try:
        # If daddy's home already early-fired while holding, we're done
        if _early_home_fired:
            logger.info("Daddy's home already handled early — skipping full pipeline")
            if overlay_manager:
                overlay_manager.set_state(OverlayState.DONE, subtitle="Done", hold_ms=1200)
            return

        if tray_manager:
            tray_manager.set_state(KuroState.THINKING)
        if overlay_manager:
            overlay_manager.set_state(OverlayState.THINKING, subtitle="Understanding...")

        # Final STT on the full clip; live is HUD-only unless final misses a clear command.
        live = (_live_transcript or "").strip()
        logger.info("Transcribing full clip...")
        final = (_transcribe(audio_file, partial=False) or "").strip()
        timing.mark("stt_done")
        user_prompt = final
        if not user_prompt and live:
            logger.info("Final STT empty — falling back to live: %r", live)
            user_prompt = live
        elif final and live and final.lower() != live.lower():
            live_hit = phrases.matches(live)
            final_hit = phrases.matches(final)
            if live_hit and not final_hit:
                logger.info("Preferring live command %r over final %r", live, final)
                user_prompt = live
            else:
                logger.info("Live was %r → final %r", live, user_prompt)

        _live_transcript = ""

        if not user_prompt or not user_prompt.strip():
            logger.info("No audible speech detected.")
            if overlay_manager:
                overlay_manager.set_transcript(you="(silence)", kuro="")
                overlay_manager.set_state(OverlayState.ERROR, subtitle="Heard nothing", hold_ms=2500)
            return

        logger.info('You: "%s"', user_prompt)
        if overlay_manager:
            overlay_manager.set_transcript(you=user_prompt)

        reply, skip_tts, after_speak, force_fast = _route_command(user_prompt)
        timing.mark("route_done")

        logger.info('Kuro: "%s"', reply)

        if overlay_manager:
            overlay_manager.set_transcript(kuro=reply)

        if not skip_tts:
            # Stay on Thinking while XTTS synthesizes; Speaking starts with first audio.
            _speak(reply, fast=force_fast, on_audio_start=_mark_speaking)
        if after_speak:
            try:
                after_speak()
            except Exception:
                logger.exception("after_speak failed")

        asked = reply.strip().endswith("?")
        if overlay_manager:
            if asked:
                overlay_manager.set_state(OverlayState.LISTENING, subtitle="Go on")
            else:
                overlay_manager.set_state(OverlayState.DONE, subtitle="Done", hold_ms=1200)

    except Exception as e:
        logger.exception("Error during interaction: %s", e)
        if overlay_manager:
            overlay_manager.set_state(OverlayState.ERROR, subtitle=str(e)[:80], hold_ms=3000)
        try:
            _speak("Pardon me, sir.", fast=False)
        except Exception:
            pass
    finally:
        _early_home_fired = False
        if tray_manager:
            tray_manager.set_state(KuroState.IDLE)
        with process_lock:
            is_processing = False
            _processing_started_at = 0.0
        timing.mark("idle")
        timing.finish()
        if asked and not _followup:
            threading.Thread(target=_follow_up_listen, daemon=True, name="KuroFollow").start()
        elif _keep_music_stopped(user_prompt):
            media_hold.forget()
        else:
            media_hold.release()


def on_tray_trigger() -> None:
    """Tray menu: fixed short capture (cannot hold a menu click)."""
    def _run():
        global is_processing
        with process_lock:
            if is_processing or _ptt_active:
                return
        if overlay_manager:
            overlay_manager.set_transcript(you="", kuro="")
            overlay_manager.set_state(
                OverlayState.LISTENING,
                subtitle=f"Tray listen — {TRAY_RECORD_SECONDS}s",
                listen_seconds=TRAY_RECORD_SECONDS,
            )
        if tray_manager:
            tray_manager.set_state(KuroState.LISTENING)
        path = record_fixed(TRAY_RECORD_SECONDS)
        process_audio_file(path)

    threading.Thread(target=_run, daemon=True, name="KuroTrayTrigger").start()


def _write_heartbeat() -> None:
    path = os.path.join(os.environ.get("LOCALAPPDATA", "."), "Kuro", "heartbeat.txt")
    try:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="ascii") as f:
            f.write(str(int(time.time())))
    except Exception:
        logger.debug("heartbeat write failed", exc_info=True)


def _set_process_priority(*, below_normal: bool) -> None:
    try:
        import ctypes

        # BELOW_NORMAL_PRIORITY_CLASS / NORMAL_PRIORITY_CLASS
        cls = 0x00004000 if below_normal else 0x00000020
        ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(), cls)
        logger.info("Process priority %s", "below normal" if below_normal else "normal")
    except Exception:
        logger.debug("priority change skipped", exc_info=True)


def register_push_to_talk(hotkey_str: str) -> None:
    """Hold Ctrl+Alt+Space to record; release to process."""
    from pynput import keyboard as pk

    logger.info("Registering push-to-talk: hold %s", hotkey_str)
    print(f"[Kuro] Push-to-talk: HOLD '{hotkey_str}' to talk, RELEASE to send.")

    parts = {p.strip().lower() for p in hotkey_str.split("+")}
    want_ctrl = "ctrl" in parts or "control" in parts
    want_alt = "alt" in parts
    want_shift = "shift" in parts
    want_space = "space" in parts

    state = {"ctrl": False, "alt": False, "shift": False, "space": False, "held": False}

    def combo_down() -> bool:
        ok = True
        if want_ctrl:
            ok = ok and state["ctrl"]
        if want_alt:
            ok = ok and state["alt"]
        if want_shift:
            ok = ok and state["shift"]
        if want_space:
            ok = ok and state["space"]
        return ok

    def sync() -> None:
        active = combo_down()
        if active and not state["held"]:
            state["held"] = True
            start_push_to_talk()
        elif (not active) and state["held"]:
            state["held"] = False
            stop_push_to_talk()

    def on_press(key):
        try:
            import cancel

            if cancel.synthetic_active():
                return
            if key == pk.Key.esc:
                logger.info("Escape: force unblock + hide overlay")
                _force_unblock("escape")
                if overlay_manager:
                    overlay_manager.force_hide()
                return
            if key in (pk.Key.ctrl, pk.Key.ctrl_l, pk.Key.ctrl_r):
                state["ctrl"] = True
            elif key in (pk.Key.alt, pk.Key.alt_l, pk.Key.alt_r):
                state["alt"] = True
            elif key in (pk.Key.shift, pk.Key.shift_l, pk.Key.shift_r):
                state["shift"] = True
            elif key == pk.Key.space:
                state["space"] = True
            sync()
        except Exception:
            logger.exception("PTT on_press error")

    def on_release(key):
        try:
            import cancel

            if cancel.synthetic_active():
                return
            if key in (pk.Key.ctrl, pk.Key.ctrl_l, pk.Key.ctrl_r):
                state["ctrl"] = False
            elif key in (pk.Key.alt, pk.Key.alt_l, pk.Key.alt_r):
                state["alt"] = False
            elif key in (pk.Key.shift, pk.Key.shift_l, pk.Key.shift_r):
                state["shift"] = False
            elif key == pk.Key.space:
                state["space"] = False
            sync()
        except Exception:
            logger.exception("PTT on_release error")

    listener = pk.Listener(on_press=on_press, on_release=on_release)
    listener.start()
    logger.info("PTT listener active.")
    _write_heartbeat()

    if overlay_manager:
        overlay_manager.set_state(
            OverlayState.READY,
            subtitle=f"Hold {hotkey_str}",
            hold_ms=2500,
        )

    last_beat = time.time()
    while not stop_event.is_set():
        time.sleep(0.5)
        if time.time() - last_beat >= 20.0:
            _write_heartbeat()
            last_beat = time.time()
    listener.stop()


def on_quit() -> None:
    logger.info("Shutting down...")
    print("\n[Kuro] Shutting down...")
    stop_event.set()
    try:
        hub_client.stop_event_worker()
    except Exception:
        pass
    try:
        stop_push_to_talk()
    except Exception:
        pass
    if overlay_manager:
        overlay_manager.stop()
    os._exit(0)


_BOOT_TICK = timing.tick()


def main() -> None:
    global tray_manager, overlay_manager

    _setup_logging()

    # Force policy off regardless of stale process env
    config.ONLY_WHEN_CHARGING = False

    print("=" * 60)
    print("                      KURO AI ASSISTANT")
    print("          Hold Ctrl+Alt+Space to talk (push-to-talk)")
    print("=" * 60)
    print(f"Hotkey (hold):        {config.KURO_HOTKEY}")
    print(f"LLM mode:             {config.KURO_LLM_MODE} ({config.OLLAMA_MODEL})")
    print(f"Battery gate:         OFF (works on battery)")
    print("-" * 60)
    logger.info(
        "Boot PTT hotkey=%s llm_mode=%s charging_gate=OFF (singleton ok)",
        config.KURO_HOTKEY,
        config.KURO_LLM_MODE,
    )

    import engine_api

    if engine_api.api_requested():
        logger.info("Shell owns the HUD. Python card stays hidden.")
        overlay_manager = HeadlessOverlay()
        overlay_manager.start()
    else:
        overlay_manager = OverlayManager()
        overlay_manager.start()

    _set_process_priority(below_normal=True)

    def _heavy_gate_loop() -> None:
        """Stay light until Performance mode on the real charger, then warm once."""
        import compute_policy

        held_since: float | None = None
        stt_warmed = False
        xtts_warmed = False
        while not stop_event.is_set():
            try:
                ready = compute_policy.ready_for_heavy()
            except Exception:
                logger.exception("Heavy gate check failed")
                ready = False
            now = time.time()
            if ready:
                if held_since is None:
                    held_since = now
                    logger.info("Heavy gate open — warming Whisper")
                if not stt_warmed:
                    _set_process_priority(below_normal=False)
                    try:
                        import stt

                        warm_started = timing.tick()
                        label = stt.warm_for_policy()
                        timing.once("startup", "whisper_load", timing.since(warm_started))
                        logger.info("STT warm complete (%s)", label)
                        stt_warmed = True
                    except Exception:
                        logger.exception("STT warm failed")
                elif (not xtts_warmed) and (now - held_since) >= 15.0:
                    try:
                        import stt

                        if stt.is_busy():
                            logger.info("XTTS warm waiting until Whisper is idle")
                        else:
                            logger.info("Warming XTTS")
                            warm_started = timing.tick()
                            with stt.exclusive():
                                import tts

                                tts._warm_xtts(blocking=True)
                            timing.once("startup", "xtts_load", timing.since(warm_started))
                            logger.info("XTTS warm complete")
                            xtts_warmed = True
                    except Exception:
                        logger.exception("XTTS warm failed")
                        xtts_warmed = True
            else:
                if held_since is not None and not stt_warmed:
                    logger.info("Heavy gate closed before warm — staying light")
                held_since = None
                if stt_warmed or xtts_warmed:
                    logger.info("Leaving Performance/AC — unloading GPU models")
                    try:
                        import stt

                        stt.unload_all()
                    except Exception:
                        logger.exception("STT unload failed")
                    try:
                        import tts

                        tts.unload_model()
                    except Exception:
                        logger.exception("XTTS unload failed")
                    stt_warmed = False
                    xtts_warmed = False
                    _set_process_priority(below_normal=True)
            if stop_event.wait(5.0):
                break

    def _warm_audio():
        try:
            import music as music_mod

            music_mod._ensure_mixer()
            logger.info("Audio stack warmed (mixer).")
        except Exception:
            logger.exception("Audio warm-up failed")

    def _boot_policy():
        try:
            import compute_policy

            rep = compute_policy.device_report()
            heavy = rep.get("ready_for_heavy")
            logger.info(
                "Compute: mode=%s stt=%s tts=%s heavy=%s charging=%s batt=%s",
                rep.get("mode"),
                rep.get("stt"),
                rep.get("tts"),
                heavy,
                rep.get("power", {}).get("is_charging"),
                rep.get("power", {}).get("battery_percent"),
            )
            if not heavy:
                logger.info("Heavy warm skipped until Legion Performance and the real charger")
        except Exception:
            logger.exception("Compute policy report failed")

    threading.Thread(target=_warm_audio, daemon=True, name="KuroAudioWarm").start()
    threading.Thread(target=_boot_policy, daemon=True, name="KuroPolicyBoot").start()
    threading.Thread(target=_heavy_gate_loop, daemon=True, name="KuroHeavyGate").start()

    def _wa_warmup() -> None:
        try:
            from tools.wa_link import warmup

            warmup()
        except Exception:
            logger.exception("WhatsApp link warmup failed")

    threading.Thread(target=_wa_warmup, daemon=True, name="KuroWaLink").start()

    if engine_api.api_requested():
        print("[Kuro] Shell mode. Hotkey stays here.")
    else:
        tray_manager = TrayManager(on_trigger=on_tray_trigger, on_quit=on_quit)
        tray_manager.start()
        print("[Kuro] Tray + overlay ready.")
    print(f"[Kuro] HOLD {config.KURO_HOTKEY}, speak, then RELEASE.")

    if hub_client.configured():
        print(f"[Kuro] Hub: {config.KURO_HUB_URL}")

        def _hub_notify(text: str) -> None:
            if overlay_manager:
                overlay_manager.flash("Kuro", text[:80], hold_ms=2500)

        hub_client.start_event_worker(speak=_speak, notify=_hub_notify)
    else:
        print("[Kuro] Hub: not configured (set KURO_HUB_TOKEN in .env)")

    timing.once("startup", "ready_for_hotkey", timing.since(_BOOT_TICK))
    if engine_api.api_requested():
        engine_api.on_shutdown(on_quit)

        def _dismiss_from_shell() -> None:
            _force_unblock("dismiss")
            if overlay_manager:
                overlay_manager.force_hide()

        engine_api.on_dismiss(_dismiss_from_shell)
        engine_api.ensure_token()
        engine_api.start_background()
    try:
        register_push_to_talk(config.KURO_HOTKEY)
    except KeyboardInterrupt:
        on_quit()


if __name__ == "__main__":
    # Prefer: python run_kuro.py (singleton before heavy imports).
    # Fallback if someone launches main.py directly:
    import singleton as _singleton

    if not _singleton.acquire():
        _singleton.refuse_and_exit()
    main()
