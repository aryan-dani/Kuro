use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FeatureManifest {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_tier")]
    pub tier: String,
    #[serde(default = "default_true")]
    pub default_enabled: bool,
    #[serde(default)]
    pub priority: u32,
    #[serde(default)]
    pub llm_description: String,
    #[serde(default)]
    pub replies: BTreeMap<String, String>,
}

fn default_tier() -> String {
    "read_only".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError {
    pub message: String,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ManifestError {}

pub fn parse_manifest(text: &str) -> Result<FeatureManifest, ManifestError> {
    toml::from_str(text).map_err(|err| ManifestError { message: err.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_id() {
        let err = parse_manifest("title = \"no id\"\n").expect_err("missing id");
        assert!(err.message.contains("id"));
    }

    #[test]
    fn parses_minimal_manifest() {
        let manifest = parse_manifest("id = \"ping\"\ntitle = \"Ping\"\n").expect("parse");
        assert_eq!(manifest.id, "ping");
        assert_eq!(manifest.title, "Ping");
        assert_eq!(manifest.tier, "read_only");
    }
}
