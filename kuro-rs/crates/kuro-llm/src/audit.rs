//! In-memory audit rows for agent tool attempts (phase 26A).

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditEntry {
    pub turn_id: u64,
    pub source: &'static str,
    pub feature_id: String,
    pub tier: String,
    pub result: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuditLog {
    rows: Vec<AuditEntry>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: AuditEntry) {
        self.rows.push(entry);
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.rows
    }
}
