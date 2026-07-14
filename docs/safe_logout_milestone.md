# Milestone 2 — Explicit Safe Logout and Offline Protection

## Status

Checkpoints 1, **Safe Logout State Foundation and Eligibility**, 2,
**Protected Simulation Enforcement**, and 3, **Network Protocol and
Player-Facing UI**, are implemented. This update records the Checkpoint 3
architecture, implementation, and validation while retaining the Checkpoint 1
and 2 records below. Checkpoint 4 hardening and final milestone sign-off remain
deferred.

The authoritative gameplay rule for the milestone is:

> Ordinary disconnect provides no protection. Offline protection can only
> result from an explicit safe-logout flow that successfully completes all
> server-authoritative safety checks.

The completed personal-crisis commitment rule is unchanged:

> Once a personal assault enters `AssaultActive`, it continues in the
> persistent world even if the owner disconnects.

Checkpoint 1 created the runtime presence model, internal-only request and
cancel messages, eligibility validation, cancellable countdown, lifecycle
cleanup, and headless support. Checkpoint 2 makes the completed state enforce an
owner-scoped freeze, preserves that state across disconnect, rejects protected
mutations, prevents hostile targeting and damage, and rebases owner deadlines
before reconnect resumes simulation. Checkpoint 3 exposes that authority through
authenticated production commands, deduplicated status snapshots, a desktop
sanctuary control, and an intentional protected-confirmation close flow.

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

Items 1 through 3 are implemented. Item 4 remains deferred.

## Checkpoint 3 implementation record

Checkpoint 3 adds only the production request/cancellation protocol, structured
status delivery, compact desktop presentation, and intentional client-close
flow. It reuses the Checkpoint 1 eligibility/countdown authority and Checkpoint
2 simulation gates; it does not recreate or weaken either checkpoint.

> Safe logout is now available through the production protocol and client. Protection remains server-authoritative and begins only after the server confirms `OfflineProtected`.

> Closing the client before countdown completion remains an ordinary unprotected disconnect.

### Protocol and client architecture findings

* A WebSocket is authenticated before its gameplay loop starts. The upgrade
  handler resolves the session cookie through the existing `sessions` table,
  obtains the authoritative `player_id`, creates a fresh connection UUID, and
  inserts `Client { id, player_id, sender }` into `Clients`. The command loop
  therefore has an authenticated connection identity; safe-logout commands do
  not need and must not accept a player ID.
* The asynchronous network task cannot write Bevy messages directly. Its only
  production ingress into the ECS schedule is the crossbeam `PlayerEvent`
  channel consumed by `message_broker_system`. The smallest compatible bridge
  is consequently a pair of lifecycle-only `PlayerEvent` variants that are
  resolved from the authenticated `Client` UUID and converted during `Update`
  into the existing `RequestSafeLogout` and `CancelSafeLogout` Bevy messages.
  The network task never changes presence, evaluates eligibility, or starts or
  completes a countdown.
* Existing outgoing packets use an internally tagged `ResponsePacket` with
  `packet` as the discriminator. The crisis protocol established the compatible
  flat-payload pattern: a versioned snapshot is flattened into the tagged enum
  variant and absent optional values are omitted by `skip_serializing_none`.
* `send_to_client` broadcasts to every connection for one player and does not
  report whether a particular sender accepted the packet. Safe-logout delivery
  needs connection-specific login/reconnect synchronization and retry after a
  full channel, so it uses each validated `Client.sender` directly and caches
  only a successfully queued snapshot per connection UUID.
* Safe-logout transitions are committed in the chained `PostUpdate` systems.
  Status delivery must be the final member of that chain. In particular, the
  completion snapshot is constructed only after `safe_logout_pending_system`
  has changed presence to `OfflineProtected`, and a protected reconnect is
  reported as online only after the exclusive Checkpoint 2 timer rebase has
  completed.
* The existing Checkpoint 1 `request_rejection` function already owns the live
  hero, exact sanctuary, hostility, combat, damage, crisis, run, connection,
  and presence checks. Checkpoint 3 wraps that read-only result with
  `in_own_sanctuary` and `active_assault` presentation facts. Both request
  handling and status construction consume the same result; the client has no
  parallel eligibility model.
* The internal presence/reason strings are intentionally optimized for logs and
  predate the public protocol (`safe_logout_pending`, `offline_protected`,
  `manual`, and `disconnected`, for example). Checkpoint 3 retains those
  internal values and defines a separate stable wire mapping rather than
  silently changing Checkpoint 1 observability.
* The desktop monolith surfaces provide generic inspect, transfer, and
  investigate actions. They do not know whether the selected monolith is the
  hero's authoritative `BoundMonolith`, so placing an owner-only control there
  would require unsafe client inference or a broader interaction redesign. The
  existing desktop Survival Thread is server-status-driven, already owns
  crisis/objective lifecycle resets, and already supports compact and wide
  layouts. It is the selected safe-logout surface.
* The frontend network class has no unconditional automatic reconnect loop.
  Ordinary close/error schedules the existing server-offline or reconnect error
  surface, and the player may then reconnect explicitly. Intentional safe
  logout therefore suppresses only that existing failure path. It does not
  change ordinary network-failure behavior or add a new reconnect subsystem.
* The production login component automatically reconnects a valid browser
  session during initial mount. A completed safe logout must survive the
  intentional return to the title surface without immediately entering the
  world again. A narrowly scoped session-storage suppression flag persists
  until the player explicitly begins a new login; the one-time completion copy
  is consumed independently.
* The repository has Jest packages but no TypeScript transform, component DOM
  environment, Jest configuration, or test script. The established practical
  client pattern is dependency-free pure TypeScript helpers tested with Node,
  followed by TypeScript compilation and both production webpack bundles.

### Files affected by Checkpoint 3

* `sp_server/src/network.rs` — fieldless request/cancel commands,
  connection-owner resolution, flat status packet schema, and protocol/auth
  tests.
* `sp_server/src/player.rs` — lifecycle `PlayerEvent` bridge into the existing
  internal Bevy messages.
* `sp_server/src/safe_logout.rs` — shared eligibility result, stable reason
  presentation, canonical status builder, per-connection delivery cache,
  countdown throttling, and focused delivery tests.
* `sp_server/src/headless.rs` — sparse safe-logout packet capture, production-
  ingress helpers, and the four deterministic protocol scenarios.
* `sp_frontend/sp_ts/src/sp/core/network.ts` — typed commands/response,
  dispatcher integration, request methods, and protected-confirmation close.
* `sp_frontend/sp_ts/src/sp/core/networkEvent.ts` — safe-logout status, reset,
  and completion events.
* `sp_frontend/sp_ts/src/sp/core/safeLogoutStatus.ts` — typed snapshot,
  presentation mapping, duplicate-action guards, intentional-close guard, and
  completion/suppression storage helpers.
* `sp_frontend/sp_ts/src/sp/core/safeLogoutStatus.test.ts` — dependency-free
  protocol/view/lifecycle assertions.
* `sp_frontend/sp_ts/src/sp/desktop/ui/objectivesPanel.tsx` — sanctuary-visible
  Safe Logout section, explanation, server countdown, cancellation, reasons,
  accessibility, and lifecycle clearing.
* `sp_frontend/sp_ts/src/sp/desktop/login.tsx` — intentional return to the
  existing landing surface, one-time confirmation, and explicit-login reset.
* `docs/safe_logout_milestone.md` — this implementation and validation record.

No database, authentication, deployment, mobile-specific, map, crisis-balance,
resource, recipe, production, profession, or persistence file receives a
Checkpoint 3 semantic change.

### Repository conflicts and selected resolutions

| Repository reality | Checkpoint 3 design issue | Selected resolution |
| --- | --- | --- |
| Network code can send only `PlayerEvent`, while Checkpoint 1 accepts Bevy messages. | Direct presence writes from the Tokio task would bypass schedule ordering and authority. | Add two lifecycle `PlayerEvent` variants and a narrow `Update` bridge that writes the existing messages. |
| The packet loop retains a local session player ID, while stale/replaced connections are removed from `Clients`. | Trusting only the task-local value could let a removed connection enqueue a request during replacement. | Re-resolve the exact connection UUID in `Clients`, require matching/live client identity, and derive the player from that record. |
| `send_to_client` is player-broadcast and has no success result. | A per-player cache can miss a reconnect and a failed send can be cached accidentally. | Send and cache per live connection UUID; cache only successful `try_send` calls and purge stale UUIDs. |
| Global `GameTick` advances ten times per second. | Sending a status every update would create unnecessary packet traffic. | Ceil-round only pending time to whole seconds and compare complete snapshots; other meaningful eligibility/state changes still send immediately. |
| The completion system and delivery would otherwise be unordered peers. | A client could close after seeing `protected` before protection was authoritative. | Chain delivery after completion and reconnect rebase in `PostUpdate`. |
| Generic monolith UI cannot prove bound ownership. | Client-side sanctuary inference would weaken the contract. | Render the control in the Survival Thread only from server `in_own_sanctuary`, `can_request`, state, and reason fields. |
| Ordinary WebSocket close opens the existing failure/reconnect surface. | The protected confirmation close would look like a network outage. | Use a one-shot intentional-close guard; suppress only intentional close/error handling and preserve ordinary failures unchanged. |
| Initial mount silently reconnects an existing session. | Reloading to clear gameplay state could immediately wake the protected run. | Retain a session-scoped reconnect-suppression flag until explicit player login, while presenting one-time completion feedback. |

### Request, cancellation, and authentication boundary

The stable client commands are exactly:

```json
{"cmd":"request_safe_logout"}
{"cmd":"cancel_safe_logout"}
```

Neither command contains `player_id`; the decoder rejects either command if it
contains any additional field. The handler resolves the exact live connection
UUID, reads its authenticated `Client.player_id`, and enqueues only that
player's lifecycle event. Missing, removed, mismatched, closed, or poisoned
mappings fail safely at that boundary.

The player-system bridge consumes each lifecycle event once and writes the
existing internal message. Duplicate requests retain the original start tick
and do not create another transition, completion, or per-tick log. Cancellation
is idempotent outside `SafeLogoutPending`; in particular it cannot wake an
`OfflineProtected` run or affect another owner.

### Status packet and stable presentation values

The flat version-one packet is:

| Field | Wire meaning |
| --- | --- |
| `packet: "safe_logout_status"` | Stable response discriminator |
| `version` | Schema version, currently `1` |
| `state` | `online`, `pending`, `protected`, or `disconnected` |
| `can_request`, `can_cancel` | Server-authoritative available actions |
| `countdown_total_seconds`, `countdown_remaining_seconds` | Optional whole-second server countdown; protected completion carries remaining `0` |
| `reason` | Optional stable rejection/cancellation code |
| `message` | Concise server-authored player copy |
| `in_own_sanctuary` | Exact bound-sanctuary result |
| `active_assault` | Whether the personal crisis is committed and active |
| `protected` | True only for authoritative `OfflineProtected` |

Rejections map to `outside_sanctuary`, `sanctuary_invalid`,
`hostile_nearby`, `recent_combat`, `recent_damage`, `assault_active`,
`hero_invalid`, `hero_dead`, `true_death`, `run_invalid`, `already_pending`,
`already_protected`, or the fail-safe `unknown`. Cancellations map to `moved`,
`entered_combat`, `took_damage`, `hostile_nearby`, `left_sanctuary`,
`sanctuary_invalid`, `assault_started`, `hero_died`,
`disconnected_before_completion`, `manually_cancelled`, or `run_ended`.

### Snapshot delivery and countdown

One pure builder converts the current presence record and shared eligibility
result into the wire snapshot. It does not mutate presence, crisis, hero,
sanctuary, hostile, run, or client state.

Every new authenticated connection UUID receives its current snapshot. Later
delivery occurs only when the full semantic snapshot changes. Entering/leaving
the own sanctuary, eligibility cooldown expiry, hostile proximity, assault
state, request acceptance, cancellation, completion, True Death cleanup, and a
fresh run therefore send naturally. During a pending countdown, ceil-rounded
remaining seconds change no more than once per second. The final completion
packet always contains `state: "protected"`, `protected: true`, and
`countdown_remaining_seconds: 0`, after the authoritative transition.

### Client control, countdown, and intentional close

The compact Safe Logout section appears in the desktop Survival Thread from the
server snapshot, without inspecting local position or monolith state. An
eligible player sees the ten-second conditions and one Begin action. Pending
shows the latest server countdown, the instruction to remain still and avoid
combat, and one Cancel action. The client uses no completion timer: browser
background throttling cannot grant protection, cancellation immediately
replaces pending state, and only a protected server packet can complete the
flow. Local in-flight guards prevent repeated button activation while the
server response is pending.

Rejections and cancellations use the server message and stable reason as
fallback copy. Active assault explicitly says that safe logout is unavailable
and disconnecting will not stop the assault. State is communicated in text,
buttons have descriptive labels, the pending value is an atomic `aria-live`
status, disabled actions explain why, and the existing compact/wide Survival
Thread layouts remain shared. No flashing animation or new audio is added.

When the dispatcher receives `state: "protected"` with `protected: true`, it
first emits the complete status, records one completion message and the narrow
reconnect-suppression flag, and then performs one clean WebSocket close. The
ordinary gameplay logout endpoint is not called. Duplicate protected packets
cannot close or navigate twice. The intentional close suppresses the ordinary
server-offline/network-error surface, clears gameplay through the existing
page/title lifecycle, and shows the one-time confirmation. The suppression flag
remains until the player explicitly starts another login; ordinary failures do
not set it.

Safe-logout UI state clears on class/new-run selection, first login, True Death,
hero replacement, an intentional completion, and an authoritative fresh
snapshot. A normal protected-run reconnect receives the post-rebase `online`
snapshot and never restarts a countdown locally.

### Checkpoint 3 focused server coverage

The 20-test `safe_logout_checkpoint3_` group covers exact fieldless command
decoding, rejection of client-controlled extra fields, flat optional-field
serialization, all stable states and reasons, authenticated owner routing,
owner-scoped cancellation, unauthenticated/stale/closed mappings, the
`PlayerEvent` bridge, canonical eligibility presentation, per-connection
deduplication and isolation, failed-send retry, whole-second countdown values,
terminal protected ordering, cleanup/fresh-run clearing, manual cancellation,
and the four deterministic production-schedule scenarios below.

The retained 18-test Checkpoint 1 group covers eligibility, countdown,
cancellation, disconnect, death, connection replacement, and same-tick danger.
The 17-test Checkpoint 2 group covers protected input, crisis/environment
continuity, AI, damage, work, economy, long protection, and reconnect rebasing.
The personal-crisis and legacy-mode filters additionally prove that the default
director, introductory encounter, no-automatic-dusk rule, and legacy scheduled
horde remain unchanged.

### Checkpoint 3 client coverage

`safeLogoutStatus.test.ts` is a dependency-free pure-helper suite. It covers
the exact request/cancel shapes, absence of `player_id`, event constant,
eligible/ineligible/active-assault views, server-message precedence, pending
countdown, optional fields, stable reason fallbacks, request/cancel click guards,
cancellation clearing pending state, the rule that local countdown data cannot
grant protection, status-before-close ordering, the one-shot protected-close
guard, ordinary-failure behavior, new-login reset, one-time completion storage,
reconnect suppression, visibility policy, and state clearing. The existing
crisis helper suite was compiled and rerun alongside it.

The client repository has no configured TypeScript transform, DOM test
environment, Jest configuration, or test script. Therefore this checkpoint
does not claim browser-rendered component or real-WebSocket automation. The
actual dispatcher, `Network` close/error handlers, LoginControl lifecycle,
Survival Thread buttons, accessibility attributes, and compact/wide source were
inspected, then both production webpack bundles were compiled.

### Checkpoint 3 production-schedule scenarios

The scenarios combine protocol/authentication unit coverage with the same
`PlayerEvent` ingress, Bevy bridge, schedule, and packet-capture path used after
production decoding. They do not open a real TLS WebSocket.

* **Scenario A — successful flow:** a new connection receives an eligible
  `online` snapshot, an authenticated-ingress request produces exactly
  `pending 10, 9, 8, 7, 6, 5, 4, 3, 2, 1`, and the next update first commits
  `OfflineProtected` and then sends `protected 0`. Adjacent snapshots are
  distinct and pending deliveries are at least ten game ticks apart. Closing
  the simulated client preserves `OfflineProtected`; 25 more world ticks leave
  the protected hero snapshot unchanged. A later login receives one post-rebase
  `online` snapshot.
* **Scenario B — early disconnect:** disconnecting one third of the way through
  the countdown produces `Disconnected` with
  `disconnected_before_completion`, no protected run key, and no protected
  packet. A queued hostile spell then reduces hero HP, proving ordinary
  persistent-world simulation remains active.
* **Scenario C — cancellation:** movement produces one `online/moved` snapshot,
  no automatic retry, and an explicit valid re-request starts a new countdown.
  Manual cancellation produces `online/manually_cancelled`; a duplicate cancel
  changes neither record nor packets.
* **Scenario D — active assault:** the request remains `online` with
  `assault_active`, never emits protected state, and an ordinary disconnect
  preserves the same assault ID, spawn generation, unit identities, and HP.

### Checkpoint 3 validation record

Server commands were run from `sp_server/`; client commands were run from
`sp_frontend/sp_ts/` unless stated otherwise.

* `cargo fmt --check` — passed with exit status 0 and no output.
* `cargo check` — passed with exit status 0 and the repository's retained 70
  warnings.
* `cargo test --lib safe_logout_checkpoint3_ -- --nocapture` — passed: 20
  passed, 0 failed, 360 filtered out.
* `cargo test --lib safe_logout_checkpoint1_ -- --nocapture` — passed: 18
  passed, 0 failed, 362 filtered out.
* `cargo test --lib checkpoint2_ -- --nocapture` — passed: 17 passed, 0
  failed, 363 filtered out.
* `cargo test --lib personal_crisis -- --nocapture` — passed: 7 passed, 0
  failed, 373 filtered out.
* `cargo test --lib legacy_mode -- --nocapture` — passed: 2 passed, 0 failed,
  378 filtered out.
* `cargo test` — passed. The library target ran 380 tests, all passing; the day
  integration target ran 6 tests, all passing; binary/main targets ran 0 tests;
  and the one documentation test remained ignored. Total executed tests: 386
  passed, 0 failed, 1 ignored.
* `cargo clippy --all-targets --all-features` — passed with exit status 0 and
  warnings only. Clippy reported 1,330 warnings for the library and 1,345 for
  the library-test build, of which 1,330 were duplicates.
* `cargo run --bin headless_runner -- 1 6000` — passed with exit status 0. The
  bounded run ended at `MaxTicks`: 6,007 ticks, 2 days, 5 enemies, 0 deaths, 61
  HP, 590 skill XP, inventory count 18, 2 structures, `signs` crisis phase, 0
  launches, 0 resolutions, and 4 packets. Aggregate invariants were 0 panics, 0
  duplicate assaults, 0 automatic dusk waves, and 0 invalid crisis states.
  The runner wrote its ignored CSV and JSON reports.
* `npx tsc --module commonjs --target es2020 --esModuleInterop --skipLibCheck
  --outDir /tmp/sp-checkpoint3-client-tests src/sp/core/crisisStatus.ts
  src/sp/core/crisisStatus.test.ts src/sp/core/networkEvent.ts
  src/sp/core/safeLogoutStatus.ts src/sp/core/safeLogoutStatus.test.ts` — passed
  with exit status 0.
* `node /tmp/sp-checkpoint3-client-tests/crisisStatus.test.js` — passed and
  printed `crisisStatus helper checks passed`.
* `node /tmp/sp-checkpoint3-client-tests/safeLogoutStatus.test.js` — passed and
  printed `safeLogoutStatus helper checks passed`.
* `npx tsc --noEmit --skipLibCheck` — passed with exit status 0.
* `npx webpack --mode production --stats=errors-warnings` — passed with exit
  status 0. Desktop (3.35 MiB) and mobile (2.42 MiB) compiled; each retained
  three asset/entrypoint/code-splitting performance warnings.
* `npx tsc --noEmit` — executed but did not pass (exit status 2). The errors are
  the pre-existing collision between `src/phaser.d.ts` and
  `node_modules/phaser/types/phaser.d.ts`: `TS6200` duplicate definitions,
  `TS2432` merged-enum initializers, and `TS2688` for the missing local
  `./matter` type definition. The supported skip-library-check compile and both
  production bundles pass, so no Checkpoint 3 TypeScript error remains hidden.
* Jest was not run: packages are present, but the repository has no Jest config,
  TypeScript transform, DOM environment, or test script. A new framework was
  intentionally not introduced for this checkpoint.
* `git diff --check` (repository root) — passed with exit status 0 and no
  whitespace errors.
* `pgrep -fl webpack-dev-server` (repository root) — exited 1 with no output,
  confirming no webpack development server was left running. `npm run build`
  was not used because it starts `webpack-dev-server` after compiling.

### Known Checkpoint 3 limitations

* Presence and protection remain process-memory-only. There is no database
  schema, restart restoration, offline production, shop simulation, healing,
  or repair.
* The server caches a status after it is accepted by the connection's outbound
  channel; there is no client acknowledgment protocol. A lost completion packet
  never revokes server protection, and a new connection UUID receives a fresh
  snapshot, but delivery acknowledgment is not tracked.
* The network boundary validates the exact connection UUID, then the current
  lifecycle event and internal message carry only the derived player ID. A
  session-replacement race between validation and ECS handling is therefore not
  bound to the originating UUID. Static stale, removed, mismatched, and closed
  mappings fail safely; the full replacement-race/exploit matrix is Checkpoint
  4 work.
* Refresh-persistent reconnect suppression and the one-time completion copy use
  `sessionStorage`. If storage is unavailable, the mounted client still closes
  intentionally and returns to its landing surface through the in-memory event,
  but suppression and copy cannot survive a page refresh.
* Client tests are pure-helper and compile/build tests. They do not instantiate
  a browser DOM, a live `WebSocket`, or React component click/focus behavior.
* Headless protocol scenarios exercise production ECS ingress and serialized
  response capture, not a real HTTP session, TLS upgrade, or browser socket.
* Safe logout remains intentionally unavailable during `AssaultActive`; an
  ordinary disconnect remains dangerous and does not stop the assault.
* The player-facing control is desktop-only. Mobile-specific UI redesign was
  explicitly out of scope.
* Plain `npx tsc --noEmit` remains blocked by the retained Phaser declaration
  collision described above; `--skipLibCheck` and production webpack are green.

### Work deferred to Checkpoint 4

Checkpoint 4 retains reconnect and replacement-connection hardening, detailed
status/close/resume telemetry, real-browser and live-WebSocket lifecycle
coverage, the full disconnect/reconnect/exploit race matrix, broader
multi-player isolation validation, and final milestone sign-off. It must also
decide whether the originating connection UUID should remain bound through ECS
request handling.

Database persistence, server-restart restoration, offline production or shops,
passive healing or repair, active-assault safe logout, automatic or
inactivity-based protection, push notifications, guild/party permissions,
regional or new crisis families, larger maps, and world/cross-world scaling
remain outside this checkpoint and are not implied by Checkpoint 4.

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
* Reconciliation also unions existing presence keys, authoritative hero
  mappings, and assigned runs. A valid existing run with a missing record is
  initialized lazily from current `Clients` presence; an invalid run or missing
  authoritative hero removes any stale record.
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
  agree, requires live `Monolith.sanctuary_level` to match the zone level, then
  applies the existing strict full-zone boundary:
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
  watermark and currently includes the non-damaging Ward and Disengage
  abilities. Disengage therefore cancels immediately as combat before its
  delayed movement resolves; movement caused by an accepted action is also
  caught by the position comparison.
* The repository did not have a single authoritative incoming-damage watermark.
  Checkpoint 1 adds optional ECS component `LastDamageTick` and inserts or
  replaces it with deferred `try_insert` at positive combat, spell/ability, and
  world-damage sites. Reconciliation merges it into the requesting player's
  retained damage tick for eligibility and cancellation; it is not bundled on
  every object.
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
* The harness is extended in place with internal message writers and read-only
  presence inspection. No separate simulator is needed.

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
| `Clients` is shared with asynchronous socket tasks. | Boolean online samples can miss a disconnect followed by replacement authentication between samples. | Capture active request-time connection UUIDs, require at least one of those identities to remain live on every evaluation, and sample that continuity on both sides of the provisional exclusive ECS commit. A replacement socket cannot inherit a countdown. |
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
    // Crate-internal observation/continuity fields:
    last_observed_hp: Option<i32>,
    client_connected: bool,
    safe_logout_connection_ids: Vec<Uuid>,
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
* `OfflineProtected`: the explicit countdown completed for the recorded run
  identity. Checkpoint 2 gates that owner's simulation, inputs, targeting, and
  final mutation boundaries. It does not remove entities, pause global systems,
  or grant healing, repairs, production, or resources.
* `Disconnected`: no authoritative client exists and no completed protected
  handoff applies. It implies no protection.

The three final fields are crate-internal rather than public API. They retain
only the HP baseline, last observed connectivity, and request-time active
connection identities; authoritative hero/client state remains external. The
resource is per process and per run. It is neither reflected into the dynamic
scene nor written to the database.

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
`MissingHero` is retained as a defensive validator branch; in the normal
chained schedule, reconciliation first removes a missing-hero record, so the
subsequent request ordinarily reports `InvalidRun`.

A rejected request does not alter the current state, restart a pending timer,
modify crisis state, remove entities, grant rewards, or create protection.

## Countdown and idempotency

An accepted request:

1. changes `Online` to `SafeLogoutPending`;
2. stores the current `GameTick` and authoritative hero `Position`;
3. snapshots the active connection UUIDs that are allowed to carry the
   countdown;
4. clears the prior cancellation and rejection reasons; and
5. emits transition-only request/countdown logs.

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
* every connection identity present when the request was accepted disappears,
  even if a replacement socket has already authenticated;
* an internal manual cancellation is received; or
* the run is removed or replaced.

Cancellation re-samples authoritative client presence at its transition,
returns a connected player to `Online` and a disconnected player to
`Disconnected`, stores one typed reason, clears pending-only fields, and logs
the transition once. Run cleanup is the intentional exception: it logs typed
`RunEnded` and removes the entire record rather than retaining a reason. No
cancellation changes pressure, phase, assault identity, attackers, damage,
score, reward, inventory, or production.

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
* Completion still does not close the client. Disconnect after completion
  preserves `OfflineProtected`. Login or a disconnected-to-connected edge asks
  for an ordered exit; the reconnect update remains protected, then an exclusive
  `PostUpdate` barrier validates the exact run, rebases owner deadlines, and
  publishes `Online` for the following update.
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
4. evaluate pending cancellation before completion; and
5. validate, rebase, and resume a reconnecting protected run.

`PostUpdate` runs after the existing `Update` systems and their deferred command
application. This lets the evaluator observe authoritative movement, combat,
damage, sanctuary synchronization, personal-crisis launch, death markers, and
True Death cleanup from the update before considering completion.

Within pending evaluation, terminal safety conditions precede time completion:
disconnect/run loss, death/True Death, `AssaultActive`, damage, outgoing combat,
movement, sanctuary loss/invalidation, and hostile proximity are checked before
the elapsed countdown. Consequently same-update assault commitment, damage,
death/True Death, or disconnect wins over an otherwise due completion.

At the final completion boundary, the server samples continuity of the
request-time connection UUIDs immediately before and after a provisional
`OfflineProtected` write. The presence resource is exclusively borrowed, so no
ECS consumer can observe that provisional value. If either sample finds no
request-time connection still active, the write is rolled back, pending fields
are cleared, and only a typed `Disconnected` cancellation is logged; current
presence decides whether the resulting state is `Online` on a replacement
socket or `Disconnected`. Production assigns a fresh UUID to every socket, so
a replacement cannot inherit the pending handoff. The second successful
continuity sample is the selected completion boundary; a close after it is
ordered after completed safe logout.

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

The following commands were executed from `sp_server/` on 2026-07-13:

* `cargo fmt --check` — passed with exit status 0 and no output.
* `cargo check` — passed with exit status 0. The crate emitted its retained set
  of 73 warnings.
* `cargo test safe_logout_checkpoint1_ --lib -- --nocapture` — passed: 18
  passed, 0 failed, 326 filtered out. The crate emitted 73 warnings.
* `cargo test
  safe_logout_checkpoint1_deterministic_move_cancel_then_exact_completion
  --lib -- --nocapture` — passed: 1 passed, 0 failed, 343 filtered out.
* `cargo test
  safe_logout_checkpoint1_active_assault_cancels_then_disconnect_continues
  --lib -- --nocapture` — passed: 1 passed, 0 failed, 343 filtered out.
* `cargo test` — passed. The library target ran 344 tests, all passing; the
  integration target ran 6 tests, all passing; binary/main targets ran 0 tests;
  and the one documentation test remained ignored. Total executed tests: 350
  passed, 0 failed. The crate emitted 73 warnings.
* `cargo clippy --all-targets --all-features` — passed with exit status 0 and
  warnings only. Clippy reported 1,331 warnings for the library build (88
  duplicates) and 1,344 for the library test build (1,243 duplicates).
* `cargo run --bin headless_runner -- 1 6000` — passed with exit status 0. The
  bounded run ended at `MaxTicks`: 6,007 ticks, 2 days, 6 enemies, 0 deaths,
  62 HP, 310 skill XP, inventory count 16, 2 structures, `signs` crisis phase,
  0 crisis launches, 0 resolutions, and 3 packets. Aggregate invariants were 0
  panics, 0 duplicate assaults, 0 automatic dusk waves, and 0 invalid crisis
  states. The runner wrote its ignored `headless_runs.csv` and
  `headless_runs.json` reports.

One earlier attempt did not pass and is not counted as validation:

* `cargo test headless_smoke -- --nocapture` — failed before test execution
  because the incremental compiler cache could not be written:
  `No space left on device (os error 28)`. Generated
  `sp_server/target/debug/incremental` data was removed to recover workspace
  space; all successful commands above were run afterward.

## Historical Checkpoint 1 boundary

Checkpoint 1 intentionally reserved `OfflineProtected` without simulation
effects. That limitation is superseded by the Checkpoint 2 implementation
below. The remaining internal-only protocol and runtime-only persistence
limitations are not superseded.

## Checkpoint 2 architecture findings

### Ownership and protected-run identity

* Ordinary owned entities use `PlayerId` in ECS and `Ids.obj_player_map` for
  object-ID ownership. Human `PlayerId`s are authoritative player IDs.
* `Ids.player_hero_map` and `EntityObjMap` resolve the exact live hero.
  `AssignedStartLocations` distinguishes a current run from a recycled start
  slot. `BoundMonolith.id` is required because monoliths use a shared monolith
  faction instead of the owning human `PlayerId`.
* `RunSpawnedObjs` is the existing per-run attribution for introduction,
  merchant, legendary, sanctuary, and other neutral-faction objects. Delayed
  `SpawnNPC` events previously had no run attribution, so Checkpoint 2 adds an
  optional `run_owner` and supplies it at the existing player-run spawn sites.
* `ProtectedRunKey` records `player_id`, authoritative `hero_id`, assigned
  start-location name, bound-monolith ID, and a sorted/deduplicated snapshot of
  the run's existing `RunSpawnedObjs` IDs at completion. Every protected update
  validates this exact runtime identity before any gameplay system runs. A
  dead/missing hero, recycled run, changed binding, or changed attributed-run
  object set revokes stale protection instead of freezing a replacement run.
* The canonical helpers are `is_player_offline_protected`,
  `is_owner_offline_protected`, `object_belongs_to_protected_run`, and
  `entity_belongs_to_protected_run`. Systems use the narrowest helper supported
  by their existing data; ownership is never inferred from proximity, current
  target, start coordinates, inventory contents, or another player's action.

### System and timer audit

| System category | Protected-owner behaviour | Gate/rebase method | Focused coverage | Remaining risk |
| --- | --- | --- | --- | --- |
| Presence completion, disconnect, reconnect | Completion captures the exact run; later disconnect preserves protection; reconnect stays protected for one full update and resumes only after rebase | `First` integrity validation plus chained exclusive `PostUpdate`; rebase required | Checkpoint 1 lifecycle tests; long-protection and corrupt-active-assault tests | Runtime state is not restored after server restart |
| Personal crisis before `AssaultActive` | Pressure, online-active time, phase, warnings, launch grace, notices, resolution, and launch are frozen | Player-level early returns; `phase_started_tick` and `last_evaluated_tick` rebased | Long-protection snapshot; short personal-crisis regression | Future crisis fields with absolute deadlines must join the rebase inventory |
| Committed personal assault | Safe logout is still rejected; corrupt protection is revoked in `First`; an ordinary disconnected assault continues unchanged | Integrity fail-safe, not a freeze | Corrupt-state recovery and existing active-disconnect scenario | Production recovery policy for a genuinely corrupt state remains log-based |
| Introduction, initial encounter, legacy wolf/goblin/undead/pillager/nightly, legendary | The protected owner's timelines and run-specific spawns pause; no rewards or transitions occur. Unrelated players and the selected global director continue | Player/owner gates; intro, encounter, legendary, run-score, and owned event ticks rebased | Long-protection plus existing introduction, legacy, personal-director, and full suites | A pre-existing neutral NPC with no `PlayerId`, `RunSpawnedObjs`, crisis attribution, or `run_owner` cannot be generically assigned to a run |
| Hero needs, recovery, automatic consumption/sleep | Hunger, thirst, tiredness, heat, stamina, mana, HP, auto-consumption, sleep, warnings, and need consequences do not change | Owner gates at incremental systems and event handlers; need/effect deadlines rebased | Exact hero snapshot across 10,000 ticks | No offline healing or resource consumption is intentionally performed |
| Effects, damage-over-time, weather effects, death | Existing effects remain installed but do not tick, expire, damage, heal, or improve the protected hero. True Death and final run cleanup remain authoritative | Owner/target gates at effect, burning, weather, spell, item, and death boundaries; absolute status ticks rebased | Hero/effect snapshot, queued hostile damage, active-assault and cleanup regressions | New damage producers must use the same final target gate |
| Villager needs and BigBrain AI | Scores, requested action state, current action, active task, movement, needs, inventory, gathering, hauling, crafting, farming, rest, dialogue, and combat remain unchanged | Every registered owner-scoped scorer/action/support system no-ops; existing state is retained; owned deferred deadlines rebased | Protected villager state test and long-protection snapshot; full 89-test villager suite | BigBrain framework internals are not globally paused; all repository-owned villager mutations are gated |
| Structures, building, upgrading, repair, fuel, work queues | HP, state, build/upgrade progress, fuel, work entries, assigned work, storage, and output remain unchanged | Actor and target checks at observers and event handlers; build/item/event deadlines rebased | Structure/work/resource snapshots and existing build/craft/refine tests | The dormant combined `gather_farm_refine_craft_system` remains registered behaviorally as before; active production paths are separately guarded |
| Crops and farming | Stage, stage start/end, readiness, tending, harvest, inputs, and outputs remain unchanged | `crop_system` skips protected structures; stage ticks rebased | Protected-versus-unprotected crop test and long-protection snapshot | No offline growth or catch-up is granted |
| Inventory, transfers, trade, hiring, assignment, deletion | Protected sources and protected targets reject mutating commands; read-only inspection remains available and side-effect-free | Central `PlayerEvent` classifier/guard plus handler-level fail-closed checks | Classifier/guard tests, protected input scenario, existing economy suite | Future `PlayerEvent` variants must be classified explicitly |
| Tax collector and merchant timers | Collection, demand, forfeiture, movement/actions, and merchant/run timers pause without changing inventory | Target-player gates; tax/merchant and owned event deadlines rebased | Protected tax/forfeiture test and long-protection resources | No offline shop behavior is added |
| Hostile target selection and actions | Protected targets are invalidated and filtered from targeting, fortification redirects, path blockers, corpse/spoil/steal/torch selection, melee, casting, and raise-dead actions; the hostile remains and does not retarget another player merely because its prior target became protected | Candidate filters, target-specific invalidation retained while that exact target remains protected, action installer checks, and final action checks | Protected-target invalidation test and queued hostile scenario | Neutral actors without attribution still need target-level protection, which current final checks provide |
| Deferred `MapEvents` and `GameEvents` | Unsafe hostile or cross-owner events already aimed at the run are removed at entry or rejected at execution; owner work events remain queued and are rebased | Selective entry purge, event ownership classification, handler gates, resume rebase | Queued spell-damage scenario and long-protection work snapshots | New event variants must declare actor, target, and deadline semantics |
| Objectives, score, victory, discovery, merchants | The protected run cannot advance objectives, score, survival day, victory, investigation, or merchant progression | Player/owner gates; run score and event deadlines rebased | Long-protection crisis/resource assertions and full regression suite | Presentation-only login packets are intentionally not rebased |
| Global day/night, visibility, weather, world packets | Continue for the shared world; connected protected clients still receive normal environmental packets | Deliberately ungated global systems | First-light visibility/packet and weather-cycle test | A disconnected client naturally receives no packets |

The audit also inspected direct attacks, abilities, combos, villager fight-back,
NPC attacks and spells, structure/event damage, burning and survival damage,
item effects, farming, fishing, gathering, refining, crafting, queues, fuel,
trade, merchants, hiring, assignments, objectives, victory, True Death, the
headless bot, and the multi-run headless runner. The repository has no generic
passive structure repair or health-regeneration system beyond the concrete
sleep/effect paths listed above; no new economy or production abstraction was
introduced.

### Files affected by Checkpoint 2

* `sp_server/src/safe_logout.rs` — protected-run key, canonical helpers,
  integrity checks, disconnect preservation, entry event purge, ordered resume,
  timestamp rebasing, and protection telemetry.
* `sp_server/src/player.rs` — ordered input collection/guard/handling,
  exhaustive event classification, cross-owner target rejection, and final
  player/economy handler gates.
* `sp_server/src/game.rs` — crisis, introduction, legacy threat, effects,
  damage, needs, weather-owner effects, structure/work, events, objectives,
  merchants, score, victory, and cleanup gates.
* `sp_server/src/ai/npc/npc.rs` — protected-target invalidation, candidate
  filtering, no-fallback cycle, and final hostile action gates.
* `sp_server/src/ai/villager/villager.rs` — owner gates for registered villager
  scorers, actions, support systems, movement, work, needs, and combat.
* `sp_server/src/ai/tax_collector/tax_collector.rs` — protected target gates for
  collection, scorers, actions, demand, and forfeiture.
* `sp_server/src/farm.rs` — protected crop progression gate and focused test.
* `sp_server/src/event.rs` — optional run ownership on delayed NPC spawns.
* `sp_server/src/player_setup.rs` — run attribution for delayed per-player NPC
  spawns.
* `sp_server/src/headless.rs` — protected-state snapshots, activation,
  disconnect/reconnect, hostile-event/input helpers, long-duration scenarios,
  and environmental continuity coverage.
* `docs/safe_logout_milestone.md` — this implementation record.

`sp_server/src/world.rs`, `network.rs`, `combat.rs`, `obj.rs`, `item.rs`,
`structure.rs`, `resource.rs`, `recipe.rs`, `trade.rs`, `villager_util.rs`,
`headless_bot.rs`, and `src/bin/headless_runner.rs` were inspected and did not
require Checkpoint 2 semantic changes. In particular, there is no new network
packet, client command, database field, map change, resource, recipe,
profession, currency, or production chain.

### Repository conflicts and selected resolutions

| Repository reality | Design conflict | Checkpoint 2 resolution |
| --- | --- | --- |
| Global `GameTick` drives both shared environment and owner deadlines. | Pausing it would stop the world; skipping absolute comparisons would cause catch-up. | Keep global time unchanged. Skip incremental owner systems and rebase only inventoried owner deadlines on validated resume. |
| A completed internal logout can remain connected because Checkpoint 3 has no disconnect protocol. | A login cannot be the sole protection-exit signal in headless/internal use. | Preserve protection on disconnect; use Login or a disconnected-to-connected edge to request the same ordered resume barrier. |
| Player systems previously consumed one shared event map without semantic ordering. | A mutating handler could run before a protection guard. | Add chained `Collect -> ProtectionGuard -> Handle` system sets and retain final handler checks. |
| Damage and work are distributed across direct systems, observers, and queued events. | A target-selection-only check cannot stop stale or same-tick actions. | Gate selection, event queues, handler/observer execution, and the final mutation call sites. |
| Monolith ownership is a hero binding, not its shared ECS faction. | `PlayerId` alone misses the sanctuary asset. | Carry the bound-monolith ID in `ProtectedRunKey` and include it in object/entity target helpers. |
| Delayed `SpawnNPC` carried no owning run. | Intro/follow-up spawns could mature during protection without ownership. | Add optional `run_owner` only to this existing internal event and populate it at player-run scheduling sites. |
| BigBrain owns thinker lifecycle, while gameplay actions are repository systems. | Removing thinkers would destroy state and create reconnect churn. | Leave thinkers/entities installed and no-op every owner-scoped scorer/action/support mutation. |
| NPCs can redirect to walls, corpses, storage, or another visible player. | Dropping only the current hero target could damage another protected asset or retarget another player. | Filter every target kind and retain a target-specific invalidation marker until the exact former target is no longer protected; only then may ordinary scoring resume. |
| Active assaults are intentionally committed world state. | Freezing one behind a corrupt protected record violates the personal-crisis contract. | `First` revokes invalid protection without despawning, resolving, rewarding, or changing assault identity/units. |

## Checkpoint 2 enforcement design

### Entry, disconnect, and input behavior

At successful pending-to-protected completion the server captures the exact run
key and `protected_since_tick`, then selectively removes unsafe queued hostile
or cross-owner mutations aimed at that run. It does not clear global queues,
despawn enemies, remove the settlement, clear conditions, complete work, or
award progress.

After that boundary, losing the final client leaves the record
`OfflineProtected`; repeated disconnect reconciliation is idempotent. An
ordinary `Online -> Disconnected` transition still grants no protection, and a
pending disconnect still cancels the countdown.

Incoming `PlayerEvent`s are collected, checked, and handled in explicit system
sets. Mutating input from the protected player is removed. Mutating input from
another player is also removed when any classified source or target belongs to
the protected run. Read-only queries and lifecycle `NewPlayer`/`Login` messages
remain routable, but read-only handlers must not perform hidden merchant,
monolith, quest, inventory, or discovery mutation.

### Crisis, hero, villagers, and economy

All pre-active personal-crisis evaluation returns before pressure, online time,
phase, warning, grace, launch, reset, or notice changes. Introduction and legacy
owner-scoped timelines follow the same rule. `AssaultActive` is never a valid
protected state: normal requests reject it and the `First` integrity barrier
recovers a corrupt record while leaving the assault intact.

Hero needs, regeneration, automatic consumption/sleep, status consequences,
effects, damage, death countdowns, weather-applied owner effects, and event
actions skip protected entities without clearing their state. Villager BigBrain
systems preserve scores, action states, active tasks, events, needs, inventories,
and positions. Structure, work, queue, crop, fuel, resource, storage, tax,
merchant, crafting, refining, farming, fishing, building, upgrade, repair, and
assignment paths similarly return before mutation. The result is a freeze, not
offline production, repair, consumption, or healing.

### Targeting, damage, and queued actions

Hostile target state aimed at a protected object is invalidated before scoring.
The invalidation removes only transient target/movement/casting intent and
queued hostile actions; it preserves the hostile entity, HP, loot, ownership,
attribution, and kill credit. A target-specific invalidation marker remains
while the exact former target is protected, preventing target loss from
selecting another player as a fallback merely because protection began.

Candidate filters cover the hero, villagers, structures, fortification
redirects, storage/spoil/steal/torch targets, corpses, scripted targets, and path
blockers. Final attack, spell, spoil, steal, torch, item/effect, observer, and
event boundaries independently reject a protected target. Already-queued
hostile destructive events are removed selectively at entry or resume and are
also harmless if they reach a gated handler. Other players' events are retained.

### Incremental freeze and absolute-deadline rebase

Incremental systems simply skip protected owners, so no value changes and no
catch-up counter accumulates. Absolute timers cannot merely be skipped because
the shared `GameTick` continues. On validated reconnect, the exclusive resume
barrier adds exactly the protected duration to:

* player introduction and initial-encounter deadlines;
* crisis phase start and last-evaluated ticks;
* run-score start tick;
* legendary active, defeated, and follower deadlines;
* crop stage start/end ticks;
* owned `MapEvent` and `GameEvent` run/start ticks;
* structure build/upgrade start times and inventory-item work start times;
* campfire, dehydration, starvation, exhaustion, missing-food/drink, and idle
  timestamps;
* combat, damage, attacker, death, and burning timestamps;
* tax collection/demand and collector idle timestamps; and
* presence-level combat and damage watermarks.

Saturating arithmetic prevents overflow. Dead crops keep their terminal
sentinel. Login/presentation events are not owner simulation deadlines. Rebase
applies only when the current run key exactly matches the recorded key; no
timestamp is moved for a deleted or replacement run.

### Ordering, reconnect, and cleanup

The `First` integrity system runs before gameplay, ensuring stale protection or
an impossible active-assault/protected combination cannot freeze a run. Normal
gameplay then observes the protected state for the entire `Update`. The chained
safe-logout reconciliation, request, cancellation, completion, and exclusive
resume systems run in `PostUpdate` after deferred commands.

Login or a new connected edge sets `protection_exit_requested` but does not
publish `Online`. The exclusive resume system validates the run, calculates
`GameTick - protected_since_tick`, rebases every inventoried deadline, and only
then changes the state to `Online` for the next update. Therefore reconnect
cannot launch an assault, expire an effect, complete work, grow a crop, consume
fuel, or process a protected-duration backlog in the reconnect update.

True Death, missing/dead authoritative hero state, and final run cleanup still
win. Cleanup removes only that player's presence record and the existing run
state. A fresh run gets a new unprotected record and cannot inherit the old run
key or timer rebase.

## Checkpoint 2 focused and headless coverage

The 17-test focused `checkpoint2_` group covers event classification,
protected source and cross-owner target rejection, canonical neutral-run and
bound-monolith ownership, protected crop advancement versus an unprotected
crop, tax collection/forfeiture suspension, protected villager and run-NPC
state, target invalidation without forced fallback, global environment
continuity, invalid active-assault recovery, stale-run fail-open handling,
queued hostile damage, protected connected input, bound-monolith item expiry,
reconnect launch ordering, and the long protected interval.

Scenario A protects a populated owner for 10,000 ticks and compares exact hero,
villager, structure, active construction, real Firewood craft, real structure
refine, queue/work, crop, resource, introduction, legendary, and crisis
snapshots. A connected neighboring player simultaneously accumulates needs,
completes a real craft, and advances its personal-crisis clock. The scenario
also proves global time advances, reconnect rebases each installed absolute
deadline by exactly the protected duration, the reconnect update performs no
catch-up, and owner burning, crafting, refining, and construction resume only
afterward.

Scenario B queues hostile spell damage before and after the protection boundary
against the hero, a villager, and a structure, then queues theft, spoilage, and
torching against the structure. The pre-entry event is purged and post-entry
events are rejected at execution; every target and the run score remain
unchanged while the hostile remains alive.

Scenario C is the retained Checkpoint 1 active-assault test: ordinary disconnect
ends in `Disconnected`, does not create protection, and the same committed
assault identity and units remain in world simulation.

## Checkpoint 2 validation record

All commands below were run from `sp_server/` unless stated otherwise.

* `cargo fmt --check` — passed with exit status 0 and no output.
* `cargo check` — passed with exit status 0; the crate emitted its retained set
  of 70 warnings.
* `cargo test --lib checkpoint2_ -- --nocapture` — passed: 17 passed, 0
  failed, 343 filtered out. This includes the 10,000-tick owner freeze,
  connected-neighbor isolation, reconnect rebase, environment, queued damage,
  target invalidation, AI, crop, tax, input, stale-key, and bound-monolith
  cases.
* `cargo test --lib safe_logout_checkpoint1_ -- --nocapture` — passed: 18
  passed, 0 failed, 342 filtered out.
* `cargo test --lib personal_crisis -- --nocapture` — passed: 7 passed, 0
  failed, 353 filtered out.
* `cargo test --lib
  checkpoint3_connected_helper_resolves_offline_owner_assault_once --
  --nocapture` — passed: 1 passed, 0 failed, 359 filtered out.
* `cargo test --lib
  checkpoint3_legacy_mode_does_not_run_the_personal_assault_lifecycle --
  --nocapture` — passed: 1 passed, 0 failed, 359 filtered out.
* `cargo test` — passed. The library target ran 360 tests, all passing; the
  integration target ran 6 tests, all passing; binary/main targets ran 0 tests;
  and the one documentation test remained ignored. Total executed tests: 366
  passed, 0 failed, 1 ignored. The crate emitted 70 warnings.
* `cargo clippy --all-targets --all-features` — passed with exit status 0 and
  warnings only. Clippy reported 1,330 warnings for the library build and 1,345
  for the library test build, of which 1,330 were duplicates.
* `cargo run --bin headless_runner -- 1 6000` — passed with exit status 0. The
  bounded run ended at `MaxTicks`: 6,007 ticks, 2 days, 6 enemies, 0 deaths, 59
  HP, 1,110 skill XP, inventory count 19, 2 structures, `signs` crisis phase, 0
  launches, 0 resolutions, and 3 packets. Aggregate invariants were 0 panics,
  0 duplicate assaults, 0 automatic dusk waves, and 0 invalid crisis states.
  The runner wrote its ignored `headless_runs.csv` and `headless_runs.json`
  reports.
* `git diff --check` (repository root) — passed with exit status 0 and no
  whitespace errors.

One intermediate full-suite run is not counted as passing validation. It ran
359 tests successfully but failed
`game::tests::stamina_recovery_increases_stamina_every_second` because the
new protection query initially required identity components that this isolated
legacy fixture intentionally omits. The query was made identity-optional while
retaining canonical run checks whenever identity exists; the focused test and
the complete 360-test library rerun then passed.

## Historical Checkpoint 2 limitations

The following list records the boundary at the end of Checkpoint 2. Its first
item is superseded by the Checkpoint 3 implementation above; the remaining
simulation and persistence boundaries still apply.

* At the Checkpoint 2 boundary there was no production request/response packet,
  client button, countdown, cancellation display, wake-up UI, or automatic
  socket close. Checkpoint 3 now supplies those production surfaces. Push
  notification remains out of scope.
* Presence, run identity, and protected duration are process-memory-only. There
  is no database schema, restart persistence, or restoration after server
  restart.
* A neutral pre-existing NPC that has neither human `PlayerId`, crisis
  attribution, `RunSpawnedObjs`, nor the new delayed-spawn `run_owner` cannot be
  generically assigned to one protected run. Current target and final action
  gates still prevent it from mutating protected assets.
* The repository has no single universal combat/damage API or faction service.
  Current production mutation sites are gated; future event variants and damage
  call sites must explicitly use the canonical helper.
* Disconnected clients naturally receive no day/night, weather, visibility, or
  world-time packets even though those global systems continue.
* Protection remains intentionally invalid during `AssaultActive`. Ordinary
  disconnect during an active assault remains dangerous and unchanged.
* Checkpoint 2 does not add offline gains of any kind and does not rebalance the
  personal or legacy crisis directors.

## Historical work deferred from Checkpoint 2 to Checkpoint 3

Checkpoint 3 has now added the production network request/response contract and
client experience described at this former boundary: an explicit safe-logout
control, eligibility/countdown presentation, typed rejection/cancellation
feedback, completed-state feedback, and normal reconnect presentation. Those
surfaces route into the existing authoritative internal messages without
weakening Checkpoint 1 eligibility or Checkpoint 2 simulation gates.

The current Checkpoint 4 and out-of-scope boundaries are recorded in the
Checkpoint 3 section above.
