#Requires -Version 5.1
<#
.SYNOPSIS
  Starts Kuro in the background (tray + hotkey). Safe to run at Windows logon.

.PARAMETER ForceRestart
  Kill every existing Kuro main.py (any Python) then start a fresh venv instance.
#>
[CmdletBinding()]
param(
    [switch]$ShowWindow,
    [switch]$ForceRestart
)

$ErrorActionPreference = "Stop"

$KuroRoot = Split-Path -Parent $PSScriptRoot
$Pythonw  = Join-Path $KuroRoot ".venv\Scripts\pythonw.exe"
$Python   = Join-Path $KuroRoot ".venv\Scripts\python.exe"
$MainPy   = Join-Path $KuroRoot "legacy\python\run_kuro.py"
if (-not (Test-Path -LiteralPath $MainPy)) {
    $MainPy = Join-Path $KuroRoot "legacy\python\main.py"
}

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
        exit 0
    }
}

function Get-KuroProcesses {
    Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            ($_.Name -eq "kuro-engine.exe") -or
            ($_.Name -eq "KuroShell.exe") -or
            (
                ($_.Name -eq "python.exe" -or $_.Name -eq "pythonw.exe") -and
                $_.CommandLine -and
                ($_.CommandLine -match 'Kuro') -and
                ($_.CommandLine -match '(run_kuro|main)\.py')
            )
        }
}

function Find-KuroShell {
    $candidates = @(
        (Join-Path $KuroRoot "KuroShell\KuroShell\bin\x64\Debug\net10.0-windows10.0.19041.0\win-x64\KuroShell.exe"),
        (Join-Path $env:LOCALAPPDATA "Kuro\shell\KuroShell.exe")
    ) | Where-Object { Test-Path -LiteralPath $_ }
    if (-not $candidates -or $candidates.Count -eq 0) {
        return $null
    }
    return $candidates[0]
}

function Publish-KuroShell {
    $built = Join-Path $KuroRoot "KuroShell\KuroShell\bin\x64\Debug\net10.0-windows10.0.19041.0\win-x64\KuroShell.exe"
    if (-not (Test-Path -LiteralPath $built)) {
        return
    }
    $destDir = Join-Path $env:LOCALAPPDATA "Kuro\shell"
    try {
        New-Item -ItemType Directory -Force -Path $destDir | Out-Null
        $srcDir = Split-Path -Parent $built
        Copy-Item -LiteralPath (Join-Path $srcDir "*") -Destination $destDir -Recurse -Force
        Write-KuroLog ("Published KuroShell to {0}" -f (Join-Path $destDir "KuroShell.exe"))
    }
    catch {
        Write-KuroLog ("Publish KuroShell skipped: {0}" -f $_.Exception.Message)
    }
}

function Test-KuroMutexHeld {
    # True when another process already owns Global\KuroAssistantSingleton.v1.
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
        Write-KuroLog ("Mutex probe failed: {0}" -f $_.Exception.Message)
        return $false
    }
    finally {
        if ($m) { $m.Dispose() }
    }
}

function Stop-KuroProcesses {
    $procs = @(Get-KuroProcesses)
    foreach ($p in $procs) {
        Write-KuroLog ("Stopping duplicate Kuro PID={0}" -f $p.ProcessId)
        Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
    }
    $lock = Join-Path $LogDir "kuro.lock"
    if (Test-Path -LiteralPath $lock) {
        Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
    }
    if ($procs.Count -gt 0) {
        Start-Sleep -Milliseconds 800
    }
    return $procs.Count
}

function Test-OllamaServing {
    try {
        $r = Invoke-WebRequest -Uri "http://127.0.0.1:11434/api/tags" -UseBasicParsing -TimeoutSec 2
        return ($r.StatusCode -ge 200 -and $r.StatusCode -lt 300)
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
        Start-Process -FilePath $ollama -ArgumentList "serve" -WindowStyle Hidden | Out-Null
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

    # Only preload a chat model into VRAM when KURO_LLM_MODE=always
    $llmMode = "fallback"
    $model = "qwen2.5:7b-instruct"
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

    # Prewarm model into GPU VRAM asynchronously without spawning any child process
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

try {
    Start-OllamaBackend
    Publish-KuroShell

    $Engine = Join-Path $KuroRoot "kuro-rs\target\release\kuro-engine.exe"
    if (-not (Test-Path -LiteralPath $Engine)) {
        $Engine = Join-Path $env:LOCALAPPDATA "Kuro\kuro-engine.exe"
    }
    elseif (Test-Path -LiteralPath $Engine) {
        $engineDest = Join-Path $env:LOCALAPPDATA "Kuro\kuro-engine.exe"
        try { Copy-Item -LiteralPath $Engine -Destination $engineDest -Force } catch { }
    }
    $Shell = Find-KuroShell
    $UsePython = ($env:KURO_LEGACY_PYTHON -eq "1") -or (-not (Test-Path -LiteralPath $Engine))
    if ($UsePython) {
        if (-not (Test-Path -LiteralPath $MainPy)) {
            throw ("entrypoint not found at: {0}" -f $MainPy)
        }
        $exe = $null
        if ((-not $ShowWindow) -and (Test-Path -LiteralPath $Pythonw)) {
            $exe = $Pythonw
        }
        elseif (Test-Path -LiteralPath $Python) {
            $exe = $Python
        }
        else {
            throw ("Virtualenv Python not found. Expected: {0}" -f $Python)
        }
        $arg = '"{0}"' -f $MainPy
        Write-KuroLog ("Starting legacy Python Kuro with '{0}'" -f $exe)
    }
    elseif ($Shell) {
        $exe = $Shell
        $arg = ""
        Write-KuroLog ("Starting KuroShell '{0}' (supervises kuro-engine)" -f $exe)
    }
    else {
        $exe = $Engine
        $arg = ""
        $env:KURO_API = "on"
        $env:KURO_HUD_SHOW_TEXT = "1"
        Write-KuroLog ("Starting kuro-engine '{0}' (no KuroShell build found)" -f $exe)
    }

    $existing = @(Get-KuroProcesses)
    $shellLive = @($existing | Where-Object { $_.Name -eq "KuroShell.exe" }).Count -gt 0
    $engineLive = @($existing | Where-Object { $_.Name -eq "kuro-engine.exe" }).Count -gt 0
    # The Windows venv launcher (Scripts\pythonw.exe) often keeps a child
    # pythonw from Python311 home. That pair is ONE instance, not a duplicate.
    if ($ForceRestart) {
        $n = Stop-KuroProcesses
        Write-KuroLog ("ForceRestart cleared {0} process(es)" -f $n)
        Start-Sleep -Milliseconds 400
    }
    elseif ($Shell -and $shellLive -and $engineLive) {
        Write-KuroLog ("Kuro already running (shell+engine) - skip start.")
        exit 0
    }
    elseif (-not $Shell -and ((Test-KuroMutexHeld) -or $engineLive -or ($existing.Count -gt 0))) {
        $pidShown = if ($existing.Count -gt 0) { $existing[0].ProcessId } else { "mutex" }
        Write-KuroLog ("Kuro already running ({0}) - skip start." -f $pidShown)
        exit 0
    }
    elseif ($Shell -and $shellLive -and -not $engineLive) {
        Write-KuroLog "KuroShell is up but engine is down - leaving shell to relaunch engine."
        exit 0
    }

    if ($env:ONLY_WHEN_CHARGING) {
        Remove-Item Env:ONLY_WHEN_CHARGING -ErrorAction SilentlyContinue
    }

    Write-KuroLog ("Starting Kuro with '{0}' from '{1}'" -f $exe, $KuroRoot)

    if ($UsePython) {
        if ($ShowWindow) {
            $proc = Start-Process -FilePath $exe -ArgumentList $arg -WorkingDirectory $KuroRoot -PassThru
        }
        else {
            $proc = Start-Process -FilePath $exe -ArgumentList $arg -WorkingDirectory $KuroRoot -WindowStyle Hidden -PassThru
        }
    }
    elseif ($Shell) {
        $proc = Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -PassThru
    }
    elseif ($ShowWindow) {
        $proc = Start-Process -FilePath $exe -WorkingDirectory $KuroRoot -PassThru
    }
    else {
        $proc = Start-Process -FilePath $exe -WorkingDirectory $KuroRoot -WindowStyle Hidden -PassThru
    }

    Write-KuroLog ("Kuro started. PID={0}" -f $proc.Id)
    exit 0
}
catch {
    $msg = $_.Exception.Message
    try { Write-KuroLog ("ERROR: {0}" -f $msg) } catch { }
    Write-Error $msg
    exit 1
}
