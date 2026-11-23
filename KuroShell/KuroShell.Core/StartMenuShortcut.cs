namespace KuroShell.Core;

/// Start Menu shortcut path helpers (install lives in the WinUI shell).
public static class StartMenuShortcut
{
    public static string DefaultShortcutPath()
    {
        var programs = Environment.GetFolderPath(Environment.SpecialFolder.Programs);
        return Path.Combine(programs, "Kuro.lnk");
    }

    /// Prefer the published LocalAppData shell so Start Menu does not stick to a stale build output.
    public static string PreferredTargetExe()
    {
        return ShellPublish.PreferredExePath() ?? "KuroShell.exe";
    }
}
