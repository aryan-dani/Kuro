# features/

Phase 10 first slice (migrated folders):

| id | kuro-tools / router source | status |
|----|---------------------------|--------|
| cancel | `phrases.rs` `Intent::Cancel` | folder + tests |
| thanks | `phrases.rs` `Intent::Thanks` | folder + tests |
| help | `phrases.rs` `Intent::Help` | folder + tests |
| identity | `phrases.rs` `Intent::Identity` | folder + tests |
| time | `phrases.rs` `clock_reply` | folder + bridge format |
| recall | phase 20 turn log | folder + in-memory log |
| calendar | phase 24 ICS read | folder + minimal parser |
| system | phase 26B control | folder + fake control |
| weather | phase 26C conditions | folder + fixture JSON |
| morning_brief | phase 26C ritual compose | folder + ritual.toml |

Remaining migration (still in `kuro-tools` or `phrases.rs`):

| id | module |
|----|--------|
| date | `phrases.rs` `date_reply` |
| desk / plug | `plug.rs`, power intents |
| music | `music.rs`, `music_player.rs` |
| news | `feeds.rs` `get_news` |
| inbox | `feeds.rs` `check_inbox` |
| read_screen | `feeds.rs` / `screen.rs` |
| meet | `meet.rs` |
| whatsapp | `whatsapp.rs`, `wa_neonize.rs` |
| workflows | `n8n.rs` |
| hub | `hub.rs` |
| brain / llm tool | `brain.rs` |

Production execution still uses `kuro-tools::ToolRegistry` and `kuro-router`
`try_handle`. Call `kuro_features::execute_spoken` in tests to exercise the registry
bridge until router wiring lands.