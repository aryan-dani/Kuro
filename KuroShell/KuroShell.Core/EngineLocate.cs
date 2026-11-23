namespace KuroShell.Core;

/// Picks which kuro-engine.exe to launch when several builds exist.
public static class EngineLocate
{
    public static string LocalEnginePath(string localAppData) =>
        Path.Combine(localAppData, "Kuro", "kuro-engine.exe");

    public static string PendingEnginePath(string localAppData) =>
        Path.Combine(localAppData, "Kuro", "kuro-engine.next.exe");

    /// Applies a staged next binary over the LocalAppData engine when the file is unlocked.
    public static bool TryApplyPending(string localAppData)
    {
        var pending = PendingEnginePath(localAppData);
        var current = LocalEnginePath(localAppData);
        if (!File.Exists(pending))
        {
            return false;
        }

        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(current)!);
            File.Copy(pending, current, overwrite: true);
            File.Delete(pending);
            return true;
        }
        catch (IOException)
        {
            return false;
        }
        catch (UnauthorizedAccessException)
        {
            return false;
        }
    }

    public static string? PreferNewest(IEnumerable<string> candidates)
    {
        string? best = null;
        var bestWrite = DateTime.MinValue;
        foreach (var path in candidates)
        {
            if (string.IsNullOrWhiteSpace(path) || !File.Exists(path))
            {
                continue;
            }

            DateTime write;
            try
            {
                write = File.GetLastWriteTimeUtc(path);
            }
            catch (IOException)
            {
                continue;
            }
            catch (UnauthorizedAccessException)
            {
                continue;
            }

            if (best is null || write > bestWrite)
            {
                best = path;
                bestWrite = write;
            }
        }

        return best;
    }

    public static IEnumerable<string> CandidatePaths(string? repoRoot, string localAppData)
    {
        if (!string.IsNullOrWhiteSpace(repoRoot))
        {
            yield return Path.Combine(repoRoot, "kuro-rs", "target", "release", "kuro-engine.exe");
        }

        yield return PendingEnginePath(localAppData);
        yield return LocalEnginePath(localAppData);

        if (!string.IsNullOrWhiteSpace(repoRoot))
        {
            yield return Path.Combine(repoRoot, "kuro-rs", "target", "debug", "kuro-engine.exe");
        }
    }
}
