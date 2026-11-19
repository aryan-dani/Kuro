use std::fs;
use std::path::Path;

use crate::paths::repo_root;

pub fn run(title: &str) -> i32 {
    match write_adr(&repo_root(), title) {
        Ok(path) => {
            eprintln!("new-adr: wrote {}", path.display());
            0
        }
        Err(e) => {
            eprintln!("new-adr: {e}");
            1
        }
    }
}

pub fn write_adr(root: &Path, title: &str) -> Result<std::path::PathBuf, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("ADR title must not be empty".to_string());
    }
    let adr_dir = root.join("docs/adr");
    let next = next_adr_number(&adr_dir)?;
    let slug = slugify(title);
    let file_name = format!("{next:04}-{slug}.md");
    let path = adr_dir.join(&file_name);
    if path.exists() {
        return Err(format!("ADR file already exists: {}", path.display()));
    }
    let template = fs::read_to_string(adr_dir.join("0000-template.md")).map_err(|e| e.to_string())?;
    let today = "2026-11-19";
    let body = template.replace("ADR NNNN: Title", &format!("ADR {next:04}: {title}")).replace("YYYY-MM-DD", today);
    fs::write(&path, body).map_err(|e| e.to_string())?;
    patch_adr_index(&adr_dir.join("README.md"), next, &file_name, title)?;
    Ok(path)
}

pub fn next_adr_number(adr_dir: &Path) -> Result<u32, String> {
    let mut max = 0u32;
    let entries = fs::read_dir(adr_dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(digits) = name.get(0..4)
            && digits.chars().all(|c| c.is_ascii_digit())
            && let Ok(n) = digits.parse::<u32>()
        {
            max = max.max(n);
        }
    }
    Ok(max + 1)
}

fn slugify(title: &str) -> String {
    let lower: String = title.chars().map(|c| c.to_ascii_lowercase()).collect();
    let mut out = String::new();
    let mut prev_hyphen = false;
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen {
            out.push('-');
            prev_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "untitled".to_string() } else { out }
}

fn patch_adr_index(readme: &Path, number: u32, file_name: &str, title: &str) -> Result<(), String> {
    let content = fs::read_to_string(readme).map_err(|e| e.to_string())?;
    let row = format!("| [{number:04}]({file_name}) | {title} | Proposed |");
    if content.contains(&format!("[{number:04}]")) {
        return Ok(());
    }
    let marker = "\n\nTemplate: [0000-template.md](0000-template.md).";
    let Some(idx) = content.find(marker) else {
        return Err("docs/adr/README.md missing template marker".to_string());
    };
    let mut out = String::new();
    out.push_str(&content[..idx]);
    out.push('\n');
    out.push_str(&row);
    out.push_str(&content[idx..]);
    fs::write(readme, out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_number_from_temp_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("0001-foo.md"), "#").unwrap();
        fs::write(dir.path().join("0019-bar.md"), "#").unwrap();
        assert_eq!(next_adr_number(dir.path()).unwrap(), 20);
    }

    #[test]
    fn slugify_title() {
        assert_eq!(slugify("Crate Layering"), "crate-layering");
    }
}
