namespace KuroShell.Core;

/// Copies the running shell build into %LocalAppData%\Kuro\shell for Start Menu and restart.
public static class ShellPublish
{
    public static string PublishDirectory()
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return Path.Combine(root, "Kuro", "shell");
    }

    public static string PublishedExePath() => Path.Combine(PublishDirectory(), "KuroShell.exe");

    public static string? PreferredExePath()
    {
        var published = PublishedExePath();
        if (File.Exists(published))
        {
            return published;
        }

        var running = Environment.ProcessPath;
        return string.IsNullOrWhiteSpace(running) ? null : running;
    }

    public static bool TryPublishFrom(string sourceDirectory)
    {
        if (string.IsNullOrWhiteSpace(sourceDirectory) || !Directory.Exists(sourceDirectory))
        {
            return false;
        }

        var dest = PublishDirectory();
        var sourceFull = Path.GetFullPath(sourceDirectory).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        var destFull = Path.GetFullPath(dest).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        if (string.Equals(sourceFull, destFull, StringComparison.OrdinalIgnoreCase))
        {
            return File.Exists(PublishedExePath());
        }

        Directory.CreateDirectory(dest);
        foreach (var file in Directory.EnumerateFiles(sourceDirectory, "*", SearchOption.AllDirectories))
        {
            var relative = Path.GetRelativePath(sourceDirectory, file);
            var target = Path.Combine(dest, relative);
            var folder = Path.GetDirectoryName(target);
            if (!string.IsNullOrEmpty(folder))
            {
                Directory.CreateDirectory(folder);
            }

            try
            {
                File.Copy(file, target, overwrite: true);
            }
            catch (IOException)
            {
                // Destination exe may be locked if an older shell is still running.
            }
            catch (UnauthorizedAccessException)
            {
            }
        }

        return File.Exists(PublishedExePath());
    }
}
