#!/usr/bin/env bash
# Cross-fork import guard for the mobile/desktop split.
# Rules:
#   - core/    must NOT import from desktop/ or mobile/
#   - desktop/ must NOT import from mobile/
#   - mobile/  must NOT import from desktop/
set -euo pipefail

cd "$(dirname "$0")"
SRC=src/sp

fail=0

check() {
  # $1 = root tree, $2 = forbidden segment
  if grep -rEn "from ['\"][^'\"]*\\b${2}/" "$1" 2>/dev/null; then
    echo "ERROR: ${1#${SRC}/} imports from forbidden tree '${2}/'"
    fail=1
  fi
}

check "$SRC/core"    desktop
check "$SRC/core"    mobile
check "$SRC/desktop" mobile
check "$SRC/mobile"  desktop

if [ "$fail" -ne 0 ]; then
  echo "Cross-fork import guard FAILED"
  exit 1
fi

echo "Cross-fork import guard OK"
