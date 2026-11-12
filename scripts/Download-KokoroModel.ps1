#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# Kokoro-82M v1.0 ONNX + voice bins into bench/models/kokoro and LOCALAPPDATA.

$Root = Split-Path -Parent $PSScriptRoot
$Dest = Join-Path $env:LOCALAPPDATA "Kuro\models\kokoro"
$VoicesDest = Join-Path $Dest "voices"
$Bench = Join-Path $Root "bench\models\kokoro"
$BenchVoices = Join-Path $Bench "voices"
New-Item -ItemType Directory -Force -Path $Dest, $VoicesDest, $Bench, $BenchVoices | Out-Null

$Base = "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/main"
# Prefer quantized for CPU footprint; fall back to fp32.
$Models = @(
    @{ Url = "$Base/onnx/model_quantized.onnx"; Out = "model_quantized.onnx" },
    @{ Url = "$Base/onnx/model.onnx"; Out = "model.onnx" },
    @{ Url = "$Base/config.json"; Out = "config.json" },
    @{ Url = "$Base/tokenizer.json"; Out = "tokenizer.json" }
)
$VoiceNames = @("af_heart", "af_bella", "af_nicole", "af_sarah", "bf_emma", "af")

function Get-File([string]$Url, [string]$Out) {
    if (Test-Path -LiteralPath $Out) {
        Write-Host "Already present: $Out"
        return
    }
    Write-Host "Downloading $Url"
    try {
        Invoke-WebRequest -Uri $Url -OutFile "$Out.partial" -UseBasicParsing
        Move-Item -Force "$Out.partial" $Out
    }
    catch {
        Remove-Item -Force "$Out.partial" -ErrorAction SilentlyContinue
        Write-Warning "Skip: $Url — $($_.Exception.Message)"
    }
}

foreach ($m in $Models) {
    Get-File $m.Url (Join-Path $Dest $m.Out)
}
foreach ($name in $VoiceNames) {
    Get-File "$Base/voices/$name.bin" (Join-Path $VoicesDest "$name.bin")
}

# Prefer quantized as model.onnx alias if fp32 missing.
$q = Join-Path $Dest "model_quantized.onnx"
$fp = Join-Path $Dest "model.onnx"
if ((Test-Path $q) -and -not (Test-Path $fp)) {
    Copy-Item -Force $q $fp
}

Copy-Item -Force (Join-Path $Dest "model*.onnx") $Bench -ErrorAction SilentlyContinue
Copy-Item -Force (Join-Path $Dest "config.json") $Bench -ErrorAction SilentlyContinue
Copy-Item -Force (Join-Path $Dest "tokenizer.json") $Bench -ErrorAction SilentlyContinue
Copy-Item -Force (Join-Path $VoicesDest "*.bin") $BenchVoices -ErrorAction SilentlyContinue
Write-Host "Kokoro ready at $Dest / $Bench"
