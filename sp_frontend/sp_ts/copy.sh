#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

./check-imports.sh

repo_root="$(cd ../.. && pwd)"
axum_root="$repo_root/sp_axum/root"
frontend_static="$repo_root/sp_frontend/priv/static"
server_tileset="$repo_root/sp_server/tileset"

cp dist/sp2.desktop.js "$axum_root/"
cp dist/sp2.mobile.js "$axum_root/"

mkdir -p "$axum_root/static/art"
mkdir -p "$server_tileset"

for art_file in \
  supplycache.png \
  supplycache.json \
  washedashore.png \
  washedashore.json
do
  cp "$frontend_static/art/$art_file" "$axum_root/static/art/"

  if [[ "$art_file" == *.json ]]; then
    cp "$frontend_static/art/$art_file" "$server_tileset/"
  fi
done
