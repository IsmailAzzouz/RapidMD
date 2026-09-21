@echo off
echo ========================================================
echo Building RapidMD Release and Windows Installer
echo ========================================================

cd /d "%~dp0\.."

echo [1/3] Building optimized release binary...
cargo build --release
if %errorlevel% neq 0 (
    echo Error: Cargo build failed!
    exit /b %errorlevel%
)

echo [2/3] Checking Inno Setup compiler...
set ISCC="C:\Users\%USERNAME%\AppData\Local\Programs\Inno Setup 6\ISCC.exe"
if not exist %ISCC% (
    set ISCC="C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
)
if not exist %ISCC% (
    set ISCC="C:\Program Files\Inno Setup 6\ISCC.exe"
)

echo [3/3] Compiling installer executable...
%ISCC% installer\rapidmd.iss
if %errorlevel% neq 0 (
    echo Error: Inno Setup compilation failed!
    exit /b %errorlevel%
)

echo.
echo ========================================================
echo Installer successfully created!
echo Location: installer\dist\RapidMD-Setup-v0.2.0.exe
echo ========================================================
pause
