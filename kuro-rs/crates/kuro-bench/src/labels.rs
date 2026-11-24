use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ClipLabel {
    pub id: String,
    pub session: String,
    pub prompt: String,
    pub expected_text: String,
    pub expected_intent: String,
    pub conditions: Vec<String>,
    pub kind: String,
    pub duration_ms: u64,
}

pub fn load_labels(path: &Path) -> Result<Vec<ClipLabel>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path).map_err(|err| err.to_string())?;
    let mut out = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|err| err.to_string())?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        out.push(serde_json::from_str(line).map_err(|err| err.to_string())?);
    }
    Ok(out)
}

pub fn known_ids(path: &Path) -> Result<HashSet<String>, String> {
    Ok(load_labels(path)?.into_iter().map(|row| row.id).collect())
}

/// Append one label. Refuses when `id` is already present.
pub fn append_label(path: &Path, label: &ClipLabel) -> Result<(), String> {
    if known_ids(path)?.contains(&label.id) {
        return Err(format!("label {} already exists", label.id));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path).map_err(|err| err.to_string())?;
    let line = serde_json::to_string(label).map_err(|err| err.to_string())?;
    writeln!(file, "{line}").map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_refuses_duplicate_id() {
        let dir = std::env::temp_dir().join(format!("kuro-labels-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("labels.jsonl");
        let row = ClipLabel {
            id: "s1-desk-t1".into(),
            session: "1".into(),
            prompt: "Desk on".into(),
            expected_text: "Desk on".into(),
            expected_intent: "power_on".into(),
            conditions: vec!["quiet".into()],
            kind: "command".into(),
            duration_ms: 1000,
        };
        append_label(&path, &row).unwrap();
        assert!(append_label(&path, &row).is_err());
        assert_eq!(load_labels(&path).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
