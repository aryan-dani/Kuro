use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Debug)]
struct OpenCmd {
    kind: String,
    last: Instant,
}

/// Stage timings. Disabled writes nothing. Rows never store transcript text.
#[derive(Debug)]
pub struct Timing {
    pub enabled: bool,
    pub path: PathBuf,
    open: std::collections::HashMap<String, OpenCmd>,
    active: String,
    seq: u64,
}

impl Timing {
    pub fn new(path: impl Into<PathBuf>, enabled: bool) -> Self {
        Self { enabled, path: path.into(), open: std::collections::HashMap::new(), active: String::new(), seq: 0 }
    }

    pub fn begin(&mut self, kind: &str) -> String {
        if !self.enabled {
            return String::new();
        }
        self.seq += 1;
        let id = format!("{:012x}", self.seq);
        let now = Instant::now();
        self.open.insert(id.clone(), OpenCmd { kind: kind.to_string(), last: now });
        self.active = id.clone();
        id
    }

    pub fn mark(&mut self, stage: &str) {
        self.mark_id(stage, "");
    }

    pub fn mark_id(&mut self, stage: &str, command_id: &str) {
        if !self.enabled {
            return;
        }
        let command_id = if command_id.is_empty() { self.active.as_str() } else { command_id };
        if command_id.is_empty() {
            return;
        }
        let Some(row) = self.open.get_mut(command_id) else {
            return;
        };
        let now = Instant::now();
        let delta = now.saturating_duration_since(row.last).as_secs_f64() * 1000.0;
        row.last = now;
        let kind = row.kind.clone();
        let id = command_id.to_string();
        self.append(&id, &kind, stage, delta);
    }

    pub fn finish(&mut self, command_id: &str) {
        if !self.enabled {
            return;
        }
        let command_id = if command_id.is_empty() { self.active.clone() } else { command_id.to_string() };
        self.open.remove(&command_id);
        if self.active == command_id {
            self.active.clear();
        }
    }

    pub fn once(&mut self, kind: &str, stage: &str, ms: f64) {
        if !self.enabled {
            return;
        }
        self.seq += 1;
        let id = format!("{:012x}", self.seq);
        self.append(&id, kind, stage, ms);
    }

    fn append(&self, command_id: &str, kind: &str, stage: &str, ms: f64) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let new_file = !self.path.is_file();
        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.path) else {
            return;
        };
        if new_file {
            let _ = writeln!(file, "command_id,kind,stage,ms");
        }
        let _ = writeln!(file, "{command_id},{kind},{stage},{ms:.1}");
    }
}
