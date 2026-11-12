import json
from pathlib import Path

root = Path(__file__).resolve().parents[2]
src = root / "bench" / "models" / "kokoro" / "tokenizer.json"
out = root / "kuro-rs" / "crates" / "kuro-tts" / "src" / "kokoro_vocab.rs"
vocab = json.loads(src.read_text(encoding="utf-8"))["model"]["vocab"]
lines = [
    "// Auto-generated from bench/models/kokoro/tokenizer.json — do not edit by hand.",
    "pub fn kokoro_char_id(ch: char) -> Option<i64> {",
    "    match ch {",
]
for k, v in sorted(vocab.items(), key=lambda x: x[1]):
    if len(k) != 1:
        continue
    ch = k
    if ch == "'":
        lit = r"'\''"
    elif ch == "\\":
        lit = r"'\\'"
    elif ch == "\n":
        lit = r"'\n'"
    elif ch == "\r":
        lit = r"'\r'"
    elif ch == "\t":
        lit = r"'\t'"
    else:
        lit = "'" + ch + "'"
    lines.append(f"        {lit} => Some({v}),")
lines += ["        _ => None,", "    }", "}", ""]
out.write_text("\n".join(lines), encoding="utf-8")
print("wrote", out, "entries", len(vocab))
