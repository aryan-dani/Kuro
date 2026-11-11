using System.Runtime.InteropServices;

namespace KuroShell;

internal static class WindowChrome
{
    private const int GwlExstyle = -20;
    private const int WsExToolwindow = 0x00000080;
    private const int WsExTopmost = 0x00000008;
    private const int WsExNoactivate = 0x08000000;
    private const int WsExTransparent = 0x00000020;
    private static readonly IntPtr HwndTopmost = new(-1);
    private const uint SwpNoactivate = 0x0010;
    private const uint SwpShowwindow = 0x0040;
    private const uint MonitorDefaultToNearest = 2;
    private const uint MonitorDefaultToPrimary = 1;

    public static bool PerPixelAvailable => false;

    public static void MakeHud(IntPtr hwnd)
    {
        var style = GetWindowLongPtr(hwnd, GwlExstyle).ToInt64();
        style |= WsExToolwindow | WsExNoactivate | WsExTransparent;
        SetWindowLongPtr(hwnd, GwlExstyle, new IntPtr(style));
    }

    public static void MakeOverlay(IntPtr hwnd)
    {
        var style = GetWindowLongPtr(hwnd, GwlExstyle).ToInt64();
        style |= WsExToolwindow | WsExNoactivate;
        style &= ~WsExTransparent;
        style &= ~0x00200000;
        SetWindowLongPtr(hwnd, GwlExstyle, new IntPtr(style));
        SetWindowRgn(hwnd, IntPtr.Zero, true);
    }

    /// <summary>Topmost tool window that can take focus for scroll and text selection.</summary>
    public static void MakeInteractiveOverlay(IntPtr hwnd)
    {
        var style = GetWindowLongPtr(hwnd, GwlExstyle).ToInt64();
        style |= WsExToolwindow;
        style &= ~WsExNoactivate;
        style &= ~WsExTransparent;
        style &= ~0x00200000;
        SetWindowLongPtr(hwnd, GwlExstyle, new IntPtr(style));
    }

    public static void ShowTopmost(IntPtr hwnd, int x, int y, int width, int height, bool activate)
    {
        var flags = SwpShowwindow | (activate ? 0u : SwpNoactivate);
        SetWindowPos(hwnd, HwndTopmost, x, y, width, height, flags);
        if (activate)
        {
            BringWindowToTop(hwnd);
        }
    }

    public static double DpiScale(IntPtr hwnd)
    {
        var dpi = GetDpiForWindow(hwnd);
        if (dpi < 96)
        {
            return 1;
        }

        return dpi / 96.0;
    }

    public static void ClipToCluster(
        IntPtr hwnd,
        int windowWidth,
        int circle,
        int gap,
        int capsuleWidth,
        int dockY,
        int dockWidth,
        int dockHeight,
        int dockRadius)
    {
        if (circle < 8 || capsuleWidth < 8)
        {
            return;
        }

        var row = circle + gap + capsuleWidth;
        var origin = Math.Max(0, (windowWidth - row) / 2);
        var cluster = CreateEllipticRgn(origin, 0, origin + circle + 1, circle + 1);
        var capsuleLeft = origin + circle + gap;
        var capsule = CreateRoundRectRgn(
            capsuleLeft,
            0,
            capsuleLeft + capsuleWidth + 1,
            circle + 1,
            circle,
            circle);
        CombineRgn(cluster, cluster, capsule, 2);
        DeleteObject(capsule);
        if (dockHeight > 2 && dockWidth > 2)
        {
            var dockLeft = Math.Max(0, (windowWidth - dockWidth) / 2);
            var dock = CreateRoundRectRgn(
                dockLeft,
                dockY,
                dockLeft + dockWidth + 1,
                dockY + dockHeight + 1,
                dockRadius,
                dockRadius);
            CombineRgn(cluster, cluster, dock, 2);
            DeleteObject(dock);
        }

        SetWindowRgn(hwnd, cluster, true);
    }

    public static void ClipRoundRect(IntPtr hwnd, int width, int height, int radius)
    {
        if (width < 8 || height < 8)
        {
            return;
        }

        var r = Math.Clamp(radius, 0, Math.Min(width, height) / 2);
        var region = CreateRoundRectRgn(0, 0, width + 1, height + 1, r * 2, r * 2);
        SetWindowRgn(hwnd, region, true);
    }

    public static (int X, int Y, int Width, int Height) Place(int width, int height, int gapPixels)
    {
        // Prefer the primary display so the capsule stays screen-center, not under the cursor.
        var monitor = MonitorFromPoint(default, MonitorDefaultToPrimary);
        var info = new MonitorInfo { Size = Marshal.SizeOf<MonitorInfo>() };
        if (!GetMonitorInfo(monitor, ref info))
        {
            GetCursorPos(out var cursor);
            monitor = MonitorFromPoint(cursor, MonitorDefaultToNearest);
            info = new MonitorInfo { Size = Marshal.SizeOf<MonitorInfo>() };
            if (!GetMonitorInfo(monitor, ref info))
            {
                return (80, 80, width, height);
            }
        }

        var work = info.Work;
        var area = work.Right - work.Left;
        // Circle sits left of a wider pill, so a small left nudge keeps the pair optically centered.
        var optical = Math.Clamp((width / 18), 0, 16);
        var x = work.Left + Math.Max(0, (area - width) / 2) - optical;
        var y = work.Bottom - height - gapPixels;
        return (x, y, width, height);
    }

    public static (int X, int Y) WorkCenter()
    {
        GetCursorPos(out var cursor);
        var monitor = MonitorFromPoint(cursor, MonitorDefaultToNearest);
        var info = new MonitorInfo { Size = Marshal.SizeOf<MonitorInfo>() };
        if (!GetMonitorInfo(monitor, ref info))
        {
            return (640, 360);
        }

        var work = info.Work;
        return ((work.Left + work.Right) / 2, (work.Top + work.Bottom) / 2);
    }

    public static void ShowTopmost(IntPtr hwnd, int x, int y, int width, int height)
    {
        SetWindowPos(hwnd, IntPtr.Zero, x, y, width, height, SwpNoactivate | SwpShowwindow);
        BringWindowToTop(hwnd);
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Point
    {
        public int X;
        public int Y;
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
    private struct MonitorInfo
    {
        public int Size;
        public Rect Monitor;
        public Rect Work;
        public uint Flags;
    }

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hwnd);

    [DllImport("gdi32.dll")]
    private static extern IntPtr CreateRoundRectRgn(int left, int top, int right, int bottom, int widthEllipse, int heightEllipse);

    [DllImport("gdi32.dll")]
    private static extern IntPtr CreateEllipticRgn(int left, int top, int right, int bottom);

    [DllImport("gdi32.dll")]
    private static extern int CombineRgn(IntPtr dest, IntPtr src1, IntPtr src2, int mode);

    [DllImport("gdi32.dll")]
    private static extern bool DeleteObject(IntPtr obj);

    [DllImport("user32.dll")]
    private static extern int SetWindowRgn(IntPtr hwnd, IntPtr region, bool redraw);

    [DllImport("user32.dll")]
    private static extern bool GetCursorPos(out Point point);

    [DllImport("user32.dll")]
    private static extern IntPtr MonitorFromPoint(Point point, uint flags);

    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    private static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW", SetLastError = true)]
    private static extern IntPtr GetWindowLongPtr(IntPtr hwnd, int index);

    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW", SetLastError = true)]
    private static extern IntPtr SetWindowLongPtr(IntPtr hwnd, int index, IntPtr value);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(IntPtr hwnd, IntPtr insertAfter, int x, int y, int cx, int cy, uint flags);

    [DllImport("user32.dll")]
    private static extern bool BringWindowToTop(IntPtr hwnd);
}
