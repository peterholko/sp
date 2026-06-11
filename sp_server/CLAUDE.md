# CLAUDE.md - Siege Perilous Game Server

## Project Overview

Siege Perilous is a single-player-per-world **survival** game server written in Rust. It uses the **Bevy ECS** engine for game logic, **WebSocket over TLS** for networking, and **PostgreSQL** for persistence. Each player controls a hero (plus villagers and structures) in a procedurally generated world with resource gathering, crafting, combat, and NPC AI on top of an escalating survival loop.

- **Package:** `siege_perilous` v0.5.0
- **Rust Edition:** 2021
- **Binary:** `siege_perilous`
- **Entry point:** `src/main.rs` → `src/lib.rs::setup()`

### Game Direction (the north star)

The game is a **prepare-and-survive** experience: the world periodically floods the player
with escalating waves of enemies, and the goal is to **survive as long as possible** while
preparing (gathering, building, fortifying, recruiting). The final score rewards how long and
how well you survived, not conquest. Most of this logic lives in `game.rs`. Key systems:

- **Per-player survival timing** — each player's clock starts when their intro chain begins
  (`PlayerIntroState`), so survival day/time is measured per player, not off the global tick.
- **Crisis tiers (1–5)** — `PlayerCrisis` / `crisis_tier()` escalate threats: rat spoilage →
  wolf pack → goblin raid → undead incursion → goblin pillager. Each tier fires on an organic
  condition with a time-based **fallback deadline** so passive players still face escalation.
- **Survival director & hordes** — from ~day 6 (`survival_director_active`), `survival_horde_size`
  / `survival_horde_composition` send periodic night hordes that scale with day, crisis tier, and
  active legendary threats.
- **Legendary threat** — a day-6 rumor / day-7 activation arc (`LegendaryThreat`) culminating in
  the **Ashen Warlord** and the **Warlord Hideout**, with follower/captain waves and a reveal.
- **Monolith / Sanctuary** — `Monolith` (collects soulshards), `BoundMonolith`, `Sanctuary` /
  `WeakSanctuary` provide protected zones; sealing the Monolith is the major end-game legacy goal.
- **Scoring** — `calculate_run_score_breakdown` produces a 6-component `ScoreBreakdown`
  (survival, progression, wealth, defense, valor, legacy); `score_total_from_breakdown` applies a
  highest-pressure-level multiplier. Persisted to the `scores` table on death.
- **True Death & start-location recycling** — `true_death_system` ends a run; the hero's start
  location is recycled back into the in-memory pool (`StartLocations`) for reuse. There are 5
  start locations.
- **Objectives** — `PlayerObjectives` tracks an onboarding/goal checklist (scavenge shipwreck,
  build campfire, win first fight, recruit villager, survive 5 nights, find the legendary hideout,
  defeat the Ashen Warlord, …) that feeds the legacy score component.

## Build & Run Commands

```bash
# Build
cargo build
cargo build --release

# Run (starts new game by default)
cargo run
cargo run -- reload    # Reload existing game state from saved scene

# Tests
cargo test                              # All tests
cargo test --lib game_tests             # Game unit tests
cargo test --lib villager_tests         # Villager AI tests
cargo test --test day_system_test       # Day/night integration tests
cargo test -- --nocapture               # Show println/log output

# Lint & format
cargo fmt
cargo clippy
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
│                        #   survival loop: crisis tiers, hordes, legendary threat,
│                        #   monolith/sanctuary, scoring, true death (~16K lines)
├── game_tests.rs        # Unit tests for game systems
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

## Database

**PostgreSQL** with tables: `accounts`, `sessions`, `scores`. Passwords hashed with Argon2. Session-based authentication.

The `scores` table (`scores_schema.sql`) is the run-history / leaderboard sink, written on True
Death. Beyond `hero_name` / `hero_rank` / `total_xp` / `fate`, it stores the full score breakdown
(`score_survival`, `score_progression`, `score_wealth`, `score_defense`, `score_valor`,
`score_legacy`, `total_score`) plus survival telemetry: `days_survived`, `waves_survived`,
`highest_pressure_level`, `crisis_tier`, `legendary_kills`, `hideouts_cleared`.

## Network Protocol

WebSocket over TLS. Packets serialized as JSON `ResponsePacket` enums. Key packet types: `Login`, `Register`, `Move`, `Attack`, plus various state update responses. Survival-loop packets carry the
score/run state — e.g. `ScoreBreakdown` (the 6 score components) and the objectives, sanctuary,
and true-death updates the client uses to drive its survival UI.

## Important Notes

- `game.rs` is the largest file (~16K lines): the core game loop, most system logic, **and** the
  survival loop (crisis tiers, hordes, legendary threat, monolith/sanctuary, scoring, true death)
- `player.rs` (~11K lines) handles all player-facing event processing
- The `big-brain` dependency uses a pinned commit from a Codeberg fork, not crates.io
- AI debug logs rotate daily to `logs/ai_debug.log`
- The app requires a `.env` file for database and TLS configuration (not checked in)
- Running `cargo run` starts a new game; `cargo run -- reload` loads from saved state
