@echo off
setlocal EnableExtensions EnableDelayedExpansion
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
set FORGE_WINDOWS_TASKBAR_HELPER_EXE=%REPO_ROOT%\.codex-targets\ingen-electron-shortcut\debug\ingen_windows_taskbar_helper.exe
set FORGE_ELECTRON_EXE=%~dp0node_modules\electron\dist\electron.exe
set INGEN_ELECTRON_LEGACY_USER_DATA_DIR=%APPDATA%\InGen
set INGEN_ELECTRON_USER_DATA_DIR=%APPDATA%\InGenRuntime
set FORGE_ELECTRON_BUILD_WIDGET_SCRIPT=%~dp0scripts\launcher-build-progress-widget.ps1
set FORGE_ELECTRON_VITE_SERVER_SCRIPT=%~dp0scripts\ensure-vite-dev-server.ps1
for /f %%H in ('C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$root = (Resolve-Path -LiteralPath '%~dp0').Path.TrimEnd('\').ToLowerInvariant(); $bytes = [Text.Encoding]::UTF8.GetBytes($root); $sha = [Security.Cryptography.SHA256]::Create(); $hash = [BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-', '').Substring(0, 12).ToLowerInvariant(); $sha.Dispose(); Write-Output $hash"') do set WORKSPACE_BUILD_ID=%%H
if "%WORKSPACE_BUILD_ID%"=="" set WORKSPACE_BUILD_ID=default
set BUILD_LOCK=C:\tmp\ingen-electron-launch-build-%WORKSPACE_BUILD_ID%.lock
set VITE_DEV_SERVER_URL_FILE=C:\tmp\ingen-vite-%WORKSPACE_BUILD_ID%.url
if not "%FORGE_ELECTRON_BUILD_WIDGET%"=="0" if exist "%FORGE_ELECTRON_BUILD_WIDGET_SCRIPT%" (
  start "" "C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -Sta -File "%FORGE_ELECTRON_BUILD_WIDGET_SCRIPT%" -LogPath "%LOG%" -ElectronPath "%FORGE_ELECTRON_EXE%" -ShellRoot "%~dp0" -BuildLockPath "%BUILD_LOCK%"
)
set NEED_BACKEND_REBUILD=0
set NEED_TASKBAR_HELPER_REBUILD=0
set NEED_ELECTRON_REBUILD=0
set NEED_RENDERER_REBUILD=0
set VITE_DEV_SERVER_URL=
set APP_ALREADY_RUNNING=0
set OWN_BUILD_LOCK=0
set RESTART_RUNNING_APP_AFTER_REBUILD=0
set DESKTOP_AUTO_REBUILD=1
set DESKTOP_DEV_SERVER=1
if "%FORGE_ELECTRON_DESKTOP_STABLE%"=="1" set DESKTOP_AUTO_REBUILD=0
if "%FORGE_ELECTRON_DESKTOP_STABLE%"=="1" set DESKTOP_DEV_SERVER=0
if "%FORGE_ELECTRON_DESKTOP_AUTO_REBUILD%"=="0" set DESKTOP_AUTO_REBUILD=0
if "%FORGE_ELECTRON_DESKTOP_AUTO_REBUILD%"=="1" set DESKTOP_AUTO_REBUILD=1
if "%FORGE_ELECTRON_DESKTOP_DEV_SERVER%"=="0" set DESKTOP_DEV_SERVER=0
if "%FORGE_ELECTRON_DESKTOP_DEV_SERVER%"=="1" set DESKTOP_DEV_SERVER=1

if not exist "%INGEN_ELECTRON_USER_DATA_DIR%" mkdir "%INGEN_ELECTRON_USER_DATA_DIR%"
if not exist "%INGEN_ELECTRON_USER_DATA_DIR%\brain" if exist "%INGEN_ELECTRON_LEGACY_USER_DATA_DIR%\brain" (
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "Copy-Item -LiteralPath '%INGEN_ELECTRON_LEGACY_USER_DATA_DIR%\brain' -Destination '%INGEN_ELECTRON_USER_DATA_DIR%\brain' -Recurse -Force" >> "%LOG%" 2>>&1
)
for %%F in (workspace.json llm-provider-runtime.json llm-providers.json llm-runtime-request.json native-session-ledger.json) do (
  if not exist "%INGEN_ELECTRON_USER_DATA_DIR%\%%F" if exist "%INGEN_ELECTRON_LEGACY_USER_DATA_DIR%\%%F" copy /Y "%INGEN_ELECTRON_LEGACY_USER_DATA_DIR%\%%F" "%INGEN_ELECTRON_USER_DATA_DIR%\%%F" >> "%LOG%" 2>>&1
)
echo Electron userData: %INGEN_ELECTRON_USER_DATA_DIR% >> "%LOG%"

if not "%FORGE_ELECTRON_FORCE_REBUILD%"=="1" if not "%DESKTOP_AUTO_REBUILD%"=="1" (
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$root = '%~dp0'; $dist = Get-Item -LiteralPath (Join-Path $root 'dist-electron\main\main.js') -ErrorAction SilentlyContinue; $renderer = Get-Item -LiteralPath (Join-Path $root 'dist\renderer\index.html') -ErrorAction SilentlyContinue; $sourceRoots = @((Join-Path $root 'src'), (Join-Path $root 'preload.cjs')); $latest = foreach ($entry in $sourceRoots) { Get-ChildItem -LiteralPath $entry -Recurse -File -Include *.ts,*.tsx,*.css,*.cjs -ErrorAction SilentlyContinue }; $latest = $latest | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1; if (-not $dist -or -not $renderer -or ($latest -and ($latest.LastWriteTimeUtc -gt $dist.LastWriteTimeUtc -or $latest.LastWriteTimeUtc -gt $renderer.LastWriteTimeUtc))) { exit 1 }"
  if errorlevel 1 set NEED_ELECTRON_REBUILD=1
  if exist "%FORGE_ELECTRON_EXE%" if exist "%~dp0dist-electron\main\main.js" if exist "%~dp0dist\renderer\index.html" if exist "%FORGE_ELECTRON_BACKEND_EXE%" if exist "%FORGE_WINDOWS_TASKBAR_HELPER_EXE%" (
    if "%NEED_ELECTRON_REBUILD%"=="0" (
      echo Desktop fast path: starting existing Electron build. >> "%LOG%"
      start "" /D "%~dp0" "%FORGE_ELECTRON_EXE%" . "--user-data-dir=%INGEN_ELECTRON_USER_DATA_DIR%" >> "%LOG%" 2>>&1
      exit /b 0
    )
  )
)

C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$root = (Resolve-Path -LiteralPath '%~dp0').Path.TrimEnd('\'); $electron = '%FORGE_ELECTRON_EXE%'; $running = Get-CimInstance Win32_Process -Filter \"Name = 'electron.exe'\" -ErrorAction SilentlyContinue | Where-Object { $_.ExecutablePath -eq $electron -and $_.CommandLine -like ('*' + $root + '*') } | Select-Object -First 1; if ($running) { exit 0 } exit 1"
if not errorlevel 1 set APP_ALREADY_RUNNING=1

if "%APP_ALREADY_RUNNING%"=="1" (
  if "%NEED_ELECTRON_REBUILD%"=="0" (
    echo InGen is already running. Preparing freshness checks without interrupting the active window. >> "%LOG%"
  )
  echo InGen is already running. Any build work will prepare the next launch; the active app will not be restarted. >> "%LOG%"
  set RESTART_RUNNING_APP_AFTER_REBUILD=0
)

2>nul mkdir "%BUILD_LOCK%"
if errorlevel 1 (
  echo Another InGen launcher is already preparing this workspace build. Waiting briefly, then continuing freshness checks. >> "%LOG%"
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$lock = '%BUILD_LOCK%'; $deadline = (Get-Date).AddSeconds(180); while ((Test-Path -LiteralPath $lock) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }; if (Test-Path -LiteralPath $lock) { exit 1 }"
  if errorlevel 1 goto fail
  2>nul mkdir "%BUILD_LOCK%"
  if not errorlevel 1 set OWN_BUILD_LOCK=1
  if "%OWN_BUILD_LOCK%"=="0" echo Build lock was released but could not be reacquired; continuing without interrupting the active app. >> "%LOG%"
  goto build_lock_ready
)
set OWN_BUILD_LOCK=1

:build_lock_ready
if not exist "%FORGE_ELECTRON_BACKEND_EXE%" set NEED_BACKEND_REBUILD=1
if not exist "%FORGE_WINDOWS_TASKBAR_HELPER_EXE%" set NEED_TASKBAR_HELPER_REBUILD=1
if "%FORGE_ELECTRON_FORCE_REBUILD%"=="1" set NEED_BACKEND_REBUILD=1
if "%FORGE_ELECTRON_FORCE_REBUILD%"=="1" set NEED_TASKBAR_HELPER_REBUILD=1
if "%DESKTOP_AUTO_REBUILD%"=="1" (
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$exe = '%FORGE_ELECTRON_BACKEND_EXE%'; $srcRoot = '%REPO_ROOT%\examples\ingen_native_services'; $bin = Get-Item -LiteralPath $exe -ErrorAction SilentlyContinue; $latest = Get-ChildItem -LiteralPath (Join-Path $srcRoot 'src') -Recurse -File -Include *.rs -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1; if (-not $bin -or ($latest -and $latest.LastWriteTimeUtc -gt $bin.LastWriteTimeUtc)) { exit 1 }"
  if errorlevel 1 set NEED_BACKEND_REBUILD=1
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$exe = '%FORGE_WINDOWS_TASKBAR_HELPER_EXE%'; $src = '%REPO_ROOT%\examples\ingen_native_services\src\bin\ingen_windows_taskbar_helper.rs'; $bin = Get-Item -LiteralPath $exe -ErrorAction SilentlyContinue; $source = Get-Item -LiteralPath $src -ErrorAction SilentlyContinue; if (-not $bin -or ($source -and $source.LastWriteTimeUtc -gt $bin.LastWriteTimeUtc)) { exit 1 }"
  if errorlevel 1 set NEED_TASKBAR_HELPER_REBUILD=1
) else (
  if "%NEED_BACKEND_REBUILD%"=="0" echo Desktop stable mode: using existing backend bridge. >> "%LOG%"
  if "%NEED_TASKBAR_HELPER_REBUILD%"=="0" echo Desktop stable mode: using existing taskbar helper. >> "%LOG%"
)

if "%NEED_BACKEND_REBUILD%"=="1" (
  echo Building Rust backend bridge... >> "%LOG%"
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%REPO_ROOT%\scripts\forge-cargo.ps1" build --manifest-path "%REPO_ROOT%\examples\ingen_native_services\Cargo.toml" --bin ingen_electron_backend_bridge >> "%LOG%" 2>>&1
  if errorlevel 1 (
    if exist "%FORGE_ELECTRON_BACKEND_EXE%" (
      echo Backend rebuild failed. Falling back to the existing backend bridge. >> "%LOG%"
    ) else (
      goto fail
    )
  )
)

if "%NEED_TASKBAR_HELPER_REBUILD%"=="1" (
  echo Building Windows taskbar helper... >> "%LOG%"
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%REPO_ROOT%\scripts\forge-cargo.ps1" build --manifest-path "%REPO_ROOT%\examples\ingen_native_services\Cargo.toml" --bin ingen_windows_taskbar_helper >> "%LOG%" 2>>&1
  if errorlevel 1 (
    if exist "%FORGE_WINDOWS_TASKBAR_HELPER_EXE%" (
      echo Taskbar helper rebuild failed. Falling back to the existing taskbar helper. >> "%LOG%"
    ) else (
      goto fail
    )
  )
)

if not exist "%~dp0dist-electron\main\main.js" set NEED_ELECTRON_REBUILD=1
if not exist "%~dp0dist\renderer\index.html" set NEED_RENDERER_REBUILD=1
if "%FORGE_ELECTRON_FORCE_REBUILD%"=="1" set NEED_ELECTRON_REBUILD=1
if "%FORGE_ELECTRON_FORCE_REBUILD%"=="1" set NEED_RENDERER_REBUILD=1
if "%DESKTOP_AUTO_REBUILD%"=="1" (
  if "%NEED_ELECTRON_REBUILD%"=="0" (
    C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$root = '%~dp0'; $dist = Get-Item -LiteralPath (Join-Path $root 'dist-electron\main\main.js') -ErrorAction SilentlyContinue; $sourceRoots = @((Join-Path $root 'src\main'), (Join-Path $root 'src\preload'), (Join-Path $root 'src\shared')); $latest = foreach ($entry in $sourceRoots) { Get-ChildItem -LiteralPath $entry -Recurse -File -Include *.ts,*.tsx -ErrorAction SilentlyContinue }; $extra = foreach ($entry in @((Join-Path $root 'preload.cjs'), (Join-Path $root 'tsconfig.electron.json'))) { Get-Item -LiteralPath $entry -ErrorAction SilentlyContinue }; $latest = @($latest) + @($extra) | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1; if (-not $dist -or ($latest -and $latest.LastWriteTimeUtc -gt $dist.LastWriteTimeUtc)) { exit 1 }"
    if errorlevel 1 set NEED_ELECTRON_REBUILD=1
  )
  if "%NEED_RENDERER_REBUILD%"=="0" (
    C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$root = '%~dp0'; $renderer = Get-Item -LiteralPath (Join-Path $root 'dist\renderer\index.html') -ErrorAction SilentlyContinue; $sourceRoots = @((Join-Path $root 'src\renderer'), (Join-Path $root 'src\shared'), (Join-Path $root 'public')); $latest = foreach ($entry in $sourceRoots) { Get-ChildItem -LiteralPath $entry -Recurse -File -Include *.ts,*.tsx,*.css,*.json,*.svg,*.png,*.jpg,*.jpeg,*.webp -ErrorAction SilentlyContinue }; $extra = foreach ($entry in @((Join-Path $root 'index.html'), (Join-Path $root 'event-text-lab.html'), (Join-Path $root 'vite.config.ts'), (Join-Path $root 'tsconfig.json'), (Join-Path $root 'package.json'))) { Get-Item -LiteralPath $entry -ErrorAction SilentlyContinue }; $latest = @($latest) + @($extra) | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1; if (-not $renderer -or ($latest -and $latest.LastWriteTimeUtc -gt $renderer.LastWriteTimeUtc)) { exit 1 }"
    if errorlevel 1 set NEED_RENDERER_REBUILD=1
  )
) else (
  if "%NEED_ELECTRON_REBUILD%"=="0" echo Desktop stable mode: using existing Electron build. Set FORGE_ELECTRON_DESKTOP_AUTO_REBUILD=1 or FORGE_ELECTRON_FORCE_REBUILD=1 to rebuild. >> "%LOG%"
)

if "%NEED_ELECTRON_REBUILD%"=="1" (
  echo Building Electron main process... >> "%LOG%"
  call npx.cmd tsc -p tsconfig.electron.json >> "%LOG%" 2>>&1
  if errorlevel 1 goto electron_build_failed

  if not exist "%~dp0src\shared\generated\forge-ipc.generated.ts" (
    echo Generating Electron IPC contract... >> "%LOG%"
    call npm.cmd run generate:ipc >> "%LOG%" 2>>&1
    if errorlevel 1 goto electron_build_failed
  )
)

if "%DESKTOP_DEV_SERVER%"=="1" if not "%FORGE_ELECTRON_FORCE_REBUILD%"=="1" if exist "%FORGE_ELECTRON_VITE_SERVER_SCRIPT%" (
  echo Ensuring Vite renderer dev server... >> "%LOG%"
  if exist "%VITE_DEV_SERVER_URL_FILE%" del /Q "%VITE_DEV_SERVER_URL_FILE%" 2>nul
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%FORGE_ELECTRON_VITE_SERVER_SCRIPT%" -ShellRoot "%~dp0" -LogPath "%LOG%" -WorkspaceBuildId "%WORKSPACE_BUILD_ID%" -UrlPath "%VITE_DEV_SERVER_URL_FILE%" >> "%LOG%" 2>>&1
  if exist "%VITE_DEV_SERVER_URL_FILE%" (
    set /p VITE_DEV_SERVER_URL=<"%VITE_DEV_SERVER_URL_FILE%"
  )
  if not "!VITE_DEV_SERVER_URL!"=="" (
    echo Vite renderer dev server ready: !VITE_DEV_SERVER_URL! >> "%LOG%"
    set NEED_RENDERER_REBUILD=0
  ) else (
    echo Vite renderer dev server unavailable. Falling back to renderer build. >> "%LOG%"
  )
)

if "%NEED_RENDERER_REBUILD%"=="1" (
  echo Building Electron renderer... >> "%LOG%"
  call npx.cmd vite build >> "%LOG%" 2>>&1
  if errorlevel 1 goto electron_build_failed
) else if not "!VITE_DEV_SERVER_URL!"=="" (
  echo Using Vite renderer dev server. Set FORGE_ELECTRON_DESKTOP_DEV_SERVER=0 to use built renderer assets. >> "%LOG%"
) else (
  echo Using existing Electron renderer build. Set FORGE_ELECTRON_FORCE_REBUILD=1 to rebuild. >> "%LOG%"
)

goto start_electron

:electron_build_failed
if exist "%~dp0dist-electron\main\main.js" (
  if exist "%~dp0dist\renderer\index.html" (
    echo Electron rebuild failed. Falling back to the existing Electron build. >> "%LOG%"
    goto start_electron
  )
)
goto fail

:start_electron
if "%APP_ALREADY_RUNNING%"=="1" (
  echo Active InGen window preserved. Focusing the existing app instead of restarting it. >> "%LOG%"
  C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -Command "$root = (Resolve-Path -LiteralPath '%~dp0').Path.TrimEnd('\'); $electron = '%FORGE_ELECTRON_EXE%'; $running = Get-CimInstance Win32_Process -Filter \"Name = 'electron.exe'\" -ErrorAction SilentlyContinue | Where-Object { $_.ExecutablePath -eq $electron -and $_.CommandLine -like ('*' + $root + '*') } | Select-Object -First 1; if ($running) { $shell = New-Object -ComObject WScript.Shell; [void]$shell.AppActivate([int]$running.ProcessId) }" >> "%LOG%" 2>>&1
  if "%OWN_BUILD_LOCK%"=="1" if exist "%BUILD_LOCK%" rmdir "%BUILD_LOCK%" 2>nul
  exit /b 0
)
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
start "" /D "%~dp0" "%FORGE_ELECTRON_EXE%" . "--user-data-dir=%INGEN_ELECTRON_USER_DATA_DIR%" >> "%LOG%" 2>>&1
if errorlevel 1 goto fail
if "%OWN_BUILD_LOCK%"=="1" if exist "%BUILD_LOCK%" rmdir "%BUILD_LOCK%" 2>nul
exit /b 0

:fail
echo Launch failed with code %ERRORLEVEL%. See %LOG%.
if "%OWN_BUILD_LOCK%"=="1" if exist "%BUILD_LOCK%" rmdir "%BUILD_LOCK%" 2>nul
start "InGen Electron launch error" notepad.exe "%LOG%"
exit /b %ERRORLEVEL%
