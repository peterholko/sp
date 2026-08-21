# Source artwork

This directory retains the high-resolution source artwork used to produce the
game's optimized runtime images. These files are source masters only: the game
continues to load its resized and cleaned assets from
`sp_frontend/priv/static/art`, with deployed copies under
`sp_axum/root/static/art`.

The archived images are the exact generated outputs recovered from the Codex
image cache on 2026-08-20. Some structure sources contain a baked checkerboard
because that was present in the original generated image; it is intentionally
preserved here. The corresponding runtime images already have transparent
backgrounds and should not be regenerated during the frontend copy step.

## Resource category sources

The seven 1254x1254 RGBA masters are in `resource-categories/`:

- `ore.png`
- `stone.png`
- `timber.png`
- `forage.png`
- `water.png`
- `fish.png`
- `game.png`

Their optimized 48x48 runtime icons live in
`sp_frontend/priv/static/art/ui/resource_categories/`.

## Structure sources

The high-resolution structure masters are in `structures/`:

- `foundation.png`
- `burrow.png`
- `wellstructure.png`
- `craftingtent.png`
- `stockade.png`
- `mine.png`
- `lumbercamp.png`
- `quarry.png`
- `trapper.png`
- `cache.png`
- `warehouse.png`
- `blacksmith.png`
- `workshop.png`
- `farm.png`
- `watchtower.png`

The filenames intentionally match their canonical runtime asset names.

## Intro slideshow sources

The three 1672x941 narrative masters are in `intro/`:

- `01-exploring-new-lands.png`
- `02-storm-and-reefs.png`
- `03-washed-ashore.png`

Their optimized 800x450 runtime images live in
`sp_frontend/priv/static/art/ui/`.
