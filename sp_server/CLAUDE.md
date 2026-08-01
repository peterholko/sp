# CLAUDE.md - Siege Perilous Game Server

## Project Overview

Siege Perilous is a persistent shared-world settlement-survival game server
written in Rust. It uses Bevy ECS for authoritative game logic, WebSocket over
TLS for the live protocol, and PostgreSQL for account, session, and score
records. Multiple players can coexist in the same continuously advancing world;
each owns a hero, villagers, settlement, introduction state, and ordered
personal-crisis progression.

- **Package:** `siege_perilous` v0.5.0
- **Rust Edition:** 2021
- **Binaries:** `siege_perilous`, `headless_runner`
- **Entry point:** `src/main.rs` → `src/lib.rs::setup()`
- **Simulation rate:** 10 game ticks per second
- **Active map:** 60×50, loaded from `map/test3.tmx`
- **Start locations:** five reusable entries in `templates/player_start.yaml`

### Game Direction (the north star)

The current game is a prepare-and-survive experience inside a shared
environment:

- **Global environment:** day/night, weather, lighting, visibility, needs, and
  night-travel consequences advance for the whole world.
- **Revised opening:** a fresh hero investigates the run-owned Shipwreck,
  recovers the exact starter salvage, builds a normal Burrow, defeats the
  one-to-three Giant Rat wave, and then receives the rescued first villager.
- **Personal crises:** `SettlementCrisisState` allows at most one current crisis
  per player. `PERSONAL_CRISIS_SEQUENCE` is Goblin then Undead. Pre-assault
  progression is owner-online-only; once launched, an assault remains committed
  through an ordinary disconnect.
- **Authority:** personal attackers and spells carry player/assault
  attribution, and resolution is idempotent. Solo completion remains possible;
  helpers may assist but are not required.
- **Safe Logout:** explicit server-accepted protection freezes the owning run
  after its countdown. Reconnect removes protection before that run resumes.
- **Monolith, sanctuary, scoring, and True Death:** the run retains its bound
  sanctuary, resurrection and permanent-death path, score breakdown, start
  recycling, and replay lifecycle.
- **Established economy:** gathering, farming, fishing, hunting, refining,
  smelting, tanning, cooking, recipes, work queues, villagers, crafting, and
  trade remain active systems.

`SurvivalDirectorMode::PersonalCrisis` is the production and headless default.
The earlier rat/wolf/tiered wave, nightly horde, Goblin Pillager, and legendary
director remains available only through `SurvivalDirectorMode::Legacy`; do not
describe that legacy schedule as a second active danger authority.

The current design contracts are indexed in `../docs/README.md`. Historical
balance reports describe the revision that generated them rather than the
current checkout.

## Build & Run Commands

```bash
# Build
cargo build
cargo build --release

# Run (starts new game by default)
cargo run
cargo run -- reload    # Reload existing game state from saved scene

# Tests
cargo fmt --all -- --check
cargo check
cargo test --no-fail-fast               # All test targets; keep all failures visible
cargo test --lib game_tests             # Game unit tests
cargo test --lib villager_tests         # Villager AI tests
cargo test --test day_system_test       # Day/night integration tests
cargo test --lib headless::tests::smoke # In-process production-schedule smoke
cargo test -- --nocapture               # Show println/log output

# Lint & format
cargo clippy --all-targets --all-features

# Bounded headless run
cargo run --bin headless_runner -- 1 6000 standard
```

## Architecture

### Core Framework

The server runs as a headless Bevy app at 10 ticks/second (`TIMESTEP_10_PER_SECOND`). There is no rendering — Bevy is used purely for its ECS and scheduling. The app progresses through states: `Loading` → `PreRunning` → `Running`.

### Module Layout

```
src/
├── main.rs              # CLI entry point, parses "reload" arg
├── lib.rs               # Bevy App setup, plugin registration, clippy config
├── game.rs              # Core game loop, event processing, tick systems AND the
│                        #   introductions, personal + legacy danger directors,
│                        #   monolith/sanctuary, scoring, true death (~23K lines)
├── game_tests.rs        # Unit tests for game systems
├── safe_logout.rs       # Safe Logout authority, freezing, delivery snapshots
├── headless.rs          # In-process harness, fixtures, telemetry, regressions
├── headless_bot.rs      # Scripted production-path bot
├── bin/headless_runner.rs # Repeated-run CLI and CSV/JSON reporting
│
├── Network & Persistence
│   ├── network.rs       # WebSocket/TLS server, packet handling
│   ├── player.rs        # Player event handling, info queries
│   ├── player_setup.rs  # Game initialization, hero creation
│   ├── database.rs      # Database event definitions
│   └── account.rs       # Account management (placeholder)
│
├── Gameplay Systems
│   ├── obj.rs           # Entity components (Id, Position, State, Class, etc.)
│   ├── item.rs          # Items, inventory, equipment
│   ├── map.rs           # Map generation, terrain, tiles
│   ├── resource.rs      # Resource gathering mechanics
│   ├── combat.rs        # Combat calculations, damage
│   ├── effect.rs        # Buffs/debuffs, status effects
│   ├── encounter.rs     # Random encounters, spawn mechanics
│   ├── structure.rs     # Buildings, construction
│   ├── recipe.rs        # Crafting system
│   ├── farm.rs          # Farming, crops
│   ├── trade.rs         # Trading system
│   ├── experiment.rs    # Experimentation/research
│   ├── event.rs         # Game event system, state machines
│   ├── world.rs         # Weather, time of day, vision
│   ├── constants.rs     # Game balance constants
│   ├── terrain_feature.rs
│   └── villager_util.rs
│
├── AI (big-brain utility AI)
│   ├── ai/common/common.rs    # Shared AI components
│   ├── ai/common/logging.rs   # AI debug logging (daily rotating files)
│   ├── ai/npc/npc.rs          # Generic NPC behavior
│   ├── ai/villager/villager.rs          # Villager AI (needs-based)
│   ├── ai/villager/villager_tests.rs    # Villager integration tests
│   └── ai/tax_collector/tax_collector.rs
│
├── Skills
│   ├── skill/skill.rs      # Skill tracking & progression
│   └── skill/skill_defs.rs # Skill definitions enum
│
├── Utilities
│   ├── ids.rs           # Entity ID mapping
│   └── templates.rs     # YAML template loading
```

**Note:** AI and skill modules use `#[path = "..."]` attributes in `lib.rs` rather than standard `mod` directories.

### Data Files

```
templates/               # YAML game data
├── item_template.yaml
├── obj_template.yaml
├── obj_init.yaml
├── recipe_template.yaml
├── refine_template.yaml
├── effect_template.yaml
├── skill_xp_template.yaml
├── skills.yaml
├── res_template.yaml
├── res_property_template.yaml
├── price_template.yaml
├── combo_template.yaml
├── dialogue_template.yaml
├── terrain_feature_template.yaml
└── player_start.yaml

db/                      # SQL schemas
├── accounts.sql
schema.sql
accounts_schema.sql
scores_schema.sql

map/                     # Tiled map files
tileset/                 # JSON sprite definitions
```

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| `bevy 0.17.3` | ECS game engine (headless, no rendering) |
| `big-brain` | Utility AI system (custom Codeberg fork) |
| `tokio` | Async runtime for networking |
| `tokio-tungstenite` | WebSocket server |
| `tokio-postgres` / `deadpool-postgres` | PostgreSQL with connection pooling |
| `tokio-rustls` | TLS/SSL |
| `serde` / `serde_json` / `serde_yaml` | Serialization |
| `pathfinding` | A* pathfinding |
| `argon2` | Password hashing |
| `tracing` | Structured logging |

## Code Conventions

### Naming
- **Structs/Enums:** PascalCase (`GameTick`, `EventExecuting`)
- **Functions:** snake_case (`spawn_all_resources`, `get_by_type`)
- **Constants:** UPPER_CASE (`MAX_PLAYER_ID`, `GAME_HOUR`)
- **Files:** snake_case (`game_tests.rs`, `villager_util.rs`)

### Clippy Configuration
The project uses permissive clippy settings appropriate for game development (configured in `lib.rs`):
```rust
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::enum_glob_use)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]
```

### Patterns
- **ECS Components:** Derive `Component` + `Reflect` with `#[reflect(Component)]` for serialization
- **Error handling:** Custom enums (e.g., `ResourceGatherError`), pattern matching on Results, `error!()` macro for logging
- **Async:** `tokio` runtime with `Arc<Mutex<>>` for shared state, crossbeam channels for inter-thread communication
- **Serialization:** `#[serde(skip_serializing_none)]` on structs with optional fields
- **AI:** big-brain Scorer + Action pattern for NPC decision-making

### Entity Model
Core components attached to game entities:
- `Id(i32)` — unique entity identifier
- `Position { x, y }` — map coordinates
- `PlayerId(i32)` — owner (player or NPC group)
- `Class` / `Subclass` — entity classification (unit, structure, hero, villager, etc.)
- `State` — state machine (Idle, Moving, Building, Dead, etc.)
- `Viewshed { range }` — vision radius
- `Template` — object template reference
- `Inventory` — items
- `Skills` — skill levels and XP

### Game Tick System
The game uses a custom tick system (not real-time clock):
- `GAME_TICKS_PER_DAY = 2400`
- Key phases: FIRST_LIGHT(400), DAWN(500), MORNING(600), AFTERNOON(1200), EVENING(1800), DUSK(2000), NIGHT(2200)

## Testing Patterns

### Game Tests (`src/game_tests.rs`)
Standard Bevy App-based testing:
```rust
#[test]
fn test_something() {
    let mut app = App::new();
    // Add systems and components
    // Run app.update()
    // Assert state
}
```

### Villager AI Tests (`src/ai/villager/villager_tests.rs`)
Uses a custom `setup_test_app!` macro and `TestVillagerBuilder` for flexible test entity setup. Tests full behavior cycles (drinking, eating, sleeping).

### Integration Tests (`tests/day_system_test.rs`)
Tests day/night cycle effects on viewshed ranges.

## Persistence boundary

PostgreSQL stores accounts, sessions, and scores; passwords use Argon2 and
login uses session authentication. The production server also has Bevy dynamic
scene snapshot/reload support. Personal introduction, crisis, start-assignment,
and Safe Logout coordination include runtime-only state, so the current design
does not claim complete process-restart restoration for every per-run graph.

The `scores` table (`scores_schema.sql`) is the run-history / leaderboard sink, written on True
Death. Beyond `hero_name` / `hero_rank` / `total_xp` / `fate`, it stores the full score breakdown
(`score_survival`, `score_progression`, `score_wealth`, `score_defense`, `score_valor`,
`score_legacy`, `total_score`) plus survival telemetry: `days_survived`, `waves_survived`,
`highest_pressure_level`, `crisis_tier`, `legendary_kills`, `hideouts_cleared`. Some field names
remain legacy-compatible even though Personal Crisis is now the default director.

## Network Protocol

WebSocket traffic runs over TLS and server responses serialize through
`ResponsePacket`. Player commands become `PlayerEvent` values and remain
server-validated. The client receives full or incremental terrain/object
perception plus focused state snapshots for weather, objectives, personal
crisis, Safe Logout, protected settlements, scoring, resurrection, and True
Death. Treat those packets as presentation state, not client authority.

## Important Notes

- `game.rs` remains the largest module at roughly 23,000 lines. Inspect its
  actual schedules and resources before assuming the higher-level ownership
  list in this guide is exhaustive.
- Bevy deferred commands can race despawns. Use safe/idempotent command patterns
  such as `try_insert` whenever an entity may disappear before commands apply.
- Avoid spawning from read-only reporting systems, per-tick database writes,
  and per-tick log spam.
- The `big-brain` dependency uses a pinned commit from a Codeberg fork rather
  than crates.io.
- AI debug logs rotate daily to `logs/ai_debug.log`.
- Production startup requires database and TLS environment configuration.
- `cargo run` starts a new game; `cargo run -- reload` loads the supported
  dynamic-scene state.
