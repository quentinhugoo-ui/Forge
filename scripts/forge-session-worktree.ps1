param(
    [Parameter(Mandatory = $true)]
    [string] $Name,

    [string] $Base = "origin/master",

    [string] $SessionsRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function ConvertTo-SafeName {
    param([string] $Value)

    $safe = ($Value.Trim().ToLowerInvariant() -replace "[^a-z0-9._-]+", "-").Trim("-")
    if ([string]::IsNullOrWhiteSpace($safe)) {
        throw "Session name must contain at least one letter or number."
    }
    return $safe
}

$repoRoot = (& git rev-parse --show-toplevel).Trim()
if ([string]::IsNullOrWhiteSpace($repoRoot)) {
    throw "This script must run inside the Forge git repository."
}

$safeName = ConvertTo-SafeName $Name
$branchName = "codex/$safeName"

if ([string]::IsNullOrWhiteSpace($SessionsRoot)) {
    $parent = Split-Path -Parent $repoRoot
    $SessionsRoot = Join-Path $parent "Forge-sessions"
}

$sessionsRootPath = [System.IO.Path]::GetFullPath($SessionsRoot)
$targetPath = [System.IO.Path]::GetFullPath((Join-Path $sessionsRootPath $safeName))

if (-not ($targetPath.StartsWith($sessionsRootPath, [System.StringComparison]::OrdinalIgnoreCase))) {
    throw "Resolved worktree path escaped the sessions root."
}

if (Test-Path -LiteralPath $targetPath) {
    throw "Worktree already exists: $targetPath"
}

New-Item -ItemType Directory -Path $sessionsRootPath -Force | Out-Null

& git -C $repoRoot fetch origin
if ($LASTEXITCODE -ne 0) {
    throw "git fetch origin failed."
}

& git -C $repoRoot worktree add -b $branchName $targetPath $Base
if ($LASTEXITCODE -ne 0) {
    throw "git worktree add failed."
}

[pscustomobject]@{
    schema = "forge.agent_session_worktree.v1"
    branch = $branchName
    path = $targetPath
    base = $Base
} | ConvertTo-Json -Depth 3
