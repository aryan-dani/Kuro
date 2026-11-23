namespace KuroShell.Core;

/// Decides whether a process start should show the main shell window.
public static class ShellLaunch
{
    public const string TrayOnlyArg = "--tray-only";

    public static bool ShouldOpenHome(IReadOnlyList<string> args)
    {
        if (args is null || args.Count == 0)
        {
            return true;
        }

        foreach (var arg in args)
        {
            if (string.Equals(arg, TrayOnlyArg, StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }
        }

        return true;
    }
}
