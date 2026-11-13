use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
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

const WM_KURO_PTT_DOWN: u32 = 0x8000 + 0x21; // WM_APP + 0x21
const WM_KURO_PTT_UP: u32 = 0x8000 + 0x22;
const WM_KURO_ESC: u32 = 0x8000 + 0x23;

static HOOK_HWND: AtomicIsize = AtomicIsize::new(0);
static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
static ALT_DOWN: AtomicBool = AtomicBool::new(false);
static SHIFT_DOWN: AtomicBool = AtomicBool::new(false);
static WIN_DOWN: AtomicBool = AtomicBool::new(false);
static TRIG_DOWN: AtomicBool = AtomicBool::new(false);
static PTT_HELD: AtomicBool = AtomicBool::new(false);
static NEED_CTRL: AtomicBool = AtomicBool::new(true);
static NEED_ALT: AtomicBool = AtomicBool::new(true);
static NEED_SHIFT: AtomicBool = AtomicBool::new(false);
static NEED_WIN: AtomicBool = AtomicBool::new(false);
static TRIG_VK: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0x20);

fn modifiers_held() -> bool {
    (!NEED_CTRL.load(Ordering::Relaxed) || CTRL_DOWN.load(Ordering::Relaxed))
        && (!NEED_ALT.load(Ordering::Relaxed) || ALT_DOWN.load(Ordering::Relaxed))
        && (!NEED_SHIFT.load(Ordering::Relaxed) || SHIFT_DOWN.load(Ordering::Relaxed))
        && (!NEED_WIN.load(Ordering::Relaxed) || WIN_DOWN.load(Ordering::Relaxed))
}

fn chord_armed() -> bool {
    modifiers_held() && TRIG_DOWN.load(Ordering::Relaxed)
}

fn post_to_hook_hwnd(msg: u32) {
    let hwnd = HOOK_HWND.load(Ordering::SeqCst);
    if hwnd == 0 {
        return;
    }
    unsafe {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;
        let _ = PostMessageW(hwnd as HWND, msg, 0, 0);
    }
}

fn sync_ptt_from_keys() {
    let held = PTT_HELD.load(Ordering::SeqCst);
    if !held {
        // Start only when the full chord is down (Ctrl+Alt+Space).
        if chord_armed() {
            PTT_HELD.store(true, Ordering::SeqCst);
            post_to_hook_hwnd(WM_KURO_PTT_DOWN);
        }
        return;
    }
    // Once listening, keep going until a *modifier* lifts. Space alone may bounce
    // up early on real keyboards and was cutting captures to <1s with empty STT.
    if !modifiers_held() {
        PTT_HELD.store(false, Ordering::SeqCst);
        TRIG_DOWN.store(false, Ordering::Relaxed);
        post_to_hook_hwnd(WM_KURO_PTT_UP);
    }
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    if code >= 0 && lparam != 0 {
        let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let vk = info.vkCode as u16;
        let is_down = wparam == WM_KEYDOWN as usize || wparam == WM_SYSKEYDOWN as usize;
        let is_up = wparam == WM_KEYUP as usize || wparam == WM_SYSKEYUP as usize;
        if is_down || is_up {
            let down = is_down;
            match vk {
                0x11 | 0xA2 | 0xA3 => CTRL_DOWN.store(down, Ordering::Relaxed),
                0x12 | 0xA4 | 0xA5 => ALT_DOWN.store(down, Ordering::Relaxed),
                0x10 | 0xA0 | 0xA1 => SHIFT_DOWN.store(down, Ordering::Relaxed),
                0x5B | 0x5C => WIN_DOWN.store(down, Ordering::Relaxed),
                0x1B if down => {
                    if PTT_HELD.load(Ordering::SeqCst) {
                        PTT_HELD.store(false, Ordering::SeqCst);
                        TRIG_DOWN.store(false, Ordering::Relaxed);
                        post_to_hook_hwnd(WM_KURO_ESC);
                    }
                }
                other if other == TRIG_VK.load(Ordering::Relaxed) => {
                    TRIG_DOWN.store(down, Ordering::Relaxed);
                }
                _ => {}
            }
            sync_ptt_from_keys();
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// Low-level keyboard hook PTT. Hold the chord to talk; release any required key to stop.
/// Escape cancels an active hold. Blocks until the message loop ends.
pub fn register_and_wait(
    spec: HotkeySpec,
    mut on_down: impl FnMut(),
    mut on_up: impl FnMut(),
    mut on_escape: impl FnMut(),
) -> Result<(), String> {
    unsafe {
        use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
        use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG,
            RegisterClassW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, UnregisterClassW, WH_KEYBOARD_LL,
            WNDCLASSW, WS_OVERLAPPED,
        };

        NEED_CTRL.store(spec.ctrl, Ordering::Relaxed);
        NEED_ALT.store(spec.alt, Ordering::Relaxed);
        NEED_SHIFT.store(spec.shift, Ordering::Relaxed);
        NEED_WIN.store(spec.win, Ordering::Relaxed);
        TRIG_VK.store(spec.vk, Ordering::Relaxed);
        CTRL_DOWN.store(false, Ordering::Relaxed);
        ALT_DOWN.store(false, Ordering::Relaxed);
        SHIFT_DOWN.store(false, Ordering::Relaxed);
        WIN_DOWN.store(false, Ordering::Relaxed);
        TRIG_DOWN.store(false, Ordering::Relaxed);
        PTT_HELD.store(false, Ordering::SeqCst);

        let class_name = windows_sys::w!("KuroHotkeyMsg");
        let wnd_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = DefWindowProcW;
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()),
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name,
        };
        let atom = RegisterClassW(&class);
        if atom == 0 {
            let err = GetLastError();
            // Class may already exist from a previous run in the same process.
            if err != 1410 {
                return Err(format!("RegisterClassW failed err={err}"));
            }
        }

        let hwnd = CreateWindowExW(
            0,
            class_name,
            windows_sys::w!("KuroHotkey"),
            WS_OVERLAPPED,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err(format!("CreateWindowExW failed err={}", GetLastError()));
        }
        HOOK_HWND.store(hwnd as isize, Ordering::SeqCst);

        let hook =
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), GetModuleHandleW(std::ptr::null()), 0);
        if hook.is_null() {
            HOOK_HWND.store(0, Ordering::SeqCst);
            return Err(format!("SetWindowsHookExW failed err={}", GetLastError()));
        }
        eprintln!(
            "Kuro hotkey: LL hook armed for ctrl={} alt={} shift={} win={} vk=0x{:02X}",
            spec.ctrl, spec.alt, spec.shift, spec.win, spec.vk
        );

        let mut msg = MSG {
            hwnd: std::ptr::null_mut(),
            message: 0,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: windows_sys::Win32::Foundation::POINT { x: 0, y: 0 },
        };
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            match msg.message {
                WM_KURO_PTT_DOWN => on_down(),
                WM_KURO_PTT_UP => on_up(),
                WM_KURO_ESC => on_escape(),
                _ => {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        UnhookWindowsHookEx(hook);
        HOOK_HWND.store(0, Ordering::SeqCst);
        let _ = UnregisterClassW(class_name, GetModuleHandleW(std::ptr::null()));
        Ok(())
    }
}
