# Siege Perilous — Drive-to-Done Prompt

The project doesn't lack mechanics — it lacks a finish line. Use the **kickoff prompt** once
to establish the definition of done, the cut list, and the milestone roadmap. After that,
use the **recurring execution prompt** day-to-day (great with `/loop`) to grind milestones
to completion without re-litigating scope each session.

---

## Kickoff prompt (run once to establish ROADMAP.md)

```
You are my co-developer on Siege Perilous, a persistent shared-world settlement
survival game (Rust/Bevy server in sp_server, Phaser/React/Redux TypeScript client
in sp_frontend/sp_ts). North-star: a learnable "prepare-and-survive" loop — each
player recovers from the Shipwreck, establishes a settlement, prepares for ordered
personal crises, and reaches victory or True Death while the global environment and
overlapping players remain shared. Score records how long and how well the run
survived. Start with README.md and docs/README.md, then verify every claim against
the current source.

My problem is NOT a lack of features. It's that this is an extremely ambitious project
and I need it to become a FINISHED, shippable game. Your job is to fight scope, not feed
it. Be ruthless. Push back on anything that doesn't move toward "done."

Do this in order, and STOP for my sign-off after each step:

1. TRACE THE LOOP. Read the actual code and trace one full player run end-to-end:
   new player → Shipwreck/opening → settlement → Goblin crisis → Undead crisis →
   later progression/victory OR True Death → score → leaderboard → replay. Tell me,
   grounded in real file:line references,
   where this loop is COMPLETE, where it's HALF-BUILT or stubbed, where it's BROKEN,
   and where it's only on the server with no client representation (or vice versa).
   Don't trust the docs or my memory — verify against current code.

2. DEFINE "DONE" — the smallest COMPLETE, satisfying v1.0. One sitting (~20-40 min),
   one clear arc: a player can start, learn the game, face escalating stages, reach a
   real ending (win by sealing the Monolith or die with a score), see their run ranked,
   and want to play again. Write this as an explicit, testable checklist in a new
   ROADMAP.md. If a system isn't required for that checklist, it is NOT in v1.

3. FOCUS. List everything currently in the codebase that is not needed on the
   critical v1 path. Preserve the shared-world architecture and existing resource,
   production, villager, crafting, farming, fishing, hunting, refining, and trade
   systems. Recommend hide-behind-flag, leave available but off the guided path, or
   keep-and-finish. Do not delete an established gameplay system without approval.

4. SEQUENCE. Turn the gap into a milestone roadmap in ROADMAP.md, ordered so the game
   is PLAYABLE END-TO-END as early as possible (vertical slice first: a rough but
   complete start→stages→ending→score loop), THEN balance/content, THEN polish. Each
   milestone must be independently shippable and verifiable by actually running the game,
   not just by tests passing. Keep milestones small (a few days of work each).

5. EXECUTE one milestone at a time. After each, RUN THE GAME (server + client) and
   confirm the loop still works and feels right — verification is "I played it and the
   arc holds," not "it compiles." Update ROADMAP.md (check off done, note new cuts).
   Surface balance/pacing problems you notice while playing.

Constraints: preserve the current shared map, environmental cycle, server authority,
solo-completable personal content, and disconnect/Safe Logout contracts. Prefer
finishing and tightening existing systems over adding new ones. When you see an
opportunity to add scope, propose parking it instead. Treat "a coherent session with a
real ending" as the bar, not exhaustive use of every existing system.

Start with step 1 only.
```

---

## Recurring execution prompt (use once ROADMAP.md exists — good for `/loop` or day-to-day sessions)

```
Read ROADMAP.md. Pick the next unchecked milestone. Implement it, run the game to verify
the full loop still holds, check it off, and stop.
```
