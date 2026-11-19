use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use kuro_core::TurnClock;

pub struct TurnGate {
    busy: Arc<AtomicBool>,
    clock: Arc<Mutex<TurnClock>>,
    hud_idle: Arc<dyn Fn() + Send + Sync>,
    hud_error: Arc<dyn Fn(&str) + Send + Sync>,
}

impl TurnGate {
    pub fn new(
        busy: Arc<AtomicBool>,
        clock: Arc<Mutex<TurnClock>>,
        hud_idle: Arc<dyn Fn() + Send + Sync>,
        hud_error: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        Self { busy, clock, hud_idle, hud_error }
    }

    pub fn busy(&self) -> &Arc<AtomicBool> {
        &self.busy
    }

    pub fn press_blocked(&self) -> bool {
        if self.busy.load(Ordering::Relaxed) {
            return true;
        }
        self.clock.lock().map(|c| c.processing).unwrap_or(false)
    }

    pub fn cancel(&self) {
        self.busy.store(false, Ordering::Relaxed);
        if let Ok(mut clock) = self.clock.lock() {
            clock.force_unblock();
        }
        (self.hud_idle)();
    }

    pub fn set_processing(&self) {
        if let Ok(mut clock) = self.clock.lock() {
            clock.processing = true;
        }
    }

    pub fn try_claim_worker(&self) -> bool {
        !self.busy.swap(true, Ordering::Relaxed)
    }

    pub fn worker_guard(&self, epoch: u64) -> TurnGuard<'_> {
        TurnGuard { gate: self, epoch }
    }

    pub fn error_then_idle(&self, message: &str) {
        (self.hud_error)(message);
        std::thread::sleep(Duration::from_millis(1500));
        (self.hud_idle)();
    }

    fn finish_worker(&self, epoch: u64) {
        let live = self.clock.lock().map(|c| c.live(epoch)).unwrap_or(false);
        if live {
            (self.hud_idle)();
        }
        if let Ok(mut clock) = self.clock.lock()
            && (clock.snapshot() == epoch || clock.live(epoch))
        {
            clock.processing = false;
        }
        self.busy.store(false, Ordering::Relaxed);
    }
}

pub struct TurnGuard<'a> {
    gate: &'a TurnGate,
    epoch: u64,
}

impl Drop for TurnGuard<'_> {
    fn drop(&mut self) {
        self.gate.finish_worker(self.epoch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    type TestGateParts = (TurnGate, Arc<Mutex<TurnClock>>, Arc<AtomicU64>, Arc<Mutex<String>>);

    fn test_gate() -> TestGateParts {
        let busy = Arc::new(AtomicBool::new(false));
        let clock = Arc::new(Mutex::new(TurnClock::default()));
        let hud_state = Arc::new(Mutex::new(String::from("hidden")));
        let err = Arc::new(Mutex::new(String::new()));
        let last = Arc::new(AtomicU64::new(0));
        let hs = hud_state.clone();
        let er = err.clone();
        let last_idle = last.clone();
        let idle = Arc::new(move || {
            *hs.lock().unwrap() = String::from("idle");
            last_idle.store(1, Ordering::Relaxed);
        });
        let er2 = er.clone();
        let error = Arc::new(move |msg: &str| {
            *er2.lock().unwrap() = msg.to_string();
            *hud_state.lock().unwrap() = String::from("error");
        });
        let gate = TurnGate::new(busy, clock.clone(), idle, error);
        (gate, clock, last, err)
    }

    #[test]
    fn press_after_dismiss_starts_turn() {
        let (gate, _, _, _) = test_gate();
        gate.busy.store(true, Ordering::Relaxed);
        gate.cancel();
        assert!(!gate.press_blocked());
    }

    #[test]
    fn stale_worker_does_not_clobber_new_turn() {
        let busy = Arc::new(AtomicBool::new(false));
        let clock = Arc::new(Mutex::new(TurnClock::default()));
        let hud = Arc::new(Mutex::new(String::new()));
        let h = hud.clone();
        let idle = Arc::new(move || *h.lock().unwrap() = String::from("idle"));
        let eh = hud.clone();
        let error = Arc::new(move |_: &str| *eh.lock().unwrap() = String::from("error"));
        let gate = TurnGate::new(busy, clock.clone(), idle, error);
        let epoch1 = clock.lock().unwrap().begin_turn();
        let _g1 = gate.worker_guard(epoch1);
        gate.cancel();
        let epoch2 = clock.lock().unwrap().begin_turn();
        *hud.lock().unwrap() = String::from("listening");
        drop(_g1);
        assert_eq!(*hud.lock().unwrap(), "listening");
        let _g2 = gate.worker_guard(epoch2);
        drop(_g2);
        assert_eq!(*hud.lock().unwrap(), "idle");
    }

    #[test]
    fn error_returns_to_idle() {
        let (gate, _, last, _) = test_gate();
        gate.error_then_idle("mic");
        assert_eq!(last.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn panic_in_worker_releases_gate() {
        let (gate, clock, _, _) = test_gate();
        let epoch = clock.lock().unwrap().begin_turn();
        gate.try_claim_worker();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = gate.worker_guard(epoch);
            panic!("worker");
        }));
        assert!(result.is_err());
        assert!(!gate.busy().load(Ordering::Relaxed));
    }
}
