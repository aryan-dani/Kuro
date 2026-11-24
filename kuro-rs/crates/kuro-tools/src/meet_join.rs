//! Press Join now on the Edge Meet window. Opening the link is not joining.

use std::collections::HashMap;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub(crate) struct WinSnap {
    pub hwnd: isize,
    pub title: String,
    pub class_name: String,
    pub edge: bool,
    pub area: i64,
}

pub(crate) fn choose_meet_window(before: &HashMap<isize, String>, now: &[WinSnap], elapsed_ms: u128) -> Option<isize> {
    let mut chosen = 0isize;
    let mut best = 0i64;
    for win in now {
        if win.class_name != "Chrome_WidgetWin_1" || !win.edge {
            continue;
        }
        let prev = before.get(&win.hwnd).map(String::as_str);
        let is_new = prev.is_none();
        let became = prev.is_some_and(|title| !has_meet(title) && has_meet(&win.title));
        if !is_new && !became {
            continue;
        }
        if !has_meet(&win.title) && elapsed_ms < 1500 {
            continue;
        }
        let mut area = win.area;
        if has_meet(&win.title) {
            area += 10_000_000;
        }
        if area > best {
            best = area;
            chosen = win.hwnd;
        }
    }
    if chosen == 0 { None } else { Some(chosen) }
}

fn has_meet(title: &str) -> bool {
    title.to_ascii_lowercase().contains("meet")
}

pub(crate) fn titles() -> HashMap<isize, String> {
    list_windows().into_iter().map(|win| (win.hwnd, win.title)).collect()
}

pub(crate) fn wait_meet_window(before: &HashMap<isize, String>, timeout: Duration) -> Option<isize> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Some(hwnd) = choose_meet_window(before, &list_windows(), started.elapsed().as_millis()) {
            return Some(hwnd);
        }
        thread::sleep(Duration::from_millis(200));
    }
    None
}

pub(crate) fn focus(hwnd: isize) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SW_SHOWMAXIMIZED, SetForegroundWindow,
            ShowWindow,
        };
        let fg = GetForegroundWindow();
        let current = GetCurrentThreadId();
        let mut other = 0u32;
        if !fg.is_null() {
            GetWindowThreadProcessId(fg, &mut other);
        }
        let attached = other != 0 && other != current && AttachThreadInput(current, other, 1) != 0;
        let handle = hwnd as HWND;
        ShowWindow(handle, SW_SHOWMAXIMIZED);
        BringWindowToTop(handle);
        SetForegroundWindow(handle);
        if attached {
            AttachThreadInput(current, other, 0);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
    }
}

pub(crate) fn click_join(hwnd: isize, timeout: Duration) -> bool {
    let seconds = timeout.as_secs().max(1);
    let script = format!(
        r#"
$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName System.Windows.Forms
if (-not ('KuroMouse' -as [type])) {{
  Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class KuroMouse {{ [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, int e); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y); }}'
}}
$root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]{hwnd})
if ($null -eq $root) {{ exit 3 }}
$deadline = [DateTime]::UtcNow.AddSeconds({seconds})
$names = @('Join now','Ask to join')
while ([DateTime]::UtcNow -lt $deadline) {{
  foreach ($name in $names) {{
    try {{
      $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
      $el = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
      if ($null -eq $el -or -not $el.Current.IsEnabled) {{ continue }}
      try {{
        $pattern = $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $pattern.Invoke()
        exit 0
      }} catch {{}}
      $point = New-Object System.Windows.Point
      if ($el.TryGetClickablePoint([ref]$point)) {{
        [KuroMouse]::SetForegroundWindow([IntPtr]{hwnd})
        Start-Sleep -Milliseconds 150
        [KuroMouse]::SetCursorPos([int]$point.X, [int]$point.Y)
        Start-Sleep -Milliseconds 80
        [KuroMouse]::mouse_event(2, 0, 0, 0, 0)
        [KuroMouse]::mouse_event(4, 0, 0, 0, 0)
        exit 0
      }}
    }} catch {{}}
  }}
  Start-Sleep -Milliseconds 400
}}
exit 2
"#
    );
    let mut child = match Command::new("powershell")
        .args(["-NoProfile", "-Sta", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    let limit = timeout + Duration::from_secs(10);
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code() == Some(0),
            Ok(None) if started.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(_) => return false,
        }
    }
}

fn list_windows() -> Vec<WinSnap> {
    #[cfg(windows)]
    {
        use std::sync::Mutex;
        use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT, TRUE};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
        };
        static ROWS: Mutex<Vec<WinSnap>> = Mutex::new(Vec::new());
        unsafe extern "system" fn each(hwnd: HWND, _lparam: LPARAM) -> i32 {
            unsafe {
                if IsWindowVisible(hwnd) == 0 {
                    return TRUE;
                }
                let title = window_text(hwnd);
                let class_name = class_text(hwnd);
                let area = window_area(hwnd);
                let edge = is_edge(hwnd);
                if let Ok(mut rows) = ROWS.lock() {
                    rows.push(WinSnap { hwnd: hwnd as isize, title, class_name, edge, area });
                }
                TRUE
            }
        }
        fn window_text(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
            unsafe {
                let len = GetWindowTextLengthW(hwnd);
                if len <= 0 {
                    return String::new();
                }
                let mut buf = vec![0u16; (len as usize) + 1];
                GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
                utf16_buf(&buf)
            }
        }
        fn class_text(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
            unsafe {
                let mut buf = [0u16; 256];
                GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
                utf16_buf(&buf)
            }
        }
        fn window_area(hwnd: windows_sys::Win32::Foundation::HWND) -> i64 {
            unsafe {
                let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                if GetWindowRect(hwnd, &mut rect) == 0 {
                    return 0;
                }
                let w = i64::from(rect.right - rect.left).max(0);
                let h = i64::from(rect.bottom - rect.top).max(0);
                w * h
            }
        }
        if let Ok(mut rows) = ROWS.lock() {
            rows.clear();
        }
        unsafe {
            EnumWindows(Some(each), 0);
        }
        ROWS.lock().map(|rows| rows.clone()).unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
fn utf16_buf(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[cfg(windows)]
fn is_edge(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return false;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let ok = query_image(handle, 0, buf.as_mut_ptr(), &mut size);
        windows_sys::Win32::Foundation::CloseHandle(handle);
        if ok == 0 {
            return false;
        }
        utf16_buf(&buf).to_ascii_lowercase().ends_with("\\msedge.exe")
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "QueryFullProcessImageNameW"]
    fn query_image(handle: *mut core::ffi::c_void, flags: u32, buf: *mut u16, size: *mut u32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_new_edge_meet_window() {
        let before = HashMap::from([(7isize, "Inbox - Outlook".into())]);
        let now = vec![
            WinSnap {
                hwnd: 7,
                title: "Inbox - Outlook".into(),
                class_name: "Chrome_WidgetWin_1".into(),
                edge: true,
                area: 100,
            },
            WinSnap {
                hwnd: 9,
                title: "Meet - Edge".into(),
                class_name: "Chrome_WidgetWin_1".into(),
                edge: true,
                area: 50,
            },
            WinSnap {
                hwnd: 11,
                title: "Meet - Chrome".into(),
                class_name: "Chrome_WidgetWin_1".into(),
                edge: false,
                area: 9_000,
            },
        ];
        assert_eq!(choose_meet_window(&before, &now, 2_000), Some(9));
    }

    #[test]
    fn join_script_names_the_button() {
        let src = include_str!("meet_join.rs");
        assert!(src.contains("'Join now'"));
        assert!(src.contains("'Ask to join'"));
    }

    #[test]
    #[ignore = "opens Edge and clicks Join now"]
    fn clicks_join_now_in_edge() {
        let dir = std::env::temp_dir().join(format!("kuro-join-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let page = dir.join("preview.html");
        std::fs::write(
            &page,
            r#"<!DOCTYPE html><html><head><title>Meet preview</title></head><body><button onclick="document.title='Meet joined'">Join now</button></body></html>"#,
        )
        .unwrap();
        let link = format!("file:///{}", page.display().to_string().replace('\\', "/"));
        let edge = std::env::var_os("ProgramFiles(x86)")
            .map(std::path::PathBuf::from)
            .map(|p| p.join(r"Microsoft\Edge\Application\msedge.exe"))
            .filter(|p| p.is_file())
            .expect("edge");
        let before = titles();
        let mut child = Command::new(edge).args([format!("--app={link}"), "--start-maximized".into()]).spawn().unwrap();
        let hwnd = wait_meet_window(&before, Duration::from_secs(10)).expect("meet window");
        focus(hwnd);
        assert!(click_join(hwnd, Duration::from_secs(8)), "Join now was not clicked");
        let mut title = String::new();
        let look = Instant::now();
        while look.elapsed() < Duration::from_secs(2) {
            title = titles().get(&hwnd).cloned().unwrap_or_default();
            if title.to_ascii_lowercase().contains("joined") {
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(title.to_ascii_lowercase().contains("joined"), "title={title}");
    }
}
