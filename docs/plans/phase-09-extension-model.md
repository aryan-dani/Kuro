# Phase 09: Feature extension model

- Size: L
- Depends on: phase 08
- Behavior change: none intended. Introduce `kuro-features` crate and Feature trait;
  registry API; keep `kuro-tools` as a facade or migrate one sample feature only.
- ADRs: 0003, 0004, 0006
- Protected paths: none unless router exports change. Trailers as needed.

## Goal

Land the extension model: one folder per feature, `feature.toml`, explicit
registration in `features/mod.rs`, FeatureContext ports, typed FeatureError.
Prove with one migrated sample (desk power or time) before wholesale migration.

## Scope

In: `kuro-features` crate skeleton; `Feature` trait; `Outcome`; `FeatureContext`
(http, clock, store stubs); `feature.toml` schema; registry `register` / `all`;
DynamicSource trait stub; one sample feature folder; docs/guides/add-a-feature.md
stub; rename cursor rule tools.mdc to features.mdc when ready.

Out: Migrating all tools (phase 10); scaffolders (phase 11); turn machine (12).

## Context card

Read:
- `docs/adr/0003-feature-extension-model.md`
- `docs/adr/0004-data-driven-phrases-and-rituals.md`
- `docs/adr/0006-typed-errors-and-spoken-replies.md`
- `docs/plans/01-architecture.md` extension model sections
- `kuro-rs/crates/kuro-tools/src/lib.rs` (ToolRegistry, execute_tool)

Create:
- `kuro-rs/crates/kuro-features/` with `src/lib.rs`, `registry.rs`, `context.rs`,
  `error.rs`, `features/mod.rs`, `features/<sample>/` (mod.rs, feature.toml,
  README.md, tests.rs)
- `docs/guides/add-a-feature.md` stub

Edit: workspace Cargo.toml; optionally thin re-export from kuro-tools.

Do not touch: full phrase TOML migration; shell; hotkey.

Commands:
```powershell
cargo test -p kuro-features
cargo xtask gate fast
```

## Tests first

- Sample feature execute with FakeClock / FakeHttp returns expected Outcome.
- Registry lists sample; unknown id returns typed error.
- feature.toml parse rejects missing id.

## Steps

1. Define traits and types in kuro-core or kuro-features per ADR 0003.
2. Implement registry with two-line registration pattern documented.
3. Port one simple tool as the sample; keep old path working via bridge.
4. Document: agents edit only `features/<id>/` plus two registry lines.

## Acceptance

- Agent-ability for "add a tool" path documented with sample.
- Existing product path still works (bridge).
- Gate + corpus green.

## Risks

- Big-bang rewrite: do not delete kuro-tools in this phase.
- Circular deps with router: features must not depend on router.
