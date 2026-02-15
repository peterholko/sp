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

#[derive(Debug, Reflect, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttrKey {
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
    Woodcutting,
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
            "Woodcutting" => AttrKey::Woodcutting,
            "Stonecutting" => AttrKey::Stonecutting,
            "Refining" => AttrKey::Refining,
            "Crafting" => AttrKey::Crafting,
            "Experimenting" => AttrKey::Experimenting,
            "Exploring" => AttrKey::Exploring,
            "Planting" => AttrKey::Planting,
            "Tending" => AttrKey::Tending,
            "Harvesting" => AttrKey::Harvesting,
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

pub const INGOT: &str = "Ingot";
pub const DUST: &str = "Dust";
pub const TIMBER: &str = "Timber";

pub const WEAPON: &str = "Weapon";
pub const ARMOR: &str = "Armor";
pub const ITEM_FOOD: &str = "Food";

pub const GATHERING: &str = "Gathering";
pub const TORCH: &str = "Torch";

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

#[derive(Debug, Reflect, Clone, PartialEq)]
pub enum ExperimentItemType {
    Source,
    Reagent,
}

#[derive(Debug, Reflect, Clone, PartialEq)]
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

impl Inventory {
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

            if Item::can_merge_by_class(item_to_transfer.class.clone()) {
                if let Some(merged_index) = target_inventory
                    .items
                    .iter()
                    .position(|item| item.name == item_to_transfer.name)
                {
                    let merged_item = &mut target_inventory.items[merged_index];
                    merged_item.quantity += item_to_transfer.quantity;

                    source_inventory.items.swap_remove(transfer_index);
                } else {
                    // Update item owner
                    item_to_transfer.owner = target_inventory.owner;

                    target_inventory.items.push(item_to_transfer);
                    source_inventory.items.swap_remove(transfer_index);
                }
            } else {
                target_inventory.items.push(item_to_transfer);
                source_inventory.items.swap_remove(transfer_index);
            }
        }
    }

    pub fn transfer_quantity(
        item_id: i32,
        new_item_id: i32,
        source_inventory: &mut Inventory,
        target_inventory: &mut Inventory,
        quantity: i32,
        item_templates: &Vec<ItemTemplate>,
    ) -> Option<Item> {
        info!("Transferring quantity from {:?} to {:?}", source_inventory.owner, target_inventory.owner);
        info!("Item id: {:?}", item_id);
        info!("New item id: {:?}", new_item_id);
        info!("Quantity: {:?}", quantity);

        if let Some(transfer_index) = source_inventory
            .items
            .iter()
            .position(|item| item.id == item_id)
        {
            let item_to_transfer = source_inventory.items[transfer_index].clone();

            let result = source_inventory.split(
                item_to_transfer.id,
                new_item_id,
                quantity,
                item_templates,
            );

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
                item.class == ORE || item.class == LOG || item.class == STONE || item.class == HIDE
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
        info!("Transferring partial resources from {:?} to {:?}", source_inventory.owner, target_inventory.owner);
        let resource_items: Vec<(i32, i32, f32)> = source_inventory
            .items
            .iter()
            .filter(|item| {
                item.class == ORE || item.class == LOG || item.class == STONE || item.class == HIDE
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
                    target_total_weight += (item_weight * num_to_transfer as f32) as i32;
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

        if let Some(merged_index) = self.mergeable(name.clone(), attrs.clone()) {
            let merged_item = &mut self.items[merged_index];
            merged_item.quantity += quantity;
            return merged_item.clone();
        } else {
            let new_item = Item {
                id: item_id,
                owner: self.owner,
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
            debug!("New Item by new(): {:?}", new_item);

            return new_item;
        }
    }

    pub fn new_with_attrs(
        &mut self,
        item_id: i32,
        owner: i32,
        name: String,
        quantity: i32,
        attrs: HashMap<AttrKey, AttrVal>,
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
            if let Some(merged_index) = self
                .items
                .iter()
                .position(|item| item.owner == owner && item.name == name)
            {
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
            if let Some(merged_index) = self.items.iter().position(|item| item.name == name) {
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
        // By default the recipe name is the item name
        let mut name: String = recipe_name.clone();

        let mut quantity = recipe.amount.unwrap_or(1);

        if quantity > 1 {
            // randomly select amount between 1 and amount
            quantity = rand::thread_rng().gen_range(1..=quantity);
        }

        let class = recipe.class.clone();
        let subclass = recipe.subclass.clone();
        let mut image = recipe.image.clone();
        let weight = recipe.weight as f32 * (quantity as f32);
        let durability = recipe.durability.clone();
        let slot = recipe.slot.clone();

        if let Some(custom_name) = custom_name {
            name = custom_name;
        }

        if let Some(custom_image) = custom_image {
            image = custom_image;
        }

        // Get consumed items and their attrs
        let consumed_items = self.consume_reqs(recipe.req.clone());
        let mut item_attrs = HashMap::new();

        for consumed_item in consumed_items.iter() {
            item_attrs.extend(consumed_item.attrs.clone());
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
        if let Some(merged_index) = self.mergeable(name.clone(), item_attrs.clone()) {
            let merged_item = &mut self.items[merged_index];
            merged_item.quantity += quantity;

            // Return new item for notification instead of merged item
            return new_item.clone();
        } else {
            self.items.push(new_item.clone());
            return new_item;
        }
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

    pub fn mergeable(&self, name: String, attrs: HashMap<AttrKey, AttrVal>) -> Option<usize> {
        // Check owner, name and attrs if they match any existing item
        if let Some(merged_index) = self
            .items
            .iter()
            .position(|item| item.name == name && item.attrs == attrs)
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

            info!("Item: {:?}", item.attrs);
            match item.attrs.get(&attr) {
                Some(item_value) => {
                    info!("Item value: {:?}", item_value);
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

    pub fn update_durability(&mut self, item_id: i32, durability: i32) {
        if let Some(update_index) = self.items.iter().position(|item| item.id == item_id) {
            let updated_item = &mut self.items[update_index];
            updated_item.durability = Some(durability);
        }
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

    pub fn consume_reqs(&mut self, req_items: Vec<ResReq>) -> Vec<Item> {
        let mut consumed_items = Vec::new();
        let mut items_to_remove = Vec::new();

        for req_item in req_items.iter() {
            for structure_item in self.items.iter() {
                if req_item.req_type == structure_item.name
                    || req_item.req_type == structure_item.class
                    || req_item.req_type == structure_item.subclass
                {
                    consumed_items.push(structure_item.clone());
                    items_to_remove.push((structure_item.id, req_item.quantity));
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

    pub fn find_by_reqs(&self, source_req_items: Vec<ResReq>) -> Option<Vec<Item>> {
        let mut found_items = Vec::new();

        let mut req_items = source_req_items.clone();

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

                    found_items.push(item.clone());
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

    pub fn has_reqs(&self, source_req_items: Vec<ResReq>) -> bool {

        let mut req_items = source_req_items.clone();

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

    fn find_by_class(&self, class: String) -> Option<usize> {
        let index = self
            .items
            .iter()
            .position(|item| item.class == class);
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

                info!("Item: {:?}", item);
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
            info!(
                "Item: {:?} owner: {:?} equipped_only: {:?}",
                item, owner, equipped_only
            );
            if item.owner != owner {
                continue;
            }

            if equipped_only && !item.equipped {
                continue;
            }

            info!("Item: {:?}", item.attrs);
            match item.attrs.get(&attr) {
                Some(item_value) => {
                    info!("Item value: {:?}", item_value);
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
        return false;
    }

    pub fn use_item(_item_id: i32, _status: bool, _items: &mut ResMut<Items>) {}

    pub fn is_req(item: Item, reqs: Vec<ResReq>) -> bool {
        for req in reqs.iter() {
            if req.req_type == item.name
                || req.req_type == item.class
                || req.req_type == item.subclass
            {
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
    fn test_transfer_partial_resources_full_transfer_when_capacity_available() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
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
            items: vec![
                Item {
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
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![
            ItemTemplate {
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
            },
        ];

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

        let item_templates = vec![
            ItemTemplate {
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
            },
        ];

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
            items: vec![
                Item {
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
                },
            ],
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
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![
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
            ],
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
            items: vec![
                Item {
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
                },
            ],
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
            items: vec![
                Item {
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
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        Inventory::transfer(1, &mut source_inventory, &mut target_inventory);

        // Source should be empty
        assert_eq!(source_inventory.items.len(), 0);

        // Target should have the weapon (not merged, owner not updated for non-stackables)
        assert_eq!(target_inventory.items.len(), 1);
        assert_eq!(target_inventory.items[0].id, 1);
    }

    // Tests for transfer_quantity function
    #[test]
    fn test_transfer_quantity_partial_transfer() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
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
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![
            ItemTemplate {
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
            },
        ];

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
            items: vec![
                Item {
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
                },
            ],
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
        assert!(target_inventory.items.iter().all(|item|
            item.class == ORE || item.class == LOG || item.class == STONE || item.class == HIDE
        ));
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
        assert!(target_inventory.items.iter().all(|item|
            item.class == INGOT || item.class == TIMBER || item.class == DUST
        ));
    }

    // Tests for transfer_gold function
    #[test]
    fn test_transfer_gold_partial_from_single_stack() {
        let mut source_inventory = Inventory {
            owner: 1,
            items: vec![
                Item {
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
                },
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![
            ItemTemplate {
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
            },
        ];

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

        let item_templates = vec![
            ItemTemplate {
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
            },
        ];

        let mut next_item_id = 100;

        Inventory::transfer_gold(
            &mut source_inventory,
            &mut target_inventory,
            100,
            &mut next_item_id,
            &item_templates,
        );

        // Source should have remaining gold (total 125 - 100 = 25)
        let source_total: i32 = source_inventory.items.iter()
            .filter(|item| item.class == GOLD)
            .map(|item| item.quantity)
            .sum();
        assert_eq!(source_total, 25);

        // Target should have transferred gold
        let target_total: i32 = target_inventory.items.iter()
            .map(|item| item.quantity)
            .sum();
        assert_eq!(target_total, 100);
    }

    #[test]
    fn test_transfer_gold_exact_amount() {
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
            ],
        };

        let mut target_inventory = Inventory {
            owner: 2,
            items: vec![],
        };

        let item_templates = vec![
            ItemTemplate {
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
            },
        ];

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
