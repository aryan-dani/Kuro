# Baseline measurements

Recorded during phase 00 / 01 setup. Machine class: Windows 10/11 x64 developer
workstation (no personal identifiers). Re-measure before treating numbers as
ratchets.

| Metric | Command | Value | Notes |
|--------|---------|-------|-------|
| Phrase corpus | `cargo test -p kuro-router --test corpus` | ~13 s local | 293 cases; model skips printed |
| xtask unit tests | `cargo test -p xtask` | 6 passed | phase 01 |
| check size | `cargo xtask check size` | ok | soft warnings for allowlisted files |
| check text | `cargo xtask check text` | ok | no emoji / em dash |
| Full gate | `cargo xtask gate full` / `Test-KuroGate.ps1` | not timed this run | must not kill/restart live Kuro |
| Silent model skips | kokoro / semantic / cache tests | present historically | explicit skip accounting lands with testkit |

Cold and warm release build times: not measured in this pass; fill before phase 21
bench ratchets.

Route microbench (no models): `cargo xtask bench` or `cargo xtask bench router`;
writes JSON under `kuro-rs/target/bench/`.
