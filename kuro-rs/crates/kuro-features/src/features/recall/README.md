# recall

First slice of conversation recall (phase 20). Keeps an in-memory `TurnLog` inside the
feature until the turn machine wires Store-backed persistence. Intents `recall.repeat`
and `recall.last_question` are stubs with spoken outcomes only; router corpus is unchanged.