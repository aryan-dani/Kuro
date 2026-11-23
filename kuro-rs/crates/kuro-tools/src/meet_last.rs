//! Last Meet link memory for join follow-ups. Runtime file under Kuro data dir only.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeetLastStore {
    pub last: Option<MeetLastEntry>,
    #[serde(default)]
    pub by_contact: HashMap<String, MeetLastEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetLastEntry {
    pub url: String,
    #[serde(default)]
    pub contact: String,
    #[serde(default)]
    pub ts: u64,
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn store_path() -> PathBuf {
    kuro_core::KuroPaths::from_env().meet_last()
}

fn contact_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

pub fn remember(path: &Path, url: &str, contact: &str) {
    let url = url.trim();
    if url.is_empty() {
        return;
    }
    let mut store = load(path);
    let entry = MeetLastEntry { url: url.to_string(), contact: contact.trim().to_string(), ts: unix_now() };
    store.last = Some(entry.clone());
    let key = contact_key(contact);
    if !key.is_empty() {
        store.by_contact.insert(key, entry);
    }
    let _ = save(path, &store);
}

pub fn lookup(path: &Path, contact: &str) -> Option<String> {
    let store = load(path);
    let key = contact_key(contact);
    if !key.is_empty()
        && let Some(entry) = store.by_contact.get(&key)
        && !entry.url.trim().is_empty()
    {
        return Some(entry.url.clone());
    }
    // Named miss still falls back to the most recent Meet Kuro created.
    store.last.map(|e| e.url).filter(|u| !u.trim().is_empty())
}

pub fn load(path: &Path) -> MeetLastStore {
    let Ok(raw) = fs::read_to_string(path) else {
        return MeetLastStore::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save(path: &Path, store: &MeetLastStore) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(store).unwrap_or_else(|_| "{}".into());
    fs::write(path, raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn remember_and_lookup_by_contact_and_last() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("kuro-meet-last-{stamp}"));
        let path = dir.join("meet_last.json");
        remember(&path, "https://meet.google.com/aaa-bbbb-ccc", "Siya");
        assert_eq!(lookup(&path, "siya").as_deref(), Some("https://meet.google.com/aaa-bbbb-ccc"));
        assert_eq!(lookup(&path, "").as_deref(), Some("https://meet.google.com/aaa-bbbb-ccc"));
        // Unknown contact still falls back to the last Meet Kuro made.
        assert_eq!(lookup(&path, "unknown").as_deref(), Some("https://meet.google.com/aaa-bbbb-ccc"));
        let _ = fs::remove_dir_all(&dir);
    }
}
