@echo off
rem Start the local streaming server (curl-friendly /stream endpoint).
rem   scripts\serve.bat
rem   set PORT=9000 && scripts\serve.bat
setlocal
call "%~dp0_common.bat" || exit /b 1

echo framefeed serve on http://127.0.0.1:%PORT%  ^(Ctrl-C to stop^)
echo   clips by name:   curl -N "http://127.0.0.1:%PORT%/"   ^(demo library^)
echo   one clip:        curl -N "http://127.0.0.1:%PORT%/sintel?backend=sixel"
echo   your own media:  curl -N "http://127.0.0.1:%PORT%/stream?url=videos/mon-film.mp4"
echo   play through it: scripts\play-server.bat target\fixtures\bad-apple-8s.mp4
echo   health:          curl "http://127.0.0.1:%PORT%/healthz"

"%BIN% serve --allow-local --listen "127.0.0.1:%PORT%" --media "%ROOT%\videos\demo"
