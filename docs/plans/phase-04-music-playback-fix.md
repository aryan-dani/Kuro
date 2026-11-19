# Phase 04: Music playback fix

- Size: S (one PR)
- Depends on: phases 00, 01
- Behavior change: yes, deliberate. Music keeps playing after it starts and obeys
  pause, resume, and stop.
- ADRs: 0006 (error kinds), 0015 (why this is early)
- Protected paths touched: none

## Goal

Make the music feature real. Today `play_file` drops the output stream when it
returns and the control functions only flip flags (finding 1; unverified at
runtime, so step 1 confirms it).

## Scope

In: `kuro-rs/crates/kuro-tools/src/music.rs` and its tests.
Out: moving music into the new feature model (phase 10), ducking, playlists, any
phrase or reply change, the song path convention (phase 08).

## Context card

Read:
- `kuro-rs/crates/kuro-tools/src/music.rs` (136 lines): `MUSIC`, `CANDIDATES`,
  `play_file`, `pause_song`, `resume_song`, `stop_song`, `apply_action`
- `kuro-rs/crates/kuro-router/src/phrases.rs`: the callers (`is_stop_music` and the
  pause, resume, stop arms; `fire_daddys_home`) - read only
- `kuro-rs/fixtures/phrases/music.toml`

Edit: `kuro-rs/crates/kuro-tools/src/music.rs`
Create: `kuro-rs/crates/kuro-tools/src/music_player.rs` (under 200 lines),
`kuro-rs/crates/kuro-tools/tests/music_player.rs`
Do not touch: `phrases.rs`, the corpus expectations, `kuro-tts/**`, `kuro-audio/**`.

Commands:
```powershell
cargo test -p kuro-tools --test music_player
cargo xtask gate fast
```

## Tests written first

A `SinkLike` trait (play, pause, resume, stop, is_empty) with a recording fake.

| Test | Asserts |
|------|---------|
| `stream_outlives_play_call` | after `play` returns, the fake output is still alive until `stop` |
| `pause_resume_stop_reach_sink` | each command is received by the fake in order |
| `stop_is_idempotent` | stop twice, and stop with nothing playing, are not errors |
| `missing_track_is_reported` | play with a missing path returns the not-configured error |
| `bad_file_is_reported` | play with an undecodable file returns the invalid-input error |
| `no_device_is_reported` | the factory failing returns the device-unavailable error |
| `play_replaces_current` | a second play stops the first |

## Steps

1. Confirm the defect: write `stream_outlives_play_call` against the current code
   structure and see it fail (or report that the finding is wrong and stop).
2. `music_player.rs`: a `MusicPlayer` that owns one thread. The thread owns the
   `rodio::OutputStream` and `Sink` for as long as something may play, and receives
   `Play(path)`, `Pause`, `Resume`, `Stop` over a bounded channel (capacity 8). The
   thread is started lazily on the first play and exits on drop.
3. Errors are a small enum (`NotConfigured`, `DeviceUnavailable`, `InvalidInput`)
   returned to the caller.
4. `music.rs` keeps its public functions and their return strings exactly, and
   delegates to a `MusicPlayer` stored in the existing `MUSIC` state. The global
   stays for now; it is on the ratchet list and goes away in phase 10.
5. Map errors to the existing reply strings. Where today's code spoke a success
   that did not happen, keep the wording and add a log line; changing replies is
   out of scope.

## Acceptance criteria

- The seven tests pass with the fake; no test opens a real audio device.
- The corpus is unchanged.
- Owner hardware check (the agent cannot do this): trigger the arrival ritual, hear
  the track continue past 5 seconds, say "pause the song", "resume", "stop music",
  and confirm each takes effect.
- `music.rs` and `music_player.rs` are each under 200 lines.

## Rollback

Revert the PR. No data or config is migrated.

## Guardrails

- No new statics. No `unwrap` on device or file results.
- Do not change which file is chosen, which phrases match, or what is spoken.
- The player thread must stop within 500 ms of `Stop` or drop; test with the fake.
