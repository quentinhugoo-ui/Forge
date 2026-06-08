param(
    [double]$MinFreeGB = 45,
    [double]$TargetFreeGB = 60,
    [int]$IntervalMinutes = 15,
    [string]$TaskName = "ForgeDiskGuard",
    [switch]$NoRunNow
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$GuardScript = Join-Path $PSScriptRoot "forge-disk-guard.ps1"
$LoopScript = Join-Path $PSScriptRoot "forge-disk-guard-loop.ps1"

if (-not (Test-Path -LiteralPath $GuardScript -PathType Leaf)) {
    throw "Missing guard script: $GuardScript"
}
if (-not (Test-Path -LiteralPath $LoopScript -PathType Leaf)) {
    throw "Missing guard loop script: $LoopScript"
}

& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $GuardScript -MinFreeGB $MinFreeGB -TargetFreeGB $TargetFreeGB
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$installed = $false
$startLoopNow = $false
try {
    $argument = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$GuardScript`" -MinFreeGB $MinFreeGB -TargetFreeGB $TargetFreeGB -Apply"
    $action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument $argument -WorkingDirectory $Root
    $logonTrigger = New-ScheduledTaskTrigger -AtLogOn
    $timeTrigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) `
        -RepetitionInterval (New-TimeSpan -Minutes $IntervalMinutes) `
        -RepetitionDuration (New-TimeSpan -Days 3650)
    $settings = New-ScheduledTaskSettingsSet `
        -AllowStartIfOnBatteries `
        -DontStopIfGoingOnBatteries `
        -StartWhenAvailable `
        -MultipleInstances IgnoreNew `
        -ExecutionTimeLimit (New-TimeSpan -Minutes 10)

    Register-ScheduledTask `
        -TaskName $TaskName `
        -Action $action `
        -Trigger @($logonTrigger, $timeTrigger) `
        -Settings $settings `
        -Description "Forge guard: keeps C: from falling below $MinFreeGB GB by deleting only whitelisted regenerable caches." `
        -Force | Out-Null

    $installed = $true
    Write-Host "Forge disk guard installed as scheduled task '$TaskName'." -ForegroundColor Green
} catch {
    Write-Host "Scheduled task install failed; installing HKCU Run fallback. $($_.Exception.Message)" -ForegroundColor Yellow
    $runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
    $runValue = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$LoopScript`" -MinFreeGB $MinFreeGB -TargetFreeGB $TargetFreeGB -IntervalMinutes $IntervalMinutes"
    New-Item -Path $runKey -Force | Out-Null
    New-ItemProperty -Path $runKey -Name $TaskName -Value $runValue -PropertyType String -Force | Out-Null
    $startLoopNow = $true
    $installed = $true
    Write-Host "Forge disk guard installed as HKCU Run fallback '$TaskName'." -ForegroundColor Green
}

if (-not $installed) {
    throw "Forge disk guard install failed"
}

Write-Host "Cleanup starts below $MinFreeGB GB and targets $TargetFreeGB GB." -ForegroundColor Green

if (-not $NoRunNow) {
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $GuardScript -MinFreeGB $MinFreeGB -TargetFreeGB $TargetFreeGB -Apply
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

if ($startLoopNow) {
    Start-Process -FilePath "powershell.exe" `
        -ArgumentList "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$LoopScript`" -MinFreeGB $MinFreeGB -TargetFreeGB $TargetFreeGB -IntervalMinutes $IntervalMinutes" `
        -WindowStyle Hidden
}
