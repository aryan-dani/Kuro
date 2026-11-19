use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEvent {
    pub start: DateTime<Utc>,
    pub summary: String,
}

/// Minimal VEVENT parser for fixture ICS strings in tests.
pub fn parse_events(ics: &str) -> Vec<CalendarEvent> {
    let unfolded = unfold_lines(ics);
    let mut events = Vec::new();
    let mut in_event = false;
    let mut dtstart: Option<DateTime<Utc>> = None;
    let mut summary: Option<String> = None;

    for line in unfolded {
        if line == "BEGIN:VEVENT" {
            in_event = true;
            dtstart = None;
            summary = None;
            continue;
        }
        if line == "END:VEVENT" {
            if in_event && let (Some(start), Some(title)) = (dtstart.take(), summary.take()) {
                events.push(CalendarEvent { start, summary: title });
            }
            in_event = false;
            continue;
        }
        if !in_event {
            continue;
        }
        if let Some(rest) = line.strip_prefix("DTSTART") {
            if let Some(value) = property_value(rest) {
                dtstart = parse_dtstart(value);
            }
        } else if let Some(rest) = line.strip_prefix("SUMMARY")
            && let Some(value) = property_value(rest)
        {
            summary = Some(value.to_string());
        }
    }

    events.sort_by_key(|e| e.start);
    events
}

pub fn next_event_after(events: &[CalendarEvent], now: DateTime<Utc>) -> Option<&CalendarEvent> {
    events.iter().find(|e| e.start > now)
}

pub fn events_on_local_day<'a, Tz: TimeZone>(
    events: &'a [CalendarEvent],
    day: NaiveDate,
    tz: &Tz,
) -> Vec<&'a CalendarEvent>
where
    Tz::Offset: Copy,
{
    events.iter().filter(|e| e.start.with_timezone(tz).date_naive() == day).collect()
}

fn unfold_lines(ics: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in ics.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            current.push_str(line.trim_start());
        } else {
            if !current.is_empty() {
                out.push(current.clone());
            }
            current = line.to_string();
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn property_value(rest: &str) -> Option<&str> {
    let rest = rest.trim();
    let value = if let Some((_, val)) = rest.split_once(':') {
        val
    } else if let Some(stripped) = rest.strip_prefix(':') {
        stripped
    } else {
        rest
    };
    Some(value.trim())
}

fn parse_dtstart(raw: &str) -> Option<DateTime<Utc>> {
    if raw.len() == 8 {
        let date = NaiveDate::parse_from_str(raw, "%Y%m%d").ok()?;
        return Some(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0)?));
    }
    if raw.ends_with('Z') && raw.len() >= 16 {
        let naive = NaiveDateTime::parse_from_str(raw.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()?;
        return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    if raw.len() >= 15 {
        let naive = NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%S").ok()?;
        return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    None
}
