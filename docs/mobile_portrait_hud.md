# Mobile Portrait HUD

## Goal

Make the existing mobile client practical at 320–430 CSS pixels wide without
changing the map scale, game rules, or server authority.

## Layout

- A safe-area-aware status bar contains the hero portrait, HP, stamina, optional
  mana, needs, sanctuary state, day, and time.
- The active Survival objective is a single line below the status bar. Tapping
  it opens the existing full-screen Survival drawer.
- The lower-left thumb area contains a 104–116 pixel movement compass. Movement
  direction is calculated from the compass's rendered bounds, not source-image
  dimensions.
- Tile and object selection uses a horizontally scrolling tray. The selected
  object is kept beside the tile so it remains visible on narrow phones.
- Context actions share the same bottom row and scroll horizontally.
- General hero actions live in a labelled, collapsible drawer.
- Attack, defence, ability, and combat-information controls appear only while a
  living, perceived NPC is targeted or is the authoritative combat target.

## Behavioural parity

- Brace, Parry, and Dodge send their distinct server-authoritative defence
  values.
- Mobile shows attack history, combo guidance, enemy intent, counter guidance,
  and target effects.
- Safe Logout can be started or cancelled from the mobile Survival drawer using
  the same authoritative status packets and request locks as desktop.
- Small modal panels constrain themselves to the visual viewport and safe-area
  insets.

## Supported viewport checks

The layout policy is regression-tested at representative 320×568, 360×800,
390×844, 430×932, and landscape dimensions. Final device QA should still cover
touch ergonomics, browser address-bar collapse, and both notched and non-notched
iOS/Android devices.

## Deferred

- A visual device-matrix pass on real Safari and Chrome mobile browsers.
- Reworking legacy inventory-item positioning shared by older quantity dialogs.
- Bundle splitting; the existing desktop and mobile production bundles continue
  to exceed Webpack's advisory size threshold.
