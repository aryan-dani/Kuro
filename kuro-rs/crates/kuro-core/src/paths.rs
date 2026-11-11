use std::path::{Path, PathBuf};

/// Files under `%LOCALAPPDATA%\Kuro`.
#[derive(Clone, Debug)]
pub struct KuroPaths {
    pub root: PathBuf,
}

impl KuroPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn from_env() -> Self {
        Self::new(crate::local_root())
    }

    fn join(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn api_token(&self) -> PathBuf {
        self.join("api_token")
    }
    pub fn log(&self) -> PathBuf {
        self.join("kuro.log")
    }
    pub fn timing_csv(&self) -> PathBuf {
        self.join("timing.csv")
    }
    pub fn tuya_endpoint(&self) -> PathBuf {
        self.join("tuya_endpoint.json")
    }
    pub fn workflows(&self) -> PathBuf {
        self.join("n8n_workflows.json")
    }
    pub fn contacts(&self) -> PathBuf {
        self.join("contacts.json")
    }
    pub fn wa_session(&self) -> PathBuf {
        self.join("wa_session.db")
    }
    pub fn google_token(&self) -> PathBuf {
        self.root.join("google").join("token.json")
    }
    pub fn heartbeat(&self) -> PathBuf {
        self.join("heartbeat.txt")
    }
    pub fn home_ssid(&self) -> PathBuf {
        self.join("home_ssid.txt")
    }
    pub fn disabled_flag(&self) -> PathBuf {
        self.join("DISABLED")
    }
    pub fn away_ok(&self) -> PathBuf {
        self.join("AWAY_OK")
    }
    pub fn wifi_log(&self) -> PathBuf {
        self.join("wifi-watch.log")
    }
    pub fn legion_mode(&self) -> PathBuf {
        self.join("legion_mode.json")
    }
    pub fn singleton_lock(&self) -> PathBuf {
        self.join("kuro.lock")
    }

    pub fn exists_flag(path: &Path) -> bool {
        path.is_file()
    }
}
