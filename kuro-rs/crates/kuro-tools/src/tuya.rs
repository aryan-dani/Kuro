//! Tuya local protocol frames: 3.3 (55AA + AES-ECB), 3.4 (session HMAC), 3.5 (6699 + AES-GCM).

use aes::Aes128;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes128Gcm, Nonce};
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const PREFIX_55AA: u32 = 0x0000_55AA;
pub const SUFFIX_55AA: u32 = 0x0000_AA55;
pub const PREFIX_6699: u32 = 0x0000_6699;
pub const SUFFIX_6699: u32 = 0x0000_9966;
pub const CMD_SESS_START: u32 = 0x03;
pub const CMD_SESS_KEY: u32 = 0x04;
pub const CMD_SESS_FINISH: u32 = 0x05;
pub const CMD_CONTROL: u32 = 0x07;

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("hmac key");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

pub fn key16(local_key: &str) -> Option<[u8; 16]> {
    let raw = local_key.as_bytes();
    if raw.len() < 16 {
        return None;
    }
    let mut key = [0u8; 16];
    key.copy_from_slice(&raw[..16]);
    Some(key)
}

fn pkcs7(plain: &[u8]) -> Vec<u8> {
    let pad = 16 - (plain.len() % 16);
    let mut out = plain.to_vec();
    out.extend(std::iter::repeat_n(pad as u8, pad));
    out
}

pub fn encrypt_ecb(key: &[u8; 16], plain: &[u8]) -> Vec<u8> {
    let cipher = Aes128::new(key.into());
    let mut padded = pkcs7(plain);
    for chunk in padded.chunks_mut(16) {
        let mut block = aes::cipher::generic_array::GenericArray::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        chunk.copy_from_slice(&block);
    }
    padded
}

pub fn decrypt_ecb(key: &[u8; 16], cipher_bytes: &[u8]) -> Option<Vec<u8>> {
    if cipher_bytes.is_empty() || !cipher_bytes.len().is_multiple_of(16) {
        return None;
    }
    let cipher = Aes128::new(key.into());
    let mut data = cipher_bytes.to_vec();
    for chunk in data.chunks_mut(16) {
        let mut block = aes::cipher::generic_array::GenericArray::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        chunk.copy_from_slice(&block);
    }
    let pad = *data.last()? as usize;
    if pad == 0 || pad > 16 || pad > data.len() {
        return None;
    }
    data.truncate(data.len() - pad);
    Some(data)
}

/// 3.3 body: protocol tag, 12 zero bytes, then raw AES-ECB ciphertext.
pub fn payload_33(key: &[u8; 16], json: &[u8]) -> Vec<u8> {
    let mut body = b"3.3".to_vec();
    body.extend(std::iter::repeat_n(0u8, 12));
    body.extend(encrypt_ecb(key, json));
    body
}

pub fn pack_55aa(seq: u32, cmd: u32, payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(16 + payload.len() + 8);
    msg.extend(PREFIX_55AA.to_be_bytes());
    msg.extend(seq.to_be_bytes());
    msg.extend(cmd.to_be_bytes());
    msg.extend(((payload.len() + 8) as u32).to_be_bytes());
    msg.extend(payload);
    let crc = crc32(&msg);
    msg.extend(crc.to_be_bytes());
    msg.extend(SUFFIX_55AA.to_be_bytes());
    msg
}

pub fn pack_6699(seq: u32, cmd: u32, payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(20 + payload.len() + 8);
    msg.extend(PREFIX_6699.to_be_bytes());
    msg.extend(0u32.to_be_bytes());
    msg.extend(seq.to_be_bytes());
    msg.extend(cmd.to_be_bytes());
    msg.extend(((payload.len() + 8) as u32).to_be_bytes());
    msg.extend(payload);
    let crc = crc32(&msg);
    msg.extend(crc.to_be_bytes());
    msg.extend(SUFFIX_6699.to_be_bytes());
    msg
}

pub fn frame_crc_ok(frame: &[u8]) -> bool {
    if frame.len() < 16 {
        return false;
    }
    let (body, tail) = frame.split_at(frame.len() - 8);
    let crc = u32::from_be_bytes(tail[0..4].try_into().unwrap_or([0; 4]));
    crc32(body) == crc
}

pub fn frame_cmd(frame: &[u8]) -> Option<u32> {
    if frame.len() < 12 {
        return None;
    }
    let prefix = u32::from_be_bytes(frame[0..4].try_into().ok()?);
    let cmd_at = if prefix == PREFIX_6699 { 12 } else { 8 };
    if frame.len() < cmd_at + 4 {
        return None;
    }
    Some(u32::from_be_bytes(frame[cmd_at..cmd_at + 4].try_into().ok()?))
}

pub fn session_key_34(local_key: &[u8; 16], remote_nonce: &[u8]) -> [u8; 16] {
    let full = hmac_sha256(local_key, remote_nonce);
    let mut key = [0u8; 16];
    key.copy_from_slice(&full[..16]);
    key
}

pub fn negotiate_start_34(seq: u32, local_nonce: &[u8]) -> Vec<u8> {
    pack_55aa(seq, CMD_SESS_START, local_nonce)
}

pub fn negotiate_finish_34(seq: u32, local_key: &[u8; 16], local_nonce: &[u8], remote_nonce: &[u8]) -> Vec<u8> {
    let session = session_key_34(local_key, remote_nonce);
    let tag = hmac_sha256(&session, local_nonce);
    let mut payload = b"3.4".to_vec();
    payload.extend(std::iter::repeat_n(0u8, 12));
    payload.extend(tag);
    pack_55aa(seq, CMD_SESS_FINISH, &payload)
}

pub fn payload_34(session: &[u8; 16], json: &[u8]) -> Vec<u8> {
    let mut body = b"3.4".to_vec();
    body.extend(std::iter::repeat_n(0u8, 12));
    body.extend(encrypt_ecb(session, json));
    body
}

pub fn encrypt_gcm(key: &[u8; 16], nonce: &[u8; 12], plain: &[u8]) -> Vec<u8> {
    let cipher = Aes128Gcm::new(key.into());
    let n = Nonce::from_slice(nonce);
    let mut out = nonce.to_vec();
    let ct = cipher.encrypt(n, Payload { msg: plain, aad: b"" }).unwrap_or_default();
    out.extend(ct);
    out
}

pub fn decrypt_gcm(key: &[u8; 16], packed: &[u8]) -> Option<Vec<u8>> {
    if packed.len() < 12 + 16 {
        return None;
    }
    let (nonce, ct) = packed.split_at(12);
    let cipher = Aes128Gcm::new(key.into());
    cipher.decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad: b"" }).ok()
}

pub fn pack_35(seq: u32, cmd: u32, key: &[u8; 16], nonce: &[u8; 12], json: &[u8]) -> Vec<u8> {
    let secret = encrypt_gcm(key, nonce, json);
    let mut body = b"3.5".to_vec();
    body.extend(std::iter::repeat_n(0u8, 12));
    body.extend(secret);
    pack_6699(seq, cmd, &body)
}

pub fn control_json(device_id: &str, on: bool) -> Vec<u8> {
    serde_json::json!({
        "devId": device_id,
        "dps": {"1": on},
        "uid": device_id,
        "t": "0",
    })
    .to_string()
    .into_bytes()
}

pub fn control_frame(version: f64, seq: u32, local_key: &str, device_id: &str, on: bool) -> Option<Vec<u8>> {
    let key = key16(local_key)?;
    let json = control_json(device_id, on);
    if version >= 3.5 {
        let nonce = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        Some(pack_35(seq, CMD_CONTROL, &key, &nonce, &json))
    } else if version >= 3.4 {
        Some(pack_55aa(seq, CMD_CONTROL, &payload_34(&key, &json)))
    } else {
        Some(pack_55aa(seq, CMD_CONTROL, &payload_33(&key, &json)))
    }
}
