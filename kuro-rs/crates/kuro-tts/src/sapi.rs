use windows::Media::SpeechSynthesis::SpeechSynthesizer;
use windows::Storage::Streams::DataReader;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::HSTRING;

pub fn synthesize(text: &str) -> Result<Vec<f32>, String> {
    let bytes = synthesize_wav(text)?;
    Ok(wav_to_f32(&bytes).unwrap_or_default().1)
}

fn synthesize_wav(text: &str) -> Result<Vec<u8>, String> {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
    let synth = SpeechSynthesizer::new().map_err(|err| err.to_string())?;
    let stream = synth
        .SynthesizeTextToStreamAsync(&HSTRING::from(text))
        .map_err(|err| err.to_string())?
        .get()
        .map_err(|err| err.to_string())?;
    let size = stream.Size().map_err(|err| err.to_string())? as usize;
    if size == 0 {
        return Err("Windows speech returned no audio.".into());
    }
    let input = stream.GetInputStreamAt(0).map_err(|err| err.to_string())?;
    let reader = DataReader::CreateDataReader(&input).map_err(|err| err.to_string())?;
    reader.LoadAsync(size as u32).map_err(|err| err.to_string())?.get().map_err(|err| err.to_string())?;
    let mut bytes = vec![0u8; size];
    reader.ReadBytes(&mut bytes).map_err(|err| err.to_string())?;
    Ok(bytes)
}

pub fn wav_to_f32(bytes: &[u8]) -> Option<(u32, Vec<f32>)> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut offset = 12usize;
    let mut rate = 22050u32;
    let mut channels = 1u16;
    let mut bits = 16u16;
    let mut data = None;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let start = offset + 8;
        let end = start.saturating_add(size).min(bytes.len());
        if id == b"fmt " && end >= start + 16 {
            channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().ok()?);
            rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().ok()?);
            bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().ok()?);
        } else if id == b"data" {
            data = Some(&bytes[start..end]);
        }
        offset = end + (size & 1);
    }
    let data = data?;
    let samples: Vec<f32> = match bits {
        16 => {
            let (chunks, _rest) = data.as_chunks::<2>();
            chunks.iter().map(|chunk| i16::from_le_bytes(*chunk) as f32 / 32768.0).collect()
        }
        32 => {
            let (chunks, _rest) = data.as_chunks::<4>();
            chunks.iter().map(|chunk| f32::from_le_bytes(*chunk)).collect()
        }
        _ => return None,
    };
    let mono = if channels <= 1 {
        samples
    } else {
        samples.chunks(channels as usize).map(|frame| frame.iter().sum::<f32>() / channels as f32).collect()
    };
    Some((rate, mono))
}
