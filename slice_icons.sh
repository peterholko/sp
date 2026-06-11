#!/bin/zsh
# Slice weapon/armor sprites from the RPG_Weapon_and_Armor pack into the game's
# item icon dirs. Resizes 256x256 -> 48x48 (matches existing icon size).
set -e
PACK="/Users/peter/projects/assets/items/RPG_Weapon_and_Armor"
DESTS=(
  "/Users/peter/projects/sp/sp_axum/root/static/art/items"   # authoritative (served by axum)
  "/Users/peter/projects/sp/sp_axum/root/static/.0/items"    # mirror
  "/Users/peter/projects/sp/sp_frontend/priv/static/art/items" # frontend source
)

# dest_name  category/file
MAP=(
  # --- weapons ---
  "bonedagger Knives/3631.png"
  "mithrildagger Knives/1426.png"
  "flinthatchet Axe/1300.PNG"
  "mithrilwaraxe Axe/9930.PNG"
  "ironwarhammer Hammer/1478.PNG"
  "mithrilmaul Hammer/9995.PNG"
  "coppermace Maces/1423.png"
  "ironmace Maces/9759.PNG"
  "mithrilgreatsword Swords/7201.png"
  "throwingspear Spears/2479.png"
  "copperspear Spears/1461.png"
  "ironspear Spears/6508.PNG"
  "ironhalberd Alibards/1428.png"
  "mithrilhalberd Alibards/nj_621.png"
  "mithrilglaive Spears/9757.PNG"
  "longbow Bow/1641.png"
  "warbow Bow/7524.PNG"
  "crossbow Crossbow/1429.png"
  # --- helmets ---
  "hidecap Helmet/finp_332.PNG"
  "ironhelm Helmet/7350.PNG"
  "mithrilhelm Helmet/nj_652.PNG"
  "studdedleathercap Helmet/1453.png"
  # --- chest ---
  "coppercuirass Armor/finp_902.PNG"
  "studdedleathervest Armor/nj_422.PNG"
  # --- pants ---
  "hideleggings Pants/finp_922.png"
  "coppergreaves Pants/6775.png"
  "irongreaves Pants/1443.png"
  "mithrilgreaves Pants/9679.png"
  # --- boots ---
  "hideboots Boots/finp_916.png"
  "coppersabatons Boots/6408.png"
  "ironsabatons Boots/9708.PNG"
  "mithrilsabatons Boots/nj_441.PNG"
  "reinforcedleatherboots Boots/4443.PNG"
  # --- shoulders ---
  "hidemantle Shoulders/finp_919.png"
  "copperpauldrons Shoulders/6256.png"
  "ironpauldrons Shoulders/1459.png"
  "mithrilpauldrons Shoulders/9665.png"
  # --- shields ---
  "copperbuckler Shields/6249.png"
  "ironkiteshield Shields/5494.png"
  "mithriltowershield Shields/nj_634.png"
)

count=0
for entry in "${MAP[@]}"; do
  name="${entry%% *}"
  rel="${entry#* }"
  cat="${rel%%/*}"
  file="${rel#*/}"
  src="$PACK/$cat/transparent/$file"
  if [[ ! -f "$src" ]]; then
    echo "MISSING SRC: $src"; exit 1
  fi
  # resize into first dest, then copy to mirrors
  first="${DESTS[1]}/$name.png"
  sips -z 48 48 "$src" --out "$first" >/dev/null 2>&1
  for d in "${DESTS[@]:1}"; do cp "$first" "$d/$name.png"; done
  count=$((count+1))
done
echo "Sliced $count icons into ${#DESTS[@]} dirs."
