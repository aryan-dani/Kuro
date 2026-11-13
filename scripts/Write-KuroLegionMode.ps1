#Requires -Version 5.1
<#
.SYNOPSIS
  Write Legion performance state for Kuro. Meant to run elevated.

.DESCRIPTION
  Every 15 seconds writes %LOCALAPPDATA%\Kuro\legion_mode.json:
    smart_fan_mode  (Lenovo GetSmartFanMode; 3 = Performance on this Legion)
    ac_fit          (IsACFitForOC; 1 = real brick, 0 = weak adapter, -1 = unknown)
    ac_online       (Windows AC line)
  A normal user cannot read root\WMI LENOVO_GAMEZONE_DATA on this machine.
#>
[CmdletBinding()]
param(
    [switch]$Silent,
    [int]$PollSeconds = 15
)

$ErrorActionPreference = "Continue"

if (-not $Silent) {
    $vbs = Join-Path $PSScriptRoot "start_kuro_silent.vbs"
    if (Test-Path -LiteralPath $vbs) {
        Start-Process "wscript.exe" -ArgumentList ('//nologo "{0}" "{1}"' -f $vbs, $PSCommandPath)
        return
    }
}

if (-not ("KuroConsoleHider" -as [type])) {
    try {
        Add-Type -Name KuroConsoleHider -Namespace KuroWinAPI -MemberDefinition '
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
[DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow();
' -ErrorAction SilentlyContinue
    } catch { }
}
try {
    $hwnd = [KuroWinAPI.KuroConsoleHider]::GetConsoleWindow()
    if ($hwnd -ne [IntPtr]::Zero) {
        [void][KuroWinAPI.KuroConsoleHider]::ShowWindow($hwnd, 0)
    }
} catch { }

$LogDir = Join-Path $env:LOCALAPPDATA "Kuro"
$OutFile = Join-Path $LogDir "legion_mode.json"
$LogFile = Join-Path $LogDir "legion-mode.log"

if (-not (Test-Path -LiteralPath $LogDir)) {
    New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
}

if (-not ("KuroPowerStatus" -as [type])) {
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public class KuroPowerStatus {
  [StructLayout(LayoutKind.Sequential)]
  public struct SYSTEM_POWER_STATUS {
    public byte ACLineStatus;
    public byte BatteryFlag;
    public byte BatteryLifePercent;
    public byte SystemStatusFlag;
    public uint BatteryLifeTime;
    public uint BatteryFullLifeTime;
  }
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool GetSystemPowerStatus(out SYSTEM_POWER_STATUS sps);
}
"@
}

function Write-ModeLog {
    param([string]$Message)
    $stamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Add-Content -LiteralPath $LogFile -Value ("[{0}] {1}" -f $stamp, $Message)
}

function Get-AcOnline {
    $sps = New-Object KuroPowerStatus+SYSTEM_POWER_STATUS
    $ok = [KuroPowerStatus]::GetSystemPowerStatus([ref]$sps)
    if (-not $ok) { return $false }
    return ($sps.ACLineStatus -eq 1)
}

function Invoke-LenovoMethod {
    param([string]$Name)
    $obj = $null
    try {
        $searcher = New-Object System.Management.ManagementObjectSearcher(
            "root\wmi",
            "SELECT * FROM LENOVO_GAMEZONE_DATA"
        )
        $obj = $searcher.Get() | Select-Object -First 1
    }
    catch {
        $obj = $null
    }
    if (-not $obj) { return $null }
    try {
        $inParams = $obj.GetMethodParameters($Name)
        $result = $obj.InvokeMethod($Name, $inParams, $null)
        if ($null -eq $result) { return $null }
        if ($result.Properties["Data"]) { return [int]$result.Properties["Data"].Value }
        if ($null -ne $result.Data) { return [int]$result.Data }
        return $null
    }
    catch {
        Write-ModeLog ("{0} failed: {1}" -f $Name, $_.Exception.Message)
        return $null
    }
}

$created = $false
$gate = New-Object System.Threading.Mutex($true, "Global\KuroLegionMode.v1", [ref]$created)
if (-not $created) { exit 0 }

Write-ModeLog "Legion mode writer started."

while ($true) {
    $fan = Invoke-LenovoMethod -Name "GetSmartFanMode"
    $fit = Invoke-LenovoMethod -Name "IsACFitForOC"
    if ($null -eq $fit) { $fit = -1 }
    $ac = Get-AcOnline
    $payload = [ordered]@{
        smart_fan_mode = $fan
        ac_fit         = $fit
        ac_online      = [bool]$ac
        updated        = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    }
    $json = $payload | ConvertTo-Json -Compress
    $tmp = $OutFile + ".tmp"
    try {
        Set-Content -LiteralPath $tmp -Value $json -Encoding ASCII
        Move-Item -LiteralPath $tmp -Destination $OutFile -Force
    }
    catch {
        Write-ModeLog ("Write failed: {0}" -f $_.Exception.Message)
    }
    Start-Sleep -Seconds $PollSeconds
}
