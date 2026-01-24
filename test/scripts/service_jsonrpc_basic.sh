#!/usr/bin/env bash
# Minimal JSON-RPC smoke test for surf-service
set -u
set -o pipefail

# --- formatting helpers ---
pass() {
  echo "PASS"
  echo "EXIT_CODE:0"
  exit 0
}
fail() {
  local code=${1:-1}
  echo "FAIL"
  echo "EXIT_CODE:${code}"
  exit "${code}"
}

log() {
  echo "[service_jsonrpc_basic] $*"
}

# --- locate repo root and binary ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
SERVICE_BIN="${ROOT_DIR}/release/linux-x86_64/service/surf-service"

if [[ ! -f "${SERVICE_BIN}" ]]; then
  log "ERROR: binary not found at ${SERVICE_BIN}"
  fail 2
fi

if [[ ! -x "${SERVICE_BIN}" ]]; then
  log "binary exists but is not executable; attempting to run anyway"
fi

# --- prerequisites ---
if ! command -v nc >/dev/null 2>&1; then
  log "ERROR: 'nc' (netcat) is required for this smoke test but was not found"
  fail 1
fi

# --- temp workspace ---
TMP_DIR="$(mktemp -d -t service_jsonrpc_basic.XXXXXX)"
OUT_SERVER_STDOUT="${TMP_DIR}/server.out"
OUT_SERVER_STDERR="${TMP_DIR}/server.err"
RESP_FILE="${TMP_DIR}/resp.json"

cleanup() {
  # shellcheck disable=SC2086
  if [[ -n "${SERVER_PID:-}" ]]; then
    if kill -0 "${SERVER_PID}" >/dev/null 2>&1; then
      kill "${SERVER_PID}" >/dev/null 2>&1 || true
      # give it a moment to exit
      sleep 0.2
    fi
  fi
  rm -rf "${TMP_DIR}" 2>/dev/null || true
}
trap cleanup EXIT

# --- start service ---
PORT="21523"
log "starting service on 127.0.0.1:${PORT}"
"${SERVICE_BIN}" --host 127.0.0.1 --port "${PORT}" >"${OUT_SERVER_STDOUT}" 2>"${OUT_SERVER_STDERR}" &
SERVER_PID=$!

# wait for readiness: try connecting several times
READY=0
for i in {1..10}; do
  if nc -z 127.0.0.1 "${PORT}" >/dev/null 2>&1; then
    READY=1
    break
  fi
  sleep 0.3
done

if [[ "${READY}" -ne 1 ]]; then
  # see if the process already exited and capture exit code
  if ! kill -0 "${SERVER_PID}" >/dev/null 2>&1; then
    if wait "${SERVER_PID}" >/dev/null 2>&1; then
      # unlikely: exited with 0 but not ready
      log "ERROR: service exited before becoming ready"
      fail 1
    else
      SERVER_EXIT=$?
      log "ERROR: service failed to start (exit=${SERVER_EXIT})"
      # echo a short snippet from stderr for debugging
      if [[ -s "${OUT_SERVER_STDERR}" ]]; then
        log "server stderr (first 200 bytes):"
        head -c 200 "${OUT_SERVER_STDERR}" | sed 's/^/[service_jsonrpc_basic] /'
        echo
      fi
      fail "${SERVER_EXIT}"
    fi
  fi
  log "ERROR: unable to connect to port ${PORT} after retries"
  fail 1
fi

# --- send minimal JSON-RPC request ---
JSON='{"jsonrpc":"2.0","id":1,"method":"Surf.Status","params":{"task_id":null}}'
log "sending JSON-RPC Surf.Status"
if ! printf '%s\n' "${JSON}" | nc 127.0.0.1 "${PORT}" >"${RESP_FILE}" 2>"${TMP_DIR}/nc.err"; then
  log "ERROR: nc failed to send or receive response"
  # short snippet from nc error
  if [[ -s "${TMP_DIR}/nc.err" ]]; then
    log "nc stderr (first 200 bytes):"
    head -c 200 "${TMP_DIR}/nc.err" | sed 's/^/[service_jsonrpc_basic] /'
    echo
  fi
  fail 1
fi

# --- validate response ---
if [[ ! -s "${RESP_FILE}" ]]; then
  log "ERROR: empty response"
  fail 1
fi

if ! grep -q '"jsonrpc"' "${RESP_FILE}"; then
  log "ERROR: response missing jsonrpc field; snippet:" 
  head -c 200 "${RESP_FILE}" | sed 's/^/[service_jsonrpc_basic] /'
  echo
  fail 1
fi

if ! grep -q '"result"' "${RESP_FILE}"; then
  log "ERROR: response missing result field; snippet:"
  head -c 200 "${RESP_FILE}" | sed 's/^/[service_jsonrpc_basic] /'
  echo
  fail 1
fi

log "received valid JSON-RPC response"
pass

