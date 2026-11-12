#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# BAAI/bge-small-en-v1.5 ONNX + tokenizer into bench/models/router-embed
# and %LOCALAPPDATA%\Kuro\models\router-embed

$Root = Split-Path -Parent $PSScriptRoot
$Dest = Join-Path $env:LOCALAPPDATA "Kuro\models\router-embed"
$Bench = Join-Path $Root "bench\models\router-embed"
New-Item -ItemType Directory -Force -Path $Dest, $Bench | Out-Null

$Base = "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main"
$Files = @(
    @{ Url = "$Base/onnx/model_quantized.onnx"; Out = "model_quantized.onnx" },
    @{ Url = "$Base/onnx/model_int8.onnx"; Out = "model_int8.onnx" },
    @{ Url = "$Base/onnx/model.onnx"; Out = "model.onnx" },
    @{ Url = "$Base/vocab.txt"; Out = "vocab.txt" },
    @{ Url = "$Base/tokenizer.json"; Out = "tokenizer.json" },
    @{ Url = "$Base/tokenizer_config.json"; Out = "tokenizer_config.json" }
)

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
        Write-Warning "Skip (unavailable): $Url — $($_.Exception.Message)"
    }
}

foreach ($f in $Files) {
    Get-File $f.Url (Join-Path $Dest $f.Out)
}
Copy-Item -Force (Join-Path $Dest "*") $Bench -ErrorAction SilentlyContinue
Write-Host "Router embed ready at $Dest / $Bench"
