@echo off
setlocal enabledelayedexpansion
chcp 437 >nul 2>nul
title LOL Assistant Build

rem ============================================================
rem  LOL Assistant build script (wails amd64 + NSIS)
rem  Usage: double-click, or  build.bat -clean
rem  Output is copied to dist\ after a successful build.
rem  NOTE: keep this file ASCII-only. Non-ASCII breaks cmd.exe.
rem ============================================================

rem ---- paths ----------------------------------------------------
set "PROJECT=%~dp0"
if "%PROJECT:~-1%"=="\" set "PROJECT=%PROJECT:~0,-1%"
set "WAILS=%USERPROFILE%\go\bin\wails.exe"
set "NSIS_DIR=%LOCALAPPDATA%\tauri\NSIS"
set "TOOL_DIR=E:\idea\LOL\.tools"
set "OUT_DIR=%PROJECT%\build\bin"
set "DIST_DIR=%PROJECT%\dist"

rem ---- preflight ------------------------------------------------
if not exist "%WAILS%" (
  echo [ERROR] Wails CLI not found: %WAILS%
  echo         Install: go install github.com/wailsapp/wails/v2/cmd/wails@latest
  goto :fail
)
if not exist "%TOOL_DIR%\pnpm.cmd" (
  echo [WARN] pnpm shim not found: %TOOL_DIR%\pnpm.cmd
  echo        Frontend install may fail if missing.
)
set "NSIS_ARGS=-nsis"
if not exist "%NSIS_DIR%\makensis.exe" (
  echo [WARN] NSIS not found: %NSIS_DIR%\makensis.exe
  echo        Will build exe only, skip installer.
  set "NSIS_ARGS="
)

set "PATH=%TOOL_DIR%;%NSIS_DIR%;%PATH%"

rem ---- ensure project.nsi UTF-8 BOM -----------------------------
rem makensis fails with "Bad text encoding" if Chinese NSI lacks BOM
set "NSI=%PROJECT%\build\windows\installer\project.nsi"
pwsh -NoProfile -ExecutionPolicy Bypass -Command "$p=$env:NSI; $b=[IO.File]::ReadAllBytes($p); if (-not ($b.Length -ge 3 -and $b[0] -eq 0xEF -and $b[1] -eq 0xBB -and $b[2] -eq 0xBF)) { $t=[IO.File]::ReadAllText($p,[Text.UTF8Encoding]::new($false)); [IO.File]::WriteAllText($p,$t,[Text.UTF8Encoding]::new($true)); Write-Host '[NSIS] BOM added to project.nsi' } else { Write-Host '[NSIS] project.nsi already has UTF-8 BOM' }"
if errorlevel 1 (
  echo [WARN] BOM step failed; makensis may report Bad text encoding
)

echo [1/3] wails build -platform windows/amd64 %NSIS_ARGS% %*
cd /d "%PROJECT%"
"%WAILS%" build -platform windows/amd64 %NSIS_ARGS% %*
if errorlevel 1 (
  echo [ERROR] Build failed. See output above.
  goto :fail
)

rem ---- copy artifacts to dist\ -----------------------------------
echo [2/3] Copy artifacts to dist\ ...
if not exist "%DIST_DIR%" mkdir "%DIST_DIR%"
copy /y "%OUT_DIR%\lol-assistant-amd64-installer.exe" "%DIST_DIR%\" >nul 2>nul
copy /y "%OUT_DIR%\LOL*.exe" "%DIST_DIR%\" >nul 2>nul

echo [3/3] Done. dist\ contents:
echo.
dir "%DIST_DIR%\*.exe" 2>nul
echo.
echo Output dir: %DIST_DIR%
echo Build dir:  %OUT_DIR%
set "EXITCODE=0"
goto :end

:fail
echo.
echo Troubleshoot:
echo   - pnpm shim: ensure %TOOL_DIR%\pnpm.cmd exists
echo   - NSIS: ensure %NSIS_DIR%\makensis.exe exists
echo   - Go: ensure go.exe works and GOPROXY is set
set "EXITCODE=1"

:end
echo.
pause
exit /b %EXITCODE%
