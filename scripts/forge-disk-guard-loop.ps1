param(
    [double]$MinFreeGB = 45,
    [double]$TargetFreeGB = 60,
    [int]$IntervalMinutes = 15
)

$ErrorActionPreference = "Stop"

$GuardScript = Join-Path $PSScriptRoot "forge-disk-guard.ps1"

while ($true) {
    try {
        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $GuardScript `
            -MinFreeGB $MinFreeGB `
            -TargetFreeGB $TargetFreeGB `
            -Apply
    } catch {
        Write-Host "Forge disk guard loop error: $($_.Exception.Message)" -ForegroundColor Yellow
    }
    Start-Sleep -Seconds ([Math]::Max(60, $IntervalMinutes * 60))
}
