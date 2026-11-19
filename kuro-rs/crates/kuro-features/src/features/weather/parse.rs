use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct WeatherSnapshot {
    pub celsius: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeatherParseError {
    pub message: String,
}

impl std::fmt::Display for WeatherParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WeatherParseError {}

/// Open-Meteo style `current_weather.temperature` or a top-level `temperature` field.
pub fn parse_weather_json(body: &str) -> Result<WeatherSnapshot, WeatherParseError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| WeatherParseError { message: format!("invalid weather json: {err}") })?;
    let temp = value
        .get("current_weather")
        .and_then(|cw| cw.get("temperature"))
        .or_else(|| value.get("temperature"))
        .and_then(|t| t.as_f64())
        .ok_or_else(|| WeatherParseError { message: "weather json missing temperature".to_string() })?;
    Ok(WeatherSnapshot { celsius: temp })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_weather_block() {
        let snap = parse_weather_json(r#"{"current_weather":{"temperature":21.5}}"#).expect("parse");
        assert_eq!(snap.celsius, 21.5);
    }
}
