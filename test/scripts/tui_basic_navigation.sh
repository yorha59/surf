#!/usr/bin/env bash

# Minimal smoke test for surf CLI TUI mode
# - Verifies the release binary exists (or at least is present)
# - Checks that "surf --help" exposes the --tui flag
# - Ensures conflicting args "--tui --json" fail with non-zero exit code
#   and emit a clear error message on stderr.

set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BIN_PATH="$ROOT_DIR/release/linux-x86_64/cli/surf"

echo "[tui_basic_navigation] repo_root=$ROOT_DIR"
echo "[tui_basic_navigation] bin=$BIN_PATH"

if [ ! -x "$BIN_PATH" ]; then
  if [ -f "$BIN_PATH" ]; then
    echo "[tui_basic_navigation] binary exists but is not executable; attempting to run anyway"
  else
    echo "[tui_basic_navigation] ERROR: binary not found at $BIN_PATH"
    echo "FAIL"
    echo "EXIT_CODE:2"
    exit 2
  fi
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# 1) Help output check: must succeed and include --tui
OUT_HELP="$TMP_DIR/help.out"
OUT_ERR_HELP="$TMP_DIR/help.err"
"$BIN_PATH" --help >"$OUT_HELP" 2>"$OUT_ERR_HELP"
rc_help=$?

if [ $rc_help -ne 0 ]; then
  echo "[tui_basic_navigation] ERROR: 'surf --help' failed with rc=$rc_help"
  if [ -s "$OUT_ERR_HELP" ]; then
    echo "[tui_basic_navigation] stderr sample (first 200 bytes):"
    head -c 200 "$OUT_ERR_HELP" | sed -e 's/\n/ /g'
    echo
  fi
  echo "FAIL"
  echo "EXIT_CODE:$rc_help"
  exit $rc_help
fi

if ! grep -q -- "--tui" "$OUT_HELP"; then
  echo "[tui_basic_navigation] ERROR: --tui flag not found in surf --help output"
  echo "[tui_basic_navigation] help sample (first 200 bytes):"
  head -c 200 "$OUT_HELP" | sed -e 's/\n/ /g'
  echo
  echo "FAIL"
  echo "EXIT_CODE:1"
  exit 1
fi

# 2) Argument conflict check: --tui with --json must fail with non-zero exit
printf "alpha\n" > "$TMP_DIR/a.txt"
printf "beta\n"  > "$TMP_DIR/b.log"

OUT_STD="$TMP_DIR/out.std"
OUT_ERR="$TMP_DIR/out.err"

"$BIN_PATH" --tui --json --path "$TMP_DIR" >"$OUT_STD" 2>"$OUT_ERR"
rc_conflict=$?

if [ $rc_conflict -eq 0 ]; then
  echo "[tui_basic_navigation] ERROR: expected non-zero exit when using --tui with --json; got rc=0"
  if [ -s "$OUT_STD" ]; then
    echo "[tui_basic_navigation] stdout sample (first 200 bytes):"
    head -c 200 "$OUT_STD" | sed -e 's/\n/ /g'
    echo
  fi
  if [ -s "$OUT_ERR" ]; then
    echo "[tui_basic_navigation] stderr sample (first 200 bytes):"
    head -c 200 "$OUT_ERR" | sed -e 's/\n/ /g'
    echo
  fi
  echo "FAIL"
  echo "EXIT_CODE:$rc_conflict"
  exit 1
fi

if [ ! -s "$OUT_ERR" ]; then
  echo "[tui_basic_navigation] ERROR: stderr is empty for conflicting args (--tui --json)"
  if [ -s "$OUT_STD" ]; then
    echo "[tui_basic_navigation] stdout sample (first 200 bytes):"
    head -c 200 "$OUT_STD" | sed -e 's/\n/ /g'
    echo
  fi
  echo "FAIL"
  echo "EXIT_CODE:$rc_conflict"
  exit 1
fi

# Expect a readable error indicating the conflict
if ! grep -q -- "--json cannot be used together with --tui" "$OUT_ERR" && \
   ! grep -q -- "--tui cannot be used together with --json" "$OUT_ERR" && \
   ! grep -q -- "cannot be used together with" "$OUT_ERR"; then
  echo "[tui_basic_navigation] ERROR: missing expected conflict message in stderr"
  echo "[tui_basic_navigation] stderr sample (first 200 bytes):"
  head -c 200 "$OUT_ERR" | sed -e 's/\n/ /g'
  echo
  echo "FAIL"
  echo "EXIT_CODE:$rc_conflict"
  exit 1
fi

echo "PASS"
echo "EXIT_CODE:0"
exit 0
