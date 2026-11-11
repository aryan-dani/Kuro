using System.Text.Json;
using KuroShell.Core;

namespace KuroShell;

public static class ConversationLog
{
    private static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true,
    };

    private static bool _loaded;

    public static event Action? Changed;

    public static ConversationBuffer Buffer { get; } = new();

    public static string FilePath
    {
        get
        {
            var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            return Path.Combine(root, "Kuro", "thread.json");
        }
    }

    public static void EnsureLoaded()
    {
        if (_loaded)
        {
            return;
        }

        _loaded = true;
        Buffer.Seed(Read());
    }

    public static void Observe(string you, string kuro)
    {
        EnsureLoaded();
        var at = DateTime.Now.ToString("yyyy-MM-ddTHH:mm:ss", System.Globalization.CultureInfo.InvariantCulture);
        if (!Buffer.Observe(you, kuro, at))
        {
            return;
        }

        Write(Buffer.Turns);
        Changed?.Invoke();
    }

    public static void Clear()
    {
        _loaded = true;
        Buffer.Clear();
        try
        {
            if (File.Exists(FilePath))
            {
                File.Delete(FilePath);
            }
        }
        catch (Exception)
        {
            // The window still clears. The next save replaces the file.
        }

        Changed?.Invoke();
    }

    private static List<Turn> Read()
    {
        try
        {
            if (!File.Exists(FilePath))
            {
                return [];
            }

            var text = File.ReadAllText(FilePath);
            return JsonSerializer.Deserialize<List<Turn>>(text, Json) ?? [];
        }
        catch (Exception)
        {
            return [];
        }
    }

    private static void Write(IReadOnlyList<Turn> turns)
    {
        try
        {
            var path = FilePath;
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            var tmp = path + ".tmp";
            File.WriteAllText(tmp, JsonSerializer.Serialize(turns, Json));
            File.Move(tmp, path, overwrite: true);
        }
        catch (Exception)
        {
            // A failed save must not take down the window.
        }
    }
}
