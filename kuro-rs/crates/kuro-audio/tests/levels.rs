use kuro_audio::{FollowUpListen, PttBuffer, levels_11, resample_linear, to_mono};

#[test]
fn levels_are_eleven_and_clamped() {
    let quiet = vec![0.0; 1100];
    let loud = vec![1.0; 1100];
    let q = levels_11(&quiet);
    let l = levels_11(&loud);
    assert_eq!(q.len(), 11);
    assert!(q.iter().all(|v| *v == 0.0));
    assert!(l.iter().all(|v| *v <= 1.0 && *v > 0.5));
    assert_eq!(levels_11(&[]), [0.0; 11]);
}

#[test]
fn resample_and_mono() {
    let stereo = vec![0.0, 1.0, 0.0, 1.0];
    assert_eq!(to_mono(&stereo, 2), vec![0.5, 0.5]);
    let up = resample_linear(&[0.0, 1.0], 8_000, 16_000);
    assert!(up.len() >= 3);
}

#[test]
fn follow_up_stops_on_epoch_loss_silence_and_cap() {
    let mut mic = FollowUpListen::default();
    assert!(mic.tick(0.0, 400, true));
    assert!(mic.tick(0.1, 100, true));
    assert!(!mic.tick(0.0, 700, true));
    let mut mic = FollowUpListen::default();
    assert!(!mic.tick(0.2, 100, false));
    let mut mic = FollowUpListen { max_ms: 1000, ..FollowUpListen::default() };
    assert!(!mic.tick(0.0, 1000, true));
}

#[test]
fn partial_window_after_point_eight_seconds() {
    let (mut ptt, _) = PttBuffer::new();
    ptt.push(&vec![0.1; 12_000]);
    assert!(ptt.take_partial().is_none());
    ptt.push(&vec![0.1; 1_000]);
    assert!(ptt.take_partial().is_some());
}
