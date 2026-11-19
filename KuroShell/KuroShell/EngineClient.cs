using System.IO.Pipes;
using System.Net;
using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using KuroShell.Core;
using KuroShell.Core.Protocol;

namespace KuroShell;

public sealed class EngineClient : IDisposable
{
    private readonly HttpClient _http = new() { Timeout = TimeSpan.FromSeconds(4) };
    public const string PipeName = "KuroEngine";

    private string _token;
    private readonly string? _tokenPath;

    public EngineClient(string token, int port = 8765, string? tokenPath = null)
    {
        _token = token;
        _tokenPath = tokenPath;
        Port = port;
    }

    public string Token => _token;
    public int Port { get; }

    public Uri Base => new($"http://127.0.0.1:{Port}");

    private bool TryRereadToken(int priorAttempts, int? httpStatus = 401, bool pipeDenied = false)
    {
        if (_tokenPath is null || !ApiAuthRetry.ShouldRereadTokenOnce(priorAttempts, httpStatus, pipeDenied))
        {
            return false;
        }

        if (!File.Exists(_tokenPath))
        {
            return false;
        }

        _token = File.ReadAllText(_tokenPath, Encoding.UTF8).Trim().TrimStart('\uFEFF');
        return true;
    }

    public async Task<JsonDocument?> GetAsync(string path, CancellationToken cancel = default)
    {
        for (var attempt = 0; attempt < 2; attempt++)
        {
            using var request = new HttpRequestMessage(HttpMethod.Get, new Uri(Base, path));
            request.Headers.TryAddWithoutValidation("X-Kuro-Token", _token);
            using var response = await _http.SendAsync(request, cancel);
            if (response.StatusCode == HttpStatusCode.Unauthorized && TryRereadToken(attempt))
            {
                continue;
            }

            if (!response.IsSuccessStatusCode)
            {
                return null;
            }

            var text = await response.Content.ReadAsStringAsync(cancel);
            return JsonDocument.Parse(text);
        }

        return null;
    }

    /// <summary>
    /// Negotiates protocol version with the engine. Returns null when the route is missing (older engine).
    /// </summary>
    public async Task<bool?> HandshakeAsync(CancellationToken cancel = default)
    {
        var payload = JsonSerializer.Serialize(new HandshakeRequest
        {
            Protocol = ProtocolVersion.Current,
            Engine = ProtocolIds.ShellClient,
        });

        for (var attempt = 0; attempt < 2; attempt++)
        {
            using var request = new HttpRequestMessage(HttpMethod.Post, new Uri(Base, "/handshake"));
            request.Headers.TryAddWithoutValidation("X-Kuro-Token", _token);
            request.Content = new StringContent(payload, Encoding.UTF8, "application/json");
            using var response = await _http.SendAsync(request, cancel);
            if (response.StatusCode == HttpStatusCode.Unauthorized && TryRereadToken(attempt))
            {
                continue;
            }

            if (response.StatusCode == HttpStatusCode.NotFound)
            {
                return null;
            }

            if (!response.IsSuccessStatusCode)
            {
                return null;
            }

            var text = await response.Content.ReadAsStringAsync(cancel);
            var parsed = JsonSerializer.Deserialize<HandshakeResponse>(text);
            return parsed?.Accepted ?? false;
        }

        return null;
    }

    public async Task<bool> SendAsync(HttpMethod method, string path, string json, CancellationToken cancel = default)
    {
        for (var attempt = 0; attempt < 2; attempt++)
        {
            using var request = new HttpRequestMessage(method, new Uri(Base, path));
            request.Headers.TryAddWithoutValidation("X-Kuro-Token", _token);
            request.Content = new StringContent(json, Encoding.UTF8, "application/json");
            using var response = await _http.SendAsync(request, cancel);
            if (response.StatusCode == HttpStatusCode.Unauthorized && TryRereadToken(attempt))
            {
                continue;
            }

            return response.IsSuccessStatusCode;
        }

        return false;
    }

    public async Task ListenAsync(Action<string> onFrame, CancellationToken cancel)
    {
        try
        {
            if (await ListenPipeAsync(onFrame, priorAttempts: 0, cancel))
            {
                return;
            }
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch
        {
            // Named pipe unavailable; fall through to WebSocket loopback.
        }

        await ListenWebSocketAsync(onFrame, cancel);
    }

    private async Task<bool> ListenPipeAsync(Action<string> onFrame, int priorAttempts, CancellationToken cancel)
    {
        using var pipe = new NamedPipeClientStream(".", PipeName, PipeDirection.InOut, PipeOptions.Asynchronous);
        await pipe.ConnectAsync(750, cancel);
        var auth = Encoding.UTF8.GetBytes($"AUTH {_token}\n");
        await pipe.WriteAsync(auth, cancel);
        await pipe.FlushAsync(cancel);

        using var reader = new StreamReader(pipe, Encoding.UTF8, detectEncodingFromByteOrderMarks: false, bufferSize: 1024, leaveOpen: true);
        var ack = await reader.ReadLineAsync(cancel);
        if (string.Equals(ack, "DENY", StringComparison.Ordinal)
            && TryRereadToken(priorAttempts, httpStatus: null, pipeDenied: true))
        {
            return await ListenPipeAsync(onFrame, priorAttempts: 1, cancel);
        }

        if (!string.Equals(ack, "OK", StringComparison.Ordinal))
        {
            throw new InvalidOperationException("Named pipe auth failed.");
        }

        var helloLine = await reader.ReadLineAsync(cancel);
        if (helloLine is null)
        {
            throw new InvalidOperationException("Engine closed the pipe before hello.");
        }

        EngineHello.ParseRequired(helloLine);

        while (!cancel.IsCancellationRequested)
        {
            var line = await reader.ReadLineAsync(cancel);
            if (line is null)
            {
                return true;
            }

            if (line.Length > 0)
            {
                onFrame(line);
            }
        }

        return true;
    }

    private async Task ListenWebSocketAsync(Action<string> onFrame, CancellationToken cancel)
    {
        using var socket = new ClientWebSocket();
        socket.Options.SetRequestHeader("X-Kuro-Token", _token);
        var uri = new Uri($"ws://127.0.0.1:{Port}/events");
        await socket.ConnectAsync(uri, cancel);
        var buffer = new byte[8192];
        var hello = await ReadWebSocketTextAsync(socket, buffer, cancel);
        EngineHello.ParseRequired(hello);
        while (!cancel.IsCancellationRequested && socket.State == WebSocketState.Open)
        {
            onFrame(await ReadWebSocketTextAsync(socket, buffer, cancel));
        }
    }

    private static async Task<string> ReadWebSocketTextAsync(ClientWebSocket socket, byte[] buffer, CancellationToken cancel)
    {
        var built = new StringBuilder();
        WebSocketReceiveResult result;
        do
        {
            result = await socket.ReceiveAsync(buffer, cancel);
            if (result.MessageType == WebSocketMessageType.Close)
            {
                throw new InvalidOperationException("Engine closed the WebSocket.");
            }

            built.Append(Encoding.UTF8.GetString(buffer, 0, result.Count));
        }
        while (!result.EndOfMessage);

        return built.ToString();
    }

    public void Dispose()
    {
        _http.Dispose();
        GC.SuppressFinalize(this);
    }
}
