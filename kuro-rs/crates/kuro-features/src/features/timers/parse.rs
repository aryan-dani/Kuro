use chrono::Duration;

/// Parses a simple duration phrase: `N minutes`, `N seconds`, or `N hours`.
pub fn parse_duration(text: &str) -> Option<Duration> {
    let trimmed = text.trim().to_ascii_lowercase();
    let mut parts = trimmed.split_whitespace();
    let amount: i64 = parts.next()?.parse().ok()?;
    if amount <= 0 {
        return None;
    }
    let unit = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let secs = match unit {
        "second" | "seconds" | "sec" | "secs" => amount,
        "minute" | "minutes" | "min" | "mins" => amount * 60,
        "hour" | "hours" | "hr" | "hrs" => amount * 3600,
        _ => return None,
    };
    Some(Duration::seconds(secs))
}

pub fn format_duration(d: Duration) -> String {
    let secs = d.num_seconds();
    if secs % 3600 == 0 && secs >= 3600 {
        let h = secs / 3600;
        let word = if h == 1 { "hour" } else { "hours" };
        return format!("{h} {word}");
    }
    if secs % 60 == 0 && secs >= 60 {
        let m = secs / 60;
        let word = if m == 1 { "minute" } else { "minutes" };
        return format!("{m} {word}");
    }
    let word = if secs == 1 { "second" } else { "seconds" };
    format!("{secs} {word}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_units() {
        assert_eq!(parse_duration("5 minutes"), Some(Duration::minutes(5)));
        assert_eq!(parse_duration("1 hour"), Some(Duration::hours(1)));
        assert_eq!(parse_duration("30 seconds"), Some(Duration::seconds(30)));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_duration("soon"), None);
        assert_eq!(parse_duration("0 minutes"), None);
    }
}
