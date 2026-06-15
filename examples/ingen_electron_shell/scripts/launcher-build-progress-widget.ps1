param(
  [Parameter(Mandatory = $true)]
  [string]$LogPath,

  [Parameter(Mandatory = $true)]
  [string]$ElectronPath,

  [Parameter(Mandatory = $true)]
  [string]$ShellRoot,

  [Parameter(Mandatory = $true)]
  [string]$BuildLockPath
)

$ErrorActionPreference = "SilentlyContinue"

Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase

function Normalize-PathText {
  param([string]$PathText)
  try {
    return [System.IO.Path]::GetFullPath($PathText).TrimEnd("\").ToLowerInvariant()
  } catch {
    if ($null -eq $PathText) {
      return ""
    }
    return $PathText.TrimEnd("\").ToLowerInvariant()
  }
}

$normalizedElectronPath = Normalize-PathText $ElectronPath
$normalizedShellRoot = Normalize-PathText $ShellRoot
$failedAt = $null

function Read-LaunchLog {
  if (-not (Test-Path -LiteralPath $LogPath)) {
    return ""
  }

  try {
    $stream = [System.IO.File]::Open($LogPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
    try {
      $reader = [System.IO.StreamReader]::new($stream)
      try {
        return $reader.ReadToEnd()
      } finally {
        $reader.Dispose()
      }
    } finally {
      $stream.Dispose()
    }
  } catch {
    return ""
  }
}

function Test-InGenElectronRunning {
  try {
    $processes = Get-CimInstance Win32_Process -Filter "Name = 'electron.exe'"
    foreach ($process in $processes) {
      $path = Normalize-PathText $process.ExecutablePath
      $commandLine = ""
      if ($null -ne $process.CommandLine) {
        $commandLine = $process.CommandLine.ToLowerInvariant()
      }
      if ($path -eq $normalizedElectronPath -and $commandLine.Contains($normalizedShellRoot)) {
        return $true
      }
    }
  } catch {
  }

  return $false
}

function Get-WidgetState {
  param([string]$Text)

  $compileCount = ([regex]::Matches($Text, "(?m)^\s+Compiling\s+")).Count
  $downloadCount = ([regex]::Matches($Text, "(?m)^\s+Downloaded\s+")).Count

  if ($Text -match "Launch failed") {
    return @{ Label = "Launch failed"; Detail = "Opening launcher log"; Percent = 100; Failed = $true }
  }

  if ($Text -match "Starting Electron") {
    return @{ Label = "Launching InGen"; Detail = "Opening desktop shell"; Percent = 98; Failed = $false }
  }

  if ($Text -match "Building Electron renderer") {
    return @{ Label = "Building renderer"; Detail = "Vite production bundle"; Percent = [Math]::Min(96, 88 + [Math]::Floor($compileCount / 5)); Failed = $false }
  }

  if ($Text -match "Generating Electron IPC contract") {
    return @{ Label = "Generating IPC"; Detail = "Refreshing typed bridge"; Percent = 84; Failed = $false }
  }

  if ($Text -match "Building Electron main process") {
    return @{ Label = "Building Electron"; Detail = "Main process TypeScript"; Percent = 78; Failed = $false }
  }

  if ($Text -match "Building Windows taskbar helper") {
    return @{ Label = "Building helper"; Detail = "Windows taskbar bridge"; Percent = [Math]::Min(76, 62 + [Math]::Floor($compileCount / 6)); Failed = $false }
  }

  if ($Text -match "Building Rust backend bridge") {
    $progress = 20 + [Math]::Min(40, [Math]::Floor(($compileCount + $downloadCount) / 3))
    return @{ Label = "Building backend"; Detail = "Rust crates and native bridge"; Percent = $progress; Failed = $false }
  }

  if ($Text -match "Another InGen launcher") {
    $pulse = [DateTimeOffset]::Now.ToUnixTimeSeconds() % 10
    return @{ Label = "Waiting for build"; Detail = "Another launcher owns the build lock"; Percent = 10 + $pulse; Failed = $false }
  }

  if ($Text -match "Using existing Electron build" -or $Text -match "Desktop fast path") {
    return @{ Label = "Starting InGen"; Detail = "Using cached desktop build"; Percent = 92; Failed = $false }
  }

  if (Test-Path -LiteralPath $BuildLockPath) {
    return @{ Label = "Preparing build"; Detail = "Launcher lock acquired"; Percent = 8; Failed = $false }
  }

  return @{ Label = "Preparing InGen"; Detail = "Checking launcher state"; Percent = 4; Failed = $false }
}

[xml]$xaml = @"
<Window
  xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
  xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
  Title="InGen build progress"
  Width="336"
  Height="92"
  WindowStyle="None"
  AllowsTransparency="True"
  Background="Transparent"
  ResizeMode="NoResize"
  ShowInTaskbar="False"
  ShowActivated="False"
  Topmost="True">
  <Grid Background="Transparent">
    <StackPanel HorizontalAlignment="Right" VerticalAlignment="Top" Margin="0">
      <TextBlock
        x:Name="TitleText"
        Text="Preparing InGen"
        Foreground="#F8FAFC"
        FontFamily="Segoe UI Variable, Segoe UI"
        FontSize="15"
        FontWeight="SemiBold"
        TextAlignment="Right">
        <TextBlock.Effect>
          <DropShadowEffect BlurRadius="9" ShadowDepth="1" Opacity="0.55" Color="#000000"/>
        </TextBlock.Effect>
      </TextBlock>
      <TextBlock
        x:Name="DetailText"
        Text="Checking launcher state"
        Margin="0,3,0,10"
        Foreground="#D7DEE8"
        FontFamily="Segoe UI Variable, Segoe UI"
        FontSize="12"
        TextAlignment="Right">
        <TextBlock.Effect>
          <DropShadowEffect BlurRadius="8" ShadowDepth="1" Opacity="0.55" Color="#000000"/>
        </TextBlock.Effect>
      </TextBlock>
      <Grid Width="264" Height="7" HorizontalAlignment="Right">
        <Border Background="#40FFFFFF" CornerRadius="3.5"/>
        <Border x:Name="ProgressFill" Width="10" HorizontalAlignment="Left" Background="#FF42E8C6" CornerRadius="3.5"/>
      </Grid>
      <TextBlock
        x:Name="PercentText"
        Text="4%"
        Margin="0,5,0,0"
        Foreground="#B8FFF1"
        FontFamily="Segoe UI Variable, Segoe UI"
        FontSize="11"
        TextAlignment="Right">
        <TextBlock.Effect>
          <DropShadowEffect BlurRadius="8" ShadowDepth="1" Opacity="0.55" Color="#000000"/>
        </TextBlock.Effect>
      </TextBlock>
    </StackPanel>
  </Grid>
</Window>
"@

$reader = [System.Xml.XmlNodeReader]::new($xaml)
$window = [Windows.Markup.XamlReader]::Load($reader)
$titleText = $window.FindName("TitleText")
$detailText = $window.FindName("DetailText")
$percentText = $window.FindName("PercentText")
$progressFill = $window.FindName("ProgressFill")

$window.Add_Loaded({
  $workArea = [System.Windows.SystemParameters]::WorkArea
  $window.Left = $workArea.Right - $window.Width - 24
  $window.Top = $workArea.Top + 24
})

$timer = [System.Windows.Threading.DispatcherTimer]::new()
$timer.Interval = [TimeSpan]::FromMilliseconds(650)
$timer.Add_Tick({
  if (Test-InGenElectronRunning) {
    $timer.Stop()
    $window.Close()
    return
  }

  $text = Read-LaunchLog
  $state = Get-WidgetState $text
  $percent = [Math]::Max(0, [Math]::Min(100, [int]$state.Percent))

  $titleText.Text = [string]$state.Label
  $detailText.Text = [string]$state.Detail
  $percentText.Text = "$percent%"
  $progressFill.Width = [Math]::Max(10, 264 * ($percent / 100))

  if ($state.Failed) {
    $progressFill.Background = [System.Windows.Media.SolidColorBrush]::new([System.Windows.Media.Color]::FromRgb(255, 107, 107))
    if ($script:failedAt -eq $null) {
      $script:failedAt = Get-Date
    }
    if (((Get-Date) - $script:failedAt).TotalSeconds -gt 10) {
      $timer.Stop()
      $window.Close()
    }
  }
})

$timer.Start()
[void]$window.ShowDialog()
