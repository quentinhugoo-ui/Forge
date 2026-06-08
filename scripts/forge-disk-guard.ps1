param(
    [string]$DriveName = "C",
    [double]$MinFreeGB = 45,
    [double]$TargetFreeGB = 60,
    [double]$CriticalFreeGB = 15,
    [int]$TempOlderThanHours = 24,
    [switch]$Apply
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$UserProfile = $env:USERPROFILE
$Now = Get-Date
$LogDir = Join-Path $env:LOCALAPPDATA "ForgeDiskGuard"
$ProtectedPrefixes = @(
    (Join-Path $UserProfile "Documents\EVE\MAP"),
    $Root,
    (Join-Path $Root ".git")
)

function Get-FreeGB {
    $drive = [System.IO.DriveInfo]::GetDrives() |
        Where-Object { $_.Name.TrimEnd('\') -ieq "$DriveName`:" } |
        Select-Object -First 1
    if ($null -eq $drive) {
        throw "Drive not found: $DriveName"
    }
    return [math]::Round($drive.AvailableFreeSpace / 1GB, 2)
}

function Resolve-ExistingPath([string]$PathText) {
    if ([string]::IsNullOrWhiteSpace($PathText)) { return $null }
    if (-not (Test-Path -LiteralPath $PathText -ErrorAction SilentlyContinue)) { return $null }
    return (Resolve-Path -LiteralPath $PathText).Path
}

function Assert-SafeCleanupPath([string]$PathText) {
    $resolved = Resolve-ExistingPath $PathText
    if (-not $resolved) { return $null }

    foreach ($protected in $ProtectedPrefixes) {
        $protectedResolved = Resolve-ExistingPath $protected
        if ($protectedResolved -and $resolved.StartsWith($protectedResolved, [System.StringComparison]::OrdinalIgnoreCase)) {
            if ($resolved -notlike "$Root\.codex-targets*" -and
                $resolved -notlike "$Root\.codex-tmp*" -and
                $resolved -notlike "$Root\target-fuzz-local*" -and
                $resolved -notlike "$Root\examples\ingen_native_front\target-check*" -and
                $resolved -notlike "$Root\examples\ingen_native_front\target-desktop*") {
                throw "Refusing protected cleanup path: $resolved"
            }
        }
    }

    if ($resolved -match '\\\.git(\\|$)') {
        throw "Refusing .git cleanup path: $resolved"
    }
    if ($resolved -match '\\Documents\\EVE\\MAP(\\|$)') {
        throw "Refusing EVE MAP cleanup path: $resolved"
    }

    return $resolved
}

function Get-SizeBytes([string]$PathText) {
    if (-not (Test-Path -LiteralPath $PathText -ErrorAction SilentlyContinue)) { return 0 }
    if (Test-Path -LiteralPath $PathText -PathType Leaf) {
        return (Get-Item -LiteralPath $PathText).Length
    }
    try {
        $sum = (Get-ChildItem -LiteralPath $PathText -Recurse -Force -File -ErrorAction SilentlyContinue |
            Measure-Object -Property Length -Sum).Sum
        if ($null -eq $sum) { return 0 }
        return [int64]$sum
    } catch {
        Write-Host "skip size scan: $PathText" -ForegroundColor DarkYellow
        return 0
    }
}

function Remove-PathContents([string]$PathText, [bool]$OnlyOldTemp) {
    if (-not (Test-Path -LiteralPath $PathText -PathType Container)) { return }
    $items = Get-ChildItem -LiteralPath $PathText -Force -ErrorAction SilentlyContinue
    if ($OnlyOldTemp) {
        $cutoff = $Now.AddHours(-1 * $TempOlderThanHours)
        $items = $items | Where-Object { $_.LastWriteTime -lt $cutoff }
    }
    foreach ($item in $items) {
        try {
            Remove-Item -LiteralPath $item.FullName -Recurse -Force -ErrorAction Stop
        } catch {
            Write-Host "skip locked: $($item.FullName)" -ForegroundColor DarkYellow
        }
    }
}

function Invoke-CleanupCandidate([object]$Candidate) {
    $resolved = Assert-SafeCleanupPath $Candidate.Path
    if (-not $resolved) {
        return [pscustomobject]@{
            Tier = $Candidate.Tier
            Name = $Candidate.Name
            Path = $Candidate.Path
            Status = "missing"
            BeforeGB = 0
            AfterGB = 0
            FreedGB = 0
        }
    }

    $before = Get-SizeBytes $resolved
    if ($Apply -and $before -gt 0) {
        if ($Candidate.Mode -eq "Contents") {
            Remove-PathContents $resolved ([bool]$Candidate.OnlyOldTemp)
        } elseif ($Candidate.Mode -eq "Directory") {
            try {
                Remove-Item -LiteralPath $resolved -Recurse -Force -ErrorAction Stop
            } catch {
                Write-Host "skip locked: $resolved" -ForegroundColor DarkYellow
                Remove-PathContents $resolved $false
            }
        } else {
            throw "Unknown cleanup mode: $($Candidate.Mode)"
        }
    }
    $after = Get-SizeBytes $resolved

    return [pscustomobject]@{
        Tier = $Candidate.Tier
        Name = $Candidate.Name
        Path = $resolved
        Status = if ($Apply) { "cleaned" } else { "would-clean" }
        BeforeGB = [math]::Round($before / 1GB, 2)
        AfterGB = [math]::Round($after / 1GB, 2)
        FreedGB = [math]::Round(($before - $after) / 1GB, 2)
    }
}

function New-Candidate([int]$Tier, [string]$Name, [string]$Path, [string]$Mode, [bool]$OnlyOldTemp = $false) {
    [pscustomobject]@{
        Tier = $Tier
        Name = $Name
        Path = $Path
        Mode = $Mode
        OnlyOldTemp = $OnlyOldTemp
    }
}

$candidates = @(
    New-Candidate 1 "Windows user temp old files" (Join-Path $UserProfile "AppData\Local\Temp") "Contents" $true
    New-Candidate 1 "Squirrel temp" (Join-Path $UserProfile "AppData\Local\SquirrelTemp") "Directory"
    New-Candidate 1 "npm cache" (Join-Path $UserProfile "AppData\Local\npm-cache") "Directory"
    New-Candidate 1 "pnpm store" (Join-Path $UserProfile "AppData\Local\pnpm-store") "Directory"
    New-Candidate 1 "Yarn cache" (Join-Path $UserProfile "AppData\Local\Yarn\Cache") "Directory"
    New-Candidate 1 "user cache contents" (Join-Path $UserProfile ".cache") "Contents"
    New-Candidate 1 "Cargo registry cache" (Join-Path $UserProfile ".cargo\registry") "Directory"
    New-Candidate 2 "Forge codex targets" (Join-Path $Root ".codex-targets") "Directory"
    New-Candidate 2 "Forge codex temp" (Join-Path $Root ".codex-tmp") "Directory"
    New-Candidate 2 "Forge fuzz target" (Join-Path $Root "target-fuzz-local") "Directory"
    New-Candidate 2 "Native front check target" (Join-Path $Root "examples\ingen_native_front\target-check") "Directory"
    New-Candidate 2 "Native front desktop target" (Join-Path $Root "examples\ingen_native_front\target-desktop") "Directory"
    New-Candidate 3 "NVIDIA shader cache" (Join-Path $UserProfile "AppData\Local\NVIDIA\DXCache") "Contents"
    New-Candidate 3 "Unreal derived data cache" (Join-Path $UserProfile "AppData\Local\UnrealEngine\Common\Zen\Data\cache") "Contents"
    New-Candidate 3 "Claude app cache" (Join-Path $UserProfile "AppData\Roaming\Claude\Cache") "Contents"
    New-Candidate 3 "Claude code cache" (Join-Path $UserProfile "AppData\Roaming\Claude\Code Cache") "Contents"
    New-Candidate 3 "Claude GPU cache" (Join-Path $UserProfile "AppData\Roaming\Claude\GPUCache") "Contents"
    New-Candidate 4 "Claude VM bundles" (Join-Path $UserProfile "AppData\Roaming\Claude\vm_bundles") "Directory"
)

$initialFree = Get-FreeGB
$report = [System.Collections.Generic.List[object]]::new()

if ($initialFree -ge $MinFreeGB) {
    $summary = [pscustomobject]@{
        Time = $Now.ToString("o")
        Apply = [bool]$Apply
        Status = "ok"
        InitialFreeGB = $initialFree
        FinalFreeGB = $initialFree
        MinFreeGB = $MinFreeGB
        TargetFreeGB = $TargetFreeGB
        FreedGB = 0
        Items = @()
    }
    Write-Host "Forge disk guard: C: has $initialFree GB free; threshold is $MinFreeGB GB. No cleanup needed." -ForegroundColor Green
} else {
    Write-Host "Forge disk guard: C: has $initialFree GB free; cleaning toward $TargetFreeGB GB." -ForegroundColor Yellow
    foreach ($candidate in ($candidates | Sort-Object Tier)) {
        $currentFree = Get-FreeGB
        if ($currentFree -ge $TargetFreeGB) { break }
        if ($candidate.Tier -ge 4 -and $currentFree -ge $CriticalFreeGB) { continue }
        $result = Invoke-CleanupCandidate $candidate
        [void]$report.Add($result)
    }

    $finalFree = Get-FreeGB
    $freed = [math]::Round($finalFree - $initialFree, 2)
    $summary = [pscustomobject]@{
        Time = $Now.ToString("o")
        Apply = [bool]$Apply
        Status = if ($finalFree -ge $MinFreeGB) { "recovered" } else { "still-low" }
        InitialFreeGB = $initialFree
        FinalFreeGB = $finalFree
        MinFreeGB = $MinFreeGB
        TargetFreeGB = $TargetFreeGB
        FreedGB = $freed
        Items = @($report)
    }

    $report | Sort-Object Tier, Name | Format-Table -AutoSize -Wrap
    Write-Host "Forge disk guard: freed $freed GB; C: free is now $finalFree GB." -ForegroundColor Green
}

if ($Apply) {
    New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
    $logPath = Join-Path $LogDir "history.jsonl"
    ($summary | ConvertTo-Json -Depth 5 -Compress) | Add-Content -LiteralPath $logPath -Encoding UTF8
}
