using System.Net.WebSockets;
using System.Text;
using System.Text.Json;

namespace KuroShell;

public sealed class EngineClient
{
    private readonly HttpClient _http = new() { Timeout = TimeSpan.FromSeconds(4) };

    public EngineClient(string token, int port = 8765)
    {
        Token = token;
        Port = port;
    }

    public string Token { get; }
    public int Port { get; }

    public Uri Base => new($"http://127.0.0.1:{Port}");

    public async Task<JsonDocument?> GetAsync(string path, CancellationToken cancel = default)
    {
        using var request = new HttpRequestMessage(HttpMethod.Get, new Uri(Base, path));
        request.Headers.TryAddWithoutValidation("X-Kuro-Token", Token);
        using var response = await _http.SendAsync(request, cancel);
        if (!response.IsSuccessStatusCode)
        {
            return null;
        }

        var text = await response.Content.ReadAsStringAsync(cancel);
        return JsonDocument.Parse(text);
    }

    public async Task<bool> SendAsync(HttpMethod method, string path, string json, CancellationToken cancel = default)
    {
        using var request = new HttpRequestMessage(method, new Uri(Base, path));
        request.Headers.TryAddWithoutValidation("X-Kuro-Token", Token);
        request.Content = new StringContent(json, Encoding.UTF8, "application/json");
        using var response = await _http.SendAsync(request, cancel);
        return response.IsSuccessStatusCode;
    }

    public async Task ListenAsync(Action<string> onFrame, CancellationToken cancel)
    {
        using var socket = new ClientWebSocket();
        socket.Options.SetRequestHeader("X-Kuro-Token", Token);
        var uri = new Uri($"ws://127.0.0.1:{Port}/events");
        await socket.ConnectAsync(uri, cancel);
        var buffer = new byte[8192];
        while (!cancel.IsCancellationRequested && socket.State == WebSocketState.Open)
        {
            var built = new StringBuilder();
            WebSocketReceiveResult result;
            do
            {
                result = await socket.ReceiveAsync(buffer, cancel);
                if (result.MessageType == WebSocketMessageType.Close)
                {
                    return;
                }

                built.Append(Encoding.UTF8.GetString(buffer, 0, result.Count));
            }
            while (!result.EndOfMessage);

            onFrame(built.ToString());
        }
    }
}
