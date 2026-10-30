@echo off
rem Watch a video served by the local framefeed server (curl client).
rem Requires scripts\serve.bat running in another window.
rem   scripts\play-server.bat
rem   scripts\play-server.bat path\to\video.mp4
setlocal
call "%~dp0_common.bat" || exit /b 1

set "SOURCE=%~1"
if not defined SOURCE if defined FRAMEFEED_SOURCE set "SOURCE=%FRAMEFEED_SOURCE%"
rem Relative paths are fine: _common.bat already cd's to the project root.

rem Same rule as play.bat: this terminal decides the backend, so reopen in WT.
if "%IN_WT%"=="0" (
    if exist "%WT_EXE%" (
        echo framefeed: this terminal cannot draw pixels, reopening in Windows Terminal...
        set "FRAMEFEED_SOURCE=%SOURCE%"
        start "" "%WT_EXE%" new-tab --title "framefeed" cmd /c ""%~f0""
        exit /b 0
    )
    echo framefeed: Windows Terminal 1.22+ not found, falling back to ANSI half-blocks.
)

for %%F in ("%ROOT%\videos\*.mp4" "%ROOT%\videos\*.webm" "%ROOT%\videos\*.mkv") do if not defined SOURCE if exist "%%~F" set "SOURCE=%%~F"
if not defined SOURCE set "SOURCE=%ROOT%\target\fixtures\bad-apple-8s.mp4"

rem The server cannot detect this terminal: resolve "auto" here.
if /i "%BACKEND%"=="auto" if defined WT_SESSION set "BACKEND=sixel"
if /i "%BACKEND%"=="auto" set "BACKEND=ansi"

curl -fsS --max-time 2 "http://127.0.0.1:%PORT%/healthz" >nul 2>&1
if errorlevel 1 (
    echo No server on 127.0.0.1:%PORT% - start it with scripts\serve.bat
    exit /b 1
)

set "FPS_ARG="
if defined FPS set "FPS_ARG=--data-urlencode fps=%FPS%"

curl -sS -N -G "http://127.0.0.1:%PORT%/stream" ^
 --data-urlencode "url=%SOURCE%" ^
 --data-urlencode "q=%QUALITY%" ^
 --data-urlencode "backend=%BACKEND%" ^
 --data-urlencode "cols=%COLS%" ^
 --data-urlencode "rows=%ROWS%" %FPS_ARG%
