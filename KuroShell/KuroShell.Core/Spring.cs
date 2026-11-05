namespace KuroShell.Core;

public sealed class Spring
{
    public double Stiffness { get; init; } = 180;
    public double Damping { get; init; } = 22;

    private double _value;
    private double _velocity;

    public double Value => _value;

    public void Snap(double target)
    {
        _value = target;
        _velocity = 0;
    }

    public double Step(double target, double dtSeconds, bool animate)
    {
        if (!animate)
        {
            Snap(target);
            return _value;
        }

        var dt = Math.Clamp(dtSeconds, 0, 0.05);
        _velocity += (target - _value) * Stiffness * dt;
        _velocity *= Math.Exp(-Damping * dt);
        _value += _velocity * dt;
        var overshoot = Math.Abs(_value - target);
        var span = Math.Max(Math.Abs(target), 1);
        if (overshoot > span * 0.04 && Math.Sign(_value - target) != 0)
        {
            _value = target + Math.CopySign(span * 0.04, _value - target);
            _velocity *= 0.2;
        }

        return _value;
    }
}
