#!/usr/bin/env bash
set -euo pipefail

frontend_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
typescript_root="$frontend_root/sp_ts"
webpack_bin="$typescript_root/node_modules/.bin/webpack"

if [[ ! -x "$webpack_bin" ]]; then
  echo "Frontend dependencies are missing." >&2
  echo "Run 'cd $typescript_root && npm ci', then retry." >&2
  exit 1
fi

echo "Rebuilding the Siege Perilous UX..."
cd "$typescript_root"
npm run rebuild:ux

repo_root="$(cd "$frontend_root/.." && pwd)"
axum_root="$repo_root/sp_axum/root"

for bundle in sp2.desktop.js sp2.mobile.js; do
  if ! cmp -s "$typescript_root/dist/$bundle" "$axum_root/$bundle"; then
    echo "Bundle verification failed: $axum_root/$bundle" >&2
    exit 1
  fi
done

echo "UX rebuild complete."
echo "Desktop and mobile bundles are ready in $axum_root."
