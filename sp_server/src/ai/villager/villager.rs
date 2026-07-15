use bevy::{
    ecs::query::{Or, QueryData},
    ecs::system::SystemParam,
    prelude::*,
};
use big_brain::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::{
    ai_logging::entity_display,
    combat::{AttackType, Combat, CombatQuery},
    common::{
        Dehydrated, Destination, Drink, Eat, Exhausted, Heat, Hunger, Idle, MoveTo, Sleep,
        Starving, Thirst, Tired,
    },
    constants::*,
    effect::{Effect, Effects},
    event::{
        DrinkEventCompleted, EatEventCompleted, EventCompleted, EventExecuting,
        EventExecutingState, FindEventCompleted, GameEvent, GameEventType, GameEvents, MapEvents,
        SleepEventCompleted, VisibleEvent,
    },
    experiment::{self, Experiment, Experiments},
    game::{Clients, GameTick, ObjQuery, ObjQueryMutPlayerTemplate},
    ids::{EntityObjMap, Ids},
    item::{self, AttrKey, Inventory, Item, ItemLocation},
    map::{Map, MapPos, TileType},
    network::{send_to_client, ResponsePacket},
    obj::{Name, StartBuild, State, *},
    player::{ActiveInfoType, ActiveInfos},
    recipe::Recipes,
    resource::{Resource, Resources},
    safe_logout::{
        is_owner_offline_protected, object_belongs_to_protected_run, PlayerWorldPresenceState,
    },
    templates::Templates,
    villager_debug, villager_error, villager_info, villager_trace,
    villager_util::VillagerUtil,
    villager_warn, with_span, AppState,
};

#[derive(SystemParam)]
pub struct VillagerProtection<'w, 's> {
    presence: Option<Res<'w, PlayerWorldPresenceState>>,
    owners: Query<'w, 's, &'static PlayerId>,
}

impl VillagerProtection<'_, '_> {
    fn owner_is_protected(&self, owner: &PlayerId) -> bool {
        self.presence
            .as_deref()
            .map(|presence| is_owner_offline_protected(owner, presence))
            .unwrap_or(false)
    }

    fn is_protected(&self, actor: Entity) -> bool {
        self.owners
            .get(actor)
            .map(|owner| self.owner_is_protected(owner))
            .unwrap_or(false)
    }

    fn object_is_protected(&self, object_id: i32, ids: &Ids) -> bool {
        self.presence
            .as_deref()
            .map(|presence| object_belongs_to_protected_run(object_id, ids, presence))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct EnemyDistanceScorer;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct ArmedRetaliationScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct FightBack;

const RETALIATION_MEMORY_TICKS: i32 = 5 * TICKS_PER_SEC;
const FLEE_SEARCH_RADIUS: u32 = 6;
const FLEE_THREAT_SCAN_RADIUS: u32 = FLEE_SEARCH_RADIUS + 2;
const FLEE_HERO_FALLBACK_RANGE: u32 = 4;
const FLEE_SAFE_BONUS: i32 = 100_000;
const FLEE_WALL_BONUS: i32 = 50_000;
const FLEE_SHELTER_BONUS: i32 = 40_000;

#[derive(Debug, Clone)]
struct FleeThreat {
    pos: Position,
    player_id: i32,
    blocking_list: Vec<Blocker>,
}

#[derive(Debug, Clone, Copy)]
struct FleeDestinationChoice {
    pos: Position,
    score: i32,
    safe: bool,
    fortified: bool,
}

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct IdleScorer;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct ThirstyScorer;

// Hunger
#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct HungryScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetFleeDestination;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Flee;

#[derive(Debug, Clone, Component)]
pub struct ShelterAvailable;

#[derive(Debug, Clone, Component)]
pub struct ShelterUnavailable;

fn actor_is_combat_locked(
    actor: Entity,
    game_tick: i32,
    last_combat_tick_query: &Query<&LastCombatTick>,
) -> bool {
    last_combat_tick_query
        .get(actor)
        .map(|last_combat_tick| is_combat_locked(game_tick, last_combat_tick))
        .unwrap_or(false)
}

fn order_is_combat_locked(order: &Order) -> bool {
    matches!(
        order,
        Order::Gather { .. } | Order::Build | Order::WorkQueue
    )
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetOrderDestination;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetStorageDestination;

#[derive(Debug, Clone, Component)]
pub struct ToolFetchTarget {
    pub storage_id: i32,
    pub item_id: i32,
    pub res_type: String,
    pub required_attr: AttrKey,
}

#[derive(Debug, Clone, Component)]
pub struct BlockedWork {
    pub reason: String,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct MaybeTransferGatherTool;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct FindDrink;

#[derive(Debug, Clone, Component)]
pub struct NoDrinks {
    pub at_tick: i32,
}

// A villager with no drink item of its own is heading to / drinking at a natural
// water source (a revealed spring near base). Set by find_drink_system's fallback,
// consumed by drink_action_system. Lets settlement villagers stay hydrated without
// being handed waterskins.
#[derive(Debug, Clone, Component)]
pub struct DrinkingFromWater;

// How far a thirsty villager will look for a revealed spring to drink at.
const VILLAGER_WATER_RANGE: i32 = 15;

// Nearest natural water the villager can drink at, within `range`: a passable tile
// that either holds a revealed spring (stand on it, like the hero) or sits beside a
// river (drink from the bank). Rivers are always visible, so this gives villagers a
// reliable water source even before the hero has prospected a spring nearby.
fn nearest_water(pos: &Position, resources: &Resources, map: &Map, range: i32) -> Option<Position> {
    for r in 0..=range {
        for (x, y) in Map::ring((pos.x, pos.y), r) {
            if !Map::is_valid_pos((x, y)) {
                continue;
            }
            let tile = Position { x, y };
            if !Resource::get_by_type(tile, SPRING_WATER.to_string(), resources, true).is_empty() {
                return Some(tile);
            }
            if Map::is_passable(x, y, map)
                && Map::are_tile_types_nearby(tile, vec![TileType::River], map)
            {
                return Some(tile);
            }
        }
    }
    None
}

#[derive(Debug, Clone, Component)]
pub struct NoFood {
    pub at_tick: i32,
}

#[derive(Debug, Clone, Component)]
pub struct NoShelter {
    pub _at_tick: i32,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct TransferDrink;

#[derive(Debug, Clone, Component)]
pub struct Storage {
    pub id: i32,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct FindFood;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct TransferFood;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct FindShelterScorer;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct ShelterDistanceScorer;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct NearShelterScorer;

// Sleep
#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct DrowsyScorer;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct ExhaustedScorer;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct HeatScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct FindShelter {
    pub trigger_event: String,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct ProcessOrder;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct GoodMorale;

#[derive(Component, Debug)]
pub struct Morale {
    pub morale: f32,
    pub rough_sleep_penalty: f32,
}

impl Morale {
    pub fn new(morale: f32) -> Self {
        Self {
            morale,
            rough_sleep_penalty: 0.0,
        }
    }

    pub fn add_rough_sleep_penalty(&mut self, penalty: f32) {
        self.rough_sleep_penalty = (self.rough_sleep_penalty + penalty).clamp(0.0, 100.0);
        self.morale = (self.morale - penalty).clamp(0.0, 100.0);
    }
}

const ROUGH_SLEEP_MORALE_PENALTY: f32 = 5.0;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct CapacityScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct UnloadItems;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct StructureCapacityScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct LoadItems;

#[derive(Debug, Clone, Component)]
pub struct Dialogue {
    pub trigger_event: String,
    pub at_tick: i32,
}

#[derive(Debug, Clone, Component)]
pub struct TargetItem(pub Item);

#[derive(Debug, Clone, Component)]
pub struct Target(pub i32);

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct VillagerQuery {
    id: &'static Id,
    player_id: &'static PlayerId,
    pos: &'static Position,
    class: &'static Class,
    state: &'static mut State,
    inventory: &'static mut Inventory,
    active_task: &'static mut ActiveTask,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct VillagerWithOrderQuery {
    id: &'static Id,
    player_id: &'static PlayerId,
    pos: &'static Position,
    class: &'static Class,
    order: &'static Order,
    template: &'static Template,
    inventory: &'static Inventory,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct VillagerBaseQuery {
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub state: &'static State,
    pub active_task: &'static ActiveTask,
}

#[derive(QueryData)]
#[query_data(derive(Debug))]
pub struct ActiveTaskActionQuery {
    actor: &'static Actor,
    state: &'static ActionState,
    find_drink: Option<&'static FindDrink>,
    drink: Option<&'static Drink>,
    transfer_drink: Option<&'static TransferDrink>,
    find_food: Option<&'static FindFood>,
    transfer_food: Option<&'static TransferFood>,
    eat: Option<&'static Eat>,
    move_to: Option<&'static MoveTo>,
    sleep: Option<&'static Sleep>,
    find_shelter: Option<&'static FindShelter>,
    set_order_destination: Option<&'static SetOrderDestination>,
    maybe_transfer_gather_tool: Option<&'static MaybeTransferGatherTool>,
    set_storage_destination: Option<&'static SetStorageDestination>,
    unload_items: Option<&'static UnloadItems>,
    set_flee_destination: Option<&'static SetFleeDestination>,
    idle: Option<&'static Idle>,
    process_order: Option<&'static ProcessOrder>,
    flee: Option<&'static Flee>,
    fight_back: Option<&'static FightBack>,
}

fn order_activity(state: &State, order: &Order) -> ActiveTask {
    match order {
        Order::Follow { .. } => ActiveTask::Following,
        Order::Gather { res_type, .. } => match res_type.as_str() {
            ORE => ActiveTask::Mining,
            LOG => ActiveTask::Woodcutting,
            STONE => ActiveTask::Stonecutting,
            _ => ActiveTask::Gathering,
        },
        Order::Build => ActiveTask::Building,
        Order::WorkQueue => {
            if *state == State::Crafting {
                ActiveTask::Crafting
            } else if *state == State::Refining {
                ActiveTask::Refining
            } else if *state == State::Experimenting {
                ActiveTask::Experimenting
            } else {
                ActiveTask::Operating
            }
        }
        Order::Explore => ActiveTask::Exploring,
        Order::Plant => ActiveTask::Planting,
        Order::Tend => ActiveTask::Tending,
        _ => ActiveTask::Unknown,
    }
}

fn gather_active_task_for_display(
    state: &State,
    res_type: &str,
    inventory: &Inventory,
    blocked_work: Option<&BlockedWork>,
    tool_fetch_target: Option<&ToolFetchTarget>,
) -> Option<ActiveTask> {
    let active_task = ActiveTask::get_activity_from_res_type(res_type.to_string());
    let Some(required_attr) = item::required_tool_attr_for_res_type(res_type) else {
        return Some(active_task);
    };

    if *state == State::Gathering || inventory.has_equipped_tool_for_attr(&required_attr) {
        return Some(active_task);
    }

    if tool_fetch_target
        .map(|target| target.res_type.as_str() == res_type && target.required_attr == required_attr)
        .unwrap_or(false)
    {
        return Some(ActiveTask::MovingToGatherPos);
    }

    if blocked_work.is_some() {
        return Some(ActiveTask::Unknown);
    }

    None
}

fn order_activity_for_display(
    state: &State,
    order: &Order,
    inventory: &Inventory,
    blocked_work: Option<&BlockedWork>,
    tool_fetch_target: Option<&ToolFetchTarget>,
) -> Option<ActiveTask> {
    match order {
        Order::Gather { res_type, .. } => gather_active_task_for_display(
            state,
            res_type,
            inventory,
            blocked_work,
            tool_fetch_target,
        ),
        _ => Some(order_activity(state, order)),
    }
}

fn movement_activity_from_previous(previous: ActiveTask) -> Option<ActiveTask> {
    match previous {
        ActiveTask::Fleeing
        | ActiveTask::FindingShelter
        | ActiveTask::GettingDrink
        | ActiveTask::GettingFood
        | ActiveTask::Following
        | ActiveTask::Mining
        | ActiveTask::Woodcutting
        | ActiveTask::Stonecutting
        | ActiveTask::Gathering
        | ActiveTask::Building
        | ActiveTask::Operating
        | ActiveTask::Refining
        | ActiveTask::Crafting
        | ActiveTask::Experimenting
        | ActiveTask::Exploring
        | ActiveTask::Planting
        | ActiveTask::Tending
        | ActiveTask::Harvesting
        | ActiveTask::Repairing
        | ActiveTask::Unloading => Some(previous),
        _ => None,
    }
}

fn visible_activity_for_action(
    action: &ActiveTaskActionQueryItem,
    previous: ActiveTask,
    order_context: Option<(
        &State,
        &Order,
        &Inventory,
        Option<&BlockedWork>,
        Option<&ToolFetchTarget>,
    )>,
) -> Option<ActiveTask> {
    if action.fight_back.is_some() {
        Some(ActiveTask::FightingBack)
    } else if action.flee.is_some() || action.set_flee_destination.is_some() {
        Some(ActiveTask::Fleeing)
    } else if action.move_to.is_some() {
        movement_activity_from_previous(previous)
    } else if action.find_drink.is_some()
        || action.drink.is_some()
        || action.transfer_drink.is_some()
    {
        Some(ActiveTask::GettingDrink)
    } else if action.find_food.is_some() || action.eat.is_some() || action.transfer_food.is_some() {
        Some(ActiveTask::GettingFood)
    } else if action.sleep.is_some() {
        Some(ActiveTask::Sleeping)
    } else if action.find_shelter.is_some() {
        Some(ActiveTask::FindingShelter)
    } else if action.set_order_destination.is_some() {
        match order_context {
            Some((state, order, inventory, blocked_work, tool_fetch_target)) => {
                order_activity_for_display(state, order, inventory, blocked_work, tool_fetch_target)
            }
            None => Some(ActiveTask::Operating),
        }
    } else if action.maybe_transfer_gather_tool.is_some() {
        match order_context {
            Some((state, order, inventory, blocked_work, tool_fetch_target)) => {
                order_activity_for_display(state, order, inventory, blocked_work, tool_fetch_target)
            }
            None => Some(ActiveTask::Operating),
        }
    } else if action.set_storage_destination.is_some() {
        Some(ActiveTask::Unloading)
    } else if action.unload_items.is_some() {
        Some(ActiveTask::Unloading)
    } else if action.process_order.is_some() {
        match order_context {
            Some((state, order, inventory, blocked_work, tool_fetch_target)) => {
                order_activity_for_display(state, order, inventory, blocked_work, tool_fetch_target)
            }
            None => Some(ActiveTask::Operating),
        }
    } else if action.idle.is_some() {
        Some(ActiveTask::Idle)
    } else {
        None
    }
}

fn active_task_priority(task: &ActiveTask) -> i32 {
    match task {
        ActiveTask::Fleeing | ActiveTask::FightingBack => 100,
        ActiveTask::GettingDrink
        | ActiveTask::GettingFood
        | ActiveTask::FindingShelter
        | ActiveTask::Eating
        | ActiveTask::Drinking
        | ActiveTask::Sleeping => 80,
        ActiveTask::Following
        | ActiveTask::Building
        | ActiveTask::Gathering
        | ActiveTask::Operating
        | ActiveTask::Mining
        | ActiveTask::Hunting
        | ActiveTask::Woodcutting
        | ActiveTask::Stonecutting
        | ActiveTask::Refining
        | ActiveTask::Crafting
        | ActiveTask::Experimenting
        | ActiveTask::Exploring
        | ActiveTask::Planting
        | ActiveTask::Tending
        | ActiveTask::Harvesting
        | ActiveTask::Repairing
        | ActiveTask::Unloading => 60,
        ActiveTask::MovingToGatherPos
        | ActiveTask::MovingToOperatePos
        | ActiveTask::MovingToRefinePos
        | ActiveTask::MovingToCraftPos
        | ActiveTask::MovingToExperimentPos
        | ActiveTask::MovingToExplorePos
        | ActiveTask::MovingToFoodPos
        | ActiveTask::MovingToDrinkPos
        | ActiveTask::MovingToShelterPos => 40,
        ActiveTask::Idle => 10,
        ActiveTask::None | ActiveTask::Unknown => 0,
    }
}

fn active_task_tiebreaker(task: &ActiveTask) -> i32 {
    match task {
        ActiveTask::Fleeing => 35,
        ActiveTask::FightingBack => 34,
        ActiveTask::Mining => 33,
        ActiveTask::Woodcutting => 32,
        ActiveTask::Stonecutting => 31,
        ActiveTask::Gathering => 30,
        ActiveTask::Unloading => 29,
        ActiveTask::Building => 28,
        ActiveTask::Operating => 27,
        ActiveTask::Refining => 26,
        ActiveTask::Crafting => 25,
        ActiveTask::Experimenting => 24,
        ActiveTask::Exploring => 23,
        ActiveTask::Planting => 22,
        ActiveTask::Tending => 21,
        ActiveTask::Harvesting => 20,
        ActiveTask::Repairing => 19,
        ActiveTask::Following => 18,
        ActiveTask::GettingDrink => 17,
        ActiveTask::GettingFood => 16,
        ActiveTask::FindingShelter => 15,
        ActiveTask::Drinking => 14,
        ActiveTask::Eating => 13,
        ActiveTask::Sleeping => 12,
        ActiveTask::Hunting => 11,
        ActiveTask::MovingToGatherPos => 10,
        ActiveTask::MovingToOperatePos => 9,
        ActiveTask::MovingToRefinePos => 8,
        ActiveTask::MovingToCraftPos => 7,
        ActiveTask::MovingToExperimentPos => 6,
        ActiveTask::MovingToExplorePos => 5,
        ActiveTask::MovingToFoodPos => 4,
        ActiveTask::MovingToDrinkPos => 3,
        ActiveTask::MovingToShelterPos => 2,
        ActiveTask::Idle => 1,
        ActiveTask::None | ActiveTask::Unknown => 0,
    }
}

pub struct VillagerPlugin;

impl Plugin for VillagerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                fight_back_system.in_set(BigBrainSet::Actions),
                move_to_system.in_set(BigBrainSet::Actions),
                find_drink_system.in_set(BigBrainSet::Actions),
                transfer_drink_system.in_set(BigBrainSet::Actions),
                drink_action_system.in_set(BigBrainSet::Actions),
                find_food_system.in_set(BigBrainSet::Actions),
                transfer_food_system.in_set(BigBrainSet::Actions),
                eat_action_system.in_set(BigBrainSet::Actions),
                sleep_action_system.in_set(BigBrainSet::Actions),
                find_shelter_system.in_set(BigBrainSet::Actions),
                set_order_destination_system.in_set(BigBrainSet::Actions),
                maybe_transfer_gather_tool_system.in_set(BigBrainSet::Actions),
                process_order_system.in_set(BigBrainSet::Actions),
                set_flee_destination_system.in_set(BigBrainSet::Actions),
                set_storage_destination_system.in_set(BigBrainSet::Actions),
                load_items_system.in_set(BigBrainSet::Actions),
                unload_items_system.in_set(BigBrainSet::Actions),
                idle_action_system.in_set(BigBrainSet::Actions),
            )
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                armed_retaliation_scorer_system.in_set(BigBrainSet::Scorers),
                enemy_distance_scorer_system.in_set(BigBrainSet::Scorers),
                idle_scorer_system.in_set(BigBrainSet::Scorers),
                thirsty_scorer_system.in_set(BigBrainSet::Scorers),
                hungry_scorer_system.in_set(BigBrainSet::Scorers),
                drowsy_scorer_system.in_set(BigBrainSet::Scorers),
                exhausted_scorer_system.in_set(BigBrainSet::Scorers),
            )
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                heat_scorer_system.in_set(BigBrainSet::Scorers),
                morale_scorer_system.in_set(BigBrainSet::Scorers),
                structure_capacity_scorer_system.in_set(BigBrainSet::Scorers),
                capacity_scorer_system.in_set(BigBrainSet::Scorers),
                vital_dialogue_system,
                remove_no_drinks_system,
                remove_no_food_system,
                active_task_system.after(BigBrainSet::Actions),
                activity_update_system.after(active_task_system),
            )
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(Update, clear_event_executing.after(BigBrainSet::Actions));
    }
}

pub fn armed_retaliation_scorer_system(
    game_tick: Res<GameTick>,
    ids: Option<Res<Ids>>,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<ArmedRetaliationScorer>>,
    villager_query: Query<
        (
            &PlayerId,
            &Position,
            &Inventory,
            Option<&LastAttacker>,
            &State,
        ),
        With<SubclassVillager>,
    >,
    target_query: Query<(&Id, &PlayerId, &Position, &State)>,
) {
    for (Actor(actor), mut score, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        score.set(0.0);

        let Ok((villager_player_id, villager_pos, inventory, last_attacker_opt, villager_state)) =
            villager_query.get(*actor)
        else {
            span.span().in_scope(|| {
                villager_error!(
                    *actor,
                    obj_id,
                    None,
                    "Cannot find villager for armed retaliation scorer"
                );
            });
            continue;
        };

        if Obj::is_dead(villager_state) {
            continue;
        }

        let Some(last_attacker) = last_attacker_opt else {
            continue;
        };

        if game_tick.0.saturating_sub(last_attacker.tick) > RETALIATION_MEMORY_TICKS {
            continue;
        }

        if inventory.get_equipped_weapons().is_empty() {
            continue;
        }

        let Some(attacker_entity) = entity_map.get_entity(last_attacker.id) else {
            span.span().in_scope(|| {
                villager_warn!(
                    *actor,
                    obj_id,
                    None,
                    "Cannot find last attacker entity id={}",
                    last_attacker.id
                );
            });
            continue;
        };

        let Ok((attacker_id, attacker_player_id, attacker_pos, attacker_state)) =
            target_query.get(attacker_entity)
        else {
            span.span().in_scope(|| {
                villager_warn!(
                    *actor,
                    obj_id,
                    None,
                    "Cannot query last attacker entity={:?}",
                    attacker_entity
                );
            });
            continue;
        };

        if attacker_player_id.0 == villager_player_id.0
            || attacker_player_id.0 == MERCHANT_PLAYER_ID
            || protection.owner_is_protected(attacker_player_id)
            || ids
                .as_deref()
                .map(|ids| protection.object_is_protected(attacker_id.0, ids))
                .unwrap_or(false)
            || Obj::is_dead(attacker_state)
            || Map::dist(*villager_pos, *attacker_pos) > 1
        {
            continue;
        }

        span.span().in_scope(|| {
            villager_debug!(
                *actor,
                obj_id,
                None,
                "Armed retaliation ready against obj_id={}",
                last_attacker.id
            );
        });
        score.set(1.0);
    }
}

pub fn enemy_distance_scorer_system(
    ids: Res<Ids>,
    map: Res<Map>,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    hero_query: Query<ObjQuery, With<SubclassHero>>,
    obj_query: Query<
        (
            Entity,
            &Id,
            &PlayerId,
            &Position,
            Option<&Template>,
            &Class,
            &State,
            Option<&Effects>,
        ),
        Without<SubclassHero>,
    >,
    blocking_query: Query<BaseQuery>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<EnemyDistanceScorer>>,
) {
    for (Actor(actor), mut score, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        if let Ok((
            _villager_entity,
            _villager_id,
            villager_player_id,
            villager_pos,
            _villager_template,
            _villager_class,
            _villager_state,
            villager_effects,
        )) = obj_query.get(*actor)
        {
            // Merchant-owned and NPC-owned villagers have no hero — skip silently
            if villager_player_id.0 >= MAX_PLAYER_ID {
                score.set(0.0);
                continue;
            }

            let Some(hero_id) = ids.get_hero(villager_player_id.0) else {
                span.span().in_scope(|| {
                    villager_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find hero for player={}",
                        villager_player_id.0
                    );
                });
                continue;
            };

            let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                span.span().in_scope(|| {
                    villager_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find hero entity for hero_id={}",
                        hero_id
                    );
                });
                continue;
            };

            let Ok(_hero) = hero_query.get(hero_entity) else {
                span.span().in_scope(|| {
                    villager_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find hero for entity={:?}",
                        hero_entity
                    );
                });
                continue;
            };

            let mut dangerous_nearby_enemy = false;

            for (
                enemy_entity,
                enemy_id,
                enemy_player_id,
                enemy_pos,
                enemy_template,
                enemy_class,
                enemy_state,
                _enemy_effects,
            ) in obj_query.iter()
            {
                if enemy_entity == *actor {
                    continue;
                }

                if *enemy_state == State::Dead {
                    continue;
                }

                if enemy_class.is_poi() {
                    continue;
                }

                if enemy_player_id.0 == villager_player_id.0
                    || enemy_player_id.0 == MERCHANT_PLAYER_ID
                {
                    continue;
                }

                let blocking_list =
                    Obj::blocking_list_basequery(enemy_player_id.0, &blocking_query);

                if enemy_threatens_villager(
                    *villager_pos,
                    villager_effects,
                    *enemy_pos,
                    enemy_player_id.0,
                    enemy_template,
                    &map,
                    &blocking_list,
                ) {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            obj_id,
                            None,
                            "Flee threat from obj_id={}",
                            enemy_id.0
                        );
                    });
                    dangerous_nearby_enemy = true;
                    break;
                }
            }

            if dangerous_nearby_enemy {
                score.set(1.0);
            } else {
                score.set(0.0);
            }
        }
    }
}

fn enemy_threatens_villager(
    villager_pos: Position,
    villager_effects: Option<&Effects>,
    enemy_pos: Position,
    enemy_player_id: i32,
    _enemy_template: Option<&Template>,
    map: &Map,
    blocking_list: &[Blocker],
) -> bool {
    let distance = Map::distance((villager_pos.x, villager_pos.y), (enemy_pos.x, enemy_pos.y));
    if distance > 2 {
        return false;
    }

    if villager_is_fortified(villager_effects) {
        return false;
    }

    Map::find_fast_path(
        enemy_pos,
        villager_pos,
        map,
        enemy_player_id,
        blocking_list.to_vec(),
        true,
        false,
        false,
        true,
        false,
    )
    .is_some()
}

fn villager_is_fortified(villager_effects: Option<&Effects>) -> bool {
    villager_effects
        .map(|effects| effects.has(Effect::Fortified))
        .unwrap_or(false)
}

pub fn idle_scorer_system(
    templates: Res<Templates>,
    protection: VillagerProtection,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<IdleScorer>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        score.set(0.1);
    }
}

pub fn thirsty_scorer_system(
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    thirsts: Query<&Thirst>,
    no_drinks: Query<&NoDrinks>,
    event_query: Query<(&EventExecuting, &ActiveTask)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<ThirstyScorer>>,
) {
    for (Actor(actor), mut score, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        if let Ok(thirst) = thirsts.get(*actor) {
            let (event_executing, active_task) = event_query
                .get(*actor)
                .expect("Missing event executing or active task component");

            // Skip calculating the score if the event is completed,
            // as this will cause a cancellation of the action due to transition state
            if event_executing.state == EventExecutingState::Completed {
                continue;
            }

            // Calculate thirst score
            let mut thirst_score;

            // Do not understand why we need this check
            if thirst.thirst >= DEHYDRATED_SCORE {
                thirst_score = EMERGENCY_SCORE;
            } else {
                let mut no_drink_mod = 0.0;

                // If no drinks, subtract 10 f
                if let Ok(_no_drinks) = no_drinks.get(*actor) {
                    no_drink_mod = -50.0;
                }

                if *active_task == ActiveTask::GettingDrink || *active_task == ActiveTask::Drinking
                {
                    thirst_score = thirst.thirst * 1.50 + no_drink_mod;

                    if thirst_score >= MAX_ROUTINE_SCORE {
                        thirst_score = MAX_ROUTINE_SCORE;
                    }
                } else {
                    thirst_score = thirst.thirst + no_drink_mod;

                    if thirst_score >= MAX_ROUTINE_SCORE {
                        thirst_score = MAX_ROUTINE_SCORE;
                    }
                }
            }

            let mut final_score = thirst_score / 100.0;

            if final_score > 1.0 {
                final_score = 1.0;
            } else if final_score < 0.0 {
                final_score = 0.0;
            }
            span.span().in_scope(|| {
                villager_debug!(
                    *actor,
                    obj_id,
                    None,
                    "Thirst score={:.2} state={:?}",
                    final_score,
                    event_executing.state
                );
            });
            score.set(final_score);
        }
    }
}

pub fn hungry_scorer_system(
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    hungers: Query<&Hunger>,
    no_food: Query<&NoFood>,
    event_query: Query<(&EventExecuting, &ActiveTask)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<HungryScorer>>,
) {
    for (Actor(actor), mut score, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        if let Ok(hunger) = hungers.get(*actor) {
            let (event_executing, active_task) = event_query
                .get(*actor)
                .expect("Missing event executing or active task component");

            // Skip calculating the score if the event is completed,
            // as this will cause a cancellation of the action due to transition state
            if event_executing.state == EventExecutingState::Completed {
                continue;
            }

            let mut hunger_score;

            if hunger.hunger >= STARVING_SCORE {
                hunger_score = EMERGENCY_SCORE;
            } else {
                let mut no_food_mod = 0.0;

                // If no food, subtract 10 f
                if let Ok(_no_food) = no_food.get(*actor) {
                    no_food_mod = -50.0;
                }

                if *active_task == ActiveTask::GettingFood || *active_task == ActiveTask::Eating {
                    hunger_score = hunger.hunger * 1.50 + no_food_mod;

                    if hunger_score >= MAX_ROUTINE_SCORE {
                        hunger_score = MAX_ROUTINE_SCORE;
                    }
                } else {
                    hunger_score = hunger.hunger + no_food_mod;

                    if hunger_score >= MAX_ROUTINE_SCORE {
                        hunger_score = MAX_ROUTINE_SCORE;
                    }
                }
            }

            let mut final_score = hunger_score / 100.0;

            if final_score > 1.0 {
                final_score = 1.0;
            } else if final_score < 0.0 {
                final_score = 0.0;
            }

            span.span().in_scope(|| {
                villager_debug!(
                    *actor,
                    obj_id,
                    None,
                    "Hunger score={:.2} state={:?}",
                    final_score,
                    event_executing.state
                );
            });
            score.set(final_score);
        }
    }
}

pub fn drowsy_scorer_system(
    protection: VillagerProtection,
    tired_query: Query<&Tired>,
    no_shelter: Query<&NoShelter>,
    event_query: Query<(&EventExecuting, &ActiveTask)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<DrowsyScorer>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        if let Ok(tired) = tired_query.get(*actor) {
            let (event_executing, active_task) = event_query
                .get(*actor)
                .expect("Missing event executing or active task component");

            // Skip calculating the score if the event is completed,
            // as this will cause a cancellation of the action due to transition state
            if event_executing.state == EventExecutingState::Completed {
                continue;
            }

            let mut tired_score;
            let mut no_shelter_mod = 0.0;

            // If no shelter, subtract 10 f
            if let Ok(_no_shelter) = no_shelter.get(*actor) {
                no_shelter_mod = -50.0;
            }

            if *active_task == ActiveTask::FindingShelter {
                tired_score = tired.tired * 1.50 + no_shelter_mod;

                if tired_score >= MAX_ROUTINE_SCORE {
                    tired_score = MAX_ROUTINE_SCORE;
                }
            } else {
                tired_score = tired.tired;

                if tired_score >= MAX_ROUTINE_SCORE {
                    tired_score = MAX_ROUTINE_SCORE;
                }
            }

            let mut final_score = tired_score / 100.0;

            if final_score > 1.0 {
                final_score = 1.0;
            } else if final_score < 0.0 {
                final_score = 0.0;
            }

            score.set(final_score);
        }
    }
}

pub fn exhausted_scorer_system(
    protection: VillagerProtection,
    tired_query: Query<&Tired, Without<EventExecuting>>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<ExhaustedScorer>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        if let Ok(tired) = tired_query.get(*actor) {
            let exhausted_score;

            if tired.tired >= EXHAUSTED_SCORE {
                exhausted_score = EMERGENCY_SCORE;
            } else {
                exhausted_score = 0.0;
            }
            score.set(exhausted_score / 100.0);
        }
    }
}

pub fn heat_scorer_system(
    protection: VillagerProtection,
    heat_query: Query<&Heat>,
    no_shelter: Query<&NoShelter>,
    active_task: Query<&ActiveTask>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<HeatScorer>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        if let Ok(heat) = heat_query.get(*actor) {
            /*let Ok(villager_attrs) = villager_attrs.get(*actor) else {
                error!("No villager attrs component for {:?}", *actor);
                continue;
            };
            let mut heat_score;

            if heat.heat >= OVERHEATED {
                heat_score = EMERGENCY_SCORE;
            } else if heat.heat <= HYPOTHERMIC {
                heat_score = EMERGENCY_SCORE;
            } else {
                let normalized_heat = heat.heat.abs();

                let mut no_shelter_mod = 0.0;

                if let Ok(_no_shelter) = no_shelter.get(*actor) {
                    no_shelter_mod = -50.0;
                }

                if villager_attrs.activity == villager::Activity::FindingShelter {
                    heat_score = normalized_heat * 1.50 + no_shelter_mod;

                    if heat_score >= MAX_ROUTINE_SCORE {
                        heat_score = MAX_ROUTINE_SCORE;
                    }
                } else {
                    heat_score = normalized_heat;

                    if heat_score >= MAX_ROUTINE_SCORE {
                        heat_score = MAX_ROUTINE_SCORE;
                    }
                }
            }

            score.set(heat_score / 100.0);*/
            score.set(0.0);
        }
    }
}

pub fn morale_scorer_system(
    protection: VillagerProtection,
    morale_query: Query<(&Morale, Option<&Order>)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<GoodMorale>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        if let Ok((_morale, order)) = morale_query.get(*actor) {
            if matches!(order, None | Some(Order::None)) {
                score.set(0.0);
                continue;
            }

            score.set(0.6);
            /*if tired.tired >= 80.0 {
                span.span()
                    .in_scope(|| debug!("Tired above threshold! Score: {}", tired.tired / 100.0));
            }*/
        }
    }
}

/// Scores the need to load items into an assigned structure.
///
/// Returns score 0.0 for villagers without assignments (expected for idle villagers).
/// This is not an error - villagers start with no assignment until work is assigned.
pub fn structure_capacity_scorer_system(
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    protection: VillagerProtection,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<StructureCapacityScorer>>,
    villager_query: Query<(&Id, &Assignment)>,
    structure_query: Query<(&Id, &Template, &Inventory)>,
) {
    for (Actor(actor), mut score, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);

        let Ok((villager_id, assignment)) = villager_query.get(*actor) else {
            // Expected for idle villagers without work assignments
            span.span().in_scope(|| {
                villager_debug!(*actor, obj_id, None, "No villager assignment");
            });
            score.set(0.0);
            continue;
        };

        let Some(structure_entity) = entity_map.get_entity(assignment.structure_id) else {
            span.span().in_scope(|| {
                villager_error!(
                    *actor,
                    obj_id,
                    None,
                    "No structure entity for structure_id={}",
                    assignment.structure_id
                );
            });
            score.set(0.0);
            continue;
        };

        let Ok((structure_id, structure_template, inventory)) =
            structure_query.get(structure_entity)
        else {
            span.span().in_scope(|| {
                villager_error!(
                    *actor,
                    obj_id,
                    None,
                    "No inventory component for structure_id={}",
                    assignment.structure_id
                );
            });
            score.set(0.0);
            continue;
        };

        // If structure capacity is over 90% full set score to MAX ROUTINE
        let current_weight = inventory.get_total_weight();
        let structure_capacity = Obj::get_capacity(&structure_template.0, &templates.obj_templates);

        span.span().in_scope(|| {
            villager_trace!(
                *actor,
                obj_id,
                None,
                "weight={} capacity={}",
                current_weight,
                structure_capacity
            );
        });

        if current_weight >= (structure_capacity as f32 * 0.9) as i32 {
            score.set(MAX_ROUTINE_SCORE / 100.0);
        } else {
            score.set(0.0);
        }
    }
}

// Foraged food is light (weight 1). The haul-to-storage action (CapacityScorer)
// only outscores gathering (GoodMorale = 0.6) once resource_weight passes ~90, so
// raw food weight would make a villager hoard a near-full stack before stocking
// the larder. Counting each food unit as this many weight units makes a foraging
// villager carry its harvest back after ~12 items — frequent enough that the
// hero's larder actually fills during a run.
const FOOD_HAUL_WEIGHT: i32 = 8;

pub fn capacity_scorer_system(
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    protection: VillagerProtection,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<CapacityScorer>>,
    villager_query: Query<(&Id, &Template, &Inventory)>,
) {
    for (Actor(actor), mut score, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        let Ok((villager_id, villager_template, inventory)) = villager_query.get(*actor) else {
            span.span().in_scope(|| {
                villager_error!(*actor, obj_id, None, "No villager component");
            });
            continue;
        };

        // Check if villager has an order, if not set score to 0.0
        let total_weight = inventory.get_total_weight();
        let total_weight_ore = inventory.get_total_weight_by_class(ORE.to_string());
        let total_weight_log = inventory.get_total_weight_by_class(LOG.to_string());
        // Food gathered for the larder is haulable too: a villager assigned to
        // forage (Plant -> berries/mushrooms) should carry its harvest back to a
        // storage so the hero can eat it, just like ore/logs. Food is light
        // (weight 1), so without a count multiplier the haul would only trigger
        // after ~90 items; FOOD_HAUL_WEIGHT makes a foraging villager top up the
        // larder after a reasonable trip instead of hoarding a full stack.
        let total_weight_food = inventory.get_total_weight_by_class(FOOD.to_string());

        let resource_weight =
            total_weight_ore + total_weight_log + total_weight_food * FOOD_HAUL_WEIGHT;
        let non_resource_weight =
            total_weight - total_weight_ore - total_weight_log - total_weight_food;

        let capacity = Obj::get_capacity(&villager_template.0, &templates.obj_templates);

        let mut capacity_score =
            (resource_weight as f32) / (capacity - non_resource_weight) as f32 * 100.0;

        if capacity_score > MAX_ROUTINE_SCORE {
            capacity_score = MAX_ROUTINE_SCORE;
        }

        //info!("Capacity score: {:?}", capacity_score);

        score.set(capacity_score / 100.0);
    }
}

pub fn idle_action_system(
    game_tick: Res<GameTick>,
    protection: VillagerProtection,
    mut active_task_query: Query<&mut ActiveTask>,
    mut query: Query<(&Actor, &mut ActionState, &mut Idle, &ActionSpan)>,
) {
    for (Actor(actor), mut state, mut idle, _span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        match *state {
            ActionState::Requested => {
                if let Ok(mut active_task) = active_task_query.get_mut(*actor) {
                    ActiveTask::set_if_changed(&mut active_task, ActiveTask::Idle);
                }

                idle.start_time = game_tick.0;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if game_tick.0.saturating_sub(idle.start_time) >= idle.duration {
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

/// Sets the destination based on the villager's current order.
///
/// Returns ActionState::Failure for Order::None and orders without valid positions.
/// This is expected behavior - idle villagers have Order::None by design.
pub fn set_order_destination_system(
    mut commands: Commands,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    map: Res<Map>,
    obj_query: Query<(&PlayerId, &Id, &Position, &Class, &Stats)>,
    storage_query: Query<BaseQuery, (With<ClassStructure>, Without<SubclassVillager>)>,
    mut villager_query: Query<
        (
            &PlayerId,
            &Position,
            &Order,
            Option<&Assignment>, // Villager may not have an assignment
            &mut Inventory,
            &mut ActiveTask,
        ),
        With<SubclassVillager>,
    >,
    mut action_query: Query<(&Actor, &mut ActionState, &SetOrderDestination, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _set_order_destination, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((
                    villager_player_id,
                    villager_pos,
                    order,
                    assignment,
                    mut villager_inventory,
                    mut active_task,
                )) = villager_query.get_mut(*actor)
                else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let order_pos = match order {
                    Order::Follow { target } => {
                        let Ok((_player_id, _id, target_pos, _class, _stats)) =
                            obj_query.get(*target)
                        else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No target position for follow order"
                                );
                            });
                            *state = ActionState::Failure;
                            continue;
                        };

                        Some(*target_pos)
                    }
                    Order::Gather {
                        res_type,
                        pos,
                        storage_pos: _,
                        storage_id: _,
                    } => {
                        if let Some(required_attr) = item::required_tool_attr_for_res_type(res_type)
                        {
                            let updated_items =
                                villager_inventory.auto_equip_best_tool_for_attr(&required_attr);

                            if !updated_items.is_empty()
                                || villager_inventory.has_equipped_tool_for_attr(&required_attr)
                            {
                                commands.entity(*actor).remove::<BlockedWork>();
                                commands.entity(*actor).remove::<ToolFetchTarget>();
                                ActiveTask::set_if_changed(
                                    &mut active_task,
                                    ActiveTask::get_activity_from_res_type(res_type.clone()),
                                );
                                Some(*pos)
                            } else {
                                let mut best_storage_tool: Option<(f32, u32, i32, i32, Position)> =
                                    None;

                                for structure in storage_query.iter() {
                                    if villager_player_id.0 != structure.player_id.0 {
                                        continue;
                                    }

                                    if *structure.subclass != Subclass::Storage {
                                        continue;
                                    }

                                    if *structure.state != State::None {
                                        continue;
                                    }

                                    let Some(tool) =
                                        structure.inventory.best_tool_for_attr(&required_attr)
                                    else {
                                        continue;
                                    };

                                    let Some((_path, cost)) = Map::find_fast_path(
                                        *villager_pos,
                                        *structure.pos,
                                        &map,
                                        villager_player_id.0,
                                        Vec::new(),
                                        true,
                                        false,
                                        false,
                                        false,
                                        true,
                                    ) else {
                                        continue;
                                    };

                                    let score = tool.attr_num(&required_attr);
                                    let is_better = match best_storage_tool {
                                        None => true,
                                        Some((best_score, best_cost, best_item_id, _, _)) => {
                                            score > best_score
                                                || (score == best_score
                                                    && (cost < best_cost
                                                        || (cost == best_cost
                                                            && tool.id < best_item_id)))
                                        }
                                    };

                                    if is_better {
                                        best_storage_tool = Some((
                                            score,
                                            cost,
                                            tool.id,
                                            structure.id.0,
                                            *structure.pos,
                                        ));
                                    }
                                }

                                if let Some((_score, _cost, item_id, storage_id, storage_pos)) =
                                    best_storage_tool
                                {
                                    commands.entity(*actor).insert(ToolFetchTarget {
                                        storage_id,
                                        item_id,
                                        res_type: res_type.clone(),
                                        required_attr,
                                    });
                                    commands.entity(*actor).remove::<BlockedWork>();
                                    ActiveTask::set_if_changed(
                                        &mut active_task,
                                        ActiveTask::MovingToGatherPos,
                                    );
                                    Some(storage_pos)
                                } else {
                                    let reason = format!(
                                        "Needs {} tool",
                                        item::tool_attr_label(&required_attr)
                                    );
                                    commands.entity(*actor).insert(BlockedWork { reason });
                                    commands.entity(*actor).remove::<ToolFetchTarget>();
                                    ActiveTask::set_if_changed(
                                        &mut active_task,
                                        ActiveTask::Unknown,
                                    );
                                    *state = ActionState::Failure;
                                    continue;
                                }
                            }
                        } else {
                            Some(*pos)
                        }
                    }
                    Order::Build
                    | Order::Operate
                    | Order::Plant
                    | Order::Tend
                    | Order::Harvest
                    | Order::WorkQueue => {
                        if let Some(assignment) = assignment {
                            Some(assignment.structure_pos)
                        } else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No assignment for work queue order"
                                );
                            });
                            *state = ActionState::Failure;
                            continue;
                        }
                    }
                    Order::Repair => {
                        let mut structures_to_repair = Vec::new();

                        for (player_id, id, target_pos, class, stats) in obj_query.iter() {
                            if villager_player_id.0 == player_id.0
                                && class.is_structure()
                                && stats.hp < stats.base_hp
                            {
                                structures_to_repair.push((*id, *target_pos));
                            }
                        }

                        let nearest_structure = structures_to_repair
                            .iter()
                            .min_by_key(|(_id, pos)| Map::dist(*villager_pos, *pos));

                        let Some(nearest_structure) = nearest_structure else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No nearest structure for repair order"
                                );
                            });
                            *state = ActionState::Failure;
                            continue;
                        };

                        let (id, pos) = nearest_structure;

                        commands.entity(*actor).insert(Target(id.0));

                        Some(*pos)
                    }
                    _ => None,
                };

                let Some(order_pos) = order_pos else {
                    // Only log error for non-idle orders that should have a position
                    if !matches!(order, Order::None | Order::Explore) {
                        span.span().in_scope(|| {
                            villager_error!(
                                *actor,
                                obj_id,
                                None,
                                "No order position for order={:?}",
                                order
                            );
                        });
                    } else {
                        span.span().in_scope(|| {
                            villager_debug!(
                                *actor,
                                obj_id,
                                None,
                                "No order position for idle order={:?}",
                                order
                            );
                        });
                    }
                    *state = ActionState::Failure;
                    continue;
                };

                commands
                    .entity(*actor)
                    .insert(Destination { pos: order_pos });

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling set order destination");
                });
                *state = ActionState::Failure
            }
            _ => {}
        }
    }
}

pub fn maybe_transfer_gather_tool_system(
    mut commands: Commands,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    mut inventory_query: Query<(&PlayerId, &Position, &mut Inventory)>,
    mut villager_query: Query<(&Order, &mut ActiveTask), With<SubclassVillager>>,
    fetch_query: Query<&ToolFetchTarget>,
    mut action_query: Query<(
        &Actor,
        &mut ActionState,
        &MaybeTransferGatherTool,
        &ActionSpan,
    )>,
) {
    for (Actor(actor), mut state, _maybe_transfer, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok(fetch_target) = fetch_query.get(*actor) else {
                    *state = ActionState::Success;
                    continue;
                };

                let missing_tool_reason = format!(
                    "Needs {} tool",
                    item::tool_attr_label(&fetch_target.required_attr)
                );

                let Some(storage_entity) = entity_map.get_entity(fetch_target.storage_id) else {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Cannot find storage entity for tool fetch storage_id={}",
                            fetch_target.storage_id
                        );
                    });
                    commands.entity(*actor).insert(BlockedWork {
                        reason: missing_tool_reason,
                    });
                    commands.entity(*actor).remove::<ToolFetchTarget>();
                    if let Ok((_order, mut active_task)) = villager_query.get_mut(*actor) {
                        ActiveTask::set_if_changed(&mut active_task, ActiveTask::Unknown);
                    }
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(
                    [(villager_player_id, villager_pos, mut villager_inventory), (storage_player_id, storage_pos, mut storage_inventory)],
                ) = inventory_query.get_many_mut([*actor, storage_entity])
                else {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Cannot find inventories for tool fetch"
                        );
                    });
                    commands.entity(*actor).insert(BlockedWork {
                        reason: missing_tool_reason,
                    });
                    commands.entity(*actor).remove::<ToolFetchTarget>();
                    if let Ok((_order, mut active_task)) = villager_query.get_mut(*actor) {
                        ActiveTask::set_if_changed(&mut active_task, ActiveTask::Unknown);
                    }
                    *state = ActionState::Failure;
                    continue;
                };

                if villager_player_id.0 != storage_player_id.0 || villager_pos != storage_pos {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Villager is not at owned storage for tool fetch"
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                let Some(tool) = storage_inventory.get_by_id(fetch_target.item_id) else {
                    commands.entity(*actor).insert(BlockedWork {
                        reason: missing_tool_reason,
                    });
                    commands.entity(*actor).remove::<ToolFetchTarget>();
                    if let Ok((_order, mut active_task)) = villager_query.get_mut(*actor) {
                        ActiveTask::set_if_changed(&mut active_task, ActiveTask::Unknown);
                    }
                    *state = ActionState::Failure;
                    continue;
                };

                if !tool.is_gather_tool_for_attr(&fetch_target.required_attr) {
                    commands.entity(*actor).insert(BlockedWork {
                        reason: missing_tool_reason,
                    });
                    commands.entity(*actor).remove::<ToolFetchTarget>();
                    if let Ok((_order, mut active_task)) = villager_query.get_mut(*actor) {
                        ActiveTask::set_if_changed(&mut active_task, ActiveTask::Unknown);
                    }
                    *state = ActionState::Failure;
                    continue;
                }

                Inventory::transfer(
                    fetch_target.item_id,
                    &mut storage_inventory,
                    &mut villager_inventory,
                );
                villager_inventory.auto_equip_best_tool_for_res_type(&fetch_target.res_type);

                if !villager_inventory.has_equipped_tool_for_attr(&fetch_target.required_attr) {
                    commands.entity(*actor).insert(BlockedWork {
                        reason: missing_tool_reason,
                    });
                    commands.entity(*actor).remove::<ToolFetchTarget>();
                    if let Ok((_order, mut active_task)) = villager_query.get_mut(*actor) {
                        ActiveTask::set_if_changed(&mut active_task, ActiveTask::Unknown);
                    }
                    *state = ActionState::Failure;
                    continue;
                }

                let Ok((order, mut active_task)) = villager_query.get_mut(*actor) else {
                    commands.entity(*actor).remove::<ToolFetchTarget>();
                    commands.entity(*actor).remove::<BlockedWork>();
                    *state = ActionState::Success;
                    continue;
                };

                if let Order::Gather { pos, .. } = order {
                    commands.entity(*actor).insert(Destination { pos: *pos });
                }

                commands.entity(*actor).remove::<ToolFetchTarget>();
                commands.entity(*actor).remove::<BlockedWork>();
                ActiveTask::set_if_changed(
                    &mut active_task,
                    ActiveTask::get_activity_from_res_type(fetch_target.res_type.clone()),
                );

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn process_order_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    event_completed: Query<&EventCompleted>,
    (
        villager_query,
        template_query,
        mut active_task_query,
        mut state_query,
        mut work_queue_query,
        mut event_executing_query,
        mut query,
        last_combat_tick_query,
    ): (
        Query<
            (
                &Id,
                &Order,
                Option<&Assignment>,
                Option<&Target>,
                &Inventory,
            ),
            With<SubclassVillager>,
        >,
        Query<(&Name, &Template, &Inventory)>,
        Query<&mut ActiveTask>,
        Query<&mut State>,
        Query<&mut WorkQueue>,
        Query<&mut EventExecuting>,
        Query<(&Actor, &mut ActionState, &ProcessOrder, &ActionSpan)>,
        Query<&LastCombatTick>,
    ),
) {
    for (Actor(actor), mut state, _process_order, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                let Ok((
                    villager_id,
                    villager_order,
                    villager_assignment,
                    villager_target,
                    villager_inventory,
                )) = villager_query.get(*actor)
                else {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            obj_id,
                            None,
                            "No order to execute or villager is busy"
                        );
                    });
                    continue;
                };

                if order_is_combat_locked(villager_order)
                    && actor_is_combat_locked(*actor, game_tick.0, &last_combat_tick_query)
                {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            obj_id,
                            None,
                            "Cannot process peaceful order while in combat"
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                let Ok(mut active_task) = active_task_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "No active task component");
                    });
                    continue;
                };

                let Ok(mut villager_state) = state_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "No state component");
                    });
                    continue;
                };

                if *villager_state != State::None {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            obj_id,
                            None,
                            "Process order called but villager is not idle"
                        );
                    });
                    continue;
                }

                span.span().in_scope(|| {
                    villager_debug!(
                        *actor,
                        obj_id,
                        None,
                        "Processing order={:?}",
                        villager_order
                    );
                });

                match villager_order {
                    Order::Follow { target: _ } => {
                        ActiveTask::set_if_changed(&mut active_task, ActiveTask::Following);
                        // Follow has no event to wait for — succeed immediately
                        // so the behavior tree loops (SetOrderDestination → MoveTo → ProcessOrder)
                        *state = ActionState::Success;
                        continue;
                    }
                    Order::Gather {
                        res_type,
                        pos: _,
                        storage_pos: _,
                        storage_id: _,
                    } => {
                        if let Some(required_attr) = item::required_tool_attr_for_res_type(res_type)
                        {
                            if !villager_inventory.has_equipped_tool_for_attr(&required_attr) {
                                commands.entity(*actor).insert(BlockedWork {
                                    reason: format!(
                                        "Needs {} tool",
                                        item::tool_attr_label(&required_attr)
                                    ),
                                });
                                ActiveTask::set_if_changed(&mut active_task, ActiveTask::Unknown);
                                *state = ActionState::Failure;
                                continue;
                            }
                        }

                        commands.entity(*actor).remove::<BlockedWork>();
                        *villager_state = State::Gathering;

                        // Add Game Event to start work
                        let event = GameEvent {
                            event_id: ids.new_map_event_id(),
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + 1,
                            event_type: GameEventType::GatherEvent {
                                gatherer_id: villager_id.0,
                                res_type: res_type.clone(),
                            },
                        };

                        game_events.insert(event.event_id, event);
                    }
                    Order::Build => {
                        span.span().in_scope(|| {
                            villager_debug!(*actor, obj_id, None, "Processing build order");
                        });

                        let Some(assignment) = villager_assignment else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No assignment for build order"
                                );
                            });
                            continue;
                        };

                        // Get structure entity
                        let Some(structure_entity) = entity_map.get_entity(assignment.structure_id)
                        else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No structure entity for structure_id={}",
                                    assignment.structure_id
                                );
                            });
                            continue;
                        };

                        let Ok(structure_state) = state_query.get(structure_entity) else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No structure state component for entity={:?}",
                                    structure_entity
                                );
                            });
                            continue;
                        };

                        // If structure is already completed don't add trigger
                        if *structure_state == State::None {
                            span.span().in_scope(|| {
                                villager_debug!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "Structure is already completed"
                                );
                            });
                            continue;
                        }

                        // Set active task to building
                        ActiveTask::set_if_changed(&mut active_task, ActiveTask::Building);

                        // Add trigger to start build
                        span.span().in_scope(|| {
                            villager_debug!(*actor, obj_id, None, "Adding trigger to start build");
                        });
                        commands.trigger(StartBuild {
                            entity: structure_entity,
                            builder_entity: *actor,
                        });
                    }
                    Order::WorkQueue => {
                        // Unwrap assignment
                        let Some(assignment) = villager_assignment else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No assignment for work queue order"
                                );
                            });
                            continue;
                        };

                        // Get structure entity
                        let Some(structure_entity) = entity_map.get_entity(assignment.structure_id)
                        else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No structure entity for structure_id={}",
                                    assignment.structure_id
                                );
                            });
                            continue;
                        };

                        let Ok((structure_name, template, inventory)) =
                            template_query.get(structure_entity)
                        else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No template for structure entity={:?}",
                                    structure_entity
                                );
                            });
                            continue;
                        };

                        let Ok(mut work_queue) = work_queue_query.get_mut(structure_entity) else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No work queue for structure entity={:?}",
                                    structure_entity
                                );
                            });
                            continue;
                        };

                        // Check if there is an available work entry
                        if work_queue.0.iter().all(|entry| entry.worker_id != -1) {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No available work entry for structure_id={}",
                                    assignment.structure_id
                                );
                            });
                            continue;
                        }

                        // Find first work entry with no villager id
                        let Some(work_entry) =
                            work_queue.0.iter_mut().find(|entry| entry.worker_id == -1)
                        else {
                            span.span().in_scope(|| {
                                villager_debug!(*actor, obj_id, None, "No available work entry");
                            });
                            continue;
                        };

                        // Assign villager id to work entry
                        work_entry.worker_id = villager_id.0;

                        // Trigger start work
                        commands.trigger(StartWork {
                            entity: *actor,
                            worker_id: villager_id.0,
                            structure_id: assignment.structure_id,
                        });
                    }
                    Order::Operate => {
                        // Unwrap assignment
                        let Some(assignment) = villager_assignment else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No assignment for operate order"
                                );
                            });
                            continue;
                        };

                        // Get structure entity
                        let Some(structure_entity) = entity_map.get_entity(assignment.structure_id)
                        else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No structure entity for structure_id={}",
                                    assignment.structure_id
                                );
                            });
                            continue;
                        };

                        let Ok((structure_name, template, inventory)) =
                            template_query.get(structure_entity)
                        else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No template for structure entity={:?}",
                                    structure_entity
                                );
                            });
                            continue;
                        };

                        let obj_template = templates
                            .obj_templates
                            .get_by_name_template(structure_name.0.clone(), template.0.clone());
                        let capacity = obj_template.capacity.unwrap_or(0);
                        let activity_str = obj_template.activity.unwrap_or("Operating".to_string());

                        let current_total_weight = inventory.get_total_weight();

                        if current_total_weight >= capacity {
                            span.span().in_scope(|| {
                                villager_debug!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "Structure is full, transferring resources"
                                );
                            });
                            commands.trigger(TransferAllResources {
                                entity: structure_entity,
                                target_entity: *actor,
                            });
                            continue;
                        } else {
                            let operate_event = VisibleEvent::OperateEvent {
                                structure_id: assignment.structure_id,
                            };

                            *villager_state = State::Operating;

                            map_events.new(
                                villager_id.0,
                                game_tick.0 + 40, // in the future
                                operate_event,
                            );
                        }
                    }
                    Order::Plant => {
                        // Unwrap assignment
                        let Some(assignment) = villager_assignment else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No assignment for plant order"
                                );
                            });
                            continue;
                        };

                        // Create plant event
                        let plant_event = VisibleEvent::PlantEvent {
                            structure_id: assignment.structure_id,
                        };

                        *villager_state = State::Planting;

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Planting,
                        });

                        map_events.new(
                            villager_id.0,
                            game_tick.0 + 50, // in the future
                            plant_event,
                        );
                    }
                    Order::Harvest => {
                        // Unwrap assignment
                        let Some(assignment) = villager_assignment else {
                            span.span().in_scope(|| {
                                villager_error!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "No assignment for harvest order"
                                );
                            });
                            continue;
                        };

                        span.span().in_scope(|| {
                            villager_debug!(*actor, obj_id, None, "Creating Harvest Event");
                        });
                        // Create harvest event
                        let harvest_event = VisibleEvent::HarvestEvent {
                            structure_id: assignment.structure_id,
                        };

                        *villager_state = State::Harvesting;

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Harvesting,
                        });

                        map_events.new(
                            villager_id.0,
                            game_tick.0 + 50, // in the future
                            harvest_event,
                        );
                    }
                    Order::Repair => {
                        *villager_state = State::Repairing;

                        // Get target structure id
                        let Some(villager_target) = villager_target else {
                            span.span().in_scope(|| {
                                villager_error!(*actor, obj_id, None, "No target for repair order");
                            });
                            *state = ActionState::Failure;
                            continue;
                        };

                        // Add Repair Event
                        let repair_event = VisibleEvent::RepairEvent {
                            structure_id: villager_target.0,
                        };

                        map_events.new(
                            villager_id.0,
                            game_tick.0 + 50, // in the future
                            repair_event,
                        );

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Repairing,
                        });
                    }
                    Order::Explore => {
                        let explore_event = VisibleEvent::ProspectEvent;

                        map_events.new(
                            villager_id.0,
                            game_tick.0 + 8, // in the future
                            explore_event,
                        );
                    }
                    _ => {}
                }

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if actor_is_combat_locked(*actor, game_tick.0, &last_combat_tick_query) {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cancelling order for combat lock");
                    });
                    if let Ok(mut event_executing) = event_executing_query.get_mut(*actor) {
                        event_executing.state = EventExecutingState::None;
                    }
                    commands.trigger(CancelEvents { entity: *actor });
                    *state = ActionState::Failure;
                    continue;
                }

                span.span().in_scope(|| {
                    villager_trace!(*actor, obj_id, None, "Process Order executing");
                });
                if let Ok(_event_completed) = event_completed.get(*actor) {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Event completed");
                    });
                    let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
                        span.span().in_scope(|| {
                            villager_error!(*actor, obj_id, None, "Cannot find event executing");
                        });
                        continue;
                    };
                    event_executing.state = EventExecutingState::Completed;
                    commands.entity(*actor).remove::<EventCompleted>();

                    *state = ActionState::Success;
                } else {
                    span.span().in_scope(|| {
                        villager_trace!(
                            *actor,
                            obj_id,
                            None,
                            "Process Order still executing, waiting for completed"
                        );
                    });
                }
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling order action");
                });
                let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find event executing");
                    });
                    continue;
                };
                event_executing.state = EventExecutingState::Completed;
                commands.entity(*actor).remove::<EventCompleted>(); // TODO is this needed?

                commands.trigger(CancelEvents { entity: *actor });

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn fight_back_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    mut game_events: ResMut<GameEvents>,
    last_attacker_query: Query<&LastAttacker>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut combat_query: Query<CombatQuery>,
    mut action_query: Query<(&Actor, &mut ActionState, &FightBack, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _fight_back, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "FightBack requested");
                });

                let Ok(last_attacker) = last_attacker_query.get(*actor) else {
                    span.span().in_scope(|| {
                        villager_warn!(*actor, obj_id, None, "FightBack had no LastAttacker");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                if game_tick.0.saturating_sub(last_attacker.tick) > RETALIATION_MEMORY_TICKS {
                    span.span().in_scope(|| {
                        villager_warn!(
                            *actor,
                            obj_id,
                            None,
                            "LastAttacker id={} expired",
                            last_attacker.id
                        );
                    });
                    commands.entity(*actor).remove::<LastAttacker>();
                    *state = ActionState::Failure;
                    continue;
                }

                let Some(attacker_entity) = entity_map.get_entity(last_attacker.id) else {
                    span.span().in_scope(|| {
                        villager_warn!(
                            *actor,
                            obj_id,
                            None,
                            "Cannot find LastAttacker entity id={}",
                            last_attacker.id
                        );
                    });
                    commands.entity(*actor).remove::<LastAttacker>();
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok([mut villager, mut attacker]) =
                    combat_query.get_many_mut([*actor, attacker_entity])
                else {
                    span.span().in_scope(|| {
                        villager_warn!(
                            *actor,
                            obj_id,
                            None,
                            "Cannot query villager or attacker for FightBack"
                        );
                    });
                    commands.entity(*actor).remove::<LastAttacker>();
                    *state = ActionState::Failure;
                    continue;
                };

                if Obj::is_dead(&villager.state) {
                    commands.entity(*actor).remove::<LastAttacker>();
                    *state = ActionState::Failure;
                    continue;
                }

                if villager.inventory.get_equipped_weapons().is_empty() {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            Some(villager.id.0),
                            None,
                            "Cannot fight back without an equipped weapon"
                        );
                    });
                    commands.entity(*actor).remove::<LastAttacker>();
                    *state = ActionState::Failure;
                    continue;
                }

                let villager_stamina = villager.stats.stamina.unwrap_or(0);
                if villager_stamina < 5 {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            Some(villager.id.0),
                            None,
                            "Cannot fight back with stamina={}",
                            villager_stamina
                        );
                    });
                    commands.entity(*actor).remove::<LastAttacker>();
                    *state = ActionState::Failure;
                    continue;
                }

                if attacker.player_id.0 == villager.player_id.0
                    || attacker.player_id.0 == MERCHANT_PLAYER_ID
                    || protection.owner_is_protected(attacker.player_id)
                    || protection.object_is_protected(attacker.id.0, &ids)
                    || Obj::is_dead(&attacker.state)
                    || Map::dist(*villager.pos, *attacker.pos) > 1
                {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            Some(villager.id.0),
                            None,
                            "LastAttacker id={} is no longer a valid adjacent enemy",
                            last_attacker.id
                        );
                    });
                    commands.entity(*actor).remove::<LastAttacker>();
                    *state = ActionState::Failure;
                    continue;
                }

                if Combat::target_is_fortified(&attacker) {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            Some(villager.id.0),
                            None,
                            "Cannot fight back with melee against fortified attacker id={}",
                            last_attacker.id
                        );
                    });
                    commands.entity(*actor).remove::<LastAttacker>();
                    *state = ActionState::Failure;
                    continue;
                }

                if let Some(errmsg) =
                    Combat::fortified_outbound_attack_error_from_combat(&villager, &attacker, false)
                {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, Some(villager.id.0), None, "{}", errmsg);
                    });
                    commands.entity(*actor).remove::<LastAttacker>();
                    *state = ActionState::Failure;
                    continue;
                }

                span.span().in_scope(|| {
                    villager_info!(
                        *actor,
                        Some(villager.id.0),
                        None,
                        "Fighting back against obj_id={}",
                        attacker.id.0
                    );
                });

                let (damage, combo, _skill_updated, _countered) = Combat::process_attack(
                    AttackType::Quick,
                    &mut villager,
                    &mut attacker,
                    &mut commands,
                    &templates,
                    &map,
                    &mut ids,
                    &game_tick,
                    &mut map_events,
                );

                Combat::add_damage_event(
                    game_tick.0,
                    "quick".to_string(),
                    damage,
                    combo,
                    false,
                    &villager,
                    &attacker,
                    &mut map_events,
                );

                commands.entity(*actor).remove::<LastAttacker>();

                let cooldown = villager.stats.base_speed.unwrap_or(1) * TICKS_PER_SEC;
                map_events.new(
                    villager.id.0,
                    game_tick.0 + cooldown,
                    VisibleEvent::CooldownEvent { duration: cooldown },
                );

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::Executing;

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                if !event_executing.state.is_finished() {
                    continue;
                }

                if event_executing.state.is_failed() {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "FightBack cooldown event failed");
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "FightBack cancelled");
                });

                if let Some(villager_id) = obj_id {
                    let event_id = ids.new_map_event_id();
                    game_events.insert(
                        event_id,
                        GameEvent {
                            event_id,
                            start_tick: game_tick.0,
                            run_tick: game_tick.0 + 1,
                            event_type: GameEventType::CancelAllMapEvents {
                                obj_id: villager_id,
                            },
                        },
                    );
                }

                commands.entity(*actor).remove::<LastAttacker>();
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

fn flee_path_to(
    src_pos: Position,
    dst_pos: Position,
    map: &Map,
    mover_player_id: i32,
    blocking_list: &[Blocker],
) -> Option<(Vec<MapPos>, u32)> {
    if src_pos == dst_pos {
        return Some((vec![MapPos(src_pos.x, src_pos.y)], 0));
    }

    Map::find_fast_path(
        src_pos,
        dst_pos,
        map,
        mover_player_id,
        blocking_list.to_vec(),
        true,
        false,
        false,
        false,
        false,
    )
}

fn flee_destination_is_threatened(pos: Position, threats: &[FleeThreat], map: &Map) -> bool {
    threats.iter().any(|threat| {
        enemy_threatens_villager(
            pos,
            None,
            threat.pos,
            threat.player_id,
            None,
            map,
            &threat.blocking_list,
        )
    })
}

fn min_flee_threat_distance(pos: Position, threats: &[FleeThreat]) -> u32 {
    threats
        .iter()
        .map(|threat| Map::dist(pos, threat.pos))
        .min()
        .unwrap_or(FLEE_THREAT_SCAN_RADIUS)
}

fn score_flee_candidate(
    candidate: Position,
    villager_pos: Position,
    villager_player_id: i32,
    hero_pos: Option<Position>,
    map: &Map,
    movement_blocking_list: &[Blocker],
    threats: &[FleeThreat],
    friendly_wall_positions: &HashSet<Position>,
    bonus: i32,
    skip_hero_position: bool,
) -> Option<FleeDestinationChoice> {
    if candidate == villager_pos || (skip_hero_position && Some(candidate) == hero_pos) {
        return None;
    }

    if !Map::is_valid_pos((candidate.x, candidate.y))
        || !Map::is_passable_by_obj(candidate.x, candidate.y, true, false, false, map)
    {
        return None;
    }

    let Some((path, path_cost)) = flee_path_to(
        villager_pos,
        candidate,
        map,
        villager_player_id,
        movement_blocking_list,
    ) else {
        return None;
    };

    if path.len() <= 1 {
        return None;
    }

    let fortified = friendly_wall_positions.contains(&candidate);
    let safe = fortified || !flee_destination_is_threatened(candidate, threats, map);
    if !safe {
        return None;
    }

    let threat_distance = min_flee_threat_distance(candidate, threats).min(20) as i32;
    let travel_distance = Map::dist(villager_pos, candidate).min(FLEE_SEARCH_RADIUS) as i32;
    let path_cost = path_cost.min(50) as i32;
    let path_len = path.len().min(20) as i32;

    let score = FLEE_SAFE_BONUS
        + bonus
        + if fortified { FLEE_WALL_BONUS } else { 0 }
        + threat_distance * 1_000
        + travel_distance * 75
        - path_cost * 40
        - path_len * 20;

    Some(FleeDestinationChoice {
        pos: candidate,
        score,
        safe,
        fortified,
    })
}

fn choose_best_safe_flee_destination(
    villager_pos: Position,
    villager_player_id: i32,
    hero_pos: Option<Position>,
    map: &Map,
    movement_blocking_list: &[Blocker],
    threats: &[FleeThreat],
    friendly_wall_positions: &HashSet<Position>,
) -> Option<FleeDestinationChoice> {
    Map::range((villager_pos.x, villager_pos.y), FLEE_SEARCH_RADIUS)
        .into_iter()
        .filter_map(|(x, y)| {
            score_flee_candidate(
                Position { x, y },
                villager_pos,
                villager_player_id,
                hero_pos,
                map,
                movement_blocking_list,
                threats,
                friendly_wall_positions,
                0,
                true,
            )
        })
        .max_by_key(|choice| (choice.score, choice.pos.y, choice.pos.x))
}

fn choose_hero_fallback_destination(
    villager_pos: Position,
    hero_pos: Position,
    villager_player_id: i32,
    map: &Map,
    movement_blocking_list: &[Blocker],
) -> Option<FleeDestinationChoice> {
    if villager_pos == hero_pos || Map::dist(villager_pos, hero_pos) > FLEE_HERO_FALLBACK_RANGE {
        return None;
    }

    flee_path_to(
        villager_pos,
        hero_pos,
        map,
        villager_player_id,
        movement_blocking_list,
    )?;

    Some(FleeDestinationChoice {
        pos: hero_pos,
        score: 0,
        safe: false,
        fortified: false,
    })
}

fn choose_emergency_flee_step(
    villager_pos: Position,
    villager_player_id: i32,
    map: &Map,
    movement_blocking_list: &[Blocker],
    threats: &[FleeThreat],
) -> Option<FleeDestinationChoice> {
    Map::get_neighbour_tiles(
        villager_pos.x,
        villager_pos.y,
        map,
        villager_player_id,
        &movement_blocking_list.to_vec(),
        true,
        false,
        false,
        false,
        false,
        MapPos(villager_pos.x, villager_pos.y),
    )
    .into_iter()
    .map(|(pos, cost)| {
        let candidate = Position { x: pos.0, y: pos.1 };
        let threatened = flee_destination_is_threatened(candidate, threats, map);
        let threat_distance = min_flee_threat_distance(candidate, threats).min(20) as i32;
        let score = if threatened { 0 } else { FLEE_SAFE_BONUS / 2 } + threat_distance * 1_000
            - cost.min(50) as i32 * 40;

        FleeDestinationChoice {
            pos: candidate,
            score,
            safe: !threatened,
            fortified: false,
        }
    })
    .max_by_key(|choice| (choice.score, choice.pos.y, choice.pos.x))
}

pub fn set_flee_destination_system(
    mut commands: Commands,
    map: Res<Map>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    hero_query: Query<&Position, (With<SubclassHero>, Without<SubclassVillager>)>,
    villager_query: Query<
        (
            &Id,
            &PlayerId,
            &Position,
            Option<&Effects>,
            Option<&ActiveShelter>,
        ),
        With<SubclassVillager>,
    >,
    shelter_query: Query<&Position, (With<Shelter>, Without<SubclassVillager>)>,
    blocking_query: Query<BaseQuery>,
    threat_query: Query<
        (
            Entity,
            &Id,
            &PlayerId,
            &Position,
            Option<&Template>,
            &Class,
            &State,
            Option<&Effects>,
        ),
        Without<SubclassHero>,
    >,
    mut action_query: Query<(&Actor, &mut ActionState, &SetFleeDestination, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _set_flee_destination, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "SetFleeDestination requested");
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((
                    villager_id,
                    villager_player_id,
                    villager_pos,
                    villager_effects,
                    active_shelter,
                )) = villager_query.get(*actor)
                else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find villager");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                if villager_is_fortified(villager_effects) {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            Some(villager_id.0),
                            None,
                            "Holding fortified position instead of fleeing"
                        );
                    });
                    commands.entity(*actor).remove::<Destination>();
                    *state = ActionState::Failure;
                    continue;
                }

                let mut immediate_threat = false;
                let mut nearby_threats = Vec::new();

                for (
                    enemy_entity,
                    _enemy_id,
                    enemy_player_id,
                    enemy_pos,
                    enemy_template,
                    enemy_class,
                    enemy_state,
                    _enemy_effects,
                ) in threat_query.iter()
                {
                    if enemy_entity == *actor
                        || *enemy_state == State::Dead
                        || enemy_class.is_poi()
                        || enemy_player_id.0 == villager_player_id.0
                        || enemy_player_id.0 == MERCHANT_PLAYER_ID
                    {
                        continue;
                    }

                    let threat_blocking_list =
                        Obj::blocking_list_basequery(enemy_player_id.0, &blocking_query);

                    let is_immediate_threat = enemy_threatens_villager(
                        *villager_pos,
                        villager_effects,
                        *enemy_pos,
                        enemy_player_id.0,
                        enemy_template,
                        &map,
                        &threat_blocking_list,
                    );

                    if Map::dist(*villager_pos, *enemy_pos) <= FLEE_THREAT_SCAN_RADIUS
                        || is_immediate_threat
                    {
                        nearby_threats.push(FleeThreat {
                            pos: *enemy_pos,
                            player_id: enemy_player_id.0,
                            blocking_list: threat_blocking_list,
                        });
                    }

                    if is_immediate_threat {
                        immediate_threat = true;
                    }
                }

                if !immediate_threat {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            Some(villager_id.0),
                            None,
                            "No immediate flee threat"
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                let blocking_list =
                    Obj::blocking_list_basequery(villager_player_id.0, &blocking_query);
                let friendly_wall_positions: HashSet<Position> = blocking_query
                    .iter()
                    .filter(|obj| {
                        obj.player_id.0 == villager_player_id.0
                            && *obj.subclass == Subclass::Wall
                            && obj.state.is_active()
                    })
                    .map(|obj| *obj.pos)
                    .collect();

                let Some(hero_id) = ids.get_hero(villager_player_id.0) else {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            Some(villager_id.0),
                            None,
                            "Cannot find hero for player_id={}",
                            villager_player_id.0
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            Some(villager_id.0),
                            None,
                            "Cannot find hero entity for hero_id={}",
                            hero_id
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(hero_pos_ref) = hero_query.get(hero_entity) else {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            Some(villager_id.0),
                            None,
                            "Cannot find hero for entity={:?}",
                            hero_entity
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                };
                let hero_pos = *hero_pos_ref;

                let mut best_choice = choose_best_safe_flee_destination(
                    *villager_pos,
                    villager_player_id.0,
                    Some(hero_pos),
                    &map,
                    &blocking_list,
                    &nearby_threats,
                    &friendly_wall_positions,
                );

                if let Some(active_shelter) = active_shelter {
                    if active_shelter.0 != NO_SHELTER {
                        let shelter_choice = entity_map
                            .get_entity(active_shelter.0)
                            .and_then(|shelter_entity| shelter_query.get(shelter_entity).ok())
                            .and_then(|shelter_pos| {
                                if Map::dist(*villager_pos, *shelter_pos) > FLEE_SEARCH_RADIUS {
                                    return None;
                                }

                                score_flee_candidate(
                                    *shelter_pos,
                                    *villager_pos,
                                    villager_player_id.0,
                                    Some(hero_pos),
                                    &map,
                                    &blocking_list,
                                    &nearby_threats,
                                    &friendly_wall_positions,
                                    FLEE_SHELTER_BONUS,
                                    true,
                                )
                            });

                        if let Some(shelter_choice) = shelter_choice {
                            if best_choice
                                .map(|choice| shelter_choice.score > choice.score)
                                .unwrap_or(true)
                            {
                                best_choice = Some(shelter_choice);
                            }
                        } else {
                            span.span().in_scope(|| {
                                villager_warn!(
                                    *actor,
                                    Some(villager_id.0),
                                    None,
                                    "Active shelter id={} was not safe or reachable for fleeing",
                                    active_shelter.0
                                );
                            });
                        }
                    }
                }

                if let Some(choice) = best_choice {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            Some(villager_id.0),
                            None,
                            "Fleeing to safe tile ({}, {}), fortified={}, score={}",
                            choice.pos.x,
                            choice.pos.y,
                            choice.fortified,
                            choice.score
                        );
                    });
                    commands
                        .entity(*actor)
                        .insert(Destination { pos: choice.pos });
                    *state = ActionState::Success;
                    continue;
                }

                if let Some(choice) = choose_hero_fallback_destination(
                    *villager_pos,
                    hero_pos,
                    villager_player_id.0,
                    &map,
                    &blocking_list,
                ) {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            Some(villager_id.0),
                            None,
                            "Fleeing to nearby hero fallback ({}, {})",
                            choice.pos.x,
                            choice.pos.y
                        );
                    });
                    commands
                        .entity(*actor)
                        .insert(Destination { pos: choice.pos });
                    *state = ActionState::Success;
                    continue;
                }

                if let Some(choice) = choose_emergency_flee_step(
                    *villager_pos,
                    villager_player_id.0,
                    &map,
                    &blocking_list,
                    &nearby_threats,
                ) {
                    span.span().in_scope(|| {
                        villager_debug!(
                            *actor,
                            Some(villager_id.0),
                            None,
                            "Fleeing to emergency tile ({}, {}), safe={}",
                            choice.pos.x,
                            choice.pos.y,
                            choice.safe
                        );
                    });
                    commands
                        .entity(*actor)
                        .insert(Destination { pos: choice.pos });
                    *state = ActionState::Success;
                } else {
                    span.span().in_scope(|| {
                        villager_warn!(
                            *actor,
                            Some(villager_id.0),
                            None,
                            "No valid flee destination"
                        );
                    });
                    *state = ActionState::Failure;
                }
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "SetFleeDestination cancelled");
                });
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn find_drink_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    protection: VillagerProtection,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    map: Res<Map>,
    resources: Res<Resources>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    mut villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    structure_query: Query<
        (&Id, &PlayerId, &Position, &Inventory),
        (With<ClassStructure>, Without<SubclassVillager>),
    >,
    find_event_completed: Query<&FindEventCompleted>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut action_query: Query<(&Actor, &mut ActionState, &FindDrink, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _find_drink, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                let Ok(villager) = villager_query.get(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    continue;
                };

                // Create find event
                map_events.new(
                    villager.id.0,
                    game_tick.0 + FIND_DRINK_TICKS,
                    VisibleEvent::FindDrinkEvent {
                        obj_id: villager.id.0,
                    },
                );

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::Executing;

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                if event_executing.state != EventExecutingState::Completed {
                    span.span().in_scope(|| {
                        villager_trace!(*actor, obj_id, None, "Find Drink still executing");
                    });
                    continue;
                }

                // Reset EventExecutingState back to none
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Find Drink completed");
                });
                event_executing.state = EventExecutingState::None;

                let Ok(villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    continue;
                };

                let Some((item_location, item, item_pos)) = find_item_location_by_class(
                    villager.player_id.0,
                    &villager.pos,
                    &villager.inventory,
                    &structure_query,
                    DRINK.to_string(),
                    &map,
                ) else {
                    // No drink item on hand or in storage: fall back to a natural
                    // water source (a revealed spring, or a river bank). The villager
                    // walks there and drinks directly.
                    if let Some(water) =
                        nearest_water(&villager.pos, &resources, &map, VILLAGER_WATER_RANGE)
                    {
                        span.span().in_scope(|| {
                            villager_debug!(*actor, obj_id, None, "Heading to water to drink");
                        });
                        commands.entity(*actor).remove::<NoDrinks>();
                        commands.entity(*actor).insert(DrinkingFromWater);
                        commands.entity(*actor).insert(Destination { pos: water });
                        *state = ActionState::Success;
                        continue;
                    }

                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot find any drinks");
                    });
                    commands.entity(*actor).insert(NoDrinks {
                        at_tick: game_tick.0,
                    });

                    *state = ActionState::Failure;
                    continue;
                };

                // Found a drink item: clear any pending water-drink intent and the
                // NoDrinks marker.
                commands.entity(*actor).remove::<NoDrinks>();
                commands.entity(*actor).remove::<DrinkingFromWater>();

                // Add TargetItem component
                commands.entity(*actor).insert(TargetItem(item.clone()));

                // Set destination to item position
                if item_location == ItemLocation::Own {
                    commands
                        .entity(*actor)
                        .insert(Destination { pos: *villager.pos });
                } else if item_location == ItemLocation::OwnStructure {
                    commands
                        .entity(*actor)
                        .insert(Destination { pos: item_pos });
                }

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling Find Drink action");
                });
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::None;

                commands.trigger(CancelEvents { entity: *actor });

                *state = ActionState::Failure
            }
            _ => {}
        }
    }
}

pub fn move_to_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    dest_query: Query<&Destination>,
    obj_query: Query<(&Id, &PlayerId, &Position, &Class, &Subclass, &Stats)>,
    state_query: Query<&mut State>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut action_query: Query<(&Actor, &mut ActionState, &MoveTo, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _move_to, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "MoveTo requested");
                });
                let Some(obj_id_val) = obj_id else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, None, None, "Cannot find obj id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(villager_player_id) = ids.get_player(obj_id_val) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find player id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let blocking_list =
                    Obj::blocking_list(villager_player_id, actor, &obj_query, &state_query);

                let Ok(destination) = dest_query.get(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "No Destination component");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok((id, _player_id, pos, _class, _subclass, _stats)) = obj_query.get(*actor)
                else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot get obj query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                if *pos == destination.pos {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Already at destination, success");
                    });
                    *state = ActionState::Success;
                    continue;
                }

                if let Some(path_result) = Map::find_fast_path(
                    *pos,
                    destination.pos,
                    &map,
                    villager_player_id,
                    blocking_list,
                    true,
                    false,
                    false,
                    false,
                    true,
                ) {
                    span.span().in_scope(|| {
                        villager_trace!(
                            *actor,
                            obj_id,
                            None,
                            "Path found, length={}",
                            path_result.0.len()
                        );
                    });

                    let (path, _c) = path_result;
                    let next_pos = &path[1];

                    span.span().in_scope(|| {
                        villager_trace!(
                            *actor,
                            obj_id,
                            None,
                            "Next pos=({}, {})",
                            next_pos.0,
                            next_pos.1
                        );
                    });

                    commands.trigger(StateChange {
                        entity: *actor,
                        new_state: State::Moving,
                    });

                    // Add Move Event
                    let move_event = VisibleEvent::MoveEvent {
                        src: *pos,
                        dst: Position {
                            x: next_pos.0,
                            y: next_pos.1,
                        },
                    };

                    map_events.new(
                        id.0,
                        game_tick.0 + 48, // in the future
                        move_event,
                    );

                    let mut event_executing = event_executing_query
                        .get_mut(*actor)
                        .expect("Missing EventExecuting component");
                    event_executing.state = EventExecutingState::Executing;
                } else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot find path to destination");
                    });
                    *state = ActionState::Failure
                }

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                span.span().in_scope(|| {
                    villager_trace!(*actor, obj_id, None, "MoveTo executing");
                });
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                span.span().in_scope(|| {
                    villager_trace!(
                        *actor,
                        obj_id,
                        None,
                        "Event state={:?}",
                        event_executing.state
                    );
                });
                if !event_executing.state.is_finished() {
                    span.span().in_scope(|| {
                        villager_trace!(*actor, obj_id, None, "MoveTo still executing");
                    });
                    continue;
                }

                let Some(obj_id_val) = obj_id else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, None, None, "Cannot find obj id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(villager_player_id) = ids.get_player(obj_id_val) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find player id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let blocking_list =
                    Obj::blocking_list(villager_player_id, actor, &obj_query, &state_query);

                if let Ok((id, _player_id, pos, _class, _subclass, _stats)) = obj_query.get(*actor)
                {
                    let Ok(destination) = dest_query.get(*actor) else {
                        span.span().in_scope(|| {
                            villager_error!(*actor, obj_id, None, "No Destination component");
                        });
                        *state = ActionState::Failure;
                        continue;
                    };

                    if *pos != destination.pos {
                        // Check if moving event failed
                        if event_executing.state.is_failed() {
                            span.span().in_scope(|| {
                                villager_warn!(*actor, obj_id, None, "Moving event failed");
                            });
                            *state = ActionState::Failure;
                            continue;
                        }

                        let Some(path_result) = Map::find_fast_path(
                            *pos,
                            destination.pos,
                            &map,
                            villager_player_id,
                            blocking_list,
                            true,
                            false,
                            false,
                            false,
                            true,
                        ) else {
                            span.span().in_scope(|| {
                                villager_trace!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "Cannot find path to destination"
                                );
                            });
                            *state = ActionState::Failure;
                            continue;
                        };

                        span.span().in_scope(|| {
                            villager_trace!(
                                *actor,
                                obj_id,
                                None,
                                "Path found, length={}",
                                path_result.0.len()
                            );
                        });

                        let (path, _c) = path_result;
                        let next_pos = &path[1];

                        span.span().in_scope(|| {
                            villager_trace!(
                                *actor,
                                obj_id,
                                None,
                                "Next pos=({}, {})",
                                next_pos.0,
                                next_pos.1
                            );
                        });

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Moving,
                        });

                        // Add Move Event
                        let move_event = VisibleEvent::MoveEvent {
                            src: *pos,
                            dst: Position {
                                x: next_pos.0,
                                y: next_pos.1,
                            },
                        };

                        map_events.new(
                            id.0,
                            game_tick.0 + 48, // in the future
                            move_event,
                        );

                        // Set EventExecutingState to Executing
                        event_executing.state = EventExecutingState::Executing;
                    } else {
                        span.span().in_scope(|| {
                            villager_debug!(
                                *actor,
                                obj_id,
                                None,
                                "Adjacent to destination, success"
                            );
                        });
                        *state = ActionState::Success;
                    }
                }
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling MoveTo");
                });

                let Some(villager_id) = obj_id else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, None, None, "Cannot find obj id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let event_type = GameEventType::CancelAllMapEvents {
                    obj_id: villager_id,
                };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn transfer_drink_system(
    entity_map: Res<EntityObjMap>,
    mut ids: ResMut<Ids>,
    templates: Res<Templates>,
    protection: VillagerProtection,
    villager_query: Query<(&PlayerId, &Id, &TargetItem), With<SubclassVillager>>,
    water_query: Query<&DrinkingFromWater>,
    mut inventory_query: Query<(&Id, &Position, &mut Inventory)>,
    mut action_query: Query<(&Actor, &mut ActionState, &TransferDrink, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _transfer_drink, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);

        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Transfer Drink requested");
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                // Drinking straight from a spring — nothing to transfer.
                if water_query.get(*actor).is_ok() {
                    *state = ActionState::Success;
                    continue;
                }
                let Ok((_villager_player_id, villager_id, target_item)) =
                    villager_query.get(*actor)
                else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                span.span().in_scope(|| {
                    villager_trace!(
                        *actor,
                        obj_id,
                        None,
                        "Target item owner={}",
                        target_item.0.owner
                    );
                });

                // Check if target item owner is the villager
                if target_item.0.owner == villager_id.0 {
                    // Item is already in the villager's inventory
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Item already in inventory");
                    });
                    *state = ActionState::Success;
                    continue;
                }

                // Get target item owner entity
                let Some(target_entity) = entity_map.get_entity(target_item.0.owner) else {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Cannot find target item owner entity for owner={}",
                            target_item.0.owner
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(
                    [(villager_id, villager_pos, mut villager_inventory), (target_id, target_pos, mut target_inventory)],
                ) = inventory_query.get_many_mut([*actor, target_entity])
                else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find inventories");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if villager and target are on the same position
                span.span().in_scope(|| {
                    villager_trace!(
                        *actor,
                        obj_id,
                        None,
                        "Villager pos={:?} target_id={} target_pos={:?}",
                        villager_pos,
                        target_id.0,
                        target_pos
                    );
                });
                if villager_pos != target_pos {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Villager and target not on same position"
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                // Transfer item to villager's inventory from target's inventory
                Inventory::transfer_quantity(
                    target_item.0.id,
                    ids.new_item_id(),
                    &mut target_inventory,
                    &mut villager_inventory,
                    1,
                    &templates.item_templates,
                );

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling transfer drink");
                });
                *state = ActionState::Failure
            }
            _ => {}
        }
    }
}

pub fn drink_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    protection: VillagerProtection,
    mut ids: ResMut<Ids>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    entity_map: Res<EntityObjMap>,
    event_completed: Query<&EventCompleted>,
    mut villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    mut event_executing_query: Query<&mut EventExecuting>,
    last_combat_tick_query: Query<&LastCombatTick>,
    water_query: Query<&DrinkingFromWater>,
    mut thirst_query: Query<&mut Thirst>,
    mut query: Query<(&Actor, &mut ActionState, &Drink, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _drink, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);

        // Use the drink_action's actor to look up the corresponding Thirst Component.
        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Drink action requested");
                });

                if actor_is_combat_locked(*actor, game_tick.0, &last_combat_tick_query) {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot drink while in combat");
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                // Drinking straight from the spring we walked to — quench directly,
                // no waterskin/drink item involved.
                if water_query.get(*actor).is_ok() {
                    if let Ok(mut thirst) = thirst_query.get_mut(*actor) {
                        thirst.thirst = 0.0;
                    }
                    commands.entity(*actor).remove::<DrinkingFromWater>();
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Drank from water");
                    });
                    *state = ActionState::Success;
                    continue;
                }

                let Ok(mut villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(drink_item) = villager.inventory.get_by_class(DRINK.to_owned()) else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot find drink item");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                *villager.state = State::Drinking;

                commands.trigger(StateChange {
                    entity: *actor,
                    new_state: State::Drinking,
                });

                // Create drinking event
                let drink_event = VisibleEvent::DrinkEvent {
                    item_id: drink_item.id,
                    obj_id: villager.id.0,
                };

                map_events.new(
                    villager.id.0,
                    game_tick.0 + TICKS_PER_SEC * 3, // in the future
                    drink_event,
                );

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::Executing;

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if actor_is_combat_locked(*actor, game_tick.0, &last_combat_tick_query) {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cancelling drink for combat lock");
                    });
                    if let Ok(mut event_executing) = event_executing_query.get_mut(*actor) {
                        event_executing.state = EventExecutingState::None;
                    }
                    commands.trigger(CancelEvents { entity: *actor });
                    *state = ActionState::Failure;
                    continue;
                }

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                if event_executing.state != EventExecutingState::Completed {
                    span.span().in_scope(|| {
                        villager_trace!(*actor, obj_id, None, "Drink Event still executing");
                    });
                    continue;
                }

                event_executing.state = EventExecutingState::None;
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling Drink action");
                });

                let Ok(mut villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let event_type = GameEventType::CancelAllMapEvents {
                    obj_id: villager.id.0,
                };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn find_food_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    protection: VillagerProtection,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    map: Res<Map>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    mut villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    structure_query: Query<
        (&Id, &PlayerId, &Position, &Inventory),
        (With<ClassStructure>, Without<SubclassVillager>),
    >,
    find_event_completed: Query<&FindEventCompleted>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut action_query: Query<(&Actor, &mut ActionState, &FindFood, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _find_food, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                let Ok(mut villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    continue;
                };

                map_events.new(
                    villager.id.0,
                    game_tick.0 + FIND_FOOD_TICKS, // in the future
                    VisibleEvent::FindFoodEvent {
                        obj_id: villager.id.0,
                    },
                );

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::Executing;

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                if event_executing.state != EventExecutingState::Completed {
                    span.span().in_scope(|| {
                        villager_trace!(*actor, obj_id, None, "Find Food still executing");
                    });
                    continue;
                }

                // Reset EventExecutingState back to none
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Find Food completed");
                });
                event_executing.state = EventExecutingState::None;

                let Ok(villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some((item_location, item, item_pos)) = find_item_location_by_class(
                    villager.player_id.0,
                    &villager.pos,
                    &villager.inventory,
                    &structure_query,
                    FOOD.to_string(),
                    &map,
                ) else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot find any food");
                    });
                    commands.entity(*actor).insert(NoFood {
                        at_tick: game_tick.0,
                    });

                    *state = ActionState::Failure;
                    continue;
                };

                // Remove NoFood if a food is found
                commands.entity(*actor).remove::<NoFood>();

                // Add TargetItem component
                commands.entity(*actor).insert(TargetItem(item.clone()));

                // Set destination to item position
                if item_location == ItemLocation::Own {
                    commands
                        .entity(*actor)
                        .insert(Destination { pos: *villager.pos });
                } else if item_location == ItemLocation::OwnStructure {
                    commands
                        .entity(*actor)
                        .insert(Destination { pos: item_pos });
                }

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling find food");
                });
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::None;

                let Ok(mut villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let event_type = GameEventType::CancelAllMapEvents {
                    obj_id: villager.id.0,
                };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure
            }
            _ => {}
        }
    }
}

pub fn transfer_food_system(
    entity_map: Res<EntityObjMap>,
    mut ids: ResMut<Ids>,
    templates: Res<Templates>,
    protection: VillagerProtection,
    villager_query: Query<(&PlayerId, &Id, &TargetItem), With<SubclassVillager>>,
    mut inventory_query: Query<(&Id, &Position, &mut Inventory)>,
    mut action_query: Query<(&Actor, &mut ActionState, &TransferFood, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _transfer_food, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);

        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Transfer Food requested");
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((_villager_player_id, villager_id, target_item)) =
                    villager_query.get(*actor)
                else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                span.span().in_scope(|| {
                    villager_trace!(
                        *actor,
                        obj_id,
                        None,
                        "Target item owner={}",
                        target_item.0.owner
                    );
                });

                // Check if target item owner is the villager
                if target_item.0.owner == villager_id.0 {
                    // Item is already in the villager's inventory
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Item already in inventory");
                    });
                    *state = ActionState::Success;
                    continue;
                }

                // Get target item owner entity
                let Some(target_entity) = entity_map.get_entity(target_item.0.owner) else {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Cannot find target item owner entity for owner={}",
                            target_item.0.owner
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(
                    [(villager_id, villager_pos, mut villager_inventory), (target_id, target_pos, mut target_inventory)],
                ) = inventory_query.get_many_mut([*actor, target_entity])
                else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find inventories");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if villager and target are on the same position
                span.span().in_scope(|| {
                    villager_trace!(
                        *actor,
                        obj_id,
                        None,
                        "Villager pos={:?} target_id={} target_pos={:?}",
                        villager_pos,
                        target_id.0,
                        target_pos
                    );
                });
                if villager_pos != target_pos {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Villager and target not on same position"
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                // Transfer item to villager's inventory from target's inventory
                Inventory::transfer_quantity(
                    target_item.0.id,
                    ids.new_item_id(),
                    &mut target_inventory,
                    &mut villager_inventory,
                    1,
                    &templates.item_templates,
                );

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling transfer food");
                });
                *state = ActionState::Failure
            }
            _ => {}
        }
    }
}

pub fn eat_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    protection: VillagerProtection,
    mut ids: ResMut<Ids>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    entity_map: Res<EntityObjMap>,
    mut villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    mut event_executing_query: Query<&mut EventExecuting>,
    last_combat_tick_query: Query<&LastCombatTick>,
    mut query: Query<(&Actor, &mut ActionState, &Eat, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _eat, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);

        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Eat action requested");
                });

                if actor_is_combat_locked(*actor, game_tick.0, &last_combat_tick_query) {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot eat while in combat");
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                let Ok(mut villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find villager");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(food_item) = villager.inventory.get_by_class(FOOD.to_owned()) else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot find food item");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                *villager.state = State::Eating;

                commands.trigger(StateChange {
                    entity: *actor,
                    new_state: State::Eating,
                });

                let eat_event = VisibleEvent::EatEvent {
                    item_id: food_item.id,
                    obj_id: villager.id.0,
                };

                map_events.new(
                    villager.id.0,
                    game_tick.0 + TICKS_PER_SEC * 3, // in the future
                    eat_event,
                );

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::Executing;

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if actor_is_combat_locked(*actor, game_tick.0, &last_combat_tick_query) {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cancelling eat for combat lock");
                    });
                    if let Ok(mut event_executing) = event_executing_query.get_mut(*actor) {
                        event_executing.state = EventExecutingState::None;
                    }
                    commands.trigger(CancelEvents { entity: *actor });
                    *state = ActionState::Failure;
                    continue;
                }

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                if event_executing.state != EventExecutingState::Completed {
                    span.span().in_scope(|| {
                        villager_trace!(*actor, obj_id, None, "Eat Event still executing");
                    });
                    continue;
                }

                event_executing.state = EventExecutingState::None;
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling Eat action");
                });

                let Ok(mut villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let event_type = GameEventType::CancelAllMapEvents {
                    obj_id: villager.id.0,
                };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn find_shelter_system(
    mut commands: Commands,
    mut ids: ResMut<Ids>,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    mut villager_query: Query<(&Id, &Position, &mut ActiveShelter), With<SubclassVillager>>,
    structure_query: Query<&Position, (With<ClassStructure>, Without<SubclassVillager>)>,
    mut event_executing_query: Query<&mut EventExecuting>,
    exhausted: Query<&Exhausted>,
    last_combat_tick_query: Query<&LastCombatTick>,
    mut morale_query: Query<&mut Morale>,
    mut action_query: Query<(&Actor, &mut ActionState, &FindShelter, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _find_shelter_action, span) in &mut action_query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                let Ok((villager_id, _villager_pos, _active_shelter)) = villager_query.get(*actor)
                else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    continue;
                };

                // Create find event
                map_events.new(
                    villager_id.0,
                    game_tick.0 + FIND_SHELTER_TICKS,
                    VisibleEvent::FindShelterEvent {
                        obj_id: villager_id.0,
                    },
                );

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::Executing;

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                if event_executing.state != EventExecutingState::Completed {
                    span.span().in_scope(|| {
                        villager_trace!(*actor, obj_id, None, "Find Shelter still executing");
                    });
                    continue;
                }

                let Ok((villager_id, villager_pos, mut active_shelter)) =
                    villager_query.get_mut(*actor)
                else {
                    span.span().in_scope(|| {
                        villager_trace!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    continue;
                };

                if active_shelter.0 == NO_SHELTER {
                    if exhausted.get(*actor).is_ok() {
                        if actor_is_combat_locked(*actor, game_tick.0, &last_combat_tick_query) {
                            span.span().in_scope(|| {
                                villager_debug!(
                                    *actor,
                                    obj_id,
                                    None,
                                    "Cannot rest without shelter while in combat"
                                );
                            });
                            *state = ActionState::Failure;
                            continue;
                        }

                        commands
                            .entity(*actor)
                            .insert(Destination { pos: *villager_pos });

                        if let Ok(mut morale) = morale_query.get_mut(*actor) {
                            morale.add_rough_sleep_penalty(ROUGH_SLEEP_MORALE_PENALTY);
                        }

                        *state = ActionState::Success;
                    } else {
                        *state = ActionState::Failure;
                    }
                } else {
                    let Some(shelter_entity) = entity_map.get_entity(active_shelter.0) else {
                        span.span().in_scope(|| {
                            villager_error!(
                                *actor,
                                obj_id,
                                None,
                                "Cannot find shelter entity for shelter_id={}",
                                active_shelter.0
                            );
                        });
                        continue;
                    };

                    if let Ok(shelter_pos) = structure_query.get(shelter_entity) {
                        commands
                            .entity(*actor)
                            .insert(Destination { pos: *shelter_pos });

                        *state = ActionState::Success;
                    } else {
                        span.span().in_scope(|| {
                            villager_error!(
                                *actor,
                                obj_id,
                                None,
                                "Cannot find shelter {:?}",
                                shelter_entity
                            );
                        });
                        *state = ActionState::Failure;
                    }
                }
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling find shelter");
                });
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::None;

                let Ok((villager_id, villager_pos, _active_shelter)) = villager_query.get(*actor)
                else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let event_type = GameEventType::CancelAllMapEvents {
                    obj_id: villager_id.0,
                };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure
            }
            _ => {}
        }
    }
}

pub fn sleep_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    protection: VillagerProtection,
    mut ids: ResMut<Ids>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    entity_map: Res<EntityObjMap>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut villager_query: Query<VillagerQuery, With<SubclassVillager>>,
    last_combat_tick_query: Query<&LastCombatTick>,
    mut query: Query<(&Actor, &mut ActionState, &Sleep, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _sleep, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);

        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Sleep action requested");
                });

                if actor_is_combat_locked(*actor, game_tick.0, &last_combat_tick_query) {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot sleep while in combat");
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                let Ok(mut villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find villager");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                *villager.state = State::Sleeping;

                commands.trigger(StateChange {
                    entity: *actor,
                    new_state: State::Sleeping,
                });

                map_events.new(
                    villager.id.0,
                    game_tick.0 + 50, // in the future
                    VisibleEvent::SleepEvent {
                        obj_id: villager.id.0,
                    },
                );

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::Executing;

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if actor_is_combat_locked(*actor, game_tick.0, &last_combat_tick_query) {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cancelling sleep for combat lock");
                    });
                    if let Ok(mut event_executing) = event_executing_query.get_mut(*actor) {
                        event_executing.state = EventExecutingState::None;
                    }
                    commands.trigger(CancelEvents { entity: *actor });
                    *state = ActionState::Failure;
                    continue;
                }

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                if event_executing.state != EventExecutingState::Completed {
                    span.span().in_scope(|| {
                        villager_trace!(*actor, obj_id, None, "Sleep Event still executing");
                    });
                    continue;
                }

                // Reset EventExecutingState back to none
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Sleep Event completed");
                });
                event_executing.state = EventExecutingState::None;

                *state = ActionState::Success;
            }
            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::None;

                // Reset activity
                let Ok(mut villager) = villager_query.get_mut(*actor) else {
                    span.span().in_scope(|| {
                        villager_debug!(*actor, obj_id, None, "Cannot get villager query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let event_type = GameEventType::CancelAllMapEvents {
                    obj_id: villager.id.0,
                };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_storage_destination_system(
    mut commands: Commands,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    map: Res<Map>,
    _templates: Res<Templates>,
    (obj_query, mut query): (
        Query<BaseQuery>,
        Query<(
            &Actor,
            &mut ActionState,
            &SetStorageDestination,
            &ActionSpan,
        )>,
    ),
) {
    for (Actor(actor), mut state, _set_storage_destination, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Some(obj_id_val) = obj_id else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, None, None, "Cannot find obj id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(villager_player_id) = ids.get_player(obj_id_val) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find player id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(villager) = obj_query.get(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find obj");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let mut nearest_storage_dist = 10000 as u32;
                let mut nearest_storage_id = None;
                let mut nearest_storage_pos = None;

                for structure in obj_query.iter() {
                    // Skip if player_id of villager and structure are not matching
                    if villager_player_id != structure.player_id.0 {
                        continue;
                    }

                    // Check if the structure is a shelter
                    if *structure.subclass != Subclass::Storage {
                        continue;
                    }

                    if *structure.state != State::None {
                        continue;
                    }

                    let Some(path_result) = Map::find_fast_path(
                        *villager.pos,
                        *structure.pos,
                        &map,
                        villager_player_id,
                        Vec::new(),
                        true,
                        false,
                        false,
                        false,
                        true,
                    ) else {
                        span.span().in_scope(|| {
                            villager_trace!(*actor, obj_id, None, "No path found to structure");
                        });
                        continue;
                    };

                    span.span().in_scope(|| {
                        villager_trace!(
                            *actor,
                            obj_id,
                            None,
                            "Path to structure, cost={}",
                            path_result.1
                        );
                    });

                    let (path, c) = path_result;

                    if nearest_storage_dist > c {
                        nearest_storage_dist = c;
                        nearest_storage_id = Some(structure.id.0);
                        nearest_storage_pos = Some(*structure.pos);
                    }
                }

                span.span().in_scope(|| {
                    villager_debug!(
                        *actor,
                        obj_id,
                        None,
                        "Nearest storage id={:?} pos={:?}",
                        nearest_storage_id,
                        nearest_storage_pos
                    );
                });

                if let (Some(storage_id), Some(storage_pos)) =
                    (nearest_storage_id, nearest_storage_pos)
                {
                    commands
                        .entity(*actor)
                        .insert(Destination { pos: storage_pos });

                    commands.entity(*actor).insert(Storage { id: storage_id });

                    *state = ActionState::Success;
                } else {
                    *state = ActionState::Failure;
                }
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Cancelling set storage destination");
                });
                *state = ActionState::Failure
            }
            _ => {}
        }
    }
}

pub fn load_items_system(
    mut commands: Commands,
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    villager_query: Query<&Assignment>,
    mut query: Query<(&Actor, &mut ActionState, &LoadItems, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _load_items, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Load items requested");
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Loading items");
                });

                let Ok(assignment) = villager_query.get(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find assignment");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(structure_entity) = entity_map.get_entity(assignment.structure_id) else {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Cannot find structure entity for structure_id={}",
                            assignment.structure_id
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                commands.trigger(TransferAllResources {
                    entity: structure_entity,
                    target_entity: *actor,
                });

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn unload_items_system(
    entity_map: Res<EntityObjMap>,
    protection: VillagerProtection,
    mut inventory_query: Query<(&PlayerId, &Position, &mut Inventory)>,
    storage_query: Query<&Storage>,
    mut query: Query<(&Actor, &mut ActionState, &UnloadItems, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _unload_items, span) in &mut query {
        if protection.is_protected(*actor) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Unload items requested");
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                span.span().in_scope(|| {
                    villager_debug!(*actor, obj_id, None, "Unloading items");
                });

                let Ok(storage) = storage_query.get(*actor) else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find storage component");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                // Get storage entity
                let Some(storage_entity) = entity_map.get_entity(storage.id) else {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Cannot find storage entity for storage_id={}",
                            storage.id
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(
                    [(villager_player_id, villager_pos, mut villager_inventory), (storage_player_id, storage_pos, mut storage_inventory)],
                ) = inventory_query.get_many_mut([*actor, storage_entity])
                else {
                    span.span().in_scope(|| {
                        villager_error!(*actor, obj_id, None, "Cannot find inventories");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if villager and storage player_id are matching
                if villager_player_id.0 != storage_player_id.0 {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Villager and storage player_id not matching"
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                // Check if villager and storage are on the same position
                if villager_pos != storage_pos {
                    span.span().in_scope(|| {
                        villager_error!(
                            *actor,
                            obj_id,
                            None,
                            "Villager and storage not on same position"
                        );
                    });
                    *state = ActionState::Failure;
                    continue;
                }

                // Equipped gear belongs to the villager, even during bulk unload.
                Inventory::transfer_all_unequipped_items(
                    &mut villager_inventory,
                    &mut storage_inventory,
                );

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

fn active_task_is_current_action(
    active_task: &ActiveTask,
    state: &State,
    order: Option<&Order>,
    inventory: &Inventory,
    tool_fetch_target: Option<&ToolFetchTarget>,
) -> bool {
    match active_task {
        ActiveTask::None | ActiveTask::Idle | ActiveTask::Unknown => false,
        ActiveTask::MovingToGatherPos if tool_fetch_target.is_some() => false,
        _ => {
            if let Some(Order::Gather { res_type, .. }) = order {
                let gather_task = ActiveTask::get_activity_from_res_type(res_type.clone());
                if *active_task == gather_task {
                    return gather_active_task_for_display(
                        state,
                        res_type,
                        inventory,
                        None,
                        tool_fetch_target,
                    )
                    .map(|task| task == *active_task)
                    .unwrap_or(false);
                }
            }

            true
        }
    }
}

pub fn villager_activity_text(
    active_task: &ActiveTask,
    state: &State,
    order: Option<&Order>,
    inventory: &Inventory,
    blocked_work: Option<&BlockedWork>,
    tool_fetch_target: Option<&ToolFetchTarget>,
) -> String {
    if active_task_is_current_action(active_task, state, order, inventory, tool_fetch_target) {
        return active_task.to_string();
    }

    if let Some(tool_fetch_target) = tool_fetch_target {
        return format!(
            "Fetching {} tool",
            item::tool_attr_label(&tool_fetch_target.required_attr)
        );
    }

    if let Some(blocked_work) = blocked_work {
        return blocked_work.reason.clone();
    }

    if let Some(Order::Gather { res_type, .. }) = order {
        let gather_task = ActiveTask::get_activity_from_res_type(res_type.clone());
        if *active_task == gather_task {
            return ActiveTask::Unknown.to_string();
        }
    }

    active_task.to_string()
}

pub fn activity_update_system(
    clients: Res<Clients>,
    active_infos: Res<ActiveInfos>,
    protection: VillagerProtection,
    changed_query: Query<
        (
            Entity,
            &Id,
            &ActiveTask,
            &State,
            Option<&Order>,
            &Inventory,
            Option<&BlockedWork>,
            Option<&ToolFetchTarget>,
        ),
        (
            With<SubclassVillager>,
            Or<(
                Changed<ActiveTask>,
                Changed<State>,
                Changed<Order>,
                Changed<Inventory>,
                Changed<BlockedWork>,
                Changed<ToolFetchTarget>,
            )>,
        ),
    >,
    query: Query<
        (
            &Id,
            &ActiveTask,
            &State,
            Option<&Order>,
            &Inventory,
            Option<&BlockedWork>,
            Option<&ToolFetchTarget>,
        ),
        With<SubclassVillager>,
    >,
    mut removed_blocked_work: RemovedComponents<BlockedWork>,
    mut removed_tool_fetch_target: RemovedComponents<ToolFetchTarget>,
) {
    let mut changed_entities = HashSet::new();

    for (entity, _, _, _, _, _, _, _) in changed_query.iter() {
        changed_entities.insert(entity);
    }

    for entity in removed_blocked_work.read() {
        changed_entities.insert(entity);
    }

    for entity in removed_tool_fetch_target.read() {
        changed_entities.insert(entity);
    }

    for entity in changed_entities {
        if protection.is_protected(entity) {
            continue;
        }

        let Ok((id, active_task, state, order, inventory, blocked_work, tool_fetch_target)) =
            query.get(entity)
        else {
            continue;
        };

        let Some(active_info_players) = active_infos.get(&(id.0, ActiveInfoType::Obj)) else {
            continue;
        };

        for player_id in active_info_players {
            let response_packet = ResponsePacket::InfoActivityUpdate {
                id: id.0,
                activity: villager_activity_text(
                    active_task,
                    state,
                    order,
                    inventory,
                    blocked_work,
                    tool_fetch_target,
                ),
            };

            debug!("Activity sending to client {:?}", response_packet);

            send_to_client(*player_id, response_packet, &clients);
        }
    }
}

fn find_item_location_by_class(
    villager_player_id: i32,
    villager_pos: &Position,
    villager_inventory: &Inventory,
    structure_query: &Query<
        (&Id, &PlayerId, &Position, &Inventory),
        (With<ClassStructure>, Without<SubclassVillager>),
    >,
    item_class: String,
    map: &Res<Map>,
) -> Option<(ItemLocation, Item, Position)> {
    let mut nearest_source_dist = 10000 as u32;
    let mut nearest_item = None;
    let mut nearest_pos = None;

    // T3.6: for food, villagers prefer the lowest-Feed item (eat cheap
    // forage before high-value prepared meals). For other classes, fall
    // back to the first match.
    let pick_item = |inv: &Inventory| -> Option<Item> {
        if item_class == "Food" {
            inv.get_food_to_eat()
        } else {
            inv.get_by_class(item_class.clone())
        }
    };

    // Check if the villager has any items of the given class
    if let Some(item) = pick_item(villager_inventory) {
        return Some((ItemLocation::Own, item.clone(), *villager_pos));
    }

    // Check if the structures have any items of the given class
    for (id, player_id, pos, inventory) in structure_query.iter() {
        if player_id.0 == villager_player_id {
            let Some(item) = pick_item(inventory) else {
                debug!(
                    "Structure does not have any items of class {:?}",
                    item_class
                );
                continue;
            };

            let Some(path_result) = Map::find_fast_path(
                *villager_pos,
                *pos,
                &map,
                villager_player_id,
                Vec::new(),
                true,
                false,
                false,
                false,
                true,
            ) else {
                debug!("Not path found to structure...");
                continue;
            };

            let (_path, c) = path_result;
            debug!("Path count: {:?}", c);

            if nearest_source_dist > c {
                nearest_source_dist = c;
                nearest_item = Some(item.clone());
                nearest_pos = Some(*pos);
            }
        }
    }

    if let (Some(nearest_item), Some(nearest_pos)) = (nearest_item, nearest_pos) {
        return Some((
            ItemLocation::OwnStructure,
            nearest_item.clone(),
            nearest_pos.clone(),
        ));
    } else {
        return None;
    }
}

pub fn dialogue_system(
    game_tick: Res<GameTick>,
    templates: Res<Templates>,
    mut map_events: ResMut<MapEvents>,
    protection: VillagerProtection,
    dialogue_query: Query<(Entity, &Id, &Dialogue)>,
) {
    for (entity, id, dialogue) in dialogue_query.iter() {
        if protection.is_protected(entity) {
            continue;
        }

        if game_tick.0 >= dialogue.at_tick {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("NoDrinks"),
                id,
                &mut map_events,
            );
        }
    }
}

pub fn vital_dialogue_system(
    game_tick: Res<GameTick>,
    templates: Res<Templates>,
    mut map_events: ResMut<MapEvents>,
    protection: VillagerProtection,
    dehydrated: Query<(Entity, &Id, &Dehydrated), Without<StateDead>>,
    starving: Query<(Entity, &Id, &Starving), Without<StateDead>>,
    exhausted: Query<(Entity, &Id, &Exhausted), Without<StateDead>>,
) {
    for (entity, id, dehydrated) in dehydrated.iter() {
        if protection.is_protected(entity) {
            continue;
        }

        if game_tick.0 == dehydrated.at_tick + 5 {
            // Add 5 ticks for delay
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("DehydratedLevel1"),
                id,
                &mut map_events,
            );
        } else if game_tick.0 == dehydrated.at_tick + DEHYDRATED_WARNING1_AT {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("DehydratedLevel2"),
                id,
                &mut map_events,
            );
        } else if game_tick.0 == dehydrated.at_tick + DEHYDRATED_WARNING2_AT {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("DehydratedLevel3"),
                id,
                &mut map_events,
            );
        } else if game_tick.0 == dehydrated.at_tick + DEHYDRATED_DEATH_AT - 20 {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("DehydratedDying"),
                id,
                &mut map_events,
            );
        }
    }

    for (entity, id, starving) in starving.iter() {
        if protection.is_protected(entity) {
            continue;
        }

        if game_tick.0 == starving.at_tick + 5 {
            // Add 5 ticks for delay
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("StarvingLevel1"),
                id,
                &mut map_events,
            );
        } else if game_tick.0 == starving.at_tick + STARVING_WARNING1_AT {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("StarvingLevel2"),
                id,
                &mut map_events,
            );
        } else if game_tick.0 == starving.at_tick + STARVING_WARNING2_AT {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("StarvingLevel3"),
                id,
                &mut map_events,
            );
        } else if game_tick.0 == starving.at_tick + STARVING_DEATH_AT - 20 {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("StarvingDying"),
                id,
                &mut map_events,
            );
        }
    }

    for (entity, id, exhausted) in exhausted.iter() {
        if protection.is_protected(entity) {
            continue;
        }

        if game_tick.0 == exhausted.at_tick + 5 {
            // Add 5 ticks for delay
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("ExhaustedLevel1"),
                id,
                &mut map_events,
            );
        } else if game_tick.0 == exhausted.at_tick + EXHAUSTED_WARNING1_AT {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("ExhaustedLevel2"),
                id,
                &mut map_events,
            );
        } else if game_tick.0 == exhausted.at_tick + EXHAUSTED_WARNING2_AT {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("ExhaustedLevel3"),
                id,
                &mut map_events,
            );
        } else if game_tick.0 == exhausted.at_tick + EXHAUSTED_DEATH_AT - 20 {
            Obj::add_speech_event(
                game_tick.0,
                templates.get_dialogue("ExhaustedDying"),
                id,
                &mut map_events,
            );
        }
    }
}

pub fn remove_no_drinks_system(
    mut commands: Commands,
    no_drink_query: Query<(Entity, &NoDrinks)>,
    game_tick: Res<GameTick>,
    protection: VillagerProtection,
) {
    for (entity, no_drink) in no_drink_query.iter() {
        if protection.is_protected(entity) {
            continue;
        }

        if game_tick.0 > no_drink.at_tick + TICKS_PER_SEC * 10 {
            commands.entity(entity).remove::<NoDrinks>();
        }
    }
}

pub fn remove_no_food_system(
    mut commands: Commands,
    no_food_query: Query<(Entity, &NoFood)>,
    game_tick: Res<GameTick>,
    protection: VillagerProtection,
) {
    for (entity, no_food) in no_food_query.iter() {
        if protection.is_protected(entity) {
            continue;
        }

        if game_tick.0 > no_food.at_tick + TICKS_PER_SEC * 10 {
            commands.entity(entity).remove::<NoFood>();
        }
    }
}

pub fn active_task_system(
    protection: VillagerProtection,
    mut villager_queries: ParamSet<(
        Query<(Entity, &mut ActiveTask), With<SubclassVillager>>,
        Query<&ActiveTask, With<SubclassVillager>>,
    )>,
    order_query: Query<
        (
            &State,
            &Order,
            &Inventory,
            Option<&BlockedWork>,
            Option<&ToolFetchTarget>,
        ),
        With<SubclassVillager>,
    >,
    actions: Query<ActiveTaskActionQuery>,
) {
    // Best task per villager: (task priority, action state priority, deterministic tiebreaker, task)
    let mut best: HashMap<Entity, (i32, i32, i32, ActiveTask)> = HashMap::new();

    for action in &actions {
        let actor = action.actor.0;

        if protection.is_protected(actor) {
            continue;
        }

        // Inline ranking: Executing > Requested > everything else
        let state_rank = match *action.state {
            ActionState::Executing => 2,
            ActionState::Requested => 1,
            _ => 0,
        };

        if state_rank == 0 {
            continue;
        }

        let previous_task = villager_queries
            .p1()
            .get(actor)
            .cloned()
            .unwrap_or(ActiveTask::Idle);
        let order_state = order_query.get(actor).ok();

        let Some(task) = visible_activity_for_action(&action, previous_task, order_state) else {
            continue;
        };

        let priority = active_task_priority(&task);
        let tiebreaker = active_task_tiebreaker(&task);

        match best.get(&actor) {
            Some((best_priority, best_state_rank, best_tiebreaker, _))
                if (*best_priority, *best_state_rank, *best_tiebreaker)
                    >= (priority, state_rank, tiebreaker) => {}
            _ => {
                best.insert(actor, (priority, state_rank, tiebreaker, task));
            }
        }
    }

    for (villager_entity, mut active_task) in &mut villager_queries.p0() {
        if protection.is_protected(villager_entity) {
            continue;
        }

        if let Some((_, _, _, next_task)) = best.remove(&villager_entity) {
            let previous_task = (*active_task).clone();
            if ActiveTask::set_if_changed(&mut active_task, next_task.clone()) {
                debug!(
                    "Active Task for {:?}: {:?} -> {:?}",
                    villager_entity, previous_task, next_task
                );
            }
        }
    }
}

pub fn clear_event_executing(
    protection: VillagerProtection,
    mut event_executing_query: Query<&mut EventExecuting>,
    actions: Query<(&Actor, &ActionState)>,
) {
    let mut actor_actions: HashMap<Entity, (bool, bool)> = HashMap::new();

    for (Actor(actor), action_state) in &actions {
        match *action_state {
            ActionState::Requested | ActionState::Executing => {
                actor_actions.entry(*actor).or_insert((false, false)).1 = true;
            }
            ActionState::Failure | ActionState::Cancelled | ActionState::Success => {
                actor_actions.entry(*actor).or_insert((false, false)).0 = true;
            }
            _ => {}
        }
    }

    for (actor, (has_terminal_action, has_active_action)) in actor_actions {
        if protection.is_protected(actor) {
            continue;
        }

        if !has_terminal_action || has_active_action {
            continue;
        }

        if let Ok(mut event_executing) = event_executing_query.get_mut(actor) {
            if event_executing.state != EventExecutingState::None {
                debug!("Clearing EventExecuting for {:?}", actor);
                event_executing.state = EventExecutingState::None;
            }
        }
    }
}

#[cfg(test)]
mod protected_simulation_tests {
    use super::*;
    use big_brain::{actions::spawn_action, scorers::spawn_scorer};

    use crate::safe_logout::{PlayerPresenceRecord, PlayerWorldPresence, PlayerWorldPresenceState};

    #[test]
    fn checkpoint2_protected_villager_score_action_and_active_task_stay_unchanged() {
        let mut app = App::new();
        app.add_systems(Update, (morale_scorer_system, idle_action_system));
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));

        let mut protected_record = PlayerPresenceRecord::new(false);
        protected_record.state = PlayerWorldPresence::OfflineProtected;
        let mut presence = PlayerWorldPresenceState::default();
        presence.players.insert(1, protected_record);
        app.world_mut().insert_resource(presence);

        let actor = app
            .world_mut()
            .spawn((
                PlayerId(1),
                Morale::new(50.0),
                Order::None,
                ActiveTask::Fleeing,
            ))
            .id();

        let scorer = {
            let mut commands = app.world_mut().commands();
            spawn_scorer(&GoodMorale, &mut commands, actor)
        };
        let action = {
            let mut commands = app.world_mut().commands();
            spawn_action(
                &Idle {
                    start_time: 0,
                    duration: 1,
                },
                &mut commands,
                actor,
            )
        };
        app.world_mut().flush();
        app.world_mut()
            .entity_mut(scorer)
            .get_mut::<Score>()
            .unwrap()
            .set(0.37);
        *app.world_mut()
            .entity_mut(action)
            .get_mut::<ActionState>()
            .unwrap() = ActionState::Requested;

        app.update();

        assert_eq!(
            app.world().entity(scorer).get::<Score>().unwrap().get(),
            0.37
        );
        assert_eq!(
            *app.world().entity(action).get::<ActionState>().unwrap(),
            ActionState::Requested
        );
        assert_eq!(
            *app.world().entity(actor).get::<ActiveTask>().unwrap(),
            ActiveTask::Fleeing
        );
    }
}

#[cfg(test)]
#[path = "villager_tests.rs"]
mod tests;
