//! Keep recent push-to-talk clips on disk so recognition can be measured against real speech.
//! Clips stay local under the Kuro data folder. Set `KURO_SAVE_AUDIO=0` to turn this off.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const KEEP_CLIPS: usize = 200;

pub fn clips_enabled() -> bool {
    std::env::var("KURO_SAVE_AUDIO").map(|v| v.trim() != "0").unwrap_or(true)
}

/// Save the raw 16 kHz mono clip as a 16-bit WAV. Returns the path, or None when off or on error.
pub fn save_clip(dir: &Path, pcm: &[f32]) -> Option<PathBuf> {
    if !clips_enabled() || pcm.is_empty() {
        return None;
    }
    std::fs::create_dir_all(dir).ok()?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_millis();
    let path = dir.join(format!("ptt-{stamp:020}.wav"));
    std::fs::write(&path, wav_bytes(pcm)).ok()?;
    prune(dir, KEEP_CLIPS);
    Some(path)
}

/// Write the transcript the engine routed next to its clip (same name, `.txt`).
pub fn write_transcript(clip: &Path, text: &str) {
    let _ = std::fs::write(clip.with_extension("txt"), text);
}

pub fn wav_bytes(pcm: &[f32]) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&16_000u32.to_le_bytes());
    out.extend_from_slice(&32_000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in pcm {
        let value = (sample.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut wavs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("wav"))
        .collect();
    wavs.sort();
    let extra = wavs.len().saturating_sub(keep);
    for old in wavs.into_iter().take(extra) {
        let _ = std::fs::remove_file(old.with_extension("txt"));
        let _ = std::fs::remove_file(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kuro-clip-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn wav_header_describes_16k_mono_16_bit() {
        let bytes = wav_bytes(&[0.0, 0.5, -0.5, 1.0]);
        assert_eq!(bytes.len(), 44 + 8);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 16_000);
        assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(bytes[34..36].try_into().unwrap()), 16);
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 8);
        assert_eq!(i16::from_le_bytes(bytes[46..48].try_into().unwrap()), 16_384);
    }

    #[test]
    fn prune_keeps_only_the_newest_clips_and_their_transcripts() {
        let dir = scratch("prune");
        for n in 0..5 {
            let wav = dir.join(format!("ptt-{n:020}.wav"));
            std::fs::write(&wav, b"x").unwrap();
            std::fs::write(wav.with_extension("txt"), b"t").unwrap();
        }
        prune(&dir, 3);
        let mut left: Vec<String> =
            std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left.len(), 6, "{left:?}");
        assert!(left.iter().any(|n| n.ends_with("00000000000000000004.wav")));
        assert!(!left.iter().any(|n| n.contains("00000000000000000001")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_clip_writes_wav_and_transcript_sidecar() {
        let dir = scratch("save");
        let path = save_clip(&dir, &[0.1; 1600]).expect("clip saved");
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 44 + 3200);
        write_transcript(&path, "send it to Siya");
        assert_eq!(std::fs::read_to_string(path.with_extension("txt")).unwrap(), "send it to Siya");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
