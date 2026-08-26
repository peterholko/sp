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
cp index.html "$axum_root/index.html"

mkdir -p "$axum_root/static/art"
mkdir -p "$axum_root/static/art/ui/activity"
mkdir -p "$axum_root/static/art/ui/resource_categories"
mkdir -p "$axum_root/static/art/portraits/heroes"
mkdir -p "$axum_root/static/art/portraits/villagers"
mkdir -p "$axum_root/static/art/social"
mkdir -p "$server_tileset"

cp "$frontend_static/art/ui/activity/"*.png "$axum_root/static/art/ui/activity/"
cp "$frontend_static/art/ui/resource_categories/"*.png "$axum_root/static/art/ui/resource_categories/"
cp "$frontend_static/art/portraits/heroes/"*.png "$axum_root/static/art/portraits/heroes/"
cp "$frontend_static/art/portraits/villagers/"*.png "$axum_root/static/art/portraits/villagers/"
cp "$frontend_static/art/social/perilous-discord-embed.png" "$axum_root/static/art/social/"

for ui_art_file in \
  intro_01_new_lands.png \
  intro_02_shipwreck.png \
  intro_03_ashore.png \
  terrainfeaturebutton.png \
  terrainfeaturebutton_click.png
do
  cp "$frontend_static/art/ui/$ui_art_file" "$axum_root/static/art/ui/"
done

mkdir -p "$axum_root/static/art/items"
for item_art_file in \
  bones.png \
  crudehatchet.png \
  fishingrod.png \
  foragingkit.png \
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
  resintorch.png \
  litresintorch.png \
  torch.png \
  littorch.png \
  trainingstonecutterhammer.png
do
  cp "$frontend_static/art/items/$item_art_file" "$axum_root/static/art/items/"
done

for art_file in \
  blacksmith.png \
  burrow.png \
  cache.png \
  cache.json \
  craftingtent.png \
  farm.png \
  foundation.png \
  lumbercamp.png \
  mine.png \
  quarry.png \
  stockade.png \
  trapper.png \
  warehouse.png \
  warehouse.json \
  watchtower.png \
  workshop.png \
  supplycache.png \
  supplycache.json \
  washedashore.png \
  washedashore.json \
  novicewarrior.png \
  novicewarrior.json \
  droppedbag.png \
  droppedbag.json \
  wellstructure.png \
  wellstructure.json
do
  cp "$frontend_static/art/$art_file" "$axum_root/static/art/"

  if [[ "$art_file" == *.json ]]; then
    cp "$frontend_static/art/$art_file" "$server_tileset/"
  fi
done
