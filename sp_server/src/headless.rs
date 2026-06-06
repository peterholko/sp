// In-process headless test harness for sp_server.
//
// Builds the Bevy game `App` directly (no TLS / WebSocket / Postgres / real-time
// scheduler), drives it with a deterministic scripted bot, and fast-forwards
// game time by pumping `app.update()`. Run many full games back-to-back to
// collect balance/metrics data. See `headless_bot.rs` for the bot and
// `bin/headless_runner.rs` for the multi-game runner.
//
// Isolation: each `HeadlessGame` owns its own `App` (its own `World`/resources).
// Dropping and recreating a `HeadlessGame` between runs fully isolates them — the
// only process-global statics (`LOG_RELOAD_HANDLE`, `TILESET`) hold no per-game
// mutable state and are not touched on the headless path.

use std::collections::HashSet;

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Sender as CBSender};
use serde::Serialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::constants::{DATABASE_MANAGER_ID, GAME_TICKS_PER_DAY};
use crate::database::DatabaseEvent;
use crate::game::{
    Client, Clients, DatabaseClient, DatabaseManagers, GameTick, NetworkReceiver, Objectives,
    PlayerObjectives, PlayerRunScore, PlayerStats, PlayerVictory, RunScoreState, VictoryState,
};
use crate::item::Inventory;
use crate::map::Map;
use crate::obj::{ClassStructure, Id, PlayerId, Position, State, Stats, SubclassHero, SubclassNPC, TrueDeath};
use crate::resource::Resources;
use crate::skill::Skills;
use crate::{build_headless_app, AppState, PlayerEvent, ResponsePacket};

// Deterministic player id for the single headless hero. MUST be < MAX_PLAYER_ID
// (1000) so `PlayerId::is_human()` is true and NPC factions (player id 1000+)
// stay distinct.
pub const HEADLESS_PLAYER_ID: i32 = 1;

// Bounded tokio channel capacity for captured client packets. `tick()` drains
// every update so this never has to hold more than one update's worth of output.
const PACKET_CHANNEL_CAP: usize = 16_384;
const DB_CHANNEL_CAP: usize = 1_024;

// Cheap, owned snapshot of the bits of `World` the bot reasons about. Read once
// per decision step via `observe()` so the bot stays pure data-in / action-out.
pub struct WorldView {
    pub hero: Option<HeroView>,
    pub enemies: Vec<UnitView>,
    pub occupied: HashSet<(i32, i32)>,
    pub resource_tiles: Vec<Position>,
    pub structures_built: i32,
    pub game_tick: i32,
    pub day: i32,
}

#[derive(Clone, Copy)]
pub struct HeroView {
    pub id: i32,
    pub pos: Position,
    pub hp: i32,
    pub state: State,
    pub dead: bool,
    pub true_death: bool,
}

impl HeroView {
    // Ready to receive a new command: idle (not moving/gathering/etc.) and alive.
    pub fn is_idle(&self) -> bool {
        !self.dead && !self.true_death && self.state == State::None
    }
}

#[derive(Clone, Copy)]
pub struct UnitView {
    pub id: i32,
    pub player_id: i32,
    pub pos: Position,
}

// Per-run metrics emitted by the runner (CSV + JSON). Field names mirror the
// game's own state structs (`PlayerRunScore`, `PlayerObjectives`,
// `PlayerVictory`) so the data lines up with in-game scoring.
#[derive(Debug, Clone, Serialize)]
pub struct RunMetrics {
    pub run_index: u32,
    pub outcome: String,
    pub ticks: i32,
    pub days_survived: i32,
    // PlayerRunScore
    pub waves_survived: i32,
    pub enemies_killed: i32,
    pub elites_killed: i32,
    pub captains_killed: i32,
    pub legendary_kills: i32,
    pub hideouts_cleared: i32,
    pub repairs: i32,
    pub highest_pressure_level: i32,
    pub num_deaths: u32,
    // PlayerObjectives (the 10 onboarding/goal flags)
    pub obj_scavenge_shipwreck: bool,
    pub obj_build_campfire: bool,
    pub obj_win_first_fight: bool,
    pub obj_build_3_structures: bool,
    pub obj_recruit_villager: bool,
    pub obj_explore_poi: bool,
    pub obj_choose_expansion: bool,
    pub obj_survive_5_nights: bool,
    pub obj_find_legendary_hideout: bool,
    pub obj_defeat_ashen_warlord: bool,
    // PlayerVictory
    pub victory_rescue_progress: i32,
    pub victory_prosperity: bool,
    pub victory_conquest: bool,
    // Hero end-state
    pub final_hp: i32,
    pub final_skill_total: i32,
    pub final_inventory_count: i32,
    pub structures_built: i32,
}

pub struct HeadlessGame {
    app: App,
    player_id: i32,
    // game-tick at which the hero was spawned; survival measured relative to this
    spawn_tick: i32,
    // run for at most this many game ticks past `spawn_tick`
    max_ticks: i32,

    // crossbeam: harness -> game (player input). Game side is `NetworkReceiver`.
    event_tx: CBSender<PlayerEvent>,
    // shared client map; we hold a clone of the same Arc the resource holds.
    clients: Clients,
    // tokio mpsc: game -> harness (captured client packets / db events).
    packet_tx: mpsc::Sender<String>,
    packet_rx: mpsc::Receiver<String>,
    // kept alive so the dummy DatabaseManager's sender stays open.
    _db_rx: mpsc::Receiver<DatabaseEvent>,

    // number of `app.update()` calls made (debug/info only).
    tick_count: u64,
}

impl HeadlessGame {
    // Build a fresh headless game. `max_ticks` is the number of game ticks the
    // run is allowed to advance past hero spawn before `is_over()` caps it.
    pub fn new(max_ticks: i32) -> Self {
        let mut app = build_headless_app();

        // Player input channel (harness -> game).
        let (event_tx, event_rx) = unbounded::<PlayerEvent>();

        // Output channels (game -> harness). Bounded tokio mpsc; `try_send` /
        // `try_recv` need no running runtime.
        let (packet_tx, packet_rx) = mpsc::channel::<String>(PACKET_CHANNEL_CAP);
        let (db_tx, db_rx) = mpsc::channel::<DatabaseEvent>(DB_CHANNEL_CAP);

        // Insert the network resources the production path would build in
        // `network_init`. We keep a clone of `clients` so `spawn_hero` can add a
        // Client routed to our packet channel.
        let clients = Clients::default();

        let database_managers = DatabaseManagers::default();
        database_managers.lock().unwrap().insert(
            DATABASE_MANAGER_ID,
            DatabaseClient { sender: db_tx },
        );

        app.insert_resource(NetworkReceiver::new(event_rx));
        app.insert_resource(clients.clone());
        app.insert_resource(database_managers);

        let mut game = HeadlessGame {
            app,
            player_id: 0,
            spawn_tick: 0,
            max_ticks,
            event_tx,
            clients,
            packet_tx,
            packet_rx,
            _db_rx: db_rx,
            tick_count: 0,
        };

        // Pump PreStartup (world_init) -> set Running -> OnEnter(Running) init.
        game.pump_until_running();

        game
    }

    fn pump_until_running(&mut self) {
        for _ in 0..32 {
            self.app.update();
            self.tick_count += 1;
            self.drain_io();
            if self.is_running() {
                break;
            }
        }
        // A couple more so OnEnter(Running) systems (e.g. init_objs) have run and
        // the first Update systems execute under AppState::Running.
        for _ in 0..2 {
            self.app.update();
            self.tick_count += 1;
            self.drain_io();
        }
    }

    fn is_running(&self) -> bool {
        self.app
            .world()
            .get_resource::<bevy::state::state::State<AppState>>()
            .map(|s| *s.get() == AppState::Running)
            .unwrap_or(false)
    }

    // Create the hero. Inserts a Client routed to our packet channel, injects the
    // exact `NewPlayer` event `handle_selected_class` would send, and ticks until
    // the hero has spawned. `class` is "Warrior" | "Ranger" | "Mage".
    pub fn spawn_hero(&mut self, class: &str, name: &str) -> i32 {
        let pid = HEADLESS_PLAYER_ID;
        self.player_id = pid;

        // Deterministic client uuid (no RNG) — the game keys by player_id, the
        // uuid is only the client-map key.
        let client = Client {
            id: Uuid::from_u128(pid as u128),
            player_id: pid,
            sender: self.packet_tx.clone(),
        };
        self.clients.lock().unwrap().insert(client.id, client);

        self.inject(PlayerEvent::NewPlayer {
            player_id: pid,
            hero_name: name.to_string(),
            class_name: class.to_string(),
        });

        // Broker drains one event/tick; new_player_system spawns the hero
        // synchronously; the Login game event runs at +4 ticks.
        self.tick(8);
        self.spawn_tick = self.game_tick();

        pid
    }

    pub fn inject(&mut self, event: PlayerEvent) {
        // Send is effectively infallible (unbounded crossbeam channel); ignore the
        // error if the game side was somehow dropped.
        let _ = self.event_tx.send(event);
    }

    // Advance `n` game ticks by pumping `app.update()`, draining output each step
    // so the bounded packet/db channels never overflow.
    pub fn tick(&mut self, n: u32) {
        for _ in 0..n {
            self.app.update();
            self.tick_count += 1;
            self.drain_io();
        }
    }

    // Keep the bounded output channels empty. Captured packets are debug-only and
    // the bot reads `World` directly, so we discard here.
    fn drain_io(&mut self) {
        while self.packet_rx.try_recv().is_ok() {}
        while self._db_rx.try_recv().is_ok() {}
    }

    // Drain whatever client packets are currently queued, deserialized. Mainly for
    // debugging/asserts — note `tick()` already drains, so call this between an
    // inject and the next tick if you want to observe a specific response.
    pub fn drain_packets(&mut self) -> Vec<ResponsePacket> {
        let mut out = Vec::new();
        while let Ok(s) = self.packet_rx.try_recv() {
            if let Ok(p) = serde_json::from_str::<ResponsePacket>(&s) {
                out.push(p);
            }
        }
        out
    }

    pub fn world(&self) -> &World {
        self.app.world()
    }

    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    pub fn player_id(&self) -> i32 {
        self.player_id
    }

    pub fn game_tick(&self) -> i32 {
        self.app
            .world()
            .get_resource::<GameTick>()
            .map(|t| t.0)
            .unwrap_or(0)
    }

    pub fn ticks_pumped(&self) -> u64 {
        self.tick_count
    }

    // Read the slice of `World` the bot needs, as owned data.
    pub fn observe(&mut self) -> WorldView {
        let pid = self.player_id;
        let game_tick = self.game_tick();
        let day = (game_tick / GAME_TICKS_PER_DAY) + 1;

        let world = self.app.world_mut();

        // Hero.
        let hero = {
            let mut q = world.query_filtered::<(
                &Id,
                &PlayerId,
                &Position,
                &Stats,
                &State,
                Option<&TrueDeath>,
            ), With<SubclassHero>>();
            q.iter(world)
                .find(|(_, p, _, _, _, _)| p.0 == pid)
                .map(|(id, _p, pos, stats, state, td)| HeroView {
                    id: id.0,
                    pos: *pos,
                    hp: stats.hp,
                    state: *state,
                    dead: *state == State::Dead,
                    true_death: td.is_some(),
                })
        };

        // Living enemy NPCs.
        let enemies = {
            let mut q = world
                .query_filtered::<(&Id, &PlayerId, &Position, &State), With<SubclassNPC>>();
            q.iter(world)
                .filter(|(_, _, _, state)| **state != State::Dead)
                .map(|(id, p, pos, _)| UnitView {
                    id: id.0,
                    player_id: p.0,
                    pos: *pos,
                })
                .collect::<Vec<_>>()
        };

        // Every positioned object's tile (move-target occupancy avoidance).
        let occupied = {
            let mut q = world.query::<&Position>();
            q.iter(world).map(|p| (p.x, p.y)).collect::<HashSet<_>>()
        };

        // Structures owned by the hero's player.
        let structures_built = {
            let mut q = world.query_filtered::<&PlayerId, With<ClassStructure>>();
            q.iter(world).filter(|p| p.0 == pid).count() as i32
        };

        // Resource node tiles (the `Resources` map is keyed by Position).
        let resource_tiles = world
            .resource::<Resources>()
            .keys()
            .copied()
            .collect::<Vec<_>>();

        WorldView {
            hero,
            enemies,
            occupied,
            resource_tiles,
            structures_built,
            game_tick,
            day,
        }
    }

    pub fn map(&self) -> &Map {
        self.app.world().resource::<Map>()
    }

    // Run is over when capped, the hero permadied (TrueDeath or despawned), or a
    // victory was achieved.
    pub fn is_over(&mut self) -> bool {
        if self.game_tick() - self.spawn_tick >= self.max_ticks {
            return true;
        }

        let pid = self.player_id;
        let world = self.app.world_mut();

        // Victory?
        if let Some(v) = world.resource::<VictoryState>().get(&pid) {
            if v.prosperity || v.conquest || v.rescue_progress > 0 {
                return true;
            }
        }

        // Hero permadead or gone?
        let mut q = world
            .query_filtered::<(&PlayerId, Option<&TrueDeath>), With<SubclassHero>>();
        match q.iter(world).find(|(p, _)| p.0 == pid) {
            Some((_, td)) => td.is_some(),
            None => true,
        }
    }

    pub fn metrics(&mut self) -> RunMetrics {
        let pid = self.player_id;
        let current_tick = self.game_tick();
        let world = self.app.world_mut();

        // Hero end-state (owned primitives so the borrow ends before the next query).
        let (final_hp, final_skill_total, final_inventory_count, hero_true_death, hero_present) = {
            let mut q = world.query_filtered::<(
                &PlayerId,
                &Stats,
                &Skills,
                &Inventory,
                Option<&TrueDeath>,
            ), With<SubclassHero>>();
            match q.iter(world).find(|(p, ..)| p.0 == pid) {
                Some((_p, stats, skills, inv, td)) => (
                    stats.hp,
                    skills.get_total_xp(),
                    inv.items.len() as i32,
                    td.is_some(),
                    true,
                ),
                None => (0, 0, 0, true, false),
            }
        };

        let structures_built = {
            let mut q = world.query_filtered::<&PlayerId, With<ClassStructure>>();
            q.iter(world).filter(|p| p.0 == pid).count() as i32
        };

        let run: PlayerRunScore = world
            .resource::<RunScoreState>()
            .get(&pid)
            .cloned()
            .unwrap_or_default();
        let num_deaths = world
            .resource::<PlayerStats>()
            .get(&pid)
            .map(|s| s.num_deaths)
            .unwrap_or(0);
        let objectives: PlayerObjectives = world
            .resource::<Objectives>()
            .get(&pid)
            .cloned()
            .unwrap_or_default();
        let victory: PlayerVictory = world
            .resource::<VictoryState>()
            .get(&pid)
            .cloned()
            .unwrap_or_default();

        let start_tick = if run.start_tick != 0 {
            run.start_tick
        } else {
            self.spawn_tick
        };
        let ticks = (current_tick - start_tick).max(0);
        let days_survived = ticks / GAME_TICKS_PER_DAY;

        let outcome = if !hero_present || hero_true_death {
            "TrueDeath".to_string()
        } else if victory.prosperity {
            "Victory:Prosperity".to_string()
        } else if victory.conquest {
            "Victory:Conquest".to_string()
        } else if victory.rescue_progress > 0 {
            "Victory:Rescue".to_string()
        } else {
            "MaxTicks".to_string()
        };

        RunMetrics {
            run_index: 0,
            outcome,
            ticks,
            days_survived,
            waves_survived: run.waves_survived,
            enemies_killed: run.enemies_killed,
            elites_killed: run.elites_killed,
            captains_killed: run.captains_killed,
            legendary_kills: run.legendary_kills,
            hideouts_cleared: run.hideouts_cleared,
            repairs: run.repairs,
            highest_pressure_level: run.highest_pressure_level,
            num_deaths,
            obj_scavenge_shipwreck: objectives.scavenge_shipwreck,
            obj_build_campfire: objectives.build_campfire,
            obj_win_first_fight: objectives.win_first_fight,
            obj_build_3_structures: objectives.build_3_structures,
            obj_recruit_villager: objectives.recruit_villager,
            obj_explore_poi: objectives.explore_poi,
            obj_choose_expansion: objectives.choose_expansion,
            obj_survive_5_nights: objectives.survive_5_nights,
            obj_find_legendary_hideout: objectives.find_legendary_hideout,
            obj_defeat_ashen_warlord: objectives.defeat_ashen_warlord,
            victory_rescue_progress: victory.rescue_progress,
            victory_prosperity: victory.prosperity,
            victory_conquest: victory.conquest,
            final_hp,
            final_skill_total,
            final_inventory_count,
            structures_built,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // One short capped game end-to-end. Must run with CWD = sp_server/ so the
    // templates/map/tileset files load by relative path.
    #[test]
    fn smoke() {
        let mut game = HeadlessGame::new(1_000);
        let pid = game.spawn_hero("Warrior", "SmokeBot");

        assert!(pid > 0, "player_id should be positive");

        // World built and hero present after spawn.
        let view = game.observe();
        assert!(view.hero.is_some(), "hero should exist after spawn_hero");
        assert!(
            !view.resource_tiles.is_empty(),
            "resource nodes should have spawned (world built)"
        );

        // Fast-forward; game time should advance.
        let before = game.game_tick();
        game.tick(100);
        let after = game.game_tick();
        assert!(after > before, "game tick should advance when pumping");
        assert!(game.ticks_pumped() > 0);

        // Metrics readable.
        let m = game.metrics();
        assert!(m.ticks >= 0);
    }
}
