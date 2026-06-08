param(
    [string]$Message,
    [switch]$NoPush
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root

& (Join-Path $PSScriptRoot "forge-guard.ps1") -IncludeUntracked
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$status = & git status --porcelain=v1
if ($LASTEXITCODE -ne 0) {
    throw "git status failed"
}

if (-not $status -or $status.Count -eq 0) {
    Write-Host "Forge snapshot: no changes to commit." -ForegroundColor DarkGray
    exit 0
}

& git add -A
if ($LASTEXITCODE -ne 0) {
    throw "git add failed"
}

& (Join-Path $PSScriptRoot "forge-guard.ps1") -StagedOnly
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

if ([string]::IsNullOrWhiteSpace($Message)) {
    $Message = "Forge autosnapshot $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
}

& git commit -m $Message
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

if (-not $NoPush) {
    & git push
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

Write-Host "Forge snapshot complete." -ForegroundColor Green
