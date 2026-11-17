#!/usr/bin/env bash
# Convenience wrapper — run from project root on Linux or macOS
exec bash "$(cd "$(dirname "$0")" && pwd)/scripts/setup.sh" "$@"
