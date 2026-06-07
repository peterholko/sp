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

use crate::common::{Hunger, Thirst, Tired};
use crate::constants::{
    DATABASE_MANAGER_ID, FOOD, GAME_ANIMAL, GAME_TICKS_PER_DAY, PLANT, SPRING_WATER,
};
use crate::database::DatabaseEvent;
use crate::common::Transport;
use crate::game::{
    Client, Clients, DatabaseClient, DatabaseManagers, GameTick, Merchant, MerchantSailState,
    NetworkReceiver, Objectives, PlayerObjectives, PlayerRunScore, PlayerStats, PlayerVictory,
    RunScoreState, VictoryState,
};
use crate::item::{AttrKey, AttrVal, Inventory};
use crate::map::Map;
use crate::obj::{
    ClassStructure, Id, Order, PlayerId, Position, State, StateDead, Stats, Subclass, SubclassHero,
    SubclassNPC, SubclassVillager, Template, TrueDeath,
};
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
    pub inventory: Vec<ItemView>,
    pub enemies: Vec<UnitView>,
    pub villagers: Vec<VillagerView>,
    pub pois: Vec<PoiView>,
    pub merchant: Option<MerchantView>,
    pub structures: Vec<StructureView>,
    pub resource_tiles: Vec<ResTileView>,
    pub occupied: HashSet<(i32, i32)>,
    pub game_tick: i32,
    pub day: i32,
}

impl WorldView {
    pub fn structures_built(&self) -> i32 {
        self.structures.iter().filter(|s| s.built).count() as i32
    }

    // The hero's "home" anchor for retreat: prefer the campfire, else any owned
    // built structure (the starter Burrow), else None.
    pub fn home(&self) -> Option<Position> {
        self.structures
            .iter()
            .find(|s| s.subclass == "campfire" && s.built)
            .or_else(|| self.structures.iter().find(|s| s.built))
            .map(|s| s.pos)
    }

    pub fn has_built(&self, subclass: &str) -> bool {
        self.structures
            .iter()
            .any(|s| s.subclass == subclass && s.built)
    }
}

#[derive(Clone, Copy)]
pub struct HeroView {
    pub id: i32,
    pub pos: Position,
    pub hp: i32,
    pub base_hp: i32,
    pub state: State,
    pub dead: bool,
    pub true_death: bool,
    // Survival needs (0..=100); the game auto-eats/drinks/sleeps only while idle.
    pub thirst: f32,
    pub hunger: f32,
    pub tired: f32,
}

impl HeroView {
    // Ready to receive a new command: idle (not moving/gathering/etc.) and alive.
    pub fn is_idle(&self) -> bool {
        !self.dead && !self.true_death && self.state == State::None
    }

    pub fn hp_frac(&self) -> f32 {
        if self.base_hp <= 0 {
            1.0
        } else {
            self.hp as f32 / self.base_hp as f32
        }
    }
}

#[derive(Clone, Copy)]
pub struct UnitView {
    pub id: i32,
    pub player_id: i32,
    pub pos: Position,
}

#[derive(Clone)]
pub struct ItemView {
    pub id: i32,
    pub name: String,
    pub class: String,
    pub subclass: String,
    pub quantity: i32,
    pub equipped: bool,
    pub is_healing: bool,
    pub is_weapon: bool,
    pub is_hunting: bool, // weapon/tool with a Hunting attr (can take down game)
    pub feed: f32,        // Feed value (0 if not edible)
    pub food_poisoning: bool, // eating it risks food poisoning (raw meat)
}

impl ItemView {
    pub fn is_drink(&self) -> bool {
        self.class == "Drink"
    }
    // Safe to eat for hunger: has Feed and won't poison the hero.
    pub fn is_edible(&self) -> bool {
        self.class == "Food" && self.feed > 0.0 && !self.food_poisoning
    }
}

impl ItemView {
    // Mirrors item::req_matches_build: a build requirement of `req_type` is met
    // by an item whose name, class, or subclass equals it (plus Log<-Timber).
    pub fn matches_req(&self, req_type: &str) -> bool {
        req_type == self.name
            || req_type == self.class
            || req_type == self.subclass
            || (req_type == "Log" && self.class == "Timber")
    }
}

#[derive(Clone, Copy)]
pub struct VillagerView {
    pub id: i32,
    pub pos: Position,
    pub idle: bool,
    /// True while the villager has a Gather order (vs. None/other).
    pub gathering_order: bool,
    /// True while the villager is actively in the Gathering state.
    pub gathering_now: bool,
    /// Count of Food-class items the villager is currently carrying.
    pub food_carried: i32,
}

#[derive(Clone)]
pub struct PoiView {
    pub id: i32,
    pub pos: Position,
    pub template: String, // e.g. "Shipwreck"
}

#[derive(Clone)]
pub struct MerchantView {
    pub id: i32,
    pub pos: Position,
    /// True only while the merchant has sailed in and is docked (trade window
    /// open). The bot should only approach/hire when this is true.
    pub at_landing: bool,
    /// Obj ids of the villagers currently aboard the merchant, available to hire.
    pub hireable: Vec<i32>,
}

#[derive(Clone)]
pub struct StructureView {
    pub id: i32,
    pub pos: Position,
    pub subclass: String,
    pub founded: bool, // placed, not yet built (needs resources + build)
    pub building: bool,
    pub built: bool, // construction complete
    pub inventory: Vec<ItemView>,
}

#[derive(Clone, Copy)]
pub struct ResTileView {
    pub pos: Position,
    pub revealed: bool,        // any resource on the tile is revealed (gatherable)
    pub has_spring: bool,      // a Spring Water resource exists here (maybe hidden)
    pub spring_revealed: bool, // ...and it's revealed -> waterskins refill here
    pub has_game: bool,        // a Game Animal resource exists here (maybe hidden)
    pub game_revealed: bool,   // ...and it's revealed -> huntable with a Hunting tool
    pub has_plant: bool,       // a Plant resource exists here (maybe hidden)
    pub plant_revealed: bool,  // ...and it's revealed -> villagers gather it tool-free
}

fn to_item_view(item: &crate::item::Item) -> ItemView {
    ItemView {
        id: item.id,
        name: item.name.clone(),
        class: item.class.clone(),
        subclass: item.subclass.clone(),
        quantity: item.quantity,
        equipped: item.equipped,
        is_healing: item.attrs.contains_key(&AttrKey::Healing),
        is_weapon: item.class == "Weapon",
        is_hunting: item.attrs.contains_key(&AttrKey::Hunting),
        feed: match item.attrs.get(&AttrKey::Feed) {
            Some(AttrVal::Num(v)) => *v,
            _ => 0.0,
        },
        food_poisoning: item.attrs.contains_key(&AttrKey::FoodPoisoning),
    }
}

// Per-run metrics emitted by the runner (CSV + JSON). Field names mirror the
// game's own state structs (`PlayerRunScore`, `PlayerObjectives`,
// `PlayerVictory`) so the data lines up with in-game scoring.
#[derive(Debug, Clone, Serialize)]
pub struct RunMetrics {
    pub run_index: u32,
    pub outcome: String,
    pub killer: String, // StateDead.killer at end (e.g. "Starvation", a creature name, or "")
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

        // Hero + its inventory (same entity).
        let (hero, inventory) = {
            let mut q = world.query_filtered::<(
                &Id,
                &PlayerId,
                &Position,
                &Stats,
                &State,
                &Inventory,
                &Thirst,
                &Hunger,
                Option<&Tired>,
                Option<&TrueDeath>,
            ), With<SubclassHero>>();
            match q.iter(world).find(|(_, p, ..)| p.0 == pid) {
                Some((id, _p, pos, stats, state, inv, thirst, hunger, tired, td)) => (
                    Some(HeroView {
                        id: id.0,
                        pos: *pos,
                        hp: stats.hp,
                        base_hp: stats.base_hp,
                        state: *state,
                        dead: *state == State::Dead,
                        true_death: td.is_some(),
                        thirst: thirst.thirst,
                        hunger: hunger.hunger,
                        tired: tired.map(|t| t.tired).unwrap_or(0.0),
                    }),
                    inv.items.iter().map(to_item_view).collect::<Vec<_>>(),
                ),
                None => (None, Vec::new()),
            }
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

        // The player's villagers, with whether they are idle (Order::None),
        // whether they hold a gather order / are actively gathering, and how much
        // food they are carrying (to diagnose the forage->haul->larder loop).
        let villagers = {
            let mut q = world.query_filtered::<(&Id, &PlayerId, &Position, &State, &Order, &Inventory), With<SubclassVillager>>();
            q.iter(world)
                .filter(|(_, p, _, state, _, _)| p.0 == pid && **state != State::Dead)
                .map(|(id, _p, pos, state, order, inv)| VillagerView {
                    id: id.0,
                    pos: *pos,
                    idle: *order == Order::None,
                    gathering_order: matches!(order, Order::Gather { .. }),
                    gathering_now: *state == State::Gathering,
                    food_carried: inv.get_total_weight_by_class(FOOD.to_string()),
                })
                .collect::<Vec<_>>()
        };

        // Points of interest (e.g. the Shipwreck — investigating it recruits the
        // first villager).
        let pois = {
            let mut q = world.query::<(&Id, &Position, &Subclass, &Template)>();
            q.iter(world)
                .filter(|(_, _, subclass, _)| **subclass == Subclass::Poi)
                .map(|(id, pos, _subclass, template)| PoiView {
                    id: id.0,
                    pos: *pos,
                    template: template.0.clone(),
                })
                .collect::<Vec<_>>()
        };

        // The travelling merchant (if any). Its `Transport.hauling` are the
        // villagers available to hire; `sail_state` says whether it's docked.
        let merchant = {
            let mut q = world.query::<(&Id, &Position, &Merchant, &Transport)>();
            q.iter(world)
                .next()
                .map(|(id, pos, merchant, transport)| MerchantView {
                    id: id.0,
                    pos: *pos,
                    at_landing: merchant.sail_state == MerchantSailState::AtLanding,
                    hireable: transport.hauling.clone(),
                })
        };

        // The player's structures (+ inventories so the bot can pull build
        // resources from the Burrow and check foundation contents).
        let structures = {
            let mut q = world.query_filtered::<(&Id, &PlayerId, &Position, &Subclass, &State, &Inventory), With<ClassStructure>>();
            q.iter(world)
                .filter(|(_, p, _, _, _, _)| p.0 == pid)
                .map(|(id, _p, pos, subclass, state, inv)| StructureView {
                    id: id.0,
                    pos: *pos,
                    subclass: subclass.to_string(),
                    founded: *state == State::Founded,
                    building: *state == State::Building || *state == State::Progressing,
                    built: !matches!(
                        *state,
                        State::Founded | State::Building | State::Progressing | State::Stalled
                    ),
                    inventory: inv.items.iter().map(to_item_view).collect(),
                })
                .collect::<Vec<_>>()
        };

        // Every positioned object's tile (move-target occupancy avoidance).
        let occupied = {
            let mut q = world.query::<&Position>();
            q.iter(world).map(|p| (p.x, p.y)).collect::<HashSet<_>>()
        };

        // Resource node tiles (the `Resources` map is keyed by Position). Track
        // both general reveal state and spring-water specifically (the bot
        // prospects a tile to reveal a spring, then refills waterskins there).
        let resource_tiles = world
            .resource::<Resources>()
            .iter()
            .map(|(pos, res_on_tile)| {
                let (has_spring, spring_revealed) = res_on_tile
                    .values()
                    .filter(|r| r.res_type == SPRING_WATER)
                    .fold((false, false), |acc, r| (true, acc.1 || r.reveal));
                let (has_game, game_revealed) = res_on_tile
                    .values()
                    .filter(|r| r.res_type == GAME_ANIMAL)
                    .fold((false, false), |acc, r| (true, acc.1 || r.reveal));
                let (has_plant, plant_revealed) = res_on_tile
                    .values()
                    .filter(|r| r.res_type == PLANT)
                    .fold((false, false), |acc, r| (true, acc.1 || r.reveal));
                ResTileView {
                    pos: *pos,
                    revealed: res_on_tile.values().any(|r| r.reveal),
                    has_spring,
                    spring_revealed,
                    has_game,
                    game_revealed,
                    has_plant,
                    plant_revealed,
                }
            })
            .collect::<Vec<_>>();

        WorldView {
            hero,
            inventory,
            enemies,
            villagers,
            pois,
            merchant,
            structures,
            resource_tiles,
            occupied,
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
        let (final_hp, final_skill_total, final_inventory_count, hero_true_death, hero_present, killer) = {
            let mut q = world.query_filtered::<(
                &PlayerId,
                &Stats,
                &Skills,
                &Inventory,
                Option<&TrueDeath>,
                Option<&StateDead>,
            ), With<SubclassHero>>();
            match q.iter(world).find(|(p, ..)| p.0 == pid) {
                Some((_p, stats, skills, inv, td, dead)) => (
                    stats.hp,
                    skills.get_total_xp(),
                    inv.items.len() as i32,
                    td.is_some(),
                    true,
                    dead.map(|d| d.killer.clone()).unwrap_or_default(),
                ),
                None => (0, 0, 0, true, false, String::new()),
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
            killer,
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

    // The merchant hire mechanic, end-to-end and isolated from the bot's survival:
    // dock the merchant on the hero's tile, give the hero gold, send Hire, and
    // confirm a player-owned villager appears and the wage is deducted.
    #[test]
    fn hire_from_merchant_adds_villager() {
        use crate::ids::Ids;
        use crate::templates::Templates;

        let mut game = HeadlessGame::new(5_000);
        let pid = game.spawn_hero("Warrior", "HireBot");
        game.tick(50); // let new_player setup spawn the merchant + cargo villagers

        // Hero position (the merchant will dock here so the adjacency check passes).
        let hero_pos = {
            let world = game.app.world_mut();
            let mut q = world.query_filtered::<&Position, With<SubclassHero>>();
            *q.iter(world).next().expect("hero exists")
        };

        // Give the hero 50 Gold Coins to pay the wage from.
        {
            let world = game.app.world_mut();
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut q = world.query_filtered::<&mut Inventory, With<SubclassHero>>();
            let mut hero_inv = q.iter_mut(world).next().expect("hero inventory");
            hero_inv.new(item_id, "Gold Coins".to_string(), 50, &item_templates);
        }
        let gold_before: i32 = game
            .observe()
            .inventory
            .iter()
            .filter(|i| i.class == "Gold Coins")
            .map(|i| i.quantity)
            .sum();
        assert_eq!(gold_before, 50, "hero should be carrying the gold we added");

        // Dock the merchant on the hero's tile and grab a cargo villager to hire.
        let (merchant_id, target_id) = {
            let world = game.app.world_mut();
            let mut q = world.query::<(&Id, &mut Position, &mut Merchant, &Transport)>();
            let (id, mut pos, mut merchant, transport) =
                q.iter_mut(world).next().expect("merchant exists");
            *pos = hero_pos;
            merchant.sail_state = MerchantSailState::AtLanding;
            let target = *transport
                .hauling
                .first()
                .expect("merchant carries cargo villagers");
            (id.0, target)
        };

        assert_eq!(
            game.observe().villagers.len(),
            0,
            "player has no villager before hiring"
        );

        game.inject(PlayerEvent::Hire {
            player_id: pid,
            merchant_id,
            target_id,
        });
        game.tick(20);

        assert_eq!(
            game.observe().villagers.len(),
            1,
            "hiring should give the player one villager"
        );
        let gold_after: i32 = game
            .observe()
            .inventory
            .iter()
            .filter(|i| i.class == "Gold Coins")
            .map(|i| i.quantity)
            .sum();
        assert_eq!(gold_after, 25, "the wage (25) should be deducted from the hero");
    }

    // Empowering the Monolith sanctuary: pay Soulshards, level rises, and the
    // suppression radius grows with it.
    #[test]
    fn upgrade_sanctuary_raises_level_and_radius() {
        use crate::game::{sanctuary_full_radius, Monolith, SANCTUARY_UPGRADE_COST};
        use crate::ids::Ids;
        use crate::templates::Templates;

        let mut game = HeadlessGame::new(5_000);
        let pid = game.spawn_hero("Warrior", "SancBot");
        game.tick(50);

        // Find the Monolith and confirm it starts at sanctuary level 0.
        let (monolith_id, monolith_pos, level0) = {
            let world = game.app.world_mut();
            let mut q = world.query::<(&Id, &Position, &Monolith)>();
            let (id, pos, m) = q.iter(world).next().expect("a monolith exists");
            (id.0, *pos, m.sanctuary_level)
        };
        assert_eq!(level0, 0, "sanctuary starts at level 0");

        // Stand the hero on the Monolith and give them enough Soulshards.
        {
            let world = game.app.world_mut();
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut q =
                world.query_filtered::<(&mut Position, &mut Inventory), With<SubclassHero>>();
            let (mut hpos, mut hinv) = q.iter_mut(world).next().expect("hero");
            *hpos = monolith_pos;
            hinv.new(
                item_id,
                "Soulshard".to_string(),
                SANCTUARY_UPGRADE_COST + 1,
                &item_templates,
            );
        }

        game.inject(PlayerEvent::UpgradeSanctuary {
            player_id: pid,
            monolith_id,
        });
        game.tick(20);

        let level1 = {
            let world = game.app.world_mut();
            let mut q = world.query::<&Monolith>();
            q.iter(world).next().expect("monolith").sanctuary_level
        };
        assert_eq!(level1, 1, "upgrade should raise the sanctuary level");
        assert!(
            sanctuary_full_radius(level1) > sanctuary_full_radius(level0),
            "the suppression radius should grow with level"
        );

        let shards: i32 = game
            .observe()
            .inventory
            .iter()
            .filter(|i| i.class == "Soulshard")
            .map(|i| i.quantity)
            .sum();
        assert_eq!(shards, 1, "the upgrade cost should be deducted in Soulshards");
    }
}
