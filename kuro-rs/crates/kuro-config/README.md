# kuro-config

Typed `Settings` loaded in order: defaults, tracked example profile, user profile,
then environment.

## Listener (phase 20)

`listener_enabled` defaults to `false`. Opt in with profile `[listener] enabled = true`
or `KURO_LISTENER=1`. The engine does not open a continuous listener loop until a
later wiring slice; hold-to-talk stays primary.