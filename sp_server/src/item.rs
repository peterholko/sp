use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::slice::Iter;

use crate::constants::{self, *};
use crate::effect::Effect;
use crate::ids::Ids;
use crate::network;
use crate::recipe::Recipe;
use crate::templates::{ItemTemplate, ResReq, Templates};

use crate::constants::CONTAINER;

#[derive(Debug, Reflect, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttrKey {
    Rarity,
    Affixes,
    Damage,
    Defense,
    Speed,
    Durability,
    Feed,
    Healing,
    Thirst,
    Equipable,
    Consumable,
    DeepWoundChance,
    BleedChance,
    ConcussedChance,
    DisarmedChance,
    AllAttributes,
    Creativity,
    Dexterity,
    Endurance,
    Focus,
    Intellect,
    Spirit,
    Strength,
    Toughness,
    AxeDamage,
    SwordDamage,
    HammerDamage,
    DaggerDamage,
    SpearDamage,
    AxeSpeed,
    BowDamage,
    AttackRange,
    Accuracy,
    HeavyArmorDefense,
    HeavyArmorDurability,
    MediumArmorDefense,
    MediumArmorDurabilility,
    StructureHp,
    StructureDefense,
    Vision,
    Duration,
    Mining,
    Logging,
    Farming,
    Hunting,
    Fishing,
    Timberworking,
    Stonecutting,
    Refining,
    Crafting,
    Experimenting,
    Exploring,
    Planting,
    Tending,
    Harvesting,
    Foraging,
    Repairing,
    Butchery,
    Cooking,
    FoodPoisoning,
}

impl AttrKey {
    pub fn proc_iter() -> Iter<'static, AttrKey> {
        static PROC_ATTR_KEYS: [AttrKey; 4] = [
            AttrKey::DeepWoundChance,
            AttrKey::BleedChance,
            AttrKey::ConcussedChance,
            AttrKey::DisarmedChance,
        ];
        PROC_ATTR_KEYS.iter()
    }

    pub fn proc_to_effect(self) -> Effect {
        match self {
            AttrKey::DeepWoundChance => Effect::DeepWound,
            AttrKey::BleedChance => Effect::Bleed,
            AttrKey::ConcussedChance => Effect::Concussed,
            AttrKey::DisarmedChance => Effect::Disarmed,
            _ => panic!("Invalid Proc AttrKey, could not find Effect"),
        }
    }

    pub fn str_to_key(val: String) -> AttrKey {
        match val.as_str() {
            "Rarity" => AttrKey::Rarity,
            "Affixes" => AttrKey::Affixes,
            "Damage" => AttrKey::Damage,
            "Defense" => AttrKey::Defense,
            "Speed" => AttrKey::Speed,
            "Durability" => AttrKey::Durability,
            "Feed" => AttrKey::Feed,
            "Healing" => AttrKey::Healing,
            "Thirst" => AttrKey::Thirst,
            "Equipable" => AttrKey::Equipable,
            "Consumable" => AttrKey::Consumable,
            "Deep Wound Chance" => AttrKey::DeepWoundChance,
            "Bleed Chance" => AttrKey::BleedChance,
            "Concussed Chance" => AttrKey::ConcussedChance,
            "Disarmed Chance" => AttrKey::DisarmedChance,
            "All Attributes" => AttrKey::AllAttributes,
            "Creativity" => AttrKey::Creativity,
            "Dexterity" => AttrKey::Dexterity,
            "Endurance" => AttrKey::Endurance,
            "Focus" => AttrKey::Focus,
            "Intellect" => AttrKey::Intellect,
            "Spirit" => AttrKey::Spirit,
            "Strength" => AttrKey::Strength,
            "Toughness" => AttrKey::Toughness,
            "Axe Damage" => AttrKey::AxeDamage,
            "Sword Damage" => AttrKey::SwordDamage,
            "Hammer Damage" => AttrKey::HammerDamage,
            "Dagger Damage" => AttrKey::DaggerDamage,
            "Spear Damage" => AttrKey::SpearDamage,
            "Axe Speed" => AttrKey::AxeSpeed,
            "Bow Damage" => AttrKey::BowDamage,
            "Attack Range" => AttrKey::AttackRange,
            "Accuracy" => AttrKey::Accuracy,
            "Heavy Armor Defense" => AttrKey::HeavyArmorDefense,
            "Heavy Armor Durability" => AttrKey::HeavyArmorDurability,
            "Medium Armor Defense" => AttrKey::MediumArmorDefense,
            "Medium Armor Durability" => AttrKey::MediumArmorDurabilility,
            "Structure HP" => AttrKey::StructureHp,
            "Structure Defense" => AttrKey::StructureDefense,
            "Vision" => AttrKey::Vision,
            "Duration" => AttrKey::Duration,
            "Mining" => AttrKey::Mining,
            "Logging" => AttrKey::Logging,
            "Farming" => AttrKey::Farming,
            "Hunting" => AttrKey::Hunting,
            "Fishing" => AttrKey::Fishing,
            "Timberworking" => AttrKey::Timberworking,
            "Stonecutting" => AttrKey::Stonecutting,
            "Refining" => AttrKey::Refining,
            "Crafting" => AttrKey::Crafting,
            "Experimenting" => AttrKey::Experimenting,
            "Exploring" => AttrKey::Exploring,
            "Planting" => AttrKey::Planting,
            "Tending" => AttrKey::Tending,
            "Harvesting" => AttrKey::Harvesting,
            "Foraging" => AttrKey::Foraging,
            "Repairing" => AttrKey::Repairing,
            "Butchery" => AttrKey::Butchery,
            "Cooking" => AttrKey::Cooking,
            "Food Poisoning" => AttrKey::FoodPoisoning,
            _ => AttrKey::AllAttributes,
        }
    }
}

#[derive(Debug, Reflect, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttrVal {
    Num(f32),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Reflect, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Magic,
    Rare,
}

impl ItemRarity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Common => "Common",
            Self::Uncommon => "Uncommon",
            Self::Magic => "Magic",
            Self::Rare => "Rare",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Common => 0,
            Self::Uncommon => 1,
            Self::Magic => 2,
            Self::Rare => 3,
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "Uncommon" => Self::Uncommon,
            "Magic" => Self::Magic,
            "Rare" => Self::Rare,
            _ => Self::Common,
        }
    }

    /// Roll a bounded component rarity from a 0..1000 roll. Stronger enemies
    /// improve the odds, but Common remains the most likely result at every
    /// currently supported danger tier.
    pub fn from_loot_roll(danger_score: i32, roll: u16) -> Self {
        let (uncommon_start, magic_start, rare_start) = if danger_score <= 55 {
            (850, 985, 999)
        } else if danger_score <= 160 {
            (780, 960, 995)
        } else {
            (600, 880, 980)
        };

        if roll >= rare_start {
            Self::Rare
        } else if roll >= magic_start {
            Self::Magic
        } else if roll >= uncommon_start {
            Self::Uncommon
        } else {
            Self::Common
        }
    }
}

#[derive(Clone, Copy)]
struct ComponentAffix {
    name: &'static str,
    key: AttrKey,
}

fn component_affix_pool(item_template: &ItemTemplate) -> Vec<ComponentAffix> {
    let affixes = match item_template.class.as_str() {
        "Hide" | "Leather" | "Game Animal" => vec![
            ComponentAffix {
                name: "Stout",
                key: AttrKey::Defense,
            },
            ComponentAffix {
                name: "Tracker's",
                key: AttrKey::Hunting,
            },
            ComponentAffix {
                name: "Fanged",
                key: AttrKey::Damage,
            },
        ],
        "Log" | "Timber" | "Stick" => vec![
            ComponentAffix {
                name: "Keen",
                key: AttrKey::Damage,
            },
            ComponentAffix {
                name: "Woodsman's",
                key: AttrKey::Logging,
            },
            ComponentAffix {
                name: "Hunter's",
                key: AttrKey::Hunting,
            },
        ],
        "Ore" | "Ingot" | "Dust" | "Stone" | "Block" | "Raw" => vec![
            ComponentAffix {
                name: "Keen",
                key: AttrKey::Damage,
            },
            ComponentAffix {
                name: "Stout",
                key: AttrKey::Defense,
            },
            ComponentAffix {
                name: "Miner's",
                key: AttrKey::Mining,
            },
        ],
        _ => Vec::new(),
    };

    affixes
}

fn affix_value_range(key: AttrKey, rarity: ItemRarity) -> (i32, i32) {
    match key {
        AttrKey::Damage | AttrKey::Defense => match rarity {
            ItemRarity::Uncommon => (1, 1),
            ItemRarity::Magic => (1, 2),
            ItemRarity::Rare => (2, 3),
            ItemRarity::Common => (0, 0),
        },
        _ => match rarity {
            ItemRarity::Uncommon => (1, 2),
            ItemRarity::Magic => (2, 3),
            ItemRarity::Rare => (3, 5),
            ItemRarity::Common => (0, 0),
        },
    }
}

/// Generate rarity and affix metadata for a component dropped by an enemy.
/// Non-component loot and Common drops return no metadata, so they remain
/// stack-compatible with the existing economy.
pub fn roll_loot_component_attrs<R: Rng + ?Sized>(
    item_template: &ItemTemplate,
    danger_score: i32,
    rng: &mut R,
) -> HashMap<AttrKey, AttrVal> {
    let mut attrs = HashMap::new();
    let mut pool = component_affix_pool(item_template);
    if pool.is_empty() {
        return attrs;
    }

    let rarity = ItemRarity::from_loot_roll(danger_score, rng.gen_range(0..1000));
    if rarity == ItemRarity::Common {
        return attrs;
    }

    let affix_count = rarity.rank() as usize;
    let mut names = Vec::with_capacity(affix_count);
    for _ in 0..affix_count.min(pool.len()) {
        let index = rng.gen_range(0..pool.len());
        let affix = pool.swap_remove(index);
        let (min, max) = affix_value_range(affix.key, rarity);
        attrs.insert(affix.key, AttrVal::Num(rng.gen_range(min..=max) as f32));
        names.push(affix.name);
    }

    attrs.insert(AttrKey::Rarity, AttrVal::Str(rarity.as_str().to_string()));
    attrs.insert(AttrKey::Affixes, AttrVal::Str(names.join(", ")));
    attrs
}

fn is_component_affix_attr(key: &AttrKey) -> bool {
    matches!(
        key,
        AttrKey::Rarity
            | AttrKey::Affixes
            | AttrKey::Damage
            | AttrKey::Defense
            | AttrKey::Logging
            | AttrKey::Hunting
            | AttrKey::Mining
    )
}

fn without_component_affixes(attrs: &HashMap<AttrKey, AttrVal>) -> HashMap<AttrKey, AttrVal> {
    attrs
        .iter()
        .filter(|(key, _)| !is_component_affix_attr(key))
        .map(|(key, value)| (*key, value.clone()))
        .collect()
}

pub const FILTER_ALL: &str = "all";

pub const _DAMAGE: &str = "Damage";
pub const _DEFENSE: &str = "Defense";

pub const _THIRST: &str = "Thirst";

pub const _FEED: &str = "Feed";
pub const GOLD: &str = "Gold Coins";
pub const SOULSHARD: &str = "Soulshard";
pub const SEEDS: &str = "Seeds";

pub const HARVESTING: &str = "Harvesting";

pub const ORE: &str = "Ore";
pub const LOG: &str = "Log";
pub const STONE: &str = "Stone";
pub const HIDE: &str = "Hide";

pub const FUEL: &str = "Fuel";
pub const FIREWOOD: &str = "Firewood";
pub const CHARCOAL: &str = "Charcoal";
pub const TATTERED_SHIRT: &str = "Tattered Shirt";
pub const TATTERED_PANTS: &str = "Tattered Pants";

pub const INGOT: &str = "Ingot";
pub const DUST: &str = "Dust";
pub const TIMBER: &str = "Timber";
pub const LOGS_OR_TIMBER: &str = "Logs or Timber";

pub fn req_matches(req_type: &str, item_name: &str, item_class: &str, item_subclass: &str) -> bool {
    req_type == item_name || req_type == item_class || req_type == item_subclass
}

/// Returns true if an item (described by name/class/subclass) satisfies a
/// structure requirement of the given type. Ordinary requirements match by
/// name, class, or subclass. The explicit `Logs or Timber` construction
/// requirement accepts either wood material at 1:1.
///
/// Used only for structure build/upgrade/upkeep checks. Recipe ingredient
/// matching uses strict equality (no substitution) to preserve recipe intent.
pub fn req_matches_build(
    req_type: &str,
    item_name: &str,
    item_class: &str,
    item_subclass: &str,
) -> bool {
    if req_type == LOGS_OR_TIMBER {
        return req_matches(LOG, item_name, item_class, item_subclass)
            || req_matches(TIMBER, item_name, item_class, item_subclass);
    }

    req_matches(req_type, item_name, item_class, item_subclass)
}

/// Attribute used by a tool that improves this gathering category. Some
/// categories, such as Forage, have an optional tool even though the action
/// remains available by hand.
pub fn gather_tool_attr_for_res_type(res_type: &str) -> Option<AttrKey> {
    match res_type {
        ORE => Some(AttrKey::Mining),
        LOG => Some(AttrKey::Logging),
        STONE => Some(AttrKey::Stonecutting),
        constants::FISH => Some(AttrKey::Fishing),
        constants::FOOD => Some(AttrKey::Farming),
        constants::FORAGE | constants::PLANT => Some(AttrKey::Foraging),
        constants::GAME_ANIMAL => Some(AttrKey::Hunting),
        _ => None,
    }
}

pub fn required_tool_attr_for_res_type(res_type: &str) -> Option<AttrKey> {
    match res_type {
        constants::FORAGE | constants::PLANT => None,
        _ => gather_tool_attr_for_res_type(res_type),
    }
}

pub fn gather_resource_type_for_tool(item: &Item) -> Option<&'static str> {
    if item.is_gather_tool_for_attr(&AttrKey::Mining) {
        Some(ORE)
    } else if item.is_gather_tool_for_attr(&AttrKey::Logging) {
        Some(LOG)
    } else if item.is_gather_tool_for_attr(&AttrKey::Stonecutting) {
        Some(STONE)
    } else if item.is_gather_tool_for_attr(&AttrKey::Fishing) {
        Some(constants::FISH)
    } else if item.is_gather_tool_for_attr(&AttrKey::Farming) {
        Some(constants::FOOD)
    } else if item.is_gather_tool_for_attr(&AttrKey::Foraging) {
        Some(constants::FORAGE)
    } else if item.is_gather_tool_for_attr(&AttrKey::Hunting) {
        Some(constants::GAME_ANIMAL)
    } else {
        None
    }
}

pub fn gather_duration_ticks(base_seconds: i32, tool_rating: f32) -> i32 {
    let rating = tool_rating.max(1.0);
    let speed_multiplier = (1.0 - 0.20 * (rating - 1.0)).max(0.40);
    ((base_seconds * TICKS_PER_SEC) as f32 * speed_multiplier).round() as i32
}

pub const FORAGING_TOOL_GATHER_TIME_SEC: i32 = 8;

pub fn gather_duration_ticks_for_res_type(
    base_seconds: i32,
    res_type: &str,
    tool_rating: Option<f32>,
) -> i32 {
    match tool_rating {
        Some(_) if matches!(res_type, constants::FORAGE | constants::PLANT) => {
            FORAGING_TOOL_GATHER_TIME_SEC * TICKS_PER_SEC
        }
        Some(rating) => gather_duration_ticks(base_seconds, rating),
        None => base_seconds * TICKS_PER_SEC,
    }
}

pub fn harvest_tool_break_chance(current_durability: i32, max_durability: i32) -> f32 {
    if current_durability <= 0 {
        1.0
    } else if max_durability <= 0 || current_durability * 5 > max_durability {
        0.0
    } else if current_durability * 10 <= max_durability {
        0.25
    } else {
        0.10
    }
}

pub fn tool_attr_label(attr: &AttrKey) -> &'static str {
    match attr {
        AttrKey::Mining => "Mining",
        AttrKey::Logging => "Logging",
        AttrKey::Stonecutting => "Stonecutting",
        AttrKey::Fishing => "Fishing",
        AttrKey::Farming => "Farming",
        AttrKey::Foraging => "Foraging",
        AttrKey::Hunting => "Hunting",
        _ => "Work",
    }
}

pub const WEAPON: &str = "Weapon";
pub const ARMOR: &str = "Armor";
pub const CORPSE_ITEM: &str = "Corpse";
pub const ITEM_FOOD: &str = "Food";

pub const GATHERING: &str = "Gathering";
pub const TORCH: &str = "Torch";
pub const MATERIAL: &str = "Material";
pub const MEDICAL: &str = "Medical";

pub const POTION: &str = "Potion";
pub const HEALTH: &str = "Health";
pub const DEED: &str = "Deed";

pub const _HEALING: &str = "Healing";

pub const _VISIBLE: &str = "Visble";

// TODO consider moving this to a template file
#[derive(Debug, Reflect, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum ItemSubclass {
    CopperOre,
    IronOre,
    MithrilOre,
    CopperIngot,
    IronIngot,
    MithrilIngot,
    CopperDust,
    IronDust,
    MithrilDust,
    MapleLog,
    BirchLog,
    MapleTimber,
    BirchTimber,
    HoneybellCloth,
    RawHide,
    StiffLeather,
    SpringWater,
    Berries,
    Grapes,
    Grain,
    Axe,
    Armor,
    Seeds,
    CrudeTorch,
    IgnitionTool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemLocation {
    Own,
    OwnStructure,
    _OtherOwnUnit,
    _OtherStructure,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemAction {
    Updated,
    Removed,
}

#[derive(Debug, Clone)]
pub enum DurabilityUseOutcome {
    Updated(Item),
    Removed { id: i32, name: String },
}

#[derive(Debug, Reflect, Clone, PartialEq)]
pub enum ExperimentItemType {
    Source,
    Reagent,
}

#[derive(Debug, Reflect, Clone, Copy, PartialEq)]
pub enum Slot {
    Invalid,
    Helm,
    Shoulder,
    Chest,
    Pants,
    Boots,
    MainHand,
    OffHand,
}

impl Slot {
    pub fn str_to_slot(slot: String) -> Slot {
        match slot.as_str() {
            "Helm" => Slot::Helm,
            "Shoulder" => Slot::Shoulder,
            "Chest" => Slot::Chest,
            "Pants" => Slot::Pants,
            "Boots" => Slot::Boots,
            "Main Hand" => Slot::MainHand,
            "Off Hand" => Slot::OffHand,
            _ => {
                error!("Invalid slot: {:?}", slot);
                Slot::Invalid
            }
        }
    }

    pub fn to_str(slot: Option<Slot>) -> Option<String> {
        if let Some(slot) = slot {
            let slot_str = match slot {
                Slot::Helm => "Helm",
                Slot::Shoulder => "Shoulder",
                Slot::Chest => "Chest",
                Slot::Pants => "Pants",
                Slot::Boots => "Boots",
                Slot::MainHand => "Main Hand",
                Slot::OffHand => "Off Hand",
                _ => {
                    error!("Invalid slot: {:?}", slot);
                    "Invalid"
                }
            };

            return Some(slot_str.to_string());
        } else {
            return None;
        }
    }
}

#[derive(Debug, Reflect, Component, Clone)]
#[reflect(Component)]
pub struct Inventory {
    pub owner: i32,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftError {
    InsufficientResources,
    InventoryFull,
}

#[derive(Debug, Clone)]
pub struct RefineOutcome {
    pub remaining_source: Option<Item>,
    pub produced: Vec<(Item, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefineError {
    ItemNotFound,
    ItemNotRefineable,
    MissingItemTemplate(String),
    InventoryFull,
}

pub fn produced_item_packets(
    outputs: &[String],
    item_templates: &Vec<ItemTemplate>,
) -> Vec<network::ProducedItem> {
    let mut produced: Vec<network::ProducedItem> = Vec::new();

    for output in outputs {
        if let Some(existing) = produced.iter_mut().find(|item| item.name == *output) {
            existing.quantity += 1;
            continue;
        }

        let template = Item::get_template(output.clone(), item_templates);
        produced.push(network::ProducedItem {
            name: template.name.clone(),
            image: template.image.clone(),
            class: template.class.clone(),
            subclass: template.subclass.clone(),
            quantity: 1,
        });
    }

    produced
}

impl Inventory {
    /// Give a newly-created person the shared starter clothing loadout.
    /// Each piece is created separately and equipped into its template slot.
    pub fn add_equipped_tattered_clothing(
        &mut self,
        shirt_item_id: i32,
        pants_item_id: i32,
        item_templates: &Vec<ItemTemplate>,
    ) {
        let shirt = self.new(shirt_item_id, TATTERED_SHIRT.to_string(), 1, item_templates);
        let pants = self.new(pants_item_id, TATTERED_PANTS.to_string(), 1, item_templates);

        self.equip(shirt.id, shirt.slot);
        self.equip(pants.id, pants.slot);
    }

    pub fn transfer(
        item_id: i32,
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
    ) {
        if let Some(transfer_index) = source_inventory
            .items
            .iter()
            .position(|item| item.id == item_id)
        {
            let mut item_to_transfer = source_inventory.items[transfer_index].clone();
            // Items always arrive unequipped. Normalize that state before
            // looking for a destination stack so an equipped stack can never
            // absorb a transferred item.
            item_to_transfer.owner = target_inventory.owner;
            item_to_transfer.equipped = false;

            if Item::can_merge_by_class(item_to_transfer.class.clone()) {
                if let Some(merged_index) = target_inventory
                    .items
                    .iter()
                    .position(|item| item.stack_identity_matches(&item_to_transfer))
                {
                    let merged_item = &mut target_inventory.items[merged_index];
                    merged_item.quantity += item_to_transfer.quantity;

                    source_inventory.items.swap_remove(transfer_index);
                } else {
                    target_inventory.items.push(item_to_transfer);
                    source_inventory.items.swap_remove(transfer_index);
                }
            } else {
                // Non-mergeable items (Weapon, Armor, Container — e.g. Bucket,
                // Pick Axe, Sickle) still need their owner field updated, or
                // the use_item / equip / etc. flows will reject the item with
                // "Item not owned by player" because they look up the obj
                // entity by `item.owner`.
                target_inventory.items.push(item_to_transfer);
                source_inventory.items.swap_remove(transfer_index);
            }
        }
    }

    /// Transfer exactly one unit while preserving the source item's current
    /// durability and generated attributes. `new_item_id` is used only when
    /// the source is a stack and therefore has to be split.
    pub fn transfer_one(
        item_id: i32,
        new_item_id: i32,
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
    ) -> bool {
        let Some(transfer_index) = source_inventory
            .items
            .iter()
            .position(|item| item.id == item_id)
        else {
            return false;
        };

        match source_inventory.items[transfer_index].quantity {
            quantity if quantity <= 0 => false,
            1 => {
                Inventory::transfer(item_id, source_inventory, target_inventory);
                true
            }
            _ => {
                let Some((item_to_transfer, _source_item)) =
                    source_inventory.split_instance_stack(item_id, new_item_id, 1)
                else {
                    return false;
                };

                Inventory::transfer(item_to_transfer.id, source_inventory, target_inventory);
                true
            }
        }
    }

    /// Split part of an existing stack without rebuilding it from its template.
    /// This preserves durability and generated attributes while ensuring the
    /// separated quantity is unequipped. It is primarily a repair path for
    /// legacy equipment stacks where one entry represented multiple tools.
    pub fn split_instance_stack(
        &mut self,
        item_id: i32,
        new_item_id: i32,
        quantity: i32,
    ) -> Option<(Item, Item)> {
        let index = self.items.iter().position(|item| item.id == item_id)?;
        if quantity <= 0 || self.items[index].quantity <= quantity {
            return None;
        }

        self.items[index].quantity -= quantity;
        let source_item = self.items[index].clone();
        let mut split_item = source_item.clone();
        split_item.id = new_item_id;
        split_item.quantity = quantity;
        split_item.equipped = false;
        self.items.push(split_item.clone());

        Some((split_item, source_item))
    }

    pub fn transfer_quantity(
        item_id: i32,
        new_item_id: i32,
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
        quantity: i32,
        item_templates: &Vec<ItemTemplate>,
    ) -> Option<Item> {
        info!(
            "Transferring quantity from {:?} to {:?}",
            source_inventory.owner, target_inventory.owner
        );
        info!("Item id: {:?}", item_id);
        info!("New item id: {:?}", new_item_id);
        info!("Quantity: {:?}", quantity);

        if let Some(transfer_index) = source_inventory
            .items
            .iter()
            .position(|item| item.id == item_id)
        {
            let item_to_transfer = source_inventory.items[transfer_index].clone();

            let result =
                source_inventory.split(item_to_transfer.id, new_item_id, quantity, item_templates);

            if let Some((new_item, source_item)) = result {
                Inventory::transfer(new_item.id, source_inventory, target_inventory);

                // Return remaining source item
                return Some(source_item);
            } else {
                Inventory::transfer(item_id, source_inventory, target_inventory);

                // Return nothing as no remaining source item
                return None;
            }
        } else {
            // Return nothing as item not found in source inventory
            error!("Item not found in source inventory: {:?}", item_id);
            return None;
        }
    }

    pub fn transfer_all_items(source_inventory: &mut Inventory, target_inventory: &mut Inventory) {
        let item_ids: Vec<i32> = source_inventory.items.iter().map(|item| item.id).collect();

        for item_id in item_ids {
            Inventory::transfer(item_id, source_inventory, target_inventory);
        }
    }

    pub fn transfer_all_unequipped_items(
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
    ) {
        let item_ids: Vec<i32> = source_inventory
            .items
            .iter()
            .filter(|item| !item.equipped)
            .map(|item| item.id)
            .collect();

        for item_id in item_ids {
            Inventory::transfer(item_id, source_inventory, target_inventory);
        }
    }

    pub fn transfer_all_items_by_type(
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
        item_type: String,
    ) {
        let item_ids: Vec<i32> = source_inventory
            .items
            .iter()
            .filter(|item| item.class == item_type)
            .map(|item| item.id)
            .collect();

        for item_id in item_ids {
            Inventory::transfer(item_id, source_inventory, target_inventory);
        }
    }

    pub fn transfer_all_resources(
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
    ) {
        let item_ids: Vec<i32> = source_inventory
            .items
            .iter()
            .filter(|item| {
                !item.equipped
                    && (item.class == ORE
                        || item.class == LOG
                        || item.class == STONE
                        || item.class == HIDE)
            })
            .map(|item| item.id)
            .collect();

        for item_id in item_ids {
            Inventory::transfer(item_id, source_inventory, target_inventory);
        }
    }

    pub fn transfer_partial_resources(
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
        ids: &mut Ids,
        target_capacity: i32,
        item_templates: &Vec<ItemTemplate>,
    ) {
        info!(
            "Transferring partial resources from {:?} to {:?}",
            source_inventory.owner, target_inventory.owner
        );
        let resource_items: Vec<(i32, i32, f32)> = source_inventory
            .items
            .iter()
            .filter(|item| {
                !item.equipped
                    && (item.class == ORE
                        || item.class == LOG
                        || item.class == STONE
                        || item.class == HIDE)
            })
            .map(|item| (item.id, item.quantity, item.weight))
            .collect();

        let mut target_total_weight = target_inventory.get_total_weight();
        info!("Target total weight: {:?}", target_total_weight);

        for (item_id, quantity, item_weight) in resource_items {
            let remaining_capacity = target_capacity - target_total_weight;
            info!("Remaining capacity: {:?}", remaining_capacity);

            // If no capacity left, stop transferring
            if remaining_capacity <= 0 {
                break;
            }

            let total_item_weight = (item_weight * quantity as f32) as i32;

            // If the entire item fits, transfer it all
            info!("Total item weight: {:?}", total_item_weight);
            if total_item_weight <= remaining_capacity {
                info!("Transferring entire item");
                Inventory::transfer(item_id, source_inventory, target_inventory);
                target_total_weight += total_item_weight;
            } else {
                // Transfer only what fits
                info!("Transferring partial item");
                let num_to_transfer = remaining_capacity / item_weight as i32;
                info!("Number to transfer: {:?}", num_to_transfer);

                if num_to_transfer > 0 {
                    let new_id = ids.new_item_id();
                    info!("New item id: {:?}", new_id);
                    Inventory::transfer_quantity(
                        item_id,
                        new_id,
                        source_inventory,
                        target_inventory,
                        num_to_transfer,
                        &item_templates,
                    );
                }
                // Receiver is now full, stop transferring
                break;
            }
        }
    }

    pub fn transfer_all_refined(
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
    ) {
        let item_ids: Vec<i32> = source_inventory
            .items
            .iter()
            .filter(|item| item.class == INGOT || item.class == DUST || item.class == TIMBER)
            .map(|item| item.id)
            .collect();

        for item_id in item_ids {
            Inventory::transfer(item_id, source_inventory, target_inventory);
        }
    }

    pub fn transfer_gold(
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
        quantity: i32,
        next_item_id: &mut i32,
        item_templates: &Vec<ItemTemplate>,
    ) {
        let mut remainder = quantity;
        let mut transfer_items = Vec::new();

        for item in &mut source_inventory.items.iter() {
            if item.class == GOLD.to_string() {
                if item.quantity >= remainder {
                    transfer_items.push((item.id, remainder));
                } else {
                    transfer_items.push((item.id, item.quantity));

                    remainder = remainder - item.quantity;
                }
            }
        }

        for (transfer_item_id, transfer_quantity) in transfer_items.iter() {
            let new_id = *next_item_id;
            *next_item_id += 1;
            Inventory::transfer_quantity(
                *transfer_item_id,
                new_id,
                source_inventory,
                target_inventory,
                *transfer_quantity,
                item_templates,
            );
        }
    }

    fn _can_merge_by_class(item_class: String) -> bool {
        match item_class.as_str() {
            constants::WEAPON => false,
            constants::ARMOR => false,
            constants::CONTAINER => false,
            CORPSE_ITEM => false,
            _ => true,
        }
    }

    pub fn new(
        &mut self,
        item_id: i32,
        name: String,
        quantity: i32,
        item_templates: &Vec<ItemTemplate>,
    ) -> Item {
        let mut class = "Invalid".to_string();
        let mut subclass = "Invalid".to_string();
        let mut image = "Invalid".to_string();
        let mut weight = 0.0;
        let mut slot = None;
        let mut durability = None;

        let mut attrs = HashMap::new();
        let mut produces = Vec::new();

        for item_template in item_templates.iter() {
            if name == item_template.name {
                class = item_template.class.clone();
                subclass = item_template.subclass.clone();
                image = item_template.image.clone();
                weight = item_template.weight;

                if let Some(item_template_durability) = &item_template.durability {
                    durability = Some(*item_template_durability);
                }

                if let Some(item_template_slot) = &item_template.slot {
                    slot = Some(Slot::str_to_slot(item_template_slot.to_string()));
                }

                if let Some(item_template_attrs) = &item_template.attrs {
                    for item_attr in item_template_attrs.iter() {
                        let attr_key = AttrKey::str_to_key(item_attr.name.clone());
                        let attr_val = AttrVal::Num(item_attr.value.parse::<f32>().unwrap());
                        attrs.insert(attr_key, attr_val);
                    }
                }

                if let Some(item_template_produces) = &item_template.produces {
                    produces = item_template_produces.clone();
                }
            }
        }
        debug!("Item new attrs: {:?}", attrs);

        if Item::can_merge_by_class(class.clone()) {
            if let Some(merged_index) = self.mergeable(name.clone(), attrs.clone(), false) {
                let merged_item = &mut self.items[merged_index];
                merged_item.quantity += quantity;
                return merged_item.clone();
            }
        }

        let new_item = Item {
            id: item_id,
            owner: self.owner,
            name,
            quantity,
            durability,
            class,
            subclass,
            slot,
            image,
            weight,
            equipped: false,
            experiment: None,
            start_time: 0,
            attrs,
            produces,
        };

        self.items.push(new_item.clone());
        debug!("New Item by new(): {:?}", new_item);

        new_item
    }

    pub fn new_with_attrs(
        &mut self,
        item_id: i32,
        owner: i32,
        name: String,
        quantity: i32,
        mut attrs: HashMap<AttrKey, AttrVal>,
        item_templates: &Vec<ItemTemplate>,
    ) -> (Item, bool) {
        let mut class = "Invalid".to_string();
        let mut subclass = "Invalid".to_string();
        let mut image = "Invalid".to_string();
        let mut weight = 0.0;
        let mut durability = None;
        let mut slot = None;
        let mut produces = Vec::new();

        for item_template in item_templates.iter() {
            if name == item_template.name {
                class = item_template.class.clone();
                subclass = item_template.subclass.clone();
                image = item_template.image.clone();
                weight = item_template.weight;

                if let Some(item_template_durability) = &item_template.durability {
                    durability = Some(*item_template_durability);
                }

                if let Some(item_template_slot) = &item_template.slot {
                    slot = Some(Slot::str_to_slot(item_template_slot.to_string()));
                }

                if let Some(item_template_produces) = &item_template.produces {
                    produces = item_template_produces.clone();
                }

                // Resource and refinement attributes augment the output
                // template. Keep intrinsic attributes such as Feed or Food
                // Poisoning unless the source explicitly overrides them.
                let mut combined_attrs = item_template.convert_attrs();
                combined_attrs.extend(attrs.clone());
                attrs = combined_attrs;
            }
        }

        info!("New item: {:?}", name);
        info!("Class: {:?}", class);
        info!("Subclass: {:?}", subclass);
        info!("Image: {:?}", image);
        info!("Weight: {:?}", weight);
        info!("Durability: {:?}", durability);
        info!("Slot: {:?}", slot);
        info!("Produces: {:?}", produces);

        // Can new item be merged into existing
        if Item::can_merge_by_class(class.clone()) {
            if let Some(merged_index) = self.mergeable(name.clone(), attrs.clone(), false) {
                info!("Merged index: {:?}", merged_index);
                let merged_item = &mut self.items[merged_index];
                info!("Merged item: {:?}", merged_item);
                merged_item.quantity += quantity;

                return (merged_item.clone(), true);
            } else {
                // Create the new item
                let new_item = Item {
                    id: item_id,
                    owner: owner,
                    name: name,
                    quantity: quantity,
                    durability: durability,
                    class: class,
                    subclass: subclass,
                    slot: slot,
                    image: image,
                    weight: weight,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: attrs,
                    produces: produces.clone(),
                };

                self.items.push(new_item.clone());

                // Return new item to send to client
                return (new_item, false);
            }
        } else {
            // Create the new item
            let new_item = Item {
                id: item_id,
                owner: owner,
                name: name,
                quantity: quantity,
                durability: durability,
                class: class,
                subclass: subclass,
                slot: slot,
                image: image,
                weight: weight,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: attrs,
                produces: produces,
            };

            self.items.push(new_item.clone());

            // Return new item to send to client
            return (new_item, false);
        }
    }

    pub fn create(
        &mut self,
        item_id: i32,
        _owner: i32,
        name: String,
        quantity: i32,
        item_templates: &Vec<ItemTemplate>,
    ) -> (Item, bool) {
        let mut class = "Invalid".to_string();
        let mut _subclass;
        let mut _image;
        let mut _weight;

        for item_template in item_templates.iter() {
            if name == item_template.name {
                class = item_template.class.clone();
                _subclass = item_template.subclass.clone();
                _image = item_template.image.clone();
                _weight = item_template.weight;
            }
        }

        // Can new item be merged into existing
        if Item::can_merge_by_class(class) {
            if let Some(merged_index) = self
                .items
                .iter()
                .position(|item| item.name == name && !item.equipped)
            {
                let merged_item = &mut self.items[merged_index];
                merged_item.quantity += quantity;

                return (merged_item.clone(), true);
            } else {
                // Create the new item
                let new_item = self.new(item_id, name, quantity, item_templates);

                // Return new item to send to client
                return (new_item, false);
            }
        } else {
            // Create the new item
            let new_item = self.new(item_id, name, quantity, item_templates);

            // Return new item to send to client
            return (new_item, false);
        }
    }

    pub fn craft(
        &mut self,
        item_id: i32,
        owner: i32,
        recipe_name: String,
        recipe: &Recipe,
        custom_name: Option<String>,  //override
        custom_image: Option<String>, //override
    ) -> Item {
        self.craft_with_signature(
            item_id,
            owner,
            recipe_name,
            recipe,
            custom_name,
            custom_image,
            None,
        )
    }

    fn craft_with_signature(
        &mut self,
        item_id: i32,
        owner: i32,
        recipe_name: String,
        recipe: &Recipe,
        custom_name: Option<String>,
        custom_image: Option<String>,
        signature_item_id: Option<i32>,
    ) -> Item {
        // By default the recipe name is the item name
        let mut name: String = recipe_name.clone();

        let quantity = recipe.amount.unwrap_or(1).max(1);

        let class = recipe.class.clone();
        let subclass = recipe.subclass.clone();
        let mut image = recipe.image.clone();
        // Item::weight is per unit; Inventory::get_total_weight applies the
        // stack quantity.
        let weight = recipe.weight;
        let durability = recipe.durability.clone();
        let slot = recipe.slot.clone();

        if let Some(custom_name) = custom_name {
            name = custom_name;
        }

        if let Some(custom_image) = custom_image {
            image = custom_image;
        }

        // Get consumed items and their attrs
        let consumed_items = self
            .try_consume_craft_reqs(&recipe.req, signature_item_id)
            .expect("craft called without sufficient recipe inputs");
        let mut item_attrs = HashMap::new();

        for consumed_item in consumed_items.iter() {
            for (key, value) in consumed_item.attrs.iter() {
                if Some(consumed_item.id) == signature_item_id && is_component_affix_attr(key) {
                    continue;
                }
                item_attrs.insert(*key, value.clone());
            }
        }

        // Preparing food (cooking, smoking, salting, stewing) neutralizes raw-meat
        // food poisoning. Without this, the FoodPoisoning attr on Raw Meat carries
        // through into Cooked Meat / Smoked Meat / Salted Meat Strip / Stew.
        if recipe.class == "Food" {
            item_attrs.remove(&AttrKey::FoodPoisoning);
        }

        // Check if recipe has attrs and merge into item attrs
        if let Some(recipe_attrs) = &recipe.attrs {
            for attr in recipe_attrs.iter() {
                item_attrs.insert(
                    AttrKey::str_to_key(attr.name.clone()),
                    AttrVal::Num(attr.value.parse::<f32>().unwrap_or(0.0)),
                );
            }
        }

        // Exactly one explicitly selected component controls crafted rarity.
        // Its numeric affixes are bonuses, so they add to a recipe's base
        // attribute rather than replacing it (for example +2 Damage augments
        // a 9-damage spear instead of turning it into a 2-damage spear).
        if let Some(signature_id) = signature_item_id {
            if let Some(signature_item) = consumed_items.iter().find(|item| item.id == signature_id)
            {
                for (key, value) in signature_item.attrs.iter() {
                    if !is_component_affix_attr(key) {
                        continue;
                    }
                    match (key, value) {
                        (AttrKey::Rarity | AttrKey::Affixes, _) => {
                            item_attrs.insert(*key, value.clone());
                        }
                        (_, AttrVal::Num(bonus)) => {
                            let base = match item_attrs.get(key) {
                                Some(AttrVal::Num(value)) => *value,
                                _ => 0.0,
                            };
                            item_attrs.insert(*key, AttrVal::Num(base + bonus));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Create new item
        let new_item = Item {
            id: item_id,
            owner: owner,
            name: name.clone(),
            quantity: quantity,
            durability: durability,
            class: class,
            subclass: subclass,
            slot: slot,
            image: image,
            weight: weight,
            equipped: false,
            experiment: None,
            start_time: 0,
            attrs: item_attrs.clone(),
            produces: Vec::new(),
        };

        // Check if any other items are mergeable
        if let Some(merged_index) = self.mergeable(name.clone(), item_attrs.clone(), false) {
            let merged_item = &mut self.items[merged_index];
            merged_item.quantity += quantity;

            // Return new item for notification instead of merged item
            return new_item.clone();
        } else {
            self.items.push(new_item.clone());
            return new_item;
        }
    }

    /// Execute a craft as one inventory transaction. Inputs and outputs are
    /// staged on a clone so a failed capacity check cannot consume materials.
    pub fn try_craft(
        &mut self,
        item_id: i32,
        owner: i32,
        recipe_name: String,
        recipe: &Recipe,
        custom_name: Option<String>,
        custom_image: Option<String>,
        capacity: i32,
    ) -> Result<Item, CraftError> {
        self.try_craft_with_signature(
            item_id,
            owner,
            recipe_name,
            recipe,
            custom_name,
            custom_image,
            None,
            capacity,
        )
    }

    pub fn try_craft_with_signature(
        &mut self,
        item_id: i32,
        owner: i32,
        recipe_name: String,
        recipe: &Recipe,
        custom_name: Option<String>,
        custom_image: Option<String>,
        signature_item_id: Option<i32>,
        capacity: i32,
    ) -> Result<Item, CraftError> {
        if !self.has_craft_reqs(recipe.req.clone(), signature_item_id) {
            return Err(CraftError::InsufficientResources);
        }

        let mut candidate = self.clone();
        let new_item = candidate.craft_with_signature(
            item_id,
            owner,
            recipe_name,
            recipe,
            custom_name,
            custom_image,
            signature_item_id,
        );

        if candidate.get_total_weight() > capacity {
            return Err(CraftError::InventoryFull);
        }

        *self = candidate;
        Ok(new_item)
    }

    /// Refine one source unit and create all outputs atomically.
    pub fn try_refine(
        &mut self,
        item_id: i32,
        yield_multiplier: i32,
        capacity: i32,
        item_templates: &Vec<ItemTemplate>,
        ids: &mut Ids,
    ) -> Result<RefineOutcome, RefineError> {
        let source = self.get_by_id(item_id).ok_or(RefineError::ItemNotFound)?;
        let source_template = Item::find_template(source.name.clone(), item_templates)
            .ok_or_else(|| RefineError::MissingItemTemplate(source.name.clone()))?;
        let outputs = source_template
            .produces
            .clone()
            .ok_or(RefineError::ItemNotRefineable)?;
        let yield_multiplier = yield_multiplier.max(1);

        let mut output_templates: Vec<(ItemTemplate, i32)> = Vec::new();
        let mut output_weight = 0.0;
        for output in outputs.iter() {
            let template = Item::find_template(output.clone(), item_templates)
                .ok_or_else(|| RefineError::MissingItemTemplate(output.clone()))?
                .clone();
            output_weight += template.weight * yield_multiplier as f32;
            if let Some((_, quantity)) = output_templates
                .iter_mut()
                .find(|(existing, _)| existing.name == template.name)
            {
                *quantity += 1;
            } else {
                output_templates.push((template, 1));
            }
        }

        let final_weight = self.get_total_weight() as f32 - source.weight + output_weight;
        if final_weight > capacity as f32 {
            return Err(RefineError::InventoryFull);
        }

        let mut produced = Vec::new();
        for (template, output_quantity) in output_templates {
            let inherited_attrs = if source.class == GAME_ANIMAL && template.class != HIDE {
                // A quality carcass represents useful hide quality. Do not
                // duplicate the same signature affixes onto every parallel
                // food output produced by butchery.
                without_component_affixes(&source.attrs)
            } else {
                source.attrs.clone()
            };
            let produced_quantity = output_quantity * yield_multiplier;
            let (item, _) = self.new_with_attrs(
                ids.new_item_id(),
                self.owner,
                template.name,
                produced_quantity,
                inherited_attrs,
                item_templates,
            );
            produced.push((item, produced_quantity));
        }

        let remaining_source = self.remove_quantity(source.id, 1);
        Ok(RefineOutcome {
            remaining_source,
            produced,
        })
    }

    pub fn split(
        &mut self,
        item_id: i32,
        new_item_id: i32,
        quantity: i32,
        item_templates: &Vec<ItemTemplate>,
    ) -> Option<(Item, Item)> {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            if (item.quantity - quantity) > 0 {
                item.quantity -= quantity;
                debug!("Split source item: {:?}", item);

                let mut class = "Invalid".to_string();
                let mut subclass = "Invalid".to_string();
                let mut image = "Invalid".to_string();
                let mut weight = 0.0;
                let mut durability = None;
                let mut slot = None;
                let mut produces = Vec::new();

                for item_template in item_templates.iter() {
                    if item.name == item_template.name {
                        class = item_template.class.clone();
                        subclass = item_template.subclass.clone();
                        image = item_template.image.clone();
                        weight = item_template.weight;

                        if let Some(item_template_durability) = &item_template.durability {
                            durability = Some(*item_template_durability);
                        }

                        if let Some(item_template_slot) = &item_template.slot {
                            slot = Some(Slot::str_to_slot(item_template_slot.to_string()));
                        }

                        if let Some(item_template_produces) = &item_template.produces {
                            produces = item_template_produces.clone();
                        }
                    }
                }

                let new_item = Item {
                    id: new_item_id,
                    owner: item.owner,
                    name: item.name.clone(),
                    quantity: quantity,
                    durability: durability,
                    class: class,
                    subclass: subclass,
                    slot: slot,
                    image: image,
                    weight: weight,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: item.attrs.clone(),
                    produces: produces,
                };

                self.items.push(new_item.clone());

                let source_item = self.items[index].clone();

                return Some((new_item.clone(), source_item));
            } else {
                return None;
            }
        }

        return None;
    }

    pub fn update_quantity(&mut self, name: String, mod_quantity: i32) -> Option<Item> {
        if let Some(index) = self.items.iter().position(|item| item.name == name) {
            let item = &mut self.items[index];
            item.quantity += mod_quantity;
            return Some(item.clone());
        } else {
            return None;
        }
    }

    pub fn update_quantity_by_class(
        &mut self,
        class: String,
        mod_quantity: i32,
    ) -> Option<(Item, ItemAction)> {
        if let Some(index) = self.find_by_class(class) {
            let item = &mut self.items[index];
            debug!(
                "item quantity: {:?} mod_quantity: {:?}",
                item.quantity, mod_quantity
            );
            if (item.quantity + mod_quantity) > 0 {
                item.quantity += mod_quantity;
                return Some((item.clone(), ItemAction::Updated));
            } else {
                let removed_item = item.clone();
                debug!("Removing item {:?}", index);
                self.items.swap_remove(index);
                debug!("items: {:?}", self.items);
                return Some((removed_item, ItemAction::Removed)); // Return the item that was removed
            }
        } else {
            return None;
        }
    }

    pub fn switch_image(&mut self, item_id: i32, new_image: String) {
        if let Some(switch_index) = self.items.iter().position(|item| item.id == item_id) {
            let switched_item = &mut self.items[switch_index];
            switched_item.image = new_image;
        }
    }

    pub fn transform(
        &mut self,
        item_id: i32,
        new_name: String,
        new_quantity: i32,
        item_templates: &Vec<ItemTemplate>,
    ) {
        let mut class = "Invalid".to_string();
        let mut subclass = "Invalid".to_string();
        let mut image = "Invalid".to_string();
        let mut weight = 0.0;
        let mut slot = None;

        let mut attrs = HashMap::new();
        let mut produces = Vec::new();

        for item_template in item_templates.iter() {
            if new_name == item_template.name {
                class = item_template.class.clone();
                subclass = item_template.subclass.clone();
                image = item_template.image.clone();
                weight = item_template.weight;

                if let Some(item_template_slot) = &item_template.slot {
                    slot = Some(Slot::str_to_slot(item_template_slot.to_string()));
                }

                if let Some(item_template_attrs) = &item_template.attrs {
                    for item_attr in item_template_attrs.iter() {
                        let attr_key = AttrKey::str_to_key(item_attr.name.clone());
                        let attr_val = AttrVal::Num(item_attr.value.parse::<f32>().unwrap());
                        attrs.insert(attr_key, attr_val);
                    }
                }

                if let Some(item_template_produces) = &item_template.produces {
                    produces = item_template_produces.clone();
                }
            }
        }

        if let Some(transform_index) = self.items.iter().position(|item| item.id == item_id) {
            let transformed_item = &mut self.items[transform_index];
            transformed_item.name = new_name;
            transformed_item.quantity = new_quantity;
            transformed_item.class = class;
            transformed_item.subclass = subclass;
            transformed_item.image = image;
            transformed_item.weight = weight;
            transformed_item.slot = slot;
            transformed_item.attrs = attrs;
            transformed_item.produces = produces;
        }
    }

    pub fn mergeable(
        &self,
        name: String,
        attrs: HashMap<AttrKey, AttrVal>,
        equipped: bool,
    ) -> Option<usize> {
        // Equipped state belongs to the whole stack. A newly created
        // unequipped item must never inherit equipment state by merging into
        // the stack currently occupying a character slot.
        if let Some(merged_index) = self
            .items
            .iter()
            .position(|item| item.name == name && item.attrs == attrs && item.equipped == equipped)
        {
            return Some(merged_index);
        }

        None
    }

    pub fn equip(&mut self, item_id: i32, slot: Option<Slot>) -> Vec<Item> {
        let mut items_updated = Vec::new();

        for item in &mut self.items.iter_mut() {
            // Unequip item with matching slot
            if item.id != item_id && item.equipped && item.slot == slot {
                item.equipped = false;
                items_updated.push(item.clone());
            }

            // Equip item
            if item.id == item_id {
                item.equipped = true;
                items_updated.push(item.clone());
            }
        }

        return items_updated;
    }

    pub fn unequip(&mut self, item_id: i32) -> Vec<Item> {
        let mut items_updated = Vec::new();

        for item in &mut self.items.iter_mut() {
            if item.id == item_id {
                item.equipped = false;
                items_updated.push(item.clone());
            }
        }

        return items_updated;
    }

    pub fn remove_quantity(&mut self, item_id: i32, quantity: i32) -> Option<Item> {
        let index = self
            .items
            .iter()
            .position(|item| item.id == item_id)
            .unwrap(); // Should panic if item is not found
        let item = &mut self.items[index];
        if item.quantity >= quantity {
            item.quantity -= quantity;

            if item.quantity == 0 {
                self.items.swap_remove(index);
                return None;
            }
        }

        return Some(item.clone());
    }

    pub fn remove_item(&mut self, item_id: i32) {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            self.items.remove(index);
        } else {
            error!("Item does not exist");
        }
    }

    pub fn set_start_time(&mut self, item_id: i32, game_tick: i32) {
        if let Some(index) = self.items.iter_mut().position(|item| item.id == item_id) {
            self.items[index].start_time = game_tick;
        }
    }

    pub fn get_by_id(&self, item_id: i32) -> Option<Item> {
        self.items.iter().find(|item| item.id == item_id).cloned()
    }

    pub fn get_items_value_by_attr(&self, attr: &AttrKey, equipped_only: bool) -> f32 {
        let mut item_values = 0.0;

        for item in self.items.iter() {
            if equipped_only && !item.equipped {
                continue;
            }

            match item.attrs.get(&attr) {
                Some(item_value) => {
                    let val = match item_value {
                        AttrVal::Num(attr_val) => *attr_val,
                        _ => 0.0,
                    };
                    item_values += val;
                }
                None => item_values += 0.0,
            }
        }

        return item_values;
    }

    pub fn get_total_weight(&self) -> i32 {
        let mut total_weight = 0.0;

        for item in self.items.iter() {
            total_weight += item.weight * item.quantity as f32;
        }

        return total_weight as i32;
    }

    pub fn get_total_weight_by_class(&self, class: String) -> i32 {
        let mut total_weight = 0.0;

        for item in self.items.iter() {
            if item.class == class {
                total_weight += item.weight * item.quantity as f32;
            }
        }

        return total_weight as i32;
    }

    pub fn get_packet(&self) -> Vec<network::Item> {
        let mut packets = Vec::new();

        for item in self.items.iter() {
            packets.push(item.packet());
        }

        packets
    }

    pub fn get_packet_filter(&self, filter: Vec<String>) -> Vec<network::Item> {
        let mut owner_items: Vec<network::Item> = Vec::new();

        if filter.contains(&FILTER_ALL.to_string()) {
            return vec![];
        }

        for item in self.items.iter() {
            if !filter.contains(&item.name) {
                let item_packet = network::Item {
                    id: item.id,
                    owner: item.owner,
                    name: item.name.clone(),
                    quantity: item.quantity,
                    durability: item.durability.clone(),
                    class: item.class.clone(),
                    subclass: item.subclass.clone(),
                    slot: Slot::to_str(item.slot.clone()),
                    image: item.image.clone(),
                    weight: item.weight,
                    equipped: item.equipped,
                    refineable: item.produces.len() > 0,
                    attrs: None,
                };

                owner_items.push(item_packet);
            }
        }

        return owner_items;
    }

    pub fn get_item_packet(&self, item_id: i32) -> Option<network::Item> {
        if let Some(item) = self.get_by_id(item_id) {
            return Some(item.packet());
        }

        None
    }

    pub fn get_by_name_packet(&self, item_name: String) -> Option<network::Item> {
        for item in self.items.iter() {
            if item.name == item_name {
                return Some(network::Item {
                    id: item.id,
                    owner: item.owner,
                    name: item.name.clone(),
                    quantity: item.quantity,
                    durability: item.durability.clone(),
                    class: item.class.clone(),
                    subclass: item.subclass.clone(),
                    slot: Slot::to_str(item.slot.clone()),
                    image: item.image.clone(),
                    weight: item.weight,
                    equipped: item.equipped,
                    refineable: item.produces.len() > 0,
                    attrs: None, //TODO actually get the attrs
                });
            }
        }

        return None;
    }

    pub fn get_by_class(&self, class: String) -> Option<Item> {
        self.items.iter().find(|item| item.class == class).cloned()
    }

    /// Pick the food item with the lowest Feed value — villagers should eat
    /// cheap, plentiful food (berries, mushrooms) before consuming high-value
    /// prepared meals (bread, stew) intended for the hero or emergencies.
    pub fn get_food_to_eat(&self) -> Option<Item> {
        self.items
            .iter()
            .filter(|item| item.class == "Food")
            .filter_map(|item| {
                let feed = match item.attrs.get(&AttrKey::Feed)? {
                    AttrVal::Num(v) => *v,
                    _ => return None,
                };
                Some((feed, item))
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, item)| item.clone())
    }

    pub fn get_by_name(&self, name: String) -> Option<Item> {
        self.items.iter().find(|item| item.name == name).cloned()
    }

    pub fn has_by_class(&self, class: String) -> bool {
        self.items.iter().any(|item| item.class == class)
    }

    pub fn get_one_item_by_id(
        &mut self,
        item_id: i32,
        new_item_id: i32,
        item_templates: &Vec<ItemTemplate>,
    ) -> Option<(Item, Item)> {
        let item = self.get_by_id(item_id);

        if let Some(item) = item {
            if item.quantity > 1 {
                let Some((new_item, source_item)) =
                    self.split(item_id, new_item_id, 1, item_templates)
                else {
                    return None;
                };

                return Some((new_item, source_item));
            } else {
                return Some((item.clone(), item.clone()));
            }
        }

        return None;
    }

    pub fn set_durability(&mut self, item_id: i32, durability: i32) {
        if let Some(update_index) = self.items.iter().position(|item| item.id == item_id) {
            let updated_item = &mut self.items[update_index];
            updated_item.durability = Some(durability);
        }
    }

    pub fn get_usable_by_class(&self, class: &str) -> Option<Item> {
        self.items
            .iter()
            .find(|item| {
                item.class == class
                    && item.quantity > 0
                    && item.durability.is_none_or(|durability| durability > 0)
            })
            .cloned()
    }

    /// Consume durability from one item. Durable stacks model each unit as
    /// having the template maximum: when one unit is exhausted, advance to the
    /// next unit in the stack at full durability. The final exhausted unit is
    /// removed from the inventory.
    pub fn consume_durability_use(
        &mut self,
        item_id: i32,
        amount: i32,
        maximum_durability: i32,
    ) -> Option<DurabilityUseOutcome> {
        let index = self.items.iter().position(|item| item.id == item_id)?;
        let maximum_durability = maximum_durability.max(1);
        let item = &mut self.items[index];

        if item.quantity <= 0 || item.durability.is_some_and(|durability| durability <= 0) {
            return None;
        }

        let current_durability = item.durability.unwrap_or(maximum_durability);
        let remaining_durability = current_durability.saturating_sub(amount.max(0));

        if remaining_durability > 0 {
            item.durability = Some(remaining_durability);
            return Some(DurabilityUseOutcome::Updated(item.clone()));
        }

        if item.quantity > 1 {
            item.quantity -= 1;
            item.durability = Some(maximum_durability);
            return Some(DurabilityUseOutcome::Updated(item.clone()));
        }

        let removed = self.items.remove(index);
        Some(DurabilityUseOutcome::Removed {
            id: removed.id,
            name: removed.name,
        })
    }

    pub fn find_expired_items(&self, game_tick: i32) -> Vec<Item> {
        let mut expired_items = Vec::new();
        for item in self.items.iter() {
            if item.start_time > 0 {
                let duration = match item.attrs.get(&AttrKey::Duration) {
                    Some(AttrVal::Num(duration)) => *duration as i32,
                    _ => 0,
                };

                if item.start_time + duration < game_tick {
                    expired_items.push(item.clone());
                }
            }
        }
        return expired_items;
    }

    fn requirement_consumption_plan(&self, req_items: &[ResReq]) -> Option<Vec<(usize, i32)>> {
        let mut available = self
            .items
            .iter()
            .map(|item| item.quantity)
            .collect::<Vec<_>>();
        let mut plan = Vec::new();

        for requirement in req_items {
            if requirement.quantity < 0 {
                return None;
            }

            let mut remaining = requirement.quantity;
            for (index, item) in self.items.iter().enumerate() {
                if remaining == 0 {
                    break;
                }
                if req_matches(
                    &requirement.req_type,
                    &item.name,
                    &item.class,
                    &item.subclass,
                ) {
                    let take = remaining.min(available[index]);
                    if take > 0 {
                        available[index] -= take;
                        remaining -= take;
                        plan.push((index, take));
                    }
                }
            }

            if remaining != 0 {
                return None;
            }
        }

        Some(plan)
    }

    /// Crafting uses Common inputs unless the player explicitly names one
    /// signature component. This keeps valuable drops out of villager work
    /// queues and prevents a generic recipe from silently eating a rare stack.
    fn craft_requirement_consumption_plan(
        &self,
        req_items: &[ResReq],
        signature_item_id: Option<i32>,
    ) -> Option<Vec<(usize, i32)>> {
        let signature_index = match signature_item_id {
            Some(item_id) => Some(
                self.items
                    .iter()
                    .position(|item| item.id == item_id && item.quantity > 0)?,
            ),
            None => None,
        };
        let mut signature_used = false;
        let mut available = self
            .items
            .iter()
            .map(|item| item.quantity)
            .collect::<Vec<_>>();
        let mut plan = Vec::new();

        for requirement in req_items {
            if requirement.quantity < 0 {
                return None;
            }

            let mut remaining = requirement.quantity;
            if let Some(index) = signature_index {
                let item = &self.items[index];
                if !signature_used
                    && remaining > 0
                    && req_matches(
                        &requirement.req_type,
                        &item.name,
                        &item.class,
                        &item.subclass,
                    )
                {
                    available[index] -= 1;
                    remaining -= 1;
                    signature_used = true;
                    plan.push((index, 1));
                }
            }

            let mut candidates = self
                .items
                .iter()
                .enumerate()
                .filter(|(index, item)| {
                    available[*index] > 0
                        && req_matches(
                            &requirement.req_type,
                            &item.name,
                            &item.class,
                            &item.subclass,
                        )
                        && (item.rarity() == ItemRarity::Common || Some(*index) == signature_index)
                })
                .map(|(index, item)| (index, item.rarity().rank()))
                .collect::<Vec<_>>();
            candidates.sort_by_key(|(index, rarity)| (*rarity, *index));

            for (index, _) in candidates {
                if remaining == 0 {
                    break;
                }
                // A non-Common signature contributes exactly one unit. The
                // rest of a recipe must be satisfied by ordinary materials.
                if Some(index) == signature_index
                    && self.items[index].rarity() != ItemRarity::Common
                {
                    continue;
                }
                let take = remaining.min(available[index]);
                if take > 0 {
                    available[index] -= take;
                    remaining -= take;
                    plan.push((index, take));
                }
            }

            if remaining != 0 {
                return None;
            }
        }

        if signature_item_id.is_some() && !signature_used {
            return None;
        }
        Some(plan)
    }

    pub fn try_consume_reqs(&mut self, req_items: &[ResReq]) -> Option<Vec<Item>> {
        let plan = self.requirement_consumption_plan(req_items)?;
        let mut consumed_items = Vec::new();
        let removals = plan
            .iter()
            .map(|(index, quantity)| {
                let mut consumed = self.items[*index].clone();
                consumed.quantity = *quantity;
                consumed_items.push(consumed);
                (self.items[*index].id, *quantity)
            })
            .collect::<Vec<_>>();

        for (item_id, quantity) in removals {
            self.remove_quantity(item_id, quantity);
        }

        Some(consumed_items)
    }

    fn try_consume_craft_reqs(
        &mut self,
        req_items: &[ResReq],
        signature_item_id: Option<i32>,
    ) -> Option<Vec<Item>> {
        let plan = self.craft_requirement_consumption_plan(req_items, signature_item_id)?;
        let mut consumed_items = Vec::new();
        let removals = plan
            .iter()
            .map(|(index, quantity)| {
                let mut consumed = self.items[*index].clone();
                consumed.quantity = *quantity;
                consumed_items.push(consumed);
                (self.items[*index].id, *quantity)
            })
            .collect::<Vec<_>>();

        for (item_id, quantity) in removals {
            self.remove_quantity(item_id, quantity);
        }
        Some(consumed_items)
    }

    pub fn consume_reqs(&mut self, req_items: Vec<ResReq>) -> Vec<Item> {
        self.try_consume_reqs(&req_items).unwrap_or_default()
    }

    /// Like `consume_reqs`, but understands explicit flexible construction
    /// requirements such as `Logs or Timber`.
    pub fn consume_reqs_for_build(&mut self, req_items: Vec<ResReq>) -> Vec<Item> {
        let mut consumed_items = Vec::new();
        let mut items_to_remove = Vec::new();

        for req_item in req_items.iter() {
            let mut remaining = req_item.quantity;
            for structure_item in self.items.iter() {
                if remaining == 0 {
                    break;
                }
                if req_matches_build(
                    &req_item.req_type,
                    &structure_item.name,
                    &structure_item.class,
                    &structure_item.subclass,
                ) {
                    let take = remaining.min(structure_item.quantity);
                    consumed_items.push(structure_item.clone());
                    items_to_remove.push((structure_item.id, take));
                    remaining -= take;
                }
            }
        }

        for (item_id, quantity) in items_to_remove {
            self.remove_quantity(item_id, quantity);
        }

        return consumed_items;
    }

    pub fn process_req_items(&self, mut req_items: Vec<ResReq>) -> Vec<ResReq> {
        // Check current required quantity from structure items
        for req_item in req_items.iter_mut() {
            let mut req_quantity = req_item.quantity;

            for item in self.items.iter() {
                if req_item.req_type == item.name
                    || req_item.req_type == item.class
                    || req_item.req_type == item.subclass
                {
                    if req_quantity - item.quantity > 0 {
                        req_quantity -= item.quantity;
                    } else {
                        req_quantity = 0;
                    }
                }
            }

            req_item.cquantity = Some(req_quantity);
        }

        return req_items;
    }

    /// Like `process_req_items`, but understands explicit flexible construction
    /// requirements such as `Logs or Timber`.
    pub fn process_req_items_for_build(&self, mut req_items: Vec<ResReq>) -> Vec<ResReq> {
        for req_item in req_items.iter_mut() {
            let mut req_quantity = req_item.quantity;

            for item in self.items.iter() {
                if req_matches_build(&req_item.req_type, &item.name, &item.class, &item.subclass) {
                    if req_quantity - item.quantity > 0 {
                        req_quantity -= item.quantity;
                    } else {
                        req_quantity = 0;
                    }
                }
            }

            req_item.cquantity = Some(req_quantity);
        }

        return req_items;
    }

    pub fn set_experiment_source(&mut self, item_id: i32) -> Item {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            item.experiment = Some(ExperimentItemType::Source);
            return item.clone();
        } else {
            panic!("Cannot find item: {:?}", item_id);
        }
    }

    pub fn remove_experiment_source(&mut self, item_id: i32) -> Item {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            item.experiment = None;
            return item.clone();
        } else {
            panic!("Cannot find item: {:?}", item_id);
        }
    }

    pub fn set_experiment_reagent(&mut self, item_id: i32) {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            item.experiment = Some(ExperimentItemType::Reagent);
        } else {
            error!("Cannot find item: {:?}", item_id);
        }
    }

    pub fn remove_experiment_reagent(&mut self, item_id: i32) {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            item.experiment = None;
        } else {
            error!("Cannot find item: {:?}", item_id);
        }
    }

    pub fn get_experiment_details_packet(
        &self,
    ) -> (Vec<network::Item>, Vec<network::Item>, Vec<network::Item>) {
        let mut experiment_source: Vec<network::Item> = Vec::new();
        let mut experiment_reagents: Vec<network::Item> = Vec::new();
        let mut other_resources: Vec<network::Item> = Vec::new();

        for item in self.items.iter() {
            if let Some(item_experiment_type) = &item.experiment {
                if *item_experiment_type == ExperimentItemType::Reagent {
                    experiment_reagents.push(Item::to_packet(item.clone()));
                } else if *item_experiment_type == ExperimentItemType::Source {
                    experiment_source.push(Item::to_packet(item.clone()));
                }
            } else {
                other_resources.push(Item::to_packet(item.clone()));
            }
        }

        return (experiment_source, experiment_reagents, other_resources);
    }

    pub fn get_experiment_source_reagents(&self) -> (Option<Item>, Vec<Item>) {
        let mut experiment_source = None;
        let mut experiment_reagents = Vec::new();

        for item in self.items.iter() {
            if let Some(item_experiment_type) = &item.experiment {
                if *item_experiment_type == ExperimentItemType::Reagent {
                    experiment_reagents.push(item.clone());
                } else if *item_experiment_type == ExperimentItemType::Source {
                    experiment_source = Some(item.clone());
                }
            }
        }

        return (experiment_source, experiment_reagents);
    }

    pub fn get_experiment_reagent(&self, subclass: String) -> Option<i32> {
        for item in self.items.iter() {
            if item.subclass == subclass && item.experiment == Some(ExperimentItemType::Reagent) {
                return Some(item.id);
            }
        }
        return None;
    }

    pub fn get_total_gold(&self) -> i32 {
        let mut total_gold = 0;

        for item in self.items.iter() {
            if item.class == GOLD.to_string() {
                total_gold += item.quantity;
            }
        }

        return total_gold;
    }

    pub fn get_equipped(&self) -> Vec<Item> {
        let mut equipped = Vec::new();

        for item in self.items.iter() {
            if item.equipped {
                equipped.push(item.clone());
            }
        }

        return equipped;
    }

    pub fn get_equipped_weapons(&self) -> Vec<Item> {
        let mut equipped_weapons = Vec::new();

        for item in self.items.iter() {
            if item.class == WEAPON && item.equipped {
                equipped_weapons.push(item.clone());
            }
        }

        return equipped_weapons;
    }

    pub fn get_equipped_main_hand(&self) -> Option<Item> {
        for item in self.items.iter() {
            if item.equipped && item.slot == Some(Slot::MainHand) {
                return Some(item.clone());
            }
        }

        return None;
    }

    pub fn get_equipped_by_slot(&self, slot: Slot) -> Option<Item> {
        self.items
            .iter()
            .find(|item| item.equipped && item.slot == Some(slot))
            .cloned()
    }

    pub fn has_equipped_tool_for_attr(&self, attr: &AttrKey) -> bool {
        self.items
            .iter()
            .any(|item| item.equipped && item.is_gather_tool_for_attr(attr))
    }

    pub fn get_equipped_tool_for_attr(&self, attr: &AttrKey) -> Option<Item> {
        self.items
            .iter()
            .filter(|item| item.equipped && item.is_gather_tool_for_attr(attr))
            .max_by(|a, b| {
                a.attr_num(attr)
                    .partial_cmp(&b.attr_num(attr))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    pub fn get_equipped_tool_for_res_type(&self, res_type: &str) -> Option<Item> {
        gather_tool_attr_for_res_type(res_type)
            .and_then(|attr| self.get_equipped_tool_for_attr(&attr))
    }

    pub fn best_tool_for_attr(&self, attr: &AttrKey) -> Option<Item> {
        self.items
            .iter()
            .filter(|item| item.is_gather_tool_for_attr(attr))
            .max_by(|a, b| {
                let score_cmp = a
                    .attr_num(attr)
                    .partial_cmp(&b.attr_num(attr))
                    .unwrap_or(std::cmp::Ordering::Equal);

                if score_cmp == std::cmp::Ordering::Equal {
                    b.id.cmp(&a.id)
                } else {
                    score_cmp
                }
            })
            .cloned()
    }

    pub fn auto_equip_best_tool_for_attr(&mut self, attr: &AttrKey) -> Vec<Item> {
        let Some(tool) = self.best_tool_for_attr(attr) else {
            return Vec::new();
        };

        if self.should_equip_for_attr(&tool, attr) {
            return self.equip(tool.id, tool.slot);
        }

        Vec::new()
    }

    pub fn auto_equip_best_tool_for_res_type(&mut self, res_type: &str) -> Vec<Item> {
        let Some(attr) = gather_tool_attr_for_res_type(res_type) else {
            return Vec::new();
        };

        self.auto_equip_best_tool_for_attr(&attr)
    }

    pub fn auto_equip_item_for_context(
        &mut self,
        item_id: i32,
        gather_res_type: Option<&str>,
    ) -> Vec<Item> {
        let Some(item) = self.get_by_id(item_id) else {
            return Vec::new();
        };

        if item.equipped || item.slot.is_none() || item.class == TORCH {
            return Vec::new();
        }

        let required_gather_attr = gather_res_type.and_then(gather_tool_attr_for_res_type);

        if let Some(required_attr) = required_gather_attr {
            if self.should_equip_for_attr(&item, &required_attr) {
                return self.equip(item.id, item.slot);
            }
        }

        if item.class == ARMOR && self.should_equip_for_attr(&item, &AttrKey::Defense) {
            return self.equip(item.id, item.slot);
        }

        if required_gather_attr.is_none()
            && item.class == WEAPON
            && self.should_equip_for_attr(&item, &AttrKey::Damage)
        {
            return self.equip(item.id, item.slot);
        }

        Vec::new()
    }

    fn should_equip_for_attr(&self, item: &Item, attr: &AttrKey) -> bool {
        let Some(slot) = item.slot else {
            return false;
        };

        let item_score = item.attr_num(attr);
        if item_score <= 0.0 && *attr != AttrKey::Defense {
            return false;
        }

        match self.get_equipped_by_slot(slot) {
            None => true,
            Some(equipped) if equipped.id == item.id => false,
            Some(equipped) => item_score > equipped.attr_num(attr),
        }
    }

    pub fn find_by_reqs(&self, source_req_items: Vec<ResReq>) -> Option<Vec<Item>> {
        self.requirement_consumption_plan(&source_req_items)
            .map(|plan| {
                plan.into_iter()
                    .map(|(index, quantity)| {
                        let mut item = self.items[index].clone();
                        item.quantity = quantity;
                        item
                    })
                    .collect()
            })
    }

    pub fn find_by_craft_reqs(
        &self,
        source_req_items: Vec<ResReq>,
        signature_item_id: Option<i32>,
    ) -> Option<Vec<Item>> {
        self.craft_requirement_consumption_plan(&source_req_items, signature_item_id)
            .map(|plan| {
                plan.into_iter()
                    .map(|(index, quantity)| {
                        let mut item = self.items[index].clone();
                        item.quantity = quantity;
                        item
                    })
                    .collect()
            })
    }

    pub fn has_craft_reqs(
        &self,
        source_req_items: Vec<ResReq>,
        signature_item_id: Option<i32>,
    ) -> bool {
        self.craft_requirement_consumption_plan(&source_req_items, signature_item_id)
            .is_some()
    }

    pub fn has_reqs(&self, source_req_items: Vec<ResReq>) -> bool {
        self.requirement_consumption_plan(&source_req_items)
            .is_some()
    }

    /// Like `has_reqs`, but understands explicit flexible construction
    /// requirements such as `Logs or Timber`.
    pub fn has_reqs_for_build(&self, source_req_items: Vec<ResReq>) -> bool {
        let mut req_items = source_req_items.clone();

        for req_item in req_items.iter_mut() {
            let mut req_quantity = req_item.quantity;

            for item in self.items.iter() {
                if req_matches_build(&req_item.req_type, &item.name, &item.class, &item.subclass) {
                    if req_quantity - item.quantity > 0 {
                        req_quantity -= item.quantity;
                    } else {
                        req_quantity = 0;
                    }
                }
            }
            req_item.cquantity = Some(req_quantity);
        }

        for req_item in req_items.iter() {
            if let Some(current_req_quantity) = req_item.cquantity {
                if current_req_quantity != 0 {
                    return false;
                }
            } else {
                return false;
            }
        }

        return true;
    }

    // QW4: total quantity the inventory holds that counts toward a build
    // requirement of the given type (mirrors has_reqs_for_build matching),
    // used to show the player have/need before they commit to a build.
    pub fn count_for_build_req(&self, req_type: &str) -> i32 {
        let mut count = 0;

        for item in self.items.iter() {
            if req_matches_build(req_type, &item.name, &item.class, &item.subclass) {
                count += item.quantity;
            }
        }

        return count;
    }

    fn find_by_class(&self, class: String) -> Option<usize> {
        let index = self.items.iter().position(|item| item.class == class);
        return index;
    }
}

#[derive(Debug, Reflect, Clone)]
pub struct Item {
    pub id: i32,
    pub owner: i32,
    pub name: String,
    pub quantity: i32,
    pub durability: Option<i32>,
    pub class: String,
    pub subclass: String,
    pub slot: Option<Slot>,
    pub image: String,
    pub weight: f32,
    pub equipped: bool,
    pub experiment: Option<ExperimentItemType>,
    pub start_time: i32,
    pub attrs: HashMap<AttrKey, AttrVal>,
    pub produces: Vec<String>,
}

#[derive(Resource, Default, Debug)]
pub struct Items {
    items: Vec<Item>,
    _next_id: i32,
    item_templates: Vec<ItemTemplate>,
}

impl Items {
    pub fn set_templates(&mut self, item_templates: Vec<ItemTemplate>) {
        self.item_templates = item_templates;
    }

    /*pub fn transfer_all_items(&mut self, source_id: i32, target_id: i32) {
        let source_items = self.get_by_owner(source_id);

        for source_item in source_items.iter() {
            self.transfer(source_item.id, target_id);
        }
    }

    pub fn transfer_all_items_by_type(
        &mut self,
        source_id: i32,
        target_id: i32,
        item_type: String,
    ) {
        let source_items = self.get_by_owner(source_id);

        for source_item in source_items.iter() {
            if source_item.class == item_type {
                self.transfer(source_item.id, target_id);
            }
        }
    }

    pub fn transfer_all_resources(&mut self, source_id: i32, target_id: i32) {
        let source_items = self.get_by_owner(source_id);

        for source_item in source_items.iter() {
            if source_item.class == ORE
                || source_item.class == LOG
                || source_item.class == STONE
                || source_item.class == HIDE
            {
                self.transfer(source_item.id, target_id);
            }
        }
    }

    pub fn transfer_all_refined(&mut self, source_id: i32, target_id: i32) {
        let source_items = self.get_by_owner(source_id);

        for source_item in source_items.iter() {
            if source_item.class == INGOT
                || source_item.class == DUST
                || source_item.class == TIMBER
            {
                self.transfer(source_item.id, target_id);
            }
        }
    }*/

    pub fn get_by_id(&self, item_id: i32) -> Option<Item> {
        for item in self.items.iter() {
            if item.id == item_id {
                return Some(item.clone());
            }
        }

        return None;
    }

    pub fn get_by_owner(&self, owner: i32) -> Vec<Item> {
        let mut owner_items: Vec<Item> = Vec::new();

        for item in self.items.iter() {
            if item.owner == owner {
                owner_items.push(item.clone());
            }
        }

        return owner_items;
    }

    pub fn get_by_class(&self, owner: i32, class: String) -> Option<Item> {
        if let Some(index) = self.find_by_class(owner, class) {
            let item = &self.items[index];
            return Some(item.clone());
        }

        return None;
    }

    pub fn get_by_subclass(&self, owner: i32, subclass: String) -> Option<Item> {
        if let Some(index) = self.find_by_subclass(owner, subclass) {
            let item = &self.items[index];
            return Some(item.clone());
        }

        return None;
    }

    pub fn has_by_class(&self, owner: i32, class: String) -> bool {
        if let Some(_index) = self.find_by_class(owner, class) {
            return true;
        }

        return false;
    }

    pub fn get_by_owner_packet(&self, owner: i32) -> Vec<network::Item> {
        let mut owner_items: Vec<network::Item> = Vec::new();

        for item in self.items.iter() {
            if item.owner == owner {
                let item_packet = network::Item {
                    id: item.id,
                    owner: item.owner,
                    name: item.name.clone(),
                    quantity: item.quantity,
                    durability: item.durability.clone(),
                    class: item.class.clone(),
                    subclass: item.subclass.clone(),
                    slot: Slot::to_str(item.slot.clone()),
                    image: item.image.clone(),
                    weight: item.weight,
                    equipped: item.equipped,
                    refineable: item.produces.len() > 0,
                    attrs: Some(item.attrs.clone()),
                };

                owner_items.push(item_packet);
            }
        }

        return owner_items;
    }

    pub fn get_by_owner_packet_filter(
        &self,
        owner: i32,
        filter: Vec<String>,
    ) -> Vec<network::Item> {
        let mut owner_items: Vec<network::Item> = Vec::new();

        if filter.contains(&FILTER_ALL.to_string()) {
            return vec![];
        }

        for item in self.items.iter() {
            if item.owner == owner {
                if !filter.contains(&item.name) {
                    let item_packet = network::Item {
                        id: item.id,
                        owner: item.owner,
                        name: item.name.clone(),
                        quantity: item.quantity,
                        durability: item.durability.clone(),
                        class: item.class.clone(),
                        subclass: item.subclass.clone(),
                        slot: Slot::to_str(item.slot.clone()),
                        image: item.image.clone(),
                        weight: item.weight,
                        equipped: item.equipped,
                        refineable: item.produces.len() > 0,
                        attrs: None,
                    };

                    owner_items.push(item_packet);
                }
            }
        }

        return owner_items;
    }

    pub fn get_by_owner_packet_include(
        &self,
        owner: i32,
        filter: Vec<String>,
    ) -> Vec<network::Item> {
        let mut owner_items: Vec<network::Item> = Vec::new();

        if filter.contains(&FILTER_ALL.to_string()) {
            return vec![];
        }

        for item in self.items.iter() {
            if item.owner == owner {
                if filter.contains(&item.name) {
                    let item_packet = network::Item {
                        id: item.id,
                        owner: item.owner,
                        name: item.name.clone(),
                        quantity: item.quantity,
                        durability: item.durability.clone(),
                        class: item.class.clone(),
                        subclass: item.subclass.clone(),
                        slot: Slot::to_str(item.slot.clone()),
                        image: item.image.clone(),
                        weight: item.weight,
                        equipped: item.equipped,
                        refineable: item.produces.len() > 0,
                        attrs: None,
                    };

                    owner_items.push(item_packet);
                }
            }
        }

        return owner_items;
    }

    pub fn get_packet(&self, item_id: i32) -> Option<network::Item> {
        for item in self.items.iter() {
            if item.id == item_id {
                return Some(network::Item {
                    id: item.id,
                    owner: item.owner,
                    name: item.name.clone(),
                    quantity: item.quantity,
                    durability: item.durability.clone(),
                    class: item.class.clone(),
                    subclass: item.subclass.clone(),
                    slot: Slot::to_str(item.slot.clone()),
                    image: item.image.clone(),
                    weight: item.weight,
                    equipped: item.equipped,
                    refineable: item.produces.len() > 0,
                    attrs: Some(item.attrs.clone()),
                });
            }
        }

        return None;
    }

    pub fn get_by_name_packet(&self, item_name: String) -> Option<network::Item> {
        for item in self.items.iter() {
            if item.name == item_name {
                return Some(network::Item {
                    id: item.id,
                    owner: item.owner,
                    name: item.name.clone(),
                    quantity: item.quantity,
                    durability: item.durability.clone(),
                    class: item.class.clone(),
                    subclass: item.subclass.clone(),
                    slot: Slot::to_str(item.slot.clone()),
                    image: item.image.clone(),
                    weight: item.weight,
                    equipped: item.equipped,
                    refineable: item.produces.len() > 0,
                    attrs: None, //TODO actually get the attrs
                });
            }
        }

        return None;
    }

    pub fn list_to_packet(items: Vec<Item>) -> Vec<network::Item> {
        let mut network_item_list = Vec::new();

        for item in items.iter() {
            network_item_list.push(item.packet())
        }

        return network_item_list;
    }

    pub fn get_equipped(&self, owner: i32) -> Vec<Item> {
        let mut equipped = Vec::new();

        for item in self.items.iter() {
            if item.owner == owner && item.equipped {
                equipped.push(item.clone());
            }
        }

        return equipped;
    }

    pub fn get_equipped_main_hand(&self, owner: i32) -> Option<Item> {
        for item in self.items.iter() {
            if item.owner == owner && item.equipped && item.slot == Some(Slot::MainHand) {
                return Some(item.clone());
            }
        }

        return None;
    }

    pub fn get_total_weight(&self, owner: i32) -> i32 {
        let mut total_weight = 0.0;

        for item in self.items.iter() {
            if item.owner == owner {
                total_weight += item.weight * item.quantity as f32;
            }
        }

        return total_weight as i32;
    }

    pub fn get_total_weight_by_class(&self, owner: i32, class: String) -> i32 {
        let mut total_weight = 0.0;

        for item in self.items.iter() {
            if item.owner == owner && item.class == class {
                total_weight += item.weight * item.quantity as f32;
            }
        }

        return total_weight as i32;
    }

    pub fn equip(&mut self, item_id: i32, owner: i32, slot: Option<Slot>) -> Vec<Item> {
        let mut items_updated = Vec::new();

        for item in &mut self.items.iter_mut() {
            // Unequip item with matching slot
            if item.owner == owner && item.id != item_id && item.equipped && item.slot == slot {
                item.equipped = false;
                items_updated.push(item.clone());
            }

            // Equip item
            if item.id == item_id {
                item.equipped = true;
                items_updated.push(item.clone());
            }
        }

        return items_updated;
    }

    pub fn unequip(&mut self, item_id: i32) -> Vec<Item> {
        let mut items_updated = Vec::new();

        for item in &mut self.items.iter_mut() {
            if item.id == item_id {
                item.equipped = false;
                items_updated.push(item.clone());
            }
        }

        return items_updated;
    }

    pub fn update_quantity(&mut self, owner: i32, name: String, mod_quantity: i32) -> Option<Item> {
        if let Some(index) = self
            .items
            .iter()
            .position(|item| item.owner == owner && item.name == name)
        {
            let item = &mut self.items[index];
            item.quantity += mod_quantity;
            return Some(item.clone());
        } else {
            return None;
        }
    }

    pub fn update_quantity_by_class(
        &mut self,
        owner: i32,
        class: String,
        mod_quantity: i32,
    ) -> Option<(Item, ItemAction)> {
        if let Some(index) = self.find_by_class(owner, class) {
            let item = &mut self.items[index];
            debug!(
                "item quantity: {:?} mod_quantity: {:?}",
                item.quantity, mod_quantity
            );
            if (item.quantity + mod_quantity) > 0 {
                item.quantity += mod_quantity;
                return Some((item.clone(), ItemAction::Updated));
            } else {
                let removed_item = item.clone();
                debug!("Removing item {:?}", index);
                self.items.swap_remove(index);
                debug!("items: {:?}", self.items);
                return Some((removed_item, ItemAction::Removed)); // Return the item that was removed
            }
        } else {
            return None;
        }
    }

    pub fn update_durability(&mut self, item_id: i32, durability: i32) {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            if let Some(item_durability) = &item.durability {
                let new_durability = *item_durability - durability;
                item.durability = Some(new_durability);

                if new_durability <= 0 {
                    self.items.swap_remove(index);
                    return;
                }
            }
        } else {
            error!("Cannot find item: {:?}", item_id);
        }
    }

    /*pub fn set_experiment_source(&mut self, item_id: i32) -> Item {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            item.experiment = Some(ExperimentItemType::Source);
            return item.clone();
        } else {
            panic!("Cannot find item: {:?}", item_id);
        }
    }

    pub fn remove_experiment_source(&mut self, item_id: i32) -> Item {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            item.experiment = None;
            return item.clone();
        } else {
            panic!("Cannot find item: {:?}", item_id);
        }
    }

    pub fn set_experiment_reagent(&mut self, item_id: i32) {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            item.experiment = Some(ExperimentItemType::Reagent);
        } else {
            error!("Cannot find item: {:?}", item_id);
        }
    }

    pub fn remove_experiment_reagent(&mut self, item_id: i32) {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            let item = &mut self.items[index];

            item.experiment = None;
        } else {
            error!("Cannot find item: {:?}", item_id);
        }
    }

    pub fn get_experiment_details_packet(
        &self,
        structure_id: i32,
    ) -> (Vec<network::Item>, Vec<network::Item>, Vec<network::Item>) {
        let mut experiment_source: Vec<network::Item> = Vec::new();
        let mut experiment_reagents: Vec<network::Item> = Vec::new();
        let mut other_resources: Vec<network::Item> = Vec::new();

        for item in self.items.iter() {
            if item.owner == structure_id {
                if let Some(item_experiment_type) = &item.experiment {
                    if *item_experiment_type == ExperimentItemType::Reagent {
                        experiment_reagents.push(Item::to_packet(item.clone()));
                    } else if *item_experiment_type == ExperimentItemType::Source {
                        experiment_source.push(Item::to_packet(item.clone()));
                    }
                } else {
                    other_resources.push(Item::to_packet(item.clone()));
                }
            }
        }

        return (experiment_source, experiment_reagents, other_resources);
    }

    pub fn get_experiment_source_reagents(&self, structure_id: i32) -> (Option<Item>, Vec<Item>) {
        let mut experiment_source = None;
        let mut experiment_reagents = Vec::new();

        for item in self.items.iter() {
            if item.owner == structure_id {
                if let Some(item_experiment_type) = &item.experiment {
                    if *item_experiment_type == ExperimentItemType::Reagent {
                        experiment_reagents.push(item.clone());
                    } else if *item_experiment_type == ExperimentItemType::Source {
                        experiment_source = Some(item.clone());
                    }
                }
            }
        }

        return (experiment_source, experiment_reagents);
    }*/

    /*pub fn get_experiment_reagent(&self, structure_id: i32, subclass: String) -> Option<i32> {
        for item in self.items.iter() {
            if item.owner == structure_id
                && item.subclass == subclass
                && item.experiment == Some(ExperimentItemType::Reagent)
            {
                return Some(item.id);
            }
        }
        return None;
    }

    pub fn get_total_gold(&self, owner: i32) -> i32 {
        let mut total_gold = 0;

        for item in self.items.iter() {
            if item.owner == owner && item.class == GOLD.to_string() {
                total_gold += item.quantity;
            }
        }

        return total_gold;
    }*/

    /*pub fn transfer_gold(&mut self, owner: i32, target_id: i32, quantity: i32) {
        let mut remainder = quantity;
        let mut transfer_items = Vec::new();

        for item in &mut self.items.iter() {
            if item.owner == owner && item.class == GOLD.to_string() {
                if item.quantity >= remainder {
                    transfer_items.push((item.id, remainder));
                } else {
                    transfer_items.push((item.id, item.quantity));

                    remainder = remainder - item.quantity;
                }
            }
        }

        for (transfer_item_id, transfer_quantity) in transfer_items.iter() {
            self.transfer_quantity(*transfer_item_id, target_id, *transfer_quantity);
        }
    }*/

    pub fn has_refinable_items(&self, owner: i32) -> bool {
        for item in self.items.iter() {
            if item.owner == owner
                && (item.class == ORE
                    || item.class == LOG
                    || item.class == HIDE
                    || item.class == GAME_ANIMAL)
            {
                return true;
            }
        }
        return false;
    }

    pub fn get_items_value_by_attr(&self, owner: i32, attr: &AttrKey, equipped_only: bool) -> f32 {
        let mut item_values = 0.0;

        for item in self.items.iter() {
            if item.owner != owner {
                continue;
            }

            if equipped_only && !item.equipped {
                continue;
            }

            match item.attrs.get(&attr) {
                Some(item_value) => {
                    let val = match item_value {
                        AttrVal::Num(attr_val) => *attr_val,
                        _ => 0.0,
                    };
                    item_values += val;
                }
                None => item_values += 0.0,
            }
        }

        return item_values;
    }

    pub fn has_reqs(&self, owner: i32, source_req_items: Vec<ResReq>) -> bool {
        let owner_items = self.get_by_owner(owner);

        let mut req_items = source_req_items.clone();

        for req_item in req_items.iter_mut() {
            let mut req_quantity = req_item.quantity;

            for owner_item in owner_items.iter() {
                if req_item.req_type == owner_item.name
                    || req_item.req_type == owner_item.class
                    || req_item.req_type == owner_item.subclass
                {
                    if req_quantity - owner_item.quantity > 0 {
                        req_quantity -= owner_item.quantity;
                    } else {
                        req_quantity = 0;
                    }
                }
            }
            req_item.cquantity = Some(req_quantity);
        }

        for req_item in req_items.iter() {
            if let Some(current_req_quantity) = req_item.cquantity {
                info!("Current req quantity: {:?}", current_req_quantity);
                if current_req_quantity != 0 {
                    return false;
                }
            } else {
                // If cquantity is None
                return false;
            }
        }

        return true;
    }

    pub fn find_by_reqs(&self, owner: i32, source_req_items: Vec<ResReq>) -> Option<Vec<Item>> {
        let mut found_items = Vec::new();
        let owner_items = self.get_by_owner(owner);

        let mut req_items = source_req_items.clone();

        for req_item in req_items.iter_mut() {
            let mut req_quantity = req_item.quantity;

            for owner_item in owner_items.iter() {
                if req_item.req_type == owner_item.name
                    || req_item.req_type == owner_item.class
                    || req_item.req_type == owner_item.subclass
                {
                    if req_quantity - owner_item.quantity > 0 {
                        req_quantity -= owner_item.quantity;
                    } else {
                        req_quantity = 0;
                    }

                    found_items.push(owner_item.clone());
                }
            }
            req_item.cquantity = Some(req_quantity);
        }

        for req_item in req_items.iter() {
            if let Some(current_req_quantity) = req_item.cquantity {
                if current_req_quantity != 0 {
                    return None;
                }
            } else {
                // If cquantity is None
                return None;
            }
        }

        return Some(found_items);
    }

    // TODO reconsider returning the cloned item...
    pub fn find_by_id(&self, item_id: i32) -> Option<Item> {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            return Some(self.items[index].clone());
        }

        return None;
    }

    /*pub fn get_one_item_by_id(&mut self, item_id: i32) -> Option<(Item, Item)> {
        let item = self.find_by_id(item_id);

        if let Some(item) = item {
            if item.quantity > 1 {
                let Some((new_item, source_item)) = self.split(item_id, 1) else {
                    return None;
                };

                return Some((new_item, source_item));
            } else {
                return Some((item.clone(), item.clone()));
            }
        }

        return None;
    }*/

    pub fn get_mut_by_id(&mut self, item_id: i32) -> Option<&mut Item> {
        if let Some(index) = self.items.iter_mut().position(|item| item.id == item_id) {
            return Some(&mut self.items[index]);
        }

        return None;
    }

    pub fn find_index_by_id(&self, item_id: i32) -> Option<usize> {
        self.items.iter().position(|item| item.id == item_id)
    }

    pub fn find_expired_items(&self, game_tick: i32) -> Vec<Item> {
        let mut expired_items = Vec::new();
        for item in self.items.iter() {
            if item.start_time > 0 {
                let duration = match item.attrs.get(&AttrKey::Duration) {
                    Some(AttrVal::Num(duration)) => *duration as i32,
                    _ => 0,
                };

                /*info!(
                    "Start time: {:?}, Duration: {:?}, Game tick: {:?}",
                    item.start_time, duration, game_tick
                );*/
                if item.start_time + duration < game_tick {
                    //info!("Expired item: {:?}", item);
                    expired_items.push(item.clone());
                }
            }
        }
        return expired_items;
    }

    pub fn set_start_time(&mut self, item_id: i32, game_tick: i32) {
        if let Some(index) = self.items.iter_mut().position(|item| item.id == item_id) {
            self.items[index].start_time = game_tick;
        }
    }

    fn find_by_class(&self, owner: i32, class: String) -> Option<usize> {
        let index = self
            .items
            .iter()
            .position(|item| item.owner == owner && item.class == class);
        return index;
    }

    fn find_by_subclass(&self, owner: i32, subclass: String) -> Option<usize> {
        let index = self
            .items
            .iter()
            .position(|item| item.owner == owner && item.subclass == subclass);
        return index;
    }

    fn _get_next_id(&mut self) -> i32 {
        let next_id = self._next_id;
        self._next_id += 1;
        return next_id;
    }
}

impl Item {
    pub fn rarity(&self) -> ItemRarity {
        match self.attrs.get(&AttrKey::Rarity) {
            Some(AttrVal::Str(value)) => ItemRarity::from_str(value),
            _ => ItemRarity::Common,
        }
    }

    pub fn stack_identity_matches(&self, other: &Item) -> bool {
        self.name == other.name
            && self.attrs == other.attrs
            && self.durability == other.durability
            && self.experiment == other.experiment
            && self.equipped == other.equipped
    }

    pub fn attr_num(&self, attr: &AttrKey) -> f32 {
        match self.attrs.get(attr) {
            Some(AttrVal::Num(value)) => *value,
            _ => 0.0,
        }
    }

    pub fn is_gather_tool_for_attr(&self, attr: &AttrKey) -> bool {
        self.class != TORCH
            && self.class != ARMOR
            && self.slot.is_some()
            && self.attr_num(attr) > 0.0
    }

    pub fn is_gather_tool_for_res_type(&self, res_type: &str) -> bool {
        gather_tool_attr_for_res_type(res_type)
            .map(|attr| self.is_gather_tool_for_attr(&attr))
            .unwrap_or(false)
    }

    pub fn has_any_gather_tool_attr(&self) -> bool {
        [
            AttrKey::Mining,
            AttrKey::Logging,
            AttrKey::Stonecutting,
            AttrKey::Fishing,
            AttrKey::Farming,
            AttrKey::Foraging,
            AttrKey::Hunting,
        ]
        .iter()
        .any(|attr| self.is_gather_tool_for_attr(attr))
    }

    pub fn packet(&self) -> network::Item {
        return network::Item {
            id: self.id,
            owner: self.owner,
            name: self.name.clone(),
            quantity: self.quantity,
            durability: self.durability.clone(),
            class: self.class.clone(),
            subclass: self.subclass.clone(),
            slot: Slot::to_str(self.slot.clone()),
            image: self.image.clone(),
            weight: self.weight,
            equipped: self.equipped,
            refineable: self.produces.len() > 0,
            attrs: Some(self.attrs.clone()),
        };
    }

    pub fn to_packet(item: Item) -> network::Item {
        return network::Item {
            id: item.id,
            owner: item.owner,
            name: item.name.clone(),
            quantity: item.quantity,
            durability: item.durability.clone(),
            class: item.class.clone(),
            subclass: item.subclass.clone(),
            slot: Slot::to_str(item.slot),
            image: item.image.clone(),
            weight: item.weight,
            equipped: item.equipped,
            refineable: item.produces.len() > 0,
            attrs: Some(item.attrs),
        };
    }

    pub fn equipable(&self) -> bool {
        if self.class == WEAPON || self.class == ARMOR || self.class == TORCH || self.class == TOOL
        {
            return true;
        }
        if self.has_any_gather_tool_attr() {
            return true;
        }
        return false;
    }

    pub fn use_item(_item_id: i32, _status: bool, _items: &mut ResMut<Items>) {}

    pub fn is_req(item: Item, reqs: Vec<ResReq>) -> bool {
        for req in reqs.iter() {
            if req_matches(&req.req_type, &item.name, &item.class, &item.subclass) {
                return true;
            }
        }

        return false;
    }

    /// Like `is_req`, but understands explicit flexible construction
    /// requirements such as `Logs or Timber`.
    pub fn is_req_for_build(item: Item, reqs: Vec<ResReq>) -> bool {
        for req in reqs.iter() {
            if req_matches_build(&req.req_type, &item.name, &item.class, &item.subclass) {
                return true;
            }
        }

        return false;
    }

    pub fn get_weight_from_template(
        item_name: String,
        item_quantity: i32,
        item_templates: &Vec<ItemTemplate>,
    ) -> i32 {
        let item_template = Item::get_template(item_name, item_templates);

        return (item_quantity as f32 * item_template.weight) as i32;
    }

    pub fn get_template(item_name: String, item_templates: &Vec<ItemTemplate>) -> &ItemTemplate {
        for item_template in item_templates.iter() {
            if item_name == item_template.name {
                return item_template;
            }
        }

        panic!("Cannot find item template: {:?}", item_name);
    }

    pub fn find_template(
        item_name: String,
        item_templates: &Vec<ItemTemplate>,
    ) -> Option<&ItemTemplate> {
        for item_template in item_templates.iter() {
            if item_name == item_template.name {
                return Some(item_template);
            }
        }

        return None;
    }

    pub fn is_resource(item: Item) -> bool {
        match item.class.as_str() {
            ORE => true,
            LOG => true,
            STONE => true,
            INGOT => true,
            TIMBER => true,
            BLOCK => true,
            _ => false,
        }
    }

    fn can_merge_by_class(item_class: String) -> bool {
        match item_class.as_str() {
            WEAPON => false,
            ARMOR => false,
            CONTAINER => false,
            CORPSE_ITEM => false,
            _ => true,
        }
    }
}

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        let items = Items {
            items: Vec::new(),
            _next_id: 0,
            item_templates: Vec::new(),
        };

        app.insert_resource(items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flexible_wood_build_requirement_is_explicit_and_bidirectional() {
        assert!(req_matches_build(LOGS_OR_TIMBER, "Maple Log", LOG, LOG));
        assert!(req_matches_build(
            LOGS_OR_TIMBER,
            "Maple Timber",
            TIMBER,
            TIMBER
        ));
        assert!(!req_matches_build(
            LOGS_OR_TIMBER,
            "Fieldstone",
            STONE,
            STONE
        ));

        assert!(!req_matches_build(LOG, "Maple Timber", TIMBER, TIMBER));
        assert!(!req_matches_build(TIMBER, "Maple Log", LOG, LOG));
    }

    #[test]
    fn flexible_wood_build_requirement_counts_and_consumes_a_mixed_stack() {
        let mut logs = test_item(1, LOG, None, false, Vec::new());
        logs.name = "Maple Log".to_string();
        logs.quantity = 2;
        let mut timber = test_item(2, TIMBER, None, false, Vec::new());
        timber.name = "Maple Timber".to_string();
        timber.quantity = 3;
        let mut inventory = Inventory {
            owner: 1,
            items: vec![logs, timber],
        };
        let flexible_requirement = vec![ResReq {
            req_type: LOGS_OR_TIMBER.to_string(),
            quantity: 5,
            cquantity: None,
        }];

        assert_eq!(inventory.count_for_build_req(LOGS_OR_TIMBER), 5);
        assert!(inventory.has_reqs_for_build(flexible_requirement.clone()));
        assert!(!inventory.has_reqs_for_build(vec![ResReq {
            req_type: LOG.to_string(),
            quantity: 5,
            cquantity: None,
        }]));

        inventory.consume_reqs_for_build(flexible_requirement);
        assert!(inventory.items.is_empty());
    }

    #[test]
    fn human_corpse_items_remain_separate_when_transferred() {
        let mut first = test_item(1, CORPSE_ITEM, None, false, Vec::new());
        first.name = "Human Corpse".to_string();
        let mut second = test_item(2, CORPSE_ITEM, None, false, Vec::new());
        second.name = "Human Corpse".to_string();
        let mut source = Inventory {
            owner: 10,
            items: vec![first, second],
        };
        let mut target = Inventory {
            owner: 20,
            items: Vec::new(),
        };

        Inventory::transfer(1, &mut source, &mut target);
        Inventory::transfer(2, &mut source, &mut target);

        assert!(source.items.is_empty());
        assert_eq!(target.items.len(), 2);
        assert!(target.items.iter().all(|item| {
            item.name == "Human Corpse" && item.quantity == 1 && item.owner == target.owner
        }));
        assert_ne!(target.items[0].id, target.items[1].id);
    }

    #[test]
    fn gathering_tool_rating_reduces_work_time_without_instant_actions() {
        assert_eq!(gather_duration_ticks(30, 1.0), 300);
        assert_eq!(gather_duration_ticks(30, 2.0), 240);
        assert_eq!(gather_duration_ticks(30, 9.0), 120);
    }

    #[test]
    fn foraging_is_unassisted_at_base_speed_and_eight_seconds_with_a_kit() {
        assert_eq!(
            gather_duration_ticks_for_res_type(15, constants::FORAGE, None),
            150
        );
        assert_eq!(
            gather_duration_ticks_for_res_type(15, constants::FORAGE, Some(2.0)),
            80
        );
    }

    #[test]
    fn harvesting_tools_only_risk_breaking_in_low_durability_band() {
        assert_eq!(harvest_tool_break_chance(7, 30), 0.0);
        assert_eq!(harvest_tool_break_chance(6, 30), 0.10);
        assert_eq!(harvest_tool_break_chance(3, 30), 0.25);
        assert_eq!(harvest_tool_break_chance(0, 30), 1.0);
    }

    #[test]
    fn durability_use_decrements_then_removes_the_final_item() {
        let mut flint = test_item(10, IGNITION_TOOL, None, false, Vec::new());
        flint.name = "Flint Shard".to_string();
        flint.durability = Some(2);
        let mut inventory = Inventory {
            owner: 1,
            items: vec![flint],
        };

        assert!(inventory.get_usable_by_class(IGNITION_TOOL).is_some());
        assert!(matches!(
            inventory.consume_durability_use(10, 1, 2),
            Some(DurabilityUseOutcome::Updated(item)) if item.durability == Some(1)
        ));
        assert!(matches!(
            inventory.consume_durability_use(10, 1, 2),
            Some(DurabilityUseOutcome::Removed { id: 10, .. })
        ));
        assert!(inventory.get_by_id(10).is_none());
        assert!(inventory.get_usable_by_class(IGNITION_TOOL).is_none());
    }

    #[test]
    fn durability_use_advances_a_stack_to_a_fresh_item() {
        let mut flint = test_item(11, IGNITION_TOOL, None, false, Vec::new());
        flint.name = "Flint Shard".to_string();
        flint.quantity = 2;
        flint.durability = Some(1);
        let mut inventory = Inventory {
            owner: 1,
            items: vec![flint],
        };

        assert!(matches!(
            inventory.consume_durability_use(11, 1, 20),
            Some(DurabilityUseOutcome::Updated(item))
                if item.quantity == 1 && item.durability == Some(20)
        ));
    }

    #[test]
    fn splitting_legacy_equipment_stack_preserves_one_equipped_item() {
        let mut stacked_hatchet = test_item(
            20,
            WEAPON,
            Some(Slot::MainHand),
            true,
            vec![(AttrKey::Logging, 1.0)],
        );
        stacked_hatchet.name = "Crude Hatchet".to_string();
        stacked_hatchet.quantity = 2;
        stacked_hatchet.durability = Some(7);
        let mut inventory = Inventory {
            owner: 1,
            items: vec![stacked_hatchet],
        };

        let (spare, equipped) = inventory
            .split_instance_stack(20, 21, 1)
            .expect("legacy Hatchet stack should split");

        assert_eq!(equipped.id, 20);
        assert_eq!(equipped.quantity, 1);
        assert!(equipped.equipped);
        assert_eq!(equipped.durability, Some(7));
        assert_eq!(spare.id, 21);
        assert_eq!(spare.quantity, 1);
        assert!(!spare.equipped);
        assert_eq!(spare.durability, Some(7));
        assert_eq!(spare.attrs, equipped.attrs);
    }

    fn test_item(
        id: i32,
        class: &str,
        slot: Option<Slot>,
        equipped: bool,
        attrs: Vec<(AttrKey, f32)>,
    ) -> Item {
        Item {
            id,
            owner: 1,
            name: format!("Item {}", id),
            quantity: 1,
            durability: None,
            class: class.to_string(),
            subclass: class.to_string(),
            slot,
            image: "item.png".to_string(),
            weight: 1.0,
            equipped,
            experiment: None,
            start_time: 0,
            attrs: attrs
                .into_iter()
                .map(|(key, value)| (key, AttrVal::Num(value)))
                .collect(),
            produces: Vec::new(),
        }
    }

    #[test]
    fn required_tool_attr_maps_gather_resources() {
        assert_eq!(required_tool_attr_for_res_type(ORE), Some(AttrKey::Mining));
        assert_eq!(required_tool_attr_for_res_type(LOG), Some(AttrKey::Logging));
        assert_eq!(
            required_tool_attr_for_res_type(STONE),
            Some(AttrKey::Stonecutting)
        );
        assert_eq!(
            required_tool_attr_for_res_type(constants::FISH),
            Some(AttrKey::Fishing)
        );
        assert_eq!(
            required_tool_attr_for_res_type(constants::FOOD),
            Some(AttrKey::Farming)
        );
        assert_eq!(required_tool_attr_for_res_type(constants::FORAGE), None);
        assert_eq!(required_tool_attr_for_res_type(constants::PLANT), None);
        assert_eq!(
            gather_tool_attr_for_res_type(constants::FORAGE),
            Some(AttrKey::Foraging)
        );
        assert_eq!(
            required_tool_attr_for_res_type(constants::GAME_ANIMAL),
            Some(AttrKey::Hunting)
        );
    }

    #[test]
    fn gathering_affix_on_armor_does_not_turn_it_into_a_tool() {
        let armor = test_item(
            12,
            ARMOR,
            Some(Slot::Chest),
            true,
            vec![(AttrKey::Hunting, 3.0)],
        );
        assert!(!armor.is_gather_tool_for_attr(&AttrKey::Hunting));
    }

    #[test]
    fn auto_equip_best_tool_requires_strictly_better_matching_stat() {
        let mut inventory = Inventory {
            owner: 1,
            items: vec![
                test_item(
                    1,
                    TOOL,
                    Some(Slot::MainHand),
                    true,
                    vec![(AttrKey::Mining, 2.0)],
                ),
                test_item(
                    2,
                    TOOL,
                    Some(Slot::MainHand),
                    false,
                    vec![(AttrKey::Mining, 2.0)],
                ),
            ],
        };

        assert!(inventory
            .auto_equip_item_for_context(2, Some(ORE))
            .is_empty());
        assert!(inventory.get_by_id(1).unwrap().equipped);
        assert!(!inventory.get_by_id(2).unwrap().equipped);

        inventory.items.push(test_item(
            3,
            TOOL,
            Some(Slot::MainHand),
            false,
            vec![(AttrKey::Mining, 3.0)],
        ));

        let updated = inventory.auto_equip_item_for_context(3, Some(ORE));
        assert_eq!(updated.len(), 2);
        assert!(!inventory.get_by_id(1).unwrap().equipped);
        assert!(inventory.get_by_id(3).unwrap().equipped);

        inventory.items.push(test_item(
            4,
            TORCH,
            Some(Slot::MainHand),
            false,
            vec![(AttrKey::Mining, 5.0)],
        ));

        assert!(inventory
            .auto_equip_best_tool_for_attr(&AttrKey::Mining)
            .is_empty());
        assert!(inventory.get_by_id(3).unwrap().equipped);
        assert!(!inventory.get_by_id(4).unwrap().equipped);
    }

    #[test]
    fn auto_equip_armor_by_slot_and_weapon_only_without_gather_requirement() {
        let mut inventory = Inventory {
            owner: 1,
            items: vec![
                test_item(
                    1,
                    ARMOR,
                    Some(Slot::Chest),
                    true,
                    vec![(AttrKey::Defense, 1.0)],
                ),
                test_item(
                    2,
                    ARMOR,
                    Some(Slot::Chest),
                    false,
                    vec![(AttrKey::Defense, 2.0)],
                ),
                test_item(
                    3,
                    WEAPON,
                    Some(Slot::MainHand),
                    false,
                    vec![(AttrKey::Damage, 4.0)],
                ),
            ],
        };

        inventory.auto_equip_item_for_context(2, Some(ORE));
        assert!(!inventory.get_by_id(1).unwrap().equipped);
        assert!(inventory.get_by_id(2).unwrap().equipped);

        assert!(inventory
            .auto_equip_item_for_context(3, Some(ORE))
            .is_empty());
        assert!(!inventory.get_by_id(3).unwrap().equipped);

        inventory.auto_equip_item_for_context(3, Some(HIDE));
        assert!(inventory.get_by_id(3).unwrap().equipped);

        inventory.unequip(3);
        inventory.auto_equip_item_for_context(3, None);
        assert!(inventory.get_by_id(3).unwrap().equipped);
    }

    #[test]
    fn test_transfer_partial_resources_full_transfer_when_capacity_available() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Copper Ore".to_string(),
                quantity: 10,
                durability: None,
                class: ORE.to_string(),
                subclass: "Ore".to_string(),
                slot: None,
                image: "copper_ore.png".to_string(),
                weight: 2.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = Vec::new();
        let target_capacity = 100;
        let mut ids = Ids::default();
        ids.item = 99;

        Inventory::transfer_partial_resources(
            &mut source_inventory,
            &mut target_inventory,
            &mut ids,
            target_capacity,
            &item_templates,
        );

        // Source should be empty
        assert_eq!(source_inventory.items.len(), 0);

        // Target should have the full item
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].quantity, 10);
        assert_eq!(target_inventory.items[0].owner, 2);
    }

    #[test]
    fn test_transfer_partial_resources_partial_transfer_when_limited_capacity() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Maple Log".to_string(),
                quantity: 20,
                durability: None,
                class: LOG.to_string(),
                subclass: "Log".to_string(),
                slot: None,
                image: "maple_log.png".to_string(),
                weight: 3.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![ItemTemplate {
            name: "Maple Log".to_string(),
            class: LOG.to_string(),
            subclass: "Log".to_string(),
            image: "maple_log.png".to_string(),
            weight: 3.0,
            slot: None,
            durability: None,
            refine_skill: None,
            refine_skill_req: None,
            refine_time: None,
            produces: None,
            duration: None,
            attrs: None,
        }];

        let target_capacity = 15; // Only fits 5 items (5 * 3.0 = 15)
        let mut ids = Ids::default();
        ids.item = 99;

        Inventory::transfer_partial_resources(
            &mut source_inventory,
            &mut target_inventory,
            &mut ids,
            target_capacity,
            &item_templates,
        );

        // Source should have remaining items
        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].quantity, 15);

        // Target should have partial transfer
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].quantity, 5);
        assert_eq!(target_inventory.items[0].owner, 2);
        assert!(target_inventory.get_total_weight() <= target_capacity);
    }

    #[test]
    fn test_transfer_partial_resources_multiple_items_with_capacity_limit() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
                    id: 1,
                    owner: 1,
                    name: "Iron Ore".to_string(),
                    quantity: 5,
                    durability: None,
                    class: ORE.to_string(),
                    subclass: "Ore".to_string(),
                    slot: None,
                    image: "iron_ore.png".to_string(),
                    weight: 2.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 2,
                    owner: 1,
                    name: "Stone".to_string(),
                    quantity: 10,
                    durability: None,
                    class: STONE.to_string(),
                    subclass: "Stone".to_string(),
                    slot: None,
                    image: "stone.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![ItemTemplate {
            name: "Stone".to_string(),
            class: STONE.to_string(),
            subclass: "Stone".to_string(),
            image: "stone.png".to_string(),
            weight: 1.0,
            slot: None,
            durability: None,
            refine_skill: None,
            refine_skill_req: None,
            refine_time: None,
            produces: None,
            duration: None,
            attrs: None,
        }];

        let target_capacity = 15; // Fits all ore (10) + 5 stone
        let mut ids = Ids::default();
        ids.item = 99;

        Inventory::transfer_partial_resources(
            &mut source_inventory,
            &mut target_inventory,
            &mut ids,
            target_capacity,
            &item_templates,
        );

        // Source should have remaining stone
        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].name, "Stone");
        assert_eq!(source_inventory.items[0].quantity, 5);

        // Target should have all ore + partial stone
        assert_eq!(target_inventory.items.len(), 2);
    }

    #[test]
    fn test_transfer_partial_resources_no_transfer_when_no_capacity() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Hide".to_string(),
                quantity: 10,
                durability: None,
                class: HIDE.to_string(),
                subclass: "Hide".to_string(),
                slot: None,
                image: "hide.png".to_string(),
                weight: 1.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = Vec::new();
        let target_capacity = 0;
        let mut ids = Ids::default();
        ids.item = 99;

        Inventory::transfer_partial_resources(
            &mut source_inventory,
            &mut target_inventory,
            &mut ids,
            target_capacity,
            &item_templates,
        );

        // Source should be unchanged
        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].quantity, 10);

        // Target should be empty
        assert_eq!(target_inventory.items.len(), 0);
    }

    #[test]
    fn test_transfer_partial_resources_skips_non_resource_items() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
                    id: 1,
                    owner: 1,
                    name: "Iron Sword".to_string(),
                    quantity: 1,
                    durability: None,
                    class: WEAPON.to_string(),
                    subclass: "Sword".to_string(),
                    slot: None,
                    image: "iron_sword.png".to_string(),
                    weight: 5.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 2,
                    owner: 1,
                    name: "Copper Ore".to_string(),
                    quantity: 5,
                    durability: None,
                    class: ORE.to_string(),
                    subclass: "Ore".to_string(),
                    slot: None,
                    image: "copper_ore.png".to_string(),
                    weight: 2.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = Vec::new();
        let target_capacity = 100;
        let mut ids = Ids::default();
        ids.item = 99;

        Inventory::transfer_partial_resources(
            &mut source_inventory,
            &mut target_inventory,
            &mut ids,
            target_capacity,
            &item_templates,
        );

        // Weapon should remain in source
        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].class, WEAPON);

        // Only ore should be in target
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].class, ORE);
    }

    // Tests for transfer function
    #[test]
    fn test_transfer_stackable_item_merges_with_existing() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Maple Log".to_string(),
                quantity: 5,
                durability: None,
                class: LOG.to_string(),
                subclass: "Log".to_string(),
                slot: None,
                image: "maple_log.png".to_string(),
                weight: 1.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![Item {
                id: 2,
                owner: 2,
                name: "Maple Log".to_string(),
                quantity: 3,
                durability: None,
                class: LOG.to_string(),
                subclass: "Log".to_string(),
                slot: None,
                image: "maple_log.png".to_string(),
                weight: 1.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        Inventory::transfer(1, &mut source_inventory, &mut target_inventory);

        // Source should be empty
        assert_eq!(source_inventory.items.len(), 0);

        // Target should have merged item
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].id, 2);
        assert_eq!(target_inventory.items[0].quantity, 8);
    }

    #[test]
    fn test_transfer_stackable_item_creates_new_stack() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Iron Ore".to_string(),
                quantity: 10,
                durability: None,
                class: ORE.to_string(),
                subclass: "Ore".to_string(),
                slot: None,
                image: "iron_ore.png".to_string(),
                weight: 2.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer(1, &mut source_inventory, &mut target_inventory);

        // Source should be empty
        assert_eq!(source_inventory.items.len(), 0);

        // Target should have the item with updated owner
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].owner, 2);
        assert_eq!(target_inventory.items[0].quantity, 10);
    }

    #[test]
    fn test_transfer_non_stackable_item_updates_owner() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Iron Sword".to_string(),
                quantity: 1,
                durability: Some(100),
                class: WEAPON.to_string(),
                subclass: "Sword".to_string(),
                slot: Some(Slot::MainHand),
                image: "iron_sword.png".to_string(),
                weight: 5.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer(1, &mut source_inventory, &mut target_inventory);

        // Source should be empty
        assert_eq!(source_inventory.items.len(), 0);

        // Target should have the weapon as its new owner.
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].id, 1);
        assert_eq!(target_inventory.items[0].owner, 2);
    }

    #[test]
    fn test_transfer_clears_equipped_state_when_owner_changes() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![test_item(1, WEAPON, Some(Slot::MainHand), true, vec![])],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer(1, &mut source_inventory, &mut target_inventory);

        assert!(source_inventory.items.is_empty());
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].owner, 2);
        assert!(!target_inventory.items[0].equipped);
    }

    #[test]
    fn test_transfer_all_unequipped_items_skips_equipped_items() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                test_item(1, WEAPON, Some(Slot::MainHand), true, vec![]),
                test_item(2, ORE, None, false, vec![]),
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer_all_unequipped_items(&mut source_inventory, &mut target_inventory);

        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].id, 1);
        assert!(source_inventory.items[0].equipped);
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].id, 2);
        assert_eq!(target_inventory.items[0].owner, 2);
    }

    // Tests for transfer_quantity function
    #[test]
    fn test_transfer_quantity_partial_transfer() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Copper Ore".to_string(),
                quantity: 20,
                durability: None,
                class: ORE.to_string(),
                subclass: "Ore".to_string(),
                slot: None,
                image: "copper_ore.png".to_string(),
                weight: 2.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![ItemTemplate {
            name: "Copper Ore".to_string(),
            class: ORE.to_string(),
            subclass: "Ore".to_string(),
            image: "copper_ore.png".to_string(),
            weight: 2.0,
            slot: None,
            durability: None,
            refine_skill: None,
            refine_skill_req: None,
            refine_time: None,
            produces: None,
            duration: None,
            attrs: None,
        }];

        let result = Inventory::transfer_quantity(
            1,
            100,
            &mut source_inventory,
            &mut target_inventory,
            7,
            &item_templates,
        );

        // Source should have remaining items
        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].quantity, 13);

        // Target should have transferred quantity
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].quantity, 7);
        assert_eq!(target_inventory.items[0].owner, 2);

        // Result should contain source item
        assert!(result.is_some());
        let remaining_item = result.unwrap();
        assert_eq!(remaining_item.quantity, 13);
    }

    #[test]
    fn test_transfer_quantity_full_transfer_when_exact_amount() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Stone".to_string(),
                quantity: 10,
                durability: None,
                class: STONE.to_string(),
                subclass: "Stone".to_string(),
                slot: None,
                image: "stone.png".to_string(),
                weight: 1.0,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = Vec::new();

        let result = Inventory::transfer_quantity(
            1,
            100,
            &mut source_inventory,
            &mut target_inventory,
            10,
            &item_templates,
        );

        // Source should be empty
        assert_eq!(source_inventory.items.len(), 0);

        // Target should have all items
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].quantity, 10);

        // Result should be None (no remaining items)
        assert!(result.is_none());
    }

    // Tests for transfer_all_items function
    #[test]
    fn test_transfer_all_items_transfers_everything() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
                    id: 1,
                    owner: 1,
                    name: "Iron Ore".to_string(),
                    quantity: 5,
                    durability: None,
                    class: ORE.to_string(),
                    subclass: "Ore".to_string(),
                    slot: None,
                    image: "iron_ore.png".to_string(),
                    weight: 2.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 2,
                    owner: 1,
                    name: "Iron Sword".to_string(),
                    quantity: 1,
                    durability: Some(100),
                    class: WEAPON.to_string(),
                    subclass: "Sword".to_string(),
                    slot: Some(Slot::MainHand),
                    image: "iron_sword.png".to_string(),
                    weight: 5.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer_all_items(&mut source_inventory, &mut target_inventory);

        // Source should be empty
        assert_eq!(source_inventory.items.len(), 0);

        // Target should have all items
        assert_eq!(target_inventory.items.len(), 2);
    }

    #[test]
    fn test_transfer_all_items_with_empty_source() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer_all_items(&mut source_inventory, &mut target_inventory);

        // Both should remain empty
        assert_eq!(source_inventory.items.len(), 0);
        assert_eq!(target_inventory.items.len(), 0);
    }

    // Tests for transfer_all_items_by_type function
    #[test]
    fn test_transfer_all_items_by_type_filters_correctly() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
                    id: 1,
                    owner: 1,
                    name: "Iron Ore".to_string(),
                    quantity: 5,
                    durability: None,
                    class: ORE.to_string(),
                    subclass: "Ore".to_string(),
                    slot: None,
                    image: "iron_ore.png".to_string(),
                    weight: 2.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 2,
                    owner: 1,
                    name: "Iron Sword".to_string(),
                    quantity: 1,
                    durability: Some(100),
                    class: WEAPON.to_string(),
                    subclass: "Sword".to_string(),
                    slot: Some(Slot::MainHand),
                    image: "iron_sword.png".to_string(),
                    weight: 5.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 3,
                    owner: 1,
                    name: "Copper Ore".to_string(),
                    quantity: 10,
                    durability: None,
                    class: ORE.to_string(),
                    subclass: "Ore".to_string(),
                    slot: None,
                    image: "copper_ore.png".to_string(),
                    weight: 2.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer_all_items_by_type(
            &mut source_inventory,
            &mut target_inventory,
            ORE.to_string(),
        );

        // Source should only have weapon
        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].class, WEAPON);

        // Target should have both ores
        assert_eq!(target_inventory.items.len(), 2);
        assert!(target_inventory.items.iter().all(|item| item.class == ORE));
    }

    // Tests for transfer_all_resources function
    #[test]
    fn test_transfer_all_resources_transfers_only_resources() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
                    id: 1,
                    owner: 1,
                    name: "Iron Ore".to_string(),
                    quantity: 5,
                    durability: None,
                    class: ORE.to_string(),
                    subclass: "Ore".to_string(),
                    slot: None,
                    image: "iron_ore.png".to_string(),
                    weight: 2.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 2,
                    owner: 1,
                    name: "Maple Log".to_string(),
                    quantity: 10,
                    durability: None,
                    class: LOG.to_string(),
                    subclass: "Log".to_string(),
                    slot: None,
                    image: "maple_log.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 3,
                    owner: 1,
                    name: "Stone".to_string(),
                    quantity: 15,
                    durability: None,
                    class: STONE.to_string(),
                    subclass: "Stone".to_string(),
                    slot: None,
                    image: "stone.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 4,
                    owner: 1,
                    name: "Hide".to_string(),
                    quantity: 8,
                    durability: None,
                    class: HIDE.to_string(),
                    subclass: "Hide".to_string(),
                    slot: None,
                    image: "hide.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 5,
                    owner: 1,
                    name: "Iron Sword".to_string(),
                    quantity: 1,
                    durability: Some(100),
                    class: WEAPON.to_string(),
                    subclass: "Sword".to_string(),
                    slot: Some(Slot::MainHand),
                    image: "iron_sword.png".to_string(),
                    weight: 5.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer_all_resources(&mut source_inventory, &mut target_inventory);

        // Source should only have weapon
        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].class, WEAPON);

        // Target should have all resources (ore, log, stone, hide)
        assert_eq!(target_inventory.items.len(), 4);
        assert!(target_inventory.items.iter().all(|item| item.class == ORE
            || item.class == LOG
            || item.class == STONE
            || item.class == HIDE));
    }

    // Tests for transfer_all_refined function
    #[test]
    fn test_transfer_all_refined_transfers_only_refined() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
                    id: 1,
                    owner: 1,
                    name: "Iron Ingot".to_string(),
                    quantity: 5,
                    durability: None,
                    class: INGOT.to_string(),
                    subclass: "Ingot".to_string(),
                    slot: None,
                    image: "iron_ingot.png".to_string(),
                    weight: 2.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 2,
                    owner: 1,
                    name: "Maple Timber".to_string(),
                    quantity: 10,
                    durability: None,
                    class: TIMBER.to_string(),
                    subclass: "Timber".to_string(),
                    slot: None,
                    image: "maple_timber.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 3,
                    owner: 1,
                    name: "Iron Dust".to_string(),
                    quantity: 3,
                    durability: None,
                    class: DUST.to_string(),
                    subclass: "Dust".to_string(),
                    slot: None,
                    image: "iron_dust.png".to_string(),
                    weight: 0.5,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 4,
                    owner: 1,
                    name: "Iron Ore".to_string(),
                    quantity: 20,
                    durability: None,
                    class: ORE.to_string(),
                    subclass: "Ore".to_string(),
                    slot: None,
                    image: "iron_ore.png".to_string(),
                    weight: 2.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer_all_refined(&mut source_inventory, &mut target_inventory);

        // Source should only have ore
        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].class, ORE);

        // Target should have all refined items (ingot, timber, dust)
        assert_eq!(target_inventory.items.len(), 3);
        assert!(target_inventory
            .items
            .iter()
            .all(|item| item.class == INGOT || item.class == TIMBER || item.class == DUST));
    }

    // Tests for transfer_gold function
    #[test]
    fn test_transfer_gold_partial_from_single_stack() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Gold".to_string(),
                quantity: 100,
                durability: None,
                class: GOLD.to_string(),
                subclass: "Currency".to_string(),
                slot: None,
                image: "gold.png".to_string(),
                weight: 0.01,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![ItemTemplate {
            name: "Gold".to_string(),
            class: GOLD.to_string(),
            subclass: "Currency".to_string(),
            image: "gold.png".to_string(),
            weight: 0.01,
            slot: None,
            durability: None,
            refine_skill: None,
            refine_skill_req: None,
            refine_time: None,
            produces: None,
            duration: None,
            attrs: None,
        }];

        let mut next_item_id = 100;

        Inventory::transfer_gold(
            &mut source_inventory,
            &mut target_inventory,
            30,
            &mut next_item_id,
            &item_templates,
        );

        // Source should have remaining gold
        assert_eq!(source_inventory.items.len(), 1);
        assert_eq!(source_inventory.items[0].quantity, 70);

        // Target should have transferred gold
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].quantity, 30);
        assert_eq!(target_inventory.items[0].class, GOLD);
    }

    #[test]
    fn test_transfer_gold_from_multiple_stacks() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
                    id: 1,
                    owner: 1,
                    name: "Gold".to_string(),
                    quantity: 50,
                    durability: None,
                    class: GOLD.to_string(),
                    subclass: "Currency".to_string(),
                    slot: None,
                    image: "gold.png".to_string(),
                    weight: 0.01,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 2,
                    owner: 1,
                    name: "Gold".to_string(),
                    quantity: 75,
                    durability: None,
                    class: GOLD.to_string(),
                    subclass: "Currency".to_string(),
                    slot: None,
                    image: "gold.png".to_string(),
                    weight: 0.01,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![ItemTemplate {
            name: "Gold".to_string(),
            class: GOLD.to_string(),
            subclass: "Currency".to_string(),
            image: "gold.png".to_string(),
            weight: 0.01,
            slot: None,
            durability: None,
            refine_skill: None,
            refine_skill_req: None,
            refine_time: None,
            produces: None,
            duration: None,
            attrs: None,
        }];

        let mut next_item_id = 100;

        Inventory::transfer_gold(
            &mut source_inventory,
            &mut target_inventory,
            100,
            &mut next_item_id,
            &item_templates,
        );

        // Source should have remaining gold (total 125 - 100 = 25)
        let source_total: i32 = source_inventory
            .items
            .iter()
            .filter(|item| item.class == GOLD)
            .map(|item| item.quantity)
            .sum();
        assert_eq!(source_total, 25);

        // Target should have transferred gold
        let target_total: i32 = target_inventory
            .items
            .iter()
            .map(|item| item.quantity)
            .sum();
        assert_eq!(target_total, 100);
    }

    #[test]
    fn test_transfer_gold_exact_amount() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![Item {
                id: 1,
                owner: 1,
                name: "Gold".to_string(),
                quantity: 50,
                durability: None,
                class: GOLD.to_string(),
                subclass: "Currency".to_string(),
                slot: None,
                image: "gold.png".to_string(),
                weight: 0.01,
                equipped: false,
                experiment: None,
                start_time: 0,
                attrs: HashMap::new(),
                produces: Vec::new(),
            }],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![ItemTemplate {
            name: "Gold".to_string(),
            class: GOLD.to_string(),
            subclass: "Currency".to_string(),
            image: "gold.png".to_string(),
            weight: 0.01,
            slot: None,
            durability: None,
            refine_skill: None,
            refine_skill_req: None,
            refine_time: None,
            produces: None,
            duration: None,
            attrs: None,
        }];

        let mut next_item_id = 100;

        Inventory::transfer_gold(
            &mut source_inventory,
            &mut target_inventory,
            50,
            &mut next_item_id,
            &item_templates,
        );

        // Source should be empty
        assert_eq!(source_inventory.items.len(), 0);

        // Target should have all gold
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].quantity, 50);
    }

    fn production_test_item(
        id: i32,
        name: &str,
        class: &str,
        subclass: &str,
        quantity: i32,
        weight: f32,
    ) -> Item {
        Item {
            id,
            owner: 1,
            name: name.to_string(),
            quantity,
            durability: None,
            class: class.to_string(),
            subclass: subclass.to_string(),
            slot: None,
            image: name.to_lowercase(),
            weight,
            equipped: false,
            experiment: None,
            start_time: 0,
            attrs: HashMap::new(),
            produces: Vec::new(),
        }
    }

    fn production_test_recipe(amount: i32, weight: f32, req: Vec<ResReq>) -> Recipe {
        Recipe {
            name: "Output".to_string(),
            class: "Material".to_string(),
            subclass: "Output".to_string(),
            image: "output".to_string(),
            weight,
            durability: None,
            attrs: None,
            owner: 1,
            tier: None,
            slot: None,
            damage: None,
            speed: None,
            armor: None,
            crafting_time: Some(1),
            structure_req: None,
            stamina_req: None,
            skill_req: None,
            amount: Some(amount),
            req,
            item_name_from_req: None,
        }
    }

    fn production_test_template(
        name: &str,
        class: &str,
        weight: f32,
        produces: Option<Vec<String>>,
        attrs: Option<Vec<crate::templates::ItemAttr>>,
    ) -> ItemTemplate {
        ItemTemplate {
            name: name.to_string(),
            class: class.to_string(),
            subclass: class.to_string(),
            image: name.to_lowercase(),
            weight,
            durability: None,
            refine_skill: None,
            refine_skill_req: None,
            refine_time: None,
            produces,
            slot: None,
            duration: None,
            attrs,
        }
    }

    #[test]
    fn consume_reqs_uses_exact_quantities_across_split_stacks() {
        let mut first = production_test_item(1, "Wood", "Material", "Wood", 1, 1.0);
        first.attrs.insert(AttrKey::Damage, AttrVal::Num(1.0));
        let mut second = production_test_item(2, "Wood", "Material", "Wood", 1, 1.0);
        second.attrs.insert(AttrKey::Damage, AttrVal::Num(2.0));
        let mut inventory = Inventory {
            owner: 1,
            items: vec![first, second],
        };
        let req = vec![ResReq {
            req_type: "Wood".to_string(),
            quantity: 2,
            cquantity: None,
        }];

        let consumed = inventory
            .try_consume_reqs(&req)
            .expect("two split stacks satisfy the requirement");

        assert_eq!(consumed.iter().map(|item| item.quantity).sum::<i32>(), 2);
        assert!(inventory.items.is_empty());
    }

    #[test]
    fn overlapping_requirements_cannot_reuse_the_same_unit() {
        let inventory = Inventory {
            owner: 1,
            items: vec![production_test_item(
                1, "Stick", "Material", "Stick", 1, 1.0,
            )],
        };
        let req = vec![
            ResReq {
                req_type: "Material".to_string(),
                quantity: 1,
                cquantity: None,
            },
            ResReq {
                req_type: "Stick".to_string(),
                quantity: 1,
                cquantity: None,
            },
        ];

        assert!(!inventory.has_reqs(req));
        assert_eq!(inventory.items[0].quantity, 1);
    }

    #[test]
    fn rarity_roll_thresholds_are_bounded_by_danger() {
        assert_eq!(ItemRarity::from_loot_roll(20, 849), ItemRarity::Common);
        assert_eq!(ItemRarity::from_loot_roll(20, 850), ItemRarity::Uncommon);
        assert_eq!(ItemRarity::from_loot_roll(20, 985), ItemRarity::Magic);
        assert_eq!(ItemRarity::from_loot_roll(20, 999), ItemRarity::Rare);

        assert_eq!(ItemRarity::from_loot_roll(250, 600), ItemRarity::Uncommon);
        assert_eq!(ItemRarity::from_loot_roll(250, 880), ItemRarity::Magic);
        assert_eq!(ItemRarity::from_loot_roll(250, 980), ItemRarity::Rare);
    }

    #[test]
    fn ordinary_crafting_preserves_special_components() {
        let mut rare = production_test_item(1, "Wood", "Log", "Wood", 1, 1.0);
        rare.attrs.insert(
            AttrKey::Rarity,
            AttrVal::Str(ItemRarity::Rare.as_str().to_string()),
        );
        rare.attrs.insert(AttrKey::Damage, AttrVal::Num(3.0));
        let common = production_test_item(2, "Wood", "Log", "Wood", 1, 1.0);
        let mut inventory = Inventory {
            owner: 1,
            // Put Rare first to prove inventory ordering cannot consume it.
            items: vec![rare, common],
        };
        let recipe = production_test_recipe(
            1,
            1.0,
            vec![ResReq {
                req_type: "Wood".to_string(),
                quantity: 1,
                cquantity: None,
            }],
        );

        inventory
            .try_craft(10, 1, "Output".to_string(), &recipe, None, None, 20)
            .expect("Common input should satisfy ordinary crafting");

        assert!(inventory.get_by_id(1).is_some());
        assert!(inventory.get_by_id(2).is_none());
        assert_eq!(
            inventory.get_by_id(10).unwrap().rarity(),
            ItemRarity::Common
        );
    }

    #[test]
    fn signature_component_sets_rarity_and_adds_affix_to_recipe_base() {
        let mut rare = production_test_item(1, "Wood", "Log", "Wood", 1, 1.0);
        rare.attrs.insert(
            AttrKey::Rarity,
            AttrVal::Str(ItemRarity::Rare.as_str().to_string()),
        );
        rare.attrs.insert(
            AttrKey::Affixes,
            AttrVal::Str("Keen, Woodsman's".to_string()),
        );
        rare.attrs.insert(AttrKey::Damage, AttrVal::Num(3.0));
        rare.attrs.insert(AttrKey::Logging, AttrVal::Num(4.0));
        let mut inventory = Inventory {
            owner: 1,
            items: vec![rare],
        };
        let mut recipe = production_test_recipe(
            1,
            1.0,
            vec![ResReq {
                req_type: "Wood".to_string(),
                quantity: 1,
                cquantity: None,
            }],
        );
        recipe.class = WEAPON.to_string();
        recipe.attrs = Some(vec![crate::templates::ItemAttr {
            name: "Damage".to_string(),
            value: "9".to_string(),
        }]);

        let crafted = inventory
            .try_craft_with_signature(
                10,
                1,
                "Output".to_string(),
                &recipe,
                None,
                None,
                Some(1),
                20,
            )
            .expect("selected Rare component should be consumed");

        assert_eq!(crafted.rarity(), ItemRarity::Rare);
        assert_eq!(crafted.attr_num(&AttrKey::Damage), 12.0);
        assert_eq!(crafted.attr_num(&AttrKey::Logging), 4.0);
        assert!(inventory.get_by_id(1).is_none());
    }

    #[test]
    fn stack_transfer_does_not_merge_different_rarities() {
        let common = production_test_item(1, "bones", "Raw", "bones", 1, 1.0);
        let mut magic = production_test_item(2, "bones", "Raw", "bones", 1, 1.0);
        magic.attrs.insert(
            AttrKey::Rarity,
            AttrVal::Str(ItemRarity::Magic.as_str().to_string()),
        );
        let mut source = Inventory {
            owner: 1,
            items: vec![magic],
        };
        let mut target = Inventory {
            owner: 2,
            items: vec![common],
        };

        Inventory::transfer(2, &mut source, &mut target);

        assert_eq!(target.items.len(), 2);
        assert_eq!(
            target.items.iter().map(|item| item.quantity).sum::<i32>(),
            2
        );
    }

    #[test]
    fn transferred_item_does_not_merge_into_an_equipped_stack() {
        let incoming = production_test_item(2, "Crude Torch", TORCH, "Torch", 1, 1.0);
        let mut equipped_torch = incoming.clone();
        equipped_torch.id = 1;
        equipped_torch.owner = 2;
        equipped_torch.equipped = true;

        let mut source = Inventory {
            owner: 1,
            items: vec![incoming],
        };
        let mut target = Inventory {
            owner: 2,
            items: vec![equipped_torch],
        };

        Inventory::transfer(2, &mut source, &mut target);

        assert!(source.items.is_empty());
        assert_eq!(target.items.len(), 2);
        assert!(target.get_by_id(1).unwrap().equipped);
        assert_eq!(target.get_by_id(1).unwrap().quantity, 1);
        assert!(!target.get_by_id(2).unwrap().equipped);
        assert_eq!(target.get_by_id(2).unwrap().owner, 2);
    }

    #[test]
    fn craft_amount_is_deterministic_and_weight_is_per_unit() {
        let mut inventory = Inventory {
            owner: 1,
            items: Vec::new(),
        };
        let recipe = production_test_recipe(5, 2.0, Vec::new());

        let crafted = inventory
            .try_craft(10, 1, "Output".to_string(), &recipe, None, None, 10)
            .expect("five two-weight outputs exactly fit");

        assert_eq!(crafted.quantity, 5);
        assert_eq!(crafted.weight, 2.0);
        assert_eq!(inventory.get_total_weight(), 10);
    }

    #[test]
    fn crafted_item_does_not_merge_into_an_equipped_stack() {
        let mut equipped_stick =
            production_test_item(1, "Sharpened Stick", WEAPON, "Spear", 1, 10.0);
        equipped_stick.slot = Some(Slot::MainHand);
        equipped_stick.durability = Some(25);
        equipped_stick.equipped = true;

        let mut inventory = Inventory {
            owner: 1,
            items: vec![equipped_stick],
        };
        let mut recipe = production_test_recipe(1, 10.0, Vec::new());
        recipe.class = WEAPON.to_string();
        recipe.subclass = "Spear".to_string();
        recipe.slot = Some(Slot::MainHand);
        recipe.durability = Some(25);

        inventory
            .try_craft(
                2,
                1,
                "Sharpened Stick".to_string(),
                &recipe,
                None,
                None,
                100,
            )
            .expect("a second Sharpened Stick should fit");

        assert_eq!(inventory.items.len(), 2);
        assert_eq!(inventory.get_by_id(1).unwrap().quantity, 1);
        assert!(inventory.get_by_id(1).unwrap().equipped);
        assert_eq!(inventory.get_by_id(2).unwrap().quantity, 1);
        assert!(!inventory.get_by_id(2).unwrap().equipped);

        // Matching unequipped output can still join the unequipped stack.
        inventory
            .try_craft(
                3,
                1,
                "Sharpened Stick".to_string(),
                &recipe,
                None,
                None,
                100,
            )
            .expect("a third Sharpened Stick should fit");

        assert_eq!(inventory.items.len(), 2);
        assert_eq!(inventory.get_by_id(1).unwrap().quantity, 1);
        assert!(inventory.get_by_id(1).unwrap().equipped);
        assert_eq!(inventory.get_by_id(2).unwrap().quantity, 2);
        assert!(!inventory.get_by_id(2).unwrap().equipped);
    }

    #[test]
    fn failed_craft_capacity_check_leaves_inputs_untouched() {
        let mut inventory = Inventory {
            owner: 1,
            items: vec![production_test_item(1, "Wood", "Material", "Wood", 1, 1.0)],
        };
        let recipe = production_test_recipe(
            1,
            10.0,
            vec![ResReq {
                req_type: "Wood".to_string(),
                quantity: 1,
                cquantity: None,
            }],
        );

        assert!(matches!(
            inventory.try_craft(10, 1, "Output".to_string(), &recipe, None, None, 5),
            Err(CraftError::InventoryFull)
        ));
        assert_eq!(inventory.items.len(), 1);
        assert_eq!(inventory.items[0].name, "Wood");
        assert_eq!(inventory.items[0].quantity, 1);
    }

    #[test]
    fn failed_multi_output_refine_is_atomic() {
        let mut inventory = Inventory {
            owner: 1,
            items: vec![production_test_item(1, "Ore", "Ore", "Ore", 1, 1.0)],
        };
        let templates = vec![
            production_test_template(
                "Ore",
                "Ore",
                1.0,
                Some(vec!["Ingot".to_string(), "Dust".to_string()]),
                None,
            ),
            production_test_template("Ingot", "Ingot", 5.0, None, None),
            production_test_template("Dust", "Dust", 5.0, None, None),
        ];
        let mut ids = Ids::default();

        assert!(matches!(
            inventory.try_refine(1, 1, 6, &templates, &mut ids),
            Err(RefineError::InventoryFull)
        ));
        assert_eq!(inventory.items.len(), 1);
        assert_eq!(inventory.items[0].name, "Ore");
        assert_eq!(inventory.items[0].quantity, 1);
    }

    #[test]
    fn butchery_carries_carcass_rarity_to_hide_without_duplicating_it_to_food() {
        let mut carcass = production_test_item(1, "Carcass", GAME_ANIMAL, "Deer", 1, 1.0);
        carcass.produces = vec!["Raw Meat".to_string(), "Raw Hide".to_string()];
        carcass.attrs.insert(
            AttrKey::Rarity,
            AttrVal::Str(ItemRarity::Magic.as_str().to_string()),
        );
        carcass.attrs.insert(AttrKey::Defense, AttrVal::Num(2.0));
        let mut inventory = Inventory {
            owner: 1,
            items: vec![carcass],
        };
        let templates = vec![
            production_test_template(
                "Carcass",
                GAME_ANIMAL,
                1.0,
                Some(vec!["Raw Meat".to_string(), "Raw Hide".to_string()]),
                None,
            ),
            production_test_template("Raw Meat", ITEM_FOOD, 1.0, None, None),
            production_test_template("Raw Hide", HIDE, 1.0, None, None),
        ];
        let mut ids = Ids::default();

        inventory
            .try_refine(1, 1, 20, &templates, &mut ids)
            .expect("carcass should butcher");

        let meat = inventory
            .items
            .iter()
            .find(|item| item.name == "Raw Meat")
            .unwrap();
        let hide = inventory
            .items
            .iter()
            .find(|item| item.name == "Raw Hide")
            .unwrap();
        assert_eq!(meat.rarity(), ItemRarity::Common);
        assert_eq!(hide.rarity(), ItemRarity::Magic);
        assert_eq!(hide.attr_num(&AttrKey::Defense), 2.0);
    }

    #[test]
    fn repeated_refine_outputs_are_one_stack_with_a_combined_quantity() {
        let outputs = vec![
            "Raw Meat".to_string(),
            "Raw Meat".to_string(),
            "Raw Meat".to_string(),
            "Raw Meat".to_string(),
            "Raw Meat".to_string(),
            "Raw Meat".to_string(),
            "Raw Hide".to_string(),
        ];
        let mut inventory = Inventory {
            owner: 1,
            items: vec![production_test_item(
                1,
                "Carcass",
                GAME_ANIMAL,
                "Boar",
                1,
                1.0,
            )],
        };
        let templates = vec![
            production_test_template("Carcass", GAME_ANIMAL, 1.0, Some(outputs.clone()), None),
            production_test_template("Raw Meat", ITEM_FOOD, 1.0, None, None),
            production_test_template("Raw Hide", HIDE, 1.0, None, None),
        ];
        let mut ids = Ids::default();

        let outcome = inventory
            .try_refine(1, 1, 20, &templates, &mut ids)
            .expect("carcass should butcher");

        assert_eq!(outcome.produced.len(), 2);
        assert!(outcome
            .produced
            .iter()
            .any(|(item, quantity)| item.name == "Raw Meat" && *quantity == 6));
        assert_eq!(
            inventory
                .items
                .iter()
                .find(|item| item.name == "Raw Meat")
                .expect("combined meat stack")
                .quantity,
            6
        );

        let preview = produced_item_packets(&outputs, &templates);
        assert_eq!(preview.len(), 2);
        assert!(preview
            .iter()
            .any(|item| item.name == "Raw Meat" && item.quantity == 6));
    }

    /*#[test]
     fn consume_reqs_removes_quantity_and_collects_matching_items() {
        let mut items = Items {
            items: vec![
                Item {
                    id: 1,
                    owner: 42,
                    name: "Iron Sword".to_string(),
                    quantity: 5,
                    durability: None,
                    class: "Weapon".to_string(),
                    subclass: "Sword".to_string(),
                    slot: None,
                    image: "iron_sword.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 2,
                    owner: 100,
                    name: "Wood".to_string(),
                    quantity: 10,
                    durability: None,
                    class: "Resource".to_string(),
                    subclass: "Lumber".to_string(),
                    slot: None,
                    image: "wood.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
            ],
            _next_id: 3,
            item_templates: Vec::new(),
        };

        let reqs = vec![ResReq {
            req_type: "Weapon".to_string(),
            quantity: 3,
            cquantity: None,
        }];

        let consumed = items.consume_reqs(42, reqs);

        assert_eq!(consumed.len(), 1);
        assert_eq!(consumed[0].id, 1);
        assert_eq!(consumed[0].owner, 42);

        let updated_sword = items
            .items
            .iter()
            .find(|item| item.id == 1)
            .expect("Sword should remain with reduced quantity");
        assert_eq!(updated_sword.quantity, 2);

        let other_owner_item = items
            .items
            .iter()
            .find(|item| item.id == 2)
            .expect("Other owner's item should be untouched");
        assert_eq!(other_owner_item.quantity, 10);
    }

    #[test]
    fn transfer_merges_stackable_items_and_updates_owner_for_non_stackables() {
        let mut items = Items {
            items: vec![
                Item {
                    id: 1,
                    owner: 1,
                    name: "Maple Log".to_string(),
                    quantity: 5,
                    durability: None,
                    class: LOG.to_string(),
                    subclass: "Log".to_string(),
                    slot: None,
                    image: "maple_log.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 2,
                    owner: 2,
                    name: "Maple Log".to_string(),
                    quantity: 3,
                    durability: None,
                    class: LOG.to_string(),
                    subclass: "Log".to_string(),
                    slot: None,
                    image: "maple_log.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
                Item {
                    id: 3,
                    owner: 3,
                    name: "Iron Sword".to_string(),
                    quantity: 1,
                    durability: None,
                    class: WEAPON.to_string(),
                    subclass: "Sword".to_string(),
                    slot: None,
                    image: "iron_sword.png".to_string(),
                    weight: 2.5,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                },
            ],
            _next_id: 4,
            item_templates: Vec::new(),
        };

        items.transfer(1, 2);

        let merged_item = items
            .items
            .iter()
            .find(|item| item.id == 2)
            .expect("Merged item should exist");
        assert_eq!(merged_item.quantity, 8);
        assert!(items.items.iter().all(|item| item.id != 1));

        items.transfer(3, 4);

        let transferred_weapon = items
            .items
            .iter()
            .find(|item| item.id == 3)
            .expect("Weapon should still exist");
        assert_eq!(transferred_weapon.owner, 4);
    }*/
}
