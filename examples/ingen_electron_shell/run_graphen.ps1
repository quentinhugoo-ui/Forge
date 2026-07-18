param(
  [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$ShellRoot = $PSScriptRoot
$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $ShellRoot "..\..")).Path
$Electron = Join-Path $ShellRoot "node_modules\electron\dist\electron.exe"
$Backend = Join-Path $RepoRoot ".codex-targets\ingen-electron-shortcut\debug\ingen_electron_backend_bridge.exe"
$TaskbarHelper = Join-Path $RepoRoot ".codex-targets\ingen-electron-shortcut\debug\ingen_windows_taskbar_helper.exe"
$UserData = Join-Path $env:APPDATA "InGenRuntime"
$OandaTokenFile = Join-Path $RepoRoot "TOKEN OANDA.txt"
$DefaultGraphenIcon = "C:\Users\Quentin\Documents\graphen-studio\assets\graphen.ico"
$AppIcon = if ($env:GRAPHEN_APP_ICON) { $env:GRAPHEN_APP_ICON } else { $DefaultGraphenIcon }
$LogPath = "C:\tmp\graphen-launch.log"

function Write-GraphenLog([string]$Message) {
  $logDir = Split-Path -Parent $LogPath
  if (-not (Test-Path -LiteralPath $logDir)) {
    New-Item -ItemType Directory -Path $logDir -Force | Out-Null
  }
  Add-Content -LiteralPath $LogPath -Value ("[{0}] {1}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss"), $Message)
}

try {
  Write-GraphenLog "Launching Graphen from $ShellRoot"

  foreach ($required in @($ShellRoot, $Electron, $Backend, $TaskbarHelper, $AppIcon, $OandaTokenFile)) {
    if (-not (Test-Path -LiteralPath $required)) {
      throw "Missing required path: $required"
    }
  }

  if (-not (Test-Path -LiteralPath $UserData)) {
    New-Item -ItemType Directory -Path $UserData -Force | Out-Null
  }

  $env:FORGE_FRONT_SLICE_HEADER = "electron"
  $env:FORGE_FRONT_SLICE_SIDEBAR = "electron"
  $env:FORGE_FRONT_SLICE_CANVAS = "electron"
  $env:FORGE_FRONT_SLICE_RIGHT_PANEL = "electron"
  $env:FORGE_FRONT_SLICE_PANELS_CHAT_BOTTOM = "electron"
  $env:FORGE_CARGO_SESSION = "ingen-electron-shortcut"
  $env:FORGE_ELECTRON_BACKEND_EXE = $Backend
  $env:FORGE_WINDOWS_TASKBAR_HELPER_EXE = $TaskbarHelper
  $env:INGEN_ELECTRON_USER_DATA_DIR = $UserData
  $env:GRAPHEN_APP_ICON = $AppIcon
  $env:OANDA_TOKEN_FILE = $OandaTokenFile

  if ($DryRun) {
    [pscustomobject]@{
      ShellRoot = $ShellRoot
      Electron = $Electron
      Backend = $Backend
      TaskbarHelper = $TaskbarHelper
      UserData = $UserData
      AppIcon = $AppIcon
      OandaTokenFile = $OandaTokenFile
      OandaTokenFileExists = Test-Path -LiteralPath $OandaTokenFile
      LogPath = $LogPath
    }
    exit 0
  }

  Start-Process -FilePath $Electron -ArgumentList @(".", "--user-data-dir=$UserData") -WorkingDirectory $ShellRoot
  Write-GraphenLog "Graphen start request sent."
} catch {
  Write-GraphenLog ("Launch failed: " + $_.Exception.Message)
  throw
}
