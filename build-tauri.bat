@echo off
setlocal
title LOL Assistant Build (Tauri)

rem ============================================================
rem  LOL Assistant build script (Tauri / NSIS)
rem  Usage: double-click, or  build-tauri.bat --debug
rem  Output: src-tauri\target\release\bundle\...
rem  NOTE: keep this file ASCII-only. Non-ASCII breaks cmd.exe.
rem ============================================================

set "PROJECT=%~dp0"
if "%PROJECT:~-1%"=="\" set "PROJECT=%PROJECT:~0,-1%"

where pnpm >nul 2>nul
if errorlevel 1 (
  echo [ERROR] pnpm not found on PATH
  goto :fail
)

where cargo >nul 2>nul
if errorlevel 1 (
  echo [ERROR] cargo not found on PATH
  goto :fail
)

rem Load updater signing key if not already set (required by createUpdaterArtifacts)
if defined TAURI_SIGNING_PRIVATE_KEY goto :key_ok
if not exist "%USERPROFILE%\.tauri\lol-assistant.key" goto :key_missing
for /f "usebackq delims=" %%A in ("%USERPROFILE%\.tauri\lol-assistant.key") do set "TAURI_SIGNING_PRIVATE_KEY=%%A"
if not defined TAURI_SIGNING_PRIVATE_KEY goto :key_missing
:key_ok

cd /d "%PROJECT%"
call pnpm install --frozen-lockfile
if errorlevel 1 goto :fail

call pnpm tauri build %*
if errorlevel 1 goto :fail

echo.
echo [OK] Build finished. See src-tauri\target\release\bundle\
set "EXITCODE=0"
goto :end

:key_missing
echo [ERROR] TAURI_SIGNING_PRIVATE_KEY not set and %USERPROFILE%\.tauri\lol-assistant.key not found.
echo         Updater artifacts require signing. See docs\RELEASE.md
set "EXITCODE=1"
goto :end

:fail
echo [ERROR] Build failed. See output above.
set "EXITCODE=1"

:end
pause
exit /b %EXITCODE%
