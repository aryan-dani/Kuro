# Architecture

Kuro is a **Windows product** with a **portable Rust core**. See
[docs/adr/0001](docs/adr/0001-portable-core-not-cross-platform.md).

## Layers

```
KuroShell (WinUI)  --IPC 127.0.0.1:8765-->  kuro-engine (composition root)
                                              |
                    kuro-router (decide) -----+---- kuro-tools / features (execute)
                                              |
                         kuro-stt / kuro-tts / kuro-audio / kuro-hotkey
```

- The router never executes tools.
- Only config (phase 07) should read environment variables.
- One supervisor owns the engine process (phase 18).

## Where things live

| Concern | Today | Target |
|---------|-------|--------|
| Phrases / tools | `kuro-router`, `kuro-tools` | `kuro-features/<id>/` |
| Turn loop | `kuro-engine` voice | `kuro-turn` |
| Wire types | `kuro-ipc` + shell DTOs | `kuro-protocol` + `contract/v1` |
| Settings | env vars | `kuro-config` + profiles |

Decisions: [docs/adr/](docs/adr/README.md). Plan index: [docs/plans/README.md](docs/plans/README.md).
