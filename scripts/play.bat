@echo off
rem Play a video directly in this terminal, scaled to the window.
rem   scripts\play.bat                       (8 s Bad Apple fixture)
rem   scripts\play.bat path\to\video.mp4
rem   scripts\play.bat https://host/master.m3u8
rem   set QUALITY=720p && scripts\play.bat  (override the automatic raster)
setlocal
call "%~dp0_common.bat" || exit /b 1

set "SOURCE=%~1"
if not defined SOURCE if defined FRAMEFEED_SOURCE set "SOURCE=%FRAMEFEED_SOURCE%"
rem Relative paths are fine: _common.bat already cd's to the project root.

rem Outside Windows Terminal there is no Sixel: reopen there instead of
rem silently drawing 80x48 half-blocks in conhost or an IDE terminal.
if "%IN_WT%"=="0" (
    if exist "%WT_EXE%" (
        echo framefeed: this terminal cannot draw pixels, reopening in Windows Terminal...
        rem Pass the source through the environment: quoting a spaced path through
        rem wt's own command line loses the quotes and splits the path.
        set "FRAMEFEED_SOURCE=%SOURCE%"
        start "" "%WT_EXE%" new-tab --title "framefeed" cmd /c ""%~f0""
        exit /b 0
    )
    echo framefeed: Windows Terminal 1.22+ not found, falling back to ANSI half-blocks.
)

rem No argument: take the first video dropped in videos\, else the test fixture.
for %%F in ("%ROOT%\videos\*.mp4" "%ROOT%\videos\*.webm" "%ROOT%\videos\*.mkv") do if not defined SOURCE if exist "%%~F" set "SOURCE=%%~F"
if not defined SOURCE set "SOURCE=%ROOT%\target\fixtures\bad-apple-8s.mp4"

if not exist "%SOURCE%" if not "%SOURCE:~0,5%"=="https" if not "%SOURCE:~0,4%"=="http" (
    echo No such file: %SOURCE%
    echo Hint: fixtures live in target\fixtures ^(git-ignored^); pass any path or URL.
    exit /b 1
)

if defined FPS (
    "%BIN%" play "%SOURCE%" --quality "%QUALITY%" --backend "%BACKEND%" --fps "%FPS%" --stats
) else (
    "%BIN%" play "%SOURCE%" --quality "%QUALITY%" --backend "%BACKEND%" --stats
)
