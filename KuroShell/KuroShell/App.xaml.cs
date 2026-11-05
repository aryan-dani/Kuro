using H.NotifyIcon;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace KuroShell;

public partial class App : Application
{
    private Window? _host;
    private TaskbarIcon? _tray;
    private EngineClient? _client;
    private CancellationTokenSource? _events;

    public static EngineSupervisor Supervisor { get; } = new();
    public static HudWindow Hud { get; private set; } = null!;
    public static SettingsWindow? Settings { get; private set; }

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        Supervisor.StartOrAttach();
        _client = new EngineClient(Supervisor.Token);
        Hud = new HudWindow();
        _host = new Window { Title = "Kuro" };
        _host.AppWindow.Resize(new Windows.Graphics.SizeInt32(1, 1));
        _host.AppWindow.Hide();
        BuildTray();
        _events = new CancellationTokenSource();
        _ = Listen(_events.Token);
    }

    private void BuildTray()
    {
        var menu = new MenuFlyout();
        menu.Items.Add(Item("Status", () => _ = Open("General")));
        menu.Items.Add(Item("Pause listening", () => _ = Pause(true)));
        menu.Items.Add(Item("Settings", () => _ = Open("General")));
        menu.Items.Add(Item("Logs folder", () => _ = Open("Logs")));
        menu.Items.Add(Item("Restart engine", () => Task.Run(Supervisor.Restart)));
        menu.Items.Add(Item("Quit", () => _ = Quit()));
        try
        {
            _tray = new TaskbarIcon
            {
                ToolTipText = "Kuro " + Supervisor.Status,
                ContextFlyout = menu,
            };
            _tray.LeftClickCommand = new CommunityToolkit.Mvvm.Input.RelayCommand(() => _ = Open("General"));
            _tray.ForceCreate();
        }
        catch (Exception)
        {
            NativeTray.Show(menu, () => _ = Open("General"));
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

        Settings ??= new SettingsWindow(_client);
        Settings.Activate();
        await Settings.ShowPage(page);
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

    private static void ApplyFrame(string frame)
    {
        try
        {
            using var doc = System.Text.Json.JsonDocument.Parse(frame);
            var state = doc.RootElement.TryGetProperty("state", out var stateValue)
                ? stateValue.GetString() ?? "hidden"
                : "hidden";
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

            Hud.ShowState(state);
        }
        catch (Exception)
        {
            Hud.ShowState("error");
        }
    }

    private async Task Quit()
    {
        var owner = Settings ?? _host;
        if (owner is null || _client is null)
        {
            Exit();
            return;
        }

        if (Settings is null)
        {
            Settings = new SettingsWindow(_client);
        }

        Settings.Activate();
        var dialog = new ContentDialog
        {
            Title = "Quit Kuro",
            Content = "Stop the voice engine, or leave it running?",
            PrimaryButtonText = "Stop engine",
            SecondaryButtonText = "Leave it",
            CloseButtonText = "Cancel",
            XamlRoot = Settings.Content.XamlRoot,
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
