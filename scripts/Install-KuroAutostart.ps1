#Requires -Version 5.1
<#
.SYNOPSIS
  Installs a Windows Scheduled Task that starts the Kuro home-Wi‑Fi watcher at logon.

.DESCRIPTION
  Creates/updates task "KuroAutostart":
  - Trigger: at logon for the current user
  - Runs Watch-KuroWifi.ps1 (starts Kuro only on home Wi‑Fi; stops it when away)
  - Allowed on battery (watcher is cheap; GPU work still gated inside Kuro)

  Self-elevates via UAC when not already running as Administrator.

.PARAMETER HomeSsid
  Home Wi‑Fi name(s). Written to %LOCALAPPDATA%\Kuro\home_ssid.txt.
  If omitted, the current SSID is captured when detectable.
#>
[CmdletBinding()]
param(
    [switch]$Uninstall,
    [switch]$StartNow,
    [string[]]$HomeSsid
)

$ErrorActionPreference = "Stop"
$TaskName = "KuroAutostart"
$WatchScript = Join-Path $PSScriptRoot "Watch-KuroWifi.ps1"
$StartScript = Join-Path $PSScriptRoot "Start-Kuro.ps1"
$LogDir = Join-Path $env:LOCALAPPDATA "Kuro"
$SsidFile = Join-Path $LogDir "home_ssid.txt"

function Test-IsAdmin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($id)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Request-Elevation {
    if (Test-IsAdmin) { return }

    Write-Host "Administrator privileges required. Prompting for UAC elevation..."
    $argList = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", $PSCommandPath
    )
    if ($Uninstall) { $argList += "-Uninstall" }
    if ($StartNow)   { $argList += "-StartNow" }
    foreach ($s in @($HomeSsid)) {
        if ($s) { $argList += @("-HomeSsid", $s) }
    }

    $p = Start-Process -FilePath "powershell.exe" -Verb RunAs -ArgumentList $argList -Wait -PassThru
    if ($null -eq $p) { exit 1 }
    exit $p.ExitCode
}

function Get-CurrentSsid {
    try {
        $out = netsh wlan show interfaces 2>$null
        foreach ($line in $out) {
            if ($line -match '^\s*SSID\s*:\s*(.+?)\s*$') { return $Matches[1].Trim() }
        }
    }
    catch { }
    return $null
}

function Save-HomeSsid {
    param([string[]]$Names)
    if (-not (Test-Path -LiteralPath $LogDir)) {
        New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
    }
    $clean = @($Names | ForEach-Object { $_.Trim() } | Where-Object { $_ } | Select-Object -Unique)
    if ($clean.Count -eq 0) {
        $detected = Get-CurrentSsid
        if ($detected) { $clean = @($detected) }
    }
    if ($clean.Count -eq 0) {
        Write-Warning ("No home SSID set. Create {0} (one name per line) before the watcher can start Kuro." -f $SsidFile)
        return
    }
    Set-Content -LiteralPath $SsidFile -Value $clean -Encoding UTF8
    Write-Host ("Home Wi-Fi saved to {0}: {1}" -f $SsidFile, ($clean -join ", "))
}

function Uninstall-KuroTask {
    foreach ($name in @($TaskName, "KuroWifiWatcher")) {
        $existing = Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
        if ($existing) {
            Unregister-ScheduledTask -TaskName $name -Confirm:$false
            Write-Host ("Removed scheduled task: {0}" -f $name)
        }
    }
}

function Install-KuroTask {
    if (-not (Test-Path -LiteralPath $WatchScript)) {
        throw ("Watch script missing: {0}" -f $WatchScript)
    }

    $userId = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    $psExe = Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0\powershell.exe"
    $actionArgs = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "{0}"' -f $WatchScript

    $action = New-ScheduledTaskAction -Execute $psExe -Argument $actionArgs
    $trigger = New-ScheduledTaskTrigger -AtLogOn -User $userId
    $settings = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries `
        -StartWhenAvailable `
        -MultipleInstances IgnoreNew `
        -ExecutionTimeLimit ([TimeSpan]::Zero)
    $principal = New-ScheduledTaskPrincipal `
        -UserId $userId `
        -LogonType Interactive `
        -RunLevel Limited

    Register-ScheduledTask `
        -TaskName $TaskName `
        -Action $action `
        -Trigger $trigger `
        -Settings $settings `
        -Principal $principal `
        -Description "Starts Kuro Wi-Fi watcher at logon. Kuro runs only on home Wi-Fi; stops away to save battery." `
        -Force | Out-Null

    # Remove legacy duplicate task name if present
    $legacy = Get-ScheduledTask -TaskName "KuroWifiWatcher" -ErrorAction SilentlyContinue
    if ($legacy) {
        Unregister-ScheduledTask -TaskName "KuroWifiWatcher" -Confirm:$false
    }

    $task = Get-ScheduledTask -TaskName $TaskName
    Write-Host ("Installed scheduled task: {0}" -f $TaskName)
    Write-Host ("  User:    {0}" -f $userId)
    Write-Host ("  State:   {0}" -f $task.State)
    Write-Host ("  Action:  {0} {1}" -f $psExe, $actionArgs)
    Write-Host "  Trigger: At logon (Wi-Fi watcher)"
}

Request-Elevation

if ($Uninstall) {
    Uninstall-KuroTask
    exit 0
}

Save-HomeSsid -Names $HomeSsid
Install-KuroTask

if ($StartNow) {
    Write-Host "Starting Wi-Fi watcher now..."
    try {
        Start-Process -FilePath "powershell.exe" -ArgumentList @(
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden",
            "-File", $WatchScript
        ) -WindowStyle Hidden | Out-Null
    }
    catch {
        Write-Warning ("Could not start watcher immediately: {0}. Task is still installed." -f $_.Exception.Message)
    }
}

Write-Host ""
Write-Host "Done. At logon the Wi-Fi watcher starts; Kuro runs only on home Wi-Fi."
Write-Host ("SSID file: {0}" -f $SsidFile)
Write-Host ("Watcher log: {0}" -f (Join-Path $LogDir "wifi-watch.log"))
Write-Host ("Autostart log: {0}" -f (Join-Path $LogDir "autostart.log"))
Write-Host "Docs: docs/WIFI_WATCH.md"
exit 0
