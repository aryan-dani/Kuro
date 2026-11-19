using System.Text.Json;

namespace KuroShell.Core.Protocol;

/// <summary>Parses the engine HELLO JSON on pipe/WebSocket connect (ADR 0009).</summary>
public static class EngineHello
{
    public static HandshakeRequest ParseRequired(string json)
    {
        var hello = JsonSerializer.Deserialize<HandshakeRequest>(json)
            ?? throw new InvalidOperationException("Engine hello was not valid JSON.");
        if (!ProtocolVersion.CompatibleWith(hello.Protocol))
        {
            throw new InvalidOperationException(
                $"Engine protocol {hello.Protocol} is not compatible with shell {ProtocolVersion.Current}.");
        }

        return hello;
    }
}