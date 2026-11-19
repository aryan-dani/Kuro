using System.Text.Json.Serialization;

namespace KuroShell.Core.Protocol;

public sealed class HandshakeResponse
{
    [JsonPropertyName("protocol")]
    public uint Protocol { get; init; }

    [JsonPropertyName("shell")]
    public string Shell { get; init; } = "";

    [JsonPropertyName("accepted")]
    public bool Accepted { get; init; }
}