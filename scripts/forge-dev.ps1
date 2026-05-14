param(
    [switch]$NoMcpTerminal
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Quote-PwshLiteral([string]$Value) {
    return "'" + $Value.Replace("'", "''") + "'"
}

$QuotedRoot = Quote-PwshLiteral $Root

$UiCommand = @"
cd $QuotedRoot
Write-Host "Forge UI: cargo tauri dev" -ForegroundColor Cyan
cargo tauri dev
"@

$McpCommand = @"
cd $QuotedRoot
Write-Host "Forge MCP stdio server" -ForegroundColor Cyan
Write-Host "This terminal will stay quiet until a MCP client sends JSON-RPC over stdin." -ForegroundColor Yellow
Write-Host "Claude/Codex should be configured to launch: C:\scan-shared-target\debug\forge_mcp.exe" -ForegroundColor Yellow
cargo mcp
"@

Start-Process powershell.exe -ArgumentList @(
    "-NoExit",
    "-ExecutionPolicy", "Bypass",
    "-Command", $UiCommand
)

if (-not $NoMcpTerminal) {
    Start-Process powershell.exe -ArgumentList @(
        "-NoExit",
        "-ExecutionPolicy", "Bypass",
        "-Command", $McpCommand
    )
}

Write-Host "Forge dev launched." -ForegroundColor Green
