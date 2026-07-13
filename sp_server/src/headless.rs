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
use crate::effect::Effects;
use crate::game::{
    BoundMonolith, Client, Clients, CrisisAssaultUnit, CrisisPhase, CrisisTelemetryState,
    DatabaseClient, DatabaseManagers, GameTick, Merchant, MerchantSailState, Monolith,
    NetworkReceiver, Objectives, PlayerIntroEncounters, PlayerObjectives, PlayerRunScore,
    PlayerStats, PlayerVictory, RunScoreState, SettlementCrisis, SettlementCrisisState,
    SurvivalDirectorMode, VictoryState,
};
use crate::item::{AttrKey, AttrVal, Inventory};
use crate::map::Map;
use crate::obj::{
    Class, ClassStructure, Id, LastCombatTick, LastDamageTick, Misc, Name, Order, PlayerId,
    Position, State, StateDead, Stats, Subclass, SubclassHero, SubclassNPC, SubclassVillager,
    Template, TrueDeath,
};
use crate::resource::Resources;
use crate::safe_logout::{
    record_player_combat_activity, CancelSafeLogout, PlayerPresenceRecord, PlayerWorldPresence,
    PlayerWorldPresenceState, RequestSafeLogout, SafeLogoutCancelReason, SafeLogoutRejectionReason,
};
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
    pub hp: i32,
    pub base_hp: i32,
    pub pos: Position,
    pub visible_target: Option<i32>,
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

fn packet_has_tag(packet: &str, expected: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(packet)
        .ok()
        .and_then(|value| value.get("packet")?.as_str().map(str::to_owned))
        .as_deref()
        == Some(expected)
}

const fn crisis_phase_name(phase: CrisisPhase) -> &'static str {
    match phase {
        CrisisPhase::Dormant => "dormant",
        CrisisPhase::Signs => "signs",
        CrisisPhase::Pressure => "pressure",
        CrisisPhase::Preparing => "preparing",
        CrisisPhase::AssaultReady => "assault_ready",
        CrisisPhase::AssaultActive => "assault_active",
        CrisisPhase::Resolved => "resolved",
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
    // Personal-crisis runtime telemetry. These are appended to the runner
    // schemas so every pre-Checkpoint-4 field keeps its original name/order.
    pub crisis_highest_phase: String,
    pub crisis_final_phase: String,
    pub crisis_final_pressure: i32,
    pub crisis_signs_tick: Option<i32>,
    pub crisis_pressure_tick: Option<i32>,
    pub crisis_preparing_tick: Option<i32>,
    pub crisis_assault_ready_tick: Option<i32>,
    pub crisis_assault_active_tick: Option<i32>,
    pub crisis_resolved_tick: Option<i32>,
    pub crisis_assaults_launched: i32,
    pub crisis_assaults_resolved: i32,
    pub crisis_units_remaining: i32,
    pub crisis_status_packets_sent: i32,
    pub crisis_login_snapshots_sent: i32,
    pub crisis_duplicate_assaults: i32,
    pub personal_crisis_automatic_dusk_hordes: i32,
    pub crisis_invariants_ok: bool,
}

/// Runtime-only observations collected by the in-process harness. Gameplay
/// state remains authoritative; this tracker only samples it after updates and
/// never writes back into the world.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeadlessCrisisTelemetry {
    pub highest_phase: String,
    pub signs_tick: Option<i32>,
    pub pressure_tick: Option<i32>,
    pub preparing_tick: Option<i32>,
    pub assault_ready_tick: Option<i32>,
    pub assault_active_tick: Option<i32>,
    pub resolved_tick: Option<i32>,
    pub assaults_launched: i32,
    pub assaults_resolved: i32,
    pub units_remaining: i32,
    pub status_packets_sent: i32,
    pub login_snapshots_sent: i32,
    pub duplicate_assaults: i32,
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
    // Crisis packets are sparse because production delivery is deduplicated, so
    // retain them for deterministic scenarios and per-run packet telemetry.
    crisis_packet_history: Vec<String>,
    // Full packet capture is opt-in around short scenarios; normal long runner
    // runs do not retain the high-volume perception/update stream.
    capture_packets: bool,
    captured_packets: Vec<String>,
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
            crisis_packet_history: Vec::new(),
            capture_packets: false,
            captured_packets: Vec::new(),
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
        let mut nearest: Option<(Entity, i32, u32)> = None;
        {
            let mut q = world.query_filtered::<(Entity, &Id, &Position), With<Monolith>>();
            for (e, id, p) in q.iter(world) {
                let d = Map::distance((hero_pos.x, hero_pos.y), (p.x, p.y));
                if nearest.map_or(true, |(_, _, best_distance)| d < best_distance) {
                    nearest = Some((e, id.0, d));
                }
            }
        }
        if let Some((entity, monolith_id, _)) = nearest {
            if let Some(mut pos) = world.get_mut::<Position>(entity) {
                *pos = hero_pos;
            }
            if let Some(mut monolith) = world.get_mut::<Monolith>(entity) {
                monolith.sanctuary_level = level;
            }
            let mut heroes = world.query_filtered::<&mut BoundMonolith, With<SubclassHero>>();
            for mut bound in heroes.iter_mut(world) {
                if bound.id == monolith_id {
                    bound.pos = hero_pos;
                }
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
        while let Ok(packet) = self.packet_rx.try_recv() {
            if packet_has_tag(&packet, "crisis_status") {
                self.crisis_packet_history.push(packet.clone());
            }
            if self.capture_packets {
                self.captured_packets.push(packet);
            }
        }
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

    /// Start bounded, explicit capture of all outgoing packets. Long-running
    /// balance simulations should leave this disabled.
    pub fn start_packet_capture(&mut self) {
        self.captured_packets.clear();
        self.capture_packets = true;
    }

    /// Stop full capture and return every successfully decoded packet observed
    /// since `start_packet_capture`.
    pub fn finish_packet_capture(&mut self) -> Vec<ResponsePacket> {
        self.capture_packets = false;
        std::mem::take(&mut self.captured_packets)
            .into_iter()
            .filter_map(|packet| serde_json::from_str::<ResponsePacket>(&packet).ok())
            .collect()
    }

    /// Return and clear retained structured crisis-status packets. Cumulative
    /// telemetry counters are intentionally unaffected.
    pub fn take_crisis_status_packets(&mut self) -> Vec<ResponsePacket> {
        std::mem::take(&mut self.crisis_packet_history)
            .into_iter()
            .filter_map(|packet| serde_json::from_str::<ResponsePacket>(&packet).ok())
            .collect()
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

    /// Reconnect through the production ordering: install the authenticated
    /// client, then enqueue the ordinary Login event that drives resynchronization.
    pub fn reconnect_player_with_login(&mut self) {
        self.reconnect_player();
        // Production authentication emits Login after inserting the client.
        // Reuse that path so reconnect snapshot tests exercise real delivery.
        self.inject(PlayerEvent::Login {
            player_id: self.player_id,
        });
    }

    /// Enqueue the milestone's internal-only safe-logout request. This helper
    /// deliberately does not pump the app; callers control the exact update on
    /// which the authoritative server systems evaluate the request.
    pub fn request_safe_logout(&mut self) {
        self.app.world_mut().write_message(RequestSafeLogout {
            player_id: self.player_id,
        });
    }

    /// Enqueue an internal manual cancellation without advancing `GameTick`.
    pub fn cancel_safe_logout(&mut self) {
        self.app.world_mut().write_message(CancelSafeLogout {
            player_id: self.player_id,
        });
    }

    pub fn player_presence_record(&self) -> Option<PlayerPresenceRecord> {
        self.app
            .world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&self.player_id)
            .cloned()
    }

    pub fn player_presence(&self) -> Option<PlayerWorldPresence> {
        self.player_presence_record().map(|record| record.state)
    }

    pub fn safe_logout_start_tick(&self) -> Option<i32> {
        self.player_presence_record()
            .and_then(|record| record.safe_logout_requested_tick)
    }

    pub fn safe_logout_cancel_reason(&self) -> Option<SafeLogoutCancelReason> {
        self.player_presence_record()
            .and_then(|record| record.cancel_reason)
    }

    pub fn safe_logout_rejection_reason(&self) -> Option<SafeLogoutRejectionReason> {
        self.player_presence_record()
            .and_then(|record| record.rejection_reason)
    }

    /// Put the hero on the Monolith named by its authoritative `BoundMonolith`
    /// component. This never falls back to the nearest arbitrary sanctuary and
    /// does not advance the simulation.
    pub fn place_hero_in_own_bound_sanctuary(&mut self) -> Position {
        use crate::ids::EntityObjMap;

        let player_id = self.player_id;
        let world = self.app.world_mut();
        let (hero_entity, bound_monolith_id) = {
            let mut query =
                world.query_filtered::<(Entity, &PlayerId, &BoundMonolith), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner, _)| owner.0 == player_id)
                .map(|(entity, _, bound)| (entity, bound.id))
                .expect("headless hero with a bound Monolith")
        };
        let monolith_entity = world
            .resource::<EntityObjMap>()
            .get_entity(bound_monolith_id)
            .expect("bound Monolith entity-map entry");
        let monolith_pos = *world
            .get::<Position>(monolith_entity)
            .expect("bound Monolith position");
        *world
            .get_mut::<Position>(hero_entity)
            .expect("headless hero position") = monolith_pos;
        world
            .get_mut::<BoundMonolith>(hero_entity)
            .expect("headless hero bound Monolith")
            .pos = monolith_pos;
        monolith_pos
    }

    /// Move the authoritative hero position directly without pumping the app.
    /// Focused tests use this to make cancellation ordering deterministic.
    pub fn move_hero_for_test(&mut self, position: Position) {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        let hero_entity = {
            let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        *world
            .get_mut::<Position>(hero_entity)
            .expect("headless hero position") = position;
    }

    /// Spawn a live hostile recognized by both production combat and the
    /// safe-logout hostility query. The entity is server-authoritative and
    /// registered in both object-id indexes, but has no behaviour tree so it
    /// remains deterministic until the test moves, kills, or removes it.
    pub fn spawn_safe_logout_test_hostile(&mut self, position: Position) -> i32 {
        use crate::event::{EventExecuting, EventExecutingState};
        use crate::ids::{EntityObjMap, Ids};
        use crate::npc::VisibleTarget;

        let hero_id = {
            let world = self.app.world_mut();
            let mut query = world.query_filtered::<(&Id, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == self.player_id)
                .map(|(id, _)| id.0)
                .expect("headless hero id")
        };
        let world = self.app.world_mut();
        let obj_id = world.resource_mut::<Ids>().new_obj_id();
        let entity = world
            .spawn((
                Id(obj_id),
                PlayerId(crate::constants::NPC_PLAYER_ID),
                position,
                Name("Wolf".to_string()),
                Template("Wolf".to_string()),
                Class("Unit".to_string()),
                Subclass::Npc,
                SubclassNPC,
                State::None,
                Misc::default(),
                Stats {
                    hp: 20,
                    stamina: Some(100),
                    mana: None,
                    base_hp: 20,
                    base_stamina: Some(100),
                    base_mana: None,
                    base_def: 0,
                    damage_range: Some(1),
                    base_damage: Some(1),
                    base_speed: Some(1),
                    base_vision: Some(8),
                },
                Effects(std::collections::HashMap::new()),
                Inventory {
                    owner: obj_id,
                    items: Vec::new(),
                },
                LastCombatTick::default(),
                VisibleTarget::new(hero_id),
            ))
            .id();
        world.entity_mut(entity).insert(EventExecuting {
            event_type: String::new(),
            state: EventExecutingState::None,
        });
        world
            .resource_mut::<Ids>()
            .new_obj(obj_id, crate::constants::NPC_PLAYER_ID);
        world.resource_mut::<EntityObjMap>().new_obj(obj_id, entity);
        obj_id
    }

    pub fn move_safe_logout_test_hostile(&mut self, obj_id: i32, position: Position) {
        use crate::ids::EntityObjMap;

        let world = self.app.world_mut();
        let entity = world
            .resource::<EntityObjMap>()
            .get_entity(obj_id)
            .expect("safe-logout test hostile");
        *world
            .get_mut::<Position>(entity)
            .expect("safe-logout test hostile position") = position;
    }

    pub fn kill_safe_logout_test_hostile(&mut self, obj_id: i32) {
        use crate::ids::EntityObjMap;

        let dead_at = self.game_tick();
        let world = self.app.world_mut();
        let entity = world
            .resource::<EntityObjMap>()
            .get_entity(obj_id)
            .expect("safe-logout test hostile");
        world
            .get_mut::<Stats>(entity)
            .expect("safe-logout test hostile stats")
            .hp = 0;
        world.entity_mut(entity).insert((
            State::Dead,
            StateDead {
                dead_at,
                killer: "Headless safe-logout test".to_string(),
            },
        ));
    }

    pub fn remove_safe_logout_test_hostile(&mut self, obj_id: i32) {
        use crate::ids::{EntityObjMap, Ids};

        let world = self.app.world_mut();
        if let Some(entity) = world.resource::<EntityObjMap>().get_entity(obj_id) {
            let _ = world.despawn(entity);
        }
        world.resource_mut::<EntityObjMap>().remove_obj(obj_id);
        world.resource_mut::<Ids>().remove_obj(obj_id);
    }

    /// Apply authoritative incoming HP loss without advancing the app. The
    /// presence reconciler observes the decrease on the caller's next update.
    pub fn damage_hero_for_test(&mut self, damage: i32) -> i32 {
        let player_id = self.player_id;
        let tick = self.game_tick();
        let world = self.app.world_mut();
        let hero_entity = {
            let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let hp = {
            let mut stats = world
                .get_mut::<Stats>(hero_entity)
                .expect("headless hero stats");
            stats.hp = stats.hp.saturating_sub(damage.max(0));
            stats.hp
        };
        world.entity_mut(hero_entity).insert(LastDamageTick(tick));
        hp
    }

    /// Record the same aggregate activity used by successful server-authorized
    /// combat commands. This helper does not tick or fabricate a client packet.
    pub fn record_player_combat_for_test(&mut self) {
        let tick = self.game_tick();
        let player_id = self.player_id;
        record_player_combat_activity(
            player_id,
            tick,
            &mut self
                .app
                .world_mut()
                .resource_mut::<PlayerWorldPresenceState>(),
        );
    }

    pub fn crisis_telemetry(&self) -> HeadlessCrisisTelemetry {
        let mut snapshot = HeadlessCrisisTelemetry::default();
        if let Some(telemetry) = self
            .app
            .world()
            .get_resource::<CrisisTelemetryState>()
            .and_then(|state| state.get(&self.player_id))
        {
            snapshot.highest_phase = crisis_phase_name(telemetry.highest_phase).to_string();
            snapshot.signs_tick = telemetry.signs_tick;
            snapshot.pressure_tick = telemetry.pressure_tick;
            snapshot.preparing_tick = telemetry.preparing_tick;
            snapshot.assault_ready_tick = telemetry.assault_ready_tick;
            snapshot.assault_active_tick = telemetry.assault_active_tick;
            snapshot.resolved_tick = telemetry.resolved_tick;
            snapshot.assaults_launched = telemetry.assaults_launched;
            snapshot.assaults_resolved = telemetry.assaults_resolved;
            snapshot.status_packets_sent = telemetry.status_packets_sent;
            snapshot.login_snapshots_sent = telemetry.login_snapshots_sent;
            snapshot.duplicate_assaults = telemetry.duplicate_assaults;
        }
        snapshot.units_remaining = self
            .app
            .world()
            .resource::<SettlementCrisisState>()
            .get(&self.player_id)
            .map(|crisis| {
                crisis
                    .assault_unit_ids
                    .len()
                    .saturating_sub(crisis.assault_defeated_unit_ids.len()) as i32
            })
            .unwrap_or(0);
        snapshot
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
        let mut query = world.query::<(
            &Id,
            &Template,
            &CrisisAssaultUnit,
            &Stats,
            &Position,
            Option<&StateDead>,
            Option<&crate::npc::VisibleTarget>,
        )>();
        let mut units = query
            .iter(world)
            .map(
                |(id, template, assault, stats, pos, dead, visible_target)| CrisisAssaultUnitView {
                    obj_id: id.0,
                    template: template.0.clone(),
                    owner_player_id: assault.owner_player_id,
                    assault_id: assault.assault_id,
                    spawn_generation: assault.spawn_generation,
                    hp: stats.hp,
                    base_hp: stats.base_hp,
                    pos: *pos,
                    visible_target: visible_target.map(|target| target.target),
                    dead: dead.is_some(),
                },
            )
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
        let crisis_telemetry = self.crisis_telemetry();
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
        let final_crisis = world.resource::<SettlementCrisisState>().get(&pid).cloned();

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
            crisis_highest_phase: if crisis_telemetry.highest_phase.is_empty() {
                "none".to_string()
            } else {
                crisis_telemetry.highest_phase.clone()
            },
            crisis_final_phase: final_crisis
                .as_ref()
                .map(|crisis| crisis_phase_name(crisis.phase).to_string())
                .unwrap_or_else(|| "none".to_string()),
            crisis_final_pressure: final_crisis
                .as_ref()
                .map(|crisis| crisis.pressure)
                .unwrap_or(0),
            crisis_signs_tick: crisis_telemetry.signs_tick,
            crisis_pressure_tick: crisis_telemetry.pressure_tick,
            crisis_preparing_tick: crisis_telemetry.preparing_tick,
            crisis_assault_ready_tick: crisis_telemetry.assault_ready_tick,
            crisis_assault_active_tick: crisis_telemetry.assault_active_tick,
            crisis_resolved_tick: crisis_telemetry.resolved_tick,
            crisis_assaults_launched: crisis_telemetry.assaults_launched,
            crisis_assaults_resolved: crisis_telemetry.assaults_resolved,
            crisis_units_remaining: crisis_telemetry.units_remaining,
            crisis_status_packets_sent: crisis_telemetry.status_packets_sent,
            crisis_login_snapshots_sent: crisis_telemetry.login_snapshots_sent,
            crisis_duplicate_assaults: crisis_telemetry.duplicate_assaults,
            personal_crisis_automatic_dusk_hordes: run.waves_survived,
            crisis_invariants_ok: crisis_telemetry.duplicate_assaults == 0
                && crisis_telemetry.assaults_resolved <= crisis_telemetry.assaults_launched,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::CrisisStatusSnapshot;

    fn crisis_statuses(packets: Vec<ResponsePacket>) -> Vec<CrisisStatusSnapshot> {
        packets
            .into_iter()
            .filter_map(|packet| match packet {
                ResponsePacket::CrisisStatus { status } => Some(status),
                _ => None,
            })
            .collect()
    }

    fn crisis_phase_sequence(statuses: &[CrisisStatusSnapshot]) -> Vec<String> {
        let mut phases = Vec::new();
        for phase in statuses
            .iter()
            .filter(|status| status.exists)
            .filter_map(|status| status.phase.clone())
        {
            if phases.last() != Some(&phase) {
                phases.push(phase);
            }
        }
        phases
    }

    fn prepare_full_crisis_progression_facts(game: &mut HeadlessGame) {
        use crate::game::{InitialEncounterState, PlayerIntroState};

        let player_id = game.player_id();
        game.set_sanctuary_at_base(5);
        let world = game.app.world_mut();
        world
            .resource_mut::<PlayerIntroState>()
            .get_mut(&player_id)
            .expect("player intro state")
            .danger_unlocked = true;
        {
            let mut objectives = world.resource_mut::<Objectives>();
            let objectives = objectives.entry(player_id).or_default();
            objectives.explore_poi = true;
            objectives.choose_expansion = true;
        }
        world
            .resource_mut::<InitialEncounterState>()
            .remove(&player_id);
        for _ in 0..3 {
            world.spawn((PlayerId(player_id), State::None, ClassStructure));
        }
        world.spawn((PlayerId(player_id), State::None, SubclassVillager));
    }

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

    fn kill_assault_unit_through_normal_combat(
        game: &mut HeadlessGame,
        target_id: i32,
    ) -> Vec<(String, i32)> {
        let attacker_player_id = game.player_id();
        kill_assault_unit_through_normal_combat_as(game, attacker_player_id, target_id)
    }

    fn kill_assault_unit_through_normal_combat_as(
        game: &mut HeadlessGame,
        attacker_player_id: i32,
        target_id: i32,
    ) -> Vec<(String, i32)> {
        use crate::constants::ATTACK_COOLDOWN_TICKS;
        use crate::ids::EntityObjMap;

        let (hero_entity, hero_id, hero_pos, target_entity, loot_before) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(Entity, &Id, &PlayerId, &Position), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_pos) = hero_query
                .iter(world)
                .find(|(_, _, owner, _)| owner.0 == attacker_player_id)
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
            player_id: attacker_player_id,
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

    fn spawn_connected_helper(game: &mut HeadlessGame, name: &str) -> i32 {
        let helper_player_id = game.player_id() + 1;
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
            hero_name: name.to_string(),
            class_name: "Warrior".to_string(),
        });
        game.tick(8);
        helper_player_id
    }

    fn spawn_armed_owner_villager(
        game: &mut HeadlessGame,
        owner_player_id: i32,
        pos: Position,
    ) -> (Entity, i32) {
        use crate::event::{GameEvent, GameEventType, GameEvents};
        use crate::ids::Ids;
        use crate::templates::Templates;

        let current_tick = game.game_tick();
        {
            let world = game.app.world_mut();
            let event_id = world.resource_mut::<Ids>().new_map_event_id();
            world.resource_mut::<GameEvents>().insert(
                event_id,
                GameEvent {
                    event_id,
                    start_tick: current_tick,
                    run_tick: current_tick,
                    event_type: GameEventType::SpawnVillager {
                        pos,
                        player_id: owner_player_id,
                    },
                },
            );
        }
        game.tick(3);

        let (villager_entity, villager_id) = {
            let world = game.app.world_mut();
            let mut query =
                world.query_filtered::<(Entity, &Id, &PlayerId), With<SubclassVillager>>();
            query
                .iter(world)
                .find(|(_, _, owner)| owner.0 == owner_player_id)
                .map(|(entity, id, _)| (entity, id.0))
                .expect("owner villager")
        };

        {
            let world = game.app.world_mut();
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut inventory = world
                .get_mut::<Inventory>(villager_entity)
                .expect("villager inventory");
            inventory.new(
                item_id,
                "Copper Training Axe".to_string(),
                1,
                &item_templates,
            );
            let weapon = inventory
                .items
                .iter_mut()
                .find(|item| item.id == item_id)
                .expect("villager test weapon");
            weapon.equipped = true;
            weapon.attrs.insert(AttrKey::Damage, AttrVal::Num(20.0));
            drop(inventory);
            *world
                .get_mut::<Position>(villager_entity)
                .expect("villager position") = pos;
            let mut stats = world
                .get_mut::<Stats>(villager_entity)
                .expect("villager stats");
            stats.hp = stats.hp.max(500);
            stats.base_hp = stats.base_hp.max(500);
            stats.base_def = 0;
        }

        (villager_entity, villager_id)
    }

    fn safe_logout_fixture(name: &str) -> (HeadlessGame, Position) {
        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", name);
        let sanctuary = game.place_hero_in_own_bound_sanctuary();
        move_nearby_headless_hostiles_away(&mut game, sanctuary);
        // Let the ordinary sanctuary sync and presence reconciliation observe
        // the exact bound Monolith before the first request.
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        (game, sanctuary)
    }

    fn move_nearby_headless_hostiles_away(game: &mut HeadlessGame, hero_pos: Position) {
        use crate::npc::VisibleTarget;
        use crate::safe_logout::SAFE_LOGOUT_HOSTILE_RADIUS;

        let far = far_map_position(game, hero_pos);
        let world = game.app.world_mut();
        let entities = {
            let mut query = world.query_filtered::<(
                Entity,
                &PlayerId,
                &Position,
                &Subclass,
                &State,
                &Stats,
                Option<&StateDead>,
            ), (With<SubclassNPC>, With<VisibleTarget>)>();
            query
                .iter(world)
                .filter(|(_, owner, pos, subclass, state, stats, dead)| {
                    owner.is_npc()
                        && **subclass == Subclass::Npc
                        && state.is_alive()
                        && dead.is_none()
                        && stats.hp > 0
                        && Map::distance((hero_pos.x, hero_pos.y), (pos.x, pos.y))
                            <= SAFE_LOGOUT_HOSTILE_RADIUS
                })
                .map(|(entity, ..)| entity)
                .collect::<Vec<_>>()
        };
        for entity in entities {
            *world
                .get_mut::<Position>(entity)
                .expect("headless hostile position") = far;
        }
    }

    fn begin_safe_logout(game: &mut HeadlessGame) -> i32 {
        game.request_safe_logout();
        game.tick(1);
        let record = game.player_presence_record();
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending),
            "safe-logout request was not accepted: {record:?}"
        );
        game.safe_logout_start_tick()
            .expect("accepted safe-logout start tick")
    }

    fn move_one_tile(position: Position) -> Position {
        Position {
            x: if position.x < 49 {
                position.x + 1
            } else {
                position.x - 1
            },
            y: position.y,
        }
    }

    fn far_map_position(game: &HeadlessGame, position: Position) -> Position {
        let map = game.map();
        let candidates = [
            Position { x: 0, y: 0 },
            Position {
                x: map.width - 1,
                y: map.height - 1,
            },
            Position {
                x: 0,
                y: map.height - 1,
            },
            Position {
                x: map.width - 1,
                y: 0,
            },
        ];
        candidates
            .into_iter()
            .max_by_key(|candidate| {
                Map::distance((position.x, position.y), (candidate.x, candidate.y))
            })
            .expect("map corner")
    }

    fn expire_safe_logout_activity_cooldown(game: &mut HeadlessGame) {
        game.app.world_mut().resource_mut::<GameTick>().0 +=
            crate::safe_logout::SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS + 1;
    }

    fn spawn_safe_logout_non_hostile_candidate(
        game: &mut HeadlessGame,
        owner_player_id: i32,
        subclass: Subclass,
        template: &str,
        position: Position,
    ) -> i32 {
        use crate::ids::{EntityObjMap, Ids};
        use crate::npc::VisibleTarget;

        let hero_id = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(&Id, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == game.player_id)
                .map(|(id, _)| id.0)
                .expect("headless hero id")
        };
        let world = game.app.world_mut();
        let obj_id = world.resource_mut::<Ids>().new_obj_id();
        let entity = world
            .spawn((
                Id(obj_id),
                PlayerId(owner_player_id),
                position,
                Template(template.to_string()),
                subclass,
                SubclassNPC,
                State::None,
                Stats {
                    hp: 20,
                    stamina: None,
                    mana: None,
                    base_hp: 20,
                    base_stamina: None,
                    base_mana: None,
                    base_def: 0,
                    damage_range: Some(1),
                    base_damage: Some(1),
                    base_speed: Some(1),
                    base_vision: Some(8),
                },
                VisibleTarget::new(hero_id),
            ))
            .id();
        world.resource_mut::<Ids>().new_obj(obj_id, owner_player_id);
        world.resource_mut::<EntityObjMap>().new_obj(obj_id, entity);
        obj_id
    }

    #[test]
    fn safe_logout_checkpoint1_presence_lifecycle_is_explicit_and_idempotent() {
        let (mut game, _) = safe_logout_fixture("SafeLogoutPresenceBot");

        // Requirements 1-6: only an explicit request can leave ordinary online
        // presence; repeated connection edges remain idempotent.
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        assert_ne!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        let disconnected = game.player_presence_record();
        game.disconnect_player();
        game.tick(2);
        assert_eq!(game.player_presence_record(), disconnected);

        game.reconnect_player();
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        let online = game.player_presence_record();
        game.reconnect_player();
        game.tick(2);
        assert_eq!(game.player_presence_record(), online);

        // A socket gap that opens and closes between ECS updates is still
        // observed at the authenticated Login boundary and cannot leave an
        // old countdown running on the replacement connection.
        let (mut gap_game, _) = safe_logout_fixture("SafeLogoutSocketGapBot");
        begin_safe_logout(&mut gap_game);
        gap_game.disconnect_player();
        gap_game.reconnect_player_with_login();
        gap_game.tick(3);
        assert_eq!(
            gap_game.player_presence(),
            Some(PlayerWorldPresence::Online)
        );
        assert_eq!(
            gap_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Disconnected)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_sanctuary_eligibility_is_owned_and_fail_closed() {
        use crate::ids::EntityObjMap;

        let (mut game, own_sanctuary) = safe_logout_fixture("SafeLogoutSanctuaryBot");
        let player_id = game.player_id;

        // Requirement 7: the player's exact bound sanctuary qualifies.
        begin_safe_logout(&mut game);
        game.cancel_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Manual)
        );

        // Requirement 8: being elsewhere does not qualify.
        let outside = far_map_position(&game, own_sanctuary);
        game.move_hero_for_test(outside);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::OutsideOwnSanctuary)
        );

        // Requirement 9: another connected player's bound sanctuary is not a
        // fallback for the owner under test.
        let helper_id = spawn_connected_helper(&mut game, "ForeignSanctuaryBot");
        let foreign_sanctuary = {
            let world = game.app.world_mut();
            let mut heroes =
                world.query_filtered::<(&PlayerId, &BoundMonolith), With<SubclassHero>>();
            let foreign_bound = heroes
                .iter(world)
                .find(|(owner, _)| owner.0 == helper_id)
                .map(|(_, bound)| bound.id)
                .expect("helper bound Monolith");
            let entity = world
                .resource::<EntityObjMap>()
                .get_entity(foreign_bound)
                .expect("helper Monolith entity");
            *world
                .get::<Position>(entity)
                .expect("helper Monolith position")
        };
        game.move_hero_for_test(foreign_sanctuary);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::OutsideOwnSanctuary)
        );

        game.move_hero_for_test(own_sanctuary);
        let (hero_entity, own_monolith_entity) = {
            let world = game.app.world_mut();
            let mut heroes =
                world.query_filtered::<(Entity, &PlayerId, &BoundMonolith), With<SubclassHero>>();
            let (hero_entity, bound_id) = heroes
                .iter(world)
                .find(|(_, owner, _)| owner.0 == player_id)
                .map(|(entity, _, bound)| (entity, bound.id))
                .expect("owner hero binding");
            let monolith_entity = world
                .resource::<EntityObjMap>()
                .get_entity(bound_id)
                .expect("owner Monolith entity");
            (hero_entity, monolith_entity)
        };

        // Requirement 10: a missing binding rejects without a nearest-zone
        // fallback.
        let binding = game
            .app
            .world_mut()
            .entity_mut(hero_entity)
            .take::<BoundMonolith>()
            .expect("owner binding");
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::MissingBoundMonolith)
        );
        game.app.world_mut().entity_mut(hero_entity).insert(binding);

        // Requirement 11: removing the live Monolith also removes its synced
        // zone; the stale entity/id is not accepted.
        let monolith = game
            .app
            .world_mut()
            .entity_mut(own_monolith_entity)
            .take::<Monolith>()
            .expect("owner Monolith component");
        game.tick(1);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::MissingSanctuaryZone)
        );
        game.app
            .world_mut()
            .entity_mut(own_monolith_entity)
            .insert(monolith);
        game.tick(1);

        // A stale bound position and a dead bound Monolith both fail closed.
        game.app
            .world_mut()
            .get_mut::<BoundMonolith>(hero_entity)
            .expect("owner binding")
            .pos = move_one_tile(own_sanctuary);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::SanctuaryInvalid)
        );
        game.place_hero_in_own_bound_sanctuary();
        *game
            .app
            .world_mut()
            .get_mut::<State>(own_monolith_entity)
            .expect("Monolith state") = State::Dead;
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::SanctuaryInvalid)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_dead_and_true_death_never_protect() {
        // Requirements 12-13: ordinary death rejects, and the presence of a
        // fresh True Death marker rejects immediately rather than waiting for
        // the delayed final run cleanup.
        let (mut dead_game, _) = safe_logout_fixture("SafeLogoutDeadBot");
        let dead_entity = {
            let world = dead_game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == dead_game.player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let dead_at = dead_game.game_tick();
        dead_game.app.world_mut().entity_mut(dead_entity).insert((
            State::Dead,
            StateDead {
                dead_at,
                killer: "Eligibility test".to_string(),
            },
        ));
        dead_game
            .app
            .world_mut()
            .get_mut::<Stats>(dead_entity)
            .expect("hero stats")
            .hp = 0;
        dead_game.request_safe_logout();
        dead_game.tick(1);
        assert_eq!(
            dead_game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::HeroDied)
        );
        assert_ne!(
            dead_game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );

        let (mut true_death_game, _) = safe_logout_fixture("SafeLogoutTrueDeathBot");
        let true_death_entity = {
            let world = true_death_game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == true_death_game.player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let true_death_at = true_death_game.game_tick();
        let state_dead = StateDead {
            dead_at: true_death_at,
            killer: "True Death eligibility test".to_string(),
        };
        true_death_game
            .app
            .world_mut()
            .entity_mut(true_death_entity)
            .insert((TrueDeath { true_death_at }, State::Dead, state_dead));
        true_death_game.request_safe_logout();
        true_death_game.tick(1);
        assert_eq!(
            true_death_game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::TrueDeath)
        );
        assert_ne!(
            true_death_game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_activity_hostility_and_crisis_eligibility() {
        use crate::ids::EntityObjMap;

        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutEligibilityBot");

        // Requirements 14-15: both authoritative outgoing combat and incoming
        // damage impose the configured cooldown.
        game.record_player_combat_for_test();
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::RecentCombat)
        );
        expire_safe_logout_activity_cooldown(&mut game);
        game.damage_hero_for_test(1);
        game.tick(1);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::RecentDamage)
        );
        expire_safe_logout_activity_cooldown(&mut game);

        // Requirement 16: a live immediate threat blocks the request.
        let hostile = game.spawn_safe_logout_test_hostile(sanctuary);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::HostileNearby)
        );

        // A still-live threat cannot disappear from eligibility merely because
        // object-map cleanup raced ahead of deferred entity cleanup.
        let hostile_entity = game
            .app
            .world()
            .resource::<EntityObjMap>()
            .get_entity(hostile)
            .expect("registered hostile");
        game.app
            .world_mut()
            .resource_mut::<EntityObjMap>()
            .remove_obj(hostile);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::HostileNearby)
        );
        game.app
            .world_mut()
            .resource_mut::<EntityObjMap>()
            .new_obj(hostile, hostile_entity);

        // Requirement 17: the same registered entity no longer blocks once it
        // is a dead corpse.
        game.kill_safe_logout_test_hostile(hostile);
        begin_safe_logout(&mut game);
        game.cancel_safe_logout();
        game.tick(1);

        // Requirements 18-19 and the cross-player friendly-unit rule: fixtures
        // satisfy the hostile query's shape but are excluded by ownership or
        // subclass, just like real villagers and merchants.
        let player_id = game.player_id;
        let villager = spawn_safe_logout_non_hostile_candidate(
            &mut game,
            player_id,
            Subclass::Villager,
            "Villager",
            sanctuary,
        );
        let merchant = spawn_safe_logout_non_hostile_candidate(
            &mut game,
            crate::constants::MERCHANT_PLAYER_ID,
            Subclass::Merchant,
            "Merchant",
            sanctuary,
        );
        let friendly_other = spawn_safe_logout_non_hostile_candidate(
            &mut game,
            player_id + 1,
            Subclass::Npc,
            "Wolf",
            sanctuary,
        );
        begin_safe_logout(&mut game);
        game.cancel_safe_logout();
        game.tick(1);

        // Requirement 21: every explicit pre-assault phase remains eligible;
        // the safe-logout system must not accidentally narrow this list when
        // crisis phases evolve.
        for phase in [
            CrisisPhase::Dormant,
            CrisisPhase::Signs,
            CrisisPhase::Pressure,
            CrisisPhase::Preparing,
            CrisisPhase::AssaultReady,
        ] {
            let current_tick = game.game_tick();
            let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
            let crisis = crises.get_mut(&game.player_id).expect("personal crisis");
            crisis.phase = phase;
            crisis.phase_online_ticks = 0;
            crisis.last_evaluated_tick = current_tick;
            begin_safe_logout(&mut game);
            game.cancel_safe_logout();
            game.tick(1);
        }

        // Requirement 20: committed assault state always rejects.
        game.app
            .world_mut()
            .resource_mut::<SettlementCrisisState>()
            .get_mut(&game.player_id)
            .expect("personal crisis")
            .phase = CrisisPhase::AssaultActive;
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::AssaultActive)
        );

        for obj_id in [hostile, villager, merchant, friendly_other] {
            game.remove_safe_logout_test_hostile(obj_id);
        }
    }

    #[test]
    fn safe_logout_checkpoint1_deterministic_move_cancel_then_exact_completion() {
        use crate::safe_logout::SAFE_LOGOUT_COUNTDOWN_TICKS;

        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutCountdownBot");

        // Requirements 22-28 plus the first mandated deterministic scenario.
        let first_start = begin_safe_logout(&mut game);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::AlreadyPending)
        );
        assert_eq!(game.safe_logout_start_tick(), Some(first_start));

        game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS / 2) as u32);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending)
        );
        let evaluated_tick = game.game_tick();
        game.app.world_mut().resource_mut::<GameTick>().0 = evaluated_tick - 1;
        game.tick(1);
        assert_eq!(game.game_tick(), evaluated_tick);
        assert_eq!(game.safe_logout_start_tick(), Some(first_start));
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending),
            "evaluating the same authoritative tick twice must not advance the countdown"
        );
        game.move_hero_for_test(move_one_tile(sanctuary));
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Moved)
        );

        game.place_hero_in_own_bound_sanctuary();
        let second_start = begin_safe_logout(&mut game);
        assert!(second_start > first_start);
        game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending),
            "countdown must not complete one tick early"
        );
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        assert_eq!(game.safe_logout_start_tick(), None);

        // Re-evaluation and a duplicate request cannot produce a second
        // transition or restart pending state.
        game.tick(2);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::AlreadyProtected)
        );

        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected),
            "a socket close after explicit completion preserves the completed handoff"
        );
        game.reconnect_player();
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
    }

    #[test]
    fn safe_logout_checkpoint1_combat_damage_and_hostile_entry_cancel() {
        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutCancelBot");

        // Requirements 30-34: the aggregate server combat hook is shared by
        // attacks, damaging abilities, and combos; incoming damage and a newly
        // arrived hostile have their own cancellation reasons.
        begin_safe_logout(&mut game);
        game.record_player_combat_for_test();
        game.tick(1);
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::EnteredCombat)
        );

        expire_safe_logout_activity_cooldown(&mut game);
        begin_safe_logout(&mut game);
        game.damage_hero_for_test(1);
        game.tick(1);
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::TookDamage)
        );

        expire_safe_logout_activity_cooldown(&mut game);
        let hostile = game.spawn_safe_logout_test_hostile(far_map_position(&game, sanctuary));
        begin_safe_logout(&mut game);
        game.move_safe_logout_test_hostile(hostile, sanctuary);
        game.tick(1);
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::HostileNearby)
        );
        game.remove_safe_logout_test_hostile(hostile);
    }

    #[test]
    fn safe_logout_checkpoint1_production_incoming_damage_cancels() {
        use big_brain::prelude::{ActionState, Actor};

        use crate::common::AttackTarget;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;

        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutProductionDamageBot");
        begin_safe_logout(&mut game);

        let hostile_id = game.spawn_safe_logout_test_hostile(sanctuary);
        let (hero_entity, hero_id, hp_before, hostile_entity) = {
            let world = game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &Id, &Stats), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_stats) =
                heroes.iter(world).next().expect("headless hero");
            let hostile_entity = world
                .resource::<EntityObjMap>()
                .get_entity(hostile_id)
                .expect("production damage attacker");
            (hero_entity, hero_id.0, hero_stats.hp, hostile_entity)
        };
        {
            let world = game.app.world_mut();
            world
                .get_mut::<Stats>(hostile_entity)
                .expect("production damage attacker stats")
                .base_damage = Some(30);
            world
                .get_mut::<VisibleTarget>(hostile_entity)
                .expect("production damage attacker target")
                .target = hero_id;
            world.spawn((Actor(hostile_entity), ActionState::Requested, AttackTarget));
        }

        game.tick(1);

        let hero_stats = game
            .world()
            .get::<Stats>(hero_entity)
            .expect("headless hero stats after production attack");
        assert!(
            hero_stats.hp < hp_before,
            "the production combat system must apply incoming damage"
        );
        assert!(game.world().get::<StateDead>(hero_entity).is_none());
        assert!(
            game.world().get::<LastDamageTick>(hero_entity).is_some(),
            "the production damage writer must stamp the authoritative hero"
        );
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::TookDamage),
            "damage is evaluated before the same attacker's proximity"
        );
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
    }

    #[test]
    fn safe_logout_checkpoint1_real_attack_ability_and_combo_paths_cancel() {
        use crate::combat::{AttackType, ComboTracker};
        use crate::ids::EntityObjMap;
        use crate::player::PlayerEvents;

        fn hero_id(game: &mut HeadlessGame) -> i32 {
            let player_id = game.player_id;
            let world = game.app.world_mut();
            let mut heroes = world.query_filtered::<(&Id, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(id, _)| id.0)
                .expect("headless hero id")
        }

        fn run_event(
            game: &mut HeadlessGame,
            event_id: i32,
            event: PlayerEvent,
        ) -> SafeLogoutCancelReason {
            game.app
                .world_mut()
                .resource_mut::<PlayerEvents>()
                .insert(event_id, event);
            game.tick(1);
            game.safe_logout_cancel_reason()
                .expect("accepted combat event should cancel safe logout")
        }

        // Requirement 30: a successfully accepted ordinary attack records
        // authoritative activity before the PostUpdate completion check.
        let (mut attack_game, sanctuary) = safe_logout_fixture("SafeLogoutRealAttackBot");
        begin_safe_logout(&mut attack_game);
        let attack_source = hero_id(&mut attack_game);
        attack_game
            .app
            .world_mut()
            .resource_mut::<PlayerEvents>()
            .insert(
                -29,
                PlayerEvent::Attack {
                    player_id: HEADLESS_PLAYER_ID,
                    attack_type: "quick".to_string(),
                    source_id: attack_source,
                    target_id: i32::MAX,
                },
            );
        attack_game.tick(1);
        assert_eq!(
            attack_game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending),
            "a rejected target id is not authoritative combat activity"
        );
        assert_eq!(attack_game.safe_logout_cancel_reason(), None);

        let attack_target = attack_game.spawn_safe_logout_test_hostile(sanctuary);
        assert_eq!(
            run_event(
                &mut attack_game,
                -30,
                PlayerEvent::Attack {
                    player_id: HEADLESS_PLAYER_ID,
                    attack_type: "quick".to_string(),
                    source_id: attack_source,
                    target_id: attack_target,
                },
            ),
            SafeLogoutCancelReason::EnteredCombat
        );

        // Requirement 31: a damaging class ability follows the same accepted
        // server path; this uses the Warrior's adjacent Guard Bash.
        let (mut ability_game, sanctuary) = safe_logout_fixture("SafeLogoutRealAbilityBot");
        begin_safe_logout(&mut ability_game);
        let ability_target = ability_game.spawn_safe_logout_test_hostile(sanctuary);
        let ability_source = hero_id(&mut ability_game);
        assert_eq!(
            run_event(
                &mut ability_game,
                -31,
                PlayerEvent::Ability {
                    player_id: HEADLESS_PLAYER_ID,
                    ability_id: "shield_bash".to_string(),
                    source_id: ability_source,
                    target_id: Some(ability_target),
                },
            ),
            SafeLogoutCancelReason::EnteredCombat
        );

        // Requirement 32: a ready damaging combo must also cancel through the
        // real combo event rather than a test-only activity shortcut.
        let (mut combo_game, sanctuary) = safe_logout_fixture("SafeLogoutRealComboBot");
        begin_safe_logout(&mut combo_game);
        let combo_target = combo_game.spawn_safe_logout_test_hostile(sanctuary);
        let combo_source = hero_id(&mut combo_game);
        let combo_hero = combo_game
            .app
            .world()
            .resource::<EntityObjMap>()
            .get_entity(combo_source)
            .expect("headless hero entity");
        combo_game
            .app
            .world_mut()
            .entity_mut(combo_hero)
            .insert(ComboTracker {
                target_id: combo_target,
                attacks: vec![AttackType::Quick, AttackType::Quick],
            });
        assert_eq!(
            run_event(
                &mut combo_game,
                -32,
                PlayerEvent::Combo {
                    player_id: HEADLESS_PLAYER_ID,
                    source_id: combo_source,
                    target_id: combo_target,
                    combo_type: "Hamstring".to_string(),
                },
            ),
            SafeLogoutCancelReason::EnteredCombat
        );
    }

    #[test]
    fn safe_logout_checkpoint1_manual_disconnect_sanctuary_and_death_cancel_once() {
        // Requirements 29, 35-44. Movement is covered by the deterministic
        // scenario; these fixtures exercise the remaining state sources and
        // verify cancellation has no economy/crisis side effects.
        let (mut manual_game, _) = safe_logout_fixture("SafeLogoutManualBot");
        let pressure_before = manual_game
            .settlement_crisis()
            .expect("personal crisis")
            .pressure;
        let rewards_before = manual_game.personal_crises_resolved();
        begin_safe_logout(&mut manual_game);
        manual_game.cancel_safe_logout();
        manual_game.tick(1);
        assert_eq!(
            manual_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Manual)
        );
        let cancelled = manual_game.player_presence_record();
        manual_game.cancel_safe_logout();
        manual_game.tick(1);
        assert_eq!(manual_game.player_presence_record(), cancelled);
        assert_eq!(
            manual_game
                .settlement_crisis()
                .expect("personal crisis")
                .pressure,
            pressure_before
        );
        assert_eq!(manual_game.personal_crises_resolved(), rewards_before);

        let (mut disconnect_game, _) = safe_logout_fixture("SafeLogoutDisconnectBot");
        begin_safe_logout(&mut disconnect_game);
        disconnect_game.disconnect_player();
        disconnect_game.tick(1);
        assert_eq!(
            disconnect_game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        assert_eq!(
            disconnect_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Disconnected)
        );

        let (mut invalid_game, _) = safe_logout_fixture("SafeLogoutInvalidZoneBot");
        let (hero_entity, monolith_entity) = {
            use crate::ids::EntityObjMap;
            let world = invalid_game.app.world_mut();
            let mut heroes =
                world.query_filtered::<(Entity, &PlayerId, &BoundMonolith), With<SubclassHero>>();
            let (hero, monolith_id) = heroes
                .iter(world)
                .find(|(_, owner, _)| owner.0 == invalid_game.player_id)
                .map(|(entity, _, bound)| (entity, bound.id))
                .expect("headless binding");
            let monolith = world
                .resource::<EntityObjMap>()
                .get_entity(monolith_id)
                .expect("bound Monolith");
            (hero, monolith)
        };
        begin_safe_logout(&mut invalid_game);
        invalid_game
            .app
            .world_mut()
            .get_mut::<BoundMonolith>(hero_entity)
            .expect("binding")
            .pos = move_one_tile(
            *invalid_game
                .app
                .world()
                .get::<Position>(monolith_entity)
                .expect("Monolith position"),
        );
        invalid_game.tick(1);
        assert_eq!(
            invalid_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::SanctuaryInvalid)
        );

        let (mut left_game, sanctuary) = safe_logout_fixture("SafeLogoutLeftZoneBot");
        let (left_hero, left_monolith) = {
            use crate::ids::EntityObjMap;
            let world = left_game.app.world_mut();
            let mut heroes =
                world.query_filtered::<(Entity, &PlayerId, &BoundMonolith), With<SubclassHero>>();
            let (hero, bound_id) = heroes
                .iter(world)
                .find(|(_, owner, _)| owner.0 == left_game.player_id)
                .map(|(entity, _, bound)| (entity, bound.id))
                .expect("headless binding");
            let monolith = world
                .resource::<EntityObjMap>()
                .get_entity(bound_id)
                .expect("bound Monolith");
            (hero, monolith)
        };
        begin_safe_logout(&mut left_game);
        let far = far_map_position(&left_game, sanctuary);
        *left_game
            .app
            .world_mut()
            .get_mut::<Position>(left_monolith)
            .expect("Monolith position") = far;
        left_game
            .app
            .world_mut()
            .get_mut::<BoundMonolith>(left_hero)
            .expect("binding")
            .pos = far;
        left_game.tick(1);
        assert_eq!(
            left_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::LeftSanctuary)
        );

        let (mut dead_game, _) = safe_logout_fixture("SafeLogoutPendingDeathBot");
        begin_safe_logout(&mut dead_game);
        let dead_entity = {
            let world = dead_game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == dead_game.player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let dead_at = dead_game.game_tick();
        dead_game.app.world_mut().entity_mut(dead_entity).insert((
            State::Dead,
            StateDead {
                dead_at,
                killer: "Cancellation test".to_string(),
            },
        ));
        dead_game.tick(1);
        assert_eq!(
            dead_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::HeroDied)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_same_tick_danger_wins_over_completion() {
        use crate::safe_logout::SAFE_LOGOUT_COUNTDOWN_TICKS;

        // Requirements 45-47: arrange each danger on the exact update that
        // would otherwise complete the absolute-tick countdown.
        let (mut assault_game, _) = safe_logout_fixture("SafeLogoutRaceAssaultBot");
        let preferred_tick = set_personal_assault_ready(&mut assault_game);
        let pre_request_tick = preferred_tick - SAFE_LOGOUT_COUNTDOWN_TICKS - 1;
        {
            use crate::game::ASSAULT_READY_GRACE_TICKS;

            let world = assault_game.app.world_mut();
            world.resource_mut::<GameTick>().0 = pre_request_tick;
            let mut crises = world.resource_mut::<SettlementCrisisState>();
            let crisis = crises
                .get_mut(&assault_game.player_id)
                .expect("ready personal crisis");
            crisis.phase_online_ticks = ASSAULT_READY_GRACE_TICKS - SAFE_LOGOUT_COUNTDOWN_TICKS - 1;
            crisis.last_evaluated_tick = pre_request_tick;
        }
        let requested_tick = begin_safe_logout(&mut assault_game);
        assert_eq!(requested_tick + SAFE_LOGOUT_COUNTDOWN_TICKS, preferred_tick);
        assault_game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
        assert_eq!(
            assault_game
                .settlement_crisis()
                .expect("ready personal crisis")
                .phase,
            CrisisPhase::AssaultReady
        );
        assert!(assault_game.crisis_assault_units().is_empty());
        assault_game.tick(1);
        assert_eq!(
            assault_game
                .settlement_crisis()
                .expect("launched personal crisis")
                .phase,
            CrisisPhase::AssaultActive
        );
        assert!(!assault_game.crisis_assault_units().is_empty());
        assert_eq!(
            assault_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::AssaultStarted)
        );

        let (mut damage_game, _) = safe_logout_fixture("SafeLogoutRaceDamageBot");
        begin_safe_logout(&mut damage_game);
        damage_game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
        damage_game.damage_hero_for_test(1);
        damage_game.tick(1);
        assert_eq!(
            damage_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::TookDamage)
        );

        let (mut death_game, _) = safe_logout_fixture("SafeLogoutRaceTrueDeathBot");
        begin_safe_logout(&mut death_game);
        death_game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
        let hero_entity = {
            let world = death_game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == death_game.player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let true_death_at = death_game.game_tick() - 101;
        let state_dead = StateDead {
            dead_at: true_death_at,
            killer: "True Death ordering test".to_string(),
        };
        death_game.app.world_mut().entity_mut(hero_entity).insert((
            TrueDeath { true_death_at },
            State::Dead,
            state_dead,
        ));
        death_game.tick(1);
        assert_ne!(
            death_game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_fresh_run_and_cleanup_clear_stale_presence() {
        use crate::safe_logout::remove_player_presence_for_run_cleanup;

        // Requirements 48-49: new-run setup overwrites both stale pending and
        // stale protected records before gameplay begins.
        for stale_state in [
            PlayerWorldPresence::SafeLogoutPending,
            PlayerWorldPresence::OfflineProtected,
        ] {
            let mut game = HeadlessGame::new(20_000);
            let mut stale = PlayerPresenceRecord::new(true);
            stale.state = stale_state;
            stale.safe_logout_requested_tick = Some(123);
            stale.safe_logout_start_position = Some(Position { x: 1, y: 1 });
            game.app
                .world_mut()
                .resource_mut::<PlayerWorldPresenceState>()
                .players
                .insert(HEADLESS_PLAYER_ID, stale);
            game.spawn_hero("Warrior", "SafeLogoutFreshRunBot");
            let fresh = game
                .player_presence_record()
                .expect("fresh run presence record");
            assert_eq!(fresh.state, PlayerWorldPresence::Online);
            assert_eq!(fresh.safe_logout_requested_tick, None);
            assert_eq!(fresh.safe_logout_start_position, None);
        }

        // Requirements 50-51: cleanup is player-scoped and repeatable.
        let (mut game, _) = safe_logout_fixture("SafeLogoutCleanupOwnerBot");
        let helper_id = spawn_connected_helper(&mut game, "SafeLogoutCleanupNeighborBot");
        let neighbor_before = game
            .app
            .world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&helper_id)
            .cloned()
            .expect("neighbor presence");
        let tick = game.game_tick();
        {
            let mut presence = game
                .app
                .world_mut()
                .resource_mut::<PlayerWorldPresenceState>();
            remove_player_presence_for_run_cleanup(game.player_id, tick, &mut presence);
            remove_player_presence_for_run_cleanup(game.player_id, tick, &mut presence);
        }
        let presence = game.app.world().resource::<PlayerWorldPresenceState>();
        assert!(!presence.players.contains_key(&game.player_id));
        assert_eq!(presence.players.get(&helper_id), Some(&neighbor_before));
    }

    #[test]
    fn safe_logout_checkpoint1_active_assault_cancels_then_disconnect_continues() {
        let (mut game, _) = safe_logout_fixture("SafeLogoutAssaultContinuityBot");
        let preferred_tick = set_personal_assault_ready(&mut game);
        game.app.world_mut().resource_mut::<GameTick>().0 = preferred_tick - 20;
        begin_safe_logout(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);

        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::AssaultStarted)
        );
        let active = game.settlement_crisis().expect("active assault");
        assert_eq!(active.phase, CrisisPhase::AssaultActive);
        let assault_id = active.assault_id.expect("assault id");
        let generation = active.assault_spawn_generation;
        let unit_ids = game
            .crisis_assault_units()
            .into_iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        assert!(!unit_ids.is_empty());

        // Requirements 52-54 and the second mandated deterministic scenario:
        // ordinary disconnect changes only presence; the committed assault,
        // identity, generation, and live entities remain.
        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        let disconnected = game.settlement_crisis().expect("offline assault");
        assert_eq!(disconnected.phase, CrisisPhase::AssaultActive);
        assert_eq!(disconnected.assault_id, Some(assault_id));
        assert_eq!(disconnected.assault_spawn_generation, generation);
        assert_eq!(
            game.crisis_assault_units()
                .into_iter()
                .map(|unit| unit.obj_id)
                .collect::<Vec<_>>(),
            unit_ids
        );
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
    fn checkpoint4_true_death_clears_status_before_a_fresh_run() {
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
        {
            let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
            crises.insert(
                player_id,
                SettlementCrisis {
                    kind: CrisisKind::Goblin,
                    phase: CrisisPhase::AssaultActive,
                    pressure: 55,
                    phase_started_tick: current_tick,
                    online_active_ticks: 50,
                    phase_online_ticks: 50,
                    warning_active: true,
                    last_evaluated_tick: current_tick,
                    assault_id: Some(99),
                    assault_started_tick: Some(current_tick),
                    assault_unit_ids: vec![999_999],
                    assault_spawn_generation: 1,
                    ..SettlementCrisis::default()
                },
            );
            crises.insert(
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
        }
        game.tick(1);
        let old_statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(
            old_statuses
                .last()
                .and_then(|status| status.phase.as_deref()),
            Some("assault_active")
        );
        assert_eq!(
            old_statuses
                .last()
                .and_then(|status| status.remaining_attackers),
            Some(1)
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
        let cleanup_statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(cleanup_statuses.len(), 1);
        assert!(!cleanup_statuses[0].exists);
        assert!(cleanup_statuses[0].phase.is_none());
        assert!(cleanup_statuses[0].pressure.is_none());
        assert!(cleanup_statuses[0].remaining_attackers.is_none());
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
        let fresh_statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(fresh_statuses.len(), 1);
        assert!(fresh_statuses[0].exists);
        assert_eq!(fresh_statuses[0].phase.as_deref(), Some("dormant"));
        assert_eq!(fresh_statuses[0].pressure, Some(0));
        assert_eq!(fresh_statuses[0].remaining_attackers, None);
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
        assert!(fresh_crisis.assault_defeated_unit_ids.is_empty());
        assert_eq!(fresh_crisis.assault_spawn_generation, 0);
        assert!(!fresh_crisis.assault_recovery_required);
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
    fn checkpoint4_normal_packet_progression_and_runtime_telemetry_headless() {
        use crate::constants::TICKS_PER_SEC;

        let mut game = HeadlessGame::new(30_000);
        game.spawn_hero("Warrior", "CrisisPacketProgressionBot");
        prepare_full_crisis_progression_facts(&mut game);

        game.tick(1);
        for seconds in [60, 120, 180] {
            game.app.world_mut().resource_mut::<GameTick>().0 =
                game.game_tick() + seconds * TICKS_PER_SEC;
            game.tick(1);
        }

        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultReady
        );
        let preferred_tick = next_preferred_assault_tick(game.game_tick());
        {
            use crate::game::ASSAULT_READY_GRACE_TICKS;

            let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
            let crisis = crises.get_mut(&game.player_id).unwrap();
            crisis.phase_online_ticks = 0;
            crisis.last_evaluated_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;
        }
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched_units = game.crisis_assault_units();
        assert_eq!(launched_units.len(), 3);

        for unit in launched_units {
            kill_assault_unit_through_normal_combat(&mut game, unit.obj_id);
        }
        game.tick(2);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );

        let statuses = crisis_statuses(game.take_crisis_status_packets());
        let phases = crisis_phase_sequence(&statuses);
        assert_eq!(
            phases,
            vec![
                "dormant",
                "signs",
                "pressure",
                "preparing",
                "assault_ready",
                "assault_active",
                "resolved",
            ]
        );
        let remaining = statuses
            .iter()
            .filter(|status| status.assault_active)
            .filter_map(|status| status.remaining_attackers)
            .collect::<Vec<_>>();
        assert!(remaining.starts_with(&[3]));
        assert!(remaining.windows(2).all(|counts| counts[1] <= counts[0]));
        assert!(remaining.iter().any(|remaining| *remaining < 3));
        assert_eq!(statuses.iter().filter(|status| status.resolved).count(), 1);

        let telemetry = game.crisis_telemetry();
        assert_eq!(telemetry.highest_phase, "resolved");
        assert!(telemetry.signs_tick.is_some());
        assert!(telemetry.pressure_tick.is_some());
        assert!(telemetry.preparing_tick.is_some());
        assert!(telemetry.assault_ready_tick.is_some());
        assert!(telemetry.assault_active_tick.is_some());
        assert!(telemetry.resolved_tick.is_some());
        assert_eq!(telemetry.assaults_launched, 1);
        assert_eq!(telemetry.assaults_resolved, 1);
        assert_eq!(telemetry.units_remaining, 0);
        assert_eq!(telemetry.status_packets_sent as usize, statuses.len());
        assert_eq!(telemetry.login_snapshots_sent, 1);
        assert_eq!(telemetry.duplicate_assaults, 0);

        let metrics = game.metrics();
        assert_eq!(metrics.crisis_highest_phase, "resolved");
        assert_eq!(metrics.crisis_final_phase, "resolved");
        assert_eq!(metrics.crisis_assaults_launched, 1);
        assert_eq!(metrics.crisis_assaults_resolved, 1);
        assert_eq!(metrics.personal_crisis_automatic_dusk_hordes, 0);
        assert!(metrics.crisis_invariants_ok);

        println!(
            "checkpoint4_normal_packet_progression phases={phases:?} remaining_attackers={remaining:?} status_packets={} login_snapshots={} duplicate_assaults={}",
            telemetry.status_packets_sent,
            telemetry.login_snapshots_sent,
            telemetry.duplicate_assaults,
        );
    }

    #[test]
    fn checkpoint4_active_disconnect_reconnect_sends_one_current_snapshot() {
        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "CrisisReconnectPacketBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let _ = game.take_crisis_status_packets();

        let units_before = game.crisis_assault_units();
        game.disconnect_player();
        game.tick(8);
        let units_offline = game.crisis_assault_units();
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive
        );
        assert_eq!(
            units_offline
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<Vec<_>>(),
            units_before
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<Vec<_>>()
        );

        game.start_packet_capture();
        game.reconnect_player_with_login();
        game.tick(8);
        let reconnect_packets = game.finish_packet_capture();
        let repeated_launch_notice = reconnect_packets.iter().any(|packet| {
            matches!(
                packet,
                ResponsePacket::Notice { noticemsg, .. }
                    if noticemsg == "The goblin assault has begun. It will continue if you disconnect."
            )
        });
        assert!(!repeated_launch_notice);

        let statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.phase.as_deref(), Some("assault_active"));
        assert!(status.assault_active);
        assert!(status.continues_while_disconnected);
        assert_eq!(
            status.remaining_attackers,
            Some(units_offline.iter().filter(|unit| !unit.dead).count() as i32)
        );

        let reconnected = game.settlement_crisis().unwrap();
        let units_reconnected = game.crisis_assault_units();
        assert_eq!(reconnected.assault_id, Some(assault_id));
        assert_eq!(reconnected.assault_spawn_generation, generation);
        assert_eq!(
            units_reconnected
                .iter()
                .map(|unit| (unit.obj_id, unit.hp))
                .collect::<Vec<_>>(),
            units_offline
                .iter()
                .map(|unit| (unit.obj_id, unit.hp))
                .collect::<Vec<_>>()
        );
        assert_eq!(game.crisis_telemetry().login_snapshots_sent, 2);
    }

    #[test]
    fn checkpoint4_offline_resolution_reconnect_first_snapshot_is_resolved() {
        let mut game = HeadlessGame::new(30_000);
        game.spawn_hero("Warrior", "CrisisOfflineResolutionPacketBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let helper_player_id = spawn_connected_helper(&mut game, "CrisisResolutionHelper");
        let unit_ids = game
            .crisis_assault_units()
            .iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        let _ = game.take_crisis_status_packets();

        game.disconnect_player();
        game.tick(2);
        for unit_id in unit_ids {
            kill_assault_unit_through_normal_combat_as(&mut game, helper_player_id, unit_id);
        }
        game.tick(2);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        assert!(game.take_crisis_status_packets().is_empty());

        game.reconnect_player_with_login();
        game.tick(8);
        let statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].phase.as_deref(), Some("resolved"));
        assert!(statuses[0].resolved);
        assert!(!statuses[0].assault_active);
        assert_eq!(game.crisis_telemetry().assaults_resolved, 1);
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
        for unit in &units {
            let entity = game
                .world()
                .resource::<crate::ids::EntityObjMap>()
                .get_entity(unit.obj_id)
                .expect("attributed assault entity");
            assert!(game
                .world()
                .get::<crate::common::TaskTarget>(entity)
                .is_none());
            assert!(game
                .world()
                .get::<crate::npc::ItemsToSteal>(entity)
                .is_none());
        }
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
                .assault_defeated_unit_ids
                .len(),
            0
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
    fn checkpoint3_attributed_npc_attack_continues_after_owner_disconnect() {
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
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let unit_ids = launched
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
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
        let action_entity = {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(unit_entity).unwrap() = hero_pos;
            world.get_mut::<Stats>(unit_entity).unwrap().base_damage = Some(30);
            world.get_mut::<VisibleTarget>(unit_entity).unwrap().target = hero_id;
            world
                .spawn((Actor(unit_entity), ActionState::Requested, AttackTarget))
                .id()
        };

        game.disconnect_player();
        game.tick(1);

        assert!(
            game.world().get::<Stats>(hero_entity).unwrap().hp < hp_before,
            "attributed NPC combat must apply damage while its owner is offline"
        );
        assert!(game.world().get::<StateDead>(hero_entity).is_none());
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive
        );
        assert_eq!(
            game.settlement_crisis().unwrap().assault_id,
            Some(assault_id)
        );
        assert_eq!(
            game.settlement_crisis().unwrap().assault_spawn_generation,
            generation
        );
        assert_eq!(
            game.crisis_assault_units()
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<HashSet<_>>(),
            unit_ids
        );
        assert_eq!(
            *game.world().get::<ActionState>(action_entity).unwrap(),
            ActionState::Executing
        );
    }

    #[test]
    fn checkpoint3_stale_cross_owner_action_target_is_rejected_offline() {
        use big_brain::prelude::{ActionState, Actor};

        use crate::common::{AttackTarget, SetAttackTarget};
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;

        let mut game = HeadlessGame::new(20_000);
        let owner_player_id = game.spawn_hero("Warrior", "AssaultTargetOwnerBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let assault_id = game.settlement_crisis().unwrap().assault_id.unwrap();
        let generation = game.settlement_crisis().unwrap().assault_spawn_generation;
        let helper_player_id = spawn_connected_helper(&mut game, "AssaultForeignTargetBot");
        let unit_id = game.crisis_assault_units()[0].obj_id;

        let (unit_entity, helper_entity, helper_id, helper_pos, helper_hp) = {
            let world = game.app.world_mut();
            let unit_entity = world
                .resource::<EntityObjMap>()
                .get_entity(unit_id)
                .unwrap();
            let mut query = world
                .query_filtered::<(Entity, &Id, &PlayerId, &Position, &Stats), With<SubclassHero>>(
                );
            let (helper_entity, helper_id, _, helper_pos, helper_stats) = query
                .iter(world)
                .find(|(_, _, owner, _, _)| owner.0 == helper_player_id)
                .expect("foreign helper hero");
            (
                unit_entity,
                helper_entity,
                helper_id.0,
                *helper_pos,
                helper_stats.hp,
            )
        };

        while (game.game_tick() + 1) % crate::constants::TICKS_PER_SEC == 0 {
            game.tick(1);
        }
        let set_action = {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(unit_entity).unwrap() = helper_pos;
            world.get_mut::<VisibleTarget>(unit_entity).unwrap().target = helper_id;
            world
                .spawn((Actor(unit_entity), ActionState::Requested, SetAttackTarget))
                .id()
        };
        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            *game.world().get::<ActionState>(set_action).unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            game.world()
                .get::<VisibleTarget>(unit_entity)
                .unwrap()
                .target,
            crate::constants::NO_TARGET
        );

        while (game.game_tick() + 1) % crate::constants::TICKS_PER_SEC == 0 {
            game.tick(1);
        }
        let attack_action = {
            let world = game.app.world_mut();
            world.get_mut::<VisibleTarget>(unit_entity).unwrap().target = helper_id;
            world
                .spawn((Actor(unit_entity), ActionState::Requested, AttackTarget))
                .id()
        };
        game.tick(1);

        assert_eq!(
            *game.world().get::<ActionState>(attack_action).unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            game.world().get::<Stats>(helper_entity).unwrap().hp,
            helper_hp
        );
        let crisis = game.settlement_crisis().unwrap();
        assert_eq!(crisis.phase, CrisisPhase::AssaultActive);
        assert_eq!(crisis.assault_id, Some(assault_id));
        assert_eq!(crisis.assault_spawn_generation, generation);
        assert_eq!(owner_player_id, game.player_id());
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
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let unit_ids = launched
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
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
        assert_eq!(crisis.phase, CrisisPhase::AssaultActive);
        assert_eq!(crisis.assault_id, Some(assault_id));
        assert_eq!(crisis.assault_spawn_generation, generation);
        assert_eq!(
            crisis
                .assault_unit_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            unit_ids
        );
        assert_eq!(score_after.enemies_killed, score_before.enemies_killed);
        assert_eq!(score_after.elites_killed, score_before.elites_killed);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert!(game.world().get::<StateDead>(target_entity).is_none());
        assert_eq!(game.crisis_assault_units().len(), 3);
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
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let unit_ids = launched
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
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
        assert_eq!(crisis.phase, CrisisPhase::AssaultActive);
        assert_eq!(crisis.assault_id, Some(assault_id));
        assert_eq!(crisis.assault_spawn_generation, generation);
        assert_eq!(
            crisis
                .assault_unit_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            unit_ids
        );
        assert!(!effects.has(Effect::Bracing));
        assert!(!effects.has(Effect::Dodging));
        assert!(!effects.has(Effect::Parrying));
        assert_eq!(score_after.enemies_killed, score_before.enemies_killed);
        assert_eq!(score_after.elites_killed, score_before.elites_killed);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert_eq!(game.crisis_assault_units().len(), 3);
    }

    #[test]
    fn checkpoint3_connected_helper_resolves_offline_owner_assault_once() {
        use crate::game::CrisisPhase;

        let mut game = HeadlessGame::new(20_000);
        let owner_player_id = game.spawn_hero("Warrior", "AssaultOfflineOwnerBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let unit_ids = launched.assault_unit_ids.clone();
        let helper_player_id = spawn_connected_helper(&mut game, "AssaultOnlineHelperBot");
        let helper_score_before = game
            .world()
            .resource::<RunScoreState>()
            .get(&helper_player_id)
            .cloned()
            .unwrap_or_default();
        game.disconnect_player();

        kill_assault_unit_through_normal_combat_as(&mut game, helper_player_id, unit_ids[0]);
        let partial = game.settlement_crisis().unwrap();
        assert_eq!(partial.phase, CrisisPhase::AssaultActive);
        assert_eq!(partial.assault_id, Some(assault_id));
        assert_eq!(partial.assault_spawn_generation, generation);
        assert_eq!(partial.assault_defeated_unit_ids.len(), 1);
        assert_eq!(game.personal_crises_resolved(), 0);

        for unit_id in unit_ids.iter().skip(1) {
            kill_assault_unit_through_normal_combat_as(&mut game, helper_player_id, *unit_id);
        }

        let helper_score_after = game
            .world()
            .resource::<RunScoreState>()
            .get(&helper_player_id)
            .cloned()
            .unwrap_or_default();
        let crisis = game.settlement_crisis().unwrap();
        assert_eq!(crisis.phase, CrisisPhase::Resolved);
        assert_eq!(crisis.assault_id, Some(assault_id));
        assert_eq!(crisis.assault_spawn_generation, generation);
        assert_eq!(
            helper_score_after.enemies_killed,
            helper_score_before.enemies_killed + unit_ids.len() as i32
        );
        assert_eq!(
            helper_score_after.elites_killed,
            helper_score_before.elites_killed + unit_ids.len() as i32
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            helper_score_after.personal_crises_resolved, 0,
            "crisis completion remains with the attributed owner"
        );

        game.tick(20);
        assert_eq!(game.personal_crises_resolved(), 1);
        game.reconnect_player();
        game.tick(3);
        let reconnected = game.settlement_crisis().unwrap();
        assert_eq!(reconnected.phase, CrisisPhase::Resolved);
        assert_eq!(reconnected.assault_id, Some(assault_id));
        assert_eq!(reconnected.assault_spawn_generation, generation);
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            game.world()
                .resource::<RunScoreState>()
                .get(&owner_player_id)
                .map(|score| score.personal_crises_resolved),
            Some(1)
        );
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
                .assault_defeated_unit_ids
                .len(),
            1
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
    fn checkpoint3_missing_live_unit_stays_committed_and_requires_recovery() {
        use crate::game::CrisisPhase;
        use crate::ids::{EntityObjMap, Ids};

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultMissingUnitBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let units = game.crisis_assault_units();
        let missing_id = units[0].obj_id;
        let surviving_ids = units
            .iter()
            .skip(1)
            .map(|unit| unit.obj_id)
            .collect::<HashSet<_>>();
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

        let committed = game.settlement_crisis().unwrap();
        assert_eq!(committed.phase, CrisisPhase::AssaultActive);
        assert_eq!(committed.assault_id, Some(assault_id));
        assert_eq!(committed.assault_spawn_generation, generation);
        assert!(committed.assault_recovery_required);
        assert!(committed.assault_defeated_unit_ids.is_empty());
        assert!(!committed.resolution_recorded);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert_eq!(
            game.crisis_assault_units()
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<HashSet<_>>(),
            surviving_ids
        );
    }

    #[test]
    fn checkpoint3_disconnect_continuation_headless() {
        use big_brain::actions::spawn_action;
        use big_brain::prelude::{ActionState, Actor};

        use crate::common::AttackTarget;
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;
        use crate::obj::LastAttacker;
        use crate::villager::FightBack;

        let mut game = HeadlessGame::new(30_000);
        let player_id = game.spawn_hero("Warrior", "AssaultContinuationBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);

        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let assault_started_tick = launched.assault_started_tick;
        let phase_started_tick = launched.phase_started_tick;
        let units = game.crisis_assault_units();
        let unit_ids_before = units.iter().map(|unit| unit.obj_id).collect::<Vec<_>>();
        let sentinel_id = units[0].obj_id;
        let npc_action_id = units[1].obj_id;
        let helper_target_id = units[2].obj_id;
        let helper_player_id = spawn_connected_helper(&mut game, "AssaultContinuationHelper");

        let foreign_structure_health_before = {
            let world = game.app.world_mut();
            let mut query =
                world.query_filtered::<(&Id, &PlayerId, &Stats), With<ClassStructure>>();
            let mut structures = query
                .iter(world)
                .filter(|(_, owner, _)| owner.0 == helper_player_id)
                .map(|(id, _, stats)| (id.0, stats.hp))
                .collect::<Vec<_>>();
            structures.sort_by_key(|(id, _)| *id);
            assert!(!structures.is_empty(), "helper run should own structures");
            structures
        };

        let action_tick = game.game_tick();
        let (
            villager_entity,
            villager_id,
            npc_entity,
            sentinel_damaged_hp,
            villager_hp_before,
            npc_hp_before,
        ) = {
            let npc_pos = units[1].pos;
            let (villager_entity, villager_id) =
                spawn_armed_owner_villager(&mut game, player_id, npc_pos);
            let world = game.app.world_mut();
            let entity_map = world.resource::<EntityObjMap>();
            let sentinel_entity = entity_map.get_entity(sentinel_id).unwrap();
            let npc_entity = entity_map.get_entity(npc_action_id).unwrap();
            let sentinel_base_hp = world.get::<Stats>(sentinel_entity).unwrap().base_hp;
            let sentinel_damaged_hp = sentinel_base_hp - 7;
            world.get_mut::<Stats>(sentinel_entity).unwrap().hp = sentinel_damaged_hp;
            *world.get_mut::<Position>(sentinel_entity).unwrap() = Position { x: 49, y: 49 };
            world
                .get_mut::<VisibleTarget>(sentinel_entity)
                .unwrap()
                .target = crate::constants::NO_TARGET;
            *world.get_mut::<Position>(npc_entity).unwrap() = npc_pos;
            world.get_mut::<Stats>(npc_entity).unwrap().base_damage = Some(30);
            world.get_mut::<VisibleTarget>(npc_entity).unwrap().target = villager_id;
            world.entity_mut(villager_entity).insert(LastAttacker {
                id: npc_action_id,
                tick: action_tick,
            });
            let villager_hp_before = world.get::<Stats>(villager_entity).unwrap().hp;
            let npc_hp_before = world.get::<Stats>(npc_entity).unwrap().hp;
            (
                villager_entity,
                villager_id,
                npc_entity,
                sentinel_damaged_hp,
                villager_hp_before,
                npc_hp_before,
            )
        };

        let (npc_action, villager_action) = {
            let world = game.app.world_mut();
            let npc_action = world
                .spawn((Actor(npc_entity), ActionState::Requested, AttackTarget))
                .id();
            let villager_action = {
                let mut commands = world.commands();
                spawn_action(&FightBack, &mut commands, villager_entity)
            };
            world.flush();
            *world
                .entity_mut(villager_action)
                .get_mut::<ActionState>()
                .unwrap() = ActionState::Requested;
            (npc_action, villager_action)
        };

        let units_before = game.crisis_assault_units();
        let unit_health_before = units_before
            .iter()
            .map(|unit| (unit.obj_id, unit.hp))
            .collect::<Vec<_>>();
        assert_eq!(
            units_before
                .iter()
                .find(|unit| unit.obj_id == sentinel_id)
                .unwrap()
                .hp,
            sentinel_damaged_hp
        );
        let crisis_before_disconnect = game.settlement_crisis().unwrap();
        let pressure = crisis_before_disconnect.pressure;
        let warning_active = crisis_before_disconnect.warning_active;

        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            *game.world().get::<ActionState>(npc_action).unwrap(),
            ActionState::Executing,
            "attributed NPC AI must continue while its owner is offline"
        );
        assert_eq!(
            *game.world().get::<ActionState>(villager_action).unwrap(),
            ActionState::Executing,
            "owner villager AI must continue defending while its owner is offline"
        );
        assert!(
            game.world().get::<Stats>(villager_entity).unwrap().hp < villager_hp_before,
            "the attributed NPC must damage a valid owner villager while offline"
        );
        assert!(
            game.world().get::<Stats>(npc_entity).unwrap().hp < npc_hp_before,
            "the owner villager must damage an attributed attacker while offline"
        );
        assert_eq!(
            game.world()
                .get::<VisibleTarget>(npc_entity)
                .unwrap()
                .target,
            villager_id,
            "a still-valid owner-associated target should remain selected"
        );
        game.tick(19);

        kill_assault_unit_through_normal_combat_as(&mut game, helper_player_id, helper_target_id);
        let offline = game.settlement_crisis().unwrap();
        assert_eq!(offline.phase, CrisisPhase::AssaultActive);
        assert_eq!(offline.assault_id, Some(assault_id));
        assert_eq!(offline.assault_spawn_generation, generation);
        assert_eq!(offline.assault_started_tick, assault_started_tick);
        assert_eq!(offline.phase_started_tick, phase_started_tick);
        assert_eq!(offline.pressure, pressure);
        assert_eq!(offline.warning_active, warning_active);
        assert_eq!(offline.assault_defeated_unit_ids.len(), 1);
        assert!(!offline.assault_recovery_required);
        assert_eq!(game.personal_crises_resolved(), 0);

        let units_offline = game.crisis_assault_units();
        let unit_ids_offline = units_offline
            .iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        let unit_health_offline = units_offline
            .iter()
            .map(|unit| (unit.obj_id, unit.hp))
            .collect::<Vec<_>>();
        assert_eq!(unit_ids_offline, unit_ids_before);
        for unit in units_offline.iter().filter(|unit| !unit.dead) {
            let hp_before = unit_health_before
                .iter()
                .find(|(id, _)| *id == unit.obj_id)
                .unwrap()
                .1;
            assert!(unit.hp <= hp_before, "disconnect must never heal attackers");
        }
        assert_eq!(
            units_offline
                .iter()
                .find(|unit| unit.obj_id == sentinel_id)
                .unwrap()
                .hp,
            sentinel_damaged_hp,
            "the isolated damaged attacker must retain its health"
        );

        let foreign_target_ids = {
            let world = game.app.world_mut();
            let mut query = world.query::<(&Id, &PlayerId)>();
            query
                .iter(world)
                .filter(|(_, owner)| owner.0 == helper_player_id)
                .map(|(id, _)| id.0)
                .collect::<HashSet<_>>()
        };
        assert!(units_offline.iter().all(|unit| unit
            .visible_target
            .map(|target| !foreign_target_ids.contains(&target))
            .unwrap_or(true)));

        let foreign_structure_health_offline = {
            let world = game.app.world_mut();
            let mut query =
                world.query_filtered::<(&Id, &PlayerId, &Stats), With<ClassStructure>>();
            let mut structures = query
                .iter(world)
                .filter(|(_, owner, _)| owner.0 == helper_player_id)
                .map(|(id, _, stats)| (id.0, stats.hp))
                .collect::<Vec<_>>();
            structures.sort_by_key(|(id, _)| *id);
            structures
        };
        assert_eq!(
            foreign_structure_health_offline,
            foreign_structure_health_before
        );

        game.reconnect_player();
        let reconnected = game.settlement_crisis().unwrap();
        let units_reconnected = game.crisis_assault_units();
        let unit_ids_reconnected = units_reconnected
            .iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        let unit_health_reconnected = units_reconnected
            .iter()
            .map(|unit| (unit.obj_id, unit.hp))
            .collect::<Vec<_>>();
        assert_eq!(reconnected.phase, CrisisPhase::AssaultActive);
        assert_eq!(reconnected.assault_id, Some(assault_id));
        assert_eq!(reconnected.assault_spawn_generation, generation);
        assert_eq!(reconnected.assault_started_tick, assault_started_tick);
        assert_eq!(reconnected.phase_started_tick, phase_started_tick);
        assert_eq!(unit_ids_reconnected, unit_ids_offline);
        assert_eq!(unit_health_reconnected, unit_health_offline);

        game.tick(1);
        let after_reconnect_update = game.settlement_crisis().unwrap();
        let units_after_reconnect_update = game.crisis_assault_units();
        assert_eq!(after_reconnect_update.phase, CrisisPhase::AssaultActive);
        assert_eq!(after_reconnect_update.assault_id, Some(assault_id));
        assert_eq!(after_reconnect_update.assault_spawn_generation, generation);
        assert_eq!(
            after_reconnect_update.assault_started_tick,
            assault_started_tick
        );
        assert_eq!(
            after_reconnect_update.phase_started_tick,
            phase_started_tick
        );
        assert_eq!(after_reconnect_update.pressure, pressure);
        assert_eq!(after_reconnect_update.warning_active, warning_active);
        assert_eq!(
            units_after_reconnect_update
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<Vec<_>>(),
            unit_ids_reconnected
        );
        for unit in units_after_reconnect_update
            .iter()
            .filter(|unit| !unit.dead)
        {
            let hp_at_reconnect = unit_health_reconnected
                .iter()
                .find(|(id, _)| *id == unit.obj_id)
                .unwrap()
                .1;
            assert!(
                unit.hp <= hp_at_reconnect,
                "the first reconnected update must not heal a surviving attacker"
            );
        }

        for unit in units_after_reconnect_update
            .iter()
            .filter(|unit| !unit.dead)
        {
            kill_assault_unit_through_normal_combat(&mut game, unit.obj_id);
        }
        game.tick(2);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        game.tick(20);
        assert_eq!(game.personal_crises_resolved(), 1);

        println!(
            "checkpoint3_disconnect_continuation assault_id_before={assault_id} assault_id_while_offline={} assault_id_after_reconnect={} generation_before={generation} generation_while_offline={} generation_after_reconnect={} unit_ids_before={unit_ids_before:?} unit_ids_while_offline={unit_ids_offline:?} unit_ids_after_reconnect={unit_ids_reconnected:?} unit_health_before={unit_health_before:?} unit_health_while_offline={unit_health_offline:?} unit_health_after_reconnect={unit_health_reconnected:?} phase_before={:?} phase_while_offline={:?} phase_after_reconnect={:?} phase_final={:?} resolution_count={} other_player_structures_targeted=false",
            offline.assault_id.unwrap(),
            reconnected.assault_id.unwrap(),
            offline.assault_spawn_generation,
            reconnected.assault_spawn_generation,
            launched.phase,
            offline.phase,
            reconnected.phase,
            game.settlement_crisis().unwrap().phase,
            game.personal_crises_resolved(),
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

        let (other_entity, other_id, other_crisis, unrelated_entity, unrelated_id) = {
            let world = game.app.world_mut();
            let other_id = world.resource_mut::<Ids>().new_obj_id();
            let other_assault_id = own_assault_id + 1;
            let entity = world
                .spawn((
                    Id(other_id),
                    PlayerId(crate::constants::NPC_PLAYER_ID),
                    Position { x: 0, y: 0 },
                    Template("Wolf Rider".to_string()),
                    State::None,
                    Stats {
                        hp: 75,
                        stamina: None,
                        mana: None,
                        base_hp: 75,
                        base_stamina: None,
                        base_mana: None,
                        base_def: 5,
                        damage_range: Some(1),
                        base_damage: Some(6),
                        base_speed: Some(1),
                        base_vision: Some(4),
                    },
                    CrisisAssaultUnit {
                        owner_player_id: player_id + 1,
                        assault_id: other_assault_id,
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
            let other_crisis = SettlementCrisis {
                phase: crate::game::CrisisPhase::AssaultActive,
                pressure: 100,
                warning_active: true,
                assault_id: Some(other_assault_id),
                assault_started_tick: Some(world.resource::<GameTick>().0),
                assault_unit_ids: vec![other_id],
                assault_spawn_generation: 1,
                ..SettlementCrisis::default()
            };
            world
                .resource_mut::<SettlementCrisisState>()
                .insert(player_id + 1, other_crisis.clone());

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
            (
                entity,
                other_id,
                other_crisis,
                unrelated_entity,
                unrelated_id,
            )
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
        let other_after_cleanup = game
            .world()
            .resource::<SettlementCrisisState>()
            .get(&(player_id + 1))
            .cloned()
            .expect("another player's active crisis");
        assert_eq!(other_after_cleanup.phase, other_crisis.phase);
        assert_eq!(other_after_cleanup.pressure, other_crisis.pressure);
        assert_eq!(
            other_after_cleanup.warning_active,
            other_crisis.warning_active
        );
        assert_eq!(other_after_cleanup.assault_id, other_crisis.assault_id);
        assert_eq!(
            other_after_cleanup.assault_spawn_generation,
            other_crisis.assault_spawn_generation
        );
        assert_eq!(
            other_after_cleanup.assault_unit_ids,
            other_crisis.assault_unit_ids
        );
        assert_eq!(
            other_after_cleanup.assault_defeated_unit_ids,
            other_crisis.assault_defeated_unit_ids
        );
        assert_eq!(
            other_after_cleanup.resolution_recorded,
            other_crisis.resolution_recorded
        );
        assert_eq!(
            other_after_cleanup.assault_recovery_required,
            other_crisis.assault_recovery_required
        );
        assert_eq!(game.world().get::<Stats>(other_entity).unwrap().hp, 75);
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
    fn checkpoint3_repeated_disconnect_reconnect_preserves_one_generation() {
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;

        let mut game = HeadlessGame::new(40_000);
        game.spawn_hero("Warrior", "AssaultRepeatBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let phase_started_tick = launched.phase_started_tick;
        let assault_started_tick = launched.assault_started_tick;
        let units = game.crisis_assault_units();
        let unit_ids = units.iter().map(|unit| unit.obj_id).collect::<Vec<_>>();
        let sentinel_id = unit_ids[0];
        {
            let world = game.app.world_mut();
            let sentinel = world
                .resource::<EntityObjMap>()
                .get_entity(sentinel_id)
                .unwrap();
            let base_hp = world.get::<Stats>(sentinel).unwrap().base_hp;
            world.get_mut::<Stats>(sentinel).unwrap().hp = base_hp - 5;
            *world.get_mut::<Position>(sentinel).unwrap() = Position { x: 49, y: 49 };
            world.get_mut::<VisibleTarget>(sentinel).unwrap().target = crate::constants::NO_TARGET;
        }

        for _ in 0..3 {
            game.disconnect_player();
            game.tick(2);
            let offline = game.settlement_crisis().unwrap();
            let units_offline = game.crisis_assault_units();
            assert_eq!(offline.phase, CrisisPhase::AssaultActive);
            assert_eq!(offline.assault_id, Some(assault_id));
            assert_eq!(offline.assault_spawn_generation, generation);
            assert_eq!(offline.phase_started_tick, phase_started_tick);
            assert_eq!(offline.assault_started_tick, assault_started_tick);
            assert_eq!(
                units_offline
                    .iter()
                    .map(|unit| unit.obj_id)
                    .collect::<Vec<_>>(),
                unit_ids
            );
            let health_offline = units_offline
                .iter()
                .map(|unit| (unit.obj_id, unit.hp))
                .collect::<Vec<_>>();

            game.reconnect_player();
            let reconnected = game.settlement_crisis().unwrap();
            let units_reconnected = game.crisis_assault_units();
            assert_eq!(reconnected.phase, CrisisPhase::AssaultActive);
            assert_eq!(reconnected.assault_id, Some(assault_id));
            assert_eq!(reconnected.assault_spawn_generation, generation);
            assert_eq!(
                units_reconnected
                    .iter()
                    .map(|unit| (unit.obj_id, unit.hp))
                    .collect::<Vec<_>>(),
                health_offline
            );
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
            generation
        );

        println!(
            "checkpoint3_repeated_safety assault_id={} disconnects=3 final_generation={} stale_units=0 completion_count={} duplicate_assault=false panic=false",
            assault_id,
            generation,
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
