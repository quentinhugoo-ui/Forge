param(
  [Parameter(Mandatory = $true)]
  [string]$ShellRoot,

  [Parameter(Mandatory = $true)]
  [string]$LogPath,

  [Parameter(Mandatory = $true)]
  [string]$WorkspaceBuildId
)

$ErrorActionPreference = "Continue"

function Add-LaunchLog {
  param([string]$Message)
  try {
    Add-Content -LiteralPath $LogPath -Value $Message -Encoding UTF8 -ErrorAction Stop
  } catch {
  }
}

Add-LaunchLog "Vite dev server helper invoked for $ShellRoot."

function Test-DevServer {
  param([int]$Port)
  try {
    $response = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$Port/" -TimeoutSec 2
    return [int]$response.StatusCode -ge 200 -and [int]$response.StatusCode -lt 500
  } catch {
    return $false
  }
}

function Test-PortOpen {
  param([int]$Port)
  $client = New-Object System.Net.Sockets.TcpClient
  try {
    $iar = $client.BeginConnect("127.0.0.1", $Port, $null, $null)
    if (-not $iar.AsyncWaitHandle.WaitOne(200, $false)) {
      return $false
    }
    $client.EndConnect($iar)
    return $true
  } catch {
    return $false
  } finally {
    $client.Close()
  }
}

function Get-WorkspacePort {
  $hex = "default"
  if ($null -ne $WorkspaceBuildId -and $WorkspaceBuildId.Length -gt 0) {
    $hex = $WorkspaceBuildId.ToLowerInvariant()
  }
  if ($hex.Length -lt 4 -or $hex -notmatch "^[0-9a-f]+$") {
    return 5173
  }

  $seed = [Convert]::ToInt32($hex.Substring(0, 4), 16)
  return 5173 + ($seed % 300)
}

try {
  $root = (Resolve-Path -LiteralPath $ShellRoot -ErrorAction Stop).Path
} catch {
  Add-LaunchLog "Vite dev server failed: shell root not found: $ShellRoot."
  exit 2
}

$viteBin = Join-Path $root "node_modules\vite\bin\vite.js"
if (-not (Test-Path -LiteralPath $viteBin)) {
  Add-LaunchLog "Vite dev server unavailable: $viteBin is missing."
  exit 2
}

$preferredPort = Get-WorkspacePort
$selectedPort = $null
for ($offset = 0; $offset -lt 80; $offset += 1) {
  $port = $preferredPort + $offset
  if (Test-DevServer $port) {
    Write-Output "http://127.0.0.1:$port"
    exit 0
  }
  if (-not (Test-PortOpen $port)) {
    $selectedPort = $port
    break
  }
}

if ($null -eq $selectedPort) {
  Add-LaunchLog "Vite dev server failed: no free local port near $preferredPort."
  exit 3
}

Add-LaunchLog "Starting Vite renderer dev server on http://127.0.0.1:$selectedPort."
$nodeExe = "node.exe"
try {
  $resolvedNode = & where.exe node 2>$null | Select-Object -First 1
  if ($resolvedNode) {
    $nodeExe = $resolvedNode
  }
} catch {
}
$startInfo = New-Object System.Diagnostics.ProcessStartInfo
$startInfo.FileName = $nodeExe
$startInfo.Arguments = '"' + $viteBin + '" --host 127.0.0.1 --port ' + $selectedPort + ' --strictPort'
$startInfo.WorkingDirectory = $root
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $true
try {
  $process = [System.Diagnostics.Process]::Start($startInfo)
} catch {
  Add-LaunchLog "Vite dev server process failed to start: $($_.Exception.Message)"
  Write-Error $_.Exception.Message
  exit 4
}
if ($null -eq $process) {
  Add-LaunchLog "Vite dev server process failed to start."
  exit 4
}
Add-LaunchLog "Vite dev server process started: pid=$($process.Id)."

$deadline = (Get-Date).AddSeconds(45)
while ((Get-Date) -lt $deadline) {
  if (Test-DevServer $selectedPort) {
    Write-Output "http://127.0.0.1:$selectedPort"
    exit 0
  }
  Start-Sleep -Milliseconds 350
}

Add-LaunchLog "Vite dev server did not become ready on port $selectedPort."
exit 4
