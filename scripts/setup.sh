#!/usr/bin/env bash
# Arcs — setup, deps, and release build (Linux / macOS)
# Usage:  bash scripts/setup.sh
#         bash scripts/setup.sh --run

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RUN_AFTER=0
for arg in "$@"; do
  case "$arg" in
    --run|-r) RUN_AFTER=1 ;;
    --help|-h)
      echo "Usage: $0 [--run]"
      echo "  Installs Rust (if needed), OS libraries, builds release binary."
      exit 0
      ;;
  esac
done

echo "==> Arcs setup"
echo "    project: $ROOT"
echo

# ---- OS detect ----
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
echo "==> Detected: $OS / $ARCH"

# ---- Rust toolchain ----
if ! command -v rustc >/dev/null 2>&1 || ! command -v cargo >/dev/null 2>&1; then
  echo "==> Installing Rust via rustup…"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
else
  echo "==> Rust found: $(rustc --version)"
fi

# Ensure cargo in PATH for this session
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

# ---- System dependencies ----
install_linux_deps() {
  if command -v apt-get >/dev/null 2>&1; then
    echo "==> Installing Debian/Ubuntu packages…"
    sudo apt-get update -y
    sudo apt-get install -y \
      build-essential pkg-config curl \
      libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev \
      libxcb-xfixes0-dev libxkbcommon-dev libssl-dev \
      libclang-dev cmake
  elif command -v dnf >/dev/null 2>&1; then
    echo "==> Installing Fedora packages…"
    sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl \
      gtk3-devel libxcb-devel libxkbcommon-devel openssl-devel \
      clang cmake
  elif command -v pacman >/dev/null 2>&1; then
    echo "==> Installing Arch packages…"
    sudo pacman -Sy --needed --noconfirm base-devel gtk3 libxcb libxkbcommon openssl clang cmake curl
  elif command -v zypper >/dev/null 2>&1; then
    echo "==> Installing openSUSE packages…"
    sudo zypper install -y gcc gcc-c++ make pkg-config curl \
      gtk3-devel libxcb-devel libxkbcommon-devel libopenssl-devel clang cmake
  else
    echo "!! Unknown Linux package manager. Install GTK3, libxcb, libxkbcommon, OpenSSL, and a C toolchain manually."
  fi
}

install_macos_deps() {
  if ! command -v brew >/dev/null 2>&1; then
    echo "==> Installing Homebrew…"
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    if [ -f /opt/homebrew/bin/brew ]; then
      eval "$(/opt/homebrew/bin/brew shellenv)"
    fi
  fi
  echo "==> Installing macOS packages (openssl, pkg-config)…"
  brew install openssl@3 pkg-config || true
}

case "$OS" in
  linux*)  install_linux_deps ;;
  darwin*) install_macos_deps ;;
  *)
    echo "!! Unsupported OS for this script: $OS"
    echo "   On Windows use:  powershell -File scripts/setup.ps1"
    exit 1
    ;;
esac

# ---- Build ----
echo "==> Building release (this may take a few minutes)…"
cargo build --release

BIN="$ROOT/target/release/arcs"
if [ ! -x "$BIN" ]; then
  echo "!! Build finished but binary not found at $BIN"
  exit 1
fi

echo
echo "==> Done."
echo "    Binary: $BIN"
echo "    Run:    $BIN"
echo "            $BIN --tui"
echo "            $BIN discover"
echo

if [ "$RUN_AFTER" -eq 1 ]; then
  echo "==> Launching GUI…"
  exec "$BIN"
fi
