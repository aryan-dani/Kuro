#Requires -Version 5.1
<#
.SYNOPSIS
  Thin logon watcher: ensure KuroShell is running. Do not restart the engine.

.DESCRIPTION
  A second copy of this script exits immediately.
  The WinUI shell owns engine restarts (ADR 0010 / RestartPolicy).
  This script only starts Kuro via Start-Kuro.ps1 when KuroShell is not running.
  It never force-kills a healthy shell or its engine.

  %LOCALAPPDATA%\Kuro\DISABLED means do not start.
#>
[CmdletBinding()]
param(
    [switch]$ShowConsole,
    [int]$PollSeconds = 15,
    [int]$RestartCooldownSeconds = 10,
    [switch]$Silent
)

$ErrorActionPreference = "Continue"

if (-not ("KuroWatchHider" -as [type])) {
    try {
        Add-Type -Name KuroWatchHider -Namespace KuroWinAPI -MemberDefinition '
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
[DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow();
' -ErrorAction SilentlyContinue
    } catch { }
}
try {
    if (-not $ShowConsole) {
        $hwnd = [KuroWinAPI.KuroWatchHider]::GetConsoleWindow()
        if ($hwnd -ne [IntPtr]::Zero) {
            [void][KuroWinAPI.KuroWatchHider]::ShowWindow($hwnd, 0)
        }
    }
} catch { }

$StartScript = Join-Path $PSScriptRoot "Start-Kuro.ps1"
$LogDir = Join-Path $env:LOCALAPPDATA "Kuro"
$LogFile = Join-Path $LogDir "supervisor.log"
$DisableFlag = Join-Path $LogDir "DISABLED"
$BuildFlag = Join-Path $LogDir "BUILDING"

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

Write-SuperLog ("Thin watcher started. Poll={0}s (shell owns engine restarts)" -f $PollSeconds)

function Test-ShellRunning {
    @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -eq "KuroShell.exe" }).Count -gt 0
}

$lastStart = [datetime]::MinValue

while ($true) {
    try {
        if (Test-Path -LiteralPath $DisableFlag) {
            Write-SuperLog "DISABLED flag set - watcher idle."
        }
        elseif (Test-Path -LiteralPath $BuildFlag) {
            Write-SuperLog "BUILDING flag set - watcher waiting."
        }
        elseif (Test-ShellRunning) {
            # Shell is up; EngineSupervisor owns any engine restart.
        }
        else {
            $since = ((Get-Date) - $lastStart).TotalSeconds
            if ($since -ge $RestartCooldownSeconds) {
                Write-SuperLog "KuroShell not running. Starting via Start-Kuro.ps1."
                & $StartScript | Out-Null
                $lastStart = Get-Date
            }
        }
    }
    catch {
        Write-SuperLog ("Watcher loop error: {0}" -f $_.Exception.Message)
    }
    Start-Sleep -Seconds $PollSeconds
}
