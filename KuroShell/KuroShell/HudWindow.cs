using KuroShell.Core;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using Windows.UI.ViewManagement;
using WinRT.Interop;

namespace KuroShell;

public sealed class HudWindow : Window
{
    private static double CloseSize => DesignTokens.CapsuleHeight;
    private const double RowGap = 16;
    private const double DockGap = 12;
    private const double DockWidth = 480;
    private const double DockHeight = 320;

    private readonly Grid _root;
    private readonly Border _capsule;
    private readonly Border _dock;
    private readonly StackPanel _turns;
    private readonly TextBlock _empty;
    private readonly TranslateTransform _shift = new();
    private readonly Border[] _bars;
    private readonly Spring[] _springs;
    private readonly Spring _widthSpring = new();
    private readonly Spring _heightSpring = new();
    private readonly DispatcherTimer _timer;
    private readonly NativeCapsule _native = new();
    private readonly UISettings _ui = new();
    private readonly Action _onThread;
    private double[] _levels = [];
    private string _state = "hidden";
    private double _phase;
    private bool _shown;
    private bool _showIdleDot;
    private bool _suppress;
    private bool _dockOpen;
    private double _drawnW = -1;
    private double _drawnH = -1;
    private int _anchorCenter;
    private int _anchorBottom;
    private int _placeX;
    private int _placeY;
    private int _placeW;
    private int _placeH;
    private bool _anchored;

    public HudWindow()
    {
        var presenter = OverlappedPresenter.CreateForContextMenu();
        presenter.SetBorderAndTitleBar(false, false);
        presenter.IsAlwaysOnTop = true;
        presenter.IsResizable = false;
        presenter.IsMaximizable = false;
        presenter.IsMinimizable = false;
        AppWindow.SetPresenter(presenter);
        if (AppWindow.Presenter is OverlappedPresenter applied)
        {
            applied.IsAlwaysOnTop = true;
        }
        AppWindow.IsShownInSwitchers = false;
        SystemBackdrop = null;
        Title = "Kuro";

        var count = DesignTokens.BarCount;
        _bars = new Border[count];
        _springs = new Spring[count];
        var canvas = new Canvas
        {
            Width = count * DesignTokens.BarWidth + Math.Max(0, count - 1) * DesignTokens.BarGap,
            Height = Travel,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            RenderTransform = _shift,
        };
        for (var i = 0; i < count; i++)
        {
            var bar = new Border
            {
                Width = DesignTokens.BarWidth,
                Height = 4,
                CornerRadius = new CornerRadius(0),
                Background = new SolidColorBrush(DesignTokens.Ink),
            };
            Canvas.SetLeft(bar, i * (DesignTokens.BarWidth + DesignTokens.BarGap));
            Canvas.SetTop(bar, (Travel - 4) / 2);
            _bars[i] = bar;
            _springs[i] = new Spring();
            _springs[i].Snap(4);
            canvas.Children.Add(bar);
        }

        _capsule = new Border
        {
            Width = DesignTokens.CapsuleWidth,
            Height = DesignTokens.CapsuleHeight,
            CornerRadius = new CornerRadius(0),
            Background = DesignTokens.FillBrush,
            BorderBrush = DesignTokens.HairlineBrush,
            BorderThickness = new Thickness(1),
            Child = canvas,
        };
        _capsule.PointerPressed += (_, args) =>
        {
            ToggleDock();
            args.Handled = true;
        };

        var close = new Border
        {
            Width = DesignTokens.CapsuleHeight,
            Height = DesignTokens.CapsuleHeight,
            CornerRadius = new CornerRadius(0),
            Background = DesignTokens.FillBrush,
            BorderBrush = DesignTokens.HairlineBrush,
            BorderThickness = new Thickness(1),
            Child = new TextBlock
            {
                Text = "\u00D7",
                FontFamily = new FontFamily(DesignTokens.FaceMedium),
                FontSize = 20,
                Foreground = DesignTokens.InkBrush,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
                TextAlignment = TextAlignment.Center,
            },
        };
        close.PointerPressed += (_, args) =>
        {
            Dismiss();
            args.Handled = true;
        };

        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = RowGap,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Top,
        };
        row.Children.Add(close);
        row.Children.Add(_capsule);

        _turns = new StackPanel { Spacing = 14 };
        _empty = new TextBlock
        {
            Text = "Hold Ctrl+Alt+Space.",
            FontFamily = new FontFamily(DesignTokens.Face),
            FontSize = DesignTokens.BodySize,
            Foreground = DesignTokens.MutedBrush,
        };
        _turns.Children.Add(_empty);
        _dock = new Border
        {
            Width = DockWidth,
            Height = 0,
            Margin = new Thickness(0, DockGap, 0, 0),
            CornerRadius = new CornerRadius(0),
            Background = DesignTokens.CardBrush,
            BorderBrush = DesignTokens.HairlineBrush,
            BorderThickness = new Thickness(1),
            Padding = new Thickness(18, 14, 18, 14),
            Visibility = Visibility.Collapsed,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Top,
            Child = new ScrollViewer
            {
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                Content = _turns,
            },
        };

        var column = new StackPanel
        {
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Top,
        };
        column.Children.Add(row);
        column.Children.Add(_dock);
        _root = new Grid
        {
            Background = DesignTokens.FillBrush,
            Children = { column },
        };
        Content = _root;
        AppWindow.Resize(new SizeInt32(200, 56));
        AppWindow.Hide();

        _widthSpring.Snap(RowWidth);
        _heightSpring.Snap(DesignTokens.CapsuleHeight);
        ConversationLog.EnsureLoaded();
        RenderDock();
        _onThread = RenderDock;
        ConversationLog.Changed += _onThread;

        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(33) };
        _timer.Tick += (_, _) => Tick();
        _timer.Start();
        _native.DismissRequested = Dismiss;
    }

    public Action? DismissRequested { get; set; }

    public bool ShowIdleDot
    {
        get => _showIdleDot;
        set => _showIdleDot = value;
    }

    private static double Travel => Math.Max(8, DesignTokens.CapsuleHeight - 16);

    private static double RowWidth => CloseSize + RowGap + DesignTokens.CapsuleWidth;

    public void ShowState(string state)
    {
        var next = string.IsNullOrWhiteSpace(state) ? "hidden" : state;
        if (_suppress && next != "hidden")
        {
            return;
        }

        if (next == "hidden")
        {
            _suppress = false;
        }

        _state = next;
        _native.SetMode(_state);
        if (_state == "hidden" && !_showIdleDot)
        {
            _shown = false;
            _dockOpen = false;
            _widthSpring.Snap(RowWidth);
            _heightSpring.Snap(DesignTokens.CapsuleHeight);
            _dock.Height = 0;
            _dock.Visibility = Visibility.Collapsed;
            _native.Hide();
            AppWindow.Hide();
            return;
        }

        _shown = true;
        _native.Show();
        _native.Paint(CurrentHeights());
    }

    public void SetLevels(double[] levels)
    {
        _levels = levels;
    }

    private void Dismiss()
    {
        _suppress = true;
        _dockOpen = false;
        ShowState("hidden");
        DismissRequested?.Invoke();
    }

    private void ToggleDock()
    {
        _dockOpen = !_dockOpen;
        if (_dockOpen)
        {
            RenderDock();
            _dock.Visibility = Visibility.Visible;
            _widthSpring.Snap(DockWidth);
        }
    }

    private void Place()
    {
        var hwnd = WindowNative.GetWindowHandle(this);
        var scale = WindowChrome.DpiScale(hwnd);
        if (!_anchored)
        {
            var gap = (int)Math.Ceiling(80 * scale);
            var probe = WindowChrome.Place(10, 10, gap);
            _anchorCenter = probe.X + 5;
            _anchorBottom = probe.Y + 10;
            _anchored = true;
        }

        WindowChrome.MakeOverlay(hwnd);
        _shown = true;
        ApplyBounds(scale, force: true);
        AppWindow.Show();
        if (AppWindow.Presenter is OverlappedPresenter live)
        {
            live.IsAlwaysOnTop = true;
        }

        WindowChrome.ShowTopmost(hwnd, _placeX, _placeY, _placeW, _placeH);
        WindowChrome.ShowTopmost(hwnd, _placeX, _placeY, _placeW, _placeH);
    }

    private void Tick()
    {
        if (!_shown)
        {
            return;
        }

        _phase += 0.033;
        _shift.X = 0;
        var animate = _ui.AnimationsEnabled;
        for (var i = 0; i < _bars.Length; i++)
        {
            var bar = _springs[i].Step(Target(i), 0.033, animate);
            bar = Math.Clamp(bar, 3, Travel);
            _bars[i].Height = bar;
            Canvas.SetTop(_bars[i], (Travel - bar) / 2);
        }

        _native.Paint(CurrentHeights());

        var targetH = DesignTokens.CapsuleHeight + (_dockOpen ? DockGap + DockHeight : 0);
        var height = _heightSpring.Step(targetH, 0.033, animate);
        if (!_dockOpen && height < DesignTokens.CapsuleHeight + 1)
        {
            _dock.Visibility = Visibility.Collapsed;
            _widthSpring.Snap(RowWidth);
        }
    }

    private double[] CurrentHeights()
    {
        var values = new double[_bars.Length];
        for (var i = 0; i < _bars.Length; i++)
        {
            values[i] = _bars[i].Height;
        }

        return values;
    }

    private void ApplyBounds(double scale, bool force, double width = 0, double height = 0)
    {
        width = Math.Clamp(width <= 0 ? RowWidth : width, 80, 520);
        height = Math.Clamp(height <= 0 ? DesignTokens.CapsuleHeight : height, 32, 420);

        if (!force && Math.Abs(width - _drawnW) < 0.8 && Math.Abs(height - _drawnH) < 0.8)
        {
            return;
        }

        _drawnW = width;
        _drawnH = height;
        var dock = Math.Max(0, height - DesignTokens.CapsuleHeight - DockGap);
        _dock.Height = dock;
        _dock.Visibility = dock < 1 ? Visibility.Collapsed : Visibility.Visible;
        _root.Width = width;
        _root.Height = height;

        var pixelsW = Math.Max(1, (int)Math.Ceiling(width * scale));
        var pixelsH = Math.Max(1, (int)Math.Ceiling(height * scale));
        _placeX = _anchorCenter - (pixelsW / 2);
        _placeY = _anchorBottom - pixelsH;
        _placeW = pixelsW;
        _placeH = pixelsH;
        AppWindow.Resize(new SizeInt32(pixelsW, pixelsH));
        AppWindow.Move(new PointInt32(_placeX, _placeY));
        if (_shown)
        {
            var hwnd = WindowNative.GetWindowHandle(this);
            WindowChrome.ShowTopmost(hwnd, _placeX, _placeY, pixelsW, pixelsH);
        }
    }

    private void RenderDock()
    {
        if (DispatcherQueue is not null && !DispatcherQueue.HasThreadAccess)
        {
            DispatcherQueue.TryEnqueue(RenderDock);
            return;
        }

        _turns.Children.Clear();
        var turns = ConversationLog.Buffer.Turns;
        if (turns.Count == 0)
        {
            _turns.Children.Add(_empty);
            return;
        }

        foreach (var turn in turns)
        {
            _turns.Children.Add(TurnBlock(turn));
        }
    }

    private static StackPanel TurnBlock(Turn turn)
    {
        var stack = new StackPanel { Spacing = 4 };
        stack.Children.Add(Meta(Clock(turn.At)));
        stack.Children.Add(Who("You"));
        stack.Children.Add(Body(turn.You));
        stack.Children.Add(Who("Kuro"));
        stack.Children.Add(Body(turn.Kuro));
        return stack;
    }

    private static TextBlock Who(string text)
    {
        return new TextBlock
        {
            Text = text,
            FontFamily = new FontFamily(DesignTokens.FaceMedium),
            FontSize = 12,
            Foreground = DesignTokens.MutedBrush,
            Margin = new Thickness(0, 6, 0, 0),
        };
    }

    private static TextBlock Body(string text)
    {
        return new TextBlock
        {
            Text = text,
            FontFamily = new FontFamily(DesignTokens.Face),
            FontSize = 14,
            Foreground = DesignTokens.InkBrush,
            TextWrapping = TextWrapping.Wrap,
        };
    }

    private static TextBlock Meta(string text)
    {
        return new TextBlock
        {
            Text = text,
            FontFamily = new FontFamily(DesignTokens.Face),
            FontSize = 12,
            Foreground = DesignTokens.MutedBrush,
        };
    }

    private double Target(int index)
    {
        const double floor = 3;
        var span = Travel - floor;
        if (_state == "thinking")
        {
            var wave = 0.5 + 0.5 * Math.Sin(_phase * 5.2 - index * 0.62);
            return floor + span * (0.18 + 0.62 * wave);
        }

        if (_state == "result")
        {
            return floor + span * 0.38;
        }

        var level = index < _levels.Length ? Math.Clamp(_levels[index], 0, 1) : 0;
        var shaped = Math.Pow(level, 0.55);
        var mid = Math.Max(1, (_bars.Length - 1) / 2.0);
        var center = 1 - (Math.Abs(index - mid) / mid);
        var wobble = 0.12 * Math.Sin((_phase * 2.6) + (index * 0.55));
        var idle = Math.Clamp((0.08 + (0.72 * center * center)) + (wobble * center), 0.06, 1);
        var mixed = shaped > 0.12 ? shaped : idle;
        return floor + (span * mixed);
    }

    private static string Clock(string at)
    {
        if (DateTime.TryParse(at, out var parsed))
        {
            return parsed.ToString("h:mm tt");
        }

        return at;
    }
}
