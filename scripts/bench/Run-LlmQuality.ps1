# Offline Ollama quality and latency check. Does not start Kuro or call tools.
param(
    [Parameter(Mandatory = $true)][string]$Model,
    [int]$Runs = 3
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Prompts = Join-Path $Root "bench\llm_quality.jsonl"
$OutDir = Join-Path $Root "bench\data\llm"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$safe = $Model -replace "[:\\/]", "_"
$out = Join-Path $OutDir "$safe.jsonl"

$system = "You are Kuro, a local Windows voice assistant. Answer in one or two short spoken sentences. No lists, no numbered steps, no markdown, no emoji. You cannot send messages, open apps, or control devices. If you do not know a live or private fact, say you do not know. Do not invent success."
$rows = Get-Content -LiteralPath $Prompts | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json }

function Invoke-Chat($messages) {
    $body = @{
        model = $Model
        messages = $messages
        stream = $true
        think = $false
        keep_alive = "30m"
        options = @{ num_ctx = 4096; temperature = 0.2 }
    } | ConvertTo-Json -Depth 8
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $first = $null
    $sentence = $null
    $text = ""
    $procLine = $null
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = "curl.exe"
    $psi.Arguments = "-sN http://127.0.0.1:11434/api/chat -H `"Content-Type: application/json`" --data-binary `@-"
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $proc = [Diagnostics.Process]::Start($psi)
    $proc.StandardInput.Write($body)
    $proc.StandardInput.Close()
    while (-not $proc.StandardOutput.EndOfStream) {
        $line = $proc.StandardOutput.ReadLine()
        if (-not $line) { continue }
        $obj = $line | ConvertFrom-Json
        $piece = $obj.message.content
        if ($piece) {
            if (-not $first) { $first = $sw.Elapsed.TotalMilliseconds }
            $text += $piece
            if (-not $sentence -and $text -match "[.!?]") { $sentence = $sw.Elapsed.TotalMilliseconds }
        }
        if ($obj.psobject.Properties.Name -contains "done" -and $obj.done) { break }
    }
    $proc.WaitForExit()
    $eval = $sw.Elapsed.TotalMilliseconds
    $ps = ollama ps --format json 2>$null | Out-String
    [pscustomobject]@{
        ms_first = $first
        ms_sentence = $sentence
        ms_done = $eval
        text = $text
        ps = $ps
    }
}

$history = @(@{ role = "system"; content = $system })
foreach ($row in $rows) {
    for ($run = 1; $run -le $Runs; $run++) {
        if ($row.id -like "mem-*") {
            $messages = @($history + @(@{ role = "user"; content = $row.prompt }))
        }
        else {
            $messages = @(
                @{ role = "system"; content = $system },
                @{ role = "user"; content = $row.prompt }
            )
        }
        $result = Invoke-Chat $messages
        if ($row.id -like "mem-*" -and $run -eq 1) {
            $history += @{ role = "user"; content = $row.prompt }
            $history += @{ role = "assistant"; content = $result.text }
        }
        $record = [pscustomobject]@{
            model = $Model
            id = $row.id
            kind = $row.kind
            run = $run
            ms_first = $result.ms_first
            ms_sentence = $result.ms_sentence
            ms_done = $result.ms_done
            chars = $result.text.Length
            ends_q = $result.text.Trim().EndsWith("?")
            text = $result.text
            ps = $result.ps
        }
        ($record | ConvertTo-Json -Compress) | Add-Content -LiteralPath $out
        Write-Output ("{0} {1} run {2} first={3:N0}ms" -f $Model, $row.id, $run, $result.ms_first)
    }
}
Write-Output "wrote $out"
