param(
    [string]$BinaryPath,
    [string]$OutputRoot = "C:\tmp\unreal-ghidra",
    [int]$SampleLimit = 2000
)

$ErrorActionPreference = "Stop"

$ghidraRoot = Join-Path $env:LOCALAPPDATA "Programs\Ghidra\ghidra_12.1.2_PUBLIC"
$headless = Join-Path $ghidraRoot "support\analyzeHeadless.bat"
$scriptPath = Join-Path (Resolve-Path ".").Path "scripts\ghidra"
$scriptName = "UnrealBinaryMap.java"

if (-not (Test-Path -LiteralPath $headless)) {
    throw "Ghidra headless launcher not found: $headless"
}

if (-not $BinaryPath) {
    $candidates = @(
        "C:\Program Files\Epic Games",
        "C:\Program Files (x86)\Epic Games"
    ) | Where-Object { Test-Path -LiteralPath $_ }

    $BinaryPath = $candidates |
        ForEach-Object {
            Get-ChildItem -LiteralPath $_ -Recurse -Filter "UnrealEditor.exe" -ErrorAction SilentlyContinue
        } |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}

if (-not $BinaryPath -or -not (Test-Path -LiteralPath $BinaryPath)) {
    throw "UnrealEditor.exe was not found. Install Unreal Engine from Epic Games Launcher, then rerun this script or pass -BinaryPath."
}

New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null

$binaryItem = Get-Item -LiteralPath $BinaryPath
$binaryHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $BinaryPath).Hash.ToLowerInvariant()
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$projectRoot = Join-Path $OutputRoot "ghidra-project"
$reportDir = Join-Path $OutputRoot "reports"
$projectName = "unreal-editor-$stamp"
$jsonPath = Join-Path $reportDir "unreal-editor-binary-map-$stamp.json"
$manifestPath = Join-Path $reportDir "unreal-editor-analysis-manifest-$stamp.json"

New-Item -ItemType Directory -Force -Path $projectRoot, $reportDir | Out-Null

$manifest = [ordered]@{
    binaryPath = $binaryItem.FullName
    binaryLength = $binaryItem.Length
    binarySha256 = $binaryHash
    ghidraRoot = $ghidraRoot
    projectRoot = $projectRoot
    projectName = $projectName
    binaryMapJson = $jsonPath
    cleanRoomRule = "Metadata only: no decompiled bodies, no copied Unreal Engine code."
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $manifestPath -Encoding UTF8

& $headless $projectRoot $projectName `
    -import $BinaryPath `
    -overwrite `
    -scriptPath $scriptPath `
    -postScript $scriptName $jsonPath $SampleLimit `
    -deleteProject

if ($LASTEXITCODE -ne 0) {
    throw "Ghidra analyzeHeadless failed with exit code $LASTEXITCODE"
}

Write-Host "Manifest: $manifestPath"
Write-Host "Binary map: $jsonPath"
