#Requires -Version 5.1
<#
.SYNOPSIS
  Stop locked Kuro processes, ensure Whisper GGML is present, then build the native engine and WinUI shell.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Manifest = Join-Path $Root "kuro-rs\Cargo.toml"
$Shell = Join-Path $Root "KuroShell\KuroShell\KuroShell.csproj"
$EngineOut = Join-Path $Root "kuro-rs\target\release\kuro-engine.exe"
$CoreDll = Join-Path $Root "KuroShell\KuroShell\bin\x64\Debug\net10.0-windows10.0.19041.0\win-x64\KuroShell.Core.dll"

function Test-FileWritable {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        return $true
    }
    try {
        $stream = [System.IO.File]::Open($Path, "Open", "ReadWrite", "None")
        $stream.Close()
        return $true
    }
    catch {
        return $false
    }
}

function Stop-KuroBuildLocks {
    Get-Process -Name KuroShell, kuro-engine -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
    $python = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            ($_.Name -eq "python.exe" -or $_.Name -eq "pythonw.exe") -and
            $_.CommandLine -and
            ($_.CommandLine -match 'Kuro') -and
            ($_.CommandLine -match '(run_kuro|main)\.py')
        })
    foreach ($proc in $python) {
        Stop-Process -Id $proc.ProcessId -Force -ErrorAction SilentlyContinue
    }
    $lock = Join-Path $env:LOCALAPPDATA "Kuro\kuro.lock"
    if (Test-Path -LiteralPath $lock) {
        Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
    }
    $deadline = (Get-Date).AddSeconds(2)
    do {
        $engineOk = Test-FileWritable -Path $EngineOut
        $dllOk = Test-FileWritable -Path $CoreDll
        if ($engineOk -and $dllOk) {
            break
        }
        Start-Sleep -Milliseconds 100
    } while ((Get-Date) -lt $deadline)
}

function Ensure-WhisperGgml {
    $models = Join-Path $env:LOCALAPPDATA "Kuro\models"
    New-Item -ItemType Directory -Force -Path $models | Out-Null
    $ggml = Join-Path $models "ggml-small.en.bin"
    if (Test-Path -LiteralPath $ggml) {
        Write-Host ("Whisper GGML present: {0}" -f $ggml)
        return
    }
    $url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin"
    Write-Host ("Downloading Whisper GGML to {0}" -f $ggml)
    Invoke-WebRequest -Uri $url -OutFile $ggml
}

Write-Host "Stopping KuroShell, kuro-engine, and legacy Python Kuro"
Stop-KuroBuildLocks
Ensure-WhisperGgml

$HooksInstaller = Join-Path $Root "scripts\Install-GitHooks.ps1"
if (Test-Path -LiteralPath $HooksInstaller) {
    & $HooksInstaller
}

# Prefer portable LLVM. The Program Files MSI has been unreliable (exit 1603 / UAC abort).
$portableLlvm = Join-Path $env:LOCALAPPDATA "LLVM-portable\clang+llvm-18.1.8-x86_64-pc-windows-msvc\bin"
$candidates = @(
    $portableLlvm,
    "C:\Program Files\LLVM\bin"
)
if (-not $env:LIBCLANG_PATH) {
    foreach ($llvm in $candidates) {
        if (Test-Path -LiteralPath (Join-Path $llvm "libclang.dll")) {
            $env:LIBCLANG_PATH = $llvm
            break
        }
    }
}
if (-not $env:LIBCLANG_PATH -or -not (Test-Path -LiteralPath (Join-Path $env:LIBCLANG_PATH "libclang.dll"))) {
    Write-Error "libclang.dll not found. Install portable LLVM under $portableLlvm or set LIBCLANG_PATH."
    exit 1
}
Write-Host ("LIBCLANG_PATH={0}" -f $env:LIBCLANG_PATH)
$userLlvm = [Environment]::GetEnvironmentVariable("LIBCLANG_PATH", "User")
if ($userLlvm -ne $env:LIBCLANG_PATH) {
    [Environment]::SetEnvironmentVariable("LIBCLANG_PATH", $env:LIBCLANG_PATH, "User")
    Write-Host "Pinned LIBCLANG_PATH for this Windows user."
}

Write-Host "Building kuro-engine (release)"
Push-Location (Join-Path $Root "kuro-rs")
try {
    cargo build --manifest-path $Manifest --release -p kuro-engine
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
finally {
    Pop-Location
}

$Built = $EngineOut
$DestDir = Join-Path $env:LOCALAPPDATA "Kuro"
New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
Copy-Item -LiteralPath $Built -Destination (Join-Path $DestDir "kuro-engine.exe") -Force
Write-Host ("Engine: {0}" -f $Built)

Write-Host "Building KuroShell (Release x64)"
dotnet build $Shell -c Release -p:Platform=x64
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Building KuroShell (Debug x64)"
dotnet build $Shell -c Debug -p:Platform=x64
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
