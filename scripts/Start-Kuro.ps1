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
$MainPy   = Join-Path $KuroRoot "run_kuro.py"
if (-not (Test-Path -LiteralPath $MainPy)) {
    $MainPy = Join-Path $KuroRoot "main.py"
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
        Write-KuroLog "Kuro DISABLED — refusing start (Start-Kuro.ps1 -ForceRestart to re-enable)."
        exit 0
    }
}

function Write-KuroLog {
    param([string]$Message)
    if (-not (Test-Path -LiteralPath $LogDir)) {
        New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
    }
    $stamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Add-Content -LiteralPath $LogFile -Value ("[{0}] {1}" -f $stamp, $Message)
}

function Get-KuroProcesses {
    Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            ($_.Name -eq "python.exe" -or $_.Name -eq "pythonw.exe") -and
            $_.CommandLine -and
            ($_.CommandLine -match 'Kuro') -and
            ($_.CommandLine -match '(run_kuro|main)\.py')
        }
}

function Stop-KuroProcesses {
    $procs = @(Get-KuroProcesses)
    foreach ($p in $procs) {
        Write-KuroLog ("Stopping duplicate Kuro PID={0}" -f $p.ProcessId)
        Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
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
        Write-KuroLog "Ollama not installed — skip backend start"
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

    if ($llmMode -eq "always") {
        $warm = @"
try {
  `$body = @{ model = '$model'; prompt = 'hi'; stream = `$false; options = @{ num_predict = 1 } } | ConvertTo-Json
  Invoke-RestMethod -Uri 'http://127.0.0.1:11434/api/generate' -Method Post -Body `$body -ContentType 'application/json' -TimeoutSec 180 | Out-Null
} catch {}
"@
        Start-Process -FilePath "powershell.exe" -ArgumentList @(
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-Command", $warm
        ) -WindowStyle Hidden | Out-Null
        Write-KuroLog ("Ollama model warm kicked off: {0}" -f $model)
    }
    else {
        Write-KuroLog ("Ollama serve ready; skipping model warm (KURO_LLM_MODE={0})" -f $llmMode)
    }
}

try {
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

    # Backend first — Kuro brain depends on it
    Start-OllamaBackend

    $existing = @(Get-KuroProcesses)
    # The Windows venv launcher (Scripts\pythonw.exe) often keeps a child
    # pythonw from Python311 home — that pair is ONE instance, not a duplicate.
    $launchers = @($existing | Where-Object {
        ($_.ExecutablePath -and ($_.ExecutablePath -match '\\.venv\\Scripts\\pythonw?\.exe$')) -or
        ($_.CommandLine -match '\\.venv\\Scripts\\pythonw?\.exe')
    })
    if ($ForceRestart) {
        $n = Stop-KuroProcesses
        Write-KuroLog ("ForceRestart cleared {0} process(es)" -f $n)
    }
    elseif ($launchers.Count -ge 1) {
        Write-KuroLog ("Kuro already running PID={0} - skip start." -f $launchers[0].ProcessId)
        exit 0
    }
    elseif ($existing.Count -gt 0) {
        # Orphaned home-python copies with no venv parent — clear then start
        Write-KuroLog ("Found {0} orphan Kuro process(es) — clearing" -f $existing.Count)
        Stop-KuroProcesses | Out-Null
    }

    if ($env:ONLY_WHEN_CHARGING) {
        Remove-Item Env:ONLY_WHEN_CHARGING -ErrorAction SilentlyContinue
    }

    Write-KuroLog ("Starting Kuro with '{0}' from '{1}'" -f $exe, $KuroRoot)

    $arg = '"{0}"' -f $MainPy
    if ($ShowWindow) {
        $proc = Start-Process -FilePath $exe -ArgumentList $arg -WorkingDirectory $KuroRoot -PassThru
    }
    else {
        $proc = Start-Process -FilePath $exe -ArgumentList $arg -WorkingDirectory $KuroRoot -WindowStyle Hidden -PassThru
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
