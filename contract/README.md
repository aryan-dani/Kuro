# Protocol contract fixtures

Golden JSON under `contract/v1/` freezes today's localhost API and HUD event
shapes. Rust wire types live in `kuro-rs/crates/kuro-protocol`; `cargo test -p
kuro-protocol` round-trips HUD fixtures. `kuro-ipc` still owns live HTTP snapshots
until ipc depends on the crate; the C# side joins in later phase 15 slices.

These files become PROTECTED from phase 01 onward (see ADR 0009 and
`xtask/data/protected_paths.txt`). Breaking changes need a new `vN/` folder and
an ADR trailer.

Volatile fields (`pid`, `uptime`, absolute log paths) are replaced with the
string `"<volatile>"` in fixtures. Set `KURO_BLESS=1` when regenerating from
`cargo test -p kuro-ipc --test contract_snapshot`.
