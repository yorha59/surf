#!/usr/bin/env bash

# Minimal smoke test for surf CLI JSON mode
# - Creates a tiny temp directory with a few files
# - Runs: surf --path <tmp> --min-size 0 --limit 5 --json
# - Verifies: exit code == 0, stdout non-empty, and contains "root" & "entries"

set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BIN_PATH="$ROOT_DIR/release/linux-x86_64/cli/surf"

echo "[cli_json_mode] repo_root=$ROOT_DIR"
echo "[cli_json_mode] bin=$BIN_PATH"

if [ ! -x "$BIN_PATH" ]; then
  if [ -f "$BIN_PATH" ]; then
    echo "[cli_json_mode] binary exists but is not executable; attempting to run anyway"
  else
    echo "[cli_json_mode] ERROR: binary not found at $BIN_PATH"
    echo "FAIL"
    echo "EXIT_CODE:2"
    exit 2
  fi
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# Prepare tiny test data
printf "alpha\n" > "$TMP_DIR/a.txt"
printf "beta\n"  > "$TMP_DIR/b.log"

OUT_JSON="$TMP_DIR/out.json"
OUT_ERR="$TMP_DIR/stderr.log"

# Run JSON mode once
"$BIN_PATH" --path "$TMP_DIR" --min-size 0 --limit 5 --json >"$OUT_JSON" 2>"$OUT_ERR"
rc=$?

if [ $rc -ne 0 ]; then
  echo "[cli_json_mode] surf exited with code $rc"
  if [ -s "$OUT_ERR" ]; then
    echo "[cli_json_mode] stderr:"
    cat "$OUT_ERR"
  fi
  echo "FAIL"
  echo "EXIT_CODE:$rc"
  exit $rc
fi

# stdout must be non-empty
if [ ! -s "$OUT_JSON" ]; then
  echo "[cli_json_mode] ERROR: stdout is empty in JSON mode"
  if [ -s "$OUT_ERR" ]; then
    echo "[cli_json_mode] stderr:"
    cat "$OUT_ERR"
  fi
  echo "FAIL"
  echo "EXIT_CODE:$rc"
  exit 1
fi

# Must contain required JSON fields: root and entries
have_root=0
have_entries=0
if grep -q '"root"' "$OUT_JSON"; then
  have_root=1
fi
if grep -q '"entries"' "$OUT_JSON"; then
  have_entries=1
fi

if [ $have_root -ne 1 ] || [ $have_entries -ne 1 ]; then
  echo "[cli_json_mode] ERROR: missing required JSON fields (root/entries)"
  echo "[cli_json_mode] root_present=$have_root entries_present=$have_entries"
  echo "[cli_json_mode] sample stdout (first 200 bytes):"
  head -c 200 "$OUT_JSON" | sed -e 's/\n/ /g'
  echo
  echo "FAIL"
  echo "EXIT_CODE:$rc"
  exit 1
fi

echo "PASS"
echo "EXIT_CODE:0"
exit 0

