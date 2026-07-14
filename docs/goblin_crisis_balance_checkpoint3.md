# Goblin Crisis Balance Milestone — Checkpoint 3

## Evidence and Change Proposal

This checkpoint starts from the completed Checkpoint 2 candidate at commit
`4b6bc48`. Checkpoint 2 changed only the `Preparing` and `AssaultReady`
pressure thresholds and the `AssaultReady` explanation. It deliberately left
pressure weights, phase minima, launch rules, assault composition, enemy
statistics, spawn rules, targeting, Safe Logout, disconnect behavior, and the
economy unchanged.

### Evidence reviewed before implementation

The completed candidate comparison contains 117 rows: 54 natural-progression
rows and 63 staged attainable-facts rows, balanced across Warrior, Ranger, and
Mage. Seventy-two assaults launched and 21 resolved. Natural play launched
9/54 rows and resolved 1/9 launches; the staged cohort launched 63/63 and
resolved 20/63. A typical launched candidate row still recorded one hero death
(median 1, n=63). Candidate wall damage had median zero, so wall destruction
alone is not a sensitive measure of defensive usefulness.

The late-phase timing evidence is sufficient to freeze pacing for this
checkpoint:

| Observation | Mean ticks | Median ticks | Samples | Observed range |
|---|---:|---:|---:|---:|
| Natural `Preparing` duration | 1,894.0 | 1,800 | 10 | 1,800–2,235 |
| Natural `AssaultReady` duration | 839.0 | 790 | 9 | 355–1,200 |
| Natural Preparing warning to launch | 2,726.7 | 2,590 | 9 | Not reported per-row in the Checkpoint 2 summary |
| Staged Preparing warning to launch | 2,991.1 | 3,000 | 63 | Determined by unchanged phase minima/window behavior |
| Staged AssaultReady warning to launch | 1,191.1 | 1,200 | 63 | Determined by unchanged grace/window behavior |

The existing coarse sampler observed at least one preparation action in 41/63
staged rows (65.1%) and 9/20 natural rows that reached Preparing (45.0%). Those
rates establish that actions occur in the available window, but the old
positive-delta counter is not causal evidence that any one action helped.

The median staged actionable budget is therefore 3,000 online ticks, or 300
seconds. The hard configured floor is 1,800 Preparing ticks plus the 300-tick
Ready grace, or 2,100 online ticks (210 seconds). The natural median observed
Preparing-warning-to-launch budget was 2,590 ticks (259 seconds). The original
raw rows were overwritten during the completed Checkpoint 2 workflow, so an
exact observed per-row shortest and longest total cannot be reconstructed and
must not be claimed. The separately reported completed phase ranges are not
combined as though their endpoints came from the same run.

Checkpoint 1 already observes equipped weapons and armor count, carried
healing quantities, completed/foundation structures, wall count and HP,
living/combat-capable villagers, sanctuary level, storage, and settlement
proximity. Its sampled delta telemetry observes positive preparation changes,
but repeated equipment toggles can inflate its raw equipment counter and it
does not yet expose first-action timing or distinct meaningful categories.

### Gameplay-change proposal

No numeric or behavioral gameplay change is proposed before the paired
evidence. This is an intentional zero-change proposal, within the checkpoint's
maximum of three:

| Field | Decision |
|---|---|
| Finding | Checkpoint 2 proves that the preparation window exists and that combat remains difficult, but its scenario groups are unpaired and do not isolate repair, walls, equipment, healing, villagers, or sanctuary. |
| Evidence | 117 candidate rows; 72 launches; 21 resolutions; staged prepared-solo 1/9, fortified-solo 3/9, no-villager 1/9, villager-supported 4/9. Production randomness makes those directional groups non-causal. |
| Player problem | The Preparing copy names broad activities but does not identify the owner's current weaknesses or distinguish an available action from a blocked one. |
| Hypothesis | Server-authoritative guidance plus paired measurement will make existing useful systems discoverable without changing their effects. |
| Exact change | Add a read-only preparation-options field, client presentation, meaningful-action telemetry, and matched scenario instrumentation. Do not change repair, defence, healing, equipment, villager, sanctuary, pressure, phase, assault, or economic values. |
| Success metric | At least two existing paths improve a declared paired outcome; the UI shows no more than four owner-only authoritative options; unchanged options deduplicate; focused configuration/invariant tests prove the frozen Checkpoint 2 controls remain unchanged. |
| Rollback condition | Remove or revise an option/path if it recommends an action that cannot reasonably begin, leaks another owner's state, produces packet churn, creates a hidden gameplay effect, or fails its paired usefulness criterion. Any later gameplay experiment must be separately predeclared here before implementation. |
| Wider effects | Additive v1 network payload, crisis-card layout, opt-in headless telemetry/report rows. No production simulation effect. |
| Validation scenarios | Owner-isolation and phase-visibility tests; option-state update/dedup tests; client Preparing/Ready/Active tests; matched control versus existing-wall, equipment, healing, and combined preparation where feasible. |

The known starting Health Potion discrepancy (runtime healing 10 versus item
template healing 50) is recorded as an architecture conflict, not silently
treated as a bug. Prior milestone text calls the runtime override intentional,
and the completed evidence does not isolate its effect. Checkpoint 3 therefore
does not normalize it without a separately declared and successful experiment.

### Predeclared acceptance bands

These bands are grounded in the Checkpoint 2 staged resolution rate (31.7%),
prepared-solo rate (11.1%), fortified rate (33.3%), villager-supported rate
(44.4%), typical one hero death, and median zero wall destruction:

- Every included matched pair must pass its selected observed-launch-field
  equivalence check after excluding only its declared preparation difference.
- Setup failures, panics, timeouts, and unresolved assaults remain in the raw
  result set.
- An individual path is useful when at least 3/5 valid pairs improve its
  path-relevant composite outcome without worsening hero survival, or when it
  improves assault resolution by at least 15 percentage points.
- Basic Survival must not become automatic: a matched control cohort resolving
  4/5 or 5/5 assaults would be outside the Checkpoint 2 staged 31.7% reference
  band and would trigger a regression review rather than count as preparation
  success.
- Wall usefulness is judged by damage absorbed, first-contact delay, hero
  damage, or core exposure—not wall-destruction median alone.
- Combined preparation must improve at least one major survival/combat outcome
  in at least 3/5 valid pairs, but a 5/5 effortless resolution result is a
  trivialization warning rather than an automatic acceptance.
- A path is treated as dominant/mandatory if it is the only individual path to
  improve at least 3/5 pairs and the combined result cannot meet its 3/5 band
  without that path. Acceptance requires at least two independently useful
  paths, so this condition fails the checkpoint rather than selecting a single
  prescribed preparation.
- Each supported class must record at least one valid improved solo pair in an
  accepted path without worse survival. A class with no improved pair has not
  demonstrated that it can benefit, regardless of aggregate results.
- Every primary preparation comparison remains owner-solo: no helper fixture
  may be required for control or treatment. Helper-supported runs are a
  separate compatibility check and cannot satisfy a failed solo band.
- No path may change pressure, phase timing, assault composition, enemy stats,
  spawn rules, ownership, offline protection, Safe Logout, or the economy.

### Repository conflict: deterministic seeds

The requested identical-seed replay is not currently available. Production
uses entropy-backed `thread_rng` across start allocation, crisis spawn
selection, combat rolls, player hit chance, NPC movement and attack choices,
weather, encounters, and other systems. The current headless run ID is a matrix
identifier, not an RNG seed. Seeding only one call site would falsely imply
determinism and routing all simulation randomness through a new replay RNG is a
broad cross-system refactor outside this checkpoint.

The smallest safe implementation is therefore a matched-observed-fields pair:
both legs use the same declared scenario key, class, progression facts, phase,
pressure, selected settlement facts, and normalized launch geometry. A launch
fingerprint rejects differences in its declared observed fields and the one
intended preparation fixture difference is recorded. Hidden ECS state and RNG
state are not matched. Results must state that the random streams are not
replay-equivalent, must not call the pair a seeded causal replay, and must keep
this limitation in the final outcome. This preserves production RNG behavior
while improving on the unpaired Checkpoint 2 groups without claiming
deterministic causality.

## Architecture Findings

### Audit scope

The implementation audit covered the requested production and harness paths:
`sp_server/src/game.rs`, `game_tests.rs`, `headless.rs`, `headless_bot.rs`,
`bin/headless_runner.rs`, `player.rs`, `combat.rs`, `structure.rs`, `recipe.rs`,
`resource.rs`, `farm.rs`, `item.rs`, `ai/villager/villager.rs`, and
`ai/npc/npc.rs`. It also traced the crisis-status packet and delivery cache,
desktop crisis/Safe Logout UI and tests, Checkpoint 1 telemetry, the Checkpoint
2 report and scenario drivers, repair/build/equip/heal/recruit/sanctuary
mechanics, template/recipe prerequisites, deterministic setup helpers and RNG
call sites, and the supported client compile/test/build commands. The affected
file list below is deliberately narrower than this inspected architecture.

### Crisis status and delivery

- `sp_server/src/network.rs` defines the flat, versioned
  `CrisisStatusSnapshot`; `ResponsePacket::CrisisStatus` flattens it on the
  wire.
- `sp_server/src/game.rs::build_crisis_status` derives phase presentation,
  pressure, ready countdown, and active attacker counts without mutating
  gameplay.
- `crisis_status_delivery_system` builds an owner-specific snapshot, sends it
  only to the current authenticated connection, caches the last successfully
  queued value per connection/player, retries failed sends, and sends current
  state at login/resume.
- `crisis_status_changed` rate-limits only small pressure and countdown deltas.
  Any deterministic preparation-option change is structural and therefore
  sends immediately; equal fixed-order vectors already deduplicate. Phase
  notices remain transition-only and are not emitted for option changes.
- Legacy director mode sends the existing no-crisis clear snapshot.

### Existing preparation systems

- A Stockade is an existing level-zero blocking wall with 20 HP and zero
  defence. Every new player receives its plan. It costs three Logs and 30 work.
- Repair is an existing villager order. After the villager reaches an owned
  damaged structure, the current event is scheduled 50 ticks (5 seconds)
  later and restores HP to maximum. No Checkpoint 3 auto-repair is added.
- Weapon and armor equip are immediate authoritative player events. The
  inventory's equipped state and item class are the source of truth.
- Healing usability follows the actual use paths: Medical/Bandage items apply
  their fixed heal and positive-Health potions use `AttrKey::Healing`. The
  existing starting potion, Herbal Poultice recipe, and normal use-item flow
  are retained; Healing-tagged food is not misreported as a usable crisis heal.
- A living Human Villager has zero base damage unless armed. Existing telemetry
  therefore correctly distinguishes living from combat-capable villagers.
- Sanctuary upgrades are existing Soulshard purchases with escalating costs
  3/6/9/12/15 and the existing 0.25 defence amplifier per level. Sanctuary is
  retained in telemetry but is not one of the four initial guidance rows
  because Checkpoint 2 did not establish frequent actionable access to its
  prerequisites.

### Architecture conflicts and selected resolutions

| Requested/design concern | Repository reality | Checkpoint 3 resolution |
|---|---|---|
| Identical seeded replay | Outcome-relevant systems use entropy-backed `thread_rng`; run IDs are labels, not seeds | Preserve production RNG and implement matched observed launch fields with fixed start/geometry, declared differences, comparability rejection, `full_ecs_state_matched: false`, and `random_stream_replayed: false` |
| Repair cost/rate tuning | Current repair already costs no material, schedules 50 ticks after arrival, and restores full HP | Do not loosen it. Measure travel plus completion and expose repair only when a living villager and damaged wall exist. |
| “Healing item” detection | Crude Bandage heals a fixed 10 HP without `AttrKey::Healing`; some food carries a Healing attribute but follows Eat; the starting potion is runtime 10 versus template 50 and its potion path does not currently consume it | Guidance/telemetry follow actual Medical/Bandage and positive Health-potion semantics, exclude food, and leave potency/consumption unchanged. Record the potion issues for Checkpoint 4/follow-up. |
| Villager count as defence | Human Villagers have zero base damage and the intro villager starts unarmed | Ready requires positive base damage or an equipped weapon. An equip recommendation requires an idle unarmed villager that already holds a spare weapon. |
| Structure-damage outcome | Personal attackers normally target owner human units and blocking walls, not ordinary core buildings | Treat “structure damage” primarily as wall damage and pair it with hero outcomes. Contact/core-exposure metrics do not yet exist, so do not claim ordinary-building protection or wall engagement from zero damage. |
| Exact observed shortest/longest budget | Completed Checkpoint 2 aggregate reports survive, but raw rows were overwritten | Report hard 210-second configured floor and observed medians/ranges only; do not fabricate per-row extrema. |

### Guidance design selected

The additive status field contains at most four fixed-order rows:

1. `defences`
2. `defenders`
3. `equipment`
4. `recovery`

Each row has a stable ID, label, one of `ready`, `needs_attention`, or
`unavailable`, concise authoritative detail, and an action hint. A row is
`needs_attention` only when the existing action can reasonably begin from
current owner state; otherwise it explains the blocker as `unavailable`.
Collection uses exact owner IDs and living/built predicates, never proximity
to infer ownership. Options exist only in `Preparing` and `AssaultReady`; the
optional field is absent in all other phases.

### Files affected

- `sp_server/src/network.rs` — additive preparation-option wire schema.
- `sp_server/src/game.rs` and `sp_server/src/game_tests.rs` — authoritative
  option derivation, delivery integration, telemetry sampling, and tests.
- `sp_server/src/player.rs` — successful preparation-event telemetry hooks.
- `sp_server/src/crisis_balance.rs` — additive action telemetry and
  idempotency state.
- `sp_server/src/headless.rs` — observed-launch-field comparison fixtures,
  fingerprints, and headless tests.
- `sp_server/src/bin/preparation_pair_runner.rs` — bounded paired report runner.
- `sp_server/src/bin/headless_runner.rs` — append-only CSV/report fields.
- `sp_server/goblin_crisis_balance_checkpoint3_pairs.json` — complete final
  20-pair evidence artifact.
- `sp_frontend/sp_ts/src/sp/core/crisisStatus.ts` and
  `crisisStatus.test.ts` — optional schema parsing and bounds tests.
- `sp_frontend/sp_ts/src/sp/desktop/ui/objectivesPanel.tsx`,
  `objectivesPanel.crisis.test.tsx`, and
  `objectivesPanel.safeLogout.test.tsx` — desktop presentation and coexistence
  tests.
- `docs/goblin_crisis_balance_checkpoint3.md` and
  `docs/goblin_crisis_balance_milestone.md` — proposal, evidence, validation,
  limitations, and milestone status.

The crisis payload remains version 1 because the vector is additive and
omitted when empty. The client treats it as optional, validates strings and
stable states, bounds malformed input to four, and hides it outside the two
preparation phases.

## Preparation Budget

The final table will record measured action duration, prerequisites, resource
availability, travel, and paired completion evidence. Architecture timing
before scenario execution is:

| Existing action | Intrinsic server time | Required existing prerequisites | Travel / feasibility assessment |
|---|---:|---|---|
| Repair one damaged defence | 50 ticks (5 s) after arrival | Living owned villager and damaged owned structure; the current implementation has no material charge and restores full HP | Villager movement schedules 48 ticks/tile: about 9.8 s at one tile, 29 s at five, or 53 s at ten including repair. Fits unless the villager is remote; no further cost/rate relaxation is justified. |
| Build one Stockade | 30 work; hero fallback work 5 gives about 60 ticks (6 s) active build | Existing Stockade plan and 3 Log-compatible units deposited into the foundation | Hero movement is about 12 ticks/tile. Placement, deposit, and a local build fit; gathering missing Logs is not assumed to fit. The shipwreck originally contains 10 Logs, but guidance checks current carried facts rather than assuming they remain. |
| Equip carried weapon | Immediate event processing | Live idle owner and unequipped carried equippable weapon | Fits without travel. A storage item requires normal adjacent transfer first. |
| Equip carried armor | Immediate event processing | Live idle owner and unequipped carried armor | Fits without travel. Crafting Hide Wraps takes 75 ticks (7.5 s) but also needs a Crafting Tent, 2 Hide, and 1 Twine, so guidance does not promise the full chain. |
| Prepare one existing healing option | Crude Bandage craft 25 ticks (2.5 s); Herbal Poultice craft 50 ticks (5 s); use follows the normal queued item event | Bandage: 1 Cloth. Poultice: Crafting Tent, 1 Berries, 1 Cloth. A carried item needs no acquisition travel. | A carried Bandage or positive-healing Health potion fits. A stored item is actionable only when normal transfer is currently available. Missing station/resources are not assumed obtainable in time. |
| Prepare one existing villager | Equip is immediate once the villager is live, idle, and already holds the weapon | Living owned villager and existing weapon. Intro rescue is encounter-driven; merchant hire costs 25 Gold and requires the docked/adjacent merchant flow. | Equipping a held weapon fits. Recruitment/hiring is not presented as an on-demand action when its encounter/location prerequisites are absent. |
| Upgrade sanctuary | Immediate event processing | Bound sanctuary, location within weak radius, and 3/6/9/12/15 Soulshards for the next level | Fits when shards and location already qualify. It also adds the existing +2 pressure per level; it is measured but omitted from the four guidance rows because Checkpoint 2 does not establish typical actionable access. |

With a hard 210-second floor and observed typical 259–300-second budget, the
server timings support at least two immediate/short actions when their
prerequisites are already present. Gathering, crafting infrastructure,
recruitment, or long travel can still force a choice. The synthetic paired
fixture confirms that multiple prepared launch facts can coexist, but it does
not prove an ordinary player completed acquisition/build/crafting end to end
inside the window. No phase timing change is proposed.

## Preparation Guidance

The v1 crisis snapshot gains an additive optional `preparation_options` vector.
Each row contains `id`, `label`, stable `state`, `detail`, and `action_hint`.
The field is omitted when it does not apply; packet version and all existing
fields remain unchanged.

The production collector is read-only and owner exact. It uses the mapped live
hero, completed owned structures, living owned villagers, and normal inventory
state. The four rows are fixed-order `defences`, `defenders`, `equipment`, and
`recovery`:

- Defences is ready for complete healthy walls, needs attention for a damaged
  wall with a living repair villager, or for an idle live hero with the actual
  Stockade plan and three carried Log-compatible units. Other cases name the
  precise blocker.
- Defenders is ready only for a combat-capable villager. It recommends equipping
  only when an idle unarmed villager already holds an unequipped weapon; a
  hero/storage weapon that still needs transfer is reported as blocked.
- Equipment is ready when the hero has a weapon and armor equipped. It
  recommends only a currently held idle equip or a normally transferable
  adjacent-storage item.
- Recovery recognizes the real Medical/Bandage fixed-heal path and positive
  Health-potion Healing attribute, while excluding Healing-tagged food. It is
  ready when carried and recommends only a currently valid normal transfer.

Delivery calls the collector only in `Preparing` and `AssaultReady`. All other
phases force the field absent. Existing successfully-queued snapshot caching
deduplicates equal fixed-order vectors; an option change is structural and
sends immediately without a transition Notice. Login/resume receives the
current complete vector. Legacy mode retains its no-crisis clear snapshot.

The TypeScript client treats the field as optional, validates nonempty strings
and the three stable states, rejects duplicate IDs/unknown states/malformed
rows, and examines at most the first four input rows. The existing crisis card
shows an in-card **Prepare your settlement** section only in Preparing/Ready.
Each row communicates `Ready`, `Needs attention`, or `Unavailable` in literal
text as well as color, followed by concise detail/action copy. Active assault
continues to show attacker count and disconnect warning instead. Existing
objective, compact expansion, accessibility, and Safe Logout state/control
paths are unchanged.

### Crisis-copy review

- `Signs` remains observational and uncertain: tracks and distant movement
  suggest attention without declaring an imminent attack.
- `Pressure` continues to connect settlement growth/activity to goblin
  attention without starting a countdown or emitting option-change notices.
- `Preparing` retains the gathering-raid identity and gains owner-specific
  weaknesses/actions through the structured rows.
- `AssaultReady` retains the return/final-preparation instruction and the
  Checkpoint 2 dusk-or-night preference with a finite fallback.
- `AssaultActive` keeps the remaining-attacker count and disconnect warning and
  suppresses all repair/crafting preparation rows.

No Signs/Pressure phase copy or transition identity was changed in this
checkpoint; detailed option changes remain panel state, not Notices.

## Telemetry

The opt-in Checkpoint 1 sampler retains every old field and adds:

- repair starts and completions;
- defensive-structure starts and completions;
- healing carried at launch and used before launch;
- combat-capable villagers at launch;
- the absolute tick of the first meaningful preparation action; and
- stable distinct categories (`defenses`, `equipment`, `healing`, `repair`,
  `sanctuary`, and `villager_support`) plus their count.

Private serde-skipped ID sets and high-water marks prevent repeated repair
orders/completions, equip toggles, item transfers, villager observations, and
sanctuary oscillation from inflating meaningful counts. Existing legacy
counters are updated through the same methods rather than incremented twice.
Actions are admitted only while the authoritative previous/current phase is
Preparing or AssaultReady. After a spawn succeeds and before the phase flips,
the opt-in sampler closes the final Ready interval so an action after the last
periodic sample is not lost; the following Active sample captures launch
readiness/outcomes without recounting it. Event hooks are preferred for repair
and actual successful healing use; bounded state deltas remain for structure,
equipment, inventory, villager, sanctuary, and location observations.

The normal headless runner preserves all prior CSV columns as an exact prefix
and appends the ten new serialized action fields. Nested JSON telemetry remains
available unchanged except for these additive fields.

## Paired Results

The final artifact is
`sp_server/goblin_crisis_balance_checkpoint3_pairs.json`, schema
`checkpoint3_preparation_pair_v1`. It contains 20 control/treatment pairs and
40 legs: five pair labels for each of Existing Walls, Equipment Prepared,
Healing Prepared, and Combined Preparation. The assault-relative cap was
15,000 ticks. All 20 pairs passed the selected observed-launch-field and
declared-fixture validation and produced quantitative deltas. The runner
retained every result: zero setup failures and zero caught panics. Thirty-eight
legs terminated because the hero reached True Death after two deaths. The
control and treatment of Healing Prepared `requested-pair-0004` both remained
alive with zero damage, zero deaths, zero kills, and three attackers remaining
at the 15,000-tick cap, so the artifact explicitly retains two
`timeout_unresolved` legs. Attempted engagement is not measured.

These are **not deterministic seeded pairs**. `requested-pair-0000` through
`requested-pair-0004` are stable pair labels, not RNG seeds. Control runs
precede treatment runs; production entropy is not replayed. The artifact says
`matched_observed_launch_fields: true`, `full_ecs_state_matched: false`, and
`random_stream_replayed: false`. The harness fixes the selected start,
progression facts, phase/pressure facts, declared fixture facts, and
post-launch attacker positions, but it cannot prove equality of hidden AI
state, effects, queued events, cooldowns, weather, villagers, or random draws.
Accordingly the deltas below are descriptive and cannot establish causation.
Overall direction ignores wall/structure damage (which can represent
absorption) and healing-path cumulative hero damage, but any remaining
guardrail regression makes a mixed pair `worsened`; metric ordering cannot hide
that regression.

Every leg, including every control, also receives one common synthetic,
unequipped Hide Wraps item. This keeps the equipment comparison's inventory
opportunity identical before its declared equipped-state difference, but means
"control" is a BasicSurvival policy on a staged shared inventory rather than an
untouched ordinary-start run. The same item is inert and equal in the wall and
healing comparisons; its presence is still a material synthetic setup
condition and is not evidence of ordinary acquisition during the warning.

### Feasible comparison subset

- Existing Walls adds one already-completed, ordinary Stockade at the declared
  anchor. It tests an attainable launch fact, not whether the player gathered,
  deposited, and built it inside the window.
- Equipment Prepared gives both legs a legitimately craftable Hide Wraps item;
  treatment equips it through the ordinary `PlayerEvent::Equip` path. That
  production action necessarily unequips the starting chest-slot Tattered
  Shirt, so the displaced shirt is an explicit validated consequence of the
  declared equipment difference rather than erased from the fingerprint.
- Healing Prepared directly installs one legitimately craftable Crude Bandage
  in treatment; treatment later consumes it once through the ordinary
  use-item event after the hero is wounded. Four of five healing treatments
  and all five combined treatments recorded that use. The capped healing
  Ranger was never wounded, so its bandage correctly remained unused.
- Combined Preparation applies all three declared differences.
- Repaired Defences, Villager Supported, and Sanctuary were not paired. They
  require additional pathing/AI/currency state, and this first matrix produced
  neither wall/structure damage or destruction nor one attacker defeat in any
  leg; wall contact was not measured. Adding those dimensions would not have
  produced an interpretable preparation-value signal before the first-assault
  outcome-range problem is addressed.

`completed_action`/the fixture record proves that the declared launch state was
present. It does not prove ordinary acquisition, crafting, building, or repair
completion during the warning window.

The generated JSON artifact is the per-leg raw result table required by the
matrix: each `.pairs[].control` and `.pairs[].treatment` object records
resolution, survival, hero damage/deaths, villager losses/damage, structure and
wall damage/destruction, attackers defeated, preparation completion, status,
and failures. All 40 `assault_duration_ticks` values are `null` because no
assault resolved; the `Ticks` column below is `observed_assault_ticks`, not a
resolved assault duration. The Markdown table focuses on paired deltas and
must be read with those retained raw legs.

### Per-label deltas

Every value is treatment minus control. `R`, `S`, `Deaths`, `Hero dmg`,
`Struct dmg`, and `Kills` mean resolution, survival, hero deaths, total hero
damage taken, total crisis-caused structure damage, and assault units defeated.
Villager losses and villager damage were also zero in every pair. `Ticks` is
descriptive observed assault time, not an accepted success metric under
unreplayed RNG.

| Comparison | Pair label | Class | R | S | Deaths | Hero dmg | Struct dmg | Kills | Ticks | Directional result |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| Existing Walls | requested-pair-0000 | Warrior | 0 | 0 | 0 | 0 | 0 | 0 | +72 | Unchanged |
| Existing Walls | requested-pair-0001 | Ranger | 0 | 0 | 0 | 0 | 0 | 0 | +64 | Unchanged |
| Existing Walls | requested-pair-0002 | Mage | 0 | 0 | 0 | 0 | 0 | 0 | +296 | Unchanged |
| Existing Walls | requested-pair-0003 | Warrior | 0 | 0 | 0 | 0 | 0 | 0 | +88 | Unchanged |
| Existing Walls | requested-pair-0004 | Ranger | 0 | 0 | 0 | 0 | 0 | 0 | -160 | Unchanged |
| Equipment Prepared | requested-pair-0000 | Warrior | 0 | 0 | 0 | 0 | 0 | 0 | +360 | Unchanged |
| Equipment Prepared | requested-pair-0001 | Ranger | 0 | 0 | 0 | 0 | 0 | 0 | -304 | Unchanged |
| Equipment Prepared | requested-pair-0002 | Mage | 0 | 0 | 0 | 0 | 0 | 0 | +336 | Unchanged |
| Equipment Prepared | requested-pair-0003 | Warrior | 0 | 0 | 0 | 0 | 0 | 0 | -120 | Unchanged |
| Equipment Prepared | requested-pair-0004 | Ranger | 0 | 0 | 0 | 0 | 0 | 0 | +72 | Unchanged |
| Healing Prepared | requested-pair-0000 | Warrior | 0 | 0 | 0 | +2 | 0 | 0 | -176 | Unchanged |
| Healing Prepared | requested-pair-0001 | Ranger | 0 | 0 | 0 | +4 | 0 | 0 | +192 | Unchanged |
| Healing Prepared | requested-pair-0002 | Mage | 0 | 0 | 0 | +2 | 0 | 0 | -200 | Unchanged |
| Healing Prepared | requested-pair-0003 | Warrior | 0 | 0 | 0 | +2 | 0 | 0 | +64 | Unchanged |
| Healing Prepared | requested-pair-0004 | Ranger | 0 | 0 | 0 | 0 | 0 | 0 | 0 | Unchanged; both capped |
| Combined Preparation | requested-pair-0000 | Warrior | 0 | 0 | 0 | +1 | 0 | 0 | +16 | Unchanged |
| Combined Preparation | requested-pair-0001 | Ranger | 0 | 0 | 0 | +3 | 0 | 0 | +176 | Unchanged |
| Combined Preparation | requested-pair-0002 | Mage | 0 | 0 | 0 | +3 | 0 | 0 | -72 | Unchanged |
| Combined Preparation | requested-pair-0003 | Warrior | 0 | 0 | 0 | +2 | 0 | 0 | +328 | Unchanged |
| Combined Preparation | requested-pair-0004 | Ranger | 0 | 0 | 0 | +4 | 0 | 0 | -24 | Unchanged |

### Aggregate deltas

All directional outcome metrics—resolution, survival, hero deaths, villager
losses/damage, and attackers defeated—had mean and median delta zero for every
comparison. Structure and wall damage were also zero in every leg. Damage to
an added wall is descriptive by design because absorption could be beneficial;
here no added wall was damaged at all. Hero damage is descriptive for healing
comparisons because a consumed heal increases the amount of damage a hero can
absorb before the same terminal outcome.

| Comparison | Pairs | Major outcomes mean / median | Descriptive hero-damage mean / median | Descriptive tick mean / median | Improved / unchanged / worsened |
|---|---:|---|---|---|---|
| Existing Walls | 5 | 0 / 0 | 0 / 0 | +72.0 / +72 | 0 / 5 / 0 |
| Equipment Prepared | 5 | 0 / 0 | 0 / 0 | +68.8 / +72 | 0 / 5 / 0 |
| Healing Prepared | 5 | 0 / 0 | +2.0 / +2 | -24.0 / 0 | 0 / 5 / 0 |
| Combined Preparation | 5 | 0 / 0 | +2.6 / +3 | +84.8 / +16 | 0 / 5 / 0 |

All 40 legs recorded zero attackers defeated, three attackers remaining, and
zero resolution. Thirty-eight recorded two hero deaths and no terminal
survival; the two capped Ranger legs recorded zero deaths and were alive but
unresolved. The positive healing damage deltas show only that a consumed
bandage supplied a small additional damage buffer; they did not change a
declared directional outcome.

### Class results

The matrix contains eight Warrior pairs, eight Ranger pairs, and four Mage
pairs (16/16/8 legs). Every class recorded zero directional improvements,
zero resolutions, and zero attackers defeated. One Ranger pair had both legs
survive unresolved to the cap; every other leg reached True Death. The
descriptive mean hero-damage deltas were +0.875 Warrior, +1.375 Ranger, and
+1.25 Mage; mean observed-tick deltas were +79, +2, and +90 respectively. The
uneven, very small samples and unreplayed RNG make those descriptive values
unsuitable for class tuning. No class demonstrated a preparation benefit.

## Changes Implemented

No gameplay value or simulation behavior was changed. The accepted
Checkpoint 3 implementation consists of:

- the optional owner-only preparation-options schema and server-authoritative
  read-only collector;
- the desktop crisis-card preparation section;
- additive, idempotent preparation-action telemetry and append-only normal
  runner columns; and
- the bounded observed-launch-field comparison harness and generated raw JSON
  artifact.

Pressure weights and thresholds, phase minima/windows, launch behavior,
assault composition, enemy stats, spawn/targeting rules, repair/build/craft/
heal/equip values, villager AI, sanctuary values, Safe Logout, ordinary
disconnect behavior, and every resource/production system remain unchanged.

### Accepted gameplay changes

Zero. There is therefore no old/new gameplay value table. The evidence did not
justify a safe preparation-value change before first-assault tuning, and this
checkpoint does not smuggle Checkpoint 4 assault changes into production.

## Changes Reverted

No gameplay experiment was implemented, so no production gameplay change was
reverted. The four no-tuning hypotheses were measured and failed their
predeclared benefit bands: existing wall, equipped Hide Wraps, one consumed
Crude Bandage, and their combination each produced 0/5 improved pairs. The
runner's early development cap/death-stop behavior was corrected as an
instrumentation defect before the final matrix; it was not a gameplay
experiment.

The proposal's rollback wording said a path failing usefulness should be
removed or revised. The factual guidance rows are retained as a documented
deviation because the user-facing requirement is to expose current settlement
weaknesses and actionability, not to promise a combat buff. Copy reports what
exists and grants no benefit. The failed usefulness hypothesis blocks gameplay
acceptance and is explicit below; it is not presented as proof that the
guidance action improves survival.

## Outcome

1. **Can a typical player complete at least two useful preparation actions?**
   Timing arithmetic supports two immediate/short actions when prerequisites
   already exist, and the combined synthetic launch fixture contains multiple
   prepared facts. Ordinary end-to-end acquisition and, more importantly,
   usefulness were not demonstrated. The strict answer is therefore not yet.
2. **Which preparation paths demonstrably help?** None of the four tested paths
   improved a declared directional outcome.
3. **Which systems still provide little value?** Existing Walls, Hide Wraps,
   one Crude Bandage, and their combination provided no measured major benefit
   in this fixture. Repair, villager support, and sanctuary remain untested by
   the paired subset.
4. **Is one path dominant?** No. All four were directionally unchanged.
5. **Does prepared solo materially outperform control?** No; combined
   preparation was 0 improved, 5 unchanged, 0 worsened.
6. **Did an accepted change trivialize the assault?** No gameplay change was
   accepted, and no treatment resolved even one assault.
7. **Can Warrior benefit?** No demonstrated benefit in eight pairs.
8. **Can Ranger benefit?** No demonstrated benefit in eight pairs.
9. **Can Mage benefit?** No demonstrated benefit in four pairs.
10. **What remains for Checkpoint 4?** First create measurable dynamic range in
    first-assault lethality/outcomes: 38/40 legs died twice and the other two
    took zero damage and remained unresolved at the cap; no leg killed one of
    three attackers.
    Then measure target/path contact and core exposure so walls can be
    evaluated, revisit existing healing/equipment magnitude, and validate
    Warrior/Ranger/Mage combat policy and outcomes. Seeded replay or a
    controlled simulation RNG is a separate prerequisite for causal A/B
    claims. Checkpoint 4 must preserve ownership, Safe Logout, offline rules,
    economy, and unchanged pacing unless separately evidenced.

### Formal definition-of-done status

The guidance, telemetry, UI, runner, raw matrix, and no-scope-creep deliverables
are implemented. The formal balance acceptance is **not complete**:
requirement 12 is supported only by timing analysis/synthetic launch facts,
requirement 13 (two beneficial paths) is false, requirement 14 (prepared solo
materially outperforms control) is false, and requirement 26 is blocked because
the required unskipped TypeScript check fails on the repository declaration
baseline. Identical-seed replay was also not achievable without an out-of-scope
RNG refactor.

| DoD | Status | Evidence / limitation |
|---:|---|---|
| 1 | Met | Checkpoint 1/2 source, reports, and artifacts were reviewed. |
| 2 | Partial / not fully met | Medians, configured floor, action timings, prerequisites, and conditional travel examples are recorded, but overwritten Checkpoint 2 rows prevent exact observed total-window extrema and ordinary resource/travel availability reconstruction. |
| 3–11 | Met | Zero gameplay proposals/acceptances remain within the cap; success/rollback rules are recorded; guidance is owner-authoritative, non-mutating, and displayed in the desktop crisis card. |
| 12 | Not demonstrated | Timing supports short actions, but ordinary end-to-end completion of two **useful** actions and exact observed total-window extrema are unavailable. |
| 13 | Not met | Zero of four tested paths met the paired benefit band. |
| 14 | Not met | Combined preparation was 0 improved / 5 unchanged / 0 worsened. |
| 15–25 | Met | No gameplay value was accepted; focused/full server regressions preserve pacing, composition, stats, spawn, Safe Logout, disconnect, ownership, and economy. |
| 26 | Blocked / not met | `npx tsc --noEmit` fails on duplicate Phaser declarations and missing Matter types; focused/skip-lib-check compile and webpack pass. |
| 27–29 | Met with replay deviation | The 20-pair matrix completed and the report documents results/Checkpoint 4 focus, but the pairs are selected-field matched rather than deterministic seeded replay. |

The branch must not be called Checkpoint 3 balance-acceptance complete.

## Regression and Validation Record

This section is finalized from commands executed on the completed tree. A
failed command remains listed even when a later focused or full rerun passes.

### Server checks

- `cargo fmt --check` — passed.
- `cargo check` — passed; 70 existing compiler warnings.
- First `cargo test` — failed with 438 passed and 1 failed. The failure was
  `safe_logout_checkpoint2_global_time_visibility_and_world_packets_continue`:
  its visibility comparison sampled `3 == 3`. Its exact isolated rerun passed
  1/1, identifying an existing random/order-sensitive assertion rather than a
  repeatable Checkpoint 3 failure.
- Final `cargo test` rerun — passed: 444/444 library tests, 17/17 normal-runner
  tests, 4/4 paired-runner tests, 6/6 day-system integration tests, zero-test
  main/legacy integration targets, and one ignored doc test.
- `cargo clippy --all-targets --all-features` — passed with no errors; 1,334
  library warnings, one normal-runner warning, three normal-runner-test warnings
  (one duplicate), and 1,351 lib-test warnings (1,334 duplicates) from the
  repository's existing lint baseline plus test-style lints.
- `cargo test checkpoint3_ --lib` — passed 48/48 after the final telemetry and
  guidance regressions were added.
- `cargo test crisis_balance::tests --lib` — passed 15/15.
- `cargo test safe_logout --lib` — passed 64/64, including the visibility test.
- `cargo test personal_crisis --lib` — passed 7/7.
- `cargo test goblin_balance_checkpoint2 --lib` — passed 2/2.
- `cargo test --bin preparation_pair_runner` — passed 4/4.
- `cargo test --bin headless_runner` — passed 17/17.
- `cargo test preparation_pair --lib -- --nocapture` — passed 5/5 after exact
  inventory/Stockade fingerprint hardening.

### Headless checks

- `cargo run --release --bin preparation_pair_runner -- --pairs 5 --output
  goblin_crisis_balance_checkpoint3_pairs.json` — completed 20 pairs/40 legs;
  all 20 fingerprints valid, all 20 quantitative, zero setup failures or caught
  panics, 38 `hero_dead` legs, and two retained `timeout_unresolved` legs at the
  15,000-tick cap.
- `cargo run --release --bin headless_runner -- 1 1000 standard` — completed
  one row: zero panics, zero automatic dusk waves, zero crisis invariant
  failures.
- `cargo run --release --bin headless_runner -- 8 2000
  safe-logout-matrix` — completed and retained all eight rows, including two
  pre-existing random `Cannot find item template: "Windstride Stag"` gather
  panics. The six non-panic rows exercised accepted/completed/cancelled Safe
  Logout, ordinary disconnect, active-assault rejection/disconnect, long
  protection/resume, and reported zero automatic dusk waves. Because the
  process catches row panics and exits zero, this is not reported as an
  eight-row gameplay pass; the focused 64-test suite is the clean regression
  result.
- The current-side Checkpoint 2 matrix was rerun with the already-selected
  candidate constants through `env
  CARGO_MANIFEST_DIR=/Users/peter/projects/sp/sp_server
  target/release/headless_runner 39 20000 goblin-balance candidate`: 39/39 quantitative
  rows, 13 each Warrior/Ranger/Mage, 18 natural and 21 staged. Scenario counts
  were Basic Survival 3, Passive 3, Helper Supported 3, Ordinary Disconnect 3,
  Safe Logout Before Assault 3, Prepared Solo 6, Fortified Solo 6, No
  Villagers 6, and Villager Supported 6. It retained 27 tick-cap rows and 23
  unresolved-at-cap rows, launched 21 assaults, resolved 5, and recorded zero
  panics, automatic dusk hordes, duplicate assaults, cross-player target
  violations, crisis invariant failures, or Safe Logout invariant recoveries.
  Three Safe Logout and three ordinary/active disconnect cases completed. The
  historical Checkpoint 2 control artifact was reviewed rather than relabeled:
  this binary correctly refuses to call candidate thresholds `control`.
- The first isolated direct-binary attempt,
  `target/release/headless_runner 39 20000 goblin-balance candidate`, lacked
  runtime `CARGO_MANIFEST_DIR` and retained 39/39 setup panics; it was corrected
  by setting the exact manifest path and rerun rather than excluded.
- The ordinary disconnect/helper/cross-owner isolation headless tests are part
  of the 48-test `checkpoint3_` pass, including offline assault continuation,
  connected-helper resolution, helper-kill ownership, and rejection of stale
  cross-owner action targets.
- No dedicated adjacent-settlement scenario was run in this checkpoint (zero
  samples). The cross-owner target tests prove owner isolation but are not a
  substitute for the requested adjacent-settlement balance scenario.

### Client checks

- `npx tsc --noEmit` — failed on the repository's duplicate Phaser declarations
  with exit 2 and exactly 12 diagnostics: `TS6200` x2, `TS2432` x9, and
  `TS2688` x1 (missing `./matter`) between
  `node_modules/phaser/types/phaser.d.ts` and `src/phaser.d.ts`. No
  application-source diagnostic class appeared. This minimum command is not
  claimed as passed.
- `npx tsc --noEmit --skipLibCheck` — passed with exit 0 and no output.
- `npx webpack --mode production` — passed with exit 0 for desktop and mobile.
  Desktop emitted `sp2.desktop.js` (3.36 MiB) with three performance warnings;
  mobile emitted `sp2.mobile.js` (2.42 MiB) with the same three warning classes
  (asset size, entrypoint size, and code-splitting recommendation).
- `mktemp -d /tmp/sp_checkpoint3_frontend_tests.XXXXXX` created the fresh
  output directory `/tmp/sp_checkpoint3_frontend_tests.mvIgPF`.
- Focused compile — passed with exit 0:
  `npx tsc --module commonjs --target es2020 --jsx react --esModuleInterop
  --skipLibCheck --sourceMap false --outDir
  /tmp/sp_checkpoint3_frontend_tests.mvIgPF src/phaser.d.ts
  src/sp/core/crisisStatus.test.ts
  src/sp/desktop/ui/objectivesPanel.crisis.test.tsx
  src/sp/desktop/ui/objectivesPanel.safeLogout.test.tsx`.
- Focused execution — passed 3/3 scripts with exit 0:
  `NODE_PATH=/Users/peter/projects/sp/sp_frontend/sp_ts/node_modules node -e
  "require('/tmp/sp_checkpoint3_frontend_tests.mvIgPF/core/crisisStatus.test.js');
  require('/tmp/sp_checkpoint3_frontend_tests.mvIgPF/desktop/ui/objectivesPanel.crisis.test.js');
  require('/tmp/sp_checkpoint3_frontend_tests.mvIgPF/desktop/ui/objectivesPanel.safeLogout.test.js');"`.
  The scripts printed `crisisStatus helper checks passed`, `ObjectivesPanel
  crisis countdown checks passed`, and `ObjectivesPanel Safe Logout component
  checks passed`.
- No development server was started.

Two non-final frontend invocations were corrected and are not called passes:
the first Node command used the wrong emitted `/tmp` subpath, and the second
lacked `NODE_PATH` for React resolution. Neither changed source.

## Known Limitations

- The comparison is neither same-seed nor full-ECS matched. Sequential
  control-first ordering is not counterbalanced.
- The selected fingerprint omits hidden attacker AI/path/cooldown/effect and
  queued-event state, and treatment attacker positions are normalized after
  production launch. It otherwise compares all inventory and all structures,
  normalizing only the exact comparison-specific Stockade, Hide Wraps plus its
  displaced Tattered Shirt, and Crude Bandage facts.
- The synthetic fixture jumps crisis/time and directly installs completed
  launch facts. It measures launch-state effects, not economic opportunity cost
  or end-to-end player preparation feasibility.
- The synthetic Stockade is spawned directly as a completed blocking wall and
  does not exercise the production foundation/build/`NewObj` observer path.
  No unit occupies its tile, so the omitted occupant-fortification observer is
  not part of this comparison, but the fixture is not proof of end-to-end wall
  construction behavior.
- Five pair labels per comparison yield only two Warrior, two Ranger, and one
  Mage pair within each comparison.
- The bot's BasicSurvival combat remains melee-biased and is not a human class
  skill study.
- No wall/structure damage or destruction was recorded. Wall contact,
  first-contact delay, absorption, and core exposure were not measured.
  Observation duration cannot substitute for those metrics under unreplayed
  RNG.
- Repair, villager support, and sanctuary were not paired.
- The known starting Health Potion runtime/template potency and consumption
  discrepancy remains unchanged.
- The desktop receives preparation presentation; the existing mobile surface
  still lacks crisis-status presentation. No manual human-comprehension study
  was run.
- No dedicated adjacent-settlement matrix sample was executed; only focused
  cross-owner target/isolation regressions ran.
- The first broad Rust run and required unskipped TypeScript run have the exact
  limitations recorded above.

## Manual Playtest Checklist

This checklist is supplied for a human playtest; it was not manually executed
in this automated pass.

1. Progress an ordinary owner through `Preparing`; verify the crisis card shows
   no more than four factual rows in Defences, Defenders, Equipment, Recovery
   order and that another player's assets do not affect them.
2. Damage/repair a wall, equip an item, transfer/carry a usable heal, and arm a
   villager where possible; verify each changed fact updates once without
   packet spam, free resources, pressure change, or hidden buff.
3. Continue to `AssaultReady`; verify the rows remain, the copy communicates
   that dusk/night is preferred but launch will not wait indefinitely, and the
   existing countdown/phase pacing is unchanged.
4. Continue to `AssaultActive`; verify the preparation section disappears,
   attacker count/disconnect warning appears, and Safe Logout remains rejected
   under the existing rules.
5. Reconnect once in Preparing/Ready and once during the assault; verify the
   owner gets one current status, ordinary disconnect behavior continues, and
   helper/adjacent settlements remain isolated.
