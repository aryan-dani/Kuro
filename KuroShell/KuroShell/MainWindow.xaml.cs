using System.Text.Json;
using KuroShell.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;

namespace KuroShell;

public sealed partial class MainWindow : Window
{
    private readonly EngineClient _client;
    private readonly Button[] _nav;
    private readonly Action _onThread;
    private bool _loading = true;
    private bool _voiceReady;

    public MainWindow(EngineClient client)
    {
        _client = client;
        InitializeComponent();
        SystemBackdrop = null;
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(DragRegion);
        AppWindow.Resize(new SizeInt32(1080, 720));
        _nav =
        [
            NavConversation,
            NavVoice,
            NavDevices,
            NavContacts,
            NavSystem,
        ];
        ConversationLog.EnsureLoaded();
        RenderThread();
        _onThread = RenderThread;
        ConversationLog.Changed += _onThread;
        Closed += (_, _) => ConversationLog.Changed -= _onThread;
        BuildPlayground();
        Select("Conversation");
        _loading = false;
    }

    public async Task ShowPage(string page)
    {
        page = page switch
        {
            "General" or "Logs" or "Performance" or "Workflows" or "HUD" => page == "General" ? "Conversation" : "System",
            _ => page,
        };
        if (page is not ("Conversation" or "Voice" or "Devices" or "Contacts" or "System"))
        {
            page = "Conversation";
        }

        ConversationPage.Visibility = page == "Conversation" ? Visibility.Visible : Visibility.Collapsed;
        VoicePage.Visibility = page == "Voice" ? Visibility.Visible : Visibility.Collapsed;
        DevicesPage.Visibility = page == "Devices" ? Visibility.Visible : Visibility.Collapsed;
        ContactsPage.Visibility = page == "Contacts" ? Visibility.Visible : Visibility.Collapsed;
        SystemPage.Visibility = page == "System" ? Visibility.Visible : Visibility.Collapsed;
        Select(page);
        try
        {
            await Fill(page);
            SetStatus("");
        }
        catch (Exception)
        {
            SetStatus("The engine API is not answering.");
        }
    }

    private void OnNav(object sender, RoutedEventArgs e)
    {
        if (sender is Button button && button.Tag is string page)
        {
            _ = ShowPage(page);
        }
    }

    private async void OnNewConversation(object sender, RoutedEventArgs e)
    {
        var ok = await _client.SendAsync(HttpMethod.Post, "/thread/clear", "{}");
        if (!ok)
        {
            SetStatus("The engine kept its memory. Try again.");
            return;
        }

        ConversationLog.Clear();
        SetStatus("");
    }

    private async void OnSaveContact(object sender, RoutedEventArgs e)
    {
        var name = ContactName.Text.Trim();
        var number = ContactNumber.Text.Trim();
        if (name.Length == 0 || number.Length == 0)
        {
            SetStatus("Name and number are both required.");
            return;
        }

        var payload = JsonSerializer.Serialize(new Dictionary<string, string> { [name] = number });
        var ok = await _client.SendAsync(HttpMethod.Put, "/contacts", payload);
        SetStatus(ok ? "Saved." : "That number was rejected.");
        if (ok)
        {
            ContactName.Text = "";
            ContactNumber.Text = "";
            await ShowContacts();
        }
    }

    private async void OnPauseToggled(object sender, RoutedEventArgs e)
    {
        if (_loading)
        {
            return;
        }

        var json = PauseToggle.IsOn ? "{\"paused\":true}" : "{\"paused\":false}";
        var ok = await _client.SendAsync(HttpMethod.Put, "/config", json);
        if (!ok)
        {
            SetStatus("The engine API is not answering.");
        }
    }

    private void OnOpenLogs(object sender, RoutedEventArgs e)
    {
        var logs = LogPath.Text;
        if (string.IsNullOrWhiteSpace(logs))
        {
            return;
        }

        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
        {
            FileName = logs,
            UseShellExecute = true,
        });
    }

    private void OnRestart(object sender, RoutedEventArgs e)
    {
        SetStatus("Restarting.");
        Task.Run(App.Supervisor.Restart);
    }

    private async Task Fill(string page)
    {
        switch (page)
        {
            case "Voice":
                await ShowVoice();
                break;
            case "Devices":
                await ShowDevices();
                break;
            case "Contacts":
                await ShowContacts();
                break;
            case "System":
                await ShowSystem();
                break;
        }
    }

    private async Task ShowVoice()
    {
        var config = await _client.GetAsync("/config");
        if (config is null)
        {
            SetStatus("The engine API is not answering.");
            return;
        }

        if (!_voiceReady)
        {
            VoiceRows.Children.Add(Choice("Language model", "llm_mode", ["off", "fallback", "always"], config));
            VoiceRows.Children.Add(Choice("Speech recognition", "stt_backend", ["auto", "cuda", "npu", "cpu"], config));
            VoiceRows.Children.Add(Choice("Voice", "tts_engine", ["auto", "xtts", "piper", "pyttsx3"], config));
            VoiceRows.Children.Add(LanguageRow(config));
            _voiceReady = true;
            return;
        }

        _loading = true;
        foreach (var child in VoiceRows.Children)
        {
            if (child is FrameworkElement element && element.Tag is ComboBox box && box.Tag is string key)
            {
                if (config.RootElement.TryGetProperty(key, out var value))
                {
                    box.SelectedItem = value.GetString();
                }
            }
        }

        _loading = false;
    }

    private Border Choice(string label, string key, string[] options, JsonDocument config)
    {
        var current = config.RootElement.TryGetProperty(key, out var value) ? value.GetString() ?? options[0] : options[0];
        var box = new ComboBox
        {
            ItemsSource = options,
            SelectedItem = current,
            Tag = key,
            HorizontalAlignment = HorizontalAlignment.Right,
            MinWidth = 160,
            CornerRadius = new CornerRadius(DesignTokens.RadiusRow),
            FontFamily = new FontFamily(DesignTokens.Face),
        };
        box.SelectionChanged += async (_, _) =>
        {
            if (_loading || box.SelectedItem is not string picked)
            {
                return;
            }

            var payload = JsonSerializer.Serialize(new Dictionary<string, string> { [key] = picked });
            var ok = await _client.SendAsync(HttpMethod.Put, "/config", payload);
            if (!ok)
            {
                SetStatus("That setting was rejected.");
            }
        };
        var row = Row(label, box, last: key == "xtts_language");
        row.Tag = box;
        return row;
    }

    private Border LanguageRow(JsonDocument config)
    {
        var current = config.RootElement.TryGetProperty("xtts_language", out var value) ? value.GetString() ?? "en" : "en";
        var box = new TextBox
        {
            Text = current,
            Width = 120,
            HorizontalAlignment = HorizontalAlignment.Right,
            CornerRadius = new CornerRadius(DesignTokens.RadiusRow),
            FontFamily = new FontFamily(DesignTokens.Face),
        };
        box.LostFocus += async (_, _) =>
        {
            var picked = box.Text.Trim().ToLowerInvariant();
            if (picked.Length < 2)
            {
                return;
            }

            var payload = JsonSerializer.Serialize(new Dictionary<string, string> { ["xtts_language"] = picked });
            var ok = await _client.SendAsync(HttpMethod.Put, "/config", payload);
            if (!ok)
            {
                SetStatus("That language was rejected.");
            }
        };
        return Row("Language", box, last: true);
    }

    private async Task ShowDevices()
    {
        DeviceRows.Children.Clear();
        var config = await _client.GetAsync("/config");
        var masked = config?.RootElement.TryGetProperty("plug_id_masked", out var id) == true ? id.GetString() : "";
        DeviceRows.Children.Add(Row("Desk plug", Text(string.IsNullOrEmpty(masked) ? "Not configured" : masked!), last: true));
    }

    private async Task ShowContacts()
    {
        ContactList.Children.Clear();
        var doc = await _client.GetAsync("/contacts");
        var any = false;
        if (doc is not null && doc.RootElement.TryGetProperty("contacts", out var contacts))
        {
            foreach (var item in contacts.EnumerateObject())
            {
                any = true;
                ContactList.Children.Add(Row(item.Name, Text(item.Value.GetString() ?? "masked"), last: false));
            }
        }

        if (!any)
        {
            ContactList.Children.Add(new TextBlock
            {
                Text = "No saved contacts yet.",
                Style = (Style)Root.Resources["MetaText"],
                Margin = new Thickness(0, 12, 0, 4),
            });
        }
    }

    private async Task ShowSystem()
    {
        var config = await _client.GetAsync("/config");
        var status = await _client.GetAsync("/status");
        _loading = true;
        PauseToggle.IsOn = config?.RootElement.TryGetProperty("paused", out var paused) == true && paused.GetBoolean();
        _loading = false;
        var logs = status?.RootElement.TryGetProperty("logs", out var folder) == true ? folder.GetString() : "";
        LogPath.Text = string.IsNullOrEmpty(logs) ? "Log folder unavailable." : logs!;

        var flows = await _client.GetAsync("/workflows");
        var names = new List<string>();
        if (flows is not null && flows.RootElement.TryGetProperty("workflows", out var registry) && registry.ValueKind == JsonValueKind.Object)
        {
            foreach (var item in registry.EnumerateObject())
            {
                names.Add(item.Name);
            }
        }

        WorkflowNote.Text = names.Count == 0 ? "No workflows saved." : string.Join(", ", names);

        TimingRows.Children.Clear();
        var timing = await _client.GetAsync("/timing");
        var added = 0;
        if (timing is not null && timing.RootElement.TryGetProperty("rows", out var rows))
        {
            foreach (var row in rows.EnumerateArray())
            {
                if (added == 8)
                {
                    break;
                }

                var stage = row.TryGetProperty("stage", out var stageValue) ? stageValue.GetString() : "";
                var ms = row.TryGetProperty("ms", out var msValue) ? msValue.GetString() : "";
                TimingRows.Children.Add(Row(stage ?? "", Text((ms ?? "") + " ms"), last: false));
                added++;
            }
        }

        if (added == 0)
        {
            TimingRows.Children.Add(new TextBlock
            {
                Text = "No timing rows yet.",
                Style = (Style)Root.Resources["MetaText"],
                Margin = new Thickness(0, 12, 0, 12),
            });
        }
    }

    private void BuildPlayground()
    {
        var wrap = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        foreach (var state in new[] { "listening", "thinking", "speaking", "result", "error", "hidden" })
        {
            var name = state;
            var button = new Button
            {
                Content = name,
                Style = (Style)Root.Resources["QuietButton"],
            };
            button.Click += (_, _) => App.Hud.ShowState(name);
            wrap.Children.Add(button);
        }

        var width = Slider("Width", DesignTokens.BarWidth, value => DesignTokens.BarWidth = value);
        var gap = Slider("Gap", DesignTokens.BarGap, value => DesignTokens.BarGap = value);
        Playground.Children.Add(wrap);
        Playground.Children.Add(width);
        Playground.Children.Add(gap);
    }

    private static Slider Slider(string label, double start, Action<double> apply)
    {
        var slider = new Slider
        {
            Header = label,
            Minimum = 1,
            Maximum = 8,
            Value = start,
            Width = 240,
            Margin = new Thickness(0, 12, 0, 0),
            FontFamily = new FontFamily(DesignTokens.Face),
        };
        slider.ValueChanged += (_, args) => apply(args.NewValue);
        return slider;
    }

    private void RenderThread()
    {
        if (DispatcherQueue is not null && !DispatcherQueue.HasThreadAccess)
        {
            DispatcherQueue.TryEnqueue(RenderThread);
            return;
        }

        ThreadList.Children.Clear();
        var turns = ConversationLog.Buffer.Turns;
        EmptyState.Visibility = turns.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        foreach (var turn in turns)
        {
            ThreadList.Children.Add(TurnCard(turn));
        }

        ThreadScroll.UpdateLayout();
        ThreadScroll.ChangeView(null, ThreadScroll.ScrollableHeight, null);
    }

    private Border TurnCard(Turn turn)
    {
        var stack = new StackPanel { Spacing = 6 };
        stack.Children.Add(new TextBlock
        {
            Text = Clock(turn.At),
            Style = (Style)Root.Resources["MetaText"],
        });
        stack.Children.Add(Label("You"));
        stack.Children.Add(new TextBlock { Text = turn.You, Style = (Style)Root.Resources["BodyText"] });
        stack.Children.Add(Label("Kuro"));
        stack.Children.Add(new TextBlock { Text = turn.Kuro, Style = (Style)Root.Resources["BodyText"] });
        return new Border
        {
            Style = (Style)Root.Resources["Card"],
            Margin = new Thickness(0),
            Padding = new Thickness(18, 14, 18, 16),
            Child = stack,
        };
    }

    private static TextBlock Label(string text)
    {
        return new TextBlock
        {
            Text = text,
            FontFamily = new FontFamily(DesignTokens.FaceMedium),
            FontSize = DesignTokens.MetaSize,
            CharacterSpacing = 40,
            Foreground = DesignTokens.MutedBrush,
            Margin = new Thickness(0, 6, 0, 0),
        };
    }

    private static TextBlock Text(string value)
    {
        return new TextBlock
        {
            Text = value,
            FontFamily = new FontFamily(DesignTokens.Face),
            FontSize = DesignTokens.BodySize,
            Foreground = DesignTokens.MutedBrush,
            HorizontalAlignment = HorizontalAlignment.Right,
        };
    }

    private static Border Row(string label, FrameworkElement trailing, bool last)
    {
        var grid = new Grid { Padding = new Thickness(0, 14, 0, 14) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var title = new TextBlock
        {
            Text = label,
            FontFamily = new FontFamily(DesignTokens.Face),
            FontSize = DesignTokens.BodySize,
            Foreground = DesignTokens.InkBrush,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(trailing, 1);
        grid.Children.Add(title);
        grid.Children.Add(trailing);
        return new Border
        {
            BorderBrush = DesignTokens.HairlineBrush,
            BorderThickness = last ? new Thickness(0) : new Thickness(0, 0, 0, 1),
            Child = grid,
        };
    }

    private void Select(string page)
    {
        foreach (var button in _nav)
        {
            var on = button.Tag as string == page;
            button.Background = on ? DesignTokens.DimBrush : new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        }
    }

    private void SetStatus(string text)
    {
        StatusLine.Text = text;
        StatusLine.Visibility = string.IsNullOrWhiteSpace(text) ? Visibility.Collapsed : Visibility.Visible;
    }

    private static string Clock(string at)
    {
        if (DateTime.TryParse(at, out var parsed))
        {
            return parsed.ToString("h:mm tt", System.Globalization.CultureInfo.InvariantCulture);
        }

        return at;
    }
}
