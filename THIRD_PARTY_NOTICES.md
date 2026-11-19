# Third-party notices

Kuro distributes source, fonts, and scripts. Models and some native tools are
downloaded or installed by the user and are not redistributed from this repo
unless listed in `models.lock.toml` with a verified license.

| Component | Role | License (SPDX / notes) | Template action |
|-----------|------|------------------------|-----------------|
| Whisper GGML models | STT download | MIT (verify upstream) | `models.lock.toml` |
| Kokoro-82M ONNX | TTS download | Apache-2.0 (verify voices) | `models.lock.toml` |
| bge-small-en-v1.5 | router embed | MIT (verify) | `models.lock.toml` |
| Silero VAD | VAD download | MIT (verify) | `models.lock.toml` |
| Piper + voices | optional TTS process | code and voices vary; some Piper builds GPL | user install |
| espeak-ng | optional TTS process | GPL-3.0-or-later | not linked; optional |
| neonize / whatsmeow | WhatsApp adapter DLL | Apache-2.0 / MPL-2.0; unofficial client | off by default |
| Space Grotesk, JetBrains Mono, Poppins | bundled fonts | OFL-1.1 | texts under Assets/Fonts |
| Windows App SDK, .NET | shell | MIT | list |
| Rust crates | linked | mostly MIT / Apache-2.0 | `cargo deny` later |

Verify each SPDX identifier against the upstream repository before a public
release. Arrival / ritual audio is optional and lives only in the user profile
(`profile/sounds/arrive.*`); no copyrighted track name is shipped in code.
