#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

./check-imports.sh

cp dist/sp2.desktop.js ~/projects/sp/sp_axum/root/
cp dist/sp2.mobile.js  ~/projects/sp/sp_axum/root/
