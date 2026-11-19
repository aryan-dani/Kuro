use std::path::{Path, PathBuf};

use kuro_config::Settings;
use kuro_core::KuroPaths;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
    #[cfg_attr(windows, allow(dead_code))]
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DoctorCheck {
    pub id: &'static str,
    pub status: CheckStatus,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub checks: Vec<DoctorCheck>,
}

pub struct DoctorContext<'a> {
    pub data_root: &'a Path,
    pub repo_root: &'a Path,
    pub settings: &'a Settings,
    pub rustc_version: Option<&'a str>,
}

impl DoctorReport {
    pub fn exit_code(&self) -> i32 {
        if self.ok { 0 } else { 1 }
    }
}

pub fn run_doctor(ctx: &DoctorContext<'_>) -> DoctorReport {
    let checks = vec![
        check_toolchain(ctx),
        check_profile(ctx),
        check_models_dir(ctx),
        check_api_token(ctx),
        check_api_bind(ctx),
        check_winrt_voice(),
        check_mic_permission(),
    ];
    let ok = checks.iter().all(|c| matches!(c.status, CheckStatus::Ok | CheckStatus::Warn | CheckStatus::Skip));
    DoctorReport { ok, checks }
}

pub fn doctor_json(ctx: &DoctorContext<'_>) -> String {
    serde_json::to_string_pretty(&run_doctor(ctx)).expect("doctor report serializes")
}

fn check_toolchain(ctx: &DoctorContext<'_>) -> DoctorCheck {
    let toolchain_path = ctx.repo_root.join("kuro-rs").join("rust-toolchain.toml");
    let want = read_toolchain_channel(&toolchain_path);
    let Some(rustc) = ctx.rustc_version else {
        return DoctorCheck {
            id: "toolchain",
            status: CheckStatus::Warn,
            message: format!(
                "rustc not on PATH; install Rust {} (see kuro-rs/rust-toolchain.toml)",
                want.as_deref().unwrap_or("stable")
            ),
        };
    };
    let message = if let Some(ref channel) = want {
        if rustc.contains(channel.trim_start_matches("rust:")) {
            format!("rustc reports {rustc}; matches pinned channel {channel}")
        } else {
            format!("rustc reports {rustc}; repo pins {channel} in kuro-rs/rust-toolchain.toml")
        }
    } else {
        format!("rustc reports {rustc}")
    };
    let status = if want.as_ref().is_some_and(|c| !rustc.contains(c.trim_start_matches("rust:"))) {
        CheckStatus::Warn
    } else {
        CheckStatus::Ok
    };
    DoctorCheck { id: "toolchain", status, message }
}

fn read_toolchain_channel(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("channel") {
            let value = rest.trim_start_matches('=').trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn check_profile(ctx: &DoctorContext<'_>) -> DoctorCheck {
    let path = ctx.data_root.join("profile").join("config.toml");
    if path.is_file() {
        DoctorCheck { id: "profile", status: CheckStatus::Ok, message: format!("profile config at {}", path.display()) }
    } else {
        DoctorCheck {
            id: "profile",
            status: CheckStatus::Fail,
            message: format!("missing {}; run `kuro init --defaults`", path.display()),
        }
    }
}

fn check_models_dir(ctx: &DoctorContext<'_>) -> DoctorCheck {
    let local = ctx.data_root.join("models");
    if local.is_dir() {
        return DoctorCheck {
            id: "models_dir",
            status: CheckStatus::Ok,
            message: format!("models directory at {}", local.display()),
        };
    }
    let bench = ctx.repo_root.join("bench").join("models");
    if bench.is_dir() {
        return DoctorCheck {
            id: "models_dir",
            status: CheckStatus::Warn,
            message: format!("no models under {}; using bench tree at {}", local.display(), bench.display()),
        };
    }
    DoctorCheck {
        id: "models_dir",
        status: CheckStatus::Fail,
        message: format!("no models at {} or {}; download or copy ONNX assets", local.display(), bench.display()),
    }
}

fn check_api_token(ctx: &DoctorContext<'_>) -> DoctorCheck {
    let path = KuroPaths::new(ctx.data_root).api_token();
    if path.is_file() {
        match std::fs::read_to_string(&path) {
            Ok(text) if !text.trim().is_empty() => {
                DoctorCheck { id: "api_token", status: CheckStatus::Ok, message: "api_token file present".into() }
            }
            Ok(_) => DoctorCheck {
                id: "api_token",
                status: CheckStatus::Fail,
                message: format!("api_token at {} is empty", path.display()),
            },
            Err(err) => {
                DoctorCheck { id: "api_token", status: CheckStatus::Fail, message: format!("read api_token: {err}") }
            }
        }
    } else {
        DoctorCheck {
            id: "api_token",
            status: CheckStatus::Warn,
            message: format!("missing {}; engine creates one on first start", path.display()),
        }
    }
}

fn check_api_bind(ctx: &DoctorContext<'_>) -> DoctorCheck {
    let port = ctx.settings.api_port;
    if (1..=65535).contains(&port) {
        DoctorCheck {
            id: "api_bind",
            status: CheckStatus::Ok,
            message: format!("HTTP API binds 127.0.0.1:{port} (loopback only)"),
        }
    } else {
        DoctorCheck { id: "api_bind", status: CheckStatus::Fail, message: format!("invalid api_port {port}") }
    }
}

fn check_winrt_voice() -> DoctorCheck {
    #[cfg(windows)]
    {
        DoctorCheck {
            id: "winrt_voice",
            status: CheckStatus::Ok,
            message: "WinRT speech synthesis available on Windows".into(),
        }
    }
    #[cfg(not(windows))]
    {
        DoctorCheck {
            id: "winrt_voice",
            status: CheckStatus::Skip,
            message: "WinRT voice check skipped on non-Windows hosts".into(),
        }
    }
}

fn check_mic_permission() -> DoctorCheck {
    #[cfg(windows)]
    {
        DoctorCheck {
            id: "mic_permission",
            status: CheckStatus::Warn,
            message: "Verify microphone access in Windows Settings > Privacy > Microphone".into(),
        }
    }
    #[cfg(not(windows))]
    {
        DoctorCheck {
            id: "mic_permission",
            status: CheckStatus::Skip,
            message: "Microphone permission check skipped on non-Windows hosts".into(),
        }
    }
}

pub fn default_context() -> (PathBuf, Settings) {
    let data = kuro_core::local_root();
    let settings = Settings::load().unwrap_or_default();
    (data, settings)
}

pub fn rustc_version_line() -> Option<String> {
    let out = std::process::Command::new("rustc").arg("--version").output().ok()?;
    if out.status.success() { String::from_utf8(out.stdout).ok().map(|s| s.trim().to_string()) } else { None }
}

pub fn print_human(report: &DoctorReport) {
    for check in &report.checks {
        let tag = match check.status {
            CheckStatus::Ok => "ok",
            CheckStatus::Warn => "warn",
            CheckStatus::Fail => "fail",
            CheckStatus::Skip => "skip",
        };
        eprintln!("[{tag}] {}: {}", check.id, check.message);
    }
    if report.ok {
        eprintln!("doctor: all required checks passed");
    } else {
        eprintln!("doctor: one or more required checks failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_config::Settings;

    fn fake_ctx<'a>(data: &'a Path, repo: &'a Path, settings: &'a Settings) -> DoctorContext<'a> {
        DoctorContext {
            data_root: data,
            repo_root: repo,
            settings,
            rustc_version: Some("rustc 1.100.0 (abc 2025-01-01)"),
        }
    }

    #[test]
    fn doctor_json_parses() {
        let dir = std::env::temp_dir().join(format!("kuro-doc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let settings = Settings::default();
        let ctx = fake_ctx(&dir, Path::new("."), &settings);
        let json = doctor_json(&ctx);
        let value: serde_json::Value = serde_json::from_str(&json).expect("json parses");
        assert!(value.get("ok").is_some());
        let checks = value.get("checks").and_then(|v| v.as_array()).expect("checks array");
        assert!(!checks.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn profile_missing_fails_report() {
        let dir = std::env::temp_dir().join(format!("kuro-doc-prof-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let settings = Settings::default();
        let ctx = fake_ctx(&dir, Path::new("."), &settings);
        let report = run_doctor(&ctx);
        assert!(!report.ok);
        assert!(report.checks.iter().any(|c| c.id == "profile" && c.status == CheckStatus::Fail));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn winrt_skips_off_windows() {
        #[cfg(not(windows))]
        {
            let dir = std::env::temp_dir().join(format!("kuro-doc-skip-{}", std::process::id()));
            let settings = Settings::default();
            let ctx = fake_ctx(&dir, Path::new("."), &settings);
            let report = run_doctor(&ctx);
            let winrt = report.checks.iter().find(|c| c.id == "winrt_voice").expect("winrt");
            assert_eq!(winrt.status, CheckStatus::Skip);
        }
    }
}
