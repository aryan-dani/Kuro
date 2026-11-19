# kuro-triggers

Trigger port for PTT hotkey wrappers and wake word adapters (phase 25).

## Status

- `NullWakeWord` is the default engine adapter: no polling stream, no activations.
- `FakeDetector` is for unit tests only (one-shot activation).
- A redistributable on-device wake word model is **not** wired yet; phase 25 ships the
  registry, settings knobs (`wake_word.enabled`, optional `wake_word.model`), and engine
  `trigger_bridge` mapping wake activations to `HotkeyDown`. Hold-to-talk remains default.
- Opt in with profile `[wake_word] enabled = true` or `KURO_WAKE_WORD=1`. Real inference,
  cooldown, and open-mic capture land after license-verified model selection (ADR 0013).

## Commands

```powershell
cargo test -p kuro-triggers
```