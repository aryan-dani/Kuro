using System.Text.Json;

namespace KuroShell.Core;

/// Builds /say payloads and marks phrases the shell must not execute.
public static class ShellSay
{
    public static string ExecuteBody(string text) =>
        JsonSerializer.Serialize(new Dictionary<string, string>
        {
            ["text"] = text.Trim(),
            ["mode"] = "execute",
        });

    public static string DryRunBody(string text) =>
        JsonSerializer.Serialize(new Dictionary<string, string>
        {
            ["text"] = text.Trim(),
            ["mode"] = "dry_run",
        });

    /// Meet, WhatsApp, and n8n workflows stay voice-only (ADR 0019 /say scope).
    public static bool IsVoiceOnly(string text)
    {
        var t = (text ?? "").Trim().ToLowerInvariant();
        if (t.Length == 0)
        {
            return false;
        }

        if (t.Contains("meet", StringComparison.Ordinal)
            || t.Contains("mate", StringComparison.Ordinal)
            || t.Contains("whatsapp", StringComparison.Ordinal)
            || t.Contains("workflow", StringComparison.Ordinal))
        {
            return true;
        }

        if (t.StartsWith("send ", StringComparison.Ordinal)
            && (t.Contains(" link", StringComparison.Ordinal) || t.Contains(" message", StringComparison.Ordinal)))
        {
            return true;
        }

        return t.StartsWith("run ", StringComparison.Ordinal) && t.Contains("workflow", StringComparison.Ordinal);
    }

    public static string VoiceOnlyMessage => "That one is voice only. Hold Ctrl+Alt+Space.";
}
