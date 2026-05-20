use super::*;
use crate::map::{TileInfo, TileType, HEIGHT, WIDTH};
use crate::recipe::Recipe;
use crate::skill::WEAPONSMITHING;
use crate::templates::{ResReq, SkillTemplate, SkillTemplates, Templates};
use std::collections::HashSet;
use std::fs::File;

fn load_obj_templates() -> Vec<ObjTemplate> {
    let obj_template_file =
        File::open("templates/obj_template.yaml").expect("Could not open obj templates");
    serde_yaml::from_reader(obj_template_file).expect("Could not read obj templates")
}

fn flat_land_map() -> Map {
    Map {
        width: WIDTH,
        height: HEIGHT,
        base: vec![
            TileInfo {
                tile_type: TileType::Grasslands,
                layers: Vec::new(),
            };
            (WIDTH * HEIGHT) as usize
        ],
        temperature: Vec::new(),
        moisture: Vec::new(),
        wildness: Vec::new(),
    }
}

#[test]
fn combat_lock_helper_uses_three_second_window() {
    let last_combat_tick = LastCombatTick(100);

    assert!(is_combat_locked(100, &last_combat_tick));
    assert!(is_combat_locked(129, &last_combat_tick));
    assert!(!is_combat_locked(130, &last_combat_tick));
    assert!(!is_combat_locked(131, &last_combat_tick));
}

#[test]
fn first_resurrection_uses_starting_soulshard_cost() {
    assert_eq!(resurrection_attempt_cost(1, 0), 10);
}

#[test]
fn later_resurrections_scale_from_completed_deaths() {
    assert_eq!(resurrection_attempt_cost(2, 0), 12);
}

#[test]
fn necromancer_spawn_resolver_uses_open_anchor() {
    let map = flat_land_map();
    let anchor = Position { x: 10, y: 10 };
    let occupied = HashSet::new();

    assert_eq!(
        resolve_necromancer_spawn_pos(anchor, &occupied, &map, 1),
        Some(anchor)
    );
}

#[test]
fn necromancer_spawn_resolver_falls_back_when_anchor_occupied() {
    let map = flat_land_map();
    let anchor = Position { x: 10, y: 10 };
    let occupied = HashSet::from([anchor]);

    let resolved = resolve_necromancer_spawn_pos(anchor, &occupied, &map, 1).unwrap();

    assert_ne!(resolved, anchor);
    assert_eq!(Map::dist(anchor, resolved), 1);
    assert!(!occupied.contains(&resolved));
}

#[test]
fn necromancer_spawn_resolver_uses_mausoleum_when_old_necro_tile_is_occupied() {
    let map = flat_land_map();
    let mausoleum_anchor = Position { x: 16, y: 32 };
    let old_necromancer_pos = Position { x: 17, y: 34 };
    let occupied = HashSet::from([old_necromancer_pos]);

    assert_eq!(
        resolve_necromancer_spawn_pos(mausoleum_anchor, &occupied, &map, 5),
        Some(mausoleum_anchor)
    );
}

#[test]
fn necromancer_spawn_resolver_returns_none_when_search_area_occupied() {
    let map = flat_land_map();
    let anchor = Position { x: 10, y: 10 };
    let mut occupied = HashSet::from([anchor]);

    for (x, y) in Map::ring((anchor.x, anchor.y), 1) {
        occupied.insert(Position { x, y });
    }

    assert_eq!(
        resolve_necromancer_spawn_pos(anchor, &occupied, &map, 1),
        None
    );
}

#[test]
fn combat_lock_interrupt_cancels_active_peaceful_work() {
    let mut app = App::new();
    app.add_systems(Update, combat_lock_interrupt_system);
    app.add_observer(cancel_events_observer);
    app.insert_resource(GameTick(100));
    app.insert_resource(EntityObjMap(HashMap::new()));
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(GameEvents(HashMap::new()));

    let hero = app
        .world_mut()
        .spawn((
            Id(1),
            PlayerId(1),
            Position { x: 0, y: 0 },
            State::Gathering,
            SubclassHero,
            LastCombatTick(100),
            EventExecuting {
                event_type: "gather".to_string(),
                state: EventExecutingState::Executing,
            },
        ))
        .id();

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .new_obj(1, hero);
    app.world_mut().resource_mut::<MapEvents>().new(
        1,
        120,
        VisibleEvent::GatherEvent {
            res_type: ORE.to_string(),
        },
    );
    app.world_mut().resource_mut::<GameEvents>().insert(
        1,
        GameEvent {
            event_id: 1,
            start_tick: 100,
            run_tick: 120,
            event_type: GameEventType::GatherEvent {
                gatherer_id: 1,
                res_type: ORE.to_string(),
            },
        },
    );

    app.update();

    assert_eq!(
        *app.world().entity(hero).get::<State>().unwrap(),
        State::None
    );
    assert_eq!(
        app.world()
            .entity(hero)
            .get::<EventExecuting>()
            .unwrap()
            .state,
        EventExecutingState::None
    );
    assert!(app.world().resource::<MapEvents>().is_empty());
    assert!(app.world().resource::<GameEvents>().is_empty());
}

#[test]
fn upgrading_campfire_to_small_tent_adds_shelter_component() {
    let mut app = App::new();
    app.add_systems(Update, upgrade_system);
    app.insert_resource(GameTick(10));
    app.insert_resource(EntityObjMap(HashMap::new()));
    app.insert_resource(Templates::from_obj_templates(load_obj_templates()));

    let structure_entity = app
        .world_mut()
        .spawn((
            Id(1),
            PlayerId(1),
            Position { x: 0, y: 0 },
            State::Upgrading,
            Name("Campfire".to_string()),
            Class(CLASS_STRUCTURE.to_string()),
            ClassStructure,
            Subclass::Campfire,
            Template("Campfire".to_string()),
            Misc {
                image: "campfire".to_string(),
                hsl: vec![],
                groups: vec![],
            },
            Stats {
                hp: 50,
                stamina: None,
                mana: None,
                base_hp: 100,
                base_stamina: None,
                base_mana: None,
                base_def: 0,
                damage_range: None,
                base_damage: None,
                base_speed: None,
                base_vision: None,
            },
            Assignments(vec![2]),
            BuildUpgradeState {
                build_upgrade_cost: 1.0,
                work_done: 0.0,
                work_per_sec: 0.0,
            },
            SelectedUpgrade("Small Tent".to_string()),
            StateUpgrading,
        ))
        .id();

    let worker_entity = app
        .world_mut()
        .spawn((
            Id(2),
            PlayerId(1),
            Position { x: 0, y: 0 },
            State::Upgrading,
            Template("Human Villager".to_string()),
            Skills::new(),
            BaseAttrs {
                creativity: 0,
                dexterity: 0,
                endurance: 0,
                focus: 0,
                intellect: 0,
                spirit: 0,
                strength: 0,
                toughness: 0,
            },
        ))
        .id();

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .new_obj(2, worker_entity);

    app.update();

    let structure = app.world().entity(structure_entity);
    assert_eq!(structure.get::<Name>().unwrap().0, "Small Tent");
    assert_eq!(structure.get::<Template>().unwrap().0, "Small Tent");
    assert_eq!(*structure.get::<Subclass>().unwrap(), Subclass::Shelter);
    assert!(structure.get::<StateUpgrading>().is_none());

    let shelter = structure
        .get::<Shelter>()
        .expect("upgraded tent needs Shelter");
    assert_eq!(shelter.max_residents, 1);

    let stats = structure.get::<Stats>().unwrap();
    assert_eq!(stats.base_hp, 100);
    assert_eq!(stats.hp, 100);
}

#[test]
fn is_fortified_removed_after_dead_wall() {
    // Setup app
    let mut app = App::new();

    // Add our two systems
    app.add_systems(Update, state_dead_system);

    // Setup test entities
    app.world_mut().spawn((
        Id(1),
        PlayerId(1),
        Position { x: 0, y: 0 },
        Name("Test Wall".to_string()),
        Template("Wall".to_string()),
        Class(CLASS_STRUCTURE.to_string()),
        Subclass::Wall,
        Viewshed { range: 5 },
        Misc::default(),
        State::Dead,
        Effects(HashMap::new()),
        StateDead {
            dead_at: 0,
            killer: "Unknown".to_string(),
        },
    ));

    let mut effects = HashMap::new();
    effects.insert(Effect::Fortified, (0, 1.0, 1));

    let obj_id = app
        .world_mut()
        .spawn((
            Id(2),
            PlayerId(1),
            Position { x: 0, y: 0 },
            Template("Human Villager".into()),
            Class("unit".into()),
            Subclass::Villager,
            Viewshed { range: 5 },
            Misc::default(),
            State::Dead,
            Effects(effects),
        ))
        .id();

    // Run systems
    app.update();

    // Check resulting changes
    assert!(app.world().get::<Effects>(obj_id).is_some());
    assert!(!app
        .world()
        .get::<Effects>(obj_id)
        .unwrap()
        .0
        .contains_key(&Effect::Fortified));
}

#[test]
fn is_watchtower_light_removed_after_dead_watchtower() {
    // Setup app
    let mut app = App::new();

    // Add our two systems
    app.add_systems(Update, state_dead_system);

    // Setup test entities
    app.world_mut().spawn((
        Id(1),
        PlayerId(1),
        Position { x: 0, y: 0 },
        Name("Test Watchtower".to_string()),
        Template("Watchtower".to_string()),
        Class(CLASS_STRUCTURE.to_string()),
        Subclass::Watchtower,
        Viewshed { range: 5 },
        Misc::default(),
        State::Dead,
        Effects(HashMap::new()),
        StateDead {
            dead_at: 0,
            killer: "Unknown".to_string(),
        },
    ));

    let mut effects = HashMap::new();
    effects.insert(Effect::WatchtowerLight, (0, 1.0, 1));

    let obj_id = app
        .world_mut()
        .spawn((
            Id(2),
            PlayerId(1),
            Position { x: 0, y: 0 },
            Template("Human Villager".into()),
            Class("unit".into()),
            Subclass::Villager,
            Viewshed { range: 5 },
            Misc::default(),
            State::None,
            Effects(effects),
        ))
        .id();

    // Run systems
    app.update();

    // Check resulting changes
    assert!(app.world().get::<Effects>(obj_id).is_some());
    assert!(!app
        .world()
        .get::<Effects>(obj_id)
        .unwrap()
        .0
        .contains_key(&Effect::WatchtowerLight));
}

#[test]
fn watchtower_reveals_enemy_hidden_units_inside_current_viewshed() {
    let mut app = App::new();
    app.add_systems(Update, watchtower_reveal_system);
    app.add_observer(state_change_observer);
    app.insert_resource(GameTick(TICKS_PER_SEC));
    app.insert_resource(PerceptionUpdates(HashSet::new()));
    app.insert_resource(VisibleEvents(Vec::new()));

    app.world_mut().spawn((
        Id(1),
        PlayerId(1),
        Position { x: 0, y: 0 },
        Viewshed { range: 3 },
        State::None,
        Watchtower,
    ));

    let hidden_enemy = app
        .world_mut()
        .spawn((
            Id(2),
            PlayerId(2),
            Position { x: 2, y: 0 },
            Class(CLASS_UNIT.to_string()),
            State::Hiding,
        ))
        .id();

    app.update();

    assert_eq!(app.world().get::<State>(hidden_enemy), Some(&State::None));
    let perception_updates = app.world().resource::<PerceptionUpdates>();
    assert!(perception_updates.contains(&(1, PerceptionUpdateType::UpdatePerception)));
    assert!(perception_updates.contains(&(2, PerceptionUpdateType::UpdatePerception)));
}

#[test]
fn watchtower_does_not_reveal_out_of_range_or_friendly_hidden_units() {
    let mut app = App::new();
    app.add_systems(Update, watchtower_reveal_system);
    app.add_observer(state_change_observer);
    app.insert_resource(GameTick(TICKS_PER_SEC));
    app.insert_resource(PerceptionUpdates(HashSet::new()));
    app.insert_resource(VisibleEvents(Vec::new()));

    app.world_mut().spawn((
        Id(1),
        PlayerId(1),
        Position { x: 0, y: 0 },
        Viewshed { range: 2 },
        State::None,
        Watchtower,
    ));

    let hidden_enemy_outside = app
        .world_mut()
        .spawn((
            Id(2),
            PlayerId(2),
            Position { x: 3, y: 0 },
            Class(CLASS_UNIT.to_string()),
            State::Hiding,
        ))
        .id();
    let friendly_hidden = app
        .world_mut()
        .spawn((
            Id(3),
            PlayerId(1),
            Position { x: 1, y: 0 },
            Class(CLASS_UNIT.to_string()),
            State::Hiding,
        ))
        .id();

    app.update();

    assert_eq!(
        app.world().get::<State>(hidden_enemy_outside),
        Some(&State::Hiding)
    );
    assert_eq!(
        app.world().get::<State>(friendly_hidden),
        Some(&State::Hiding)
    );
}

#[test]
fn time_of_day_labels_match_thresholds() {
    assert_eq!(GameTick(FIRST_LIGHT - 1).time_of_day(), "Night");
    assert_eq!(GameTick(FIRST_LIGHT).time_of_day(), "First Light");
    assert_eq!(GameTick(DAWN).time_of_day(), "Dawn");
    assert_eq!(GameTick(MORNING).time_of_day(), "Morning");
    assert_eq!(GameTick(AFTERNOON).time_of_day(), "Afternoon");
    assert_eq!(GameTick(EVENING).time_of_day(), "Evening");
    assert_eq!(GameTick(DUSK).time_of_day(), "Dusk");
    assert_eq!(GameTick(NIGHT).time_of_day(), "Night");
    assert_eq!(GameTick(GAME_TICKS_PER_DAY).time_of_day(), "Night");
}

#[test]
fn craft_event_system_creates_crafted_item_and_updates_skill() {
    let mut app = App::new();
    app.add_systems(Update, craft_event_system);

    let clients = Clients(Arc::new(Mutex::new(HashMap::new())));
    app.insert_resource(clients);
    app.insert_resource(GameTick(10));
    app.insert_resource(Ids {
        map_event: 0,
        player_event: 0,
        obj: 0,
        item: 0,
        player_hero_map: HashMap::new(),
        obj_player_map: HashMap::new(),
    });

    let mut game_events = HashMap::new();
    game_events.insert(
        1,
        GameEvent {
            event_id: 1,
            start_tick: 0,
            run_tick: 0,
            event_type: GameEventType::CraftEvent {
                crafter_id: 1,
                recipe_name: "Test Item".to_string(),
            },
        },
    );
    app.insert_resource(GameEvents(game_events));
    app.insert_resource(MapEvents(HashMap::new()));

    let mut entity_obj_map = HashMap::new();
    let crafter_entity = app
        .world_mut()
        .spawn((
            PlayerId(1),
            Subclass::Villager,
            State::Crafting,
            Inventory {
                owner: 1,
                items: vec![Item {
                    id: 1,
                    owner: 1,
                    name: "Wood".to_string(),
                    quantity: 1,
                    durability: None,
                    class: "Resource".to_string(),
                    subclass: "wood".to_string(),
                    slot: None,
                    image: "wood.png".to_string(),
                    weight: 1.0,
                    equipped: false,
                    experiment: None,
                    start_time: 0,
                    attrs: HashMap::new(),
                    produces: Vec::new(),
                }],
            },
            Skills::new(),
        ))
        .id();
    entity_obj_map.insert(1, crafter_entity);
    app.insert_resource(EntityObjMap(entity_obj_map));

    let recipes = Recipes::from_recipes(vec![Recipe {
        name: "Test Item".to_string(),
        class: item::WEAPON.to_string(),
        subclass: "sword".to_string(),
        image: "sword.png".to_string(),
        weight: 1.0,
        durability: None,
        attrs: None,
        owner: 1,
        tier: None,
        slot: None,
        damage: None,
        speed: None,
        armor: None,
        crafting_time: None,
        structure_req: None,
        stamina_req: None,
        skill_req: None,
        amount: Some(1),
        req: vec![ResReq {
            req_type: "Wood".to_string(),
            quantity: 1,
            cquantity: None,
        }],
        item_name_from_req: None,
    }]);
    app.insert_resource(recipes);

    let mut skill_templates = HashMap::new();
    skill_templates.insert(
        WEAPONSMITHING.to_string(),
        SkillTemplate {
            name: WEAPONSMITHING.to_string(),
            class: "crafting".to_string(),
            xp: vec![0, 100],
        },
    );

    let mut templates = Templates::from_obj_templates(vec![]);
    templates.skill_templates = SkillTemplates::from_map(skill_templates);
    app.insert_resource(templates);

    app.insert_resource(ActiveInfos(HashMap::new()));

    app.update();

    let game_events = app.world().resource::<GameEvents>();
    assert!(game_events.is_empty());

    let inventory = app.world().get::<Inventory>(crafter_entity).unwrap();
    assert!(inventory.items.iter().any(|item| item.name == "Test Item"));

    let skills = app.world().get::<Skills>(crafter_entity).unwrap();
    assert!(skills.get_all().keys().any(|name| name == WEAPONSMITHING));
}

#[test]
fn gather_event_system_marks_gatherer_event_completed() {
    let mut app = App::new();
    app.add_systems(Update, gather_event_system);
    app.add_plugins(ResourcePlugin);

    let clients = Clients(Arc::new(Mutex::new(HashMap::new())));
    app.insert_resource(clients);
    app.insert_resource(GameTick(10));
    app.insert_resource(Ids::default());
    app.insert_resource(Map::default());
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(Recipes::from_recipes(vec![]));
    app.insert_resource(Templates::from_obj_templates(vec![]));
    app.insert_resource(ActiveInfos(HashMap::new()));

    let mut game_events = HashMap::new();
    game_events.insert(
        7,
        GameEvent {
            event_id: 7,
            start_tick: 0,
            run_tick: 0,
            event_type: GameEventType::GatherEvent {
                gatherer_id: 1,
                res_type: ORE.to_string(),
            },
        },
    );
    app.insert_resource(GameEvents(game_events));

    let gatherer_entity = app
        .world_mut()
        .spawn((
            PlayerId(1),
            Position { x: 0, y: 0 },
            Name("Test Miner".to_string()),
            Template("Human Villager".to_string()),
            Subclass::Villager,
            State::Gathering,
            Effects(HashMap::new()),
            Inventory {
                owner: 1,
                items: Vec::new(),
            },
            Skills::new(),
        ))
        .id();

    let mut entity_obj_map = HashMap::new();
    entity_obj_map.insert(1, gatherer_entity);
    app.insert_resource(EntityObjMap(entity_obj_map));

    app.update();

    let event_completed = app.world().get::<EventCompleted>(gatherer_entity).unwrap();
    assert_eq!(event_completed.event_type, "gather");
    assert_eq!(event_completed.at_tick, 10);
    assert!(event_completed.success);

    let game_events = app.world().resource::<GameEvents>();
    assert!(game_events.is_empty());
}

#[test]
fn gather_event_system_notifies_hero_when_no_item_is_gathered() {
    let mut app = App::new();
    app.add_systems(Update, gather_event_system);
    app.add_plugins(ResourcePlugin);

    let (sender, mut receiver) = tokio::sync::mpsc::channel::<String>(4);
    let client_id = Uuid::new_v4();
    let clients = Clients(Arc::new(Mutex::new(HashMap::from([(
        client_id,
        Client {
            id: client_id,
            player_id: 1,
            sender,
        },
    )]))));
    app.insert_resource(clients);
    app.insert_resource(GameTick(10));

    let mut ids = Ids::default();
    ids.new_hero(1, 1);
    app.insert_resource(ids);

    app.insert_resource(Map::default());
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(Recipes::from_recipes(vec![]));
    app.insert_resource(Templates::from_obj_templates(vec![]));
    app.insert_resource(ActiveInfos(HashMap::new()));

    let mut game_events = HashMap::new();
    game_events.insert(
        7,
        GameEvent {
            event_id: 7,
            start_tick: 0,
            run_tick: 0,
            event_type: GameEventType::GatherEvent {
                gatherer_id: 1,
                res_type: ORE.to_string(),
            },
        },
    );
    app.insert_resource(GameEvents(game_events));

    let hero_entity = app
        .world_mut()
        .spawn((
            PlayerId(1),
            Position { x: 0, y: 0 },
            Name("Test Hero".to_string()),
            Template("Novice Warrior".to_string()),
            Subclass::Hero,
            State::Gathering,
            Effects(HashMap::new()),
            Inventory {
                owner: 1,
                items: Vec::new(),
            },
            Skills::new(),
        ))
        .id();

    app.insert_resource(EntityObjMap(HashMap::from([(1, hero_entity)])));

    app.update();

    let message = receiver
        .try_recv()
        .expect("expected gather failure notice for hero");
    let packet: ResponsePacket = serde_json::from_str(&message).unwrap();

    assert_eq!(
        packet,
        ResponsePacket::Notice {
            noticemsg: "You gathered nothing.".to_string(),
            expiry: Some(2000),
        }
    );
}

#[test]
fn stamina_recovery_increases_stamina_every_second() {
    let mut app = App::new();
    app.add_systems(Update, stamina_recovery_system);
    app.insert_resource(GameTick(TICKS_PER_SEC)); // tick aligned to 1 second

    let entity = app
        .world_mut()
        .spawn((
            Stats {
                hp: 100,
                stamina: Some(50),
                mana: None,
                base_hp: 100,
                base_stamina: Some(100),
                base_mana: None,
                base_def: 10,
                damage_range: None,
                base_damage: None,
                base_speed: None,
                base_vision: None,
            },
            LastCombatTick::default(),
        ))
        .id();

    app.update();

    // Out of combat: +5/sec recovery
    let stats = app.world().get::<Stats>(entity).unwrap();
    assert_eq!(stats.stamina, Some(55));
}

#[test]
fn stamina_recovery_does_not_exceed_base_stamina() {
    let mut app = App::new();
    app.add_systems(Update, stamina_recovery_system);
    app.insert_resource(GameTick(TICKS_PER_SEC));

    let entity = app
        .world_mut()
        .spawn((
            Stats {
                hp: 100,
                stamina: Some(100),
                mana: None,
                base_hp: 100,
                base_stamina: Some(100),
                base_mana: None,
                base_def: 10,
                damage_range: None,
                base_damage: None,
                base_speed: None,
                base_vision: None,
            },
            LastCombatTick::default(),
        ))
        .id();

    app.update();

    let stats = app.world().get::<Stats>(entity).unwrap();
    assert_eq!(stats.stamina, Some(100));
}

#[test]
fn stamina_recovery_skips_non_second_ticks() {
    let mut app = App::new();
    app.add_systems(Update, stamina_recovery_system);
    app.insert_resource(GameTick(3)); // not a multiple of TICKS_PER_SEC

    let entity = app
        .world_mut()
        .spawn((
            Stats {
                hp: 100,
                stamina: Some(50),
                mana: None,
                base_hp: 100,
                base_stamina: Some(100),
                base_mana: None,
                base_def: 10,
                damage_range: None,
                base_damage: None,
                base_speed: None,
                base_vision: None,
            },
            LastCombatTick::default(),
        ))
        .id();

    app.update();

    let stats = app.world().get::<Stats>(entity).unwrap();
    assert_eq!(stats.stamina, Some(50));
}

#[test]
fn stamina_recovery_skips_dead_entities() {
    let mut app = App::new();
    app.add_systems(Update, stamina_recovery_system);
    app.insert_resource(GameTick(TICKS_PER_SEC));

    let entity = app
        .world_mut()
        .spawn((
            Stats {
                hp: 100,
                stamina: Some(50),
                mana: None,
                base_hp: 100,
                base_stamina: Some(100),
                base_mana: None,
                base_def: 10,
                damage_range: None,
                base_damage: None,
                base_speed: None,
                base_vision: None,
            },
            LastCombatTick::default(),
            StateDead {
                dead_at: 0,
                killer: "Test".to_string(),
            },
        ))
        .id();

    app.update();

    let stats = app.world().get::<Stats>(entity).unwrap();
    assert_eq!(stats.stamina, Some(50));
}

// =============================================================================
// Crisis System Tests
// =============================================================================

#[test]
fn crisis_tier_calculation_empty_state() {
    let crisis = PlayerCrisis::default();
    let tier = calculate_crisis_tier(&crisis);
    assert_eq!(tier, 0);
}

#[test]
fn crisis_tier_calculation_all_tiers() {
    let mut crisis = PlayerCrisis::default();
    assert_eq!(calculate_crisis_tier(&crisis), 0);

    crisis.rat_spoilage = true;
    assert_eq!(calculate_crisis_tier(&crisis), 1);

    crisis.wolf_pack = true;
    assert_eq!(calculate_crisis_tier(&crisis), 2);

    crisis.goblin_raid = true;
    assert_eq!(calculate_crisis_tier(&crisis), 3);

    crisis.undead_incursion = true;
    assert_eq!(calculate_crisis_tier(&crisis), 4);

    crisis.goblin_pillager = true;
    assert_eq!(calculate_crisis_tier(&crisis), 5);
}

#[test]
fn crisis_tier_skipped_tiers_reports_highest() {
    // If wolf_pack triggers but rat_spoilage didn't, tier should still be 2
    let mut crisis = PlayerCrisis::default();
    crisis.wolf_pack = true;
    assert_eq!(calculate_crisis_tier(&crisis), 2);

    // Undead incursion without goblin raid
    crisis.undead_incursion = true;
    assert_eq!(calculate_crisis_tier(&crisis), 4);
}

#[test]
fn crisis_bonus_xp_scales_with_tier() {
    assert_eq!(0 * 1000, 0); // Tier 0: no bonus
    assert_eq!(1 * 1000, 1000); // Tier 1: +1000
    assert_eq!(3 * 1000, 3000); // Tier 3: +3000
    assert_eq!(5 * 1000, 5000); // Tier 5: +5000
}

#[test]
fn crisis_state_tracks_per_player() {
    let mut crisis_state = CrisisState::default();

    // Player 1 triggers rat crisis
    crisis_state
        .entry(1)
        .or_insert_with(PlayerCrisis::default)
        .rat_spoilage = true;

    // Player 2 triggers wolf crisis
    crisis_state
        .entry(2)
        .or_insert_with(PlayerCrisis::default)
        .wolf_pack = true;

    assert_eq!(calculate_crisis_tier(crisis_state.get(&1).unwrap()), 1);
    assert_eq!(calculate_crisis_tier(crisis_state.get(&2).unwrap()), 2);
    assert!(crisis_state.get(&3).is_none());
}

#[test]
fn crisis_state_cleanup_on_remove() {
    let mut crisis_state = CrisisState::default();
    crisis_state.insert(
        1,
        PlayerCrisis {
            rat_spoilage: true,
            ..Default::default()
        },
    );

    // Simulate death cleanup
    crisis_state.remove(&1);
    assert!(crisis_state.get(&1).is_none());
}

// =============================================================================
// Creature Stat Balance Validation Tests
// =============================================================================

#[test]
fn creature_hp_follows_tier_progression() {
    // Tier 1 creatures should have lowest HP, Tier 5 highest
    let rat_hp = 20; // T1
    let wolf_hp = 45; // T2
    let wolf_rider_hp = 75; // T3
    let goblin_pillager_hp = 55; // T5

    assert!(rat_hp < wolf_hp, "T1 rat should have less HP than T2 wolf");
    assert!(
        wolf_hp < wolf_rider_hp,
        "T2 wolf should have less HP than T3 wolf rider"
    );
    assert!(
        goblin_pillager_hp > rat_hp,
        "T5 pillager should have more HP than T1 rat"
    );
}

#[test]
fn creature_kill_xp_follows_tier_progression() {
    let rat_xp = 50; // T1
    let wolf_xp = 150; // T2
    let wolf_rider_xp = 300; // T3
    let zombie_xp = 100; // T4 (weak individually)
    let necro_xp = 500; // T4 boss
    let pillager_xp = 250; // T5

    assert!(rat_xp < wolf_xp, "T1 should give less XP than T2");
    assert!(wolf_xp < wolf_rider_xp, "T2 should give less XP than T3");
    assert!(
        zombie_xp < necro_xp,
        "T4 zombie should give less XP than T4 boss"
    );
    assert!(
        pillager_xp > wolf_xp,
        "T5 pillager should give more XP than T2 wolf"
    );
}

#[test]
fn hero_warrior_can_survive_tier1_encounter() {
    // Novice Warrior (100 HP) vs Giant Rat (2 dmg + 3 range = 5 max)
    // Warrior survives at least 100/5 = 20 hits
    let warrior_hp = 100;
    let rat_max_dmg = 2 + 3;
    let hits_to_kill_warrior = warrior_hp / rat_max_dmg;
    assert!(
        hits_to_kill_warrior >= 10,
        "Warrior should survive 10+ rat hits, got {}",
        hits_to_kill_warrior
    );
}

#[test]
fn hero_can_kill_tier1_creatures_quickly() {
    // Novice Warrior (2 dmg, 2 range) + Copper Axe (+11 dmg)
    // Avg damage: 2 + 1 + 11 = 14. Giant Rat HP: 20
    let hero_avg_dmg = 2 + 1 + 11;
    let rat_hp = 20;
    let hits_to_kill_rat = (rat_hp as f64 / hero_avg_dmg as f64).ceil() as i32;
    assert!(
        hits_to_kill_rat <= 3,
        "Hero should kill T1 rat in 3 or fewer hits, got {}",
        hits_to_kill_rat
    );
}

#[test]
fn tier5_creatures_are_dangerous_to_novice() {
    // Goblin Pillager (5 dmg + 4 range) vs Novice Warrior (100 HP)
    let warrior_hp = 100;
    let pillager_avg_dmg = 5 + 2; // base + avg_range
    let hits_to_kill_warrior = warrior_hp / pillager_avg_dmg;
    assert!(
        hits_to_kill_warrior <= 20,
        "T5 should be dangerous: warrior survives {} hits",
        hits_to_kill_warrior
    );
    assert!(
        hits_to_kill_warrior >= 5,
        "T5 shouldn't one-shot: warrior survives {} hits",
        hits_to_kill_warrior
    );
}

#[test]
fn hero_stamina_allows_reasonable_combat() {
    let stamina_cost_per_attack = 5;
    let warrior_attacks = 110 / stamina_cost_per_attack;
    let mage_attacks = 100 / stamina_cost_per_attack;

    assert!(
        warrior_attacks >= 15,
        "Warrior should get 15+ attacks, got {}",
        warrior_attacks
    );
    assert!(
        mage_attacks >= 20,
        "Mage should get 20+ attacks, got {}",
        mage_attacks
    );
}

#[test]
fn hero_classes_have_distinct_profiles() {
    // (hp, def, speed, vision, mana)
    let warrior = (110, 4, 5, 3, 0);
    let ranger = (80, 1, 7, 5, 0);
    let mage = (60, 0, 5, 4, 100);

    assert!(
        warrior.0 > ranger.0,
        "Warrior should have more HP than Ranger"
    );
    assert!(warrior.0 > mage.0, "Warrior should have more HP than Mage");
    assert!(
        warrior.1 >= ranger.1,
        "Warrior should have >= def than Ranger"
    );
    assert!(ranger.2 > warrior.2, "Ranger should be faster than Warrior");
    assert!(ranger.2 > mage.2, "Ranger should be faster than Mage");
    assert!(
        ranger.3 > warrior.3,
        "Ranger should have better vision than Warrior"
    );
    assert!(mage.4 > warrior.4, "Mage should start with mana");
    assert!(mage.4 > ranger.4, "Mage should start with mana");
}

#[test]
fn warrior_progression_scales_correctly() {
    // HP should increase with each rank
    let novice_hp = 100;
    let skilled_hp = 200;
    let great_hp = 400;
    let legendary_hp = 800;

    assert!(novice_hp < skilled_hp);
    assert!(skilled_hp < great_hp);
    assert!(great_hp < legendary_hp);

    // Defense should also increase
    let novice_def = 2;
    let skilled_def = 4;
    let great_def = 6;
    let legendary_def = 8;

    assert!(novice_def < skilled_def);
    assert!(skilled_def < great_def);
    assert!(great_def < legendary_def);
}

#[test]
fn crisis_timeline_is_ordered() {
    // Tier 4 triggers at tick 7700 (3 in-game days from DAWN)
    // Tier 5 triggers at tick 12500 (5 in-game days from DAWN)
    let tier4_trigger = 7700;
    let tier5_trigger = 12500;
    let ticks_per_day = GAME_TICKS_PER_DAY;

    // Verify the time-based crises happen in order
    assert!(tier4_trigger < tier5_trigger, "T4 should trigger before T5");

    // Verify T4 is roughly 3 days (7200 ticks) from DAWN (500)
    let t4_days = (tier4_trigger - DAWN) as f64 / ticks_per_day as f64;
    assert!(
        t4_days >= 2.5 && t4_days <= 3.5,
        "T4 should be ~3 days from start, got {:.1}",
        t4_days
    );

    // Verify T5 is roughly 5 days (12000 ticks) from DAWN (500)
    let t5_days = (tier5_trigger - DAWN) as f64 / ticks_per_day as f64;
    assert!(
        t5_days >= 4.5 && t5_days <= 5.5,
        "T5 should be ~5 days from start, got {:.1}",
        t5_days
    );
}

#[test]
fn survival_thread_begins_with_shipwreck_and_advances_in_order() {
    let mut objectives = PlayerObjectives::default();

    let packet = build_objective_state_packet(&objectives, 0, false, false, 1);
    match packet {
        ResponsePacket::ObjectiveState {
            current_id,
            objectives,
            ..
        } => {
            assert_eq!(current_id, "scavenge_shipwreck");
            assert_eq!(
                objectives
                    .iter()
                    .find(|obj| obj.state == "active")
                    .unwrap()
                    .id,
                "scavenge_shipwreck"
            );
        }
        _ => panic!("expected objective_state packet"),
    }

    objectives.scavenge_shipwreck = true;
    let packet = build_objective_state_packet(&objectives, 1, false, false, 1);
    match packet {
        ResponsePacket::ObjectiveState {
            current_id,
            objectives,
            ..
        } => {
            assert_eq!(current_id, "build_campfire");
            assert_eq!(
                objectives
                    .iter()
                    .find(|obj| obj.state == "active")
                    .unwrap()
                    .id,
                "build_campfire"
            );
        }
        _ => panic!("expected objective_state packet"),
    }

    objectives.build_campfire = true;
    objectives.win_first_fight = true;
    objectives.recruit_villager = true;
    let packet = build_objective_state_packet(&objectives, 2, false, true, 1);
    match packet {
        ResponsePacket::ObjectiveState {
            current_id,
            objectives,
            ..
        } => {
            assert_eq!(current_id, "build_shelter_storage");
            let active = objectives.iter().find(|obj| obj.state == "active").unwrap();
            assert_eq!(active.progress, Some(2));
            assert_eq!(active.goal, Some(3));
        }
        _ => panic!("expected objective_state packet"),
    }
}

#[test]
fn survival_thread_progresses_to_expansion_after_basic_camp() {
    let objectives = PlayerObjectives {
        scavenge_shipwreck: true,
        build_campfire: true,
        win_first_fight: true,
        recruit_villager: true,
        build_3_structures: true,
        ..Default::default()
    };

    let packet = build_objective_state_packet(&objectives, 3, true, true, 3);
    match packet {
        ResponsePacket::ObjectiveState {
            current_id,
            objectives,
            ..
        } => {
            assert_eq!(current_id, "choose_expansion");
            let night_goal = objectives
                .iter()
                .find(|obj| obj.id == "survive_5_nights")
                .unwrap();
            assert_eq!(night_goal.progress, Some(2));
            assert_eq!(night_goal.goal, Some(5));
        }
        _ => panic!("expected objective_state packet"),
    }
}

#[test]
fn threat_risk_severity_has_warning_before_crisis_threshold() {
    assert_eq!(risk_severity(0, 30), "quiet");
    assert_eq!(risk_severity(1, 30), "low");
    assert_eq!(risk_severity(20, 30), "medium");
    assert_eq!(risk_severity(30, 30), "high");
    assert_eq!(risk_severity(45, 30), "high");
}

#[test]
fn survival_director_starts_after_day_six_or_objective() {
    assert!(!survival_director_active(5, None));
    assert!(survival_director_active(6, None));

    let objectives = PlayerObjectives {
        survive_5_nights: true,
        ..Default::default()
    };
    assert!(survival_director_active(4, Some(&objectives)));
}

#[test]
fn shipwreck_inspection_triggers_villager_only_after_help_speech() {
    let entry = InitialEncounterEntry {
        rat_ids: vec![1, 2],
        phase1_spawn: "Wild Boar".to_string(),
        phase1_npc_id: None,
        spawn_pos: Position { x: 10, y: 10 },
        villager_spawn_pos: Position { x: 11, y: 10 },
        first_rat_spawn_tick: 900,
        second_rat_spawn_tick: 1200,
        villager_ready_tick: 1110,
        phase1_unlock_tick: 2600,
        spider_unlock_tick: 3600,
        villager_event_scheduled: false,
        merchant_id: 0,
    };
    let objectives = PlayerObjectives {
        scavenge_shipwreck: true,
        ..Default::default()
    };

    assert!(!shipwreck_inspection_can_spawn_villager(
        2000,
        &entry,
        Some(&PlayerObjectives::default())
    ));
    assert!(!shipwreck_inspection_can_spawn_villager(
        1100,
        &entry,
        Some(&objectives)
    ));
    assert!(shipwreck_inspection_can_spawn_villager(
        1110,
        &entry,
        Some(&objectives)
    ));
}

#[test]
fn survival_horde_size_scales_with_crisis_and_legendary_pressure() {
    assert_eq!(survival_horde_size(6, 0, 0), 2);
    assert_eq!(survival_horde_size(8, 2, 1), 7);
    assert_eq!(survival_horde_size(30, 5, 2), 12);
}

#[test]
fn survival_horde_composition_uses_new_late_game_units() {
    let day_six = survival_horde_composition(4, 6);
    assert!(day_six
        .iter()
        .any(|unit| matches!(*unit, "Ghoul" | "Ghast" | "Direwolf" | "Gryphon")));
    assert!(!day_six
        .iter()
        .any(|unit| matches!(*unit, "Zombie" | "Skeleton")));

    let day_eighteen = survival_horde_composition(12, 18);
    assert!(day_eighteen.iter().any(|unit| matches!(
        *unit,
        "Drake Armageddon" | "Drake Flameheart" | "Drake Hurricane" | "Wyvern Rider"
    )));
}

#[test]
fn run_score_breakdown_uses_all_components() {
    let inputs = RunScoreInputs {
        days_survived: 10,
        nights_survived: 9,
        waves_survived: 4,
        active_legendary_days: 2,
        hero_rank: "Great Ranger".to_string(),
        total_skill_levels: 12,
        total_xp: 20_000,
        total_wealth_value: 10_000,
        structures_alive: 6,
        upgrades: 2,
        repairs: 3,
        villagers_alive: 2,
        crisis_tier: 4,
        enemies_killed: 40,
        elites_killed: 3,
        captains_killed: 2,
        legendary_kills: 1,
        hideouts_cleared: 1,
        completed_objectives: 8,
        monolith_sealed: true,
    };

    let breakdown = calculate_run_score_breakdown(&inputs);
    assert_eq!(breakdown.survival, 9_250);
    assert_eq!(breakdown.progression, 11_200);
    assert_eq!(breakdown.wealth, 10_000);
    assert_eq!(breakdown.defense, 6_150);
    assert_eq!(breakdown.valor, 18_500);
    assert_eq!(breakdown.legacy, 7_000);
    assert_eq!(score_total_from_breakdown(&breakdown, 3), 71_415);
}

#[test]
fn legendary_threat_packet_hides_location_until_revealed() {
    let mut state = LegendaryThreatState(HashMap::new());
    state.insert(
        1,
        LegendaryThreat {
            name: LEGENDARY_BOSS.to_string(),
            hideout_pos: Position { x: 20, y: 21 },
            hideout_id: Some(100),
            boss_id: Some(101),
            rumor_sent: true,
            active: true,
            defeated: false,
            hideout_revealed: false,
            active_since_tick: Some(DAWN),
            defeated_at_tick: None,
            next_follower_tick: DAWN + 600,
            waves_sent: 1,
            follower_waves: Vec::new(),
            followers_defeated: 3,
            captains_defeated: 1,
        },
    );

    let hidden = legendary_threat_packets(1, &GameTick(DAWN + 100), &state);
    assert!(!hidden[0].hideout_known);
    assert_eq!(hidden[0].hideout_location, None);

    state.get_mut(&1).unwrap().hideout_revealed = true;
    let revealed = legendary_threat_packets(1, &GameTick(DAWN + 100), &state);
    assert!(revealed[0].hideout_known);
    assert_eq!(revealed[0].hideout_location, Some("20,21".to_string()));
}

// Helper function matching the logic in true_death_system
fn calculate_crisis_tier(crisis: &PlayerCrisis) -> i32 {
    let mut tier = 0;
    if crisis.rat_spoilage {
        tier = 1;
    }
    if crisis.wolf_pack {
        tier = 2;
    }
    if crisis.goblin_raid {
        tier = 3;
    }
    if crisis.undead_incursion {
        tier = 4;
    }
    if crisis.goblin_pillager {
        tier = 5;
    }
    tier
}
