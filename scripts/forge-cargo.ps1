param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $CargoArgs
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function ConvertTo-SafeName([string] $Value) {
    $safe = ($Value.ToLowerInvariant() -replace '[^a-z0-9._-]+', '-').Trim('-')
    if ([string]::IsNullOrWhiteSpace($safe)) {
        return "local"
    }
    return $safe
}

$session = $env:FORGE_CARGO_SESSION
if ([string]::IsNullOrWhiteSpace($session)) {
    if (-not [string]::IsNullOrWhiteSpace($env:CODEX_THREAD_ID)) {
        $session = "codex-$($env:CODEX_THREAD_ID)"
    } elseif (-not [string]::IsNullOrWhiteSpace($env:CLAUDE_SESSION_ID)) {
        $session = "claude-$($env:CLAUDE_SESSION_ID)"
    } elseif (-not [string]::IsNullOrWhiteSpace($env:CLAUDE_PROJECT_DIR)) {
        $session = "claude-local"
    } else {
        $session = "local"
    }
}

$targetName = ConvertTo-SafeName $session
$targetDir = Join-Path $Root (Join-Path ".codex-targets" $targetName)
New-Item -ItemType Directory -Path $targetDir -Force | Out-Null

$env:FORGE_CARGO_SESSION = $targetName
$env:CARGO_TARGET_DIR = $targetDir
$env:FORGE_CARGO_TARGET_DIR = $targetDir

Write-Host "Forge Cargo target: $targetDir" -ForegroundColor DarkCyan

if ([string]::IsNullOrWhiteSpace($env:RUSTC_WRAPPER)) {
    $sccache = $null
    $command = Get-Command sccache -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        $sccache = $command.Source
    } else {
        $workspaceSccache = @(Get-ChildItem -LiteralPath (Join-Path $Root ".codex-tools\sccache") -Filter "sccache.exe" -Recurse -File -ErrorAction SilentlyContinue | Select-Object -First 1)
        if ($workspaceSccache.Count -gt 0) {
            $sccache = $workspaceSccache[0].FullName
        } else {
            $cargoBinSccache = Join-Path $env:USERPROFILE ".cargo\bin\sccache.exe"
            if (Test-Path -LiteralPath $cargoBinSccache) {
                $sccache = $cargoBinSccache
            }
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($sccache)) {
        $env:RUSTC_WRAPPER = $sccache
        Write-Host "Forge Cargo rustc wrapper: sccache" -ForegroundColor DarkCyan
    }
}

if ($CargoArgs.Count -eq 0) {
    & cargo --version
    exit $LASTEXITCODE
}

& cargo @CargoArgs
exit $LASTEXITCODE
