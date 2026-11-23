namespace KuroShell.Core;

/// Built-in speakable phrases shown in the Phrases page.
public static class PhraseCatalog
{
    public static IReadOnlyList<string> All { get; } =
    [
        "Help",
        "What can you do",
        "Desk on",
        "Desk off",
        "Good night",
        "Wake up daddy's home",
        "Pause the song",
        "Resume the song",
        "Stop the music",
        "What's the news",
        "Any urgent mail",
        "Make a Google Meet",
        "Send a Google Meet link to Mom",
        "Join the meet",
        "Join that meet link",
        "Open the meet you sent",
        "Send this link on WhatsApp",
        "What time is it",
        "What's today's date",
        "Cancel",
        "Thanks",
        "Who are you",
    ];

    public static IReadOnlyList<string> Filter(string query)
    {
        var q = (query ?? "").Trim();
        if (q.Length == 0)
        {
            return All;
        }

        return All
            .Where(p => p.Contains(q, StringComparison.OrdinalIgnoreCase))
            .ToArray();
    }
}
