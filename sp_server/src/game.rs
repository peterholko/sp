use bevy::ecs::query::{QueryData, WorldQuery};
use bevy::ecs::system::SystemParam;

use bevy::reflect::EnumInfo;
use bevy::{
    asset::LoadState,
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy::{scene, state};

use big_brain::thinker::ThinkerBuilder;
use big_brain::{BigBrainPlugin, BigBrainSet};
use rand::distributions::Distribution;
use rand::distributions::WeightedIndex;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing_subscriber::{reload, EnvFilter, Registry};

use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fs::{self, File};
use std::io::Write;
use std::{
    collections::HashSet,
    hash::Hash,
    sync::{Arc, Mutex, OnceLock},
};

use uuid::Uuid;

use crossbeam_channel::{unbounded, Receiver as CBReceiver};
use tokio::sync::mpsc::Sender;

use async_compat::Compat;
use std::env;

use crate::combat::{Combat, CombatSpellQuery};
use crate::common::{
    Dehydrated, Exhausted, Heat, Hunger, Starving, Target, TaskTarget, Thirst, Tired, Transport,
};
use crate::constants::*;
use crate::crisis_balance::{
    crisis_attack_telemetry_observer, crisis_combat_telemetry_observer,
    crisis_engagement_snapshot_system, crisis_true_death_telemetry_system,
    phase_name as balance_phase_name, CrisisBalanceObservation, CrisisBalanceObservationState,
    CrisisBalanceTelemetry, CrisisBalanceTelemetryConfig, CrisisBalanceTelemetryState,
    CrisisPreparationSnapshot, CrisisPressureBreakdown, CrisisPressureSnapshot,
    GoblinCrisisBalanceConfigSnapshot,
};
use crate::database::DatabaseEvent;
use crate::effect::{self, Effect, Effects};
use crate::encounter::{Encounter, EncounterMapObj, EncounterProbability};
use crate::event::{self, EventExecutingState};
use crate::event::{
    EatEventCompleted, EventCompleted, EventExecuting, FindEventCompleted, GameEvent,
    GameEventType, GameEvents, MapEvent, MapEvents, MoveEvent, MoveEventCompleted,
    MoveEventPrecheck, MoveEventUpdate, SleepEventCompleted, Spell, VisibleEvent, VisibleEvents,
};
use crate::experiment::{Experiment, ExperimentPlugin, ExperimentState, Experiments};
use crate::farm::{Crop, CropStages, Crops, FarmPlugin};
use crate::ids::{EntityObjMap, Ids};
use crate::item::{self, AttrKey, Inventory, Item, ItemAction, ItemPlugin, GOLD, SOULSHARD};
use crate::map::{Map, MapPlugin, Season, TileType};
use crate::network::{
    self, send_to_client, send_to_database, BroadcastEvents, CrisisPreparationOption,
    CrisisStatusSnapshot, ObjAttr, RefiningItem,
};
use crate::network::{ResponsePacket, StatsData};
use crate::npc::{NPCPlugin, VisibleTarget};
use crate::obj::{
    is_combat_locked, is_peaceful_interruptible_state, ActiveShelter, ActiveTask, AddLightEffect,
    Assignment, Assignments, BaseAttrs, BuildProgressUpdate, BuildUpgradeState, Campfire,
    CancelEvents, Class, ClassStructure, EndRepeatAction, FoodPoisoningEffect, HeroClass, Id,
    LastAttacker, LastCombatTick, LastDamageTick, Misc, Name, NewObj, Obj, ObjStatQuery, Order,
    PlayerId, Position, RemoveLightEffect, RemoveObj, RemoveWorker, SelectedUpgrade, Shelter,
    Sheltered, StartBuild, StartUpgrade, StartWork, State, StateAboard, StateBuilding, StateChange,
    StateDead, StateUpgrading, Stats, Storage, Subclass, SubclassHero, SubclassNPC,
    SubclassVillager, Template, TemplateChange, TransferAllResources, TrueDeath, UpdateObj,
    Viewshed, Watchtower, WorkEntry, WorkQueue, WorkStatus, WorkType,
};
use crate::player::{self, ActiveInfoType, ActiveInfos, PlayerEvent, PlayerEvents, PlayerPlugin};
use crate::player_setup::{AssignedStartLocations, RunSpawnedObjs, StartLocations};
use crate::recipe::{RecipePlugin, Recipes};
use crate::resource::{Resource, ResourceGatherError, ResourcePlugin, Resources};
use crate::safe_logout::{
    entity_belongs_to_protected_run, is_owner_offline_protected, is_player_offline_protected,
    mark_player_login_sync_complete, object_belongs_to_protected_run, protected_player_for_object,
    remove_player_presence_for_run_cleanup, PlayerWorldPresenceState, SafeLogoutPlugin,
    SafeLogoutTelemetryState,
};
use crate::skill::{SkillData, SkillPlugin, Skills, CARPENTRY, CONSTRUCTION, MASONRY};
use crate::skill_defs::Skill;
use crate::structure::{Plans, Structure, StructurePlugin};
use crate::tax_collector::{TaxCollector, TaxCollectorPlugin};
use crate::templates::{self, ObjTemplate, ResTemplates, Templates, TemplatesPlugin};
use crate::terrain_feature::{TerrainFeature, TerrainFeaturePlugin, TerrainFeatures};
use crate::trade::{Prices, TradePorts, WantedItem};
use crate::villager::{Morale, VillagerPlugin};
use crate::world::{Weather, WeatherAreas, WorldPlugin};
use crate::{villager_util, AppState};

#[derive(Resource, Deref, DerefMut, Clone, Debug, Default)]
pub struct Clients(Arc<Mutex<HashMap<Uuid, Client>>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentConnectionSendError {
    NotCurrent,
    Full,
    Closed,
    RegistryUnavailable,
}

impl Clients {
    fn client_is_active(client_id: &Uuid, client: &Client, player_id: i32) -> bool {
        *client_id == client.id && client.player_id == player_id && !client.sender.is_closed()
    }

    /// Returns whether at least one active network client belongs to `player_id`.
    /// Hero entities persist across disconnects, so the client registry is the
    /// authoritative source of online presence for personal-crisis timing.
    pub fn is_player_online(&self, player_id: i32) -> bool {
        match self.0.lock() {
            Ok(clients) => clients
                .iter()
                .any(|(client_id, client)| Self::client_is_active(client_id, client, player_id)),
            Err(_) => false,
        }
    }

    /// Snapshot the active connection identities for one player. Production
    /// creates a fresh UUID for every socket, so retaining one of these IDs
    /// proves that at least one request-time connection remained uninterrupted.
    pub fn active_connection_ids(&self, player_id: i32) -> Vec<Uuid> {
        match self.0.lock() {
            Ok(clients) => {
                let mut ids = clients
                    .iter()
                    .filter_map(|(client_id, client)| {
                        Self::client_is_active(client_id, client, player_id).then_some(*client_id)
                    })
                    .collect::<Vec<_>>();
                ids.sort_unstable();
                ids
            }
            Err(_) => Vec::new(),
        }
    }

    /// Return true only while at least one connection captured at request time
    /// is still the player's active connection. A replacement connection does
    /// not erase an intervening ordinary disconnect.
    pub fn has_active_connection_from(&self, player_id: i32, request_ids: &[Uuid]) -> bool {
        if request_ids.is_empty() {
            return false;
        }
        match self.0.lock() {
            Ok(clients) => clients.iter().any(|(client_id, client)| {
                request_ids.contains(client_id)
                    && Self::client_is_active(client_id, client, player_id)
            }),
            Err(_) => false,
        }
    }

    /// Atomically makes `client` the sole authoritative connection for its
    /// player. Returning the displaced connection ids lets the network layer
    /// close their streams without granting them any further command authority.
    pub fn activate(&self, client: Client) -> Vec<Uuid> {
        let Ok(mut clients) = self.0.lock() else {
            return Vec::new();
        };
        let mut displaced = clients
            .iter()
            .filter_map(|(client_id, current)| {
                (current.player_id == client.player_id && *client_id != client.id)
                    .then_some(*client_id)
            })
            .collect::<Vec<_>>();
        displaced.sort_unstable();
        for client_id in &displaced {
            clients.remove(client_id);
        }
        clients.insert(client.id, client);
        displaced
    }

    /// Exact authority check used at network ingress and delayed-login
    /// boundaries. Player-level online presence is intentionally insufficient.
    pub fn is_current_connection(&self, player_id: i32, connection_id: Uuid) -> bool {
        match self.0.lock() {
            Ok(clients) => clients
                .get(&connection_id)
                .map(|client| {
                    Self::client_is_active(&connection_id, client, player_id)
                        && !clients.iter().any(|(other_id, other)| {
                            *other_id != connection_id
                                && Self::client_is_active(other_id, other, player_id)
                        })
                })
                .unwrap_or(false),
            Err(_) => false,
        }
    }

    /// The sole current connection for `player_id`, if the registry is in its
    /// authoritative single-session shape.
    pub fn current_connection_id(&self, player_id: i32) -> Option<Uuid> {
        let clients = self.0.lock().ok()?;
        let mut active = clients.iter().filter_map(|(client_id, client)| {
            Self::client_is_active(client_id, client, player_id).then_some(*client_id)
        });
        let connection_id = active.next()?;
        active.next().is_none().then_some(connection_id)
    }

    /// Removes only the exact current connection. An obsolete socket cleanup
    /// cannot erase a replacement connection for the same player.
    pub fn remove_if_current(&self, connection_id: Uuid) -> Option<Client> {
        let mut clients = self.0.lock().ok()?;
        let is_exact = clients
            .get(&connection_id)
            .map(|client| client.id == connection_id)
            .unwrap_or(false);
        is_exact.then(|| clients.remove(&connection_id)).flatten()
    }

    /// Atomically validates the sole current connection, reserves capacity for
    /// the complete ordered bundle, and queues every serialized packet while
    /// replacement activation is excluded by the same registry mutex.
    pub fn try_send_current_bundle(
        &self,
        player_id: i32,
        connection_id: Uuid,
        packets: Vec<String>,
    ) -> Result<(), CurrentConnectionSendError> {
        if packets.is_empty() {
            return Ok(());
        }

        let clients = self
            .0
            .lock()
            .map_err(|_| CurrentConnectionSendError::RegistryUnavailable)?;
        let Some(client) = clients.get(&connection_id) else {
            return Err(CurrentConnectionSendError::NotCurrent);
        };
        if !Self::client_is_active(&connection_id, client, player_id)
            || clients.iter().any(|(other_id, other)| {
                *other_id != connection_id && Self::client_is_active(other_id, other, player_id)
            })
        {
            return Err(CurrentConnectionSendError::NotCurrent);
        }

        let permits =
            client
                .sender
                .try_reserve_many(packets.len())
                .map_err(|error| match error {
                    tokio::sync::mpsc::error::TrySendError::Full(()) => {
                        CurrentConnectionSendError::Full
                    }
                    tokio::sync::mpsc::error::TrySendError::Closed(()) => {
                        CurrentConnectionSendError::Closed
                    }
                })?;
        for (permit, packet) in permits.zip(packets) {
            permit.send(packet);
        }
        Ok(())
    }
}

#[derive(Resource, Deref, DerefMut, Clone, Debug, Default)]
pub struct DatabaseManagers(Arc<Mutex<HashMap<i32, DatabaseClient>>>);

#[derive(Resource, Deref, DerefMut)]
pub struct NetworkReceiver(CBReceiver<PlayerEvent>);

impl NetworkReceiver {
    // Constructor so the headless harness (a sibling module) can wrap a crossbeam
    // receiver it owns the sending half of. The production path builds this inline
    // in `Game::network_init`.
    pub fn new(receiver: CBReceiver<PlayerEvent>) -> Self {
        Self(receiver)
    }
}

#[derive(Resource, Deref, DerefMut, Reflect, Debug)]
#[reflect(Resource)]
pub struct GameTick(pub i32);

/// Test-friendly view of the safe-logout resource. Production always installs
/// `PlayerWorldPresenceState` through `SafeLogoutPlugin`; isolated legacy unit
/// tests that register one gameplay system directly should retain their old
/// unprotected behavior rather than failing system-parameter validation.
#[derive(SystemParam)]
pub struct OptionalPlayerWorldPresence<'w> {
    value: Option<Res<'w, PlayerWorldPresenceState>>,
}

static EMPTY_PLAYER_WORLD_PRESENCE: OnceLock<PlayerWorldPresenceState> = OnceLock::new();

impl std::ops::Deref for OptionalPlayerWorldPresence<'_> {
    type Target = PlayerWorldPresenceState;

    fn deref(&self) -> &Self::Target {
        self.value
            .as_deref()
            .unwrap_or_else(|| EMPTY_PLAYER_WORLD_PRESENCE.get_or_init(Default::default))
    }
}

// custom implementation for unusual values
impl Default for GameTick {
    fn default() -> Self {
        GameTick(DAWN)
    }
}

impl GameTick {
    pub fn to_hour(&self) -> i32 {
        let ticks_in_day = self.0 % GAME_TICKS_PER_DAY;
        let hour = (ticks_in_day / 100) + 1;

        return hour;
    }

    pub fn time_of_day(&self) -> String {
        let ticks_in_day = self.0.rem_euclid(GAME_TICKS_PER_DAY);

        if ticks_in_day < FIRST_LIGHT {
            "Night"
        } else if ticks_in_day < DAWN {
            "First Light"
        } else if ticks_in_day < MORNING {
            "Dawn"
        } else if ticks_in_day < AFTERNOON {
            "Morning"
        } else if ticks_in_day < EVENING {
            "Afternoon"
        } else if ticks_in_day < DUSK {
            "Evening"
        } else if ticks_in_day < NIGHT {
            "Dusk"
        } else {
            "Night"
        }
        .to_string()
    }

    pub fn day(&self) -> i32 {
        let day = (self.0 / GAME_TICKS_PER_DAY) + 1;

        return day;
    }
}

#[derive(Resource, Deref, DerefMut, Reflect, Debug, Default)]
#[reflect(Resource)]
pub struct ExploredMap(pub HashMap<i32, Vec<(i32, i32)>>);

#[derive(Resource, Deref, DerefMut, Reflect, Debug, Default)]
#[reflect(Resource)]
pub struct SurveyHistory(pub HashMap<i32, HashSet<Position>>);

#[derive(Resource, Deref, DerefMut, Reflect, Debug, Default)]
#[reflect(Resource)]
pub struct InvestigatedPOIs(pub HashMap<i32, HashSet<i32>>);

pub const SURVEY_STATUS_UNSURVEYED: &str = "Unsurveyed";
pub const SURVEY_STATUS_SURVEYED: &str = "Surveyed";

pub fn survey_status_for_tile(
    player_id: i32,
    pos: Position,
    survey_history: &SurveyHistory,
) -> String {
    if survey_history
        .get(&player_id)
        .map(|tiles| tiles.contains(&pos))
        .unwrap_or(false)
    {
        SURVEY_STATUS_SURVEYED.to_string()
    } else {
        SURVEY_STATUS_UNSURVEYED.to_string()
    }
}

pub fn record_tile_survey(
    player_id: i32,
    pos: Position,
    survey_history: &mut SurveyHistory,
) -> bool {
    survey_history
        .entry(player_id)
        .or_insert_with(HashSet::new)
        .insert(pos)
}

pub fn record_poi_investigation(
    player_id: i32,
    target_id: i32,
    investigated_pois: &mut InvestigatedPOIs,
) -> bool {
    investigated_pois
        .entry(player_id)
        .or_insert_with(HashSet::new)
        .insert(target_id)
}

pub fn explore_cure_for_item(
    item_name: &str,
    item_class: &str,
    item_subclass: &str,
) -> Option<Effect> {
    match (item_name, item_class, item_subclass) {
        ("Crude Bandage", item::MEDICAL, "Bandage") => Some(Effect::Bleed),
        ("Herbal Poultice", item::POTION, item::HEALTH) => Some(Effect::Sickness),
        ("Health Potion", item::POTION, item::HEALTH) => Some(Effect::Sickness),
        (CRUDE_TORCH, item::TORCH, _) | (RESIN_TORCH, item::TORCH, _) => Some(Effect::Cursed),
        _ => None,
    }
}

pub fn remove_explore_negative_effect(effects: &mut Effects, effect: Effect) -> bool {
    effects.0.remove(&effect).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreOutcomeKind {
    ResourceGlimpse,
    MinorSalvage,
    SupplyCache,
    WashedAshoreMaterials,
    PoiClue,
    EarlyMerchantSignal,
    StirredEnemy,
    BrambleWound,
    FoulSpores,
    DarkOmen,
}

pub fn explore_outcome_from_slot(slot: usize) -> ExploreOutcomeKind {
    match slot % 12 {
        0 | 1 => ExploreOutcomeKind::ResourceGlimpse,
        2 | 3 => ExploreOutcomeKind::MinorSalvage,
        4 => ExploreOutcomeKind::SupplyCache,
        5 => ExploreOutcomeKind::WashedAshoreMaterials,
        6 => ExploreOutcomeKind::PoiClue,
        7 | 8 => ExploreOutcomeKind::EarlyMerchantSignal,
        9 => ExploreOutcomeKind::StirredEnemy,
        10 => ExploreOutcomeKind::BrambleWound,
        _ => ExploreOutcomeKind::FoulSpores,
    }
}

pub fn explore_outcome_is_positive(outcome: ExploreOutcomeKind) -> bool {
    matches!(
        outcome,
        ExploreOutcomeKind::ResourceGlimpse
            | ExploreOutcomeKind::MinorSalvage
            | ExploreOutcomeKind::SupplyCache
            | ExploreOutcomeKind::WashedAshoreMaterials
            | ExploreOutcomeKind::PoiClue
            | ExploreOutcomeKind::EarlyMerchantSignal
    )
}

fn roll_explore_outcome() -> ExploreOutcomeKind {
    let slot = rand::thread_rng().gen_range(0..12);
    if slot == 11 && rand::thread_rng().gen_range(0..2) == 1 {
        ExploreOutcomeKind::DarkOmen
    } else {
        explore_outcome_from_slot(slot)
    }
}

#[derive(Resource, Deref, DerefMut, Reflect, Debug, Default)]
#[reflect(Resource)]
pub struct DebugObjs(pub HashSet<i32>);

#[derive(Resource, Debug)]
pub struct LogLevelOverrides {
    pub overrides: HashMap<String, String>,
    #[allow(clippy::type_complexity)]
    pub reload_handle: Option<Arc<Mutex<reload::Handle<EnvFilter, Registry>>>>,
}

impl Default for LogLevelOverrides {
    fn default() -> Self {
        Self {
            overrides: HashMap::from([("big_brain".to_string(), "DEBUG".to_string())]),
            reload_handle: None,
        }
    }
}

// Enum for the different two type types of perception updates (init and update)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PerceptionUpdateType {
    InitPerception,
    ResumeInitPerception(Uuid),
    UpdatePerception,
}

#[derive(Resource, Deref, DerefMut, Debug)]
struct PerceptionUpdates(HashSet<(i32, PerceptionUpdateType)>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResumeLoginSyncProgress {
    connection_id: Uuid,
    crisis_status_queued: bool,
    perception_queued: bool,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
struct ResumeLoginSyncState(HashMap<i32, ResumeLoginSyncProgress>);

#[derive(Debug, Clone)]
pub struct Client {
    pub id: Uuid,
    pub player_id: i32,
    pub sender: Sender<String>,
}

#[derive(Debug, Clone)]
pub struct DatabaseClient {
    pub sender: Sender<DatabaseEvent>,
}

#[derive(Resource, Debug, Reflect, Default)]
#[reflect(Resource)]
pub struct PlayerStat {
    pub player_id: i32,
    pub num_deaths: u32,
    pub damage_records: VecDeque<DamageRecord>,
}

#[derive(Resource, Deref, DerefMut, Debug, Reflect, Default)]
#[reflect(Resource)]
pub struct PlayerStats(HashMap<i32, PlayerStat>);

// Tracks which crisis tiers have been triggered per player
#[derive(Debug, Default, Clone)]
pub struct PlayerCrisis {
    pub rat_spoilage: bool,
    pub wolf_pack: bool,
    pub goblin_raid: bool,
    pub undead_incursion: bool,
    pub goblin_pillager: bool,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct CrisisState(pub HashMap<i32, PlayerCrisis>);

/// Selects which server-authoritative settlement danger model is active.
/// Environmental time, weather, visibility, and introductory encounters are
/// intentionally independent of this configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurvivalDirectorMode {
    Legacy,
    PersonalCrisis,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurvivalDirectorConfig {
    pub mode: SurvivalDirectorMode,
}

impl Default for SurvivalDirectorConfig {
    fn default() -> Self {
        Self {
            mode: SurvivalDirectorMode::PersonalCrisis,
        }
    }
}

impl SurvivalDirectorConfig {
    pub const fn new(mode: SurvivalDirectorMode) -> Self {
        Self { mode }
    }
}

fn legacy_survival_director(config: Res<SurvivalDirectorConfig>) -> bool {
    config.mode == SurvivalDirectorMode::Legacy
}

fn personal_survival_director(config: Res<SurvivalDirectorConfig>) -> bool {
    config.mode == SurvivalDirectorMode::PersonalCrisis
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrisisKind {
    Goblin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CrisisPhase {
    Dormant,
    Signs,
    Pressure,
    Preparing,
    AssaultReady,
    AssaultActive,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementCrisis {
    pub kind: CrisisKind,
    pub phase: CrisisPhase,
    pub pressure: i32,
    pub phase_started_tick: i32,
    pub online_active_ticks: i32,
    pub phase_online_ticks: i32,
    pub warning_active: bool,
    pub last_evaluated_tick: i32,
    pub assault_id: Option<u64>,
    pub assault_started_tick: Option<i32>,
    pub assault_online_ticks: i32,
    pub assault_unit_ids: Vec<i32>,
    pub assault_defeated_unit_ids: Vec<i32>,
    pub assault_spawn_generation: u32,
    pub resolution_recorded: bool,
    pub resolved_at_tick: Option<i32>,
    pub assault_recovery_required: bool,
    pub assault_grace_logged: bool,
    pub assault_anchor_warning_logged: bool,
    pub assault_spawn_warning_logged: bool,
}

impl SettlementCrisis {
    fn new(game_tick: i32) -> Self {
        Self {
            kind: CrisisKind::Goblin,
            phase: CrisisPhase::Dormant,
            pressure: 0,
            phase_started_tick: game_tick,
            online_active_ticks: 0,
            phase_online_ticks: 0,
            warning_active: false,
            last_evaluated_tick: game_tick,
            assault_id: None,
            assault_started_tick: None,
            assault_online_ticks: 0,
            assault_unit_ids: Vec::new(),
            assault_defeated_unit_ids: Vec::new(),
            assault_spawn_generation: 0,
            resolution_recorded: false,
            resolved_at_tick: None,
            assault_recovery_required: false,
            assault_grace_logged: false,
            assault_anchor_warning_logged: false,
            assault_spawn_warning_logged: false,
        }
    }
}

impl Default for SettlementCrisis {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct SettlementCrisisState(pub HashMap<i32, SettlementCrisis>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrisisTelemetry {
    pub highest_phase: CrisisPhase,
    pub dormant_tick: Option<i32>,
    pub signs_tick: Option<i32>,
    pub pressure_tick: Option<i32>,
    pub preparing_tick: Option<i32>,
    pub assault_ready_tick: Option<i32>,
    pub assault_active_tick: Option<i32>,
    pub resolved_tick: Option<i32>,
    pub assaults_launched: i32,
    pub assaults_resolved: i32,
    pub duplicate_assaults: i32,
    pub status_packets_sent: i32,
    pub login_snapshots_sent: i32,
}

impl Default for CrisisTelemetry {
    fn default() -> Self {
        Self {
            highest_phase: CrisisPhase::Dormant,
            dormant_tick: None,
            signs_tick: None,
            pressure_tick: None,
            preparing_tick: None,
            assault_ready_tick: None,
            assault_active_tick: None,
            resolved_tick: None,
            assaults_launched: 0,
            assaults_resolved: 0,
            duplicate_assaults: 0,
            status_packets_sent: 0,
            login_snapshots_sent: 0,
        }
    }
}

impl CrisisTelemetry {
    fn new(created_tick: i32) -> Self {
        Self {
            dormant_tick: Some(created_tick),
            ..Self::default()
        }
    }

    fn observe_phase(&mut self, phase: CrisisPhase, tick: i32) {
        self.highest_phase = self.highest_phase.max(phase);
        let phase_tick = match phase {
            CrisisPhase::Dormant => &mut self.dormant_tick,
            CrisisPhase::Signs => &mut self.signs_tick,
            CrisisPhase::Pressure => &mut self.pressure_tick,
            CrisisPhase::Preparing => &mut self.preparing_tick,
            CrisisPhase::AssaultReady => &mut self.assault_ready_tick,
            CrisisPhase::AssaultActive => &mut self.assault_active_tick,
            CrisisPhase::Resolved => &mut self.resolved_tick,
        };
        if phase_tick.is_none() {
            *phase_tick = Some(tick);
        }
    }

    fn record_launch(&mut self, tick: i32) {
        if self.assaults_launched > 0 {
            self.duplicate_assaults = self.duplicate_assaults.saturating_add(1);
        }
        self.assaults_launched = self.assaults_launched.saturating_add(1);
        self.observe_phase(CrisisPhase::AssaultActive, tick);
    }

    fn record_resolution(&mut self, tick: i32) {
        self.assaults_resolved = self.assaults_resolved.saturating_add(1);
        self.observe_phase(CrisisPhase::Resolved, tick);
    }
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct CrisisTelemetryState(pub HashMap<i32, CrisisTelemetry>);

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct CrisisStatusLoginSync(pub HashSet<i32>);

#[derive(Debug, Clone)]
struct SentCrisisStatus {
    player_id: i32,
    status: CrisisStatusSnapshot,
}

#[derive(Resource, Debug, Default)]
struct CrisisStatusDeliveryState {
    sent: HashMap<Uuid, SentCrisisStatus>,
    observed_phases: HashMap<i32, CrisisPhase>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CrisisPreparationFacts {
    completed_walls: usize,
    damaged_walls: usize,
    current_tile_wall_present: bool,
    stockade_plan_available: bool,
    stockade_log_units_carried: usize,
    can_start_stockade: bool,
    living_villagers: usize,
    combat_capable_villagers: usize,
    villager_held_spare_weapons: usize,
    actionable_villager_weapons: usize,
    hero_spare_weapons: usize,
    live_hero: bool,
    hero_idle: bool,
    hero_equipped_weapon: Option<String>,
    hero_equipped_armor: usize,
    hero_carried_weapons: usize,
    hero_carried_armor: usize,
    hero_carried_healing: usize,
    stored_weapons: usize,
    stored_armor: usize,
    stored_healing: usize,
    transferable_stored_weapons: usize,
    transferable_stored_armor: usize,
    transferable_stored_healing: usize,
}

#[derive(SystemParam)]
struct CrisisPreparationCollector<'w, 's> {
    ids: Option<Res<'w, Ids>>,
    templates: Option<Res<'w, Templates>>,
    plans: Option<Res<'w, Plans>>,
    hero_query: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static Id,
            &'static Position,
            &'static Template,
            &'static State,
            &'static Stats,
            &'static Inventory,
            Option<&'static StateDead>,
            Option<&'static TrueDeath>,
        ),
        With<SubclassHero>,
    >,
    villager_query: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static State,
            &'static Stats,
            &'static Inventory,
            Option<&'static StateDead>,
        ),
        With<SubclassVillager>,
    >,
    structure_query: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static Position,
            &'static Subclass,
            &'static State,
            &'static Stats,
            &'static Inventory,
            Option<&'static StateDead>,
        ),
        With<ClassStructure>,
    >,
}

pub(crate) fn is_usable_crisis_healing_item(item: &Item) -> bool {
    if item.quantity <= 0 {
        return false;
    }

    match (item.class.as_str(), item.subclass.as_str()) {
        // Bandages use a fixed server-side heal and bleed cure rather than a
        // Healing attribute, so attribute-only detection would omit them.
        (item::MEDICAL, "Bandage") => true,
        (item::POTION, item::HEALTH) => matches!(
            item.attrs.get(&AttrKey::Healing),
            Some(item::AttrVal::Num(value)) if *value > 0.0
        ),
        // Food follows the Eat path. A Healing attribute on food is not a
        // currently usable crisis heal and must not create a false-ready row.
        _ => false,
    }
}

fn positive_item_units(item: &Item) -> usize {
    item.quantity.max(0) as usize
}

fn item_fits_normal_transfer(item: &Item, target_weight: i32, target_capacity: i32) -> bool {
    let transfer_weight = (item.quantity.max(0) as f32 * item.weight) as i32;
    item.quantity > 0
        && target_capacity >= 0
        && target_weight.saturating_add(transfer_weight) <= target_capacity
}

impl CrisisPreparationCollector<'_, '_> {
    fn collect(&self, player_id: i32) -> CrisisPreparationFacts {
        let mut facts = CrisisPreparationFacts::default();
        let mapped_hero_id = self.ids.as_ref().and_then(|ids| ids.get_hero(player_id));
        let hero = self
            .hero_query
            .iter()
            .filter(|(owner, id, _, _, state, stats, _, dead, true_death)| {
                owner.0 == player_id
                    && mapped_hero_id.map(|mapped| mapped == id.0).unwrap_or(true)
                    && state.is_alive()
                    && stats.hp > 0
                    && dead.is_none()
                    && true_death.is_none()
            })
            .min_by_key(|(_, id, _, _, _, _, _, _, _)| id.0);

        let mut hero_position = None;
        let mut hero_inventory_weight = 0;
        let mut hero_capacity = None;
        if let Some((_, _, position, template, state, _, inventory, _, _)) = hero {
            facts.live_hero = true;
            facts.hero_idle = *state == State::None;
            hero_position = Some(*position);
            hero_inventory_weight = inventory.get_total_weight();
            hero_capacity = self
                .templates
                .as_ref()
                .map(|templates| Obj::get_capacity(&template.0, &templates.obj_templates));
            facts.hero_equipped_weapon = inventory
                .items
                .iter()
                .filter(|item| item.quantity > 0 && item.equipped && item.class == WEAPON)
                .map(|item| item.name.clone())
                .min();
            facts.hero_equipped_armor = inventory
                .items
                .iter()
                .filter(|item| item.quantity > 0 && item.equipped && item.class == ARMOR)
                .count();
            facts.hero_carried_weapons = inventory
                .items
                .iter()
                .filter(|item| item.quantity > 0 && !item.equipped && item.class == WEAPON)
                .map(positive_item_units)
                .sum();
            facts.hero_carried_armor = inventory
                .items
                .iter()
                .filter(|item| item.quantity > 0 && !item.equipped && item.class == ARMOR)
                .map(positive_item_units)
                .sum();
            facts.hero_carried_healing = inventory
                .items
                .iter()
                .filter(|item| is_usable_crisis_healing_item(item))
                .map(positive_item_units)
                .sum();
            facts.stockade_log_units_carried = inventory.count_for_build_req(LOG).max(0) as usize;
        }

        facts.stockade_plan_available = self
            .plans
            .as_ref()
            .zip(self.templates.as_ref())
            .map(|(plans, templates)| {
                Structure::available_to_build(player_id, plans.to_vec(), &templates.obj_templates)
                    .iter()
                    .any(|structure| structure.template == "Stockade")
            })
            .unwrap_or(false);
        for (owner, state, stats, inventory, dead) in self.villager_query.iter() {
            if owner.0 != player_id || !state.is_alive() || stats.hp <= 0 || dead.is_some() {
                continue;
            }
            facts.living_villagers = facts.living_villagers.saturating_add(1);
            let combat_capable = stats.base_damage.unwrap_or(0) > 0
                || inventory
                    .items
                    .iter()
                    .any(|item| item.quantity > 0 && item.equipped && item.class == WEAPON);
            if combat_capable {
                facts.combat_capable_villagers = facts.combat_capable_villagers.saturating_add(1);
            }
            let held_spare_weapons = inventory
                .items
                .iter()
                .filter(|item| item.quantity > 0 && !item.equipped && item.class == WEAPON)
                .map(positive_item_units)
                .sum::<usize>();
            facts.villager_held_spare_weapons = facts
                .villager_held_spare_weapons
                .saturating_add(held_spare_weapons);
            if !combat_capable && *state == State::None {
                facts.actionable_villager_weapons = facts
                    .actionable_villager_weapons
                    .saturating_add(held_spare_weapons);
            }
        }

        for (owner, position, subclass, state, stats, inventory, dead) in
            self.structure_query.iter()
        {
            // Foundation placement rejects any wall already on the hero's
            // current tile, including an unfinished or other-player wall. Keep
            // this owner-agnostic occupancy fact separate from the owner-only
            // preparation details below so guidance never promises a command
            // the authoritative placement system will reject.
            if hero_position.is_some_and(|hero_position| {
                hero_position == *position && *subclass == Subclass::Wall
            }) {
                facts.current_tile_wall_present = true;
            }

            if owner.0 != player_id || dead.is_some() || !Structure::is_built(*state) {
                continue;
            }

            if *subclass == Subclass::Wall {
                facts.completed_walls = facts.completed_walls.saturating_add(1);
                if stats.hp < stats.base_hp {
                    facts.damaged_walls = facts.damaged_walls.saturating_add(1);
                }
            }

            if *subclass != Subclass::Storage {
                continue;
            }

            let normal_transfer_available = hero_position
                .map(|hero_position| Map::is_adjacent_including_source(hero_position, *position))
                .unwrap_or(false);

            for stored_item in inventory.items.iter().filter(|item| item.quantity > 0) {
                let units = positive_item_units(stored_item);
                let fits = normal_transfer_available
                    && hero_capacity
                        .map(|capacity| {
                            item_fits_normal_transfer(stored_item, hero_inventory_weight, capacity)
                        })
                        .unwrap_or(false);

                if !stored_item.equipped && stored_item.class == WEAPON {
                    facts.stored_weapons = facts.stored_weapons.saturating_add(units);
                    if fits {
                        facts.transferable_stored_weapons =
                            facts.transferable_stored_weapons.saturating_add(units);
                    }
                }
                if !stored_item.equipped && stored_item.class == ARMOR {
                    facts.stored_armor = facts.stored_armor.saturating_add(units);
                    if fits {
                        facts.transferable_stored_armor =
                            facts.transferable_stored_armor.saturating_add(units);
                    }
                }
                if is_usable_crisis_healing_item(stored_item) {
                    facts.stored_healing = facts.stored_healing.saturating_add(units);
                    if fits {
                        facts.transferable_stored_healing =
                            facts.transferable_stored_healing.saturating_add(units);
                    }
                }
            }
        }

        facts.can_start_stockade = facts.live_hero
            && facts.hero_idle
            && !facts.current_tile_wall_present
            && facts.stockade_plan_available
            && facts.stockade_log_units_carried >= 3;

        // A hero without an equipped weapon should retain the first carried
        // weapon as an equipment option. Hero/storage weapons are observed,
        // but never presented as immediately equippable by a villager without
        // the normal adjacent transfer step.
        facts.hero_spare_weapons = facts.hero_carried_weapons;
        if facts.live_hero && facts.hero_equipped_weapon.is_none() {
            facts.hero_spare_weapons = facts.hero_spare_weapons.saturating_sub(1);
        }

        facts
    }
}

/// Explicit ownership for a personal-crisis combatant. NPC faction ownership
/// remains `NPC_PLAYER_ID`; this component is the authoritative link to the
/// settlement owner and logical assault.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrisisAssaultUnit {
    pub owner_player_id: i32,
    pub assault_id: u64,
    pub spawn_generation: u32,
}

/// Monotonic process-local identity source for logical personal assaults.
/// Checkpoint 2 state is runtime-only under the prototype snapshot path, so the
/// matching identity source is intentionally runtime-only as well.
#[derive(Resource, Debug)]
pub struct NextCrisisAssaultId {
    next: u64,
}

impl Default for NextCrisisAssaultId {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl NextCrisisAssaultId {
    fn allocate(&mut self) -> Option<u64> {
        let id = self.next;
        self.next = self.next.checked_add(1)?;
        Some(id)
    }
}

pub const EARLY_GAME_ENEMY_TEMPLATES: [&str; 2] = [
    "Cave Bat",
    "Cave Bat", //"Thorn Beetle",
               //"Ash Viper",
               //"Moss Mite",
               //"Reef Skitter",
];

fn random_early_game_enemy_template() -> &'static str {
    let enemy_index = rand::thread_rng().gen_range(0..EARLY_GAME_ENEMY_TEMPLATES.len());
    EARLY_GAME_ENEMY_TEMPLATES[enemy_index]
}

const SANCTUARY_HUNTER_CAP: usize = 18;
const SANCTUARY_POWER_UNLOCK_SCORE: i32 = 200;

#[derive(Debug, Clone, Default)]
pub struct SanctuaryExcursionEntry {
    pub exposure_moves: i32,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct SanctuaryExcursions(pub HashMap<i32, SanctuaryExcursionEntry>);

// Base Soulshard cost for the first sanctuary upgrade. Cost escalates per level
// (see [`sanctuary_upgrade_cost`]) so maxing the sanctuary is a multi-stage goal
// stretched across the run rather than done in the first few days.
pub const SANCTUARY_UPGRADE_COST: i32 = 3;
// Highest sanctuary level (keeps the suppression radius from swallowing the map).
pub const SANCTUARY_MAX_LEVEL: i32 = 5;

/// Soulshards required to go from `current_level` to the next level. Escalates:
/// 3, 6, 9, 12, 15 (45 total to max), so each tier is a bigger commitment.
pub fn sanctuary_upgrade_cost(current_level: i32) -> i32 {
    SANCTUARY_UPGRADE_COST * (current_level.max(0) + 1)
}
// Extra in-zone defensive multiplier per sanctuary level (applied to the Sanctuary
// effect's amplifier, which combat multiplies the sanctuary defense by).
pub const SANCTUARY_DEFENSE_PER_LEVEL: f32 = 0.25;

/// Effective full-suppression radius for a sanctuary at `level`. Inside this
/// radius random encounters are fully suppressed and the defensive bonus applies.
/// Level 0 = the innate `SANCTUARY_RANGE`; each level adds one tile.
pub fn sanctuary_full_radius(level: i32) -> u32 {
    (SANCTUARY_RANGE as i32 + level.max(0)) as u32
}

/// Effective weak-sanctuary radius (outer ring) for a sanctuary at `level`.
pub fn sanctuary_weak_radius(level: i32) -> u32 {
    (WEAK_SANCTUARY_RANGE as i32 + level.max(0)) as u32
}

/// A single Monolith's protective zone, kept in the [`SanctuaryZones`] resource so
/// every system (encounter suppression, wildness regen, crisis spawning, the
/// defensive bonus) reads one source of truth instead of re-querying Monoliths.
#[derive(Debug, Clone, Copy)]
pub struct SanctuaryZone {
    pub pos: Position,
    pub level: i32,
}

impl SanctuaryZone {
    pub fn full_radius(&self) -> u32 {
        sanctuary_full_radius(self.level)
    }
    pub fn weak_radius(&self) -> u32 {
        sanctuary_weak_radius(self.level)
    }
}

/// monolith obj id -> its current sanctuary zone. Rebuilt each tick by
/// `sanctuary_zones_sync_system` from the live Monolith entities.
#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct SanctuaryZones(pub HashMap<i32, SanctuaryZone>);

impl SanctuaryZones {
    /// True if `pos` is within the full-suppression radius of any sanctuary
    /// (matches the `dist < full_radius` boundary used for encounter suppression).
    pub fn in_full_zone(&self, pos: Position) -> bool {
        self.0
            .values()
            .any(|z| Map::distance((pos.x, pos.y), (z.pos.x, z.pos.y)) < z.full_radius())
    }

    /// The nearest sanctuary zone to `pos`, if any (by centre distance).
    pub fn nearest(&self, pos: Position) -> Option<SanctuaryZone> {
        self.0
            .values()
            .min_by_key(|z| Map::distance((pos.x, pos.y), (z.pos.x, z.pos.y)))
            .copied()
    }
}

// Rebuild the SanctuaryZones lookup from the live Monolith entities each tick.
// Cheap (a handful of Monoliths) and keeps the single source of truth in sync
// with sanctuary-level upgrades without threading the resource through spawn code.
fn sanctuary_zones_sync_system(
    mut zones: ResMut<SanctuaryZones>,
    monolith_query: Query<(&Id, &Position, &Monolith)>,
) {
    zones.0.clear();
    for (id, pos, monolith) in monolith_query.iter() {
        zones.0.insert(
            id.0,
            SanctuaryZone {
                pos: *pos,
                level: monolith.sanctuary_level,
            },
        );
    }
}

// Players that just logged in and need their hero's sanctuary state re-sent.
// The server only emits sanctuary effect packets on movement transitions, so on
// (re)login the client has no idea whether the hero is currently protected.
// Each entry is (player_id, due_tick): the resend is held until due_tick so it is
// sent after the login perception (which is what assigns the hero id client-side),
// keeping the WebSocket delivery order correct.
#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct SanctuaryLoginChecks(pub Vec<(i32, i32)>);

#[derive(Debug, Component, Clone)]
pub struct SanctuaryHunter {
    pub player_id: i32,
}

// Tracks where each player's hero originally spawned
#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct SpawnPositions(pub HashMap<i32, Position>);

#[derive(Debug, Clone)]
pub struct PlayerIntroEntry {
    pub start_tick: i32,
    pub shipwreck_chain_started: bool,
    pub villager_spawned: bool,
    pub danger_unlocked: bool,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct PlayerIntroState(pub HashMap<i32, PlayerIntroEntry>);

/// Completion flags for the scripted shipwreck combat chain. These are kept
/// separate from [`PlayerCrisis`], which now contains only legacy director
/// progression.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlayerIntroEncounters {
    pub initial_encounter: bool,
    pub spider_encounter: bool,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct IntroEncounterState(pub HashMap<i32, PlayerIntroEncounters>);

// Tracks the initial shipwreck encounter chain.
#[derive(Debug, Clone)]
pub struct InitialEncounterEntry {
    pub rat_ids: Vec<i32>, // IDs of the two starting enemies
    pub opening_enemy_templates: Vec<String>,
    pub phase1_spawn: String,       // "Giant Crab" or "Wild Boar"
    pub phase1_npc_id: Option<i32>, // set when phase1 creature spawns
    pub spawn_pos: Position,
    pub villager_spawn_pos: Position,
    pub first_rat_spawn_tick: i32,
    pub second_rat_spawn_tick: i32,
    pub villager_ready_tick: i32,
    pub phase1_unlock_tick: i32,
    pub spider_unlock_tick: i32,
    pub villager_event_scheduled: bool,
    pub merchant_id: i32,
    // Necromancer encounter data. The necromancer and its mausoleum are spawned
    // hidden in player_setup; the reveal/activation NecroEvent is scheduled later,
    // 5 minutes after the villager is rescued (see the SpawnVillager handler).
    pub necromancer_id: i32,
    pub mausoleum_id: i32,
    pub necro_spawn_anchor: Position,
    pub necro_corpse_anchor: Position,
    pub necro_home: Position,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct InitialEncounterState(pub HashMap<i32, InitialEncounterEntry>);

// Tracks objective completion per player
#[derive(Debug, Default, Clone, Serialize)]
pub struct PlayerObjectives {
    pub scavenge_shipwreck: bool,
    pub build_campfire: bool,
    pub win_first_fight: bool,
    pub build_3_structures: bool,
    pub recruit_villager: bool,
    pub explore_poi: bool,
    pub choose_expansion: bool,
    pub survive_5_nights: bool,
    pub find_legendary_hideout: bool,
    pub defeat_ashen_warlord: bool,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct Objectives(pub HashMap<i32, PlayerObjectives>);

#[derive(Debug, Default, Clone)]
pub struct PlayerRunScore {
    pub start_tick: i32,
    pub waves_survived: i32,
    pub enemies_killed: i32,
    pub elites_killed: i32,
    pub captains_killed: i32,
    pub legendary_kills: i32,
    pub hideouts_cleared: i32,
    pub repairs: i32,
    pub highest_pressure_level: i32,
    /// Runtime-only Checkpoint 3 completion record. Tangible crisis rewards and
    /// final reporting packets remain deferred.
    pub personal_crises_resolved: i32,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct RunScoreState(pub HashMap<i32, PlayerRunScore>);

#[derive(Debug, Clone)]
pub struct LegendaryFollowerWave {
    pub ids: Vec<i32>,
    pub defeated: bool,
}

#[derive(Debug, Clone)]
pub struct LegendaryThreat {
    pub name: String,
    pub hideout_pos: Position,
    pub hideout_id: Option<i32>,
    pub boss_id: Option<i32>,
    pub rumor_sent: bool,
    pub active: bool,
    pub defeated: bool,
    pub hideout_revealed: bool,
    pub active_since_tick: Option<i32>,
    pub defeated_at_tick: Option<i32>,
    pub next_follower_tick: i32,
    pub waves_sent: i32,
    pub follower_waves: Vec<LegendaryFollowerWave>,
    pub followers_defeated: i32,
    pub captains_defeated: i32,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct LegendaryThreatState(pub HashMap<i32, LegendaryThreat>);

#[derive(Debug, Component)]
pub struct LegendaryHideout {
    pub player_id: i32,
}

#[derive(Debug, Component)]
pub struct LegendaryBoss {
    pub player_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegendaryFollowerRole {
    Raider,
    Torchbearer,
    Thief,
    Captain,
}

#[derive(Debug, Component)]
pub struct LegendaryFollower {
    pub player_id: i32,
    pub role: LegendaryFollowerRole,
}

fn intro_age(
    game_tick: &GameTick,
    player_id: i32,
    player_intro_state: &PlayerIntroState,
) -> Option<i32> {
    player_intro_state
        .get(&player_id)
        .map(|entry| game_tick.0 - entry.start_tick)
}

fn intro_is_younger_than(
    game_tick: &GameTick,
    player_id: i32,
    player_intro_state: &PlayerIntroState,
    threshold: i32,
) -> bool {
    intro_age(game_tick, player_id, player_intro_state)
        .map(|age| age < threshold)
        .unwrap_or(false)
}

fn player_survival_ticks(
    game_tick: &GameTick,
    player_id: i32,
    player_intro_state: &PlayerIntroState,
) -> i32 {
    intro_age(game_tick, player_id, player_intro_state)
        .unwrap_or_else(|| game_tick.0 - DAWN)
        .max(0)
}

// Provisional personal-goblin-crisis tuning. Pressure is derived from current
// settlement facts, not accumulated per evaluation, so repeated evaluation is
// naturally idempotent and global calendar days are irrelevant.
pub(crate) const GOBLIN_PRESSURE_MAX: i32 = 100;
pub(crate) const GOBLIN_DANGER_UNLOCKED_PRESSURE: i32 = 10;
pub(crate) const GOBLIN_THREE_STRUCTURES_PRESSURE: i32 = 20;
pub(crate) const GOBLIN_VILLAGER_PRESSURE: i32 = 15;
pub(crate) const GOBLIN_EXPLORE_POI_PRESSURE: i32 = 10;
pub(crate) const GOBLIN_CHOOSE_EXPANSION_PRESSURE: i32 = 15;
pub(crate) const GOBLIN_GOLD_TIER_ONE: i32 = 25;
pub(crate) const GOBLIN_GOLD_TIER_TWO: i32 = 50;
pub(crate) const GOBLIN_GOLD_TIER_THREE: i32 = 100;
pub(crate) const GOBLIN_GOLD_PRESSURE_PER_TIER: i32 = 5;
pub(crate) const GOBLIN_SANCTUARY_PRESSURE_PER_LEVEL: i32 = 2;
pub(crate) const GOBLIN_SANCTUARY_PRESSURE_MAX: i32 = 10;
pub(crate) const GOBLIN_ONLINE_PRESSURE_TIER_ONE_TICKS: i32 = 60 * TICKS_PER_SEC;
pub(crate) const GOBLIN_ONLINE_PRESSURE_TIER_TWO_TICKS: i32 = 180 * TICKS_PER_SEC;
pub(crate) const GOBLIN_ONLINE_PRESSURE_TIER_THREE_TICKS: i32 = 360 * TICKS_PER_SEC;
pub(crate) const GOBLIN_ONLINE_PRESSURE_PER_TIER: i32 = 5;

pub(crate) const GOBLIN_SIGNS_PRESSURE: i32 = 20;
pub(crate) const GOBLIN_PRESSURE_PHASE_PRESSURE: i32 = 45;
// Checkpoint 2 keeps the existing contributor model and ordered online-time
// gates, but makes a maintained developed-settlement path reachable. Reaching
// Pressure at 45 can now mature into Preparing; AssaultReady still requires a
// further persistent fact (the lowest observed developed-solo path was 49).
pub(crate) const GOBLIN_PREPARING_PRESSURE: i32 = 45;
pub(crate) const GOBLIN_ASSAULT_READY_PRESSURE: i32 = 49;
pub(crate) const GOBLIN_SIGNS_MIN_ONLINE_TICKS: i32 = 60 * TICKS_PER_SEC;
pub(crate) const GOBLIN_PRESSURE_MIN_ONLINE_TICKS: i32 = 120 * TICKS_PER_SEC;
pub(crate) const GOBLIN_PREPARING_MIN_ONLINE_TICKS: i32 = 180 * TICKS_PER_SEC;

const CRISIS_STATUS_VERSION: u32 = 1;
const CRISIS_STATUS_PRESSURE_DELTA: i32 = 5;
const CRISIS_STATUS_COUNTDOWN_DELTA_SECONDS: i32 = 5;
pub(crate) const ASSAULT_READY_GRACE_TICKS: i32 = 30 * TICKS_PER_SEC;
pub(crate) const ASSAULT_MAX_ONLINE_WAIT_TICKS: i32 = 120 * TICKS_PER_SEC;
const PERSONAL_ASSAULT_VISION: u32 = 14;
const PERSONAL_ASSAULT_FALLBACK_MIN_RADIUS: i32 = 6;
const PERSONAL_ASSAULT_FALLBACK_MAX_RADIUS: i32 = 8;
const PERSONAL_ASSAULT_SANCTUARY_MIN_OFFSET: i32 = 1;
const PERSONAL_ASSAULT_SANCTUARY_MAX_OFFSET: i32 = 3;
const PERSONAL_ASSAULT_NEIGHBOUR_EXCLUSION_DISTANCE: u32 = 3;
const PERSONAL_ASSAULT_SPAWN_CANDIDATE_LIMIT: usize = 96;
pub(crate) const GOBLIN_ASSAULT_COMPOSITION: [&str; 3] =
    ["Wolf Rider", "Wolf Rider", "Goblin Pillager"];

pub fn goblin_crisis_balance_config_snapshot() -> GoblinCrisisBalanceConfigSnapshot {
    GoblinCrisisBalanceConfigSnapshot {
        pressure_max: GOBLIN_PRESSURE_MAX,
        danger_unlocked_pressure: GOBLIN_DANGER_UNLOCKED_PRESSURE,
        three_structures_pressure: GOBLIN_THREE_STRUCTURES_PRESSURE,
        villager_pressure: GOBLIN_VILLAGER_PRESSURE,
        explore_poi_pressure: GOBLIN_EXPLORE_POI_PRESSURE,
        choose_expansion_pressure: GOBLIN_CHOOSE_EXPANSION_PRESSURE,
        gold_tier_thresholds: vec![
            GOBLIN_GOLD_TIER_ONE,
            GOBLIN_GOLD_TIER_TWO,
            GOBLIN_GOLD_TIER_THREE,
        ],
        gold_pressure_per_tier: GOBLIN_GOLD_PRESSURE_PER_TIER,
        sanctuary_pressure_per_level: GOBLIN_SANCTUARY_PRESSURE_PER_LEVEL,
        sanctuary_pressure_max: GOBLIN_SANCTUARY_PRESSURE_MAX,
        online_pressure_tier_ticks: vec![
            GOBLIN_ONLINE_PRESSURE_TIER_ONE_TICKS,
            GOBLIN_ONLINE_PRESSURE_TIER_TWO_TICKS,
            GOBLIN_ONLINE_PRESSURE_TIER_THREE_TICKS,
        ],
        online_pressure_per_tier: GOBLIN_ONLINE_PRESSURE_PER_TIER,
        signs_threshold: GOBLIN_SIGNS_PRESSURE,
        pressure_threshold: GOBLIN_PRESSURE_PHASE_PRESSURE,
        preparing_threshold: GOBLIN_PREPARING_PRESSURE,
        assault_ready_threshold: GOBLIN_ASSAULT_READY_PRESSURE,
        signs_min_online_ticks: GOBLIN_SIGNS_MIN_ONLINE_TICKS,
        pressure_min_online_ticks: GOBLIN_PRESSURE_MIN_ONLINE_TICKS,
        preparing_min_online_ticks: GOBLIN_PREPARING_MIN_ONLINE_TICKS,
        assault_ready_grace_ticks: ASSAULT_READY_GRACE_TICKS,
        assault_max_online_wait_ticks: ASSAULT_MAX_ONLINE_WAIT_TICKS,
        preferred_launch_window: "dusk_or_night".to_string(),
        game_ticks_per_day: GAME_TICKS_PER_DAY,
        preferred_launch_start_tick: DUSK,
        preferred_launch_wrap_end_tick: FIRST_LIGHT,
        assault_composition: GOBLIN_ASSAULT_COMPOSITION
            .iter()
            .map(|template| (*template).to_string())
            .collect(),
        assault_vision: PERSONAL_ASSAULT_VISION,
        fallback_spawn_min_distance: PERSONAL_ASSAULT_FALLBACK_MIN_RADIUS,
        fallback_spawn_max_distance: PERSONAL_ASSAULT_FALLBACK_MAX_RADIUS,
        sanctuary_spawn_min_offset_from_weak_radius: PERSONAL_ASSAULT_SANCTUARY_MIN_OFFSET,
        sanctuary_spawn_max_offset_from_weak_radius: PERSONAL_ASSAULT_SANCTUARY_MAX_OFFSET,
        neighbouring_structure_exclusion_distance: PERSONAL_ASSAULT_NEIGHBOUR_EXCLUSION_DISTANCE,
        spawn_candidate_limit: PERSONAL_ASSAULT_SPAWN_CANDIDATE_LIMIT,
    }
}

fn is_assault_preferred_time(game_tick: i32) -> bool {
    let ticks_in_day = game_tick.rem_euclid(GAME_TICKS_PER_DAY);
    ticks_in_day >= DUSK || ticks_in_day < FIRST_LIGHT
}

fn assault_launch_allowed(online_ready_ticks: i32, game_tick: i32) -> bool {
    online_ready_ticks >= ASSAULT_READY_GRACE_TICKS
        && (is_assault_preferred_time(game_tick)
            || online_ready_ticks >= ASSAULT_MAX_ONLINE_WAIT_TICKS)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct GoblinPressureFacts {
    danger_unlocked: bool,
    completed_structures: usize,
    living_villagers: usize,
    stored_gold: i32,
    sanctuary_level: i32,
    explore_poi: bool,
    choose_expansion: bool,
    online_active_ticks: i32,
}

fn calculate_goblin_pressure_breakdown(facts: &GoblinPressureFacts) -> CrisisPressureBreakdown {
    if !facts.danger_unlocked {
        return CrisisPressureBreakdown::default();
    }

    let mut breakdown = CrisisPressureBreakdown {
        danger_unlocked: GOBLIN_DANGER_UNLOCKED_PRESSURE,
        structures: (facts.completed_structures >= 3)
            .then_some(GOBLIN_THREE_STRUCTURES_PRESSURE)
            .unwrap_or(0),
        villagers: (facts.living_villagers > 0)
            .then_some(GOBLIN_VILLAGER_PRESSURE)
            .unwrap_or(0),
        explore_poi: facts
            .explore_poi
            .then_some(GOBLIN_EXPLORE_POI_PRESSURE)
            .unwrap_or(0),
        choose_expansion: facts
            .choose_expansion
            .then_some(GOBLIN_CHOOSE_EXPANSION_PRESSURE)
            .unwrap_or(0),
        ..CrisisPressureBreakdown::default()
    };

    breakdown.stored_gold = if facts.stored_gold >= GOBLIN_GOLD_TIER_THREE {
        GOBLIN_GOLD_PRESSURE_PER_TIER * 3
    } else if facts.stored_gold >= GOBLIN_GOLD_TIER_TWO {
        GOBLIN_GOLD_PRESSURE_PER_TIER * 2
    } else if facts.stored_gold >= GOBLIN_GOLD_TIER_ONE {
        GOBLIN_GOLD_PRESSURE_PER_TIER
    } else {
        0
    };

    breakdown.sanctuary = (facts.sanctuary_level.max(0) * GOBLIN_SANCTUARY_PRESSURE_PER_LEVEL)
        .min(GOBLIN_SANCTUARY_PRESSURE_MAX);

    breakdown.online_time = if facts.online_active_ticks >= GOBLIN_ONLINE_PRESSURE_TIER_THREE_TICKS
    {
        GOBLIN_ONLINE_PRESSURE_PER_TIER * 3
    } else if facts.online_active_ticks >= GOBLIN_ONLINE_PRESSURE_TIER_TWO_TICKS {
        GOBLIN_ONLINE_PRESSURE_PER_TIER * 2
    } else if facts.online_active_ticks >= GOBLIN_ONLINE_PRESSURE_TIER_ONE_TICKS {
        GOBLIN_ONLINE_PRESSURE_PER_TIER
    } else {
        0
    };

    breakdown.raw_total = breakdown.contributor_sum();
    breakdown.clamped_total = breakdown.raw_total.min(GOBLIN_PRESSURE_MAX);
    breakdown
}

fn calculate_goblin_pressure(facts: &GoblinPressureFacts) -> i32 {
    calculate_goblin_pressure_breakdown(facts).clamped_total
}

fn next_goblin_crisis_phase(crisis: &SettlementCrisis) -> Option<CrisisPhase> {
    match crisis.phase {
        CrisisPhase::Dormant if crisis.pressure >= GOBLIN_SIGNS_PRESSURE => {
            Some(CrisisPhase::Signs)
        }
        CrisisPhase::Signs
            if crisis.pressure >= GOBLIN_PRESSURE_PHASE_PRESSURE
                && crisis.phase_online_ticks >= GOBLIN_SIGNS_MIN_ONLINE_TICKS =>
        {
            Some(CrisisPhase::Pressure)
        }
        CrisisPhase::Pressure
            if crisis.pressure >= GOBLIN_PREPARING_PRESSURE
                && crisis.phase_online_ticks >= GOBLIN_PRESSURE_MIN_ONLINE_TICKS =>
        {
            Some(CrisisPhase::Preparing)
        }
        CrisisPhase::Preparing
            if crisis.pressure >= GOBLIN_ASSAULT_READY_PRESSURE
                && crisis.phase_online_ticks >= GOBLIN_PREPARING_MIN_ONLINE_TICKS =>
        {
            Some(CrisisPhase::AssaultReady)
        }
        _ => None,
    }
}

fn transition_goblin_crisis(
    crisis: &mut SettlementCrisis,
    game_tick: i32,
) -> Option<(CrisisPhase, CrisisPhase)> {
    let old_phase = crisis.phase;
    let new_phase = next_goblin_crisis_phase(crisis)?;

    crisis.phase = new_phase;
    crisis.phase_started_tick = game_tick;
    crisis.phase_online_ticks = 0;
    if new_phase == CrisisPhase::Preparing {
        crisis.warning_active = true;
    }

    Some((old_phase, new_phase))
}

fn advance_online_crisis_time(
    crisis: &mut SettlementCrisis,
    game_tick: i32,
    count_online_time: bool,
) -> i32 {
    let elapsed = game_tick.saturating_sub(crisis.last_evaluated_tick).max(0);
    // Keep the watermark monotonic. A transient tick rollback must not make
    // already-credited time eligible to be counted again when time catches up.
    crisis.last_evaluated_tick = crisis.last_evaluated_tick.max(game_tick);

    if count_online_time {
        crisis.online_active_ticks = crisis.online_active_ticks.saturating_add(elapsed);
        crisis.phase_online_ticks = crisis.phase_online_ticks.saturating_add(elapsed);
        if crisis.phase == CrisisPhase::AssaultActive {
            crisis.assault_online_ticks = crisis.assault_online_ticks.saturating_add(elapsed);
        }
        elapsed
    } else {
        0
    }
}

fn crisis_phase_name(phase: CrisisPhase) -> &'static str {
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

fn crisis_phase_presentation(
    phase: CrisisPhase,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match phase {
        CrisisPhase::Dormant => (
            "No Organized Threat",
            "Your settlement has not yet attracted organized goblin attention.",
            "Continue establishing your camp.",
            "quiet",
        ),
        CrisisPhase::Signs => (
            "Goblin Signs",
            "Tracks and distant movement suggest goblins are watching the settlement.",
            "Build supplies and improve your defenses.",
            "low",
        ),
        CrisisPhase::Pressure => (
            "Goblin Pressure",
            "Goblin raiders are testing the settlement and watching its growth.",
            "Prepare weapons, healing supplies, walls, and defenders.",
            "medium",
        ),
        CrisisPhase::Preparing => (
            "Raiders Gathering",
            "A major goblin raid is being organized against your settlement.",
            "Finish repairs, equip your defenders, and stock essential supplies.",
            "high",
        ),
        CrisisPhase::AssaultReady => (
            "Goblin Raid Imminent",
            "The raiders are ready. After the minimum warning, they favor dusk or night but will not wait indefinitely.",
            "Return to your settlement and prepare for the assault.",
            "crisis",
        ),
        CrisisPhase::AssaultActive => (
            "Settlement Under Attack",
            "Goblin raiders are attacking your settlement.",
            "Defeat the remaining attackers. This assault continues if you disconnect.",
            "crisis",
        ),
        CrisisPhase::Resolved => (
            "Goblin Raid Defeated",
            "The organized goblin assault has been defeated.",
            "Recover, repair, and rebuild.",
            "resolved",
        ),
    }
}

fn crisis_preparation_option(
    id: &str,
    label: &str,
    state: &str,
    detail: String,
    action_hint: &str,
) -> CrisisPreparationOption {
    CrisisPreparationOption {
        id: id.to_string(),
        label: label.to_string(),
        state: state.to_string(),
        detail,
        action_hint: action_hint.to_string(),
    }
}

fn derive_crisis_preparation_options(
    facts: &CrisisPreparationFacts,
) -> Vec<CrisisPreparationOption> {
    let defences = if facts.completed_walls > 0 && facts.damaged_walls == 0 {
        crisis_preparation_option(
            "defences",
            "Defences",
            "ready",
            format!(
                "{} completed wall{} fully repaired.",
                facts.completed_walls,
                if facts.completed_walls == 1 {
                    " is"
                } else {
                    "s are"
                }
            ),
            "No wall repair action is needed.",
        )
    } else if facts.damaged_walls > 0 && facts.living_villagers > 0 {
        crisis_preparation_option(
            "defences",
            "Defences",
            "needs_attention",
            format!(
                "{} of {} completed wall{} damaged.",
                facts.damaged_walls,
                facts.completed_walls,
                if facts.completed_walls == 1 {
                    " is"
                } else {
                    "s are"
                }
            ),
            "Order a living villager to repair a damaged wall.",
        )
    } else if facts.damaged_walls > 0 {
        crisis_preparation_option(
            "defences",
            "Defences",
            "unavailable",
            format!(
                "{} of {} completed wall{} damaged.",
                facts.damaged_walls,
                facts.completed_walls,
                if facts.completed_walls == 1 {
                    " is"
                } else {
                    "s are"
                }
            ),
            "Wall repair orders require a living owned villager.",
        )
    } else if facts.can_start_stockade {
        crisis_preparation_option(
            "defences",
            "Defences",
            "needs_attention",
            "No completed wall is present; a Stockade plan and 3 carried Log-compatible units are ready."
                .to_string(),
            "Place a Stockade foundation using the existing plan and carried materials.",
        )
    } else {
        let blocker = if !facts.live_hero {
            "A live owned hero is required to place a Stockade foundation."
        } else if !facts.hero_idle {
            "Finish the current hero action before placing a Stockade foundation."
        } else if facts.current_tile_wall_present {
            "Move to a tile without an existing wall before placing a Stockade foundation."
        } else if !facts.stockade_plan_available {
            "The Stockade plan is not available to this player."
        } else if facts.stockade_log_units_carried < 3 {
            "A Stockade requires 3 carried Log-compatible units."
        } else {
            "A completed wall is required before wall repairs are available."
        };
        crisis_preparation_option(
            "defences",
            "Defences",
            "unavailable",
            "No completed defensive walls are available.".to_string(),
            blocker,
        )
    };

    let unarmed_villagers = facts
        .living_villagers
        .saturating_sub(facts.combat_capable_villagers);
    let defenders = if facts.living_villagers == 0 {
        crisis_preparation_option(
            "defenders",
            "Defenders",
            "unavailable",
            "No living owned villagers are available.".to_string(),
            "A living villager is required for this preparation option.",
        )
    } else if unarmed_villagers == 0 {
        crisis_preparation_option(
            "defenders",
            "Defenders",
            "ready",
            format!(
                "{} living villager{} combat-capable.",
                facts.combat_capable_villagers,
                if facts.combat_capable_villagers == 1 {
                    " is"
                } else {
                    "s are"
                }
            ),
            "Your existing combat-capable defenders are ready.",
        )
    } else if facts.actionable_villager_weapons > 0 {
        crisis_preparation_option(
            "defenders",
            "Defenders",
            "needs_attention",
            format!(
                "{} of {} living villager{} combat-capable; {} spare weapon{} available.",
                facts.combat_capable_villagers,
                facts.living_villagers,
                if facts.living_villagers == 1 {
                    " is"
                } else {
                    "s are"
                },
                facts.actionable_villager_weapons,
                if facts.actionable_villager_weapons == 1 {
                    " is"
                } else {
                    "s are"
                }
            ),
            "Equip the weapon already carried by an idle unarmed villager.",
        )
    } else if facts.combat_capable_villagers > 0 {
        crisis_preparation_option(
            "defenders",
            "Defenders",
            "ready",
            format!(
                "{} of {} living villager{} combat-capable; no spare weapon is available.",
                facts.combat_capable_villagers,
                facts.living_villagers,
                if facts.living_villagers == 1 {
                    " is"
                } else {
                    "s are"
                }
            ),
            "Your existing combat-capable defenders are ready.",
        )
    } else {
        let blocker = if facts.villager_held_spare_weapons > 0 {
            "A carried villager weapon cannot be equipped until its unarmed owner is idle."
        } else if facts.hero_spare_weapons > 0 || facts.transferable_stored_weapons > 0 {
            "Available hero or storage weapons require a normal adjacent transfer first."
        } else {
            "An existing spare weapon is required to arm a villager."
        };
        crisis_preparation_option(
            "defenders",
            "Defenders",
            "unavailable",
            format!(
                "{} living villager{} unarmed, and no spare weapon is available.",
                facts.living_villagers,
                if facts.living_villagers == 1 {
                    " is"
                } else {
                    "s are"
                }
            ),
            blocker,
        )
    };

    let equipment = if !facts.live_hero {
        crisis_preparation_option(
            "equipment",
            "Equipment",
            "unavailable",
            "No live hero is available for equipment preparation.".to_string(),
            "A live owned hero is required for this preparation option.",
        )
    } else {
        let weapon_ready = facts.hero_equipped_weapon.is_some();
        let armor_ready = facts.hero_equipped_armor > 0;
        if weapon_ready && armor_ready {
            crisis_preparation_option(
                "equipment",
                "Equipment",
                "ready",
                format!(
                    "{} is equipped with {} armor piece{}.",
                    facts.hero_equipped_weapon.as_deref().unwrap_or("A weapon"),
                    facts.hero_equipped_armor,
                    if facts.hero_equipped_armor == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
                "Your hero's combat equipment is ready.",
            )
        } else {
            let carried_action = facts.hero_idle
                && ((!weapon_ready && facts.hero_carried_weapons > 0)
                    || (!armor_ready && facts.hero_carried_armor > 0));
            let storage_action = (!weapon_ready && facts.transferable_stored_weapons > 0)
                || (!armor_ready && facts.transferable_stored_armor > 0);
            let (detail, missing_label) =
                match (weapon_ready, armor_ready) {
                    (false, false) => (
                        format!(
                        "No weapon or armor is equipped; carried: {} weapon{}, {} armor piece{}.",
                        facts.hero_carried_weapons,
                        if facts.hero_carried_weapons == 1 { "" } else { "s" },
                        facts.hero_carried_armor,
                        if facts.hero_carried_armor == 1 { "" } else { "s" }
                    ),
                        "weapon or armor",
                    ),
                    (false, true) => (
                        format!(
                            "No weapon is equipped; {} armor piece{} equipped.",
                            facts.hero_equipped_armor,
                            if facts.hero_equipped_armor == 1 {
                                " is"
                            } else {
                                "s are"
                            }
                        ),
                        "weapon",
                    ),
                    (true, false) => (
                        format!(
                            "{} is equipped; no armor is equipped.",
                            facts.hero_equipped_weapon.as_deref().unwrap_or("A weapon")
                        ),
                        "armor",
                    ),
                    (true, true) => unreachable!(),
                };

            if carried_action || storage_action {
                let hint = match (carried_action, storage_action) {
                    (true, true) => {
                        "Equip carried gear or transfer missing gear from nearby storage."
                    }
                    (true, false) => "Equip the available carried gear while your hero is idle.",
                    (false, true) => {
                        "Transfer the missing gear from nearby storage, then equip it."
                    }
                    (false, false) => unreachable!(),
                };
                crisis_preparation_option("equipment", "Equipment", "needs_attention", detail, hint)
            } else {
                let has_carried_missing = (!weapon_ready && facts.hero_carried_weapons > 0)
                    || (!armor_ready && facts.hero_carried_armor > 0);
                let hint = if has_carried_missing && !facts.hero_idle {
                    "Finish the current hero action before changing carried equipment."
                } else if (!weapon_ready && facts.stored_weapons > 0)
                    || (!armor_ready && facts.stored_armor > 0)
                {
                    "Stored equipment is not currently available through normal item transfer."
                } else {
                    match missing_label {
                        "weapon" => "No weapon is carried or transferable from nearby storage.",
                        "armor" => "No armor is carried or transferable from nearby storage.",
                        _ => "No missing equipment is carried or transferable from nearby storage.",
                    }
                };
                crisis_preparation_option("equipment", "Equipment", "unavailable", detail, hint)
            }
        }
    };

    let recovery = if !facts.live_hero {
        crisis_preparation_option(
            "recovery",
            "Recovery",
            "unavailable",
            "No live hero is available to carry healing supplies.".to_string(),
            "A live owned hero is required for this preparation option.",
        )
    } else if facts.hero_carried_healing > 0 {
        crisis_preparation_option(
            "recovery",
            "Recovery",
            "ready",
            format!(
                "{} usable healing item{} carried by your hero.",
                facts.hero_carried_healing,
                if facts.hero_carried_healing == 1 {
                    " is"
                } else {
                    "s are"
                }
            ),
            "Your hero is carrying an existing recovery option.",
        )
    } else if facts.transferable_stored_healing > 0 {
        crisis_preparation_option(
            "recovery",
            "Recovery",
            "needs_attention",
            format!(
                "No healing is carried; {} usable item{} available in nearby storage.",
                facts.transferable_stored_healing,
                if facts.transferable_stored_healing == 1 {
                    " is"
                } else {
                    "s are"
                }
            ),
            "Transfer a usable healing item from nearby storage to your hero.",
        )
    } else if facts.stored_healing > 0 {
        crisis_preparation_option(
            "recovery",
            "Recovery",
            "unavailable",
            "No healing is carried; stored supplies are not currently transferable.".to_string(),
            "Stored healing must be available through normal item transfer.",
        )
    } else {
        crisis_preparation_option(
            "recovery",
            "Recovery",
            "unavailable",
            "No usable healing supplies are carried or held in completed storage.".to_string(),
            "An existing usable healing item is required for this preparation option.",
        )
    };

    let options = vec![defences, defenders, equipment, recovery];
    debug_assert!(options.len() <= 4);
    options
}

pub(crate) fn build_crisis_status(crisis: Option<&SettlementCrisis>) -> CrisisStatusSnapshot {
    let Some(crisis) = crisis else {
        return CrisisStatusSnapshot {
            version: CRISIS_STATUS_VERSION,
            exists: false,
            kind: None,
            phase: None,
            pressure: None,
            pressure_max: None,
            title: None,
            summary: None,
            action_hint: None,
            severity: None,
            warning: false,
            assault_ready: false,
            assault_active: false,
            resolved: false,
            remaining_attackers: None,
            total_attackers: None,
            preparation_seconds_remaining: None,
            preferred_launch_window: None,
            preparation_options: None,
            continues_while_disconnected: false,
        };
    };

    let (title, summary, action_hint, severity) = crisis_phase_presentation(crisis.phase);
    let assault_ready = crisis.phase == CrisisPhase::AssaultReady;
    let assault_active = crisis.phase == CrisisPhase::AssaultActive;
    let resolved = crisis.phase == CrisisPhase::Resolved;

    let (remaining_attackers, total_attackers) = if assault_active {
        let defeated = crisis
            .assault_defeated_unit_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        (
            Some(
                crisis
                    .assault_unit_ids
                    .iter()
                    .filter(|id| !defeated.contains(id))
                    .count() as i32,
            ),
            Some(crisis.assault_unit_ids.len() as i32),
        )
    } else {
        (None, None)
    };

    let preparation_seconds_remaining = assault_ready.then(|| {
        let remaining_ticks = (ASSAULT_READY_GRACE_TICKS - crisis.phase_online_ticks).max(0);
        (remaining_ticks + TICKS_PER_SEC - 1) / TICKS_PER_SEC
    });

    CrisisStatusSnapshot {
        version: CRISIS_STATUS_VERSION,
        exists: true,
        kind: Some(match crisis.kind {
            CrisisKind::Goblin => "goblin".to_string(),
        }),
        phase: Some(crisis_phase_name(crisis.phase).to_string()),
        pressure: Some(crisis.pressure),
        pressure_max: Some(GOBLIN_PRESSURE_MAX),
        title: Some(title.to_string()),
        summary: Some(summary.to_string()),
        action_hint: Some(action_hint.to_string()),
        severity: Some(severity.to_string()),
        warning: crisis.warning_active,
        assault_ready,
        assault_active,
        resolved,
        remaining_attackers,
        total_attackers,
        preparation_seconds_remaining,
        preferred_launch_window: assault_ready.then(|| "dusk_or_night".to_string()),
        preparation_options: None,
        continues_while_disconnected: assault_active,
    }
}

fn build_crisis_status_with_preparation(
    crisis: Option<&SettlementCrisis>,
    facts: Option<&CrisisPreparationFacts>,
) -> CrisisStatusSnapshot {
    let mut status = build_crisis_status(crisis);
    if crisis.is_some_and(|crisis| {
        matches!(
            crisis.phase,
            CrisisPhase::Preparing | CrisisPhase::AssaultReady
        )
    }) {
        status.preparation_options = facts.map(derive_crisis_preparation_options);
    }
    status
}

pub(crate) fn crisis_status_changed(
    previous: &CrisisStatusSnapshot,
    current: &CrisisStatusSnapshot,
) -> bool {
    if previous == current {
        return false;
    }

    // Pressure and the ready countdown are rate-limited display values. All
    // other snapshot changes (phase, warning, launch, roster, resolution, copy,
    // or clear state) are authoritative and send immediately.
    let mut structural = current.clone();
    structural.pressure = previous.pressure;
    structural.preparation_seconds_remaining = previous.preparation_seconds_remaining;
    if structural != *previous {
        return true;
    }

    let pressure_changed = match (previous.pressure, current.pressure) {
        (Some(previous), Some(current)) => {
            current.abs_diff(previous) >= CRISIS_STATUS_PRESSURE_DELTA as u32
        }
        (None, None) => false,
        _ => true,
    };
    if pressure_changed {
        return true;
    }

    match (
        previous.preparation_seconds_remaining,
        current.preparation_seconds_remaining,
    ) {
        (Some(previous), Some(current)) => {
            current.abs_diff(previous) >= CRISIS_STATUS_COUNTDOWN_DELTA_SECONDS as u32
        }
        (None, None) => false,
        _ => true,
    }
}

fn crisis_transition_notice(phase: CrisisPhase) -> Option<&'static str> {
    match phase {
        CrisisPhase::Preparing => Some("Goblin raiders are gathering. Prepare your settlement."),
        CrisisPhase::AssaultReady => Some("A goblin raid is imminent."),
        CrisisPhase::AssaultActive => {
            Some("The goblin assault has begun. It will continue if you disconnect.")
        }
        CrisisPhase::Resolved => Some("The goblin assault has been defeated."),
        _ => None,
    }
}

fn crisis_status_delivery_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    director: Res<SurvivalDirectorConfig>,
    crisis_state: Res<SettlementCrisisState>,
    mut login_sync: ResMut<CrisisStatusLoginSync>,
    mut delivery: ResMut<CrisisStatusDeliveryState>,
    mut telemetry_state: ResMut<CrisisTelemetryState>,
    mut balance_telemetry_state: ResMut<CrisisBalanceTelemetryState>,
    mut resume_login_sync: ResMut<ResumeLoginSyncState>,
    mut safe_logout_telemetry: ResMut<SafeLogoutTelemetryState>,
    preparation_collector: CrisisPreparationCollector,
) {
    let active_clients = match clients.lock() {
        Ok(clients) => clients
            .iter()
            .filter(|(client_id, client)| **client_id == client.id && !client.sender.is_closed())
            .map(|(client_id, client)| (*client_id, client.player_id, client.sender.clone()))
            .collect::<Vec<_>>(),
        Err(_) => return,
    };
    let active_clients = active_clients
        .into_iter()
        .filter(|(client_id, player_id, _)| clients.is_current_connection(*player_id, *client_id))
        .collect::<Vec<_>>();
    let active_client_players = active_clients
        .iter()
        .map(|(client_id, player_id, _)| (*client_id, *player_id))
        .collect::<HashMap<_, _>>();

    delivery
        .sent
        .retain(|client_id, sent| active_client_players.get(client_id) == Some(&sent.player_id));

    // Track actual phase transitions independently of login snapshots. This
    // prevents a reconnect from replaying historical launch notices.
    if director.mode == SurvivalDirectorMode::PersonalCrisis {
        let current_players = crisis_state.keys().copied().collect::<HashSet<_>>();
        delivery
            .observed_phases
            .retain(|player_id, _| current_players.contains(player_id));

        for (player_id, crisis) in crisis_state.iter() {
            match delivery.observed_phases.insert(*player_id, crisis.phase) {
                Some(previous) if previous != crisis.phase => {
                    if clients.is_player_online(*player_id) {
                        if let Some(message) = crisis_transition_notice(crisis.phase) {
                            send_to_client(
                                *player_id,
                                ResponsePacket::Notice {
                                    noticemsg: message.to_string(),
                                    expiry: None,
                                },
                                &clients,
                            );
                            info!(
                                "personal_crisis_notice_delivered player_id={} old_phase={:?} new_phase={:?}",
                                player_id, previous, crisis.phase
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    } else {
        delivery.observed_phases.clear();
    }

    for (client_id, player_id, _sender) in &active_clients {
        let status = if director.mode == SurvivalDirectorMode::PersonalCrisis {
            let crisis = crisis_state.get(player_id);
            if crisis.is_some_and(|crisis| {
                matches!(
                    crisis.phase,
                    CrisisPhase::Preparing | CrisisPhase::AssaultReady
                )
            }) {
                let facts = preparation_collector.collect(*player_id);
                build_crisis_status_with_preparation(crisis, Some(&facts))
            } else {
                build_crisis_status_with_preparation(crisis, None)
            }
        } else {
            build_crisis_status(None)
        };
        let previous = delivery.sent.get(client_id);
        let new_connection = previous.is_none();
        let resume_requires_sync = resume_login_sync
            .get(player_id)
            .map(|progress| progress.connection_id == *client_id && !progress.crisis_status_queued)
            .unwrap_or(false);
        let should_send = if resume_requires_sync {
            previous
                .map(|previous| previous.status != status)
                .unwrap_or(true)
        } else if new_connection {
            login_sync.contains(player_id)
        } else {
            previous.is_some_and(|previous| crisis_status_changed(&previous.status, &status))
        };

        if !should_send {
            if resume_requires_sync
                && previous
                    .map(|previous| previous.status == status)
                    .unwrap_or(false)
            {
                if let Some(progress) = resume_login_sync.get_mut(player_id) {
                    if progress.connection_id == *client_id {
                        progress.crisis_status_queued = true;
                    }
                }
            }
            continue;
        }

        let packet = ResponsePacket::CrisisStatus {
            status: status.clone(),
        };
        let Ok(serialized) = serde_json::to_string(&packet) else {
            error!(
                "personal_crisis_status_serialization_failed player_id={}",
                player_id
            );
            continue;
        };

        match clients.try_send_current_bundle(*player_id, *client_id, vec![serialized]) {
            Ok(()) => {
                delivery.sent.insert(
                    *client_id,
                    SentCrisisStatus {
                        player_id: *player_id,
                        status: status.clone(),
                    },
                );
                if let Some(telemetry) = telemetry_state.get_mut(player_id) {
                    telemetry.status_packets_sent = telemetry.status_packets_sent.saturating_add(1);
                    if new_connection {
                        telemetry.login_snapshots_sent =
                            telemetry.login_snapshots_sent.saturating_add(1);
                    }
                }
                if let Some(crisis) = crisis_state.get(player_id) {
                    let balance = balance_telemetry_state.entry(*player_id).or_default();
                    balance.warnings.record(
                        crisis.phase,
                        game_tick.0,
                        crisis.online_active_ticks,
                        true,
                        balance.latest_near_settlement,
                    );
                }
                if let Some(progress) = resume_login_sync.get_mut(player_id) {
                    if progress.connection_id == *client_id {
                        progress.crisis_status_queued = true;
                    }
                }
                info!(
                    "personal_crisis_status_sent player_id={} phase={} exists={} reason={}",
                    player_id,
                    status.phase.as_deref().unwrap_or("none"),
                    status.exists,
                    if new_connection { "login" } else { "changed" }
                );
            }
            Err(error) => {
                if error != CurrentConnectionSendError::Full {
                    safe_logout_telemetry.record_stale_connection_event(*player_id);
                }
                debug!(
                    "personal_crisis_status_send_deferred player_id={} error={:?}",
                    player_id, error
                );
            }
        }
    }

    // A login request is satisfied only after every active connection for that
    // player has a cached authoritative snapshot. Duplicate Login events on an
    // already-synchronized connection therefore do not emit duplicates.
    let pending_players = login_sync.iter().copied().collect::<Vec<_>>();
    for player_id in pending_players {
        let player_clients = active_client_players
            .iter()
            .filter_map(|(client_id, owner)| (*owner == player_id).then_some(*client_id))
            .collect::<Vec<_>>();
        if player_clients.is_empty()
            || player_clients
                .iter()
                .all(|client_id| delivery.sent.contains_key(client_id))
        {
            login_sync.remove(&player_id);
        }
    }
}

fn player_survival_day(
    game_tick: &GameTick,
    player_id: i32,
    player_intro_state: &PlayerIntroState,
) -> i32 {
    intro_age(game_tick, player_id, player_intro_state)
        .map(|age| (age.max(0) / GAME_TICKS_PER_DAY) + 1)
        .unwrap_or_else(|| game_tick.day())
}

fn player_days_survived(
    game_tick: &GameTick,
    player_id: i32,
    player_intro_state: &PlayerIntroState,
) -> i32 {
    (player_survival_day(game_tick, player_id, player_intro_state) - 1).max(0)
}

const LEGENDARY_BOSS: &str = "Fire Dragon";
const LEGENDARY_RAIDER: &str = "Wyvern Rider";
const LEGENDARY_TORCHBEARER: &str = "Great Troll";
const LEGENDARY_THIEF: &str = "Gryphon";
const LEGENDARY_CAPTAIN: &str = "Death Knight";
const LEGENDARY_HIDEOUT: &str = "Warlord Hideout";
const LEGENDARY_RUMOR_DAY: i32 = 6;
const LEGENDARY_ACTIVE_DAY: i32 = 7;
const LEGENDARY_STANDARD_WAVE_TICKS: i32 = 600;
const LEGENDARY_FAST_WAVE_TICKS: i32 = 300;
const LEGENDARY_FAST_AFTER_TICKS: i32 = GAME_TICKS_PER_DAY * 3;
const LEGENDARY_HIDEOUT_REVEAL_CAPTAINS: i32 = 2;
const LEGENDARY_HIDEOUT_REVEAL_WAVES: i32 = 4;
const LEGENDARY_HIDEOUT_REVEAL_RANGE: u32 = 5;
const UNDEAD_INCURSION_SURVIVAL_TICKS: i32 = GAME_TICKS_PER_DAY * 3;
const GOBLIN_PILLAGER_SURVIVAL_TICKS: i32 = GAME_TICKS_PER_DAY * 5;

// Fallback deadlines: if a crisis tier has not fired from its organic condition
// within this much survival time, force it so the threat curve keeps advancing
// for passive players. Staggered so escalation arrives in tier order even for a
// player who never leaves camp: T2@8m, T3@10m, then the tier 4/5 survival-time
// primaries (3 days ≈ 12m, 5 days ≈ 20m) take over; the 16m/24m fallbacks are
// pure backstops should those primary thresholds ever be raised past them.
const WOLF_PACK_FALLBACK_TICKS: i32 = TICKS_PER_SEC * 60 * 8; // 8 min
const GOBLIN_RAID_FALLBACK_TICKS: i32 = TICKS_PER_SEC * 60 * 10; // 10 min
const UNDEAD_INCURSION_FALLBACK_TICKS: i32 = TICKS_PER_SEC * 60 * 16; // 16 min
const GOBLIN_PILLAGER_FALLBACK_TICKS: i32 = TICKS_PER_SEC * 60 * 24; // 24 min

pub fn crisis_tier(crisis: &PlayerCrisis) -> i32 {
    let mut tier = 0;
    if crisis.rat_spoilage {
        tier = 1;
    }
    if crisis.wolf_pack {
        tier = 2;
    }
    if crisis.goblin_raid {
        tier = 3;
    }
    if crisis.undead_incursion {
        tier = 4;
    }
    if crisis.goblin_pillager {
        tier = 5;
    }
    tier
}

pub fn survival_director_active(day: i32, objectives: Option<&PlayerObjectives>) -> bool {
    // Heavy scaling hordes hold off until day 8, widening the early calm window so
    // the player can hunt + cook + bank a food reserve before food-gathering gets
    // cut off by nightly sieges. (Days 2-7 still face the lighter fixed waves.)
    day >= 8 || objectives.map(|obj| obj.survive_5_nights).unwrap_or(false)
}

pub fn survival_horde_size(day: i32, crisis_tier: i32, active_legendary_count: i32) -> usize {
    let day_pressure = ((day - 6).max(0)) / 2;
    (2 + day_pressure + crisis_tier + active_legendary_count * 2).clamp(2, 12) as usize
}

pub fn score_total_from_breakdown(
    breakdown: &network::ScoreBreakdown,
    highest_pressure_level: i32,
) -> i32 {
    let components_sum = breakdown.survival
        + breakdown.progression
        + breakdown.wealth
        + breakdown.defense
        + breakdown.valor
        + breakdown.legacy;
    ((components_sum as f32) * (1.0 + highest_pressure_level as f32 * 0.05)).floor() as i32
}

#[derive(Debug, Clone, Default)]
pub struct RunScoreInputs {
    pub days_survived: i32,
    pub nights_survived: i32,
    pub waves_survived: i32,
    pub active_legendary_days: i32,
    pub hero_rank: String,
    pub total_skill_levels: i32,
    pub total_xp: i32,
    pub total_wealth_value: i32,
    pub structures_alive: i32,
    pub upgrades: i32,
    pub repairs: i32,
    pub villagers_alive: i32,
    pub crisis_tier: i32,
    pub enemies_killed: i32,
    pub elites_killed: i32,
    pub captains_killed: i32,
    pub legendary_kills: i32,
    pub hideouts_cleared: i32,
    pub completed_objectives: i32,
    pub monolith_sealed: bool,
}

pub fn calculate_run_score_breakdown(inputs: &RunScoreInputs) -> network::ScoreBreakdown {
    network::ScoreBreakdown {
        survival: inputs.days_survived * 500
            + inputs.nights_survived * 250
            + inputs.waves_survived * 400
            + inputs.active_legendary_days * 200,
        progression: rank_score(&inputs.hero_rank)
            + inputs.total_skill_levels * 100
            + (inputs.total_xp / 5).min(8000),
        wealth: (((inputs.total_wealth_value.max(0) as f64).sqrt() * 100.0).floor() as i32)
            .min(10000),
        defense: inputs.structures_alive * 150
            + inputs.upgrades * 300
            + inputs.repairs * 50
            + inputs.villagers_alive * 250
            + inputs.crisis_tier * 1000,
        valor: inputs.enemies_killed * 25
            + inputs.elites_killed * 500
            + inputs.captains_killed * 1500
            + inputs.legendary_kills * 10000
            + inputs.hideouts_cleared * 3000,
        legacy: inputs.completed_objectives * 250 + if inputs.monolith_sealed { 5000 } else { 0 },
    }
}

fn pressure_level_value(level: &str) -> i32 {
    match level {
        "Crisis" => 3,
        "High" => 2,
        "Building" => 1,
        _ => 0,
    }
}

fn completed_objectives_count(obj: Option<&PlayerObjectives>) -> i32 {
    let Some(obj) = obj else {
        return 0;
    };

    [
        obj.scavenge_shipwreck,
        obj.build_campfire,
        obj.win_first_fight,
        obj.build_3_structures,
        obj.recruit_villager,
        obj.explore_poi,
        obj.choose_expansion,
        obj.survive_5_nights,
        obj.find_legendary_hideout,
        obj.defeat_ashen_warlord,
    ]
    .iter()
    .filter(|done| **done)
    .count() as i32
}

fn rank_score(template_name: &str) -> i32 {
    if template_name.starts_with("Legendary ") {
        12000
    } else if template_name.starts_with("Great ") {
        6000
    } else if template_name.starts_with("Skilled ") {
        2000
    } else {
        0
    }
}

fn survival_horde_composition(size: usize, day: i32) -> Vec<&'static str> {
    let mut rotation = vec!["Ghoul", "Ghast", "Direwolf", "Gryphon"];
    if day >= 8 {
        rotation.extend(["Spectre", "Wraith", "Ogre"]);
    }
    if day >= 10 {
        rotation.extend(["Bone Knight", "Chocobone", "Dark Sorcerer"]);
    }
    if day >= 12 {
        rotation.extend(["Death Knight", "Draug", "Great Troll", "Roc"]);
    }
    if day >= 14 {
        rotation.extend(["Lich", "Drake Hurricane", "Wose Shaman"]);
    }
    if day >= 16 {
        rotation.extend([
            "Ancient Lich",
            "Drake Flameheart",
            "Ancient Wose",
            "Elder Wose",
        ]);
    }
    if day >= 18 {
        rotation.extend(["Drake Armageddon", "Wyvern Rider"]);
    }

    (0..size)
        .map(|index| rotation[(index + day as usize) % rotation.len()])
        .collect()
}

fn shipwreck_inspection_can_spawn_villager(
    game_tick: i32,
    entry: &InitialEncounterEntry,
    objectives: Option<&PlayerObjectives>,
) -> bool {
    game_tick >= entry.villager_ready_tick
        && objectives
            .map(|obj| obj.scavenge_shipwreck)
            .unwrap_or(false)
}

fn is_elite_enemy_template(template: &str) -> bool {
    matches!(
        template,
        "Elite Zombie"
            | "Necromancer"
            | "Goblin Pillager"
            | "Wolf Rider"
            | "Dark Sorcerer"
            | "Lich"
            | "Ancient Lich"
            | "Death Knight"
            | "Bone Knight"
            | "Draug"
            | "Wraith"
            | "Spectre"
            | "Ancient Wose"
            | "Elder Wose"
            | "Wose Shaman"
            | "Great Troll"
            | "Ogre"
            | "Gryphon"
            | "Roc"
            | "Wyvern Rider"
            | "Drake Hurricane"
            | "Drake Flameheart"
            | "Drake Armageddon"
            | "Fire Dragon"
    )
}

#[derive(Resource, Debug, Reflect, Default)]
#[reflect(Resource)]
pub struct DamageRecord {
    pub source: String,
    pub target: String,
    pub amount: i32,
    pub damage_type: String,
    pub tick: i32,
}

#[derive(Debug, Reflect, Component, Default)]
#[reflect(Component)]
pub struct Merchant {
    pub trade_port: Position,
    pub landing_at: Position,
    pub wanted_items: Vec<WantedItem>,
    /// Lifecycle phase. Drives the per-tick `merchant_sailing_system`
    /// (movement) and `merchant_arrival_system` (transition + announce).
    pub sail_state: MerchantSailState,
}

#[derive(Debug, Reflect, Default, Clone, Copy, PartialEq, Eq)]
pub enum MerchantSailState {
    /// Sitting at `trade_port` offshore, waiting for the next scheduled
    /// `MerchantArrival` event.
    #[default]
    AtEmpire,
    /// In transit from `trade_port` toward `landing_at`. Movement system
    /// drives one tile per `BASE_MOVE_TICKS / speed` ticks.
    SailingToLanding,
    /// Arrived at `landing_at`. Trade window is open. The follow-up
    /// `MerchantLeavingSoon` and `MerchantDeparture` events were scheduled
    /// when the merchant transitioned into this state.
    AtLanding,
    /// In transit back to `trade_port` after the trade window closed.
    SailingToEmpire,
}

// Canonical merchant inventory — used at first spawn (player_setup) and on
// every restock when the merchant returns from the empire. Each tuple is
// (item_name, quantity).
pub const MERCHANT_INVENTORY: &[(&str, i32)] = &[
    ("Gold Coins", 1500),
    ("Yurt Deed", 1),
    ("Lumbercamp Deed", 1),
    ("Quarry Deed", 1),
    ("Trapper Deed", 1),
    ("Mine Deed", 1),
    ("Farm Deed", 1),
    ("Small Tent Deed", 1),
    ("Training Pick Axe", 1),
    ("Sickle", 1),
    ("Bucket", 2),
    ("Bedroll", 1),
    ("Resin Torch", 5),
    ("Health Potion", 3),
    ("Seeds", 25),
    ("Honeybell Cloth", 10),
];

// Canonical wanted-item subclasses — refreshed on each restock so the merchant
// always offers the same trade categories. Live name/quantity/price are filled
// in by info_merchant_system at request time from the Prices resource.
pub const MERCHANT_WANTED_SUBCLASSES: &[&str] = &[
    "Copper Ore",
    "Iron Ore",
    "Copper Ingot",
    "Iron Ingot",
    "Maple Log",
    "Maple Timber",
    "Raw Hide",
    "Stiff Leather",
    "Cooked Meat",
    "Honeybell Cloth",
];

// Lifecycle tick offsets (10 ticks per second).
const MERCHANT_LEAVING_SOON_OFFSET: i32 = 2400; // 4 min after arrival
const MERCHANT_DEPARTURE_OFFSET: i32 = 3000; // 5 min after arrival
const MERCHANT_RETURN_GAP: i32 = 6000; // 10 min away at the empire
const MERCHANT_FIRST_ARRIVAL_DELAY: i32 = 1800; // 3 min after villager rescue
const NECRO_EVENT_DELAY_AFTER_RESCUE: i32 = 3000; // 5 min after villager rescue
const NECROMANCER_SPAWN_SEARCH_RADIUS: i32 = 5;

// Bundle of game_event_system parameters that don't fit in the 16-param limit.
#[derive(SystemParam)]
pub struct GameEventExtras<'w, 's> {
    pub merchant_query: Query<'w, 's, &'static mut Merchant>,
    pub initial_encounter_state: Res<'w, InitialEncounterState>,
    pub plans: ResMut<'w, Plans>,
    pub sanctuary_login_checks: ResMut<'w, SanctuaryLoginChecks>,
    pub crisis_status_login_sync: ResMut<'w, CrisisStatusLoginSync>,
    resume_login_sync: ResMut<'w, ResumeLoginSyncState>,
    // Used to give a spawned villager its player's start-location team color.
    pub assigned_start_locations: Res<'w, AssignedStartLocations>,
    pub run_spawned_objs: ResMut<'w, RunSpawnedObjs>,
    pub presence: Res<'w, PlayerWorldPresenceState>,
    pub safe_logout_telemetry: ResMut<'w, SafeLogoutTelemetryState>,
}

#[derive(Debug, Reflect, Component, Default)]
#[reflect(Component)]
pub struct Minions {
    pub ids: Vec<i32>,
}

#[derive(Debug, Reflect, Component, Default)]
#[reflect(Component)]
pub struct Home {
    pub pos: Position,
}

#[derive(Debug, Component, Clone)]
pub struct WanderingBehavior {
    pub num_moves: i32,
}

#[derive(Debug, Component, Clone)]
pub struct HunterBehavior {
    pub target: i32,
}

#[derive(Debug, Component, Clone)]
pub struct SpoilTargetBehavior {
    pub target: i32,
    pub item_classes: Vec<String>,
}

#[derive(Debug, Component, Clone)]
pub struct StealTargetBehavior {
    pub target: i32,
    pub item_types: Vec<String>,
}

#[derive(Debug, Component, Clone)]
pub struct EncounterMoves(pub i32);

#[derive(Debug, Component)]
pub struct EventInProgress {
    pub event_id: uuid::Uuid,
}

#[derive(Debug, Component)]
pub struct Fortified {
    pub id: i32,
}

fn wall_grants_fortification(state: &State) -> bool {
    Structure::is_built(*state)
}

// Builders can still be in Building on the tick their wall completes.
fn occupant_receives_wall_fortification(state: &State) -> bool {
    state.is_active() || matches!(state, State::Building)
}

#[derive(Debug, Component)]
pub struct Burning {
    pub start_tick: i32,
    pub dps: i32,
    pub state: String,
}

#[derive(Debug, Component)]
pub struct Sanctuary {
    pub id: i32,
    pub pos: Position,
}

#[derive(Debug, Component)]
pub struct WeakSanctuary {
    pub id: i32,
    pub pos: Position,
}

#[derive(Debug, Component)]
pub struct EffectAdded {
    pub effect: Effect,
}

/// Marks the one-shot stat modifier for an [`EffectAdded`] payload as applied.
/// Keeping the payload pending while its owner is protected lets the modifier
/// resume exactly once instead of being lost with Bevy's one-update `Added`
/// filter.
#[derive(Debug, Component)]
struct EffectAddedProcessed;

#[derive(Debug, Component)]
pub struct Monolith {
    pub soulshards: i32,
    /// How far the player has empowered this Monolith's protective sanctuary.
    /// Level 0 is the innate zone; each level (bought with Soulshards via
    /// `PlayerEvent::UpgradeSanctuary`) widens the suppression radius and the
    /// in-zone defensive bonus. See [`sanctuary_full_radius`] / [`SanctuaryZones`].
    pub sanctuary_level: i32,
}

#[derive(Debug, Component)]
pub struct BoundMonolith {
    pub id: i32,
    pub pos: Position,
}

#[derive(QueryData)]
#[query_data(derive(Debug))]
pub struct MapObjQuery {
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub name: &'static Name,
    pub template: &'static Template,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub state: &'static State,
    pub misc: &'static Misc,
    pub build_upgrade_state: Option<&'static BuildUpgradeState>,
}

#[derive(QueryData)]
#[query_data(derive(Debug))]
pub struct ObjQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub name: &'static Name,
    pub template: &'static Template,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub state: &'static State,
    //pub viewshed: &'static Viewshed,
    pub misc: &'static Misc,
    pub build_upgrade_state: Option<&'static BuildUpgradeState>,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct MoverQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub name: &'static Name,
    pub template: &'static Template,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub state: &'static State,
    pub misc: &'static Misc,
    pub move_event: &'static mut MoveEvent,
}

#[derive(QueryData)]
#[query_data(derive(Debug))]
pub struct ObjQueryVision {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub name: &'static Name,
    pub template: &'static Template,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub state: &'static State,
    pub viewshed: &'static Viewshed,
    pub misc: &'static Misc,
    pub build_upgrade_state: Option<&'static BuildUpgradeState>,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct ObjWithStatsQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static mut Position,
    pub name: &'static mut Name,
    pub template: &'static mut Template,
    pub class: &'static mut Class,
    pub subclass: &'static mut Subclass,
    pub state: &'static mut State,
    pub misc: &'static mut Misc,
    pub stats: &'static mut Stats,
    pub inventory: &'static mut Inventory,
    pub effects: &'static mut Effects,
    pub last_combat_tick: &'static LastCombatTick,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct FisherQuery {
    pub player_id: &'static PlayerId,
    pub id: &'static Id,
    pub pos: &'static mut Position,
    pub name: &'static mut Name,
    pub template: &'static mut Template,
    pub class: &'static mut Class,
    pub subclass: &'static mut Subclass,
    pub inventory: &'static mut Inventory,
    pub skills: &'static mut Skills,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
struct StructureQuery {
    entity: Entity,
    id: &'static Id,
    player_id: &'static PlayerId,
    pos: &'static Position,
    name: &'static Name,
    class: &'static Class,
    subclass: &'static Subclass,
    template: &'static Template,
    state: &'static State,
    work_queue: &'static mut WorkQueue,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct UpdateObjQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static mut PlayerId,
    pub pos: &'static Position,
    pub name: &'static Name,
    pub template: &'static mut Template,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub misc: &'static mut Misc,
    pub effects: &'static Effects,
    pub inventory: &'static Inventory,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct ObjQueryMutPlayerTemplate {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static mut PlayerId,
    pub pos: &'static Position,
    pub name: &'static Name,
    pub template: &'static mut Template,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub state: &'static mut State,
    //pub viewshed: &'static mut Viewshed,  // Not used and not all components have viewshed
    pub misc: &'static mut Misc,
    pub effects: &'static Effects,
    pub inventory: &'static mut Inventory,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct GathererQuery {
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub name: &'static Name,
    pub template: &'static Template,
    pub subclass: &'static Subclass,
    pub state: &'static State,
    pub effects: &'static Effects,
    pub inventory: &'static mut Inventory,
    pub skills: &'static mut Skills,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct ObjQueryMut {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static mut Position,
    pub name: &'static mut Name,
    pub template: &'static mut Template,
    pub class: &'static Class,
    pub subclass: &'static mut Subclass,
    pub state: &'static mut State,
    pub viewshed: &'static Viewshed,
    pub misc: &'static Misc,
    pub effects: &'static mut Effects,
    pub inventory: &'static mut Inventory,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct AllObjsQueryMut {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static mut Position,
    pub name: &'static mut Name,
    pub template: &'static mut Template,
    pub class: &'static Class,
    pub subclass: &'static mut Subclass,
    pub state: &'static mut State,
    pub misc: &'static Misc,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct ObjQueryMutStats {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static mut Position,
    pub name: &'static mut Name,
    pub template: &'static mut Template,
    pub class: &'static Class,
    pub subclass: &'static mut Subclass,
    pub state: &'static mut State,
    pub viewshed: &'static Viewshed,
    pub misc: &'static Misc,
    pub stats: &'static mut Stats,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct HeroResurrectQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static mut Position,
    pub name: &'static mut Name,
    pub template: &'static mut Template,
    pub class: &'static Class,
    pub subclass: &'static mut Subclass,
    pub state: &'static mut State,
    pub viewshed: &'static Viewshed,
    pub misc: &'static Misc,
    pub stats: &'static mut Stats,
    pub skills: &'static mut Skills,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct VillagerQuery {
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub state: &'static mut State,
    pub order: &'static Order,
}
pub struct GamePlugin {
    pub new_game: bool,
    // When true the production network/tokio path is skipped and the world is
    // built with `new_game_setup_headless`. The in-process headless test harness
    // (see `headless.rs`) inserts the network resources itself. Always false for
    // the real server.
    pub headless: bool,
    pub survival_director_mode: SurvivalDirectorMode,
}

impl Default for GamePlugin {
    fn default() -> Self {
        Self {
            new_game: true,
            headless: false,
            survival_director_mode: SurvivalDirectorMode::PersonalCrisis,
        }
    }
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SurvivalDirectorConfig::new(self.survival_director_mode));

        if self.new_game {
            if self.headless {
                app.add_systems(PreStartup, Game::new_game_setup_headless);
            } else {
                app.add_systems(PreStartup, Game::new_game_setup);
            }
        } else {
            app.add_systems(PreStartup, Game::reload_game);
            //app.add_systems(PreUpdate, Game::check_resources_ready.run_if(in_state(AppState::PreRunning));
            app.add_systems(
                PreUpdate,
                Game::loading_complete.run_if(in_state(AppState::Loading)),
            );
            app.add_systems(
                PreUpdate,
                Game::load_entity_map.run_if(in_state(AppState::PreRunning)),
            );
        }

        app.add_plugins(BigBrainPlugin::new(PreUpdate));

        app.add_plugins(MapPlugin)
            .add_plugins(PlayerPlugin)
            .add_plugins(TemplatesPlugin)
            .add_plugins(ItemPlugin)
            .add_plugins(ResourcePlugin)
            .add_plugins(TerrainFeaturePlugin)
            .add_plugins(SkillPlugin)
            .add_plugins(RecipePlugin)
            .add_plugins(ExperimentPlugin)
            .add_plugins(StructurePlugin)
            .add_plugins(FarmPlugin)
            .add_plugins(WorldPlugin)
            .add_plugins(NPCPlugin)
            .add_plugins(VillagerPlugin)
            .add_plugins(TaxCollectorPlugin)
            .add_plugins(SafeLogoutPlugin);
        app.add_systems(OnEnter(AppState::Running), init_objs);
        app.add_systems(OnEnter(AppState::Running), inject_log_reload_handle);
        app.init_resource::<SanctuaryExcursions>();
        app.init_resource::<SanctuaryLoginChecks>();
        app.init_resource::<SurveyHistory>();
        app.init_resource::<InvestigatedPOIs>();
        app.init_resource::<IntroEncounterState>();
        app.init_resource::<SettlementCrisisState>();
        app.init_resource::<NextCrisisAssaultId>();
        app.init_resource::<CrisisTelemetryState>();
        app.init_resource::<CrisisBalanceTelemetryState>();
        app.init_resource::<CrisisBalanceTelemetryConfig>();
        app.init_resource::<CrisisBalanceObservationState>();
        app.init_resource::<CrisisStatusLoginSync>();
        app.init_resource::<CrisisStatusDeliveryState>();
        app.init_resource::<ResumeLoginSyncState>();

        if !self.headless {
            // The simulation harness must not exercise the live server's
            // filesystem persistence path. Production keeps the existing
            // fail-fast snapshot behavior unchanged.
            app.add_systems(Update, snapshot_system.run_if(in_state(AppState::Running)));
        }

        app.add_systems(Update, update_game_tick.run_if(in_state(AppState::Running)))
            .add_systems(
                Update,
                stamina_recovery_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                combat_lock_interrupt_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                stamina_update_system
                    .after(stamina_recovery_system)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                mana_update_system
                    .after(stamina_recovery_system)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                (move_event_system, move_event_completed_system)
                    .chain()
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                hide_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                update_obj_event_system.run_if(in_state(AppState::Running)),
            )
            /* .add_systems(
                Update,
                update_obj_vision_system.run_if(in_state(AppState::Running)),
            ) */
            .add_systems(
                Update,
                activate_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                forage_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                gather_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                refine_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                craft_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                experiment_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                explore_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                investigate_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                farm_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                repair_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                spell_raise_dead_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                player_intro_state_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                personal_crisis_system
                    .after(player_intro_state_system)
                    .after(update_game_tick)
                    .run_if(in_state(AppState::Running))
                    .run_if(personal_survival_director),
            )
            .add_systems(
                Update,
                personal_crisis_assault_system
                    .after(personal_crisis_system)
                    .after(sanctuary_zones_sync_system)
                    .after(hero_dead_system)
                    .after(resurrect_system)
                    .after(true_death_system)
                    .after(BigBrainSet::Actions)
                    .before(map_event_system)
                    .before(remove_dead_system)
                    .before(despawn_wandering_npc_system)
                    .before(perception_system)
                    .run_if(in_state(AppState::Running))
                    .run_if(personal_survival_director),
            )
            .add_systems(
                Update,
                rat_event_system
                    .run_if(in_state(AppState::Running))
                    .run_if(legacy_survival_director),
            )
            .add_systems(
                Update,
                initial_encounter_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                wolf_pack_system
                    .run_if(in_state(AppState::Running))
                    .run_if(legacy_survival_director),
            )
            .add_systems(
                Update,
                goblin_raid_system
                    .run_if(in_state(AppState::Running))
                    .run_if(legacy_survival_director),
            )
            .add_systems(
                Update,
                undead_incursion_system
                    .run_if(in_state(AppState::Running))
                    .run_if(legacy_survival_director),
            )
            .add_systems(
                Update,
                goblin_pillager_system
                    .run_if(in_state(AppState::Running))
                    .run_if(legacy_survival_director),
            )
            .add_systems(
                Update,
                nightly_threat_system
                    .run_if(in_state(AppState::Running))
                    .run_if(legacy_survival_director),
            )
            .add_systems(
                Update,
                legendary_threat_system
                    .run_if(in_state(AppState::Running))
                    .run_if(legacy_survival_director),
            )
            .add_systems(
                Update,
                legendary_death_tracking_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                run_score_kill_tracking_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                wildness_reduction_on_enemy_death_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                wildness_regen_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                objectives_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                sanctuary_zones_sync_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                merchant_sailing_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                merchant_arrival_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(Update, map_event_system.run_if(in_state(AppState::Running)))
            .add_systems(
                Update,
                weather_cycle_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                weather_effects_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(Update, morale_system.run_if(in_state(AppState::Running)))
            .add_systems(
                Update,
                victory_check_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                spell_damage_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                broadcast_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                effect_expired_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                cooldown_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(Update, use_item_system.run_if(in_state(AppState::Running)))
            .add_systems(Update, drink_eat_system.run_if(in_state(AppState::Running)))
            .add_systems(
                Update,
                hero_auto_consume_system
                    .after(use_item_system)
                    .after(drink_eat_system)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                find_shelter_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                steal_spoil_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                fishing_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                visible_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                game_event_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                sanctuary_login_system
                    .after(game_event_system)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                crisis_balance_snapshot_system
                    .after(personal_crisis_assault_system)
                    .after(game_event_system)
                    .before(crisis_status_delivery_system)
                    .run_if(in_state(AppState::Running))
                    .run_if(personal_survival_director),
            )
            .add_systems(
                Update,
                crisis_engagement_snapshot_system
                    .after(crisis_balance_snapshot_system)
                    .after(resurrect_system)
                    .after(true_death_system)
                    .before(crisis_status_delivery_system)
                    .run_if(in_state(AppState::Running))
                    .run_if(personal_survival_director),
            )
            .add_systems(
                Update,
                crisis_true_death_telemetry_system
                    .after(resurrect_system)
                    .before(crisis_engagement_snapshot_system)
                    .run_if(in_state(AppState::Running))
                    .run_if(personal_survival_director),
            )
            .add_systems(
                Update,
                crisis_status_delivery_system
                    .after(game_event_system)
                    .after(personal_crisis_system)
                    .after(personal_crisis_assault_system)
                    .after(true_death_system)
                    .run_if(in_state(AppState::Running)),
            )
            /* .add_systems(
                Update,
                cancel_game_event_system.run_if(in_state(AppState::Running)),
            )*/
            .add_systems(
                Update,
                (
                    state_dead_system.run_if(in_state(AppState::Running)),
                    resurrect_system.run_if(in_state(AppState::Running)),
                    hero_dead_system.run_if(in_state(AppState::Running)),
                    true_death_system.run_if(in_state(AppState::Running)),
                ),
            )
            .add_systems(
                Update,
                (
                    remove_dead_system.run_if(in_state(AppState::Running)),
                    hero_needs_warning_system.run_if(in_state(AppState::Running)),
                    dehydrated_system.run_if(in_state(AppState::Running)),
                    starving_system.run_if(in_state(AppState::Running)),
                    exhausted_system.run_if(in_state(AppState::Running)),
                    burning_system.run_if(in_state(AppState::Running)),
                    item_duration_system.run_if(in_state(AppState::Running)),
                    fuel_system.run_if(in_state(AppState::Running)),
                    effect_added_system.run_if(in_state(AppState::Running)),
                    inventory_changed_system.run_if(in_state(AppState::Running)),
                    skill_changed_system.run_if(in_state(AppState::Running)),
                    structure_refine_event_system.run_if(in_state(AppState::Running)),
                    structure_craft_event_system.run_if(in_state(AppState::Running)),
                    structure_operate_event_system.run_if(in_state(AppState::Running)),
                    build_system.run_if(in_state(AppState::Running)),
                    upgrade_system.run_if(in_state(AppState::Running)),
                ),
            )
            .add_systems(
                Update,
                despawn_wandering_npc_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                perception_system
                    .after(game_event_system)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                resume_login_sync_completion_system
                    .after(crisis_status_delivery_system)
                    .after(perception_system)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                watchtower_reveal_system
                    .run_if(in_state(AppState::Running))
                    .before(perception_system),
            )
            .add_systems(
                Update,
                reveal_unhidden_system
                    .run_if(in_state(AppState::Running))
                    .before(perception_system),
            )
            .add_systems(
                Update,
                work_queue_update_system.run_if(in_state(AppState::Running)),
            )
            .add_observer(state_change_observer)
            .add_observer(template_change_observer)
            .add_observer(new_obj_observer)
            .add_observer(remove_obj_observer)
            .add_observer(start_build_observer)
            .add_observer(start_upgrade_observer)
            .add_observer(start_work_observer)
            .add_observer(build_progress_update_observer)
            .add_observer(transfer_all_resources_observer)
            .add_observer(food_poisoning_effect_observer)
            .add_observer(remove_worker_from_work_queue_observer)
            .add_observer(cancel_events_observer)
            .add_observer(update_obj_observer)
            .add_observer(add_light_effect_system)
            .add_observer(remove_light_effect_system)
            .add_observer(crisis_combat_telemetry_observer)
            .add_observer(crisis_attack_telemetry_observer);
    }
}

#[derive(Resource)]
pub struct DynamicSceneHandle(pub Handle<DynamicScene>);

fn inject_log_reload_handle(mut log_overrides: ResMut<LogLevelOverrides>) {
    if let Ok(handle_lock) = crate::LOG_RELOAD_HANDLE.lock() {
        if let Some(reload_handle) = &*handle_lock {
            log_overrides.reload_handle = Some(Arc::new(Mutex::new(reload_handle.clone())));
            info!("Log reload handle injected successfully");
        }
    }
}

pub struct Game {
    pub num_players: u32,
}

impl Game {
    // Production new-game setup: build the world, wire up the network/tokio path,
    // then enter Running. Behaviour-preserving wrapper around `world_init` +
    // `network_init` (split so the headless harness can reuse `world_init` and
    // supply its own in-process network resources instead).
    pub fn new_game_setup(
        mut commands: Commands,
        recipes: ResMut<Recipes>,
        resources: ResMut<Resources>,
        terrain_features: ResMut<TerrainFeatures>,
        templates: Res<Templates>,
        map: Res<Map>,
        mut next_state: ResMut<NextState<AppState>>,
    ) {
        println!("Bevy Setup System");

        Self::world_init(
            &mut commands,
            recipes,
            resources,
            terrain_features,
            templates,
            map,
        );
        Self::network_init(&mut commands);

        next_state.set(AppState::Running);
    }

    // Headless new-game setup for the in-process test harness: build the world
    // and enter Running, WITHOUT spawning the tokio/WebSocket/Postgres network
    // path. The harness inserts `NetworkReceiver`/`Clients`/`DatabaseManagers`
    // itself (see `headless.rs`) before pumping the app.
    pub fn new_game_setup_headless(
        mut commands: Commands,
        recipes: ResMut<Recipes>,
        resources: ResMut<Resources>,
        terrain_features: ResMut<TerrainFeatures>,
        templates: Res<Templates>,
        map: Res<Map>,
        mut next_state: ResMut<NextState<AppState>>,
    ) {
        Self::world_init(
            &mut commands,
            recipes,
            resources,
            terrain_features,
            templates,
            map,
        );

        next_state.set(AppState::Running);
    }

    // World-only initialization: spawn resources/terrain, load recipes/prices and
    // insert every per-game state resource. Deliberately EXCLUDES the three
    // network resources (`NetworkReceiver`/`Clients`/`DatabaseManagers`) and the
    // tokio_setup spawn — those are handled by `network_init` (production) or by
    // the headless harness. Does not change AppState.
    pub fn world_init(
        commands: &mut Commands,
        mut recipes: ResMut<Recipes>,
        mut resources: ResMut<Resources>,
        mut terrain_features: ResMut<TerrainFeatures>,
        templates: Res<Templates>,
        map: Res<Map>,
    ) {
        // Initialize game tick
        let game_tick: GameTick = GameTick(EVENING); // Set to Evening for testing campfire vision

        // Initialize map events vector
        let map_events: MapEvents = MapEvents(HashMap::new());
        let processed_map_events: VisibleEvents = VisibleEvents(Vec::new());

        let game_events: GameEvents = GameEvents(HashMap::new());

        let perception_updates: PerceptionUpdates = PerceptionUpdates(HashSet::new());

        // Initialize explored map
        let explored_map: ExploredMap = ExploredMap(HashMap::new());
        let survey_history: SurveyHistory = SurveyHistory(HashMap::new());
        let investigated_pois: InvestigatedPOIs = InvestigatedPOIs(HashMap::new());

        // Initialize indexes
        let ids: Ids = Ids {
            map_event: 0,
            player_event: 0,
            obj: 0,
            item: 0,
            player_hero_map: HashMap::new(),
            obj_player_map: HashMap::new(),
        };

        // Initialize Entity to Obj Id map
        let entity_obj_map: EntityObjMap = EntityObjMap(HashMap::new());

        // Initialize game world
        info!("Spawning resources...");
        Resource::spawn_all_resources(&mut resources, &templates, &map);
        info!("Spawning terrain features...");
        TerrainFeature::spawn(&mut terrain_features, &templates, &map);

        // Initialize items, recipes
        recipes.set_templates(templates.recipe_templates.clone());

        // Initial trade prices
        let mut prices = Prices(HashMap::new());
        prices.load_from_template(templates::PriceTemplates(templates.price_templates.clone()));

        // Initial encounter probability
        let encounter_probability = EncounterProbability(HashMap::new());

        let player_stats = PlayerStats(HashMap::new());
        let crisis_state = CrisisState(HashMap::new());
        let sanctuary_excursions = SanctuaryExcursions(HashMap::new());
        let sanctuary_zones = SanctuaryZones(HashMap::new());
        let spawn_positions = SpawnPositions(HashMap::new());
        let player_intro_state = PlayerIntroState(HashMap::new());
        let initial_encounter_state = InitialEncounterState(HashMap::new());
        let objectives = Objectives(HashMap::new());
        let run_score_state = RunScoreState(HashMap::new());
        let legendary_threat_state = LegendaryThreatState(HashMap::new());
        let monolith_investigation = MonolithInvestigation(HashMap::new());
        let victory_state = VictoryState(HashMap::new());

        let debug_objs = DebugObjs(HashSet::new());
        let log_overrides = LogLevelOverrides::default();

        commands.insert_resource(ids);
        commands.insert_resource(entity_obj_map);
        commands.insert_resource(game_tick);
        commands.insert_resource(map_events);
        commands.insert_resource(processed_map_events);
        commands.insert_resource(game_events);
        commands.insert_resource(perception_updates);
        commands.insert_resource(explored_map);
        commands.insert_resource(survey_history);
        commands.insert_resource(investigated_pois);
        commands.insert_resource(prices);
        commands.insert_resource(encounter_probability);
        commands.insert_resource(player_stats);
        commands.insert_resource(crisis_state);
        commands.insert_resource(sanctuary_excursions);
        commands.insert_resource(sanctuary_zones);
        commands.insert_resource(spawn_positions);
        commands.insert_resource(player_intro_state);
        commands.insert_resource(initial_encounter_state);
        commands.insert_resource(objectives);
        commands.insert_resource(run_score_state);
        commands.insert_resource(legendary_threat_state);
        commands.insert_resource(monolith_investigation);
        commands.insert_resource(victory_state);
        commands.insert_resource(debug_objs);
        commands.insert_resource(log_overrides);
    }

    // Network initialization for the production path: create the crossbeam client
    // channel + database channel, spawn the tokio/WebSocket/Postgres runtime via
    // the IO task pool, and insert the three network resources. NOT used by the
    // headless harness.
    pub fn network_init(commands: &mut Commands) {
        // Initialize database manager arc mutex sender
        let database_managers = DatabaseManagers(Arc::new(Mutex::new(HashMap::new())));

        //Create the database to game channel, note the sender will be cloned by each connected client
        let (database_to_game_sender, _database_to_game_receiver) = unbounded::<DatabaseEvent>();

        //Initialize Arc Mutex Hashmap to store the client to game channel per connected client
        let clients = Clients(Arc::new(Mutex::new(HashMap::new())));

        //Create the client to game channel, note the sender will be cloned by each connected client
        let (client_to_game_sender, client_to_game_receiver) = unbounded::<PlayerEvent>();

        let thread_pool = IoTaskPool::get();

        //Spawn the tokio runtime setup using a Compat with the clients and client to game channel
        thread_pool
            .spawn(Compat::new(network::tokio_setup(
                database_to_game_sender,
                database_managers.clone(),
                client_to_game_sender,
                clients.clone(),
                true,
            )))
            .detach();

        let network_receiver = NetworkReceiver(client_to_game_receiver);

        commands.insert_resource(database_managers);
        commands.insert_resource(clients);
        commands.insert_resource(network_receiver);
    }

    pub fn reload_game(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        mut next_state: ResMut<NextState<AppState>>,
    ) {
        println!("Reloading Game");
        let handle = asset_server.load::<DynamicScene>("dynamic_scene.ron");

        // Initialize database manager arc mutex sender
        let database_managers = DatabaseManagers(Arc::new(Mutex::new(HashMap::new())));

        //Create the database to game channel, note the sender will be cloned by each connected client
        let (database_to_game_sender, _database_to_game_receiver) = unbounded::<DatabaseEvent>();

        //Initialize Arc Mutex Hashmap to store the client to game channel per connected client
        let clients = Clients(Arc::new(Mutex::new(HashMap::new())));

        //Create the client to game channel, note the sender will be cloned by each connected client
        let (client_to_game_sender, client_to_game_receiver) = unbounded::<PlayerEvent>();

        let thread_pool = IoTaskPool::get();

        //Spawn the tokio runtime setup using a Compat with the clients and client to game channel
        println!("Spawning tokio runtime...");
        thread_pool
            .spawn(Compat::new(network::tokio_setup(
                database_to_game_sender,
                database_managers.clone(),
                client_to_game_sender,
                clients.clone(),
                false,
            )))
            .detach();

        let network_receiver = NetworkReceiver(client_to_game_receiver);

        let processed_map_events: VisibleEvents = VisibleEvents(Vec::new());
        let perception_updates: PerceptionUpdates = PerceptionUpdates(HashSet::new());

        println!("Initializing entity map...");
        // Initialize Entity to Obj Id map
        let entity_obj_map: EntityObjMap = EntityObjMap(HashMap::new());

        println!("Inserting resources...");
        let debug_objs = DebugObjs(HashSet::new());
        let log_overrides = LogLevelOverrides::default();

        commands.insert_resource(DynamicSceneHandle(handle));
        commands.insert_resource(database_managers);
        commands.insert_resource(entity_obj_map);
        commands.insert_resource(clients);
        commands.insert_resource(network_receiver);
        commands.insert_resource(processed_map_events);
        commands.insert_resource(perception_updates);
        commands.insert_resource(RunScoreState(HashMap::new()));
        commands.insert_resource(LegendaryThreatState(HashMap::new()));
        commands.insert_resource(debug_objs);
        commands.insert_resource(log_overrides);
        println!("Inserting resources complete...");

        // Initialize game world
        //TerrainFeature::spawn(&mut terrain_features, &templates, &map);

        // Initialize items, recipes
        //items.set_templates(templates.item_templates.clone());
        //recipes.set_templates(templates.recipe_templates.clone());
        next_state.set(AppState::Loading);
    }

    pub fn loading_complete(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        scene_handle: Res<DynamicSceneHandle>,
        mut next_state: ResMut<NextState<AppState>>,
    ) {
        info!("Loading complete...");
        let Some(state) = asset_server.get_load_state(&scene_handle.0) else {
            error!("Scene not found!");
            return;
        };

        match state {
            LoadState::Loaded => {
                info!("Scene finished loading!");
                commands.spawn(DynamicSceneRoot(scene_handle.0.clone()));
                next_state.set(AppState::PreRunning);
            }
            LoadState::Loading => {
                info!("Scene still loading...");
            }
            LoadState::NotLoaded => {
                error!("Scene not loaded!");
            }
            LoadState::Failed(_) => {
                error!("Scene failed to load!");
            }
        }
    }

    pub fn load_entity_map(
        mut entity_map: ResMut<EntityObjMap>,
        query: Query<(Entity, &Id)>,
        mut next_state: ResMut<NextState<AppState>>,
    ) {
        println!("Loading entity map...");
        for (entity, id) in query.iter() {
            println!("EntityId: {:?} Id: {:?}", entity, id);
            entity_map.insert(id.0, entity);
        }

        println!("Setting AppState to Running...");
        next_state.set(AppState::Running);
    }

    /*pub fn reload_complete(
        mut next_state: ResMut<NextState<AppState>>
    ) {
        next_state.set(AppState::Running);
    }*/

    // A run condition for all assets being loaded.
    /*fn check_resources_ready(events: Option<Res<MapEvents>>) -> bool {
        // If our barrier isn't ready, return early and wait another cycle
        println!("Checking resources ready...");
        for event in events.iter() {
            println!("MapEvents ready!");
            return true;
        }

        return false;
    }*/

    /*fn check_resources_ready(
        //mut items: ResMut<Items>,
        map_events: Res<MapEvents>,
        mut next_state: ResMut<NextState<AppState>>
    ) {
        println!("Checking resources added...");
        next_state.set(AppState::Running);

        for map_event in map_events.iter() {
            println!("Map Event: {:?}", map_event);
        };

        //println!("Map Event: {:?}", map_event);
    }*/
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct ObjInit {
    pub name: String,
    pub pos: Vec<i32>,
    pub obj_type: String,
}

#[derive(Debug)]
pub struct ObjInitList(pub Vec<ObjInit>);

fn init_objs(
    mut commands: Commands,
    templates: Res<Templates>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    game_tick: Res<GameTick>,
) {
    let obj_init_file = fs::File::open("templates/obj_init.yaml").expect("Could not open file.");
    let obj_init_list =
        ObjInitList(serde_yaml::from_reader(obj_init_file).expect("Could not read values."));

    for obj_init in obj_init_list.0.iter() {
        let obj_id = ids.new_obj_id();

        let mut monolith = Obj::create_nospawn(
            obj_id,
            MONOLITH_PLAYER_ID,
            obj_init.obj_type.clone(),
            Position {
                x: obj_init.pos[0],
                y: obj_init.pos[1],
            },
            State::None,
            Inventory {
                items: Vec::new(),
                owner: obj_id,
            },
            &templates,
        );

        // Create items
        monolith.inventory.new(
            ids.new_item_id(),
            SOULSHARD.to_string(),
            INIT_MONOLITH_SOULSHARDS,
            &templates.item_templates,
        );

        let monolith_attrs = Monolith {
            soulshards: INIT_MONOLITH_SOULSHARDS,
            sanctuary_level: 0,
        };
        // Spawn entity
        let monolith_entity_id = commands.spawn((monolith.clone(), monolith_attrs)).id();

        // Create mappings
        ids.new_obj(monolith.id.0, MONOLITH_PLAYER_ID);
        entity_map.new_obj(monolith.id.0, monolith_entity_id);

        // Create a new object event
        commands.trigger(NewObj {
            entity: monolith_entity_id,
        });
    }
}

fn initial_encounter_object_owner(
    object_id: i32,
    encounters: &InitialEncounterState,
) -> Option<i32> {
    encounters.iter().find_map(|(player_id, entry)| {
        (entry.merchant_id == object_id
            || entry.necromancer_id == object_id
            || entry.mausoleum_id == object_id
            || entry.rat_ids.contains(&object_id)
            || entry.phase1_npc_id == Some(object_id))
        .then_some(*player_id)
    })
}

fn initial_encounter_object_is_protected(
    object_id: i32,
    encounters: &InitialEncounterState,
    presence: &PlayerWorldPresenceState,
) -> bool {
    initial_encounter_object_owner(object_id, encounters)
        .map(|player_id| is_player_offline_protected(player_id, presence))
        .unwrap_or(false)
}

fn attributed_threat_owner(
    legendary_follower: Option<&LegendaryFollower>,
    legendary_boss: Option<&LegendaryBoss>,
    sanctuary_hunter: Option<&SanctuaryHunter>,
) -> Option<i32> {
    legendary_follower
        .map(|follower| follower.player_id)
        .or_else(|| legendary_boss.map(|boss| boss.player_id))
        .or_else(|| sanctuary_hunter.map(|hunter| hunter.player_id))
}

fn game_event_belongs_to_protected_run(
    event_type: &GameEventType,
    ids: &Ids,
    presence: &PlayerWorldPresenceState,
) -> bool {
    let object_is_protected = |obj_id| object_belongs_to_protected_run(obj_id, ids, presence);

    match event_type {
        // Login is part of the reconnect/rebase protocol and must never be
        // parked behind the protection state it is responsible for leaving.
        GameEventType::Login { .. } => false,
        GameEventType::PlayerNotice { player_id, .. }
        | GameEventType::MerchantArrival { player_id, .. }
        | GameEventType::MerchantLeavingSoon { player_id, .. }
        | GameEventType::MerchantDeparture { player_id, .. }
        | GameEventType::SpawnVillager { player_id, .. }
        | GameEventType::AddEffectOnTile { player_id, .. }
        | GameEventType::RemoveEffectOnTile { player_id, .. } => {
            is_player_offline_protected(*player_id, presence)
        }
        GameEventType::ForageEvent { forager_id } => object_is_protected(*forager_id),
        GameEventType::GatherEvent { gatherer_id, .. } => object_is_protected(*gatherer_id),
        GameEventType::StructureGatherEvent {
            operator_id,
            structure_id,
        }
        | GameEventType::StructureOperateEvent {
            operator_id,
            structure_id,
        } => object_is_protected(*operator_id) || object_is_protected(*structure_id),
        GameEventType::RefineEvent { refiner_id, .. } => object_is_protected(*refiner_id),
        GameEventType::CraftEvent { crafter_id, .. } => object_is_protected(*crafter_id),
        GameEventType::StructureRefineEvent {
            refiner_id,
            structure_id,
            ..
        } => object_is_protected(*refiner_id) || object_is_protected(*structure_id),
        GameEventType::StructureCraftEvent {
            crafter_id,
            structure_id,
            ..
        } => object_is_protected(*crafter_id) || object_is_protected(*structure_id),
        GameEventType::ExperimentEvent {
            experimenter_id,
            structure_id,
        } => object_is_protected(*experimenter_id) || object_is_protected(*structure_id),
        GameEventType::UpdatePos { obj_id, .. }
        | GameEventType::DespawnObj { obj_id }
        | GameEventType::CancelRefineEvent { obj_id }
        | GameEventType::CancelAllMapEvents { obj_id }
        | GameEventType::CancelAllowedMapEvents { obj_id } => object_is_protected(*obj_id),
        GameEventType::NecroEvent {
            necromancer_id,
            mausoleum_id,
            ..
        } => {
            necromancer_id.is_some_and(|id| object_is_protected(id))
                || mausoleum_id.is_some_and(|id| object_is_protected(id))
        }
        GameEventType::SpawnNPC { run_owner, .. } => run_owner
            .map(|player_id| is_player_offline_protected(player_id, presence))
            .unwrap_or(false),
        GameEventType::RemoveEntity { .. } | GameEventType::CancelMapEventsById { .. } => false,
    }
}

fn visible_event_references_ended_run(
    event: &VisibleEvent,
    ended_object_ids: &HashSet<i32>,
) -> bool {
    let referenced = match event {
        VisibleEvent::EmbarkEvent { transport_id } => Some(*transport_id),
        VisibleEvent::DamageEvent { target_id, .. }
        | VisibleEvent::StealEvent { target_id, .. }
        | VisibleEvent::BroadcastStealEvent { target_id, .. }
        | VisibleEvent::SpoilEvent { target_id, .. }
        | VisibleEvent::BroadcastSpoilEvent { target_id, .. }
        | VisibleEvent::TorchEvent { target_id, .. }
        | VisibleEvent::BroadcastTorchEvent { target_id, .. }
        | VisibleEvent::InvestigateEvent { target_id }
        | VisibleEvent::SpellDamageEvent { target_id, .. } => Some(*target_id),
        VisibleEvent::ActivateEvent { structure_id }
        | VisibleEvent::OperateEvent { structure_id }
        | VisibleEvent::RefineEvent { structure_id }
        | VisibleEvent::CraftEvent { structure_id, .. }
        | VisibleEvent::ExperimentEvent { structure_id }
        | VisibleEvent::PlantEvent { structure_id }
        | VisibleEvent::TendEvent { structure_id }
        | VisibleEvent::HarvestEvent { structure_id }
        | VisibleEvent::RepairEvent { structure_id } => Some(*structure_id),
        VisibleEvent::UseItemEvent { item_owner_id, .. } => Some(*item_owner_id),
        VisibleEvent::FindDrinkEvent { obj_id }
        | VisibleEvent::DrinkEvent { obj_id, .. }
        | VisibleEvent::FindFoodEvent { obj_id }
        | VisibleEvent::EatEvent { obj_id, .. }
        | VisibleEvent::FindShelterEvent { obj_id }
        | VisibleEvent::SleepEvent { obj_id }
        | VisibleEvent::FishingEvent { obj_id } => Some(*obj_id),
        VisibleEvent::SpellRaiseDeadEvent { corpse_id } => Some(*corpse_id),
        _ => None,
    };
    referenced
        .map(|object_id| ended_object_ids.contains(&object_id))
        .unwrap_or(false)
}

fn game_event_belongs_to_ended_run(
    event_type: &GameEventType,
    player_id: i32,
    ended_object_ids: &HashSet<i32>,
    entity_map: &EntityObjMap,
    map_events: &MapEvents,
) -> bool {
    let ended = |object_id: i32| ended_object_ids.contains(&object_id);
    match event_type {
        GameEventType::Login {
            player_id: owner, ..
        }
        | GameEventType::PlayerNotice {
            player_id: owner, ..
        }
        | GameEventType::MerchantArrival {
            player_id: owner, ..
        }
        | GameEventType::MerchantLeavingSoon {
            player_id: owner, ..
        }
        | GameEventType::MerchantDeparture {
            player_id: owner, ..
        }
        | GameEventType::SpawnVillager {
            player_id: owner, ..
        }
        | GameEventType::AddEffectOnTile {
            player_id: owner, ..
        }
        | GameEventType::RemoveEffectOnTile {
            player_id: owner, ..
        } => *owner == player_id,
        GameEventType::SpawnNPC { run_owner, .. } => *run_owner == Some(player_id),
        GameEventType::ForageEvent { forager_id } => ended(*forager_id),
        GameEventType::GatherEvent { gatherer_id, .. } => ended(*gatherer_id),
        GameEventType::StructureGatherEvent {
            operator_id,
            structure_id,
        }
        | GameEventType::StructureOperateEvent {
            operator_id,
            structure_id,
        } => ended(*operator_id) || ended(*structure_id),
        GameEventType::RefineEvent { refiner_id, .. } => ended(*refiner_id),
        GameEventType::CraftEvent { crafter_id, .. } => ended(*crafter_id),
        GameEventType::StructureRefineEvent {
            refiner_id,
            structure_id,
            ..
        } => ended(*refiner_id) || ended(*structure_id),
        GameEventType::StructureCraftEvent {
            crafter_id,
            structure_id,
            ..
        } => ended(*crafter_id) || ended(*structure_id),
        GameEventType::ExperimentEvent {
            experimenter_id,
            structure_id,
        } => ended(*experimenter_id) || ended(*structure_id),
        GameEventType::UpdatePos { obj_id, .. }
        | GameEventType::DespawnObj { obj_id }
        | GameEventType::CancelRefineEvent { obj_id }
        | GameEventType::CancelAllMapEvents { obj_id }
        | GameEventType::CancelAllowedMapEvents { obj_id } => ended(*obj_id),
        GameEventType::NecroEvent {
            necromancer_id,
            mausoleum_id,
            ..
        } => necromancer_id.map(ended).unwrap_or(false) || mausoleum_id.map(ended).unwrap_or(false),
        GameEventType::RemoveEntity { entity } => entity_map
            .get_obj_by_entity(*entity)
            .map(ended)
            .unwrap_or(false),
        GameEventType::CancelMapEventsById { event_ids } => event_ids.iter().any(|event_id| {
            map_events
                .get(event_id)
                .map(|event| {
                    ended(event.obj_id)
                        || visible_event_references_ended_run(&event.event_type, ended_object_ids)
                })
                .unwrap_or(false)
        }),
    }
}

fn move_event_actor_is_dead(state: State, has_state_dead: bool) -> bool {
    state == State::Dead || has_state_dead
}

fn move_event_system(
    mut commands: Commands,
    entity_map: ResMut<EntityObjMap>,
    ids: ResMut<Ids>,
    mut map_events: ResMut<MapEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    game_tick: Res<GameTick>,
    presence: Res<PlayerWorldPresenceState>,
    initial_encounter_state: Res<InitialEncounterState>,
    threat_attribution_query: Query<(
        Option<&LegendaryFollower>,
        Option<&LegendaryBoss>,
        Option<&SanctuaryHunter>,
        Option<&CrisisAssaultUnit>,
        Option<&TaxCollector>,
    )>,
    mut set: ParamSet<(
        Query<(&mut Position, &mut State, Option<&StateDead>)>, // for the chosen entity (mutable)
        Query<(&PlayerId, &Id, &Position, &Class, &Subclass, &State)>, // for all entities (read-only)
    )>,
    mut event_executing_query: Query<&mut EventExecuting>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence)
                || initial_encounter_object_is_protected(
                    map_event.obj_id,
                    &initial_encounter_state,
                    &presence,
                )
            {
                continue;
            }
            if let Some(entity) = entity_map.get_entity(map_event.obj_id) {
                if let Ok((follower, boss, hunter, assault, collector)) =
                    threat_attribution_query.get(entity)
                {
                    let attributed_owner_is_protected = assault.is_none()
                        && attributed_threat_owner(follower, boss, hunter)
                            .map(|player_id| is_player_offline_protected(player_id, &presence))
                            .unwrap_or(false);
                    let collector_target_is_protected = collector
                        .map(|collector| {
                            is_player_offline_protected(collector.target_player, &presence)
                        })
                        .unwrap_or(false);
                    if attributed_owner_is_protected || collector_target_is_protected {
                        continue;
                    }
                }
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::MoveEvent { src: _, dst } => {
                    info!("Processing MoveEvent: {:?}", map_event);
                    events_to_remove.push(*map_event_id);

                    let Some(mover_entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find entity from id: {:?}", map_event.obj_id);
                        continue;
                    };

                    let Some(mover_player_id) = ids.get_player(map_event.obj_id) else {
                        error!("Cannot find player from id: {:?}", map_event.obj_id);
                        continue;
                    };

                    let Ok(mut event_executing) = event_executing_query.get_mut(mover_entity)
                    else {
                        error!(
                            "Missing EventExecuting component for mover entity {:?} (obj_id {})",
                            mover_entity, map_event.obj_id
                        );
                        continue;
                    };

                    let mut is_dst_open = true;
                    let mut objs_on_tile = Vec::new();

                    for (player_id, id, pos, class, subclass, state) in set.p1().iter() {
                        // Skip the mover object
                        if map_event.obj_id == id.0 {
                            continue;
                        }

                        if (mover_player_id != player_id.0)
                            && *dst == *pos
                            && !class.is_poi()
                            && state.is_blocking()
                        {
                            info!(
                                "Dst is not open id: {:?} player: {:?} pos: {:?}",
                                id.0, player_id.0, *pos
                            );
                            is_dst_open = false;
                            break;
                        }

                        if pos == dst && state.is_active() {
                            objs_on_tile.push((player_id.0, id.0, *subclass));
                        }
                    }

                    let mut mover_query = set.p0();
                    let Ok((mut mover_pos, mut mover_state, mover_dead)) =
                        mover_query.get_mut(mover_entity)
                    else {
                        // If reached, there is a legitimate error case
                        error!(
                            "Cannot find mutable position and state from entity: {:?}",
                            mover_entity
                        );
                        event_executing.state = EventExecutingState::Failed;
                        continue;
                    };

                    // A move can already be queued when ranged or spell damage
                    // kills the mover. Consuming that stale event must not move the
                    // corpse or reset its authoritative dead state.
                    if move_event_actor_is_dead(*mover_state, mover_dead.is_some()) {
                        event_executing.state = EventExecutingState::Failed;
                        continue;
                    }

                    // Check if mover position is adjacent to dst
                    if !Map::is_adjacent_excluding_source(*mover_pos, *dst) {
                        warn!(
                            "Mover position is not adjacent to dst: {:?} {:?}",
                            *mover_pos, *dst
                        );

                        // Reset state only if not aboard
                        if *mover_state != State::Aboard {
                            *mover_state = State::None;
                        }

                        event_executing.state = EventExecutingState::Failed;
                        continue;
                    }

                    // If destination is not open, reset state only if not aboard
                    if !is_dst_open {
                        warn!(
                            "Destination is no longer open: {:?} {:?}",
                            map_event.obj_id, *dst
                        );

                        // Reset state only if not aboard
                        if *mover_state != State::Aboard {
                            *mover_state = State::None;
                        }

                        event_executing.state = EventExecutingState::Failed;
                        continue;
                    }

                    // Reset state and move object
                    *mover_state = State::None;
                    *mover_pos = dst.clone();

                    commands.entity(mover_entity).insert(MoveEventCompleted);

                    event_executing.state = EventExecutingState::Completed;

                    visible_events.push(map_event.clone());
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn sanctuary_rank_points(template_name: &str) -> i32 {
    if template_name.starts_with("Legendary ") {
        260
    } else if template_name.starts_with("Great ") {
        150
    } else if template_name.starts_with("Skilled ") {
        70
    } else {
        0
    }
}

fn sanctuary_power_score(
    template: &Template,
    skills: &Skills,
    inventory: &Inventory,
    player_gold: i32,
) -> i32 {
    let rank_points = sanctuary_rank_points(&template.0);
    if rank_points == 0 {
        return 0;
    }

    let equipped_points = (inventory.get_items_value_by_attr(&AttrKey::Damage, true) * 6.0
        + inventory.get_items_value_by_attr(&AttrKey::Defense, true) * 18.0
        + inventory.get_items_value_by_attr(&AttrKey::AttackRange, true) * 6.0)
        .floor() as i32;
    let skill_points = skills.get_levels().values().sum::<i32>() * 5;
    let wealth_points = player_gold / 2;

    rank_points
        + equipped_points.clamp(0, 90)
        + skill_points.clamp(0, 90)
        + wealth_points.clamp(0, 70)
}

fn sanctuary_exploration_unlocked(power_score: i32) -> bool {
    power_score >= SANCTUARY_POWER_UNLOCK_SCORE
}

fn record_sanctuary_exposure(
    sanctuary_excursions: &mut SanctuaryExcursions,
    player_id: i32,
    sanctuary_protected: bool,
    exploration_unlocked: bool,
) -> Option<i32> {
    if sanctuary_protected || exploration_unlocked {
        sanctuary_excursions.remove(&player_id);
        return None;
    }

    let entry = sanctuary_excursions
        .entry(player_id)
        .or_insert_with(SanctuaryExcursionEntry::default);
    entry.exposure_moves += 1;
    Some(entry.exposure_moves)
}

fn should_spawn_sanctuary_hunters(exposure_moves: i32) -> bool {
    exposure_moves >= 1
}

fn sanctuary_hunter_template_for_slot(
    slot_index: usize,
    exposure_moves: i32,
    power_score: i32,
) -> &'static str {
    if power_score >= 150 {
        if slot_index % 2 == 0 {
            "Wolf Rider"
        } else {
            "Goblin Pillager"
        }
    } else if power_score >= 90 || exposure_moves >= 5 {
        if slot_index % 2 == 0 {
            "Wolf Rider"
        } else {
            "Goblin Pillager"
        }
    } else if exposure_moves >= 3 {
        if slot_index % 3 == 2 {
            "Spider"
        } else {
            "Wolf"
        }
    } else {
        "Wolf"
    }
}

fn active_sanctuary_hunters(
    player_id: i32,
    hunter_query: &Query<(&SanctuaryHunter, Option<&StateDead>)>,
) -> usize {
    hunter_query
        .iter()
        .filter(|(hunter, dead)| hunter.player_id == player_id && dead.is_none())
        .count()
}

fn total_player_gold(player_id: i32, inventory_query: &Query<(&PlayerId, &Inventory)>) -> i32 {
    inventory_query
        .iter()
        .filter(|(obj_player_id, _)| obj_player_id.0 == player_id)
        .map(|(_, inventory)| inventory.get_total_gold())
        .sum()
}

fn outside_weak_sanctuary_from_monolith_positions(
    pos: Position,
    monolith_positions: &[Position],
) -> bool {
    monolith_positions
        .iter()
        .all(|monolith_pos| Map::dist(pos, *monolith_pos) >= WEAK_SANCTUARY_RANGE)
}

fn sanctuary_hunter_adjacent_spawn_positions(
    hero_pos: Position,
    all_objs: &Vec<EncounterMapObj>,
    map: &Map,
) -> Vec<Position> {
    let mut candidates = Vec::new();
    let monolith_positions = all_objs
        .iter()
        .filter(|obj| obj.subclass == Subclass::Monolith.to_string())
        .map(|obj| Position { x: obj.x, y: obj.y })
        .collect::<Vec<_>>();

    for (x, y) in Map::ring((hero_pos.x, hero_pos.y), 1) {
        let pos = Position { x, y };
        if !Map::is_valid_pos((x, y))
            || !Map::is_passable(x, y, map)
            || !outside_weak_sanctuary_from_monolith_positions(pos, &monolith_positions)
        {
            continue;
        }

        let blocked = all_objs.iter().any(|obj| {
            let class = obj.class.as_str();
            obj.x == x && obj.y == y && (class == CLASS_UNIT || class == CLASS_STRUCTURE)
        });

        if !blocked {
            candidates.push(pos);
        }
    }

    candidates
}

fn spawn_sanctuary_hunter_wave(
    commands: &mut Commands,
    ids: &mut ResMut<Ids>,
    entity_map: &mut ResMut<EntityObjMap>,
    templates: &Res<Templates>,
    map: &Map,
    player_id: i32,
    hero_pos: Position,
    all_objs: &Vec<EncounterMapObj>,
    exposure_moves: i32,
    power_score: i32,
    active_hunters: usize,
) -> usize {
    let slots = SANCTUARY_HUNTER_CAP.saturating_sub(active_hunters);
    if slots == 0 {
        return 0;
    }

    let spawn_positions = sanctuary_hunter_adjacent_spawn_positions(hero_pos, all_objs, map);
    let mut spawned = 0;

    for (slot_index, spawn_pos) in spawn_positions.into_iter().take(slots).enumerate() {
        let npc_type = sanctuary_hunter_template_for_slot(slot_index, exposure_moves, power_score);
        let (entity, _, _, _) = Encounter::spawn_npc(
            NPC_PLAYER_ID,
            spawn_pos,
            npc_type.to_string(),
            commands,
            ids,
            entity_map,
            templates,
        );
        commands
            .entity(entity)
            .insert(SanctuaryHunter { player_id });
        commands.trigger(NewObj { entity });
        spawned += 1;
    }

    spawned
}

fn move_event_completed_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    mut entity_map: ResMut<EntityObjMap>,
    mut explored_map: ResMut<ExploredMap>,
    mut ids: ResMut<Ids>,
    mut map: ResMut<Map>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    player_intro_state: Res<PlayerIntroState>,
    initial_encounter_state: Res<InitialEncounterState>,
    mut sanctuary_excursions: ResMut<SanctuaryExcursions>,
    sanctuary_zones: Res<SanctuaryZones>,
    presence: Res<PlayerWorldPresenceState>,
    (
        mover_query,
        map_obj_query,
        mut effect_query,
        sanctuary_query,
        weak_sanctuary_query,
        mut transport_query,
        aboard_query,
        mut encounter_moves_query,
        hunter_query,
        hero_power_query,
        inventory_query,
    ): (
        Query<
            (
                Entity,
                &Id,
                &PlayerId,
                &Position,
                &Subclass,
                &Viewshed,
                Option<&LegendaryFollower>,
                Option<&LegendaryBoss>,
                Option<&SanctuaryHunter>,
                Option<&CrisisAssaultUnit>,
            ),
            With<MoveEventCompleted>,
        >,
        Query<MapObjQuery>,
        Query<&mut Effects>,
        Query<&Sanctuary>,
        Query<&WeakSanctuary>,
        Query<&mut Transport>,
        Query<&StateAboard>,
        Query<&mut EncounterMoves>,
        Query<(&SanctuaryHunter, Option<&StateDead>)>,
        Query<(&Template, &Skills, &Inventory), With<SubclassHero>>,
        Query<(&PlayerId, &Inventory)>,
    ),
) {
    for (
        mover_entity,
        mover_id,
        mover_player_id,
        mover_pos,
        mover_subclass,
        mover_viewshed,
        legendary_follower,
        legendary_boss,
        sanctuary_hunter,
        crisis_assault,
    ) in mover_query.iter()
    {
        if object_belongs_to_protected_run(mover_id.0, &ids, &presence)
            || initial_encounter_object_is_protected(
                mover_id.0,
                &initial_encounter_state,
                &presence,
            )
            || (crisis_assault.is_none()
                && attributed_threat_owner(legendary_follower, legendary_boss, sanctuary_hunter)
                    .map(|player_id| is_player_offline_protected(player_id, &presence))
                    .unwrap_or(false))
        {
            continue;
        }
        info!("MoveEventCompletedSystem - {:?}", mover_pos);
        commands.entity(mover_entity).remove::<MoveEventCompleted>();

        let mut all_objs = Vec::new();
        let mut objs_on_tile = Vec::new();
        let mut in_range_sanctuary = None;
        let mut in_range_weak_sanctuary = None;
        let mut is_dst_shelter = None;

        // Compile lists of objects for collision detection and effect detection
        for obj in map_obj_query.iter() {
            all_objs.push(EncounterMapObj {
                player_id: obj.player_id.0,
                x: obj.pos.x,
                y: obj.pos.y,
                name: obj.name.0.clone(),
                class: obj.class.0.clone(),
                subclass: obj.subclass.to_string(),
                template: obj.template.0.clone(),
            });

            if *obj.pos == *mover_pos && obj.state.is_active() {
                objs_on_tile.push((obj.player_id.0, obj.id.0, *obj.subclass));
            }

            if *obj.subclass == Subclass::Monolith {
                // Suppression radius scales with the Monolith's sanctuary level
                // (upgraded with Soulshards); fall back to the innate range if the
                // zone hasn't been synced yet this frame.
                let (full_r, weak_r) = sanctuary_zones
                    .0
                    .get(&obj.id.0)
                    .map(|z| (z.full_radius(), z.weak_radius()))
                    .unwrap_or((SANCTUARY_RANGE, WEAK_SANCTUARY_RANGE));
                let dist = Map::dist(*mover_pos, *obj.pos);
                if dist < full_r {
                    in_range_sanctuary = Some((obj.id.0, obj.pos.clone()));
                } else if dist < weak_r {
                    in_range_weak_sanctuary = Some((obj.id.0, obj.pos.clone()));
                }
            } else if *obj.subclass == Subclass::Shelter {
                if Map::dist(*mover_pos, *obj.pos) < 1 {
                    is_dst_shelter = Some(obj.id.0);
                }
            }
        }

        // Check if player spawns an encounter (not near monolith)
        if player::is_player(mover_player_id.0) && in_range_sanctuary.is_none() {
            // Grace period: no random encounters in the first 6 minutes (3600 ticks)
            let in_grace_period =
                intro_is_younger_than(&game_tick, mover_player_id.0, &player_intro_state, 3600);

            let mut encounter_moves = encounter_moves_query
                .get_mut(mover_entity)
                .expect("Encounter moves not found");
            encounter_moves.0 = encounter_moves.0 + 1;
            let wildness = if in_grace_period {
                0
            } else {
                map.get_wildness(mover_pos.x, mover_pos.y)
            };

            info!(
                "Encounter moves: {:?}, wildness: {:?}",
                encounter_moves.0, wildness
            );
            let encounter_probability = Encounter::probability(encounter_moves.0, wildness);

            // Roll for encounter
            let roll = rand::thread_rng().gen_range(0.0..1.0);
            //let roll = 99.0;
            info!("Encounter roll: {:?}", roll);

            if roll < encounter_probability {
                info!("Spawning encounter at {:?}", mover_pos);
                // Reset encounter moves
                encounter_moves.0 = 0;

                let encounter_pos = Encounter::get_encounter_pos(
                    NPC_PLAYER_ID,
                    mover_pos.x,
                    mover_pos.y,
                    all_objs.clone(),
                    &map,
                );

                if let Some(encounter_pos) = encounter_pos {
                    // Reduce wildness at mover pos
                    map.update_wildness(mover_pos.x, mover_pos.y, -1);

                    let npc_type = "Wolf".to_string();

                    debug!("Spawning a NPC of type: {:?}", npc_type);

                    let wolf_id = ids.new_obj_id();

                    let event_type = GameEventType::SpawnNPC {
                        npc_type: npc_type,
                        pos: encounter_pos,
                        npc_id: Some(wolf_id),
                        run_owner: mover_player_id.is_human().then_some(mover_player_id.0),
                    };

                    let event_id = ids.new_map_event_id();

                    let event = GameEvent {
                        event_id: event_id,
                        start_tick: game_tick.0,
                        run_tick: game_tick.0 + 4, // Add one game tick
                        event_type,
                    };

                    game_events.insert(event.event_id, event);

                    let sound_event = VisibleEvent::SoundEvent {
                        pos: encounter_pos,
                        sound: templates.get_dialogue("Wolf"),
                        intensity: 10,
                    };

                    map_events.new(wolf_id, game_tick.0 + 15, sound_event);
                }
            }
        }

        // If object has Fortified effect and is leaving a wall tile
        if let Ok(mut effects) = effect_query.get_mut(mover_entity) {
            if effects.has(Effect::Fortified) {
                if !objs_on_tile
                    .iter()
                    .any(|(_, _, subclass)| *subclass == Subclass::Wall)
                {
                    trace!("Removing Fortified effect {:?}", effects);
                    effects.0.remove(&Effect::Fortified);

                    commands.entity(mover_entity).remove::<Fortified>();
                }
            } else if effects.has(Effect::WatchtowerLight) {
                if !objs_on_tile
                    .iter()
                    .any(|(_, _, subclass)| *subclass == Subclass::Watchtower)
                {
                    trace!("Removing Watchtower Light effect {:?}", effects);
                    effects.0.remove(&Effect::WatchtowerLight);

                    //Add obj update event
                    commands.trigger(UpdateObj {
                        entity: mover_entity,
                        attrs: vec![(VISION.to_string(), "Pending".to_string())],
                    });
                }
            }
        }

        // Check if moving object is enter a tile with a wall
        for (player_id, obj_id, subclass) in objs_on_tile.iter() {
            if mover_player_id.0 != *player_id {
                continue;
            }

            trace!("Checking if mover is entering a wall tile: {:?}", subclass);
            if *subclass == Subclass::Wall {
                if let Ok(mut effects) = effect_query.get_mut(mover_entity) {
                    effects
                        .0
                        .insert(Effect::Fortified, (game_tick.0 + 1, 0.0, 1));
                    trace!("Effects on {:?}", effects);

                    commands
                        .entity(mover_entity)
                        .insert(Fortified { id: *obj_id });
                }
            } else if *subclass == Subclass::Watchtower {
                if let Ok(mut effects) = effect_query.get_mut(mover_entity) {
                    effects
                        .0
                        .insert(Effect::WatchtowerLight, (game_tick.0 + 1, 0.0, 1));

                    //Add obj update event
                    commands.trigger(UpdateObj {
                        entity: mover_entity,
                        attrs: vec![(VISION.to_string(), "Pending".to_string())],
                    });
                }
            }
        }

        // Check if player is entering/leaving sanctuary or weak sanctuary
        if player::is_player(mover_player_id.0) {
            let Ok(mut effects) = effect_query.get_mut(mover_entity) else {
                error!("No effects found for player obj {:?}", mover_entity);
                continue;
            };

            if let Some((monolith_id, monolith_pos)) = in_range_sanctuary {
                // In-zone defensive bonus scales with how far the sanctuary is upgraded.
                let sanctuary_amp = 1.0
                    + sanctuary_zones
                        .0
                        .get(&monolith_id)
                        .map(|z| z.level)
                        .unwrap_or(0) as f32
                        * SANCTUARY_DEFENSE_PER_LEVEL;
                // Check if coming from weak sanctuary
                if effects.has(Effect::WeakSanctuary) {
                    // Add weak sanctuary
                    effects
                        .0
                        .insert(Effect::Sanctuary, (game_tick.0 + 1, sanctuary_amp, 1));

                    commands.entity(mover_entity).insert(Sanctuary {
                        id: monolith_id,
                        pos: monolith_pos,
                    });

                    // Remove sanctuary
                    effects.0.remove(&Effect::WeakSanctuary);
                    commands.entity(mover_entity).remove::<WeakSanctuary>();

                    // Skip sending for villagers
                    if !mover_subclass.is_villager() {
                        let response_packet = ResponsePacket::IncreasedEffect {
                            id: mover_id.0,
                            x: mover_pos.x,
                            y: mover_pos.y,
                            label: "Elevated".to_owned(),
                            effect: Effect::Sanctuary.to_str(),
                        };

                        send_to_client(mover_player_id.0, response_packet, &clients);
                    }
                } else if !effects.has(Effect::Sanctuary) {
                    effects
                        .0
                        .insert(Effect::Sanctuary, (game_tick.0 + 1, sanctuary_amp, 1));

                    commands.entity(mover_entity).insert(Sanctuary {
                        id: monolith_id,
                        pos: monolith_pos,
                    });

                    // Skip sending for villagers
                    if !mover_subclass.is_villager() {
                        let response_packet = ResponsePacket::GainedEffect {
                            id: mover_id.0,
                            x: mover_pos.x,
                            y: mover_pos.y,
                            effect: Effect::Sanctuary.to_str(),
                        };

                        send_to_client(mover_player_id.0, response_packet, &clients);
                    }
                }
            } else if effects.has(Effect::Sanctuary) {
                let Ok(monolith) = sanctuary_query.get(mover_entity) else {
                    error!(
                        "Sanctuary effect and component out of sync for {:?}",
                        mover_entity
                    );
                    continue;
                };

                let distance = Map::dist(*mover_pos, monolith.pos);

                if distance >= SANCTUARY_RANGE && distance < WEAK_SANCTUARY_RANGE {
                    // Add weak sanctuary
                    effects
                        .0
                        .insert(Effect::WeakSanctuary, (game_tick.0 + 1, 1.0, 1));

                    commands.entity(mover_entity).insert(WeakSanctuary {
                        id: monolith.id,
                        pos: monolith.pos,
                    });

                    // Remove sanctuary
                    effects.0.remove(&Effect::Sanctuary);
                    commands.entity(mover_entity).remove::<Sanctuary>();

                    // Skip sending for villagers
                    if !mover_subclass.is_villager() {
                        let response_packet = ResponsePacket::ReducedEffect {
                            id: mover_id.0,
                            x: mover_pos.x,
                            y: mover_pos.y,
                            label: "Diminished".to_owned(),
                            effect: Effect::Sanctuary.to_str(),
                        };

                        send_to_client(mover_player_id.0, response_packet, &clients);
                    }
                }
            } else if let Some((monolith_id, monolith_pos)) = in_range_weak_sanctuary {
                if !effects.has(Effect::WeakSanctuary) {
                    effects
                        .0
                        .insert(Effect::WeakSanctuary, (game_tick.0 + 1, 1.0, 1));

                    commands.entity(mover_entity).insert(WeakSanctuary {
                        id: monolith_id,
                        pos: monolith_pos,
                    });

                    // Skip sending for villagers
                    if !mover_subclass.is_villager() {
                        let response_packet = ResponsePacket::GainedEffect {
                            id: mover_id.0,
                            x: mover_pos.x,
                            y: mover_pos.y,
                            effect: Effect::WeakSanctuary.to_str(),
                        };

                        send_to_client(mover_player_id.0, response_packet, &clients);
                    }
                }
            } else if effects.has(Effect::WeakSanctuary) {
                let Ok(weak_sanctuary) = weak_sanctuary_query.get(mover_entity) else {
                    error!(
                        "Weak sanctuary effect and component out of sync for {:?}",
                        mover_entity
                    );
                    continue;
                };

                let distance = Map::dist(*mover_pos, weak_sanctuary.pos);

                if distance >= WEAK_SANCTUARY_RANGE {
                    effects.0.remove(&Effect::WeakSanctuary);

                    commands.entity(mover_entity).remove::<WeakSanctuary>();

                    // Skip sending for villagers
                    if !mover_subclass.is_villager() {
                        let response_packet = ResponsePacket::LostEffect {
                            id: mover_id.0,
                            x: mover_pos.x,
                            y: mover_pos.y,
                            effect: Effect::WeakSanctuary.to_str(),
                        };

                        send_to_client(mover_player_id.0, response_packet, &clients);
                    }
                }
            }

            let sanctuary_protected =
                effects.has(Effect::Sanctuary) || effects.has(Effect::WeakSanctuary);
            drop(effects);

            if mover_subclass.is_hero() {
                if let Ok((hero_template, hero_skills, hero_inventory)) =
                    hero_power_query.get(mover_entity)
                {
                    let player_gold = total_player_gold(mover_player_id.0, &inventory_query);
                    let power_score = sanctuary_power_score(
                        hero_template,
                        hero_skills,
                        hero_inventory,
                        player_gold,
                    );
                    let exploration_unlocked = sanctuary_exploration_unlocked(power_score);

                    if let Some(exposure_moves) = record_sanctuary_exposure(
                        &mut sanctuary_excursions,
                        mover_player_id.0,
                        sanctuary_protected,
                        exploration_unlocked,
                    ) {
                        if exposure_moves == 1 {
                            let packet = ResponsePacket::Notice {
                                noticemsg:
                                    "The Monolith's sanctuary fades. The wilds have noticed you."
                                        .to_string(),
                                expiry: Some(8000),
                            };
                            send_to_client(mover_player_id.0, packet, &clients);
                        }

                        if should_spawn_sanctuary_hunters(exposure_moves) {
                            let active_hunters =
                                active_sanctuary_hunters(mover_player_id.0, &hunter_query);
                            let spawned = spawn_sanctuary_hunter_wave(
                                &mut commands,
                                &mut ids,
                                &mut entity_map,
                                &templates,
                                &map,
                                mover_player_id.0,
                                *mover_pos,
                                &all_objs,
                                exposure_moves,
                                power_score,
                                active_hunters,
                            );

                            if spawned > 0 {
                                info!(
                                    "Sanctuary excursion spawned {} hunters for player {} at exposure {}",
                                    spawned, mover_player_id.0, exposure_moves
                                );
                            }
                        }
                    }
                } else {
                    error!(
                        "Hero {:?} missing progression data for sanctuary excursion",
                        mover_entity
                    );
                }
            }

            // Check if moving object is leaving a transport
            if let Ok(aboard) = aboard_query.get(mover_entity) {
                trace!("Mover is leaving a transport: {:?}", mover_id.0);

                // Get transport entity
                let transport_entity = entity_map.get_entity(aboard.transport_id).unwrap();

                // Get transport
                let Ok(mut transport) = transport_query.get_mut(transport_entity) else {
                    error!("Query failed to find transport {:?}", transport_entity);
                    continue;
                };

                // Remove object from transport
                transport.hauling.retain(|&x| x != mover_id.0);

                // Remove StateAboard component from mover
                commands.entity(mover_entity).remove::<StateAboard>();
            }

            // Check if moving object is entering a shelter
            if let Some(shelter_id) = is_dst_shelter {
                commands
                    .entity(mover_entity)
                    .insert(Sheltered { id: shelter_id });
            } else {
                commands.entity(mover_entity).remove::<Sheltered>();
            }

            // Adding new maps to explored map
            // Assume player has some explored map tiles
            let viewshed_tiles_pos = Map::range((mover_pos.x, mover_pos.y), mover_viewshed.range);
            info!("Viewshed Tiles: {:?}", viewshed_tiles_pos);

            let player_explored_map = explored_map.get_mut(&mover_player_id.0).unwrap();

            let mut new_explored_tiles = Vec::new();

            for tile in viewshed_tiles_pos {
                if !player_explored_map.contains(&tile) {
                    new_explored_tiles.push(tile);

                    // Add tile to player explored map
                    player_explored_map.push(tile);
                }
            }

            let mut new_objs = Vec::new();

            // Get new objs in viewshed
            for map_obj in map_obj_query.iter() {
                let distance =
                    Map::distance((mover_pos.x, mover_pos.y), (map_obj.pos.x, map_obj.pos.y));

                // Skip player's own observers
                if mover_player_id.0 == map_obj.player_id.0 && mover_viewshed.range >= distance {
                    continue;
                }

                if mover_viewshed.range >= distance
                    && Obj::state_to_enum(map_obj.state.to_string()).is_visible()
                {
                    let (work_done, total_work, work_per_sec) =
                        network::build_progress_fields(map_obj.build_upgrade_state);

                    // Convert to network::MapObj
                    let network_map_obj = network::MapObj {
                        id: map_obj.id.0,
                        player: map_obj.player_id.0,
                        x: map_obj.pos.x,
                        y: map_obj.pos.y,
                        name: map_obj.name.0.clone(),
                        template: map_obj.template.0.clone(),
                        class: map_obj.class.0.clone(),
                        subclass: map_obj.subclass.to_string(),
                        state: map_obj.state.to_string(),
                        vision: None,
                        image: map_obj.misc.image.clone(),
                        hsl: map_obj.misc.hsl.clone(),
                        groups: map_obj.misc.groups.clone(),
                        work_done,
                        total_work,
                        work_per_sec,
                    };

                    new_objs.push(network_map_obj);
                }
            }

            // Only send new explored tiles
            let tiles_to_send = if !new_explored_tiles.is_empty() {
                Map::pos_to_tiles(&new_explored_tiles, &map)
            } else {
                Vec::new()
            };

            let map_packet = ResponsePacket::NewObjPerception {
                new_objs,
                new_tiles: tiles_to_send,
            };
            send_to_client(mover_player_id.0, map_packet, &clients);
        }
    }
}

fn hide_event_system(
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    mut state_query: Query<&mut State>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::HideEvent => {
                    debug!("Processing HideEvent {:?}", map_event);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find corpse from {:?}", map_event.obj_id);
                        continue;
                    };

                    let Ok(mut state) = state_query.get_mut(entity) else {
                        error!("state_query failed to find entity {:?}", entity);
                        continue;
                    };

                    *state = State::Hiding;

                    visible_events.push(map_event.clone());
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn update_obj_event_system(
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut map_events: ResMut<MapEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut perception_updates: ResMut<PerceptionUpdates>,
    mut query: Query<UpdateObjQuery>,
    mut viewshed_query: Query<&mut Viewshed>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::UpdateObjEvent { attrs } => {
                    debug!("Processing UpdateObjEvent: {:?}", attrs);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!(
                            "Cannot find entity from id: {:?} map_event: {:?}",
                            map_event.obj_id, map_event
                        );
                        continue;
                    };

                    let Ok(mut obj) = query.get_mut(entity) else {
                        error!("Query failed to find entity {:?}", entity);
                        continue;
                    };

                    for (attr, value) in attrs.iter() {
                        match attr.as_str() {
                            PLAYER_ID => {
                                let new_player_id = value.parse::<i32>().unwrap();

                                *obj.player_id = PlayerId(new_player_id);
                                ids.change_obj_player_id(obj.id.0, new_player_id.clone());

                                visible_events.push(map_event.clone());
                            }
                            TEMPLATE => {
                                obj.template.0 = value.to_string();

                                let template = templates.obj_templates.get(value.to_string());

                                if let Some(images) = template.images {
                                    let random_image =
                                        rand::thread_rng().gen_range(0..images.len());
                                    obj.misc.image = images[random_image].clone();
                                } else {
                                    obj.misc.image = Obj::template_to_image(&template.template);
                                }

                                visible_events.push(map_event.clone());
                            }
                            IMAGE => {
                                obj.misc.image = value.to_string();
                                visible_events.push(map_event.clone());
                            }
                            VISION => {
                                let vision_modifier = obj.effects.get_vision_modifier(&templates);

                                let Ok(mut viewshed) = viewshed_query.get_mut(entity) else {
                                    error!("Query failed to find entity {:?}", entity);
                                    continue;
                                };

                                info!(
                                    "Id: {:?} Template: {:?} viewshed: {:?}",
                                    obj.id.0, obj.template.0, viewshed.range
                                );
                                let new_range = Obj::set_viewshed_range(
                                    obj.id.0,
                                    obj.template.0.clone(),
                                    game_tick.0,
                                    &obj.inventory,
                                    &templates,
                                    vision_modifier,
                                );

                                info!(
                                    "Updating viewshed range to: {:?} for id: {:?} template: {:?}",
                                    new_range, obj.id.0, obj.template.0
                                );

                                viewshed.range = new_range;

                                perception_updates.insert((
                                    obj.player_id.0,
                                    PerceptionUpdateType::UpdatePerception,
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn activate_event_system(
    mut commands: Commands,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    game_tick: Res<GameTick>,
    mut map_events: ResMut<MapEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut game_events: ResMut<GameEvents>,
    mut query: Query<ObjWithStatsQuery>,
) {
    let mut events_to_add: Vec<MapEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::ActivateEvent { structure_id } => {
                    if object_belongs_to_protected_run(*structure_id, &ids, &presence) {
                        continue;
                    }
                    debug!(
                        "Processing ActivateEvent: structure_id: {:?} ",
                        structure_id
                    );
                    events_to_remove.push(*map_event_id);

                    let Some(activator_entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find activator from {:?}", map_event.obj_id);
                        continue;
                    };

                    let Ok(mut activator) = query.get_mut(activator_entity) else {
                        error!("Query failed to find entity {:?}", activator_entity);
                        continue;
                    };

                    // Set state to None
                    *activator.state = State::None;

                    commands.trigger(StateChange {
                        entity: activator_entity,
                        new_state: State::None,
                    });

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find structure from {:?}", *structure_id);
                        continue;
                    };

                    let Ok(structure) = query.get_mut(structure_entity) else {
                        error!("Query failed to find entity {:?}", structure_entity);
                        continue;
                    };

                    let structure_template =
                        templates.obj_templates.get(structure.template.0.clone());

                    if structure_template.campfire.unwrap_or(false) {
                        commands.entity(structure_entity).insert(Campfire {
                            is_lit: true,
                            lit_at: game_tick.0 + 1,
                            duration: 1000,
                        });

                        let structure_campfire_image =
                            Obj::template_to_image(&structure_template.template.clone()) + "lit";

                        // Structure State Change Event to Lit
                        commands.trigger(UpdateObj {
                            entity: structure_entity,
                            attrs: vec![(IMAGE.to_string(), structure_campfire_image)],
                        });

                        // Add campfire light effect
                        commands.trigger(AddLightEffect {
                            entity: structure_entity,
                            effect: Effect::CampfireLight.clone(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        map_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

pub fn build_system(
    mut commands: Commands,
    entity_map: Res<EntityObjMap>,
    game_tick: Res<GameTick>,
    templates: Res<Templates>,
    presence: OptionalPlayerWorldPresence,
    mut structure_query: Query<
        (
            Entity,
            &Id,
            &PlayerId,
            &Position,
            &State,
            &Subclass,
            &Template,
            &Assignments,
            &mut Stats,
            &mut BuildUpgradeState,
            &mut WorkQueue,
        ),
        With<StateBuilding>,
    >,
    worker_query: Query<(&Id, &Position, &State, &Template, &Skills, &BaseAttrs)>,
    mut occupant_query: Query<
        (Entity, &PlayerId, &Position, &State, &mut Effects),
        Without<ClassStructure>,
    >,
    crisis_state: Option<Res<SettlementCrisisState>>,
    mut balance_telemetry_state: Option<ResMut<CrisisBalanceTelemetryState>>,
) {
    if game_tick.0 % 10 != 0 {
        return;
    }

    for (
        structure_entity,
        structure_id,
        structure_player_id,
        structure_pos,
        structure_state,
        structure_subclass,
        structure_template,
        structure_assignments,
        mut structure_stats,
        mut build_state,
        mut structure_work_queue,
    ) in structure_query.iter_mut()
    {
        if is_owner_offline_protected(structure_player_id, &presence) {
            continue;
        }
        debug!("Building system processing structure: {:?}", structure_id);
        debug!("Structure position: {:?}", structure_pos);
        debug!("Structure state: {:?}", structure_state);
        debug!("Assignments: {:?}", structure_assignments.0.len());

        let mut total_build_rate = 0.0;
        let mut active_workers = Vec::new();

        for worker_id in structure_assignments.0.iter() {
            let Some(worker_entity) = entity_map.get_entity(*worker_id) else {
                error!("Cannot find worker entity for {:?}", worker_id);
                continue;
            };

            let Ok((
                worker_id,
                worker_pos,
                worker_state,
                worker_template,
                worker_skills,
                worker_attrs,
            )) = worker_query.get(worker_entity)
            else {
                error!("Query failed to find entity {:?}", worker_entity);
                continue;
            };

            // Check if worker is on the same position as the structure
            if worker_pos != structure_pos {
                continue;
            }

            // Check if worker is in building state
            if worker_state != &State::Building {
                continue;
            }

            // Get template from villager
            let worker_template = templates.obj_templates.get(worker_template.0.clone());

            // Get base work from villager template
            let base_work = worker_template.base_work.unwrap_or(5);

            // Get skills from villager
            let carpentry_skill = worker_skills.get_level_by_name(Skill::Carpentry);
            let masonry_skill = worker_skills.get_level_by_name(Skill::Masonry);
            let construction_skill = worker_skills.get_level_by_name(Skill::Construction);

            // Get build rate from villager
            let build_rate = Obj::construction_skill_multiplier(
                base_work,
                construction_skill,
                carpentry_skill,
                masonry_skill,
            );

            total_build_rate += build_rate;

            // Add worker to active workers
            active_workers.push(worker_entity);
        }

        info!("Build state work done: {:?}", build_state.work_done);
        info!(
            "Build state total work: {:?}",
            build_state.build_upgrade_cost
        );
        let prev_build_rate = build_state.work_per_sec;
        build_state.work_done += total_build_rate;
        build_state.work_per_sec = total_build_rate;

        // If the build rate changed (e.g. a worker joined or left), push a
        // fresh progress update so the client re-anchors its progress bar.
        if (total_build_rate - prev_build_rate).abs() > f32::EPSILON {
            commands.trigger(BuildProgressUpdate {
                entity: structure_entity,
            });
        }

        if build_state.work_done >= build_state.build_upgrade_cost {
            build_state.work_done = build_state.build_upgrade_cost;
            build_state.work_per_sec = 0.0;
            build_state.start_time = 0;

            // Change structure state to none
            commands.trigger(StateChange {
                entity: structure_entity,
                new_state: State::None,
            });

            // Remove StateBuilding
            commands.entity(structure_entity).remove::<StateBuilding>();

            // Set structure hp to base hp
            structure_stats.hp = structure_stats.base_hp;

            if matches!(*structure_subclass, Subclass::Wall | Subclass::Watchtower)
                && matches!(
                    crisis_state
                        .as_ref()
                        .and_then(|state| state.get(&structure_player_id.0))
                        .map(|crisis| crisis.phase),
                    Some(CrisisPhase::Preparing | CrisisPhase::AssaultReady)
                )
            {
                if let Some(telemetry_state) = balance_telemetry_state.as_deref_mut() {
                    telemetry_state
                        .entry(structure_player_id.0)
                        .or_default()
                        .preparation_actions
                        .record_defensive_structure_completed(
                            structure_id.0,
                            *structure_subclass == Subclass::Wall,
                            game_tick.0,
                        );
                }
            }

            if *structure_subclass == Subclass::Wall {
                for (entity, player_id, pos, state, mut effects) in occupant_query.iter_mut() {
                    if *player_id == *structure_player_id
                        && *pos == *structure_pos
                        && occupant_receives_wall_fortification(state)
                    {
                        effects
                            .0
                            .insert(Effect::Fortified, (game_tick.0 + 1, 0.0, 1));
                        commands
                            .entity(entity)
                            .insert(Fortified { id: structure_id.0 });
                    }
                }
            }

            // Handle structure subclass specific actions
            match *structure_subclass {
                Subclass::Shelter => {
                    let shelter_template =
                        templates.obj_templates.get(structure_template.0.clone());

                    if let Some(max_residents) = shelter_template.max_residents {
                        commands.entity(structure_entity).insert(Shelter {
                            max_residents: max_residents,
                            residents: Vec::new(),
                        });
                    }
                }
                Subclass::Storage => {
                    commands.entity(structure_entity).insert(Storage);
                }
                Subclass::Watchtower => {
                    // Add watchtower light game event
                    commands.trigger(AddLightEffect {
                        entity: structure_entity,
                        effect: Effect::WatchtowerLight,
                    });

                    commands.entity(structure_entity).insert(Watchtower);
                }
                Subclass::Resource => {
                    // Get structure template
                    let structure_template =
                        templates.obj_templates.get(structure_template.0.clone());

                    // Get workspaces from structure template
                    let workspaces = structure_template.workspaces.unwrap_or(0);

                    for _workspace in 0..workspaces {
                        let work_entry = WorkEntry {
                            worker_id: -1,
                            work_type: WorkType::Operate,
                            work_status: WorkStatus::Idle,
                            recipe_name: None,
                            recipe_image: None,
                            refine_item_id: None,
                            refine_item_image: None,
                            refine_item_class: None,
                        };

                        structure_work_queue.0.push(work_entry);
                    }
                }
                _ => {}
            }

            // Change builders state to none
            for worker_entity in active_workers.iter() {
                info!("Changing builder state to none: {:?}", *worker_entity);
                commands.trigger(StateChange {
                    entity: *worker_entity,
                    new_state: State::None,
                });
            }
        }
    }
}

pub fn upgrade_system(
    mut commands: Commands,
    entity_map: Res<EntityObjMap>,
    game_tick: Res<GameTick>,
    templates: Res<Templates>,
    presence: OptionalPlayerWorldPresence,
    mut structure_query: Query<
        (
            Entity,
            &Id,
            &PlayerId,
            &Position,
            &State,
            &mut Name,
            &mut Class,
            &mut Subclass,
            &mut Template,
            &mut Misc,
            &mut Stats,
            &Assignments,
            &mut BuildUpgradeState,
            &SelectedUpgrade,
        ),
        (With<StateUpgrading>, With<ClassStructure>),
    >,
    worker_query: Query<
        (&Id, &Position, &State, &Template, &Skills, &BaseAttrs),
        Without<ClassStructure>,
    >,
    mut shelters: Query<&mut Shelter>,
) {
    if game_tick.0 % 10 != 0 {
        return;
    }

    for (
        structure_entity,
        structure_id,
        structure_player_id,
        structure_pos,
        structure_state,
        mut structure_name,
        mut structure_class,
        mut structure_subclass,
        mut structure_template,
        mut structure_misc,
        mut structure_stats,
        structure_assignments,
        mut build_state,
        selected_upgrade,
    ) in structure_query.iter_mut()
    {
        if is_owner_offline_protected(structure_player_id, &presence) {
            continue;
        }
        debug!("Upgrading system processing structure: {:?}", structure_id);
        debug!("Structure position: {:?}", structure_pos);
        debug!("Structure state: {:?}", structure_state);
        debug!("Assignments: {:?}", structure_assignments.0.len());

        let mut total_build_rate = 0.0;
        let mut active_workers = Vec::new();

        for worker_id in structure_assignments.0.iter() {
            let Some(worker_entity) = entity_map.get_entity(*worker_id) else {
                error!("Cannot find worker entity for {:?}", worker_id);
                continue;
            };

            let Ok((
                worker_id,
                worker_pos,
                worker_state,
                worker_template,
                worker_skills,
                worker_attrs,
            )) = worker_query.get(worker_entity)
            else {
                error!("Query failed to find entity {:?}", worker_entity);
                continue;
            };

            // Check if worker is on the same position as the structure
            if worker_pos != structure_pos {
                continue;
            }

            // Check if worker is in building state
            if worker_state != &State::Upgrading {
                continue;
            }

            // Get template from villager
            let worker_template = templates.obj_templates.get(worker_template.0.clone());

            // Get base work from villager template
            let base_work = worker_template.base_work.unwrap_or(5);

            // Get skills from villager
            let carpentry_skill = worker_skills.get_level_by_name(Skill::Carpentry);
            let masonry_skill = worker_skills.get_level_by_name(Skill::Masonry);
            let construction_skill = worker_skills.get_level_by_name(Skill::Construction);

            // Get build rate from villager
            let build_rate = Obj::construction_skill_multiplier(
                base_work,
                construction_skill,
                carpentry_skill,
                masonry_skill,
            );

            total_build_rate += build_rate;

            // Add worker to active workers
            active_workers.push(worker_entity);
        }

        info!("Build state work done: {:?}", build_state.work_done);
        info!(
            "Build state total work: {:?}",
            build_state.build_upgrade_cost
        );
        let prev_build_rate = build_state.work_per_sec;
        build_state.work_done += total_build_rate;
        build_state.work_per_sec = total_build_rate;

        // If the upgrade rate changed (e.g. a worker joined or left), push a
        // fresh progress update so the client re-anchors its progress bar.
        if (total_build_rate - prev_build_rate).abs() > f32::EPSILON {
            commands.trigger(BuildProgressUpdate {
                entity: structure_entity,
            });
        }

        if build_state.work_done >= build_state.build_upgrade_cost {
            build_state.work_done = build_state.build_upgrade_cost;
            build_state.work_per_sec = 0.0;
            build_state.start_time = 0;

            let upgrade_template = templates.obj_templates.get(selected_upgrade.0.clone());

            // Upgrade structure attributes
            *structure_name = Name(upgrade_template.template.clone());
            *structure_template = Template(upgrade_template.template);
            *structure_class = Class(upgrade_template.class);
            *structure_subclass = Subclass::from_str(&upgrade_template.subclass);
            structure_misc.image = upgrade_template.image.clone();
            structure_stats.base_hp = upgrade_template.base_hp.unwrap_or(structure_stats.base_hp);
            structure_stats.hp = structure_stats.base_hp;
            structure_stats.base_def = upgrade_template
                .base_def
                .unwrap_or(structure_stats.base_def);

            // Change structure state to none
            commands.trigger(StateChange {
                entity: structure_entity,
                new_state: State::None,
            });

            // Remove StateUpgrading
            commands.entity(structure_entity).remove::<StateUpgrading>();

            // Handle structure subclass specific actions
            match *structure_subclass {
                Subclass::Shelter => {
                    let shelter_template =
                        templates.obj_templates.get(structure_template.0.clone());

                    if let Some(max_residents) = shelter_template.max_residents {
                        if let Ok(mut shelter) = shelters.get_mut(structure_entity) {
                            shelter.max_residents = max_residents;
                        } else {
                            commands.entity(structure_entity).insert(Shelter {
                                max_residents: max_residents,
                                residents: Vec::new(),
                            });
                        }
                    }
                }
                _ => {}
            }

            // Trigger a template change event
            commands.trigger(TemplateChange {
                entity: structure_entity,
                new_template: structure_template.0.clone(),
            });

            // Change builders state to none
            for worker_entity in active_workers.iter() {
                info!("Changing builder state to none: {:?}", *worker_entity);
                commands.trigger(StateChange {
                    entity: *worker_entity,
                    new_state: State::None,
                });
            }
        }
    }
}

fn add_light_effect_system(
    light_effect: On<AddLightEffect>,
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut map_events: ResMut<MapEvents>,
    source_query: Query<(&PlayerId, &Position, &State)>,
    mut query_with_effects: Query<
        (Entity, &PlayerId, &Id, &Position, &State, &mut Effects),
        With<Viewshed>,
    >,
    query_without_effects: Query<
        (Entity, &PlayerId, &Id, &Position, &State),
        (Without<Effects>, With<Viewshed>),
    >,
) {
    // Get source player id and position
    let Ok((source_player_id, source_pos, source_state)) = source_query.get(light_effect.entity)
    else {
        error!("Cannot find source from {:?}", light_effect.entity);
        return;
    };

    // Add light effect to all objects on the same tile that already have Effects
    for (obj_entity, obj_player_id, obj_id, obj_pos, obj_state, mut obj_effects) in
        query_with_effects.iter_mut()
    {
        if obj_pos.x == source_pos.x
            && obj_pos.y == source_pos.y
            && obj_player_id.0 == source_player_id.0
            && obj_state.is_active()
        {
            // Add effect to object
            obj_effects
                .0
                .insert(light_effect.effect.clone(), (game_tick.0 + 1, 0.0, 1));

            commands.trigger(UpdateObj {
                entity: obj_entity,
                attrs: vec![(VISION.to_string(), "Pending".to_string())],
            });
        }
    }

    // Add light effect to objects that don't have Effects yet (insert Effects component first)
    for (obj_entity, obj_player_id, obj_id, obj_pos, obj_state) in query_without_effects.iter() {
        if obj_pos.x == source_pos.x
            && obj_pos.y == source_pos.y
            && obj_player_id.0 == source_player_id.0
            && obj_state.is_active()
        {
            // Create new Effects with the light effect
            let mut new_effects = Effects(HashMap::new());
            new_effects
                .0
                .insert(light_effect.effect.clone(), (game_tick.0 + 1, 0.0, 1));

            commands.entity(obj_entity).insert(new_effects);

            commands.trigger(UpdateObj {
                entity: obj_entity,
                attrs: vec![(VISION.to_string(), "Pending".to_string())],
            });
        }
    }
}

fn remove_light_effect_system(
    light_effect: On<RemoveLightEffect>,
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut map_events: ResMut<MapEvents>,
    source_query: Query<(&PlayerId, &Position, &State)>,
    mut query: Query<(Entity, &PlayerId, &Id, &Position, &State, &mut Effects), With<Viewshed>>,
) {
    // Get source player id and position
    let Ok((source_player_id, source_pos, source_state)) = source_query.get(light_effect.entity)
    else {
        error!("Cannot find source from {:?}", light_effect.entity);
        return;
    };

    // Remove watchtower light effect from all objects on the same tile
    for (obj_entity, obj_player_id, obj_id, obj_pos, obj_state, mut obj_effects) in query.iter_mut()
    {
        if obj_pos.x == source_pos.x
            && obj_pos.y == source_pos.y
            && obj_player_id.0 == source_player_id.0
            && obj_state.is_active()
        {
            // Remove effect from object
            obj_effects.0.remove(&light_effect.effect);

            commands.trigger(UpdateObj {
                entity: obj_entity,
                attrs: vec![(VISION.to_string(), "Pending".to_string())],
            });
        }
    }
}

/*fn gather_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut resources: ResMut<Resources>,
    map: Res<Map>,
    mut items: ResMut<Items>,
    mut skills: ResMut<Skills>,
    templates: Res<Templates>,
    mut map_events: ResMut<MapEvents>,
    active_infos: Res<ActiveInfos>,
    mut query: Query<ObjQueryMutPlayerTemplate>,
) {
    let mut events_to_add: Vec<MapEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            // Execute event
            match &map_event.event_type {
                VisibleEvent::GatherEvent { res_type } => {
                    debug!("Processing GatherEvent res_type: {:?}", res_type);
                    events_to_remove.push(*map_event_id);

                    let Some(gatherer_entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find gatherer from {:?}", map_event.obj_id);
                        continue;
                    };

                    let Ok(mut gatherer) = query.get_mut(gatherer_entity) else {
                        error!("Query failed to find entity {:?}", gatherer_entity);
                        continue;
                    };

                    // Remove Event In Progress
                    commands.entity(gatherer_entity).remove::<EventInProgress>();

                    // Reset operator state to None if not a hero
                    if !gatherer.subclass.is_hero() {
                        *gatherer.state = State::None;
                    }

                    let capacity =
                        Obj::get_capacity(&gatherer.template.0, &templates.obj_templates);

                    let gather_result;

                    // Check if fishing and select a random nearby water tile
                    if res_type == FISH {
                        let nearby_tile_types = Map::get_nearby_tiles_by_types(
                            gatherer.pos.clone(),
                            vec![TileType::Ocean, TileType::River],
                            &map,
                        );

                        if nearby_tile_types.len() > 0 {
                            let (position, _) = nearby_tile_types
                                [rand::thread_rng().gen_range(0..nearby_tile_types.len())];

                            gather_result = Resource::gather_fishing(
                                gatherer.id.0,
                                gatherer.id.0,
                                Position {
                                    x: position.x,
                                    y: position.y,
                                },
                                res_type.to_string(),
                                &mut skills,
                                capacity,
                                &mut items,
                                &mut resources,
                                &templates,
                            );
                        } else {
                            error!("No nearby water tiles found for gatherer {:?}", gatherer.id);
                            continue;
                        }
                    } else {
                        gather_result = Resource::gather_by_type(
                            map_event.obj_id,
                            map_event.obj_id,
                            gatherer.pos.clone(),
                            res_type.to_string(),
                            &mut skills,
                            capacity,
                            &mut items,
                            &resources,
                            &templates,
                        );
                    }

                    let mut new_items = Vec::new();
                    let mut xp_list = Vec::new();

                    match gather_result {
                        Ok((gather_new_items, gather_xp_list)) => {
                            new_items = gather_new_items;
                            xp_list = gather_xp_list;
                        }
                        Err(ResourceGatherError::NoResourcesAvailable) => {
                            // Reset state to None
                            *gatherer.state = State::None;

                            let packet = ResponsePacket::Error {
                                errmsg: "No resources available on tile".to_string(),
                            };
                            send_to_client(gatherer.player_id.0, packet, &clients);
                            continue;
                        }
                        Err(ResourceGatherError::NoInventoryRoom) => {
                            // Reset state to None
                            *gatherer.state = State::None;

                            let packet = ResponsePacket::Error {
                                errmsg: "No inventory room available".to_string(),
                            };
                            send_to_client(gatherer.player_id.0, packet, &clients);
                            continue;
                        }
                        Err(ResourceGatherError::NoItemGathered) => {
                            // Only send notice if gatherer is a hero
                            if gatherer.subclass.is_hero() {
                                let packet = ResponsePacket::Notice {
                                    noticemsg: "No item gathered".to_string(),
                                    expiry: None,
                                };
                                send_to_client(gatherer.player_id.0, packet, &clients);
                            }

                            // Do not continue if no item gathered
                        }
                        Err(e) => {
                            // Just log the other errors
                            error!("Error gathering resource: {:?}", e);
                            continue;
                        }
                    }

                    if new_items.len() > 0 {
                        let notification_packet: ResponsePacket = ResponsePacket::NewItems {
                            action: STATE_GATHERING.to_string(),
                            source_id: map_event.obj_id, // Villager Id
                            item_name: new_items[0].name.clone(),
                            amount: 1,
                        };

                        send_to_client(gatherer.player_id.0, notification_packet, &clients);
                    }

                    if gatherer.subclass.is_hero() {
                        if xp_list.len() > 0 {
                            let skill_updated_packet = ResponsePacket::Xp {
                                id: map_event.obj_id,
                                xp_list: xp_list,
                            };

                            send_to_client(gatherer.player_id.0, skill_updated_packet, &clients);
                        }
                    }

                    // Check if gatherer is a hero and if so add another gather event
                    if gatherer.subclass.is_hero() {
                        let gather_event = VisibleEvent::GatherEvent {
                            res_type: res_type.clone(),
                        };

                        let map_event = MapEvent {
                            event_id: Uuid::new_v4(),
                            obj_id: map_event.obj_id,
                            run_tick: game_tick.0 + 40,
                            event_type: gather_event,
                        };

                        events_to_add.push(map_event);
                    }

                    let active_info_key =
                        (gatherer.player_id.0, gatherer.id.0, "inventory".to_string());

                    if let Some(active_info) = active_infos.get(&active_info_key) {
                        let item_update_packet: ResponsePacket = ResponsePacket::InfoItemsUpdate {
                            id: map_event.obj_id,
                            items_updated: new_items,
                            items_removed: Vec::new(),
                        };

                        send_to_client(gatherer.player_id.0, item_update_packet, &clients);
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        map_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}*/

/*fn operate_refine_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    resources: ResMut<Resources>,
    mut items: ResMut<Items>,
    mut skills: ResMut<Skills>,
    templates: Res<Templates>,
    mut map_events: ResMut<MapEvents>,
    active_infos: Res<ActiveInfos>,
    mut query: Query<ObjQueryMutPlayerTemplate>,
) {
    let mut events_to_add: Vec<MapEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    'event_loop: for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            // Execute event
            match &map_event.event_type {
                VisibleEvent::OperateEvent { structure_id } => {
                    info!("Processing OperateEvent");
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find entity from id: {:?}", map_event.obj_id);
                        continue;
                    };

                    // Add EventExecuting
                    commands.entity(entity).insert(EventExecuting {
                        at_tick: game_tick.0,
                    });

                    // Set state back to none
                    let Ok(mut operator) = query.get_mut(entity) else {
                        error!("Query failed to find entity {:?}", entity);
                        continue;
                    };

                    let operator_id = operator.id.0;
                    let operator_player_id = operator.player_id.0;
                    let operator_subclass = operator.subclass.clone();

                    // Reset operator state to None if not a hero
                    if !operator_subclass.is_hero() {
                        *operator.state = State::None;
                    }

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find entity from structure_id: {:?}", structure_id);
                        continue;
                    };

                    let Ok(structure) = query.get(structure_entity) else {
                        error!("Query failed to find entity {:?}", entity);
                        continue;
                    };

                    let res_type = Structure::resource_type(structure.template.0.clone());

                    let capacity =
                        Obj::get_capacity(&structure.template.0, &templates.obj_templates);

                    let gather_result = Resource::gather_by_type(
                        operator_id,
                        *structure_id,
                        Position {
                            x: structure.pos.x,
                            y: structure.pos.y,
                        },
                        res_type.to_string(),
                        &mut skills,
                        capacity,
                        &mut items,
                        &resources,
                        &templates,
                    );

                    let new_items;
                    let xp_list;

                    match gather_result {
                        Ok((gather_new_items, gather_xp_list)) => {
                            new_items = gather_new_items;
                            xp_list = gather_xp_list;
                        }
                        Err(ResourceGatherError::NoResourcesAvailable) => {
                            commands.trigger(StateChange {
                                entity,
                                new_state: State::None,
                            });

                            let packet = ResponsePacket::Error {
                                errmsg: "No resources available on tile".to_string(),
                            };
                            send_to_client(operator_player_id, packet, &clients);
                            continue;
                        }
                        Err(ResourceGatherError::NoInventoryRoom) => {
                            commands.trigger(StateChange {
                                entity,
                                new_state: State::None,
                            });

                            let packet = ResponsePacket::Error {
                                errmsg: "No inventory room available".to_string(),
                            };
                            send_to_client(operator_player_id, packet, &clients);
                            continue;
                        }
                        Err(ResourceGatherError::NoItemGathered) => {
                            // Only send notice if operator is a hero
                            if operator_subclass.is_hero() {
                                let packet = ResponsePacket::Notice {
                                    noticemsg: "No item gathered".to_string(),
                                    expiry: None,
                                };
                                send_to_client(operator_player_id, packet, &clients);
                            }
                            continue;
                        }
                        Err(e) => {
                            // Just log the other errors
                            error!("Error gathering resource: {:?}", e);
                            continue;
                        }
                    }

                    let active_info_key = (
                        structure.player_id.0,
                        structure.id.0,
                        "inventory".to_string(),
                    );

                    // Check if Xp notification must be sent
                    if operator_subclass.is_hero() {
                        let skill_updated_packet = ResponsePacket::Xp {
                            id: map_event.obj_id,
                            xp_list: xp_list,
                        };

                        send_to_client(structure.player_id.0, skill_updated_packet, &clients);
                    }

                    // Check if refiner is a hero and if so add another refine event
                    if operator_subclass.is_hero() {
                        let operate_event = VisibleEvent::OperateEvent {
                            structure_id: structure_id.clone(),
                        };

                        let map_event = MapEvent {
                            event_id: Uuid::new_v4(),
                            obj_id: map_event.obj_id,
                            run_tick: game_tick.0 + 40,
                            event_type: operate_event,
                        };

                        events_to_add.push(map_event);
                    }

                    if let Some(_active_info) = active_infos.get(&active_info_key) {
                        let item_update_packet: ResponsePacket = ResponsePacket::InfoItemsUpdate {
                            id: *structure_id,
                            items_updated: new_items,
                            items_removed: Vec::new(),
                        };

                        send_to_client(structure.player_id.0, item_update_packet, &clients);
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        map_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}*/

fn forage_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    templates: Res<Templates>,
    mut query: Query<ObjQueryMutPlayerTemplate>,
    skills_query: Query<&Skills>,
) {
    let events_to_add: Vec<GameEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (event_id, game_event_type) in game_events.iter_mut() {
        if game_event_type.run_tick < game_tick.0 {
            if game_event_belongs_to_protected_run(&game_event_type.event_type, &ids, &presence) {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::ForageEvent { forager_id } => {
                    info!("Processing ForageEvent");
                    events_to_remove.push(*event_id);

                    let Some(forager_entity) = entity_map.get_entity(*forager_id) else {
                        error!("Cannot find entity from forager_id: {:?}", forager_id);
                        continue;
                    };

                    let foraging_level = skills_query
                        .get(forager_entity)
                        .map(|s| s.get_level_by_name(Skill::Foraging))
                        .unwrap_or(0);

                    let Ok(mut forager) = query.get_mut(forager_entity) else {
                        error!("Query failed to find entity {:?}", forager_entity);
                        continue;
                    };

                    commands.trigger(StateChange {
                        entity: forager_entity,
                        new_state: State::None,
                    });

                    let mut new_items;

                    // Get map tile type from forager position
                    let map_tile_type = Map::tile_type(forager.pos.x, forager.pos.y, &map);

                    let forage_result = Resource::forage(
                        *forager_id,
                        map_tile_type,
                        ids.new_item_id(),
                        &mut forager.inventory,
                        &templates,
                    );

                    match forage_result {
                        Ok(forage_new_items) => {
                            new_items = forage_new_items;
                        }
                        Err(e) => {
                            error!("Error foraging: {:?}", e);
                            continue;
                        }
                    }

                    // T3.4: bonus forage roll scales with Foraging skill — at level 10 it's
                    // 10% chance for a second item, capped at 50% near max levels.
                    let bonus_chance = (foraging_level as f32 / 100.0).clamp(0.0, 0.5);
                    if bonus_chance > 0.0 && rand::random::<f32>() < bonus_chance {
                        let bonus_result = Resource::forage(
                            *forager_id,
                            map_tile_type,
                            ids.new_item_id(),
                            &mut forager.inventory,
                            &templates,
                        );
                        if let Ok(bonus_items) = bonus_result {
                            new_items.extend(bonus_items);
                        }
                    }

                    info!("New items: {:?}", new_items);

                    if new_items.len() > 0 {
                        let notification_packet: ResponsePacket = ResponsePacket::NewItems {
                            action: STATE_GATHERING.to_string(),
                            source_id: *forager_id, // Villager Id
                            item_name: new_items[0].name.clone(),
                            amount: 1,
                        };

                        send_to_client(forager.player_id.0, notification_packet, &clients);
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        game_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }
}

fn gather_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: OptionalPlayerWorldPresence,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    resources: Res<Resources>,
    recipes: Res<Recipes>,
    templates: Res<Templates>,
    active_infos: Res<ActiveInfos>,
    mut query: Query<GathererQuery>,
) {
    let events_to_add: Vec<GameEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (event_id, game_event_type) in game_events.iter_mut() {
        if game_event_type.run_tick < game_tick.0 {
            if game_event_belongs_to_protected_run(&game_event_type.event_type, &ids, &presence) {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::GatherEvent {
                    gatherer_id,
                    res_type,
                } => {
                    info!("Processing GatherEvent");
                    events_to_remove.push(*event_id);

                    let Some(gatherer_entity) = entity_map.get_entity(*gatherer_id) else {
                        error!("Cannot find entity from gatherer_id: {:?}", gatherer_id);
                        continue;
                    };

                    let Ok(mut gatherer) = query.get_mut(gatherer_entity) else {
                        error!("Query failed to find entity {:?}", gatherer_entity);
                        continue;
                    };

                    if *gatherer.state != State::Gathering {
                        debug!(
                            "Skipping stale GatherEvent for {:?}; state is {:?}",
                            gatherer_id, gatherer.state
                        );
                        continue;
                    }

                    commands.trigger(StateChange {
                        entity: gatherer_entity,
                        new_state: State::None,
                    });

                    // Get gatherer capacity
                    let capacity =
                        Obj::get_capacity(&gatherer.template.0, &templates.obj_templates);

                    let mut rng = rand::thread_rng();

                    let resources_on_tile = Resource::get_by_type(
                        gatherer.pos.clone(),
                        res_type.clone(),
                        &resources,
                        true,
                    );
                    let res_templates = &templates.res_templates;
                    let item_templates = &templates.item_templates;

                    let mut items_to_update: Vec<network::Item> = Vec::new();
                    // QW2: accumulate gathered skill XP so the hero gets the
                    // floating "+N Skill XP" outcome feedback on completion.
                    let mut gathered_xp: i32 = 0;
                    let mut gathered_skill: Option<String> = None;
                    let mut gathered_levelup: Option<i32> = None;

                    info!("Resources on tile: {:?}", resources_on_tile);
                    for resource in resources_on_tile.iter() {
                        if let Some(res_template) = res_templates.get(&resource.name) {
                            let skill_name = Resource::type_to_skill(res_type.clone());
                            let skill_name_enum = Skill::from_str(&skill_name)
                                .expect(&format!("Invalid skill name: {}", skill_name));

                            let mut skill_value = 0;

                            if let Some(gatherer_skill) =
                                gatherer.skills.get_by_name(skill_name_enum.clone())
                            {
                                skill_value = gatherer_skill.level;
                            }

                            info!("Res template: {:?}", res_template);
                            info!("Skill value: {:?}", skill_value);
                            info!("Skill name: {:?}", skill_name);
                            let gather_chance =
                                Resource::gather_chance(skill_value, res_template.skill_req);

                            let random_num = rng.gen::<f32>();

                            info!("Gather chance: {:?}", gather_chance);
                            info!("Random number: {:?}", random_num);

                            if random_num < gather_chance {
                                info!("Gathering resource: {:?}", resource.name);
                                let resource_quantity = 1;

                                let current_total_weight = gatherer.inventory.get_total_weight();
                                let mut total_needed_weight = 0;

                                if let Some(produces) = &resource.produces {
                                    for produce in produces.iter() {
                                        total_needed_weight += Item::get_weight_from_template(
                                            produce.clone(),
                                            resource_quantity,
                                            &item_templates,
                                        );
                                    }
                                } else {
                                    total_needed_weight = Item::get_weight_from_template(
                                        resource.name.clone(),
                                        resource_quantity,
                                        &item_templates,
                                    );
                                }

                                if (current_total_weight + total_needed_weight) < capacity {
                                    // Update skill
                                    let levelup = gatherer.skills.update(
                                        skill_name_enum.clone(),
                                        100,
                                        &templates.skill_templates,
                                    );
                                    gathered_xp += 100;
                                    gathered_skill = Some(skill_name.clone());
                                    if levelup.is_some() {
                                        gathered_levelup = levelup;
                                    }

                                    let mut item_attrs = HashMap::new();

                                    let quality_rate = res_template
                                        .quality_rate
                                        .clone()
                                        .unwrap_or(vec![60, 30, 10]);

                                    // Determine quality
                                    let dist = WeightedIndex::new(quality_rate).unwrap();
                                    let sample = dist.sample(&mut rng);
                                    let quality_level = sample as i32;

                                    debug!("Quality Level: {:?}", quality_level);

                                    for property in resource.properties.iter() {
                                        debug!("{:?} {:?}", property.name, property.value);
                                        //let characteristic_value = rng.gen_range(characteristic.min..characteristic.max);

                                        let attr_key = AttrKey::str_to_key(property.name.clone());

                                        item_attrs.insert(
                                            attr_key,
                                            item::AttrVal::Num(property.value as f32),
                                        );
                                    }

                                    debug!("item_attrs: {:?}", item_attrs);
                                    debug!("Produces: {:?}", resource.produces);

                                    if let Some(produces) = &resource.produces {
                                        for produce in produces.iter() {
                                            let item_name = produce.clone();

                                            let (new_item, _merged) =
                                                gatherer.inventory.new_with_attrs(
                                                    ids.new_item_id(),
                                                    *gatherer_id,
                                                    item_name,
                                                    1, //TODO should this be only 1
                                                    item_attrs.clone(),
                                                    &templates.item_templates,
                                                );

                                            items_to_update.push(Item::to_packet(new_item));
                                        }
                                    } else {
                                        let (new_item, _merged) =
                                            gatherer.inventory.new_with_attrs(
                                                ids.new_item_id(),
                                                *gatherer_id,
                                                resource.name.clone(),
                                                1, //TODO should this be only 1
                                                item_attrs.clone(),
                                                &templates.item_templates,
                                            );

                                        items_to_update.push(Item::to_packet(new_item));
                                    }
                                } else {
                                    info!(
                                        "Not enough inventory capacity to gather resource: {:?}",
                                        resource.name
                                    );
                                }
                            } else {
                                info!("Failed to gather resource: {:?}", resource.name);
                            }
                        } else {
                            info!(
                                "No resource template found for resource: {:?}",
                                resource.name
                            );
                        }
                    }

                    if gatherer.subclass.is_hero() {
                        // QW2: surface the skill XP gained as floating "+N Skill XP"
                        // text (mirrors the refine path); the item result is shown
                        // by the NewItems notice below.
                        if gathered_xp > 0 {
                            if let Some(skill) = gathered_skill.clone() {
                                let xp_update_packet = ResponsePacket::Xp {
                                    id: *gatherer_id,
                                    xp_list: vec![network::Xp {
                                        skill,
                                        xp: gathered_xp,
                                        levelup: gathered_levelup,
                                    }],
                                };
                                send_to_client(gatherer.player_id.0, xp_update_packet, &clients);
                            }
                        }

                        if let Some(first) = items_to_update.first() {
                            let packet = ResponsePacket::NewItems {
                                action: STATE_GATHERING.to_string(),
                                source_id: *gatherer_id,
                                item_name: first.name.clone(),
                                amount: 1,
                            };
                            send_to_client(gatherer.player_id.0, packet, &clients);
                        } else {
                            let packet = ResponsePacket::Notice {
                                noticemsg: "You gathered nothing.".to_string(),
                                expiry: Some(2000),
                            };
                            send_to_client(gatherer.player_id.0, packet, &clients);
                        }
                    }

                    commands.entity(gatherer_entity).insert(EventCompleted {
                        event_id: Uuid::new_v4(),
                        event_type: "gather".to_string(),
                        at_tick: game_tick.0,
                        success: true,
                    });
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        game_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }
}

fn structure_gather_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    resources: Res<Resources>,
    recipes: Res<Recipes>,
    templates: Res<Templates>,
    active_infos: Res<ActiveInfos>,
    mut operator_query: Query<(&Template, &mut Skills)>,
    mut structure_query: Query<(&Position, &Template, &mut Inventory)>,
) {
    let events_to_add: Vec<GameEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (event_id, game_event_type) in game_events.iter_mut() {
        if game_event_type.run_tick < game_tick.0 {
            if game_event_belongs_to_protected_run(&game_event_type.event_type, &ids, &presence) {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::StructureGatherEvent {
                    operator_id,
                    structure_id,
                } => {
                    info!("Processing StructureGatherEvent");
                    events_to_remove.push(*event_id);

                    let Some(operator_entity) = entity_map.get_entity(*operator_id) else {
                        error!("Cannot find entity from operator_id: {:?}", operator_id);
                        continue;
                    };

                    let Ok((operator_template, mut operator_skills)) =
                        operator_query.get_mut(operator_entity)
                    else {
                        error!("Query failed to find entity {:?}", operator_entity);
                        continue;
                    };

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find entity from structure_id: {:?}", structure_id);
                        continue;
                    };

                    let Ok((structure_pos, structure_template, mut structure_inventory)) =
                        structure_query.get_mut(structure_entity)
                    else {
                        error!("Query failed to find structure {:?}", structure_id);
                        continue;
                    };

                    commands.trigger(StateChange {
                        entity: operator_entity,
                        new_state: State::None,
                    });

                    let structure_template =
                        templates.obj_templates.get(structure_template.0.clone());

                    // Get structure resource type
                    let activity = structure_template.activity.unwrap_or("".to_string());
                    let res_type = match activity.as_str() {
                        "Mining" => ORE.to_string(),
                        "Logging" => LOG.to_string(),
                        "Stonecutting" => STONE.to_string(),
                        "Hunting" => GAME_ANIMAL.to_string(),
                        _ => "Invalid".to_string(),
                    };

                    // Get gatherer capacity
                    let capacity = structure_template.capacity.unwrap_or(0);

                    let mut rng = rand::thread_rng();

                    let resources_on_tile = Resource::get_by_type(
                        structure_pos.clone(),
                        res_type.clone(),
                        &resources,
                        true,
                    );
                    let res_templates = &templates.res_templates;
                    let item_templates = &templates.item_templates;

                    let mut items_to_update: Vec<network::Item> = Vec::new();

                    info!("Resources on tile: {:?}", resources_on_tile);
                    for resource in resources_on_tile.iter() {
                        if let Some(res_template) = res_templates.get(&resource.name) {
                            let skill_name = Resource::type_to_skill(res_type.clone());
                            let skill_name_enum = Skill::from_str(&skill_name)
                                .expect(&format!("Invalid skill name: {}", skill_name));

                            let mut skill_value = 0;

                            if let Some(operator_skill) =
                                operator_skills.get_by_name(skill_name_enum.clone())
                            {
                                skill_value = operator_skill.level;
                            }

                            info!("Res template: {:?}", res_template);
                            info!("Skill value: {:?}", skill_value);
                            info!("Skill name: {:?}", skill_name);
                            let gather_chance =
                                Resource::gather_chance(skill_value, res_template.skill_req);

                            let random_num = rng.gen::<f32>();

                            info!("Gather chance: {:?}", gather_chance);
                            info!("Random number: {:?}", random_num);

                            if random_num < gather_chance {
                                info!("Gathering resource: {:?}", resource.name);
                                let resource_quantity = 1;

                                let current_total_weight = structure_inventory.get_total_weight();
                                let mut total_needed_weight = 0;

                                if let Some(produces) = &resource.produces {
                                    for produce in produces.iter() {
                                        total_needed_weight += Item::get_weight_from_template(
                                            produce.clone(),
                                            resource_quantity,
                                            &item_templates,
                                        );
                                    }
                                } else {
                                    total_needed_weight = Item::get_weight_from_template(
                                        resource.name.clone(),
                                        resource_quantity,
                                        &item_templates,
                                    );
                                }

                                if (current_total_weight + total_needed_weight) < capacity {
                                    // Update skill
                                    operator_skills.update(
                                        skill_name_enum.clone(),
                                        100,
                                        &templates.skill_templates,
                                    );

                                    let mut item_attrs = HashMap::new();

                                    let quality_rate = res_template
                                        .quality_rate
                                        .clone()
                                        .unwrap_or(vec![60, 30, 10]);

                                    // Determine quality
                                    let dist = WeightedIndex::new(quality_rate).unwrap();
                                    let sample = dist.sample(&mut rng);
                                    let quality_level = sample as i32;

                                    debug!("Quality Level: {:?}", quality_level);

                                    for property in resource.properties.iter() {
                                        debug!("{:?} {:?}", property.name, property.value);
                                        //let characteristic_value = rng.gen_range(characteristic.min..characteristic.max);

                                        let attr_key = AttrKey::str_to_key(property.name.clone());

                                        item_attrs.insert(
                                            attr_key,
                                            item::AttrVal::Num(property.value as f32),
                                        );
                                    }

                                    debug!("item_attrs: {:?}", item_attrs);
                                    debug!("Produces: {:?}", resource.produces);

                                    if let Some(produces) = &resource.produces {
                                        for produce in produces.iter() {
                                            let item_name = produce.clone();

                                            let (new_item, _merged) = structure_inventory
                                                .new_with_attrs(
                                                    ids.new_item_id(),
                                                    *structure_id,
                                                    item_name,
                                                    1, //TODO should this be only 1
                                                    item_attrs.clone(),
                                                    &templates.item_templates,
                                                );

                                            items_to_update.push(Item::to_packet(new_item));
                                        }
                                    } else {
                                        let (new_item, _merged) = structure_inventory
                                            .new_with_attrs(
                                                ids.new_item_id(),
                                                *structure_id,
                                                resource.name.clone(),
                                                1, //TODO should this be only 1
                                                item_attrs.clone(),
                                                &templates.item_templates,
                                            );

                                        items_to_update.push(Item::to_packet(new_item));
                                    }
                                } else {
                                    info!(
                                        "No enough inventory capacity to gather resource: {:?}",
                                        resource.name
                                    );
                                }
                            } else {
                                info!("Failed to gather resource: {:?}", resource.name);
                            }
                        } else {
                            info!(
                                "No resource template found for resource: {:?}",
                                resource.name
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        game_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }
}

fn refine_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    active_infos: Res<ActiveInfos>,
    mut query: Query<(&Template, &State, &mut Inventory, &mut Skills)>,
) {
    let events_to_add: Vec<GameEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    'event_loop: for (event_id, game_event_type) in game_events.iter_mut() {
        if game_event_type.run_tick < game_tick.0 {
            if game_event_belongs_to_protected_run(&game_event_type.event_type, &ids, &presence) {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::RefineEvent {
                    refiner_id,
                    item_id,
                } => {
                    info!("Processing RefineEvent");
                    events_to_remove.push(*event_id);

                    // Get refiner player id
                    let Some(refiner_player_id) = ids.get_player(*refiner_id) else {
                        error!("Cannot find player id from refiner_id: {:?}", refiner_id);
                        continue;
                    };

                    let Some(refiner_entity) = entity_map.get_entity(*refiner_id) else {
                        error!("Cannot find entity from refiner_id: {:?}", refiner_id);
                        continue;
                    };

                    let Ok((
                        refiner_template,
                        refiner_state,
                        mut refiner_inventory,
                        mut refiner_skills,
                    )) = query.get_mut(refiner_entity)
                    else {
                        error!("Cannot find refiner from entity {:?}", refiner_entity);
                        continue;
                    };

                    if *refiner_state != State::Refining {
                        debug!(
                            "Skipping stale RefineEvent for {:?}; state is {:?}",
                            refiner_id, refiner_state
                        );
                        continue;
                    }

                    // Remove Event In Progress
                    commands.entity(refiner_entity).remove::<EventInProgress>();

                    // Set State back to None
                    commands.trigger(StateChange {
                        entity: refiner_entity,
                        new_state: State::None,
                    });

                    let Some(item_to_refine) = refiner_inventory.get_by_id(*item_id) else {
                        error!("Cannot find item to refine from item id: {:?}", item_id);
                        continue 'event_loop;
                    };

                    let refiner_capacity =
                        Obj::get_capacity(&refiner_template.0, &templates.obj_templates);

                    let mut items_to_update = Vec::new();

                    let item_template =
                        Item::get_template(item_to_refine.name, &templates.item_templates);

                    let Some(produces_list) = item_template.produces.clone() else {
                        error!(
                            "Missing item produces attribute for item template {:?}",
                            item_template
                        );
                        continue 'event_loop;
                    };

                    let item_to_refine_weight = item_to_refine.weight as i32;
                    let mut produced_items = Vec::new();

                    // Butchery doubles yield per produces-entry (better cuts than improvised tools)
                    let yield_multiplier = if refiner_template.0 == "Butchery" {
                        2
                    } else {
                        1
                    };

                    // Create new items
                    for produce_item_name in produces_list.iter() {
                        let produce_item_template = Item::get_template(
                            produce_item_name.to_string(),
                            &templates.item_templates,
                        );
                        let produce_quantity = yield_multiplier;

                        let current_total_weight = refiner_inventory.get_total_weight();
                        let item_weight = produce_item_template.weight as i32 * produce_quantity;

                        info!("Current total weight: {:?}", current_total_weight);
                        info!("Item to refine weight: {:?}", item_to_refine_weight);
                        info!("Item weight: {:?}", item_weight);
                        info!("Source capacity: {:?}", refiner_capacity);
                        if current_total_weight - item_to_refine_weight + item_weight
                            > refiner_capacity
                        {
                            info!("Refining refiner is full {:?}", refiner_id);
                            // Send error packet to refiner
                            let error_packet: ResponsePacket = ResponsePacket::Error {
                                errmsg: "Inventory is full, cannot refine".to_string(),
                            };
                            send_to_client(refiner_player_id, error_packet, &clients);

                            // Add State Change Event to None
                            commands.trigger(StateChange {
                                entity: refiner_entity,
                                new_state: State::None,
                            });

                            continue 'event_loop;
                        }

                        if let Some(merged_item) = refiner_inventory
                            .update_quantity(produce_item_template.name.clone(), produce_quantity)
                        {
                            items_to_update.push(Item::to_packet(merged_item.clone()));
                            produced_items.push((merged_item.id, produce_quantity));
                        } else {
                            let new_item = refiner_inventory.new(
                                ids.new_item_id(),
                                produce_item_template.name.clone(),
                                produce_quantity,
                                &templates.item_templates,
                            );

                            items_to_update.push(Item::to_packet(new_item.clone()));
                            produced_items.push((new_item.id, produce_quantity));
                        }
                    }

                    // Consume item to refine
                    let refined_item = refiner_inventory.remove_quantity(item_to_refine.id, 1);
                    let refined_item_packet;

                    info!("Refined item: {:?}", refined_item);

                    // No refined item, set packet to none
                    refined_item_packet = None;

                    let refine_skill = item_template
                        .refine_skill
                        .clone()
                        .expect("Item template missing refine skill.");
                    let refine_skill_enum = Skill::from_str(&refine_skill)
                        .expect(&format!("Invalid skill name: {}", refine_skill));

                    let levelup = refiner_skills.update(
                        refine_skill_enum.clone(),
                        100,
                        &templates.skill_templates,
                    );

                    // If hero, send xp update packet
                    if ids.is_hero(*refiner_id) {
                        let xp_update_packet: ResponsePacket = ResponsePacket::Xp {
                            id: *refiner_id,
                            xp_list: vec![network::Xp {
                                skill: refine_skill,
                                xp: 100,
                                levelup: levelup,
                            }],
                        };
                        send_to_client(refiner_player_id, xp_update_packet, &clients);
                    }

                    let refine_key = (*refiner_id, ActiveInfoType::Refine);

                    if let Some(_active_info) = active_infos.get(&refine_key) {
                        let refiner_capacity =
                            Obj::get_capacity(&refiner_template.0, &templates.obj_templates);
                        let refiner_total_weight = refiner_inventory.get_total_weight();
                        let refiner_items = refiner_inventory.get_packet();

                        let item_update_packet: ResponsePacket = ResponsePacket::InfoRefine {
                            refiner_id: *refiner_id,
                            structure_id: None,
                            refiner_items: refiner_items, // TODO update to use capacity and total weight
                            structure_items: None,
                            refining_item: refined_item_packet.clone(),
                            produced_items: produced_items,
                        };

                        send_to_client(refiner_player_id, item_update_packet, &clients);
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        game_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }
}

fn structure_refine_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    active_infos: Res<ActiveInfos>,
    mut refiner_query: Query<(&Subclass, &State, &mut Skills)>,
    mut structure_query: Query<(&Template, &mut Inventory, &mut WorkQueue)>,
) {
    let events_to_add: Vec<GameEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    'event_loop: for (event_id, game_event_type) in game_events.iter_mut() {
        if game_event_type.run_tick < game_tick.0 {
            if game_event_belongs_to_protected_run(&game_event_type.event_type, &ids, &presence) {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::StructureRefineEvent {
                    refiner_id,
                    structure_id,
                    item_id,
                } => {
                    info!("Processing RefineEvent");
                    events_to_remove.push(*event_id);

                    let Some(player_id) = ids.get_player(*refiner_id) else {
                        error!("Cannot find player id from refiner_id: {:?}", refiner_id);
                        continue;
                    };

                    let Some(refiner_entity) = entity_map.get_entity(*refiner_id) else {
                        error!("Cannot find entity from refiner_id: {:?}", refiner_id);
                        continue;
                    };

                    let Ok((refiner_subclass, refiner_state, mut refiner_skills)) =
                        refiner_query.get_mut(refiner_entity)
                    else {
                        error!("Cannot find refiner from entity {:?}", refiner_entity);
                        continue;
                    };

                    if *refiner_state != State::Refining {
                        debug!(
                            "Skipping stale StructureRefineEvent for {:?}; state is {:?}",
                            refiner_id, refiner_state
                        );
                        continue;
                    }

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find entity from structure_id: {:?}", structure_id);
                        continue;
                    };

                    let Ok((structure_template, mut structure_inventory, mut work_queue_entries)) =
                        structure_query.get_mut(structure_entity)
                    else {
                        error!("Cannot find structure from entity {:?}", structure_entity);
                        continue;
                    };

                    // Remove Event In Progress
                    commands.entity(refiner_entity).remove::<EventInProgress>();

                    // Set State back to None
                    commands.trigger(StateChange {
                        entity: refiner_entity,
                        new_state: State::None,
                    });

                    let Some(item_to_refine) = structure_inventory.get_by_id(*item_id) else {
                        // No items to refine, skip event
                        debug!(
                            "No items to refine for structure {:?} with item id {:?}",
                            structure_id, item_id
                        );
                        continue;
                    };

                    let structure_capacity =
                        Obj::get_capacity(&structure_template.0, &templates.obj_templates);

                    let mut items_to_update = Vec::new();
                    let mut items_to_remove = Vec::new();

                    let item_template =
                        Item::get_template(item_to_refine.name.clone(), &templates.item_templates);

                    let Some(produces_list) = item_template.produces.clone() else {
                        error!(
                            "Missing item produces attribute for item template {:?}",
                            item_template
                        );
                        continue;
                    };

                    let item_to_refine_weight = item_to_refine.weight as i32;
                    let mut produced_items = Vec::new();

                    // Butchery doubles yield per produces-entry (better cuts than improvised tools)
                    let yield_multiplier = if structure_template.0 == "Butchery" {
                        2
                    } else {
                        1
                    };

                    // Create new items
                    for produce_item_name in produces_list.iter() {
                        let produce_item_template = Item::get_template(
                            produce_item_name.to_string(),
                            &templates.item_templates,
                        );
                        let produce_quantity = yield_multiplier;

                        let current_total_weight = structure_inventory.get_total_weight();
                        let item_weight = produce_item_template.weight as i32 * produce_quantity;

                        info!("Current total weight: {:?}", current_total_weight);
                        info!("Item to refine weight: {:?}", item_to_refine_weight);
                        info!("Item weight: {:?}", item_weight);
                        info!("Structure capacity: {:?}", structure_capacity);
                        if current_total_weight - item_to_refine_weight + item_weight
                            > structure_capacity
                        {
                            info!("Refining structure is full {:?}", structure_id);
                            // Send error packet to refiner
                            let error_packet: ResponsePacket = ResponsePacket::Error {
                                errmsg: "Inventory is full, cannot refine".to_string(),
                            };
                            send_to_client(player_id, error_packet, &clients);
                            continue 'event_loop;
                        }

                        if let Some(merged_item) = structure_inventory
                            .update_quantity(produce_item_template.name.clone(), produce_quantity)
                        {
                            items_to_update.push(Item::to_packet(merged_item.clone()));
                            produced_items.push((merged_item.id, produce_quantity));
                        } else {
                            let new_item = structure_inventory.new(
                                ids.new_item_id(),
                                produce_item_template.name.clone(),
                                produce_quantity,
                                &templates.item_templates,
                            );

                            items_to_update.push(Item::to_packet(new_item.clone()));
                            produced_items.push((new_item.id, produce_quantity));
                        }
                    }

                    // Consume item to refine
                    let refined_item = structure_inventory.remove_quantity(item_to_refine.id, 1);
                    let refined_item_packet;

                    info!("Refined item: {:?}", refined_item);
                    if let Some(refined_item) = refined_item {
                        let mut produces_list_packet: Vec<network::ProducedItem> = Vec::new();

                        for produce in produces_list.iter() {
                            let produce_template =
                                Item::get_template(produce.to_string(), &templates.item_templates);

                            produces_list_packet.push(network::ProducedItem {
                                name: produce_template.name.clone(),
                                image: produce_template.image.clone(),
                                class: produce_template.class.clone(),
                                subclass: produce_template.subclass.clone(),
                            });
                        }

                        // Get refine time
                        let item_template = Item::get_template(
                            refined_item.name.clone(),
                            &templates.item_templates,
                        );
                        let refine_time = item_template.get_refine_time();

                        refined_item_packet = Some(RefiningItem {
                            id: refined_item.id,
                            name: refined_item.name,
                            image: refined_item.image,
                            class: refined_item.class,
                            subclass: refined_item.subclass,
                            quantity: refined_item.quantity,
                            produces: produces_list_packet,
                            refining_skill: item_template
                                .refine_skill
                                .clone()
                                .expect("Missing refine skill"),
                            refine_time: refine_time / TICKS_PER_SEC,
                            progress: 0,
                        });

                        // Add another refine event
                        /*let game_event = GameEvent {
                            event_id: ids.new_map_event_id(),
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + refine_time,
                            event_type: GameEventType::RefineEvent {
                                refiner_id: *refiner_id,
                                structure_id: *structure_id,
                                item_id: item_to_refine.id,
                            },
                        };

                        events_to_add.push(game_event);*/
                    } else {
                        // No refined item, set packet to none
                        refined_item_packet = None;

                        // Item was removed, add to remove list
                        items_to_remove.push(item_to_refine.id);

                        // Add State Change Event to None
                        commands.trigger(StateChange {
                            entity: refiner_entity,
                            new_state: State::None,
                        });
                    }

                    let refine_skill = item_template
                        .refine_skill
                        .clone()
                        .expect("Item template missing refine skill.");
                    let refine_skill_enum = Skill::from_str(&refine_skill)
                        .expect(&format!("Invalid skill name: {}", refine_skill));

                    let levelup = refiner_skills.update(
                        refine_skill_enum.clone(),
                        100,
                        &templates.skill_templates,
                    );

                    // Remove completed work queue entry
                    work_queue_entries
                        .0
                        .retain(|entry| entry.worker_id != *refiner_id);

                    if let Some(_active_info_players) =
                        active_infos.get(&(*structure_id, ActiveInfoType::StructureRefine))
                    {
                        let structure_capacity =
                            Obj::get_capacity(&structure_template.0, &templates.obj_templates);
                        let structure_total_weight = structure_inventory.get_total_weight();

                        let structure_inventory_packet = network::Inventory {
                            id: *structure_id,
                            cap: structure_capacity,
                            tw: structure_total_weight,
                            items: structure_inventory.get_packet(),
                        };

                        let item_update_packet: ResponsePacket =
                            ResponsePacket::InfoStructureRefine {
                                structure_inventory: structure_inventory_packet,
                                refining_item: refined_item_packet.clone(),
                                produced_items: produced_items,
                            };

                        send_to_client(player_id, item_update_packet, &clients);
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        game_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }
}

fn craft_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: OptionalPlayerWorldPresence,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    recipes: Res<Recipes>,
    templates: Res<Templates>,
    active_infos: Res<ActiveInfos>,
    mut query: Query<(&PlayerId, &Subclass, &State, &mut Inventory, &mut Skills)>,
) {
    let events_to_add: Vec<GameEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (event_id, game_event_type) in game_events.iter_mut() {
        if game_event_type.run_tick < game_tick.0 {
            if game_event_belongs_to_protected_run(&game_event_type.event_type, &ids, &presence) {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::CraftEvent {
                    crafter_id,
                    recipe_name,
                } => {
                    info!("Processing CraftEvent");
                    events_to_remove.push(*event_id);

                    let Some(crafter_entity) = entity_map.get_entity(*crafter_id) else {
                        error!("Cannot find entity from crafter_id: {:?}", crafter_id);
                        continue;
                    };

                    let Ok((
                        crafter_player_id,
                        crafter_subclass,
                        crafter_state,
                        mut crafter_inventory,
                        mut crafter_skills,
                    )) = query.get_mut(crafter_entity)
                    else {
                        error!("Cannot find crafter from entity {:?}", crafter_entity);
                        continue;
                    };

                    if *crafter_state != State::Crafting {
                        debug!(
                            "Skipping stale CraftEvent for {:?}; state is {:?}",
                            crafter_id, crafter_state
                        );
                        continue;
                    }

                    // Add State Change Event to None
                    commands.trigger(StateChange {
                        entity: crafter_entity,
                        new_state: State::None,
                    });

                    let Some(recipe) = recipes.get_by_name(recipe_name.clone()) else {
                        error!("Recipe not found {:?}", recipe_name);
                        continue;
                    };

                    if let Some(item_reqs) = crafter_inventory.find_by_reqs(recipe.req.clone()) {
                        let item_name = if let Some(_item_name_from_req) = recipe.item_name_from_req
                        {
                            // Get the first item in the item reqs and then the first word in the item name
                            let req_type_name =
                                item_reqs[0].name.split_whitespace().next().unwrap_or("");
                            info!("Req type name: {:?}", req_type_name);

                            // Prepend the req_type_name to the recipe name
                            format!("{} {}", req_type_name, recipe_name)
                        } else {
                            recipe_name.to_string()
                        };

                        info!("Item name: {:?}", item_name);

                        // Create new item
                        let new_item_id = crafter_inventory.craft(
                            ids.new_item_id(),
                            *crafter_id,
                            item_name,
                            &recipe,
                            None,
                            None,
                        );

                        // Get skill from recipe class first if not found get from subclass
                        let skill_name_enum = match SkillData::item_class_to_skill(&recipe.class) {
                            Some(name) => name,
                            None => match SkillData::item_subclass_to_skill(&recipe.subclass) {
                                Some(name) => name,
                                None => {
                                    error!("Invalid item subclass {:?}", recipe.subclass);
                                    continue;
                                }
                            },
                        };

                        // Update skill
                        crafter_skills.update(
                            skill_name_enum.clone(),
                            100,
                            &templates.skill_templates,
                        );

                        if let Some(_active_info_players) =
                            active_infos.get(&(*crafter_id, ActiveInfoType::Craft))
                        {
                            let crafter_items = crafter_inventory.get_packet();
                            let crafter_recipes = recipes.get_basic_recipes_packet();

                            let info_craft_packet: ResponsePacket = ResponsePacket::InfoCraft {
                                crafter_id: *crafter_id,
                                structure_id: None,
                                items: crafter_items,
                                recipes: crafter_recipes,
                                crafting_item: None,
                            };
                            send_to_client(crafter_player_id.0, info_craft_packet, &clients);
                        }
                    } else {
                        error!(
                            "Crafter {:?} does not have required items {:?}",
                            crafter_id, recipe_name
                        );
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        game_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }
}

fn structure_craft_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    recipes: Res<Recipes>,
    templates: Res<Templates>,
    active_infos: Res<ActiveInfos>,
    mut crafter_query: Query<(&State, &mut Skills)>,
    mut query: Query<(&Template, &mut Inventory, &mut WorkQueue)>,
) {
    let events_to_add: Vec<GameEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (event_id, game_event_type) in game_events.iter_mut() {
        if game_event_type.run_tick < game_tick.0 {
            if game_event_belongs_to_protected_run(&game_event_type.event_type, &ids, &presence) {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::StructureCraftEvent {
                    crafter_id,
                    structure_id,
                    recipe_name,
                } => {
                    info!("Processing CraftEvent");
                    events_to_remove.push(*event_id);

                    let Some(crafter_player_id) = ids.get_player(*crafter_id) else {
                        error!("Cannot find player from crafter_id: {:?}", crafter_id);
                        continue;
                    };
                    let Some(structure_player_id) = ids.get_player(*structure_id) else {
                        error!("Cannot find player from structure_id: {:?}", structure_id);
                        continue;
                    };

                    let Some(crafter_entity) = entity_map.get_entity(*crafter_id) else {
                        error!("Cannot find entity from crafter_id: {:?}", crafter_id);
                        continue;
                    };

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find entity from crafter_id: {:?}", crafter_id);
                        continue;
                    };

                    let Ok((crafter_state, mut crafter_skills)) =
                        crafter_query.get_mut(crafter_entity)
                    else {
                        error!(
                            "Cannot find crafter skills from entity {:?}",
                            crafter_entity
                        );
                        continue;
                    };

                    if *crafter_state != State::Crafting {
                        debug!(
                            "Skipping stale StructureCraftEvent for {:?}; state is {:?}",
                            crafter_id, crafter_state
                        );
                        continue;
                    }

                    // The craft event is consumed past this point, so release the
                    // crafter BEFORE the structure lookup can bail — failing that
                    // lookup (structure despawned / missing WorkQueue) used to leave
                    // the crafter wedged in State::Crafting forever, paralyzing it
                    // until it died of thirst.
                    commands.trigger(StateChange {
                        entity: crafter_entity,
                        new_state: State::None,
                    });

                    let Ok((structure_template, mut structure_inventory, mut work_queue_entries)) =
                        query.get_mut(structure_entity)
                    else {
                        error!("Cannot find structure from entity {:?}", structure_entity);
                        continue;
                    };

                    let Some(recipe) = recipes.get_by_name(recipe_name.clone()) else {
                        error!(
                            "Recipe not found {:?} for structure {:?}",
                            recipe_name, structure_id
                        );
                        continue;
                    };

                    if let Some(item_reqs) = structure_inventory.find_by_reqs(recipe.req.clone()) {
                        let item_name = if let Some(_item_name_from_req) = recipe.item_name_from_req
                        {
                            // Get the first item in the item reqs and then the first word in the item name
                            let req_type_name =
                                item_reqs[0].name.split_whitespace().next().unwrap_or("");
                            info!("Req type name: {:?}", req_type_name);

                            // Prepend the req_type_name to the recipe name
                            format!("{} {}", req_type_name, recipe_name)
                        } else {
                            recipe_name.to_string()
                        };

                        info!("Item name: {:?}", item_name);

                        // Create new item
                        let new_item = structure_inventory.craft(
                            ids.new_item_id(),
                            *structure_id,
                            item_name,
                            &recipe,
                            None,
                            None,
                        );

                        // Get skill from recipe class first if not found get from subclass
                        let skill_name_enum = match SkillData::item_class_to_skill(&recipe.class) {
                            Some(name) => name,
                            None => match SkillData::item_subclass_to_skill(&recipe.subclass) {
                                Some(name) => name,
                                None => {
                                    error!("Invalid item subclass {:?}", recipe.subclass);
                                    continue;
                                }
                            },
                        };

                        // Update skill
                        crafter_skills.update(
                            skill_name_enum.clone(),
                            100,
                            &templates.skill_templates,
                        );

                        // Remove crafter from work queue
                        work_queue_entries
                            .0
                            .retain(|entry| entry.worker_id != *crafter_id);

                        if let Some(_active_info_players) =
                            active_infos.get(&(*structure_id, ActiveInfoType::StructureCraft))
                        {
                            let structure_inventory_packet = network::Inventory {
                                id: *structure_id,
                                cap: Obj::get_capacity(
                                    &structure_template.0,
                                    &templates.obj_templates,
                                ),
                                tw: structure_inventory.get_total_weight(),
                                items: structure_inventory.get_packet(),
                            };

                            let info_structure_craft_packet: ResponsePacket =
                                ResponsePacket::InfoStructureCraft {
                                    structure_inventory: structure_inventory_packet,
                                    recipes: Some(recipes.get_by_structure_packet(
                                        structure_player_id,
                                        structure_template.0.clone(),
                                    )),
                                    queue: vec![],
                                    crafting_item: None,
                                };

                            send_to_client(
                                crafter_player_id,
                                info_structure_craft_packet,
                                &clients,
                            );
                        }
                    } else {
                        error!("Structure does not have required items {:?}", structure_id);
                        // Send error packet to crafter
                        let error_packet: ResponsePacket = ResponsePacket::Error {
                            errmsg: "Structure does not have required items".to_string(),
                        };
                        send_to_client(crafter_player_id, error_packet, &clients);
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        game_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }
}

fn structure_operate_event_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    resources: Res<Resources>,
    mut operator_query: Query<(&PlayerId, &Position, &mut Skills)>,
    mut query: Query<(
        &PlayerId,
        &Position,
        &Template,
        &State,
        &mut Inventory,
        &mut WorkQueue,
    )>,
) {
    let mut events_to_add: Vec<GameEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (event_id, game_event_type) in game_events.iter() {
        if game_event_type.run_tick < game_tick.0 {
            if game_event_belongs_to_protected_run(&game_event_type.event_type, &ids, &presence) {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::StructureOperateEvent {
                    operator_id,
                    structure_id,
                } => {
                    info!("Processing StructureOperateEvent");
                    events_to_remove.push(*event_id);

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find structure from {:?}", structure_id);
                        continue;
                    };

                    let Some(operator_entity) = entity_map.get_entity(*operator_id) else {
                        error!("Cannot find operator from {:?}", operator_id);
                        continue;
                    };

                    // Set Operator State back to None
                    commands.trigger(StateChange {
                        entity: operator_entity,
                        new_state: State::None,
                    });

                    let Ok((operator_player_id, operator_pos, mut operator_skills)) =
                        operator_query.get_mut(operator_entity)
                    else {
                        error!("Cannot find operator from entity {:?}", operator_entity);
                        continue;
                    };

                    let Ok((
                        structure_player_id,
                        structure_pos,
                        structure_template,
                        structure_state,
                        mut structure_inventory,
                        mut work_queue_entries,
                    )) = query.get_mut(structure_entity)
                    else {
                        error!("Cannot find structure from entity {:?}", structure_entity);
                        continue;
                    };

                    // Check if operator is in the same position as the structure
                    if operator_pos != structure_pos {
                        error!("Operator is not in the same position as the structure");
                        continue;
                    }

                    // Check if operator and structure are owned by the same player
                    if operator_player_id.0 != structure_player_id.0 {
                        error!("Operator and structure are not owned by the same player");
                        continue;
                    }

                    // Check if structure is active
                    if !structure_state.is_active() {
                        error!("Structure is not active");
                        continue;
                    }

                    // Remove Event In Progress
                    commands.entity(operator_entity).remove::<EventInProgress>();

                    // T2.7 integration point: for food-production structures (Bakery,
                    // Smoker, Millhouse, Butchery) we want villagers assigned via
                    // Order::Operate to auto-craft from inventory rather than gather
                    // from the tile. The recipe picker is `recipe::pick_available_recipe_at`.
                    // Triggering a StructureCraftEvent here requires (a) adding the
                    // `recipes: Res<Recipes>` param to this system and (b) transitioning
                    // the villager to State::Crafting, both of which need their own
                    // tests — leaving the wiring as a follow-up.

                    // Get gatherer capacity
                    let capacity =
                        Obj::get_capacity(&structure_template.0, &templates.obj_templates);

                    // Get resource type
                    let res_type = Structure::resource_type(structure_template.0.clone());

                    // Get resources on tile by type
                    let resources_on_tile = Resource::get_by_type(
                        structure_pos.clone(),
                        res_type.clone(),
                        &resources,
                        true,
                    );

                    let res_templates = &templates.res_templates;
                    let item_templates = &templates.item_templates;

                    let mut rng = rand::thread_rng();

                    info!("Resources on tile: {:?}", resources_on_tile);
                    for resource in resources_on_tile.iter() {
                        if let Some(res_template) = res_templates.get(&resource.name) {
                            let skill_name = Resource::type_to_skill(res_type.clone());
                            let skill_name_enum = Skill::from_str(&skill_name)
                                .expect(&format!("Invalid skill name: {}", skill_name));

                            let mut skill_value = 0;

                            if let Some(operator_skill) =
                                operator_skills.get_by_name(skill_name_enum.clone())
                            {
                                skill_value = operator_skill.level;
                            }

                            info!("Res template: {:?}", res_template);
                            info!("Skill value: {:?}", skill_value);
                            info!("Skill name: {:?}", skill_name);
                            let gather_chance =
                                Resource::gather_chance(skill_value, res_template.skill_req);

                            let random_num = rng.gen::<f32>();

                            info!("Gather chance: {:?}", gather_chance);
                            info!("Random number: {:?}", random_num);

                            if random_num < gather_chance {
                                info!("Gathering resource: {:?}", resource.name);
                                let resource_quantity = 1;

                                let current_total_weight = structure_inventory.get_total_weight();
                                let mut total_needed_weight = 0;

                                if let Some(produces) = &resource.produces {
                                    for produce in produces.iter() {
                                        total_needed_weight += Item::get_weight_from_template(
                                            produce.clone(),
                                            resource_quantity,
                                            &item_templates,
                                        );
                                    }
                                } else {
                                    total_needed_weight = Item::get_weight_from_template(
                                        resource.name.clone(),
                                        resource_quantity,
                                        &item_templates,
                                    );
                                }

                                if (current_total_weight + total_needed_weight) < capacity {
                                    // Update skill
                                    operator_skills.update(
                                        skill_name_enum.clone(),
                                        100,
                                        &templates.skill_templates,
                                    );

                                    let mut item_attrs = HashMap::new();

                                    let quality_rate = res_template
                                        .quality_rate
                                        .clone()
                                        .unwrap_or(vec![60, 30, 10]);

                                    // Determine quality
                                    let dist = WeightedIndex::new(quality_rate).unwrap();
                                    let sample = dist.sample(&mut rng);
                                    let quality_level = sample as i32;

                                    debug!("Quality Level: {:?}", quality_level);

                                    for property in resource.properties.iter() {
                                        debug!("{:?} {:?}", property.name, property.value);
                                        //let characteristic_value = rng.gen_range(characteristic.min..characteristic.max);

                                        let attr_key = AttrKey::str_to_key(property.name.clone());

                                        item_attrs.insert(
                                            attr_key,
                                            item::AttrVal::Num(property.value as f32),
                                        );
                                    }

                                    debug!("item_attrs: {:?}", item_attrs);
                                    debug!("Produces: {:?}", resource.produces);

                                    if let Some(produces) = &resource.produces {
                                        for produce in produces.iter() {
                                            let item_name = produce.clone();

                                            let (_new_item, _merged) = structure_inventory
                                                .new_with_attrs(
                                                    ids.new_item_id(),
                                                    *structure_id,
                                                    item_name,
                                                    1, //TODO should this be only 1
                                                    item_attrs.clone(),
                                                    &templates.item_templates,
                                                );
                                        }
                                    } else {
                                        let (_new_item, _merged) = structure_inventory
                                            .new_with_attrs(
                                                ids.new_item_id(),
                                                *structure_id,
                                                resource.name.clone(),
                                                1, //TODO should this be only 1
                                                item_attrs.clone(),
                                                &templates.item_templates,
                                            );
                                    }
                                } else {
                                    info!(
                                        "No enough inventory capacity to for operating structure: {:?}",
                                        resource.name
                                    );
                                }
                            } else {
                                info!(
                                    "Failed to gather resource for operating structure: {:?}",
                                    resource.name
                                );
                            }
                        } else {
                            info!(
                                "No resource template found for resource: {:?}",
                                resource.name
                            );
                        }
                    }

                    // Set work status to completed
                    if let Some(work_entry) = work_queue_entries
                        .0
                        .iter_mut()
                        .find(|entry| entry.worker_id == *operator_id)
                    {
                        work_entry.work_status = WorkStatus::Idle;
                    }

                    // Trigger start work
                    commands.trigger(StartWork {
                        entity: operator_entity,
                        worker_id: *operator_id,
                        structure_id: *structure_id,
                    });
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        game_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }
}

fn experiment_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    mut recipes: ResMut<Recipes>,
    templates: Res<Templates>,
    mut experiments: ResMut<Experiments>,
    active_infos: Res<ActiveInfos>,
    mut structure_query: Query<(&PlayerId, &Name, &mut Inventory)>,
) {
    let events_to_add: Vec<GameEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (event_id, game_event_type) in game_events.iter_mut() {
        if game_event_type.run_tick < game_tick.0 {
            if game_event_belongs_to_protected_run(&game_event_type.event_type, &ids, &presence) {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::ExperimentEvent {
                    experimenter_id,
                    structure_id,
                } => {
                    info!("Processing ExperimentEvent");
                    events_to_remove.push(*event_id);

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find structure from {:?}", structure_id);
                        continue;
                    };

                    let Some(experimenter_entity) = entity_map.get_entity(*experimenter_id) else {
                        error!("Cannot find experimenter from {:?}", experimenter_id);
                        continue;
                    };

                    let Ok((structure_player_id, structure_name, mut structure_inventory)) =
                        structure_query.get_mut(structure_entity)
                    else {
                        error!("Cannot find structure from entity {:?}", structure_entity);
                        continue;
                    };

                    // Add State Change Event to None
                    commands.trigger(StateChange {
                        entity: experimenter_entity,
                        new_state: State::None,
                    });

                    // Remove Event In Progress
                    commands
                        .entity(experimenter_entity)
                        .remove::<EventInProgress>();

                    if let Some(experiment) = experiments.get_mut(structure_id) {
                        debug!("Finding experiment recipe... {:?}", experiment.recipe);

                        // If recipe is none, find a valid recipe for experimentation
                        if experiment.recipe == None {
                            let recipe = Experiment::find_recipe(
                                *structure_id,
                                structure_name.0.clone(),
                                &structure_inventory,
                                &recipes,
                                &templates,
                            );

                            if let Some(recipe) = recipe {
                                Experiment::set_recipe(recipe, experiment);
                            } else {
                                Experiment::set_trivial_source(experiment);
                            }
                        }

                        // Check res reqs
                        debug!("Checking experiment reagents");
                        if Experiment::check_reqs(*structure_id, experiment, &structure_inventory) {
                            // Check discovery and create new recipe
                            let exp_state = Experiment::check_discovery(
                                structure_player_id.0,
                                *structure_id,
                                experiment,
                                &mut structure_inventory,
                                &templates,
                                &mut recipes,
                            );

                            if exp_state == ExperimentState::Discovery {
                                // Remove Villager order
                                // TODO Order should be set to non
                                //commands.entity(villager_entity).remove::<Order>();
                            }

                            /*player::active_info_experiment(
                                structure_player_id.0,
                                *structure_id,
                                experiment.clone(),
                                &items,
                                &active_infos,
                                &clients,
                                &templates,
                            );*/
                        } else {
                            debug!("Not enough reagents to continue experiment");
                        }
                    } else {
                        error!("No experiment found for {:?}", structure_id);
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        game_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }
}

/*fn experiment_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    _resources: ResMut<Resources>,
    mut items: ResMut<Items>,
    _skills: ResMut<Skills>,
    templates: Res<Templates>,
    mut recipes: ResMut<Recipes>,
    mut experiments: ResMut<Experiments>,
    mut map_events: ResMut<MapEvents>,
    active_infos: Res<ActiveInfos>,
    mut query: Query<ObjQueryMutPlayerTemplate>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            // Execute event
            match &map_event.event_type {
                VisibleEvent::ExperimentEvent { structure_id } => {
                    info!("Processing ExperimentEvent");
                    events_to_remove.push(*map_event_id);

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find structure from {:?}", map_event.obj_id);
                        continue;
                    };

                    let Some(experimenter_entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find experimenter from {:?}", map_event.obj_id);
                        continue;
                    };

                    let entities = [experimenter_entity, structure_entity];

                    let Ok([mut experimenter, structure]) = query.get_many_mut(entities) else {
                        error!(
                            "Cannot find experimenter or structure from entities {:?}",
                            entities
                        );
                        continue;
                    };

                    // Reset experimenter state
                    *experimenter.state = State::None;

                    // Remove Event In Progress
                    commands
                        .entity(experimenter_entity)
                        .remove::<EventInProgress>();

                    if let Some(experiment) = experiments.get_mut(structure_id) {
                        debug!("Finding experiment recipe... {:?}", experiment.recipe);

                        // If recipe is none, find a valid recipe for experimentation
                        if experiment.recipe == None {
                            let recipe = Experiment::find_recipe(
                                *structure_id,
                                structure.name.0.clone(),
                                &structure.inventory,
                                &recipes,
                                &templates,
                            );

                            if let Some(recipe) = recipe {
                                Experiment::set_recipe(recipe, experiment);
                            } else {
                                Experiment::set_trivial_source(experiment);
                            }
                        }

                        // Check res reqs
                        debug!("Checking experiment reagents");
                        if Experiment::check_reqs(*structure_id, experiment, &items) {
                            // Check discovery and create new recipe
                            let exp_state = Experiment::check_discovery(
                                structure.player_id.0,
                                *structure_id,
                                experiment,
                                &mut structure.inventory,
                                &templates,
                                &mut recipes,
                            );

                            if exp_state == ExperimentState::Discovery {
                                // Remove Villager order
                                // TODO Order should be set to non
                                //commands.entity(villager_entity).remove::<Order>();
                            }

                            player::active_info_experiment(
                                structure.player_id.0,
                                *structure_id,
                                experiment.clone(),
                                &items,
                                &active_infos,
                                &clients,
                                &templates,
                            );
                        } else {
                            debug!("Not enough reagents to continue experiment");
                        }
                    } else {
                        error!("No experiment found for {:?}", structure_id);
                    }
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}*/

fn nearby_passable_pos(center: Position, radius: u32, map: &Map) -> Option<Position> {
    let mut candidates: Vec<Position> = Map::range((center.x, center.y), radius)
        .into_iter()
        .filter(|(x, y)| (*x != center.x || *y != center.y) && Map::is_valid_pos((*x, *y)))
        .filter(|(x, y)| Map::is_passable(*x, *y, map))
        .map(|(x, y)| Position { x, y })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let index = rand::thread_rng().gen_range(0..candidates.len());
    Some(candidates.swap_remove(index))
}

fn nearby_ocean_adjacent_passable_pos(
    center: Position,
    radius: u32,
    map: &Map,
) -> Option<Position> {
    let mut candidates: Vec<Position> = Map::range((center.x, center.y), radius)
        .into_iter()
        .filter(|(x, y)| (*x != center.x || *y != center.y) && Map::is_valid_pos((*x, *y)))
        .filter(|(x, y)| Map::is_passable(*x, *y, map))
        .map(|(x, y)| Position { x, y })
        .filter(|pos| Map::are_tile_types_nearby(*pos, vec![TileType::Ocean], map))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let index = rand::thread_rng().gen_range(0..candidates.len());
    Some(candidates.swap_remove(index))
}

fn loot_poi_spawn_pos(template_name: &str, center: Position, map: &Map) -> Option<Position> {
    match template_name {
        "Washed Ashore Materials" => nearby_ocean_adjacent_passable_pos(center, 2, map),
        _ => nearby_passable_pos(center, 2, map),
    }
}

fn reveal_nearby_resource(
    center: Position,
    resources: &mut Resources,
    res_templates: &ResTemplates,
) -> Option<Resource> {
    for (x, y) in Map::range((center.x, center.y), 3) {
        let revealed = Resource::explore(0, Position { x, y }, resources, res_templates);
        if let Some(resource) = revealed.first() {
            return Some(resource.clone());
        }
    }

    None
}

fn send_notice(player_id: i32, message: &str, clients: &Res<Clients>) {
    send_to_client(
        player_id,
        ResponsePacket::Notice {
            noticemsg: message.to_string(),
            expiry: Some(8000),
        },
        clients,
    );
}

fn add_inventory_salvage(
    player_id: i32,
    obj_id: i32,
    _owner_template: &String,
    action: &str,
    inventory: &mut Inventory,
    ids: &mut ResMut<Ids>,
    templates: &Res<Templates>,
    clients: &Res<Clients>,
) {
    let salvage_table = [
        ("Firewood", 3),
        ("Honeybell Berries", 2),
        ("Crude Bandage", 1),
        ("Pebble", 2),
        ("Cragroot Maple Stick", 1),
    ];
    let (item_name, quantity) = salvage_table[rand::thread_rng().gen_range(0..salvage_table.len())];

    let item = inventory.new(
        ids.new_item_id(),
        item_name.to_string(),
        quantity,
        &templates.item_templates,
    );
    let item_packet = item.packet();

    send_to_client(
        player_id,
        ResponsePacket::InfoItemsUpdate {
            id: inventory.owner,
            items_updated: vec![item_packet],
            items_removed: Vec::new(),
        },
        clients,
    );

    send_to_client(
        player_id,
        ResponsePacket::InfoItem {
            action: Some("inventory".to_string()),
            id: item.id,
            owner: item.owner,
            name: item.name.clone(),
            quantity: item.quantity,
            durability: item.durability,
            class: item.class.clone(),
            subclass: item.subclass.clone(),
            image: item.image.clone(),
            weight: item.weight,
            equipped: item.equipped,
            price: None,
            attrs: Some(item.attrs.clone()),
            produces: if item.produces.is_empty() {
                None
            } else {
                Some(item.produces.clone())
            },
        },
        clients,
    );

    send_to_client(
        player_id,
        ResponsePacket::NewItems {
            action: action.to_string(),
            source_id: obj_id,
            item_name: item_name.to_string(),
            amount: quantity,
        },
        clients,
    );
}

fn add_timed_explore_effect(
    player_id: i32,
    obj_id: i32,
    pos: Position,
    effects: &mut Effects,
    effect: Effect,
    game_tick: i32,
    templates: &Res<Templates>,
    map_events: &mut ResMut<MapEvents>,
    clients: &Res<Clients>,
) {
    let effect_template = templates
        .effect_templates
        .get(&effect.clone().to_str())
        .expect("Effect missing from templates");
    let duration_ticks = effect_template.duration * TICKS_PER_SEC;

    effects.0.insert(effect.clone(), (duration_ticks, 1.0, 1));
    map_events.new(
        obj_id,
        game_tick + duration_ticks,
        VisibleEvent::EffectExpiredEvent {
            effect: effect.clone(),
        },
    );

    send_to_client(
        player_id,
        ResponsePacket::GainedEffect {
            id: obj_id,
            x: pos.x,
            y: pos.y,
            effect: effect.to_str(),
        },
        clients,
    );
}

fn clear_effect_with_item(
    player_id: i32,
    obj_id: i32,
    pos: Position,
    effects: &mut Effects,
    effect: Effect,
    clients: &Res<Clients>,
) -> bool {
    if !remove_explore_negative_effect(effects, effect.clone()) {
        return false;
    }

    send_to_client(
        player_id,
        ResponsePacket::LostEffect {
            id: obj_id,
            x: pos.x,
            y: pos.y,
            effect: effect.to_str(),
        },
        clients,
    );

    true
}

/// Loot caches that are placed by exploration and should auto-despawn: quickly
/// once emptied, and after a timeout even if the player never returns for them.
pub fn is_loot_poi(template_name: &str) -> bool {
    matches!(template_name, "Supply Cache" | "Washed Ashore Materials")
}

fn spawn_loot_poi(
    template_name: &str,
    center: Position,
    commands: &mut Commands,
    ids: &mut ResMut<Ids>,
    entity_map: &mut ResMut<EntityObjMap>,
    map: &Res<Map>,
    templates: &Res<Templates>,
    game_tick: &Res<GameTick>,
    game_events: &mut ResMut<GameEvents>,
) -> Option<Position> {
    let pos = loot_poi_spawn_pos(template_name, center, map)?;
    let poi_id = ids.new_obj_id();
    let mut inventory = Inventory {
        owner: poi_id,
        items: Vec::new(),
    };

    match template_name {
        "Supply Cache" => {
            inventory.new(
                ids.new_item_id(),
                "Crude Bandage".to_string(),
                1,
                &templates.item_templates,
            );
            inventory.new(
                ids.new_item_id(),
                "Firewood".to_string(),
                4,
                &templates.item_templates,
            );
            inventory.new(
                ids.new_item_id(),
                "Honeybell Berries".to_string(),
                2,
                &templates.item_templates,
            );
        }
        "Washed Ashore Materials" => {
            inventory.new(
                ids.new_item_id(),
                "Cragroot Maple Timber".to_string(),
                3,
                &templates.item_templates,
            );
            inventory.new(
                ids.new_item_id(),
                "Pebble".to_string(),
                2,
                &templates.item_templates,
            );
            inventory.new(
                ids.new_item_id(),
                "Crude Torch".to_string(),
                1,
                &templates.item_templates,
            );
        }
        _ => {}
    }

    let poi = Obj::create_nospawn(
        poi_id,
        MERCHANT_PLAYER_ID,
        template_name.to_string(),
        pos,
        State::None,
        inventory,
        templates,
    );
    let poi_entity = commands.spawn(poi).id();
    ids.new_obj(poi_id, MERCHANT_PLAYER_ID);
    entity_map.new_obj(poi_id, poi_entity);
    commands.trigger(NewObj { entity: poi_entity });

    // Schedule an auto-despawn so abandoned caches don't litter the map forever.
    // If the player empties it first, item_transfer_system schedules an earlier
    // despawn and this timeout becomes a harmless no-op.
    let despawn_event_id = ids.new_map_event_id();
    game_events.insert(
        despawn_event_id,
        GameEvent {
            event_id: despawn_event_id,
            start_tick: game_tick.0,
            run_tick: game_tick.0 + LOOT_POI_DESPAWN_TICKS,
            event_type: GameEventType::DespawnObj { obj_id: poi_id },
        },
    );

    Some(pos)
}

fn schedule_early_merchant_signal(
    player_id: i32,
    game_tick: i32,
    ids: &mut ResMut<Ids>,
    game_events: &mut ResMut<GameEvents>,
    initial_encounter_state: &Res<InitialEncounterState>,
) -> bool {
    let Some(entry) = initial_encounter_state.get(&player_id) else {
        return false;
    };
    if entry.merchant_id == 0 {
        return false;
    }
    if game_events.iter().any(|(_, event)| {
        matches!(
            event.event_type,
            GameEventType::MerchantArrival {
                merchant_id,
                player_id: event_player_id,
            } if merchant_id == entry.merchant_id && event_player_id == player_id
        )
    }) {
        return false;
    }

    let event_id = ids.new_map_event_id();
    game_events.insert(
        event_id,
        GameEvent {
            event_id,
            start_tick: game_tick,
            run_tick: game_tick + 600,
            event_type: GameEventType::MerchantArrival {
                merchant_id: entry.merchant_id,
                player_id,
            },
        },
    );
    true
}

fn apply_explore_outcome(
    outcome: ExploreOutcomeKind,
    player_id: i32,
    obj_id: i32,
    pos: Position,
    owner_template: &String,
    action: &str,
    commands: &mut Commands,
    ids: &mut ResMut<Ids>,
    entity_map: &mut ResMut<EntityObjMap>,
    map: &Res<Map>,
    resources: &mut ResMut<Resources>,
    inventory: &mut Inventory,
    effects: &mut Effects,
    templates: &Res<Templates>,
    clients: &Res<Clients>,
    game_tick: &Res<GameTick>,
    map_events: &mut ResMut<MapEvents>,
    game_events: &mut ResMut<GameEvents>,
    initial_encounter_state: &Res<InitialEncounterState>,
) {
    match outcome {
        ExploreOutcomeKind::ResourceGlimpse => {
            if let Some(resource) = reveal_nearby_resource(pos, resources, &templates.res_templates)
            {
                send_notice(
                    player_id,
                    &format!(
                        "A careful search reveals signs of {} nearby.",
                        resource.name
                    ),
                    clients,
                );
            } else {
                add_inventory_salvage(
                    player_id,
                    obj_id,
                    owner_template,
                    action,
                    inventory,
                    ids,
                    templates,
                    clients,
                );
            }
        }
        ExploreOutcomeKind::MinorSalvage => {
            add_inventory_salvage(
                player_id,
                obj_id,
                owner_template,
                action,
                inventory,
                ids,
                templates,
                clients,
            );
        }
        ExploreOutcomeKind::SupplyCache => {
            if let Some(cache_pos) = spawn_loot_poi(
                "Supply Cache",
                pos,
                commands,
                ids,
                entity_map,
                map,
                templates,
                game_tick,
                game_events,
            ) {
                send_notice(
                    player_id,
                    &format!(
                        "You spot a tucked-away supply cache at {},{}.",
                        cache_pos.x, cache_pos.y
                    ),
                    clients,
                );
            } else {
                add_inventory_salvage(
                    player_id,
                    obj_id,
                    owner_template,
                    action,
                    inventory,
                    ids,
                    templates,
                    clients,
                );
            }
        }
        ExploreOutcomeKind::WashedAshoreMaterials => {
            if let Some(cache_pos) = spawn_loot_poi(
                "Washed Ashore Materials",
                pos,
                commands,
                ids,
                entity_map,
                map,
                templates,
                game_tick,
                game_events,
            ) {
                send_notice(
                    player_id,
                    &format!(
                        "You notice useful debris washed ashore at {},{}.",
                        cache_pos.x, cache_pos.y
                    ),
                    clients,
                );
            } else {
                add_inventory_salvage(
                    player_id,
                    obj_id,
                    owner_template,
                    action,
                    inventory,
                    ids,
                    templates,
                    clients,
                );
            }
        }
        ExploreOutcomeKind::PoiClue => {
            send_to_client(
                player_id,
                ResponsePacket::DiscoveryEvent {
                    version: 1,
                    discovery_type: "clue".to_string(),
                    title: "Old trail sign".to_string(),
                    unlock_source: "Exploration".to_string(),
                    location: Some(format!("{},{}", pos.x, pos.y)),
                    result: "Tracks and broken brush suggest there is something worth investigating nearby.".to_string(),
                },
                clients,
            );
        }
        ExploreOutcomeKind::EarlyMerchantSignal => {
            if schedule_early_merchant_signal(
                player_id,
                game_tick.0,
                ids,
                game_events,
                initial_encounter_state,
            ) {
                send_notice(
                    player_id,
                    "You catch sight of a sail changing course toward the island.",
                    clients,
                );
            } else {
                add_inventory_salvage(
                    player_id,
                    obj_id,
                    owner_template,
                    action,
                    inventory,
                    ids,
                    templates,
                    clients,
                );
            }
        }
        ExploreOutcomeKind::StirredEnemy => {
            if let Some(spawn_pos) = nearby_passable_pos(pos, 3, map) {
                let (entity, _, _, _) = Encounter::spawn_npc(
                    NPC_PLAYER_ID,
                    spawn_pos,
                    random_early_game_enemy_template().to_string(),
                    commands,
                    ids,
                    entity_map,
                    templates,
                );
                commands.trigger(NewObj { entity });
                send_notice(
                    player_id,
                    "Your search stirs something hungry in the brush.",
                    clients,
                );
            } else {
                send_notice(player_id, "The brush goes still. Too still.", clients);
            }
        }
        ExploreOutcomeKind::BrambleWound => {
            add_timed_explore_effect(
                player_id,
                obj_id,
                pos,
                effects,
                Effect::Bleed,
                game_tick.0,
                templates,
                map_events,
                clients,
            );
        }
        ExploreOutcomeKind::FoulSpores => {
            add_timed_explore_effect(
                player_id,
                obj_id,
                pos,
                effects,
                Effect::Sickness,
                game_tick.0,
                templates,
                map_events,
                clients,
            );
        }
        ExploreOutcomeKind::DarkOmen => {
            add_timed_explore_effect(
                player_id,
                obj_id,
                pos,
                effects,
                Effect::Cursed,
                game_tick.0,
                templates,
                map_events,
                clients,
            );
        }
    }
}

fn explore_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut entity_map: ResMut<EntityObjMap>,
    map: Res<Map>,
    mut resources: ResMut<Resources>,
    templates: Res<Templates>,
    mut game_events: ResMut<GameEvents>,
    initial_encounter_state: Res<InitialEncounterState>,
    mut survey_history: ResMut<SurveyHistory>,
    mut player_events: ResMut<PlayerEvents>,
    mut query: Query<(
        &PlayerId,
        &Id,
        &Position,
        &Template,
        &mut State,
        &mut Inventory,
        &mut Effects,
    )>,
    mut map_events: ResMut<MapEvents>,
) {
    let mut events_to_remove = Vec::new();
    let due_events: Vec<_> = map_events
        .iter()
        .filter(|(_, map_event)| map_event.run_tick < game_tick.0)
        .map(|(event_id, map_event)| (*event_id, map_event.clone()))
        .collect();

    for (map_event_id, map_event) in due_events {
        if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
            continue;
        }
        match &map_event.event_type {
            VisibleEvent::SurveyEvent
            | VisibleEvent::ProspectEvent
            | VisibleEvent::ExploreEvent => {
                let is_survey = matches!(map_event.event_type, VisibleEvent::SurveyEvent);
                let is_prospect = matches!(
                    map_event.event_type,
                    VisibleEvent::ProspectEvent | VisibleEvent::ExploreEvent
                );
                debug!("Processing discovery event: {:?}", map_event.event_type);
                events_to_remove.push(map_event_id);

                let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                    error!("Cannot find entity from id: {:?}", map_event.obj_id);
                    continue;
                };

                let Ok((
                    player_id,
                    explorer_id,
                    position,
                    template,
                    mut explorer_state,
                    mut inventory,
                    mut effects,
                )) = query.get_mut(entity)
                else {
                    error!("Query failed to find entity {:?}", entity);
                    continue;
                };

                let player_id_value = player_id.0;
                let explorer_id_value = explorer_id.0;
                let pos = Position {
                    x: position.x,
                    y: position.y,
                };

                *explorer_state = State::None;
                commands.trigger(StateChange {
                    entity,
                    new_state: State::None,
                });

                if is_prospect {
                    let revealed_resources = Resource::explore(
                        map_event.obj_id,
                        pos,
                        &mut resources,
                        &templates.res_templates,
                    );

                    if let Some(resource) = revealed_resources.first() {
                        let notification_packet: ResponsePacket = ResponsePacket::NewItems {
                            action: STATE_PROSPECTING.to_string(),
                            source_id: map_event.obj_id,
                            item_name: resource.name.clone(),
                            amount: revealed_resources.len() as i32,
                        };

                        send_to_client(player_id_value, notification_packet, &clients);
                    }

                    // Push a refreshed info tile so the player immediately sees the
                    // newly revealed resources on the prospected tile.
                    player_events.insert(
                        ids.player_event,
                        PlayerEvent::InfoTile {
                            player_id: player_id_value,
                            x: pos.x,
                            y: pos.y,
                        },
                    );
                    ids.player_event += 1;
                }

                if is_survey && record_tile_survey(player_id_value, pos, &mut survey_history) {
                    let outcome = roll_explore_outcome();
                    apply_explore_outcome(
                        outcome,
                        player_id_value,
                        explorer_id_value,
                        pos,
                        &template.0,
                        STATE_SURVEYING,
                        &mut commands,
                        &mut ids,
                        &mut entity_map,
                        &map,
                        &mut resources,
                        &mut inventory,
                        &mut effects,
                        &templates,
                        &clients,
                        &game_tick,
                        &mut map_events,
                        &mut game_events,
                        &initial_encounter_state,
                    );
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn send_objectives_packet(player_id: i32, obj: &PlayerObjectives, clients: &Res<Clients>) {
    let objectives_packet = ResponsePacket::Objectives {
        build_campfire: obj.build_campfire,
        build_3_structures: obj.build_3_structures,
        recruit_villager: obj.recruit_villager,
        explore_poi: obj.explore_poi,
        survive_5_nights: obj.survive_5_nights,
    };
    send_to_client(player_id, objectives_packet, clients);
}

fn investigate_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut entity_map: ResMut<EntityObjMap>,
    map: Res<Map>,
    mut resources: ResMut<Resources>,
    templates: Res<Templates>,
    mut game_events: ResMut<GameEvents>,
    initial_encounter_state: Res<InitialEncounterState>,
    mut investigated_pois: ResMut<InvestigatedPOIs>,
    mut objectives: ResMut<Objectives>,
    mut query: Query<(
        &PlayerId,
        &Id,
        &Position,
        &Template,
        &Subclass,
        &mut State,
        &mut Inventory,
        &mut Effects,
    )>,
    mut map_events: ResMut<MapEvents>,
) {
    let mut events_to_remove = Vec::new();
    let due_events: Vec<_> = map_events
        .iter()
        .filter(|(_, map_event)| map_event.run_tick < game_tick.0)
        .map(|(event_id, map_event)| (*event_id, map_event.clone()))
        .collect();

    for (map_event_id, map_event) in due_events {
        if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
            continue;
        }
        let VisibleEvent::InvestigateEvent { target_id } = map_event.event_type else {
            continue;
        };

        debug!("Processing InvestigateEvent");
        events_to_remove.push(map_event_id);

        let Some(explorer_entity) = entity_map.get_entity(map_event.obj_id) else {
            error!("Cannot find investigator from id: {:?}", map_event.obj_id);
            continue;
        };
        let Some(target_entity) = entity_map.get_entity(target_id) else {
            error!("Cannot find investigate target from id: {:?}", target_id);
            continue;
        };

        let Ok([mut explorer, target]) = query.get_many_mut([explorer_entity, target_entity])
        else {
            error!(
                "Cannot find investigator/target entities {:?} {:?}",
                explorer_entity, target_entity
            );
            continue;
        };

        let (
            explorer_player_id,
            explorer_id,
            explorer_pos,
            explorer_template,
            _explorer_subclass,
            mut explorer_state,
            mut explorer_inventory,
            mut explorer_effects,
        ) = explorer;
        let (
            _target_player_id,
            target_obj_id,
            target_pos,
            target_template,
            target_subclass,
            _target_state,
            _target_inventory,
            _target_effects,
        ) = target;

        let player_id = explorer_player_id.0;
        let investigator_id = explorer_id.0;
        let investigator_pos = *explorer_pos;
        let target_id_value = target_obj_id.0;
        let target_template_name = target_template.0.clone();
        let target_is_poi = *target_subclass == Subclass::Poi;
        let target_is_monolith = target_subclass.is_monolith();

        *explorer_state = State::None;
        commands.trigger(StateChange {
            entity: explorer_entity,
            new_state: State::None,
        });

        if target_is_monolith {
            commands.trigger(player::InfoMonolithEvent {
                entity: target_entity,
                player_id,
            });
            continue;
        }

        if !target_is_poi {
            send_notice(
                player_id,
                "There is nothing special to investigate here.",
                &clients,
            );
            continue;
        }

        commands.trigger(player::InfoPOIEvent {
            entity: target_entity,
            player_id,
        });

        let first_investigation =
            record_poi_investigation(player_id, target_id_value, &mut investigated_pois);

        if first_investigation {
            let player_obj = objectives
                .entry(player_id)
                .or_insert_with(PlayerObjectives::default);

            if target_template_name == "Shipwreck" && !player_obj.scavenge_shipwreck {
                player_obj.scavenge_shipwreck = true;
                // BB-A/BB-B: action-driven nudge toward the next objective.
                send_to_client(
                    player_id,
                    ResponsePacket::Notice {
                        noticemsg: "Supplies salvaged from the wreck. Build a campfire before dusk — light keeps the dark and its dangers at bay.".to_string(),
                        expiry: Some(10000),
                    },
                    &clients,
                );
                add_inventory_salvage(
                    player_id,
                    investigator_id,
                    &explorer_template.0,
                    STATE_INVESTIGATING,
                    &mut explorer_inventory,
                    &mut ids,
                    &templates,
                    &clients,
                );
            }

            if !player_obj.explore_poi {
                player_obj.explore_poi = true;
            }
            send_objectives_packet(player_id, player_obj, &clients);

            if !matches!(
                target_template_name.as_str(),
                "Shipwreck" | "Supply Cache" | "Washed Ashore Materials"
            ) {
                apply_explore_outcome(
                    roll_explore_outcome(),
                    player_id,
                    investigator_id,
                    investigator_pos,
                    &explorer_template.0,
                    STATE_INVESTIGATING,
                    &mut commands,
                    &mut ids,
                    &mut entity_map,
                    &map,
                    &mut resources,
                    &mut explorer_inventory,
                    &mut explorer_effects,
                    &templates,
                    &clients,
                    &game_tick,
                    &mut map_events,
                    &mut game_events,
                    &initial_encounter_state,
                );
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn farm_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    mut crops: ResMut<Crops>,
    resources: ResMut<Resources>,
    templates: Res<Templates>,
    mut query: Query<ObjQueryMutPlayerTemplate>,
    mut map_events: ResMut<MapEvents>,
    active_infos: Res<ActiveInfos>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::PlantEvent { structure_id } => {
                    info!("Processing PlantEvent");
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find entity from id: {:?}", map_event.obj_id);
                        continue;
                    };

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find entity from structure_id: {:?}", structure_id);
                        continue;
                    };

                    let entities = [entity, structure_entity];

                    let Ok([mut planter, mut structure]) = query.get_many_mut(entities) else {
                        error!(
                            "Cannot find planter or structure from entities {:?}",
                            entities
                        );
                        continue;
                    };

                    // Remove Event In Progress
                    commands.entity(entity).remove::<EventInProgress>();

                    // Reset villager state to None
                    *planter.state = State::None;

                    // Get seeds
                    let seeds = structure.inventory.get_by_class(item::SEEDS.to_string());

                    // Determine how many seeds the villager can plant TODO
                    let mut seeds_to_plant = 2;

                    let Some(seeds) = seeds else {
                        debug!("No seeds found to plant");
                        continue;
                    };

                    if seeds.quantity < seeds_to_plant {
                        seeds_to_plant = seeds.quantity;
                    }

                    // Derive crop type from the seed's `produces` field (e.g. Wheat Seeds -> Wheat).
                    // Falls back to "Wheat" for legacy generic "Seeds" items.
                    let seed_template =
                        Item::get_template(seeds.name.clone(), &templates.item_templates);
                    let crop_type = seed_template
                        .produces
                        .as_ref()
                        .and_then(|p| p.first().cloned())
                        .unwrap_or_else(|| "Wheat".to_string());

                    info!("Planting {:?} crops: {:?}", crop_type, seeds_to_plant);
                    crops.plant(game_tick.0, *structure_id, crop_type, seeds_to_plant);

                    // Consume item to refine
                    let new_seeds = structure
                        .inventory
                        .remove_quantity(seeds.id, seeds_to_plant);

                    let mut items_to_update: Vec<network::Item> = Vec::new();
                    let mut items_to_remove = Vec::new();

                    // Add item with zero quantity to remove list
                    if let Some(new_seeds) = new_seeds {
                        let new_seeds_packet = Item::to_packet(new_seeds);
                        items_to_update.push(new_seeds_packet);
                    } else {
                        // Item was removed, add to remove list
                        items_to_remove.push(seeds.id);
                    }

                    if let Some(_active_info_players) =
                        active_infos.get(&(*structure_id, ActiveInfoType::Inventory))
                    {
                        let item_update_packet: ResponsePacket = ResponsePacket::InfoItemsUpdate {
                            id: *structure_id,
                            items_updated: items_to_update,
                            items_removed: items_to_remove,
                        };

                        send_to_client(planter.player_id.0, item_update_packet, &clients);
                    }

                    if let Some(_active_info_players) =
                        active_infos.get(&(*structure_id, ActiveInfoType::Structure))
                    {
                        if let Some(crop) = crops.get(structure_id) {
                            let info_crop_packet: ResponsePacket = ResponsePacket::InfoCrop {
                                id: *structure_id,
                                crop_type: crop.crop_type.clone(),
                                crop_quantity: crop.quantity,
                                crop_stage: crop.stage.to_string(),
                            };

                            send_to_client(planter.player_id.0, info_crop_packet, &clients);
                        }
                    }
                }
                VisibleEvent::HarvestEvent { structure_id } => {
                    info!("Processing HarvestEvent");
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find entity from id: {:?}", map_event.obj_id);
                        continue;
                    };

                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find entity from structure_id: {:?}", structure_id);
                        continue;
                    };

                    let entities = [entity, structure_entity];

                    let Ok([mut villager, mut structure]) = query.get_many_mut(entities) else {
                        error!(
                            "Cannot find villager or structure from entities {:?}",
                            entities
                        );
                        continue;
                    };

                    // Remove Event In Progress
                    commands.entity(entity).remove::<EventInProgress>();

                    // Reset villager state to None
                    *villager.state = State::None;

                    if let Some(crop) = crops.harvest(*structure_id, 1) {
                        info!("Harvesting crop: {:?}", crop);
                        let item_template =
                            Item::get_template(crop.crop_type.clone(), &templates.item_templates);

                        let capacity =
                            Obj::get_capacity(&structure.template.0, &templates.obj_templates);

                        let current_total_weight = structure.inventory.get_total_weight();
                        let item_weight = Item::get_weight_from_template(
                            crop.crop_type.clone(),
                            1,
                            &templates.item_templates,
                        );

                        if current_total_weight + item_weight > capacity {
                            info!("Harvest structure is full {:?}", structure);
                            continue;
                        }

                        let (new_item, _merged) = structure.inventory.new_with_attrs(
                            ids.new_item_id(),
                            structure.id.0,
                            crop.crop_type.clone(),
                            1,
                            HashMap::new(),
                            &templates.item_templates,
                        );

                        // Convert items to be updated to packets
                        let new_item_packet = Item::to_packet(new_item.clone());

                        if let Some(_active_info_players) =
                            active_infos.get(&(*structure_id, ActiveInfoType::Inventory))
                        {
                            let item_update_packet: ResponsePacket =
                                ResponsePacket::InfoItemsUpdate {
                                    id: *structure_id,
                                    items_updated: vec![new_item_packet],
                                    items_removed: Vec::new(),
                                };

                            send_to_client(villager.player_id.0, item_update_packet, &clients);
                        }
                    } else {
                        info!("No crops to harvest");
                    }
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn repair_event_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    crops: ResMut<Crops>,
    resources: ResMut<Resources>,
    templates: Res<Templates>,
    //mut villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    //mut state_query: Query<&mut State>,
    mut query: Query<ObjWithStatsQuery>,
    mut map_events: ResMut<MapEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    active_infos: Res<ActiveInfos>,
    mut run_score_state: ResMut<RunScoreState>,
    crisis_state: Option<Res<SettlementCrisisState>>,
    mut balance_telemetry_state: Option<ResMut<CrisisBalanceTelemetryState>>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::RepairEvent { structure_id } => {
                    if object_belongs_to_protected_run(*structure_id, &ids, &presence) {
                        continue;
                    }
                    info!("Processing RepairEvent");
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find entity from id: {:?}", map_event.obj_id);
                        continue;
                    };

                    let Ok(mut villager) = query.get_mut(entity) else {
                        error!("Cannot find villager from entity: {:?}", entity);
                        continue;
                    };

                    // Remove Event In Progress
                    commands.entity(entity).remove::<EventInProgress>();

                    // Reset villager state to None
                    *villager.state = State::None;

                    commands.trigger(StateChange {
                        entity,
                        new_state: State::None,
                    });

                    // Get structure
                    let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                        error!("Cannot find structure from id: {:?}", structure_id);
                        continue;
                    };

                    let Ok(mut structure) = query.get_mut(structure_entity) else {
                        error!("Cannot find structure from entity: {:?}", structure_entity);
                        continue;
                    };

                    // Repair structure to full health.
                    let previous_hp = structure.stats.hp;
                    structure.stats.hp = structure.stats.base_hp;
                    if previous_hp < structure.stats.hp
                        && matches!(
                            crisis_state
                                .as_ref()
                                .and_then(|state| state.get(&structure.player_id.0))
                                .map(|crisis| crisis.phase),
                            Some(CrisisPhase::Preparing | CrisisPhase::AssaultReady)
                        )
                    {
                        if let Some(telemetry_state) = balance_telemetry_state.as_deref_mut() {
                            telemetry_state
                                .entry(structure.player_id.0)
                                .or_default()
                                .preparation_actions
                                .record_repair_completed(*structure_id, game_tick.0);
                        }
                    }
                    run_score_state
                        .entry(structure.player_id.0)
                        .or_insert_with(|| PlayerRunScore {
                            start_tick: game_tick.0,
                            ..PlayerRunScore::default()
                        })
                        .repairs += 1;
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

// Each spell requires a separate system
fn spell_raise_dead_event_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    run_spawned_objs: Res<RunSpawnedObjs>,
    mut entity_map: ResMut<EntityObjMap>,
    pos_query: Query<(&Position, &Template)>,
    mut caster_query: Query<(&mut State, &mut Minions)>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    mut event_executing_query: Query<&mut EventExecuting>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::SpellRaiseDeadEvent { corpse_id } => {
                    if object_belongs_to_protected_run(*corpse_id, &ids, &presence) {
                        events_to_remove.push(*map_event_id);
                        if let Some(caster_entity) = entity_map.get_entity(map_event.obj_id) {
                            commands.trigger(StateChange {
                                entity: caster_entity,
                                new_state: State::None,
                            });
                            if let Ok(mut event_executing) =
                                event_executing_query.get_mut(caster_entity)
                            {
                                event_executing.state = EventExecutingState::Failed;
                            }
                        }
                        continue;
                    }
                    debug!("Processing SpellRaiseDeadEvent {:?}", corpse_id);
                    events_to_remove.push(*map_event_id);

                    let Some(corpse_entity) = entity_map.get_entity(*corpse_id) else {
                        error!("Cannot find corpse from {:?}", corpse_id);
                        continue;
                    };

                    let Ok((corpse_pos, corpse_template)) = pos_query.get(corpse_entity) else {
                        error!("Cannot find corpse position {:?}", corpse_entity);
                        continue;
                    };

                    let Some(caster_entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find caster from {:?}", map_event.obj_id);
                        continue;
                    };

                    let Ok(mut event_executing) = event_executing_query.get_mut(caster_entity)
                    else {
                        error!(
                            "Cannot find event executing from entity: {:?}",
                            caster_entity
                        );
                        continue;
                    };

                    event_executing.state = EventExecutingState::Executing;

                    let Ok((mut caster_state, mut caster_minions)) =
                        caster_query.get_mut(caster_entity)
                    else {
                        error!("Cannot find caster state {:?}", caster_entity);
                        continue;
                    };

                    // Change state to casting
                    *caster_state = State::None;

                    commands.trigger(StateChange {
                        entity: caster_entity,
                        new_state: State::None,
                    });

                    let minion_id = ids.new_obj_id();

                    // Add to list of minions
                    caster_minions.ids.push(minion_id);

                    // Spawn weaker Shipwreck Zombie for Human Corpses (shipwreck sailors)
                    let zombie_type = if corpse_template.0 == "Human Corpse" {
                        "Shipwreck Zombie".to_string()
                    } else {
                        "Zombie".to_string()
                    };

                    let run_owner = ids
                        .get_player(map_event.obj_id)
                        .and_then(|owner| PlayerId(owner).is_human().then_some(owner))
                        .or_else(|| {
                            run_spawned_objs.iter().find_map(|(player_id, object_ids)| {
                                object_ids.contains(&map_event.obj_id).then_some(*player_id)
                            })
                        });
                    let event_type = GameEventType::SpawnNPC {
                        npc_type: zombie_type,
                        pos: *corpse_pos,
                        npc_id: Some(minion_id),
                        run_owner,
                    };

                    let event_id = ids.new_map_event_id();

                    let event = GameEvent {
                        event_id: event_id,
                        start_tick: game_tick.0,
                        run_tick: game_tick.0 + 1, // Add one game tick
                        event_type,
                    };

                    game_events.insert(event.event_id, event);

                    info!("Removing corpse {:?}", corpse_entity);
                    // Remove corpse
                    commands.entity(corpse_entity).despawn();
                    entity_map.remove_obj(*corpse_id);

                    // Remove obj observer event
                    commands.trigger(RemoveObj {
                        entity: corpse_entity,
                    });

                    commands.entity(caster_entity).insert(EventCompleted {
                        event_id: map_event.event_id,
                        event_type: "spell_raise_dead".to_string(),
                        at_tick: game_tick.0,
                        success: true,
                    });
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn spell_damage_event_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    mut query: Query<CombatSpellQuery>,
    mut map_events: ResMut<MapEvents>,
    _game_events: ResMut<GameEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut telemetry: Option<ResMut<SafeLogoutTelemetryState>>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::SpellDamageEvent { spell, target_id } => {
                    if object_belongs_to_protected_run(*target_id, &ids, &presence) {
                        if let (Some(player_id), Some(telemetry)) = (
                            protected_player_for_object(*target_id, &ids, &presence),
                            telemetry.as_deref_mut(),
                        ) {
                            telemetry.record_protected_target_rejection(player_id);
                            telemetry.record_protected_damage_block(player_id);
                        }
                        events_to_remove.push(*map_event_id);
                        if let Some(caster_entity) = entity_map.get_entity(map_event.obj_id) {
                            commands.trigger(StateChange {
                                entity: caster_entity,
                                new_state: State::None,
                            });
                            if let Ok(mut event_executing) =
                                event_executing_query.get_mut(caster_entity)
                            {
                                event_executing.state = EventExecutingState::Failed;
                            }
                        }
                        continue;
                    }
                    debug!("Processing SpellDamageEvent");
                    events_to_remove.push(*map_event_id);

                    let Some(caster_entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find caster from {:?}", map_event.obj_id);
                        continue;
                    };

                    let Some(target_entity) = entity_map.get_entity(*target_id) else {
                        error!("Cannot find caster from {:?}", target_id);
                        continue;
                    };

                    let Ok(mut event_executing) = event_executing_query.get_mut(caster_entity)
                    else {
                        error!(
                            "Cannot find event executing from entity: {:?}",
                            caster_entity
                        );
                        continue;
                    };

                    event_executing.state = EventExecutingState::Executing;

                    let entities = [caster_entity, target_entity];

                    let Ok([mut caster, mut target]) = query.get_many_mut(entities) else {
                        error!("Cannot find caster or target from entities {:?}", entities);
                        continue;
                    };

                    if Obj::is_dead(&caster.state) {
                        continue;
                    }

                    if let Some(errmsg) =
                        Combat::fortified_outbound_attack_error_from_spell(&caster, &target, true)
                    {
                        debug!("Spell damage blocked: {}", errmsg);
                        *caster.state = State::None;
                        commands.trigger(StateChange {
                            entity: caster_entity,
                            new_state: State::None,
                        });
                        commands.entity(caster_entity).insert(EventCompleted {
                            event_id: map_event.event_id,
                            event_type: "spell_damage".to_string(),
                            at_tick: game_tick.0,
                            success: false,
                        });
                        continue;
                    }

                    // Process spell damage
                    let damage = Combat::process_spell_damage(
                        &mut commands,
                        &game_tick,
                        spell.clone(),
                        &caster,
                        &mut target,
                    );

                    caster.last_combat_tick.0 = game_tick.0;
                    target.last_combat_tick.0 = game_tick.0;

                    let target_state_str = target.state.to_string();
                    let attack_type = match spell {
                        Spell::ShadowBolt => "Shadow Bolt".to_string(),
                        Spell::ArcaneBolt => "Arcane Bolt".to_string(),
                    };

                    let damage_event = VisibleEvent::DamageEvent {
                        target_id: target.id.0,
                        target_pos: target.pos.clone(),
                        attack_type,
                        damage,
                        combo: None,
                        state: target_state_str,
                        missed: false,
                    };

                    let damage_map_event = MapEvent {
                        event_id: Uuid::new_v4(),
                        obj_id: map_event.obj_id,
                        run_tick: game_tick.0 + 1,
                        event_type: damage_event.clone(),
                    };

                    visible_events.push(damage_map_event);

                    // Change state to casting
                    *caster.state = State::None;

                    commands.trigger(StateChange {
                        entity: caster_entity,
                        new_state: State::None,
                    });

                    // Add event in progress to caster
                    commands.entity(caster_entity).insert(EventCompleted {
                        event_id: map_event.event_id,
                        event_type: "spell_damage".to_string(),
                        at_tick: game_tick.0,
                        success: true,
                    });
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn broadcast_event_system(
    game_tick: Res<GameTick>,
    mut visible_events: ResMut<VisibleEvents>,
    mut map_events: ResMut<MapEvents>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            // Execute event
            match &map_event.event_type {
                VisibleEvent::DamageEvent { .. } => {
                    debug!("Processing DamageEvent");
                    events_to_remove.push(*map_event_id);
                    visible_events.push(map_event.clone());
                }
                VisibleEvent::SpeechEvent { .. } => {
                    debug!("Processing SpeechEvent");
                    events_to_remove.push(*map_event_id);
                    visible_events.push(map_event.clone());
                }
                VisibleEvent::SoundEvent { .. } => {
                    debug!("Processing SoundEvent");
                    events_to_remove.push(*map_event_id);
                    visible_events.push(map_event.clone());
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn effect_expired_event_system(
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    mut effect_query: Query<&mut Effects>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::EffectExpiredEvent { effect } => {
                    debug!("Processing EffectExpiredEvent {:?}", effect);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find entity from {:?}", map_event.obj_id);
                        continue;
                    };

                    if let Ok(mut effects) = effect_query.get_mut(entity) {
                        debug!("Effects on {:?}", map_event.obj_id);
                        effects.0.remove(effect);
                    }
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn cooldown_event_system(
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    mut event_executing_query: Query<&mut EventExecuting>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::CooldownEvent { duration } => {
                    debug!("Processing CooldownEvent {:?}", duration);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!("Cannot find corpse from {:?}", map_event.obj_id);
                        continue;
                    };

                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!(
                            "Missing EventExecuting component for entity {:?} (obj_id {})",
                            entity, map_event.obj_id
                        );
                        continue;
                    };
                    event_executing.state = EventExecutingState::Completed;
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn use_item_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    map: Res<Map>,
    resources: Res<Resources>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut plans: ResMut<Plans>,
    mut visible_events: ResMut<VisibleEvents>,
    mut map_events: ResMut<MapEvents>,
    crisis_state: Option<Res<SettlementCrisisState>>,
    mut balance_telemetry_state: Option<ResMut<CrisisBalanceTelemetryState>>,
    mut query: Query<ObjWithStatsQuery, Without<ClassStructure>>,
    mut structure_query: Query<
        (&Id, &PlayerId, &Position, &mut State, &mut Effects),
        With<ClassStructure>,
    >,
) {
    let mut events_to_add: Vec<MapEvent> = Vec::new();
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            // Execute event
            match &map_event.event_type {
                VisibleEvent::UseItemEvent {
                    item_id,
                    item_owner_id,
                } => {
                    if object_belongs_to_protected_run(*item_owner_id, &ids, &presence)
                        || object_belongs_to_protected_run(map_event.obj_id, &ids, &presence)
                    {
                        continue;
                    }
                    debug!("Processing UseItemEvent {:?}", item_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(*item_owner_id) else {
                        error!("Cannot find item owner entity from id: {:?}", item_owner_id);
                        continue;
                    };

                    let Ok(mut item_owner) = query.get_mut(entity) else {
                        error!("Query failed to find entity {:?}", entity);
                        continue;
                    };

                    let Some(item) = item_owner.inventory.get_by_id(*item_id) else {
                        error!("Cannot find item from id: {:?}", item_id);
                        continue;
                    };

                    let hp_before_use = item_owner.stats.hp;
                    let mut successful_healing_use = false;
                    match (item.class.as_str(), item.subclass.as_str()) {
                        (item::POTION, item::HEALTH) => {
                            if let Some(healing_attrval) = item.attrs.get(&item::AttrKey::Healing) {
                                debug!("Healing AttrVal: {:?}", healing_attrval);

                                let healing_value = match healing_attrval {
                                    item::AttrVal::Num(val) => *val as i32,
                                    _ => panic!("Invalid healing attribute value"),
                                };

                                if item_owner.stats.hp < item_owner.stats.base_hp {
                                    successful_healing_use = healing_value > 0;
                                    if (item_owner.stats.hp + healing_value)
                                        > item_owner.stats.base_hp
                                    {
                                        item_owner.stats.hp = item_owner.stats.base_hp;
                                    } else {
                                        item_owner.stats.hp += healing_value;
                                    }

                                    debug!(
                                        "Entity: {:?} Hp: {:?}",
                                        item_owner_id, item_owner.stats.hp
                                    );

                                    let packet = ResponsePacket::Stats {
                                        data: StatsData {
                                            id: *item_owner_id,
                                            hp: item_owner.stats.hp,
                                            base_hp: item_owner.stats.base_hp,
                                            stamina: 10000, // TODO missing stamina
                                            base_stamina: 10000,
                                            mana: item_owner.stats.mana.unwrap_or(0),
                                            base_mana: item_owner.stats.base_mana.unwrap_or(0),
                                            thirst: None,
                                            hunger: None,
                                            tiredness: None,
                                            effects: Vec::new(),
                                        },
                                    };

                                    send_to_client(item_owner.player_id.0, packet, &clients);
                                }
                            }

                            let cured_sickness =
                                explore_cure_for_item(&item.name, &item.class, &item.subclass)
                                    == Some(Effect::Sickness)
                                    && clear_effect_with_item(
                                        item_owner.player_id.0,
                                        item_owner.id.0,
                                        *item_owner.pos,
                                        &mut item_owner.effects,
                                        Effect::Sickness,
                                        &clients,
                                    );
                            if cured_sickness {
                                send_notice(
                                    item_owner.player_id.0,
                                    &format!("{} clears the sickness.", item.name),
                                    &clients,
                                );
                            }

                            // Health potions and poultices are consumables. The
                            // previous path healed without decrementing quantity,
                            // which allowed one starting potion to be reused for an
                            // entire assault. Preserve the no-op behavior at full
                            // health when no curable effect was removed.
                            if consume_successful_healing_item(
                                &mut item_owner.inventory,
                                item.id,
                                successful_healing_use || cured_sickness,
                            ) {
                                send_to_client(
                                    item_owner.player_id.0,
                                    ResponsePacket::InfoInventory {
                                        id: item.owner,
                                        cap: Obj::get_capacity(
                                            &item_owner.template.0,
                                            &templates.obj_templates,
                                        ),
                                        tw: item_owner.inventory.get_total_weight(),
                                        items: item_owner.inventory.get_packet(),
                                    },
                                    &clients,
                                );
                            }
                        }
                        (item::MEDICAL, "Bandage") => {
                            // A bandage is the budget heal: stops bleeding and
                            // closes minor wounds. Consumed when it did either;
                            // refusing to consume on a full-health, non-bleeding
                            // target keeps mis-clicks free.
                            let stopped_bleeding =
                                explore_cure_for_item(&item.name, &item.class, &item.subclass)
                                    == Some(Effect::Bleed)
                                    && clear_effect_with_item(
                                        item_owner.player_id.0,
                                        item_owner.id.0,
                                        *item_owner.pos,
                                        &mut item_owner.effects,
                                        Effect::Bleed,
                                        &clients,
                                    );

                            let missing_hp = item_owner.stats.base_hp - item_owner.stats.hp;
                            let healed = missing_hp.min(BANDAGE_HEAL_HP).max(0);

                            if stopped_bleeding || healed > 0 {
                                successful_healing_use = true;
                                if healed > 0 {
                                    item_owner.stats.hp += healed;

                                    let packet = ResponsePacket::Stats {
                                        data: StatsData {
                                            id: *item_owner_id,
                                            hp: item_owner.stats.hp,
                                            base_hp: item_owner.stats.base_hp,
                                            stamina: 10000, // TODO missing stamina
                                            base_stamina: 10000,
                                            mana: item_owner.stats.mana.unwrap_or(0),
                                            base_mana: item_owner.stats.base_mana.unwrap_or(0),
                                            thirst: None,
                                            hunger: None,
                                            tiredness: None,
                                            effects: Vec::new(),
                                        },
                                    };
                                    send_to_client(item_owner.player_id.0, packet, &clients);
                                }

                                item_owner.inventory.remove_quantity(item.id, 1);

                                let info_inventory_packet = ResponsePacket::InfoInventory {
                                    id: item.owner,
                                    cap: Obj::get_capacity(
                                        &item_owner.template.0,
                                        &templates.obj_templates,
                                    ),
                                    tw: item_owner.inventory.get_total_weight(),
                                    items: item_owner.inventory.get_packet(),
                                };

                                send_to_client(
                                    item_owner.player_id.0,
                                    info_inventory_packet,
                                    &clients,
                                );

                                let msg = match (stopped_bleeding, healed > 0) {
                                    (true, true) => format!(
                                        "{} stops the bleeding and closes the wound.",
                                        item.name
                                    ),
                                    (true, false) => {
                                        format!("{} stops the bleeding.", item.name)
                                    }
                                    _ => format!("{} closes the wound.", item.name),
                                };
                                send_notice(item_owner.player_id.0, &msg, &clients);
                            } else {
                                send_to_client(
                                    item_owner.player_id.0,
                                    ResponsePacket::Error {
                                        errmsg: "You have no wounds to bandage.".to_string(),
                                    },
                                    &clients,
                                );
                            }
                        }
                        (item::TORCH, _) => {
                            if explore_cure_for_item(&item.name, &item.class, &item.subclass)
                                == Some(Effect::Cursed)
                            {
                                if clear_effect_with_item(
                                    item_owner.player_id.0,
                                    item_owner.id.0,
                                    *item_owner.pos,
                                    &mut item_owner.effects,
                                    Effect::Cursed,
                                    &clients,
                                ) {
                                    send_notice(
                                        item_owner.player_id.0,
                                        &format!("{} burns away the curse.", item.name),
                                        &clients,
                                    );
                                } else {
                                    send_to_client(
                                        item_owner.player_id.0,
                                        ResponsePacket::Error {
                                            errmsg: "There is no curse to clear.".to_string(),
                                        },
                                        &clients,
                                    );
                                }
                            }
                        }
                        (item::DEED, _) => {
                            plans.add(item_owner.player_id.0, item.subclass, 0, 0);

                            item_owner.inventory.remove_item(item.id);

                            let inventory_items = item_owner.inventory.get_packet();

                            let info_inventory_packet: ResponsePacket =
                                ResponsePacket::InfoInventory {
                                    id: item.owner,
                                    cap: Obj::get_capacity(
                                        &item_owner.template.0,
                                        &templates.obj_templates,
                                    ),
                                    tw: item_owner.inventory.get_total_weight(),
                                    items: inventory_items,
                                };

                            send_to_client(item_owner.player_id.0, info_inventory_packet, &clients);

                            let packet = ResponsePacket::Error {
                                errmsg: format!("You have learnt how to build a {:?}", item.name),
                            };

                            send_to_client(item_owner.player_id.0, packet, &clients);
                        }
                        (_, BUCKET) => {
                            if item.name == BUCKET {
                                let is_near_water = Map::are_tile_types_nearby(
                                    item_owner.pos.clone(),
                                    vec![TileType::Ocean, TileType::River],
                                    &map,
                                );

                                if is_near_water {
                                    // Fill bucket with water
                                    item_owner.inventory.transform(
                                        item.id,
                                        WATER_BUCKET.to_string(),
                                        1,
                                        &templates.item_templates,
                                    );

                                    let inventory_items = item_owner.inventory.get_packet();

                                    let info_inventory_packet: ResponsePacket =
                                        ResponsePacket::InfoInventory {
                                            id: item.owner,
                                            cap: Obj::get_capacity(
                                                &item_owner.template.0,
                                                &templates.obj_templates,
                                            ),
                                            tw: item_owner.inventory.get_total_weight(),
                                            items: inventory_items,
                                        };

                                    send_to_client(
                                        item_owner.player_id.0,
                                        info_inventory_packet,
                                        &clients,
                                    );
                                } else {
                                    let packet = ResponsePacket::Error {
                                        errmsg: "You need to be near water to fill the bucket"
                                            .to_string(),
                                    };

                                    send_to_client(item_owner.player_id.0, packet, &clients);
                                }
                            } else if item.name == WATER_BUCKET {
                                let mut used_water_bucket = false;
                                // Get any adjacent structures to item owner
                                for (id, player_id, pos, mut state, mut effects) in
                                    structure_query.iter_mut()
                                {
                                    if *pos == *item_owner.pos {
                                        if player_id.0 == item_owner.player_id.0
                                            && *state == State::Burning
                                        {
                                            // Put out fire
                                            effects.0.remove(&Effect::Burning);

                                            item_owner.inventory.transform(
                                                item.id,
                                                BUCKET.to_string(),
                                                1,
                                                &templates.item_templates,
                                            );

                                            // Get entity and trigger state change
                                            if let Some(structure_entity) =
                                                entity_map.get_entity(id.0)
                                            {
                                                commands.trigger(StateChange {
                                                    entity: structure_entity,
                                                    new_state: State::None,
                                                });
                                            }

                                            let inventory_items = item_owner.inventory.get_packet();

                                            let info_inventory_packet: ResponsePacket =
                                                ResponsePacket::InfoInventory {
                                                    id: item.owner,
                                                    cap: Obj::get_capacity(
                                                        &item_owner.template.0,
                                                        &templates.obj_templates,
                                                    ),
                                                    tw: item_owner.inventory.get_total_weight(),
                                                    items: inventory_items,
                                                };

                                            send_to_client(
                                                item_owner.player_id.0,
                                                info_inventory_packet,
                                                &clients,
                                            );
                                            used_water_bucket = true;
                                            break;
                                        }
                                    }
                                }

                                if !used_water_bucket {
                                    let packet = ResponsePacket::Error {
                                        errmsg: "You need to be near a burning object to put out the fire".to_string(),
                                    };

                                    send_to_client(item_owner.player_id.0, packet, &clients);
                                }
                            }
                        }
                        (CONTAINER, WATERSKIN_SUBCLASS) => {
                            let is_near_fresh_water = Map::are_tile_types_nearby(
                                item_owner.pos.clone(),
                                vec![TileType::River],
                                &map,
                            );

                            // Check if tile has spring water resource
                            let has_spring_water = Resource::is_valid_type(
                                SPRING_WATER.to_string(),
                                item_owner.pos.clone(),
                                &resources,
                            );

                            if is_near_fresh_water || has_spring_water {
                                // Burn one empty waterskin and replace it with
                                // one filled. See the matching DrinkEvent
                                // comment in drink_eat_system for why we don't
                                // reuse transform() here (stack-aware + handles
                                // the remove-to-zero case correctly).
                                item_owner.inventory.remove_quantity(item.id, 1);
                                item_owner.inventory.new(
                                    ids.new_item_id(),
                                    WATERSKIN_FILLED.to_string(),
                                    1,
                                    &templates.item_templates,
                                );
                            } else {
                                let packet = ResponsePacket::Error {
                                    errmsg: "You need to be near fresh water or a spring water resource to fill the waterskin".to_string(),
                                };

                                send_to_client(item_owner.player_id.0, packet, &clients);
                                continue;
                            }

                            let inventory_items = item_owner.inventory.get_packet();

                            let info_inventory_packet: ResponsePacket =
                                ResponsePacket::InfoInventory {
                                    id: item.owner,
                                    cap: Obj::get_capacity(
                                        &item_owner.template.0,
                                        &templates.obj_templates,
                                    ),
                                    tw: item_owner.inventory.get_total_weight(),
                                    items: inventory_items,
                                };

                            send_to_client(item_owner.player_id.0, info_inventory_packet, &clients);

                            // Send notification to player
                            let packet = ResponsePacket::Notice {
                                noticemsg: format!(
                                    "{} has filled the waterskin",
                                    item_owner.name.0
                                ),
                                expiry: None,
                            };

                            send_to_client(item_owner.player_id.0, packet, &clients);
                        }
                        (_, FISHING_ROD) => {
                            let is_near_water = Map::are_tile_types_nearby(
                                item_owner.pos.clone(),
                                vec![TileType::Ocean, TileType::River],
                                &map,
                            );

                            info!("Is near water: {:?}", is_near_water);
                            if is_near_water {
                                info!(
                                    "Submitting Fishing event, visible events: {:?}",
                                    visible_events
                                );
                                commands.trigger(StateChange {
                                    entity: entity,
                                    new_state: State::Fishing,
                                });

                                let fishing_event = VisibleEvent::FishingEvent {
                                    obj_id: item_owner.id.0,
                                };

                                let fishing_map_event = MapEvent {
                                    event_id: Uuid::new_v4(),
                                    obj_id: item_owner.id.0,
                                    run_tick: game_tick.0 + 10,
                                    event_type: fishing_event,
                                };

                                events_to_add.push(fishing_map_event);
                                info!(
                                    "Done submitting Fishing event, visible events: {:?}",
                                    visible_events
                                );
                            } else {
                                let packet = ResponsePacket::Error {
                                    errmsg: "You need to be near water to fish".to_string(),
                                };

                                send_to_client(item_owner.player_id.0, packet, &clients);
                            }
                        }
                        (FOOD, _) => {
                            commands.trigger(StateChange {
                                entity: entity,
                                new_state: State::Eating,
                            });

                            let eating_event = VisibleEvent::EatEvent {
                                item_id: item.id,
                                obj_id: item_owner.id.0,
                            };

                            let eating_map_event = MapEvent {
                                event_id: Uuid::new_v4(),
                                obj_id: item_owner.id.0,
                                run_tick: game_tick.0 + 30,
                                event_type: eating_event,
                            };

                            events_to_add.push(eating_map_event);
                            info!(
                                "Done submittin Eating event, visible events: {:?}",
                                visible_events
                            );
                        }
                        (DRINK, _) => {
                            commands.trigger(StateChange {
                                entity: entity,
                                new_state: State::Drinking,
                            });

                            let drinking_event = VisibleEvent::DrinkEvent {
                                item_id: item.id,
                                obj_id: item_owner.id.0,
                            };

                            let drinking_map_event = MapEvent {
                                event_id: Uuid::new_v4(),
                                obj_id: item_owner.id.0,
                                run_tick: game_tick.0 + 30,
                                event_type: drinking_event,
                            };

                            events_to_add.push(drinking_map_event);
                            info!(
                                "Done submittin Drinking event, visible events: {:?}",
                                visible_events
                            );
                        }
                        (BEDROLL, _) => {
                            commands.trigger(StateChange {
                                entity: entity,
                                new_state: State::Sleeping,
                            });

                            let sleep_event = VisibleEvent::SleepEvent {
                                obj_id: item_owner.id.0,
                            };

                            let sleep_map_event = MapEvent {
                                event_id: Uuid::new_v4(),
                                obj_id: item_owner.id.0,
                                run_tick: game_tick.0 + 30,
                                event_type: sleep_event,
                            };

                            events_to_add.push(sleep_map_event);
                        }
                        _ => {}
                    }

                    let crisis_phase = crisis_state
                        .as_ref()
                        .and_then(|state| state.get(&item_owner.player_id.0))
                        .map(|crisis| crisis.phase);
                    if successful_healing_use && is_preparation_phase(crisis_phase) {
                        if let Some(telemetry_state) = balance_telemetry_state.as_deref_mut() {
                            telemetry_state
                                .entry(item_owner.player_id.0)
                                .or_default()
                                .preparation_actions
                                .record_healing_item_used_before_launch(*map_event_id, game_tick.0);
                        }
                    } else if successful_healing_use
                        && crisis_phase == Some(CrisisPhase::AssaultActive)
                    {
                        if let Some(telemetry_state) = balance_telemetry_state.as_deref_mut() {
                            telemetry_state
                                .entry(item_owner.player_id.0)
                                .or_default()
                                .engagement
                                .record_healing_use(
                                    item_owner.stats.hp.saturating_sub(hp_before_use),
                                );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    for event in events_to_add.iter() {
        map_events.insert(event.event_id, event.clone());
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

const HERO_AUTO_CONSUME_THRESHOLD: f32 = THIRSTY_SCORE;
const HERO_AUTO_CONSUME_TICKS: i32 = TICKS_PER_SEC * 3;
/// Tiredness at which an idle hero will bed down to sleep if a bedroll is on hand.
const HERO_AUTO_SLEEP_THRESHOLD: f32 = 75.0;

// Sleep heals up to this fraction of max hp, scaled by how tired the sleeper
// was (a fully exhausted sleeper gets the whole amount; a rested one gets
// ~nothing, so sleep cannot be spammed as a free heal).
const SLEEP_HEAL_MAX_FRACTION: f32 = 0.20;

/// Hp restored by a sleep, given how tired the sleeper was when lying down
/// (0.0 = fully rested, 1.0 = at the exhaustion ceiling).
pub fn sleep_heal_amount(base_hp: i32, tired_fraction: f32) -> i32 {
    (base_hp as f32 * SLEEP_HEAL_MAX_FRACTION * tired_fraction.clamp(0.0, 1.0)) as i32
}

// Flat heal applied by using a bandage — the cheap, craftable counterpart to
// the Health Potion's Healing attr.
const BANDAGE_HEAL_HP: i32 = 10;

fn consume_successful_healing_item(
    inventory: &mut Inventory,
    item_id: i32,
    successful: bool,
) -> bool {
    if !successful
        || !inventory
            .items
            .iter()
            .any(|item| item.id == item_id && item.quantity > 0)
    {
        return false;
    }
    inventory.remove_quantity(item_id, 1);
    true
}

fn hero_has_pending_map_event(obj_id: i32, map_events: &MapEvents) -> bool {
    map_events.iter().any(|(_, event)| event.obj_id == obj_id)
}

fn hero_auto_consume_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut map_events: ResMut<MapEvents>,
    presence: OptionalPlayerWorldPresence,
    mut hero_query: Query<
        (
            Entity,
            &Id,
            Option<&PlayerId>,
            &State,
            &Inventory,
            &Thirst,
            &Hunger,
            Option<&Tired>,
            Option<&LastCombatTick>,
            Option<&mut EventExecuting>,
        ),
        With<SubclassHero>,
    >,
) {
    for (
        entity,
        id,
        player_id,
        state,
        inventory,
        thirst,
        hunger,
        tired,
        last_combat_tick,
        event_executing,
    ) in hero_query.iter_mut()
    {
        if player_id
            .map(|player_id| is_owner_offline_protected(player_id, &presence))
            .unwrap_or(false)
        {
            continue;
        }
        if *state != State::None
            || last_combat_tick
                .map(|last_combat_tick| is_combat_locked(game_tick.0, last_combat_tick))
                .unwrap_or(false)
            || event_executing
                .as_ref()
                .map(|event_executing| event_executing.state == EventExecutingState::Executing)
                .unwrap_or(false)
            || hero_has_pending_map_event(id.0, &map_events)
        {
            continue;
        }

        let drink_item = (thirst.thirst >= HERO_AUTO_CONSUME_THRESHOLD)
            .then(|| inventory.get_by_class(DRINK.to_string()))
            .flatten();
        let food_item = (hunger.hunger >= HERO_AUTO_CONSUME_THRESHOLD)
            .then(|| inventory.get_food_to_eat())
            .flatten();

        let (next_state, event_type, event_name) = match (drink_item, food_item) {
            (Some(drink_item), Some(_food_item)) if thirst.thirst >= hunger.hunger => (
                State::Drinking,
                VisibleEvent::DrinkEvent {
                    item_id: drink_item.id,
                    obj_id: id.0,
                },
                "Drink",
            ),
            (Some(_drink_item), Some(food_item)) => (
                State::Eating,
                VisibleEvent::EatEvent {
                    item_id: food_item.id,
                    obj_id: id.0,
                },
                "Eat",
            ),
            (Some(drink_item), None) => (
                State::Drinking,
                VisibleEvent::DrinkEvent {
                    item_id: drink_item.id,
                    obj_id: id.0,
                },
                "Drink",
            ),
            (None, Some(food_item)) => (
                State::Eating,
                VisibleEvent::EatEvent {
                    item_id: food_item.id,
                    obj_id: id.0,
                },
                "Eat",
            ),
            (None, None) => {
                // No food or drink need — an idle, tired hero beds down to sleep
                // and fully recover, provided a bedroll is in their inventory.
                let tired_enough = tired
                    .map(|tired| tired.tired >= HERO_AUTO_SLEEP_THRESHOLD)
                    .unwrap_or(false);

                if tired_enough && inventory.has_by_class(BEDROLL.to_string()) {
                    (
                        State::Sleeping,
                        VisibleEvent::SleepEvent { obj_id: id.0 },
                        "Sleep",
                    )
                } else {
                    continue;
                }
            }
        };

        commands.trigger(StateChange {
            entity,
            new_state: next_state,
        });
        map_events.new(id.0, game_tick.0 + HERO_AUTO_CONSUME_TICKS, event_type);

        if let Some(mut event_executing) = event_executing {
            event_executing.event_type = event_name.to_string();
            event_executing.state = EventExecutingState::Executing;
        }
    }
}

fn drink_eat_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut visible_events: ResMut<VisibleEvents>,
    mut map_events: ResMut<MapEvents>,
    active_infos: ResMut<ActiveInfos>,
    mut needs_query: Query<(&mut Thirst, &mut Hunger, &mut Tired)>,
    mut query: Query<&mut Inventory>,
    state_query: Query<&State>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut stats_query: Query<&mut Stats>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::FindDrinkEvent { obj_id } => {
                    debug!("Processing FindDrinkEvent {:?}", obj_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        error!("Cannot find item owner entity from id: {:?}", obj_id);
                        continue;
                    };

                    info!("Setting EventExecutingState to Completed");
                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!(
                            "Missing EventExecuting component for entity {:?} (obj_id {})",
                            entity, map_event.obj_id
                        );
                        continue;
                    };
                    event_executing.state = EventExecutingState::Completed;
                }
                VisibleEvent::FindFoodEvent { obj_id } => {
                    debug!("Processing FindFoodEvent {:?}", obj_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        error!("Cannot find item owner entity from id: {:?}", obj_id);
                        continue;
                    };

                    info!("Setting EventExecutingState to Completed");
                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!(
                            "Missing EventExecuting component for entity {:?} (obj_id {})",
                            entity, map_event.obj_id
                        );
                        continue;
                    };
                    event_executing.state = EventExecutingState::Completed;
                }
                VisibleEvent::DrinkEvent { item_id, obj_id } => {
                    debug!("Processing DrinkEvent {:?}", item_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        error!("Cannot find item owner entity from id: {:?}", obj_id);
                        continue;
                    };

                    if !matches!(state_query.get(entity), Ok(state) if *state == State::Drinking) {
                        debug!("Skipping stale DrinkEvent for {:?}", obj_id);
                        continue;
                    }

                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!(
                            "Missing EventExecuting component for entity {:?} (obj_id {})",
                            entity, map_event.obj_id
                        );
                        continue;
                    };

                    let Ok(mut obj_inventory) = query.get_mut(entity) else {
                        error!("Query failed to find inventory entity {:?}", entity);
                        continue;
                    };

                    let Some(item) = obj_inventory.get_by_id(*item_id) else {
                        error!("Cannot find item from id: {:?}", item_id);
                        continue;
                    };

                    let Ok((mut thirst, hunger, tired)) = needs_query.get_mut(entity) else {
                        error!("Query failed to find needs entity {:?}", entity);
                        continue;
                    };

                    let Some(thirst_attrval) = item.attrs.get(&item::AttrKey::Thirst) else {
                        error!("Missing thirst attribute on item: {:?}", item);
                        continue;
                    };

                    let thirst_value = match thirst_attrval {
                        item::AttrVal::Num(val) => *val,
                        _ => panic!("Invalid thirst attribute value"),
                    };

                    thirst.thirst -= thirst_value;

                    // Burn one filled waterskin and replace it with one empty
                    // waterskin. We can't reuse `transform()` here because:
                    //   1. After remove_quantity decrements to 0, the source
                    //      item is swap_removed, so transform() can't find it.
                    //   2. With a stack > 1, transform() would overwrite the
                    //      whole remaining stack to the new variant, losing
                    //      the rest. Inventory::new merges with any existing
                    //      empty-waterskin stack via the name+attrs match in
                    //      Inventory::mergeable, so this works for both the
                    //      single-waterskin and stacked cases.
                    obj_inventory.remove_quantity(*item_id, 1);
                    obj_inventory.new(
                        ids.new_item_id(),
                        WATERSKIN_EMPTY.to_string(),
                        1,
                        &templates.item_templates,
                    );

                    if thirst.thirst <= 80.0 {
                        info!("Removing Dehydrated at tick: {:?}", game_tick.0);
                        commands.entity(entity).remove::<Dehydrated>();
                    }

                    // None visible state change
                    commands.trigger(StateChange {
                        entity: entity,
                        new_state: State::None,
                    });

                    info!("Setting EventExecutingState to Completed");
                    event_executing.state = EventExecutingState::Completed;

                    // TODO move this to a Changed<Thirst, Hunger, Tired> system
                    if ids.is_hero(map_event.obj_id) {
                        let info_thirst_update_packet: ResponsePacket =
                            ResponsePacket::InfoNeedsUpdate {
                                id: map_event.obj_id,
                                thirst: thirst.num_to_string(),
                                hunger: hunger.num_to_string(),
                                tiredness: tired.num_to_string(),
                            };

                        send_to_client(
                            ids.get_player(map_event.obj_id).unwrap(),
                            info_thirst_update_packet,
                            &clients,
                        );
                    }
                }
                VisibleEvent::EatEvent { item_id, obj_id } => {
                    debug!("Processing EatEvent {:?}", item_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        error!("Cannot find item owner entity from id: {:?}", obj_id);
                        continue;
                    };

                    if !matches!(state_query.get(entity), Ok(state) if *state == State::Eating) {
                        debug!("Skipping stale EatEvent for {:?}", obj_id);
                        continue;
                    }

                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!(
                            "Missing EventExecuting component for entity {:?} (obj_id {})",
                            entity, map_event.obj_id
                        );
                        continue;
                    };

                    let Ok(mut obj_inventory) = query.get_mut(entity) else {
                        error!("Query failed to find inventory entity {:?}", entity);
                        continue;
                    };

                    let Some(item) = obj_inventory.get_by_id(*item_id) else {
                        debug!("Failed to find item: {:?}", item_id);
                        continue;
                    };

                    let Ok((thirst, mut hunger, tired)) = needs_query.get_mut(entity) else {
                        error!("Query failed to find needs entity {:?}", entity);
                        continue;
                    };

                    let Some(feed_attrval) = item.attrs.get(&item::AttrKey::Feed) else {
                        error!("Missing feed attribute on item: {:?}", item);
                        continue;
                    };

                    let feed_value = match feed_attrval {
                        item::AttrVal::Num(val) => *val,
                        _ => panic!("Invalid feed attribute value"),
                    };

                    hunger.hunger -= feed_value;

                    let mut items_to_update = Vec::new();
                    let mut items_to_remove = Vec::new();

                    let updated_item = obj_inventory.remove_quantity(*item_id, 1);

                    if let Some(updated_item) = updated_item {
                        items_to_update.push(Item::to_packet(updated_item));
                    } else {
                        items_to_remove.push(*item_id);
                    }

                    if hunger.hunger <= 80.0 {
                        info!("Removing Starving at tick: {:?}", game_tick.0);
                        commands.entity(entity).remove::<Starving>();
                    }

                    // If item has FoodPoisoning attribute, trigger a FoodPoisoningEffect event
                    if let Some(food_poisoning_attrval) =
                        item.attrs.get(&item::AttrKey::FoodPoisoning)
                    {
                        commands.trigger(FoodPoisoningEffect {
                            entity: entity,
                            food_poisoning_attr: food_poisoning_attrval.clone(),
                        });
                    }

                    // T3.2 integration point: food buff effects.
                    // - A Healing attribute should restore HP one-shot here (mirrors the
                    //   POTION/HEALTH path at line ~6010).
                    // - Named effects (Hearty Meal, Bread Filling, Wine Cheer) defined in
                    //   effect_template.yaml should be applied via the Effects component.
                    // The needs_query in this handler doesn't have Stats or Effects; wiring
                    // requires either expanding the query or triggering a follow-up observer.

                    // None visible state change
                    commands.trigger(StateChange {
                        entity: entity,
                        new_state: State::None,
                    });

                    info!("Setting EventExecutingState to Completed");
                    event_executing.state = EventExecutingState::Completed;

                    if ids.is_hero(map_event.obj_id) {
                        let info_hunger_update_packet: ResponsePacket =
                            ResponsePacket::InfoNeedsUpdate {
                                id: map_event.obj_id,
                                thirst: thirst.num_to_string(),
                                hunger: hunger.num_to_string(),
                                tiredness: tired.num_to_string(),
                            };

                        send_to_client(
                            ids.get_player(map_event.obj_id).unwrap(),
                            info_hunger_update_packet,
                            &clients,
                        );
                    }
                }
                VisibleEvent::SleepEvent { obj_id } => {
                    debug!("Processing SleepEvent {:?}", obj_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        error!("Cannot find entity from id: {:?}", obj_id);
                        continue;
                    };

                    if !matches!(state_query.get(entity), Ok(state) if *state == State::Sleeping) {
                        debug!("Skipping stale SleepEvent for {:?}", obj_id);
                        continue;
                    }

                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!(
                            "Missing EventExecuting component for entity {:?} (obj_id {})",
                            entity, map_event.obj_id
                        );
                        continue;
                    };

                    // Update Tired, remove all tiredness
                    let Ok((thirst, hunger, mut tired)) = needs_query.get_mut(entity) else {
                        error!("Query failed to find tired entity {:?}", entity);
                        continue;
                    };

                    // How tired the sleeper was, before the rest wipes it —
                    // this scales the heal below.
                    let tired_fraction = (tired.tired / 100.0).clamp(0.0, 1.0);

                    tired.update(-100.0);

                    if tired.tired <= 80.0 {
                        commands.entity(entity).remove::<Exhausted>();
                    }

                    // Fully restore stamina on sleep, and knit wounds a little:
                    // up to SLEEP_HEAL_MAX_FRACTION of max hp for a sleeper who
                    // was fully exhausted, scaling down to ~nothing when rested,
                    // so spamming sleep is not a free infinite heal.
                    if let Ok(mut stats) = stats_query.get_mut(entity) {
                        if let Some(base_stamina) = stats.base_stamina {
                            stats.stamina = Some(base_stamina);
                        }
                        if let Some(base_mana) = stats.base_mana {
                            stats.mana = Some(base_mana);
                        }

                        let heal = sleep_heal_amount(stats.base_hp, tired_fraction);
                        if heal > 0 && stats.hp < stats.base_hp {
                            stats.hp = (stats.hp + heal).min(stats.base_hp);

                            if ids.is_hero(*obj_id) {
                                let packet = ResponsePacket::Stats {
                                    data: StatsData {
                                        id: *obj_id,
                                        hp: stats.hp,
                                        base_hp: stats.base_hp,
                                        stamina: stats.stamina.unwrap_or(0),
                                        base_stamina: stats.base_stamina.unwrap_or(0),
                                        mana: stats.mana.unwrap_or(0),
                                        base_mana: stats.base_mana.unwrap_or(0),
                                        thirst: None,
                                        hunger: None,
                                        tiredness: None,
                                        effects: Vec::new(),
                                    },
                                };
                                send_to_client(ids.get_player(*obj_id).unwrap(), packet, &clients);
                            }
                        }
                    }

                    // None visible state change
                    commands.trigger(StateChange {
                        entity: entity,
                        new_state: State::None,
                    });

                    info!("Setting EventExecutingState to Completed");
                    event_executing.state = EventExecutingState::Completed;

                    // TODO move this to a Changed<Thirst, Hunger, Tired> system
                    if ids.is_hero(map_event.obj_id) {
                        let info_tiredness_update_packet: ResponsePacket =
                            ResponsePacket::InfoNeedsUpdate {
                                id: map_event.obj_id,
                                thirst: thirst.num_to_string(),
                                hunger: hunger.num_to_string(),
                                tiredness: tired.num_to_string(),
                            };

                        send_to_client(
                            ids.get_player(map_event.obj_id).unwrap(),
                            info_tiredness_update_packet,
                            &clients,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn find_shelter_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    mut villager_query: Query<
        (&PlayerId, &Id, &Position, &mut ActiveShelter),
        With<SubclassVillager>,
    >,
    mut shelter_query: Query<
        (Entity, &PlayerId, &Id, &Position, &State, &mut Shelter),
        Without<SubclassVillager>,
    >,
    mut event_executing_query: Query<&mut EventExecuting>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            // Execute event
            match &map_event.event_type {
                VisibleEvent::FindShelterEvent { obj_id } => {
                    debug!("Processing FindShelterEvent {:?}", obj_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        error!("Cannot find entity from id: {:?}", obj_id);
                        continue;
                    };

                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!(
                            "Missing EventExecuting component for entity {:?} (obj_id {})",
                            entity, map_event.obj_id
                        );
                        continue;
                    };

                    let Ok((villager_player_id, villager_id, villager_pos, mut active_shelter)) =
                        villager_query.get_mut(entity)
                    else {
                        error!("Cannot find villager entity from id: {:?}", obj_id);
                        continue;
                    };

                    let mut closest_shelter = None;
                    let mut closest_distance = u32::MAX;

                    // Iterate through shelters and find the closest one that has space and is owned by the player
                    for (entity, player_id, id, pos, state, shelter) in shelter_query.iter_mut() {
                        if state.is_dead() {
                            continue;
                        }

                        if player_id.0 != villager_player_id.0 {
                            continue;
                        }

                        if shelter.max_residents > shelter.residents.len() as i32 {
                            let distance =
                                Map::distance((villager_pos.x, villager_pos.y), (pos.x, pos.y));

                            if distance < closest_distance {
                                closest_distance = distance;
                                closest_shelter = Some((id, shelter));
                            }
                        }
                    }

                    info!("Found closest shelter: {:?}", closest_shelter);
                    if let Some((id, mut shelter)) = closest_shelter {
                        shelter.residents.push(villager_id.0);
                        active_shelter.0 = id.0;
                    }

                    info!("Setting EventExecutingState to Completed");
                    event_executing.state = EventExecutingState::Completed;
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn steal_spoil_event_system(
    mut commands: Commands,
    mut map_events: ResMut<MapEvents>,
    game_tick: Res<GameTick>,
    mut visible_events: ResMut<VisibleEvents>,
    entity_map: Res<EntityObjMap>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    clients: Res<Clients>,
    active_infos: Res<ActiveInfos>,
    templates: Res<Templates>,
    mut inventory_query: Query<&mut Inventory>,
    mut effect_query: Query<&mut Effects>,
    mut state_query: Query<&mut State>,
    mut event_executing_query: Query<&mut EventExecuting>,
) {
    let mut events_to_remove = Vec::new();

    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            match &map_event.event_type {
                VisibleEvent::SpoilEvent {
                    target_id,
                    target_pos,
                    item_type,
                } => {
                    if object_belongs_to_protected_run(*target_id, &ids, &presence) {
                        events_to_remove.push(*map_event_id);
                        if let Some(entity) = entity_map.get_entity(map_event.obj_id) {
                            if let Ok(mut state) = state_query.get_mut(entity) {
                                *state = State::None;
                            }
                            if let Ok(mut event_executing) = event_executing_query.get_mut(entity) {
                                event_executing.state = EventExecutingState::Failed;
                            }
                        }
                        continue;
                    }
                    debug!("Processing SpoilEvent {:?}", target_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!(
                            "Cannot find item owner entity from id: {:?}",
                            map_event.obj_id
                        );
                        continue;
                    };

                    let Some(target_player_id) = ids.get_player(*target_id) else {
                        error!("Cannot find player id from id: {:?}", target_id);
                        continue;
                    };

                    let Some(target_entity) = entity_map.get_entity(*target_id) else {
                        error!("Cannot find target entity from id: {:?}", target_id);
                        continue;
                    };

                    let Ok(mut target_inventory) = inventory_query.get_mut(target_entity) else {
                        error!(
                            "Cannot find target inventory from entity {:?}",
                            target_entity
                        );
                        continue;
                    };

                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!("Cannot find event executing from entity: {:?}", entity);
                        continue;
                    };

                    event_executing.state = EventExecutingState::Executing;

                    //let mut items_to_update = Vec::new();
                    //let mut items_to_remove = Vec::new();

                    let item_quantity = -5;

                    target_inventory.update_quantity_by_class(item_type.to_string(), item_quantity);

                    let broadcast_spoil_event = VisibleEvent::BroadcastSpoilEvent {
                        target_id: *target_id,
                        target_pos: *target_pos,
                        item_type: item_type.to_string(),
                        item_quantity: item_quantity,
                    };

                    let broadcast_spoil_map_event = MapEvent {
                        event_id: Uuid::new_v4(),
                        obj_id: map_event.obj_id,
                        run_tick: game_tick.0 + 1,
                        event_type: broadcast_spoil_event.clone(),
                    };

                    visible_events.push(broadcast_spoil_map_event);
                }
                VisibleEvent::StealEvent {
                    target_id,
                    target_pos,
                    item_types,
                } => {
                    if object_belongs_to_protected_run(*target_id, &ids, &presence) {
                        events_to_remove.push(*map_event_id);
                        if let Some(entity) = entity_map.get_entity(map_event.obj_id) {
                            if let Ok(mut state) = state_query.get_mut(entity) {
                                *state = State::None;
                            }
                            if let Ok(mut event_executing) = event_executing_query.get_mut(entity) {
                                event_executing.state = EventExecutingState::Failed;
                            }
                        }
                        continue;
                    }
                    debug!("Processing StealEvent {:?}", target_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!(
                            "Cannot find item owner entity from id: {:?}",
                            map_event.obj_id
                        );
                        continue;
                    };

                    let Some(source_player_id) = ids.get_player(map_event.obj_id) else {
                        error!("Cannot find player id from id: {:?}", map_event.obj_id);
                        continue;
                    };

                    let Some(target_player_id) = ids.get_player(*target_id) else {
                        error!("Cannot find player id from id: {:?}", target_id);
                        continue;
                    };

                    let Some(target_entity) = entity_map.get_entity(*target_id) else {
                        error!("Cannot find target entity from id: {:?}", target_id);
                        continue;
                    };

                    let Ok([mut source_inventory, mut target_inventory]) =
                        inventory_query.get_many_mut([entity, target_entity])
                    else {
                        error!(
                            "Cannot find inventories from entities {:?}",
                            [entity, target_entity]
                        );
                        continue;
                    };

                    commands.entity(entity).remove::<EventInProgress>();

                    //let mut items_to_update = Vec::new();
                    //let mut items_to_remove = Vec::new();

                    // TODO: Make this dynamic based on the thief skills
                    let quantity = 5;

                    for item_type in item_types {
                        if let Some(item) = target_inventory.get_by_class(item_type.to_string()) {
                            Inventory::transfer_quantity(
                                item.id,
                                ids.new_item_id(),
                                &mut source_inventory,
                                &mut target_inventory,
                                quantity,
                                &templates.item_templates,
                            );

                            /*if let Some(item) = items.get_by_id(item.id) {
                                let item_packet = Item::to_packet(item);
                                items_to_update.push(item_packet);
                            } else {
                                items_to_remove.push(item.id);
                            }*/
                        }
                    }

                    let broadcast_steal_event = VisibleEvent::BroadcastStealEvent {
                        target_id: *target_id,
                        target_pos: *target_pos,
                    };

                    let broadcast_steal_map_event = MapEvent {
                        event_id: Uuid::new_v4(),
                        obj_id: map_event.obj_id,
                        run_tick: game_tick.0 + 1,
                        event_type: broadcast_steal_event.clone(),
                    };

                    visible_events.push(broadcast_steal_map_event);

                    commands.entity(entity).insert(EventCompleted {
                        event_id: map_event.event_id,
                        event_type: "steal".to_string(),
                        at_tick: game_tick.0,
                        success: true,
                    });
                }
                VisibleEvent::TorchEvent {
                    target_id,
                    target_pos,
                } => {
                    if object_belongs_to_protected_run(*target_id, &ids, &presence) {
                        events_to_remove.push(*map_event_id);
                        if let Some(entity) = entity_map.get_entity(map_event.obj_id) {
                            if let Ok(mut state) = state_query.get_mut(entity) {
                                *state = State::None;
                            }
                            if let Ok(mut event_executing) = event_executing_query.get_mut(entity) {
                                event_executing.state = EventExecutingState::Failed;
                            }
                        }
                        continue;
                    }
                    debug!("Processing TorchEvent {:?}", target_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                        error!(
                            "Cannot find item owner entity from id: {:?}",
                            map_event.obj_id
                        );
                        continue;
                    };

                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!("Cannot find event executing from entity: {:?}", entity);
                        continue;
                    };

                    event_executing.state = EventExecutingState::Executing;

                    let Some(target_entity) = entity_map.get_entity(*target_id) else {
                        error!(
                            "Entity: {:?} Cannot find target entity: {:?}",
                            entity, *target_id
                        );
                        commands.entity(entity).insert(EventCompleted {
                            event_id: map_event.event_id,
                            event_type: "torch".to_string(),
                            at_tick: game_tick.0,
                            success: true,
                        });
                        continue;
                    };

                    if let Ok(mut effects) = effect_query.get_mut(target_entity) {
                        // Check if the target is already burning
                        if effects.has(Effect::Burning) {
                            info!(
                                "Actor: {:?} Target {:?} is already burning, skipping torch event",
                                entity, *target_id
                            );
                            commands.entity(entity).insert(EventCompleted {
                                event_id: map_event.event_id,
                                event_type: "torch".to_string(),
                                at_tick: game_tick.0,
                                success: true,
                            });
                            continue;
                        }

                        effects.0.insert(Effect::Burning, (game_tick.0 + 1, 1.0, 1));
                    }

                    let Ok(mut state) = state_query.get_mut(target_entity) else {
                        error!(
                            "Actor: {:?} Cannot find state query for target: {:?}",
                            entity, *target_id
                        );
                        commands.entity(entity).insert(EventCompleted {
                            event_id: map_event.event_id,
                            event_type: "torch".to_string(),
                            at_tick: game_tick.0,
                            success: true,
                        });
                        continue;
                    };

                    *state = State::Burning;

                    commands.trigger(StateChange {
                        entity: target_entity,
                        new_state: State::Burning,
                    });

                    let broadcast_torch_event = VisibleEvent::BroadcastTorchEvent {
                        target_id: *target_id,
                        target_pos: *target_pos,
                    };

                    let broadcast_steal_map_event = MapEvent {
                        event_id: Uuid::new_v4(),
                        obj_id: map_event.obj_id,
                        run_tick: game_tick.0 + 1,
                        event_type: broadcast_torch_event.clone(),
                    };

                    visible_events.push(broadcast_steal_map_event);

                    commands.entity(entity).insert(EventCompleted {
                        event_id: map_event.event_id,
                        event_type: "torch".to_string(),
                        at_tick: game_tick.0,
                        success: true,
                    });
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn fishing_event_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut map_events: ResMut<MapEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    map: ResMut<Map>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut fisher_query: Query<FisherQuery>,
    mut event_executing_query: Query<&mut EventExecuting>,
) {
    let mut events_to_remove = Vec::new();
    for (map_event_id, map_event) in map_events.iter_mut() {
        if map_event.run_tick < game_tick.0 {
            if object_belongs_to_protected_run(map_event.obj_id, &ids, &presence) {
                continue;
            }
            match &map_event.event_type {
                VisibleEvent::FishingEvent { obj_id } => {
                    debug!("Processing FishingEvent {:?}", obj_id);
                    events_to_remove.push(*map_event_id);

                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        error!("Cannot find entity from id: {:?}", obj_id);
                        continue;
                    };

                    let Ok(mut fisher) = fisher_query.get_mut(entity) else {
                        error!("Cannot find obj entity from id: {:?}", obj_id);
                        continue;
                    };

                    let Ok(mut event_executing) = event_executing_query.get_mut(entity) else {
                        error!("Cannot find event executing from entity: {:?}", entity);
                        continue;
                    };

                    event_executing.state = EventExecutingState::Executing;

                    // Create state change event
                    commands.trigger(StateChange {
                        entity: entity,
                        new_state: State::None,
                    });

                    let nearby_tile_types = Map::get_nearby_tile_types(
                        fisher.pos.clone(),
                        vec![TileType::Ocean, TileType::River],
                        &map,
                    );

                    if nearby_tile_types.len() > 0 {
                        // TODO base the success on the skill of the fisher and the type of fish and the tile type

                        // Randomly select between carp and lake perch
                        let fish_type = if rand::thread_rng().gen_bool(0.5) {
                            CARP.to_string()
                        } else {
                            LAKE_PERCH.to_string()
                        };

                        // Create the fish item in the inventory
                        fisher.inventory.create(
                            ids.new_item_id(),
                            fisher.id.0,
                            fish_type,
                            1,
                            &templates.item_templates,
                        );

                        let inventory_items = fisher.inventory.get_packet();

                        let info_inventory_packet: ResponsePacket = ResponsePacket::InfoInventory {
                            id: fisher.id.0,
                            cap: Obj::get_capacity(&fisher.template.0, &templates.obj_templates),
                            tw: fisher.inventory.get_total_weight(),
                            items: inventory_items,
                        };

                        send_to_client(fisher.player_id.0, info_inventory_packet, &clients);

                        fisher
                            .skills
                            .update(Skill::Fishing, 100, &templates.skill_templates);
                    } else {
                        error!(
                            "Obj: {:?} is not near ocean or river tile, skipping fishing event",
                            obj_id
                        );
                    }
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        map_events.remove(event_id);
    }
}

fn visible_event_system(
    clients: Res<Clients>,
    mut visible_events: ResMut<VisibleEvents>,
    entity_map: Res<EntityObjMap>,
    obj_query: Query<ObjQuery>,
    observer_query: Query<ObjQueryVision>,
) {
    let mut all_change_events: HashMap<i32, HashSet<network::ChangeEvents>> = HashMap::new();
    let mut all_broadcast_events: HashMap<i32, HashSet<BroadcastEvents>> = HashMap::new();

    for map_event in visible_events.iter() {
        debug!("Checking if map_event is visible: {:?}", map_event);
        debug!("Entity map: {:?}", entity_map);
        if let Some(entity) = entity_map.get_entity(map_event.obj_id) {
            debug!("Entity: {:?}", entity);
            if let Ok(event_obj) = obj_query.get(entity) {
                debug!("Event obj: {:?}", event_obj);
                let network_obj = network::create_network_obj(&event_obj);

                for observer in observer_query.iter() {
                    // Skip corpse observers
                    if observer.class.is_corpse() {
                        continue;
                    }

                    // Skip npc dead observers, dead humans still get visibility
                    if observer.player_id.is_npc() && observer.state.is_dead() {
                        continue;
                    }

                    match &map_event.event_type {
                        VisibleEvent::NewObjEvent => {
                            let distance = Map::distance(
                                (event_obj.pos.x, event_obj.pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= distance {
                                let change_event = network::ChangeEvents::ObjCreate {
                                    event: "obj_create".to_string(),
                                    obj: network_obj.to_owned(),
                                };

                                // Notify observer
                                all_change_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(change_event);
                            }
                        }
                        VisibleEvent::MoveEvent { src, dst } => {
                            let src_distance = Map::dist(*observer.pos, *src);

                            if observer.viewshed.range >= src_distance {
                                let change_event = network::ChangeEvents::ObjMove {
                                    event: "obj_move".to_string(),
                                    obj: network_obj.to_owned(),
                                    src_x: src.x,
                                    src_y: src.y,
                                };

                                // Notify observer
                                all_change_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(change_event);
                            }

                            let dst_distance = Map::dist(*observer.pos, *dst);

                            if observer.viewshed.range >= dst_distance {
                                let change_event = network::ChangeEvents::ObjMove {
                                    event: "obj_move".to_string(),
                                    obj: network_obj.to_owned(),
                                    src_x: src.x,
                                    src_y: src.y,
                                };

                                all_change_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(change_event);
                            }
                        }
                        VisibleEvent::HideEvent => {
                            let distance = Map::dist(*event_obj.pos, *observer.pos);

                            if observer.viewshed.range >= distance {
                                let change_event = network::ChangeEvents::ObjDelete {
                                    event: "obj_delete".to_string(),
                                    obj_id: map_event.obj_id,
                                };

                                all_change_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(change_event);
                            }
                        }
                        VisibleEvent::DamageEvent {
                            target_id,
                            target_pos,
                            attack_type,
                            damage,
                            combo,
                            state,
                            missed,
                        } => {
                            let attacker_distance = Map::distance(
                                (event_obj.pos.x, event_obj.pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= attacker_distance {
                                let damage_event = BroadcastEvents::Damage {
                                    source_id: map_event.obj_id,
                                    target_id: *target_id,
                                    attack_type: attack_type.to_string(),
                                    dmg: *damage,
                                    state: state.to_string(),
                                    combo: combo.clone(),
                                    countered: None,
                                    missed: if *missed { Some(true) } else { None },
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(damage_event);
                            }

                            let target_distance = Map::distance(
                                (target_pos.x, target_pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= target_distance {
                                let damage_event = BroadcastEvents::Damage {
                                    source_id: map_event.obj_id,
                                    target_id: *target_id,
                                    attack_type: attack_type.to_string(),
                                    dmg: *damage,
                                    state: state.to_string(),
                                    combo: combo.clone(),
                                    countered: None,
                                    missed: if *missed { Some(true) } else { None },
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(damage_event);
                            }
                        }
                        VisibleEvent::BroadcastSpoilEvent {
                            target_id,
                            target_pos,
                            item_type,
                            item_quantity,
                        } => {
                            let distance = Map::distance(
                                (event_obj.pos.x, event_obj.pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= distance {
                                let spoil_event = BroadcastEvents::Spoil {
                                    source_id: map_event.obj_id,
                                    target_id: *target_id,
                                    itemtype: item_type.to_string(),
                                    itemquantity: *item_quantity,
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(spoil_event);
                            }

                            let target_distance = Map::distance(
                                (target_pos.x, target_pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= target_distance {
                                let spoil_event = BroadcastEvents::Spoil {
                                    source_id: map_event.obj_id,
                                    target_id: *target_id,
                                    itemtype: item_type.to_string(),
                                    itemquantity: *item_quantity,
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(spoil_event);
                            }
                        }
                        VisibleEvent::BroadcastStealEvent {
                            target_id,
                            target_pos,
                        } => {
                            let distance = Map::distance(
                                (event_obj.pos.x, event_obj.pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= distance {
                                let steal_event = BroadcastEvents::Steal {
                                    source_id: map_event.obj_id,
                                    target_id: *target_id,
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(steal_event);
                            }

                            let target_distance = Map::distance(
                                (target_pos.x, target_pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= target_distance {
                                let steal_event = BroadcastEvents::Steal {
                                    source_id: map_event.obj_id,
                                    target_id: *target_id,
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(steal_event);
                            }
                        }
                        VisibleEvent::BroadcastTorchEvent {
                            target_id,
                            target_pos,
                        } => {
                            let distance = Map::distance(
                                (event_obj.pos.x, event_obj.pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= distance {
                                let torch_event = BroadcastEvents::Torch {
                                    source_id: map_event.obj_id,
                                    target_id: *target_id,
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(torch_event);
                            }

                            let target_distance = Map::distance(
                                (target_pos.x, target_pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= target_distance {
                                let torch_event = BroadcastEvents::Torch {
                                    source_id: map_event.obj_id,
                                    target_id: *target_id,
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(torch_event);
                            }
                        }
                        VisibleEvent::SpeechEvent { speech, intensity } => {
                            let distance = Map::distance(
                                (event_obj.pos.x, event_obj.pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            debug!(
                                "SpeechEvent: {:?}, intensity: {:?}, distance: {:?}",
                                speech, intensity, distance
                            );
                            if *intensity >= distance as i32 {
                                let speech_event = BroadcastEvents::Speech {
                                    source: map_event.obj_id,
                                    speech: speech.clone(),
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(speech_event);
                            }
                        }
                        VisibleEvent::SoundEvent {
                            pos,
                            sound,
                            intensity,
                        } => {
                            let distance = Map::distance(
                                (event_obj.pos.x, event_obj.pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            debug!(
                                "SoundEvent: {:?}, intensity: {:?}, distance: {:?}",
                                sound, intensity, distance
                            );
                            if *intensity >= distance as i32 {
                                let sound_event = BroadcastEvents::Sound {
                                    x: pos.x,
                                    y: pos.y,
                                    sound: sound.clone(),
                                };

                                all_broadcast_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(sound_event);
                            }
                        }
                        VisibleEvent::StateChangeEvent { new_state } => {
                            let distance = Map::distance(
                                (event_obj.pos.x, event_obj.pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= distance {
                                let change_event = network::ChangeEvents::ObjUpdate {
                                    event: "obj_update".to_string(),
                                    obj_id: map_event.obj_id,
                                    attrs: vec![ObjAttr {
                                        attr: "state".to_string(),
                                        value: new_state.clone(),
                                    }],
                                };

                                all_change_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(change_event);
                            }
                        }
                        VisibleEvent::UpdateObjEvent { attrs } => {
                            let distance = Map::distance(
                                (event_obj.pos.x, event_obj.pos.y),
                                (observer.pos.x, observer.pos.y),
                            );

                            if observer.viewshed.range >= distance {
                                let change_event = network::ChangeEvents::ObjUpdate {
                                    event: "obj_update".to_string(),
                                    obj_id: map_event.obj_id,
                                    attrs: attrs
                                        .clone()
                                        .into_iter()
                                        .map(|(attr, value)| ObjAttr {
                                            attr: attr.to_string(),
                                            value: value.to_string(),
                                        })
                                        .collect(),
                                };

                                all_change_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(change_event);
                            }
                        }
                        VisibleEvent::UpdateObjPosEvent { src, dst } => {
                            let src_distance = Map::dist(*observer.pos, *src);

                            if observer.viewshed.range >= src_distance {
                                let change_event = network::ChangeEvents::ObjMove {
                                    event: "obj_move".to_string(),
                                    obj: network_obj.to_owned(),
                                    src_x: src.x,
                                    src_y: src.y,
                                };

                                // Notify observer
                                all_change_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(change_event);
                            }

                            let dst_distance = Map::dist(*observer.pos, *dst);

                            if observer.viewshed.range >= dst_distance {
                                let change_event = network::ChangeEvents::ObjMove {
                                    event: "obj_move".to_string(),
                                    obj: network_obj.to_owned(),
                                    src_x: src.x,
                                    src_y: src.y,
                                };

                                all_change_events
                                    .entry(observer.player_id.0)
                                    .or_default()
                                    .insert(change_event);
                            }
                        }
                        _ => {}
                    }
                }
            }
        } else {
            debug!(
                "No entity found for map_event, must be RemoveObjEvent: {:?}",
                map_event.event_type
            );
            for observer in observer_query.iter() {
                match &map_event.event_type {
                    VisibleEvent::RemoveObjEvent { pos } => {
                        let distance =
                            Map::distance((pos.x, pos.y), (observer.pos.x, observer.pos.y));

                        if observer.viewshed.range >= distance {
                            let change_event = network::ChangeEvents::ObjDelete {
                                event: "obj_delete".to_string(),
                                obj_id: map_event.obj_id,
                            };

                            all_change_events
                                .entry(observer.player_id.0)
                                .or_default()
                                .insert(change_event);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for (player_id, change_events) in all_change_events.iter_mut() {
        let changes_packet = ResponsePacket::PerceptionChanges {
            events: change_events.clone().into_iter().collect(),
        };

        for (_client_id, client) in clients.lock().unwrap().iter() {
            if client.player_id == *player_id {
                match client
                    .sender
                    .try_send(serde_json::to_string(&changes_packet).unwrap())
                {
                    Ok(_) => {}
                    Err(e) => {
                        error!(
                            "Could not send perception changes player_id={} error={:?}",
                            player_id, e
                        );
                    }
                }
            }
        }
    }

    // TODO reconsider these 3 loops
    for (player_id, broadcast_events) in all_broadcast_events.iter_mut() {
        for (_client_id, client) in clients.lock().unwrap().iter() {
            if client.player_id == *player_id {
                for broadcast_event in broadcast_events.iter() {
                    match client
                        .sender
                        .try_send(serde_json::to_string(&broadcast_event).unwrap())
                    {
                        Ok(_) => {}
                        Err(e) => {
                            error!(
                                "Could not send perception message player_id={} error={:?}",
                                player_id, e
                            );
                        }
                    }
                }
            }
        }
    }

    visible_events.clear();
}

fn watchtower_reveal_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut perception_updates: ResMut<PerceptionUpdates>,
    watchtower_query: Query<(&PlayerId, &Position, &Viewshed, &State), With<Watchtower>>,
    hidden_query: Query<(Entity, &PlayerId, &Position, &State, &Class), Without<Watchtower>>,
) {
    if game_tick.0 % TICKS_PER_SEC != 0 {
        return;
    }

    let mut revealed_entities = HashSet::new();
    let mut updated_players = HashSet::new();

    for (tower_player, tower_pos, tower_viewshed, tower_state) in watchtower_query.iter() {
        if !tower_state.is_active() {
            continue;
        }

        for (enemy_entity, enemy_player, enemy_pos, enemy_state, enemy_class) in hidden_query.iter()
        {
            if enemy_player.0 == tower_player.0
                || *enemy_state != State::Hiding
                || enemy_class.0 != CLASS_UNIT
                || Map::dist(*tower_pos, *enemy_pos) > tower_viewshed.range
            {
                continue;
            }

            if revealed_entities.insert(enemy_entity) {
                commands.trigger(StateChange {
                    entity: enemy_entity,
                    new_state: State::None,
                });
            }

            updated_players.insert(tower_player.0);
            updated_players.insert(enemy_player.0);
        }
    }

    for player_id in updated_players {
        perception_updates.insert((player_id, PerceptionUpdateType::UpdatePerception));
    }
}

// When a unit leaves stealth — combat breaking it, chasing a target, a
// watchtower revealing it, etc. — it was removed from observers' clients when
// it hid (HideEvent -> obj_delete). A plain state update can't bring a deleted
// object back, so the tick a unit stops hiding we refresh every connected
// player's perception, re-creating the now-visible unit for anyone in range.
// Tracking the previous hidden set catches the transition no matter how the
// state changed (observer trigger or a direct write like the chase move).
fn reveal_unhidden_system(
    clients: Res<Clients>,
    mut perception_updates: ResMut<PerceptionUpdates>,
    state_query: Query<(&Id, &State), With<SubclassNPC>>,
    mut hiding_ids: Local<HashSet<i32>>,
) {
    let mut still_hiding = HashSet::new();
    let mut revealed = false;

    for (id, state) in state_query.iter() {
        if *state == State::Hiding {
            still_hiding.insert(id.0);
        } else if hiding_ids.contains(&id.0) {
            revealed = true;
        }
    }

    // A despawned hider also drops out of `still_hiding`; refreshing perception
    // in that case is harmless.
    if revealed {
        for (_client_id, client) in clients.lock().unwrap().iter() {
            perception_updates.insert((client.player_id, PerceptionUpdateType::UpdatePerception));
        }
    }

    *hiding_ids = still_hiding;
}

fn perception_system(
    map: Res<Map>,
    game_tick: Res<GameTick>,
    mut explored_map: ResMut<ExploredMap>,
    weather_areas: Res<WeatherAreas>,
    clients: Res<Clients>,
    mut perception_updates: ResMut<PerceptionUpdates>,
    presence: Res<PlayerWorldPresenceState>,
    mut resume_login_sync: ResMut<ResumeLoginSyncState>,
    mut safe_logout_telemetry: ResMut<SafeLogoutTelemetryState>,
    observer_query: Query<ObjQueryVision>,
    obj_query: Query<ObjQuery>,
) {
    //let mut perceptions_to_send: HashMap<i32, HashSet<network::MapObj>> = HashMap::new();

    let mut observer_objs_map: HashMap<i32, HashSet<network::MapObj>> = HashMap::new();
    let mut visible_objs_map: HashMap<i32, HashSet<network::MapObj>> = HashMap::new();

    // Could not use HashSet here due to the trait `FromIterator<&std::collections::HashSet<(i32, i32)>>` is not implemented for `Vec<(i32, i32)>`
    let mut tiles_to_send: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
    let mut retry_updates = Vec::new();

    for (perception_player, perception_update_type) in perception_updates.iter() {
        let is_init_perception = matches!(
            perception_update_type,
            PerceptionUpdateType::InitPerception | PerceptionUpdateType::ResumeInitPerception(_)
        );
        let resume_connection_id = match perception_update_type {
            PerceptionUpdateType::ResumeInitPerception(connection_id) => Some(*connection_id),
            _ => None,
        };
        let mut init_perception_queued = false;
        let mut retry_resume_perception = resume_connection_id.is_some();
        for observer in observer_query.iter() {
            debug!("Observer: {:?}", observer);
            if observer.player_id.0 == *perception_player {
                for obj in obj_query.iter() {
                    if obj.id != observer.id {
                        let distance =
                            Map::distance((observer.pos.x, observer.pos.y), (obj.pos.x, obj.pos.y));

                        if observer.viewshed.range >= distance && obj.state.is_visible() {
                            debug!("Adding visible obj to percetion");
                            let (work_done, total_work, work_per_sec) =
                                network::build_progress_fields(obj.build_upgrade_state);

                            let visible_obj = network::MapObj {
                                id: obj.id.0,
                                player: obj.player_id.0,
                                x: obj.pos.x,
                                y: obj.pos.y,
                                name: obj.name.0.to_owned(),
                                template: obj.template.0.to_owned(),
                                class: obj.class.0.to_owned(),
                                subclass: obj.subclass.to_string(),
                                state: Obj::state_to_str(obj.state.to_owned()),
                                vision: None,
                                image: obj.misc.image.to_owned(),
                                hsl: obj.misc.hsl.to_owned(),
                                groups: obj.misc.groups.to_owned(),
                                work_done,
                                total_work,
                                work_per_sec,
                            };

                            visible_objs_map
                                .entry(*perception_player)
                                .or_default()
                                .insert(visible_obj);
                        }
                    }
                }

                let (work_done, total_work, work_per_sec) =
                    network::build_progress_fields(observer.build_upgrade_state);

                // Add observer to perception data
                let observer_obj = network::MapObj {
                    id: observer.id.0,
                    player: observer.player_id.0,
                    x: observer.pos.x,
                    y: observer.pos.y,
                    name: observer.name.0.to_owned(),
                    template: observer.template.0.to_owned(),
                    class: observer.class.0.to_owned(),
                    subclass: observer.subclass.to_string(),
                    state: Obj::state_to_str(observer.state.to_owned()),
                    vision: Some(observer.viewshed.range),
                    image: observer.misc.image.to_owned(),
                    hsl: observer.misc.hsl.to_owned(),
                    groups: observer.misc.groups.to_owned(),
                    work_done,
                    total_work,
                    work_per_sec,
                };

                observer_objs_map
                    .entry(*perception_player)
                    .or_default()
                    .insert(observer_obj);

                // Get visible tiles by player owned obj
                let visible_tiles_pos =
                    Map::range((observer.pos.x, observer.pos.y), observer.viewshed.range);

                debug!("Visible tiles: {:?}", visible_tiles_pos);
                debug!("Pre addition ExploredMap: {:?}", explored_map);

                // Add explored map
                match explored_map.entry(*perception_player) {
                    Entry::Occupied(mut o) => {
                        o.get_mut().extend(visible_tiles_pos.clone());
                        o.get_mut().sort_unstable();
                        o.get_mut().dedup();
                    }
                    Entry::Vacant(v) => {
                        v.insert(visible_tiles_pos.clone());
                    }
                };
                debug!("Post addition ExploredMap: {:?}", explored_map);

                tiles_to_send
                    .entry(*perception_player)
                    .or_default()
                    .extend(visible_tiles_pos);
            }
        }

        info!("Observer objs: {:?}", observer_objs_map);

        for (player_id, observer_objs) in observer_objs_map.iter_mut() {
            // Get player visible objs and convert to vec, if empty hashset is emptyreturn empty vec

            let mut visible_objs_list = Vec::new();

            if let Some(visible_objs) = visible_objs_map.get(player_id) {
                visible_objs_list = visible_objs.iter().cloned().collect();
            }

            // Gated behind NETWORK_DEBUG (same convention as message_broker_system)
            // so this per-tick perception spam stays off by default — it otherwise
            // floods stdout, e.g. when running many headless games.
            if std::env::var("NETWORK_DEBUG").is_ok() {
                println!(
                    "Perceptions to send player: {:?} observers: {:?}",
                    player_id, observer_objs
                );
            }

            let mut visible_tiles: &mut Vec<(i32, i32)> = tiles_to_send.get_mut(player_id).unwrap();

            dedup(&mut visible_tiles);

            let tiles = Map::pos_to_tiles(&visible_tiles.clone(), &map); // Used for network obj

            let weather_tiles = weather_areas.get_visible_weather_tiles(&visible_tiles.clone());

            let perception_data = network::PerceptionData {
                map: tiles,
                observers: observer_objs.clone().into_iter().collect(),
                visible_objs: visible_objs_list.clone(),
                weather: weather_tiles,
            };

            let init_for_requested_player = is_init_perception && *player_id == *perception_player;
            let perception_packet = if is_init_perception {
                ResponsePacket::InitPerception {
                    data: perception_data,
                }
            } else {
                ResponsePacket::NewPerception {
                    data: perception_data,
                }
            };

            if let Some(connection_id) = resume_connection_id {
                if !init_for_requested_player {
                    continue;
                }
                let Ok(serialized) = serde_json::to_string(&perception_packet) else {
                    error!(
                        "player_login_sync_serialization_failed player_id={} packet=init_perception",
                        player_id
                    );
                    retry_resume_perception = false;
                    resume_login_sync.remove(player_id);
                    continue;
                };
                match clients.try_send_current_bundle(*player_id, connection_id, vec![serialized]) {
                    Ok(()) => {
                        init_perception_queued = true;
                        retry_resume_perception = false;
                    }
                    Err(CurrentConnectionSendError::Full) => {
                        debug!(
                            "player_login_sync_deferred player_id={} game_tick={} reason=perception_channel_full",
                            player_id, game_tick.0
                        );
                    }
                    Err(error) => {
                        retry_resume_perception = false;
                        resume_login_sync.remove(player_id);
                        safe_logout_telemetry.record_stale_connection_event(*player_id);
                        info!(
                            "player_login_sync_stale_connection_rejected player_id={} game_tick={} stage=perception reason={:?}",
                            player_id, game_tick.0, error
                        );
                    }
                }
                continue;
            }

            for (client_id, client) in clients.lock().unwrap().iter() {
                if client.player_id == *player_id {
                    match client
                        .sender
                        .try_send(serde_json::to_string(&perception_packet).unwrap())
                    {
                        Ok(_) => {
                            if init_for_requested_player && *client_id == client.id {
                                init_perception_queued = true;
                            }
                        }
                        Err(e) => {
                            error!(
                                "Could not send perception message player_id={} error={:?}",
                                player_id, e
                            );
                        }
                    }
                }
            }
        }

        if let Some(connection_id) = resume_connection_id {
            if init_perception_queued {
                if let Some(progress) = resume_login_sync.get_mut(perception_player) {
                    if progress.connection_id == connection_id {
                        progress.perception_queued = true;
                    }
                }
            } else if retry_resume_perception
                && presence
                    .players
                    .get(perception_player)
                    .map(|record| {
                        record.resume_in_progress
                            && record.resume_connection_id == Some(connection_id)
                    })
                    .unwrap_or(false)
            {
                retry_updates.push((*perception_player, perception_update_type.clone()));
            }
        }
    }

    perception_updates.clear();
    perception_updates.extend(retry_updates);
}

/// Release-ready is published only after the exact reconnect has queued the
/// complete core login bundle, its current crisis snapshot, and its initial
/// perception. Final simulation release remains in the ordered PostUpdate
/// resume barrier and therefore occurs no earlier than the following update.
fn resume_login_sync_completion_system(
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut presence: ResMut<PlayerWorldPresenceState>,
    mut resume_login_sync: ResMut<ResumeLoginSyncState>,
    mut telemetry: ResMut<SafeLogoutTelemetryState>,
) {
    let pending = resume_login_sync
        .iter()
        .map(|(player_id, progress)| (*player_id, *progress))
        .collect::<Vec<_>>();

    for (player_id, progress) in pending {
        let record_matches = presence
            .players
            .get(&player_id)
            .map(|record| {
                record.resume_in_progress
                    && record.resume_connection_id == Some(progress.connection_id)
            })
            .unwrap_or(false);
        let connection_is_current =
            clients.is_current_connection(player_id, progress.connection_id);

        if !record_matches || !connection_is_current {
            resume_login_sync.remove(&player_id);
            if record_matches && !connection_is_current {
                telemetry.record_stale_connection_event(player_id);
                info!(
                    "player_login_sync_stale_connection_rejected player_id={} game_tick={} stage=completion",
                    player_id, game_tick.0
                );
            }
            continue;
        }

        if progress.crisis_status_queued
            && progress.perception_queued
            && mark_player_login_sync_complete(
                player_id,
                progress.connection_id,
                game_tick.0,
                &mut presence,
            )
        {
            resume_login_sync.remove(&player_id);
        }
    }
}

// On (re)login, evaluate the hero's proximity to monoliths and re-send its current
// sanctuary state. The sanctuary effect is normally applied/removed only as the hero
// crosses monolith ranges during movement, so a freshly connected client (and a hero
// that hasn't moved since spawn or a scene reload) would otherwise show no protection.
// Mirrors the apply/clear logic in move_event_completed_system to keep the Effects map
// and the Sanctuary/WeakSanctuary marker components in sync.
fn sanctuary_login_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    presence: Res<PlayerWorldPresenceState>,
    mut login_checks: ResMut<SanctuaryLoginChecks>,
    mut hero_query: Query<(Entity, &Id, &PlayerId, &Position, &mut Effects), With<SubclassHero>>,
    monolith_query: Query<(&Id, &Position), With<Monolith>>,
) {
    if login_checks.is_empty() {
        return;
    }

    // Process only entries whose hold has elapsed; keep the rest for a later tick.
    let now = game_tick.0;
    let mut due_player_ids: Vec<i32> = Vec::new();
    let mut pending: Vec<(i32, i32)> = Vec::new();

    for (player_id, due_tick) in login_checks.drain(..) {
        if is_player_offline_protected(player_id, &presence) {
            pending.push((player_id, now.saturating_add(1)));
        } else if due_tick <= now {
            due_player_ids.push(player_id);
        } else {
            pending.push((player_id, due_tick));
        }
    }

    login_checks.0 = pending;

    for player_id in due_player_ids {
        for (entity, hero_id, hero_player_id, hero_pos, mut effects) in hero_query.iter_mut() {
            if hero_player_id.0 != player_id {
                continue;
            }

            // Nearest monolith determines strength: strong wins over weak.
            let mut in_range_sanctuary: Option<(i32, Position)> = None;
            let mut in_range_weak_sanctuary: Option<(i32, Position)> = None;

            for (monolith_id, monolith_pos) in monolith_query.iter() {
                let distance = Map::dist(*hero_pos, *monolith_pos);

                if distance < SANCTUARY_RANGE {
                    in_range_sanctuary = Some((monolith_id.0, monolith_pos.clone()));
                    break;
                } else if distance < WEAK_SANCTUARY_RANGE && in_range_weak_sanctuary.is_none() {
                    in_range_weak_sanctuary = Some((monolith_id.0, monolith_pos.clone()));
                }
            }

            if let Some((monolith_id, monolith_pos)) = in_range_sanctuary {
                if !effects.has(Effect::Sanctuary) {
                    effects
                        .0
                        .insert(Effect::Sanctuary, (game_tick.0 + 1, 1.0, 1));
                }
                effects.0.remove(&Effect::WeakSanctuary);

                commands.entity(entity).insert(Sanctuary {
                    id: monolith_id,
                    pos: monolith_pos,
                });
                commands.entity(entity).remove::<WeakSanctuary>();

                let response_packet = ResponsePacket::GainedEffect {
                    id: hero_id.0,
                    x: hero_pos.x,
                    y: hero_pos.y,
                    effect: Effect::Sanctuary.to_str(),
                };

                send_to_client(player_id, response_packet, &clients);
            } else if let Some((monolith_id, monolith_pos)) = in_range_weak_sanctuary {
                if !effects.has(Effect::WeakSanctuary) {
                    effects
                        .0
                        .insert(Effect::WeakSanctuary, (game_tick.0 + 1, 1.0, 1));
                }
                effects.0.remove(&Effect::Sanctuary);

                commands.entity(entity).insert(WeakSanctuary {
                    id: monolith_id,
                    pos: monolith_pos,
                });
                commands.entity(entity).remove::<Sanctuary>();

                let response_packet = ResponsePacket::GainedEffect {
                    id: hero_id.0,
                    x: hero_pos.x,
                    y: hero_pos.y,
                    effect: Effect::WeakSanctuary.to_str(),
                };

                send_to_client(player_id, response_packet, &clients);
            } else {
                // Outside every monolith range: clear any stale sanctuary state.
                effects.0.remove(&Effect::Sanctuary);
                effects.0.remove(&Effect::WeakSanctuary);
                commands.entity(entity).remove::<Sanctuary>();
                commands.entity(entity).remove::<WeakSanctuary>();
            }

            break; // one hero per player
        }
    }
}

fn game_event_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    explored_map: Res<ExploredMap>,
    map: Res<Map>,
    templates: Res<Templates>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    mut query: Query<AllObjsQueryMut>,
    mut perception_updates: ResMut<PerceptionUpdates>,
    mut player_intro_state: ResMut<PlayerIntroState>,
    mut extras: GameEventExtras,
) {
    // Field access via `extras.<name>` below — bundled into one SystemParam
    // because Bevy systems are limited to 16 top-level params.
    let mut events_to_remove = Vec::new();
    // Events scheduled by handlers in this iteration. Collected here and
    // inserted after the loop to avoid invalidating the iterator.
    let mut events_to_insert: Vec<GameEvent> = Vec::new();

    for (event_id, game_event_type) in game_events.iter_mut() {
        if game_event_type.run_tick < game_tick.0 {
            let protected_intro_event = match &game_event_type.event_type {
                GameEventType::NecroEvent {
                    necromancer_id,
                    mausoleum_id,
                    ..
                } => necromancer_id
                    .iter()
                    .chain(mausoleum_id.iter())
                    .any(|object_id| {
                        initial_encounter_object_is_protected(
                            *object_id,
                            &extras.initial_encounter_state,
                            &extras.presence,
                        )
                    }),
                _ => false,
            };
            if protected_intro_event
                || game_event_belongs_to_protected_run(
                    &game_event_type.event_type,
                    &ids,
                    &extras.presence,
                )
            {
                continue;
            }
            // Execute event
            match &game_event_type.event_type {
                GameEventType::Login {
                    player_id,
                    connection_id,
                } => {
                    debug!("Processing Login: {:?}", player_id);
                    let connection_id = Uuid::from_u128(*connection_id);
                    if !clients.is_current_connection(*player_id, connection_id) {
                        events_to_remove.push(*event_id);
                        extras
                            .safe_logout_telemetry
                            .record_stale_connection_event(*player_id);
                        info!(
                            "player_login_sync_stale_connection_rejected player_id={} game_tick={}",
                            player_id, game_tick.0
                        );
                        continue;
                    }

                    let mut sync_packets = Vec::new();
                    if let Some(player_explored_map) = explored_map.get(&player_id) {
                        let explored_map_packet = ResponsePacket::ExploredMap {
                            tiles: Map::pos_to_tiles(player_explored_map, &map),
                        };
                        match serde_json::to_string(&explored_map_packet) {
                            Ok(packet) => sync_packets.push(packet),
                            Err(error) => {
                                events_to_remove.push(*event_id);
                                error!(
                                    "player_login_sync_serialization_failed player_id={} packet=explored_map error={:?}",
                                    player_id, error
                                );
                                continue;
                            }
                        }
                    }

                    debug!("Game tick: {:?}", game_tick.0);
                    let world_packet = ResponsePacket::World {
                        time_of_day: game_tick.time_of_day(),
                        day: game_tick.day(),
                    };
                    match serde_json::to_string(&world_packet) {
                        Ok(packet) => sync_packets.push(packet),
                        Err(error) => {
                            events_to_remove.push(*event_id);
                            error!(
                                "player_login_sync_serialization_failed player_id={} packet=world error={:?}",
                                player_id, error
                            );
                            continue;
                        }
                    }

                    match clients.try_send_current_bundle(*player_id, connection_id, sync_packets) {
                        Ok(()) => {}
                        Err(CurrentConnectionSendError::Full) => {
                            debug!(
                                "player_login_sync_deferred player_id={} game_tick={} reason=channel_full",
                                player_id, game_tick.0
                            );
                            continue;
                        }
                        Err(error) => {
                            events_to_remove.push(*event_id);
                            extras
                                .safe_logout_telemetry
                                .record_stale_connection_event(*player_id);
                            info!(
                                "player_login_sync_stale_connection_rejected player_id={} game_tick={} reason={:?}",
                                player_id, game_tick.0, error
                            );
                            continue;
                        }
                    }
                    events_to_remove.push(*event_id);

                    // Crisis status uses this established delayed login point
                    // so the first snapshot follows the Login packet and is
                    // deduplicated per authenticated connection.
                    extras.crisis_status_login_sync.insert(*player_id);

                    // Re-send the hero's current sanctuary state to the client, which
                    // otherwise only learns it from movement transitions. Held a few
                    // ticks so it lands after the login perception that sets the hero id.
                    extras
                        .sanctuary_login_checks
                        .push((*player_id, game_tick.0 + 10));

                    let resuming = extras
                        .presence
                        .players
                        .get(player_id)
                        .map(|record| {
                            record.resume_in_progress
                                && record.resume_connection_id == Some(connection_id)
                        })
                        .unwrap_or(false);
                    if resuming {
                        extras.resume_login_sync.insert(
                            *player_id,
                            ResumeLoginSyncProgress {
                                connection_id,
                                crisis_status_queued: false,
                                perception_queued: false,
                            },
                        );
                        perception_updates.insert((
                            *player_id,
                            PerceptionUpdateType::ResumeInitPerception(connection_id),
                        ));
                    } else {
                        perception_updates
                            .insert((*player_id, PerceptionUpdateType::InitPerception));
                    }
                }
                GameEventType::PlayerNotice {
                    player_id,
                    message,
                    expiry,
                } => {
                    events_to_remove.push(*event_id);

                    let packet = ResponsePacket::Notice {
                        noticemsg: message.to_string(),
                        expiry: *expiry,
                    };
                    send_to_client(*player_id, packet, &clients);
                }
                GameEventType::MerchantArrival {
                    merchant_id,
                    player_id,
                } => {
                    events_to_remove.push(*event_id);

                    let Some(entity) = entity_map.get_entity(*merchant_id) else {
                        error!("MerchantArrival: cannot find entity for {:?}", merchant_id);
                        continue;
                    };

                    // Kick off the sail. The merchant_sailing_system moves the
                    // ship one tile at a time toward `landing_at`. When it gets
                    // there, merchant_arrival_system handles the on-arrival
                    // logic (restock, notice, speech, schedule next events).
                    //
                    // Note: sailing rather than teleporting is the visible
                    // hook the player sees — the ship comes in from the
                    // empire side of the map. Stays in transit for a few
                    // game-seconds depending on map distance.
                    if let Ok(mut merchant) = extras.merchant_query.get_mut(entity) {
                        merchant.sail_state = MerchantSailState::SailingToLanding;
                    } else {
                        error!(
                            "MerchantArrival: cannot find Merchant component for {:?}",
                            merchant_id
                        );
                    }

                    info!(
                        "MerchantArrival: merchant {} setting sail toward landing for player {}",
                        merchant_id, player_id
                    );
                }
                GameEventType::MerchantLeavingSoon {
                    merchant_id,
                    player_id,
                } => {
                    events_to_remove.push(*event_id);

                    let notice = ResponsePacket::Notice {
                        noticemsg: "The traveling merchant is preparing to sail. One minute until they depart.".to_string(),
                        expiry: Some(2000),
                    };
                    send_to_client(*player_id, notice, &clients);

                    map_events.new(
                        *merchant_id,
                        game_tick.0 + 4,
                        VisibleEvent::SpeechEvent {
                            speech: "I sail with the next tide! Speak now, or wait for my return."
                                .to_string(),
                            intensity: 4,
                        },
                    );
                }
                GameEventType::MerchantDeparture {
                    merchant_id,
                    player_id,
                } => {
                    events_to_remove.push(*event_id);

                    map_events.new(
                        *merchant_id,
                        game_tick.0 + 4,
                        VisibleEvent::SpeechEvent {
                            speech: "Until next tide!".to_string(),
                            intensity: 3,
                        },
                    );

                    // Kick off the return sail. Movement system drives it
                    // back toward `trade_port`; merchant_arrival_system
                    // schedules the next MerchantArrival once it arrives.
                    if let Some(entity) = entity_map.get_entity(*merchant_id) {
                        if let Ok(mut merchant) = extras.merchant_query.get_mut(entity) {
                            merchant.sail_state = MerchantSailState::SailingToEmpire;
                        }
                    }

                    info!(
                        "MerchantDeparture: merchant {} setting sail back to the empire for player {}",
                        merchant_id, player_id
                    );
                }
                GameEventType::SpawnNPC {
                    npc_type,
                    pos,
                    npc_id,
                    run_owner,
                } => {
                    debug!("Processing SpawnNPC");
                    events_to_remove.push(*event_id);

                    let result;

                    if let Some(npc_id) = npc_id {
                        info!("Spawning NPC with id: {:?}", npc_id);
                        result = Encounter::spawn_npc_with_id(
                            *npc_id,
                            NPC_PLAYER_ID,
                            *pos,
                            npc_type.to_string(),
                            &mut commands,
                            &mut ids,
                            &mut entity_map,
                            &templates,
                        );
                    } else {
                        result = Encounter::spawn_npc(
                            1000,
                            *pos,
                            npc_type.to_string(),
                            &mut commands,
                            &mut ids,
                            &mut entity_map,
                            &templates,
                        );
                    }

                    let (entity, npc_id, _player_id, _pos) = result;

                    if let Some(player_id) = run_owner {
                        extras
                            .run_spawned_objs
                            .entry(*player_id)
                            .or_default()
                            .push(npc_id.0);
                    }

                    // Create a new object event
                    commands.trigger(NewObj { entity: entity });
                }
                GameEventType::UpdatePos { obj_id, pos } => {
                    debug!("Processing UpdatePos: {:?} {:?}", obj_id, pos);
                    events_to_remove.push(*event_id);

                    // Update object position
                    debug!("entity_map: {:?}", entity_map);
                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        error!("Cannot find entity from id: {:?}", obj_id);
                        continue;
                    };

                    let Ok(mut obj) = query.get_mut(entity) else {
                        error!("Query failed to find entity {:?}", entity);
                        continue;
                    };

                    // Store src for event visibility check
                    let src_pos = *obj.pos;

                    // Update position
                    *obj.pos = pos.clone();

                    visible_events.new(
                        *obj_id,
                        game_tick.0 + 1,
                        VisibleEvent::UpdateObjPosEvent {
                            src: src_pos,
                            dst: *pos,
                        },
                    );
                }
                GameEventType::NecroEvent {
                    necromancer_id,
                    mausoleum_id,
                    spawn_anchor,
                    corpse_anchor,
                    home,
                } => {
                    debug!("Processing NecroEvent");
                    events_to_remove.push(*event_id);

                    let occupied_positions: HashSet<Position> = query
                        .iter()
                        .filter(|obj| Some(obj.id.0) != *necromancer_id)
                        .map(|obj| *obj.pos)
                        .collect();

                    let Some(spawn_pos) = resolve_necromancer_spawn_pos(
                        *spawn_anchor,
                        &occupied_positions,
                        &map,
                        NECROMANCER_SPAWN_SEARCH_RADIUS,
                    ) else {
                        warn!(
                            "NecroEvent skipped: no open spawn tile near {:?}",
                            spawn_anchor
                        );
                        continue;
                    };

                    let (necro_entity, npc_id) = if let Some(necromancer_id) = necromancer_id {
                        if let Some(necro_entity) = entity_map.get_entity(*necromancer_id) {
                            if let Ok(mut necro) = query.get_mut(necro_entity) {
                                *necro.pos = spawn_pos;
                                *necro.state = State::None;
                                Encounter::activate_necromancer_hunting_corpse(
                                    necro_entity,
                                    *home,
                                    *corpse_anchor,
                                    &mut commands,
                                );
                                (necro_entity, *necro.id)
                            } else {
                                warn!(
                                    "NecroEvent dormant obj {:?} missing required components; spawning replacement",
                                    necromancer_id
                                );
                                let (entity, id, _player_id, _pos) =
                                    Encounter::spawn_necromancer_hunting_corpse(
                                        NPC_PLAYER_ID,
                                        spawn_pos,
                                        *home,
                                        *corpse_anchor,
                                        &mut commands,
                                        &mut ids,
                                        &mut entity_map,
                                        &templates,
                                    );
                                (entity, id)
                            }
                        } else {
                            warn!(
                                "NecroEvent dormant obj {:?} missing from entity map; spawning replacement",
                                necromancer_id
                            );
                            let (entity, id, _player_id, _pos) =
                                Encounter::spawn_necromancer_hunting_corpse(
                                    NPC_PLAYER_ID,
                                    spawn_pos,
                                    *home,
                                    *corpse_anchor,
                                    &mut commands,
                                    &mut ids,
                                    &mut entity_map,
                                    &templates,
                                );
                            (entity, id)
                        }
                    } else {
                        let (entity, id, _player_id, _pos) =
                            Encounter::spawn_necromancer_hunting_corpse(
                                NPC_PLAYER_ID,
                                spawn_pos,
                                *home,
                                *corpse_anchor,
                                &mut commands,
                                &mut ids,
                                &mut entity_map,
                                &templates,
                            );
                        (entity, id)
                    };

                    // Create a new object event now that the hidden necromancer is visible.
                    commands.trigger(NewObj {
                        entity: necro_entity,
                    });

                    // Reveal the hidden Mausoleum together with the necromancer.
                    if let Some(mausoleum_id) = mausoleum_id {
                        if let Some(mausoleum_entity) = entity_map.get_entity(*mausoleum_id) {
                            if let Ok(mut mausoleum) = query.get_mut(mausoleum_entity) {
                                *mausoleum.state = State::None;
                                commands.trigger(NewObj {
                                    entity: mausoleum_entity,
                                });
                            } else {
                                warn!(
                                    "NecroEvent mausoleum obj {:?} missing required components",
                                    mausoleum_id
                                );
                            }
                        } else {
                            warn!(
                                "NecroEvent mausoleum obj {:?} missing from entity map",
                                mausoleum_id
                            );
                        }
                    }

                    // Necromancer announces arrival
                    let speech_event = VisibleEvent::SpeechEvent {
                        speech: "Rise... serve me...".to_string(),
                        intensity: 2,
                    };

                    map_events.new(npc_id.0, game_tick.0 + 10, speech_event);
                }

                GameEventType::SpawnVillager { pos, player_id } => {
                    debug!("Processing SpawnVillager event");
                    events_to_remove.push(*event_id);

                    if player_intro_state
                        .get(player_id)
                        .map(|entry| entry.villager_spawned)
                        .unwrap_or(false)
                    {
                        continue;
                    }

                    let villager_hsl = extras
                        .assigned_start_locations
                        .get(player_id)
                        .map(|location| location.hsl.clone())
                        .unwrap_or_default();

                    let (villager_entity, villager_id) = Encounter::spawn_villager(
                        *player_id,
                        *pos,
                        villager_hsl,
                        &mut commands,
                        &mut ids,
                        &mut entity_map,
                        &templates,
                        &game_tick,
                    );

                    if let Some(intro_entry) = player_intro_state.get_mut(player_id) {
                        intro_entry.villager_spawned = true;
                    }

                    commands.trigger(NewObj {
                        entity: villager_entity,
                    });

                    let speech_event = VisibleEvent::SpeechEvent {
                        speech: "Thank the gods! I thought I was going to die in that wreck..."
                            .to_string(),
                        intensity: 3,
                    };
                    map_events.new(villager_id.0, game_tick.0 + 10, speech_event);

                    // Villager teaches the first dedicated lookout plan.
                    extras.plans.add(*player_id, "Watchtower".to_string(), 0, 0);

                    let discovery_packet = ResponsePacket::DiscoveryEvent {
                        version: 1,
                        discovery_type: "plan".to_string(),
                        title: "Watchtower plan shared".to_string(),
                        unlock_source: "Rescued villager".to_string(),
                        location: Some(format!("{},{}", pos.x, pos.y)),
                        result: "A Watchtower turns warning time into safety before night pressure reaches camp.".to_string(),
                    };
                    send_to_client(*player_id, discovery_packet, &clients);

                    let plan_speech = VisibleEvent::SpeechEvent {
                        speech: "I can show you how to raise a watchtower. Seeing trouble early keeps a camp alive."
                            .to_string(),
                        intensity: 3,
                    };
                    map_events.new(villager_id.0, game_tick.0 + 50, plan_speech);

                    // Trigger the first traveling-merchant visit. The merchant
                    // entity was spawned offshore at empire_pos in player_setup;
                    // this event flips its sail_state and the sailing system
                    // brings it tile-by-tile toward the landing position.
                    if let Some(entry) = extras.initial_encounter_state.get(player_id) {
                        if entry.merchant_id != 0 {
                            events_to_insert.push(GameEvent {
                                event_id: ids.new_map_event_id(),
                                start_tick: game_tick.0,
                                run_tick: game_tick.0 + MERCHANT_FIRST_ARRIVAL_DELAY,
                                event_type: GameEventType::MerchantArrival {
                                    merchant_id: entry.merchant_id,
                                    player_id: *player_id,
                                },
                            });
                        }

                        // Reveal and activate the hidden necromancer + mausoleum
                        // 5 minutes after the villager is rescued. This handler runs
                        // exactly once per player (guarded by villager_spawned above).
                        events_to_insert.push(GameEvent {
                            event_id: ids.new_map_event_id(),
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + NECRO_EVENT_DELAY_AFTER_RESCUE,
                            event_type: GameEventType::NecroEvent {
                                necromancer_id: Some(entry.necromancer_id),
                                mausoleum_id: Some(entry.mausoleum_id),
                                spawn_anchor: entry.necro_spawn_anchor,
                                corpse_anchor: entry.necro_corpse_anchor,
                                home: entry.necro_home,
                            },
                        });
                    }
                }

                GameEventType::CancelAllMapEvents { obj_id } => {
                    debug!("Processing CancelEventsByObjId: {:?}", obj_id);
                    events_to_remove.push(*event_id);

                    let mut events_to_cancel = Vec::new();

                    for (_map_event_id, map_event) in map_events.iter() {
                        if map_event.obj_id == *obj_id {
                            // TODO: Check if event is cancellable
                            events_to_cancel.push(map_event.clone());
                        }
                    }

                    debug!("Canceling map events: {:?}", events_to_cancel);
                    for map_event in events_to_cancel.iter() {
                        match map_event.event_type {
                            _ => {
                                let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                                    error!(
                                        "Cannot find item owner entity from id: {:?}",
                                        map_event.obj_id
                                    );
                                    continue;
                                };

                                let Ok(mut obj) = query.get_mut(entity) else {
                                    error!("Query failed to find entity {:?}", entity);
                                    continue;
                                };

                                debug!("Cancel event - reseting obj state to none.");
                                *obj.state = State::None;

                                debug!(
                                    "Cancel event - removing EventInProgress for entity: {:?}",
                                    obj.entity
                                );
                                commands.entity(obj.entity).remove::<EventInProgress>();

                                /*debug!("Cancel event - removing drink, eat, sleep completed events {:?}", map_event.entity_id);
                                commands
                                    .entity(map_event.entity_id)
                                    .remove::<DrinkEventCompleted>()
                                    .remove::<EatEventCompleted>()
                                    .remove::<SleepEventCompleted>();  */

                                // None visible state change
                                commands.trigger(StateChange {
                                    entity: obj.entity,
                                    new_state: State::None,
                                });
                            }
                        }
                    }

                    debug!("Removing map events {:?} from queue", events_to_cancel);
                    for event in events_to_cancel.iter() {
                        map_events.remove(&event.event_id);
                    }
                }
                GameEventType::CancelAllowedMapEvents { obj_id } => {
                    debug!("Processing CancelAllowedEvents: {:?}", obj_id);
                    events_to_remove.push(*event_id);

                    let mut events_to_cancel = Vec::new();

                    for (_map_event_id, map_event) in map_events.iter() {
                        if map_event.obj_id == *obj_id {
                            match map_event.event_type {
                                VisibleEvent::MoveEvent { .. }
                                | VisibleEvent::GatherEvent { .. }
                                | VisibleEvent::RefineEvent { .. }
                                | VisibleEvent::OperateEvent { .. }
                                | VisibleEvent::CraftEvent { .. }
                                | VisibleEvent::SurveyEvent
                                | VisibleEvent::ProspectEvent
                                | VisibleEvent::ExploreEvent
                                | VisibleEvent::InvestigateEvent { .. }
                                | VisibleEvent::UseItemEvent { .. } => {
                                    events_to_cancel.push(map_event.clone());
                                }
                                _ => {}
                            }
                        }
                    }

                    debug!("Canceling map events: {:?}", events_to_cancel);
                    for map_event in events_to_cancel.iter() {
                        match map_event.event_type {
                            _ => {
                                let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                                    error!(
                                        "Cannot find item owner entity from id: {:?}",
                                        map_event.obj_id
                                    );
                                    continue;
                                };

                                let Ok(mut obj) = query.get_mut(entity) else {
                                    error!("Query failed to find entity {:?}", entity);
                                    continue;
                                };

                                debug!("Cancel event - reseting obj state to none.");
                                *obj.state = State::None;

                                debug!(
                                    "Cancel event - removing EventInProgress for entity: {:?}",
                                    obj.entity
                                );
                                commands.entity(obj.entity).remove::<EventInProgress>();

                                /*debug!("Cancel event - removing drink, eat, sleep completed events {:?}", map_event.entity_id);
                                commands
                                    .entity(map_event.entity_id)
                                    .remove::<DrinkEventCompleted>()
                                    .remove::<EatEventCompleted>()
                                    .remove::<SleepEventCompleted>();  */

                                // None visible state change
                                commands.trigger(StateChange {
                                    entity: obj.entity,
                                    new_state: State::None,
                                });
                            }
                        }
                    }

                    debug!("Removing map events {:?} from queue", events_to_cancel);
                    for map_event in events_to_cancel.iter() {
                        map_events.remove(&map_event.event_id);
                    }
                }
                GameEventType::CancelMapEventsById { event_ids } => {
                    debug!("Processing CancelEvents: {:?}", event_ids);
                    events_to_remove.push(*event_id);

                    let mut events_to_cancel = Vec::new();

                    for event_id in event_ids.iter() {
                        if let Some(event) = map_events.get(event_id) {
                            events_to_cancel.push(event.clone());
                        }
                    }

                    debug!("Canceling map events: {:?}", events_to_cancel);
                    for map_event in events_to_cancel.iter() {
                        match map_event.event_type {
                            _ => {
                                let Some(entity) = entity_map.get_entity(map_event.obj_id) else {
                                    error!(
                                        "Cannot find item owner entity from id: {:?}",
                                        map_event.obj_id
                                    );
                                    continue;
                                };

                                let Ok(mut obj) = query.get_mut(entity) else {
                                    error!("Query failed to find entity {:?}", entity);
                                    continue;
                                };

                                debug!("Cancel event - reseting obj state to none.");
                                *obj.state = State::None;

                                debug!(
                                    "Cancel event - removing EventInProgress for entity: {:?}",
                                    obj.entity
                                );
                                commands.entity(obj.entity).remove::<EventInProgress>();

                                /*debug!("Cancel event - removing drink, eat, sleep completed events {:?}", map_event.entity_id);
                                commands
                                    .entity(map_event.entity_id)
                                    .remove::<DrinkEventCompleted>()
                                    .remove::<EatEventCompleted>()
                                    .remove::<SleepEventCompleted>();  */

                                // None visible state change
                                commands.trigger(StateChange {
                                    entity: obj.entity,
                                    new_state: State::None,
                                });
                            }
                        }
                    }

                    debug!("Removing map events {:?} from queue", event_ids);
                    for event_id in event_ids.iter() {
                        map_events.remove(event_id);
                    }
                }
                GameEventType::DespawnObj { obj_id } => {
                    debug!("Processing DespawnObj: {:?}", obj_id);
                    events_to_remove.push(*event_id);

                    // The obj may already be gone (e.g. an earlier empty-triggered
                    // despawn fired first), in which case there is nothing to do.
                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        continue;
                    };

                    commands.trigger(RemoveObj { entity });
                }
                _ => {}
            }
        }
    }

    for event_id in events_to_remove.iter() {
        game_events.remove(event_id);
    }

    for event in events_to_insert {
        game_events.insert(event.event_id, event);
    }
}

fn player_intro_state_system(
    game_tick: Res<GameTick>,
    presence: Res<PlayerWorldPresenceState>,
    mut player_intro_state: ResMut<PlayerIntroState>,
) {
    if game_tick.0 % 10 != 0 {
        return;
    }

    for (player_id, intro_entry) in player_intro_state.iter_mut() {
        if is_player_offline_protected(*player_id, &presence) {
            continue;
        }
        if !intro_entry.danger_unlocked && game_tick.0 >= intro_entry.start_tick + 4800 {
            intro_entry.danger_unlocked = true;
        }
    }
}

// Tier 1: Spawns low-tier pests when 20+ food items are stored in a storage structure
fn rat_event_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    storage_query: Query<(&Id, &PlayerId, &Position, &Inventory, &Storage)>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    map: Res<Map>,
    player_intro_state: Res<PlayerIntroState>,
    presence: Res<PlayerWorldPresenceState>,
    mut crisis_state: ResMut<CrisisState>,
    mut run_spawned_objs: ResMut<RunSpawnedObjs>,
) {
    // only run this every 20 ticks (2 seconds)
    if game_tick.0 % 20 != 0 {
        return;
    }

    let mut food_items_stored = HashMap::new();

    for (id, player_id, pos, inventory, _storage) in storage_query.iter() {
        if object_belongs_to_protected_run(id.0, &ids, &presence) {
            continue;
        }
        // Skip players who have already triggered the tier 1 pest crisis
        if let Some(crisis) = crisis_state.get(&player_id.0) {
            if crisis.rat_spoilage {
                continue;
            }
        }

        for item in inventory.items.iter() {
            if item.class == FOOD {
                food_items_stored
                    .entry(player_id.0)
                    .and_modify(|(_id, _pos, count)| *count += item.quantity)
                    .or_insert((id.0, pos.clone(), item.quantity));
            }
        }
    }

    for (player_id, (id, pos, count)) in food_items_stored.iter() {
        if intro_is_younger_than(&game_tick, *player_id, &player_intro_state, 4800) {
            continue;
        }

        if *count >= 20 {
            // Try to find a valid spawn position with max attempts to prevent infinite loop
            let mut spawned = false;
            for _attempt in 0..10 {
                let spawn_pos =
                    get_random_pos_at_range(*player_id, pos.x, pos.y, 5, Vec::new(), &map);

                if let Some(spawn_pos) = spawn_pos {
                    let path = Map::find_path(
                        *pos,
                        spawn_pos,
                        &map,
                        *player_id,
                        Vec::new(),
                        true,
                        false,
                        false,
                        true,
                        true,
                    );

                    if let Some((path, _cost)) = path {
                        if path.len() < 20 {
                            let num_pests = rand::thread_rng().gen_range(2..=3);
                            for _ in 0..num_pests {
                                let npc_id = ids.new_obj_id();
                                Encounter::spawn_spoil_crisis(
                                    npc_id,
                                    NPC_PLAYER_ID,
                                    spawn_pos,
                                    random_early_game_enemy_template().to_string(),
                                    &mut commands,
                                    &mut ids,
                                    &mut entity_map,
                                    &templates,
                                    *id,
                                );
                                run_spawned_objs.entry(*player_id).or_default().push(npc_id);
                            }
                            spawned = true;
                            break;
                        }
                    }
                }
            }

            if spawned {
                info!(
                    "Tier 1 Crisis: Food pest spoilage triggered for player {}",
                    player_id
                );
                crisis_state
                    .entry(*player_id)
                    .or_insert_with(PlayerCrisis::default)
                    .rat_spoilage = true;
            }
        }
    }
}

/// Evaluates the Checkpoint 2 personal goblin crisis. This system only derives
/// state and advances ordered phases through `AssaultReady`; it deliberately
/// has no Commands, combat spawning, client packets, rewards, or database I/O.
fn personal_crisis_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    presence: OptionalPlayerWorldPresence,
    player_intro_state: Res<PlayerIntroState>,
    objectives: Res<Objectives>,
    mut settlement_crisis_state: ResMut<SettlementCrisisState>,
    mut crisis_telemetry_state: ResMut<CrisisTelemetryState>,
    mut balance_telemetry_state: ResMut<CrisisBalanceTelemetryState>,
    hero_query: Query<
        (
            &PlayerId,
            &State,
            Option<&StateDead>,
            Option<&TrueDeath>,
            Option<&BoundMonolith>,
        ),
        With<SubclassHero>,
    >,
    structure_query: Query<(&PlayerId, &State), With<ClassStructure>>,
    villager_query: Query<(&PlayerId, &State, Option<&StateDead>), With<SubclassVillager>>,
    storage_query: Query<(&PlayerId, &State, &Inventory), (With<Storage>, With<ClassStructure>)>,
    monolith_query: Query<(&Id, &Monolith)>,
) {
    let current_tick = game_tick.0;

    // Aggregate settlement facts once per evaluation rather than rescanning
    // the whole ECS separately for every player.
    let mut completed_structures: HashMap<i32, usize> = HashMap::new();
    for (player_id, state) in structure_query.iter() {
        if player_id.is_human() && Structure::is_built(*state) {
            *completed_structures.entry(player_id.0).or_default() += 1;
        }
    }

    let mut living_villagers: HashMap<i32, usize> = HashMap::new();
    for (player_id, state, state_dead) in villager_query.iter() {
        if player_id.is_human() && state.is_alive() && state_dead.is_none() {
            *living_villagers.entry(player_id.0).or_default() += 1;
        }
    }

    let mut stored_gold: HashMap<i32, i32> = HashMap::new();
    for (player_id, state, inventory) in storage_query.iter() {
        if player_id.is_human() && Structure::is_built(*state) {
            let total = stored_gold.entry(player_id.0).or_default();
            *total = total.saturating_add(inventory.get_total_gold());
        }
    }

    let monolith_levels: HashMap<i32, i32> = monolith_query
        .iter()
        .map(|(id, monolith)| (id.0, monolith.sanctuary_level))
        .collect();

    // Collapse duplicate hero rows conservatively. A player has a valid run if
    // any current hero row is alive and has neither death marker.
    let mut hero_runs: HashMap<i32, (bool, Option<i32>)> = HashMap::new();
    for (player_id, state, state_dead, true_death, bound_monolith) in hero_query.iter() {
        if !player_id.is_human() {
            continue;
        }

        let valid = state.is_alive() && state_dead.is_none() && true_death.is_none();
        let bound_id = bound_monolith.map(|bound| bound.id);
        hero_runs
            .entry(player_id.0)
            .and_modify(|(existing_valid, existing_bound)| {
                if valid {
                    *existing_valid = true;
                    *existing_bound = bound_id.or(*existing_bound);
                }
            })
            .or_insert((valid, bound_id));
    }

    for (player_id, (valid_run, bound_monolith_id)) in hero_runs.iter() {
        if is_player_offline_protected(*player_id, &presence) {
            continue;
        }

        let Some(intro) = player_intro_state.get(player_id) else {
            if let Some(crisis) = settlement_crisis_state.get_mut(player_id) {
                advance_online_crisis_time(crisis, current_tick, false);
            }
            continue;
        };

        if !valid_run {
            if let Some(crisis) = settlement_crisis_state.get_mut(player_id) {
                advance_online_crisis_time(crisis, current_tick, false);
            }
            continue;
        }

        let online = clients.is_player_online(*player_id);
        let crisis = match settlement_crisis_state.entry(*player_id) {
            Entry::Vacant(entry) => {
                entry.insert(SettlementCrisis::new(current_tick));
                crisis_telemetry_state.insert(*player_id, CrisisTelemetry::new(current_tick));
                let balance = balance_telemetry_state.entry(*player_id).or_default();
                balance.record_pressure(
                    CrisisPhase::Dormant,
                    current_tick,
                    0,
                    CrisisPressureBreakdown::default(),
                );
                balance.record_phase(CrisisPhase::Dormant, current_tick, 0);
                info!(
                    "personal_crisis_created player_id={} kind={:?} phase={:?} pressure=0 game_tick={} online={}",
                    player_id,
                    CrisisKind::Goblin,
                    CrisisPhase::Dormant,
                    current_tick,
                    online
                );
                // Keep the freshly-created state observably Dormant for one
                // evaluation even if a developed settlement already exists.
                continue;
            }
            Entry::Occupied(entry) => entry.into_mut(),
        };

        let count_online_time = intro.danger_unlocked && online;
        advance_online_crisis_time(crisis, current_tick, count_online_time);

        let objective = objectives.get(player_id);
        let pressure_breakdown = calculate_goblin_pressure_breakdown(&GoblinPressureFacts {
            danger_unlocked: intro.danger_unlocked,
            completed_structures: completed_structures.get(player_id).copied().unwrap_or(0),
            living_villagers: living_villagers.get(player_id).copied().unwrap_or(0),
            stored_gold: stored_gold.get(player_id).copied().unwrap_or(0),
            sanctuary_level: bound_monolith_id
                .and_then(|id| monolith_levels.get(&id).copied())
                .unwrap_or(0),
            explore_poi: objective.map(|value| value.explore_poi).unwrap_or(false),
            choose_expansion: objective
                .map(|value| value.choose_expansion)
                .unwrap_or(false),
            online_active_ticks: crisis.online_active_ticks,
        });
        crisis.pressure = pressure_breakdown.clamped_total;
        balance_telemetry_state
            .entry(*player_id)
            .or_default()
            .record_pressure(
                crisis.phase,
                current_tick,
                crisis.online_active_ticks,
                pressure_breakdown,
            );

        if count_online_time {
            if let Some((old_phase, new_phase)) = transition_goblin_crisis(crisis, current_tick) {
                crisis_telemetry_state
                    .entry(*player_id)
                    .or_insert_with(|| CrisisTelemetry::new(crisis.phase_started_tick))
                    .observe_phase(new_phase, current_tick);
                balance_telemetry_state
                    .entry(*player_id)
                    .or_default()
                    .record_phase(new_phase, current_tick, crisis.online_active_ticks);
                info!(
                    "personal_crisis_transition player_id={} old_phase={:?} new_phase={:?} pressure={} game_tick={} online=true",
                    player_id, old_phase, new_phase, crisis.pressure, current_tick
                );
            }
        }
    }

    // A hero entity can be absent while the world continues. Catch the
    // watermark up without crediting that interval, so a later recreation or
    // ECS repair cannot backfill missing-hero time.
    for (player_id, crisis) in settlement_crisis_state.iter_mut() {
        if !hero_runs.contains_key(player_id) && !is_player_offline_protected(*player_id, &presence)
        {
            advance_online_crisis_time(crisis, current_tick, false);
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct CrisisBalanceSnapshotQueries<'w, 's> {
    hero_query: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static Id,
            &'static Position,
            &'static Template,
            Option<&'static HeroClass>,
            &'static Stats,
            &'static Inventory,
            Option<&'static BoundMonolith>,
            &'static State,
            Option<&'static StateDead>,
            Option<&'static TrueDeath>,
        ),
        With<SubclassHero>,
    >,
    villager_query: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static Id,
            &'static State,
            &'static Stats,
            &'static Inventory,
            Option<&'static Assignment>,
            Option<&'static StateDead>,
        ),
        With<SubclassVillager>,
    >,
    structure_query: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static Id,
            &'static Position,
            &'static Template,
            &'static Subclass,
            &'static State,
            &'static Stats,
            &'static Inventory,
            Option<&'static StateDead>,
        ),
        With<ClassStructure>,
    >,
    monolith_query: Query<
        'w,
        's,
        (
            &'static Id,
            &'static Position,
            &'static Monolith,
            &'static State,
            Option<&'static StateDead>,
        ),
    >,
}

fn crisis_preparation_snapshot(
    player_id: i32,
    crisis: &SettlementCrisis,
    spawn_positions: &SpawnPositions,
    queries: &CrisisBalanceSnapshotQueries,
) -> (CrisisPreparationSnapshot, CrisisBalanceObservation) {
    let hero = queries
        .hero_query
        .iter()
        .find(|(owner, _, _, _, _, _, _, _, _, _, _)| owner.0 == player_id);
    let hero_is_alive = hero
        .map(|(_, _, _, _, _, stats, _, _, state, dead, true_death)| {
            state.is_alive() && stats.hp > 0 && dead.is_none() && true_death.is_none()
        })
        .unwrap_or(false);

    let mut snapshot = CrisisPreparationSnapshot {
        game_tick: crisis.last_evaluated_tick,
        phase: balance_phase_name(crisis.phase).to_string(),
        ..CrisisPreparationSnapshot::default()
    };
    let mut observation = CrisisBalanceObservation {
        tick: crisis.last_evaluated_tick,
        online_active_ticks: crisis.online_active_ticks,
        phase: Some(crisis.phase),
        ..CrisisBalanceObservation::default()
    };

    let mut hero_info = None;
    let mut bound_monolith_id = None;
    let mut hero_pos = None;
    if let Some((_, id, pos, template, hero_class, stats, inventory, bound, _, _, _)) = hero {
        bound_monolith_id = bound.map(|bound| bound.id);
        if hero_is_alive {
            hero_pos = Some(*pos);
            hero_info = Some(AssaultHeroInfo {
                id: id.0,
                pos: *pos,
                bound_monolith_id,
                valid_run: spawn_positions.contains_key(&player_id),
            });
        }
        snapshot.hero_class = hero_class
            .map(|class| class.to_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        snapshot.hero_template = template.0.clone();
        snapshot.hero_health = stats.hp;
        snapshot.hero_max_health = stats.base_hp;
        snapshot.equipped_weapon = inventory
            .items
            .iter()
            .find(|item| item.equipped && item.class == WEAPON)
            .map(|item| item.name.clone());
        snapshot.equipped_armor_count = inventory
            .items
            .iter()
            .filter(|item| item.equipped && item.class == ARMOR)
            .count() as i32;
        observation.equipped_item_ids = inventory
            .items
            .iter()
            .filter(|item| {
                item.quantity > 0 && item.equipped && (item.class == WEAPON || item.class == ARMOR)
            })
            .map(|item| item.id)
            .collect();
        snapshot.healing_items = inventory
            .items
            .iter()
            .filter(|item| is_usable_crisis_healing_item(item))
            .map(|item| item.quantity.max(0))
            .sum();
        snapshot.food_items = inventory
            .items
            .iter()
            .filter(|item| item.class == FOOD)
            .map(|item| item.quantity.max(0))
            .sum();
        snapshot.drink_items = inventory
            .items
            .iter()
            .filter(|item| item.class == DRINK)
            .map(|item| item.quantity.max(0))
            .sum();
        observation.total_run_items = inventory
            .items
            .iter()
            .map(|item| item.quantity.max(0))
            .sum();
        observation.equipped_weapon = snapshot.equipped_weapon.clone();
        observation.equipped_armor_count = snapshot.equipped_armor_count;
        observation.healing_items = snapshot.healing_items;
    }

    let mut assault_structures = Vec::new();
    for (owner, id, pos, template, subclass, state, stats, inventory, dead) in
        queries.structure_query.iter()
    {
        if owner.0 != player_id || dead.is_some() {
            continue;
        }
        observation.total_run_items = observation.total_run_items.saturating_add(
            inventory
                .items
                .iter()
                .map(|item| item.quantity.max(0))
                .sum::<i32>(),
        );
        if Structure::is_built(*state) {
            snapshot.completed_structures = snapshot.completed_structures.saturating_add(1);
            observation.completed_structure_ids.insert(id.0);
            observation.structure_health.insert(id.0, stats.hp);
            assault_structures.push(AssaultStructureInfo {
                id: id.0,
                owner_player_id: owner.0,
                pos: *pos,
                subclass: *subclass,
            });
            if *subclass == Subclass::Wall {
                snapshot.wall_segments = snapshot.wall_segments.saturating_add(1);
                snapshot.wall_total_health =
                    snapshot.wall_total_health.saturating_add(stats.hp.max(0));
                snapshot.wall_total_max_health = snapshot
                    .wall_total_max_health
                    .saturating_add(stats.base_hp.max(0));
                observation.wall_ids.insert(id.0);
                match template.0.as_str() {
                    "Stockade" => snapshot.stockades = snapshot.stockades.saturating_add(1),
                    "Palisade" => snapshot.palisades = snapshot.palisades.saturating_add(1),
                    _ => {}
                }
            }
            if *subclass == Subclass::Watchtower {
                snapshot.watchtowers = snapshot.watchtowers.saturating_add(1);
            }
            if matches!(*subclass, Subclass::Wall | Subclass::Watchtower) {
                observation.defensive_structure_ids.insert(id.0);
            }
            if *subclass == Subclass::Storage {
                let stored = inventory
                    .items
                    .iter()
                    .map(|item| item.quantity.max(0))
                    .sum::<i32>();
                observation.stored_items = observation.stored_items.saturating_add(stored);
                snapshot.stored_resources_total =
                    snapshot.stored_resources_total.saturating_add(stored);
                snapshot.stored_gold = snapshot
                    .stored_gold
                    .saturating_add(inventory.get_total_gold());
                snapshot.stored_food = snapshot.stored_food.saturating_add(
                    inventory
                        .items
                        .iter()
                        .filter(|item| item.class == FOOD)
                        .map(|item| item.quantity.max(0))
                        .sum::<i32>(),
                );
            }
        } else {
            snapshot.foundations = snapshot.foundations.saturating_add(1);
            observation.foundation_ids.insert(id.0);
            if matches!(*subclass, Subclass::Wall | Subclass::Watchtower) {
                observation.defensive_foundation_ids.insert(id.0);
            }
        }
    }

    let mut living_villager_ids = BTreeSet::new();
    for (owner, id, state, stats, inventory, assignment, dead) in queries.villager_query.iter() {
        if owner.0 != player_id || !state.is_alive() || dead.is_some() || stats.hp <= 0 {
            continue;
        }
        snapshot.villagers_alive = snapshot.villagers_alive.saturating_add(1);
        living_villager_ids.insert(id.0);
        if stats.base_damage.unwrap_or(0) > 0
            || inventory
                .items
                .iter()
                .any(|item| item.equipped && item.class == WEAPON)
        {
            snapshot.villagers_combat_capable = snapshot.villagers_combat_capable.saturating_add(1);
            observation.combat_capable_villagers.insert(id.0);
        }
        observation.total_run_items = observation.total_run_items.saturating_add(
            inventory
                .items
                .iter()
                .map(|item| item.quantity.max(0))
                .sum::<i32>(),
        );
        if let Some(assignment) = assignment {
            observation
                .villager_assignments
                .insert(id.0, assignment.structure_id);
        }
    }
    observation.villagers = living_villager_ids;

    let assault_monoliths = queries
        .monolith_query
        .iter()
        .filter(|(_, _, _, state, dead)| Structure::is_built(**state) && dead.is_none())
        .map(|(id, pos, monolith, _, _)| {
            (
                id.0,
                AssaultMonolithInfo {
                    pos: *pos,
                    sanctuary_level: monolith.sanctuary_level,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    if let Some(level) = bound_monolith_id
        .and_then(|id| assault_monoliths.get(&id))
        .map(|monolith| monolith.sanctuary_level)
    {
        snapshot.sanctuary_level = level;
        observation.sanctuary_level = level;
    }
    let settlement_anchor = hero_info
        .and_then(|hero| {
            select_personal_assault_anchor(
                player_id,
                hero,
                spawn_positions,
                &assault_structures,
                &assault_monoliths,
            )
        })
        .map(|anchor| {
            (
                anchor.pos,
                anchor
                    .sanctuary_level
                    .map(sanctuary_full_radius)
                    .unwrap_or(WEAK_SANCTUARY_RANGE),
            )
        });
    snapshot.hero_near_settlement = hero_pos
        .zip(settlement_anchor)
        .map(|(hero, (anchor, radius))| Map::dist(hero, anchor) < radius)
        .unwrap_or(false);
    observation.near_settlement = snapshot.hero_near_settlement;

    (snapshot, observation)
}

fn is_preparation_phase(phase: Option<CrisisPhase>) -> bool {
    matches!(
        phase,
        Some(CrisisPhase::Preparing | CrisisPhase::AssaultReady)
    )
}

fn record_crisis_preparation_observation(
    telemetry: &mut CrisisBalanceTelemetry,
    previous: Option<&CrisisBalanceObservation>,
    current: &CrisisBalanceObservation,
    offline_protected: bool,
) {
    telemetry.latest_near_settlement = current.near_settlement;
    telemetry.latest_online = current.online;

    let Some(previous) = previous else {
        return;
    };

    if is_preparation_phase(previous.phase)
        && is_preparation_phase(current.phase)
        && current.online
        && !offline_protected
    {
        let elapsed = current
            .online_active_ticks
            .saturating_sub(previous.online_active_ticks)
            .max(0);
        if current.near_settlement {
            telemetry.preparation_actions.online_ticks_near_settlement = telemetry
                .preparation_actions
                .online_ticks_near_settlement
                .saturating_add(elapsed);
        } else {
            telemetry
                .preparation_actions
                .online_ticks_away_from_settlement = telemetry
                .preparation_actions
                .online_ticks_away_from_settlement
                .saturating_add(elapsed);
        }

        let actions = &mut telemetry.preparation_actions;

        for structure_id in current
            .defensive_foundation_ids
            .difference(&previous.defensive_foundation_ids)
        {
            actions.record_defensive_structure_started(*structure_id, current.tick);
        }

        for structure_id in current
            .completed_structure_ids
            .difference(&previous.completed_structure_ids)
        {
            if current.defensive_structure_ids.contains(structure_id) {
                actions.record_defensive_structure_completed(
                    *structure_id,
                    current.wall_ids.contains(structure_id),
                    current.tick,
                );
            } else {
                actions.structures_built = actions.structures_built.saturating_add(1);
                actions.mark_action_at(current.tick);
            }
        }

        for (structure_id, hp) in &current.structure_health {
            let repaired = previous.completed_structure_ids.contains(structure_id)
                && current.completed_structure_ids.contains(structure_id)
                && previous
                    .structure_health
                    .get(structure_id)
                    .is_some_and(|previous_hp| *hp > *previous_hp);
            if repaired {
                actions.record_repair_completed(*structure_id, current.tick);
            }
        }

        for item_id in current
            .equipped_item_ids
            .difference(&previous.equipped_item_ids)
        {
            actions.record_equipment_change(*item_id, current.tick);
        }

        // Establish each high-water baseline from the prior sample before
        // observing the current value. This counts genuine new supply once
        // while ignoring transfer-out/transfer-back loops.
        actions.observe_healing_items(previous.healing_items, previous.tick);
        actions.observe_healing_items(current.healing_items, current.tick);

        for villager_id in current.villagers.difference(&previous.villagers) {
            actions.record_villager_recruited(*villager_id, current.tick);
        }
        for (villager_id, structure_id) in &current.villager_assignments {
            if previous.villager_assignments.get(villager_id) != Some(structure_id) {
                actions.record_villager_assignment_changed(*villager_id, current.tick);
            }
        }

        actions.observe_sanctuary_level(previous.sanctuary_level, previous.tick);
        actions.observe_sanctuary_level(current.sanctuary_level, current.tick);
        actions.observe_total_run_items(previous.total_run_items, previous.tick);
        actions.observe_total_run_items(current.total_run_items, current.tick);
        actions.observe_stored_items(previous.stored_items, previous.tick);
        actions.observe_stored_items(current.stored_items, current.tick);
    }

    // Warning response is observation, not a preparation-phase gate: a player
    // may receive Signs while away and return during Signs or Pressure, before
    // the formal Preparing phase begins.
    let warning_was_away = matches!(telemetry.warnings.signs_near_settlement, Some(false))
        || matches!(telemetry.warnings.preparing_near_settlement, Some(false))
        || matches!(
            telemetry.warnings.assault_ready_near_settlement,
            Some(false)
        );
    if warning_was_away && !previous.near_settlement && current.near_settlement {
        telemetry
            .preparation_actions
            .returned_to_settlement_after_warning = true;
    }
}

/// Samples authoritative state after crisis evaluation. It writes only the
/// runtime telemetry resources and never feeds a value back into gameplay.
pub(crate) fn crisis_balance_snapshot_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    presence: Res<PlayerWorldPresenceState>,
    spawn_positions: Res<SpawnPositions>,
    crisis_state: Res<SettlementCrisisState>,
    config: Res<CrisisBalanceTelemetryConfig>,
    mut telemetry_state: ResMut<CrisisBalanceTelemetryState>,
    mut observation_state: ResMut<CrisisBalanceObservationState>,
    snapshot_queries: CrisisBalanceSnapshotQueries,
) {
    // Detailed preparation deltas require a full inventory/unit/structure
    // observation each update. Production keeps this analysis sampler off;
    // explicit headless balance runs opt in with a bounded sample interval.
    if config.sample_interval_ticks.is_none() {
        return;
    }
    let sample_interval = config.sample_interval_ticks.unwrap_or(1).max(1);
    for (player_id, crisis) in crisis_state.iter() {
        let online = clients.is_player_online(*player_id);
        let hero_alive = snapshot_queries
            .hero_query
            .iter()
            .find(|(owner, _, _, _, _, _, _, _, _, _, _)| owner.0 == *player_id)
            .map(|(_, _, _, _, _, stats, _, _, state, dead, true_death)| {
                state.is_alive() && stats.hp > 0 && dead.is_none() && true_death.is_none()
            })
            .unwrap_or(false);
        {
            let telemetry = telemetry_state.entry(*player_id).or_default();
            let previous_phase = telemetry.latest_phase;
            if let (Some(previous_phase), Some(previous_alive)) =
                (previous_phase, telemetry.latest_hero_alive)
            {
                telemetry.assault_outcome.record_hero_lifecycle_transition(
                    Some(previous_phase),
                    Some(crisis.phase),
                    previous_alive,
                    hero_alive,
                );
            }
            telemetry.latest_phase = Some(crisis.phase);
            telemetry.latest_hero_alive = Some(hero_alive);
            if crisis.phase == CrisisPhase::Resolved
                && telemetry.assault_outcome.hero_alive_at_resolution.is_none()
            {
                telemetry.assault_outcome.hero_alive_at_resolution = Some(hero_alive);
            }
            if is_player_offline_protected(*player_id, &presence)
                && crisis.phase < CrisisPhase::AssaultActive
            {
                telemetry.assault_outcome.safe_logout_before_assault = true;
            }
            if telemetry.latest_online
                && !online
                && (crisis.phase == CrisisPhase::AssaultActive
                    || previous_phase == Some(CrisisPhase::AssaultActive))
            {
                telemetry.assault_outcome.ordinary_disconnect_during_assault = true;
            }
            if crisis.phase == CrisisPhase::AssaultActive
                && !telemetry.latest_online
                && online
                && telemetry.assault_outcome.ordinary_disconnect_during_assault
            {
                telemetry.assault_outcome.reconnected_during_assault = true;
            }
            telemetry.latest_online = online;
        }

        let should_sample = observation_state
            .0
            .get(player_id)
            .map(|previous| {
                previous.phase != Some(crisis.phase)
                    || game_tick.0.saturating_sub(previous.tick) >= sample_interval
            })
            .unwrap_or(true);
        if !should_sample {
            continue;
        }
        let (mut snapshot, mut current) =
            crisis_preparation_snapshot(*player_id, crisis, &spawn_positions, &snapshot_queries);
        snapshot.game_tick = game_tick.0;
        current.tick = game_tick.0;
        current.online = online;

        let previous = observation_state.0.remove(player_id);
        let telemetry = telemetry_state.entry(*player_id).or_default();
        record_crisis_preparation_observation(
            telemetry,
            previous.as_ref(),
            &current,
            is_player_offline_protected(*player_id, &presence),
        );

        match crisis.phase {
            CrisisPhase::Preparing => {
                telemetry
                    .preparation_snapshots
                    .preparing
                    .get_or_insert_with(|| snapshot.clone());
            }
            CrisisPhase::AssaultReady => {
                telemetry
                    .preparation_snapshots
                    .assault_ready
                    .get_or_insert_with(|| snapshot.clone());
            }
            CrisisPhase::AssaultActive => {
                if telemetry.preparation_snapshots.assault_launch.is_none() {
                    telemetry.preparation_actions.record_launch_readiness(
                        snapshot.healing_items,
                        current.combat_capable_villagers.iter().copied(),
                    );
                    telemetry.preparation_snapshots.assault_launch = Some(snapshot.clone());
                    telemetry.assault_outcome.villagers_at_launch = snapshot.villagers_alive;
                    telemetry.assault_outcome.structures_at_launch = snapshot.completed_structures;
                    telemetry.assault_outcome.wall_segments_at_launch = snapshot.wall_segments;
                }
                telemetry.assault_outcome.assault_units_remaining = crisis
                    .assault_unit_ids
                    .len()
                    .saturating_sub(crisis.assault_defeated_unit_ids.len())
                    as i32;
                telemetry.assault_outcome.assault_units_defeated =
                    crisis.assault_defeated_unit_ids.len() as i32;
            }
            CrisisPhase::Resolved => {
                // Captured below. The first resolved sample replaces the latest
                // pre-resolution/end-of-run sample and is then preserved.
            }
            _ => {}
        }
        if telemetry
            .preparation_snapshots
            .resolution_or_end
            .as_ref()
            .map(|existing| existing.phase.as_str())
            != Some(balance_phase_name(CrisisPhase::Resolved))
        {
            telemetry.preparation_snapshots.resolution_or_end = Some(snapshot);
        }

        if let Some(interval) = config.sample_interval_ticks.filter(|value| *value > 0) {
            if telemetry
                .pressure_snapshots
                .periodic
                .last()
                .map(|sample| game_tick.0.saturating_sub(sample.game_tick) >= interval)
                .unwrap_or(true)
            {
                telemetry
                    .pressure_snapshots
                    .periodic
                    .push(CrisisPressureSnapshot {
                        game_tick: game_tick.0,
                        online_active_ticks: crisis.online_active_ticks,
                        phase: balance_phase_name(crisis.phase).to_string(),
                        breakdown: telemetry.latest_pressure,
                    });
            }
        }

        observation_state.0.insert(*player_id, current);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssaultAnchorKind {
    BoundMonolith,
    PrimaryStructure,
    BuiltStructure,
    HeroFallback,
}

impl AssaultAnchorKind {
    fn as_str(self) -> &'static str {
        match self {
            AssaultAnchorKind::BoundMonolith => "bound_monolith",
            AssaultAnchorKind::PrimaryStructure => "primary_structure",
            AssaultAnchorKind::BuiltStructure => "built_structure",
            AssaultAnchorKind::HeroFallback => "hero_fallback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AssaultAnchor {
    id: i32,
    pos: Position,
    kind: AssaultAnchorKind,
    sanctuary_level: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct AssaultHeroInfo {
    id: i32,
    pos: Position,
    bound_monolith_id: Option<i32>,
    valid_run: bool,
}

#[derive(Debug, Clone, Copy)]
struct AssaultStructureInfo {
    id: i32,
    owner_player_id: i32,
    pos: Position,
    subclass: Subclass,
}

#[derive(Debug, Clone, Copy)]
struct AssaultMonolithInfo {
    pos: Position,
    sanctuary_level: i32,
}

#[derive(Debug, Clone, Copy)]
struct AssaultSanctuaryExclusion {
    owner_player_id: i32,
    pos: Position,
}

#[derive(Debug, Clone)]
struct AssaultUnitSnapshot {
    id: i32,
    attribution: CrisisAssaultUnit,
    normally_dead: bool,
}

fn select_personal_assault_anchor(
    player_id: i32,
    hero: AssaultHeroInfo,
    spawn_positions: &SpawnPositions,
    structures: &[AssaultStructureInfo],
    monoliths: &HashMap<i32, AssaultMonolithInfo>,
) -> Option<AssaultAnchor> {
    if let Some(bound_id) = hero.bound_monolith_id {
        if let Some(monolith) = monoliths.get(&bound_id) {
            return Some(AssaultAnchor {
                id: bound_id,
                pos: monolith.pos,
                kind: AssaultAnchorKind::BoundMonolith,
                sanctuary_level: Some(monolith.sanctuary_level),
            });
        }
    }

    let home = spawn_positions.get(&player_id).copied().unwrap_or(hero.pos);
    let mut owned = structures
        .iter()
        .filter(|structure| structure.owner_player_id == player_id)
        .copied()
        .collect::<Vec<_>>();
    owned.sort_by_key(|structure| {
        let primary_priority = match structure.subclass {
            Subclass::Campfire => 0,
            Subclass::Storage => 1,
            _ => 2,
        };
        (
            primary_priority,
            Map::dist(home, structure.pos),
            structure.id,
        )
    });

    if let Some(primary) = owned
        .iter()
        .find(|structure| matches!(structure.subclass, Subclass::Campfire | Subclass::Storage))
    {
        return Some(AssaultAnchor {
            id: primary.id,
            pos: primary.pos,
            kind: AssaultAnchorKind::PrimaryStructure,
            sanctuary_level: None,
        });
    }

    if let Some(structure) = owned.first() {
        return Some(AssaultAnchor {
            id: structure.id,
            pos: structure.pos,
            kind: AssaultAnchorKind::BuiltStructure,
            sanctuary_level: None,
        });
    }

    // A real allocated run always has a SpawnPositions entry. Requiring that
    // evidence prevents a partially constructed or stale hero row from turning
    // into a settlement anchor while retaining current-position compatibility.
    spawn_positions
        .contains_key(&player_id)
        .then_some(AssaultAnchor {
            id: hero.id,
            pos: hero.pos,
            kind: AssaultAnchorKind::HeroFallback,
            sanctuary_level: None,
        })
}

fn personal_assault_spawn_positions(
    owner_player_id: i32,
    anchor: AssaultAnchor,
    count: usize,
    occupied: &HashSet<Position>,
    structures: &[AssaultStructureInfo],
    sanctuaries: &[AssaultSanctuaryExclusion],
    map: &Map,
) -> Option<Vec<Position>> {
    if count == 0 {
        return Some(Vec::new());
    }

    let (minimum_radius, maximum_radius) = match anchor.sanctuary_level {
        Some(level) => {
            let weak_radius = sanctuary_weak_radius(level) as i32;
            (
                weak_radius + PERSONAL_ASSAULT_SANCTUARY_MIN_OFFSET,
                weak_radius + PERSONAL_ASSAULT_SANCTUARY_MAX_OFFSET,
            )
        }
        None => (
            PERSONAL_ASSAULT_FALLBACK_MIN_RADIUS,
            PERSONAL_ASSAULT_FALLBACK_MAX_RADIUS,
        ),
    };

    let mut candidates = Vec::new();
    for radius in minimum_radius..=maximum_radius {
        candidates.extend(Map::ring((anchor.pos.x, anchor.pos.y), radius));
    }
    candidates.shuffle(&mut rand::thread_rng());

    let mut selected = Vec::with_capacity(count);
    for (x, y) in candidates
        .into_iter()
        .take(PERSONAL_ASSAULT_SPAWN_CANDIDATE_LIMIT)
    {
        let pos = Position { x, y };
        if !Map::is_valid_pos((x, y))
            || !Map::is_passable(x, y, map)
            || occupied.contains(&pos)
            || selected.contains(&pos)
        {
            continue;
        }

        // Avoid putting a personal assault in the immediate footprint of a
        // neighbouring settlement. Helpers can still engage after the spawn.
        if structures.iter().any(|structure| {
            structure.owner_player_id != owner_player_id
                && Map::dist(pos, structure.pos) < PERSONAL_ASSAULT_NEIGHBOUR_EXCLUSION_DISTANCE
        }) || sanctuaries.iter().any(|sanctuary| {
            sanctuary.owner_player_id != owner_player_id
                && Map::dist(pos, sanctuary.pos) < PERSONAL_ASSAULT_NEIGHBOUR_EXCLUSION_DISTANCE
        }) {
            continue;
        }

        if Map::find_path(
            pos,
            anchor.pos,
            map,
            NPC_PLAYER_ID,
            Vec::new(),
            true,
            false,
            false,
            true,
            true,
        )
        .is_none()
        {
            continue;
        }

        selected.push(pos);
        if selected.len() == count {
            return Some(selected);
        }
    }

    None
}

#[derive(Debug, PartialEq, Eq)]
enum AssaultSpawnError {
    EmptyComposition,
    PositionCountMismatch,
    MissingTemplate(String),
}

fn spawn_goblin_assault(
    owner_player_id: i32,
    assault_id: u64,
    spawn_generation: u32,
    unit_templates: &[String],
    spawn_positions: &[Position],
    commands: &mut Commands,
    ids: &mut ResMut<Ids>,
    entity_map: &mut ResMut<EntityObjMap>,
    templates: &Res<Templates>,
    run_spawned_objs: &mut ResMut<RunSpawnedObjs>,
) -> Result<Vec<i32>, AssaultSpawnError> {
    if unit_templates.is_empty() {
        return Err(AssaultSpawnError::EmptyComposition);
    }
    if unit_templates.len() != spawn_positions.len() {
        return Err(AssaultSpawnError::PositionCountMismatch);
    }
    for template in unit_templates {
        if !templates
            .obj_templates
            .iter()
            .any(|candidate| candidate.template == *template)
        {
            return Err(AssaultSpawnError::MissingTemplate(template.clone()));
        }
    }

    // Template and position validation happens before the first deferred spawn,
    // so this loop is an all-or-nothing logical operation in the current ECS.
    let mut spawned_ids = Vec::with_capacity(unit_templates.len());
    for (template, spawn_pos) in unit_templates.iter().zip(spawn_positions.iter()) {
        let (entity, id, _, _) = Encounter::spawn_npc(
            NPC_PLAYER_ID,
            *spawn_pos,
            template.clone(),
            commands,
            ids,
            entity_map,
            templates,
        );
        commands.entity(entity).try_insert((
            CrisisAssaultUnit {
                owner_player_id,
                assault_id,
                spawn_generation,
            },
            Viewshed {
                range: PERSONAL_ASSAULT_VISION,
            },
        ));
        commands.trigger(NewObj { entity });
        spawned_ids.push(id.0);
    }

    run_spawned_objs
        .entry(owner_player_id)
        .or_default()
        .extend(spawned_ids.iter().copied());
    Ok(spawned_ids)
}

fn clear_assault_target_references(
    removed_ids: &HashSet<i32>,
    commands: &mut Commands,
    target_query: &Query<(
        Entity,
        Option<&Target>,
        Option<&VisibleTarget>,
        Option<&TaskTarget>,
    )>,
) {
    for (entity, target, visible_target, task_target) in target_query.iter() {
        if target
            .map(|target| removed_ids.contains(&target.id))
            .unwrap_or(false)
        {
            commands.entity(entity).try_remove::<Target>();
        }
        if visible_target
            .map(|target| removed_ids.contains(&target.target))
            .unwrap_or(false)
        {
            commands
                .entity(entity)
                .try_insert(VisibleTarget::new(NO_TARGET));
        }
        if task_target
            .map(|target| removed_ids.contains(&target.target))
            .unwrap_or(false)
        {
            commands
                .entity(entity)
                .try_insert(TaskTarget::new(NO_TARGET));
        }
    }
}

fn record_personal_assault_resolution(
    player_id: i32,
    game_tick: i32,
    crisis: &mut SettlementCrisis,
    run_score_state: &mut RunScoreState,
) -> bool {
    if crisis.resolution_recorded || crisis.phase == CrisisPhase::Resolved {
        return false;
    }

    crisis.resolution_recorded = true;
    crisis.resolved_at_tick = Some(game_tick);
    crisis.phase = CrisisPhase::Resolved;
    crisis.phase_started_tick = game_tick;
    crisis.phase_online_ticks = 0;
    crisis.warning_active = false;
    crisis.assault_unit_ids.clear();
    crisis.assault_recovery_required = false;
    run_score_state
        .entry(player_id)
        .or_insert_with(|| PlayerRunScore {
            start_tick: game_tick,
            ..PlayerRunScore::default()
        })
        .personal_crises_resolved += 1;

    info!(
        "personal_crisis_assault_resolved player_id={} phase={:?} assault_id={:?} generation={} game_tick={} completion_count={}",
        player_id,
        crisis.phase,
        crisis.assault_id,
        crisis.assault_spawn_generation,
        game_tick,
        run_score_state
            .get(&player_id)
            .map(|score| score.personal_crises_resolved)
            .unwrap_or(0)
    );
    true
}

/// Owns the Checkpoint 3 transition from ready through committed launch and
/// normal-combat resolution. A successful launch is the commitment point: an
/// ordinary disconnect cannot remove, reset, or rebuild the active assault.
fn personal_crisis_assault_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    presence: Res<PlayerWorldPresenceState>,
    map: Res<Map>,
    templates: Res<Templates>,
    (
        mut ids,
        mut entity_map,
        mut crisis_state,
        mut next_assault_id,
        mut run_spawned_objs,
        mut run_score_state,
        mut crisis_telemetry_state,
        mut balance_telemetry_state,
        balance_telemetry_config,
        mut balance_observation_state,
    ): (
        ResMut<Ids>,
        ResMut<EntityObjMap>,
        ResMut<SettlementCrisisState>,
        ResMut<NextCrisisAssaultId>,
        ResMut<RunSpawnedObjs>,
        ResMut<RunScoreState>,
        ResMut<CrisisTelemetryState>,
        ResMut<CrisisBalanceTelemetryState>,
        Res<CrisisBalanceTelemetryConfig>,
        ResMut<CrisisBalanceObservationState>,
    ),
    spawn_positions: Res<SpawnPositions>,
    balance_snapshot_queries: CrisisBalanceSnapshotQueries,
    hero_query: Query<
        (
            &PlayerId,
            &Id,
            &Position,
            &State,
            Option<&StateDead>,
            Option<&TrueDeath>,
            Option<&BoundMonolith>,
        ),
        With<SubclassHero>,
    >,
    structure_query: Query<
        (
            &PlayerId,
            &Id,
            &Position,
            &Subclass,
            &State,
            Option<&StateDead>,
        ),
        With<ClassStructure>,
    >,
    monolith_query: Query<(&Id, &Position, &Monolith, &State, Option<&StateDead>)>,
    occupied_query: Query<&Position>,
    assault_unit_query: Query<(&Id, &CrisisAssaultUnit, &State, Option<&StateDead>)>,
    target_query: Query<(
        Entity,
        Option<&Target>,
        Option<&VisibleTarget>,
        Option<&TaskTarget>,
    )>,
) {
    let current_tick = game_tick.0;
    let mut heroes = HashMap::new();
    for (player_id, id, pos, state, state_dead, true_death, bound_monolith) in hero_query.iter() {
        if !player_id.is_human() {
            continue;
        }
        let candidate = AssaultHeroInfo {
            id: id.0,
            pos: *pos,
            bound_monolith_id: bound_monolith.map(|bound| bound.id),
            valid_run: state.is_alive() && state_dead.is_none() && true_death.is_none(),
        };
        heroes
            .entry(player_id.0)
            .and_modify(|existing: &mut AssaultHeroInfo| {
                if candidate.valid_run && !existing.valid_run {
                    *existing = candidate;
                }
            })
            .or_insert(candidate);
    }

    let structures = structure_query
        .iter()
        .filter_map(|(player_id, id, pos, subclass, state, dead)| {
            (player_id.is_human() && Structure::is_built(*state) && dead.is_none()).then_some(
                AssaultStructureInfo {
                    id: id.0,
                    owner_player_id: player_id.0,
                    pos: *pos,
                    subclass: *subclass,
                },
            )
        })
        .collect::<Vec<_>>();
    let monoliths = monolith_query
        .iter()
        .filter_map(|(id, pos, monolith, state, dead)| {
            (Structure::is_built(*state) && dead.is_none()).then_some((
                id.0,
                AssaultMonolithInfo {
                    pos: *pos,
                    sanctuary_level: monolith.sanctuary_level,
                },
            ))
        })
        .collect::<HashMap<_, _>>();
    let sanctuary_exclusions = heroes
        .iter()
        .filter_map(|(owner_player_id, hero)| {
            let monolith = monoliths.get(&hero.bound_monolith_id?)?;
            Some(AssaultSanctuaryExclusion {
                owner_player_id: *owner_player_id,
                pos: monolith.pos,
            })
        })
        .collect::<Vec<_>>();
    let occupied = occupied_query.iter().copied().collect::<HashSet<_>>();
    let unit_snapshots = assault_unit_query
        .iter()
        .map(|(id, attribution, state, dead)| AssaultUnitSnapshot {
            id: id.0,
            attribution: *attribution,
            // Normal combat writes State::Dead synchronously and queues
            // StateDead. Observing either closes the same-update gap without
            // treating controlled cleanup (which writes neither) as defeat.
            normally_dead: *state == State::Dead || dead.is_some(),
        })
        .collect::<Vec<_>>();

    let player_ids = crisis_state.keys().copied().collect::<Vec<_>>();
    for player_id in player_ids {
        let online = clients.is_player_online(player_id);
        let hero = heroes.get(&player_id).copied();
        let valid_run = hero.map(|value| value.valid_run).unwrap_or(false);
        let Some(crisis) = crisis_state.get_mut(&player_id) else {
            continue;
        };

        // An active assault is committed world state and deliberately keeps
        // running after an ordinary disconnect. Protection is only a launch
        // barrier for the pre-active state; the presence reconciler repairs
        // the impossible protected+active invariant separately.
        if crisis.phase != CrisisPhase::AssaultActive
            && is_player_offline_protected(player_id, &presence)
        {
            continue;
        }

        match crisis.phase {
            CrisisPhase::AssaultReady => {
                // Ready state never reuses a prior logical assault. Once an ID
                // is committed the crisis must already be AssaultActive (or
                // Resolved); anything else requires explicit recovery rather
                // than an automatic second wave. Detect this before presence,
                // anchor, or stale-unit gates so corruption cannot hide behind
                // an ordinary disconnect or another pre-launch prerequisite.
                if crisis.assault_id.is_some() {
                    if !crisis.assault_recovery_required {
                        warn!(
                            "personal_crisis_assault_recovery_required player_id={} phase={:?} assault_id={:?} generation={} reason=ready_with_committed_assault game_tick={}",
                            player_id,
                            crisis.phase,
                            crisis.assault_id,
                            crisis.assault_spawn_generation,
                            current_tick
                        );
                    }
                    crisis.assault_recovery_required = true;
                    continue;
                }

                if !online || !valid_run {
                    continue;
                }
                if !crisis.assault_grace_logged {
                    info!(
                        "personal_crisis_assault_grace_started player_id={} phase={:?} assault_id={:?} game_tick={} online=true grace_ticks={} max_wait_ticks={}",
                        player_id,
                        crisis.phase,
                        crisis.assault_id,
                        current_tick,
                        ASSAULT_READY_GRACE_TICKS,
                        ASSAULT_MAX_ONLINE_WAIT_TICKS
                    );
                    crisis.assault_grace_logged = true;
                }
                if !assault_launch_allowed(crisis.phase_online_ticks, current_tick) {
                    continue;
                }

                if unit_snapshots
                    .iter()
                    .any(|unit| unit.attribution.owner_player_id == player_id)
                {
                    if !crisis.assault_spawn_warning_logged {
                        warn!(
                            "personal_crisis_assault_spawn_blocked player_id={} phase={:?} assault_id={:?} generation={} reason=prior_attributed_units game_tick={}",
                            player_id,
                            crisis.phase,
                            crisis.assault_id,
                            crisis.assault_spawn_generation,
                            current_tick
                        );
                        crisis.assault_spawn_warning_logged = true;
                    }
                    continue;
                }

                let Some(hero) = hero else {
                    continue;
                };
                let Some(anchor) = select_personal_assault_anchor(
                    player_id,
                    hero,
                    &spawn_positions,
                    &structures,
                    &monoliths,
                ) else {
                    if !crisis.assault_anchor_warning_logged {
                        warn!(
                            "personal_crisis_assault_spawn_blocked player_id={} phase={:?} assault_id={:?} reason=no_settlement_anchor game_tick={}",
                            player_id,
                            crisis.phase,
                            crisis.assault_id,
                            current_tick
                        );
                        crisis.assault_anchor_warning_logged = true;
                    }
                    continue;
                };
                crisis.assault_anchor_warning_logged = false;

                let unit_templates = GOBLIN_ASSAULT_COMPOSITION
                    .iter()
                    .map(|template| (*template).to_string())
                    .collect::<Vec<_>>();
                let Some(unit_positions) = personal_assault_spawn_positions(
                    player_id,
                    anchor,
                    unit_templates.len(),
                    &occupied,
                    &structures,
                    &sanctuary_exclusions,
                    &map,
                ) else {
                    if !crisis.assault_spawn_warning_logged {
                        warn!(
                            "personal_crisis_assault_spawn_failed player_id={} phase={:?} assault_id={:?} generation={} reason=no_valid_spawn anchor_kind={} anchor_id={} anchor=({}, {}) game_tick={}",
                            player_id,
                            crisis.phase,
                            crisis.assault_id,
                            crisis.assault_spawn_generation.saturating_add(1),
                            anchor.kind.as_str(),
                            anchor.id,
                            anchor.pos.x,
                            anchor.pos.y,
                            current_tick
                        );
                        crisis.assault_spawn_warning_logged = true;
                    }
                    continue;
                };

                let Some(assault_id) = next_assault_id.allocate() else {
                    if !crisis.assault_spawn_warning_logged {
                        error!(
                            "personal_crisis_assault_spawn_failed player_id={} phase={:?} reason=assault_id_exhausted game_tick={}",
                            player_id,
                            crisis.phase,
                            current_tick
                        );
                        crisis.assault_spawn_warning_logged = true;
                    }
                    continue;
                };
                let generation = crisis.assault_spawn_generation.saturating_add(1);
                match spawn_goblin_assault(
                    player_id,
                    assault_id,
                    generation,
                    &unit_templates,
                    &unit_positions,
                    &mut commands,
                    &mut ids,
                    &mut entity_map,
                    &templates,
                    &mut run_spawned_objs,
                ) {
                    Ok(spawned_ids) => {
                        // The periodic sampler normally runs after this system
                        // so it can capture launch readiness and outcomes from
                        // AssaultActive. Preserve that ordering, but close the
                        // final Ready interval at the successful launch
                        // commitment point before changing the phase. This is
                        // opt-in and runs at most once per committed assault.
                        if balance_telemetry_config.sample_interval_ticks.is_some() {
                            let (_, mut current) = crisis_preparation_snapshot(
                                player_id,
                                crisis,
                                &spawn_positions,
                                &balance_snapshot_queries,
                            );
                            current.tick = current_tick;
                            current.online = online;
                            let previous = balance_observation_state.0.remove(&player_id);
                            let telemetry = balance_telemetry_state.entry(player_id).or_default();
                            record_crisis_preparation_observation(
                                telemetry,
                                previous.as_ref(),
                                &current,
                                is_player_offline_protected(player_id, &presence),
                            );
                            balance_observation_state.0.insert(player_id, current);
                        }

                        crisis.assault_id = Some(assault_id);
                        crisis.assault_started_tick = Some(current_tick);
                        crisis.assault_online_ticks = 0;
                        crisis.assault_unit_ids = spawned_ids;
                        crisis.assault_defeated_unit_ids.clear();
                        crisis.assault_spawn_generation = generation;
                        crisis.phase = CrisisPhase::AssaultActive;
                        crisis.phase_started_tick = current_tick;
                        crisis.phase_online_ticks = 0;
                        crisis.assault_recovery_required = false;
                        crisis.assault_spawn_warning_logged = false;
                        crisis_telemetry_state
                            .entry(player_id)
                            .or_default()
                            .record_launch(current_tick);
                        let balance = balance_telemetry_state.entry(player_id).or_default();
                        balance.record_phase(
                            CrisisPhase::AssaultActive,
                            current_tick,
                            crisis.online_active_ticks,
                        );
                        balance
                            .assault_outcome
                            .record_launch_units(&crisis.assault_unit_ids);
                        info!(
                            "personal_crisis_assault_launched player_id={} phase={:?} assault_id={} generation={} game_tick={} online=true unit_count={} templates={:?} anchor_kind={} anchor_id={} anchor=({}, {}) spawn_positions={:?}",
                            player_id,
                            crisis.phase,
                            assault_id,
                            generation,
                            current_tick,
                            crisis.assault_unit_ids.len(),
                            unit_templates,
                            anchor.kind.as_str(),
                            anchor.id,
                            anchor.pos.x,
                            anchor.pos.y,
                            unit_positions
                        );
                    }
                    Err(error) => {
                        if !crisis.assault_spawn_warning_logged {
                            warn!(
                                "personal_crisis_assault_spawn_failed player_id={} phase={:?} assault_id={} generation={} error={:?} game_tick={}",
                                player_id,
                                crisis.phase,
                                assault_id,
                                generation,
                                error,
                                current_tick
                            );
                            crisis.assault_spawn_warning_logged = true;
                        }
                    }
                }
            }
            CrisisPhase::AssaultActive => {
                let Some(assault_id) = crisis.assault_id else {
                    if !crisis.assault_recovery_required {
                        warn!(
                            "personal_crisis_assault_recovery_required player_id={} phase={:?} assault_id=None generation={} reason=missing_assault_id game_tick={} online={} valid_run={}",
                            player_id,
                            crisis.phase,
                            crisis.assault_spawn_generation,
                            current_tick,
                            online,
                            valid_run
                        );
                    }
                    crisis.assault_recovery_required = true;
                    continue;
                };
                let generation = crisis.assault_spawn_generation;

                if crisis.assault_unit_ids.is_empty() {
                    if !crisis.assault_recovery_required {
                        warn!(
                            "personal_crisis_assault_recovery_required player_id={} phase={:?} assault_id={} generation={} reason=no_tracked_units game_tick={} online={} valid_run={}",
                            player_id,
                            crisis.phase,
                            assault_id,
                            generation,
                            current_tick,
                            online,
                            valid_run
                        );
                    }
                    crisis.assault_recovery_required = true;
                    continue;
                }

                // Record each normally-dead attributed object once. Ordinary
                // disconnect and controlled despawn are not death evidence.
                for id in crisis.assault_unit_ids.clone() {
                    if crisis.assault_defeated_unit_ids.contains(&id) {
                        continue;
                    }
                    let normally_dead = unit_snapshots.iter().any(|unit| {
                        unit.id == id
                            && unit.attribution.owner_player_id == player_id
                            && unit.attribution.assault_id == assault_id
                            && unit.attribution.spawn_generation == generation
                            && unit.normally_dead
                    });
                    if normally_dead {
                        crisis.assault_defeated_unit_ids.push(id);
                    }
                }

                let missing_expected_ids = crisis
                    .assault_unit_ids
                    .iter()
                    .filter(|id| !crisis.assault_defeated_unit_ids.contains(id))
                    .filter(|id| {
                        !unit_snapshots.iter().any(|unit| {
                            unit.id == **id
                                && unit.attribution.owner_player_id == player_id
                                && unit.attribution.assault_id == assault_id
                                && unit.attribution.spawn_generation == generation
                        })
                    })
                    .copied()
                    .collect::<Vec<_>>();

                if !missing_expected_ids.is_empty() {
                    if !crisis.assault_recovery_required {
                        warn!(
                            "personal_crisis_assault_recovery_required player_id={} phase={:?} assault_id={} generation={} reason=expected_unit_missing_without_normal_death missing_ids={:?} game_tick={} online={} valid_run={}",
                            player_id,
                            crisis.phase,
                            assault_id,
                            generation,
                            missing_expected_ids,
                            current_tick,
                            online,
                            valid_run
                        );
                    }
                    crisis.assault_recovery_required = true;
                    continue;
                }
                crisis.assault_recovery_required = false;

                if crisis
                    .assault_unit_ids
                    .iter()
                    .all(|id| crisis.assault_defeated_unit_ids.contains(id))
                {
                    clear_assault_target_references(
                        &crisis
                            .assault_unit_ids
                            .iter()
                            .copied()
                            .collect::<HashSet<_>>(),
                        &mut commands,
                        &target_query,
                    );
                    if record_personal_assault_resolution(
                        player_id,
                        current_tick,
                        crisis,
                        &mut run_score_state,
                    ) {
                        crisis_telemetry_state
                            .entry(player_id)
                            .or_default()
                            .record_resolution(current_tick);
                        let balance = balance_telemetry_state.entry(player_id).or_default();
                        balance.record_phase(
                            CrisisPhase::Resolved,
                            current_tick,
                            crisis.online_active_ticks,
                        );
                        balance.assault_outcome.assault_resolved = true;
                        balance.assault_outcome.assault_units_defeated = crisis
                            .assault_defeated_unit_ids
                            .len()
                            .try_into()
                            .unwrap_or(i32::MAX);
                        balance.assault_outcome.assault_units_remaining = 0;
                        balance.assault_outcome.assault_duration_ticks = crisis
                            .assault_started_tick
                            .map(|started| current_tick.saturating_sub(started));
                        balance.assault_outcome.resolved_while_owner_offline = !online;
                    }
                }
            }
            _ => {}
        }
    }
}

// Watches for the initial enemy kills and chains boar/crab into spider spawns
fn initial_encounter_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    mut game_events: ResMut<GameEvents>,
    mut initial_encounter_state: ResMut<InitialEncounterState>,
    mut player_intro_state: ResMut<PlayerIntroState>,
    mut intro_encounter_state: ResMut<IntroEncounterState>,
    objectives: Res<Objectives>,
    presence: Res<PlayerWorldPresenceState>,
    mut run_spawned_objs: ResMut<RunSpawnedObjs>,
    dead_query: Query<&Id, With<StateDead>>,
    hero_query: Query<
        (&PlayerId, &State, Option<&StateDead>, Option<&TrueDeath>),
        With<SubclassHero>,
    >,
) {
    if game_tick.0 % 10 != 0 {
        return;
    }

    let dead_ids: std::collections::HashSet<i32> = dead_query.iter().map(|id| id.0).collect();
    let live_player_ids: HashSet<i32> = hero_query
        .iter()
        .filter_map(|(player_id, state, state_dead, true_death)| {
            (player_id.is_human()
                && state.is_alive()
                && state_dead.is_none()
                && true_death.is_none())
            .then_some(player_id.0)
        })
        .collect();

    for (player_id, entry) in initial_encounter_state.iter_mut() {
        if is_player_offline_protected(*player_id, &presence) {
            continue;
        }

        // True Death cleanup and encounter spawning can otherwise share an
        // Update. Since spawns are deferred, a newly queued hostile would not
        // be visible to that cleanup sweep and could survive at a recycled
        // start location. Ordinary-dead and absent heroes also pause the chain.
        if !live_player_ids.contains(player_id) {
            continue;
        }

        let Some(intro_entry) = player_intro_state.get_mut(player_id) else {
            continue;
        };

        let intro_progress = intro_encounter_state
            .entry(*player_id)
            .or_insert_with(PlayerIntroEncounters::default);
        let all_opening_enemies_dead = entry.rat_ids.iter().all(|id| dead_ids.contains(id));

        if !intro_entry.shipwreck_chain_started && game_tick.0 >= entry.first_rat_spawn_tick {
            let first_rat_id = entry.rat_ids[0];
            let first_enemy_template = entry
                .opening_enemy_templates
                .get(0)
                .map(String::as_str)
                .unwrap_or(EARLY_GAME_ENEMY_TEMPLATES[0]);
            let (entity, _, _, _) = Encounter::spawn_npc_with_id(
                first_rat_id,
                NPC_PLAYER_ID,
                entry.spawn_pos,
                first_enemy_template.to_string(),
                &mut commands,
                &mut ids,
                &mut entity_map,
                &templates,
            );
            commands.trigger(NewObj { entity });
            intro_entry.shipwreck_chain_started = true;
            info!(
                "Initial Encounter: spawning first {} for player {}",
                first_enemy_template, player_id
            );
        }

        if game_tick.0 >= entry.second_rat_spawn_tick && !dead_ids.contains(&entry.rat_ids[1]) {
            let second_rat_id = entry.rat_ids[1];
            if entity_map.get_entity(second_rat_id).is_none() {
                let second_enemy_template = entry
                    .opening_enemy_templates
                    .get(1)
                    .map(String::as_str)
                    .unwrap_or(EARLY_GAME_ENEMY_TEMPLATES[1]);
                let (entity, _, _, _) = Encounter::spawn_npc_with_id(
                    second_rat_id,
                    NPC_PLAYER_ID,
                    entry.spawn_pos,
                    second_enemy_template.to_string(),
                    &mut commands,
                    &mut ids,
                    &mut entity_map,
                    &templates,
                );
                commands.trigger(NewObj { entity });
                intro_entry.shipwreck_chain_started = true;
                info!(
                    "Initial Encounter: spawning second {} for player {}",
                    second_enemy_template, player_id
                );
            }
        }

        if !entry.villager_event_scheduled
            && shipwreck_inspection_can_spawn_villager(
                game_tick.0,
                entry,
                objectives.get(player_id),
            )
        {
            let villager_event_id = ids.new_map_event_id();
            let villager_event = GameEvent {
                event_id: villager_event_id,
                start_tick: game_tick.0,
                run_tick: game_tick.0 + 1,
                event_type: GameEventType::SpawnVillager {
                    pos: entry.villager_spawn_pos,
                    player_id: *player_id,
                },
            };
            game_events.insert(villager_event.event_id, villager_event);
            entry.villager_event_scheduled = true;
        }

        // Phase 0: waiting for both opening enemies to die, then spawn boar/crab
        if !intro_progress.initial_encounter {
            if game_tick.0 >= entry.phase1_unlock_tick && all_opening_enemies_dead {
                let npc_id = ids.new_obj_id();
                let (entity, _, _, _) = Encounter::spawn_npc_with_id(
                    npc_id,
                    NPC_PLAYER_ID,
                    entry.spawn_pos,
                    entry.phase1_spawn.clone(),
                    &mut commands,
                    &mut ids,
                    &mut entity_map,
                    &templates,
                );
                commands.trigger(NewObj { entity });
                entry.phase1_npc_id = Some(npc_id);
                run_spawned_objs.entry(*player_id).or_default().push(npc_id);
                intro_progress.initial_encounter = true;
                info!(
                    "Initial Encounter: spawning {} after opening enemies killed for player {}",
                    entry.phase1_spawn, player_id
                );
            }
            continue;
        }

        // Phase 1: waiting for the boar/crab to die → spawn spider
        if !intro_progress.spider_encounter {
            if let Some(phase1_id) = entry.phase1_npc_id {
                if game_tick.0 >= entry.spider_unlock_tick && dead_ids.contains(&phase1_id) {
                    let (entity, spider_id, _, _) = Encounter::spawn_npc(
                        NPC_PLAYER_ID,
                        entry.spawn_pos,
                        "Spider".to_string(),
                        &mut commands,
                        &mut ids,
                        &mut entity_map,
                        &templates,
                    );
                    commands.trigger(NewObj { entity });
                    run_spawned_objs
                        .entry(*player_id)
                        .or_default()
                        .push(spider_id.0);
                    intro_progress.spider_encounter = true;
                    info!(
                        "Initial Encounter: spawning Spider after {} killed for player {}",
                        entry.phase1_spawn, player_id
                    );
                }
            }
        }
    }
}

// Tier 2: Spawns Wolf Pack when hero moves 8+ tiles from spawn position
fn wolf_pack_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    hero_query: Query<(&Id, &PlayerId, &Position), With<SubclassHero>>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    map: Res<Map>,
    spawn_positions: Res<SpawnPositions>,
    player_intro_state: Res<PlayerIntroState>,
    presence: Res<PlayerWorldPresenceState>,
    mut crisis_state: ResMut<CrisisState>,
    mut run_spawned_objs: ResMut<RunSpawnedObjs>,
) {
    // Check every 10 ticks (1 second)
    if game_tick.0 % 10 != 0 {
        return;
    }

    for (id, player_id, pos) in hero_query.iter() {
        if entity_belongs_to_protected_run(id, player_id, &presence) {
            continue;
        }
        if intro_is_younger_than(&game_tick, player_id.0, &player_intro_state, 4800) {
            continue;
        }

        // Skip players who have already triggered wolf pack
        if let Some(crisis) = crisis_state.get(&player_id.0) {
            if crisis.wolf_pack {
                continue;
            }
        }

        // Trigger when 8+ tiles from spawn (distance_sq >= 64), or force the pack
        // on the fallback deadline if the player has stayed close to home.
        let fallback_due = player_survival_ticks(&game_tick, player_id.0, &player_intro_state)
            >= WOLF_PACK_FALLBACK_TICKS;

        if !fallback_due {
            // Check distance from spawn
            let Some(spawn_pos) = spawn_positions.get(&player_id.0) else {
                continue;
            };

            let dx = (pos.x - spawn_pos.x) as f64;
            let dy = (pos.y - spawn_pos.y) as f64;
            let distance_sq = dx * dx + dy * dy;

            if distance_sq < 64.0 {
                continue;
            }
        }

        // Find spawn position near the hero
        let mut spawned = false;
        for _attempt in 0..10 {
            let wolf_spawn =
                get_random_pos_at_range(player_id.0, pos.x, pos.y, 4, Vec::new(), &map);

            if let Some(wolf_spawn) = wolf_spawn {
                let path = Map::find_path(
                    *pos,
                    wolf_spawn,
                    &map,
                    player_id.0,
                    Vec::new(),
                    true,
                    false,
                    false,
                    true,
                    true,
                );

                if let Some((path, _cost)) = path {
                    if path.len() < 15 {
                        // Spawn 2-3 wolves
                        let num_wolves = rand::thread_rng().gen_range(2..=3);
                        for _ in 0..num_wolves {
                            let (_, npc_id, _, _) = Encounter::spawn_npc(
                                NPC_PLAYER_ID,
                                wolf_spawn,
                                "Wolf".to_string(),
                                &mut commands,
                                &mut ids,
                                &mut entity_map,
                                &templates,
                            );
                            run_spawned_objs
                                .entry(player_id.0)
                                .or_default()
                                .push(npc_id.0);
                        }
                        spawned = true;
                        break;
                    }
                }
            }
        }

        if spawned {
            info!(
                "Tier 2 Crisis: Wolf Pack triggered for player {}",
                player_id.0
            );
            crisis_state
                .entry(player_id.0)
                .or_insert_with(PlayerCrisis::default)
                .wolf_pack = true;
        }
    }
}

// Tier 3: Spawns Goblin Wolf Riders when 30+ gold stored
fn goblin_raid_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    storage_query: Query<(&Id, &PlayerId, &Position, &Inventory, &Storage)>,
    hero_query: Query<(&Id, &PlayerId, &Position), With<SubclassHero>>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    map: Res<Map>,
    player_intro_state: Res<PlayerIntroState>,
    presence: Res<PlayerWorldPresenceState>,
    mut crisis_state: ResMut<CrisisState>,
    mut run_spawned_objs: ResMut<RunSpawnedObjs>,
) {
    // Check every 30 ticks (3 seconds)
    if game_tick.0 % 30 != 0 {
        return;
    }

    let mut gold_stored: HashMap<i32, (i32, Position, i32)> = HashMap::new();

    for (id, player_id, pos, inventory, _storage) in storage_query.iter() {
        if entity_belongs_to_protected_run(id, player_id, &presence) {
            continue;
        }
        // Skip players who have already triggered goblin raid
        if let Some(crisis) = crisis_state.get(&player_id.0) {
            if crisis.goblin_raid {
                continue;
            }
        }

        let gold = inventory.get_total_gold();
        if gold > 0 {
            gold_stored
                .entry(player_id.0)
                .and_modify(|(_id, _pos, count)| *count += gold)
                .or_insert((id.0, pos.clone(), gold));
        }
    }

    for (player_id, (id, pos, gold_count)) in gold_stored.iter() {
        if *gold_count >= 30 {
            let mut spawned = false;
            for _attempt in 0..10 {
                let spawn_pos =
                    get_random_pos_at_range(*player_id, pos.x, pos.y, 6, Vec::new(), &map);

                if let Some(spawn_pos) = spawn_pos {
                    let path = Map::find_path(
                        *pos,
                        spawn_pos,
                        &map,
                        *player_id,
                        Vec::new(),
                        true,
                        false,
                        false,
                        true,
                        true,
                    );

                    if let Some((path, _cost)) = path {
                        if path.len() < 20 {
                            // Spawn 2 Wolf Riders that steal gold/weapons
                            for _ in 0..2 {
                                let npc_id = ids.new_obj_id();
                                Encounter::spawn_steal_crisis(
                                    npc_id,
                                    NPC_PLAYER_ID,
                                    spawn_pos,
                                    "Wolf Rider".to_string(),
                                    &mut commands,
                                    &mut ids,
                                    &mut entity_map,
                                    &templates,
                                    *id,
                                );
                                run_spawned_objs.entry(*player_id).or_default().push(npc_id);
                            }
                            spawned = true;
                            break;
                        }
                    }
                }
            }

            if spawned {
                info!(
                    "Tier 3 Crisis: Goblin Wolf Rider Raid triggered for player {}",
                    player_id
                );
                crisis_state
                    .entry(*player_id)
                    .or_insert_with(PlayerCrisis::default)
                    .goblin_raid = true;
            }
        }
    }

    // Fallback: force the raid if the player never stockpiled enough gold by the
    // deadline. Steal from a storage if the player has one, otherwise the hero.
    for (hero_id, player_id, hero_pos) in hero_query.iter() {
        if entity_belongs_to_protected_run(hero_id, player_id, &presence) {
            continue;
        }
        if let Some(crisis) = crisis_state.get(&player_id.0) {
            if crisis.goblin_raid {
                continue;
            }
        }

        if player_survival_ticks(&game_tick, player_id.0, &player_intro_state)
            < GOBLIN_RAID_FALLBACK_TICKS
        {
            continue;
        }

        let (target_id, target_pos) = storage_query
            .iter()
            .find(|(_, storage_player_id, _, _, _)| storage_player_id.0 == player_id.0)
            .map(|(id, _, pos, _, _)| (id.0, *pos))
            .unwrap_or((hero_id.0, *hero_pos));

        let mut spawned = false;
        for _attempt in 0..10 {
            let spawn_pos = get_random_pos_at_range(
                player_id.0,
                target_pos.x,
                target_pos.y,
                6,
                Vec::new(),
                &map,
            );

            if let Some(spawn_pos) = spawn_pos {
                let path = Map::find_path(
                    target_pos,
                    spawn_pos,
                    &map,
                    player_id.0,
                    Vec::new(),
                    true,
                    false,
                    false,
                    true,
                    true,
                );

                if let Some((path, _cost)) = path {
                    if path.len() < 20 {
                        for _ in 0..2 {
                            let npc_id = ids.new_obj_id();
                            Encounter::spawn_steal_crisis(
                                npc_id,
                                NPC_PLAYER_ID,
                                spawn_pos,
                                "Wolf Rider".to_string(),
                                &mut commands,
                                &mut ids,
                                &mut entity_map,
                                &templates,
                                target_id,
                            );
                            run_spawned_objs
                                .entry(player_id.0)
                                .or_default()
                                .push(npc_id);
                        }
                        spawned = true;
                        break;
                    }
                }
            }
        }

        if spawned {
            info!(
                "Tier 3 Crisis: Goblin Wolf Rider Raid fallback triggered for player {}",
                player_id.0
            );
            crisis_state
                .entry(player_id.0)
                .or_insert_with(PlayerCrisis::default)
                .goblin_raid = true;
        }
    }
}

// Tier 4: Undead Incursion - triggers after each player survives 3 in-game days.
fn undead_incursion_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    hero_query: Query<(&Id, &PlayerId, &Position), With<SubclassHero>>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    map: Res<Map>,
    player_intro_state: Res<PlayerIntroState>,
    presence: Res<PlayerWorldPresenceState>,
    mut crisis_state: ResMut<CrisisState>,
    mut run_spawned_objs: ResMut<RunSpawnedObjs>,
) {
    if game_tick.0 % 10 != 0 {
        return;
    }

    for (id, player_id, pos) in hero_query.iter() {
        if entity_belongs_to_protected_run(id, player_id, &presence) {
            continue;
        }
        // Primary trigger at 3 survived days; the fallback guarantees it by the
        // deadline even if the primary threshold is ever raised past it.
        let survival_ticks = player_survival_ticks(&game_tick, player_id.0, &player_intro_state);
        let trigger_tick = UNDEAD_INCURSION_SURVIVAL_TICKS.min(UNDEAD_INCURSION_FALLBACK_TICKS);
        if survival_ticks < trigger_tick {
            continue;
        }

        // Skip if already triggered
        if let Some(crisis) = crisis_state.get(&player_id.0) {
            if crisis.undead_incursion {
                continue;
            }
        }

        let mut spawned = false;
        for _attempt in 0..10 {
            let spawn_pos = get_random_pos_at_range(player_id.0, pos.x, pos.y, 6, Vec::new(), &map);

            if let Some(spawn_pos) = spawn_pos {
                let path = Map::find_path(
                    *pos,
                    spawn_pos,
                    &map,
                    player_id.0,
                    Vec::new(),
                    true,
                    false,
                    false,
                    true,
                    true,
                );

                if let Some((path, _cost)) = path {
                    if path.len() < 20 {
                        // Spawn 3 Zombies + 1 Skeleton + 1 Necromancer
                        for _ in 0..3 {
                            let (_, npc_id, _, _) = Encounter::spawn_npc(
                                NPC_PLAYER_ID,
                                spawn_pos,
                                "Zombie".to_string(),
                                &mut commands,
                                &mut ids,
                                &mut entity_map,
                                &templates,
                            );
                            run_spawned_objs
                                .entry(player_id.0)
                                .or_default()
                                .push(npc_id.0);
                        }
                        let (_, skeleton_id, _, _) = Encounter::spawn_npc(
                            NPC_PLAYER_ID,
                            spawn_pos,
                            "Skeleton".to_string(),
                            &mut commands,
                            &mut ids,
                            &mut entity_map,
                            &templates,
                        );
                        let (_, necromancer_id, _, _) = Encounter::spawn_necromancer(
                            NPC_PLAYER_ID,
                            spawn_pos,
                            spawn_pos,
                            &mut commands,
                            &mut ids,
                            &mut entity_map,
                            &templates,
                        );
                        run_spawned_objs
                            .entry(player_id.0)
                            .or_default()
                            .extend([skeleton_id.0, necromancer_id.0]);
                        spawned = true;
                        break;
                    }
                }
            }
        }

        if spawned {
            info!(
                "Tier 4 Crisis: Undead Incursion triggered for player {}",
                player_id.0
            );
            crisis_state
                .entry(player_id.0)
                .or_insert_with(PlayerCrisis::default)
                .undead_incursion = true;
        }
    }
}

// Tier 5: Goblin Pillagers - triggers after each player survives 5 in-game days.
fn goblin_pillager_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    storage_query: Query<(&Id, &PlayerId, &Position), (With<Storage>, With<ClassStructure>)>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    map: Res<Map>,
    player_intro_state: Res<PlayerIntroState>,
    presence: Res<PlayerWorldPresenceState>,
    mut crisis_state: ResMut<CrisisState>,
    mut run_spawned_objs: ResMut<RunSpawnedObjs>,
) {
    if game_tick.0 % 10 != 0 {
        return;
    }

    // Collect one structure per player as the torch target
    let mut player_targets: HashMap<i32, (i32, Position)> = HashMap::new();

    for (id, player_id, pos) in storage_query.iter() {
        if entity_belongs_to_protected_run(id, player_id, &presence) {
            continue;
        }
        // Primary trigger at 5 survived days; the fallback guarantees it by the
        // deadline even if the primary threshold is ever raised past it.
        let survival_ticks = player_survival_ticks(&game_tick, player_id.0, &player_intro_state);
        let trigger_tick = GOBLIN_PILLAGER_SURVIVAL_TICKS.min(GOBLIN_PILLAGER_FALLBACK_TICKS);
        if survival_ticks < trigger_tick {
            continue;
        }

        if let Some(crisis) = crisis_state.get(&player_id.0) {
            if crisis.goblin_pillager {
                continue;
            }
        }

        player_targets
            .entry(player_id.0)
            .or_insert((id.0, pos.clone()));
    }

    for (player_id, (target_id, pos)) in player_targets.iter() {
        let mut spawned = false;
        for _attempt in 0..10 {
            let spawn_pos = get_random_pos_at_range(*player_id, pos.x, pos.y, 7, Vec::new(), &map);

            if let Some(spawn_pos) = spawn_pos {
                let path = Map::find_path(
                    *pos,
                    spawn_pos,
                    &map,
                    *player_id,
                    Vec::new(),
                    true,
                    false,
                    false,
                    true,
                    true,
                );

                if let Some((path, _cost)) = path {
                    if path.len() < 25 {
                        // Spawn 3 Goblin Pillagers that set structures on fire
                        for _ in 0..3 {
                            let npc_id = ids.new_obj_id();
                            Encounter::spawn_torch_crisis(
                                npc_id,
                                NPC_PLAYER_ID,
                                spawn_pos,
                                "Goblin Pillager".to_string(),
                                &mut commands,
                                &mut ids,
                                &mut entity_map,
                                &templates,
                                *target_id,
                            );
                            run_spawned_objs.entry(*player_id).or_default().push(npc_id);
                        }
                        spawned = true;
                        break;
                    }
                }
            }
        }

        if spawned {
            info!(
                "Tier 5 Crisis: Goblin Pillager Raid triggered for player {}",
                player_id
            );
            crisis_state
                .entry(*player_id)
                .or_insert_with(PlayerCrisis::default)
                .goblin_pillager = true;
        }
    }
}

// Nightly threat: spawns enemies near the hero at dusk each day, scaling with day count
// Nightly horde senses. NPC AI only chases targets within its viewshed and
// wanders otherwise; the default spawn viewshed of 2 left hordes milling around
// at the sanctuary ring instead of attacking. 14 covers the spawn ring radius
// plus the spread of a camp, so the horde actually descends on the settlement.
const HORDE_HUNT_VISION: u32 = 14;

fn nightly_threat_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    hero_query: Query<(&Id, &PlayerId, &Position), With<SubclassHero>>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    map: Res<Map>,
    clients: Res<Clients>,
    (player_intro_state, presence): (Res<PlayerIntroState>, Res<PlayerWorldPresenceState>),
    objectives: Res<Objectives>,
    crisis_state: Res<CrisisState>,
    legendary_threat_state: Res<LegendaryThreatState>,
    sanctuary_zones: Res<SanctuaryZones>,
    mut run_score_state: ResMut<RunScoreState>,
    mut run_spawned_objs: ResMut<RunSpawnedObjs>,
    mut last_threat_day: Local<HashMap<i32, i32>>,
) {
    let ticks_in_day = game_tick.0 % GAME_TICKS_PER_DAY;

    // Trigger at DUSK (tick 2000 within the day)
    if ticks_in_day != DUSK {
        return;
    }

    let current_day = game_tick.day();

    // Skip day 1 — let the player settle in
    if current_day <= 1 {
        return;
    }

    for (_id, player_id, pos) in hero_query.iter() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        if intro_is_younger_than(&game_tick, player_id.0, &player_intro_state, 4800) {
            continue;
        }
        let player_day = player_survival_day(&game_tick, player_id.0, &player_intro_state);

        // Only trigger once per day per player
        if let Some(&last_day) = last_threat_day.get(&player_id.0) {
            if last_day >= current_day {
                continue;
            }
        }

        // Determine threat composition based on day count
        let player_objectives = objectives.get(&player_id.0);
        let (creatures, warning) = if survival_director_active(player_day, player_objectives) {
            let tier = crisis_state.get(&player_id.0).map(crisis_tier).unwrap_or(0);
            let active_legendary_count = legendary_threat_state
                .get(&player_id.0)
                .map(|threat| {
                    if threat.active && !threat.defeated {
                        1
                    } else {
                        0
                    }
                })
                .unwrap_or(0);
            let horde_size = survival_horde_size(player_day, tier, active_legendary_count);
            (
                survival_horde_composition(horde_size, player_day),
                "The survival horde descends upon your settlement!",
            )
        } else {
            match player_day {
                2 => (vec!["Wolf"; 2], "Wolves howl nearby as darkness falls..."),
                3 => (
                    vec!["Wolf", "Cave Bat", "Ash Viper", "Reef Skitter"],
                    "The pack grows bolder tonight...",
                ),
                4 => (
                    vec!["Wolf", "Wolf", "Wolf", "Spider"],
                    "A chill wind carries the sound of many creatures...",
                ),
                5 => (
                    vec!["Skeleton", "Skeleton", "Zombie"],
                    "The dead stir as night approaches...",
                ),
                // Days 6-7 stay on the gentle ramp so the player has a calm window
                // to bank a food reserve; the heavy scaling horde takes over at day 8
                // (see survival_director_active).
                6 => (
                    vec!["Wolf", "Wolf", "Skeleton", "Spider"],
                    "Something larger stirs in the dark...",
                ),
                7 => (
                    vec!["Skeleton", "Skeleton", "Zombie", "Spider"],
                    "The dead are restless tonight...",
                ),
                _ => (
                    vec!["Skeleton", "Skeleton", "Zombie", "Zombie", "Shadow"],
                    "An unnatural darkness gathers...",
                ),
            }
        };

        // Send warning notice to player
        let packet = ResponsePacket::Notice {
            noticemsg: warning.to_string(),
            expiry: Some(8000),
        };
        send_to_client(player_id.0, packet, &clients);

        // The horde rises from the wilderness beyond the sanctuary and marches in
        // (it ignores the sanctuary — that's the scheduled threat you prepare for).
        let mut spawned = false;
        if let Some(spawn_pos) = crisis_spawn_pos(player_id.0, &sanctuary_zones, *pos, &map) {
            for creature_type in &creatures {
                let (npc_entity, npc_id, _, _) = Encounter::spawn_npc(
                    NPC_PLAYER_ID,
                    spawn_pos,
                    creature_type.to_string(),
                    &mut commands,
                    &mut ids,
                    &mut entity_map,
                    &templates,
                );
                // Widen the horde's senses so it can find the settlement from
                // the spawn ring (see HORDE_HUNT_VISION).
                commands.entity(npc_entity).insert(Viewshed {
                    range: HORDE_HUNT_VISION,
                });
                // Attribute the wave to this run so True Death removes any
                // survivors — they spawn outside the camp-cleanup radius.
                run_spawned_objs
                    .entry(player_id.0)
                    .or_default()
                    .push(npc_id.0);
            }
            spawned = true;
        }

        if spawned {
            info!(
                "Nightly threat: player day {} spawned {} creatures for player {}",
                player_day,
                creatures.len(),
                player_id.0
            );
            last_threat_day.insert(player_id.0, current_day);
            run_score_state
                .entry(player_id.0)
                .or_insert_with(|| PlayerRunScore {
                    start_tick: game_tick.0,
                    ..PlayerRunScore::default()
                })
                .waves_survived += 1;
        }
    }
}

fn find_legendary_hideout_pos(player_id: i32, spawn_pos: Position, map: &Map) -> Position {
    let mut candidates = Vec::new();
    for range in 14..=22 {
        for (x, y) in Map::ring((spawn_pos.x, spawn_pos.y), range) {
            if Map::is_valid_pos((x, y)) && Map::is_passable(x, y, map) {
                candidates.push(Position { x, y });
            }
        }
    }

    if candidates.is_empty() {
        return get_random_pos_at_range(player_id, spawn_pos.x, spawn_pos.y, 14, Vec::new(), map)
            .unwrap_or(spawn_pos);
    }

    let index = rand::thread_rng().gen_range(0..candidates.len());
    candidates[index]
}

fn spawn_legendary_npc(
    player_id: i32,
    npc_id: i32,
    pos: Position,
    template: &str,
    commands: &mut Commands,
    ids: &mut ResMut<Ids>,
    entity_map: &mut ResMut<EntityObjMap>,
    templates: &Res<Templates>,
) -> Entity {
    let (entity, _, _, _) = Encounter::spawn_npc_with_id(
        npc_id,
        NPC_PLAYER_ID,
        pos,
        template.to_string(),
        commands,
        ids,
        entity_map,
        templates,
    );

    if template == LEGENDARY_BOSS {
        commands.entity(entity).insert(LegendaryBoss { player_id });
    }

    entity
}

fn spawn_legendary_hideout(
    player_id: i32,
    pos: Position,
    commands: &mut Commands,
    ids: &mut ResMut<Ids>,
    entity_map: &mut ResMut<EntityObjMap>,
    templates: &Res<Templates>,
) -> (i32, i32) {
    let hideout_id = ids.new_obj_id();
    let hideout = Obj::create_nospawn(
        hideout_id,
        MERCHANT_PLAYER_ID,
        LEGENDARY_HIDEOUT.to_string(),
        pos,
        State::None,
        Inventory {
            owner: hideout_id,
            items: Vec::new(),
        },
        templates,
    );
    let hideout_entity = commands
        .spawn((hideout, LegendaryHideout { player_id }))
        .id();
    ids.new_obj(hideout_id, MERCHANT_PLAYER_ID);
    entity_map.new_obj(hideout_id, hideout_entity);
    commands.trigger(NewObj {
        entity: hideout_entity,
    });

    let boss_id = ids.new_obj_id();
    let boss_entity = spawn_legendary_npc(
        player_id,
        boss_id,
        pos,
        LEGENDARY_BOSS,
        commands,
        ids,
        entity_map,
        templates,
    );
    commands.trigger(NewObj {
        entity: boss_entity,
    });

    for guard_template in [LEGENDARY_RAIDER, LEGENDARY_RAIDER, LEGENDARY_CAPTAIN] {
        let guard_id = ids.new_obj_id();
        let guard_entity = spawn_legendary_npc(
            player_id,
            guard_id,
            pos,
            guard_template,
            commands,
            ids,
            entity_map,
            templates,
        );
        let role = if guard_template == LEGENDARY_CAPTAIN {
            LegendaryFollowerRole::Captain
        } else {
            LegendaryFollowerRole::Raider
        };
        commands
            .entity(guard_entity)
            .insert(LegendaryFollower { player_id, role });
        commands.trigger(NewObj {
            entity: guard_entity,
        });
    }

    (hideout_id, boss_id)
}

fn reveal_legendary_hideout(
    player_id: i32,
    threat: &mut LegendaryThreat,
    objectives: &mut Objectives,
    clients: &Res<Clients>,
    reason: &str,
) {
    if threat.hideout_revealed {
        return;
    }

    threat.hideout_revealed = true;
    objectives
        .entry(player_id)
        .or_insert_with(PlayerObjectives::default)
        .find_legendary_hideout = true;

    let packet = ResponsePacket::DiscoveryEvent {
        version: 1,
        discovery_type: "legendary_hideout".to_string(),
        title: "Dragon hideout revealed".to_string(),
        unlock_source: reason.to_string(),
        location: Some(format!("{},{}", threat.hideout_pos.x, threat.hideout_pos.y)),
        result: "The raids have a source. The Fire Dragon can be ended only inside the hideout."
            .to_string(),
    };
    send_to_client(player_id, packet, clients);
}

fn spawn_legendary_follower(
    player_id: i32,
    role: LegendaryFollowerRole,
    pos: Position,
    structure_target: Option<i32>,
    storage_target: Option<i32>,
    commands: &mut Commands,
    ids: &mut ResMut<Ids>,
    entity_map: &mut ResMut<EntityObjMap>,
    templates: &Res<Templates>,
) -> i32 {
    let npc_id = ids.new_obj_id();
    let entity = match role {
        LegendaryFollowerRole::Torchbearer => {
            if let Some(target_id) = structure_target {
                let (entity, _, _, _) = Encounter::spawn_torch_crisis(
                    npc_id,
                    NPC_PLAYER_ID,
                    pos,
                    LEGENDARY_TORCHBEARER.to_string(),
                    commands,
                    ids,
                    entity_map,
                    templates,
                    target_id,
                );
                entity
            } else {
                spawn_legendary_npc(
                    player_id,
                    npc_id,
                    pos,
                    LEGENDARY_TORCHBEARER,
                    commands,
                    ids,
                    entity_map,
                    templates,
                )
            }
        }
        LegendaryFollowerRole::Thief => {
            if let Some(target_id) = storage_target {
                let (entity, _, _, _) = Encounter::spawn_steal_crisis(
                    npc_id,
                    NPC_PLAYER_ID,
                    pos,
                    LEGENDARY_THIEF.to_string(),
                    commands,
                    ids,
                    entity_map,
                    templates,
                    target_id,
                );
                entity
            } else {
                spawn_legendary_npc(
                    player_id,
                    npc_id,
                    pos,
                    LEGENDARY_THIEF,
                    commands,
                    ids,
                    entity_map,
                    templates,
                )
            }
        }
        LegendaryFollowerRole::Captain => spawn_legendary_npc(
            player_id,
            npc_id,
            pos,
            LEGENDARY_CAPTAIN,
            commands,
            ids,
            entity_map,
            templates,
        ),
        LegendaryFollowerRole::Raider => spawn_legendary_npc(
            player_id,
            npc_id,
            pos,
            LEGENDARY_RAIDER,
            commands,
            ids,
            entity_map,
            templates,
        ),
    };

    commands.entity(entity).insert(LegendaryFollower {
        player_id,
        role: role.clone(),
    });
    commands.trigger(NewObj { entity });

    npc_id
}

fn legendary_threat_status(threat: &LegendaryThreat) -> String {
    if threat.defeated {
        "defeated".to_string()
    } else if threat.active {
        "active".to_string()
    } else if threat.rumor_sent {
        "rumored".to_string()
    } else {
        "unknown".to_string()
    }
}

fn legendary_threat_packets(
    player_id: i32,
    game_tick: &GameTick,
    legendary_threat_state: &LegendaryThreatState,
) -> Vec<network::LegendaryThreatPacket> {
    let Some(threat) = legendary_threat_state.get(&player_id) else {
        return Vec::new();
    };

    let days_active = threat
        .active_since_tick
        .map(|tick| ((game_tick.0 - tick).max(0) / GAME_TICKS_PER_DAY) + 1)
        .unwrap_or(0);
    let next_attack_eta = if threat.active && !threat.defeated {
        Some(((threat.next_follower_tick - game_tick.0).max(0)) / TICKS_PER_SEC)
    } else {
        None
    };

    vec![network::LegendaryThreatPacket {
        name: threat.name.clone(),
        status: legendary_threat_status(threat),
        days_active,
        hideout_known: threat.hideout_revealed,
        hideout_location: if threat.hideout_revealed {
            Some(format!("{},{}", threat.hideout_pos.x, threat.hideout_pos.y))
        } else {
            None
        },
        next_attack_eta,
        followers_defeated: threat.followers_defeated,
        captains_defeated: threat.captains_defeated,
    }]
}

fn legendary_threat_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    map: Res<Map>,
    spawn_positions: Res<SpawnPositions>,
    (player_intro_state, presence): (Res<PlayerIntroState>, Res<PlayerWorldPresenceState>),
    hero_query: Query<(&Id, &PlayerId, &Position), With<SubclassHero>>,
    structure_query: Query<(&Id, &PlayerId, &Position), With<ClassStructure>>,
    storage_query: Query<(&Id, &PlayerId, &Position, &Inventory), With<Storage>>,
    dead_query: Query<&Id, With<StateDead>>,
    mut objectives: ResMut<Objectives>,
    mut legendary_threat_state: ResMut<LegendaryThreatState>,
    mut run_spawned_objs: ResMut<RunSpawnedObjs>,
) {
    if game_tick.0 % 10 != 0 {
        return;
    }

    let dead_ids: HashSet<i32> = dead_query.iter().map(|id| id.0).collect();

    for (_hero_id, player_id, hero_pos) in hero_query.iter() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        let player_day = player_survival_day(&game_tick, player_id.0, &player_intro_state);
        if player_day < LEGENDARY_RUMOR_DAY {
            continue;
        }

        let Some(spawn_pos) = spawn_positions.get(&player_id.0).copied() else {
            continue;
        };

        if !legendary_threat_state.contains_key(&player_id.0) {
            let hideout_pos = find_legendary_hideout_pos(player_id.0, spawn_pos, &map);
            let (hideout_id, boss_id) = spawn_legendary_hideout(
                player_id.0,
                hideout_pos,
                &mut commands,
                &mut ids,
                &mut entity_map,
                &templates,
            );
            // Attribute the hideout + boss to this run so True Death removes
            // them — they sit far outside the camp-cleanup radius.
            run_spawned_objs
                .entry(player_id.0)
                .or_default()
                .extend([hideout_id, boss_id]);
            legendary_threat_state.insert(
                player_id.0,
                LegendaryThreat {
                    name: LEGENDARY_BOSS.to_string(),
                    hideout_pos,
                    hideout_id: Some(hideout_id),
                    boss_id: Some(boss_id),
                    rumor_sent: true,
                    active: false,
                    defeated: false,
                    hideout_revealed: false,
                    active_since_tick: None,
                    defeated_at_tick: None,
                    next_follower_tick: 0,
                    waves_sent: 0,
                    follower_waves: Vec::new(),
                    followers_defeated: 0,
                    captains_defeated: 0,
                },
            );

            let packet = ResponsePacket::Notice {
                noticemsg: "Smoke coils from somewhere inland. Survivors whisper of a Fire Dragon."
                    .to_string(),
                expiry: Some(12000),
            };
            send_to_client(player_id.0, packet, &clients);
        }

        let Some(threat) = legendary_threat_state.get_mut(&player_id.0) else {
            continue;
        };

        for wave in threat.follower_waves.iter_mut() {
            if !wave.defeated && wave.ids.iter().all(|id| dead_ids.contains(id)) {
                wave.defeated = true;
            }
        }

        let defeated_wave_count = threat
            .follower_waves
            .iter()
            .filter(|wave| wave.defeated)
            .count() as i32;

        if !threat.hideout_revealed
            && (threat.captains_defeated >= LEGENDARY_HIDEOUT_REVEAL_CAPTAINS
                || defeated_wave_count >= LEGENDARY_HIDEOUT_REVEAL_WAVES)
        {
            reveal_legendary_hideout(
                player_id.0,
                threat,
                &mut objectives,
                &clients,
                "Follower campaign",
            );
        }

        if !threat.hideout_revealed
            && Map::dist(*hero_pos, threat.hideout_pos) <= LEGENDARY_HIDEOUT_REVEAL_RANGE
        {
            reveal_legendary_hideout(player_id.0, threat, &mut objectives, &clients, "Scouting");
        }

        if player_day >= LEGENDARY_ACTIVE_DAY && !threat.active && !threat.defeated {
            threat.active = true;
            threat.active_since_tick = Some(game_tick.0);
            threat.next_follower_tick = game_tick.0;
            let packet = ResponsePacket::Notice {
                noticemsg: "The Fire Dragon has found your trail. Its followers will keep coming until the hideout falls.".to_string(),
                expiry: Some(14000),
            };
            send_to_client(player_id.0, packet, &clients);
        }

        if !threat.active || threat.defeated || game_tick.0 < threat.next_follower_tick {
            continue;
        }

        let structure_target = structure_query
            .iter()
            .find(|(_, structure_player_id, _)| structure_player_id.0 == player_id.0)
            .map(|(id, _, pos)| (id.0, *pos));
        let storage_target = storage_query
            .iter()
            .filter(|(_, storage_player_id, _, _)| storage_player_id.0 == player_id.0)
            .max_by_key(|(_, _, _, inventory)| inventory.get_total_gold())
            .map(|(id, _, pos, _)| (id.0, *pos));

        let target_pos = storage_target
            .map(|(_, pos)| pos)
            .or_else(|| structure_target.map(|(_, pos)| pos))
            .unwrap_or(*hero_pos);
        let spawn_pos =
            get_random_pos_at_range(player_id.0, target_pos.x, target_pos.y, 6, Vec::new(), &map)
                .unwrap_or(target_pos);

        threat.waves_sent += 1;
        let mut roles = vec![LegendaryFollowerRole::Raider];
        if storage_target.is_some() {
            roles.push(LegendaryFollowerRole::Thief);
        }
        if structure_target.is_some() {
            roles.push(LegendaryFollowerRole::Torchbearer);
        }
        if threat.waves_sent % 3 == 0 {
            roles.push(LegendaryFollowerRole::Captain);
        }

        let mut wave_ids = Vec::new();
        for role in roles {
            let follower_id = spawn_legendary_follower(
                player_id.0,
                role,
                spawn_pos,
                structure_target.map(|(id, _)| id),
                storage_target.map(|(id, _)| id),
                &mut commands,
                &mut ids,
                &mut entity_map,
                &templates,
            );
            wave_ids.push(follower_id);
        }

        // Followers roam between their spawn point and the player's camp;
        // attribute them to this run so True Death removes any survivors.
        run_spawned_objs
            .entry(player_id.0)
            .or_default()
            .extend(wave_ids.iter().copied());

        threat.follower_waves.push(LegendaryFollowerWave {
            ids: wave_ids,
            defeated: false,
        });

        let active_ticks = threat
            .active_since_tick
            .map(|tick| game_tick.0 - tick)
            .unwrap_or(0);
        let delay = if active_ticks >= LEGENDARY_FAST_AFTER_TICKS {
            LEGENDARY_FAST_WAVE_TICKS
        } else {
            LEGENDARY_STANDARD_WAVE_TICKS
        };
        threat.next_follower_tick = game_tick.0 + delay;
    }
}

fn legendary_death_tracking_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    dead_query: Query<(&Id, Option<&LegendaryFollower>, Option<&LegendaryBoss>), Added<StateDead>>,
    mut objectives: ResMut<Objectives>,
    mut run_score_state: ResMut<RunScoreState>,
    mut legendary_threat_state: ResMut<LegendaryThreatState>,
    presence: Res<PlayerWorldPresenceState>,
) {
    for (id, follower, boss) in dead_query.iter() {
        if let Some(follower) = follower {
            if is_player_offline_protected(follower.player_id, &presence) {
                continue;
            }
            let run_score = run_score_state
                .entry(follower.player_id)
                .or_insert_with(|| PlayerRunScore {
                    start_tick: game_tick.0,
                    ..PlayerRunScore::default()
                });
            run_score.enemies_killed += 1;

            if follower.role == LegendaryFollowerRole::Captain {
                run_score.elites_killed += 1;
                run_score.captains_killed += 1;
            }

            if let Some(threat) = legendary_threat_state.get_mut(&follower.player_id) {
                threat.followers_defeated += 1;
                if follower.role == LegendaryFollowerRole::Captain {
                    threat.captains_defeated += 1;
                }

                if threat.captains_defeated >= LEGENDARY_HIDEOUT_REVEAL_CAPTAINS {
                    reveal_legendary_hideout(
                        follower.player_id,
                        threat,
                        &mut objectives,
                        &clients,
                        "Captain defeated",
                    );
                }
            }
        }

        if let Some(boss) = boss {
            if is_player_offline_protected(boss.player_id, &presence) {
                continue;
            }
            let run_score =
                run_score_state
                    .entry(boss.player_id)
                    .or_insert_with(|| PlayerRunScore {
                        start_tick: game_tick.0,
                        ..PlayerRunScore::default()
                    });
            run_score.legendary_kills += 1;
            run_score.hideouts_cleared += 1;

            objectives
                .entry(boss.player_id)
                .or_insert_with(PlayerObjectives::default)
                .defeat_ashen_warlord = true;

            if let Some(threat) = legendary_threat_state.get_mut(&boss.player_id) {
                threat.active = false;
                threat.defeated = true;
                threat.defeated_at_tick = Some(game_tick.0);
                reveal_legendary_hideout(
                    boss.player_id,
                    threat,
                    &mut objectives,
                    &clients,
                    "Legendary defeated",
                );
            }

            let packet = ResponsePacket::DiscoveryEvent {
                version: 1,
                discovery_type: "legendary_victory".to_string(),
                title: "Fire Dragon defeated".to_string(),
                unlock_source: "Hideout cleared".to_string(),
                location: None,
                result: format!(
                    "The Fire Dragon falls. The raids stop, and your legend grows around kill {}.",
                    id.0
                ),
            };
            send_to_client(boss.player_id, packet, &clients);
        }
    }
}

fn run_score_kill_tracking_system(
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    dead_query: Query<
        (&Template, Option<&LastAttacker>),
        (
            With<SubclassNPC>,
            Added<StateDead>,
            Without<LegendaryFollower>,
            Without<LegendaryBoss>,
        ),
    >,
    mut run_score_state: ResMut<RunScoreState>,
) {
    for (template, last_attacker) in dead_query.iter() {
        let Some(last_attacker) = last_attacker else {
            continue;
        };
        let Some(player_id) = ids.get_player(last_attacker.id) else {
            continue;
        };
        if !player::is_player(player_id) {
            continue;
        }
        if is_player_offline_protected(player_id, &presence) {
            continue;
        }

        let run_score = run_score_state
            .entry(player_id)
            .or_insert_with(|| PlayerRunScore {
                start_tick: game_tick.0,
                ..PlayerRunScore::default()
            });
        run_score.enemies_killed += 1;

        if is_elite_enemy_template(&template.0) {
            run_score.elites_killed += 1;
        }
    }
}

fn reduce_wildness_at_pos(map: &mut Map, pos: Position) -> bool {
    if !Map::is_valid_pos((pos.x, pos.y)) || map.get_wildness(pos.x, pos.y) <= 0 {
        return false;
    }

    map.update_wildness(pos.x, pos.y, -1);
    true
}

// Default/maximum wildness a tile drifts back toward (matches the spawn-time init).
const WILDNESS_MAX: i32 = 4;
// How often the wilderness reclaims pacified ground. Every interval a cleared tile
// gains +1 wildness, so clearing a tile buys ~WILDNESS_MAX intervals of calm before
// random spawns return there. Tiles inside a sanctuary are held at 0.
const WILDNESS_REGEN_INTERVAL: i32 = 600;

// Slowly regrow wildness on tiles the player has pacified (so the wilderness stays
// dangerous all game), while keeping sanctuary ground suppressed. This is what makes
// "clear the area" temporary outside the zone and permanent inside it.
fn wildness_regen_system(
    game_tick: Res<GameTick>,
    mut map: ResMut<Map>,
    sanctuary_zones: Res<SanctuaryZones>,
) {
    if game_tick.0 % WILDNESS_REGEN_INTERVAL != 0 {
        return;
    }

    for y in 0..crate::map::HEIGHT {
        for x in 0..crate::map::WIDTH {
            let pos = Position { x, y };
            let w = map.get_wildness(x, y);
            if sanctuary_zones.in_full_zone(pos) {
                // Inside the sanctuary the ground stays pacified.
                if w > 0 {
                    map.update_wildness(x, y, -w);
                }
            } else if w < WILDNESS_MAX {
                map.update_wildness(x, y, 1);
            }
        }
    }
}

fn wildness_reduction_on_enemy_death_system(
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut map: ResMut<Map>,
    dead_query: Query<
        (&Position, Option<&LastAttacker>),
        (
            With<SubclassNPC>,
            Added<StateDead>,
            Without<LegendaryFollower>,
            Without<LegendaryBoss>,
        ),
    >,
    monolith_query: Query<&Position, With<Monolith>>,
) {
    let monolith_positions = monolith_query.iter().copied().collect::<Vec<_>>();

    for (pos, last_attacker) in dead_query.iter() {
        let Some(last_attacker) = last_attacker else {
            continue;
        };
        let Some(player_id) = ids.get_player(last_attacker.id) else {
            continue;
        };
        if !player::is_player(player_id)
            || is_player_offline_protected(player_id, &presence)
            || !outside_weak_sanctuary_from_monolith_positions(*pos, &monolith_positions)
        {
            continue;
        }

        if reduce_wildness_at_pos(&mut map, *pos) {
            info!(
                "Player {} reduced wildness at {:?} by killing an enemy",
                player_id, pos
            );
        }
    }
}

fn atmospheric_event_message(player_day: i32, ticks_in_day: i32) -> Option<&'static str> {
    match (player_day, ticks_in_day) {
        // Day 1 morning hints
        (1, 700..=800) => Some(
            "The shipwreck groans as waves crash against the hull. There may be supplies worth salvaging.",
        ),
        // Day 1 afternoon
        (1, 1400..=1500) => {
            Some("Smoke rises from somewhere in the distance. You are not alone on this island.")
        }
        // Day 2 morning
        (2, 600..=700) => {
            Some("Strange markings on the rocks point toward the interior of the island.")
        }
        // Day 2 afternoon
        (2, 1300..=1400) => {
            Some("The Monolith pulses faintly. Its power draws the attention of the dead.")
        }
        // Day 3 morning
        (3, 600..=700) => {
            Some("A cold wind blows from the mountains. The creatures grow bolder each night.")
        }
        // Day 3 evening
        (3, 1700..=1800) => {
            Some("The ground near the graveyard trembles. Something stirs beneath.")
        }
        // Day 4
        (4, 600..=700) => {
            Some("Footprints in the mud suggest scouts have been watching your settlement.")
        }
        // Day 5
        (5, 600..=700) => {
            Some("The island's dangers grow. Your settlement must be strong to survive.")
        }
        // Day 7+, every morning
        (d, 600..=699) if d >= 7 && d % 2 == 1 => {
            Some("The darkness beyond your walls grows deeper. Fortify your defenses.")
        }
        _ => None,
    }
}

// Periodic map events: atmospheric discoveries and world events
fn map_event_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    hero_query: Query<(&PlayerId, &Position), With<SubclassHero>>,
    player_intro_state: Res<PlayerIntroState>,
    presence: Res<PlayerWorldPresenceState>,
    mut last_event_tick: Local<HashMap<i32, i32>>,
) {
    // Check every 100 ticks (10 seconds)
    if game_tick.0 % 100 != 0 {
        return;
    }

    let ticks_in_day = game_tick.0 % GAME_TICKS_PER_DAY;

    for (player_id, _pos) in hero_query.iter() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        let last_tick = last_event_tick.get(&player_id.0).copied().unwrap_or(0);

        // Don't send events too frequently — at least 600 ticks (1 minute) between events
        if game_tick.0 - last_tick < 600 {
            continue;
        }

        let player_day = player_survival_day(&game_tick, player_id.0, &player_intro_state);
        let event_message = atmospheric_event_message(player_day, ticks_in_day);

        if let Some(message) = event_message {
            let packet = ResponsePacket::Notice {
                noticemsg: message.to_string(),
                expiry: Some(10000),
            };
            send_to_client(player_id.0, packet, &clients);
            last_event_tick.insert(player_id.0, game_tick.0);
        }
    }
}

// Checks objective completion and sends updates to players
fn objectives_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    hero_query: Query<(&PlayerId, &Position), With<SubclassHero>>,
    structure_query: Query<(&PlayerId, &Template), With<ClassStructure>>,
    storage_query: Query<(&PlayerId, &Position, &Inventory), With<Storage>>,
    villager_query: Query<&PlayerId, With<SubclassVillager>>,
    initial_encounter_state: Res<InitialEncounterState>,
    dead_query: Query<&Id, With<StateDead>>,
    spawn_positions: Res<SpawnPositions>,
    crisis_state: Res<CrisisState>,
    legendary_threat_state: Res<LegendaryThreatState>,
    player_intro_state: Res<PlayerIntroState>,
    presence: Res<PlayerWorldPresenceState>,
    mut objectives: ResMut<Objectives>,
    mut run_score_state: ResMut<RunScoreState>,
) {
    // Check every 50 ticks (5 seconds)
    if game_tick.0 % 50 != 0 {
        return;
    }

    let dead_ids: HashSet<i32> = dead_query.iter().map(|id| id.0).collect();

    for (player_id, hero_pos) in hero_query.iter() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        let player_day = player_survival_day(&game_tick, player_id.0, &player_intro_state);
        let player_structures: Vec<String> = structure_query
            .iter()
            .filter(|(pid, _)| pid.0 == player_id.0)
            .map(|(_, template)| template.0.clone())
            .collect();
        let structure_count = player_structures.len();
        let has_campfire = player_structures.iter().any(|name| name == "Campfire");
        let has_shelter = player_structures.iter().any(|name| {
            name == "Small Tent" || name == "Large Tent" || name == "Yurt" || name == "Large Yurt"
        });
        let has_expansion = player_structures.iter().any(|name| {
            matches!(
                name.as_str(),
                "Crafting Tent" | "Mine" | "Lumbercamp" | "Quarry" | "Trapper" | "Farm"
            )
        });
        let starting_enemies_dead = initial_encounter_state
            .get(&player_id.0)
            .map(|entry| entry.rat_ids.iter().all(|id| dead_ids.contains(id)))
            .unwrap_or(false);

        let obj = objectives
            .entry(player_id.0)
            .or_insert_with(PlayerObjectives::default);

        // Check: Build a Campfire
        if !obj.build_campfire && has_campfire {
            obj.build_campfire = true;
            // BB-B: action-driven nudge — confirm + point to the next danger/step.
            send_to_client(
                player_id.0,
                ResponsePacket::Notice {
                    noticemsg: "Your fire is lit — it wards the night and reveals what creeps in the dark. Keep your weapon close.".to_string(),
                    expiry: Some(10000),
                },
                &clients,
            );
        }

        if !obj.win_first_fight && starting_enemies_dead {
            obj.win_first_fight = true;
            send_to_client(
                player_id.0,
                ResponsePacket::Notice {
                    noticemsg: "You held your ground. Gather wood and food, then raise walls before night falls.".to_string(),
                    expiry: Some(10000),
                },
                &clients,
            );
        }

        // Check: Build 3 Structures
        if !obj.build_3_structures && structure_count >= 3 {
            obj.build_3_structures = true;
            send_to_client(
                player_id.0,
                ResponsePacket::Notice {
                    noticemsg: "A real camp takes shape. Each building should answer a need — rest, storage, defense.".to_string(),
                    expiry: Some(10000),
                },
                &clients,
            );
        }

        // Check: Recruit a Villager
        if !obj.recruit_villager {
            for villager_pid in villager_query.iter() {
                if villager_pid.0 == player_id.0 {
                    obj.recruit_villager = true;
                    send_discovery_event(
                        player_id.0,
                        "villager",
                        "A settler joins you",
                        "Shipwreck rescue",
                        None,
                        "Villagers turn survival into a settlement: assign repeatable work and protect their needs.",
                        &clients,
                    );
                    break;
                }
            }
        }

        if !obj.choose_expansion && has_expansion {
            obj.choose_expansion = true;
            send_discovery_event(
                player_id.0,
                "progression",
                "Expansion path chosen",
                "Settlement plan",
                None,
                "Resource camps feed crafting; crafting structures turn discoveries into stronger tools and defenses.",
                &clients,
            );
        }

        // Check: Survive 5 Nights (day >= 6 means survived 5 nights)
        if !obj.survive_5_nights && player_day >= 6 {
            obj.survive_5_nights = true;
            send_discovery_event(
                player_id.0,
                "survival",
                "Five nights survived",
                "Island pressure",
                None,
                "Longer survival raises the stakes. Watch threat pressure before storing wealth or pushing deep inland.",
                &clients,
            );
        }

        // Always send current state — ensures reconnecting clients get objectives
        let packet = ResponsePacket::Objectives {
            build_campfire: obj.build_campfire,
            build_3_structures: obj.build_3_structures,
            recruit_villager: obj.recruit_villager,
            explore_poi: obj.explore_poi,
            survive_5_nights: obj.survive_5_nights,
        };
        send_to_client(player_id.0, packet, &clients);

        let objective_state_packet = build_objective_state_packet(
            obj,
            structure_count as i32,
            has_shelter,
            starting_enemies_dead,
            player_day,
        );
        send_to_client(player_id.0, objective_state_packet, &clients);

        let threat_state_packet = build_threat_state_packet(
            player_id.0,
            hero_pos,
            &game_tick,
            player_day,
            &storage_query,
            &spawn_positions,
            &crisis_state,
            &legendary_threat_state,
        );
        if let ResponsePacket::ThreatState { pressure_level, .. } = &threat_state_packet {
            let pressure_value = pressure_level_value(pressure_level);
            let run_score = run_score_state
                .entry(player_id.0)
                .or_insert_with(|| PlayerRunScore {
                    start_tick: game_tick.0,
                    ..PlayerRunScore::default()
                });
            run_score.highest_pressure_level = run_score.highest_pressure_level.max(pressure_value);
        }
        send_to_client(player_id.0, threat_state_packet, &clients);
    }
}

fn send_discovery_event(
    player_id: i32,
    discovery_type: &str,
    title: &str,
    unlock_source: &str,
    location: Option<String>,
    result: &str,
    clients: &Res<Clients>,
) {
    let packet = ResponsePacket::DiscoveryEvent {
        version: 1,
        discovery_type: discovery_type.to_string(),
        title: title.to_string(),
        unlock_source: unlock_source.to_string(),
        location,
        result: result.to_string(),
    };
    send_to_client(player_id, packet, clients);
}

fn objective_progress(
    id: &str,
    title: &str,
    done: bool,
    current_id: &str,
    category: &str,
    target: Option<&str>,
    action_hint: &str,
    lesson: &str,
    reward: &str,
    progress: Option<i32>,
    goal: Option<i32>,
) -> network::ObjectiveProgress {
    let state = if done {
        "complete"
    } else if id == current_id {
        "active"
    } else {
        "locked"
    };

    network::ObjectiveProgress {
        id: id.to_string(),
        title: title.to_string(),
        state: state.to_string(),
        category: category.to_string(),
        target: target.map(|value| value.to_string()),
        action_hint: action_hint.to_string(),
        lesson: lesson.to_string(),
        reward: reward.to_string(),
        progress,
        goal,
    }
}

fn first_incomplete_objective_id(obj: &PlayerObjectives, has_shelter: bool) -> String {
    if !obj.scavenge_shipwreck {
        return "scavenge_shipwreck".to_string();
    }
    if !obj.build_campfire {
        return "build_campfire".to_string();
    }
    if !obj.win_first_fight {
        return "win_first_fight".to_string();
    }
    if !obj.recruit_villager {
        return "recruit_villager".to_string();
    }
    if !has_shelter && !obj.build_3_structures {
        return "build_shelter_storage".to_string();
    }
    if !obj.choose_expansion {
        return "choose_expansion".to_string();
    }
    if !obj.survive_5_nights {
        return "survive_5_nights".to_string();
    }
    if !obj.find_legendary_hideout {
        return "find_legendary_hideout".to_string();
    }
    if !obj.defeat_ashen_warlord {
        return "defeat_ashen_warlord".to_string();
    }

    "complete".to_string()
}

fn build_objective_state_packet(
    obj: &PlayerObjectives,
    structure_count: i32,
    has_shelter: bool,
    starting_enemies_dead: bool,
    day: i32,
) -> ResponsePacket {
    let current_id = first_incomplete_objective_id(obj, has_shelter);
    let objectives = vec![
        objective_progress(
            "scavenge_shipwreck",
            "Scavenge the shipwreck",
            obj.scavenge_shipwreck,
            &current_id,
            "First Hour",
            Some("Shipwreck"),
            "Inspect the wreck and move useful supplies into your camp.",
            "The island starts as a pattern hunt: inspect, learn, take what solves the next danger.",
            "Starting supplies, POI awareness, and your first survival clue.",
            None,
            None,
        ),
        objective_progress(
            "build_campfire",
            "Build a campfire",
            obj.build_campfire,
            &current_id,
            "Settlement",
            Some("Campfire"),
            "Use Stick and Resin to place and build a Campfire near your burrow.",
            "Light is safety: it expands what you can see and makes night pressure legible.",
            "Warmth, vision, and a center for the first camp.",
            None,
            None,
        ),
        objective_progress(
            "win_first_fight",
            "Win the first fight",
            obj.win_first_fight || starting_enemies_dead,
            &current_id,
            "Combat",
            Some("Cave Bat"),
            "Read the threat, use quick attacks for control, and block if you need time.",
            "Combat is a grammar: quick, precise, fierce, and block each mean something.",
            "XP, loot, and confidence to leave the firelight.",
            None,
            None,
        ),
        objective_progress(
            "recruit_villager",
            "Rescue a settler",
            obj.recruit_villager,
            &current_id,
            "Villager",
            Some("Shipwreck survivor"),
            "Keep the survivor alive and use their lookout knowledge to improve camp safety.",
            "Villagers are not just workers; they reveal needs, skills, and settlement loops.",
            "Watchtower plan and another pair of hands.",
            None,
            None,
        ),
        objective_progress(
            "build_shelter_storage",
            "Solve rest and storage",
            has_shelter || obj.build_3_structures,
            &current_id,
            "Settlement",
            Some("Shelter or storage"),
            "Build shelter for fatigue and storage for supplies, then add a defensive structure.",
            "Buildings should answer visible problems, not just fill space.",
            "A camp that can survive work, weather, and the next night.",
            Some(structure_count.min(3)),
            Some(3),
        ),
        objective_progress(
            "choose_expansion",
            "Choose an expansion path",
            obj.choose_expansion,
            &current_id,
            "Progression",
            Some("Crafting Tent or resource camp"),
            "Build a Crafting Tent, Mine, Lumbercamp, Quarry, Trapper, or Farm.",
            "Expansion creates strategy: gather better, craft better, defend better.",
            "A repeatable resource or crafting loop.",
            None,
            None,
        ),
        objective_progress(
            "survive_5_nights",
            "Survive five nights",
            obj.survive_5_nights,
            &current_id,
            "Survival",
            Some("Nightfall"),
            "Use warnings, walls, light, gear, and villagers to prepare before dusk.",
            "Threats are pressure signals. Mastery means preparing before the attack.",
            "A stable foothold and a score worth remembering.",
            Some((day - 1).clamp(0, 5)),
            Some(5),
        ),
        objective_progress(
            "find_legendary_hideout",
            "Find the source of the raids",
            obj.find_legendary_hideout,
            &current_id,
            "Legendary Threat",
            Some("Fire Dragon"),
            "Defeat captains, break follower waves, or scout inland until the hideout is revealed.",
            "Late survival is not only defense: pressure has a source, and veterans hunt it.",
            "The Fire Dragon's hideout location.",
            None,
            None,
        ),
        objective_progress(
            "defeat_ashen_warlord",
            "Eliminate the Fire Dragon",
            obj.defeat_ashen_warlord,
            &current_id,
            "Legendary Threat",
            Some("Dragon Hideout"),
            "Prepare supplies, breach the hideout, and defeat the Fire Dragon to stop its followers.",
            "A legendary enemy is a campaign, not a single fight.",
            "Follower raids stop and your final score gains a major valor bonus.",
            None,
            None,
        ),
    ];

    ResponsePacket::ObjectiveState {
        version: 1,
        current_id,
        objectives,
    }
}

fn risk_severity(current: i32, threshold: i32) -> String {
    if current >= threshold {
        "high".to_string()
    } else if current >= (threshold * 2) / 3 {
        "medium".to_string()
    } else if current > 0 {
        "low".to_string()
    } else {
        "quiet".to_string()
    }
}

fn build_threat_state_packet(
    player_id: i32,
    hero_pos: &Position,
    game_tick: &GameTick,
    player_day: i32,
    storage_query: &Query<(&PlayerId, &Position, &Inventory), With<Storage>>,
    spawn_positions: &SpawnPositions,
    crisis_state: &CrisisState,
    legendary_threat_state: &LegendaryThreatState,
) -> ResponsePacket {
    let mut food_stored = 0;
    let mut gold_stored = 0;

    for (storage_player_id, _pos, inventory) in storage_query.iter() {
        if storage_player_id.0 != player_id {
            continue;
        }

        gold_stored += inventory.get_total_gold();
        for item in inventory.items.iter() {
            if item.class == FOOD {
                food_stored += item.quantity;
            }
        }
    }

    let distance_from_spawn = spawn_positions
        .get(&player_id)
        .map(|spawn| Map::dist(*spawn, *hero_pos))
        .unwrap_or(0) as i32;
    let ticks_in_day = game_tick.0.rem_euclid(GAME_TICKS_PER_DAY);
    let ticks_to_dusk = if ticks_in_day <= DUSK {
        DUSK - ticks_in_day
    } else {
        GAME_TICKS_PER_DAY - ticks_in_day + DUSK
    };
    let nights_survived = (player_day - 1).max(0);

    let known_risks = vec![
        network::ThreatRisk {
            id: "food_spoilage".to_string(),
            label: "Food stores attract pests".to_string(),
            severity: risk_severity(food_stored, 20),
            trigger_hint: "20 food in storage can trigger a pest spoilage crisis.".to_string(),
            counter_hint: "Cook, spend, split, or defend food stores before they pile up."
                .to_string(),
            current: Some(food_stored),
            threshold: Some(20),
        },
        network::ThreatRisk {
            id: "gold_raid".to_string(),
            label: "Gold attracts raiders".to_string(),
            severity: risk_severity(gold_stored, 30),
            trigger_hint: "30 gold in storage can draw mounted thieves.".to_string(),
            counter_hint: "Build walls, watchtowers, or invest gold before hoarding it."
                .to_string(),
            current: Some(gold_stored),
            threshold: Some(30),
        },
        network::ThreatRisk {
            id: "wilderness_distance".to_string(),
            label: "Distance from camp raises danger".to_string(),
            severity: risk_severity(distance_from_spawn, 8),
            trigger_hint: "Traveling 8 tiles from spawn can draw a wolf pack.".to_string(),
            counter_hint: "Scout with stamina, light, and an escape path.".to_string(),
            current: Some(distance_from_spawn),
            threshold: Some(8),
        },
        network::ThreatRisk {
            id: "nightfall".to_string(),
            label: "Nightfall pressure".to_string(),
            severity: if ticks_to_dusk <= 300 {
                "high"
            } else if ticks_to_dusk <= 700 {
                "medium"
            } else {
                "low"
            }
            .to_string(),
            trigger_hint: "Dusk brings nightly pressure after the first day.".to_string(),
            counter_hint: "Return to light, finish building, and keep villagers close before dusk."
                .to_string(),
            current: Some(ticks_to_dusk),
            threshold: Some(300),
        },
    ];

    let crisis = crisis_state.get(&player_id).cloned().unwrap_or_default();
    let active_crisis_count = crisis_tier(&crisis);
    let active_legendary_count = legendary_threat_state
        .get(&player_id)
        .map(|threat| {
            if threat.active && !threat.defeated {
                1
            } else {
                0
            }
        })
        .unwrap_or(0);
    let high_risks = known_risks
        .iter()
        .filter(|risk| risk.severity == "high")
        .count() as i32;
    let pressure_score = high_risks
        + active_crisis_count
        + active_legendary_count
        + if nights_survived >= 3 { 1 } else { 0 };
    let pressure_level = match pressure_score {
        0 => "Calm",
        1 => "Building",
        2 => "High",
        _ => "Crisis",
    }
    .to_string();

    let next_night_warning = if player_day <= 1 {
        "First day: learn the camp loop before the island pushes back.".to_string()
    } else if ticks_to_dusk <= 300 {
        "Dusk is close. Stop long errands and prepare the camp.".to_string()
    } else {
        "Use daylight to solve the highest visible risk before nightfall.".to_string()
    };

    ResponsePacket::ThreatState {
        version: 1,
        day: player_day,
        phase: game_tick.time_of_day(),
        pressure_level,
        next_night_warning,
        known_risks,
        legendary_threats: legendary_threat_packets(player_id, game_tick, legendary_threat_state),
    }
}

// Weather cycling: generates weather areas each day at DAWN, clears old weather
fn weather_cycle_system(
    game_tick: Res<GameTick>,
    mut weather_areas: ResMut<WeatherAreas>,
    map: Res<Map>,
    clients: Res<Clients>,
    hero_query: Query<&PlayerId, With<SubclassHero>>,
) {
    let ticks_in_day = game_tick.0 % GAME_TICKS_PER_DAY;

    // Generate new weather at DAWN each day
    if ticks_in_day != DAWN {
        return;
    }

    let current_day = game_tick.day();

    // Clear old weather
    weather_areas.clear();

    let mut rng = rand::thread_rng();

    // Day 1: mild weather only
    // Day 2-3: rain/fog possible
    // Day 4+: full range including cold/heat/storms
    let possible_weathers: Vec<Weather> = match current_day {
        1 => vec![Weather::ClearSunny, Weather::Fog],
        2..=3 => vec![Weather::ClearSunny, Weather::HeavyRain, Weather::Fog],
        4..=6 => vec![
            Weather::ClearSunny,
            Weather::HeavyRain,
            Weather::Fog,
            Weather::ColdSnap,
            Weather::Snow,
            Weather::Heatwave,
            Weather::Thunderstorm,
        ],
        _ => vec![
            Weather::ClearSunny,
            Weather::HeavyRain,
            Weather::Fog,
            Weather::ColdSnap,
            Weather::Snow,
            Weather::Blizzard,
            Weather::Heatwave,
            Weather::Thunderstorm,
        ],
    };

    // Generate 1-3 weather areas
    let num_areas = rng.gen_range(1..=3);
    for _ in 0..num_areas {
        let weather = possible_weathers[rng.gen_range(0..possible_weathers.len())].clone();

        // Skip clear weather — no area needed
        if matches!(weather, Weather::ClearSunny) {
            continue;
        }

        let x = rng.gen_range(2..map.width - 2);
        let y = rng.gen_range(2..map.height - 2);

        let area = crate::world::create_weather_area(x, y, weather);
        weather_areas.push(area);
    }

    // Notify players about weather conditions
    // NOTE: Weather report notices are temporarily disabled to reduce early-game
    // notification noise. Revisit later (the weather areas themselves still spawn
    // and apply gameplay effects; only the toast is suppressed).
    // if !weather_areas.is_empty() {
    //     let weather_names: Vec<String> = weather_areas
    //         .iter()
    //         .map(|w| w.weather.to_string())
    //         .collect();
    //     let unique_names: Vec<String> = weather_names
    //         .into_iter()
    //         .collect::<std::collections::HashSet<_>>()
    //         .into_iter()
    //         .collect();
    //     let weather_msg = format!(
    //         "Weather report: {} observed on the island.",
    //         unique_names.join(", ")
    //     );
    //
    //     for player_id in hero_query.iter() {
    //         let packet = ResponsePacket::Notice {
    //             noticemsg: weather_msg.clone(),
    //             expiry: Some(8000),
    //         };
    //         send_to_client(player_id.0, packet, &clients);
    //     }
    // }
}

// Weather effects: applies gameplay effects based on weather at entity positions
fn weather_effects_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    weather_areas: Res<WeatherAreas>,
    clients: Res<Clients>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut hero_query: Query<
        (
            Entity,
            &Id,
            &PlayerId,
            &Position,
            &mut Stats,
            Option<&Sheltered>,
        ),
        With<SubclassHero>,
    >,
    campfire_query: Query<(&Position, &Campfire)>,
    mut structure_query: Query<
        (&Id, &PlayerId, &Position, &mut Stats, &Template),
        (With<ClassStructure>, Without<SubclassHero>),
    >,
    mut crops: ResMut<Crops>,
    mut last_weather_notice: Local<HashMap<i32, i32>>,
) {
    // Check every 50 ticks (5 seconds)
    if game_tick.0 % 50 != 0 {
        return;
    }

    if weather_areas.is_empty() {
        return;
    }

    // Process hero weather effects
    for (entity, id, player_id, pos, mut stats, sheltered) in hero_query.iter_mut() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        let Some(weather) = weather_areas.get_weather_at(pos.x, pos.y) else {
            continue;
        };

        let is_sheltered = sheltered.is_some();
        let near_campfire = campfire_query.iter().any(|(cf_pos, cf)| {
            cf.is_lit && Map::distance((pos.x, pos.y), (cf_pos.x, cf_pos.y)) <= 2
        });

        // Cold weather: damage if not sheltered and not near campfire
        if weather.is_cold() && !is_sheltered && !near_campfire {
            let cold_damage = match weather {
                Weather::Blizzard | Weather::PolarVortex => 3,
                Weather::Snow | Weather::IceStorm => 2,
                _ => 1,
            };
            let previous_hp = stats.hp;
            stats.hp = (stats.hp - cold_damage).max(0);
            if stats.hp < previous_hp {
                commands
                    .entity(entity)
                    .try_insert(LastDamageTick(game_tick.0));
            }

            let last_tick = last_weather_notice.get(&player_id.0).copied().unwrap_or(0);
            if game_tick.0 - last_tick >= 300 {
                let packet = ResponsePacket::Notice {
                    noticemsg: format!(
                        "The {} chills you to the bone! Seek shelter or warmth.",
                        weather.to_string()
                    ),
                    expiry: Some(5000),
                };
                send_to_client(player_id.0, packet, &clients);
                last_weather_notice.insert(player_id.0, game_tick.0);
            }
        }

        // Heatwave: notice about increased thirst (actual thirst is handled per-tick elsewhere)
        if weather.is_hot() {
            let last_tick = last_weather_notice.get(&player_id.0).copied().unwrap_or(0);
            if game_tick.0 - last_tick >= 600 {
                let packet = ResponsePacket::Notice {
                    noticemsg: "The oppressive heat drains your energy. Stay hydrated!".to_string(),
                    expiry: Some(5000),
                };
                send_to_client(player_id.0, packet, &clients);
                last_weather_notice.insert(player_id.0, game_tick.0);
            }
        }

        // Fog: reduced vision notice
        if weather.is_fog() {
            let last_tick = last_weather_notice.get(&player_id.0).copied().unwrap_or(0);
            if game_tick.0 - last_tick >= 600 {
                let packet = ResponsePacket::Notice {
                    noticemsg: "A thick fog rolls in, obscuring your vision.".to_string(),
                    expiry: Some(5000),
                };
                send_to_client(player_id.0, packet, &clients);
                last_weather_notice.insert(player_id.0, game_tick.0);
            }
        }

        // Rain: can extinguish campfires (small chance per check)
        if weather.is_rainy() {
            let last_tick = last_weather_notice.get(&player_id.0).copied().unwrap_or(0);
            if game_tick.0 - last_tick >= 600 {
                let packet = ResponsePacket::Notice {
                    noticemsg: "Rain pounds the ground. Fires may be extinguished!".to_string(),
                    expiry: Some(5000),
                };
                send_to_client(player_id.0, packet, &clients);
                last_weather_notice.insert(player_id.0, game_tick.0);
            }
        }
    }

    // Storm damage to structures
    for (id, _player_id, pos, mut stats, _template) in structure_query.iter_mut() {
        if object_belongs_to_protected_run(id.0, &ids, &presence) {
            continue;
        }
        let Some(weather) = weather_areas.get_weather_at(pos.x, pos.y) else {
            continue;
        };

        if weather.is_storm() {
            let storm_damage = match weather {
                Weather::Hurricane | Weather::SuperTyphoon | Weather::Tornado => 5,
                Weather::Thunderstorm | Weather::Moonsoon => 2,
                _ => 1,
            };
            // Only damage structures with HP < 500 (low-HP structures)
            if stats.base_hp < 500 {
                stats.hp = (stats.hp - storm_damage).max(0);
            }
        }
    }

    // Rain boosts crop growth: reduce remaining growth time
    if weather_areas.iter().any(|w| w.weather.is_rainy()) {
        for (structure_id, crop) in crops.iter_mut() {
            if object_belongs_to_protected_run(*structure_id, &ids, &presence) {
                continue;
            }
            // Boost growth by reducing stage_end (effectively 50% faster when checked every 50 ticks)
            if crop.stage != CropStages::Mature && crop.stage != CropStages::Dead {
                // Shave 25 ticks off remaining time per check (50% boost over natural 50-tick intervals)
                if crop.stage_end > game_tick.0 + 1 {
                    crop.stage_end -= 25;
                }
            }
        }
    }
}

// Villager morale system: updates morale based on conditions and triggers dialogue
fn morale_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    templates: Res<Templates>,
    mut map_events: ResMut<MapEvents>,
    weather_areas: Res<WeatherAreas>,
    presence: Res<PlayerWorldPresenceState>,
    mut villager_query: Query<
        (&Id, &PlayerId, &Position, &mut Morale, Option<&Sheltered>),
        With<SubclassVillager>,
    >,
    shelter_query: Query<Entity, With<Shelter>>,
    structure_query: Query<(&PlayerId, &Inventory), With<ClassStructure>>,
    dead_query: Query<(&PlayerId, &StateDead)>,
    hero_query: Query<(&PlayerId, &Position), With<SubclassHero>>,
    mut last_dialogue_tick: Local<HashMap<i32, i32>>,
) {
    // Update morale every 200 ticks (20 seconds)
    if game_tick.0 % 200 != 0 {
        return;
    }

    for (id, player_id, pos, mut morale, sheltered) in villager_query.iter_mut() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        let mut new_morale: f32 = 50.0; // Base morale

        // +20 if sheltered
        if sheltered.is_some() {
            new_morale += 20.0;
        } else {
            new_morale -= 20.0;
        }

        // +10 if food available in player's structures
        let has_food = structure_query
            .iter()
            .any(|(pid, inv)| pid.0 == player_id.0 && inv.get_by_class(FOOD.to_string()).is_some());
        if has_food {
            new_morale += 10.0;
        } else {
            new_morale -= 10.0;
        }

        // -15 if any ally died recently (within last 600 ticks / 1 min)
        let recent_death = dead_query
            .iter()
            .any(|(pid, dead)| pid.0 == player_id.0 && (game_tick.0 - dead.dead_at) < 600);
        if recent_death {
            new_morale -= 15.0;
        }

        // Weather affects morale
        if let Some(weather) = weather_areas.get_weather_at(pos.x, pos.y) {
            if weather.is_cold() {
                new_morale -= 10.0;
            }
            if weather.is_hot() {
                new_morale -= 5.0;
            }
            if weather.is_rainy() {
                new_morale -= 5.0;
            }
        }

        new_morale -= morale.rough_sleep_penalty;

        // Clamp morale to 0-100
        new_morale = new_morale.clamp(0.0, 100.0);
        morale.morale = new_morale;

        // Trigger dialogue based on morale and world state
        let last_tick = last_dialogue_tick.get(&id.0).copied().unwrap_or(0);
        if game_tick.0 - last_tick < 1200 {
            continue; // Don't spam dialogue — at most once per 2 minutes
        }

        let dialogue = if new_morale >= 75.0 {
            // Happy villager comments
            let options = vec![
                "This settlement grows stronger every day!",
                "Fine weather for working!",
                "I feel safe here. Good leadership.",
                "Another good day at the settlement.",
            ];
            Some(options[game_tick.0 as usize % options.len()])
        } else if new_morale <= 25.0 {
            // Miserable villager complaints
            let options = vec![
                "I'm cold and hungry...",
                "When will this nightmare end?",
                "We need better shelter...",
                "I don't know how much longer I can take this.",
            ];
            Some(options[game_tick.0 as usize % options.len()])
        } else if let Some(weather) = weather_areas.get_weather_at(pos.x, pos.y) {
            // Weather comments at medium morale
            if weather.is_cold() {
                Some("Brrr... this cold is unbearable.")
            } else if weather.is_rainy() {
                Some("This rain won't let up!")
            } else if weather.is_fog() {
                Some("Can barely see my hand in this fog...")
            } else {
                None
            }
        } else {
            None
        };

        if let Some(speech) = dialogue {
            Obj::add_speech_event(game_tick.0, speech.to_string(), id, &mut map_events);
            last_dialogue_tick.insert(id.0, game_tick.0);
        }
    }
}

// Monolith investigation stages per player
#[derive(Debug, Clone, Default)]
pub struct MonolithProgress {
    pub stage: i32, // 0 = unvisited, 1 = observed, 2 = offering made, 3 = sealed/empowered
    pub sealed: bool,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct MonolithInvestigation(pub HashMap<i32, MonolithProgress>);

// Victory tracking per player
#[derive(Debug, Clone, Default, Serialize)]
pub struct PlayerVictory {
    pub rescue_progress: i32, // Ticks survived (for day counting)
    pub prosperity: bool,
    pub conquest: bool,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct VictoryState(pub HashMap<i32, PlayerVictory>);

fn rescue_victory_ready(player_day: i32, victory: &PlayerVictory) -> bool {
    // Rescue arrives after surviving 50 full days (i.e. on day 51).
    player_day >= 51 && victory.rescue_progress == 0
}

// Victory condition check system
fn victory_check_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    hero_query: Query<(&PlayerId, &Position), With<SubclassHero>>,
    structure_query: Query<(&PlayerId, &Template), With<ClassStructure>>,
    villager_query: Query<&PlayerId, With<SubclassVillager>>,
    objectives: Res<Objectives>,
    monolith_investigation: Res<MonolithInvestigation>,
    player_intro_state: Res<PlayerIntroState>,
    presence: Res<PlayerWorldPresenceState>,
    mut victory_state: ResMut<VictoryState>,
) {
    // Check every 200 ticks (20 seconds)
    if game_tick.0 % 200 != 0 {
        return;
    }

    for (player_id, _pos) in hero_query.iter() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        let victory = victory_state
            .entry(player_id.0)
            .or_insert_with(PlayerVictory::default);

        // Skip if already achieved a victory
        if victory.prosperity || victory.conquest {
            continue;
        }

        // Rescue: Survive 10 days
        let player_day = player_survival_day(&game_tick, player_id.0, &player_intro_state);
        if rescue_victory_ready(player_day, victory) {
            victory.rescue_progress = 1;
            let packet = ResponsePacket::Notice {
                noticemsg: "VICTORY! You have survived 50 days on the island. A passing ship spots your settlement and sends a rescue party! You may continue playing or celebrate your achievement.".to_string(),
                expiry: Some(30000),
            };
            send_to_client(player_id.0, packet, &clients);
        }

        // Prosperity: 10+ structures, 3+ villagers
        let structure_count = structure_query
            .iter()
            .filter(|(pid, _)| pid.0 == player_id.0)
            .count();
        let villager_count = villager_query
            .iter()
            .filter(|pid| pid.0 == player_id.0)
            .count();

        if structure_count >= 10 && villager_count >= 3 && !victory.prosperity {
            victory.prosperity = true;
            let packet = ResponsePacket::Notice {
                noticemsg: "VICTORY — PROSPERITY! Your thriving settlement of {:} structures and {:} villagers stands as a testament to your leadership! You may continue playing.".to_string()
                    .replace("{:}", &structure_count.to_string())
                    .replace("{:}", &villager_count.to_string()),
                expiry: Some(30000),
            };
            send_to_client(player_id.0, packet, &clients);
        }

        // Conquest: All POIs explored + Monolith sealed
        if let Some(obj) = objectives.get(&player_id.0) {
            if obj.explore_poi {
                if let Some(monolith) = monolith_investigation.get(&player_id.0) {
                    if monolith.sealed && !victory.conquest {
                        victory.conquest = true;
                        let packet = ResponsePacket::Notice {
                            noticemsg: "VICTORY — CONQUEST! You have uncovered the island's secrets and sealed the Monolith. The darkness recedes! You may continue playing.".to_string(),
                            expiry: Some(30000),
                        };
                        send_to_client(player_id.0, packet, &clients);
                    }
                }
            }
        }
    }
}

fn soulshard_count(inventory: &Inventory) -> i32 {
    inventory
        .get_by_class(SOULSHARD.to_string())
        .map(|soulshards| soulshards.quantity)
        .unwrap_or(0)
}

// The first death must always be affordable from the monolith's starting stash
// (10 shards) regardless of XP earned, so a run never hard-ends on the first
// mistake. The XP/death-count formula takes over from the second death.
pub const FIRST_DEATH_SOULSHARD_COST: i32 = 5;

fn resurrection_attempt_cost(num_deaths: u32, total_xp: i32) -> i32 {
    if num_deaths <= 1 {
        return FIRST_DEATH_SOULSHARD_COST;
    }
    soulshard_res_cost(num_deaths.saturating_sub(2), total_xp)
}

fn send_hero_death_state(
    clients: &Res<Clients>,
    player_id: i32,
    phase: &str,
    hero_id: i32,
    hero_name: &str,
    resurrect_cost: i32,
    soulshards_available: i32,
    seconds_remaining: i32,
    message: String,
) {
    let packet = ResponsePacket::HeroDeathState {
        phase: phase.to_string(),
        hero_id,
        hero_name: hero_name.to_string(),
        resurrect_cost,
        soulshards_available,
        seconds_remaining,
        message,
    };

    send_to_client(player_id, packet, clients);
}

fn resurrect_system(
    mut commands: Commands,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    mut map_events: ResMut<MapEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    game_tick: Res<GameTick>,
    player_stats: ResMut<PlayerStats>,
    active_infos: Res<ActiveInfos>,
    (
        mut hero_query,
        dead_state_query,
        revival_monolith_query,
        mut monolith_inventory_query,
        mut effect_query,
        obj_query,
        mut needs_query,
    ): (
        Query<
            HeroResurrectQuery,
            (
                With<StateDead>,
                With<SubclassHero>,
                Without<TrueDeath>,
                Without<Monolith>,
            ),
        >,
        Query<&StateDead>,
        Query<&BoundMonolith>,
        Query<&mut Inventory, With<Monolith>>,
        Query<&mut Effects>,
        Query<ObjQuery, Without<SubclassHero>>,
        Query<(&mut Thirst, &mut Hunger, &mut Tired)>,
    ),
) {
    for mut hero in hero_query.iter_mut() {
        let Ok(dead_state) = dead_state_query.get(hero.entity) else {
            error!("No dead state found for entity: {:?}", hero.entity);
            continue;
        };

        if (game_tick.0 - dead_state.dead_at) > 15 * TICKS_PER_SEC {
            let num_deaths = player_stats
                .get(&hero.player_id.0)
                .expect("Player stats not found")
                .num_deaths;
            let total_xp = hero.skills.get_total_xp();
            let resurrect_cost = resurrection_attempt_cost(num_deaths, total_xp);

            let Ok(revival_monolith) = revival_monolith_query.get(hero.entity) else {
                error!(
                    "Revival monolith no longer exists, cannot resurrect entity {:?}",
                    hero.entity
                );
                send_hero_death_state(
                    &clients,
                    hero.player_id.0,
                    "true_death_pending",
                    hero.id.0,
                    &hero.name.0,
                    resurrect_cost,
                    0,
                    10,
                    "The Monolith bond is broken. True Death is near.".to_string(),
                );
                commands.entity(hero.entity).insert(TrueDeath {
                    true_death_at: game_tick.0,
                });
                continue;
            };

            let Ok(mut effects) = effect_query.get_mut(hero.entity) else {
                error!("No effects found for entity {:?}", hero.entity);
                continue;
            };

            let Some(monolith_entity) = entity_map.get_entity(revival_monolith.id) else {
                error!("No entity found for monolith id {:?}", revival_monolith.id);
                send_hero_death_state(
                    &clients,
                    hero.player_id.0,
                    "true_death_pending",
                    hero.id.0,
                    &hero.name.0,
                    resurrect_cost,
                    0,
                    10,
                    "The bound Monolith is gone. True Death is near.".to_string(),
                );
                commands.entity(hero.entity).insert(TrueDeath {
                    true_death_at: game_tick.0,
                });
                continue;
            };

            let Ok(mut monolith_inventory) = monolith_inventory_query.get_mut(monolith_entity)
            else {
                error!("No inventory found for entity {:?}", monolith_entity);
                send_hero_death_state(
                    &clients,
                    hero.player_id.0,
                    "true_death_pending",
                    hero.id.0,
                    &hero.name.0,
                    resurrect_cost,
                    0,
                    10,
                    "The Monolith cannot find its Soulshards. True Death is near.".to_string(),
                );
                commands.entity(hero.entity).insert(TrueDeath {
                    true_death_at: game_tick.0,
                });
                continue;
            };

            let soulshards_available = soulshard_count(&monolith_inventory);

            debug!("Resurrect cost: {:?}", resurrect_cost);

            let Some(soulshards) = monolith_inventory.get_by_class(SOULSHARD.to_string()) else {
                debug!("Hero {:?} has no soulshards, cannot resurrect", hero.id);
                send_hero_death_state(
                    &clients,
                    hero.player_id.0,
                    "true_death_pending",
                    hero.id.0,
                    &hero.name.0,
                    resurrect_cost,
                    soulshards_available,
                    10,
                    format!(
                        "The Monolith finds no Soulshards to bind. {} is lost to True Death.",
                        hero.name.0
                    ),
                );
                let packet = ResponsePacket::Notice {
                    noticemsg: format!(
                        "The Monolith finds no soulshards to bind. {} is lost to True Death.",
                        hero.name.0
                    ),
                    expiry: Some(10000),
                };

                send_to_client(hero.player_id.0, packet, &clients);

                // Insert true death state
                commands.entity(hero.entity).insert(TrueDeath {
                    true_death_at: game_tick.0,
                });

                continue;
            };

            if soulshards.quantity < resurrect_cost {
                debug!(
                    "Hero {:?} has insufficient soulshards, cannot resurrect",
                    hero.id
                );
                send_hero_death_state(
                    &clients,
                    hero.player_id.0,
                    "true_death_pending",
                    hero.id.0,
                    &hero.name.0,
                    resurrect_cost,
                    soulshards_available,
                    10,
                    format!(
                        "The Monolith lacks Soulshards ({}/{}). True Death claims {}.",
                        soulshards.quantity, resurrect_cost, hero.name.0
                    ),
                );
                let packet = ResponsePacket::Notice {
                    noticemsg: format!(
                        "The Monolith lacks shards ({}/{}). True Death claims {}.",
                        soulshards.quantity, resurrect_cost, hero.name.0
                    ),
                    expiry: Some(10000),
                };
                send_to_client(hero.player_id.0, packet, &clients);

                // Insert true death state
                commands.entity(hero.entity).insert(TrueDeath {
                    true_death_at: game_tick.0,
                });
                continue;
            }

            // Remove soulshards from monolith
            let updated_soulshards =
                monolith_inventory.remove_quantity(soulshards.id, resurrect_cost);
            let remaining_soulshards = updated_soulshards
                .as_ref()
                .map(|soulshards| soulshards.quantity)
                .unwrap_or(0);

            let active_info_key = (
                MONOLITH_PLAYER_ID,
                revival_monolith.id,
                "inventory".to_string(),
            );

            /*if let Some(_active_info) = active_infos.get(&active_info_key) {
                let mut items_to_remove = Vec::new();
                let mut items_to_update = Vec::new();

                if let Some(updated_soulshards) = updated_soulshards {
                    items_to_update.push(Item::to_packet(updated_soulshards));
                } else {
                    items_to_remove.push(soulshards.id);
                }

                let item_update_packet: ResponsePacket = ResponsePacket::InfoItemsUpdate {
                    id: revival_monolith.id,
                    items_updated: items_to_update,
                    items_removed: items_to_remove,
                };

                send_to_client(hero.player_id.0, item_update_packet, &clients);
            }*/

            debug!("Resurrecting hero {:?}", hero.id);
            send_hero_death_state(
                &clients,
                hero.player_id.0,
                "resurrected",
                hero.id.0,
                &hero.name.0,
                resurrect_cost,
                remaining_soulshards,
                0,
                format!(
                    "The Monolith spends {} Soulshards and binds {} again.",
                    resurrect_cost, hero.name.0
                ),
            );

            // Create human corpse
            let (corpse_id, _entity) = Obj::create(
                hero.player_id.0,
                "Human Corpse".to_string(),
                *hero.pos,
                State::Dead,
                &mut commands,
                &mut ids,
                &mut entity_map,
                &mut map_events,
                &game_tick,
                &templates,
            );

            // Transfer all items to corpse TODO fix
            //items.transfer_all_items(hero.id.0, corpse_id);

            //Reset hp & state
            hero.stats.hp = hero.stats.base_hp;
            *hero.state = State::None;

            // The Monolith restores the body whole: clear needs and any ticking
            // needs-death timers, otherwise the hero resurrects mid-countdown
            // and dies again seconds later.
            if let Ok((mut thirst, mut hunger, mut tired)) = needs_query.get_mut(hero.entity) {
                thirst.thirst = 0.0;
                hunger.hunger = 0.0;
                tired.tired = 0.0;
            }
            commands
                .entity(hero.entity)
                .remove::<Dehydrated>()
                .remove::<Starving>()
                .remove::<Exhausted>();

            //TODO replace with monolith location
            let src = hero.pos.clone();
            let dst = revival_monolith.pos;

            *hero.pos = dst.clone();

            // Add sanctuary effect if not already added
            if !effects.has(Effect::Sanctuary) {
                effects
                    .0
                    .insert(Effect::Sanctuary, (game_tick.0 + 1, 1.0, 1));

                commands.entity(hero.entity).insert(Sanctuary {
                    id: revival_monolith.id,
                    pos: revival_monolith.pos,
                });

                let response_packet = ResponsePacket::GainedEffect {
                    id: hero.id.0,
                    x: revival_monolith.pos.x,
                    y: revival_monolith.pos.y,
                    effect: Effect::Sanctuary.to_str(),
                };

                send_to_client(hero.player_id.0, response_packet, &clients);
            }

            commands.entity(hero.entity).remove::<StateDead>();

            let packet = ResponsePacket::Stats {
                data: StatsData {
                    id: hero.id.0,
                    hp: hero.stats.hp,
                    base_hp: hero.stats.base_hp,
                    stamina: hero.stats.stamina.unwrap_or(100),
                    base_stamina: hero.stats.base_stamina.unwrap_or(100),
                    mana: hero.stats.mana.unwrap_or(0),
                    base_mana: hero.stats.base_mana.unwrap_or(0),
                    thirst: None,
                    hunger: None,
                    tiredness: None,
                    effects: Vec::new(),
                },
            };

            send_to_client(hero.player_id.0, packet, &clients);

            // None visible state change
            commands.trigger(StateChange {
                entity: hero.entity,
                new_state: State::None,
            });

            // Move event
            let move_event = VisibleEvent::MoveEvent { src: src, dst: dst };

            // Move change
            let move_map_event = MapEvent {
                event_id: Uuid::new_v4(),
                obj_id: hero.id.0,
                run_tick: game_tick.0 + 2,
                event_type: move_event.clone(),
            };

            visible_events.push(move_map_event);

            // Get new objs in viewshed
            let mut new_objs = Vec::new();

            for obj in obj_query.iter() {
                let distance = Map::distance((hero.pos.x, hero.pos.y), (obj.pos.x, obj.pos.y));

                if hero.viewshed.range >= distance && obj.state.is_visible() {
                    new_objs.push(network::to_map_obj(obj));
                }
            }

            let map_packet = ResponsePacket::NewObjPerception {
                new_objs: new_objs,
                new_tiles: Vec::new(),
            };

            send_to_client(hero.player_id.0, map_packet, &clients);
        }
    }
}

fn state_change_observer(
    state_change: On<StateChange>,
    game_tick: Res<GameTick>,
    presence: OptionalPlayerWorldPresence,
    mut visible_events: ResMut<VisibleEvents>,
    mut query: Query<(&Id, Option<&PlayerId>, &mut State)>,
) {
    let Ok((id, player_id, mut state)) = query.get_mut(state_change.entity) else {
        error!("Query failed to find entity {:?}", state_change.entity);
        return;
    };

    if player_id
        .map(|player_id| is_owner_offline_protected(player_id, &presence))
        .unwrap_or(false)
    {
        return;
    }

    *state = state_change.new_state;

    // Create a new map event for the visible event
    let map_event = MapEvent {
        event_id: Uuid::new_v4(),
        obj_id: id.0,
        run_tick: game_tick.0,
        event_type: VisibleEvent::StateChangeEvent {
            new_state: state_change.new_state.to_string(),
        },
    };

    visible_events.push(map_event.clone());
}

fn template_change_observer(
    template_change: On<TemplateChange>,
    game_tick: Res<GameTick>,
    mut visible_events: ResMut<VisibleEvents>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut query: Query<(&Id, &mut Template, &mut Misc)>,
) {
    let Ok((id, mut template, mut misc)) = query.get_mut(template_change.entity) else {
        error!("Query failed to find entity {:?}", template_change.entity);
        return;
    };

    *template = Template(template_change.new_template.clone());

    let new_template = templates.obj_templates.get(template.0.clone());

    if let Some(images) = new_template.images {
        let random_image = rand::thread_rng().gen_range(0..images.len());
        misc.image = images[random_image].clone();
    } else {
        misc.image = new_template.image.clone();
    }

    let mut attrs = Vec::new();

    attrs.push((TEMPLATE.to_string(), template.0.clone()));
    attrs.push((IMAGE.to_string(), misc.image.clone()));

    // Create a new map event for the visible event
    let map_event = MapEvent {
        event_id: Uuid::new_v4(),
        obj_id: id.0,
        run_tick: game_tick.0,
        event_type: VisibleEvent::UpdateObjEvent { attrs: attrs },
    };

    visible_events.push(map_event.clone());
}

fn new_obj_observer(
    new_obj: On<NewObj>,
    game_tick: Res<GameTick>,
    mut commands: Commands,
    clients: Res<Clients>,
    mut visible_events: ResMut<VisibleEvents>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    new_obj_query: Query<(&PlayerId, &Id, &Position, &Class, &Subclass, &State)>,
    query: Query<(Entity, &PlayerId, &Id, &Position, &Subclass, &State)>,
    mut effect_query: Query<&mut Effects>,
) {
    let Ok((
        new_obj_player_id,
        new_obj_id,
        new_obj_position,
        new_obj_class,
        new_obj_subclass,
        new_obj_state,
    )) = new_obj_query.get(new_obj.entity)
    else {
        error!("Query failed to find entity {:?}", new_obj.entity);
        return;
    };

    if *new_obj_subclass == Subclass::Wall {
        if wall_grants_fortification(new_obj_state) {
            for (entity, player_id, id, position, _subclass, state) in query.iter() {
                if *player_id == *new_obj_player_id
                    && id.0 != new_obj_id.0
                    && *new_obj_position == *position
                    && state.is_active()
                {
                    if let Ok(mut effects) = effect_query.get_mut(entity) {
                        effects
                            .0
                            .insert(Effect::Fortified, (game_tick.0 + 1, 0.0, 1));

                        commands
                            .entity(entity)
                            .insert(Fortified { id: new_obj_id.0 });
                    }
                }
            }
        }
    } else {
        let mut wall_on_tile = None;

        for (_entity, player_id, id, position, subclass, state) in query.iter() {
            if new_obj_id.0 != id.0
                && *new_obj_position == *position
                && new_obj_player_id == player_id
                && *subclass == Subclass::Wall
                && wall_grants_fortification(state)
            {
                wall_on_tile = Some(id.0);
            }
        }

        if let Some(wall_id) = wall_on_tile {
            if let Ok(mut effects) = effect_query.get_mut(new_obj.entity) {
                trace!("Adding Fortified on {:?}", new_obj_id.0);
                effects
                    .0
                    .insert(Effect::Fortified, (game_tick.0 + 1, 0.0, 1));

                commands
                    .entity(new_obj.entity)
                    .insert(Fortified { id: wall_id });
            }
        }
    }

    if player::is_player(new_obj_player_id.0) {
        let Ok(mut effects) = effect_query.get_mut(new_obj.entity) else {
            error!("No effects found for player obj {:?}", new_obj.entity);
            return;
        };

        let mut monolith_in_range = None;

        // Check if any monoliths are in range of new obj
        for (entity, player_id, id, position, subclass, state) in query.iter() {
            if new_obj_id.0 == id.0 {
                // Skip self
                continue;
            }

            if *subclass == Subclass::Monolith {
                if Map::dist(*new_obj_position, *position) < 5 {
                    monolith_in_range = Some((id.0, position.clone()));
                }
            }
        }

        if let Some((monolith_id, monolith_pos)) = monolith_in_range {
            if !effects.has(Effect::Sanctuary) {
                effects
                    .0
                    .insert(Effect::Sanctuary, (game_tick.0 + 1, 1.0, 1));

                commands.entity(new_obj.entity).insert(Sanctuary {
                    id: monolith_id,
                    pos: monolith_pos,
                });

                // Do not send the notification for structures and villagers, not needed
                if !new_obj_class.is_structure() && !new_obj_subclass.is_villager() {
                    let response_packet = ResponsePacket::GainedEffect {
                        id: new_obj_id.0,
                        x: new_obj_position.x,
                        y: new_obj_position.y,
                        effect: Effect::Sanctuary.to_str(),
                    };

                    send_to_client(new_obj_player_id.0, response_packet, &clients);
                }
            }
        }
    }

    // Create a new map event for the visible event
    let map_event = MapEvent {
        event_id: Uuid::new_v4(),
        obj_id: new_obj_id.0,
        run_tick: game_tick.0,
        event_type: VisibleEvent::NewObjEvent,
    };

    visible_events.push(map_event.clone());
}

fn remove_obj_observer(
    remove_obj: On<RemoveObj>,
    mut commands: Commands,
    mut entity_map: ResMut<EntityObjMap>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    initial_encounter_state: Res<InitialEncounterState>,
    mut visible_events: ResMut<VisibleEvents>,
    query: Query<(&Id, &Position)>,
) {
    let Ok((id, pos)) = query.get(remove_obj.entity) else {
        error!("Query failed to find entity {:?}", remove_obj.entity);
        return;
    };

    if object_belongs_to_protected_run(id.0, &ids, &presence)
        || initial_encounter_object_is_protected(id.0, &initial_encounter_state, &presence)
    {
        return;
    }

    // Guard against double-removal: if the obj is already gone from the entity map,
    // a previous RemoveObj observer already processed this entity this frame.
    // entity_map mutations are immediate (not deferred), so this is a reliable guard.
    if entity_map.get_entity(id.0).is_none() {
        return;
    }

    commands.entity(remove_obj.entity).despawn();
    entity_map.remove_obj(id.0);

    let map_event = MapEvent {
        event_id: Uuid::new_v4(),
        obj_id: id.0,
        run_tick: game_tick.0,
        event_type: VisibleEvent::RemoveObjEvent { pos: pos.clone() },
    };

    visible_events.push(map_event.clone());
}

fn update_obj_observer(
    update_obj: On<UpdateObj>,
    commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    mut visible_events: ResMut<VisibleEvents>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut perception_updates: ResMut<PerceptionUpdates>,
    mut query: Query<UpdateObjQuery>,
    mut viewshed_query: Query<&mut Viewshed>,
) {
    let Ok(mut obj) = query.get_mut(update_obj.entity) else {
        error!("Query failed to find entity {:?}", update_obj.entity);
        return;
    };

    for (attr, value) in update_obj.attrs.iter() {
        match attr.as_str() {
            PLAYER_ID => {
                let new_player_id = value.parse::<i32>().unwrap();

                *obj.player_id = PlayerId(new_player_id);
                ids.change_obj_player_id(obj.id.0, new_player_id.clone());

                // Create a new map event for the visible event
                let map_event = MapEvent {
                    event_id: Uuid::new_v4(),
                    obj_id: obj.id.0,
                    run_tick: game_tick.0,
                    event_type: VisibleEvent::UpdateObjEvent {
                        attrs: update_obj.attrs.clone(),
                    },
                };

                visible_events.push(map_event.clone());
            }
            TEMPLATE => {
                obj.template.0 = value.to_string();

                let template = templates.obj_templates.get(value.to_string());

                if let Some(images) = template.images {
                    let random_image = rand::thread_rng().gen_range(0..images.len());
                    obj.misc.image = images[random_image].clone();
                } else {
                    obj.misc.image = Obj::template_to_image(&template.template);
                }

                // Create a new map event for the visible event
                let map_event = MapEvent {
                    event_id: Uuid::new_v4(),
                    obj_id: obj.id.0,
                    run_tick: game_tick.0,
                    event_type: VisibleEvent::UpdateObjEvent {
                        attrs: update_obj.attrs.clone(),
                    },
                };

                visible_events.push(map_event.clone());
            }
            IMAGE => {
                obj.misc.image = value.to_string();
                // Create a new map event for the visible event
                let map_event = MapEvent {
                    event_id: Uuid::new_v4(),
                    obj_id: obj.id.0,
                    run_tick: game_tick.0,
                    event_type: VisibleEvent::UpdateObjEvent {
                        attrs: update_obj.attrs.clone(),
                    },
                };

                visible_events.push(map_event.clone());
            }
            VISION => {
                let vision_modifier = obj.effects.get_vision_modifier(&templates);

                let Ok(mut viewshed) = viewshed_query.get_mut(update_obj.entity) else {
                    error!("Query failed to find entity {:?}", update_obj.entity);
                    return;
                };

                info!(
                    "Id: {:?} Template: {:?} viewshed: {:?}",
                    obj.id.0, obj.template.0, viewshed.range
                );
                let new_range = Obj::set_viewshed_range(
                    obj.id.0,
                    obj.template.0.clone(),
                    game_tick.0,
                    &obj.inventory,
                    &templates,
                    vision_modifier,
                );

                info!(
                    "Updating viewshed range to: {:?} for id: {:?} template: {:?}",
                    new_range, obj.id.0, obj.template.0
                );

                viewshed.range = new_range;

                perception_updates
                    .insert((obj.player_id.0, PerceptionUpdateType::UpdatePerception));
            }
            _ => {
                error!("Unknown attribute {:?}", attr);
            }
        }
    }
}

fn start_build_observer(
    start_build: On<StartBuild>,
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut query: Query<(
        &PlayerId,
        &Position,
        &State,
        &Template,
        &mut Inventory,
        &Assignments,
    )>,
    worker_query: Query<(&Position, &State)>,
) {
    info!("Starting build observer");
    let Ok((
        structure_player_id,
        structure_pos,
        structure_state,
        structure_template_name,
        mut structure_inventory,
        structure_assignments,
    )) = query.get_mut(start_build.entity)
    else {
        info!("Query failed to find entity {:?}", start_build.entity);
        return;
    };

    if is_owner_offline_protected(structure_player_id, &presence) {
        return;
    }

    let structure_template = templates
        .obj_templates
        .get(structure_template_name.0.clone());
    let structure_req = structure_template
        .req
        .expect("Template should have req field");

    match *structure_state {
        State::None => {
            info!("Structure is already completed, skipping build observer");
        }
        State::Founded => {
            let has_reqs = structure_inventory.has_reqs_for_build(structure_req.clone());

            if !has_reqs {
                info!("Not enough items to build structure, skipping build observer");
                return;
            }

            info!("Consuming required items for structure...");
            structure_inventory.consume_reqs_for_build(structure_req.clone());

            // Add StateBuilding component
            commands.entity(start_build.entity).insert(StateBuilding);

            // Change structure state to building
            commands.trigger(StateChange {
                entity: start_build.entity,
                new_state: State::Building,
            });

            // Change all assigned and available builders to building state and get total_build_rate
            for worker_id in structure_assignments.0.iter() {
                let Some(worker_entity) = entity_map.get_entity(*worker_id) else {
                    error!("Cannot find worker entity for {:?}", worker_id);
                    continue;
                };

                let Ok((worker_pos, worker_state)) = worker_query.get(worker_entity) else {
                    error!("Query failed to find entity {:?}", worker_entity);
                    continue;
                };

                // Check if worker is on the same position as the structure
                if *worker_pos != *structure_pos {
                    continue;
                }

                if *worker_state == State::None {
                    commands.trigger(StateChange {
                        entity: worker_entity,
                        new_state: State::Building,
                    });
                }
            }

            // Trigger a build progress update to client
            commands.trigger(BuildProgressUpdate {
                entity: start_build.entity,
            });
        }
        State::Building => {
            // Set builder state to building
            commands.trigger(StateChange {
                entity: start_build.builder_entity,
                new_state: State::Building,
            });

            // Trigger a build progress update to client
            commands.trigger(BuildProgressUpdate {
                entity: start_build.entity,
            });
        }
        _ => {
            info!("Structure is in an unknown state, skipping build observer");
        }
    }
}

fn transfer_all_resources_observer(
    event: On<TransferAllResources>,
    commands: Commands,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    templates: Res<Templates>,
    mut inventory_query: Query<(&PlayerId, &Template, &mut Inventory)>,
) {
    let Ok(
        [(source_player_id, _source_template, mut source_inventory), (target_player_id, target_template, mut target_inventory)],
    ) = inventory_query.get_many_mut([event.entity, event.target_entity])
    else {
        error!(
            "Query failed to find inventories for entities {:?}",
            [event.entity, event.target_entity]
        );
        return;
    };

    if is_owner_offline_protected(source_player_id, &presence)
        || is_owner_offline_protected(target_player_id, &presence)
    {
        return;
    }

    let target_capacity = templates
        .obj_templates
        .get_capacity(target_template.0.clone());

    info!(
        "Transferring all resources from {:?} to {:?}",
        source_inventory.owner, target_inventory.owner
    );
    Inventory::transfer_partial_resources(
        &mut source_inventory,
        &mut target_inventory,
        &mut ids,
        target_capacity,
        &templates.item_templates,
    );
}

fn food_poisoning_effect_observer(
    event: On<FoodPoisoningEffect>,
    mut commands: Commands,
    clients: Res<Clients>,
    presence: Res<PlayerWorldPresenceState>,
    mut query: Query<(&PlayerId, &Id, &Position, &mut Effects)>,
) {
    let food_poisoning_value = match event.food_poisoning_attr {
        item::AttrVal::Num(val) => val,
        _ => panic!("Invalid food poisoning attribute value"),
    };

    if food_poisoning_value > rand::thread_rng().gen_range(0.0..1.0) {
        let Ok((player_id, id, pos, mut effects)) = query.get_mut(event.entity) else {
            error!("Query failed to find effects entity {:?}", event.entity);
            return;
        };

        if is_owner_offline_protected(player_id, &presence) {
            return;
        }

        effects.0.insert(Effect::FoodPoisoning, (10, 1.0, 1));

        commands
            .entity(event.entity)
            .remove::<EffectAddedProcessed>()
            .insert(EffectAdded {
                effect: Effect::FoodPoisoning,
            });

        let response_packet = ResponsePacket::GainedEffect {
            id: id.0,
            x: pos.x,
            y: pos.y,
            effect: Effect::FoodPoisoning.to_str(),
        };

        send_to_client(player_id.0, response_packet, &clients);
    }
}

fn build_progress_update_observer(
    event: On<BuildProgressUpdate>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    templates: Res<Templates>,
    mut perception_updates: ResMut<PerceptionUpdates>,
    mut structure_query: Query<(
        &PlayerId,
        &Id,
        &Position,
        &Assignments,
        &mut BuildUpgradeState,
    )>,
    worker_query: Query<(&Position, &State, &Template, &Skills)>,
) {
    let Ok((
        structure_player_id,
        structure_id,
        structure_position,
        structure_assignments,
        mut build_state,
    )) = structure_query.get_mut(event.entity)
    else {
        error!("Query failed to find entity {:?}", event.entity);
        return;
    };

    let mut total_build_rate = 0.0;

    // Calculate current build rate from all assigned workers
    info!("Structure assignments: {:?}", structure_assignments.0);
    for worker_id in structure_assignments.0.iter() {
        let Some(worker_entity) = entity_map.get_entity(*worker_id) else {
            error!("Cannot find worker entity for {:?}", worker_id);
            continue;
        };

        let Ok((worker_pos, worker_state, worker_template, worker_skills)) =
            worker_query.get(worker_entity)
        else {
            error!("Query failed to find worker entity {:?}", worker_entity);
            continue;
        };

        // Check if worker is on the same position as the structure
        if worker_pos != structure_position {
            continue;
        }

        // Only count workers in building or upgrading state
        if worker_state != &State::Building && worker_state != &State::Upgrading {
            continue;
        }

        // Get template from worker
        let worker_template = templates.obj_templates.get(worker_template.0.clone());

        // Get base work from worker template
        let base_work = worker_template.base_work.unwrap_or(5);

        // Get skills from worker
        let carpentry_skill = worker_skills.get_level_by_name(Skill::Carpentry);
        let masonry_skill = worker_skills.get_level_by_name(Skill::Masonry);
        let construction_skill = worker_skills.get_level_by_name(Skill::Construction);

        // Get build rate from worker
        let build_rate = Obj::construction_skill_multiplier(
            base_work,
            construction_skill,
            carpentry_skill,
            masonry_skill,
        );

        total_build_rate += build_rate;
    }

    // Anchor the build start time so the client can animate the progress bar
    // from when construction actually began.
    if build_state.start_time == 0 {
        build_state.start_time = game_tick.0;
    }

    // Store the live build rate on the structure so it is included in the
    // perception data sent to the client (the WorkUpdate packet alone is not
    // enough: the client prefers the structure's perception work fields).
    build_state.work_per_sec = total_build_rate;

    // Send progress packet to client
    let packet = ResponsePacket::WorkUpdate {
        structure_id: structure_id.0,
        work_done: build_state.work_done,
        total_work: build_state.build_upgrade_cost,
        work_per_sec: total_build_rate,
    };

    send_to_client(structure_player_id.0, packet, &clients);

    // Re-send the structure's perception so the client receives the new
    // work_per_sec / work_done and can animate the progress bar.
    perception_updates.insert((
        structure_player_id.0,
        PerceptionUpdateType::UpdatePerception,
    ));
}

fn start_upgrade_observer(
    start_upgrade: On<StartUpgrade>,
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    presence: Res<PlayerWorldPresenceState>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut query: Query<(
        &PlayerId,
        &Position,
        &State,
        &Template,
        &mut Inventory,
        &Assignments,
        &SelectedUpgrade,
    )>,
    worker_query: Query<(&Position, &State)>,
) {
    info!("Starting upgrade observer");
    let Ok((
        structure_player_id,
        structure_pos,
        structure_state,
        structure_template_name,
        mut structure_inventory,
        structure_assignments,
        selected_upgrade,
    )) = query.get_mut(start_upgrade.entity)
    else {
        info!("Query failed to find entity {:?}", start_upgrade.entity);
        return;
    };

    if is_owner_offline_protected(structure_player_id, &presence) {
        return;
    }

    let selected_upgrade_structure_template =
        templates.obj_templates.get(selected_upgrade.0.clone());

    let structure_upgrade_req = selected_upgrade_structure_template
        .upgrade_req
        .expect("Template should have upgrade_req field");

    match *structure_state {
        State::None => {
            info!("Structure is already upgraded, skipping upgrade observer");
        }
        State::PlanningUpgrade => {
            let has_reqs = structure_inventory.has_reqs_for_build(structure_upgrade_req.clone());

            if !has_reqs {
                info!("Not enough items to upgrade structure, skipping upgrade observer");
                return;
            }

            info!("Consuming required items for structure...");
            structure_inventory.consume_reqs_for_build(structure_upgrade_req.clone());

            // Add StateUpgrading component
            commands.entity(start_upgrade.entity).insert(StateUpgrading);

            // Change structure state to upgrading
            commands.trigger(StateChange {
                entity: start_upgrade.entity,
                new_state: State::Upgrading,
            });

            // Change all assigned and available builders to building state and get total_build_rate
            for worker_id in structure_assignments.0.iter() {
                let Some(worker_entity) = entity_map.get_entity(*worker_id) else {
                    error!("Cannot find worker entity for {:?}", worker_id);
                    continue;
                };

                let Ok((worker_pos, worker_state)) = worker_query.get(worker_entity) else {
                    error!("Query failed to find entity {:?}", worker_entity);
                    continue;
                };

                // Check if worker is on the same position as the structure
                if *worker_pos != *structure_pos {
                    continue;
                }

                if *worker_state == State::None {
                    commands.trigger(StateChange {
                        entity: worker_entity,
                        new_state: State::Upgrading,
                    });
                }
            }

            // Trigger a build progress update to client
            commands.trigger(BuildProgressUpdate {
                entity: start_upgrade.entity,
            });
        }
        State::Upgrading => {
            // Set builder state to building
            commands.trigger(StateChange {
                entity: start_upgrade.builder_entity,
                new_state: State::Upgrading,
            });

            // Trigger a build progress update to client
            commands.trigger(BuildProgressUpdate {
                entity: start_upgrade.entity,
            });
        }
        _ => {
            info!("Structure is in an unknown state, skipping build observer");
        }
    }
}

fn start_work_observer(
    start_work: On<StartWork>,
    mut commands: Commands,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut game_events: ResMut<GameEvents>,
    recipes: Res<Recipes>,
    templates: Res<Templates>,
    mut query: Query<(&PlayerId, &Id, &mut WorkQueue)>,
    inventory_query: Query<&Inventory>,
    mut active_task_query: Query<&mut ActiveTask>,
) {
    info!("Processing StartWork");

    if object_belongs_to_protected_run(start_work.worker_id, &ids, &presence)
        || object_belongs_to_protected_run(start_work.structure_id, &ids, &presence)
    {
        return;
    }

    // Get player id from worker
    let Some(player_worker_id) = ids.get_player(start_work.worker_id) else {
        error!("Cannot find player from worker {:?}", start_work.worker_id);
        return;
    };

    let Some(player_structure_id) = ids.get_player(start_work.structure_id) else {
        error!(
            "Cannot find player from structure {:?}",
            start_work.structure_id
        );
        return;
    };

    let Some(structure_entity) = entity_map.get_entity(start_work.structure_id) else {
        error!("Cannot find structure from {:?}", start_work.structure_id);
        return;
    };

    let Ok(mut active_task) = active_task_query.get_mut(start_work.entity) else {
        error!("No active task component for {:?}", start_work.entity);
        return;
    };

    // Get assigned work entry from work queue that matches worker_id
    for (player_id, id, mut work_queue) in query.iter_mut() {
        // Skip if player id does not match worker id
        if player_id.0 != player_worker_id {
            continue;
        }

        for work_queue_entry in work_queue.0.iter_mut() {
            // Skip if worker id does not match work queue entry worker id
            if start_work.worker_id != work_queue_entry.worker_id {
                continue;
            }

            // Process the work entry here where the reference is valid
            match work_queue_entry.work_type {
                WorkType::Craft => {
                    // Add State Change Event to None
                    commands.trigger(StateChange {
                        entity: start_work.entity,
                        new_state: State::Crafting,
                    });

                    let recipe_name = work_queue_entry
                        .recipe_name
                        .clone()
                        .unwrap_or("".to_string());

                    let Some(recipe) = recipes.get_by_name(recipe_name.clone()) else {
                        error!("Cannot find recipe for {:?}", recipe_name);
                        continue;
                    };

                    let work_time = recipe.crafting_time.unwrap_or(200);

                    let event = GameEvent {
                        event_id: ids.new_map_event_id(),
                        start_tick: game_tick.0,
                        run_tick: game_tick.0 + work_time, // in the future
                        event_type: GameEventType::StructureCraftEvent {
                            crafter_id: start_work.worker_id,
                            structure_id: start_work.structure_id,
                            recipe_name: recipe_name.clone(),
                        },
                    };

                    game_events.insert(event.event_id, event);

                    // Set active task to crafting
                    ActiveTask::set_if_changed(&mut active_task, ActiveTask::Crafting);
                }
                WorkType::Refine => {
                    commands.trigger(StateChange {
                        entity: start_work.entity,
                        new_state: State::Refining,
                    });

                    let refine_item_id = work_queue_entry.refine_item_id.clone().unwrap_or(-1);

                    let Ok(inventory) = inventory_query.get(structure_entity) else {
                        error!("Cannot find inventory for entity: {:?}", structure_entity);
                        continue;
                    };

                    let Some(item) = inventory.get_by_id(refine_item_id) else {
                        error!("Cannot find item for {:?}", refine_item_id);
                        continue;
                    };

                    let item_template =
                        Item::get_template(item.name.clone(), &templates.item_templates);
                    let work_time = item_template.get_refine_time();

                    let event = GameEvent {
                        event_id: ids.new_map_event_id(),
                        start_tick: game_tick.0,
                        run_tick: game_tick.0 + work_time, // in the future
                        event_type: GameEventType::StructureRefineEvent {
                            refiner_id: start_work.worker_id,
                            structure_id: start_work.structure_id,
                            item_id: work_queue_entry.refine_item_id.clone().unwrap_or(-1),
                        },
                    };

                    game_events.insert(event.event_id, event);

                    // Set active task to refining
                    ActiveTask::set_if_changed(&mut active_task, ActiveTask::Refining);
                }
                WorkType::Experiment => {
                    // TODO: Implement experiment work type
                }
                WorkType::Operate => {
                    commands.trigger(StateChange {
                        entity: start_work.entity,
                        new_state: State::Operating,
                    });

                    // TODO: Get work time from structure
                    let work_time = 200;

                    let event = GameEvent {
                        event_id: ids.new_map_event_id(),
                        start_tick: game_tick.0,
                        run_tick: game_tick.0 + work_time, // in the future
                        event_type: GameEventType::StructureOperateEvent {
                            operator_id: start_work.worker_id,
                            structure_id: start_work.structure_id,
                        },
                    };

                    game_events.insert(event.event_id, event);

                    // Set active task to operating
                    ActiveTask::set_if_changed(&mut active_task, ActiveTask::Operating);
                }
                _ => {}
            }

            // Set work status to in progress
            work_queue_entry.work_status = WorkStatus::InProgress;

            // Set only one work entry to in progress
            break;
        }
    }
}

fn cancel_events_observer(
    event: On<CancelEvents>,
    mut commands: Commands,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    query: Query<&Id>,
) {
    let Ok(obj_id) = query.get(event.entity) else {
        error!("Query failed to find entity {:?}", event.entity);
        return;
    };

    let mut events_to_cancel = Vec::new();

    for (_map_event_id, map_event) in map_events.iter() {
        if map_event.obj_id == obj_id.0 {
            // TODO: Check if event is cancellable
            events_to_cancel.push(map_event.clone());
        }
    }

    for (_map_event_id, map_event) in map_events.iter() {
        if map_event.obj_id == obj_id.0 {
            match map_event.event_type {
                VisibleEvent::MoveEvent { .. }
                | VisibleEvent::GatherEvent { .. }
                | VisibleEvent::RefineEvent { .. }
                | VisibleEvent::OperateEvent { .. }
                | VisibleEvent::CraftEvent { .. }
                | VisibleEvent::SurveyEvent
                | VisibleEvent::ProspectEvent
                | VisibleEvent::ExploreEvent
                | VisibleEvent::InvestigateEvent { .. }
                | VisibleEvent::UseItemEvent { .. } => {
                    events_to_cancel.push(map_event.clone());
                }
                _ => {}
            }
        }
    }

    // Remove events from map events
    for map_event in events_to_cancel.iter() {
        map_events.remove(&map_event.event_id);
    }

    let mut game_events_to_cancel = Vec::new();

    // Remove game events
    info!("Removing game events for {:?}", obj_id.0);
    info!("Game events: {:?}", game_events);
    for (game_event_id, game_event) in game_events.iter() {
        match game_event.event_type {
            GameEventType::StructureGatherEvent {
                operator_id,
                structure_id,
            } => {
                if operator_id == obj_id.0 {
                    game_events_to_cancel.push(game_event.clone());

                    commands.trigger(RemoveWorker {
                        entity: event.entity,
                        worker_id: operator_id,
                        structure_id: structure_id,
                    });
                }
            }
            GameEventType::StructureRefineEvent {
                refiner_id,
                structure_id,
                item_id: _,
            } => {
                if refiner_id == obj_id.0 {
                    game_events_to_cancel.push(game_event.clone());

                    commands.trigger(RemoveWorker {
                        entity: event.entity,
                        worker_id: refiner_id,
                        structure_id: structure_id,
                    });
                }
            }
            GameEventType::StructureCraftEvent {
                crafter_id,
                structure_id,
                recipe_name: _,
            } => {
                if crafter_id == obj_id.0 {
                    game_events_to_cancel.push(game_event.clone());

                    commands.trigger(RemoveWorker {
                        entity: event.entity,
                        worker_id: crafter_id,
                        structure_id: structure_id,
                    });
                }
            }
            GameEventType::StructureOperateEvent {
                operator_id,
                structure_id,
            } => {
                if operator_id == obj_id.0 {
                    game_events_to_cancel.push(game_event.clone());

                    info!(
                        "Removing worker {:?} from structure {:?}",
                        operator_id, structure_id
                    );
                    commands.trigger(RemoveWorker {
                        entity: event.entity,
                        worker_id: operator_id,
                        structure_id: structure_id,
                    });
                }
            }
            GameEventType::ForageEvent { forager_id } => {
                if forager_id == obj_id.0 {
                    game_events_to_cancel.push(game_event.clone());
                }
            }
            GameEventType::GatherEvent { gatherer_id, .. } => {
                if gatherer_id == obj_id.0 {
                    game_events_to_cancel.push(game_event.clone());
                }
            }
            GameEventType::RefineEvent { refiner_id, .. } => {
                if refiner_id == obj_id.0 {
                    game_events_to_cancel.push(game_event.clone());
                }
            }
            GameEventType::CraftEvent { crafter_id, .. } => {
                if crafter_id == obj_id.0 {
                    game_events_to_cancel.push(game_event.clone());
                }
            }
            _ => {}
        }
    }

    // Remove game events
    for game_event in game_events_to_cancel.iter() {
        game_events.remove(&game_event.event_id);
    }

    commands.trigger(StateChange {
        entity: event.entity,
        new_state: State::None,
    });

    // TODO check if this is needed
    commands.entity(event.entity).remove::<EventInProgress>();
}

fn remove_worker_from_work_queue_observer(
    remove_worker_event: On<RemoveWorker>,
    entity_map: Res<EntityObjMap>,
    mut work_queue_query: Query<&mut WorkQueue>,
) {
    info!(
        "Removing worker {:?} from structure {:?}",
        remove_worker_event.worker_id, remove_worker_event.structure_id
    );
    let Some(structure_entity) = entity_map.get_entity(remove_worker_event.structure_id) else {
        error!(
            "Cannot find structure entity for {:?}",
            remove_worker_event.structure_id
        );
        return;
    };

    let Ok(mut work_queue) = work_queue_query.get_mut(structure_entity) else {
        error!("Query failed to find work queue for {:?}", structure_entity);
        return;
    };

    // Set worker's work entries back to worker_id -1
    for entry in work_queue.0.iter_mut() {
        if entry.worker_id == remove_worker_event.worker_id {
            // Clear worker id and reset work entry to idle
            entry.worker_id = -1;
            entry.work_status = WorkStatus::Idle;
        }
    }
}

fn hero_dead_system(
    clients: Res<Clients>,
    presence: Res<PlayerWorldPresenceState>,
    mut player_stats: ResMut<PlayerStats>,
    entity_map: Res<EntityObjMap>,
    hero_query: Query<
        (&PlayerId, &Id, &Name, &Skills, Option<&BoundMonolith>),
        (With<SubclassHero>, Added<StateDead>),
    >,
    monolith_inventory_query: Query<&Inventory, With<Monolith>>,
) {
    for (player_id, id, name, skills, bound_monolith) in hero_query.iter() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        info!("Hero dead: {:?}", player_id.0);
        let player_stat = player_stats.entry(player_id.0).or_insert(PlayerStat {
            player_id: player_id.0,
            num_deaths: 0,
            damage_records: VecDeque::new(),
        });
        player_stat.num_deaths += 1;

        let resurrect_cost =
            resurrection_attempt_cost(player_stat.num_deaths, skills.get_total_xp());
        let soulshards_available = bound_monolith
            .and_then(|monolith| entity_map.get_entity(monolith.id))
            .and_then(|monolith_entity| monolith_inventory_query.get(monolith_entity).ok())
            .map(soulshard_count)
            .unwrap_or(0);
        let message = if soulshards_available >= resurrect_cost {
            format!(
                "The Monolith weighs your soul. Resurrection will cost {} Soulshards.",
                resurrect_cost
            )
        } else {
            format!(
                "The Monolith weighs your soul, but it holds {}/{} Soulshards.",
                soulshards_available, resurrect_cost
            )
        };

        send_hero_death_state(
            &clients,
            player_id.0,
            "weighing",
            id.0,
            &name.0,
            resurrect_cost,
            soulshards_available,
            15,
            message,
        );
    }
}

fn true_death_system(
    mut commands: Commands,
    database_managers: Res<DatabaseManagers>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    mut explored_map: ResMut<ExploredMap>,
    (mut map_events, mut game_events): (ResMut<MapEvents>, ResMut<GameEvents>),
    player_stats: Res<PlayerStats>,
    (
        mut crisis_state,
        mut spawn_positions,
        mut objectives,
        monolith_investigation,
        prices,
        templates,
        mut run_score_state,
        mut legendary_threat_state,
        mut player_intro_state,
        mut start_locations,
        mut assigned_start_locations,
        mut run_spawned_objs,
        mut intro_encounter_state,
        mut settlement_crisis_state,
        mut player_world_presence_state,
        mut safe_logout_telemetry,
    ): (
        ResMut<CrisisState>,
        ResMut<SpawnPositions>,
        ResMut<Objectives>,
        Res<MonolithInvestigation>,
        Res<Prices>,
        Res<Templates>,
        ResMut<RunScoreState>,
        ResMut<LegendaryThreatState>,
        ResMut<PlayerIntroState>,
        ResMut<StartLocations>,
        ResMut<AssignedStartLocations>,
        ResMut<RunSpawnedObjs>,
        ResMut<IntroEncounterState>,
        ResMut<SettlementCrisisState>,
        ResMut<PlayerWorldPresenceState>,
        ResMut<SafeLogoutTelemetryState>,
    ),
    mut initial_encounter_state: ResMut<InitialEncounterState>,
    mut hero_query: Query<
        (
            Entity,
            &PlayerId,
            &Id,
            &Name,
            &Template,
            &Skills,
            &TrueDeath,
            &StateDead,
            Option<&BoundMonolith>,
        ),
        With<SubclassHero>,
    >,
    // p0: score sweep over all owned objects; p1: bound-monolith inventory for
    // the Soulshard top-up (mutable Inventory access, hence the ParamSet).
    mut world_queries: ParamSet<(
        Query<(&PlayerId, &Inventory, &Class, &Template, &State)>,
        Query<&mut Inventory, With<Monolith>>,
    )>,
    villager_query: Query<(Entity, &PlayerId, &Id, Option<&StateDead>), With<SubclassVillager>>,
    cleanup_query: Query<
        (
            Entity,
            &Id,
            &PlayerId,
            &Position,
            Option<&CrisisAssaultUnit>,
        ),
        (Without<SubclassHero>, Without<SubclassVillager>),
    >,
    target_query: Query<(
        Entity,
        Option<&Target>,
        Option<&VisibleTarget>,
        Option<&TaskTarget>,
    )>,
) {
    for (entity, player_id, id, name, template, skills, true_death, state_dead, bound_monolith) in
        hero_query.iter_mut()
    {
        if (game_tick.0 - true_death.true_death_at) > 10 * TICKS_PER_SEC {
            info!("Hero true death: {:?}", id.0);

            // Calculate highest crisis tier survived (+1000 XP per tier for legacy XP)
            let crisis_tier = crisis_state.get(&player_id.0).map(crisis_tier).unwrap_or(0);

            let crisis_bonus_xp = crisis_tier * 1000;
            let total_xp = skills.get_total_xp() + crisis_bonus_xp;

            let run_score = run_score_state
                .get(&player_id.0)
                .cloned()
                .unwrap_or_default();

            let mut total_wealth_value = 0;
            let mut structures_alive = 0;
            let mut upgrades = 0;

            let score_obj_query = world_queries.p0();
            for (obj_player_id, inventory, class, obj_template, obj_state) in score_obj_query.iter()
            {
                if obj_player_id.0 != player_id.0 || Obj::is_dead(obj_state) {
                    continue;
                }

                for item in inventory.items.iter() {
                    if item.class == GOLD {
                        total_wealth_value += item.quantity;
                    } else {
                        let item_value = prices
                            .find_sell_price(
                                item.name.clone(),
                                item.subclass.clone(),
                                item.class.clone(),
                            )
                            .unwrap_or(1);
                        total_wealth_value += item_value.max(1) * item.quantity;
                    }
                }

                if class.0 == CLASS_STRUCTURE {
                    structures_alive += 1;
                    let template = templates.obj_templates.get(obj_template.0.clone());
                    total_wealth_value += template.build_cost.unwrap_or(0);
                    if template.level.unwrap_or(0) > 0 {
                        upgrades += 1;
                    }
                }
            }

            let villagers_alive = villager_query
                .iter()
                .filter(|(_, villager_player_id, _, dead)| {
                    villager_player_id.0 == player_id.0 && dead.is_none()
                })
                .count() as i32;

            let active_legendary_days = legendary_threat_state
                .get(&player_id.0)
                .and_then(|threat| threat.active_since_tick)
                .map(|active_since| {
                    let end_tick = legendary_threat_state
                        .get(&player_id.0)
                        .and_then(|threat| threat.defeated_at_tick)
                        .unwrap_or(game_tick.0);
                    ((end_tick - active_since).max(0) / GAME_TICKS_PER_DAY) + 1
                })
                .unwrap_or(0);

            let monolith_sealed = monolith_investigation
                .get(&player_id.0)
                .map(|progress| progress.sealed)
                .unwrap_or(false);
            let completed_objectives = completed_objectives_count(objectives.get(&player_id.0));
            let total_skill_levels: i32 = skills.get_levels().values().sum();
            let days_survived = player_days_survived(&game_tick, player_id.0, &player_intro_state);
            let nights_survived = days_survived;
            let score_inputs = RunScoreInputs {
                days_survived,
                nights_survived,
                waves_survived: run_score.waves_survived,
                active_legendary_days,
                hero_rank: template.0.clone(),
                total_skill_levels,
                total_xp,
                total_wealth_value,
                structures_alive,
                upgrades,
                repairs: run_score.repairs,
                villagers_alive,
                crisis_tier,
                enemies_killed: run_score.enemies_killed,
                elites_killed: run_score.elites_killed,
                captains_killed: run_score.captains_killed,
                legendary_kills: run_score.legendary_kills,
                hideouts_cleared: run_score.hideouts_cleared,
                completed_objectives,
                monolith_sealed,
            };
            let score_breakdown = calculate_run_score_breakdown(&score_inputs);
            let total_score =
                score_total_from_breakdown(&score_breakdown, run_score.highest_pressure_level);

            let killer = state_dead.killer.clone();

            let fate = match killer.as_str() {
                "Unknown" => "Killed by unknown".to_string(),
                "Dehydration" => "Killed by Dehydration".to_string(),
                "Exhaustion" => "Killed by Exhaustion".to_string(),
                "Starvation" => "Killed by Starvation".to_string(),
                "Burns" => "Killed by burns".to_string(),
                _ => "Killed by a ".to_string() + &killer,
            };

            let database_event = DatabaseEvent::AddScore {
                player_id: player_id.0,
                hero_name: name.0.clone(),
                hero_rank: template.0.clone(),
                total_xp: total_xp,
                total_score,
                score_survival: score_breakdown.survival,
                score_progression: score_breakdown.progression,
                score_wealth: score_breakdown.wealth,
                score_defense: score_breakdown.defense,
                score_valor: score_breakdown.valor,
                score_legacy: score_breakdown.legacy,
                days_survived,
                highest_pressure_level: run_score.highest_pressure_level,
                waves_survived: run_score.waves_survived,
                legendary_kills: run_score.legendary_kills,
                hideouts_cleared: run_score.hideouts_cleared,
                fate: fate.clone(),
                crisis_tier: crisis_tier,
            };

            send_to_database(database_event, &database_managers);

            let packet = ResponsePacket::InfoTrueDeath {
                hero_name: name.0.clone(),
                hero_rank: template.0.clone(),
                total_xp: total_xp,
                score_total: total_score,
                score_breakdown,
                days_survived,
                waves_survived: run_score.waves_survived,
                highest_pressure_level: run_score.highest_pressure_level,
                legendary_kills: run_score.legendary_kills,
                hideouts_cleared: run_score.hideouts_cleared,
                fate: fate.clone(),
                crisis_tier: crisis_tier,
            };
            send_to_client(player_id.0, packet, &clients);

            // Clean up crisis state and spawn position for this player
            crisis_state.remove(&player_id.0);
            spawn_positions.remove(&player_id.0);
            run_score_state.remove(&player_id.0);
            legendary_threat_state.remove(&player_id.0);
            // Also drop the scripted per-run state: a dead player's intro /
            // initial-encounter chains would otherwise keep spawning enemies
            // at the released start location (observed: a fresh Cave Bat at
            // the old shipwreck after the cleanup sweep ran).
            objectives.remove(&player_id.0);
            player_intro_state.remove(&player_id.0);
            initial_encounter_state.remove(&player_id.0);
            intro_encounter_state.remove(&player_id.0);
            if let Some(personal_crisis) = settlement_crisis_state.get(&player_id.0) {
                if personal_crisis.assault_id.is_some() {
                    info!(
                        "personal_crisis_assault_true_death_cleanup player_id={} phase={:?} assault_id={:?} generation={} game_tick={} tracked_units={} recovery_required={}",
                        player_id.0,
                        personal_crisis.phase,
                        personal_crisis.assault_id,
                        personal_crisis.assault_spawn_generation,
                        game_tick.0,
                        personal_crisis.assault_unit_ids.len(),
                        personal_crisis.assault_recovery_required
                    );
                }
            }
            settlement_crisis_state.remove(&player_id.0);
            remove_player_presence_for_run_cleanup(
                player_id.0,
                game_tick.0,
                &mut player_world_presence_state,
                &mut safe_logout_telemetry,
            );

            // Release this player's start location back to the pool so a new hero can
            // spawn there. (In-memory: lost on restart, same as StartLocations itself.)
            if let Some(start_location) = assigned_start_locations.remove(&player_id.0) {
                let location_name = start_location.name.clone();
                start_locations.push(start_location);
                info!(
                    "Released start location '{}' back to the pool after true death of player {}",
                    location_name, player_id.0
                );
            }

            // Transfer villagers to merchant player. Their old-run queued work
            // is cleared below even though the entities themselves survive.
            let mut ended_run_villager_ids = Vec::new();
            for (villager_entity, villager_player_id, villager_id, _) in villager_query.iter() {
                if villager_player_id.0 == player_id.0 {
                    info!("Transferring villager: {:?}", villager_id.0);
                    ended_run_villager_ids.push(villager_id.0);

                    // Add Update Event for Player Id change
                    commands.trigger(UpdateObj {
                        entity: villager_entity,
                        attrs: vec![(PLAYER_ID.to_string(), MERCHANT_PLAYER_ID.to_string())],
                    });
                }
            }

            // Clean only objects owned by or explicitly attributed to this run.
            // Nearby unrelated world hostiles survive start-location recycling;
            // villagers are kept because they were just transferred to the
            // merchant for re-hire.
            let run_objs = run_spawned_objs.remove(&player_id.0).unwrap_or_default();
            let mut removed_obj_ids: Vec<i32> = Vec::new();
            for (obj_entity, obj_id, obj_player_id, _obj_pos, crisis_assault) in
                cleanup_query.iter()
            {
                // Another player's explicitly attributed personal assault is
                // never collateral cleanup.
                if crisis_assault
                    .map(|assault| assault.owner_player_id != player_id.0)
                    .unwrap_or(false)
                {
                    continue;
                }
                let owned_by_dead_player = obj_player_id.0 == player_id.0;
                let spawned_for_this_run = run_objs.contains(&obj_id.0);
                let attributed_to_dead_run = crisis_assault
                    .map(|assault| assault.owner_player_id == player_id.0)
                    .unwrap_or(false);

                if owned_by_dead_player || spawned_for_this_run || attributed_to_dead_run {
                    removed_obj_ids.push(obj_id.0);
                    ids.remove_obj(obj_id.0);
                    if entity_map.get_entity(obj_id.0) == Some(obj_entity) {
                        commands.trigger(RemoveObj { entity: obj_entity });
                    } else {
                        commands.entity(obj_entity).try_despawn();
                    }
                }
            }

            // Drop pending map events for everything just removed (and the
            // hero) — an in-flight MoveEvent applying to a despawned entity
            // panics when its completion command runs.
            removed_obj_ids.push(id.0);
            let ended_run_object_ids = removed_obj_ids
                .iter()
                .copied()
                .chain(run_objs.iter().copied())
                .chain(ended_run_villager_ids.iter().copied())
                .collect::<HashSet<_>>();
            game_events.retain(|_, event| {
                !game_event_belongs_to_ended_run(
                    &event.event_type,
                    player_id.0,
                    &ended_run_object_ids,
                    &entity_map,
                    &map_events,
                )
            });
            map_events.retain(|_, map_event| {
                !ended_run_object_ids.contains(&map_event.obj_id)
                    && !visible_event_references_ended_run(
                        &map_event.event_type,
                        &ended_run_object_ids,
                    )
            });
            clear_assault_target_references(&ended_run_object_ids, &mut commands, &target_query);

            // The next run bound to this monolith must get the same first-death
            // safety net: restore its Soulshards to the starting amount.
            if let Some(bound_monolith) = bound_monolith {
                if let Some(monolith_entity) = entity_map.get_entity(bound_monolith.id) {
                    let mut monolith_inventory_query = world_queries.p1();
                    if let Ok(mut monolith_inventory) =
                        monolith_inventory_query.get_mut(monolith_entity)
                    {
                        let current = soulshard_count(&monolith_inventory);
                        if current < INIT_MONOLITH_SOULSHARDS {
                            monolith_inventory.new(
                                ids.new_item_id(),
                                SOULSHARD.to_string(),
                                INIT_MONOLITH_SOULSHARDS - current,
                                &templates.item_templates,
                            );
                        }
                    }
                }
            }

            // Remove hero
            commands.entity(entity).despawn();

            // Remove hero from ids
            ids.remove_hero(player_id.0, id.0);

            // Remove entity from entity map
            entity_map.remove_obj(id.0);

            // Remove explored tiles from player
            explored_map.remove(&player_id.0);
        }
    }
}

fn state_dead_system(
    state_dead_query: Query<(&Id, &Position, &Subclass), With<StateDead>>,
    mut obj_query: Query<(&Id, &Position, &mut Effects)>,
) {
    for (_dead_id, dead_pos, dead_subclass) in state_dead_query.iter() {
        if *dead_subclass == Subclass::Wall {
            for (_id, pos, mut effects) in obj_query.iter_mut() {
                if dead_pos == pos {
                    // Remove fortified effect
                    effects.0.remove(&Effect::Fortified);
                }
            }
        } else if *dead_subclass == Subclass::Watchtower {
            for (_id, pos, mut effects) in obj_query.iter_mut() {
                if dead_pos == pos {
                    // Remove watchtower light effect
                    effects.0.remove(&Effect::WatchtowerLight);
                }
            }
        }
    }
}

fn remove_dead_system(
    mut commands: Commands,
    game_tick: ResMut<GameTick>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    initial_encounter_state: Res<InitialEncounterState>,
    state_dead_query: Query<(Entity, &Id, &Position, &Inventory, &StateDead)>,
    mut map_events: ResMut<MapEvents>,
) {
    // Every 10 ticks
    if (game_tick.0 % 10) == 0 {
        for (entity, id, pos, inventory, dead_state) in state_dead_query.iter() {
            if object_belongs_to_protected_run(id.0, &ids, &presence)
                || initial_encounter_object_is_protected(id.0, &initial_encounter_state, &presence)
            {
                continue;
            }
            if (game_tick.0 - dead_state.dead_at) > 500 {
                // Remove obj observer event
                commands.trigger(RemoveObj { entity: entity });
            } else if (game_tick.0 - dead_state.dead_at) > 100 {
                // Remove dead object faster if it contains no items
                if inventory.items.is_empty() {
                    // Remove obj observer event
                    commands.trigger(RemoveObj { entity: entity });
                }
            }
        }
    }
}

fn despawn_wandering_npc_system(
    mut commands: Commands,
    game_tick: ResMut<GameTick>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    wandering_behavior_query: Query<(Entity, &Id, &Position, &WanderingBehavior)>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
) {
    // Every 100 ticks
    if (game_tick.0 % 100) == 0 {
        trace!("Attempting to despawn NPCs that have been wandering for 10 moves...");
        for (entity, id, pos, wandering_behavior) in wandering_behavior_query.iter() {
            if object_belongs_to_protected_run(id.0, &ids, &presence) {
                continue;
            }
            if wandering_behavior.num_moves > 10 {
                trace!("Despawning NPC: {:?} due to excess wandering.", id.0);

                // Remove Thinker
                commands.entity(entity).remove::<ThinkerBuilder>();

                // Remove events that are cancellable
                let mut events_to_remove = Vec::new();

                // TODO move this into a function
                for (map_event_id, map_event) in map_events.iter() {
                    if map_event.obj_id == id.0 {
                        events_to_remove.push(*map_event_id);
                    }
                }

                let event_type = GameEventType::CancelMapEventsById {
                    event_ids: events_to_remove,
                };
                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                // Remove obj observer event
                commands.trigger(RemoveObj { entity: entity });
            }
        }
    }
}

fn snapshot_system(world: &mut World) {
    let game_tick = world.resource::<GameTick>();
    if game_tick.0 % 100 == 0 {
        trace!("Taking snapshot at {}...", game_tick.0);

        let scene = DynamicScene::from_world(&world);
        let registry = world.resource::<AppTypeRegistry>().read();

        /*for registration in registry.iter() {
            println!("{}", registration.type_info().type_path());
        }*/
        //let serialized_scene = build_scene(world)
        //    .serialize(&registry)
        //    .expect("serialization failed");

        // Scenes can be serialized like this:
        //let type_registry = type_registry.read();
        let serialized_scene = scene.serialize(&registry).unwrap();

        // Showing the scene in the console
        trace!("Scene length: {}", serialized_scene.len());

        IoTaskPool::get()
            .spawn(async move {
                // Write the scene RON data to file
                File::create(format!("dynamic_scene.ron"))
                    .and_then(|mut file| file.write(serialized_scene.as_bytes()))
                    .expect("Error while writing scene to file");
            })
            .detach();
    }
}

fn update_game_tick(
    mut commands: Commands,
    mut game_tick: ResMut<GameTick>,
    map: Res<Map>,
    clients: Res<Clients>,
    ids: Res<Ids>,
    active_infos: Res<ActiveInfos>,
    presence: Res<PlayerWorldPresenceState>,
    mut attrs: Query<(Entity, &mut Thirst, &mut Hunger, &mut Tired, &mut Heat)>,
    obj_query: Query<(&Id, &PlayerId, &Position)>,
    dehydrated: Query<&Dehydrated>,
    starving: Query<&Starving>,
    exhausted: Query<&Exhausted>,
    sheltered: Query<&Sheltered>,
    state_query: Query<&State>,
) {
    game_tick.0 = game_tick.0 + 1;

    // Update thirst
    for (entity, mut thirst, mut hunger, mut tired, mut heat) in &mut attrs {
        let Ok((id, player_id, pos)) = obj_query.get(entity) else {
            error!("No obj found for entity: {:?}", entity);
            continue;
        };

        if entity_belongs_to_protected_run(id, player_id, &presence) {
            continue;
        }

        let current_thirst_level = thirst.num_to_string();
        let current_hunger_level = hunger.num_to_string();
        let current_tiredness_level = tired.num_to_string();

        //info!("thirst: {:?}", current_thirst_level);
        //info!("hunger: {:?}", current_hunger_level);
        //info!("tiredness: {:?}", current_tiredness_level);
        //info!("--------------------------------");

        // Do not increase thirst, hunger, tiredness if drinking, eating, sleeping
        if let Ok(state) = state_query.get(entity) {
            if *state != State::Drinking {
                thirst.update_by_tick_amount(1.0);
            }
        }

        if let Ok(state) = state_query.get(entity) {
            if *state != State::Eating {
                hunger.update_by_tick_amount(1.0);
            }
        }

        if let Ok(state) = state_query.get(entity) {
            if *state != State::Sleeping {
                tired.update_by_tick_amount(1.0);
            }
        }

        // Update heat attribute every hour
        if game_tick.0 % GAME_HOUR == 0 {
            if !sheltered.get(entity).is_ok() {
                let tile_temperature = map.tile_temperature(pos.x, pos.y);
                let tile_moisture = map.tile_moisture(pos.x, pos.y);

                debug!(
                    "tile_temperature: {:?} tile_moisture: {:?}",
                    tile_temperature, tile_moisture
                );
                /*let current_temperature = Map::get_temperature(
                    Season::Winter,
                    1,
                    tile_temperature,
                    tile_moisture,
                    Weather::ClearSunny,
                );*/
                let current_temperature = 5.0;
                trace!("Current temperature: {:?}", current_temperature);

                let clothing_mod = 1.0;

                let heat_level_change = (current_temperature - COMFORT_TEMPERATURE) * clothing_mod;
                trace!("Heat level change: {:?}", heat_level_change);

                heat.update(heat_level_change);

                trace!("Heat level: {:?}", heat.heat);
            } else {
                heat.update_to_comfortable(50.0);
                trace!("Returning to comform, heat level: {:?}", heat.heat);
            }
        }

        if thirst.thirst > DEHYDRATED_SCORE {
            if let Ok(_dehydrated) = dehydrated.get(entity) {
                // Do nothing
            } else {
                info!("Adding Dehydrated at tick: {:?}", game_tick.0);
                commands.entity(entity).insert(Dehydrated {
                    at_tick: game_tick.0,
                });
            }
        }

        if hunger.hunger > STARVING_SCORE {
            if let Ok(_starving) = starving.get(entity) {
                // Do nothing
            } else {
                info!("Adding Starving at tick: {:?}", game_tick.0);
                commands.entity(entity).insert(Starving {
                    at_tick: game_tick.0,
                });
            }
        }

        if tired.tired > EXHAUSTED_SCORE {
            if let Ok(_exhausted) = exhausted.get(entity) {
                // Do nothing
            } else {
                info!("Adding Exhausted at tick: {:?}", game_tick.0);
                commands.entity(entity).insert(Exhausted {
                    at_tick: game_tick.0,
                });
            }
        }

        trace!("Current thirst: {:?} new thirst: {:?} Current hunger: {:?} new hunger: {:?} Current tiredness: {:?} new tiredness: {:?}", current_thirst_level, thirst.num_to_string(), current_hunger_level, hunger.num_to_string(), current_tiredness_level, tired.num_to_string());

        if current_thirst_level != thirst.num_to_string()
            || current_hunger_level != hunger.num_to_string()
            || current_tiredness_level != tired.num_to_string()
        {
            // TODO consider a thrist, hunder, tiredness Changed component system
            if let Some(_active_info) = active_infos.get(&(id.0, ActiveInfoType::Obj)) {
                info!(
                    "Thirst level changed: {:?} -> {:?}",
                    current_thirst_level,
                    thirst.num_to_string()
                );
                info!(
                    "Hunger level changed: {:?} -> {:?}",
                    current_hunger_level,
                    hunger.num_to_string()
                );
                info!(
                    "Tiredness level changed: {:?} -> {:?}",
                    current_tiredness_level,
                    tired.num_to_string()
                );

                let item_needs_packet: ResponsePacket = ResponsePacket::InfoNeedsUpdate {
                    id: id.0,
                    thirst: thirst.num_to_string(),
                    hunger: hunger.num_to_string(),
                    tiredness: tired.num_to_string(),
                };

                send_to_client(player_id.0, item_needs_packet, &clients);
            }

            // TODO consider a thrist, hunder, tiredness Changed component system
            if ids.is_hero(id.0) {
                let item_needs_packet: ResponsePacket = ResponsePacket::InfoNeedsUpdate {
                    id: id.0,
                    thirst: thirst.num_to_string(),
                    hunger: hunger.num_to_string(),
                    tiredness: tired.num_to_string(),
                };

                send_to_client(player_id.0, item_needs_packet, &clients);
            }
        }

        /*debug!(
            "Thirst: {:?} Hunger: {:?} Tired: {:?}",
            thirst.thirst, hunger.hunger, tired.tired
        );*/
        // Is thirsty
        /*if thirst.thirst >= 80.0 {
            morale.morale -= morale.per_tick;
        } else if thirst.thirst >= 90.0 {
            morale.morale -= 2.0 * morale.per_tick;
        } else if thirst.thirst >= 95.0 {
            morale.morale -= 5.0 * morale.per_tick;
        } else {
            morale.morale += morale.per_tick;

            if morale.morale >= 100.0 {
                morale.morale = 100.0;
            }
        }*/

        //debug!("thirst: {:?} morale: {:?}", thirst.thirst, morale.morale);
    }
}

fn stamina_recovery_system(
    game_tick: Res<GameTick>,
    presence: OptionalPlayerWorldPresence,
    mut stats_query: Query<
        (Option<&Id>, Option<&PlayerId>, &mut Stats, &LastCombatTick),
        Without<StateDead>,
    >,
) {
    if game_tick.0 % TICKS_PER_SEC != 0 {
        return;
    }

    for (id, player_id, mut stats, last_combat_tick) in stats_query.iter_mut() {
        if id
            .zip(player_id)
            .map(|(id, player_id)| entity_belongs_to_protected_run(id, player_id, &presence))
            .or_else(|| player_id.map(|player_id| is_owner_offline_protected(player_id, &presence)))
            .unwrap_or(false)
        {
            continue;
        }
        let in_combat = game_tick.0.saturating_sub(last_combat_tick.0) < 30;
        if let (Some(stamina), Some(base_stamina)) = (stats.stamina, stats.base_stamina) {
            if stamina < base_stamina {
                // Recover faster out of combat (5/sec) vs in combat (1/sec)
                let recovery = if in_combat { 1 } else { 5 };
                stats.stamina = Some((stamina + recovery).min(base_stamina));
            }
        }

        if let (Some(mana), Some(base_mana)) = (stats.mana, stats.base_mana) {
            if base_mana > 0 && mana < base_mana && !in_combat {
                stats.mana = Some((mana + 2).min(base_mana));
            }
        }
    }
}

fn combat_lock_interrupt_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut query: Query<
        (
            Entity,
            &mut State,
            &LastCombatTick,
            Option<&mut EventExecuting>,
        ),
        (
            Changed<LastCombatTick>,
            Or<(With<SubclassHero>, With<SubclassVillager>)>,
        ),
    >,
) {
    for (entity, mut state, last_combat_tick, event_executing) in query.iter_mut() {
        if !is_combat_locked(game_tick.0, last_combat_tick)
            || !is_peaceful_interruptible_state(&state)
        {
            continue;
        }

        if let Some(mut event_executing) = event_executing {
            event_executing.state = EventExecutingState::None;
        }

        *state = State::None;
        commands.trigger(CancelEvents { entity });
    }
}

// Drives the merchant ship one tile at a time toward its current sail
// destination. Mirrors the tax_collector move_to_pos pattern but is scoped
// to entities with a `Merchant` component so it doesn't interfere with the
// big-brain-driven NPCs.
//
// Skipped when:
//   - `sail_state` isn't a sailing variant (AtEmpire / AtLanding → idle)
//   - already at destination (the arrival_system handles transition)
//   - already moving (an EventInProgress exists, the move event resolves
//     and this system fires again next tick)
fn merchant_sailing_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    initial_encounter_state: Res<InitialEncounterState>,
    presence: Res<PlayerWorldPresenceState>,
    mut merchant_query: Query<(Entity, ObjStatQuery, &Merchant)>,
) {
    for (entity, mut obj, merchant) in merchant_query.iter_mut() {
        if initial_encounter_object_is_protected(obj.id.0, &initial_encounter_state, &presence) {
            continue;
        }
        let dest = match merchant.sail_state {
            MerchantSailState::SailingToLanding => merchant.landing_at,
            MerchantSailState::SailingToEmpire => merchant.trade_port,
            MerchantSailState::AtEmpire | MerchantSailState::AtLanding => continue,
        };

        if *obj.pos == dest {
            // Arrived; merchant_arrival_system handles the transition.
            continue;
        }

        // Only step the move when we're idle; if the previous move event is
        // still resolving, wait for it.
        if *obj.state != State::None {
            continue;
        }

        if obj.effects.has(Effect::Stunned) {
            continue;
        }

        // Speed-aware tile duration. Merchant template has base_speed: 3 so
        // each hex takes ~33 ticks (~3.3s) which feels like a sail.
        let npc_speed = obj.stats.base_speed.unwrap_or(1).max(1);
        let effect_speed_mod = obj.effects.get_speed_effects(&templates);
        let move_duration =
            (BASE_MOVE_TICKS * (BASE_SPEED / npc_speed as f32) * (1.0 / effect_speed_mod)) as i32;

        let path_result = Map::find_fast_path(
            *obj.pos,
            dest,
            &map,
            obj.player_id.0,
            Vec::new(),
            false, // landwalk: ships don't walk on land
            true,  // waterwalk
            false, // mountainwalk
            false, // ignore_goal_terrain_type
            true,  // allow_attackable_blockers
        );

        let Some((path, _cost)) = path_result else {
            error!(
                "merchant_sailing_system: no water path from {:?} to {:?} for merchant {}",
                *obj.pos, dest, obj.id.0
            );
            continue;
        };

        if path.len() < 2 {
            // Already at destination (defensive — pos == dest guard above
            // should have caught this).
            continue;
        }

        let next_pos = &path[1];

        commands.trigger(StateChange {
            entity,
            new_state: State::Moving,
        });

        let move_event = VisibleEvent::MoveEvent {
            src: *obj.pos,
            dst: Position {
                x: next_pos.0,
                y: next_pos.1,
            },
        };

        map_events.new(obj.id.0, game_tick.0 + move_duration, move_event);

        // NOTE: this used to also insert `EventInProgress` as a "currently sailing"
        // guard, but nothing ever removed it after a move completed (unlike the
        // gather/refine/craft paths), so the merchant sailed exactly one tile then
        // froze forever. The `*obj.state != State::None` check above already gates
        // re-issuing a move while one is in flight, so the guard was redundant.
    }
}

// Watches for the moment a sailing merchant lands at landing_at (or returns
// to trade_port) and fires the on-arrival side effects: restock, notice,
// speech, and scheduling the next lifecycle phase events.
//
// Polls every 10 ticks (1 second) — finer granularity than the trade window
// needs and cheap with at most one merchant per player.
fn merchant_arrival_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    initial_encounter_state: Res<InitialEncounterState>,
    presence: Res<PlayerWorldPresenceState>,
    mut merchant_query: Query<(&Id, &mut Merchant, &Position, &mut Inventory)>,
) {
    if game_tick.0 % 10 != 0 {
        return;
    }

    for (id, mut merchant, pos, mut inventory) in merchant_query.iter_mut() {
        if initial_encounter_object_is_protected(id.0, &initial_encounter_state, &presence) {
            continue;
        }
        match merchant.sail_state {
            MerchantSailState::SailingToLanding if *pos == merchant.landing_at => {
                merchant.sail_state = MerchantSailState::AtLanding;

                // Find the player this merchant serves. With one merchant per
                // initial-encounter entry today, scan the map for a matching
                // merchant_id; cheap (at most a handful of entries).
                let player_id = initial_encounter_state
                    .iter()
                    .find(|(_, entry)| entry.merchant_id == id.0)
                    .map(|(pid, _)| *pid);

                // Refresh wanted-item list (Prices resource is left intact so
                // supply/demand drift from prior trades persists across cycles).
                merchant.wanted_items = MERCHANT_WANTED_SUBCLASSES
                    .iter()
                    .map(|s| WantedItem::new_by_subclass((*s).to_string()))
                    .collect();

                // Restock inventory: top up any item that's below its target qty.
                for (item_name, target_qty) in MERCHANT_INVENTORY.iter() {
                    let current = inventory
                        .items
                        .iter()
                        .find(|i| i.name == *item_name)
                        .map(|i| i.quantity)
                        .unwrap_or(0);

                    if current < *target_qty {
                        inventory.new(
                            ids.new_item_id(),
                            (*item_name).to_string(),
                            *target_qty - current,
                            &templates.item_templates,
                        );
                    }
                }

                if let Some(player_id) = player_id {
                    let notice = ResponsePacket::Notice {
                        noticemsg: "A traveling merchant has set up a stall by the shore. Bring goods to trade — they will not stay for long.".to_string(),
                        expiry: Some(8000),
                    };
                    send_to_client(player_id, notice, &clients);

                    map_events.new(
                        id.0,
                        game_tick.0 + 4,
                        VisibleEvent::SpeechEvent {
                            speech: "Wares! Fresh wares from across the sea! Come trade before the tide turns!".to_string(),
                            intensity: 4,
                        },
                    );

                    let leaving_soon_id = ids.new_map_event_id();
                    game_events.insert(
                        leaving_soon_id,
                        GameEvent {
                            event_id: leaving_soon_id,
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + MERCHANT_LEAVING_SOON_OFFSET,
                            event_type: GameEventType::MerchantLeavingSoon {
                                merchant_id: id.0,
                                player_id,
                            },
                        },
                    );
                    let departure_id = ids.new_map_event_id();
                    game_events.insert(
                        departure_id,
                        GameEvent {
                            event_id: departure_id,
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + MERCHANT_DEPARTURE_OFFSET,
                            event_type: GameEventType::MerchantDeparture {
                                merchant_id: id.0,
                                player_id,
                            },
                        },
                    );

                    info!(
                        "merchant_arrival_system: merchant {} arrived at landing for player {}",
                        id.0, player_id
                    );
                } else {
                    error!(
                        "merchant_arrival_system: no player_id found for merchant {}",
                        id.0
                    );
                }
            }
            MerchantSailState::SailingToEmpire if *pos == merchant.trade_port => {
                merchant.sail_state = MerchantSailState::AtEmpire;

                let player_id = initial_encounter_state
                    .iter()
                    .find(|(_, entry)| entry.merchant_id == id.0)
                    .map(|(pid, _)| *pid);

                if let Some(player_id) = player_id {
                    // Schedule the next return cycle.
                    let arrival_id = ids.new_map_event_id();
                    game_events.insert(
                        arrival_id,
                        GameEvent {
                            event_id: arrival_id,
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + MERCHANT_RETURN_GAP,
                            event_type: GameEventType::MerchantArrival {
                                merchant_id: id.0,
                                player_id,
                            },
                        },
                    );

                    info!(
                        "merchant_arrival_system: merchant {} arrived at empire for player {}; next arrival at tick {}",
                        id.0,
                        player_id,
                        game_tick.0 + MERCHANT_RETURN_GAP
                    );
                }
            }
            _ => {}
        }
    }
}

fn stamina_update_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    hero_query: Query<(&Id, &PlayerId, &Stats), (With<SubclassHero>, Without<StateDead>)>,
) {
    if game_tick.0 % TICKS_PER_SEC != 0 {
        return;
    }

    for (id, player_id, stats) in hero_query.iter() {
        if let (Some(stamina), Some(base_stamina)) = (stats.stamina, stats.base_stamina) {
            if stamina < base_stamina {
                let packet = ResponsePacket::InfoStaminaUpdate {
                    id: id.0,
                    stamina: stamina,
                };
                send_to_client(player_id.0, packet, &clients);
            }
        }
    }
}

fn mana_update_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    hero_query: Query<(&Id, &PlayerId, &Stats), (With<SubclassHero>, Without<StateDead>)>,
) {
    if game_tick.0 % TICKS_PER_SEC != 0 {
        return;
    }

    for (id, player_id, stats) in hero_query.iter() {
        if let (Some(mana), Some(base_mana)) = (stats.mana, stats.base_mana) {
            if base_mana > 0 {
                let packet = ResponsePacket::InfoManaUpdate { id: id.0, mana };
                send_to_client(player_id.0, packet, &clients);
            }
        }
    }
}

/// Returns the warning stage (0..=3) when `game_tick` lands exactly on one of
/// the needs-death countdown boundaries. Relies on this being evaluated once
/// per tick value — the same contract `vital_dialogue_system` uses.
fn needs_warning_stage(
    game_tick: i32,
    at_tick: i32,
    warning1_at: i32,
    warning2_at: i32,
    death_at: i32,
) -> Option<usize> {
    if game_tick == at_tick + 5 {
        Some(0)
    } else if game_tick == at_tick + warning1_at {
        Some(1)
    } else if game_tick == at_tick + warning2_at {
        Some(2)
    } else if game_tick == at_tick + death_at - 20 {
        Some(3)
    } else {
        None
    }
}

// Needs are the single largest killer of runs, and the hero's death countdowns
// were silent (staged warnings existed only as villager speech bubbles). Surface
// each countdown stage as an explicit notice naming the counter-action.
fn hero_needs_warning_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    presence: Res<PlayerWorldPresenceState>,
    dehydrated_query: Query<(&PlayerId, &Dehydrated), (With<SubclassHero>, Without<StateDead>)>,
    starving_query: Query<(&PlayerId, &Starving), (With<SubclassHero>, Without<StateDead>)>,
    exhausted_query: Query<(&PlayerId, &Exhausted), (With<SubclassHero>, Without<StateDead>)>,
) {
    const DEHYDRATED_WARNINGS: [&str; 4] = [
        "You are dehydrated! Drink water before it kills you.",
        "Dehydration is draining your life. Find water now!",
        "You are about to die of thirst. Drink immediately!",
        "Seconds from death — drink NOW!",
    ];
    const STARVING_WARNINGS: [&str; 4] = [
        "You are starving! Eat food before it kills you.",
        "Starvation is draining your life. Eat now!",
        "You are about to starve to death. Eat immediately!",
        "Seconds from death — eat NOW!",
    ];
    const EXHAUSTED_WARNINGS: [&str; 4] = [
        "You are exhausted! Sleep before you collapse.",
        "Exhaustion is draining your life. Sleep now!",
        "You are about to collapse. Sleep immediately!",
        "Seconds from death — sleep NOW!",
    ];

    let mut warnings: Vec<(i32, &str)> = Vec::new();

    for (player_id, dehydrated) in dehydrated_query.iter() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        if let Some(stage) = needs_warning_stage(
            game_tick.0,
            dehydrated.at_tick,
            DEHYDRATED_WARNING1_AT,
            DEHYDRATED_WARNING2_AT,
            DEHYDRATED_DEATH_AT,
        ) {
            warnings.push((player_id.0, DEHYDRATED_WARNINGS[stage]));
        }
    }

    for (player_id, starving) in starving_query.iter() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        if let Some(stage) = needs_warning_stage(
            game_tick.0,
            starving.at_tick,
            STARVING_WARNING1_AT,
            STARVING_WARNING2_AT,
            STARVING_DEATH_AT,
        ) {
            warnings.push((player_id.0, STARVING_WARNINGS[stage]));
        }
    }

    for (player_id, exhausted) in exhausted_query.iter() {
        if is_owner_offline_protected(player_id, &presence) {
            continue;
        }
        if let Some(stage) = needs_warning_stage(
            game_tick.0,
            exhausted.at_tick,
            EXHAUSTED_WARNING1_AT,
            EXHAUSTED_WARNING2_AT,
            EXHAUSTED_DEATH_AT,
        ) {
            warnings.push((player_id.0, EXHAUSTED_WARNINGS[stage]));
        }
    }

    for (player_id, msg) in warnings {
        let packet = ResponsePacket::Notice {
            noticemsg: msg.to_string(),
            expiry: Some(8000),
        };
        send_to_client(player_id, packet, &clients);
    }
}

fn dehydrated_system(
    mut commands: Commands,
    game_tick: ResMut<GameTick>,
    mut map_events: ResMut<MapEvents>,
    presence: Res<PlayerWorldPresenceState>,
    mut dehydrated_query: Query<
        (Entity, &Id, &PlayerId, &mut State, &Dehydrated),
        Without<StateDead>,
    >,
) {
    for (entity, id, player_id, mut state, dehydrated) in dehydrated_query.iter_mut() {
        if entity_belongs_to_protected_run(id, player_id, &presence) {
            continue;
        }
        if game_tick.0 - dehydrated.at_tick > DEHYDRATED_DEATH_AT {
            // Add state dead event
            debug!(
                "Dehydrated: at tick: {:?} Adding state dead event for {:?}",
                game_tick.0, id.0
            );

            // Remove thinker
            debug!("Removing thinker for {:?}", id.0);
            commands.entity(entity).remove::<ThinkerBuilder>();

            // Add state dead component
            debug!("Adding state dead component for {:?}", id.0);
            commands.entity(entity).insert(StateDead {
                dead_at: game_tick.0,
                killer: "Dehydration".to_string(),
            });

            // Set state to dead
            debug!("Setting state to dead");
            commands.trigger(StateChange {
                entity,
                new_state: State::Dead,
            });
        }
    }
}

fn starving_system(
    mut commands: Commands,
    game_tick: ResMut<GameTick>,
    mut map_events: ResMut<MapEvents>,
    presence: Res<PlayerWorldPresenceState>,
    mut starving_query: Query<(Entity, &Id, &PlayerId, &mut State, &Starving), Without<StateDead>>,
) {
    for (entity, id, player_id, mut state, starving) in starving_query.iter_mut() {
        if entity_belongs_to_protected_run(id, player_id, &presence) {
            continue;
        }
        if game_tick.0 - starving.at_tick > STARVING_DEATH_AT {
            debug!(
                "Starving: at tick: {:?} Adding state dead event for {:?}",
                game_tick.0, id.0
            );

            // Remove thinker
            debug!("Removing thinker for {:?}", id.0);
            commands.entity(entity).remove::<ThinkerBuilder>();

            // Add state dead component
            debug!("Adding state dead component for {:?}", id.0);
            commands.entity(entity).insert(StateDead {
                dead_at: game_tick.0,
                killer: "Starvation".to_string(),
            });

            // Set state to dead
            debug!("Setting state to dead");
            commands.trigger(StateChange {
                entity,
                new_state: State::Dead,
            });
        }
    }
}

fn exhausted_system(
    mut commands: Commands,
    game_tick: ResMut<GameTick>,
    mut map_events: ResMut<MapEvents>,
    presence: Res<PlayerWorldPresenceState>,
    mut exhausted_query: Query<
        (Entity, &Id, &PlayerId, &mut State, &Exhausted),
        Without<StateDead>,
    >,
) {
    for (entity, id, player_id, mut state, exhausted) in exhausted_query.iter_mut() {
        if entity_belongs_to_protected_run(id, player_id, &presence) {
            continue;
        }
        if game_tick.0 - exhausted.at_tick > EXHAUSTED_DEATH_AT {
            debug!(
                "Exhausted: at tick: {:?} Adding state dead event for {:?}",
                game_tick.0, id.0
            );

            // Remove thinker
            debug!("Removing thinker for {:?}", id.0);
            commands.entity(entity).remove::<ThinkerBuilder>();

            // Add state dead component
            debug!("Adding state dead component for {:?}", id.0);
            commands.entity(entity).insert(StateDead {
                dead_at: game_tick.0,
                killer: "Exhaustion".to_string(),
            });

            // Set state to dead
            debug!("Setting state to dead");
            commands.trigger(StateChange {
                entity,
                new_state: State::Dead,
            });
        }
    }
}

fn burning_system(
    mut commands: Commands,
    game_tick: ResMut<GameTick>,
    mut map_events: ResMut<MapEvents>,
    presence: Res<PlayerWorldPresenceState>,
    initial_encounter_state: Res<InitialEncounterState>,
    mut burning_query: Query<
        (
            Entity,
            &Id,
            &PlayerId,
            &mut State,
            &mut Stats,
            &Effects,
            Option<&LegendaryFollower>,
            Option<&LegendaryBoss>,
            Option<&SanctuaryHunter>,
            Option<&CrisisAssaultUnit>,
        ),
        Without<StateDead>,
    >,
) {
    if game_tick.0 % 10 == 0 {
        for (
            entity,
            id,
            player_id,
            mut state,
            mut stats,
            effects,
            legendary_follower,
            legendary_boss,
            sanctuary_hunter,
            crisis_assault,
        ) in burning_query.iter_mut()
        {
            if entity_belongs_to_protected_run(id, player_id, &presence)
                || initial_encounter_object_is_protected(id.0, &initial_encounter_state, &presence)
                || (crisis_assault.is_none()
                    && attributed_threat_owner(
                        legendary_follower,
                        legendary_boss,
                        sanctuary_hunter,
                    )
                    .map(|owner| is_player_offline_protected(owner, &presence))
                    .unwrap_or(false))
            {
                continue;
            }
            if effects.has(Effect::Burning) {
                stats.hp -= 1;
                commands
                    .entity(entity)
                    .try_insert(LastDamageTick(game_tick.0));

                if stats.hp <= 0 {
                    commands.entity(entity).insert(StateDead {
                        dead_at: game_tick.0,
                        killer: "Burns".to_string(),
                    });

                    // Set state to dead
                    debug!("Setting state to dead");
                    commands.trigger(StateChange {
                        entity,
                        new_state: State::Dead,
                    });
                }
            }
        }
    }
}

fn inventory_changed_system(
    clients: Res<Clients>,
    active_infos: Res<ActiveInfos>,
    templates: Res<Templates>,
    query: Query<(&Id, &Template, &Inventory), Changed<Inventory>>,
) {
    for (id, template, inventory) in query.iter() {
        info!("Inventory changed: {:?}", inventory);

        info!("Active infos: {:?}", active_infos);
        // Only single inventory, send InfoInventory instead of Snapshot
        if let Some(active_info_players) = active_infos.get(&(id.0, ActiveInfoType::Inventory)) {
            for player_id in active_info_players.iter() {
                let inventory_packet: ResponsePacket = ResponsePacket::InfoInventory {
                    id: id.0,
                    cap: Obj::get_capacity(&template.0, &templates.obj_templates),
                    tw: inventory.get_total_weight(),
                    items: inventory.get_packet(),
                };

                send_to_client(*player_id, inventory_packet, &clients);
            }
        }

        if let Some(active_info_players) = active_infos.get(&(id.0, ActiveInfoType::ItemTransfer)) {
            for player_id in active_info_players.iter() {
                let inventory_packet: ResponsePacket = ResponsePacket::InfoInventorySnapshot {
                    id: id.0,
                    cap: Obj::get_capacity(&template.0, &templates.obj_templates),
                    tw: inventory.get_total_weight(),
                    items: inventory.get_packet(),
                };

                send_to_client(*player_id, inventory_packet, &clients);
            }
        }
    }
}

fn skill_changed_system(query: Query<(&PlayerId, &Id, &mut Skills), Changed<Skills>>) {
    for (player_id, id, skills) in query.iter() {
        info!("Skills changed: {:?}", skills);
    }
}

fn effect_added_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    templates: Res<Templates>,
    presence: Res<PlayerWorldPresenceState>,
    mut query: Query<
        (Entity, &Id, &PlayerId, &mut Stats, &EffectAdded),
        Without<EffectAddedProcessed>,
    >,
) {
    for (entity, id, player_id, mut stats, effect_added) in query.iter_mut() {
        if entity_belongs_to_protected_run(id, player_id, &presence) {
            continue;
        }
        info!("Effect added: {:?}", player_id.0);
        let effect = effect_added.effect.clone();

        let effect_template = templates
            .effect_templates
            .get(&effect.to_str())
            .expect("Effect missing from templates");

        if let Some(health_modifier) = effect_template.health {
            let previous_hp = stats.hp;
            let modifier = 1.0 + health_modifier;
            stats.hp = (stats.hp as f32 * modifier) as i32;
            if stats.hp < previous_hp {
                commands
                    .entity(entity)
                    .try_insert(LastDamageTick(game_tick.0));
            }
        }

        if let Some(stamina_modifier) = effect_template.stamina {
            let modifier = 1.0 + stamina_modifier;
            stats.stamina = Some((stats.stamina.unwrap() as f32 * modifier) as i32);
        }

        commands.entity(entity).try_insert(EffectAddedProcessed);
    }
}

pub fn item_duration_system(
    mut commands: Commands,
    game_tick: ResMut<GameTick>,
    clients: Res<Clients>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut map_events: ResMut<MapEvents>,
    active_infos: Res<ActiveInfos>,
    entity_map: Res<EntityObjMap>,
    mut query: Query<(&PlayerId, &Id, &mut Inventory)>,
) {
    if game_tick.0 % TICKS_PER_SEC == 0 {
        for (player_id, id, mut inventory) in query.iter_mut() {
            if entity_belongs_to_protected_run(id, player_id, &presence) {
                continue;
            }
            let expired_items = inventory.find_expired_items(game_tick.0);

            for item in expired_items {
                info!("Removing expired item: {:?}", item);
                // Remove item
                inventory.remove_item(item.id);

                if item.class == TORCH {
                    if let Some(owner_entity) = entity_map.get_entity(item.owner) {
                        commands.trigger(UpdateObj {
                            entity: owner_entity,
                            attrs: vec![(VISION.to_string(), "Pending".to_string())],
                        });
                    } else {
                        error!("Cannot find entity for obj id: {:?}", item.owner);
                    }

                    // Get player id from obj id
                    let Some(player_id) = ids.get_player(item.owner) else {
                        error!("Cannot find player from obj id: {:?}", item.owner);
                        continue;
                    };

                    // Check if the target is actively being observed
                    /*let active_info_key = (player_id, item.owner, "inventory".to_string());

                    if let Some(_active_info) = active_infos.get(&active_info_key) {
                        let item_update_packet: ResponsePacket = ResponsePacket::InfoItemsUpdate {
                            id: item.owner,
                            items_updated: vec![],
                            items_removed: vec![item.id],
                        };

                        send_to_client(player_id, item_update_packet, &clients);
                    }*/
                }
            }
        }
    }
}

pub fn fuel_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: ResMut<GameTick>,
    presence: Res<PlayerWorldPresenceState>,
    mut ids: ResMut<Ids>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    active_infos: Res<ActiveInfos>,
    mut obj_query: Query<
        (
            Entity,
            &PlayerId,
            &Id,
            &Position,
            &Class,
            &Template,
            &mut Inventory,
        ),
        With<Campfire>,
    >,
) {
    if game_tick.0 % (TICKS_PER_SEC * 10) == 0 {
        for (entity, player_id, id, pos, class, template, mut inventory) in obj_query.iter_mut() {
            if is_owner_offline_protected(player_id, &presence) {
                continue;
            }
            info!("Fueling campfire with fuel");

            // Remove wood from tent
            let Some(fuel_item) = inventory.get_by_class(item::FUEL.to_string()) else {
                info!("No fuel found in tent, unlit campfire");

                // Remove campfire light effect
                commands.trigger(RemoveLightEffect {
                    entity: entity,
                    effect: Effect::CampfireLight,
                });

                // Swap image back to non-lit tent
                let structure_image = Obj::template_to_image(&template.0.clone());

                // Structure State Change Event to Lit
                commands.trigger(UpdateObj {
                    entity: entity,
                    attrs: vec![(IMAGE.to_string(), structure_image)],
                });

                // Remove campfire component from entity
                commands.entity(entity).remove::<Campfire>();
                continue;
            };

            // Charcoal burns 5x longer than Firewood: only consume one Charcoal
            // unit every 50s (5 fuel-system cycles) instead of every 10s.
            let consume_this_tick = if fuel_item.subclass == item::CHARCOAL {
                game_tick.0 % (TICKS_PER_SEC * 50) == 0
            } else {
                true
            };

            let updated_item = if consume_this_tick {
                info!("Removing 1 fuel from tent");
                inventory.remove_quantity(fuel_item.id, 1)
            } else {
                Some(fuel_item.clone())
            };

            /*let active_info_key = (player_id.0, id.0, "inventory".to_string());

            if let Some(_active_info) = active_infos.get(&active_info_key) {
                let item_update_packet: ResponsePacket = if let Some(updated_item) = updated_item {
                    ResponsePacket::InfoItemsUpdate {
                        id: id.0,
                        items_updated: vec![Item::to_packet(updated_item)],
                        items_removed: Vec::new(),
                    }
                } else {
                    ResponsePacket::InfoItemsUpdate {
                        id: id.0,
                        items_updated: Vec::new(),
                        items_removed: vec![fuel_item.id],
                    }
                };

                send_to_client(player_id.0, item_update_packet, &clients);
            }*/
        }
    }
}

pub fn work_queue_update_system(
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    game_events: Res<GameEvents>,
    recipes: Res<Recipes>,
    templates: Res<Templates>,
    active_infos: Res<ActiveInfos>,
    query: Query<(&PlayerId, &Id, &Inventory, &WorkQueue), Changed<WorkQueue>>,
) {
    for (player_id, structure_id, inventory, work_queue) in query.iter() {
        info!("Work queue entries changed...: {:?}", work_queue.0);

        if let Some(_active_info) =
            active_infos.get(&(structure_id.0, ActiveInfoType::StructureQueue))
        {
            let mut work_queue_packet = Vec::new();

            for work_entry in work_queue.0.iter() {
                let mut work_time = -1;
                let mut progress = 0;

                // Get progress of work entry
                if work_entry.work_type == WorkType::Craft {
                    if let Some(crafting_event) = game_events.get_craft_event(work_entry.worker_id)
                    {
                        let recipe = recipes
                            .get_by_name(crafting_event.recipe_name.clone())
                            .expect(&format!(
                                "Cannot find recipe for {:?}",
                                crafting_event.recipe_name
                            ));

                        progress = (game_tick.0 - crafting_event.start_tick) / TICKS_PER_SEC;
                        work_time = recipe.crafting_time.unwrap_or(100) / TICKS_PER_SEC;
                    } else if let Some(structure_craft_event) =
                        game_events.get_structure_craft_event(work_entry.worker_id)
                    {
                        let recipe = recipes
                            .get_by_name(structure_craft_event.recipe_name.clone())
                            .expect(&format!(
                                "Cannot find recipe for {:?}",
                                structure_craft_event.recipe_name
                            ));

                        progress = (game_tick.0 - structure_craft_event.start_tick) / TICKS_PER_SEC;
                        work_time = recipe.crafting_time.unwrap_or(100) / TICKS_PER_SEC;
                    }
                } else if work_entry.work_type == WorkType::Refine {
                    if let Some(refine_event) = game_events.get_refine_event(work_entry.worker_id) {
                        let Some(item) = inventory.get_by_id(refine_event.item_id) else {
                            error!("Cannot find item for {:?}", refine_event.item_id);
                            continue;
                        };

                        let item_template =
                            Item::get_template(item.name.clone(), &templates.item_templates);

                        work_time = item_template.get_refine_time() / TICKS_PER_SEC;
                        progress = (game_tick.0 - refine_event.start_tick) / TICKS_PER_SEC;
                    } else if let Some(structure_refine_event) =
                        game_events.get_structure_refine_event(work_entry.worker_id)
                    {
                        let Some(item) = inventory.get_by_id(structure_refine_event.item_id) else {
                            error!("Cannot find item for {:?}", structure_refine_event.item_id);
                            continue;
                        };

                        let item_template =
                            Item::get_template(item.name.clone(), &templates.item_templates);

                        work_time = item_template.get_refine_time() / TICKS_PER_SEC;
                        progress =
                            (game_tick.0 - structure_refine_event.start_tick) / TICKS_PER_SEC;
                    }
                } else if work_entry.work_type == WorkType::Operate {
                    if let Some(operate_event) =
                        game_events.get_structure_operate_event(work_entry.worker_id)
                    {
                        progress = (game_tick.0 - operate_event.start_tick) / TICKS_PER_SEC;
                        work_time = 200 / TICKS_PER_SEC;
                    }
                }

                work_queue_packet.push(network::WorkEntry {
                    work_type: work_entry.work_type.to_string(),
                    work_status: work_entry.work_status.to_string(),
                    villager_id: work_entry.worker_id,
                    recipe_name: work_entry.recipe_name.clone(),
                    recipe_image: work_entry.recipe_image.clone(),
                    refine_item_class: work_entry.refine_item_class.clone(),
                    refine_item_id: work_entry.refine_item_id.clone(),
                    refine_item_image: work_entry.refine_item_image.clone(),
                    work_time: work_time,
                    progress: progress,
                });
            }

            info!(
                "(Changed)Sending work queue packet: {:?}",
                work_queue_packet
            );

            let packet = ResponsePacket::InfoStructureQueue {
                structure_id: structure_id.0,
                queue: work_queue_packet,
            };

            send_to_client(player_id.0, packet, &clients);
        }
    }
}

fn dedup<T: Eq + Hash + Copy>(v: &mut Vec<T>) {
    // note the Copy constraint
    let mut uniques = HashSet::new();
    v.retain(|e| uniques.insert(*e));
}

pub fn is_pos_empty(player_id: i32, x: i32, y: i32, query: &Query<ObjQuery>) -> bool {
    let mut objs = Vec::new();

    for q in query {
        if player_id != q.player_id.0 && x == q.pos.x && y == q.pos.y && q.state.is_blocking() {
            objs.push(q.entity);
        }
    }

    return objs.len() == 0;
}

fn resolve_necromancer_spawn_pos(
    spawn_anchor: Position,
    occupied_positions: &HashSet<Position>,
    map: &Map,
    search_radius: i32,
) -> Option<Position> {
    let mut candidates = vec![(spawn_anchor.x, spawn_anchor.y)];

    for radius in 1..=search_radius {
        candidates.extend(Map::ring((spawn_anchor.x, spawn_anchor.y), radius));
    }

    candidates.sort_by_key(|(x, y)| (Map::dist(spawn_anchor, Position { x: *x, y: *y }), *y, *x));
    candidates.dedup();

    candidates
        .into_iter()
        .map(|(x, y)| Position { x, y })
        .find(|pos| is_necromancer_spawn_tile_open(*pos, occupied_positions, map))
}

fn is_necromancer_spawn_tile_open(
    pos: Position,
    occupied_positions: &HashSet<Position>,
    map: &Map,
) -> bool {
    Map::is_valid_pos((pos.x, pos.y))
        && Map::is_passable(pos.x, pos.y, map)
        && !occupied_positions.contains(&pos)
}

/*impl GameEvents {
    pub fn new(&mut self, event_id: i32, run_tick: i32, game_event_type: GameEventType) {
        let game_event = GameEvent {
            event_id: event_id,
            run_tick: run_tick,
            game_event_type: game_event_type,
        };

        //self.insert(map_event_id, map_state_event);
        self.insert(event_id, game_event);
    }
}*/

fn get_random_adjacent_pos(
    player_id: i32,
    center_x: i32,
    center_y: i32,
    all_obj_pos: Vec<network::MapObj>,
    map: &Map,
) -> Option<Position> {
    let mut selected_pos;

    // Check for a valid stop within 2 tiles
    let mut neighbours = Map::range((center_x, center_y), 2);
    selected_pos = find_valid_pos(neighbours, player_id, &all_obj_pos, map);

    // If none found, check for a valid spot on the 3rd and 4th ring
    if selected_pos.is_none() {
        neighbours = Map::ring((center_x, center_y), 3);
        selected_pos = find_valid_pos(neighbours, player_id, &all_obj_pos, map);

        if selected_pos.is_none() {
            neighbours = Map::ring((center_x, center_y), 4);
            selected_pos = find_valid_pos(neighbours, player_id, &all_obj_pos, map);
        }
    }

    debug!("Selected Pos (before fallback): {:?}", selected_pos);

    // If no valid tile can be selected return center x,y
    if selected_pos.is_none() {
        selected_pos = Some(Position {
            x: center_x,
            y: center_y,
        });
    }

    return selected_pos;
}

// Spawn position for a timed/event crisis: a passable, reachable tile just beyond
// the nearest sanctuary's outer ring, so the wave appears out in the wilderness and
// marches inward toward the settlement (the NPCs' own AI handles the approach). When
// no sanctuary is near `fallback` (the hero), it reverts to the old ring around the
// hero. This is what makes crises "ignore the sanctuary, spawn outside, and move in."
fn crisis_spawn_pos(
    player_id: i32,
    sanctuary_zones: &SanctuaryZones,
    fallback: Position,
    map: &Map,
) -> Option<Position> {
    let Some(zone) = sanctuary_zones.nearest(fallback) else {
        return get_random_pos_at_range(player_id, fallback.x, fallback.y, 6, Vec::new(), map);
    };

    let mut rng = rand::thread_rng();
    for _ in 0..16 {
        let ring_r = zone.weak_radius() as i32 + 1 + rng.gen_range(0..3);
        let ring = Map::ring((zone.pos.x, zone.pos.y), ring_r);
        if ring.is_empty() {
            continue;
        }
        let (x, y) = ring[rng.gen_range(0..ring.len())];
        if !Map::is_valid_pos((x, y)) || !Map::is_passable(x, y, map) {
            continue;
        }
        let pos = Position { x, y };
        // Must be able to march in to the settlement.
        if Map::find_path(
            pos,
            zone.pos,
            map,
            player_id,
            Vec::new(),
            true,
            false,
            false,
            true,
            true,
        )
        .is_some()
        {
            return Some(pos);
        }
    }
    None
}

fn get_random_pos_at_range(
    player_id: i32,
    center_x: i32,
    center_y: i32,
    range: i32,
    all_obj_pos: Vec<network::MapObj>,
    map: &Map,
) -> Option<Position> {
    let mut selected_pos;

    // Check for a valid stop within 2 tiles
    let mut neighbours = Map::ring((center_x, center_y), range);
    selected_pos = find_valid_pos(neighbours, player_id, &all_obj_pos, map);

    // If none found, check for a valid spot on the 3rd and 4th ring
    if selected_pos.is_none() {
        neighbours = Map::ring((center_x, center_y), range + 1);
        selected_pos = find_valid_pos(neighbours, player_id, &all_obj_pos, map);
    }

    debug!("Selected Pos (before fallback): {:?}", selected_pos);

    // If no valid tile can be selected return center x,y
    if selected_pos.is_none() {
        selected_pos = Some(Position {
            x: center_x,
            y: center_y,
        });
    }

    return selected_pos;
}

fn find_valid_pos(
    neighbours: Vec<(i32, i32)>,
    player_id: i32,
    all_obj_pos: &Vec<network::MapObj>,
    map: &Map,
) -> Option<Position> {
    let valid_neighbours: Vec<(i32, i32)> = neighbours
        .into_iter()
        .filter(|(x, y)| is_valid_pos(*x, *y, player_id, all_obj_pos, map))
        .collect();

    if valid_neighbours.len() > 0 {
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..valid_neighbours.len());
        trace!("Random valid pos index: {:?}", index);
        let (pos_x, pos_y) = valid_neighbours[index];

        return Some(Position { x: pos_x, y: pos_y });
    } else {
        return None;
    }
}

fn is_valid_pos(
    x: i32,
    y: i32,
    player_id: i32,
    all_obj_pos: &Vec<network::MapObj>,
    map: &Map,
) -> bool {
    let is_passable = Map::is_passable(x, y, &map);
    let is_valid_pos = Map::is_valid_pos((x, y));
    let is_not_blocked = is_not_blocked(player_id, x, y, &all_obj_pos);
    trace!("is_not_blocked: {:?}", is_not_blocked);

    if is_passable && is_valid_pos && is_not_blocked {
        return true;
    }

    return false;
}

fn is_not_blocked(player_id: i32, x: i32, y: i32, all_obj_pos: &Vec<network::MapObj>) -> bool {
    trace!(
        "is_not_blocked: {:?} {:?} {:?} {:?}",
        player_id,
        x,
        y,
        all_obj_pos
    );
    // TODO reconsider if player id should be compared
    for obj in all_obj_pos.iter() {
        if x == obj.x && y == obj.y && player_id != obj.player {
            // found blocking obj
            return false;
        }
    }

    return true;
}

fn soulshard_res_cost(deaths: u32, total_xp: i32) -> i32 {
    const XP_SCALE: f64 = 1000.0;
    const BASE_COST: f64 = 10.0;
    const DEATH_FACTOR: f64 = 1.20;

    let xp_norm = (total_xp as f64) / XP_SCALE;
    let base_from_xp = BASE_COST * (1.0 + (1.0 + xp_norm).ln());
    let death_mult = DEATH_FACTOR.powf(deaths as f64);

    let cost = (base_from_xp * death_mult).ceil() as i32;

    cost
}

#[cfg(test)]
#[path = "game_tests.rs"]
mod tests;
