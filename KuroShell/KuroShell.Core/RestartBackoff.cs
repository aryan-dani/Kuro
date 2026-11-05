namespace KuroShell.Core;

public static class RestartBackoff
{
    public static int NextDelayMs(int failures)
    {
        if (failures < 1)
        {
            return 0;
        }

        var shift = Math.Min(failures - 1, 5);
        return Math.Min(1000 << shift, 30_000);
    }
}
