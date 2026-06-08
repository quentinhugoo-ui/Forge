param(
    [int]$IntervalSeconds = 60,
    [int]$QuietSeconds = 20,
    [string]$MessagePrefix = "Forge autosnapshot"
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root

Write-Host "Forge autopush watching $Root" -ForegroundColor DarkCyan
Write-Host "Interval=$IntervalSeconds seconds Quiet=$QuietSeconds seconds" -ForegroundColor DarkCyan

$lastSignature = ""
$lastChangedAt = Get-Date

while ($true) {
    $statusText = (& git status --porcelain=v1) -join "`n"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "git status failed; retrying." -ForegroundColor Yellow
        Start-Sleep -Seconds $IntervalSeconds
        continue
    }

    if ([string]::IsNullOrWhiteSpace($statusText)) {
        $lastSignature = ""
        Start-Sleep -Seconds $IntervalSeconds
        continue
    }

    if ($statusText -ne $lastSignature) {
        $lastSignature = $statusText
        $lastChangedAt = Get-Date
        Start-Sleep -Seconds $IntervalSeconds
        continue
    }

    $quietFor = ((Get-Date) - $lastChangedAt).TotalSeconds
    if ($quietFor -lt $QuietSeconds) {
        Start-Sleep -Seconds $IntervalSeconds
        continue
    }

    $message = "$MessagePrefix $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
    & (Join-Path $PSScriptRoot "forge-snapshot.ps1") -Message $message
    if ($LASTEXITCODE -ne 0) {
        Write-Host "snapshot failed; leaving changes in working tree." -ForegroundColor Yellow
    }

    $lastSignature = ""
    Start-Sleep -Seconds $IntervalSeconds
}
