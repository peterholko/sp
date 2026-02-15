use super::*;
use crate::recipe::Recipe;
use crate::skill::WEAPONSMITHING;
use crate::templates::{ResReq, SkillTemplate, SkillTemplates, Templates};

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
fn stamina_recovery_increases_stamina_every_second() {
    let mut app = App::new();
    app.add_systems(Update, stamina_recovery_system);
    app.insert_resource(GameTick(TICKS_PER_SEC)); // tick aligned to 1 second

    let entity = app
        .world_mut()
        .spawn(Stats {
            hp: 100,
            stamina: Some(50),
            base_hp: 100,
            base_stamina: Some(100),
            base_def: 10,
            damage_range: None,
            base_damage: None,
            base_speed: None,
            base_vision: None,
        })
        .id();

    app.update();

    let stats = app.world().get::<Stats>(entity).unwrap();
    assert_eq!(stats.stamina, Some(51));
}

#[test]
fn stamina_recovery_does_not_exceed_base_stamina() {
    let mut app = App::new();
    app.add_systems(Update, stamina_recovery_system);
    app.insert_resource(GameTick(TICKS_PER_SEC));

    let entity = app
        .world_mut()
        .spawn(Stats {
            hp: 100,
            stamina: Some(100),
            base_hp: 100,
            base_stamina: Some(100),
            base_def: 10,
            damage_range: None,
            base_damage: None,
            base_speed: None,
            base_vision: None,
        })
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
        .spawn(Stats {
            hp: 100,
            stamina: Some(50),
            base_hp: 100,
            base_stamina: Some(100),
            base_def: 10,
            damage_range: None,
            base_damage: None,
            base_speed: None,
            base_vision: None,
        })
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
                base_hp: 100,
                base_stamina: Some(100),
                base_def: 10,
                damage_range: None,
                base_damage: None,
                base_speed: None,
                base_vision: None,
            },
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
