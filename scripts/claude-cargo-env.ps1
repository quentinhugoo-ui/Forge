$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$rawInput = [Console]::In.ReadToEnd()
$sessionId = ""

if (-not [string]::IsNullOrWhiteSpace($rawInput)) {
    try {
        $payload = $rawInput | ConvertFrom-Json
        if ($payload.session_id) {
            $sessionId = [string] $payload.session_id
        }
    } catch {
        $sessionId = ""
    }
}

function ConvertTo-SafeName([string] $Value) {
    $safe = ($Value.ToLowerInvariant() -replace '[^a-z0-9._-]+', '-').Trim('-')
    if ([string]::IsNullOrWhiteSpace($safe)) {
        return "claude-local"
    }
    return $safe
}

$targetName = if ([string]::IsNullOrWhiteSpace($sessionId)) {
    "claude-local"
} else {
    ConvertTo-SafeName "claude-$sessionId"
}

$targetDir = Join-Path $Root (Join-Path ".codex-targets" $targetName)
New-Item -ItemType Directory -Path $targetDir -Force | Out-Null

if (-not [string]::IsNullOrWhiteSpace($env:CLAUDE_ENV_FILE)) {
    $targetForBash = $targetDir.Replace("\", "/")
    Add-Content -LiteralPath $env:CLAUDE_ENV_FILE -Value "export FORGE_CARGO_SESSION='$targetName'"
    Add-Content -LiteralPath $env:CLAUDE_ENV_FILE -Value "export CARGO_TARGET_DIR='$targetForBash'"
    Add-Content -LiteralPath $env:CLAUDE_ENV_FILE -Value "export FORGE_CARGO_TARGET_DIR='$targetForBash'"
}

exit 0
