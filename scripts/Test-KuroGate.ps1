#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$Manifest = Join-Path $Root "kuro-rs\Cargo.toml"
$ShellProj = Join-Path $Root "KuroShell\KuroShell\KuroShell.csproj"
$ShellTests = Join-Path $Root "KuroShell\KuroShell.Tests\KuroShell.Tests.csproj"
$EngineRelease = Join-Path $Root "kuro-rs\target\release\kuro-engine.exe"
$Invariants = Join-Path $Root "scripts\Test-KuroInvariants.ps1"
$VenvPy = Join-Path $Root ".venv\Scripts\python.exe"

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

function Stop-LockingKuro {
    Get-Process -Name KuroShell, kuro-engine -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
    $lock = Join-Path $env:LOCALAPPDATA "Kuro\kuro.lock"
    if (Test-Path -LiteralPath $lock) {
        Remove-Item -LiteralPath $lock -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Milliseconds 300
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

Invoke-Stage "1 Process lock clean" {
    Stop-LockingKuro
}

Invoke-Stage "2 Test-KuroInvariants" {
    & $Invariants
    if ($LASTEXITCODE -ne 0) { throw "invariants failed" }
}

Invoke-Stage "3 cargo fmt + clippy" {
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        Push-Location $Root
        cargo fmt --manifest-path $Manifest --all -- --check 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "fmt check failed" }
        cargo clippy --manifest-path $Manifest --workspace --all-targets -- -D warnings 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "clippy failed" }
    }
    finally {
        Pop-Location
        $ErrorActionPreference = $prevEap
    }
}

Invoke-Stage "4 cargo test" {
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        Push-Location $Root
        $log = Join-Path $env:TEMP ("kuro-gate-cargo-test-{0}.log" -f [guid]::NewGuid().ToString("N"))
        cargo test --manifest-path $Manifest --workspace -- --nocapture *> $log
        $code = $LASTEXITCODE
        if ($code -ne 0) {
            Write-Host "----- cargo test tail -----"
            Get-Content -LiteralPath $log -Tail 80 -ErrorAction SilentlyContinue
            Write-Host "----- end cargo test tail -----"
            throw "cargo test failed"
        }
    }
    finally {
        Pop-Location
        $ErrorActionPreference = $prevEap
    }
}

Invoke-Stage "5 dotnet build + test" {
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        dotnet build $ShellProj -c Debug -p:Platform=x64 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "shell build failed" }
        dotnet test $ShellTests -c Debug -p:Platform=x64 --no-build 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "shell tests failed" }
    }
    finally {
        $ErrorActionPreference = $prevEap
    }
}

Invoke-Stage "6 pytest" {
    if (-not (Test-Path -LiteralPath $VenvPy)) {
        Write-Host "  (skip: no .venv)"
        return
    }
    & $VenvPy -m pytest (Join-Path $Root "tests") -q 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "pytest failed" }
}

Invoke-Stage "7 live smoke kuro-engine" {
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    cargo build --manifest-path $Manifest --release -p kuro-engine 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "engine build failed" }
    $ErrorActionPreference = $prevEap
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
    $proc = Start-Process -FilePath $EngineRelease -PassThru -WindowStyle Hidden
    Start-Sleep -Seconds 2
    try {
        $headers = @{ "x-kuro-token" = $token }
        $status = Invoke-RestMethod -Uri "http://127.0.0.1:18799/status" -Headers $headers -Method Get
        if (-not $status.ok) { throw "status not ok" }
        $say = Invoke-RestMethod -Uri "http://127.0.0.1:18799/say" -Headers $headers -Method Post `
            -ContentType "application/json" -Body '{"text":"who are you"}'
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
    }
}

Write-Host "[PASS] Test-KuroGate (all stages)"
exit 0
