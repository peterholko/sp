use bevy::prelude::*;
use big_brain::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::constants::*;
use crate::effect::Effect;
use crate::encounter::Encounter;
use crate::event::{EventExecuting, EventExecutingState};
use crate::game::{
    InitialEncounterEntry, InitialEncounterState, Merchant, MerchantSailState, Monolith, ObjQuery,
    PlayerIntroEntry, PlayerIntroState, SpawnPositions, EARLY_GAME_ENEMY_TEMPLATES,
};
use crate::item::{Inventory, Slot};
use crate::obj::{ActiveShelter, AddLightEffect, Campfire, LastCombatTick, NewObj, UpdateObj};
use crate::tax_collector::{MerchantScorer, MoveToPos, SetDestination};
use crate::trade::WantedItem;
use crate::world::get_time_of_day;

use crate::common::{
    Destination, Drink, Eat, Heat, Hunger, Idle, MoveTo, Sleep, Thirst, Tired, Transport,
};
use crate::villager::{
    ArmedRetaliationScorer, CapacityScorer, DrowsyScorer, EnemyDistanceScorer, ExhaustedScorer,
    FightBack, FindDrink, FindFood, FindShelter, GoodMorale, HeatScorer, HungryScorer, IdleScorer,
    LoadItems, MaybeTransferGatherTool, Morale, ProcessOrder, SetFleeDestination,
    SetOrderDestination, SetStorageDestination, StructureCapacityScorer, ThirstyScorer,
    TransferDrink, TransferFood, UnloadItems,
};

use crate::{
    effect::Effects,
    event::{GameEvent, GameEventType, GameEvents, MapEvents, VisibleEvent},
    game::{BoundMonolith, EncounterMoves, GameTick},
    ids::{EntityObjMap, Ids},
    item::{self},
    obj::Obj,
    obj::{
        ActiveTask, Class, ClassStructure, HeroClass, HeroClassProfile, Id, Misc, Name, Order,
        PlayerId, Position, State, StateAboard, Stats, Storage, Subclass, SubclassHero,
        SubclassVillager, Template, Viewshed,
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
    assigned_start_locations: &mut ResMut<AssignedStartLocations>,
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
    player_intro_state: &mut ResMut<PlayerIntroState>,
    initial_encounter_state: &mut ResMut<InitialEncounterState>,
) -> Result<(), String> {
    // Select a start location and remove it from the list
    let start_location = match start_locations.get_start_location() {
        Ok(start_location) => start_location,
        Err(e) => return Err(e),
    };

    // Remember the assignment so True Death can release this location back to the pool.
    assigned_start_locations.insert(player_id, start_location.clone());

    // Record spawn position for crisis tracking
    spawn_positions.insert(
        player_id,
        Position {
            x: start_location.hero_pos[0],
            y: start_location.hero_pos[1],
        },
    );

    player_intro_state.insert(
        player_id,
        PlayerIntroEntry {
            start_tick: game_tick.0,
            shipwreck_chain_started: false,
            villager_spawned: false,
            danger_unlocked: false,
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
    let hero_class = HeroClass::from_str(&class_name).unwrap_or_default();
    let hero_profile = HeroClassProfile::for_class(hero_class);
    let hero_template_name = hero_profile.novice_template.to_string();
    let hero_template = templates.obj_templates.get(hero_template_name.clone());
    let base_mana = hero_template.base_mana.unwrap_or(hero_profile.base_mana);

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
        2,
        &templates.item_templates,
    );
    let sharpened_stick = inventory.new(
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
    if time_of_day == crate::world::TimeOfDay::Dusk || time_of_day == crate::world::TimeOfDay::Night
    {
        inventory.equip(torch.id, Some(Slot::OffHand));
    }

    match hero_class {
        HeroClass::Warrior => {
            let weapon_attrs = warrior_starting_weapon_attrs();

            let axe = inventory.new_with_attrs(
                ids.new_item_id(),
                hero_id,
                "Copper Training Axe".to_string(),
                1,
                weapon_attrs,
                &templates.item_templates,
            );
            inventory.equip(axe.0.id, Some(Slot::MainHand));

            let mut armor_attrs = HashMap::new();
            armor_attrs.insert(item::AttrKey::Defense, item::AttrVal::Num(3.0));

            let helm = inventory.new_with_attrs(
                ids.new_item_id(),
                hero_id,
                "Copper Helm".to_string(),
                1,
                armor_attrs,
                &templates.item_templates,
            );
            inventory.equip(helm.0.id, Some(Slot::Helm));
        }
        HeroClass::Ranger => {
            let mut bow_attrs = HashMap::new();
            bow_attrs.insert(item::AttrKey::Damage, item::AttrVal::Num(8.0));
            bow_attrs.insert(item::AttrKey::Hunting, item::AttrVal::Num(2.0));
            bow_attrs.insert(item::AttrKey::AttackRange, item::AttrVal::Num(3.0));
            bow_attrs.insert(item::AttrKey::Accuracy, item::AttrVal::Num(85.0));

            let bow = inventory.new_with_attrs(
                ids.new_item_id(),
                hero_id,
                "Training Bow".to_string(),
                1,
                bow_attrs,
                &templates.item_templates,
            );
            inventory.equip(bow.0.id, Some(Slot::MainHand));
        }
        HeroClass::Mage => {
            inventory.equip(sharpened_stick.id, Some(Slot::MainHand));
            inventory.new(
                ids.new_item_id(),
                "Mana".to_string(),
                5,
                &templates.item_templates,
            );
        }
    }

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
            hsl: start_location.hsl.clone(),
            groups: Vec::new(),
        },
        stats: Stats {
            hp: hero_template.base_hp.unwrap(),
            base_hp: hero_template.base_hp.unwrap(),
            stamina: hero_template.base_stamina,
            mana: Some(base_mana),
            base_stamina: hero_template.base_stamina,
            base_mana: Some(base_mana),
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
            hero_class,
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
    if time_of_day == crate::world::TimeOfDay::Dusk || time_of_day == crate::world::TimeOfDay::Night
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
        .step(MaybeTransferGatherTool)
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
            .when(ArmedRetaliationScorer, FightBack)
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
                .when(ArmedRetaliationScorer, FightBack)
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
    recipes.create(player_id, "Sharpened Stick".to_string(), &templates);
    recipes.create(player_id, "Crude Torch".to_string(), &templates);
    recipes.create(player_id, "Crude Bandage".to_string(), &templates);
    recipes.create(player_id, "Twine".to_string(), &templates);
    recipes.create(player_id, "Improvised Sling".to_string(), &templates);
    recipes.create(player_id, "Stone Knife".to_string(), &templates);
    recipes.create(player_id, "Resin Torch".to_string(), &templates);
    recipes.create(player_id, "Herbal Poultice".to_string(), &templates);
    recipes.create(player_id, "Hide Wraps".to_string(), &templates);

    // Starting plans (survival basics only — more plans acquired through exploration and villager)
    plans.add(player_id, "Campfire".to_string(), 0, 0);
    plans.add(player_id, "Burrow".to_string(), 0, 0);
    plans.add(player_id, "Stockade".to_string(), 0, 0);
    plans.add(player_id, "Crafting Tent".to_string(), 0, 0);

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

    // Wanted items keyed by subclass so any biome/colour variant matches via the
    // name → subclass → class fallthrough in trade.rs::find_buy_price.
    let wanted_items = vec![
        WantedItem::new_by_subclass("Copper Ore".to_string()),
        WantedItem::new_by_subclass("Iron Ore".to_string()),
        WantedItem::new_by_subclass("Copper Ingot".to_string()),
        WantedItem::new_by_subclass("Iron Ingot".to_string()),
        WantedItem::new_by_subclass("Maple Log".to_string()),
        WantedItem::new_by_subclass("Maple Timber".to_string()),
        WantedItem::new_by_subclass("Raw Hide".to_string()),
        WantedItem::new_by_subclass("Stiff Leather".to_string()),
        WantedItem::new_by_subclass("Cooked Meat".to_string()),
        WantedItem::new_by_subclass("Honeybell Cloth".to_string()),
    ];

    let merchant_component = Merchant {
        trade_port: empire_pos,
        landing_at: landing_pos,
        wanted_items,
        sail_state: MerchantSailState::AtEmpire,
    };

    let merchant_template_name = "Meager Merchant".to_string();

    // Merchant inventory — the canonical list is in game.rs::MERCHANT_INVENTORY
    // and is reused by the restock path on each return trip.
    for (item_name, qty) in crate::game::MERCHANT_INVENTORY.iter() {
        merchant.inventory.new(
            ids.new_item_id(),
            (*item_name).to_string(),
            *qty,
            &templates.item_templates,
        );
    }

    // Spawn the merchant offshore at empire_pos. They stay there (out of the
    // player's viewshed) until MerchantArrival fires, scheduled from the
    // SpawnVillager handler in game.rs ~3 minutes after the villager rescue.
    // The big-brain Thinker / Transport sail-in is intentionally omitted for
    // this slice — see plan note "Out of scope".
    let viewshed_range = Obj::set_viewshed_range(
        merchant_id,
        merchant_template_name.clone(),
        game_tick.0,
        &merchant.inventory,
        templates,
        0.0,
    );

    let merchant_entity_id = commands
        .spawn((
            merchant,
            Viewshed {
                range: viewshed_range,
            },
            merchant_component,
        ))
        .id();

    ids.new_obj(merchant_id, merchant_player_id);
    entity_map.new_obj(merchant_id, merchant_entity_id);

    map_events.new(merchant_id, game_tick.0 + 1, VisibleEvent::NewObjEvent);

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
        "Cragroot Maple Timber".to_string(),
        15,
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

    // Scripted shipwreck intro pacing is handled relative to the player's join time
    let shipwreck_pos = Position {
        x: start_location.shipwreck_pos[0],
        y: start_location.shipwreck_pos[1],
    };

    let mut rat_ids = Vec::new();
    for i in 0..2 {
        let rat_npc_id = ids.new_obj_id();
        rat_ids.push(rat_npc_id);
    }

    // Register the initial encounter chain: two pests, then boar/crab, then spider.
    // The villager waits for shipwreck inspection, but only after the help call has fired.
    let villager_spawn_pos = Position {
        x: start_location.villager_pos[0],
        y: start_location.villager_pos[1],
    };
    let villager_help_tick = game_tick.0 + 1100;
    let first_enemy_index = rand::thread_rng().gen_range(0..EARLY_GAME_ENEMY_TEMPLATES.len());
    let mut second_enemy_index =
        rand::thread_rng().gen_range(0..EARLY_GAME_ENEMY_TEMPLATES.len() - 1);
    if second_enemy_index >= first_enemy_index {
        second_enemy_index += 1;
    }
    let opening_enemy_templates = vec![
        EARLY_GAME_ENEMY_TEMPLATES[first_enemy_index].to_string(),
        EARLY_GAME_ENEMY_TEMPLATES[second_enemy_index].to_string(),
    ];
    let phase1_spawn = if rand::thread_rng().gen_range(0..2) == 0 {
        "Giant Crab".to_string()
    } else {
        "Wild Boar".to_string()
    };

    // Spawn the necromancer and its mausoleum up front, but hidden: State::Hiding
    // keeps them out of every perception path and we deliberately skip the NewObj
    // trigger so the client is never told they exist. They are revealed and
    // activated later by the NecroEvent, which is scheduled 5 minutes after the
    // villager is rescued (see the SpawnVillager handler in game.rs).
    let mausoleum_pos = Position {
        x: start_location.mausoleum_pos[0],
        y: start_location.mausoleum_pos[1],
    };
    let (_necro_entity, necromancer_id, _necro_player_id, _necro_pos) =
        Encounter::spawn_dormant_necromancer(
            NPC_PLAYER_ID,
            mausoleum_pos,
            mausoleum_pos,
            commands,
            ids,
            entity_map,
            templates,
        );
    let mausoleum_id = ids.new_obj_id();
    let mausoleum = Obj::create_nospawn(
        mausoleum_id,
        NPC_PLAYER_ID,
        "Mausoleum".to_string(),
        mausoleum_pos,
        State::Hiding,
        Inventory {
            owner: mausoleum_id,
            items: Vec::new(),
        },
        templates,
    );
    let mausoleum_entity = commands.spawn(mausoleum).id();
    ids.new_obj(mausoleum_id, NPC_PLAYER_ID);
    entity_map.new_obj(mausoleum_id, mausoleum_entity);

    initial_encounter_state.insert(
        player_id,
        InitialEncounterEntry {
            rat_ids,
            opening_enemy_templates,
            phase1_spawn,
            phase1_npc_id: None,
            spawn_pos: shipwreck_pos,
            villager_spawn_pos,
            first_rat_spawn_tick: game_tick.0 + 900,
            second_rat_spawn_tick: game_tick.0 + 1200,
            villager_ready_tick: villager_help_tick + TICKS_PER_SEC,
            phase1_unlock_tick: game_tick.0 + 2600,
            spider_unlock_tick: game_tick.0 + 3600,
            villager_event_scheduled: false,
            merchant_id,
            necromancer_id: necromancer_id.0,
            mausoleum_id,
            necro_spawn_anchor: mausoleum_pos,
            necro_corpse_anchor: shipwreck_pos,
            necro_home: mausoleum_pos,
        },
    );

    let intro_notice = GameEvent {
        event_id: ids.new_map_event_id(),
        start_tick: game_tick.0,
        run_tick: game_tick.0 + 120,
        event_type: GameEventType::PlayerNotice {
            player_id,
            message: "Survival thread started: inspect the shipwreck, check your burrow, then build fire before dusk.".to_string(),
            expiry: Some(10000),
        },
    };
    game_events.insert(intro_notice.event_id, intro_notice);

    // BB-B: the campfire lesson is now delivered as an action-driven nudge when
    // the player actually builds a campfire (see objectives_system), instead of
    // firing on a fixed clock here.

    let distress_notice = GameEvent {
        event_id: ids.new_map_event_id(),
        start_tick: game_tick.0,
        run_tick: villager_help_tick,
        event_type: GameEventType::PlayerNotice {
            player_id,
            message: "A voice cries out from the shipwreck. Someone may still be alive."
                .to_string(),
            expiry: Some(10000),
        },
    };
    game_events.insert(distress_notice.event_id, distress_notice);

    // Distress call from the shipwreck after the first pressure beat. Anchored to
    // the shipwreck object (rather than a bare position) so it renders as an HTML
    // speech bubble in the UI layer (SpeechBubbleLayer) instead of canvas text.
    // Intensity 5 preserves the original audible radius.
    let distress_event = VisibleEvent::SpeechEvent {
        speech: "A desperate voice calls from the shipwreck: \"Is anyone out there?!\"".to_string(),
        intensity: 5,
    };
    map_events.new(shipwreck_id, villager_help_tick, distress_event);

    // Wolf howl sound event after the player has learned the first camp loop
    let hero_pos = Position {
        x: start_location.hero_pos[0],
        y: start_location.hero_pos[1],
    };
    let wolf_howl_event = VisibleEvent::SoundEvent {
        pos: hero_pos,
        sound: "A wolf howls in the distance".to_string(),
        intensity: 10,
    };
    map_events.new(hero_id, game_tick.0 + 3000, wolf_howl_event);

    // Spawn POIs around the player's starting area
    // Burned House - contains loot, guarded by undead
    if let Some(ref pos) = start_location.burned_house_pos {
        let poi_id = ids.new_obj_id();
        let mut poi_inventory = Inventory {
            owner: poi_id,
            items: Vec::new(),
        };
        poi_inventory.new(
            ids.new_item_id(),
            "Health Potion".to_string(),
            3,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Gold Coins".to_string(),
            25,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Copper Broad Axe".to_string(),
            1,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Resin Torch".to_string(),
            2,
            &templates.item_templates,
        );

        let poi = Obj::create_nospawn(
            poi_id,
            MERCHANT_PLAYER_ID,
            "Burned House".to_string(),
            Position {
                x: pos[0],
                y: pos[1],
            },
            State::None,
            poi_inventory,
            &templates,
        );
        let poi_entity = commands.spawn(poi).id();
        ids.new_obj(poi_id, MERCHANT_PLAYER_ID);
        entity_map.new_obj(poi_id, poi_entity);
        commands.trigger(NewObj { entity: poi_entity });

        // Spawn skeletons guarding the burned house after 8 minutes
        let poi_pos = Position {
            x: pos[0],
            y: pos[1],
        };
        for i in 0..2 {
            let event_id = ids.new_map_event_id();
            let spawn_event = GameEvent {
                event_id,
                start_tick: game_tick.0,
                run_tick: game_tick.0 + 4800 + (i * 10),
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
        poi_inventory.new(
            ids.new_item_id(),
            "Soulshard".to_string(),
            3,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Health Potion".to_string(),
            2,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Yurt Deed".to_string(),
            1,
            &templates.item_templates,
        );

        let poi = Obj::create_nospawn(
            poi_id,
            MERCHANT_PLAYER_ID,
            "Graveyard".to_string(),
            Position {
                x: pos[0],
                y: pos[1],
            },
            State::None,
            poi_inventory,
            &templates,
        );
        let poi_entity = commands.spawn(poi).id();
        ids.new_obj(poi_id, MERCHANT_PLAYER_ID);
        entity_map.new_obj(poi_id, poi_entity);
        commands.trigger(NewObj { entity: poi_entity });

        // Spawn zombies at the graveyard after 12 minutes
        let poi_pos = Position {
            x: pos[0],
            y: pos[1],
        };
        for i in 0..3 {
            let event_id = ids.new_map_event_id();
            let spawn_event = GameEvent {
                event_id,
                start_tick: game_tick.0,
                run_tick: game_tick.0 + 7200 + (i * 10),
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
        poi_inventory.new(
            ids.new_item_id(),
            "Quickforge Iron Ore".to_string(),
            5,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Valleyrun Copper Ingot".to_string(),
            5,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Gold Coins".to_string(),
            50,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Mine Deed".to_string(),
            1,
            &templates.item_templates,
        );

        let poi = Obj::create_nospawn(
            poi_id,
            MERCHANT_PLAYER_ID,
            "Sealed Cavern".to_string(),
            Position {
                x: pos[0],
                y: pos[1],
            },
            State::None,
            poi_inventory,
            &templates,
        );
        let poi_entity = commands.spawn(poi).id();
        ids.new_obj(poi_id, MERCHANT_PLAYER_ID);
        entity_map.new_obj(poi_id, poi_entity);
        commands.trigger(NewObj { entity: poi_entity });

        // Spawn spiders guarding the cavern after 14 minutes
        let poi_pos = Position {
            x: pos[0],
            y: pos[1],
        };
        for i in 0..2 {
            let event_id = ids.new_map_event_id();
            let spawn_event = GameEvent {
                event_id,
                start_tick: game_tick.0,
                run_tick: game_tick.0 + 8400 + (i * 10),
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
        poi_inventory.new(
            ids.new_item_id(),
            "Valleyrun Copper Ore".to_string(),
            10,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Flameforge Copper Ore".to_string(),
            5,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Training Pick Axe".to_string(),
            1,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Firewood".to_string(),
            5,
            &templates.item_templates,
        );
        poi_inventory.new(
            ids.new_item_id(),
            "Quarry Deed".to_string(),
            1,
            &templates.item_templates,
        );

        let poi = Obj::create_nospawn(
            poi_id,
            MERCHANT_PLAYER_ID,
            "Abandoned Mine".to_string(),
            Position {
                x: pos[0],
                y: pos[1],
            },
            State::None,
            poi_inventory,
            &templates,
        );
        let poi_entity = commands.spawn(poi).id();
        ids.new_obj(poi_id, MERCHANT_PLAYER_ID);
        entity_map.new_obj(poi_id, poi_entity);
        commands.trigger(NewObj { entity: poi_entity });

        // Spawn low-tier pests in the mine after 10 minutes
        let poi_pos = Position {
            x: pos[0],
            y: pos[1],
        };
        for i in 0..3 {
            let enemy_index = rand::thread_rng().gen_range(0..EARLY_GAME_ENEMY_TEMPLATES.len());
            let npc_type = EARLY_GAME_ENEMY_TEMPLATES[enemy_index].to_string();
            let event_id = ids.new_map_event_id();
            let spawn_event = GameEvent {
                event_id,
                start_tick: game_tick.0,
                run_tick: game_tick.0 + 6000 + (i * 10),
                event_type: GameEventType::SpawnNPC {
                    npc_type,
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

fn warrior_starting_weapon_attrs() -> HashMap<item::AttrKey, item::AttrVal> {
    let mut weapon_attrs = HashMap::new();
    weapon_attrs.insert(item::AttrKey::Damage, item::AttrVal::Num(11.0));
    weapon_attrs.insert(item::AttrKey::Logging, item::AttrVal::Num(2.0));
    weapon_attrs.insert(item::AttrKey::DeepWoundChance, item::AttrVal::Num(0.9));
    weapon_attrs
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
    // Team color (HSL: [hue 0-360, sat 0-100, light 0-100]) assigned at startup by
    // `assign_start_location_colors`. Empty in the YAML; filled in after load so the
    // hero + villagers spawned at this location share a distinct color. Travels to the
    // client via each obj's `Misc.hsl`, where the pinkish "team" pixels are recolored.
    #[serde(default)]
    pub hsl: Vec<i32>,
}

/// Curated, visually-distinct HSL colors ([hue, sat, light]). One is assigned to
/// each start location so every player's hero and villagers read as a distinct team
/// color. There are more entries than start locations so the shuffle has slack.
pub const LOCATION_COLOR_PALETTE: [[i32; 3]; 6] = [
    [210, 75, 55], // blue
    [130, 55, 45], // green
    [28, 90, 55],  // orange
    [275, 65, 60], // purple
    [350, 75, 55], // crimson
    [48, 90, 55],  // gold
];

/// Randomly assign a distinct palette color to each start location, in place.
/// Called once after `player_start.yaml` is loaded. If there happen to be more
/// locations than palette entries the palette wraps (still deterministic per run).
pub fn assign_start_location_colors(locations: &mut [StartLocation]) {
    use rand::seq::SliceRandom;
    let mut palette: Vec<[i32; 3]> = LOCATION_COLOR_PALETTE.to_vec();
    palette.shuffle(&mut rand::thread_rng());
    for (i, location) in locations.iter_mut().enumerate() {
        let color = palette[i % palette.len()];
        location.hsl = vec![color[0], color[1], color[2]];
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct StartLocations(pub Vec<StartLocation>);

// Tracks which start location each player was handed, keyed by player id, so the
// location can be returned to `StartLocations` when that player's hero meets True
// Death. In-memory only: this is rebuilt empty on restart (as is StartLocations
// itself, which reloads from player_start.yaml).
#[derive(Debug, Default, Resource, Deref, DerefMut)]
pub struct AssignedStartLocations(pub HashMap<i32, StartLocation>);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{Item, LOG, WEAPON};

    #[test]
    fn warrior_starting_axe_counts_as_log_gathering_tool() {
        let axe = Item {
            id: 1,
            owner: 1,
            name: "Copper Training Axe".to_string(),
            quantity: 1,
            durability: None,
            class: WEAPON.to_string(),
            subclass: "Axe".to_string(),
            slot: Some(Slot::MainHand),
            image: "trainingaxe".to_string(),
            weight: 10.0,
            equipped: false,
            experiment: None,
            start_time: 0,
            attrs: warrior_starting_weapon_attrs(),
            produces: Vec::new(),
        };

        assert!(axe.is_gather_tool_for_res_type(LOG));
    }
}
