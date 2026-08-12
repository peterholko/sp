# Native 144px map presentation

## Goal

Render the map on a true 144px logical hex grid at the normal 1× camera zoom.
Terrain, opening-game structures, and opening-game units use native 144px art at
1:1 scale. The existing 50×50 world and all server-side hex coordinates remain
unchanged; only the client-side conversion from a hex coordinate to world pixels
is larger.

This is intentionally a visual scale change. Compared with the former 72px
grid, fewer hexes are visible at once and each unit has a larger canvas for more
readable detail.

## Geometry contract

- A logical hex canvas is 144×144 world pixels.
- Adjacent columns are 108 pixels apart (`144 × 0.75`).
- Odd columns are vertically staggered by 72 pixels.
- A hex centre is 72 pixels from its top-left origin.
- Camera tracking, selection, shroud, weather masks, combat effects, floating
  text, progress bars, speech bubbles, and sanctuary borders use this same
  geometry.

The authoritative server still sends integer hex coordinates. No pathfinding,
visibility, combat-range, sanctuary-radius, or world-size rule changes as part
of this presentation migration.

## Authoring contract

A native one-hex unit or structure uses a 144×144 frame and `map_scale: 1`:

```json
{
  "frames": {
    "width": 144,
    "height": 144
  },
  "map_scale": 1,
  "animations": {
    "none": [0],
    "moving": {
      "frames": [0, 1, 2],
      "repeat": -1,
      "speed": 750
    }
  }
}
```

Definitions that still omit `map_scale` are treated as legacy 72px art and are
temporarily enlarged 2×. This keeps deferred mid- and late-game art usable while
making native 144px definitions explicit. Scale is never inferred from source
dimensions because some scenery intentionally spans more than one hex.

Multi-image structures use doubled source dimensions and registration offsets,
then render with `map_scale: 1`. Terrain offsets in `static/tileset.json` are
also doubled because they are world-space pixel offsets on the larger grid.

## Opening-game conversion

The opening-game map set is available as native 2× source canvases:

- Starter Warrior, Ranger, and Mage presentations.
- Rescued villagers, opening Giant Rats, and early random-encounter creatures.
- Meager Merchant, Necromancer, and the Shipwreck Zombie.
- Shipwreck, Campfire, Burrow, Lumber Camp, Crafting Tent, Shelter Tent,
  Stockade, Monolith, and the start-area POIs.
- The terrain entries used by the current map, including oversized forests and
  mountains.

Most migrated legacy art was expanded with nearest-neighbour resampling. This
preserves its pixels and prevents interpolation but does not invent detail.
Individual frames can now be replaced with newly authored 144px art.

The checked-in migration and validation command is:

```bash
python3 sp_frontend/sp_ts/scripts/migrate_opening_map_art_144.py --check
```

## Temporary Novice Warrior experiment

`novicewarrior` currently aliases the experimental 144×144 Sailor test art. Its
five-frame sword attack adds a wind-up, forward lunge, full-extension thrust, and
recovery while keeping every pose on the same foot baseline. The four-frame walk
loop uses neutral stance, left-foot contact, neutral stance, and right-foot
contact, with the sword held upright consistently across all four poses. Other
actions retain the sword-equipped resting frame.

## Deferred art

- Newly painted mid- and late-game units and structures.
- React-only inventory and target-preview art, which uses independent UI sizing.
- Newly painted versions of legacy transition, shroud, selection, weather,
  spell, fire-effect, and resource-overlay textures. These currently scale to
  the 144px geometry at runtime.
