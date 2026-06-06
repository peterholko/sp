// Multi-game headless balance/metrics runner.
//
//   cargo run --bin headless_runner [N] [MAX_TICKS]
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

// Game ticks a single run may advance past hero spawn before being capped.
// 2400 ticks = 1 in-game day; 30k ≈ 12.5 days, enough to reach the rescue
// victory / late survival waves while staying fast.
const DEFAULT_MAX_TICKS: i32 = 30_000;
const DEFAULT_NUM_GAMES: u32 = 20;
// Game ticks advanced between bot decisions. A hero move resolves in ~12 ticks;
// 8 lets the move start before the bot re-evaluates without cancelling it.
const DECISION_TICKS: u32 = 8;

fn run_one(run_index: u32, max_ticks: i32) -> RunMetrics {
    let mut game = HeadlessGame::new(max_ticks);
    let pid = game.spawn_hero("Warrior", &format!("Bot{run_index}"));
    let mut bot = Bot::new(pid);

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
    metrics
    // `game` dropped here -> App/World freed -> next run fully isolated.
}

// Run one game, but never let a panic inside the game-under-test abort the whole
// batch. A panicking run is recorded with outcome "Panic" and its (discarded)
// App is dropped; the next run builds a fresh one. Each run already owns its own
// App, so a caught panic cannot leak state into later runs.
fn run_one_safe(run_index: u32, max_ticks: i32) -> RunMetrics {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_one(run_index, max_ticks)
    }));
    match result {
        Ok(metrics) => metrics,
        Err(_) => panic_metrics(run_index),
    }
}

fn panic_metrics(run_index: u32) -> RunMetrics {
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

    println!(
        "Running {num_games} headless games (max_ticks={max_ticks}, decision_ticks={DECISION_TICKS})..."
    );

    let mut results: Vec<RunMetrics> = Vec::with_capacity(num_games as usize);
    for i in 0..num_games {
        let t0 = std::time::Instant::now();
        let m = run_one_safe(i, max_ticks);
        let elapsed = t0.elapsed();
        println!(
            "  run {:>4}: {:<16} killer={:<12} ticks={:>6} days={:>2} enemies={:>3} deaths={} hp={:>4} skillxp={:>5} inv={:>2} structs={} [{:.2}s]",
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
            elapsed.as_secs_f64(),
        );
        results.push(m);
    }

    write_csv(&results, "headless_runs.csv");
    write_json(&results, "headless_runs.json");
    print_summary(&results);
}

fn write_csv(results: &[RunMetrics], path: &str) {
    let mut file = match File::create(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create {path}: {e}");
            return;
        }
    };

    let header = "run_index,outcome,killer,ticks,days_survived,waves_survived,enemies_killed,\
elites_killed,captains_killed,legendary_kills,hideouts_cleared,repairs,highest_pressure_level,\
num_deaths,obj_scavenge_shipwreck,obj_build_campfire,obj_win_first_fight,obj_build_3_structures,\
obj_recruit_villager,obj_explore_poi,obj_choose_expansion,obj_survive_5_nights,\
obj_find_legendary_hideout,obj_defeat_ashen_warlord,victory_rescue_progress,victory_prosperity,\
victory_conquest,final_hp,final_skill_total,final_inventory_count,structures_built";
    let _ = writeln!(file, "{header}");

    for m in results {
        let _ = writeln!(
            file,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            m.run_index,
            m.outcome,
            m.killer,
            m.ticks,
            m.days_survived,
            m.waves_survived,
            m.enemies_killed,
            m.elites_killed,
            m.captains_killed,
            m.legendary_kills,
            m.hideouts_cleared,
            m.repairs,
            m.highest_pressure_level,
            m.num_deaths,
            m.obj_scavenge_shipwreck,
            m.obj_build_campfire,
            m.obj_win_first_fight,
            m.obj_build_3_structures,
            m.obj_recruit_villager,
            m.obj_explore_poi,
            m.obj_choose_expansion,
            m.obj_survive_5_nights,
            m.obj_find_legendary_hideout,
            m.obj_defeat_ashen_warlord,
            m.victory_rescue_progress,
            m.victory_prosperity,
            m.victory_conquest,
            m.final_hp,
            m.final_skill_total,
            m.final_inventory_count,
            m.structures_built,
        );
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
    let deaths = results
        .iter()
        .filter(|m| m.outcome == "TrueDeath")
        .count();
    let panics = results.iter().filter(|m| m.outcome == "Panic").count();

    let mean = |f: &dyn Fn(&RunMetrics) -> f64| -> f64 {
        results.iter().map(|m| f(m)).sum::<f64>() / n
    };

    let mut ticks: Vec<i32> = results.iter().map(|m| m.ticks).collect();
    ticks.sort_unstable();
    let pct = |p: f64| -> i32 {
        let idx = (((ticks.len() as f64) * p).ceil() as usize)
            .saturating_sub(1)
            .min(ticks.len() - 1);
        ticks[idx]
    };

    println!("\n========== Aggregate summary ({} runs) ==========", results.len());
    println!("win rate            : {:.1}% ({wins}/{})", 100.0 * wins as f64 / n, results.len());
    println!("true-death rate     : {:.1}% ({deaths}/{})", 100.0 * deaths as f64 / n, results.len());
    println!("panic rate          : {:.1}% ({panics}/{})", 100.0 * panics as f64 / n, results.len());
    println!("mean days survived  : {:.2}", mean(&|m| m.days_survived as f64));
    println!("mean enemies killed : {:.2}", mean(&|m| m.enemies_killed as f64));
    println!("mean deaths         : {:.2}", mean(&|m| m.num_deaths as f64));
    println!("mean final skill xp : {:.1}", mean(&|m| m.final_skill_total as f64));
    println!("mean inventory count: {:.1}", mean(&|m| m.final_inventory_count as f64));
    println!("mean structures     : {:.2}", mean(&|m| m.structures_built as f64));
    println!("ticks p50 / p90     : {} / {}", pct(0.50), pct(0.90));
    println!("=================================================");
}
