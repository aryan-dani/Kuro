namespace KuroShell.Core;

/// Whether the HUD capsule should paint for a given engine state.
public static class HudShowPolicy
{
    public static bool CapsuleVisible(string state, bool showIdleDot)
    {
        if (string.IsNullOrWhiteSpace(state))
        {
            return false;
        }

        return state is "listening" or "thinking" or "processing" or "speaking" or "error" or "result"
            || (showIdleDot && state is "idle" or "hidden");
    }
}
