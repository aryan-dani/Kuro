# morning_brief

Phase 26C first slice: ritual TOML composes `weather`, `calendar`, and optional
`recall` into one spoken string. Fakes live on `FeatureContext` and slots.

## Profile

```toml
[morning_brief]
enabled = false

[weather]
enabled = false

[calendar]
enabled = false
```

Engine wiring should pass `weather_json`, calendar ICS path or `ics_data`, and
`include_recall` when the turn log is available.

## Router (later)

Phrases are declared in `feature.toml` (`morning brief`, `good morning`, etc.).
Do not add corpus cases until the router maps feature intents. Until then, call
`kuro_features` registry with `intent = "morning_brief.run"`.