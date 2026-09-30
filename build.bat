@echo off
cd /d "%~dp0"
cargo build --release -p app
copy /y target\release\app.exe ExpOverlay.exe
