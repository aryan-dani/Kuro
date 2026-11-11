use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

pub fn load_cache(path: &Path) -> Value {
    fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}))
}

pub fn save_cache(path: &Path, ip: &str, version: f64) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let body = json!({"ip": ip, "version": version, "updated": 0});
    let _ = fs::write(path, serde_json::to_string_pretty(&body).unwrap_or_default());
}

fn scene_action(scene: &str) -> Option<&'static str> {
    match scene {
        "daddy_home" | "power_on" => Some("on"),
        "good_night" | "power_off" => Some("off"),
        other => {
            let low = other.to_ascii_lowercase();
            if low.contains("off") {
                Some("off")
            } else if ["home", "daddy", "wake", "on", "light"].iter().any(|k| low.contains(k)) {
                Some("on")
            } else {
                None
            }
        }
    }
}

pub fn run(arguments: &Value) -> String {
    let mut scene = arguments.get("scene").and_then(|v| v.as_str()).unwrap_or("").trim().to_ascii_lowercase();
    if !["daddy_home", "good_night", "power_on", "power_off"].contains(&scene.as_str()) {
        scene = match scene_action(&scene) {
            Some("on") => "power_on".into(),
            Some("off") => "power_off".into(),
            _ => return "I couldn't reach the plug.".into(),
        };
    }
    let id = std::env::var("TUYA_DEVICE_ID").unwrap_or_default();
    let key = std::env::var("TUYA_LOCAL_KEY").unwrap_or_default();
    let ip = std::env::var("TUYA_IP").unwrap_or_default();
    if id.is_empty() || key.is_empty() {
        return "Smart plug credentials are missing in .env.".into();
    }
    let cache_path = kuro_core::KuroPaths::from_env().tuya_endpoint();
    let cache = load_cache(&cache_path);
    let target = cache.get("ip").and_then(|v| v.as_str()).unwrap_or(if ip.is_empty() { "127.0.0.1" } else { &ip });
    let version = cache
        .get("version")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| std::env::var("TUYA_VERSION").ok().and_then(|v| v.parse().ok()).unwrap_or(3.3));
    let on = scene_action(&scene) == Some("on");
    match send_switch(target, &id, &key, version, on) {
        Ok(()) => {
            save_cache(&cache_path, target, version);
            "Success: Desk power updated.".into()
        }
        Err(message) => message,
    }
}

fn send_switch(ip: &str, device_id: &str, local_key: &str, version: f64, on: bool) -> Result<(), String> {
    if (3.4..3.5).contains(&version) {
        if send_switch_34(ip, device_id, local_key, on).is_ok() {
            return Ok(());
        }
    } else {
        let frame = crate::tuya::control_frame(version, 1, local_key, device_id, on)
            .ok_or_else(|| "Smart plug credentials are missing in .env.".to_string())?;
        if write_frame(ip, &frame).is_ok() {
            return Ok(());
        }
    }
    Err("I couldn't reach the plug.".into())
}

fn send_switch_34(ip: &str, device_id: &str, local_key: &str, on: bool) -> Result<(), ()> {
    let key = crate::tuya::key16(local_key).ok_or(())?;
    let mut local_nonce = [0u8; 16];
    for (i, b) in local_nonce.iter_mut().enumerate() {
        *b = (i as u8).wrapping_add(1);
    }
    let addr = format!("{ip}:6668").parse().map_err(|_| ())?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|_| ())?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();

    let start = crate::tuya::negotiate_start_34(1, &local_nonce);
    stream.write_all(&start).map_err(|_| ())?;
    let remote = read_sess_key(&mut stream)?;
    let finish = crate::tuya::negotiate_finish_34(3, &key, &local_nonce, &remote);
    stream.write_all(&finish).map_err(|_| ())?;
    let _ = read_some(&mut stream);

    let session = crate::tuya::session_key_34(&key, &remote);
    let json = crate::tuya::control_json(device_id, on);
    let frame = crate::tuya::pack_55aa(5, crate::tuya::CMD_CONTROL, &crate::tuya::payload_34(&session, &json));
    stream.write_all(&frame).map_err(|_| ())?;
    let _ = read_some(&mut stream);
    Ok(())
}

fn read_sess_key(stream: &mut TcpStream) -> Result<Vec<u8>, ()> {
    let mut buf = [0u8; 512];
    let n = stream.read(&mut buf).map_err(|_| ())?;
    if n < 16 {
        return Err(());
    }
    let frame = &buf[..n];
    if crate::tuya::frame_cmd(frame) != Some(crate::tuya::CMD_SESS_KEY) {
        // Some firmwares ack differently; take trailing 16 bytes as nonce when present.
        if n >= 16 {
            return Ok(frame[n - 16..].to_vec());
        }
        return Err(());
    }
    // 55AA header: 4+4+4+4 = 16, then payload, then crc+suffix.
    if frame.len() < 24 {
        return Err(());
    }
    let payload_len = u32::from_be_bytes(frame[12..16].try_into().map_err(|_| ())?) as usize;
    if payload_len < 8 || 16 + payload_len > frame.len() {
        return Err(());
    }
    let payload = &frame[16..16 + payload_len - 8];
    Ok(payload.to_vec())
}

fn read_some(stream: &mut TcpStream) -> usize {
    let mut buf = [0u8; 64];
    stream.read(&mut buf).unwrap_or(0)
}

fn write_frame(ip: &str, frame: &[u8]) -> Result<(), ()> {
    let addr = format!("{ip}:6668").parse().map_err(|_| ())?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|_| ())?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.write_all(frame).map_err(|_| ())?;
    let mut buf = [0u8; 16];
    let _ = stream.read(&mut buf);
    Ok(())
}

pub fn bulb_run(arguments: &Value) -> String {
    let action = arguments.get("action").and_then(|v| v.as_str()).unwrap_or("").trim().to_ascii_lowercase();
    if action.is_empty() {
        return "I couldn't reach the bulb.".into();
    }
    let on = !matches!(action.as_str(), "off");
    let id = std::env::var("TUYA_DEVICE_ID").unwrap_or_default();
    let key = std::env::var("TUYA_LOCAL_KEY").unwrap_or_default();
    let ip = std::env::var("TUYA_IP").unwrap_or_default();
    if id.is_empty() || key.is_empty() || ip.is_empty() {
        return "I couldn't reach the bulb.".into();
    }
    let version = std::env::var("TUYA_VERSION").ok().and_then(|v| v.parse().ok()).unwrap_or(3.3);
    match send_switch(&ip, &id, &key, version, on) {
        Ok(()) => format!("Success: Bulb {action}."),
        Err(_) => "I couldn't reach the bulb.".into(),
    }
}
