# Combat combo milestone

## Goal

Make existing player-only attack chains reliable, faster as they develop, and
readable in combat without changing the seven combo recipes, stance counters,
stamina costs, per-target ownership, or adjacent finisher requirement.

## Implemented checkpoint

- Timed combat effects use an authoritative expiry tick. Reapplication refreshes
  expiry, stackable effects cap at five, and stale expiry events are harmless.
- Bleed deals one HP per second for 20 seconds and performs normal authoritative
  death cleanup. Offline-protected runs do not advance DoT damage.
- Basic attacks, primary finishers, and secondary finisher damage share one
  physical-damage calculation, including weapon skill and Expose Armor stacks.
- Combo histories time out after 150 ticks and recover to their longest live
  suffix after a dead end.
- Valid chain lengths use 50/40/30/25-tick basic-attack cooldowns. A ready
  finisher remains immediate and starts a fresh 50-tick cooldown afterward.
- Stunned, Fear, Concussed, and Hamstrung use per-target, per-effect
  100%/50%/25%/immune diminishing returns, resetting after 150 ticks.
- Intimidating Shout, Shatter Cleave, and Massive Pummel apply their specified
  adjacent-NPC secondary behavior while excluding protected, dead, fortified,
  non-unit, and player-owned targets.
- Combat State v2 exposes target effects. Desktop and mobile show live/next
  chain pips, target-effect badges, ready-finisher pulse, dynamic cooldowns,
  and distinct finisher popup/shake feedback.

## Architecture and risks

The server remains authoritative. `combat.rs` owns damage, effect application,
DR, and AoE target rules; `player.rs` owns input validation, cooldown state,
combo discovery, and Combat State packets; `game.rs` owns timed ticks and effect
expiry. Secondary AoE targets are collected read-only and mutated in a second
pass to respect Bevy query borrowing and deferred despawn behavior.

The DR and discovery registries are runtime state, matching the current runtime
combo tracker. They are intentionally not a new persistence subsystem. AoE
events are emitted per secondary target, so clients can present every sweep hit.

## Verification contract

The checkpoint is covered by focused Rust tests for combo matching and suffixes,
timeout, expiry and stacking, Bleed ticking/death, DR, cooldown tempo, secondary
selection, shared damage, and immediate finishers. Completion also requires the
repository server format/check/Clippy/test commands, the bounded 6,000-tick
headless run, and frontend import/type/production-build checks.
