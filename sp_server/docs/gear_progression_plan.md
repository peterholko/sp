# Hero Gear Progression — Runtime Milestone

## Status

The player-facing primitive-to-copper progression is implemented. Fresh
players know the primitive equipment recipes and the foundational copper and
bow recipes, while their ordinary structure and skill requirements remain
authoritative. The rescued villager teaches the Lumbercamp plan, the Crafting
Tent supports basic Hide tanning, and primitive/copper/bow families now carry
explicit tiers into the existing iron and mithril experimentation ladder.
Logging tools now have their own progression instead of treating every combat
axe as a lumber tool. The Crude Hatchet remains the deliberate starter hybrid;
the Flint Hatchet and Copper/Iron/Mithril Felling Axes are dedicated tools.
Hunting intentionally differs: spears and bows are both combat weapons and
Hunting tools. Daggers and slings remain combat-only, keeping the eligible
weapon families explicit without adding a parallel trapping-kit item line.
Fishing uses a dedicated rod line. The recovered Fishing Rod remains the
tier-0 tool and known replacement recipe, while Copper, Iron, and Mithril rods
advance through the existing experimentation ladder.
Farming now follows the same material progression through the existing Sickle
family; the Farm harvest action uses the equipped tool's rating and durability.

Autonomous bot execution of the complete production loop remains parked. The
current headless bot does not yet build a Crafting Tent or Blacksmith and does
not independently execute a tiered smithing/equipment loop.

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
| Crafting Tent | 5 Log, 5 Hide; 100 work | Level-0 crafting; refines Log, Ore, Game Animal, and Hide; upgrade root for specialized crafting |
| Blacksmith | 15 Timber, 10 Ingot, 5 Stone; 600 work | Level-2 metal crafting and Ore refining |
| Workshop | 15 Timber, 5 Ingot; 600 work | Level-2 bow/carpentry crafting |
| Tannery | 10 Timber, 5 Ingot; 500 work | Specialized Hide-to-Leather refining; available as a Crafting Tent upgrade |
| Lumbercamp | 5 Log; 100 work | One persistent Logging workspace; requires and wears the operator's equipped Logging tool, stores output locally, and works 20% faster than field logging |
| Mine | 5 Log, 3 Ingot | Two persistent Mining workspaces; each requires and wears the operator's equipped Mining tool, stores one deposit's output per cycle locally, and works 20% faster than field mining; the Mine Deed is a later POI reward, not starter salvage |
| Quarry | 5 Log, 3 Ingot | One persistent Stonecutting workspace; requires and wears the operator's equipped Stonecutting tool, stores one Stone output per cycle locally, and works 20% faster than field stonecutting; the Quarry Deed and a Training Stonecutter Hammer are available at the Abandoned Mine |
| Trapper | 5 Log, 3 Hide | One persistent Hunting workspace; requires and wears the operator's equipped Hunting tool, stores one eligible animal's carcass outputs per cycle locally, and works 20% faster than field hunting; its Deed remains a merchant purchase |
| Farm | 5 Log | Crop planting and harvesting structure; harvesting requires and wears an equipped Farming tool, while villagers can retrieve the best reachable Sickle from owned storage; its Deed and starter Sickle remain merchant purchases |

Relevant material chains remain:

- Ore to Ingot through Smelting;
- Log to Timber, Resin, and Stick through Timberworking;
- Raw Hide to Leather through Tanning; and
- Game Animal to meat, hide, and bones through Butchery.

The Logging tool line is:

| Tool | Logging | Durability | Lumbercamp cycle |
| --- | ---: | ---: | ---: |
| Crude Hatchet | 1 | 30 | 24.0 seconds |
| Flint Hatchet | 2 | 45 | 19.2 seconds |
| Copper Felling Axe | 3 | 75 | 14.4 seconds |
| Iron Felling Axe | 4 | 120 | 9.6 seconds |
| Mithril Felling Axe | 4 | 180 | 9.6 seconds |

The dedicated Mining tool line is:

| Tool | Mining | Durability | Mine cycle |
| --- | ---: | ---: | ---: |
| Training Pick Axe | 2 | 60 | 19.2 seconds |
| Copper Pick Axe | 3 | 90 | 14.4 seconds |
| Iron Pick Axe | 4 | 140 | 9.6 seconds |
| Mithril Pick Axe | 4 | 210 | 9.6 seconds |

The known Training Pick Axe recipe is the tier-0 root. Experimentation advances
through Copper, Iron, and Mithril Pick Axes without treating any of them as
combat weapons.

The dedicated Stonecutting tool line is:

| Tool | Stonecutting | Durability | Quarry cycle |
| --- | ---: | ---: | ---: |
| Training Stonecutter Hammer | 2 | 60 | 19.2 seconds |
| Copper Stonecutter Hammer | 3 | 90 | 14.4 seconds |
| Iron Stonecutter Hammer | 4 | 140 | 9.6 seconds |
| Mithril Stonecutter Hammer | 4 | 210 | 9.6 seconds |

The Training Stonecutter Hammer recipe is known from the start and forms the
tier-0 experimentation root. A Quarry worker automatically equips a carried
Stonecutter Hammer or retrieves the best reachable one from owned storage.
Stonecutting remains resource extraction; Masonry remains the separate skill
for refining and building with stone.

The dedicated Fishing tool line is:

| Tool | Fishing | Durability | Direct fishing cycle |
| --- | ---: | ---: | ---: |
| Fishing Rod | 1 | 40 | 30.0 seconds |
| Copper Fishing Rod | 2 | 75 | 24.0 seconds |
| Iron Fishing Rod | 3 | 120 | 18.0 seconds |
| Mithril Fishing Rod | 4 | 180 | 12.0 seconds |

Fishing remains a direct action beside Ocean or River tiles; it does not add a
new structure or standing villager job. The selected rod determines the
authoritative action duration, loses durability after a successful catch, and
can break under the same low-durability rules as other gathering tools. The
base recipe is known from the start, and experimentation advances through the
Copper, Iron, and Mithril recipes at the Crafting Tent or Workshop requirements
declared by each tier.

The dedicated Farming tool line is:

| Tool | Farming | Durability | Farm harvest cycle |
| --- | ---: | ---: | ---: |
| Sickle | 2 | 60 | 19.2 seconds |
| Copper Sickle | 3 | 90 | 14.4 seconds |
| Iron Sickle | 4 | 140 | 9.6 seconds |
| Mithril Sickle | 4 | 210 | 9.6 seconds |

The known Sickle recipe is the tier-0 experimentation root. Copper, Iron, and
Mithril Sickles advance through the existing crafting structure and material
tier requirements. Planting remains hand work and does not consume Sickle
durability; a successful crop harvest grants Farming XP and wears the exact
tool used to begin the action.

The Hunting weapon/tool lines are:

| Weapon | Hunting | Durability | Trapper cycle |
| --- | ---: | ---: | ---: |
| Sharpened Stick | 1 | 25 | 24.0 seconds |
| Stone-Tipped / Throwing Spear | 2 | 45 / 40 | 19.2 seconds |
| Copper Spear | 3 | 75 | 14.4 seconds |
| Iron Spear | 4 | 120 | 9.6 seconds |
| Mithril Glaive | 4 | 180 | 9.6 seconds |
| Training Bow | 2 | 60 | 19.2 seconds |
| Hunting Bow | 3 | 70 | 14.4 seconds |
| Iron-Limbed Longbow | 4 | 90 | 9.6 seconds |
| Mithril Warbow | 4 | 120 | 9.6 seconds |

The Trapper worker automatically equips a carried spear or bow or retrieves
the best reachable Hunting weapon from owned storage before reporting the job
as blocked. Hunting wear uses the weapon's ordinary durability.

Copper Training/Broad/Heavy Axes and the Iron/Mithril War Axes are combat
weapons and do not grant Logging. A Lumbercamp worker automatically equips a
matching tool already carried or retrieves the best reachable one from owned
storage before reporting the job as blocked.

Spears and bows grant Hunting. Daggers and slings do not, even if their names
or flavor suggest survival use, so the tool-selection rule remains predictable.

Twine still depends on Cloth. The Shipwreck now contains five Honeybell Cloth,
enough to craft the first primitive weapon bindings and hide armor without
waiting for the traveling merchant.

## Current opening baseline

Fresh heroes carry an unequipped Sharpened Stick alongside their equipped
Tattered Shirt and Tattered Pants. The run-owned Shipwreck contains:

- Crude Hatchet, three Crude Torches, Bedroll;
- three filled Waterskins, three Salted Meat Strips, three Honeybell Berries,
  and one 10-healing Health Potion;
- Flint Shard, Resin, Stick, five Honeybell Cloth, one Windstride Raw Hide,
  five Logs, one Timber, three Copper Ingots, ten Gold Coins, and a Fishing
  Rod; and
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

## Runtime checkpoints

1. **Sustainable material bootstrap**
   - The Shipwreck supplies the Burrow Logs, early Cloth, tools, and three
     Copper Ingots without supplying a second building for free. Its one Raw
     Hide introduces the Shelter Tent requirement without replacing hunting.
   - The rescued villager teaches the Lumbercamp plan.
   - The tutorial sends the hero to Prospect a forest, assigns the rescued
     villager to persistent Logging, then returns control to the hero to hunt
     and butcher a carcass for additional Raw Hide and meat.
   - Ordinary Logging must still supply the next five Logs while preserving
     the Campfire and cooking reserve.

2. **Generic refining and first equipment**
   - Primitive weapon and hide-armor recipes are deterministic player recipes.
   - The Crafting Tent can refine Log, Ore, Game Animal, and Hide.
   - The Tannery is a valid Crafting Tent upgrade and can refine Hide.

3. **Blacksmith, Workshop, and copper progression**
   - Foundational copper melee/armor recipes are known but remain gated by the
     Blacksmith and their existing crafting skill requirements.
   - Training Bow and Hunting Bow are known but remain Workshop-gated.
   - Primitive, copper, iron, and mithril recipes use explicit tiers so
     experimentation advances one material tier at a time.

4. **Mining and later tiers**
   - Acquire the later Mine Deed through its existing POI path.
   - Build the Mine, acquire the normal mining tool, and exercise Ore-to-Ingot
     progression.
   - Treat Mithril as a long-run target rather than an opening requirement.

Focused template, experimentation, setup-manifest, and rescue-plan tests cover
the runtime links. A future autonomous-bot checkpoint must still use normal
production events for every build, transfer, refine, craft, retrieve, and equip
step rather than injecting results.

## Known constraint

The bot's survival ceiling and material acquisition remain the limiting
factors. Implementing generic crafting mechanics is useful even if ordinary
runs reach only the first equipment step, but later tiers should not be claimed
until a bounded production-path run reaches them without fixture-only grants.
