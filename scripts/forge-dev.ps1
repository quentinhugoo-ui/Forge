$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Quote-PwshLiteral([string]$Value) {
    return "'" + $Value.Replace("'", "''") + "'"
}

$QuotedRoot = Quote-PwshLiteral $Root

$UiCommand = @"
cd $QuotedRoot
Write-Host "InGen native front: classic build + launch" -ForegroundColor Cyan
`$env:FORGE_CARGO_SESSION = "desktop-classic"
.\scripts\forge-cargo.ps1 build --manifest-path examples\ingen_native_front\Cargo.toml
if (`$LASTEXITCODE -ne 0) {
    Write-Host "Build failed." -ForegroundColor Red
    exit `$LASTEXITCODE
}
Start-Process -FilePath (Join-Path `$PWD ".codex-targets\desktop-classic\debug\ingen-native-front.exe")
"@

Start-Process powershell.exe -ArgumentList @(
    "-NoExit",
    "-ExecutionPolicy", "Bypass",
    "-Command", $UiCommand
)

Write-Host "Forge dev launched." -ForegroundColor Green
