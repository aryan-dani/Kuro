#Requires -Version 5.1
<#
.SYNOPSIS
  Runs Kuro only while this laptop is on your home Wi-Fi.

.DESCRIPTION
  A tiny always-on watcher (a PowerShell loop that wakes every few seconds, so the
  battery cost is negligible):
    - On home Wi-Fi  : starts Kuro through Start-Kuro.ps1 if it is not running.
    - Off home Wi-Fi : after a short grace period, stops Kuro so Whisper, XTTS and
                       the GPU boost loop stop using the battery.

  Home network name(s), first match wins:
    1. -HomeSsid "MyWifi","MyWifi-5G"
    2. environment variable KURO_HOME_SSID (comma separated)
    3. file %LOCALAPPDATA%\Kuro\home_ssid.txt (one name per line)

  Escape hatches:
    %LOCALAPPDATA%\Kuro\DISABLED  : watcher does nothing at all (same flag Start-Kuro uses)
    %LOCALAPPDATA%\Kuro\AWAY_OK   : Kuro is never stopped, even away from home
#>
[CmdletBinding()]
param(
    [string[]]$HomeSsid,
    [int]$PollSeconds = 20,
    [int]$AwayPolls = 3,
    [int]$RestartCooldownSeconds = 60
)

$ErrorActionPreference = "Continue"

$StartScript = Join-Path $PSScriptRoot "Start-Kuro.ps1"
$LogDir      = Join-Path $env:LOCALAPPDATA "Kuro"
$LogFile     = Join-Path $LogDir "wifi-watch.log"
$DisableFlag = Join-Path $LogDir "DISABLED"
$AwayOkFlag  = Join-Path $LogDir "AWAY_OK"
$SsidFile    = Join-Path $LogDir "home_ssid.txt"

function Write-WatchLog {
    param([string]$Message)
    if (-not (Test-Path -LiteralPath $LogDir)) {
        New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
    }
    $stamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Add-Content -LiteralPath $LogFile -Value ("[{0}] {1}" -f $stamp, $Message)
}

# One watcher at a time
$created = $false
$mutex = New-Object System.Threading.Mutex($true, "Global\KuroWifiWatcher.v1", [ref]$created)
if (-not $created) { exit 0 }

# Resolve home network names
if (-not $HomeSsid -or $HomeSsid.Count -eq 0) {
    if ($env:KURO_HOME_SSID) {
        $HomeSsid = $env:KURO_HOME_SSID -split ","
    }
    elseif (Test-Path -LiteralPath $SsidFile) {
        $HomeSsid = Get-Content -LiteralPath $SsidFile
    }
}
$HomeSsid = @($HomeSsid | ForEach-Object { $_.Trim() } | Where-Object { $_ })
if ($HomeSsid.Count -eq 0) {
    Write-WatchLog "No home Wi-Fi name set. Use -HomeSsid, KURO_HOME_SSID, or create home_ssid.txt. Exiting."
    exit 1
}
Write-WatchLog ("Watcher started. Home network(s): {0}" -f ($HomeSsid -join ", "))

function Get-CurrentSsid {
    try {
        $out = netsh wlan show interfaces 2>$null
        foreach ($line in $out) {
            # Matches "SSID : name" but not "BSSID : ..."
            if ($line -match '^\s*SSID\s*:\s*(.+?)\s*$') { return $Matches[1] }
        }
    }
    catch { }
    return $null
}

function Test-AtHome {
    # Method 1: netsh (needs Location services enabled on newer Windows 11)
    $ssid = Get-CurrentSsid
    if ($ssid -and ($HomeSsid -contains $ssid)) { return $true }

    # Method 2: Windows network profile name (usually equals the Wi-Fi name)
    try {
        $profiles = Get-NetConnectionProfile -ErrorAction Stop
        foreach ($p in $profiles) {
            if (($HomeSsid -contains $p.Name) -and
                ($p.IPv4Connectivity -ne "Disconnected" -or $p.IPv6Connectivity -ne "Disconnected")) {
                return $true
            }
        }
    }
    catch { }
    return $false
}

function Get-KuroProcesses {
    Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            ($_.Name -eq "kuro-engine.exe") -or
            ($_.Name -eq "KuroShell.exe")
        }
}

function Stop-Kuro {
    $procs = @(Get-KuroProcesses)
    foreach ($p in $procs) {
        Write-WatchLog ("Stopping Kuro PID={0}" -f $p.ProcessId)
        Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue
    }
    return $procs.Count
}

function Start-KuroHome {
    Write-WatchLog "Starting Kuro via Start-Kuro.ps1"
    try {
        # Do not -Wait: Start-Kuro may spend time on ollama/serve; keep the poll loop alive.
        Start-Process -FilePath "powershell.exe" -ArgumentList @(
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden",
            "-File", $StartScript
        ) -WindowStyle Hidden | Out-Null
    }
    catch {
        Write-WatchLog ("Start failed: {0}" -f $_.Exception.Message)
    }
}

$awayCount = 0
$lastStart = [datetime]::MinValue
$lastState = ""

while ($true) {
    try {
        if (Test-Path -LiteralPath $DisableFlag) {
            Start-Sleep -Seconds $PollSeconds
            continue
        }

        if (Test-AtHome) {
            $awayCount = 0
            if ($lastState -ne "home") {
                Write-WatchLog "At home Wi-Fi"
                $lastState = "home"
            }
            $running = @(Get-KuroProcesses).Count -gt 0
            $cooled = ((Get-Date) - $lastStart).TotalSeconds -ge $RestartCooldownSeconds
            if ((-not $running) -and $cooled) {
                $lastStart = Get-Date
                Start-KuroHome
            }
        }
        else {
            $awayCount++
            if ($awayCount -ge $AwayPolls) {
                if ($lastState -ne "away") {
                    Write-WatchLog "Away from home Wi-Fi"
                    $lastState = "away"
                }
                if (-not (Test-Path -LiteralPath $AwayOkFlag)) {
                    $n = Stop-Kuro
                    if ($n -gt 0) { Write-WatchLog ("Stopped {0} Kuro process(es) to save battery" -f $n) }
                }
            }
        }
    }
    catch {
        Write-WatchLog ("Loop error: {0}" -f $_.Exception.Message)
    }
    Start-Sleep -Seconds $PollSeconds
}
