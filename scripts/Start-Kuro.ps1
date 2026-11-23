#Requires -Version 5.1
<#
.SYNOPSIS
  Starts Kuro in the background (tray + hotkey). Safe to run at Windows logon.

.PARAMETER ForceRestart
  Kill every existing KuroShell / kuro-engine, then start a fresh instance.
#>
[CmdletBinding()]
param(
    [switch]$ShowWindow,
    [switch]$ForceRestart
)

$ErrorActionPreference = "Stop"

$KuroRoot = Split-Path -Parent $PSScriptRoot

$LogDir   = Join-Path $env:LOCALAPPDATA "Kuro"
$LogFile  = Join-Path $LogDir "autostart.log"
$DisableFlag = Join-Path $LogDir "DISABLED"

function Write-KuroLog {
    param([string]$Message)
    if (-not (Test-Path -LiteralPath $LogDir)) {
        New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
    }
    $stamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Add-Content -LiteralPath $LogFile -Value ("[{0}] {1}" -f $stamp, $Message)
}

# Permanent pause: %LOCALAPPDATA%\Kuro\DISABLED means never autostart.
# -ForceRestart clears the flag so one command re-enables Kuro.
if (Test-Path -LiteralPath $DisableFlag) {
    if ($ForceRestart) {
        Remove-Item -LiteralPath $DisableFlag -Force -ErrorAction SilentlyContinue
        Write-KuroLog "Cleared DISABLED flag (ForceRestart re-enable)."
    }
    else {
        Write-KuroLog "Kuro DISABLED. Refusing start (Start-Kuro.ps1 -ForceRestart to re-enable)."
        return
    }
}

function Get-KuroProcesses {
    Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            ($_.Name -eq "kuro-engine.exe") -or
            ($_.Name -eq "KuroShell.exe")
        }
}

function Find-KuroShell {
    $published = Join-Path $env:LOCALAPPDATA "Kuro\shell\KuroShell.exe"
    if (Test-Path -LiteralPath $published) {
        return $published
    }
    $binRoot = Join-Path $KuroRoot "KuroShell\KuroShell\bin"
    if (-not (Test-Path -LiteralPath $binRoot)) {
        return $null
    }
    $built = @(Get-ChildItem -LiteralPath $binRoot -Recurse -Filter "KuroShell.exe" -ErrorAction SilentlyContinue) |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if ($built) { return $built.FullName }
    return $null
}

function Publish-KuroShell {
    $binRoot = Join-Path $KuroRoot "KuroShell\KuroShell\bin"
    if (-not (Test-Path -LiteralPath $binRoot)) {
        return
    }
    # Any configuration (Release/Debug, with or without x64): pick the newest build.
    $candidates = @(Get-ChildItem -LiteralPath $binRoot -Recurse -Filter "KuroShell.exe" -ErrorAction SilentlyContinue)
    if (-not $candidates) {
        return
    }
    $built = $candidates | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
    $destDir = Join-Path $env:LOCALAPPDATA "Kuro\shell"
    try {
        New-Item -ItemType Directory -Force -Path $destDir | Out-Null
        $srcDir = $built.DirectoryName
        $copied = 0
        $skipped = 0
        Get-ChildItem -LiteralPath $srcDir -Recurse -File | ForEach-Object {
            $rel = $_.FullName.Substring($srcDir.Length).TrimStart('\')
            $target = Join-Path $destDir $rel
            $folder = Split-Path -Parent $target
            if (-not (Test-Path -LiteralPath $folder)) {
                New-Item -ItemType Directory -Force -Path $folder | Out-Null
            }
            try {
                Copy-Item -LiteralPath $_.FullName -Destination $target -Force -ErrorAction Stop
                $copied++
            }
            catch {
                $skipped++
            }
        }
        Write-KuroLog ("Published KuroShell from {0} (copied={1} skipped={2})" -f $srcDir, $copied, $skipped)
        Install-KuroStartMenuShortcut (Join-Path $destDir "KuroShell.exe")
    }
    catch {
        Write-KuroLog ("Publish KuroShell skipped: {0}" -f $_.Exception.Message)
    }
}

function Install-KuroStartMenuShortcut {
    param([Parameter(Mandatory = $true)][string]$TargetExe)
    if (-not (Test-Path -LiteralPath $TargetExe)) { return }
    try {
        $programs = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
        New-Item -ItemType Directory -Force -Path $programs | Out-Null
        $lnk = Join-Path $programs "Kuro.lnk"
        $w = New-Object -ComObject WScript.Shell
        $s = $w.CreateShortcut($lnk)
        $s.TargetPath = $TargetExe
        $s.WorkingDirectory = Split-Path -Parent $TargetExe
        $s.Description = "Kuro voice assistant"
        $s.Save()
        Write-KuroLog ("Start Menu shortcut: {0}" -f $lnk)
    }
    catch {
        Write-KuroLog ("Start Menu shortcut skipped: {0}" -f $_.Exception.Message)
    }
}

function Test-KuroMutexHeld {
    $m = $null
    try {
        $m = New-Object System.Threading.Mutex($false, "Global\KuroAssistantSingleton.v1")
        try {
            $got = $m.WaitOne(0)
        }
        catch [System.Threading.AbandonedMutexException] {
            try { $m.ReleaseMutex() } catch { }
            return $false
        }
        if ($got) {
            try { $m.ReleaseMutex() } catch { }
            return $false
        }
        return $true
    }
    catch {
        return $false
    }
    finally {
        if ($m) { $m.Dispose() }
    }
}

function Stop-KuroProcesses {
    $procs = @(Get-KuroProcesses)
    foreach ($p in $procs) {
        Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
    }
    $lock = Join-Path $LogDir "kuro.lock"
    if (Test-Path -LiteralPath $lock) {
        Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
    }
    if ($procs.Count -gt 0) {
        Start-Sleep -Milliseconds 400
    }
    return $procs.Count
}

function Test-OllamaServing {
    try {
        $resp = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:11434/api/tags" -TimeoutSec 1
        return $resp.StatusCode -ge 200 -and $resp.StatusCode -lt 500
    }
    catch {
        return $false
    }
}

function Start-OllamaBackend {
    $ollama = Join-Path $env:LOCALAPPDATA "Programs\Ollama\ollama.exe"
    if (-not (Test-Path -LiteralPath $ollama)) {
        $cmd = Get-Command ollama -ErrorAction SilentlyContinue
        if ($cmd) { $ollama = $cmd.Source }
    }
    if (-not (Test-Path -LiteralPath $ollama)) {
        Write-KuroLog "Ollama not installed. Skip backend start"
        return
    }

    if (Test-OllamaServing) {
        Write-KuroLog "Ollama already serving on :11434"
    }
    else {
        Write-KuroLog ("Starting Ollama serve via '{0}'" -f $ollama)
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $ollama
        $psi.Arguments = "serve"
        $psi.CreateNoWindow = $true
        $psi.UseShellExecute = $false
        $psi.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
        [void][System.Diagnostics.Process]::Start($psi)
        $ok = $false
        for ($i = 0; $i -lt 20; $i++) {
            Start-Sleep -Milliseconds 500
            if (Test-OllamaServing) { $ok = $true; break }
        }
        if ($ok) {
            Write-KuroLog "Ollama serve is up"
        }
        else {
            Write-KuroLog "WARNING: Ollama serve did not respond on :11434 yet"
        }
    }

    $llmMode = "fallback"
    $model = "gemma4:e4b"
    try {
        $envPath = Join-Path $KuroRoot ".env"
        if (Test-Path -LiteralPath $envPath) {
            foreach ($line in Get-Content -LiteralPath $envPath) {
                if ($line -match '^\s*KURO_LLM_MODE\s*=\s*"?([^"#]+)"?') { $llmMode = $Matches[1].Trim().ToLower() }
                if ($line -match '^\s*OLLAMA_MODEL\s*=\s*"?([^"#]+)"?') { $model = $Matches[1].Trim() }
            }
        }
    }
    catch { }

    if ($llmMode -eq "always") {
        try {
            $wc = New-Object System.Net.WebClient
            $wc.Headers.Add("Content-Type", "application/json")
            $body = '{"model":"' + $model + '","keep_alive":"60m"}'
            $wc.UploadStringAsync((New-Object System.Uri("http://127.0.0.1:11434/api/generate")), "POST", $body)
            Write-KuroLog ("Ollama serve ready; async model warm sent for {0}" -f $model)
        }
        catch {
            Write-KuroLog ("Model warm request skipped: {0}" -f $_.Exception.Message)
        }
    }
    else {
        Write-KuroLog ("Ollama serve ready; model warm skipped (KURO_LLM_MODE={0})" -f $llmMode)
    }
}

function Publish-KuroEngine {
    # Prefer newest among release, debug, pending, and LocalAppData. Never stomp a newer
    # LocalAppData/debug engine with a stale release binary (that made Restart look like no change).
    $engineCandidates = @(
        (Join-Path $KuroRoot "kuro-rs\target\release\kuro-engine.exe"),
        (Join-Path $KuroRoot "kuro-rs\target\debug\kuro-engine.exe"),
        (Join-Path $env:LOCALAPPDATA "Kuro\kuro-engine.next.exe"),
        (Join-Path $env:LOCALAPPDATA "Kuro\kuro-engine.exe")
    ) | Where-Object { Test-Path -LiteralPath $_ }
    $best = $engineCandidates |
        Sort-Object { (Get-Item -LiteralPath $_).LastWriteTimeUtc } -Descending |
        Select-Object -First 1
    if (-not $best) {
        throw "kuro-engine.exe not found. Build with scripts\Build-KuroNative.ps1 first."
    }
    $engineDest = Join-Path $env:LOCALAPPDATA "Kuro\kuro-engine.exe"
    $pendingDest = Join-Path $env:LOCALAPPDATA "Kuro\kuro-engine.next.exe"
    if ($best -eq $engineDest) {
        return $engineDest
    }
    try {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $engineDest) | Out-Null
        Copy-Item -LiteralPath $best -Destination $engineDest -Force
        if (($best -eq $pendingDest) -and (Test-Path -LiteralPath $pendingDest)) {
            Remove-Item -LiteralPath $pendingDest -Force -ErrorAction SilentlyContinue
        }
        Write-KuroLog ("Published engine to {0} from {1}" -f $engineDest, $best)
        return $engineDest
    }
    catch {
        if ($best -ne $pendingDest) {
            try {
                Copy-Item -LiteralPath $best -Destination $pendingDest -Force
                Write-KuroLog ("Staged engine next at {0} ({1})" -f $pendingDest, $_.Exception.Message)
            }
            catch {
                Write-KuroLog ("Engine publish skipped: {0}" -f $_.Exception.Message)
            }
        }
        else {
            Write-KuroLog ("Engine dest locked; leaving staged next at {0}" -f $pendingDest)
        }
        return $best
    }
}

try {
    Start-OllamaBackend

    # Stop before publish so LocalAppData shell/engine files are not locked mid-copy.
    $existing = @(Get-KuroProcesses)
    $shellLive = @($existing | Where-Object { $_.Name -eq "KuroShell.exe" }).Count -gt 0
    $engineLive = @($existing | Where-Object { $_.Name -eq "kuro-engine.exe" }).Count -gt 0
    if ($ForceRestart) {
        $n = Stop-KuroProcesses
        Write-KuroLog ("ForceRestart cleared {0} process(es)" -f $n)
        Start-Sleep -Milliseconds 600
        $shellLive = $false
        $engineLive = $false
    }

    Publish-KuroShell
    $Engine = Publish-KuroEngine

    $Shell = Find-KuroShell
    if ($Shell) {
        $exe = $Shell
        Install-KuroStartMenuShortcut $exe
        Write-KuroLog ("Starting KuroShell '{0}' (supervises kuro-engine)" -f $exe)
    }
    else {
        $exe = $Engine
        $env:KURO_API = "on"
        $env:KURO_HUD_SHOW_TEXT = "1"
        Write-KuroLog ("Starting kuro-engine '{0}' (no KuroShell build found)" -f $exe)
    }

    if (-not $ForceRestart) {
        if ($Shell -and $shellLive -and $engineLive) {
            Write-KuroLog ("Kuro already running (shell+engine) - skip start.")
            return
        }
        elseif (-not $Shell -and ((Test-KuroMutexHeld) -or $engineLive -or ($existing.Count -gt 0))) {
            $pidShown = if ($existing.Count -gt 0) { $existing[0].ProcessId } else { "mutex" }
            Write-KuroLog ("Kuro already running ({0}) - skip start." -f $pidShown)
            return
        }
        elseif ($Shell -and $shellLive -and -not $engineLive) {
            Write-KuroLog "KuroShell is up but engine is down - leaving shell to relaunch engine."
            return
        }
    }

    if (-not $engineLive) {
        $lock = Join-Path $LogDir "kuro.lock"
        if (Test-Path -LiteralPath $lock) {
            Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
        }
    }

    if ($env:ONLY_WHEN_CHARGING) {
        Remove-Item Env:ONLY_WHEN_CHARGING -ErrorAction SilentlyContinue
    }

    Write-KuroLog ("Starting Kuro with '{0}' from '{1}'" -f $exe, $KuroRoot)

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.WorkingDirectory = if ($Shell) { Split-Path -Parent $exe } else { $KuroRoot }
    $psi.UseShellExecute = $false
    if (-not $ShowWindow) {
        $psi.CreateNoWindow = $true
        $psi.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
    }
    $proc = [System.Diagnostics.Process]::Start($psi)
    Write-KuroLog ("Kuro started. PID={0}" -f $proc.Id)
    return
}
catch {
    $msg = $_.Exception.Message
    try { Write-KuroLog ("ERROR: {0}" -f $msg) } catch { }
    Write-Error $msg
    exit 1
}
