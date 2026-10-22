@echo off
setlocal enabledelayedexpansion
title Launchpad Drummer - Build

:: Standalone production build: frontend + Rust, no installer.
:: Output: target\release\launchpad-drummer.exe
:: Set LPD_NO_PAUSE=1 to skip the "press a key" prompts.

cd /d "%~dp0"

where cargo >nul 2>&1
if %errorlevel% neq 0 (
    if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
        set "PATH=%USERPROFILE%\.cargo\bin;!PATH!"
    ) else (
        echo [FAIL] Rust / Cargo not found. Install from https://rustup.rs/
        goto FAIL
    )
)

where node >nul 2>&1
if %errorlevel% neq 0 (
    echo [FAIL] Node.js not found. Install 22.18 or newer from https://nodejs.org/
    goto FAIL
)

if not exist "node_modules\" (
    echo [INFO] node_modules missing. Installing dependencies...
    call npm install
    if !errorlevel! neq 0 (
        echo [FAIL] npm install failed.
        goto FAIL
    )
)

echo Building. This can take a while...
call npm run tauri -- build --no-bundle
if %errorlevel% neq 0 (
    echo [FAIL] Build failed.
    goto FAIL
)

echo.
echo [OK] Built %~dp0target\release\launchpad-drummer.exe
if not defined LPD_NO_PAUSE pause
exit /b 0

:FAIL
if not defined LPD_NO_PAUSE pause
exit /b 1
