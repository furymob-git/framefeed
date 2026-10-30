@echo off
rem Shared setup for the local framefeed helper scripts. Called, never run alone.
rem This folder is git-ignored: it holds convenience wrappers with machine paths.

for %%I in ("%~dp0..") do set "ROOT=%%~fI"
cd /d "%ROOT%"

if not defined PORT set "PORT=8080"
rem auto = the binary measures the real terminal and sizes the raster itself.
if not defined QUALITY set "QUALITY=auto"
if not defined BACKEND set "BACKEND=auto"
if not defined FPS set "FPS="

rem Real console size, so serve clients ask for the window they are actually in.
rem (play measures it itself; this only matters for the ?cols=&rows= query.)
if not defined COLS (
    for /f "usebackq delims=" %%A in (`powershell -NoProfile -Command "$s=$Host.UI.RawUI.WindowSize; ($s.Width.ToString() + ' ' + $s.Height.ToString())"`) do (
        for /f "tokens=1,2" %%B in ("%%A") do (
            set "COLS=%%B"
            set "ROWS=%%C"
        )
    )
)
if not defined COLS set "COLS=80"
if not defined ROWS set "ROWS=24"

rem Windows Terminal is the only pixel-capable terminal here (Sixel).
set "IN_WT=0"
if defined WT_SESSION set "IN_WT=1"
if defined WT_PROFILE_ID set "IN_WT=1"
set "WT_EXE=%LOCALAPPDATA%\Microsoft\WindowsApps\wt.exe"

set "BIN=%ROOT%\target\release\framefeed.exe"
if not exist "%BIN%" (
    echo framefeed binary missing, building with cargo build --release...
    cargo build --release
    if errorlevel 1 exit /b 1
)

rem Prefer the ffmpeg build unpacked under target\tools, or winget's package, otherwise rely on PATH.
if not defined FRAMEFEED_FFMPEG (
    for /d %%D in ("%ROOT%\target\tools\*") do (
        if exist "%%~fD\bin\ffmpeg.exe" (
            set "FRAMEFEED_FFMPEG=%%~fD\bin\ffmpeg.exe"
            set "FRAMEFEED_FFPROBE=%%~fD\bin\ffprobe.exe"
        )
    )
)
if not defined FRAMEFEED_FFMPEG (
    for /d %%D in ("%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg*") do (
        for /d %%S in ("%%~fD\ffmpeg*") do (
            if exist "%%~fS\bin\ffmpeg.exe" (
                set "FRAMEFEED_FFMPEG=%%~fS\bin\ffmpeg.exe"
                set "FRAMEFEED_FFPROBE=%%~fS\bin\ffprobe.exe"
            )
        )
    )
)

exit /b 0
