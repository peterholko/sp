use bevy::ecs::query::{QueryData, WorldQuery};
use bevy::prelude::*;

use std::collections::HashMap;

use crate::combat::CombatQuery;
use crate::constants::*;
use crate::effect::{Effect, Effects};
use crate::event::{MapEvents, VisibleEvent};
use crate::game::{GameTick, ObjQueryMut};
use crate::ids::{EntityObjMap, Ids};
use crate::item::{AttrKey, AttrVal, Inventory, Item, Items, Slot};
use crate::resource::Resource;

use crate::map::MapPos;

use crate::templates::{ItemTemplate, ObjTemplate, ObjTemplates, Templates};
use crate::world::time_of_day_vision_mod;

#[derive(Debug, Reflect, Component, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[reflect(Component)]
pub struct Id(pub i32);

#[derive(Debug, Reflect, Component, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[reflect(Component)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Reflect, Component, Default, Clone, PartialEq, Eq, Hash)]
#[reflect(Component)]
pub struct PlayerId(pub i32);

impl PlayerId {
    pub fn is_human(&self) -> bool {
        self.0 < MAX_PLAYER_ID
    }

    pub fn is_npc(&self) -> bool {
        self.0 >= NPC_PLAYER_ID
    }
}

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct Name(pub String);

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct Template(pub String);

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct Class(pub String);

#[derive(Debug, Reflect, Component, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[reflect(Component)]
pub enum HeroClass {
    #[default]
    Warrior,
    Ranger,
    Mage,
}

impl HeroClass {
    pub fn from_str(class_name: &str) -> Option<Self> {
        match class_name {
            "Warrior" | "Novice Warrior" => Some(HeroClass::Warrior),
            "Ranger" | "Novice Ranger" => Some(HeroClass::Ranger),
            "Mage" | "Novice Mage" => Some(HeroClass::Mage),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            HeroClass::Warrior => "Warrior",
            HeroClass::Ranger => "Ranger",
            HeroClass::Mage => "Mage",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HeroClassProfile {
    pub hero_class: HeroClass,
    pub label: &'static str,
    pub novice_template: &'static str,
    pub base_mana: i32,
    pub ability_ids: &'static [&'static str],
    pub selection_hint: &'static str,
}

impl HeroClassProfile {
    pub fn for_class(hero_class: HeroClass) -> Self {
        match hero_class {
            HeroClass::Warrior => HeroClassProfile {
                hero_class,
                label: "Warrior",
                novice_template: "Novice Warrior",
                base_mana: 0,
                ability_ids: &["shield_bash"],
                selection_hint: "Survives pressure, braces, and finishes adjacent fights.",
            },
            HeroClass::Ranger => HeroClassProfile {
                hero_class,
                label: "Ranger",
                novice_template: "Novice Ranger",
                base_mana: 0,
                ability_ids: &["aimed_shot", "disengage"],
                selection_hint:
                    "Scouts farther, shoots from range, and slips away before being surrounded.",
            },
            HeroClass::Mage => HeroClassProfile {
                hero_class,
                label: "Mage",
                novice_template: "Novice Mage",
                base_mana: 100,
                ability_ids: &["arcane_bolt", "ward"],
                selection_hint: "Spends mana on bolts and wards, then recovers through rest.",
            },
        }
    }
}

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct EndRepeatAction;

impl Class {
    pub fn is_structure(&self) -> bool {
        return self.0 == CLASS_STRUCTURE.to_string();
    }

    pub fn is_poi(&self) -> bool {
        return self.0 == CLASS_POI.to_string();
    }

    pub fn is_corpse(&self) -> bool {
        return self.0 == CLASS_CORPSE.to_string();
    }

    pub fn is_blocking(&self) -> bool {
        return self.0 == CLASS_STRUCTURE.to_string() || self.0 == CLASS_UNIT.to_string();
    }
}

#[derive(Debug, Reflect, Component, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[reflect(Component)]
pub enum Subclass {
    #[default]
    None,
    Hero,
    Villager,
    Craft,
    Shelter,
    Farm,
    Wall,
    Monolith,
    Poi,
    Merchant,
    Transport,
    Corpse,
    Campfire,
    Storage,
    Npc,
    Watchtower,
    Resource,
}

impl Subclass {
    pub fn from_str(s: &str) -> Self {
        match s {
            SUBCLASS_HERO => Subclass::Hero,
            SUBCLASS_VILLAGER => Subclass::Villager,
            SUBCLASS_CRAFT => Subclass::Craft,
            SUBCLASS_SHELTER => Subclass::Shelter,
            SUBCLASS_FARM => Subclass::Farm,
            SUBCLASS_WALL => Subclass::Wall,
            SUBCLASS_MONOLITH => Subclass::Monolith,
            SUBCLASS_POI => Subclass::Poi,
            SUBCLASS_MERCHANT => Subclass::Merchant,
            SUBCLASS_TRANSPORT => Subclass::Transport,
            SUBCLASS_CORPSE => Subclass::Corpse,
            SUBCLASS_CAMPFIRE => Subclass::Campfire,
            SUBCLASS_STORAGE => Subclass::Storage,
            SUBCLASS_NPC => Subclass::Npc,
            SUBCLASS_WATCHTOWER => Subclass::Watchtower,
            SUBCLASS_RESOURCE => Subclass::Resource,
            _ => Subclass::None,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Subclass::Hero => SUBCLASS_HERO.to_string(),
            Subclass::Villager => SUBCLASS_VILLAGER.to_string(),
            Subclass::Craft => SUBCLASS_CRAFT.to_string(),
            Subclass::Shelter => SUBCLASS_SHELTER.to_string(),
            Subclass::Farm => SUBCLASS_FARM.to_string(),
            Subclass::Wall => SUBCLASS_WALL.to_string(),
            Subclass::Monolith => SUBCLASS_MONOLITH.to_string(),
            Subclass::Poi => SUBCLASS_POI.to_string(),
            Subclass::Merchant => SUBCLASS_MERCHANT.to_string(),
            Subclass::Transport => SUBCLASS_TRANSPORT.to_string(),
            Subclass::Corpse => SUBCLASS_CORPSE.to_string(),
            Subclass::Campfire => SUBCLASS_CAMPFIRE.to_string(),
            Subclass::Storage => SUBCLASS_STORAGE.to_string(),
            Subclass::Npc => SUBCLASS_NPC.to_string(),
            Subclass::Watchtower => SUBCLASS_WATCHTOWER.to_string(),
            Subclass::Resource => SUBCLASS_RESOURCE.to_string(),
            Subclass::None => "none".to_string(),
        }
    }

    pub fn is_monolith(&self) -> bool {
        *self == Subclass::Monolith
    }

    pub fn is_merchant(&self) -> bool {
        *self == Subclass::Merchant
    }

    pub fn is_hero(&self) -> bool {
        *self == Subclass::Hero
    }

    pub fn is_villager(&self) -> bool {
        *self == Subclass::Villager
    }

    pub fn is_resource(&self) -> bool {
        *self == Subclass::Resource
    }

    pub fn is_storage(&self) -> bool {
        *self == Subclass::Storage
    }

    pub fn is_shelter(&self) -> bool {
        *self == Subclass::Shelter
    }

    pub fn is_watchtower(&self) -> bool {
        *self == Subclass::Watchtower
    }

    pub fn is_npc(&self) -> bool {
        *self == Subclass::Npc
    }
}

#[derive(Debug, Reflect, Component, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[reflect(Component)]
pub enum State {
    #[default]
    None,
    Dead,
    Moving,
    Founded,
    Progressing,
    Building,
    PlanningUpgrade,
    Upgrading,
    Stalled,
    Gathering,
    Refining,
    Operating,
    Mining,
    Lumberjacking,
    Crafting,
    Exploring,
    Surveying,
    Prospecting,
    Investigating,
    Experimenting,
    Planting,
    Harvesting,
    Drinking,
    Eating,
    Sleeping,
    Aboard,
    Casting,
    Hiding,
    Repairing,
    Burning,
    Fishing,
}

impl State {
    pub fn is_blocking(&self) -> bool {
        match self {
            State::Dead => false,
            State::Founded => false,
            State::Progressing => false,
            State::Building => false,
            State::Stalled => false,
            State::Hiding => false,
            _ => true,
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            State::Dead => false,
            State::Founded => false,
            State::Progressing => false,
            State::Building => false,
            State::PlanningUpgrade => false,
            State::Upgrading => false,
            State::Stalled => false,
            _ => true,
        }
    }

    pub fn is_visible(&self) -> bool {
        match self {
            State::Hiding => false,
            _ => true,
        }
    }

    pub fn is_alive(&self) -> bool {
        match self {
            State::Dead => false,
            _ => true,
        }
    }

    pub fn is_dead(&self) -> bool {
        match self {
            State::Dead => true,
            _ => false,
        }
    }

    pub fn is_founded_building_or_upgrading(&self) -> bool {
        match self {
            State::Founded => true,
            State::Building => true,
            State::Upgrading => true,
            _ => false,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            State::Dead => STATE_DEAD.to_string(),
            State::Moving => STATE_MOVING.to_string(),
            State::Founded => STATE_FOUNDED.to_string(),
            State::Progressing => STATE_PROGRESSING.to_string(),
            State::Building => STATE_BUILDING.to_string(),
            State::PlanningUpgrade => STATE_PLANNING_UPGRADE.to_string(),
            State::Upgrading => STATE_UPGRADING.to_string(),
            State::Stalled => STATE_STALLED.to_string(),
            State::Gathering => STATE_GATHERING.to_string(),
            State::Refining => STATE_REFINING.to_string(),
            State::Operating => STATE_OPERATING.to_string(),
            State::Mining => STATE_MINING.to_string(),
            State::Lumberjacking => STATE_LUMBERJACKING.to_string(),
            State::Crafting => STATE_CRAFTING.to_string(),
            State::Exploring => STATE_EXPLORING.to_string(),
            State::Surveying => STATE_SURVEYING.to_string(),
            State::Prospecting => STATE_PROSPECTING.to_string(),
            State::Investigating => STATE_INVESTIGATING.to_string(),
            State::Experimenting => STATE_EXPERIMENTING.to_string(),
            State::Planting => STATE_PLANTING.to_string(),
            State::Harvesting => STATE_HARVESTING.to_string(),
            State::Drinking => STATE_DRINKING.to_string(),
            State::Eating => STATE_EATING.to_string(),
            State::Sleeping => STATE_SLEEPING.to_string(),
            State::Aboard => STATE_ABOARD.to_string(),
            State::Casting => STATE_CASTING.to_string(),
            State::Hiding => STATE_HIDING.to_string(),
            State::Repairing => STATE_REPAIRING.to_string(),
            State::Burning => STATE_BURNING.to_string(),
            State::Fishing => STATE_FISHING.to_string(),
            State::None => STATE_NONE.to_string(),
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct BaseAttrs {
    pub creativity: i32,
    pub dexterity: i32,
    pub endurance: i32,
    pub focus: i32,
    pub intellect: i32,
    pub spirit: i32,
    pub strength: i32,
    pub toughness: i32,
}

#[derive(Debug, Component, Clone, PartialEq)]
pub enum Personality {
    Brave,
    Diligent,
    Lazy,
    Greedy,
    Loyal,
    Curious,
}

impl Personality {
    pub fn to_str(&self) -> &str {
        match self {
            Personality::Brave => "Brave",
            Personality::Diligent => "Diligent",
            Personality::Lazy => "Lazy",
            Personality::Greedy => "Greedy",
            Personality::Loyal => "Loyal",
            Personality::Curious => "Curious",
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct LastCombatTick(pub i32);

pub const COMBAT_LOCK_TICKS: i32 = 3 * TICKS_PER_SEC;

pub fn is_combat_locked(game_tick: i32, last_combat_tick: &LastCombatTick) -> bool {
    game_tick.saturating_sub(last_combat_tick.0) < COMBAT_LOCK_TICKS
}

pub fn is_peaceful_interruptible_state(state: &State) -> bool {
    matches!(
        state,
        State::Building
            | State::Gathering
            | State::Refining
            | State::Operating
            | State::Mining
            | State::Lumberjacking
            | State::Crafting
            | State::Drinking
            | State::Eating
            | State::Sleeping
            | State::Fishing
    )
}

impl Default for LastCombatTick {
    fn default() -> Self {
        LastCombatTick(-1000)
    }
}

#[derive(Debug, Component, Clone)]
pub struct LastAttacker {
    pub id: i32,
    pub tick: i32,
}

#[derive(Debug, Component, Clone)]
pub struct Stats {
    pub hp: i32,
    pub stamina: Option<i32>,
    pub mana: Option<i32>,
    pub base_hp: i32,
    pub base_stamina: Option<i32>,
    pub base_mana: Option<i32>,
    pub base_def: i32,
    pub damage_range: Option<i32>,
    pub base_damage: Option<i32>,
    pub base_speed: Option<i32>,
    pub base_vision: Option<u32>,
}

impl Stats {
    pub fn get_strength(&self) -> i32 {
        let damage = self.base_damage.unwrap_or(0);
        let damage_range = self.damage_range.unwrap_or(0);
        let speed = self.base_speed.unwrap_or(0);

        let score = self.hp * self.base_def + damage * damage_range + speed;

        return score;
    }
}

#[derive(Debug, Component, Clone, Default, Eq, PartialEq)]
pub enum ActiveTask {
    #[default]
    None, // None is absolutely nothing vs Idle is an action
    Idle,
    GettingDrink,
    GettingFood,
    FindingShelter,
    Eating,
    Drinking,
    Sleeping,
    Fleeing,
    FightingBack,
    Following,
    Building,
    Gathering,
    Operating,
    Mining,
    Hunting,
    Woodcutting,
    Stonecutting,
    Refining,
    Crafting,
    Experimenting,
    Exploring,
    Planting,
    Tending,
    Harvesting,
    Repairing,
    Unloading,
    MovingToGatherPos,
    MovingToOperatePos,
    MovingToRefinePos,
    MovingToCraftPos,
    MovingToExperimentPos,
    MovingToExplorePos,
    MovingToFoodPos,
    MovingToDrinkPos,
    MovingToShelterPos,
    Unknown,
}

impl ActiveTask {
    pub fn set_if_changed(current: &mut Mut<ActiveTask>, next: ActiveTask) -> bool {
        if **current == next {
            return false;
        }

        **current = next;
        true
    }

    pub fn to_string(&self) -> String {
        let str = match self {
            ActiveTask::None => "None",
            ActiveTask::Idle => "Idle",
            ActiveTask::Following => "Following",
            ActiveTask::GettingDrink => "Getting a drink",
            ActiveTask::Drinking => "Drinking",
            ActiveTask::GettingFood => "Getting some food",
            ActiveTask::Eating => "Eating",
            ActiveTask::FindingShelter => "Finding shelter",
            ActiveTask::Sleeping => "Sleeping",
            ActiveTask::Fleeing => "Fleeing",
            ActiveTask::FightingBack => "Fighting back",
            ActiveTask::Building => "Building",
            ActiveTask::Gathering => "Gathering",
            ActiveTask::Operating => "Operating",
            ActiveTask::Mining => "Mining",
            ActiveTask::Hunting => "Hunting",
            ActiveTask::Woodcutting => "Woodcutting",
            ActiveTask::Stonecutting => "Stonecutting",
            ActiveTask::Refining => "Refining",
            ActiveTask::Crafting => "Crafting",
            ActiveTask::Experimenting => "Experimenting",
            ActiveTask::Exploring => "Prospecting",
            ActiveTask::Planting => "Planting",
            ActiveTask::Tending => "Tending",
            ActiveTask::Harvesting => "Harvesting",
            ActiveTask::Repairing => "Repairing",
            ActiveTask::Unloading => "Unloading",
            ActiveTask::MovingToGatherPos => "Moving to gather",
            ActiveTask::MovingToOperatePos => "Moving to operate",
            ActiveTask::MovingToRefinePos => "Moving to refine",
            ActiveTask::MovingToCraftPos => "Moving to craft",
            ActiveTask::MovingToExperimentPos => "Moving to experiment",
            ActiveTask::MovingToExplorePos => "Moving to prospect",
            ActiveTask::MovingToFoodPos => "Moving to food",
            ActiveTask::MovingToDrinkPos => "Moving to drink",
            ActiveTask::MovingToShelterPos => "Moving to shelter",
            ActiveTask::Unknown => "Unknown",
        };

        return str.to_string();
    }

    pub fn get_activity_from_string(activity: String) -> ActiveTask {
        match activity.as_str() {
            "Mining" => ActiveTask::Mining,
            "Hunting" => ActiveTask::Hunting,
            "Woodcutting" => ActiveTask::Woodcutting,
            "Stonecutting" => ActiveTask::Stonecutting,
            "Refining" => ActiveTask::Refining,
            "Crafting" => ActiveTask::Crafting,
            "Operating" => ActiveTask::Operating,
            "Planting" => ActiveTask::Planting,
            "Tending" => ActiveTask::Tending,
            "Harvesting" => ActiveTask::Harvesting,
            "Unloading" => ActiveTask::Unloading,
            _ => ActiveTask::Unknown,
        }
    }

    pub fn get_activity_from_res_type(res_type: String) -> ActiveTask {
        let activity_str = Resource::type_to_skill(res_type);
        return Self::get_activity_from_string(activity_str);
    }
}

#[derive(Debug, Component, Clone)]
pub struct ActiveShelter(pub i32);

#[derive(Debug, Component, Clone)]
pub struct StateBuilding;

#[derive(Debug, Component, Clone)]
pub struct StateUpgrading;

#[derive(Debug, Component, Clone)]
pub struct StateDead {
    pub dead_at: i32,
    pub killer: String,
}

#[derive(Debug, Component, Clone)]
pub struct TrueDeath {
    pub true_death_at: i32,
}

#[derive(Debug, Component, Clone)]
pub struct NotActive;

#[derive(Debug, Component, Clone)]
pub struct StateAboard {
    pub transport_id: i32,
}

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct Sheltered {
    pub id: i32,
}

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct Viewshed {
    pub range: u32,
}

#[derive(Debug, Component)]
pub struct SubclassHero; //Subclass Hero

#[derive(Debug, Component)]
pub struct SubclassVillager; //Subclass Villager

#[derive(Debug, Component)]
pub struct SubclassNPC; //Subclass NPC

#[derive(Debug, Component)]
pub struct ClassStructure; //Class Structure

#[derive(Debug, Component)]
pub struct ClassCorpse; //Class Corpse

#[derive(Debug, Component)]
pub struct AI;

#[derive(Debug, Reflect, Component, Default, Clone)]
#[reflect(Component)]
pub struct Misc {
    pub image: String,
    pub hsl: Vec<i32>,
    pub groups: Vec<String>,
}

#[derive(Debug, Component, Clone)]
pub struct Assignment {
    pub structure_id: i32,
    pub structure_name: String,
    pub structure_pos: Position,
}

#[derive(Debug, Component, Clone)]
pub struct Assignments(pub Vec<i32>); // List of Ids

#[derive(Debug, Component, Clone)]
pub struct SelectedUpgrade(pub String);

#[derive(EntityEvent)]
pub struct StateChange {
    pub entity: Entity,
    pub new_state: State,
}

#[derive(EntityEvent)]
pub struct TemplateChange {
    pub entity: Entity,
    pub new_template: String,
}

#[derive(EntityEvent)]
pub struct NewObj {
    pub entity: Entity,
}

#[derive(EntityEvent)]
pub struct RemoveObj {
    pub entity: Entity,
}

#[derive(EntityEvent)]
pub struct UpdateObj {
    pub entity: Entity,
    pub attrs: Vec<(String, String)>,
}

#[derive(EntityEvent)]
pub struct StartBuild {
    pub entity: Entity,
    pub builder_entity: Entity,
}

#[derive(EntityEvent)]
pub struct StartUpgrade {
    pub entity: Entity,
    pub builder_entity: Entity,
}

#[derive(EntityEvent)]
pub struct StartWork {
    pub entity: Entity,
    pub worker_id: i32,
    pub structure_id: i32,
}

#[derive(EntityEvent)]
pub struct TransferAllResources {
    pub entity: Entity,
    pub target_entity: Entity,
}

#[derive(EntityEvent)]
pub struct FoodPoisoningEffect {
    pub entity: Entity,
    pub food_poisoning_attr: AttrVal,
}

#[derive(EntityEvent)]
pub struct BuildProgressUpdate {
    pub entity: Entity,
}

#[derive(EntityEvent)]
pub struct AddLightEffect {
    pub entity: Entity,
    pub effect: Effect,
}

#[derive(EntityEvent)]
pub struct RemoveLightEffect {
    pub entity: Entity,
    pub effect: Effect,
}

#[derive(EntityEvent)]
pub struct RemoveWorker {
    pub entity: Entity,
    pub worker_id: i32,
    pub structure_id: i32,
}

#[derive(EntityEvent)]
pub struct CancelEvents {
    pub entity: Entity,
}

#[derive(Debug, Component)]
pub struct BuildUpgradeState {
    pub build_upgrade_cost: f32,
    pub work_done: f32,
    pub work_per_sec: f32,
}

#[derive(Debug, Component, Clone)]
pub struct WorkQueue(pub Vec<WorkEntry>);

#[derive(Debug, Clone)]

pub struct WorkEntry {
    pub worker_id: i32,
    pub work_type: WorkType,
    pub work_status: WorkStatus,
    pub recipe_name: Option<String>,
    pub recipe_image: Option<String>,
    pub refine_item_id: Option<i32>,
    pub refine_item_image: Option<String>,
    pub refine_item_class: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkType {
    Build,
    Craft,
    Refine,
    Experiment,
    Operate,
}

impl ToString for WorkType {
    fn to_string(&self) -> String {
        match self {
            WorkType::Build => "Build".to_string(),
            WorkType::Craft => "Craft".to_string(),
            WorkType::Refine => "Refine".to_string(),
            WorkType::Experiment => "Experiment".to_string(),
            WorkType::Operate => "Operate".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkStatus {
    Idle,
    InProgress,
    Completed,
}

impl ToString for WorkStatus {
    fn to_string(&self) -> String {
        match self {
            WorkStatus::Idle => "Idle".to_string(),
            WorkStatus::InProgress => "InProgress".to_string(),
            WorkStatus::Completed => "Completed".to_string(),
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct Shelter {
    pub max_residents: i32,  // Max number of residents
    pub residents: Vec<i32>, // Villager entities assigned to this shelter
}

#[derive(Debug, Component, Clone)]
pub struct Campfire {
    pub is_lit: bool,
    pub lit_at: i32,
    pub duration: i32,
}

#[derive(Debug, Component, Clone)]
pub struct Storage;

#[derive(Debug, Component, Clone)]
pub struct Watchtower;

#[derive(Debug, Component, Clone)]
pub struct NPCAttrs {
    pub target: i32,
}

#[derive(Debug, Component, Eq, PartialEq)]
pub enum Order {
    None,
    Follow {
        target: Entity,
    },
    Build,
    Gather {
        res_type: String,
        pos: Position,
        storage_pos: Option<Position>,
        storage_id: Option<i32>,
    },
    WorkQueue,
    Operate,
    Explore,
    Plant,
    Tend,
    Harvest,
    Repair,
}

impl Order {
    pub fn to_string(&self) -> String {
        match self {
            Order::None => format!("None"),
            Order::Follow { target: _ } => format!("Follow"),
            Order::Gather {
                res_type: _,
                pos: _,
                storage_pos: _,
                storage_id: _,
            } => format!("Gather"),
            Order::Build => format!("Build"),
            Order::Operate => format!("Operate"),
            Order::Explore => format!("Prospect"),
            Order::Plant => format!("Plant"),
            Order::Tend => format!("Tend"),
            Order::Harvest => format!("Harvest"),
            Order::Repair => format!("Repair"),
            Order::WorkQueue => format!("Work Queue"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Blocker {
    pub player_id: PlayerId,
    pub id: Id,
    pub pos: Position,
    pub class: Class,
    pub subclass: Subclass,
    pub state: State,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum HeroClassList {
    Warrior,
    Ranger,
    Mage,
    None,
}

#[derive(QueryData)]
#[query_data(derive(Debug))]
pub struct BaseQuery {
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub state: &'static State,
    pub inventory: &'static Inventory,
}

#[derive(QueryData)]
#[query_data(derive(Debug))]
pub struct BaseQueryEffects {
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub state: &'static State,
    pub effects: &'static Effects,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct BaseQueryMutState {
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub state: &'static mut State,
    pub effects: &'static Effects,
    pub template: &'static Template,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct ObjStatQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub template: &'static Template,
    pub state: &'static mut State,
    pub misc: &'static mut Misc,
    pub stats: &'static mut Stats,
    pub effects: &'static mut Effects,
}

#[derive(Bundle, Clone)]
pub struct Obj {
    pub id: Id,
    pub player_id: PlayerId,
    pub position: Position,
    pub name: Name,
    pub template: Template,
    pub class: Class,
    pub subclass: Subclass,
    pub state: State,
    pub misc: Misc,
    pub stats: Stats,
    pub effects: Effects,
    pub inventory: Inventory,
    pub last_combat_tick: LastCombatTick,
}

impl Obj {
    pub fn create(
        player_id: i32,
        template_name: String,
        pos: Position,
        state: State,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        map_events: &mut ResMut<MapEvents>,
        game_tick: &Res<GameTick>,
        templates: &Res<Templates>,
    ) -> (i32, Entity) {
        let template = templates.obj_templates.get(template_name);
        let obj_id = ids.new_obj_id();

        let obj = Obj {
            id: Id(obj_id),
            player_id: PlayerId(player_id),
            position: pos,
            name: Name(template.template.clone()),
            template: Template(template.template.clone()),
            class: Class(template.class),
            subclass: Subclass::from_str(&template.subclass),
            state: state,
            misc: Misc {
                image: template.image,
                hsl: Vec::new(),
                groups: Vec::new(),
            },
            stats: Stats {
                hp: template.base_hp.unwrap_or(100),
                base_hp: template.base_hp.unwrap_or(100),
                stamina: template.base_stamina,
                mana: template.base_mana.filter(|base_mana| *base_mana > 0),
                base_stamina: template.base_stamina,
                base_mana: template.base_mana.filter(|base_mana| *base_mana > 0),
                base_def: template.base_def.unwrap_or(0),
                base_damage: template.base_dmg,
                damage_range: template.dmg_range,
                base_speed: template.base_speed,
                base_vision: template.base_vision,
            },
            effects: Effects(HashMap::new()),
            inventory: Inventory {
                owner: obj_id,
                items: Vec::new(),
            },
            last_combat_tick: LastCombatTick::default(),
        };

        let entity_id;

        if let Some(vision) = template.base_vision {
            // Spawn entity
            entity_id = commands.spawn((obj, Viewshed { range: vision })).id();
        } else {
            entity_id = commands.spawn(obj).id();
        }

        // Create mappings
        ids.new_obj(obj_id, player_id);
        entity_map.new_obj(obj_id, entity_id);

        // Create a new object event
        commands.trigger(NewObj { entity: entity_id });

        (obj_id, entity_id)
    }

    pub fn create_nospawn(
        obj_id: i32,
        player_id: i32,
        template_name: String,
        pos: Position,
        state: State,
        inventory: Inventory,
        templates: &Res<Templates>,
    ) -> Obj {
        let template = templates.obj_templates.get(template_name);

        let mut groups = Vec::new();

        if let Some(template_groups) = &template.groups {
            groups = template_groups.clone();
        }

        let obj = Obj {
            id: Id(obj_id),
            player_id: PlayerId(player_id),
            position: pos,
            name: Name(template.template.clone()),
            template: Template(template.template.clone()),
            class: Class(template.class),
            subclass: Subclass::from_str(&template.subclass),
            state: state,
            misc: Misc {
                image: template.image,
                hsl: Vec::new(),
                groups: groups,
            },
            stats: Stats {
                hp: template.base_hp.unwrap_or(100),
                base_hp: template.base_hp.unwrap_or(100),
                stamina: template.base_stamina,
                mana: template.base_mana.filter(|base_mana| *base_mana > 0),
                base_stamina: template.base_stamina,
                base_mana: template.base_mana.filter(|base_mana| *base_mana > 0),
                base_def: template.base_def.unwrap_or(0),
                base_damage: template.base_dmg,
                damage_range: template.dmg_range,
                base_speed: template.base_speed,
                base_vision: template.base_vision,
            },
            effects: Effects(HashMap::new()),
            inventory: inventory,
            last_combat_tick: LastCombatTick::default(),
        };

        return obj;
    }

    pub fn state_to_enum(state: String) -> State {
        match state.as_str() {
            STATE_NONE => State::None,
            STATE_MOVING => State::Moving,
            STATE_DEAD => State::Dead,
            STATE_FOUNDED => State::Founded,
            STATE_PROGRESSING => State::Progressing,
            STATE_BUILDING => State::Building,
            STATE_PLANNING_UPGRADE => State::PlanningUpgrade,
            STATE_UPGRADING => State::Upgrading,
            STATE_STALLED => State::Stalled,
            STATE_GATHERING => State::Gathering,
            STATE_REFINING => State::Refining,
            STATE_OPERATING => State::Operating,
            STATE_MINING => State::Mining,
            STATE_LUMBERJACKING => State::Lumberjacking,
            STATE_CRAFTING => State::Crafting,
            STATE_EXPLORING => State::Exploring,
            STATE_SURVEYING => State::Surveying,
            STATE_PROSPECTING => State::Prospecting,
            STATE_INVESTIGATING => State::Investigating,
            STATE_DRINKING => State::Drinking,
            STATE_EATING => State::Eating,
            STATE_SLEEPING => State::Sleeping,
            STATE_CASTING => State::Casting,
            STATE_HIDING => State::Hiding,
            _ => State::None,
        }
    }

    pub fn state_to_str(state: State) -> String {
        let state_string = match state {
            State::None => STATE_NONE,
            State::Moving => STATE_MOVING,
            State::Dead => STATE_DEAD,
            State::Founded => STATE_FOUNDED,
            State::Progressing => STATE_PROGRESSING,
            State::Building => STATE_BUILDING,
            State::PlanningUpgrade => STATE_PLANNING_UPGRADE,
            State::Upgrading => STATE_UPGRADING,
            State::Stalled => STATE_STALLED,
            State::Gathering => STATE_GATHERING,
            State::Refining => STATE_REFINING,
            State::Operating => STATE_OPERATING,
            State::Mining => STATE_MINING,
            State::Lumberjacking => STATE_LUMBERJACKING,
            State::Crafting => STATE_CRAFTING,
            State::Exploring => STATE_EXPLORING,
            State::Surveying => STATE_SURVEYING,
            State::Prospecting => STATE_PROSPECTING,
            State::Investigating => STATE_INVESTIGATING,
            State::Drinking => STATE_DRINKING,
            State::Eating => STATE_EATING,
            State::Sleeping => STATE_SLEEPING,
            State::Casting => STATE_CASTING,
            State::Hiding => STATE_HIDING,
            State::Burning => STATE_BURNING,
            _ => STATE_NONE,
        };

        return state_string.to_string();
    }

    pub fn is_dead(obj_state: &State) -> bool {
        return *obj_state == State::Dead;
    }

    pub fn get_capacity(template: &String, obj_templates: &ObjTemplates) -> i32 {
        for obj_template in obj_templates.iter() {
            if obj_template.template == *template {
                if let Some(capacity) = obj_template.capacity {
                    return capacity;
                } else {
                    info!(
                        "No capacity found for obj template: {:?} defaulting to 0",
                        template
                    );
                    return 0;
                }
            }
        }

        info!("No template found for {:?}", template);

        return 0;
    }

    /*pub fn get_colliding_and_all_objs(
        player_id: i32,
        dst: Position,
        query: &Query<ObjQueryMut>,
    ) -> (bool, Vec<(PlayerId, Id, Position)>, Vec<network::MapObj>) {
        // Check if destination is open
        let mut is_dst_open = true;
        let mut colliding_objs: Vec<(PlayerId, Id, Position)> = Vec::new();
        let mut all_map_objs: Vec<network::MapObj> = Vec::new();

        //TODO Move this logic to another function
        for obj in query.iter() {
            debug!(
                "entity: {:?} id: {:?} player_id: {:?} pos: {:?}",
                obj.entity, obj.id, obj.player_id, obj.pos
            );
            if (player_id != obj.player_id.0)
                && obj.pos.x == dst.x
                && obj.pos.y == dst.y
                && obj.state.is_blocking()
            {
                is_dst_open = false;
            }

            colliding_objs.push((obj.player_id.clone(), obj.id.clone(), obj.pos.clone()));
            all_map_objs.push(network::to_map_obj_mut(obj));
        }

        return (is_dst_open, colliding_objs, all_map_objs);
    }*/

    // Revisit consolidation of these functions based on different world queries
    pub fn blocking_list_objstatquery(player_id: i32, query: &Query<ObjStatQuery>) -> Vec<Blocker> {
        let mut collision_list: Vec<Blocker> = Vec::new();

        for obj in query.iter() {
            if player_id != obj.player_id.0 && obj.state.is_blocking() {
                collision_list.push(Blocker {
                    player_id: obj.player_id.clone(),
                    id: obj.id.clone(),
                    pos: obj.pos.clone(),
                    class: obj.class.clone(),
                    subclass: obj.subclass.clone(),
                    state: obj.state.clone(),
                });
            }
        }

        return collision_list;
    }

    pub fn blocking_list_combatquery(
        player_id: i32,
        query: &Query<CombatQuery, Without<SubclassNPC>>,
    ) -> Vec<Blocker> {
        let mut collision_list: Vec<Blocker> = Vec::new();

        for obj in query.iter() {
            if player_id != obj.player_id.0 && obj.state.is_blocking() {
                collision_list.push(Blocker {
                    player_id: obj.player_id.clone(),
                    id: obj.id.clone(),
                    pos: obj.pos.clone(),
                    class: obj.class.clone(),
                    subclass: obj.subclass.clone(),
                    state: obj.state.clone(),
                });
            }
        }

        return collision_list;
    }

    pub fn blocking_list(
        player_id: i32,
        entity: &Entity,
        query: &Query<(&Id, &PlayerId, &Position, &Class, &Subclass, &Stats)>,
        state_query: &Query<&mut State>,
    ) -> Vec<Blocker> {
        let mut collision_list: Vec<Blocker> = Vec::new();

        for (obj_id, obj_player_id, obj_pos, obj_class, obj_subclass, _obj_stats) in query.iter() {
            if let Ok(state) = state_query.get(*entity) {
                if player_id != obj_player_id.0 && state.is_blocking() {
                    let blocker = Blocker {
                        player_id: obj_player_id.clone(),
                        id: obj_id.clone(),
                        pos: obj_pos.clone(),
                        class: obj_class.clone(),
                        subclass: obj_subclass.clone(),
                        state: state.clone(),
                    };

                    collision_list.push(blocker);
                }
            }
        }

        return collision_list;
    }

    pub fn blocking_list_basequery(player_id: i32, query: &Query<BaseQuery>) -> Vec<Blocker> {
        let mut collision_list: Vec<Blocker> = Vec::new();

        for obj in query.iter() {
            if player_id != obj.player_id.0 && obj.state.is_blocking() && obj.class.is_blocking() {
                let blocker = Blocker {
                    player_id: obj.player_id.clone(),
                    id: obj.id.clone(),
                    pos: obj.pos.clone(),
                    class: obj.class.clone(),
                    subclass: obj.subclass.clone(),
                    state: obj.state.clone(),
                };

                collision_list.push(blocker);
            }
        }

        return collision_list;
    }

    pub fn blocking_list_basequery_npc(
        player_id: i32,
        query: &Query<BaseQuery, Without<SubclassNPC>>,
    ) -> Vec<Blocker> {
        let mut collision_list: Vec<Blocker> = Vec::new();

        for obj in query.iter() {
            if player_id != obj.player_id.0 && obj.state.is_blocking() && obj.class.is_blocking() {
                let blocker = Blocker {
                    player_id: obj.player_id.clone(),
                    id: obj.id.clone(),
                    pos: obj.pos.clone(),
                    class: obj.class.clone(),
                    subclass: obj.subclass.clone(),
                    state: obj.state.clone(),
                };

                collision_list.push(blocker);
            }
        }

        return collision_list;
    }

    pub fn monolith_list(query: &Query<CombatQuery, Without<SubclassNPC>>) -> Vec<MapPos> {
        let mut monolith_list: Vec<MapPos> = Vec::new();

        for obj in query.iter() {
            if obj.subclass.is_monolith() {
                monolith_list.push(MapPos(obj.pos.x, obj.pos.y));
            }
        }

        return monolith_list;
    }

    pub fn add_speech_event(
        game_tick: i32,
        speech: String,
        obj_id: &Id,
        map_events: &mut ResMut<MapEvents>,
    ) {
        let sound_event = VisibleEvent::SpeechEvent {
            speech: speech,
            intensity: 2,
        };

        map_events.new(obj_id.0, game_tick, sound_event);
    }

    pub fn generate_hero_attrs() -> BaseAttrs {
        let attrs = BaseAttrs {
            creativity: 10,
            dexterity: 10,
            endurance: 10,
            focus: 10,
            intellect: 10,
            spirit: 10,
            strength: 10,
            toughness: 10,
        };

        return attrs;
    }

    pub fn is_visible(state: State) -> bool {
        match state {
            //State::Aboard => false,
            State::Hiding => false,
            _ => true,
        }
    }

    pub fn is_subclass(subclass_name: &String, subclass: &String) -> bool {
        subclass_name == subclass
    }

    pub fn has_group(group_name: &str, groups: Vec<String>) -> bool {
        for group in groups {
            if group == group_name.to_string() {
                return true;
            }
        }

        return false;
    }

    pub fn template_to_image(template: &String) -> String {
        let image = template
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        return image;
    }

    pub fn set_viewshed_range(
        owner: i32,
        template: String,
        game_tick: i32,
        inventory: &Inventory,
        templates: &Templates,
        vision_modifier: f32,
    ) -> u32 {
        let template = templates.get_obj_template_by_name(template);
        let remainder = game_tick % GAME_TICKS_PER_DAY;
        let is_night = remainder >= NIGHT || remainder < FIRST_LIGHT;

        let base_vision = template.base_vision.unwrap_or(0) as f32;
        info!("Base vision: {:?}", base_vision);
        let time_mod = time_of_day_vision_mod(game_tick);
        info!("Time mod: {:?}", time_mod);
        info!("Is night: {:?}", is_night);
        let item_vision_mod = if is_night {
            inventory.get_items_value_by_attr(&AttrKey::Vision, true) as f32
        } else {
            0.0
        };
        info!("Item vision mod: {:?}", item_vision_mod);
        info!("Vision modifier: {:?}", vision_modifier);
        let vision = (base_vision * time_mod + item_vision_mod + vision_modifier)
            .floor()
            .max(0.0) as u32;
        info!("Vision: {:?}", vision);
        return vision;
    }

    pub fn construction_skill_multiplier(
        base_work: i32,
        construction_skill: i32,
        carpentry_skill: i32,
        masonry_skill: i32,
    ) -> f32 {
        let c = construction_skill as f32;
        let m = masonry_skill as f32;
        let ca = carpentry_skill as f32;

        let multiplier = 1.0 + 0.10 * c + 0.06 * m + 0.06 * ca;
        (base_work as f32) * multiplier
    }
}

#[cfg(test)]
mod tests {
    use super::ActiveTask;

    #[test]
    fn active_task_labels_cover_common_villager_panel_states() {
        let cases = [
            (ActiveTask::None, "None"),
            (ActiveTask::Idle, "Idle"),
            (ActiveTask::GettingDrink, "Getting a drink"),
            (ActiveTask::GettingFood, "Getting some food"),
            (ActiveTask::Sleeping, "Sleeping"),
            (ActiveTask::FindingShelter, "Finding shelter"),
            (ActiveTask::FightingBack, "Fighting back"),
            (ActiveTask::Following, "Following"),
            (ActiveTask::Building, "Building"),
            (ActiveTask::Mining, "Mining"),
            (ActiveTask::Woodcutting, "Woodcutting"),
            (ActiveTask::Stonecutting, "Stonecutting"),
            (ActiveTask::Refining, "Refining"),
            (ActiveTask::Crafting, "Crafting"),
            (ActiveTask::Experimenting, "Experimenting"),
            (ActiveTask::Exploring, "Prospecting"),
            (ActiveTask::Planting, "Planting"),
            (ActiveTask::Tending, "Tending"),
            (ActiveTask::Harvesting, "Harvesting"),
            (ActiveTask::Repairing, "Repairing"),
            (ActiveTask::Unloading, "Unloading"),
            (ActiveTask::MovingToGatherPos, "Moving to gather"),
            (ActiveTask::MovingToDrinkPos, "Moving to drink"),
            (ActiveTask::MovingToShelterPos, "Moving to shelter"),
        ];

        for (task, label) in cases {
            assert_eq!(task.to_string(), label);
        }
    }
}
