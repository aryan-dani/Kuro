//! Music player tests using a recording fake (no real audio device).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use kuro_tools::music_player::{MusicPlayer, PlayError, SinkLike};

struct RecordingSink {
    log: Arc<Mutex<Vec<&'static str>>>,
    alive: Arc<AtomicBool>,
    fail_decode: bool,
}

impl SinkLike for RecordingSink {
    fn play(&mut self, _path: &Path) -> Result<(), PlayError> {
        self.log.lock().unwrap().push("play");
        if self.fail_decode {
            return Err(PlayError::InvalidInput("undecodable".into()));
        }
        Ok(())
    }
    fn pause(&self) {
        self.log.lock().unwrap().push("pause");
    }
    fn resume(&self) {
        self.log.lock().unwrap().push("resume");
    }
    fn stop(&mut self) {
        self.log.lock().unwrap().push("stop");
        self.alive.store(false, Ordering::SeqCst);
    }
    fn is_empty(&self) -> bool {
        false
    }
}

fn temp_file(bytes: &[u8]) -> PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("kuro-music-test-{}-{n}.bin", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

fn player_with(
    alive: Arc<AtomicBool>,
    fail_device: bool,
    fail_decode: bool,
) -> (MusicPlayer, Arc<Mutex<Vec<&'static str>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let log_in = log.clone();
    let alive_in = alive.clone();
    let player = MusicPlayer::new_injectable(move || {
        if fail_device {
            return Err(PlayError::DeviceUnavailable);
        }
        alive_in.store(true, Ordering::SeqCst);
        Ok(Box::new(RecordingSink { log: log_in.clone(), alive: alive_in.clone(), fail_decode }))
    });
    (player, log)
}

#[test]
fn stream_outlives_play_call() {
    let alive = Arc::new(AtomicBool::new(false));
    let (mut player, _) = player_with(alive.clone(), false, false);
    let path = temp_file(b"fake-audio");
    player.play(&path).unwrap();
    assert!(alive.load(Ordering::SeqCst));
    player.stop();
    assert!(!alive.load(Ordering::SeqCst));
    let _ = std::fs::remove_file(path);
}

#[test]
fn pause_resume_stop_reach_sink() {
    let alive = Arc::new(AtomicBool::new(false));
    let (mut player, log) = player_with(alive, false, false);
    let path = temp_file(b"fake-audio");
    player.play(&path).unwrap();
    player.pause();
    player.resume();
    player.stop();
    assert_eq!(*log.lock().unwrap(), ["play", "pause", "resume", "stop"]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn stop_is_idempotent() {
    let alive = Arc::new(AtomicBool::new(false));
    let (mut player, _) = player_with(alive, false, false);
    player.stop();
    player.stop();
    let path = temp_file(b"fake-audio");
    player.play(&path).unwrap();
    player.stop();
    player.stop();
    let _ = std::fs::remove_file(path);
}

#[test]
fn missing_track_is_reported() {
    let alive = Arc::new(AtomicBool::new(false));
    let (mut player, _) = player_with(alive, false, false);
    let missing = PathBuf::from("/no/such/kuro-track.mp3");
    assert_eq!(player.play(&missing), Err(PlayError::NotConfigured));
}

#[test]
fn bad_file_is_reported() {
    let alive = Arc::new(AtomicBool::new(false));
    let (mut player, _) = player_with(alive, false, true);
    let path = temp_file(b"not-audio");
    assert!(matches!(player.play(&path), Err(PlayError::InvalidInput(_))));
    let _ = std::fs::remove_file(path);
}

#[test]
fn no_device_is_reported() {
    let alive = Arc::new(AtomicBool::new(false));
    let (mut player, _) = player_with(alive, true, false);
    let path = temp_file(b"fake-audio");
    assert_eq!(player.play(&path), Err(PlayError::DeviceUnavailable));
    let _ = std::fs::remove_file(path);
}

#[test]
fn play_replaces_current() {
    let alive = Arc::new(AtomicBool::new(false));
    let (mut player, log) = player_with(alive, false, false);
    let a = temp_file(b"a");
    let b = temp_file(b"b");
    player.play(&a).unwrap();
    player.play(&b).unwrap();
    let entries = log.lock().unwrap().clone();
    assert_eq!(entries, ["play", "stop", "play"]);
    player.stop();
    let _ = std::fs::remove_file(a);
    let _ = std::fs::remove_file(b);
}
