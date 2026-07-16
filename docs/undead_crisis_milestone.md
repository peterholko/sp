# Milestone 4 — Second Personal Crisis: Undead Incursion

## Status and checkpoint boundary

Checkpoint 1, **Multi-Crisis Foundation and Undead Pre-Assault Progression**, is
complete on the latest `main`. Checkpoint 2, **Undead Assault Lifecycle and
Feature Validation**, is implemented and validated on
`undead_crisis_milestone`. It completes the second personal crisis by extending
the existing personal-assault lifecycle by `CrisisKind`; it does not introduce
another scheduler, balance candidates, a new runner, broad telemetry, or a
third crisis.

The milestone has two checkpoints:

1. **Checkpoint 1 — multi-crisis foundation and pre-assault progression:** add
   explicit crisis kinds, per-run completion history, ordered Goblin-to-Undead
   sequencing, the online-only inter-crisis delay, deterministic Undead
   pressure, ordered Undead phases through `AssaultReady`, kind-correct status
   presentation, cleanup, focused tests, and two bounded headless smokes.
2. **Checkpoint 2 — Undead assault lifecycle and final validation:** define and
   implement the Undead launch, attributed units, assault identity, combat,
   resolution, score, disconnect/helper behavior, cleanup, and final feature
   validation. This is a fixed-composition feature checkpoint, not a balance
   checkpoint.

## Checkpoint 1 architecture findings

### Existing crisis authority and clocks

* `SurvivalDirectorConfig` in `sp_server/src/game.rs` defaults to
  `PersonalCrisis`. Personal crisis evaluation and launch are separately
  scheduled behind `personal_survival_director`; the retained rat, wolf,
  goblin, legacy Undead, Pillager, nightly-horde, and legendary systems remain
  registered behind `legacy_survival_director`.
* `SettlementCrisisState` is the authoritative runtime map and currently holds
  at most one `SettlementCrisis` per player. `SettlementCrisis::new` currently
  hardcodes Goblin, while the only existing `CrisisKind` value is `Goblin`.
* `personal_crisis_system` owns derived Goblin pressure, online-active time,
  and at-most-one ordered pre-assault transition per evaluation.
  `personal_crisis_assault_system` separately owns Goblin ready grace, launch
  timing, ID allocation, composition, spawn attribution, committed assault,
  normal-death evidence, and exactly-once resolution.
* `GameTick` remains the global environmental clock. A crisis already records
  `last_evaluated_tick`, `online_active_ticks`, and `phase_online_ticks`.
  `advance_online_crisis_time` advances its watermark without credit while an
  owner is ordinarily offline, so the existing `phase_online_ticks` is the
  narrow clock for the 60-online-second post-Goblin delay.
* `record_personal_assault_resolution` is the only normal Goblin-resolution
  boundary. It resets `phase_online_ticks` to zero, records `Resolved`, and
  increments the current run's `personal_crises_resolved` once. Controlled
  cleanup and True Death do not call it.

### Lifecycle and Safe Logout

* `PlayerStats[player_id].num_deaths` is the authoritative current-run hero
  death count. `hero_dead_system` increments it on `Added<StateDead>`,
  resurrection preserves it, and successful fresh-run creation replaces it
  with zero.
* Successful fresh-run creation in `sp_server/src/player.rs` clears the prior
  personal crisis and Goblin balance observation state only after setup
  succeeds. True Death in `sp_server/src/game.rs` removes the same player's
  active crisis at final run cleanup. Both are the correct player-scoped seams
  for clearing completion history.
* Safe Logout is already phase-based rather than Goblin-kind-based. Any
  pre-assault phase is eligible when the other safety checks pass, while
  `AssaultActive` blocks a request.
* `OfflineProtected` already makes `personal_crisis_system` return before
  pressure or clock mutation. Resume rebases `phase_started_tick` and
  `last_evaluated_tick` while leaving online counters unchanged. This freezes
  both the post-Goblin delay and Undead progression without another timer or
  Safe Logout code path.
* Ordinary disconnect leaves the crisis in the world but
  `Clients::is_player_online` is false. Advancing the crisis watermark without
  credit pauses all pre-assault online-only timing and avoids reconnect
  catch-up.

### Status, telemetry, and legacy Undead

* The existing flat version-one `crisis_status` packet already carries a
  string `kind`; the TypeScript interface accepts arbitrary strings and its
  network dispatcher forwards the packet without rejecting `undead`. No
  protocol or parser change is required.
* Server status construction currently hardcodes `goblin`, Goblin pressure
  limits, Goblin phase copy, the Goblin ready countdown, and the
  `dusk_or_night` launch window. Transition notices are also phase-only and
  Goblin-specific. These paths must become kind-aware.
* Existing preparation options are derived from live walls, defenders,
  equipment, and recovery supplies. They do not depend on Goblin identity and
  remain accurate for Undead `Preparing` and `AssaultReady`.
* `CrisisTelemetryState` and the opt-in `CrisisBalanceTelemetryState` predate a
  second crisis and represent the Goblin balance lifecycle. Goblin balance
  snapshots and preparation-action hooks currently gate only on phase. They
  must explicitly ignore Undead rather than treating its phase names as a
  second Goblin sample. Checkpoint 1 adds no replacement multi-crisis telemetry
  schema. Status delivery retains the last observed personal-crisis kind after
  active-state removal so an Undead clear or reconnect-clear cannot increment
  the older Goblin runtime packet counters.
* The retained legacy `PlayerCrisis.undead_incursion` and
  `undead_incursion_system` are part of `SurvivalDirectorMode::Legacy`. They
  are distinct from `SettlementCrisis { kind: Undead }` and remain unchanged.

### Headless and regression surfaces

* `HeadlessGame` uses the production plugins, authoritative `Clients`, normal
  player events, deterministic tick control, normal personal-assault combat,
  Safe Logout completion/resume, and current crisis/status inspection.
* Existing focused tests already prove global environmental continuity during
  Offline Protection, personal mode's lack of a scheduled dusk horde, legacy
  mode's scheduled dusk horde, the complete shipwreck/follow-up chain, generic
  pre-assault Safe Logout eligibility, active-assault disconnect behavior, and
  True Death/fresh-run isolation.
* Existing runner metrics are deliberately Goblin- and single-crisis-oriented.
  Checkpoint 1 smokes therefore inspect runtime crisis/history state, status
  packets, the assault-ID source, and attributed units directly. They do not
  add runner fields, execute matrices, or write JSON/CSV artifacts.

## Repository conflicts and selected resolutions

| Repository reality | Design conflict | Selected Checkpoint 1 resolution |
| --- | --- | --- |
| A new crisis is currently created lazily by the personal evaluator after a live run exists. | The design requires a new run to begin with Goblin and construction to accept an explicit kind. | Preserve the established lazy scheduling, but select the first kind from the centralized sequence and call `SettlementCrisis::new(CrisisKind::Goblin, tick)` explicitly. Fresh-run history is cleared before that evaluation. |
| `CrisisPhase` is shared and Goblin systems dispatch only on phase. | An Undead `AssaultReady` entry would currently allocate an ID and launch the Goblin composition. | Add defensive Goblin-kind checks to Goblin pressure, transition, assault, status-notice, and balance-telemetry paths. Undead Ready remains terminal. |
| Safe Logout resumes by rebasing absolute crisis ticks. | A separate inter-crisis deadline would require another rebase inventory entry and risk protected catch-up. | Reuse the resolved crisis's existing `phase_online_ticks`, reset by normal resolution, and the existing online-time watermark. |
| The TypeScript parser accepts `undead`, but the existing phase labels, desktop accessibility label, and pressure tooltip are Goblin-specific. | The protocol needs no change, but forwarding a valid `undead` kind would still present misleading Goblin copy in the client. | Keep the packet shape and parser unchanged; make the existing compact labels, accessibility label, and pressure tooltip kind-aware. Preserve Goblin wording, use Undead wording for `undead`, retain Goblin compatibility when the version-one kind is omitted, and use neutral wording for unknown future kinds. |
| Goblin balance telemetry has no crisis-kind dimension. | Reusing it for Undead would corrupt Milestone 3 evidence; generalizing it would violate the narrow/no-broad-telemetry limit. | Keep it Goblin-only with explicit kind guards. Validate Undead from focused state and packet tests instead. |
| The legacy director already has an unrelated automatic Undead tier. | Replacing it would alter legacy behavior and conflate two authorities. | Leave all legacy state, systems, registration, templates, and values untouched. |

## Selected runtime design

`CrisisKind` becomes ordered and contains exactly `Goblin` and `Undead`.
`crisis_kind_name` supplies the stable machine values `goblin` and `undead`.
Construction becomes `SettlementCrisis::new(kind, game_tick)`.

Runtime-only per-run history is:

```rust
pub struct PlayerCrisisHistory {
    pub completed: BTreeSet<CrisisKind>,
}

#[derive(Resource, Default)]
pub struct PersonalCrisisHistory {
    pub by_player: HashMap<i32, PlayerCrisisHistory>,
}
```

One sequence is authoritative:

```rust
const PERSONAL_CRISIS_SEQUENCE: [CrisisKind; 2] = [
    CrisisKind::Goblin,
    CrisisKind::Undead,
];
```

The next-kind helper returns the first sequence entry not present in the
player's completion set. Empty history therefore selects Goblin; completed
Goblin selects Undead; both completed selects none. A vacant runtime slot may
create only the helper's initial `Goblin` result. Undead is created only by
replacing the intact resolved Goblin after its delay; removing that delay holder
administratively cannot cause an instant Undead start. Because the active state
is one map entry, no second crisis is created while another entry exists.

Normal Goblin resolution inserts `Goblin` into the set. `BTreeSet::insert`
makes duplicate observation idempotent. Reconnect and resurrection do not touch
history. True Death and successful fresh-run replacement remove only that
player's history.

The resolved Goblin remains the active status for
`NEXT_PERSONAL_CRISIS_DELAY_TICKS = 60 * TICKS_PER_SEC`. Its existing
`phase_online_ticks` advances only while the owner is online. At the boundary,
and only when history selects Undead, the entry is replaced once with
`SettlementCrisis::new(CrisisKind::Undead, current_tick)` in `Dormant`.

## Provisional Undead pressure and phases

Undead pressure is recomputed from current authoritative facts on every active,
unprotected evaluation. It is not accumulated and is separate from the Goblin
calculator.

| Contributor | Pressure |
| --- | ---: |
| Goblin normally completed in current-run history | 20 |
| Introduction danger unlocked | 10 |
| `explore_poi` complete | 10 |
| `choose_expansion` complete | 10 |
| Current bound sanctuary level | 3 per level, capped at 15 |
| Current-run hero deaths | 10 per death, capped at 20 |
| Undead online-active time | 5 at 600 ticks, 10 at 1,800 ticks, 15 at 3,600 ticks |

The total is clamped to 100. Structure count, villagers, stored gold, corpse
state, Soulshards, graveyards, resources, and legacy crisis flags are not
Undead pressure inputs.

Transitions remain ordered and at most one can occur per evaluation:

| Transition | Pressure | Minimum online time in current phase |
| --- | ---: | ---: |
| `Dormant` → `Signs` | 20 | none |
| `Signs` → `Pressure` | 40 | 600 ticks / 60 seconds |
| `Pressure` → `Preparing` | 60 | 1,200 ticks / 120 seconds |
| `Preparing` → `AssaultReady` | 80 | 1,800 ticks / 180 seconds |

`AssaultReady` is terminal in Checkpoint 1. It has no grace countdown, launch
window, ID, generation, units, reward, or resolution path.

The existing status packet uses kind `undead` and the following exact title and
action-hint pairs:

| Phase | Title | Action hint |
| --- | --- | --- |
| Dormant | The Dead Are Quiet | Continue strengthening your settlement. |
| Signs | Restless Dead | Prepare healing supplies and defenders. |
| Pressure | Deathly Pressure | Strengthen defenses and remain near the settlement. |
| Preparing | Undead Gathering | Repair defenses, equip defenders, and prepare recovery supplies. |
| AssaultReady | Undead Incursion Imminent | Return to the settlement and finish preparing. |

## Exact Checkpoint 1 files

* `sp_server/src/game.rs` — crisis kinds/history/sequence, explicit
  construction, post-resolution transition, Undead pressure and phases,
  kind-aware status, Goblin assault guard, plugin resource, True Death cleanup,
  and Goblin-only balance hooks.
* `sp_server/src/player.rs` — successful fresh-run history cleanup and
  Goblin-only preparation telemetry hooks.
* `sp_server/src/crisis_balance.rs` — defensive Goblin-kind guards on the
  opt-in attribution, True Death, and engagement samplers.
* `sp_server/src/game_tests.rs` — focused sequence, history, pressure, phase,
  delay, presentation, kind-safety, disconnect, and resurrection regressions.
* `sp_server/src/headless.rs` — exactly two bounded production-schedule smoke
  scenarios, including True Death/fresh-run isolation, and narrow test-only
  state inspection.
* `sp_frontend/sp_ts/src/sp/core/crisisStatus.ts` and its focused test —
  kind-aware compact phase labels with Goblin-compatible and unknown-kind
  fallbacks.
* `sp_frontend/sp_ts/src/sp/desktop/ui/objectivesPanel.tsx` and its focused
  crisis/Safe Logout tests — kind-correct crisis-card accessibility and pressure
  labels without a new UI surface.
* `docs/undead_crisis_milestone.md` — this architecture, design, scope, and
  validation record.

`safe_logout.rs`, `network.rs`, legacy crisis systems, templates, resources,
recipes, farming, refining, structures, trade, villagers, database, map,
deployment, and infrastructure are audited regression surfaces and receive no
Checkpoint 1 semantic change.

## Checkpoint 1 validation plan

Focused tests cover explicit Goblin start, exactly-once history, sequence order,
active-crisis exclusion, online-only delay, protected freeze, deterministic
Undead pressure, separation from Goblin facts/telemetry, ordered phases, terminal
Ready with no ID or units, exact status kind/copy, player-isolated True Death
and fresh-run reset, retained Goblin behavior, Safe Logout, legacy scheduling,
no automatic personal dusk horde, and the introductory chain.

The only new headless smokes are:

1. normal Goblin resolution, 60 online seconds, Undead creation and progression
   through `AssaultReady`, with no attributed spawn or assault-ID allocation;
2. normal Goblin resolution, partial online delay, real Safe Logout protection
   and freeze, resume into Undead, reapply that production-created same-run
   protection identity to freeze the pre-assault Undead, then True Death and a
   fresh empty-history Goblin run from deliberately seeded stale run state.

No balance matrix, balance candidate, new runner, repeated sample, or generated
simulation artifact is part of this checkpoint.

## Checkpoint 2 architecture findings

### Shared personal-assault authority

* `SettlementCrisisState` remains the single runtime authority and still holds
  at most one `SettlementCrisis` per player. `CrisisKind`,
  `PersonalCrisisHistory`, and `PERSONAL_CRISIS_SEQUENCE` already describe the
  ordered `Goblin` then `Undead` run. Checkpoint 2 does not recreate that
  Checkpoint 1 foundation.
* `personal_crisis_system` remains responsible for online-only pre-assault
  pressure and phase progression. It advances a normally resolved Goblin into
  Undead after 60 credited online seconds and leaves Undead in
  `AssaultReady` for the assault system.
* `personal_crisis_assault_system` already owns the common launch grace,
  global dusk/night preference, bounded maximum wait, owner-online and
  valid-run requirements, Offline Protection barrier, anchor and position
  selection, assault-ID allocation, spawn generation, active-unit tracking,
  normal-death evidence, recovery-required behavior, and exactly-once
  resolution. The selected design dispatches this lifecycle by `CrisisKind`
  instead of adding an Undead-only scheduler.
* `NextCrisisAssaultId` allocates one monotonically increasing runtime ID only
  after the complete wave has valid templates and six valid positions.
  `CrisisAssaultUnit { owner_player_id, assault_id, spawn_generation }` remains
  the sole personal-assault attribution component.
* The existing personal anchor order remains bound monolith, primary completed
  structure, another completed structure, then valid-run hero fallback. The
  bounded 96-candidate selector rejects invalid, occupied, duplicate,
  unpathable, sanctuary-internal, and neighbouring-settlement positions. No
  Undead-specific settlement or map rule is added.

### Necromancer and Raise Dead reality

* `Encounter::spawn_necromancer` is the active specialized constructor. It
  installs the existing thinker, spell target, visible-corpse scorer,
  `RaiseDead`, `Minions`, flee behavior, and `Home`. The selected personal wave
  passes the initial Necromancer tile as `Home` and adds only the normal
  personal-assault attribution, viewshed, run tracking, and visibility
  notification around that constructor.
* `spawn_dormant_necromancer` and the scripted introductory corpse-hunt variant
  are separate encounter paths and are not used.
* The existing Raise Dead implementation creates a new object ID, appends that
  ID to `Minions`, schedules/spawns an ordinary Zombie, and removes the source
  corpse. It does not revive an object in place.
* Repository reality conflicts with the requested personal-corpse rule: normal
  Zombie and Skeleton combat deaths remain NPC entities carrying `StateDead`,
  while the legacy visible-corpse scorer and Raise Dead source query expect
  generic `ClassCorpse` objects and exclude NPCs. Without a narrow bridge, no
  normally defeated member of the personal wave could be a legitimate source.
* The selected bridge applies only when the caster has
  `CrisisAssaultUnit`. Corpse scoring, target installation, movement/action
  validation, and the final Raise Dead event all revalidate a `StateDead` NPC
  with the exact same owner, assault ID, and generation. Unattributed legacy
  Necromancers retain the existing generic-corpse path.
* At the successful personal Raise Dead boundary, the implementation uses the
  existing ordinary Zombie construction with a new ID, propagates the caster's
  exact attribution, applies the personal viewshed, appends the new ID once to
  `Minions`, `SettlementCrisis.assault_unit_ids`, and `RunSpawnedObjs`, records
  the removed source as normally defeated, and removes the source through the
  canonical `RemoveObj` observer so despawn, entity-map removal, and client
  visibility remain one operation. A same-pass personal-corpse claim rejects a
  duplicate queued event before it can allocate a second ID. The final boundary
  rechecks active Undead kind, phase, owner, assault ID, generation, tracked
  caster, tracked source, and source death before committing. This preserves
  one-corpse/one-raise semantics without a new corpse or minion system. The
  unattributed legacy Raise Dead branch retains its prior delayed spawn and
  removal sequence.

### Targeting, presence, and cleanup

* Ordinary personal-assault target scoring already admits only the owner's
  living human units and blocking defensive walls. Ordinary target movement
  and final melee damage already contain redundant ownership checks.
* The Necromancer's spell-target, corpse-target, movement/action, and final
  spell-event boundaries require the same owner for living targets or exact
  same-assault attribution for dead sources. A stale or cross-owner target
  fails safely instead of redirecting to a neighbouring settlement. Rejection
  also cancels any already-queued movement event, removes the stale target and
  in-progress marker, fails the event boundary, and returns the actor to its
  neutral state so a deferred move cannot cross the ownership boundary later.
* Safe Logout remains phase-based: it is available before `AssaultActive` and
  unavailable while the assault is active. Protected and offline time earn no
  launch progress. Once launched, ordinary disconnect does not remove or pause
  the assault; the same assault ID, generation, health, initial units, and
  raised units remain in the shared world. Reconnect reads the same status and
  does not replay launch.
* Existing combat allows connected helpers and villagers to damage attributed
  units. Attribution and history remain with the settlement owner; there is no
  participant scaling, ownership transfer, contribution score, or special
  helper reward.
* Active resolution compares tracked IDs with normal-death evidence and also
  checks for living entities carrying the exact active attribution. A tracked
  or attributed unit that disappears without normal-death evidence sets the
  existing recovery-required state instead of granting victory. A raised unit
  therefore blocks resolution exactly like an initial attacker.
* True Death captures the committed
  `(owner_player_id, assault_id, spawn_generation)` before clearing crisis
  state and removes attributed combatants only when all three values match.
  `RunSpawnedObjs` continues to clean non-attributed objects belonging to the
  ended run. Adding every raised ID to both tracking surfaces therefore covers
  initial and raised units without touching another assault or generation.
  Raise Dead is explicitly ordered after True Death, so a spell queued for the
  same update cannot commit a replacement after cleanup has removed the crisis
  authority. Controlled cleanup never calls the normal resolution helper,
  never grants score, and never inserts Undead history.

### Status, resolution, and legacy isolation

* The version-one `crisis_status` schema already contains kind-neutral
  `assault_ready`, countdown, preferred-window, active, resolved, and attacker
  count fields. Checkpoint 2 exposes the existing 300-online-tick ready grace
  and `dusk_or_night` preferred window for Undead without a packet-version or
  frontend schema change.
* Undead uses the active hint `Defeat the remaining undead. This assault
  continues if you disconnect.` The existing transition-notice stream emits
  `The undead incursion has begun. It will continue if you disconnect.` once at
  launch and `The undead incursion has been defeated.` once at resolution.
* `record_personal_assault_resolution` resolves the active crisis's actual
  `CrisisKind`. It records one generic `personal_crises_resolved` score,
  inserts the actual kind into the per-run history set, and remains guarded by
  `AssaultActive` plus `resolution_recorded`. The sequence helper returns
  `None` after both Goblin and Undead are complete.
* A resolved Undead entry is deliberately stable: it remains visible through
  reconnect, does not accrue phase or pressure time, does not create another
  crisis, and cannot add another score.
* Goblin keeps its existing three-unit composition and balance telemetry.
  Checkpoint 2 does not add Undead samples to the Goblin-only balance schema.
  The separate legacy `PlayerCrisis.undead_incursion` and
  `undead_incursion_system` stay gated by `SurvivalDirectorMode::Legacy` and
  are unchanged.

## Repository conflicts and selected Checkpoint 2 resolutions

| Repository reality | Checkpoint 2 conflict | Selected resolution |
| --- | --- | --- |
| The shared assault system had a blanket Goblin-kind return. | Undead could reach Ready but could never launch. | Retain one lifecycle and dispatch only composition and specialized spawn details by `CrisisKind`. |
| The Goblin helper accepted only ordinary generic NPC templates. | The fixed Undead wave needs five generic units plus the specialized active Necromancer. | Centralize standard composition by kind, validate all six templates/positions first, spawn the five ordinary units through `Encounter::spawn_npc`, and spawn the sixth through `Encounter::spawn_necromancer`. |
| Normal dead assault NPCs are not `ClassCorpse` objects. | The existing Necromancer could not raise a legitimate personal-wave Zombie or Skeleton. | For an attributed Necromancer only, admit a normally dead NPC with the exact same attribution at every corpse boundary; leave the legacy corpse path unchanged. |
| Raise Dead creates a new ID through delayed event behavior. | The new unit would otherwise be absent from crisis and run tracking, allowing premature resolution or stale cleanup. | At successful personal Raise Dead, propagate attribution and append the new ID exactly once to `Minions`, active assault tracking, and `RunSpawnedObjs`; mark the consumed source defeated before removal. |
| ECS commands defer spawned entities and removal until schedule application. | Duplicate spell events could allocate twice, while a Raise Dead event due in the same update as True Death could outlive run cleanup. | Claim a personal source corpse once per Raise Dead pass, use the canonical removal observer, order Raise Dead after True Death, and keep the assault evaluator after Raise Dead. |
| Necromancer spell and movement systems were written for ambient/legacy combat. | A stale target could cross the personal owner boundary after initial scoring. | Revalidate owner or exact corpse attribution during target installation, movement/action execution, and final spell/damage application. |
| Successful fresh-run setup can encounter an attributed orphan after overlapping old cleanup, when no prior crisis triple remains authoritative. | Exact-triple matching is impossible at that last-resort seam, but a fresh run must not inherit stale attributed units. | True Death uses exact triple matching. The pre-existing successful-new-run orphan sweep remains deliberately owner-scoped and runs only after duplicate-live-run creation has been rejected; it removes stale units for that player and never another player's attribution. |
| Runtime and balance telemetry are Goblin-specific. | Recording Undead into them would corrupt Milestone 3 evidence or require broad telemetry. | Keep those paths explicitly Goblin-only; validate Undead through focused state assertions and three bounded smokes. |
| The existing kind-aware client already displays the reused status fields. | A frontend change would exceed the narrow feature boundary. | Keep the packet at version one and make no frontend change unless validation proves a display defect. |

## Selected Checkpoint 2 runtime configuration

The fixed initial composition is exactly:

```text
3 Zombies
2 Skeletons
1 Necromancer
```

The centralized ordinary-unit constant is:

```rust
const UNDEAD_ASSAULT_STANDARD_UNITS: [&str; 5] = [
    "Zombie",
    "Zombie",
    "Zombie",
    "Skeleton",
    "Skeleton",
];
```

The Necromancer is appended as the sixth validated template and spawned
separately through the active specialized constructor. All six initial units
receive the same newly allocated assault ID and incremented spawn generation,
personal-assault viewshed range 14, normal visibility notification, and
run-spawn tracking.

Both crisis kinds use the existing launch policy:

| Rule | Configuration |
| --- | --- |
| Ready grace | 300 credited online ticks / 30 seconds |
| Preferred global window | dusk or night: day ticks 2,000–2,399 or 0–399 |
| Maximum wait | 1,200 credited online Ready ticks / 120 seconds |
| Offline owner | no launch and no credited progress |
| Offline Protection | no launch and no credited progress |
| Successful launch | one ID, one generation, one complete attributed wave |
| Failed validation or spawn | remain Ready; no committed active assault |

No template statistics, AI values, loot, XP, health, damage, defence, speed,
vision other than the existing personal viewshed policy, hero values, villager
values, structure values, items, resources, recipes, professions, or economy
rules are changed.

## Exact Checkpoint 2 files

* `sp_server/src/game.rs` — kind-dispatched composition and spawn, specialized
  Necromancer insertion, shared launch/status/notices, attributed Raise Dead
  finalization, active-unit recovery checks, kind-correct resolution, stable
  final Undead state, final spell ownership validation, and schedule ordering.
* `sp_server/src/ai/npc/npc.rs` — narrow attributed-Necromancer corpse scoring,
  corpse target installation, same-owner spell target checks, and stale
  movement/action validation while retaining unattributed legacy behavior.
* `sp_server/src/game_tests.rs` — focused composition, all-or-nothing spawn,
  launch, attribution, Raise Dead, resolution, status, idempotency, ownership,
  presence, history, and cleanup regressions.
* `sp_server/src/headless.rs` — exactly three bounded, single-run Checkpoint 2
  smoke scenarios using the production schedule and normal combat/death path.
* `docs/undead_crisis_milestone.md` — this architecture, conflict, selected
  design, scope, validation, and limitation record.

`sp_server/src/encounter.rs`, `safe_logout.rs`, `network.rs`, frontend code,
templates, the legacy Undead system, map, database, deployment, resources,
recipes, farming, refining, trade, structures, and villager production are
audited/reused surfaces and require no selected Checkpoint 2 semantic change.

## Focused and regression validation plan

Focused tests cover:

* exact three-Zombie, two-Skeleton, one-Necromancer composition;
* complete prevalidation and no partial spawn/ID commitment on failure;
* all-six attribution, viewshed, active IDs, run IDs, and Necromancer `Home`;
* the same 300-tick grace, preferred window, 1,200-tick maximum wait,
  offline/protection freeze, and duplicate-launch rejection for both kinds;
* Undead ready countdown/window fields plus exact active/resolved presentation;
* same-assault and same-generation corpse eligibility at selection, action,
  and final event boundaries;
* one-corpse/one-new-ID Raise Dead tracking, `Minions`, missing-unit recovery,
  and resolution waiting for living raised units;
* owner-only ordinary, spell, corpse, movement, action, and final damage rules;
* disconnect continuation, reconnect identity, helper combat without ownership,
  exactly-once actual-kind history/score, terminal sequence, and stable resolved
  Undead;
* matching True Death/fresh-run cleanup without completion and isolation from a
  neighbour's personal assault or legacy Undead;
* retained Goblin, Safe Logout, introductory encounter, environment, economy,
  crafting, farming, refining, villager, and headless behavior.

The required commands and final results are:

| Command, run from `sp_server/` | Result |
| --- | --- |
| `cargo fmt --check` | Passed with exit code 0. |
| `cargo check` | Passed with exit code 0; the existing warning backlog remains. |
| `cargo test --lib undead_crisis` | Passed: 16 passed, 0 failed, 514 filtered out in 15.83 seconds. This includes all three Checkpoint 2 smokes. |
| `cargo test --lib personal_crisis` | Passed: 7 passed, 0 failed, 523 filtered out in 1.13 seconds. |
| `cargo test --lib safe_logout` | Passed: 65 passed, 0 failed, 465 filtered out in 47.88 seconds. |
| `cargo test` | Passed: library 530/530; binary targets 9/9, 17/17, and 5/5; integration day tests 6/6; other targets 0/0; doc tests 0 failed with 1 ignored. Total: 567 passed, 0 failed, 1 ignored. |
| `cargo clippy --all-targets --all-features` | Passed with exit code 0; warning-only output from the repository's existing Clippy backlog remains. |

`git diff --check` also passed. No balance runner, matrix, long headless
batch, report writer, or artifact-generating command was executed.

No balance matrix, preparation matrix, candidate comparison, statistical class
test, long headless batch, report-generating runner, JSON artifact, or CSV
artifact is part of this checkpoint.

## Exactly three bounded Checkpoint 2 smoke scenarios

1. **Full Undead lifecycle:** establish Undead Ready through the normal
   Goblin-to-Undead sequence, complete real launch timing, assert the exact six
   specialized/generic units and attribution, allow one legitimate same-assault
   Raise Dead, prove the new ID remains tracked, normally defeat every
   attributed unit, and assert one Undead resolution and a completed sequence.
2. **Disconnect and helper:** launch normally, obtain a legitimate raised unit,
   disconnect the owner ordinarily, prove the same initial/raised units, health,
   assault ID, and generation remain, let a connected helper normally kill at
   least one, reconnect the owner, finish the wave, and assert unchanged owner
   plus one resolution.
3. **Isolation and cleanup:** launch near another settlement, prove the
   Necromancer ignores neighbouring assets and unrelated corpses while it can
   raise only an exact same-assault corpse, trigger True Death/run cleanup,
   prove all matching initial/raised units are removed without score/history,
   preserve the neighbour, and prove the fresh run has empty history and starts
   again with Goblin.

Each smoke was also executed individually after its final edit:

1. `cargo test -q undead_crisis_checkpoint2_smoke_full_lifecycle_and_raise_dead -- --nocapture`
   passed 1/1. The real Necromancer AI selected a normally dead same-assault
   Zombie; duplicate same-corpse events produced one raised identity and one
   entry in `Minions`, the assault roster, and `RunSpawnedObjs`; the raised
   Zombie alone kept remaining attackers at one until its normal death resolved
   the crisis.
2. `cargo test -q undead_crisis_checkpoint2_smoke_disconnect_reconnect_and_helper`
   passed 1/1. The same initial and raised roster survived ordinary owner
   disconnect; a connected helper killed an attributed unit without taking
   ownership; reconnect preserved assault identity and completion occurred
   once.
3. `cargo test -q undead_crisis_checkpoint2_smoke_isolation_and_true_death_cleanup -- --nocapture`
   passed 1/1. Neighbour assets and corpses were rejected; a same-update queued
   Raise Dead lost to True Death without allocating an ID or granting
   completion; canonical removal visibility was delivered on the next update;
   exact-triple True Death cleanup preserved both the neighbour and a
   deliberately stale generation fixture, then successful fresh-run setup
   removed that owner-scoped orphan before beginning again with Goblin.

Exactly these three bounded, single-run Checkpoint 2 smoke scenarios were
executed. No fourth Checkpoint 2 smoke exists.

## Known limitations and deferred work

* Personal crisis history, assault identity, Offline Protection, and active
  world state remain runtime-only across a process restart. Durable restart
  persistence is deferred.
* The Necromancer deliberately retains its current spell selection, flee,
  `Home`, and `Minions` behavior. This checkpoint validates integration and
  ownership, not whether those values produce final solo/class balance.
* Goblin Milestone 3 acceptance remains separate. Checkpoint 2 does not tune or
  claim to close Goblin balance.
* The personal dead-NPC eligibility bridge is intentionally limited to an
  attributed Undead Necromancer and exact attribution. It does not redesign the
  ambient corpse model or change the legacy Necromancer's corpse semantics.
* No offline production, assault suspension, new client panel, distress beacon,
  regional crisis, larger map, 25-player world, cross-world interaction, new
  reward, new resource, or third personal crisis is included.

> Milestone 4 completes the functional Undead crisis using three Zombies, two Skeletons, one Necromancer, and the Necromancer’s existing spellcasting and Raise Dead behavior. It does not claim final combat balance.

The focused, full, Clippy, and exactly three bounded smoke validations above
all pass. Checkpoint 2 and Milestone 4 are complete.
