@echo off
rem Play a video directly in this terminal, scaled to the window.
rem   scripts\play.bat                       (lists videos\ and asks which one)
rem   scripts\play.bat path\to\video.mp4
rem   scripts\play.bat https://host/master.m3u8
rem   set QUALITY=720p && scripts\play.bat  (override the automatic raster)
setlocal
call "%~dp0_common.bat" || exit /b 1

set "SOURCE=%~1"
if not defined SOURCE if defined FRAMEFEED_SOURCE set "SOURCE=%FRAMEFEED_SOURCE%"

rem No argument: list the local videos and let the user pick one.
if not defined SOURCE call :pick_video
if not defined SOURCE exit /b 1

rem Outside Windows Terminal there is no Sixel: reopen there instead of
rem silently drawing 80x48 half-blocks in conhost or an IDE terminal.
if "%IN_WT%"=="0" (
    if exist "%WT_EXE%" (
        echo framefeed: this terminal cannot draw pixels, reopening in Windows Terminal...
        rem Pass the source through the environment: quoting a spaced path through
        rem wt's own command line loses the quotes and splits the path.
        rem Keep -d after `new-tab`: wt only parses subcommand options there, and
        rem anything before it becomes the tab's commandline (hence 0x80070002).
        set "FRAMEFEED_SOURCE=%SOURCE%"
        start "" "%WT_EXE%" new-tab -d "%ROOT%" --title "framefeed" cmd /k ""%~f0""
        exit /b 0
    )
    echo framefeed: Windows Terminal 1.22+ not found, falling back to ANSI half-blocks.
)

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
exit /b %errorlevel%

rem --- interactive picker ----------------------------------------------------

:pick_video
set "COUNT=0"
for /r "%ROOT%\videos" %%F in (*.mp4 *.webm *.mkv) do if exist "%%~fF" call :add_video "%%~fF"
if "%COUNT%"=="0" (
    echo No videos found under "%ROOT%\videos".
    echo Drop an .mp4/.webm/.mkv there, or pass a path or URL.
    exit /b 1
)
echo.
echo Available videos:
echo.
for /l %%I in (1,1,%COUNT%) do call :show_video %%I
echo.

set "TRIES=0"
:pick_video_ask
set "CHOICE="
set /p "CHOICE=Play which video? [1-%COUNT%, q to quit] "
if defined CHOICE goto pick_video_chosen
set /a TRIES+=1
if %TRIES% geq 5 exit /b 1
goto pick_video_ask

:pick_video_chosen
if /i "%CHOICE%"=="q" exit /b 1
set "SOURCE="
for /l %%I in (1,1,%COUNT%) do if "%CHOICE%"=="%%I" call :select_video %%I
if defined SOURCE exit /b 0
echo Please enter a number between 1 and %COUNT%, or q to quit.
goto pick_video_ask

:add_video
set /a COUNT+=1
set "VIDEO_%COUNT%=%~1"
exit /b 0

:show_video
call set "REL=%%VIDEO_%1:%ROOT%\=%%"
echo   [%1] %REL%
exit /b 0

:select_video
call set "SOURCE=%%VIDEO_%1%%"
exit /b 0
