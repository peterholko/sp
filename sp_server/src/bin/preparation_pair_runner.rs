use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

use serde::Serialize;
use siege_perilous::constants::GAME_TICKS_PER_DAY;
use siege_perilous::crisis_balance::CrisisBalanceScenario;
use siege_perilous::game::CrisisPhase;
use siege_perilous::headless::{
    validate_preparation_pair_launches, HeadlessGame, PreparationCommonLaunchFingerprint,
    PreparationComparison, PreparationDeclaredDifference, PreparationFixtureState,
    PreparationPairLaunch, PreparationPairLeg,
};
use siege_perilous::headless_bot::Bot;

const SCHEMA_VERSION: &str = "checkpoint3_preparation_pair_v1";
const DEFAULT_PAIRS_PER_COMPARISON: u32 = 5;
const DEFAULT_ASSAULT_RELATIVE_CAP_TICKS: i32 = 15_000;
const DECISION_TICKS: u32 = 8;

#[derive(Debug, Clone)]
struct RunnerConfig {
    pairs_per_comparison: u32,
    assault_relative_cap_ticks: i32,
    comparisons: Vec<PreparationComparison>,
    output: Option<String>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            pairs_per_comparison: DEFAULT_PAIRS_PER_COMPARISON,
            assault_relative_cap_ticks: DEFAULT_ASSAULT_RELATIVE_CAP_TICKS,
            comparisons: PreparationComparison::ALL.to_vec(),
            output: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct Methodology {
    design: &'static str,
    matched_observed_launch_fields: bool,
    full_ecs_state_matched: bool,
    launch_geometry_normalized: bool,
    random_stream_replayed: bool,
    requested_seed_label_semantics: &'static str,
    combat_policy: &'static str,
    treatment_validation: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct LegResult {
    requested_seed_label: String,
    random_stream_replayed: bool,
    comparison: String,
    leg: String,
    hero_class: String,
    status: String,
    failure: Option<String>,
    completed_action: bool,
    completed_preparation_actions: Vec<String>,
    combat_actions_completed: Vec<String>,
    resolution: bool,
    survival: bool,
    hero_damage_taken: i32,
    hero_deaths: i32,
    villager_losses: i32,
    total_villager_damage: i32,
    structures_at_launch: i32,
    structures_damaged: i32,
    structures_destroyed: i32,
    total_structure_damage: i32,
    walls_at_launch: i32,
    walls_destroyed: i32,
    assault_duration_ticks: Option<i32>,
    observed_assault_ticks: i32,
    assault_units_defeated: i32,
    assault_units_remaining: i32,
    launch_fixture: Option<PreparationFixtureState>,
}

impl LegResult {
    fn failure(
        requested_seed_label: &str,
        comparison: PreparationComparison,
        leg: PreparationPairLeg,
        hero_class: &str,
        status: &str,
        failure: String,
    ) -> Self {
        Self {
            requested_seed_label: requested_seed_label.to_string(),
            random_stream_replayed: false,
            comparison: comparison.label().to_string(),
            leg: leg.label().to_string(),
            hero_class: hero_class.to_string(),
            status: status.to_string(),
            failure: Some(failure),
            completed_action: false,
            completed_preparation_actions: Vec::new(),
            combat_actions_completed: Vec::new(),
            resolution: false,
            survival: false,
            hero_damage_taken: 0,
            hero_deaths: 0,
            villager_losses: 0,
            total_villager_damage: 0,
            structures_at_launch: 0,
            structures_damaged: 0,
            structures_destroyed: 0,
            total_structure_damage: 0,
            walls_at_launch: 0,
            walls_destroyed: 0,
            assault_duration_ticks: None,
            observed_assault_ticks: 0,
            assault_units_defeated: 0,
            assault_units_remaining: 0,
            launch_fixture: None,
        }
    }

    fn is_quantitative(&self) -> bool {
        !matches!(self.status.as_str(), "setup_failure" | "panic")
    }
}

#[derive(Debug)]
struct LegExecution {
    result: LegResult,
    launch: PreparationPairLaunch,
}

#[derive(Debug, Clone, Default, Serialize)]
struct PairDeltas {
    resolution: Option<i32>,
    survival: Option<i32>,
    hero_damage_taken: Option<i32>,
    hero_deaths: Option<i32>,
    villager_losses: Option<i32>,
    total_villager_damage: Option<i32>,
    structures_damaged: Option<i32>,
    structures_destroyed: Option<i32>,
    total_structure_damage: Option<i32>,
    walls_destroyed: Option<i32>,
    observed_assault_ticks: Option<i32>,
    assault_units_defeated: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
struct PairResult {
    pair_index: u32,
    requested_seed_label: String,
    hero_class: String,
    comparison: String,
    random_stream_replayed: bool,
    declared_difference: PreparationDeclaredDifference,
    common_launch_fingerprint: Option<PreparationCommonLaunchFingerprint>,
    launch_validation_error: Option<String>,
    control: LegResult,
    treatment: LegResult,
    deltas_treatment_minus_control: PairDeltas,
    overall_classification: String,
}

#[derive(Debug, Clone, Default, Serialize)]
struct DeltaAggregate {
    direction: String,
    samples: usize,
    mean: Option<f64>,
    median: Option<f64>,
    improved: Option<usize>,
    unchanged: Option<usize>,
    worsened: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
enum MetricDirection {
    HigherIsBetter,
    LowerIsBetter,
    DescriptiveOnly,
}

impl MetricDirection {
    const fn label(self) -> &'static str {
        match self {
            Self::HigherIsBetter => "higher_is_better",
            Self::LowerIsBetter => "lower_is_better",
            Self::DescriptiveOnly => "descriptive_only",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ComparisonAggregate {
    comparison: String,
    pairs_requested: usize,
    pairs_with_valid_launch_fingerprint: usize,
    pairs_with_quantitative_deltas: usize,
    setup_failures: usize,
    panics: usize,
    timeouts_or_unresolved: usize,
    metrics: BTreeMap<String, DeltaAggregate>,
    overall_improved: usize,
    overall_unchanged: usize,
    overall_worsened: usize,
    overall_unclassified: usize,
}

#[derive(Debug, Serialize)]
struct PreparationPairReport {
    schema_version: &'static str,
    methodology: Methodology,
    pairs_per_comparison: u32,
    assault_relative_cap_ticks: i32,
    pairs: Vec<PairResult>,
    aggregates: Vec<ComparisonAggregate>,
}

fn parse_args() -> Result<RunnerConfig, String> {
    let mut config = RunnerConfig::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pairs" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--pairs requires a positive integer".to_string())?;
                config.pairs_per_comparison = value
                    .parse::<u32>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| format!("invalid --pairs value: {value}"))?;
            }
            "--assault-cap-ticks" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--assault-cap-ticks requires a positive integer".to_string())?;
                config.assault_relative_cap_ticks = value
                    .parse::<i32>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| format!("invalid --assault-cap-ticks value: {value}"))?;
            }
            "--comparison" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--comparison requires a label".to_string())?;
                let comparison = PreparationComparison::from_label(&value)
                    .ok_or_else(|| format!("unknown comparison: {value}"))?;
                config.comparisons = vec![comparison];
            }
            "--output" => {
                config.output = Some(
                    args.next()
                        .ok_or_else(|| "--output requires a path".to_string())?,
                );
            }
            "--help" | "-h" => {
                return Err(format!(
                    "usage: preparation_pair_runner [--pairs N] [--assault-cap-ticks N] [--comparison {}] [--output PATH]",
                    PreparationComparison::ALL
                        .iter()
                        .map(|comparison| comparison.label())
                        .collect::<Vec<_>>()
                        .join("|")
                ));
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    Ok(config)
}

fn hero_class(pair_index: u32) -> &'static str {
    match pair_index % 3 {
        0 => "Warrior",
        1 => "Ranger",
        _ => "Mage",
    }
}

fn should_stop_assault_observation(
    hero_present: bool,
    _hero_dead: bool,
    hero_true_death: bool,
) -> bool {
    // Ordinary death is not terminal: production can resurrect the hero at the
    // bound Monolith, and the personal assault continues through that cycle.
    !hero_present || hero_true_death
}

fn run_leg(
    requested_seed_label: &str,
    comparison: PreparationComparison,
    leg: PreparationPairLeg,
    class: &str,
    cap_ticks: i32,
    control_launch: Option<&PreparationPairLaunch>,
) -> Result<LegExecution, String> {
    let max_ticks = GAME_TICKS_PER_DAY
        .saturating_mul(2)
        .saturating_add(cap_ticks)
        .saturating_add(1_000);
    let mut game = HeadlessGame::new(max_ticks);
    game.restrict_to_preparation_pair_start_location()?;
    let player_id = game.spawn_hero(
        class,
        &format!("PrepPair-{requested_seed_label}-{}", leg.label()),
    );
    game.set_crisis_balance_sample_interval(Some(1));
    let launch = game.prepare_preparation_pair_launch(
        comparison,
        leg,
        control_launch.map(|launch| launch.geometry.as_slice()),
    )?;
    let assault_started_tick = launch.common_fingerprint.world_tick;
    let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
    let mut bandage_use_requested = false;
    let mut bandage_used = false;

    loop {
        let phase = game.settlement_crisis().map(|crisis| crisis.phase);
        if phase == Some(CrisisPhase::Resolved) {
            break;
        }
        let view = game.observe();
        let (hero_present, hero_dead, hero_true_death) = view
            .hero
            .map(|hero| (true, hero.dead, hero.true_death))
            .unwrap_or((false, false, false));
        if should_stop_assault_observation(hero_present, hero_dead, hero_true_death) {
            break;
        }
        let elapsed = game.game_tick().saturating_sub(assault_started_tick).max(0);
        if elapsed >= cap_ticks {
            break;
        }

        let mut ordinary_healing_action = false;
        if leg == PreparationPairLeg::Treatment
            && comparison.includes_healing()
            && !bandage_use_requested
        {
            if let Some(event) = game.preparation_bandage_use_event() {
                game.inject(event);
                bandage_use_requested = true;
                ordinary_healing_action = true;
            }
        }
        if !ordinary_healing_action {
            if let Some(event) = bot.step(&view, game.map()) {
                game.inject(event);
            }
        }
        bot.advance_phase(&view);
        let remaining = cap_ticks.saturating_sub(elapsed);
        game.tick(DECISION_TICKS.min(remaining.max(1) as u32));
        if bandage_use_requested
            && !game
                .observe()
                .inventory
                .iter()
                .any(|item| item.name == "Crude Bandage" && item.quantity > 0)
        {
            bandage_used = true;
        }
    }

    let final_view = game.observe();
    let survival = final_view
        .hero
        .is_some_and(|hero| !hero.dead && !hero.true_death);
    let phase = game.settlement_crisis().map(|crisis| crisis.phase);
    let resolution = phase == Some(CrisisPhase::Resolved);
    let observed_assault_ticks = game.game_tick().saturating_sub(assault_started_tick).max(0);
    let timed_out = !resolution && survival && observed_assault_ticks >= cap_ticks;
    let telemetry = game.crisis_balance_telemetry();
    let outcome = telemetry.assault_outcome;
    let mut completed_preparation_actions = vec!["hide_wraps_carried_at_launch".to_string()];
    if leg == PreparationPairLeg::Treatment && comparison.includes_wall() {
        completed_preparation_actions.push("completed_stockade_present_at_preparing".to_string());
    }
    if leg == PreparationPairLeg::Treatment && comparison.includes_equipment() {
        completed_preparation_actions
            .push("hide_wraps_equipped_via_player_event_at_preparing".to_string());
    }
    if leg == PreparationPairLeg::Treatment && comparison.includes_healing() {
        completed_preparation_actions.push("crude_bandage_carried_at_launch".to_string());
    }
    let combat_actions_completed = if bandage_used {
        vec!["crude_bandage_used_via_player_event_when_wounded".to_string()]
    } else {
        Vec::new()
    };
    let completed_action = match leg {
        PreparationPairLeg::Control => true,
        PreparationPairLeg::Treatment => {
            (!comparison.includes_wall() || launch.fixture.completed_stockades > 0)
                && (!comparison.includes_equipment() || launch.fixture.hide_wraps_equipped)
                && (!comparison.includes_healing() || launch.fixture.crude_bandages == 1)
        }
    };
    let status = if resolution {
        "resolved"
    } else if !survival {
        "hero_dead"
    } else if timed_out {
        "timeout_unresolved"
    } else {
        "unresolved"
    };
    Ok(LegExecution {
        result: LegResult {
            requested_seed_label: requested_seed_label.to_string(),
            random_stream_replayed: false,
            comparison: comparison.label().to_string(),
            leg: leg.label().to_string(),
            hero_class: class.to_string(),
            status: status.to_string(),
            failure: None,
            completed_action,
            completed_preparation_actions,
            combat_actions_completed,
            resolution,
            survival,
            hero_damage_taken: outcome.hero_damage_taken,
            hero_deaths: outcome.hero_deaths_during_assault,
            villager_losses: outcome.villagers_killed,
            total_villager_damage: outcome.total_villager_damage,
            structures_at_launch: outcome.structures_at_launch,
            structures_damaged: outcome.structures_damaged,
            structures_destroyed: outcome.structures_destroyed,
            total_structure_damage: outcome.total_structure_damage,
            walls_at_launch: outcome.wall_segments_at_launch,
            walls_destroyed: outcome.wall_segments_destroyed,
            assault_duration_ticks: outcome.assault_duration_ticks,
            observed_assault_ticks,
            assault_units_defeated: outcome.assault_units_defeated,
            assault_units_remaining: outcome.assault_units_remaining,
            launch_fixture: Some(launch.fixture.clone()),
        },
        launch,
    })
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

fn run_leg_caught(
    requested_seed_label: &str,
    comparison: PreparationComparison,
    leg: PreparationPairLeg,
    class: &str,
    cap_ticks: i32,
    control_launch: Option<&PreparationPairLaunch>,
) -> Result<LegExecution, Box<LegResult>> {
    match catch_unwind(AssertUnwindSafe(|| {
        run_leg(
            requested_seed_label,
            comparison,
            leg,
            class,
            cap_ticks,
            control_launch,
        )
    })) {
        Ok(Ok(execution)) => Ok(execution),
        Ok(Err(error)) => Err(Box::new(LegResult::failure(
            requested_seed_label,
            comparison,
            leg,
            class,
            "setup_failure",
            error,
        ))),
        Err(payload) => Err(Box::new(LegResult::failure(
            requested_seed_label,
            comparison,
            leg,
            class,
            "panic",
            panic_message(payload),
        ))),
    }
}

fn deltas(control: &LegResult, treatment: &LegResult, valid_launch: bool) -> PairDeltas {
    if !valid_launch || !control.is_quantitative() || !treatment.is_quantitative() {
        return PairDeltas::default();
    }
    PairDeltas {
        resolution: Some(i32::from(treatment.resolution) - i32::from(control.resolution)),
        survival: Some(i32::from(treatment.survival) - i32::from(control.survival)),
        hero_damage_taken: Some(treatment.hero_damage_taken - control.hero_damage_taken),
        hero_deaths: Some(treatment.hero_deaths - control.hero_deaths),
        villager_losses: Some(treatment.villager_losses - control.villager_losses),
        total_villager_damage: Some(
            treatment.total_villager_damage - control.total_villager_damage,
        ),
        structures_damaged: Some(treatment.structures_damaged - control.structures_damaged),
        structures_destroyed: Some(treatment.structures_destroyed - control.structures_destroyed),
        total_structure_damage: Some(
            treatment.total_structure_damage - control.total_structure_damage,
        ),
        walls_destroyed: Some(treatment.walls_destroyed - control.walls_destroyed),
        observed_assault_ticks: Some(
            treatment.observed_assault_ticks - control.observed_assault_ticks,
        ),
        assault_units_defeated: Some(
            treatment.assault_units_defeated - control.assault_units_defeated,
        ),
    }
}

fn classify_pair(comparison: PreparationComparison, deltas: &PairDeltas) -> &'static str {
    // Structure and wall damage are deliberately excluded from the overall
    // direction. Comparisons that add a Stockade can record more structure
    // damage precisely because that added wall absorbed attacks; without a
    // core-exposure metric, treating that delta as automatically worse would
    // reverse the intended interpretation.
    let comparisons = [
        deltas.survival,
        deltas.resolution,
        deltas.hero_deaths.map(|value| -value),
        deltas.villager_losses.map(|value| -value),
        // A consumed heal adds an HP buffer, so treatment may correctly absorb
        // more cumulative damage before the same terminal outcome. Interpret
        // damage direction only when healing is not the declared difference.
        (!comparison.includes_healing())
            .then_some(deltas.hero_damage_taken.map(|value| -value))
            .flatten(),
        deltas.assault_units_defeated,
    ];
    if comparisons.iter().all(Option::is_none) {
        return "unclassified";
    }
    let mut improved = false;
    let mut worsened = false;
    for comparison in comparisons.into_iter().flatten() {
        improved |= comparison > 0;
        worsened |= comparison < 0;
    }
    // A benefit cannot hide a simultaneous guardrail regression. Mixed
    // directional outcomes conservatively fail the pair instead of allowing
    // metric order to decide the label.
    if worsened {
        "worsened"
    } else if improved {
        "improved"
    } else {
        "unchanged"
    }
}

fn run_pair(pair_index: u32, comparison: PreparationComparison, cap_ticks: i32) -> PairResult {
    let requested_seed_label = format!("requested-pair-{pair_index:04}");
    let class = hero_class(pair_index);
    let declared_difference = PreparationDeclaredDifference::for_comparison(comparison);

    let control_execution = run_leg_caught(
        &requested_seed_label,
        comparison,
        PreparationPairLeg::Control,
        class,
        cap_ticks,
        None,
    );
    let treatment_execution = match &control_execution {
        Ok(control) => run_leg_caught(
            &requested_seed_label,
            comparison,
            PreparationPairLeg::Treatment,
            class,
            cap_ticks,
            Some(&control.launch),
        ),
        Err(_) => Err(Box::new(LegResult::failure(
            &requested_seed_label,
            comparison,
            PreparationPairLeg::Treatment,
            class,
            "setup_failure",
            "control leg did not provide launch geometry".to_string(),
        ))),
    };

    let (common_launch_fingerprint, launch_validation_error) =
        match (&control_execution, &treatment_execution) {
            (Ok(control), Ok(treatment)) => match validate_preparation_pair_launches(
                comparison,
                &control.launch,
                &treatment.launch,
            ) {
                Ok(_) => (Some(control.launch.common_fingerprint.clone()), None),
                Err(error) => (None, Some(error)),
            },
            _ => (
                None,
                Some("one or both pair legs failed before validation".to_string()),
            ),
        };
    let control = match control_execution {
        Ok(execution) => execution.result,
        Err(result) => *result,
    };
    let treatment = match treatment_execution {
        Ok(execution) => execution.result,
        Err(result) => *result,
    };
    let pair_deltas = deltas(&control, &treatment, launch_validation_error.is_none());
    let overall_classification = classify_pair(comparison, &pair_deltas).to_string();
    PairResult {
        pair_index,
        requested_seed_label,
        hero_class: class.to_string(),
        comparison: comparison.label().to_string(),
        random_stream_replayed: false,
        declared_difference,
        common_launch_fingerprint,
        launch_validation_error,
        control,
        treatment,
        deltas_treatment_minus_control: pair_deltas,
        overall_classification,
    }
}

fn delta_aggregate(
    values: impl Iterator<Item = Option<i32>>,
    direction: MetricDirection,
) -> DeltaAggregate {
    let mut values = values.flatten().collect::<Vec<_>>();
    if values.is_empty() {
        return DeltaAggregate {
            direction: direction.label().to_string(),
            ..DeltaAggregate::default()
        };
    }
    values.sort_unstable();
    let samples = values.len();
    let mean = values.iter().map(|value| *value as f64).sum::<f64>() / samples as f64;
    let median = if samples % 2 == 1 {
        values[samples / 2] as f64
    } else {
        (values[samples / 2 - 1] as f64 + values[samples / 2] as f64) / 2.0
    };
    let mut aggregate = DeltaAggregate {
        direction: direction.label().to_string(),
        samples,
        mean: Some(mean),
        median: Some(median),
        ..DeltaAggregate::default()
    };
    if matches!(direction, MetricDirection::DescriptiveOnly) {
        return aggregate;
    }
    aggregate.improved = Some(0);
    aggregate.unchanged = Some(0);
    aggregate.worsened = Some(0);
    for value in values {
        let interpreted = match direction {
            MetricDirection::HigherIsBetter => value,
            MetricDirection::LowerIsBetter => -value,
            MetricDirection::DescriptiveOnly => unreachable!(),
        };
        match interpreted.cmp(&0) {
            std::cmp::Ordering::Greater => {
                aggregate.improved = aggregate.improved.map(|count| count + 1)
            }
            std::cmp::Ordering::Equal => {
                aggregate.unchanged = aggregate.unchanged.map(|count| count + 1)
            }
            std::cmp::Ordering::Less => {
                aggregate.worsened = aggregate.worsened.map(|count| count + 1)
            }
        }
    }
    aggregate
}

fn aggregate_comparison(
    comparison: PreparationComparison,
    pairs: &[PairResult],
) -> ComparisonAggregate {
    let selected = pairs
        .iter()
        .filter(|pair| pair.comparison == comparison.label())
        .collect::<Vec<_>>();
    let mut metrics = BTreeMap::new();
    macro_rules! metric {
        ($name:literal, $field:ident, $higher:expr) => {
            metrics.insert(
                $name.to_string(),
                delta_aggregate(
                    selected
                        .iter()
                        .map(|pair| pair.deltas_treatment_minus_control.$field),
                    $higher,
                ),
            );
        };
    }
    metric!("resolution", resolution, MetricDirection::HigherIsBetter);
    metric!("survival", survival, MetricDirection::HigherIsBetter);
    let hero_damage_direction = if comparison.includes_healing() {
        MetricDirection::DescriptiveOnly
    } else {
        MetricDirection::LowerIsBetter
    };
    metric!(
        "hero_damage_taken",
        hero_damage_taken,
        hero_damage_direction
    );
    metric!("hero_deaths", hero_deaths, MetricDirection::LowerIsBetter);
    metric!(
        "villager_losses",
        villager_losses,
        MetricDirection::LowerIsBetter
    );
    metric!(
        "total_villager_damage",
        total_villager_damage,
        MetricDirection::LowerIsBetter
    );
    let structure_direction = if comparison.includes_wall() {
        MetricDirection::DescriptiveOnly
    } else {
        MetricDirection::LowerIsBetter
    };
    metric!(
        "structures_damaged",
        structures_damaged,
        structure_direction
    );
    metric!(
        "structures_destroyed",
        structures_destroyed,
        structure_direction
    );
    metric!(
        "total_structure_damage",
        total_structure_damage,
        structure_direction
    );
    metric!("walls_destroyed", walls_destroyed, structure_direction);
    metric!(
        "observed_assault_ticks",
        observed_assault_ticks,
        MetricDirection::DescriptiveOnly
    );
    metric!(
        "assault_units_defeated",
        assault_units_defeated,
        MetricDirection::HigherIsBetter
    );

    ComparisonAggregate {
        comparison: comparison.label().to_string(),
        pairs_requested: selected.len(),
        pairs_with_valid_launch_fingerprint: selected
            .iter()
            .filter(|pair| pair.launch_validation_error.is_none())
            .count(),
        pairs_with_quantitative_deltas: selected
            .iter()
            .filter(|pair| pair.deltas_treatment_minus_control.survival.is_some())
            .count(),
        setup_failures: selected
            .iter()
            .flat_map(|pair| [&pair.control, &pair.treatment])
            .filter(|leg| leg.status == "setup_failure")
            .count(),
        panics: selected
            .iter()
            .flat_map(|pair| [&pair.control, &pair.treatment])
            .filter(|leg| leg.status == "panic")
            .count(),
        timeouts_or_unresolved: selected
            .iter()
            .flat_map(|pair| [&pair.control, &pair.treatment])
            .filter(|leg| matches!(leg.status.as_str(), "timeout_unresolved" | "unresolved"))
            .count(),
        overall_improved: selected
            .iter()
            .filter(|pair| pair.overall_classification == "improved")
            .count(),
        overall_unchanged: selected
            .iter()
            .filter(|pair| pair.overall_classification == "unchanged")
            .count(),
        overall_worsened: selected
            .iter()
            .filter(|pair| pair.overall_classification == "worsened")
            .count(),
        overall_unclassified: selected
            .iter()
            .filter(|pair| pair.overall_classification == "unclassified")
            .count(),
        metrics,
    }
}

fn build_report(config: &RunnerConfig) -> PreparationPairReport {
    let mut pairs = Vec::new();
    for comparison in config.comparisons.iter().copied() {
        for pair_index in 0..config.pairs_per_comparison {
            pairs.push(run_pair(
                pair_index,
                comparison,
                config.assault_relative_cap_ticks,
            ));
        }
    }
    let aggregates = config
        .comparisons
        .iter()
        .copied()
        .map(|comparison| aggregate_comparison(comparison, &pairs))
        .collect();
    PreparationPairReport {
        schema_version: SCHEMA_VERSION,
        methodology: Methodology {
            design: "matched_observed_launch_fields_control_treatment_pairs",
            matched_observed_launch_fields: true,
            full_ecs_state_matched: false,
            launch_geometry_normalized: true,
            random_stream_replayed: false,
            requested_seed_label_semantics: "pair label requested by the harness; not an RNG seed and not evidence of shared random draws",
            combat_policy: "existing Bot BasicSurvival policy with production combat",
            treatment_validation: "the selected observed launch fingerprint must match after normalizing only the comparison-specific Stockade, Hide Wraps plus its exact displaced starting shirt, or Crude Bandage artifact; hidden ECS state and RNG state are not claimed equivalent; exact declared fixture details are validated",
        },
        pairs_per_comparison: config.pairs_per_comparison,
        assault_relative_cap_ticks: config.assault_relative_cap_ticks,
        pairs,
        aggregates,
    }
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
    let json = serde_json::to_string_pretty(&report).expect("serialize preparation pair report");
    if let Some(path) = &config.output {
        if let Err(error) = std::fs::write(path, format!("{json}\n")) {
            eprintln!("failed to write {path}: {error}");
            std::process::exit(1);
        }
    } else {
        println!("{json}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_reports_mean_median_and_direction_counts() {
        let aggregate = delta_aggregate(
            [Some(-4), Some(0), Some(2), None].into_iter(),
            MetricDirection::LowerIsBetter,
        );
        assert_eq!(aggregate.samples, 3);
        assert_eq!(aggregate.direction, "lower_is_better");
        assert_eq!(aggregate.mean, Some(-2.0 / 3.0));
        assert_eq!(aggregate.median, Some(0.0));
        assert_eq!(aggregate.improved, Some(1));
        assert_eq!(aggregate.unchanged, Some(1));
        assert_eq!(aggregate.worsened, Some(1));

        let descriptive = delta_aggregate(
            [Some(20), Some(0)].into_iter(),
            MetricDirection::DescriptiveOnly,
        );
        assert_eq!(descriptive.direction, "descriptive_only");
        assert_eq!(descriptive.improved, None);
        assert_eq!(descriptive.unchanged, None);
        assert_eq!(descriptive.worsened, None);
    }

    #[test]
    fn report_serialization_is_explicit_about_rng_provenance() {
        let report = PreparationPairReport {
            schema_version: SCHEMA_VERSION,
            methodology: Methodology {
                design: "matched_observed_launch_fields_control_treatment_pairs",
                matched_observed_launch_fields: true,
                full_ecs_state_matched: false,
                launch_geometry_normalized: true,
                random_stream_replayed: false,
                requested_seed_label_semantics: "not an RNG seed",
                combat_policy: "BasicSurvival",
                treatment_validation: "fingerprint",
            },
            pairs_per_comparison: 5,
            assault_relative_cap_ticks: DEFAULT_ASSAULT_RELATIVE_CAP_TICKS,
            pairs: Vec::new(),
            aggregates: Vec::new(),
        };
        let value = serde_json::to_value(report).expect("serialize report");
        assert_eq!(value["schema_version"], SCHEMA_VERSION);
        assert_eq!(value["methodology"]["random_stream_replayed"], false);
        assert_eq!(value["pairs_per_comparison"], 5);
        assert_eq!(
            value["assault_relative_cap_ticks"],
            DEFAULT_ASSAULT_RELATIVE_CAP_TICKS
        );
    }

    #[test]
    fn assault_observation_continues_through_ordinary_death() {
        assert!(!should_stop_assault_observation(true, false, false));
        assert!(!should_stop_assault_observation(true, true, false));
        assert!(should_stop_assault_observation(true, true, true));
        assert!(should_stop_assault_observation(false, false, false));
    }

    #[test]
    fn added_wall_damage_is_descriptive_not_automatically_worse() {
        let wall_absorption_only = PairDeltas {
            survival: Some(0),
            resolution: Some(0),
            hero_deaths: Some(0),
            villager_losses: Some(0),
            hero_damage_taken: Some(0),
            assault_units_defeated: Some(0),
            structures_damaged: Some(1),
            structures_destroyed: Some(1),
            total_structure_damage: Some(20),
            walls_destroyed: Some(1),
            ..PairDeltas::default()
        };
        assert_eq!(
            classify_pair(PreparationComparison::ExistingWalls, &wall_absorption_only),
            "unchanged"
        );

        let hero_benefit = PairDeltas {
            hero_damage_taken: Some(-10),
            ..wall_absorption_only
        };
        assert_eq!(
            classify_pair(PreparationComparison::ExistingWalls, &hero_benefit),
            "improved"
        );

        let healing_absorption = PairDeltas {
            survival: Some(0),
            resolution: Some(0),
            hero_deaths: Some(0),
            hero_damage_taken: Some(10),
            assault_units_defeated: Some(0),
            ..PairDeltas::default()
        };
        assert_eq!(
            classify_pair(PreparationComparison::HealingPrepared, &healing_absorption),
            "unchanged"
        );

        let conflicting_major_outcomes = PairDeltas {
            survival: Some(1),
            resolution: Some(-1),
            hero_deaths: Some(0),
            villager_losses: Some(0),
            hero_damage_taken: Some(0),
            assault_units_defeated: Some(0),
            ..PairDeltas::default()
        };
        assert_eq!(
            classify_pair(
                PreparationComparison::EquipmentPrepared,
                &conflicting_major_outcomes,
            ),
            "worsened",
            "an improvement must not hide a simultaneous guardrail regression"
        );
    }
}
