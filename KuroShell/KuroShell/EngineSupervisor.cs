using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;
using KuroShell.Core.Supervisor;

namespace KuroShell;

public sealed class EngineSupervisor
{
    private const string MutexName = @"Global\KuroAssistantSingleton.v1";
    private Process? _process;
    private readonly RestartPolicy _restartPolicy = new();
    private bool _stopRequested;
    private bool _manualRestartInProgress;

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
            if (File.Exists(Path.Combine(dir.FullName, "kuro-rs", "Cargo.toml")))
            {
                return dir.FullName;
            }

            dir = dir.Parent;
        }

        return null;
    }

    public static string TokenFilePath()
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return Path.Combine(root, "Kuro", "api_token");
    }

    public static string EnsureToken()
    {
        var path = TokenFilePath();
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
        var envRoot = Environment.GetEnvironmentVariable("KURO_ROOT");
        RepoRoot = FindRepo(AppContext.BaseDirectory)
            ?? FindRepo(Environment.CurrentDirectory)
            ?? (string.IsNullOrWhiteSpace(envRoot) ? null : FindRepo(envRoot));
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
        _manualRestartInProgress = true;
        if (_process is { HasExited: false })
        {
            try
            {
                _process.Kill(entireProcessTree: true);
            }
            catch (Exception)
            {
                _manualRestartInProgress = false;
                Status = "restart failed";
                Changed?.Invoke();
                return;
            }
        }

        var now = UtcNowMs();
        _restartPolicy.OnEngineStopped(now);
        var advice = _restartPolicy.AdviseAfterExit(EngineExitCodes.WatchdogRestart, now);
        Status = "restarting";
        Changed?.Invoke();
        Thread.Sleep(advice.DelayMs);
        _manualRestartInProgress = false;
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

    public static string? FindEngine(string? repoRoot)
    {
        var local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var candidates = new List<string>(3);
        if (!string.IsNullOrWhiteSpace(repoRoot))
        {
            candidates.Add(Path.Combine(repoRoot, "kuro-rs", "target", "release", "kuro-engine.exe"));
        }

        candidates.Add(Path.Combine(local, "Kuro", "kuro-engine.exe"));
        if (!string.IsNullOrWhiteSpace(repoRoot))
        {
            candidates.Add(Path.Combine(repoRoot, "kuro-rs", "target", "debug", "kuro-engine.exe"));
        }

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
        var engine = FindEngine(RepoRoot);
        if (engine is null)
        {
            Status = "engine not found";
            Changed?.Invoke();
            return;
        }

        try
        {
            var lockPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Kuro", "kuro.lock");
            if (File.Exists(lockPath))
            {
                var text = File.ReadAllText(lockPath).Trim();
                if (!int.TryParse(text, out var pid))
                {
                    File.Delete(lockPath);
                }
                else
                {
                    try
                    {
                        var proc = Process.GetProcessById(pid);
                        if (proc.HasExited)
                        {
                            File.Delete(lockPath);
                        }
                    }
                    catch (ArgumentException)
                    {
                        File.Delete(lockPath);
                    }
                }
            }
        }
        catch
        {
            // Ignore lock cleanup errors
        }

        var info = new ProcessStartInfo
        {
            FileName = engine,
            WorkingDirectory = RepoRoot ?? Path.GetDirectoryName(engine) ?? Environment.CurrentDirectory,
            CreateNoWindow = true,
            UseShellExecute = false,
        };

        info.Environment["KURO_API"] = "on";
        info.Environment["KURO_HUD_SHOW_TEXT"] = "1";
        info.Environment["KURO_TTS_PLAY"] = "1";
        info.Environment["KURO_STT_BACKEND"] = "cpu";
        if (RepoRoot is not null)
        {
            info.Environment["KURO_ROOT"] = RepoRoot;
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

            if (_manualRestartInProgress)
            {
                return;
            }

            var exitCode = 0;
            try
            {
                exitCode = _process?.ExitCode ?? 0;
            }
            catch (InvalidOperationException)
            {
                exitCode = 1;
            }

            var now = UtcNowMs();
            _restartPolicy.OnEngineStopped(now);
            var advice = _restartPolicy.AdviseAfterExit(exitCode, now);
            if (!advice.ShouldRestart)
            {
                Status = advice.ExitKind is EngineExitKind.CleanExit or EngineExitKind.InvalidConfig
                    ? "stopped"
                    : "crashed";
                Changed?.Invoke();
                return;
            }

            Status = "crashed";
            Changed?.Invoke();
            Thread.Sleep(advice.DelayMs);
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
        _restartPolicy.OnEngineStarted(UtcNowMs());
        Attached = false;
        Status = "running";
        Changed?.Invoke();
    }

    private static long UtcNowMs() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

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
