using KuroShell.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace KuroShell.Pages.Phrases;

public sealed partial class PhrasesPage : UserControl
{
    private readonly EngineClient? _client;
    private bool _busy;

    public PhrasesPage()
        : this(null)
    {
    }

    public PhrasesPage(EngineClient? client)
    {
        _client = client;
        InitializeComponent();
        Render(PhraseCatalog.All);
    }

    private void OnSearchChanged(object sender, TextChangedEventArgs e) => Render(PhraseCatalog.Filter(SearchBox.Text));

    private void Render(IReadOnlyList<string> phrases)
    {
        PhraseList.Children.Clear();
        if (phrases.Count == 0)
        {
            PhraseList.Children.Add(new TextBlock
            {
                Text = "No phrases match.",
                FontFamily = new FontFamily("ms-appx:///Assets/Fonts/JetBrainsMono-Regular.ttf#JetBrains Mono"),
                FontSize = 12,
                Foreground = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 82, 81, 78)),
                Margin = new Thickness(0, 8, 0, 0),
            });
            return;
        }

        foreach (var phrase in phrases)
        {
            var row = new Grid { Padding = new Thickness(0, 10, 0, 10) };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var label = new TextBlock
            {
                Text = phrase,
                FontFamily = new FontFamily("ms-appx:///Assets/Fonts/SpaceGrotesk-Regular.ttf#Space Grotesk"),
                FontSize = 14,
                Foreground = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 20, 20, 20)),
                VerticalAlignment = VerticalAlignment.Center,
                TextWrapping = TextWrapping.Wrap,
            };
            var action = ShellSay.IsVoiceOnly(phrase)
                ? new TextBlock
                {
                    Text = "Voice only",
                    FontFamily = new FontFamily("ms-appx:///Assets/Fonts/JetBrainsMono-Regular.ttf#JetBrains Mono"),
                    FontSize = 12,
                    Foreground = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 82, 81, 78)),
                    VerticalAlignment = VerticalAlignment.Center,
                }
                : (FrameworkElement)new Button
                {
                    Content = "Try",
                    Tag = phrase,
                    Background = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 255, 255, 255)),
                    BorderBrush = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 228, 226, 216)),
                    BorderThickness = new Thickness(1),
                    CornerRadius = new CornerRadius(8),
                    Padding = new Thickness(12, 6, 12, 6),
                    FontFamily = new FontFamily("ms-appx:///Assets/Fonts/SpaceGrotesk-Regular.ttf#Space Grotesk"),
                    Foreground = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 20, 20, 20)),
                };
            if (action is Button tryButton)
            {
                tryButton.Click += OnTry;
            }

            Grid.SetColumn(action, 1);
            row.Children.Add(label);
            row.Children.Add(action);
            PhraseList.Children.Add(new Border
            {
                BorderBrush = new SolidColorBrush(Windows.UI.Color.FromArgb(255, 228, 226, 216)),
                BorderThickness = new Thickness(0, 0, 0, 1),
                Child = row,
            });
        }
    }

    private async void OnTry(object sender, RoutedEventArgs e)
    {
        if (_busy || _client is null || sender is not Button button || button.Tag is not string phrase)
        {
            return;
        }

        if (ShellSay.IsVoiceOnly(phrase))
        {
            StatusNote.Text = ShellSay.VoiceOnlyMessage;
            return;
        }

        _busy = true;
        button.IsEnabled = false;
        try
        {
            var dry = await _client.SayAsync(ShellSay.DryRunBody(phrase));
            if (!dry.Ok && dry.Status == 403)
            {
                StatusNote.Text = ShellSay.VoiceOnlyMessage;
                return;
            }

            var result = await _client.SayAsync(ShellSay.ExecuteBody(phrase));
            if (result.Status == 403)
            {
                StatusNote.Text = ShellSay.VoiceOnlyMessage;
                return;
            }

            if (!result.Ok)
            {
                StatusNote.Text = "The engine API is not answering.";
                return;
            }

            var reply = string.IsNullOrWhiteSpace(result.Reply)
                ? (result.Matched ? "Done." : "I did not catch that.")
                : result.Reply;
            ConversationLog.Record(phrase, reply);
            StatusNote.Text = "Sent to Conversation.";
        }
        finally
        {
            _busy = false;
            button.IsEnabled = true;
        }
    }
}
