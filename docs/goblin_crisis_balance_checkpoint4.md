# Goblin Crisis Balance — Checkpoint 4

Status: diagnostic implementation and final validation are complete, but the
balance acceptance gate failed. Three predeclared production candidates were
tested and reverted, no fourth speculative change was made, and the final branch
uses the original assault unchanged. The target bands below were declared after
the corrected, unchanged-production baseline and before any numerical,
composition, spawn, or AI tuning. Checkpoint 4 and Milestone 3 remain open because
the restored configuration does not provide a prepared victory path for every
class or the required preparation ordering.

## Diagnostic provenance

The branch was `goblin_crisis_balance_milestone` at commit
`db6cf6490f27adeb65787a55c791c525ae5efc14` (`Completed Milestone 3 checkpoint 3`).
The working tree was clean before either reproduction. Both binaries were built in
the release profile with:

```bash
cd /Users/peter/projects/sp/sp_server
cargo build --release --bin headless_runner --bin preparation_pair_runner
```

The first isolated-cwd preflight failed before producing a valid row because the
runtime opens `templates/player_start.yaml` relative to the process working
directory even when `CARGO_MANIFEST_DIR` is set. Every attempted row reported the
same panic. The isolated diagnostic directory was therefore given a `templates`
symlink to the repository templates before the bounded rerun. This preflight is a
harness/runtime-layout finding, not a gameplay result.

The effective isolated setup was:

```bash
cd /private/tmp/sp_cp4_diagnostic.evnefG
ln -s /Users/peter/projects/sp/sp_server/templates templates
```

### Checkpoint 2-style reproduction

From the isolated directory, with the repository templates available:

```bash
env CARGO_MANIFEST_DIR=/Users/peter/projects/sp/sp_server \
  /Users/peter/projects/sp/sp_server/target/release/headless_runner \
  39 20000 goblin-balance candidate
```

Raw artifact:
`sp_server/goblin_crisis_checkpoint4_diagnostic_headless.json`.
Aggregate copy:
`sp_server/goblin_crisis_checkpoint4_diagnostic_headless_aggregate.json`.

This was 39 rows at a 20,000-tick total-run cap: 13 rows per class. It covered
`basic_survival`, `passive`, `prepared_solo`, `fortified_solo`, `no_villagers`,
`villager_supported`, `helper_supported`, `ordinary_disconnect`, and
`safe_logout_before_assault`. Twenty-seven rows launched an assault, 38 attributed
assault units died, six assaults resolved, heroes recorded 21 ordinary deaths,
three rows ended in True Death, 36 reached the total-run cap, and no row panicked.
The runner did not retain panic payloads, so a future panic would not have been
classifiable from this schema.

This sample is not a staged Basic Survival assault sample. Basic Survival is a
natural-progression variant and none of its three rows launched. The resolved rows
came from other, staged policies after roughly 4,800 ticks of ordinary online phase
progression. The runner cap begins at hero creation, does not stop at crisis
resolution, and may stop for unrelated global victory.

### Checkpoint 3 pair reproduction

From the same isolated directory:

```bash
env CARGO_MANIFEST_DIR=/Users/peter/projects/sp/sp_server \
  /Users/peter/projects/sp/sp_server/target/release/preparation_pair_runner \
  --pairs 5 --assault-cap-ticks 15000 \
  --output /Users/peter/projects/sp/sp_server/goblin_crisis_checkpoint4_diagnostic_pairs.json
```

The artifact contains 20 requested pairs and 40 legs: five pairs each for Existing
Walls, Equipment Prepared, Healing Prepared, and Combined Preparation. It contains
eight Warrior pairs, eight Ranger pairs, and four Mage pairs. All 40 legs launched,
all ended in terminal True Death after two ordinary deaths, zero assault attackers
died, zero assaults resolved, and no leg timed out, panicked, or failed setup. The
pair labels are not RNG seeds, random streams were not replayed, and hidden ECS
state was not matched.

The rerun differs from the earlier Checkpoint 3 artifact's 38 True Deaths and two
Ranger tick-cap outcomes. That variance is expected from entropy-backed production
RNG and reinforces that sequential pairs are descriptive evidence rather than
causal trials.

## Architecture and engagement pipeline

The current branch was inspected directly in all requested areas:
`sp_server/src/game.rs`, `game_tests.rs`, `crisis_balance.rs`, `headless.rs`,
`headless_bot.rs`, `bin/headless_runner.rs`,
`bin/preparation_pair_runner.rs`, `combat.rs`, `encounter.rs`, `player.rs`,
`obj.rs`, `structure.rs`, `item.rs`, `templates.rs`, `constants.rs`,
`ai/npc/npc.rs`, and `ai/villager/villager.rs`, plus the loaded player, item,
object, and NPC templates. The audit covered class policies, hero and NPC
movement, perception and target acquisition, attack request/acceptance, cooldown,
hit/damage/death boundaries, resurrection and True Death, needs and ambient
deaths, assault/anchor geometry, stop conditions, disconnect/reconnect, helpers,
corpses, loot, and the exact three-unit composition.

- Personal crisis launch is server-authoritative. It creates two Wolf Riders and
  one Goblin Pillager through the ordinary encounter spawn path, gives each a
  `CrisisAssaultUnit` owner/assault/generation attribution, and installs the normal
  NPC brain plus a 14-tile viewshed. Resolution scans the attributed generation and
  requires ordinary death evidence; cleanup paths deliberately do not resolve.
- Generic NPC scoring can select only the owning hero and villagers. Ordinary
  structures are excluded. A cunning attacker can select an owning wall only when
  that wall blocks its path to an eligible unit. There is no production core-target
  concept and no fallback order to march on a settlement anchor when no unit target
  is visible.
- NPC target choice is refreshed on a cadence, then `NpcMoveToTarget` validates and
  schedules ordinary movement. `AttackTarget` validates owner and protection state
  before calling the shared combat mutation. Exact request and acceptance are
  separate boundaries.
- Hero combat first enters the server as `PlayerEvent::Attack` or
  `PlayerEvent::Ability`; there is no authoritative selected-target component.
  Ranger normal shots have range and a hit roll; Aimed Shot and Mage Arcane Bolt are
  ordinary supported ability paths. Damage converges on shared combat mutation and
  existing crisis attribution events.
- Villagers do not proactively acquire attackers. Armed retaliation requires a
  recent attacker, a weapon, and a valid adjacent opportunity.
- Ordinary hero death leaves the active assault intact and permits sanctuary
  resurrection. The second death can become True Death. Existing `StateDead` and
  `LastAttacker` data can distinguish an attributed assault killer from ambient and
  needs-related death; template text alone is insufficient.
- The current opt-in balance sampler is disabled unless a headless sampling
  interval is configured. Extending that sampler and exact combat boundaries can
  collect bounded first-event timestamps/counters without changing production
  behavior or logging every tick.

### Exact affected files

The final Checkpoint 4 working tree affects the following implementation source
files; this is not a prospective list:

- `sp_server/src/crisis_balance.rs`: additive opt-in engagement, wall/core,
  healing, defeat, resolution, and cleanup telemetry plus unit tests.
- `sp_server/src/combat.rs`: exact hit/damage/death telemetry boundaries and
  lethal-NPC thinker removal.
- `sp_server/src/player.rs`: class attack/ability telemetry, consumable
  completion semantics, and exact personal-assault ownership checks.
- `sp_server/src/ai/npc/npc.rs`: NPC request/acceptance telemetry, dead/stale
  action and missing-event-boundary guards, current-position melee validation,
  and personal wall/owner targeting safety.
- `sp_server/src/ai/villager/villager.rs` and
  `sp_server/src/ai/villager/villager_tests.rs`: fallible pre-mutation guards for
  orphaned villager actions, fail-closed vital-needs scorers, synchronous stale
  event cancellation, and movement/combat/needs/shelter regressions.
- `sp_server/src/game.rs` and `sp_server/src/game_tests.rs`: personal-assault
  spawn isolation, full-enclosure wall handling, battle-copy correction,
  resolution/cleanup observations, pre-mutation drink/eat/sleep/shelter event
  validation, and focused production regressions.
- `sp_server/src/headless.rs` and `sp_server/src/headless_bot.rs`: corrected
  fixtures, expanded launch fingerprints, class-valid policies, exact world
  observations, edge scenarios, and harness tests.
- `sp_server/src/bin/headless_runner.rs` and
  `sp_server/src/bin/preparation_pair_runner.rs`: retained panic/setup evidence
  and corrected runner plumbing.
- `sp_server/src/bin/goblin_crisis_checkpoint4_runner.rs` and
  `sp_server/src/lib.rs`: the new protected-output Checkpoint 4 matrix runner and
  its library exposure.
- `docs/goblin_crisis_balance_checkpoint4.md` and
  `docs/goblin_crisis_balance_milestone.md`: evidence, decisions, status, and
  deferred contract.

The exact generated Checkpoint 4 evidence files retained in the working tree are:

- `sp_server/goblin_crisis_checkpoint4_diagnostic_headless.json`
- `sp_server/goblin_crisis_checkpoint4_diagnostic_headless_aggregate.json`
- `sp_server/goblin_crisis_checkpoint4_diagnostic_pairs.json`
- `sp_server/goblin_crisis_checkpoint4_corrected_headless.csv`
- `sp_server/goblin_crisis_checkpoint4_corrected_headless.json`
- `sp_server/goblin_crisis_checkpoint4_corrected_headless_aggregate.json`
- `sp_server/goblin_crisis_checkpoint4_corrected_pairs.json`
- `sp_server/goblin_crisis_balance_checkpoint4_corrected_baseline.json`
- `sp_server/goblin_crisis_balance_checkpoint4_corrected_current_final.json`
- `sp_server/goblin_crisis_balance_checkpoint4_corrected_current_final_harness.json`
- `sp_server/goblin_crisis_balance_checkpoint4_corrected_preparation_final.json`
- `sp_server/goblin_crisis_balance_checkpoint4_corrected_preparation_final_harness.json`
- `sp_server/goblin_crisis_balance_checkpoint4_policy_v3_current_smoke.json`
- `sp_server/goblin_crisis_balance_checkpoint4_candidate1.json`
- `sp_server/goblin_crisis_balance_checkpoint4_candidate1_preparation.json`
- `sp_server/goblin_crisis_balance_checkpoint4_candidate2_smoke.json`
- `sp_server/goblin_crisis_balance_checkpoint4_candidate2_preparation_smoke.json`
- `sp_server/goblin_crisis_balance_checkpoint4_candidate3_smoke.json`
- `sp_server/goblin_crisis_balance_checkpoint4_candidate3_preparation_smoke.json`
- `sp_server/goblin_crisis_balance_checkpoint4_candidate3_policy_v4_smoke.json`
- `sp_server/goblin_crisis_balance_checkpoint4_candidate3_policy_v4_preparation_smoke.json`
- `sp_server/goblin_crisis_balance_checkpoint4_prefinal_race_failure.json`
- `sp_server/goblin_crisis_balance_checkpoint4_pre_event_boundary_final.json`
- `sp_server/goblin_crisis_balance_final.json`

The two `*_final_harness.json` artifacts are earlier immutable exploratory runs
under the distinct `release-corrected-current-final-harness` label (100 broad
rows: 9 resolutions; 120 focused legs: 0 resolutions). They were superseded by
the Ranger-fixed decision baseline below, are retained so adverse evidence is not
discarded, and are not pooled into any reported decision or acceptance result.

`goblin_crisis_balance_checkpoint4_pre_event_boundary_final.json` is the earlier
260-row final (19 resolutions, 236 True Deaths, four caps, one unrelated panic)
generated before the generalized NPC/villager `EventExecuting` hardening. It is
retained for provenance, superseded by `goblin_crisis_balance_final.json`, and is
not pooled into the final results below.

No object/item template, resource, recipe, structure, farm, trade, database, map,
world-clock, network-schema, or frontend file is changed. In particular,
the shared Wolf Rider and Goblin Pillager template values and the full production
economy remain unchanged.

## Harness reconciliation

| Dimension | Checkpoint 2-style runner | Checkpoint 3 pair runner | Diagnostic consequence |
|---|---|---|---|
| Progression | Natural or staged, then real phase ticks | Injects phases and jumps directly to dusk | Not the same launch history |
| Basic Survival | Natural only; normally no staged launch | Used for every directly launched leg | Resolved CP2 rows are not comparable controls |
| Starting healing | Keeps the starting Health Potion | Removes every ordinary healing item; treatment gets one bandage | Large survivability difference |
| Potion semantics | Bot reuses it whenever low | Potion absent | Production potion heals without consuming quantity |
| Class policy | Scenario-specific but class-blind combat | Always class-blind Basic Survival | Ranger and Mage tools are not exercised |
| Attack behavior | Adjacent quick attack only | Adjacent quick attack only | Bow range, spells, and class abilities are ignored |
| Pursuit | Enemy considered only within radius three | Same | A stalled/distant assault can be ignored forever |
| Wall | Scenario-driven production state | Direct-spawned at `(15,13)` | Pair wall overlaps the fixed corpse tile |
| Ambient cleanup | Ordinary world lifecycle | Sets NPCs dead in place | Dead positioned actors remain bot path blockers |
| Assault geometry | Production spawn positions | Treatment positions normalized after launch | AI/pending state is not normalized with position |
| Cap | From hero creation | Assault-relative | Exposure duration differs |
| Stop | True Death, victory, or total cap; not resolution | Resolution, True Death, or assault cap | Outcome cohorts differ |
| Panic data | Payload discarded | Payload retained | Normal-runner failures cannot be classified |

The pair's declared fingerprint also omits needs, effects, stamina, mana, cooldowns,
selected policy target, AI action state, pending events, deferred commands, and RNG
state. Control always runs first. Those facts do not invalidate its recorded facts,
but they prohibit deterministic or causal claims.

### Final field-by-field Phase A setup comparison

The table above describes the reproduced Checkpoint 3 harness. After correction,
the dedicated Checkpoint 4 fixture is the common launch path for broad and focused
workloads. The final exact audit is:

| Required field | Reproduced difference / final handling |
|---|---|
| Hero starting position | The old pair mutated treatment geometry after launch. The corrected fixture finds a clear six-tile ring before launch and places the hero at its center; control and treatment record the exact position, while entropy-selected attacker geometry remains separately reported. |
| Hero state and health | Both are now exact common-fingerprint fields. The final fixture requires a live `State::None` hero at full class-template HP; treatment setup cannot normalize either away. |
| Hero needs | The old fingerprint omitted them. Final thirst, hunger, tiredness and all three per-tick rates are compared bit-for-bit. |
| Hero effects | The old fingerprint omitted them. Final effect name, duration/deadline, amplifier bits, and stack count are sorted and compared exactly. |
| Equipment and inventory | The old pair deleted all ordinary healing items and treatment added one bandage. The corrected fixture retains the starting potion, changes only the declared armor/bandage artifact, and compares every unrelated item, quantity, slot, and equipped flag. |
| Cooldowns | The production player attack deadline is a system-local `HashMap` and is not queryable from `World`; Checkpoint 4 does not pretend otherwise. Accessible `LastCombatTick` is compared exactly for the hero and each assault unit, and every combat command still passes through the production 50-tick cooldown. |
| Selected target | There is no authoritative player selected-target component. The bot's local policy target is intentionally not claimed as matched; target changes and accepted production commands are observed during play. NPC `VisibleTarget`, `Target`, and `TaskTarget` are inspected at the launch/action boundaries. |
| Current activity/state | Hero ECS `State` is exact. Bot-local cadence and Big Brain action entities are not a full-ECS match; the corrected fixtures launch before bot combat and validate normal production events after launch. |
| Bot policy | Checkpoint 3 used adjacent quick attacks for every class. Final Warrior uses its equipped spear and Guard Bash, Ranger its Training Bow/Aimed Shot/Disengage plus legal cooldown repositioning, and Mage Arcane Bolt/Ward, all through ordinary `PlayerEvent` paths. |
| Crisis phase, pressure, and clocks | Exact common fields include `AssaultActive`, pressure, online-active ticks, phase-online ticks, launch tick, and world tick. Both legs use the same staged authoritative transition. |
| Assault positions | The old treatment rewrote positions post-launch without resetting AI. Final production spawn positions are never rewritten. Geometry is reported separately because production RNG is not replayed. |
| Assault health and combat state | Final fingerprints compare complete `Stats`, effects, and `LastCombatTick` for every live attributed unit. A focused regression compares those values with the loaded production templates. |
| Assault AI and viewshed | Every launch must have the ordinary thinker, configured 14-tile personal viewshed, live state, unique passable tile, and exact owner/assault/generation attribution. No read-only reporting system spawns an actor. |
| Settlement anchor | Both paths bind the same fixed start location and exact owner sanctuary/anchor. The launch report records the resulting anchor position. |
| Walls | The old single wall overlapped the fixed corpse. Final treatment creates six real, live 20-HP Stockades around the hero before launch; only those declared wall artifacts are normalized out of the common fingerprint. |
| Villagers | Basic/prepared controls use zero living villagers; supported workloads spawn the declared existing villager fixture. Counts, contribution, damage, and losses remain measured rather than normalized into a hidden bonus. |
| Weather and time of day | Global weather/day systems are unchanged. The exact world tick is matched; the harness does not override production visibility or weather combat values. |
| Pending events and deferred commands | They are not fully enumerable or matched. Fixture operations flush at explicit setup boundaries, then launch and play through ordinary schedules; the report keeps `full_ecs_state_matched: false`. |
| Tick advancement | Old pair jumped to dusk while broad scenarios advanced differently. Final direct workloads use the same authoritative launch tick and an assault-relative 15,000-tick cap, sampled every tick. |
| Stop conditions | Final rows stop only on resolution, attributable True Death/missing hero, explicit scenario completion, retained panic/setup failure, or the assault-relative cap. An ordinary first death and inactivity do not terminate a row. |
| RNG | Production `thread_rng` remains entropy-backed and unreplayed. Run labels are not seeds; controls/treatments are counterbalanced descriptive observations, never deterministic causal pairs. |

The final fingerprint is deliberately stronger than Checkpoint 3 but is still not
a full ECS or RNG snapshot. That limitation is serialized in every comparison and
is why this report uses “directional” and “descriptive,” not “causal.”

### Corrected Phase A reruns

After the first harness and class-policy corrections, but before the later v2
one-tick sampler, six-Stockade fixture, and final launch fields, the unchanged
production assault was rerun with the same bounded workloads:

```bash
cd /Users/peter/projects/sp/sp_server
env CARGO_MANIFEST_DIR="$PWD" \
  target/release/headless_runner \
  39 20000 goblin-balance candidate

env CARGO_MANIFEST_DIR="$PWD" \
  target/release/preparation_pair_runner \
  --pairs 5 --assault-cap-ticks 15000 \
  --output goblin_crisis_checkpoint4_corrected_pairs.json
```

The normal runner's CSV, raw JSON, and aggregate output are retained as
`goblin_crisis_checkpoint4_corrected_headless.csv`,
`goblin_crisis_checkpoint4_corrected_headless.json`, and
`goblin_crisis_checkpoint4_corrected_headless_aggregate.json`. They contain 39
rows (13 per class), 23 launches, zero resolutions, four retained caps, and zero
panics. Target acquisition and damage occurred, but the then-current normal-runner
schema did not stop at resolution and remained unsuitable as the final balance
matrix.

The corrected pair artifact contains 20 pairs/40 legs: all 20 declared launch
fingerprints and quantitative pairs were valid, all 40 legs ended in True Death,
zero assaults resolved, and there were zero setup failures, panics, or caps. These
reruns satisfied the Phase A requirement to test the corrected paths; they are
preserved as intermediate evidence and are not pooled with the later v2 decision
baseline or final matrix.

## Phase A cause classification

Primary classifications before correction:

- `harness_setup_defect`: the pair removes a starting item that the broader runner
  retains; its wall occupies an existing corpse tile; ambient actors are killed but
  remain positioned; treatment geometry is mutated post-launch; stop conditions and
  phase histories differ; normal-runner panic payloads are lost.
- `bot_policy_defect`: every class is driven as an adjacent quick-attack melee hero;
  the Ranger does not use its Training Bow or Aimed Shot, the Mage does not cast
  Arcane Bolt, and the bot abandons targets beyond radius three.
- `production_combat_defect`: a successful Health Potion use heals but does not
  decrement the item, allowing the broader bot to heal repeatedly from one starting
  potion. Spell death also does not remove the NPC thinker, and a pending move can
  later reset/move a dead actor; both require focused regression checks before Mage
  results are trustworthy.
- `engagement_or_pathing_defect` remains a measured risk: an attacker with no visible
  owning unit has no core/anchor fallback, and ordinary structures are never valid
  targets. It is not yet established as the primary cause of the reproduced 40
  deaths, because those legs show attackers repeatedly damaging the hero.
- `production_balance_problem` is not yet established. Corrected, class-valid,
  production-equivalent samples are required before numeric tuning.

## Correction gate

Before target bands or balance values were proposed, Checkpoint 4:

1. added idempotent engagement, wall/core exposure, and defeat-cause telemetry;
2. made the headless combat policy class-valid while retaining ordinary production
   events and combat rules;
3. removed the repeatable-potion production defect and covered spell/movement death
   races with focused regressions;
4. replaced invalid fixture geometry and dead-position blockers;
5. used assault-relative, attributable stop conditions and retained panic reasons;
   and
6. reran unchanged production until target acquisition, accepted attacks, damage,
   and gameplay-attributable termination are demonstrated.

## Phase A corrections and change-budget accounting

The diagnosis produced three separate categories of correction. Only the
explicit composition/timing experiments below count as production balance
proposals.

### Harness and telemetry corrections (zero production gameplay effect)

- The pair fixture retains the real starting Health Potion, removes dead
  positioned ambient blockers instead of leaving collision ghosts, creates a
  real six-Stockade enclosure before launch, never rewrites attacker positions
  after launch, uses assault-relative stop conditions, captures panic payloads,
  and fingerprints the expanded exact fields listed above.
- The opt-in sampler records first target/perception/movement/request/accept/hit/
  positive-damage boundaries, bounded stall counters, wall/core contact, healing,
  exact defeat/resurrection/True Death causes, cleanup, and ownership violations.
  The production sampler remains disabled unless the headless interval is set.
- Warrior, Ranger, and Mage bots use only normal production events and real
  equipment/abilities. Ranger cooldown repositioning is an ordinary legal move;
  no bot writes HP, mana, damage, defence, cooldowns, position teleports, or NPC
  state to manufacture an outcome.
- The final Ranger policy also computes the production Disengage destination and
  requires it to be unoccupied before emitting the ability or advancing its local
  cadence. The earlier passability-only check could issue an ability that the
  server rejected against a wall, then suppress real bow offense for 50 ticks.
  A focused occupied-retreat regression proves the corrected bot uses a legitimate
  bow attack and an alternate ordinary cooldown move without manufacturing a new
  combat deadline.
- The dedicated runner creates outputs with `create_new`, rejects protected
  Checkpoint 1–3 names, records commit/dirty-tree/build provenance, keeps every
  panic/setup failure/cap/invalid fingerprint, and serializes the RNG/full-ECS
  disclaimers.

### Intended-production bug and safety fixes (not balance changes)

These restore behavior already implied by the production action/combat/ownership
contracts. Each is independent of the goblin template values and is covered at
the real mutation boundary:

1. Successful Health Potion and Herbal Poultice use now decrements exactly one
   item and sends the updated inventory. A full-health, non-sick no-op consumes
   nothing. Existing Crude Bandage behavior is preserved and regression-tested.
   This fixes the diagnostic runner's reusable single potion; it does not change
   any healing amount, recipe, item template, or starting inventory.
2. Lethal spell/direct-ability damage removes the dead NPC thinker and emits the
   ordinary dead-state transition. A dead actor cannot finish a queued move or a
   stale queued attack, and NPC melee acceptance rechecks current adjacency.
   These close deferred-command races without changing damage, accuracy, range,
   speed, or cooldown values.
3. Exact action-boundary ownership checks reject foreign and neutral targets for
   attributed personal attackers. Candidate spawn positions also exclude the
   neighbouring owner's sanctuary footprint, not only ordinary structures.
   Helpers remain able to attack the owner's assault; ownership never transfers.
4. A cunning attributed attacker whose owner target is fully enclosed can select
   the actual blocking owner wall. This is an unreachable-target functional
   correction using the existing wall combat path, not a new pathfinder or wall
   statistic.
5. The prefinal payload reported only `Missing EventExecuting component:
   EntityDoesNotExist` and contained no backtrace identifying the exact action.
   Every NPC/villager site using that identical panic string now uses a fallible
   lookup. Requested movement, combat, drink/food, shelter, and sleep actions
   validate the boundary before damage, state mutation, or event scheduling;
   executing/cancelled actions tolerate component removal. NPC `AttackTarget`
   likewise validates before accepted telemetry, damage, telegraph mutation, and
   cooldown scheduling. Vital-needs scorers fail closed at zero for a stale actor;
   orphaned drink/eat/sleep/shelter actions cancel their queued event and reset
   state; and the real due-event handlers validate before inventory, needs,
   healing, stamina/mana, shelter-resident, or state mutation. Representative NPC
   Requested/Executing movement and attack plus villager movement, fight-back,
   scoring, consumption, sleep, and shelter lifecycle regressions cover both
   no-side-effect and safe-failure behavior.

The Safe Logout edge fixture also clears only the fixture hero's inherited pending
map events and restores a non-dead idle state after its deliberate sanctuary
placement. This prevents the ordinary bot move queued before fixture placement
from cancelling the ten-second logout. It changes no Safe Logout production rule.

### Focused battle-presentation correction

The single permitted presentation correction changes only personal-assault enemy
intent text to “Raider advancing on your defenders and blocking walls,” which
matches actual eligible targets. Legacy Wolf Rider/Pillager text retains its old
stored-value/structure wording. No protocol or full-screen UI was added.

The corrections above are intentionally retained even though no balance candidate
was accepted: they make the evidence valid and close real correctness/safety
defects. They do not constitute a hidden preparation modifier.

The first 260-row prefinal attempt is retained as
`goblin_crisis_balance_checkpoint4_prefinal_race_failure.json`: 11 resolutions,
239 True Deaths, four caps, one setup failure, five panics, two invalid paired
fingerprints, and 11 invariant-failure rows. Four panics were the known unrelated
`Windstride Stag` gather/template failure. The fifth was the unidentified
missing-`EventExecuting` action race above; the setup failure was `Safe Logout
Cancelled(Moved)`. Neither failing row was discarded. The Safe Logout correction
and generalized action-boundary hardening were made before the required final
artifact was regenerated.

## Corrected unchanged-production baseline

The class-valid, corrected harness was run in the release profile with the existing
personal-assault composition and values:

```bash
cd /Users/peter/projects/sp/sp_server
env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile corrected-baseline \
  --repetitions 10 \
  --assault-cap-ticks 15000 \
  --build-profile release-corrected-pre-tuning \
  --output goblin_crisis_balance_checkpoint4_corrected_baseline.json
```

The artifact has 60 rows: ten Basic Survival and ten Combined Preparation rows for
each of Warrior, Ranger, and Mage. All 60 launch fingerprints were valid; there
were no setup failures, panics, or tick-cap outcomes. Every row recorded accepted
hero and attacker attacks plus damage, so valid engagement was 60/60. All 60 rows
ended in assault-attributed True Death after two ordinary hero defeats, and none
resolved.

| Class / cell | Runs | Resolution | Runs with at least one kill | Mean kills | Mean hero damage | True Death |
|---|---:|---:|---:|---:|---:|---:|
| Warrior / Basic | 10 | 0 | 4 | 0.4 | 70.0 | 10 |
| Warrior / Prepared | 10 | 0 | 9 | 0.9 | 84.0 | 10 |
| Ranger / Basic | 10 | 0 | 1 | 0.1 | 21.5 | 10 |
| Ranger / Prepared | 10 | 0 | 0 | 0.0 | 30.6 | 10 |
| Mage / Basic | 10 | 0 | 2 | 0.2 | 62.1 | 10 |
| Mage / Prepared | 10 | 0 | 4 | 0.4 | 73.4 | 10 |

The original retained run used a 600-tick physical-state sampler. Attack requests,
acceptances, hits, damage, deaths, resolution, and True Death are exact
event-boundary observations and support the table above. Visibility, movement,
minimum-distance, and transient wall/core observations from that artifact are
coarse and will not be used as exact evidence. The final runner now samples those
physical observations every tick while removing only the redundant periodic
pressure vector from serialized rows; the final matrix will therefore retain exact
engagement observations without unbounded report growth.

This establishes `production_balance_problem` in addition to the diagnosed harness,
bot-policy, and production-functional defects. Corrected combat reliably engages,
but unchanged production offers no demonstrated prepared victory path for any
class and routinely reaches arbitrary True Death.

## Target bands declared before tuning

These bands were frozen before any production assault tuning. They are broad
acceptance ranges for repeated descriptive evidence, not confidence intervals and
not deterministic promises. Entropy-backed `thread_rng` is retained.

| Measure | Predeclared final band |
|---|---|
| Valid engagement | At least 90% overall and at least 80% in every class cell; an accepted attack and positive attributed combat damage are required |
| Prepared-solo resolution | 30–75% for every class; at least one legitimate prepared win for each class is mandatory |
| Basic-survival resolution | 5–40% overall; no class above 50% |
| Passive/unprepared resolution | 0–15%; passive play remains dangerous |
| At least one attacker defeated | 70–100% prepared; 25–75% basic |
| Hero survival | 50–90% prepared; 20–65% basic |
| Ordinary hero defeats | Prepared mean no more than 1.25; basic mean 0.75–1.75 |
| True Death | At most 20% prepared; 20–70% basic; passive expected at or above 60% |
| Assault duration | Resolved median 300–5,000 assault ticks; no instant resolution |
| Tick-cap unresolved | At most 20% prepared and 40% basic at the 15,000-tick cap |
| Wall contact | At least 60% of Existing Walls treatment rows record a wall target, hit, damage, or an explicit geometrically valid no-contact reason |
| Wall absorption | Positive absorbed damage in at least 40% of Existing Walls treatment rows |
| Core exposure | No increase versus matched control; wall treatment should reduce either core reach, core targeting, or core damage in repeated observations |
| Structure damage | May occur, but prepared victories must not routinely destroy core structures; damage must remain attributable and recoverable through normal repair |
| Villager losses | No more than 50% of living supported villagers in the median supported run; contribution or its current limitation must be measured |
| Helper-supported resolution | At least 60%, ownership unchanged, no added assault unit, and no participant scaling |
| Cross-player target violations | Exactly zero |
| Duplicate assaults or resolutions | Exactly zero |
| Setup failures / invalid fingerprints | Exactly zero in accepted final cells |
| Crisis-related panics | Exactly zero |
| Disconnect / reconnect invariants | 100% preserve assault identity, generation, surviving units, and continuation |
| Safe Logout invariants | 100% freeze before launch, never launch while protected, and remain unavailable during an active assault |

The required ordering is Prepared Solo > Basic Survival > Passive/Unprepared.
Preparation need not win every run, basic play must not become automatic success,
and no single preparation option may become universally mandatory.

## Corrected v2 decision baseline

The earlier 60-row artifact remains useful historical evidence, but it predates
the exact one-tick physical sampler, the six-Stockade blocking fixture, separate
preparation workloads, villager-supported workload, healing-use completion
telemetry, and the final launch-validity fields. It is therefore not pooled with
the decision baseline.

After all diagnostic harness, bot, telemetry, and intended-behaviour corrections,
and while the production composition was still two Wolf Riders plus one Goblin
Pillager, the release runner executed the final fixed-policy pre-tuning workloads:

```bash
cd /Users/peter/projects/sp/sp_server
env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile corrected-baseline --repetitions 10 \
  --assault-cap-ticks 15000 \
  --build-profile release-corrected-current-ranger-fixed \
  --output goblin_crisis_balance_checkpoint4_corrected_current_final.json

env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile focused-preparation --repetitions 15 \
  --assault-cap-ticks 15000 \
  --build-profile release-corrected-current-ranger-fixed \
  --output goblin_crisis_balance_checkpoint4_corrected_preparation_final.json
```

Both artifacts identify commit `db6cf6490f27adeb65787a55c791c525ae5efc14`
and a dirty working tree containing the documented Checkpoint 4 corrections.
Production `thread_rng` was retained; labels are not seeds, random streams were
not replayed, and hidden ECS state was not matched.

The broad artifact contains 100 rows: ten Basic Survival, ten Prepared Solo, and
ten Villager Supported rows for each class, plus ten rotating Passive rows. All
100 launch fingerprints were valid, every row acquired an owner target, accepted
combat, and recorded positive attributed damage, and there were no setup failures,
panics, invalid launches, automatic dusk waves, duplicate assaults, or cross-owner
target violations. Ten assaults resolved and 90 rows ended in True Death. The only
resolutions were in the controlled Villager Supported fixture.

| Class / cell | Runs | Resolved | Any attacker defeated | Mean defeated | True Death |
|---|---:|---:|---:|---:|---:|
| Warrior / Basic | 10 | 0 | 9 | 0.9 | 10 |
| Warrior / Prepared | 10 | 0 | 8 | 0.8 | 10 |
| Warrior / Villager Supported | 10 | 0 | 9 | 0.9 | 10 |
| Ranger / Basic | 10 | 0 | 0 | 0.0 | 10 |
| Ranger / Prepared | 10 | 0 | 2 | 0.2 | 10 |
| Ranger / Villager Supported | 10 | 5 | 9 | 2.1 | 5 |
| Mage / Basic | 10 | 0 | 3 | 0.3 | 10 |
| Mage / Prepared | 10 | 0 | 4 | 0.4 | 10 |
| Mage / Villager Supported | 10 | 5 | 8 | 1.9 | 5 |
| Passive / rotating classes | 10 | 0 | 6 | 0.6 | 10 |

The focused artifact contains 60 counterbalanced descriptive pairs and 120 legs:
15 pairs (five per class) for each of Existing Walls, Equipment Prepared, Healing
Prepared, and Combined Preparation. All 120 legs validly engaged. Four assaults
resolved and 116 rows ended in True Death. Existing Walls treatment recorded a
wall target, hit, and positive absorption in 15/15 rows (mean 30.67 HP absorbed),
proving the corrected blocking fixture exercises ordinary wall combat. Walls
improved resolution from 0/15 to 1/15 and mean attacker defeats from 0.33 to 0.53.
Combined Preparation improved resolution from 0/15 to 3/15 and mean defeats from
0.47 to 0.93. Equipment remained 0/15 in both legs with mean defeats unchanged at
0.53. Healing remained 0/15 in both legs, although mean defeats rose from 0.40 to
0.53 and the treatment completed a second existing healing item. These are
useful-system signals inside an assault whose total lethality still overwhelms
them.

Across the two authoritative v2 artifacts, 220/220 rows validly engaged, only
14/220 resolved, and 206/220 reached True Death. Prepared Solo, Basic, and Passive
resolved 0/30, 0/30, and 0/10 respectively. The only broad resolutions required
the controlled Villager Supported fixture; the four focused resolutions required
Existing Walls or Combined Preparation. This is the evidence gate for one narrowly
scoped production balance proposal; it is not inferred from the invalid Checkpoint
3 melee-only bot.

## Production change proposal recorded before implementation

Only the following first candidate is proposed. No enemy-template statistic,
pressure, timing, spawn radius, player statistic, hidden buff, or participant
scaling change is proposed at this gate.

| Field | Pre-implementation proposal |
|---|---|
| Finding | With the exact current composition, the authoritative corrected workloads engaged in 220/220 rows but 206 ended in True Death. Prepared Solo resolved 0/30 and only 4/120 focused preparation legs resolved. The three units have 205 total template HP (75 + 75 + 55) and three simultaneous ordinary attack sources. The earlier fixed-range-policy artifact that first motivated this proposal recorded the same direction with slightly different entropy-backed outcomes; it is not pooled with the authoritative workload. |
| Sample | `corrected-current-final`: 100 rows (10 Basic, 10 Prepared, and 10 Villager Supported per class plus 10 rotating Passive); `corrected-preparation-final`: 120 legs/60 pairs (five pairs per class for each of four comparisons). Run labels and complete failures remain in the raw artifacts. |
| Player problem | Legitimate class combat and existing preparation inflict damage and often defeat one attacker, but three concurrent attackers routinely preserve enough HP and damage throughput to force the second hero defeat. Preparation signals rarely cross the resolution boundary, making the first assault not credibly solo-completable for Warrior or Mage. |
| Hypothesis | Removing one duplicate Wolf Rider lowers simultaneous pressure from three attackers to two and required total damage from 205 to 130 (36.6% less) while retaining both existing goblin-family roles, ordinary AI, ordinary combat, ordinary loot, and a dangerous multi-enemy assault. |
| Exact change | Personal `GOBLIN_ASSAULT_COMPOSITION`: `["Wolf Rider", "Wolf Rider", "Goblin Pillager"]` -> `["Wolf Rider", "Goblin Pillager"]`. Shared Wolf Rider and Goblin Pillager templates remain unchanged; Legacy director systems remain unchanged. |
| Success metric | The frozen bands above: 30–75% Prepared Solo resolution in every class with at least one prepared win per class; 5–40% Basic overall; Passive 0–15%; Prepared materially above Basic and Passive; at least two preparation paths show repeated useful signal; wall/villager/helper and safety bands remain satisfied. |
| Rollback condition | Revert if valid engagement drops, any class still lacks a prepared win, Prepared exceeds 75% or Passive exceeds 15%, the change erases wall/villager relevance, creates a cross-owner/duplicate/panic/invariant failure, or fails to improve resolution and True Death materially. |
| Wider risks | One fewer normal enemy means one fewer possible corpse/loot roll and reduces maximum kill XP from 850 to 550; it changes remaining-attacker UI counts, spawn geometry, wall pressure, settlement damage, and helper time-to-resolution. It does not change item/resource values, recipes, production, normal template statistics, or legacy hordes. |
| Validation matrix | First run a bounded candidate smoke with the identical corrected and focused workloads. If it meets the direction gate, run the full 100-row final workload, 120-leg focused preparation workload, 40-row/five-repetition edge matrix, 15-pair offline/helper comparison where applicable, all focused production regressions, full server tests, Clippy, and the supported headless regression runner. |

This proposal was the first of at most four permitted production balance changes.

## Candidate 1 result and reversion

Candidate 1 was built in release mode with the bounded Ranger Disengage correction
and run with the same 15,000-tick cap:

```bash
cd /Users/peter/projects/sp/sp_server
env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile corrected-baseline --repetitions 10 \
  --assault-cap-ticks 15000 \
  --build-profile release-candidate1-ranger-fixed \
  --output goblin_crisis_balance_checkpoint4_candidate1.json

env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile focused-preparation --repetitions 15 \
  --assault-cap-ticks 15000 \
  --build-profile release-candidate1-ranger-fixed \
  --output goblin_crisis_balance_checkpoint4_candidate1_preparation.json
```

The broad workload had 100/100 valid engagements, no setup failure, panic,
invalid launch, tick cap, cross-owner target, duplicate assault, or invariant
failure. Resolution improved from 10/100 to 49/100 and True Death fell from
90/100 to 51/100, demonstrating that simultaneous three-unit pressure and total
attrition were real balance problems. It did not, however, satisfy the frozen
class and risk bands.

| Cell | Runs | Resolved | Any kill | Mean kills | Survived | True Death | Mean ordinary defeats |
|---|---:|---:|---:|---:|---:|---:|---:|
| Basic total | 30 | 8 (26.7%) | 17 | 0.83 | 8 | 22 (73.3%) | 1.73 |
| Basic Warrior | 10 | 8 (80%) | 10 | 1.80 | 8 | 2 | 1.20 |
| Basic Ranger | 10 | 0 | 0 | 0.00 | 0 | 10 | 2.00 |
| Basic Mage | 10 | 0 | 7 | 0.70 | 0 | 10 | 2.00 |
| Prepared total | 30 | 13 (43.3%) | 23 | 1.20 | 13 | 17 (56.7%) | 1.40 |
| Prepared Warrior | 10 | 8 (80%) | 10 | 1.80 | 8 | 2 | 1.20 |
| Prepared Ranger | 10 | 1 (10%) | 5 | 0.60 | 1 | 9 | 1.80 |
| Prepared Mage | 10 | 4 (40%) | 8 | 1.20 | 4 | 6 | 1.20 |
| Passive rotating | 10 | 2 (20%) | 7 | 0.90 | 2 | 8 | 1.80 |
| Villager Supported | 30 | 26 (86.7%) | 30 | 1.87 | 26 | 4 | 0.87 |

The focused workload recorded 38 resolutions, 79 True Deaths, and two tick caps.
Walls improved resolution from 3/15 to 7/15, reduced core-reaching attackers,
and recorded target, hit, and positive absorption in every treatment row. Combined
Preparation improved 4/15 to 6/15. Equipment regressed 5/15 to 3/15 and Healing
was flat at 5/15. One control leg panicked before launch with the known unrelated
`Cannot find item template: "Windstride Stag"` gather/template failure. Its pair
was retained and both rows were marked invalid rather than silently excluded.

Candidate 1 is **reverted**. Its declared rollback condition fired because
Prepared Warrior exceeded 75%, Prepared Ranger remained below 30%, Basic Warrior
exceeded 50%, and Passive resolution exceeded 15%. Prepared survival, Prepared
True Death, Prepared mean defeats, and Basic True Death also missed their frozen
bands. The result proves that simply deleting 75 HP and one attack source
overcorrects Warrior and passive play while still underserving Ranger. Shared
enemy values were never changed.

## Production change proposal 2 recorded before implementation

The first candidate's failed class split supports changing *when* the original
pressure arrives instead of deleting it. This proposal restores the exact original
three-unit multiset and all shared template values.

| Field | Pre-implementation proposal |
|---|---|
| Finding | Corrected three-unit play validly engaged but Prepared resolved 0/30; deleting one Rider moved Warrior Basic and Prepared to 80% while Ranger Prepared remained 10% and Passive reached 20%. Median first accepted attacker contact in the corrected three-unit workload was 140 ticks after launch (range 93–454). |
| Sample | Authoritative corrected broad 100 plus focused 120 rows, followed by Candidate 1 broad 100 plus focused 120 rows. Every row, including the unrelated panic and invalid pair, remains in the named artifacts. |
| Player problem | All three enemies focus the hero or settlement in the same opening window. Removing one makes durable melee play too easy, but leaving all three simultaneous gives fragile ranged classes too little time to use their legitimate range and existing preparation. |
| Hypothesis | Keeping all 205 HP and all three ordinary enemies while activating them in a short, visible sequence reduces opening focus pressure without erasing later attrition. A lower-HP Pillager first gives each class an attainable opening objective; later Riders preserve danger, loot opportunity, XP, walls, and villager relevance. |
| Exact change | Restore `GOBLIN_ASSAULT_COMPOSITION` to three live attributed units ordered `Goblin Pillager`, `Wolf Rider`, `Wolf Rider`. Activation offsets become `0`, `300`, and `600` ticks from the committed assault launch instead of all `0`. Delayed units remain ordinary, visible, damageable existing NPCs at their valid launch positions but cannot acquire, move toward, or attack a target before their explicit activation tick. Spawn positions are ordered nearest-to-farthest from the settlement anchor to align the activation order with the visible approach. |
| Success metric | All frozen bands, including Prepared 30–75% in every class, Basic 5–40% overall/no class above 50%, Passive 0–15%, valid engagement at least 90%, Prepared > Basic > Passive, at least two useful preparation paths, walls/core and helper bands, and zero lifecycle/safety violations. |
| Rollback condition | Revert if any delayed unit acquires, moves toward, or attacks a target early; resolution occurs before all three ordinary deaths; activation changes across disconnect/reconnect; any class misses a prepared victory path or frozen band; Passive exceeds 15%; engagement falls; walls/villagers become irrelevant; or any cross-owner, duplicate, cleanup, reward, panic, or invariant regression appears. |
| Wider risks | Dormant-but-visible units can be deliberately intercepted and damaged, so skilled early focus is possible. Activation must remain tied to the original world tick, assault ID, and generation across disconnect. The change affects approach timing, target choice, remaining-attacker interpretation, wall contact, structure exposure, and duration, but not shared templates, legacy director enemies, pressure, phase timing, economy, loot tables, recipes, or rewards. |
| Validation matrix | Focused component/action-boundary tests; bounded three-class smoke; then the exact 100-row broad, 120-leg preparation, and 40-row edge workloads at the same cap; focused lifecycle, class, economy, Safe Logout, persistent-crisis, and legacy tests; full format/check/test/Clippy; supported headless regression runner. |

This is the second proposed production balance change. No enemy value, pressure,
phase, spawn-radius, reward, player-stat, hidden-buff, or participant-scaling change
is proposed.

## Candidate 2 result and reversion

Candidate 2 was evaluated only through the predeclared bounded smoke gate. The
release runner and both artifacts used the same 15,000 assault-relative cap:

```bash
cd /Users/peter/projects/sp/sp_server
env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile corrected-baseline --repetitions 3 \
  --assault-cap-ticks 15000 \
  --build-profile release-candidate2-stagger-smoke \
  --output goblin_crisis_balance_checkpoint4_candidate2_smoke.json

env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile focused-preparation --repetitions 3 \
  --assault-cap-ticks 15000 \
  --build-profile release-candidate2-stagger-smoke \
  --output goblin_crisis_balance_checkpoint4_candidate2_preparation_smoke.json
```

The broad artifact retained 30/30 validly engaged rows with no setup failure,
panic, invalid launch fingerprint, invariant failure, or tick cap. It produced
only 4/30 resolutions, all in Villager Supported; 26/30 rows reached True Death.
Basic, Prepared Solo, and Passive resolved 0/9, 0/9, and 0/3. Every class was
0/3 in both Basic and Prepared Solo. Prepared recorded at least one attacker
defeated in 8/9 rows, showing that the stagger improved progress without creating
a credible solo completion path. One broad defeat was correctly retained as an
ambient Skeleton outcome rather than assault-attributed True Death.

| Broad cell | Runs | Resolved | Any kill | Mean kills | Survived | True Death | Mean ordinary defeats |
|---|---:|---:|---:|---:|---:|---:|---:|
| Basic total | 9 | 0 | 6 | 0.78 | 0 | 9 | 2.00 |
| Prepared total | 9 | 0 | 8 | 0.89 | 0 | 9 | 2.00 |
| Villager Supported | 9 | 4 | 9 | 2.22 | 4 | 5 | 1.56 |
| Passive | 3 | 0 | 2 | 0.67 | 0 | 3 | 2.00 |

The focused artifact retained 24 rows/12 descriptive pairs. It produced one
resolution, 22 True Deaths, and one unresolved Mage control at the cap. Equipment
was the only path to cross the resolution boundary (0/3 control to 1/3 treatment).
Walls, Healing, and Combined resolved 0/3 in both legs. Pair directions were
Existing Walls 0 improved/2 unchanged/1 worsened, Equipment 1/2/0, Healing
1/1/1, and Combined 0/2/1. One focused defeat was retained as an ambient Wolf
outcome.

Wall behavior itself was valid: all six wall-treatment legs recorded a wall
target, hit, and positive absorption, totaling 158 absorbed HP (26.3 mean).
There was no core target, core damage, core destruction, wall bypass, cross-owner
target, duplicate assault, or invariant failure. Core-reaching counts were 15 in
control and 15 in treatment, so the smoke did not establish net core-exposure
improvement.

The timing evidence explains why the exact stagger failed. In Prepared Solo the
first accepted hero action occurred 185–649 ticks after launch; the +300 Rider
was already active before that action in 8/9 rows. First ordinary hero defeat
occurred 975–1,553 ticks after launch; the +600 Rider was active before it in all
9 rows. The schedule therefore remained one overlapping opening fight in practice.

Candidate 2 is **reverted**. Its frozen rollback condition fired: Prepared Solo
resolution and survival were 0% in every class, Prepared and Basic True Death
were 100%, mean ordinary defeats were 2.0, and one focused row capped. The exact
`0/300/600` activation schedule, activation component/action gates, reordered
composition, and nearest-to-farthest spawn ordering were removed. The original
simultaneous `["Wolf Rider", "Wolf Rider", "Goblin Pillager"]` production
composition is restored while the next evidence gate is evaluated. The smoke
artifacts remain immutable evidence of the reverted experiment.

## Final class-policy validity correction before proposal 3

The Candidate 2 class split triggered one final harness-only policy audit before
using either remaining production-change slot. The server exposes Disengage and
Ward as normal class abilities, but the corrected v2 Ranger used Disengage only
once for an attacker and then traded indefinitely in melee, while the Mage never
used Ward. Those omissions materially underrepresented the two fragile classes.

The headless policy now uses only ordinary production events and the production
50-tick shared combat cooldown. Ranger interleaves one Disengage after two bow or
Aimed Shot commands when wounded and adjacent. Mage interleaves one Ward after
two Arcane Bolt or melee commands when wounded and adjacent. Changing targets
resets the bounded cadence. Focused integrated tests prove Ranger damage follows
a real bow/Aimed Shot event and that Mage Ward is accepted by the server through
the normal `ResponsePacket::Ability` path. No hero statistic, ability value,
enemy value, combat rule, or production code changed.

An unchanged-production three-unit smoke with the final policy used:

```bash
cd /Users/peter/projects/sp/sp_server
env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile corrected-baseline --repetitions 3 \
  --assault-cap-ticks 15000 \
  --build-profile release-policy-v3-current-smoke \
  --output goblin_crisis_balance_checkpoint4_policy_v3_current_smoke.json
```

All 30 rows validly engaged and ended through gameplay with no cap, setup
failure, panic, invalid launch, or invariant failure. The simultaneous current
assault still resolved 0/30 and reached True Death in 30/30. Basic resolved 0/9,
Prepared Solo 0/9, Villager Supported 0/9, and Passive 0/3. This confirms that
production tuning remains necessary even after both ranged-class policies use
their real defensive tools. It also supersedes the earlier bot-policy ceiling
for all subsequent candidate evidence; earlier immutable artifacts remain
historical rather than being pooled with this policy version.

## Production change proposal 3 recorded before implementation

Candidate 2 failed because its offsets were shorter than the measured opening
combat milestones, not because attributed live-unit staging itself failed its
action-boundary or lifecycle checks. Proposal 3 changes only those offsets; it
does not add a fifth proposal or combine an enemy-stat change.

| Field | Pre-implementation proposal |
|---|---|
| Finding | Candidate 2 Prepared first accepted hero actions occurred 185–649 ticks after launch, so +300 activated before 8/9 first actions. First ordinary defeats occurred 975–1,553 ticks after launch, so +600 activated before all 9. The final class-policy unchanged-production smoke still resolved 0/30 with 30 True Deaths. Candidate 1's two-enemy prepared resolutions completed at 1,865 ticks for its one Ranger win, 1,179–1,267 for Mage wins, and 2,425–3,105 for Warrior wins. |
| Sample | Candidate 2 smoke: 30 broad rows and 24 focused legs; final-policy unchanged-production smoke: 30 broad rows; Candidate 1 historical broad matrix: 100 rows. Entropy was not replayed and full ECS state was not matched, so the timing ranges are repeated descriptive evidence, not deterministic paired effects. |
| Player problem | A nominal stagger that overlaps before the hero's first real action feels and behaves simultaneous. Fragile classes cannot establish their ranged/defensive loop, while simply deleting a Rider makes durable Warrior and passive play too successful. |
| Hypothesis | Offsets beyond measured action and early-resolution boundaries create three visible engagement stages while retaining all 205 HP, all three ordinary deaths, full later danger, normal loot, and wall/villager relevance. A third activation at 2,000 should arrive before the historical durable-Warrior two-unit completion range but after the only historical Ranger completion, directly targeting the failed class split without class-specific buffs. |
| Exact change | Personal composition remains three live attributed units ordered `Goblin Pillager`, `Wolf Rider`, `Wolf Rider`; activation offsets are `0`, `800`, and `2,000` ticks from the committed launch. Delayed units remain visible, normally damageable NPCs but cannot acquire, pursue, or attack early. Positions are nearest-to-farthest from the owner anchor. The final activation occurs within one 2,400-tick world day. Shared templates and Legacy mode remain unchanged. |
| Success metric | The frozen bands: 30–75% Prepared resolution in every class with a win in each; Basic 5–40% overall and no class over 50%; Passive 0–15%; Prepared > Basic > Passive; Prepared/Basic kill, survival, defeat, True Death, duration, and cap bands; two useful preparation paths; wall/core/helper bands; zero lifecycle, ownership, duplicate, cleanup, panic, and invariant failures. |
| Rollback condition | Revert if any unit acts early or retimes across disconnect; the longer window produces combat stalls, pre-activation resolution, or universal interception; any class misses its Prepared band; Basic/Passive become too successful; two preparation paths do not show repeated value; or any wall, helper, ownership, cleanup, duplicate, reward, panic, legacy, economy, or invariant regression appears. |
| Wider risks | Visible inactive units can be deliberately intercepted for longer; duration can increase; remaining-attacker presentation includes enemies that have not activated; spawn ordering influences which unit is intercepted first; Safe Logout and ordinary disconnect must preserve absolute timing; a long isolated opening could reduce wall contact or make one path mandatory. No hidden immunity is added: delayed units remain damageable. |
| Validation matrix | Focused activation/action/lifecycle tests; 3-repetition broad and focused smoke using final class policies; promote only on direction; then exact 100-row broad, 120-leg focused, and 40-row edge workloads at 15,000 ticks, followed by all focused/full server, Clippy, legacy, persistent-crisis, Safe Logout, economy, villager, combat, network, and supported headless validations. |

This is the third of at most four permitted production balance proposals. One
production-change slot remains; no fourth proposal is implied or preapproved.

## Final Ranger policy validity correction

The first Candidate 3 smoke showed that the Ranger correctly issued Disengage
but then remained adjacent for the remaining 49 ticks of the real shared combat
cooldown. That was still an invalid representation of a ranged combat loop. The
final harness-only Ranger policy may issue one ordinary legal `PlayerEvent::Move`
during that cooldown when wounded and adjacent, but only to a passable unoccupied
tile that increases separation and remains within the equipped Training Bow's
range. Movement does not reset or bypass the combat deadline. The existing two
offensive commands per Disengage cadence remains bounded.

Focused policy tests prove the legal range-two move, two subsequent bow/Aimed
Shot commands on the original production cooldown, no defensive no-damage loop,
and accepted positive bow damage through the server. This changes no production
class, ability, movement, weapon, enemy, or cooldown value. All Candidate 3
decision evidence below uses this final policy-v4 bot unless explicitly labeled
pre-v4.

The later final-diff audit found one more harness-only validity defect in that
artifact-era policy: the bot checked that Disengage's computed retreat was valid
and passable, but not that it was unoccupied as production requires. It could
therefore advance local cooldown/cadence state after issuing a command the server
rejected against a Stockade. Non-damaging ability rejections were not serialized,
so affected historical Ranger rows cannot be identified or repaired post hoc.
Candidate-era Ranger resolution, survival, damage, preparation-direction, and
Ranger-inclusive wall totals are quarantined as diagnostics rather than acceptance
evidence. The final 260-row artifact was generated only after the occupancy fix.

No candidate needs to be resurrected to preserve its rejection: Candidate 1
independently failed on Basic Warrior 8/10, Prepared Warrior 8/10, and Passive
Warrior-driven 2/10; Candidate 2 had Prepared Warrior and Mage both 0/3; Candidate
3 had Prepared Warrior and Mage both 0/3 and a Warrior Passive resolution. Those
unaffected cells each trigger an already-declared rollback criterion.

## Candidate 3 result and reversion

The pre-v4 direction smoke is retained as
`goblin_crisis_balance_checkpoint4_candidate3_smoke.json` (30 broad rows) and
`goblin_crisis_balance_checkpoint4_candidate3_preparation_smoke.json` (24 focused
rows). It produced four broad resolutions, 25 True Deaths and one cap; only one
Prepared row resolved. The focused sample produced three resolutions, 20 True
Deaths, and retained one unrelated `Windstride Stag` gather/template panic rather
than rerunning it away. Because Ranger policy validity was still incomplete,
those outcomes were not an acceptance sample.

The final policy-v4 bounded gate used:

```bash
cd /Users/peter/projects/sp/sp_server
env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile corrected-baseline --repetitions 3 \
  --assault-cap-ticks 15000 \
  --build-profile release-candidate3-policy-v4-smoke \
  --output goblin_crisis_balance_checkpoint4_candidate3_policy_v4_smoke.json

env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile focused-preparation --repetitions 3 \
  --assault-cap-ticks 15000 \
  --build-profile release-candidate3-policy-v4-smoke \
  --output goblin_crisis_balance_checkpoint4_candidate3_policy_v4_preparation_smoke.json
```

All 30 broad rows validly launched and engaged with zero setup failure, panic,
invalid fingerprint, duplicate assault, or cross-owner violation. Five assaults
resolved, 24 reached True Death, and one Prepared Warrior remained alive but
unresolved at the cap. The distribution failed the frozen ordering:

| Broad cell | Runs | Resolved | Any kill | Survived | True Death | Cap |
|---|---:|---:|---:|---:|---:|---:|
| Basic total | 9 | 1 (11.1%) | 9 | 1 | 8 | 0 |
| Prepared total | 9 | 0 | 9 | 1 | 8 | 1 |
| Passive | 3 | 1 (33.3%) | 3 | 1 | 2 | 0 |
| Villager Supported | 9 | 3 (33.3%) | 9 | 3 | 6 | 0 |

Every Prepared class was 0/3. The Basic and Passive resolutions were both
Warrior outcomes. Prepared any-kill was 9/9, showing valid progress, but survival
was 1/9, True Death 8/9, and mean ordinary defeats 1.78. Basic survival was 1/9,
True Death 8/9, and mean defeats 1.89. Passive resolution exceeded the frozen
15% maximum.

The 24-row policy-v4 focused artifact had 24/24 valid launches, one resolution,
22 True Deaths, and one Combined-treatment cap. The only resolution was an
Existing-Walls **control**. Resolution direction was Walls 1 control to 0
treatment, Equipment 0 to 0, Healing 0 to 0, and Combined 0 to 0. All three wall
treatments nevertheless recorded real contact, hits, and absorption (20, 40,
and 20 HP; 80 total), proving wall engagement while failing to demonstrate a
repeatable outcome benefit.

Candidate 3 is **reverted**. The `0/800/2000` activation component, every early
AI/action gate, reordered `Pillager/Rider/Rider` composition, and
nearest-to-farthest spawn ordering were removed. The production configuration is
again the original simultaneous `Wolf Rider`, `Wolf Rider`, `Goblin Pillager`.
Shared templates were never edited.

## Fourth-slot decision: no speculative proposal

No fourth production balance change is proposed. That is a deliberate evidence
decision, not an unreported experiment. The remaining one-dimensional knobs
(composition count, global personal-assault HP, damage, or repeat cooldown) did not
have clean evidence capable of satisfying all frozen bands:

- Candidate 1's easier 130-total-HP/two-source fight made Basic and Prepared
  Warrior resolve 8/10 and Passive resolve 2/10, independently exceeding the
  frozen Basic-class, Prepared-class, and Passive ceilings.
- Candidate 2 and Candidate 3 both left clean Prepared Warrior and Mage cells at
  0/3. Candidate 3 also produced a clean Warrior Passive win. Their intended
  staggering mechanisms therefore failed independently of quarantined Ranger
  evidence.
- A proposed 1.5× personal repeat-attack cooldown was independently considered:
  its aggregate attack cadence nearly equals Candidate 1 while restoring the
  removed Rider's 75 HP. That indirect analogy is not clean predeclared evidence
  that it would satisfy every class and preparation band, so implementing it would
  be speculative rather than evidence-supported.

The least-bad future direction is a separately designed, explicit target-pressure
distribution rule that makes additional attackers interact with existing walls
and defenders instead of applying a global lethality scalar. That is not proven,
would need new predeclared acceptance evidence, and is deferred rather than
smuggled into the final slot.

Accordingly, the final Checkpoint 4 report describes the corrected original
configuration and the three reverted candidates. There are **zero accepted
production balance changes**. Milestone 3 must remain open because Warrior and
Ranger lack a demonstrated prepared victory path under the restored configuration
and the preparation/outcome ordering is not met.

## Final restored-configuration matrix

After the despawn race, Safe Logout fixture, occupied-Disengage correction, and
generalized NPC/villager event-boundary hardening, the required final artifact
was generated once without discarding any row:

```bash
cd /Users/peter/projects/sp/sp_server
cargo build --release --bin goblin_crisis_checkpoint4_runner
test ! -e goblin_crisis_balance_final.json && \
  env CARGO_MANIFEST_DIR="$PWD" \
  target/release/goblin_crisis_checkpoint4_runner \
  --profile full --repetitions 10 \
  --assault-cap-ticks 15000 \
  --build-profile release-final-corrected-event-boundary-hardening \
  --output goblin_crisis_balance_final.json
```

The artifact records commit `db6cf6490f27adeb65787a55c791c525ae5efc14`, a
dirty working tree containing this checkpoint, 12,000 prelaunch ticks, a 15,000
assault-relative cap, production `thread_rng`, no replayed random stream, and no
claim of full-ECS matching. Its 260 rows are 100 broad, 80 focused-preparation,
and 80 edge rows. Endpoints were 14 resolutions, 239 True Deaths, six caps, no
setup failure or missing hero, and one retained unrelated panic. All 70 paired
fingerprints are valid; 13 edge rows retain invariant failures. Telemetry exists
for 259 rows. Its SHA-256 is
`02dcd4c06651602d3e585ca9e5d3b8db4eb8c5e7552f1222ffa95b10536e6e0d`.

The one panic is
`corrected_baseline-villager_supported-warrior-r008`, classified
`missing_windstride_stag_template` with payload `Cannot find item template:
"Windstride Stag"`. It occurred before launch, is unrelated to goblin combat, and
is not silently rerun away.

## Final production composition, values, and approach

There are no accepted balance changes. All 259 launches in the final artifact use
the original simultaneous composition, ordinary templates, and exact attribution:

| Unit | Count | HP | Stamina | Base damage | Damage range | Defence | Speed | Template vision | Personal viewshed | Kill XP |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Wolf Rider | 2 | 75 | 250 | 6 | 5 | 5 | 6 | 4 | 14 | 300 |
| Goblin Pillager | 1 | 55 | 200 | 5 | 4 | 4 | 5 | 3 | 14 | 250 |

All three are active at launch. Spawn selection remains the original shuffled,
unique, valid, passable, unoccupied set: sanctuary-relative candidates use the
existing weak-radius offsets of +1 through +3 and fallback candidates are six to
eight tiles from the selected owner anchor. The candidate limit remains 96 and a
neighbour's structure/sanctuary footprint has a three-tile exclusion. No
nearest-to-farthest ordering, activation delay, new template, theft AI, torch AI,
hidden scaling, or participant scaling remains.

All 259 launch ticks are the configured dusk tick 2,000 modulo the 2,400-tick day.
Every one of the 99 nonpanic broad rows and all 80 focused rows recorded owner
targeting on both sides, accepted attacks, hits, and positive attributed damage.
The runner classified 77/80 edge rows as validly engaged; three retained
`no_perception` rows did not. Thus the mechanically valid engagement rate is
256/259 telemetry rows (256/260 retained rows), including 179/179 intended online
balance rows. All 259 rows recorded an attacker target; 255 recorded visibility,
241 a hero target, and 240 two-sided accepted attacks, hits, and damage. The lower
two-sided count is concentrated in deliberate disconnect/target-loss edge cases,
not the class-balance fixtures.

## Final broad class and balance results

| Cell | Class | Runs | Resolved | Any kill | Mean kills | Survived | True Death | Mean defeats | Cap/panic | Mean hero damage to assault |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Basic | Warrior | 10 | 0 | 7 | 0.70 | 0 | 10 | 2.00 | 0/0 | 79.8 |
| Basic | Ranger | 10 | 0 | 0 | 0.00 | 0 | 10 | 2.00 | 0/0 | 31.1 |
| Basic | Mage | 10 | 0 | 0 | 0.00 | 0 | 10 | 2.00 | 0/0 | 44.6 |
| Prepared Solo | Warrior | 10 | 0 | 4 | 0.40 | 0 | 10 | 2.00 | 0/0 | 76.4 |
| Prepared Solo | Ranger | 10 | 0 | 3 | 0.30 | 0 | 10 | 2.00 | 0/0 | 55.6 |
| Prepared Solo | Mage | 10 | 2 | 4 | 0.80 | 2 | 8 | 1.60 | 0/0 | 91.8 |
| Villager Supported | Warrior | 10 | 1 | 7 | 0.90 | 1 | 8 | 1.70 | 0/1 | 109.4 across telemetry rows |
| Villager Supported | Ranger | 10 | 4 | 9 | 2.00 | 4 | 6 | 1.40 | 0/0 | 165.3 |
| Villager Supported | Mage | 10 | 3 | 9 | 1.80 | 3 | 7 | 1.60 | 0/0 | 154.2 |
| Passive | Warrior | 4 | 0 | 3 | 1.00 | 0 | 4 | 2.00 | 0/0 | 98.8 |
| Passive | Ranger | 3 | 0 | 0 | 0.00 | 0 | 3 | 2.00 | 0/0 | 41.7 |
| Passive | Mage | 3 | 0 | 0 | 0.00 | 0 | 3 | 2.00 | 0/0 | 48.0 |

The two Prepared resolutions were Mage at 2,290 and 2,291 ticks. The eight
Villager-Supported resolutions were Warrior 1/10, Ranger 4/10, and Mage 3/10;
their median duration was 2,637.5 ticks. Basic resolved 0/30 and Passive 0/10.
The frozen bands therefore read:

| Gate | Final evidence | Result |
|---|---|---|
| Valid broad engagement | 99/100 retained; 99/99 nonpanic | Pass |
| Prepared resolution, each class 30–75% | Warrior 0%, Ranger 0%, Mage 20% | Fail |
| Basic resolution 5–40% | 0/30 | Fail |
| Passive resolution 0–15% | 0/10 | Pass |
| Prepared any kill 70–100% | 11/30 (36.7%) | Fail |
| Basic any kill 25–75% | 7/30 (23.3%) | Fail |
| Prepared survival 50–90% | 2/30 (6.7%) | Fail |
| Basic survival 20–65% | 0/30 | Fail |
| Prepared mean defeats <=1.25 | 1.87 | Fail |
| Basic mean defeats 0.75–1.75 | 2.00 | Fail |
| Prepared True Death <=20% | 28/30 (93.3%) | Fail |
| Basic True Death 20–70% | 30/30 | Fail |
| Passive True Death >=60% | 10/10 | Pass |
| Basic/Prepared cap limits | 0/30 in both | Pass |
| Prepared > Basic > Passive materially | 6.7% > 0% = 0% | Fail |

The restored assault is engageable and can be resolved, but it is not reliably
solo-completable or fair across the three classes. Two Mage wins are not a
credible all-class prepared path.

## Final preparation comparisons

The focused workload has 40 counterbalanced pairs/80 valid legs: ten pairs per
path, with four Warrior, three Ranger, and three Mage pairs in each path. All 80
legs reached True Death; no control or treatment resolved or capped. Random
streams were not replayed, so deltas are repeated descriptive evidence rather
than causal effects.

| Path | Mean/median resolution delta | Mean/median kill delta | Mean/median survival delta | Mean/median structure-damage delta | Improved / unchanged / worsened |
|---|---:|---:|---:|---:|---:|
| Existing Walls | 0 / 0 | -0.10 / 0 | 0 / 0 | +26.2 / +25 | 1 / 7 / 2 |
| Equipment | 0 / 0 | +0.40 / 0 | 0 / 0 | 0 / 0 | 3 / 6 / 1 |
| Healing | 0 / 0 | -0.10 / 0 | 0 / 0 | 0 / 0 | 0 / 9 / 1 |
| Combined | 0 / 0 | -0.20 / 0 | 0 / 0 | +24.5 / +22 | 0 / 8 / 2 |

The conservative classification treats any simultaneous guardrail regression as
worsened and excludes hero-damage direction when healing is the declared
difference. Existing Walls had kill deltas positive in one pair, unchanged in
seven, and negative in two. Equipment had three positive and seven equal kill
deltas; one equal-kill row was classified worsened because the treatment took
five additional hero damage. Healing had zero positive, nine equal, and one
negative kill delta. Combined had zero positive, eight equal, and two negative.
Equipment therefore has a limited repeated attacker-kill signal, but none of the
paths improved survival or resolution.

The requested “two useful preparation paths” threshold was not quantitatively
defined before the final run. It would be improper to invent a permissive
post-hoc rule. Under the conservative outcome standard above, no treatment
resolved and no path showed a repeated survival/resolution advantage,
so the closure gate is recorded as failed.

### Walls, core exposure, and structure damage

“Core structure” is telemetry-only, not a new production target category: a live,
built, human-owned `ClassStructure` whose object class is Structure and whose
subclass is not Wall. “Reached core” means an attributed attacker came within one
tile. A bypass is counted only when a complete live owner wall ring surrounded the
reached core and the attacker had not targeted a ring wall.

Existing-Walls treatments recorded wall target/contact, hits, and positive
absorption in 10/10 rows: 35 acquisitions, 44 hits, and 262 HP absorbed. Combined
recorded all three in 10/10: 33 acquisitions, 40 hits, and 245 HP. Exactly one of
six Stockades was destroyed in each treatment. No attacker was classified as
bypassing an available ring. Core reach was 26 control versus 25 wall treatment,
and 28 control versus 30 Combined treatment. Across all 260 rows there were zero
core targets, zero core damage, and zero core destruction. All 2,749 structure
damage in the full artifact was wall damage.

Walls therefore pass the mechanical contact/absorption bands, but they did not
produce a repeated resolution or survival benefit, and Combined regressed the
core-reach count. The two Prepared wins took 24 and 26 wall damage; each damaged
two Stockades and destroyed one. The matrix does not perform post-battle repair,
so actual recoverability is not established.

### Equipment, healing, and villagers

The Equipment treatment equipped the existing Hide Wraps without a hidden stat or
crisis modifier. Its ten pairs had no resolution/survival difference and three
positive, seven equal, and zero negative kill deltas. This is a limited repeated
progress signal, not a demonstrated prepared victory path.

Every Healing treatment legitimately consumed one extra Crude Bandage while
retaining the normal starting potion. Healing restored a mean 3.0 additional HP
(median 3); Combined restored a mean 7.3 additional HP (median 8.5). The item
accounting works, but Healing treatments had one fewer kill and no survival or
resolution benefit. More cumulative damage in these rows is expected when an
extra HP buffer is consumed and is not itself an improvement.

The 29 completed Villager-Supported launches lost 12 villagers (median row loss
zero). They absorbed 10,786 damage; 1,011 zero-effective villager hit events were
observed, but villagers dealt zero positive assault damage and received zero
kill credit. All 47 supported-row kills belonged to the owner hero. The eight
resolutions demonstrate a supported configuration, not a causal villager benefit:
the scenario also contains walls and uses unreplayed RNG. Existing villager attack
request/acceptance events are not exposed at the same boundary as hero/NPC events,
so those counters remain zero and are a telemetry limitation.

## Final edge and lifecycle results

| Scenario | Runs | Resolved / TD / cap | Exact result |
|---|---:|---:|---|
| Ordinary disconnect | 10 | 0 / 8 / 2 | 10/10 preserved `AssaultActive`, assault identity/generation/timing, unit IDs/HP, and reconnect to the same assault without reset. |
| Safe Logout before launch | 10 | 0 / 8 / 2 | 10/10 completed protection, froze crisis/world state, launched nothing while protected, resumed without catch-up, and rejected Safe Logout once active. |
| Helper Supported | 10 | 3 / 7 / 0 | Helper dealt positive accepted damage in 9/10, 406 HP total, and received five kills; one Warrior row failed the participation invariant. Ownership stayed with the owner and unit count stayed three. |
| Helper Departure | 10 | 0 / 10 / 0 | 9/10 helpers dealt damage, departed, and preserved assault identity/generation; one Mage row never engaged or departed and retained three related invariant labels. |
| Offline Owner Helper | 10 | 1 / 9 / 0 | All ten owners disconnected. Helpers dealt 452 HP while owners were offline (469 HP total including 17 before disconnect), with five kills. One assault resolved exactly once while its owner was offline; nine failed the repeated resolution gate, and one of those also recorded no offline helper damage. |
| Adjacent isolation | 10 | 0 / 9 / 1 | Deliberate helper assistance dealt positive damage in 9/10; one Mage row failed participation. There were zero neighbour target/spawn violations. |
| Adjacent target loss | 10 | 0 / 10 / 0 | Owner-target loss was observed 10/10 and never redirected to neighbour assets. |
| True Death cleanup/fresh run | 10 | 0 / 9 / 1 | The nine rows that reached True Death removed the crisis/units, granted no resolution, and created a clean fresh run. One disconnected Warrior remained out of perception at the cap, so cleanup was not reached and the row retained five invariant labels. |

Helper Supported resolved 3/10 (30%), below the frozen >=60% helper band. The
one offline resolution proves the code path but does not repair that repeated
outcome failure.

Across both adjacent workloads there were zero neighbour hero, villager,
structure, or sanctuary targets; zero footprint overlaps; zero spawn-exclusion
violations; and zero action-boundary/telemetry cross-owner violations. The owner
and neighbour anchors were nine tiles apart and helpers rendezvoused through
ordinary movement. The artifact does not snapshot every neighbour HP value, so
“unaffected” is limited to targeting, spawn, and ownership evidence.

Safe Logout recorded 20 requests: ten accepted/completed before launch and ten
rejected for `assault_active`; there were ten protected sessions, resumes, and
timer rebases, 2,580 protected ticks, and zero invariant recoveries. This proves
no catch-up and no state/resource progress while protected, but entropy-backed
post-reconnect outcomes cannot be called causally equivalent.

The offline-helper result demonstrates that offline completion is mechanically
possible, but 1/10 is not reliable enough to pass the repeated outcome gate.
Focused lifecycle tests continue to cover the exact resolution boundary.

## Defeat causes, resolution, and repeated-run safety

The 260 final rows classify as 229 assault-attributed True Deaths, 10 ambient-enemy
True Deaths, six unresolved caps, 14 resolutions, and one unrelated panic. No
needs death, sanctuary destruction, critical/core destruction, pathing-stall
defeat, or unknown combat defeat was recorded. Among the 259 telemetry rows, the
secondary engagement reasons were 18 `ambient_death`, three `tick_cap`, three
`no_perception`, and 235 null; the panic has no telemetry. These labels are
diagnostic rather than the authoritative defeat cause.

Every resolved row has exactly three attributed defeats, zero remaining units,
and `crisis_assaults_resolved == 1`. No row reports more than one launch or
resolution; duplicate assaults/resolutions, automatic PersonalCrisis dusk hordes,
cross-owner targets, wall bypasses, core damage, and runtime crisis/Safe-Logout
invariant flags are all zero. Resolution kill attribution was 36 owner, six helper,
zero villager, and zero other. Controlled/True Death cleanup never resolved.
Normal NPC loot and existing crisis score integration are unchanged; no new reward
was added. The machine artifact does not itself expose duplicate notice, loot, or
score packets, so those claims rely on focused mutation-boundary tests rather than
row fields.

The retained exceptions are material: six caps, one unrelated panic, zero invalid
paired fingerprints, and 13 edge-invariant rows. Nine are failed offline-helper
resolution expectations; the remaining four are one Helper-Supported
participation failure, one Helper-Departure participation/departure failure, one
Adjacent-Isolation participation failure, and one cleanup row that never reached
True Death. These prevent an accepted-final safety claim even though no
crisis-related panic or ownership/duplicate violation occurred.

## Validation command record

All final server commands below ran from `sp_server/`. No frontend file changed,
so the conditional frontend commands were not run and are not claimed as passes.

- `cargo fmt --all -- --check` passed.
- `cargo check` passed with 68 existing warnings.
- `cargo test --lib checkpoint4` passed 42/42.
- `cargo test --lib crisis_balance::tests` passed 28/28.
- The focused occupied-Ranger-retreat and NPC despawn/action regressions passed.
  The NPC attack/movement missing-boundary regressions passed. Villager scorer,
  movement, fight-back, drink, eat, sleep, and shelter lifecycle filters passed;
  the complete villager test module passed 97/97.
- `cargo test --lib safe_logout` passed 64/64 after correcting a stale test
  fixture that placed an attacker on the hero's tile. Its first run retained that
  failure at 63/64; the isolated corrected regression passed before the full
  filter was repeated.
- The final unfiltered `cargo test` passed: 511 library tests, 9 Checkpoint 4
  runner tests, 17 headless-runner tests, 5 preparation-runner tests, and 6
  day-system tests. The remaining targets had zero tests and the one doctest was
  ignored. The first full run retained two failures (494/496) because two older
  Checkpoint 3 disconnect fixtures also placed attacker and target on the same
  tile; only those test fixtures were corrected, and their focused regressions
  passed before the full rerun.
- `cargo test --lib personal_crisis` passed 7/7. The focused Checkpoint 1–3
  filters also passed: `goblin_balance_checkpoint2` 2/2, `goblin_phase` 3/3,
  `goblin_pressure` 3/3, `preparation_pair_` 7/7,
  `checkpoint3_legacy_mode` 1/1, `scheduled_dusk_horde` 2/2, and
  `introductory_encounter` 1/1.
- The runner targets passed independently: `cargo test --bin headless_runner`
  17/17, `cargo test --bin preparation_pair_runner` 5/5, and
  `cargo test --bin goblin_crisis_checkpoint4_runner` 9/9.
- `cargo clippy --all-targets --all-features` exited successfully. It reported
  existing warnings rather than a warning-free tree: 1,351 library-test warnings
  (1,332 duplicates), plus the existing runner warnings.
- `env CARGO_MANIFEST_DIR="$PWD" cargo run --release --bin headless_runner -- 1
  1000 standard` passed its supported regression sample: one retained `MaxTicks`
  row, zero panics, zero invariant failures, and zero automatic dusk hordes. It
  wrote the ignored generic `headless_runs.csv` and `headless_runs.json` outputs.
- `cargo build --release --bin goblin_crisis_checkpoint4_runner` passed before
  the protected matrix, ensuring the runner contained the final hardening source.
- The protected final-report command was
  `test ! -e goblin_crisis_balance_final.json && env
  CARGO_MANIFEST_DIR="$PWD" target/release/goblin_crisis_checkpoint4_runner
  --profile full --repetitions 10 --assault-cap-ticks 15000 --build-profile
  release-final-corrected-event-boundary-hardening --output
  goblin_crisis_balance_final.json`. It completed and retained all 260 rows: 14
  resolutions, 239 True Deaths, six caps, no setup failure, one unrelated panic,
  zero invalid fingerprints, and 13 invariant rows. A final `jq -e` audit
  confirmed those aggregate counts, zero duplicate launches/resolutions, zero
  automatic PersonalCrisis dusk hordes, and zero cross-owner violations.
- Final separate `cargo fmt --check` and `cargo check` runs passed, and `git diff
  --check` reported no whitespace errors.

The prefinal full artifact is intentionally retained as
`goblin_crisis_balance_checkpoint4_prefinal_race_failure.json`: it contains one
setup failure and five panics (four known `Windstride Stag` panics and one
unidentified `EntityDoesNotExist`/missing-`EventExecuting` action race). The Safe
Logout fixture cancellation and every identical NPC/villager panic site were
corrected before the protected final artifact; unrelated template panics remain
retained rather than being silently rerun away.

## Final conclusions

1. Checkpoint 3's zero kills came from a combination of incomparable fixtures,
   removal of a reusable-potion bug's effect, invalid wall/dead-actor geometry, and
   a class-blind adjacent-melee bot. The broader Checkpoint 2 rows used different
   progression/stops and could repeatedly reuse the starting potion.
2. The primary issue was a combination of harness setup, bot policy, intended
   production combat defects, and—after correction—a real production balance
   problem.
3. Yes, the assault is reliably engageable: 99/99 nonpanic broad rows and 80/80
   focused rows reached visibility, targeting, accepted combat, hits, and damage.
4. No, it is not reliably solo-completable under the restored configuration.
5. No Prepared Warrior resolved (0/10).
6. No Prepared Ranger resolved (0/10).
7. Mage had two legitimate Prepared wins (2/10), but that is below the credible
   path band and does not close the class gate.
8. Prepared play does not materially outperform Basic: 2/30 versus 0/30 with
   nearly identical death/survival failure, and Passive also remains 0/10.
9. Equipment shows a limited repeated attacker-kill signal, but no two paths show
   a repeated survival/resolution benefit.
10. Yes, walls mechanically engage and absorb pressure in every wall treatment,
    but no repeated survival/resolution benefit was demonstrated and Combined
    core reach increased from 28 to 30.
11. Villagers measurably absorb pressure but produced zero effective assault
    damage or kills; their independent benefit is not established.
12. Healing is legitimately consumed and restores HP, but it was not useful at the
    outcome level in this matrix.
13. Hide Wraps equipment improved attacker defeats in three pairs and matched
    seven, but showed no repeated survival or resolution benefit.
14. Wall damage is attributable and bounded in the fixture, with zero core damage,
    but recoverability in successful prepared runs is not established.
15. Connected helpers improved the descriptive resolution range to 3/10 without
    changing ownership or unit count; one of ten offline-helper rows resolved
    exactly once while its owner was offline.
16. Ordinary disconnect showed no identity, timing, HP, or respawn advantage in
    10/10, though unreplayed RNG prevents a causal outcome claim.
17. Safe Logout froze state and granted no catch-up/resource progress in 10/10;
    causal post-reconnect balance equivalence is not claimed.
18. Yes, neighbouring settlements were isolated in 20/20 dedicated target/spawn
    scenarios while deliberate assistance remained possible.
19. Failures are substantially attributable: 229 assault True Deaths, 10 ambient
    True Deaths, six caps, and one named unrelated content panic.
20. No, Milestone 3 is not ready to close.

## Known limitations and deferred work

- Production entropy is not replayed and full ECS state is not matched. Pair labels
  are identifiers, not seeds, and no preparation or logout comparison is causal.
- The system-local player combat deadline cannot be fingerprinted; accessible
  `LastCombatTick`, complete Stats/Needs/Effects, fixtures, and production-event
  acceptance are covered instead.
- Warrior and Ranger still lack a prepared win; all-class, Basic, survival, death,
  True Death, ordering, and two-path preparation bands fail.
- Offline-helper resolution is only 1/10. One Helper-Supported row, one
  Helper-Departure row, and one Adjacent-Isolation row never record helper damage.
  Helper and villager effectiveness need a new evidence-backed design, not
  participant scaling or hidden bonuses.
- Villager request/acceptance counters are not emitted from the ordinary villager
  AI boundary; zero-effective hits, positive damage, kills, losses, and absorbed
  damage remain observable.
- Core reach is a telemetry-only geometric definition. Core structures are not a
  production fallback target, and this matrix recorded no core damage.
- True Death cleanup and fresh-run reset passed all nine rows that reached True
  Death; one disconnected cleanup row stalled outside perception at the cap, so
  the repeated cohort is 9/10 rather than complete.
- The known random `Windstride Stag` template panic remains unrelated work. It is
  retained rather than fixed by broadening this checkpoint.
- Client files did not change, and the artifact cannot prove duplicate notice/UI,
  loot, or score packets by itself. Focused server tests are the applicable
  evidence.
- Durable restart persistence, a second crisis family, regional crises, offline
  production, larger maps, additional starts, and cross-world systems remain out
  of scope and unimplemented.

The least speculative next balance work is an explicitly designed target-pressure
distribution experiment that makes additional attackers interact with existing
walls and defenders without globally trivializing Warrior/Passive play. It must
predeclare quantitative preparation-path value, class bands, helper/offline-helper
expectations, and rollback rules before touching production. A controlled RNG
replay facility would improve evidence quality but is separate test infrastructure.
