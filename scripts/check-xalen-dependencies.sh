#!/usr/bin/env bash
set -euo pipefail

workspace_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
audit_dir="$(mktemp -d -t mirabile-xalen-audit.XXXXXX)"
trap 'rm -rf "${audit_dir}"' EXIT

cargo tree \
  --manifest-path "${workspace_dir}/Cargo.toml" \
  -p mirabile-engine \
  --features xalen-backend \
  -e features >"${audit_dir}/engine.txt"
cargo tree \
  --manifest-path "${workspace_dir}/Cargo.toml" \
  -p mirabile-web \
  -e features >"${audit_dir}/web.txt"

for forbidden in \
  xalen-stars-hip-data \
  hip-catalog \
  kernel-autodownload \
  xalen-cloud \
  xalen-western \
  xalen-vedic \
  'xalen-ephemeris v'
do
  if rg -n --fixed-strings "${forbidden}" "${audit_dir}/engine.txt" "${audit_dir}/web.txt"; then
    echo "Forbidden XALEN package or feature is active: ${forbidden}" >&2
    exit 1
  fi
done

for required in xalen-ephem xalen-time xalen-coords xalen-houses; do
  if ! rg -q --fixed-strings "${required} v0.6.0" "${audit_dir}/engine.txt"; then
    echo "Required pinned XALEN leaf package is missing: ${required}" >&2
    exit 1
  fi
done

echo "XALEN dependency guard passed: required leaves present; NC, network, umbrella, cloud, Western, and Vedic layers absent"
