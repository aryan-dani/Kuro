using System.Text.Json.Serialization;

namespace KuroShell;

public sealed class EventFrame
{
    public string? State { get; set; }
    public string? Label { get; set; }
    public double[]? Levels { get; set; }
    public bool Paused { get; set; }
    public string? You { get; set; }
    public string? Kuro { get; set; }
}

public sealed class StatusDto
{
    public bool Ok { get; set; }
    public bool Paused { get; set; }
    public string? State { get; set; }
    public bool Api { get; set; }
    public string? Logs { get; set; }

    [JsonPropertyName("router_mode")]
    public string? RouterMode { get; set; }

    [JsonPropertyName("tts_active")]
    public string? TtsActive { get; set; }
}

public sealed class ConfigDto
{
    [JsonPropertyName("llm_mode")]
    public string? LlmMode { get; set; }

    [JsonPropertyName("stt_backend")]
    public string? SttBackend { get; set; }

    [JsonPropertyName("tts_engine")]
    public string? TtsEngine { get; set; }

    [JsonPropertyName("xtts_language")]
    public string? XttsLanguage { get; set; }

    public bool Paused { get; set; }

    [JsonPropertyName("hud_show_text")]
    public bool HudShowText { get; set; }

    [JsonPropertyName("plug_id_masked")]
    public string? PlugIdMasked { get; set; }

    [JsonPropertyName("router_mode")]
    public string? RouterMode { get; set; }

    [JsonPropertyName("tts_active")]
    public string? TtsActive { get; set; }
}

public sealed class TimingRow
{
    public string? Kind { get; set; }
    public string? Stage { get; set; }
    public string? Ms { get; set; }
}

public sealed class TimingDto
{
    public TimingRow[]? Rows { get; set; }
}

public sealed class WorkflowMap
{
    public Dictionary<string, System.Text.Json.JsonElement>? Workflows { get; set; }
}

public sealed class ContactMap
{
    public Dictionary<string, string>? Contacts { get; set; }
}

[JsonSourceGenerationOptions(PropertyNameCaseInsensitive = true)]
[JsonSerializable(typeof(EventFrame))]
[JsonSerializable(typeof(StatusDto))]
[JsonSerializable(typeof(ConfigDto))]
[JsonSerializable(typeof(TimingDto))]
[JsonSerializable(typeof(WorkflowMap))]
[JsonSerializable(typeof(ContactMap))]
public partial class EngineJsonContext : JsonSerializerContext;
