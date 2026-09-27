@echo off
setlocal enabledelayedexpansion
echo ========================================================
echo Building RapidMD (fast profile) and Windows Installer
echo ========================================================

cd /d "%~dp0\.."

rem Must match the profile referenced in rapidmd.iss [Files]
set "PROFILE=fast"

echo [1/3] Building optimized binary (--profile %PROFILE%)...
cargo build --profile %PROFILE%
if %errorlevel% neq 0 (
    echo Error: Cargo build failed!
    exit /b %errorlevel%
)

if not exist "target\%PROFILE%\rapidmd.exe" (
    echo Error: target\%PROFILE%\rapidmd.exe was not produced!
    exit /b 1
)

echo [2/3] Checking Inno Setup compiler...
set "ISCC="
if exist "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" (
    set "ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe"
) else if exist "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" (
    set "ISCC=C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
) else if exist "C:\Program Files\Inno Setup 6\ISCC.exe" (
    set "ISCC=C:\Program Files\Inno Setup 6\ISCC.exe"
)

if "!ISCC!"=="" (
    echo Error: Inno Setup 6 compiler not found.
    echo        Install it from https://jrsoftware.org/isdl.php
    exit /b 1
)

echo [3/3] Compiling installer executable...
"!ISCC!" installer\rapidmd.iss
if %errorlevel% neq 0 (
    echo Error: Inno Setup compilation failed!
    exit /b %errorlevel%
)

rem Read the version from the .iss so this message can never drift from it.
rem Line is: #define MyAppVersion "0.3.0"  -> space-delimited token 3, quotes stripped.
for /f "tokens=3" %%v in ('findstr /b /c:"#define MyAppVersion" installer\rapidmd.iss') do (
    set "APPVER=%%~v"
)
if "!APPVER!"=="" (
    echo Error: could not read MyAppVersion from installer\rapidmd.iss
    exit /b 1
)

echo.
echo ========================================================
echo Installer successfully created!
echo Version:  %APPVER%
echo Location: installer\dist\RapidMD-Setup-v%APPVER%.exe
echo Packed:   target\%PROFILE%\rapidmd.exe
echo ========================================================
pause
