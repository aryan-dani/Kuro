/// Voice-turn epoch. Dismiss and Escape increment it so a stale turn cannot speak.
#[derive(Clone, Debug, Default)]
pub struct TurnClock {
    pub turn: u64,
    pub followup: bool,
    pub cancel: bool,
    pub processing: bool,
}

impl TurnClock {
    pub fn live(&self, epoch: u64) -> bool {
        epoch == self.turn && !self.cancel
    }

    pub fn snapshot(&self) -> u64 {
        self.turn
    }

    /// Escape and `POST /dismiss`.
    pub fn force_unblock(&mut self) {
        self.turn = self.turn.saturating_add(1);
        self.followup = false;
        self.cancel = true;
        self.processing = false;
    }

    pub fn begin_turn(&mut self) -> u64 {
        self.cancel = false;
        self.processing = false;
        self.turn = self.turn.saturating_add(1);
        self.turn
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowUp {
    None,
    Listen,
}

pub fn follow_up_for(reply: &str, live: bool, already: bool) -> FollowUp {
    if live && !already && reply.trim().ends_with('?') { FollowUp::Listen } else { FollowUp::None }
}

/// Live partial wins when final is empty, or when only the live text matches a phrase.
pub fn prefer_transcript(live: &str, final_text: &str, live_match: bool, final_match: bool) -> String {
    let live = live.trim();
    let final_text = final_text.trim();
    if final_text.is_empty() && !live.is_empty() {
        return live.to_string();
    }
    if !final_text.is_empty()
        && !live.is_empty()
        && !final_text.eq_ignore_ascii_case(live)
        && live_match
        && !final_match
    {
        return live.to_string();
    }
    final_text.to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudState {
    Hidden,
    Listening,
    Thinking,
    Speaking,
    Dormant,
    Error,
    Ready,
    Done,
}

impl HudState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Listening => "listening",
            Self::Thinking => "thinking",
            Self::Speaking => "speaking",
            Self::Dormant => "dormant",
            Self::Error => "error",
            Self::Ready => "ready",
            Self::Done => "done",
        }
    }
}
