# Milestone 4 — Second Personal Crisis: Undead Incursion

## Status and checkpoint boundary

Checkpoint 1, **Multi-Crisis Foundation and Undead Pre-Assault Progression**, is
the only implementation authorized by this branch. It extends the existing
runtime personal-crisis state through an Undead `AssaultReady` phase and stops
there. It does not spawn an Undead assault, allocate an Undead assault ID, grant
an Undead reward, or add a new runner or balance artifact.

The milestone has two checkpoints:

1. **Checkpoint 1 — multi-crisis foundation and pre-assault progression:** add
   explicit crisis kinds, per-run completion history, ordered Goblin-to-Undead
   sequencing, the online-only inter-crisis delay, deterministic Undead
   pressure, ordered Undead phases through `AssaultReady`, kind-correct status
   presentation, cleanup, focused tests, and two bounded headless smokes.
2. **Checkpoint 2 — Undead assault lifecycle and final validation:** define and
   implement the Undead launch, attributed units, assault identity, combat,
   resolution, reward, disconnect/helper behavior, cleanup, and final feature
   validation. None of that work belongs to Checkpoint 1.

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

## Work deferred to Checkpoint 2

Checkpoint 2 must define and validate the Undead assault composition, launch
conditions, settlement anchor/spawn behavior, attribution and assault identity,
combat/AI, disconnect and helper continuation, normal resolution, reward and
score behavior, cleanup of active units, status/notice behavior after Ready,
and final end-to-end validation. It must preserve the existing Safe Logout,
ownership, legacy-director, economy, map, and persistent-world contracts.

Undead balance tuning, regional crises, offline production, durable runtime
history persistence, broader client crisis UI redesign, distress beacons, larger maps,
25-player worlds, and cross-world systems remain outside Checkpoint 1.
