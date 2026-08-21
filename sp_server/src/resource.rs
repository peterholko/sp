use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet};

use rand::distributions::Distribution;
use rand::distributions::WeightedIndex;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::constants::*;
use crate::ids::Ids;
use crate::item::{self, AttrKey, Inventory};
use crate::map::{Map, TileType};
use crate::network;
use crate::obj::Position;

use crate::skill::{self, SkillData, Skills};
use crate::skill_defs::Skill;
use crate::templates::{ItemTemplate, ResTemplate, ResTemplates, Templates};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Property {
    pub name: String,
    pub value: i32,
}

#[derive(Debug, Clone)]
pub struct Resource {
    pub name: String,
    pub image: String,
    pub res_type: String,
    pub pos: Position,
    pub max: i32,
    pub yield_level: i32,
    pub yield_mod: f32,
    pub quantity_level: i32,
    pub quantity: i32,
    pub properties: Vec<Property>,
    pub produces: Option<Vec<String>>,
    pub reveal: bool, //pub obj_id: Option<i32>,
}

#[derive(Debug, Clone)]
pub enum ResourceGatherError {
    NoResourcesAvailable,
    NoInventoryRoom,
    CannotFindResourceTemplate,
    NoItemGathered,
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct Resources(HashMap<Position, HashMap<String, Resource>>);

/// Resource deposits are part of the shared world, but ordinary prospecting
/// knowledge belongs to the player who found the deposit. `Resource::reveal`
/// remains the explicit world-visible escape hatch used by special systems
/// such as emergency spring discovery.
#[derive(Resource, Debug, Default)]
pub struct ResourceDiscoveries(HashMap<i32, HashMap<Position, HashSet<String>>>);

impl ResourceDiscoveries {
    pub fn is_discovered(&self, player_id: i32, pos: Position, resource_name: &str) -> bool {
        self.0
            .get(&player_id)
            .and_then(|tiles| tiles.get(&pos))
            .map(|resources| resources.contains(resource_name))
            .unwrap_or(false)
    }

    pub fn discover(&mut self, player_id: i32, pos: Position, resource_name: String) -> bool {
        self.0
            .entry(player_id)
            .or_default()
            .entry(pos)
            .or_default()
            .insert(resource_name)
    }

    pub fn clear_player(&mut self, player_id: i32) {
        self.0.remove(&player_id);
    }
}

impl Resources {
    pub fn get_by_type(&self, pos: Position, res_type: String, reveal: bool) -> Vec<Resource> {
        let mut resources = Vec::new();

        if let Some(resources_on_tile) = self.get(&pos) {
            for (_, resource) in resources_on_tile.iter() {
                if resource.res_type == res_type && resource.reveal == reveal {
                    resources.push(resource.clone());
                }
            }
        }

        return resources;
    }

    pub fn set_reveal(&mut self, pos: Position, res_type: String, reveal: bool) {
        if let Some(resources_on_tile) = self.get_mut(&pos) {
            for (_resource_name, resource) in resources_on_tile.iter_mut() {
                if resource.res_type == res_type {
                    resource.reveal = reveal;
                }
            }
        }
    }
}

impl Resource {
    pub fn scout_category_for_type(res_type: &str) -> Option<&'static str> {
        match res_type {
            ORE => Some("Ore"),
            STONE => Some("Stone"),
            LOG => Some("Timber"),
            FORAGE => Some("Forage"),
            SPRING_WATER => Some("Water"),
            FISH => Some("Fish"),
            GAME_ANIMAL => Some("Game"),
            _ => None,
        }
    }

    /// Return broad resource categories for the supplied tiles without
    /// revealing individual deposits to the player. Multiple named deposits
    /// of the same type collapse to one category icon per tile.
    pub fn get_scouted_resource_categories(
        positions: impl IntoIterator<Item = Position>,
        resources: &Resources,
    ) -> Vec<network::ScoutedResourceCategory> {
        let mut categories = Vec::new();

        for position in positions {
            let Some(resources_on_tile) = resources.get(&position) else {
                continue;
            };

            let mut tile_categories = resources_on_tile
                .values()
                .filter_map(|resource| Resource::scout_category_for_type(&resource.res_type))
                .collect::<Vec<_>>();
            tile_categories.sort_unstable();
            tile_categories.dedup();

            categories.extend(tile_categories.into_iter().map(|category| {
                network::ScoutedResourceCategory {
                    category: category.to_string(),
                    x: position.x,
                    y: position.y,
                }
            }));
        }

        categories.sort_by(|a, b| (a.y, a.x, &a.category).cmp(&(b.y, b.x, &b.category)));
        categories
    }

    pub fn is_visible_to(
        resource: &Resource,
        player_id: i32,
        discoveries: &ResourceDiscoveries,
    ) -> bool {
        resource.reveal
            || discoveries.is_discovered(player_id, resource.pos, resource.name.as_str())
    }

    pub fn spawn_all_resources(
        resources: &mut ResMut<Resources>,
        templates: &Templates,
        map: &Res<Map>,
    ) {
        let res_templates = &templates.res_templates;
        let res_property_templates = &templates.res_property_templates;

        let mut terrain_list: HashMap<String, Vec<ResTemplate>> = HashMap::new();
        let mut rng = rand::thread_rng();

        for (_resource_name, res_template) in res_templates.iter() {
            for terrain in res_template.terrain.iter() {
                match terrain_list.entry(terrain.to_string()) {
                    Vacant(entry) => {
                        let mut res_template_list = Vec::new();
                        res_template_list.push(res_template.clone());
                        entry.insert(res_template_list);
                    }
                    Occupied(entry) => {
                        entry.into_mut().push(res_template.clone());
                    }
                };
            }
        }

        for (index, tile_info) in map.base.iter().enumerate() {
            //debug!("{}", tile_info.tile_type.to_string().as_str());

            if let Some(res_template_list) =
                terrain_list.get(tile_info.tile_type.to_string().as_str())
            {
                for res_template in res_template_list.iter() {
                    // Randomize quantity
                    let dist = WeightedIndex::new(&res_template.quantity_rate).unwrap();

                    let sample = dist.sample(&mut rng);
                    let quantity = res_template.quantity[sample];
                    let quantity_level = sample as i32;

                    if quantity > 0 {
                        let pos = Map::index_to_pos(index);

                        // Randomize yield
                        let yield_dist = WeightedIndex::new(&res_template.yield_rate).unwrap();

                        let yield_sample = yield_dist.sample(&mut rng);
                        let yield_level = (yield_sample + 1) as i32;
                        let yield_mod = res_template.yield_mod[yield_sample];

                        let mut property_available_list = Vec::new();
                        let mut property_selected_list = Vec::new();

                        if let Some(properties) = &mut res_template.properties.clone() {
                            let mut num_properties = 1;

                            if let Some(num) = &res_template.num_properties {
                                num_properties = *num;
                            }

                            //debug!("num_properties: {:?}", num_properties);

                            for property in properties.iter() {
                                let property_templates =
                                    res_property_templates.get(property.to_string());

                                for property_template in property_templates.iter() {
                                    let level_range = &property_template.ranges
                                        [(res_template.level - 1) as usize];

                                    let min = level_range[0];
                                    let max = level_range[1];

                                    // Generate property value and round to 2 decimal places
                                    let property_value = rng.gen_range(min..=max);

                                    let property = Property {
                                        name: property_template.name.to_string(),
                                        value: property_value,
                                    };

                                    property_available_list.push(property);
                                }

                                // let index = rng.gen_range(0..characteristics.len());
                                // let characteristic_name = &characteristics[index];
                            }

                            //debug!("property_available_list: {:?}", property_available_list);
                            for _i in 0..num_properties {
                                if property_available_list.len() > 0 {
                                    let index = rng.gen_range(0..property_available_list.len());
                                    let selected_property = &property_available_list[index];

                                    //debug!("selected_property: {:?}", selected_property);
                                    property_selected_list.push(selected_property.clone());

                                    property_available_list.remove(index);
                                }
                            }
                        }

                        let terrain_name = tile_info.tile_type.to_string();
                        let produces = res_template
                            .produces_by_terrain
                            .as_ref()
                            .and_then(|outputs| outputs.get(&terrain_name).cloned())
                            .or_else(|| res_template.produces.clone());

                        Resource::create(
                            res_template.name.to_string(),
                            res_template.res_type.to_string(),
                            res_template.image.clone(),
                            yield_level,
                            yield_mod,
                            quantity_level,
                            quantity,
                            Position { x: pos.0, y: pos.1 },
                            property_selected_list,
                            produces,
                            resources,
                        );
                    }
                }
            }
        }
    }

    pub fn create(
        name: String,
        res_type: String,
        image: String,
        yield_level: i32,
        yield_mod: f32,
        quantity_level: i32,
        quantity: i32,
        position: Position,
        characteristics: Vec<Property>,
        produces: Option<Vec<String>>,
        resources: &mut Resources,
    ) {
        let resource = Resource {
            name: name.clone(),
            image: image.clone(),
            res_type: res_type.clone(),
            pos: position,
            max: quantity,
            yield_level: yield_level,
            yield_mod: yield_mod,
            quantity_level: quantity_level,
            quantity: quantity,
            properties: characteristics.clone(),
            produces: produces.clone(),
            reveal: false,
        };

        /*if characteristics.len() > 0 {
            debug!("{:?}", resource);
        }*/

        if let Some(resources_on_tile) = resources.get_mut(&position) {
            resources_on_tile.insert(name.clone(), resource);
        } else {
            let mut resources_on_tile = HashMap::new();

            resources_on_tile.insert(name.clone(), resource);

            resources.insert(position, resources_on_tile);
        }
    }

    pub fn get_on_tile(
        position: Position,
        resources: &Resources,
        discoveries: &ResourceDiscoveries,
        player_id: i32,
    ) -> Vec<network::TileResource> {
        let mut tile_resources = Vec::new();

        if let Some(resources_on_tile) = resources.get(&position) {
            for (resource_type, resource) in &*resources_on_tile {
                if Resource::is_visible_to(resource, player_id, discoveries) {
                    let tile_resource = network::TileResource {
                        name: resource_type.to_string(),
                        image: resource.image.clone(),
                        color: (resource.yield_level + resource.quantity_level) / 2,
                        yield_label: Resource::yield_level_to_label(resource.yield_level),
                        quantity_label: Resource::quantity_level_to_label(resource.quantity_level),
                        properties: resource.properties.clone(),
                    };

                    tile_resources.push(tile_resource);
                }
            }
        }

        return tile_resources;
    }

    pub fn get_nearby_resources(
        center: Position,
        resources: &Resources,
        discoveries: &ResourceDiscoveries,
        player_id: i32,
    ) -> Vec<network::TileResourceWithPos> {
        let mut tile_resources = Vec::new();

        let nearby_tiles = Map::range((center.x, center.y), 5);

        for (x, y) in nearby_tiles.iter() {
            let tile = Position { x: *x, y: *y };

            if let Some(resources_on_tile) = resources.get(&tile) {
                for (resource_type, resource) in &*resources_on_tile {
                    if Resource::is_visible_to(resource, player_id, discoveries) {
                        let tile_resource = network::TileResourceWithPos {
                            name: resource_type.to_string(),
                            image: resource.image.clone(),
                            color: (resource.yield_level + resource.quantity_level) / 2,
                            yield_label: Resource::yield_level_to_label(resource.yield_level),
                            quantity_label: Resource::quantity_level_to_label(
                                resource.quantity_level,
                            ),
                            x: *x,
                            y: *y,
                        };

                        tile_resources.push(tile_resource);
                    }
                }
            }
        }

        return tile_resources;
    }

    /// Resolve one successful gather into concrete item templates. Ordinary
    /// resource recipes keep all declared outputs, while hunting grounds and
    /// forage sites select one result from their available pool.
    pub fn gather_output_names<R: Rng + ?Sized>(resource: &Resource, rng: &mut R) -> Vec<String> {
        match resource.produces.as_ref() {
            Some(outputs) if matches!(resource.res_type.as_str(), GAME_ANIMAL | FORAGE) => {
                outputs.choose(rng).cloned().into_iter().collect::<Vec<_>>()
            }
            Some(outputs) if !outputs.is_empty() => outputs.clone(),
            _ => vec![resource.name.clone()],
        }
    }

    pub fn resource_color(yield_level: i32, quantity_level: i32) -> String {
        let total_level = (yield_level + quantity_level) / 2;

        match total_level {
            1 => "None".to_string(),
            2 => "None".to_string(),
            3 => "Green".to_string(),
            4 => "Blue".to_string(),
            5 => "Purple".to_string(),
            6 => "Orange".to_string(),
            7 => "Gold".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    pub fn yield_level_to_label(level: i32) -> String {
        match level {
            1 => "Worthless".to_string(),
            2 => "Meager".to_string(),
            3 => "Fair".to_string(),
            4 => "Outstanding".to_string(),
            5 => "Supreme".to_string(),
            6 => "Legendary".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    pub fn quantity_level_to_label(level: i32) -> String {
        match level {
            1 => "Inadequate".to_string(),
            2 => "Sparse".to_string(),
            3 => "Moderate".to_string(),
            4 => "Significant".to_string(),
            5 => "Pleantiful".to_string(),
            6 => "Immense".to_string(),
            7 => "Fabled".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    pub fn num_unrevealed_on_tile(
        position: Position,
        resources: &Resources,
        discoveries: &ResourceDiscoveries,
        player_id: i32,
    ) -> i32 {
        let mut num_unrevealed = 0;

        if let Some(resources_on_tile) = resources.get(&position) {
            for (_resource_type, resource) in &*resources_on_tile {
                if !Resource::is_visible_to(resource, player_id, discoveries) {
                    num_unrevealed += 1;
                }
            }
        }

        return num_unrevealed;
    }

    pub fn get_by_type(
        position: Position,
        res_type: String,
        resources: &Resources,
        reveal: bool,
    ) -> Vec<Resource> {
        if let Some(resources_on_tile) = resources.get(&position) {
            debug!(
                "Restype: {:?} Resources on tile: {:?}",
                res_type, resources_on_tile
            );

            return resources_on_tile
                .clone()
                .into_values()
                .filter(|x| x.reveal == reveal && x.res_type == res_type)
                .collect();
        }

        // Return empty vector
        return Vec::new();
    }

    pub fn get_by_type_for_player(
        position: Position,
        res_type: String,
        resources: &Resources,
        discoveries: &ResourceDiscoveries,
        player_id: i32,
    ) -> Vec<Resource> {
        resources
            .get(&position)
            .map(|resources_on_tile| {
                resources_on_tile
                    .values()
                    .filter(|resource| {
                        resource.res_type == res_type
                            && Resource::is_visible_to(resource, player_id, discoveries)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn forage(
        forager_id: i32,
        tile_type: TileType,
        new_item_id: i32,
        inventory: &mut Inventory,
        templates: &Templates,
    ) -> Result<Vec<network::Item>, ResourceGatherError> {
        let mut rng = rand::thread_rng();

        let mut items_to_update: Vec<network::Item> = Vec::new();

        /* ##################
        ### FORAGE TIER 0
        ##################

        Grasslands:
        - Stick
        - Plant Fibers
        - Edible Berries

        Plains:
        - Stick
        - Plant Fibers
        - Mushrooms (rare)

        Deciduous Forest:
        - Stick
        - Resin
        - Edible Berries
        - Mushrooms

        Pine Forest:
        - Stick
        - Resin
        - Pine Nuts (edible)

        Rainforest:
        - Stick
        - Resin
        - Exotic Fruit (edible)
        - Mushrooms (poison risk)

        Jungle:
        - Stick
        - Resin
        - Fruit
        - Mushrooms (higher poison risk)

        Frozen Forest:
        - Stick
        - Resin (low chance)
        - Edible Bark (emergency food)

        Snow Hills:
        - Stick (low chance)
        - Lichen (low Feed)
        - Resin (very rare)

        Desert:
        - Stick (very rare)
        - Cactus Fruit (hydration)
        - Dry Fiber (rope precursor)

        Rivers / Wetlands:
        - Reed
        - Stick
        - Mushrooms
        - Plant Fibers
        */

        let item_classes: Vec<String> = match tile_type {
            TileType::Grasslands => vec![
                STICK.to_string(),
                BERRIES.to_string(),
                PLANT_FIBERS.to_string(),
            ],
            TileType::Plains => vec![
                STICK.to_string(),
                PLANT_FIBERS.to_string(),
                MUSHROOM.to_string(),
            ],
            TileType::DeciduousForest => vec![
                STICK.to_string(),
                RESIN.to_string(),
                BERRIES.to_string(),
                MUSHROOM.to_string(),
                HONEY.to_string(),
            ],
            TileType::PineForest => {
                vec![STICK.to_string(), RESIN.to_string(), PINE_NUTS.to_string()]
            }
            TileType::FrozenForest => vec![STICK.to_string(), EDIBLE_BARK.to_string()],
            _ => {
                return Err(ResourceGatherError::CannotFindResourceTemplate);
            }
        };

        let item_class = &item_classes[rng.gen_range(0..item_classes.len())];
        info!("Foraged item class: {:?}", item_class);

        let mut item_templates = templates.get_item_templates_by_class(item_class);
        if item_templates.is_empty() {
            item_templates = templates.get_item_templates_by_subclass(item_class);
        }
        if item_templates.is_empty() {
            return Err(ResourceGatherError::CannotFindResourceTemplate);
        }
        info!("Foraged item templates: {:?}", item_templates);

        let item_template = &item_templates[rng.gen_range(0..item_templates.len())];
        info!("Foraged item template: {:?}", item_template);

        let item = inventory.new(
            new_item_id,
            item_template.name.clone(),
            1,
            &templates.item_templates,
        );
        info!("Foraged item: {:?}", item);
        items_to_update.push(item.packet());

        return Ok(items_to_update);
    }

    /*pub fn gather_by_type(
        gatherer_id: i32,
        gatherer_inventory: &mut Inventory,
        position: Position,
        res_type: String,
        skills: &mut Skills,
        capacity: i32,
        items: &mut Items,
        resources: &Resources,
        templates: &Templates,
    ) -> Result<(Vec<network::Item>, Vec<network::Xp>), ResourceGatherError> {
        let mut rng = rand::thread_rng();

        let resources_on_tile = Resource::get_by_type(position, res_type.clone(), resources, true);
        let res_templates = &templates.res_templates;
        let item_templates = &templates.item_templates;

        let mut items_to_update: Vec<network::Item> = Vec::new();
        let mut xp_list = Vec::new();

        info!("Resources on tile: {:?}", resources_on_tile);
        for resource in resources_on_tile.iter() {
            if let Some(res_template) = res_templates.get(&resource.name) {
                let skill_name = Resource::type_to_skill(res_type.clone());

                let mut skill_value = 0;

                if let Some(skill) = Skill::get_by_name(gatherer_id, skill_name.clone(), skills) {
                    skill_value = skill.level;
                }

                info!("Res template: {:?}", res_template);
                info!("Skill value: {:?}", skill_value);
                info!("Skill name: {:?}", skill_name);
                let gather_chance = Resource::gather_chance(skill_value, res_template.skill_req);

                let random_num = rng.gen::<f32>();

                info!("Gather chance: {:?}", gather_chance);
                info!("Random number: {:?}", random_num);

                if random_num < gather_chance {
                    info!("Gathering resource: {:?}", resource.name);
                    let resource_quantity = 1;

                    let current_total_weight = gatherer_inventory.get_total_weight();
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
                        let levelup = skills.update(
                            gatherer_id,
                            skill_name.clone(),
                            100,
                            &templates.skill_templates,
                        );

                        xp_list.push(network::Xp {
                            skill: skill_name.clone(),
                            xp: 100,
                            levelup: levelup,
                        });

                        let mut item_attrs = HashMap::new();

                        let mut quality_rate = Vec::new();

                        if let Some(template_quality_rate) = &res_template.quality_rate {
                            quality_rate = template_quality_rate.clone();
                        } else {
                            quality_rate = vec![60, 30, 10];
                        }

                        // Determine quality
                        let dist = WeightedIndex::new(quality_rate).unwrap();
                        let sample = dist.sample(&mut rng);
                        let quality_level = sample as i32;

                        debug!("Quality Level: {:?}", quality_level);

                        for property in resource.properties.iter() {
                            debug!("{:?} {:?}", property.name, property.value);
                            //let characteristic_value = rng.gen_range(characteristic.min..characteristic.max);

                            let attr_key = AttrKey::str_to_key(property.name.clone());

                            item_attrs.insert(attr_key, item::AttrVal::Num(property.value as f32));
                        }

                        debug!("item_attrs: {:?}", item_attrs);
                        debug!("Produces: {:?}", resource.produces);

                        if let Some(produces) = &resource.produces {
                            for produce in produces.iter() {
                                let item_name = produce.clone();

                                let (new_item, _merged) = gatherer_inventory.new_with_attrs(
                                    gatherer_id,
                                    item_name,
                                    1, //TODO should this be only 1
                                    item_attrs.clone(),
                                );

                                items_to_update.push(Item::to_packet(new_item));
                            }
                        } else {
                            let (new_item, _merged) = gatherer_inventory.new_with_attrs(
                                gatherer_id,
                                resource.name.clone(),
                                1, //TODO should this be only 1
                                item_attrs.clone(),
                            );

                            items_to_update.push(Item::to_packet(new_item));
                        }
                    } else {
                        return Err(ResourceGatherError::NoInventoryRoom);
                    }
                }
            } else {
                return Err(ResourceGatherError::CannotFindResourceTemplate);
            }
        }

        // Return none if no resources are available on the tile
        if resources_on_tile.len() == 0 {
            return Err(ResourceGatherError::NoResourcesAvailable);
        }

        if items_to_update.len() == 0 {
            return Err(ResourceGatherError::NoItemGathered);
        }

        return Ok((items_to_update, xp_list));
    }

    pub fn structure_gather_by_type(
        operator_id: i32,
        structure_id: i32,
        structure_inventory: &mut Inventory,
        position: Position,
        res_type: String,
        mut ids: ResMut<Ids>,
        skills: &mut Skills,
        capacity: i32,
        resources: &Resources,
        templates: &Templates,
    ) -> Result<(Vec<network::Item>, Vec<network::Xp>), ResourceGatherError> {
        let mut rng = rand::thread_rng();

        let resources_on_tile = Resource::get_by_type(position, res_type.clone(), resources, true);
        let res_templates = &templates.res_templates;
        let item_templates = &templates.item_templates;

        let mut items_to_update: Vec<network::Item> = Vec::new();
        let mut xp_list = Vec::new();

        info!("Resources on tile: {:?}", resources_on_tile);
        for resource in resources_on_tile.iter() {
            if let Some(res_template) = res_templates.get(&resource.name) {
                let skill_name = Resource::type_to_skill(res_type.clone());

                let mut skill_value = 0;

                if let Some(skill) = Skill::get_by_name(operator_id, skill_name.clone(), skills) {
                    skill_value = skill.level;
                }

                info!("Res template: {:?}", res_template);
                info!("Skill value: {:?}", skill_value);
                info!("Skill name: {:?}", skill_name);
                let gather_chance = Resource::gather_chance(skill_value, res_template.skill_req);

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
                        let levelup = skills.update(
                            operator_id,
                            skill_name.clone(),
                            100,
                            &templates.skill_templates,
                        );

                        xp_list.push(network::Xp {
                            skill: skill_name.clone(),
                            xp: 100,
                            levelup: levelup,
                        });

                        let mut item_attrs = HashMap::new();

                        let mut quality_rate = Vec::new();

                        if let Some(template_quality_rate) = &res_template.quality_rate {
                            quality_rate = template_quality_rate.clone();
                        } else {
                            quality_rate = vec![60, 30, 10];
                        }

                        // Determine quality
                        let dist = WeightedIndex::new(quality_rate).unwrap();
                        let sample = dist.sample(&mut rng);
                        let quality_level = sample as i32;

                        debug!("Quality Level: {:?}", quality_level);

                        for property in resource.properties.iter() {
                            debug!("{:?} {:?}", property.name, property.value);
                            //let characteristic_value = rng.gen_range(characteristic.min..characteristic.max);

                            let attr_key = AttrKey::str_to_key(property.name.clone());

                            item_attrs.insert(attr_key, item::AttrVal::Num(property.value as f32));
                        }

                        debug!("item_attrs: {:?}", item_attrs);
                        debug!("Produces: {:?}", resource.produces);

                        if let Some(produces) = &resource.produces {
                            for produce in produces.iter() {
                                let item_name = produce.clone();

                                let (new_item, _merged) = structure_inventory.new_with_attrs(
                                    structure_id,
                                    item_name,
                                    1, //TODO should this be only 1
                                    item_attrs.clone(),
                                );

                                items_to_update.push(Item::to_packet(new_item));
                            }
                        } else {
                            let (new_item, _merged) = structure_inventory.new_with_attrs(
                                dest_obj_id,
                                resource.name.clone(),
                                1, //TODO should this be only 1
                                item_attrs.clone(),
                            );

                            items_to_update.push(Item::to_packet(new_item));
                        }
                    } else {
                        return Err(ResourceGatherError::NoInventoryRoom);
                    }
                }
            } else {
                return Err(ResourceGatherError::CannotFindResourceTemplate);
            }
        }

        // Return none if no resources are available on the tile
        if resources_on_tile.len() == 0 {
            return Err(ResourceGatherError::NoResourcesAvailable);
        }

        if items_to_update.len() == 0 {
            return Err(ResourceGatherError::NoItemGathered);
        }

        return Ok((items_to_update, xp_list));
    }*/

    /*pub fn gather_fishing(
        obj_id: i32,
        dest_obj_id: i32,
        position: Position,
        res_type: String,
        skills: &mut Skills,
        capacity: i32,
        items: &mut Items,
        resources: &mut Resources,
        templates: &Templates,
    ) -> Result<(Vec<network::Item>, Vec<network::Xp>), ResourceGatherError> {
        let mut rng = rand::thread_rng();

        let fishing_resources_on_tile = resources.get_by_type(position, res_type.clone(), false);
        let res_templates = &templates.res_templates;
        let item_templates = &templates.item_templates;

        let mut items_to_update: Vec<network::Item> = Vec::new();
        let mut xp_list = Vec::new();

        for fish_resource in fishing_resources_on_tile.iter() {
            if let Some(res_template) = res_templates.get(&fish_resource.name) {
                let mut skill_value = 0;

                if let Some(skill) = Skill::get_by_name(obj_id, skill::FISHING.to_string(), skills)
                {
                    skill_value = skill.level;
                }

                let gather_chance = Resource::gather_chance(skill_value, res_template.skill_req);

                let random_num = rng.gen::<f32>();

                if random_num < gather_chance {
                    // Set resource to revealed if not already

                    if fish_resource.reveal == false {
                        resources.set_reveal(position, res_type.clone(), true);
                    }

                    let resource_quantity = 1;

                    // Get a random fish from list
                    let fish_templates = templates.get_item_templates_by_class(FISH);
                    let num_fish_templates = fish_templates.len();
                    let fish_template = &fish_templates[rng.gen_range(0..num_fish_templates)];

                    let current_total_weight = items.get_total_weight(dest_obj_id);

                    let new_item_weight = Item::get_weight_from_template(
                        fish_template.name.clone(),
                        resource_quantity,
                        &item_templates,
                    );

                    if (current_total_weight + new_item_weight) < capacity {
                        // Update skill
                        let levelup = skills.update(
                            obj_id,
                            skill::FISHING.to_string(),
                            100,
                            &templates.skill_templates,
                        );

                        xp_list.push(network::Xp {
                            skill: skill::FISHING.to_string(),
                            xp: 100,
                            levelup: levelup,
                        });

                        let mut item_attrs = HashMap::new();

                        let mut quality_rate = Vec::new();

                        if let Some(template_quality_rate) = &res_template.quality_rate {
                            quality_rate = template_quality_rate.clone();
                        } else {
                            quality_rate = vec![60, 30, 10];
                        }

                        // Determine quality
                        let dist = WeightedIndex::new(quality_rate).unwrap();
                        let sample = dist.sample(&mut rng);
                        let quality_level = sample as i32;

                        debug!("Quality Level: {:?}", quality_level);

                        for property in fish_resource.properties.iter() {
                            debug!("{:?} {:?}", property.name, property.value);
                            //let characteristic_value = rng.gen_range(characteristic.min..characteristic.max);

                            let attr_key = AttrKey::str_to_key(property.name.clone());

                            item_attrs.insert(attr_key, item::AttrVal::Num(property.value as f32));
                        }

                        debug!("item_attrs: {:?}", item_attrs);

                        let (new_item, _merged) = items.new_with_attrs(
                            dest_obj_id,
                            fish_template.name.clone(),
                            1, //TODO should this be only 1
                            item_attrs.clone(),
                        );

                        info!("Gather item created: {:?}", new_item);

                        // Convert items to be updated to packets
                        let new_item_packet = Item::to_packet(new_item);

                        items_to_update.push(new_item_packet);
                    } else {
                        return Err(ResourceGatherError::NoInventoryRoom);
                    }
                } else {
                    return Err(ResourceGatherError::NoItemGathered);
                }
            } else {
                return Err(ResourceGatherError::CannotFindResourceTemplate);
            }
        }

        info!("Resources on tile: {:?}", resources.get(&position));

        if fishing_resources_on_tile.len() == 0 {
            return Err(ResourceGatherError::NoResourcesAvailable);
        }

        return Ok((items_to_update, xp_list));
    }*/

    pub fn explore(
        player_id: i32,
        position: Position,
        resources: &Resources,
        discoveries: &mut ResourceDiscoveries,
        res_templates: &ResTemplates,
        skills: &Skills,
        preferred_res_type: Option<&str>,
    ) -> Option<Resource> {
        let Some(resources_on_tile) = resources.get(&position) else {
            return None;
        };

        let mut candidates: Vec<(&Resource, i32)> = resources_on_tile
            .values()
            .filter(|resource| !Resource::is_visible_to(resource, player_id, discoveries))
            .filter_map(|resource| {
                let template = res_templates.get(&resource.name)?;
                let skill_name = Resource::type_to_skill(resource.res_type.clone());
                let skill_level = Skill::from_str(&skill_name)
                    .map(|skill| skills.get_level_by_name(skill))
                    .unwrap_or(0);

                (skill_level >= Resource::minimum_skill_level(template.skill_req))
                    .then_some((resource, skill_level))
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        if let Some(preferred) = preferred_res_type {
            let preferred_candidates = candidates
                .iter()
                .filter(|(resource, _)| resource.res_type == preferred)
                .copied()
                .collect::<Vec<_>>();
            if !preferred_candidates.is_empty() {
                candidates = preferred_candidates;
            }
        }

        let index = rand::thread_rng().gen_range(0..candidates.len());
        let (resource, skill_level) = candidates[index];
        let template = res_templates.get(&resource.name)?;

        if rand::thread_rng().gen::<f32>()
            >= Resource::discovery_chance(skill_level, template.skill_req)
        {
            return None;
        }

        discoveries.discover(player_id, position, resource.name.clone());
        debug!("Player {:?} discovered resource {:?}", player_id, resource);
        Some(resource.clone())
    }

    pub fn is_valid_type(res_type: String, pos: Position, resources: &Resources) -> bool {
        let resources_on_tile = Resource::get_by_type(pos, res_type.clone(), resources, true);
        debug!("Filtered resources_on_tile: {:?}", resources_on_tile);

        if resources_on_tile.len() > 0 {
            return true;
        } else {
            return false;
        }
    }

    pub fn is_valid_type_for_player(
        res_type: String,
        pos: Position,
        resources: &Resources,
        discoveries: &ResourceDiscoveries,
        player_id: i32,
    ) -> bool {
        !Resource::get_by_type_for_player(pos, res_type, resources, discoveries, player_id)
            .is_empty()
    }

    pub fn type_to_skill(res_type: String) -> String {
        match res_type.as_str() {
            ORE => skill::MINING.to_string(),
            LOG => skill::LOGGING.to_string(),
            STONE => skill::STONECUTTING.to_string(),
            FISH => skill::FISHING.to_string(),
            FOOD => skill::FARMING.to_string(),
            FORAGE | PLANT => skill::FORAGING.to_string(),
            GAME_ANIMAL => "Hunting".to_string(),
            _ => skill::FORAGING.to_string(),
            /*WATER => skill::FORAGING.to_string(),
            FOOD => skill::FARMING.to_string(),
            PLANT => skill::FORAGING.to_string(),*/
        }
    }

    pub fn gather_chance(skill_value: i32, res_skill_req: i32) -> f32 {
        let minimum = Resource::minimum_skill_level(res_skill_req);
        if skill_value < minimum {
            return 0.0;
        }

        let base = if res_skill_req <= 0 {
            0.85
        } else if res_skill_req <= 25 {
            0.60
        } else {
            0.35
        };

        (base + 0.02 * (skill_value - minimum) as f32).min(0.95)
    }

    pub fn discovery_chance(skill_value: i32, res_skill_req: i32) -> f32 {
        let minimum = Resource::minimum_skill_level(res_skill_req);
        if skill_value < minimum {
            return 0.0;
        }

        let base = if res_skill_req <= 0 {
            0.85
        } else if res_skill_req <= 25 {
            0.60
        } else {
            0.35
        };

        (base + 0.05 * (skill_value - minimum) as f32).min(0.95)
    }

    pub fn minimum_skill_level(res_skill_req: i32) -> i32 {
        if res_skill_req <= 0 {
            0
        } else if res_skill_req <= 25 {
            2
        } else {
            4
        }
    }

    fn quantity_skill_req(max: i32, quantity_rates: Vec<i32>) -> i32 {
        let index = quantity_rates.iter().position(|&q| q == max).unwrap();

        match index {
            1 => 0,
            2 => 0,
            3 => 10,
            4 => 20,
            5 => 30,
            6 => 40,
            7 => 50,
            _ => 50,
        }
    }
}

pub struct ResourcePlugin;

impl Plugin for ResourcePlugin {
    fn build(&self, app: &mut App) {
        let resources = Resources(HashMap::new());

        app.insert_resource(resources)
            .init_resource::<ResourceDiscoveries>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn test_resource(reveal: bool) -> Resource {
        Resource {
            name: "Test Maple".to_string(),
            image: "maple".to_string(),
            res_type: LOG.to_string(),
            pos: Position { x: 3, y: 4 },
            max: 10,
            yield_level: 1,
            yield_mod: 1.0,
            quantity_level: 1,
            quantity: 10,
            properties: Vec::new(),
            produces: None,
            reveal,
        }
    }

    #[test]
    fn scouting_groups_named_deposits_into_one_category_per_tile() {
        let position = Position { x: 3, y: 4 };
        let mut first_ore = test_resource(false);
        first_ore.name = "Valleyrun Copper Ore".to_string();
        first_ore.res_type = ORE.to_string();
        let mut second_ore = first_ore.clone();
        second_ore.name = "Flameforge Copper Ore".to_string();
        let timber = test_resource(false);

        let resources = Resources(HashMap::from([(
            position,
            HashMap::from([
                (first_ore.name.clone(), first_ore),
                (second_ore.name.clone(), second_ore),
                (timber.name.clone(), timber),
            ]),
        )]));

        let categories = Resource::get_scouted_resource_categories([position], &resources);

        assert_eq!(
            categories,
            vec![
                network::ScoutedResourceCategory {
                    category: "Ore".to_string(),
                    x: 3,
                    y: 4,
                },
                network::ScoutedResourceCategory {
                    category: "Timber".to_string(),
                    x: 3,
                    y: 4,
                },
            ]
        );
    }

    #[test]
    fn scouting_only_reports_requested_tiles_and_supported_categories() {
        let requested = Position { x: 3, y: 4 };
        let outside = Position { x: 4, y: 4 };
        let mut timber = test_resource(false);
        let mut unsupported = test_resource(false);
        unsupported.name = "Unknown Resource".to_string();
        unsupported.res_type = "Unknown".to_string();
        let mut outside_ore = test_resource(false);
        outside_ore.name = "Outside Ore".to_string();
        outside_ore.res_type = ORE.to_string();
        outside_ore.pos = outside;
        timber.pos = requested;
        unsupported.pos = requested;

        let resources = Resources(HashMap::from([
            (
                requested,
                HashMap::from([
                    (timber.name.clone(), timber),
                    (unsupported.name.clone(), unsupported),
                ]),
            ),
            (
                outside,
                HashMap::from([(outside_ore.name.clone(), outside_ore)]),
            ),
        ]));

        let categories = Resource::get_scouted_resource_categories([requested], &resources);

        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].category, "Timber");
        assert_eq!((categories[0].x, categories[0].y), (3, 4));
    }

    #[test]
    fn scouting_maps_every_gather_resource_type_to_a_player_facing_category() {
        assert_eq!(Resource::scout_category_for_type(ORE), Some("Ore"));
        assert_eq!(Resource::scout_category_for_type(STONE), Some("Stone"));
        assert_eq!(Resource::scout_category_for_type(LOG), Some("Timber"));
        assert_eq!(Resource::scout_category_for_type(FORAGE), Some("Forage"));
        assert_eq!(
            Resource::scout_category_for_type(SPRING_WATER),
            Some("Water")
        );
        assert_eq!(Resource::scout_category_for_type(FISH), Some("Fish"));
        assert_eq!(Resource::scout_category_for_type(GAME_ANIMAL), Some("Game"));
    }

    #[test]
    fn ordinary_resource_discovery_is_player_scoped() {
        let resource = test_resource(false);
        let mut discoveries = ResourceDiscoveries::default();

        discoveries.discover(7, resource.pos, resource.name.clone());

        assert!(Resource::is_visible_to(&resource, 7, &discoveries));
        assert!(!Resource::is_visible_to(&resource, 8, &discoveries));
    }

    #[test]
    fn explicit_world_reveal_is_visible_to_every_player() {
        let resource = test_resource(true);
        let discoveries = ResourceDiscoveries::default();

        assert!(Resource::is_visible_to(&resource, 7, &discoveries));
        assert!(Resource::is_visible_to(&resource, 8, &discoveries));
    }

    #[test]
    fn nearby_resource_packet_preserves_the_template_image_key() {
        let resource = test_resource(true);
        let position = resource.pos;
        let mut resources_on_tile = HashMap::new();
        resources_on_tile.insert(resource.name.clone(), resource);
        let resources = Resources(HashMap::from([(position, resources_on_tile)]));

        let nearby = Resource::get_nearby_resources(
            position,
            &resources,
            &ResourceDiscoveries::default(),
            7,
        );

        assert_eq!(nearby.len(), 1);
        assert_eq!(nearby[0].name, "Test Maple");
        assert_eq!(nearby[0].image, "maple");
    }

    #[test]
    fn gather_chance_has_smooth_tiers_and_caps() {
        assert_eq!(Resource::gather_chance(0, 0), 0.85);
        assert_eq!(Resource::gather_chance(1, 25), 0.0);
        assert_eq!(Resource::gather_chance(2, 25), 0.60);
        assert_eq!(Resource::gather_chance(4, 50), 0.35);
        assert_eq!(Resource::gather_chance(99, 0), 0.95);
    }

    #[test]
    fn encounter_and_forage_sites_select_one_output() {
        let mut hunting_ground = test_resource(false);
        hunting_ground.name = "Fruitful Hunting Grounds".to_string();
        hunting_ground.res_type = GAME_ANIMAL.to_string();
        hunting_ground.produces = Some(vec![
            "Windstride Deer Carcass".to_string(),
            "Felled Bristleback Boar".to_string(),
            "Felled Swiftstep Hare".to_string(),
        ]);
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let hunted = Resource::gather_output_names(&hunting_ground, &mut rng);
        assert_eq!(hunted.len(), 1);
        assert!(hunting_ground
            .produces
            .as_ref()
            .unwrap()
            .contains(&hunted[0]));

        let mut forage_site = test_resource(false);
        forage_site.name = "Useful Underbrush".to_string();
        forage_site.res_type = FORAGE.to_string();
        forage_site.produces = Some(vec![
            "Cragroot Maple Stick".to_string(),
            "Plant Fibers".to_string(),
        ]);
        let foraged = Resource::gather_output_names(&forage_site, &mut rng);
        assert_eq!(foraged.len(), 1);
        assert!(forage_site.produces.as_ref().unwrap().contains(&foraged[0]));

        let mut ordinary = test_resource(false);
        ordinary.produces = Some(vec!["Log".to_string(), "Bark".to_string()]);
        assert_eq!(
            Resource::gather_output_names(&ordinary, &mut rng),
            vec!["Log".to_string(), "Bark".to_string()]
        );
    }
}
