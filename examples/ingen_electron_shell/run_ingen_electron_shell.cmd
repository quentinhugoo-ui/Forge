@echo off
setlocal
cd /d "%~dp0"
if not exist C:\tmp mkdir C:\tmp
set LOG=C:\tmp\ingen-electron-launch-%RANDOM%.log
echo ==== InGen Electron launch %DATE% %TIME% ==== > "%LOG%"
set FORGE_FRONT_SLICE_HEADER=electron
set FORGE_FRONT_SLICE_SIDEBAR=electron
set FORGE_FRONT_SLICE_CANVAS=electron
set FORGE_FRONT_SLICE_RIGHT_PANEL=electron
set FORGE_FRONT_SLICE_PANELS_CHAT_BOTTOM=electron
set FORGE_CARGO_SESSION=ingen-electron-shortcut
set REPO_ROOT=%~dp0..\..
for %%I in ("%REPO_ROOT%") do set REPO_ROOT=%%~fI
set FORGE_ELECTRON_BACKEND_EXE=%REPO_ROOT%\.codex-targets\ingen-electron-shortcut\debug\ingen_electron_backend_bridge.exe
set FORGE_ELECTRON_EXE=%~dp0node_modules\electron\dist\electron.exe
set INGEN_ELECTRON_LEGACY_USER_DATA_DIR=%APPDATA%\InGen
set INGEN_ELECTRON_USER_DATA_DIR=%APPDATA%\InGenRuntime
set INGEN_ELECTRON_BYPASS_SINGLE_INSTANCE_LOCK=1
set BUILD_LOCK=C:\tmp\ingen-electron-launch-build.lock
set NEED_BACKEND_REBUILD=0
set NEED_ELECTRON_REBUILD=0
set APP_ALREADY_RUNNING=0
set OWN_BUILD_LOCK=0

if not exist "%INGEN_ELECTRON_USER_DATA_DIR%" mkdir "%INGEN_ELECTRON_USER_DATA_DIR%"
if not exist "%INGEN_ELECTRON_USER_DATA_DIR%\brain" if exist "%INGEN_ELECTRON_LEGACY_USER_DATA_DIR%\brain" (
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "Copy-Item -LiteralPath '%INGEN_ELECTRON_LEGACY_USER_DATA_DIR%\brain' -Destination '%INGEN_ELECTRON_USER_DATA_DIR%\brain' -Recurse -Force" >> "%LOG%" 2>>&1
)
for %%F in (workspace.json llm-provider-runtime.json llm-providers.json llm-runtime-request.json native-session-ledger.json) do (
  if not exist "%INGEN_ELECTRON_USER_DATA_DIR%\%%F" if exist "%INGEN_ELECTRON_LEGACY_USER_DATA_DIR%\%%F" copy /Y "%INGEN_ELECTRON_LEGACY_USER_DATA_DIR%\%%F" "%INGEN_ELECTRON_USER_DATA_DIR%\%%F" >> "%LOG%" 2>>&1
)
echo Electron userData: %INGEN_ELECTRON_USER_DATA_DIR% >> "%LOG%"

C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$root = (Resolve-Path -LiteralPath '%~dp0').Path.TrimEnd('\'); $electron = '%FORGE_ELECTRON_EXE%'; $running = Get-CimInstance Win32_Process -Filter \"Name = 'electron.exe'\" -ErrorAction SilentlyContinue | Where-Object { $_.ExecutablePath -eq $electron -and $_.CommandLine -like ('*' + $root + '*') } | Select-Object -First 1; if ($running) { exit 0 } exit 1"
if not errorlevel 1 set APP_ALREADY_RUNNING=1

if "%APP_ALREADY_RUNNING%"=="1" (
  echo InGen is already running. Sending focus request through Electron single-instance lock. >> "%LOG%"
  goto start_electron
)

2>nul mkdir "%BUILD_LOCK%"
if errorlevel 1 (
  echo Another InGen launcher is already preparing the build. Waiting briefly, then focusing the app. >> "%LOG%"
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$lock = '%BUILD_LOCK%'; $deadline = (Get-Date).AddSeconds(180); while ((Test-Path -LiteralPath $lock) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }; if (Test-Path -LiteralPath $lock) { exit 1 }"
  goto start_electron
)
set OWN_BUILD_LOCK=1

if not exist "%FORGE_ELECTRON_BACKEND_EXE%" set NEED_BACKEND_REBUILD=1
C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$exe = '%FORGE_ELECTRON_BACKEND_EXE%'; $srcRoot = '%REPO_ROOT%\examples\ingen_native_services'; $bin = Get-Item -LiteralPath $exe -ErrorAction SilentlyContinue; $latest = Get-ChildItem -LiteralPath (Join-Path $srcRoot 'src') -Recurse -File -Include *.rs -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1; if (-not $bin -or ($latest -and $latest.LastWriteTimeUtc -gt $bin.LastWriteTimeUtc)) { exit 1 }"
if errorlevel 1 set NEED_BACKEND_REBUILD=1

if "%NEED_BACKEND_REBUILD%"=="1" (
  echo Building Rust backend bridge... >> "%LOG%"
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%REPO_ROOT%\scripts\forge-cargo.ps1" build --manifest-path "%REPO_ROOT%\examples\ingen_native_services\Cargo.toml" --bin ingen_electron_backend_bridge >> "%LOG%" 2>>&1
  if errorlevel 1 goto fail
)

if not exist "%~dp0dist-electron\main\main.js" set NEED_ELECTRON_REBUILD=1
if not exist "%~dp0dist\renderer\index.html" set NEED_ELECTRON_REBUILD=1
if "%FORGE_ELECTRON_FORCE_REBUILD%"=="1" set NEED_ELECTRON_REBUILD=1
C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$root = '%~dp0'; $dist = Get-Item -LiteralPath (Join-Path $root 'dist-electron\main\main.js') -ErrorAction SilentlyContinue; $renderer = Get-Item -LiteralPath (Join-Path $root 'dist\renderer\index.html') -ErrorAction SilentlyContinue; $latest = Get-ChildItem -LiteralPath (Join-Path $root 'src') -Recurse -File -Include *.ts,*.tsx,*.css -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1; if (-not $dist -or -not $renderer -or ($latest -and ($latest.LastWriteTimeUtc -gt $dist.LastWriteTimeUtc -or $latest.LastWriteTimeUtc -gt $renderer.LastWriteTimeUtc))) { exit 1 }"
if errorlevel 1 set NEED_ELECTRON_REBUILD=1

if "%NEED_ELECTRON_REBUILD%"=="1" (
  echo Building Electron main process... >> "%LOG%"
  call npx.cmd tsc -p tsconfig.electron.json >> "%LOG%" 2>>&1
  if errorlevel 1 goto fail

  if not exist "%~dp0src\shared\generated\forge-ipc.generated.ts" (
    echo Generating Electron IPC contract... >> "%LOG%"
    call npm.cmd run generate:ipc >> "%LOG%" 2>>&1
    if errorlevel 1 goto fail
  )

  echo Building Electron renderer... >> "%LOG%"
  call npx.cmd vite build >> "%LOG%" 2>>&1
  if errorlevel 1 goto fail
) else (
  echo Using existing Electron build. Set FORGE_ELECTRON_FORCE_REBUILD=1 to rebuild. >> "%LOG%"
)

:start_electron
echo Starting Electron... >> "%LOG%"
if not exist "%FORGE_ELECTRON_EXE%" (
  echo Electron executable is missing. Repairing Electron runtime... >> "%LOG%"
  if not exist "%~dp0node_modules\electron\install.js" (
    echo Electron package is missing. Running npm install... >> "%LOG%"
    call npm.cmd install >> "%LOG%" 2>>&1
    if errorlevel 1 goto fail
  )
  call node "%~dp0node_modules\electron\install.js" >> "%LOG%" 2>>&1
  if errorlevel 1 goto fail
)
if not exist "%FORGE_ELECTRON_EXE%" (
  echo Electron executable is still missing after repair. Run npm install in %~dp0. >> "%LOG%"
  goto fail
)
C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -Command "Start-Process -FilePath '%FORGE_ELECTRON_EXE%' -ArgumentList @('.', '--user-data-dir=%INGEN_ELECTRON_USER_DATA_DIR%') -WorkingDirectory '%~dp0' -WindowStyle Normal" >> "%LOG%" 2>>&1
if errorlevel 1 goto fail
if "%OWN_BUILD_LOCK%"=="1" if exist "%BUILD_LOCK%" rmdir "%BUILD_LOCK%" 2>nul
exit /b 0

:fail
echo Launch failed with code %ERRORLEVEL%. See %LOG%.
if "%OWN_BUILD_LOCK%"=="1" if exist "%BUILD_LOCK%" rmdir "%BUILD_LOCK%" 2>nul
start "InGen Electron launch error" notepad.exe "%LOG%"
exit /b %ERRORLEVEL%
