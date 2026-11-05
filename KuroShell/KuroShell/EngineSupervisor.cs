using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;

namespace KuroShell;

public sealed class EngineSupervisor
{
    private const string MutexName = @"Global\KuroAssistantSingleton.v1";
    private Process? _process;
    private int _failures;
    private bool _stopRequested;

    public string Status { get; private set; } = "stopped";
    public bool Attached { get; private set; }
    public string? RepoRoot { get; private set; }
    public string Token { get; private set; } = "";

    public event Action? Changed;

    public static string? FindRepo(string start)
    {
        var dir = new DirectoryInfo(start);
        for (var i = 0; i < 10 && dir is not null; i++)
        {
            if (File.Exists(Path.Combine(dir.FullName, "run_kuro.py")))
            {
                return dir.FullName;
            }

            dir = dir.Parent;
        }

        return null;
    }

    public static string EnsureToken()
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var path = Path.Combine(root, "Kuro", "api_token");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        if (!File.Exists(path))
        {
            var bytes = RandomNumberGenerator.GetBytes(32);
            File.WriteAllText(path, Convert.ToHexString(bytes).ToLowerInvariant(), new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
        }

        return File.ReadAllText(path, Encoding.UTF8).Trim().TrimStart('\uFEFF');
    }

    public void StartOrAttach()
    {
        Token = EnsureToken();
        RepoRoot = FindRepo(AppContext.BaseDirectory) ?? FindRepo(Environment.CurrentDirectory);
        if (MutexExists())
        {
            Attached = true;
            Status = "attached";
            Changed?.Invoke();
            return;
        }

        Launch();
    }

    public void Restart()
    {
        _stopRequested = false;
        if (_process is { HasExited: false })
        {
            try
            {
                _process.Kill(entireProcessTree: true);
            }
            catch (Exception)
            {
                Status = "restart failed";
                Changed?.Invoke();
                return;
            }
        }

        _failures = Math.Max(_failures, 1);
        var delay = KuroShell.Core.RestartBackoff.NextDelayMs(_failures);
        Status = "restarting";
        Changed?.Invoke();
        Thread.Sleep(delay);
        Attached = false;
        Launch();
    }

    public async Task<bool> StopEngine(EngineClient client)
    {
        _stopRequested = true;
        var sent = await client.SendAsync(HttpMethod.Post, "/shutdown", "{}");
        if (_process is { HasExited: false })
        {
            try
            {
                if (!_process.WaitForExit(1500))
                {
                    _process.Kill(entireProcessTree: true);
                }
            }
            catch (Exception)
            {
                return sent;
            }
        }

        Status = "stopped";
        Changed?.Invoke();
        return sent;
    }

    private void Launch()
    {
        if (RepoRoot is null)
        {
            Status = "repo not found";
            Changed?.Invoke();
            return;
        }

        var python = Path.Combine(RepoRoot, ".venv", "Scripts", "python.exe");
        if (!File.Exists(python))
        {
            python = "python";
        }

        var info = new ProcessStartInfo
        {
            FileName = python,
            Arguments = "run_kuro.py",
            WorkingDirectory = RepoRoot,
            CreateNoWindow = true,
            UseShellExecute = false,
        };
        info.Environment["KURO_API"] = "on";
        info.Environment["KURO_HUD_SHOW_TEXT"] = "1";
        try
        {
            _process = Process.Start(info);
        }
        catch (Exception)
        {
            _failures++;
            Status = "launch failed";
            Changed?.Invoke();
            return;
        }

        if (_process is null)
        {
            Status = "launch failed";
            Changed?.Invoke();
            return;
        }

        _process.EnableRaisingEvents = true;
        _process.Exited += (_, _) =>
        {
            if (_stopRequested)
            {
                Status = "stopped";
                Changed?.Invoke();
                return;
            }

            _failures++;
            Status = "crashed";
            Changed?.Invoke();
            var delay = KuroShell.Core.RestartBackoff.NextDelayMs(_failures);
            Thread.Sleep(delay);
            if (!_stopRequested)
            {
                Launch();
            }
        };
        _failures = 0;
        Attached = false;
        Status = "running";
        Changed?.Invoke();
    }

    private static bool MutexExists()
    {
        try
        {
            using var mutex = Mutex.OpenExisting(MutexName);
            return true;
        }
        catch (WaitHandleCannotBeOpenedException)
        {
            return false;
        }
        catch (UnauthorizedAccessException)
        {
            return true;
        }
    }
}
