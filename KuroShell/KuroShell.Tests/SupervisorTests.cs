using KuroShell.Core.Supervisor;
using Xunit;

namespace KuroShell.Tests;

/// <summary>
/// ADR 0010: single supervisor restart policy (pure; no process actuation).
/// </summary>
public class SupervisorTests
{
    [Fact]
    public void Backoff_follows_adr_schedule()
    {
        Assert.Equal(0, RestartPolicy.BackoffDelayMs(0));
        Assert.Equal(1000, RestartPolicy.BackoffDelayMs(1));
        Assert.Equal(2000, RestartPolicy.BackoffDelayMs(2));
        Assert.Equal(5000, RestartPolicy.BackoffDelayMs(3));
        Assert.Equal(15_000, RestartPolicy.BackoffDelayMs(4));
        Assert.Equal(60_000, RestartPolicy.BackoffDelayMs(5));
        Assert.Equal(60_000, RestartPolicy.BackoffDelayMs(20));
    }

    [Fact]
    public void Crash_loop_applies_backoff_per_exit()
    {
        var policy = new RestartPolicy();
        var t = 0L;
        var expectedDelays = new[] { 1000, 2000, 5000, 15_000, 60_000 };
        foreach (var delay in expectedDelays)
        {
            var advice = policy.AdviseAfterExit(1, t);
            Assert.True(advice.ShouldRestart);
            Assert.Equal(delay, advice.DelayMs);
            Assert.Equal(EngineExitKind.Crash, advice.ExitKind);
            t += delay;
        }
    }

    [Fact]
    public void Max_restarts_in_window_stops_supervised_restart()
    {
        var policy = new RestartPolicy();
        for (var i = 0; i < RestartPolicy.MaxRestartsInWindow; i++)
        {
            var advice = policy.AdviseAfterExit(EngineExitCodes.WatchdogRestart, i * 1000L);
            Assert.True(advice.ShouldRestart);
            Assert.False(advice.GaveUp);
        }

        var giveUp = policy.AdviseAfterExit(70, 6000);
        Assert.False(giveUp.ShouldRestart);
        Assert.True(giveUp.GaveUp);
    }

    [Fact]
    public void Clean_exit_does_not_restart()
    {
        var policy = new RestartPolicy();
        var advice = policy.AdviseAfterExit(EngineExitCodes.Clean, 0);
        Assert.False(advice.ShouldRestart);
        Assert.Equal(EngineExitKind.CleanExit, advice.ExitKind);

        advice = policy.AdviseAfterExit(EngineExitCodes.InvalidConfig, 0);
        Assert.False(advice.ShouldRestart);
        Assert.Equal(EngineExitKind.InvalidConfig, advice.ExitKind);
    }

    [Fact]
    public void Healthy_run_resets_backoff_sequence()
    {
        var policy = new RestartPolicy();
        policy.AdviseAfterExit(70, 0);
        policy.AdviseAfterExit(70, 1000);
        Assert.Equal(5000, policy.AdviseAfterExit(70, 2000).DelayMs);

        policy.OnEngineStarted(10_000);
        policy.OnEngineStopped(10_000 + RestartPolicy.HealthyRunResetMs);
        Assert.Equal(1000, policy.AdviseAfterExit(70, 400_000).DelayMs);
    }
}