//! Neonize protobuf helpers. Field numbers match `JID` and `SendMessageReturnFunction`.

pub(crate) fn encode_jid(user: &str, server: &str) -> Vec<u8> {
    let mut out = Vec::new();
    pb_string(1, user, &mut out);
    pb_varint_field(2, 0, &mut out);
    pb_varint_field(3, 0, &mut out);
    pb_varint_field(4, 0, &mut out);
    pb_string(5, server, &mut out);
    out
}

pub(crate) fn encode_text_message(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    pb_string(1, text, &mut out);
    out
}

/// Field 1 on `SendMessageReturnFunction` is the error string. Any non-empty value is a failure.
pub(crate) fn proto_error(bytes: &[u8]) -> Option<String> {
    let text = proto_string_field(bytes, 1)?;
    let trimmed = text.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

/// `GetLIDFromPN` returns a JID in field 2. Only a `lid` server is a different send target.
pub(crate) fn lid_user(bytes: &[u8]) -> Option<String> {
    if proto_error(bytes).is_some() {
        return None;
    }
    let jid = proto_bytes_field(bytes, 2)?;
    let server = proto_string_field(&jid, 5).unwrap_or_default();
    if server != "lid" {
        return None;
    }
    let user = proto_string_field(&jid, 1)?;
    if user.is_empty() { None } else { Some(user) }
}

fn proto_string_field(bytes: &[u8], want: u64) -> Option<String> {
    let raw = proto_bytes_field(bytes, want)?;
    Some(String::from_utf8_lossy(&raw).into_owned())
}

fn proto_bytes_field(bytes: &[u8], want: u64) -> Option<Vec<u8>> {
    let mut i = 0;
    while i < bytes.len() {
        let (tag, n) = read_varint(&bytes[i..])?;
        i += n;
        let field = tag >> 3;
        let wire = tag & 7;
        match wire {
            0 => {
                let (_, n) = read_varint(&bytes[i..])?;
                i += n;
            }
            2 => {
                let (len, n) = read_varint(&bytes[i..])?;
                i += n;
                let len = usize::try_from(len).ok()?;
                let end = i.checked_add(len)?;
                if end > bytes.len() {
                    return None;
                }
                if field == want {
                    return Some(bytes[i..end].to_vec());
                }
                i = end;
            }
            _ => return None,
        }
    }
    None
}

fn read_varint(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0u64;
    let mut shift = 0;
    for (i, byte) in bytes.iter().enumerate() {
        if i > 9 {
            return None;
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
    }
    None
}

fn pb_varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn pb_string(field: u32, s: &str, out: &mut Vec<u8>) {
    pb_varint(u64::from((field << 3) | 2), out);
    pb_varint(s.len() as u64, out);
    out.extend_from_slice(s.as_bytes());
}

fn pb_varint_field(field: u32, v: u64, out: &mut Vec<u8>) {
    pb_varint(u64::from(field << 3), out);
    pb_varint(v, out);
}

/// Hide phone-number-like digit runs (5 or more) before a reason goes into the log.
pub fn redact_digits(text: &str) -> String {
    fn flush(out: &mut String, run: &mut String) {
        if run.len() >= 5 {
            out.push('#');
        } else {
            out.push_str(run);
        }
        run.clear();
    }
    let mut out = String::new();
    let mut run = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            run.push(ch);
        } else {
            flush(&mut out, &mut run);
            out.push(ch);
        }
    }
    flush(&mut out, &mut run);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_hides_numbers_but_keeps_short_codes() {
        assert_eq!(redact_digits("jid 15551234567 rejected"), "jid # rejected");
        assert_eq!(redact_digits("status 401"), "status 401");
    }

    #[test]
    fn any_error_string_is_a_failure() {
        let mut bytes = Vec::new();
        pb_string(1, "not connected", &mut bytes);
        assert_eq!(proto_error(&bytes).as_deref(), Some("not connected"));
        assert!(proto_error(&[]).is_none());
        let mut ok = Vec::new();
        pb_string(2, "ack", &mut ok);
        assert!(proto_error(&ok).is_none());
    }

    #[test]
    fn lid_user_requires_lid_server() {
        let mut jid = encode_jid("abc", "lid");
        let mut wrapped = Vec::new();
        pb_varint(u64::from((2u32 << 3) | 2), &mut wrapped);
        pb_varint(jid.len() as u64, &mut wrapped);
        wrapped.append(&mut jid);
        assert_eq!(lid_user(&wrapped).as_deref(), Some("abc"));
        assert!(lid_user(&encode_jid("15550001111", "s.whatsapp.net")).is_none());
    }
}
