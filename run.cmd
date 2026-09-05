@echo off
rem Blockhaven one-command build and run. Requires a Rust toolchain (https://rustup.rs).
rem Falls back to the per-user rustup install and a portable MinGW if they are not on PATH yet.
where cargo >nul 2>nul
if errorlevel 1 (
  if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)
where cargo >nul 2>nul
if errorlevel 1 (
  echo cargo was not found. Install Rust from https://rustup.rs and run this script again.
  pause
  exit /b 1
)
where gcc >nul 2>nul
if errorlevel 1 (
  if exist "%USERPROFILE%\winlibs\mingw64\bin\gcc.exe" set "PATH=%USERPROFILE%\winlibs\mingw64\bin;%PATH%"
)
cargo run --release -- %*
