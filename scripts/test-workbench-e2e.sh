#!/usr/bin/env bash
set -euo pipefail

workspace_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
scenario="smoke"
mode="semantic"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --scenario)
      scenario="$2"
      shift 2
      ;;
    --mode)
      mode="$2"
      shift 2
      ;;
    *)
      echo "Unknown workbench E2E argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ "${mode}" != "semantic" && "${mode}" != "control" && "${mode}" != "all" ]]; then
  echo "Workbench E2E mode must be semantic, control, or all" >&2
  exit 2
fi

scenario_file="${workspace_dir}/scripts/workbench-scenarios/${scenario}.json"
if [[ ! -f "${scenario_file}" ]]; then
  echo "Unknown workbench E2E scenario: ${scenario}" >&2
  exit 2
fi

dist_dir="${workspace_dir}/target/workbench-e2e-dist"
artifact_dir="${workspace_dir}/target/workbench-e2e-artifacts/${scenario}"
profile_dir="$(mktemp -d -t mirabile-workbench-e2e.XXXXXX)"
server_log="${workspace_dir}/target/workbench-e2e-server.log"
chromium_log="${workspace_dir}/target/workbench-e2e-chromium.log"
server_pid=""
chromium_pid=""

cleanup() {
  if [[ -n "${server_pid}" ]]; then
    kill "${server_pid}" 2>/dev/null || true
    wait "${server_pid}" 2>/dev/null || true
  fi
  if [[ -n "${chromium_pid}" ]]; then
    kill "${chromium_pid}" 2>/dev/null || true
    wait "${chromium_pid}" 2>/dev/null || true
  fi
  rm -rf "${profile_dir}"
}
trap cleanup EXIT

(
  cd "${workspace_dir}/apps/web"
  env -u NO_COLOR trunk build --features workbench-automation --dist "${dist_dir}"
)

read -r server_port debug_port < <(python3 - <<'PY'
import socket
ports = []
for _ in range(2):
    value = socket.socket()
    value.bind(("127.0.0.1", 0))
    ports.append(value.getsockname()[1])
    value.close()
print(*ports)
PY
)

python3 -m http.server "${server_port}" --bind 127.0.0.1 --directory "${dist_dir}" >"${server_log}" 2>&1 &
server_pid=$!

server_ready=false
for _ in $(seq 1 100); do
  if python3 -c 'import sys, urllib.request; urllib.request.urlopen(sys.argv[1], timeout=0.2)' "http://127.0.0.1:${server_port}/" >/dev/null 2>&1; then
    server_ready=true
    break
  fi
  sleep 0.05
done
if [[ "${server_ready}" != true ]]; then
  echo "Local workbench E2E server did not become ready" >&2
  exit 1
fi

chromium_bin="$(command -v chromium || command -v chromium-browser || true)"
if [[ -z "${chromium_bin}" ]]; then
  echo "Chromium is required for workbench E2E" >&2
  exit 1
fi

database_name="mirabile-workbench-e2e-$(date +%s)-${debug_port}"
url="http://127.0.0.1:${server_port}/?mirabileAutomation=1&database=${database_name}"
"${chromium_bin}" \
  --headless=new \
  --no-sandbox \
  --disable-gpu \
  --disable-dev-shm-usage \
  --window-size=1600,1000 \
  --user-data-dir="${profile_dir}" \
  --remote-debugging-address=127.0.0.1 \
  --remote-debugging-port="${debug_port}" \
  --remote-allow-origins='*' \
  "${url}" >"${chromium_log}" 2>&1 &
chromium_pid=$!

rm -rf "${artifact_dir}"
if [[ "${scenario}" == "artifact-smoke" ]]; then
  if python3 "${workspace_dir}/scripts/workbench-control.py" \
      --port "${debug_port}" run "${scenario_file}" --artifacts "${artifact_dir}"; then
    echo "Expected artifact-smoke scenario to fail" >&2
    exit 1
  fi
  for required in screenshot.png dom.html controls.json application.json trace.json browser.log scenario.json error.txt; do
    if [[ ! -s "${artifact_dir}/${required}" ]]; then
      echo "Artifact smoke test did not preserve ${required}" >&2
      exit 1
    fi
  done
  echo "Workbench expected-failure artifact pipeline passed"
  exit 0
fi

python3 "${workspace_dir}/scripts/workbench-control.py" \
  --port "${debug_port}" run "${scenario_file}" --artifacts "${artifact_dir}"

if [[ "${mode}" == "control" || "${mode}" == "all" ]]; then
  python3 "${workspace_dir}/scripts/workbench-control.py" \
    --port "${debug_port}" screenshot "${workspace_dir}/target/workbench-e2e-${scenario}.png"
fi

echo "Workbench ${mode} E2E scenario passed: ${scenario}"
