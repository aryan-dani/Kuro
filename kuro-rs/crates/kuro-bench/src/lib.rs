//! Offline voice recording and STT scoring. Never routes live tools.

mod labels;
mod mine;
mod prompts;
mod prompts_more;
mod score;
mod wav;

pub use labels::{ClipLabel, append_label, known_ids, load_labels};
pub use mine::{command_attempt_miss, mine_logs, mine_stt_lines};
pub use prompts::{PromptSpec, catalog_for_session, conditions_for_session};
pub use score::{ClipResult, EvalSummary, percentile, render_summary, score_clip, summarize, wer};
pub use wav::{read_wav_f32, write_wav_f32};

pub fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("..")
}

pub fn voice_dir() -> std::path::PathBuf {
    repo_root().join("bench").join("data").join("voice")
}
