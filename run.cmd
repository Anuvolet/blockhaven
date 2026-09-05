@echo off
rem Blockhaven one-command build and run. Requires a Rust toolchain (https://rustup.rs).
where cargo >nul 2>nul
if errorlevel 1 (
  echo cargo was not found. Install Rust from https://rustup.rs and run this script again.
  pause
  exit /b 1
)
cargo run --release -- %*
