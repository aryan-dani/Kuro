using H.NotifyIcon;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;

namespace KuroShell;

public partial class App : Application
{
    private TaskbarIcon? _tray;
    private EngineClient? _client;
    private CancellationTokenSource? _events;

    public static EngineSupervisor Supervisor { get; } = new();
    public static HudWindow Hud { get; private set; } = null!;
    public static MainWindow? Home { get; private set; }

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        Supervisor.StartOrAttach();
        _client = new EngineClient(Supervisor.Token);
        Hud = new HudWindow();
        Hud.DismissRequested = () =>
        {
            if (_client is not null)
            {
                _ = _client.SendAsync(HttpMethod.Post, "/dismiss", "{}");
            }
        };
        BuildTray();
        _events = new CancellationTokenSource();
        _ = Listen(_events.Token);
    }

    private void BuildTray()
    {
        var menu = new MenuFlyout();
        menu.Items.Add(Item("Open Kuro", () => _ = Open("Conversation")));
        menu.Items.Add(Item("Pause listening", () => _ = Pause(true)));
        menu.Items.Add(Item("Voice", () => _ = Open("Voice")));
        menu.Items.Add(Item("Logs folder", () => _ = Open("System")));
        menu.Items.Add(Item("Restart engine", () => Task.Run(Supervisor.Restart)));
        menu.Items.Add(Item("Quit", () => _ = Quit()));
        try
        {
            _tray = new TaskbarIcon
            {
                ToolTipText = "Kuro " + Supervisor.Status,
                ContextFlyout = menu,
            };
            _tray.LeftClickCommand = new CommunityToolkit.Mvvm.Input.RelayCommand(() => _ = Open("Conversation"));
            _tray.ForceCreate();
            SetTray("idle");
        }
        catch (Exception)
        {
            NativeTray.Show(menu, () => _ = Open("Conversation"));
        }
    }

    private static MenuFlyoutItem Item(string label, Action action)
    {
        var item = new MenuFlyoutItem { Text = label };
        item.Click += (_, _) => action();
        return item;
    }

    private async Task Open(string page)
    {
        if (_client is null)
        {
            return;
        }

        if (Home is null)
        {
            Home = new MainWindow(_client);
            Home.Closed += (_, _) => Home = null;
        }

        Home.Activate();
        await Home.ShowPage(page);
    }

    private async Task Pause(bool paused)
    {
        if (_client is null)
        {
            return;
        }

        var json = paused ? "{\"paused\":true}" : "{\"paused\":false}";
        await _client.SendAsync(HttpMethod.Put, "/config", json);
    }

    private async Task Listen(CancellationToken cancel)
    {
        if (_client is null)
        {
            return;
        }

        while (!cancel.IsCancellationRequested)
        {
            try
            {
                await _client.SendAsync(HttpMethod.Put, "/config", "{\"hud_show_text\":true}", cancel);
                await _client.ListenAsync(frame =>
                {
                    Hud.DispatcherQueue.TryEnqueue(() => ApplyFrame(frame));
                }, cancel);
            }
            catch (Exception)
            {
                await Task.Delay(1000, cancel);
            }
        }
    }

    private void ApplyFrame(string frame)
    {
        try
        {
            using var doc = System.Text.Json.JsonDocument.Parse(frame);
            var state = doc.RootElement.TryGetProperty("state", out var stateValue)
                ? stateValue.GetString() ?? "hidden"
                : "hidden";
            var you = doc.RootElement.TryGetProperty("you", out var youValue) ? youValue.GetString() ?? "" : "";
            var kuro = doc.RootElement.TryGetProperty("kuro", out var kuroValue) ? kuroValue.GetString() ?? "" : "";
            if (doc.RootElement.TryGetProperty("levels", out var levels))
            {
                var values = new double[levels.GetArrayLength()];
                var index = 0;
                foreach (var level in levels.EnumerateArray())
                {
                    values[index++] = level.GetDouble();
                }

                Hud.SetLevels(values);
            }

            ConversationLog.Observe(you, kuro);
            Hud.ShowState(state);
            SetTray(state);
        }
        catch (Exception)
        {
            Hud.ShowState("error");
            SetTray("idle");
        }
    }

    private void SetTray(string state)
    {
        if (_tray is null)
        {
            return;
        }

        var name = state switch
        {
            "listening" => "tray_listening.png",
            "thinking" or "processing" => "tray_processing.png",
            "speaking" => "tray_speaking.png",
            _ => "tray_idle.png",
        };
        var path = Path.Combine(AppContext.BaseDirectory, "Assets", name);
        if (!File.Exists(path))
        {
            return;
        }

        _tray.IconSource = new BitmapImage(new Uri(path));
    }

    private async Task Quit()
    {
        if (_client is null)
        {
            Exit();
            return;
        }

        if (Home is null)
        {
            Home = new MainWindow(_client);
            Home.Closed += (_, _) => Home = null;
        }

        Home.Activate();
        var dialog = new ContentDialog
        {
            Title = "Quit Kuro",
            Content = "Stop the voice engine, or leave it running?",
            PrimaryButtonText = "Stop engine",
            SecondaryButtonText = "Leave it",
            CloseButtonText = "Cancel",
            XamlRoot = Home.Content.XamlRoot,
        };
        var choice = await dialog.ShowAsync();
        if (choice == ContentDialogResult.None)
        {
            return;
        }

        if (choice == ContentDialogResult.Primary)
        {
            await Supervisor.StopEngine(_client);
        }

        _events?.Cancel();
        _tray?.Dispose();
        Exit();
    }
}
