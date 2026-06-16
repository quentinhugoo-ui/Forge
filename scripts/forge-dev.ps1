$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Quote-PwshLiteral([string]$Value) {
    return "'" + $Value.Replace("'", "''") + "'"
}

$QuotedRoot = Quote-PwshLiteral $Root

$UiCommand = @"
cd $QuotedRoot
Write-Host "InGen Electron shell: build freshness + launch" -ForegroundColor Cyan
cmd.exe /c examples\ingen_electron_shell\run_ingen_electron_shell.cmd
"@

Start-Process powershell.exe -ArgumentList @(
    "-NoExit",
    "-ExecutionPolicy", "Bypass",
    "-Command", $UiCommand
)

Write-Host "Forge dev launched." -ForegroundColor Green
