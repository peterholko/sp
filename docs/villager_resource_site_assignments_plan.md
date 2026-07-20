# Villager Resource-Site Assignments

Status: implementation plan

Branch: `codex/villager-resource-assignments`

## Outcome

Replace the hero-position-bound villager gathering interaction with an
authoritative, map-first resource-site assignment flow:

1. The player clicks a known resource-site badge anywhere on the map.
2. The player chooses **Assign gatherer** (or **Reassign**).
3. The player selects a villager from an idle-first picker.
4. That villager travels to the site, gathers, unloads, handles personal needs,
   and returns to the same standing job without requiring the hero to visit the
   tile.

Form A, the map as the board, is the first deliverable. Form B, a consolidated
site panel, is a later convenience view over the same server snapshot. This
milestone does not add painting, rectangle selection, resource depletion, new
professions, or a second gathering engine.

## Recommendation

Build the feature around a sparse, player-owned standing-site registry and keep
the existing `Order::Gather` as its execution projection. Add a dedicated
Phaser site-badge layer and a server-fed villager picker. Preserve the existing
`order_gather` path during migration, but route both old and new commands
through one idempotent work-assignment transition.

The first implementation should treat "matching job re-pulls work" as the
explicitly assigned villager resuming its persistent gather order after needs,
combat, hauling, or tool-fetch interruptions. There is no villager profession
or labor-preference component in the repository today, so automatic claiming
of unstaffed standing sites must not be inferred from skill level. That is a
separate follow-up if it is still wanted after direct assignment ships.

## Repository reality

The proposed interaction fits the existing game, but several implementation
assumptions need correcting.

| Area | Current implementation | Consequence for this plan |
|---|---|---|
| Renderer | React UI over Phaser 3, not Pixi | Use a Phaser container and hex graphics/sprites for badges and tints. |
| Resource sites | `Resources` is a world resource keyed by `Position`, then resource name; deposits are not ECS entities | Do not spawn synthetic site entities. Use a stable logical key. |
| Resources per tile | A tile can contain multiple resource names and types | Key a site by tile and exact `res_type`; show a resource chooser or stacked badge when needed. |
| Discovery | `SurveyHistory` is per player, while prospecting sets a global `Resource.reveal` flag | Define site eligibility explicitly and never send a global all-resource snapshot. |
| Current command | `order_gather` sends only villager ID and resource type | The server currently copies the hero's position into the order, which creates the hero-travel requirement. |
| Gather execution | `Order::Gather` already stores `res_type` and `pos` | The worker AI already knows how to travel to a remote coordinate. |
| Standing behavior | `Order::Gather` remains installed after each gather | The AI already repeats gathering and returns after needs or unloading. |
| Storage | The AI dynamically finds owned storage when capacity pressure wins | The cached `storage_id` and `storage_pos` fields in `Order::Gather` are legacy data and should not drive the new model. |
| Resource lifetime | Gathering reads `Resources` immutably and never decrements `quantity` | There is no honest depleted/regenerating state to expose in this milestone. |
| Existing picker | Structure assignment has a server-driven candidate flow, but desktop is a carousel and the gather popup is a hard-coded icon strip | Reuse panel lifecycle, portraits, buttons, and network patterns; add new rich row content. |
| Remote targeting | Every rendered terrain tile is already interactive | No new general targeting primitive is required, although the site badge needs its own click event to win over objects on the same tile. |
| Persistence | Reconnect keeps ECS state in the running process; full-run process-restart persistence is incomplete | Guarantee ordinary reconnect and Offline Protection behavior. Do not claim durable restart persistence in the first checkpoint. |

Relevant existing paths are:

- `sp_server/src/resource.rs`: resource storage, reveal state, validation, and
  resource-to-skill mapping.
- `sp_server/src/obj.rs`: `Order::Gather`, `Assignment`, and `Assignments`.
- `sp_server/src/player.rs`: current `order_gather_system`, tile information,
  structure assignment, event classification, and protection guards.
- `sp_server/src/ai/villager/villager.rs`: travel, tool fetch, gathering,
  storage selection, unloading, needs, blocked work, and activity reporting.
- `sp_server/src/encounter.rs` and `sp_server/src/player_setup.rs`: the real
  rescued and hired villager Thinker construction paths.
- `sp_server/src/game.rs`: gather completion, login synchronization, object
  cleanup, True Death, and best-effort scene snapshots.
- `sp_frontend/sp_ts/src/sp/core/scenes/mapScene.ts`: interactive tiles and the
  current temporary nearby-resource container.
- `sp_frontend/sp_ts/src/sp/desktop/ui.tsx` and `sp_frontend/sp_ts/src/sp/mobile/ui.tsx`:
  panel state and event wiring.
- The desktop/mobile `tilePanel.tsx`, `targetActionPanel.tsx`, `assignPanel.tsx`,
  and `gatherPanel.tsx` files: current tile, action, candidate, and resource-type
  interaction precedents.

## Product and data contract

### Site identity

A gatherable site is a player-scoped logical key:

```text
(player_id, x, y, res_type)
```

`res_type`, rather than a resource-name or invented generic resource, matches
the existing `Order::Gather` and `GatherEvent` behavior. If a tile contains two
different resource types, it produces two site rows. Multiple names of the same
type remain one site because one gather event already processes them together.

The client may derive an opaque display key such as `x:y:res_type`, but mutation
packets should carry typed `x`, `y`, and `res_type` fields and the server must
validate all three.

### Eligibility

For the initial implementation, an eligible site should satisfy all of these:

1. The coordinate is in bounds and passable.
2. The requesting player's `SurveyHistory` contains the tile.
3. The tile currently contains at least one revealed resource of the exact
   `res_type`.
4. The resource type is supported by the existing gathering and skill mapping.

This literal interpretation of "surveyed resource site" is player-scoped and
does not leak every resource globally revealed by another player. It also
exposes a current terminology conflict: **Survey** records a tile, while
**Prospect** reveals its resources. The first checkpoint should use clear UI
copy such as "Survey and prospect this tile before assigning a gatherer" and
leave discovery-system consolidation to a separate change. Silently making a
global revealed resource assignable to everyone would be a privacy regression.

### Canonical standing-site state

Add a sparse resource along these lines; exact Rust names can be chosen during
implementation:

```rust
struct GatherSiteKey {
    pos: Position,
    res_type: String,
}

struct GatherSiteRecord {
    standing: bool,
    assigned_villager_id: Option<i32>,
}

struct StandingGatherSites {
    by_player: HashMap<i32, HashMap<GatherSiteKey, GatherSiteRecord>>,
    revision_by_player: HashMap<i32, u64>,
}
```

Rules:

- One exact site has at most one worker for one player.
- One worker has at most one primary work assignment.
- Assigning a worker directly also sets the site to standing.
- Unassigning a worker leaves the site standing and visible as unstaffed.
- Disabling standing clears its worker, cancels that gather order, and removes
  the sparse record; the eligible site remains visible as available.
- Assigning a worker already bound to another site atomically leaves the old
  site standing but unassigned.
- Assigning a structure worker to a site, or a site worker to a structure,
  atomically clears the old forward and reverse assignment state.
- Repeating the same mutation is an idempotent no-op apart from returning the
  current canonical state.
- Two players may designate the same physical resource tile independently.
  The current resource supply is shared and infinite, so this adds no new
  ownership or reservation mechanic.

`StandingGatherSites` is player intent and the source for snapshots, reconnect,
cleanup, and the optional board. `Order::Gather` remains the AI execution
projection. A reconciliation system should fail safe: if a worker disappears,
dies, changes owner, or receives another primary assignment outside the normal
helper, clear the worker reference and leave the site standing/unassigned. It
must not fight another order by reinstalling `Order::Gather` every tick.

### Stable visual states

The first map language has four server-derived states:

| State | Meaning | Suggested treatment |
|---|---|---|
| `available` | Eligible surveyed/prospected site, not standing | Subtle neutral badge/tint |
| `standing_unassigned` | Standing site waiting for a worker | Dashed border plus assignment badge |
| `assigned` | A living owned villager is bound to the site | Solid border plus portrait/worker marker |
| `blocked` | The assigned worker has a stable work blocker | Amber/red warning marker with a readable reason |

Ordinary eating, drinking, sleeping, hauling, combat interruption, or travel
does not make the site blocked; it remains assigned and the picker shows the
worker's current activity. A missing/dead worker is reconciled to
`standing_unassigned`. Do not add a depleted state until quantity is actually
consumed and regenerated by the resource system.

## Interaction design

### Form A: map as the board

1. On login/reconnect and when the resource-work layer opens, the client obtains
   a full player-scoped site snapshot.
2. The client stores it separately from terrain `TileState`, keyed by the stable
   site key and guarded by a monotonically increasing revision.
3. `MapScene` renders a dedicated `resourceSites` Phaser container above terrain
   and below units/selection. It must not reuse the temporary nearby-resource
   container, which collapses a tile to its best resource and is cleared when
   toggled off.
4. A badge is interactive and emits `RESOURCE_SITE_CLICK` with the exact site
   key. This bypasses the current "select the last object on the tile" behavior
   when a unit or structure occupies the same hex.
5. The contextual strip shows **Assign gatherer**, **Reassign**, **Unassign**, and
   the standing toggle as appropriate for the canonical state.
6. Opening the picker requests a fresh server candidate list for that exact
   site. A row click commits immediately; no extra OK click is required.
7. The UI disables duplicate submission and changes the badge only after an
   authoritative response/delta. Existing Notice/Error presentation handles
   rejection.

The desired common path is therefore:

```text
site badge -> Assign gatherer -> villager row
```

When a tile has multiple resource types, the badge click first opens a compact
resource list or the site panel with one row per type. The common one-resource
tile still follows the three-click path.

Because map, object, and weather visuals use separate Phaser scenes, keep the
badge in an unobscured hex corner. A later "Locate" action for Form B must center
all scene cameras together rather than calling only `MapScene.centerOn`.

### Candidate picker

The server returns facts and eligibility; the client renders and sorts them.
Each row includes:

- portrait, name, and villager ID;
- relevant existing skill and level via `Resource::type_to_skill`;
- current activity/order and current structure or site assignment;
- hex/path distance to the site;
- selected/current-worker state; and
- an eligibility flag plus a readable disabled reason.

The deterministic sort is:

1. current worker;
2. eligible idle/unassigned villagers;
3. eligible busy villagers who will be reassigned;
4. ineligible villagers;
5. within each group, relevant skill descending, distance ascending, then ID.

The server should perform the authoritative reachability check with the real
pathfinder. The client can use its existing hex-distance helper for immediate
display, but it must not decide that a command is legal.

Desktop can reuse `HalfPanel`, existing portrait rules, and action buttons.
Mobile should use `MobilePanelScreen`/`MobilePanelLayout`. Shared packet types,
site reducers, revision handling, state-to-badge mapping, and sorting belong in
`sp/core`; the existing duplicated desktop/mobile UI trees do not justify a
broad UI refactor in this milestone.

### Form B: panel later

Form B uses the same snapshot, candidate request, and mutation commands. It adds
no server-side gameplay model.

Each row contains resource/type, coordinates, canonical state, assigned worker
or **Unassigned**, and one Assign/Reassign action. Sort standing-unassigned
first, then blocked, then assigned, then merely available sites. Add Form B only
after Form A testing shows that finding known sites by panning is a real cost.

## Server and protocol design

### Additive commands

Keep `order_gather` for the current client, headless bot, and agent tooling while
the new flow rolls out. Add authenticated commands such as:

```json
{"cmd":"info_gather_sites"}
{"cmd":"info_gather_site_candidates","x":12,"y":8,"res_type":"Log"}
{"cmd":"set_gather_site","x":12,"y":8,"res_type":"Log","worker_id":44}
{"cmd":"set_gather_site","x":12,"y":8,"res_type":"Log"}
{"cmd":"unassign_gather_site","x":12,"y":8,"res_type":"Log"}
{"cmd":"remove_gather_site","x":12,"y":8,"res_type":"Log"}
```

The authenticated connection supplies `player_id`; never accept it from the
payload. The worker-less `set_gather_site` creates a standing-unassigned site.

Do not overload structure-only `info_assign`, `assign`, or `remove_assign`.
Those packets are keyed by `structure_id` and their candidate DTO lacks the
resource skill, activity, distance, eligibility, and site state required here.

### Responses

Add:

- a versioned, revisioned `gather_sites` full snapshot containing every eligible
  site joined with its standing record and worker-derived status;
- a `gather_site_candidates` response for one exact site;
- a canonical `gather_site_changed` delta, including the previous site when a
  worker moves between sites; and
- an optional compact site summary in `InfoTile` so an open tile panel refreshes
  immediately after a mutation.

Snapshots must be player-scoped. Send a full snapshot as part of the existing
delayed core login/reconnect bundle and on explicit query. Send deltas only on
mutations and relevant `Changed<Order>`, `Changed<BlockedWork>`, worker removal,
or ownership changes; do not poll, write to the database, or log every tick.

### Assignment transition

Create one focused, idempotent primary-work transition used by new site
commands, legacy `OrderGather`, and structure assignment paths. It must:

1. Validate that the entity still exists, is living, is owned by the requesting
   player, and is not Offline-Protection frozen.
2. Validate the exact site, survey/resource eligibility, and reachability.
3. Cancel pending work events for the old order before changing position or
   state. The current stale gather guard checks only `State::Gathering`, so an
   uncancelled event could otherwise gather from the worker's new location.
4. Detach the worker from its old site and from any structure's reverse
   `Assignments` list.
5. Clear transient `Destination`, `Storage`, `ToolFetchTarget`, `BlockedWork`,
   `EventCompleted`, and stale active-task/state data.
6. Update the canonical registry and install the new `Order::Gather` with safe
   checked/`try_insert` command patterns.
7. Emit canonical changes for every affected site and structure.

The structure `Assign`/`RemoveAssign` and direct structure-order handlers must
use the same detach path when taking a site worker. This is directly related
cleanup, not a general labor-system rewrite.

### AI prerequisites

Remote assignments make two current defects user-visible and they must be fixed
before enabling Form A:

1. The actual rescued-villager and hired/cargo-conversion Thinker sequences in
   `sp_server/src/encounter.rs` omit `MaybeTransferGatherTool`, even though the
   action system and focused tests exist. Add it between the first move and the
   second move, matching the intended sequence in `player_setup.rs`.
2. The no-path branch in `move_to_system` sets `Failure` and then overwrites it
   with `Executing`. Preserve failure, attach a stable `BlockedWork` reason, and
   retry on a bounded cadence without log spam.

Also make storage selection capacity-aware. If there is no reachable owned
storage with room, retain the standing assignment, expose **No available
storage**, and retry when a relevant structure/inventory change occurs. Keep
equipped tools on the villager during unload as the existing code does.

## Implementation checkpoints

### Checkpoint 0: contract and remote-work hardening

- Freeze the `(player, position, res_type)` key, four states, one-site/one-worker
  rules, and literal Survey + revealed-resource eligibility.
- Add regression tests and fixes for missing `MaybeTransferGatherTool` in real
  villager constructors.
- Fix no-path failure/retry behavior and add a stable blocked reason.
- Add storage reachability/capacity checks needed by long-distance standing
  work.
- Do not expose new UI yet.

Exit criterion: an existing manually installed remote `Order::Gather` either
completes travel/tool/gather/unload/repeat or settles into a recoverable blocked
state; it never wedges or logs every tick.

### Checkpoint 1: authoritative registry and protocol

- Add pure standing-site registry operations and revisioning.
- Add eligibility, candidate, snapshot, mutation, and reconciliation helpers.
- Add new `NetworkPacket`, `ResponsePacket`, internal `PlayerEvent`, routing,
  mutability, and Offline Protection classifications.
- Route legacy `order_gather` through the shared transition while preserving its
  hero-position behavior.
- Integrate structure-to-site and site-to-structure detachment.
- Send canonical snapshot/deltas and add login/reconnect synchronization.
- Keep the feature inaccessible from the production UI until Checkpoint 2.

Exit criterion: protocol/headless tests can create, assign, reassign, unassign,
and remove remote sites idempotently without moving the hero.

### Checkpoint 2: Form A desktop and mobile

- Add shared site DTOs/store/revision reducer and candidate sorting helpers.
- Add the dedicated Phaser badge/hex-overlay container and direct site click.
- Add contextual site actions and new desktop/mobile candidate picker panels.
- Add one-resource fast path and multi-resource tile chooser.
- Refresh open tile/site views from authoritative deltas.
- Migrate first-party client use to the new commands; keep the legacy server
  command and migrate `sp_agent/tools.py` additively.

Exit criterion: from any known tile on both desktop and mobile, the normal flow
is badge -> assign -> villager, and server rejection cannot leave a false badge.

### Checkpoint 3: lifecycle and production-path regression

- Clear a dead/despawned/transferred worker while retaining its site as standing
  and unassigned.
- Remove only the dead player's site records during True Death and start-location
  recycling; preserve another player's record on the same physical tile.
- Decide explicitly whether `SurveyHistory` is run-scoped. Current True Death
  clears `ExploredMap` but not survey history; the recommended consistent rule
  is to clear both survey and standing-site records for the dead run.
- Verify ordinary disconnect behavior, Offline Protection freeze/no catch-up,
  reconnect snapshot, combat/needs interruption, tool recovery, storage
  recovery, and entity cleanup.
- Extend the existing headless harness rather than adding another simulator.

Exit criterion: all lifecycle cases leave no stale worker IDs, reverse structure
assignments, destinations, or gather events.

### Checkpoint 4: optional Form B

- Add the panel over the existing snapshot/store.
- Add unassigned-first sorting and a Locate action that centers all three Phaser
  scene cameras.
- Do not change the server model or mutation protocol.

### Deferred: automatic labor claiming

If an unassigned standing site should be claimed automatically, first define an
explicit player-controlled villager labor preference or job affinity. Then add a
deterministic claim/release scheduler with ownership, reachability, needs,
structure-work priority, and duplicate-tick tests. Do not use "highest skill" as
an implicit profession and do not add new professions as part of Form A.

## Expected files

The exact diff should remain checkpoint-scoped, but implementation is expected
to touch these areas:

### Server

- `sp_server/src/resource.rs`: site keys, registry, eligibility, and snapshot
  helpers.
- `sp_server/src/obj.rs`: gather-order cleanup/supporting reflected types if
  needed.
- `sp_server/src/network.rs`: additive commands, responses, DTOs, routing, and
  serialization tests.
- `sp_server/src/player.rs`: events, classification, query/mutation handlers,
  shared primary-work transition, and tile summary.
- `sp_server/src/ai/villager/villager.rs`: path failure, blocked/retry state,
  storage selection, and reconciliation/activity integration.
- `sp_server/src/encounter.rs` and `sp_server/src/player_setup.rs`: identical
  Thinker sequences for every real villager creation path.
- `sp_server/src/game.rs` and `sp_server/src/safe_logout.rs`: login sync,
  cancellation, ownership/death/True Death cleanup, and protection coverage.
- `sp_server/src/lib.rs`: type registration only if best-effort scene reflection
  is included.
- `sp_server/src/headless.rs`, `sp_server/src/headless_bot.rs`, and
  `sp_server/src/bin/headless_runner.rs`: production-path scenarios and first-
  party migration.
- `sp_server/src/ai/villager/villager_tests.rs` and
  `sp_server/src/game_tests.rs`: focused unit/integration regressions.

### Client and tooling

- `sp_frontend/sp_ts/src/sp/core/network.ts`, `networkEvent.ts`, `gameEvent.ts`,
  and a focused shared `resourceSiteState.ts`.
- `sp_frontend/sp_ts/src/sp/core/scenes/mapScene.ts` and, only if required for
  unobscured markers, `objectScene.ts`.
- Desktop/mobile `ui.tsx` and `targetActionPanel.tsx`.
- New focused desktop/mobile resource-site picker panels using existing shells.
- `sp_agent/tools.py`: additive commands for automated/manual agent control.

Form B files are deliberately excluded from the first user-visible checkpoint.

## Regression matrix

### Authority and idempotency

- Valid remote assignment succeeds without moving the hero.
- Out-of-bounds, impassable, unsurveyed, unrevealed/unknown-type, foreign-player,
  dead-worker, and protected-run commands fail closed.
- Duplicate set/unassign/remove commands do not duplicate work, revisions, or
  pending events.
- One site cannot retain two workers; one worker cannot remain on two sites.
- Reassignment publishes both old and new canonical site states.
- Two players can designate the same tile without seeing or mutating each
  other's worker binding.

### Work execution

- The worker reaches the exact tile and gathers only the selected resource type.
- A required tool is equipped locally or fetched from reachable owned storage.
- Tool-free plant gathering remains valid.
- Capacity pressure causes unload, preserves equipped gear, and returns to site.
- Missing tool, unreachable site, missing/full storage, and changed terrain
  produce recoverable blocked states.
- Thirst, hunger, tiredness, shelter, combat, and hauling interrupt then resume
  the standing order.
- Reassigning during an in-flight gather cannot complete the old event at the
  new location.

### Lifecycle

- Ordinary disconnect retains the site/order and follows current online-world
  rules.
- Offline Protection freezes travel, gathering, hauling, deadlines, and site
  mutation with no catch-up.
- Reconnect receives one authoritative full snapshot before deltas.
- Villager death/despawn/owner transfer makes the site standing-unassigned.
- Structure assignment removes the site binding and vice versa.
- True Death and fresh-run setup remove only the relevant player's records and
  leave another player's same-tile record intact.

### Client

- Snapshot and stale/duplicate delta application is revision-safe.
- Clicking a badge on an occupied tile opens the site flow, not the last object.
- One-resource and multi-resource tiles target the correct `res_type`.
- Candidate ordering and disabled reasons are deterministic.
- Empty candidate lists do not crash; current assignment refreshes safely.
- Server rejection never leaves optimistic assignment state behind.
- Badges remain legible under day/night, weather, visibility, and selection
  effects on desktop and mobile.

## Validation commands for implementation

Run from `sp_server/` as applicable to each Rust checkpoint:

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features
cargo run --bin headless_runner -- 5 12000 standard
```

For the TypeScript checkpoint, run the repository import check, a development
webpack build, focused Jest tests for the site reducer/sorting/protocol handling,
and manual desktop/mobile interaction checks. Record exact commands and results
in the checkpoint completion report.

## Risks and explicit non-goals

- **Survey versus Prospect:** the code treats them as separate actions. This
  plan preserves that reality and requires both a player survey record and a
  revealed matching resource. Merging the actions needs its own product change.
- **Global resource reveal:** the feature must not turn it into a global site
  directory. Site snapshots are filtered by the requesting player's survey
  history.
- **Assignment consistency:** structure assignment currently has forward and
  reverse state separate from `Order`; partial cleanup is the highest server
  risk. The shared transition and lifecycle tests are mandatory.
- **Remote pathing:** existing no-path and real-villager tool-fetch defects are
  release blockers for Form A, not optional polish.
- **Process restart:** the current full-run DynamicScene/database persistence
  path is incomplete and `Order` is not reflected. The first milestone promises
  same-process reconnect, not durable restart recovery or per-tick DB writes. A
  later best-effort scene checkpoint can reflect/register the registry and
  reconstruct `Order::Gather` after load.
- **No resource-system redesign:** quantities, yields, recipes, skills, tools,
  inventories, hauling, and gathering rates remain unchanged.
- **No territorial reservation:** other players may gather the same shared tile.
- **No painting:** a designation targets one exact hex/resource type. Freeform
  brushes, lasso selection, drag rectangles, and forest districts are out of
  scope.
- **No Form B dependency:** the panel ships only if map navigation proves
  annoying; all gameplay is complete through Form A.
