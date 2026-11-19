namespace KuroShell.Core.Protocol;

/// <summary>Wire protocol integer (ADR 0009). Today the full value is the major.</summary>
public static class ProtocolVersion
{
    public const uint Current = 1;

    public static bool CompatibleWith(uint peer) => peer == Current;
}