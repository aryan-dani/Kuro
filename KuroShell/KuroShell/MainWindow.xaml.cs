using System.Text.Json;
using KuroShell.Core;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using Windows.UI;

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
        ApplyLightCaptionButtons();
        AppWindow.Resize(new SizeInt32(1080, 720));
        _nav =
        [
            NavConversation,
            NavVoice,
            NavDevices,
            NavContacts,
            NavSystem,
            NavHealth,
            NavPhrases,
            NavSettings,
        ];
        ConversationLog.EnsureLoaded();
        RenderThread();
        _onThread = RenderThread;
        ConversationLog.Changed += _onThread;
        App.ShellStateChanged += OnShellState;
        Closed += (_, _) =>
        {
            ConversationLog.Changed -= _onThread;
            App.ShellStateChanged -= OnShellState;
        };
        Composer.Bind(_client);
        Composer.Status += SetStatus;
        Select("Conversation");
        SetShellState("idle");
        _loading = false;
    }

    private void ApplyLightCaptionButtons()
    {
        var ink = Color.FromArgb(255, 20, 20, 20);
        var mute = Color.FromArgb(255, 82, 81, 78);
        var hover = Color.FromArgb(255, 230, 229, 222);
        var titleBar = AppWindow.TitleBar;
        titleBar.ButtonForegroundColor = ink;
        titleBar.ButtonHoverForegroundColor = ink;
        titleBar.ButtonPressedForegroundColor = ink;
        titleBar.ButtonInactiveForegroundColor = mute;
        titleBar.ButtonBackgroundColor = Colors.Transparent;
        titleBar.ButtonInactiveBackgroundColor = Colors.Transparent;
        titleBar.ButtonHoverBackgroundColor = hover;
        titleBar.ButtonPressedBackgroundColor = Color.FromArgb(255, 212, 211, 204);
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

    private void OnEmptyChip(object sender, RoutedEventArgs e)
    {
        if (sender is Button button && button.Tag is string phrase)
        {
            Composer.Fill(phrase);
        }
    }

    private void OnDeskOn(object sender, RoutedEventArgs e) => _ = Composer.SendTextAsync("Desk on");

    private void OnDeskOff(object sender, RoutedEventArgs e) => _ = Composer.SendTextAsync("Desk off");

    private void OnShellState(string state) => SetShellState(state);

    private void SetShellState(string state)
    {
        var label = string.IsNullOrWhiteSpace(state) || state is "hidden" ? "idle" : state;
        StateChipText.Text = label;
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
            VoiceRows.Children.Add(Choice("Voice", "tts_engine", ["auto", "kokoro", "piper", "xtts", "pyttsx3"], config));
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
        var status = await _client.GetAsync("/status");
        var logs = status?.RootElement.TryGetProperty("logs", out var folder) == true ? folder.GetString() : "";
        LogPath.Text = string.IsNullOrEmpty(logs) ? "Log folder unavailable." : logs!;
        var routerMode = status?.RootElement.TryGetProperty("router_mode", out var rm) == true ? rm.GetString() : "";
        RouterModeText.Text = string.IsNullOrEmpty(routerMode) ? "unknown" : routerMode!;
        var ttsActive = status?.RootElement.TryGetProperty("tts_active", out var ta) == true ? ta.GetString() : "";
        TtsActiveText.Text = string.IsNullOrEmpty(ttsActive) ? "unknown" : ttsActive!;

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

    private void RenderThread()
    {
        if (DispatcherQueue is not null && !DispatcherQueue.HasThreadAccess)
        {
            DispatcherQueue.TryEnqueue(RenderThread);
            return;
        }

        ThreadList.Children.Clear();
        var turns = ConversationLog.Buffer.Turns;
        var empty = turns.Count == 0;
        EmptyState.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
        EmptyChips.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
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
            button.BorderBrush = on ? DesignTokens.InkBrush : new SolidColorBrush(Microsoft.UI.Colors.Transparent);
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
