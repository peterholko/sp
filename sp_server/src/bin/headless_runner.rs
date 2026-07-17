// Multi-game headless balance/metrics runner.
//
//   cargo run --bin headless_runner [N] [MAX_TICKS]
//       [standard|safe-logout|safe-logout-matrix]
//   cargo run --bin headless_runner [N] [MAX_TICKS]
//       goblin-balance [control|candidate]
//
// Runs N full games (default 20) back-to-back, each in a fresh in-process Bevy
// `App` (full isolation), driven by the deterministic scripted bot. Emits
// `headless_runs.csv` + `headless_runs.json` and prints an aggregate summary.
//
// MUST be run with CWD = sp_server/ so templates/map/tileset load by relative
// path (same as the existing tests).

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;

use serde::{Deserialize, Serialize};
use siege_perilous::crisis_balance::{
    CrisisBalanceScenario, CrisisPressureBreakdown, GoblinCrisisBalanceConfigSnapshot,
};
use siege_perilous::game::CrisisPhase;
use siege_perilous::headless::{HeadlessGame, RunMetrics, SafeLogoutCompletionOutcome};
use siege_perilous::headless_bot::Bot;
use siege_perilous::safe_logout::PlayerWorldPresence;

// Game ticks a single run may advance past hero spawn before being capped.
// 2400 ticks = 1 in-game day; 120k = ~50 days, enough to reach the Rescue
// victory (now at 50 days survived).
const DEFAULT_MAX_TICKS: i32 = 120_000;
const DEFAULT_NUM_GAMES: u32 = 20;
// Game ticks advanced between bot decisions. A hero move resolves in ~12 ticks;
// 8 lets the move start before the bot re-evaluates without cancelling it.
const DECISION_TICKS: u32 = 8;
const SAFE_LOGOUT_SCENARIO_PROTECTED_TICKS: u32 = 250;
const SAFE_LOGOUT_LONG_PROTECTION_TICKS: u32 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerMode {
    Standard,
    SafeLogout,
    SafeLogoutMatrix,
    GoblinBalance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BalanceComparisonSide {
    Control,
    Candidate,
}

impl BalanceComparisonSide {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "control" | "old" => Some(Self::Control),
            "candidate" | "new" => Some(Self::Candidate),
            _ => None,
        }
    }

    const fn report_path(self) -> &'static str {
        match self {
            Self::Control => "goblin_crisis_balance_checkpoint2_control_report.json",
            Self::Candidate => "goblin_crisis_balance_checkpoint2_candidate_report.json",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Candidate => "candidate",
        }
    }
}

fn validate_balance_comparison_config(
    side: BalanceComparisonSide,
    config: &GoblinCrisisBalanceConfigSnapshot,
) -> Result<(), String> {
    let (expected_preparing, expected_ready) = match side {
        BalanceComparisonSide::Control => (70, 90),
        BalanceComparisonSide::Candidate => (45, 49),
    };
    if config.preparing_threshold == expected_preparing
        && config.assault_ready_threshold == expected_ready
    {
        return Ok(());
    }

    Err(format!(
        "refusing to label this binary '{}' because its Preparing/AssaultReady thresholds are {}/{}; expected {}/{}",
        side.label(),
        config.preparing_threshold,
        config.assault_ready_threshold,
        expected_preparing,
        expected_ready,
    ))
}

impl RunnerMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(Self::Standard),
            "safe-logout" | "safe_logout" => Some(Self::SafeLogout),
            "safe-logout-matrix" | "safe_logout_matrix" => Some(Self::SafeLogoutMatrix),
            "goblin-balance" | "goblin_balance" | "crisis-balance" | "crisis_balance" => {
                Some(Self::GoblinBalance)
            }
            _ => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::SafeLogout => "safe_logout",
            Self::SafeLogoutMatrix => "safe_logout_matrix",
            Self::GoblinBalance => "goblin_balance",
        }
    }
}

const BALANCE_HERO_CLASSES: [&str; 3] = ["Warrior", "Ranger", "Mage"];
const BALANCE_SAMPLE_INTERVAL_TICKS: i32 = 600;
const BALANCE_ORDINARY_DISCONNECT_TICKS: u32 = 100;
const BALANCE_SAFE_LOGOUT_PROTECTED_TICKS: u32 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BalanceDriverVariant {
    scenario: CrisisBalanceScenario,
    progression_fixture: bool,
}

const BALANCE_DRIVER_VARIANTS: [BalanceDriverVariant; 13] = [
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::PreparedSolo,
        progression_fixture: true,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::FortifiedSolo,
        progression_fixture: true,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::NoVillagers,
        progression_fixture: true,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::VillagerSupported,
        progression_fixture: true,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::OrdinaryDisconnect,
        progression_fixture: true,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::SafeLogoutBeforeAssault,
        progression_fixture: true,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::HelperSupported,
        progression_fixture: true,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::Passive,
        progression_fixture: false,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::BasicSurvival,
        progression_fixture: false,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::PreparedSolo,
        progression_fixture: false,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::FortifiedSolo,
        progression_fixture: false,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::NoVillagers,
        progression_fixture: false,
    },
    BalanceDriverVariant {
        scenario: CrisisBalanceScenario::VillagerSupported,
        progression_fixture: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BalanceRunSpec {
    scenario: CrisisBalanceScenario,
    hero_class: &'static str,
    repetition: u32,
    progression_fixture: bool,
}

impl BalanceRunSpec {
    const COMBINATIONS: usize = BALANCE_DRIVER_VARIANTS.len() * BALANCE_HERO_CLASSES.len();

    fn for_run(run_index: u32) -> Self {
        let combination = run_index as usize % Self::COMBINATIONS;
        let variant = BALANCE_DRIVER_VARIANTS[combination / BALANCE_HERO_CLASSES.len()];
        Self {
            scenario: variant.scenario,
            hero_class: BALANCE_HERO_CLASSES[combination % BALANCE_HERO_CLASSES.len()],
            repetition: run_index / Self::COMBINATIONS as u32,
            progression_fixture: variant.progression_fixture,
        }
    }

    fn run_id(self, run_index: u32) -> String {
        format!(
            "{}-{}-{}-r{}-{}",
            self.scenario.label(),
            self.hero_class.to_ascii_lowercase(),
            if self.progression_fixture {
                "staged"
            } else {
                "natural"
            },
            self.repetition,
            run_index
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafeLogoutMatrixScenario {
    NormalPlay,
    Completion,
    Cancellation,
    LongProtection,
    Reconnect,
    OrdinaryDisconnect,
    ActiveAssaultDisconnect,
    MultiplePlayers,
}

impl SafeLogoutMatrixScenario {
    const ALL: [Self; 8] = [
        Self::NormalPlay,
        Self::Completion,
        Self::Cancellation,
        Self::LongProtection,
        Self::Reconnect,
        Self::OrdinaryDisconnect,
        Self::ActiveAssaultDisconnect,
        Self::MultiplePlayers,
    ];

    fn for_run(run_index: u32) -> Self {
        Self::ALL[run_index as usize % Self::ALL.len()]
    }

    const fn label(self) -> &'static str {
        match self {
            Self::NormalPlay => "matrix_normal_play",
            Self::Completion => "matrix_completion",
            Self::Cancellation => "matrix_cancellation",
            Self::LongProtection => "matrix_long_protection",
            Self::Reconnect => "matrix_reconnect",
            Self::OrdinaryDisconnect => "matrix_ordinary_disconnect",
            Self::ActiveAssaultDisconnect => "matrix_active_assault_disconnect",
            Self::MultiplePlayers => "matrix_multiple_players",
        }
    }
}

fn run_safe_logout_completion(game: &mut HeadlessGame, protected_ticks: u32) {
    game.prepare_safe_logout_scenario();
    game.complete_valid_safe_logout_via_authenticated_ingress();
    game.disconnect_after_completed_safe_logout();
    game.advance_protected_world_ticks(protected_ticks);
    game.reconnect_and_exit_protection();
}

fn run_safe_logout_matrix_scenario(game: &mut HeadlessGame, scenario: SafeLogoutMatrixScenario) {
    match scenario {
        SafeLogoutMatrixScenario::NormalPlay => {}
        SafeLogoutMatrixScenario::Completion => {
            game.prepare_safe_logout_scenario();
            game.complete_valid_safe_logout_via_authenticated_ingress();
            game.reconnect_and_exit_protection();
        }
        SafeLogoutMatrixScenario::Cancellation => {
            game.prepare_safe_logout_scenario();
            game.request_safe_logout_via_authenticated_ingress();
            game.tick(2);
            assert_eq!(
                game.player_presence(),
                Some(PlayerWorldPresence::SafeLogoutPending)
            );
            game.cancel_safe_logout_via_authenticated_ingress();
            game.tick(2);
            assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        }
        SafeLogoutMatrixScenario::LongProtection => {
            run_safe_logout_completion(game, SAFE_LOGOUT_LONG_PROTECTION_TICKS);
        }
        SafeLogoutMatrixScenario::Reconnect => {
            run_safe_logout_completion(game, SAFE_LOGOUT_SCENARIO_PROTECTED_TICKS);
        }
        SafeLogoutMatrixScenario::OrdinaryDisconnect => {
            game.disconnect_player();
            game.tick(1);
            game.reconnect_and_exit_protection();
        }
        SafeLogoutMatrixScenario::ActiveAssaultDisconnect => {
            game.prepare_active_assault_disconnect_scenario();
            game.request_safe_logout_via_authenticated_ingress();
            game.tick(2);
            game.disconnect_player();
            game.tick(1);
            game.reconnect_and_exit_protection();
        }
        SafeLogoutMatrixScenario::MultiplePlayers => {
            let helper_player_id = game.spawn_connected_scenario_helper("SafeLogoutMatrixHelper");
            assert!(game.is_player_connected(helper_player_id));
            run_safe_logout_completion(game, SAFE_LOGOUT_SCENARIO_PROTECTED_TICKS);
            assert!(game.is_player_connected(helper_player_id));
        }
    }
}

fn result_scenario_label(run_index: u32, mode: RunnerMode) -> &'static str {
    match mode {
        RunnerMode::SafeLogoutMatrix => SafeLogoutMatrixScenario::for_run(run_index).label(),
        RunnerMode::GoblinBalance => BalanceRunSpec::for_run(run_index).scenario.label(),
        _ => mode.label(),
    }
}

fn run_one(run_index: u32, max_ticks: i32, mode: RunnerMode) -> RunMetrics {
    let mut game = HeadlessGame::new(max_ticks);
    let balance_spec =
        (mode == RunnerMode::GoblinBalance).then(|| BalanceRunSpec::for_run(run_index));
    let hero_class = balance_spec
        .map(|spec| spec.hero_class)
        .unwrap_or("Warrior");
    let pid = game.spawn_hero(hero_class, &format!("Bot{run_index}"));
    let mut bot = balance_spec
        .map(|spec| Bot::for_balance_scenario(pid, spec.scenario))
        .unwrap_or_else(|| Bot::new(pid));
    if balance_spec.is_some() {
        game.set_crisis_balance_sample_interval(Some(BALANCE_SAMPLE_INTERVAL_TICKS));
    }
    if balance_spec.is_some_and(|spec| spec.progression_fixture) {
        let spec = balance_spec.expect("checked balance spec");
        game.prepare_crisis_balance_progression_fixture(spec.scenario);
    }
    let mut helper_bot = if balance_spec
        .is_some_and(|spec| spec.scenario == CrisisBalanceScenario::HelperSupported)
    {
        let owner_view = game.observe();
        let owner_anchor = owner_view
            .home()
            .or_else(|| owner_view.hero.map(|hero| hero.pos))
            .expect("helper-supported owner anchor");
        let helper_player_id = game.spawn_connected_scenario_helper("CrisisBalanceHelper");
        Some((
            helper_player_id,
            Bot::for_helper_support(helper_player_id, pid, owner_anchor),
        ))
    } else {
        None
    };

    match mode {
        RunnerMode::Standard => {}
        RunnerMode::SafeLogout => {
            run_safe_logout_completion(&mut game, SAFE_LOGOUT_SCENARIO_PROTECTED_TICKS);
        }
        RunnerMode::SafeLogoutMatrix => {
            run_safe_logout_matrix_scenario(
                &mut game,
                SafeLogoutMatrixScenario::for_run(run_index),
            );
        }
        RunnerMode::GoblinBalance => {}
    }

    let mut ordinary_disconnect_done = false;
    let mut safe_logout_done = false;

    while !game.is_over() {
        if let Some(spec) = balance_spec {
            let phase = game.settlement_crisis().map(|crisis| crisis.phase);
            if spec.scenario == CrisisBalanceScenario::OrdinaryDisconnect
                && phase == Some(CrisisPhase::AssaultActive)
                && !ordinary_disconnect_done
            {
                ordinary_disconnect_done = true;
                game.disconnect_player();
                game.tick(BALANCE_ORDINARY_DISCONNECT_TICKS);
                if !game.is_over() {
                    game.reconnect_player_with_login();
                    game.tick(8);
                }
            }
            if spec.scenario == CrisisBalanceScenario::SafeLogoutBeforeAssault
                && phase == Some(CrisisPhase::AssaultReady)
                && !safe_logout_done
            {
                safe_logout_done = true;
                game.prepare_safe_logout_scenario();
                if game.try_complete_valid_safe_logout_via_authenticated_ingress()
                    == SafeLogoutCompletionOutcome::Completed
                {
                    game.disconnect_after_completed_safe_logout();
                    game.advance_protected_world_ticks(BALANCE_SAFE_LOGOUT_PROTECTED_TICKS);
                    game.reconnect_and_exit_protection();
                }
            }
        }
        let view = game.observe();
        let action = bot.step(&view, game.map());
        if let Some(target_id) = bot.observed_assault_target_id() {
            game.record_observed_crisis_target(target_id);
        }
        if let Some(event) = action {
            game.inject(event);
        }
        bot.advance_phase(&view);
        if let Some((helper_player_id, helper_bot)) = helper_bot.as_mut() {
            let helper_view = game.observe_for_player(*helper_player_id);
            let helper_action = helper_bot.step(&helper_view, game.map());
            if let Some(event) = helper_action {
                game.inject(event);
            }
            helper_bot.advance_phase(&helper_view);
        }
        game.tick(DECISION_TICKS);
    }

    let mut metrics = game.metrics();
    metrics.run_index = run_index;
    metrics.safe_logout_scenario_mode = result_scenario_label(run_index, mode).to_string();
    if let Some(spec) = balance_spec {
        metrics.safe_logout_scenario_mode = RunnerMode::Standard.label().to_string();
        metrics.crisis_balance_scenario = spec.scenario.label().to_string();
        metrics.crisis_balance_hero_class = spec.hero_class.to_string();
        metrics.crisis_balance_run_id = spec.run_id(run_index);
        metrics.crisis_balance_tick_cap = max_ticks;
        metrics.crisis_balance_progression_fixture = spec.progression_fixture;
    }
    metrics
    // `game` dropped here -> App/World freed -> next run fully isolated.
}

// Run one game, but never let a panic inside the game-under-test abort the whole
// batch. A panicking run is recorded with outcome "Panic" and its (discarded)
// App is dropped; the next run builds a fresh one. Each run already owns its own
// App, so a caught panic cannot leak state into later runs.
fn run_one_safe(run_index: u32, max_ticks: i32, mode: RunnerMode) -> RunMetrics {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_one(run_index, max_ticks, mode)
    }));
    match result {
        Ok(metrics) => metrics,
        Err(_) => panic_metrics(run_index, max_ticks, mode),
    }
}

fn panic_metrics(run_index: u32, max_ticks: i32, mode: RunnerMode) -> RunMetrics {
    let balance_spec =
        (mode == RunnerMode::GoblinBalance).then(|| BalanceRunSpec::for_run(run_index));
    RunMetrics {
        run_index,
        outcome: "Panic".to_string(),
        killer: String::new(),
        ticks: 0,
        days_survived: 0,
        waves_survived: 0,
        enemies_killed: 0,
        elites_killed: 0,
        captains_killed: 0,
        legendary_kills: 0,
        hideouts_cleared: 0,
        repairs: 0,
        highest_pressure_level: 0,
        num_deaths: 0,
        obj_scavenge_shipwreck: false,
        obj_build_campfire: false,
        obj_win_first_fight: false,
        obj_build_3_structures: false,
        obj_recruit_villager: false,
        obj_explore_poi: false,
        obj_choose_expansion: false,
        obj_survive_5_nights: false,
        obj_find_legendary_hideout: false,
        obj_defeat_ashen_warlord: false,
        victory_rescue_progress: 0,
        victory_prosperity: false,
        victory_conquest: false,
        final_hp: 0,
        final_skill_total: 0,
        final_inventory_count: 0,
        structures_built: 0,
        crisis_highest_phase: "none".to_string(),
        crisis_final_phase: "none".to_string(),
        crisis_final_pressure: 0,
        crisis_signs_tick: None,
        crisis_pressure_tick: None,
        crisis_preparing_tick: None,
        crisis_assault_ready_tick: None,
        crisis_assault_active_tick: None,
        crisis_resolved_tick: None,
        crisis_assaults_launched: 0,
        crisis_assaults_resolved: 0,
        crisis_units_remaining: 0,
        crisis_status_packets_sent: 0,
        crisis_login_snapshots_sent: 0,
        crisis_duplicate_assaults: 0,
        personal_crisis_automatic_dusk_hordes: 0,
        crisis_invariants_ok: false,
        safe_logout_scenario_mode: result_scenario_label(run_index, mode).to_string(),
        safe_logout_requests: 0,
        safe_logout_accepted: 0,
        safe_logout_rejected: 0,
        safe_logout_cancelled: 0,
        safe_logout_completed: 0,
        safe_logout_protected_sessions_started: 0,
        safe_logout_resumed: 0,
        safe_logout_protected_ticks_total: 0,
        safe_logout_ordinary_disconnects: 0,
        safe_logout_active_assault_disconnects: 0,
        safe_logout_status_packets_sent: 0,
        safe_logout_status_packets_duplicate_suppressed: 0,
        safe_logout_protected_input_rejections: 0,
        safe_logout_protected_damage_blocks: 0,
        safe_logout_protected_target_rejections: 0,
        safe_logout_queued_events_discarded: 0,
        safe_logout_invariant_recoveries: 0,
        safe_logout_run_key_mismatches: 0,
        safe_logout_timer_rebases: 0,
        safe_logout_stale_connection_events_rejected: 0,
        safe_logout_rejection_reasons: Default::default(),
        safe_logout_cancellation_reasons: Default::default(),
        safe_logout_invariant_reasons: Default::default(),
        safe_logout_invariants_ok: false,
        crisis_balance_scenario: balance_spec
            .map(|spec| spec.scenario.label())
            .unwrap_or("standard")
            .to_string(),
        crisis_balance_hero_class: balance_spec
            .map(|spec| spec.hero_class)
            .unwrap_or("unknown")
            .to_string(),
        crisis_balance_run_id: balance_spec
            .map(|spec| spec.run_id(run_index))
            .unwrap_or_default(),
        crisis_balance_tick_cap: max_ticks,
        crisis_balance_tick_cap_reached: false,
        crisis_balance_progression_fixture: balance_spec
            .is_some_and(|spec| spec.progression_fixture),
        crisis_balance_config: siege_perilous::game::goblin_crisis_balance_config_snapshot(),
        crisis_balance: Default::default(),
        crisis_warning_signs_to_launch_global_ticks: None,
        crisis_warning_signs_to_launch_online_ticks: None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = match args.get(3) {
        Some(value) => match RunnerMode::parse(value) {
            Some(mode) => mode,
            None => {
                eprintln!(
                    "Unknown runner mode '{value}'. Expected 'standard', 'safe-logout', 'safe-logout-matrix', or 'goblin-balance'."
                );
                std::process::exit(2);
            }
        },
        None => RunnerMode::Standard,
    };
    let balance_side = if mode == RunnerMode::GoblinBalance {
        match args.get(4) {
            Some(value) => match BalanceComparisonSide::parse(value) {
                Some(side) => Some(side),
                None => {
                    eprintln!(
                        "Unknown goblin-balance comparison side '{value}'. Expected 'control' or 'candidate'."
                    );
                    std::process::exit(2);
                }
            },
            None => Some(BalanceComparisonSide::Candidate),
        }
    } else {
        None
    };
    if let Some(side) = balance_side {
        let config = siege_perilous::game::goblin_crisis_balance_config_snapshot();
        if let Err(error) = validate_balance_comparison_config(side, &config) {
            eprintln!("Invalid goblin-balance comparison: {error}.");
            std::process::exit(2);
        }
    }
    let num_games: u32 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or_else(|| {
        if mode == RunnerMode::GoblinBalance {
            BalanceRunSpec::COMBINATIONS as u32
        } else {
            DEFAULT_NUM_GAMES
        }
    });
    let max_ticks: i32 = args
        .get(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(DEFAULT_MAX_TICKS);

    let comparison_label = balance_side
        .map(|side| format!(", comparison={}", side.label()))
        .unwrap_or_default();
    println!(
        "Running {num_games} headless games (max_ticks={max_ticks}, decision_ticks={DECISION_TICKS}, mode={}{})...",
        mode.label(),
        comparison_label,
    );

    let mut results: Vec<RunMetrics> = Vec::with_capacity(num_games as usize);
    for i in 0..num_games {
        let t0 = std::time::Instant::now();
        let m = run_one_safe(i, max_ticks, mode);
        let elapsed = t0.elapsed();
        println!(
            "  run {:>4}: {:<16} killer={:<12} ticks={:>6} days={:>2} enemies={:>3} deaths={} hp={:>4} skillxp={:>5} inv={:>2} structs={} crisis={:<14} launches={} resolutions={} packets={} safe_logout={}/{}/{} protected_ticks={} [{:.2}s]",
            m.run_index,
            m.outcome,
            if m.killer.is_empty() { "-" } else { &m.killer },
            m.ticks,
            m.days_survived,
            m.enemies_killed,
            m.num_deaths,
            m.final_hp,
            m.final_skill_total,
            m.final_inventory_count,
            m.structures_built,
            m.crisis_highest_phase,
            m.crisis_assaults_launched,
            m.crisis_assaults_resolved,
            m.crisis_status_packets_sent,
            m.safe_logout_accepted,
            m.safe_logout_completed,
            m.safe_logout_resumed,
            m.safe_logout_protected_ticks_total,
            elapsed.as_secs_f64(),
        );
        results.push(m);
    }

    write_csv(&results, "headless_runs.csv");
    write_json(&results, "headless_runs.json");
    print_summary(&results);
    if mode == RunnerMode::GoblinBalance {
        if let Err(error) =
            write_checkpoint2_balance_report(&results, balance_side.expect("goblin-balance side"))
        {
            eprintln!("Failed to write goblin balance report: {error}");
            std::process::exit(1);
        }
    }
}

const PRE_SAFE_LOGOUT_CSV_FIELDS: &[&str] = &[
    "run_index",
    "outcome",
    "killer",
    "ticks",
    "days_survived",
    "waves_survived",
    "enemies_killed",
    "elites_killed",
    "captains_killed",
    "legendary_kills",
    "hideouts_cleared",
    "repairs",
    "highest_pressure_level",
    "num_deaths",
    "obj_scavenge_shipwreck",
    "obj_build_campfire",
    "obj_win_first_fight",
    "obj_build_3_structures",
    "obj_recruit_villager",
    "obj_explore_poi",
    "obj_choose_expansion",
    "obj_survive_5_nights",
    "obj_find_legendary_hideout",
    "obj_defeat_ashen_warlord",
    "victory_rescue_progress",
    "victory_prosperity",
    "victory_conquest",
    "final_hp",
    "final_skill_total",
    "final_inventory_count",
    "structures_built",
    "crisis_highest_phase",
    "crisis_final_phase",
    "crisis_final_pressure",
    "crisis_signs_tick",
    "crisis_pressure_tick",
    "crisis_preparing_tick",
    "crisis_assault_ready_tick",
    "crisis_assault_active_tick",
    "crisis_resolved_tick",
    "crisis_assaults_launched",
    "crisis_assaults_resolved",
    "crisis_units_remaining",
    "crisis_status_packets_sent",
    "crisis_login_snapshots_sent",
    "crisis_duplicate_assaults",
    "personal_crisis_automatic_dusk_hordes",
    "crisis_invariants_ok",
];

const SAFE_LOGOUT_CSV_FIELDS: &[&str] = &[
    "safe_logout_scenario_mode",
    "safe_logout_requests",
    "safe_logout_accepted",
    "safe_logout_rejected",
    "safe_logout_cancelled",
    "safe_logout_completed",
    "safe_logout_protected_sessions_started",
    "safe_logout_resumed",
    "safe_logout_protected_ticks_total",
    "safe_logout_ordinary_disconnects",
    "safe_logout_active_assault_disconnects",
    "safe_logout_status_packets_sent",
    "safe_logout_status_packets_duplicate_suppressed",
    "safe_logout_protected_input_rejections",
    "safe_logout_protected_damage_blocks",
    "safe_logout_protected_target_rejections",
    "safe_logout_queued_events_discarded",
    "safe_logout_invariant_recoveries",
    "safe_logout_run_key_mismatches",
    "safe_logout_timer_rebases",
    "safe_logout_stale_connection_events_rejected",
    "safe_logout_rejection_reasons",
    "safe_logout_cancellation_reasons",
    "safe_logout_invariant_reasons",
    "safe_logout_invariants_ok",
];

const BALANCE_CSV_FIELDS: &[&str] = &[
    "crisis_balance_scenario",
    "crisis_balance_hero_class",
    "crisis_balance_run_id",
    "crisis_balance_tick_cap",
    "crisis_balance_tick_cap_reached",
    "crisis_balance_progression_fixture",
    "crisis_balance_config",
    "crisis_created_tick",
    "crisis_created_online_tick",
    "crisis_signs_online_tick",
    "crisis_pressure_online_tick",
    "crisis_preparing_online_tick",
    "crisis_assault_ready_online_tick",
    "crisis_assault_active_online_tick",
    "crisis_resolved_online_tick",
    "crisis_dormant_duration_ticks",
    "crisis_signs_duration_ticks",
    "crisis_pressure_duration_ticks",
    "crisis_preparing_duration_ticks",
    "crisis_ready_duration_ticks",
    "crisis_assault_duration_ticks",
    "crisis_total_duration_ticks",
    "crisis_total_online_before_launch_ticks",
    "crisis_pressure_creation_raw",
    "crisis_pressure_creation_clamped",
    "crisis_pressure_signs_raw",
    "crisis_pressure_signs_clamped",
    "crisis_pressure_pressure_raw",
    "crisis_pressure_pressure_clamped",
    "crisis_pressure_preparing_raw",
    "crisis_pressure_preparing_clamped",
    "crisis_pressure_ready_raw",
    "crisis_pressure_ready_clamped",
    "crisis_pressure_launch_raw",
    "crisis_pressure_launch_clamped",
    "crisis_pressure_resolution_raw",
    "crisis_pressure_resolution_clamped",
    "crisis_pressure_raw_total",
    "crisis_pressure_clamped_total",
    "crisis_pressure_danger_unlocked",
    "crisis_pressure_structures",
    "crisis_pressure_villagers",
    "crisis_pressure_explore_poi",
    "crisis_pressure_choose_expansion",
    "crisis_pressure_stored_gold",
    "crisis_pressure_sanctuary",
    "crisis_pressure_online_time",
    "crisis_pressure_dominant_contributor",
    "crisis_prep_hero_health",
    "crisis_prep_hero_max_health",
    "crisis_prep_equipped_weapon",
    "crisis_prep_equipped_armor_count",
    "crisis_prep_healing_items",
    "crisis_prep_food_items",
    "crisis_prep_drink_items",
    "crisis_prep_completed_structures",
    "crisis_prep_foundations",
    "crisis_prep_wall_segments",
    "crisis_prep_wall_total_health",
    "crisis_prep_wall_total_max_health",
    "crisis_prep_villagers_alive",
    "crisis_prep_villagers_combat_capable",
    "crisis_prep_sanctuary_level",
    "crisis_prep_stored_gold",
    "crisis_prep_stored_food",
    "crisis_prep_stored_resources_total",
    "crisis_prep_structures_built",
    "crisis_prep_walls_built",
    "crisis_prep_structures_repaired",
    "crisis_prep_equipment_changes",
    "crisis_prep_healing_items_acquired",
    "crisis_prep_villagers_recruited",
    "crisis_prep_villager_assignments_changed",
    "crisis_prep_sanctuary_upgrades",
    "crisis_prep_resource_units_acquired",
    "crisis_prep_storage_units_added",
    "crisis_prep_online_ticks_near_settlement",
    "crisis_prep_online_ticks_away_from_settlement",
    "crisis_prep_returned_after_warning",
    "crisis_prep_action_performed",
    "crisis_outcome_assault_launched",
    "crisis_outcome_assault_resolved",
    "crisis_outcome_units_total",
    "crisis_outcome_units_defeated",
    "crisis_outcome_units_remaining",
    "crisis_outcome_hero_damage",
    "crisis_outcome_hero_deaths",
    "crisis_outcome_hero_alive_at_resolution",
    "crisis_outcome_villagers_at_launch",
    "crisis_outcome_villagers_killed",
    "crisis_outcome_structures_at_launch",
    "crisis_outcome_structures_damaged",
    "crisis_outcome_structures_destroyed",
    "crisis_outcome_walls_at_launch",
    "crisis_outcome_walls_destroyed",
    "crisis_outcome_structure_damage",
    "crisis_outcome_villager_damage",
    "crisis_outcome_player_kills",
    "crisis_outcome_villager_kills",
    "crisis_outcome_helper_kills",
    "crisis_outcome_defence_or_other_kills",
    "crisis_outcome_ordinary_disconnect",
    "crisis_outcome_reconnected",
    "crisis_outcome_resolved_owner_offline",
    "crisis_outcome_safe_logout_before_assault",
    "crisis_outcome_helper_participated",
    "crisis_outcome_cross_player_target_violations",
    "crisis_warning_signs_delivery_tick",
    "crisis_warning_preparing_delivery_tick",
    "crisis_warning_ready_delivery_tick",
    "crisis_warning_launch_delivery_tick",
    "crisis_warning_preparing_to_launch_online_ticks",
    "crisis_warning_ready_to_launch_online_ticks",
    "crisis_warning_preparing_near_settlement",
    "crisis_warning_ready_near_settlement",
    "crisis_warning_launch_near_settlement",
];

// The 73 legacy columns plus BALANCE_CSV_FIELDS are the existing 189-column
// Checkpoint 1 schema. New fields must be appended here, never inserted into
// that frozen prefix.
const SIGNS_WARNING_CSV_FIELDS: &[&str] = &[
    "crisis_warning_signs_to_launch_global_ticks",
    "crisis_warning_signs_to_launch_online_ticks",
];

// Checkpoint 3 preparation telemetry is append-only so every earlier CSV
// consumer continues to see its complete schema as an unchanged prefix.
const CHECKPOINT3_PREPARATION_CSV_FIELDS: &[&str] = &[
    "crisis_prep_repairs_started",
    "crisis_prep_repairs_completed",
    "crisis_prep_defensive_structures_started",
    "crisis_prep_defensive_structures_completed",
    "crisis_prep_healing_items_carried_at_launch",
    "crisis_prep_healing_items_used_before_launch",
    "crisis_prep_combat_capable_villagers_at_launch",
    "crisis_prep_first_preparation_action_tick",
    "crisis_prep_meaningful_preparation_categories",
    "crisis_prep_meaningful_preparation_category_count",
];

fn csv_header_fields() -> Vec<&'static str> {
    PRE_SAFE_LOGOUT_CSV_FIELDS
        .iter()
        .chain(SAFE_LOGOUT_CSV_FIELDS)
        .chain(BALANCE_CSV_FIELDS)
        .chain(SIGNS_WARNING_CSV_FIELDS)
        .chain(CHECKPOINT3_PREPARATION_CSV_FIELDS)
        .copied()
        .collect()
}

fn optional_tick(tick: Option<i32>) -> String {
    tick.map_or_else(String::new, |tick| tick.to_string())
}

fn reason_counts_json(counts: &std::collections::BTreeMap<String, u64>) -> String {
    serde_json::to_string(counts).unwrap_or_else(|_| "{}".to_string())
}

fn optional_bool(value: Option<bool>) -> String {
    value.map_or_else(String::new, |value| value.to_string())
}

fn balance_csv_row(m: &RunMetrics) -> Vec<String> {
    let balance = &m.crisis_balance;
    let timing = &balance.phase_timing;
    let pressure = pressure_for_analysis(m);
    let transition_pressure =
        |snapshot: Option<&siege_perilous::crisis_balance::CrisisPressureSnapshot>, raw: bool| {
            snapshot.map_or_else(String::new, |snapshot| {
                if raw {
                    snapshot.breakdown.raw_total
                } else {
                    snapshot.breakdown.clamped_total
                }
                .to_string()
            })
        };
    let prep = balance
        .preparation_snapshots
        .assault_launch
        .as_ref()
        .or(balance.preparation_snapshots.resolution_or_end.as_ref())
        .or(balance.preparation_snapshots.assault_ready.as_ref())
        .or(balance.preparation_snapshots.preparing.as_ref());
    let prep_value =
        |value: fn(&siege_perilous::crisis_balance::CrisisPreparationSnapshot) -> i32| {
            prep.map(value)
                .map_or_else(String::new, |value| value.to_string())
        };
    let actions = &balance.preparation_actions;
    let outcome = &balance.assault_outcome;
    let warnings = &balance.warnings;
    vec![
        m.crisis_balance_scenario.clone(),
        m.crisis_balance_hero_class.clone(),
        m.crisis_balance_run_id.clone(),
        m.crisis_balance_tick_cap.to_string(),
        m.crisis_balance_tick_cap_reached.to_string(),
        m.crisis_balance_progression_fixture.to_string(),
        serde_json::to_string(&m.crisis_balance_config).unwrap_or_else(|_| "{}".to_string()),
        optional_tick(timing.crisis_created_tick),
        optional_tick(timing.crisis_created_online_tick),
        optional_tick(timing.signs_entered_online_tick),
        optional_tick(timing.pressure_entered_online_tick),
        optional_tick(timing.preparing_entered_online_tick),
        optional_tick(timing.assault_ready_entered_online_tick),
        optional_tick(timing.assault_active_entered_online_tick),
        optional_tick(timing.resolved_online_tick),
        optional_tick(timing.dormant_duration()),
        optional_tick(timing.signs_duration()),
        optional_tick(timing.pressure_duration()),
        optional_tick(timing.preparing_duration()),
        optional_tick(timing.assault_ready_duration()),
        optional_tick(timing.assault_duration()),
        optional_tick(timing.total_crisis_duration()),
        optional_tick(timing.total_online_before_launch()),
        transition_pressure(balance.pressure_snapshots.creation.as_ref(), true),
        transition_pressure(balance.pressure_snapshots.creation.as_ref(), false),
        transition_pressure(balance.pressure_snapshots.signs.as_ref(), true),
        transition_pressure(balance.pressure_snapshots.signs.as_ref(), false),
        transition_pressure(balance.pressure_snapshots.pressure.as_ref(), true),
        transition_pressure(balance.pressure_snapshots.pressure.as_ref(), false),
        transition_pressure(balance.pressure_snapshots.preparing.as_ref(), true),
        transition_pressure(balance.pressure_snapshots.preparing.as_ref(), false),
        transition_pressure(balance.pressure_snapshots.assault_ready.as_ref(), true),
        transition_pressure(balance.pressure_snapshots.assault_ready.as_ref(), false),
        transition_pressure(balance.pressure_snapshots.assault_launch.as_ref(), true),
        transition_pressure(balance.pressure_snapshots.assault_launch.as_ref(), false),
        transition_pressure(balance.pressure_snapshots.resolution.as_ref(), true),
        transition_pressure(balance.pressure_snapshots.resolution.as_ref(), false),
        pressure.raw_total.to_string(),
        pressure.clamped_total.to_string(),
        pressure.danger_unlocked.to_string(),
        pressure.structures.to_string(),
        pressure.villagers.to_string(),
        pressure.explore_poi.to_string(),
        pressure.choose_expansion.to_string(),
        pressure.stored_gold.to_string(),
        pressure.sanctuary.to_string(),
        pressure.online_time.to_string(),
        pressure.dominant_contributor().unwrap_or("").to_string(),
        prep_value(|value| value.hero_health),
        prep_value(|value| value.hero_max_health),
        prep.and_then(|value| value.equipped_weapon.clone())
            .unwrap_or_default(),
        prep_value(|value| value.equipped_armor_count),
        prep_value(|value| value.healing_items),
        prep_value(|value| value.food_items),
        prep_value(|value| value.drink_items),
        prep_value(|value| value.completed_structures),
        prep_value(|value| value.foundations),
        prep_value(|value| value.wall_segments),
        prep_value(|value| value.wall_total_health),
        prep_value(|value| value.wall_total_max_health),
        prep_value(|value| value.villagers_alive),
        prep_value(|value| value.villagers_combat_capable),
        prep_value(|value| value.sanctuary_level),
        prep_value(|value| value.stored_gold),
        prep_value(|value| value.stored_food),
        prep_value(|value| value.stored_resources_total),
        actions.structures_built.to_string(),
        actions.walls_built.to_string(),
        actions.structures_repaired.to_string(),
        actions.equipment_changes.to_string(),
        actions.healing_items_acquired.to_string(),
        actions.villagers_recruited.to_string(),
        actions.villager_assignments_changed.to_string(),
        actions.sanctuary_upgrades.to_string(),
        actions.resource_units_acquired.to_string(),
        actions.storage_units_added.to_string(),
        actions.online_ticks_near_settlement.to_string(),
        actions.online_ticks_away_from_settlement.to_string(),
        actions.returned_to_settlement_after_warning.to_string(),
        actions.performed_preparation_action.to_string(),
        outcome.assault_launched.to_string(),
        outcome.assault_resolved.to_string(),
        outcome.assault_unit_count.to_string(),
        outcome.assault_units_defeated.to_string(),
        outcome.assault_units_remaining.to_string(),
        outcome.hero_damage_taken.to_string(),
        outcome.hero_deaths_during_assault.to_string(),
        optional_bool(outcome.hero_alive_at_resolution),
        outcome.villagers_at_launch.to_string(),
        outcome.villagers_killed.to_string(),
        outcome.structures_at_launch.to_string(),
        outcome.structures_damaged.to_string(),
        outcome.structures_destroyed.to_string(),
        outcome.wall_segments_at_launch.to_string(),
        outcome.wall_segments_destroyed.to_string(),
        outcome.total_structure_damage.to_string(),
        outcome.total_villager_damage.to_string(),
        outcome.player_kills.to_string(),
        outcome.villager_kills.to_string(),
        outcome.helper_kills.to_string(),
        outcome.defence_or_other_kills.to_string(),
        outcome.ordinary_disconnect_during_assault.to_string(),
        outcome.reconnected_during_assault.to_string(),
        outcome.resolved_while_owner_offline.to_string(),
        outcome.safe_logout_before_assault.to_string(),
        outcome.helper_participated.to_string(),
        outcome.cross_player_target_violations.to_string(),
        optional_tick(warnings.signs_delivery_tick),
        optional_tick(warnings.preparing_delivery_tick),
        optional_tick(warnings.assault_ready_delivery_tick),
        optional_tick(warnings.assault_launch_delivery_tick),
        optional_tick(warnings.preparing_to_launch_online_ticks()),
        optional_tick(warnings.ready_to_launch_online_ticks()),
        optional_bool(warnings.preparing_near_settlement),
        optional_bool(warnings.assault_ready_near_settlement),
        optional_bool(warnings.assault_launch_near_settlement),
        optional_tick(m.crisis_warning_signs_to_launch_global_ticks),
        optional_tick(m.crisis_warning_signs_to_launch_online_ticks),
        actions.repairs_started.to_string(),
        actions.repairs_completed.to_string(),
        actions.defensive_structures_started.to_string(),
        actions.defensive_structures_completed.to_string(),
        actions.healing_items_carried_at_launch.to_string(),
        actions.healing_items_used_before_launch.to_string(),
        actions.combat_capable_villagers_at_launch.to_string(),
        optional_tick(actions.first_preparation_action_tick),
        serde_json::to_string(&actions.meaningful_preparation_categories)
            .unwrap_or_else(|_| "[]".to_string()),
        actions.meaningful_preparation_category_count.to_string(),
    ]
}

fn metrics_csv_row(m: &RunMetrics) -> Vec<String> {
    let mut row = vec![
        m.run_index.to_string(),
        m.outcome.clone(),
        m.killer.clone(),
        m.ticks.to_string(),
        m.days_survived.to_string(),
        m.waves_survived.to_string(),
        m.enemies_killed.to_string(),
        m.elites_killed.to_string(),
        m.captains_killed.to_string(),
        m.legendary_kills.to_string(),
        m.hideouts_cleared.to_string(),
        m.repairs.to_string(),
        m.highest_pressure_level.to_string(),
        m.num_deaths.to_string(),
        m.obj_scavenge_shipwreck.to_string(),
        m.obj_build_campfire.to_string(),
        m.obj_win_first_fight.to_string(),
        m.obj_build_3_structures.to_string(),
        m.obj_recruit_villager.to_string(),
        m.obj_explore_poi.to_string(),
        m.obj_choose_expansion.to_string(),
        m.obj_survive_5_nights.to_string(),
        m.obj_find_legendary_hideout.to_string(),
        m.obj_defeat_ashen_warlord.to_string(),
        m.victory_rescue_progress.to_string(),
        m.victory_prosperity.to_string(),
        m.victory_conquest.to_string(),
        m.final_hp.to_string(),
        m.final_skill_total.to_string(),
        m.final_inventory_count.to_string(),
        m.structures_built.to_string(),
        m.crisis_highest_phase.clone(),
        m.crisis_final_phase.clone(),
        m.crisis_final_pressure.to_string(),
        optional_tick(m.crisis_signs_tick),
        optional_tick(m.crisis_pressure_tick),
        optional_tick(m.crisis_preparing_tick),
        optional_tick(m.crisis_assault_ready_tick),
        optional_tick(m.crisis_assault_active_tick),
        optional_tick(m.crisis_resolved_tick),
        m.crisis_assaults_launched.to_string(),
        m.crisis_assaults_resolved.to_string(),
        m.crisis_units_remaining.to_string(),
        m.crisis_status_packets_sent.to_string(),
        m.crisis_login_snapshots_sent.to_string(),
        m.crisis_duplicate_assaults.to_string(),
        m.personal_crisis_automatic_dusk_hordes.to_string(),
        m.crisis_invariants_ok.to_string(),
        m.safe_logout_scenario_mode.clone(),
        m.safe_logout_requests.to_string(),
        m.safe_logout_accepted.to_string(),
        m.safe_logout_rejected.to_string(),
        m.safe_logout_cancelled.to_string(),
        m.safe_logout_completed.to_string(),
        m.safe_logout_protected_sessions_started.to_string(),
        m.safe_logout_resumed.to_string(),
        m.safe_logout_protected_ticks_total.to_string(),
        m.safe_logout_ordinary_disconnects.to_string(),
        m.safe_logout_active_assault_disconnects.to_string(),
        m.safe_logout_status_packets_sent.to_string(),
        m.safe_logout_status_packets_duplicate_suppressed
            .to_string(),
        m.safe_logout_protected_input_rejections.to_string(),
        m.safe_logout_protected_damage_blocks.to_string(),
        m.safe_logout_protected_target_rejections.to_string(),
        m.safe_logout_queued_events_discarded.to_string(),
        m.safe_logout_invariant_recoveries.to_string(),
        m.safe_logout_run_key_mismatches.to_string(),
        m.safe_logout_timer_rebases.to_string(),
        m.safe_logout_stale_connection_events_rejected.to_string(),
        reason_counts_json(&m.safe_logout_rejection_reasons),
        reason_counts_json(&m.safe_logout_cancellation_reasons),
        reason_counts_json(&m.safe_logout_invariant_reasons),
        m.safe_logout_invariants_ok.to_string(),
    ];
    row.extend(balance_csv_row(m));
    row
}

fn escape_csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn write_csv(results: &[RunMetrics], path: &str) {
    let mut file = match File::create(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create {path}: {e}");
            return;
        }
    };

    let header = csv_header_fields().join(",");
    let _ = writeln!(file, "{header}");

    for m in results {
        let row = metrics_csv_row(m)
            .iter()
            .map(|value| escape_csv_cell(value))
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(file, "{row}");
    }

    println!("Wrote {path} ({} rows)", results.len());
}

fn write_json(results: &[RunMetrics], path: &str) {
    match serde_json::to_string_pretty(results) {
        Ok(json) => match File::create(path) {
            Ok(mut f) => {
                let _ = f.write_all(json.as_bytes());
                println!("Wrote {path}");
            }
            Err(e) => eprintln!("Failed to create {path}: {e}"),
        },
        Err(e) => eprintln!("Failed to serialize JSON: {e}"),
    }
}

const BALANCE_REPORT_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct NumericSummary {
    samples_with_value: usize,
    mean: Option<f64>,
    median: Option<f64>,
}

impl NumericSummary {
    fn from_values(mut values: Vec<f64>) -> Self {
        if values.is_empty() {
            return Self::default();
        }
        values.sort_by(f64::total_cmp);
        let samples_with_value = values.len();
        let mean = values.iter().sum::<f64>() / samples_with_value as f64;
        let middle = samples_with_value / 2;
        let median = if samples_with_value % 2 == 0 {
            (values[middle - 1] + values[middle]) / 2.0
        } else {
            values[middle]
        };
        Self {
            samples_with_value,
            mean: Some(mean),
            median: Some(median),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RateSummary {
    count: usize,
    sample_count: usize,
    rate: Option<f64>,
}

impl RateSummary {
    fn new(count: usize, sample_count: usize) -> Self {
        Self {
            count,
            sample_count,
            rate: (sample_count > 0).then_some(count as f64 / sample_count as f64),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PhaseDurationSummary {
    dormant: NumericSummary,
    signs: NumericSummary,
    pressure: NumericSummary,
    preparing: NumericSummary,
    assault_ready: NumericSummary,
    assault: NumericSummary,
    total: NumericSummary,
    online_before_launch: NumericSummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PressureContributorSummary {
    danger_unlocked: NumericSummary,
    structures: NumericSummary,
    villagers: NumericSummary,
    explore_poi: NumericSummary,
    choose_expansion: NumericSummary,
    stored_gold: NumericSummary,
    sanctuary: NumericSummary,
    online_time: NumericSummary,
    raw_total: NumericSummary,
    clamped_total: NumericSummary,
    dominant_contributor_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BalanceAggregate {
    total_runs: usize,
    quantitative_runs: usize,
    panic_count: usize,
    tick_cap_reached_count: usize,
    unresolved_at_tick_cap_count: usize,
    assault_launch_rate: RateSummary,
    assault_resolution_rate: RateSummary,
    hero_survival_rate: RateSummary,
    preparation_action_rate: RateSummary,
    signs_warning_delivery_rate: RateSummary,
    preparing_warning_delivery_rate: RateSummary,
    assault_ready_warning_delivery_rate: RateSummary,
    phase_durations: PhaseDurationSummary,
    signs_warning_to_launch_global: NumericSummary,
    signs_warning_to_launch_online: NumericSummary,
    preparing_warning_to_launch_online: NumericSummary,
    assault_ready_warning_to_launch_online: NumericSummary,
    assault_duration: NumericSummary,
    hero_damage: NumericSummary,
    hero_deaths: NumericSummary,
    villager_losses: NumericSummary,
    structure_damage: NumericSummary,
    structures_destroyed: NumericSummary,
    walls_destroyed: NumericSummary,
    pressure_contributors: PressureContributorSummary,
    automatic_dusk_hordes: i32,
    duplicate_assaults: i32,
    cross_player_target_violations: i32,
    crisis_invariant_failures: usize,
    safe_logout_invariant_recoveries: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BalanceSampleSummary {
    total_runs: usize,
    quantitative_runs: usize,
    panic_runs: usize,
    tick_cap_reached_runs: usize,
    unresolved_at_tick_cap_runs: usize,
    natural_progression_runs: usize,
    staged_progression_runs: usize,
    scenario_counts: BTreeMap<String, usize>,
    hero_class_counts: BTreeMap<String, usize>,
    tick_caps: BTreeMap<i32, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoblinCrisisBalanceReport {
    version: u32,
    generated_from_commit: String,
    working_tree_dirty: bool,
    balance_config: GoblinCrisisBalanceConfigSnapshot,
    sample_summary: BalanceSampleSummary,
    by_scenario: BTreeMap<String, BalanceAggregate>,
    by_scenario_cohort: BTreeMap<String, BalanceAggregate>,
    by_progression_cohort: BTreeMap<String, BalanceAggregate>,
    by_hero_class: BTreeMap<String, BalanceAggregate>,
    by_hero_class_cohort: BTreeMap<String, BalanceAggregate>,
    by_preparation: BTreeMap<String, BalanceAggregate>,
    by_preparation_policy: BTreeMap<String, BalanceAggregate>,
    by_villagers: BTreeMap<String, BalanceAggregate>,
    by_villagers_cohort: BTreeMap<String, BalanceAggregate>,
    by_connection: BTreeMap<String, BalanceAggregate>,
    by_helper: BTreeMap<String, BalanceAggregate>,
    pressure_analysis: PressureContributorSummary,
    phase_timing_analysis: PhaseDurationSummary,
    assault_outcomes: BalanceAggregate,
    invariants: BalanceInvariantSummary,
    limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BalanceInvariantSummary {
    automatic_dusk_hordes: i32,
    duplicate_assaults: i32,
    cross_player_target_violations: i32,
    crisis_invariant_failures: usize,
    safe_logout_invariant_recoveries: u64,
    panics: usize,
}

fn numeric_summary(
    runs: &[&RunMetrics],
    value: impl Fn(&RunMetrics) -> Option<f64>,
) -> NumericSummary {
    NumericSummary::from_values(runs.iter().filter_map(|run| value(run)).collect())
}

fn pressure_for_analysis(run: &RunMetrics) -> CrisisPressureBreakdown {
    run.crisis_balance
        .pressure_snapshots
        .assault_launch
        .as_ref()
        .or(run
            .crisis_balance
            .pressure_snapshots
            .final_snapshot
            .as_ref())
        .map(|snapshot| snapshot.breakdown)
        .unwrap_or(run.crisis_balance.latest_pressure)
}

fn pressure_summary(runs: &[&RunMetrics]) -> PressureContributorSummary {
    let contributor = |select: fn(CrisisPressureBreakdown) -> i32| {
        numeric_summary(runs, |run| Some(select(pressure_for_analysis(run)) as f64))
    };
    let mut dominant_contributor_counts = BTreeMap::new();
    for run in runs {
        if let Some(name) = pressure_for_analysis(run).dominant_contributor() {
            *dominant_contributor_counts
                .entry(name.to_string())
                .or_default() += 1;
        }
    }
    PressureContributorSummary {
        danger_unlocked: contributor(|value| value.danger_unlocked),
        structures: contributor(|value| value.structures),
        villagers: contributor(|value| value.villagers),
        explore_poi: contributor(|value| value.explore_poi),
        choose_expansion: contributor(|value| value.choose_expansion),
        stored_gold: contributor(|value| value.stored_gold),
        sanctuary: contributor(|value| value.sanctuary),
        online_time: contributor(|value| value.online_time),
        raw_total: contributor(|value| value.raw_total),
        clamped_total: contributor(|value| value.clamped_total),
        dominant_contributor_counts,
    }
}

fn phase_duration_summary(runs: &[&RunMetrics]) -> PhaseDurationSummary {
    PhaseDurationSummary {
        dormant: numeric_summary(runs, |run| {
            run.crisis_balance
                .phase_timing
                .dormant_duration()
                .map(f64::from)
        }),
        signs: numeric_summary(runs, |run| {
            run.crisis_balance
                .phase_timing
                .signs_duration()
                .map(f64::from)
        }),
        pressure: numeric_summary(runs, |run| {
            run.crisis_balance
                .phase_timing
                .pressure_duration()
                .map(f64::from)
        }),
        preparing: numeric_summary(runs, |run| {
            run.crisis_balance
                .phase_timing
                .preparing_duration()
                .map(f64::from)
        }),
        assault_ready: numeric_summary(runs, |run| {
            run.crisis_balance
                .phase_timing
                .assault_ready_duration()
                .map(f64::from)
        }),
        assault: numeric_summary(runs, |run| {
            run.crisis_balance
                .phase_timing
                .assault_duration()
                .map(f64::from)
        }),
        total: numeric_summary(runs, |run| {
            run.crisis_balance
                .phase_timing
                .total_crisis_duration()
                .map(f64::from)
        }),
        online_before_launch: numeric_summary(runs, |run| {
            run.crisis_balance
                .phase_timing
                .total_online_before_launch()
                .map(f64::from)
        }),
    }
}

fn aggregate_balance_runs(all_runs: &[&RunMetrics]) -> BalanceAggregate {
    let quantitative = all_runs
        .iter()
        .copied()
        .filter(|run| run.outcome != "Panic")
        .collect::<Vec<_>>();
    let launched = quantitative
        .iter()
        .copied()
        .filter(|run| run.crisis_balance.assault_outcome.assault_launched)
        .collect::<Vec<_>>();
    let prepared = quantitative
        .iter()
        .copied()
        .filter(|run| run.crisis_balance.preparation_snapshots.preparing.is_some())
        .collect::<Vec<_>>();
    let signs = quantitative
        .iter()
        .copied()
        .filter(|run| run.crisis_balance.phase_timing.signs_entered_tick.is_some())
        .collect::<Vec<_>>();
    let resolution_count = launched
        .iter()
        .filter(|run| run.crisis_balance.assault_outcome.assault_resolved)
        .count();
    let hero_survival_observations = launched
        .iter()
        .filter(|run| {
            run.crisis_balance
                .assault_outcome
                .hero_alive_at_resolution
                .is_some()
        })
        .count();
    let hero_survival_count = launched
        .iter()
        .filter(|run| run.crisis_balance.assault_outcome.hero_alive_at_resolution == Some(true))
        .count();
    let preparation_action_count = prepared
        .iter()
        .filter(|run| {
            run.crisis_balance
                .preparation_actions
                .performed_preparation_action
        })
        .count();
    let ready = quantitative
        .iter()
        .copied()
        .filter(|run| {
            run.crisis_balance
                .phase_timing
                .assault_ready_entered_tick
                .is_some()
        })
        .collect::<Vec<_>>();

    BalanceAggregate {
        total_runs: all_runs.len(),
        quantitative_runs: quantitative.len(),
        panic_count: all_runs.iter().filter(|run| run.outcome == "Panic").count(),
        tick_cap_reached_count: all_runs
            .iter()
            .filter(|run| run.crisis_balance_tick_cap_reached)
            .count(),
        unresolved_at_tick_cap_count: all_runs
            .iter()
            .filter(|run| {
                run.crisis_balance_tick_cap_reached
                    && !run.crisis_balance.assault_outcome.assault_resolved
            })
            .count(),
        assault_launch_rate: RateSummary::new(launched.len(), quantitative.len()),
        assault_resolution_rate: RateSummary::new(resolution_count, launched.len()),
        hero_survival_rate: RateSummary::new(hero_survival_count, hero_survival_observations),
        preparation_action_rate: RateSummary::new(preparation_action_count, prepared.len()),
        signs_warning_delivery_rate: RateSummary::new(
            signs
                .iter()
                .filter(|run| run.crisis_balance.warnings.signs_delivery_tick.is_some())
                .count(),
            signs.len(),
        ),
        preparing_warning_delivery_rate: RateSummary::new(
            prepared
                .iter()
                .filter(|run| {
                    run.crisis_balance
                        .warnings
                        .preparing_delivery_tick
                        .is_some()
                })
                .count(),
            prepared.len(),
        ),
        assault_ready_warning_delivery_rate: RateSummary::new(
            ready
                .iter()
                .filter(|run| {
                    run.crisis_balance
                        .warnings
                        .assault_ready_delivery_tick
                        .is_some()
                })
                .count(),
            ready.len(),
        ),
        phase_durations: phase_duration_summary(&quantitative),
        signs_warning_to_launch_global: numeric_summary(&quantitative, |run| {
            run.crisis_warning_signs_to_launch_global_ticks
                .map(f64::from)
        }),
        signs_warning_to_launch_online: numeric_summary(&quantitative, |run| {
            run.crisis_warning_signs_to_launch_online_ticks
                .map(f64::from)
        }),
        preparing_warning_to_launch_online: numeric_summary(&quantitative, |run| {
            run.crisis_balance
                .warnings
                .preparing_to_launch_online_ticks()
                .map(f64::from)
        }),
        assault_ready_warning_to_launch_online: numeric_summary(&quantitative, |run| {
            run.crisis_balance
                .warnings
                .ready_to_launch_online_ticks()
                .map(f64::from)
        }),
        assault_duration: numeric_summary(&launched, |run| {
            run.crisis_balance
                .assault_outcome
                .assault_duration_ticks
                .map(f64::from)
        }),
        hero_damage: numeric_summary(&launched, |run| {
            Some(run.crisis_balance.assault_outcome.hero_damage_taken as f64)
        }),
        hero_deaths: numeric_summary(&launched, |run| {
            Some(
                run.crisis_balance
                    .assault_outcome
                    .hero_deaths_during_assault as f64,
            )
        }),
        villager_losses: numeric_summary(&launched, |run| {
            Some(run.crisis_balance.assault_outcome.villagers_killed as f64)
        }),
        structure_damage: numeric_summary(&launched, |run| {
            Some(run.crisis_balance.assault_outcome.total_structure_damage as f64)
        }),
        structures_destroyed: numeric_summary(&launched, |run| {
            Some(run.crisis_balance.assault_outcome.structures_destroyed as f64)
        }),
        walls_destroyed: numeric_summary(&launched, |run| {
            Some(run.crisis_balance.assault_outcome.wall_segments_destroyed as f64)
        }),
        pressure_contributors: pressure_summary(&quantitative),
        automatic_dusk_hordes: all_runs
            .iter()
            .map(|run| run.personal_crisis_automatic_dusk_hordes)
            .sum(),
        duplicate_assaults: all_runs
            .iter()
            .map(|run| run.crisis_duplicate_assaults)
            .sum(),
        cross_player_target_violations: all_runs
            .iter()
            .map(|run| {
                run.crisis_balance
                    .assault_outcome
                    .cross_player_target_violations
            })
            .sum(),
        crisis_invariant_failures: all_runs
            .iter()
            .filter(|run| run.outcome != "Panic" && !run.crisis_invariants_ok)
            .count(),
        safe_logout_invariant_recoveries: all_runs
            .iter()
            .map(|run| run.safe_logout_invariant_recoveries)
            .sum(),
    }
}

fn grouped_aggregates(
    results: &[RunMetrics],
    group: impl Fn(&RunMetrics) -> String,
) -> BTreeMap<String, BalanceAggregate> {
    let mut grouped = BTreeMap::<String, Vec<&RunMetrics>>::new();
    for run in results {
        grouped.entry(group(run)).or_default().push(run);
    }
    grouped
        .into_iter()
        .map(|(label, runs)| (label, aggregate_balance_runs(&runs)))
        .collect()
}

const fn progression_cohort_label(run: &RunMetrics) -> &'static str {
    if run.crisis_balance_progression_fixture {
        "staged_attainable_facts"
    } else {
        "natural_progression"
    }
}

fn build_balance_report(results: &[RunMetrics]) -> GoblinCrisisBalanceReport {
    let all = results.iter().collect::<Vec<_>>();
    let aggregate = aggregate_balance_runs(&all);
    let mut sample_summary = BalanceSampleSummary {
        total_runs: results.len(),
        quantitative_runs: results.iter().filter(|run| run.outcome != "Panic").count(),
        panic_runs: results.iter().filter(|run| run.outcome == "Panic").count(),
        tick_cap_reached_runs: results
            .iter()
            .filter(|run| run.crisis_balance_tick_cap_reached)
            .count(),
        unresolved_at_tick_cap_runs: results
            .iter()
            .filter(|run| {
                run.crisis_balance_tick_cap_reached
                    && !run.crisis_balance.assault_outcome.assault_resolved
            })
            .count(),
        natural_progression_runs: results
            .iter()
            .filter(|run| !run.crisis_balance_progression_fixture)
            .count(),
        staged_progression_runs: results
            .iter()
            .filter(|run| run.crisis_balance_progression_fixture)
            .count(),
        ..Default::default()
    };
    for run in results {
        *sample_summary
            .scenario_counts
            .entry(run.crisis_balance_scenario.clone())
            .or_default() += 1;
        *sample_summary
            .hero_class_counts
            .entry(run.crisis_balance_hero_class.clone())
            .or_default() += 1;
        *sample_summary
            .tick_caps
            .entry(run.crisis_balance_tick_cap)
            .or_default() += 1;
    }

    let generated_from_commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let working_tree_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(true);

    GoblinCrisisBalanceReport {
        version: BALANCE_REPORT_VERSION,
        generated_from_commit,
        working_tree_dirty,
        balance_config: results
            .first()
            .map(|run| run.crisis_balance_config.clone())
            .unwrap_or_else(siege_perilous::game::goblin_crisis_balance_config_snapshot),
        sample_summary,
        by_scenario: grouped_aggregates(results, |run| run.crisis_balance_scenario.clone()),
        by_scenario_cohort: grouped_aggregates(results, |run| {
            format!(
                "{} / {}",
                run.crisis_balance_scenario,
                progression_cohort_label(run)
            )
        }),
        by_progression_cohort: grouped_aggregates(results, |run| {
            progression_cohort_label(run).to_string()
        }),
        by_hero_class: grouped_aggregates(results, |run| run.crisis_balance_hero_class.clone()),
        by_hero_class_cohort: grouped_aggregates(results, |run| {
            format!(
                "{} / {}",
                run.crisis_balance_hero_class,
                progression_cohort_label(run)
            )
        }),
        by_preparation: grouped_aggregates(results, |run| {
            let observation = if run
                .crisis_balance
                .preparation_snapshots
                .preparing
                .is_none()
            {
                "preparing_not_observed"
            } else if run
                .crisis_balance
                .preparation_actions
                .performed_preparation_action
            {
                "observed_preparation_action"
            } else {
                "no_observed_preparation_action"
            };
            format!("{} / {}", observation, progression_cohort_label(run))
        }),
        by_preparation_policy: grouped_aggregates(results, |run| {
            let policy = CrisisBalanceScenario::from_label(&run.crisis_balance_scenario)
                .map(CrisisBalanceScenario::prepared_group)
                .unwrap_or("unknown");
            format!("{} / {}", policy, progression_cohort_label(run))
        }),
        by_villagers: grouped_aggregates(results, |run| {
            run.crisis_balance
                .preparation_snapshots
                .assault_launch
                .as_ref()
                .map(|snapshot| {
                    if snapshot.villagers_alive > 0 {
                        "villagers_at_launch"
                    } else {
                        "no_villagers_at_launch"
                    }
                })
                .unwrap_or("launch_not_observed")
                .to_string()
        }),
        by_villagers_cohort: grouped_aggregates(results, |run| {
            let villagers = run
                .crisis_balance
                .preparation_snapshots
                .assault_launch
                .as_ref()
                .map(|snapshot| {
                    if snapshot.villagers_alive > 0 {
                        "villagers_at_launch"
                    } else {
                        "no_villagers_at_launch"
                    }
                })
                .unwrap_or("launch_not_observed");
            format!("{villagers} / {}", progression_cohort_label(run))
        }),
        by_connection: grouped_aggregates(results, |run| {
            let outcome = &run.crisis_balance.assault_outcome;
            if !outcome.assault_launched && outcome.safe_logout_before_assault {
                "safe_logout_before_launch_no_assault"
            } else if !outcome.assault_launched {
                "no_assault"
            } else if outcome.ordinary_disconnect_during_assault {
                "ordinary_disconnect_during_assault"
            } else if outcome.safe_logout_before_assault {
                "safe_logout_before_launch_then_assault"
            } else {
                "connected_through_assault"
            }
            .to_string()
        }),
        by_helper: grouped_aggregates(results, |run| {
            if run.crisis_balance.assault_outcome.helper_participated {
                "helper_participated"
            } else {
                "no_helper_observed"
            }
            .to_string()
        }),
        pressure_analysis: aggregate.pressure_contributors.clone(),
        phase_timing_analysis: aggregate.phase_durations.clone(),
        assault_outcomes: aggregate.clone(),
        invariants: BalanceInvariantSummary {
            automatic_dusk_hordes: aggregate.automatic_dusk_hordes,
            duplicate_assaults: aggregate.duplicate_assaults,
            cross_player_target_violations: aggregate.cross_player_target_violations,
            crisis_invariant_failures: aggregate.crisis_invariant_failures,
            safe_logout_invariant_recoveries: aggregate.safe_logout_invariant_recoveries,
            panics: aggregate.panic_count,
        },
        limitations: vec![
            format!("This bounded report contains {} rows. A {}-row base cycle provides exactly one observation for each of {} driver-variant × 3 hero-class cells; every row records its own tick cap and repetition. Rows ending in MaxTicks reached the overall run cap and may already have resolved the crisis, so unresolved-at-cap is reported separately.", results.len(), BalanceRunSpec::COMBINATIONS, BALANCE_DRIVER_VARIANTS.len()),
            "The game uses thread_rng in production systems; run identifiers are deterministic matrix identifiers, not RNG seeds.".to_string(),
            "Staged prepared, fortified, villager, disconnect, Safe Logout, and helper-supported rows use a transparent headless-only progression fixture: existing objectives are complete, the nearest monolith is relocated to the base and set to sanctuary level 3, and existing Logs and Gold Coins are supplied. The bot must still build through player events before authoritative pressure, phase minima, launch, spawn, and combat run normally. Natural variants remain separate; staged rows are not evidence of natural launch rate or organic preparation, and monolith relocation changes the settlement anchor and assault spawn geometry.".to_string(),
            "A singular dominant-pressure label uses first-declared contributor order to break equal-value ties; the full contributor vector remains available.".to_string(),
            "The headless bot is deterministic but uses a predominantly melee combat policy, which can bias Ranger and Mage comparisons.".to_string(),
            "The staged helper-supported row uses a real connected Warrior driven through ordinary movement and combat events toward the owner's settlement. Personal assault units intentionally cannot target the non-owner helper, so this is a low-risk assistance probe rather than a fair second combat target. The adjacent-settlement scenario remains omitted from this balance matrix; existing isolation regressions remain the evidence for it.".to_string(),
            "Preparation actions are derived from bounded state deltas; crafting intent and every transient inventory transfer are not reconstructed.".to_string(),
            "Near/away preparation time is interval-sampled: each elapsed interval is assigned wholly to the location observed at its endpoint (600 ticks in this matrix), so it is directional rather than an exact movement trace.".to_string(),
            "The prepared policies can return home, equip an available non-hunting weapon, build existing walls, and upgrade the sanctuary, but the bot has no explicit armor-selection or structure-repair driver.".to_string(),
            "The Safe Logout setup helper repositions the hero and every currently alive, visible-target NPC and rebases headless recent-combat/damage observations beyond the unchanged production cooldown. Later spawns or new damage can still reject or cancel, and their typed telemetry remains in the ordinary run row. Comparison with prepared-solo is therefore a lifecycle probe rather than a perfectly paired balance experiment.".to_string(),
            "Ordinary crisis attackers currently damage owner units and walls; ordinary non-wall structures are not normal attack targets, limiting structure-damage observations.".to_string(),
            "The run-associated Shipwreck's Health Potion is overridden to Healing 10 even though the item template declares 50; fresh heroes begin with equipped Tattered Shirt and Tattered Pants only, and must recover the potion manually.".to_string(),
            "A passive run has at most 25 pressure from danger unlock and online time, so it can enter Signs but cannot naturally reach Pressure under the current formula.".to_string(),
            "Warning timestamps represent the first successfully sent crisis status packet for the phase, not client rendering acknowledgement.".to_string(),
            "Control and candidate reports must use identical scenario order, repetitions, and tick caps. Production thread_rng still makes them independent repeated samples rather than paired deterministic seeds.".to_string(),
        ],
    }
}

#[cfg(test)]
fn display_rate(summary: &RateSummary) -> String {
    match summary.rate {
        Some(rate) => format!(
            "{:.1}% ({}/{})",
            rate * 100.0,
            summary.count,
            summary.sample_count
        ),
        None => format!("n/a ({}/{})", summary.count, summary.sample_count),
    }
}

#[cfg(test)]
fn display_numeric(summary: &NumericSummary) -> String {
    match (summary.mean, summary.median) {
        (Some(mean), Some(median)) => format!(
            "mean {:.1}, median {:.1} (n={})",
            mean, median, summary.samples_with_value
        ),
        _ => "n/a (n=0)".to_string(),
    }
}

#[cfg(test)]
fn aggregate_table(title: &str, groups: &BTreeMap<String, BalanceAggregate>) -> String {
    let mut output = format!(
        "### {title}\n\n| Group | Runs | Launch | Resolution after launch | Hero alive at resolution | Assault duration | Hero damage | Hero deaths | Structure damage |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|\n"
    );
    for (label, aggregate) in groups {
        output.push_str(&format!(
            "| {} | {} ({} quantitative) | {} | {} | {} | {} | {} | {} | {} |\n",
            label,
            aggregate.total_runs,
            aggregate.quantitative_runs,
            display_rate(&aggregate.assault_launch_rate),
            display_rate(&aggregate.assault_resolution_rate),
            display_rate(&aggregate.hero_survival_rate),
            display_numeric(&aggregate.assault_duration),
            display_numeric(&aggregate.hero_damage),
            display_numeric(&aggregate.hero_deaths),
            display_numeric(&aggregate.structure_damage),
        ));
    }
    output.push('\n');
    output
}

#[cfg(test)]
fn aggregate_finding(aggregate: Option<&BalanceAggregate>, description: &str) -> String {
    match aggregate {
        Some(aggregate) if aggregate.assault_launch_rate.count > 0 => format!(
            "Confirmed for this bounded sample — {description}: launch {}, assault resolution {}, hero alive at resolution {}.",
            display_rate(&aggregate.assault_launch_rate),
            display_rate(&aggregate.assault_resolution_rate),
            display_rate(&aggregate.hero_survival_rate),
        ),
        Some(aggregate) => format!(
            "Insufficient assault data — {description}: no assault launched in {} quantitative runs (launch {}).",
            aggregate.quantitative_runs,
            display_rate(&aggregate.assault_launch_rate),
        ),
        None => format!("Insufficient data — no {description} runs were executed."),
    }
}

#[cfg(test)]
fn scenario_cohort_finding(
    report: &GoblinCrisisBalanceReport,
    scenario: &str,
    progression_fixture: bool,
    description: &str,
) -> String {
    let cohort = if progression_fixture {
        "staged_attainable_facts"
    } else {
        "natural_progression"
    };
    let key = format!("{scenario} / {cohort}");
    aggregate_finding(report.by_scenario_cohort.get(&key), description)
}

#[cfg(test)]
fn scenario_cohort_measurement(
    report: &GoblinCrisisBalanceReport,
    scenario: &str,
    progression_fixture: bool,
    description: &str,
) -> String {
    let cohort = if progression_fixture {
        "staged_attainable_facts"
    } else {
        "natural_progression"
    };
    let key = format!("{scenario} / {cohort}");
    match report.by_scenario_cohort.get(&key) {
        Some(aggregate) if aggregate.assault_launch_rate.count > 0 => format!(
            "Measured in this bounded lifecycle sample — {description}: launch {}, assault resolution {}, hero alive at resolution {}.",
            display_rate(&aggregate.assault_launch_rate),
            display_rate(&aggregate.assault_resolution_rate),
            display_rate(&aggregate.hero_survival_rate),
        ),
        Some(aggregate) => format!(
            "Measured but without a launched assault — {description}: {} quantitative runs (launch {}).",
            aggregate.quantitative_runs,
            display_rate(&aggregate.assault_launch_rate),
        ),
        None => format!("Not measured — no {description} runs were executed."),
    }
}

#[cfg(test)]
fn safe_logout_probe_summary(report: &GoblinCrisisBalanceReport) -> String {
    let completed = [
        "safe_logout_before_launch_no_assault",
        "safe_logout_before_launch_then_assault",
    ]
    .into_iter()
    .filter_map(|key| report.by_connection.get(key))
    .map(|aggregate| aggregate.quantitative_runs)
    .sum::<usize>();
    if completed > 0 {
        format!(
            "The staged matrix contains {completed} completed pre-launch Safe Logout lifecycle sample(s), so freeze/resume was exercised; the bounded sample still cannot establish natural balance equivalence."
        )
    } else {
        "No staged row completed pre-launch Safe Logout, so this matrix provides no freeze/resume balance sample; rejection/cancellation telemetry and focused Safe Logout regressions remain visible, but balance equivalence is unmeasured.".to_string()
    }
}

#[cfg(test)]
fn pressure_dominance(summary: &PressureContributorSummary) -> String {
    if summary.dominant_contributor_counts.is_empty() {
        "none observed".to_string()
    } else {
        summary
            .dominant_contributor_counts
            .iter()
            .map(|(name, count)| format!("{name}={count}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
fn phase_duration_line(summary: &PhaseDurationSummary) -> String {
    format!(
        "Dormant {}; Signs {}; Pressure {}; Preparing {}; AssaultReady {}; Assault {}",
        display_numeric(&summary.dormant),
        display_numeric(&summary.signs),
        display_numeric(&summary.pressure),
        display_numeric(&summary.preparing),
        display_numeric(&summary.assault_ready),
        display_numeric(&summary.assault),
    )
}

#[cfg(test)]
fn render_balance_markdown(report: &GoblinCrisisBalanceReport) -> String {
    let overall = &report.assault_outcomes;
    let config_json = serde_json::to_string_pretty(&report.balance_config)
        .unwrap_or_else(|_| "{\"error\":\"configuration serialization failed\"}".to_string());
    let natural = report.by_progression_cohort.get("natural_progression");
    let staged = report.by_progression_cohort.get("staged_attainable_facts");
    let staged_assault = staged.unwrap_or(overall);
    let mut output = format!(
        "# Goblin Crisis Balance Baseline\n\nCheckpoint 1 measures the current goblin crisis and intentionally does not make material balance changes.\n\nThis report was generated from commit `{}`{} using report schema version {}. An assault \"win\" below means authoritative transition to `Resolved`; whether the hero was alive at resolution and the number of assault-time deaths are reported separately. Panics are retained in counts but excluded from quantitative means and rates. Overall tick-cap reaches and unresolved-at-cap rows remain visible and are not silently discarded; a resolved crisis may continue to the overall cap.\n\n## Sample summary\n\n- Runs: {} total, {} quantitative, {} panics, {} reached the overall tick cap, {} remained crisis-unresolved at that cap.\n- Progression cohorts: {} natural-progression runs and {} staged-attainable-facts runs.\n- Scenario samples: `{}`.\n- Hero-class samples: `{}`.\n- Tick caps: `{}`.\n\n",
        report.generated_from_commit,
        if report.working_tree_dirty {
            " from a dirty working tree"
        } else {
            ""
        },
        report.version,
        report.sample_summary.total_runs,
        report.sample_summary.quantitative_runs,
        report.sample_summary.panic_runs,
        report.sample_summary.tick_cap_reached_runs,
        report.sample_summary.unresolved_at_tick_cap_runs,
        report.sample_summary.natural_progression_runs,
        report.sample_summary.staged_progression_runs,
        serde_json::to_string(&report.sample_summary.scenario_counts).unwrap_or_default(),
        serde_json::to_string(&report.sample_summary.hero_class_counts).unwrap_or_default(),
        serde_json::to_string(&report.sample_summary.tick_caps).unwrap_or_default(),
    );

    output.push_str("## Exact current configuration\n\nThe snapshot below is serialized from the constants used by the authoritative crisis implementation; it is not a runtime tuning interface.\n\n```json\n");
    output.push_str(&config_json);
    output.push_str("\n```\n\n");

    output.push_str("The snapshot covers the crisis-owned constants. The following architecture-audited runtime values are also part of the current baseline and are intentionally unchanged. The personal wave overrides both attackers' viewsheds to 14.\n\n| Assault unit | Count | HP | Stamina | Damage / span | Defence | Speed | Template vision | Personal-wave vision | Kill XP |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n| Wolf Rider | 2 | 75 | 250 | 6 / 5 | 5 | 6 | 4 | 14 | 300 |\n| Goblin Pillager | 1 | 55 | 200 | 5 / 4 | 4 | 5 | 3 | 14 | 250 |\n\nHuman Villagers have 500 HP, 10,000 stamina, zero base damage/span, zero defence, zero speed, vision 2, and base work 25. They count as combat-capable only when current base damage is positive or a weapon is equipped.\n\n| Existing defence | HP | Defence | Current role |\n|---|---:|---:|---|\n| Stockade | 20 | 0 | blocking level-0 wall |\n| Palisade | 200 | 0 | blocking level-1 wall |\n| Fieldstone Walls | 400 | 0 | blocking level-2 wall |\n| Watchtower | 50 | 0 | vision/light support; not a wall |\n\nThe sanctuary maximum is level 5; upgrade costs are 3, 6, 9, 12, and 15 Soulshards; full and weak radii are `3 + level` and `5 + level`; each level contributes 0.25 to the existing defence amplifier. Full audit context, including anchor priority, target eligibility, equipment, and the runtime Health Potion/template discrepancy, is recorded in `docs/goblin_crisis_balance_milestone.md`.\n\n");

    output.push_str("## Hero-class starting baseline\n\nThese are architecture-confirmed values from the current hero templates and revised setup path. Every fresh hero begins with only equipped Tattered Shirt and Tattered Pants. The run-associated Shipwreck holds the shared starter-only Sharpened Stick (including Logging 1), one custom 10-point Health Potion, the other common supplies, and the class salvage below; the bot recovers them through ordinary investigation and item-transfer events.\n\n| Class | HP | Stamina | Mana | Base damage / span | Defence | Speed | Vision | Hero inventory at spawn | Shipwreck class salvage |\n|---|---:|---:|---:|---:|---:|---:|---:|---|---|\n| Warrior | 110 | 110 | 0 | 2 / 2 | 4 | 5 | 3 | Tattered Shirt and Tattered Pants | Copper Helm (+3 defence), plus the shared Sharpened Stick |\n| Ranger | 80 | 120 | 0 | 1 / 3 | 1 | 7 | 5 | Tattered Shirt and Tattered Pants | Training Bow (8 damage, range 2, 85 accuracy), plus the shared Sharpened Stick |\n| Mage | 60 | 100 | 100 | 1 / 2 | 0 | 5 | 4 | Tattered Shirt and Tattered Pants | 5 Mana items, plus the shared Sharpened Stick |\n\n");

    output.push_str("## Aggregate results\n\n");
    output.push_str("Natural-progression rows observe the existing starting economy and bot path. `staged_attainable_facts` rows are separate assault probes: their headless-only fixture supplies attainable existing facts and resources, then leaves authoritative pressure, phase gates, launch, spawning, and combat unchanged. Staged rows are not evidence of the natural launch rate or of an organic preparation path.\n\n");
    output.push_str(&aggregate_table(
        "By scenario and progression cohort",
        &report.by_scenario_cohort,
    ));
    output.push_str(&aggregate_table(
        "Natural versus staged progression cohort",
        &report.by_progression_cohort,
    ));
    output.push_str(&aggregate_table(
        "By hero class and progression cohort",
        &report.by_hero_class_cohort,
    ));
    output.push_str(&aggregate_table(
        "Observed preparation actions by progression cohort",
        &report.by_preparation,
    ));
    output.push_str(&aggregate_table(
        "Prepared-policy versus unprepared-policy by progression cohort",
        &report.by_preparation_policy,
    ));
    output.push_str(&aggregate_table(
        "Villagers versus no villagers by progression cohort",
        &report.by_villagers_cohort,
    ));
    output.push_str(&aggregate_table("Connection state", &report.by_connection));
    output.push_str(&aggregate_table("Helper versus solo", &report.by_helper));

    output.push_str("## Required baseline questions\n\n");
    output.push_str("1. **What is the exact current crisis configuration?** Confirmed — the configuration snapshot above is derived from the authoritative constants. Pressure is capped at 100; thresholds are 20/45/70/90; phase-online minima are 0/600/1,200/1,800 ticks; ready grace is 300 online ticks; maximum ready wait is 1,200 online ticks; the wave remains two Wolf Riders and one Goblin Pillager.\n\n");
    output.push_str(&format!("2. **Which pressure contributors dominate?** Natural-progression dominant-at-analysis counts were `{}`; staged-attainable-facts counts were `{}`. These cohorts are reported separately because fixture-supplied facts deliberately change the contributor distribution. Contributor means are available in the machine report; architecture alone shows structures are the largest single fixed contributor at +20.\n\n", natural.map(|aggregate| pressure_dominance(&aggregate.pressure_contributors)).unwrap_or_else(|| "none observed".to_string()), staged.map(|aggregate| pressure_dominance(&aggregate.pressure_contributors)).unwrap_or_else(|| "none observed".to_string())));
    output.push_str(&format!("3. **How long does each phase last?** Natural progression: {}. Staged attainable facts: {}. Missing transitions remain absent rather than being converted to zero; staged timing measures production phase gates after controlled setup, not natural time-to-preparation.\n\n", natural.map(|aggregate| phase_duration_line(&aggregate.phase_durations)).unwrap_or_else(|| "no samples".to_string()), staged.map(|aggregate| phase_duration_line(&aggregate.phase_durations)).unwrap_or_else(|| "no samples".to_string())));
    output.push_str(&format!("4. **How much online preparation time exists?** Natural progression online-before-launch: {}; staged attainable-facts online-before-launch: {}. Staged Signs-warning-to-launch lead was {} in global ticks and {} in online-active ticks; Preparing-warning-to-launch lead was {}; AssaultReady-warning-to-launch lead was {}.\n\n", natural.map(|aggregate| display_numeric(&aggregate.phase_durations.online_before_launch)).unwrap_or_else(|| "n/a (n=0)".to_string()), staged.map(|aggregate| display_numeric(&aggregate.phase_durations.online_before_launch)).unwrap_or_else(|| "n/a (n=0)".to_string()), staged.map(|aggregate| display_numeric(&aggregate.signs_warning_to_launch_global)).unwrap_or_else(|| "n/a (n=0)".to_string()), staged.map(|aggregate| display_numeric(&aggregate.signs_warning_to_launch_online)).unwrap_or_else(|| "n/a (n=0)".to_string()), staged.map(|aggregate| display_numeric(&aggregate.preparing_warning_to_launch_online)).unwrap_or_else(|| "n/a (n=0)".to_string()), staged.map(|aggregate| display_numeric(&aggregate.assault_ready_warning_to_launch_online)).unwrap_or_else(|| "n/a (n=0)".to_string())));
    output.push_str(&format!("5. **How often does the assault launch?** Natural progression: {}; staged attainable-facts probe: {}. The staged rate is a harness-success measure, not a natural launch probability.\n\n", natural.map(|aggregate| display_rate(&aggregate.assault_launch_rate)).unwrap_or_else(|| "n/a (0/0)".to_string()), staged.map(|aggregate| display_rate(&aggregate.assault_launch_rate)).unwrap_or_else(|| "n/a (0/0)".to_string())));
    output.push_str(&format!(
        "6. **How often does a passive player win?** {}\n\n",
        scenario_cohort_finding(report, "passive", false, "natural passive")
    ));
    output.push_str(&format!(
        "7. **How often does a basically competent player win?** {}\n\n",
        scenario_cohort_finding(report, "basic_survival", false, "natural basic-survival")
    ));
    output.push_str(&format!(
        "8. **How often does a deliberately prepared solo player win?** {}\n\n",
        format!("{} The separate staged combat probe reports: {} Staged results do not establish an organic preparation or natural launch rate.", scenario_cohort_finding(report, "prepared_solo", false, "natural prepared-solo"), scenario_cohort_finding(report, "prepared_solo", true, "staged prepared-solo"))
    ));

    let villager_comparison = match (
        report
            .by_villagers_cohort
            .get("villagers_at_launch / staged_attainable_facts"),
        report
            .by_villagers_cohort
            .get("no_villagers_at_launch / staged_attainable_facts"),
    ) {
        (Some(with), Some(without)) if with.assault_launch_rate.count > 0 && without.assault_launch_rate.count > 0 => format!("Observed within staged attainable-facts rows, not causal and not an organic preparation comparison: villager resolution {} versus no-villager {}; hero alive at resolution {} versus {}.", display_rate(&with.assault_resolution_rate), display_rate(&without.assault_resolution_rate), display_rate(&with.hero_survival_rate), display_rate(&without.hero_survival_rate)),
        _ => "Insufficient comparable launched samples; no causal villager effect or organic preparation effect can be claimed.".to_string(),
    };
    output.push_str(&format!(
        "9. **How much do villagers improve outcomes?** {villager_comparison}\n\n"
    ));
    let wall_comparison = match (
        report
            .by_scenario_cohort
            .get("fortified_solo / staged_attainable_facts"),
        report
            .by_scenario_cohort
            .get("prepared_solo / staged_attainable_facts"),
    ) {
        (Some(fortified), Some(prepared)) if fortified.assault_launch_rate.count > 0 && prepared.assault_launch_rate.count > 0 => format!("Observed within staged attainable-facts rows, not causal: fortified resolution {} versus prepared {}; structure damage {} versus {}. The policies differ by their wall cap (six versus three), but actual achieved state, fixture geometry, and random world events may also differ, so this is directional staged evidence only.", display_rate(&fortified.assault_resolution_rate), display_rate(&prepared.assault_resolution_rate), display_numeric(&fortified.structure_damage), display_numeric(&prepared.structure_damage)),
        _ => "Insufficient comparable staged launched samples. The current scenario driver does not isolate wall count as its only variable.".to_string(),
    };
    output.push_str(&format!(
        "10. **How much do walls improve outcomes?** {wall_comparison}\n\n"
    ));
    output.push_str(&format!("11. **How much damage does the settlement take?** Across launched staged assaults: structure damage {}; villager damage is retained per run in JSON; villager losses {}. Natural-progression assault outcomes, if any, remain separately visible in the cohort tables.\n\n", display_numeric(&staged_assault.structure_damage), display_numeric(&staged_assault.villager_losses)));
    output.push_str(&format!("12. **Are structures routinely destroyed?** In launched staged assaults, observed structures destroyed {}; walls destroyed {}. Ordinary personal-crisis attackers currently target owner units and walls rather than ordinary non-wall structures, so this metric cannot support a broad conclusion about every structure type or natural preparation.\n\n", display_numeric(&staged_assault.structures_destroyed), display_numeric(&staged_assault.walls_destroyed)));
    output.push_str("13. **Are any hero classes structurally disadvantaged?** The class table reports the current starting asymmetry, and the class/cohort aggregates above separate natural progression from staged combat probes. Any class with zero launched or resolved staged samples remains insufficient data; even launched staged rows cannot establish natural solo viability, and the melee-biased bot especially limits Ranger/Mage interpretation.\n\n");
    output.push_str(&format!(
        "14. **Does the assault remain solo-completable?** {} A separate combat probe reports: {} This can show whether the unchanged wave can resolve under staged attainable facts, but it cannot establish organic solo-completability.\n\n",
        scenario_cohort_finding(report, "prepared_solo", false, "natural prepared-solo"),
        scenario_cohort_finding(report, "prepared_solo", true, "staged prepared-solo")
    ));
    output.push_str(&format!("15. **Does ordinary disconnect create an advantage?** {} This is a staged lifecycle probe. Compare only directionally with staged prepared-solo because connection timing and combat exposure differ; no natural-progression advantage is established.\n\n", scenario_cohort_measurement(report, "ordinary_disconnect", true, "staged ordinary-disconnect")));
    output.push_str(&format!(
        "16. **Does Safe Logout before launch alter later balance?** {} {}\n\n",
        scenario_cohort_measurement(
            report,
            "safe_logout_before_assault",
            true,
            "staged Safe-Logout-before-assault"
        ),
        safe_logout_probe_summary(report)
    ));
    let helper_summary = if report
        .by_scenario_cohort
        .contains_key("helper_supported / staged_attainable_facts")
    {
        format!(
            "{} The helper is a real connected Warrior driven through ordinary Move and Attack events, but personal attackers intentionally cannot target that non-owner helper. This low-risk staged probe cannot establish that helpers trivialize an organic assault.",
            scenario_cohort_measurement(
                report,
                "helper_supported",
                true,
                "staged helper-supported",
            )
        )
    } else {
        "Insufficient data — no helper-supported matrix row was executed. Attribution fields and focused tests still cover player/villager/helper classification.".to_string()
    };
    output.push_str(&format!(
        "17. **Can helpers trivialize the assault?** {helper_summary}\n\n"
    ));
    output.push_str("18. **Are adjacent settlements isolated correctly?** Insufficient new balance-matrix data — the adjacent-settlement scenario was deliberately omitted. Existing crisis isolation regressions remain the behavioral evidence; cross-player target violations are reported as an invariant.\n\n");
    output.push_str(&format!("19. **Are warnings delivered with useful lead time?** Staged Signs delivery {}; staged Preparing delivery {}; staged AssaultReady delivery {}. Staged Signs-warning-to-launch lead was {} in global ticks and {} in online-active ticks; later online-active lead times were {} from Preparing and {} from AssaultReady. Natural rows that never reached those phases or never launched provide no warning-lead sample. Whether staged server-delivery values are *useful* remains a likely finding only until natural and human-play validation exists.\n\n", staged.map(|aggregate| display_rate(&aggregate.signs_warning_delivery_rate)).unwrap_or_else(|| "n/a (0/0)".to_string()), staged.map(|aggregate| display_rate(&aggregate.preparing_warning_delivery_rate)).unwrap_or_else(|| "n/a (0/0)".to_string()), staged.map(|aggregate| display_rate(&aggregate.assault_ready_warning_delivery_rate)).unwrap_or_else(|| "n/a (0/0)".to_string()), staged.map(|aggregate| display_numeric(&aggregate.signs_warning_to_launch_global)).unwrap_or_else(|| "n/a (n=0)".to_string()), staged.map(|aggregate| display_numeric(&aggregate.signs_warning_to_launch_online)).unwrap_or_else(|| "n/a (n=0)".to_string()), staged.map(|aggregate| display_numeric(&aggregate.preparing_warning_to_launch_online)).unwrap_or_else(|| "n/a (n=0)".to_string()), staged.map(|aggregate| display_numeric(&aggregate.assault_ready_warning_to_launch_online)).unwrap_or_else(|| "n/a (n=0)".to_string())));
    output.push_str("20. **Which three to five balance issues should Checkpoint 2 address?** Checkpoint 2 is limited to pressure and phase pacing. Candidate evidence questions are: (a) whether natural play has a reachable contributor path beyond `Signs`/`Pressure`; (b) whether required objective, structure, wealth, villager, and sanctuary combinations make later thresholds unintentionally inaccessible; (c) whether the ordered online phase minima create the intended preparation cadence once facts are met; (d) whether ready grace and dusk/night preference provide adequate server-side lead time; and (e) whether a natural, non-fixture scenario can launch within a bounded but representative play window. This single-cycle, heavily censored sample does not yet support selecting exact tuning changes; Checkpoint 2 should begin with repeated natural-path validation. Class, defence, villager, helper, and adjacent-settlement work belongs to later checkpoints or additional baseline validation.\n\n");

    output.push_str("## Safety invariants\n\n");
    output.push_str(&format!("- Automatic dusk hordes in PersonalCrisis mode: {}.\n- Duplicate assault launches: {}.\n- Cross-player target violations: {}.\n- Crisis invariant failures: {}.\n- Safe Logout invariant recoveries: {}.\n- Panics: {}.\n\n", report.invariants.automatic_dusk_hordes, report.invariants.duplicate_assaults, report.invariants.cross_player_target_violations, report.invariants.crisis_invariant_failures, report.invariants.safe_logout_invariant_recoveries, report.invariants.panics));
    output.push_str("## Instrumentation limitations\n\n");
    for limitation in &report.limitations {
        output.push_str(&format!("- {limitation}\n"));
    }
    output.push_str("\n## Checkpoint status\n\nThis is the Checkpoint 1 baseline, not milestone completion. Checkpoint 2 may tune only issues supported by this evidence and further controlled runs.\n");
    output
}

#[cfg(test)]
fn write_balance_outputs_to(
    results: &[RunMetrics],
    json_path: &str,
    markdown_path: &str,
) -> std::io::Result<()> {
    let report = build_balance_report(results);
    let json = serde_json::to_vec_pretty(&report)
        .map_err(|error| std::io::Error::other(format!("serialize report: {error}")))?;
    fs::write(json_path, json)?;
    fs::write(markdown_path, render_balance_markdown(&report))?;
    Ok(())
}

fn write_checkpoint2_balance_report(
    results: &[RunMetrics],
    side: BalanceComparisonSide,
) -> std::io::Result<()> {
    write_checkpoint2_balance_report_to(results, side.report_path())?;
    println!("Wrote {}", side.report_path());
    Ok(())
}

fn write_checkpoint2_balance_report_to(results: &[RunMetrics], path: &str) -> std::io::Result<()> {
    let report = build_balance_report(results);
    let json = serde_json::to_vec_pretty(&report)
        .map_err(|error| std::io::Error::other(format!("serialize report: {error}")))?;
    fs::write(path, json)?;
    Ok(())
}

fn mean_optional(
    results: &[RunMetrics],
    value: impl Fn(&RunMetrics) -> Option<i32>,
) -> Option<(f64, usize)> {
    let samples = results.iter().filter_map(value).collect::<Vec<_>>();
    if samples.is_empty() {
        None
    } else {
        Some((
            samples.iter().map(|sample| *sample as f64).sum::<f64>() / samples.len() as f64,
            samples.len(),
        ))
    }
}

fn print_phase_mean(label: &str, sample: Option<(f64, usize)>) {
    match sample {
        Some((mean, count)) => println!("{label:<20}: {mean:.1} ticks ({count} samples)"),
        None => println!("{label:<20}: n/a (0 samples)"),
    }
}

fn print_rate(label: &str, numerator: u64, denominator: u64) {
    if denominator == 0 {
        println!("{label:<20}: n/a (0 eligible)");
    } else {
        println!(
            "{label:<20}: {:.1}% ({numerator}/{denominator})",
            100.0 * numerator as f64 / denominator as f64
        );
    }
}

fn print_reason_counts(label: &str, counts: &std::collections::BTreeMap<String, u64>) {
    if counts.is_empty() {
        println!("{label:<20}: none");
        return;
    }
    let values = counts
        .iter()
        .map(|(reason, count)| format!("{reason}={count}"))
        .collect::<Vec<_>>()
        .join(", ");
    println!("{label:<20}: {values}");
}

fn print_summary(results: &[RunMetrics]) {
    if results.is_empty() {
        println!("No runs.");
        return;
    }

    let n = results.len() as f64;
    let wins = results
        .iter()
        .filter(|m| m.outcome.starts_with("Victory"))
        .count();
    let deaths = results.iter().filter(|m| m.outcome == "TrueDeath").count();
    let panics = results.iter().filter(|m| m.outcome == "Panic").count();
    let runs_launched = results
        .iter()
        .filter(|m| m.crisis_assaults_launched > 0)
        .count();
    let runs_resolved = results
        .iter()
        .filter(|m| m.crisis_assaults_resolved > 0)
        .count();
    let assaults_launched = results
        .iter()
        .map(|m| m.crisis_assaults_launched)
        .sum::<i32>();
    let assaults_resolved = results
        .iter()
        .map(|m| m.crisis_assaults_resolved)
        .sum::<i32>();
    let duplicate_assaults = results
        .iter()
        .map(|m| m.crisis_duplicate_assaults)
        .sum::<i32>();
    let automatic_dusk_hordes = results
        .iter()
        .map(|m| m.personal_crisis_automatic_dusk_hordes)
        .sum::<i32>();
    let invariant_failures = results
        .iter()
        .filter(|m| m.outcome != "Panic" && !m.crisis_invariants_ok)
        .count();
    let safe_logout_scenario_runs = results
        .iter()
        .filter(|m| m.safe_logout_scenario_mode != RunnerMode::Standard.label())
        .count();
    let safe_logout_requests = results.iter().map(|m| m.safe_logout_requests).sum::<u64>();
    let safe_logout_accepted = results.iter().map(|m| m.safe_logout_accepted).sum::<u64>();
    let safe_logout_rejected = results.iter().map(|m| m.safe_logout_rejected).sum::<u64>();
    let safe_logout_cancelled = results.iter().map(|m| m.safe_logout_cancelled).sum::<u64>();
    let safe_logout_completed = results.iter().map(|m| m.safe_logout_completed).sum::<u64>();
    let protected_sessions = results
        .iter()
        .map(|m| m.safe_logout_protected_sessions_started)
        .sum::<u64>();
    let safe_logout_resumed = results.iter().map(|m| m.safe_logout_resumed).sum::<u64>();
    let protected_ticks = results
        .iter()
        .map(|m| m.safe_logout_protected_ticks_total)
        .sum::<u64>();
    let ordinary_disconnects = results
        .iter()
        .map(|m| m.safe_logout_ordinary_disconnects)
        .sum::<u64>();
    let active_assault_disconnects = results
        .iter()
        .map(|m| m.safe_logout_active_assault_disconnects)
        .sum::<u64>();
    let status_packets = results
        .iter()
        .map(|m| m.safe_logout_status_packets_sent)
        .sum::<u64>();
    let duplicate_statuses_suppressed = results
        .iter()
        .map(|m| m.safe_logout_status_packets_duplicate_suppressed)
        .sum::<u64>();
    let protected_inputs = results
        .iter()
        .map(|m| m.safe_logout_protected_input_rejections)
        .sum::<u64>();
    let protected_damage = results
        .iter()
        .map(|m| m.safe_logout_protected_damage_blocks)
        .sum::<u64>();
    let protected_targets = results
        .iter()
        .map(|m| m.safe_logout_protected_target_rejections)
        .sum::<u64>();
    let queued_events = results
        .iter()
        .map(|m| m.safe_logout_queued_events_discarded)
        .sum::<u64>();
    let timer_rebases = results
        .iter()
        .map(|m| m.safe_logout_timer_rebases)
        .sum::<u64>();
    let stale_connection_events = results
        .iter()
        .map(|m| m.safe_logout_stale_connection_events_rejected)
        .sum::<u64>();
    let safe_logout_invariant_recoveries = results
        .iter()
        .map(|m| m.safe_logout_invariant_recoveries)
        .sum::<u64>();
    let safe_logout_run_key_mismatches = results
        .iter()
        .map(|m| m.safe_logout_run_key_mismatches)
        .sum::<u64>();
    let safe_logout_invariant_failures = results
        .iter()
        .filter(|m| !m.safe_logout_invariants_ok)
        .count();
    let mut rejection_reasons = std::collections::BTreeMap::<String, u64>::new();
    let mut cancellation_reasons = std::collections::BTreeMap::<String, u64>::new();
    let mut safe_logout_invariant_reasons = std::collections::BTreeMap::<String, u64>::new();
    for metrics in results {
        for (reason, count) in &metrics.safe_logout_rejection_reasons {
            *rejection_reasons.entry(reason.clone()).or_default() += count;
        }
        for (reason, count) in &metrics.safe_logout_cancellation_reasons {
            *cancellation_reasons.entry(reason.clone()).or_default() += count;
        }
        for (reason, count) in &metrics.safe_logout_invariant_reasons {
            *safe_logout_invariant_reasons
                .entry(reason.clone())
                .or_default() += count;
        }
    }

    let mean = |f: &dyn Fn(&RunMetrics) -> f64| -> f64 { results.iter().map(f).sum::<f64>() / n };

    let mut ticks: Vec<i32> = results.iter().map(|m| m.ticks).collect();
    ticks.sort_unstable();
    let pct = |p: f64| -> i32 {
        let idx = (((ticks.len() as f64) * p).ceil() as usize)
            .saturating_sub(1)
            .min(ticks.len() - 1);
        ticks[idx]
    };

    println!(
        "\n========== Aggregate summary ({} runs) ==========",
        results.len()
    );
    println!(
        "win rate            : {:.1}% ({wins}/{})",
        100.0 * wins as f64 / n,
        results.len()
    );
    println!(
        "true-death rate     : {:.1}% ({deaths}/{})",
        100.0 * deaths as f64 / n,
        results.len()
    );
    println!(
        "panic rate          : {:.1}% ({panics}/{})",
        100.0 * panics as f64 / n,
        results.len()
    );
    println!(
        "crisis launch rate  : {:.1}% ({runs_launched}/{})",
        100.0 * runs_launched as f64 / n,
        results.len()
    );
    println!(
        "crisis resolve rate : {:.1}% ({runs_resolved}/{})",
        100.0 * runs_resolved as f64 / n,
        results.len()
    );
    if assaults_launched > 0 {
        println!(
            "assault completion : {:.1}% ({assaults_resolved}/{assaults_launched})",
            100.0 * assaults_resolved as f64 / assaults_launched as f64
        );
    } else {
        println!("assault completion : n/a (0 launched)");
    }
    println!("duplicate assaults  : {duplicate_assaults}");
    println!("automatic dusk waves: {automatic_dusk_hordes}");
    println!("crisis invariant bad: {invariant_failures}");
    println!("safe-logout scenarios: {safe_logout_scenario_runs}");
    println!("safe-logout requests : {safe_logout_requests}");
    print_rate(
        "safe-logout accepted",
        safe_logout_accepted,
        safe_logout_requests,
    );
    print_rate(
        "safe-logout rejected",
        safe_logout_rejected,
        safe_logout_requests,
    );
    print_rate(
        "safe-logout complete",
        safe_logout_completed,
        safe_logout_accepted,
    );
    print_rate(
        "safe-logout cancelled",
        safe_logout_cancelled,
        safe_logout_accepted,
    );
    print_rate(
        "safe-logout resumed",
        safe_logout_resumed,
        protected_sessions,
    );
    print_rate(
        "active disconnects",
        active_assault_disconnects,
        ordinary_disconnects,
    );
    if protected_sessions == 0 {
        println!("protected duration  : 0 ticks (0 sessions)");
    } else {
        println!(
            "protected duration  : {protected_ticks} ticks total, {:.1} mean",
            protected_ticks as f64 / protected_sessions as f64
        );
    }
    println!("reconnect successes : {safe_logout_resumed}");
    println!("ordinary disconnects: {ordinary_disconnects}");
    println!("active disconnects  : {active_assault_disconnects}");
    println!(
        "safe status packets : {status_packets} sent, {duplicate_statuses_suppressed} suppressed"
    );
    println!(
        "protected boundaries: input={protected_inputs} damage={protected_damage} target={protected_targets} queued={queued_events}"
    );
    println!("resume timer rebases: {timer_rebases}");
    println!("stale connection evt: {stale_connection_events}");
    println!(
        "invariant recoveries: {safe_logout_invariant_recoveries} (run-key mismatches={safe_logout_run_key_mismatches})"
    );
    println!("safe invariant bad  : {safe_logout_invariant_failures}");
    print_reason_counts("rejection reasons", &rejection_reasons);
    print_reason_counts("cancel reasons", &cancellation_reasons);
    print_reason_counts("invariant reasons", &safe_logout_invariant_reasons);
    for phase in [
        "none",
        "dormant",
        "signs",
        "pressure",
        "preparing",
        "assault_ready",
        "assault_active",
        "resolved",
    ] {
        let count = results
            .iter()
            .filter(|metrics| metrics.crisis_highest_phase == phase)
            .count();
        if count > 0 {
            println!("highest {phase:<11}: {count}");
        }
    }
    print_phase_mean(
        "mean signs tick",
        mean_optional(results, |m| m.crisis_signs_tick),
    );
    print_phase_mean(
        "mean pressure tick",
        mean_optional(results, |m| m.crisis_pressure_tick),
    );
    print_phase_mean(
        "mean preparing tick",
        mean_optional(results, |m| m.crisis_preparing_tick),
    );
    print_phase_mean(
        "mean ready tick",
        mean_optional(results, |m| m.crisis_assault_ready_tick),
    );
    print_phase_mean(
        "mean active tick",
        mean_optional(results, |m| m.crisis_assault_active_tick),
    );
    print_phase_mean(
        "mean resolved tick",
        mean_optional(results, |m| m.crisis_resolved_tick),
    );
    println!(
        "mean days survived  : {:.2}",
        mean(&|m| m.days_survived as f64)
    );
    println!(
        "mean enemies killed : {:.2}",
        mean(&|m| m.enemies_killed as f64)
    );
    println!(
        "mean deaths         : {:.2}",
        mean(&|m| m.num_deaths as f64)
    );
    println!(
        "mean final skill xp : {:.1}",
        mean(&|m| m.final_skill_total as f64)
    );
    println!(
        "mean inventory count: {:.1}",
        mean(&|m| m.final_inventory_count as f64)
    );
    println!(
        "mean structures     : {:.2}",
        mean(&|m| m.structures_built as f64)
    );
    println!("ticks p50 / p90     : {} / {}", pct(0.50), pct(0.90));
    println!("=================================================");
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn csv_schema_preserves_every_existing_column_before_checkpoint3_fields() {
        let header = csv_header_fields();
        let metrics = panic_metrics(7, 12_345, RunnerMode::SafeLogout);
        let row = metrics_csv_row(&metrics);
        let legacy_len = PRE_SAFE_LOGOUT_CSV_FIELDS.len() + SAFE_LOGOUT_CSV_FIELDS.len();
        let existing_len = legacy_len + BALANCE_CSV_FIELDS.len();
        let pre_checkpoint3_len = existing_len + SIGNS_WARNING_CSV_FIELDS.len();

        assert_eq!(PRE_SAFE_LOGOUT_CSV_FIELDS.len(), 48);
        assert_eq!(legacy_len, 73);
        assert_eq!(existing_len, 189);
        assert_eq!(pre_checkpoint3_len, 191);
        assert_eq!(
            &header[..PRE_SAFE_LOGOUT_CSV_FIELDS.len()],
            PRE_SAFE_LOGOUT_CSV_FIELDS
        );
        assert_eq!(
            &header[PRE_SAFE_LOGOUT_CSV_FIELDS.len()..legacy_len],
            SAFE_LOGOUT_CSV_FIELDS
        );
        assert_eq!(&header[legacy_len..existing_len], BALANCE_CSV_FIELDS);
        assert_eq!(
            &header[existing_len..pre_checkpoint3_len],
            SIGNS_WARNING_CSV_FIELDS
        );
        assert_eq!(
            &header[pre_checkpoint3_len..],
            CHECKPOINT3_PREPARATION_CSV_FIELDS
        );
        assert_eq!(header.len(), 201);
        assert_eq!(header.len(), row.len());
        assert_eq!(header[47], "crisis_invariants_ok");
        assert_eq!(header[48], "safe_logout_scenario_mode");
        assert_eq!(row[48], "safe_logout");
        assert_eq!(header[legacy_len], "crisis_balance_scenario");
        assert_eq!(row[legacy_len + 3], "12345");
    }

    #[test]
    fn checkpoint3_preparation_actions_reach_csv_and_nested_json() {
        let mut metrics = aggregate_fixture(1, "prepared_solo", "Warrior", Some(25));
        let actions = &mut metrics.crisis_balance.preparation_actions;
        assert!(actions.record_repair_started(100, 500));
        assert!(actions.record_repair_completed(100, 550));
        assert!(actions.record_defensive_structure_started(200, 600));
        assert!(actions.record_defensive_structure_completed(200, true, 650));
        assert!(actions.record_equipment_change(300, 700));
        assert_eq!(actions.observe_healing_items(0, 750), 0);
        assert_eq!(actions.observe_healing_items(2, 800), 2);
        assert!(actions.record_healing_item_used_before_launch(Uuid::from_u128(400), 850));
        assert!(actions.record_villager_recruited(500, 900));
        assert!(actions.record_launch_readiness(2, [500, 500, 501]));

        let header = csv_header_fields();
        let row = metrics_csv_row(&metrics);
        for (name, expected) in [
            ("crisis_prep_repairs_started", "1"),
            ("crisis_prep_repairs_completed", "1"),
            ("crisis_prep_defensive_structures_started", "1"),
            ("crisis_prep_defensive_structures_completed", "1"),
            ("crisis_prep_healing_items_carried_at_launch", "2"),
            ("crisis_prep_healing_items_used_before_launch", "1"),
            ("crisis_prep_combat_capable_villagers_at_launch", "2"),
            ("crisis_prep_first_preparation_action_tick", "500"),
            ("crisis_prep_meaningful_preparation_category_count", "5"),
        ] {
            let index = header
                .iter()
                .position(|field| *field == name)
                .expect("Checkpoint 3 preparation column");
            assert_eq!(row[index], expected);
        }
        let categories_index = header
            .iter()
            .position(|field| *field == "crisis_prep_meaningful_preparation_categories")
            .expect("meaningful preparation categories column");
        assert_eq!(
            row[categories_index],
            r#"["defenses","equipment","healing","repair","villager_support"]"#
        );

        let json = serde_json::to_value(&metrics).expect("serialize runner metrics");
        let actions_json = &json["crisis_balance"]["preparation_actions"];
        assert_eq!(actions_json["repairs_started"], 1);
        assert_eq!(actions_json["repairs_completed"], 1);
        assert_eq!(actions_json["defensive_structures_started"], 1);
        assert_eq!(actions_json["defensive_structures_completed"], 1);
        assert_eq!(actions_json["healing_items_carried_at_launch"], 2);
        assert_eq!(actions_json["healing_items_used_before_launch"], 1);
        assert_eq!(actions_json["combat_capable_villagers_at_launch"], 2);
        assert_eq!(actions_json["first_preparation_action_tick"], 500);
        assert_eq!(actions_json["meaningful_preparation_category_count"], 5);
        assert_eq!(
            actions_json["meaningful_preparation_categories"],
            serde_json::json!([
                "defenses",
                "equipment",
                "healing",
                "repair",
                "villager_support"
            ])
        );
        assert!(actions_json.get("repair_starts").is_none());
        assert!(actions_json.get("meaningful_category_keys").is_none());
    }

    #[test]
    fn signs_warning_to_launch_values_reach_metrics_csv_aggregate_json_and_markdown() {
        let mut metrics = aggregate_fixture(1, "prepared_solo", "Warrior", Some(25));
        metrics.crisis_balance_progression_fixture = true;
        metrics
            .crisis_balance
            .phase_timing
            .record_phase(CrisisPhase::Signs, 90, 10);
        metrics
            .crisis_balance
            .warnings
            .record(CrisisPhase::Signs, 100, 20, true, false);
        metrics
            .crisis_balance
            .warnings
            .record(CrisisPhase::AssaultActive, 650, 250, true, true);
        metrics.crisis_warning_signs_to_launch_global_ticks = metrics
            .crisis_balance
            .warnings
            .signs_to_launch_global_ticks();
        metrics.crisis_warning_signs_to_launch_online_ticks = metrics
            .crisis_balance
            .warnings
            .signs_to_launch_online_ticks();

        assert_eq!(
            metrics.crisis_warning_signs_to_launch_global_ticks,
            Some(550)
        );
        assert_eq!(
            metrics.crisis_warning_signs_to_launch_online_ticks,
            Some(230)
        );

        let header = csv_header_fields();
        let row = metrics_csv_row(&metrics);
        for (name, expected) in [
            ("crisis_warning_signs_to_launch_global_ticks", "550"),
            ("crisis_warning_signs_to_launch_online_ticks", "230"),
        ] {
            let index = header
                .iter()
                .position(|field| *field == name)
                .expect("Signs warning duration column");
            assert_eq!(row[index], expected);
        }

        let metrics_json = serde_json::to_value(&metrics).expect("serialize metrics");
        assert_eq!(
            metrics_json["crisis_warning_signs_to_launch_global_ticks"],
            550
        );
        assert_eq!(
            metrics_json["crisis_warning_signs_to_launch_online_ticks"],
            230
        );

        let report = build_balance_report(&[metrics]);
        let report_json = serde_json::to_value(&report).expect("serialize report");
        let staged = &report_json["by_progression_cohort"]["staged_attainable_facts"];
        assert_eq!(staged["signs_warning_delivery_rate"]["count"], 1);
        assert_eq!(
            staged["signs_warning_to_launch_global"]["samples_with_value"],
            1
        );
        assert_eq!(staged["signs_warning_to_launch_global"]["mean"], 550.0);
        assert_eq!(staged["signs_warning_to_launch_online"]["mean"], 230.0);

        let markdown = render_balance_markdown(&report);
        assert!(markdown.contains(
            "Staged Signs-warning-to-launch lead was mean 550.0, median 550.0 (n=1) in global ticks and mean 230.0, median 230.0 (n=1) in online-active ticks"
        ));
    }

    #[test]
    fn missing_signs_warning_to_launch_values_serialize_as_null_and_empty_csv_cells() {
        let metrics = aggregate_fixture(1, "prepared_solo", "Warrior", Some(25));
        assert_eq!(metrics.crisis_warning_signs_to_launch_global_ticks, None);
        assert_eq!(metrics.crisis_warning_signs_to_launch_online_ticks, None);

        let header = csv_header_fields();
        let row = metrics_csv_row(&metrics);
        for name in SIGNS_WARNING_CSV_FIELDS {
            let index = header
                .iter()
                .position(|field| field == name)
                .expect("Signs warning duration column");
            assert_eq!(row[index], "");
        }

        let metrics_json = serde_json::to_value(&metrics).expect("serialize metrics");
        assert!(metrics_json["crisis_warning_signs_to_launch_global_ticks"].is_null());
        assert!(metrics_json["crisis_warning_signs_to_launch_online_ticks"].is_null());

        let report = build_balance_report(&[metrics]);
        let report_json = serde_json::to_value(&report).expect("serialize report");
        let aggregate = &report_json["assault_outcomes"];
        assert_eq!(
            aggregate["signs_warning_to_launch_global"]["samples_with_value"],
            0
        );
        assert!(aggregate["signs_warning_to_launch_global"]["mean"].is_null());
        assert!(aggregate["signs_warning_to_launch_online"]["median"].is_null());
    }

    #[test]
    fn json_and_csv_reason_fields_are_structured_and_csv_safe() {
        let mut metrics = panic_metrics(3, 120_000, RunnerMode::SafeLogout);
        metrics
            .safe_logout_rejection_reasons
            .insert("active_assault".to_string(), 2);
        metrics
            .safe_logout_rejection_reasons
            .insert("hostile_nearby".to_string(), 1);

        let json = serde_json::to_value(&metrics).expect("serialize runner metrics");
        assert_eq!(json["safe_logout_rejection_reasons"]["active_assault"], 2);
        assert_eq!(json["safe_logout_scenario_mode"], "safe_logout");

        let row = metrics_csv_row(&metrics);
        let reason_index = csv_header_fields()
            .iter()
            .position(|field| *field == "safe_logout_rejection_reasons")
            .expect("safe-logout rejection column");
        let encoded = escape_csv_cell(&row[reason_index]);
        assert!(encoded.starts_with('"'));
        assert!(encoded.ends_with('"'));
        assert!(encoded.contains("active_assault"));
        assert!(encoded.contains("hostile_nearby"));
    }

    #[test]
    fn balance_csv_uses_end_snapshot_when_an_assault_did_not_launch() {
        let mut metrics = aggregate_fixture(3, "prepared_solo", "Warrior", None);
        metrics.crisis_balance.assault_outcome.assault_launched = false;
        metrics.crisis_balance.preparation_snapshots.assault_launch = None;
        metrics.crisis_balance.preparation_snapshots.assault_ready =
            Some(siege_perilous::crisis_balance::CrisisPreparationSnapshot {
                completed_structures: 1,
                ..Default::default()
            });
        metrics
            .crisis_balance
            .preparation_snapshots
            .resolution_or_end = Some(siege_perilous::crisis_balance::CrisisPreparationSnapshot {
            completed_structures: 2,
            ..Default::default()
        });

        let header = csv_header_fields();
        let row = metrics_csv_row(&metrics);
        let index = header
            .iter()
            .position(|field| *field == "crisis_prep_completed_structures")
            .expect("preparation structure column");
        assert_eq!(row[index], "2");
    }

    #[test]
    fn safe_logout_runner_mode_is_explicit_and_standard_remains_default_label() {
        assert_eq!(RunnerMode::parse("standard"), Some(RunnerMode::Standard));
        assert_eq!(
            RunnerMode::parse("safe-logout"),
            Some(RunnerMode::SafeLogout)
        );
        assert_eq!(
            RunnerMode::parse("safe-logout-matrix"),
            Some(RunnerMode::SafeLogoutMatrix)
        );
        assert_eq!(RunnerMode::parse("unknown"), None);
        assert_eq!(RunnerMode::Standard.label(), "standard");
    }

    #[test]
    fn safe_logout_matrix_cycles_through_every_required_scenario() {
        assert_eq!(SafeLogoutMatrixScenario::ALL.len(), 8);
        assert_eq!(
            SafeLogoutMatrixScenario::for_run(0).label(),
            "matrix_normal_play"
        );
        assert_eq!(
            SafeLogoutMatrixScenario::for_run(1).label(),
            "matrix_completion"
        );
        assert_eq!(
            SafeLogoutMatrixScenario::for_run(2).label(),
            "matrix_cancellation"
        );
        assert_eq!(
            SafeLogoutMatrixScenario::for_run(3).label(),
            "matrix_long_protection"
        );
        assert_eq!(
            SafeLogoutMatrixScenario::for_run(4).label(),
            "matrix_reconnect"
        );
        assert_eq!(
            SafeLogoutMatrixScenario::for_run(5).label(),
            "matrix_ordinary_disconnect"
        );
        assert_eq!(
            SafeLogoutMatrixScenario::for_run(6).label(),
            "matrix_active_assault_disconnect"
        );
        assert_eq!(
            SafeLogoutMatrixScenario::for_run(7).label(),
            "matrix_multiple_players"
        );
        assert_eq!(
            SafeLogoutMatrixScenario::for_run(8),
            SafeLogoutMatrixScenario::NormalPlay
        );
    }

    #[test]
    fn matrix_panic_rows_retain_the_exact_scenario_label() {
        for (run_index, scenario) in SafeLogoutMatrixScenario::ALL.into_iter().enumerate() {
            let metrics = panic_metrics(
                run_index as u32,
                DEFAULT_MAX_TICKS,
                RunnerMode::SafeLogoutMatrix,
            );
            assert_eq!(metrics.safe_logout_scenario_mode, scenario.label());
        }
    }

    #[test]
    fn goblin_balance_matrix_cycles_every_scenario_and_class() {
        assert_eq!(BalanceRunSpec::COMBINATIONS, 39);
        let specs = (0..BalanceRunSpec::COMBINATIONS as u32)
            .map(BalanceRunSpec::for_run)
            .collect::<Vec<_>>();
        for variant in BALANCE_DRIVER_VARIANTS {
            for hero_class in BALANCE_HERO_CLASSES {
                assert_eq!(
                    specs
                        .iter()
                        .filter(|spec| {
                            spec.scenario == variant.scenario
                                && spec.hero_class == hero_class
                                && spec.progression_fixture == variant.progression_fixture
                        })
                        .count(),
                    1
                );
            }
        }
        assert_eq!(BalanceRunSpec::for_run(39).repetition, 1);

        let panic = panic_metrics(5, 77_777, RunnerMode::GoblinBalance);
        let spec = BalanceRunSpec::for_run(5);
        assert_eq!(panic.crisis_balance_scenario, spec.scenario.label());
        assert_eq!(panic.crisis_balance_hero_class, spec.hero_class);
        assert_eq!(panic.crisis_balance_tick_cap, 77_777);
        assert_eq!(
            panic.crisis_balance_progression_fixture,
            spec.progression_fixture
        );
        assert!(panic.crisis_balance_run_id.contains(spec.scenario.label()));
    }

    #[test]
    fn checkpoint2_comparison_side_paths_are_explicit_and_do_not_select_checkpoint1_artifacts() {
        assert_eq!(
            BalanceComparisonSide::parse("control"),
            Some(BalanceComparisonSide::Control)
        );
        assert_eq!(
            BalanceComparisonSide::parse("candidate"),
            Some(BalanceComparisonSide::Candidate)
        );
        assert!(BalanceComparisonSide::parse("baseline").is_none());
        for path in [
            BalanceComparisonSide::Control.report_path(),
            BalanceComparisonSide::Candidate.report_path(),
        ] {
            assert!(path.contains("checkpoint2"));
            assert_ne!(path, "goblin_crisis_balance_report.json");
            assert_ne!(path, "../docs/goblin_crisis_balance_baseline.md");
        }

        let candidate = siege_perilous::game::goblin_crisis_balance_config_snapshot();
        assert!(
            validate_balance_comparison_config(BalanceComparisonSide::Candidate, &candidate)
                .is_ok()
        );
        assert!(
            validate_balance_comparison_config(BalanceComparisonSide::Control, &candidate).is_err()
        );

        let mut control = candidate;
        control.preparing_threshold = 70;
        control.assault_ready_threshold = 90;
        assert!(
            validate_balance_comparison_config(BalanceComparisonSide::Control, &control).is_ok()
        );
        assert!(
            validate_balance_comparison_config(BalanceComparisonSide::Candidate, &control).is_err()
        );
    }

    fn aggregate_fixture(
        index: u32,
        scenario: &str,
        hero_class: &str,
        assault_duration: Option<i32>,
    ) -> RunMetrics {
        let mut metrics = panic_metrics(index, 10_000, RunnerMode::Standard);
        metrics.outcome = "MaxTicks".to_string();
        metrics.crisis_invariants_ok = true;
        metrics.safe_logout_invariants_ok = true;
        metrics.crisis_balance_scenario = scenario.to_string();
        metrics.crisis_balance_hero_class = hero_class.to_string();
        metrics.crisis_balance_run_id = format!("fixture-{index}");
        metrics.crisis_balance_tick_cap_reached = true;
        metrics.crisis_balance.assault_outcome.assault_launched = true;
        metrics.crisis_balance.assault_outcome.assault_resolved = assault_duration.is_some();
        metrics
            .crisis_balance
            .assault_outcome
            .hero_alive_at_resolution = assault_duration.map(|_| true);
        metrics
            .crisis_balance
            .assault_outcome
            .assault_duration_ticks = assault_duration;
        metrics.crisis_balance.assault_outcome.hero_damage_taken = index as i32 * 10;
        metrics
    }

    #[test]
    fn aggregate_means_medians_rates_and_missing_sample_counts_are_exact() {
        let first = aggregate_fixture(1, "prepared_solo", "Warrior", Some(10));
        let second = aggregate_fixture(2, "prepared_solo", "Ranger", Some(20));
        let third = aggregate_fixture(3, "prepared_solo", "Mage", None);
        let refs = vec![&first, &second, &third];
        let aggregate = aggregate_balance_runs(&refs);

        assert_eq!(aggregate.assault_launch_rate.count, 3);
        assert_eq!(aggregate.assault_launch_rate.sample_count, 3);
        assert_eq!(aggregate.assault_resolution_rate.count, 2);
        assert_eq!(aggregate.assault_resolution_rate.sample_count, 3);
        assert_eq!(aggregate.tick_cap_reached_count, 3);
        assert_eq!(aggregate.unresolved_at_tick_cap_count, 1);
        assert_eq!(aggregate.hero_survival_rate.count, 2);
        assert_eq!(aggregate.hero_survival_rate.sample_count, 2);
        assert_eq!(aggregate.assault_duration.samples_with_value, 2);
        assert_eq!(aggregate.assault_duration.mean, Some(15.0));
        assert_eq!(aggregate.assault_duration.median, Some(15.0));
        assert_eq!(aggregate.hero_damage.samples_with_value, 3);
        assert_eq!(aggregate.hero_damage.mean, Some(20.0));

        let odd = NumericSummary::from_values(vec![30.0, 10.0, 20.0]);
        assert_eq!(odd.median, Some(20.0));
        let empty = NumericSummary::from_values(Vec::new());
        assert_eq!(empty.samples_with_value, 0);
        assert_eq!(empty.mean, None);
        assert_eq!(empty.median, None);
    }

    #[test]
    fn report_and_new_metrics_serialize_nested_options_and_required_groups() {
        let metrics = aggregate_fixture(1, "prepared_solo", "Warrior", Some(25));
        let metrics_json = serde_json::to_value(&metrics).expect("serialize metrics");
        assert!(metrics_json.get("run_index").is_some());
        assert_eq!(metrics_json["crisis_balance_scenario"], "prepared_solo");
        assert!(metrics_json["crisis_balance"]["phase_timing"]["pressure_entered_tick"].is_null());

        let report = build_balance_report(&[metrics]);
        let json = serde_json::to_value(&report).expect("serialize report");
        assert_eq!(json["version"], BALANCE_REPORT_VERSION);
        assert!(json["balance_config"].is_object());
        assert!(json["by_scenario"]["prepared_solo"].is_object());
        assert!(json["by_scenario_cohort"].is_object());
        assert!(json["by_progression_cohort"].is_object());
        assert!(json["by_hero_class"]["Warrior"].is_object());
        assert!(json["by_hero_class_cohort"].is_object());
        assert!(json["by_preparation"].is_object());
        assert!(json["by_villagers"].is_object());
        assert!(json["by_villagers_cohort"].is_object());
        assert!(json["by_connection"].is_object());
        assert!(json["by_helper"].is_object());
        assert_eq!(json["sample_summary"]["total_runs"], 1);
    }

    #[test]
    fn report_separates_preparation_groups_by_progression_cohort() {
        let mut natural = aggregate_fixture(1, "prepared_solo", "Warrior", None);
        natural.crisis_balance.preparation_snapshots.preparing = Some(Default::default());
        let mut staged = natural.clone();
        staged.run_index = 2;
        staged.crisis_balance_progression_fixture = true;
        staged.crisis_balance_run_id = "fixture-2".to_string();

        let report = build_balance_report(&[natural, staged]);
        assert!(report
            .by_preparation
            .contains_key("no_observed_preparation_action / natural_progression"));
        assert!(report
            .by_preparation
            .contains_key("no_observed_preparation_action / staged_attainable_facts"));
        assert!(report
            .by_preparation_policy
            .contains_key("prepared / natural_progression"));
        assert!(report
            .by_preparation_policy
            .contains_key("prepared / staged_attainable_facts"));
    }

    #[test]
    fn panic_rows_are_reported_without_becoming_crisis_invariant_failures() {
        let panic = panic_metrics(5, 10_000, RunnerMode::GoblinBalance);
        let report = build_balance_report(&[panic]);

        assert_eq!(report.invariants.panics, 1);
        assert_eq!(report.invariants.crisis_invariant_failures, 0);
    }

    #[test]
    fn balance_report_writer_writes_both_outputs_and_propagates_errors() {
        let root = std::env::temp_dir().join(format!(
            "siege-perilous-balance-report-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create report test directory");
        let json_path = root.join("report.json");
        let markdown_path = root.join("report.md");
        let metrics = aggregate_fixture(1, "prepared_solo", "Warrior", Some(25));

        write_balance_outputs_to(
            &[metrics.clone()],
            json_path.to_str().unwrap(),
            markdown_path.to_str().unwrap(),
        )
        .expect("write both reports");
        let json = fs::read_to_string(&json_path).expect("read report JSON");
        let markdown = fs::read_to_string(&markdown_path).expect("read report markdown");
        assert!(json.contains("\"by_scenario\""));
        assert!(markdown.contains("## Required baseline questions"));
        assert!(markdown.contains("No staged row completed pre-launch Safe Logout"));

        let missing_parent = root.join("missing").join("report.json");
        assert!(write_balance_outputs_to(
            &[metrics],
            missing_parent.to_str().unwrap(),
            markdown_path.to_str().unwrap(),
        )
        .is_err());
        fs::remove_dir_all(&root).expect("remove report test directory");
    }

    #[test]
    fn checkpoint2_aggregate_report_round_trips_for_control_candidate_comparison() {
        let root = std::env::temp_dir().join(format!(
            "siege-perilous-checkpoint2-report-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create checkpoint2 report test directory");
        let report_path = root.join("control.json");
        let metrics = aggregate_fixture(1, "helper_supported", "Warrior", Some(25));

        write_checkpoint2_balance_report_to(
            &[metrics],
            report_path.to_str().expect("utf-8 report path"),
        )
        .expect("write checkpoint2 aggregate report");
        let bytes = fs::read(&report_path).expect("read checkpoint2 aggregate report");
        let report: GoblinCrisisBalanceReport =
            serde_json::from_slice(&bytes).expect("deserialize checkpoint2 aggregate report");
        assert_eq!(report.sample_summary.total_runs, 1);
        assert!(report.by_scenario.contains_key("helper_supported"));
        assert_eq!(
            report.balance_config,
            siege_perilous::game::goblin_crisis_balance_config_snapshot()
        );

        fs::remove_dir_all(&root).expect("remove checkpoint2 report test directory");
    }
}
