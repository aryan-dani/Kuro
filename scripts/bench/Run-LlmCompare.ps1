# Compare local Ollama models. Does not start Kuro or call Kuro tools.
param(
    [int]$Runs = 3
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$PromptFile = Join-Path $Root "bench\llm_quality.jsonl"
$OutDir = Join-Path $Root "bench\data\llm"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$Out = Join-Path $OutDir "compare.jsonl"
if (Test-Path $Out) { Remove-Item $Out -Force }

$System = "You are Kuro, a local Windows voice assistant. Answer in one or two short spoken sentences. No lists, no numbered steps, no markdown, no emoji. You cannot send messages, open apps, or control devices. If you do not know a live or private fact, say you do not know. Do not invent success."
$Models = @("qwen2.5:7b-instruct", "qwen3.5:4b", "gemma4:e4b")
$Rows = @(Get-Content -LiteralPath $PromptFile | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json })

function Invoke-StreamChat {
    param($Model, $Messages)
    $payload = @{
        model = $Model
        messages = $Messages
        stream = $false
        think = $false
        keep_alive = "30m"
        options = @{ num_ctx = 4096; temperature = 0.2 }
    } | ConvertTo-Json -Depth 8
    $resp = Invoke-RestMethod -Uri "http://127.0.0.1:11434/api/chat" -Method Post -ContentType "application/json" -Body $payload -TimeoutSec 180
    $evalMs = [double]$resp.eval_duration / 1000000.0
    $promptMs = [double]$resp.prompt_eval_duration / 1000000.0
    $loadMs = [double]$resp.load_duration / 1000000.0
    $totalMs = [double]$resp.total_duration / 1000000.0
    [pscustomobject]@{
        ms_first = $loadMs + $promptMs
        ms_sentence = $totalMs
        ms_done = $totalMs
        eval_ms = $evalMs
        text = [string]$resp.message.content
    }
}

function New-Messages {
    param($History, $User)
    $list = @(@{ role = "system"; content = $System })
    foreach ($item in $History) { $list += $item }
    $list += @{ role = "user"; content = $User }
    return $list
}

foreach ($model in $Models) {
    Write-Output "model $model"
    $null = Invoke-StreamChat -Model $model -Messages @(New-Messages @() "Say hello in one sentence.")
    $ps = (ollama ps | Out-String).Trim()
    $memRuns = 1..$Runs
    foreach ($run in $memRuns) {
        $history = @()
        foreach ($row in $Rows) {
            if ($row.id -notlike "mem-*") { $history = @() }
            $messages = New-Messages $history $row.prompt
            $result = Invoke-StreamChat -Model $model -Messages $messages
            if ($row.id -like "mem-*") {
                $history += @{ role = "user"; content = $row.prompt }
                $history += @{ role = "assistant"; content = $result.text }
            }
            $record = [pscustomobject]@{
                model = $model
                id = $row.id
                kind = $row.kind
                run = $run
                ms_first = $result.ms_first
                ms_sentence = $result.ms_sentence
                ms_done = $result.ms_done
                chars = $result.text.Length
                ends_q = $result.text.Trim().EndsWith("?")
                text = $result.text
                ps = $ps
            }
            ($record | ConvertTo-Json -Compress) | Add-Content -LiteralPath $Out
            Write-Output ("{0} {1} r{2} first={3} chars={4}" -f $model, $row.id, $run, [int]$result.ms_first, $result.text.Length)
        }
    }
}
Write-Output "wrote $Out"
