use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) static MEET_BUSY: AtomicBool = AtomicBool::new(false);

/// Live Google/Edge Meet actuation. Off until the engine process enables it so
/// `cargo test` / gate never create a real Meet or open Edge on the desk.
static LIVE_MEET: AtomicBool = AtomicBool::new(false);

/// Unix seconds until a new Meet may start (cooldown after a claim).
static MEET_COOLDOWN_UNTIL: AtomicU64 = AtomicU64::new(0);
const MEET_COOLDOWN_SECS: u64 = 45;

pub(crate) fn enable_live_meet() {
    LIVE_MEET.store(true, Ordering::SeqCst);
}

pub(crate) fn live_meet_enabled() -> bool {
    LIVE_MEET.load(Ordering::SeqCst)
}

pub(crate) struct MeetBusyGuard {
    cool: std::cell::Cell<bool>,
}

impl MeetBusyGuard {
    pub(crate) fn mark_cooldown(&self) {
        self.cool.set(true);
    }
}

impl Drop for MeetBusyGuard {
    fn drop(&mut self) {
        release_meet(self.cool.get());
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub(crate) fn claim_meet() -> Option<MeetBusyGuard> {
    if MEET_BUSY.swap(true, Ordering::SeqCst) {
        return None;
    }
    let now = unix_now();
    let until = MEET_COOLDOWN_UNTIL.load(Ordering::SeqCst);
    if now < until {
        MEET_BUSY.store(false, Ordering::SeqCst);
        return None;
    }
    Some(MeetBusyGuard { cool: std::cell::Cell::new(false) })
}

fn release_meet(apply_cooldown: bool) {
    if apply_cooldown {
        MEET_COOLDOWN_UNTIL.store(unix_now().saturating_add(MEET_COOLDOWN_SECS), Ordering::SeqCst);
    }
    MEET_BUSY.store(false, Ordering::SeqCst);
}
