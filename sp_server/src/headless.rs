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

use crate::common::Transport;
use crate::common::{Hunger, Thirst, Tired};
use crate::constants::{
    DATABASE_MANAGER_ID, FOOD, GAME_ANIMAL, GAME_TICKS_PER_DAY, PLANT, SPRING_WATER,
};
use crate::database::DatabaseEvent;
use crate::game::{
    Client, Clients, CrisisAssaultUnit, DatabaseClient, DatabaseManagers, GameTick, Merchant,
    MerchantSailState, Monolith, NetworkReceiver, Objectives, PlayerIntroEncounters,
    PlayerObjectives, PlayerRunScore, PlayerStats, PlayerVictory, RunScoreState, SettlementCrisis,
    SettlementCrisisState, SurvivalDirectorMode, VictoryState,
};
use crate::item::{AttrKey, AttrVal, Inventory};
use crate::map::Map;
use crate::obj::{
    ClassStructure, Id, Order, PlayerId, Position, State, StateDead, Stats, Subclass, SubclassHero,
    SubclassNPC, SubclassVillager, Template, TrueDeath,
};
use crate::resource::Resources;
use crate::skill::Skills;
use crate::{build_headless_app_with_director, AppState, PlayerEvent, ResponsePacket};

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
    pub monolith: Option<MonolithView>,
    pub corpses: Vec<CorpseView>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrisisAssaultUnitView {
    pub obj_id: i32,
    pub template: String,
    pub owner_player_id: i32,
    pub assault_id: u64,
    pub spawn_generation: u32,
    pub dead: bool,
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

#[derive(Clone, Copy)]
pub struct MonolithView {
    pub id: i32,
    pub pos: Position,
    /// Current sanctuary level (0 = innate); upgraded with Soulshards.
    pub level: i32,
}

#[derive(Clone, Copy)]
pub struct CorpseView {
    pub id: i32,
    pub pos: Position,
    /// Item id of a Soulshard stack sitting on this corpse, ready to loot.
    pub soulshard_item: i32,
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
        Self::new_with_director(max_ticks, SurvivalDirectorMode::PersonalCrisis)
    }

    pub fn new_with_director(max_ticks: i32, survival_director_mode: SurvivalDirectorMode) -> Self {
        let mut app = build_headless_app_with_director(survival_director_mode);

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
        database_managers
            .lock()
            .unwrap()
            .insert(DATABASE_MANAGER_ID, DatabaseClient { sender: db_tx });

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

        // Experiment hook: SANCTUARY_LEVEL re-homes the nearest Monolith onto the
        // hero's base and sets its level, so we can A/B "how much does a stronger
        // sanctuary extend survival?" without first wiring the bot to earn/spend
        // Soulshards. Unset = stock behaviour.
        if let Ok(level) = std::env::var("SANCTUARY_LEVEL") {
            if let Ok(level) = level.parse::<i32>() {
                self.set_sanctuary_at_base(level);
            }
        }

        pid
    }

    // Move the nearest Monolith onto the hero's tile and set its sanctuary level.
    // Test/experiment use only (see the SANCTUARY_LEVEL hook in spawn_hero).
    fn set_sanctuary_at_base(&mut self, level: i32) {
        let world = self.app.world_mut();
        let hero_pos = {
            let mut q = world.query_filtered::<&Position, With<SubclassHero>>();
            match q.iter(world).next() {
                Some(p) => *p,
                None => return,
            }
        };
        let mut nearest: Option<(Entity, u32)> = None;
        {
            let mut q = world.query_filtered::<(Entity, &Position), With<Monolith>>();
            for (e, p) in q.iter(world) {
                let d = Map::distance((hero_pos.x, hero_pos.y), (p.x, p.y));
                if nearest.map_or(true, |(_, bd)| d < bd) {
                    nearest = Some((e, d));
                }
            }
        }
        if let Some((entity, _)) = nearest {
            if let Some(mut pos) = world.get_mut::<Position>(entity) {
                *pos = hero_pos;
            }
            if let Some(mut monolith) = world.get_mut::<Monolith>(entity) {
                monolith.sanctuary_level = level;
            }
        }
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

    /// Removes every active test connection for this player while leaving the
    /// hero entity in the ECS, matching production disconnect semantics.
    pub fn disconnect_player(&mut self) {
        let player_id = self.player_id;
        self.clients
            .lock()
            .unwrap()
            .retain(|_, client| client.player_id != player_id);
    }

    /// Re-adds the harness's deterministic active client without recreating the
    /// hero or resetting run state.
    pub fn reconnect_player(&mut self) {
        let client = Client {
            id: Uuid::from_u128(self.player_id as u128),
            player_id: self.player_id,
            sender: self.packet_tx.clone(),
        };
        self.clients.lock().unwrap().insert(client.id, client);
    }

    pub fn settlement_crisis(&self) -> Option<SettlementCrisis> {
        self.app
            .world()
            .resource::<SettlementCrisisState>()
            .get(&self.player_id)
            .cloned()
    }

    pub fn crisis_assault_units(&mut self) -> Vec<CrisisAssaultUnitView> {
        let world = self.app.world_mut();
        let mut query = world.query::<(&Id, &Template, &CrisisAssaultUnit, Option<&StateDead>)>();
        let mut units = query
            .iter(world)
            .map(|(id, template, assault, dead)| CrisisAssaultUnitView {
                obj_id: id.0,
                template: template.0.clone(),
                owner_player_id: assault.owner_player_id,
                assault_id: assault.assault_id,
                spawn_generation: assault.spawn_generation,
                dead: dead.is_some(),
            })
            .collect::<Vec<_>>();
        units.sort_by_key(|unit| unit.obj_id);
        units
    }

    pub fn personal_crises_resolved(&self) -> i32 {
        self.app
            .world()
            .resource::<RunScoreState>()
            .get(&self.player_id)
            .map(|score| score.personal_crises_resolved)
            .unwrap_or(0)
    }

    pub fn intro_encounters(&self) -> Option<PlayerIntroEncounters> {
        self.app
            .world()
            .resource::<crate::game::IntroEncounterState>()
            .get(&self.player_id)
            .cloned()
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
            let mut q =
                world.query_filtered::<(&Id, &PlayerId, &Position, &State), With<SubclassNPC>>();
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

        // The Monolith nearest the hero — the sanctuary the bot empowers.
        let hero_pos_opt = hero.as_ref().map(|h| h.pos);
        let monolith = {
            let mut q = world.query::<(&Id, &Position, &Monolith)>();
            let mut best: Option<MonolithView> = None;
            let mut best_d = u32::MAX;
            for (id, pos, m) in q.iter(world) {
                let d = hero_pos_opt
                    .map(|hp| Map::distance((hp.x, hp.y), (pos.x, pos.y)))
                    .unwrap_or(0);
                if d < best_d {
                    best_d = d;
                    best = Some(MonolithView {
                        id: id.0,
                        pos: *pos,
                        level: m.sanctuary_level,
                    });
                }
            }
            best
        };

        // Enemy corpses still holding a Soulshard, ready to loot for upgrades.
        let corpses = {
            let mut q = world.query::<(&Id, &PlayerId, &Position, &State, &Inventory)>();
            q.iter(world)
                .filter(|(_, p, _, state, _)| p.0 != pid && **state == State::Dead)
                .filter_map(|(id, _p, pos, _state, inv)| {
                    inv.items
                        .iter()
                        .find(|i| i.class == "Soulshard")
                        .map(|item| CorpseView {
                            id: id.0,
                            pos: *pos,
                            soulshard_item: item.id,
                        })
                })
                .collect::<Vec<_>>()
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
            monolith,
            corpses,
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
        let mut q = world.query_filtered::<(&PlayerId, Option<&TrueDeath>), With<SubclassHero>>();
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
        let (
            final_hp,
            final_skill_total,
            final_inventory_count,
            hero_true_death,
            hero_present,
            killer,
        ) = {
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

    fn prepare_for_scheduled_dusk(game: &mut HeadlessGame) {
        prepare_for_scheduled_dusk_on_day(game, 3);
    }

    fn prepare_for_scheduled_dusk_on_day(game: &mut HeadlessGame, day_offset: i32) {
        use crate::constants::DUSK;
        use crate::game::PlayerIntroState;

        let dusk = DUSK + (GAME_TICKS_PER_DAY * day_offset);
        let player_id = game.player_id();
        let world = game.app.world_mut();
        {
            let mut intro_state = world.resource_mut::<PlayerIntroState>();
            let intro = intro_state
                .get_mut(&player_id)
                .expect("headless player intro state");
            intro.start_tick = dusk - 4_801;
            intro.danger_unlocked = true;
        }
        world.resource_mut::<GameTick>().0 = dusk - 2;
    }

    fn cross_scheduled_dusk(game: &mut HeadlessGame) -> i32 {
        game.tick(5);
        game.metrics().waves_survived
    }

    fn run_intro_check_at_or_after(game: &mut HeadlessGame, due_tick: i32) {
        let check_tick = ((due_tick + 9) / 10) * 10;
        game.app.world_mut().resource_mut::<GameTick>().0 = check_tick - 2;
        game.tick(15);
    }

    fn mark_obj_ids_dead(game: &mut HeadlessGame, obj_ids: &[i32], dead_at: i32) {
        let entities = {
            let world = game.app.world_mut();
            let mut query = world.query::<(Entity, &Id)>();
            query
                .iter(world)
                .filter(|(_, id)| obj_ids.contains(&id.0))
                .map(|(entity, _)| entity)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            entities.len(),
            obj_ids.len(),
            "every scripted encounter object should exist before being defeated"
        );
        for entity in entities {
            game.app.world_mut().entity_mut(entity).insert((
                State::Dead,
                StateDead {
                    dead_at,
                    killer: "Headless checkpoint test".to_string(),
                },
            ));
        }
    }

    fn next_preferred_assault_tick(current_tick: i32) -> i32 {
        use crate::constants::DUSK;
        use crate::game::ASSAULT_READY_GRACE_TICKS;

        let mut dusk = current_tick.div_euclid(GAME_TICKS_PER_DAY) * GAME_TICKS_PER_DAY + DUSK;
        while dusk - ASSAULT_READY_GRACE_TICKS <= current_tick {
            dusk += GAME_TICKS_PER_DAY;
        }
        dusk
    }

    fn set_personal_assault_ready(game: &mut HeadlessGame) -> i32 {
        use crate::game::{
            CrisisKind, CrisisPhase, InitialEncounterState, PlayerIntroState,
            ASSAULT_READY_GRACE_TICKS,
        };

        let player_id = game.player_id();
        let preferred_tick = next_preferred_assault_tick(game.game_tick());
        let ready_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;
        let world = game.app.world_mut();
        world.resource_mut::<GameTick>().0 = ready_tick;
        world
            .resource_mut::<PlayerIntroState>()
            .get_mut(&player_id)
            .expect("player intro state")
            .danger_unlocked = true;
        world
            .resource_mut::<InitialEncounterState>()
            .remove(&player_id);
        world.resource_mut::<SettlementCrisisState>().insert(
            player_id,
            SettlementCrisis {
                kind: CrisisKind::Goblin,
                phase: CrisisPhase::AssaultReady,
                pressure: 100,
                phase_started_tick: ready_tick,
                online_active_ticks: 10_000,
                phase_online_ticks: 0,
                warning_active: true,
                last_evaluated_tick: ready_tick,
                ..SettlementCrisis::default()
            },
        );
        preferred_tick
    }

    fn advance_ready_clock_to_launch(game: &mut HeadlessGame, preferred_tick: i32) {
        use crate::game::CrisisPhase;

        game.app.world_mut().resource_mut::<GameTick>().0 = preferred_tick - 2;
        game.tick(1);
        let before = game.settlement_crisis().expect("ready crisis");
        assert_eq!(before.phase, CrisisPhase::AssaultReady);
        assert!(game.crisis_assault_units().is_empty());

        game.tick(1);
        assert_eq!(
            game.settlement_crisis().expect("launched crisis").phase,
            CrisisPhase::AssaultActive
        );
    }

    fn set_reconnect_ready_clock(game: &mut HeadlessGame) -> i32 {
        use crate::game::ASSAULT_READY_GRACE_TICKS;

        let preferred_tick = next_preferred_assault_tick(game.game_tick());
        let ready_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;
        let world = game.app.world_mut();
        world.resource_mut::<GameTick>().0 = ready_tick;
        let mut crises = world.resource_mut::<SettlementCrisisState>();
        let crisis = crises
            .get_mut(&game.player_id)
            .expect("retry crisis remains ready");
        crisis.phase_online_ticks = 0;
        crisis.last_evaluated_tick = ready_tick;
        preferred_tick
    }

    fn kill_assault_unit_through_normal_combat(
        game: &mut HeadlessGame,
        target_id: i32,
    ) -> Vec<(String, i32)> {
        use crate::constants::ATTACK_COOLDOWN_TICKS;
        use crate::ids::EntityObjMap;

        let owner_player_id = game.player_id();
        let (hero_entity, hero_id, hero_pos, target_entity, loot_before) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(Entity, &Id, &PlayerId, &Position), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_pos) = hero_query
                .iter(world)
                .find(|(_, _, owner, _)| owner.0 == owner_player_id)
                .map(|(entity, id, _, pos)| (entity, id.0, *pos))
                .expect("headless hero");
            let target_entity = world
                .resource::<EntityObjMap>()
                .get_entity(target_id)
                .expect("assault target entity");
            let loot_before = world
                .get::<Inventory>(target_entity)
                .expect("assault inventory")
                .items
                .iter()
                .map(|item| (item.name.clone(), item.quantity))
                .collect::<Vec<_>>();
            (hero_entity, hero_id, hero_pos, target_entity, loot_before)
        };

        {
            let world = game.app.world_mut();
            *world
                .get_mut::<Position>(target_entity)
                .expect("target position") = hero_pos;
            world
                .get_mut::<Stats>(target_entity)
                .expect("target stats")
                .hp = 0;
            *world.get_mut::<State>(hero_entity).expect("hero state") = State::None;
            world
                .get_mut::<Stats>(hero_entity)
                .expect("hero stats")
                .stamina = Some(100);
            world.resource_mut::<GameTick>().0 += ATTACK_COOLDOWN_TICKS + 1;
        }

        game.inject(PlayerEvent::Attack {
            player_id: game.player_id(),
            attack_type: "quick".to_string(),
            source_id: hero_id,
            target_id,
        });
        game.tick(3);

        let world = game.app.world();
        assert!(
            world.get::<StateDead>(target_entity).is_some(),
            "real PlayerEvent::Attack should produce StateDead"
        );
        assert_eq!(
            world
                .get::<Inventory>(target_entity)
                .expect("normal corpse retains inventory")
                .items
                .iter()
                .map(|item| (item.name.clone(), item.quantity))
                .collect::<Vec<_>>(),
            loot_before,
            "normal combat leaves pre-generated ordinary loot on the corpse"
        );
        loot_before
    }

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

    #[test]
    fn personal_crisis_mode_does_not_spawn_a_scheduled_dusk_horde() {
        let mut game = HeadlessGame::new(10_000);
        game.spawn_hero("Warrior", "PersonalDuskBot");
        prepare_for_scheduled_dusk(&mut game);

        assert_eq!(cross_scheduled_dusk(&mut game), 0);
    }

    #[test]
    fn legacy_mode_still_runs_the_scheduled_dusk_horde() {
        let mut game = HeadlessGame::new_with_director(10_000, SurvivalDirectorMode::Legacy);
        game.spawn_hero("Warrior", "LegacyDuskBot");

        // Legacy spawn placement samples wilderness outside the sanctuary. Try
        // several distinct dusks so random selection of blocked edge tiles
        // cannot make the director-registration regression flaky.
        let mut waves = 0;
        for day_offset in 3..=10 {
            prepare_for_scheduled_dusk_on_day(&mut game, day_offset);
            waves = cross_scheduled_dusk(&mut game);
            if waves > 0 {
                break;
            }
        }

        assert!(
            waves > 0,
            "legacy nightly_threat_system should schedule its dusk wave"
        );
        assert!(
            game.settlement_crisis().is_none(),
            "legacy mode must not activate the personal crisis state machine"
        );
    }

    #[test]
    fn personal_crisis_mode_preserves_the_introductory_encounter() {
        use crate::event::{GameEventType, GameEvents};
        use crate::game::{InitialEncounterState, IntroEncounterState, PlayerIntroState};

        let mut game = HeadlessGame::new(10_000);
        let player_id = game.spawn_hero("Warrior", "IntroBot");
        let entry = game
            .world()
            .resource::<InitialEncounterState>()
            .get(&player_id)
            .expect("initial encounter state")
            .clone();

        game.app
            .world_mut()
            .resource_mut::<Objectives>()
            .entry(player_id)
            .or_default()
            .scavenge_shipwreck = true;

        run_intro_check_at_or_after(&mut game, entry.second_rat_spawn_tick);

        assert!(
            game.world()
                .resource::<crate::game::PlayerIntroState>()
                .get(&player_id)
                .expect("player intro state")
                .shipwreck_chain_started,
            "the delayed opening encounter should start in personal-crisis mode"
        );
        let opening_deadline = entry.phase1_unlock_tick;
        mark_obj_ids_dead(&mut game, &entry.rat_ids, opening_deadline);
        run_intro_check_at_or_after(&mut game, opening_deadline);

        let phase1_id = game
            .world()
            .resource::<InitialEncounterState>()
            .get(&player_id)
            .and_then(|state| state.phase1_npc_id)
            .expect("boar/crab follow-up should spawn after opening enemies die");
        assert!(
            game.world()
                .resource::<IntroEncounterState>()
                .get(&player_id)
                .expect("separate intro encounter progress")
                .initial_encounter
        );

        mark_obj_ids_dead(&mut game, &[phase1_id], entry.spider_unlock_tick);
        run_intro_check_at_or_after(&mut game, entry.spider_unlock_tick);

        assert!(
            game.world()
                .resource::<IntroEncounterState>()
                .get(&player_id)
                .expect("separate intro encounter progress")
                .spider_encounter
        );
        let spider_exists = {
            let world = game.app.world_mut();
            let mut query = world.query::<(&Template, &Position, Option<&StateDead>)>();
            query.iter(world).any(|(template, pos, dead)| {
                template.0 == "Spider" && *pos == entry.spawn_pos && dead.is_none()
            })
        };
        assert!(
            spider_exists,
            "the Spider follow-up should be alive at the wreck"
        );

        assert!(
            game.world()
                .resource::<PlayerIntroState>()
                .get(&player_id)
                .expect("player intro state")
                .villager_spawned,
            "shipwreck inspection should still rescue the villager"
        );
        let villager_exists = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(&PlayerId, &State), With<SubclassVillager>>();
            query
                .iter(world)
                .any(|(owner, state)| owner.0 == player_id && state.is_alive())
        };
        assert!(villager_exists);
        assert!(game
            .world()
            .resource::<GameEvents>()
            .values()
            .any(|event| matches!(event.event_type, GameEventType::NecroEvent { .. })));

        assert!(matches!(
            entry.phase1_spawn.as_str(),
            "Wild Boar" | "Giant Crab"
        ));
        assert!(entry.phase1_unlock_tick < entry.spider_unlock_tick);
    }

    #[test]
    fn true_death_clears_intro_and_personal_crisis_before_a_fresh_run() {
        use crate::game::{
            CrisisKind, CrisisPhase, InitialEncounterState, IntroEncounterState,
            PlayerIntroEncounters, PlayerIntroState, SettlementCrisis, SettlementCrisisState,
        };

        let mut game = HeadlessGame::new(10_000);
        let player_id = game.spawn_hero("Warrior", "FirstRunBot");
        let neighbor_id = player_id + 1;
        let current_tick = game.game_tick();

        {
            let world = game.app.world_mut();
            let mut intro = world.resource_mut::<IntroEncounterState>();
            intro.insert(
                player_id,
                PlayerIntroEncounters {
                    initial_encounter: true,
                    spider_encounter: true,
                },
            );
            intro.insert(
                neighbor_id,
                PlayerIntroEncounters {
                    initial_encounter: true,
                    spider_encounter: false,
                },
            );
        }
        game.app
            .world_mut()
            .resource_mut::<SettlementCrisisState>()
            .insert(
                neighbor_id,
                SettlementCrisis {
                    kind: CrisisKind::Goblin,
                    phase: CrisisPhase::Signs,
                    pressure: 40,
                    phase_started_tick: current_tick,
                    online_active_ticks: 50,
                    phase_online_ticks: 50,
                    warning_active: false,
                    last_evaluated_tick: current_tick,
                    ..SettlementCrisis::default()
                },
            );

        let hero = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(entity, _)| entity)
                .expect("first-run hero")
        };
        game.app.world_mut().entity_mut(hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Checkpoint cleanup".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick - (10 * crate::constants::TICKS_PER_SEC) - 1,
            },
        ));

        game.tick(3);
        assert!(game
            .world()
            .resource::<IntroEncounterState>()
            .get(&player_id)
            .is_none());
        assert!(game
            .world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .is_none());
        assert!(game
            .world()
            .resource::<PlayerIntroState>()
            .get(&player_id)
            .is_none());
        assert!(game
            .world()
            .resource::<InitialEncounterState>()
            .get(&player_id)
            .is_none());
        assert!(game
            .world()
            .resource::<IntroEncounterState>()
            .contains_key(&neighbor_id));
        assert!(game
            .world()
            .resource::<SettlementCrisisState>()
            .contains_key(&neighbor_id));

        // Model an attributed orphan whose entity-map entry was already removed
        // by an overlapping cleanup. Fresh-run creation must still sweep it.
        let (stale_assault_entity, stale_assault_id) = {
            use crate::ids::Ids;

            let world = game.app.world_mut();
            let stale_id = world.resource_mut::<Ids>().new_obj_id();
            world
                .resource_mut::<Ids>()
                .new_obj(stale_id, crate::constants::NPC_PLAYER_ID);
            let entity = world
                .spawn((
                    Id(stale_id),
                    crate::game::CrisisAssaultUnit {
                        owner_player_id: player_id,
                        assault_id: 99,
                        spawn_generation: 1,
                    },
                ))
                .id();
            (entity, stale_id)
        };

        game.spawn_hero("Warrior", "FreshRunBot");
        assert!(game.world().get_entity(stale_assault_entity).is_err());
        assert!(!game
            .world()
            .resource::<crate::ids::Ids>()
            .obj_player_map
            .contains_key(&stale_assault_id));
        assert_eq!(
            game.intro_encounters(),
            Some(PlayerIntroEncounters::default())
        );
        let fresh_crisis = game
            .settlement_crisis()
            .expect("fresh run should lazily initialize a personal crisis");
        assert_eq!(fresh_crisis.phase, CrisisPhase::Dormant);
        assert_eq!(fresh_crisis.pressure, 0);
        assert_eq!(fresh_crisis.online_active_ticks, 0);
        assert_eq!(fresh_crisis.assault_id, None);
        assert!(fresh_crisis.assault_unit_ids.is_empty());
        assert!(fresh_crisis.assault_remaining_templates.is_empty());
        assert_eq!(fresh_crisis.assault_spawn_generation, 0);
        assert_eq!(fresh_crisis.assault_retry_count, 0);
        assert!(!fresh_crisis.resolution_recorded);
    }

    #[test]
    fn intro_encounter_does_not_queue_spawns_for_a_true_death_owner() {
        use crate::game::InitialEncounterState;

        let mut game = HeadlessGame::new(10_000);
        let player_id = game.spawn_hero("Warrior", "IntroCleanupRaceBot");
        let current_tick = game.game_tick();
        let opening_ids = {
            let mut encounters = game.app.world_mut().resource_mut::<InitialEncounterState>();
            let encounter = encounters
                .get_mut(&player_id)
                .expect("initial encounter state");
            encounter.first_rat_spawn_tick = current_tick;
            encounter.second_rat_spawn_tick = current_tick;
            encounter.phase1_unlock_tick = current_tick;
            encounter.rat_ids.clone()
        };

        let hero = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(entity, _)| entity)
                .expect("hero")
        };
        game.app.world_mut().entity_mut(hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Checkpoint cleanup race".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick,
            },
        ));

        // Cross at least one 10-tick intro evaluation while remaining inside
        // the True Death system's cleanup delay.
        game.tick(20);

        let spawned_opening_enemy = {
            let world = game.app.world_mut();
            let mut query = world.query::<&Id>();
            query.iter(world).any(|id| opening_ids.contains(&id.0))
        };
        assert!(
            !spawned_opening_enemy,
            "deferred intro spawns must not escape a dying run's cleanup"
        );
    }

    #[test]
    fn headless_connection_helpers_leave_the_offline_hero_in_the_world() {
        let mut game = HeadlessGame::new(1_000);
        let player_id = game.spawn_hero("Warrior", "PresenceBot");
        assert!(game
            .world()
            .resource::<Clients>()
            .is_player_online(player_id));

        game.disconnect_player();
        assert!(!game
            .world()
            .resource::<Clients>()
            .is_player_online(player_id));
        assert!(
            game.observe().hero.is_some(),
            "hero existence must not be treated as online presence"
        );

        game.reconnect_player();
        assert!(game
            .world()
            .resource::<Clients>()
            .is_player_online(player_id));
    }

    #[test]
    fn checkpoint2_short_personal_crisis_simulation() {
        use crate::constants::TICKS_PER_SEC;
        use crate::game::{
            CrisisPhase, InitialEncounterState, PlayerIntroState, SettlementCrisisState,
        };

        let mut game = HeadlessGame::new(10_000);
        let player_id = game.spawn_hero("Warrior", "CrisisFoundationBot");
        game.set_sanctuary_at_base(5);

        {
            let world = game.app.world_mut();
            world
                .resource_mut::<PlayerIntroState>()
                .get_mut(&player_id)
                .expect("player intro state")
                .danger_unlocked = true;
            {
                let mut objectives = world.resource_mut::<Objectives>();
                let objective = objectives.entry(player_id).or_default();
                objective.explore_poi = true;
                objective.choose_expansion = true;
            }
            world
                .resource_mut::<InitialEncounterState>()
                .remove(&player_id);

            // Existing-world facts only: completed owned structures and a
            // living recruited villager. These test entities carry exactly the
            // components read by the crisis evaluator.
            for _ in 0..3 {
                world.spawn((PlayerId(player_id), State::None, ClassStructure));
            }
            world.spawn((PlayerId(player_id), State::None, SubclassVillager));
        }

        game.tick(1);
        assert_eq!(game.settlement_crisis().unwrap().phase, CrisisPhase::Signs);

        let next_tick = game.game_tick() + (60 * TICKS_PER_SEC);
        game.app.world_mut().resource_mut::<GameTick>().0 = next_tick;
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Pressure
        );

        let next_tick = game.game_tick() + (120 * TICKS_PER_SEC);
        game.app.world_mut().resource_mut::<GameTick>().0 = next_tick;
        game.tick(1);
        let preparing = game.settlement_crisis().unwrap();
        assert_eq!(preparing.phase, CrisisPhase::Preparing);
        assert!(preparing.warning_active);

        let enemies_before_ready = game
            .observe()
            .enemies
            .into_iter()
            .map(|enemy| enemy.id)
            .collect::<std::collections::HashSet<_>>();
        let next_tick = game.game_tick() + (180 * TICKS_PER_SEC);
        game.app.world_mut().resource_mut::<GameTick>().0 = next_tick;
        game.tick(1);
        let final_crisis = game
            .settlement_crisis()
            .expect("personal crisis after controlled simulation");
        let enemies_after_ready = game
            .observe()
            .enemies
            .into_iter()
            .map(|enemy| enemy.id)
            .collect::<std::collections::HashSet<_>>();
        let automatic_dusk_hordes = game.metrics().waves_survived;

        assert_eq!(final_crisis.phase, CrisisPhase::AssaultReady);
        assert_eq!(enemies_after_ready, enemies_before_ready);
        assert_eq!(automatic_dusk_hordes, 0);
        assert!(game
            .world()
            .resource::<SettlementCrisisState>()
            .contains_key(&player_id));

        println!(
            "checkpoint2_headless highest_phase={:?} online_active_ticks={} final_pressure={} major_assault_entities_spawned=0 automatic_dusk_hordes_spawned={}",
            final_crisis.phase,
            final_crisis.online_active_ticks,
            final_crisis.pressure,
            automatic_dusk_hordes
        );
    }

    #[test]
    fn checkpoint3_normal_victory_headless() {
        use crate::game::{CrisisPhase, GOBLIN_ASSAULT_COMPOSITION};
        use crate::ids::Ids;
        use crate::player_setup::RunSpawnedObjs;

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultVictoryBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);

        let launched = game.settlement_crisis().expect("active assault");
        let assault_id = launched.assault_id.expect("logical assault id");
        assert_eq!(launched.assault_spawn_generation, 1);
        assert_eq!(launched.assault_unit_ids.len(), 3);
        assert!(!launched.resolution_recorded);

        let units = game.crisis_assault_units();
        assert_eq!(units.len(), GOBLIN_ASSAULT_COMPOSITION.len());
        let mut actual_templates = units
            .iter()
            .map(|unit| unit.template.clone())
            .collect::<Vec<_>>();
        let mut expected_templates = GOBLIN_ASSAULT_COMPOSITION
            .iter()
            .map(|template| (*template).to_string())
            .collect::<Vec<_>>();
        actual_templates.sort();
        expected_templates.sort();
        assert_eq!(actual_templates, expected_templates);
        assert!(units.iter().all(|unit| {
            unit.owner_player_id == player_id
                && unit.assault_id == assault_id
                && unit.spawn_generation == 1
                && !unit.dead
        }));
        let run_ids = game
            .world()
            .resource::<RunSpawnedObjs>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        assert!(units.iter().all(|unit| run_ids.contains(&unit.obj_id)));

        // Neither an ordinary legacy goblin nor a normally dead unit attributed
        // to another owner can advance this settlement's logical remainder.
        let current_tick = game.game_tick();
        {
            let world = game.app.world_mut();
            let unrelated_id = world.resource_mut::<Ids>().new_obj_id();
            world.spawn((
                Id(unrelated_id),
                Template("Wolf Rider".to_string()),
                State::Dead,
                StateDead {
                    dead_at: current_tick,
                    killer: "Unrelated".to_string(),
                },
            ));
            let other_owner_id = world.resource_mut::<Ids>().new_obj_id();
            world.spawn((
                Id(other_owner_id),
                Template("Goblin Pillager".to_string()),
                State::Dead,
                StateDead {
                    dead_at: current_tick,
                    killer: "Other owner".to_string(),
                },
                CrisisAssaultUnit {
                    owner_player_id: player_id + 1,
                    assault_id: assault_id + 1,
                    spawn_generation: 1,
                },
            ));
        }
        game.tick(1);
        assert_eq!(
            game.settlement_crisis()
                .unwrap()
                .assault_remaining_templates
                .len(),
            GOBLIN_ASSAULT_COMPOSITION.len()
        );

        // Force two schedule evaluations to observe the exact same GameTick.
        // Active bookkeeping and generation identity must remain idempotent.
        let repeated_tick = game.game_tick() + 1;
        let launched_ids = game
            .crisis_assault_units()
            .iter()
            .filter(|unit| unit.owner_player_id == player_id)
            .map(|unit| unit.obj_id)
            .collect::<HashSet<_>>();
        for _ in 0..2 {
            game.app.world_mut().resource_mut::<GameTick>().0 = repeated_tick - 1;
            game.tick(1);
            assert_eq!(game.game_tick(), repeated_tick);
            assert_eq!(
                game.settlement_crisis().unwrap().assault_spawn_generation,
                1
            );
            assert_eq!(
                game.crisis_assault_units()
                    .iter()
                    .filter(|unit| unit.owner_player_id == player_id)
                    .map(|unit| unit.obj_id)
                    .collect::<HashSet<_>>(),
                launched_ids
            );
        }

        game.tick(5);
        assert_eq!(
            game.crisis_assault_units()
                .iter()
                .filter(|unit| unit.owner_player_id == player_id)
                .count(),
            3
        );
        assert_eq!(
            game.settlement_crisis().unwrap().assault_spawn_generation,
            1,
            "active evaluation must not duplicate the generation"
        );

        let unit_ids = units.iter().map(|unit| unit.obj_id).collect::<Vec<_>>();
        kill_assault_unit_through_normal_combat(&mut game, unit_ids[0]);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive,
            "a partial normal defeat cannot resolve the assault"
        );
        let hero_entity = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<Entity, With<SubclassHero>>();
            query.iter(world).next().unwrap()
        };
        game.app
            .world_mut()
            .entity_mut(hero_entity)
            .insert(crate::common::Target {
                id: *unit_ids.last().unwrap(),
            });
        for unit_id in unit_ids.iter().skip(1) {
            kill_assault_unit_through_normal_combat(&mut game, *unit_id);
        }
        game.tick(1);

        let resolved = game.settlement_crisis().expect("resolved crisis state");
        assert_eq!(resolved.phase, CrisisPhase::Resolved);
        assert!(resolved.resolution_recorded);
        assert!(resolved.resolved_at_tick.is_some());
        assert!(!resolved.warning_active);
        assert!(resolved.assault_unit_ids.is_empty());
        assert_eq!(game.personal_crises_resolved(), 1);
        assert!(game
            .world()
            .get::<crate::common::Target>(hero_entity)
            .is_none());
        assert_eq!(game.metrics().waves_survived, 0);

        game.tick(20);
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            game.settlement_crisis().unwrap().assault_spawn_generation,
            1
        );

        println!(
            "checkpoint3_normal_victory assault_id={} generation={} templates={:?} units={} resolution_count={} automatic_dusk_hordes={}",
            assault_id,
            resolved.assault_spawn_generation,
            actual_templates,
            units.len(),
            game.personal_crises_resolved(),
            game.metrics().waves_survived
        );
    }

    #[test]
    fn checkpoint3_ready_clock_pauses_offline_and_resumes_on_reconnect() {
        use crate::game::{CrisisPhase, ASSAULT_READY_GRACE_TICKS};

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultPresenceBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        let ready_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;

        game.app.world_mut().resource_mut::<GameTick>().0 = ready_tick + 9;
        game.tick(1);
        assert_eq!(game.settlement_crisis().unwrap().phase_online_ticks, 10);

        game.disconnect_player();
        game.app.world_mut().resource_mut::<GameTick>().0 += 5_000;
        game.tick(1);
        let offline = game.settlement_crisis().unwrap();
        assert_eq!(offline.phase, CrisisPhase::AssaultReady);
        assert_eq!(offline.phase_online_ticks, 10);
        assert!(game.crisis_assault_units().is_empty());

        let reconnect_preferred = next_preferred_assault_tick(game.game_tick());
        let remaining_online_ticks = ASSAULT_READY_GRACE_TICKS - 10;
        {
            let world = game.app.world_mut();
            world.resource_mut::<GameTick>().0 = reconnect_preferred - 1;
            let mut crises = world.resource_mut::<SettlementCrisisState>();
            crises
                .get_mut(&game.player_id)
                .expect("ready crisis")
                .last_evaluated_tick = reconnect_preferred - remaining_online_ticks;
        }
        game.reconnect_player();
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive
        );
        assert_eq!(game.crisis_assault_units().len(), 3);
    }

    #[test]
    fn checkpoint3_missing_anchor_stays_ready_without_consuming_an_assault_id() {
        use crate::game::{BoundMonolith, CrisisPhase, SpawnPositions};

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultNoAnchorBot");
        let preferred_tick = set_personal_assault_ready(&mut game);

        {
            let world = game.app.world_mut();
            let hero = {
                let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
                query
                    .iter(world)
                    .find(|(_, owner)| owner.0 == player_id)
                    .map(|(entity, _)| entity)
                    .expect("owner hero")
            };
            world.entity_mut(hero).remove::<BoundMonolith>();

            let structures = {
                let mut query = world.query_filtered::<(Entity, &PlayerId), With<ClassStructure>>();
                query
                    .iter(world)
                    .filter(|(_, owner)| owner.0 == player_id)
                    .map(|(entity, _)| entity)
                    .collect::<Vec<_>>()
            };
            for entity in structures {
                *world.get_mut::<State>(entity).expect("structure state") = State::Building;
            }
            world.resource_mut::<SpawnPositions>().remove(&player_id);
            world.resource_mut::<GameTick>().0 = preferred_tick - 2;
        }

        game.tick(2);

        let crisis = game.settlement_crisis().expect("ready crisis remains");
        assert_eq!(crisis.phase, CrisisPhase::AssaultReady);
        assert_eq!(crisis.assault_id, None);
        assert_eq!(crisis.assault_spawn_generation, 0);
        assert!(crisis.assault_unit_ids.is_empty());
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_spawn_failure_stays_ready_without_consuming_a_generation() {
        use crate::game::CrisisPhase;
        use crate::map::TileType;

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultNoSpawnBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);

        {
            let world = game.app.world_mut();
            for tile in &mut world.resource_mut::<Map>().base {
                tile.tile_type = TileType::Ocean;
            }
            world.resource_mut::<GameTick>().0 = preferred_tick - 2;
        }
        game.tick(2);

        let crisis = game.settlement_crisis().expect("ready crisis remains");
        assert_eq!(crisis.phase, CrisisPhase::AssaultReady);
        assert_eq!(crisis.assault_id, None);
        assert_eq!(crisis.assault_spawn_generation, 0);
        assert!(crisis.assault_unit_ids.is_empty());
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_disconnect_blocks_an_already_requested_npc_attack() {
        use big_brain::prelude::{ActionState, Actor};

        use crate::common::AttackTarget;
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultOfflineDamageBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let unit_id = game.crisis_assault_units()[0].obj_id;

        let (hero_entity, hero_id, hero_pos, hp_before, unit_entity) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(Entity, &Id, &Position, &Stats), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_pos, hero_stats) =
                hero_query.iter(world).next().expect("owner hero");
            let unit_entity = world
                .resource::<EntityObjMap>()
                .get_entity(unit_id)
                .expect("assault attacker");
            (
                hero_entity,
                hero_id.0,
                *hero_pos,
                hero_stats.hp,
                unit_entity,
            )
        };
        {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(unit_entity).unwrap() = hero_pos;
            world.get_mut::<VisibleTarget>(unit_entity).unwrap().target = hero_id;
            world.spawn((Actor(unit_entity), ActionState::Requested, AttackTarget));
        }

        game.disconnect_player();
        game.tick(1);

        assert_eq!(
            game.world().get::<Stats>(hero_entity).unwrap().hp,
            hp_before
        );
        assert!(game.world().get::<StateDead>(hero_entity).is_none());
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultReady
        );
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_disconnect_drops_queued_player_combat_without_progress() {
        use crate::constants::ATTACK_COOLDOWN_TICKS;
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultQueuedCombatBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let target_id = game.crisis_assault_units()[0].obj_id;
        let score_before = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();

        let (hero_id, hero_entity, hero_pos, target_entity) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(Entity, &Id, &Position), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_pos) = hero_query.iter(world).next().unwrap();
            let target_entity = world
                .resource::<EntityObjMap>()
                .get_entity(target_id)
                .unwrap();
            (hero_id.0, hero_entity, *hero_pos, target_entity)
        };
        {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(target_entity).unwrap() = hero_pos;
            world.get_mut::<Stats>(target_entity).unwrap().hp = 0;
            *world.get_mut::<State>(hero_entity).unwrap() = State::None;
            world.get_mut::<Stats>(hero_entity).unwrap().stamina = Some(100);
            world.resource_mut::<GameTick>().0 += ATTACK_COOLDOWN_TICKS + 1;
        }
        game.inject(PlayerEvent::Attack {
            player_id,
            attack_type: "quick".to_string(),
            source_id: hero_id,
            target_id,
        });
        game.disconnect_player();
        game.tick(2);

        let crisis = game.settlement_crisis().unwrap();
        let score_after = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        assert_eq!(crisis.phase, CrisisPhase::AssaultReady);
        assert_eq!(crisis.assault_retry_count, 1);
        assert_eq!(crisis.assault_remaining_templates.len(), 3);
        assert_eq!(score_after.enemies_killed, score_before.enemies_killed);
        assert_eq!(score_after.elites_killed, score_before.elites_killed);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_disconnect_drops_queued_combo_and_block_state() {
        use crate::combat::{AttackType, ComboTracker};
        use crate::effect::{Effect, Effects};
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultQueuedComboBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let target_id = game.crisis_assault_units()[0].obj_id;
        let score_before = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();

        let (hero_id, hero_entity, hero_pos, target_entity) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(Entity, &Id, &Position), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_pos) = hero_query.iter(world).next().unwrap();
            let target_entity = world
                .resource::<EntityObjMap>()
                .get_entity(target_id)
                .unwrap();
            (hero_id.0, hero_entity, *hero_pos, target_entity)
        };
        {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(target_entity).unwrap() = hero_pos;
            world.get_mut::<Stats>(target_entity).unwrap().hp = 0;
            *world.get_mut::<State>(hero_entity).unwrap() = State::None;
            world.get_mut::<Stats>(hero_entity).unwrap().stamina = Some(100);
            world.entity_mut(hero_entity).insert(ComboTracker {
                target_id,
                attacks: vec![AttackType::Quick, AttackType::Quick],
            });
        }
        game.inject(PlayerEvent::Combo {
            player_id,
            source_id: hero_id,
            target_id,
            combo_type: "Hamstring".to_string(),
        });
        game.inject(PlayerEvent::Block {
            player_id,
            source_id: hero_id,
            defense: "brace".to_string(),
        });
        game.disconnect_player();
        game.tick(2);

        let crisis = game.settlement_crisis().unwrap();
        let effects = game.world().get::<Effects>(hero_entity).unwrap();
        let score_after = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        assert_eq!(crisis.phase, CrisisPhase::AssaultReady);
        assert_eq!(crisis.assault_remaining_templates.len(), 3);
        assert!(!effects.has(Effect::Bracing));
        assert!(!effects.has(Effect::Dodging));
        assert!(!effects.has(Effect::Parrying));
        assert_eq!(score_after.enemies_killed, score_before.enemies_killed);
        assert_eq!(score_after.elites_killed, score_before.elites_killed);
        assert_eq!(game.personal_crises_resolved(), 0);
    }

    #[test]
    fn checkpoint3_connected_helper_cannot_progress_an_offline_owner_assault() {
        use crate::constants::ATTACK_COOLDOWN_TICKS;
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;

        let mut game = HeadlessGame::new(20_000);
        let owner_player_id = game.spawn_hero("Warrior", "AssaultOfflineOwnerBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let target_id = game.crisis_assault_units()[0].obj_id;
        let helper_player_id = owner_player_id + 1;

        let helper_client = Client {
            id: Uuid::from_u128(helper_player_id as u128),
            player_id: helper_player_id,
            sender: game.packet_tx.clone(),
        };
        game.clients
            .lock()
            .unwrap()
            .insert(helper_client.id, helper_client);
        game.inject(PlayerEvent::NewPlayer {
            player_id: helper_player_id,
            hero_name: "AssaultOnlineHelperBot".to_string(),
            class_name: "Warrior".to_string(),
        });
        game.tick(8);

        let (helper_id, helper_entity, helper_pos, target_entity) = {
            let world = game.app.world_mut();
            let mut helper_query =
                world.query_filtered::<(Entity, &Id, &PlayerId, &Position), With<SubclassHero>>();
            let (helper_entity, helper_id, _, helper_pos) = helper_query
                .iter(world)
                .find(|(_, _, owner, _)| owner.0 == helper_player_id)
                .expect("connected helper hero");
            let target_entity = world
                .resource::<EntityObjMap>()
                .get_entity(target_id)
                .expect("assault target");
            (helper_id.0, helper_entity, *helper_pos, target_entity)
        };
        let helper_score_before = game
            .world()
            .resource::<RunScoreState>()
            .get(&helper_player_id)
            .cloned()
            .unwrap_or_default();
        {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(target_entity).unwrap() = helper_pos;
            world.get_mut::<Stats>(target_entity).unwrap().hp = 0;
            *world.get_mut::<State>(helper_entity).unwrap() = State::None;
            world.get_mut::<Stats>(helper_entity).unwrap().stamina = Some(100);
            world.resource_mut::<GameTick>().0 += ATTACK_COOLDOWN_TICKS + 1;
        }
        game.inject(PlayerEvent::Attack {
            player_id: helper_player_id,
            attack_type: "quick".to_string(),
            source_id: helper_id,
            target_id,
        });
        game.disconnect_player();
        game.tick(2);

        let helper_score_after = game
            .world()
            .resource::<RunScoreState>()
            .get(&helper_player_id)
            .cloned()
            .unwrap_or_default();
        let crisis = game.settlement_crisis().unwrap();
        assert_eq!(crisis.phase, CrisisPhase::AssaultReady);
        assert_eq!(crisis.assault_remaining_templates.len(), 3);
        assert_eq!(
            helper_score_after.enemies_killed,
            helper_score_before.enemies_killed
        );
        assert_eq!(
            helper_score_after.elites_killed,
            helper_score_before.elites_killed
        );
        assert_eq!(game.personal_crises_resolved(), 0);
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_duplicate_new_player_cannot_erase_an_active_assault() {
        use crate::game::CrisisPhase;
        use crate::player_setup::{AssignedStartLocations, StartLocations};

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultDuplicateRunBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let before = game.settlement_crisis().unwrap();
        let assault_id = before.assault_id.unwrap();
        let generation = before.assault_spawn_generation;
        let unit_ids = game
            .crisis_assault_units()
            .iter()
            .filter(|unit| unit.owner_player_id == player_id)
            .map(|unit| unit.obj_id)
            .collect::<HashSet<_>>();
        let starts_before = game.world().resource::<StartLocations>().len();

        game.inject(PlayerEvent::NewPlayer {
            player_id,
            hero_name: "DuplicateRunBot".to_string(),
            class_name: "Warrior".to_string(),
        });
        game.tick(8);

        let after = game.settlement_crisis().unwrap();
        let owned_heroes = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<&PlayerId, With<SubclassHero>>();
            query
                .iter(world)
                .filter(|owner| owner.0 == player_id)
                .count()
        };
        assert_eq!(owned_heroes, 1);
        assert_eq!(after.phase, CrisisPhase::AssaultActive);
        assert_eq!(after.assault_id, Some(assault_id));
        assert_eq!(after.assault_spawn_generation, generation);
        assert_eq!(
            game.world().resource::<StartLocations>().len(),
            starts_before
        );
        assert!(game
            .world()
            .resource::<AssignedStartLocations>()
            .contains_key(&player_id));
        assert_eq!(
            game.crisis_assault_units()
                .iter()
                .filter(|unit| unit.owner_player_id == player_id)
                .map(|unit| unit.obj_id)
                .collect::<HashSet<_>>(),
            unit_ids
        );
    }

    #[test]
    fn checkpoint3_helper_kill_counts_for_owner_without_transferring_crisis() {
        use crate::constants::ATTACK_COOLDOWN_TICKS;
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultOwnerBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let units = game.crisis_assault_units();
        let helper_player_id = player_id + 1;

        // Create a real connected helper run and submit the same normal combat
        // event a nearby human client would send.
        let helper_client = Client {
            id: Uuid::from_u128(helper_player_id as u128),
            player_id: helper_player_id,
            sender: game.packet_tx.clone(),
        };
        game.clients
            .lock()
            .unwrap()
            .insert(helper_client.id, helper_client);
        game.inject(PlayerEvent::NewPlayer {
            player_id: helper_player_id,
            hero_name: "AssaultHelperBot".to_string(),
            class_name: "Warrior".to_string(),
        });
        game.tick(8);

        let target_id = units[0].obj_id;
        let (helper_id, helper_entity, helper_pos, target_entity) = {
            let world = game.app.world_mut();
            let mut helper_query =
                world.query_filtered::<(Entity, &Id, &PlayerId, &Position), With<SubclassHero>>();
            let (helper_entity, helper_id, _, helper_pos) = helper_query
                .iter(world)
                .find(|(_, _, owner, _)| owner.0 == helper_player_id)
                .map(|(entity, id, owner, pos)| (entity, id.0, owner.0, *pos))
                .expect("connected helper hero");
            let target_entity = world
                .resource::<EntityObjMap>()
                .get_entity(target_id)
                .expect("helper combat target");
            (helper_id, helper_entity, helper_pos, target_entity)
        };
        {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(target_entity).unwrap() = helper_pos;
            world.get_mut::<Stats>(target_entity).unwrap().hp = 0;
            *world.get_mut::<State>(helper_entity).unwrap() = State::None;
            world.get_mut::<Stats>(helper_entity).unwrap().stamina = Some(100);
            world.resource_mut::<GameTick>().0 += ATTACK_COOLDOWN_TICKS + 1;
        }
        game.inject(PlayerEvent::Attack {
            player_id: helper_player_id,
            attack_type: "quick".to_string(),
            source_id: helper_id,
            target_id,
        });
        game.tick(3);
        assert!(game.world().get::<StateDead>(target_entity).is_some());

        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive
        );
        assert_eq!(
            game.settlement_crisis()
                .unwrap()
                .assault_remaining_templates
                .len(),
            2
        );
        assert_eq!(game.personal_crises_resolved(), 0);
        assert_eq!(
            game.world()
                .resource::<RunScoreState>()
                .get(&helper_player_id)
                .map(|score| score.enemies_killed),
            Some(1),
            "ordinary kill score follows LastAttacker"
        );

        for unit in units.iter().skip(1) {
            kill_assault_unit_through_normal_combat(&mut game, unit.obj_id);
        }
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            game.world()
                .resource::<RunScoreState>()
                .get(&helper_player_id)
                .map(|score| score.personal_crises_resolved)
                .unwrap_or(0),
            0,
            "crisis completion remains with the attributed owner"
        );
    }

    #[test]
    fn checkpoint3_missing_live_unit_resets_instead_of_resolving() {
        use crate::game::CrisisPhase;
        use crate::ids::{EntityObjMap, Ids};

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultMissingUnitBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let units = game.crisis_assault_units();
        let missing_id = units[0].obj_id;
        let missing_entity = game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(missing_id)
            .unwrap();
        {
            let world = game.app.world_mut();
            world.resource_mut::<EntityObjMap>().remove_obj(missing_id);
            world.resource_mut::<Ids>().remove_obj(missing_id);
            world.despawn(missing_entity);
        }
        game.tick(1);

        let reset = game.settlement_crisis().unwrap();
        assert_eq!(reset.phase, CrisisPhase::AssaultReady);
        assert_eq!(reset.assault_retry_count, 1);
        assert!(!reset.resolution_recorded);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_disconnect_retry_headless_preserves_defeated_slots() {
        use crate::game::CrisisPhase;
        use crate::ids::{EntityObjMap, Ids};
        use crate::player_setup::RunSpawnedObjs;

        let mut game = HeadlessGame::new(30_000);
        let player_id = game.spawn_hero("Warrior", "AssaultRetryBot");
        game.set_sanctuary_at_base(3);
        let run_ids_before = game
            .world()
            .resource::<RunSpawnedObjs>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        let score_before = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);

        let first = game.settlement_crisis().unwrap();
        let assault_id = first.assault_id.unwrap();
        let old_units = game.crisis_assault_units();
        let old_ids = old_units.iter().map(|unit| unit.obj_id).collect::<Vec<_>>();
        let defeated_template = old_units[0].template.clone();
        kill_assault_unit_through_normal_combat(&mut game, old_units[0].obj_id);
        assert_eq!(
            game.settlement_crisis()
                .unwrap()
                .assault_remaining_templates
                .len(),
            2
        );

        let hero_entity = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<Entity, With<SubclassHero>>();
            query.iter(world).next().unwrap()
        };
        game.app
            .world_mut()
            .entity_mut(hero_entity)
            .insert(crate::common::Target {
                id: old_units[1].obj_id,
            });

        game.disconnect_player();
        game.tick(1);
        let reset = game.settlement_crisis().expect("ready after disconnect");
        assert_eq!(reset.phase, CrisisPhase::AssaultReady);
        assert_eq!(reset.assault_retry_count, 1);
        assert!(reset.warning_active);
        assert!(!reset.resolution_recorded);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert!(game.crisis_assault_units().is_empty());
        assert!(game
            .world()
            .get::<crate::common::Target>(hero_entity)
            .is_none());
        assert!(old_ids.iter().all(|id| game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(*id)
            .is_none()));
        assert!(old_ids.iter().all(|id| !game
            .world()
            .resource::<Ids>()
            .obj_player_map
            .contains_key(id)));
        let run_ids_after_reset = game
            .world()
            .resource::<RunSpawnedObjs>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        assert!(run_ids_before
            .iter()
            .all(|id| run_ids_after_reset.contains(id)));
        assert!(old_ids.iter().all(|id| !run_ids_after_reset.contains(id)));

        game.tick(5);
        assert_eq!(game.settlement_crisis().unwrap().assault_retry_count, 1);
        assert!(game.crisis_assault_units().is_empty());

        game.reconnect_player();
        let retry_preferred_tick = set_reconnect_ready_clock(&mut game);
        advance_ready_clock_to_launch(&mut game, retry_preferred_tick);
        let relaunched = game.settlement_crisis().expect("retried assault");
        assert_eq!(relaunched.phase, CrisisPhase::AssaultActive);
        assert_eq!(relaunched.assault_id, Some(assault_id));
        assert_eq!(relaunched.assault_spawn_generation, 2);
        assert_eq!(relaunched.assault_retry_count, 1);
        let retry_units = game.crisis_assault_units();
        assert_eq!(retry_units.len(), 2);
        assert!(retry_units.iter().all(|unit| {
            unit.assault_id == assault_id
                && unit.spawn_generation == 2
                && !old_ids.contains(&unit.obj_id)
        }));
        let mut expected_retry_templates = old_units
            .iter()
            .map(|unit| unit.template.clone())
            .collect::<Vec<_>>();
        let removed_index = expected_retry_templates
            .iter()
            .position(|template| template == &defeated_template)
            .unwrap();
        expected_retry_templates.remove(removed_index);
        let mut actual_retry_templates = retry_units
            .iter()
            .map(|unit| unit.template.clone())
            .collect::<Vec<_>>();
        expected_retry_templates.sort();
        actual_retry_templates.sort();
        assert_eq!(actual_retry_templates, expected_retry_templates);

        for unit in retry_units {
            kill_assault_unit_through_normal_combat(&mut game, unit.obj_id);
        }
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        let score_after = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        assert_eq!(score_after.enemies_killed - score_before.enemies_killed, 3);
        assert_eq!(score_after.elites_killed - score_before.elites_killed, 3);

        game.disconnect_player();
        game.tick(3);
        game.reconnect_player();
        game.tick(3);
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            game.settlement_crisis().unwrap().assault_spawn_generation,
            2
        );

        println!(
            "checkpoint3_disconnect_retry assault_id={} retry_count={} units_before_disconnect={} units_after_cleanup=0 new_generation={} retry_units={} completions_before=0 completions_after={} duplicated_completion=false",
            assault_id,
            game.settlement_crisis().unwrap().assault_retry_count,
            old_units.len(),
            game.settlement_crisis().unwrap().assault_spawn_generation,
            actual_retry_templates.len(),
            game.personal_crises_resolved()
        );
    }

    #[test]
    fn checkpoint3_legacy_mode_does_not_run_the_personal_assault_lifecycle() {
        use crate::game::{CrisisPhase, ASSAULT_MAX_ONLINE_WAIT_TICKS};

        let mut game = HeadlessGame::new_with_director(10_000, SurvivalDirectorMode::Legacy);
        game.spawn_hero("Warrior", "LegacyPersonalAssaultIsolationBot");
        let preferred_tick = set_personal_assault_ready(&mut game);
        {
            let world = game.app.world_mut();
            world.resource_mut::<GameTick>().0 = preferred_tick;
            let mut crises = world.resource_mut::<SettlementCrisisState>();
            let crisis = crises.get_mut(&game.player_id).unwrap();
            crisis.phase_online_ticks = ASSAULT_MAX_ONLINE_WAIT_TICKS;
            crisis.last_evaluated_tick = preferred_tick;
        }
        game.tick(3);

        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultReady
        );
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_true_death_cleanup_isolated_and_idempotent() {
        use crate::ids::{EntityObjMap, Ids};

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultTrueDeathBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let own_assault_id = game.settlement_crisis().unwrap().assault_id.unwrap();
        let own_ids = game
            .crisis_assault_units()
            .iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        let orphaned_entity = {
            let world = game.app.world_mut();
            let entity = world
                .resource::<EntityObjMap>()
                .get_entity(own_ids[0])
                .expect("attributed assault entity");
            world.resource_mut::<EntityObjMap>().remove_obj(own_ids[0]);
            entity
        };

        let (other_entity, other_id, unrelated_entity, unrelated_id) = {
            let world = game.app.world_mut();
            let other_id = world.resource_mut::<Ids>().new_obj_id();
            let entity = world
                .spawn((
                    Id(other_id),
                    PlayerId(crate::constants::NPC_PLAYER_ID),
                    Position { x: 0, y: 0 },
                    Template("Wolf Rider".to_string()),
                    CrisisAssaultUnit {
                        owner_player_id: player_id + 1,
                        assault_id: own_assault_id + 1,
                        spawn_generation: 1,
                    },
                ))
                .id();
            world
                .resource_mut::<Ids>()
                .new_obj(other_id, crate::constants::NPC_PLAYER_ID);
            world
                .resource_mut::<EntityObjMap>()
                .new_obj(other_id, entity);

            // An un-attributed world hostile next to the recycled start is not
            // owned by this run and must not be removed as collateral cleanup.
            let unrelated_id = world.resource_mut::<Ids>().new_obj_id();
            let unrelated_entity = world
                .spawn((
                    Id(unrelated_id),
                    PlayerId(crate::constants::NPC_PLAYER_ID),
                    Position { x: 0, y: 0 },
                    Template("Wolf".to_string()),
                ))
                .id();
            world
                .resource_mut::<Ids>()
                .new_obj(unrelated_id, crate::constants::NPC_PLAYER_ID);
            world
                .resource_mut::<EntityObjMap>()
                .new_obj(unrelated_id, unrelated_entity);
            (entity, other_id, unrelated_entity, unrelated_id)
        };

        let current_tick = game.game_tick();
        let hero = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<Entity, With<SubclassHero>>();
            query.iter(world).next().expect("hero")
        };
        game.disconnect_player();
        game.app.world_mut().entity_mut(hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Checkpoint 3 cleanup".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick - (10 * crate::constants::TICKS_PER_SEC) - 1,
            },
        ));
        game.tick(3);

        assert!(game.settlement_crisis().is_none());
        assert_eq!(game.personal_crises_resolved(), 0);
        assert!(own_ids.iter().all(|id| game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(*id)
            .is_none()));
        assert!(
            game.world().get_entity(orphaned_entity).is_err(),
            "True Death must despawn an attributed orphan even without its entity-map entry"
        );
        assert!(game.world().get_entity(other_entity).is_ok());
        assert_eq!(
            game.world()
                .get::<CrisisAssaultUnit>(other_entity)
                .unwrap()
                .owner_player_id,
            player_id + 1
        );
        assert_eq!(
            game.world().resource::<EntityObjMap>().get_entity(other_id),
            Some(other_entity)
        );
        assert!(game.world().get_entity(unrelated_entity).is_ok());
        assert_eq!(
            game.world()
                .resource::<EntityObjMap>()
                .get_entity(unrelated_id),
            Some(unrelated_entity)
        );

        game.tick(5);
        assert!(game.world().get_entity(other_entity).is_ok());
        assert!(game.world().get_entity(unrelated_entity).is_ok());
    }

    #[test]
    fn checkpoint3_true_death_while_ready_cancels_without_launch_or_completion() {
        use crate::game::CrisisPhase;

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultReadyTrueDeathBot");
        let _preferred_tick = set_personal_assault_ready(&mut game);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultReady
        );

        let current_tick = game.game_tick();
        let hero = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<Entity, With<SubclassHero>>();
            query.iter(world).next().expect("hero")
        };
        game.app.world_mut().entity_mut(hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Checkpoint 3 ready cleanup".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick - (10 * crate::constants::TICKS_PER_SEC) - 1,
            },
        ));
        game.tick(3);

        assert!(game.settlement_crisis().is_none());
        assert!(game.crisis_assault_units().is_empty());
        assert_eq!(game.personal_crises_resolved(), 0);
    }

    #[test]
    fn checkpoint3_repeated_disconnect_relaunch_sample_has_no_duplicates() {
        use crate::game::CrisisPhase;

        let mut game = HeadlessGame::new(40_000);
        game.spawn_hero("Warrior", "AssaultRepeatBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let assault_id = game.settlement_crisis().unwrap().assault_id.unwrap();

        for retry in 1..=3 {
            game.disconnect_player();
            game.tick(2);
            assert!(game.crisis_assault_units().is_empty());
            assert_eq!(game.settlement_crisis().unwrap().assault_retry_count, retry);

            game.reconnect_player();
            let preferred_tick = set_reconnect_ready_clock(&mut game);
            advance_ready_clock_to_launch(&mut game, preferred_tick);
            let crisis = game.settlement_crisis().unwrap();
            assert_eq!(crisis.assault_id, Some(assault_id));
            assert_eq!(crisis.assault_spawn_generation, retry + 1);
            assert_eq!(game.crisis_assault_units().len(), 3);
        }

        let final_units = game.crisis_assault_units();
        for unit in final_units {
            kill_assault_unit_through_normal_combat(&mut game, unit.obj_id);
        }
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            game.settlement_crisis().unwrap().assault_spawn_generation,
            4
        );

        println!(
            "checkpoint3_repeated_safety assault_id={} retries=3 final_generation=4 stale_units=0 completion_count={} duplicate_assault=false panic=false",
            assault_id,
            game.personal_crises_resolved()
        );
    }

    // Hand-crafting Firewood from a Log — the cook economy's fuel chain. The bot
    // relies on this to keep cooking past the 10 starting Firewood.
    #[test]
    fn craft_firewood_from_log() {
        use crate::ids::Ids;
        use crate::templates::Templates;

        let mut game = HeadlessGame::new(5_000);
        let pid = game.spawn_hero("Warrior", "FuelBot");
        game.tick(50);

        // Give the hero a Log (same item the Burrow stocks).
        {
            let world = game.app.world_mut();
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut q = world.query_filtered::<&mut Inventory, With<SubclassHero>>();
            let mut inv = q.iter_mut(world).next().expect("hero inventory");
            inv.new(
                item_id,
                "Springbranch Maple Log".to_string(),
                1,
                &item_templates,
            );
        }
        let firewood_before: i32 = game
            .observe()
            .inventory
            .iter()
            .filter(|i| i.name == "Firewood")
            .map(|i| i.quantity)
            .sum();

        game.inject(PlayerEvent::Craft {
            player_id: pid,
            recipe_name: "Firewood".to_string(),
        });
        game.tick(100); // crafting_time is 60 ticks

        let view = game.observe();
        let firewood_after: i32 = view
            .inventory
            .iter()
            .filter(|i| i.name == "Firewood")
            .map(|i| i.quantity)
            .sum();
        let gained = firewood_after - firewood_before;
        // Inventory::craft rolls a random 1..=amount yield (amount=5 for Firewood).
        assert!(
            (1..=5).contains(&gained),
            "crafting should turn 1 Log into 1-5 Firewood, got {gained}"
        );
        assert!(
            !view.inventory.iter().any(|i| i.class == "Log"),
            "the Log should be consumed by the craft"
        );
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
        assert_eq!(
            gold_after, 25,
            "the wage (25) should be deducted from the hero"
        );
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
        assert_eq!(
            shards, 1,
            "the upgrade cost should be deducted in Soulshards"
        );
    }
}
