# Persistent Personal Crisis Foundation

## Status

All four checkpoints are implemented and validated. The persistent personal-
crisis foundation is complete in the current runtime architecture. This does
not complete safe logout, offline protection, regional crises, larger worlds,
or the broader persistent-world redesign.

## Current gameplay contract

Global day/night controls environmental conditions.

Personal settlement danger is controlled by the player's goblin crisis.

Before the assault launches, crisis timing advances only while the player is
online.

Once the assault launches, it remains active in the persistent world and
continues if the player disconnects.

Personal-crisis attackers may affect only the owning player's settlement and
associated units.

Defeating all attributed attackers resolves the crisis exactly once.

## Checkpoint 4 implementation record

Checkpoint 4 adds only the crisis protocol, delivery synchronization, compact
desktop Survival Thread presentation, runtime telemetry, runner reporting, and
final validation. It does not change the Checkpoint 2 state machine, the
Checkpoint 3 assault lifecycle, the director default, the map, the economy, or
the committed `AssaultActive` disconnect rule.

### Architecture findings

* The server's authoritative phases remain `Dormant`, `Signs`, `Pressure`,
  `Preparing`, `AssaultReady`, `AssaultActive`, and `Resolved`. Goblin pressure
  remains clamped to `0..=100`, with transition thresholds 20, 45, 70, and 90.
  The per-phase online minima remain 0, 600, 1,200, and 1,800 ticks at ten
  ticks per second. The launch grace remains 300 online ticks, the maximum
  online wait remains 1,200 ticks, and the preferred window remains dusk or
  night.
* `SettlementCrisisState` remains the gameplay source of truth. The status
  builder reads it without mutating phase, pressure, warning, assault identity,
  generation, timers, or unit bookkeeping. During `AssaultActive`, logical
  remaining attackers are the current generation's tracked object IDs minus
  IDs with authoritative normal-death evidence.
* Outgoing packets use `ResponsePacket`'s existing internally tagged Serde
  representation. A flattened, `skip_serializing_none` crisis snapshot gives a
  stable top-level `packet: "crisis_status"` payload and omits absent optional
  fields instead of sending `null`.
* Authentication inserts a fresh connection UUID and `Client` before queuing
  `PlayerEvent::Login`. The player message broker schedules the existing
  delayed `GameEventType::Login`; `game_event_system` sends map/world state and
  requests initial perception at that established synchronization point.
  Crisis login synchronization is attached there rather than inventing a
  second login path.
* One player may briefly have more than one connection record during session
  replacement. A player-only last-snapshot cache would therefore either miss a
  reconnect or resend to the old connection. Delivery is cached per live
  connection UUID and records its player ID and last successfully sent
  snapshot.
* `AssaultActive` remains the commitment point. The lifecycle continues to
  evaluate matching attributed units while the owner is offline, and ordinary
  combat by villagers or connected helpers may resolve the assault. Status
  delivery observes this state but never launches, heals, despawns, rebuilds,
  pauses, or resolves units.
* True Death and successful new-run setup already remove the old run's crisis
  state. The delivery system runs after True Death, crisis evaluation, assault
  lifecycle, and the delayed login event, so the same authoritative mapping
  emits a clear state or the new run's fresh state.
* The desktop Survival Thread is component-local event-driven UI. It already
  owns objective, threat, discovery, and compact expansion state; adding crisis
  state there avoids an unrelated Redux or global-store redesign. The generic
  threat-risk details were already intentionally hidden and remain hidden.
* The headless harness uses the production gameplay plugins, a real `Clients`
  map, production `PlayerEvent` input, and deterministic direct tick control.
  It previously drained all outgoing packets, so Checkpoint 4 retains only the
  sparse `crisis_status` stream by default and provides bounded opt-in capture
  for short notice assertions.
* Crisis state and phase telemetry remain runtime-only, matching the existing
  prototype run architecture. No current coherent snapshot or database path
  persists the complete introduction, start assignment, objectives, assault
  identity, and per-run cleanup graph, so Checkpoint 4 deliberately adds no
  partial database migration.

### Files changed for Checkpoint 4

* `sp_server/src/network.rs` — versioned response snapshot, flattened packet
  variant, and serialization tests.
* `sp_server/src/game.rs` — centralized presentation, meaningful-change policy,
  per-connection delivery cache, delayed-login synchronization, transition
  notices, phase/assault telemetry, and schedule ordering.
* `sp_server/src/game_tests.rs` — presentation, throttling, delivery, reconnect,
  clear-state, legacy-mode, and direct-system fixture coverage.
* `sp_server/src/headless.rs` — sparse packet capture, production-faithful
  reconnect helper, runtime metrics, and the four end-to-end scenarios.
* `sp_server/src/bin/headless_runner.rs` — appended CSV/JSON fields and aggregate
  crisis reporting.
* `sp_frontend/sp_ts/src/sp/core/crisisStatus.ts` — typed display and compact-
  expansion helpers.
* `sp_frontend/sp_ts/src/sp/core/crisisStatus.test.ts` — 26 focused pure-helper
  assertions.
* `sp_frontend/sp_ts/src/sp/core/network.ts` — typed crisis response packet and
  dispatcher integration.
* `sp_frontend/sp_ts/src/sp/core/networkEvent.ts` — `CRISIS_STATUS` event name.
* `sp_frontend/sp_ts/src/sp/desktop/ui/objectivesPanel.tsx` — compact crisis
  card, lifecycle clearing, responsive collapsed priority, and accessibility.
* `docs/persistent_crisis_milestone.md` — corrected final contract and this
  implementation/validation record.

`player.rs`, `event.rs`, `headless_bot.rs`, `global.ts`, Notice rendering,
mobile UI, package configuration, the economy, database, map, deployment, and
infrastructure were inspected or reused and did not require Checkpoint 4
semantic changes.

### Packet schema and presentation

The wire payload is flat and versioned from one:

| Field | Wire type | Meaning |
| --- | --- | --- |
| `packet` | `"crisis_status"` | Stable tagged-packet discriminator |
| `version` | integer | Schema version, currently `1` |
| `exists` | boolean | Whether this run has a personal crisis |
| `kind` | optional string | Stable machine kind, currently `goblin` |
| `phase` | optional string | Stable snake-case phase |
| `pressure`, `pressure_max` | optional integer | Exact server pressure and server model maximum |
| `title`, `summary`, `action_hint` | optional string | Centralized player-facing copy |
| `severity` | optional string | `quiet`, `low`, `medium`, `high`, `crisis`, or `resolved` |
| `warning` | boolean | Authoritative warning state |
| `assault_ready`, `assault_active`, `resolved` | boolean | Direct display state |
| `remaining_attackers`, `total_attackers` | optional integer | Logical current-generation counts while active |
| `preparation_seconds_remaining` | optional integer | Ceil-rounded ready-grace countdown |
| `preferred_launch_window` | optional string | `dusk_or_night` while ready |
| `continues_while_disconnected` | boolean | True only for `AssaultActive` |

No ECS entity ID, object ID, assault ID, generation, target, or cleanup flag is
exposed. A no-crisis snapshot contains `version: 1`, `exists: false`, the false
state booleans, and no optional crisis fields. TypeScript accepts unknown
future fields and maps unknown future phase values to a neutral display.

The single server presentation mapping is:

| Phase | Title | Severity | Action emphasis |
| --- | --- | --- | --- |
| `dormant` | No Organized Threat | quiet | Continue establishing your camp |
| `signs` | Goblin Signs | low | Build supplies and improve defenses |
| `pressure` | Goblin Pressure | medium | Prepare weapons, healing, walls, and defenders |
| `preparing` | Raiders Gathering | high | Finish repairs, equipment, and supplies |
| `assault_ready` | Goblin Raid Imminent | crisis | Return and prepare for the assault |
| `assault_active` | Settlement Under Attack | crisis | Defeat attackers; disconnect does not stop it |
| `resolved` | Goblin Raid Defeated | resolved | Recover, repair, and rebuild |

### Status delivery, deduplication, and warnings

`CrisisStatusDeliveryState` stores the last successfully sent snapshot for each
live connection UUID. Closed, replaced, or player-mismatched connection entries
are purged. A channel-full or failed send is not cached and remains retryable.
Every structural change sends immediately, including creation, clear state,
phase, warning, ready/active/resolved flags, attacker counts, launch window, and
copy. Pressure sends after a cumulative difference of at least five from the
last successful packet. The ready countdown sends after a cumulative five-
second difference. Phase transitions, login, reconnect, launch, and resolution
therefore always carry exact current values without a per-tick stream.

An independent observed-phase map is used only to identify actual transitions;
it is updated even while the owner is offline. The existing Notice channel
therefore emits exactly the following major notices to an online owner and does
not reconstruct historical notices from a login snapshot:

* Preparing: `Goblin raiders are gathering. Prepare your settlement.`
* AssaultReady: `A goblin raid is imminent.`
* AssaultActive: `The goblin assault has begun. It will continue if you disconnect.`
* Resolved: `The goblin assault has been defeated.`

Pressure updates do not create notices. The structured status packet remains
authoritative.

### Login, reconnect, cleanup, and legacy behavior

The delayed Login event marks the player for synchronization. Each new live
connection receives one exact current snapshot; duplicate Login events for an
already cached connection do not resend it. Reconnect does not modify crisis
state and does not invoke launch logic, replace the assault ID or generation,
heal units, restore grace, or replay the launch notice. An active reconnect
reports the existing remaining attackers and the disconnect-continuation flag.
If villagers or a helper resolve the assault offline, the first reconnect
snapshot is `resolved`.

True Death removal sends `exists: false` to a still-live connection, and fresh
run creation sends its new `dormant` state with zero pressure and no attacker
counts. No old phase, pressure, assault identity, or unit count is copied into
the new run. In legacy director mode the same protocol sends only a no-crisis
clear snapshot; it does not present legacy automatic threats as a personal
crisis. Legacy scheduling itself remains unchanged.

### Client behavior

The network dispatcher emits the complete typed packet through
`NetworkEvent.CRISIS_STATUS`. The Survival Thread stores it locally, clears it
on `exists: false`, True Death, class/new-run selection, first-login reset, and
hero-ID replacement, and preserves same-hero phase history through an ordinary
reconnect. It renders whenever an objective or crisis exists, so a crisis card
survives completion of all objectives.

The card displays server title, human phase, summary, action hint, clamped
server-pressure value and bar, ready countdown, active remaining/total attacker
count, resolution state, and the visible sentence `The assault continues while
disconnected.` The compact collapsed header prioritizes imminent or active
crisis state. `Preparing`, `AssaultReady`, and `AssaultActive` each auto-expand
once on entry; a duplicate same-phase packet respects a player's later manual
collapse.

The wide and compact desktop layouts continue sharing the existing panel and
pointer-event behavior. The card uses a labelled region, phase text in addition
to static color, restrained phase-specific borders, phase-change `aria-live`,
and an accessible labelled progress bar with a readable value. No flashing or
continuous urgent animation is added. The generic verbose threat-risk list
remains hidden.

### Headless telemetry and runner schema

The runtime telemetry records the highest phase; first ticks for Signs,
Pressure, Preparing, AssaultReady, AssaultActive, and Resolved; assaults
launched and resolved; duplicate launch attempts; successful status packets;
and login snapshots. Final metrics add current phase, pressure, and logical
units remaining. This is observation-only state and never drives gameplay.

The runner preserves the original first 31 CSV/JSON fields in their original
order and appends 17 fields: `crisis_highest_phase`, `crisis_final_phase`,
`crisis_final_pressure`, six phase-entry ticks, assaults launched/resolved,
units remaining, status packets, login snapshots, duplicate assaults,
personal-mode automatic dusk hordes, and a crisis invariant result. The console
adds launch/resolve rates, completion, duplicate and dusk-horde totals,
invariant failures, highest-phase counts, and mean phase-entry ticks where a
sample exists. Panic reporting remains present.

### Tests and final validation

Final server validation from `sp_server/`:

* `cargo fmt --check` — passed after applying Rustfmt to the new Rust code.
* `cargo check` — passed; its emitted output retained the existing unused
  `net_error` macro warning.
* `cargo check --bin headless_runner` — passed with the existing 74-warning
  library set.
* `cargo test checkpoint4 -- --nocapture` — passed 14 tests, with 312 filtered
  out. The ordered functional scenario emitted phases `dormant`, `signs`,
  `pressure`, `preparing`, `assault_ready`, `assault_active`, and `resolved`;
  active remaining counts included 3, 2, and 1; ten status packets, one initial
  login snapshot, and zero duplicate assaults were recorded. The four major
  transition notices each emitted exactly once under duplicate evaluation.
* `cargo test checkpoint2 -- --nocapture` — passed its focused headless test,
  with 324 filtered out.
* `cargo test checkpoint3 -- --nocapture` — passed all 21 focused tests, with
  304 filtered out.
* `cargo test` — passed all 326 library tests and all 6 day-system integration
  tests; the one documentation test remains intentionally ignored.
* `cargo clippy --all-targets --all-features` — passed with the existing lint
  backlog: 1,332 library warnings and 1,345 library-test warnings, including
  1,332 duplicates. The one new runner closure warning found on the first pass
  was corrected before the final pass.
* `cargo run --bin headless_runner -- 3 6000` — completed all three bounded
  runs at tick 6,007 with `MaxTicks`; all reached Signs, none naturally reached
  later phases, and launches, resolutions, duplicate assaults, personal-mode
  automatic dusk hordes, and panics were all zero. Mean absolute Signs entry
  tick was 6,611; later phase means were correctly reported as unavailable.
  CSV and JSON each contained three rows and the same 48-field schema, with the
  original first 31 columns unchanged.
* `git diff --check` — passed as the final repository check.

Final client validation from `sp_frontend/sp_ts/`:

* The focused pure-helper TypeScript compile and Node execution passed all 26
  assertions.
* `npx tsc --noEmit --skipLibCheck` — passed.
* `npx webpack --mode production --stats=errors-warnings` — both desktop and
  mobile production bundles compiled. Each retained the three existing asset-
  size, entrypoint-size, and code-splitting performance warnings; output sizes
  were 3.34 MiB desktop and 2.41 MiB mobile.
* Plain `npx tsc --noEmit` remains blocked by the pre-existing generated
  `src/phaser.d.ts` and package Phaser declaration collision (`TS6200` and
  `TS2432`) plus the generated file's missing `./matter` reference (`TS2688`).
  Checkpoint 4 did not alter generated Phaser declarations or TypeScript
  configuration; the skip-lib-check compile and production webpack builds are
  the finite supported validations used here.

Two intermediate server failures improved the final fixtures. The first new
normal-progression headless test crossed the maximum-wait fallback during a
manual tick jump; its deterministic ready watermark was corrected to exactly
one grace interval before dusk. The first full `cargo test` then passed 323 of
325 library tests and exposed two direct-system fixtures missing the new
runtime telemetry resource; the shared fixture now initializes it, and the
final full suite passed. The first Rustfmt check also reported formatting-only
differences that were formatted before final validation.

### Known limitations and deferred work

* Crisis and telemetry state remain process-memory-only; there is no database
  migration or coherent full run snapshot in this checkpoint.
* The repository has no configured TypeScript component/DOM test harness. Pure
  display/expansion helpers are tested and both production bundles compile, but
  the card was not browser-automation tested.
* The new card is in the requested desktop Survival Thread. The distinct mobile
  objective UI remains unchanged.
* The bounded runner sample did not naturally reach Pressure or an assault. It
  validates telemetry shape and personal-mode invariants but supports no later-
  phase pacing or balance conclusion; deterministic headless scenarios provide
  the functional lifecycle validation.
* Ordinary disconnect remains unsafe after `AssaultActive`: combat and world
  damage continue. Missing tracked-unit recovery still requires a future
  deterministic administrative policy.
* Explicit safe logout, offline protection/production/shops, distress beacons,
  assistance rewards, participant scaling, party/guild systems, regional
  crises or strongholds, more crisis families, Fire Dragon redesign, a full
  quest journal or leaderboard redesign, larger maps, 25-player worlds,
  cross-world systems, database persistence redesign, and resource-system
  simplification remain out of scope.

## Checkpoint 3 implementation record

Checkpoint 3 implements only the first personal goblin assault lifecycle. It
does not add a crisis-status protocol packet, client UI, distress beacons,
assistance rewards, regional crises, new crisis families, offline production,
larger maps, or cross-world systems.

### Architecture findings

* Checkpoint 2's `personal_crisis_system` in `sp_server/src/game.rs` is a
  fact-aggregation and phase-timing evaluator. It owns the ordered transition
  through `AssaultReady`, has no `Commands`, and already uses
  `Clients::is_player_online` plus a monotonic `last_evaluated_tick` watermark.
  Checkpoint 3 retains that separation and adds a second PersonalCrisis-only
  lifecycle system rather than reimplementing pressure or phase progression.
* `SettlementCrisisState`, the introduction resources, `RunSpawnedObjs`, start
  assignments, and similar per-run resources are not reflected or restored by
  the prototype dynamic-scene path. Personal crisis state and its new ID source
  therefore remain runtime-only, matching the current Checkpoint 2 architecture.
* A hero's `BoundMonolith.id` is the authoritative sanctuary link.
  `SanctuaryZones` is keyed by monolith ID, while the legacy
  `crisis_spawn_pos` chooses the sanctuary nearest a fallback and its
  no-sanctuary helper can return the invalid centre fallback. The personal
  assault consequently uses a separate exact-anchor, fail-closed spawn helper.
* `Encounter::spawn_npc` creates an ordinary combat NPC, pre-generates its
  normal loot in `Inventory`, installs the chase thinker, registers `Ids` and
  `EntityObjMap`, and returns the entity. It hard-codes NPC vision to two tiles,
  so personal assault units replace that component with the existing horde
  hunting distance of 14 and explicitly trigger `NewObj`.
* The only loaded goblin-family templates suitable for this wave are `Wolf
  Rider` (75 HP, 6 base damage, 5 defence, vision 4, 300 kill XP) and `Goblin
  Pillager` (55 HP, 5 base damage, 4 defence, vision 3, 250 kill XP). There is
  no ordinary `Goblin` object template.
* The specialized Wolf Rider and Goblin Pillager helpers are not safe as the
  first personal assault brain. Their scorers search all players' structures
  without owner filtering; the steal scorer also reads the NPC inventory while
  deciding whether a target has loot. More importantly, a Pillager torch event
  creates indefinite `Burning` damage. The personal assault therefore retains
  the ordinary owner-filtered combat brain and does not install theft or torch
  scorers.
* Normal combat sets `State::Dead`, adds `StateDead`, records `LastAttacker`,
  awards skill XP, and leaves the pre-generated inventory on the corpse.
  `run_score_kill_tracking_system` credits the actual killer. Dead NPCs remain
  queryable for 100 ticks when empty and 500 ticks when carrying loot, so the
  attribution tracker can observe normal death before ordinary cleanup.
* Incoming player combat is buffered in `PlayerEvents`, so a socket can close
  after an input packet is accepted but before `attack_system` consumes it.
  The source player's client presence remains mandatory for queued Attack,
  Ability, Combo, and Block inputs. The crisis owner's presence is not a target
  gate after commitment: a connected helper can attack an attributed unit and
  the owner's villagers can defend while the owner is offline.
* Personal-assault target ownership has two enforcement points. Scoring admits
  only the owner's human units and blocking walls, and the requested action is
  revalidated immediately before its target is installed or damage is applied.
  Stale or forged cross-owner targets are cleared and fail safely. These gates
  depend on attribution, not owner presence.
  In the current ordinary combat scorer, structures other than blocking walls
  are skipped. Monoliths use `MONOLITH_PLAYER_ID` rather than the owning human
  player's ID, so personal-assault targeting rejects every monolith instead of
  guessing sanctuary ownership and risking another player's sanctuary.
* `RemoveObj` is the correct visible controlled-removal path and guards a
  duplicate observer invocation through `EntityObjMap`. It does not clear
  `Ids`, pending `MapEvents`, or `RunSpawnedObjs`; the retained True Death and
  run-recycling paths clear those explicitly before triggering removal.
  Controlled cleanup never adds `StateDead`, so it generates no loot, kill
  score, wildness reduction, or crisis completion. Ordinary disconnect is not
  a controlled-cleanup event.
* True Death removes per-run state after its existing ten-second delay, drains
  `RunSpawnedObjs`, filters pending map events, and recycles the start location.
  Checkpoint 3 removes the historical radius-based hostile sweep: cleanup now
  removes only dead-player-owned objects, objects explicitly tracked for that
  run, and same-owner attributed assault units. Nearby unrelated hostiles and
  another player's attributed assault therefore survive recycling.
  Intro phase-one/spider enemies and the retained legacy rat, wolf, goblin,
  undead, Pillager, nightly, and legendary spawns are explicitly entered in
  `RunSpawnedObjs`, replacing reliance on the removed proximity sweep.
* `new_player_system` previously accepted another `NewPlayer` event for a
  player who already had an assigned start or live hero. It now rejects that
  duplicate before setup so a queued or repeated class-selection event cannot
  erase or replace an active run's assault state.
* `headless.rs` already provided real client presence, disconnect/reconnect,
  direct tick control, full plugin scheduling, and normal `PlayerEvent` input.
  The Checkpoint 3 deterministic scenario can inspect attributed unit identity,
  health, position, and targets across disconnect/reconnect and can drive
  ordinary owner, helper, and villager combat. Checkpoint 4 subsequently
  extended its packet capture and the runner's CSV/JSON metrics.

### Files changed for Checkpoint 3 and its corrective addendum

* `sp_server/src/game.rs` — assault data, monotonic ID resource, timing policy,
  anchor/spawn selection, lifecycle system, one-time run completion record,
  committed-offline continuation, conservative missing-unit recovery state,
  ordering, and isolated True Death cleanup.
* `sp_server/src/ai/npc/npc.rs` — a narrow target-scorer rule that makes an
  attributed personal-assault unit select only its owning player's human units
  and owned blocking walls, plus action-boundary ownership revalidation for
  requested and current targets and the focused target-owner tests. Ordinary
  and legacy NPC targeting is unchanged.
* `sp_server/src/ai/villager/villager.rs` — owner villagers retain ordinary
  retaliation against attributed assault units while their owner is offline.
* `sp_server/src/ai/villager/villager_tests.rs` — proves ordinary villager
  defence continues for an offline owner.
* `sp_server/src/player.rs` — successful hero recreation removes any attributed
  orphan from that same player's prior run; duplicate run creation is rejected;
  and queued Attack, Ability, Combo, and Block inputs require a live source
  client. A connected helper's target is not rejected merely because the
  crisis owner is offline.
* `sp_server/src/game_tests.rs` — timing, ID, template, anchor, and fail-closed
  spawn tests.
* `sp_server/src/headless.rs` — attributed-unit inspection, completion
  inspection, real-combat victory, offline timing, helper attribution,
  committed disconnect/reconnect continuation, offline resolution,
  missing-unit recovery, legacy isolation, True Death, fresh-run, and repeated
  disconnect/reconnect scenarios.
* `docs/persistent_crisis_milestone.md` — this implementation record.

The corrective addendum changes exactly `game.rs`, `ai/npc/npc.rs`,
`ai/villager/villager.rs`, `ai/villager/villager_tests.rs`, `player.rs`,
`headless.rs`, and this document. No resource, recipe, item, crafting,
refining, farming, fishing, trade, map, network-protocol, client,
database-schema, deployment, or infrastructure file receives a semantic
change. `encounter.rs`, `combat.rs`, `obj.rs`, `event.rs`, and
`player_setup.rs` were audited and their existing mechanisms were reused.

### Assault data model and identity

`SettlementCrisis` retains the Checkpoint 2 fields plus the logical assault ID,
start tick, active online-tick observation, tracked object IDs, normally
defeated object IDs, successful spawn generation, one-time resolution guard and
tick, a narrow `assault_recovery_required` corruption marker, and one-shot
observability flags. The corrective addendum removes the rollback-only template
copies, remaining-template list, retry counter, reset intent, reconnect intent,
and controlled-generation cleanup field. No disconnect state or relaunch
generation remains.

Every major-assault NPC carries:

```rust
struct CrisisAssaultUnit {
    owner_player_id: i32,
    assault_id: u64,
    spawn_generation: u32,
}
```

NPC faction ownership remains `NPC_PLAYER_ID`; the component, not the faction
or killer, is authoritative for personal-crisis ownership. A dedicated
`NextCrisisAssaultId` Bevy resource starts at one and advances with checked
`u64` addition. It is monotonic for the server process and is never derived
from `GameTick`. A generation is recorded only after the initial complete spawn
succeeds; an active assault is never rebuilt as another generation.

### Launch timing policy

`AssaultReady` uses `phase_online_ticks`, so the existing Checkpoint 2 watermark
excludes disconnected time and duplicate evaluation of one tick. The named
provisional tuning is:

```text
ready grace:          300 online ticks (30 seconds)
maximum online wait: 1,200 online ticks (120 seconds)
preferred window:    DUSK through NIGHT, plus pre-FIRST_LIGHT night
```

Before 300 online ticks the assault cannot launch. At or after 300 it launches
during the preferred window. At 1,200 online ticks it may launch at any time,
so a player is not forced to remain connected through a world day. Logging out
before the first launch leaves `AssaultReady` and preserves already-earned
preparation time. A successful transition to `AssaultActive` is the commitment
point; the ready grace is never restarted because of an ordinary disconnect
after that transition.

### Settlement anchor and spawn policy

Anchor priority is the live bound monolith, a completed Campfire or Storage,
another completed owned structure, then the current hero position only when a
real run's `SpawnPositions` entry exists. This last check keeps the compatibility
fallback while preventing a partial/stale hero row from becoming a settlement.
No anchor leaves the crisis ready and produces one warning.

For a bound sanctuary, candidates are drawn from the three rings beginning one
tile outside that exact monolith's weak-sanctuary radius. Other anchors use
rings six through eight. The bounded helper rejects out-of-map, impassable,
occupied, duplicate, and neighbouring-settlement-footprint tiles and requires
a terrain path back to the anchor. It returns `None` rather than an invalid
centre fallback and preselects one distinct tile per unit. Failure keeps the
crisis ready and cannot consume a spawn generation.

### Spawn composition and target selection

The centralized first-wave composition is:

```text
2 Wolf Riders
1 Goblin Pillager
```

This is the smallest coherent wave available from current templates: it is the
size of the legacy rider raid and smaller than combining it with the later
three-Pillager tier. The templates use `Encounter::spawn_npc`'s ordinary
server-authoritative chase/combat brain, 14-tile vision, existing loot, normal
walls and combat, and an owner-filtered target scorer. The owner, villagers,
and nearby connected helpers can attack normally whether or not the owner is
online; helpers may receive ordinary kill credit, while component attribution
keeps crisis progress with the settlement owner. Queued input from the
disconnected owner still fails because its source client is absent. Specialized
theft and torch actions remain excluded. The scorer and action boundary both
prohibit cross-player hero, villager, sanctuary, storage, structure, and wall
targets; no other settlement is a fallback. The ordinary scorer attacks owned
human units and owned blocking walls; storage and neutral-ID monoliths are not
selected as personal-assault targets.

### Disconnect commitment and recovery policy

`AssaultActive` is the commitment point. An ordinary disconnect does not change
the phase, logical assault ID, spawn generation, tracked unit IDs, current hit
points, current valid targets, warning state, pressure, or settlement damage.
It does not remove attackers or pending combat events, increment a retry, reset
the launch grace, respawn a template, heal a survivor, or start another
generation. Ordinary NPC AI and structure damage continue, the owner's
villagers continue normal defence, and connected helpers may continue normal
combat. Only the disconnected owner's queued player actions fail their retained
source-presence check.

Reconnect restores control of the existing hero and run. It observes the same
active assault and surviving world entities, or `Resolved` if ordinary defence
finished the wave while the owner was away. It does not replay the warning or
grant another preparation window.

True Death, run abandonment/recreation, start-location recycling, and explicit
administrative cleanup remain controlled cleanup events. If a tracked active
unit disappears without normal-death evidence or one of those controlled
causes, the lifecycle logs it, leaves the crisis `AssaultActive` and unresolved,
and sets `assault_recovery_required`. It does not infer victory or automatically
reset the wave. A deterministic administrative recovery mechanism is deferred.

### Resolution and completion policy

The tracker treats matching current-generation attribution as authoritative and
stored IDs as expected-object bookkeeping. Only matching objects with
normal-combat `State::Dead` or `StateDead` evidence remove logical remaining
slots. Controlled removal, True Death, legacy goblins, unrelated goblins,
another owner's units, obsolete generations, and unexplained disappearance
never count as defeat.

When every tracked unit has normal-death evidence, the system sets
`resolution_recorded` before changing to `Resolved`, records the tick, clears
the active warning and active unit IDs, emits one structured log, and increments
`PlayerRunScore::personal_crises_resolved` once. This applies while the owner is
offline as well as online. The runtime run-score entry remains available after
an ordinary socket disconnect, so no deferred score path is currently needed.
No item, resource, currency, objective chain, pressure reduction, or tangible
reward is added.
Normal NPC loot remains on normally defeated corpses. A resolved crisis does
not start another crisis or re-enable any legacy escalation.

### Cleanup and system ordering

`personal_crisis_system` is explicitly after `update_game_tick`. The lifecycle
system is PersonalCrisis-only and explicitly after Checkpoint 2 evaluation,
sanctuary synchronization, NPC action systems, resurrection/death handling, and
True Death cleanup; it runs before map-event execution, ordinary dead/wandering
removal, and perception. This makes True Death state deletion win over
resolution, allows synchronous `State::Dead` or deferred `StateDead` evidence
to be read before corpse removal, and prevents a launched deferred generation
from being inspected as empty in the launch evaluation. Source-presence checks
reject stale input from a disconnected player. Attribution-based ownership
checks at NPC scoring, target installation, damage application, and fortified
target redirection reject stale or cross-owner targets without freezing combat
for an offline owner.

True Death logs the logical assault, removes same-owner attributed units even
if bookkeeping is incomplete, protects another owner's attributed units, and
then removes the crisis state. Successful hero recreation performs a final
same-owner orphan sweep. Missing `EntityObjMap` entries use a direct safe
despawn fallback. Both paths are idempotent, never insert `StateDead`, and never
award completion.

### Tests and validation

Focused unit and full-schedule headless coverage now exercises:

* grace, preferred darkness, maximum-wait fallback, and monotonic IDs;
* anchor priority, missing anchors, passable/pathable/occupied placement, and
  deterministic spawn failure on an impassable map;
* exact loaded composition and owner-filtered NPC target selection;
* offline-ready pause and reconnect continuation;
* one successful launch, attribution, run bookkeeping, and duplicate-tick
  protection;
* real `PlayerEvent::Attack` partial and complete victory with ordinary corpse
  inventory retained;
* helper `LastAttacker` score with owner-attributed crisis completion;
* queued Attack, Combo, and Block source-presence rejection for the disconnected
  owner without freezing connected-helper input;
* attributed NPC combat and owner-villager retaliation continuing while the
  owner is offline;
* owner association at target scoring, target installation, damage application,
  and fortified redirection, including rejection of another player's hero,
  villager, sanctuary, storage, structures, and walls;
* disconnect preserving `AssaultActive`, logical ID, spawn generation, unit
  IDs, survivor health, pressure, and launch state without despawn or relaunch;
* partial offline kills remaining active, complete offline victory resolving
  once, and reconnect observing the existing active or resolved state;
* unexpected disappearance remaining unresolved with recovery required;
* True Death from both `AssaultReady` and `AssaultActive`, disconnect overlap,
  protection of another owner's assault and nearby unrelated hostiles,
  missing-map orphan cleanup, idempotency, and clean fresh-run fields;
* three consecutive disconnect/reconnect cycles preserving one generation,
  followed by one resolution;
* PersonalCrisis nightly-horde isolation, legacy-mode isolation, the complete
  introduction, environmental visibility, and existing production regressions.

Final validation from `sp_server/`:

* `cargo fmt --check` — passed.
* `cargo check` — passed; the existing compiler warning set remains (74
  warnings in the library build).
* `cargo test checkpoint3 -- --nocapture` — passed all 21 Checkpoint 3 tests
  (292 filtered out).
* `cargo test fight_back_system -- --nocapture` — passed all 4 focused
  retaliation tests (309 filtered out).
* `cargo test` — passed all 313 library tests and all 6 integration tests; the
  single documentation test remains intentionally ignored.
* `cargo clippy --all-targets --all-features` — passed with the existing lint
  backlog (1,332 library warnings, including 32 duplicates, and 1,345
  library-test warnings, including 1,300 duplicates).
* `cargo run --bin headless_runner -- 3 6000` — passed all 3 deterministic
  6,000-tick runs with zero panics and zero True Deaths. Runs ended at tick
  6,007 with 2.00 mean days survived, 5.67 mean enemies killed, 436.7 mean
  final skill XP, 19.3 mean inventory count, and 2.00 mean structures.
* `git diff --check` — passed from the repository root.

Three intermediate validation failures were corrected before those final
passes: the first `cargo fmt && cargo check` exposed a removed `Clients` import
still required by another villager system; the first focused Checkpoint 3 run
passed 16 of 17 tests and exposed a test action missing its required
`ActionSpan`; and a strengthened focused run passed 19 of 21 tests and exposed
two overly broad snapshot assertions. The import, action construction, and
assertions were corrected, then the applicable commands above were rerun to
their recorded passing results.

### Checkpoint 4 completion and remaining limitations

Checkpoint 4 subsequently added the dedicated crisis-status packet, reconnect
snapshot/UI state, compact desktop client presentation, and runtime runner
telemetry documented above. It did not add a player-facing material reward.
Crisis persistence remains limited by the prototype's existing runtime-only
per-run state. Spawn selection is intentionally randomized among bounded valid
candidates, although all asserted invariants are deterministic. An ordinary
disconnect provides no protection after `AssaultActive`: current combat and
settlement damage continue, and no damage is rolled back. A missing tracked
unit deliberately leaves the crisis unresolved and requires a future
administrative or deterministic recovery path. Specialized theft/torch
behaviours remain available to legacy systems but are not part of this personal
wave. The ordinary personal-assault brain does not attack storage or monolith
objects; monolith ownership would need an explicit authoritative mapping before
sanctuary targeting could be enabled safely. Explicit safe logout/offline
protection is a separate deferred design.

The repository-root `AGENTS.md` states the broader product invariant that
personal irreversible danger must not progress while the owner is offline. The
corrective Checkpoint 3 product decision is a deliberate, narrow exception for
an assault that has already launched successfully: pre-launch timing remains
online-only, but committed `AssaultActive` combat continues. This patch does not
generalize that exception to other personal-crisis phases or systems.

## Checkpoint 1 implementation record

Checkpoint 1 is limited to director separation. The goblin crisis state machine,
online/offline crisis timing, the active-assault disconnect policy, crisis packets and UI, and
headless crisis metrics remain deferred to later checkpoints.

### Architecture findings

* `GameTick` is a reflected Bevy resource in `sp_server/src/game.rs`. It defaults
  to `DAWN`, is incremented by `update_game_tick`, and provides global day,
  hour, and time-of-day calculations. It is global rather than player-specific.
* `WorldPlugin` in `sp_server/src/world.rs` registers `day_system` independently
  of the settlement-danger systems. At the time boundaries defined in
  `constants.rs`, `day_system` recalculates non-NPC `Viewshed` ranges from base
  vision, time of day, equipped-item vision, and effects. It also sends the
  existing `ResponsePacket::World { time_of_day, day }`. Weather state is held
  by `WeatherAreas`; `weather_cycle_system` and `weather_effects_system` are
  registered separately in `GamePlugin` and remain unconditional in both
  director modes.
* `PlayerCrisis` and `CrisisState` in `sp_server/src/game.rs` are in-memory,
  per-player legacy state. `PlayerCrisis` contains five automatic ladder flags
  (`rat_spoilage`, `wolf_pack`, `goblin_raid`, `undead_incursion`, and
  `goblin_pillager`) plus `initial_encounter` and `spider_encounter`. Thus the
  introduction and legacy crisis ladder are not yet cleanly separated.
* `rat_event_system` checks stored food every 20 ticks after the 4,800-tick
  introduction grace. `wolf_pack_system` checks distance from spawn every 10
  ticks and has an eight-minute fallback. `goblin_raid_system` checks for 30
  stored gold every 30 ticks and has a ten-minute fallback. The undead and
  pillager systems check every 10 ticks and use three-day/16-minute and
  five-day/24-minute thresholds respectively.
* `nightly_threat_system` checks the global tick for exact `DUSK`, skips global
  day one and players younger than 4,800 ticks, then spawns a day-scaled wave
  outside the sanctuary. On a successful spawn it records the global day in a
  system-local map and increments `PlayerRunScore::waves_survived`.
* `legendary_threat_system` checks every 10 ticks. It creates the Fire Dragon
  hideout at player survival day six, activates the campaign at day seven, and
  sends recurring follower waves. `legendary_death_tracking_system` performs
  cleanup, objective, and score bookkeeping for already-existing legendary
  entities and is not itself an automatic escalation trigger.
* `GamePlugin` registers the legacy ladder, nightly horde, and legendary
  escalation as separate `Update` systems. The introductory
  `initial_encounter_system` is also separate, so it can remain active while the
  automatic systems are mode-gated. Economy plugins and their systems are
  registered independently and require no Checkpoint 1 changes.
* New-player setup in `player_setup.rs` creates `PlayerIntroState` and
  `InitialEncounterState`, including one delayed wave of one to three Giant
  Rats at the Shipwreck, the boar/crab follow-up, Spider follow-up, villager
  timing, merchant, and later Necromancer data. The hostile follow-ups use
  randomized valid, passable, reachable, unoccupied tiles two to four tiles
  from the assigned hero start; narrative NPC anchors are unchanged.
  `initial_encounter_system` still uses the two introduction flags in
  `PlayerCrisis`; this temporary coupling is retained for Checkpoint 1.
* Login enters through `PlayerEvent::Login` in `player.rs`, which schedules a
  `GameEventType::Login`; `game_event_system` later performs login perception
  and queues the sanctuary resend. Network disconnect paths remove the active
  client UUID from the shared `Clients` map. Hero entities can remain in the
  ECS, so they are not authoritative presence. Presence semantics are inspected
  but intentionally unchanged until Checkpoints 2 and 3.
* `build_headless_app` builds the same gameplay plugins without the realtime
  schedule runner or production network. `HeadlessGame` supplies in-process
  client/database channels, pumps `App::update`, exposes world observations and
  run metrics, and drives the existing deterministic bot through
  `headless_runner`.

### Checkpoint 1 affected files

* `sp_server/src/game.rs` — named director mode/configuration and mode gates on
  legacy automatic danger systems.
* `sp_server/src/lib.rs` — explicit personal-crisis default for production and
  headless app builders.
* `sp_server/src/headless.rs` — explicit mode-selecting constructor and focused
  director/introduction regression tests.
* `sp_server/src/game_tests.rs` — default-mode regression test.
* `sp_server/tests/day_system_test.rs` — personal-mode night-visibility
  regression coverage.
* `docs/persistent_crisis_milestone.md` — this architecture and configuration
  record.

No resource, recipe, crafting, farming, structure, trade, villager AI, map,
network protocol, client, database, or deployment file is changed.

### Design conflicts and selected scope

The milestone-level goal and definition of done describe the completed
four-checkpoint initiative, including a goblin phase machine, offline rules,
assault attribution, networking, and UI. Section 24 explicitly assigns those
features to Checkpoints 2 through 4. The Checkpoint 1 request is narrower, so
this implementation follows Section 24 and does not satisfy or implement those
later milestone-level items yet.

The preferred design separates introductory state from crisis state, but the
current introduction reads and writes `PlayerCrisis.initial_encounter` and
`spider_encounter`. Moving those fields is Checkpoint 2 work and would expand
this patch. Checkpoint 1 therefore retains `PlayerCrisis` unchanged, leaves
`initial_encounter_system` active in both modes, and gates `rat_event_system`
separately because it is the automatic food-spoilage tier rather than the
shipwreck chain.

### Selected configuration approach

The server uses one named Bevy resource:

```rust
enum SurvivalDirectorMode {
    Legacy,
    PersonalCrisis,
}

struct SurvivalDirectorConfig {
    mode: SurvivalDirectorMode,
}
```

`PersonalCrisis` is the `Default` and is passed explicitly by both the
production and standard headless builders. `Legacy` remains selectable by app
construction and by the headless regression harness. A shared Bevy run
condition gates only `rat_event_system`, `wolf_pack_system`,
`goblin_raid_system`, `undead_incursion_system`,
`goblin_pillager_system`, `nightly_threat_system`, and
`legendary_threat_system`. Environmental time, weather, visibility, world-time
packets, introductory encounters, legendary death bookkeeping, and the economy
remain registered in both modes.

## Checkpoint 2 implementation record

Checkpoint 2 adds only the server-authoritative personal-crisis state
foundation. It does not spawn a goblin assault, create assault IDs or units,
send crisis packets, change the client, define committed combat, grant crisis rewards, or
add Checkpoint 4 metrics.

### Current architecture findings

* `Clients` in `sp_server/src/game.rs` is the shared
  `Arc<Mutex<HashMap<Uuid, Client>>>` populated after authentication in
  `network.rs`. Hero entities persist after socket disconnect, so ECS hero
  existence is not presence. Explicit close and socket-error paths remove
  clients; a clean EOF can leave a map entry, although its game-to-client
  receiver drops and makes the sender observably closed. Duplicate-session
  manager termination previously removed only the stream.
* `PlayerCrisis` and `CrisisState` are still the per-player legacy director
  state. Their production `crisis_tier` helper reads only the rat, wolf, goblin,
  undead, and pillager flags. The two shipwreck follow-up flags were the only
  remaining introduction/legacy coupling.
* `PlayerIntroState.danger_unlocked` is the existing introduction safety gate.
  `InitialEncounterState` owns the delayed one-to-three Giant Rat wave,
  villager, merchant, and Necromancer data. `initial_encounter_system` owns the
  boar/crab and Spider follow-ups, selects their valid randomized positions two
  to four tiles from the assigned hero start, and remains registered in both
  director modes. Narrative NPCs retain their authored spawn anchors.
* Completed structures have the canonical predicate
  `Structure::is_built(State)`. Living villagers can be identified by owner,
  live `State`, and absence of `StateDead`. Existing goblin wealth logic already
  treats completed `Storage` inventories as stored wealth and uses
  `Inventory::get_total_gold`. A hero's `BoundMonolith.id` is the authoritative
  link to the otherwise global `Monolith.sanctuary_level`.
* `new_player_system` delegates run construction to `player_setup::new` and is
  already near Bevy's outer system-parameter limit, so related run resources are
  bundled in one tuple. True Death cleanup is delayed until more than ten
  seconds after `TrueDeath`, then removes run-scoped state and releases the start
  location. The hero despawn is deferred, so crisis evaluation must reject the
  death markers before it can lazily create state.
* World snapshots use `DynamicScene::from_world`, which serializes only
  reflected, registered types. Existing `CrisisState`, `PlayerIntroState`,
  `InitialEncounterState`, `Objectives`, start assignments, and run-spawn
  bookkeeping are not reflected or registered. In addition, snapshots write a
  working-directory `dynamic_scene.ron`, while reload asks the asset server for
  `dynamic_scene.ron` under its asset source. Comparable per-run crisis and intro
  state is therefore runtime-only under the current prototype reload model.
* The headless harness owns the same `Clients` resource as production, retains
  an in-process hero after a test disconnect, and can inspect ECS resources
  directly. `headless_bot.rs` already exercises building, recruitment,
  exploration, gathering, crafting, and villager work, so its behaviour did not
  need to change for this checkpoint.

### Files changed

* `sp_server/src/game.rs` — authoritative presence helper; separated intro
  flags; personal crisis types, pressure calculator, timing and transition
  helpers; PersonalCrisis-only evaluation registration; and True Death cleanup.
* `sp_server/src/player_setup.rs` — initializes fresh shipwreck follow-up flags
  for every successfully allocated run.
* `sp_server/src/player.rs` — bundles the new run resources into player setup and
  clears any stale personal crisis after successful hero recreation.
* `sp_server/src/network.rs` — makes manager-driven duplicate-session shutdown
  remove the client record and end the old handler immediately.
* `sp_server/src/game_tests.rs` — focused presence, pressure, clock,
  initialization, NPC-exclusion, ordered-phase, and production legacy-tier
  tests.
* `sp_server/src/headless.rs` — deterministic disconnect/reconnect and read-only
  state access, full introduction and lifecycle regressions, and the short
  Checkpoint 2 simulation.
* `docs/persistent_crisis_milestone.md` — this implementation record.

No resource, recipe, farming, structure, trade, item, villager-AI, map, client,
database-schema, or deployment file is changed. `headless_bot.rs`, `event.rs`,
`lib.rs`, `obj.rs`, and the environmental world implementation were audited but
did not require Checkpoint 2 changes.

### Selected crisis data model

`PlayerCrisis` now contains only the five legacy ladder flags. Shipwreck combat
completion lives independently in:

```rust
struct PlayerIntroEncounters {
    initial_encounter: bool,
    spider_encounter: bool,
}

struct IntroEncounterState(HashMap<i32, PlayerIntroEncounters>);
```

Personal danger lives in a separate `SettlementCrisisState`, with one
`SettlementCrisis` per player ID. The crisis records `CrisisKind::Goblin`, an
explicit `CrisisPhase`, derived pressure, phase start tick, total and per-phase
online-active ticks, warning-active state, and the last evaluated game tick.
The phase enum contains the future `AssaultActive` and `Resolved` variants for
the complete lifecycle, but Checkpoint 2 has no transition to them.

The PersonalCrisis-only evaluator creates a valid live human run in `Dormant`.
Creation is deterministic and occurs once; it can exist while the safety gate
is closed, but its pressure and active clocks remain zero and it cannot
transition. Reconnect reads the existing entry rather than recreating it. Legacy
mode initializes the resource for app compatibility but never runs the personal
evaluator or changes gameplay through it.

### Selected pressure model

Pressure is recomputed from current read-only facts on a 0–100 scale. It is not
incrementally awarded and has no global-day input:

| Existing fact | Pressure |
| --- | ---: |
| Introduction danger unlocked | 10 |
| At least three completed player-owned structures | 20 |
| At least one living player-owned villager | 15 |
| `explore_poi` objective complete | 10 |
| `choose_expansion` objective complete | 15 |
| Stored gold at 25 / 50 / 100 | 5 / 10 / 15 |
| Bound sanctuary level | 2 per level, maximum 10 |
| Online-active time at 600 / 1,800 / 3,600 ticks | 5 / 10 / 15 |

The raw sum is capped at 100. Completed structures and living villagers are read
from world facts; their corresponding `build_3_structures` and
`recruit_villager` objective flags are deliberately not added again. Stored gold
is read only from completed owned storage structures, so unfinished or dead
storage does not count and no inventory is consumed or altered. Settlement facts
are aggregated once per evaluation before per-player calculation.

### Presence and online timing semantics

`Clients::is_player_online(player_id)` is the single read-only presence check.
It requires at least one map entry whose key matches `Client.id`, whose player ID
matches, and whose Tokio sender is still open. Multiple records are safe: one
remaining valid record keeps the player online. Removing the last record, a
closed stale sender, a malformed key, or a poisoned client mutex produces
offline. Manager-driven duplicate-session termination now removes the client as
well as the stream and exits the handler.

Both crisis clocks use `GameTick`, never wall time. The elapsed delta is clamped
at zero, and `last_evaluated_tick` is a monotonic watermark, so a transient tick
rollback cannot make already-credited time eligible again. The delta is credited
only when the introduction gate is open, the owner has a valid live hero, and
the presence helper says the owner is online. Repeated evaluation at one tick
adds nothing. Offline, ordinary-death, True Death, missing-hero, invalid-run,
and pre-gate intervals update only the watermark, so none can be backfilled
after reconnect, resurrection, or entity recreation.

### Phase thresholds, warning, and observability

Transitions are strictly ordered and limited to one per player per evaluation:

| Transition | Pressure | Minimum online time in current phase |
| --- | ---: | ---: |
| `Dormant` → `Signs` | 20 | none |
| `Signs` → `Pressure` | 45 | 600 ticks / 60 seconds |
| `Pressure` → `Preparing` | 70 | 1,200 ticks / 120 seconds |
| `Preparing` → `AssaultReady` | 90 | 1,800 ticks / 180 seconds |

Every transition updates `phase_started_tick` and resets
`phase_online_ticks`. Entering `Preparing` sets `warning_active`; entering
`AssaultReady` retains it. Creation and transitions emit one concise log with
player, phase, pressure, tick, and online state. Pressure changes do not log per
tick. The evaluator has no `Commands`, packet send, reward, damage, or database
parameter, so reaching `AssaultReady` cannot spawn or resolve an assault.

### Cleanup semantics

Successful player setup writes fresh default `IntroEncounterState` and removes
any stale `SettlementCrisisState`; a failed start allocation does neither. True
Death idempotently removes both new entries alongside existing legacy crisis,
intro, objective, score, start-assignment, and run-spawn cleanup. Evaluation
checks `State`, `StateDead`, and `TrueDeath` before initialization, so deferred
despawn cannot recreate a just-cleaned crisis. The introductory encounter also
requires a live, non-True-Death owner before queuing deferred follow-up spawns;
this prevents a spawn from escaping a same-update cleanup sweep at a recycled
start location. Cleanup is keyed only by the dead owner and does not alter
neighbouring players or the global director mode.

### Persistence decision and limitation

Both new resources are initialized by `GamePlugin` so new games, headless apps,
and reload startup always have the resources. They remain runtime-only, matching
the comparable legacy crisis and introduction state. Persisting only the new
maps would produce an inconsistent partial run because the safety gate,
detailed encounter chain, objectives, start assignment, and run-spawn cleanup
state would still reset. Checkpoint 2 therefore adds no reflection registration,
database table, migration, or periodic write. Complete coherent world/run
persistence, including the existing snapshot path mismatch, remains separate
future work.

### Design conflicts and deferred Checkpoint 3 work

The milestone's overall definition includes online-only assault launch,
attributed units, committed continuation, resolution, and client status. The
Checkpoint 2 request explicitly stops at `AssaultReady`, so that implementation
intentionally did not satisfy those later lifecycle items. Checkpoint 3
subsequently added the warning grace/launch policy, assault identity and unit
attribution, online-only launch, committed disconnect handling, cleanup, and
idempotent resolution.
Checkpoint 4 subsequently added structured network status, reconnect delivery,
the compact desktop client UI, and full runtime headless crisis metrics.

## Purpose

This document defines the first implementation milestone in the Siege Perilous persistent-world redesign.

The milestone separates the shared environmental day/night cycle from automatic settlement attacks and establishes the foundation for player-specific crises that respect independent play schedules.

This is not the complete long-term multiplayer redesign.

The milestone is deliberately limited to:

* Preserving global environmental time
* Disabling automatic dusk hordes in the new default mode
* Replacing the current automatic crisis ladder with one explicit goblin crisis
* Establishing online/offline crisis rules
* Preserving the existing resource and production economy
* Adding focused server, client, and headless-test support

---

# 1. Product context

Siege Perilous is intended to become a persistent shared-world action-survival game.

Each player:

* Controls a hero in real time
* Builds an independent settlement
* Gathers and processes resources
* Crafts equipment
* Recruits and assigns villagers
* Survives personal and regional dangers
* Plays on an independent schedule
* Interacts with other players when their sessions overlap

The game should provide a persistent-world feeling without requiring players to coordinate sessions or log in at specific real-world times.

The Checkpoint 3 commitment rule is:

> Before launch, personal-crisis timing and launch remain online-only. After a
> successful transition to `AssaultActive`, the assault is committed and the
> world continues even if the owner disconnects.

Explicit safe logout/offline protection is deferred and is not implied by an
ordinary network disconnect.

---

# 2. Problem statement

The current implementation contains several systems that independently control danger and progression:

* The global day/night cycle
* Automatic nightly hordes
* A tiered crisis ladder
* Survival-time fallback timers
* Day-based undead and pillager triggers
* Legendary Fire Dragon escalation
* Settlement wealth triggers
* Player objectives
* Sanctuary progression

These systems compete with one another.

Examples of resulting problems include:

* Every dusk automatically creates settlement danger.
* Personal crises and night hordes can overlap.
* Players may be forced into combat because of the world clock.
* Players who log in shortly before dusk receive an immediate mandatory attack.
* Day count advances regardless of player activity.
* Different crisis families are treated as mandatory tiers of one run.
* Offline or irregular players cannot safely participate in a persistent world.
* The player may not understand which action caused an attack.
* The game is difficult to tune because several escalation systems stack together.

The redesign assigns different responsibilities to different systems.

The global clock controls environmental conditions.

The personal crisis system controls directed settlement danger.

---

# 3. Milestone goal

Implement the foundation for this relationship:

> The crisis decides what happens. The global clock influences the conditions under which it happens.

After this milestone:

* Global day/night continues normally.
* Darkness, lighting, visibility, weather, and world time remain shared.
* Reaching dusk alone does not create a settlement horde in the default prototype mode.
* The old automatic crisis ladder does not run in the default prototype mode.
* A player-specific goblin crisis progresses through explicit phases.
* Major crisis assaults begin only while the owning player is online.
* Once `AssaultActive`, ordinary disconnect does not pause, reset, despawn,
  rebuild, or weaken the committed assault.
* The current resource, crafting, refining, farming, fishing, trade, and villager systems remain intact.

---

# 4. Current repository architecture

The relevant server architecture currently includes:

## `sp_server/src/game.rs`

Contains or coordinates:

* `GameTick`
* `PlayerCrisis`
* `CrisisState`
* Introductory encounter state
* Rat event logic
* Wolf pack logic
* Goblin raid logic
* Undead incursion logic
* Goblin pillager logic
* `nightly_threat_system`
* Legendary threat logic
* Sanctuary zones
* Run scoring
* System registration
* Player notices
* Structure processing systems
* True Death and run cleanup

The current `PlayerCrisis` structure mixes two distinct concepts:

* Introductory encounter progression
* Automatic crisis-ladder completion flags

Those responsibilities should be separated.

## `sp_server/src/world.rs`

Contains environmental world-time behaviour including:

* Time-of-day calculation
* Global day/night state
* Visibility modifiers
* Weather
* Vision changes
* World-time updates

This environmental behaviour must remain intact.

## `sp_server/src/player_setup.rs`

Contains:

* Player and hero setup
* Starting locations
* Initial world objects
* Run-spawned object tracking
* Introductory encounter setup

## `sp_server/src/network.rs`

Contains:

* Incoming network packets
* Outgoing response packets
* Login and disconnect messages
* Resource, crafting, structure, villager, trade, and information commands

The crisis redesign may add an outgoing crisis-status packet.

## `sp_server/src/headless.rs`

Contains the in-process headless game harness used to:

* Build the Bevy application
* Advance game ticks without real-time scheduling
* Capture packets
* Observe world state
* Collect per-run metrics

## `sp_server/src/headless_bot.rs`

Contains deterministic scripted player behaviour used for balance and regression testing.

## Existing economy modules

The current economy is implemented across modules including:

* `resource.rs`
* `recipe.rs`
* `farm.rs`
* `structure.rs`
* `trade.rs`
* `item.rs`
* Villager AI and utility modules
* Work queues and assignment systems

These systems are not targets for simplification in this milestone.

---

# 5. Critical constraints

The implementation must follow these constraints.

## World constraints

* Keep the current 50×50 map.
* Keep the current start-location model.
* Do not implement the future 20–25-player world yet.
* Do not implement map resizing.
* Do not implement cross-world or cross-instance interaction.
* Do not implement world migration or seasonal resets.

## Environmental constraints

* Keep the global `GameTick`.
* Keep environmental day/night.
* Keep weather.
* Keep lighting and visibility changes.
* Keep world-time packets.
* Keep night as a dangerous environmental condition.
* Do not use dusk alone as the default trigger for a settlement attack.

## Economy constraints

Preserve the current systems for:

* Gathering
* Mining
* Wood harvesting
* Stone gathering
* Hunting
* Farming
* Fishing
* Butchery
* Refining
* Smelting
* Tanning
* Milling
* Baking
* Herbalism
* Spinning
* Tailoring
* Food preservation
* Crafting
* Recipes
* Inventory
* Work queues
* Villager assignments
* Trade
* Gear progression

Do not:

* Collapse resources into generic categories
* Remove processing stages
* Replace existing materials with generic “metal,” “food,” or equivalent counters
* Add new resource families
* Add new professions
* Add new crafting tiers
* Add new currencies

The existing economy is feature-frozen but remains active and testable.

## Crisis constraints

* Use goblins as the only complete personal crisis in this milestone.
* Do not run the old mandatory wolf → goblin → undead → pillager ladder in the default mode.
* Do not run automatic legendary escalation in the default mode.
* Preserve the old systems behind a named legacy mode where practical.
* Do not delete working content merely because it is disabled in the new prototype mode.
* Keep the introductory shipwreck encounter chain functional.
* Keep its initial Giant Rat wave, follow-up creature, villager, Spider, and
  related progression intact.

## Offline constraints

* A major personal crisis assault must not begin unless the owner is online.
* Before launch, offline crisis time must not advance toward an unavoidable
  assault.
* After a successful launch, `AssaultActive` is committed and ordinary ECS
  combat continues while the owner is offline, including attacks on the valid
  owner-associated heroes, villagers, and blocking walls admitted by the
  ordinary personal-assault brain. Storage and monolith targets remain
  excluded as documented in the Checkpoint 3 implementation record.
* Ordinary disconnect is not a safe-logout signal and grants no rollback,
  healing, reset, despawn, or relaunch protection.
* Do not implement a separate offline combat simulator; the loaded world uses
  its existing server-authoritative NPC, villager, structure, and combat systems.
* Explicit safe logout/offline protection remains deferred.

---

# 6. Explicit survival-director modes

Introduce an explicit configuration that selects the active danger model.

Suggested conceptual modes:

```rust
enum SurvivalDirectorMode {
    Legacy,
    PersonalCrisis,
}
```

Names may follow repository conventions.

## Legacy mode

Preserves current behaviour as closely as practical:

* Existing crisis ladder
* Automatic dusk hordes
* Survival director
* Legendary escalation
* Existing day and fallback triggers

Legacy mode exists for comparison and regression testing.

It should not be the default prototype mode.

## Personal-crisis mode

Becomes the default.

In this mode:

* Environmental day/night remains active.
* Introductory encounters remain active.
* Automatic dusk hordes are disabled.
* The old automatic crisis ladder is disabled.
* Legendary escalation is disabled.
* The new personal goblin crisis is active.

The director mode should be represented by a named resource or configuration rather than scattered booleans.

---

# 7. Authoritative player presence

The server requires one authoritative way to determine whether a player is online.

Conceptually:

```rust
fn is_player_online(player_id: i32) -> bool
```

Presence must be derived from active connected clients, not merely from whether the player’s hero entity remains in the Bevy world.

Presence handling must account for:

* Login
* Reconnect
* Clean disconnect
* Unexpected disconnect
* Stale client records
* Headless test clients
* More than one client record, if the architecture allows it

Crisis launch authority remains entirely server-side.

The client must not decide whether an assault can begin.

---

# 8. Separate introductory encounters from crises

The current `PlayerCrisis` contains both crisis-ladder flags and introductory encounter flags.

The new architecture should separate them.

Preferred conceptual structure:

```rust
struct PlayerIntroEncounters {
    initial_encounter: bool,
    spider_encounter: bool,
}
```

and:

```rust
struct SettlementCrisisState {
    crises: HashMap<i32, SettlementCrisis>,
}
```

An alternative compatible implementation may retain the old `PlayerCrisis` temporarily for legacy and introduction support while adding a new personal-crisis resource.

The new personal crisis must not be implemented as additional unrelated booleans added to the old ladder.

---

# 9. Personal crisis data model

Implement an explicit crisis state machine.

Suggested conceptual model:

```rust
enum CrisisKind {
    Goblin,
}

enum CrisisPhase {
    Dormant,
    Signs,
    Pressure,
    Preparing,
    AssaultReady,
    AssaultActive,
    Resolved,
}

struct SettlementCrisis {
    kind: CrisisKind,
    phase: CrisisPhase,
    pressure: i32,
    phase_started_tick: i32,
    online_active_ticks: i32,
    warning_sent: bool,
    assault_id: Option<u64>,
    assault_unit_ids: Vec<i32>,
    resolved_at_tick: Option<i32>,
}
```

The exact structure may be adapted to repository conventions.

Required properties:

* One primary personal crisis per player
* Explicit phase transitions
* Inspectable state
* Testable transitions
* Idempotent updates
* No duplicate assault spawning
* No duplicate resolution rewards
* State can be sent to the player
* State is compatible with the existing world save/reload model
* Introductory encounter state remains separate

---

# 10. Goblin crisis progression

The first crisis should use existing goblin content and existing player progression signals.

It should not require a new quest framework.

## Pressure sources

Potential pressure signals include:

* Structures built
* Settlement wealth
* Stored gold
* Villagers recruited
* Sanctuary upgrades
* Resource extraction
* Production activity
* Objective completion
* Goblin incidents left unresolved
* Online active time

The crisis must not progress only because the global day number increased.

The implementation may use a configurable combination of:

* Settlement-development milestones
* Pressure points
* Minimum online-active time
* Unresolved goblin activity

## Dormant

The player has not yet attracted organized goblin attention.

The introductory encounter may still be active.

## Signs

The settlement has become noticeable.

Possible presentation:

* Distant smoke
* Missing goods
* Goblin tracks
* A warning notice
* A small discovery or status update

The player should understand that settlement growth has attracted attention.

## Pressure

The goblins begin interfering with the player.

Use existing content where practical, such as:

* Wolf Riders
* Theft behaviour
* Ambushes
* Small scouting groups
* Sabotage

This phase must not be settlement-ending.

## Preparing

The goblins are organizing a major raid.

The player receives a clear warning and preparation window.

Preparation should support several existing-system responses, including:

* Building or repairing walls
* Crafting weapons or armour
* Producing healing supplies
* Assigning guards
* Recruiting villagers
* Improving the sanctuary
* Defeating a pressure incident
* Stocking food and resources

The crisis should not require one exact recipe chain.

## AssaultReady

The crisis has satisfied the conditions for a major raid.

Rules:

* The assault does not begin while the owner is offline.
* The state may remain ready across disconnects.
* The player receives a warning on login or transition.
* A preparation grace period is required before launch.

## AssaultActive

A major goblin attack is active.

The initial assault must be:

* Solo-completable
* Supported by existing villagers and defences
* Clearly attributed to the crisis
* Safe against duplicate spawning
* Safe against entity-despawn races
* Committed after successful launch, including during ordinary owner disconnect
* Able to continue normal NPC, structure, villager, helper, death, corpse, and
  resolution behaviour while the owner is offline
* Restricted to valid targets associated with the crisis owner at both target
  selection and destructive-action boundaries

The implementation should reuse existing enemy templates and behaviour where practical.

## Resolved

The crisis is complete after required attackers or objectives are defeated.

Resolution must happen exactly once.

Potential rewards may use existing:

* Items
* Gold
* Soulshards
* Score
* Objectives
* Resources

Do not add an entirely new reward economy.

---

# 11. Relationship with global time

Global time remains shared across the world.

The goblin crisis may prefer thematically appropriate attack timing.

Acceptable launch behaviour:

* If an assault becomes ready during daytime, it may launch at the next dusk while the owner remains online.
* If the player logs in at night with an assault already ready, provide a short grace period before launch.
* If the player logs out before launch, keep the crisis in `AssaultReady`.
* Do not require a player to remain online indefinitely waiting for a specific global tick.

The clock influences timing and atmosphere.

The clock does not independently create the assault.

---

# 12. Removing automatic dusk hordes

The current `nightly_threat_system` creates a wave when the global tick reaches `DUSK`.

In the new default personal-crisis mode:

* Do not schedule or execute that system.
* Do not spawn a wave solely because dusk occurred.
* Do not increment wave score merely because the clock reached dusk.
* Preserve the system for legacy mode where practical.
* Preserve all unrelated environmental night effects.

The following must continue functioning:

* Time-of-day display
* Vision changes
* Darkness
* Torches
* Weather
* Environmental night risk
* Nocturnal creatures not associated with scheduled settlement hordes

---

# 13. Disabling the automatic crisis ladder

In personal-crisis mode, the following old settlement-escalation systems should not run:

* Rat crisis escalation, except introductory behaviour that is genuinely required
* Wolf pack automatic tier
* Old goblin gold-raid tier
* Undead day-based tier
* Goblin pillager day-based tier
* Automatic nightly survival hordes
* Legendary Fire Dragon escalation

The legacy systems should remain available under legacy mode where practical.

Comments and tests that assume numbered days control the default prototype danger should be updated.

---

# 14. Assault entity attribution

Every NPC spawned for a personal crisis assault must be explicitly attributed.

Conceptual component:

```rust
struct CrisisAssaultUnit {
    owner_player_id: i32,
    assault_id: u64,
    spawn_generation: u32,
}
```

The implementation must not rely only on NPC faction ownership because crisis ownership is a separate concept.

Attribution is required for:

* Resolution
* Cleanup
* Identity preservation across disconnect and reconnect
* True Death interaction
* Duplicate prevention
* Metrics
* Debugging

Use existing `RunSpawnedObjs` where appropriate, but do not overload it if a dedicated crisis attribution component is clearer.

---

# 15. Disconnect during an active assault

`AssaultActive` is the commitment point. The existing online-only phase timing,
warning, grace, and launch authority remain unchanged before launch. Once a
complete wave is spawned successfully and the phase becomes `AssaultActive`,
ordinary network disconnect is a lifecycle no-op.

During an owner disconnect:

1. Keep `AssaultActive`, the same logical assault ID, the same spawn generation,
   the tracked unit IDs, survivor health, valid targets, and existing settlement
   damage.
2. Continue ordinary NPC AI, structure damage, villager defence, connected-
   helper combat, normal death, corpse/loot behaviour, and defeat tracking.
3. Reject only queued player actions whose source client disconnected.
4. Allow normal offline resolution exactly once when all required tracked units
   have normal-death evidence.
5. Never use disconnect to clean up map events or units, return to
   `AssaultReady`, increment a retry, restart grace, rebuild a wave, heal a
   survivor, replay the warning, or grant a special reward.

On reconnect the player regains control of the continuing world state. The
assault remains active with its surviving entities and current health, or the
player observes `Resolved` if defenders completed it while offline. Explicit
safe logout/offline protection is deferred.

True Death, run abandonment/recreation, start-location recycling, explicit
administrative cleanup, and invalid/corrupted run recovery remain separate from
ordinary disconnect. An unexplained missing tracked entity is logged and leaves
the assault unresolved with recovery required; it is not silently defeated or
automatically relaunched.

---

# 16. Crisis status packet

Add an outgoing crisis-status response packet.

Suggested conceptual payload:

```rust
ResponsePacket::CrisisStatus {
    kind: String,
    phase: String,
    pressure: i32,
    title: String,
    summary: String,
    next_warning: Option<String>,
    assault_ready: bool,
}
```

Adapt field names to existing network conventions.

Send the status:

* On login
* On reconnect
* On crisis creation
* On phase transition
* On meaningful pressure changes
* On assault launch
* On resolution

The server remains authoritative.

---

# 17. Minimal client presentation

Add the smallest useful client presentation.

Display:

* Crisis name
* Current phase
* Short description
* Pressure or escalation indicator
* Clear assault warning

Do not build a full quest journal in this milestone.

If a new UI panel is disproportionately expensive, use an existing status, notice, or event presentation pattern while still implementing the structured packet.

---

# 18. Resource-system integration

The resource economy remains part of the prototype.

The new crisis should use it as a source of meaningful preparation decisions.

Examples of existing-system preparation:

* Gather timber and metal for defences
* Craft improved weapons
* Produce armour
* Make healing items
* Preserve food
* Assign villagers to production
* Stock ammunition or consumables
* Upgrade the sanctuary

The crisis may observe economic activity when calculating pressure.

It must not:

* Rewrite existing recipes
* Rename broad classes of resources
* Introduce generic replacement resources
* Remove processing buildings
* Remove profession roles
* Make one obscure chain mandatory for survival

---

# 19. Scoring compatibility

The current scoring system includes:

* Days survived
* Nights survived
* Waves survived
* Crisis tier
* Legendary kills
* Other progression and defence metrics

This milestone does not include a complete scoring redesign.

Required changes:

* Do not increment wave-related values merely because a horde spawned.
* Ensure a crisis assault resolves and rewards only once.
* Avoid breaking existing database score writes.
* Add a personal-crisis completion value only if it can be done safely without broad schema work.
* Document obsolete fields for future revision.

Avoid database migrations unless required for correctness.

---

# 20. Headless testing requirements

Use the existing in-process headless harness.

Add deterministic coverage for:

1. Global day/night still advances.
2. Visibility still changes at night.
3. Dusk does not create a scheduled horde in personal-crisis mode.
4. Legacy mode can still run the old director.
5. The introductory encounter still progresses.
6. A goblin crisis enters `Signs`.
7. A goblin crisis enters `Pressure`.
8. The player receives a warning before a major assault.
9. Offline-active time does not advance.
10. An offline player cannot transition to `AssaultActive`.
11. An online player can transition to `AssaultActive`.
12. Repeated ticks do not duplicate the assault.
13. Crisis NPCs are attributed.
14. Disconnect during `AssaultActive` preserves phase, assault ID, spawn
    generation, unit identity, survivor health, AI, and one-time completion.
15. Reconnect observes the continuing active state or the offline resolution.
16. Defeating required assault units resolves the crisis exactly once.
17. Existing gathering behaviour still works.
18. Existing crafting and refining behaviour still works.
19. Farming and fishing still work.
20. Villager work queues and assignments still work.
21. True Death cleanup remains safe.
22. Start-location recycling remains safe.

---

# 21. Headless metrics

Extend headless metrics where practical to include:

* Highest crisis phase reached
* Tick or online-active time at each phase
* Major assault launched
* Major assault defeated
* Crisis resolution count
* Duplicate assault count
* Automatic dusk hordes spawned
* Crisis units alive at disconnect
* Crisis state after reconnect

In personal-crisis mode, automatic dusk-horde count should remain zero.

---

# 22. Observability

Add concise structured logging for:

* Crisis creation
* Pressure changes
* Phase transitions
* Warning delivery
* Assault readiness
* Assault launch
* Assault ID
* Missing-unit recovery requirement
* Resolution

Include where applicable:

* Player ID
* Old phase
* New phase
* Pressure
* Game tick
* Online state
* Assault ID

Do not log every tick.

Avoid excessive production logging.

---

# 23. Code-quality expectations

* Use server-authoritative state.
* Keep phase transitions idempotent.
* Avoid giant multipurpose systems where focused systems are clearer.
* Do not spawn entities from status-reporting systems.
* Explicitly attribute crisis NPCs.
* Avoid unbounded database writes.
* Avoid per-tick client status packets.
* Account for Bevy deferred-command races.
* Use safe entity commands when an entity may be despawned.
* Do not assume an entity still exists later in the same update.
* Add focused unit and integration tests.
* Preserve current behaviour outside the milestone.

---

# 24. Implementation checkpoints

## Checkpoint 1: Director separation

Implement:

* Survival-director mode
* Personal-crisis mode as default
* Legacy mode
* Disable automatic dusk hordes in personal mode
* Disable old crisis ladder in personal mode
* Preserve environmental day/night
* Preserve introductory encounters
* Add tests

## Checkpoint 2: Crisis state foundation

Implement:

* Separate introduction and crisis state
* Goblin crisis types and phases
* Pressure
* Online-active timing
* Phase transitions
* Persistence compatibility
* Logging
* Tests

## Checkpoint 3: Assault lifecycle

Implement:

* Warning and grace period
* Online-only launch
* Crisis-unit attribution
* Assault resolution
* Disconnect policy
* Reconnect policy
* Duplicate prevention
* Cleanup tests

## Checkpoint 4: Network, UI, and validation

Implement:

* Crisis status packet
* Login/reconnect status delivery
* Minimal client display
* Headless metrics
* Multi-run validation
* Documentation updates

Each checkpoint should remain independently reviewable.

---

# 25. Out of scope

Do not implement during this milestone:

* Larger maps
* Approximately 25-player worlds
* Multiple-world orchestration
* Cross-world trade
* Cross-world travel
* World migration
* Seasonal resets
* Guilds
* PvP
* Distress beacons
* Regional goblin strongholds
* Regional crisis framework
* Offline shops
* Complex offline production
* Offline combat simulation
* Full settlement ruin system
* Successor settlement system
* Fire Dragon redesign
* Ranger redesign
* Mage redesign
* Full leaderboard redesign
* Resource-system simplification
* New crafting professions
* New resource tiers

These should remain documented follow-up work.

---

# 26. Validation commands

Determine exact commands from repository documentation and configuration.

At minimum, run applicable equivalents from `sp_server/`:

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features
cargo run --bin headless_runner -- --help
```

Run appropriate headless samples after gameplay changes.

Do not report a command as passing unless it actually ran successfully.

If a dependency or environment prevents a command from running, document:

* The exact command
* The exact error
* The affected validation
* Other checks that did complete

---

# 27. Definition of done

This milestone is complete when:

1. Global day/night remains functional.
2. Environmental darkness and visibility still change normally.
3. Dusk alone no longer spawns settlement hordes in personal-crisis mode.
4. Legacy mode preserves the old director for comparison.
5. The introductory encounter still functions.
6. The old automatic crisis ladder does not run in personal-crisis mode.
7. A goblin personal crisis progresses through explicit phases.
8. Progression uses player or settlement activity rather than only global day count.
9. The player receives clear warnings.
10. A major assault launches only while the owner is online.
11. After launch, ordinary disconnect leaves the active assault committed and
    normal world combat continues.
12. Reconnect preserves the same assault identity, generation, surviving units,
    current health, and world damage, or observes the already-resolved state.
13. Crisis enemies are explicitly attributed.
14. Duplicate assaults cannot spawn.
15. Resolution occurs exactly once.
16. Existing resource and production systems remain intact.
17. Existing economy-related tests continue passing.
18. New focused unit and headless tests cover the core rules.
19. The selected disconnect policy is documented.
20. Known limitations and deferred work are recorded.

---

# 28. Expected implementation report

At the end of the milestone, the implementation report should include:

1. Architecture inspected
2. Files changed
3. Behaviour before and after
4. Survival-director configuration
5. Crisis-state design
6. Disconnect and reconnect policy
7. Tests and commands run
8. Headless-run results
9. Known limitations
10. Deferred milestones
11. Any deviations caused by repository architecture

The implementation must not claim that the entire long-term Siege Perilous redesign has been completed.

This milestone implements only the persistent personal-crisis foundation.
