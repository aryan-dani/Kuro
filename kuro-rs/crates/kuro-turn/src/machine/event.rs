/// Outcome of routing (and, when `needs_execute`, of tool execution via a second event).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RouteOutcome {
    pub reply: String,
    pub needs_execute: bool,
    pub skip_tts: bool,
    /// When true after `SpeakDone`, enter `FollowUp` instead of `Idle` (phase 20 policy).
    pub allow_follow_up: bool,
    pub needs_confirm: bool,
    pub confirm_summary: String,
    pub confirm_tier: String,
    pub plan_id: u64,
}

/// Events fed back into `step` by the driver or scenario tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    HotkeyDown,
    HotkeyUp,
    SttDone { text: String },
    RouteDone(RouteOutcome),
    SpeakDone,
    Cancel,
    Error,
    ConfirmYes,
    ConfirmNo,
    ConfirmTimeout,
}
