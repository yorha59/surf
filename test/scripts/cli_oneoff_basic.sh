#!/usr/bin/env bash

# Minimal smoke test for surf CLI one-off mode
# This script locates repo root relative to its own path,
# verifies the release binary exists, runs a safe command (--help),
# and reports PASS/FAIL based on exit status.

set -u

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BIN_PATH="$ROOT_DIR/release/linux-x86_64/cli/surf"

echo "[cli_oneoff_basic] repo_root=$ROOT_DIR"
echo "[cli_oneoff_basic] bin=$BIN_PATH"

if [ ! -x "$BIN_PATH" ]; then
  if [ -f "$BIN_PATH" ]; then
    echo "[cli_oneoff_basic] binary exists but is not executable; attempting to run anyway"
  else
    echo "[cli_oneoff_basic] ERROR: binary not found at $BIN_PATH"
    echo "FAIL"
    echo "EXIT_CODE:2"
    exit 2
  fi
fi

"$BIN_PATH" --help >/dev/null 2>&1
rc=$?

if [ $rc -eq 0 ]; then
  echo "PASS"
  echo "EXIT_CODE:0"
  exit 0
else
  echo "[cli_oneoff_basic] surf --help exited with code $rc"
  echo "FAIL"
  echo "EXIT_CODE:$rc"
  exit $rc
fi
