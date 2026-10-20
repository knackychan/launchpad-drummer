@echo off
setlocal enabledelayedexpansion
title Launchpad Drummer (LPD-01)

:: Ensure execution from the repository directory
cd /d "%~dp0"

:: Usage: run.bat [release | build | dev | test | probe | cli <args>]
::   (none), release  open the built app directly: no Node, Cargo or Vite involved
::   build            build the production frontend and executable (no installer)
::   dev              development workflow (Vite dev server + debug build)
::   test, probe, cli unchanged
:: Set LPD_NO_PAUSE=1 to skip the "press a key" prompts (scripts, CI).
:: The launcher expects Cargo's default target folder (target\release); a custom
:: CARGO_TARGET_DIR is not supported here.

:: Route before any tool check: a plain launch must not need Rust, Node or npm.
if "%~1"=="" goto RUN_APP
if /i "%~1"=="release" goto RUN_APP
if /i "%~1"=="--release" goto RUN_APP
if /i "%~1"=="build" goto RUN_BUILD
if /i "%~1"=="--build" goto RUN_BUILD
if /i "%~1"=="dev" goto RUN_DEV
if /i "%~1"=="--dev" goto RUN_DEV
if /i "%~1"=="probe" goto RUN_PROBE
if /i "%~1"=="--probe" goto RUN_PROBE
if /i "%~1"=="cli" goto RUN_CLI
if /i "%~1"=="--cli" goto RUN_CLI
if /i "%~1"=="test" goto RUN_TEST
if /i "%~1"=="--test" goto RUN_TEST
echo [FAIL] Unknown mode "%~1".
echo        Use: run.bat [release ^| build ^| dev ^| test ^| probe ^| cli ^<args^>]
call :PAUSE
exit /b 2

:RUN_APP
set "APP_EXE=%~dp0target\release\launchpad-drummer.exe"
if not exist "%APP_EXE%" (
    echo [FAIL] The built app was not found:
    echo        %APP_EXE%
    echo        Build it once with:  run.bat build
    call :PAUSE
    exit /b 1
)
"%APP_EXE%"
set "APP_RC=%errorlevel%"
if not "%APP_RC%"=="0" (
    echo.
    echo [WARN] Launchpad Drummer exited with code %APP_RC%.
    call :PAUSE
)
exit /b %APP_RC%

:RUN_BUILD
call :NEED_CARGO || exit /b 1
call :NEED_NODE || exit /b 1
call :NEED_MODULES || exit /b 1
echo.
echo Building the production app. This compiles the frontend and the Rust code and can take a while.
echo ====================================================
call npm run tauri build
if %errorlevel% neq 0 (
    echo.
    echo [FAIL] Build failed.
    call :PAUSE
    exit /b 1
)
echo.
echo [OK] Built target\release\launchpad-drummer.exe. Start it with:  run.bat
exit /b 0

:RUN_DEV
call :NEED_CARGO || exit /b 1
call :NEED_NODE || exit /b 1
call :NEED_MODULES || exit /b 1
echo.
echo Launching Launchpad Drummer (development build). Close the window or press Ctrl+C to exit.
echo ====================================================
call npm run tauri dev
if %errorlevel% neq 0 (
    echo.
    echo [WARN] Launchpad Drummer exited with code %errorlevel%.
    call :PAUSE
)
exit /b %errorlevel%

:RUN_CLI
call :NEED_CARGO || exit /b 1
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
echo Launching Launchpad Drummer CLI Player...
echo ====================================================
echo.
cargo run -p lpd-cli --%CLI_ARGS%
set "CLI_RC=%errorlevel%"
if not "%CLI_RC%"=="0" (
    echo.
    call :PAUSE
)
exit /b %CLI_RC%

:RUN_PROBE
call :NEED_CARGO || exit /b 1
echo.
echo Launching Launchpad Hardware Probe...
echo ====================================================
echo.
cargo run -p lp-probe
if %errorlevel% neq 0 (
    echo.
    call :PAUSE
)
exit /b %errorlevel%

:RUN_TEST
call :NEED_CARGO || exit /b 1
call :NEED_NODE || exit /b 1
call :NEED_MODULES || exit /b 1
echo.
echo Running full test suite (Rust + Frontend)...
echo ====================================================
echo.
echo === Cargo Tests ===
cargo test --workspace
if %errorlevel% neq 0 (
    echo [FAIL] Cargo tests failed.
    call :PAUSE
    exit /b 1
)
echo.
echo === Frontend Tests ^& Checks ===
call npm test
if %errorlevel% neq 0 (
    echo [FAIL] npm test failed.
    call :PAUSE
    exit /b 1
)
call npm run check
if %errorlevel% neq 0 (
    echo [FAIL] svelte-check failed.
    call :PAUSE
    exit /b 1
)
call npm run build
if %errorlevel% neq 0 (
    echo [FAIL] Frontend build failed.
    call :PAUSE
    exit /b 1
)
echo.
echo ====================================================
echo [OK] All tests and checks passed cleanly!
echo ====================================================
exit /b 0

:: ---- requirement checks, used only by the routes that need them ----

:NEED_CARGO
where cargo >nul 2>&1
if %errorlevel% neq 0 (
    if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
        set "PATH=%USERPROFILE%\.cargo\bin;!PATH!"
    ) else (
        echo [FAIL] Rust / Cargo is not found in PATH or %USERPROFILE%\.cargo\bin.
        echo        Please install Rust via rustup from: https://rustup.rs/
        call :PAUSE
        exit /b 1
    )
)
for /f "tokens=*" %%v in ('cargo --version 2^>nul') do echo   [OK] Rust / Cargo: %%v
exit /b 0

:NEED_NODE
where node >nul 2>&1
if %errorlevel% neq 0 (
    echo [FAIL] Node.js is not found in PATH.
    echo        Please install Node.js 22.18 or newer from: https://nodejs.org/
    call :PAUSE
    exit /b 1
)
where npm >nul 2>&1
if %errorlevel% neq 0 (
    echo [FAIL] npm is not found in PATH.
    call :PAUSE
    exit /b 1
)
for /f "tokens=*" %%v in ('node -v 2^>nul') do echo   [OK] Node.js: %%v ^(22.18 or newer required^)
exit /b 0

:NEED_MODULES
if not exist "node_modules\" (
    echo   [INFO] node_modules not found. Installing frontend dependencies...
    call npm install
    if !errorlevel! neq 0 (
        echo [FAIL] npm install failed.
        call :PAUSE
        exit /b 1
    )
    echo   [OK] Dependencies installed successfully.
) else (
    echo   [OK] Node modules present.
)
exit /b 0

:PAUSE
if not defined LPD_NO_PAUSE pause
exit /b 0
