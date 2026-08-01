# CLAUDE.md - Siege Perilous Frontend

## Project Overview

Siege Perilous is a browser-based multiplayer 2D tile strategy/survival game. This directory contains the active frontend client built with React 18, TypeScript, and Phaser 3.

The current backend/services are Rust:

- `../sp_server` owns the game simulation and WebSocket protocol.
- `../sp_axum` serves the browser app from `../sp_axum/root` and handles HTTP account/session endpoints.

No Erlang, OTP, or rebar source is expected under `sp_frontend`.

## Repository Structure

```text
sp_frontend/
├── sp_ts/                    # TypeScript project root
│   ├── src/sp/core/          # Shared state, network, Phaser scenes, game objects
│   ├── src/sp/desktop/       # Desktop React shell and panels
│   ├── src/sp/mobile/        # Mobile React shell and panels
│   ├── index.html            # Webpack HTML template
│   ├── package.json          # Dependencies and scripts
│   ├── tsconfig.json         # TypeScript config
│   └── webpack.config.js     # Desktop/mobile bundle config
├── priv/static/art/          # Source art and UI assets
├── priv/static/tileset.json  # Runtime tile metadata loaded by Phaser
├── docs/design-notes/        # Historical design notes
└── README.md
```

## Build And Deploy Flow

Run frontend commands from `sp_ts/`:

```bash
npm ci
npm run rebuild:ux
```

For the same checked production build-and-copy flow from the repository root,
run `./sp_frontend/rebuild-ux.sh`. It verifies that both copied bundles match
the Webpack output. `npm run serve` starts the interactive development server.

`npm run dev` runs webpack in development mode and writes:

- `dist/sp2.desktop.js`
- `dist/sp2.mobile.js`

`copy.sh` copies those bundles into `../sp_axum/root/` as `/sp2.desktop.js` and `/sp2.mobile.js`. It also copies the deploy-required static art subset into `../sp_axum/root/static/art/` and selected JSON sprite metadata into `../sp_server/tileset/`.

## Tech Stack

| Layer | Technology |
| --- | --- |
| UI Framework | React 18 |
| Game Engine | Phaser 3 |
| Language | TypeScript |
| State (Redux) | redux, minimal login state |
| State (Primary) | `Global` singleton in `core/global.ts` |
| Bundler | Webpack 5 |
| Styles | CSS modules and regular CSS |
| Communication | WebSocket messages handled in `core/network.ts` |

## Architecture Notes

- `core/global.ts` is the main state holder for game objects, tiles, weather, player info, UI state, emitters, and the network instance.
- `core/network.ts` defines most outbound packet helpers and inbound packet handling.
- `core/gameEvent.ts` and `core/networkEvent.ts` define event names used through Phaser EventEmitters.
- Desktop and mobile code should depend on `core/`, but `core/` must not import either UI shell.
- `desktop/` must not import from `mobile/`, and `mobile/` must not import from `desktop/`; use `./check-imports.sh` to verify.
- Runtime assets are loaded from `/static/art/...` and `/static/tileset.json`; keep those URL shapes stable unless the serving model changes too.
- Both shells use shared authoritative perception and ordinary burning-object
  presentation. Tutorial & Help, protected-settlement wards, the public
  Campfire action, and the lit standalone-Campfire overlay are currently
  desktop-only.

## Important Files

| File | Purpose |
| --- | --- |
| `sp_ts/src/sp/core/global.ts` | Central shared state |
| `sp_ts/src/sp/core/network.ts` | WebSocket protocol and server communication |
| `sp_ts/src/sp/core/config.ts` | Game constants and viewport helpers |
| `sp_ts/src/sp/core/scenes/mapScene.ts` | Hex map rendering |
| `sp_ts/src/sp/core/scenes/objectScene.ts` | Object and sprite rendering |
| `sp_ts/src/sp/desktop/login.tsx` | Desktop entry/login flow |
| `sp_ts/src/sp/mobile/login.tsx` | Mobile entry/login flow |
| `sp_ts/webpack.config.js` | Bundle entries, output names, asset aliases |

## Conventions

- Prefer existing React/Phaser patterns and local helpers over new abstractions.
- Keep shared behavior in `core/`; keep layout and panel differences in `desktop/` or `mobile/`.
- Webpack aliases map `ui` and `ui_comp` to `priv/static/art/ui`, and `art` and `art_comp` to `priv/static/art`.
- TypeScript is intentionally relaxed (`strict: false`, `noImplicitAny: false`).
- Focused TypeScript regression scripts live beside the helpers/components they
  cover. Project-configured TypeScript emission can run the pure policy and
  presentation scripts directly with Node; React component scripts may also
  need the repository's asset/module stubs. Run `./check-imports.sh`,
  `npx tsc --noEmit --skipLibCheck`, a finite production Webpack build, and
  focused manual browser checks for presentation changes.

Plain `npx tsc --noEmit` remains blocked by collisions in the generated
`src/phaser.d.ts` and its missing local Matter declaration. `--skipLibCheck`
is the supported whole-project type check.
