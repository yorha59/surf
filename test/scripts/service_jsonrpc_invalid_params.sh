#!/usr/bin/env bash
# JSON-RPC error path smoke test for surf-service
#
# Goal: verify that sending an invalid parameter (min_size with illegal unit)
# to Surf.Scan yields a JSON-RPC error with code -32602 (INVALID_PARAMS),
# in line with Architecture.md 4.3.2/4.3.3.
#
# The script uses only bash + POSIX tools (grep/sed) and nc.

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
  echo "[service_jsonrpc_invalid_params] $*"
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
TMP_DIR="$(mktemp -d -t service_jsonrpc_invalid.XXXXXX)"
OUT_SERVER_STDOUT="${TMP_DIR}/server.out"
OUT_SERVER_STDERR="${TMP_DIR}/server.err"
RESP_FILE="${TMP_DIR}/resp_invalid_params.json"

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
      log "ERROR: service exited before becoming ready"
      fail 1
    else
      SERVER_EXIT=$?
      log "ERROR: service failed to start (exit=${SERVER_EXIT})"
      # echo a short snippet from stderr for debugging
      if [[ -s "${OUT_SERVER_STDERR}" ]]; then
        log "server stderr (first 200 bytes):"
        head -c 200 "${OUT_SERVER_STDERR}" | sed 's/^/[service_jsonrpc_invalid_params] /'
        echo
      fi
      fail "${SERVER_EXIT}"
    fi
  fi
  log "ERROR: unable to connect to port ${PORT} after retries"
  fail 1
fi

# --- helper: send one JSON-RPC line over nc ---
send_jsonrpc() {
  local json="$1"
  local outfile="$2"
  local errfile="$3"

  if ! printf '%s\n' "${json}" | nc 127.0.0.1 "${PORT}" >"${outfile}" 2>"${errfile}"; then
    return 1
  fi
}

# --- invalid params request ---
JSON_INVALID=$(cat <<EOF
{"jsonrpc":"2.0","id":10,"method":"Surf.Scan","params":{"path":"/","min_size":"10XYZ","threads":1,"limit":10}}
EOF
)

log "sending JSON-RPC Surf.Scan with invalid min_size (10XYZ)"
if ! send_jsonrpc "${JSON_INVALID}" "${RESP_FILE}" "${TMP_DIR}/nc_invalid.err"; then
  log "ERROR: nc failed during Surf.Scan (invalid params) request"
  if [[ -s "${TMP_DIR}/nc_invalid.err" ]]; then
    log "nc stderr (first 200 bytes):"
    head -c 200 "${TMP_DIR}/nc_invalid.err" | sed 's/^/[service_jsonrpc_invalid_params] /'
    echo
  fi
  fail 1
fi

if [[ ! -s "${RESP_FILE}" ]]; then
  log "ERROR: empty response for invalid params request"
  # provide short snippets from service logs to aid debugging
  if [[ -s "${OUT_SERVER_STDERR}" ]]; then
    log "server stderr (first 200 bytes):"
    head -c 200 "${OUT_SERVER_STDERR}" | sed 's/^/[service_jsonrpc_invalid_params] /'
    echo
  fi
  if [[ -s "${OUT_SERVER_STDOUT}" ]]; then
    log "server stdout (first 200 bytes):"
    head -c 200 "${OUT_SERVER_STDOUT}" | sed 's/^/[service_jsonrpc_invalid_params] /'
    echo
  fi
  fail 1
fi

# --- minimal structural checks ---
if ! grep -q '"jsonrpc"' "${RESP_FILE}"; then
  log "ERROR: response missing jsonrpc field; snippet:"
  head -c 200 "${RESP_FILE}" | sed 's/^/[service_jsonrpc_invalid_params] /'
  echo
  fail 1
fi

if ! grep -q '"id"[[:space:]]*:[[:space:]]*10' "${RESP_FILE}"; then
  log "ERROR: response missing id=10 field; snippet:"
  head -c 200 "${RESP_FILE}" | sed 's/^/[service_jsonrpc_invalid_params] /'
  echo
  fail 1
fi

if ! grep -q '"error"' "${RESP_FILE}"; then
  log "ERROR: response missing error object; snippet:"
  head -c 200 "${RESP_FILE}" | sed 's/^/[service_jsonrpc_invalid_params] /'
  echo
  fail 1
fi

if ! grep -q '"code"[[:space:]]*:[[:space:]]*-32602' "${RESP_FILE}"; then
  log "ERROR: error.code is not -32602; snippet:"
  head -c 200 "${RESP_FILE}" | sed 's/^/[service_jsonrpc_invalid_params] /'
  echo
  fail 1
fi

if ! grep -qi 'INVALID_PARAMS' "${RESP_FILE}"; then
  log "ERROR: error.message does not contain INVALID_PARAMS; snippet:"
  head -c 200 "${RESP_FILE}" | sed 's/^/[service_jsonrpc_invalid_params] /'
  echo
  fail 1
fi

log "INVALID_PARAMS error path validated"
pass
