@echo off
setlocal enabledelayedexpansion
chcp 936 >nul 2>nul
title LOL助手 一键打包

rem ============================================================
rem  LOL助手 一键打包脚本（wails amd64 + NSIS）
rem  用法：双击运行，或命令行追加 wails 参数，如  build.bat -clean
rem  构建成功后产物复制到  dist\  目录，方便直接取用
rem ============================================================

rem ── 路径配置 ──────────────────────────────────────────────
set "PROJECT=%~dp0"
if "%PROJECT:~-1%"=="\" set "PROJECT=%PROJECT:~0,-1%"
set "WAILS=%USERPROFILE%\go\bin\wails.exe"
set "NSIS_DIR=%LOCALAPPDATA%\tauri\NSIS"
set "TOOL_DIR=E:\idea\LOL\.tools"
set "OUT_DIR=%PROJECT%\build\bin"
set "DIST_DIR=%PROJECT%\dist"

rem ── 前置检查 ──────────────────────────────────────────────
if not exist "%WAILS%" (
  echo [错误] 未找到 Wails CLI：%WAILS%
  echo        请先安装：go install github.com/wailsapp/wails/v2/cmd/wails@latest
  goto :fail
)
if not exist "%TOOL_DIR%\pnpm.cmd" (
  echo [警告] 未找到 pnpm 垫片：%TOOL_DIR%\pnpm.cmd
  echo        前端构建可能失败，请确认该文件存在
)
set "NSIS_ARGS=-nsis"
if not exist "%NSIS_DIR%\makensis.exe" (
  echo [警告] 未找到 NSIS：%NSIS_DIR%\makensis.exe
  echo        本次仅生成 exe，跳过安装包
  set "NSIS_ARGS="
)

rem ── 打包 ──────────────────────────────────────────────────
set "PATH=%TOOL_DIR%;%NSIS_DIR%;%PATH%"
echo [1/3] wails build -platform windows/amd64 %NSIS_ARGS% %*
cd /d "%PROJECT%"
"%WAILS%" build -platform windows/amd64 %NSIS_ARGS% %*
if errorlevel 1 (
  echo [错误] 打包失败，请检查上方输出
  goto :fail
)

rem ── 产物复制到 dist\ ──────────────────────────────────────
echo [2/3] 复制产物到 dist\ ...
if not exist "%DIST_DIR%" mkdir "%DIST_DIR%"
copy /y "%OUT_DIR%\lol-assistant-amd64-installer.exe" "%DIST_DIR%\" >nul 2>nul
copy /y "%OUT_DIR%\LOL助手.exe" "%DIST_DIR%\" >nul 2>nul

echo [3/3] 打包完成，dist\ 产物：
echo.
dir "%DIST_DIR%\*.exe" 2>nul
echo.
echo 取用目录：%DIST_DIR%
echo 原始输出：%OUT_DIR%
set "EXITCODE=0"
goto :end

:fail
echo.
echo 排查提示：
echo   - pnpm 垫片缺失：确认 %TOOL_DIR%\pnpm.cmd 存在
echo   - NSIS 缺失：确认 Tauri 缓存 %NSIS_DIR%\makensis.exe 存在
echo   - Go 环境：确认 go.exe 与 GOPROXY 可用
set "EXITCODE=1"

:end
echo.
pause
exit /b %EXITCODE%
