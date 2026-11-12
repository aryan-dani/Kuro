#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Manifest = Join-Path $Root "kuro-rs\Cargo.toml"

Push-Location (Join-Path $Root "kuro-rs")
try {
    Write-Host "=== unload working set ==="
    cargo test --manifest-path $Manifest -p kuro-engine --test unload_working_set --release -- --nocapture 2>&1 |
        Select-String -Pattern "peak_ws_mb|after_unload|ok|FAILED"

    Write-Host "=== kokoro synth ==="
    cargo test --manifest-path $Manifest -p kuro-tts synthesizes_when_model_present --release -- --nocapture 2>&1 |
        Select-String -Pattern "ok|FAILED|synthesizes"

    Write-Host "=== stt warm ==="
    $sw = [Diagnostics.Stopwatch]::StartNew()
    cargo test --manifest-path $Manifest -p kuro-stt --test transcribe --release -- --nocapture 2>&1 |
        Select-String -Pattern "ok|FAILED|transcript"
    $sw.Stop()
    Write-Host ("stt_test_wall_ms={0:N1}" -f $sw.Elapsed.TotalMilliseconds)
}
finally {
    Pop-Location
}
