use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use crate::music_player::{MusicPlayer, PlayError};

static MUSIC: std::sync::LazyLock<Mutex<MusicState>> = std::sync::LazyLock::new(|| Mutex::new(MusicState::default()));

struct MusicState {
    player: MusicPlayer,
    paused: bool,
    busy: bool,
    path: Option<PathBuf>,
}

impl Default for MusicState {
    fn default() -> Self {
        Self { player: MusicPlayer::new(), paused: false, busy: false, path: None }
    }
}

const CANDIDATES: &[&str] = &["arrive.mp3", "arrive.wav", "arrival.mp3", "arrival.wav"];

pub fn find_local_song() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("DADDY_HOME_SONG_PATH") {
        let env_path = env_path.trim().trim_matches('"');
        let path = PathBuf::from(env_path);
        if path.is_file() {
            return Some(path);
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = root.join("..").join("..").join("..");
    for folder in [repo.join("sounds"), repo.join("music"), repo.clone()] {
        for name in CANDIDATES {
            let candidate = folder.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn device_ok() -> bool {
    rodio::OutputStream::try_default().is_ok()
}

fn play_err_text(path: &Path, err: PlayError) -> String {
    match err {
        PlayError::NotConfigured => format!("File not found: {}", path.display()),
        PlayError::DeviceUnavailable => "Music player failed to start.".into(),
        PlayError::InvalidInput(e) => format!("Could not start music ({e})."),
    }
}

pub fn pause_song() -> String {
    let mut state = MUSIC.lock().unwrap_or_else(|e| e.into_inner());
    if !device_ok() && !state.busy && !state.paused {
        return "Music player is unavailable.".into();
    }
    if !state.busy && !state.paused {
        return "Nothing is playing.".into();
    }
    state.player.pause();
    state.paused = true;
    state.busy = false;
    "Paused.".into()
}

pub fn resume_song() -> String {
    let mut state = MUSIC.lock().unwrap_or_else(|e| e.into_inner());
    if !state.paused && !state.busy {
        if let Some(path) = state.path.clone().or_else(find_local_song) {
            drop(state);
            return play_file(&path);
        }
        return "Nothing to resume.".into();
    }
    state.player.resume();
    state.paused = false;
    state.busy = true;
    "Resumed.".into()
}

pub fn stop_song() -> String {
    let mut state = MUSIC.lock().unwrap_or_else(|e| e.into_inner());
    state.player.stop();
    state.paused = false;
    state.busy = false;
    state.path = None;
    "Music stopped.".into()
}

pub fn play_file(path: &Path) -> String {
    if !path.is_file() {
        return format!("File not found: {}", path.display());
    }
    if rodio::OutputStream::try_default().is_err() {
        return "Music player failed to start.".into();
    }
    let mut state = MUSIC.lock().unwrap_or_else(|e| e.into_inner());
    match state.player.play(path) {
        Ok(()) => {
            state.paused = false;
            state.busy = true;
            state.path = Some(path.to_path_buf());
            format!("Playing {}", path.file_name().and_then(|s| s.to_str()).unwrap_or("track"))
        }
        Err(err) => play_err_text(path, err),
    }
}

pub fn play_should_i_stay_or_should_i_go() -> String {
    match find_local_song() {
        Some(path) => play_file(&path),
        None => "I could not find the song.".into(),
    }
}

pub fn apply_action(action: &Value) {
    if action.get("type").and_then(|v| v.as_str()) != Some("music") {
        return;
    }
    match action.get("op").and_then(|v| v.as_str()).unwrap_or("") {
        "pause" => {
            let _ = pause_song();
        }
        "resume" => {
            let _ = resume_song();
        }
        "stop" => {
            let _ = stop_song();
        }
        "play" if action.get("track").and_then(|v| v.as_str()) == Some("daddy_home") => {
            let _ = play_should_i_stay_or_should_i_go();
        }
        _ => {}
    }
}
