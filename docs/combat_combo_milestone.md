# Combat combo milestone

## Goal

Make existing player-only attack chains reliable, faster as they develop, and
readable in combat without changing the seven combo recipes, stance counters,
stamina costs, or adjacent finisher requirement. Partial chains remain
target-owned; a completed finisher becomes target-transferable only when its
final setup attack kills that target.

## Implemented checkpoint

- Timed combat effects use an authoritative expiry tick. Reapplication refreshes
  expiry, stackable effects cap at five, and stale expiry events are harmless.
- Bleed deals one HP per second for 20 seconds and performs normal authoritative
  death cleanup. Offline-protected runs do not advance DoT damage.
- Basic attacks, primary finishers, and secondary finisher damage share one
  physical-damage calculation, including weapon skill and Expose Armor stacks.
- Combo histories time out after 150 ticks and recover to their longest live
  suffix after a dead end. Finisher input performs the same timeout check
  directly, so a stale tracker cannot execute during a system-order gap.
- A finisher completed by a killing blow remains ready for the next valid
  target until the normal combo timeout. Partial chains and completed chains
  against living targets remain bound to their original target.
- Strict combo prefixes use the 30/25/20/15-tick basic-attack cooldown ladder.
  Exact completed recipes stay live for finisher hints but reset basic attacks
  to 30 ticks; suffix recovery cannot turn repeated completed inputs into a
  tempo exploit. With the current four-attack maximum, the 15-tick rung is
  reserved for a future five-or-more-attack recipe.
- Stunned, Fear, Concussed, and Hamstrung use per-target, per-effect
  100%/50%/25%/immune diminishing returns, resetting 150 ticks after the last
  successfully applied effect. Immune attempts do not refresh that timestamp.
- Intimidating Shout, Shatter Cleave, and Massive Pummel apply their specified
  adjacent-NPC secondary behavior while excluding protected, dead, fortified,
  non-unit, and player-owned targets.
- Combat State v3 is emitted only from the owning hero's combo tracker; villager
  trackers still expire and drive combat internally without overwriting hero UI.
  Desktop and mobile show live/next
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
timeout (including stale finisher input), hero-only Combat State emission,
expiry and stacking, Bleed ticking/death, DR, strict-prefix cooldown tempo,
secondary selection, shared damage, and immediate finishers. Completion also
requires the repository server format/check/Clippy/test commands and the bounded
6,000-tick headless run. This fix pass does not change desktop or mobile UX.
