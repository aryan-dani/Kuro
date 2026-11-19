# Music feature

Pause, resume, and stop via `MusicControl`. Default registration uses
`NoopMusicControl` for `spoken_for_intent`. Live actuation uses
`ToolsMusicControl` in `kuro-router/src/adapters.rs`.