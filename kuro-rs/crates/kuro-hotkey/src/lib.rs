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
        RegisterHotKey(hwnd, 1, mods, u32::from(spec.vk));
        RegisterHotKey(hwnd, 2, MOD_NOREPEAT, u32::from(VK_ESCAPE));
        let mut msg = MSG {
            hwnd: std::ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: windows_sys::Win32::Foundation::POINT { x: 0, y: 0 },
        };
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                if msg.wParam == 2 {
                    on_escape();
                } else {
                    on_down();
                    loop {
                        std::thread::sleep(RELEASE_POLL);
                        let space = GetAsyncKeyState(i32::from(spec.vk));
                        let ctrl = if spec.ctrl { GetAsyncKeyState(0x11) } else { 0x8000u16 as i16 };
                        let alt = if spec.alt { GetAsyncKeyState(0x12) } else { 0x8000u16 as i16 };
                        if (space as u16) & 0x8000 == 0 || (ctrl as u16) & 0x8000 == 0 || (alt as u16) & 0x8000 == 0 {
                            on_up();
                            break;
                        }
                        if (GetAsyncKeyState(i32::from(VK_ESCAPE)) as u16) & 0x8000 != 0 {
                            on_escape();
                            break;
                        }
                    }
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = DefWindowProcW;
    }
}
