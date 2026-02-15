# CLAUDE.md - Siege Perilous Frontend

## Project Overview

Siege Perilous is a browser-based multiplayer 2D tile-based strategy/survival game. This repository contains the frontend client built with **React 18 + TypeScript + Phaser 3**. The backend is a separate Erlang server communicating over WebSockets.

## Repository Structure

```
sp_frontend/
├── sp_ts/                        # Main TypeScript project root
│   ├── src/sp/                   # All application source code
│   │   ├── main.tsx              # React entry point (renders App)
│   │   ├── app.tsx               # Root App component with Redux Provider
│   │   ├── login.tsx             # LoginControl - manages login/signup flow
│   │   ├── game.tsx              # Phaser game initialization
│   │   ├── ui.tsx                # Main UI controller
│   │   ├── network.ts            # WebSocket communication with backend
│   │   ├── config.ts             # Game constants and configuration
│   │   ├── global.ts             # Global singleton state (primary state container)
│   │   ├── gameEvent.ts          # Game event definitions (56 events)
│   │   ├── networkEvent.ts       # Network event definitions (83 events)
│   │   ├── util.ts               # Hex geometry and utility functions
│   │   ├── scenes/               # Phaser scenes (mapScene, objectScene, weatherScene)
│   │   ├── objects/              # Game object classes (container, image, sprite, tile, resource)
│   │   ├── store/                # Redux store (minimal - login state only)
│   │   └── ui/                   # React UI components (95+ panel/component files)
│   ├── index.html                # HTML template for webpack
│   ├── package.json              # Dependencies and scripts
│   ├── tsconfig.json             # TypeScript config
│   └── webpack.config.js         # Webpack bundler config
├── priv/                         # Static assets served to browser
│   ├── static/
│   │   ├── art/                  # Game artwork (sprites, tilesets, UI images)
│   │   ├── sp2.js                # Webpack build output
│   │   └── ...
│   ├── index.html
│   └── desktop.html
└── README.md                     # Game design document
```

## Build Commands

All commands run from `sp_ts/` directory:

```bash
cd sp_ts
npm install          # Install dependencies
npm run dev          # Development build (webpack --mode development)
npm run build        # Production build + dev server
```

Output is bundled to `dist/sp2.js` (also copied to `priv/static/sp2.js`).

## Tech Stack

| Layer | Technology |
|-------|-----------|
| UI Framework | React 18.2 |
| Game Engine | Phaser 3.6.0 |
| Language | TypeScript 5.1.6 |
| State (Redux) | redux 4.0.1 (minimal usage, login state only) |
| State (Primary) | Global singleton class (`global.ts`) |
| Bundler | Webpack 5.88.2 |
| Styles | CSS Modules (`*.module.css`) |
| Communication | WebSockets to Erlang backend |

## Architecture & Key Patterns

### State Management

The app uses a **multi-layer state approach** (not idiomatic Redux):

1. **`Global` class** (`global.ts`) - Primary state container. Static properties hold game objects, tiles, weather, player info, UI state, and more. Most game state lives here.
2. **Redux store** (`store/`) - Minimal usage. Only tracks `isLoggedIn` via `LOGIN_ATTEMPT` action.
3. **React component state** - Individual panels manage their own local state.
4. **Phaser scene state** - Rendering state lives in Phaser scenes.

### Event System

Communication between systems uses Phaser EventEmitters accessed via `Global`:

- `Global.gameEmitter` - UI/game interaction events (defined in `gameEvent.ts`)
- `Global.uiEmitter` - UI panel events
- `GameEvent` namespace - 56 event constants for game actions
- `NetworkEvent` namespace - 83 event constants for server communication

### Navigation Flow

No React Router. Navigation is conditional rendering controlled by boolean flags in `LoginControl`:

1. Landing page (class selection / login)
2. Account setup (character creation)
3. Intro panel
4. Main game view (Phaser canvas + UI panels)

### Map System

- Hexagonal grid using **odd-q offset coordinates**
- Hex math utilities in `util.ts` (cube/offset conversion, distance, neighbors)
- Map rendering in `scenes/mapScene.ts`

### Network Protocol

- WebSocket connection managed in `network.ts`
- Messages are event-driven, dispatched through `NetworkEvent` constants
- Backend is Erlang (separate repository)

### Asset Aliases (Webpack)

Webpack resolves these path aliases for importing art assets:

- `ui` / `ui_comp` -> `priv/static/art/ui/`
- `art` / `art_comp` -> `priv/static/art/`

The `_comp` variants are for imports from deeper nested component directories.

## Code Conventions

- **TypeScript with relaxed settings**: `strict: false`, `noImplicitAny: false`
- **React components**: Functional components as `.tsx` files in `sp_ts/src/sp/ui/`
- **CSS**: CSS Modules pattern (`*.module.css`) imported per-component
- **Naming**: PascalCase for components/classes, camelCase for functions/variables
- **Events**: String constants defined in `gameEvent.ts` and `networkEvent.ts` namespaces
- **No testing framework active**: Jest is in dependencies but unconfigured (no tests exist)
- **No linter enforcement**: ESLint extends `react-app` but no strict rules or pre-commit hooks
- **No Prettier** configured

## Common UI Component Patterns

UI panels in `sp_ts/src/sp/ui/` follow a consistent pattern:

1. Listen for events via `Global.gameEmitter` or `Global.uiEmitter` in `useEffect`
2. Read data from `Global` static properties
3. Dispatch actions via emitter events or direct `Global.network` calls
4. Use CSS Modules for scoped styling

## Important Files for Context

| File | Purpose |
|------|---------|
| `global.ts` | Central state - read this first to understand game state shape |
| `gameEvent.ts` | All game event constants |
| `networkEvent.ts` | All network event constants |
| `config.ts` | Game configuration constants |
| `network.ts` | WebSocket protocol and server communication |
| `login.tsx` | Entry flow - login, class selection, character creation |
| `game.tsx` | Phaser game bootstrap |
| `scenes/mapScene.ts` | Hex map rendering logic |
| `ui.tsx` | UI panel orchestration |

## Development Notes

- The built JS output (`priv/static/sp2.js`) is ~3.6MB
- The project targets ES2016 with CommonJS modules
- Phaser is resolved to its minified build via webpack alias
- Source maps are available (configured in tsconfig but commented out in webpack)
- The game supports responsive layout with viewport scaling for different devices
