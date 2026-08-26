# Scouting and Resource Categories

## Player-facing rule

**Scout** is the broad, visual search action. It shows what kinds of resources
may be present without identifying a specific deposit. **Prospect** remains the
focused action that identifies an exact resource on one tile and makes that
deposit available for gathering.

## Scout action

- Scouting is a hero action and takes two seconds.
- It cannot begin during the global **Night** phase. A torch or other local
  light does not bypass this restriction.
- If Night begins before the action completes, the action ends without a
  result and does not record the affected tiles as scouted.
- A successful Scout covers the hero's current tile and its six adjacent hexes.
- Scouted knowledge is stored per player and survives reconnects.
- A successful result automatically displays the category overlay.

## Resource categories

Scouting reports one icon for each category present on a tile. Multiple named
deposits in the same category collapse into one icon.

| Category | Exact resource types represented |
| --- | --- |
| Ore | Ore |
| Stone | Stone |
| Timber | Log |
| Forage | Forage |
| Water | Spring Water |
| Fish | Fish |
| Game | Game Animal |

The icons do not reveal a resource's name, quality, quantity, yield,
properties, or exact deposit. Prospecting is still required for that
information and for normal gathering access.

Each category uses a 48x48 transparent source icon. At the default 2x desktop
camera zoom it renders at its native 48 screen pixels. Tiles with several
categories arrange them in rows of three with five screen pixels of overlap,
forming a compact stack rather than shrinking the individual silhouettes.

## Nearby Resources control

- When Scout results are visible, **Nearby Resources** is active.
- Pressing the active button hides all floating category icons.
- Pressing it again immediately restores the last category overlay. If the
  client has no cached Scout result (for example after reconnecting), it
  requests the currently scouted categories within the existing nearby radius.
- An empty result leaves the overlay and button inactive.

## Scope

This change formalizes discovery and presentation only. Resource frequency,
depletion, regeneration, and scarcity remain separate balance work.
