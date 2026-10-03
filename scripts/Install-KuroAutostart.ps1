#Requires -Version 5.1
<#
.SYNOPSIS
  Installs a Windows Scheduled Task that starts Kuro at user logon (boot/login).

.DESCRIPTION
  Creates/updates task "KuroAutostart":
  - Trigger: at logon for the current user
  - Runs Start-Kuro.ps1 hidden in the background
  - Allowed on battery (tray stays up; GPU work stays gated by ONLY_WHEN_CHARGING)

  Self-elevates via UAC when not already running as Administrator.
#>
[CmdletBinding()]
param(
    [switch]$Uninstall,
    [switch]$StartNow
)

$ErrorActionPreference = "Stop"
$TaskName = "KuroAutostart"
$StartScript = Join-Path $PSScriptRoot "Start-Kuro.ps1"

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

    $p = Start-Process -FilePath "powershell.exe" -Verb RunAs -ArgumentList $argList -Wait -PassThru
    if ($null -eq $p) { exit 1 }
    exit $p.ExitCode
}

function Uninstall-KuroTask {
    $existing = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
    if ($existing) {
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
        Write-Host ("Removed scheduled task: {0}" -f $TaskName)
    }
    else {
        Write-Host ("No scheduled task named '{0}' was found." -f $TaskName)
    }
}

function Install-KuroTask {
    if (-not (Test-Path -LiteralPath $StartScript)) {
        throw ("Start script missing: {0}" -f $StartScript)
    }

    $userId = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    $psExe = Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0\powershell.exe"
    $actionArgs = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "{0}"' -f $StartScript

    $action = New-ScheduledTaskAction -Execute $psExe -Argument $actionArgs
    $trigger = New-ScheduledTaskTrigger -AtLogOn -User $userId
    $settings = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries `
        -StartWhenAvailable `
        -MultipleInstances IgnoreNew `
        -ExecutionTimeLimit ([TimeSpan]::Zero)
    # Limited: no UAC at every logon. Installer itself still elevates once.
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
        -Description "Starts Kuro AI assistant in the background at logon. GPU workloads remain gated until AC power (ONLY_WHEN_CHARGING)." `
        -Force | Out-Null

    $task = Get-ScheduledTask -TaskName $TaskName
    Write-Host ("Installed scheduled task: {0}" -f $TaskName)
    Write-Host ("  User:    {0}" -f $userId)
    Write-Host ("  State:   {0}" -f $task.State)
    Write-Host ("  Action:  {0} {1}" -f $psExe, $actionArgs)
    Write-Host "  Trigger: At logon"
}

Request-Elevation

if ($Uninstall) {
    Uninstall-KuroTask
    exit 0
}

Install-KuroTask

if ($StartNow) {
    Write-Host "Starting Kuro now..."
    try {
        & $StartScript
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
            Write-Warning ("Start-Kuro.ps1 exited with code {0}. Task is still installed." -f $LASTEXITCODE)
        }
    }
    catch {
        Write-Warning ("Could not start Kuro immediately: {0}. Task is still installed." -f $_.Exception.Message)
    }
}

Write-Host ""
Write-Host "Done. Kuro will start automatically at the next Windows logon."
Write-Host ("Logs: {0}" -f (Join-Path $env:LOCALAPPDATA "Kuro\autostart.log"))
exit 0
