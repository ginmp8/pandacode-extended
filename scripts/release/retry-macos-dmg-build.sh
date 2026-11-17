#!/usr/bin/env bash
# Retry only the known transient hdiutil busy-detach failure during Tauri DMG bundling.
# On every other build failure, exit immediately rather than hiding a real regression.
set -euo pipefail

log="$(mktemp "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/pandacode-dmg-build.XXXXXX")"
trap 'rm -f "$log"' EXIT

for attempt in 1 2 3; do
  if node scripts/release/build.mjs 2>&1 | tee "$log"; then
    exit 0
  fi

  if ! grep -Fq 'error running bundle_dmg.sh' "$log" || \
     ! grep -Eq "hdiutil: couldn't eject .* - Resource busy" "$log"; then
    echo '::error::Build failed for a reason other than a busy macOS DMG; not retrying.' >&2
    exit 1
  fi

  if (( attempt == 3 )); then
    echo '::error::macOS DMG eject was busy on all three build attempts.' >&2
    exit 1
  fi

  echo "::warning::Transient macOS DMG eject failure; retrying the cached build (attempt $((attempt + 1))/3)." >&2
  sleep 15
done
