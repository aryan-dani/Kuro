#Requires -Version 5.1
<#
.SYNOPSIS
  Installs logon tasks that keep one light Kuro running.

.DESCRIPTION
  KuroAutostart (limited): Watch-Kuro.ps1, 90 seconds after logon.
  KuroLegionMode (highest): Write-KuroLegionMode.ps1 so Kuro can see Performance mode.

  Self-elevates via UAC when not already running as Administrator.
#>
[CmdletBinding()]
param(
    [switch]$Uninstall,
    [switch]$StartNow,
    [string[]]$HomeSsid
)

$ErrorActionPreference = "Stop"
$TaskName = "KuroAutostart"
$ModeTask = "KuroLegionMode"
$WatchScript = Join-Path $PSScriptRoot "Watch-Kuro.ps1"
$ModeScript = Join-Path $PSScriptRoot "Write-KuroLegionMode.ps1"
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
    if (-not $Names -or $Names.Count -eq 0) { return }
    if (-not (Test-Path -LiteralPath $LogDir)) {
        New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
    }
    $clean = @($Names | ForEach-Object { $_.Trim() } | Where-Object { $_ } | Select-Object -Unique)
    if ($clean.Count -eq 0) {
        $detected = Get-CurrentSsid
        if ($detected) { $clean = @($detected) }
    }
    if ($clean.Count -eq 0) { return }
    Set-Content -LiteralPath $SsidFile -Value $clean -Encoding ASCII
    Write-Host ("Home Wi-Fi saved to {0}: {1}" -f $SsidFile, ($clean -join ", "))
}

function Uninstall-KuroTask {
    foreach ($name in @($TaskName, $ModeTask, "KuroWifiWatcher")) {
        $existing = Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
        if ($existing) {
            Unregister-ScheduledTask -TaskName $name -Confirm:$false
            Write-Host ("Removed scheduled task: {0}" -f $name)
        }
    }
}

function Set-TriggerDelay {
    param($Trigger, [int]$Seconds)
    # Logon trigger Delay is an ISO-8601 duration string. A TimeSpan serializes
    # as 00:01:30 and Task Scheduler rejects that XML.
    $Trigger.Delay = "PT{0}S" -f $Seconds
}

function Install-KuroTask {
    if (-not (Test-Path -LiteralPath $WatchScript)) {
        throw ("Supervisor script missing: {0}" -f $WatchScript)
    }
    if (-not (Test-Path -LiteralPath $ModeScript)) {
        throw ("Legion mode script missing: {0}" -f $ModeScript)
    }

    $userId = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    $psExe = Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0\powershell.exe"

    $settings = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries `
        -StartWhenAvailable `
        -MultipleInstances IgnoreNew `
        -ExecutionTimeLimit ([TimeSpan]::Zero)

    $watchArgs = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "{0}"' -f $WatchScript
    $watchTrigger = New-ScheduledTaskTrigger -AtLogOn -User $userId
    Set-TriggerDelay -Trigger $watchTrigger -Seconds 90
    $watchPrincipal = New-ScheduledTaskPrincipal -UserId $userId -LogonType Interactive -RunLevel Limited
    Register-ScheduledTask `
        -TaskName $TaskName `
        -Action (New-ScheduledTaskAction -Execute $psExe -Argument $watchArgs) `
        -Trigger $watchTrigger `
        -Settings $settings `
        -Principal $watchPrincipal `
        -Description "Starts the Kuro supervisor 90 seconds after logon. One light instance; GPU warm only in Legion Performance on the real charger." `
        -Force | Out-Null

    $modeArgs = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "{0}"' -f $ModeScript
    $modeTrigger = New-ScheduledTaskTrigger -AtLogOn -User $userId
    Set-TriggerDelay -Trigger $modeTrigger -Seconds 30
    $modePrincipal = New-ScheduledTaskPrincipal -UserId $userId -LogonType Interactive -RunLevel Highest
    Register-ScheduledTask `
        -TaskName $ModeTask `
        -Action (New-ScheduledTaskAction -Execute $psExe -Argument $modeArgs) `
        -Trigger $modeTrigger `
        -Settings $settings `
        -Principal $modePrincipal `
        -Description "Writes Legion Performance mode for Kuro. Highest so root\WMI is readable." `
        -Force | Out-Null

    $legacy = Get-ScheduledTask -TaskName "KuroWifiWatcher" -ErrorAction SilentlyContinue
    if ($legacy) {
        Unregister-ScheduledTask -TaskName "KuroWifiWatcher" -Confirm:$false
    }

    Write-Host ("Installed {0} (delay 90s) and {1} (delay 30s, highest)" -f $TaskName, $ModeTask)
    Write-Host ("  User: {0}" -f $userId)
}

Request-Elevation

$InstallLog = Join-Path $LogDir "install.log"
if (-not (Test-Path -LiteralPath $LogDir)) {
    New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
}

try {
    if ($Uninstall) {
        Uninstall-KuroTask
        exit 0
    }

    Save-HomeSsid -Names $HomeSsid
    Install-KuroTask

    if ($StartNow) {
        Write-Host "Starting Legion mode writer and Kuro supervisor now..."
        Start-Process -FilePath "powershell.exe" -ArgumentList @(
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden",
            "-File", $ModeScript
        ) -WindowStyle Hidden | Out-Null
        Start-Process -FilePath "powershell.exe" -ArgumentList @(
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden",
            "-File", $WatchScript
        ) -WindowStyle Hidden | Out-Null
    }

    "OK $(Get-Date -Format o)" | Set-Content -LiteralPath $InstallLog -Encoding ASCII
    Write-Host ""
    Write-Host "Done. Logon starts one light Kuro. GPU models load only in Legion Performance on the real charger."
    Write-Host ("Supervisor log: {0}" -f (Join-Path $LogDir "supervisor.log"))
    Write-Host ("Mode file:      {0}" -f (Join-Path $LogDir "legion_mode.json"))
    Write-Host ("Autostart log:  {0}" -f (Join-Path $LogDir "autostart.log"))
    exit 0
}
catch {
    $msg = $_.Exception.ToString()
    try { Set-Content -LiteralPath $InstallLog -Value $msg -Encoding ASCII } catch { }
    Write-Error $msg
    exit 1
}
