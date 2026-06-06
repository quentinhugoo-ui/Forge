$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Quote-PwshLiteral([string]$Value) {
    return "'" + $Value.Replace("'", "''") + "'"
}

$QuotedRoot = Quote-PwshLiteral $Root

$UiCommand = @"
cd $QuotedRoot
Write-Host "InGen native front: cargo run" -ForegroundColor Cyan
cargo run --manifest-path examples\ingen_native_front\Cargo.toml
"@

Start-Process powershell.exe -ArgumentList @(
    "-NoExit",
    "-ExecutionPolicy", "Bypass",
    "-Command", $UiCommand
)

Write-Host "Forge dev launched." -ForegroundColor Green
