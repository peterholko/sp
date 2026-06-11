# Gear progression for the headless bot — implementation plan

Goal: the bot drives the game's core crafting + skill loop — build crafting
structures, refine raw materials, craft tiered weapons/armor, equip them, and
grind the crafting/combat skills that gate higher tiers. Item crafting + skill
progression is the game's central mechanic.

This is a large, multi-slice feature. Build it in the order below; each slice is
independently committable and verifiable in the headless harness.

## Economy facts (verified)

Recipes: `templates/recipe_template.yaml`. Refine outputs: item `produces` lists in
`templates/item_template.yaml`. Structures: `templates/obj_template.yaml`.

- **Crafting grants the crafting skill XP** (CraftEvent handler, game.rs:
  `crafter_skills.update(skill_from_recipe_class_or_subclass, 100, ...)`), so
  crafting skill-0 gear levels Weaponsmithing/Armorsmithing → unlocks skill-10/15/
  25/... recipes. **Refining grants refine-skill XP** the same way (Smelting,
  Woodcutting, Tanning, Butchery). **Combat grants weapon-subclass XP on kill**
  (combat.rs) → weapon damage bonus.
- Combat uses equipped item attrs: `damage_from_items` (sum of `Damage`) and
  `defense_from_items` (sum of `Defense`); `defense_reduction = D/(D+50)`. Better
  gear measurably helps (combat.rs process_attack).

### Structures (build `req` from obj_template.yaml)
- **Crafting Tent** — Log 5 + Hide 5, level 0 (early). Refines Log→Timber,
  Raw Hide→Leather, Ore→Ingot, Game Animal→meat. Crafts hide/leather armor.
- **Blacksmith** — Timber 15 + Ingot 10 + Stone 5, level 2 (expensive). Crafts
  copper/iron/mithril weapons + armor; refines ore.
- **Workshop** — Timber 15 + Ingot 5. Bows.
- **Mine** — Log 5 + Ingot 3 (Mine Deed in burrow). Mining → ore. Needs a
  Training Pick Axe (recipe: Ingot 2 + Log 2; gives Mining tool bonus).

### Material chains
- Ore → Ingot: Smelting (skill 0 for Copper/basic Iron; 25 Mistvale Iron; 75
  Mithril) at Crafting Tent/Blacksmith.
- Log → Timber (+Resin+Stick): Woodcutting skill 0.
- Raw Hide (class `Hide`) → Stiff Leather (class `Leather`): Tanning skill 0.
- Felled animal → Raw Meat + Raw Hide + bones: Butchery (already used for food).
- Twine ← Cloth (Honeybell Cloth) — Cloth source is unclear/gathered; AVOID
  Twine-gated recipes early. Metal gear (Blacksmith) needs no Twine.

### Hero start (player_setup.rs)
- Burrow: Copper Ingot 3, Maple Timber 3, Maple Log 5, Mine Deed, Yurt Deed.
- Hero: Copper Training Axe (Dmg 11) equipped, Copper Helm, shirt, pants; 0 skills.
- Pre-built: Burrow + (now) a lit Campfire.

### Representative recipe gates
- skill 0: Copper Dagger (Dmg12, Blacksmith, CuIngot1+Timber1), Hide armor (Tent).
- skill 10: Copper Short Sword (Dmg18), Copper Mace (Dmg18).
- skill 15: Copper Cuirass (Def4).
- skill 25: Iron Sword (Dmg30), Iron gear (Blacksmith, IronIngot+Timber/Leather).
- skill 60+: Mithril.

## Bot architecture

Add a generic crafting layer to `headless_bot.rs`, reusing existing patterns:
- BuildJob/next_build_job → already builds campfire/walls; generalize to also
  build Crafting Tent, then Blacksmith, then Mine when their material reqs are met.
- StructureCraft (used for cooking) → generalize: stage recipe `req` into the
  structure, StructureCraft, retrieve + equip the result.
- Refine (used for butchering) → generalize: refine raw materials to inputs
  (hide→leather, log→timber, ore→ingot) at the right structure.
- Equip: `PlayerEvent::Equip { item_id, slot }` for crafted weapons/armor that
  beat the currently-equipped item's Damage/Defense.

A **gear target list** (ordered by achievability) drives it: each entry =
{item, structure, reqs, skill_req, slot}. The bot works the first target whose
skill_req it meets, gathering/refining missing inputs, building the missing
structure, crafting, equipping. Crafting also grinds the skill toward the next.

WorldView additions: per-structure refine/craft capability already exposed via
StructureView; add hero skill levels (Weaponsmithing/Armorsmithing/Smelting/…)
and ensure `inventory` ItemViews expose Damage/Defense attrs + slot so the bot can
compare gear.

## Phases

**Phase 1 — Crafting framework + Crafting Tent + refining + first equip.**
1. Bot keeps Raw Hide from hunts (don't discard) toward the Tent + leather.
2. Build a Crafting Tent (Log 5 + Hide 5).
3. Refine at the Tent: Log→Timber, Raw Hide→Leather, (Copper Ore→Ingot if mined).
4. Craft + equip the best skill-0 gear reachable without Twine (start with metal
   once a Blacksmith exists; until then the Tent mainly refines + levels skills).
   Validates: build→stage→craft→retrieve→equip + skill XP gain.

**Phase 2 — Blacksmith + copper/iron weapons & armor.**
1. Refine enough Timber (logs) + Ingots (ore) + gather Stone for the Blacksmith.
2. Build Blacksmith.
3. Craft skill-0 Copper Dagger first (grinds Weaponsmithing), then Copper Short
   Sword/Mace (skill 10), Copper Cuirass (15); equip upgrades over the starting axe.
4. Combat with the new weapon grinds weapon-subclass skill (damage).

**Phase 3 — Mining + smelting + iron/mithril + skill grind.**
1. Build a Mine (Mine Deed) + craft a Training Pick Axe; mine ore + stone.
2. Smelt ore→ingots (grinds Smelting); craft Iron gear at skill 25+, Mithril at 60+.
3. Loop: craft to grind smithing skill → unlock next tier → craft+equip.

## Known constraint

Bootstrapping: the deep chains + the bot's ~day-7 lifespan mean the bot realistically
reaches only ~Phase 1–2 before dying; meaningful tiers (iron/mithril) need the
survival ceiling raised first (separate work). The crafting *system* still gets
exercised + validated; full realization needs longer bot survival.
