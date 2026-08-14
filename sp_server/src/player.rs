use bevy::ecs::query::{QueryData, WorldQuery};
use bevy::prelude::*;
use big_brain::thinker::ThinkerBuilder;
use rand::Rng;
use serde::Deserialize;
use std::env;
use std::fs;
use std::sync::{Arc, Mutex};
use tracing_subscriber::{reload, EnvFilter, Registry};
use uuid::Uuid;

use std::collections::{HashMap, HashSet, VecDeque};

use crate::common::{Destination, Heat, Hunger, Idle, Thirst, Tired, Transport};
use crate::constants::*;
use crate::crisis_balance::{
    is_live_built_human_core_structure, CrisisAttackTelemetryStage, CrisisBalanceObservationState,
    CrisisBalanceTelemetryState,
};
use crate::encounter::Encounter;
use crate::event::{
    EventCompleted, GameEvent, GameEventType, GameEvents, MapEvents, Spell, VisibleEvent,
    VisibleEvents,
};
use crate::farm::Crops;
use crate::ids::{EntityObjMap, Ids};

use crate::combat::{AttackOptions, Combat, CombatEffectsChanged, CombatQuery, CombatQueryItem};
use crate::effect::{ControlEffectDiminishingReturns, Effect, Effects};
use crate::experiment::{self, Experiment, ExperimentState, Experiments};
use crate::game::{
    burrow_supply_type_count, farm_harvest_duration_ticks, is_loot_poi, is_pos_empty,
    sanctuary_radius, sanctuary_upgrade_cost, survey_status_for_tile, BoundMonolith,
    CampfireVisibilityState, Clients, CrisisAssaultUnit, CrisisKind, CrisisPhase, DamageRecord,
    DebugObjs, EventInProgress, GameTick, InitialEncounterState, IntroEncounterState,
    InvestigatedPOIs, LogLevelOverrides, Merchant, Monolith, MonolithInvestigation,
    MonolithProgress, NetworkReceiver, ObjQuery, Objectives, PersonalCrisisHistory,
    PlayerIntroState, PlayerObjectives, PlayerRunScore, PlayerStat, PlayerStats, RunScoreState,
    SettlementCrisisState, SpawnPositions, SurveyHistory, BANDAGE_USE_TICKS, BURROW_SUPPLY_GOAL,
    SANCTUARY_MAX_LEVEL,
};
use crate::item::{self, AttrKey, AttrVal, Inventory, Item};
use crate::map::Map;
use crate::network::{
    self, send_to_client, CraftingItem, RefiningItem, ResponsePacket, StatsData, StructureList,
};
use crate::obj::{
    is_combat_locked, ActionProgress, ActiveTask, Assignment, Assignments, BaseAttrs, BaseQuery,
    BuildProgressUpdate, BuildUpgradeState, Campfire, CancelEvents, Class, ClassStructure,
    DroppedBag, EndRepeatAction, HeroClass, HeroClassProfile, Id, LastCombatTick, LastDamageTick,
    Misc, Name, NewObj, Obj, Order, Personality, PlayerId, Portrait, Position, RemoveObj,
    SelectedUpgrade, Shelter, StartBuild, StartUpgrade, State, StateBuilding, StateChange,
    StateDead, Stats, Storage, Subclass, SubclassHero, SubclassVillager, Template, TrueDeath,
    UpdateObj, Viewshed, WorkEntry, WorkQueue, WorkStatus, WorkType,
};
use crate::player_setup::{AssignedStartLocations, RunSpawnedObjs, StartLocations};
use crate::recipe::Recipes;
use crate::resource::{Resource, ResourceDiscoveries, Resources};
use crate::safe_logout::{
    initialize_player_presence, is_owner_offline_protected, is_player_offline_protected,
    mark_player_logged_in, object_belongs_to_protected_run, record_player_combat_activity,
    CancelSafeLogout, PlayerWorldPresenceState, RequestSafeLogout, SafeLogoutTelemetryState,
};
use crate::skill::{SkillData, Skills, MAX_RANK};
use crate::skill_defs::Skill;
use crate::structure::{self, Plans, Structure, WALL};
use crate::templates::{self, ObjTemplate, ResReq, Templates};
use crate::terrain_feature::{TerrainFeature, TerrainFeatures};
use crate::trade::{Prices, WantedItem};
use crate::villager::{villager_activity_text, BlockedWork, ToolFetchTarget};
use crate::villager_util::{self, VillagerUtil};
use crate::world::time_of_day_vision_mod;
use crate::{player_setup, AppState};

#[derive(Resource, Deref, DerefMut)]
pub struct Player(pub HashMap<i32, PlayerEvent>);

#[derive(Resource, Deref, DerefMut)]
pub struct PlayerEvents(pub HashMap<i32, PlayerEvent>);

#[derive(EntityEvent)]
pub struct InfoHeroEvent {
    pub entity: Entity,
    pub player_id: i32,
}

#[derive(EntityEvent)]
pub struct InfoVillagerEvent {
    pub entity: Entity,
    pub player_id: i32,
}

#[derive(EntityEvent)]
pub struct InfoStructureEvent {
    pub entity: Entity,
    pub player_id: i32,
}

#[derive(EntityEvent)]
pub struct InfoMonolithEvent {
    pub entity: Entity,
    pub player_id: i32,
}

#[derive(EntityEvent)]
pub struct InfoPOIEvent {
    pub entity: Entity,
    pub player_id: i32,
}

#[derive(EntityEvent)]
pub struct InfoNPCEvent {
    pub entity: Entity,
    pub player_id: i32,
}

#[derive(Resource, Clone, Debug, Deserialize)]
pub enum PlayerEvent {
    NewPlayer {
        player_id: i32,
        hero_name: String,
        class_name: String,
        portrait: String,
    },
    Login {
        player_id: i32,
        connection_id: Uuid,
    },
    RequestSafeLogout {
        player_id: i32,
        connection_id: Uuid,
    },
    CancelSafeLogout {
        player_id: i32,
        connection_id: Uuid,
    },
    Move {
        player_id: i32,
        x: i32,
        y: i32,
    },
    Attack {
        player_id: i32,
        attack_type: String,
        source_id: i32,
        target_id: i32,
    },
    Ability {
        player_id: i32,
        ability_id: String,
        source_id: i32,
        target_id: Option<i32>,
    },
    Combo {
        player_id: i32,
        source_id: i32,
        target_id: i32,
        combo_type: String,
    },
    Block {
        player_id: i32,
        source_id: i32,
        defense: String,
    },
    Gather {
        player_id: i32,
    },
    Operate {
        player_id: i32,
        structure_id: i32,
    },
    Plant {
        player_id: i32,
        structure_id: i32,
    },
    Tend {
        player_id: i32,
        structure_id: i32,
    },
    Harvest {
        player_id: i32,
        structure_id: i32,
    },
    Refine {
        player_id: i32,
        item_id: i32,
    },
    Craft {
        player_id: i32,
        recipe_name: String,
        signature_item_id: Option<i32>,
    },
    StructureRefine {
        player_id: i32,
        structure_id: i32,
        item_id: i32,
    },
    StructureCraft {
        player_id: i32,
        structure_id: i32,
        recipe_name: String,
        signature_item_id: Option<i32>,
    },
    GetStats {
        player_id: i32,
        id: i32,
    },
    InfoObj {
        player_id: i32,
        id: i32,
    },
    InfoSkills {
        player_id: i32,
        id: i32,
    },
    InfoAttrs {
        player_id: i32,
        id: i32,
    },
    InfoAdvance {
        player_id: i32,
        id: i32,
    },
    InfoUpgrade {
        player_id: i32,
        structure_id: i32,
    },
    InfoTile {
        player_id: i32,
        x: i32,
        y: i32,
    },
    InfoTileResources {
        player_id: i32,
        x: i32,
        y: i32,
    },
    InvestigatePOI {
        player_id: i32,
        target_id: i32,
    },
    InfoInventory {
        player_id: i32,
        id: i32,
    },
    InfoEquip {
        player_id: i32,
        id: i32,
    },
    InfoItem {
        player_id: i32,
        obj_id: i32,
        item_id: i32,
        action: String,
    },
    InfoItemByName {
        player_id: i32,
        name: String,
    },
    InfoItemTransfer {
        player_id: i32,
        source_id: i32,
        target_id: i32,
    },
    InfoExit {
        player_id: i32,
        id: i32,
        panel_type: String,
    },
    InfoMerchant {
        player_id: i32,
        source_id: i32,
        merchant_id: i32,
    },
    InfoHire {
        player_id: i32,
        source_id: i32,
    },
    ItemTransfer {
        player_id: i32,
        item_id: i32,
        source_id: i32,
        target_id: i32,
    },
    LootAll {
        player_id: i32,
        source_id: i32,
        target_id: i32,
    },
    DropItem {
        player_id: i32,
        item_id: i32,
    },
    ItemSplit {
        player_id: i32,
        owner_id: i32,
        item_id: i32,
        quantity: i32,
    },
    OrderFollow {
        player_id: i32,
        source_id: i32,
    },
    OrderGather {
        player_id: i32,
        source_id: i32,
        res_type: String,
    },
    OrderOperate {
        player_id: i32,
        villager_id: i32,
        structure_id: i32,
    },
    OrderRefine {
        player_id: i32,
        villager_id: i32,
        structure_id: i32,
    },
    OrderCraft {
        player_id: i32,
        villager_id: i32,
        structure_id: i32,
    },
    OrderExplore {
        player_id: i32,
        villager_id: i32,
    },
    OrderProspect {
        player_id: i32,
        villager_id: i32,
    },
    OrderExperiment {
        player_id: i32,
        villager_id: i32,
        structure_id: i32,
    },
    OrderPlant {
        player_id: i32,
        villager_id: i32,
        structure_id: i32,
    },
    OrderTend {
        player_id: i32,
        villager_id: i32,
        structure_id: i32,
    },
    OrderHarvest {
        player_id: i32,
        villager_id: i32,
        structure_id: i32,
    },
    OrderRepair {
        player_id: i32,
        villager_id: i32,
    },
    StructureList {
        player_id: i32,
    },
    CreateFoundation {
        player_id: i32,
        source_id: i32,
        structure_name: String,
    },
    Build {
        player_id: i32,
        builder_id: i32,
        structure_id: i32,
    },
    Sleep {
        player_id: i32,
        structure_id: i32,
    },
    StartUpgrade {
        player_id: i32,
        structure_id: i32,
        selected_upgrade: String,
    },
    Upgrade {
        player_id: i32,
        builder_id: i32,
        structure_id: i32,
    },
    Experiment {
        player_id: i32,
        structure_id: i32,
    },
    Activate {
        player_id: i32,
        structure_id: i32,
    },
    Survey {
        player_id: i32,
        source_id: i32,
    },
    Prospect {
        player_id: i32,
    },
    Explore {
        player_id: i32,
    },
    NearbyResources {
        player_id: i32,
    },
    Assign {
        player_id: i32,
        worker_id: i32,
        structure_id: i32,
    },
    RemoveAssign {
        player_id: i32,
        worker_id: i32,
        structure_id: i32,
    },
    Equip {
        player_id: i32,
        obj_id: i32,
        item_id: i32,
        status: bool,
    },
    DeleteItem {
        player_id: i32,
        obj_id: i32,
        item_id: i32,
    },
    InfoAssign {
        player_id: i32,
        structure_id: i32,
    },
    InfoCraft {
        player_id: i32,
        crafter_id: i32,
    },
    InfoStructureCraft {
        player_id: i32,
        structure_id: i32,
    },
    InfoStructureQueue {
        player_id: i32,
        structure_id: i32,
    },
    InfoWorkQueueEntry {
        player_id: i32,
        structure_id: i32,
        index: i32,
    },
    AddCraftingEntry {
        player_id: i32,
        structure_id: i32,
        recipe_name: String,
    },
    AddRefineEntry {
        player_id: i32,
        structure_id: i32,
        refine_item_id: i32,
    },
    RemoveWorkEntry {
        player_id: i32,
        structure_id: i32,
        index: i32,
    },
    InfoRefine {
        player_id: i32,
        refiner_id: i32,
    },
    InfoStructureRefine {
        player_id: i32,
        structure_id: i32,
    },
    InfoStructureRefineItem {
        player_id: i32,
        structure_id: i32,
        item_id: i32,
    },
    Use {
        player_id: i32,
        obj_id: i32,
        item_id: i32,
    },
    Remove {
        player_id: i32,
        structure_id: i32,
    },
    Advance {
        player_id: i32,
        id: i32,
    },
    InfoExperinment {
        player_id: i32,
        structure_id: i32,
    },
    SetExperimentItem {
        player_id: i32,
        structure_id: i32,
        item_id: i32,
        is_resource: bool, //assume is source if not resource
    },
    ResetExperiment {
        player_id: i32,
        structure_id: i32,
    },
    Hire {
        player_id: i32,
        merchant_id: i32,
        target_id: i32,
    },
    UpgradeSanctuary {
        player_id: i32,
        monolith_id: i32,
    },
    BuyItem {
        player_id: i32,
        seller_id: i32,
        item_id: i32,
        quantity: i32,
    },
    SellItem {
        player_id: i32,
        item_id: i32,
        target_id: i32,
        quantity: i32,
    },
    CancelAction {
        player_id: i32,
    },
    DebugObj {
        player_id: i32,
        obj_id: i32,
    },
    SetLogLevel {
        player_id: i32,
        target: String,
        level: String,
    },
    GetLogLevels {
        player_id: i32,
    },
}

impl PlayerEvent {
    fn player_id(&self) -> i32 {
        match self {
            Self::NewPlayer { player_id, .. }
            | Self::Login { player_id, .. }
            | Self::RequestSafeLogout { player_id, .. }
            | Self::CancelSafeLogout { player_id, .. }
            | Self::Move { player_id, .. }
            | Self::Attack { player_id, .. }
            | Self::Ability { player_id, .. }
            | Self::Combo { player_id, .. }
            | Self::Block { player_id, .. }
            | Self::Gather { player_id }
            | Self::Operate { player_id, .. }
            | Self::Plant { player_id, .. }
            | Self::Tend { player_id, .. }
            | Self::Harvest { player_id, .. }
            | Self::Refine { player_id, .. }
            | Self::Craft { player_id, .. }
            | Self::StructureRefine { player_id, .. }
            | Self::StructureCraft { player_id, .. }
            | Self::GetStats { player_id, .. }
            | Self::InfoObj { player_id, .. }
            | Self::InfoSkills { player_id, .. }
            | Self::InfoAttrs { player_id, .. }
            | Self::InfoAdvance { player_id, .. }
            | Self::InfoUpgrade { player_id, .. }
            | Self::InfoTile { player_id, .. }
            | Self::InfoTileResources { player_id, .. }
            | Self::InvestigatePOI { player_id, .. }
            | Self::InfoInventory { player_id, .. }
            | Self::InfoEquip { player_id, .. }
            | Self::InfoItem { player_id, .. }
            | Self::InfoItemByName { player_id, .. }
            | Self::InfoItemTransfer { player_id, .. }
            | Self::InfoExit { player_id, .. }
            | Self::InfoMerchant { player_id, .. }
            | Self::InfoHire { player_id, .. }
            | Self::ItemTransfer { player_id, .. }
            | Self::LootAll { player_id, .. }
            | Self::DropItem { player_id, .. }
            | Self::ItemSplit { player_id, .. }
            | Self::OrderFollow { player_id, .. }
            | Self::OrderGather { player_id, .. }
            | Self::OrderOperate { player_id, .. }
            | Self::OrderRefine { player_id, .. }
            | Self::OrderCraft { player_id, .. }
            | Self::OrderExplore { player_id, .. }
            | Self::OrderProspect { player_id, .. }
            | Self::OrderExperiment { player_id, .. }
            | Self::OrderPlant { player_id, .. }
            | Self::OrderTend { player_id, .. }
            | Self::OrderHarvest { player_id, .. }
            | Self::OrderRepair { player_id, .. }
            | Self::StructureList { player_id }
            | Self::CreateFoundation { player_id, .. }
            | Self::Build { player_id, .. }
            | Self::Sleep { player_id, .. }
            | Self::StartUpgrade { player_id, .. }
            | Self::Upgrade { player_id, .. }
            | Self::Experiment { player_id, .. }
            | Self::Activate { player_id, .. }
            | Self::Survey { player_id, .. }
            | Self::Prospect { player_id }
            | Self::Explore { player_id }
            | Self::NearbyResources { player_id }
            | Self::Assign { player_id, .. }
            | Self::RemoveAssign { player_id, .. }
            | Self::Equip { player_id, .. }
            | Self::DeleteItem { player_id, .. }
            | Self::InfoAssign { player_id, .. }
            | Self::InfoCraft { player_id, .. }
            | Self::InfoStructureCraft { player_id, .. }
            | Self::InfoStructureQueue { player_id, .. }
            | Self::InfoWorkQueueEntry { player_id, .. }
            | Self::AddCraftingEntry { player_id, .. }
            | Self::AddRefineEntry { player_id, .. }
            | Self::RemoveWorkEntry { player_id, .. }
            | Self::InfoRefine { player_id, .. }
            | Self::InfoStructureRefine { player_id, .. }
            | Self::InfoStructureRefineItem { player_id, .. }
            | Self::Use { player_id, .. }
            | Self::Remove { player_id, .. }
            | Self::Advance { player_id, .. }
            | Self::InfoExperinment { player_id, .. }
            | Self::SetExperimentItem { player_id, .. }
            | Self::ResetExperiment { player_id, .. }
            | Self::Hire { player_id, .. }
            | Self::UpgradeSanctuary { player_id, .. }
            | Self::BuyItem { player_id, .. }
            | Self::SellItem { player_id, .. }
            | Self::CancelAction { player_id }
            | Self::DebugObj { player_id, .. }
            | Self::SetLogLevel { player_id, .. }
            | Self::GetLogLevels { player_id } => *player_id,
        }
    }

    /// Commands that can change gameplay state. Run creation and login remain
    /// outside this gate because their existing handlers own stale-run and
    /// reconnect validation respectively.
    fn is_mutating_gameplay(&self) -> bool {
        !matches!(
            self,
            Self::NewPlayer { .. }
                | Self::Login { .. }
                | Self::RequestSafeLogout { .. }
                | Self::CancelSafeLogout { .. }
                | Self::GetStats { .. }
                | Self::InfoObj { .. }
                | Self::InfoSkills { .. }
                | Self::InfoAttrs { .. }
                | Self::InfoAdvance { .. }
                | Self::InfoUpgrade { .. }
                | Self::InfoTile { .. }
                | Self::InfoTileResources { .. }
                | Self::InfoInventory { .. }
                | Self::InfoEquip { .. }
                | Self::InfoItem { .. }
                | Self::InfoItemByName { .. }
                | Self::InfoItemTransfer { .. }
                | Self::InfoExit { .. }
                | Self::InfoMerchant { .. }
                | Self::InfoHire { .. }
                | Self::StructureList { .. }
                | Self::NearbyResources { .. }
                | Self::InfoAssign { .. }
                | Self::InfoCraft { .. }
                | Self::InfoStructureCraft { .. }
                | Self::InfoStructureQueue { .. }
                | Self::InfoWorkQueueEntry { .. }
                | Self::InfoRefine { .. }
                | Self::InfoStructureRefine { .. }
                | Self::InfoStructureRefineItem { .. }
                | Self::InfoExperinment { .. }
                | Self::DebugObj { .. }
                | Self::SetLogLevel { .. }
                | Self::GetLogLevels { .. }
        )
    }

    fn targets_protected_run(&self, ids: &Ids, presence: &PlayerWorldPresenceState) -> bool {
        let protected = |obj_id: i32| object_belongs_to_protected_run(obj_id, ids, presence);

        match self {
            Self::Attack {
                source_id,
                target_id,
                ..
            }
            | Self::Combo {
                source_id,
                target_id,
                ..
            } => protected(*source_id) || protected(*target_id),
            Self::Ability {
                source_id,
                target_id,
                ..
            } => protected(*source_id) || target_id.map(protected).unwrap_or(false),
            Self::Block { source_id, .. }
            | Self::OrderFollow { source_id, .. }
            | Self::OrderGather { source_id, .. }
            | Self::CreateFoundation { source_id, .. }
            | Self::Survey { source_id, .. } => protected(*source_id),
            Self::Operate { structure_id, .. }
            | Self::Plant { structure_id, .. }
            | Self::Tend { structure_id, .. }
            | Self::Harvest { structure_id, .. }
            | Self::StructureRefine { structure_id, .. }
            | Self::StructureCraft { structure_id, .. }
            | Self::Sleep { structure_id, .. }
            | Self::StartUpgrade { structure_id, .. }
            | Self::Experiment { structure_id, .. }
            | Self::Activate { structure_id, .. }
            | Self::AddCraftingEntry { structure_id, .. }
            | Self::AddRefineEntry { structure_id, .. }
            | Self::RemoveWorkEntry { structure_id, .. }
            | Self::Remove { structure_id, .. }
            | Self::SetExperimentItem { structure_id, .. }
            | Self::ResetExperiment { structure_id, .. }
            | Self::UpgradeSanctuary {
                monolith_id: structure_id,
                ..
            } => protected(*structure_id),
            Self::InvestigatePOI { target_id, .. } => protected(*target_id),
            Self::ItemTransfer {
                source_id,
                target_id,
                ..
            }
            | Self::LootAll {
                source_id,
                target_id,
                ..
            } => protected(*source_id) || protected(*target_id),
            Self::ItemSplit { owner_id, .. } => protected(*owner_id),
            Self::OrderOperate {
                villager_id,
                structure_id,
                ..
            }
            | Self::OrderRefine {
                villager_id,
                structure_id,
                ..
            }
            | Self::OrderCraft {
                villager_id,
                structure_id,
                ..
            }
            | Self::OrderExperiment {
                villager_id,
                structure_id,
                ..
            }
            | Self::OrderPlant {
                villager_id,
                structure_id,
                ..
            }
            | Self::OrderTend {
                villager_id,
                structure_id,
                ..
            }
            | Self::OrderHarvest {
                villager_id,
                structure_id,
                ..
            }
            | Self::Assign {
                worker_id: villager_id,
                structure_id,
                ..
            }
            | Self::RemoveAssign {
                worker_id: villager_id,
                structure_id,
                ..
            } => protected(*villager_id) || protected(*structure_id),
            Self::OrderExplore { villager_id, .. }
            | Self::OrderProspect { villager_id, .. }
            | Self::OrderRepair { villager_id, .. } => protected(*villager_id),
            Self::Build {
                builder_id,
                structure_id,
                ..
            }
            | Self::Upgrade {
                builder_id,
                structure_id,
                ..
            } => protected(*builder_id) || protected(*structure_id),
            Self::Equip { obj_id, .. }
            | Self::DeleteItem { obj_id, .. }
            | Self::Use { obj_id, .. }
            | Self::Advance { id: obj_id, .. } => protected(*obj_id),
            Self::Hire {
                merchant_id,
                target_id,
                ..
            } => protected(*merchant_id) || protected(*target_id),
            Self::BuyItem { seller_id, .. } => protected(*seller_id),
            Self::SellItem { target_id, .. } => protected(*target_id),
            _ => false,
        }
    }
}

fn protected_player_event_mutation(
    event: &PlayerEvent,
    ids: &Ids,
    presence: &PlayerWorldPresenceState,
) -> bool {
    event.is_mutating_gameplay()
        && (is_player_offline_protected(event.player_id(), presence)
            || event.targets_protected_run(ids, presence))
}

pub type ActiveInfoPlayerId = i32;
pub type ActiveInfoObjId = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActiveInfoType {
    Obj,
    Structure,
    Inventory,
    ItemTransfer,
    Refine,
    StructureRefine,
    Craft,
    StructureCraft,
    Equip,
    Experiment,
    StructureQueue,
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct ActiveInfos(pub HashMap<(ActiveInfoObjId, ActiveInfoType), HashSet<ActiveInfoPlayerId>>);

impl ActiveInfos {
    pub fn add(&mut self, key: (ActiveInfoObjId, ActiveInfoType), value: ActiveInfoPlayerId) {
        self.0.entry(key).or_insert_with(HashSet::new).insert(value);
    }

    pub fn remove(&mut self, key: (ActiveInfoObjId, ActiveInfoType), value: ActiveInfoPlayerId) {
        // Remove the value from the set, if the set becomes empty remove the key from the map
        if let Some(set) = self.0.get_mut(&key) {
            set.remove(&value);
            if set.is_empty() {
                self.0.remove(&key);
            }
        }
    }
}

#[derive(QueryData)]
struct CoreQuery {
    entity: Entity,
    id: &'static Id,
    player_id: &'static PlayerId,
    pos: &'static Position,
    name: &'static Name,
    class: &'static Class,
    subclass: &'static Subclass,
    template: &'static Template,
    state: &'static State,
    active_task: Option<&'static ActiveTask>,
    misc: &'static Misc,
    portrait: Option<&'static Portrait>,
    effects: &'static Effects,
    inventory: &'static Inventory,
    hero_class: Option<&'static HeroClass>,
    last_combat_tick: Option<&'static LastCombatTick>,
    assignment: Option<&'static Assignment>,
    dropped_bag: Option<&'static DroppedBag>,
    viewshed: Option<&'static Viewshed>,
}

fn combat_locked(last_combat_tick: Option<&LastCombatTick>, game_tick: i32) -> bool {
    last_combat_tick
        .map(|last_combat_tick| is_combat_locked(game_tick, last_combat_tick))
        .unwrap_or(false)
}

fn send_combat_locked_error(player_id: i32, clients: &Res<Clients>) {
    send_to_client(
        player_id,
        ResponsePacket::Error {
            errmsg: "Cannot do that while in combat.".to_string(),
        },
        clients,
    );
}

const MIN_WORK_VISIBILITY_RANGE: u32 = 0;
pub(crate) const INSUFFICIENT_WORK_VISIBILITY_NOTICE: &str =
    "It is too dark to work. Equip and light a torch or move near a stronger light source.";

pub(crate) fn has_sufficient_work_visibility(
    viewshed: Option<&Viewshed>,
    player_id: i32,
    campfire_visibility: &CampfireVisibilityState,
) -> bool {
    viewshed.is_some_and(|viewshed| viewshed.range > MIN_WORK_VISIBILITY_RANGE)
        || campfire_visibility.illuminates_player(player_id)
}

pub(crate) fn send_insufficient_work_visibility_notice(player_id: i32, clients: &Res<Clients>) {
    send_to_client(
        player_id,
        ResponsePacket::Notice {
            noticemsg: INSUFFICIENT_WORK_VISIBILITY_NOTICE.to_string(),
            expiry: Some(5000),
        },
        clients,
    );
}

fn accepts_build_resource_transfer(class: &Class, state: &State) -> bool {
    class.is_structure() && matches!(state, State::Founded | State::PlanningUpgrade)
}

fn incomplete_structure_blocks_inventory_transfer(class: &Class, state: &State) -> bool {
    class.is_structure()
        && !accepts_build_resource_transfer(class, state)
        && !Structure::is_built(*state)
}

fn is_discovery_action_state(state: &State) -> bool {
    matches!(
        state,
        State::Surveying | State::Prospecting | State::Investigating | State::Exploring
    )
}

fn can_access_run_shipwreck(
    player_id: i32,
    obj_id: i32,
    template: &Template,
    run_spawned_objs: &RunSpawnedObjs,
) -> bool {
    template.0 != "Shipwreck" || run_spawned_objs.contains_for_player(player_id, obj_id)
}

fn send_shipwreck_owner_error(player_id: i32, clients: &Res<Clients>) {
    send_to_client(
        player_id,
        ResponsePacket::Error {
            errmsg: "This Shipwreck belongs to another survivor.".to_string(),
        },
        clients,
    );
}

fn can_access_shipwreck_inventory(
    player_id: i32,
    obj_id: i32,
    template: &Template,
    investigated_pois: &InvestigatedPOIs,
) -> bool {
    template.0 != "Shipwreck"
        || investigated_pois
            .get(&player_id)
            .map(|poi_ids| poi_ids.contains(&obj_id))
            .unwrap_or(false)
}

fn is_restricted_cooking_storage(template: &str) -> bool {
    matches!(
        template,
        templates::CAMPFIRE_TEMPLATE | templates::SHELTER_TENT_TEMPLATE
    )
}

fn accepts_completed_storage_item(template: &str, item_name: &str, item_subclass: &str) -> bool {
    !is_restricted_cooking_storage(template)
        || item_name == item::FIREWOOD
        || item_name == item::CHARCOAL
        || matches!(item_subclass, "Raw Meat" | "Cooked Meat")
}

fn send_shipwreck_search_error(player_id: i32, clients: &Res<Clients>) {
    send_to_client(
        player_id,
        ResponsePacket::Error {
            errmsg: "Search the Shipwreck before recovering its supplies.".to_string(),
        },
        clients,
    );
}

fn is_loot_all_source(
    player_id: i32,
    source_player_id: i32,
    source_template: &Template,
    source_state: &State,
) -> bool {
    source_template.0 == templates::DROPPED_BAG_TEMPLATE
        || (*source_state == State::Dead && source_player_id != player_id)
}

fn transfer_loot_that_fits(
    source_inventory: &mut Inventory,
    target_inventory: &mut Inventory,
    target_capacity: i32,
) -> usize {
    let loot: Vec<(i32, i32)> = source_inventory
        .items
        .iter()
        .filter(|item| item.quantity > 0)
        .map(|item| {
            (
                item.id,
                (item.quantity.max(0) as f32 * item.weight).max(0.0) as i32,
            )
        })
        .collect();
    let mut target_weight = target_inventory.get_total_weight();
    let mut transferred = 0;

    for (item_id, item_weight) in loot {
        if target_weight.saturating_add(item_weight) > target_capacity {
            continue;
        }

        Inventory::transfer(item_id, source_inventory, target_inventory);
        target_weight = target_weight.saturating_add(item_weight);
        transferred += 1;
    }

    transferred
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
struct ItemTransferQuery {
    entity: Entity,
    id: &'static Id,
    player_id: &'static PlayerId,
    pos: &'static Position,
    name: &'static Name,
    class: &'static Class,
    subclass: &'static Subclass,
    template: &'static Template,
    state: &'static State,
    misc: &'static Misc,
    inventory: &'static mut Inventory,
    order: Option<&'static Order>,
    active_task: Option<&'static mut ActiveTask>,
    blocked_work: Option<&'static BlockedWork>,
    tool_fetch_target: Option<&'static ToolFetchTarget>,
    dropped_bag: Option<&'static mut DroppedBag>,
}

fn dropped_bag_expires_in(expires_at: i32, game_tick: i32) -> i32 {
    let remaining_ticks = expires_at.saturating_sub(game_tick).max(0);
    (remaining_ticks + TICKS_PER_SEC - 1) / TICKS_PER_SEC
}

fn dropped_bag_can_accept(inventory: &Inventory, item: &Item) -> bool {
    inventory
        .get_total_weight()
        .saturating_add((item.quantity as f32 * item.weight) as i32)
        <= DROPPED_BAG_CAPACITY
}

fn cancel_empty_dropped_bag_despawn(
    game_events: &mut GameEvents,
    bag_id: i32,
    fixed_expires_at: i32,
) {
    game_events.retain(|_, event| {
        !matches!(event.event_type, GameEventType::DespawnObj { obj_id } if obj_id == bag_id)
            || event.run_tick == fixed_expires_at
    });
}

fn dropped_bag_warning_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    mut query: Query<(&Position, &mut DroppedBag)>,
) {
    for (pos, mut bag) in query.iter_mut() {
        let remaining = bag.expires_at.saturating_sub(game_tick.0);
        let warning =
            if remaining <= DROPPED_BAG_TEN_SECOND_WARNING_TICKS && !bag.warned_ten_seconds {
                bag.warned_ten_seconds = true;
                bag.warned_one_minute = true;
                Some("10 seconds")
            } else if remaining <= DROPPED_BAG_ONE_MINUTE_WARNING_TICKS && !bag.warned_one_minute {
                bag.warned_one_minute = true;
                Some("one minute")
            } else {
                None
            };

        let Some(time) = warning else {
            continue;
        };
        for player_id in bag.contributors.iter().copied() {
            send_to_client(
                player_id,
                ResponsePacket::Notice {
                    noticemsg: format!(
                        "Your dropped bag at ({}, {}) disappears in {}.",
                        pos.x, pos.y, time
                    ),
                    expiry: Some(8000),
                },
                &clients,
            );
        }
    }
}

fn drop_item_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    presence: Res<PlayerWorldPresenceState>,
    game_tick: Res<GameTick>,
    mut game_events: ResMut<GameEvents>,
    mut query: Query<ItemTransferQuery>,
) {
    let existing_bags: HashMap<Position, (i32, Entity)> = query
        .iter()
        .filter(|obj| obj.template.0 == templates::DROPPED_BAG_TEMPLATE)
        .map(|obj| (*obj.pos, (obj.id.0, obj.entity)))
        .collect();
    let mut pending_bags: HashMap<Position, (i32, Inventory, DroppedBag)> = HashMap::new();
    let drop_requests: Vec<(i32, PlayerEvent)> = events
        .iter()
        .filter_map(|(event_id, event)| match event {
            PlayerEvent::DropItem { .. } => Some((*event_id, event.clone())),
            _ => None,
        })
        .collect();

    for (event_id, event) in drop_requests {
        events.remove(&event_id);

        if protected_player_event_mutation(&event, &ids, &presence) {
            continue;
        }

        let PlayerEvent::DropItem { player_id, item_id } = event else {
            continue;
        };

        let Some(hero_id) = ids.get_hero(player_id) else {
            error!("Cannot drop item without a hero for player {:?}", player_id);
            continue;
        };
        let Some(hero_entity) = entity_map.get_entity(hero_id) else {
            error!("Cannot find hero entity for {:?}", hero_id);
            continue;
        };

        let Ok(hero) = query.get(hero_entity) else {
            error!("Cannot query hero entity {:?}", hero_entity);
            continue;
        };

        if hero.player_id.0 != player_id || !hero.subclass.is_hero() {
            send_to_client(
                player_id,
                ResponsePacket::Error {
                    errmsg: "Items can only be dropped from your hero's inventory.".to_string(),
                },
                &clients,
            );
            continue;
        }
        if !hero.state.is_alive() {
            send_to_client(
                player_id,
                ResponsePacket::Error {
                    errmsg: "The dead cannot drop items.".to_string(),
                },
                &clients,
            );
            continue;
        }

        let hero_pos = *hero.pos;
        let Some(dropped_item) = hero.inventory.get_by_id(item_id) else {
            send_to_client(
                player_id,
                ResponsePacket::Error {
                    errmsg: "That item is no longer in your inventory.".to_string(),
                },
                &clients,
            );
            continue;
        };

        let bag_expires_at;
        if let Some((bag_id, bag_entity)) = existing_bags.get(&hero_pos).copied() {
            let Ok([mut hero, mut bag]) = query.get_many_mut([hero_entity, bag_entity]) else {
                error!("Cannot query hero and dropped bag on {:?}", hero_pos);
                continue;
            };
            let Some(dropped_bag) = bag.dropped_bag.as_deref_mut() else {
                error!("Dropped bag {:?} is missing its lifetime component", bag_id);
                continue;
            };
            if dropped_bag.expires_at <= game_tick.0 {
                send_to_client(
                    player_id,
                    ResponsePacket::Error {
                        errmsg: "That dropped bag has already expired.".to_string(),
                    },
                    &clients,
                );
                continue;
            }
            if !dropped_bag_can_accept(&bag.inventory, &dropped_item) {
                send_to_client(
                    player_id,
                    ResponsePacket::Error {
                        errmsg: format!(
                            "Dropped bags can hold only {} weight. Split the stack or use another tile.",
                            DROPPED_BAG_CAPACITY
                        ),
                    },
                    &clients,
                );
                continue;
            }
            Inventory::transfer(item_id, &mut hero.inventory, &mut bag.inventory);
            dropped_bag.add_contributor(player_id);
            cancel_empty_dropped_bag_despawn(&mut game_events, bag_id, dropped_bag.expires_at);
            bag_expires_at = dropped_bag.expires_at;
        } else {
            let (bag_id, expires_at) = if let Some((bag_id, bag_inventory, dropped_bag)) =
                pending_bags.get_mut(&hero_pos)
            {
                let Ok(mut hero) = query.get_mut(hero_entity) else {
                    error!("Cannot query hero entity {:?}", hero_entity);
                    continue;
                };
                if !dropped_bag_can_accept(bag_inventory, &dropped_item) {
                    send_to_client(
                        player_id,
                        ResponsePacket::Error {
                            errmsg: format!(
                                "Dropped bags can hold only {} weight. Split the stack or use another tile.",
                                DROPPED_BAG_CAPACITY
                            ),
                        },
                        &clients,
                    );
                    continue;
                }
                Inventory::transfer(item_id, &mut hero.inventory, bag_inventory);
                dropped_bag.add_contributor(player_id);
                (*bag_id, dropped_bag.expires_at)
            } else {
                if !dropped_bag_can_accept(
                    &Inventory {
                        owner: -1,
                        items: Vec::new(),
                    },
                    &dropped_item,
                ) {
                    send_to_client(
                        player_id,
                        ResponsePacket::Error {
                            errmsg: format!(
                                "Dropped bags can hold only {} weight. Split the stack before dropping it.",
                                DROPPED_BAG_CAPACITY
                            ),
                        },
                        &clients,
                    );
                    continue;
                }
                let bag_id = ids.new_obj_id();
                let mut bag_inventory = Inventory {
                    owner: bag_id,
                    items: Vec::new(),
                };
                let Ok(mut hero) = query.get_mut(hero_entity) else {
                    error!("Cannot query hero entity {:?}", hero_entity);
                    continue;
                };
                Inventory::transfer(item_id, &mut hero.inventory, &mut bag_inventory);
                let expires_at = game_tick.0 + DROPPED_BAG_LIFETIME_TICKS;
                pending_bags.insert(
                    hero_pos,
                    (
                        bag_id,
                        bag_inventory,
                        DroppedBag::new(expires_at, player_id),
                    ),
                );
                (bag_id, expires_at)
            };

            debug!("Queued item {:?} for dropped bag {:?}", item_id, bag_id);
            bag_expires_at = expires_at;
        }

        send_to_client(
            player_id,
            ResponsePacket::InfoItemsUpdate {
                id: hero_id,
                items_updated: Vec::new(),
                items_removed: vec![item_id],
            },
            &clients,
        );
        send_to_client(
            player_id,
            ResponsePacket::Notice {
                noticemsg: format!(
                    "Dropped {} x{}. Anyone can loot it; this bag disappears in {} seconds. Adding items does not reset the timer.",
                    dropped_item.name,
                    dropped_item.quantity,
                    dropped_bag_expires_in(bag_expires_at, game_tick.0),
                ),
                expiry: Some(6000),
            },
            &clients,
        );
    }

    for (pos, (bag_id, inventory, dropped_bag)) in pending_bags {
        let expires_at = dropped_bag.expires_at;
        let bag = Obj::create_nospawn(
            bag_id,
            NPC_PLAYER_ID,
            templates::DROPPED_BAG_TEMPLATE.to_string(),
            pos,
            State::None,
            inventory,
            &templates,
        );
        let bag_entity = commands.spawn((bag, dropped_bag)).id();
        ids.new_obj(bag_id, NPC_PLAYER_ID);
        entity_map.new_obj(bag_id, bag_entity);
        commands.trigger(NewObj { entity: bag_entity });

        let despawn_event_id = ids.new_map_event_id();
        game_events.insert(
            despawn_event_id,
            GameEvent {
                event_id: despawn_event_id,
                start_tick: game_tick.0,
                run_tick: expires_at,
                event_type: GameEventType::DespawnObj { obj_id: bag_id },
            },
        );
    }
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
    inventory: &'static mut Inventory,
    work_queue: &'static mut WorkQueue,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
struct VillagerQuery {
    entity: Entity,
    id: &'static Id,
    player_id: &'static PlayerId,
    pos: &'static Position,
    name: &'static Name,
    class: &'static Class,
    subclass: &'static Subclass,
    state: &'static State,
    misc: &'static Misc,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PlayerInputSet {
    Collect,
    ProtectionGuard,
    Handle,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // Initialize events
        let player_events: PlayerEvents = PlayerEvents(HashMap::new());
        let active_infos: ActiveInfos = ActiveInfos(HashMap::new());

        let start_file =
            fs::File::open("templates/player_start.yaml").expect("Could not open file.");
        let mut start_locations =
            StartLocations(serde_yaml::from_reader(start_file).expect("Could not read values."));
        // Give each start location a distinct random team color (hero + villagers).
        crate::player_setup::assign_start_location_colors(&mut start_locations.0);

        app.configure_sets(
            Update,
            (
                PlayerInputSet::Collect,
                PlayerInputSet::ProtectionGuard,
                PlayerInputSet::Handle,
            )
                .chain(),
        )
        .add_systems(
            Update,
            message_broker_system
                .in_set(PlayerInputSet::Collect)
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            protected_player_event_guard_system
                .in_set(PlayerInputSet::ProtectionGuard)
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                safe_logout_command_bridge_system,
                new_player_system,
                login_system,
                move_system,
                combo_tracker_timeout_system.before(attack_system),
                attack_system,
            )
                .in_set(PlayerInputSet::Handle)
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                gather_system,
                get_stats_system,
                info_skills_system,
                info_attrs_system,
                info_advance_system,
            )
                .in_set(PlayerInputSet::Handle)
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                info_upgrade_system,
                info_tile_system,
                info_item_system,
                info_merchant_system,
                info_hire_system,
                info_experiment_system,
                item_transfer_system,
                drop_item_system,
                dropped_bag_warning_system,
                item_split_system,
                info_refine_system,
                order_follow_system,
                order_gather_system,
                order_operate_system,
                structure_queue_system,
                order_farm_system,
                order_repair_system,
            )
                .in_set(PlayerInputSet::Handle)
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                order_experiment_system,
                structure_list_system,
                create_foundation_system,
                build_system,
                start_upgrade_system,
                upgrade_system,
                info_assign_system,
                assign_system,
                equip_system,
                info_craft_system,
                info_structure_craft_system,
                info_structure_queue_system,
                use_item_system,
                remove_system,
                set_experiment_item_system,
                hire_system,
                upgrade_sanctuary_system,
                buy_sell_system,
                activate_system,
            )
                .in_set(PlayerInputSet::Handle)
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                survey_system,
                prospect_system,
                investigate_system,
                order_prospect_system,
            )
                .in_set(PlayerInputSet::Handle)
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                refine_system,
                structure_refine_system,
                info_structure_refine_system,
                sleep_system,
                cancel_action_system,
                experiment_system,
                debug_obj_system,
                set_log_level_system,
                get_log_levels_system,
            )
                .in_set(PlayerInputSet::Handle)
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (info_obj_system,)
                .in_set(PlayerInputSet::Handle)
                .run_if(in_state(AppState::Running)),
        )
        .add_observer(info_hero_system)
        .add_observer(info_villager_system)
        .add_observer(info_structure_system)
        .add_observer(info_monolith_system)
        .add_observer(info_poi_system)
        .add_observer(info_npc_system)
        .add_observer(combat_effects_changed_observer)
        .insert_resource(player_events)
        .insert_resource(active_infos)
        .insert_resource(start_locations)
        .init_resource::<AssignedStartLocations>()
        .init_resource::<InvestigatedPOIs>()
        .init_resource::<RunSpawnedObjs>();
    }
}

fn message_broker_system(
    client_to_game_receiver: Res<NetworkReceiver>,
    mut player_events: ResMut<PlayerEvents>,
    mut ids: ResMut<Ids>,
) {
    if let Ok(evt) = client_to_game_receiver.try_recv() {
        if env::var("NETWORK_DEBUG").is_ok() {
            println!("{:?}", evt);
        }

        player_events.insert(ids.player_event, evt.clone());

        ids.player_event += 1;
    }
}

fn protected_player_event_guard_system(
    mut player_events: ResMut<PlayerEvents>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    mut telemetry: ResMut<SafeLogoutTelemetryState>,
) {
    let mut rejected = Vec::new();

    for (event_id, event) in player_events.iter() {
        if protected_player_event_mutation(event, &ids, &presence) {
            let player_id = event.player_id();
            telemetry.record_protected_input_rejection(player_id);
            info!(
                "safe_logout_protected_input_rejected player_id={} reason=protected_source_or_target",
                player_id
            );
            rejected.push(*event_id);
        }
    }

    for event_id in rejected {
        player_events.remove(&event_id);
    }
}

/// Convert authenticated network ingress into the existing internal
/// safe-logout messages. This bridge deliberately owns no validation or
/// presence transition; the Checkpoint 1 systems remain the sole authority.
fn safe_logout_command_bridge_system(
    mut player_events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    mut telemetry: ResMut<SafeLogoutTelemetryState>,
    mut requests: MessageWriter<RequestSafeLogout>,
    mut cancellations: MessageWriter<CancelSafeLogout>,
) {
    let command_ids = player_events
        .iter()
        .filter_map(|(event_id, event)| {
            matches!(
                event,
                PlayerEvent::RequestSafeLogout { .. } | PlayerEvent::CancelSafeLogout { .. }
            )
            .then_some(*event_id)
        })
        .collect::<Vec<_>>();

    for event_id in command_ids {
        match player_events.remove(&event_id) {
            Some(PlayerEvent::RequestSafeLogout {
                player_id,
                connection_id,
            }) if clients.is_current_connection(player_id, connection_id) => {
                requests.write(RequestSafeLogout {
                    player_id,
                    connection_id,
                });
            }
            Some(PlayerEvent::CancelSafeLogout {
                player_id,
                connection_id,
            }) if clients.is_current_connection(player_id, connection_id) => {
                cancellations.write(CancelSafeLogout {
                    player_id,
                    connection_id,
                });
            }
            Some(PlayerEvent::RequestSafeLogout { player_id, .. }) => {
                telemetry.record_stale_connection_event(player_id);
                info!(
                    "safe_logout_stale_command_rejected player_id={} command=request",
                    player_id
                );
            }
            Some(PlayerEvent::CancelSafeLogout { player_id, .. }) => {
                telemetry.record_stale_connection_event(player_id);
                info!(
                    "safe_logout_stale_command_rejected player_id={} command=cancel",
                    player_id
                );
            }
            _ => {}
        }
    }
}

fn new_player_system(
    mut events: ResMut<PlayerEvents>,
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    // Bundled into one tuple param to stay within Bevy's 16 system-parameter limit.
    mut start_location_res: (
        ResMut<StartLocations>,
        ResMut<AssignedStartLocations>,
        ResMut<RunSpawnedObjs>,
    ),
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    mut recipes: ResMut<Recipes>,
    mut plans: ResMut<Plans>,
    templates: Res<Templates>,
    mut player_setup_state: ParamSet<(
        ResMut<PlayerStats>,
        ResMut<SpawnPositions>,
        ResMut<RunScoreState>,
    )>,
    mut run_intro_state: (
        ResMut<PlayerIntroState>,
        ResMut<IntroEncounterState>,
        ResMut<InitialEncounterState>,
        ResMut<SettlementCrisisState>,
        ResMut<PlayerWorldPresenceState>,
        ResMut<SafeLogoutTelemetryState>,
        ResMut<CrisisBalanceTelemetryState>,
        ResMut<CrisisBalanceObservationState>,
        ResMut<PersonalCrisisHistory>,
        ResMut<ResourceDiscoveries>,
        ResMut<SurveyHistory>,
    ),
    monoliths: Query<ObjQuery, With<Monolith>>,
    crisis_assault_units: Query<(Entity, &Id, &CrisisAssaultUnit)>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::NewPlayer {
                player_id,
                hero_name,
                class_name,
                portrait,
            } => {
                events_to_remove.push(*event_id);
                // SelectedClass is client-originated, so reject attempts to
                // create a second run before True Death has released the
                // current hero and start assignment. Besides duplicate heroes,
                // accepting this here would let a player erase an active
                // personal assault through the fresh-run orphan sweep below.
                if start_location_res.1.contains_key(player_id)
                    || ids.get_hero(*player_id).is_some()
                {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "A run is already active for this player.".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }
                let setup_result = {
                    let mut spawn_positions = player_setup_state.p1();
                    player_setup::new(
                        *player_id,
                        hero_name.to_string(),
                        class_name.to_string(),
                        portrait.to_string(),
                        &mut commands,
                        &mut start_location_res.0,
                        &mut start_location_res.1,
                        &mut ids,
                        &mut entity_map,
                        &mut map_events,
                        &mut game_events,
                        &mut recipes,
                        &mut plans,
                        &templates,
                        &game_tick,
                        &monoliths,
                        &mut spawn_positions,
                        &mut run_intro_state.0,
                        &mut run_intro_state.1,
                        &mut run_intro_state.2,
                        &mut start_location_res.2,
                    )
                };

                match setup_result {
                    Ok(_) => {
                        // Successful hero recreation is a fresh run. Sweep any
                        // attributed orphan left by an overlapping old cleanup
                        // without touching another player's assault.
                        let stale_units = crisis_assault_units
                            .iter()
                            .filter(|(_, _, assault)| assault.owner_player_id == *player_id)
                            .map(|(entity, id, _)| (entity, id.0))
                            .collect::<Vec<_>>();
                        if !stale_units.is_empty() {
                            let stale_ids = stale_units
                                .iter()
                                .map(|(_, id)| *id)
                                .collect::<HashSet<_>>();
                            map_events.retain(|_, event| !stale_ids.contains(&event.obj_id));
                            if let Some(run_ids) = start_location_res.2.get_mut(player_id) {
                                run_ids.retain(|id| !stale_ids.contains(id));
                            }
                            for (entity, id) in stale_units {
                                ids.remove_obj(id);
                                if entity_map.get_entity(id) == Some(entity) {
                                    commands.trigger(RemoveObj { entity });
                                } else {
                                    // An overlapping cleanup may already have
                                    // removed the map entry while leaving the
                                    // attributed entity visible to this query.
                                    commands.entity(entity).try_despawn();
                                }
                            }
                        }
                        // A recreated hero is a fresh run. The personal crisis
                        // system will deterministically create a new Dormant
                        // entry on the next eligible evaluation.
                        run_intro_state.3.remove(player_id);
                        run_intro_state.6.remove(player_id);
                        run_intro_state.7 .0.remove(player_id);
                        run_intro_state.8.by_player.remove(player_id);
                        run_intro_state.9.clear_player(*player_id);
                        run_intro_state.10.remove(player_id);
                        initialize_player_presence(
                            *player_id,
                            clients.is_player_online(*player_id),
                            game_tick.0,
                            &mut run_intro_state.4,
                            &mut run_intro_state.5,
                        );
                        let event_type = GameEventType::Login {
                            player_id: *player_id,
                            connection_id: clients
                                .current_connection_id(*player_id)
                                .map(|connection_id| connection_id.as_u128())
                                .unwrap_or_default(),
                        };
                        let event_id = ids.new_map_event_id();

                        let event = GameEvent {
                            event_id: event_id,
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + 4, // Add one game tick
                            event_type,
                        };

                        player_setup_state.p0().insert(
                            *player_id,
                            PlayerStat {
                                player_id: *player_id,
                                num_deaths: 0,
                                damage_records: VecDeque::with_capacity(10),
                            },
                        );

                        player_setup_state.p2().insert(
                            *player_id,
                            PlayerRunScore {
                                start_tick: game_tick.0,
                                ..PlayerRunScore::default()
                            },
                        );

                        game_events.insert(event.event_id, event);
                    }
                    Err(err) => {
                        let packet = ResponsePacket::Error {
                            errmsg: err.to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }
                }
            }
            _ => {}
        }
    }

    for index in events_to_remove.iter() {
        events.remove(index);
    }
}

fn login_system(
    clients: Res<Clients>,
    mut events: ResMut<PlayerEvents>,
    game_tick: ResMut<GameTick>,
    mut game_events: ResMut<GameEvents>,
    mut ids: ResMut<Ids>,
    mut presence: ResMut<PlayerWorldPresenceState>,
    mut safe_logout_telemetry: ResMut<SafeLogoutTelemetryState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Login {
                player_id,
                connection_id,
            } => {
                events_to_remove.push(*event_id);

                if !clients.is_current_connection(*player_id, *connection_id) {
                    safe_logout_telemetry.record_stale_connection_event(*player_id);
                    info!(
                        "player_login_stale_connection_rejected player_id={} game_tick={}",
                        player_id, game_tick.0
                    );
                    continue;
                }
                if !presence.players.contains_key(player_id) {
                    // Presence is normally reconciled in PostUpdate, but an
                    // authenticated Login queued during loading can be the
                    // first Running-update event. Initialize that exact current
                    // session here so its delayed map/world/perception sync is
                    // not mistaken for a duplicate and permanently discarded.
                    initialize_player_presence(
                        *player_id,
                        true,
                        game_tick.0,
                        &mut presence,
                        &mut safe_logout_telemetry,
                    );
                }
                if !mark_player_logged_in(
                    *player_id,
                    *connection_id,
                    game_tick.0,
                    &mut presence,
                    &mut safe_logout_telemetry,
                ) {
                    continue;
                }

                let event_type = GameEventType::Login {
                    player_id: *player_id,
                    connection_id: connection_id.as_u128(),
                };
                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 4, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);
            }
            _ => {}
        }
    }

    for index in events_to_remove.iter() {
        events.remove(index);
    }
}

fn move_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: ResMut<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    map: Res<Map>,
    hero_query: Query<CoreQuery, With<SubclassHero>>,
    query: Query<ObjQuery>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Move { player_id, x, y } => {
                debug!("Move Event: {:?}", event);
                events_to_remove.push(*event_id);

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    break;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    break;
                };

                let Ok(hero) = hero_query.get(hero_entity) else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    break;
                };

                if Obj::is_dead(hero.state) {
                    let error = ResponsePacket::Error {
                        errmsg: "The dead cannot move.".to_owned(),
                    };
                    send_to_client(*player_id, error, &clients);
                    continue;
                }

                if !Map::is_passable(*x, *y, &map) {
                    let error = ResponsePacket::Error {
                        errmsg: "Tile is not passable.".to_owned(),
                    };
                    send_to_client(*player_id, error, &clients);
                    continue;
                }

                if !is_pos_empty(*player_id, *x, *y, &query) {
                    let error = ResponsePacket::Error {
                        errmsg: "Tile is occupied.".to_owned(),
                    };
                    send_to_client(*player_id, error, &clients);
                    continue;
                }

                // Remove events that are cancellable
                let mut events_to_remove = Vec::new();

                // TODO move this into a function
                for (map_event_id, map_event) in map_events.iter() {
                    if map_event.obj_id == hero_id {
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
                                events_to_remove.push(*map_event_id);
                            }
                            _ => {}
                        }
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

                // Add State Change Event to Moving
                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Moving,
                });

                // Add Move Event
                let move_event = VisibleEvent::MoveEvent {
                    src: hero.pos.clone(),
                    dst: Position { x: *x, y: *y },
                };

                map_events.new(
                    hero.id.0,
                    game_tick.0 + 12, // in the future
                    move_event,
                );
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn combo_hints_for_history(
    attack_history: &Vec<String>,
    templates: &Templates,
) -> (Vec<network::ComboHint>, Option<String>) {
    let mut matching_combos = Vec::new();
    let mut available_finisher = None;

    if attack_history.is_empty() {
        return (matching_combos, available_finisher);
    }

    for (_combo_name, combo_template) in templates.combo_templates.iter() {
        if attack_history.len() > combo_template.attacks.len() {
            continue;
        }

        let is_prefix = attack_history
            .iter()
            .zip(combo_template.attacks.iter())
            .all(|(history_attack, combo_attack)| history_attack == combo_attack);

        if !is_prefix {
            continue;
        }

        if attack_history.len() == combo_template.attacks.len() {
            available_finisher = Some(combo_template.name.clone());
        } else {
            matching_combos.push(network::ComboHint {
                name: combo_template.name.clone(),
                remaining_attacks: combo_template.attacks[attack_history.len()..].to_vec(),
                effect: combo_template.effects.first().cloned(),
            });
        }
    }

    return (matching_combos, available_finisher);
}

fn live_combo_history_for_target(
    tracker: Option<&crate::combat::ComboTracker>,
    target_id: i32,
    game_tick: i32,
) -> Vec<String> {
    Combat::live_combo_attacks_before_append(tracker, target_id, game_tick)
        .into_iter()
        .map(|attack| attack.to_str())
        .collect()
}

pub(crate) fn combo_chain_cooldown_ticks(chain_length: usize) -> i32 {
    match chain_length {
        0 | 1 => ATTACK_COOLDOWN_TICKS,
        2 => 25,
        3 => 20,
        // Current templates top out at four attacks, so strict-prefix tempo
        // cannot reach this rung until a five-attack combo is introduced.
        _ => 15,
    }
}

pub(crate) fn combo_tempo_prefix_len(
    attacks: &[crate::combat::AttackType],
    templates: &Templates,
) -> usize {
    templates
        .combo_templates
        .iter()
        .any(|(_, combo)| {
            attacks.len() < combo.attacks.len()
                && attacks
                    .iter()
                    .zip(combo.attacks.iter())
                    .all(|(attack, expected)| attack.clone().to_str() == *expected)
        })
        .then_some(attacks.len())
        .unwrap_or_default()
}

fn cooldown_seconds(cooldown_ticks: i32) -> f32 {
    cooldown_ticks as f32 / TICKS_PER_SEC as f32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlayerAttackCooldown {
    attack_tick: i32,
    cooldown_ticks: i32,
}

fn attack_is_on_cooldown(
    cooldowns: &HashMap<i32, PlayerAttackCooldown>,
    player_id: i32,
    game_tick: i32,
) -> bool {
    cooldowns.get(&player_id).is_some_and(|cooldown| {
        cooldown.attack_tick > 0
            && game_tick.saturating_sub(cooldown.attack_tick) < cooldown.cooldown_ticks
    })
}

fn record_attack_cooldown(
    cooldowns: &mut HashMap<i32, PlayerAttackCooldown>,
    player_id: i32,
    game_tick: i32,
    cooldown_ticks: i32,
) {
    cooldowns.insert(
        player_id,
        PlayerAttackCooldown {
            attack_tick: game_tick,
            cooldown_ticks,
        },
    );
}

fn is_first_combo_discovery(
    discoveries: &mut HashSet<(i32, String)>,
    source_id: i32,
    combo_name: &str,
) -> bool {
    discoveries.insert((source_id, combo_name.to_string()))
}

fn combo_finisher_can_fire(
    available_finisher: Option<&str>,
    _basic_attack_cooldown_active: bool,
) -> bool {
    // Finishers are the payoff for completing a chain and intentionally bypass
    // the basic-attack cooldown. The finisher itself starts the next cooldown.
    available_finisher.is_some()
}

fn combo_finisher_rejection(
    available_finisher: Option<&str>,
    basic_attack_cooldown_active: bool,
) -> Option<&'static str> {
    (!combo_finisher_can_fire(available_finisher, basic_attack_cooldown_active))
        .then_some("No combo is ready.")
}

fn combo_tracker_timeout_system(
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    entity_map: Res<EntityObjMap>,
    mut query: Query<(
        &PlayerId,
        &mut crate::combat::ComboTracker,
        Option<&SubclassHero>,
    )>,
    target_query: Query<(&Template, &Effects, Option<&CrisisAssaultUnit>)>,
) {
    for (player_id, mut tracker, hero) in query.iter_mut() {
        if !tracker.attacks.is_empty()
            && game_tick.0.saturating_sub(tracker.last_attack_tick)
                > crate::combat::COMBO_CHAIN_TIMEOUT_TICKS
        {
            let target_id = tracker.target_id;
            tracker.attacks.clear();
            tracker.target_id = -1;

            // Villagers also use ComboTracker internally, but CombatState is
            // the owning hero's UI. Always expire stale NPC histories while
            // allowing only the hero tracker to publish that UI state.
            if hero.is_none() {
                continue;
            }

            let (enemy_intent, target_effects) = entity_map
                .get_entity(target_id)
                .and_then(|entity| target_query.get(entity).ok())
                .map(|(template, effects, crisis_assault)| {
                    let mut target_effects = effects
                        .0
                        .keys()
                        .cloned()
                        .map(Effect::to_str)
                        .collect::<Vec<_>>();
                    target_effects.sort();
                    (
                        enemy_intent_for_template(&template.0, crisis_assault.is_some()),
                        target_effects,
                    )
                })
                .unwrap_or_else(|| (String::new(), Vec::new()));
            send_to_client(
                player_id.0,
                ResponsePacket::CombatState {
                    version: 2,
                    target_id,
                    enemy_intent,
                    attack_history: Vec::new(),
                    matching_combos: Vec::new(),
                    available_finisher: None,
                    target_effects,
                    stamina_costs: network::StaminaCosts {
                        quick: 5,
                        precise: 5,
                        fierce: 5,
                        block: 0,
                    },
                    abilities: Vec::new(),
                    counter_hint: String::new(),
                },
                &clients,
            );
        }
    }
}

fn enemy_intent_for_template(template: &str, personal_assault: bool) -> String {
    match template {
        "Giant Rat" | "Cave Bat" | "Ash Viper" | "Mountain Lion" | "Saberfang Cat"
        | "Terror Bird" | "Spider" | "Scorpion" => {
            "Fast creature looking for an opening".to_string()
        }
        "Bog Leech" => "Low creature trying to drag the fight close".to_string(),
        "Moss Mite" => "Tiny pest chipping at close range".to_string(),
        "Thorn Beetle" => "Armored pest bracing through light attacks".to_string(),
        "Swiftstep Hare" | "Windstride Stag" | "Frostmane Elk" => {
            "Startled wildlife trying to stay clear".to_string()
        }
        "Reef Skitter" | "Wolf" | "Wild Boar" | "Giant Crab" => {
            "Close-range attacker testing your position".to_string()
        }
        "Black Bear" => "Heavy predator ready to maul anything too close".to_string(),
        "Cave Bear" => "Ancient brute bearing down with crushing force".to_string(),
        "Zombie" | "Skeleton" | "Shipwreck Zombie" | "Shadow" => {
            "Undead pressure advancing steadily".to_string()
        }
        "Necromancer" => "Caster seeking distance and corpses to exploit".to_string(),
        "Wolf Rider" | "Goblin Pillager" if personal_assault => {
            "Raider advancing on your defenders and blocking walls".to_string()
        }
        "Wolf Rider" | "Goblin Pillager" => {
            "Raider targeting your stored value and structures".to_string()
        }
        _ => "Hostile target preparing to attack".to_string(),
    }
}

fn counter_hint_for_template(template: &str, attack_history: &Vec<String>) -> String {
    if attack_history.is_empty() {
        return "Start with quick for control, precise for setup, fierce for damage, or block to buy time.".to_string();
    }

    match template {
        "Giant Rat"
        | "Cave Bat"
        | "Ash Viper"
        | "Mountain Lion"
        | "Saberfang Cat"
        | "Terror Bird"
        | "Spider"
        | "Wolf" => "Fast enemies reward control: quick chains toward Hamstring, while block protects low stamina.".to_string(),
        "Bog Leech" | "Moss Mite" => {
            "Weak close-range enemies can be finished quickly; block if stamina is low.".to_string()
        }
        "Reef Skitter" => {
            "Mobile shell enemies reward quick control or a block before trading damage.".to_string()
        }
        "Thorn Beetle" => {
            "Armored enemies reward setup: use precise attacks before committing fierce damage.".to_string()
        }
        "Swiftstep Hare" | "Windstride Stag" | "Frostmane Elk" => {
            "Passive wildlife rarely presses the fight; use light attacks if you must hunt it.".to_string()
        }
        "Black Bear" => {
            "Heavy predators punish sloppy trades; block to stabilize, then use precise setup before fierce damage.".to_string()
        }
        "Cave Bear" => {
            "Ancient brutes punish sloppy trades; block to stabilize, then use precise setup before fierce damage.".to_string()
        }
        "Skeleton" | "Zombie" | "Shipwreck Zombie" => {
            "Steady undead can be set up with precise attacks, then punished with a combo finisher.".to_string()
        }
        "Necromancer" => {
            "Pressure the caster before corpses become resources; block if you cannot close safely.".to_string()
        }
        _ => "Follow the visible combo hints or block when the exchange is turning against you.".to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AbilityCostType {
    Stamina,
    Mana,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AbilityEffect {
    ShieldBash,
    AimedShot,
    Disengage,
    ArcaneBolt,
    Ward,
}

const GUARD_BASH_STUN_TICKS: i32 = 2 * TICKS_PER_SEC;
const WARRIOR_BRACE_DURATION_TICKS: i32 = 75;
const WARRIOR_BRACE_AMPLIFIER: f32 = 1.5;
const STANDARD_BRACE_DURATION_TICKS: i32 = 50;
const STANDARD_BRACE_AMPLIFIER: f32 = 1.0;
const MAGE_WARD_DURATION_TICKS: i32 = 75;
const MAGE_WARD_AMPLIFIER: f32 = 1.0;
const BASIC_ATTACK_STAMINA_COST: i32 = 5;
const WATCHTOWER_RANGED_STAMINA_COST: i32 = 4;
const WATCHTOWER_AIMED_SHOT_STAMINA_COST: i32 = 7;
const WATCHTOWER_RANGED_RANGE_BONUS: u32 = 1;
const WATCHTOWER_RANGED_ACCURACY_BONUS: i32 = 10;
const WATCHTOWER_RANGED_DAMAGE_BONUS: i32 = 1;
const DEFAULT_RANGED_ACCURACY: i32 = 75;
const MIN_RANGED_HIT_CHANCE: i32 = 35;
const MAX_RANGED_HIT_CHANCE: i32 = 95;

#[derive(Debug, Clone, Copy, PartialEq)]
struct AttackProfile {
    range: u32,
    is_ranged: bool,
    accuracy: Option<i32>,
    damage_bonus: i32,
    stamina_cost: i32,
}

impl Default for AttackProfile {
    fn default() -> Self {
        Self {
            range: 1,
            is_ranged: false,
            accuracy: None,
            damage_bonus: 0,
            stamina_cost: BASIC_ATTACK_STAMINA_COST,
        }
    }
}

fn numeric_attr(item: &Item, attr: &AttrKey) -> Option<i32> {
    match item.attrs.get(attr) {
        Some(AttrVal::Num(value)) => Some(*value as i32),
        _ => None,
    }
}

fn equipped_ranged_weapon_profile(
    inventory: &Inventory,
    effects: &Effects,
) -> Option<AttackProfile> {
    let main_hand = inventory.get_equipped_main_hand()?;

    if main_hand.class != WEAPON {
        return None;
    }

    let base_range = numeric_attr(&main_hand, &AttrKey::AttackRange)?;
    if base_range <= 1 {
        return None;
    }

    let stationed = effects.has(Effect::WatchtowerLight);
    let mut profile = AttackProfile {
        range: base_range as u32,
        is_ranged: true,
        accuracy: Some(
            numeric_attr(&main_hand, &AttrKey::Accuracy).unwrap_or(DEFAULT_RANGED_ACCURACY),
        ),
        damage_bonus: 0,
        stamina_cost: BASIC_ATTACK_STAMINA_COST,
    };

    if stationed {
        profile.range += WATCHTOWER_RANGED_RANGE_BONUS;
        profile.accuracy = profile
            .accuracy
            .map(|accuracy| accuracy + WATCHTOWER_RANGED_ACCURACY_BONUS);
        profile.damage_bonus += WATCHTOWER_RANGED_DAMAGE_BONUS;
        profile.stamina_cost = WATCHTOWER_RANGED_STAMINA_COST;
    }

    Some(profile)
}

fn basic_attack_profile(actor: &CombatQueryItem) -> AttackProfile {
    equipped_ranged_weapon_profile(&actor.inventory, &actor.effects).unwrap_or_default()
}

fn ranged_hit_chance(profile: AttackProfile, distance: u32) -> i32 {
    let accuracy = profile.accuracy.unwrap_or(DEFAULT_RANGED_ACCURACY);
    let distance_penalty = (distance as i32 - 1).max(0) * 10;
    (accuracy - distance_penalty).clamp(MIN_RANGED_HIT_CHANCE, MAX_RANGED_HIT_CHANCE)
}

fn ranged_attack_hits(profile: AttackProfile, distance: u32) -> bool {
    if !profile.is_ranged {
        return true;
    }

    let hit_chance = ranged_hit_chance(profile, distance);
    rand::thread_rng().gen_range(0..100) < hit_chance
}

fn ability_effective_cost(ability: AbilityDef, actor: &CombatQueryItem) -> i32 {
    if ability.effect == AbilityEffect::AimedShot
        && equipped_ranged_weapon_profile(&actor.inventory, &actor.effects).is_some()
        && actor.effects.has(Effect::WatchtowerLight)
    {
        WATCHTOWER_AIMED_SHOT_STAMINA_COST
    } else {
        ability.cost
    }
}

fn ability_effective_range(ability: AbilityDef, actor: &CombatQueryItem) -> u32 {
    if ability.effect == AbilityEffect::AimedShot
        && equipped_ranged_weapon_profile(&actor.inventory, &actor.effects).is_some()
        && actor.effects.has(Effect::WatchtowerLight)
    {
        ability.range + WATCHTOWER_RANGED_RANGE_BONUS
    } else {
        ability.range
    }
}

fn aimed_shot_profile(actor: &CombatQueryItem) -> Option<AttackProfile> {
    let mut profile = equipped_ranged_weapon_profile(&actor.inventory, &actor.effects)?;
    profile.stamina_cost = ability_effective_cost(ability_def("aimed_shot")?, actor);
    Some(profile)
}

#[derive(Clone, Copy)]
struct AbilityDef {
    id: &'static str,
    label: &'static str,
    hero_class: HeroClass,
    cost_type: AbilityCostType,
    cost: i32,
    range: u32,
    cooldown: i32,
    required_weapon_subclass: Option<&'static str>,
    requires_target: bool,
    effect: AbilityEffect,
    hint: &'static str,
}

fn ability_def(ability_id: &str) -> Option<AbilityDef> {
    match ability_id {
        "shield_bash" => Some(AbilityDef {
            id: "shield_bash",
            label: "Guard Bash",
            hero_class: HeroClass::Warrior,
            cost_type: AbilityCostType::Stamina,
            cost: 10,
            range: 1,
            cooldown: ATTACK_COOLDOWN_SECONDS,
            required_weapon_subclass: None,
            requires_target: true,
            effect: AbilityEffect::ShieldBash,
            hint: "Stuns an adjacent threat and raises your guard.",
        }),
        "aimed_shot" => Some(AbilityDef {
            id: "aimed_shot",
            label: "Aimed Shot",
            hero_class: HeroClass::Ranger,
            cost_type: AbilityCostType::Stamina,
            cost: 8,
            range: 3,
            cooldown: ATTACK_COOLDOWN_SECONDS,
            required_weapon_subclass: Some("Bow"),
            requires_target: true,
            effect: AbilityEffect::AimedShot,
            hint: "Deals reliable bow damage before enemies reach you.",
        }),
        "disengage" => Some(AbilityDef {
            id: "disengage",
            label: "Disengage",
            hero_class: HeroClass::Ranger,
            cost_type: AbilityCostType::Stamina,
            cost: 8,
            range: 1,
            cooldown: ATTACK_COOLDOWN_SECONDS,
            required_weapon_subclass: None,
            requires_target: true,
            effect: AbilityEffect::Disengage,
            hint: "Steps one tile away from an adjacent enemy.",
        }),
        "arcane_bolt" => Some(AbilityDef {
            id: "arcane_bolt",
            label: "Arcane Bolt",
            hero_class: HeroClass::Mage,
            cost_type: AbilityCostType::Mana,
            cost: 20,
            range: 3,
            cooldown: ATTACK_COOLDOWN_SECONDS,
            required_weapon_subclass: None,
            requires_target: true,
            effect: AbilityEffect::ArcaneBolt,
            hint: "Spends mana for dependable ranged damage.",
        }),
        "ward" => Some(AbilityDef {
            id: "ward",
            label: "Ward",
            hero_class: HeroClass::Mage,
            cost_type: AbilityCostType::Mana,
            cost: 15,
            range: 0,
            cooldown: ATTACK_COOLDOWN_SECONDS,
            required_weapon_subclass: None,
            requires_target: false,
            effect: AbilityEffect::Ward,
            hint: "Raises a short defensive ward against the next hit.",
        }),
        _ => None,
    }
}

fn ability_defs_for_class(hero_class: HeroClass) -> Vec<AbilityDef> {
    HeroClassProfile::for_class(hero_class)
        .ability_ids
        .iter()
        .map(|ability_id| ability_def(ability_id).expect("class profile references ability"))
        .collect()
}

fn has_required_weapon(actor: &CombatQueryItem, required_weapon_subclass: Option<&str>) -> bool {
    let Some(required_weapon_subclass) = required_weapon_subclass else {
        return true;
    };

    actor
        .inventory
        .get_equipped_weapons()
        .iter()
        .any(|item| item.subclass == required_weapon_subclass)
}

fn ability_cost_value(actor: &CombatQueryItem, cost_type: AbilityCostType) -> i32 {
    match cost_type {
        AbilityCostType::Stamina => actor.stats.stamina.unwrap_or(0),
        AbilityCostType::Mana => actor.stats.mana.unwrap_or(0),
    }
}

fn ability_disabled_reason(
    ability: AbilityDef,
    actor: &CombatQueryItem,
    target: Option<&CombatQueryItem>,
) -> Option<String> {
    if actor.hero_class.copied() != Some(ability.hero_class) {
        return Some(format!("Requires {}", ability.hero_class.to_str()));
    }

    if !has_required_weapon(actor, ability.required_weapon_subclass) {
        return Some(format!(
            "Equip a {}",
            ability
                .required_weapon_subclass
                .unwrap_or("required weapon")
        ));
    }

    let ability_cost = ability_effective_cost(ability, actor);
    if ability_cost_value(actor, ability.cost_type) < ability_cost {
        return Some(match ability.cost_type {
            AbilityCostType::Stamina => "Not enough stamina".to_string(),
            AbilityCostType::Mana => "Not enough mana".to_string(),
        });
    }

    if ability.requires_target {
        let Some(target) = target else {
            return Some("Select a target".to_string());
        };

        if Obj::is_dead(&target.state) {
            return Some("Target is dead".to_string());
        }

        if ability_is_damaging(ability) {
            if let Some(errmsg) = Combat::non_attackable_target_error(target) {
                return Some(errmsg);
            }
        }

        if ability_is_damaging(ability)
            && Combat::target_is_fortified(target)
            && !ability_is_ranged_attack(ability)
        {
            return Some("Only ranged attacks can hit a fortified target.".to_string());
        }

        if ability_is_damaging(ability) {
            if let Some(errmsg) = Combat::fortified_outbound_attack_error_from_combat(
                actor,
                target,
                ability_is_ranged_attack(ability),
            ) {
                return Some(errmsg);
            }
        }

        if Map::dist(*actor.pos, *target.pos) > ability_effective_range(ability, actor) {
            return Some("Out of range".to_string());
        }
    }

    None
}

fn ability_is_damaging(ability: AbilityDef) -> bool {
    matches!(
        ability.effect,
        AbilityEffect::ShieldBash | AbilityEffect::AimedShot | AbilityEffect::ArcaneBolt
    )
}

fn ability_is_ranged_attack(ability: AbilityDef) -> bool {
    matches!(
        ability.effect,
        AbilityEffect::AimedShot | AbilityEffect::ArcaneBolt
    )
}

fn ability_hints_for(
    actor: &CombatQueryItem,
    target: Option<&CombatQueryItem>,
) -> Vec<network::AbilityHint> {
    let Some(hero_class) = actor.hero_class.copied() else {
        return Vec::new();
    };

    ability_defs_for_class(hero_class)
        .iter()
        .map(|ability| network::AbilityHint {
            id: ability.id.to_string(),
            label: ability.label.to_string(),
            cost_type: match ability.cost_type {
                AbilityCostType::Stamina => "stamina".to_string(),
                AbilityCostType::Mana => "mana".to_string(),
            },
            cost: ability_effective_cost(*ability, actor),
            range: ability_effective_range(*ability, actor) as i32,
            disabled_reason: ability_disabled_reason(*ability, actor, target),
            hint: ability.hint.to_string(),
        })
        .collect()
}

fn spend_ability_cost(actor: &mut CombatQueryItem, ability: AbilityDef, cost: i32) {
    match ability.cost_type {
        AbilityCostType::Stamina => {
            let stamina = actor.stats.stamina.unwrap_or(0);
            actor.stats.stamina = Some(stamina - cost);
        }
        AbilityCostType::Mana => {
            let mana = actor.stats.mana.unwrap_or(0);
            actor.stats.mana = Some(mana - cost);
        }
    }
}

fn ability_response_packet(source_id: i32, ability: AbilityDef, cost: i32) -> ResponsePacket {
    ResponsePacket::Ability {
        source_id,
        ability_id: ability.id.to_string(),
        cooldown: ability.cooldown,
        stamina_cost: match ability.cost_type {
            AbilityCostType::Stamina => Some(cost),
            AbilityCostType::Mana => None,
        },
        mana_cost: match ability.cost_type {
            AbilityCostType::Stamina => None,
            AbilityCostType::Mana => Some(cost),
        },
    }
}

fn equipped_damage(actor: &CombatQueryItem, weapon_subclass: Option<&str>) -> i32 {
    actor
        .inventory
        .get_equipped_weapons()
        .iter()
        .filter(|item| {
            weapon_subclass
                .map(|subclass| item.subclass == subclass)
                .unwrap_or(true)
        })
        .filter_map(|item| match item.attrs.get(&AttrKey::Damage) {
            Some(AttrVal::Num(value)) => Some(*value as i32),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

fn add_timed_effect(
    obj_id: i32,
    effects: &mut Effects,
    map_events: &mut MapEvents,
    game_tick: i32,
    effect: Effect,
    duration_ticks: i32,
    amplifier: f32,
) {
    let expires_at = game_tick.saturating_add(duration_ticks);
    effects.0.insert(effect.clone(), (expires_at, amplifier, 1));
    map_events.new(
        obj_id,
        expires_at,
        VisibleEvent::EffectExpiredEvent { effect },
    );
}

fn disengage_destination(attacker_pos: Position, target_pos: Position) -> Option<Position> {
    let dx = (attacker_pos.x - target_pos.x).signum();
    let dy = (attacker_pos.y - target_pos.y).signum();

    if dx == 0 && dy == 0 {
        return None;
    }

    Some(Position {
        x: attacker_pos.x + dx,
        y: attacker_pos.y + dy,
    })
}

fn apply_ability_damage(
    commands: &mut Commands,
    game_tick: &Res<GameTick>,
    actor: &mut CombatQueryItem,
    target: &mut CombatQueryItem,
    damage: i32,
) -> i32 {
    let damage = damage.max(1);
    let target_hp_before = target.stats.hp;
    let target_was_core_structure = is_live_built_human_core_structure(
        target.class_structure,
        target.class,
        target.player_id,
        *target.subclass,
        *target.state,
        false,
    );
    target.stats.hp -= damage;
    commands
        .entity(target.entity)
        .try_insert(LastDamageTick(game_tick.0));
    actor.last_combat_tick.0 = game_tick.0;
    target.last_combat_tick.0 = game_tick.0;

    if actor.player_id.0 != target.player_id.0 {
        commands
            .entity(target.entity)
            .insert(crate::obj::LastAttacker {
                id: actor.id.0,
                tick: game_tick.0,
            });
    }

    if target.stats.hp <= 0 {
        *target.state = State::Dead;
        commands
            .entity(target.entity)
            .try_insert(StateDead {
                dead_at: game_tick.0,
                killer: actor.template.0.clone(),
            })
            .try_remove::<ThinkerBuilder>();
        commands.trigger(StateChange {
            entity: target.entity,
            new_state: State::Dead,
        });
    }

    Combat::emit_crisis_combat_telemetry(
        commands,
        game_tick.0,
        actor,
        target,
        target_hp_before,
        target_was_core_structure,
    );

    damage
}

fn base_mana_for_template(hero_class: Option<HeroClass>, template: &ObjTemplate) -> i32 {
    template.base_mana.unwrap_or_else(|| {
        hero_class
            .map(|hero_class| HeroClassProfile::for_class(hero_class).base_mana)
            .unwrap_or(0)
    })
}

fn refresh_stats_from_template(
    stats: &mut Stats,
    hero_class: Option<HeroClass>,
    template: &ObjTemplate,
) {
    let base_hp = template.base_hp.unwrap_or(stats.base_hp);
    let base_mana = base_mana_for_template(hero_class, template);

    stats.hp = base_hp;
    stats.base_hp = base_hp;
    stats.stamina = template.base_stamina;
    stats.base_stamina = template.base_stamina;
    stats.mana = Some(base_mana);
    stats.base_mana = Some(base_mana);
    stats.base_def = template.base_def.unwrap_or(0);
    stats.base_damage = template.base_dmg;
    stats.damage_range = template.dmg_range;
    stats.base_speed = template.base_speed;
    stats.base_vision = template.base_vision;
}

fn send_combat_state(
    player_id: i32,
    target_id: i32,
    target_template: String,
    attack_history: Vec<String>,
    actor: &CombatQueryItem,
    target: &CombatQueryItem,
    templates: &Templates,
    clients: &Res<Clients>,
) {
    let (matching_combos, available_finisher) = combo_hints_for_history(&attack_history, templates);
    let mut target_effects = target
        .effects
        .0
        .keys()
        .cloned()
        .map(Effect::to_str)
        .collect::<Vec<_>>();
    target_effects.sort();
    let packet = ResponsePacket::CombatState {
        version: 2,
        target_id,
        enemy_intent: enemy_intent_for_template(&target_template, target.crisis_assault.is_some()),
        attack_history: attack_history.clone(),
        matching_combos,
        available_finisher,
        target_effects,
        stamina_costs: network::StaminaCosts {
            quick: 5,
            precise: 5,
            fierce: 5,
            block: 0,
        },
        abilities: ability_hints_for(actor, Some(target)),
        counter_hint: counter_hint_for_template(&target_template, &attack_history),
    };
    send_to_client(player_id, packet, clients);
}

fn combat_effects_changed_observer(
    event: On<CombatEffectsChanged>,
    clients: Res<Clients>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut query_set: ParamSet<(
        Query<(Entity, &PlayerId, &crate::combat::ComboTracker), With<SubclassHero>>,
        Query<CombatQuery>,
    )>,
) {
    let Some(target_entity) = entity_map.get_entity(event.target_id) else {
        return;
    };
    let observers = query_set
        .p0()
        .iter()
        .filter(|(_, player_id, tracker)| {
            player_id.0 < 1000 && tracker.target_id == event.target_id
        })
        .map(|(entity, player_id, tracker)| {
            (
                entity,
                player_id.0,
                tracker
                    .attacks
                    .iter()
                    .cloned()
                    .map(|attack| attack.to_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    for (actor_entity, player_id, attack_history) in observers {
        let mut combat_query = query_set.p1();
        let Ok([actor, target]) = combat_query.get_many_mut([actor_entity, target_entity]) else {
            continue;
        };
        send_combat_state(
            player_id,
            event.target_id,
            target.template.0.clone(),
            attack_history,
            &actor,
            &target,
            &templates,
            &clients,
        );
    }
}

fn attack_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    map: Res<Map>,
    player_stats: ResMut<PlayerStats>,
    mut query_set: ParamSet<(Query<CombatQuery>, Query<ObjQuery>)>,
    mut last_player_attack: Local<HashMap<i32, PlayerAttackCooldown>>,
    mut discovered_combos: Local<HashSet<(i32, String)>>,
    mut presence: ResMut<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Attack {
                player_id,
                attack_type,
                source_id,
                target_id,
            } => {
                events_to_remove.push(*event_id);

                // A socket may close after its packet reached the broker. Drop
                // queued combat input at the authoritative presence boundary so
                // disconnect cannot earn XP or crisis progress.
                if !clients.is_player_online(*player_id) {
                    continue;
                }

                let Some(attacker_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find attacker entity from id: {:?}", source_id);
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(*target_id) else {
                    error!("Cannot find target entity from id: {:?}", target_id);
                    continue;
                };

                let entities = [attacker_entity, target_entity];

                let mut combat_query = query_set.p0();
                let Ok([mut attacker, mut target]) = combat_query.get_many_mut(entities) else {
                    error!(
                        "Cannot find attacker or target from entities {:?}",
                        entities
                    );
                    continue;
                };

                if Obj::is_dead(&attacker.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot attack.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if attacker is owned by player
                if attacker.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Attacker not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                Combat::emit_crisis_attack_telemetry(
                    &mut commands,
                    game_tick.0,
                    CrisisAttackTelemetryStage::Requested,
                    &attacker,
                    &target,
                );

                let attack_profile = basic_attack_profile(&attacker);
                let target_distance = Map::dist(*attacker.pos, *target.pos);

                if target_distance > attack_profile.range {
                    let packet = ResponsePacket::Error {
                        errmsg: "Out of range.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if target is dead
                if *target.state == State::Dead {
                    let packet = ResponsePacket::Error {
                        errmsg: "Target is dead.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if let Some(errmsg) = Combat::non_attackable_target_error(&target) {
                    let packet = ResponsePacket::Error { errmsg };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !attack_profile.is_ranged {
                    if let Some(errmsg) = Combat::fortified_target_melee_error(&target) {
                        let packet = ResponsePacket::Error { errmsg };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }
                }

                if let Some(errmsg) = Combat::fortified_outbound_attack_error_from_combat(
                    &attacker,
                    &target,
                    attack_profile.is_ranged
                        || Combat::equipped_weapon_has_fortification_reach(&attacker.inventory),
                ) {
                    let packet = ResponsePacket::Error { errmsg };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if attacker has enough stamina
                let attacker_stamina = attacker.stats.stamina.expect("Missing stamina stat");
                if attacker_stamina < attack_profile.stamina_cost {
                    let packet = ResponsePacket::Error {
                        errmsg: "Not enough stamina to attack.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check global attack cooldown (per-player, not affected by being attacked)
                if attack_is_on_cooldown(&last_player_attack, *player_id, game_tick.0) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Attack is on cooldown.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Final authoritative mutation boundary. Keep this check next
                // to stamina/HP/combat-state mutation even though the ordered
                // input guard normally rejected the event earlier.
                if object_belongs_to_protected_run(attacker.id.0, &ids, &presence)
                    || object_belongs_to_protected_run(target.id.0, &ids, &presence)
                {
                    continue;
                }

                Combat::emit_crisis_attack_telemetry(
                    &mut commands,
                    game_tick.0,
                    CrisisAttackTelemetryStage::Accepted,
                    &attacker,
                    &target,
                );

                let attack_type_enum = Combat::attack_type_to_enum(attack_type.to_string());
                let previous_attacks = Combat::live_combo_attacks_before_append(
                    attacker.combo_tracker.as_deref(),
                    target.id.0,
                    game_tick.0,
                );
                let mut live_attacks = Combat::next_combo_attacks(
                    &previous_attacks,
                    attack_type_enum.clone(),
                    &templates,
                );
                let mut attack_history = live_attacks
                    .iter()
                    .cloned()
                    .map(|attack| attack.to_str())
                    .collect::<Vec<_>>();
                let target_template = target.template.0.clone();

                if attack_profile.is_ranged && !ranged_attack_hits(attack_profile, target_distance)
                {
                    // A miss does not land a chain step. Keep the UI aligned
                    // with the authoritative tracker instead of previewing the
                    // suffix that would have been recorded on a hit.
                    live_attacks = previous_attacks;
                    attack_history = live_attacks
                        .iter()
                        .cloned()
                        .map(|attack| attack.to_str())
                        .collect();
                    attacker.stats.stamina = Some(attacker_stamina - attack_profile.stamina_cost);
                    attacker.last_combat_tick.0 = game_tick.0;
                    record_attack_cooldown(
                        &mut last_player_attack,
                        *player_id,
                        game_tick.0,
                        ATTACK_COOLDOWN_TICKS,
                    );
                    record_player_combat_activity(*player_id, game_tick.0, &mut presence);

                    Combat::add_damage_event(
                        game_tick.0,
                        attack_type.to_string(),
                        0,
                        None,
                        true,
                        &attacker,
                        &target,
                        &mut map_events,
                    );

                    let packet = ResponsePacket::Attack {
                        source_id: *source_id,
                        attack_type: attack_type.clone(),
                        cooldown: cooldown_seconds(ATTACK_COOLDOWN_TICKS),
                        stamina_cost: attack_profile.stamina_cost,
                    };

                    send_to_client(*player_id, packet, &clients);
                    send_combat_state(
                        *player_id,
                        *target_id,
                        target_template,
                        attack_history,
                        &attacker,
                        &target,
                        &templates,
                        &clients,
                    );
                    continue;
                }

                // Calculate and process damage
                let (damage, combo, skill_updated, countered) = Combat::process_attack_with_options(
                    attack_type_enum,
                    &mut attacker,
                    &mut target,
                    &mut commands,
                    &templates,
                    &map,
                    &mut ids,
                    &game_tick,
                    &mut map_events,
                    AttackOptions {
                        stamina_cost: attack_profile.stamina_cost,
                        damage_bonus: attack_profile.damage_bonus,
                    },
                );
                if countered.is_some() {
                    live_attacks.clear();
                    attack_history.clear();
                }

                // Add visible damage event to broadcast to everyone nearby
                Combat::add_damage_event(
                    game_tick.0,
                    attack_type.to_string(),
                    damage,
                    combo,
                    false,
                    &attacker,
                    &target,
                    &mut map_events,
                );

                // Track player attack cooldown
                attacker.last_combat_tick.0 = game_tick.0;
                let tempo_prefix_len = combo_tempo_prefix_len(&live_attacks, &templates);
                let effective_cooldown = combo_chain_cooldown_ticks(tempo_prefix_len);
                record_attack_cooldown(
                    &mut last_player_attack,
                    *player_id,
                    game_tick.0,
                    effective_cooldown,
                );
                record_player_combat_activity(*player_id, game_tick.0, &mut presence);

                // Response to client with attack response packet
                let packet = ResponsePacket::Attack {
                    source_id: *source_id,
                    attack_type: attack_type.clone(),
                    cooldown: cooldown_seconds(effective_cooldown),
                    stamina_cost: attack_profile.stamina_cost,
                };

                send_to_client(*player_id, packet, &clients);
                send_combat_state(
                    *player_id,
                    *target_id,
                    target_template,
                    attack_history,
                    &attacker,
                    &target,
                    &templates,
                    &clients,
                );

                // Update skill
                if let Some(skill_updated) = skill_updated {
                    if let Some(ref mut attacker_skills) = attacker.skills {
                        match Skill::from_str(&skill_updated.xp_type) {
                            Some(skill_name) => {
                                attacker_skills.update(
                                    skill_name,
                                    skill_updated.xp,
                                    &templates.skill_templates,
                                );
                            }
                            None => {
                                warn!(
                                    "No combat skill mapped for weapon subclass '{}', skipping XP gain",
                                    skill_updated.xp_type
                                );
                            }
                        }
                    }
                }
            }
            PlayerEvent::Ability {
                player_id,
                ability_id,
                source_id,
                target_id,
            } => {
                events_to_remove.push(*event_id);

                if !clients.is_player_online(*player_id) {
                    continue;
                }

                let Some(ability) = ability_def(ability_id) else {
                    let packet = ResponsePacket::Error {
                        errmsg: "Unknown ability.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                let Some(attacker_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find ability source entity from id: {:?}", source_id);
                    continue;
                };

                if attack_is_on_cooldown(&last_player_attack, *player_id, game_tick.0) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Ability is on cooldown.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !ability.requires_target {
                    let mut combat_query = query_set.p0();
                    let Ok(mut attacker) = combat_query.get_mut(attacker_entity) else {
                        error!("Cannot find ability source entity {:?}", attacker_entity);
                        continue;
                    };

                    if attacker.player_id.0 != *player_id {
                        let packet = ResponsePacket::Error {
                            errmsg: "Ability source not owned by player.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }

                    if Obj::is_dead(&attacker.state) {
                        let packet = ResponsePacket::Error {
                            errmsg: "The dead cannot use abilities.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }

                    if let Some(reason) = ability_disabled_reason(ability, &attacker, None) {
                        let packet = ResponsePacket::Error { errmsg: reason };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }

                    if object_belongs_to_protected_run(attacker.id.0, &ids, &presence) {
                        continue;
                    }

                    let ability_cost = ability_effective_cost(ability, &attacker);
                    spend_ability_cost(&mut attacker, ability, ability_cost);
                    match ability.effect {
                        AbilityEffect::Ward => {
                            add_timed_effect(
                                attacker.id.0,
                                &mut attacker.effects,
                                &mut map_events,
                                game_tick.0,
                                Effect::Sanctuary,
                                MAGE_WARD_DURATION_TICKS,
                                MAGE_WARD_AMPLIFIER,
                            );
                            attacker.last_combat_tick.0 = game_tick.0;
                        }
                        _ => {}
                    }

                    record_attack_cooldown(
                        &mut last_player_attack,
                        *player_id,
                        game_tick.0,
                        ATTACK_COOLDOWN_TICKS,
                    );
                    send_to_client(
                        *player_id,
                        ability_response_packet(*source_id, ability, ability_cost),
                        &clients,
                    );
                    continue;
                }

                let Some(target_id) = target_id else {
                    let packet = ResponsePacket::Error {
                        errmsg: "Select a target for that ability.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(*target_id) else {
                    error!("Cannot find ability target entity from id: {:?}", target_id);
                    continue;
                };

                if ability.effect == AbilityEffect::Disengage {
                    let obj_query = query_set.p1();
                    let (Ok(attacker), Ok(target)) =
                        (obj_query.get(attacker_entity), obj_query.get(target_entity))
                    else {
                        error!(
                            "Cannot find ability source or target for retreat precheck {:?}",
                            [attacker_entity, target_entity]
                        );
                        continue;
                    };

                    let Some(dst) = disengage_destination(*attacker.pos, *target.pos) else {
                        let packet = ResponsePacket::Error {
                            errmsg: "No open retreat tile.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    };

                    if !Map::is_valid_pos((dst.x, dst.y))
                        || !Map::is_passable_by_obj(dst.x, dst.y, true, false, false, &map)
                        || !is_pos_empty(*player_id, dst.x, dst.y, &obj_query)
                    {
                        let packet = ResponsePacket::Error {
                            errmsg: "No open retreat tile.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }
                }

                let entities = [attacker_entity, target_entity];
                let mut combat_query = query_set.p0();
                let Ok([mut attacker, mut target]) = combat_query.get_many_mut(entities) else {
                    error!(
                        "Cannot find ability source or target from entities {:?}",
                        entities
                    );
                    continue;
                };

                if attacker.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Ability source not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if Obj::is_dead(&attacker.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot use abilities.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if ability_is_damaging(ability) {
                    Combat::emit_crisis_attack_telemetry(
                        &mut commands,
                        game_tick.0,
                        CrisisAttackTelemetryStage::Requested,
                        &attacker,
                        &target,
                    );
                }

                if let Some(reason) = ability_disabled_reason(ability, &attacker, Some(&target)) {
                    let packet = ResponsePacket::Error { errmsg: reason };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Final boundary before ability cost, effects, movement, or
                // HP mutation. This includes neutral run objects and the bound
                // monolith exception through the canonical object helper.
                if object_belongs_to_protected_run(attacker.id.0, &ids, &presence)
                    || object_belongs_to_protected_run(target.id.0, &ids, &presence)
                {
                    continue;
                }

                if ability_is_damaging(ability) {
                    Combat::emit_crisis_attack_telemetry(
                        &mut commands,
                        game_tick.0,
                        CrisisAttackTelemetryStage::Accepted,
                        &attacker,
                        &target,
                    );
                }

                let target_template = target.template.0.clone();
                let ability_cost = ability_effective_cost(ability, &attacker);

                match ability.effect {
                    AbilityEffect::ShieldBash => {
                        let damage_amount = 3 + attacker.stats.base_damage.unwrap_or(0);
                        spend_ability_cost(&mut attacker, ability, ability_cost);
                        let damage = apply_ability_damage(
                            &mut commands,
                            &game_tick,
                            &mut attacker,
                            &mut target,
                            damage_amount,
                        );
                        add_timed_effect(
                            target.id.0,
                            &mut target.effects,
                            &mut map_events,
                            game_tick.0,
                            Effect::Stunned,
                            GUARD_BASH_STUN_TICKS,
                            1.0,
                        );
                        add_timed_effect(
                            attacker.id.0,
                            &mut attacker.effects,
                            &mut map_events,
                            game_tick.0,
                            Effect::Bracing,
                            WARRIOR_BRACE_DURATION_TICKS,
                            WARRIOR_BRACE_AMPLIFIER,
                        );
                        Combat::add_damage_event(
                            game_tick.0,
                            "Guard Bash".to_string(),
                            damage,
                            None,
                            false,
                            &attacker,
                            &target,
                            &mut map_events,
                        );
                    }
                    AbilityEffect::AimedShot => {
                        let distance = Map::dist(*attacker.pos, *target.pos);
                        let Some(profile) = aimed_shot_profile(&attacker) else {
                            let packet = ResponsePacket::Error {
                                errmsg: "Equip a Bow".to_string(),
                            };
                            send_to_client(*player_id, packet, &clients);
                            continue;
                        };

                        spend_ability_cost(&mut attacker, ability, ability_cost);
                        if ranged_attack_hits(profile, distance) {
                            let damage_amount = 4
                                + attacker.stats.base_damage.unwrap_or(0)
                                + equipped_damage(&attacker, Some("Bow"))
                                + profile.damage_bonus;
                            let damage = apply_ability_damage(
                                &mut commands,
                                &game_tick,
                                &mut attacker,
                                &mut target,
                                damage_amount,
                            );
                            Combat::add_damage_event(
                                game_tick.0,
                                "Aimed Shot".to_string(),
                                damage,
                                None,
                                false,
                                &attacker,
                                &target,
                                &mut map_events,
                            );
                        } else {
                            attacker.last_combat_tick.0 = game_tick.0;
                            Combat::add_damage_event(
                                game_tick.0,
                                "Aimed Shot".to_string(),
                                0,
                                None,
                                true,
                                &attacker,
                                &target,
                                &mut map_events,
                            );
                        }
                    }
                    AbilityEffect::Disengage => {
                        let Some(dst) = disengage_destination(*attacker.pos, *target.pos) else {
                            let packet = ResponsePacket::Error {
                                errmsg: "No open retreat tile.".to_string(),
                            };
                            send_to_client(*player_id, packet, &clients);
                            continue;
                        };

                        if !Map::is_valid_pos((dst.x, dst.y))
                            || !Map::is_passable_by_obj(dst.x, dst.y, true, false, false, &map)
                        {
                            let packet = ResponsePacket::Error {
                                errmsg: "No open retreat tile.".to_string(),
                            };
                            send_to_client(*player_id, packet, &clients);
                            continue;
                        }

                        spend_ability_cost(&mut attacker, ability, ability_cost);
                        commands.trigger(StateChange {
                            entity: attacker.entity,
                            new_state: State::Moving,
                        });
                        map_events.new(
                            attacker.id.0,
                            game_tick.0 + 6,
                            VisibleEvent::MoveEvent {
                                src: *attacker.pos,
                                dst,
                            },
                        );
                        attacker.last_combat_tick.0 = game_tick.0;
                    }
                    AbilityEffect::ArcaneBolt => {
                        spend_ability_cost(&mut attacker, ability, ability_cost);
                        map_events.new(
                            attacker.id.0,
                            game_tick.0,
                            VisibleEvent::SpellDamageEvent {
                                spell: Spell::ArcaneBolt,
                                target_id: *target_id,
                            },
                        );
                        attacker.last_combat_tick.0 = game_tick.0;
                        target.last_combat_tick.0 = game_tick.0;
                    }
                    AbilityEffect::Ward => {}
                }

                record_attack_cooldown(
                    &mut last_player_attack,
                    *player_id,
                    game_tick.0,
                    ATTACK_COOLDOWN_TICKS,
                );
                if ability_is_damaging(ability) {
                    record_player_combat_activity(*player_id, game_tick.0, &mut presence);
                }
                send_to_client(
                    *player_id,
                    ability_response_packet(*source_id, ability, ability_cost),
                    &clients,
                );
                send_combat_state(
                    *player_id,
                    *target_id,
                    target_template,
                    Vec::new(),
                    &attacker,
                    &target,
                    &templates,
                    &clients,
                );
            }
            PlayerEvent::Combo {
                player_id,
                source_id,
                target_id,
                combo_type: _,
            } => {
                events_to_remove.push(*event_id);

                if !clients.is_player_online(*player_id) {
                    continue;
                }

                let Some(attacker_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find attacker entity from id: {:?}", source_id);
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(*target_id) else {
                    error!("Cannot find target entity from id: {:?}", target_id);
                    continue;
                };

                let entities = [attacker_entity, target_entity];

                let adjacent_npc_entities = {
                    let obj_query = query_set.p1();
                    let Ok(attacker_obj) = obj_query.get(attacker_entity) else {
                        continue;
                    };
                    obj_query
                        .iter()
                        .filter(|candidate| {
                            candidate.entity != target_entity
                                && candidate.player_id.0 >= 1000
                                && candidate.class.0 == CLASS_UNIT
                                && *candidate.state != State::Dead
                                && Map::dist(*attacker_obj.pos, *candidate.pos) <= 1
                        })
                        .map(|candidate| candidate.entity)
                        .collect::<Vec<_>>()
                };

                let mut combat_query = query_set.p0();
                let Ok([mut attacker, mut target]) = combat_query.get_many_mut(entities) else {
                    error!(
                        "Cannot find attacker or target from entities {:?}",
                        entities
                    );
                    continue;
                };

                if Obj::is_dead(&attacker.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot attack.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if attacker is owned by player
                if attacker.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Attacker not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                Combat::emit_crisis_attack_telemetry(
                    &mut commands,
                    game_tick.0,
                    CrisisAttackTelemetryStage::Requested,
                    &attacker,
                    &target,
                );

                // Is target adjacent
                if Map::dist(*attacker.pos, *target.pos) > 1 {
                    let packet = ResponsePacket::Error {
                        errmsg: "Target is not adjacent.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if target is dead
                if *target.state == State::Dead {
                    let packet = ResponsePacket::Error {
                        errmsg: "Target is dead.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if let Some(errmsg) = Combat::non_attackable_target_error(&target) {
                    let packet = ResponsePacket::Error { errmsg };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if let Some(errmsg) = Combat::fortified_target_melee_error(&target) {
                    let packet = ResponsePacket::Error { errmsg };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if let Some(errmsg) = Combat::fortified_outbound_attack_error_from_combat(
                    &attacker,
                    &target,
                    Combat::equipped_weapon_has_fortification_reach(&attacker.inventory),
                ) {
                    let packet = ResponsePacket::Error { errmsg };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if attacker has enough stamina
                let attacker_stamina = attacker.stats.stamina.expect("Missing stamina stat");
                if attacker_stamina < 5 {
                    let packet = ResponsePacket::Error {
                        errmsg: "Not enough stamina to attack.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let attack_history = live_combo_history_for_target(
                    attacker.combo_tracker.as_deref(),
                    target.id.0,
                    game_tick.0,
                );
                let (_matching_combos, available_finisher) =
                    combo_hints_for_history(&attack_history, &templates);
                let basic_attack_cooldown_active =
                    attack_is_on_cooldown(&last_player_attack, *player_id, game_tick.0);
                if let Some(errmsg) = combo_finisher_rejection(
                    available_finisher.as_deref(),
                    basic_attack_cooldown_active,
                ) {
                    let packet = ResponsePacket::Error {
                        errmsg: errmsg.to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if object_belongs_to_protected_run(attacker.id.0, &ids, &presence)
                    || object_belongs_to_protected_run(target.id.0, &ids, &presence)
                {
                    continue;
                }

                Combat::emit_crisis_attack_telemetry(
                    &mut commands,
                    game_tick.0,
                    CrisisAttackTelemetryStage::Accepted,
                    &attacker,
                    &target,
                );

                let target_template = target.template.0.clone();

                // Intentionally no ATTACK_COOLDOWN_TICKS gate here: a ready
                // finisher fires immediately after the chain's final basic
                // attack. Executing it still starts the next basic cooldown.
                // Calculate and process damage
                let (damage, combo, skill_updated) = Combat::process_combo(
                    &mut attacker,
                    &mut target,
                    &mut commands,
                    &templates,
                    &map,
                    &mut ids,
                    &game_tick,
                    &mut map_events,
                );

                debug!("Found combo: {:?}", combo);

                // Add visible damage event to broadcast to everyone nearby
                Combat::add_damage_event(
                    game_tick.0,
                    "combo".to_string(),
                    damage,
                    combo.clone(),
                    false,
                    &attacker,
                    &target,
                    &mut map_events,
                );

                // Track player attack cooldown
                record_attack_cooldown(
                    &mut last_player_attack,
                    *player_id,
                    game_tick.0,
                    ATTACK_COOLDOWN_TICKS,
                );
                record_player_combat_activity(*player_id, game_tick.0, &mut presence);

                // Response to client with attack response packet
                let packet = ResponsePacket::Attack {
                    source_id: *source_id,
                    attack_type: "combo".to_string(),
                    cooldown: cooldown_seconds(ATTACK_COOLDOWN_TICKS),
                    stamina_cost: 5,
                };

                send_to_client(*player_id, packet, &clients);
                send_combat_state(
                    *player_id,
                    *target_id,
                    target_template,
                    Vec::new(),
                    &attacker,
                    &target,
                    &templates,
                    &clients,
                );

                if let Some(combo_name) = combo.clone().filter(|combo_name| {
                    is_first_combo_discovery(&mut discovered_combos, *source_id, combo_name)
                }) {
                    let discovery_packet = ResponsePacket::DiscoveryEvent {
                        version: 1,
                        discovery_type: "combat".to_string(),
                        title: format!("Combo landed: {}", combo_name),
                        unlock_source: "Combat pattern".to_string(),
                        location: None,
                        result: "You completed an attack sequence. Combos are learned patterns: repeat the sequence when the same problem appears.".to_string(),
                    };
                    send_to_client(*player_id, discovery_packet, &clients);
                }

                debug!("Skill gain: {:?}", skill_updated);

                // Update skill
                if let Some(skill_updated) = skill_updated {
                    if let Some(ref mut attacker_skills) = attacker.skills {
                        match Skill::from_str(&skill_updated.xp_type) {
                            Some(skill_name) => {
                                attacker_skills.update(
                                    skill_name,
                                    skill_updated.xp,
                                    &templates.skill_templates,
                                );
                            }
                            None => {
                                warn!(
                                    "No combat skill mapped for weapon subclass '{}', skipping XP gain",
                                    skill_updated.xp_type
                                );
                            }
                        }
                    }
                }

                let combo_area = combo.as_deref().and_then(|combo_name| {
                    Combat::combo_area_profile(combo_name).and_then(|profile| {
                        templates
                            .combo_templates
                            .get(combo_name)
                            .cloned()
                            .map(|template| (combo_name.to_string(), template, profile))
                    })
                });
                drop(attacker);
                drop(target);
                drop(combat_query);

                if let Some((combo_name, combo_template, profile)) = combo_area {
                    for secondary_entity in adjacent_npc_entities {
                        let mut combat_query = query_set.p0();
                        let Ok([mut secondary_attacker, mut secondary]) =
                            combat_query.get_many_mut([attacker_entity, secondary_entity])
                        else {
                            continue;
                        };
                        if !Combat::valid_combo_secondary(
                            *secondary_attacker.pos,
                            *target_id,
                            &secondary,
                        ) || object_belongs_to_protected_run(secondary.id.0, &ids, &presence)
                        {
                            continue;
                        }

                        let (secondary_damage, secondary_skill) = Combat::process_combo_secondary(
                            &mut secondary_attacker,
                            &mut secondary,
                            &combo_template,
                            &profile,
                            &mut commands,
                            &templates,
                            &map,
                            &game_tick,
                            &mut map_events,
                        );
                        Combat::add_damage_event(
                            game_tick.0,
                            "combo".to_string(),
                            secondary_damage,
                            Some(combo_name.clone()),
                            false,
                            &secondary_attacker,
                            &secondary,
                            &mut map_events,
                        );

                        if let (Some(skill_updated), Some(mut attacker_skills)) =
                            (secondary_skill, secondary_attacker.skills)
                        {
                            if let Some(skill_name) = Skill::from_str(&skill_updated.xp_type) {
                                attacker_skills.update(
                                    skill_name,
                                    skill_updated.xp,
                                    &templates.skill_templates,
                                );
                            }
                        }
                    }
                }

                /*let Some(attacker_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find attacker entity from id: {:?}", source_id);
                    continue;
                };

                let Ok(attacker) = query.get_mut(attacker_entity) else {
                    error!("Cannot find attacker entity {:?}", attacker_entity);
                    continue;
                };

                // Check if attacker is owned by player
                if attacker.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Attacker not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if let Some(mut combo_tracker) = attacker.combo_tracker {
                    combo_tracker.attacks.clear();
                    combo_tracker.target_id = -1;
                }*/
            }
            PlayerEvent::Block {
                player_id,
                source_id,
                defense,
            } => {
                events_to_remove.push(*event_id);

                if !clients.is_player_online(*player_id) {
                    continue;
                }

                let Some(attacker_entity) = entity_map.get_entity(*source_id) else {
                    continue;
                };

                let mut combat_query = query_set.p0();
                let Ok(mut attacker) = combat_query.get_mut(attacker_entity) else {
                    continue;
                };

                if attacker.player_id.0 != *player_id {
                    continue;
                }

                if Obj::is_dead(&attacker.state) {
                    continue;
                }

                // Check cooldown
                if attack_is_on_cooldown(&last_player_attack, *player_id, game_tick.0) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Attack is on cooldown.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // BB-A: pick the stance matching the chosen defense.
                // dodge -> Quick, parry -> Precise, brace -> Fierce.
                let stance = match defense.as_str() {
                    "dodge" => Effect::Dodging,
                    "parry" => Effect::Parrying,
                    _ => Effect::Bracing,
                };

                // Only one stance at a time.
                attacker.effects.0.remove(&Effect::Bracing);
                attacker.effects.0.remove(&Effect::Dodging);
                attacker.effects.0.remove(&Effect::Parrying);

                // Brace is the generalist: warriors get a longer, stronger Iron
                // Stance and recover a little stamina.
                let (stance_duration, stance_amp) = if stance == Effect::Bracing
                    && matches!(attacker.hero_class, Some(&HeroClass::Warrior))
                {
                    if let (Some(stamina), Some(base_stamina)) =
                        (attacker.stats.stamina, attacker.stats.base_stamina)
                    {
                        attacker.stats.stamina = Some((stamina + 3).min(base_stamina));
                    }
                    (WARRIOR_BRACE_DURATION_TICKS, WARRIOR_BRACE_AMPLIFIER)
                } else {
                    (STANDARD_BRACE_DURATION_TICKS, STANDARD_BRACE_AMPLIFIER)
                };

                add_timed_effect(
                    attacker.id.0,
                    &mut attacker.effects,
                    &mut map_events,
                    game_tick.0,
                    stance,
                    stance_duration,
                    stance_amp,
                );

                record_attack_cooldown(
                    &mut last_player_attack,
                    *player_id,
                    game_tick.0,
                    ATTACK_COOLDOWN_TICKS,
                );

                let packet = ResponsePacket::Attack {
                    source_id: *source_id,
                    attack_type: "block".to_string(),
                    cooldown: cooldown_seconds(ATTACK_COOLDOWN_TICKS),
                    stamina_cost: 0,
                };

                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn gather_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: ResMut<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    campfire_visibility: Res<CampfireVisibilityState>,
    mut map_events: ResMut<MapEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    mut game_events: ResMut<GameEvents>,
    resources: Res<Resources>,
    discoveries: Res<ResourceDiscoveries>,
    mut hero_query: Query<(
        &Position,
        &State,
        &mut Inventory,
        Option<&LastCombatTick>,
        Option<&mut ActiveTask>,
        Option<&Viewshed>,
    )>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Gather { player_id } => {
                debug!("PlayerEvent::Gather");
                events_to_remove.push(*event_id);

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok((
                    hero_pos,
                    hero_state,
                    hero_inventory,
                    last_combat_tick,
                    hero_active_task,
                    hero_viewshed,
                )) = hero_query.get_mut(hero_entity)
                else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                if Obj::is_dead(&hero_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot gather".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !has_sufficient_work_visibility(hero_viewshed, *player_id, &campfire_visibility)
                {
                    send_insufficient_work_visibility_notice(*player_id, &clients);
                    continue;
                }

                if combat_locked(last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                // Map equipped main-hand tool to its preferred resource type. The tool
                // gets priority *if* its resource is actually on the tile; otherwise we
                // fall through to plant-picking / forage so the player isn't forced to
                // unequip just to grab grapes or berries with a sword in hand.
                let tool_resource_type = hero_inventory.get_equipped_main_hand().and_then(|tool| {
                    item::gather_resource_type_for_tool(&tool).map(str::to_string)
                });
                let hunting_tool_equipped =
                    hero_inventory.has_equipped_tool_for_attr(&item::AttrKey::Hunting);

                // Decide what to do, in priority order:
                //   1. Tool's preferred resource is on the tile -> gather that.
                //   2. A Plant resource (grapes, berries) is on the tile -> pick it.
                //   3. Nothing specific -> terrain-based forage.
                let event_type = if let Some(rt) = tool_resource_type.filter(|rt| {
                    Resource::is_valid_type_for_player(
                        rt.clone(),
                        *hero_pos,
                        &resources,
                        &discoveries,
                        *player_id,
                    )
                }) {
                    GameEventType::GatherEvent {
                        gatherer_id: hero_id,
                        res_type: rt,
                    }
                } else if Resource::is_valid_type_for_player(
                    PLANT.to_string(),
                    *hero_pos,
                    &resources,
                    &discoveries,
                    *player_id,
                ) {
                    GameEventType::GatherEvent {
                        gatherer_id: hero_id,
                        res_type: PLANT.to_string(),
                    }
                } else {
                    GameEventType::ForageEvent {
                        forager_id: hero_id,
                    }
                };

                let gather_duration = match &event_type {
                    GameEventType::GatherEvent { res_type, .. } => hero_inventory
                        .get_equipped_tool_for_res_type(res_type)
                        .and_then(|tool| {
                            item::required_tool_attr_for_res_type(res_type)
                                .map(|attr| tool.attr_num(&attr))
                        })
                        .map(|rating| item::gather_duration_ticks(GATHER_TIME_SEC, rating))
                        .unwrap_or(GATHER_TIME_SEC * TICKS_PER_SEC),
                    _ => GATHER_TIME_SEC * TICKS_PER_SEC,
                };

                let gather_activity = gather_activity_for_event(&event_type, hunting_tool_equipped);
                if let Some(mut active_task) = hero_active_task {
                    ActiveTask::set_if_changed(&mut active_task, gather_activity.clone());
                } else {
                    commands.entity(hero_entity).insert(gather_activity.clone());
                }

                let activity = gather_activity.to_string();
                visible_events.new(
                    hero_id,
                    game_tick.0,
                    VisibleEvent::UpdateObjEvent {
                        attrs: vec![("activity".to_string(), activity.clone())],
                    },
                );
                send_to_client(
                    *player_id,
                    ResponsePacket::InfoActivityUpdate {
                        id: hero_id,
                        activity,
                    },
                    &clients,
                );

                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Gathering,
                });

                let event = GameEvent {
                    event_id: ids.new_map_event_id(),
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + gather_duration,
                    event_type,
                };

                game_events.insert(event.event_id, event);

                let packet = ResponsePacket::Gather {
                    gather_time: GATHER_TIME_SEC,
                };
                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn gather_activity_for_event(
    event_type: &GameEventType,
    hunting_tool_equipped: bool,
) -> ActiveTask {
    let activity = match event_type {
        GameEventType::GatherEvent { res_type, .. } => {
            let task = ActiveTask::get_activity_from_res_type(res_type.clone());
            if task == ActiveTask::Unknown {
                ActiveTask::Gathering
            } else {
                task
            }
        }
        GameEventType::ForageEvent { .. } => ActiveTask::Gathering,
        _ => ActiveTask::Gathering,
    };

    // A hunt can begin on a tile where no hunting ground is currently revealed.
    // That uses the terrain-forage fallback internally, but it is still a hunt
    // from the player's perspective while a Hunting tool is equipped.
    if activity == ActiveTask::Gathering && hunting_tool_equipped {
        ActiveTask::Hunting
    } else {
        activity
    }
}

fn gather_farm_refine_craft_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: ResMut<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut map_events: ResMut<MapEvents>,
    game_events: ResMut<GameEvents>,
    resources: Res<Resources>,
    discoveries: Res<ResourceDiscoveries>,
    templates: Res<Templates>,
    recipes: Res<Recipes>,
    active_infos: ResMut<ActiveInfos>,
    mut visible_events: ResMut<VisibleEvents>,
    hero_query: Query<CoreQuery, With<SubclassHero>>,
    structure_query: Query<StructureQuery, With<ClassStructure>>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::NearbyResources { player_id } => {
                debug!("PlayerEvent::NearbyResources");
                events_to_remove.push(*event_id);

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok(hero) = hero_query.get(hero_entity) else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                let nearby_resources =
                    Resource::get_nearby_resources(*hero.pos, &resources, &discoveries, *player_id);

                let nearby_resources_packet = ResponsePacket::NearbyResources {
                    data: nearby_resources,
                };

                send_to_client(*player_id, nearby_resources_packet, &clients);
            }
            PlayerEvent::Plant {
                player_id,
                structure_id,
            } => {
                debug!("PlayerEvent::Plant");
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok(hero) = hero_query.get(hero_entity) else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!(
                        "Cannot find structure entity for structure {:?}",
                        structure_id
                    );
                    continue;
                };

                if Obj::is_dead(&hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot plant.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Ok(structure) = structure_query.get(structure_entity) else {
                    error!("Cannot find structure from entity: {:?}", structure_entity);
                    continue;
                };

                if structure.player_id.0 != *player_id {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if structure contains seeds
                if !structure.inventory.has_by_class(item::SEEDS.to_string()) {
                    trace!("Insufficient seeds in farm to plant");
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient seeds in farm to plant".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    break;
                }

                //Planting state change
                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Planting,
                });

                let plant_event = VisibleEvent::PlantEvent {
                    structure_id: structure.id.0,
                };

                map_events.new(
                    hero.id.0,
                    game_tick.0 + 100, // in the future
                    plant_event,
                );
            }
            PlayerEvent::Harvest {
                player_id,
                structure_id,
            } => {
                debug!("PlayerEvent::Harvest");
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok(hero) = hero_query.get(hero_entity) else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!(
                        "Cannot find structure entity for structure {:?}",
                        structure_id
                    );
                    continue;
                };

                if Obj::is_dead(&hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot harvest.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Ok(structure) = structure_query.get(structure_entity) else {
                    error!("Cannot find structure from entity: {:?}", structure_entity);
                    continue;
                };

                if structure.player_id.0 != *player_id {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Some(tool) = hero.inventory.get_equipped_tool_for_attr(&AttrKey::Farming)
                else {
                    trace!("Require a harvesting tool to harvest the crop.");
                    let packet = ResponsePacket::Error {
                        errmsg: "Equip a Farming tool to harvest the crop.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    break;
                };
                let Some(work_duration) = farm_harvest_duration_ticks(hero.inventory) else {
                    continue;
                };

                //Harvesting state change
                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Harvesting,
                });

                let action_id = ids.new_map_event_id();
                commands.entity(hero_entity).insert(ActionProgress {
                    action_id,
                    start_tick: game_tick.0,
                    end_tick: game_tick.0 + work_duration,
                });
                visible_events.new(
                    hero.id.0,
                    game_tick.0,
                    VisibleEvent::UpdateObjEvent {
                        attrs: vec![
                            ("state".to_string(), STATE_HARVESTING.to_string()),
                            ("action_id".to_string(), action_id.to_string()),
                            (
                                "action_duration_ms".to_string(),
                                (work_duration.saturating_mul(1000) / TICKS_PER_SEC).to_string(),
                            ),
                            ("action_elapsed_ms".to_string(), "0".to_string()),
                        ],
                    },
                );

                let plant_event = VisibleEvent::HarvestEvent {
                    structure_id: structure.id.0,
                    tool_item_id: tool.id,
                };

                map_events.new(hero.id.0, game_tick.0 + work_duration, plant_event);
            }
            PlayerEvent::Operate {
                player_id,
                structure_id,
            } => {
                debug!("PlayerEvent::Operate");
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok(hero) = hero_query.get(hero_entity) else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!(
                        "Cannot find structure entity for structure {:?}",
                        structure_id
                    );
                    continue;
                };

                if Obj::is_dead(&hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot operate.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Ok(structure) = structure_query.get(structure_entity) else {
                    error!("Cannot find structure from entity: {:?}", structure_entity);
                    continue;
                };

                if structure.player_id.0 != *player_id {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if hero and structure are on the same pos
                if hero.pos.x != structure.pos.x || hero.pos.y != structure.pos.y {
                    error!("Hero must be on structure to operate");
                    let packet = ResponsePacket::Error {
                        errmsg: "Must be on structure to operate".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Operating,
                });

                let operate_event = VisibleEvent::OperateEvent {
                    structure_id: *structure_id,
                };

                map_events.new(
                    hero.id.0,
                    game_tick.0 + 40, // in the future
                    operate_event,
                );
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn refine_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: ResMut<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    mut visible_events: ResMut<VisibleEvents>,
    templates: Res<Templates>,
    recipes: Res<Recipes>,
    mut active_infos: ResMut<ActiveInfos>,
    campfire_visibility: Res<CampfireVisibilityState>,
    hero_query: Query<(
        &Position,
        &State,
        &mut Inventory,
        &mut Skills,
        Option<&LastCombatTick>,
        Option<&Viewshed>,
    )>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Refine { player_id, item_id } => {
                debug!("PlayerEvent::Refine");
                events_to_remove.push(*event_id);

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok((
                    _hero_pos,
                    hero_state,
                    hero_inventory,
                    hero_skills,
                    last_combat_tick,
                    hero_viewshed,
                )) = hero_query.get(hero_entity)
                else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                if Obj::is_dead(&hero_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot refine.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !has_sufficient_work_visibility(hero_viewshed, *player_id, &campfire_visibility)
                {
                    send_insufficient_work_visibility_notice(*player_id, &clients);
                    continue;
                }

                if combat_locked(last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                // Get item to refine
                let Some(item) = hero_inventory.get_by_id(*item_id) else {
                    error!("Cannot find item for {:?}", item_id);
                    continue;
                };

                if *hero_state == State::Refining {
                    let packet = ResponsePacket::Error {
                        errmsg: "Already refining".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Get item template
                let item_template = Item::get_template(item.name, &templates.item_templates);

                // Check if hero has the required refine skill level
                let refine_skill = item_template
                    .refine_skill
                    .clone()
                    .expect("Missing refine skill");
                let refine_skill_req = item_template
                    .refine_skill_req
                    .expect("Missing refine skill req");

                let refine_skill_allowed = Skill::from_str(&refine_skill).is_some_and(|skill| {
                    hero_skills.has_proficiency_requirement(
                        skill,
                        refine_skill_req,
                        &templates.skill_templates,
                    )
                });
                if !refine_skill_allowed {
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient refine skill level".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Get refine time
                let refine_time = item_template.get_refine_time();

                //Refine state change
                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Refining,
                });

                let action_id = ids.new_map_event_id();
                commands.entity(hero_entity).insert(ActionProgress {
                    action_id,
                    start_tick: game_tick.0,
                    end_tick: game_tick.0 + refine_time,
                });
                visible_events.new(
                    hero_id,
                    game_tick.0,
                    VisibleEvent::UpdateObjEvent {
                        attrs: vec![
                            ("state".to_string(), STATE_REFINING.to_string()),
                            ("action_id".to_string(), action_id.to_string()),
                            (
                                "action_duration_ms".to_string(),
                                (refine_time.saturating_mul(1000) / TICKS_PER_SEC).to_string(),
                            ),
                            ("action_elapsed_ms".to_string(), "0".to_string()),
                        ],
                    },
                );

                // Add Refine Event
                let event = GameEvent {
                    event_id: action_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + refine_time,
                    event_type: GameEventType::RefineEvent {
                        refiner_id: hero_id,
                        item_id: *item_id,
                    },
                };

                game_events.insert(event.event_id, event);

                let refine_packet = ResponsePacket::Refine {
                    refine_time: refine_time / TICKS_PER_SEC,
                };

                send_to_client(*player_id, refine_packet, &clients);

                active_infos.add((hero_id, ActiveInfoType::Refine), *player_id);
            }
            PlayerEvent::Craft {
                player_id,
                recipe_name,
                signature_item_id,
            } => {
                debug!("PlayerEvent::Craft");
                events_to_remove.push(*event_id);

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok((
                    _hero_pos,
                    hero_state,
                    hero_inventory,
                    hero_skills,
                    last_combat_tick,
                    _hero_viewshed,
                )) = hero_query.get(hero_entity)
                else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                if Obj::is_dead(&hero_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot craft.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if combat_locked(last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                let Some(recipe) = recipes.get_for_owner_by_name(*player_id, recipe_name) else {
                    error!("Invalid recipe name {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Invalid recipe".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                if recipe.requires_structure() {
                    error!("Recipe requires a crafting structure {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Recipe requires a crafting structure".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !recipe.skill_requirement_met(hero_skills, &templates) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient crafting skill level".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !hero_inventory.has_craft_reqs(recipe.req.clone(), *signature_item_id) {
                    error!("Insufficient resources to craft {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: if signature_item_id.is_some() {
                            "The selected signature component is unavailable or does not match this recipe"
                                .to_string()
                        } else {
                            "Insufficient Common resources to craft; select a special component explicitly"
                                .to_string()
                        },
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Get crafting time
                let crafting_time = recipe.crafting_time.unwrap_or(100);

                //Crafting state change
                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Crafting,
                });

                // Add Craft     Event
                let event = GameEvent {
                    event_id: ids.new_map_event_id(),
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + crafting_time,
                    event_type: GameEventType::CraftEvent {
                        crafter_id: hero_id,
                        recipe_name: recipe_name.clone(),
                        signature_item_id: *signature_item_id,
                    },
                };

                game_events.insert(event.event_id, event);

                let craft_packet = ResponsePacket::Craft {
                    craft_time: crafting_time / TICKS_PER_SEC,
                };

                send_to_client(*player_id, craft_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn structure_refine_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: ResMut<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    recipes: Res<Recipes>,
    mut active_infos: ResMut<ActiveInfos>,
    mut query: Query<(
        &PlayerId,
        &Position,
        &State,
        &mut Inventory,
        Option<&LastCombatTick>,
    )>,
    skills_query: Query<&mut Skills>,
    template_query: Query<&Template>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::StructureRefine {
                player_id,
                structure_id,
                item_id,
            } => {
                debug!("PlayerEvent::StructureRefine");
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!(
                        "Cannot find structure entity for structure {:?}",
                        structure_id
                    );
                    continue;
                };

                let Ok(
                    [(hero_player_id, hero_pos, hero_state, hero_inventory, last_combat_tick), (
                        structure_player_id,
                        structure_pos,
                        structure_state,
                        structure_inventory,
                        _structure_last_combat_tick,
                    )],
                ) = query.get_many_mut([hero_entity, structure_entity])
                else {
                    error!("Cannot find hero or structure for {:?}", hero_entity);
                    continue;
                };

                let Ok(hero_skills) = skills_query.get(hero_entity) else {
                    error!("Cannot find hero skills for {:?}", hero_entity);
                    continue;
                };

                if Obj::is_dead(&hero_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot refine.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if combat_locked(last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                if structure_player_id.0 != *player_id {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !Structure::is_built(*structure_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure is not active".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Hero and Structure on the same location
                if hero_pos != structure_pos {
                    error!("Hero and Structure are not on the same location");
                    let packet = ResponsePacket::Error {
                        errmsg: "Must be on structure to refine".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Get item to refine
                let Some(item) = structure_inventory.get_by_id(*item_id) else {
                    error!("Cannot find item for {:?}", item_id);
                    continue;
                };

                let Ok(structure_template) = template_query.get(structure_entity) else {
                    error!("Cannot find structure template for {:?}", structure_entity);
                    continue;
                };
                let station_template = templates.obj_templates.get(structure_template.0.clone());
                let supports_item = station_template.refine.as_ref().is_some_and(|types| {
                    types.iter().any(|item_type| {
                        item_type == &item.name
                            || item_type == &item.class
                            || item_type == &item.subclass
                    })
                });
                if !supports_item {
                    let packet = ResponsePacket::Error {
                        errmsg: "This structure cannot refine that item".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if *hero_state == State::Refining {
                    let packet = ResponsePacket::Error {
                        errmsg: "Already refining".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Get item template
                let item_template = Item::get_template(item.name, &templates.item_templates);

                // Check if hero has the required refine skill level
                let refine_skill = item_template
                    .refine_skill
                    .clone()
                    .expect("Missing refine skill");
                let refine_skill_req = item_template
                    .refine_skill_req
                    .expect("Missing refine skill req");

                let refine_skill_allowed = Skill::from_str(&refine_skill).is_some_and(|skill| {
                    hero_skills.has_proficiency_requirement(
                        skill,
                        refine_skill_req,
                        &templates.skill_templates,
                    )
                });
                if !refine_skill_allowed {
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient refine skill level".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Get refine time
                let refine_time = item_template.get_refine_time();

                //Refine state change
                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Refining,
                });

                // Add Refine Event
                let event = GameEvent {
                    event_id: ids.new_map_event_id(),
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + refine_time,
                    event_type: GameEventType::StructureRefineEvent {
                        refiner_id: hero_id,
                        structure_id: *structure_id,
                        item_id: *item_id,
                        work_entry_id: None,
                    },
                };

                game_events.insert(event.event_id, event);

                let refine_packet = ResponsePacket::Refine {
                    refine_time: refine_time / TICKS_PER_SEC,
                };

                send_to_client(*player_id, refine_packet, &clients);

                active_infos.add((*structure_id, ActiveInfoType::StructureRefine), *player_id);
            }
            PlayerEvent::StructureCraft {
                player_id,
                structure_id,
                recipe_name,
                signature_item_id,
            } => {
                debug!("PlayerEvent::StructureCraft");
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!(
                        "Cannot find structure entity for structure {:?}",
                        structure_id
                    );
                    continue;
                };

                let Ok(
                    [(hero_player_id, hero_pos, hero_state, hero_inventory, last_combat_tick), (
                        structure_player_id,
                        structure_pos,
                        structure_state,
                        structure_inventory,
                        _structure_last_combat_tick,
                    )],
                ) = query.get_many_mut([hero_entity, structure_entity])
                else {
                    error!("Cannot find hero or structure for {:?}", hero_entity);
                    continue;
                };

                if Obj::is_dead(&hero_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot refine.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if combat_locked(last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                if structure_player_id.0 != *player_id {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !Structure::is_built(*structure_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure is not active".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if hero_pos != structure_pos {
                    let packet = ResponsePacket::Error {
                        errmsg: "Must be on structure to craft".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Some(recipe) = recipes.get_for_owner_by_name(*player_id, recipe_name) else {
                    error!("Invalid recipe name {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Invalid recipe".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                let Ok(structure_template) = template_query.get(structure_entity) else {
                    error!("Cannot find structure template for {:?}", structure_entity);
                    continue;
                };
                if !recipe.supports_structure(&structure_template.0) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Recipe is not compatible with this structure".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Ok(hero_skills) = skills_query.get(hero_entity) else {
                    error!("Cannot find hero skills for {:?}", hero_entity);
                    continue;
                };
                if !recipe.skill_requirement_met(hero_skills, &templates) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient crafting skill level".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !structure_inventory.has_craft_reqs(recipe.req.clone(), *signature_item_id) {
                    error!("Insufficient resources to craft {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: if signature_item_id.is_some() {
                            "The selected signature component is unavailable or does not match this recipe"
                                .to_string()
                        } else {
                            "Insufficient Common resources to craft; select a special component explicitly"
                                .to_string()
                        },
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Get crafting time
                let crafting_time = recipe.crafting_time.unwrap_or(100);

                // Crafting state change
                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Crafting,
                });

                // Add Craft Event
                let event = GameEvent {
                    event_id: ids.new_map_event_id(),
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + crafting_time,
                    event_type: GameEventType::StructureCraftEvent {
                        crafter_id: hero_id,
                        structure_id: *structure_id,
                        recipe_name: recipe_name.clone(),
                        signature_item_id: *signature_item_id,
                        work_entry_id: None,
                    },
                };

                game_events.insert(event.event_id, event);

                let craft_packet = ResponsePacket::Craft {
                    craft_time: crafting_time / TICKS_PER_SEC,
                };

                send_to_client(*player_id, craft_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn get_stats_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    entity_map: Res<EntityObjMap>,
    query: Query<(&PlayerId, &Stats, &Thirst, &Hunger, &Tired, &Heat)>,
    attrs_query: Query<()>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::GetStats { player_id, id } => {
                info!("PlayerEvent::GetStats for id: {:?}", id);
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*id) else {
                    error!("Cannot find entity for {:?}", id);
                    break;
                };

                let Ok((obj_player_id, obj_stats, obj_thirst, obj_hunger, obj_tired, obj_heat)) =
                    query.get(entity)
                else {
                    error!("Cannot find obj for {:?}", entity);
                    break;
                };

                if obj_player_id.0 != *player_id {
                    // Silent error
                    error!("GetStats request for object not owned by player.");
                    continue;
                };

                let mut thirst_str = None;
                let mut hunger_str = None;
                let mut tired_str = None;

                thirst_str = Some(obj_thirst.num_to_string());
                hunger_str = Some(obj_hunger.num_to_string());
                tired_str = Some(obj_tired.num_to_string());

                let packet = ResponsePacket::Stats {
                    data: StatsData {
                        id: *id,
                        hp: obj_stats.hp,
                        base_hp: obj_stats.base_hp,
                        stamina: obj_stats.stamina.unwrap_or(100),
                        base_stamina: obj_stats.base_stamina.unwrap_or(100),
                        mana: obj_stats.mana.unwrap_or(0),
                        base_mana: obj_stats.base_mana.unwrap_or(0),
                        thirst: thirst_str,
                        hunger: hunger_str,
                        tiredness: tired_str,
                        effects: Vec::new(),
                    },
                };

                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_hero_system(
    info_hero_event: On<InfoHeroEvent>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    templates: Res<Templates>,
    query: Query<CoreQuery>,
    attrs_query: Query<(&BaseAttrs, &Skills, &Stats, &Thirst, &Hunger, &Tired, &Heat)>,
) {
    let Ok(obj) = query.get(info_hero_event.entity) else {
        error!("Cannot find obj for {:?}", info_hero_event.entity);
        return;
    };

    let items_packet = Some(obj.inventory.get_packet());

    let mut attributes: HashMap<String, i32> = HashMap::new();
    let mut skills_packet = None;

    let total_weight = Some(obj.inventory.get_total_weight());
    let capacity = Some(Obj::get_capacity(
        &obj.template.0.to_string(),
        &templates.obj_templates,
    ));

    let vision_modifier = obj.effects.get_vision_modifier(&templates);

    let Ok((attrs, skills, stats, thirst, hunger, tired, heat)) = attrs_query.get(obj.entity)
    else {
        error!("Cannot find attributes for hero {:?}", obj.entity);
        return;
    };

    attributes.insert(CREATIVITY.to_string(), attrs.creativity);
    attributes.insert(DEXTERITY.to_string(), attrs.dexterity);
    attributes.insert(ENDURANCE.to_string(), attrs.endurance);
    attributes.insert(FOCUS.to_string(), attrs.focus);
    attributes.insert(INTELLECT.to_string(), attrs.intellect);
    attributes.insert(SPIRIT.to_string(), attrs.spirit);
    attributes.insert(STRENGTH.to_string(), attrs.strength);
    attributes.insert(TOUGHNESS.to_string(), attrs.toughness);

    skills_packet = Some(skills.get_levels());

    let effects = obj.effects.get_info_list(&templates.effect_templates);

    let damage_from_items = obj
        .inventory
        .get_items_value_by_attr(&item::AttrKey::Damage, true);

    let defense_from_items = obj
        .inventory
        .get_items_value_by_attr(&item::AttrKey::Defense, true);

    let total_damage = stats.base_damage.unwrap() as f32 + damage_from_items;
    let total_defense = stats.base_def as f32 + defense_from_items;

    let range = Obj::set_viewshed_range(
        obj.id.0,
        obj.template.0.clone(),
        game_tick.0,
        &obj.inventory,
        &templates,
        vision_modifier,
    );

    let response_packet = ResponsePacket::InfoHero {
        id: obj.id.0,
        name: obj.name.0.to_string(),
        template: obj.template.0.to_string(),
        class: obj.class.0.to_string(),
        subclass: obj.subclass.to_string(),
        state: obj.state.to_string(),
        activity: obj.active_task.map(ActiveTask::to_string),
        image: obj.misc.image.clone(),
        portrait: obj.portrait.map(|portrait| portrait.0.clone()),
        hsl: obj.misc.hsl.clone(),
        items: items_packet,
        skills: skills_packet,
        attributes: Some(attributes),
        effects: effects,
        hp: Some(stats.hp),
        stamina: stats.stamina,
        mana: stats.mana,
        thirst: thirst.num_to_string(),
        hunger: hunger.num_to_string(),
        tiredness: tired.num_to_string(),
        base_hp: Some(stats.base_hp),
        base_stamina: stats.base_stamina,
        base_mana: stats.base_mana,
        hero_class: obj
            .hero_class
            .map(|hero_class| hero_class.to_str().to_string()),
        base_def: Some(stats.base_def),
        base_vision: stats.base_vision,
        base_speed: stats.base_speed,
        dmg_range: stats.damage_range,
        base_dmg: stats.base_damage,
        total_dmg: Some(total_damage),
        total_def: Some(total_defense),
        vision: Some(range),
    };

    send_to_client(info_hero_event.player_id, response_packet, &clients);
}

fn info_villager_system(
    info_villager_event: On<InfoVillagerEvent>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    templates: Res<Templates>,
    mut active_infos: ResMut<ActiveInfos>,
    query: Query<CoreQuery>,
    base_attrs_query: Query<(&BaseAttrs, &Skills)>,
    stats_query: Query<&Stats>,
    attrs_query: Query<(&Thirst, &Hunger, &Tired, &Heat)>,
    order_query: Query<&Order>,
    activity_query: Query<(&ActiveTask, Option<&BlockedWork>, Option<&ToolFetchTarget>)>,
    assignment_query: Query<&Assignment>,
    personality_query: Query<&Personality>,
) {
    let Ok(obj) = query.get(info_villager_event.entity) else {
        error!("Cannot find obj for {:?}", info_villager_event.entity);
        return;
    };

    let items_packet = Some(obj.inventory.get_packet());

    let mut attributes: HashMap<String, i32> = HashMap::new();
    let mut skills_packet = None;

    let effects = Some(Vec::<String>::new());

    // Required stats for all objects
    let mut hp = None;
    let mut base_hp = None;
    let mut base_def = None;

    let mut damage_range = None;
    let mut base_damage = None;
    let mut base_speed = None;
    let mut base_vision = None;

    let stamina = None;
    let base_stamina = None;

    let mut activity = None;

    let morale = None;
    let mut order: Option<String> = None;
    let current_order = order_query.get(obj.entity).ok();

    let total_weight = Some(obj.inventory.get_total_weight());
    let capacity = Some(Obj::get_capacity(
        &obj.template.0.to_string(),
        &templates.obj_templates,
    ));

    let vision_modifier = obj.effects.get_vision_modifier(&templates);

    if let Ok((attrs, skills)) = base_attrs_query.get(obj.entity) {
        attributes.insert(CREATIVITY.to_string(), attrs.creativity);
        attributes.insert(DEXTERITY.to_string(), attrs.dexterity);
        attributes.insert(ENDURANCE.to_string(), attrs.endurance);
        attributes.insert(FOCUS.to_string(), attrs.focus);
        attributes.insert(INTELLECT.to_string(), attrs.intellect);
        attributes.insert(SPIRIT.to_string(), attrs.spirit);
        attributes.insert(STRENGTH.to_string(), attrs.strength);
        attributes.insert(TOUGHNESS.to_string(), attrs.toughness);

        skills_packet = Some(skills.get_levels());
    }

    if let Ok(stats) = stats_query.get(obj.entity) {
        hp = Some(stats.hp);
        base_hp = Some(stats.base_hp);
        base_def = Some(stats.base_def);

        damage_range = stats.damage_range;
        base_damage = stats.base_damage;
        base_speed = stats.base_speed;
        base_vision = stats.base_vision;
    }

    let range = Obj::set_viewshed_range(
        obj.id.0,
        obj.template.0.clone(),
        game_tick.0,
        &obj.inventory,
        &templates,
        vision_modifier,
    );

    if let Ok(assignment) = assignment_query.get(obj.entity) {
        //structure = Some(assignment.structure_name.clone());
    }

    let Ok((thirst, hunger, tired, heat)) = attrs_query.get(obj.entity) else {
        error!("Cannot find attributes for villager {:?}", obj.entity);
        return;
    };

    if let Some(current_order) = current_order {
        order = Some(current_order.to_string());
    }

    if let Ok((active_task, blocked_work, tool_fetch_target)) = activity_query.get(obj.entity) {
        activity = Some(villager_activity_text(
            active_task,
            obj.state,
            current_order,
            obj.inventory,
            blocked_work,
            tool_fetch_target,
        ));
    }

    let response_packet = ResponsePacket::InfoVillager {
        id: obj.id.0,
        name: obj.name.0.to_string(),
        template: obj.template.0.to_string(),
        class: obj.class.0.to_string(),
        subclass: obj.subclass.to_string(),
        state: obj.state.to_string(),
        image: obj.misc.image.clone(),
        portrait: obj.portrait.map(|portrait| portrait.0.clone()),
        hsl: obj.misc.hsl.clone(),
        items: items_packet,
        skills: skills_packet,
        attributes: Some(attributes),
        effects: effects,
        need: "".to_string(),
        thirst: thirst.num_to_string(),
        hunger: hunger.num_to_string(),
        tiredness: tired.num_to_string(),
        hp: hp,
        stamina: stamina,
        base_hp: base_hp,
        base_stamina: base_stamina,
        base_def: base_def,
        base_vision: base_vision,
        base_speed: base_speed,
        dmg_range: damage_range,
        base_dmg: base_damage,
        vision: Some(range),
        structure: None,
        activity,
        shelter: None,
        morale: morale,
        order: order,
        capacity: capacity,
        total_weight: total_weight,
        personality: personality_query
            .get(obj.entity)
            .ok()
            .map(|p| p.to_str().to_string()),
    };

    active_infos.add(
        (obj.id.0, ActiveInfoType::Obj),
        info_villager_event.player_id,
    );
    send_to_client(info_villager_event.player_id, response_packet, &clients);
}

fn info_structure_system(
    info_structure_event: On<InfoStructureEvent>,
    clients: Res<Clients>,
    templates: Res<Templates>,
    crops: Res<Crops>,
    mut active_infos: ResMut<ActiveInfos>,
    query: Query<CoreQuery>,
    stats_query: Query<&Stats>,
    build_state_query: Query<&BuildUpgradeState>,
    selected_upgrade_query: Query<&SelectedUpgrade>,
    shelters: Query<&Shelter>,
) {
    info!("processing info_structure_system");
    let Ok(obj) = query.get(info_structure_event.entity) else {
        error!("Cannot find obj for {:?}", info_structure_event.entity);
        return;
    };

    let items_packet = Some(obj.inventory.get_packet());
    let effects = Some(Vec::<String>::new());

    let total_weight = Some(obj.inventory.get_total_weight());
    let capacity = Some(Obj::get_capacity(
        &obj.template.0.to_string(),
        &templates.obj_templates,
    ));
    let structure_template =
        Structure::get_template(obj.template.0.to_string(), &templates.obj_templates)
            .expect("Cannot find structure template");

    // Required stats for all objects
    let mut hp = None;
    let mut base_hp = None;
    let mut base_def = None;

    let mut work_done = None;
    let mut work_per_sec = None;
    let mut selected_upgrade_name = None;
    let mut upgrade_req = Vec::new();
    let mut upgrade_cost = None;
    let mut residents = None;

    if let Ok(stats) = stats_query.get(obj.entity) {
        hp = Some(stats.hp);
        base_hp = Some(stats.base_hp);
        base_def = Some(stats.base_def);
    }

    if *obj.state == State::Building || *obj.state == State::Upgrading {
        if let Ok(build_state) = build_state_query.get(obj.entity) {
            work_done = Some(build_state.work_done);
            work_per_sec = Some(build_state.work_per_sec);
        }
    }

    if *obj.state == State::PlanningUpgrade || *obj.state == State::Upgrading {
        if let Ok(selected_upgrade) = selected_upgrade_query.get(obj.entity) {
            selected_upgrade_name = Some(selected_upgrade.0.clone());

            let upgrade_structure_template =
                Structure::get_template(selected_upgrade.0.clone(), &templates.obj_templates);

            upgrade_req = upgrade_structure_template
                .clone()
                .expect("Cannot find upgrade structure template")
                .upgrade_req
                .unwrap_or(vec![]);

            upgrade_cost = Some(
                upgrade_structure_template
                    .clone()
                    .expect("Cannot find upgrade structure template")
                    .upgrade_cost
                    .unwrap_or(MAX_BUILD_UPGRADE_COST) as f32,
            );
        }
    }

    // Shelter specific attributes
    if let Ok(shelter) = shelters.get(obj.entity) {
        residents = Some(shelter.residents.len() as i32);
    }

    // Farm specific attributes
    let mut crop_type = None;
    let mut crop_quantity = None;
    let mut crop_stage = None;

    info!("info_structure_system: crops {:?}", crops);
    if let Some(crop) = crops.get(&obj.id.0) {
        info!("info_structure_system: crop {:?}", crop);
        crop_type = Some(crop.crop_type.clone());
        crop_quantity = Some(crop.quantity);
        crop_stage = Some(crop.stage.to_string());
    }

    let req_items = Structure::get_current_req_quantities(
        obj.template.0.clone(),
        obj.class.0.clone(),
        obj.state.clone(),
        &obj.inventory,
        &templates,
        selected_upgrade_name.clone(),
    );

    let upgradeable = structure_template
        .upgrade_to
        .as_ref()
        .map(|list| !list.is_empty())
        .unwrap_or(false);

    let response_packet = ResponsePacket::InfoStructure {
        id: obj.id.0,
        name: obj.name.0.to_string(),
        template: obj.template.0.to_string(),
        class: obj.class.0.to_string(),
        subclass: obj.subclass.to_string(),
        x: obj.pos.x,
        y: obj.pos.y,
        state: obj.state.to_string(),
        image: obj.misc.image.clone(),
        hsl: obj.misc.hsl.clone(),
        items: items_packet,
        effects: effects,
        hp: hp,
        base_hp: base_hp,
        base_def: base_def,
        capacity: capacity,
        total_weight: total_weight,
        workspaces: structure_template.workspaces,
        max_residents: structure_template.max_residents,
        residents: residents,
        build_cost: Some(
            structure_template
                .build_cost
                .unwrap_or(MAX_BUILD_UPGRADE_COST) as f32,
        ),
        upgrade_cost: upgrade_cost,
        work_done: work_done,
        work_per_sec: work_per_sec,
        req: Some(req_items.clone()),
        upgrade_req: Some(req_items.clone()),
        selected_upgrade: selected_upgrade_name,
        crop_type: crop_type,
        crop_quantity: crop_quantity,
        crop_stage: crop_stage,
        upgradeable: upgradeable,
    };

    active_infos.add(
        (obj.id.0, ActiveInfoType::Structure),
        info_structure_event.player_id,
    );
    send_to_client(info_structure_event.player_id, response_packet, &clients);
}

fn info_monolith_system(
    info_monolith_event: On<InfoMonolithEvent>,
    clients: Res<Clients>,
    mut queries: ParamSet<(Query<CoreQuery>, Query<&mut Inventory, With<SubclassHero>>)>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    presence: Res<PlayerWorldPresenceState>,
    mut monolith_investigation: ResMut<MonolithInvestigation>,
) {
    // First pass: read monolith info via CoreQuery
    let (
        monolith_id,
        monolith_name,
        monolith_class,
        monolith_subclass,
        monolith_template,
        monolith_image,
        soulshards,
    ) = {
        let query = queries.p0();
        let Ok(obj) = query.get(info_monolith_event.entity) else {
            error!("Cannot find obj for {:?}", info_monolith_event.entity);
            return;
        };

        (
            obj.id.0,
            obj.name.0.to_string(),
            obj.class.0.to_string(),
            obj.subclass.to_string(),
            obj.template.0.to_string(),
            obj.misc.image.clone(),
            obj.inventory
                .get_by_class(item::SOULSHARD.to_string())
                .map(|soulshards| soulshards.quantity)
                .unwrap_or(0),
        )
    };

    let response_packet = ResponsePacket::InfoMonolith {
        id: monolith_id,
        name: monolith_name,
        class: monolith_class,
        subclass: monolith_subclass,
        template: monolith_template,
        image: monolith_image,
        soulshards,
    };

    send_to_client(info_monolith_event.player_id, response_packet, &clients);

    // The info packet is read-only and remains available, but this legacy
    // "info" observer also advances a quest and consumes items. Freeze that
    // hidden mutation while the requesting run is protected.
    if is_player_offline_protected(info_monolith_event.player_id, &presence) {
        return;
    }

    // Monolith investigation chain
    let progress = monolith_investigation
        .entry(info_monolith_event.player_id)
        .or_insert_with(MonolithProgress::default);

    match progress.stage {
        0 => {
            // Stage 0 → 1: First observation
            progress.stage = 1;
            let lore_packet = ResponsePacket::Notice {
                noticemsg: "The Monolith hums with ancient power. Strange runes glow faintly on its surface. You sense it could be investigated further... perhaps with Soulshards.".to_string(),
                expiry: Some(15000),
            };
            send_to_client(info_monolith_event.player_id, lore_packet, &clients);
        }
        1 => {
            // Stage 1 → 2: Requires 3 Soulshards in hero inventory
            let hero_id = ids.get_hero(info_monolith_event.player_id);
            if let Some(hero_id) = hero_id {
                if let Some(hero_entity) = entity_map.get_entity(hero_id) {
                    let mut hero_query = queries.p1();
                    if let Ok(mut inventory) = hero_query.get_mut(hero_entity) {
                        let soulshards = inventory.get_by_name(item::SOULSHARD.to_string());
                        if let Some(shard_item) = soulshards {
                            if shard_item.quantity >= 3 {
                                // Consume 3 soulshards
                                inventory.remove_quantity(shard_item.id, 3);
                                progress.stage = 2;

                                let lore_packet = ResponsePacket::Notice {
                                    noticemsg: "You press the Soulshards into the Monolith's surface. The runes flare to life! Visions flood your mind — this island was once a great kingdom, destroyed by dark magic. The Monolith is the source of the undead plague. It can be sealed... but you must bring a powerful offering. Craft a Seal Stone and return.".to_string(),
                                    expiry: Some(20000),
                                };
                                send_to_client(
                                    info_monolith_event.player_id,
                                    lore_packet,
                                    &clients,
                                );
                            } else {
                                let hint_packet = ResponsePacket::Notice {
                                    noticemsg: format!("The Monolith resonates with your Soulshards ({}/3 needed). Gather more to proceed.", shard_item.quantity),
                                    expiry: Some(8000),
                                };
                                send_to_client(
                                    info_monolith_event.player_id,
                                    hint_packet,
                                    &clients,
                                );
                            }
                        } else {
                            let hint_packet = ResponsePacket::Notice {
                                noticemsg: "The Monolith's runes pulse weakly. You need 3 Soulshards to proceed with the investigation.".to_string(),
                                expiry: Some(8000),
                            };
                            send_to_client(info_monolith_event.player_id, hint_packet, &clients);
                        }
                    }
                }
            }
        }
        2 => {
            // Stage 2 → 3: Requires Seal Stone in hero inventory
            let hero_id = ids.get_hero(info_monolith_event.player_id);
            if let Some(hero_id) = hero_id {
                if let Some(hero_entity) = entity_map.get_entity(hero_id) {
                    let mut hero_query = queries.p1();
                    if let Ok(mut inventory) = hero_query.get_mut(hero_entity) {
                        let seal_stone = inventory.get_by_name("Seal Stone".to_string());
                        if let Some(seal_item) = seal_stone {
                            // Consume the Seal Stone
                            inventory.remove_quantity(seal_item.id, 1);
                            progress.stage = 3;
                            progress.sealed = true;

                            let lore_packet = ResponsePacket::Notice {
                                noticemsg: "You place the Seal Stone upon the Monolith. A brilliant light erupts from within! The dark energy dissipates and the sanctuary expands. The undead hordes weaken across the island. You have sealed the Monolith!".to_string(),
                                expiry: Some(25000),
                            };
                            send_to_client(info_monolith_event.player_id, lore_packet, &clients);
                        } else {
                            let hint_packet = ResponsePacket::Notice {
                                noticemsg: "The Monolith awaits its seal. Craft a Seal Stone and bring it here to complete the ritual.".to_string(),
                                expiry: Some(8000),
                            };
                            send_to_client(info_monolith_event.player_id, hint_packet, &clients);
                        }
                    }
                }
            }
        }
        _ => {
            // Already sealed
            let packet = ResponsePacket::Notice {
                noticemsg: "The Monolith stands sealed. Its sanctuary protects the island."
                    .to_string(),
                expiry: Some(5000),
            };
            send_to_client(info_monolith_event.player_id, packet, &clients);
        }
    }
}

fn info_poi_system(
    info_poi_event: On<InfoPOIEvent>,
    clients: Res<Clients>,
    run_spawned_objs: Res<RunSpawnedObjs>,
    investigated_pois: Res<InvestigatedPOIs>,
    game_tick: Res<GameTick>,
    query: Query<CoreQuery>,
) {
    let Ok(obj) = query.get(info_poi_event.entity) else {
        error!("Cannot find obj for {:?}", info_poi_event.entity);
        return;
    };

    if !can_access_run_shipwreck(
        info_poi_event.player_id,
        obj.id.0,
        obj.template,
        &run_spawned_objs,
    ) {
        send_shipwreck_owner_error(info_poi_event.player_id, &clients);
        return;
    }

    let items_packet = can_access_shipwreck_inventory(
        info_poi_event.player_id,
        obj.id.0,
        obj.template,
        &investigated_pois,
    )
    .then(|| obj.inventory.get_packet());

    let response_packet = ResponsePacket::InfoPOI {
        id: obj.id.0,
        name: obj.name.0.to_string(),
        class: obj.class.0.to_string(),
        subclass: obj.subclass.to_string(),
        template: obj.template.0.to_string(),
        image: obj.misc.image.clone(),
        items: items_packet,
        expires_in: obj
            .dropped_bag
            .map(|bag| dropped_bag_expires_in(bag.expires_at, game_tick.0)),
    };

    send_to_client(info_poi_event.player_id, response_packet, &clients);
}

fn info_npc_system(
    info_npc_event: On<InfoNPCEvent>,
    clients: Res<Clients>,
    query: Query<CoreQuery>,
) {
    let Ok(obj) = query.get(info_npc_event.entity) else {
        error!("Cannot find obj for {:?}", info_npc_event.entity);
        return;
    };

    let mut items_packet = None;

    // Add items if object is dead
    if *obj.state == State::Dead {
        items_packet = Some(obj.inventory.get_packet());
    }

    let mut effects = Vec::new();

    // Get effects
    for (key, _val) in obj.effects.0.iter() {
        effects.push(key.clone().to_str());
    }

    let response_packet = ResponsePacket::InfoNPC {
        id: obj.id.0,
        name: obj.name.0.to_string(),
        template: obj.template.0.to_string(),
        class: obj.class.0.to_string(),
        subclass: obj.subclass.to_string(),
        state: Obj::state_to_str(obj.state.to_owned()),
        image: obj.misc.image.clone(),
        hsl: obj.misc.hsl.clone(),
        items: items_packet,
        effects: effects,
    };

    send_to_client(info_npc_event.player_id, response_packet, &clients);
}

fn info_obj_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    query: Query<CoreQuery>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoObj { player_id, id } => {
                info!("PlayerEvent::InfoObj for id: {:?}", id);
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*id) else {
                    error!("Cannot find entity for {:?}", id);
                    break;
                };

                let Ok(obj) = query.get(entity) else {
                    error!("Cannot find obj for {:?}", entity);
                    break;
                };

                let mut response_packet = ResponsePacket::None;

                if obj.player_id.0 == *player_id {
                    if obj.class.0 == CLASS_UNIT {
                        if *obj.subclass == Subclass::Hero {
                            commands.trigger(InfoHeroEvent {
                                entity: entity,
                                player_id: *player_id,
                            });
                            continue;
                        } else if *obj.subclass == Subclass::Villager {
                            commands.trigger(InfoVillagerEvent {
                                entity: entity,
                                player_id: *player_id,
                            });
                            continue;
                        }
                    } else if obj.class.0 == CLASS_STRUCTURE {
                        commands.trigger(InfoStructureEvent {
                            entity: entity,
                            player_id: *player_id,
                        });
                        continue;
                    }
                } else {
                    if *obj.subclass == Subclass::Monolith {
                        commands.trigger(InfoMonolithEvent {
                            entity: entity,
                            player_id: *player_id,
                        });
                        continue;
                    } else if *obj.subclass == Subclass::Poi {
                        commands.trigger(InfoPOIEvent {
                            entity: entity,
                            player_id: *player_id,
                        });
                        continue;
                    } else if *obj.subclass == Subclass::Npc {
                        commands.trigger(InfoNPCEvent {
                            entity: entity,
                            player_id: *player_id,
                        });
                        continue;
                    } else {
                        response_packet = ResponsePacket::InfoObj {
                            id: obj.id.0,
                            name: obj.name.0.to_string(),
                            class: obj.class.0.to_string(),
                            subclass: obj.subclass.to_string(),
                            template: obj.template.0.to_string(),
                            image: obj.misc.image.clone(),
                        };
                    }
                }

                send_to_client(*player_id, response_packet, &clients);
            }

            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_skills_system(
    mut events: ResMut<PlayerEvents>,
    entity_map: ResMut<EntityObjMap>,
    clients: Res<Clients>,
    templates: Res<Templates>,
    query: Query<(&PlayerId, &Skills)>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoSkills { player_id, id } => {
                info!("PlayerEvent::InfoSkills for id: {:?}", id);
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*id) else {
                    error!("Cannot find entity for {:?}", id);
                    continue;
                };

                let Ok((obj_player_id, obj_skills)) = query.get(entity) else {
                    error!("Cannot find villager for {:?}", entity);
                    continue;
                };

                if obj_player_id.0 != *player_id {
                    error!("Object {:?} is not owned by player {:?}", id, player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Object not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let obj_skills_packet = obj_skills.get_packet(&templates.skill_templates);

                let info_skills_packet = ResponsePacket::InfoSkills {
                    id: *id,
                    skills: obj_skills_packet,
                };

                send_to_client(*player_id, info_skills_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_attrs_system(
    mut events: ResMut<PlayerEvents>,
    entity_map: ResMut<EntityObjMap>,
    clients: Res<Clients>,
    query: Query<CoreQuery>,
    attr_query: Query<&BaseAttrs>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoAttrs { player_id, id } => {
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*id) else {
                    error!("Cannot find entity for {:?}", id);
                    continue;
                };

                let Ok(obj) = query.get(entity) else {
                    error!("Cannot find villager for {:?}", entity);
                    continue;
                };

                if obj.player_id.0 == *player_id {
                    if let Ok(attrs) = attr_query.get(entity) {
                        let mut attrs_packet = HashMap::new();

                        attrs_packet.insert(CREATIVITY.to_string(), attrs.creativity);
                        attrs_packet.insert(DEXTERITY.to_string(), attrs.dexterity);
                        attrs_packet.insert(ENDURANCE.to_string(), attrs.endurance);
                        attrs_packet.insert(FOCUS.to_string(), attrs.focus);
                        attrs_packet.insert(INTELLECT.to_string(), attrs.intellect);
                        attrs_packet.insert(SPIRIT.to_string(), attrs.spirit);
                        attrs_packet.insert(TOUGHNESS.to_string(), attrs.toughness);

                        let info_attrs_packet = ResponsePacket::InfoAttrs {
                            id: *id,
                            attrs: attrs_packet,
                        };

                        send_to_client(*player_id, info_attrs_packet, &clients);
                    } else {
                        error!("Cannot find attributes for {:?}", id);
                    }
                } else {
                    error!("Object {:?} is not owned by player {:?}", id, player_id);
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_advance_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: Res<GameTick>,
    entity_map: ResMut<EntityObjMap>,
    clients: Res<Clients>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    mut query: Query<(
        &PlayerId,
        &mut Template,
        &mut Stats,
        &Inventory,
        &Effects,
        Option<&HeroClass>,
        Option<&mut Viewshed>,
        &Skills,
    )>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoAdvance { player_id, id } => {
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*id) else {
                    error!("Cannot find entity for {:?}", id);
                    continue;
                };

                let Ok((
                    obj_player_id,
                    obj_template,
                    _obj_stats,
                    _inventory,
                    _effects,
                    _hero_class,
                    _viewshed,
                    obj_skills,
                )) = query.get_mut(entity)
                else {
                    error!("Cannot find obj for {:?}", entity);
                    continue;
                };

                if obj_player_id.0 == *player_id {
                    let (next_template, required_xp) =
                        SkillData::hero_advance(obj_template.0.clone());

                    let info_advance_packet = ResponsePacket::InfoAdvance {
                        id: *id,
                        rank: obj_template.0.clone(),
                        next_rank: next_template,
                        total_xp: obj_skills.get_total_xp(),
                        req_xp: required_xp,
                    };

                    send_to_client(*player_id, info_advance_packet, &clients);
                } else {
                    error!("Object {:?} is not owned by player {:?}", id, player_id);
                }
            }
            PlayerEvent::Advance { player_id, id } => {
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*id) else {
                    error!("Cannot find entity for {:?}", id);
                    continue;
                };

                let Ok((
                    obj_player_id,
                    mut obj_template,
                    mut obj_stats,
                    inventory,
                    effects,
                    hero_class,
                    viewshed,
                    obj_skills,
                )) = query.get_mut(entity)
                else {
                    error!("Cannot find obj for {:?}", entity);
                    continue;
                };

                if obj_player_id.0 == *player_id {
                    let (next_template, _required_xp) =
                        SkillData::hero_advance(obj_template.0.clone());

                    // Max rank reached cannot advance further
                    if next_template == MAX_RANK {
                        let advance_packet = ResponsePacket::InfoAdvance {
                            id: *id,
                            rank: next_template.clone(),
                            next_rank: next_template,
                            total_xp: 0,
                            req_xp: 0,
                        };

                        send_to_client(*player_id, advance_packet, &clients);
                        continue;
                    }

                    let next_obj_template = templates.obj_templates.get(next_template.clone());
                    refresh_stats_from_template(
                        &mut obj_stats,
                        hero_class.copied(),
                        &next_obj_template,
                    );
                    obj_template.0 = next_template.clone();

                    if let Some(mut viewshed) = viewshed {
                        viewshed.range = Obj::set_viewshed_range(
                            *id,
                            next_template.clone(),
                            game_tick.0,
                            inventory,
                            &templates,
                            effects.get_vision_modifier(&templates),
                        );
                    }

                    //Add obj update event
                    commands.trigger(UpdateObj {
                        entity: entity,
                        attrs: vec![(TEMPLATE.to_string(), next_template.clone())],
                    });

                    let (new_next_template, new_required_xp) =
                        SkillData::hero_advance(next_template.clone());

                    let advance_packet = ResponsePacket::InfoAdvance {
                        id: *id,
                        rank: next_template.clone(),
                        next_rank: new_next_template,
                        total_xp: 0, // Advancing resets to zero
                        req_xp: new_required_xp,
                    };

                    send_to_client(*player_id, advance_packet, &clients);
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_upgrade_system(
    mut events: ResMut<PlayerEvents>,
    _game_tick: Res<GameTick>,
    entity_map: ResMut<EntityObjMap>,
    clients: Res<Clients>,
    structure_query: Query<StructureQuery, With<ClassStructure>>,
    templates: Res<Templates>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoUpgrade {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    break;
                };

                let Ok(structure) = structure_query.get(structure_entity) else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    break;
                };

                if structure.player_id.0 != *player_id {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let current_structure_template =
                    templates.obj_templates.get(structure.name.0.clone());
                debug!(
                    "current_structure_template: {:?}",
                    current_structure_template
                );

                let Some(upgrade_to_list) = current_structure_template.upgrade_to else {
                    error!(
                        "Missing upgrade_to field on structure template: {:?}",
                        structure.name.0.clone()
                    );
                    continue;
                };

                let mut upgrade_template_list = Vec::new();
                debug!("upgrade_to_list {:?}", upgrade_to_list);
                for upgrade_to_structure in upgrade_to_list.iter() {
                    let upgrade_structure_template = templates
                        .obj_templates
                        .get(upgrade_to_structure.to_string());
                    debug!(
                        "upgrade_structure_template {:?}",
                        upgrade_structure_template
                    );

                    let upgrade_template = network::UpgradeTemplate {
                        name: upgrade_structure_template.template.clone(),
                        template: upgrade_structure_template.template,
                        image: upgrade_structure_template.image,
                        req: upgrade_structure_template.upgrade_req.unwrap_or(vec![]),
                        build_time: upgrade_structure_template.build_cost.unwrap_or(0),
                    };

                    upgrade_template_list.push(upgrade_template);
                }

                if upgrade_template_list.len() == 0 {
                    error!(
                        "Cannot build upgrade template list for {:?}",
                        structure.name.0.clone()
                    );
                    continue;
                }

                let upgrade_packet = ResponsePacket::InfoUpgrade {
                    id: structure.id.0,
                    upgrade_list: upgrade_template_list,
                };

                send_to_client(*player_id, upgrade_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_tile_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    map: Res<Map>,
    resources: Res<Resources>,
    discoveries: Res<ResourceDiscoveries>,
    survey_history: Res<SurveyHistory>,
    terrain_features: Res<TerrainFeatures>,
    obj_query: Query<ObjQuery>,
    monolith_query: Query<(&Position, &Monolith), With<Monolith>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoTile { player_id, x, y } => {
                debug!("PlayerEvent::InfoTile x: {:?} y: {:?}", *x, *y);
                events_to_remove.push(*event_id);

                let tile_type = Map::tile_type(*x, *y, &map);
                let mut sanctuary = "None".to_string();

                for (monolith_pos, monolith) in monolith_query.iter() {
                    let distance = Map::dist(Position { x: *x, y: *y }, *monolith_pos);
                    if distance < sanctuary_radius(monolith.sanctuary_level) {
                        sanctuary = "Sanctuary".to_string();
                        break;
                    }
                }

                let info_tile_packet: ResponsePacket = ResponsePacket::InfoTile {
                    x: *x,
                    y: *y,
                    name: Map::tile_name(tile_type),
                    mc: Map::movement_cost(tile_type),
                    def: Map::def_bonus(tile_type),
                    unrevealed: Resource::num_unrevealed_on_tile(
                        Position { x: *x, y: *y },
                        &resources,
                        &discoveries,
                        *player_id,
                    ),
                    sanctuary: sanctuary,
                    passable: Map::is_passable(*x, *y, &map),
                    wildness: map.get_wildness_string(*x, *y),
                    survey_status: survey_status_for_tile(
                        *player_id,
                        Position { x: *x, y: *y },
                        &survey_history,
                    ),
                    resources: Resource::get_on_tile(
                        Position { x: *x, y: *y },
                        &resources,
                        &discoveries,
                        *player_id,
                    ),
                    terrain_features: TerrainFeature::get_by_tile(
                        Position { x: *x, y: *y },
                        &terrain_features,
                    ),
                };

                send_to_client(*player_id, info_tile_packet, &clients);
            }
            PlayerEvent::InfoTileResources { player_id, x, y } => {
                debug!("PlayerEvent::InfoTileResources x: {:?} y: {:?}", *x, *y);
                events_to_remove.push(*event_id);

                let tile_type = Map::tile_type(*x, *y, &map);

                let info_tile_resources_packet = ResponsePacket::InfoTileResources {
                    x: *x,
                    y: *y,
                    name: Map::tile_name(tile_type),
                    resources: Resource::get_on_tile(
                        Position { x: *x, y: *y },
                        &resources,
                        &discoveries,
                        *player_id,
                    ),
                };

                send_to_client(*player_id, info_tile_resources_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_item_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    entity_map: Res<EntityObjMap>,
    run_spawned_objs: Res<RunSpawnedObjs>,
    investigated_pois: Res<InvestigatedPOIs>,
    prices: Res<Prices>,
    templates: Res<Templates>,
    query: Query<(&PlayerId, &Name, &Template, &Inventory)>,
    mut active_infos: ResMut<ActiveInfos>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoInventory { player_id, id } => {
                debug!("PlayerEvent::InfoInventory id: {:?}", id);
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*id) else {
                    error!("Cannot find entity for {:?}", id);
                    break;
                };

                let Ok((_pid, obj_name, obj_template, inventory)) = query.get(entity) else {
                    error!("Cannot find obj template or inventory for {:?}", entity);
                    break;
                };

                if !can_access_run_shipwreck(*player_id, *id, obj_template, &run_spawned_objs) {
                    send_shipwreck_owner_error(*player_id, &clients);
                    continue;
                }
                if !can_access_shipwreck_inventory(
                    *player_id,
                    *id,
                    obj_template,
                    &investigated_pois,
                ) {
                    send_shipwreck_search_error(*player_id, &clients);
                    continue;
                }

                let capacity = Obj::get_capacity(&obj_template.0, &templates.obj_templates);
                let total_weight = inventory.get_total_weight();

                let inventory_items = inventory.get_packet();

                let info_inventory_packet: ResponsePacket = ResponsePacket::InfoInventory {
                    id: *id,
                    cap: capacity as i32,
                    tw: total_weight as i32,
                    items: inventory_items,
                };

                active_infos.add((*id, ActiveInfoType::Inventory), *player_id);

                send_to_client(*player_id, info_inventory_packet, &clients);
            }
            PlayerEvent::InfoEquip { player_id, id } => {
                debug!("PlayerEvent::InfoEquip id: {:?}", id);
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*id) else {
                    error!("Cannot find entity for {:?}", id);
                    break;
                };

                let Ok((_pid, obj_name, obj_template, inventory)) = query.get(entity) else {
                    error!("Cannot find obj template or inventory for {:?}", entity);
                    break;
                };

                if !can_access_run_shipwreck(*player_id, *id, obj_template, &run_spawned_objs) {
                    send_shipwreck_owner_error(*player_id, &clients);
                    continue;
                }
                if !can_access_shipwreck_inventory(
                    *player_id,
                    *id,
                    obj_template,
                    &investigated_pois,
                ) {
                    send_shipwreck_search_error(*player_id, &clients);
                    continue;
                }

                let capacity = Obj::get_capacity(&obj_template.0, &templates.obj_templates);
                let total_weight = inventory.get_total_weight();

                let inventory_items = inventory.get_packet();

                let info_equip_packet: ResponsePacket = ResponsePacket::InfoEquip {
                    name: obj_name.0.clone(),
                    template: obj_template.0.clone(),
                    id: *id,
                    cap: capacity as i32,
                    tw: total_weight as i32,
                    items: inventory_items,
                };

                active_infos.add((*id, ActiveInfoType::Equip), *player_id);

                send_to_client(*player_id, info_equip_packet, &clients);
            }
            PlayerEvent::InfoItem {
                player_id,
                obj_id,
                item_id,
                action,
            } => {
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*obj_id) else {
                    error!("Cannot find entity for {:?}", obj_id);
                    break;
                };

                let Ok((_pid, _obj_name, obj_template, inventory)) = query.get(entity) else {
                    error!("Cannot find obj template or inventory for {:?}", entity);
                    break;
                };

                if !can_access_run_shipwreck(*player_id, *obj_id, obj_template, &run_spawned_objs) {
                    send_shipwreck_owner_error(*player_id, &clients);
                    continue;
                }
                if !can_access_shipwreck_inventory(
                    *player_id,
                    *obj_id,
                    obj_template,
                    &investigated_pois,
                ) {
                    send_shipwreck_search_error(*player_id, &clients);
                    continue;
                }

                if action == "player_selling_item" {
                    let item = inventory.get_item_packet(*item_id);

                    if let Some(item) = item {
                        debug!("InfoItem item: {:?}", item);
                        let price = prices.find_buy_price(
                            item.name.clone(),
                            item.subclass.clone(),
                            item.class.clone(),
                        );

                        let info_item_packet: ResponsePacket = ResponsePacket::InfoItem {
                            action: Some(action.clone()),
                            id: item.id,
                            owner: item.owner,
                            name: item.name,
                            quantity: item.quantity,
                            durability: item.durability.clone(),
                            class: item.class,
                            subclass: item.subclass,
                            image: item.image,
                            weight: item.weight,
                            equipped: item.equipped,
                            price: price,
                            attrs: item.attrs,
                            produces: None, //TODO get from item template
                        };

                        send_to_client(*player_id, info_item_packet, &clients);
                    }
                } else if action == "player_buying_item" {
                    let item = inventory.get_item_packet(*item_id);

                    if let Some(item) = item {
                        let price = prices.find_sell_price(
                            item.name.clone(),
                            item.subclass.clone(),
                            item.class.clone(),
                        );

                        let info_item_packet: ResponsePacket = ResponsePacket::InfoItem {
                            action: Some(action.clone()),
                            id: item.id,
                            owner: item.owner,
                            name: item.name,
                            quantity: item.quantity,
                            durability: item.durability.clone(),
                            class: item.class,
                            subclass: item.subclass,
                            image: item.image,
                            weight: item.weight,
                            equipped: item.equipped,
                            price: price,
                            attrs: item.attrs,
                            produces: None, //TODO get from item template
                        };

                        send_to_client(*player_id, info_item_packet, &clients);
                    }
                } else {
                    if let Some(item) = inventory.get_item_packet(*item_id) {
                        // Get produces from item template
                        let item_template =
                            Item::get_template(item.name.clone(), &templates.item_templates);

                        let info_item_packet: ResponsePacket = ResponsePacket::InfoItem {
                            action: Some(action.clone()),
                            id: item.id,
                            owner: item.owner,
                            name: item.name,
                            quantity: item.quantity,
                            durability: item.durability.clone(),
                            class: item.class,
                            subclass: item.subclass,
                            image: item.image,
                            weight: item.weight,
                            equipped: item.equipped,
                            price: None,
                            attrs: item.attrs,
                            produces: item_template.produces.as_deref().map(|outputs| {
                                item::produced_item_packets(outputs, &templates.item_templates)
                            }),
                        };

                        send_to_client(*player_id, info_item_packet, &clients);
                    }
                }
            }
            PlayerEvent::InfoItemByName { player_id, name } => {
                debug!("PlayerEvent::InfoItemByName name: {:?}", name.clone());
                events_to_remove.push(*event_id);

                // TODO prevent item data mining

                // Get all items from all inventories of player
                /*for (pid, obj_name, obj_template, inventory) in query.iter() {
                    if *pid == *player_id {


                    }
                */

                // Get item from template
                let item_template = Item::find_template(name.clone(), &templates.item_templates);

                let Some(item_template) = item_template else {
                    error!("Cannot find item template: {:?}", name);
                    continue;
                };

                let mut attrs = HashMap::new();

                if let Some(item_template_attrs) = &item_template.attrs {
                    for item_attr in item_template_attrs.iter() {
                        let attr_key = AttrKey::str_to_key(item_attr.name.clone());
                        let attr_val = AttrVal::Num(item_attr.value.parse::<f32>().unwrap());
                        attrs.insert(attr_key, attr_val);
                    }
                }

                let info_item_packet: ResponsePacket = ResponsePacket::InfoItem {
                    action: None,
                    id: -1,
                    owner: -1,
                    name: item_template.name.clone(),
                    quantity: 1,
                    durability: item_template.durability.clone(),
                    class: item_template.class.clone(),
                    subclass: item_template.subclass.clone(),
                    image: item_template.image.clone(),
                    weight: item_template.weight,
                    equipped: false,
                    price: None,
                    attrs: Some(attrs),
                    produces: None,
                };

                send_to_client(*player_id, info_item_packet, &clients);
            }
            PlayerEvent::InfoStructureRefineItem {
                player_id,
                structure_id,
                item_id,
            } => {
                debug!("PlayerEvent::InfoStructureRefineItem player_id: {:?} structure_id: {:?} item_id: {:?}", player_id, structure_id, item_id);
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find entity for {:?}", structure_id);
                    break;
                };

                let Ok((_pid, _obj_name, _obj_template, inventory)) = query.get(entity) else {
                    error!("Cannot find obj template or inventory for {:?}", entity);
                    break;
                };

                let item = inventory.get_by_id(*item_id);

                let Some(item) = item else {
                    error!("Cannot find item for {:?}", item_id);
                    // Send error packet
                    let packet = ResponsePacket::Error {
                        errmsg: "Cannot find item".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                let item_template =
                    Item::get_template(item.name.clone(), &templates.item_templates);

                let Some(produces) = item_template.produces.clone() else {
                    error!("Item is not refinable {:?}", item.name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Item is not refinable".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                let produces_list =
                    item::produced_item_packets(&produces, &templates.item_templates);

                // Get refine time
                let item_template =
                    Item::get_template(item.name.clone(), &templates.item_templates);
                let refine_time = item_template.get_refine_time();

                let info_refine_item_packet: ResponsePacket = ResponsePacket::InfoRefineItem {
                    id: item.id,
                    name: item.name.clone(),
                    image: item.image.clone(),
                    class: item.class.clone(),
                    subclass: item.subclass.clone(),
                    quantity: item.quantity,
                    produces: produces_list,
                    refining_skill: item_template
                        .refine_skill
                        .clone()
                        .expect("Missing refine skill"),
                    refining_skill_req: item_template
                        .refine_skill_req
                        .expect("Missing refine skill req"),
                    refine_time: refine_time / TICKS_PER_SEC,
                    progress: 0,
                };

                send_to_client(*player_id, info_refine_item_packet, &clients);
            }
            PlayerEvent::InfoExit {
                player_id,
                id,
                panel_type,
            } => {
                debug!(
                    "PlayerEvent::InfoExit {:?} {:?} {:?}",
                    player_id, id, panel_type
                );
                events_to_remove.push(*event_id);

                match panel_type.as_str() {
                    "inventory" => {
                        active_infos.remove((*id, ActiveInfoType::Inventory), *player_id);
                    }
                    "equip" => {
                        active_infos.remove((*id, ActiveInfoType::Equip), *player_id);
                    }
                    "craft" => {
                        active_infos.remove((*id, ActiveInfoType::Craft), *player_id);
                    }
                    "structure_refine" => {
                        active_infos.remove((*id, ActiveInfoType::StructureRefine), *player_id);
                    }
                    "structure_craft" => {
                        active_infos.remove((*id, ActiveInfoType::StructureCraft), *player_id);
                    }
                    "structure_queue" => {
                        active_infos.remove((*id, ActiveInfoType::StructureQueue), *player_id);
                    }
                    "villager" => {
                        active_infos.remove((*id, ActiveInfoType::Obj), *player_id);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn item_transfer_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    run_spawned_objs: Res<RunSpawnedObjs>,
    investigated_pois: Res<InvestigatedPOIs>,
    mut active_infos: ResMut<ActiveInfos>,
    mut query: Query<ItemTransferQuery>,
    selected_upgrade_query: Query<&SelectedUpgrade>,
    game_tick: Res<GameTick>,
    mut game_events: ResMut<GameEvents>,
    presence: Res<PlayerWorldPresenceState>,
    mut objectives: ResMut<Objectives>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::LootAll {
                player_id,
                source_id,
                target_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                if source_id == target_id {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Cannot loot items into the same inventory.".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                let Some(source_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find loot source entity from id: {:?}", source_id);
                    continue;
                };
                let Some(target_entity) = entity_map.get_entity(*target_id) else {
                    error!("Cannot find loot target entity from id: {:?}", target_id);
                    continue;
                };

                let entities = [source_entity, target_entity];
                let Ok([mut source, mut target]) = query.get_many_mut(entities) else {
                    error!(
                        "Cannot find loot source or target from entities {:?}",
                        entities
                    );
                    continue;
                };

                if !can_access_run_shipwreck(
                    *player_id,
                    source.id.0,
                    source.template,
                    &run_spawned_objs,
                ) {
                    send_shipwreck_owner_error(*player_id, &clients);
                    continue;
                }
                if !can_access_shipwreck_inventory(
                    *player_id,
                    source.id.0,
                    source.template,
                    &investigated_pois,
                ) {
                    send_shipwreck_search_error(*player_id, &clients);
                    continue;
                }

                if is_owner_offline_protected(source.player_id, &presence)
                    || is_owner_offline_protected(target.player_id, &presence)
                    || object_belongs_to_protected_run(source.id.0, &ids, &presence)
                    || object_belongs_to_protected_run(target.id.0, &ids, &presence)
                {
                    continue;
                }

                if !is_loot_all_source(
                    *player_id,
                    source.player_id.0,
                    source.template,
                    source.state,
                ) {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg:
                                "Loot All is only available for enemy corpses and dropped bags."
                                    .to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                if target.player_id.0 != *player_id
                    || !target.subclass.is_hero()
                    || !target.state.is_alive()
                {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Loot All can only transfer items to your living hero."
                                .to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                if !Map::is_adjacent_including_source(*source.pos, *target.pos) {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Loot is not nearby.".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                let target_capacity =
                    Obj::get_capacity(&target.template.0, &templates.obj_templates);

                let transferred = transfer_loot_that_fits(
                    &mut source.inventory,
                    &mut target.inventory,
                    target_capacity,
                );

                if transferred > 0
                    && is_loot_poi(&source.template.0)
                    && source.inventory.items.is_empty()
                {
                    let despawn_event_id = ids.new_map_event_id();
                    game_events.insert(
                        despawn_event_id,
                        GameEvent {
                            event_id: despawn_event_id,
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + LOOT_POI_EMPTY_DESPAWN_TICKS,
                            event_type: GameEventType::DespawnObj {
                                obj_id: source.id.0,
                            },
                        },
                    );
                }

                let result = if source.inventory.items.is_empty() {
                    "success"
                } else if transferred > 0 {
                    "partial"
                } else {
                    "full"
                };

                let source_capacity =
                    Obj::get_capacity(&source.template.0, &templates.obj_templates);
                let source_inventory = network::Inventory {
                    id: source.id.0,
                    cap: source_capacity,
                    tw: source.inventory.get_total_weight(),
                    items: source.inventory.get_packet(),
                };
                let target_inventory = network::Inventory {
                    id: target.id.0,
                    cap: target_capacity,
                    tw: target.inventory.get_total_weight(),
                    items: target.inventory.get_packet(),
                };

                send_to_client(
                    *player_id,
                    ResponsePacket::ItemTransfer {
                        result: result.to_string(),
                        source_id: source.id.0,
                        sourceitems: source_inventory,
                        target_id: target.id.0,
                        targetitems: target_inventory,
                        reqitems: Vec::new(),
                    },
                    &clients,
                );
            }
            PlayerEvent::ItemTransfer {
                player_id,
                source_id,
                target_id,
                item_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(owner_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find owner entity from id: {:?}", source_id);
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(*target_id) else {
                    error!("Cannot find target entity from id: {:?}", target_id);
                    continue;
                };

                let entities = [owner_entity, target_entity];

                let Ok([mut owner, mut target]) = query.get_many_mut(entities) else {
                    error!("Cannot find owner or target from entities {:?}", entities);
                    continue;
                };

                if !can_access_run_shipwreck(
                    *player_id,
                    owner.id.0,
                    owner.template,
                    &run_spawned_objs,
                ) || !can_access_run_shipwreck(
                    *player_id,
                    target.id.0,
                    target.template,
                    &run_spawned_objs,
                ) {
                    send_shipwreck_owner_error(*player_id, &clients);
                    continue;
                }
                if !can_access_shipwreck_inventory(
                    *player_id,
                    owner.id.0,
                    owner.template,
                    &investigated_pois,
                ) || !can_access_shipwreck_inventory(
                    *player_id,
                    target.id.0,
                    target.template,
                    &investigated_pois,
                ) {
                    send_shipwreck_search_error(*player_id, &clients);
                    continue;
                }

                if is_owner_offline_protected(owner.player_id, &presence)
                    || is_owner_offline_protected(target.player_id, &presence)
                    || object_belongs_to_protected_run(owner.id.0, &ids, &presence)
                    || object_belongs_to_protected_run(target.id.0, &ids, &presence)
                {
                    continue;
                }

                let Some(item) = owner.inventory.get_by_id(*item_id) else {
                    error!("Cannot find item for {:?}", item_id);
                    continue;
                };

                // Item has to be nearby
                debug!(
                    "owner.pos: {:?} target.pos {:?} is_adjacent: {:?}",
                    owner.pos,
                    target.pos,
                    Map::is_adjacent_including_source(*owner.pos, *target.pos)
                );
                if !(owner.pos == target.pos
                    || Map::is_adjacent_including_source(*owner.pos, *target.pos))
                {
                    let packet = ResponsePacket::Error {
                        errmsg: "Item is not nearby.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Transfer target is not dead
                if *target.state == State::Dead {
                    let packet = ResponsePacket::Error {
                        errmsg: "Cannot transfer items to the dead or destroyed".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Cannot take items from tax collector, only transfer to
                if Obj::has_group(GROUP_TAX_COLLECTOR, owner.misc.groups.clone()) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Cannot transfer items from tax collector".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Cannot take items from monolith
                if owner.subclass.is_monolith() {
                    let packet = ResponsePacket::Error {
                        errmsg: "Cannot transfer items from monolith".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Only allow soulshards to be transferred to monolith
                if target.subclass.is_monolith() && item.class != item::SOULSHARD {
                    let packet = ResponsePacket::Error {
                        errmsg: "Only soulshards can be transferred to monolith".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let target_accepts_build_resources =
                    accepts_build_resource_transfer(target.class, target.state);
                let owner_accepts_build_resources =
                    accepts_build_resource_transfer(owner.class, owner.state);

                // Completed Campfires and Shelter Tents hold only cooking fuel,
                // raw meat, and cooked meat rather than acting as general storage.
                // Founded structures and structures being upgraded still use
                // the build-resource path below.
                if !target_accepts_build_resources
                    && !accepts_completed_storage_item(
                        &target.template.0,
                        &item.name,
                        &item.subclass,
                    )
                {
                    let packet = ResponsePacket::Error {
                        errmsg:
                            "Only Firewood, Charcoal, Raw Meat, and Cooked Meat can be stored here."
                                .to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Incomplete structures can receive build/upgrade resources, but should not be
                // used like completed inventories.
                if incomplete_structure_blocks_inventory_transfer(target.class, target.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure is not completed.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Transfer target does not have enough capacity
                let target_total_weight = target.inventory.get_total_weight();
                let transfer_item_weight = (item.quantity as f32 * item.weight) as i32;
                let target_capacity =
                    Obj::get_capacity(&target.template.0, &templates.obj_templates);
                let target_dropped_bag_expiry =
                    target.dropped_bag.as_deref().map(|bag| bag.expires_at);
                if target_dropped_bag_expiry.is_some_and(|expires_at| expires_at <= game_tick.0) {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "That dropped bag has already expired.".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                info!(
                    "Item transfer target.class: {:?} target.template: {:?}",
                    target.class.0, target.template.0
                );

                // Structure founded and under construction use case
                if target_accepts_build_resources {
                    info!("Transfering to target structure with state founded.");

                    let mut req = Vec::new();

                    if *target.state == State::Founded {
                        let structure_template = templates
                            .obj_templates
                            .get_by_name_template(target.name.0.clone(), target.template.0.clone());

                        req = structure_template
                            .req
                            .expect("Structure template missing req");
                    } else if *target.state == State::PlanningUpgrade {
                        // Get selected upgrade
                        let Ok(selected_upgrade) = selected_upgrade_query.get(target.entity) else {
                            error!("Cannot find selected upgrade for {:?}", target.entity);
                            continue;
                        };

                        let structure_template = Structure::get_template(
                            selected_upgrade.0.clone(),
                            &templates.obj_templates,
                        );

                        req = structure_template
                            .expect("Cannot find upgrade structure template")
                            .upgrade_req
                            .unwrap_or(vec![]);
                    }

                    // Check if item is required for structure construction
                    if !Item::is_req_for_build(item.clone(), req.clone()) {
                        info!("Item not required for construction: {:?}", item);
                        let packet = ResponsePacket::Error {
                            errmsg: "Item not required for construction.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }

                    let mut req_items = target.inventory.process_req_items_for_build(req);

                    // Find first matching req item (substitution-aware)
                    let matching_req_item = req_items.iter_mut().find(|r| {
                        crate::item::req_matches_build(
                            &r.req_type,
                            &item.name,
                            &item.class,
                            &item.subclass,
                        )
                    });

                    if let Some(matching_req_item) = matching_req_item {
                        if let Some(match_req_item_cquantity) = &mut matching_req_item.cquantity {
                            if *match_req_item_cquantity > 0 {
                                if *match_req_item_cquantity == item.quantity {
                                    // Transfer entire item
                                    Inventory::transfer(
                                        item.id,
                                        &mut owner.inventory,
                                        &mut target.inventory,
                                    );

                                    // Set current quantity to 0
                                    *match_req_item_cquantity = 0;
                                } else if *match_req_item_cquantity > item.quantity {
                                    // Transfer entire item
                                    Inventory::transfer(
                                        item.id,
                                        &mut owner.inventory,
                                        &mut target.inventory,
                                    );

                                    // Subtract current quantity
                                    *match_req_item_cquantity -= item.quantity;
                                } else if *match_req_item_cquantity < item.quantity {
                                    // Split to create new item. Required here as item quantity is greater than req quantity
                                    if let Some((new_split_item, _)) = owner.inventory.split(
                                        item.id,
                                        ids.new_item_id(),
                                        *match_req_item_cquantity,
                                        &templates.item_templates.clone(),
                                    ) {
                                        // Transfer the new item
                                        Inventory::transfer(
                                            new_split_item.id,
                                            &mut owner.inventory,
                                            &mut target.inventory,
                                        );

                                        // Set current quantity to 0
                                        *match_req_item_cquantity = 0;
                                    }
                                }
                            }
                        } else {
                            error!("Matching current quantity is unexpected None.")
                        }
                    } else {
                        error!("Item transfer is invalid due to lack of matching req item")
                    }

                    if req_items.len() == 0 {
                        let packet = ResponsePacket::Error {
                            errmsg: "All structure item requirements met.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }

                    let source_capacity =
                        Obj::get_capacity(&owner.template.0, &templates.obj_templates);
                    let source_total_weight = owner.inventory.get_total_weight();

                    let source_inventory = network::Inventory {
                        id: owner.id.0,
                        cap: source_capacity,
                        tw: source_total_weight,
                        items: owner.inventory.get_packet().clone(),
                    };

                    let target_inventory = network::Inventory {
                        id: *target_id,
                        cap: target_capacity,
                        tw: (target_total_weight + transfer_item_weight),
                        items: target.inventory.get_packet().clone(),
                    };

                    let item_transfer_packet: ResponsePacket = ResponsePacket::ItemTransfer {
                        result: "success".to_string(),
                        source_id: owner.id.0,
                        sourceitems: source_inventory,
                        target_id: *target_id,
                        targetitems: target_inventory,
                        reqitems: req_items,
                    };

                    send_to_client(*player_id, item_transfer_packet, &clients);
                } else if owner_accepts_build_resources {
                    info!("Transfering from owner structure with state founded.");

                    let mut req = Vec::new();

                    if *owner.state == State::Founded {
                        let structure_template = templates
                            .obj_templates
                            .get_by_name_template(owner.name.0.clone(), owner.template.0.clone());

                        req = structure_template
                            .req
                            .expect("Structure template missing req");
                    } else if *owner.state == State::PlanningUpgrade {
                        // Get selected upgrade
                        let Ok(selected_upgrade) = selected_upgrade_query.get(owner.entity) else {
                            error!("Cannot find selected upgrade for {:?}", owner.entity);
                            continue;
                        };

                        let structure_template = Structure::get_template(
                            selected_upgrade.0.clone(),
                            &templates.obj_templates,
                        );

                        req = structure_template
                            .expect("Cannot find upgrade structure template")
                            .upgrade_req
                            .unwrap_or(vec![]);
                    }

                    Inventory::transfer(item.id, &mut owner.inventory, &mut target.inventory);

                    let req_items = owner.inventory.process_req_items_for_build(req);

                    let source_capacity =
                        Obj::get_capacity(&owner.template.0, &templates.obj_templates);
                    let source_total_weight = owner.inventory.get_total_weight();

                    let source_items = owner.inventory.get_packet().clone();
                    let target_items = target.inventory.get_packet().clone();

                    let source_inventory = network::Inventory {
                        id: owner.id.0,
                        cap: source_capacity,
                        tw: source_total_weight,
                        items: source_items.clone(),
                    };

                    let target_inventory = network::Inventory {
                        id: *target_id,
                        cap: target_capacity,
                        tw: target_total_weight + transfer_item_weight,
                        items: target_items.clone(),
                    };

                    let item_transfer_packet: ResponsePacket = ResponsePacket::ItemTransfer {
                        result: "success".to_string(),
                        source_id: owner.id.0,
                        sourceitems: source_inventory,
                        target_id: *target_id,
                        targetitems: target_inventory,
                        reqitems: req_items,
                    };

                    send_to_client(*player_id, item_transfer_packet, &clients);
                } else if is_restricted_cooking_storage(&target.template.0) {
                    info!("Transferring cooking supplies into restricted storage");

                    let remaining_capacity = target_capacity - target_total_weight;
                    if remaining_capacity <= 0 {
                        let packet = ResponsePacket::Error {
                            errmsg: "Target does not have enough capacity".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }

                    let item_weight = item.weight.max(0.0);
                    let num_to_transfer = if item_weight > 0.0 {
                        ((remaining_capacity as f32 / item_weight).floor() as i32)
                            .min(item.quantity)
                    } else {
                        item.quantity
                    };

                    if num_to_transfer <= 0 {
                        let packet = ResponsePacket::Error {
                            errmsg: "Target does not have enough capacity".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }

                    if num_to_transfer < item.quantity {
                        Inventory::transfer_quantity(
                            item.id,
                            ids.new_item_id(),
                            &mut owner.inventory,
                            &mut target.inventory,
                            num_to_transfer,
                            &templates.item_templates,
                        );
                    } else {
                        Inventory::transfer(item.id, &mut owner.inventory, &mut target.inventory);
                    }

                    let source_inventory = network::Inventory {
                        id: owner.id.0,
                        cap: Obj::get_capacity(&owner.template.0, &templates.obj_templates),
                        tw: owner.inventory.get_total_weight(),
                        items: owner.inventory.get_packet(),
                    };
                    let target_inventory = network::Inventory {
                        id: *target_id,
                        cap: target_capacity,
                        tw: target.inventory.get_total_weight(),
                        items: target.inventory.get_packet(),
                    };

                    send_to_client(
                        *player_id,
                        ResponsePacket::ItemTransfer {
                            result: if num_to_transfer < item.quantity {
                                "partial".to_string()
                            } else {
                                "success".to_string()
                            },
                            source_id: owner.id.0,
                            sourceitems: source_inventory,
                            target_id: *target_id,
                            targetitems: target_inventory,
                            reqitems: Vec::new(),
                        },
                        &clients,
                    );
                } else {
                    if target_total_weight + transfer_item_weight > target_capacity {
                        let packet = ResponsePacket::Error {
                            errmsg: "Target does not have enough capacity".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }

                    info!("Transfering item from owner to target");
                    info!("Owner inventory: {:?}", owner.inventory);
                    info!("Target inventory: {:?}", target.inventory);
                    Inventory::transfer(item.id, &mut owner.inventory, &mut target.inventory);

                    if let Some(fixed_expires_at) = target_dropped_bag_expiry {
                        if let Some(dropped_bag) = target.dropped_bag.as_deref_mut() {
                            dropped_bag.add_contributor(*player_id);
                        }
                        cancel_empty_dropped_bag_despawn(
                            &mut game_events,
                            target.id.0,
                            fixed_expires_at,
                        );
                    }

                    // A loot cache that has just been emptied should despawn shortly
                    // after, leaving a brief beat so the player sees it go empty.
                    if is_loot_poi(&owner.template.0) && owner.inventory.items.is_empty() {
                        let despawn_event_id = ids.new_map_event_id();
                        game_events.insert(
                            despawn_event_id,
                            GameEvent {
                                event_id: despawn_event_id,
                                start_tick: game_tick.0,
                                run_tick: game_tick.0 + LOOT_POI_EMPTY_DESPAWN_TICKS,
                                event_type: GameEventType::DespawnObj { obj_id: owner.id.0 },
                            },
                        );
                    }

                    let mut target_items_updated: Vec<Item> = Vec::new();
                    if owner.subclass.is_hero()
                        && target.subclass.is_villager()
                        && target.player_id.0 == *player_id
                    {
                        let gather_context = target.order.and_then(|order| match order {
                            Order::Gather { res_type, pos, .. } => Some((res_type.as_str(), *pos)),
                            _ => None,
                        });
                        let gather_res_type = gather_context.map(|(res_type, _pos)| res_type);

                        target_items_updated = target
                            .inventory
                            .auto_equip_item_for_context(item.id, gather_res_type);

                        let transferred_item_satisfies_block = gather_res_type
                            .and_then(item::required_tool_attr_for_res_type)
                            .and_then(|required_attr| {
                                target.inventory.get_by_id(item.id).map(|transferred_item| {
                                    transferred_item.equipped
                                        && transferred_item.is_gather_tool_for_attr(&required_attr)
                                })
                            })
                            .unwrap_or(false);

                        if transferred_item_satisfies_block
                            && (target.blocked_work.is_some() || target.tool_fetch_target.is_some())
                        {
                            commands.entity(target.entity).remove::<BlockedWork>();
                            commands.entity(target.entity).remove::<ToolFetchTarget>();

                            if let Some((_res_type, gather_pos)) = gather_context {
                                commands
                                    .entity(target.entity)
                                    .insert(Destination { pos: gather_pos });
                            }

                            if let (Some(order), Some(active_task)) =
                                (target.order, target.active_task.as_mut())
                            {
                                ActiveTask::set_if_changed(
                                    active_task,
                                    VillagerUtil::order_to_activity(order),
                                );
                            }
                        }
                    }

                    info!("Owner inventory after transfer: {:?}", owner.inventory);
                    info!("Target inventory after transfer: {:?}", target.inventory);

                    let structure_template = templates
                        .obj_templates
                        .get_by_name_template(owner.name.0.clone(), owner.template.0.clone());

                    let req_items = if let Some(req) = structure_template.req {
                        owner.inventory.process_req_items_for_build(req)
                    } else {
                        Vec::new()
                    };

                    let source_capacity =
                        Obj::get_capacity(&owner.template.0, &templates.obj_templates);
                    let source_total_weight = owner.inventory.get_total_weight();

                    let source_inventory = network::Inventory {
                        id: owner.id.0,
                        cap: source_capacity,
                        tw: source_total_weight,
                        items: owner.inventory.get_packet().clone(),
                    };

                    let target_inventory = network::Inventory {
                        id: *target_id,
                        cap: target_capacity,
                        tw: target_total_weight + transfer_item_weight,
                        items: target.inventory.get_packet().clone(),
                    };

                    let item_transfer_packet: ResponsePacket = ResponsePacket::ItemTransfer {
                        result: "success".to_string(),
                        source_id: owner.id.0,
                        sourceitems: source_inventory,
                        target_id: *target_id,
                        targetitems: target_inventory,
                        reqitems: req_items,
                    };

                    send_to_client(*player_id, item_transfer_packet, &clients);

                    if !target_items_updated.is_empty() {
                        let item_update_packet: ResponsePacket = ResponsePacket::InfoItemsUpdate {
                            id: target.id.0,
                            items_updated: target.inventory.get_packet(),
                            items_removed: Vec::new(),
                        };

                        send_to_client(*player_id, item_update_packet, &clients);
                    }
                }

                if target.player_id.0 == *player_id
                    && target.template.0 == "Burrow"
                    && Structure::is_built(*target.state)
                    && burrow_supply_type_count(&target.inventory) >= BURROW_SUPPLY_GOAL
                {
                    objectives
                        .entry(*player_id)
                        .or_insert_with(PlayerObjectives::default)
                        .stock_burrow = true;
                }
            }
            PlayerEvent::InfoItemTransfer {
                player_id,
                source_id,
                target_id,
            } => {
                events_to_remove.push(*event_id);

                debug!(
                    "PlayerEvent::InfoItemTransfer source_id: {:?} target_id: {:?}",
                    *source_id, *target_id
                );

                if source_id == target_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Cannot transfer items to self".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Some(source_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find source entity from id: {:?}", source_id);
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(*target_id) else {
                    error!("Cannot find target entity from id: {:?}", target_id);
                    continue;
                };

                let entities = [source_entity, target_entity];

                let Ok([source, target]) = query.get_many(entities) else {
                    error!("Cannot find source or target from entities {:?}", entities);
                    continue;
                };

                if !can_access_run_shipwreck(
                    *player_id,
                    source.id.0,
                    source.template,
                    &run_spawned_objs,
                ) || !can_access_run_shipwreck(
                    *player_id,
                    target.id.0,
                    target.template,
                    &run_spawned_objs,
                ) {
                    send_shipwreck_owner_error(*player_id, &clients);
                    continue;
                }
                if !can_access_shipwreck_inventory(
                    *player_id,
                    source.id.0,
                    source.template,
                    &investigated_pois,
                ) || !can_access_shipwreck_inventory(
                    *player_id,
                    target.id.0,
                    target.template,
                    &investigated_pois,
                ) {
                    send_shipwreck_search_error(*player_id, &clients);
                    continue;
                }

                if !Map::is_adjacent_including_source(*source.pos, *target.pos) {
                    error!("Target is not nearby {:?}", target.id.0);
                    let packet = ResponsePacket::Error {
                        errmsg: "Target is not nearby".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if target.player_id.0 != *player_id
                    && *target.state != State::Dead
                    && *target.subclass != Subclass::Merchant
                    && *target.subclass != Subclass::Monolith
                    && *target.subclass != Subclass::Poi
                    && !Obj::has_group(GROUP_TAX_COLLECTOR, (*target.misc.groups).to_vec())
                {
                    error!("Cannot transfer items with this target {:?}", target.id.0);
                    let packet = ResponsePacket::Error {
                        errmsg: "Cannot transfer items with this unit".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let source_capacity =
                    Obj::get_capacity(&source.template.0, &templates.obj_templates);
                let source_total_weight = source.inventory.get_total_weight();

                let mut target_capacity = -1; // -1 representing unknown
                let mut target_total_weight = -1; // -1 representing unknown
                let mut selected_upgrade_name = None;

                if target.player_id.0 == *player_id
                    || target.template.0 == templates::DROPPED_BAG_TEMPLATE
                {
                    target_capacity =
                        Obj::get_capacity(&target.template.0, &templates.obj_templates);
                    target_total_weight = target.inventory.get_total_weight();
                }

                if let Ok(selected_upgrade) = selected_upgrade_query.get(target.entity) {
                    selected_upgrade_name = Some(selected_upgrade.0.clone());
                }

                let source_items = source.inventory.get_packet().clone();
                let target_items;

                let mut target_filter = Vec::new();

                if target.subclass.is_merchant() {
                    target_filter.push(item::GOLD.to_string());
                    target_items = target.inventory.get_packet_filter(target_filter);
                } else if target.subclass.is_monolith() {
                    target_filter.push(item::SOULSHARD.to_string());
                    target_items = target.inventory.get_packet_filter(target_filter);
                } else if Obj::has_group(GROUP_TAX_COLLECTOR, (*target.misc.groups).to_vec()) {
                    target_filter.push(item::FILTER_ALL.to_string());
                    target_items = target.inventory.get_packet_filter(target_filter);
                } else {
                    target_items = target.inventory.get_packet().clone();
                }

                let source_inventory = network::Inventory {
                    id: *source_id,
                    cap: source_capacity,
                    tw: source_total_weight,
                    items: source_items,
                };

                let target_player_id = target.player_id.clone();

                let target_inventory = network::Inventory {
                    id: *target_id,
                    cap: target_capacity,
                    tw: target_total_weight,
                    items: target_items.clone(),
                };

                let req_items = Structure::get_current_req_quantities(
                    target.template.0.clone(),
                    target.class.0.clone(),
                    target.state.clone(),
                    &target.inventory,
                    &templates,
                    selected_upgrade_name,
                );

                let info_item_transfer_packet: ResponsePacket = ResponsePacket::InfoItemTransfer {
                    source_id: *source_id,
                    sourceitems: source_inventory,
                    source_expires_in: source
                        .dropped_bag
                        .as_deref()
                        .map(|bag| dropped_bag_expires_in(bag.expires_at, game_tick.0)),
                    target_id: *target_id,
                    targetitems: target_inventory,
                    target_expires_in: target
                        .dropped_bag
                        .as_deref()
                        .map(|bag| dropped_bag_expires_in(bag.expires_at, game_tick.0)),
                    reqitems: req_items,
                };

                send_to_client(*player_id, info_item_transfer_packet, &clients);

                active_infos.add((*source_id, ActiveInfoType::ItemTransfer), *player_id);
                active_infos.add((*target_id, ActiveInfoType::ItemTransfer), *player_id);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn item_split_system(
    mut events: ResMut<PlayerEvents>,
    mut ids: ResMut<Ids>,
    entity_map: ResMut<EntityObjMap>,
    clients: Res<Clients>,
    templates: Res<Templates>,
    run_spawned_objs: Res<RunSpawnedObjs>,
    investigated_pois: Res<InvestigatedPOIs>,
    mut query: Query<(&PlayerId, &Template, &mut Inventory)>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::ItemSplit {
                player_id,
                owner_id,
                item_id,
                quantity,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                // Check if quantity is zero
                if *quantity == 0 {
                    let packet = ResponsePacket::Error {
                        errmsg: "Quantity cannot be zero".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Some(owner_entity) = entity_map.get_entity(*owner_id) else {
                    error!("Cannot find owner entity for owner {:?}", *owner_id);
                    continue;
                };

                let Ok((owner_player_id, owner_template, mut owner_inventory)) =
                    query.get_mut(owner_entity)
                else {
                    error!("Cannot find owner inventory for {:?}", owner_entity);
                    continue;
                };

                let owns_object = owner_player_id.0 == *player_id;
                let owns_run_shipwreck = owner_template.0 == "Shipwreck"
                    && run_spawned_objs.contains_for_player(*player_id, *owner_id);
                if !owns_object && !owns_run_shipwreck {
                    error!("Owner is not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Owner is not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }
                if !can_access_shipwreck_inventory(
                    *player_id,
                    *owner_id,
                    owner_template,
                    &investigated_pois,
                ) {
                    send_shipwreck_search_error(*player_id, &clients);
                    continue;
                }

                let Some(item) = owner_inventory.get_by_id(*item_id) else {
                    error!("Cannot find item for {:?}", *item_id);
                    continue;
                };

                // Check if quantity is more than item quantity
                if item.quantity < *quantity {
                    let packet = ResponsePacket::Error {
                        errmsg: "Split quantity is more than item quantity".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                owner_inventory.split(
                    *item_id,
                    ids.new_item_id(),
                    *quantity,
                    &templates.item_templates,
                );

                let item_split_packet: ResponsePacket = ResponsePacket::ItemSplit {
                    result: "success".to_string(),
                    owner: item.owner,
                };

                send_to_client(*player_id, item_split_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_experiment_system(
    mut events: ResMut<PlayerEvents>,
    _game_tick: ResMut<GameTick>,
    entity_map: ResMut<EntityObjMap>,
    clients: Res<Clients>,
    experiments: Res<Experiments>,
    query: Query<CoreQuery>,
    templates: Res<Templates>,
    mut active_infos: ResMut<ActiveInfos>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoExperinment {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure for {:?}", structure_id);
                    continue;
                };

                let Ok(structure) = query.get(structure_entity) else {
                    error!("Cannot find structure for {:?}", structure_entity);
                    continue;
                };

                if structure.player_id.0 != *player_id {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let info_experiment;
                let (experiment_source, experiment_reagents, other_resources) =
                    structure.inventory.get_experiment_details_packet();

                if let Some(experiment) = experiments.get(structure_id) {
                    info_experiment = ResponsePacket::InfoExperiment {
                        id: *structure_id,
                        expitem: experiment_source,
                        expresources: experiment_reagents,
                        validresources: other_resources,
                        expstate: Experiment::state_to_string(experiment.state.clone()),
                        recipe: Experiment::recipe_to_packet(experiment.clone(), &templates),
                    };
                } else {
                    info_experiment = ResponsePacket::InfoExperiment {
                        id: *structure_id,
                        expitem: experiment_source,
                        expresources: experiment_reagents,
                        validresources: other_resources,
                        expstate: experiment::EXP_STATE_NONE.to_string(),
                        recipe: None,
                    };
                }

                active_infos.add((*structure_id, ActiveInfoType::Experiment), *player_id);

                send_to_client(*player_id, info_experiment, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_merchant_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    entity_map: ResMut<EntityObjMap>,
    templates: Res<Templates>,
    query: Query<&Merchant>,
    prices: ResMut<Prices>,
    template_inventory_query: Query<(&Template, &Inventory)>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoMerchant {
                player_id,
                source_id,
                merchant_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(merchant_entity) = entity_map.get_entity(*merchant_id) else {
                    error!("Cannot find entity for {:?}", merchant_id);
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find entity for {:?}", source_id);
                    continue;
                };

                let Ok(merchant) = query.get(merchant_entity) else {
                    error!("Cannot find merchant for {:?}", merchant_entity);
                    continue;
                };

                let Ok(
                    [(merchant_template, merchant_inventory), (target_template, target_inventory)],
                ) = template_inventory_query.get_many([merchant_entity, target_entity])
                else {
                    error!(
                        "Cannot find merchant or target template for {:?}",
                        [source_id, merchant_id]
                    );
                    continue;
                };

                let source_capacity =
                    Obj::get_capacity(&target_template.0, &templates.obj_templates);
                let source_total_weight = target_inventory.get_total_weight();
                let source_items = target_inventory.get_packet();

                let source_inventory = network::Inventory {
                    id: *source_id,
                    cap: source_capacity,
                    tw: source_total_weight,
                    items: source_items,
                };

                let merchant_capacity =
                    Obj::get_capacity(&merchant_template.0, &templates.obj_templates);
                let merchant_total_weight = merchant_inventory.get_total_weight();
                let merchant_items =
                    merchant_inventory.get_packet_filter(vec![item::GOLD.to_string()]);

                let merchant_inventory = network::Inventory {
                    id: *merchant_id,
                    cap: merchant_capacity,
                    tw: merchant_total_weight,
                    items: merchant_items,
                };

                // Price discovery is read-only inspection. Build an ephemeral
                // packet view rather than mutating the player-associated
                // merchant cache (important while its run is protected).
                let mut wanted_items = merchant.wanted_items.clone();
                for wanted_item in wanted_items.iter_mut() {
                    let Some(price) = prices.get_buy_price(wanted_item.get_identifier()) else {
                        error!("Cannot find price for {:?}", wanted_item.get_identifier());
                        continue;
                    };

                    let Some(quantity) = prices.get_buy_quantity(wanted_item.get_identifier())
                    else {
                        error!(
                            "Cannot find quantity for {:?}",
                            wanted_item.get_identifier()
                        );
                        continue;
                    };

                    wanted_item.price = price;
                    wanted_item.quantity = quantity;
                }

                let info_merchant = ResponsePacket::InfoMerchant {
                    source_id: *source_id,
                    inventory: source_inventory,
                    merchant_id: *merchant_id,
                    merchant_inventory: merchant_inventory,
                    merchant_wanted_items: wanted_items,
                };

                send_to_client(*player_id, info_merchant, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_hire_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    entity_map: ResMut<EntityObjMap>,
    merchant_query: Query<&Transport, With<Merchant>>,
    query: Query<CoreQuery>,
    attrs_query: Query<(&BaseAttrs, &Skills)>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoHire {
                player_id,
                source_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(merchant_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find entity for {:?}", source_id);
                    break;
                };

                let Ok(merchant) = merchant_query.get(merchant_entity) else {
                    error!("Cannot find obj for {:?}", merchant_entity);
                    break;
                };

                let mut hire_data: Vec<network::HireData> = Vec::new();

                for obj_id in merchant.hauling.iter() {
                    let Some(entity) = entity_map.get_entity(*obj_id) else {
                        error!("Cannot find entity for {:?}", obj_id);
                        break;
                    };

                    let Ok(obj) = query.get(entity) else {
                        error!("Cannot find obj for {:?}", entity);
                        break;
                    };

                    let Ok((attrs, skills)) = attrs_query.get(entity) else {
                        error!("Cannot find attrs for {:?}", entity);
                        break;
                    };

                    let skills = skills.get_levels();

                    let villager_data = network::HireData {
                        id: obj.id.0,
                        name: obj.name.0.clone(),
                        image: obj.misc.image.clone(),
                        wage: 25,
                        creativity: attrs.creativity,
                        dexterity: attrs.dexterity,
                        endurance: attrs.endurance,
                        focus: attrs.focus,
                        intellect: attrs.intellect,
                        spirit: attrs.spirit,
                        strength: attrs.strength,
                        toughness: attrs.toughness,
                        skills: skills,
                    };

                    hire_data.push(villager_data);
                }

                let info_hire = ResponsePacket::InfoHire { data: hire_data };

                send_to_client(*player_id, info_hire, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn order_follow_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: ResMut<GameTick>,
    ids: Res<Ids>,
    entity_map: ResMut<EntityObjMap>,
    mut events: ResMut<PlayerEvents>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    query: Query<ObjQuery>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderFollow {
                player_id,
                source_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    break;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    break;
                };

                // Get hero state
                let mut hero_state = State::None;

                for q in &query {
                    if q.id.0 == hero_id {
                        hero_state = q.state.clone();
                    }
                }

                if Obj::is_dead(&hero_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot give.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Add OrderFollow component to source and set hero_entity as target.
                // This is a final ownership boundary because source_id is
                // supplied by the client.
                let mut ordered = false;
                for q in &query {
                    if q.id.0 == *source_id {
                        if q.player_id.0 != *player_id
                            || is_owner_offline_protected(q.player_id, &presence)
                        {
                            continue;
                        }
                        commands.entity(q.entity).insert(Order::Follow {
                            target: hero_entity,
                        });
                        ordered = true;
                    }
                }

                if !ordered {
                    continue;
                }

                Obj::add_speech_event(
                    game_tick.0,
                    templates.get_dialogue("OrderFollow"),
                    &Id(*source_id),
                    &mut map_events,
                );
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn order_gather_system(
    mut commands: Commands,
    clients: Res<Clients>,
    ids: Res<Ids>,
    entity_map: ResMut<EntityObjMap>,
    game_tick: ResMut<GameTick>,
    mut events: ResMut<PlayerEvents>,
    mut map_events: ResMut<MapEvents>,
    resources: Res<Resources>,
    discoveries: Res<ResourceDiscoveries>,
    templates: Res<Templates>,
    query: Query<CoreQuery>,
    structure_query: Query<
        (&Id, &PlayerId, &Position, &Subclass, &Template, &Inventory),
        With<ClassStructure>,
    >,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderGather {
                player_id,
                source_id,
                res_type,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find entity for {:?}", source_id);
                    continue;
                };

                // Get hero from player
                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                // Get hero entity
                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok([villager, hero]) = query.get_many([entity, hero_entity]) else {
                    error!(
                        "Cannot find villager {:?} or hero {:?}",
                        entity, hero_entity
                    );
                    continue;
                };

                if villager.player_id.0 != *player_id {
                    error!("Villager not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Cannot order another player's villager".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !Resource::is_valid_type_for_player(
                    res_type.to_string(),
                    *hero.pos,
                    &resources,
                    &discoveries,
                    *player_id,
                ) {
                    error!("Invalid resource type {:?}", res_type);
                    let packet = ResponsePacket::Error {
                        errmsg: "Invalid resource type".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Find storage structure & free capacity
                let mut storage_structure_pos = None;
                let mut storage_structure_id = None;

                for (id, owner, pos, subclass, template, inventory) in structure_query.iter() {
                    if owner.0 != *player_id || is_owner_offline_protected(owner, &presence) {
                        continue;
                    }
                    let capacity = Obj::get_capacity(&template.0, &templates.obj_templates);
                    let total_weight = inventory.get_total_weight();

                    if total_weight < capacity {
                        if *subclass == Subclass::Storage {
                            storage_structure_pos = Some(pos.clone());
                            storage_structure_id = Some(id.0);
                        }
                    }
                }

                commands.entity(entity).insert(Order::Gather {
                    res_type: res_type.to_string(),
                    pos: *hero.pos,
                    storage_pos: storage_structure_pos.clone(),
                    storage_id: storage_structure_id.clone(),
                });

                Obj::add_speech_event(
                    game_tick.0,
                    VillagerUtil::order_to_speech(&Order::Gather {
                        res_type: res_type.to_string(),
                        pos: *hero.pos,
                        storage_pos: storage_structure_pos.clone(),
                        storage_id: storage_structure_id.clone(),
                    }),
                    villager.id,
                    &mut map_events,
                );
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn structure_list_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    plans: Res<Plans>,
    templates: Res<Templates>,
    hero_query: Query<(&PlayerId, &Subclass, &Inventory)>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::StructureList { player_id } => {
                events_to_remove.push(*event_id);
                let mut structure_list = Structure::available_to_build(
                    *player_id,
                    plans.clone(),
                    &templates.obj_templates,
                );

                // QW4: annotate each build requirement with how many matching
                // resources the hero is currently carrying, so the build panel
                // can show have/need and flag shortfalls before committing.
                if let Some((_, _, hero_inv)) = hero_query
                    .iter()
                    .find(|(pid, subclass, _)| pid.0 == *player_id && subclass.is_hero())
                {
                    for structure in structure_list.iter_mut() {
                        for req in structure.req.iter_mut() {
                            req.cquantity = Some(hero_inv.count_for_build_req(&req.req_type));
                        }
                    }
                }

                let structure_list = StructureList {
                    result: structure_list,
                };

                let res_packet = ResponsePacket::StructureList(structure_list);

                send_to_client(*player_id, res_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn create_foundation_system(
    mut events: ResMut<PlayerEvents>,
    mut commands: Commands,
    game_tick: ResMut<GameTick>,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    campfire_visibility: Res<CampfireVisibilityState>,
    hero_query: Query<CoreQuery, With<SubclassHero>>,
    structure_query: Query<(&Position, &Subclass), With<ClassStructure>>,
    presence: Res<PlayerWorldPresenceState>,
    crisis_state: Option<Res<SettlementCrisisState>>,
    mut balance_telemetry_state: Option<ResMut<CrisisBalanceTelemetryState>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::CreateFoundation {
                player_id,
                source_id,
                structure_name,
            } => {
                debug!("CreateFoundation");
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                // Validation checks and get hero entity
                let Some(hero_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find hero entity for {:?}", source_id);
                    continue;
                };

                let Ok(hero) = hero_query.get(hero_entity) else {
                    error!("Query failed to find entity {:?}", hero_entity);
                    continue;
                };

                if Obj::is_dead(&hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot build structures.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if hero is owned by player
                if hero.player_id.0 != *player_id {
                    error!("Hero is not owned by player {:?}", *player_id);
                    continue;
                }

                if !has_sufficient_work_visibility(hero.viewshed, *player_id, &campfire_visibility)
                {
                    send_insufficient_work_visibility_notice(*player_id, &clients);
                    continue;
                }

                // Get structure template
                let Some(structure_template) = Structure::get_template_by_name(
                    structure_name.clone(),
                    &templates.obj_templates,
                ) else {
                    let packet = ResponsePacket::Error {
                        errmsg: "Invalid structure name".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                // Check if structure or wall already exists on the tile
                let mut structure_on_tile = false;
                let mut wall_on_tile = false;

                for (existing_pos, existing_subclass) in structure_query.iter() {
                    if hero.pos == existing_pos && *existing_subclass != Subclass::Wall {
                        structure_on_tile = true;
                    } else if hero.pos == existing_pos && *existing_subclass == Subclass::Wall {
                        wall_on_tile = true;
                    }
                }

                if structure_on_tile && structure_template.subclass != SUBCLASS_WALL.to_string() {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure already exists on tile".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if wall_on_tile && structure_template.subclass == SUBCLASS_WALL.to_string() {
                    let packet: ResponsePacket = ResponsePacket::Error {
                        errmsg: "Wall already exists on tile".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let structure_id = ids.new_obj_id();
                let structure_subclass = Subclass::from_str(&structure_template.subclass);

                let structure = Obj {
                    id: Id(structure_id),
                    player_id: PlayerId(*player_id),
                    position: Position {
                        x: hero.pos.x,
                        y: hero.pos.y,
                    },
                    name: Name(structure_name.clone()),
                    template: Template(structure_template.template.clone()),
                    class: Class(structure_template.class),
                    subclass: structure_subclass,
                    state: State::Founded,
                    misc: Misc {
                        image: structure_template.image.clone(),
                        hsl: Vec::new(),
                        groups: Vec::new(),
                    },
                    stats: Stats {
                        hp: 1,
                        base_hp: structure_template.base_hp.unwrap(), // Convert option to non-option
                        stamina: None,
                        mana: None,
                        base_stamina: None,
                        base_mana: None,
                        base_def: 0,
                        base_damage: None,
                        damage_range: None,
                        base_speed: None,
                        base_vision: None,
                    },
                    effects: Effects(HashMap::new()),
                    control_effect_dr: ControlEffectDiminishingReturns::default(),
                    inventory: Inventory {
                        owner: structure_id,
                        items: Vec::new(),
                    },
                    last_combat_tick: LastCombatTick::default(),
                };

                let build_state = BuildUpgradeState {
                    build_upgrade_cost: structure_template.build_cost.unwrap_or(100) as f32,
                    work_done: 0.0,
                    work_per_sec: 0.0,
                    start_time: 0,
                };

                let assignments = Assignments(Vec::new());
                let work_queue = WorkQueue(Vec::new());

                let structure_entity = commands
                    .spawn((
                        structure,
                        build_state,
                        assignments,
                        work_queue,
                        ClassStructure,
                    ))
                    .id();

                ids.new_obj(structure_id, *player_id);
                entity_map.insert(structure_id, structure_entity);

                if matches!(structure_subclass, Subclass::Wall | Subclass::Watchtower)
                    && crisis_state
                        .as_ref()
                        .and_then(|state| state.get(player_id))
                        .is_some_and(|crisis| {
                            crisis.kind == CrisisKind::Goblin
                                && matches!(
                                    crisis.phase,
                                    CrisisPhase::Preparing | CrisisPhase::AssaultReady
                                )
                        })
                {
                    if let Some(telemetry_state) = balance_telemetry_state.as_deref_mut() {
                        telemetry_state
                            .entry(*player_id)
                            .or_default()
                            .preparation_actions
                            .record_defensive_structure_started(structure_id, game_tick.0);
                    }
                }

                // Create a new object event
                commands.trigger(NewObj {
                    entity: structure_entity,
                });

                let packet = ResponsePacket::CreateFoundation {
                    result: "success".to_string(),
                };

                send_to_client(*player_id, packet, &clients)
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn build_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: ResMut<GameTick>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    campfire_visibility: Res<CampfireVisibilityState>,
    builder_query: Query<(
        &Position,
        &State,
        Option<&Viewshed>,
        Option<&LastCombatTick>,
    )>,
    mut structure_query: Query<(&Name, &Position, &State, &Inventory, &mut Assignments)>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Build {
                player_id,
                builder_id,
                structure_id,
            } => {
                debug!("PlayerEvent::Build");
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                // Validation checks and get builder and structure entities
                let Some(builder_entity) = entity_map.get_entity(*builder_id) else {
                    error!("Cannot find builder entity for {:?}", builder_id);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Some(builder_player_id) = ids.get_player(*builder_id) else {
                    error!("Cannot find player for {:?}", builder_id);
                    continue;
                };

                let Some(structure_player_id) = ids.get_player(*structure_id) else {
                    error!("Cannot find structure player for {:?}", structure_id);
                    continue;
                };

                if builder_player_id != *player_id {
                    error!("Builder is not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Builder is not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if structure_player_id != *player_id {
                    error!("Structure is not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure is not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Ok((builder_pos, builder_state, builder_viewshed, last_combat_tick)) =
                    builder_query.get(builder_entity)
                else {
                    error!("Cannot find builder for {:?}", builder_id);
                    continue;
                };

                if ids.get_hero(*player_id) == Some(*builder_id)
                    && !has_sufficient_work_visibility(
                        builder_viewshed,
                        *player_id,
                        &campfire_visibility,
                    )
                {
                    send_insufficient_work_visibility_notice(*player_id, &clients);
                    continue;
                }

                if combat_locked(last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                let Ok((
                    structure_name,
                    structure_pos,
                    structure_state,
                    structure_inventory,
                    mut structure_assignments,
                )) = structure_query.get_mut(structure_entity)
                else {
                    error!("Cannot find structure for {:?}", structure_id);
                    continue;
                };

                if *structure_state == State::Founded {
                    let structure_template = templates.obj_templates.get(structure_name.0.clone());

                    let structure_req = structure_template
                        .req
                        .expect("Template should have req field");

                    // Check if structure is missing required items
                    if !structure_inventory.has_reqs_for_build(structure_req.clone()) {
                        let packet = ResponsePacket::Error {
                            errmsg: "Structure is missing required items.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }
                }

                if builder_pos != structure_pos {
                    error!("Builder is not on the structure {:?}", *structure_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Builder must be on the structure to build it.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Automatically assign the hero to the structure
                commands.entity(builder_entity).insert(Assignment {
                    structure_id: *structure_id,
                    structure_name: structure_name.0.to_string(),
                    structure_pos: *structure_pos,
                });

                // Add assignment to assignments on structure
                if !structure_assignments.0.contains(&builder_id) {
                    structure_assignments.0.push(*builder_id);
                }

                info!("Adding trigger to start build");
                commands.trigger(StartBuild {
                    entity: structure_entity,
                    builder_entity: builder_entity,
                });
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn start_upgrade_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    ids: Res<Ids>,
    game_tick: ResMut<GameTick>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut structure_query: Query<
        (
            &PlayerId,
            &Id,
            &Name,
            &Position,
            &State,
            &Template,
            &Inventory,
        ),
        With<ClassStructure>,
    >,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::StartUpgrade {
                player_id,
                structure_id,
                selected_upgrade,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok((
                    structure_player_id,
                    structure_id,
                    structure_name,
                    structure_pos,
                    structure_state,
                    structure_template,
                    structure_inventory,
                )) = structure_query.get_mut(structure_entity)
                else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                if *player_id != structure_player_id.0 {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if *structure_state != State::None {
                    error!("Structure is not in None state {:?}", structure_id.0);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure cannot be upgraded in this state.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if selected upgrade is valid structure upgrade
                let structure_template = templates.obj_templates.get(structure_template.0.clone());

                // Check if the structure can be upgraded
                let Some(upgrades_to) = structure_template.upgrade_to else {
                    error!("Structure does not have any upgrade_to field");
                    let packet = ResponsePacket::Error {
                        errmsg: "The structure cannot be upgraded".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                // Check if the selected upgrade is valid
                if !upgrades_to.contains(&selected_upgrade) {
                    error!("Invalid upgrade selected {:?}", selected_upgrade);
                    let packet = ResponsePacket::Error {
                        errmsg: "Invalid upgrade selected".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Get upgrade template from templates
                let upgrade_template = templates.obj_templates.get(selected_upgrade.clone());

                let build_state = BuildUpgradeState {
                    build_upgrade_cost: upgrade_template
                        .upgrade_cost
                        .unwrap_or(MAX_BUILD_UPGRADE_COST)
                        as f32,
                    work_done: 0.0,
                    work_per_sec: 0.0,
                    start_time: 0,
                };

                // Insert selected upgrade into structure
                commands
                    .entity(structure_entity)
                    .insert(SelectedUpgrade(selected_upgrade.clone()))
                    .insert(build_state);

                // Change state to planning upgrade
                commands.trigger(StateChange {
                    entity: structure_entity,
                    new_state: State::PlanningUpgrade,
                });

                // Send start upgrade packet to client
                let packet = ResponsePacket::StartUpgrade {
                    structure_id: structure_id.0,
                };
                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn upgrade_system(
    mut events: ResMut<PlayerEvents>,
    mut commands: Commands,
    clients: Res<Clients>,
    ids: Res<Ids>,
    game_tick: ResMut<GameTick>,
    map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    campfire_visibility: Res<CampfireVisibilityState>,
    builder_query: Query<(
        &Position,
        &State,
        Option<&Viewshed>,
        Option<&LastCombatTick>,
    )>,
    mut structure_query: Query<
        (
            &Position,
            &Name,
            &State,
            &Inventory,
            &mut Assignments,
            &SelectedUpgrade,
        ),
        With<ClassStructure>,
    >,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Upgrade {
                player_id,
                builder_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                debug!("PlayerEvent::Upgrade");
                events_to_remove.push(*event_id);

                // Validation checks and get builder and structure entities
                let Some(builder_entity) = entity_map.get_entity(*builder_id) else {
                    error!("Cannot find builder entity for {:?}", builder_id);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Some(builder_player_id) = ids.get_player(*builder_id) else {
                    error!("Cannot find player for {:?}", builder_id);
                    continue;
                };

                let Some(structure_player_id) = ids.get_player(*structure_id) else {
                    error!("Cannot find structure player for {:?}", structure_id);
                    continue;
                };

                if builder_player_id != *player_id {
                    error!("Builder is not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Builder is not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if structure_player_id != *player_id {
                    error!("Structure is not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure is not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Ok((builder_pos, builder_state, builder_viewshed, last_combat_tick)) =
                    builder_query.get(builder_entity)
                else {
                    error!("Cannot find builder for {:?}", builder_id);
                    continue;
                };

                if ids.get_hero(*player_id) == Some(*builder_id)
                    && !has_sufficient_work_visibility(
                        builder_viewshed,
                        *player_id,
                        &campfire_visibility,
                    )
                {
                    send_insufficient_work_visibility_notice(*player_id, &clients);
                    continue;
                }

                if combat_locked(last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                let Ok((
                    structure_pos,
                    structure_name,
                    structure_state,
                    structure_inventory,
                    mut structure_assignments,
                    selected_upgrade,
                )) = structure_query.get_mut(structure_entity)
                else {
                    error!("Cannot find structure for {:?}", structure_id);
                    continue;
                };

                let selected_upgrade_structure_template =
                    templates.obj_templates.get(selected_upgrade.0.clone());

                let structure_upgrade_req = selected_upgrade_structure_template
                    .upgrade_req
                    .expect("Template should have upgrade_req field");

                // Check if structure is missing required items
                if !structure_inventory.has_reqs_for_build(structure_upgrade_req.clone()) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure is missing required items to upgrade.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                info!("Structure state: {:?}", *structure_state);
                if *structure_state != State::PlanningUpgrade {
                    error!(
                        "Structure is not in Planning Upgrade state {:?}",
                        *structure_id
                    );
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure cannot be upgraded in this state.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if builder_pos != structure_pos {
                    error!("Builder is not on the structure {:?}", *structure_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Builder must be on the structure to upgrade it.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Automatically assign the hero to the structure
                commands.entity(builder_entity).insert(Assignment {
                    structure_id: *structure_id,
                    structure_name: structure_name.0.to_string(),
                    structure_pos: *structure_pos,
                });

                // Add assignment to assignments on structure
                if !structure_assignments.0.contains(&builder_id) {
                    structure_assignments.0.push(*builder_id);
                }

                info!("Adding trigger to start build");
                commands.trigger(StartUpgrade {
                    entity: structure_entity,
                    builder_entity: builder_entity,
                });
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn experiment_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    ids: Res<Ids>,
    experiments: ResMut<Experiments>,
    active_infos: Res<ActiveInfos>,
    templates: Res<Templates>,
    //hero_query: Query<CoreQuery, With<SubclassHero>>,
    //structure_query: Query<StructureQuery, With<ClassStructure>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Experiment {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn usable_ignition_tool(inventory: &Inventory, templates: &Templates) -> Option<(i32, i32)> {
    let tool = inventory.get_usable_by_class(IGNITION_TOOL)?;
    let maximum_durability = Item::find_template(tool.name.clone(), &templates.item_templates)
        .and_then(|template| template.durability)
        .or(tool.durability)
        .unwrap_or(1)
        .max(1);

    Some((tool.id, maximum_durability))
}

fn activate_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    ids: Res<Ids>,
    templates: Res<Templates>,
    mut query: Query<(
        &PlayerId,
        &Position,
        &Template,
        &Subclass,
        &State,
        &Stats,
        &mut Inventory,
        Option<&StateDead>,
        Option<&TrueDeath>,
    )>,
    campfire_query: Query<&Campfire>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();
    let mut pending_activations: HashSet<i32> = map_events
        .values()
        .filter_map(|event| match &event.event_type {
            VisibleEvent::ActivateEvent { structure_id } => Some(*structure_id),
            _ => None,
        })
        .collect();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Activate {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok(
                    [(
                        hero_player_id,
                        hero_pos,
                        _,
                        hero_subclass,
                        hero_state,
                        hero_stats,
                        mut hero_inventory,
                        hero_dead,
                        hero_true_death,
                    ), (
                        structure_player_id,
                        structure_pos,
                        structure_template,
                        structure_subclass,
                        structure_state,
                        structure_stats,
                        structure_inventory,
                        structure_dead,
                        structure_true_death,
                    )],
                ) = query.get_many_mut([hero_entity, structure_entity])
                else {
                    error!(
                        "Cannot find hero or structure for {:?}",
                        [hero_entity, structure_entity]
                    );
                    continue;
                };

                if hero_player_id.0 != *player_id
                    || *hero_subclass != Subclass::Hero
                    || hero_dead.is_some()
                    || hero_true_death.is_some()
                    || hero_stats.hp <= 0
                    || !hero_state.is_alive()
                {
                    let packet = ResponsePacket::Error {
                        errmsg: "Only your living hero can light a Campfire".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if *hero_state != State::None {
                    let packet = ResponsePacket::Error {
                        errmsg: "Finish the current action before lighting a Campfire".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let is_standalone_campfire = *structure_subclass == Subclass::Campfire;
                let is_shelter_tent = structure_template.0 == templates::SHELTER_TENT_TEMPLATE;
                // The Shelter Tent retains the upgraded Campfire's ordinary
                // same-or-adjacent tending range while remaining owner-only.
                let out_of_range = if is_standalone_campfire || is_shelter_tent {
                    Map::dist(*hero_pos, *structure_pos) > 1
                } else {
                    hero_pos != structure_pos
                };
                if out_of_range {
                    error!("Hero is not nearby the structure {:?}", *structure_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Hero must be on or adjacent to the Campfire to light it"
                            .to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if structure_player_id.0 != *player_id && !is_standalone_campfire {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let structure_template_data =
                    templates.obj_templates.get(structure_template.0.clone());
                if !structure_template_data.campfire.unwrap_or(false) {
                    let packet = ResponsePacket::Error {
                        errmsg: "This structure cannot be lit".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if structure_dead.is_some()
                    || structure_true_death.is_some()
                    || structure_stats.hp <= 0
                    || !structure_state.is_alive()
                {
                    let packet = ResponsePacket::Error {
                        errmsg: "A destroyed Campfire cannot be lit".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if *structure_state != State::None {
                    error!("Structure is not in None state {:?}", *structure_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure cannot be upgraded in this state.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if pending_activations.contains(structure_id) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Campfire is already being lit".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if the campfire is already lit
                if let Ok(campfire) = campfire_query.get(structure_entity) {
                    if campfire.is_lit {
                        error!("Campfire is already lit {:?}", structure_id);
                        let packet = ResponsePacket::Error {
                            errmsg: "Campfire is already lit".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }
                }

                // Check if structure has fuel
                if !structure_inventory.has_by_class(item::FUEL.to_string()) {
                    error!("Structure does not have fuel {:?}", *structure_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure does not have fuel".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Ignition is paid by the acting hero's directly carried tool.
                let Some((ignition_tool_id, maximum_durability)) =
                    usable_ignition_tool(&hero_inventory, &templates)
                else {
                    let packet = ResponsePacket::Error {
                        errmsg: "You must have a usable Ignition Tool in your inventory"
                            .to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                let ignition_use = hero_inventory
                    .consume_durability_use(ignition_tool_id, 1, maximum_durability)
                    .expect("validated ignition tool must remain available during activation");
                let items_removed = match ignition_use {
                    item::DurabilityUseOutcome::Updated(_) => Vec::new(),
                    item::DurabilityUseOutcome::Removed { id, name } => {
                        send_to_client(
                            *player_id,
                            ResponsePacket::Notice {
                                noticemsg: format!(
                                    "Your {name} breaks after lighting the Campfire."
                                ),
                                expiry: Some(5000),
                            },
                            &clients,
                        );
                        vec![id]
                    }
                };

                send_to_client(
                    *player_id,
                    ResponsePacket::InfoItemsUpdate {
                        id: hero_id,
                        items_updated: hero_inventory.get_packet(),
                        items_removed,
                    },
                    &clients,
                );

                let activate_event = VisibleEvent::ActivateEvent {
                    structure_id: *structure_id,
                };

                map_events.new(
                    hero_id,
                    game_tick.0 + 1, // in the future
                    activate_event,
                );
                pending_activations.insert(*structure_id);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn survey_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    hero_query: Query<CoreQuery, With<SubclassHero>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Survey {
                player_id,
                source_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok(hero) = hero_query.get(hero_entity) else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                if *source_id != hero_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Can only survey with your hero.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if Obj::is_dead(&hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot survey.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if is_discovery_action_state(hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Already surveying, prospecting, or investigating".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Surveying,
                });

                map_events.new(hero.id.0, game_tick.0 + 20, VisibleEvent::SurveyEvent);

                let packet = ResponsePacket::Survey { survey_time: 20 };
                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn prospect_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    hero_query: Query<CoreQuery, With<SubclassHero>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Prospect { player_id } | PlayerEvent::Explore { player_id } => {
                events_to_remove.push(*event_id);

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Ok(hero) = hero_query.get(hero_entity) else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                if Obj::is_dead(&hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot prospect.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if is_discovery_action_state(hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Already surveying, prospecting, or investigating".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Prospecting,
                });

                let prospect_time = 5 * TICKS_PER_SEC;
                map_events.new(
                    hero.id.0,
                    game_tick.0 + prospect_time,
                    VisibleEvent::ProspectEvent,
                );

                let packet = ResponsePacket::Prospect { prospect_time };
                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn investigate_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    run_spawned_objs: Res<RunSpawnedObjs>,
    mut map_events: ResMut<MapEvents>,
    query: Query<CoreQuery>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InvestigatePOI {
                player_id,
                target_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(*target_id) else {
                    error!("Cannot find investigate target entity {:?}", target_id);
                    continue;
                };

                let Ok(hero) = query.get(hero_entity) else {
                    error!("Cannot find hero for {:?}", hero_entity);
                    continue;
                };

                let Ok(target) = query.get(target_entity) else {
                    error!("Cannot find investigate target for {:?}", target_entity);
                    continue;
                };

                if Obj::is_dead(&hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot investigate.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if combat_locked(hero.last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                if is_discovery_action_state(hero.state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Already surveying, prospecting, or investigating".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !target.subclass.is_monolith() && *target.subclass != Subclass::Poi {
                    let packet = ResponsePacket::Error {
                        errmsg: "You can only investigate points of interest.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !can_access_run_shipwreck(
                    *player_id,
                    target.id.0,
                    target.template,
                    &run_spawned_objs,
                ) {
                    send_shipwreck_owner_error(*player_id, &clients);
                    continue;
                }

                if Map::dist(*hero.pos, *target.pos) > 1 {
                    let packet = ResponsePacket::Error {
                        errmsg: "Move closer to investigate.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Investigating,
                });

                map_events.new(
                    hero.id.0,
                    game_tick.0 + INVESTIGATE_TICKS,
                    VisibleEvent::InvestigateEvent {
                        target_id: *target_id,
                    },
                );

                let packet = ResponsePacket::Investigate {
                    investigate_time: INVESTIGATE_TICKS,
                };
                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_assign_system(
    mut events: ResMut<PlayerEvents>,
    ids: Res<Ids>,
    clients: Res<Clients>,
    entity_map: Res<EntityObjMap>,
    villager_query: Query<(
        Entity,
        &PlayerId,
        &Id,
        &Name,
        &Subclass,
        &Misc,
        Option<&Assignment>,
    )>,
    structure_query: Query<
        (
            &PlayerId,
            &Name,
            &Position,
            &State,
            &Assignments,
            &WorkQueue,
        ),
        With<ClassStructure>,
    >,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoAssign {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                // Check if structure is owned by player
                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok((
                    structure_player_id,
                    structure_name,
                    structure_pos,
                    structure_state,
                    structure_assignments,
                    structure_work_queue,
                )) = structure_query.get(structure_entity)
                else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                if structure_player_id.0 != *player_id {
                    error!("Structure not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Hero can be assigned to structures under construction
                let hero_assignable = *structure_state != State::None;

                let mut assignments_packet = Vec::new();

                // Get villager assignment data
                for (
                    villager_entity,
                    villager_player_id,
                    villager_id,
                    villager_name,
                    villager_subclass,
                    villager_misc,
                    villager_assignment,
                ) in villager_query.iter()
                {
                    if *player_id == villager_player_id.0
                        && (villager_subclass.is_villager()
                            || (hero_assignable && villager_subclass.is_hero()))
                    {
                        let mut assigned_structure_id = -1;
                        let mut assigned_structure_name = None;

                        if let Some(villager_assignment) = villager_assignment {
                            assigned_structure_id = villager_assignment.structure_id;
                            assigned_structure_name =
                                Some(villager_assignment.structure_name.clone());
                        }

                        let assignment = network::Assignment {
                            id: villager_id.0,
                            name: villager_name.0.to_string(),
                            image: villager_misc.image.to_string(),
                            structure_id: assigned_structure_id,
                            structure_name: assigned_structure_name,
                        };

                        assignments_packet.push(assignment);
                    }
                }

                if assignments_packet.len() == 0 {
                    let packet = ResponsePacket::Error {
                        errmsg: "No available workers to assign".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let packet = ResponsePacket::InfoAssign {
                    structure_id: *structure_id,
                    assignments: assignments_packet,
                };

                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn assign_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: ResMut<Ids>,
    mut events: ResMut<PlayerEvents>,
    entity_map: Res<EntityObjMap>,
    game_events: ResMut<GameEvents>,
    worker_query: Query<(&PlayerId, &Name, &Subclass, &Misc)>,
    mut structure_query: Query<
        (
            &PlayerId,
            &Name,
            &Position,
            &Subclass,
            &State,
            &mut Assignments,
            &mut WorkQueue,
        ),
        With<ClassStructure>,
    >,
    assignment_query: Query<(&PlayerId, &Id, &Name, &Subclass, &Misc)>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Assign {
                player_id,
                worker_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                // Get hero id from player id
                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(worker_entity) = entity_map.get_entity(*worker_id) else {
                    error!("Cannot find worker entity for {:?}", worker_id);
                    continue;
                };

                let Ok((worker_player_id, worker_name, worker_subclass, worker_misc)) =
                    worker_query.get(worker_entity)
                else {
                    error!("Query failed to find entity {:?}", worker_entity);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok((
                    structure_player_id,
                    structure_name,
                    structure_pos,
                    structure_subclass,
                    structure_state,
                    mut structure_assignments,
                    structure_work_queue,
                )) = structure_query.get_mut(structure_entity)
                else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if worker is owned by player
                if worker_player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Worker not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if structure is owned by player
                if structure_player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Hero can be assigned to structures under construction
                let hero_assignable = *structure_state != State::None;

                let mut assignments_packet = Vec::new();

                if worker_subclass.is_villager() {
                    // Add worker to structure assignments
                    structure_assignments.0.push(*worker_id);

                    // Set structure assignment of worker
                    commands.entity(worker_entity).insert(Assignment {
                        structure_id: *structure_id,
                        structure_name: structure_name.0.to_string(),
                        structure_pos: *structure_pos,
                    });

                    // If structure state is not None, add Build order to worker
                    info!("Structure state: {:?}", structure_state);
                    if *structure_state != State::None {
                        info!("Adding Build order to worker {:?}", worker_entity);
                        commands.entity(worker_entity).insert(Order::Build);
                    } else {
                        info!("Adding WorkQueue order to worker {:?}", worker_entity);
                        commands.entity(worker_entity).insert(Order::WorkQueue);
                    }
                } else if worker_subclass.is_hero() && hero_assignable {
                    // Add hero to structure assignments
                    structure_assignments.0.push(*worker_id);

                    // Set structure assignment of hero
                    commands.entity(worker_entity).insert(Assignment {
                        structure_id: *structure_id,
                        structure_name: structure_name.0.to_string(),
                        structure_pos: *structure_pos,
                    });
                }

                for assignment_id in structure_assignments.0.iter() {
                    let Some(assignment_entity) = entity_map.get_entity(*assignment_id) else {
                        error!("Cannot find assignment entity for {:?}", assignment_id);
                        continue;
                    };

                    // Get the current assignment data
                    let Ok((
                        assignment_player_id,
                        assignment_id,
                        assignment_name,
                        assignment_subclass,
                        assignment_misc,
                    )) = assignment_query.get(assignment_entity)
                    else {
                        error!("Query failed to find entity {:?}", assignment_entity);
                        continue;
                    };

                    if *player_id == assignment_player_id.0
                        && (assignment_subclass.is_villager()
                            || (assignment_subclass.is_hero() && hero_assignable))
                    {
                        let assignment = network::Assignment {
                            id: assignment_id.0,
                            name: assignment_name.0.to_string(),
                            image: assignment_misc.image.to_string(),
                            structure_id: *structure_id,
                            structure_name: Some(structure_name.0.to_string()),
                        };

                        assignments_packet.push(assignment);
                    }
                }

                let packet = ResponsePacket::InfoAssign {
                    structure_id: *structure_id,
                    assignments: assignments_packet,
                };

                send_to_client(*player_id, packet, &clients);
            }
            PlayerEvent::RemoveAssign {
                player_id,
                worker_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                // Validation checks get source entity
                let Some(worker_entity) = entity_map.get_entity(*worker_id) else {
                    error!("Cannot find villager entity for {:?}", worker_id);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok((worker_player_id, worker_name, worker_subclass, worker_misc)) =
                    worker_query.get(worker_entity)
                else {
                    error!("Query failed to find entity {:?}", worker_entity);
                    continue;
                };

                let Ok((
                    structure_player_id,
                    structure_name,
                    structure_pos,
                    _structure_subclass,
                    structure_state,
                    mut structure_assignments,
                    mut structure_work_queue,
                )) = structure_query.get_mut(structure_entity)
                else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if worker is owned by player
                if worker_player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Villager not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }
                // Check if structure is owned by player
                if structure_player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Remove worker from structure assignments
                structure_assignments.0.retain(|id| id != worker_id);

                // Remove assignment component from worker
                commands.entity(worker_entity).remove::<Assignment>();

                // Remove worker from work queue
                structure_work_queue
                    .0
                    .retain(|entry| entry.worker_id != *worker_id);

                let hero_assignable = *structure_state != State::None;

                let mut assignments_packet = Vec::new();

                for assignment_id in structure_assignments.0.iter() {
                    let Some(assignment_entity) = entity_map.get_entity(*assignment_id) else {
                        error!("Cannot find assignment entity for {:?}", assignment_id);
                        continue;
                    };

                    // Get the current assignment data
                    let Ok((
                        assignment_player_id,
                        assignment_id,
                        assignment_name,
                        assignment_subclass,
                        assignment_misc,
                    )) = assignment_query.get(assignment_entity)
                    else {
                        error!("Query failed to find entity {:?}", assignment_entity);
                        continue;
                    };

                    if *player_id == assignment_player_id.0
                        && (assignment_subclass.is_villager()
                            || (assignment_subclass.is_hero() && hero_assignable))
                    {
                        let assignment = network::Assignment {
                            id: assignment_id.0,
                            name: assignment_name.0.to_string(),
                            image: assignment_misc.image.to_string(),
                            structure_id: *structure_id,
                            structure_name: Some(structure_name.0.to_string()),
                        };

                        assignments_packet.push(assignment);
                    }
                }

                let packet = ResponsePacket::InfoAssign {
                    structure_id: *structure_id,
                    assignments: assignments_packet,
                };

                send_to_client(*player_id, packet, &clients);

                // Trigger a build progress update to client if structure is building
                if *structure_state == State::Building {
                    commands.trigger(BuildProgressUpdate {
                        entity: structure_entity,
                    });
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn equip_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    mut events: ResMut<PlayerEvents>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    templates: Res<Templates>,
    crisis_state: Option<Res<SettlementCrisisState>>,
    mut balance_telemetry_state: Option<ResMut<CrisisBalanceTelemetryState>>,
    mut query: Query<(
        &PlayerId,
        &Class,
        &Template,
        &State,
        &mut Inventory,
        &Effects,
    )>,
    mut viewshed_query: Query<&mut Viewshed>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Equip {
                player_id,
                obj_id,
                item_id,
                status,
            } => {
                events_to_remove.push(*event_id);

                let Some(owner_entity) = entity_map.get_entity(*obj_id) else {
                    error!("Cannot find villager entity for {:?}", obj_id);
                    continue;
                };

                let Ok((
                    owner_player_id,
                    owner_class,
                    owner_template,
                    owner_state,
                    mut owner_inventory,
                    owner_effects,
                )) = query.get_mut(owner_entity)
                else {
                    error!("Query failed to find entity {:?}", owner_entity);
                    continue;
                };

                if owner_class.is_structure() {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structures cannot equip items.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if Obj::is_dead(&owner_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot equip items.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if entity is owned by player
                if owner_player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Item not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Validate against the requested item before splitting a stack.
                // Rejected or duplicate requests must not mutate inventory.
                let Some(requested_item) = owner_inventory.get_by_id(*item_id) else {
                    error!("Cannot find item for {:?}", item_id);
                    continue;
                };

                // Check if equipable
                if requested_item.quantity <= 0 || !requested_item.equipable() {
                    let packet = ResponsePacket::Error {
                        errmsg: "Item is not equipable.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if object is busy
                if *owner_state != State::None {
                    let packet = ResponsePacket::Error {
                        errmsg: "Item owner is busy".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if requested_item.equipped == *status {
                    send_to_client(
                        *player_id,
                        ResponsePacket::InfoItemsUpdate {
                            id: *obj_id,
                            items_updated: owner_inventory.get_packet(),
                            items_removed: Vec::new(),
                        },
                        &clients,
                    );
                    continue;
                }

                let ignition_tool = if *status && requested_item.class == TORCH {
                    let Some(ignition_tool) = usable_ignition_tool(&owner_inventory, &templates)
                    else {
                        let packet = ResponsePacket::Error {
                            errmsg:
                                "You must have a usable Ignition Tool in this character's inventory"
                                    .to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    };
                    Some(ignition_tool)
                } else {
                    None
                };

                let Some((item_to_equip, source_item)) = owner_inventory.get_one_item_by_id(
                    *item_id,
                    ids.new_item_id(),
                    &templates.item_templates,
                ) else {
                    error!("Cannot prepare item for equip: {:?}", item_id);
                    continue;
                };

                let mut items_updated: Vec<Item> = Vec::new();
                let mut items_removed: Vec<i32> = Vec::new();
                let mut vision_changed = false;
                let meaningful_preparation_equip = *status
                    && !item_to_equip.equipped
                    && matches!(item_to_equip.class.as_str(), WEAPON | ARMOR);

                let vision_modifier = owner_effects.get_vision_modifier(&templates);

                // Equip if status is true
                if *status {
                    if let Some((ignition_tool_id, maximum_durability)) = ignition_tool {
                        let ignition_use = owner_inventory
                            .consume_durability_use(ignition_tool_id, 1, maximum_durability)
                            .expect("validated ignition tool must remain available during equip");
                        if let item::DurabilityUseOutcome::Removed { id, name } = ignition_use {
                            items_removed.push(id);
                            send_to_client(
                                *player_id,
                                ResponsePacket::Notice {
                                    noticemsg: format!(
                                        "Your {name} breaks after lighting the torch."
                                    ),
                                    expiry: Some(5000),
                                },
                                &clients,
                            );
                        }
                    }

                    // Equipping another off-hand item extinguishes the currently
                    // equipped torch in the same way as explicitly unequipping it.
                    let displaced_torches: Vec<i32> = owner_inventory
                        .items
                        .iter()
                        .filter(|item| {
                            item.id != item_to_equip.id
                                && item.equipped
                                && item.slot == item_to_equip.slot
                                && item.class == TORCH
                        })
                        .map(|item| item.id)
                        .collect();
                    for displaced_torch_id in displaced_torches {
                        owner_inventory.remove_item(displaced_torch_id);
                        items_removed.push(displaced_torch_id);
                        vision_changed = true;
                    }

                    if item_to_equip.class == TORCH {
                        let new_image = if item_to_equip.image.starts_with("lit") {
                            item_to_equip.image.clone()
                        } else {
                            format!("lit{}", item_to_equip.image)
                        };
                        owner_inventory.switch_image(item_to_equip.id, new_image);
                        owner_inventory.set_start_time(item_to_equip.id, game_tick.0);
                        vision_changed = true;
                    }

                    items_updated = owner_inventory.equip(item_to_equip.id, item_to_equip.slot);
                } else {
                    if item_to_equip.class == TORCH {
                        owner_inventory.remove_item(item_to_equip.id);
                        items_removed.push(item_to_equip.id);
                        vision_changed = true;
                    } else {
                        items_updated = owner_inventory.unequip(item_to_equip.id);
                    }
                }

                if vision_changed {
                    let new_vision = Obj::set_viewshed_range(
                        *obj_id,
                        owner_template.0.clone(),
                        game_tick.0,
                        &owner_inventory,
                        &templates,
                        vision_modifier,
                    );

                    let mut viewshed: Mut<'_, Viewshed> =
                        viewshed_query.get_mut(owner_entity).unwrap();
                    viewshed.range = new_vision;

                    commands.trigger(UpdateObj {
                        entity: owner_entity,
                        attrs: vec![(VISION.to_string(), viewshed.range.to_string())],
                    });
                }

                if item_to_equip.id != source_item.id {
                    items_updated.push(source_item.clone());
                }

                if meaningful_preparation_equip
                    && owner_inventory
                        .get_by_id(item_to_equip.id)
                        .is_some_and(|item| item.equipped)
                    && crisis_state
                        .as_ref()
                        .and_then(|state| state.get(player_id))
                        .is_some_and(|crisis| {
                            crisis.kind == CrisisKind::Goblin
                                && matches!(
                                    crisis.phase,
                                    CrisisPhase::Preparing | CrisisPhase::AssaultReady
                                )
                        })
                {
                    if let Some(telemetry_state) = balance_telemetry_state.as_deref_mut() {
                        telemetry_state
                            .entry(*player_id)
                            .or_default()
                            .preparation_actions
                            .record_equipment_change(item_to_equip.id, game_tick.0);
                    }
                }

                let item_update_packet: ResponsePacket = ResponsePacket::InfoItemsUpdate {
                    id: item_to_equip.owner,
                    items_updated: owner_inventory.get_packet(),
                    items_removed: items_removed,
                };

                send_to_client(*player_id, item_update_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_craft_system(
    game_tick: Res<GameTick>,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    game_events: Res<GameEvents>,
    recipes: Res<Recipes>,
    mut active_infos: ResMut<ActiveInfos>,
    query: Query<(&PlayerId, &Inventory)>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoCraft {
                player_id,
                crafter_id,
            } => {
                events_to_remove.push(*event_id);

                // Get hero id from player id
                let Some(crafter_entity) = entity_map.get_entity(*crafter_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Ok((crafter_player, inventory)) = query.get(crafter_entity) else {
                    error!("Cannot find crafter inventory for {:?}", crafter_entity);
                    continue;
                };

                // Check if crafter is owned by player
                if crafter_player.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Crafter not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let items = inventory.get_packet();

                let mut crafting_item = None;

                if let Some(crafting_event) = game_events.get_craft_event(*crafter_id) {
                    let Some(recipe) =
                        recipes.get_for_owner_by_name(*player_id, &crafting_event.recipe_name)
                    else {
                        error!("Cannot find recipe for {:?}", crafting_event.recipe_name);
                        continue;
                    };

                    let progress = game_tick.0 - crafting_event.start_tick;

                    crafting_item = Some(CraftingItem {
                        name: recipe.name,
                        image: recipe.image,
                        class: recipe.class,
                        subclass: recipe.subclass,
                        crafting_time: recipe.crafting_time.unwrap_or(100) / TICKS_PER_SEC,
                        progress: progress / TICKS_PER_SEC,
                    });
                }

                active_infos.add((*crafter_id, ActiveInfoType::Craft), *player_id);

                let packet = ResponsePacket::InfoCraft {
                    crafter_id: *crafter_id,
                    structure_id: None,
                    recipes: recipes.get_basic_recipes_packet(*player_id),
                    items: items,
                    crafting_item: crafting_item,
                };

                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_structure_craft_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    ids: Res<Ids>,
    game_events: Res<GameEvents>,
    recipes: Res<Recipes>,
    mut active_infos: ResMut<ActiveInfos>,
    templates: Res<Templates>,
    query: Query<(&PlayerId, &Template, &Inventory, &WorkQueue), With<ClassStructure>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoStructureCraft {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                // Get hero id from player id
                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok((
                    structure_player,
                    structure_template,
                    structure_inventory,
                    structure_work_queue,
                )) = query.get(structure_entity)
                else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if structure is owned by player
                if structure_player.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let structure_recipes =
                    recipes.get_by_structure_packet(*player_id, structure_template.0.clone());

                let mut crafting_item = None;

                if let Some(crafting_event) = game_events.get_structure_craft_event(hero_id) {
                    let Some(recipe) =
                        recipes.get_for_owner_by_name(*player_id, &crafting_event.recipe_name)
                    else {
                        error!("Cannot find recipe for {:?}", crafting_event.recipe_name);
                        continue;
                    };

                    let progress = game_tick.0 - crafting_event.start_tick;

                    crafting_item = Some(CraftingItem {
                        name: recipe.name,
                        image: recipe.image,
                        class: recipe.class,
                        subclass: recipe.subclass,
                        crafting_time: recipe.crafting_time.unwrap_or(100) / TICKS_PER_SEC,
                        progress: progress / TICKS_PER_SEC,
                    });
                }

                let work_queue_packet = structure_work_queue
                    .0
                    .iter()
                    .map(|work_entry| network::WorkEntry {
                        work_type: work_entry.work_type.to_string(),
                        work_status: work_entry.work_status.to_string(),
                        villager_id: work_entry.worker_id,
                        recipe_name: work_entry.recipe_name.clone(),
                        recipe_image: work_entry.recipe_image.clone(),
                        refine_item_id: work_entry.refine_item_id.clone(),
                        refine_item_image: work_entry.refine_item_image.clone(),
                        refine_item_class: work_entry.refine_item_class.clone(),
                        work_time: -1,
                        progress: 0,
                    })
                    .collect::<Vec<network::WorkEntry>>();

                let structure_items = structure_inventory.get_packet();

                let capacity = Obj::get_capacity(&structure_template.0, &templates.obj_templates);
                let total_weight = structure_inventory.get_total_weight();

                let structure_inventory_packet = network::Inventory {
                    id: *structure_id,
                    cap: capacity,
                    tw: total_weight,
                    items: structure_items,
                };

                let packet = ResponsePacket::InfoStructureCraft {
                    structure_inventory: structure_inventory_packet,
                    recipes: Some(structure_recipes),
                    queue: work_queue_packet,
                    crafting_item: crafting_item,
                };

                send_to_client(*player_id, packet, &clients);

                active_infos.add((*structure_id, ActiveInfoType::StructureCraft), *player_id);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_structure_queue_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    game_events: Res<GameEvents>,
    recipes: Res<Recipes>,
    templates: Res<Templates>,
    mut active_infos: ResMut<ActiveInfos>,
    query: Query<(&PlayerId, &Inventory, &WorkQueue), With<ClassStructure>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoStructureQueue {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok((structure_player, structure_inventory, structure_work_queue)) =
                    query.get(structure_entity)
                else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if structure is owned by player
                if structure_player.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let mut work_queue_packet = Vec::new();

                for work_entry in structure_work_queue.0.iter() {
                    let mut work_time = -1;
                    let mut progress = 0;

                    // Get progress of work entry
                    if work_entry.work_type == WorkType::Craft {
                        if let Some(crafting_event) =
                            game_events.get_structure_craft_event(work_entry.worker_id)
                        {
                            let Some(recipe) = recipes
                                .get_for_owner_by_name(*player_id, &crafting_event.recipe_name)
                            else {
                                error!("Cannot find recipe for {:?}", crafting_event.recipe_name);
                                continue;
                            };

                            progress = (game_tick.0 - crafting_event.start_tick) / TICKS_PER_SEC;
                            work_time = recipe.crafting_time.unwrap_or(100) / TICKS_PER_SEC;
                        }
                    } else if work_entry.work_type == WorkType::Refine {
                        if let Some(refine_event) =
                            game_events.get_structure_refine_event(work_entry.worker_id)
                        {
                            let Some(item) = structure_inventory.get_by_id(refine_event.item_id)
                            else {
                                error!("Cannot find item for {:?}", refine_event.item_id);
                                continue;
                            };

                            let item_template =
                                Item::get_template(item.name.clone(), &templates.item_templates);

                            work_time = item_template.get_refine_time() / TICKS_PER_SEC;
                            progress = (game_tick.0 - refine_event.start_tick) / TICKS_PER_SEC;
                        }
                    } else if work_entry.work_type == WorkType::Operate {
                        if let Some(operate_event) =
                            game_events.get_structure_operate_event(work_entry.worker_id)
                        {
                            progress = (game_tick.0 - operate_event.start_tick) / TICKS_PER_SEC;
                            work_time =
                                (operate_event.run_tick - operate_event.start_tick) / TICKS_PER_SEC;
                        }
                    }

                    work_queue_packet.push(network::WorkEntry {
                        work_type: work_entry.work_type.to_string(),
                        work_status: work_entry.work_status.to_string(),
                        villager_id: work_entry.worker_id,
                        recipe_name: work_entry.recipe_name.clone(),
                        recipe_image: work_entry.recipe_image.clone(),
                        refine_item_id: work_entry.refine_item_id.clone(),
                        refine_item_image: work_entry.refine_item_image.clone(),
                        refine_item_class: work_entry.refine_item_class.clone(),
                        work_time: work_time,
                        progress: progress,
                    });
                }

                let packet = ResponsePacket::InfoStructureQueue {
                    structure_id: *structure_id,
                    queue: work_queue_packet,
                };

                send_to_client(*player_id, packet, &clients);

                active_infos.add((*structure_id, ActiveInfoType::StructureQueue), *player_id);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_refine_system(
    game_tick: Res<GameTick>,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut active_infos: ResMut<ActiveInfos>,
    game_events: Res<GameEvents>,
    refiner_query: Query<(&PlayerId, &Inventory)>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoRefine {
                player_id,
                refiner_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(refiner_entity) = entity_map.get_entity(*refiner_id) else {
                    error!("Cannot find refiner entity for {:?}", refiner_id);
                    continue;
                };

                let Ok((refiner_player, refiner_inventory)) = refiner_query.get(refiner_entity)
                else {
                    error!("Query failed to find entity {:?}", refiner_entity);
                    continue;
                };

                // Check if structure is owned by player
                if refiner_player.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Refiner not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let refiner_items = refiner_inventory.get_packet();

                let refining_item_data: Option<RefiningItem>;

                if let Some(refine_event) = game_events.get_refine_event(*refiner_id) {
                    let Some(item) = refiner_inventory.get_by_id(refine_event.item_id) else {
                        error!("Cannot find item for {:?}", refine_event.item_id);
                        continue;
                    };

                    let item_template =
                        Item::get_template(item.name.clone(), &templates.item_templates);

                    let Some(produces) = item_template.produces.clone() else {
                        error!("Item is not refinable {:?}", item.name);
                        continue;
                    };

                    let produces_list =
                        item::produced_item_packets(&produces, &templates.item_templates);

                    // Get refine time
                    let item_template =
                        Item::get_template(item.name.clone(), &templates.item_templates);
                    let refine_time = item_template.get_refine_time();

                    let progress = game_tick.0 - refine_event.start_tick;
                    info!("Refine event start tick: {:?}", refine_event.start_tick);
                    info!("Game tick: {:?}", game_tick.0);
                    info!("Progress: {:?}", progress);

                    refining_item_data = Some(RefiningItem {
                        id: item.id,
                        name: item.name,
                        image: item.image,
                        class: item.class,
                        subclass: item.subclass,
                        quantity: item.quantity,
                        produces: produces_list,
                        refining_skill: item_template
                            .refine_skill
                            .clone()
                            .expect("Missing refine skill"),
                        refine_time: refine_time / TICKS_PER_SEC,
                        progress: progress / TICKS_PER_SEC,
                    });
                } else {
                    refining_item_data = None;
                }

                active_infos.add((*refiner_id, ActiveInfoType::Refine), *player_id);

                let packet = ResponsePacket::InfoRefine {
                    refiner_id: *refiner_id,
                    structure_id: None,
                    refiner_items: refiner_items,
                    structure_items: None,
                    refining_item: refining_item_data,
                    produced_items: Vec::new(),
                };

                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn info_structure_refine_system(
    game_tick: Res<GameTick>,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut active_infos: ResMut<ActiveInfos>,
    game_events: Res<GameEvents>,
    query: Query<(&PlayerId, &Template, &Inventory, &WorkQueue), With<ClassStructure>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoStructureRefine {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok((
                    structure_player,
                    structure_template,
                    structure_inventory,
                    structure_work_queue,
                )) = query.get(structure_entity)
                else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if structure is owned by player
                if structure_player.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                active_infos.add((*structure_id, ActiveInfoType::StructureRefine), *player_id);

                let structure_inventory_packet = network::Inventory {
                    id: *structure_id,
                    cap: Obj::get_capacity(&structure_template.0, &templates.obj_templates),
                    tw: structure_inventory.get_total_weight(),
                    items: structure_inventory.get_packet(),
                };

                let packet = ResponsePacket::InfoStructureRefine {
                    structure_inventory: structure_inventory_packet,
                    refining_item: None,
                    produced_items: Vec::new(),
                };

                send_to_client(*player_id, packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn order_operate_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut events: ResMut<PlayerEvents>,
    mut map_events: ResMut<MapEvents>,
    clients: Res<Clients>,
    mut villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    structure_query: Query<StructureQuery, With<ClassStructure>>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderOperate {
                player_id,
                villager_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(villager_entity) = entity_map.get_entity(*villager_id) else {
                    error!("Cannot find villager entity for {:?}", villager_id);
                    continue;
                };

                let Ok(villager) = villager_query.get_mut(villager_entity) else {
                    error!("Query failed to find entity {:?}", villager_entity);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok(structure) = structure_query.get(structure_entity) else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if builder is owned by player
                if villager.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Villager not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if structure is owned by player
                if structure.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                info!("Adding Order Operate to {:?}", villager.id);

                // Add assignment to villager
                commands.entity(villager.entity).insert(Assignment {
                    structure_id: *structure_id,
                    structure_name: structure.name.0.to_string(),
                    structure_pos: structure.pos.clone(),
                });

                //Add speech
                Obj::add_speech_event(
                    game_tick.0,
                    VillagerUtil::order_to_speech(&Order::Operate),
                    villager.id,
                    &mut map_events,
                );

                commands.entity(villager.entity).insert(Order::Operate);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn queued_craft_inputs_available(
    inventory: &Inventory,
    work_queue: &WorkQueue,
    new_recipe: &crate::recipe::Recipe,
    recipes: &Recipes,
    owner: i32,
) -> bool {
    let mut available_inventory = inventory.clone();

    for entry in work_queue
        .0
        .iter()
        .filter(|entry| entry.work_type == WorkType::Craft)
    {
        let Some(recipe_name) = entry.recipe_name.as_deref() else {
            return false;
        };
        let Some(recipe) = recipes.get_for_owner_by_name(owner, recipe_name) else {
            return false;
        };
        if available_inventory.try_consume_reqs(&recipe.req).is_none() {
            return false;
        }
    }

    available_inventory.has_craft_reqs(new_recipe.req.clone(), None)
}

fn structure_queue_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut events: ResMut<PlayerEvents>,
    mut game_events: ResMut<GameEvents>,
    clients: Res<Clients>,
    recipes: Res<Recipes>,
    templates: Res<Templates>,
    mut active_infos: ResMut<ActiveInfos>,
    villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    mut structure_query: Query<StructureQuery, With<ClassStructure>>,
    mut ids: ResMut<Ids>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::AddCraftingEntry {
                player_id,
                structure_id,
                recipe_name,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                /*let Some(villager_entity) = entity_map.get_entity(*source_id) else {
                    error!("Cannot find villager entity for {:?}", source_id);
                    continue;
                };

                let Ok(villager) = villager_query.get_mut(villager_entity) else {
                    error!("Query failed to find entity {:?}", villager_entity);
                    continue;
                };*/

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok(mut structure) = structure_query.get_mut(structure_entity) else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if builder is owned by player
                /*if villager.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Villager not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }*/

                // Check if structure is owned by player
                if structure.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !Structure::is_built(*structure.state) {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Structure is not active".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                info!("Adding Order Craft to {:?}", structure_id);
                let Some(recipe) = recipes.get_for_owner_by_name(*player_id, recipe_name) else {
                    error!("Invalid recipe name {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Invalid recipe".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                if !recipe.supports_structure(&structure.template.0) {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Recipe is not compatible with this structure".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                if structure.work_queue.0.iter().count() >= MAX_CRAFTING_QUEUE {
                    info!(
                        "Work queue length: {:?}",
                        structure.work_queue.0.iter().count()
                    );
                    let packet = ResponsePacket::Error {
                        errmsg: "Work queue is full".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if queued_craft_inputs_available(
                    &structure.inventory,
                    &structure.work_queue,
                    &recipe,
                    &recipes,
                    *player_id,
                ) {
                    info!("Adding CraftingEntry to {:?} queue", structure_id);

                    let work_entry = WorkEntry {
                        entry_id: ids.new_map_event_id(),
                        worker_id: -1,
                        work_type: WorkType::Craft,
                        work_status: WorkStatus::Idle,
                        recipe_name: Some(recipe_name.clone()),
                        recipe_image: Some(recipe.image.clone()),
                        refine_item_id: None,
                        refine_item_image: None,
                        refine_item_class: None,
                    };

                    // Add to crafting order to crafting orders
                    structure.work_queue.0.push(work_entry);

                    let mut work_queue_packet = Vec::new();

                    for work_entry in structure.work_queue.0.iter() {
                        let mut work_time = -1;
                        let mut progress = 0;

                        // Get progress of work entry
                        if work_entry.work_type == WorkType::Craft {
                            if let Some(crafting_event) =
                                game_events.get_craft_event(work_entry.worker_id)
                            {
                                let Some(recipe) = recipes
                                    .get_for_owner_by_name(*player_id, &crafting_event.recipe_name)
                                else {
                                    error!(
                                        "Cannot find recipe for {:?}",
                                        crafting_event.recipe_name
                                    );
                                    continue;
                                };

                                progress =
                                    (game_tick.0 - crafting_event.start_tick) / TICKS_PER_SEC;
                                work_time = recipe.crafting_time.unwrap_or(100) / TICKS_PER_SEC;
                            }
                        } else if work_entry.work_type == WorkType::Refine {
                            if let Some(refine_event) =
                                game_events.get_refine_event(work_entry.worker_id)
                            {
                                let Some(item) =
                                    structure.inventory.get_by_id(refine_event.item_id)
                                else {
                                    error!("Cannot find item for {:?}", refine_event.item_id);
                                    continue;
                                };

                                let item_template = Item::get_template(
                                    item.name.clone(),
                                    &templates.item_templates,
                                );

                                work_time = item_template.get_refine_time() / TICKS_PER_SEC;
                                progress = (game_tick.0 - refine_event.start_tick) / TICKS_PER_SEC;
                            }
                        }

                        work_queue_packet.push(network::WorkEntry {
                            work_type: work_entry.work_type.to_string(),
                            work_status: work_entry.work_status.to_string(),
                            villager_id: work_entry.worker_id,
                            recipe_name: work_entry.recipe_name.clone(),
                            recipe_image: work_entry.recipe_image.clone(),
                            refine_item_id: work_entry.refine_item_id.clone(),
                            refine_item_image: work_entry.refine_item_image.clone(),
                            refine_item_class: work_entry.refine_item_class.clone(),
                            work_time: work_time,
                            progress: progress,
                        });
                    }

                    // Add active info for structure queue
                    active_infos.add((*structure_id, ActiveInfoType::StructureQueue), *player_id);
                } else {
                    error!("Insufficient resources to craft {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient resources to craft".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }
            }
            PlayerEvent::AddRefineEntry {
                player_id,
                structure_id,
                refine_item_id,
            } => {
                events_to_remove.push(*event_id);
                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }
                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok(mut structure) = structure_query.get_mut(structure_entity) else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if structure is owned by player
                if structure.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !Structure::is_built(*structure.state) {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Structure is not active".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                if structure.work_queue.0.len() >= MAX_CRAFTING_QUEUE {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Work queue is full".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                let Some(refine_item) = structure.inventory.get_by_id(*refine_item_id) else {
                    error!("Cannot find item for {:?}", *refine_item_id);
                    continue;
                };

                let structure_template = templates.obj_templates.get(structure.template.0.clone());
                let supports_item = structure_template.refine.as_ref().is_some_and(|types| {
                    types.iter().any(|item_type| {
                        item_type == &refine_item.name
                            || item_type == &refine_item.class
                            || item_type == &refine_item.subclass
                    })
                });
                let item_is_refineable =
                    Item::find_template(refine_item.name.clone(), &templates.item_templates)
                        .is_some_and(|template| template.produces.is_some());
                if !supports_item || !item_is_refineable {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "This structure cannot refine that item".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                let reserved_quantity = structure
                    .work_queue
                    .0
                    .iter()
                    .filter(|entry| {
                        entry.work_type == WorkType::Refine
                            && entry.refine_item_id == Some(*refine_item_id)
                    })
                    .count() as i32;
                if reserved_quantity >= refine_item.quantity {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "All units in that stack are already queued".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                let work_entry = WorkEntry {
                    entry_id: ids.new_map_event_id(),
                    worker_id: -1,
                    work_type: WorkType::Refine,
                    work_status: WorkStatus::Idle,
                    recipe_name: None,
                    recipe_image: None,
                    refine_item_id: Some(*refine_item_id),
                    refine_item_image: Some(refine_item.image.clone()),
                    refine_item_class: Some(refine_item.class.clone()),
                };

                // Add to refine order to refine orders
                structure.work_queue.0.push(work_entry);

                let mut work_queue_packet = Vec::new();

                for work_entry in structure.work_queue.0.iter() {
                    let mut work_time = -1;
                    let mut progress = 0;

                    // Get progress of work entry
                    if work_entry.work_type == WorkType::Craft {
                        if let Some(crafting_event) =
                            game_events.get_craft_event(work_entry.worker_id)
                        {
                            let Some(recipe) = recipes
                                .get_for_owner_by_name(*player_id, &crafting_event.recipe_name)
                            else {
                                error!("Cannot find recipe for {:?}", crafting_event.recipe_name);
                                continue;
                            };

                            progress = (game_tick.0 - crafting_event.start_tick) / TICKS_PER_SEC;
                            work_time = recipe.crafting_time.unwrap_or(100) / TICKS_PER_SEC;
                        }
                    } else if work_entry.work_type == WorkType::Refine {
                        if let Some(refine_event) =
                            game_events.get_refine_event(work_entry.worker_id)
                        {
                            let Some(item) = structure.inventory.get_by_id(refine_event.item_id)
                            else {
                                error!("Cannot find item for {:?}", refine_event.item_id);
                                continue;
                            };

                            let item_template =
                                Item::get_template(item.name.clone(), &templates.item_templates);

                            work_time = item_template.get_refine_time() / TICKS_PER_SEC;
                            progress = (game_tick.0 - refine_event.start_tick) / TICKS_PER_SEC;
                        }
                    }

                    work_queue_packet.push(network::WorkEntry {
                        work_type: work_entry.work_type.to_string(),
                        work_status: work_entry.work_status.to_string(),
                        villager_id: work_entry.worker_id,
                        recipe_name: work_entry.recipe_name.clone(),
                        recipe_image: work_entry.recipe_image.clone(),
                        refine_item_id: work_entry.refine_item_id.clone(),
                        refine_item_image: work_entry.refine_item_image.clone(),
                        refine_item_class: work_entry.refine_item_class.clone(),
                        work_time: work_time,
                        progress: progress,
                    });
                }

                let packet = ResponsePacket::InfoStructureQueue {
                    structure_id: *structure_id,
                    queue: work_queue_packet,
                };

                send_to_client(*player_id, packet, &clients);

                active_infos.add((*structure_id, ActiveInfoType::StructureQueue), *player_id);
            }
            PlayerEvent::RemoveWorkEntry {
                player_id,
                structure_id,
                index,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok(mut structure) = structure_query.get_mut(structure_entity) else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if structure is owned by player
                if structure.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Some(work_entry) = usize::try_from(*index)
                    .ok()
                    .and_then(|index| structure.work_queue.0.get(index))
                    .cloned()
                else {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Invalid work queue entry".to_string(),
                        },
                        &clients,
                    );
                    continue;
                };

                let timed_events = game_events
                    .iter()
                    .filter_map(|(event_id, event)| {
                        (event.event_type.work_entry_id() == Some(work_entry.entry_id))
                            .then_some(*event_id)
                    })
                    .collect::<Vec<_>>();
                for event_id in timed_events {
                    game_events.remove(&event_id);
                }

                if work_entry.worker_id != -1 {
                    if let Some(worker_entity) = entity_map.get_entity(work_entry.worker_id) {
                        commands.trigger(StateChange {
                            entity: worker_entity,
                            new_state: State::None,
                        });
                        commands.entity(worker_entity).remove::<EventInProgress>();
                        commands.entity(worker_entity).insert(EventCompleted {
                            event_id: Uuid::new_v4(),
                            event_type: "work_queue_cancelled".to_string(),
                            at_tick: game_tick.0,
                            success: false,
                        });
                    }
                }

                structure
                    .work_queue
                    .0
                    .retain(|entry| entry.entry_id != work_entry.entry_id);

                let mut work_queue_packet = Vec::new();

                for work_entry in structure.work_queue.0.iter() {
                    let mut work_time = -1;
                    let mut progress = 0;

                    // Get progress of work entry
                    if work_entry.work_type == WorkType::Craft {
                        if let Some(crafting_event) =
                            game_events.get_craft_event(work_entry.worker_id)
                        {
                            let Some(recipe) = recipes
                                .get_for_owner_by_name(*player_id, &crafting_event.recipe_name)
                            else {
                                error!("Cannot find recipe for {:?}", crafting_event.recipe_name);
                                continue;
                            };

                            progress = (game_tick.0 - crafting_event.start_tick) / TICKS_PER_SEC;
                            work_time = recipe.crafting_time.unwrap_or(100) / TICKS_PER_SEC;
                        }
                    } else if work_entry.work_type == WorkType::Refine {
                        if let Some(refine_event) =
                            game_events.get_refine_event(work_entry.worker_id)
                        {
                            let Some(item) = structure.inventory.get_by_id(refine_event.item_id)
                            else {
                                error!("Cannot find item for {:?}", refine_event.item_id);
                                continue;
                            };

                            let item_template =
                                Item::get_template(item.name.clone(), &templates.item_templates);

                            work_time = item_template.get_refine_time() / TICKS_PER_SEC;
                            progress = (game_tick.0 - refine_event.start_tick) / TICKS_PER_SEC;
                        }
                    }

                    work_queue_packet.push(network::WorkEntry {
                        work_type: work_entry.work_type.to_string(),
                        work_status: work_entry.work_status.to_string(),
                        villager_id: work_entry.worker_id,
                        recipe_name: work_entry.recipe_name.clone(),
                        recipe_image: work_entry.recipe_image.clone(),
                        refine_item_id: work_entry.refine_item_id.clone(),
                        refine_item_image: work_entry.refine_item_image.clone(),
                        refine_item_class: work_entry.refine_item_class.clone(),
                        work_time: work_time,
                        progress: progress,
                    });
                }

                let packet = ResponsePacket::InfoStructureQueue {
                    structure_id: *structure_id,
                    queue: work_queue_packet,
                };

                send_to_client(*player_id, packet, &clients);
            }
            PlayerEvent::InfoWorkQueueEntry {
                player_id,
                structure_id,
                index,
            } => {
                events_to_remove.push(*event_id);

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok(structure) = structure_query.get(structure_entity) else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                let Some(work_entry) = usize::try_from(*index)
                    .ok()
                    .and_then(|index| structure.work_queue.0.get(index))
                    .cloned()
                else {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Invalid work queue entry".to_string(),
                        },
                        &clients,
                    );
                    continue;
                };

                // Get progress of work entry
                if work_entry.work_type == WorkType::Craft {
                    if let Some(crafting_event) = game_events.get_craft_event(work_entry.worker_id)
                    {
                        let Some(recipe) =
                            recipes.get_for_owner_by_name(*player_id, &crafting_event.recipe_name)
                        else {
                            error!("Cannot find recipe for {:?}", crafting_event.recipe_name);
                            continue;
                        };

                        let progress = (game_tick.0 - crafting_event.start_tick) / TICKS_PER_SEC;
                        let work_time = recipe.crafting_time.unwrap_or(100) / TICKS_PER_SEC;
                        let amount = recipe.amount.unwrap_or(1);

                        let packet = ResponsePacket::InfoWorkQueueEntry {
                            structure_id: *structure_id,
                            work_type: work_entry.work_type.to_string(),
                            index: *index,
                            worker_id: work_entry.worker_id,
                            item_name: recipe.name.clone(),
                            item_image: recipe.image.clone(),
                            item_quantity: amount,
                            work_time: work_time,
                            progress: progress,
                        };

                        send_to_client(*player_id, packet, &clients);
                    } else {
                        error!(
                            "Cannot find crafting event for worker {:?}",
                            work_entry.worker_id
                        );
                        continue;
                    }
                } else if work_entry.work_type == WorkType::Refine {
                    if let Some(refine_event) = game_events.get_refine_event(work_entry.worker_id) {
                        let Some(item) = structure.inventory.get_by_id(refine_event.item_id) else {
                            error!("Cannot find item for {:?}", refine_event.item_id);
                            continue;
                        };

                        let item_template =
                            Item::get_template(item.name.clone(), &templates.item_templates);

                        let work_time = item_template.get_refine_time() / TICKS_PER_SEC;
                        let progress = (game_tick.0 - refine_event.start_tick) / TICKS_PER_SEC;

                        let packet = ResponsePacket::InfoWorkQueueEntry {
                            structure_id: *structure_id,
                            work_type: work_entry.work_type.to_string(),
                            index: *index,
                            worker_id: work_entry.worker_id,
                            item_name: item.name.clone(),
                            item_image: item.image.clone(),
                            item_quantity: 1,
                            work_time: work_time,
                            progress: progress,
                        };

                        send_to_client(*player_id, packet, &clients);
                    }
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn order_prospect_system(
    mut events: ResMut<PlayerEvents>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut commands: Commands,
    mut map_events: ResMut<MapEvents>,
    clients: Res<Clients>,
    query: Query<ObjQuery>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderExplore {
                player_id,
                villager_id,
            }
            | PlayerEvent::OrderProspect {
                player_id,
                villager_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(entity) = entity_map.get_entity(*villager_id) else {
                    error!("Cannot find entity for {:?}", villager_id);
                    break;
                };

                let Ok(villager) = query.get(entity) else {
                    error!("Cannot find villager for {:?}", entity);
                    break;
                };

                if villager.player_id.0 != *player_id {
                    error!("Villager not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Cannot order another player's villager".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    break;
                }

                // Add OrderFollow component to source and set hero_entity as target
                for q in &query {
                    if q.id.0 == *villager_id {
                        //Add speech
                        Obj::add_speech_event(
                            game_tick.0,
                            VillagerUtil::order_to_speech(&Order::Explore),
                            villager.id,
                            &mut map_events,
                        );

                        commands.entity(q.entity).insert(Order::Explore);
                    }
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn order_experiment_system(
    commands: Commands,
    game_tick: Res<GameTick>,
    mut events: ResMut<PlayerEvents>,
    map_events: ResMut<MapEvents>,
    experiments: ResMut<Experiments>,
    templates: Res<Templates>,
    active_infos: Res<ActiveInfos>,
    clients: Res<Clients>,
    villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderExperiment {
                player_id,
                villager_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                /*let mut villager = None;

                //Find villager assigned to structure
                for villager_item in villager_query.iter() {
                    if villager_item.attrs.structure == *structure_id
                        && villager_item.player_id.0 == *player_id
                    {
                        villager = Some(villager_item);
                    }
                }

                if villager.is_none() {
                    error!(
                        "Cannot find a villager assigned to structure {:?}",
                        *structure_id
                    );
                    let packet = ResponsePacket::Error {
                        errmsg: "No villager assigned to structure to refine.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    break;
                }

                if let Some(villager) = villager {
                    info!("Adding Order Experiment to {:?}", villager.id);

                    // Update experiment state to progressing
                    let updated_experiment = Experiment::update_state(
                        villager.attrs.structure,
                        experiment::ExperimentState::Waiting,
                        &mut experiments,
                    );

                    if let Some(updated_experiment) = updated_experiment {
                        active_info_experiment(
                            villager.player_id.0,
                            villager.attrs.structure,
                            updated_experiment,
                            &items,
                            &active_infos,
                            &clients,
                            &templates,
                        );
                    }

                    commands.entity(villager.entity).insert(Order::Experiment);

                    Obj::add_speech_event(
                        game_tick.0,
                        VillagerUtil::order_to_speech(&Order::Experiment),
                        villager.id,
                        &mut map_events,
                    );
                }*/
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn order_farm_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut events: ResMut<PlayerEvents>,
    mut map_events: ResMut<MapEvents>,
    experiments: ResMut<Experiments>,
    _templates: Res<Templates>,
    active_infos: Res<ActiveInfos>,
    clients: Res<Clients>,
    mut villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    structure_query: Query<StructureQuery, With<ClassStructure>>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderPlant {
                player_id,
                villager_id,
                structure_id,
            }
            | PlayerEvent::OrderTend {
                player_id,
                villager_id,
                structure_id,
            }
            | PlayerEvent::OrderHarvest {
                player_id,
                villager_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(villager_entity) = entity_map.get_entity(*villager_id) else {
                    error!("Cannot find villager entity for {:?}", villager_id);
                    continue;
                };

                let Ok(villager) = villager_query.get_mut(villager_entity) else {
                    error!("Query failed to find entity {:?}", villager_entity);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok(structure) = structure_query.get(structure_entity) else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if builder is owned by player
                if villager.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Villager not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if structure is owned by player
                if structure.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                info!("Adding Order {:?} to {:?}", event, villager.id);

                // Add assignment to villager
                commands.entity(villager.entity).insert(Assignment {
                    structure_id: *structure_id,
                    structure_name: structure.name.0.to_string(),
                    structure_pos: structure.pos.clone(),
                });

                let order = match event {
                    PlayerEvent::OrderPlant { .. } => Order::Plant,
                    PlayerEvent::OrderTend { .. } => Order::Tend,
                    PlayerEvent::OrderHarvest { .. } => Order::Harvest,
                    _ => Order::Plant,
                };

                //Add speech
                Obj::add_speech_event(
                    game_tick.0,
                    VillagerUtil::order_to_speech(&order),
                    villager.id,
                    &mut map_events,
                );

                commands.entity(villager.entity).insert(order);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn order_repair_system(
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut events: ResMut<PlayerEvents>,
    mut map_events: ResMut<MapEvents>,
    villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    structure_query: Query<(&PlayerId, &Id, &Position, &Class, &Stats)>,
    ids: Res<Ids>,
    presence: Res<PlayerWorldPresenceState>,
    crisis_state: Option<Res<SettlementCrisisState>>,
    mut balance_telemetry_state: Option<ResMut<CrisisBalanceTelemetryState>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderRepair {
                player_id,
                villager_id,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(entity) = entity_map.get_entity(*villager_id) else {
                    error!("Cannot find villager entity for {:?}", villager_id);
                    continue;
                };

                let Ok(villager) = villager_query.get(entity) else {
                    error!("Query failed to find entity {:?}", entity);
                    continue;
                };

                if villager.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Villager not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                //Add speech
                Obj::add_speech_event(
                    game_tick.0,
                    VillagerUtil::order_to_speech(&Order::Repair),
                    villager.id,
                    &mut map_events,
                );

                commands.entity(villager.entity).insert(Order::Repair);

                // A repair starts only when this authoritative order has a
                // currently damaged owner structure to target. The balance
                // recorder is observation-only and is gated to the two
                // preparation phases, so ordinary repairs and rejected input
                // cannot contaminate crisis preparation metrics.
                if crisis_state
                    .as_ref()
                    .and_then(|state| state.get(player_id))
                    .is_some_and(|crisis| {
                        crisis.kind == CrisisKind::Goblin
                            && matches!(
                                crisis.phase,
                                CrisisPhase::Preparing | CrisisPhase::AssaultReady
                            )
                    })
                {
                    if let Some((_, structure_id, _, _, _)) = structure_query
                        .iter()
                        .filter(|(owner, _, _, class, stats)| {
                            owner.0 == *player_id
                                && class.is_structure()
                                && stats.hp < stats.base_hp
                        })
                        .min_by_key(|(_, _, pos, _, _)| Map::dist(*villager.pos, **pos))
                    {
                        if let Some(telemetry_state) = balance_telemetry_state.as_deref_mut() {
                            telemetry_state
                                .entry(*player_id)
                                .or_default()
                                .preparation_actions
                                .record_repair_started(structure_id.0, game_tick.0);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn use_item_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    mut visible_events: ResMut<VisibleEvents>,
    mut map_events: ResMut<MapEvents>,
    mut query: Query<(&PlayerId, &State, &mut Inventory, Option<&LastCombatTick>)>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Use {
                player_id,
                obj_id,
                item_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(owner_entity) = entity_map.get_entity(*obj_id) else {
                    error!("Cannot find obj entity for {:?}", *obj_id);
                    continue;
                };

                let Ok((owner_player_id, owner_state, owner_inventory, last_combat_tick)) =
                    query.get(owner_entity)
                else {
                    error!("Query failed to find entity {:?}", owner_entity);
                    continue;
                };

                if is_player_offline_protected(*player_id, &presence)
                    || is_owner_offline_protected(owner_player_id, &presence)
                {
                    continue;
                }

                if Obj::is_dead(owner_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot use items.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if entity is owned by player
                if owner_player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Item not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if item exists in inventory
                let Some(item) = owner_inventory.get_by_id(*item_id) else {
                    error!("Cannot find item for {:?}", *item_id);
                    continue;
                };

                if combat_locked(last_combat_tick, game_tick.0)
                    && matches!(
                        (item.class.as_str(), item.subclass.as_str()),
                        (FOOD, _) | (DRINK, _) | (BEDROLL, _) | (_, FISHING_ROD)
                    )
                {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                let is_bandage = item.class == item::MEDICAL && item.subclass.as_str() == "Bandage";
                let pending_bandage_use = map_events.values().any(|map_event| {
                    let VisibleEvent::UseItemEvent {
                        item_id,
                        item_owner_id,
                    } = &map_event.event_type
                    else {
                        return false;
                    };
                    *item_owner_id == *obj_id
                        && owner_inventory
                            .get_by_id(*item_id)
                            .is_some_and(|pending_item| {
                                pending_item.class == item::MEDICAL
                                    && pending_item.subclass.as_str() == "Bandage"
                            })
                });
                if *owner_state == State::Healing || pending_bandage_use {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Finish applying the current bandage first.".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }
                if is_bandage {
                    let already_using_item = map_events.values().any(|map_event| {
                        matches!(
                            &map_event.event_type,
                            VisibleEvent::UseItemEvent { item_owner_id, .. }
                                if *item_owner_id == *obj_id
                        )
                    });
                    if *owner_state != State::None || already_using_item {
                        send_to_client(
                            *player_id,
                            ResponsePacket::Error {
                                errmsg: "Finish the current action before applying a bandage."
                                    .to_string(),
                            },
                            &clients,
                        );
                        continue;
                    }

                    commands.trigger(StateChange {
                        entity: owner_entity,
                        new_state: State::Healing,
                    });
                    let action_id = ids.new_map_event_id();
                    commands.entity(owner_entity).insert(ActionProgress {
                        action_id,
                        start_tick: game_tick.0,
                        end_tick: game_tick.0 + BANDAGE_USE_TICKS,
                    });
                    visible_events.new(
                        *obj_id,
                        game_tick.0,
                        VisibleEvent::UpdateObjEvent {
                            attrs: vec![
                                ("state".to_string(), STATE_HEALING.to_string()),
                                ("action_id".to_string(), action_id.to_string()),
                                (
                                    "action_duration_ms".to_string(),
                                    (BANDAGE_USE_TICKS.saturating_mul(1000) / TICKS_PER_SEC)
                                        .to_string(),
                                ),
                                ("action_elapsed_ms".to_string(), "0".to_string()),
                            ],
                        },
                    );
                }

                let use_item_event = VisibleEvent::UseItemEvent {
                    item_id: *item_id,
                    item_owner_id: *obj_id,
                };

                let use_delay = if is_bandage { BANDAGE_USE_TICKS } else { 1 };
                map_events.new(*obj_id, game_tick.0 + use_delay, use_item_event);
            }
            PlayerEvent::DeleteItem {
                player_id,
                obj_id,
                item_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(owner_entity) = entity_map.get_entity(*obj_id) else {
                    error!("Cannot find obj entity for {:?}", *obj_id);
                    continue;
                };

                let Ok((owner_player_id, owner_state, mut owner_inventory, _last_combat_tick)) =
                    query.get_mut(owner_entity)
                else {
                    error!("Query failed to find entity {:?}", owner_entity);
                    continue;
                };

                if is_player_offline_protected(*player_id, &presence)
                    || is_owner_offline_protected(owner_player_id, &presence)
                {
                    continue;
                }

                if Obj::is_dead(owner_state) {
                    let packet = ResponsePacket::Error {
                        errmsg: "The dead cannot delete items.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if entity is owned by player
                if owner_player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Item not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check if item exists in inventory
                let Some(item) = owner_inventory.get_by_id(*item_id) else {
                    error!("Cannot find item for {:?}", *item_id);
                    continue;
                };

                info!("Removing item {:?}", item.name);

                owner_inventory.remove_item(*item_id);

                let items_to_remove = vec![*item_id];

                let item_update_packet: ResponsePacket = ResponsePacket::InfoItemsUpdate {
                    id: *obj_id,
                    items_updated: vec![],
                    items_removed: items_to_remove,
                };

                send_to_client(owner_player_id.0, item_update_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn sleep_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: Res<GameTick>,
    clients: Res<Clients>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    hero_query: Query<(&PlayerId, &Position, &State, Option<&LastCombatTick>), With<SubclassHero>>,
    shelter_query: Query<(&PlayerId, &Position, &State, &Shelter), With<ClassStructure>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Sleep {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                // Get hero id from player id
                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for {:?}", hero_id);
                    continue;
                };

                let Ok((hero_player_id, hero_pos, hero_state, last_combat_tick)) =
                    hero_query.get(hero_entity)
                else {
                    error!("Cannot find hero state for {:?}", hero_entity);
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find shelter for {:?}", structure_id);
                    continue;
                };
                let Ok((shelter_player_id, shelter_pos, shelter_state, _)) =
                    shelter_query.get(structure_entity)
                else {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "This structure is not a completed shelter".to_string(),
                        },
                        &clients,
                    );
                    continue;
                };

                if hero_player_id.0 != *player_id
                    || shelter_player_id.0 != *player_id
                    || !Structure::is_built(*shelter_state)
                {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "This shelter is not available to your hero".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                if hero_pos != shelter_pos {
                    send_to_client(
                        *player_id,
                        ResponsePacket::Error {
                            errmsg: "Your hero must be inside the shelter to sleep".to_string(),
                        },
                        &clients,
                    );
                    continue;
                }

                if Obj::is_dead(hero_state) {
                    continue;
                }

                if combat_locked(last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Sleeping,
                });

                map_events.new(
                    hero_id,
                    game_tick.0 + 30,
                    VisibleEvent::SleepEvent { obj_id: hero_id },
                );
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn remove_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut map_events: ResMut<MapEvents>,
    query: Query<ObjQuery>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Remove {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                let Some(entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find entity for {:?}", structure_id);
                    continue;
                };

                let Ok(obj) = query.get(entity) else {
                    error!("Cannot find obj for {:?}", entity);
                    continue;
                };

                if is_player_offline_protected(*player_id, &presence)
                    || is_owner_offline_protected(obj.player_id, &presence)
                {
                    continue;
                }

                // Check if entity is owned by player
                if obj.player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Obj not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                debug!("Removing obj: {:?}", obj.id.0);

                // Remove obj observer event
                commands.trigger(RemoveObj { entity: entity });
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn set_experiment_item_system(
    events: ResMut<PlayerEvents>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    experiments: ResMut<Experiments>,
    templates: Res<Templates>,
    query: Query<(&PlayerId, &mut State, &mut Inventory)>,
) {
    /*let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::SetExperimentItem {
                player_id,
                structure_id,
                item_id,
                is_resource,
            } => {
                events_to_remove.push(*event_id);

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find entity for {:?}", structure_id);
                    continue;
                };

                let Ok((structure_player_id, structure_state, structure_inventory)) =
                    query.get_mut(structure_entity)
                else {
                    error!("Query failed to find entity {:?}", structure_entity);
                    continue;
                };

                // Check if entity is owned by player
                if structure_player_id.0 != *player_id {
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure not owned by player.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Some(item) = structure_inventory.get_by_id(*item_id) else {
                    debug!("Failed to find item: {:?}", item_id);
                    continue;
                };

                if !is_resource {
                    if Item::is_resource(item.clone()) {
                        let packet = ResponsePacket::Error {
                            errmsg: "Cannot set resource item as experiment source.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }
                } else {
                    if !Item::is_resource(item.clone()) {
                        let packet = ResponsePacket::Error {
                            errmsg: "Can only set resource items as an experiment reagent."
                                .to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }
                }

                if !is_resource {
                    if let Some(experiment) = experiments.get_mut(&item.owner) {
                        debug!("Experiment: {:?}", experiment);
                        if let Some(source_item) = &experiment.source_item {
                            if source_item.id == *item_id {
                                // Player is transfering the item source out of experiment
                                items.remove_experiment_source(*item_id);
                                Experiment::reset(experiment);

                                send_info_experiment(
                                    *player_id,
                                    item.owner,
                                    experiment.clone(),
                                    &items,
                                    &clients,
                                    &templates,
                                );
                            } else {
                                let packet = ResponsePacket::Error {
                                    errmsg: "Experiment source item already set.".to_string(),
                                };
                                send_to_client(*player_id, packet, &clients);
                                continue;
                            }
                        } else {
                            let source_item = items.set_experiment_source(*item_id);
                            experiment.source_item = Some(source_item);

                            send_info_experiment(
                                *player_id,
                                item.owner,
                                experiment.clone(),
                                &items,
                                &clients,
                                &templates,
                            );
                        }
                    } else {
                        // Experiment does not exist, set experiment item source and create experiment
                        let source_item = items.set_experiment_source(*item_id);

                        let experiment = Experiment::create(
                            item.owner,
                            None,
                            ExperimentState::None,
                            source_item,
                            Vec::new(),
                            &mut experiments,
                        );

                        send_info_experiment(
                            *player_id,
                            item.owner,
                            experiment.clone(),
                            &items,
                            &clients,
                            &templates,
                        );
                    }
                } else {
                    if let Some(experiment) = experiments.get(&item.owner) {
                        if item.experiment.is_none() {
                            items.set_experiment_reagent(*item_id);
                        } else {
                            items.remove_experiment_reagent(*item_id);
                        }

                        send_info_experiment(
                            *player_id,
                            item.owner,
                            experiment.clone(),
                            &items,
                            &clients,
                            &templates,
                        );
                    }
                }
            }
            PlayerEvent::ResetExperiment {
                player_id,
                structure_id,
            } => {
                events_to_remove.push(*event_id);

                if let Some(experiment) = experiments.get_mut(structure_id) {
                    Experiment::reset(experiment);

                    send_info_experiment(
                        *player_id,
                        *structure_id,
                        experiment.clone(),
                        &items,
                        &clients,
                        &templates,
                    );
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }*/
}

/// Wage charged (in Gold Coins) to hire one villager from the merchant. Mirrors
/// the `wage` advertised by `info_hire_system`.
const HIRE_WAGE: i32 = 25;

fn hire_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut events: ResMut<PlayerEvents>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    templates: Res<Templates>,
    mut transport_query: Query<&mut Transport, With<Merchant>>,
    pos_query: Query<&Position>,
    mut inventory_query: Query<&mut Inventory>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        let PlayerEvent::Hire {
            player_id,
            merchant_id,
            target_id,
        } = event
        else {
            continue;
        };
        events_to_remove.push(*event_id);

        if protected_player_event_mutation(event, &ids, &presence) {
            continue;
        }

        // Resolve the hero, merchant, and the cargo villager being hired.
        let Some(hero_id) = ids.get_hero(*player_id) else {
            error!("Hire: cannot find hero for player {player_id}");
            continue;
        };
        let (Some(hero_entity), Some(merchant_entity), Some(cargo_entity)) = (
            entity_map.get_entity(hero_id),
            entity_map.get_entity(*merchant_id),
            entity_map.get_entity(*target_id),
        ) else {
            error!("Hire: cannot resolve hero/merchant/villager entities");
            continue;
        };

        // The villager must currently be aboard this merchant.
        let Ok(mut transport) = transport_query.get_mut(merchant_entity) else {
            error!("Hire: merchant {merchant_id} has no Transport");
            continue;
        };
        if !transport.hauling.contains(target_id) {
            send_to_client(
                *player_id,
                ResponsePacket::Error {
                    errmsg: "That villager is not for hire.".to_string(),
                },
                &clients,
            );
            continue;
        }

        // Must be standing next to the merchant to hire.
        let (Ok(hero_pos), Ok(merchant_pos)) =
            (pos_query.get(hero_entity), pos_query.get(merchant_entity))
        else {
            continue;
        };
        let hero_pos = *hero_pos;
        if !Map::is_adjacent_including_source(hero_pos, *merchant_pos) {
            send_to_client(
                *player_id,
                ResponsePacket::Error {
                    errmsg: "You must be next to the merchant to hire.".to_string(),
                },
                &clients,
            );
            continue;
        }

        // Charge the wage in Gold Coins from the hero's pack.
        let Ok([mut hero_inventory, mut merchant_inventory]) =
            inventory_query.get_many_mut([hero_entity, merchant_entity])
        else {
            error!("Hire: cannot access hero/merchant inventories");
            continue;
        };
        if hero_inventory.get_total_gold() < HIRE_WAGE {
            send_to_client(
                *player_id,
                ResponsePacket::Error {
                    errmsg: "Not enough gold to hire.".to_string(),
                },
                &clients,
            );
            continue;
        }
        let mut next_id = ids.new_item_id();
        Inventory::transfer_gold(
            &mut hero_inventory,
            &mut merchant_inventory,
            HIRE_WAGE,
            &mut next_id,
            &templates.item_templates,
        );

        // The villager disembarks: re-home it to the player at the hero's tile and
        // attach its villager behaviour (needs + AI). The same entity carries its
        // advertised attributes/skills over.
        transport.hauling.retain(|&x| x != *target_id);

        Encounter::convert_cargo_to_villager(
            &mut commands,
            cargo_entity,
            hero_pos,
            *player_id,
            &Inventory {
                owner: *target_id,
                items: Vec::new(),
            },
            &templates,
            &game_tick,
        );
        ids.new_obj(*target_id, *player_id);

        commands.trigger(NewObj {
            entity: cargo_entity,
        });

        send_to_client(
            *player_id,
            ResponsePacket::Notice {
                noticemsg: "A villager joins your camp.".to_string(),
                expiry: Some(3000),
            },
            &clients,
        );
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

// Empower a Monolith's sanctuary by one level, paid in Soulshards. The hero must
// be within the sanctuary; each level widens the random-spawn
// suppression radius and the in-zone defensive bonus (see move_event_completed_system
// and combat damage reduction). Soulshards come from killing the random spawns the
// sanctuary is meant to push back — the core "clear your area, then fortify it" loop.
fn upgrade_sanctuary_system(
    mut events: ResMut<PlayerEvents>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    pos_query: Query<&Position>,
    mut hero_inv_query: Query<&mut Inventory, With<SubclassHero>>,
    mut monolith_query: Query<&mut Monolith>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        let PlayerEvent::UpgradeSanctuary {
            player_id,
            monolith_id,
        } = event
        else {
            continue;
        };
        events_to_remove.push(*event_id);

        if protected_player_event_mutation(event, &ids, &presence) {
            continue;
        }

        let Some(hero_id) = ids.get_hero(*player_id) else {
            continue;
        };
        let (Some(hero_entity), Some(monolith_entity)) = (
            entity_map.get_entity(hero_id),
            entity_map.get_entity(*monolith_id),
        ) else {
            continue;
        };

        let Ok(mut monolith) = monolith_query.get_mut(monolith_entity) else {
            continue;
        };

        // Must be within (the current) sanctuary to channel the upgrade.
        let (Ok(hero_pos), Ok(monolith_pos)) =
            (pos_query.get(hero_entity), pos_query.get(monolith_entity))
        else {
            continue;
        };
        if Map::dist(*hero_pos, *monolith_pos) >= sanctuary_radius(monolith.sanctuary_level) {
            send_to_client(
                *player_id,
                ResponsePacket::Error {
                    errmsg: "You must be within the sanctuary to empower it.".to_string(),
                },
                &clients,
            );
            continue;
        }

        if monolith.sanctuary_level >= SANCTUARY_MAX_LEVEL {
            send_to_client(
                *player_id,
                ResponsePacket::Notice {
                    noticemsg: "The sanctuary is already at its full strength.".to_string(),
                    expiry: Some(3000),
                },
                &clients,
            );
            continue;
        }

        let cost = sanctuary_upgrade_cost(monolith.sanctuary_level);
        let Ok(mut hero_inv) = hero_inv_query.get_mut(hero_entity) else {
            continue;
        };
        let shards = hero_inv
            .get_by_class(item::SOULSHARD.to_string())
            .map(|s| s.quantity)
            .unwrap_or(0);
        if shards < cost {
            send_to_client(
                *player_id,
                ResponsePacket::Error {
                    errmsg: format!("Empowering the sanctuary needs {} Soulshards.", cost),
                },
                &clients,
            );
            continue;
        }

        if let Some(shard_item) = hero_inv.get_by_class(item::SOULSHARD.to_string()) {
            hero_inv.remove_quantity(shard_item.id, cost);
        }
        monolith.sanctuary_level += 1;

        send_to_client(
            *player_id,
            ResponsePacket::Notice {
                noticemsg: format!(
                    "The Monolith blazes brighter. Your sanctuary expands (level {}).",
                    monolith.sanctuary_level
                ),
                expiry: Some(4000),
            },
            &clients,
        );
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn buy_sell_system(
    _commands: Commands,
    _game_tick: Res<GameTick>,
    mut events: ResMut<PlayerEvents>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut prices: ResMut<Prices>,
    templates: Res<Templates>,
    run_spawned_objs: Res<RunSpawnedObjs>,
    mut query: Query<(&mut Position, &mut Inventory)>,
    template_query: Query<&Template>,
    mut merchant_query: Query<&mut Merchant>,
    presence: Res<PlayerWorldPresenceState>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::BuyItem {
                player_id,
                seller_id,
                item_id,
                quantity,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find entity for {:?}", hero_id);
                    continue;
                };

                let Some(merchant_entity) = entity_map.get_entity(*seller_id) else {
                    error!("Cannot find entity for {:?}", *seller_id);
                    continue;
                };

                let Ok(seller_template) = template_query.get(merchant_entity) else {
                    error!("Cannot find seller template for {:?}", merchant_entity);
                    continue;
                };
                if seller_template.0 == "Shipwreck" {
                    if !run_spawned_objs.contains_for_player(*player_id, *seller_id) {
                        send_shipwreck_owner_error(*player_id, &clients);
                    } else {
                        send_to_client(
                            *player_id,
                            ResponsePacket::Error {
                                errmsg: "Use item transfer to recover Shipwreck salvage."
                                    .to_string(),
                            },
                            &clients,
                        );
                    }
                    continue;
                }

                let Ok([(hero_pos, mut hero_inventory), (merchant_pos, mut merchant_inventory)]) =
                    query.get_many_mut([hero_entity, merchant_entity])
                else {
                    error!(
                        "Cannot find positions or inventories for {:?}",
                        [hero_entity, merchant_entity]
                    );
                    continue;
                };

                let Some(item) = merchant_inventory.get_by_id(*item_id) else {
                    error!("Cannot find item for {:?}", *item_id);
                    continue;
                };

                let Some(price) = prices.get_sell_price(item.name.clone()) else {
                    error!("Cannot find price for {:?}", item.name);
                    continue;
                };

                if item.quantity < *quantity {
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient quantity".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if hero_inventory.get_total_gold() < price * *quantity {
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient gold".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                if !Map::is_adjacent_including_source(*hero_pos, *merchant_pos) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Merchant is not nearby".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Transfer gold to merchant
                let mut next_id = ids.new_item_id();
                Inventory::transfer_gold(
                    &mut *hero_inventory,
                    &mut *merchant_inventory,
                    price * *quantity,
                    &mut next_id,
                    &templates.item_templates,
                );

                // Transfer item from merchant to hero
                Inventory::transfer_quantity(
                    item.id,
                    ids.new_item_id(),
                    &mut *merchant_inventory,
                    &mut *hero_inventory,
                    *quantity,
                    &templates.item_templates,
                );

                // Adjust price based on quantity
                prices.adjust_sell_price(item.name.clone(), *quantity);

                let mut item_filter = Vec::new();
                item_filter.push(item::GOLD.to_string());

                let source_items = hero_inventory.get_packet();
                let target_items = merchant_inventory.get_packet_filter(item_filter);

                let source_inventory = network::Inventory {
                    id: hero_id,
                    cap: 0,
                    tw: 0,
                    items: source_items.clone(),
                };

                let merchant_inventory = network::Inventory {
                    id: *seller_id,
                    cap: 0,
                    tw: 0,
                    items: target_items.clone(),
                };

                let item_transfer_packet: ResponsePacket = ResponsePacket::BuyItem {
                    source_id: hero_id,
                    inventory: source_inventory,
                    merchant_id: *seller_id,
                    merchant_inventory: merchant_inventory,
                };

                send_to_client(*player_id, item_transfer_packet, &clients);
            }
            PlayerEvent::SellItem {
                player_id,
                item_id,
                target_id,
                quantity,
            } => {
                events_to_remove.push(*event_id);

                if protected_player_event_mutation(event, &ids, &presence) {
                    continue;
                }

                let merchant_id = *target_id;

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find entity for {:?}", hero_id);
                    continue;
                };

                let Some(merchant_entity) = entity_map.get_entity(merchant_id) else {
                    error!("Cannot find entity for {:?}", merchant_id);
                    continue;
                };

                let Ok([(hero_pos, mut hero_inventory), (merchant_pos, mut merchant_inventory)]) =
                    query.get_many_mut([hero_entity, merchant_entity])
                else {
                    error!(
                        "Cannot find positions or inventories for {:?}",
                        [hero_entity, merchant_entity]
                    );
                    continue;
                };

                let Some(item) = hero_inventory.get_by_id(*item_id) else {
                    error!("Cannot find item for {:?}", *item_id);
                    continue;
                };

                if !Map::is_adjacent_including_source(*hero_pos, *merchant_pos) {
                    let packet = ResponsePacket::Error {
                        errmsg: "Merchant is not nearby".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Ok(mut merchant) = merchant_query.get_mut(merchant_entity) else {
                    error!("Cannot find merchant for {:?}", merchant_entity);
                    continue;
                };

                let mut target_item = None;

                for wanted_item in merchant.wanted_items.iter() {
                    if wanted_item.name == Some(item.name.clone()) {
                        target_item = Some(wanted_item);
                        break;
                    } else if wanted_item.subclass == Some(item.subclass.clone()) {
                        target_item = Some(wanted_item);
                        break;
                    } else if wanted_item.class == Some(item.class.clone()) {
                        target_item = Some(wanted_item);
                        break;
                    }
                }

                let Some(selling_item) = target_item else {
                    let packet = ResponsePacket::Error {
                        errmsg: "Merchant does not want item".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                if quantity > &selling_item.quantity {
                    let packet = ResponsePacket::Error {
                        errmsg: "Merchant does not want that quantity".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Wanted item identifier
                let item_identifier = selling_item.get_identifier();

                let Some(price) = prices.get_buy_price(item_identifier.clone()) else {
                    error!("Cannot find price for {:?}", selling_item);
                    continue;
                };

                if merchant_inventory.get_total_gold() < price * *quantity {
                    let packet = ResponsePacket::Error {
                        errmsg: "Merchant has insufficient gold".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // TOOD check if owner has room for the gold coins
                // TODO check if target has the space to hold the item

                // Transfer gold to hero from merchant
                let mut next_id = ids.new_item_id();
                Inventory::transfer_gold(
                    &mut *merchant_inventory,
                    &mut *hero_inventory,
                    price * *quantity,
                    &mut next_id,
                    &templates.item_templates,
                );

                // Transfer item from hero to merchant
                Inventory::transfer_quantity(
                    item.id,
                    ids.new_item_id(),
                    &mut *hero_inventory,
                    &mut *merchant_inventory,
                    *quantity,
                    &templates.item_templates,
                );

                // Adjust price based on quantity
                prices.adjust_buy_price(item_identifier, *quantity);

                let mut wanted_items_to_remove = vec![];

                // Update Merchant wanted items
                for wanted_item in merchant.wanted_items.iter_mut() {
                    let Some(price) = prices.get_buy_price(wanted_item.get_identifier()) else {
                        error!("Cannot find price for {:?}", wanted_item.get_identifier());
                        continue;
                    };

                    let Some(quantity) = prices.get_buy_quantity(wanted_item.get_identifier())
                    else {
                        error!(
                            "Cannot find quantity for {:?}",
                            wanted_item.get_identifier()
                        );
                        continue;
                    };

                    wanted_item.price = price;
                    wanted_item.quantity = quantity;

                    if quantity == 0 {
                        wanted_items_to_remove.push(wanted_item.clone());
                    }
                }

                // Remove items with quantity 0
                for wanted_item in wanted_items_to_remove.iter() {
                    merchant.wanted_items.retain(|x| x != wanted_item);
                }
                debug!("merchant.wanted_items: {:?}", merchant.wanted_items);

                let mut item_filter = Vec::new();
                item_filter.push(item::GOLD.to_string());

                let source_items = hero_inventory.get_packet();
                let target_items = merchant_inventory.get_packet_filter(item_filter);

                let source_inventory = network::Inventory {
                    id: item.owner,
                    cap: 0,
                    tw: 0,
                    items: source_items.clone(),
                };

                let target_inventory = network::Inventory {
                    id: *target_id,
                    cap: 0,
                    tw: 0,
                    items: target_items.clone(),
                };

                let item_transfer_packet: ResponsePacket = ResponsePacket::SellItem {
                    source_id: item.owner,
                    inventory: source_inventory,
                    merchant_id: *target_id,
                    merchant_inventory: target_inventory,
                    merchant_wanted_items: merchant.wanted_items.clone(),
                };

                send_to_client(*player_id, item_transfer_packet, &clients);
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn cancel_action_system(
    mut commands: Commands,
    ids: Res<Ids>,
    game_tick: Res<GameTick>,
    mut events: ResMut<PlayerEvents>,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::CancelAction { player_id } => {
                events_to_remove.push(*event_id);

                let Some(hero_id) = ids.get_hero(*player_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for hero {:?}", hero_id);
                    continue;
                };

                let mut game_events_to_remove = -1;

                for (game_event_id, game_event) in game_events.iter() {
                    if let GameEventType::RefineEvent { refiner_id, .. } = &game_event.event_type {
                        if *refiner_id == hero_id {
                            game_events_to_remove = *game_event_id;
                            break;
                        }
                    }

                    if let GameEventType::CraftEvent { crafter_id, .. } = &game_event.event_type {
                        if *crafter_id == hero_id {
                            game_events_to_remove = *game_event_id;
                            break;
                        }
                    }
                }

                if game_events_to_remove != -1 {
                    game_events.remove(&game_events_to_remove);

                    commands.trigger(StateChange {
                        entity: hero_entity,
                        new_state: State::None,
                    });
                    commands.entity(hero_entity).remove::<ActionProgress>();
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

/*pub fn active_info_experiment(
    player_id: i32,
    structure_id: i32,
    experiment: Experiment,
    items: &ResMut<Items>,
    active_infos: &Res<ActiveInfos>,
    clients: &Res<Clients>,
    templates: &Res<Templates>,
) {
    let active_info_key = (player_id, structure_id, "experiment".to_string());

    if let Some(_active_info) = active_infos.get(&active_info_key) {
        send_info_experiment(
            player_id,
            structure_id,
            experiment,
            items,
            clients,
            templates,
        );
    }
}*/

fn debug_obj_system(
    mut events: ResMut<PlayerEvents>,
    mut debug_objs: ResMut<DebugObjs>,
    clients: Res<Clients>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        if let PlayerEvent::DebugObj { player_id, obj_id } = event {
            events_to_remove.push(*event_id);

            let enabled = if debug_objs.0.contains(obj_id) {
                debug_objs.0.remove(obj_id);
                false
            } else {
                debug_objs.0.insert(*obj_id);
                true
            };

            info!("Debug logging for obj {} set to {}", obj_id, enabled);

            send_to_client(
                *player_id,
                ResponsePacket::DebugObj {
                    obj_id: *obj_id,
                    enabled,
                },
                &clients,
            );
        }
    }

    for id in events_to_remove {
        events.remove(&id);
    }
}

fn build_filter_from_overrides(overrides: &HashMap<String, String>) -> EnvFilter {
    let mut filter = EnvFilter::new("info");

    for (target, level) in overrides {
        let directive = format!("{}={}", target, level.to_lowercase());
        match directive.parse() {
            Ok(dir) => filter = filter.add_directive(dir),
            Err(e) => {
                warn!("Invalid log directive '{}': {}", directive, e);
            }
        }
    }

    filter
}

fn set_log_level_system(
    mut events: ResMut<PlayerEvents>,
    mut log_overrides: ResMut<LogLevelOverrides>,
    clients: Res<Clients>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        if let PlayerEvent::SetLogLevel {
            player_id,
            target,
            level,
        } = event
        {
            events_to_remove.push(*event_id);

            let mut success = false;

            // Update overrides map
            if level == "OFF" {
                log_overrides.overrides.remove(target);
                info!("Log level for '{}' cleared (OFF)", target);
            } else {
                log_overrides
                    .overrides
                    .insert(target.clone(), level.clone());
                info!("Log level for '{}' set to {}", target, level);
            }

            // Reload filter
            if let Some(handle_arc) = &log_overrides.reload_handle {
                if let Ok(handle) = handle_arc.lock() {
                    let new_filter = build_filter_from_overrides(&log_overrides.overrides);
                    match handle.reload(new_filter) {
                        Ok(_) => success = true,
                        Err(e) => error!("Failed to reload log filter: {}", e),
                    }
                }
            } else {
                error!("Reload handle not initialized");
            }

            send_to_client(
                *player_id,
                ResponsePacket::LogLevelSet {
                    target: target.clone(),
                    level: level.clone(),
                    success,
                },
                &clients,
            );
        }
    }

    for id in events_to_remove {
        events.remove(&id);
    }
}

fn get_log_levels_system(
    mut events: ResMut<PlayerEvents>,
    log_overrides: Res<LogLevelOverrides>,
    clients: Res<Clients>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        if let PlayerEvent::GetLogLevels { player_id } = event {
            events_to_remove.push(*event_id);

            let overrides: Vec<(String, String)> = log_overrides
                .overrides
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            send_to_client(
                *player_id,
                ResponsePacket::LogLevels { overrides },
                &clients,
            );
        }
    }

    for id in events_to_remove {
        events.remove(&id);
    }
}

/*pub fn send_info_experiment(
    player_id: i32,
    structure_id: i32,
    experiment: Experiment,
    items: &ResMut<Items>,
    clients: &Res<Clients>,
    templates: &Res<Templates>,
) {
    let (experiment_source, experiment_reagents, other_resources) =
        items.get_experiment_details_packet(structure_id);

    let info_experiment: ResponsePacket = ResponsePacket::InfoExperiment {
        id: structure_id,
        expitem: experiment_source,
        expresources: experiment_reagents,
        validresources: other_resources,
        expstate: Experiment::state_to_string(experiment.state.clone()),
        recipe: Experiment::recipe_to_packet(experiment.clone(), templates),
    };

    send_to_client(player_id, info_experiment, &clients);
}*/

//TODO Move this to structure module

#[derive(Debug, Clone)]
pub enum TimeOfDay {
    Dawn,
    Morning,
    Afternoon,
    Evening,
    Dusk,
    Night,
}

pub fn get_time_of_day(hour: i32) -> TimeOfDay {
    match hour {
        1..=4 => TimeOfDay::Night,
        5..=5 => TimeOfDay::Dawn,
        6..=11 => TimeOfDay::Morning,
        12..=16 => TimeOfDay::Afternoon,
        17..=22 => TimeOfDay::Evening,
        23..=23 => TimeOfDay::Dusk,
        18..=24 => TimeOfDay::Night,
        _ => TimeOfDay::Night,
    }
}

pub fn is_player(player_id: i32) -> bool {
    player_id < MAX_PLAYER_ID // TODO switch NPC players id below 1000
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Client, Fortified, SettlementCrisis};
    use crate::item::Slot;
    use crate::safe_logout::{PlayerPresenceRecord, PlayerWorldPresence};
    use std::collections::{HashMap, HashSet};
    use std::fs::File;

    fn load_obj_templates() -> Vec<ObjTemplate> {
        let obj_template_file =
            File::open("templates/obj_template.yaml").expect("Could not open obj templates");
        serde_yaml::from_reader(obj_template_file).expect("Could not read obj templates")
    }

    fn template_by_name(name: &str) -> ObjTemplate {
        load_obj_templates()
            .into_iter()
            .find(|template| template.template == name)
            .unwrap_or_else(|| panic!("Missing template {}", name))
    }

    fn base_test_stats() -> Stats {
        Stats {
            hp: 1,
            stamina: Some(1),
            mana: Some(0),
            base_hp: 1,
            base_stamina: Some(1),
            base_mana: Some(0),
            base_def: 0,
            damage_range: Some(1),
            base_damage: Some(1),
            base_speed: Some(1),
            base_vision: Some(1),
        }
    }

    fn loot_test_item(id: i32, owner: i32, weight: f32) -> Item {
        Item {
            id,
            owner,
            name: format!("Loot {id}"),
            quantity: 1,
            durability: None,
            class: "Loot".to_string(),
            subclass: "Loot".to_string(),
            slot: None,
            image: "loot.png".to_string(),
            weight,
            equipped: false,
            experiment: None,
            start_time: 0,
            attrs: HashMap::new(),
            produces: Vec::new(),
        }
    }

    #[test]
    fn loot_all_accepts_public_bags_and_enemy_corpses_but_not_shipwrecks() {
        let player_id = 7;
        assert!(is_loot_all_source(
            player_id,
            NPC_PLAYER_ID,
            &Template("Giant Rat".to_string()),
            &State::Dead,
        ));
        assert!(!is_loot_all_source(
            player_id,
            NPC_PLAYER_ID,
            &Template("Shipwreck".to_string()),
            &State::None,
        ));
        assert!(is_loot_all_source(
            player_id,
            NPC_PLAYER_ID,
            &Template(templates::DROPPED_BAG_TEMPLATE.to_string()),
            &State::None,
        ));
        assert!(!is_loot_all_source(
            player_id,
            player_id,
            &Template("Villager".to_string()),
            &State::Dead,
        ));
        assert!(!is_loot_all_source(
            player_id,
            NPC_PLAYER_ID,
            &Template("Giant Rat".to_string()),
            &State::None,
        ));
        assert!(!is_loot_all_source(
            player_id,
            player_id,
            &Template("Burrow".to_string()),
            &State::None,
        ));
    }

    #[test]
    fn dropped_bag_capacity_accepts_fifty_weight_but_rejects_more() {
        let inventory = Inventory {
            owner: 20,
            items: vec![loot_test_item(1, 20, 49.0)],
        };

        assert!(dropped_bag_can_accept(
            &inventory,
            &loot_test_item(2, 10, 1.0)
        ));
        assert!(!dropped_bag_can_accept(
            &inventory,
            &loot_test_item(3, 10, 2.0)
        ));
    }

    #[test]
    fn loot_all_transfers_every_whole_stack_that_fits_capacity() {
        let mut source = Inventory {
            owner: 20,
            items: vec![loot_test_item(1, 20, 3.0), loot_test_item(2, 20, 8.0)],
        };
        let mut target = Inventory {
            owner: 10,
            items: vec![loot_test_item(3, 10, 2.0)],
        };

        assert_eq!(transfer_loot_that_fits(&mut source, &mut target, 10), 1);
        assert_eq!(
            source.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![2]
        );
        assert!(target
            .items
            .iter()
            .any(|item| item.id == 1 && item.owner == 10));
        assert_eq!(target.get_total_weight(), 5);
    }

    #[derive(Component)]
    struct TestAbilityActor;

    #[derive(Component)]
    struct TestAbilityTarget;

    #[derive(Resource, Default)]
    struct AbilityDeathTransitions(Vec<Entity>);

    fn apply_lethal_test_ability(
        mut commands: Commands,
        game_tick: Res<GameTick>,
        mut actors: Query<CombatQuery, (With<TestAbilityActor>, Without<TestAbilityTarget>)>,
        mut targets: Query<CombatQuery, (With<TestAbilityTarget>, Without<TestAbilityActor>)>,
    ) {
        let mut actor = actors.single_mut().expect("one ability actor");
        let mut target = targets.single_mut().expect("one ability target");
        apply_ability_damage(&mut commands, &game_tick, &mut actor, &mut target, 10);
    }

    fn capture_ability_death_transition(
        event: On<StateChange>,
        mut transitions: ResMut<AbilityDeathTransitions>,
    ) {
        if event.new_state == State::Dead {
            transitions.0.push(event.entity);
        }
    }

    fn spawn_ability_test_actor(
        world: &mut World,
        id: i32,
        player_id: i32,
        position: Position,
        subclass: Subclass,
        marker: impl Bundle,
    ) -> Entity {
        world
            .spawn((
                marker,
                Id(id),
                PlayerId(player_id),
                position,
                Class(CLASS_UNIT.to_string()),
                subclass,
                Template("Ability Test Actor".to_string()),
                State::None,
                Misc {
                    image: String::new(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                base_test_stats(),
                Effects(HashMap::new()),
                Inventory {
                    owner: id,
                    items: Vec::new(),
                },
                LastCombatTick(0),
            ))
            .id()
    }

    fn connected_test_client(player_id: i32) -> (Clients, tokio::sync::mpsc::Receiver<String>) {
        let clients = Clients::default();
        let (sender, receiver) = tokio::sync::mpsc::channel(8);
        let client_id = Uuid::new_v4();
        clients.activate(Client {
            id: client_id,
            player_id,
            sender,
        });
        (clients, receiver)
    }

    #[test]
    fn lethal_direct_ability_stops_thinker_and_emits_dead_state_change() {
        let mut app = App::new();
        app.insert_resource(GameTick(42))
            .init_resource::<AbilityDeathTransitions>()
            .add_observer(capture_ability_death_transition)
            .add_systems(Update, apply_lethal_test_ability);
        spawn_ability_test_actor(
            app.world_mut(),
            1,
            1,
            Position { x: 0, y: 0 },
            Subclass::Hero,
            TestAbilityActor,
        );
        let target = spawn_ability_test_actor(
            app.world_mut(),
            2,
            NPC_PLAYER_ID,
            Position { x: 1, y: 0 },
            Subclass::Npc,
            TestAbilityTarget,
        );
        app.world_mut()
            .entity_mut(target)
            .insert(ThinkerBuilder::default());

        app.update();

        let target_ref = app.world().entity(target);
        assert_eq!(*target_ref.get::<State>().unwrap(), State::Dead);
        assert_eq!(target_ref.get::<StateDead>().unwrap().dead_at, 42);
        assert!(target_ref.get::<ThinkerBuilder>().is_none());
        assert_eq!(
            app.world().resource::<AbilityDeathTransitions>().0,
            vec![target]
        );
    }

    #[test]
    fn personal_goblin_intent_copy_matches_owner_combat_targeting() {
        for template in ["Wolf Rider", "Goblin Pillager"] {
            assert_eq!(
                enemy_intent_for_template(template, true),
                "Raider advancing on your defenders and blocking walls"
            );
            assert_eq!(
                enemy_intent_for_template(template, false),
                "Raider targeting your stored value and structures"
            );
        }
    }

    #[test]
    fn hero_gathering_uses_resource_or_equipped_hunting_activity() {
        let forage_event = GameEventType::ForageEvent { forager_id: 7 };

        assert_eq!(
            gather_activity_for_event(
                &GameEventType::GatherEvent {
                    gatherer_id: 7,
                    res_type: LOG.to_string(),
                },
                false
            ),
            ActiveTask::Logging
        );
        assert_eq!(
            gather_activity_for_event(
                &GameEventType::GatherEvent {
                    gatherer_id: 7,
                    res_type: GAME_ANIMAL.to_string(),
                },
                false,
            ),
            ActiveTask::Hunting
        );
        assert_eq!(
            gather_activity_for_event(&forage_event, false),
            ActiveTask::Gathering
        );
        assert_eq!(
            gather_activity_for_event(&forage_event, true),
            ActiveTask::Hunting
        );

        let item_template_file =
            File::open("templates/item_template.yaml").expect("Could not open item templates");
        let item_templates: Vec<crate::templates::ItemTemplate> =
            serde_yaml::from_reader(item_template_file).expect("Could not read item templates");
        let mut inventory = Inventory {
            owner: 7,
            items: Vec::new(),
        };
        inventory.new(1, "Sharpened Stick".to_string(), 1, &item_templates);
        inventory.equip(1, Some(Slot::MainHand));

        assert!(inventory.has_equipped_tool_for_attr(&item::AttrKey::Hunting));
        assert_eq!(
            gather_activity_for_event(
                &forage_event,
                inventory.has_equipped_tool_for_attr(&item::AttrKey::Hunting),
            ),
            ActiveTask::Hunting
        );
    }

    fn equipped_test_weapon(name: &str, subclass: &str, range: i32, accuracy: i32) -> Item {
        Item {
            id: 1,
            owner: 1,
            name: name.to_string(),
            quantity: 1,
            durability: None,
            class: WEAPON.to_string(),
            subclass: subclass.to_string(),
            slot: Some(item::Slot::MainHand),
            image: String::new(),
            weight: 1.0,
            equipped: true,
            experiment: None,
            start_time: 0,
            attrs: HashMap::from([
                (AttrKey::AttackRange, AttrVal::Num(range as f32)),
                (AttrKey::Accuracy, AttrVal::Num(accuracy as f32)),
            ]),
            produces: Vec::new(),
        }
    }

    fn inventory_with(item: Item) -> Inventory {
        Inventory {
            owner: 1,
            items: vec![item],
        }
    }

    #[test]
    fn checkpoint3_villager_equipment_counts_once_only_during_preparation() {
        let player_id = 7;
        let villager_id = 70;
        let first_item_id = 700;
        let second_item_id = 701;

        let mut first_weapon = equipped_test_weapon("Training Bow", "Bow", 2, 85);
        first_weapon.id = first_item_id;
        first_weapon.owner = villager_id;
        first_weapon.equipped = false;
        let mut second_weapon = equipped_test_weapon("Stone Knife", "Dagger", 1, 100);
        second_weapon.id = second_item_id;
        second_weapon.owner = villager_id;
        second_weapon.equipped = false;

        let mut app = App::new();
        let entity = app
            .world_mut()
            .spawn((
                PlayerId(player_id),
                Class("Human".to_string()),
                Template("Human Villager".to_string()),
                State::None,
                Inventory {
                    owner: villager_id,
                    items: vec![first_weapon, second_weapon],
                },
                Effects(HashMap::new()),
                SubclassVillager,
            ))
            .id();

        let mut ids = Ids::default();
        ids.item = second_item_id;
        ids.new_obj(villager_id, player_id);
        let mut crises = SettlementCrisisState::default();
        crises.insert(
            player_id,
            SettlementCrisis {
                phase: CrisisPhase::Preparing,
                ..SettlementCrisis::default()
            },
        );
        app.insert_resource(GameTick(100));
        app.insert_resource(ids);
        app.insert_resource(PlayerEvents(HashMap::from([(
            1,
            PlayerEvent::Equip {
                player_id,
                obj_id: villager_id,
                item_id: first_item_id,
                status: true,
            },
        )])));
        app.insert_resource(MapEvents::default());
        app.insert_resource(EntityObjMap(HashMap::from([(villager_id, entity)])));
        app.insert_resource(Clients::default());
        app.insert_resource(Templates::from_obj_templates(Vec::new()));
        app.insert_resource(crises);
        app.insert_resource(CrisisBalanceTelemetryState::default());
        app.add_systems(Update, equip_system);

        app.update();
        {
            let telemetry = app.world().resource::<CrisisBalanceTelemetryState>();
            let actions = &telemetry.get(&player_id).unwrap().preparation_actions;
            assert_eq!(actions.equipment_changes, 1);
            assert_eq!(actions.meaningful_preparation_category_count, 1);
            assert_eq!(actions.meaningful_preparation_categories, ["equipment"]);
        }

        // Repeating the same authoritative equip cannot inflate the action.
        app.world_mut().resource_mut::<PlayerEvents>().insert(
            2,
            PlayerEvent::Equip {
                player_id,
                obj_id: villager_id,
                item_id: first_item_id,
                status: true,
            },
        );
        app.update();
        assert_eq!(
            app.world()
                .resource::<CrisisBalanceTelemetryState>()
                .get(&player_id)
                .unwrap()
                .preparation_actions
                .equipment_changes,
            1
        );

        // A different successful equip after launch still changes ordinary
        // inventory state, but must not become crisis-preparation telemetry.
        app.world_mut()
            .resource_mut::<SettlementCrisisState>()
            .get_mut(&player_id)
            .unwrap()
            .phase = CrisisPhase::AssaultActive;
        app.world_mut().resource_mut::<PlayerEvents>().insert(
            3,
            PlayerEvent::Equip {
                player_id,
                obj_id: villager_id,
                item_id: second_item_id,
                status: true,
            },
        );
        app.update();
        assert_eq!(
            app.world()
                .resource::<CrisisBalanceTelemetryState>()
                .get(&player_id)
                .unwrap()
                .preparation_actions
                .equipment_changes,
            1
        );
    }

    #[test]
    fn class_ability_lists_are_distinct() {
        let warrior = ability_defs_for_class(HeroClass::Warrior);
        let ranger = ability_defs_for_class(HeroClass::Ranger);
        let mage = ability_defs_for_class(HeroClass::Mage);

        assert_eq!(
            warrior.iter().map(|ability| ability.id).collect::<Vec<_>>(),
            vec!["shield_bash"]
        );
        assert_eq!(
            ranger.iter().map(|ability| ability.id).collect::<Vec<_>>(),
            vec!["aimed_shot", "disengage"]
        );
        assert_eq!(
            mage.iter().map(|ability| ability.id).collect::<Vec<_>>(),
            vec!["arcane_bolt", "ward"]
        );
    }

    #[test]
    fn founded_structures_accept_build_resources_without_counting_as_completed_inventories() {
        let structure_class = Class(CLASS_STRUCTURE.to_string());

        assert!(accepts_build_resource_transfer(
            &structure_class,
            &State::Founded
        ));
        assert!(accepts_build_resource_transfer(
            &structure_class,
            &State::PlanningUpgrade
        ));
        assert!(!accepts_build_resource_transfer(
            &structure_class,
            &State::Building
        ));
        assert!(!accepts_build_resource_transfer(
            &structure_class,
            &State::None
        ));

        assert!(!incomplete_structure_blocks_inventory_transfer(
            &structure_class,
            &State::Founded
        ));
        assert!(!incomplete_structure_blocks_inventory_transfer(
            &structure_class,
            &State::PlanningUpgrade
        ));
        assert!(incomplete_structure_blocks_inventory_transfer(
            &structure_class,
            &State::Building
        ));
        assert!(!incomplete_structure_blocks_inventory_transfer(
            &structure_class,
            &State::None
        ));
    }

    #[test]
    fn ability_definitions_keep_class_costs_and_requirements() {
        let aimed_shot = ability_def("aimed_shot").expect("aimed_shot ability");
        assert_eq!(aimed_shot.hero_class, HeroClass::Ranger);
        assert_eq!(aimed_shot.cost_type, AbilityCostType::Stamina);
        assert_eq!(aimed_shot.required_weapon_subclass, Some("Bow"));
        assert_eq!(aimed_shot.range, 3);

        let arcane_bolt = ability_def("arcane_bolt").expect("arcane_bolt ability");
        assert_eq!(arcane_bolt.hero_class, HeroClass::Mage);
        assert_eq!(arcane_bolt.cost_type, AbilityCostType::Mana);
        assert_eq!(arcane_bolt.cost, 20);

        for ability_id in [
            "shield_bash",
            "aimed_shot",
            "disengage",
            "arcane_bolt",
            "ward",
        ] {
            assert_eq!(
                ability_def(ability_id).unwrap().cooldown,
                ATTACK_COOLDOWN_SECONDS,
                "ability UI cooldown must match the shared hero combat cooldown"
            );
        }
    }

    #[test]
    fn class_profiles_point_at_existing_templates_and_abilities() {
        let templates = load_obj_templates();
        let template_names: HashSet<String> = templates
            .iter()
            .map(|template| template.template.clone())
            .collect();

        for hero_class in [HeroClass::Warrior, HeroClass::Ranger, HeroClass::Mage] {
            let profile = HeroClassProfile::for_class(hero_class);
            assert_eq!(profile.hero_class, hero_class);
            assert!(template_names.contains(profile.novice_template));
            assert!(!profile.label.is_empty());
            assert!(!profile.selection_hint.is_empty());

            for ability_id in profile.ability_ids {
                assert!(
                    ability_def(ability_id).is_some(),
                    "profile references missing ability {}",
                    ability_id
                );
            }
        }
    }

    #[test]
    fn hero_advance_chains_have_templates_for_every_class() {
        let templates = load_obj_templates();
        let template_names: HashSet<String> = templates
            .iter()
            .map(|template| template.template.clone())
            .collect();

        for start in ["Novice Warrior", "Novice Ranger", "Novice Mage"] {
            let mut current = start.to_string();
            assert!(template_names.contains(&current), "missing {}", current);

            loop {
                let (next, _required_xp) = SkillData::hero_advance(current.clone());
                if next == MAX_RANK {
                    break;
                }

                assert!(template_names.contains(&next), "missing {}", next);
                current = next;
            }
        }
    }

    #[test]
    fn mage_rank_templates_scale_mana() {
        let mana_by_rank = [
            ("Novice Mage", 100),
            ("Skilled Mage", 150),
            ("Great Mage", 225),
            ("Legendary Mage", 325),
        ];

        for (template_name, expected_mana) in mana_by_rank {
            let template = template_by_name(template_name);
            assert_eq!(template.base_mana, Some(expected_mana));
        }

        assert_eq!(template_by_name("Novice Warrior").base_mana, Some(0));
        assert_eq!(template_by_name("Novice Ranger").base_mana, Some(0));
    }

    #[test]
    fn campfire_upgrades_to_shelter_tent_with_tent_requirements() {
        let campfire = template_by_name("Campfire");
        assert_eq!(campfire.upgrade_to, Some(vec!["Shelter Tent".to_string()]));

        let shelter_tent = template_by_name("Shelter Tent");
        assert_eq!(shelter_tent.upgrade_cost, Some(50));
        assert_eq!(
            shelter_tent.upgrade_req,
            Some(vec![
                ResReq {
                    req_type: item::LOGS_OR_TIMBER.to_string(),
                    quantity: 5,
                    cquantity: None,
                },
                ResReq {
                    req_type: "Hide".to_string(),
                    quantity: 3,
                    cquantity: None,
                },
            ])
        );
    }

    #[test]
    fn completed_campfire_storage_accepts_cooking_fuel_and_meat_only() {
        for template in [
            templates::CAMPFIRE_TEMPLATE,
            templates::SHELTER_TENT_TEMPLATE,
        ] {
            assert!(accepts_completed_storage_item(
                template,
                item::FIREWOOD,
                "Firewood"
            ));
            assert!(accepts_completed_storage_item(
                template,
                "Bristleback Raw Meat",
                "Raw Meat"
            ));
            assert!(accepts_completed_storage_item(
                template,
                "Bristleback Cooked Meat",
                "Cooked Meat"
            ));
            assert!(accepts_completed_storage_item(
                template,
                item::CHARCOAL,
                "Charcoal"
            ));
            assert!(!accepts_completed_storage_item(
                template,
                "Smoked Meat",
                "Smoked Meat"
            ));
            assert!(!accepts_completed_storage_item(template, LOG, LOG));
            assert!(!accepts_completed_storage_item(
                template,
                "Sharpened Stick",
                "Spear"
            ));
        }

        assert!(accepts_completed_storage_item("Burrow", LOG, LOG));
        assert!(accepts_completed_storage_item(
            "Burrow",
            "Sharpened Stick",
            "Spear"
        ));
    }

    #[test]
    fn hero_work_allows_a_lit_torch_or_active_campfire_but_not_darkness() {
        let no_vision = Viewshed { range: 0 };
        let torch_radius = Viewshed { range: 1 };
        let stronger_light = Viewshed { range: 2 };
        let mut campfire_visibility = CampfireVisibilityState::default();

        assert!(!has_sufficient_work_visibility(
            None,
            7,
            &campfire_visibility
        ));
        assert!(!has_sufficient_work_visibility(
            Some(&no_vision),
            7,
            &campfire_visibility
        ));
        assert!(has_sufficient_work_visibility(
            Some(&torch_radius),
            7,
            &campfire_visibility
        ));
        assert!(has_sufficient_work_visibility(
            Some(&stronger_light),
            7,
            &campfire_visibility
        ));

        campfire_visibility.insert((7, 99));
        assert!(has_sufficient_work_visibility(
            Some(&no_vision),
            7,
            &campfire_visibility
        ));
        assert!(!has_sufficient_work_visibility(
            Some(&no_vision),
            8,
            &campfire_visibility
        ));

        let mut app = App::new();
        app.add_plugins(crate::templates::TemplatesPlugin);
        let templates = app.world().resource::<Templates>();
        let mut inventory = Inventory {
            owner: 7,
            items: Vec::new(),
        };
        inventory.new(1, "Crude Torch".to_string(), 1, &templates.item_templates);
        inventory.equip(1, Some(Slot::OffHand));

        let actual_torch_range = Obj::set_viewshed_range(
            7,
            "Novice Warrior".to_string(),
            NIGHT,
            &inventory,
            templates,
            0.0,
        );
        assert_eq!(actual_torch_range, 1);
        assert!(has_sufficient_work_visibility(
            Some(&Viewshed {
                range: actual_torch_range,
            }),
            7,
            &CampfireVisibilityState::default(),
        ));
    }

    fn setup_refine_visibility_test(campfire_is_active: bool) -> (App, Entity) {
        const PLAYER_ID: i32 = 7;
        const HERO_ID: i32 = 70;
        const CARCASS_ID: i32 = 501;

        let mut app = App::new();
        app.add_plugins(crate::templates::TemplatesPlugin);
        app.add_systems(Update, refine_system);
        app.insert_resource(GameTick(100));
        app.insert_resource(MapEvents(HashMap::new()));
        app.insert_resource(GameEvents(HashMap::new()));
        app.insert_resource(VisibleEvents(Vec::new()));
        app.insert_resource(Recipes::from_recipes(Vec::new()));
        app.insert_resource(ActiveInfos(HashMap::new()));
        app.insert_resource(PlayerEvents(HashMap::from([(
            1,
            PlayerEvent::Refine {
                player_id: PLAYER_ID,
                item_id: CARCASS_ID,
            },
        )])));

        let mut ids = Ids::default();
        ids.new_hero(HERO_ID, PLAYER_ID);
        app.insert_resource(ids);

        app.insert_resource(Clients::default());

        let mut inventory = Inventory {
            owner: HERO_ID,
            items: Vec::new(),
        };
        inventory.new(
            CARCASS_ID,
            "Windstride Deer Carcass".to_string(),
            1,
            &app.world().resource::<Templates>().item_templates,
        );
        let hero_entity = app
            .world_mut()
            .spawn((
                Position { x: 0, y: 0 },
                State::None,
                inventory,
                Skills::new(),
                Viewshed { range: 0 },
            ))
            .id();
        app.insert_resource(EntityObjMap(HashMap::from([(HERO_ID, hero_entity)])));

        let campfire_visibility = if campfire_is_active {
            CampfireVisibilityState(HashSet::from([(PLAYER_ID, 99)]))
        } else {
            CampfireVisibilityState::default()
        };
        app.insert_resource(campfire_visibility);

        (app, hero_entity)
    }

    #[test]
    fn hero_cannot_start_refining_in_darkness_but_active_campfire_light_allows_it() {
        let (mut dark_app, dark_hero) = setup_refine_visibility_test(false);
        dark_app.update();

        assert!(dark_app.world().resource::<GameEvents>().is_empty());
        assert!(dark_app.world().get::<ActionProgress>(dark_hero).is_none());

        let (mut lit_app, lit_hero) = setup_refine_visibility_test(true);
        lit_app.update();

        assert_eq!(lit_app.world().resource::<GameEvents>().len(), 1);
        assert!(lit_app.world().get::<ActionProgress>(lit_hero).is_some());
    }

    #[test]
    fn carrying_capacities_keep_people_below_settlement_storage() {
        for hero in ["Novice Warrior", "Novice Ranger", "Novice Mage"] {
            assert_eq!(template_by_name(hero).capacity, Some(100));
        }
        for hero in ["Skilled Warrior", "Skilled Ranger", "Skilled Mage"] {
            assert_eq!(template_by_name(hero).capacity, Some(125));
        }
        for hero in ["Great Warrior", "Great Ranger", "Great Mage"] {
            assert_eq!(template_by_name(hero).capacity, Some(150));
        }
        for hero in ["Legendary Warrior", "Legendary Ranger", "Legendary Mage"] {
            assert_eq!(template_by_name(hero).capacity, Some(200));
        }

        assert_eq!(template_by_name("Human Villager").capacity, Some(75));
        assert_eq!(template_by_name("Burrow").capacity, Some(300));
        assert_eq!(template_by_name("Cache").capacity, Some(500));
        assert_eq!(template_by_name("Warehouse").capacity, Some(1000));
    }

    #[test]
    fn refresh_stats_updates_ranger_progression_values() {
        let template = template_by_name("Skilled Ranger");
        let mut stats = base_test_stats();

        refresh_stats_from_template(&mut stats, Some(HeroClass::Ranger), &template);

        assert_eq!(stats.hp, 150);
        assert_eq!(stats.base_hp, 150);
        assert_eq!(stats.stamina, Some(175));
        assert_eq!(stats.base_stamina, Some(175));
        assert_eq!(stats.mana, Some(0));
        assert_eq!(stats.base_mana, Some(0));
        assert_eq!(stats.base_speed, Some(8));
        assert_eq!(stats.base_vision, Some(6));
    }

    #[test]
    fn refresh_stats_updates_mage_max_mana() {
        let template = template_by_name("Great Mage");
        let mut stats = base_test_stats();

        refresh_stats_from_template(&mut stats, Some(HeroClass::Mage), &template);

        assert_eq!(stats.hp, 220);
        assert_eq!(stats.base_mana, Some(225));
        assert_eq!(stats.mana, Some(225));
        assert_eq!(stats.base_def, 2);
    }

    #[test]
    fn guard_bash_definition_and_effect_timers_match_profile() {
        let guard_bash = ability_def("shield_bash").expect("guard bash ability");

        assert_eq!(guard_bash.label, "Guard Bash");
        assert_eq!(guard_bash.hero_class, HeroClass::Warrior);
        assert_eq!(guard_bash.cost_type, AbilityCostType::Stamina);
        assert_eq!(guard_bash.cost, 10);
        assert_eq!(guard_bash.range, 1);

        let mut effects = Effects(HashMap::new());
        let mut map_events = MapEvents::default();
        add_timed_effect(
            42,
            &mut effects,
            &mut map_events,
            100,
            Effect::Stunned,
            GUARD_BASH_STUN_TICKS,
            1.0,
        );
        add_timed_effect(
            7,
            &mut effects,
            &mut map_events,
            100,
            Effect::Bracing,
            WARRIOR_BRACE_DURATION_TICKS,
            WARRIOR_BRACE_AMPLIFIER,
        );

        assert_eq!(
            effects.0.get(&Effect::Bracing),
            Some(&(
                100 + WARRIOR_BRACE_DURATION_TICKS,
                WARRIOR_BRACE_AMPLIFIER,
                1
            ))
        );
        assert!(map_events.values().any(|event| {
            event.obj_id == 42
                && event.run_tick == 100 + GUARD_BASH_STUN_TICKS
                && matches!(
                    &event.event_type,
                    VisibleEvent::EffectExpiredEvent { effect }
                        if *effect == Effect::Stunned
                )
        }));
    }

    #[test]
    fn ranger_ability_definitions_support_kiting() {
        let aimed_shot = ability_def("aimed_shot").expect("aimed shot ability");
        let disengage = ability_def("disengage").expect("disengage ability");

        assert_eq!(aimed_shot.hero_class, HeroClass::Ranger);
        assert_eq!(aimed_shot.required_weapon_subclass, Some("Bow"));
        assert_eq!(aimed_shot.range, 3);
        assert_eq!(disengage.hero_class, HeroClass::Ranger);
        assert_eq!(
            disengage_destination(Position { x: 2, y: 2 }, Position { x: 1, y: 2 }),
            Some(Position { x: 3, y: 2 })
        );
        assert_eq!(
            disengage_destination(Position { x: 2, y: 2 }, Position { x: 2, y: 2 }),
            None
        );
    }

    #[test]
    fn only_ranged_damage_abilities_can_hit_fortified_targets() {
        let guard_bash = ability_def("shield_bash").expect("guard bash ability");
        let aimed_shot = ability_def("aimed_shot").expect("aimed shot ability");
        let arcane_bolt = ability_def("arcane_bolt").expect("arcane bolt ability");
        let disengage = ability_def("disengage").expect("disengage ability");

        assert!(ability_is_damaging(guard_bash));
        assert!(!ability_is_ranged_attack(guard_bash));
        assert!(ability_is_ranged_attack(aimed_shot));
        assert!(ability_is_ranged_attack(arcane_bolt));
        assert!(!ability_is_damaging(disengage));
    }

    #[test]
    fn ranged_weapon_profiles_use_item_attrs() {
        let no_effects = Effects(HashMap::new());
        let bow_inventory = inventory_with(equipped_test_weapon("Training Bow", "Bow", 3, 85));
        let sling_inventory =
            inventory_with(equipped_test_weapon("Improvised Sling", "Sling", 2, 75));
        let throwing_spear_inventory =
            inventory_with(equipped_test_weapon("Throwing Spear", "Throwing", 2, 75));

        let bow_profile =
            equipped_ranged_weapon_profile(&bow_inventory, &no_effects).expect("bow profile");
        let sling_profile =
            equipped_ranged_weapon_profile(&sling_inventory, &no_effects).expect("sling profile");
        let throwing_spear_profile =
            equipped_ranged_weapon_profile(&throwing_spear_inventory, &no_effects)
                .expect("throwing spear profile");

        assert_eq!(bow_profile.range, 3);
        assert_eq!(bow_profile.accuracy, Some(85));
        assert_eq!(bow_profile.stamina_cost, BASIC_ATTACK_STAMINA_COST);
        assert_eq!(sling_profile.range, 2);
        assert_eq!(sling_profile.accuracy, Some(75));
        assert_eq!(throwing_spear_profile.range, 2);
        assert_eq!(throwing_spear_profile.accuracy, Some(75));
        assert!(throwing_spear_profile.is_ranged);
    }

    #[test]
    fn only_equipped_spear_subclass_weapons_gain_fortification_reach() {
        let spear_inventory =
            inventory_with(equipped_test_weapon("Stone-Tipped Spear", "Spear", 1, 100));
        let axe_inventory = inventory_with(equipped_test_weapon("Copper Axe", "Axe", 1, 100));
        let throwing_spear_inventory =
            inventory_with(equipped_test_weapon("Throwing Spear", "Throwing", 2, 75));

        assert!(Combat::equipped_weapon_has_fortification_reach(
            &spear_inventory
        ));
        assert!(!Combat::equipped_weapon_has_fortification_reach(
            &axe_inventory
        ));
        // Throwing Spears use their ordinary ranged profile instead of the
        // adjacent Spear-reach exception, including when equipped by Warriors.
        assert!(!Combat::equipped_weapon_has_fortification_reach(
            &throwing_spear_inventory
        ));
    }

    #[test]
    fn watchtower_stationed_profile_buffs_ranged_weapons() {
        let effects = Effects(HashMap::from([(Effect::WatchtowerLight, (0, 1.0, 1))]));
        let bow_inventory = inventory_with(equipped_test_weapon("Training Bow", "Bow", 3, 85));

        let profile =
            equipped_ranged_weapon_profile(&bow_inventory, &effects).expect("watchtower profile");

        assert_eq!(profile.range, 4);
        assert_eq!(profile.accuracy, Some(95));
        assert_eq!(profile.damage_bonus, WATCHTOWER_RANGED_DAMAGE_BONUS);
        assert_eq!(profile.stamina_cost, WATCHTOWER_RANGED_STAMINA_COST);
        assert_eq!(ranged_hit_chance(profile, 4), 65);
    }

    #[test]
    fn ranged_damage_abilities_are_not_watchtower_gated_when_attacker_is_fortified() {
        let guard_bash = ability_def("shield_bash").expect("guard bash ability");
        let aimed_shot = ability_def("aimed_shot").expect("aimed shot ability");
        let arcane_bolt = ability_def("arcane_bolt").expect("arcane bolt ability");
        let ward = ability_def("ward").expect("ward ability");

        let wall_only = Effects(HashMap::from([(Effect::Fortified, (0, 1.0, 1))]));
        let wall_and_tower = Effects(HashMap::from([
            (Effect::Fortified, (0, 1.0, 1)),
            (Effect::WatchtowerLight, (0, 1.0, 1)),
        ]));
        let outside = Effects(HashMap::new());

        assert_eq!(
            Combat::fortified_outbound_attack_error(
                &wall_only,
                Some(&Fortified { id: 9 }),
                &outside,
                None,
                ability_is_ranged_attack(guard_bash),
            ),
            Some(
                "Only ranged attacks or attacks with an equipped Spear can be used from behind a wall."
                    .to_string()
            )
        );
        assert_eq!(
            Combat::fortified_outbound_attack_error(
                &wall_only,
                Some(&Fortified { id: 9 }),
                &outside,
                None,
                ability_is_ranged_attack(aimed_shot),
            ),
            None
        );
        assert_eq!(
            Combat::fortified_outbound_attack_error(
                &wall_and_tower,
                Some(&Fortified { id: 9 }),
                &outside,
                None,
                ability_is_ranged_attack(aimed_shot),
            ),
            None
        );
        assert_eq!(
            Combat::fortified_outbound_attack_error(
                &wall_and_tower,
                Some(&Fortified { id: 9 }),
                &outside,
                None,
                ability_is_ranged_attack(arcane_bolt),
            ),
            None
        );
        assert!(!ability_is_damaging(ward));
    }

    #[test]
    fn shipwreck_is_not_attackable() {
        let class = Class(CLASS_POI.to_string());
        let template = Template("Shipwreck".to_string());

        assert!(!Combat::class_template_is_attackable(&class, &template));
        assert_eq!(
            Combat::non_attackable_class_template_error(&class, &template),
            Some("The shipwreck can only be inspected, not attacked.".to_string())
        );
    }

    #[test]
    fn starter_shipwreck_access_uses_run_association_without_restricting_other_objects() {
        let shipwreck = Template("Shipwreck".to_string());
        let campfire = Template("Campfire".to_string());
        let run_objects = RunSpawnedObjs(HashMap::from([(7, vec![101, 102])]));
        let searched = InvestigatedPOIs(HashMap::from([(7, HashSet::from([101]))]));

        assert!(can_access_run_shipwreck(7, 101, &shipwreck, &run_objects));
        assert!(!can_access_run_shipwreck(8, 101, &shipwreck, &run_objects));
        assert!(!can_access_run_shipwreck(7, 999, &shipwreck, &run_objects));
        assert!(can_access_run_shipwreck(8, 101, &campfire, &run_objects));
        assert!(can_access_shipwreck_inventory(
            7, 101, &shipwreck, &searched
        ));
        assert!(!can_access_shipwreck_inventory(
            7, 102, &shipwreck, &searched
        ));
        assert!(can_access_shipwreck_inventory(8, 101, &campfire, &searched));
    }

    #[test]
    fn mage_ward_uses_timed_sanctuary() {
        let ward = ability_def("ward").expect("ward ability");
        assert_eq!(ward.hero_class, HeroClass::Mage);
        assert_eq!(ward.cost_type, AbilityCostType::Mana);
        assert_eq!(ward.cost, 15);

        let mut effects = Effects(HashMap::new());
        let mut map_events = MapEvents::default();
        add_timed_effect(
            9,
            &mut effects,
            &mut map_events,
            200,
            Effect::Sanctuary,
            MAGE_WARD_DURATION_TICKS,
            MAGE_WARD_AMPLIFIER,
        );

        assert_eq!(
            effects.0.get(&Effect::Sanctuary),
            Some(&(200 + MAGE_WARD_DURATION_TICKS, MAGE_WARD_AMPLIFIER, 1))
        );
        assert!(map_events.values().any(|event| {
            event.obj_id == 9
                && event.run_tick == 200 + MAGE_WARD_DURATION_TICKS
                && matches!(
                    &event.event_type,
                    VisibleEvent::EffectExpiredEvent { effect }
                        if *effect == Effect::Sanctuary
                )
        }));
    }

    fn protected_presence(player_id: i32) -> PlayerWorldPresenceState {
        let mut presence = PlayerWorldPresenceState::default();
        let mut record = PlayerPresenceRecord::new(false);
        record.state = PlayerWorldPresence::OfflineProtected;
        presence.players.insert(player_id, record);
        presence
    }

    #[test]
    fn checkpoint2_player_event_classifier_keeps_read_only_and_lifecycle_events() {
        assert!(!PlayerEvent::Login {
            player_id: 1,
            connection_id: Uuid::nil(),
        }
        .is_mutating_gameplay());
        assert!(!PlayerEvent::RequestSafeLogout {
            player_id: 1,
            connection_id: Uuid::nil(),
        }
        .is_mutating_gameplay());
        assert!(!PlayerEvent::CancelSafeLogout {
            player_id: 1,
            connection_id: Uuid::nil(),
        }
        .is_mutating_gameplay());
        assert!(!PlayerEvent::NewPlayer {
            player_id: 1,
            hero_name: "Test".to_string(),
            class_name: "Warrior".to_string(),
            portrait: crate::obj::default_hero_portrait().to_string(),
        }
        .is_mutating_gameplay());
        assert!(!PlayerEvent::InfoInventory {
            player_id: 1,
            id: 100,
        }
        .is_mutating_gameplay());
        assert!(!PlayerEvent::DebugObj {
            player_id: 1,
            obj_id: 100,
        }
        .is_mutating_gameplay());

        assert!(PlayerEvent::Move {
            player_id: 1,
            x: 2,
            y: 3,
        }
        .is_mutating_gameplay());
        assert!(PlayerEvent::Craft {
            player_id: 1,
            recipe_name: "Firewood".to_string(),
            signature_item_id: None,
        }
        .is_mutating_gameplay());
        assert!(PlayerEvent::AddCraftingEntry {
            player_id: 1,
            structure_id: 100,
            recipe_name: "Firewood".to_string(),
        }
        .is_mutating_gameplay());
        assert!(PlayerEvent::DropItem {
            player_id: 1,
            item_id: 100,
        }
        .is_mutating_gameplay());
        assert!(PlayerEvent::CancelAction { player_id: 1 }.is_mutating_gameplay());
    }

    #[test]
    fn safe_logout_checkpoint3_bridge_consumes_only_safe_logout_commands() {
        let request_connection = Uuid::new_v4();
        let cancel_connection = Uuid::new_v4();
        let clients = Clients::default();
        let mut client_receivers = Vec::new();
        for (player_id, connection_id) in [(11, request_connection), (12, cancel_connection)] {
            let (sender, receiver) = tokio::sync::mpsc::channel(1);
            clients.activate(crate::game::Client {
                id: connection_id,
                player_id,
                sender,
            });
            client_receivers.push(receiver);
        }
        let mut app = App::new();
        app.add_message::<RequestSafeLogout>()
            .add_message::<CancelSafeLogout>()
            .insert_resource(clients)
            .init_resource::<SafeLogoutTelemetryState>()
            .insert_resource(PlayerEvents(HashMap::from([
                (
                    1,
                    PlayerEvent::RequestSafeLogout {
                        player_id: 11,
                        connection_id: request_connection,
                    },
                ),
                (
                    2,
                    PlayerEvent::CancelSafeLogout {
                        player_id: 12,
                        connection_id: cancel_connection,
                    },
                ),
                (
                    3,
                    PlayerEvent::InfoInventory {
                        player_id: 13,
                        id: 130,
                    },
                ),
                (
                    4,
                    PlayerEvent::RequestSafeLogout {
                        player_id: 11,
                        connection_id: Uuid::new_v4(),
                    },
                ),
            ])))
            .add_systems(Update, safe_logout_command_bridge_system);

        app.update();

        let events = app.world().resource::<PlayerEvents>();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events.get(&3),
            Some(PlayerEvent::InfoInventory {
                player_id: 13,
                id: 130
            })
        ));

        let mut request_cursor = app
            .world()
            .resource::<Messages<RequestSafeLogout>>()
            .get_cursor();
        let requests = app.world().resource::<Messages<RequestSafeLogout>>();
        assert_eq!(
            request_cursor
                .read(requests)
                .map(|request| request.player_id)
                .collect::<Vec<_>>(),
            vec![11]
        );

        let mut cancel_cursor = app
            .world()
            .resource::<Messages<CancelSafeLogout>>()
            .get_cursor();
        let cancellations = app.world().resource::<Messages<CancelSafeLogout>>();
        assert_eq!(
            cancel_cursor
                .read(cancellations)
                .map(|request| request.player_id)
                .collect::<Vec<_>>(),
            vec![12]
        );
        assert_eq!(
            app.world()
                .resource::<SafeLogoutTelemetryState>()
                .get(&11)
                .map(|telemetry| telemetry.stale_connection_events_rejected),
            Some(1)
        );
    }

    #[test]
    fn checkpoint4_stale_login_event_cannot_resume_or_schedule_sync() {
        let player_id = 21;
        let stale_connection = Uuid::new_v4();
        let current_connection = Uuid::new_v4();
        let clients = Clients::default();
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        clients.activate(crate::game::Client {
            id: current_connection,
            player_id,
            sender,
        });

        let mut app = App::new();
        app.insert_resource(clients)
            .insert_resource(PlayerEvents(HashMap::from([(
                1,
                PlayerEvent::Login {
                    player_id,
                    connection_id: stale_connection,
                },
            )])))
            .insert_resource(GameTick(100))
            .insert_resource(GameEvents::default())
            .insert_resource(Ids::default())
            .insert_resource(protected_presence(player_id))
            .init_resource::<SafeLogoutTelemetryState>()
            .add_systems(Update, login_system);

        app.update();

        assert!(app.world().resource::<PlayerEvents>().is_empty());
        assert!(app.world().resource::<GameEvents>().is_empty());
        assert_eq!(
            app.world()
                .resource::<PlayerWorldPresenceState>()
                .players
                .get(&player_id)
                .map(|record| record.state),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        assert_eq!(
            app.world()
                .resource::<SafeLogoutTelemetryState>()
                .get(&player_id)
                .map(|telemetry| telemetry.stale_connection_events_rejected),
            Some(1)
        );
    }

    #[test]
    fn checkpoint4_first_login_initializes_missing_presence_and_schedules_sync_once() {
        let player_id = 23;
        let connection_id = Uuid::new_v4();
        let clients = Clients::default();
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        clients.activate(crate::game::Client {
            id: connection_id,
            player_id,
            sender,
        });

        let mut app = App::new();
        app.insert_resource(clients)
            .insert_resource(PlayerEvents(HashMap::from([(
                1,
                PlayerEvent::Login {
                    player_id,
                    connection_id,
                },
            )])))
            .insert_resource(GameTick(100))
            .insert_resource(GameEvents::default())
            .insert_resource(Ids::default())
            .insert_resource(PlayerWorldPresenceState::default())
            .init_resource::<SafeLogoutTelemetryState>()
            .add_systems(Update, login_system);

        app.update();

        assert!(app.world().resource::<PlayerEvents>().is_empty());
        let record = app
            .world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&player_id)
            .cloned()
            .expect("first login initializes presence");
        assert_eq!(record.state, PlayerWorldPresence::Online);
        assert!(record.client_connected);
        assert_eq!(record.last_login_connection_id, Some(connection_id));
        let game_events = app.world().resource::<GameEvents>();
        assert_eq!(game_events.len(), 1);
        assert!(game_events.values().all(|event| matches!(
            &event.event_type,
            GameEventType::Login {
                player_id: event_player,
                connection_id: event_connection,
            } if *event_player == player_id && *event_connection == connection_id.as_u128()
        )));

        app.world_mut().resource_mut::<PlayerEvents>().insert(
            2,
            PlayerEvent::Login {
                player_id,
                connection_id,
            },
        );
        app.update();

        assert!(app.world().resource::<PlayerEvents>().is_empty());
        assert_eq!(
            app.world().resource::<GameEvents>().len(),
            1,
            "only a true duplicate Login is suppressed"
        );
    }

    #[test]
    fn checkpoint4_duplicate_login_event_schedules_one_connection_scoped_sync() {
        let player_id = 22;
        let connection_id = Uuid::new_v4();
        let clients = Clients::default();
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        clients.activate(crate::game::Client {
            id: connection_id,
            player_id,
            sender,
        });
        let mut presence = PlayerWorldPresenceState::default();
        presence
            .players
            .insert(player_id, PlayerPresenceRecord::new(true));

        let mut app = App::new();
        app.insert_resource(clients)
            .insert_resource(PlayerEvents(HashMap::from([
                (
                    1,
                    PlayerEvent::Login {
                        player_id,
                        connection_id,
                    },
                ),
                (
                    2,
                    PlayerEvent::Login {
                        player_id,
                        connection_id,
                    },
                ),
            ])))
            .insert_resource(GameTick(100))
            .insert_resource(GameEvents::default())
            .insert_resource(Ids::default())
            .insert_resource(presence)
            .init_resource::<SafeLogoutTelemetryState>()
            .add_systems(Update, login_system);

        app.update();

        assert!(app.world().resource::<PlayerEvents>().is_empty());
        let game_events = app.world().resource::<GameEvents>();
        assert_eq!(game_events.len(), 1);
        assert!(game_events.values().all(|event| matches!(
            &event.event_type,
            GameEventType::Login {
                player_id: event_player,
                connection_id: event_connection,
            } if *event_player == player_id && *event_connection == connection_id.as_u128()
        )));
    }

    #[test]
    fn checkpoint2_guard_rejects_protected_source_and_other_player_target() {
        let protected_player = 1;
        let active_player = 2;
        let protected_storage = 100;
        let active_hero = 200;

        let mut ids = Ids::default();
        ids.new_obj(protected_storage, protected_player);
        ids.new_obj(active_hero, active_player);

        let events = PlayerEvents(HashMap::from([
            (
                1,
                PlayerEvent::Move {
                    player_id: protected_player,
                    x: 1,
                    y: 1,
                },
            ),
            (
                2,
                PlayerEvent::ItemTransfer {
                    player_id: active_player,
                    item_id: 10,
                    source_id: active_hero,
                    target_id: protected_storage,
                },
            ),
            (
                3,
                PlayerEvent::InfoInventory {
                    player_id: protected_player,
                    id: protected_storage,
                },
            ),
            (
                4,
                PlayerEvent::Login {
                    player_id: protected_player,
                    connection_id: Uuid::nil(),
                },
            ),
            (
                5,
                PlayerEvent::Move {
                    player_id: active_player,
                    x: 2,
                    y: 2,
                },
            ),
        ]));

        let mut app = App::new();
        app.insert_resource(events)
            .insert_resource(ids)
            .insert_resource(protected_presence(protected_player))
            .init_resource::<SafeLogoutTelemetryState>()
            .add_systems(Update, protected_player_event_guard_system);
        app.update();

        let remaining = app.world().resource::<PlayerEvents>();
        assert!(!remaining.contains_key(&1), "protected source mutation");
        assert!(!remaining.contains_key(&2), "protected target mutation");
        assert!(remaining.contains_key(&3), "read-only inspection");
        assert!(remaining.contains_key(&4), "login lifecycle event");
        assert!(remaining.contains_key(&5), "other player mutation");
        let telemetry = app.world().resource::<SafeLogoutTelemetryState>();
        assert_eq!(
            telemetry
                .get(&protected_player)
                .map(|telemetry| telemetry.protected_input_rejections),
            Some(1)
        );
        assert_eq!(
            telemetry
                .get(&active_player)
                .map(|telemetry| telemetry.protected_input_rejections),
            Some(1)
        );
    }

    #[test]
    fn crafting_queue_cannot_overbook_the_same_inputs() {
        let inventory = Inventory {
            owner: 10,
            items: vec![Item {
                id: 1,
                owner: 10,
                name: "Wood".to_string(),
                quantity: 1,
                durability: None,
                class: "Material".to_string(),
                subclass: "Wood".to_string(),
                slot: None,
                image: "wood".to_string(),
                weight: 1.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };
        let recipe = crate::recipe::Recipe {
            name: "Firewood".to_string(),
            class: "Material".to_string(),
            subclass: "Firewood".to_string(),
            image: "firewood".to_string(),
            weight: 1.0,
            durability: None,
            attrs: None,
            owner: 1,
            tier: None,
            slot: None,
            damage: None,
            speed: None,
            armor: None,
            crafting_time: Some(10),
            structure_req: Some(vec!["Crafting Tent".to_string()]),
            stamina_req: None,
            skill_req: None,
            amount: Some(1),
            req: vec![ResReq {
                req_type: "Wood".to_string(),
                quantity: 1,
                cquantity: None,
            }],
            item_name_from_req: None,
        };
        let recipes = Recipes::from_recipes(vec![recipe.clone()]);
        let queue = WorkQueue(vec![WorkEntry {
            entry_id: 100,
            worker_id: -1,
            work_type: WorkType::Craft,
            work_status: WorkStatus::Idle,
            recipe_name: Some("Firewood".to_string()),
            recipe_image: Some("firewood".to_string()),
            refine_item_id: None,
            refine_item_image: None,
            refine_item_class: None,
        }]);

        assert!(!queued_craft_inputs_available(
            &inventory, &queue, &recipe, &recipes, 1
        ));
        assert_eq!(inventory.items[0].quantity, 1);
    }

    #[test]
    fn combo_prefix_hints_and_exact_finisher_match_templates() {
        let mut templates = Templates::from_obj_templates(Vec::new());
        templates
            .combo_templates
            .load(vec![crate::templates::ComboTemplate {
                name: "Hamstring".to_string(),
                attacks: vec!["quick".to_string(), "quick".to_string()],
                effects: vec!["Hamstrung".to_string()],
                quick_damage: 1.0,
                precise_damage: 1.0,
                fierce_damage: 1.0,
            }]);

        let (hints, finisher) = combo_hints_for_history(&vec!["quick".to_string()], &templates);
        assert_eq!(finisher, None);
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].name, "Hamstring");
        assert_eq!(hints[0].remaining_attacks, vec!["quick".to_string()]);

        let (hints, finisher) =
            combo_hints_for_history(&vec!["quick".to_string(), "quick".to_string()], &templates);
        assert!(hints.is_empty());
        assert_eq!(finisher.as_deref(), Some("Hamstring"));
    }

    fn tempo_test_templates() -> Templates {
        let mut templates = Templates::from_obj_templates(Vec::new());
        let combo = |name: &str, attacks: &[&str]| crate::templates::ComboTemplate {
            name: name.to_string(),
            attacks: attacks.iter().map(|attack| (*attack).to_string()).collect(),
            effects: Vec::new(),
            quick_damage: 1.0,
            precise_damage: 1.0,
            fierce_damage: 1.0,
        };
        templates.combo_templates.load(vec![
            combo("Hamstring", &["quick", "quick"]),
            combo("Intimidating Shout", &["fierce", "fierce"]),
            combo("Shrouded Slash", &["precise", "fierce", "quick"]),
            combo(
                "Nightmare Strike",
                &["fierce", "precise", "quick", "fierce"],
            ),
        ]);
        templates
    }

    #[test]
    fn combo_tempo_uses_the_decided_cooldown_ladder() {
        assert_eq!(combo_chain_cooldown_ticks(0), 30);
        assert_eq!(combo_chain_cooldown_ticks(1), 30);
        assert_eq!(combo_chain_cooldown_ticks(2), 25);
        assert_eq!(combo_chain_cooldown_ticks(3), 20);
        assert_eq!(combo_chain_cooldown_ticks(4), 15);
        assert_eq!(cooldown_seconds(combo_chain_cooldown_ticks(4)), 1.5);
    }

    #[test]
    fn combo_tempo_accelerates_only_strict_template_prefixes() {
        use crate::combat::AttackType::{Fierce, Precise, Quick};

        let templates = tempo_test_templates();
        let cooldown = |attacks: &[crate::combat::AttackType]| {
            combo_chain_cooldown_ticks(combo_tempo_prefix_len(attacks, &templates))
        };

        assert_eq!(cooldown(&[Quick]), 30);
        assert_eq!(cooldown(&[Precise, Fierce]), 25);
        assert_eq!(cooldown(&[Fierce, Precise, Quick]), 20);
        assert_eq!(cooldown(&[Quick, Quick]), 30);
        assert_eq!(cooldown(&[Fierce, Fierce]), 30);

        let mut history = Vec::new();
        for _ in 0..6 {
            history = Combat::next_combo_attacks(&history, Quick, &templates);
            assert_eq!(cooldown(&history), 30);
        }
    }

    #[test]
    fn stale_combo_history_cannot_make_a_finisher_ready() {
        use crate::combat::{AttackType::Quick, ComboTracker, COMBO_CHAIN_TIMEOUT_TICKS};

        let templates = tempo_test_templates();
        let tracker = ComboTracker {
            target_id: 9,
            attacks: vec![Quick, Quick],
            last_attack_tick: 100,
        };
        let attack_history =
            live_combo_history_for_target(Some(&tracker), 9, 100 + COMBO_CHAIN_TIMEOUT_TICKS + 1);
        let (_matching_combos, available_finisher) =
            combo_hints_for_history(&attack_history, &templates);

        assert!(attack_history.is_empty());
        assert_eq!(
            combo_finisher_rejection(available_finisher.as_deref(), false),
            Some("No combo is ready.")
        );
    }

    #[test]
    fn villager_combo_timeout_clears_tracker_without_sending_combat_state() {
        use crate::combat::{AttackType::Quick, ComboTracker, COMBO_CHAIN_TIMEOUT_TICKS};

        let player_id = 7;
        let (clients, mut receiver) = connected_test_client(player_id);
        let mut app = App::new();
        let villager = app
            .world_mut()
            .spawn((
                PlayerId(player_id),
                SubclassVillager,
                ComboTracker {
                    target_id: 99,
                    attacks: vec![Quick],
                    last_attack_tick: 10,
                },
            ))
            .id();
        app.insert_resource(GameTick(10 + COMBO_CHAIN_TIMEOUT_TICKS + 1))
            .insert_resource(clients)
            .insert_resource(EntityObjMap(HashMap::new()))
            .add_systems(Update, combo_tracker_timeout_system);

        app.update();

        let tracker = app.world().entity(villager).get::<ComboTracker>().unwrap();
        assert!(tracker.attacks.is_empty());
        assert_eq!(tracker.target_id, -1);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn hero_combo_timeout_sends_empty_combat_state() {
        use crate::combat::{AttackType::Quick, ComboTracker, COMBO_CHAIN_TIMEOUT_TICKS};

        let player_id = 7;
        let (clients, mut receiver) = connected_test_client(player_id);
        let mut app = App::new();
        app.world_mut().spawn((
            PlayerId(player_id),
            SubclassHero,
            ComboTracker {
                target_id: 99,
                attacks: vec![Quick],
                last_attack_tick: 10,
            },
        ));
        app.insert_resource(GameTick(10 + COMBO_CHAIN_TIMEOUT_TICKS + 1))
            .insert_resource(clients)
            .insert_resource(EntityObjMap(HashMap::new()))
            .add_systems(Update, combo_tracker_timeout_system);

        app.update();

        let packet: ResponsePacket =
            serde_json::from_str(&receiver.try_recv().expect("hero timeout state")).unwrap();
        assert!(matches!(
            packet,
            ResponsePacket::CombatState {
                target_id: 99,
                attack_history,
                available_finisher: None,
                ..
            } if attack_history.is_empty()
        ));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn combat_effect_updates_skip_villager_tracker_and_serve_hero_tracker() {
        use crate::combat::{AttackType::Quick, ComboTracker};

        let player_id = 7;
        let target_id = 99;
        let (clients, mut receiver) = connected_test_client(player_id);
        let mut app = App::new();
        app.insert_resource(clients)
            .insert_resource(tempo_test_templates())
            .add_observer(combat_effects_changed_observer);

        let target = spawn_ability_test_actor(
            app.world_mut(),
            target_id,
            NPC_PLAYER_ID,
            Position { x: 1, y: 0 },
            Subclass::Npc,
            TestAbilityTarget,
        );
        let villager = spawn_ability_test_actor(
            app.world_mut(),
            8,
            player_id,
            Position { x: 0, y: 0 },
            Subclass::Villager,
            SubclassVillager,
        );
        app.world_mut().entity_mut(villager).insert(ComboTracker {
            target_id,
            attacks: vec![Quick],
            last_attack_tick: 10,
        });
        app.insert_resource(EntityObjMap(HashMap::from([(target_id, target)])));

        app.world_mut().trigger(CombatEffectsChanged { target_id });
        assert!(receiver.try_recv().is_err());

        let hero = spawn_ability_test_actor(
            app.world_mut(),
            7,
            player_id,
            Position { x: 0, y: 0 },
            Subclass::Hero,
            SubclassHero,
        );
        app.world_mut().entity_mut(hero).insert(ComboTracker {
            target_id,
            attacks: vec![Quick],
            last_attack_tick: 10,
        });

        app.world_mut().trigger(CombatEffectsChanged { target_id });
        let packet: ResponsePacket =
            serde_json::from_str(&receiver.try_recv().expect("hero effect state")).unwrap();
        assert!(matches!(
            packet,
            ResponsePacket::CombatState {
                target_id: 99,
                attack_history,
                ..
            } if attack_history == vec!["quick".to_string()]
        ));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn ready_finisher_ignores_active_basic_cooldown_but_missing_finisher_does_not() {
        assert!(combo_finisher_can_fire(Some("Hamstring"), true));
        assert!(combo_finisher_can_fire(Some("Hamstring"), false));
        assert!(!combo_finisher_can_fire(None, false));
    }

    #[test]
    fn combo_discovery_is_sent_once_per_combo_and_hero_run() {
        let mut discoveries = HashSet::new();
        assert!(is_first_combo_discovery(&mut discoveries, 10, "Hamstring"));
        assert!(!is_first_combo_discovery(&mut discoveries, 10, "Hamstring"));
        assert!(is_first_combo_discovery(&mut discoveries, 10, "Gouge"));
        assert!(is_first_combo_discovery(&mut discoveries, 11, "Hamstring"));
    }
}
