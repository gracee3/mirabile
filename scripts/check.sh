#!/usr/bin/env bash
set -euo pipefail

workspace_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Mirabile fast checks require '$1' on PATH" >&2
    exit 1
  fi
}

run() {
  echo
  echo "==> $*"
  "$@"
}

require_command cargo
require_command git
require_command python3

cd "${workspace_dir}"
run cargo fmt --all -- --check
run cargo test --workspace
run cargo clippy --workspace --all-targets -- -D warnings
run python3 -m unittest scripts/test_cdp_client.py
run python3 -m unittest discover -s scripts -p 'test_workflow_assertions.py'
run git diff --check
run git diff --cached --check

echo
echo "Mirabile fast checks passed"
