# desk

Reversible desk smart-plug scenes: power on, power off, good night.

`DeskControl` is the actuation port. Default registration uses `NoopDeskControl`
so `spoken_for_intent` stays deterministic in tests. Live actuation uses
`ToolsDeskControl` in `kuro-router/src/adapters.rs`, which calls
`control_smart_plug` and maps results through this feature's `execute`.

Manifest data lives in `feature.toml`. Register lines are in `features/mod.rs`.