@echo off
setlocal enabledelayedexpansion
title Launchpad Drummer (LPD-01)

:: Ensure execution from the repository directory
cd /d "%~dp0"

echo ====================================================
echo             LAUNCHPAD DRUMMER (LPD-01)
echo ====================================================
echo.
echo [1/4] Checking system requirements...

:: 1. Check Rust / Cargo
where cargo >nul 2>&1
if %errorlevel% neq 0 (
    if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
        set "PATH=%USERPROFILE%\.cargo\bin;!PATH!"
        set "CARGO_FOUND=1"
    ) else (
        echo [FAIL] Rust / Cargo is not found in PATH or %USERPROFILE%\.cargo\bin.
        echo        Please install Rust via rustup from: https://rustup.rs/
        echo.
        pause
        exit /b 1
    )
) else (
    set "CARGO_FOUND=1"
)

for /f "tokens=*" %%v in ('cargo --version 2^>nul') do set "CARGO_VER=%%v"
echo   [OK] Rust / Cargo: %CARGO_VER%

:: 2. Check Node.js
where node >nul 2>&1
if %errorlevel% neq 0 (
    echo [FAIL] Node.js is not found in PATH.
    echo        Please install Node.js ^(v20 or v22 LTS^) from: https://nodejs.org/
    echo.
    pause
    exit /b 1
)

for /f "tokens=*" %%v in ('node -v 2^>nul') do set "NODE_VER=%%v"
echo   [OK] Node.js:      %NODE_VER%

:: 3. Check npm
where npm >nul 2>&1
if %errorlevel% neq 0 (
    echo [FAIL] npm is not found in PATH.
    echo.
    pause
    exit /b 1
)

for /f "tokens=*" %%v in ('npm -v 2^>nul') do set "NPM_VER=%%v"
echo   [OK] npm:          v%NPM_VER%

:: 4. Check & install node_modules
echo.
echo [2/4] Verifying dependencies...
if not exist "node_modules\" (
    echo   [INFO] node_modules not found. Installing frontend dependencies...
    call npm install
    if %errorlevel% neq 0 (
        echo [FAIL] npm install failed.
        echo.
        pause
        exit /b 1
    )
    echo   [OK] Dependencies installed successfully.
) else (
    echo   [OK] Node modules present.
)

:: Route execution based on command-line argument
if "%~1"=="probe" goto RUN_PROBE
if "%~1"=="--probe" goto RUN_PROBE
if "%~1"=="cli" goto RUN_CLI
if "%~1"=="--cli" goto RUN_CLI
if "%~1"=="test" goto RUN_TEST
if "%~1"=="--test" goto RUN_TEST
if "%~1"=="release" goto RUN_RELEASE
if "%~1"=="--release" goto RUN_RELEASE

:: Default: Run Tauri in dev mode
echo.
echo [3/4] Launching Launchpad Drummer GUI (dev)...
echo [4/4] Press Ctrl+C in this console or close the application window to exit.
echo ====================================================
echo.

call npm run tauri dev
if %errorlevel% neq 0 (
    echo.
    echo [WARN] Launchpad Drummer exited with code %errorlevel%.
    pause
)
exit /b %errorlevel%

:RUN_RELEASE
echo.
echo [3/4] Launching Launchpad Drummer GUI (release mode)...
echo [4/4] Press Ctrl+C in this console or close the application window to exit.
echo ====================================================
echo.
call npm run tauri dev -- --release
if %errorlevel% neq 0 (
    echo.
    echo [WARN] Application exited with code %errorlevel%.
    pause
)
exit /b %errorlevel%

:RUN_CLI
:: `shift` does not change %*, so collect everything after the routing token one argument at a
:: time. %1 keeps its quotes (paths with spaces). Delayed expansion is off so "!" survives.
setlocal disabledelayedexpansion
set "CLI_ARGS="
shift
:CLI_COLLECT
if "%~1"=="" goto CLI_RUN
set CLI_ARGS=%CLI_ARGS% %1
shift
goto CLI_COLLECT

:CLI_RUN
echo.
echo [3/4] Launching Launchpad Drummer CLI Player...
echo ====================================================
echo.
cargo run -p lpd-cli --%CLI_ARGS%
set "CLI_RC=%errorlevel%"
if not "%CLI_RC%"=="0" (
    echo.
    pause
)
exit /b %CLI_RC%

:RUN_PROBE
echo.
echo [3/4] Launching Launchpad Hardware Probe...
echo ====================================================
echo.
cargo run -p lp-probe
if %errorlevel% neq 0 (
    echo.
    pause
)
exit /b %errorlevel%

:RUN_TEST
echo.
echo [3/4] Running full test suite (Rust + Frontend)...
echo ====================================================
echo.
echo === Cargo Tests ===
cargo test --workspace
if %errorlevel% neq 0 (
    echo [FAIL] Cargo tests failed.
    pause
    exit /b 1
)
echo.
echo === Frontend Tests ^& Checks ===
call npm test
if %errorlevel% neq 0 (
    echo [FAIL] npm test failed.
    pause
    exit /b 1
)
call npm run check
if %errorlevel% neq 0 (
    echo [FAIL] svelte-check failed.
    pause
    exit /b 1
)
call npm run build
if %errorlevel% neq 0 (
    echo [FAIL] Frontend build failed.
    pause
    exit /b 1
)
echo.
echo ====================================================
echo [OK] All tests and checks passed cleanly!
echo ====================================================
exit /b 0
