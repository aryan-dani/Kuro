use kuro_stt::{initial_prompt, transcribe_pcm};

fn speak_wav(text: &str) -> Result<(u32, Vec<f32>), String> {
    let script = format!(
        "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $ms = New-Object System.IO.MemoryStream; $s.SetOutputToWaveStream($ms); $s.Speak({text:?}); [Console]::Out.Write([Convert]::ToBase64String($ms.ToArray()))"
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let b64 = String::from_utf8_lossy(&output.stdout);
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim())
        .map_err(|err| err.to_string())?;
    decode_wav(&bytes).ok_or_else(|| "SAPI did not return a WAV.".into())
}

fn decode_wav(bytes: &[u8]) -> Option<(u32, Vec<f32>)> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" {
        return None;
    }
    let mut offset = 12usize;
    let mut rate = 22050u32;
    let mut bits = 16u16;
    let mut data = None;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let start = offset + 8;
        let end = start.saturating_add(size).min(bytes.len());
        if id == b"fmt " && end >= start + 16 {
            rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().ok()?);
            bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().ok()?);
        } else if id == b"data" {
            data = Some(&bytes[start..end]);
        }
        offset = end + (size & 1);
    }
    let data = data?;
    let samples = if bits == 16 {
        data.as_chunks::<2>().0.iter().map(|chunk| i16::from_le_bytes(*chunk) as f32 / 32768.0).collect()
    } else {
        return None;
    };
    Some((rate, samples))
}

#[test]
fn spoken_wav_returns_a_real_transcript() {
    if kuro_stt::ggml_path().is_none() {
        eprintln!("skip spoken_wav_returns_a_real_transcript: Whisper GGML model not present");
        return;
    }
    let (rate, pcm) = speak_wav("hello").expect("windows speech");
    assert!(pcm.len() > 1000, "speech clip too short");
    let prompt = initial_prompt(&[], &[]);
    eprintln!("pcm_len={} rate={}", pcm.len(), rate);
    let start = std::time::Instant::now();
    let _ = transcribe_pcm(&pcm, rate, false, &prompt).expect("whisper warm");
    let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = std::time::Instant::now();
    let text = transcribe_pcm(&pcm, rate, false, &prompt).expect("whisper");
    let warm_ms = start.elapsed().as_secs_f64() * 1000.0;
    eprintln!("cold_stt_ms={cold_ms:.1} warm_stt_ms={warm_ms:.1}");
    let lower = text.to_lowercase();
    assert!(lower.contains("hello"), "transcript was {text:?}");
}
