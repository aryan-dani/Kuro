#Requires -Version 5.1
<#
.SYNOPSIS
  Keep exactly one light Kuro running after logon.

.DESCRIPTION
  Does not stop Kuro based on Wi-Fi. A second copy of this script exits immediately.
  Every poll: if the Kuro mutex is held and the heartbeat is fresh, do nothing.
  If the process is missing or the heartbeat is stale, kill that tree once and
  start again through Start-Kuro.ps1. Cooldown avoids a crash stampede.

  %LOCALAPPDATA%\Kuro\DISABLED means do not start.
#>
[CmdletBinding()]
param(
    [int]$PollSeconds = 60,
    [int]$HeartbeatStaleSeconds = 90,
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
            ($_.Name -eq "python.exe" -or $_.Name -eq "pythonw.exe") -and
            $_.CommandLine -and
            ($_.CommandLine -match 'Kuro') -and
            ($_.CommandLine -match '(run_kuro|main)\.py')
        })
}

function Test-KuroMutexHeld {
    $m = $null
    try {
        $m = New-Object System.Threading.Mutex($false, $MutexName)
        $held = $false
        try {
            $got = $m.WaitOne(0)
        }
        catch [System.Threading.AbandonedMutexException] {
            # Previous owner died. We now own it; release so a real start can take it.
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
            $age = ((Get-Date) - (Get-Item -LiteralPath $HeartbeatFile).LastWriteTime).TotalSeconds
            return $age
        }
        catch {
            return $null
        }
    }
}

function Stop-KuroTree {
    $procs = @(Get-KuroProcesses)
    foreach ($p in $procs) {
        Write-SuperLog ("Stopping Kuro PID={0}" -f $p.ProcessId)
        Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
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
            Write-SuperLog "DISABLED flag set — supervisor idle."
        }
        else {
            $procs = @(Get-KuroProcesses)
            $mutexHeld = Test-KuroMutexHeld
            $hbAge = Get-HeartbeatAgeSeconds
            $young = $false
            foreach ($p in $procs) {
                try {
                    $started = $p.CreationDate
                    if ($started -and (((Get-Date) - $started).TotalSeconds -lt $HeartbeatStaleSeconds)) {
                        $young = $true
                    }
                }
                catch { }
            }

            $heartbeatOk = ($null -ne $hbAge -and $hbAge -ge 0 -and $hbAge -lt $HeartbeatStaleSeconds) -or $young
            $healthy = ($procs.Count -gt 0) -and $mutexHeld -and $heartbeatOk

            if ($healthy) {
                # venv pythonw + Python311 child is one instance. Leave it alone.
            }
            else {
                $since = ((Get-Date) - $lastRestart).TotalSeconds
                if ($since -lt $RestartCooldownSeconds -and $procs.Count -gt 0) {
                    Write-SuperLog ("Unhealthy but cooldown {0:n0}s — waiting." -f ($RestartCooldownSeconds - $since))
                }
                else {
                    $why = "procs={0} mutex={1} heartbeatAge={2}" -f $procs.Count, $mutexHeld, $hbAge
                    Write-SuperLog ("Kuro not healthy ({0}). Restarting." -f $why)
                    if ($procs.Count -gt 0) {
                        Stop-KuroTree | Out-Null
                    }
                    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $StartScript
                    $lastRestart = Get-Date
                }
            }
        }
    }
    catch {
        Write-SuperLog ("Supervisor loop error: {0}" -f $_.Exception.Message)
    }
    Start-Sleep -Seconds $PollSeconds
}
