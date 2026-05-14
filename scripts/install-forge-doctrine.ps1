param(
    [switch]$Apply,
    [switch]$NoCreate,
    [string]$ClaudeRoot,
    [string]$CodexRoot,
    [string[]]$ProjectRoots,
    [string]$DoctrineText
)

$ErrorActionPreference = "Stop"

if (-not $DoctrineText -or $DoctrineText.Trim().Length -eq 0) {
    $DoctrineText = "Forge doctrine: when a task involves large files, repeated or expensive computation, scientific/numerical/data/code/document-heavy analysis, custom metrics/programs, or verifiable hash/proof results, use the Forge MCP before reading raw CSV/source/log files or calculating inside the LLM. Prefer capabilities -> run plan_only -> create/run -> logs/read; keep raw data on disk and exchange compact hashes, previews, artifacts and proofs."
}

$StartMarker = "<!-- FORGE_MCP_DOCTRINE_START -->"
$EndMarker = "<!-- FORGE_MCP_DOCTRINE_END -->"

function Add-UniqueTarget([System.Collections.Generic.List[object]]$Targets, [string]$Agent, [string]$PathText, [bool]$CreateIfMissing) {
    if (-not $PathText) { return }
    $full = [System.IO.Path]::GetFullPath($PathText)
    if ($Targets | Where-Object { $_.Path -eq $full }) { return }
    [void]$Targets.Add([pscustomobject]@{
        Agent = $Agent
        Path = $full
        CreateIfMissing = $CreateIfMissing
    })
}

function Get-AgentDoctrineTargets {
    $targets = [System.Collections.Generic.List[object]]::new()
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
    $repoUserHome = $null
    if ($repoRoot -match '^([A-Za-z]:\\Users\\[^\\]+)\\') {
        $repoUserHome = $Matches[1]
    } elseif ($repoRoot -match '^(/Users/[^/]+)(/|$)') {
        $repoUserHome = $Matches[1]
    } elseif ($repoRoot -match '^(/home/[^/]+)(/|$)') {
        $repoUserHome = $Matches[1]
    }
    $userHome = if ($env:FORGE_INSTALL_USER_HOME) {
        $env:FORGE_INSTALL_USER_HOME
    } elseif ($repoUserHome) {
        $repoUserHome
    } else {
        [Environment]::GetFolderPath("UserProfile")
    }

    if (-not $ClaudeRoot -or $ClaudeRoot.Trim().Length -eq 0) {
        $ClaudeRoot = Join-Path $userHome ".claude"
    }
    if (-not $CodexRoot -or $CodexRoot.Trim().Length -eq 0) {
        $CodexRoot = Join-Path $userHome ".codex"
    }

    # Claude Code/global doctrine convention.
    Add-UniqueTarget $targets "Claude" (Join-Path $ClaudeRoot "CLAUDE.md") (-not $NoCreate)

    # macOS Claude app fallback. This is optional and only patched when the
    # folder exists; the canonical CLI/source doctrine remains ~/.claude.
    $macClaudeApp = Join-Path $userHome "Library/Application Support/Claude"
    if (Test-Path -LiteralPath $macClaudeApp -PathType Container) {
        Add-UniqueTarget $targets "Claude" (Join-Path $macClaudeApp "CLAUDE.md") (-not $NoCreate)
    }

    # Codex canonical agent-instruction file. Codex-style agents generally read
    # AGENTS.md; we also maintain CLAUDE.md as a compatibility/user-visible note.
    Add-UniqueTarget $targets "Codex" (Join-Path $CodexRoot "AGENTS.md") (-not $NoCreate)
    Add-UniqueTarget $targets "Codex" (Join-Path $CodexRoot "CLAUDE.md") (-not $NoCreate)

    return $targets
}

function Get-ProjectDoctrineTargets([string[]]$Roots) {
    $targets = [System.Collections.Generic.List[object]]::new()
    if (-not $Roots -or $Roots.Count -eq 0) { return $targets }

    $names = @("CLAUDE.md", "AGENTS.md", "instructions.md")
    foreach ($root in $Roots) {
        if (-not (Test-Path -LiteralPath $root -PathType Container)) { continue }
        foreach ($name in $names) {
            Get-ChildItem -LiteralPath $root -Recurse -Force -File -Filter $name -ErrorAction SilentlyContinue |
                Where-Object {
                    $_.FullName -notmatch "\\target\\" -and
                    $_.FullName -notmatch "\\node_modules\\" -and
                    $_.FullName -notmatch "\\\.git\\" -and
                    $_.FullName -notmatch "\\dist\\" -and
                    $_.FullName -notmatch "\\build\\"
                } |
                ForEach-Object {
                    Add-UniqueTarget $targets "Project" $_.FullName $false
                }
        }
    }
    return $targets
}

function Get-PatchedText([string]$CurrentText, [string]$BlockText) {
    $escapedStart = [regex]::Escape($StartMarker)
    $escapedEnd = [regex]::Escape($EndMarker)
    $pattern = "(?s)$escapedStart.*?$escapedEnd"
    if ($CurrentText -match $pattern) {
        return [regex]::Replace($CurrentText, $pattern, $BlockText)
    }
    $trimmed = $CurrentText.TrimEnd()
    if ($trimmed.Length -eq 0) {
        return $BlockText + [Environment]::NewLine
    }
    return $trimmed + [Environment]::NewLine + [Environment]::NewLine + $BlockText + [Environment]::NewLine
}

$block = @"
$StartMarker
$DoctrineText
$EndMarker
"@

$targets = [System.Collections.Generic.List[object]]::new()
Get-AgentDoctrineTargets | ForEach-Object { Add-UniqueTarget $targets $_.Agent $_.Path $_.CreateIfMissing }
Get-ProjectDoctrineTargets $ProjectRoots | ForEach-Object { Add-UniqueTarget $targets $_.Agent $_.Path $_.CreateIfMissing }

$changed = 0
$unchanged = 0
$missing = 0

foreach ($target in $targets) {
    $file = $target.Path
    $exists = Test-Path -LiteralPath $file -PathType Leaf
    if (-not $exists -and -not $target.CreateIfMissing) {
        $missing++
        Write-Host "missing    [$($target.Agent)] $file" -ForegroundColor DarkGray
        continue
    }

    $current = if ($exists) {
        Get-Content -LiteralPath $file -Raw -ErrorAction Stop
    } else {
        ""
    }
    $patched = Get-PatchedText $current $block
    if ($exists -and $patched -eq $current) {
        $unchanged++
        Write-Host "unchanged  [$($target.Agent)] $file" -ForegroundColor DarkGray
        continue
    }

    $changed++
    if ($Apply) {
        $dir = Split-Path -Parent $file
        if (-not (Test-Path -LiteralPath $dir -PathType Container)) {
            New-Item -ItemType Directory -Force -Path $dir | Out-Null
        }
        Set-Content -LiteralPath $file -Value $patched -Encoding UTF8
        if ($exists) {
            Write-Host "patched    [$($target.Agent)] $file" -ForegroundColor Green
        } else {
            Write-Host "created    [$($target.Agent)] $file" -ForegroundColor Green
        }
    } else {
        if ($exists) {
            Write-Host "would patch [$($target.Agent)] $file" -ForegroundColor Cyan
        } else {
            Write-Host "would create [$($target.Agent)] $file" -ForegroundColor Cyan
        }
    }
}

if (-not $Apply) {
    Write-Host ""
    Write-Host "Dry run only. Re-run with -Apply to write the Forge doctrine block." -ForegroundColor Yellow
}

Write-Host "Forge doctrine installer: targets=$($targets.Count) changed=$changed unchanged=$unchanged missing=$missing apply=$Apply"
