using KuroShell.Core;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.Graphics;
using WinRT.Interop;

namespace KuroShell;

/// <summary>
/// Dark transcript card above the native capsule. Opens on pill click; collapses on chevron.
/// </summary>
public sealed class HudWindow : Window, IDisposable
{
    private const double DockGap = 12;
    private const double DockWidth = 640;
    private const double DockHeight = 360;
    private const int CapsuleBottomInset = 36;
    private const double ThumbWidth = 4;
    private const double ThumbInset = 10;

    private readonly Grid _root;
    private readonly Border _dock;
    private readonly StackPanel _turns;
    private readonly TextBlock _empty;
    private readonly ScrollViewer _scroll;
    private readonly Border _thumb;
    private readonly TranslateTransform _slide = new();
    private readonly DispatcherTimer _timer;
    private readonly NativeCapsule _native = new();
    private readonly EscapeHook _escape;
    private readonly Action _onThread;
    private double[] _levels = [];
    private string _state = "hidden";
    private bool _shown;
    private bool _showIdleDot;
    private bool _suppress;
    private bool _forceHidden;
    private bool _dockOpen;
    private bool _animating;
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

        try
        {
            AppWindow.IsShownInSwitchers = false;
        }
        catch (NotImplementedException)
        {
        }
        catch (System.Runtime.InteropServices.COMException)
        {
        }
        SystemBackdrop = null;
        Title = "Kuro";

        _turns = new StackPanel
        {
            Spacing = 14,
            Padding = new Thickness(0, 0, 14, 0),
        };
        _empty = new TextBlock
        {
            Text = "Hold Ctrl+Alt+Space.",
            FontFamily = new FontFamily(DesignTokens.ChatFace),
            FontSize = DesignTokens.BodySize,
            Foreground = DesignTokens.ChatMutedBrush,
        };
        _turns.Children.Add(_empty);
        _scroll = new ScrollViewer
        {
            VerticalScrollBarVisibility = ScrollBarVisibility.Hidden,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            Content = _turns,
        };
        _scroll.ViewChanged += (_, _) => SyncThumb();
        _scroll.SizeChanged += (_, _) => SyncThumb();

        _thumb = new Border
        {
            Width = ThumbWidth,
            Height = 48,
            CornerRadius = new CornerRadius(ThumbWidth / 2),
            Background = DesignTokens.ChatThumbBrush,
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(0, ThumbInset, ThumbInset, ThumbInset),
            Opacity = 0.55,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed,
        };

        var body = new Grid();
        body.Children.Add(_scroll);
        body.Children.Add(_thumb);

        _dock = new Border
        {
            Width = DockWidth,
            Height = DockHeight,
            CornerRadius = new CornerRadius(DesignTokens.ChatRadius),
            Background = DesignTokens.ChatPanelBrush,
            BorderThickness = new Thickness(0),
            Padding = new Thickness(22, 18, 14, 18),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch,
            Child = body,
            RenderTransform = _slide,
            Opacity = 0,
        };

        _root = new Grid
        {
            Background = DesignTokens.ChatPanelBrush,
            Width = DockWidth,
            Height = DockHeight,
            Children = { _dock },
        };
        Content = _root;
        AppWindow.Resize(new SizeInt32((int)DockWidth, (int)DockHeight));
        AppWindow.Hide();

        ConversationLog.EnsureLoaded();
        RenderDock();
        _onThread = OnThreadChanged;
        ConversationLog.Changed += _onThread;

        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(33) };
        _timer.Tick += (_, _) => Tick();
        _timer.Start();
        _native.BottomInset = CapsuleBottomInset;
        _native.DismissRequested = Dismiss;
        // Pill stays visual-only for listening/speaking; the up-chevron opens and closes chat.
        _native.CapsuleClicked = null;
        _native.CollapseRequested = ToggleDock;
        _escape = new EscapeHook(
            () => _shown && (_dockOpen || _state is "listening" or "thinking" or "processing" or "speaking" or "result" or "error"),
            Dismiss);
    }

    public Action? DismissRequested { get; set; }

    public bool ShowIdleDot
    {
        get => _showIdleDot;
        set => _showIdleDot = value;
    }

    public void ShowState(string state, string? label = null)
    {
        var next = string.IsNullOrWhiteSpace(state) ? "hidden" : state;
        var active = next is "listening" or "thinking" or "processing" or "speaking" or "error";

        // X / Escape hide the capsule until the next real turn.
        if (_forceHidden)
        {
            if (!active)
            {
                return;
            }

            _forceHidden = false;
            _suppress = false;
        }

        // After dismiss, ignore stale speaking/thinking frames until a fresh active state.
        if (_suppress && !active)
        {
            return;
        }

        if (active)
        {
            _suppress = false;
        }

        var wasShown = _shown;
        // Quiet / empty PTT may ask to hide. If the chat is open for reading, stay put.
        if (next == "hidden" && _dockOpen && !_suppress)
        {
            _state = "result";
            _native.BottomInset = CapsuleBottomInset;
            _native.SetMode("result");
            _native.SetCaption("");
            UpdateChevron();
            _native.Paint(_levels);
            return;
        }

        _state = next;
        _native.BottomInset = CapsuleBottomInset;
        _native.SetMode(_state);
        _native.SetCaption(Readable(next, label));
        if (_state == "hidden" && !_showIdleDot)
        {
            if (!wasShown)
            {
                return;
            }

            _shown = false;
            HideDock(immediate: true);
            _native.Hide();
            return;
        }

        _shown = true;
        if (!wasShown)
        {
            _native.Show();
        }

        // Up-chevron appears as soon as Kuro is answering (and stays while a turn exists).
        UpdateChevron();
        _native.Paint(_levels);
    }

    private void OnThreadChanged()
    {
        RenderDock(scrollToLatest: !_dockOpen || IsScrolledNearBottom());
        UpdateChevron();
        if (_dockOpen)
        {
            // Re-anchor once when a new turn lands; never every animation frame.
            PlaceDockAbovePill(activate: false);
        }
    }

    private bool IsScrolledNearBottom()
    {
        var max = Math.Max(0, _scroll.ScrollableHeight);
        return max <= 8 || _scroll.VerticalOffset >= max - 48;
    }

    private void UpdateChevron()
    {
        // Quiet / error still offer the chevron when there is something to read.
        var answering = _state is "speaking" or "result" or "thinking" or "processing" or "error";
        var hasTurn = ConversationLog.Buffer.Turns.Count > 0;
        _native.CollapseVisible = _shown && (answering || hasTurn || _dockOpen);
        _native.CollapsePointsDown = _dockOpen;
    }

    private static string Readable(string state, string? label)
    {
        var text = label ?? "";
        if (text.Contains("quiet", StringComparison.OrdinalIgnoreCase))
        {
            return "Quiet";
        }

        if (text.Contains("short", StringComparison.OrdinalIgnoreCase))
        {
            return "Hold";
        }

        if (text.Contains("fail", StringComparison.OrdinalIgnoreCase)
            || text.Contains("could not", StringComparison.OrdinalIgnoreCase)
            || text.Contains("heard nothing", StringComparison.OrdinalIgnoreCase))
        {
            return "Quiet";
        }

        return state switch
        {
            "thinking" or "processing" => "Thinking",
            "result" => "",
            "error" => "Quiet",
            _ => "",
        };
    }

    public void SetLevels(double[] levels)
    {
        _levels = levels;
    }

    private void Dismiss()
    {
        if (DispatcherQueue is not null && !DispatcherQueue.HasThreadAccess)
        {
            DispatcherQueue.TryEnqueue(Dismiss);
            return;
        }

        _suppress = true;
        _forceHidden = true;
        _state = "hidden";
        _shown = false;
        HideDock(immediate: true);
        _native.Hide();
        DismissRequested?.Invoke();
    }

    private void ToggleDock()
    {
        if (DispatcherQueue is not null && !DispatcherQueue.HasThreadAccess)
        {
            DispatcherQueue.TryEnqueue(ToggleDock);
            return;
        }

        if (_dockOpen)
        {
            HideDock();
            return;
        }

        ShowDock();
    }

    private void ShowDock()
    {
        if (_animating)
        {
            return;
        }

        RenderDock(scrollToLatest: true);
        _dockOpen = true;
        UpdateChevron();
        PlaceDockAbovePill(activate: true);
        AppWindow.Show();
        if (AppWindow.Presenter is OverlappedPresenter live)
        {
            live.IsAlwaysOnTop = true;
        }

        var hwnd = WindowNative.GetWindowHandle(this);
        // Must accept focus so scroll and text selection work.
        WindowChrome.MakeInteractiveOverlay(hwnd);
        ClipCard(hwnd);
        WindowChrome.ShowTopmost(hwnd, AppWindow.Position.X, AppWindow.Position.Y, AppWindow.Size.Width, AppWindow.Size.Height, activate: true);
        AnimateOpen();
        SyncThumb();
    }

    private void HideDock(bool immediate = false)
    {
        if (!_dockOpen && !AppWindow.IsVisible)
        {
            UpdateChevron();
            return;
        }

        _dockOpen = false;
        UpdateChevron();
        if (immediate || _animating)
        {
            _dock.Opacity = 0;
            _slide.Y = 18;
            AppWindow.Hide();
            return;
        }

        AnimateClose(() => AppWindow.Hide());
    }

    private void AnimateOpen()
    {
        _animating = true;
        _slide.Y = 22;
        _dock.Opacity = 0;
        var story = new Storyboard();
        var move = new DoubleAnimation
        {
            To = 0,
            Duration = TimeSpan.FromMilliseconds(280),
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        };
        Storyboard.SetTarget(move, _slide);
        Storyboard.SetTargetProperty(move, "Y");
        var fade = new DoubleAnimation
        {
            To = 1,
            Duration = TimeSpan.FromMilliseconds(240),
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
        };
        Storyboard.SetTarget(fade, _dock);
        Storyboard.SetTargetProperty(fade, "Opacity");
        story.Children.Add(move);
        story.Children.Add(fade);
        story.Completed += (_, _) => _animating = false;
        story.Begin();
    }

    private void AnimateClose(Action done)
    {
        _animating = true;
        var story = new Storyboard();
        var move = new DoubleAnimation
        {
            To = 18,
            Duration = TimeSpan.FromMilliseconds(200),
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        Storyboard.SetTarget(move, _slide);
        Storyboard.SetTargetProperty(move, "Y");
        var fade = new DoubleAnimation
        {
            To = 0,
            Duration = TimeSpan.FromMilliseconds(180),
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        Storyboard.SetTarget(fade, _dock);
        Storyboard.SetTargetProperty(fade, "Opacity");
        story.Children.Add(move);
        story.Children.Add(fade);
        story.Completed += (_, _) =>
        {
            _animating = false;
            done();
        };
        story.Begin();
    }

    private void PlaceDockAbovePill(bool activate)
    {
        var hwnd = WindowNative.GetWindowHandle(this);
        var scale = WindowChrome.DpiScale(hwnd);
        var pixelsW = Math.Max(1, (int)Math.Ceiling(DockWidth * scale));
        var pixelsH = Math.Max(1, (int)Math.Ceiling(DockHeight * scale));
        var gap = Math.Max(1, (int)Math.Ceiling(DockGap * scale));

        var pill = WindowChrome.Place(_native.PixelWidth, _native.PixelHeight, CapsuleBottomInset);
        var x = pill.X + ((_native.PixelWidth - pixelsW) / 2);
        var y = pill.Y - gap - pixelsH;
        if (y < 8)
        {
            y = 8;
        }

        _root.Width = DockWidth;
        _root.Height = DockHeight;
        _dock.Width = DockWidth;
        _dock.Height = DockHeight;
        AppWindow.Resize(new SizeInt32(pixelsW, pixelsH));
        AppWindow.Move(new PointInt32(x, y));
        ClipCard(hwnd);
        WindowChrome.ShowTopmost(hwnd, x, y, pixelsW, pixelsH, activate);
    }

    private static void ClipCard(IntPtr hwnd)
    {
        var scale = WindowChrome.DpiScale(hwnd);
        var pixelsW = Math.Max(1, (int)Math.Ceiling(DockWidth * scale));
        var pixelsH = Math.Max(1, (int)Math.Ceiling(DockHeight * scale));
        var radius = Math.Max(1, (int)Math.Ceiling(DesignTokens.ChatRadius * scale));
        WindowChrome.ClipRoundRect(hwnd, pixelsW, pixelsH, radius);
    }

    private void Tick()
    {
        if (!_shown)
        {
            return;
        }

        // Only repaint the native pill. Re-placing the chat every frame stole focus
        // and killed scroll / text selection.
        _native.Paint(_levels);
    }

    private void SyncThumb()
    {
        var extent = _scroll.ExtentHeight;
        var viewport = _scroll.ViewportHeight;
        if (extent <= viewport + 1 || viewport <= 0)
        {
            _thumb.Visibility = Visibility.Collapsed;
            return;
        }

        var track = Math.Max(24, viewport - (ThumbInset * 2));
        var size = Math.Clamp(viewport / extent * track, 28, track);
        var max = Math.Max(0, extent - viewport);
        var offset = max <= 0 ? 0 : _scroll.VerticalOffset / max;
        var top = ThumbInset + ((track - size) * offset);
        _thumb.Height = size;
        _thumb.Margin = new Thickness(0, top, ThumbInset, ThumbInset);
        _thumb.Visibility = Visibility.Visible;
    }

    private void RenderDock(bool scrollToLatest = false)
    {
        if (DispatcherQueue is not null && !DispatcherQueue.HasThreadAccess)
        {
            DispatcherQueue.TryEnqueue(() => RenderDock(scrollToLatest));
            return;
        }

        _turns.Children.Clear();
        var turns = ConversationLog.Buffer.Turns;
        if (turns.Count == 0)
        {
            _turns.Children.Add(_empty);
            SyncThumb();
            return;
        }

        // Full thread is always available; open/new-turn jumps to the latest.
        // Scrolling up keeps your place and does not rebuild the tree.
        foreach (var turn in turns)
        {
            _turns.Children.Add(TurnBlock(turn));
        }

        if (scrollToLatest)
        {
            ScrollToLatest();
        }

        SyncThumb();
    }

    private void ScrollToLatest()
    {
        DispatcherQueue?.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () =>
        {
            _ = _scroll.ChangeView(null, _scroll.ScrollableHeight, null, false);
            SyncThumb();
        });
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
            FontFamily = new FontFamily(DesignTokens.ChatFaceSemi),
            FontSize = 12,
            Foreground = DesignTokens.ChatMutedBrush,
            Margin = new Thickness(0, 6, 0, 0),
            IsTextSelectionEnabled = true,
        };
    }

    private static TextBlock Body(string text)
    {
        return new TextBlock
        {
            Text = text,
            FontFamily = new FontFamily(DesignTokens.ChatFace),
            FontSize = 15,
            Foreground = DesignTokens.ChatInkBrush,
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = true,
        };
    }

    private static TextBlock Meta(string text)
    {
        return new TextBlock
        {
            Text = text,
            FontFamily = new FontFamily(DesignTokens.ChatFace),
            FontSize = 12,
            Foreground = DesignTokens.ChatMutedBrush,
            IsTextSelectionEnabled = true,
        };
    }

    private static string Clock(string at)
    {
        if (DateTime.TryParse(at, out var parsed))
        {
            return parsed.ToString("h:mm tt", System.Globalization.CultureInfo.InvariantCulture);
        }

        return at;
    }

    public void Dispose()
    {
        _escape.Dispose();
        GC.SuppressFinalize(this);
    }
}
