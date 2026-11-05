using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace KuroShell;

public sealed class SettingsWindow : Window
{
    private readonly EngineClient _client;
    private readonly TextBlock _body = new()
    {
        TextWrapping = TextWrapping.Wrap,
        Foreground = new SolidColorBrush(DesignTokens.Ink),
        FontSize = 14,
    };
    private readonly StackPanel _extra = new() { Spacing = 8 };

    public SettingsWindow(EngineClient client)
    {
        _client = client;
        Title = "Kuro";
        SystemBackdrop = null;
        var root = new Grid
        {
            Background = new SolidColorBrush(DesignTokens.Surface),
            Padding = new Thickness(16),
            ColumnDefinitions =
            {
                new ColumnDefinition { Width = new GridLength(180) },
                new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) },
            },
        };
        var nav = new StackPanel { Spacing = 6 };
        foreach (var page in KuroShell.Core.ShellConfig.Pages)
        {
            var name = page;
            var button = new Button
            {
                Content = name,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                Background = new SolidColorBrush(Color.FromArgb(255, 236, 236, 236)),
                Foreground = new SolidColorBrush(Color.FromArgb(255, 16, 16, 16)),
                CornerRadius = new CornerRadius(14),
                Padding = new Thickness(12, 6, 12, 6),
            };
            button.Click += (_, _) => _ = ShowPage(name);
            nav.Children.Add(button);
        }

        var content = new StackPanel { Spacing = 12, Margin = new Thickness(16, 0, 0, 0) };
        content.Children.Add(_body);
        content.Children.Add(_extra);
        Grid.SetColumn(content, 1);
        root.Children.Add(nav);
        root.Children.Add(content);
        Content = root;
        AppWindow.Resize(new Windows.Graphics.SizeInt32(760, 520));
    }

    public async Task ShowPage(string page)
    {
        _extra.Children.Clear();
        _body.Text = page;
        try
        {
            await Fill(page);
        }
        catch (Exception)
        {
            _body.Text = "The engine API is not answering.";
        }
    }

    private async Task Fill(string page)
    {
        switch (page)
        {
            case "General":
                _body.Text = "Hold Ctrl+Alt+Space. The Python engine stays in charge of the hotkey.";
                AddPause();
                break;
            case "Voice":
                await ShowConfig("llm_mode", "stt_backend", "tts_engine", "xtts_language");
                break;
            case "Devices":
                var config = await _client.GetAsync("/config");
                var masked = config?.RootElement.TryGetProperty("plug_id_masked", out var id) == true
                    ? id.GetString()
                    : "";
                _body.Text = string.IsNullOrEmpty(masked) ? "No plug id is configured." : "Plug id " + masked;
                break;
            case "Workflows":
                var flows = await _client.GetAsync("/workflows");
                _body.Text = flows is null ? "No workflow registry." : "Registry loaded. Edits save the whole file.";
                break;
            case "Contacts":
                await ShowContacts();
                break;
            case "Logs":
                var status = await _client.GetAsync("/status");
                var logs = status?.RootElement.TryGetProperty("logs", out var folder) == true
                    ? folder.GetString()
                    : "";
                _body.Text = string.IsNullOrEmpty(logs) ? "Log folder unavailable." : logs;
                var open = new Button { Content = "Open logs" };
                open.Click += (_, _) =>
                {
                    if (!string.IsNullOrEmpty(logs))
                    {
                        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                        {
                            FileName = logs,
                            UseShellExecute = true,
                        });
                    }
                };
                _extra.Children.Add(open);
                break;
            case "Performance":
                var timing = await _client.GetAsync("/timing");
                _body.Text = timing is null ? "No timing rows yet." : FormatRows(timing);
                break;
            case "HUD":
                _body.Text = "Bar width, gap, and the hidden playground.";
                AddHudSliders();
                AddPlayground();
                break;
        }
    }

    private void AddPause()
    {
        var pause = new Button { Content = "Pause listening" };
        pause.Click += async (_, _) =>
        {
            await _client.SendAsync(HttpMethod.Put, "/config", "{\"paused\":true}");
            _body.Text = "Listening is paused.";
        };
        var resume = new Button { Content = "Resume listening" };
        resume.Click += async (_, _) =>
        {
            await _client.SendAsync(HttpMethod.Put, "/config", "{\"paused\":false}");
            _body.Text = "Listening is on.";
        };
        _extra.Children.Add(pause);
        _extra.Children.Add(resume);
    }

    private async Task ShowConfig(params string[] keys)
    {
        var config = await _client.GetAsync("/config");
        if (config is null)
        {
            _body.Text = "Config unavailable.";
            return;
        }

        var lines = new List<string>();
        foreach (var key in keys)
        {
            if (config.RootElement.TryGetProperty(key, out var value))
            {
                lines.Add(key + ": " + value);
            }
        }

        _body.Text = string.Join(Environment.NewLine, lines);
    }

    private async Task ShowContacts()
    {
        var doc = await _client.GetAsync("/contacts");
        _body.Text = "Saved contacts are masked.";
        if (doc is not null && doc.RootElement.TryGetProperty("contacts", out var contacts))
        {
            foreach (var item in contacts.EnumerateObject())
            {
                _extra.Children.Add(new TextBlock
                {
                    Text = item.Name + "  " + item.Value.GetString(),
                    Foreground = new SolidColorBrush(DesignTokens.Ink),
                });
            }
        }

        var name = new TextBox { PlaceholderText = "Name", Width = 220 };
        var number = new TextBox { PlaceholderText = "Number", Width = 220 };
        var save = new Button { Content = "Save contact" };
        save.Click += async (_, _) =>
        {
            var payload = "{\"" + Escape(name.Text) + "\":\"" + Escape(number.Text) + "\"}";
            var ok = await _client.SendAsync(HttpMethod.Put, "/contacts", payload);
            _body.Text = ok ? "Saved." : "That number was rejected.";
        };
        _extra.Children.Add(name);
        _extra.Children.Add(number);
        _extra.Children.Add(save);
    }

    private void AddHudSliders()
    {
        _extra.Children.Add(SliderRow("Width", DesignTokens.BarWidth, value => DesignTokens.BarWidth = value));
        _extra.Children.Add(SliderRow("Gap", DesignTokens.BarGap, value => DesignTokens.BarGap = value));
    }

    private static Slider SliderRow(string label, double start, Action<double> apply)
    {
        var slider = new Slider { Header = label, Minimum = 1, Maximum = 8, Value = start, Width = 240 };
        slider.ValueChanged += (_, args) => apply(args.NewValue);
        return slider;
    }

    private void AddPlayground()
    {
        foreach (var state in new[] { "hidden", "listening", "thinking", "speaking", "result", "error", "exit" })
        {
            var name = state;
            var button = new Button { Content = name };
            button.Click += (_, _) => App.Hud.ShowState(name);
            _extra.Children.Add(button);
        }
    }

    private static string FormatRows(System.Text.Json.JsonDocument timing)
    {
        if (!timing.RootElement.TryGetProperty("rows", out var rows))
        {
            return "No timing rows yet.";
        }

        var lines = new List<string>();
        foreach (var row in rows.EnumerateArray())
        {
            var stage = row.TryGetProperty("stage", out var stageValue) ? stageValue.GetString() : "";
            var ms = row.TryGetProperty("ms", out var msValue) ? msValue.GetString() : "";
            lines.Add(stage + "  " + ms);
        }

        return lines.Count == 0 ? "No timing rows yet." : string.Join(Environment.NewLine, lines);
    }

    private static string Escape(string raw)
    {
        return (raw ?? "").Replace("\\", "").Replace("\"", "");
    }
}
