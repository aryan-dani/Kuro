using System.Runtime.InteropServices;

namespace KuroShell;

internal static class WindowChrome
{
    private const int GwlExstyle = -20;
    private const int WsExToolwindow = 0x00000080;
    private const int WsExNoactivate = 0x08000000;
    private const int WsExTransparent = 0x00000020;
    private static readonly IntPtr HwndTopmost = new(-1);
    private const uint SwpNoactivate = 0x0010;
    private const uint SwpShowwindow = 0x0040;
    private const uint MonitorDefaultToNearest = 2;

    public static bool PerPixelAvailable => false;

    public static void MakeHud(IntPtr hwnd)
    {
        var style = GetWindowLongPtr(hwnd, GwlExstyle).ToInt64();
        style |= WsExToolwindow | WsExNoactivate | WsExTransparent;
        SetWindowLongPtr(hwnd, GwlExstyle, new IntPtr(style));
    }

    public static (int X, int Y, int Width, int Height) Place(int width, int height)
    {
        GetCursorPos(out var cursor);
        var monitor = MonitorFromPoint(cursor, MonitorDefaultToNearest);
        var info = new MonitorInfo { Size = Marshal.SizeOf<MonitorInfo>() };
        if (!GetMonitorInfo(monitor, ref info))
        {
            return (80, 80, width, height);
        }

        var work = info.Work;
        var x = work.Left + Math.Max(0, (work.Right - work.Left - width) / 2);
        var y = work.Bottom - height - (int)DesignTokens.WorkAreaGap;
        return (x, y, width, height);
    }

    public static void ShowTopmost(IntPtr hwnd, int x, int y, int width, int height)
    {
        SetWindowPos(hwnd, HwndTopmost, x, y, width, height, SwpNoactivate | SwpShowwindow);
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
}
