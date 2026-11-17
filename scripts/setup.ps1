# Arcs — setup, deps, and release build (Windows)
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\setup.ps1
#         powershell -ExecutionPolicy Bypass -File scripts\setup.ps1 -Run

param(
    [switch]$Run
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

Write-Host "==> Arcs setup (Windows)"
Write-Host "    project: $Root"
Write-Host ""

# ---- Rust ----
if (-not (Get-Command rustc -ErrorAction SilentlyContinue) -or -not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "==> Installing Rust via rustup…"
    $rustup = "$env:TEMP\rustup-init.exe"
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustup
    & $rustup -y --default-toolchain stable
    $env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
} else {
    Write-Host "==> Rust found: $(rustc --version)"
}

# Ensure cargo on PATH
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) {
    $env:Path = "$cargoBin;" + $env:Path
}

# Visual Studio Build Tools hint
Write-Host "==> Note: MSVC Build Tools are required (C++ workload)."
Write-Host "    If build fails, install: https://aka.ms/vs/17/release/vs_BuildTools.exe"
Write-Host "    Select 'Desktop development with C++'."
Write-Host ""

# ---- Build ----
Write-Host "==> Building release…"
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$Bin = Join-Path $Root "target\release\arcs.exe"
if (-not (Test-Path $Bin)) { throw "Binary not found: $Bin" }

Write-Host ""
Write-Host "==> Done."
Write-Host "    Binary: $Bin"
Write-Host "    Run:    $Bin"
Write-Host ""

if ($Run) {
    Write-Host "==> Launching GUI…"
    & $Bin
}
