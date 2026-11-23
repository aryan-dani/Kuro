using System.Diagnostics;

namespace KuroShell.Core;

/// Builds a delayed relaunch so the current process can release single-instance locks first.
public static class AppRestart
{
    public static ProcessStartInfo DelayedRelaunch(string exePath, int delaySeconds = 1)
    {
        if (string.IsNullOrWhiteSpace(exePath))
        {
            throw new ArgumentException("Executable path is required.", nameof(exePath));
        }

        if (delaySeconds < 0)
        {
            delaySeconds = 0;
        }

        // Hidden cmd: wait, then start the shell again with an empty title for paths with spaces.
        return new ProcessStartInfo
        {
            FileName = "cmd.exe",
            Arguments = $"/c timeout /t {delaySeconds} /nobreak >nul & start \"\" \"{exePath}\"",
            CreateNoWindow = true,
            UseShellExecute = false,
            WindowStyle = ProcessWindowStyle.Hidden,
        };
    }
}
