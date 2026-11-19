using System.Text.Json.Serialization;

namespace KuroShell.Core.Protocol;

public sealed class HandshakeRequest
{
    [JsonPropertyName("protocol")]
    public uint Protocol { get; init; }

    [JsonPropertyName("engine")]
    public string Engine { get; init; } = "";
}