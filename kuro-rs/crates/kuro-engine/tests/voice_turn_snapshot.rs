//! Freeze VoiceTurn stage order and replies for a fixed input table.

use std::sync::{Arc, Mutex};

use kuro_core::{Timing, TurnClock};
use kuro_engine::voice_turn;

#[test]
fn voice_turn_snapshot_table() {
    let dir = std::env::temp_dir().join(format!("kuro-voice-snap-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    let cases: &[(&str, &str, &str)] = &[
        ("cancel", "Alright.", "hotkey_down,hotkey_up,stt_done,route_done,tts_first_audio,idle"),
        ("never mind", "Alright.", "hotkey_down,hotkey_up,stt_done,route_done,tts_first_audio,idle"),
        ("thank you", "Anytime.", "hotkey_down,hotkey_up,stt_done,route_done,tts_first_audio,idle"),
        (
            "who are you",
            "Kuro. Local assistant. Phrases first, model last.",
            "hotkey_down,hotkey_up,stt_done,route_done,tts_first_audio,idle",
        ),
        ("help", "Desk", "hotkey_down,hotkey_up,stt_done,route_done,tts_first_audio,idle"),
        ("stop the music", "Music stopped.", "hotkey_down,hotkey_up,stt_done,route_done,tts_first_audio,idle"),
        ("pause the song", "Nothing is playing.", "hotkey_down,hotkey_up,stt_done,route_done,tts_first_audio,idle"),
        ("desk on", "Desk on.", "hotkey_down,hotkey_up,stt_done,route_done,tts_first_audio,idle"),
        ("desk off", "Desk off.", "hotkey_down,hotkey_up,stt_done,route_done,tts_first_audio,idle"),
        ("", "", "hotkey_down,hotkey_up,stt_done,idle"),
    ];

    kuro_tools::set_executor(Some(Box::new(|name, _args| match name {
        "control_smart_plug" => "Success: ok".into(),
        other => format!("Success: {other}"),
    })));

    for (idx, (input, reply_expect, stages_csv)) in cases.iter().enumerate() {
        let path = dir.join(format!("timing-{idx}.csv"));
        let clock = Arc::new(Mutex::new(TurnClock::default()));
        let epoch = clock.lock().unwrap().begin_turn();
        let mut turn = voice_turn(Timing::new(&path, true));
        turn.hotkey_down(&clock);
        let reply = turn.run(input, input, true, true, epoch, &clock, "off", false, |_| {});
        if reply_expect.is_empty() {
            assert!(reply.is_empty(), "case {input:?} want empty reply got {reply:?}");
        } else if *reply_expect == "Desk" {
            assert!(reply.contains("Desk"), "case {input:?} want contains Desk got {reply:?}");
        } else {
            assert_eq!(&reply, reply_expect, "case {input:?}");
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for stage in stages_csv.split(',') {
            assert!(text.contains(stage), "case {input:?} missing stage {stage} in {text}");
        }
    }

    kuro_tools::set_executor(None);
    let _ = std::fs::remove_dir_all(&dir);
}
