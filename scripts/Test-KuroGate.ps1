#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$Manifest = Join-Path $Root "kuro-rs\Cargo.toml"
$EngineRelease = Join-Path $Root "kuro-rs\target\release\kuro-engine.exe"
$GateTarget = Join-Path $Root "kuro-rs\target\gate"

function Write-Stage([string]$Name, [bool]$Ok, [double]$Seconds) {
    $tag = if ($Ok) { "[PASS]" } else { "[FAIL]" }
    $line = "{0} {1} ({2:N1}s)" -f $tag, $Name, $Seconds
    Write-Host $line
    if (-not $Ok) { exit 1 }
}

function Ensure-LibClang {
    if ($env:LIBCLANG_PATH -and (Test-Path -LiteralPath $env:LIBCLANG_PATH)) { return }
    $candidates = @(
        "C:\Program Files\LLVM\bin",
        (Join-Path $env:LOCALAPPDATA "LLVM-portable\clang+llvm-18.1.8-x86_64-pc-windows-msvc\bin"),
        (Join-Path $env:LOCALAPPDATA "LLVM-portable\llvm\bin"),
        (Join-Path $env:LOCALAPPDATA "LLVM\bin")
    )
    Get-ChildItem (Join-Path $env:LOCALAPPDATA "LLVM-portable") -Recurse -Filter libclang.dll -ErrorAction SilentlyContinue |
        ForEach-Object { Split-Path $_.FullName -Parent } |
        ForEach-Object { if ($candidates -notcontains $_) { $candidates += $_ } }
    foreach ($dir in $candidates) {
        if (Test-Path -LiteralPath (Join-Path $dir "libclang.dll")) {
            $env:LIBCLANG_PATH = $dir
            return
        }
    }
}

function Invoke-Stage {
    param([string]$Name, [scriptblock]$Action)
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        & $Action
        Write-Stage -Name $Name -Ok $true -Seconds $sw.Elapsed.TotalSeconds
    }
    catch {
        Write-Host "[FAIL] $Name ($([math]::Round($sw.Elapsed.TotalSeconds, 1))s)"
        Write-Host $_.Exception.Message
        exit 1
    }
}

Ensure-LibClang

Invoke-Stage "1 cargo xtask gate full" {
    Push-Location (Join-Path $Root "kuro-rs")
  try {
        cargo xtask gate full
        if ($LASTEXITCODE -ne 0) { throw "cargo xtask gate full failed" }
    }
    finally {
        Pop-Location
    }
}

Invoke-Stage "2 live smoke kuro-engine" {
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $smokeEngine = $EngineRelease
    if ($env:CI -or -not (Test-Path -LiteralPath $EngineRelease)) {
        Push-Location (Join-Path $Root "kuro-rs")
        try {
            $env:CARGO_TARGET_DIR = $GateTarget
            cargo build --manifest-path $Manifest -p kuro-engine
            if ($LASTEXITCODE -ne 0) { throw "engine build failed" }
        }
        finally {
            Pop-Location
            Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
        }
        $smokeEngine = Join-Path $GateTarget "release\kuro-engine.exe"
        if (-not (Test-Path -LiteralPath $smokeEngine)) {
            $smokeEngine = Join-Path $GateTarget "debug\kuro-engine.exe"
        }
    }
    else {
        $smokeEngine = Join-Path $env:TEMP ("kuro-gate-smoke-{0}.exe" -f [guid]::NewGuid().ToString("N"))
        Copy-Item -LiteralPath $EngineRelease -Destination $smokeEngine -Force
    }
    $ErrorActionPreference = $prevEap
    if (-not (Test-Path -LiteralPath $smokeEngine)) {
        throw "smoke engine missing at $smokeEngine"
    }
    $data = Join-Path $env:TEMP ("Kuro-gate-{0}" -f [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $data | Out-Null
    New-Item -ItemType File -Force -Path (Join-Path $data "AWAY_OK") | Out-Null
    $bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    $token = ($bytes | ForEach-Object { $_.ToString("x2") }) -join ""
    Set-Content -Path (Join-Path $data "api_token") -Value $token -NoNewline
    $env:KURO_DATA_DIR = $data
    $env:KURO_API = "on"
    $env:KURO_API_PORT = "18799"
    $env:KURO_GATE_CI = "1"
    $env:KURO_SAY_EXECUTE = "1"
    $proc = Start-Process -FilePath $smokeEngine -PassThru -WindowStyle Hidden
    Start-Sleep -Seconds 2
    try {
        $headers = @{ "x-kuro-token" = $token }
        $status = Invoke-RestMethod -Uri "http://127.0.0.1:18799/status" -Headers $headers -Method Get
        if (-not $status.ok) { throw "status not ok" }
        $say = Invoke-RestMethod -Uri "http://127.0.0.1:18799/say" -Headers $headers -Method Post `
            -ContentType "application/json" -Body '{"text":"who are you","mode":"execute"}'
        if ($say.reply -notlike "*Kuro. Local assistant. Phrases first, model last.*") {
            throw "unexpected say reply: $($say.reply)"
        }
        Invoke-RestMethod -Uri "http://127.0.0.1:18799/shutdown" -Headers $headers -Method Post -Body "{}" | Out-Null
        $proc.WaitForExit(3000)
        if (-not $proc.HasExited) {
            Stop-Process -Id $proc.Id -Force
            throw "engine did not exit within 3s"
        }
        if ($proc.ExitCode -ne 0) { throw "engine exit code $($proc.ExitCode)" }
    }
    finally {
        if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
        Remove-Item -LiteralPath $data -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item Env:KURO_DATA_DIR -ErrorAction SilentlyContinue
        Remove-Item Env:KURO_GATE_CI -ErrorAction SilentlyContinue
        if ($smokeEngine -like "$env:TEMP\*") {
            Remove-Item -LiteralPath $smokeEngine -Force -ErrorAction SilentlyContinue
        }
    }
}

Write-Host "[PASS] Test-KuroGate (all stages)"
Write-Host "Note: this gate does not stop or restart your daily Kuro process."
exit 0
