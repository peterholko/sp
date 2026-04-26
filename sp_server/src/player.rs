use bevy::ecs::query::{QueryData, WorldQuery};
use bevy::prelude::*;
use serde::Deserialize;
use std::env;
use std::fs;
use std::sync::{Arc, Mutex};
use tracing_subscriber::{reload, EnvFilter, Registry};
use uuid::Uuid;

use std::collections::{HashMap, HashSet, VecDeque};

use crate::common::{Heat, Hunger, Idle, Thirst, Tired, Transport};
use crate::constants::*;
use crate::event::{GameEvent, GameEventType, GameEvents, MapEvents, Spell, VisibleEvent};
use crate::farm::Crops;
use crate::ids::{EntityObjMap, Ids};

use crate::combat::{Combat, CombatQuery, CombatQueryItem};
use crate::effect::{Effect, Effects};
use crate::experiment::{self, Experiment, ExperimentState, Experiments};
use crate::game::{
    is_pos_empty, Clients, DamageRecord, DebugObjs, GameTick, InitialEncounterState,
    LogLevelOverrides, Merchant, Monolith, MonolithInvestigation, MonolithProgress,
    NetworkReceiver, ObjQuery, Objectives, PlayerIntroState, PlayerObjectives, PlayerRunScore,
    PlayerStat, PlayerStats, RunScoreState, SpawnPositions, WeakSanctuary,
};
use crate::item::{self, AttrKey, AttrVal, Inventory, Item};
use crate::map::Map;
use crate::network::{
    self, send_to_client, CraftingItem, RefiningItem, ResponsePacket, StatsData, StructureList,
};
use crate::obj::{
    is_combat_locked, ActiveTask, Assignment, Assignments, BaseAttrs, BuildProgressUpdate,
    BuildUpgradeState, Campfire, Class, ClassStructure, EndRepeatAction, HeroClass,
    HeroClassProfile, Id, LastCombatTick, Misc, Name, NewObj, Obj, Order, Personality, PlayerId,
    Position, RemoveObj, SelectedUpgrade, Shelter, StartBuild, StartUpgrade, State, StateBuilding,
    StateChange, StateDead, Stats, Subclass, SubclassHero, SubclassVillager, Template, UpdateObj,
    Viewshed, WorkEntry, WorkQueue, WorkStatus, WorkType,
};
use crate::player_setup::StartLocations;
use crate::recipe::Recipes;
use crate::resource::{Resource, Resources};
use crate::skill::{SkillData, Skills, MAX_RANK};
use crate::skill_defs::Skill;
use crate::structure::{self, Plans, Structure, WALL};
use crate::templates::{ObjTemplate, ResReq, Templates};
use crate::terrain_feature::{TerrainFeature, TerrainFeatures};
use crate::trade::{Prices, WantedItem};
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
    },
    Login {
        player_id: i32,
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
    misc: &'static Misc,
    effects: &'static Effects,
    inventory: &'static Inventory,
    hero_class: Option<&'static HeroClass>,
    last_combat_tick: Option<&'static LastCombatTick>,
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

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // Initialize events
        let player_events: PlayerEvents = PlayerEvents(HashMap::new());
        let active_infos: ActiveInfos = ActiveInfos(HashMap::new());

        let start_file =
            fs::File::open("templates/player_start.yaml").expect("Could not open file.");
        let start_locations =
            StartLocations(serde_yaml::from_reader(start_file).expect("Could not read values."));

        app.add_systems(
            Update,
            (
                message_broker_system,
                new_player_system,
                login_system,
                move_system,
                attack_system,
            )
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
                item_split_system,
                info_refine_system,
                order_follow_system,
                order_gather_system,
                order_operate_system,
                structure_queue_system,
                order_farm_system,
                order_repair_system,
            )
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
                explore_system,
                info_assign_system,
                assign_system,
                equip_system,
                info_craft_system,
                info_structure_craft_system,
                info_structure_queue_system,
                order_explore_system,
                use_item_system,
                remove_system,
                set_experiment_item_system,
                hire_system,
                buy_sell_system,
                activate_system,
            )
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
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (info_obj_system,).run_if(in_state(AppState::Running)),
        )
        .add_observer(info_hero_system)
        .add_observer(info_villager_system)
        .add_observer(info_structure_system)
        .add_observer(info_monolith_system)
        .add_observer(info_poi_system)
        .add_observer(info_npc_system)
        .insert_resource(player_events)
        .insert_resource(active_infos)
        .insert_resource(start_locations);
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

fn new_player_system(
    mut events: ResMut<PlayerEvents>,
    mut commands: Commands,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    mut entity_map: ResMut<EntityObjMap>,
    mut start_locations: ResMut<StartLocations>,
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
    mut player_intro_state: ResMut<PlayerIntroState>,
    mut initial_encounter_state: ResMut<InitialEncounterState>,
    monoliths: Query<ObjQuery, With<Monolith>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::NewPlayer {
                player_id,
                hero_name,
                class_name,
            } => {
                events_to_remove.push(*event_id);
                let setup_result = {
                    let mut spawn_positions = player_setup_state.p1();
                    player_setup::new(
                        *player_id,
                        hero_name.to_string(),
                        class_name.to_string(),
                        &mut commands,
                        &mut start_locations,
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
                        &mut player_intro_state,
                        &mut initial_encounter_state,
                    )
                };

                match setup_result {
                    Ok(_) => {
                        let event_type = GameEventType::Login {
                            player_id: *player_id,
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
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Login { player_id } => {
                events_to_remove.push(*event_id);

                let event_type = GameEventType::Login {
                    player_id: *player_id,
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
                            | VisibleEvent::ExploreEvent
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

fn enemy_intent_for_template(template: &str) -> String {
    match template {
        "Giant Rat" | "Spider" | "Scorpion" => "Fast creature looking for an opening".to_string(),
        "Wolf" | "Wild Boar" | "Giant Crab" => {
            "Close-range attacker testing your position".to_string()
        }
        "Zombie" | "Skeleton" | "Shipwreck Zombie" | "Shadow" => {
            "Undead pressure advancing steadily".to_string()
        }
        "Necromancer" => "Caster seeking distance and corpses to exploit".to_string(),
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
        "Giant Rat" | "Spider" | "Wolf" => {
            "Fast enemies reward control: quick chains toward Hamstring, while block protects low stamina.".to_string()
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
            cooldown: 5,
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
            cooldown: 5,
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
            cooldown: 5,
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
            cooldown: 5,
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
            cooldown: 5,
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

    if ability_cost_value(actor, ability.cost_type) < ability.cost {
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

        if Map::dist(*actor.pos, *target.pos) > ability.range {
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
            cost: ability.cost,
            range: ability.range as i32,
            disabled_reason: ability_disabled_reason(*ability, actor, target),
            hint: ability.hint.to_string(),
        })
        .collect()
}

fn spend_ability_cost(actor: &mut CombatQueryItem, ability: AbilityDef) {
    match ability.cost_type {
        AbilityCostType::Stamina => {
            let stamina = actor.stats.stamina.unwrap_or(0);
            actor.stats.stamina = Some(stamina - ability.cost);
        }
        AbilityCostType::Mana => {
            let mana = actor.stats.mana.unwrap_or(0);
            actor.stats.mana = Some(mana - ability.cost);
        }
    }
}

fn ability_response_packet(source_id: i32, ability: AbilityDef) -> ResponsePacket {
    ResponsePacket::Ability {
        source_id,
        ability_id: ability.id.to_string(),
        cooldown: ability.cooldown,
        stamina_cost: match ability.cost_type {
            AbilityCostType::Stamina => Some(ability.cost),
            AbilityCostType::Mana => None,
        },
        mana_cost: match ability.cost_type {
            AbilityCostType::Stamina => None,
            AbilityCostType::Mana => Some(ability.cost),
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
    effects
        .0
        .insert(effect.clone(), (duration_ticks, amplifier, 1));
    map_events.new(
        obj_id,
        game_tick + duration_ticks,
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
    target.stats.hp -= damage;
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
        commands.entity(target.entity).insert(StateDead {
            dead_at: game_tick.0,
            killer: actor.template.0.clone(),
        });
    }

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
    let packet = ResponsePacket::CombatState {
        version: 1,
        target_id,
        enemy_intent: enemy_intent_for_template(&target_template),
        attack_history: attack_history.clone(),
        matching_combos,
        available_finisher,
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
    mut last_player_attack: Local<HashMap<i32, i32>>,
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

                // Check if attacker has enough stamina
                let attacker_stamina = attacker.stats.stamina.expect("Missing stamina stat");
                if attacker_stamina < 5 {
                    let packet = ResponsePacket::Error {
                        errmsg: "Not enough stamina to attack.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Check global attack cooldown (per-player, not affected by being attacked)
                let last_attack = last_player_attack.get(player_id).copied().unwrap_or(0);
                if last_attack > 0 && (game_tick.0 - last_attack) < ATTACK_COOLDOWN_TICKS {
                    let packet = ResponsePacket::Error {
                        errmsg: "Attack is on cooldown.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let mut attack_history = attacker
                    .combo_tracker
                    .as_ref()
                    .filter(|combo_tracker| combo_tracker.target_id == target.id.0)
                    .map(|combo_tracker| {
                        combo_tracker
                            .attacks
                            .iter()
                            .map(|attack| attack.clone().to_str())
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_else(Vec::new);
                attack_history.push(attack_type.clone());
                let target_template = target.template.0.clone();

                // Calculate and process damage
                let (damage, combo, skill_updated) = Combat::process_attack(
                    Combat::attack_type_to_enum(attack_type.to_string()),
                    &mut attacker,
                    &mut target,
                    &mut commands,
                    &templates,
                    &map,
                    &mut ids,
                    &game_tick,
                    &mut map_events,
                );

                // Add visible damage event to broadcast to everyone nearby
                Combat::add_damage_event(
                    game_tick.0,
                    attack_type.to_string(),
                    damage,
                    combo,
                    &attacker,
                    &target,
                    &mut map_events,
                );

                // Track player attack cooldown
                attacker.last_combat_tick.0 = game_tick.0;
                last_player_attack.insert(*player_id, game_tick.0);

                // Response to client with attack response packet
                let packet = ResponsePacket::Attack {
                    source_id: *source_id,
                    attack_type: attack_type.clone(),
                    cooldown: 5,
                    stamina_cost: 5,
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
                    if let Some(mut attacker_skills) = attacker.skills {
                        let skill_name = Skill::from_str(&skill_updated.xp_type)
                            .expect(&format!("Invalid skill name: {}", skill_updated.xp_type));
                        attacker_skills.update(
                            skill_name,
                            skill_updated.xp,
                            &templates.skill_templates,
                        );
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

                let last_attack = last_player_attack.get(player_id).copied().unwrap_or(0);
                if last_attack > 0 && (game_tick.0 - last_attack) < ATTACK_COOLDOWN_TICKS {
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

                    spend_ability_cost(&mut attacker, ability);
                    match ability.effect {
                        AbilityEffect::Ward => {
                            add_timed_effect(
                                attacker.id.0,
                                &mut attacker.effects,
                                &mut map_events,
                                game_tick.0,
                                Effect::WeakSanctuary,
                                MAGE_WARD_DURATION_TICKS,
                                MAGE_WARD_AMPLIFIER,
                            );
                            commands.entity(attacker.entity).insert(WeakSanctuary {
                                id: attacker.id.0,
                                pos: *attacker.pos,
                            });
                            attacker.last_combat_tick.0 = game_tick.0;
                        }
                        _ => {}
                    }

                    last_player_attack.insert(*player_id, game_tick.0);
                    send_to_client(
                        *player_id,
                        ability_response_packet(*source_id, ability),
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

                if let Some(reason) = ability_disabled_reason(ability, &attacker, Some(&target)) {
                    let packet = ResponsePacket::Error { errmsg: reason };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let target_template = target.template.0.clone();

                match ability.effect {
                    AbilityEffect::ShieldBash => {
                        let damage_amount = 3 + attacker.stats.base_damage.unwrap_or(0);
                        spend_ability_cost(&mut attacker, ability);
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
                            &attacker,
                            &target,
                            &mut map_events,
                        );
                    }
                    AbilityEffect::AimedShot => {
                        let damage_amount = 4
                            + attacker.stats.base_damage.unwrap_or(0)
                            + equipped_damage(&attacker, Some("Bow"));
                        spend_ability_cost(&mut attacker, ability);
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
                            &attacker,
                            &target,
                            &mut map_events,
                        );
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

                        spend_ability_cost(&mut attacker, ability);
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
                        spend_ability_cost(&mut attacker, ability);
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

                last_player_attack.insert(*player_id, game_tick.0);
                send_to_client(
                    *player_id,
                    ability_response_packet(*source_id, ability),
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

                // Check if attacker has enough stamina
                let attacker_stamina = attacker.stats.stamina.expect("Missing stamina stat");
                if attacker_stamina < 5 {
                    let packet = ResponsePacket::Error {
                        errmsg: "Not enough stamina to attack.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let attack_history = attacker
                    .combo_tracker
                    .as_ref()
                    .filter(|combo_tracker| combo_tracker.target_id == target.id.0)
                    .map(|combo_tracker| {
                        combo_tracker
                            .attacks
                            .iter()
                            .map(|attack| attack.clone().to_str())
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                let (_matching_combos, available_finisher) =
                    combo_hints_for_history(&attack_history, &templates);
                if available_finisher.is_none() {
                    let packet = ResponsePacket::Error {
                        errmsg: "No combo is ready.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let target_template = target.template.0.clone();

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
                    &attacker,
                    &target,
                    &mut map_events,
                );

                // Track player attack cooldown
                last_player_attack.insert(*player_id, game_tick.0);

                // Response to client with attack response packet
                let packet = ResponsePacket::Attack {
                    source_id: *source_id,
                    attack_type: "combo".to_string(),
                    cooldown: 5,
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

                if let Some(combo_name) = combo.clone() {
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
                    if let Some(mut attacker_skills) = attacker.skills {
                        let skill_name = Skill::from_str(&skill_updated.xp_type)
                            .expect(&format!("Invalid skill name: {}", skill_updated.xp_type));
                        attacker_skills.update(
                            skill_name,
                            skill_updated.xp,
                            &templates.skill_templates,
                        );
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
            } => {
                events_to_remove.push(*event_id);

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
                let last_attack = last_player_attack.get(player_id).copied().unwrap_or(0);
                if last_attack > 0 && (game_tick.0 - last_attack) < ATTACK_COOLDOWN_TICKS {
                    let packet = ResponsePacket::Error {
                        errmsg: "Attack is on cooldown.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let (brace_duration, brace_amp) =
                    if matches!(attacker.hero_class, Some(&HeroClass::Warrior)) {
                        if let (Some(stamina), Some(base_stamina)) =
                            (attacker.stats.stamina, attacker.stats.base_stamina)
                        {
                            attacker.stats.stamina = Some((stamina + 3).min(base_stamina));
                        }
                        (WARRIOR_BRACE_DURATION_TICKS, WARRIOR_BRACE_AMPLIFIER)
                    } else {
                        (STANDARD_BRACE_DURATION_TICKS, STANDARD_BRACE_AMPLIFIER)
                    };

                // Apply Bracing effect. Warriors get a longer, stronger Iron Stance.
                add_timed_effect(
                    attacker.id.0,
                    &mut attacker.effects,
                    &mut map_events,
                    game_tick.0,
                    Effect::Bracing,
                    brace_duration,
                    brace_amp,
                );

                last_player_attack.insert(*player_id, game_tick.0);

                let packet = ResponsePacket::Attack {
                    source_id: *source_id,
                    attack_type: "block".to_string(),
                    cooldown: 5,
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
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    resources: Res<Resources>,
    hero_query: Query<(&Position, &State, &mut Inventory, Option<&LastCombatTick>)>,
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

                let Ok((hero_pos, hero_state, hero_inventory, last_combat_tick)) =
                    hero_query.get(hero_entity)
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

                if combat_locked(last_combat_tick, game_tick.0) {
                    send_combat_locked_error(*player_id, &clients);
                    continue;
                }

                if let Some(equipped_main_hand) = hero_inventory.get_equipped_main_hand() {
                    let mut resource_type = None;

                    if equipped_main_hand.attrs.get(&AttrKey::Mining).is_some() {
                        resource_type = Some(ORE.to_string());
                    } else if equipped_main_hand.attrs.get(&AttrKey::Logging).is_some() {
                        resource_type = Some(LOG.to_string());
                    } else if equipped_main_hand
                        .attrs
                        .get(&AttrKey::Stonecutting)
                        .is_some()
                    {
                        resource_type = Some(STONE.to_string());
                    } else if equipped_main_hand.attrs.get(&AttrKey::Fishing).is_some() {
                        resource_type = Some(FISH.to_string());
                    } else if equipped_main_hand.attrs.get(&AttrKey::Farming).is_some() {
                        resource_type = Some(FOOD.to_string());
                    } else if equipped_main_hand.attrs.get(&AttrKey::Foraging).is_some() {
                        resource_type = Some(PLANT.to_string());
                    } else if equipped_main_hand.attrs.get(&AttrKey::Hunting).is_some() {
                        resource_type = Some(GAME_ANIMAL.to_string());
                    }

                    if let Some(resource_type) = resource_type {
                        debug!("Resource type: {:?}", resource_type);
                        // Check if resource exists on tile
                        if !Resource::is_valid_type(resource_type.clone(), *hero_pos, &resources) {
                            error!("No {:?} found on tile {:?}", resource_type, *hero_pos);
                            let packet = ResponsePacket::Error {
                                errmsg: format!("No {} found on tile", resource_type),
                            };
                            send_to_client(*player_id, packet, &clients);
                            continue;
                        }

                        commands.trigger(StateChange {
                            entity: hero_entity,
                            new_state: State::Gathering,
                        });

                        // Add Gather Event
                        let event = GameEvent {
                            event_id: ids.new_map_event_id(),
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + 40,
                            event_type: GameEventType::GatherEvent {
                                gatherer_id: hero_id,
                                res_type: resource_type.clone(),
                            },
                        };

                        game_events.insert(event.event_id, event);

                        let packet = ResponsePacket::Gather { gather_time: 40 };
                        send_to_client(*player_id, packet, &clients);
                    } else {
                        error!("Invalid resource type for gathering");
                    }
                } else {
                    // If no tool is equipped, default to foraging

                    //Gathering state change
                    commands.trigger(StateChange {
                        entity: hero_entity,
                        new_state: State::Gathering,
                    });

                    // Add Forage Event
                    let event = GameEvent {
                        event_id: ids.new_map_event_id(),
                        start_tick: game_tick.0,
                        run_tick: game_tick.0 + 40,
                        event_type: GameEventType::ForageEvent {
                            forager_id: hero_id,
                        },
                    };

                    game_events.insert(event.event_id, event);

                    let packet = ResponsePacket::Gather { gather_time: 40 };
                    send_to_client(*player_id, packet, &clients);
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn gather_farm_refine_craft_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    game_tick: ResMut<GameTick>,
    ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut map_events: ResMut<MapEvents>,
    game_events: ResMut<GameEvents>,
    resources: Res<Resources>,
    templates: Res<Templates>,
    recipes: Res<Recipes>,
    active_infos: ResMut<ActiveInfos>,
    hero_query: Query<CoreQuery, With<SubclassHero>>,
    structure_query: Query<StructureQuery, With<ClassStructure>>,
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

                let nearby_resources = Resource::get_nearby_resources(*hero.pos, &resources);

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

                // Equipped item should have the ability to harvest
                // Check if structure contains seeds
                if !hero.inventory.has_by_class(item::HARVESTING.to_string()) {
                    trace!("Require a harvesting tool to harvest the crop.");
                    let packet = ResponsePacket::Error {
                        errmsg: "Require a harvesting tool to harvest the crop.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    break;
                }

                //Harvesting state change
                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Harvesting,
                });

                let plant_event = VisibleEvent::HarvestEvent {
                    structure_id: structure.id.0,
                };

                map_events.new(
                    hero.id.0,
                    game_tick.0 + 100, // in the future
                    plant_event,
                );
            }
            PlayerEvent::Operate {
                player_id,
                structure_id,
            } => {
                debug!("PlayerEvent::Operate");
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
    templates: Res<Templates>,
    recipes: Res<Recipes>,
    mut active_infos: ResMut<ActiveInfos>,
    hero_query: Query<(
        &Position,
        &State,
        &mut Inventory,
        &mut Skills,
        Option<&LastCombatTick>,
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

                let Ok((_hero_pos, hero_state, hero_inventory, hero_skills, last_combat_tick)) =
                    hero_query.get(hero_entity)
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

                if !hero_skills.has_skill_level(refine_skill, refine_skill_req) {
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

                let Ok((_hero_pos, hero_state, hero_inventory, _hero_skills, last_combat_tick)) =
                    hero_query.get(hero_entity)
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

                let Some(recipe) = recipes.get_by_name(recipe_name.clone()) else {
                    error!("Invalid recipe name {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Invalid recipe".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                if !hero_inventory.has_reqs(recipe.req.clone()) {
                    error!("Insufficient resources to craft {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient resources to craft".to_string(),
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

                if !hero_skills.has_skill_level(refine_skill, refine_skill_req) {
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
            } => {
                debug!("PlayerEvent::StructureCraft");
                events_to_remove.push(*event_id);

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

                let Some(recipe) = recipes.get_by_name(recipe_name.clone()) else {
                    error!("Invalid recipe name {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Invalid recipe".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                if !structure_inventory.has_reqs(recipe.req.clone()) {
                    error!("Insufficient resources to craft {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Insufficient resources to craft".to_string(),
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
        image: obj.misc.image.clone(),
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
    active_task_query: Query<&ActiveTask>,
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

    if let Ok(current_order) = order_query.get(obj.entity) {
        order = Some(current_order.to_string());
    }

    if let Ok(active_task) = active_task_query.get(obj.entity) {
        activity = Some(active_task.to_string());
    }

    let response_packet = ResponsePacket::InfoVillager {
        id: obj.id.0,
        name: obj.name.0.to_string(),
        template: obj.template.0.to_string(),
        class: obj.class.0.to_string(),
        subclass: obj.subclass.to_string(),
        state: obj.state.to_string(),
        image: obj.misc.image.clone(),
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
    monolith_query: Query<&Monolith>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
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

        let Ok(monolith) = monolith_query.get(obj.entity) else {
            error!("Cannot find monolith component for {:?}", obj.entity);
            return;
        };

        (
            obj.id.0,
            obj.name.0.to_string(),
            obj.class.0.to_string(),
            obj.subclass.to_string(),
            obj.template.0.to_string(),
            obj.misc.image.clone(),
            monolith.soulshards,
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
    query: Query<CoreQuery>,
    mut objectives: ResMut<Objectives>,
) {
    let Ok(obj) = query.get(info_poi_event.entity) else {
        error!("Cannot find obj for {:?}", info_poi_event.entity);
        return;
    };

    let items_packet = Some(obj.inventory.get_packet());

    let response_packet = ResponsePacket::InfoPOI {
        id: obj.id.0,
        name: obj.name.0.to_string(),
        class: obj.class.0.to_string(),
        subclass: obj.subclass.to_string(),
        template: obj.template.0.to_string(),
        image: obj.misc.image.clone(),
        items: items_packet,
    };

    send_to_client(info_poi_event.player_id, response_packet, &clients);

    // Mark explore_poi objective as completed
    let player_obj = objectives
        .entry(info_poi_event.player_id)
        .or_insert_with(PlayerObjectives::default);
    if obj.template.0 == "Shipwreck" && !player_obj.scavenge_shipwreck {
        player_obj.scavenge_shipwreck = true;
        let discovery_packet = ResponsePacket::DiscoveryEvent {
            version: 1,
            discovery_type: "poi".to_string(),
            title: "Shipwreck scavenged".to_string(),
            unlock_source: "First Hour".to_string(),
            location: Some(format!("{},{}", obj.pos.x, obj.pos.y)),
            result: "The wreck teaches the first rule: inspect places, recover supplies, and turn danger into tools.".to_string(),
        };
        send_to_client(info_poi_event.player_id, discovery_packet, &clients);
    }

    if !player_obj.explore_poi {
        player_obj.explore_poi = true;
        let objectives_packet = ResponsePacket::Objectives {
            build_campfire: player_obj.build_campfire,
            build_3_structures: player_obj.build_3_structures,
            recruit_villager: player_obj.recruit_villager,
            explore_poi: player_obj.explore_poi,
            survive_5_nights: player_obj.survive_5_nights,
        };
        send_to_client(info_poi_event.player_id, objectives_packet, &clients);
    }
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
    terrain_features: Res<TerrainFeatures>,
    obj_query: Query<ObjQuery>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::InfoTile { player_id, x, y } => {
                debug!("PlayerEvent::InfoTile x: {:?} y: {:?}", *x, *y);
                events_to_remove.push(*event_id);

                let tile_type = Map::tile_type(*x, *y, &map);
                let mut sanctuary = "None".to_string();

                for obj in obj_query.iter() {
                    if obj.subclass.is_monolith() {
                        if Map::dist(Position { x: *x, y: *y }, *obj.pos) <= SANCTUARY_RANGE {
                            sanctuary = "Strong".to_string();
                        } else if Map::dist(Position { x: *x, y: *y }, *obj.pos)
                            <= WEAK_SANCTUARY_RANGE
                        {
                            sanctuary = "Weak".to_string();
                        }
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
                    ),
                    sanctuary: sanctuary,
                    passable: Map::is_passable(*x, *y, &map),
                    wildness: map.get_wildness_string(*x, *y),
                    resources: Resource::get_on_tile(Position { x: *x, y: *y }, &resources),
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
                    resources: Resource::get_on_tile(Position { x: *x, y: *y }, &resources),
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

                let Ok((_pid, _obj_name, _obj_template, inventory)) = query.get(entity) else {
                    error!("Cannot find obj template or inventory for {:?}", entity);
                    break;
                };

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
                            produces: item_template.produces.clone(),
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

                let mut produces_list = Vec::new();

                for produce in produces.iter() {
                    let produce_template =
                        Item::get_template(produce.to_string(), &templates.item_templates);

                    produces_list.push(network::ProducedItem {
                        name: produce_template.name.clone(),
                        image: produce_template.image.clone(),
                        class: produce_template.class.clone(),
                        subclass: produce_template.subclass.clone(),
                    });
                }

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
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut active_infos: ResMut<ActiveInfos>,
    mut query: Query<ItemTransferQuery>,
    selected_upgrade_query: Query<&SelectedUpgrade>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::ItemTransfer {
                player_id,
                source_id,
                target_id,
                item_id,
            } => {
                events_to_remove.push(*event_id);

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

                // Structure is not completed
                if target.class.is_structure() {
                    if !Structure::is_built(*target.state) {
                        let packet = ResponsePacket::Error {
                            errmsg: "Structure is not completed.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }
                }

                // Transfer target does not have enough capacity
                let target_total_weight = target.inventory.get_total_weight();
                let transfer_item_weight = (item.quantity as f32 * item.weight) as i32;
                let target_capacity =
                    Obj::get_capacity(&target.template.0, &templates.obj_templates);

                let is_founded_or_planning_upgrade =
                    *target.state == State::Founded || *target.state == State::PlanningUpgrade;

                info!(
                    "Item transfer target.class: {:?} target.template: {:?}",
                    target.class.0, target.template.0
                );

                // Structure founded and under construction use case
                if target.class.0 == "structure" && is_founded_or_planning_upgrade {
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
                    if !Item::is_req(item.clone(), req.clone()) {
                        info!("Item not required for construction: {:?}", item);
                        let packet = ResponsePacket::Error {
                            errmsg: "Item not required for construction.".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }

                    let mut req_items = target.inventory.process_req_items(req);

                    // Find first matching req item
                    let matching_req_item = req_items.iter_mut().find(|r| {
                        r.req_type == item.name
                            || r.req_type == item.class
                            || r.req_type == item.subclass
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
                } else if owner.class.0 == "structure" && is_founded_or_planning_upgrade {
                    info!("Transfering from owner structure with state founded.");

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

                    Inventory::transfer(item.id, &mut owner.inventory, &mut target.inventory);

                    let req_items = owner.inventory.process_req_items(req);

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
                } else if target.class.0 == "structure" && target.template.0 == "Tent" {
                    info!("Allow fueling of campfire with wood");

                    // Only allow wood to be used to fuel campfire
                    if item.class == item::FUEL.to_string() {
                        let target_total_weight = target.inventory.get_total_weight();
                        let remaining_capacity = target_capacity - target_total_weight;

                        if transfer_item_weight > remaining_capacity {
                            let num_to_transfer = remaining_capacity / item.weight as i32;

                            Inventory::transfer_quantity(
                                item.id,
                                ids.new_item_id(),
                                &mut owner.inventory,
                                &mut target.inventory,
                                num_to_transfer,
                                &templates.item_templates,
                            );
                        } else {
                            Inventory::transfer(
                                item.id,
                                &mut owner.inventory,
                                &mut target.inventory,
                            );
                        }

                        let source_capacity =
                            Obj::get_capacity(&owner.template.0, &templates.obj_templates);
                        let source_total_weight = owner.inventory.get_total_weight();

                        let source_items = owner.inventory.get_packet().clone();
                        let target_items = target.inventory.get_packet().clone();

                        let source_inventory = network::Inventory {
                            id: item.owner,
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
                            source_id: item.owner,
                            sourceitems: source_inventory,
                            target_id: *target_id,
                            targetitems: target_inventory,
                            reqitems: Vec::new(),
                        };

                        send_to_client(*player_id, item_transfer_packet, &clients);
                    } else {
                        info!("Item is not fuel");
                        let packet = ResponsePacket::Error {
                            errmsg: "Item is not fuel".to_string(),
                        };
                        send_to_client(*player_id, packet, &clients);
                        continue;
                    }
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

                    info!("Owner inventory after transfer: {:?}", owner.inventory);
                    info!("Target inventory after transfer: {:?}", target.inventory);

                    let structure_template = templates
                        .obj_templates
                        .get_by_name_template(owner.name.0.clone(), owner.template.0.clone());

                    let req_items = if let Some(req) = structure_template.req {
                        owner.inventory.process_req_items(req)
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

                if target.player_id.0 == *player_id {
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
                    target_id: *target_id,
                    targetitems: target_inventory,
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
    mut query: Query<&mut Inventory>,
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

                // Check if quantity is zero
                if *quantity == 0 {
                    let packet = ResponsePacket::Error {
                        errmsg: "Quantity cannot be zero".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Some(owner_player_id) = ids.get_player(*owner_id) else {
                    error!("Cannot find hero for player {:?}", *player_id);
                    continue;
                };

                if owner_player_id != *player_id {
                    error!("Owner is not owned by player {:?}", *player_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Owner is not owned by player".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                let Some(owner_entity) = entity_map.get_entity(*owner_id) else {
                    error!("Cannot find owner entity for owner {:?}", *owner_id);
                    continue;
                };

                let Ok(mut owner_inventory) = query.get_mut(owner_entity) else {
                    error!("Cannot find owner inventory for {:?}", owner_entity);
                    continue;
                };

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
    mut query: Query<&mut Merchant>, // Renamed parameter to `query` and added type
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

                let Ok(mut merchant) = query.get_mut(merchant_entity) else {
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

                // Set prices and quantity of wanted items
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
                }

                let info_merchant = ResponsePacket::InfoMerchant {
                    source_id: *source_id,
                    inventory: source_inventory,
                    merchant_id: *merchant_id,
                    merchant_inventory: merchant_inventory,
                    merchant_wanted_items: merchant.wanted_items.clone(),
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
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderFollow {
                player_id,
                source_id,
            } => {
                events_to_remove.push(*event_id);

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

                // Add OrderFollow component to source and set hero_entity as target
                for q in &query {
                    if q.id.0 == *source_id {
                        commands.entity(q.entity).insert(Order::Follow {
                            target: hero_entity,
                        });
                    }
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
    templates: Res<Templates>,
    query: Query<CoreQuery>,
    structure_query: Query<
        (&Id, &Position, &Subclass, &Template, &Inventory),
        With<ClassStructure>,
    >,
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

                if !Resource::is_valid_type(res_type.to_string(), *hero.pos, &resources) {
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

                for (id, pos, subclass, template, inventory) in structure_query.iter() {
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
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::StructureList { player_id } => {
                events_to_remove.push(*event_id);
                let structure_list = Structure::available_to_build(
                    *player_id,
                    plans.clone(),
                    &templates.obj_templates,
                );

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
    hero_query: Query<CoreQuery, With<SubclassHero>>,
    structure_query: Query<(&Position, &Subclass), With<ClassStructure>>,
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
                    subclass: Subclass::from_str(&structure_template.subclass),
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
    builder_query: Query<(&Position, &State, Option<&LastCombatTick>)>,
    mut structure_query: Query<(&Name, &Position, &State, &Inventory, &mut Assignments)>,
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

                let Ok((builder_pos, builder_state, last_combat_tick)) =
                    builder_query.get(builder_entity)
                else {
                    error!("Cannot find builder for {:?}", builder_id);
                    continue;
                };

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
                    if !structure_inventory.has_reqs(structure_req.clone()) {
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
    builder_query: Query<(&Position, &State, Option<&LastCombatTick>)>,
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

                let Ok((builder_pos, builder_state, last_combat_tick)) =
                    builder_query.get(builder_entity)
                else {
                    error!("Cannot find builder for {:?}", builder_id);
                    continue;
                };

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
                if !structure_inventory.has_reqs(structure_upgrade_req.clone()) {
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

fn activate_system(
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut map_events: ResMut<MapEvents>,
    entity_map: Res<EntityObjMap>,
    ids: Res<Ids>,
    templates: Res<Templates>,
    mut query: Query<(&PlayerId, &Position, &Template, &State, &mut Inventory)>,
    campfire_query: Query<&Campfire>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Activate {
                player_id,
                structure_id,
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

                let Some(structure_entity) = entity_map.get_entity(*structure_id) else {
                    error!("Cannot find structure entity for {:?}", structure_id);
                    continue;
                };

                let Ok(
                    [(_, hero_pos, _, _, mut hero_inventory), (
                        structure_player_id,
                        structure_pos,
                        structure_template,
                        structure_state,
                        structure_inventory,
                    )],
                ) = query.get_many_mut([hero_entity, structure_entity])
                else {
                    error!(
                        "Cannot find hero or structure for {:?}",
                        [hero_entity, structure_entity]
                    );
                    continue;
                };

                // Check if hero is on the same pos as structure
                if hero_pos != structure_pos {
                    error!("Hero is not nearby the structure {:?}", *structure_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Hero must be nearby the structure to activate it".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
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

                if *structure_state != State::None {
                    error!("Structure is not in None state {:?}", *structure_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Structure cannot be upgraded in this state.".to_string(),
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

                // Player must have an Ignition Tool in their inventory
                let Some(ignition_tool) = hero_inventory.get_by_class(IGNITION_TOOL.to_string())
                else {
                    let packet = ResponsePacket::Error {
                        errmsg: "You must have an Ignition Tool in your inventory".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

                hero_inventory.update_durability(ignition_tool.id, 1);

                let structure_template = templates.obj_templates.get(structure_template.0.clone());

                if structure_template.campfire.unwrap_or(false) {
                    let activate_event = VisibleEvent::ActivateEvent {
                        structure_id: *structure_id,
                    };

                    map_events.new(
                        hero_id,
                        game_tick.0 + 1, // in the future
                        activate_event,
                    );
                }
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn explore_system(
    mut commands: Commands,
    mut events: ResMut<PlayerEvents>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    mut map_events: ResMut<MapEvents>,
    hero_query: Query<CoreQuery, With<SubclassHero>>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Explore { player_id } => {
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
                        errmsg: "The dead cannot explore.".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // If hero is not already exploring
                // TODO expand the action and state checking across all actions
                if *hero.state == State::Exploring {
                    error!("Hero is already exploring {:?}", hero_id);
                    let packet = ResponsePacket::Error {
                        errmsg: "Already exploring".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                }

                // Exploring State Change Event
                commands.trigger(StateChange {
                    entity: hero_entity,
                    new_state: State::Exploring,
                });

                // Insert explore event
                let explore_event = VisibleEvent::ExploreEvent;

                map_events.new(
                    hero.id.0,
                    game_tick.0 + 20, // in the future
                    explore_event,
                );

                let packet = ResponsePacket::Explore { explore_time: 20 };
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

                // Get item from inventory
                let Some((item_to_equip, source_item)) = owner_inventory.get_one_item_by_id(
                    *item_id,
                    ids.new_item_id(),
                    &templates.item_templates,
                ) else {
                    error!("Cannot find item for {:?}", item_id);
                    continue;
                };

                // Check if equipable
                if !item_to_equip.equipable() {
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

                let mut items_updated: Vec<Item> = Vec::new();
                let mut items_removed: Vec<i32> = Vec::new();

                let vision_modifier = owner_effects.get_vision_modifier(&templates);

                // Equip if status is true
                if *status {
                    if item_to_equip.class == TORCH {
                        // Player must have an Ignition Tool in their inventory
                        let Some(ignition_tool) =
                            owner_inventory.get_by_class(IGNITION_TOOL.to_string())
                        else {
                            let packet = ResponsePacket::Error {
                                errmsg: "You must have an Ignition Tool in your inventory"
                                    .to_string(),
                            };
                            send_to_client(*player_id, packet, &clients);
                            continue;
                        };

                        // Update durability of Ignition Tool
                        owner_inventory.update_durability(ignition_tool.id, 1);

                        // Prepend lit to image
                        let new_image = format!("lit{}", item_to_equip.image);
                        owner_inventory.switch_image(item_to_equip.id, new_image);

                        // Equip item slot after image switch
                        items_updated = owner_inventory.equip(item_to_equip.id, item_to_equip.slot);

                        // Set start time for duration of torch
                        owner_inventory.set_start_time(item_to_equip.id, game_tick.0);

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

                        //Add obj update event
                        commands.trigger(UpdateObj {
                            entity: owner_entity,
                            attrs: vec![(VISION.to_string(), viewshed.range.to_string())],
                        });
                    } else {
                        // Equip item slot
                        items_updated = owner_inventory.equip(item_to_equip.id, item_to_equip.slot);
                    }
                } else {
                    if item_to_equip.class == TORCH {
                        // Remove item from inventory
                        owner_inventory.remove_item(item_to_equip.id);
                        items_removed.push(item_to_equip.id);

                        // Recalculate vision
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

                        // Trigger update obj event
                        commands.trigger(UpdateObj {
                            entity: owner_entity,
                            attrs: vec![(VISION.to_string(), viewshed.range.to_string())],
                        });
                    } else {
                        items_updated = owner_inventory.unequip(item_to_equip.id);
                    }
                }

                if item_to_equip.id != source_item.id {
                    items_updated.push(source_item.clone());
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
                    let Some(recipe) = recipes.get_by_name(crafting_event.recipe_name.clone())
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
                    recipes: recipes.get_basic_recipes_packet(),
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
                    let Some(recipe) = recipes.get_by_name(crafting_event.recipe_name.clone())
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
                            let Some(recipe) =
                                recipes.get_by_name(crafting_event.recipe_name.clone())
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
                            work_time = 20;
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

                    let mut produces_list = Vec::new();

                    for produce in produces.iter() {
                        let produce_template =
                            Item::get_template(produce.to_string(), &templates.item_templates);

                        produces_list.push(network::ProducedItem {
                            name: produce_template.name.clone(),
                            image: produce_template.image.clone(),
                            class: produce_template.class.clone(),
                            subclass: produce_template.subclass.clone(),
                        });
                    }

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

fn structure_queue_system(
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut events: ResMut<PlayerEvents>,
    game_events: Res<GameEvents>,
    clients: Res<Clients>,
    recipes: Res<Recipes>,
    templates: Res<Templates>,
    mut active_infos: ResMut<ActiveInfos>,
    villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    mut structure_query: Query<StructureQuery, With<ClassStructure>>,
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

                info!("Adding Order Craft to {:?}", structure_id);
                let Some(recipe) = recipes.get_by_name(recipe_name.clone()) else {
                    error!("Invalid recipe name {:?}", *recipe_name);
                    let packet = ResponsePacket::Error {
                        errmsg: "Invalid recipe".to_string(),
                    };
                    send_to_client(*player_id, packet, &clients);
                    continue;
                };

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

                //TODO consider if checking reqs is required here
                if structure.inventory.has_reqs(recipe.req) {
                    info!("Adding CraftingEntry to {:?} queue", structure_id);

                    let work_entry = WorkEntry {
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
                                let Some(recipe) =
                                    recipes.get_by_name(crafting_event.recipe_name.clone())
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

                let Some(refine_item) = structure.inventory.get_by_id(*refine_item_id) else {
                    error!("Cannot find item for {:?}", *refine_item_id);
                    continue;
                };

                let work_entry = WorkEntry {
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
                            let Some(recipe) =
                                recipes.get_by_name(crafting_event.recipe_name.clone())
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

                structure.work_queue.0.remove(*index as usize);

                let mut work_queue_packet = Vec::new();

                for work_entry in structure.work_queue.0.iter() {
                    let mut work_time = -1;
                    let mut progress = 0;

                    // Get progress of work entry
                    if work_entry.work_type == WorkType::Craft {
                        if let Some(crafting_event) =
                            game_events.get_craft_event(work_entry.worker_id)
                        {
                            let Some(recipe) =
                                recipes.get_by_name(crafting_event.recipe_name.clone())
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

                let work_entry = structure.work_queue.0[*index as usize].clone();

                // Get progress of work entry
                if work_entry.work_type == WorkType::Craft {
                    if let Some(crafting_event) = game_events.get_craft_event(work_entry.worker_id)
                    {
                        let Some(recipe) = recipes.get_by_name(crafting_event.recipe_name.clone())
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

fn order_explore_system(
    mut events: ResMut<PlayerEvents>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut commands: Commands,
    mut map_events: ResMut<MapEvents>,
    clients: Res<Clients>,
    query: Query<ObjQuery>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderExplore {
                player_id,
                villager_id,
            } => {
                events_to_remove.push(*event_id);

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
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::OrderRepair {
                player_id,
                villager_id,
            } => {
                events_to_remove.push(*event_id);

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
            }
            _ => {}
        }
    }

    for event_id in events_to_remove.iter() {
        events.remove(event_id);
    }
}

fn use_item_system(
    mut events: ResMut<PlayerEvents>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    mut map_events: ResMut<MapEvents>,
    mut query: Query<(&PlayerId, &State, &mut Inventory, Option<&LastCombatTick>)>,
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

                // Insert explore event
                let use_item_event = VisibleEvent::UseItemEvent {
                    item_id: *item_id,
                    item_owner_id: *obj_id,
                };

                map_events.new(*obj_id, game_tick.0 + 1, use_item_event);
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
    query: Query<(&State, Option<&LastCombatTick>)>,
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

                let Ok((hero_state, last_combat_tick)) = query.get(hero_entity) else {
                    error!("Cannot find hero state for {:?}", hero_entity);
                    continue;
                };

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

fn hire_system(
    commands: Commands,
    game_tick: Res<GameTick>,
    mut events: ResMut<PlayerEvents>,
    ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    clients: Res<Clients>,
    map_events: ResMut<MapEvents>,
    pos_query: Query<&mut Position>,
    merchant_query: Query<&Transport, With<Merchant>>,
    player_query: Query<&mut PlayerId>,
) {
    let mut events_to_remove: Vec<i32> = Vec::new();

    for (event_id, event) in events.iter() {
        match event {
            PlayerEvent::Hire {
                player_id,
                merchant_id,
                target_id,
            } => {
                events_to_remove.push(*event_id);

                // Adding AI to villager
                /*let find_move_to_and_drink = Steps::build()
                    .label("FindMoveToAndDrink")
                    .step(FindDrink)
                    .step(MoveTo)
                    .step(TransferDrink)
                    .step(Drink { until: 70.0 });

                let find_move_to_and_eat = Steps::build()
                    .label("FindMoveToAndEat")
                    .step(FindFood)
                    .step(MoveToFoodSource)
                    .step(TransferFood)
                    .step(Eat);

                let find_move_to_and_sleep = Steps::build()
                    .label("FindMoveToAndSleep")
                    .step(FindShelter { trigger_event: "Sleep".to_string() })
                    .step(MoveToShelterAction)
                    .step(Sleep);

                let find_move_to_and_shelter = Steps::build()
                    .label("FindMoveToAndShelter")
                    .step(FindShelter { trigger_event: "Shelter".to_string() })
                    .step(MoveToShelterAction)
                    .step(Idle {
                        start_time: 0,
                        duration: 100,
                    });

                    commands.entity(target_entity).insert((
                        Thirst::new(80.0, 0.025), //0.1 before
                        Hunger::new(0.0, 0.025),
                        Tired::new(0.0, 0.025),
                        Heat::new(50.0),
                        Morale::new(50.0),
                        Thinker::build()
                            .label("Villager")
                            .picker(Highest)
                            .when(EnemyDistanceScorer, Flee)
                            .when(ThirstyScorer, find_move_to_and_drink)
                            .when(HungryScorer, find_move_to_and_eat)
                            .when(DrowsyScorer, find_move_to_and_sleep)
                            .when(ExhaustedScorer, Sleep)
                            .when(HeatScorer, find_move_to_and_shelter)
                            .when(CapacityScorer, UnloadItems)
                            .when(
                                IdleScorer,
                                Idle {
                                    start_time: 0,
                                    duration: 100,
                                },
                            )
                            .when(GoodMorale, ProcessOrder),
                    ));*/
            }
            _ => {}
        }
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
    mut query: Query<(&mut Position, &mut Inventory)>,
    mut merchant_query: Query<&mut Merchant>,
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
            Some(&(WARRIOR_BRACE_DURATION_TICKS, WARRIOR_BRACE_AMPLIFIER, 1))
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
    fn mage_ward_uses_timed_weak_sanctuary() {
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
            Effect::WeakSanctuary,
            MAGE_WARD_DURATION_TICKS,
            MAGE_WARD_AMPLIFIER,
        );

        assert_eq!(
            effects.0.get(&Effect::WeakSanctuary),
            Some(&(MAGE_WARD_DURATION_TICKS, MAGE_WARD_AMPLIFIER, 1))
        );
        assert!(map_events.values().any(|event| {
            event.obj_id == 9
                && event.run_tick == 200 + MAGE_WARD_DURATION_TICKS
                && matches!(
                    &event.event_type,
                    VisibleEvent::EffectExpiredEvent { effect }
                        if *effect == Effect::WeakSanctuary
                )
        }));
    }
}
