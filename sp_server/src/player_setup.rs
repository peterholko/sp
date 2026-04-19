use bevy::prelude::*;
use big_brain::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::constants::*;
use crate::encounter::Encounter;
use crate::event::{EventExecuting, EventExecutingState};
use crate::game::{InitialEncounterEntry, InitialEncounterState, Merchant, Monolith, ObjQuery, SpawnPositions};
use crate::item::{Inventory, Slot};
use crate::effect::Effect;
use crate::obj::{ActiveShelter, AddLightEffect, Campfire, LastCombatTick, NewObj, UpdateObj};
use crate::tax_collector::{MerchantScorer, MoveToPos, SetDestination};
use crate::trade::WantedItem;
use crate::world::get_time_of_day;

use crate::common::{
    Destination, Drink, Eat, Heat, Hunger, Idle, MoveTo, Sleep, Thirst, Tired, Transport,
};
use crate::villager::{
    CapacityScorer, DrowsyScorer, EnemyDistanceScorer, ExhaustedScorer, FindDrink, FindFood, FindShelter, GoodMorale, HeatScorer, HungryScorer, IdleScorer, LoadItems, Morale, ProcessOrder, SetFleeDestination, SetOrderDestination, SetStorageDestination, StructureCapacityScorer, ThirstyScorer, TransferDrink, TransferFood, UnloadItems
};

use crate::{
    effect::Effects,
    event::{GameEvent, GameEventType, GameEvents, MapEvents, VisibleEvent},
    game::{BoundMonolith, EncounterMoves, GameTick},
    ids::{EntityObjMap, Ids},
    item::{self},
    obj::Obj,
    obj::{
        ActiveTask, Class, ClassStructure, Id, Misc, Name, Order, PlayerId, Position, State,
        StateAboard, Stats, Storage, Subclass, SubclassHero, SubclassVillager, Template, Viewshed,
    },
    recipe::Recipes,
    skill::Skills,
    structure::Plans,
    templates::{ObjTemplate, Templates},
    villager_util::{self, VillagerUtil},
};

pub fn new(
    player_id: i32,
    hero_name: String,
    class_name: String,
    commands: &mut Commands,
    start_locations: &mut ResMut<StartLocations>,
    ids: &mut ResMut<Ids>,
    entity_map: &mut ResMut<EntityObjMap>,
    map_events: &mut ResMut<MapEvents>,
    game_events: &mut ResMut<GameEvents>,
    recipes: &mut ResMut<Recipes>,
    plans: &mut ResMut<Plans>,
    templates: &Res<Templates>,
    game_tick: &Res<GameTick>,
    monoliths: &Query<ObjQuery, With<Monolith>>,
    spawn_positions: &mut ResMut<SpawnPositions>,
    initial_encounter_state: &mut ResMut<InitialEncounterState>,
) -> Result<(), String> {
    // Select a start location and remove it from the list
    let start_location = match start_locations.get_start_location() {
        Ok(start_location) => start_location,
        Err(e) => return Err(e),
    };

    // Record spawn position for crisis tracking
    spawn_positions.insert(
        player_id,
        Position {
            x: start_location.hero_pos[0],
            y: start_location.hero_pos[1],
        },
    );

    // Find nearest monolith
    let (monolith_id, monolith_pos) =
        find_nearest_monolith(start_location.hero_pos.clone(), &monoliths);

    info!("Nearest monolith: {:?}", monolith_id);
    info!("Nearest monolith position: {:?}", monolith_pos);

    let burrow_id = ids.new_obj_id();
    let structure_name = "Burrow".to_string();
    let structure_template = templates.obj_templates.get(structure_name.clone());

    let mut burrow_inventory = Inventory {
        owner: burrow_id,
        items: Vec::new(),
    };

    // Burrow starting items (reward for checking storage)
    let mut feed_attrs = HashMap::new();
    feed_attrs.insert(item::AttrKey::Feed, item::AttrVal::Num(100.0));

    burrow_inventory.new_with_attrs(
        ids.new_item_id(),
        burrow_id,
        "Honeybell Berries".to_string(),
        5,
        feed_attrs,
        &templates.item_templates,
    );
    burrow_inventory.new(
        ids.new_item_id(),
        "Gold Coins".to_string(),
        50,
        &templates.item_templates,
    );
    burrow_inventory.new(
        ids.new_item_id(),
        "Valleyrun Copper Ingot".to_string(),
        3,
        &templates.item_templates,
    );
    burrow_inventory.new(
        ids.new_item_id(),
        "Cragroot Maple Timber".to_string(),
        3,
        &templates.item_templates,
    );
    burrow_inventory.new(
        ids.new_item_id(),
        "Springbranch Maple Log".to_string(),
        5,
        &templates.item_templates,
    );
    burrow_inventory.new(
        ids.new_item_id(),
        "Yurt Deed".to_string(),
        1,
        &templates.item_templates,
    );
    burrow_inventory.new(
        ids.new_item_id(),
        FISHING_ROD.to_string(),
        1,
        &templates.item_templates,
    );
    burrow_inventory.new(
        ids.new_item_id(),
        "Mine Deed".to_string(),
        1,
        &templates.item_templates,
    );

    let structure: Obj = Obj {
        id: Id(burrow_id),
        player_id: PlayerId(player_id),
        position: Position {
            x: start_location.burrow_pos[0],
            y: start_location.burrow_pos[1],
        },
        name: Name("Burrow".into()),
        template: Template("Burrow".into()),
        class: Class("structure".into()),
        subclass: Subclass::Storage,
        state: State::None,
        misc: Misc {
            image: "burrow".into(),
            hsl: Vec::new(),
            groups: Vec::new(),
        },
        stats: Stats {
            hp: structure_template.base_hp.unwrap_or(100),
            base_hp: structure_template.base_hp.unwrap_or(100), // Convert option to non-option
            stamina: None,
            base_stamina: None,
            base_def: 0,
            base_damage: None,
            damage_range: None,
            base_speed: None,
            base_vision: None,
        },
        effects: Effects(HashMap::new()),
        inventory: burrow_inventory,
        last_combat_tick: LastCombatTick::default(),
    };

    let structure_entity_id = commands.spawn((structure, ClassStructure, Storage)).id();

    // New Obj mappings
    ids.new_obj(burrow_id, player_id);
    entity_map.new_obj(burrow_id, structure_entity_id);

    // Create a new object event
    commands.trigger(NewObj {
        entity: structure_entity_id,
    });

    /*
    let stockade_id = ids.new_obj_id();
    let structure_name = "Stockade".to_string();
    let structure_template = ObjTemplate::get_template(structure_name.clone(), templates);

    let structure: Obj = Obj {
        id: Id(stockade_id),
        player_id: PlayerId(player_id),
        position: Position {
            x: start_location.burrow_pos[0],
            y: start_location.burrow_pos[1],
        },
        name: Name("Stockade".into()),
        template: Template("Stockade".into()),
        class: Class("structure".into()),
        subclass: Subclass::Wall,
        state: State::None,
        viewshed: Viewshed { range: 0 },
        misc: Misc {
            image: "stockade".into(),
            hsl: Vec::new(),
            groups: Vec::new(),
        },
        stats: Stats {
            hp: structure_template.base_hp.unwrap_or(100) - 10,
            base_hp: structure_template.base_hp.unwrap_or(100), // Convert option to non-option
            stamina: None,
            base_stamina: None,
            base_def: 0,
            base_damage: None,
            damage_range: None,
            base_speed: None,
            base_vision: None,
        },
        effects: Effects(HashMap::new()),
    };

    let structure_attrs = StructureAttrs {
        start_time: 0,
        end_time: 0,
        //build_time: structure_template.build_time.unwrap(), // Structure must have build time
        builder: -1,
        progress: 0,
        selected_upgrade: None, //req: structure_template.req.unwrap(),
    };

    let structure_entity_id = commands
        .spawn((structure, structure_attrs, ClassStructure))
        .id();

    // New Obj mappings
    ids.new_obj(stockade_id, player_id);
    entity_map.new_obj(stockade_id, structure_entity_id);

    map_events.new(
        stockade_id,
        game_tick.0 + 1,
        VisibleEvent::NewObjEvent { new_player: false },
    );*/

    // Creating hero
    debug!("Creating hero for player: {:?}", player_id);
    let hero_template_name = "Novice".to_string() + " " + class_name.as_str();
    let hero_template = templates.obj_templates.get(hero_template_name.clone());

    let hero_id = ids.new_obj_id();

    let mut inventory = Inventory {
        items: Vec::new(),
        owner: hero_id,
    };

    // Hero starting inventory (immediate essentials only)
    inventory.new(
        ids.new_item_id(),
        "Firewood".to_string(),
        10,
        &templates.item_templates,
    );
    inventory.new(
        ids.new_item_id(),
        "Flint Shard".to_string(),
        1,
        &templates.item_templates,
    );
    inventory.new(
        ids.new_item_id(),
        "Cragroot Maple Resin".to_string(),
        1,
        &templates.item_templates,
    );
    inventory.new(
        ids.new_item_id(),
        "Cragroot Maple Stick".to_string(),
        1,
        &templates.item_templates,
    );
    inventory.new(
        ids.new_item_id(),
        "Waterskin (Filled)".to_string(),
        5,
        &templates.item_templates,
    );
    inventory.new(
        ids.new_item_id(),
        "Salted Meat Strip".to_string(),
        5,
        &templates.item_templates,
    );
    inventory.new(
        ids.new_item_id(),
        "Sharpened Stick".to_string(),
        1,
        &templates.item_templates,
    );
    let shirt = inventory.new(
        ids.new_item_id(),
        "Tattered Shirt".to_string(),
        1,
        &templates.item_templates,
    );
    let pants = inventory.new(
        ids.new_item_id(),
        "Tattered Pants".to_string(),
        1,
        &templates.item_templates,
    );
    let torch = inventory.new(
        ids.new_item_id(),
        "Crude Torch".to_string(),
        1,
        &templates.item_templates,
    );

    inventory.equip(shirt.id, Some(Slot::Chest));
    inventory.equip(pants.id, Some(Slot::Pants));

    // Equip torch for night spawns so the hero has visibility
    let time_of_day = get_time_of_day(game_tick.0);
    if time_of_day == crate::world::TimeOfDay::Dusk
        || time_of_day == crate::world::TimeOfDay::Night
    {
        inventory.equip(torch.id, Some(Slot::OffHand));
    }

    let mut item_attrs = HashMap::new();
    item_attrs.insert(item::AttrKey::Damage, item::AttrVal::Num(11.0));
    item_attrs.insert(item::AttrKey::DeepWoundChance, item::AttrVal::Num(0.9));

    inventory.new_with_attrs(
        ids.new_item_id(),
        hero_id,
        "Copper Training Axe".to_string(),
        1,
        item_attrs.clone(),
        &templates.item_templates,
    );

    let mut item_attrs = HashMap::new();
    item_attrs.insert(item::AttrKey::Defense, item::AttrVal::Num(3.0));

    inventory.new_with_attrs(
        ids.new_item_id(),
        hero_id,
        "Copper Helm".to_string(),
        1,
        item_attrs.clone(),
        &templates.item_templates,
    );

    let mut item_attrs2 = HashMap::new();
    item_attrs2.insert(item::AttrKey::Healing, item::AttrVal::Num(10.0));

    inventory.new_with_attrs(
        ids.new_item_id(),
        hero_id,
        "Health Potion".to_string(),
        1,
        item_attrs2.clone(),
        &templates.item_templates,
    );

    let hero = Obj {
        id: Id(hero_id),
        player_id: PlayerId(player_id),
        position: Position {
            x: start_location.hero_pos[0],
            y: start_location.hero_pos[1],
        },
        name: Name(hero_name.clone()),
        template: Template(hero_template_name.clone()),
        class: Class("unit".into()),
        subclass: Subclass::Hero,
        state: State::None,
        misc: Misc {
            image: str::replace(hero_template.template.as_str(), " ", "").to_lowercase(),
            hsl: Vec::new(),
            groups: Vec::new(),
        },
        stats: Stats {
            hp: hero_template.base_hp.unwrap(),
            base_hp: hero_template.base_hp.unwrap(),
            stamina: hero_template.base_stamina,
            base_stamina: hero_template.base_stamina,
            base_def: hero_template.base_def.unwrap(),
            base_damage: hero_template.base_dmg,
            damage_range: hero_template.dmg_range,
            base_speed: hero_template.base_speed,
            base_vision: hero_template.base_vision,
        },
        effects: Effects(HashMap::new()),
        inventory: inventory.clone(),
        last_combat_tick: LastCombatTick::default(),
    };

    let hero_skills = Skills::new();

    let hero_attrs = Obj::generate_hero_attrs();
    let hero_inventory = hero.inventory.clone();

    let bound_monolith = BoundMonolith {
        id: monolith_id,
        pos: monolith_pos,
    };

    // Spawn hero
    let hero_entity_id = commands
        .spawn((
            hero,
            Viewshed {
                range: Obj::set_viewshed_range(
                    hero_id,
                    hero_template_name,
                    game_tick.0,
                    &hero_inventory,
                    &templates,
                    0.0,
                ),
            },
            hero_attrs,
            hero_skills,
            EventExecuting {
                event_type: "".to_string(),
                state: EventExecutingState::None,
            },
            EncounterMoves(0),
            bound_monolith,
            SubclassHero,            // Hero component tag
            Thirst::new(0.0, 0.025), //0.1 before
            Hunger::new(0.0, 0.025),
            Tired::new(0.0, 0.025),
            Heat::new(50.0),
        ))
        .id();

    // New Obj mappings
    ids.new_hero(hero_id, player_id);
    entity_map.new_obj(hero_id, hero_entity_id);

    // Create a new object event
    commands.trigger(NewObj {
        entity: hero_entity_id,
    });

    debug!("map_events: {:?}", map_events);

    // Create campfire at hero's location only if it's dusk or night
    if time_of_day == crate::world::TimeOfDay::Dusk
        || time_of_day == crate::world::TimeOfDay::Night
    {
        // Create campfire with inventory
        let campfire_id = ids.new_obj_id();
        let mut campfire = Obj::create_nospawn(
            campfire_id,
            player_id,
            "Campfire".to_string(),
            Position {
                x: start_location.hero_pos[0],
                y: start_location.hero_pos[1],
            },
            State::None,
            Inventory {
                owner: campfire_id,
                items: Vec::new(),
            },
            &templates,
        );

        // Add 10 firewood items to campfire's inventory
        campfire.inventory.new(
            ids.new_item_id(),
            "Firewood".to_string(),
            10,
            &templates.item_templates,
        );

        // Get the campfire template to check for vision
        let campfire_template = templates.obj_templates.get("Campfire".to_string());

        // Spawn the campfire entity
        let campfire_entity_id = if let Some(vision) = campfire_template.base_vision {
            commands.spawn((campfire, Viewshed { range: vision })).id()
        } else {
            commands.spawn(campfire).id()
        };

        // Create mappings
        ids.new_obj(campfire_id, player_id);
        entity_map.new_obj(campfire_id, campfire_entity_id);

        // Add the Campfire component with is_lit set to true
        commands.entity(campfire_entity_id).insert(Campfire {
            is_lit: true,
            lit_at: game_tick.0,
            duration: 0,
        });

        // Create a new object event
        commands.trigger(NewObj {
            entity: campfire_entity_id,
        });

        // Swap to lit image
        commands.trigger(UpdateObj {
            entity: campfire_entity_id,
            attrs: vec![(IMAGE.to_string(), "campfirelit".to_string())],
        });

        // Apply campfire light effect so nearby heroes get vision
        commands.trigger(AddLightEffect {
            entity: campfire_entity_id,
            effect: Effect::CampfireLight,
        });
    }

    // Villager obj
    let villager_id = ids.new_obj_id();

    let villager_template_name = "Human Villager".to_string();
    let villager_template = templates.obj_templates.get(villager_template_name.clone());

    let image: String;

    if let Some(template_images) = villager_template.images {
        let random_image = rand::thread_rng().gen_range(0..template_images.len());
        image = template_images[random_image].clone();
    } else {
        image = Obj::template_to_image(&villager_template.template);
    }

    /*let mut villager = Obj {
        id: Id(villager_id),
        player_id: PlayerId(player_id),
        position: Position {
            x: start_location.villager_pos[0],
            y: start_location.villager_pos[1],
        },
        name: Name(VillagerUtil::generate_name()),
        template: Template("Human Villager".into()),
        class: Class("unit".into()),
        subclass: Subclass::Villager,
        state: State::None,
        misc: Misc {
            image: image,
            hsl: Vec::new(),
            groups: Vec::new(),
        },
        stats: Stats {
            hp: villager_template.base_hp.unwrap(),
            base_hp: villager_template.base_hp.unwrap(),
            stamina: villager_template.base_stamina,
            base_stamina: villager_template.base_stamina,
            base_def: villager_template.base_def.unwrap(),
            base_damage: villager_template.base_dmg,
            damage_range: villager_template.dmg_range,
            base_speed: villager_template.base_speed,
            base_vision: villager_template.base_vision,
        },
        effects: Effects(HashMap::new()),
        inventory: Inventory {
            owner: villager_id,
            items: Vec::new(),
        },
        last_combat_tick: LastCombatTick::default(),
    };

    // Villager generate skills
    let villager_skills = VillagerUtil::generate_skills(villager_id, &templates.skill_templates);

    // Villager create attributes components ```
    let base_attrs = VillagerUtil::generate_attributes(1);

    let active_task = ActiveTask::None;

    villager.inventory.new(
        ids.new_item_id(),
        "Crude Torch".to_string(),
        1,
        &templates.item_templates,
    );

    let flee = Steps::build()
        .label("Flee")
        .step(SetFleeDestination)
        .step(MoveTo);

    let find_move_to_and_drink = Steps::build()
        .label("FindMoveToAndDrink")
        .step(FindDrink)
        .step(MoveTo)
        .step(TransferDrink)
        .step(Drink);

    let find_move_to_and_eat = Steps::build()
        .label("FindMoveToAndEat")
        .step(FindFood)
        .step(MoveTo)
        .step(TransferFood)
        .step(Eat);

    let find_move_to_and_sleep = Steps::build()
        .label("FindMoveToAndSleep")
        .step(FindShelter {
            trigger_event: "Sleep".to_string(),
        })
        .step(MoveTo)
        .step(Sleep);

    let find_move_to_and_shelter = Steps::build()
        .label("FindMoveToAndShelter")
        .step(FindShelter {
            trigger_event: "Shelter".to_string(),
        })
        .step(MoveTo)
        .step(Idle {
            start_time: 0,
            duration: 100,
        });

    let process_order = Steps::build()
        .label("ProcessOrder")
        .step(SetOrderDestination)
        .step(MoveTo)
        .step(ProcessOrder);

    let unload_items = Steps::build()
        .label("UnloadItems")
        .step(SetStorageDestination)
        .step(MoveTo)
        .step(UnloadItems);

    let load_items = Steps::build()
        .label("LoadItems")
        .step(LoadItems);

    let villager_inventory = villager.inventory.clone();

    let villager_entity_id = commands
        .spawn((
            villager,
            Viewshed {
                range: Obj::set_viewshed_range(
                    villager_id,
                    villager_template_name,
                    game_tick.0,
                    &villager_inventory,
                    &templates,
                    0.0,
                ),
            },
            SubclassVillager,
            EncounterMoves(0),
            base_attrs,
            villager_skills,
            EventExecuting {
                event_type: "".to_string(),
                state: EventExecutingState::None,
            },
            active_task,
            Order::None,
            ActiveShelter(NO_SHELTER),
        ))
        .id();

    commands.entity(villager_entity_id).insert((
        Thirst::new(10.0, 0.02), //0.1 before
        Hunger::new(10.0, 0.02),
        Tired::new(0.0, 0.02),
        Heat::new(50.0),
        Morale::new(50.0),
        Thinker::build()
            .label("Villager")
            .picker(Highest)
            .when(EnemyDistanceScorer, flee)
            .when(ThirstyScorer, find_move_to_and_drink)
            .when(HungryScorer, find_move_to_and_eat)
            .when(DrowsyScorer, find_move_to_and_sleep)
            .when(ExhaustedScorer, Sleep)
            .when(HeatScorer, find_move_to_and_shelter)
            .when(StructureCapacityScorer, load_items)
            .when(CapacityScorer, unload_items)
            .when(
                IdleScorer,
                Idle {
                    start_time: 0,
                    duration: 100,
                },
            )
            .when(GoodMorale, process_order),
    ));

    ids.new_obj(villager_id, player_id);
    entity_map.new_obj(villager_id, villager_entity_id);

    // Create a new object event
    commands.trigger(NewObj {
        entity: villager_entity_id,
    }); */

    // Villager obj
    /*let villager_id = ids.new_obj_id();

    let villager_template_name = "Human Villager".to_string();
    let villager_template = ObjTemplate::get_template(villager_template_name.clone(), templates);

    let image: String;

    if let Some(template_images) = villager_template.images {
        let random_image = rand::thread_rng().gen_range(0..template_images.len());
        image = template_images[random_image].clone();
    } else {
        image = Obj::template_to_image(&villager_template.template);
    }

    let villager = Obj {
        id: Id(villager_id),
        player_id: PlayerId(player_id),
        position: Position {
            x: start_location.villager_pos[0],
            y: start_location.villager_pos[1],
        },
        name: Name(VillagerUtil::generate_name()),
        template: Template("Human Villager".into()),
        class: Class("unit".into()),
        subclass: Subclass::Villager,
        state: State::None,
        misc: Misc {
            image: image,
            hsl: Vec::new(),
            groups: Vec::new(),
        },
        stats: Stats {
            hp: villager_template.base_hp.unwrap(),
            base_hp: villager_template.base_hp.unwrap(),
            stamina: villager_template.base_stamina,
            base_stamina: villager_template.base_stamina,
            base_def: villager_template.base_def.unwrap(),
            base_damage: villager_template.base_dmg,
            damage_range: villager_template.dmg_range,
            base_speed: villager_template.base_speed,
            base_vision: villager_template.base_vision,
        },
        effects: Effects(HashMap::new()),
    };

    // Villager generate skills
    VillagerUtil::generate_skills(villager_id, skills, &templates.skill_templates);

    // Villager create attributes components ```
    let base_attrs = VillagerUtil::generate_attributes(1);

    let villager_attrs = VillagerAttrs {
        shelter: -1,
        structure: -1,
        structure_template: "None".to_string(),
        activity: Activity::None,
    };

    let villager_entity_id = commands
        .spawn((
            villager,
            Viewshed { range: 2 },
            SubclassVillager,
            base_attrs,
            villager_attrs,
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
        ))
        .id();

    ids.new_obj(villager_id, player_id);
    entity_map.new_obj(villager_id, villager_entity_id);

    map_events.new(
        villager_id,
        game_tick.0 + 1,
        VisibleEvent::NewObjEvent { new_player: false },
    );    */

    // Starting recipes
    recipes.create(player_id, "Cooked Meat".to_string(), &templates);
    recipes.create(player_id, "Training Pick Axe".to_string(), &templates);
    //recipes.create(player_id, "Copper Training Axe".to_string(), &templates);
    recipes.create(player_id, "Firewood".to_string(), &templates);
    recipes.create(player_id, "Crude Torch".to_string(), &templates);

    // Starting plans (survival basics only — more plans acquired through exploration and villager)
    plans.add(player_id, "Campfire".to_string(), 0, 0);
    plans.add(player_id, "Stockade".to_string(), 0, 0);

    let mut thirst_attr = HashMap::new();
    thirst_attr.insert(item::AttrKey::Thirst, item::AttrVal::Num(90.0));

    let mut feed_attr = HashMap::new();
    feed_attr.insert(item::AttrKey::Feed, item::AttrVal::Num(90.0));

    /*items.new_with_attrs(
        villager_id,
        "Amitanian Grape".to_string(),
        50,
        feed_attr.clone(),
    );
    items.new_with_attrs(
        villager_id,
        "Spring Water".to_string(),
        50,
        thirst_attr.clone(),
    );*/

    // Villager obj
    let villager_id2 = ids.new_obj_id();
    let merchant_player_id = MERCHANT_PLAYER_ID;

    let empire_pos = Position { x: 1, y: 37 };
    let landing_pos = Position {
        x: start_location.merchant_pos[0],
        y: start_location.merchant_pos[1],
    };

    let merchant_id = ids.new_obj_id();

    let mut merchant = Obj::create_nospawn(
        merchant_id,
        merchant_player_id,
        "Meager Merchant".to_string(),
        empire_pos,
        State::None,
        Inventory {
            owner: merchant_id,
            items: Vec::new(),
        },
        templates,
    );

    let merchant_id = merchant.id.0;

    let wanted_copper_ore = WantedItem::new_by_subclass("Copper Ore".to_string());
    let wanted_maple_log = WantedItem::new_by_subclass("Maple Log".to_string());
    let wanted_maple_timber = WantedItem::new_by_subclass("Maple Timber".to_string());

    let merchant_component = Merchant {
        trade_port: empire_pos,
        landing_at: landing_pos,
        wanted_items: vec![wanted_copper_ore, wanted_maple_log, wanted_maple_timber],
    };

    let merchant_template_name = "Meager Merchant".to_string();

    // Merchant Items
    merchant.inventory.new(
        merchant_id,
        "Gold Coins".to_string(),
        500,
        &templates.item_templates,
    );
    merchant.inventory.new(
        merchant_id,
        "Yurt Deed".to_string(),
        1,
        &templates.item_templates,
    );
    merchant.inventory.new(
        merchant_id,
        "Training Pick Axe".to_string(),
        1,
        &templates.item_templates,
    );
    merchant.inventory.new(
        merchant_id,
        "Lumbercamp Deed".to_string(),
        1,
        &templates.item_templates,
    );
    merchant.inventory.new(
        merchant_id,
        "Quarry Deed".to_string(),
        1,
        &templates.item_templates,
    );
    merchant.inventory.new(
        merchant_id,
        "Trapper Deed".to_string(),
        1,
        &templates.item_templates,
    );

    let route = vec![empire_pos, landing_pos];

    let move_to_and_idle = Steps::build()
        .label("MoveToPos and Idle")
        // Set destination will set the move to pos
        .step(SetDestination)
        .step(MoveToPos)
        .step(Idle {
            start_time: 0,
            duration: 600,
        });

    /*let merchant_entity_id = commands
    .spawn((
        merchant,
        Viewshed {
            range: Obj::set_viewshed_range(
                merchant_id,
                merchant_template_name,
                game_tick.0,
                &items,
                &templates,
                0.0,
            ),
        },
        merchant_component,
        Transport {
            route: route,
            next_stop: 0,
            hauling: vec![],
        },
        Destination {
            // Set destination will set the move to pos
            pos: Position { x: -1, y: -1 },
        },
        Thinker::build()
            .label("Merchant")
            .picker(Highest)
            .when(MerchantScorer, move_to_and_idle),
    ))
    .id();*/

    /*ids.new_obj(merchant_id, merchant_player_id);
    entity_map.new_obj(merchant_id, merchant_entity_id);
    debug!("Inserting merchant entity_map: {:?}", entity_map);

    map_events.new(
        merchant_id,
        game_tick.0 + 1,
        VisibleEvent::NewObjEvent { new_player: false },
    );*/

    /*let villager2 = Obj {
        id: Id(villager_id2),
        player_id: PlayerId(merchant_player_id),
        position: empire_pos,
        name: Name("Villager 2".into()),
        template: Template(villager_template_name.clone()),
        class: Class("unit".into()),
        subclass: Subclass::Villager,
        state: State::Aboard,
        misc: Misc {
            image: "humanvillager2".into(),
            hsl: Vec::new(),
            groups: Vec::new(),
        },
        stats: Stats {
            hp: villager_template.base_hp.expect("Missing hp stat"),
            base_hp: villager_template.base_hp.expect("Missing base_hp stat"),
            stamina: villager_template.base_stamina,
            base_stamina: villager_template.base_stamina,
            base_def: villager_template.base_def.expect("Missing base_def stat"),
            base_damage: villager_template.base_dmg,
            damage_range: villager_template.dmg_range,
            base_speed: villager_template.base_speed,
            base_vision: villager_template.base_vision,
        },
        effects: Effects(HashMap::new()),
    };

    // Villager generate skills
    VillagerUtil::generate_skills(villager_id2, skills, &templates.skill_templates);

    // Villager create attributes components ```
    let base_attrs2 = VillagerUtil::generate_attributes(1);

    let villager_attrs2 = VillagerAttrs {
        shelter: -1,
        structure: -1,
        structure_template: "None".to_string(),
        activity: Activity::None,
    };*/

    /*let villager_entity_id2 = commands
        .spawn((
            villager2,
            Viewshed {
                range: Obj::set_viewshed_range(
                    villager_id2,
                    villager_template_name,
                    game_tick.0,
                    &items,
                    &templates,
                ),
            },
            SubclassVillager,
            base_attrs2,
            villager_attrs2,
            StateAboard {
                transport_id: merchant_id,
            },
        ))
        .id();

    ids.new_obj(villager_id2, merchant_player_id);
    entity_map.new_obj(villager_id2, villager_entity_id2);*/

    // Create shipwreck with salvageable supplies
    let shipwreck_id = ids.new_obj_id();
    let mut shipwreck_inventory = Inventory {
        owner: shipwreck_id,
        items: Vec::new(),
    };

    // Shipwreck items (reward for exploring the wreck)
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Training Pick Axe".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Bedroll".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Sickle".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Bucket".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Cragroot Maple Log".to_string(),
        10,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Windstride Raw Hide".to_string(),
        10,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Seeds".to_string(),
        25,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Felled Bristleback Boar".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Small Tent Deed".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Farm Deed".to_string(),
        1,
        &templates.item_templates,
    );

    let shipwreck = Obj::create_nospawn(
        shipwreck_id,
        MERCHANT_PLAYER_ID,
        "Shipwreck".to_string(),
        Position {
            x: start_location.shipwreck_pos[0],
            y: start_location.shipwreck_pos[1],
        },
        State::None,
        shipwreck_inventory,
        &templates,
    );

    let shipwreck_entity_id = commands.spawn(shipwreck).id();

    ids.new_obj(shipwreck_id, MERCHANT_PLAYER_ID);
    entity_map.new_obj(shipwreck_id, shipwreck_entity_id);

    commands.trigger(NewObj {
        entity: shipwreck_entity_id,
    });

    // Create human corpse
    Obj::create(
        999,
        "Human Corpse".to_string(),
        Position {
            x: start_location.corpse1_pos[0],
            y: start_location.corpse1_pos[1],
        },
        State::Dead,
        commands,
        ids,
        entity_map,
        map_events,
        &game_tick,
        &templates,
    );

    // Create human corpse
    Obj::create(
        999,
        "Human Corpse".to_string(),
        Position {
            x: start_location.corpse2_pos[0],
            y: start_location.corpse2_pos[1],
        },
        State::Dead,
        commands,
        ids,
        entity_map,
        map_events,
        &game_tick,
        &templates,
    );

    /*Encounter::spawn_npc(
        NPC_PLAYER_ID,
        Position {
            x: start_location.necromancer_pos[0],
            y: start_location.necromancer_pos[1],
        },
        "Giant Rat".to_string(),
        commands,
        ids,
        entity_map,
        items,
        &templates,
    );*/

    // Spawn giant rats from the shipwreck ~1 minute after player arrives
    let shipwreck_pos = Position {
        x: start_location.shipwreck_pos[0],
        y: start_location.shipwreck_pos[1],
    };

    let mut rat_ids = Vec::new();
    for i in 0..2 {
        let rat_npc_id = ids.new_obj_id();
        rat_ids.push(rat_npc_id);
        let rat_event_id = ids.new_map_event_id();
        let rat_event = GameEvent {
            event_id: rat_event_id,
            start_tick: game_tick.0,
            run_tick: game_tick.0 + 600 + (i * 30), // Stagger spawns ~1 minute in
            event_type: GameEventType::SpawnNPC {
                npc_type: "Giant Rat".to_string(),
                pos: shipwreck_pos,
                npc_id: Some(rat_npc_id),
            },
        };
        game_events.insert(rat_event.event_id, rat_event);
    }

    // Register the initial encounter chain: rats → boar/crab → spider
    // The boar/crab and spider spawn when the previous enemy is killed (see initial_encounter_system)
    let phase1_spawn = if rand::thread_rng().gen_range(0..2) == 0 {
        "Giant Crab".to_string()
    } else {
        "Wild Boar".to_string()
    };
    initial_encounter_state.insert(
        player_id,
        InitialEncounterEntry {
            rat_ids,
            phase1_spawn,
            phase1_npc_id: None,
            spawn_pos: shipwreck_pos,
            start_tick: game_tick.0,
        },
    );

    // Distress sound from the shipwreck ~2:30 min after player arrives
    let distress_event = VisibleEvent::SoundEvent {
        pos: shipwreck_pos,
        sound: "A desperate voice calls from the shipwreck: \"Is anyone out there?!\"".to_string(),
        intensity: 5,
    };
    map_events.new(hero_id, game_tick.0 + 1500, distress_event);

    // Castaway villager emerges from the shipwreck ~3:20 min after player arrives
    // Spawn at villager_pos (land tile) not shipwreck_pos (water tile) so they can pathfind
    let villager_spawn_pos = Position {
        x: start_location.villager_pos[0],
        y: start_location.villager_pos[1],
    };
    let villager_event = GameEvent {
        event_id: ids.new_map_event_id(),
        start_tick: game_tick.0,
        run_tick: game_tick.0 + 2000,
        event_type: GameEventType::SpawnVillager {
            pos: villager_spawn_pos,
            player_id,
        },
    };
    game_events.insert(villager_event.event_id, villager_event);

    // Wolf howl sound event ~4 minutes after player arrives (atmospheric, no wolf spawn)
    let hero_pos = Position {
        x: start_location.hero_pos[0],
        y: start_location.hero_pos[1],
    };
    let wolf_howl_event = VisibleEvent::SoundEvent {
        pos: hero_pos,
        sound: "A wolf howls in the distance".to_string(),
        intensity: 10,
    };
    map_events.new(hero_id, game_tick.0 + 2400, wolf_howl_event);

    // Schedule the necromancer event for the next evening
    let ticks_in_day = game_tick.0 % GAME_TICKS_PER_DAY;
    let event_tick = if ticks_in_day < EVENING {
        // Player joined before evening - trigger this same evening
        game_tick.0 + (EVENING - ticks_in_day)
    } else {
        // Player joined at/after evening - trigger next day's evening
        game_tick.0 + (GAME_TICKS_PER_DAY - ticks_in_day + EVENING)
    };

    let event_type = GameEventType::NecroEvent {
        pos: Position {
            x: start_location.necromancer_pos[0],
            y: start_location.necromancer_pos[1],
        },
        home: Position {
            x: start_location.mausoleum_pos[0],
            y: start_location.mausoleum_pos[1],
        },
    };
    let event_id = ids.new_map_event_id();

    let event = GameEvent {
        event_id: event_id,
        start_tick: game_tick.0,
        run_tick: event_tick,
        event_type: event_type,
    };

    game_events.insert(event.event_id, event);

    // Spawn POIs around the player's starting area
    // Burned House - contains loot, guarded by undead
    if let Some(ref pos) = start_location.burned_house_pos {
        let poi_id = ids.new_obj_id();
        let mut poi_inventory = Inventory {
            owner: poi_id,
            items: Vec::new(),
        };
        poi_inventory.new(ids.new_item_id(), "Health Potion".to_string(), 3, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Gold Coins".to_string(), 25, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Copper Broad Axe".to_string(), 1, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Resin Torch".to_string(), 2, &templates.item_templates);

        let poi = Obj::create_nospawn(
            poi_id,
            MERCHANT_PLAYER_ID,
            "Burned House".to_string(),
            Position { x: pos[0], y: pos[1] },
            State::None,
            poi_inventory,
            &templates,
        );
        let poi_entity = commands.spawn(poi).id();
        ids.new_obj(poi_id, MERCHANT_PLAYER_ID);
        entity_map.new_obj(poi_id, poi_entity);
        commands.trigger(NewObj { entity: poi_entity });

        // Spawn skeletons guarding the burned house after 1 minute
        let poi_pos = Position { x: pos[0], y: pos[1] };
        for i in 0..2 {
            let event_id = ids.new_map_event_id();
            let spawn_event = GameEvent {
                event_id,
                start_tick: game_tick.0,
                run_tick: game_tick.0 + 600 + (i * 10),
                event_type: GameEventType::SpawnNPC {
                    npc_type: "Skeleton".to_string(),
                    pos: poi_pos,
                    npc_id: None,
                },
            };
            game_events.insert(spawn_event.event_id, spawn_event);
        }
    }

    // Graveyard - contains soulshards, heavily guarded by undead
    if let Some(ref pos) = start_location.graveyard_pos {
        let poi_id = ids.new_obj_id();
        let mut poi_inventory = Inventory {
            owner: poi_id,
            items: Vec::new(),
        };
        poi_inventory.new(ids.new_item_id(), "Soulshard".to_string(), 3, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Health Potion".to_string(), 2, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Yurt Deed".to_string(), 1, &templates.item_templates);

        let poi = Obj::create_nospawn(
            poi_id,
            MERCHANT_PLAYER_ID,
            "Graveyard".to_string(),
            Position { x: pos[0], y: pos[1] },
            State::None,
            poi_inventory,
            &templates,
        );
        let poi_entity = commands.spawn(poi).id();
        ids.new_obj(poi_id, MERCHANT_PLAYER_ID);
        entity_map.new_obj(poi_id, poi_entity);
        commands.trigger(NewObj { entity: poi_entity });

        // Spawn zombies at the graveyard after 2 minutes
        let poi_pos = Position { x: pos[0], y: pos[1] };
        for i in 0..3 {
            let event_id = ids.new_map_event_id();
            let spawn_event = GameEvent {
                event_id,
                start_tick: game_tick.0,
                run_tick: game_tick.0 + 1200 + (i * 10),
                event_type: GameEventType::SpawnNPC {
                    npc_type: "Zombie".to_string(),
                    pos: poi_pos,
                    npc_id: None,
                },
            };
            game_events.insert(spawn_event.event_id, spawn_event);
        }
    }

    // Sealed Cavern - contains rare materials, guarded by spiders
    if let Some(ref pos) = start_location.sealed_cavern_pos {
        let poi_id = ids.new_obj_id();
        let mut poi_inventory = Inventory {
            owner: poi_id,
            items: Vec::new(),
        };
        poi_inventory.new(ids.new_item_id(), "Quickforge Iron Ore".to_string(), 5, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Valleyrun Copper Ingot".to_string(), 5, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Gold Coins".to_string(), 50, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Mine Deed".to_string(), 1, &templates.item_templates);

        let poi = Obj::create_nospawn(
            poi_id,
            MERCHANT_PLAYER_ID,
            "Sealed Cavern".to_string(),
            Position { x: pos[0], y: pos[1] },
            State::None,
            poi_inventory,
            &templates,
        );
        let poi_entity = commands.spawn(poi).id();
        ids.new_obj(poi_id, MERCHANT_PLAYER_ID);
        entity_map.new_obj(poi_id, poi_entity);
        commands.trigger(NewObj { entity: poi_entity });

        // Spawn spiders guarding the cavern after 3 minutes
        let poi_pos = Position { x: pos[0], y: pos[1] };
        for i in 0..2 {
            let event_id = ids.new_map_event_id();
            let spawn_event = GameEvent {
                event_id,
                start_tick: game_tick.0,
                run_tick: game_tick.0 + 1800 + (i * 10),
                event_type: GameEventType::SpawnNPC {
                    npc_type: "Spider".to_string(),
                    pos: poi_pos,
                    npc_id: None,
                },
            };
            game_events.insert(spawn_event.event_id, spawn_event);
        }
    }

    // Abandoned Mine - contains ore and mining supplies
    if let Some(ref pos) = start_location.abandoned_mine_pos {
        let poi_id = ids.new_obj_id();
        let mut poi_inventory = Inventory {
            owner: poi_id,
            items: Vec::new(),
        };
        poi_inventory.new(ids.new_item_id(), "Valleyrun Copper Ore".to_string(), 10, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Flameforge Copper Ore".to_string(), 5, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Training Pick Axe".to_string(), 1, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Firewood".to_string(), 5, &templates.item_templates);
        poi_inventory.new(ids.new_item_id(), "Quarry Deed".to_string(), 1, &templates.item_templates);

        let poi = Obj::create_nospawn(
            poi_id,
            MERCHANT_PLAYER_ID,
            "Abandoned Mine".to_string(),
            Position { x: pos[0], y: pos[1] },
            State::None,
            poi_inventory,
            &templates,
        );
        let poi_entity = commands.spawn(poi).id();
        ids.new_obj(poi_id, MERCHANT_PLAYER_ID);
        entity_map.new_obj(poi_id, poi_entity);
        commands.trigger(NewObj { entity: poi_entity });

        // Spawn giant rats in the mine after 90 seconds
        let poi_pos = Position { x: pos[0], y: pos[1] };
        for i in 0..3 {
            let event_id = ids.new_map_event_id();
            let spawn_event = GameEvent {
                event_id,
                start_tick: game_tick.0,
                run_tick: game_tick.0 + 900 + (i * 10),
                event_type: GameEventType::SpawnNPC {
                    npc_type: "Giant Rat".to_string(),
                    pos: poi_pos,
                    npc_id: None,
                },
            };
            game_events.insert(spawn_event.event_id, spawn_event);
        }
    }

    /*Encounter::spawn_tax_collector(
        MERCHANT_PLAYER_ID,
        landing_pos,
        empire_pos,
        player_id,
        commands,
        ids,
        entity_map,
        items,
        &templates,
        &game_tick,
        map_events,
    );*/

    Ok(())
}

fn find_nearest_monolith(
    hero_pos: Vec<i32>,
    monoliths: &Query<ObjQuery, With<Monolith>>,
) -> (i32, Position) {
    let mut nearest_distance = i32::MAX;
    let mut nearest_monolith = 0;
    let mut nearest_monolith_pos = Position { x: 0, y: 0 };

    for monolith in monoliths.iter() {
        info!("Monolith: {:?}", monolith.id.0);
        // find the distance between the hero and the monolith
        let distance =
            ((monolith.pos.x - hero_pos[0]).pow(2) + (monolith.pos.y - hero_pos[1]).pow(2)) as i32;
        info!("Distance: {:?}", distance);

        if distance < nearest_distance {
            nearest_distance = distance;
            nearest_monolith = monolith.id.0;
            nearest_monolith_pos = monolith.pos.clone();
        }
    }

    return (nearest_monolith, nearest_monolith_pos);
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct StartLocation {
    pub name: String,
    pub hero_pos: Vec<i32>,
    pub villager_pos: Vec<i32>,
    pub burrow_pos: Vec<i32>,
    pub monolith_pos: Vec<i32>,
    pub shipwreck_pos: Vec<i32>,
    pub corpse1_pos: Vec<i32>,
    pub corpse2_pos: Vec<i32>,
    pub necromancer_pos: Vec<i32>,
    pub mausoleum_pos: Vec<i32>,
    pub merchant_pos: Vec<i32>,
    #[serde(default)]
    pub burned_house_pos: Option<Vec<i32>>,
    #[serde(default)]
    pub graveyard_pos: Option<Vec<i32>>,
    #[serde(default)]
    pub sealed_cavern_pos: Option<Vec<i32>>,
    #[serde(default)]
    pub abandoned_mine_pos: Option<Vec<i32>>,
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct StartLocations(pub Vec<StartLocation>);

impl StartLocations {
    pub fn get_start_location(&mut self) -> Result<StartLocation, String> {
        if self.0.len() == 0 {
            return Err("No start locations available.".to_owned());
        }

        // Randomly select a start location
        let mut rng = rand::thread_rng();

        let start_location_index = rng.gen_range(0..self.0.len());

        // Get the start location and remove it from the list
        let start_location = self.0.remove(start_location_index);

        return Ok(start_location);
    }
}
