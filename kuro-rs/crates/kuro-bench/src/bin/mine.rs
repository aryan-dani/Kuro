//! Mine engine logs for command-shaped lines the phrase classifier missed.

use std::path::PathBuf;

use kuro_bench::{mine_logs, voice_dir};

fn main() {
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let Some(root) = root else {
        eprintln!("LOCALAPPDATA is unset");
        std::process::exit(1);
    };
    let dir = root.join("Kuro");
    let logs = [dir.join("kuro.log"), dir.join("kuro.log.1")];
    let dest = voice_dir().join("mined_errors.txt");
    match mine_logs(&logs, &dest) {
        Ok(n) => println!("wrote {n} lines to {}", dest.display()),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
