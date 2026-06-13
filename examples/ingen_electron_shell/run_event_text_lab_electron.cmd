@echo off
setlocal
cd /d "%~dp0"
set LOG=C:\tmp\ingen-event-text-lab-electron.log
if not exist C:\tmp mkdir C:\tmp
echo ==== InGen Event Text Lab Electron launch %DATE% %TIME% ==== > "%LOG%"
set INGEN_EVENT_TEXT_LAB=1

if not exist "%~dp0dist-electron\main\main.js" (
  echo Building Electron main... >> "%LOG%"
  call npm.cmd run build >> "%LOG%" 2>>&1
  if errorlevel 1 goto fail
)

if not exist "%~dp0dist\renderer\event-text-lab.html" (
  echo Building Electron renderer... >> "%LOG%"
  call npm.cmd run build >> "%LOG%" 2>>&1
  if errorlevel 1 goto fail
)

if not exist "%~dp0node_modules\electron\dist\electron.exe" (
  echo Electron executable is missing. Run npm install in %~dp0. >> "%LOG%"
  goto fail
)

echo Starting Event Text Lab Electron... >> "%LOG%"
C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -Command "Start-Process -FilePath '%~dp0node_modules\electron\dist\electron.exe' -ArgumentList '. --event-text-lab' -WorkingDirectory '%~dp0' -WindowStyle Normal" >> "%LOG%" 2>>&1
if errorlevel 1 goto fail
exit /b 0

:fail
echo Launch failed with code %ERRORLEVEL%. See %LOG%.
start "InGen Event Text Lab launch error" notepad.exe "%LOG%"
exit /b %ERRORLEVEL%
