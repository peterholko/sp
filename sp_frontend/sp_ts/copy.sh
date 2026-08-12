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
mkdir -p "$axum_root/static/art/ui/activity"
mkdir -p "$axum_root/static/art/portraits/heroes"
mkdir -p "$axum_root/static/art/portraits/villagers"
mkdir -p "$server_tileset"

cp "$frontend_static/art/ui/activity/"*.png "$axum_root/static/art/ui/activity/"
cp "$frontend_static/art/portraits/heroes/"*.png "$axum_root/static/art/portraits/heroes/"
cp "$frontend_static/art/portraits/villagers/"*.png "$axum_root/static/art/portraits/villagers/"

mkdir -p "$axum_root/static/art/items"
for item_art_file in \
  crudehatchet.png \
  fishingrod.png \
  copperfishingrod.png \
  ironfishingrod.png \
  mithrilfishingrod.png \
  sickle.png \
  coppersickle.png \
  ironsickle.png \
  mithrilsickle.png \
  copperfellingaxe.png \
  copperpickaxe.png \
  copperstonecutterhammer.png \
  ironfellingaxe.png \
  ironpickaxe.png \
  ironstonecutterhammer.png \
  mithrilfellingaxe.png \
  mithrilpickaxe.png \
  mithrilstonecutterhammer.png \
  fruitfulhuntinggroundsclearing.png \
  fruitfulhuntinggroundswater.png \
  fruitfulhuntinggroundswoodland.png \
  humancorpse.png \
  trainingstonecutterhammer.png
do
  cp "$frontend_static/art/items/$item_art_file" "$axum_root/static/art/items/"
done

for art_file in \
  cache.png \
  cache.json \
  warehouse.png \
  warehouse.json \
  supplycache.png \
  supplycache.json \
  washedashore.png \
  washedashore.json \
  novicewarrior.png \
  novicewarrior.json \
  sailor.png \
  sailor.json \
  droppedbag.png \
  droppedbag.json
do
  cp "$frontend_static/art/$art_file" "$axum_root/static/art/"

  if [[ "$art_file" == *.json ]]; then
    cp "$frontend_static/art/$art_file" "$server_tileset/"
  fi
done
