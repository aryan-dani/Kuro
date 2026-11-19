# timers

First slice of the timers feature (phase 23). Parses simple durations, keeps active
timers in memory, and speaks outcomes through `Feature::execute`. `poll_due` checks
`ctx.clock` for unit tests until Scheduler and Announcer ports wire in.

Intents and phrases live in `feature.toml`. Router and engine execution are unchanged
in this slice; register only in `kuro-features`.