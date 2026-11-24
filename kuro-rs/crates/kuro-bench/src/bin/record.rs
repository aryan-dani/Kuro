//! Capture raw 16 kHz clips through the production microphone path.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Instant;

use kuro_audio::{PttBuffer, start_ptt_capture};
use kuro_bench::{
    ClipLabel, PromptSpec, append_label, catalog_for_session, conditions_for_session, known_ids, voice_dir,
    write_wav_f32,
};

fn main() {
    let session = arg_u8("--session").unwrap_or(1).clamp(1, 3);
    let resume = std::env::args().any(|a| a == "--resume");
    let dir = voice_dir().join(format!("session{session}"));
    let labels_path = dir.join("labels.jsonl");
    let names = contact_names();
    let aliases = workflow_aliases();
    let prompts = catalog_for_session(session, &names, &aliases);
    let known = known_ids(&labels_path).unwrap_or_default();
    println!(
        "session {session} clips {} conditions {} contacts {} aliases {}",
        prompts.len(),
        conditions_for_session(session).join(","),
        names.len(),
        aliases.len()
    );
    println!("Same desk, AC power, Performance mode. Enter starts a clip. Enter stops it.");
    let mut done = 0usize;
    for (index, prompt) in prompts.iter().enumerate() {
        if resume && known.contains(&prompt.id) {
            continue;
        }
        if wav_path(&dir, &prompt.id).exists() {
            println!("skip existing {}", prompt.id);
            continue;
        }
        let left = prompts.len().saturating_sub(index);
        let eta_min = (left as u64 * 15) / 60;
        println!("\n[{}/{}] eta about {eta_min} min", index + 1, prompts.len());
        println!("Say: {}", prompt.prompt);
        println!("Press Enter to start.");
        wait_enter();
        let started = Instant::now();
        match capture_clip() {
            Ok(pcm) => {
                let ms = started.elapsed().as_millis() as u64;
                let path = wav_path(&dir, &prompt.id);
                if path.exists() {
                    println!("refusing overwrite {}", path.display());
                    continue;
                }
                if let Err(err) = write_wav_f32(&path, &pcm) {
                    println!("wav failed: {err}");
                    continue;
                }
                let label = label_for(prompt, session, ms);
                if let Err(err) = append_label(&labels_path, &label) {
                    println!("label failed: {err}");
                    continue;
                }
                done += 1;
                println!("saved {} ({ms} ms, {} samples)", prompt.id, pcm.len());
            }
            Err(err) => println!("mic failed: {err}"),
        }
    }
    println!("session {session} wrote {done} new clips in {}", dir.display());
}

fn capture_clip() -> Result<Vec<f32>, String> {
    println!("Recording. Press Enter to stop.");
    let (mut buffer, producer) = PttBuffer::new();
    let (session, mic) = start_ptt_capture(producer)?;
    println!("mic: {mic}");
    wait_enter();
    drop(session);
    buffer.drain_ring();
    Ok(buffer.pcm)
}

fn wait_enter() {
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    let _ = std::io::stdout().flush();
}

fn wav_path(dir: &std::path::Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.wav"))
}

fn label_for(prompt: &PromptSpec, session: u8, duration_ms: u64) -> ClipLabel {
    ClipLabel {
        id: prompt.id.clone(),
        session: session.to_string(),
        prompt: prompt.prompt.clone(),
        expected_text: prompt.expected_text.clone(),
        expected_intent: prompt.expected_intent.clone(),
        conditions: conditions_for_session(session),
        kind: prompt.kind.clone(),
        duration_ms,
    }
}

fn arg_u8(flag: &str) -> Option<u8> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == flag {
            return args.next().and_then(|v| v.parse().ok());
        }
        if let Some(v) = arg.strip_prefix(&format!("{flag}=")) {
            return v.parse().ok();
        }
    }
    None
}

fn contact_names() -> Vec<String> {
    let Some(path) = local_kuro().map(|p| p.join("contacts.json")) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    value
        .as_object()
        .map(|map| map.keys().filter(|k| !k.eq_ignore_ascii_case("me")).cloned().collect())
        .unwrap_or_default()
}

fn workflow_aliases() -> Vec<String> {
    let Some(path) = local_kuro().map(|p| p.join("n8n_workflows.json")) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let Some(flows) = value.get("workflows").and_then(|v| v.as_object()) else {
        return names;
    };
    for spec in flows.values() {
        if let Some(list) = spec.get("aliases").and_then(|v| v.as_array()) {
            for alias in list {
                if let Some(text) = alias.as_str()
                    && !text.trim().is_empty()
                {
                    names.push(text.trim().to_string());
                }
            }
        }
    }
    names
}

fn local_kuro() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|root| PathBuf::from(root).join("Kuro"))
}
