use bevy::prelude::*;
use big_brain::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::constants::*;
use crate::encounter::Encounter;
use crate::event::{EventExecuting, EventExecutingState};
use crate::game::{
    random_opening_rat_count, InitialEncounterEntry, InitialEncounterState, IntroEncounterState,
    Merchant, MerchantSailState, Monolith, ObjQuery, PlayerIntroEncounters, PlayerIntroEntry,
    PlayerIntroState, SpawnPositions, EARLY_GAME_ENEMY_TEMPLATES,
};
use crate::item::Inventory;
use crate::obj::{
    ActiveShelter, Assignments, Campfire, LastCombatTick, NewObj, UpdateObj, WorkQueue,
};
use crate::tax_collector::{MerchantScorer, MoveToPos, SetDestination};
use crate::trade::WantedItem;

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
    effect::{ControlEffectDiminishingReturns, Effects},
    event::{GameEvent, GameEventType, GameEvents, MapEvents, VisibleEvent},
    game::{BoundMonolith, EncounterMoves, GameTick},
    ids::{EntityObjMap, Ids},
    item::{self},
    obj::Obj,
    obj::{
        ActiveTask, Class, ClassStructure, HeroClass, HeroClassProfile, Id, Misc, Name, Order,
        PlayerId, Portrait, Position, State, StateAboard, Stats, Subclass, SubclassHero,
        SubclassVillager, Template, Viewshed, VILLAGER_PORTRAITS,
    },
    recipe::Recipes,
    skill::Skills,
    structure::{Plans, WELL},
    templates::{ObjTemplate, Templates},
    villager_util::{self, VillagerUtil},
};

pub fn new(
    player_id: i32,
    hero_name: String,
    class_name: String,
    portrait: String,
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
    intro_encounter_state: &mut ResMut<IntroEncounterState>,
    initial_encounter_state: &mut ResMut<InitialEncounterState>,
    run_spawned_objs: &mut ResMut<RunSpawnedObjs>,
) -> Result<(), String> {
    // Select a start location and remove it from the list
    let start_location = match start_locations.get_start_location() {
        Ok(start_location) => start_location,
        Err(e) => return Err(e),
    };

    // Remember the assignment so True Death can release this location back to the pool.
    assigned_start_locations.insert(player_id, start_location.clone());

    // Ids of this run's scripted spawns, recorded so True Death can clean the
    // start area before the location is recycled.
    let mut run_obj_ids: Vec<i32> = Vec::new();

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
    intro_encounter_state.insert(player_id, PlayerIntroEncounters::default());

    // Find nearest monolith
    let (monolith_id, monolith_pos) =
        find_nearest_monolith(start_location.hero_pos.clone(), &monoliths);

    info!("Nearest monolith: {:?}", monolith_id);
    info!("Nearest monolith position: {:?}", monolith_pos);

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

    // A fresh hero reaches the wreck with basic clothing and a carried starter
    // weapon. Survival supplies, tools, and class equipment are recovered
    // manually from that run's Shipwreck below.
    inventory.add_equipped_tattered_clothing(
        ids.new_item_id(),
        ids.new_item_id(),
        &templates.item_templates,
    );
    inventory.new(
        ids.new_item_id(),
        "Sharpened Stick".to_string(),
        1,
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
        control_effect_dr: ControlEffectDiminishingReturns::default(),
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
            Portrait(portrait),
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
            ActiveTask::None,
            EventExecuting {
                event_type: "".to_string(),
                state: EventExecutingState::None,
            },
            EncounterMoves(0),
            bound_monolith,
            hero_class,
            SubclassHero, // Hero component tag
            // 0.012/tick ≈ full→lethal in ~20 real minutes (~5 game days), so the
            // threat curve — not biology — is what ends a run. (Was 0.025: needs
            // outran the first nightly wave and killed 80% of recorded runs.)
            Thirst::new(0.0, 0.012),
            Hunger::new(0.0, 0.012),
            Tired::new(0.0, 0.012),
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

    // Always start the hero with a lit campfire: hunting + cooking is a legitimate
    // early food source for a small population, so the cook economy must be
    // available from day 1 (not after ~5 days of gathering Stick+Resin to build
    // one). The bot-side hunt/cook/eat loop bugs that once made an early campfire
    // counterproductive (hunt spinning, cook errands preempting eating) are fixed.
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

        // The starting fire remains ordinary inventory-backed fuel so it can be
        // cooked with or transferred through the existing item systems.
        campfire.inventory.new(
            ids.new_item_id(),
            "Firewood".to_string(),
            30,
            &templates.item_templates,
        );

        // Get the campfire template to check for vision
        let campfire_template = templates.obj_templates.get("Campfire".to_string());

        // Spawn the campfire entity with the same companion components a
        // foundation-built structure gets (see the CreateFoundation handler):
        // ClassStructure registers it with structure queries (perception, cook/craft
        // lookups), and WorkQueue/Assignments are required by the structure-craft
        // and work-assignment systems — StructureCraft at a structure missing
        // WorkQueue fails its query and (before the wedge fix) left the crafter
        // stuck in State::Crafting.
        let campfire_entity_id = if let Some(vision) = campfire_template.base_vision {
            commands
                .spawn((
                    campfire,
                    ClassStructure,
                    WorkQueue(Vec::new()),
                    Assignments(Vec::new()),
                    Viewshed { range: vision },
                ))
                .id()
        } else {
            commands
                .spawn((
                    campfire,
                    ClassStructure,
                    WorkQueue(Vec::new()),
                    Assignments(Vec::new()),
                ))
                .id()
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
        control_effect_dr: ControlEffectDiminishingReturns::default(),
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
    recipes.create(player_id, "Sickle".to_string(), &templates);
    recipes.create(player_id, FISHING_ROD.to_string(), &templates);
    recipes.create(player_id, "Training Pick Axe".to_string(), &templates);
    recipes.create(
        player_id,
        "Training Stonecutter Hammer".to_string(),
        &templates,
    );
    recipes.create(player_id, "Copper Training Axe".to_string(), &templates);
    recipes.create(player_id, "Copper Felling Axe".to_string(), &templates);
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

    // Primitive equipment is the dependable bridge between Shipwreck salvage
    // and specialized production. These recipes still require their ordinary
    // gathered ingredients; knowing them up front prevents core progression
    // from depending on finding an exact same-family experiment source.
    recipes.create(player_id, "Bone Dagger".to_string(), &templates);
    recipes.create(player_id, "Flint Hatchet".to_string(), &templates);
    recipes.create(player_id, "Bone-Tipped Spear".to_string(), &templates);
    recipes.create(player_id, "Bone War Club".to_string(), &templates);
    recipes.create(player_id, "Throwing Spear".to_string(), &templates);
    recipes.create(player_id, "Hide Cap".to_string(), &templates);
    recipes.create(player_id, "Hide Leggings".to_string(), &templates);
    recipes.create(player_id, "Hide Boots".to_string(), &templates);
    recipes.create(player_id, "Hide Mantle".to_string(), &templates);

    // Foundational station recipes are deterministic. Their structure and
    // skill requirements remain authoritative, while experimentation carries
    // each item family onward into its iron and mithril tiers.
    recipes.create(player_id, "Copper Dagger".to_string(), &templates);
    recipes.create(player_id, "Copper Spear".to_string(), &templates);
    recipes.create(player_id, "Copper Mace".to_string(), &templates);
    recipes.create(player_id, "Copper Short Sword".to_string(), &templates);
    recipes.create(player_id, "Copper Cuirass".to_string(), &templates);
    recipes.create(player_id, "Copper Greaves".to_string(), &templates);
    recipes.create(player_id, "Copper Sabatons".to_string(), &templates);
    recipes.create(player_id, "Copper Pauldrons".to_string(), &templates);
    recipes.create(player_id, "Copper Buckler".to_string(), &templates);
    recipes.create(player_id, "Training Bow".to_string(), &templates);
    recipes.create(player_id, "Hunting Bow".to_string(), &templates);

    // Starting plans. The rescued villager now provides the Burrow,
    // Lumbercamp, Shelter Tent, and Stockade deeds as the opening settlement
    // chain advances.
    plans.add(player_id, "Campfire".to_string(), 0, 0);
    plans.add(player_id, "Crafting Tent".to_string(), 0, 0);
    plans.add(player_id, WELL.to_string(), 0, 0);

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
    run_obj_ids.push(merchant_id);

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
    // SpawnVillager handler in game.rs five game days after the villager rescue.
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

    // Castaway villagers the merchant carries for hire. Spawned offshore at
    // empire_pos alongside the merchant, owned by the merchant, as bare cargo
    // (Obj + BaseAttrs + Skills, no AI/needs). `info_hire_system` lists them off
    // `Transport.hauling`; the hire flow re-homes the chosen one to the player and
    // attaches its villager behaviour (see player::hire_system /
    // Encounter::convert_cargo_to_villager).
    const MERCHANT_HIRE_VILLAGERS: usize = 3;
    let mut hauling: Vec<i32> = Vec::new();
    for _ in 0..MERCHANT_HIRE_VILLAGERS {
        let cargo_id = ids.new_obj_id();
        let mut cargo_inventory = Inventory {
            owner: cargo_id,
            items: Vec::new(),
        };
        cargo_inventory.add_equipped_tattered_clothing(
            ids.new_item_id(),
            ids.new_item_id(),
            &templates.item_templates,
        );
        let mut cargo = Obj::create_nospawn(
            cargo_id,
            merchant_player_id,
            "Human Villager".to_string(),
            empire_pos,
            State::None,
            cargo_inventory,
            templates,
        );
        cargo.name = Name(VillagerUtil::generate_name());

        let cargo_attrs = VillagerUtil::generate_attributes(1);
        let cargo_skills = VillagerUtil::generate_skills(cargo_id, &templates.skill_templates);

        let portrait = VILLAGER_PORTRAITS
            [rand::thread_rng().gen_range(0..VILLAGER_PORTRAITS.len())]
        .to_string();
        let cargo_entity = commands
            .spawn((cargo, cargo_attrs, cargo_skills, Portrait(portrait)))
            .id();
        ids.new_obj(cargo_id, merchant_player_id);
        entity_map.new_obj(cargo_id, cargo_entity);
        run_obj_ids.push(cargo_id);
        hauling.push(cargo_id);
    }

    let merchant_entity_id = commands
        .spawn((
            merchant,
            Viewshed {
                range: viewshed_range,
            },
            merchant_component,
            Transport {
                route: Vec::new(),
                next_stop: 0,
                hauling,
            },
            // Required by the move-application system (move_system) — without it the
            // merchant's sail MoveEvents never resolve and it stays stuck offshore.
            EventExecuting {
                event_type: "".to_string(),
                state: EventExecutingState::None,
            },
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

    // Create the run-owned starter salvage cache. The POI remains neutral in
    // ordinary object ownership; RunSpawnedObjs is the authoritative association
    // used by the narrow investigation/transfer permission checks.
    let shipwreck_id = ids.new_obj_id();
    let mut shipwreck_inventory = Inventory {
        owner: shipwreck_id,
        items: Vec::new(),
    };

    // General survival supplies. Two Crude Hatchets let the hero and rescued
    // villager both work the Logging chain. Each matches the hero's carried
    // Sharpened Stick combat profile while providing the ordinary Logging tool
    // attribute needed to lumberjack trees.
    for _ in 0..2 {
        shipwreck_inventory.new(
            ids.new_item_id(),
            "Crude Hatchet".to_string(),
            1,
            &templates.item_templates,
        );
    }
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Foraging Kit".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Crude Torch".to_string(),
        3,
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
        "Waterskin (Filled)".to_string(),
        3,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Salted Meat Strip".to_string(),
        3,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Honeybell Berries".to_string(),
        3,
        &templates.item_templates,
    );
    // The two bodies in the wreck are distinct inventory items. Corpse-class
    // items are deliberately non-mergeable, so each keeps its own item id and
    // occupies its own inventory slot rather than becoming quantity two.
    for _ in 0..2 {
        shipwreck_inventory.new_with_attrs(
            ids.new_item_id(),
            shipwreck_id,
            "Human Corpse".to_string(),
            1,
            HashMap::new(),
            &templates.item_templates,
        );
    }
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Health Potion".to_string(),
        1,
        &templates.item_templates,
    );

    // Basic crafting materials.
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Flint Shard".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Cragroot Maple Resin".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Cragroot Maple Stick".to_string(),
        1,
        &templates.item_templates,
    );
    // Enough existing Cloth to turn the known Twine recipe into a real early
    // crafting choice without introducing a new cordage resource.
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Honeybell Cloth".to_string(),
        5,
        &templates.item_templates,
    );
    // The first skin starts the Shelter Tent material lesson. Hunting and
    // butchering after the survivor is rescued teaches the renewable source.
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Windstride Raw Hide".to_string(),
        1,
        &templates.item_templates,
    );

    // Settlement salvage.
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Springbranch Maple Log".to_string(),
        5,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Cragroot Maple Timber".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Valleyrun Copper Ingot".to_string(),
        1,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        "Gold Coins".to_string(),
        10,
        &templates.item_templates,
    );
    shipwreck_inventory.new(
        ids.new_item_id(),
        FISHING_ROD.to_string(),
        1,
        &templates.item_templates,
    );

    // Preserve each class's actual inventory equipment and per-instance values,
    // but recover it from the wreck instead of spawning it on the hero.
    match hero_class {
        HeroClass::Warrior => {
            let mut armor_attrs = HashMap::new();
            armor_attrs.insert(item::AttrKey::Defense, item::AttrVal::Num(3.0));
            shipwreck_inventory.new_with_attrs(
                ids.new_item_id(),
                shipwreck_id,
                "Copper Helm".to_string(),
                1,
                armor_attrs,
                &templates.item_templates,
            );
        }
        HeroClass::Ranger => {
            let mut bow_attrs = HashMap::new();
            bow_attrs.insert(item::AttrKey::Damage, item::AttrVal::Num(8.0));
            bow_attrs.insert(item::AttrKey::Hunting, item::AttrVal::Num(2.0));
            bow_attrs.insert(item::AttrKey::AttackRange, item::AttrVal::Num(2.0));
            bow_attrs.insert(item::AttrKey::Accuracy, item::AttrVal::Num(85.0));
            shipwreck_inventory.new_with_attrs(
                ids.new_item_id(),
                shipwreck_id,
                "Training Bow".to_string(),
                1,
                bow_attrs,
                &templates.item_templates,
            );
        }
        HeroClass::Mage => {
            shipwreck_inventory.new(
                ids.new_item_id(),
                "Mana".to_string(),
                5,
                &templates.item_templates,
            );
        }
    }

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
    run_obj_ids.push(shipwreck_id);

    commands.trigger(NewObj {
        entity: shipwreck_entity_id,
    });

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

    let opening_rat_count = random_opening_rat_count(&mut rand::thread_rng());
    let mut rat_ids = Vec::with_capacity(opening_rat_count);
    for _ in 0..opening_rat_count {
        let rat_npc_id = ids.new_obj_id();
        rat_ids.push(rat_npc_id);
    }

    // Register the initial encounter chain: one rat wave, then boar/crab, then spider.
    // The first completed Shipwreck investigation rescues the survivor. The
    // opening ambush interrupts the initial attempt, so no villager appears
    // until the player successfully completes a later investigation.
    let villager_spawn_pos = Position {
        x: start_location.villager_pos[0],
        y: start_location.villager_pos[1],
    };
    let phase1_spawn = if rand::thread_rng().gen_range(0..2) == 0 {
        "Giant Crab".to_string()
    } else {
        "Wild Boar".to_string()
    };

    // Spawn the necromancer and its mausoleum outside the initial sanctuary, but
    // hidden: State::Hiding keeps them out of every perception path and we
    // deliberately skip the NewObj trigger so the client is never told they
    // exist. They are revealed and activated later by the NecroEvent, which is
    // scheduled 5 minutes after the villager is rescued (see the SpawnVillager
    // handler in game.rs).
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

    run_obj_ids.push(necromancer_id.0);
    run_obj_ids.push(mausoleum_id);
    run_obj_ids.extend(rat_ids.iter().copied());

    initial_encounter_state.insert(
        player_id,
        InitialEncounterEntry {
            rat_ids,
            opening_rat_ambush_armed: false,
            opening_enemy_spawned: vec![false; opening_rat_count],
            opening_enemy_defeated: vec![false; opening_rat_count],
            phase1_spawn,
            phase1_npc_id: None,
            phase1_defeated: false,
            spawn_pos: shipwreck_pos,
            villager_spawn_pos,
            opening_rat_spawn_tick: game_tick.0 + 900,
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
            message: "Survival thread started: search the Shipwreck, recover your supplies, and build a Burrow beside the lit Campfire.".to_string(),
            expiry: Some(10000),
        },
    };
    game_events.insert(intro_notice.event_id, intro_notice);

    // The Survival Thread now teaches the Campfire-to-Shelter-Tent upgrade from
    // authoritative objective state instead of firing a fixed-clock lesson.

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
        run_obj_ids.push(poi_id);
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
                    run_owner: Some(player_id),
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
        run_obj_ids.push(poi_id);
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
                    run_owner: Some(player_id),
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
        run_obj_ids.push(poi_id);
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
                    run_owner: Some(player_id),
                },
            };
            game_events.insert(spawn_event.event_id, spawn_event);
        }
    }

    // Abandoned Mine - contains ore plus mining and quarrying supplies.
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
            "Training Stonecutter Hammer".to_string(),
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
        run_obj_ids.push(poi_id);
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
                    run_owner: Some(player_id),
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

    run_spawned_objs.insert(player_id, run_obj_ids);

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

// Non-player-owned objects spawned for one player's run (shipwreck,
// intro NPCs, POIs, merchant...), keyed by player id. True Death removes them
// before the start location is recycled — without this the next hero at the
// same location spawns into the previous run's leftovers. In-memory only,
// like AssignedStartLocations.
#[derive(Debug, Default, Resource, Deref, DerefMut)]
pub struct RunSpawnedObjs(pub HashMap<i32, Vec<i32>>);

impl RunSpawnedObjs {
    /// Returns whether an object belongs to the player's current scripted run.
    /// Neutral POIs such as the starter Shipwreck use this association instead
    /// of ordinary PlayerId ownership.
    pub fn contains_for_player(&self, player_id: i32, obj_id: i32) -> bool {
        self.get(&player_id)
            .is_some_and(|object_ids| object_ids.contains(&obj_id))
    }
}

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
    use crate::game::sanctuary_radius;
    use crate::item::{Inventory, LOG};
    use crate::map::Map;
    use std::fs::File;

    #[test]
    fn introductory_necromancer_sites_start_outside_their_sanctuaries() {
        let start_location_file =
            File::open("templates/player_start.yaml").expect("Could not open start locations");
        let start_locations: Vec<StartLocation> =
            serde_yaml::from_reader(start_location_file).expect("Could not read start locations");
        let map = Map::load_map();
        let initial_sanctuary_radius = sanctuary_radius(0);

        for start_location in start_locations {
            let monolith_pos = Position {
                x: start_location.monolith_pos[0],
                y: start_location.monolith_pos[1],
            };
            let mausoleum_pos = Position {
                x: start_location.mausoleum_pos[0],
                y: start_location.mausoleum_pos[1],
            };
            let authored_necromancer_pos = Position {
                x: start_location.necromancer_pos[0],
                y: start_location.necromancer_pos[1],
            };

            assert_eq!(
                authored_necromancer_pos, mausoleum_pos,
                "{} dormant Necromancer and Mausoleum must share their reveal anchor",
                start_location.name,
            );

            assert!(
                Map::dist(monolith_pos, mausoleum_pos) >= initial_sanctuary_radius,
                "{} Mausoleum at {:?} must be outside the level-0 sanctuary centered at {:?}",
                start_location.name,
                mausoleum_pos,
                monolith_pos,
            );
            assert!(
                Map::is_passable(mausoleum_pos.x, mausoleum_pos.y, &map),
                "{} Mausoleum at {:?} must use passable terrain",
                start_location.name,
                mausoleum_pos,
            );
        }
    }

    #[test]
    fn authored_graveyards_start_outside_sanctuary_and_hero_vision() {
        let start_location_file =
            File::open("templates/player_start.yaml").expect("Could not open start locations");
        let start_locations: Vec<StartLocation> =
            serde_yaml::from_reader(start_location_file).expect("Could not read start locations");
        let obj_template_file =
            File::open("templates/obj_template.yaml").expect("Could not open object templates");
        let obj_templates: Vec<crate::templates::ObjTemplate> =
            serde_yaml::from_reader(obj_template_file).expect("Could not read object templates");
        let max_novice_hero_vision = obj_templates
            .iter()
            .filter(|template| {
                matches!(
                    template.template.as_str(),
                    "Novice Warrior" | "Novice Ranger" | "Novice Mage"
                )
            })
            .filter_map(|template| template.base_vision)
            .max()
            .expect("Novice heroes must define starting vision");
        let map = Map::load_map();
        let initial_sanctuary_radius = sanctuary_radius(0);

        for start_location in start_locations {
            let Some(graveyard_pos) = start_location.graveyard_pos.as_ref() else {
                continue;
            };
            let hero_pos = Position {
                x: start_location.hero_pos[0],
                y: start_location.hero_pos[1],
            };
            let monolith_pos = Position {
                x: start_location.monolith_pos[0],
                y: start_location.monolith_pos[1],
            };
            let graveyard_pos = Position {
                x: graveyard_pos[0],
                y: graveyard_pos[1],
            };

            assert!(
                Map::dist(monolith_pos, graveyard_pos) >= initial_sanctuary_radius,
                "{} Graveyard at {:?} must start outside its sanctuary centered at {:?}",
                start_location.name,
                graveyard_pos,
                monolith_pos,
            );
            assert!(
                Map::dist(hero_pos, graveyard_pos) > max_novice_hero_vision,
                "{} Graveyard at {:?} must remain hidden beyond every novice hero's starting vision from {:?}",
                start_location.name,
                graveyard_pos,
                hero_pos,
            );
            if start_location.name == "startpos3" {
                assert!(
                    Map::is_passable(graveyard_pos.x, graveyard_pos.y, &map),
                    "{} moved Graveyard at {:?} must use passable terrain",
                    start_location.name,
                    graveyard_pos,
                );
            }
        }
    }

    #[test]
    fn starter_hatchet_matches_stick_combat_profile_and_can_gather_logs() {
        let item_template_file =
            File::open("templates/item_template.yaml").expect("Could not open item templates");
        let item_templates: Vec<crate::templates::ItemTemplate> =
            serde_yaml::from_reader(item_template_file).expect("Could not read item templates");
        let mut inventory = Inventory {
            owner: 1,
            items: Vec::new(),
        };
        inventory.new(1, "Sharpened Stick".to_string(), 1, &item_templates);
        inventory.new(2, "Crude Hatchet".to_string(), 1, &item_templates);

        let stick = inventory
            .items
            .iter()
            .find(|item| item.name == "Sharpened Stick")
            .expect("starter stick");
        let hatchet = inventory
            .items
            .iter()
            .find(|item| item.name == "Crude Hatchet")
            .expect("starter hatchet");

        assert!(!stick.is_gather_tool_for_res_type(LOG));
        assert!(hatchet.is_gather_tool_for_res_type(LOG));
        assert_eq!(
            hatchet.attrs.get(&item::AttrKey::Damage),
            stick.attrs.get(&item::AttrKey::Damage)
        );
        assert_eq!(
            hatchet.attrs.get(&item::AttrKey::Speed),
            stick.attrs.get(&item::AttrKey::Speed)
        );
    }
}
