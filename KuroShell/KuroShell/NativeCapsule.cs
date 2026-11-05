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
    private const uint UlwAlpha = 2;

    private static readonly IntPtr HwndTopmost = new(-1);
    private static NativeCapsule? _live;
    private static readonly WndProcDelegate Proc = WndProc;
    private static bool _classReady;

    private readonly int _height;
    private readonly int _close;
    private readonly int _gap;
    private readonly int _mark;
    private readonly int _width;
    private IntPtr _hwnd;
    private bool _listening;
    private bool _processing;
    private bool _speaking;

    public NativeCapsule()
    {
        _mark = 64;
        _close = _mark;
        _gap = 12;
        _height = _mark;
        _width = _close + _gap + _mark;
    }

    public void SetMode(string state)
    {
        _listening = state == "listening";
        _processing = state is "thinking" or "processing";
        _speaking = state == "speaking";
    }

    public Action? DismissRequested { get; set; }

    public bool Visible { get; private set; }

    public void Show()
    {
        Ensure();
        if (Visible)
        {
            return;
        }

        var place = WindowChrome.Place(_width, _height, 36);
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
                ClassName = "KuroNativeCapsule",
            };
            RegisterClass(ref cls);
            _classReady = true;
        }

        var place = WindowChrome.Place(_width, _height, 36);
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
        var pixels = new byte[_width * _height * 4];
        var paper = _listening ? (byte)20 : (byte)245;
        var paperG = _listening ? (byte)20 : (byte)244;
        var paperB = _listening ? (byte)20 : (byte)238;
        var ink = _listening ? (byte)245 : (byte)20;
        var inkG = _listening ? (byte)244 : (byte)20;
        var inkB = _listening ? (byte)238 : (byte)20;
        FillRect(pixels, 0, 0, _close, _height, 20, 20, 20);
        DrawCross(pixels, _close / 2, _height / 2, 8, 245, 244, 238);
        DrawMark(pixels, _close + _gap, paper, paperG, paperB, ink, inkG, inkB, heights);

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
        var origin = new Point();
        var size = new Size { CX = _width, CY = _height };
        var blend = new Blend { Op = 0, Flags = 0, Alpha = 255, Format = 1 };
        UpdateLayeredWindow(_hwnd, screen, IntPtr.Zero, ref size, mem, ref origin, 0, ref blend, UlwAlpha);
        SelectObject(mem, old);
        DeleteObject(dib);
        DeleteDC(mem);
        ReleaseDC(IntPtr.Zero, screen);
    }

    private void FillRect(byte[] pixels, int left, int top, int width, int height, byte red, byte green, byte blue)
    {
        for (var y = 0; y < height; y++)
        {
            for (var x = 0; x < width; x++)
            {
                Put(pixels, left + x, top + y, red, green, blue, 255);
            }
        }
    }

    private void StrokeRect(byte[] pixels, int left, int top, int width, int height, int stroke, byte red, byte green, byte blue)
    {
        for (var y = 0; y < height; y++)
        {
            for (var x = 0; x < width; x++)
            {
                if (x < stroke || y < stroke || x >= width - stroke || y >= height - stroke)
                {
                    Put(pixels, left + x, top + y, red, green, blue, 255);
                }
            }
        }
    }

    private void DrawCross(byte[] pixels, int cx, int cy, int arm, byte red, byte green, byte blue)
    {
        for (var i = -arm; i <= arm; i++)
        {
            Put(pixels, cx + i, cy + i, red, green, blue, 255);
            Put(pixels, cx + i, cy - i, red, green, blue, 255);
            Put(pixels, cx + i + 1, cy + i, red, green, blue, 255);
            Put(pixels, cx + i, cy + i + 1, red, green, blue, 255);
        }
    }

    private void DrawMark(byte[] pixels, int origin, byte paper, byte paperG, byte paperB, byte ink, byte inkG, byte inkB, double[] heights)
    {
        FillRect(pixels, origin, 0, _mark, _mark, paper, paperG, paperB);
        var corner = 14;
        FillRect(pixels, origin, 0, corner, corner, ink, inkG, inkB);
        StrokeRect(pixels, origin + 2, 2, _mark - 4, _mark - 4, 3, ink, inkG, inkB);
        StrokeRect(pixels, origin + 8, 8, _mark - 16, _mark - 16, 2, ink, inkG, inkB);
        DrawBars(pixels, origin, ink, inkG, inkB, heights);
    }

    private void DrawBars(byte[] pixels, int origin, byte red, byte green, byte blue, double[] heights)
    {
        var profile = new[] { 0.22, 0.38, 0.58, 0.72, 0.58, 0.38, 0.22 };
        if (_processing)
        {
            profile = [0.22, 0.38, 0.78, 0.92, 0.78, 0.38, 0.22];
        }

        const int count = 7;
        const int barW = 3;
        const int gap = 2;
        var total = (count * barW) + ((count - 1) * gap);
        var left = origin + Math.Max(0, (_mark - total) / 2);
        var travel = _mark - 22;
        var level = Level(heights);
        for (var i = 0; i < count; i++)
        {
            var fraction = profile[i];
            if (_speaking)
            {
                var center = 1 - (Math.Abs(i - 3) / 3.0);
                fraction = Math.Clamp(0.24 + (level * 0.76 * center), 0.2, 1);
            }

            var bh = (int)Math.Clamp(travel * fraction, 4, travel);
            if (bh % 2 == 1)
            {
                bh++;
            }

            var x0 = left + (i * (barW + gap));
            var y0 = (_mark - bh) / 2;
            FillRect(pixels, x0, y0, barW, bh, red, green, blue);
        }
    }

    private static double Level(double[] heights)
    {
        if (heights.Length == 0)
        {
            return 0.35;
        }

        var sum = 0.0;
        foreach (var height in heights)
        {
            sum += Math.Clamp(height / 28.0, 0, 1);
        }

        return Math.Clamp(sum / heights.Length, 0, 1);
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

    private void OnClick(IntPtr lParam)
    {
        var x = (short)(lParam.ToInt64() & 0xFFFF);
        if (x < _close)
        {
            DismissRequested?.Invoke();
        }
    }

    private static IntPtr WndProc(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam)
    {
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

    [DllImport("user32.dll")]
    private static extern IntPtr GetDC(IntPtr hwnd);

    [DllImport("user32.dll")]
    private static extern int ReleaseDC(IntPtr hwnd, IntPtr dc);

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
}
