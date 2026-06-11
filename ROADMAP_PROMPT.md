# Siege Perilous — Drive-to-Done Prompt

The project doesn't lack mechanics — it lacks a finish line. Use the **kickoff prompt** once
to establish the definition of done, the cut list, and the milestone roadmap. After that,
use the **recurring execution prompt** day-to-day (great with `/loop`) to grind milestones
to completion without re-litigating scope each session.

---

## Kickoff prompt (run once to establish ROADMAP.md)

```
You are my co-developer on Siege Perilous, a single-player-per-world survival game
(Rust/Bevy server in sp_server, Phaser/React/Redux TS client in sp_frontend/sp_ts).
North-star: a "prepare-and-survive" loop — the world floods the player with escalating
waves; they prepare between stages; score = how long/well they survived. See
sp_server/CLAUDE.md and the survival-stages-direction memory.

My problem is NOT a lack of features. It's that this is an extremely ambitious project
and I need it to become a FINISHED, shippable game. Your job is to fight scope, not feed
it. Be ruthless. Push back on anything that doesn't move toward "done."

Do this in order, and STOP for my sign-off after each step:

1. TRACE THE LOOP. Read the actual code and trace one full player run end-to-end:
   new player → onboarding/intro → preparing → first crisis → escalating stages →
   legendary arc (Ashen Warlord) → win condition (seal the Monolith) OR death →
   score → leaderboard → replay. Tell me, grounded in real file:line references,
   where this loop is COMPLETE, where it's HALF-BUILT or stubbed, where it's BROKEN,
   and where it's only on the server with no client representation (or vice versa).
   Don't trust the docs or my memory — verify against current code.

2. DEFINE "DONE" — the smallest COMPLETE, satisfying v1.0. One sitting (~20-40 min),
   one clear arc: a player can start, learn the game, face escalating stages, reach a
   real ending (win by sealing the Monolith or die with a score), see their run ranked,
   and want to play again. Write this as an explicit, testable checklist in a new
   ROADMAP.md. If a system isn't required for that checklist, it is NOT in v1.

3. CUT. List everything currently in the codebase that is NOT needed for v1 — likely
   MMO-era leftovers and side systems (trade, experiment, farming, tax collector, multi-
   feature crafting, etc., whichever the trace shows aren't load-bearing). For each:
   recommend cut, hide-behind-flag, or keep-but-park. Default to cutting. I'll approve.

4. SEQUENCE. Turn the gap into a milestone roadmap in ROADMAP.md, ordered so the game
   is PLAYABLE END-TO-END as early as possible (vertical slice first: a rough but
   complete start→stages→ending→score loop), THEN balance/content, THEN polish. Each
   milestone must be independently shippable and verifiable by actually running the game,
   not just by tests passing. Keep milestones small (a few days of work each).

5. EXECUTE one milestone at a time. After each, RUN THE GAME (server + client) and
   confirm the loop still works and feels right — verification is "I played it and the
   arc holds," not "it compiles." Update ROADMAP.md (check off done, note new cuts).
   Surface balance/pacing problems you notice while playing.

Constraints: prefer finishing and tightening existing systems over adding new ones.
When you see an opportunity to add scope, propose a cut instead. If you're unsure whether
something belongs in v1, ask me — but bias toward "no." Treat "fun and finishable in one
sitting" as the bar, not "feature-complete."

Start with step 1 only.
```

---

## Recurring execution prompt (use once ROADMAP.md exists — good for `/loop` or day-to-day sessions)

```
Read ROADMAP.md. Pick the next unchecked milestone. Implement it, run the game to verify
the full loop still holds, check it off, and stop.
```
