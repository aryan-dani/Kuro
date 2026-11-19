/// Turn lifecycle state. Cancel from any non-idle state must eventually reach `Idle`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum State {
    Idle,
    Listening,
    Transcribing,
    Routing,
    Executing,
    Confirming { plan_id: u64, tier: String, summary: String, needs_execute: bool },
    Speaking,
    FollowUp,
    Cancelling,
}
