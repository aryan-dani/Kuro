namespace KuroShell.Core;

public sealed class LevelSmoother
{
    public double AttackMs { get; init; } = 40;
    public double ReleaseMs { get; init; } = 180;

    private double _value;

    public double Value => _value;

    public double Push(double sample, double dtMs)
    {
        var target = Math.Clamp(sample, 0, 1);
        var tau = target > _value ? AttackMs : ReleaseMs;
        var step = 1 - Math.Exp(-Math.Max(dtMs, 0) / Math.Max(tau, 1));
        _value += (target - _value) * step;
        return _value;
    }
}
