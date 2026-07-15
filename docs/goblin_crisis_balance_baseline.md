# Goblin Crisis Balance Baseline

Checkpoint 1 measures the current goblin crisis and intentionally does not make material balance changes.

This report was generated from commit `3fa1b9a783cff4f5f634bdea873104ce5898c313` from a dirty working tree using report schema version 1. An assault "win" below means authoritative transition to `Resolved`; whether the hero was alive at resolution and the number of assault-time deaths are reported separately. Panics are retained in counts but excluded from quantitative means and rates. Overall tick-cap reaches and unresolved-at-cap rows remain visible and are not silently discarded; a resolved crisis may continue to the overall cap.

## Sample summary

- Runs: 36 total, 36 quantitative, 0 panics, 33 reached the overall tick cap, 28 remained crisis-unresolved at that cap.
- Progression cohorts: 18 natural-progression runs and 18 staged-attainable-facts runs.
- Scenario samples: `{"basic_survival":3,"fortified_solo":6,"no_villagers":6,"ordinary_disconnect":3,"passive":3,"prepared_solo":6,"safe_logout_before_assault":3,"villager_supported":6}`.
- Hero-class samples: `{"Mage":12,"Ranger":12,"Warrior":12}`.
- Tick caps: `{"20000":36}`.

## Exact current configuration

The snapshot below is serialized from the constants used by the authoritative crisis implementation; it is not a runtime tuning interface.

```json
{
  "pressure_max": 100,
  "danger_unlocked_pressure": 10,
  "three_structures_pressure": 20,
  "villager_pressure": 15,
  "explore_poi_pressure": 10,
  "choose_expansion_pressure": 15,
  "gold_tier_thresholds": [
    25,
    50,
    100
  ],
  "gold_pressure_per_tier": 5,
  "sanctuary_pressure_per_level": 2,
  "sanctuary_pressure_max": 10,
  "online_pressure_tier_ticks": [
    600,
    1800,
    3600
  ],
  "online_pressure_per_tier": 5,
  "signs_threshold": 20,
  "pressure_threshold": 45,
  "preparing_threshold": 70,
  "assault_ready_threshold": 90,
  "signs_min_online_ticks": 600,
  "pressure_min_online_ticks": 1200,
  "preparing_min_online_ticks": 1800,
  "assault_ready_grace_ticks": 300,
  "assault_max_online_wait_ticks": 1200,
  "preferred_launch_window": "dusk_or_night",
  "game_ticks_per_day": 2400,
  "preferred_launch_start_tick": 2000,
  "preferred_launch_wrap_end_tick": 400,
  "assault_composition": [
    "Wolf Rider",
    "Wolf Rider",
    "Goblin Pillager"
  ],
  "assault_vision": 14,
  "fallback_spawn_min_distance": 6,
  "fallback_spawn_max_distance": 8,
  "sanctuary_spawn_min_offset_from_weak_radius": 1,
  "sanctuary_spawn_max_offset_from_weak_radius": 3,
  "neighbouring_structure_exclusion_distance": 3,
  "spawn_candidate_limit": 96
}
```

The snapshot covers the crisis-owned constants. The following architecture-audited runtime values are also part of the current baseline and are intentionally unchanged. The personal wave overrides both attackers' viewsheds to 14.

| Assault unit | Count | HP | Stamina | Damage / span | Defence | Speed | Template vision | Personal-wave vision | Kill XP |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Wolf Rider | 2 | 75 | 250 | 6 / 5 | 5 | 6 | 4 | 14 | 300 |
| Goblin Pillager | 1 | 55 | 200 | 5 / 4 | 4 | 5 | 3 | 14 | 250 |

Human Villagers have 500 HP, 10,000 stamina, zero base damage/span, zero defence, zero speed, vision 2, and base work 25. They count as combat-capable only when current base damage is positive or a weapon is equipped.

| Existing defence | HP | Defence | Current role |
|---|---:|---:|---|
| Stockade | 20 | 0 | blocking level-0 wall |
| Palisade | 200 | 0 | blocking level-1 wall |
| Fieldstone Walls | 400 | 0 | blocking level-2 wall |
| Watchtower | 50 | 0 | vision/light support; not a wall |

The sanctuary maximum is level 5; upgrade costs are 3, 6, 9, 12, and 15 Soulshards; full and weak radii are `3 + level` and `5 + level`; each level contributes 0.25 to the existing defence amplifier. Full audit context, including anchor priority, target eligibility, equipment, and the runtime Health Potion/template discrepancy, is recorded in `docs/goblin_crisis_balance_milestone.md`.

## Hero-class starting baseline

These are architecture-confirmed starting values from the current hero templates and setup path; this checkpoint did not change them. Every class receives one custom 10-point Health Potion, Tattered Shirt and Tattered Pants.

| Class | HP | Stamina | Mana | Base damage / span | Defence | Speed | Vision | Starting weapon | Additional starting equipment |
|---|---:|---:|---:|---:|---:|---:|---:|---|---|
| Warrior | 110 | 110 | 0 | 2 / 2 | 4 | 5 | 3 | Sharpened Stick | Copper Helm (+3 defence) |
| Ranger | 80 | 120 | 0 | 1 / 3 | 1 | 7 | 5 | Training Bow (8 damage, range 2, 85 accuracy) | none beyond common clothing |
| Mage | 60 | 100 | 100 | 1 / 2 | 0 | 5 | 4 | Sharpened Stick | 5 Mana items |

## Aggregate results

Natural-progression rows observe the existing starting economy and bot path. `staged_attainable_facts` rows are separate assault probes: their headless-only fixture supplies attainable existing facts and resources, then leaves authoritative pressure, phase gates, launch, spawning, and combat unchanged. Staged rows are not evidence of the natural launch rate or of an organic preparation path.

### By scenario and progression cohort

| Group | Runs | Launch | Resolution after launch | Hero alive at resolution | Assault duration | Hero damage | Hero deaths | Structure damage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| basic_survival / natural_progression | 3 (3 quantitative) | 0.0% (0/3) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| fortified_solo / natural_progression | 3 (3 quantitative) | 0.0% (0/3) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| fortified_solo / staged_attainable_facts | 3 (3 quantitative) | 33.3% (1/3) | 100.0% (1/1) | 100.0% (1/1) | mean 11726.0, median 11726.0 (n=1) | mean 1229.0, median 1229.0 (n=1) | mean 1.0, median 1.0 (n=1) | mean 0.0, median 0.0 (n=1) |
| no_villagers / natural_progression | 3 (3 quantitative) | 0.0% (0/3) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| no_villagers / staged_attainable_facts | 3 (3 quantitative) | 66.7% (2/3) | 50.0% (1/2) | 100.0% (1/1) | mean 3472.0, median 3472.0 (n=1) | mean 1602.0, median 1602.0 (n=2) | mean 1.0, median 1.0 (n=2) | mean 0.0, median 0.0 (n=2) |
| ordinary_disconnect / staged_attainable_facts | 3 (3 quantitative) | 66.7% (2/3) | 50.0% (1/2) | 100.0% (1/1) | mean 3772.0, median 3772.0 (n=1) | mean 413.5, median 413.5 (n=2) | mean 0.0, median 0.0 (n=2) | mean 0.0, median 0.0 (n=2) |
| passive / natural_progression | 3 (3 quantitative) | 0.0% (0/3) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| prepared_solo / natural_progression | 3 (3 quantitative) | 0.0% (0/3) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| prepared_solo / staged_attainable_facts | 3 (3 quantitative) | 33.3% (1/3) | 0.0% (0/1) | n/a (0/0) | n/a (n=0) | mean 530.0, median 530.0 (n=1) | mean 1.0, median 1.0 (n=1) | mean 0.0, median 0.0 (n=1) |
| safe_logout_before_assault / staged_attainable_facts | 3 (3 quantitative) | 66.7% (2/3) | 50.0% (1/2) | 100.0% (1/1) | mean 3771.0, median 3771.0 (n=1) | mean 394.0, median 394.0 (n=2) | mean 0.0, median 0.0 (n=2) | mean 0.0, median 0.0 (n=2) |
| villager_supported / natural_progression | 3 (3 quantitative) | 0.0% (0/3) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| villager_supported / staged_attainable_facts | 3 (3 quantitative) | 66.7% (2/3) | 100.0% (2/2) | 100.0% (2/2) | mean 8592.0, median 8592.0 (n=2) | mean 1510.5, median 1510.5 (n=2) | mean 1.0, median 1.0 (n=2) | mean 0.0, median 0.0 (n=2) |

### Natural versus staged progression cohort

| Group | Runs | Launch | Resolution after launch | Hero alive at resolution | Assault duration | Hero damage | Hero deaths | Structure damage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| natural_progression | 18 (18 quantitative) | 0.0% (0/18) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| staged_attainable_facts | 18 (18 quantitative) | 55.6% (10/18) | 60.0% (6/10) | 100.0% (6/6) | mean 6654.2, median 4142.0 (n=6) | mean 959.9, median 865.0 (n=10) | mean 0.6, median 0.5 (n=10) | mean 0.0, median 0.0 (n=10) |

### By hero class and progression cohort

| Group | Runs | Launch | Resolution after launch | Hero alive at resolution | Assault duration | Hero damage | Hero deaths | Structure damage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Mage / natural_progression | 6 (6 quantitative) | 0.0% (0/6) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| Mage / staged_attainable_facts | 6 (6 quantitative) | 0.0% (0/6) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| Ranger / natural_progression | 6 (6 quantitative) | 0.0% (0/6) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| Ranger / staged_attainable_facts | 6 (6 quantitative) | 66.7% (4/6) | 100.0% (4/4) | 100.0% (4/4) | mean 3881.8, median 3771.5 (n=4) | mean 865.8, median 865.0 (n=4) | mean 0.2, median 0.0 (n=4) | mean 0.0, median 0.0 (n=4) |
| Warrior / natural_progression | 6 (6 quantitative) | 0.0% (0/6) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| Warrior / staged_attainable_facts | 6 (6 quantitative) | 100.0% (6/6) | 33.3% (2/6) | 100.0% (2/2) | mean 12199.0, median 12199.0 (n=2) | mean 1022.7, median 879.5 (n=6) | mean 0.8, median 1.0 (n=6) | mean 0.0, median 0.0 (n=6) |

### Observed preparation actions by progression cohort

| Group | Runs | Launch | Resolution after launch | Hero alive at resolution | Assault duration | Hero damage | Hero deaths | Structure damage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| no_observed_preparation_action / staged_attainable_facts | 7 (7 quantitative) | 85.7% (6/7) | 66.7% (4/6) | 100.0% (4/4) | mean 7910.2, median 7748.5 (n=4) | mean 1311.5, median 1087.0 (n=6) | mean 0.8, median 1.0 (n=6) | mean 0.0, median 0.0 (n=6) |
| observed_preparation_action / staged_attainable_facts | 11 (11 quantitative) | 36.4% (4/11) | 50.0% (2/4) | 100.0% (2/2) | mean 4142.0, median 4142.0 (n=2) | mean 432.5, median 413.5 (n=4) | mean 0.2, median 0.0 (n=4) | mean 0.0, median 0.0 (n=4) |
| preparing_not_observed / natural_progression | 18 (18 quantitative) | 0.0% (0/18) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |

### Prepared-policy versus unprepared-policy by progression cohort

| Group | Runs | Launch | Resolution after launch | Hero alive at resolution | Assault duration | Hero damage | Hero deaths | Structure damage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| prepared / natural_progression | 12 (12 quantitative) | 0.0% (0/12) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| prepared / staged_attainable_facts | 18 (18 quantitative) | 55.6% (10/18) | 60.0% (6/10) | 100.0% (6/6) | mean 6654.2, median 4142.0 (n=6) | mean 959.9, median 865.0 (n=10) | mean 0.6, median 0.5 (n=10) | mean 0.0, median 0.0 (n=10) |
| unprepared / natural_progression | 6 (6 quantitative) | 0.0% (0/6) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |

### Villagers versus no villagers by progression cohort

| Group | Runs | Launch | Resolution after launch | Hero alive at resolution | Assault duration | Hero damage | Hero deaths | Structure damage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| launch_not_observed / natural_progression | 18 (18 quantitative) | 0.0% (0/18) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| launch_not_observed / staged_attainable_facts | 8 (8 quantitative) | 0.0% (0/8) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| no_villagers_at_launch / staged_attainable_facts | 8 (8 quantitative) | 100.0% (8/8) | 50.0% (4/8) | 100.0% (4/4) | mean 5685.2, median 3771.5 (n=4) | mean 822.2, median 807.5 (n=8) | mean 0.5, median 0.0 (n=8) | mean 0.0, median 0.0 (n=8) |
| villagers_at_launch / staged_attainable_facts | 2 (2 quantitative) | 100.0% (2/2) | 100.0% (2/2) | 100.0% (2/2) | mean 8592.0, median 8592.0 (n=2) | mean 1510.5, median 1510.5 (n=2) | mean 1.0, median 1.0 (n=2) | mean 0.0, median 0.0 (n=2) |

### Connection state

| Group | Runs | Launch | Resolution after launch | Hero alive at resolution | Assault duration | Hero damage | Hero deaths | Structure damage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| connected_through_assault | 6 (6 quantitative) | 100.0% (6/6) | 66.7% (4/6) | 100.0% (4/4) | mean 8095.5, median 8119.0 (n=4) | mean 1330.7, median 1087.0 (n=6) | mean 1.0, median 1.0 (n=6) | mean 0.0, median 0.0 (n=6) |
| no_assault | 26 (26 quantitative) | 0.0% (0/26) | n/a (0/0) | n/a (0/0) | n/a (n=0) | n/a (n=0) | n/a (n=0) | n/a (n=0) |
| ordinary_disconnect_during_assault | 2 (2 quantitative) | 100.0% (2/2) | 50.0% (1/2) | 100.0% (1/1) | mean 3772.0, median 3772.0 (n=1) | mean 413.5, median 413.5 (n=2) | mean 0.0, median 0.0 (n=2) | mean 0.0, median 0.0 (n=2) |
| safe_logout_before_launch_then_assault | 2 (2 quantitative) | 100.0% (2/2) | 50.0% (1/2) | 100.0% (1/1) | mean 3771.0, median 3771.0 (n=1) | mean 394.0, median 394.0 (n=2) | mean 0.0, median 0.0 (n=2) | mean 0.0, median 0.0 (n=2) |

### Helper versus solo

| Group | Runs | Launch | Resolution after launch | Hero alive at resolution | Assault duration | Hero damage | Hero deaths | Structure damage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| no_helper_observed | 36 (36 quantitative) | 27.8% (10/36) | 60.0% (6/10) | 100.0% (6/6) | mean 6654.2, median 4142.0 (n=6) | mean 959.9, median 865.0 (n=10) | mean 0.6, median 0.5 (n=10) | mean 0.0, median 0.0 (n=10) |

## Required baseline questions

1. **What is the exact current crisis configuration?** Confirmed — the configuration snapshot above is derived from the authoritative constants. Pressure is capped at 100; thresholds are 20/45/70/90; phase-online minima are 0/600/1,200/1,800 ticks; ready grace is 300 online ticks; maximum ready wait is 1,200 online ticks; the wave remains two Wolf Riders and one Goblin Pillager.

2. **Which pressure contributors dominate?** Natural-progression dominant-at-analysis counts were `online_time=10, structures=8`; staged-attainable-facts counts were `choose_expansion=8, structures=10`. These cohorts are reported separately because fixture-supplied facts deliberately change the contributor distribution. Contributor means are available in the machine report; architecture alone shows structures are the largest single fixed contributor at +20.

3. **How long does each phase last?** Natural progression: Dormant mean 5605.6, median 4806.0 (n=18); Signs mean 2939.1, median 3599.0 (n=10); Pressure n/a (n=0); Preparing n/a (n=0); AssaultReady n/a (n=0); Assault n/a (n=0). Staged attainable facts: Dormant mean 7.0, median 7.0 (n=18); Signs mean 600.0, median 600.0 (n=18); Pressure mean 1981.0, median 1333.5 (n=18); Preparing mean 1800.0, median 1800.0 (n=10); AssaultReady mean 1139.7, median 1200.0 (n=10); Assault mean 6654.2, median 4142.0 (n=6). Missing transitions remain absent rather than being converted to zero; staged timing measures production phase gates after controlled setup, not natural time-to-preparation.

4. **How much online preparation time exists?** Natural progression online-before-launch: n/a (n=0); staged attainable-facts online-before-launch: mean 4835.7, median 4801.0 (n=10). Staged Signs-warning-to-launch lead was mean 4886.2, median 4800.0 (n=10) in global ticks and mean 4834.7, median 4800.0 (n=10) in online-active ticks; Preparing-warning-to-launch lead was mean 2888.2, median 3000.0 (n=10); AssaultReady-warning-to-launch lead was mean 1088.2, median 1200.0 (n=10).

5. **How often does the assault launch?** Natural progression: 0.0% (0/18); staged attainable-facts probe: 55.6% (10/18). The staged rate is a harness-success measure, not a natural launch probability.

6. **How often does a passive player win?** Insufficient assault data — natural passive: no assault launched in 3 quantitative runs (launch 0.0% (0/3)).

7. **How often does a basically competent player win?** Insufficient assault data — natural basic-survival: no assault launched in 3 quantitative runs (launch 0.0% (0/3)).

8. **How often does a deliberately prepared solo player win?** Insufficient assault data — natural prepared-solo: no assault launched in 3 quantitative runs (launch 0.0% (0/3)). The separate staged combat probe reports: Confirmed for this bounded sample — staged prepared-solo: launch 33.3% (1/3), assault resolution 0.0% (0/1), hero alive at resolution n/a (0/0). Staged results do not establish an organic preparation or natural launch rate.

9. **How much do villagers improve outcomes?** Observed within staged attainable-facts rows, not causal and not an organic preparation comparison: villager resolution 100.0% (2/2) versus no-villager 50.0% (4/8); hero alive at resolution 100.0% (2/2) versus 100.0% (4/4).

10. **How much do walls improve outcomes?** Observed within staged attainable-facts rows, not causal: fortified resolution 100.0% (1/1) versus prepared 0.0% (0/1); structure damage mean 0.0, median 0.0 (n=1) versus mean 0.0, median 0.0 (n=1). The policies differ by their wall cap (six versus three), but actual achieved state, fixture geometry, and random world events may also differ, so this is directional staged evidence only.

11. **How much damage does the settlement take?** Across launched staged assaults: structure damage mean 0.0, median 0.0 (n=10); villager damage is retained per run in JSON; villager losses mean 0.0, median 0.0 (n=10). Natural-progression assault outcomes, if any, remain separately visible in the cohort tables.

12. **Are structures routinely destroyed?** In launched staged assaults, observed structures destroyed mean 0.0, median 0.0 (n=10); walls destroyed mean 0.0, median 0.0 (n=10). Ordinary personal-crisis attackers currently target owner units and walls rather than ordinary non-wall structures, so this metric cannot support a broad conclusion about every structure type or natural preparation.

13. **Are any hero classes structurally disadvantaged?** The class table reports the current starting asymmetry, and the class/cohort aggregates above separate natural progression from staged combat probes. Any class with zero launched or resolved staged samples remains insufficient data; even launched staged rows cannot establish natural solo viability, and the melee-biased bot especially limits Ranger/Mage interpretation.

14. **Does the assault remain solo-completable?** Insufficient assault data — natural prepared-solo: no assault launched in 3 quantitative runs (launch 0.0% (0/3)). A separate combat probe reports: Confirmed for this bounded sample — staged prepared-solo: launch 33.3% (1/3), assault resolution 0.0% (0/1), hero alive at resolution n/a (0/0). This can show whether the unchanged wave can resolve under staged attainable facts, but it cannot establish organic solo-completability.

15. **Does ordinary disconnect create an advantage?** Measured in this bounded lifecycle sample — staged ordinary-disconnect: launch 66.7% (2/3), assault resolution 50.0% (1/2), hero alive at resolution 100.0% (1/1). This is a staged lifecycle probe. Compare only directionally with staged prepared-solo because connection timing and combat exposure differ; no natural-progression advantage is established.

16. **Does Safe Logout before launch alter later balance?** Measured in this bounded lifecycle sample — staged Safe-Logout-before-assault: launch 66.7% (2/3), assault resolution 50.0% (1/2), hero alive at resolution 100.0% (1/1). The staged matrix contains 2 completed pre-launch Safe Logout lifecycle sample(s), so freeze/resume was exercised; the bounded sample still cannot establish natural balance equivalence.

17. **Can helpers trivialize the assault?** Insufficient data — helper-supported measurement was deliberately omitted because the harness has no second action-driving bot. Attribution fields exist and focused tests cover player/villager/helper classification.

18. **Are adjacent settlements isolated correctly?** Insufficient new balance-matrix data — the adjacent-settlement scenario was deliberately omitted. Existing crisis isolation regressions remain the behavioral evidence; cross-player target violations are reported as an invariant.

19. **Are warnings delivered with useful lead time?** Staged Signs delivery 100.0% (18/18); staged Preparing delivery 100.0% (18/18); staged AssaultReady delivery 100.0% (10/10). Staged Signs-warning-to-launch lead was mean 4886.2, median 4800.0 (n=10) in global ticks and mean 4834.7, median 4800.0 (n=10) in online-active ticks; later online-active lead times were mean 2888.2, median 3000.0 (n=10) from Preparing and mean 1088.2, median 1200.0 (n=10) from AssaultReady. Natural rows that never reached those phases or never launched provide no warning-lead sample. Whether staged server-delivery values are *useful* remains a likely finding only until natural and human-play validation exists.

20. **Which three to five balance issues should Checkpoint 2 address?** Checkpoint 2 is limited to pressure and phase pacing. Candidate evidence questions are: (a) whether natural play has a reachable contributor path beyond `Signs`/`Pressure`; (b) whether required objective, structure, wealth, villager, and sanctuary combinations make later thresholds unintentionally inaccessible; (c) whether the ordered online phase minima create the intended preparation cadence once facts are met; (d) whether ready grace and dusk/night preference provide adequate server-side lead time; and (e) whether a natural, non-fixture scenario can launch within a bounded but representative play window. This single-cycle, heavily censored sample does not yet support selecting exact tuning changes; Checkpoint 2 should begin with repeated natural-path validation. Class, defence, villager, helper, and adjacent-settlement work belongs to later checkpoints or additional baseline validation.

## Safety invariants

- Automatic dusk hordes in PersonalCrisis mode: 0.
- Duplicate assault launches: 0.
- Cross-player target violations: 0.
- Crisis invariant failures: 0.
- Safe Logout invariant recoveries: 0.
- Panics: 0.

## Instrumentation limitations

- This bounded report contains 36 rows. A 36-row base cycle provides exactly one observation for each of 12 driver-variant × 3 hero-class cells, with no within-cell repetition; every row records its own tick cap. Rows ending in MaxTicks reached the overall run cap and may already have resolved the crisis, so unresolved-at-cap is reported separately.
- The game uses thread_rng in production systems; run identifiers are deterministic matrix identifiers, not RNG seeds.
- Staged prepared, fortified, villager, disconnect, and Safe Logout rows use a transparent headless-only progression fixture: existing objectives are complete, the nearest monolith is relocated to the base and set to sanctuary level 3, and existing Logs and Gold Coins are supplied. The bot must still build through player events before authoritative pressure, phase minima, launch, spawn, and combat run normally. Natural variants remain separate; staged rows are not evidence of natural launch rate or organic preparation, and monolith relocation changes the settlement anchor and assault spawn geometry.
- A singular dominant-pressure label uses first-declared contributor order to break equal-value ties; the full contributor vector remains available.
- The headless bot is deterministic but uses a predominantly melee combat policy, which can bias Ranger and Mage comparisons.
- Helper-supported and adjacent-settlement scenarios are labelled but omitted because the current harness lacks a second action-driving bot and player-scoped observation.
- Preparation actions are derived from bounded state deltas; crafting intent and every transient inventory transfer are not reconstructed.
- Near/away preparation time is interval-sampled: each elapsed interval is assigned wholly to the location observed at its endpoint (600 ticks in this matrix), so it is directional rather than an exact movement trace.
- The prepared policies can return home, equip an available non-hunting weapon, build existing walls, and upgrade the sanctuary, but the bot has no explicit armor-selection or structure-repair driver.
- The Safe Logout setup helper repositions the hero and every currently alive, visible-target NPC and rebases headless recent-combat/damage observations beyond the unchanged production cooldown. Later spawns or new damage can still reject or cancel, and their typed telemetry remains in the ordinary run row. Comparison with prepared-solo is therefore a lifecycle probe rather than a perfectly paired balance experiment.
- Ordinary crisis attackers currently damage owner units and walls; ordinary non-wall structures are not normal attack targets, limiting structure-damage observations.
- The runtime starting Health Potion is overridden to Healing 10 even though the item template declares 50; the baseline reports runtime behavior.
- A passive run has at most 25 pressure from danger unlock and online time, so it can enter Signs but cannot naturally reach Pressure under the current formula.
- Warning timestamps represent the first successfully sent crisis status packet for the phase, not client rendering acknowledgement.
- Checkpoint 1 records current behavior and does not change pressure, phase, enemy, class, economy, or Safe Logout balance values.

## Checkpoint status

This is the Checkpoint 1 baseline, not milestone completion. Checkpoint 2 may tune only issues supported by this evidence and further controlled runs.
