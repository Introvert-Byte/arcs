#!/usr/bin/env bash
# Build if needed and launch the GUI
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
BIN="$ROOT/target/release/arcs"
if [ ! -x "$BIN" ]; then
  echo "==> No release binary — running setup…"
  bash "$ROOT/scripts/setup.sh"
fi
exec "$BIN" "$@"
