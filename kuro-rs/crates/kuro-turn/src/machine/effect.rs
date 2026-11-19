/// HUD phase echoed to the shell capsule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HudMode {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Error,
}

/// Port commands emitted by `step`. The driver performs I/O; the machine never does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    StartMic,
    StopMic,
    Transcribe,
    Route,
    Execute,
    Speak {
        text: String,
    },
    /// Stop in-flight TTS. Emitted for Cancel while Speaking and for PTT barge-in
    /// (`HotkeyDown` during Speaking). The driver may gate barge-in on
    /// `barge_in_enabled` (default true) before forwarding `HotkeyDown`.
    CancelSpeak,
    PromptConfirm {
        summary: String,
    },
    ExecuteConfirmed {
        plan_id: u64,
    },
    SetHud(HudMode),
}
