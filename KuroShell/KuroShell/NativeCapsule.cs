using System.Runtime.InteropServices;

namespace KuroShell;

/// <summary>
/// A small always-on-top window created with WS_EX_TOPMOST.
/// The WinUI presenter window stays behind other apps, so the capsule is drawn here.
/// </summary>
internal sealed class NativeCapsule
{
    private const int WsPopup = unchecked((int)0x80000000);
    private const int WsExTopmost = 0x00000008;
    private const int WsExToolwindow = 0x00000080;
    private const int WsExNoactivate = 0x08000000;
    private const int WsExLayered = 0x00080000;
    private const uint SwpNoactivate = 0x0010;
    private const uint SwpShowwindow = 0x0040;
    private const int SwShowNoactivate = 4;
    private const int SwHide = 0;
    private const uint WmLbuttonUp = 0x0202;
    private const uint WmSetCursor = 0x0020;
    private const int IdcArrow = 32512;
    private const int IdcHand = 32649;
    private const uint WmMousemove = 0x0200;
    private const uint UlwAlpha = 2;

    private const int BarCount = 9;
    private const int BarWidth = 5;
    private const int BarGap = 5;
    private const int PillPadX = 22;

    private static readonly IntPtr HwndTopmost = new(-1);
    private static NativeCapsule? _live;
    private static readonly WndProcDelegate Proc = WndProc;
    private static bool _classReady;

    private readonly int _height;
    private readonly int _circle;
    private readonly int _gap;
    private readonly int _barCapsuleWidth;
    private int _capsuleWidth;
    private int _width;
    private IntPtr _hwnd;
    private string _mode = "hidden";
    private string _caption = "";
    private double _phase;
    private readonly double[] _smooth = new double[BarCount];
    private bool _fontReady;
    private int _bottomInset = 36;
    private bool _collapseVisible;
    private bool _collapsePointsDown;

    public NativeCapsule()
    {
        _height = (int)DesignTokens.CapsuleHeight;
        _circle = _height;
        _gap = 16;
        var bars = (BarCount * BarWidth) + ((BarCount - 1) * BarGap);
        _barCapsuleWidth = bars + (PillPadX * 2);
        _capsuleWidth = _barCapsuleWidth;
        _width = _circle + _gap + _capsuleWidth;
    }

    /// <summary>Pixels from the work-area bottom to the capsule. Raise this when the chat card is open.</summary>
    public int BottomInset
    {
        get => _bottomInset;
        set => _bottomInset = Math.Max(8, value);
    }

    public void SetMode(string state)
    {
        _mode = string.IsNullOrWhiteSpace(state) ? "hidden" : state;
    }

    public void SetCaption(string caption)
    {
        _caption = caption ?? "";
    }

    public Action? DismissRequested { get; set; }

    public Action? CapsuleClicked { get; set; }

    public Action? CollapseRequested { get; set; }

    public bool Visible { get; private set; }

    public bool CollapseVisible
    {
        get => _collapseVisible;
        set => _collapseVisible = value;
    }

    /// <summary>False = ^ (open chat). True = v (card is open).</summary>
    public bool CollapsePointsDown
    {
        get => _collapsePointsDown;
        set => _collapsePointsDown = value;
    }

    public int PixelWidth => _width;

    public int PixelHeight => _height;

    public void Show()
    {
        Ensure();
        if (Visible)
        {
            return;
        }

        var place = WindowChrome.Place(_width, _height, _bottomInset);
        SetWindowPos(_hwnd, HwndTopmost, place.X, place.Y, _width, _height, SwpNoactivate | SwpShowwindow);
        ShowWindow(_hwnd, SwShowNoactivate);
        Visible = true;
    }

    public void Hide()
    {
        if (_hwnd != IntPtr.Zero)
        {
            ShowWindow(_hwnd, SwHide);
        }

        Visible = false;
    }

    public void Paint(double[] heights)
    {
        if (_hwnd == IntPtr.Zero || !Visible)
        {
            return;
        }

        Draw(heights);
    }

    private void Ensure()
    {
        if (_hwnd != IntPtr.Zero)
        {
            return;
        }

        _live = this;
        if (!_classReady)
        {
            var cls = new WndClass
            {
                Proc = Marshal.GetFunctionPointerForDelegate(Proc),
                Instance = GetModuleHandle(null),
                Cursor = LoadCursor(IntPtr.Zero, IdcArrow),
                ClassName = "KuroNativeCapsule",
            };
            RegisterClass(ref cls);
            _classReady = true;
        }

        var place = WindowChrome.Place(_width, _height, _bottomInset);
        _hwnd = CreateWindowEx(
            WsExTopmost | WsExToolwindow | WsExNoactivate | WsExLayered,
            "KuroNativeCapsule",
            "Kuro",
            WsPopup,
            place.X,
            place.Y,
            _width,
            _height,
            IntPtr.Zero,
            IntPtr.Zero,
            GetModuleHandle(null),
            IntPtr.Zero);
    }

    private void Draw(double[] heights)
    {
        var word = ShowsWord();
        var label = word ? AnimatedCaption() : _caption;
        if (word)
        {
            _phase += 0.14;
            _capsuleWidth = Math.Max(_barCapsuleWidth - 24, MeasureCaptionWidth(label) + (PillPadX * 2));
        }
        else
        {
            _capsuleWidth = _barCapsuleWidth;
        }

        _width = _circle + _gap + _capsuleWidth + (_collapseVisible ? _gap + _circle : 0);
        Reposition();

        var pixels = new byte[_width * _height * 4];
        FillCircle(pixels, _circle / 2, _height / 2, (_circle / 2) - 1);
        FillRoundRect(pixels, _circle + _gap, 0, _capsuleWidth, _height, _height / 2);
        DrawCross(pixels, _circle / 2, _height / 2, 6);
        if (_collapseVisible)
        {
            var cx = _circle + _gap + _capsuleWidth + _gap + (_circle / 2);
            // Same solid black disc as the X control.
            FillCircle(pixels, cx, _height / 2, (_circle / 2) - 1);
            // Point up to open the card; point down while the card is open (collapse).
            if (_collapsePointsDown)
            {
                DrawChevronDown(pixels, cx, _height / 2, 8);
            }
            else
            {
                DrawChevronUp(pixels, cx, _height / 2, 8);
            }
        }

        var screen = GetDC(IntPtr.Zero);
        var mem = CreateCompatibleDC(screen);
        var info = new BitmapInfo();
        info.Size = 40;
        info.Width = _width;
        info.Height = -_height;
        info.Planes = 1;
        info.BitCount = 32;
        var dib = CreateDIBSection(screen, ref info, 0, out var bits, IntPtr.Zero, 0);
        Marshal.Copy(pixels, 0, bits, pixels.Length);
        var old = SelectObject(mem, dib);

        if (word)
        {
            DrawCaption(mem, label);
            OpaqueizeText(bits, pixels);
            if (IsThinkingCaption())
            {
                ApplyThinkingShimmer(bits);
            }
        }
        else
        {
            DrawBars(pixels, heights);
            Marshal.Copy(pixels, 0, bits, pixels.Length);
        }

        var origin = new Point();
        var size = new Size { CX = _width, CY = _height };
        var blend = new Blend { Op = 0, Flags = 0, Alpha = 255, Format = 1 };
        UpdateLayeredWindow(_hwnd, screen, IntPtr.Zero, ref size, mem, ref origin, 0, ref blend, UlwAlpha);
        SelectObject(mem, old);
        DeleteObject(dib);
        DeleteDC(mem);
        ReleaseDC(IntPtr.Zero, screen);
    }

    private void Reposition()
    {
        if (_hwnd == IntPtr.Zero)
        {
            return;
        }

        var place = WindowChrome.Place(_width, _height, _bottomInset);
        SetWindowPos(_hwnd, HwndTopmost, place.X, place.Y, _width, _height, SwpNoactivate | SwpShowwindow);
    }

    private bool IsThinkingCaption()
    {
        return _mode is "thinking" or "processing"
            || string.Equals(_caption, "Thinking", StringComparison.OrdinalIgnoreCase);
    }

    private string AnimatedCaption()
    {
        if (!IsThinkingCaption())
        {
            return string.IsNullOrWhiteSpace(_caption) ? "" : _caption;
        }

        return "Thinking";
    }

    private int MeasureCaptionWidth(string text)
    {
        EnsureFont();
        var screen = GetDC(IntPtr.Zero);
        var font = CreateFont(-16, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 5, 0, "Poppins");
        if (font == IntPtr.Zero)
        {
            font = CreateFont(-16, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 5, 0, "Segoe UI");
        }

        var old = SelectObject(screen, font);
        var size = new Size();
        GetTextExtentPoint32(screen, text, text.Length, ref size);
        SelectObject(screen, old);
        DeleteObject(font);
        ReleaseDC(IntPtr.Zero, screen);
        return Math.Max(72, size.CX + 18);
    }

    private void ApplyThinkingShimmer(IntPtr bits)
    {
        // Soft band of light sweeps left-to-right across the word glyphs only.
        var left = _circle + _gap + 8;
        var right = _circle + _gap + _capsuleWidth - 8;
        var span = Math.Max(1, right - left);
        var t = (_phase * 0.22) % 1.35;
        var center = left + (int)((t - 0.15) * span);
        const int half = 26;
        var row = new byte[_width * 4];
        for (var y = 0; y < _height; y++)
        {
            Marshal.Copy(IntPtr.Add(bits, y * _width * 4), row, 0, row.Length);
            for (var x = left; x < right; x++)
            {
                var i = x * 4;
                var a = row[i + 3];
                if (a < 40)
                {
                    continue;
                }

                // Glyphs are bright-ish; skip near-black pill fill.
                if (row[i] < 30 && row[i + 1] < 30 && row[i + 2] < 30)
                {
                    continue;
                }

                var dist = Math.Abs(x - center);
                if (dist >= half)
                {
                    continue;
                }

                var boost = (1.0 - (dist / (double)half));
                boost *= boost;
                var lift = (int)(boost * 95);
                row[i] = (byte)Math.Min(255, row[i] + lift);
                row[i + 1] = (byte)Math.Min(255, row[i + 1] + lift);
                row[i + 2] = (byte)Math.Min(255, row[i + 2] + lift);
            }

            Marshal.Copy(row, 0, IntPtr.Add(bits, y * _width * 4), row.Length);
        }
    }

    private bool ShowsWord()
    {
        return !string.IsNullOrWhiteSpace(_caption)
            && (_mode is "thinking" or "processing" or "error" || _caption is "Quiet" or "Hold" or "Thinking");
    }

    private void OpaqueizeText(IntPtr bits, byte[] baseline)
    {
        var painted = new byte[baseline.Length];
        Marshal.Copy(bits, painted, 0, painted.Length);
        for (var i = 0; i < painted.Length; i += 4)
        {
            var b = painted[i];
            var g = painted[i + 1];
            var r = painted[i + 2];
            var a = painted[i + 3];
            if (a != 0)
            {
                continue;
            }

            // GDI text lands as opaque RGB with zero alpha on a layered DIB.
            if (r > 40 || g > 40 || b > 40)
            {
                painted[i + 3] = 255;
            }
        }

        Marshal.Copy(painted, 0, bits, painted.Length);
    }

    private void EnsureFont()
    {
        if (_fontReady)
        {
            return;
        }

        var path = Path.Combine(AppContext.BaseDirectory, "Assets", "Fonts", "Poppins-SemiBold.ttf");
        if (File.Exists(path))
        {
            AddFontResourceEx(path, 0x10, IntPtr.Zero);
        }

        _fontReady = true;
    }

    private void DrawCaption(IntPtr dc, string caption)
    {
        EnsureFont();
        var font = CreateFont(-16, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 5, 0, "Poppins");
        if (font == IntPtr.Zero)
        {
            font = CreateFont(-16, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 5, 0, "Segoe UI");
        }

        var old = SelectObject(dc, font);
        SetBkMode(dc, 1);
        // Slightly soft base; the shimmer band adds the moving highlight.
        var glow = IsThinkingCaption() ? 210u : 240u;
        SetTextColor(dc, glow | (glow << 8) | (glow << 16));
        var rect = new Rect
        {
            Left = _circle + _gap,
            Top = 0,
            Right = _circle + _gap + _capsuleWidth,
            Bottom = _height,
        };
        DrawText(dc, caption, -1, ref rect, 0x00000001 | 0x00000004 | 0x00000020);
        SelectObject(dc, old);
        DeleteObject(font);
    }

    private void FillCircle(byte[] pixels, int cx, int cy, int radius)
    {
        var left = Math.Max(0, cx - radius - 1);
        var right = Math.Min(_width - 1, cx + radius + 1);
        for (var y = 0; y < _height; y++)
        {
            for (var x = left; x <= right; x++)
            {
                var dx = x - cx;
                var dy = y - cy;
                if ((dx * dx) + (dy * dy) <= radius * radius)
                {
                    Put(pixels, x, y, 10, 10, 10, 255);
                }
            }
        }
    }

    private void FillRoundRect(byte[] pixels, int left, int top, int width, int height, int radius, byte red = 10, byte green = 10, byte blue = 10)
    {
        for (var y = 0; y < height; y++)
        {
            for (var x = 0; x < width; x++)
            {
                if (InsideRound(x, y, width, height, radius))
                {
                    Put(pixels, left + x, top + y, red, green, blue, 255);
                }
            }
        }
    }

    private static bool InsideRound(int x, int y, int width, int height, int radius)
    {
        var cx = x < radius ? radius : x >= width - radius ? width - radius - 1 : x;
        var cy = y < radius ? radius : y >= height - radius ? height - radius - 1 : y;
        var dx = x - cx;
        var dy = y - cy;
        return (dx * dx) + (dy * dy) <= radius * radius || (x >= radius && x < width - radius) || (y >= radius && y < height - radius);
    }

    private void DrawCross(byte[] pixels, int cx, int cy, int arm)
    {
        // Soft X: thin diagonal with a light halo instead of a thick hard stroke.
        for (var i = -arm; i <= arm; i++)
        {
            BlendPixel(pixels, cx + i, cy + i, 236, 236, 236, 200);
            BlendPixel(pixels, cx + i, cy - i, 236, 236, 236, 200);
            BlendPixel(pixels, cx + i + 1, cy + i, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i - 1, cy + i, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i, cy + i + 1, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i, cy + i - 1, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i + 1, cy - i, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i - 1, cy - i, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i, cy - i + 1, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i, cy - i - 1, 236, 236, 236, 70);
        }
    }

    private void DrawChevronUp(byte[] pixels, int cx, int cy, int arm)
    {
        // Soft ^ : tip at the top (smaller y), arms open downward.
        for (var i = 0; i <= arm; i++)
        {
            var y = cy - (arm / 2) + i;
            BlendPixel(pixels, cx - i, y, 236, 236, 236, 200);
            BlendPixel(pixels, cx + i, y, 236, 236, 236, 200);
            BlendPixel(pixels, cx - i - 1, y, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i + 1, y, 236, 236, 236, 70);
            BlendPixel(pixels, cx - i, y + 1, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i, y + 1, 236, 236, 236, 70);
        }
    }

    private void DrawChevronDown(byte[] pixels, int cx, int cy, int arm)
    {
        for (var i = 0; i <= arm; i++)
        {
            var y = cy + (arm / 2) - i;
            BlendPixel(pixels, cx - i, y, 236, 236, 236, 200);
            BlendPixel(pixels, cx + i, y, 236, 236, 236, 200);
            BlendPixel(pixels, cx - i - 1, y, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i + 1, y, 236, 236, 236, 70);
            BlendPixel(pixels, cx - i, y - 1, 236, 236, 236, 70);
            BlendPixel(pixels, cx + i, y - 1, 236, 236, 236, 70);
        }
    }

    private void DrawBars(byte[] pixels, double[] heights)
    {
        var total = (BarCount * BarWidth) + ((BarCount - 1) * BarGap);
        var left = _circle + _gap + Math.Max(0, (_capsuleWidth - total) / 2);
        var travel = Math.Max(10, _height - 16);
        _phase += 0.18;
        for (var i = 0; i < BarCount; i++)
        {
            var target = BarFraction(heights, i);
            var follow = _mode == "listening" ? 0.42 : 0.28;
            _smooth[i] = (_smooth[i] * (1 - follow)) + (target * follow);
            var bh = (int)Math.Clamp(travel * _smooth[i], 5, travel);
            if (bh % 2 == 1)
            {
                bh++;
            }

            var x0 = left + (i * (BarWidth + BarGap));
            var y0 = (_height - bh) / 2;
            FillRoundRect(pixels, x0, y0, BarWidth, bh, BarWidth / 2, 245, 245, 245);
        }
    }

    private double BarFraction(double[] heights, int index)
    {
        var mid = Math.Max(1, (BarCount - 1) / 2.0);
        var center = 1 - (Math.Abs(index - mid) / mid);

        if (_mode == "speaking")
        {
            var wave = 0.5 + (0.5 * Math.Sin((_phase * 3.6) - (index * 0.62)));
            return Math.Clamp(0.16 + (0.78 * wave * (0.4 + (0.6 * center))), 0.14, 1);
        }

        if (_mode == "listening")
        {
            var sample = BarLevel(heights, index, BarCount);
            // Quiet rest is a low center arch. Speech lifts bars from the mic shape.
            var rest = 0.1 + (0.22 * center * center);
            if (sample < 0.05)
            {
                return rest;
            }

            return Math.Clamp(rest + (Math.Pow(sample, 0.85) * (0.9 - (rest * 0.35))), 0.1, 1);
        }

        return Math.Clamp(0.1 + (0.22 * center * center), 0.1, 1);
    }

    private static double BarLevel(double[] heights, int index, int count)
    {
        if (heights.Length == 0 || count <= 1)
        {
            return 0;
        }

        var source = index * (heights.Length - 1) / (double)(count - 1);
        var left = (int)Math.Floor(source);
        var right = Math.Min(heights.Length - 1, left + 1);
        var t = source - left;
        var a = NormalizeLevel(heights[left]);
        var b = NormalizeLevel(heights[right]);
        return a + ((b - a) * t);
    }

    private static double NormalizeLevel(double value)
    {
        if (value > 1.5)
        {
            return Math.Clamp(value / 28.0, 0, 1);
        }

        return Math.Clamp(value, 0, 1);
    }

    private void Put(byte[] pixels, int x, int y, byte r, byte g, byte b, byte a)
    {
        if (x < 0 || y < 0 || x >= _width || y >= _height)
        {
            return;
        }

        var i = ((y * _width) + x) * 4;
        pixels[i] = (byte)(b * a / 255);
        pixels[i + 1] = (byte)(g * a / 255);
        pixels[i + 2] = (byte)(r * a / 255);
        pixels[i + 3] = a;
    }

    private void BlendPixel(byte[] pixels, int x, int y, byte r, byte g, byte b, byte a)
    {
        if (x < 0 || y < 0 || x >= _width || y >= _height || a == 0)
        {
            return;
        }

        var i = ((y * _width) + x) * 4;
        var destA = pixels[i + 3];
        if (destA == 0)
        {
            Put(pixels, x, y, r, g, b, a);
            return;
        }

        var inv = 255 - a;
        var outB = ((b * a) + (pixels[i] * inv)) / 255;
        var outG = ((g * a) + (pixels[i + 1] * inv)) / 255;
        var outR = ((r * a) + (pixels[i + 2] * inv)) / 255;
        pixels[i] = (byte)outB;
        pixels[i + 1] = (byte)outG;
        pixels[i + 2] = (byte)outR;
        pixels[i + 3] = 255;
    }

    private bool IsClickableX(int x) => x < _circle;

    private bool IsClickableChevron(int x)
    {
        if (!_collapseVisible)
        {
            return false;
        }

        var chevronLeft = _circle + _gap + _capsuleWidth + _gap;
        return x >= chevronLeft;
    }

    private void OnClick(IntPtr lParam)
    {
        var x = (short)(lParam.ToInt64() & 0xFFFF);
        if (IsClickableX(x))
        {
            DismissRequested?.Invoke();
            return;
        }

        if (IsClickableChevron(x))
        {
            CollapseRequested?.Invoke();
            return;
        }

        CapsuleClicked?.Invoke();
    }

    private void ApplyCursor(int x)
    {
        var hand = IsClickableX(x) || IsClickableChevron(x);
        SetCursor(LoadCursor(IntPtr.Zero, hand ? IdcHand : IdcArrow));
    }

    private static IntPtr WndProc(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam)
    {
        if (msg == WmMousemove)
        {
            var x = (short)(lParam.ToInt64() & 0xFFFF);
            _live?.ApplyCursor(x);
            return IntPtr.Zero;
        }

        if (msg == WmSetCursor)
        {
            var pos = GetMessagePos();
            var pt = new Point { X = (short)(pos & 0xFFFF), Y = (short)((pos >> 16) & 0xFFFF) };
            ScreenToClient(hwnd, ref pt);
            _live?.ApplyCursor(pt.X);
            return new IntPtr(1);
        }

        if (msg == WmLbuttonUp)
        {
            _live?.OnClick(lParam);
            return IntPtr.Zero;
        }

        return DefWindowProc(hwnd, msg, wParam, lParam);
    }

    private delegate IntPtr WndProcDelegate(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct WndClass
    {
        public uint Style;
        public IntPtr Proc;
        public int ClassExtra;
        public int WindowExtra;
        public IntPtr Instance;
        public IntPtr Icon;
        public IntPtr Cursor;
        public IntPtr Background;
        public IntPtr MenuName;
        [MarshalAs(UnmanagedType.LPWStr)]
        public string ClassName;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Point
    {
        public int X;
        public int Y;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Size
    {
        public int CX;
        public int CY;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Blend
    {
        public byte Op;
        public byte Flags;
        public byte Alpha;
        public byte Format;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Rect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct BitmapInfo
    {
        public int Size;
        public int Width;
        public int Height;
        public short Planes;
        public short BitCount;
        public int Compression;
        public int SizeImage;
        public int XPels;
        public int YPels;
        public int ClrUsed;
        public int ClrImportant;
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern ushort RegisterClass(ref WndClass cls);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateWindowEx(
        int exStyle,
        string className,
        string windowName,
        int style,
        int x,
        int y,
        int width,
        int height,
        IntPtr parent,
        IntPtr menu,
        IntPtr instance,
        IntPtr param);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hwnd, int command);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(IntPtr hwnd, IntPtr insertAfter, int x, int y, int cx, int cy, uint flags);

    [DllImport("user32.dll")]
    private static extern IntPtr DefWindowProc(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr LoadCursor(IntPtr instance, int cursor);

    [DllImport("user32.dll")]
    private static extern IntPtr SetCursor(IntPtr cursor);

    [DllImport("user32.dll")]
    private static extern int GetMessagePos();

    [DllImport("user32.dll")]
    private static extern bool ScreenToClient(IntPtr hwnd, ref Point point);

    [DllImport("user32.dll")]
    private static extern IntPtr GetDC(IntPtr hwnd);

    [DllImport("user32.dll")]
    private static extern int ReleaseDC(IntPtr hwnd, IntPtr dc);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int DrawText(IntPtr dc, string text, int count, ref Rect rect, uint format);

    [DllImport("gdi32.dll")]
    private static extern IntPtr CreateCompatibleDC(IntPtr dc);

    [DllImport("gdi32.dll")]
    private static extern bool DeleteDC(IntPtr dc);

    [DllImport("gdi32.dll")]
    private static extern IntPtr SelectObject(IntPtr dc, IntPtr obj);

    [DllImport("gdi32.dll")]
    private static extern bool DeleteObject(IntPtr obj);

    [DllImport("gdi32.dll")]
    private static extern IntPtr CreateDIBSection(IntPtr dc, ref BitmapInfo info, uint usage, out IntPtr bits, IntPtr section, uint offset);

    [DllImport("gdi32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr CreateFont(
        int height,
        int width,
        int escapement,
        int orientation,
        int weight,
        uint italic,
        uint underline,
        uint strike,
        uint charset,
        uint outPrecision,
        uint clipPrecision,
        uint quality,
        uint pitch,
        string face);

    [DllImport("gdi32.dll")]
    private static extern int SetBkMode(IntPtr dc, int mode);

    [DllImport("gdi32.dll")]
    private static extern uint SetTextColor(IntPtr dc, uint color);

    [DllImport("gdi32.dll", CharSet = CharSet.Unicode)]
    private static extern bool GetTextExtentPoint32(IntPtr dc, string text, int count, ref Size size);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UpdateLayeredWindow(
        IntPtr hwnd,
        IntPtr screen,
        IntPtr dest,
        ref Size size,
        IntPtr source,
        ref Point origin,
        uint colorKey,
        ref Blend blend,
        uint flags);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr GetModuleHandle(string? name);

    [DllImport("gdi32.dll", CharSet = CharSet.Unicode)]
    private static extern int AddFontResourceEx(string path, uint flags, IntPtr reserved);
}
