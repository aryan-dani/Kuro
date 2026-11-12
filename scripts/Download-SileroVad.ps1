#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$Dest = Join-Path $env:LOCALAPPDATA "Kuro\models\silero-vad"
$Bench = Join-Path $Root "bench\models\silero-vad"
New-Item -ItemType Directory -Force -Path $Dest, $Bench | Out-Null

$Urls = @(
    "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx",
    "https://huggingface.co/onnx-community/silero-vad/resolve/main/onnx/model.onnx"
)

$Out = Join-Path $Dest "silero_vad.onnx"
if (Test-Path -LiteralPath $Out) {
    Write-Host "Already present: $Out"
}
else {
    $ok = $false
    foreach ($url in $Urls) {
        Write-Host "Downloading $url"
        try {
            Invoke-WebRequest -Uri $url -OutFile "$Out.partial" -UseBasicParsing
            Move-Item -Force "$Out.partial" $Out
            $ok = $true
            break
        }
        catch {
            Remove-Item -Force "$Out.partial" -ErrorAction SilentlyContinue
            Write-Warning $_.Exception.Message
        }
    }
    if (-not $ok) { throw "Could not download silero_vad.onnx" }
}

Copy-Item -Force $Out (Join-Path $Bench "silero_vad.onnx")
Write-Host "Silero VAD ready at $Dest / $Bench"
