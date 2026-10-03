# Custom Voice Cloning for Kuro (XTTS-v2)

Kuro's TTS engine is powered by **Coqui XTTS-v2** running on your NVIDIA RTX 5070 GPU.

## How to use a Custom Voice:

1. Record or obtain a **~6 to 15-second** clean audio sample of the voice you want (e.g. Jarvis, a favorite movie character, or yourself).
   - Format: `.wav` (or `.mp3`), 24kHz or 44.1kHz, clear speech with minimal background noise.
2. Place the file in this folder, e.g.:
   `voices/kuro_voice.wav`
3. Open `.env` and set:
   ```env
   XTTS_SPEAKER_WAV="voices/kuro_voice.wav"
   ```
4. Restart Kuro (or trigger via hotkey). Kuro will automatically speak using that cloned voice!

If `XTTS_SPEAKER_WAV` is left blank or the file does not exist, Kuro falls back to a clean built-in default speaker (`Daisy Studious`).
