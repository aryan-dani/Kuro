//! One long-lived piper.exe child. stdin = one text line; stdout = raw int16 until idle gap.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct PiperChild {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

static PIPER: Mutex<Option<PiperChild>> = Mutex::new(None);

pub fn default_model() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("KURO_PIPER_MODEL") {
        let path = PathBuf::from(path.trim());
        if path.is_file() {
            return Some(path);
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("bench")
        .join("models")
        .join("piper")
        .join("en_US-lessac-medium.onnx");
    if bundled.is_file() {
        return Some(bundled);
    }
    None
}

pub fn piper_exe() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("KURO_PIPER_EXE") {
        let path = PathBuf::from(path.trim());
        if path.is_file() {
            return Some(path);
        }
    }
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    if let Some(root) = local {
        for candidate in [
            root.join("Kuro").join("piper").join("piper.exe"),
            root.join("Kuro").join("piper").join("piper").join("piper.exe"),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn shutdown() {
    let mut slot = match PIPER.lock() {
        Ok(slot) => slot,
        Err(poison) => poison.into_inner(),
    };
    if let Some(mut worker) = slot.take() {
        let _ = worker.stdin.flush();
        drop(worker.stdin);
        drop(worker.stdout);
        let _ = worker.child.kill();
        let _ = worker.child.wait();
    }
}

fn spawn_piper() -> Option<PiperChild> {
    let exe = piper_exe()?;
    let model = default_model()?;
    let mut command = Command::new(&exe);
    command
        .arg("--model")
        .arg(&model)
        .arg("--output_raw")
        .arg("--quiet")
        .current_dir(exe.parent().unwrap_or(std::path::Path::new(".")));
    if let Some(data) = exe.parent().map(|dir| dir.join("espeak-ng-data"))
        && data.is_dir()
    {
        command.arg("--espeak_data").arg(data);
    }
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let stdin = child.stdin.take()?;
    let stdout = child.stdout.take()?;
    Some(PiperChild { child, stdin, stdout })
}

fn ensure() -> Option<()> {
    let mut slot = PIPER.lock().ok()?;
    if slot.is_some() {
        return Some(());
    }
    *slot = spawn_piper();
    if slot.is_some() { Some(()) } else { None }
}

fn read_until_idle(stdout: &mut ChildStdout, expected_chars: usize) -> Option<Vec<u8>> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;

    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    // Piper pauses between sentences; 80ms was cutting long replies mid-thought.
    let idle = Duration::from_millis(550);
    let overall = Duration::from_secs((12 + expected_chars / 8).min(120) as u64);
    // ~22050 Hz mono int16 ≈ 12 chars/sec spoken → rough floor on byte length.
    let min_bytes = (expected_chars.saturating_mul(900)).max(2_000);
    let started = Instant::now();
    let mut last_data = Instant::now();
    let mut got_any = false;
    let handle = stdout.as_raw_handle();

    loop {
        if started.elapsed() > overall {
            break;
        }
        let mut avail: u32 = 0;
        let peek_ok = unsafe {
            PeekNamedPipe(handle, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut avail, std::ptr::null_mut())
        } != 0;
        if peek_ok && avail > 0 {
            let to_read = (avail as usize).min(chunk.len());
            match stdout.read(&mut chunk[..to_read]) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    got_any = true;
                    last_data = Instant::now();
                }
                Err(_) => break,
            }
        } else if got_any && last_data.elapsed() >= idle && buf.len() >= min_bytes {
            break;
        } else if got_any && last_data.elapsed() >= idle * 3 {
            // Absolute stop even if below floor (very short replies).
            break;
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    if buf.len() < 4 { None } else { Some(buf) }
}

/// Run the native Piper binary against the ONNX voice. Raw stdout is little-endian int16.
pub fn synthesize(text: &str) -> Option<Vec<f32>> {
    ensure()?;
    let line =
        text.chars().map(|ch| if ch.is_control() || ch == '\n' || ch == '\r' { ' ' } else { ch }).collect::<String>();
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let mut slot = PIPER.lock().ok()?;
    let worker = slot.as_mut()?;
    if writeln!(worker.stdin, "{line}").is_err() {
        *slot = None;
        return None;
    }
    if worker.stdin.flush().is_err() {
        *slot = None;
        return None;
    }
    let Some(bytes) = read_until_idle(&mut worker.stdout, line.chars().count()) else {
        if let Some(mut dead) = slot.take() {
            let _ = dead.child.kill();
            let _ = dead.child.wait();
        }
        return None;
    };
    let (chunks, _rest) = bytes.as_chunks::<2>();
    let samples = chunks.iter().map(|chunk| i16::from_le_bytes(*chunk) as f32 / 32768.0).collect::<Vec<_>>();
    if samples.iter().all(|s| s.abs() < 1e-6) {
        return None;
    }
    Some(samples)
}
