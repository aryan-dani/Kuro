using System.Globalization;
using System.Reflection;
using KuroShell.Core;

namespace KuroShell;

internal static class StartMenuInstall
{
    public static bool EnsureInstalled(string exePath)
    {
        if (string.IsNullOrWhiteSpace(exePath) || !File.Exists(exePath))
        {
            return false;
        }

        var path = StartMenuShortcut.DefaultShortcutPath();
        try
        {
            var dir = Path.GetDirectoryName(path);
            if (!string.IsNullOrEmpty(dir))
            {
                Directory.CreateDirectory(dir);
            }

            var shellType = Type.GetTypeFromProgID("WScript.Shell");
            if (shellType is null)
            {
                return false;
            }

            var shell = Activator.CreateInstance(shellType);
            if (shell is null)
            {
                return false;
            }

            var create = shellType.GetMethod("CreateShortcut", BindingFlags.Instance | BindingFlags.Public);
            if (create is null)
            {
                return false;
            }

            var shortcut = create.Invoke(shell, [path]);
            if (shortcut is null)
            {
                return false;
            }

            var type = shortcut.GetType();
            var culture = CultureInfo.InvariantCulture;
            type.InvokeMember("TargetPath", BindingFlags.SetProperty, null, shortcut, [exePath], culture);
            type.InvokeMember(
                "WorkingDirectory",
                BindingFlags.SetProperty,
                null,
                shortcut,
                [Path.GetDirectoryName(exePath) ?? ""],
                culture);
            type.InvokeMember("Description", BindingFlags.SetProperty, null, shortcut, ["Kuro voice assistant"], culture);
            type.InvokeMember("Save", BindingFlags.InvokeMethod, null, shortcut, null, culture);
            return File.Exists(path);
        }
        catch
        {
            return false;
        }
    }
}
