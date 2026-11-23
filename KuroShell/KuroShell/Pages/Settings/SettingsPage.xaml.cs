using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace KuroShell.Pages.Settings;

public sealed partial class SettingsPage : UserControl
{
    private readonly EngineClient? _client;
    private bool _loading = true;

    public SettingsPage()
        : this(null)
    {
    }

    public SettingsPage(EngineClient? client)
    {
        _client = client;
        InitializeComponent();
        Loaded += async (_, _) => await LoadAsync();
    }

    private async Task LoadAsync()
    {
        if (_client is null)
        {
            StatusNote.Text = "Engine client unavailable.";
            return;
        }

        var config = await _client.GetAsync("/config");
        _loading = true;
        if (config is not null)
        {
            PauseToggle.IsOn = config.RootElement.TryGetProperty("paused", out var paused) && paused.GetBoolean();
            HudTextToggle.IsOn =
                !config.RootElement.TryGetProperty("hud_show_text", out var hud)
                || hud.ValueKind != System.Text.Json.JsonValueKind.False;
            StatusNote.Text = "Settings loaded.";
        }
        else
        {
            StatusNote.Text = "Engine is not answering.";
        }

        _loading = false;
    }

    private async void OnPauseToggled(object sender, RoutedEventArgs e)
    {
        if (_loading || _client is null)
        {
            return;
        }

        var json = PauseToggle.IsOn ? "{\"paused\":true}" : "{\"paused\":false}";
        var ok = await _client.SendAsync(HttpMethod.Put, "/config", json);
        StatusNote.Text = ok ? "Pause updated." : "Could not update pause.";
    }

    private async void OnHudTextToggled(object sender, RoutedEventArgs e)
    {
        if (_loading || _client is null)
        {
            return;
        }

        var json = HudTextToggle.IsOn ? "{\"hud_show_text\":true}" : "{\"hud_show_text\":false}";
        var ok = await _client.SendAsync(HttpMethod.Put, "/config", json);
        StatusNote.Text = ok ? "HUD text updated." : "Could not update HUD text.";
    }

    private void OnRestartEngine(object sender, RoutedEventArgs e)
    {
        StatusNote.Text = "Restarting engine.";
        Task.Run(App.Supervisor.Restart);
    }

    private void OnRestartKuro(object sender, RoutedEventArgs e)
    {
        StatusNote.Text = "Restarting Kuro.";
        if (Application.Current is App app)
        {
            _ = app.RequestRestartKuro();
        }
    }
}
