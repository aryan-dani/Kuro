using KuroShell.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using System.Numerics;
using Windows.UI;
using Windows.UI.ViewManagement;

namespace KuroShell;

public sealed class HudWindow : Window
{
    private readonly Grid _host;
    private readonly Microsoft.UI.Composition.ContainerVisual _rootVisual;
    private readonly Microsoft.UI.Composition.SpriteVisual[] _bars;
    private readonly Spring[] _springs;
    private readonly DispatcherTimer _timer;
    private readonly UISettings _ui = new();
    private string _state = "hidden";
    private double[] _targets;
    private double _shake;
    private bool _showIdleDot;
    private DateTime _resultUntil = DateTime.MinValue;

    public HudWindow()
    {
        _host = new Grid
        {
            Background = new SolidColorBrush(DesignTokens.Fill),
        };
        Content = _host;
        ExtendsContentIntoTitleBar = true;
        SystemBackdrop = null;
        AppWindow.IsShownInSwitchers = false;
        Title = "Kuro";

        var compositor = ElementCompositionPreview.GetElementVisual(_host).Compositor;
        _rootVisual = compositor.CreateContainerVisual();
        ElementCompositionPreview.SetElementChildVisual(_host, _rootVisual);
        _bars = new Microsoft.UI.Composition.SpriteVisual[DesignTokens.BarCount];
        _springs = new Spring[DesignTokens.BarCount];
        _targets = new double[DesignTokens.BarCount];
        var brush = compositor.CreateColorBrush(DesignTokens.Ink);
        for (var i = 0; i < _bars.Length; i++)
        {
            var bar = compositor.CreateSpriteVisual();
            bar.Brush = brush;
            bar.Size = new Vector2((float)DesignTokens.BarWidth, 4);
            _rootVisual.Children.InsertAtTop(bar);
            _bars[i] = bar;
            _springs[i] = new Spring();
        }

        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(33) };
        _timer.Tick += (_, _) => Tick();
        _timer.Start();
        Activated += (_, _) => ApplyChrome();
        ApplyChrome();
        ShowState("hidden");
    }

    public bool ShowIdleDot
    {
        get => _showIdleDot;
        set => _showIdleDot = value;
    }

    public void ShowState(string state)
    {
        _state = state;
        if (state == "result")
        {
            _resultUntil = DateTime.UtcNow.AddMilliseconds(DesignTokens.ResultHoldMs);
        }

        if (state == "hidden" && !_showIdleDot)
        {
            AppWindow.Hide();
            return;
        }

        var width = state == "hidden" ? 6 : DesignTokens.CapsuleWidth;
        var height = state == "hidden" ? 6 : (int)DesignTokens.CapsuleHeight;
        var place = WindowChrome.Place(width, height);
        AppWindow.Resize(new Windows.Graphics.SizeInt32(width, height));
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        WindowChrome.ShowTopmost(hwnd, place.X, place.Y, width, height);
    }

    public void SetLevels(double[] levels)
    {
        for (var i = 0; i < _targets.Length; i++)
        {
            _targets[i] = i < levels.Length ? Math.Clamp(levels[i], 0, 1) : 0;
        }
    }

    private void ApplyChrome()
    {
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        WindowChrome.MakeHud(hwnd);
    }

    private void Tick()
    {
        if (_state == "result" && DateTime.UtcNow >= _resultUntil)
        {
            ShowState("hidden");
            return;
        }

        if (_state == "exit")
        {
            AppWindow.Hide();
            return;
        }

        var animate = _ui.AnimationsEnabled;
        var height = (float)(DesignTokens.CapsuleHeight - 16);
        var width = (float)DesignTokens.BarWidth;
        var gap = (float)DesignTokens.BarGap;
        _shake = _state == "error" && animate ? Math.Sin(Environment.TickCount64 / 40.0) * 4 : 0;
        for (var i = 0; i < _bars.Length; i++)
        {
            var target = _state switch
            {
                "listening" or "speaking" => 4 + (_targets[i] * (height - 4)),
                "thinking" => 4 + (height * 0.35),
                "result" => height * 0.55,
                _ => 4,
            };
            var sprung = _springs[i].Step(target, 0.033, animate);
            var x = 14 + (i * (width + gap)) + (float)_shake;
            _bars[i].Offset = new Vector3(x, (float)((DesignTokens.CapsuleHeight - sprung) / 2), 0);
            _bars[i].Size = new Vector2(width, (float)Math.Max(2, sprung));
        }

        if (_state == "thinking" && animate && _bars.Length > 0)
        {
            var index = (int)(Math.Abs(Environment.TickCount64 / 120) % _bars.Length);
            _bars[index].Size = new Vector2(width, height * 0.7f);
        }
    }
}
