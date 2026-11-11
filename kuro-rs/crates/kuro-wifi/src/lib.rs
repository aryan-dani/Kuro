use std::path::Path;

#[derive(Clone, Debug)]
pub struct WifiPolicy {
    pub disabled: bool,
    pub away_ok: bool,
    pub home: Vec<String>,
}

impl WifiPolicy {
    pub fn load(root: &Path) -> Self {
        let paths = kuro_core::KuroPaths::new(root);
        let mut home = Vec::new();
        if let Ok(raw) = std::env::var("KURO_HOME_SSID") {
            home.extend(raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
        }
        if let Ok(text) = std::fs::read_to_string(paths.home_ssid()) {
            for line in text.lines() {
                let line = line.trim();
                if !line.is_empty() && !home.iter().any(|s| s.eq_ignore_ascii_case(line)) {
                    home.push(line.to_string());
                }
            }
        }
        Self { disabled: paths.disabled_flag().is_file(), away_ok: paths.away_ok().is_file(), home }
    }

    pub fn should_run(&self, connected: Option<&str>) -> bool {
        if self.disabled {
            return false;
        }
        if self.away_ok || self.home.is_empty() {
            return true;
        }
        let Some(ssid) = connected else {
            return false;
        };
        self.home.iter().any(|home| home.eq_ignore_ascii_case(ssid))
    }
}

pub fn current_ssid() -> Option<String> {
    let output = std::process::Command::new("netsh").args(["wlan", "show", "interfaces"]).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("SSID") else {
            continue;
        };
        if line.to_ascii_lowercase().starts_with("bssid") {
            continue;
        }
        let name = rest.trim().trim_start_matches(':').trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// Opens a WLAN handle and registers ACM notifications. The callback is the OS entry; policy is `should_run`.
pub fn register_notifications() -> bool {
    unsafe {
        use windows_sys::Win32::NetworkManagement::WiFi::{
            WLAN_NOTIFICATION_SOURCE_ACM, WlanOpenHandle, WlanRegisterNotification,
        };
        let mut negotiated = 0u32;
        let mut handle = std::ptr::null_mut();
        let ok = WlanOpenHandle(2, std::ptr::null(), &mut negotiated, &mut handle);
        if ok != 0 || handle.is_null() {
            return false;
        }
        let status = WlanRegisterNotification(
            handle,
            WLAN_NOTIFICATION_SOURCE_ACM,
            0,
            None,
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        status == 0
    }
}
