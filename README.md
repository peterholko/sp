# Siege Perilous

Siege Perilous is a persistent shared-world, real-time settlement survival
game. Players control a hero, recover from a shipwreck, gather and process
resources, recruit villagers, build a settlement, and survive server-authoritative
personal crises while the shared environment continues to advance.

## Current runtime

The checked-in source currently implements:

- one shared Bevy ECS world running at ten game ticks per second;
- a 60×50 hex map loaded from `sp_server/map/test3.tmx`, with five reusable
  player start locations from `sp_server/templates/player_start.yaml`;
- a global day/night and weather cycle with visibility, lighting, needs, and
  night-travel consequences;
- class-based heroes, inventories, equipment, combat, death, sanctuary
  resurrection, True Death, scoring, and replay from a recycled start;
- harvesting, mining, farming, fishing, hunting, refining, smelting, tanning,
  food processing, recipes, crafting, work queues, villager AI, structure
  assignments, merchants, and trade;
- an event-driven opening built around the run-owned Shipwreck, a player-built
  Burrow, a one-to-three Giant Rat wave, and the rescued first villager;
- ordered personal Goblin and Undead crises. Pre-assault progress is
  owner-online-only; a launched assault remains committed during an ordinary
  disconnect; and
- explicit Safe Logout. Protection starts only after the server accepts and
  completes the countdown, freezes the owning run, and ends before simulation
  resumes on reconnect.

The source constants and active TMX file are 60×50. Older planning text that
mentions a 50×50 runtime map does not describe the current checkout.

Accounts, sessions, recovery tokens, device tokens, and score rows use
PostgreSQL. The game itself lives in the shared server process. Some runtime
state can be snapshotted, but the current milestone documents deliberately do
not claim complete process-restart persistence for every per-run registry,
introduction fact, crisis identity, or Safe Logout record.

## Repository layout

| Path | Current responsibility |
| --- | --- |
| `sp_server/` | Rust 2021, Bevy 0.17 authoritative game server, TLS WebSocket protocol, templates, map data, headless harness, and simulation runners |
| `sp_frontend/sp_ts/` | React 18, TypeScript, and Phaser 3 browser client with separate desktop and mobile entry points |
| `sp_frontend/priv/static/` | Frontend source art and static data used by the bundle/copy flow |
| `sp_axum/` | Axum HTTPS application for static files, account/session endpoints, password recovery, scores, and combined health checks |
| `sp_axum/root/` | Files served by Axum, including copied desktop/mobile bundles |
| `sp_agent/` | Python WebSocket automation clients retained for external play experiments |
| `docs/` | Current milestone contracts, proposals, and historical balance evidence |

The game server is authoritative. Browser state presents protocol snapshots and
sends commands; it does not own gameplay transitions.

## Development checks

Run Rust checks from `sp_server/` because templates and other runtime files are
resolved relative to that crate:

```bash
cargo fmt --all -- --check
cargo check
cargo test --no-fail-fast
```

The in-process harness runs the production gameplay schedule without TLS,
PostgreSQL, or wall-clock sleeping:

```bash
cargo test --lib headless::tests::smoke
cargo run --bin headless_runner -- 1 6000 standard
```

The runner also accepts `safe-logout`, `safe-logout-matrix`, and
`goblin-balance` modes. See
[`sp_server/docs/headless_harness_plan.md`](sp_server/docs/headless_harness_plan.md)
for the current command contract.

Run frontend checks from `sp_frontend/sp_ts/`:

```bash
npm ci
./check-imports.sh
npx tsc --noEmit --skipLibCheck
npm run dev
```

`npm run dev` emits both development bundles. For a finite production build,
use `npx webpack --mode production --stats=errors-warnings`; the package's
historical `npm run build` command starts a production-mode development server
after bundling. `./copy.sh` copies already-built bundles and the deploy-flow
assets into `sp_axum/root/`.

`sp_axum` and the production game server require PostgreSQL and TLS environment
configuration. `db_init.sql` documents the combined database schema, while
`sp_axum/env-example` lists the web application's required environment names.
Deployment scripts and production credentials are intentionally outside the
normal documentation/test workflow.

## Design documentation

Start with [`docs/README.md`](docs/README.md). It distinguishes current runtime
contracts from parked proposals and historical experiment reports.

The primary current contracts are:

- [`docs/core_gameplay_polish_milestone.md`](docs/core_gameplay_polish_milestone.md)
  for the opening, objectives, Campfire lighting, and desktop tutorial;
- [`docs/persistent_crisis_milestone.md`](docs/persistent_crisis_milestone.md)
  and [`docs/undead_crisis_milestone.md`](docs/undead_crisis_milestone.md) for
  the personal-crisis sequence and assault lifecycle; and
- [`docs/safe_logout_milestone.md`](docs/safe_logout_milestone.md) for explicit
  logout protection and its desktop sanctuary-ward presentation.

Historical validation counts and balance outcomes remain records of the source
revision that produced them. They should not be read as a claim that the same
commands were rerun after every later change.
