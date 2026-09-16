@echo off
title SpaceSpore - Voxel Planet
cd /d "%~dp0"
set "RUST_BACKTRACE=1"
echo ========================================
echo    SpaceSpore - Voxel Planet Launcher
echo ========================================
if not exist "target\release\spacespore.exe" (
    echo Compilation initiale en cours...
    cargo build --release 2>&1
    if %errorlevel% neq 0 (
        echo.
        echo [ERREUR] La compilation a echoue.
        exit /b 1
    )
)
echo.
echo Lancement de SpaceSpore...
echo.
echo Controles:
echo   WASD / Fleches  - Rotation camera
echo   Clic droit      - Rotation libre
echo   Molette         - Zoom
echo.
echo.
echo Le jeu est en cours d'execution. Fermez cette fenetre pour quitter.
target\release\spacespore.exe
set "GAME_EXIT=%errorlevel%"
if not "%GAME_EXIT%"=="0" (
    echo.
    echo [ERREUR] SpaceSpore s'est arrete avec le code %GAME_EXIT%.
)
exit /b %GAME_EXIT%
