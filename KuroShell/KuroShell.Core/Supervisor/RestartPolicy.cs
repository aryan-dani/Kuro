namespace KuroShell.Core.Supervisor;

/// <summary>
/// Pure restart policy for the shell supervisor (ADR 0010). Actuation stays in
/// <c>EngineSupervisor</c>; this type only decides whether and when to restart.
/// </summary>
public sealed class RestartPolicy
{
    public const int MaxRestartsInWindow = 5;
    public const long RestartWindowMs = 10 * 60 * 1000;
    public const long HealthyRunResetMs = 5 * 60 * 1000;

    public static readonly int[] BackoffScheduleMs = [1000, 2000, 5000, 15_000, 60_000];

    private readonly List<long> _restartTimestampsMs = [];
    private int _consecutiveRestartableExits;
    private long _lastStartMs;

    public static EngineExitKind ClassifyExit(int exitCode) =>
        exitCode switch
        {
            EngineExitCodes.Clean => EngineExitKind.CleanExit,
            EngineExitCodes.InvalidConfig => EngineExitKind.InvalidConfig,
            EngineExitCodes.WatchdogRestart => EngineExitKind.RestartRequested,
            > 0 => EngineExitKind.Crash,
            _ => EngineExitKind.CleanExit,
        };

    public static int BackoffDelayMs(int consecutiveRestartableExits)
    {
        if (consecutiveRestartableExits < 1)
        {
            return 0;
        }

        var index = Math.Min(consecutiveRestartableExits - 1, BackoffScheduleMs.Length - 1);
        return BackoffScheduleMs[index];
    }

    public void OnEngineStarted(long utcNowMs)
    {
        _lastStartMs = utcNowMs;
    }

    public void OnEngineStopped(long utcNowMs)
    {
        if (_lastStartMs > 0 && utcNowMs - _lastStartMs >= HealthyRunResetMs)
        {
            _consecutiveRestartableExits = 0;
            PruneWindow(utcNowMs);
        }
    }

    public RestartAdvice AdviseAfterExit(int exitCode, long utcNowMs)
    {
        var kind = ClassifyExit(exitCode);
        if (kind is EngineExitKind.CleanExit or EngineExitKind.InvalidConfig)
        {
            return new RestartAdvice(ShouldRestart: false, DelayMs: 0, GaveUp: false, kind);
        }

        PruneWindow(utcNowMs);
        if (_restartTimestampsMs.Count >= MaxRestartsInWindow)
        {
            return new RestartAdvice(ShouldRestart: false, DelayMs: 0, GaveUp: true, kind);
        }

        _consecutiveRestartableExits++;
        var delay = BackoffDelayMs(_consecutiveRestartableExits);
        _restartTimestampsMs.Add(utcNowMs);
        return new RestartAdvice(ShouldRestart: true, DelayMs: delay, GaveUp: false, kind);
    }

    private void PruneWindow(long utcNowMs)
    {
        _restartTimestampsMs.RemoveAll(t => utcNowMs - t > RestartWindowMs);
    }
}

public static class EngineExitCodes
{
    public const int Clean = 0;
    public const int InvalidConfig = 64;
    public const int WatchdogRestart = 70;
}

public enum EngineExitKind
{
    CleanExit,
    InvalidConfig,
    RestartRequested,
    Crash,
}

public readonly record struct RestartAdvice(
    bool ShouldRestart,
    int DelayMs,
    bool GaveUp,
    EngineExitKind ExitKind);