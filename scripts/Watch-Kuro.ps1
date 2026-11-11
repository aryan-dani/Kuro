#Requires -Version 5.1
<#
.SYNOPSIS
  Keep KuroShell + kuro-engine running after logon.

.DESCRIPTION
  A second copy of this script exits immediately.
  If nothing is running, start via Start-Kuro.ps1.
  If KuroShell is up, leave it alone - the shell relaunches a dead engine.
  Only force-kill+restart when the tree looks wedged (stale heartbeat with
  processes still listed). Never fight a healthy shell.

  %LOCALAPPDATA%\Kuro\DISABLED means do not start.
#>
[CmdletBinding()]
param(
    [int]$PollSeconds = 20,
    [int]$HeartbeatStaleSeconds = 120,
    [int]$RestartCooldownSeconds = 60
)

$ErrorActionPreference = "Continue"

$StartScript = Join-Path $PSScriptRoot "Start-Kuro.ps1"
$LogDir = Join-Path $env:LOCALAPPDATA "Kuro"
$LogFile = Join-Path $LogDir "supervisor.log"
$DisableFlag = Join-Path $LogDir "DISABLED"
$HeartbeatFile = Join-Path $LogDir "heartbeat.txt"
$MutexName = "Global\KuroAssistantSingleton.v1"

function Write-SuperLog {
    param([string]$Message)
    if (-not (Test-Path -LiteralPath $LogDir)) {
        New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
    }
    $stamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Add-Content -LiteralPath $LogFile -Value ("[{0}] {1}" -f $stamp, $Message)
}

$created = $false
$supervisor = New-Object System.Threading.Mutex($true, "Global\KuroSupervisor.v1", [ref]$created)
if (-not $created) {
    exit 0
}

Write-SuperLog ("Supervisor started. Poll={0}s heartbeatStale={1}s" -f $PollSeconds, $HeartbeatStaleSeconds)

function Get-KuroProcesses {
    @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            ($_.Name -eq "kuro-engine.exe") -or
            ($_.Name -eq "KuroShell.exe") -or
            (
                ($_.Name -eq "python.exe" -or $_.Name -eq "pythonw.exe") -and
                $_.CommandLine -and
                ($_.CommandLine -match 'Kuro') -and
                ($_.CommandLine -match '(run_kuro|main)\.py')
            )
        })
}

function Test-KuroMutexHeld {
    $m = $null
    try {
        $m = New-Object System.Threading.Mutex($false, $MutexName)
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
        Write-SuperLog ("Mutex probe failed: {0}" -f $_.Exception.Message)
        return $false
    }
    finally {
        if ($m) { $m.Dispose() }
    }
}

function Get-HeartbeatAgeSeconds {
    if (-not (Test-Path -LiteralPath $HeartbeatFile)) { return $null }
    try {
        $raw = (Get-Content -LiteralPath $HeartbeatFile -Raw).Trim()
        $epoch = [double]$raw
        $when = [DateTimeOffset]::FromUnixTimeSeconds([int64][Math]::Floor($epoch)).UtcDateTime
        return ((Get-Date).ToUniversalTime() - $when).TotalSeconds
    }
    catch {
        try {
            return ((Get-Date) - (Get-Item -LiteralPath $HeartbeatFile).LastWriteTime).TotalSeconds
        }
        catch {
            return $null
        }
    }
}

function Stop-KuroTree {
    $procs = @(Get-KuroProcesses)
    foreach ($p in $procs) {
        Write-SuperLog ("Stopping Kuro PID={0} ({1})" -f $p.ProcessId, $p.Name)
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

$lastRestart = [datetime]::MinValue

while ($true) {
    try {
        if (Test-Path -LiteralPath $DisableFlag) {
            Write-SuperLog "DISABLED flag set - supervisor idle."
        }
        else {
            $procs = @(Get-KuroProcesses)
            $shellLive = @($procs | Where-Object { $_.Name -eq "KuroShell.exe" }).Count -gt 0
            $engineLive = @($procs | Where-Object { $_.Name -eq "kuro-engine.exe" }).Count -gt 0
            $legacyLive = @($procs | Where-Object { $_.Name -match 'python' }).Count -gt 0
            $mutexHeld = Test-KuroMutexHeld
            $hbAge = Get-HeartbeatAgeSeconds
            $heartbeatOk = ($null -ne $hbAge -and $hbAge -ge 0 -and $hbAge -lt $HeartbeatStaleSeconds)
            $since = ((Get-Date) - $lastRestart).TotalSeconds

            if ($shellLive -or ($engineLive -and ($heartbeatOk -or $mutexHeld)) -or ($legacyLive -and $mutexHeld)) {
                # Shell owns engine recovery. Do not kill a live tree.
            }
            elseif ($procs.Count -eq 0) {
                if ($since -ge $RestartCooldownSeconds) {
                    Write-SuperLog "Kuro not running. Starting."
                    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $StartScript
                    $lastRestart = Get-Date
                }
            }
            elseif (-not $heartbeatOk -and $hbAge -ge ($HeartbeatStaleSeconds * 2) -and $since -ge $RestartCooldownSeconds) {
                $why = "shell={0} engine={1} mutex={2} heartbeatAge={3}" -f $shellLive, $engineLive, $mutexHeld, $hbAge
                Write-SuperLog ("Kuro wedged ({0}). Force restart." -f $why)
                Stop-KuroTree | Out-Null
                & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $StartScript -ForceRestart
                $lastRestart = Get-Date
            }
            else {
                Write-SuperLog ("Kuro recovering - leave alone (shell={0} engine={1} hb={2})." -f $shellLive, $engineLive, $hbAge)
            }
        }
    }
    catch {
        Write-SuperLog ("Supervisor loop error: {0}" -f $_.Exception.Message)
    }
    Start-Sleep -Seconds $PollSeconds
}
