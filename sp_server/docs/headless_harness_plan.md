# In-Process Headless Test Harness — Implementation Plan

> Status: **approved design, not yet implemented.** This document is the build brief for an
> in-process Rust test harness that drives the Bevy game `App` directly so we can run many full
> games quickly for balance/metrics testing.

## Context

`sp_server` (Siege Perilous) can today only be played over the real client path:
WebSocket-over-TLS → session cookie validated against PostgreSQL `sessions` → class selection →
real-time play at 10 ticks/sec. An existing Python LLM agent (`sp_agent/`) automates a single
session but inherits all that friction (needs `sp_axum` + Postgres + TLS + a registered account,
runs in wall-clock real time, one game at a time, costs LLM tokens).

To **run many full games quickly for balance/metrics testing**, we remove the
network/auth/real-time barriers and drive the game directly. The chosen approach is an
**in-process Rust harness**: build the Bevy `App` directly (no TLS / no WebSocket / no Postgres /
no real-time scheduler), drive it with a **deterministic scripted bot**, fast-forward ticks by
pumping `app.update()`, run N games back-to-back, and emit per-run + aggregate **metrics**.

### Why this is feasible (verified in code)

- **Fast-forward is free**: there are **zero** `Res<Time>` / `Instant::now` / `tokio::time` /
  `thread::sleep` usages in `sp_server/src`. The game is 100% `GameTick`-driven (incremented once
  per `Update` in `update_game_tick`, game.rs ~14880). Looping `app.update()` advances game time
  deterministically with no wall-clock waiting.
- **Output capture is trivial**: `send_to_client` (network.rs:1406) just does
  `client.sender.try_send(json)` over a `tokio::sync::mpsc::Sender<String>`. `try_send`/`try_recv`
  need no running runtime — the harness inserts a `Client` and drains the receiver.
- **Input injection is trivial**: actions are `PlayerEvent`s (player.rs:99-493) pushed into the
  `NetworkReceiver` crossbeam channel (game.rs:93) and drained by `message_broker_system`
  (player.rs ~755, one event per `update()`). Hero creation = inject
  `PlayerEvent::NewPlayer { player_id, hero_name, class_name }` (exactly what
  `handle_selected_class` sends, network.rs:2726).
- **DB never touched**: all env/TLS/Postgres reads live inside `tokio_setup`; by not spawning it,
  the headless path never touches them. One guard needed: `send_to_database` (network.rs:1421)
  does `.get(&DATABASE_MANAGER_ID).unwrap()` → harness must register a **dummy** `DatabaseClient`.
- **Multi-game isolation is clean**: the only global statics are `LOG_RELOAD_HANDLE` (logging) and
  `TILESET` (cosmetic image cache) — neither holds per-game mutable state. All game state lives in
  the App's `World`/resources, so dropping & recreating a `HeadlessGame` between runs fully isolates.

## Approach

Additive only — the existing `setup()` and `tokio_setup` networked path stays byte-for-byte
identical. New headless code compiles into the lib as extra functions + modules (no cargo feature
needed).

### 1. Server-side decoupling — `src/lib.rs`, `src/game.rs`

**a) Split `Game::new_game_setup` (game.rs:1545-~1674)** into:
   - `Game::world_init(...)` — world build only (spawn resources, terrain, recipes, prices, and
     insert `GameTick`/`MapEvents`/`Objectives`/`RunScoreState`/`VictoryState`/`CrisisState`/etc).
     **Excludes** the three network resources and the `tokio_setup` spawn (game.rs ~1582-1597,
     ~1646-1648).
   - `Game::network_init(...)` — the extracted network portion (crossbeam channel, `tokio_setup`
     spawn, insert `NetworkReceiver`/`Clients`/`DatabaseManagers`).
   - Keep `new_game_setup` as a thin wrapper calling both + `next_state.set(Running)` — **behavior
     preserving** for the production path.

**b) Add `headless: bool` to `GamePlugin`** (struct at game.rs:1205, `impl Default` at 1209). In
   `GamePlugin::build` (game.rs:1216), when `headless`, register `Game::new_game_setup_headless` on
   `PreStartup` instead of `new_game_setup`. The headless variant calls `world_init` +
   `next_state.set(Running)` only. **All other GamePlugin sub-plugins and ~50 Update systems are
   kept identical** — they are pure game logic and must run.

**c) Refactor the ~70 `register_type` calls** (lib.rs:134-200) into a shared
   `register_all_types(app: &mut App)` used by both `setup()` and the new headless builder, to
   guarantee identical reflect registry.

**d) Add `build_headless_app() -> App`** in lib.rs:
   - Include: `StatesPlugin`, `AssetPlugin`, `ScenePlugin`, `TaskPoolPlugin` (provides the IoTaskPool
     AssetPlugin needs), `FrameCountPlugin`, `GamePlugin { new_game: true, headless: true }`,
     `init_state::<AppState>()`, `register_all_types`, `init_asset::<DynamicScene>()`.
   - **Exclude**: `ScheduleRunnerPlugin::run_loop` (no real-time loop — harness pumps manually) and
     `LogPlugin` (omit for quiet/speed; `LOG_RELOAD_HANDLE` simply stays `None`).
   - Add `pub mod headless;` / `pub mod headless_bot;` and re-export `ResponsePacket` for the harness.

### 2. Harness — `src/headless.rs` (new)

`HeadlessGame` owning: `app: App`, `player_id`, `event_tx: crossbeam Sender<PlayerEvent>`,
`packet_rx: tokio mpsc Receiver<String>`, `_db_rx` (kept alive to satisfy the dummy manager),
`tick_count`, `max_ticks`.

- `new(max_ticks)` — `build_headless_app()`, create the crossbeam event channel + tokio packet/db
  channels, insert `NetworkReceiver(event_rx)`, an empty `Clients`, and a `DatabaseManagers` holding
  a dummy `DatabaseClient { sender: db_tx }` under `DATABASE_MANAGER_ID`. Pump 2 `update()`s to run
  `PreStartup` world-init → transition to `Running` → `OnEnter(Running)` init.
- `spawn_hero(class, name) -> i32` — pick a deterministic `player_id` (e.g. 1000), insert a
  matching `Client { player_id, sender: packet_tx.clone() }` into `Clients` so `send_to_client`
  reaches us, inject `PlayerEvent::NewPlayer{..}`, then `tick(8)` to let the hero spawn.
- `inject(PlayerEvent)`, `tick(n)` (loop `app.update()`), `drain_packets() -> Vec<ResponsePacket>`,
  `world()` / `app_mut()` accessors, `game_tick()`, `is_over()` (max_ticks OR `VictoryState`
  win OR hero `TrueDeath`/missing), `metrics() -> RunMetrics`.
- **Bot reads `World` directly** via queries (`Position`, `Stats`, `Skills`, `Inventory`, `State`,
  nearby entities) rather than parsing perception JSON — simpler and deterministic. `drain_packets`
  is mainly for assertions/debug.
- **Pacing constraint**: `message_broker_system` drains **one** event per `update()` (player.rs
  `if let Ok`). So inject one action per decision step, then `tick(N)` — the runner loop does this
  naturally. (Do **not** change the broker to a `while let` loop — that would alter production
  behavior.)

### 3. Scripted bot — `src/headless_bot.rs` (new)

Deterministic, phase-based `Bot` (no RNG, or per-run seeded). `step(&HeadlessGame) -> Option<PlayerEvent>`
reads World, decides one action. Phases: `Bootstrap → Survive → Gather → Build → Fight → Explore →
Done`, transitioning on `Objectives`/`RunScoreState`/day count. Action emitters map to `PlayerEvent`
variants (`Move`, `Gather`, `Craft`, `StructureCraft`, `Attack`, `Harvest`, …). Helpers
`nearest_resource_node`, `nearest_enemy`, `path_step_toward` use the `Map` resource + `Position`
queries. `DECISION_TICKS` ≈ 4-10 per step (server actions resolve over several ticks).

### 4. Multi-game runner — `src/bin/headless_runner.rs` (new) + `[[bin]]` in `Cargo.toml`

```text
for i in 0..N {
    let mut g = HeadlessGame::new(MAX_TICKS);     // fresh App = isolation
    let pid = g.spawn_hero("Warrior", &format!("Bot{i}"));
    let mut bot = Bot::new(pid);
    while !g.is_over() {
        if let Some(ev) = bot.step(&g) { g.inject(ev); }
        g.tick(DECISION_TICKS);
        bot.advance_phase(&g);
    }
    results.push(g.metrics());                    // g dropped -> full cleanup
}
write_csv + write_json + print_summary(results);  // win rate, mean days survived, p50/p90 ticks
```

**`RunMetrics`** (derive `Serialize`), read from `RunScoreState` (waves/enemies/elites/captains/
legendary kills, hideouts cleared, repairs, highest_pressure_level), `PlayerStats` (deaths),
`Objectives` (all 10 bools), `VictoryState` (rescue_progress/prosperity/conquest), plus hero
end-state (`final_hp`, skill total, inventory count, structures_built) and
`outcome`/`ticks`/`days_survived`. **Read the actual struct field names in game.rs before wiring.**

## Critical files

- `src/lib.rs` — `register_all_types`, `build_headless_app`, `pub mod headless;` /
  `pub mod headless_bot;`, re-export `ResponsePacket`.
- `src/game.rs` — split `new_game_setup` → `world_init`/`network_init`; add `headless` flag +
  `new_game_setup_headless` in `GamePlugin::build` (~1216).
- `src/headless.rs` *(new)* — `HeadlessGame`, `RunMetrics`, smoke test.
- `src/headless_bot.rs` *(new)* — deterministic `Bot`.
- `src/bin/headless_runner.rs` *(new)* — multi-game loop + CSV/JSON.
- `Cargo.toml` — add `[[bin]] headless_runner`.
- Read-only refs: `network.rs` (`send_to_client`/`send_to_database`/`tokio_setup`), `player.rs`
  (`PlayerEvent`, `message_broker_system`, `new_player_system`). Existing protocol/state mapping in
  `sp_agent/{tools.py,game_state.py}` is a useful reference for action/field names.

## Risks & mitigations

1. **Working-dir dependence (confirmed)** — `templates/*.yaml`, `map/*`, `tileset/*` load relative
   to CWD. Harness, runner, and tests **must run with CWD = `sp_server/`** (existing tests already do).
2. **`send_to_database` panic** — register a dummy `DatabaseClient` under `DATABASE_MANAGER_ID`.
3. **`register_type` parity** — shared `register_all_types` keeps reflect registry identical.
4. **Broker single-drain-per-tick** — pace one action per decision step (handled by the loop shape).
5. **App drop isolation** — verify two back-to-back runs with the same deterministic bot produce
   identical metrics (regression guard against hidden static state).

## Verification

```bash
cd sp_server
cargo build --bin headless_runner
cargo run --bin headless_runner 100         # -> headless_runs.{csv,json} + summary stats
cargo test --lib headless::tests::smoke -- --nocapture   # 1 short game, asserts world built + ticks>0
cargo test                                  # ensure existing tests still pass (additive change)
```

Expected: per-run lines then an aggregate summary (win rate, mean days survived, mean enemies
killed, p50/p90 ticks), produced in seconds for many games (no sleeps, no I/O). The smoke `#[test]`
lives in `headless.rs` and runs one capped game end-to-end.
