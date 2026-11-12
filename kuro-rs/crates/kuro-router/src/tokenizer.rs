//! Pure-Rust BERT WordPiece for bge-small-en-v1.5 (no onig / tokenizers crate).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

const CLS: i64 = 101;
const SEP: i64 = 102;
const UNK: i64 = 100;
pub const MAX_SEQ: usize = 64;

pub struct WordPiece {
    vocab: HashMap<String, i64>,
}

impl WordPiece {
    pub fn from_vocab_txt(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut vocab = HashMap::new();
        for (i, line) in text.lines().enumerate() {
            let tok = line.trim();
            if !tok.is_empty() {
                vocab.insert(tok.to_string(), i as i64);
            }
        }
        if vocab.is_empty() {
            return Err("empty vocab.txt".into());
        }
        Ok(Self { vocab })
    }

    pub fn from_tokenizer_json(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let map = value
            .pointer("/model/vocab")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "tokenizer.json missing model.vocab".to_string())?;
        let mut vocab = HashMap::new();
        for (k, v) in map {
            if let Some(id) = v.as_i64() {
                vocab.insert(k.clone(), id);
            }
        }
        Ok(Self { vocab })
    }

    pub fn encode(&self, text: &str) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
        let mut ids = vec![CLS];
        for word in basic_tokenize(text) {
            for piece in self.wordpiece(&word) {
                ids.push(piece);
                if ids.len() >= MAX_SEQ - 1 {
                    break;
                }
            }
            if ids.len() >= MAX_SEQ - 1 {
                break;
            }
        }
        ids.push(SEP);
        ids.truncate(MAX_SEQ);
        let seq_len = ids.len();
        let mask = vec![1i64; seq_len];
        let types = vec![0i64; seq_len];
        (ids, mask, types)
    }

    fn wordpiece(&self, word: &str) -> Vec<i64> {
        if word.is_empty() {
            return Vec::new();
        }
        if let Some(&id) = self.vocab.get(word) {
            return vec![id];
        }
        let chars: Vec<char> = word.chars().collect();
        let mut start = 0usize;
        let mut out = Vec::new();
        while start < chars.len() {
            let mut end = chars.len();
            let mut cur = None;
            while start < end {
                let mut substr: String = chars[start..end].iter().collect();
                if start > 0 {
                    substr = format!("##{substr}");
                }
                if let Some(&id) = self.vocab.get(&substr) {
                    cur = Some(id);
                    break;
                }
                end -= 1;
            }
            if cur.is_none() {
                out.push(self.vocab.get("[UNK]").copied().unwrap_or(UNK));
                break;
            }
            out.push(cur.unwrap());
            start = end;
        }
        out
    }
}

fn basic_tokenize(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut cleaned = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            cleaned.push(ch);
        } else if !cleaned.is_empty() && !cleaned.ends_with(' ') {
            cleaned.push(' ');
        }
    }
    cleaned.split_whitespace().map(|s| s.to_string()).collect()
}

static TOKENIZER: Mutex<Option<WordPiece>> = Mutex::new(None);

pub fn ensure_tokenizer() -> Option<()> {
    let mut slot = TOKENIZER.lock().ok()?;
    if slot.is_some() {
        return Some(());
    }
    let path = kuro_core::router_embed_vocab()?;
    let wp = if path.extension().and_then(|e| e.to_str()) == Some("json") {
        WordPiece::from_tokenizer_json(&path).ok()?
    } else {
        WordPiece::from_vocab_txt(&path).ok()?
    };
    *slot = Some(wp);
    Some(())
}

pub fn encode(text: &str) -> Option<(Vec<i64>, Vec<i64>, Vec<i64>)> {
    ensure_tokenizer()?;
    let slot = TOKENIZER.lock().ok()?;
    Some(slot.as_ref()?.encode(text))
}

pub fn unload() {
    if let Ok(mut slot) = TOKENIZER.lock() {
        *slot = None;
    }
}
