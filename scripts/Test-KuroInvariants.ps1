#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$failures = New-Object System.Collections.Generic.List[string]

function Add-Fail([string]$Message) {
    $failures.Add($Message) | Out-Null
}

function Get-TrackedAndStagedFiles {
    Push-Location $Root
    try {
        $tracked = @(git ls-files)
        $staged = @(git diff --cached --name-only --diff-filter=ACM)
        $untracked = @(git ls-files --others --exclude-standard)
        return ($tracked + $staged + $untracked | Select-Object -Unique)
    }
    finally {
        Pop-Location
    }
}

$emojiPattern = '[\u2600-\u27BF]|[\uD83C-\uDBFF][\uDC00-\uDFFF]'
$scanExtensions = @(".rs", ".cs", ".xaml", ".py", ".ps1", ".md", ".json", ".toml", ".yml", ".yaml")

foreach ($rel in Get-TrackedAndStagedFiles) {
    $ext = [System.IO.Path]::GetExtension($rel).ToLowerInvariant()
    if ($scanExtensions -notcontains $ext) { continue }
    $path = Join-Path $Root $rel
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
    $i = 0
    foreach ($line in [System.IO.File]::ReadAllLines($path)) {
        $i++
        if ($line -match $emojiPattern) {
            Add-Fail "Emoji at ${rel}:${i}"
        }
    }
}

$blockedNames = @(
    ".env", "api_token", "credentials.json", "token.json", "tuya_endpoint.json",
    "n8n_workflows.json", "contacts.json", "wa_session.db", "timing.csv",
    "heartbeat.txt", "home_ssid.txt", "legion_mode.json"
)
foreach ($rel in Get-TrackedAndStagedFiles) {
    $name = Split-Path -Leaf $rel
    if ($name -eq ".env.example") { continue }
    if ($blockedNames -contains $name) {
        Add-Fail "Secret or local-state file tracked: $rel"
    }
    if ($name -like "*.log" -or $name -like "*.wav") {
        Add-Fail "Ignored artifact tracked: $rel"
    }
}

$secretPatterns = @(
    '(?i)Bearer\s+[A-Za-z0-9\-\._~\+\/]{24,}',
    '(?i)api[_-]?token\s*=\s*["''][^"'']{16,}["'']',
    '(?i)(?:Tuya|tuya)[_-]?local[_-]?key\s*=\s*["''][^"'']{8,}["'']'
)
foreach ($rel in Get-TrackedAndStagedFiles) {
    $ext = [System.IO.Path]::GetExtension($rel).ToLowerInvariant()
    if ($scanExtensions -notcontains $ext) { continue }
    if ($rel -match '(^|/|\\)\.env\.example$') { continue }
    if ($rel -match '(^docs/|/docs/|\\docs\\)') { continue }
    if ($rel -eq "scripts/Test-KuroInvariants.ps1") { continue }
    $path = Join-Path $Root $rel
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
    $text = Get-Content -LiteralPath $path -Raw -ErrorAction SilentlyContinue
    if (-not $text) { continue }
    foreach ($pat in $secretPatterns) {
        if ($text -match $pat) {
            Add-Fail "Possible secret leak in $rel"
            break
        }
    }
}

$tokensPath = Join-Path $Root "KuroShell\KuroShell\DesignTokens.cs"
if (-not (Test-Path -LiteralPath $tokensPath)) {
    Add-Fail "Missing DesignTokens.cs"
}
else {
    $tokens = Get-Content -LiteralPath $tokensPath -Raw
    $required = @(
        "245, 244, 238", "255, 255, 255", "20, 20, 20", "82, 81, 78",
        "RadiusRow => 0", "ChatRadius { get; } = 22",
        "10, 10, 10", "240, 240, 240", "163, 163, 163",
        "SpaceGrotesk", "JetBrainsMono", "Poppins"
    )
    foreach ($needle in $required) {
        if ($tokens -notmatch [regex]::Escape($needle)) {
            Add-Fail "DesignTokens missing brand lock: $needle"
        }
    }
    $fontsDir = Join-Path $Root "KuroShell\KuroShell\Assets\Fonts"
    foreach ($font in @("SpaceGrotesk-Regular.ttf", "JetBrainsMono-Regular.ttf", "Poppins-Regular.ttf")) {
        if (-not (Test-Path -LiteralPath (Join-Path $fontsDir $font))) {
            Add-Fail "Missing font asset: $font"
        }
    }
}

$capsulePath = Join-Path $Root "KuroShell\KuroShell\NativeCapsule.cs"
$hudPath = Join-Path $Root "KuroShell\KuroShell\HudWindow.cs"
if (Test-Path -LiteralPath $hudPath) {
    $hud = Get-Content -LiteralPath $hudPath -Raw
    if ($hud -match 'void Tick\(\)[\s\S]*?AppWindow\.(Move|Resize)') {
        Add-Fail "HudWindow Tick moves or resizes the dock"
    }
}
if (Test-Path -LiteralPath $capsulePath) {
    $cap = Get-Content -LiteralPath $capsulePath -Raw
    if ($cap -match 'void Paint\([\s\S]*SetWindowPos\([^)]*place\.') {
        Add-Fail "NativeCapsule Paint moves anchor position"
    }
}

$shellSources = Get-ChildItem -Path (Join-Path $Root "KuroShell") -Recurse -Include *.cs -File -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -notmatch '\\obj\\|\\bin\\' }
foreach ($file in $shellSources) {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    if ($text -match 'RegisterHotKey' -and $text -match 'Ctrl\+Alt\+Space|VK_SPACE|0x20') {
        Add-Fail "KuroShell registers competing Ctrl+Alt+Space hotkey: $($file.FullName)"
    }
}

$allowedPyPatterns = @(
    '(^|[\\/])legacy[\\/]python[\\/]',
    '(^|[\\/])scripts[\\/]bench[\\/]',
    '(^|[\\/])scripts[\\/]setup_autostart\.py$',
    '(^|[\\/])scripts[\\/]build_icons\.py$'
)
foreach ($rel in Get-TrackedAndStagedFiles) {
    if ($rel -notmatch '\.py$') { continue }
    $allowed = $false
    foreach ($pat in $allowedPyPatterns) {
        if ($rel -match $pat) { $allowed = $true; break }
    }
    if (-not $allowed) {
        Add-Fail "Python outside legacy/python quarantine: $rel"
    }
}
if (Test-Path -LiteralPath (Join-Path $Root "tests")) {
  $rootTests = Get-ChildItem -Path (Join-Path $Root "tests") -File -Recurse -ErrorAction SilentlyContinue |
      Where-Object { $_.Extension -eq '.py' }
  if ($rootTests) {
      Add-Fail "tests/ still contains Python at repo root (use legacy/python/tests)"
  }
}

if ($failures.Count -gt 0) {
    Write-Host "[FAIL] Test-KuroInvariants ($($failures.Count) issues)"
    foreach ($item in $failures) { Write-Host "  $item" }
    exit 1
}

Write-Host "[PASS] Test-KuroInvariants"
exit 0
