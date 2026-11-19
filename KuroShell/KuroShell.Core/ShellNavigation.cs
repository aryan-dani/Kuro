namespace KuroShell.Core;

public static class ShellNavigation
{
    private static readonly HashSet<string> LegacyPages = new(StringComparer.Ordinal)
    {
        "Conversation",
        "Voice",
        "Devices",
        "Contacts",
        "System",
    };

    private static readonly HashSet<string> RegistryPages = new(StringComparer.Ordinal)
    {
        PageIds.Settings,
        PageIds.Health,
        PageIds.Phrases,
    };

    public static string NormalizePage(string page)
    {
        page = page switch
        {
            "General" or "Logs" or "Performance" or "Workflows" or "HUD" => page == "General" ? "Conversation" : "System",
            _ => page,
        };
        if (!LegacyPages.Contains(page) && !RegistryPages.Contains(page))
        {
            return "Conversation";
        }

        return page;
    }

    public static bool IsRegistryPage(string page) => RegistryPages.Contains(page);
}