// Multi-game headless balance/metrics runner.
//
//   cargo run --bin headless_runner [N] [MAX_TICKS]
//       [standard|safe-logout|safe-logout-matrix]
//
// Runs N full games (default 20) back-to-back, each in a fresh in-process Bevy
// `App` (full isolation), driven by the deterministic scripted bot. Emits
// `headless_runs.csv` + `headless_runs.json` and prints an aggregate summary.
//
// MUST be run with CWD = sp_server/ so templates/map/tileset load by relative
// path (same as the existing tests).

use std::fs::File;
use std::io::Write;

use siege_perilous::headless::{HeadlessGame, RunMetrics};
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
}

impl RunnerMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(Self::Standard),
            "safe-logout" | "safe_logout" => Some(Self::SafeLogout),
            "safe-logout-matrix" | "safe_logout_matrix" => Some(Self::SafeLogoutMatrix),
            _ => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::SafeLogout => "safe_logout",
            Self::SafeLogoutMatrix => "safe_logout_matrix",
        }
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
        _ => mode.label(),
    }
}

fn run_one(run_index: u32, max_ticks: i32, mode: RunnerMode) -> RunMetrics {
    let mut game = HeadlessGame::new(max_ticks);
    let pid = game.spawn_hero("Warrior", &format!("Bot{run_index}"));
    let mut bot = Bot::new(pid);

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
    }

    while !game.is_over() {
        let view = game.observe();
        let action = bot.step(&view, game.map());
        if let Some(event) = action {
            game.inject(event);
        }
        bot.advance_phase(&view);
        game.tick(DECISION_TICKS);
    }

    let mut metrics = game.metrics();
    metrics.run_index = run_index;
    metrics.safe_logout_scenario_mode = result_scenario_label(run_index, mode).to_string();
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
        Err(_) => panic_metrics(run_index, mode),
    }
}

fn panic_metrics(run_index: u32, mode: RunnerMode) -> RunMetrics {
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
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let num_games: u32 = args
        .get(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(DEFAULT_NUM_GAMES);
    let max_ticks: i32 = args
        .get(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(DEFAULT_MAX_TICKS);
    let mode = match args.get(3) {
        Some(value) => match RunnerMode::parse(value) {
            Some(mode) => mode,
            None => {
                eprintln!(
                    "Unknown runner mode '{value}'. Expected 'standard', 'safe-logout', or 'safe-logout-matrix'."
                );
                std::process::exit(2);
            }
        },
        None => RunnerMode::Standard,
    };

    println!(
        "Running {num_games} headless games (max_ticks={max_ticks}, decision_ticks={DECISION_TICKS}, mode={})...",
        mode.label()
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

fn csv_header_fields() -> Vec<&'static str> {
    PRE_SAFE_LOGOUT_CSV_FIELDS
        .iter()
        .chain(SAFE_LOGOUT_CSV_FIELDS)
        .copied()
        .collect()
}

fn optional_tick(tick: Option<i32>) -> String {
    tick.map_or_else(String::new, |tick| tick.to_string())
}

fn reason_counts_json(counts: &std::collections::BTreeMap<String, u64>) -> String {
    serde_json::to_string(counts).unwrap_or_else(|_| "{}".to_string())
}

fn metrics_csv_row(m: &RunMetrics) -> Vec<String> {
    vec![
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
    ]
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
    let invariant_failures = results.iter().filter(|m| !m.crisis_invariants_ok).count();
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

    #[test]
    fn csv_schema_preserves_existing_prefix_and_appends_safe_logout_fields() {
        let header = csv_header_fields();
        let metrics = panic_metrics(7, RunnerMode::SafeLogout);
        let row = metrics_csv_row(&metrics);

        assert_eq!(PRE_SAFE_LOGOUT_CSV_FIELDS.len(), 48);
        assert_eq!(
            &header[..PRE_SAFE_LOGOUT_CSV_FIELDS.len()],
            PRE_SAFE_LOGOUT_CSV_FIELDS
        );
        assert_eq!(
            &header[PRE_SAFE_LOGOUT_CSV_FIELDS.len()..],
            SAFE_LOGOUT_CSV_FIELDS
        );
        assert_eq!(header.len(), row.len());
        assert_eq!(header[47], "crisis_invariants_ok");
        assert_eq!(header[48], "safe_logout_scenario_mode");
        assert_eq!(row[48], "safe_logout");
    }

    #[test]
    fn json_and_csv_reason_fields_are_structured_and_csv_safe() {
        let mut metrics = panic_metrics(3, RunnerMode::SafeLogout);
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
            let metrics = panic_metrics(run_index as u32, RunnerMode::SafeLogoutMatrix);
            assert_eq!(metrics.safe_logout_scenario_mode, scenario.label());
        }
    }
}
