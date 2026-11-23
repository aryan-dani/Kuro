mod api;
mod boot;
mod config_log;
mod hud;
mod meet_followup_route;
mod panic_hook;
mod ptt_loop;
mod ptt_release;
mod settings_wire;
mod speech;
mod trigger_bridge;
mod turn_agent;
mod turn_driver;
mod turn_effects;
mod turn_gate;
mod util;
mod voice;

pub use boot::{
    exit_gate_ci, load_engine_env, log_hotkey_arm, shutdown_hook, spawn_heartbeat, spawn_initial_speech_warm,
    spawn_wifi_speech_watch,
};
pub use ptt_loop::{PttArms, arm_ptt_hotkey};
pub use speech::{ollama_up, unload_speech, warm_speech};
pub use util::{gate_ci_mode, log_line};

pub use config_log::log_loaded_settings;
pub use hud::{build_gate, set_hud};
pub use settings_wire::{agent_config_from_settings, engine_config_from_settings, load_and_apply_runtime_settings};
pub use trigger_bridge::{build_wake_registry, log_wake_boot, poll_wake_turn_event, wake_would_open_mic};
pub use turn_agent::AgentHook;

pub use api::{bind_with_retry, spawn_api_thread, try_bind};
pub use panic_hook::install_panic_hook;
pub use turn_gate::TurnGate;
pub use voice::VoiceTurn;

pub fn voice_turn(timing: kuro_core::Timing) -> VoiceTurn {
    VoiceTurn::new(timing)
}

#[cfg(test)]
mod tests {
    use kuro_tools::{KuroTool, ToolRegistry};

    #[test]
    fn panic_in_feature_keeps_ptt() {
        static DID_PANIC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        fn boom(_: &serde_json::Value) -> String {
            if !DID_PANIC.swap(true, std::sync::atomic::Ordering::SeqCst) {
                panic!("tool boom");
            }
            "ok".into()
        }
        let reg = ToolRegistry::single(KuroTool {
            name: "panic_tool",
            description: "test",
            parameters_schema: serde_json::json!({}),
            is_local_only: true,
            match_phrase: None,
            execute: boom,
        });
        let args = serde_json::json!({});
        let first = reg.execute("panic_tool", &args);
        assert!(first.contains("wrong") || first.contains("Something"));
        let second = reg.execute("panic_tool", &args);
        assert_eq!(second, "ok");
    }
}
