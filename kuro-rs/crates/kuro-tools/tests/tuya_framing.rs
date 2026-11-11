use kuro_tools::screen::encode_png;
use kuro_tools::tuya;

fn key() -> [u8; 16] {
    *b"0123456789abcdef"
}

#[test]
fn v33_frame_is_55aa_ecb_and_crc() {
    let json = br#"{"dps":{"1":true}}"#;
    let body = tuya::payload_33(&key(), json);
    assert!(body.starts_with(b"3.3"));
    let frame = tuya::pack_55aa(1, tuya::CMD_CONTROL, &body);
    assert_eq!(&frame[0..4], &tuya::PREFIX_55AA.to_be_bytes());
    assert_eq!(&frame[frame.len() - 4..], &tuya::SUFFIX_55AA.to_be_bytes());
    assert!(tuya::frame_crc_ok(&frame));
    assert_eq!(tuya::frame_cmd(&frame), Some(tuya::CMD_CONTROL));
    let cipher = tuya::encrypt_ecb(&key(), json);
    let plain = tuya::decrypt_ecb(&key(), &cipher).unwrap();
    assert_eq!(plain, json);
}

#[test]
fn v34_negotiation_uses_03_04_05_and_hmac_session() {
    let local = key();
    let remote = b"remote-nonce-16b";
    let start = tuya::negotiate_start_34(1, b"local-nonce");
    assert_eq!(&start[0..4], &tuya::PREFIX_55AA.to_be_bytes());
    assert_eq!(tuya::frame_cmd(&start), Some(tuya::CMD_SESS_START));
    let device = tuya::pack_55aa(2, tuya::CMD_SESS_KEY, remote);
    assert_eq!(tuya::frame_cmd(&device), Some(0x04));
    let finish = tuya::negotiate_finish_34(3, &local, b"local-nonce", remote);
    assert_eq!(tuya::frame_cmd(&finish), Some(tuya::CMD_SESS_FINISH));
    assert!(tuya::frame_crc_ok(&finish));
    let session = tuya::session_key_34(&local, remote);
    assert_eq!(session.len(), 16);
    assert_ne!(session, local);
    let payload = tuya::payload_34(&session, br#"{"dps":{"1":false}}"#);
    assert!(payload.starts_with(b"3.4"));
}

#[test]
fn v35_frame_is_6699_gcm() {
    let nonce = [9u8; 12];
    let json = br#"{"dps":{"1":true}}"#;
    let frame = tuya::pack_35(1, tuya::CMD_CONTROL, &key(), &nonce, json);
    assert_eq!(&frame[0..4], &tuya::PREFIX_6699.to_be_bytes());
    assert_eq!(&frame[frame.len() - 4..], &tuya::SUFFIX_6699.to_be_bytes());
    assert!(tuya::frame_crc_ok(&frame));
    assert_eq!(tuya::frame_cmd(&frame), Some(tuya::CMD_CONTROL));
    let packed = tuya::encrypt_gcm(&key(), &nonce, json);
    assert_eq!(tuya::decrypt_gcm(&key(), &packed).unwrap(), json);
}

#[test]
fn png_encoder_writes_a_signature() {
    let png = encode_png(1, 1, &[255, 0, 0, 255]).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
}
