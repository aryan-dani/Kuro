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
from logging.handlers import RotatingFileHandler

import numpy as np
import sounddevice as sd
import soundfile as sf

import config
import phrases
from overlay import OverlayManager, OverlayState
from tray import KuroState, TrayManager

SAMPLE_RATE = 16000
CHANNELS = 1
# Tray-menu fallback only (push-to-talk has no fixed length)
TRAY_RECORD_SECONDS = 4
MIN_PTT_SECONDS = 0.35
MAX_PTT_SECONDS = 30.0
_MIC_DEVICE: int | None = None


def _pick_mic_device() -> int | None:
    """Prefer onboard Realtek mic array over Bluetooth headsets."""
    try:
        devices = sd.query_devices()
        default_in = sd.default.device[0] if isinstance(sd.default.device, (list, tuple)) else sd.default.device
        preferred = []
        for i, d in enumerate(devices):
            if d.get("max_input_channels", 0) <= 0:
                continue
            name = (d.get("name") or "").lower()
            if "realtek" in name and "mic" in name:
                preferred.append((0, i, d["name"]))
            elif "microphone array" in name:
                preferred.append((1, i, d["name"]))
            elif "microphone" in name and "headset" not in name and "hands-free" not in name:
                preferred.append((2, i, d["name"]))
        if preferred:
            preferred.sort()
            idx = preferred[0][1]
            logger.info("Mic device: [%s] %s (default was %s)", idx, preferred[0][2], default_in)
            return idx
        logger.info("Mic device: system default (%s)", default_in)
    except Exception:
        logger.exception("Mic device probe failed")
    return None


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
_ptt_frames: list[np.ndarray] = []
_ptt_stream: sd.InputStream | None = None
_ptt_started_at = 0.0
_ptt_partial_stop = threading.Event()
_ptt_level_tick = 0
_live_transcript = ""
_cancel_speech = threading.Event()
_early_home_fired = False
BUSY_STALE_SECONDS = 12.0


def _force_unblock(reason: str = "reset") -> None:
    """Clear stuck busy/PTT state so the hotkey works again."""
    global is_processing, _ptt_active, _ptt_stream, _ptt_frames, _live_transcript, _early_home_fired
    global _processing_started_at

    logger.warning("Force unblock (%s)", reason)
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


def _speak(text: str, *, fast: bool = False) -> None:
    import tts

    _cancel_speech.clear()
    # Serialize speech so overlapping calls can't double-play
    tts.speak(text, fast=fast)


def _transcribe(audio_path: str, partial: bool = False) -> str:
    """STT with a hard timeout so a hung GPU decode can't lock the hotkey forever."""
    from stt import transcribe

    if not partial:
        try:
            import gpu_boost

            gpu_boost.kick("stt")
        except Exception:
            pass

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

    # Live waveform level (0..1)
    try:
        peak = float(np.max(np.abs(indata.astype(np.float32)))) / 32768.0
        rms = float(np.sqrt(np.mean(indata.astype(np.float32) ** 2))) / 32768.0
        level = min(1.0, max(peak, rms * 2.2))
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
    while not _ptt_partial_stop.wait(0.9):
        with _ptt_lock:
            if not _ptt_active or len(_ptt_frames) < 8:
                continue
            frames = list(_ptt_frames)
        try:
            audio = np.concatenate(frames, axis=0)
            if audio.shape[0] < int(SAMPLE_RATE * 0.8):
                continue
            path = os.path.join(tempfile.gettempdir(), "kuro_partial.wav")
            if audio.ndim == 1:
                audio = audio.reshape(-1, 1)
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
                    if overlay_manager:
                        overlay_manager.set_transcript(kuro="Welcome home, sir.")
                        overlay_manager.set_state(OverlayState.SPEAKING, subtitle="Welcome home")
                    if phrases.fire_daddys_home_parallel():
                        threading.Thread(
                            target=_speak,
                            kwargs={"text": "Welcome home, sir.", "fast": False},
                            daemon=True,
                            name="KuroHomeSpeak",
                        ).start()
        except Exception as e:
            logger.debug("Partial STT skipped: %s", e)


def start_push_to_talk() -> None:
    """Begin recording while the hotkey is held."""
    global _ptt_active, _ptt_frames, _ptt_stream, _ptt_started_at, _ptt_level_tick, _live_transcript, _early_home_fired

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
        try:
            global _MIC_DEVICE
            if _MIC_DEVICE is None:
                _MIC_DEVICE = _pick_mic_device()
            stream_kwargs = dict(
                samplerate=SAMPLE_RATE,
                channels=CHANNELS,
                dtype="int16",
                callback=_ptt_callback,
                blocksize=1024,
            )
            if _MIC_DEVICE is not None:
                stream_kwargs["device"] = _MIC_DEVICE
            _ptt_stream = sd.InputStream(**stream_kwargs)
            _ptt_stream.start()
        except Exception:
            _ptt_active = False
            _ptt_stream = None
            logger.exception("Failed to open microphone stream")
            if overlay_manager:
                overlay_manager.set_state(OverlayState.ERROR, subtitle="Mic failed to open", hold_ms=3000)
            return

    _ptt_partial_stop.clear()
    threading.Thread(target=_partial_transcribe_loop, daemon=True, name="KuroPartialSTT").start()

    logger.info("PTT recording started.")
    try:
        import gpu_boost

        gpu_boost.kick("ptt")
    except Exception:
        pass
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

    elapsed = time.time() - started
    peak = 0
    if frames:
        try:
            peak = int(np.max(np.abs(np.concatenate(frames, axis=0))))
        except Exception:
            peak = 0
    logger.info("PTT released after %.2fs (%d chunks, peak=%s)", elapsed, len(frames), peak)

    if peak < 80:
        logger.warning("Mic peak very low (%s) — check Windows mic privacy / input device", peak)
        if overlay_manager:
            overlay_manager.set_state(
                OverlayState.ERROR,
                subtitle="Mic too quiet — check Windows mic settings",
                hold_ms=3000,
            )
        if tray_manager:
            tray_manager.set_state(KuroState.IDLE)
        return

    if elapsed < MIN_PTT_SECONDS or not frames:
        logger.info("Clip too short — ignored.")
        if overlay_manager:
            overlay_manager.set_state(
                OverlayState.ERROR,
                subtitle="Too short — keep holding while you talk",
                hold_ms=2500,
            )
        if tray_manager:
            tray_manager.set_state(KuroState.IDLE)
        return

    audio = np.concatenate(frames, axis=0)
    max_samples = int(MAX_PTT_SECONDS * SAMPLE_RATE)
    if audio.shape[0] > max_samples:
        audio = audio[:max_samples]

    path = _save_wav(audio)
    threading.Thread(
        target=process_audio_file,
        args=(path,),
        daemon=True,
        name="KuroProcess",
    ).start()


def record_fixed(duration: float) -> str:
    """Fixed-length capture for tray menu trigger."""
    logger.info("Fixed record for %.1fs (tray)", duration)
    recording = sd.rec(
        int(duration * SAMPLE_RATE),
        samplerate=SAMPLE_RATE,
        channels=CHANNELS,
        dtype=np.int16,
    )
    sd.wait()
    return _save_wav(recording)


def process_audio_file(audio_file: str) -> None:
    """Transcribe -> fast phrases / brain -> speak."""
    global is_processing, _live_transcript, _early_home_fired, _processing_started_at
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

        handled = phrases.try_handle(user_prompt)
        if handled is not None:
            reply = handled.reply
            skip_tts = handled.skip_tts
            after_speak = handled.after_speak
            force_fast = handled.force_fast_tts
            if overlay_manager:
                overlay_manager.set_state(OverlayState.THINKING, subtitle="On it...")
        else:
            mode = config.KURO_LLM_MODE
            if mode == "off":
                reply = "No command matched."
                skip_tts = False
                after_speak = None
                force_fast = False
                if overlay_manager:
                    overlay_manager.set_state(OverlayState.THINKING, subtitle="No match")
            else:
                # Lazy Ollama — only imported on freeform fallback / always
                import brain

                if overlay_manager:
                    overlay_manager.set_state(OverlayState.THINKING, subtitle="Thinking...")
                reply = brain.think(user_prompt)
                skip_tts = False
                after_speak = None
                force_fast = False

        logger.info('Kuro: "%s"', reply)

        if tray_manager:
            tray_manager.set_state(KuroState.SPEAKING)
        if overlay_manager:
            overlay_manager.set_transcript(kuro=reply)
            overlay_manager.set_state(OverlayState.SPEAKING, subtitle="Speaking...")

        if not skip_tts:
            _speak(reply, fast=force_fast)
        if after_speak:
            try:
                after_speak()
            except Exception:
                logger.exception("after_speak failed")

        if overlay_manager:
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
            if key == pk.Key.esc:
                logger.info("Escape — force unblock + hide overlay")
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

    if overlay_manager:
        overlay_manager.set_state(
            OverlayState.READY,
            subtitle=f"Hold {hotkey_str}",
            hold_ms=2500,
        )

    while not stop_event.is_set():
        time.sleep(0.5)
    listener.stop()


def on_quit() -> None:
    logger.info("Shutting down...")
    print("\n[Kuro] Shutting down...")
    stop_event.set()
    try:
        stop_push_to_talk()
    except Exception:
        pass
    if overlay_manager:
        overlay_manager.stop()
    try:
        import gpu_boost

        gpu_boost.stop()
    except Exception:
        pass
    os._exit(0)


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

    overlay_manager = OverlayManager()
    overlay_manager.start()

    # Warm STT in background so live hearing is snappy
    def _warm_stt():
        try:
            from stt import transcribe  # noqa: F401
            logger.info("STT ready for live hearing.")
        except Exception:
            logger.exception("STT warm-up failed")

    def _warm_audio():
        try:
            import music as music_mod

            music_mod._ensure_mixer()
            logger.info("Audio stack warmed (mixer).")
        except Exception:
            logger.exception("Audio warm-up failed")

    def _warm_xtts():
        try:
            import tts

            print("[Kuro] Loading natural voice (XTTS) onto GPU...")
            tts._warm_xtts(blocking=True)
            print("[Kuro] Natural voice ready.")
        except Exception:
            logger.exception("XTTS warm-up failed")

    def _start_gpu_boost():
        try:
            import gpu_boost

            gpu_boost.start()
            rep = gpu_boost.device_report()
            print(
                f"[Kuro] GPU: cuda={rep.get('cuda')} "
                f"stt={rep.get('stt_device')} "
                f"charging={rep.get('power', {}).get('is_charging')} "
                f"batt={rep.get('power', {}).get('battery_percent')}%"
            )
        except Exception:
            logger.exception("GPU boost failed to start")

    threading.Thread(target=_warm_stt, daemon=True, name="KuroSTTWarm").start()
    threading.Thread(target=_warm_audio, daemon=True, name="KuroAudioWarm").start()
    threading.Thread(target=_warm_xtts, daemon=True, name="KuroXTTSWarm").start()
    threading.Thread(target=_start_gpu_boost, daemon=True, name="KuroGpuBoostBoot").start()

    tray_manager = TrayManager(on_trigger=on_tray_trigger, on_quit=on_quit)
    tray_manager.start()
    print("[Kuro] Tray + overlay ready.")
    print(f"[Kuro] HOLD {config.KURO_HOTKEY}, speak, then RELEASE.")

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
