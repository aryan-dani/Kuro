using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace KuroShell.Pages.Health;

public sealed partial class HealthPage : UserControl
{
    private readonly EngineClient? _client;

    public HealthPage()
        : this(null)
    {
    }

    public HealthPage(EngineClient? client)
    {
        _client = client;
        InitializeComponent();
        Loaded += async (_, _) => await RefreshAsync();
    }

    private async void OnRefresh(object sender, RoutedEventArgs e) => await RefreshAsync();

    private async Task RefreshAsync()
    {
        Rows.Children.Clear();
        if (_client is null)
        {
            StatusNote.Text = "Engine client unavailable.";
            return;
        }

        var status = await _client.GetAsync("/status");
        if (status is null)
        {
            StatusNote.Text = "Engine is not answering.";
            AddRow("API", "down");
            AddRow("Paused", "unknown");
            AddRow("Router", "unknown");
            AddRow("Voice", "unknown");
            return;
        }

        StatusNote.Text = "Engine reachable.";
        var root = status.RootElement;
        AddRow("API", root.TryGetProperty("api", out var api) && api.GetBoolean() ? "up" : "down");
        AddRow("Paused", root.TryGetProperty("paused", out var paused) && paused.GetBoolean() ? "yes" : "no");
        AddRow("Router", Prop(root, "router_mode"));
        AddRow("Voice", Prop(root, "tts_active"));
    }

    private static string Prop(System.Text.Json.JsonElement root, string name)
    {
        if (!root.TryGetProperty(name, out var value))
        {
            return "unknown";
        }

        return value.ValueKind switch
        {
            System.Text.Json.JsonValueKind.String => value.GetString() ?? "",
            System.Text.Json.JsonValueKind.Number => value.ToString(),
            System.Text.Json.JsonValueKind.True => "true",
            System.Text.Json.JsonValueKind.False => "false",
            _ => value.ToString(),
        };
    }

    private void AddRow(string label, string value)
    {
        var grid = new Grid { Margin = new Thickness(0, 10, 0, 10) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(140) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var left = new TextBlock
        {
            Text = label,
            FontFamily = new Microsoft.UI.Xaml.Media.FontFamily("ms-appx:///Assets/Fonts/JetBrainsMono-Regular.ttf#JetBrains Mono"),
            FontSize = 12,
            Foreground = new Microsoft.UI.Xaml.Media.SolidColorBrush(Windows.UI.Color.FromArgb(255, 82, 81, 78)),
        };
        var right = new TextBlock
        {
            Text = string.IsNullOrWhiteSpace(value) ? "unknown" : value,
            FontFamily = new Microsoft.UI.Xaml.Media.FontFamily("ms-appx:///Assets/Fonts/SpaceGrotesk-Regular.ttf#Space Grotesk"),
            FontSize = 14,
            Foreground = new Microsoft.UI.Xaml.Media.SolidColorBrush(Windows.UI.Color.FromArgb(255, 20, 20, 20)),
            TextWrapping = TextWrapping.Wrap,
        };
        Grid.SetColumn(right, 1);
        grid.Children.Add(left);
        grid.Children.Add(right);
        Rows.Children.Add(grid);
    }
}
