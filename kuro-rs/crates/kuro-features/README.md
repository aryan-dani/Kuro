# kuro-features

One folder per voice feature, explicit registration in `src/features/mod.rs`,
and typed `Outcome` / `FeatureError` at the boundary (ADR 0003, 0006).

During migration, `kuro-tools` remains the production execution path. Phase 10 slice
one moved read-only spoken features into `src/features/`; see `src/features/README.md`
for the remaining list.

`execute_spoken` exercises the registry bridge in tests only until the router calls it.

See `docs/guides/add-a-feature.md` for the agent workflow.