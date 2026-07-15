//! Dedicated Milestone 3 / Checkpoint 4 assault matrix runner.
//!
//! This binary intentionally writes only to an explicitly supplied, new path.
//! It never reuses the Checkpoint 1--3 report names and uses `create_new` so a
//! previous artifact cannot be silently replaced.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use serde_json::Value;
use siege_perilous::constants::{GAME_TICKS_PER_DAY, NO_TARGET, TICKS_PER_SEC};
use siege_perilous::crisis_balance::{
    CrisisBalanceScenario, CrisisBalanceTelemetry, CrisisBalanceTelemetryState,
};
use siege_perilous::game::{goblin_crisis_balance_config_snapshot, CrisisPhase};
use siege_perilous::headless::{
    validate_checkpoint4_preparation_pair_launches, HeadlessGame, PreparationComparison,
    PreparationDeclaredDifference, PreparationPairLaunch, PreparationPairLeg, RunMetrics,
    SafeLogoutCompletionOutcome, CHECKPOINT4_BLOCKING_STOCKADE_COUNT,
};
use siege_perilous::headless_bot::Bot;
use siege_perilous::obj::Position;
use siege_perilous::safe_logout::SafeLogoutRejectionReason;
use siege_perilous::{PlayerEvent, StartLocations};

const SCHEMA_VERSION: &str = "goblin_crisis_checkpoint4_runner_v2";
const DEFAULT_REPETITIONS: u32 = 1;
const DEFAULT_ASSAULT_CAP_TICKS: i32 = 15_000;
const PRELAUNCH_CAP_TICKS: i32 = 12_000;
const BALANCE_SAMPLE_INTERVAL_TICKS: i32 = 1;
const DECISION_TICKS: u32 = 8;
const ORDINARY_DISCONNECT_TICKS: u32 = 100;
const SAFE_LOGOUT_PROTECTED_TICKS: u32 = 250;
const HELPER_RENDEZVOUS_CAP_TICKS: i32 = 2_000;
const HELPER_RENDEZVOUS_DISTANCE: u32 = 4;
const HERO_CLASSES: [&str; 3] = ["Warrior", "Ranger", "Mage"];
const ADJACENT_START_LOCATIONS: [&str; 2] = ["startpos2", "startpos4"];
const ADJACENT_OWNER_START_LOCATION: &str = "startpos4";
const ADJACENT_HELPER_START_LOCATION: &str = "startpos2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WorkloadProfile {
    CorrectedBaseline,
    FocusedPreparation,
    EdgeCases,
    Full,
}

impl WorkloadProfile {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "corrected-baseline" | "corrected_baseline" | "baseline" => {
                Some(Self::CorrectedBaseline)
            }
            "focused-preparation" | "focused_preparation" | "preparation" => {
                Some(Self::FocusedPreparation)
            }
            "edge-cases" | "edge_cases" | "edges" => Some(Self::EdgeCases),
            "full" => Some(Self::Full),
            _ => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::CorrectedBaseline => "corrected_baseline",
            Self::FocusedPreparation => "focused_preparation",
            Self::EdgeCases => "edge_cases",
            Self::Full => "full",
        }
    }

    const fn includes_baseline(self) -> bool {
        matches!(self, Self::CorrectedBaseline | Self::Full)
    }

    const fn includes_preparation(self) -> bool {
        matches!(self, Self::FocusedPreparation | Self::Full)
    }

    const fn includes_edges(self) -> bool {
        matches!(self, Self::EdgeCases | Self::Full)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunnerConfig {
    output: PathBuf,
    profile: WorkloadProfile,
    repetitions: u32,
    assault_cap_ticks: i32,
    build_profile_label: String,
}

fn usage() -> &'static str {
    "usage: goblin_crisis_checkpoint4_runner --output PATH \
        [--profile corrected-baseline|focused-preparation|edge-cases|full] \
        [--repetitions N] [--assault-cap-ticks N] [--build-profile LABEL]"
}

fn parse_args_from<I, S>(args: I) -> Result<RunnerConfig, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut output = None;
    let mut profile = WorkloadProfile::Full;
    let mut repetitions = DEFAULT_REPETITIONS;
    let mut assault_cap_ticks = DEFAULT_ASSAULT_CAP_TICKS;
    let mut build_profile_label = "unspecified".to_string();
    let mut args = args.into_iter().map(Into::into);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => {
                output = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--output requires a path".to_string())?,
                ));
            }
            "--profile" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--profile requires a label".to_string())?;
                profile = WorkloadProfile::parse(&value)
                    .ok_or_else(|| format!("invalid --profile value: {value}"))?;
            }
            "--repetitions" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--repetitions requires a positive integer".to_string())?;
                repetitions = value
                    .parse::<u32>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| format!("invalid --repetitions value: {value}"))?;
            }
            "--assault-cap-ticks" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--assault-cap-ticks requires a positive integer".to_string())?;
                assault_cap_ticks = value
                    .parse::<i32>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| format!("invalid --assault-cap-ticks value: {value}"))?;
            }
            "--build-profile" => {
                build_profile_label = args
                    .next()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| "--build-profile requires a non-empty label".to_string())?;
            }
            "--help" | "-h" => return Err(usage().to_string()),
            _ => return Err(format!("unknown argument: {arg}\n{}", usage())),
        }
    }

    let output = output.ok_or_else(|| format!("--output is required\n{}", usage()))?;
    reject_protected_artifact_name(&output)?;
    if output.exists() {
        return Err(format!(
            "refusing to overwrite existing output: {}",
            output.display()
        ));
    }
    Ok(RunnerConfig {
        output,
        profile,
        repetitions,
        assault_cap_ticks,
        build_profile_label,
    })
}

fn parse_args() -> Result<RunnerConfig, String> {
    parse_args_from(std::env::args().skip(1))
}

fn reject_protected_artifact_name(path: &Path) -> Result<(), String> {
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let checkpoint_1_to_3 = ["checkpoint1", "checkpoint2", "checkpoint3"];
    let known_earlier_artifacts = [
        "goblin_crisis_balance_report.json",
        "goblin_crisis_balance_checkpoint2_control_report.json",
        "goblin_crisis_balance_checkpoint2_candidate_report.json",
        "goblin_crisis_balance_checkpoint3_pairs.json",
        "headless_runs.json",
        "headless_runs.csv",
    ];
    if checkpoint_1_to_3
        .iter()
        .any(|checkpoint| filename.contains(checkpoint))
        || known_earlier_artifacts.contains(&filename.as_str())
    {
        return Err(format!(
            "refusing to use a Checkpoint 1-3 artifact name: {}",
            path.display()
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct Provenance {
    git_commit: Option<String>,
    git_dirty: Option<bool>,
    current_dir: Option<String>,
    cargo_manifest_dir: Option<String>,
    build_profile_label: String,
    invocation: Vec<String>,
}

impl Provenance {
    fn capture(build_profile_label: &str) -> Self {
        let git_commit = command_stdout("git", &["rev-parse", "HEAD"]);
        let git_dirty =
            command_stdout("git", &["status", "--porcelain"]).map(|status| !status.is_empty());
        Self {
            git_commit,
            git_dirty,
            current_dir: std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string()),
            cargo_manifest_dir: std::env::var("CARGO_MANIFEST_DIR").ok(),
            build_profile_label: build_profile_label.to_string(),
            invocation: std::env::args().collect(),
        }
    }
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[derive(Debug, Clone, Serialize)]
struct Methodology {
    random_stream_replayed: bool,
    full_ecs_state_matched: bool,
    entropy_source: &'static str,
    run_label_semantics: &'static str,
    stopping_rule: &'static str,
    pair_order: &'static str,
    engagement_sampling: &'static str,
    periodic_pressure_output: &'static str,
    output_policy: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct PanicRecord {
    category: String,
    payload: String,
}

#[derive(Debug, Clone, Serialize)]
struct LaunchObservation {
    assault_launch_tick: i32,
    assault_id: Option<u64>,
    spawn_generation: u32,
    owner_player_id: i32,
    unit_count: usize,
    units: Vec<LaunchUnitObservation>,
    hero_position: Option<[i32; 2]>,
    settlement_anchor: Option<[i32; 2]>,
}

#[derive(Debug, Clone, Serialize)]
struct LaunchUnitObservation {
    obj_id: i32,
    template: String,
    owner_player_id: i32,
    assault_id: u64,
    spawn_generation: u32,
    position: [i32; 2],
    hp: i32,
    base_hp: i32,
    vision: u32,
    has_thinker: bool,
    initial_target: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize)]
struct EdgeObservations {
    retained_start_locations: Vec<RetainedStartLocation>,
    neighbour_player_id: Option<i32>,
    neighbour_hero_id: Option<i32>,
    neighbour_villager_ids: Vec<i32>,
    neighbour_structure_ids: Vec<i32>,
    neighbour_sanctuary_id: Option<i32>,
    neighbour_anchor: Option<[i32; 2]>,
    owner_neighbour_anchor_distance: Option<u32>,
    neighbour_footprint_tiles: Vec<[i32; 2]>,
    spawn_overlaps_neighbour_footprint: Vec<i32>,
    spawn_neighbour_exclusion_violations: Vec<i32>,
    neighbour_target_violations: Vec<NeighbourTargetViolation>,
    cross_player_target_violations_telemetry: Option<i32>,
    helper_prelaunch_start_position: Option<[i32; 2]>,
    helper_assault_launch_position: Option<[i32; 2]>,
    helper_prelaunch_move_events: i32,
    helper_launch_distance_to_owner: Option<u32>,
    owner_disconnected_during_assault: bool,
    disconnect_preserved_active_phase: Option<bool>,
    disconnect_preserved_timing: Option<bool>,
    disconnect_preserved_unit_ids: Option<bool>,
    disconnect_did_not_heal_units: Option<bool>,
    reconnect_did_not_reset_units: Option<bool>,
    owner_reconnected_to_same_assault: Option<bool>,
    helper_damage_before_owner_disconnect: Option<i32>,
    helper_damage_while_owner_offline: i32,
    helper_attacks_accepted_while_owner_offline: i32,
    helper_drove_combat_while_owner_offline: bool,
    helper_departed_after_dealing_damage: bool,
    helper_departure_preserved_assault: Option<bool>,
    owner_target_loss_observed: bool,
    safe_logout_completion: Option<String>,
    safe_logout_prelaunch_state_frozen: Option<bool>,
    safe_logout_world_state_frozen: Option<bool>,
    safe_logout_launched_while_protected: Option<bool>,
    safe_logout_active_assault_rejected: Option<bool>,
    true_death_cleanup_removed_crisis: Option<bool>,
    true_death_cleanup_removed_assault_units: Option<bool>,
    true_death_cleanup_granted_resolution: Option<bool>,
    fresh_run_crisis_reset: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct RetainedStartLocation {
    name: String,
    hero_position: [i32; 2],
    burrow_position: [i32; 2],
    monolith_position: [i32; 2],
}

#[derive(Debug, Clone, Serialize)]
struct NeighbourTargetViolation {
    assault_unit_id: i32,
    target_id: i32,
    target_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct RunRow {
    run_label: String,
    workload_profile: String,
    scenario: String,
    hero_class: String,
    repetition: u32,
    preparation_comparison: Option<String>,
    preparation_leg: Option<String>,
    setup_cohort: String,
    engagement_sample_interval_ticks: i32,
    random_stream_replayed: bool,
    full_ecs_state_matched: bool,
    status: String,
    setup_failure: Option<String>,
    panic: Option<PanicRecord>,
    invariant_failures: Vec<String>,
    launch_validation_error: Option<String>,
    preparation_launch: Option<PreparationPairLaunch>,
    declared_preparation_difference: Option<PreparationDeclaredDifference>,
    launch: Option<LaunchObservation>,
    engagement: Value,
    assault_resolved: bool,
    assault_units_defeated: i32,
    assault_units_remaining: i32,
    hero_damage_taken: i32,
    hero_deaths_during_assault: i32,
    hero_true_death: bool,
    hero_missing: bool,
    hero_survived: bool,
    observed_assault_ticks: i32,
    assault_duration_ticks: Option<i32>,
    tick_cap_reached: bool,
    unresolved: bool,
    defeat_cause: Option<String>,
    defeat_cause_evidence: BTreeMap<String, Value>,
    edge_observations: EdgeObservations,
    crisis_balance: Option<CrisisBalanceTelemetry>,
    run_metrics: Option<RunMetrics>,
}

impl RunRow {
    fn failure(spec: &RunSpec, status: &str, failure: String) -> Self {
        Self {
            run_label: spec.run_label.clone(),
            workload_profile: spec.workload_profile.to_string(),
            scenario: spec.scenario.to_string(),
            hero_class: spec.hero_class.to_string(),
            repetition: spec.repetition,
            preparation_comparison: spec.preparation_comparison.map(str::to_string),
            preparation_leg: spec.preparation_leg.map(str::to_string),
            setup_cohort: spec.setup_cohort.to_string(),
            engagement_sample_interval_ticks: BALANCE_SAMPLE_INTERVAL_TICKS,
            random_stream_replayed: false,
            full_ecs_state_matched: false,
            status: status.to_string(),
            setup_failure: (status == "setup_failure").then_some(failure.clone()),
            panic: (status == "panic").then(|| panic_record(failure)),
            invariant_failures: Vec::new(),
            launch_validation_error: None,
            preparation_launch: None,
            declared_preparation_difference: None,
            launch: None,
            engagement: Value::Null,
            assault_resolved: false,
            assault_units_defeated: 0,
            assault_units_remaining: 0,
            hero_damage_taken: 0,
            hero_deaths_during_assault: 0,
            hero_true_death: false,
            hero_missing: false,
            hero_survived: false,
            observed_assault_ticks: 0,
            assault_duration_ticks: None,
            tick_cap_reached: false,
            unresolved: true,
            defeat_cause: None,
            defeat_cause_evidence: BTreeMap::new(),
            edge_observations: EdgeObservations::default(),
            crisis_balance: None,
            run_metrics: None,
        }
    }
}

#[derive(Debug, Clone)]
struct RunSpec {
    run_label: String,
    workload_profile: &'static str,
    scenario: &'static str,
    hero_class: &'static str,
    repetition: u32,
    preparation_comparison: Option<&'static str>,
    preparation_leg: Option<&'static str>,
    setup_cohort: &'static str,
}

#[derive(Debug)]
struct CompletedLeg {
    row: RunRow,
    launch: PreparationPairLaunch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeScenario {
    OrdinaryDisconnect,
    SafeLogoutBeforeLaunch,
    HelperSupported,
    HelperDeparture,
    OfflineHelper,
    AdjacentSettlement,
    AdjacentTargetLoss,
    TrueDeathCleanupFreshRun,
}

impl EdgeScenario {
    const ALL: [Self; 8] = [
        Self::OrdinaryDisconnect,
        Self::SafeLogoutBeforeLaunch,
        Self::HelperSupported,
        Self::HelperDeparture,
        Self::OfflineHelper,
        Self::AdjacentSettlement,
        Self::AdjacentTargetLoss,
        Self::TrueDeathCleanupFreshRun,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::OrdinaryDisconnect => "ordinary_disconnect",
            Self::SafeLogoutBeforeLaunch => "safe_logout_before_launch",
            Self::HelperSupported => "helper_supported",
            Self::HelperDeparture => "helper_departure",
            Self::OfflineHelper => "offline_owner_helper_supported",
            Self::AdjacentSettlement => "adjacent_settlement_isolation",
            Self::AdjacentTargetLoss => "adjacent_settlement_target_loss",
            Self::TrueDeathCleanupFreshRun => "true_death_cleanup_fresh_run",
        }
    }
}

#[derive(Debug, Serialize)]
struct Aggregate {
    rows: usize,
    resolved: usize,
    true_deaths: usize,
    missing_heroes: usize,
    timeouts: usize,
    unresolved: usize,
    setup_failures: usize,
    panics: usize,
    invalid_launch_fingerprints: usize,
    invariant_failure_rows: usize,
    rows_with_engagement_telemetry: usize,
}

impl Aggregate {
    fn from_rows(rows: &[RunRow]) -> Self {
        Self {
            rows: rows.len(),
            resolved: rows.iter().filter(|row| row.assault_resolved).count(),
            true_deaths: rows.iter().filter(|row| row.hero_true_death).count(),
            missing_heroes: rows.iter().filter(|row| row.hero_missing).count(),
            timeouts: rows.iter().filter(|row| row.tick_cap_reached).count(),
            unresolved: rows
                .iter()
                .filter(|row| row.unresolved && row.launch.is_some())
                .count(),
            setup_failures: rows
                .iter()
                .filter(|row| row.status == "setup_failure")
                .count(),
            panics: rows.iter().filter(|row| row.status == "panic").count(),
            invalid_launch_fingerprints: rows
                .iter()
                .filter(|row| row.launch_validation_error.is_some())
                .count(),
            invariant_failure_rows: rows
                .iter()
                .filter(|row| !row.invariant_failures.is_empty())
                .count(),
            rows_with_engagement_telemetry: rows
                .iter()
                .filter(|row| !row.engagement.is_null())
                .count(),
        }
    }
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: &'static str,
    workload_profile: String,
    repetitions: u32,
    assault_relative_cap_ticks: i32,
    prelaunch_cap_ticks: i32,
    methodology: Methodology,
    provenance: Provenance,
    rows: Vec<RunRow>,
    aggregate: Aggregate,
    limitations: Vec<&'static str>,
}

fn panic_record(payload: String) -> PanicRecord {
    let lower = payload.to_ascii_lowercase();
    let category = if lower.contains("windstride stag") {
        "missing_windstride_stag_template"
    } else if lower.contains("cannot find item template") || lower.contains("missing template") {
        "missing_template"
    } else if lower.contains("no such file") || lower.contains("could not read") {
        "runtime_layout_or_io"
    } else if lower.contains("assert") || lower.contains("expected") {
        "assertion_or_invariant"
    } else {
        "unknown"
    };
    PanicRecord {
        category: category.to_string(),
        payload,
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else {
        "non-string panic payload".to_string()
    }
}

fn position_array(position: Position) -> [i32; 2] {
    [position.x, position.y]
}

fn hex_distance(source: [i32; 2], destination: Position) -> u32 {
    fn odd_q_to_cube(q: i32, r: i32) -> (i32, i32, i32) {
        let x = q;
        let z = r - (q - (q & 1)) / 2;
        let y = -x - z;
        (x, y, z)
    }
    let (sx, sy, sz) = odd_q_to_cube(source[0], source[1]);
    let (dx, dy, dz) = odd_q_to_cube(destination.x, destination.y);
    (((sx - dx).abs() + (sy - dy).abs() + (sz - dz).abs()) / 2) as u32
}

fn capture_launch(game: &mut HeadlessGame) -> Result<LaunchObservation, String> {
    let player_id = game.player_id();
    let crisis = game
        .settlement_crisis()
        .ok_or_else(|| "missing crisis at assault launch".to_string())?;
    if crisis.phase != CrisisPhase::AssaultActive {
        return Err(format!(
            "expected AssaultActive at launch capture, got {:?}",
            crisis.phase
        ));
    }
    let assault_id = crisis
        .assault_id
        .ok_or_else(|| "AssaultActive crisis has no assault identity".to_string())?;
    if crisis.assault_spawn_generation == 0 {
        return Err("AssaultActive crisis has generation zero".to_string());
    }
    let launch_tick = crisis
        .assault_started_tick
        .ok_or_else(|| "AssaultActive crisis has no launch tick".to_string())?;
    let expected_ids = crisis
        .assault_unit_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if expected_ids.len() != crisis.assault_unit_ids.len() {
        return Err("AssaultActive crisis contains duplicate unit ids".to_string());
    }
    let view = game.observe();
    let config = goblin_crisis_balance_config_snapshot();
    let live_units = game
        .crisis_assault_units()
        .into_iter()
        .filter(|unit| expected_ids.contains(&unit.obj_id))
        .collect::<Vec<_>>();
    if live_units.len() != expected_ids.len() {
        return Err(format!(
            "launch entity count mismatch: crisis ids={}, live attributed entities={}",
            expected_ids.len(),
            live_units.len()
        ));
    }
    let mut actual_templates = live_units
        .iter()
        .map(|unit| unit.template.clone())
        .collect::<Vec<_>>();
    let mut expected_templates = config.assault_composition.clone();
    actual_templates.sort();
    expected_templates.sort();
    if actual_templates != expected_templates {
        return Err(format!(
            "launch composition mismatch: expected={expected_templates:?}, actual={actual_templates:?}"
        ));
    }
    let mut positions = BTreeSet::new();
    for unit in &live_units {
        if unit.owner_player_id != player_id
            || unit.assault_id != assault_id
            || unit.spawn_generation != crisis.assault_spawn_generation
        {
            return Err(format!(
                "unit {} has stale/wrong attribution owner={} assault={} generation={}",
                unit.obj_id, unit.owner_player_id, unit.assault_id, unit.spawn_generation
            ));
        }
        if unit.dead || unit.hp <= 0 || unit.base_hp <= 0 {
            return Err(format!("unit {} is not live at launch", unit.obj_id));
        }
        if unit.vision != config.assault_vision || !unit.has_thinker {
            return Err(format!(
                "unit {} missing launch AI facts: vision={} expected={} thinker={}",
                unit.obj_id, unit.vision, config.assault_vision, unit.has_thinker
            ));
        }
        if !positions.insert((unit.pos.x, unit.pos.y)) || !game.is_land_passable(unit.pos) {
            return Err(format!(
                "unit {} has duplicate or impassable launch position ({}, {})",
                unit.obj_id, unit.pos.x, unit.pos.y
            ));
        }
    }
    let units = live_units
        .into_iter()
        .map(|unit| LaunchUnitObservation {
            obj_id: unit.obj_id,
            template: unit.template,
            owner_player_id: unit.owner_player_id,
            assault_id: unit.assault_id,
            spawn_generation: unit.spawn_generation,
            position: position_array(unit.pos),
            hp: unit.hp,
            base_hp: unit.base_hp,
            vision: unit.vision,
            has_thinker: unit.has_thinker,
            initial_target: unit
                .visible_target
                .or(unit.target)
                .or(unit.task_target)
                .filter(|target| *target != NO_TARGET),
        })
        .collect::<Vec<_>>();
    Ok(LaunchObservation {
        assault_launch_tick: launch_tick,
        assault_id: Some(assault_id),
        spawn_generation: crisis.assault_spawn_generation,
        owner_player_id: player_id,
        unit_count: units.len(),
        units,
        hero_position: view.hero.map(|hero| position_array(hero.pos)),
        settlement_anchor: view.home().map(position_array),
    })
}

fn engagement_value(telemetry: &CrisisBalanceTelemetry) -> Value {
    let Ok(value) = serde_json::to_value(telemetry) else {
        return Value::Null;
    };
    if let Some(nested) = [
        "engagement",
        "assault_engagement",
        "engagement_pipeline",
        "engagement_telemetry",
    ]
    .into_iter()
    .find_map(|key| value.get(key).cloned())
    {
        return nested;
    }

    // Keep this runner compatible whether the instrumentation is represented
    // by a nested struct or appended to the existing outcome object.
    let engagement_fields = [
        "assault_launch_tick",
        "first_attacker_visible_tick",
        "first_attacker_target_acquired_tick",
        "first_hero_target_acquired_tick",
        "first_hero_move_toward_attacker_tick",
        "first_attacker_move_toward_target_tick",
        "first_hero_attack_requested_tick",
        "first_hero_attack_accepted_tick",
        "first_attacker_attack_requested_tick",
        "first_attacker_attack_accepted_tick",
        "first_hero_hit_tick",
        "first_attacker_hit_tick",
        "first_damage_to_attacker_tick",
        "first_damage_to_hero_tick",
        "first_damage_to_villager_tick",
        "first_damage_to_structure_tick",
        "minimum_hero_attacker_distance",
        "minimum_attacker_settlement_distance",
        "hero_attack_attempts",
        "hero_attacks_accepted",
        "hero_hits",
        "hero_damage_dealt_to_assault",
        "healing_items_used_during_assault",
        "healing_hp_restored_during_assault",
        "attacker_attack_attempts",
        "attacker_attacks_accepted",
        "attacker_hits",
        "attacker_target_changes",
        "hero_target_changes",
        "ticks_without_any_valid_target",
        "ticks_with_target_but_no_movement",
        "ticks_in_attack_range_but_no_attack",
        "engagement_failure_reason",
    ];
    let fields = engagement_fields
        .into_iter()
        .filter_map(|key| {
            find_value_recursive(&value, key)
                .cloned()
                .map(|field| (key.to_string(), field))
        })
        .collect::<serde_json::Map<_, _>>();
    if fields.is_empty() {
        Value::Null
    } else {
        Value::Object(fields)
    }
}

fn telemetry_string(telemetry: &CrisisBalanceTelemetry, keys: &[&str]) -> Option<String> {
    let value = serde_json::to_value(telemetry).ok()?;
    keys.iter()
        .find_map(|key| find_value_recursive(&value, key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn find_value_recursive<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(object) => object.get(key).or_else(|| {
            object
                .values()
                .find_map(|child| find_value_recursive(child, key))
        }),
        Value::Array(values) => values
            .iter()
            .find_map(|child| find_value_recursive(child, key)),
        _ => None,
    }
}

fn classify_defeat_cause(
    resolved: bool,
    true_death: bool,
    tick_cap_reached: bool,
    metrics: &RunMetrics,
    telemetry: &CrisisBalanceTelemetry,
    launch: &LaunchObservation,
) -> (Option<String>, BTreeMap<String, Value>) {
    let mut evidence = BTreeMap::new();
    evidence.insert("killer".to_string(), Value::String(metrics.killer.clone()));
    evidence.insert(
        "assault_attributed_hero_damage".to_string(),
        Value::from(telemetry.assault_outcome.hero_damage_taken),
    );
    evidence.insert(
        "assault_unit_templates".to_string(),
        Value::Array(
            launch
                .units
                .iter()
                .map(|unit| Value::String(unit.template.clone()))
                .collect(),
        ),
    );
    if resolved {
        return (None, evidence);
    }
    if let Some(recorded) = telemetry_string(telemetry, &["defeat_cause"]) {
        evidence.insert(
            "telemetry_classification".to_string(),
            Value::String(recorded.clone()),
        );
        return (Some(recorded), evidence);
    }
    if true_death {
        let needs = matches!(
            metrics.killer.as_str(),
            "Dehydration" | "Starvation" | "Exhaustion" | "Burns"
        );
        if needs {
            return (Some("hero_death_from_needs".to_string()), evidence);
        }
        let attributed_killer = launch
            .units
            .iter()
            .any(|unit| unit.template == metrics.killer)
            && telemetry.assault_outcome.hero_damage_taken > 0;
        if attributed_killer {
            return (Some("hero_true_death_from_assault".to_string()), evidence);
        }
        // A non-empty display name is not attribution. Ambient classification is
        // accepted only from the authoritative death observer above; otherwise
        // retain the row as unknown instead of guessing from presentation text.
        return (Some("unknown".to_string()), evidence);
    }
    if tick_cap_reached {
        let failure = telemetry_string(telemetry, &["engagement_failure_reason"]);
        let cause = match failure.as_deref() {
            Some("path_unreachable" | "npc_policy_no_move" | "hero_policy_no_move") => {
                "pathing_stall"
            }
            Some("tick_cap") | None => "assault_unresolved_at_tick_cap",
            Some(_) => "combat_stall",
        };
        return (Some(cause.to_string()), evidence);
    }
    (Some("unknown".to_string()), evidence)
}

fn finish_row(
    spec: &RunSpec,
    game: &mut HeadlessGame,
    launch: LaunchObservation,
    mut edge_observations: EdgeObservations,
    assault_cap_ticks: i32,
) -> RunRow {
    let observed_assault_ticks = game
        .game_tick()
        .saturating_sub(launch.assault_launch_tick)
        .max(0);
    let phase = game.settlement_crisis().map(|crisis| crisis.phase);
    let final_view = game.observe();
    let hero_missing = final_view.hero.is_none();
    let hero_true_death = final_view.hero.is_some_and(|hero| hero.true_death);
    let hero_survived = final_view
        .hero
        .is_some_and(|hero| !hero.dead && !hero.true_death);
    let tick_cap_reached = phase != Some(CrisisPhase::Resolved)
        && !hero_true_death
        && !hero_missing
        && observed_assault_ticks >= assault_cap_ticks;
    if tick_cap_reached {
        let player_id = game.player_id();
        game.app_mut()
            .world_mut()
            .resource_mut::<CrisisBalanceTelemetryState>()
            .entry(player_id)
            .or_default()
            .engagement
            .record_tick_cap_failure();
    }
    let mut metrics = game.metrics();
    metrics.crisis_balance_scenario = spec.scenario.to_string();
    metrics.crisis_balance_hero_class = spec.hero_class.to_string();
    metrics.crisis_balance_run_id = spec.run_label.clone();
    metrics.crisis_balance_tick_cap = assault_cap_ticks;
    metrics.crisis_balance_tick_cap_reached = tick_cap_reached;
    // Engagement sampling remains exact at one-tick resolution. Periodic pressure
    // snapshots repeat transition/final facts and otherwise dominate retained
    // matrix size, so Checkpoint 4 omits only that redundant vector from each row.
    metrics.crisis_balance.pressure_snapshots.periodic.clear();
    let telemetry = metrics.crisis_balance.clone();
    edge_observations.cross_player_target_violations_telemetry =
        Some(telemetry.assault_outcome.cross_player_target_violations);
    let resolved =
        phase == Some(CrisisPhase::Resolved) || telemetry.assault_outcome.assault_resolved;
    let (defeat_cause, defeat_cause_evidence) = classify_defeat_cause(
        resolved,
        hero_true_death,
        tick_cap_reached,
        &metrics,
        &telemetry,
        &launch,
    );
    let status = if resolved {
        "resolved"
    } else if hero_true_death {
        "true_death"
    } else if hero_missing {
        "missing_hero"
    } else if tick_cap_reached {
        "timeout_unresolved"
    } else {
        "unresolved"
    };

    RunRow {
        run_label: spec.run_label.clone(),
        workload_profile: spec.workload_profile.to_string(),
        scenario: spec.scenario.to_string(),
        hero_class: spec.hero_class.to_string(),
        repetition: spec.repetition,
        preparation_comparison: spec.preparation_comparison.map(str::to_string),
        preparation_leg: spec.preparation_leg.map(str::to_string),
        setup_cohort: spec.setup_cohort.to_string(),
        engagement_sample_interval_ticks: BALANCE_SAMPLE_INTERVAL_TICKS,
        random_stream_replayed: false,
        full_ecs_state_matched: false,
        status: status.to_string(),
        setup_failure: None,
        panic: None,
        invariant_failures: Vec::new(),
        launch_validation_error: None,
        preparation_launch: None,
        declared_preparation_difference: None,
        launch: Some(launch),
        engagement: engagement_value(&telemetry),
        assault_resolved: resolved,
        assault_units_defeated: telemetry.assault_outcome.assault_units_defeated,
        assault_units_remaining: telemetry.assault_outcome.assault_units_remaining,
        hero_damage_taken: telemetry.assault_outcome.hero_damage_taken,
        hero_deaths_during_assault: telemetry.assault_outcome.hero_deaths_during_assault,
        hero_true_death,
        hero_missing,
        hero_survived,
        observed_assault_ticks,
        assault_duration_ticks: telemetry.assault_outcome.assault_duration_ticks,
        tick_cap_reached,
        unresolved: !resolved,
        defeat_cause,
        defeat_cause_evidence,
        edge_observations,
        crisis_balance: Some(telemetry),
        run_metrics: Some(metrics),
    }
}

fn drive_assault(
    game: &mut HeadlessGame,
    owner_bot: &mut Bot,
    mut helper: Option<(i32, &mut Bot)>,
    launch_tick: i32,
    cap_ticks: i32,
    treatment_healing: bool,
    edge: EdgeScenarioRuntime,
    observations: &mut EdgeObservations,
) {
    let mut bandage_requested = false;
    let mut disconnect_done = false;
    let mut helper_damage_at_owner_disconnect = None;
    let mut helper_attacks_at_owner_disconnect = None;
    let mut target_violations = BTreeSet::new();

    loop {
        let phase = game.settlement_crisis().map(|crisis| crisis.phase);
        if phase == Some(CrisisPhase::Resolved) {
            break;
        }
        let mut owner_view = game.observe();
        if edge.record_owner_target_loss
            && owner_view
                .hero
                .is_some_and(|hero| hero.dead && !hero.true_death)
        {
            observations.owner_target_loss_observed = true;
        }
        let hero_terminal = owner_view.hero.is_none_or(|hero| hero.true_death);
        if hero_terminal {
            break;
        }
        let elapsed = game.game_tick().saturating_sub(launch_tick).max(0);
        if elapsed >= cap_ticks {
            break;
        }

        observe_neighbour_targets(game, &edge.neighbour_targets, &mut target_violations);

        let helper_ready_for_disconnect = !edge.disconnect_after_helper_engagement
            || game
                .crisis_balance_telemetry()
                .engagement
                .helper_damage_dealt_to_assault
                > 0;
        if edge.disconnect_owner
            && !disconnect_done
            && elapsed >= DECISION_TICKS as i32
            && helper_ready_for_disconnect
        {
            disconnect_done = true;
            observations.owner_disconnected_during_assault = true;
            let engagement = &game.crisis_balance_telemetry().engagement;
            helper_damage_at_owner_disconnect = Some(engagement.helper_damage_dealt_to_assault);
            helper_attacks_at_owner_disconnect = Some(engagement.helper_attacks_accepted);
            observations.helper_damage_before_owner_disconnect = helper_damage_at_owner_disconnect;
            let before = game.settlement_crisis();
            let units_before = game.crisis_assault_units();
            game.disconnect_player();
            let requested_disconnect_ticks = if edge.leave_owner_offline {
                1
            } else {
                ORDINARY_DISCONNECT_TICKS
            };
            let disconnect_ticks = requested_disconnect_ticks.min(
                cap_ticks
                    .saturating_sub(elapsed)
                    .max(0)
                    .try_into()
                    .unwrap_or_default(),
            );
            tick_with_neighbour_observation(
                game,
                disconnect_ticks,
                &edge.neighbour_targets,
                &mut target_violations,
            );
            let offline = game.settlement_crisis();
            let units_offline = game.crisis_assault_units();
            observations.disconnect_preserved_active_phase = Some(
                offline.as_ref().map(|crisis| crisis.phase) == Some(CrisisPhase::AssaultActive),
            );
            observations.disconnect_preserved_timing =
                Some(before.as_ref().is_some_and(|before| {
                    offline.as_ref().is_some_and(|offline| {
                        before.assault_id == offline.assault_id
                            && before.assault_spawn_generation == offline.assault_spawn_generation
                            && before.assault_started_tick == offline.assault_started_tick
                            && before.phase_started_tick == offline.phase_started_tick
                    })
                }));
            observations.disconnect_preserved_unit_ids = Some(
                units_before
                    .iter()
                    .map(|unit| unit.obj_id)
                    .collect::<Vec<_>>()
                    == units_offline
                        .iter()
                        .map(|unit| unit.obj_id)
                        .collect::<Vec<_>>(),
            );
            observations.disconnect_did_not_heal_units =
                Some(units_offline.iter().all(|offline| {
                    units_before
                        .iter()
                        .find(|before| before.obj_id == offline.obj_id)
                        .is_some_and(|before| {
                            offline.hp <= before.hp && offline.base_hp == before.base_hp
                        })
                }));
            if !edge.leave_owner_offline {
                game.reconnect_player_with_login();
                let reconnect_ticks = 8_u32.min(
                    cap_ticks
                        .saturating_sub(game.game_tick().saturating_sub(launch_tick))
                        .max(0)
                        .try_into()
                        .unwrap_or_default(),
                );
                tick_with_neighbour_observation(
                    game,
                    reconnect_ticks,
                    &edge.neighbour_targets,
                    &mut target_violations,
                );
                let after = game.settlement_crisis();
                let units_after = game.crisis_assault_units();
                observations.owner_reconnected_to_same_assault = Some(
                    before.as_ref().and_then(|crisis| crisis.assault_id)
                        == after.as_ref().and_then(|crisis| crisis.assault_id)
                        && before
                            .as_ref()
                            .map(|crisis| crisis.assault_spawn_generation)
                            == after.as_ref().map(|crisis| crisis.assault_spawn_generation),
                );
                observations.reconnect_did_not_reset_units = Some(
                    units_offline
                        .iter()
                        .map(|unit| unit.obj_id)
                        .collect::<Vec<_>>()
                        == units_after
                            .iter()
                            .map(|unit| unit.obj_id)
                            .collect::<Vec<_>>()
                        && units_after.iter().all(|after| {
                            units_offline
                                .iter()
                                .find(|offline| offline.obj_id == after.obj_id)
                                .is_some_and(|offline| {
                                    after.hp <= offline.hp && after.base_hp == offline.base_hp
                                })
                        }),
                );
            }
            owner_view = game.observe();
            if owner_view.hero.is_none_or(|hero| hero.true_death)
                || game.settlement_crisis().map(|crisis| crisis.phase)
                    == Some(CrisisPhase::Resolved)
                || game.game_tick().saturating_sub(launch_tick) >= cap_ticks
            {
                break;
            }
        }

        let owner_connected = game.is_player_connected(game.player_id());
        let mut ordinary_healing_action = false;
        if owner_connected && treatment_healing && !bandage_requested {
            if let Some(event) = game.preparation_bandage_use_event() {
                game.inject(event);
                bandage_requested = true;
                ordinary_healing_action = true;
            }
        }
        if owner_connected && !ordinary_healing_action {
            let event = owner_bot.step(&owner_view, game.map());
            if let Some(target_id) = owner_bot.observed_assault_target_id() {
                game.record_observed_crisis_target(target_id);
            }
            if let Some(event) = event {
                game.inject(event);
            }
            owner_bot.advance_phase(&owner_view);
        }

        if edge.depart_helper_after_damage && !observations.helper_departed_after_dealing_damage {
            if let Some((helper_id, _)) = helper.as_ref() {
                let helper_has_dealt_damage = game
                    .crisis_balance_telemetry()
                    .engagement
                    .helper_damage_dealt_to_assault
                    > 0;
                if helper_has_dealt_damage {
                    let before = game.settlement_crisis();
                    game.disconnect_scenario_player(*helper_id);
                    let after = game.settlement_crisis();
                    observations.helper_departed_after_dealing_damage = true;
                    observations.helper_departure_preserved_assault = Some(
                        before.as_ref().and_then(|crisis| crisis.assault_id)
                            == after.as_ref().and_then(|crisis| crisis.assault_id)
                            && before
                                .as_ref()
                                .map(|crisis| crisis.assault_spawn_generation)
                                == after.as_ref().map(|crisis| crisis.assault_spawn_generation),
                    );
                }
            }
        }

        if let Some((helper_id, helper_bot)) = helper.as_mut() {
            if !game.is_player_connected(*helper_id) {
                let remaining =
                    cap_ticks.saturating_sub(game.game_tick().saturating_sub(launch_tick).max(0));
                tick_with_neighbour_observation(
                    game,
                    DECISION_TICKS.min(remaining.max(1) as u32),
                    &edge.neighbour_targets,
                    &mut target_violations,
                );
                continue;
            }
            let helper_view = game.observe_for_player(*helper_id);
            if let Some(event) = helper_bot.step(&helper_view, game.map()) {
                game.inject(event);
            }
            helper_bot.advance_phase(&helper_view);
        }

        let remaining =
            cap_ticks.saturating_sub(game.game_tick().saturating_sub(launch_tick).max(0));
        tick_with_neighbour_observation(
            game,
            DECISION_TICKS.min(remaining.max(1) as u32),
            &edge.neighbour_targets,
            &mut target_violations,
        );
    }

    observe_neighbour_targets(game, &edge.neighbour_targets, &mut target_violations);
    if edge.leave_owner_offline {
        let engagement = &game.crisis_balance_telemetry().engagement;
        observations.helper_damage_while_owner_offline = helper_damage_at_owner_disconnect
            .map(|before| {
                engagement
                    .helper_damage_dealt_to_assault
                    .saturating_sub(before)
                    .max(0)
            })
            .unwrap_or(0);
        observations.helper_attacks_accepted_while_owner_offline =
            helper_attacks_at_owner_disconnect
                .map(|before| {
                    engagement
                        .helper_attacks_accepted
                        .saturating_sub(before)
                        .max(0)
                })
                .unwrap_or(0);
        observations.helper_drove_combat_while_owner_offline =
            observations.helper_attacks_accepted_while_owner_offline > 0
                && observations.helper_damage_while_owner_offline > 0;
    }
    observations.neighbour_target_violations = target_violations
        .into_iter()
        .map(
            |(assault_unit_id, target_id, target_kind)| NeighbourTargetViolation {
                assault_unit_id,
                target_id,
                target_kind,
            },
        )
        .collect();
}

fn tick_with_neighbour_observation(
    game: &mut HeadlessGame,
    ticks: u32,
    neighbour_targets: &BTreeMap<i32, String>,
    violations: &mut BTreeSet<(i32, i32, String)>,
) {
    for _ in 0..ticks {
        game.tick(1);
        observe_neighbour_targets(game, neighbour_targets, violations);
    }
}

#[derive(Debug, Default)]
struct EdgeScenarioRuntime {
    disconnect_owner: bool,
    leave_owner_offline: bool,
    disconnect_after_helper_engagement: bool,
    depart_helper_after_damage: bool,
    record_owner_target_loss: bool,
    neighbour_targets: BTreeMap<i32, String>,
}

fn observe_neighbour_targets(
    game: &mut HeadlessGame,
    neighbour_targets: &BTreeMap<i32, String>,
    violations: &mut BTreeSet<(i32, i32, String)>,
) {
    if neighbour_targets.is_empty() {
        return;
    }
    for unit in game.crisis_assault_units() {
        for target_id in [unit.visible_target, unit.target, unit.task_target]
            .into_iter()
            .flatten()
            .filter(|target| *target != NO_TARGET)
        {
            if let Some(target_kind) = neighbour_targets.get(&target_id) {
                violations.insert((unit.obj_id, target_id, target_kind.clone()));
            }
        }
    }
}

fn run_preparation_leg(
    spec: &RunSpec,
    comparison: PreparationComparison,
    leg: PreparationPairLeg,
    bot_scenario: CrisisBalanceScenario,
    cap_ticks: i32,
) -> Result<CompletedLeg, String> {
    let max_ticks = GAME_TICKS_PER_DAY
        .saturating_mul(2)
        .saturating_add(cap_ticks)
        .saturating_add(1_000);
    let mut game = HeadlessGame::new(max_ticks);
    game.restrict_to_preparation_pair_start_location()?;
    let player_id = game.spawn_hero(spec.hero_class, &spec.run_label);
    game.set_crisis_balance_sample_interval(Some(BALANCE_SAMPLE_INTERVAL_TICKS));
    let preparation_launch = game.prepare_checkpoint4_preparation_pair_launch(comparison, leg)?;
    let launch = capture_launch(&mut game)?;
    let mut bot = Bot::for_balance_scenario(player_id, bot_scenario);
    let mut edge_observations = EdgeObservations::default();
    drive_assault(
        &mut game,
        &mut bot,
        None,
        launch.assault_launch_tick,
        cap_ticks,
        leg == PreparationPairLeg::Treatment && comparison.includes_healing(),
        EdgeScenarioRuntime::default(),
        &mut edge_observations,
    );
    Ok(CompletedLeg {
        row: finish_row(spec, &mut game, launch, edge_observations, cap_ticks),
        launch: preparation_launch,
    })
}

fn run_preparation_leg_caught(
    spec: &RunSpec,
    comparison: PreparationComparison,
    leg: PreparationPairLeg,
    bot_scenario: CrisisBalanceScenario,
    cap_ticks: i32,
) -> Result<CompletedLeg, RunRow> {
    match catch_unwind(AssertUnwindSafe(|| {
        run_preparation_leg(spec, comparison, leg, bot_scenario, cap_ticks)
    })) {
        Ok(Ok(completed)) => Ok(completed),
        Ok(Err(error)) => Err(RunRow::failure(spec, "setup_failure", error)),
        Err(payload) => Err(RunRow::failure(spec, "panic", panic_message(payload))),
    }
}

fn run_villager_supported_leg(spec: &RunSpec, cap_ticks: i32) -> Result<RunRow, String> {
    let max_ticks = GAME_TICKS_PER_DAY
        .saturating_mul(2)
        .saturating_add(cap_ticks)
        .saturating_add(1_000);
    let mut game = HeadlessGame::new(max_ticks);
    game.restrict_to_preparation_pair_start_location()?;
    let player_id = game.spawn_hero(spec.hero_class, &spec.run_label);
    game.set_crisis_balance_sample_interval(Some(BALANCE_SAMPLE_INTERVAL_TICKS));
    let (preparation_launch, _villager_id) = game.prepare_villager_supported_launch()?;
    let launch = capture_launch(&mut game)?;
    let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::VillagerSupported);
    let mut observations = EdgeObservations::default();
    drive_assault(
        &mut game,
        &mut bot,
        None,
        launch.assault_launch_tick,
        cap_ticks,
        true,
        EdgeScenarioRuntime::default(),
        &mut observations,
    );
    let mut row = finish_row(spec, &mut game, launch, observations, cap_ticks);
    row.preparation_launch = Some(preparation_launch);
    let mut difference =
        PreparationDeclaredDifference::for_comparison(PreparationComparison::CombinedPreparation);
    difference.completed_stockade_delta = CHECKPOINT4_BLOCKING_STOCKADE_COUNT;
    difference.completed_wall_segment_delta = CHECKPOINT4_BLOCKING_STOCKADE_COUNT;
    row.declared_preparation_difference = Some(difference);
    Ok(row)
}

fn run_passive_leg(spec: &RunSpec, cap_ticks: i32) -> Result<RunRow, String> {
    let max_ticks = GAME_TICKS_PER_DAY
        .saturating_mul(2)
        .saturating_add(cap_ticks)
        .saturating_add(1_000);
    let mut game = HeadlessGame::new(max_ticks);
    game.restrict_to_preparation_pair_start_location()?;
    let player_id = game.spawn_hero(spec.hero_class, &spec.run_label);
    game.set_crisis_balance_sample_interval(Some(BALANCE_SAMPLE_INTERVAL_TICKS));
    let preparation_launch = game.prepare_preparation_pair_launch(
        PreparationComparison::CombinedPreparation,
        PreparationPairLeg::Control,
    )?;
    let launch = capture_launch(&mut game)?;
    let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::Passive);
    let mut observations = EdgeObservations::default();
    drive_assault(
        &mut game,
        &mut bot,
        None,
        launch.assault_launch_tick,
        cap_ticks,
        false,
        EdgeScenarioRuntime::default(),
        &mut observations,
    );
    let mut row = finish_row(spec, &mut game, launch, observations, cap_ticks);
    row.preparation_launch = Some(preparation_launch);
    Ok(row)
}

fn run_direct_cohort_caught(spec: &RunSpec, cap_ticks: i32, villager_supported: bool) -> RunRow {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if villager_supported {
            run_villager_supported_leg(spec, cap_ticks)
        } else {
            run_passive_leg(spec, cap_ticks)
        }
    }));
    match result {
        Ok(Ok(row)) => row,
        Ok(Err(error)) => RunRow::failure(spec, "setup_failure", error),
        Err(payload) => RunRow::failure(spec, "panic", panic_message(payload)),
    }
}

fn execute_pair(
    profile: &'static str,
    scenario_control: &'static str,
    scenario_treatment: &'static str,
    class: &'static str,
    repetition: u32,
    comparison: PreparationComparison,
    control_policy: CrisisBalanceScenario,
    treatment_policy: CrisisBalanceScenario,
    cap_ticks: i32,
) -> Vec<RunRow> {
    let comparison_label = comparison.label();
    let pair_label = format!(
        "{profile}-{comparison_label}-{}-r{repetition:03}",
        class.to_ascii_lowercase()
    );
    let control_spec = RunSpec {
        run_label: format!("{pair_label}-control"),
        workload_profile: profile,
        scenario: scenario_control,
        hero_class: class,
        repetition,
        preparation_comparison: Some(comparison_label),
        preparation_leg: Some("control"),
        setup_cohort: "direct_checkpoint4_assault_fixture",
    };
    let treatment_spec = RunSpec {
        run_label: format!("{pair_label}-treatment"),
        workload_profile: profile,
        scenario: scenario_treatment,
        hero_class: class,
        repetition,
        preparation_comparison: Some(comparison_label),
        preparation_leg: Some("treatment"),
        setup_cohort: "direct_checkpoint4_assault_fixture",
    };

    let run_control = || {
        run_preparation_leg_caught(
            &control_spec,
            comparison,
            PreparationPairLeg::Control,
            control_policy,
            cap_ticks,
        )
    };
    let run_treatment = || {
        run_preparation_leg_caught(
            &treatment_spec,
            comparison,
            PreparationPairLeg::Treatment,
            treatment_policy,
            cap_ticks,
        )
    };
    let (control, treatment, control_first) = if repetition % 2 == 0 {
        (run_control(), run_treatment(), true)
    } else {
        let treatment = run_treatment();
        let control = run_control();
        (control, treatment, false)
    };

    let validation = match (&control, &treatment) {
        (Ok(control), Ok(treatment)) => validate_checkpoint4_preparation_pair_launches(
            comparison,
            &control.launch,
            &treatment.launch,
        ),
        _ => Err("one or both legs failed before launch validation".to_string()),
    };
    let validation_error = validation.as_ref().err().cloned();
    let (mut control_row, control_launch) = match control {
        Ok(completed) => (completed.row, Some(completed.launch)),
        Err(row) => (row, None),
    };
    let (mut treatment_row, treatment_launch) = match treatment {
        Ok(completed) => (completed.row, Some(completed.launch)),
        Err(row) => (row, None),
    };
    let declared_difference = validation.unwrap_or_else(|_| {
        let mut difference = PreparationDeclaredDifference::for_comparison(comparison);
        if comparison.includes_wall() {
            difference.completed_stockade_delta = CHECKPOINT4_BLOCKING_STOCKADE_COUNT;
            difference.completed_wall_segment_delta = CHECKPOINT4_BLOCKING_STOCKADE_COUNT;
        }
        difference
    });
    control_row.preparation_launch = control_launch;
    treatment_row.preparation_launch = treatment_launch;
    control_row.declared_preparation_difference = Some(declared_difference.clone());
    treatment_row.declared_preparation_difference = Some(declared_difference);
    let mut rows = if control_first {
        vec![control_row, treatment_row]
    } else {
        vec![treatment_row, control_row]
    };
    for row in &mut rows {
        row.launch_validation_error = validation_error.clone();
    }
    rows
}

fn corrected_baseline_rows(repetitions: u32, cap_ticks: i32) -> Vec<RunRow> {
    let mut rows = Vec::new();
    for repetition in 0..repetitions {
        for class in HERO_CLASSES {
            rows.extend(execute_pair(
                "corrected_baseline",
                "basic_survival_no_villagers",
                "prepared_solo",
                class,
                repetition,
                PreparationComparison::CombinedPreparation,
                CrisisBalanceScenario::BasicSurvival,
                CrisisBalanceScenario::PreparedSolo,
                cap_ticks,
            ));
            let villager_spec = RunSpec {
                run_label: format!(
                    "corrected_baseline-villager_supported-{}-r{repetition:03}",
                    class.to_ascii_lowercase()
                ),
                workload_profile: "corrected_baseline",
                scenario: "villager_supported",
                hero_class: class,
                repetition,
                preparation_comparison: None,
                preparation_leg: None,
                setup_cohort: "direct_checkpoint4_villager_fixture",
            };
            rows.push(run_direct_cohort_caught(&villager_spec, cap_ticks, true));
        }
        let passive_class = HERO_CLASSES[repetition as usize % HERO_CLASSES.len()];
        let passive_spec = RunSpec {
            run_label: format!(
                "corrected_baseline-passive-{}-r{repetition:03}",
                passive_class.to_ascii_lowercase()
            ),
            workload_profile: "corrected_baseline",
            scenario: "passive_unprepared",
            hero_class: passive_class,
            repetition,
            preparation_comparison: None,
            preparation_leg: None,
            setup_cohort: "direct_checkpoint4_passive_fixture",
        };
        rows.push(run_direct_cohort_caught(&passive_spec, cap_ticks, false));
    }
    rows
}

fn focused_preparation_rows(repetitions: u32, cap_ticks: i32) -> Vec<RunRow> {
    let mut rows = Vec::new();
    for repetition in 0..repetitions {
        let class = HERO_CLASSES[repetition as usize % HERO_CLASSES.len()];
        for comparison in PreparationComparison::ALL {
            rows.extend(execute_pair(
                "focused_preparation",
                "preparation_control",
                comparison.label(),
                class,
                repetition,
                comparison,
                CrisisBalanceScenario::BasicSurvival,
                CrisisBalanceScenario::BasicSurvival,
                cap_ticks,
            ));
        }
    }
    rows
}

fn retained_start_position(values: &[i32]) -> [i32; 2] {
    [
        values.first().copied().unwrap_or_default(),
        values.get(1).copied().unwrap_or_default(),
    ]
}

fn restrict_adjacent_starts(game: &mut HeadlessGame) -> Result<Vec<RetainedStartLocation>, String> {
    let starts = &mut game
        .app_mut()
        .world_mut()
        .resource_mut::<StartLocations>()
        .0;
    starts.retain(|start| ADJACENT_START_LOCATIONS.contains(&start.name.as_str()));
    starts.sort_by(|left, right| left.name.cmp(&right.name));
    if starts.len() != ADJACENT_START_LOCATIONS.len()
        || !ADJACENT_START_LOCATIONS
            .iter()
            .all(|name| starts.iter().any(|start| start.name == *name))
    {
        return Err(format!(
            "missing audited adjacent start locations: {:?}",
            ADJACENT_START_LOCATIONS
        ));
    }
    Ok(starts
        .iter()
        .map(|start| RetainedStartLocation {
            name: start.name.clone(),
            hero_position: retained_start_position(&start.hero_pos),
            burrow_position: retained_start_position(&start.burrow_pos),
            monolith_position: retained_start_position(&start.monolith_pos),
        })
        .collect())
}

fn neighbour_observation(
    game: &mut HeadlessGame,
    helper_player_id: i32,
    launch: &LaunchObservation,
    retained: Vec<RetainedStartLocation>,
) -> (EdgeObservations, BTreeMap<i32, String>) {
    let helper_view = game.observe_for_player(helper_player_id);
    let neighbour_hero_id = helper_view.hero.map(|hero| hero.id);
    let neighbour_anchor = helper_view.home();
    let neighbour_villager_ids = helper_view
        .villagers
        .iter()
        .map(|villager| villager.id)
        .collect::<Vec<_>>();
    let neighbour_structure_ids = helper_view
        .structures
        .iter()
        .map(|structure| structure.id)
        .collect::<Vec<_>>();
    let neighbour_sanctuary = game.bound_monolith_for_player(helper_player_id);
    let mut target_ids = BTreeMap::new();
    if let Some(hero) = helper_view.hero {
        target_ids.insert(hero.id, "neighbour_hero".to_string());
    }
    for villager in &helper_view.villagers {
        target_ids.insert(villager.id, "neighbour_villager".to_string());
    }
    for structure in &helper_view.structures {
        target_ids.insert(structure.id, "neighbour_structure".to_string());
    }
    if let Some(monolith) = neighbour_sanctuary {
        target_ids.insert(monolith.id, "neighbour_sanctuary".to_string());
    }

    let mut footprint = helper_view
        .structures
        .iter()
        .map(|structure| position_array(structure.pos))
        .collect::<Vec<_>>();
    footprint.extend(
        helper_view
            .villagers
            .iter()
            .map(|villager| position_array(villager.pos)),
    );
    if let Some(hero) = helper_view.hero {
        footprint.push(position_array(hero.pos));
    }
    if let Some(monolith) = neighbour_sanctuary {
        footprint.push(position_array(monolith.pos));
    }
    footprint.sort_unstable();
    footprint.dedup();
    let footprint_set = footprint.iter().copied().collect::<BTreeSet<_>>();
    let spawn_overlaps = launch
        .units
        .iter()
        .filter(|unit| footprint_set.contains(&unit.position))
        .map(|unit| unit.obj_id)
        .collect();
    let mut foreign_structure_tiles = helper_view
        .structures
        .iter()
        .map(|structure| structure.pos)
        .collect::<Vec<_>>();
    if let Some(monolith) = neighbour_sanctuary {
        foreign_structure_tiles.push(monolith.pos);
    }
    let exclusion_distance =
        goblin_crisis_balance_config_snapshot().neighbouring_structure_exclusion_distance;
    let spawn_exclusion_violations = launch
        .units
        .iter()
        .filter(|unit| {
            foreign_structure_tiles
                .iter()
                .any(|position| hex_distance(unit.position, *position) < exclusion_distance)
        })
        .map(|unit| unit.obj_id)
        .collect();
    let distance = launch
        .settlement_anchor
        .zip(neighbour_anchor)
        .map(|(owner, neighbour)| hex_distance(owner, neighbour));
    (
        EdgeObservations {
            retained_start_locations: retained,
            neighbour_player_id: Some(helper_player_id),
            neighbour_hero_id,
            neighbour_villager_ids,
            neighbour_structure_ids,
            neighbour_sanctuary_id: neighbour_sanctuary.map(|monolith| monolith.id),
            neighbour_anchor: neighbour_anchor.map(position_array),
            owner_neighbour_anchor_distance: distance,
            neighbour_footprint_tiles: footprint,
            spawn_overlaps_neighbour_footprint: spawn_overlaps,
            spawn_neighbour_exclusion_violations: spawn_exclusion_violations,
            ..EdgeObservations::default()
        },
        target_ids,
    )
}

fn rendezvous_helper_before_launch(
    game: &mut HeadlessGame,
    helper_player_id: i32,
    owner_player_id: i32,
    owner_anchor: Position,
) -> Result<(Bot, [i32; 2], [i32; 2], i32), String> {
    let mut bot = Bot::for_helper_support(helper_player_id, owner_player_id, owner_anchor);
    let start_tick = game.game_tick();
    let start_position = game
        .observe_for_player(helper_player_id)
        .hero
        .map(|hero| position_array(hero.pos))
        .ok_or_else(|| "missing helper hero before prelaunch rendezvous".to_string())?;
    let mut move_events = 0_i32;

    loop {
        let view = game.observe_for_player(helper_player_id);
        let hero = view
            .hero
            .ok_or_else(|| "helper hero disappeared during prelaunch rendezvous".to_string())?;
        if hero.true_death || hero.dead {
            return Err("helper died during prelaunch rendezvous".to_string());
        }
        if hex_distance(position_array(hero.pos), owner_anchor) <= HELPER_RENDEZVOUS_DISTANCE {
            return Ok((bot, start_position, position_array(hero.pos), move_events));
        }
        if game.game_tick().saturating_sub(start_tick) >= HELPER_RENDEZVOUS_CAP_TICKS {
            return Err(format!(
                "helper did not reach the owner before the prelaunch rendezvous cap: start={start_position:?}, final={:?}",
                position_array(hero.pos)
            ));
        }
        if let Some(event) = bot.step(&view, game.map()) {
            if matches!(event, PlayerEvent::Move { .. }) {
                move_events = move_events.saturating_add(1);
            }
            game.inject(event);
        }
        bot.advance_phase(&view);
        game.tick(DECISION_TICKS);
    }
}

fn run_active_edge(
    spec: &RunSpec,
    scenario: EdgeScenario,
    cap_ticks: i32,
) -> Result<RunRow, String> {
    let max_ticks = GAME_TICKS_PER_DAY
        .saturating_mul(3)
        .saturating_add(cap_ticks)
        .saturating_add(2_000);
    let mut game = HeadlessGame::new(max_ticks);
    let needs_helper = matches!(
        scenario,
        EdgeScenario::HelperSupported
            | EdgeScenario::HelperDeparture
            | EdgeScenario::OfflineHelper
            | EdgeScenario::AdjacentSettlement
            | EdgeScenario::AdjacentTargetLoss
    );
    let retained = if needs_helper {
        restrict_adjacent_starts(&mut game)?
    } else {
        Vec::new()
    };
    let deferred_helper_start = if needs_helper {
        let starts = &mut game
            .app_mut()
            .world_mut()
            .resource_mut::<StartLocations>()
            .0;
        let owner_start = starts
            .iter()
            .find(|start| start.name == ADJACENT_OWNER_START_LOCATION)
            .cloned()
            .ok_or_else(|| "missing audited owner start location".to_string())?;
        let helper_start = starts
            .iter()
            .find(|start| start.name == ADJACENT_HELPER_START_LOCATION)
            .cloned()
            .ok_or_else(|| "missing audited helper start location".to_string())?;
        starts.clear();
        starts.push(owner_start);
        Some(helper_start)
    } else {
        None
    };
    let owner_id = game.spawn_hero(spec.hero_class, &spec.run_label);
    game.set_crisis_balance_sample_interval(Some(BALANCE_SAMPLE_INTERVAL_TICKS));
    if let Some(helper_start) = deferred_helper_start {
        let starts = &mut game
            .app_mut()
            .world_mut()
            .resource_mut::<StartLocations>()
            .0;
        starts.clear();
        starts.push(helper_start);
    }
    let helper_id = needs_helper
        .then(|| game.spawn_connected_scenario_helper(&format!("{}-helper", spec.run_label)));
    if let Some(helper_id) = helper_id {
        game.spawn_connected_scenario_villager(helper_id)?;
        game.prepare_established_scenario_helper(helper_id);
    }
    let (mut helper_bot, helper_rendezvous) = if let Some(helper_id) = helper_id {
        let owner_anchor = game
            .observe()
            .home()
            .ok_or_else(|| "missing owner support anchor before launch".to_string())?;
        let (bot, start, launch_position, move_events) =
            rendezvous_helper_before_launch(&mut game, helper_id, owner_id, owner_anchor)?;
        (
            Some((helper_id, bot)),
            Some((start, launch_position, move_events, owner_anchor)),
        )
    } else {
        (None, None)
    };
    game.prepare_active_assault_disconnect_scenario();
    let launch = capture_launch(&mut game)?;

    let (mut observations, neighbour_targets) = if let Some(helper_id) = helper_id {
        neighbour_observation(&mut game, helper_id, &launch, retained)
    } else {
        (EdgeObservations::default(), BTreeMap::new())
    };
    if let Some((start, helper_position, move_events, rendezvous_anchor)) = helper_rendezvous {
        if launch.settlement_anchor != Some(position_array(rendezvous_anchor)) {
            return Err(format!(
                "owner settlement anchor changed across helper rendezvous: rendezvous={:?}, launch={:?}",
                position_array(rendezvous_anchor),
                launch.settlement_anchor
            ));
        }
        observations.helper_prelaunch_start_position = Some(start);
        observations.helper_assault_launch_position = Some(helper_position);
        observations.helper_prelaunch_move_events = move_events;
        observations.helper_launch_distance_to_owner =
            Some(hex_distance(helper_position, rendezvous_anchor));
    }
    let owner_policy = if matches!(
        scenario,
        EdgeScenario::TrueDeathCleanupFreshRun | EdgeScenario::AdjacentTargetLoss
    ) {
        CrisisBalanceScenario::Passive
    } else {
        CrisisBalanceScenario::BasicSurvival
    };
    let mut owner_bot = Bot::for_balance_scenario(owner_id, owner_policy);
    let edge_runtime = EdgeScenarioRuntime {
        disconnect_owner: matches!(
            scenario,
            EdgeScenario::OrdinaryDisconnect
                | EdgeScenario::OfflineHelper
                | EdgeScenario::TrueDeathCleanupFreshRun
        ),
        leave_owner_offline: matches!(
            scenario,
            EdgeScenario::OfflineHelper | EdgeScenario::TrueDeathCleanupFreshRun
        ),
        disconnect_after_helper_engagement: scenario == EdgeScenario::OfflineHelper,
        depart_helper_after_damage: scenario == EdgeScenario::HelperDeparture,
        record_owner_target_loss: scenario == EdgeScenario::AdjacentTargetLoss,
        neighbour_targets,
    };
    drive_assault(
        &mut game,
        &mut owner_bot,
        if scenario == EdgeScenario::AdjacentTargetLoss {
            None
        } else {
            helper_bot
                .as_mut()
                .map(|(helper_id, bot)| (*helper_id, bot))
        },
        launch.assault_launch_tick,
        cap_ticks,
        false,
        edge_runtime,
        &mut observations,
    );
    if scenario == EdgeScenario::TrueDeathCleanupFreshRun {
        let mut row = finish_row(spec, &mut game, launch, observations.clone(), cap_ticks);
        if !row.hero_true_death {
            return Ok(row);
        }

        let resolutions_before_cleanup = game.crisis_telemetry().assaults_resolved;
        game.tick((10 * TICKS_PER_SEC + 5) as u32);
        observations.true_death_cleanup_removed_crisis = Some(game.settlement_crisis().is_none());
        observations.true_death_cleanup_removed_assault_units =
            Some(game.crisis_assault_units().is_empty());
        observations.true_death_cleanup_granted_resolution =
            Some(game.crisis_telemetry().assaults_resolved > resolutions_before_cleanup);

        game.spawn_hero(spec.hero_class, &format!("{}-fresh", spec.run_label));
        observations.fresh_run_crisis_reset = Some(
            game.settlement_crisis().is_some_and(|crisis| {
                crisis.phase == CrisisPhase::Dormant
                    && crisis.pressure == 0
                    && crisis.assault_id.is_none()
                    && crisis.assault_unit_ids.is_empty()
                    && crisis.assault_defeated_unit_ids.is_empty()
                    && crisis.assault_spawn_generation == 0
                    && !crisis.resolution_recorded
            }) && game.crisis_assault_units().is_empty(),
        );
        append_post_cleanup_observations(&mut row, observations);
        return Ok(row);
    }
    Ok(finish_row(spec, &mut game, launch, observations, cap_ticks))
}

fn append_post_cleanup_observations(row: &mut RunRow, mut observations: EdgeObservations) {
    // `finish_row` adds authoritative outcome-derived observations. Preserve
    // those while appending the post-True-Death cleanup probes collected later.
    observations.cross_player_target_violations_telemetry = row
        .edge_observations
        .cross_player_target_violations_telemetry;
    observations.helper_drove_combat_while_owner_offline |= row
        .edge_observations
        .helper_drove_combat_while_owner_offline;
    row.edge_observations = observations;
}

fn safe_logout_outcome_label(outcome: SafeLogoutCompletionOutcome) -> String {
    match outcome {
        SafeLogoutCompletionOutcome::Completed => "completed".to_string(),
        SafeLogoutCompletionOutcome::Rejected(reason) => format!("rejected:{reason:?}"),
        SafeLogoutCompletionOutcome::Cancelled(reason) => format!("cancelled:{reason:?}"),
        SafeLogoutCompletionOutcome::TimedOut => "timed_out".to_string(),
        SafeLogoutCompletionOutcome::Unexpected(presence) => {
            format!("unexpected:{presence:?}")
        }
    }
}

fn run_safe_logout_before_launch(spec: &RunSpec, cap_ticks: i32) -> Result<RunRow, String> {
    let max_ticks = PRELAUNCH_CAP_TICKS
        .saturating_add(cap_ticks)
        .saturating_add(GAME_TICKS_PER_DAY * 2);
    let mut game = HeadlessGame::new(max_ticks);
    let player_id = game.spawn_hero(spec.hero_class, &spec.run_label);
    game.set_crisis_balance_sample_interval(Some(BALANCE_SAMPLE_INTERVAL_TICKS));
    game.prepare_crisis_balance_progression_fixture(CrisisBalanceScenario::PreparedSolo);
    let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::PreparedSolo);
    let prelaunch_start = game.game_tick();
    while game.settlement_crisis().map(|crisis| crisis.phase) != Some(CrisisPhase::AssaultReady) {
        if game.game_tick().saturating_sub(prelaunch_start) >= PRELAUNCH_CAP_TICKS {
            return Err(
                "Safe Logout scenario did not reach AssaultReady before prelaunch cap".to_string(),
            );
        }
        let view = game.observe();
        if view.hero.is_none_or(|hero| hero.true_death) {
            return Err("hero reached True Death before Safe Logout prelaunch probe".to_string());
        }
        if let Some(event) = bot.step(&view, game.map()) {
            game.inject(event);
        }
        bot.advance_phase(&view);
        game.tick(DECISION_TICKS);
    }

    game.prepare_safe_logout_scenario();
    let completion = game.try_complete_valid_safe_logout_via_authenticated_ingress();
    let mut observations = EdgeObservations {
        safe_logout_completion: Some(safe_logout_outcome_label(completion)),
        ..EdgeObservations::default()
    };
    if completion != SafeLogoutCompletionOutcome::Completed {
        return Err(format!(
            "Safe Logout did not complete before launch: {completion:?}"
        ));
    }
    game.disconnect_after_completed_safe_logout();
    let protected_before = game
        .settlement_crisis()
        .ok_or_else(|| "missing crisis at start of protected interval".to_string())?;
    let protected_hero_before = game.protected_hero_snapshot();
    let protected_villagers_before = game.protected_villager_snapshots();
    let protected_structures_before = game.protected_structure_snapshots();
    let protected_work_before = game.protected_work_deadlines();
    let protected_crops_before = game.protected_crop_snapshots();
    let protected_intro_before = game.protected_intro_snapshot();
    let protected_resources_before = game.protected_stored_resource_quantity();
    game.advance_protected_world_ticks(SAFE_LOGOUT_PROTECTED_TICKS);
    let protected = game
        .settlement_crisis()
        .ok_or_else(|| "missing protected crisis".to_string())?;
    observations.safe_logout_prelaunch_state_frozen = Some(
        protected_before.phase == protected.phase
            && protected_before.pressure == protected.pressure
            && protected_before.online_active_ticks == protected.online_active_ticks
            && protected_before.phase_online_ticks == protected.phase_online_ticks,
    );
    observations.safe_logout_world_state_frozen = Some(
        game.protected_hero_snapshot() == protected_hero_before
            && game.protected_villager_snapshots() == protected_villagers_before
            && game.protected_structure_snapshots() == protected_structures_before
            && game.protected_work_deadlines() == protected_work_before
            && game.protected_crop_snapshots() == protected_crops_before
            && game.protected_intro_snapshot() == protected_intro_before
            && game.protected_stored_resource_quantity() == protected_resources_before,
    );
    observations.safe_logout_launched_while_protected = Some(
        protected.phase == CrisisPhase::AssaultActive || !game.crisis_assault_units().is_empty(),
    );
    game.reconnect_and_exit_protection();

    let resume_tick = game.game_tick();
    while game.settlement_crisis().map(|crisis| crisis.phase) != Some(CrisisPhase::AssaultActive) {
        if game.game_tick().saturating_sub(resume_tick) >= PRELAUNCH_CAP_TICKS {
            return Err("Safe Logout scenario did not launch after reconnect".to_string());
        }
        let view = game.observe();
        if view.hero.is_none_or(|hero| hero.true_death) {
            return Err("hero reached True Death before post-protection launch".to_string());
        }
        if let Some(event) = bot.step(&view, game.map()) {
            game.inject(event);
        }
        bot.advance_phase(&view);
        game.tick(DECISION_TICKS);
    }
    let launch = capture_launch(&mut game)?;
    game.request_safe_logout_via_authenticated_ingress();
    game.tick(1);
    observations.safe_logout_active_assault_rejected = Some(
        game.player_presence() == Some(siege_perilous::safe_logout::PlayerWorldPresence::Online)
            && game.safe_logout_rejection_reason()
                == Some(SafeLogoutRejectionReason::AssaultActive),
    );
    drive_assault(
        &mut game,
        &mut bot,
        None,
        launch.assault_launch_tick,
        cap_ticks,
        false,
        EdgeScenarioRuntime::default(),
        &mut observations,
    );
    Ok(finish_row(spec, &mut game, launch, observations, cap_ticks))
}

fn finalize_edge_invariants(scenario: EdgeScenario, mut row: RunRow) -> RunRow {
    if row.status == "setup_failure" || row.status == "panic" {
        return row;
    }

    let observations = &row.edge_observations;
    let mut failures = Vec::new();
    let mut require = |condition: bool, label: &str| {
        if !condition {
            failures.push(label.to_string());
        }
    };

    if let Some(metrics) = row.run_metrics.as_ref() {
        require(
            metrics.crisis_duplicate_assaults == 0,
            "duplicate_assault_detected",
        );
        require(
            metrics.personal_crisis_automatic_dusk_hordes == 0,
            "automatic_dusk_horde_detected",
        );
        require(
            metrics.crisis_invariants_ok,
            "crisis_runtime_invariant_failed",
        );
    } else {
        require(false, "missing_run_metrics");
    }
    require(
        observations.cross_player_target_violations_telemetry == Some(0),
        "cross_player_target_violation",
    );

    if observations.neighbour_player_id.is_some() {
        require(
            observations.neighbour_hero_id.is_some(),
            "neighbour_fixture_missing_hero",
        );
        require(
            !observations.neighbour_villager_ids.is_empty(),
            "neighbour_fixture_missing_villager",
        );
        require(
            !observations.neighbour_structure_ids.is_empty(),
            "neighbour_fixture_missing_structure",
        );
        require(
            observations.neighbour_sanctuary_id.is_some(),
            "neighbour_fixture_missing_bound_sanctuary",
        );
        require(
            observations.helper_prelaunch_start_position.is_some()
                && observations.helper_assault_launch_position.is_some(),
            "helper_rendezvous_positions_missing",
        );
        require(
            observations.helper_prelaunch_move_events > 0,
            "helper_rendezvous_used_no_production_move_events",
        );
        require(
            observations
                .helper_launch_distance_to_owner
                .is_some_and(|distance| distance <= HELPER_RENDEZVOUS_DISTANCE),
            "helper_did_not_rendezvous_before_launch",
        );
        require(
            observations.spawn_overlaps_neighbour_footprint.is_empty(),
            "spawn_overlapped_neighbour_footprint",
        );
        require(
            observations.spawn_neighbour_exclusion_violations.is_empty(),
            "spawn_violated_neighbour_exclusion",
        );
        require(
            observations.neighbour_target_violations.is_empty(),
            "assault_targeted_neighbour_asset",
        );
    }

    let helper_engaged = row.crisis_balance.as_ref().is_some_and(|telemetry| {
        telemetry.engagement.helper_attacks_accepted > 0
            && telemetry.engagement.helper_damage_dealt_to_assault > 0
    });
    match scenario {
        EdgeScenario::OrdinaryDisconnect => {
            require(
                observations.owner_disconnected_during_assault,
                "owner_was_not_disconnected",
            );
            require(
                observations.disconnect_preserved_active_phase == Some(true),
                "disconnect_did_not_preserve_assault_active",
            );
            require(
                observations.disconnect_preserved_timing == Some(true),
                "disconnect_changed_assault_timing",
            );
            require(
                observations.disconnect_preserved_unit_ids == Some(true),
                "disconnect_changed_assault_unit_ids",
            );
            require(
                observations.disconnect_did_not_heal_units == Some(true),
                "disconnect_healed_or_reset_assault_units",
            );
            require(
                observations.owner_reconnected_to_same_assault == Some(true),
                "reconnect_changed_assault_identity_or_generation",
            );
            require(
                observations.reconnect_did_not_reset_units == Some(true),
                "reconnect_healed_respawned_or_replaced_assault_units",
            );
        }
        EdgeScenario::SafeLogoutBeforeLaunch => {
            require(
                observations.safe_logout_completion.as_deref() == Some("completed"),
                "safe_logout_did_not_complete",
            );
            require(
                observations.safe_logout_prelaunch_state_frozen == Some(true),
                "safe_logout_crisis_state_advanced",
            );
            require(
                observations.safe_logout_world_state_frozen == Some(true),
                "safe_logout_world_state_advanced",
            );
            require(
                observations.safe_logout_launched_while_protected == Some(false),
                "assault_launched_while_safe_logout_protected",
            );
            require(
                observations.safe_logout_active_assault_rejected == Some(true),
                "safe_logout_was_not_rejected_during_active_assault",
            );
        }
        EdgeScenario::HelperSupported | EdgeScenario::AdjacentSettlement => {
            require(
                helper_engaged,
                "helper_did_not_deal_accepted_assault_damage",
            );
        }
        EdgeScenario::AdjacentTargetLoss => {
            require(
                observations.owner_target_loss_observed,
                "owner_target_loss_was_not_observed",
            );
        }
        EdgeScenario::HelperDeparture => {
            require(
                helper_engaged,
                "helper_did_not_deal_accepted_assault_damage",
            );
            require(
                observations.helper_departed_after_dealing_damage,
                "helper_did_not_depart_after_dealing_damage",
            );
            require(
                observations.helper_departure_preserved_assault == Some(true),
                "helper_departure_changed_assault_identity_or_generation",
            );
        }
        EdgeScenario::OfflineHelper => {
            require(
                observations.owner_disconnected_during_assault,
                "owner_was_not_disconnected",
            );
            require(
                observations.disconnect_preserved_active_phase == Some(true),
                "disconnect_did_not_preserve_assault_active",
            );
            require(
                observations.disconnect_preserved_timing == Some(true),
                "disconnect_changed_assault_timing",
            );
            require(
                observations.disconnect_preserved_unit_ids == Some(true),
                "disconnect_changed_assault_unit_ids",
            );
            require(
                observations.disconnect_did_not_heal_units == Some(true),
                "disconnect_healed_or_reset_assault_units",
            );
            require(
                observations.helper_drove_combat_while_owner_offline,
                "helper_did_not_drive_combat_while_owner_offline",
            );
            require(
                row.assault_resolved,
                "offline_helper_did_not_resolve_assault",
            );
            require(
                row.crisis_balance.as_ref().is_some_and(|telemetry| {
                    telemetry.assault_outcome.resolved_while_owner_offline
                }),
                "resolution_was_not_recorded_while_owner_offline",
            );
        }
        EdgeScenario::TrueDeathCleanupFreshRun => {
            require(
                observations.owner_disconnected_during_assault,
                "owner_was_not_disconnected_before_true_death",
            );
            require(
                observations.disconnect_preserved_active_phase == Some(true),
                "disconnect_did_not_preserve_assault_active",
            );
            require(
                observations.disconnect_preserved_timing == Some(true),
                "disconnect_changed_assault_timing",
            );
            require(
                observations.disconnect_preserved_unit_ids == Some(true),
                "disconnect_changed_assault_unit_ids",
            );
            require(row.hero_true_death, "scenario_did_not_reach_true_death");
            require(
                observations.true_death_cleanup_removed_crisis == Some(true),
                "true_death_cleanup_left_crisis_state",
            );
            require(
                observations.true_death_cleanup_removed_assault_units == Some(true),
                "true_death_cleanup_left_assault_units",
            );
            require(
                observations.true_death_cleanup_granted_resolution == Some(false),
                "true_death_cleanup_granted_resolution",
            );
            require(
                observations.fresh_run_crisis_reset == Some(true),
                "fresh_run_retained_stale_crisis_state",
            );
        }
    }

    row.invariant_failures = failures;
    if !row.invariant_failures.is_empty() {
        row.status = "invariant_failure".to_string();
    }
    row
}

fn run_edge_caught(spec: &RunSpec, scenario: EdgeScenario, cap_ticks: i32) -> RunRow {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if scenario == EdgeScenario::SafeLogoutBeforeLaunch {
            run_safe_logout_before_launch(spec, cap_ticks)
        } else {
            run_active_edge(spec, scenario, cap_ticks)
        }
    }));
    match result {
        Ok(Ok(row)) => finalize_edge_invariants(scenario, row),
        Ok(Err(error)) => RunRow::failure(spec, "setup_failure", error),
        Err(payload) => RunRow::failure(spec, "panic", panic_message(payload)),
    }
}

fn edge_case_rows(repetitions: u32, cap_ticks: i32) -> Vec<RunRow> {
    let mut rows = Vec::new();
    for repetition in 0..repetitions {
        for (scenario_index, scenario) in EdgeScenario::ALL.into_iter().enumerate() {
            let class = if scenario == EdgeScenario::TrueDeathCleanupFreshRun {
                "Warrior"
            } else {
                HERO_CLASSES[(repetition as usize + scenario_index) % HERO_CLASSES.len()]
            };
            let spec = RunSpec {
                run_label: format!(
                    "edge-{}-{}-r{repetition:03}",
                    scenario.label(),
                    class.to_ascii_lowercase()
                ),
                workload_profile: "edge_cases",
                scenario: scenario.label(),
                hero_class: class,
                repetition,
                preparation_comparison: None,
                preparation_leg: None,
                setup_cohort: "staged_checkpoint4_edge_fixture",
            };
            rows.push(run_edge_caught(&spec, scenario, cap_ticks));
        }
    }
    rows
}

fn build_report(config: &RunnerConfig) -> Report {
    let provenance = Provenance::capture(&config.build_profile_label);
    let mut rows = Vec::new();
    if config.profile.includes_baseline() {
        rows.extend(corrected_baseline_rows(
            config.repetitions,
            config.assault_cap_ticks,
        ));
    }
    if config.profile.includes_preparation() {
        rows.extend(focused_preparation_rows(
            config.repetitions,
            config.assault_cap_ticks,
        ));
    }
    if config.profile.includes_edges() {
        rows.extend(edge_case_rows(config.repetitions, config.assault_cap_ticks));
    }
    let aggregate = Aggregate::from_rows(&rows);
    Report {
        schema_version: SCHEMA_VERSION,
        workload_profile: config.profile.label().to_string(),
        repetitions: config.repetitions,
        assault_relative_cap_ticks: config.assault_cap_ticks,
        prelaunch_cap_ticks: PRELAUNCH_CAP_TICKS,
        methodology: Methodology {
            random_stream_replayed: false,
            full_ecs_state_matched: false,
            entropy_source: "production thread_rng; no global replay control",
            run_label_semantics: "labels identify workload rows and are not random seeds",
            stopping_rule: "AssaultActive -> Resolved, owner TrueDeath/missing hero, or assault-relative tick cap; ordinary death is non-terminal",
            pair_order: "counterbalanced by repetition (even control-first, odd treatment-first); each leg keeps its actual production launch geometry; outcomes are repeated descriptive evidence, not causal or deterministic pairs",
            engagement_sampling: "one-tick opt-in sampling; exact combat events remain separately recorded at their production boundaries",
            periodic_pressure_output: "omitted from row payloads after metrics collection; transition and final pressure snapshots remain retained",
            output_policy: "explicit path opened with create_new; no artifact is overwritten",
        },
        provenance,
        rows,
        aggregate,
        limitations: vec![
            "The public preparation fixture matches declared observed launch fields, not complete ECS state or random streams.",
            "The focused preparation matrix rotates classes across repetitions; use a multiple of three for equal class counts.",
            "The Safe Logout edge includes a bounded prelaunch phase; failure to launch is retained as setup_failure.",
            "Adjacent settlement setup constrains ordinary production NewPlayer starts to audited startpos2 and startpos4; it does not relocate spawned entities after setup.",
            "This runner records panics, invalid fingerprints, setup failures, unresolved rows, and timeouts instead of filtering them.",
        ],
    }
}

fn write_report(path: &Path, report: &Report) -> Result<(), String> {
    reject_protected_artifact_name(path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            format!(
                "refusing to overwrite or unable to create {}: {error}",
                path.display()
            )
        })?;
    serde_json::to_writer_pretty(&mut file, report)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    file.write_all(b"\n")
        .map_err(|error| format!("failed to finish {}: {error}", path.display()))
}

fn main() {
    let config = match parse_args() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let report = build_report(&config);
    if let Err(error) = write_report(&config.output, &report) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    println!(
        "wrote {} Checkpoint 4 rows to {} (resolved={}, true_deaths={}, timeouts={}, setup_failures={}, panics={})",
        report.aggregate.rows,
        config.output.display(),
        report.aggregate.resolved,
        report.aggregate.true_deaths,
        report.aggregate.timeouts,
        report.aggregate.setup_failures,
        report.aggregate.panics,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_requires_explicit_new_checkpoint4_output_and_parses_profiles() {
        let config = parse_args_from([
            "--output",
            "goblin_crisis_checkpoint4_matrix.json",
            "--profile",
            "corrected-baseline",
            "--repetitions",
            "10",
            "--assault-cap-ticks",
            "9000",
            "--build-profile",
            "release",
        ])
        .expect("valid Checkpoint 4 CLI");
        assert_eq!(config.profile, WorkloadProfile::CorrectedBaseline);
        assert_eq!(config.repetitions, 10);
        assert_eq!(config.assault_cap_ticks, 9000);
        assert_eq!(config.build_profile_label, "release");
    }

    #[test]
    fn cli_rejects_checkpoint1_to_3_artifact_names() {
        for checkpoint in ["checkpoint1", "checkpoint2", "checkpoint3"] {
            let error = parse_args_from([
                "--output".to_string(),
                format!("goblin_crisis_{checkpoint}_report.json"),
            ])
            .expect_err("protected artifact name must be rejected");
            assert!(error.contains("Checkpoint 1-3"));
        }
    }

    #[test]
    fn cli_refuses_an_existing_output_before_running_workload() {
        let path = std::env::temp_dir().join(format!(
            "goblin_crisis_checkpoint4_existing_{}.json",
            std::process::id()
        ));
        std::fs::write(&path, b"earlier artifact").expect("create existing artifact");
        let error = parse_args_from(["--output".to_string(), path.display().to_string()])
            .expect_err("existing output must be rejected");
        assert!(error.contains("refusing to overwrite existing output"));
        assert_eq!(
            std::fs::read(&path).expect("read existing artifact"),
            b"earlier artifact"
        );
        std::fs::remove_file(path).expect("remove test artifact");
    }

    #[test]
    fn audited_adjacent_start_anchors_are_nine_hexes_apart() {
        assert_eq!(hex_distance([5, 21], Position { x: 5, y: 30 }), 9);
    }

    #[test]
    fn methodology_serialization_disclaims_replay_and_full_ecs_matching() {
        let methodology = Methodology {
            random_stream_replayed: false,
            full_ecs_state_matched: false,
            entropy_source: "production thread_rng; no global replay control",
            run_label_semantics: "labels are not seeds",
            stopping_rule: "resolved, TrueDeath, or cap",
            pair_order: "control then treatment",
            engagement_sampling: "one tick",
            periodic_pressure_output: "omitted after collection",
            output_policy: "create_new",
        };
        let value = serde_json::to_value(methodology).expect("serialize methodology");
        assert_eq!(value["random_stream_replayed"], false);
        assert_eq!(value["full_ecs_state_matched"], false);
        assert!(value["run_label_semantics"]
            .as_str()
            .is_some_and(|label| label.contains("not seeds")));
    }

    #[test]
    fn panic_payload_and_known_category_are_preserved() {
        let payload = "Cannot find item template: \"Windstride Stag\"".to_string();
        let panic = panic_record(payload.clone());
        assert_eq!(panic.category, "missing_windstride_stag_template");
        assert_eq!(panic.payload, payload);
    }

    #[test]
    fn post_cleanup_observations_preserve_authoritative_outcome_fields() {
        let spec = RunSpec {
            run_label: "cleanup-merge".to_string(),
            workload_profile: "edge_cases",
            scenario: "true_death_cleanup_fresh_run",
            hero_class: "Warrior",
            repetition: 0,
            preparation_comparison: None,
            preparation_leg: None,
            setup_cohort: "test",
        };
        let mut row = RunRow::failure(&spec, "unresolved", "test row".to_string());
        row.edge_observations
            .cross_player_target_violations_telemetry = Some(0);
        row.edge_observations
            .helper_drove_combat_while_owner_offline = true;
        let observations = EdgeObservations {
            true_death_cleanup_removed_crisis: Some(true),
            true_death_cleanup_removed_assault_units: Some(true),
            true_death_cleanup_granted_resolution: Some(false),
            fresh_run_crisis_reset: Some(true),
            ..EdgeObservations::default()
        };

        append_post_cleanup_observations(&mut row, observations);

        assert_eq!(
            row.edge_observations
                .cross_player_target_violations_telemetry,
            Some(0)
        );
        assert!(
            row.edge_observations
                .helper_drove_combat_while_owner_offline
        );
        assert_eq!(
            row.edge_observations
                .true_death_cleanup_removed_assault_units,
            Some(true)
        );
        assert_eq!(
            row.edge_observations.true_death_cleanup_granted_resolution,
            Some(false)
        );
    }

    #[test]
    fn helper_rendezvous_uses_normal_prelaunch_movement() {
        let mut game = HeadlessGame::new(4_000);
        restrict_adjacent_starts(&mut game).expect("audited adjacent starts");
        let helper_start = {
            let starts = &mut game
                .app_mut()
                .world_mut()
                .resource_mut::<StartLocations>()
                .0;
            let owner_start = starts
                .iter()
                .find(|start| start.name == ADJACENT_OWNER_START_LOCATION)
                .cloned()
                .expect("owner start");
            let helper_start = starts
                .iter()
                .find(|start| start.name == ADJACENT_HELPER_START_LOCATION)
                .cloned()
                .expect("helper start");
            starts.clear();
            starts.push(owner_start);
            helper_start
        };
        let owner_id = game.spawn_hero("Warrior", "RendezvousOwner");
        {
            let starts = &mut game
                .app_mut()
                .world_mut()
                .resource_mut::<StartLocations>()
                .0;
            starts.clear();
            starts.push(helper_start);
        }
        let helper_id = game.spawn_connected_scenario_helper("RendezvousHelper");
        game.prepare_established_scenario_helper(helper_id);
        let owner_anchor = game.observe().home().expect("owner anchor");

        let (_bot, start, launch_position, move_events) =
            rendezvous_helper_before_launch(&mut game, helper_id, owner_id, owner_anchor)
                .expect("prelaunch helper rendezvous");

        assert!(hex_distance(start, owner_anchor) > HELPER_RENDEZVOUS_DISTANCE);
        assert!(hex_distance(launch_position, owner_anchor) <= HELPER_RENDEZVOUS_DISTANCE);
        assert!(move_events > 0);
    }

    #[test]
    fn launch_capture_preserves_the_exact_simultaneous_three_unit_composition() {
        let mut game = HeadlessGame::new(30_000);
        game.restrict_to_preparation_pair_start_location()
            .expect("single preparation start");
        game.spawn_hero("Ranger", "StaggerCaptureBot");
        game.prepare_checkpoint4_preparation_pair_launch(
            PreparationComparison::CombinedPreparation,
            PreparationPairLeg::Treatment,
        )
        .expect("staged production launch");

        let launch = capture_launch(&mut game).expect("valid staged production launch");
        assert_eq!(launch.unit_count, 3);
        let mut templates = launch
            .units
            .iter()
            .map(|unit| unit.template.as_str())
            .collect::<Vec<_>>();
        templates.sort();
        assert_eq!(
            templates,
            vec!["Goblin Pillager", "Wolf Rider", "Wolf Rider"]
        );
        assert!(launch
            .units
            .iter()
            .all(|unit| unit.initial_target.is_none()));
    }
}
