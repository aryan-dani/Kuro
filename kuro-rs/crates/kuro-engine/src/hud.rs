use std::sync::{Arc, Mutex};

use kuro_core::TurnClock;
use kuro_ipc::AppState;

use crate::TurnGate;

pub fn set_hud(
    hud: &Mutex<kuro_ipc::HudSnap>,
    state: &str,
    label: &str,
    levels: Option<Vec<f64>>,
    you: &str,
    kuro: &str,
    wake: Option<&Arc<tokio::sync::Notify>>,
) {
    let Ok(mut snap) = hud.lock() else {
        return;
    };
    snap.state = state.into();
    snap.label = if label.is_empty() { state.into() } else { label.into() };
    let cleared = levels.is_none();
    if let Some(levels) = levels {
        snap.levels = levels;
    }
    if !you.is_empty() {
        snap.you = you.into();
    }
    if !kuro.is_empty() {
        snap.kuro = kuro.into();
    } else if state == "thinking" {
        snap.kuro.clear();
    }
    if state == "hidden" || state == "idle" {
        if state == "hidden" {
            snap.you.clear();
            snap.kuro.clear();
        }
        if cleared {
            snap.levels = vec![0.0; 11];
        }
    }
    if let Some(notify) = wake {
        notify.notify_waiters();
    }
}

pub fn build_gate(
    busy: Arc<std::sync::atomic::AtomicBool>,
    clock: Arc<Mutex<TurnClock>>,
    api: Option<Arc<AppState>>,
) -> Arc<TurnGate> {
    let idle_src = api.clone();
    let idle = Arc::new(move || {
        if let Some(state) = idle_src.as_ref() {
            set_hud(&state.hud, "idle", "idle", Some(vec![0.0; 11]), "", "", Some(&state.hud_wake));
        }
    });
    let err_src = api;
    let error = Arc::new(move |msg: &str| {
        if let Some(state) = err_src.as_ref() {
            set_hud(&state.hud, "error", msg, Some(vec![0.0; 11]), "", "", Some(&state.hud_wake));
        }
    });
    Arc::new(TurnGate::new(busy, clock, idle, error))
}
