# Goblin Crisis Balance — Checkpoint 2 Comparison

## Status and scope

Checkpoint 2, **Pressure and Phase Pacing**, is complete. It intentionally
changes only two phase-pressure thresholds and clarifies existing warning
semantics. It does not redesign the crisis authority, add gameplay systems, or
change the pressure formula, minimum online clocks, launch rules, assault,
combat, economy, Safe Logout, persistence, or legacy director.

The Checkpoint 1 baseline remains the historical pre-tuning record in
`docs/goblin_crisis_balance_baseline.md`. This report uses a larger, like-for-like
old-config control rather than comparing the candidate only with the original
36-row baseline.

## Decision summary

The old thresholds made the late crisis unreachable for bounded natural play:
0/54 repeated natural-control rows launched, 17/54 completed `Signs` and
entered `Pressure`, and none entered `Preparing`. The candidate preserves the
same fact-derived pressure and ordered online clocks but lowers the two late
fact gates:

```text
Settlement reaches pressure 45
        ↓ after at least 1,200 online ticks in Pressure
Preparing at pressure 45
        ↓ after at least 1,800 online ticks and pressure growth to 49
AssaultReady
        ↓ after at least 300 online ticks, preferring dusk/night,
          with a 1,200-online-tick maximum wait
AssaultActive
```

In the repeated candidate, 9/54 natural rows launched and all 63/63 staged
attainable-facts rows launched. Passive and basic-survival rows remained at
0/9 launches each. The intended pacing path is therefore reachable without
making online time alone sufficient.

This is a pacing success, not a combat-balance success. Candidate assaults
resolved in 21/72 launches (29.2%), including only 1/9 natural launches. Combat
and preparation effectiveness remain later-checkpoint work.

## Method and provenance

Both sides used the same 13 driver variants, three hero classes, three
repetitions per cell, and 20,000-tick cap:

* 117 rows per side: 54 natural progression and 63 staged attainable facts;
* 9 observations per scenario/cohort cell and 39 per hero class;
* passive, basic survival, prepared solo, fortified solo, no villagers,
  villager supported, ordinary disconnect, Safe Logout before assault, and
  helper supported;
* a real connected Warrior for helper rows, driven through ordinary `Move` and
  `Attack` events; and
* the same headless-only staged fixture described by Checkpoint 1. Natural and
  staged results are never pooled to infer organic launch probability.

The exact full-matrix commands were run from `sp_server/` after building the
corresponding old or new release binary:

```text
env CARGO_MANIFEST_DIR=/Users/peter/projects/sp/sp_server \
  ./target/release/headless_runner 117 20000 goblin-balance control
env CARGO_MANIFEST_DIR=/Users/peter/projects/sp/sp_server \
  ./target/release/headless_runner 117 20000 goblin-balance candidate
```

The frozen machine reports are:

* `sp_server/goblin_crisis_balance_checkpoint2_control_report.json`; and
* `sp_server/goblin_crisis_balance_checkpoint2_candidate_report.json`.

Both contain 117 quantitative rows and zero caught panics. The runner now
refuses to label a 45/49 binary as `control` or a 70/90 binary as `candidate`,
so the frozen control cannot be silently overwritten with mislabeled tuning.
Production systems still use `thread_rng`; these are independent repeated
samples, not paired seeded replays.

## Every changed balance value

| Value | Previous value | New value | Telemetry evidence | Expected player impact |
| --- | ---: | ---: | --- | --- |
| `Pressure` → `Preparing` threshold | 70 | 45 | The repeated control had 17/54 natural rows reach `Pressure`, but 0/54 reached `Preparing`. Natural latest pressure was mean 33.6, median 27; staged `Pressure` lasted mean 2,298.4 ticks, median 2,999, because rows waited for 70. | Reaching the existing `Pressure` gate can mature into a real preparation warning after the unchanged 1,200-online-tick phase minimum. The phase cannot be skipped or collapse to a few seconds. |
| `Preparing` → `AssaultReady` threshold | 90 | 49 | The control had 0/54 natural ready states or launches. Developed natural cohorts clustered far below 90: prepared-solo median 31, fortified median 31, no-villager median 29, and villager-supported median 50. A developed-solo control row reached 49. The deterministic path danger 10 + three structures 20 + online tier 15 + sanctuary level 2 × 2 = 49. | A player must retain a small additional settlement fact beyond the 45-point path before Ready. Passive and basic rows remain at pressure 25 and cannot launch, while deliberate structure/sanctuary or comparable existing growth can complete the path. |

No contributor weight changed. In particular, the change does not make
pressure incremental: it remains recomputed from current authoritative facts,
clamped, and deterministic.

### Presentation-only clarity changes

| Surface | Previous | New | Reason and expected impact |
| --- | --- | --- | --- |
| AssaultReady server summary | “The raiders are ready and may attack during darkness or when the preparation window expires.” | “The raiders are ready. After the minimum warning, they favor dusk or night but will not wait indefinitely.” | The 300-tick value is a minimum warning, not an attack ETA. The new text names both the preferred global window and bounded wait without changing either. |
| Desktop countdown label | `Preparation time` | `Minimum warning` | Avoids promising that the assault starts when the displayed value reaches zero. |
| Zero countdown | `0s` | `complete` | The minimum has completed; launch may still wait for dusk/night or the unchanged maximum wait. |

This is wording inside the existing panel, not a panel redesign. Warning
severity, packet structure, deduplication, and cadence are unchanged. Telemetry
can prove server queueing, not that a person rendered, read, or understood the
copy.

No pressure-bar, severity-label, or contributor UI was changed. The baseline
showed a reachability problem, not a human-confusion signal, and it did not
measure whether players could attribute a five-point change to a specific fact.
Adding contributor protocol/UI now would exceed the smallest evidence-backed
pacing change. The existing phase title, summary, and action hint remain the
explanation surface pending human-play evidence.

## Explicitly unchanged configuration

| Area | Unchanged values or behavior |
| --- | --- |
| Pressure cap and weights | cap 100; danger 10; three structures 20; living villager 15; `explore_poi` 10; `choose_expansion` 15 |
| Wealth, sanctuary, online time | gold thresholds 25/50/100 for 5/10/15; sanctuary 2 per level capped at 10; online tiers 600/1,800/3,600 for 5/10/15 |
| Early thresholds | `Dormant` → `Signs` 20; `Signs` → `Pressure` 45 |
| Online phase minima | Signs 600 ticks; Pressure 1,200; Preparing 1,800 |
| Ready launch timing | minimum grace 300 online ticks; maximum wait 1,200 online ticks; dusk/night preferred |
| Assault | two Wolf Riders and one Goblin Pillager; attribution, spawn rules, vision, stats, AI, combat, loot, rewards, and resolution unchanged |
| Lifecycle | owner-online pre-assault progression, reconnect watermarks, Offline Protection freeze, Safe Logout barrier, committed-assault continuation, True Death cleanup, and assault identity unchanged |
| World and modes | global day/night, weather, visibility, world-time packets, introduction/follow-ups, PersonalCrisis default, no personal dusk horde, and retained Legacy scheduling unchanged |
| Economy | resources, gathering, crafting, farming, refining, structures, inventories, work queues, villagers, professions, and trade unchanged |

## Old-versus-new outcome summary

“Win” means authoritative transition to `Resolved` after a launch. It does not
mean no hero death: hero-alive-at-resolution samples exclude unresolved
assaults, and the hero can die and return during a run.

| Metric | Old control | New candidate | Delta | Status | Interpretation / limitation |
| --- | ---: | ---: | ---: | --- | --- |
| Overall launch rate | 25/117 (21.4%) | 72/117 (61.5%) | +40.2 pp | improved | Combines natural and staged only as a total workload count. |
| Natural launch rate | 0/54 (0.0%) | 9/54 (16.7%) | +16.7 pp | improved | Establishes bounded natural reachability; it is not a population launch probability. |
| Staged launch rate | 25/63 (39.7%) | 63/63 (100.0%) | +60.3 pp | improved | Shows the old thresholds no longer strand the controlled attainable-facts probe. |
| Overall win rate | 11/25 (44.0%) | 21/72 (29.2%) | −14.8 pp | still problematic | Absolute resolutions rose from 11 to 21, but many more and different assaults entered the denominator. No combat value changed. |
| Natural win rate | n/a (0 launches) | 1/9 (11.1%) | newly measurable | still problematic | Pacing is reachable; natural combat success is not yet acceptable evidence of solo balance. |
| Staged win rate | 11/25 (44.0%) | 20/63 (31.7%) | −12.3 pp | still problematic | Independent RNG and exposure selection prevent a causal combat conclusion. |
| Hero alive at resolution | 11/11 (100%) | 21/21 (100%) | 0 pp | unchanged | Applies only to resolved rows and is not an all-launch survival rate. |
| Unresolved at tick cap | 81/117 (69.2%) | 67/117 (57.3%) | −11.9 pp | improved | Fourteen more rows completed or advanced beyond the previous censored state. |

## Phase duration comparison

The duration tables use global game ticks and include a value only when both
phase boundaries were observed. `n=0` means the phase never completed; it is
not a zero-duration phase. The gates themselves use online-active ticks, and
the online-only ranges and warning leads below verify those minima across
disconnect/protection divergence. Ten ticks equal one second.

### Natural progression

| Phase | Old mean / median / n | New mean / median / n | Status | Finding |
| --- | ---: | ---: | --- | --- |
| Dormant | 5,973.8 / 6,605 / 54 | 5,808.2 / 6,605 / 54 | unchanged | Intro and danger-unlock timing remain the dominant input. |
| Signs | 2,478.7 / 3,599 / 17 | 2,680.5 / 3,599 / 24 | unchanged | More rows completed Signs; the 600-tick minimum is unchanged. |
| Pressure | n=0 | 1,252.9 / 1,200 / 20 | improved | The phase is now observable and near its intended 1,200-tick minimum instead of stalling below 70. |
| Preparing | n=0 | 1,894.0 / 1,800 / 10 | improved | Ten natural rows completed a deliberate three-minute warning phase. |
| AssaultReady | n=0 | 839.0 / 790 / 9 | improved | All launched natural rows had more than the 300-tick minimum; observed range was 355–1,200. |
| AssaultActive | n=0 | 7,241 / 7,241 / 1 | still problematic | Only one natural resolution exists; combat duration confidence is very low. |

Candidate natural completed-phase online-active ranges were Signs 600–3,599 ticks,
Pressure 1,200–1,200, Preparing 1,800–2,235, and AssaultReady 355–1,200.
There were no skipped or seconds-long late phases.

### Staged attainable facts

| Phase | Old mean / median / n | New mean / median / n | Delta in mean | Status | Finding |
| --- | ---: | ---: | ---: | --- | --- |
| Dormant | 7 / 7 / 63 | 7 / 7 / 63 | 0 | unchanged | Fixture initialization only. |
| Signs | 600 / 600 / 63 | 600 / 600 / 63 | 0 | unchanged | Exact existing minimum. |
| Pressure | 2,298.4 / 2,999 / 63 | 1,200 / 1,200 / 63 | −1,098.4 | improved | Removes the old 70-point wait while retaining the full two-minute phase. |
| Preparing | 1,800 / 1,800 / 25 | 1,800 / 1,800 / 63 | 0 | unchanged | Duration remains exact; completion expands from 25 to all 63 rows. |
| AssaultReady | 1,013 / 1,200 / 25 | 1,223.9 / 1,200 / 63 | +210.9 | unchanged | Grace/window mechanics are unchanged. Disconnect/protection can make global time exceed online wait. |
| AssaultActive | 7,225.5 / 5,344 / 11 | 6,658.8 / 5,484 / 20 | −566.7 | still problematic | Assault timing is combat outcome data, not evidence for a pacing constant change. |

All candidate staged rows spent exactly 600/1,200/1,800 ticks in the three
ordered pre-ready phases. Their Ready range was 1,130–1,200 online ticks because
their common phase alignment missed the preferred darkness window.

## Preparation time and warnings

Preparation time is defined here as online-active time from crisis creation to
launch, plus phase-warning-to-launch lead. It is not a claim that every tick was
spent crafting or at home.

| Metric | Old control | New candidate | Status | Interpretation |
| --- | ---: | ---: | --- | --- |
| Staged online time before launch | mean 4,896.7, median 4,801 (n=25) | mean 4,792.1, median 4,801 (n=63) | unchanged | −104.5 ticks (10.5 s); the same phase minima dominate. |
| Natural online time before launch | n=0 | mean 6,527, median 7,390 (n=9) | improved | Natural rows now have a measured 8.0–12.3 minute warning/progression path. |
| Staged preparation action observed | 41/63 (65.1%) | 41/63 (65.1%) | unchanged | Snapshot-derived actions and bot policy did not change. |
| Natural preparation action after warning | n=0 eligible | 9/20 (45.0%) | improved | Newly measurable, but action deltas do not prove good choices or effectiveness. |

| Warning | Old delivery and online lead | New delivery and online lead | Status |
| --- | ---: | ---: | --- |
| Natural Signs | 54/54 delivered; no launch lead | 54/54; mean 6,526, median 7,389 to launch (n=9) | improved |
| Natural Preparing | phase not reached | 20/20; mean 2,726.7, median 2,590 to launch (n=9) | improved |
| Natural AssaultReady | phase not reached | 10/10; mean 839, median 790 to launch (n=9) | improved |
| Staged Signs | 63/63; mean 4,895.7, median 4,800 (n=25) | 63/63; mean 4,791.1, median 4,800 (n=63) | unchanged |
| Staged Preparing | 63/63; mean 2,802.7, median 3,000 (n=25) | 63/63; mean 2,991.1, median 3,000 (n=63) | unchanged |
| Staged AssaultReady | 25/25; mean 1,002.7, median 1,200 (n=25) | 63/63; mean 1,191.1, median 1,200 (n=63) | unchanged |

No warning cadence or severity was increased, so reachability does not add
per-tick spam. These delivery counts mean that the server successfully queued
the structured phase status; client rendering and human comprehension remain
unmeasured.

## Pressure growth

The telemetry proxy is the latest raw/clamped contributor snapshot per row,
plus observed phase reach. It is not a derivative or an accumulating score.

| Cohort | Old raw pressure mean / median / n | New raw pressure mean / median / n | Status | Interpretation |
| --- | ---: | ---: | --- | --- |
| Natural | 33.59 / 27 / 54 | 35.44 / 30 / 54 | unchanged | Independent worlds produced somewhat more completed structures in the candidate; the formula is byte-for-byte unchanged. |
| Staged | 80.48 / 73 / 63 | 80.21 / 71 / 63 | unchanged | The fixture and weights are unchanged; small state differences are random/bot outcomes. |

Natural dominant contributors changed from online time in 39 rows and
structures in 15 to online time in 33, structures in 20, and danger unlock in
1. No single contributor demonstrated complete domination, so no weight was
changed. The meaningful delta is threshold reach: old natural rows reached
Pressure/Preparing/Ready/launch in 17/0/0/0 cases; candidate rows did so in
24/20/10/9 cases.

Pressure and Preparing now share the fact threshold 45. They remain distinct:
Pressure must last at least 1,200 online ticks, Preparing changes the warning
and action guidance, Preparing must last at least 1,800 online ticks, and Ready
still requires pressure 49. This time-plus-further-fact design is intentional
and covered by exact boundary/no-skip tests.

## Damage and loss comparison

These values are per launched assault. Ordinary personal attackers primarily
target owner units and blocking walls, not ordinary buildings, so “structure
damage” is mostly wall damage.

| Metric | Old staged | New staged | Status | Interpretation |
| --- | ---: | ---: | --- | --- |
| Hero damage | mean 1,539.3, median 1,447 (n=25) | mean 1,202.0, median 691 (n=63) | unchanged | Directionally lower, but combat was not tuned and the launch population changed. |
| Hero deaths | mean 1.12, median 1 (n=25) | mean 0.97, median 1 (n=63) | unchanged | A typical launched row still records one assault-time death. |
| Structure damage | mean 3.36, median 0 (n=25) | mean 1.38, median 0 (n=63) | unchanged | Most rows take no wall damage; no defence or target rule changed. |
| Structures/walls destroyed | mean 0.16 / 0.16 (n=25) | mean 0.06 / 0.06 (n=63) | unchanged | Same target boundary and zero median. |
| Villager deaths | mean 0, median 0 (n=25) | mean 0, median 0 (n=63) | unchanged | Does not prove villager benefit; many villagers are not combat-capable. |

The nine natural candidate launches recorded mean hero damage 549, mean hero
deaths 0.22, zero villager deaths, and zero structure damage. Those small
samples must not be compared causally with an old `n=0` cohort.

## Required scenario comparison

Each cell contains nine rows. Rates are launch per row and resolution per
launch. Scenario labels describe the driver policy; staged labels do not prove
organic preparation.

| Scenario / cohort | Old launch; win | New launch; win | Status |
| --- | ---: | ---: | --- |
| Passive / natural | 0/9; n/a | 0/9; n/a | unchanged |
| Basic survival / natural | 0/9; n/a | 0/9; n/a | unchanged |
| Prepared solo / natural | 0/9; n/a | 2/9; 0/2 | improved reach; still problematic outcome |
| Prepared solo / staged | 4/9; 3/4 | 9/9; 1/9 | improved reach; still problematic outcome |
| Fortified solo / natural | 0/9; n/a | 2/9; 0/2 | improved reach; still problematic outcome |
| Fortified solo / staged | 3/9; 1/3 | 9/9; 3/9 | improved reach; unchanged outcome rate |
| No villagers / natural | 0/9; n/a | 2/9; 0/2 | improved reach; still problematic outcome |
| No villagers / staged | 3/9; 0/3 | 9/9; 1/9 | improved reach; still problematic outcome |
| Villager supported / natural | 0/9; n/a | 3/9; 1/3 | improved reach; still problematic sample size |
| Villager supported / staged | 4/9; 2/4 | 9/9; 4/9 | improved reach; unchanged directional outcome |
| Ordinary disconnect / staged | 4/9; 1/4 | 9/9; 4/9 | improved reach; unchanged lifecycle |
| Safe Logout before assault / staged | 3/9; 2/3 | 9/9; 2/9 | improved reach; still problematic outcome comparison |
| Helper supported / staged | 4/9; 2/4 | 9/9; 5/9 | improved reach; still problematic helper evidence |

The candidate's nine ordinary-disconnect assaults all disconnected and
reconnected during `AssaultActive`; 4/9 resolved and none resolved while the
owner was offline. The candidate issued 9 Safe Logout requests: 9 accepted, 8
completed and resumed, and 1 cancelled for movement, with zero invariant
recovery. The control had only one completed pre-launch Safe Logout lifecycle
sample because the old thresholds often prevented the scenario from reaching
its trigger. These are lifecycle probes, not evidence of a disconnect or
protection advantage.

The helper is real, but participation remained sparse: 0/9 control helper rows
and 1/9 candidate helper rows recorded helper participation; that one helper
made one attributed kill. Personal attackers correctly cannot select the
non-owner helper as a target. The 5/9 helper-scenario resolution rate therefore
cannot be attributed to help and does not show that multiplayer trivializes the
assault.

## Hero class review

No class stat, item, ability, or equipment value changed.

| Class / cohort | Old launch; win | New launch; win | Status |
| --- | ---: | ---: | --- |
| Warrior / natural (n=18) | 0/18; n/a | 3/18; 0/3 | improved reach; still problematic outcome |
| Ranger / natural (n=18) | 0/18; n/a | 4/18; 1/4 | improved reach; still problematic sample size |
| Mage / natural (n=18) | 0/18; n/a | 2/18; 0/2 | improved reach; still problematic outcome |
| Warrior / staged (n=21) | 9/21; 4/9 | 21/21; 7/21 | improved reach; still problematic outcome |
| Ranger / staged (n=21) | 10/21; 6/10 | 21/21; 10/21 | improved reach; still problematic confidence |
| Mage / staged (n=21) | 6/21; 1/6 | 21/21; 3/21 | improved reach; still problematic outcome |

Mage staged resolution (3/21) remains directionally below Ranger (10/21), but
the bot is predominantly melee-oriented and the run count is small. Tiny class
adjustments are not justified by this pacing experiment. Class/combat work
remains Checkpoint 4.

## Safety invariants

| Invariant | Old control | New candidate | Status |
| --- | ---: | ---: | --- |
| Automatic dusk hordes in PersonalCrisis | 0 | 0 | unchanged |
| Duplicate assault launches | 0 | 0 | unchanged |
| Cross-player target violations | 0 | 0 | unchanged |
| Crisis invariant failures | 0 | 0 | unchanged |
| Safe Logout invariant recoveries | 0 | 0 | unchanged |
| Caught panics in full matrix | 0 | 0 | unchanged |

Focused regressions additionally cover ordered one-step transitions, each
minimum-time boundary, deterministic pressure, runtime telemetry equality,
reconnect watermarks, Safe Logout's reconnect barrier, attributed assault
identity, no personal dusk horde, and retained Legacy scheduling.

## Validation record

Commands below were executed from `sp_server/` unless another directory is
shown.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed. |
| `cargo check` | Passed. The existing Rust warning backlog remained (70 library warnings). |
| `cargo test --quiet` | Passed: 429 library tests, 16 runner tests, and 6 day-system integration tests; 0 failed. The documentation test remained ignored. |
| `cargo clippy --all-targets --all-features` | Passed with exit 0. Existing backlog: 1,333 library warnings, 1,349 library-test warnings including duplicates, 1 runner warning, and 3 runner-test warnings including a duplicate. |
| `cargo test goblin_balance_checkpoint2 --lib` | Passed 2/2: exact changed/unchanged values and deterministic ordered no-skip growth path. |
| `cargo test goblin_phase --lib` | Passed 3/3, including min−1/exact time boundaries. |
| `cargo test goblin_pressure --lib` | Passed 3/3: deterministic categories, cap, and no double count. |
| `cargo test crisis_balance::tests --lib` | Passed 12/12. |
| `cargo test personal_crisis_initialization_and_timing_require_a_live_online_human_run --lib` | Passed 1/1, including runtime pressure = telemetry breakdown and reconnect watermark behavior. |
| `cargo test checkpoint4_normal_packet_progression_and_runtime_telemetry_headless --lib` | Passed 1/1; ordered status progression remained complete. |
| `cargo test checkpoint3_ready_clock_pauses_offline_and_resumes_on_reconnect --lib` | Passed 1/1. |
| `cargo test safe_logout_checkpoint2_assault_ready_cannot_launch_until_after_reconnect_barrier --lib` | Passed 1/1. |
| `cargo test checkpoint3_normal_victory_headless --lib` | Passed 1/1; assault composition, owner, ID, generation, and resolution identity remained intact. |
| `cargo test personal_crisis_mode_does_not_spawn_a_scheduled_dusk_horde --lib` | Passed 1/1. |
| `cargo test legacy_mode_still_runs_the_scheduled_dusk_horde --lib` | Passed 1/1. |
| `cargo test checkpoint3_legacy_mode_does_not_run_the_personal_assault_lifecycle --lib` | Passed 1/1. |
| `cargo test --bin headless_runner` | Passed 16/16, including 39-cell matrix coverage, report round trip, schema, helper, and fail-closed comparison-side coverage. |
| `cargo run --release --bin headless_runner -- 1 1000 standard` | Passed: 1 quantitative row, 0 panics, 0 automatic dusk waves, and 0 invariant failures. |
| `cargo run --release --bin headless_runner -- 8 2000 safe-logout-matrix` | Passed all 8 scenario variants with 0 panics, 0 crisis/Safe Logout invariant failures, and the expected completion, cancellation, long protection, reconnect, ordinary disconnect, active-assault rejection/continuation, and multiplayer paths. |
| Old full matrix command shown under Method | Passed: 117/117 quantitative, 0 panics, 54 natural + 63 staged. |
| New full matrix command shown under Method | Passed: 117/117 quantitative, 0 panics, 54 natural + 63 staged. |
| `jq -s` config/sample/invariant audit of both reports | Passed: the only config differences are `preparing_threshold` and `assault_ready_threshold`; counts/classes/scenarios match and all reported invariants are zero. |

From `sp_frontend/sp_ts/`:

| Command | Result |
| --- | --- |
| `npx tsc --module commonjs --target es2020 --jsx react --esModuleInterop --skipLibCheck --sourceMap false --outDir /tmp/sp_cp2_crisis_countdown_validation src/phaser.d.ts src/sp/core/crisisStatus.test.ts src/sp/desktop/ui/objectivesPanel.crisis.test.tsx` | Passed. |
| `NODE_PATH=/Users/peter/projects/sp/sp_frontend/sp_ts/node_modules node -e "require('/tmp/sp_cp2_crisis_countdown_validation/core/crisisStatus.test.js'); require('/tmp/sp_cp2_crisis_countdown_validation/desktop/ui/objectivesPanel.crisis.test.js');"` | Passed both focused assertions. |
| `npx tsc --noEmit --skipLibCheck` | Passed. |

Two non-final preflight issues were not counted as passes. Running the release
binary directly without `CARGO_MANIFEST_DIR` failed because the existing map
loader expects that runtime path; both full matrices were rerun with the exact
environment shown above and passed. One shortened 21-row control preflight hit
the existing random `Cannot find item template: "Windstride Stag"` gather-path
panic; neither 117-row final matrix reproduced it. An initial isolated frontend
compile omitted `src/phaser.d.ts` and failed on the generated global `integer`
type; the corrected focused compile and full typecheck above passed.

## Confidence and limitations

* **Moderate confidence — pacing reachability.** There are 54 natural and 63
  staged rows per side, three repetitions per exact matrix cell, exact phase
  boundary tests, and a large 0/54 to 9/54 natural reachability change.
* **Low confidence — combat, class, villagers, walls, and Safe Logout outcome
  parity.** Conditional launch populations differ, individual scenario cells
  have nine rows, the bot is melee-biased, and no combat value changed.
* **Very low confidence — helper effect.** Only one candidate helper actually
  participated. Adjacent-settlement balance remains omitted; focused ownership
  and target-isolation regressions remain its evidence.
* **Low confidence — human warning comprehension.** Packet queueing and lead
  time are measured; rendering, notice, interpretation, and player decisions
  are not.
* Production randomness is unseeded. Control and candidate are equivalent
  workloads but not paired worlds.
* Both aggregate reports identify base commit `3fa1b9a` and a dirty working
  tree. Their serialized configs prove that only the two thresholds differ,
  but no old/new source patch or binary hash was captured. The ignored raw
  `headless_runs.json` is overwritten by later runner modes; raw-only facts in
  this report (phase min/max ranges, exact Safe Logout attempt totals, the one
  helper kill, and the observed 49-point control row) come from the
  contemporaneous row/stdout audit and are not reconstructible from the two
  committed aggregate JSON files. Future comparisons should version raw rows
  or record binary/source hashes.
* Tick-cap rows remain censored: old/new cap reaches were 92/117 and 86/117;
  old/new unresolved-at-cap counts were 81/117 and 67/117.
* Snapshot deltas observe preparation state changes, not intent, repaired HP,
  craft intent, or tactical quality. Near/away time remains 600-tick sampled.
* Staged facts relocate/level a monolith and supply existing facts/resources;
  they prove authoritative lifecycle attainability, not organic preparation.
* Runtime crisis and telemetry state remains process-memory state. No
  persistence or database work was added.

## Remaining concerns and recommended Checkpoint 3 focus

Checkpoint 3 should not retune these thresholds without new evidence. Its
preparation/defensive-value work should use existing systems and prioritize:

1. make the existing Preparing guidance actionable through observable return,
   equip, craft, repair, stock, wall, and defender choices;
2. measure time from warning to first return, repair, and crafted/equipped item
   only where inexpensive telemetry can distinguish those actions;
3. establish whether current walls, sanctuary upgrades, healing supplies, and
   armed villagers materially change outcomes without redesigning the economy;
4. preserve solo completion while measuring optional assistance with a helper
   driver that reliably reaches the fight; and
5. carry Mage/Ranger/Warrior assault difficulty, wave stats, and final combat
   tuning to Checkpoint 4 rather than changing them during preparation work.

Checkpoint 2 deliberately does **not** implement preparation gameplay, change
the assault composition, add enemies/objectives/resources/buildings, alter
villager AI, loot, rewards, Safe Logout, persistence, regional crises, or any
new crisis type.
