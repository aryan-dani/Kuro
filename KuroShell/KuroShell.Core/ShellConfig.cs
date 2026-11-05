namespace KuroShell.Core;

public static class ShellConfig
{
    public static readonly string[] Pages =
    [
        "General",
        "Voice",
        "Devices",
        "Workflows",
        "Contacts",
        "Logs",
        "Performance",
        "HUD",
    ];

    private static readonly HashSet<string> Allowed = new(StringComparer.Ordinal)
    {
        "llm_mode",
        "stt_backend",
        "tts_engine",
        "xtts_language",
        "paused",
        "hud_show_text",
    };

    public static bool IsAllowed(string key) => Allowed.Contains(key);

    public static string MaskNumber(string raw)
    {
        var digits = new string((raw ?? "").Where(char.IsDigit).ToArray());
        if (digits.Length < 4)
        {
            return "masked";
        }

        return new string('x', digits.Length - 2) + digits[^2..];
    }

    public static string MaskId(string raw)
    {
        var text = (raw ?? "").Trim();
        if (text.Length == 0)
        {
            return "";
        }

        if (text.Length <= 4)
        {
            return "xxxx";
        }

        return text[..2] + new string('x', text.Length - 4) + text[^2..];
    }
}
