# Add a feature

Stub for phase 09. Full steps arrive with `cargo xtask new-feature` in phase 11.

1. Copy `kuro-rs/crates/kuro-features/src/features/time/` as a template.
2. Edit `feature.toml` (`id`, intents, replies, tier).
3. Implement `Feature` in `mod.rs` using `ctx.clock`, `ctx.http` (later), and no statics.
4. Add tests in `tests.rs` with fakes from `kuro_features::context`.
5. Register in `features/mod.rs`: one `pub mod` line and one `registry.register` line
   between the `xtask` markers.
6. Run `cargo test -p kuro-features`.

Production routing still goes through `kuro-tools` until phase 10 migrates execution.