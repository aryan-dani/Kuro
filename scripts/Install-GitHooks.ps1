#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Push-Location $Root
try {
    git config core.hooksPath .githooks
    $configured = git config --get core.hooksPath
    if ($configured -ne ".githooks") {
        throw "core.hooksPath is '$configured', expected '.githooks'"
    }
    foreach ($name in @("pre-commit", "pre-push")) {
        $hook = Join-Path $Root ".githooks\$name"
        if (-not (Test-Path -LiteralPath $hook)) {
            throw "Missing $name hook at $hook"
        }
    }
    Write-Host "Git hooks installed: core.hooksPath=.githooks"
}
finally {
    Pop-Location
}
