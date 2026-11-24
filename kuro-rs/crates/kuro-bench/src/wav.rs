use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub fn write_wav_f32(path: &Path, samples: &[f32]) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    let data_len = (samples.len() * 2) as u32;
    let riff_len = 36 + data_len;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_len.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&16_000u32.to_le_bytes());
    bytes.extend_from_slice(&32_000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let scaled = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        bytes.extend_from_slice(&scaled.to_le_bytes());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = File::create(path).map_err(|err| err.to_string())?;
    file.write_all(&bytes).map_err(|err| err.to_string())?;
    Ok(())
}

pub fn read_wav_f32(path: &Path) -> Result<Vec<f32>, String> {
    let mut file = File::open(path).map_err(|err| err.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|err| err.to_string())?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a wav file".into());
    }
    let mut offset = 12;
    let mut data_start = None;
    let mut data_len = 0;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if id == b"data" {
            data_start = Some(offset + 8);
            data_len = size;
            break;
        }
        offset += 8 + size;
    }
    let start = data_start.ok_or_else(|| "wav has no data chunk".to_string())?;
    let end = (start + data_len).min(bytes.len());
    let pcm = &bytes[start..end];
    let (chunks, _) = pcm.as_chunks::<2>();
    let mut out = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let sample = i16::from_le_bytes(*chunk);
        out.push(sample as f32 / 32767.0);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_roundtrip_keeps_silence() {
        let dir = std::env::temp_dir().join(format!("kuro-wav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.wav");
        write_wav_f32(&path, &[0.0, 0.5, -0.5]).unwrap();
        let back = read_wav_f32(&path).unwrap();
        assert_eq!(back.len(), 3);
        assert!(back[0].abs() < 0.001);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
