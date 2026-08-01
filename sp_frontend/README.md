# Siege Perilous Frontend

Browser client for Siege Perilous, built with React, TypeScript, and Phaser.

## Current architecture

- `sp_ts/` is the TypeScript project root.
- `sp_ts/src/sp/core/` contains the WebSocket client, shared packet types,
  event-driven client state, Phaser scenes, and presentation helpers.
- `sp_ts/src/sp/desktop/` and `sp_ts/src/sp/mobile/` contain separate React UI
  shells and entry points. Core cannot import either shell, and the shells
  cannot import one another.
- `priv/static/art/` and `priv/static/tileset.json` are frontend source assets.
- `docs/design-notes/` is historical product material, not a current runtime
  contract.

The Rust game server remains authoritative. `network.ts` replaces or updates
the shared object/terrain/weather state from protocol packets and emits typed
events to the React and Phaser layers.

Both clients share map, weather, object, speech, visibility, and ordinary
burning-object rendering. The current desktop shell additionally provides:

- Tutorial & Help over the server's `objective_state` packet, including a
  per-player browser preference and deferred hints;
- personal-crisis and Safe Logout cards;
- a protected-settlement sanctuary ward and read-only protected-object labels;
- the target action for lighting a completed standalone Campfire; and
- the scaled animated flame used for a lit Campfire.

The mobile shell continues to use the shared `ObjectScene` and its own existing
UI tree. It does not register the desktop sanctuary-ward subclass or the
desktop-only tutorial and Campfire affordances.

## Serving model

The Axum application in `../sp_axum` serves `../sp_axum/root`. The browser
chooses the mobile bundle at widths up to 1024 pixels (including the explicit
iPad Pro case) and the desktop bundle otherwise. The Rust game server in
`../sp_server` owns the TLS WebSocket protocol.

Frontend builds produce separate desktop and mobile bundles:

- `sp_ts/dist/sp2.desktop.js`
- `sp_ts/dist/sp2.mobile.js`

`sp_ts/copy.sh` checks cross-shell imports, copies those bundles into
`../sp_axum/root/`, and copies the small set of assets used by the current
deploy flow.

## Rebuild the UX

From anywhere in the repository, run:

```bash
./sp_frontend/rebuild-ux.sh
```

This single command runs the frontend import and TypeScript checks, creates
finite production desktop and mobile bundles, invokes the established copy
flow, and verifies that the bundles in `sp_axum/root/` match the build output.
It does not start a development server or broadly mirror the static-art tree.

If dependencies have not been installed yet, run `npm ci` once from
`sp_frontend/sp_ts/`.

## Development

Run commands from `sp_ts/`:

```bash
npm ci
./check-imports.sh
npx tsc --noEmit --skipLibCheck
npm run dev
```

`npm run dev` creates both Webpack development bundles and exits. For a finite
production build, use:

```bash
npm run build
./copy.sh
```

`npm run rebuild:ux` provides the same checked build-and-copy workflow when
already working inside `sp_ts/`. Use `npm run serve` when an interactive
development server is wanted.
`./check-imports.sh` enforces the desktop/mobile/core import boundaries.

Plain `npx tsc --noEmit` is still blocked by the repository's generated
`src/phaser.d.ts` colliding with Phaser's package declarations and referencing
a missing local Matter declaration. `--skipLibCheck` and production Webpack
builds are the supported whole-client checks recorded by the current
milestones.
