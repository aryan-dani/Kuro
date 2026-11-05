using System.Runtime.InteropServices;
using Microsoft.UI.Xaml.Controls;

namespace KuroShell;

internal static class NativeTray
{
    private const int WmUser = 0x0400;
    private const int WmLbuttonup = 0x0202;
    private const int WmRbuttonup = 0x0205;
    private const uint NimAdd = 0x00000000;
    private const uint NifMessage = 0x00000001;
    private const uint NifIcon = 0x00000002;
    private const uint NifTip = 0x00000004;

    private static IntPtr _hwnd;
    private static WndClass _windowClass;
    private static MenuFlyout? _menu;
    private static Action? _leftClick;

    public static void Show(MenuFlyout menu, Action leftClick)
    {
        _menu = menu;
        _leftClick = leftClick;
        var instance = GetModuleHandle(null);
        _windowClass = new WndClass
        {
            ClassName = "KuroShellTray",
            WindowProc = WndProc,
            Instance = instance,
        };
        RegisterClass(ref _windowClass);
        _hwnd = CreateWindowEx(0, _windowClass.ClassName, "Kuro", 0, 0, 0, 0, 0, IntPtr.Zero, IntPtr.Zero, instance, IntPtr.Zero);
        var data = new NotifyIconData
        {
            Size = Marshal.SizeOf<NotifyIconData>(),
            Window = _hwnd,
            Id = 1,
            Flags = NifMessage | NifIcon | NifTip,
            Callback = WmUser,
            Tip = "Kuro",
        };
        Shell_NotifyIcon(NimAdd, ref data);
    }

    private static IntPtr WndProc(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam)
    {
        if (msg == WmUser)
        {
            var mouse = lParam.ToInt32();
            if (mouse == WmLbuttonup)
            {
                _leftClick?.Invoke();
            }
            else if (mouse == WmRbuttonup && _menu is not null)
            {
                _menu.ShowAt(null);
            }
        }

        return DefWindowProc(hwnd, msg, wParam, lParam);
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct WndClass
    {
        public uint Style;
        public WndProcDelegate WindowProc;
        public int ClassExtra;
        public int WindowExtra;
        public IntPtr Instance;
        public IntPtr Icon;
        public IntPtr Cursor;
        public IntPtr Background;
        public string? MenuName;
        public string ClassName;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct NotifyIconData
    {
        public int Size;
        public IntPtr Window;
        public uint Id;
        public uint Flags;
        public uint Callback;
        public IntPtr Icon;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
        public string Tip;
    }

    private delegate IntPtr WndProcDelegate(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern ushort RegisterClass(ref WndClass wc);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr CreateWindowEx(
        int exStyle, string className, string windowName, int style,
        int x, int y, int width, int height,
        IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);

    [DllImport("user32.dll")]
    private static extern IntPtr DefWindowProc(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr GetModuleHandle(string? name);

    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    private static extern bool Shell_NotifyIcon(uint message, ref NotifyIconData data);
}
