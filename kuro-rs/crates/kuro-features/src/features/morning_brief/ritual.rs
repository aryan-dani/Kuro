use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RitualStep {
    pub feature: String,
    pub intent: String,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RitualFile {
    #[serde(default)]
    pub steps: Vec<RitualStep>,
}

pub fn parse_ritual(text: &str) -> Result<RitualFile, String> {
    toml::from_str(text).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[[steps]]
feature = "weather"
intent = "weather.now"

[[steps]]
feature = "recall"
intent = "recall.last_question"
optional = true
"#;

    #[test]
    fn parses_steps() {
        let file = parse_ritual(SAMPLE).expect("parse");
        assert_eq!(file.steps.len(), 2);
        assert!(file.steps[1].optional);
    }
}
