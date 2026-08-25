#!/usr/bin/env bash
set -euo pipefail

workspace_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Mirabile full verification requires '$1' on PATH" >&2
    exit 1
  fi
}

run() {
  echo
  echo "==> $*"
  "$@"
}

for command in cargo rustc git trunk python3 rg cmp; do
  require_command "${command}"
done

if ! command -v chromium >/dev/null 2>&1 && ! command -v chromium-browser >/dev/null 2>&1; then
  echo "Mirabile full verification requires Chromium ('chromium' or 'chromium-browser') on PATH" >&2
  exit 1
fi

wasm_target_libdir="$(rustc --print target-libdir --target wasm32-unknown-unknown)"
if [[ ! -d "${wasm_target_libdir}" ]]; then
  echo "Mirabile full verification requires the wasm32-unknown-unknown Rust target" >&2
  echo "Install it with: rustup target add wasm32-unknown-unknown" >&2
  exit 1
fi

cd "${workspace_dir}"

run cargo fmt --all -- --check

run cargo test -p mirabile-core
run cargo test -p mirabile-engine
run cargo test -p mirabile-store
run cargo test -p mirabile-app
run cargo test -p mirabile-web
run cargo test --workspace
run cargo test -p mirabile-engine --features xalen-backend

run cargo clippy --workspace --all-targets -- -D warnings

run cargo check -p mirabile-engine --no-default-features
run cargo check -p mirabile-engine --features xalen-backend
run cargo check -p mirabile-engine --features xalen-backend --target wasm32-unknown-unknown
run cargo check -p mirabile-web --target wasm32-unknown-unknown

echo
echo "==> Trunk main application and calculation Worker build"
(
  cd "${workspace_dir}/apps/web"
  env -u NO_COLOR trunk build
)

run "${workspace_dir}/scripts/check-xalen-dependencies.sh"
run "${workspace_dir}/scripts/test-browser.sh"
run python3 -m unittest scripts/test_cdp_client.py
run "${workspace_dir}/scripts/test-workbench-e2e.sh" --scenario smoke --mode semantic
run "${workspace_dir}/scripts/test-workbench-e2e.sh" --scenario smoke --mode control
run "${workspace_dir}/scripts/test-workbench-e2e.sh" --scenario new-chart --mode semantic
run "${workspace_dir}/scripts/test-workbench-e2e.sh" --scenario new-chart-control --mode control
run "${workspace_dir}/scripts/test-workbench-e2e.sh" --scenario saved-chart-control --mode control
run "${workspace_dir}/scripts/test-workbench-e2e.sh" --scenario artifact-smoke --mode semantic

run git diff --check
run git diff --cached --check

echo
echo "Mirabile full local verification passed"
