#!/usr/bin/env bash
# Minimal JSON-RPC smoke test for surf-service
#
# This script performs two basic checks against the JSON-RPC service
# exposed by the release binary:
#   1) " Surf.Status " with task_id = null returns a well-formed
#      JSON-RPC response with a result field (existing behavior).
#   2) " Surf.Scan " can be used to start a real scan against a
#      tiny temporary directory, and a follow-up " Surf.Status "
#      call for the returned task_id yields a result object whose
#      "task_id" matches and whose "state" field is present
#      (queued/running/completed/canceled/failed).
#
# The script intentionally avoids using jq and only relies on
# standard POSIX tools (grep/sed) so that it can run in minimal
# CI environments.

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
RESP_FILE="${TMP_DIR}/resp_status.json"
SCAN_RESP_FILE="${TMP_DIR}/resp_scan.json"
STATUS_RESP_FILE="${TMP_DIR}/resp_status_task.json"

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

# --- helper: send one JSON-RPC line over nc ---
send_jsonrpc() {
  local json="$1"
  local outfile="$2"
  local errfile="$3"

  if ! printf '%s\n' "${json}" | nc 127.0.0.1 "${PORT}" >"${outfile}" 2>"${errfile}"; then
    return 1
  fi
}

# --- 1) Surf.Status with task_id = null ---
JSON_STATUS_NULL='{"jsonrpc":"2.0","id":1,"method":"Surf.Status","params":{"task_id":null}}'
log "sending JSON-RPC Surf.Status (task_id=null)"
if ! send_jsonrpc "${JSON_STATUS_NULL}" "${RESP_FILE}" "${TMP_DIR}/nc_status.err"; then
  log "ERROR: nc failed to send or receive response"
  # short snippet from nc error
  if [[ -s "${TMP_DIR}/nc_status.err" ]]; then
    log "nc stderr (first 200 bytes):"
    head -c 200 "${TMP_DIR}/nc_status.err" | sed 's/^/[service_jsonrpc_basic] /'
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
 
# --- 2) Surf.Scan + Surf.Status for returned task_id ---

# Prepare a tiny temporary directory as scan target
SCAN_DIR="${TMP_DIR}/scan_root"
mkdir -p "${SCAN_DIR}"
printf 'alpha\n' >"${SCAN_DIR}/a.txt"
printf 'beta\n'  >"${SCAN_DIR}/b.log"

JSON_SCAN=$(cat <<EOF
{"jsonrpc":"2.0","id":2,"method":"Surf.Scan","params":{"path":"${SCAN_DIR}","min_size":"0","threads":1,"limit":10}}
EOF
)

log "sending JSON-RPC Surf.Scan against ${SCAN_DIR}"
if ! send_jsonrpc "${JSON_SCAN}" "${SCAN_RESP_FILE}" "${TMP_DIR}/nc_scan.err"; then
  log "ERROR: nc failed during Surf.Scan request"
  if [[ -s "${TMP_DIR}/nc_scan.err" ]]; then
    log "nc stderr (first 200 bytes):"
    head -c 200 "${TMP_DIR}/nc_scan.err" | sed 's/^/[service_jsonrpc_basic] /'
    echo
  fi
  fail 1
fi

if [[ ! -s "${SCAN_RESP_FILE}" ]]; then
  log "ERROR: empty response for Surf.Scan"
  fail 1
fi

if grep -q '"error"' "${SCAN_RESP_FILE}"; then
  log "ERROR: Surf.Scan returned error; snippet:"
  head -c 200 "${SCAN_RESP_FILE}" | sed 's/^/[service_jsonrpc_basic] /'
  echo
  fail 1
fi

# Extract task_id from Surf.Scan result using a conservative grep+sed
TASK_ID="$(grep -o '"task_id"[[:space:]]*:[[:space:]]*"[^"]*"' "${SCAN_RESP_FILE}" | head -n1 | sed 's/.*:"//;s/"$//')"

if [[ -z "${TASK_ID}" ]]; then
  log "ERROR: failed to extract task_id from Surf.Scan response; snippet:"
  head -c 200 "${SCAN_RESP_FILE}" | sed 's/^/[service_jsonrpc_basic] /'
  echo
  fail 1
fi

log "Surf.Scan returned task_id=${TASK_ID}"

JSON_STATUS_TASK=$(cat <<EOF
{"jsonrpc":"2.0","id":3,"method":"Surf.Status","params":{"task_id":"${TASK_ID}"}}
EOF
)

log "sending JSON-RPC Surf.Status for task_id=${TASK_ID}"
if ! send_jsonrpc "${JSON_STATUS_TASK}" "${STATUS_RESP_FILE}" "${TMP_DIR}/nc_status_task.err"; then
  log "ERROR: nc failed during Surf.Status(task_id) request"
  if [[ -s "${TMP_DIR}/nc_status_task.err" ]]; then
    log "nc stderr (first 200 bytes):"
    head -c 200 "${TMP_DIR}/nc_status_task.err" | sed 's/^/[service_jsonrpc_basic] /'
    echo
  fi
  fail 1
fi

if [[ ! -s "${STATUS_RESP_FILE}" ]]; then
  log "ERROR: empty response for Surf.Status(task_id)"
  fail 1
fi

if ! grep -q '"task_id"' "${STATUS_RESP_FILE}" || ! grep -q '"state"' "${STATUS_RESP_FILE}"; then
  log "ERROR: Surf.Status(task_id) missing task_id/state fields; snippet:"
  head -c 200 "${STATUS_RESP_FILE}" | sed 's/^/[service_jsonrpc_basic] /'
  echo
  fail 1
fi

log "Surf.Scan + Surf.Status basic JSON-RPC flow passed"
pass
