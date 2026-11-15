@echo off
rem Functional test of the Rust version on Windows. Put this file next to ClaudeVivariumRS.exe
rem (or claude-vivarium-rs.exe) and double-click it. Nothing is installed and the screen is not locked.
rem Results: the folder "vivarium-test" next to this file (test.log and PNG files).
setlocal
cd /d "%~dp0"
set EXE=ClaudeVivariumRS.exe
if not exist "%EXE%" set EXE=claude-vivarium-rs.exe
if not exist "%EXE%" (echo Program not found next to this script & pause & exit /b 1)
set OUT=%~dp0vivarium-test
if not exist "%OUT%" mkdir "%OUT%"
set LOG=%OUT%\test.log
echo Claude Vivarium test %DATE% %TIME% > "%LOG%"
ver >> "%LOG%"
echo arch %PROCESSOR_ARCHITECTURE% >> "%LOG%"

echo [1/5] self test (fonts, sessions, graphics, PNG of every theme) ...
echo ===== selftest >> "%LOG%"
"%EXE%" selftest --out "%OUT%\themes" >> "%LOG%" 2>&1
echo exit code %ERRORLEVEL% >> "%LOG%"

echo [2/5] window for 6 seconds ...
echo ===== windowed >> "%LOG%"
set VIVARIUM_TEST=6;%OUT%\windowed
"%EXE%" run --windowed --demo >> "%LOG%" 2>&1
echo exit code %ERRORLEVEL% >> "%LOG%"

echo [3/5] full screen as a screen saver (/s) for 8 seconds - do not touch mouse or keyboard ...
echo ===== full screen /s >> "%LOG%"
set VIVARIUM_TEST=8;%OUT%\full
"%EXE%" /s >> "%LOG%" 2>&1
echo exit code %ERRORLEVEL% >> "%LOG%"
set VIVARIUM_TEST=

echo [4/5] benchmark for 15 seconds ...
echo ===== bench >> "%LOG%"
"%EXE%" bench --theme synthwave --seconds 15 >> "%LOG%" 2>&1
echo exit code %ERRORLEVEL% >> "%LOG%"

echo [5/5] collecting the program's own log ...
echo ===== vivarium-rs.log >> "%LOG%"
type "%LOCALAPPDATA%\ClaudeVivarium\vivarium-rs.log" >> "%LOG%" 2>&1
echo ===== done >> "%LOG%"
echo.
echo Done. Results are in "%OUT%".
pause
