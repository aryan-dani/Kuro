using System.Diagnostics;
using System.Runtime.InteropServices;

namespace KuroShell;

/// <summary>
/// Low-level Escape hook. While the capsule is visible, Escape dismisses Kuro and
/// is not delivered to the focused app. While hidden, Escape passes through.
/// </summary>
internal sealed class EscapeHook : IDisposable
{
    private const int WhKeyboardLl = 13;
    private const int WmKeydown = 0x0100;
    private const int WmSyskeydown = 0x0104;
    private const int VkEscape = 0x1B;

    private readonly Func<bool> _active;
    private readonly Action _onEscape;
    private readonly LowLevelProc _proc;
    private IntPtr _hook;

    public EscapeHook(Func<bool> active, Action onEscape)
    {
        _active = active;
        _onEscape = onEscape;
        _proc = Hook;
        using var process = Process.GetCurrentProcess();
        using var module = process.MainModule!;
        _hook = SetWindowsHookEx(WhKeyboardLl, _proc, GetModuleHandle(module.ModuleName), 0);
    }

    public void Dispose()
    {
        if (_hook != IntPtr.Zero)
        {
            UnhookWindowsHookEx(_hook);
            _hook = IntPtr.Zero;
        }
    }

    private IntPtr Hook(int code, IntPtr wParam, IntPtr lParam)
    {
        if (code >= 0)
        {
            var msg = wParam.ToInt32();
            if (msg is WmKeydown or WmSyskeydown)
            {
                var info = Marshal.PtrToStructure<KbdLlHookStruct>(lParam);
                if (info.VkCode == VkEscape && _active())
                {
                    try
                    {
                        _onEscape();
                    }
                    catch
                    {
                        // Never let a dismiss failure wedge the keyboard.
                    }

                    return (IntPtr)1;
                }
            }
        }

        return CallNextHookEx(_hook, code, wParam, lParam);
    }

    private delegate IntPtr LowLevelProc(int code, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    private struct KbdLlHookStruct
    {
        public int VkCode;
        public int ScanCode;
        public int Flags;
        public int Time;
        public IntPtr ExtraInfo;
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern IntPtr SetWindowsHookEx(int idHook, LowLevelProc proc, IntPtr module, uint thread);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnhookWindowsHookEx(IntPtr hook);

    [DllImport("user32.dll")]
    private static extern IntPtr CallNextHookEx(IntPtr hook, int code, IntPtr wParam, IntPtr lParam);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr GetModuleHandle(string? name);
}
