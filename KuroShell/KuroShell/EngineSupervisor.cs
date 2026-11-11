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

    public void EnsureRunning()
    {
        if (MutexExists() || _process is { HasExited: false })
        {
            return;
        }

        Attached = false;
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

    public static string? FindEngine(string repoRoot)
    {
        var local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        string[] candidates =
        [
            Path.Combine(repoRoot, "kuro-rs", "target", "release", "kuro-engine.exe"),
            Path.Combine(local, "Kuro", "kuro-engine.exe"),
            Path.Combine(repoRoot, "kuro-rs", "target", "debug", "kuro-engine.exe"),
        ];
        foreach (var candidate in candidates)
        {
            if (File.Exists(candidate))
            {
                return candidate;
            }
        }

        return null;
    }

    private void Launch()
    {
        if (RepoRoot is null)
        {
            Status = "repo not found";
            Changed?.Invoke();
            return;
        }

        var legacy = string.Equals(Environment.GetEnvironmentVariable("KURO_LEGACY_PYTHON"), "1", StringComparison.Ordinal);
        var engine = legacy ? null : FindEngine(RepoRoot);
        ProcessStartInfo info;
        if (engine is not null)
        {
            info = new ProcessStartInfo
            {
                FileName = engine,
                WorkingDirectory = RepoRoot,
                CreateNoWindow = true,
                UseShellExecute = false,
            };
        }
        else
        {
            var python = Path.Combine(RepoRoot, ".venv", "Scripts", "python.exe");
            if (!File.Exists(python))
            {
                python = "python";
            }

            info = new ProcessStartInfo
            {
                FileName = python,
                Arguments = "run_kuro.py",
                WorkingDirectory = RepoRoot,
                CreateNoWindow = true,
                UseShellExecute = false,
            };
        }

        info.Environment["KURO_API"] = "on";
        info.Environment["KURO_HUD_SHOW_TEXT"] = "1";
        info.Environment["KURO_TTS_PLAY"] = "1";
        info.Environment["KURO_STT_BACKEND"] = "cpu";
        if (RepoRoot is not null)
        {
            info.Environment["KURO_ROOT"] = RepoRoot;
            var venvPython = Path.Combine(RepoRoot, ".venv", "Scripts", "python.exe");
            if (File.Exists(venvPython))
            {
                info.Environment["KURO_PYTHON"] = venvPython;
            }
        }
        if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("OLLAMA_MODEL")))
        {
            info.Environment["OLLAMA_MODEL"] = "qwen2.5:7b-instruct";
        }
        if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("LIBCLANG_PATH")))
        {
            var portable = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "LLVM-portable",
                "clang+llvm-18.1.8-x86_64-pc-windows-msvc",
                "bin");
            if (File.Exists(Path.Combine(portable, "libclang.dll")))
            {
                info.Environment["LIBCLANG_PATH"] = portable;
            }
        }
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
            if (_stopRequested)
            {
                return;
            }

            // Another launcher (watchdog / second shell) may already own the engine.
            if (MutexExists())
            {
                Attached = true;
                Status = "attached";
                Changed?.Invoke();
                return;
            }

            Launch();
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
