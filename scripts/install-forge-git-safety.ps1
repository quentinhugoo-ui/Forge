param(
    [switch]$NoSnapshot
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root

& git config core.hooksPath .githooks
if ($LASTEXITCODE -ne 0) {
    throw "git config core.hooksPath failed"
}

& (Join-Path $PSScriptRoot "forge-guard.ps1") -IncludeUntracked
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Write-Host "Forge Git safety installed: core.hooksPath=.githooks" -ForegroundColor Green

if (-not $NoSnapshot) {
    & (Join-Path $PSScriptRoot "forge-snapshot.ps1") -Message "Install Forge Git safety"
    exit $LASTEXITCODE
}
