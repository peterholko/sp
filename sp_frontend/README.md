# Siege Perilous Frontend

Browser client for Siege Perilous, built with React, TypeScript, and Phaser.

## Current Layout

- `sp_ts/` is the TypeScript project root.
- `sp_ts/src/sp/core/` contains shared game, network, state, and Phaser scene code.
- `sp_ts/src/sp/desktop/` and `sp_ts/src/sp/mobile/` contain the two UI shells.
- `priv/static/art/` and `priv/static/tileset.json` are source static assets used by the frontend build and runtime asset loading.
- `docs/design-notes/` keeps old design notes that are useful as product history but are not active source.

## Serving Model

The active site is served by the Rust Axum app in `../sp_axum`, which serves files from `../sp_axum/root`. The Rust game server in `../sp_server` owns the game WebSocket protocol.

Frontend builds produce separate desktop and mobile bundles:

- `sp_ts/dist/sp2.desktop.js`
- `sp_ts/dist/sp2.mobile.js`

`sp_ts/copy.sh` copies those bundles into `../sp_axum/root/` and copies the small set of static assets needed by the deploy flow.

## Development

Run commands from `sp_ts/`:

```bash
npm ci
./check-imports.sh
npm run dev
./copy.sh
```

`npm run dev` creates webpack development bundles. `./check-imports.sh` enforces the desktop/mobile/core import boundaries.
