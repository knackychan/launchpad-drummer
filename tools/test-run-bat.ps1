# Regression test for run.bat's CLI route (audit finding 10).
# `cargo` is replaced by a stub that echoes its arguments, so no hardware or build is needed.
# Usage: pwsh tools/test-run-bat.ps1        (exit code 0 = all checks passed)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$work = Join-Path ([IO.Path]::GetTempPath()) ("run-bat-test-" + [guid]::NewGuid().ToString('N'))
$stubs = Join-Path $work 'stubs'
New-Item -ItemType Directory -Force -Path $stubs | Out-Null
$failures = 0

# cargo stub: `--version` for the launcher's check, otherwise echo arguments and exit with $env:STUB_RC.
Set-Content -Path (Join-Path $stubs 'cargo.cmd') -Encoding ascii -Value @'
@echo off
if "%~1"=="--version" (
    echo cargo 0.0.0-stub
    exit /b 0
)
echo STUB-ARGS:%*
exit /b %STUB_RC%
'@

function Invoke-Launcher([string]$arguments, [int]$rc = 0) {
    $env:STUB_RC = "$rc"
    $env:PATH = "$stubs;$env:PATH"
    $out = & cmd.exe /c "`"$repo\run.bat`" $arguments < NUL" 2>&1 | Out-String
    return @{ Output = $out; Code = $LASTEXITCODE }
}

function Check([string]$name, [bool]$ok, [string]$detail = '') {
    if ($ok) { Write-Host "PASS  $name" } else { Write-Host "FAIL  $name`n$detail"; $script:failures++ }
}

$savedPath = $env:PATH
$setDir = Join-Path $work 'my kit'          # a path with a space
$cases = @(
    @{ Name = 'cli play';        Args = "cli play `"$setDir`"";                           Expect = "STUB-ARGS:run -p lpd-cli -- play `"$setDir`"" },
    @{ Name = '--cli play';      Args = "--cli play `"$setDir`"";                         Expect = "STUB-ARGS:run -p lpd-cli -- play `"$setDir`"" },
    @{ Name = 'cli new-set';     Args = "cli new-set `"$setDir`" `"$work\some samples`""; Expect = "STUB-ARGS:run -p lpd-cli -- new-set `"$setDir`" `"$work\some samples`"" },
    @{ Name = '--cli new-set';   Args = "--cli new-set `"$setDir`" `"$work\some samples`""; Expect = "STUB-ARGS:run -p lpd-cli -- new-set `"$setDir`" `"$work\some samples`"" },
    @{ Name = 'play flags';      Args = "cli play `"$setDir`" --buffer 128 --asio";       Expect = "STUB-ARGS:run -p lpd-cli -- play `"$setDir`" --buffer 128 --asio" }
)
foreach ($case in $cases) {
    $env:PATH = $savedPath
    $r = Invoke-Launcher $case.Args
    Check $case.Name ($r.Output.Contains($case.Expect) -and $r.Code -eq 0) $r.Output
    Check "$($case.Name): routing token not forwarded" (-not ($r.Output -match 'STUB-ARGS:run -p lpd-cli -- (--)?cli ')) $r.Output
}

$env:PATH = $savedPath
$r = Invoke-Launcher "cli play x" 7
Check 'child exit code survives the launcher' ($r.Code -eq 7) "exit code was $($r.Code)`n$($r.Output)"

# A real run (no stub): New creates a kit, and a second New in the same folder is refused.
$env:PATH = $savedPath
$real = Join-Path $work 'real'
$samples = Join-Path $work 'samples'
New-Item -ItemType Directory -Force -Path $samples | Out-Null
[IO.File]::WriteAllBytes((Join-Path $samples 'tick.wav'), [byte[]](
    0x52,0x49,0x46,0x46,0x2C,0x00,0x00,0x00,0x57,0x41,0x56,0x45,0x66,0x6D,0x74,0x20,0x10,0x00,0x00,0x00,0x01,0x00,0x01,0x00,
    0x40,0x1F,0x00,0x00,0x80,0x3E,0x00,0x00,0x02,0x00,0x10,0x00,0x64,0x61,0x74,0x61,0x08,0x00,0x00,0x00,
    0x10,0x20,0x30,0x40,0x30,0x20,0x10,0x00))
if ($env:RUN_BAT_REAL -ne '0') {
    $out = & cmd.exe /c "`"$repo\run.bat`" cli new-set `"$real`" `"$samples`" < NUL" 2>&1 | Out-String
    Check 'real new-set creates the kit' ((Test-Path (Join-Path $real 'set.json')) -and $LASTEXITCODE -eq 0) $out
    $before = Get-Content (Join-Path $real 'set.json') -Raw
    $out = & cmd.exe /c "`"$repo\run.bat`" cli new-set `"$real`" `"$samples`" < NUL" 2>&1 | Out-String
    Check 'real second new-set is refused with a non-zero code' ($LASTEXITCODE -ne 0 -and $out -match 'already contains a set') $out
    Check 'the existing kit is untouched' ((Get-Content (Join-Path $real 'set.json') -Raw) -eq $before)
}

Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
if ($failures) { Write-Host "$failures check(s) failed"; exit 1 }
Write-Host 'all launcher checks passed'
