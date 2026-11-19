# calendar

First slice of calendar read (phase 24). Parses minimal ICS `VEVENT` data from
`slots.ics_data` or a path in `slots.ics_path` / `FeatureContext::calendar_ics_path`.
Without a source, speaks that calendar is not configured. Router wiring lands later.