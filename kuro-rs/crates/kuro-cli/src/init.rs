use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// User profile directory: `%LOCALAPPDATA%\Kuro\profile` or `KURO_DATA_DIR\profile`.
pub fn user_profile_dir() -> PathBuf {
    kuro_core::local_root().join("profile")
}

/// Repository root that contains `profiles/example`.
pub fn discover_repo_root() -> PathBuf {
    for root in kuro_core::models::repo_root_candidates() {
        let example = root.join("profiles").join("example");
        if example.is_dir() {
            return root;
        }
    }
    kuro_config::repo_root()
}

/// Tracked example profile in the repository.
pub fn example_profile_dir() -> PathBuf {
    discover_repo_root().join("profiles").join("example")
}

#[derive(Debug, PartialEq, Eq)]
pub struct InitResult {
    pub profile_dir: PathBuf,
    pub copied: usize,
    pub skipped: usize,
}

#[derive(Debug)]
pub enum InitError {
    Io(io::Error),
    ExampleMissing(PathBuf),
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::ExampleMissing(p) => write!(f, "example profile missing at {}", p.display()),
        }
    }
}

impl std::error::Error for InitError {}

impl From<io::Error> for InitError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Copy `profiles/example` into the user profile location. Existing files are kept unless `force`.
pub fn init_defaults(force: bool) -> Result<InitResult, InitError> {
    let source = example_profile_dir();
    if !source.is_dir() {
        return Err(InitError::ExampleMissing(source));
    }
    let dest = user_profile_dir();
    fs::create_dir_all(&dest)?;
    let (copied, skipped) = copy_tree(&source, &dest, force)?;
    Ok(InitResult { profile_dir: dest, copied, skipped })
}

fn copy_tree(source: &Path, dest: &Path, force: bool) -> Result<(usize, usize), io::Error> {
    let mut copied = 0usize;
    let mut skipped = 0usize;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        let from = entry.path();
        let to = dest.join(&name);
        if file_type.is_dir() {
            fs::create_dir_all(&to)?;
            let (c, s) = copy_tree(&from, &to, force)?;
            copied += c;
            skipped += s;
        } else if file_type.is_file() {
            if to.exists() && !force {
                skipped += 1;
                continue;
            }
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)?;
            copied += 1;
        }
    }
    Ok((copied, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        _lock: MutexGuard<'static, ()>,
        saved_data: Option<String>,
        saved_root: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                _lock: ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner()),
                saved_data: std::env::var("KURO_DATA_DIR").ok(),
                saved_root: std::env::var("KURO_ROOT").ok(),
            }
        }

        fn set_data_dir(&mut self, path: &Path) {
            // Safety: test-only env mutation.
            unsafe { std::env::set_var("KURO_DATA_DIR", path) };
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.saved_data {
                Some(v) => unsafe { std::env::set_var("KURO_DATA_DIR", v) },
                None => unsafe { std::env::remove_var("KURO_DATA_DIR") },
            }
            match &self.saved_root {
                Some(v) => unsafe { std::env::set_var("KURO_ROOT", v) },
                None => unsafe { std::env::remove_var("KURO_ROOT") },
            }
        }
    }

    #[test]
    fn init_is_idempotent_without_force() {
        let base = std::env::temp_dir().join(format!("kuro-init-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let mut guard = EnvGuard::new();
        guard.set_data_dir(&base);

        let first = init_defaults(false).expect("first init");
        assert!(first.copied > 0);
        let marker = user_profile_dir().join("config.toml");
        assert!(marker.is_file());
        fs::write(&marker, "llm_mode = \"off\"\n").unwrap();

        let second = init_defaults(false).expect("second init");
        assert_eq!(second.skipped, first.copied);
        assert_eq!(second.copied, 0);
        let after = fs::read_to_string(&marker).unwrap();
        assert_eq!(after, "llm_mode = \"off\"\n");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn init_writes_outside_repo_tree() {
        let base = std::env::temp_dir().join(format!("kuro-init-out-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let mut guard = EnvGuard::new();
        guard.set_data_dir(&base);
        let root = discover_repo_root();
        init_defaults(false).expect("init");
        let profile = user_profile_dir();
        assert!(profile.starts_with(&base));
        assert!(!profile.starts_with(&root) || base != root);
        let _ = fs::remove_dir_all(&base);
    }
}
