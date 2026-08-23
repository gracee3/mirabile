#!/usr/bin/env bash
set -euo pipefail

workspace_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dist_dir="${workspace_dir}/target/browser-contract"
dom_file="${workspace_dir}/target/browser-contract-dom.html"
server_log="${workspace_dir}/target/browser-contract-server.log"
profile_dir="$(mktemp -d -t mirabile-browser-contract.XXXXXX)"
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
  env -u NO_COLOR trunk build --features browser-contract --dist "${dist_dir}"
)

for required_asset in \
  "${dist_dir}/THIRD_PARTY_NOTICES.md" \
  "${dist_dir}/third_party_licenses/Apache-2.0.txt" \
  "${dist_dir}/third_party_licenses/vsop87-MIT.txt"
do
  if [[ ! -f "${required_asset}" ]]; then
    echo "Browser distribution is missing third-party notice asset: ${required_asset}" >&2
    exit 1
  fi
done
cmp "${workspace_dir}/THIRD_PARTY_NOTICES.md" "${dist_dir}/THIRD_PARTY_NOTICES.md"
cmp \
  "${workspace_dir}/third_party_licenses/Apache-2.0.txt" \
  "${dist_dir}/third_party_licenses/Apache-2.0.txt"
cmp \
  "${workspace_dir}/third_party_licenses/vsop87-MIT.txt" \
  "${dist_dir}/third_party_licenses/vsop87-MIT.txt"

python3 -m http.server 18080 --bind 127.0.0.1 --directory "${dist_dir}" >"${server_log}" 2>&1 &
server_pid=$!

server_ready=false
for _ in $(seq 1 100); do
  if python3 -c 'import urllib.request; urllib.request.urlopen("http://127.0.0.1:18080/", timeout=0.2)' >/dev/null 2>&1; then
    server_ready=true
    break
  fi
  sleep 0.1
done

if [[ "${server_ready}" != true ]]; then
  echo "Local browser-contract server did not become ready" >&2
  exit 1
fi

chromium_bin="$(command -v chromium || command -v chromium-browser || true)"
if [[ -z "${chromium_bin}" ]]; then
  echo "Chromium is required for the browser contract" >&2
  exit 1
fi

debug_port="$(python3 -c 'import socket; value = socket.socket(); value.bind(("127.0.0.1", 0)); print(value.getsockname()[1]); value.close()')"

"${chromium_bin}" \
  --headless=new \
  --no-sandbox \
  --disable-gpu \
  --disable-dev-shm-usage \
  --user-data-dir="${profile_dir}" \
  --remote-debugging-port="${debug_port}" \
  --remote-allow-origins='*' \
  http://127.0.0.1:18080/ >"${workspace_dir}/target/browser-contract-chromium.log" 2>&1 &
chromium_pid=$!

if ! python3 "${workspace_dir}/scripts/wait-browser-contract.py" "${debug_port}" >"${dom_file}"; then
  echo "Mirabile browser runtime contract did not pass; DOM follows" >&2
  sed -n '1,160p' "${dom_file}" >&2
  exit 1
fi

if ! grep -q 'id="browser-contract-result" data-status="passed"' "${dom_file}" || \
   ! grep -q 'MIRABILE_BROWSER_CONTRACT:PASS' "${dom_file}"; then
  echo "Mirabile browser runtime contract failed; DOM follows" >&2
  sed -n '1,160p' "${dom_file}" >&2
  exit 1
fi

echo "IndexedDB reload and Web Worker calculation browser contract passed"
