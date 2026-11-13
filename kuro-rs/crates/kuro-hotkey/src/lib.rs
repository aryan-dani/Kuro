use std::time::Duration;

use kuro_core::TurnClock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeySpec {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub vk: u16,
}

pub fn parse_hotkey(raw: &str) -> HotkeySpec {
    let mut spec = HotkeySpec { ctrl: false, alt: false, shift: false, win: false, vk: 0x20 };
    for part in raw.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => spec.ctrl = true,
            "alt" | "menu" => spec.alt = true,
            "shift" => spec.shift = true,
            "win" | "super" => spec.win = true,
            "space" => spec.vk = 0x20,
            "esc" | "escape" => spec.vk = 0x1B,
            other => {
                if other.len() == 1 {
                    spec.vk = other.as_bytes()[0].to_ascii_uppercase() as u16;
                }
            }
        }
    }
    spec
}

pub fn default_hotkey() -> HotkeySpec {
    parse_hotkey(&std::env::var("KURO_HOTKEY").unwrap_or_else(|_| "ctrl+alt+space".into()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PttPhase {
    Idle,
    Held,
}

pub struct PttMachine {
    pub phase: PttPhase,
    pub clock: TurnClock,
}

impl PttMachine {
    pub fn new() -> Self {
        Self { phase: PttPhase::Idle, clock: TurnClock::default() }
    }

    pub fn on_hotkey_down(&mut self) -> bool {
        if self.clock.processing {
            return false;
        }
        self.phase = PttPhase::Held;
        let _ = self.clock.begin_turn();
        true
    }

    pub fn on_release(&mut self) {
        if self.phase == PttPhase::Held {
            self.phase = PttPhase::Idle;
            self.clock.processing = true;
        }
    }

    pub fn on_escape(&mut self) {
        if self.phase != PttPhase::Idle || self.clock.processing {
            self.phase = PttPhase::Idle;
            self.clock.force_unblock();
        }
    }
}

impl Default for PttMachine {
    fn default() -> Self {
        Self::new()
    }
}

pub const RELEASE_POLL: Duration = Duration::from_millis(8);

/// Win32 `RegisterHotKey` on a message-only window. Not called from unit tests.
/// Hotkey id 1 = PTT. Hotkey id 2 = Escape (always dismisses).
pub fn register_and_wait(
    spec: HotkeySpec,
    mut on_down: impl FnMut(),
    mut on_up: impl FnMut(),
    mut on_escape: impl FnMut(),
) {
    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, VK_ESCAPE,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG, TranslateMessage,
            WM_HOTKEY,
        };
        let class = windows_sys::w!("KuroHotkey");
        let hwnd = CreateWindowExW(
            0,
            class,
            windows_sys::w!(""),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        let _ = hwnd;
        let mut mods = 0u32;
        if spec.ctrl {
            mods |= MOD_CONTROL;
        }
        if spec.alt {
            mods |= MOD_ALT;
        }
        if spec.shift {
            mods |= MOD_SHIFT;
        }
        if spec.win {
            mods |= MOD_WIN;
        }
        let mut registered = false;
        for _ in 0..10 {
            if RegisterHotKey(hwnd, 1, mods | MOD_NOREPEAT, u32::from(spec.vk)) != 0 {
                registered = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if !registered {
            eprintln!("Failed to register hotkey with modifiers {mods} and vk {}", spec.vk);
        }
        let mut msg = MSG {
            hwnd: std::ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: windows_sys::Win32::Foundation::POINT { x: 0, y: 0 },
        };
        loop {
            let res = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if res == 0 {
                break;
            }
            if res == -1 {
                std::thread::sleep(Duration::from_millis(25));
                continue;
            }
            if msg.message == WM_HOTKEY && msg.wParam == 1 {
                on_down();
                // RegisterHotKey consumes the trigger VK (Space), so GetAsyncKeyState for
                // that key often already reads "up" on the first poll. Hold-to-talk is
                // therefore gated on the required *modifiers* staying down.
                loop {
                    std::thread::sleep(RELEASE_POLL);
                    let ctrl_down = !spec.ctrl || (GetAsyncKeyState(0x11) as u16) & 0x8000 != 0;
                    let alt_down = !spec.alt || (GetAsyncKeyState(0x12) as u16) & 0x8000 != 0;
                    let shift_down = !spec.shift || (GetAsyncKeyState(0x10) as u16) & 0x8000 != 0;
                    let win_down = !spec.win
                        || (GetAsyncKeyState(0x5B) as u16) & 0x8000 != 0
                        || (GetAsyncKeyState(0x5C) as u16) & 0x8000 != 0;
                    if !(ctrl_down && alt_down && shift_down && win_down) {
                        on_up();
                        break;
                    }
                    if (GetAsyncKeyState(i32::from(VK_ESCAPE)) as u16) & 0x8000 != 0 {
                        on_escape();
                        break;
                    }
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = DefWindowProcW;
    }
}
