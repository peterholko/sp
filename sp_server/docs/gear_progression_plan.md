# Headless Bot Gear Progression — Parked Design

## Status

Proposed and intentionally parked. The current headless bot does not build a
Crafting Tent or Blacksmith and does not execute a tiered smithing/equipment
loop.

The bot currently implements the revised opening: investigate the run-owned
Shipwreck, transfer its salvage, equip recovered class gear and the Sharpened
Stick, use the Crude Hatchet for lumberjacking, and build a normal Burrow from
five recovered Logs. It can then maintain food and Campfire fuel, build
Stockades, hire or assign villagers, upgrade the sanctuary, and use class-valid
combat actions.

`headless_bot.rs` explicitly parks the old Crafting Tent target because spending
the early Log supply on it removed the bot's sustainable Firewood/cooking path
and led to starvation. Gear work should resume only after ordinary wood
gathering supplies both construction and cooking.

## Current economy facts

Recipes are defined in `templates/recipe_template.yaml`, refine outputs in
`templates/item_template.yaml`, and structure requirements in
`templates/obj_template.yaml`.

- Crafting grants XP to the recipe's crafting skill.
- Refining grants the corresponding refining skill XP.
- Combat kills grant weapon-subclass XP.
- Combat reads equipped item damage and defence; defence reduction is
  `D / (D + 50)`.

Current relevant structures:

| Structure | Current requirement | Role |
| --- | --- | --- |
| Crafting Tent | 5 Log, 5 Hide; 100 work | Level-0 crafting; refines Log, Ore, and Game Animal; upgrade root for specialized crafting |
| Blacksmith | 15 Timber, 10 Ingot, 5 Stone; 600 work | Level-2 metal crafting and Ore refining |
| Workshop | 15 Timber, 5 Ingot; 600 work | Level-2 bow/carpentry crafting |
| Mine | 5 Log, 3 Ingot | Mining structure; the Mine Deed is a later POI reward, not starter salvage |

Relevant material chains remain:

- Ore to Ingot through Smelting;
- Log to Timber, Resin, and Stick through Woodcutting;
- Raw Hide to Leather through Tanning; and
- Game Animal to meat, hide, and bones through Butchery.

Twine depends on Cloth and remains a poor early bootstrap target.

## Current opening baseline

Fresh heroes carry only equipped Tattered Shirt and Tattered Pants. The
run-owned Shipwreck contains:

- Sharpened Stick, Crude Hatchet, Crude Torch, Bedroll;
- three filled Waterskins, three Salted Meat Strips, three Honeybell Berries,
  and one 10-healing Health Potion;
- Flint Shard, Resin, Stick, five Logs, one Timber, three Copper Ingots, ten
  Gold Coins, and a Fishing Rod; and
- Copper Helm for Warrior, Training Bow for Ranger, or five Mana for Mage.

There is no starter Burrow, Mine Deed, or Yurt Deed. The normal Burrow costs
five Logs and is the first construction target. The starter Campfire is already
lit and contains 20 Firewood.

## Required bot architecture

The existing `BuildJob` and structure interaction logic can be generalized,
but the gear loop still needs:

- a sustainable Logging policy before optional construction consumes the food
  economy's fuel;
- structure build/upgrade targets for Crafting Tent, Blacksmith, Workshop, and
  eventually Mine;
- generic refine and `StructureCraft` staging/retrieval;
- hero skill levels in `WorldView`;
- item damage, defence, and slot facts in `ItemView`; and
- deterministic equipment comparison before emitting `PlayerEvent::Equip`.

A target entry should identify item, structure, requirements, skill gate, and
slot. The bot should work only the first reachable target and use normal
production events for every build, transfer, refine, craft, retrieve, and equip
step.

## Proposed checkpoints

1. **Sustainable material bootstrap**
   - Prove ordinary Logging can support Burrow, food fuel, and five additional
     Logs without starving the bot.
   - Preserve Hides from hunting.
   - Build a Crafting Tent only after the survival reserve remains intact.

2. **Generic refining and first equipment**
   - Refine Log to Timber and Hide to Leather through the production structure
     path.
   - Add skill and equipment facts to `WorldView`.
   - Craft, retrieve, compare, and equip one reachable level-0 item.

3. **Blacksmith and copper/iron progression**
   - Build or upgrade to a Blacksmith from legitimately acquired materials.
   - Craft level-0 copper gear to train skills, then reach the existing
     skill-gated copper and iron recipes.

4. **Mining and later tiers**
   - Acquire the later Mine Deed through its existing POI path.
   - Build the Mine, acquire the normal mining tool, and exercise Ore-to-Ingot
     progression.
   - Treat Mithril as a long-run target rather than an opening requirement.

Each checkpoint should be independently testable in the existing headless
harness and must not inject items, skills, deeds, structures, or production
results directly.

## Known constraint

The bot's survival ceiling and material acquisition remain the limiting
factors. Implementing generic crafting mechanics is useful even if ordinary
runs reach only the first equipment step, but later tiers should not be claimed
until a bounded production-path run reaches them without fixture-only grants.
