param(
    [switch]$StagedOnly,
    [switch]$IncludeUntracked
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ProtectedPrefixes = @(
    (Join-Path $env:USERPROFILE "Documents\EVE\MAP")
) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -ErrorAction SilentlyContinue) } |
    ForEach-Object { (Resolve-Path -LiteralPath $_).Path }

$GuardedExtensions = @(
    ".rs", ".slint", ".toml", ".lock", ".ps1", ".cmd", ".sh", ".md",
    ".json", ".jsonl", ".mjs", ".js", ".ts", ".tsx", ".py", ".td",
    ".wgsl", ".sql", ".html", ".css"
)

$AllowedEmptyNames = @(
    ".gitkeep",
    ".keep"
)

function Invoke-Git {
    param(
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$Args
    )
    $output = & git @Args
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Args -join ' ') failed"
    }
    return $output
}

function Split-Nul([string]$Text) {
    if ([string]::IsNullOrEmpty($Text)) { return @() }
    return $Text -split "`0" | Where-Object { $_.Length -gt 0 }
}

function Test-GuardedPath([string]$PathText) {
    $name = [System.IO.Path]::GetFileName($PathText)
    if ($AllowedEmptyNames -contains $name) { return $false }

    $extension = [System.IO.Path]::GetExtension($PathText).ToLowerInvariant()
    return $GuardedExtensions -contains $extension
}

function Assert-InRepo([string]$PathText) {
    $full = [System.IO.Path]::GetFullPath((Join-Path $Root $PathText))
    if (-not $full.StartsWith($Root, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing path outside repo: $PathText"
    }
    foreach ($protected in $ProtectedPrefixes) {
        if ($full.StartsWith($protected, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing protected path: $full"
        }
    }
    return $full
}

Set-Location $Root

$failures = [System.Collections.Generic.List[string]]::new()

if (-not $StagedOnly) {
    $tracked = Split-Nul (Invoke-Git "ls-files" "-z")
    foreach ($path in $tracked) {
        if (-not (Test-GuardedPath $path)) { continue }
        $full = Assert-InRepo $path
        if ((Test-Path -LiteralPath $full -PathType Leaf) -and ((Get-Item -LiteralPath $full).Length -eq 0)) {
            [void]$failures.Add("working-tree zero-byte tracked source: $path")
        }
    }
}

$indexEntries = Split-Nul (Invoke-Git "ls-files" "-s" "-z")
foreach ($entry in $indexEntries) {
    if ($entry -notmatch '^\d+\s+([0-9a-f]{40,64})\s+\d+\s+(.+)$') { continue }
    $sha = $Matches[1]
    $path = $Matches[2]
    if (-not (Test-GuardedPath $path)) { continue }

    $sizeText = Invoke-Git "cat-file" "-s" $sha
    $size = [int64]($sizeText | Select-Object -First 1)
    if ($size -eq 0) {
        [void]$failures.Add("index zero-byte tracked source: $path")
    }
}

if ($IncludeUntracked) {
    $untracked = Split-Nul (Invoke-Git "ls-files" "--others" "--exclude-standard" "-z")
    foreach ($path in $untracked) {
        if (-not (Test-GuardedPath $path)) { continue }
        $full = Assert-InRepo $path
        if ((Test-Path -LiteralPath $full -PathType Leaf) -and ((Get-Item -LiteralPath $full).Length -eq 0)) {
            [void]$failures.Add("working-tree zero-byte untracked source: $path")
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Host "Forge guard blocked this operation:" -ForegroundColor Red
    foreach ($failure in $failures) {
        Write-Host " - $failure" -ForegroundColor Red
    }
    Write-Host "Recover the file before committing or pushing." -ForegroundColor Yellow
    exit 2
}

Write-Host "Forge guard passed: no zero-byte guarded source files." -ForegroundColor Green
