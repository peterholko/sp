# In-Process Headless Harness — Current Architecture

## Status

Implemented. The current harness drives the production gameplay plugin graph
and server-authoritative systems while replacing the TLS WebSocket,
PostgreSQL, filesystem snapshot, and real-time schedule runner with in-process
channels and explicit `App::update()` calls.

Implementation lives in:

- `src/lib.rs` — shared type registration plus
  `build_headless_app[_with_director]`;
- `src/game.rs` — shared world initialization and the production/headless
  `GamePlugin` split;
- `src/headless.rs` — `HeadlessGame`, world snapshots, fixtures, packet capture,
  telemetry, and regression scenarios;
- `src/headless_bot.rs` — the deterministic scripted action policy; and
- `src/bin/headless_runner.rs` — repeated-run execution, CSV/JSON output, and
  aggregate reporting.

Run Cargo commands from `sp_server/`. Template paths and several data paths are
relative to that working directory.

## Runtime shape

Production calls `setup()` with:

- Bevy state, asset, scene, task-pool, frame-count, logging, and
  `ScheduleRunnerPlugin` plugins;
- `GamePlugin { headless: false }`; and
- the Tokio TLS WebSocket/PostgreSQL initialization path.

The harness calls `build_headless_app_with_director()` with the same gameplay
plugins and reflect registration, but without `ScheduleRunnerPlugin`,
`LogPlugin`, or the production filesystem snapshot system. `HeadlessGame`
supplies:

- a real `NetworkReceiver` channel for normal `PlayerEvent` ingress;
- an authoritative `Clients` entry with bounded packet capture;
- a dummy database channel so production database-send seams remain present
  without contacting PostgreSQL; and
- a fresh Bevy `App` for each run.

`HEADLESS_PLAYER_ID` is `1`, which is inside the server's human-player range.
Additional helpers can create connected players for multiplayer, authority,
disconnect, reconnect, and Safe Logout scenarios.

The game is tick-driven. Pumping `app.update()` advances `GameTick` without a
wall-clock sleep. World generation, combat, loot, and other production systems
still call runtime RNG, so the scripted bot is deterministic but repeated
whole-game results are not bit-identical.

## Input, observation, and packet behavior

The harness creates heroes with the production `PlayerEvent::NewPlayer` path.
Actions are also normal `PlayerEvent` values. The player message broker
consumes one queued event per update, so callers inject at most one decision
and then advance the game before making another.

The bot reads an owned `WorldView` snapshot rather than reimplementing
perception JSON. The view currently exposes:

- hero state, class, needs, resources, position, and death state;
- carried item facts needed by the survival policy;
- enemies and personal-assault attribution;
- villagers, POIs, the run-owned Shipwreck, merchant, monolith, corpses, and
  structures;
- discovered resource tiles and map occupancy; and
- game tick, day, and current personal-crisis phase.

Outgoing production packets are drained every update. Sparse packet types can
be retained for assertions and telemetry without allowing a bounded channel to
fill during long simulations.

## Current scripted bot

The current phase enum is:

```text
Bootstrap -> Build -> Fortify -> Survive -> Done
```

The bot prioritizes survival and immediate threats, then:

1. investigates the run-owned Shipwreck;
2. manually transfers its salvage through the production item-transfer path;
3. equips recovered class gear and the Sharpened Stick, with the Crude Hatchet
   available for lumberjacking;
4. builds a normal Burrow from the five recovered Logs;
5. uses or replaces a Campfire when the selected policy requires one;
6. cooks and stockpiles food, gathers, hires or assigns villagers, upgrades the
   sanctuary, and builds Stockades according to the scenario policy; and
7. explores deterministic waypoints when no higher-priority work exists.

The bot has class-valid combat policies for Warrior, Ranger, and Mage and
scenario policies for passive, basic, prepared, fortified, villager-supported,
helper-supported, disconnect, and Safe Logout runs.

The bot does not implement the parked Crafting Tent/Blacksmith gear ladder.
See `docs/gear_progression_plan.md`.

## Runner command

```bash
cargo run --bin headless_runner -- [N] [MAX_TICKS] [MODE] [SIDE]
```

Defaults are 20 games, 120,000 ticks, and `standard`.

| Mode | Purpose |
| --- | --- |
| `standard` | Normal scripted survival runs |
| `safe-logout` | Bounded Safe Logout exercise |
| `safe-logout-matrix` | Rotates through eight lifecycle, reconnect, disconnect, and multiplayer scenarios |
| `goblin-balance` | Runs the class/scenario balance matrix and writes the Checkpoint 2-style report |

`SIDE` applies only to `goblin-balance` and accepts `control` or `candidate`.
The runner validates the selected label against the compiled
Preparing/AssaultReady thresholds before writing a comparison report.

Every run is wrapped in `catch_unwind`; a panic becomes a retained `Panic`
result instead of aborting the batch. The normal artifacts are:

- `headless_runs.csv`
- `headless_runs.json`

Both are ignored/generated evidence, not gameplay input.

## Validation

Focused smoke:

```bash
cargo test --lib headless::tests::smoke
```

Broader server validation:

```bash
cargo fmt --all -- --check
cargo check
cargo test --no-fail-fast
```

Bounded runner example:

```bash
cargo run --bin headless_runner -- 1 6000 standard
```

Gameplay milestones add focused tests to `src/headless.rs` instead of creating
parallel simulation frameworks. Current coverage includes the revised opening,
Campfire visibility and public ignition, personal Goblin and Undead lifecycles,
Safe Logout freezing/resume, reconnect and True Death cleanup, multiplayer
ownership, and balance telemetry.

## Known limitations

- A fresh `App` isolates runs, but production RNG prevents exact replay.
- Several metrics and scenario fixtures remain Goblin-balance-specific; Undead
  functional validation is primarily in focused headless tests.
- The harness bypasses TLS, HTTP authentication, PostgreSQL, browser rendering,
  and wall-clock scheduling. Separate protocol/client checks cover those
  boundaries.
- Runtime game data still needs the `sp_server/` working directory.
- Standard runs can legitimately end in True Death or hit the tick cap; a
  successful process exit is not evidence of a gameplay victory or final
  balance.
