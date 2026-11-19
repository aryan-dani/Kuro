# Release checklist

1. `cargo xtask gate full` green on Windows.
2. Version bump in `kuro-rs/Cargo.toml` `[workspace.package]`; stamp shell to match.
3. `CHANGELOG.md` entry for the tag.
4. Verify `LICENSE`, `THIRD_PARTY_NOTICES.md`, and `models.lock.toml` licenses.
5. Tag `vX.Y.Z`; attach binaries without models.
6. Protocol version bumps only with a new `contract/vN/` and ADR.
