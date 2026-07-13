# Milestone 2 — Explicit Safe Logout and Offline Protection

## Status

Checkpoint 1, **Safe Logout State Foundation and Eligibility**, is the only
checkpoint in scope for this implementation record. The implementation and its
focused, broad, Clippy, and headless validation are complete; exact results are
recorded below.

> Checkpoint 1 does not yet provide complete offline protection and is
> intentionally not exposed through the production client or network protocol.

The authoritative gameplay rule for the milestone is:

> Ordinary disconnect provides no protection. Offline protection can only
> result from an explicit safe-logout flow that successfully completes all
> server-authoritative safety checks.

The completed personal-crisis commitment rule is unchanged:

> Once a personal assault enters `AssaultActive`, it continues in the
> persistent world even if the owner disconnects.

Checkpoint 1 creates a runtime presence model, internal-only request and cancel
messages, eligibility validation, a cancellable game-tick countdown, lifecycle
cleanup, and headless test support. It does not make the reserved protected
state affect simulation.

## Checkpoint breakdown

1. **Checkpoint 1 — state foundation and eligibility:** authoritative presence,
   internal request/cancel messages, eligibility, countdown, cancellation,
   connection synchronization, cleanup, observability, and headless support.
2. **Checkpoint 2 — protected simulation gates:** apply the completed protected
   state to the specifically approved world and per-run simulation systems.
3. **Checkpoint 3 — protocol and client UI:** add a production request/response
   protocol, safe-logout control, countdown presentation, and cancellation
   feedback.
4. **Checkpoint 4 — reconnect, telemetry, and exploit validation:** complete
   restoration semantics, observability, race/exploit coverage, and final
   milestone validation.

Only item 1 is implemented here.

## Checkpoint 1 architecture findings

### Connection and player identity

* The authoritative player identity is the account/session `i32` player ID.
  Authentication in `sp_server/src/network.rs` inserts a `Client` containing
  that player ID into the shared `Clients` map under a connection UUID. For an
  existing run it then sends `PlayerEvent::Login` to the game.
* `Clients` in `sp_server/src/game.rs` is an
  `Arc<Mutex<HashMap<Uuid, Client>>>`. `Clients::is_player_online(player_id)`
  requires a matching player ID, a map key equal to `Client.id`, and an open
  game-to-client sender. A poisoned lock fails closed. More than one connection
  can temporarily exist; one valid remaining connection keeps the player
  online.
* Clean close, socket/protocol failure, game-requested disconnect, and session
  manager replacement remove a connection UUID in `network.rs`. Those paths do
  not emit an ECS disconnect event. The narrow integration is therefore a
  server schedule reconciliation against `Clients`, rather than a new network
  packet or a second socket-lifecycle channel.
* A hero entity remains in the ECS after an ordinary disconnect. Hero existence
  is not online presence and must never imply protection.
* `PlayerEvent::Login` is processed by `login_system` in `player.rs`, which
  schedules the established delayed `GameEventType::Login` used for map,
  perception, objective, and crisis synchronization. Presence can be marked
  online at this established event without changing those payloads.

### Authoritative run and hero

* `AssignedStartLocations` in `player_setup.rs` is the current in-memory run
  assignment keyed by player ID. A safe-logout request fails when that entry is
  absent.
* `Ids.player_hero_map` maps the player ID to the authoritative hero object ID.
  `EntityObjMap` maps that object ID to the live ECS entity. Checkpoint 1 uses
  both mappings and verifies the entity's `Id`, human `PlayerId`, and
  `SubclassHero` marker; it does not search for an arbitrary nearby hero.
* A living hero requires a live `State`, no `StateDead`, no `TrueDeath`, and
  positive `Stats.hp`. Missing or stale mappings fail closed.
* Successful `NewPlayer` setup is the fresh-run boundary. It initializes a
  clean presence record consistent with current client presence. Failed start
  allocation does not initialize a record.
* `true_death_system` performs final run cleanup more than ten seconds after the
  `TrueDeath` marker. The marker itself must reject or cancel safe logout
  immediately; final cleanup removes the per-run presence record alongside
  crisis, intro, objective, score, spawn, and start-location state.

### Sanctuary ownership

* A hero's `BoundMonolith.id` is the authoritative ownership relationship to a
  sanctuary. `BoundMonolith.pos` is retained by existing gameplay but is not
  sufficient to prove the current monolith is live.
* `SanctuaryZones` in `game.rs` is rebuilt each update from entities carrying
  `Monolith` and is keyed by monolith object ID. That sync does not itself
  filter death markers, so safe logout separately verifies live state. Each
  `SanctuaryZone` contains the current position and level and exposes
  `full_radius()` and `weak_radius()`.
* `SanctuaryZones::in_full_zone` accepts *any* sanctuary, and `nearest()` may
  select another player's zone. Neither is suitable for safe logout.
* Checkpoint 1 looks up the exact `BoundMonolith.id`, requires its matching zone,
  resolves that same ID through `EntityObjMap`, verifies a live monolith and
  requires the cached binding position, live position, and zone position to
  agree, then applies the existing strict full-zone boundary:
  `Map::distance(hero, zone.pos) < zone.full_radius()`.
* The full sanctuary is selected because it is the existing complete encounter-
  suppression and defensive zone. The weak outer ring does not qualify.
  Missing binding, missing zone, missing entity, stale ID, dead monolith, or
  inconsistent position fails closed. Another player's sanctuary never serves
  as a fallback.

### Movement, combat, and damage

* Hero movement is already server-authoritative in `player.rs` and committed to
  the hero's `Position`. A pending request records one starting `Position` and
  compares the current authoritative value; no client claim or continuous path
  history is required.
* `LastCombatTick` in `obj.rs` is the existing per-entity combat watermark.
  Successful normal attacks and combos update both combatants in `combat.rs`;
  accepted damaging abilities and player combat actions also update combat
  state in `player.rs`. Rejected, malformed, out-of-range, dead-source,
  wrong-owner, insufficient-resource, or cooldown-blocked input is not combat
  activity.
* An accepted player command may use another player-owned combatant rather than
  the hero. A small per-player aggregate therefore records successful attack,
  damaging-ability, and combo initiation without replacing the entity's
  existing `LastCombatTick`. Rejected actions do not update either source.
  Existing hero `LastCombatTick` remains a deliberately conservative combat
  watermark (and currently includes Ward); movement caused by an accepted
  action is also caught by the position comparison.
* The repository did not have a single authoritative incoming-damage watermark.
  Checkpoint 1 adds `LastDamageTick` to the combat object data and writes it at
  actual combat, spell/ability, and world-damage sites. The presence record
  retains the requesting player's latest observed damage tick for eligibility
  and cancellation.
* An authoritative hero-HP decrease observed during reconciliation is retained
  as a fallback for a damage path that does not yet write the component. This
  is intentionally narrow and avoids routing the presence resource through
  every producer of damage. Healing or unchanged HP is not damage.
* Blocking is not a damaging action and is not independently a cancellation
  reason. If blocking accompanies incoming damage, that actual damage cancels.

### Hostile NPC identification

* There is no single general-purpose `is_hostile_to_player` predicate shared by
  every NPC and faction path. The ordinary NPC target scorer combines template
  aggression, NPC markers, live state, visible-target capability, and special
  personal-assault ownership rules.
* The Checkpoint 1 immediate-threat query therefore requires `SubclassNPC`,
  `VisibleTarget`, `Subclass::Npc`, NPC ownership, live state, no `StateDead`,
  positive HP, and non-passive template aggression. Missing aggression metadata
  is treated as threatening rather than safe. Because the query runs in
  `PostUpdate`, deferred Update despawns have already been applied; a still-live
  orphan ECS threat fails closed as hostile instead of being hidden by a stale
  object map.
* An attributed personal-assault unit blocks only its owning player, matching
  the completed cross-settlement target restriction. An unrelated player's
  attributed assault is not silently reclassified as a threat to this
  settlement.
* This excludes dead corpses, friendly villagers, heroes, merchants,
  decorations, passive wildlife, despawned entities, and other non-hostile
  objects. Distance uses the map's authoritative metric and the inclusive
  safety boundary.

### Personal-crisis interaction

* `SettlementCrisisState` remains the authoritative personal-crisis resource.
  Its phases are `Dormant`, `Signs`, `Pressure`, `Preparing`, `AssaultReady`,
  `AssaultActive`, and `Resolved`.
* Pre-assault pressure and timing already use `Clients::is_player_online`.
  `personal_crisis_assault_system` owns the launch grace, anchor, spawn,
  attribution, and commitment transition. The assault lifecycle continues to
  evaluate normal combat while the owner is disconnected.
* Safe logout is allowed to start in any non-active phase when every other
  check succeeds. `AssaultActive` rejects a new request and cancels a pending
  request.
* Presence transitions do not alter pressure, phase, warnings, grace,
  `assault_id`, spawn generation, tracked attackers, attacker HP, targets,
  settlement damage, resolution, rewards, or status delivery.
* Ordinary disconnect remains a lifecycle no-op for a committed assault.

### Headless harness

* `build_headless_app_with_director` constructs the production gameplay plugins
  with an in-process client map, database channels, deterministic updates, and
  explicit survival-director mode.
* `HeadlessGame` inserts a real `Client`, drives production `PlayerEvent`s,
  changes `GameTick` deterministically, and already supports removal/reinsertion
  of the client while leaving the hero in the world. Existing disconnect and
  reconnect helpers therefore exercise the same authoritative `Clients`
  semantics used by production.
* The harness can be extended in place with internal message writers and
  read-only presence inspection. No separate simulator is needed.

## Files affected by Checkpoint 1

* `sp_server/src/safe_logout.rs` — new presence model, constants, internal
  messages, eligibility, reconciliation, countdown, cancellation, ordering,
  logs, and focused unit/system coverage.
* `sp_server/src/obj.rs` — authoritative `LastDamageTick` combat-object data.
* `sp_server/src/combat.rs` — actual attack/combo incoming-damage watermark
  updates.
* `sp_server/src/player.rs` — fresh-run and login integration, successful
  player-commanded combat tracking, and damaging ability updates.
* `sp_server/src/game.rs` — plugin registration, non-combat world-damage
  watermark updates, and True Death/run cleanup integration.
* `sp_server/src/lib.rs` — module exposure so production and headless app
  builders install the same server-side foundation.
* `sp_server/src/headless.rs` — request/cancel, state inspection, movement,
  hostile, tick, damage, combat, disconnect/reconnect helpers, plus the two
  deterministic Checkpoint 1 scenarios.
* `docs/safe_logout_milestone.md` — architecture, design, acceptance criteria,
  validation record, limitations, and deferred work.

The following files were audited and intentionally receive no Checkpoint 1
semantic change:

* `sp_server/src/network.rs` — existing authentication and connection removal
  are reused; no packet variant or handler is added.
* `sp_server/src/event.rs` — existing game/map event types are reused; safe
  logout is not placed on the production packet-to-`PlayerEvent` path.
* `sp_server/src/player_setup.rs` — existing authoritative run assignment and
  hero construction are reused.
* `sp_server/src/game_tests.rs` — audited; its existing shared fixtures and the
  full test suite remain unchanged.
* Personal-crisis AI, frontend, economy, database, map, deployment, and
  infrastructure files remain unchanged.

## Repository conflicts and selected resolutions

| Repository reality | Conflict with the conceptual design | Checkpoint 1 resolution |
| --- | --- | --- |
| Socket paths remove `Client` UUIDs but emit no ECS disconnect event. | A dedicated disconnect command cannot be assumed. | Reconcile authoritative `Clients` once per running update. Do not change the protocol. |
| Hero entities survive socket closure. | ECS hero existence cannot mean `Online`. | Derive connection state only from `Clients::is_player_online`. |
| One player may briefly have multiple client records. | Removing one connection must not disconnect the player if another is valid. | Treat any valid open record as online; transitions are state-idempotent. |
| `SanctuaryZones::in_full_zone` accepts any sanctuary. | Standing in another player's sanctuary would qualify. | Resolve only the exact `BoundMonolith.id`; fail closed. |
| The repository has no centralized universal hostility predicate. | A broad `SubclassNPC` query would count merchants, corpses, or passive wildlife. | Use the existing live monster/target/aggression signals and personal-assault ownership rule. |
| `LastCombatTick` is per entity. | A player can command an owned combatant other than the hero. | Retain `LastCombatTick` and add a minimal successful-command player aggregate. |
| Damage was written by several combat and world systems. | A single request check could miss a same-update damage source. | Add `LastDamageTick` at actual damage sites and retain HP-delta observation as a fail-safe. |
| `Clients` is shared with asynchronous socket tasks. | A client can disappear between an early update sample and a due completion. | Take authoritative connection samples on both sides of the provisional exclusive ECS commit; either failed sample publishes only `Disconnected`, and the second successful sample is the handoff boundary. |
| Crisis, death, movement, and combat systems run across the existing `Update` schedule. | A completion evaluator in the same unordered set could win a race. | Run a chained safe-logout evaluator in `PostUpdate`, after `Update` and deferred command application. |
| `TrueDeath` final cleanup is delayed. | A pending logout could otherwise finish during death processing. | Treat the marker as immediately ineligible/cancelling; remove the record at final cleanup. |
| `OfflineProtected` has no simulation gates yet. | Exposing it would imply protection that is not implemented. | Reserve and test the transition internally only; add no production ingress. |
| Comparable per-run state is runtime-only and not coherently restored. | Persisting only presence would create partial restart semantics. | Keep the Checkpoint 1 resource in memory; add no schema, save, or reload format. |

## Selected state model

The named server-authoritative resource is conceptually:

```rust
enum PlayerWorldPresence {
    Online,
    SafeLogoutPending,
    OfflineProtected,
    Disconnected,
}

struct PlayerPresenceRecord {
    state: PlayerWorldPresence,
    safe_logout_requested_tick: Option<i32>,
    safe_logout_start_position: Option<Position>,
    last_combat_tick: Option<i32>,
    last_damage_tick: Option<i32>,
    cancel_reason: Option<SafeLogoutCancelReason>,
    rejection_reason: Option<SafeLogoutRejectionReason>,
    // Internal observation fields include the HP baseline and last known
    // connection state; authoritative hero/client state remains external.
}

#[derive(Resource, Default)]
struct PlayerWorldPresenceState {
    players: HashMap<i32, PlayerPresenceRecord>,
}
```

The state meanings are:

* `Online`: at least one authoritative active client exists; normal simulation
  applies.
* `SafeLogoutPending`: an explicit request passed validation and its countdown
  is running; the hero, settlement, crisis, AI, needs, work, and economy remain
  fully active and vulnerable.
* `OfflineProtected`: the internal countdown completed. In Checkpoint 1 this is
  a reserved state only; it does not freeze, remove, pause, heal, repair, or
  protect anything.
* `Disconnected`: no authoritative client exists and no completed protected
  handoff applies. It implies no protection.

The resource is per process and per run. It is neither reflected into the
dynamic scene nor written to the database.

## Internal request mechanism

Checkpoint 1 uses Bevy messages equivalent to:

```rust
struct RequestSafeLogout { player_id: i32 }
struct CancelSafeLogout { player_id: i32 }
```

They are registered inside the server plugin and can be written only by server
tests, headless helpers, or future internal server code. They are not variants
of `NetworkPacket`, `ResponsePacket`, or the deserialized production
`PlayerEvent` input. The frontend has no button or message for them.

The manual cancellation message is retained because it gives tests and the
future protocol a single idempotent cancellation boundary without changing the
automatic cancellation rules.

## Named tuning

```rust
SAFE_LOGOUT_COUNTDOWN_TICKS = TICKS_PER_SEC * 10
SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS = TICKS_PER_SEC * 15
SAFE_LOGOUT_HOSTILE_RADIUS = 8
```

The first two values use authoritative game ticks. The radius uses the map's
tile-distance type. Raw copies of these values must not appear in eligibility
or countdown systems.

## Eligibility and rejection

A request enters `SafeLogoutPending` only when all of the following hold at the
server evaluation point:

1. The presence record is `Online` and `Clients::is_player_online` is true.
2. `AssignedStartLocations` contains the player's current run.
3. `Ids` and `EntityObjMap` resolve the matching human hero.
4. The hero is alive, has positive HP, and has neither `StateDead` nor
   `TrueDeath`.
5. The player's exact bound monolith and exact live `SanctuaryZones` entry are
   valid, and the hero is inside that zone's full radius.
6. The personal crisis is not `AssaultActive`.
7. Neither successful outgoing combat nor incoming damage is within the
   15-second game-tick cooldown.
8. No qualifying immediate hostile is within the inclusive eight-tile radius.
9. The state is neither already pending nor already protected.

Rejections are machine-readable, with reasons for not online, invalid run,
missing hero, dead hero, True Death, missing binding, missing sanctuary zone,
invalid sanctuary, outside own sanctuary, active assault, recent combat,
recent damage, nearby hostile, already pending, and already protected.

A rejected request does not alter the current state, restart a pending timer,
modify crisis state, remove entities, grant rewards, or create protection.

## Countdown and idempotency

An accepted request:

1. changes `Online` to `SafeLogoutPending`;
2. stores the current `GameTick` and authoritative hero `Position`;
3. clears the prior cancellation and rejection reasons; and
4. emits transition-only request/countdown logs.

The hero remains connected and simulation remains unchanged. Countdown progress
is derived from `current_tick.saturating_sub(requested_tick)`, not an incremented
counter. Re-evaluating one tick therefore cannot advance twice, tick rollback
cannot underflow, and a duplicate request cannot restart the countdown.

Completion is eligible when the full named interval has elapsed and every
cancellation check still passes. The transition to `OfflineProtected` occurs
once, clears the pending tick and position, and emits one completion log. It
does not disconnect the client, remove or freeze the hero, alter a crisis, or
gate any simulation system.

## Automatic and manual cancellation

`SafeLogoutCancelReason` contains the typed reasons `Moved`, `EnteredCombat`,
`TookDamage`, `HostileNearby`, `LeftSanctuary`, `SanctuaryInvalid`,
`AssaultStarted`, `HeroDied`, `Disconnected`, `Manual`, and `RunEnded`.

A pending countdown cancels when:

* current authoritative hero position differs from the recorded position;
* accepted attack, damaging ability, or combo activity occurs at or after the
  request tick;
* actual incoming damage occurs at or after the request tick;
* a qualifying hostile enters the safety radius;
* the hero leaves the exact full sanctuary;
* the bound monolith, zone, entity mapping, or ownership relationship becomes
  invalid;
* the personal crisis enters `AssaultActive`;
* the hero dies or enters True Death processing;
* the last authoritative client disappears;
* an internal manual cancellation is received; or
* the run is removed or replaced.

Cancellation returns a connected player to `Online` and a disconnected player
to `Disconnected`, stores one typed reason, clears pending-only fields, and logs
the transition once. It never changes pressure, phase, assault identity,
attackers, damage, score, reward, inventory, or production.

## Login, reconnect, disconnect, and cleanup

* A successful login or reconnect for a valid run produces `Online`. An
  authenticated Login observed during a pending handoff conservatively cancels
  it, covering a socket gap that may have occurred between ECS evaluations.
  Duplicate Login/reconcile evaluation after the first transition is
  idempotent.
* The loss of the last active client changes `Online` to `Disconnected`.
  `SafeLogoutPending` first cancels with `Disconnected` and ends in
  `Disconnected`. Repeated evaluation has no unrelated side effect.
* An ordinary disconnect never starts a countdown and never changes directly to
  `OfflineProtected`.
* Because the internal completion does not close the client in Checkpoint 1, a
  completed record can still have a live connection. Tests may remove that
  connection and later reconnect; reconnect returns the reserved state to
  `Online`. This is foundation behavior, not the final Checkpoint 4 restoration
  policy.
* Disconnect and reconnect do not mutate any personal-crisis field. In
  particular, a committed assault keeps its assault ID, generation, tracked
  units, survivor HP, targets, phase, and resolution behavior.
* True Death and successful fresh-run creation remove or replace only that
  player's record. Pending fields, protected state, combat/damage timestamps,
  HP baseline, and cancellation history do not cross into the fresh run.
  Repeated cleanup is safe.

## Explicit schedule ordering

The safe-logout plugin runs this chain in `PostUpdate`, under
`AppState::Running`:

1. reconcile live run/client presence and observe incoming damage;
2. process internal safe-logout requests;
3. process internal manual cancellations; and
4. evaluate pending cancellation before completion.

`PostUpdate` runs after the existing `Update` systems and their deferred command
application. This lets the evaluator observe authoritative movement, combat,
damage, sanctuary synchronization, personal-crisis launch, death markers, and
True Death cleanup from the update before considering completion.

Within pending evaluation, terminal safety conditions precede time completion:
disconnect/run loss, death/True Death, `AssaultActive`, damage, outgoing combat,
movement, sanctuary loss/invalidation, and hostile proximity are checked before
the elapsed countdown. Consequently same-update assault commitment, damage,
death/True Death, or disconnect wins over an otherwise due completion.

At the final completion boundary, the server samples authoritative `Clients`
presence immediately before and after a provisional `OfflineProtected` write.
The presence resource is exclusively borrowed, so no ECS consumer can observe
that provisional value. If either sample observes the client absent or its
sender closed, the write is rolled back to `Disconnected`, pending fields are
cleared, and only a typed `Disconnected` cancellation is logged. A disconnect
after the second successful sample is ordered after completed safe logout.

The safe-logout systems do not order or gate the legacy director. Legacy and
personal director behavior remains exactly as established by the completed
persistent-crisis milestone.

## Observability

Transition-only structured logs cover:

* run presence initialization;
* explicit request and countdown start;
* rejection with typed reason;
* cancellation with typed reason;
* countdown completion;
* ordinary disconnect;
* reconnect; and
* True Death/run cleanup.

Logs include player ID, previous and new state, authoritative game tick, and the
applicable cancellation/rejection reason. The countdown does not log every
tick, and no session token, password, or other credential is included.

## Headless support and deterministic scenarios

The existing `HeadlessGame` is extended with minimal operations to request and
cancel safe logout; inspect state, pending tick, and cancellation/rejection
reason; place the hero at the own sanctuary; move the hero; place/remove a
hostile; advance/set the game tick; simulate real damage and combat activity;
and disconnect/reconnect through the real `Clients` map.

The first deterministic scenario must:

1. create a valid connected player/run in the player's own sanctuary;
2. establish no recent combat/damage and no nearby hostile;
3. request safe logout and advance half the countdown;
4. observe `SafeLogoutPending`;
5. move the authoritative hero and observe one `Moved` cancellation to
   `Online`;
6. restore safety, request again, and advance the full countdown; and
7. observe exactly one `OfflineProtected` transition.

The second deterministic scenario must:

1. begin a valid pending countdown;
2. commit that player's crisis to `AssaultActive` before completion;
3. observe one `AssaultStarted` cancellation;
4. disconnect through the ordinary client path; and
5. prove presence is `Disconnected` while the same committed assault and units
   remain active.

## Required test matrix

Checkpoint 1 coverage is grouped below without replacing the existing crisis,
economy, or headless suites.

### State initialization

1. Connected run initializes `Online`.
2. Ordinary disconnect becomes `Disconnected`.
3. Ordinary disconnect never becomes `OfflineProtected`.
4. Reconnect becomes `Online`.
5. Duplicate disconnect is idempotent.
6. Duplicate reconnect is idempotent.

### Eligibility

7. Valid online hero inside the own sanctuary begins pending.
8. Outside own sanctuary is rejected.
9. Another player's sanctuary does not qualify.
10. Missing bound monolith rejects safely.
11. Missing sanctuary zone rejects safely.
12. Dead hero rejects safely.
13. True Death processing rejects safely.
14. Recent outgoing combat rejects safely.
15. Recent incoming damage rejects safely.
16. Nearby hostile rejects safely.
17. Dead hostile corpse does not block.
18. Friendly villager does not block.
19. Merchant or other non-hostile NPC does not block.
20. `AssaultActive` rejects.
21. Pre-assault phases do not independently reject.

### Countdown

22. Accepted request enters `SafeLogoutPending`.
23. Countdown is based on `GameTick`.
24. Countdown cannot complete early.
25. Countdown completes exactly once.
26. Completion enters `OfflineProtected`.
27. Duplicate pending request does not restart its start tick.
28. Duplicate evaluation of one tick does not advance twice.

### Cancellation

29. Movement cancels.
30. Successful attack activity cancels.
31. Successful damaging ability activity cancels.
32. Successful combo activity cancels.
33. Incoming damage cancels.
34. Hostile entering the radius cancels.
35. Leaving the own sanctuary cancels.
36. Sanctuary invalidation cancels.
37. Assault launch cancels.
38. Hero death cancels.
39. Pre-completion disconnect cancels to `Disconnected`.
40. Manual internal cancellation works.
41. Cancellation occurs once.
42. Cancellation does not change crisis pressure.
43. Cancellation does not remove assault units.
44. Cancellation grants no rewards.

### Ordering and cleanup

45. Same-update assault launch wins over completion.
46. Same-update damage wins over completion.
47. True Death wins over completion.
48. Fresh run has no stale pending state.
49. Fresh run has no stale protected state.
50. Cleaning one run does not alter another player's record.
51. Repeated cleanup does not panic.

### Regression coverage

52. Active assault continues after ordinary disconnect.
53. Active assault keeps the same assault ID and generation.
54. Active attackers are not despawned by disconnect.
55. Connected helpers can fight an offline owner's assault.
56. Cross-player settlement targeting remains prohibited.
57. Pre-assault personal-crisis timing retains its existing rules.
58. Personal mode still has no automatic dusk horde.
59. Legacy director behavior remains intact.
60. Existing introduction, combat, resource/production, crafting, farming,
    fishing, refining, trade, villager, cleanup, and headless suites continue
    passing.

## Validation record

Validation has not yet been run for this implementation record. Do not read the
items below as passing results. After implementation, record the exact command,
test count/result, and any retained pre-existing warning set.

Required commands from `sp_server/`:

```bash
cargo fmt --check
cargo check
cargo test checkpoint1 -- --nocapture
cargo test
cargo clippy --all-targets --all-features
```

The focused command name may use the final shared Checkpoint 1 test prefix, but
its output must remain visible. The two deterministic headless scenarios above
must also be run with visible output. A command may be recorded as passing only
after its successful execution; an environment failure must include the exact
command and exact error.

## Known Checkpoint 1 limitations

* `OfflineProtected` does not yet protect the hero, structures, villagers,
  settlement, inventories, needs, work queues, crafting, refining, farming,
  fishing, trade, or crisis clocks.
* Because no Checkpoint 2 gate exists, pre-assault crisis escalation can still
  occur after the internal test-only completion transition while the client
  remains connected. Production cannot reach this state in Checkpoint 1.
* Completion does not disconnect the socket, remove the hero, hide the player,
  or send a client response.
* The feature has no production packet, frontend control, countdown UI, or
  cancellation presentation and is therefore intentionally unreachable by
  normal players.
* Presence and activity state is process-memory-only. There is no database
  schema, server-restart restoration, or dynamic-scene persistence.
* Client presence is normally reconciled once per `PostUpdate`. A due
  completion additionally samples `Clients` immediately before and after its
  provisional exclusive ECS transition, with the second successful sample as
  the handoff boundary. The repository still has no independent ECS
  socket-lifecycle event stream; authenticated Login conservatively cancels an
  in-flight handoff.
* The incoming-damage HP observation is a fallback for missed damage writers,
  not a general damage-history system.
* Hostility is derived from the current NPC combat markers and aggression data
  because the repository has no universal faction relationship service.
* Checkpoint 1 does not settle the final reconnect restoration policy for a
  protected session; reconnect returns to `Online` for this internal prototype.
* An active personal assault remains unsafe and continues after ordinary
  disconnect exactly as it did before this milestone.

## Work explicitly deferred to Checkpoint 2

Checkpoint 2 must define and implement the actual protected simulation gates.
That includes deciding, system by system, what stops or remains active for the
protected owner's hero, structures, villagers, needs, work, production, and
pre-assault crisis timing. It must preserve the committed `AssaultActive`
exception and must not retroactively turn ordinary disconnect into protection.

Also deferred beyond Checkpoint 2 are the production protocol/client UI
(Checkpoint 3), completed restoration/telemetry/exploit work (Checkpoint 4),
automatic socket closure, persistence across server restart, offline
production or shops, passive repair/healing, regional crises, new crisis
families, guild/party systems, larger maps, and cross-world systems.
