use super::*;
use crate::common::TaskTarget;
use crate::effect::{EffectAttr, EffectVal};
use crate::encounter::Encounter;
use crate::map::{TileInfo, TileType, HEIGHT, WIDTH};
use crate::npc::{ScriptedCorpseHunt, VisibleTarget};
use crate::recipe::Recipe;
use crate::skill::WEAPONSMITHING;
use crate::templates::{EffectTemplate, ResReq, SkillTemplate, SkillTemplates, Templates};
use std::collections::{HashMap, HashSet};
use std::fs::File;

fn load_obj_templates() -> Vec<ObjTemplate> {
    let obj_template_file =
        File::open("templates/obj_template.yaml").expect("Could not open obj templates");
    serde_yaml::from_reader(obj_template_file).expect("Could not read obj templates")
}

#[test]
fn early_game_enemy_templates_are_loaded() {
    let obj_templates = load_obj_templates();
    let expected = [
        ("Cave Bat", "cavebat", 12, 35),
        ("Bog Leech", "bogleech", 18, 35),
        ("Thorn Beetle", "thornbeetle", 24, 55),
        ("Ash Viper", "ashviper", 14, 40),
        ("Moss Mite", "mossmite", 10, 30),
        ("Reef Skitter", "reefskitter", 16, 40),
    ];

    for (template_name, image, base_hp, kill_xp) in expected {
        let template = obj_templates
            .iter()
            .find(|template| template.template == template_name)
            .expect("missing early game enemy template");

        assert_eq!(template.class, "unit");
        assert_eq!(template.subclass, "npc");
        assert_eq!(template.image, image);
        assert_eq!(template.base_hp, Some(base_hp));
        assert_eq!(template.kill_xp, Some(kill_xp));
    }
}

#[test]
fn early_game_enemy_random_spawn_pool_excludes_bog_leech() {
    assert!(!EARLY_GAME_ENEMY_TEMPLATES.contains(&"Bog Leech"));
}

#[test]
fn wildlife_templates_are_loaded() {
    let obj_templates = load_obj_templates();
    let expected = [
        ("Swiftstep Hare", "hare", 8, 10, "passive"),
        ("Windstride Stag", "stag", 26, 35, "passive"),
        ("Frostmane Elk", "elk", 38, 60, "passive"),
        ("Mountain Lion", "mountainlion", 40, 160, "strategic"),
        ("Black Bear", "blackbear", 90, 240, "strategic"),
        ("Saberfang Cat", "saberfangcat", 65, 300, "strategic"),
        ("Cave Bear", "cavebear", 130, 450, "strategic"),
        ("Terror Bird", "terrorbird", 75, 320, "frenzied"),
    ];

    for (template_name, image, base_hp, kill_xp, aggression) in expected {
        let template = obj_templates
            .iter()
            .find(|template| template.template == template_name)
            .expect("missing wildlife template");

        assert_eq!(template.class, "unit");
        assert_eq!(template.subclass, "npc");
        assert_eq!(template.image, image);
        assert_eq!(template.family.as_deref(), Some("Animal"));
        assert_eq!(template.aggression.as_deref(), Some(aggression));
        assert_eq!(template.base_hp, Some(base_hp));
        assert_eq!(template.kill_xp, Some(kill_xp));
    }
}

#[test]
fn wildlife_encounters_are_available_by_terrain() {
    assert!(Encounter::npc_list(TileType::Grasslands).contains(&"Swiftstep Hare"));
    assert!(Encounter::npc_list(TileType::DeciduousForest).contains(&"Windstride Stag"));
    assert!(Encounter::npc_list(TileType::FrozenForest).contains(&"Frostmane Elk"));
    assert!(Encounter::npc_list(TileType::HillsGrasslands).contains(&"Mountain Lion"));
    assert!(Encounter::npc_list(TileType::DeciduousForest).contains(&"Black Bear"));
    assert!(Encounter::npc_list(TileType::Grasslands).contains(&"Terror Bird"));
    assert!(Encounter::npc_list(TileType::Snow).contains(&"Saberfang Cat"));
    assert!(Encounter::npc_list(TileType::FrozenForest).contains(&"Cave Bear"));
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

fn set_test_tile_type(map: &mut Map, x: i32, y: i32, tile_type: TileType) {
    let tile_index = (y * WIDTH + x) as usize;
    map.base[tile_index].tile_type = tile_type;
}

fn test_encounter_map_obj(
    player_id: i32,
    pos: Position,
    class: &str,
    subclass: &str,
) -> EncounterMapObj {
    EncounterMapObj {
        player_id,
        x: pos.x,
        y: pos.y,
        name: subclass.to_string(),
        class: class.to_string(),
        subclass: subclass.to_string(),
        template: subclass.to_string(),
    }
}

#[test]
fn survey_status_tracks_first_tile_survey_per_player() {
    let pos = Position { x: 3, y: 4 };
    let mut survey_history = SurveyHistory(HashMap::new());

    assert_eq!(
        survey_status_for_tile(1, pos, &survey_history),
        SURVEY_STATUS_UNSURVEYED
    );

    assert!(record_tile_survey(1, pos, &mut survey_history));

    assert_eq!(
        survey_status_for_tile(1, pos, &survey_history),
        SURVEY_STATUS_SURVEYED
    );
    assert_eq!(
        survey_status_for_tile(2, pos, &survey_history),
        SURVEY_STATUS_UNSURVEYED
    );
}

#[test]
fn survey_history_only_allows_one_outcome_roll() {
    let pos = Position { x: 5, y: 6 };

    let mut survey_history = SurveyHistory(HashMap::new());

    assert!(record_tile_survey(7, pos, &mut survey_history));
    assert!(!record_tile_survey(7, pos, &mut survey_history));
}

#[test]
fn poi_investigation_history_is_per_player_and_once() {
    let mut investigated_pois = InvestigatedPOIs(HashMap::new());

    assert!(record_poi_investigation(1, 10, &mut investigated_pois));
    assert!(!record_poi_investigation(1, 10, &mut investigated_pois));
    assert!(record_poi_investigation(2, 10, &mut investigated_pois));
    assert!(record_poi_investigation(1, 11, &mut investigated_pois));
}

#[test]
fn explore_outcome_table_uses_three_to_one_good_bad_ratio() {
    let positive = (0..12)
        .filter(|slot| explore_outcome_is_positive(explore_outcome_from_slot(*slot)))
        .count();

    assert_eq!(positive, 9);
    assert_eq!(12 - positive, 3);
}

#[test]
fn washed_ashore_loot_poi_only_uses_ocean_adjacent_land() {
    let center = Position { x: 10, y: 10 };
    let landlocked_map = flat_land_map();

    assert!(loot_poi_spawn_pos("Washed Ashore Materials", center, &landlocked_map).is_none());
    assert!(loot_poi_spawn_pos("Supply Cache", center, &landlocked_map).is_some());

    let mut coastal_map = flat_land_map();
    let ocean_tile = Map::range((center.x, center.y), 1)
        .into_iter()
        .find(|(x, y)| *x != center.x || *y != center.y)
        .expect("nearby ocean tile");
    set_test_tile_type(
        &mut coastal_map,
        ocean_tile.0,
        ocean_tile.1,
        TileType::Ocean,
    );

    for _ in 0..20 {
        let spawn_pos = loot_poi_spawn_pos("Washed Ashore Materials", center, &coastal_map)
            .expect("coastal washed ashore spawn");

        assert!(Map::is_passable(spawn_pos.x, spawn_pos.y, &coastal_map));
        assert!(Map::are_tile_types_nearby(
            spawn_pos,
            vec![TileType::Ocean],
            &coastal_map
        ));
    }
}

#[test]
fn map_lookups_handle_out_of_bounds_coords() {
    // Regression: rings around an edge-adjacent center (e.g. goblin_raid_system
    // spawning near the map border) produce off-map coordinates. The map helpers
    // must treat these as off-map instead of indexing out of bounds and panicking
    // with a usize underflow (y * WIDTH + x going negative).
    let map = flat_land_map();

    for (x, y) in [
        (-1, 0),
        (0, -1),
        (-43, 0),
        (WIDTH, 0),
        (0, HEIGHT),
        (WIDTH, HEIGHT),
    ] {
        assert!(!Map::is_passable(x, y, &map));
        assert!(!Map::is_passable_by_obj(x, y, true, false, false, &map));
        assert_eq!(Map::tile_type(x, y, &map), TileType::Ocean);
    }

    // are_tile_types_nearby walks the ring around the corner tile, which includes
    // off-map neighbours; it must not panic.
    let corner = Position { x: 0, y: 0 };
    assert!(Map::are_tile_types_nearby(
        corner,
        vec![TileType::Grasslands],
        &map
    ));
}

#[test]
fn info_tile_packet_serializes_survey_status() {
    let packet = ResponsePacket::InfoTile {
        x: 1,
        y: 2,
        name: "Grasslands".to_string(),
        mc: 1,
        def: 0.0,
        unrevealed: 0,
        sanctuary: "None".to_string(),
        passable: true,
        wildness: "Safe".to_string(),
        survey_status: SURVEY_STATUS_UNSURVEYED.to_string(),
        resources: Vec::new(),
        terrain_features: Vec::new(),
    };

    let json = serde_json::to_value(packet).expect("info_tile packet serializes");

    assert_eq!(json["packet"], "info_tile");
    assert_eq!(json["survey_status"], SURVEY_STATUS_UNSURVEYED);
}

#[test]
fn existing_cure_items_map_to_explore_negative_effects() {
    assert_eq!(
        explore_cure_for_item("Crude Bandage", item::MEDICAL, "Bandage"),
        Some(Effect::Bleed)
    );
    assert_eq!(
        explore_cure_for_item("Herbal Poultice", item::POTION, item::HEALTH),
        Some(Effect::Sickness)
    );
    assert_eq!(
        explore_cure_for_item("Health Potion", item::POTION, item::HEALTH),
        Some(Effect::Sickness)
    );
    assert_eq!(
        explore_cure_for_item(CRUDE_TORCH, item::TORCH, CRUDE_TORCH),
        Some(Effect::Cursed)
    );
    assert_eq!(
        explore_cure_for_item(RESIN_TORCH, item::TORCH, RESIN_TORCH),
        Some(Effect::Cursed)
    );
    assert_eq!(
        explore_cure_for_item(LANTERN_TORCH, item::TORCH, LANTERN_TORCH),
        None
    );
}

#[test]
fn remove_explore_negative_effect_only_clears_present_effect() {
    let mut effects = Effects(HashMap::new());
    effects.0.insert(Effect::Sickness, (100, 1.0, 1));

    assert!(!remove_explore_negative_effect(
        &mut effects,
        Effect::Cursed
    ));
    assert!(effects.has(Effect::Sickness));
    assert!(remove_explore_negative_effect(
        &mut effects,
        Effect::Sickness
    ));
    assert!(!effects.has(Effect::Sickness));
    assert!(!remove_explore_negative_effect(
        &mut effects,
        Effect::Sickness
    ));
}

#[test]
fn successful_healing_consumables_are_removed_exactly_once() {
    let potion = consumable_item(7, 42, "Health Potion", item::POTION, AttrKey::Healing, 10.0);
    let mut inventory = Inventory {
        owner: 42,
        items: vec![potion],
    };

    assert!(!consume_successful_healing_item(&mut inventory, 7, false));
    assert_eq!(inventory.get_by_id(7).map(|item| item.quantity), Some(1));

    assert!(consume_successful_healing_item(&mut inventory, 7, true));
    assert!(inventory.get_by_id(7).is_none());

    // A duplicate completion cannot consume or recreate an already-used item.
    assert!(!consume_successful_healing_item(&mut inventory, 7, true));
    assert!(inventory.items.is_empty());
}

#[test]
fn negative_explore_effects_include_panel_display_attrs() {
    let mut templates = Templates::from_obj_templates(vec![]);
    templates.effect_templates.load(vec![
        EffectTemplate {
            name: Effect::Cursed.to_str(),
            duration: 300,
            max_hp: None,
            healing: None,
            damage: Some(-0.15),
            damage_over_time: None,
            speed: None,
            attack_speed: None,
            defense: Some(-0.10),
            stackable: None,
            armor: None,
            lifeleech: None,
            viewshed: None,
            ignore_all_armor: None,
            instant_kill_chance: None,
            next_attack: None,
            vision: None,
            health: None,
            stamina: None,
        },
        EffectTemplate {
            name: Effect::Sickness.to_str(),
            duration: 300,
            max_hp: None,
            healing: None,
            damage: None,
            damage_over_time: None,
            speed: Some(-0.25),
            attack_speed: Some(-0.10),
            defense: None,
            stackable: None,
            armor: None,
            lifeleech: None,
            viewshed: None,
            ignore_all_armor: None,
            instant_kill_chance: None,
            next_attack: None,
            vision: None,
            health: None,
            stamina: None,
        },
    ]);

    let mut effects = Effects(HashMap::new());
    effects.0.insert(Effect::Cursed, (3000, 1.0, 1));
    effects.0.insert(Effect::Sickness, (3000, 1.0, 1));

    let effect_info = effects.get_info_list(&templates.effect_templates);
    let cursed_info = effect_info
        .iter()
        .find(|info| info.effect == Effect::Cursed)
        .expect("cursed effect info");
    let sickness_info = effect_info
        .iter()
        .find(|info| info.effect == Effect::Sickness)
        .expect("sickness effect info");

    assert_eq!(
        cursed_info.attrs.get(&EffectAttr::Damage),
        Some(&EffectVal::Num(-0.15))
    );
    assert_eq!(
        cursed_info.attrs.get(&EffectAttr::Defense),
        Some(&EffectVal::Num(-0.10))
    );
    assert_eq!(
        cursed_info.attrs.get(&EffectAttr::Duration),
        Some(&EffectVal::Num(300.0))
    );
    assert_eq!(
        sickness_info.attrs.get(&EffectAttr::Speed),
        Some(&EffectVal::Num(-0.25))
    );
    assert_eq!(
        sickness_info.attrs.get(&EffectAttr::AttackSpeed),
        Some(&EffectVal::Num(-0.10))
    );
}

fn consumable_item(
    id: i32,
    owner: i32,
    name: &str,
    class: &str,
    attr_key: AttrKey,
    attr_value: f32,
) -> Item {
    let mut attrs = HashMap::new();
    attrs.insert(attr_key, item::AttrVal::Num(attr_value));

    Item {
        id,
        owner,
        name: name.to_string(),
        quantity: 1,
        durability: None,
        class: class.to_string(),
        subclass: class.to_string(),
        slot: None,
        image: name.to_lowercase().replace(' ', ""),
        weight: 1.0,
        equipped: false,
        experiment: None,
        start_time: 0,
        attrs,
        produces: Vec::new(),
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

fn setup_new_obj_observer_test_app() -> App {
    let mut app = App::new();
    app.add_observer(new_obj_observer);
    app.world_mut().insert_resource(GameTick(0));
    app.world_mut().insert_resource(Clients::default());
    app.world_mut().insert_resource(VisibleEvents(Vec::new()));
    app.world_mut()
        .insert_resource(EntityObjMap(HashMap::new()));
    app.world_mut()
        .insert_resource(Templates::from_obj_templates(Vec::new()));
    app
}

fn spawn_fortification_test_unit(app: &mut App, id: i32) -> Entity {
    app.world_mut()
        .spawn((
            Id(id),
            PlayerId(1),
            Position { x: 0, y: 0 },
            Template("Human Villager".into()),
            Class(CLASS_UNIT.into()),
            Subclass::Villager,
            State::None,
            Effects(HashMap::new()),
        ))
        .id()
}

fn spawn_fortification_test_wall(app: &mut App, id: i32, state: State) -> Entity {
    app.world_mut()
        .spawn((
            Id(id),
            PlayerId(1),
            Position { x: 0, y: 0 },
            Template("Stockade".into()),
            Class(CLASS_STRUCTURE.into()),
            Subclass::Wall,
            state,
            Effects(HashMap::new()),
        ))
        .id()
}

#[test]
fn founded_wall_does_not_fortify_existing_occupants_on_spawn() {
    let mut app = setup_new_obj_observer_test_app();
    let unit_entity = spawn_fortification_test_unit(&mut app, 1);
    let wall_entity = spawn_fortification_test_wall(&mut app, 2, State::Founded);

    app.world_mut().trigger(NewObj {
        entity: wall_entity,
    });
    app.world_mut().flush();

    let effects = app.world().get::<Effects>(unit_entity).unwrap();
    assert!(!effects.has(Effect::Fortified));
    assert!(app.world().get::<Fortified>(unit_entity).is_none());
}

#[test]
fn completed_wall_fortifies_existing_occupants_on_spawn() {
    let mut app = setup_new_obj_observer_test_app();
    let unit_entity = spawn_fortification_test_unit(&mut app, 1);
    let wall_entity = spawn_fortification_test_wall(&mut app, 2, State::None);

    app.world_mut().trigger(NewObj {
        entity: wall_entity,
    });
    app.world_mut().flush();

    let effects = app.world().get::<Effects>(unit_entity).unwrap();
    assert!(effects.has(Effect::Fortified));
    assert_eq!(app.world().get::<Fortified>(unit_entity).unwrap().id, 2);
}

#[test]
fn founded_wall_does_not_fortify_new_occupant_on_spawn() {
    let mut app = setup_new_obj_observer_test_app();
    spawn_fortification_test_wall(&mut app, 2, State::Founded);
    let unit_entity = spawn_fortification_test_unit(&mut app, 1);

    app.world_mut().trigger(NewObj {
        entity: unit_entity,
    });
    app.world_mut().flush();

    let effects = app.world().get::<Effects>(unit_entity).unwrap();
    assert!(!effects.has(Effect::Fortified));
    assert!(app.world().get::<Fortified>(unit_entity).is_none());
}

#[test]
fn completed_wall_fortifies_new_occupant_on_spawn() {
    let mut app = setup_new_obj_observer_test_app();
    spawn_fortification_test_wall(&mut app, 2, State::None);
    let unit_entity = spawn_fortification_test_unit(&mut app, 1);

    app.world_mut().trigger(NewObj {
        entity: unit_entity,
    });
    app.world_mut().flush();

    let effects = app.world().get::<Effects>(unit_entity).unwrap();
    assert!(effects.has(Effect::Fortified));
    assert_eq!(app.world().get::<Fortified>(unit_entity).unwrap().id, 2);
}

#[test]
fn completed_wall_fortifies_builder_still_in_building_state() {
    let mut app = App::new();
    app.add_systems(Update, build_system);
    app.insert_resource(GameTick(10));
    app.insert_resource(EntityObjMap(HashMap::new()));
    app.insert_resource(Templates::from_obj_templates(load_obj_templates()));

    app.world_mut().spawn((
        Id(1),
        PlayerId(1),
        Position { x: 0, y: 0 },
        State::Building,
        Class(CLASS_STRUCTURE.to_string()),
        ClassStructure,
        Subclass::Wall,
        Template("Stockade".to_string()),
        Stats {
            hp: 1,
            stamina: None,
            mana: None,
            base_hp: 20,
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
            start_time: 0,
        },
        WorkQueue(Vec::new()),
        StateBuilding,
    ));

    let builder_entity = app
        .world_mut()
        .spawn((
            Id(2),
            PlayerId(1),
            Position { x: 0, y: 0 },
            State::Building,
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
            Effects(HashMap::new()),
        ))
        .id();

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .new_obj(2, builder_entity);

    app.update();

    let effects = app.world().get::<Effects>(builder_entity).unwrap();
    assert!(effects.has(Effect::Fortified));
    assert_eq!(app.world().get::<Fortified>(builder_entity).unwrap().id, 1);
}

#[test]
fn sanctuary_power_score_requires_non_novice_rank() {
    let skills = Skills::new();
    let inventory = Inventory {
        owner: 1,
        items: Vec::new(),
    };

    assert_eq!(
        sanctuary_power_score(
            &Template("Novice Warrior".to_string()),
            &skills,
            &inventory,
            1_000
        ),
        0
    );
    assert!(!sanctuary_exploration_unlocked(sanctuary_power_score(
        &Template("Skilled Warrior".to_string()),
        &skills,
        &inventory,
        100
    )));
    assert!(sanctuary_exploration_unlocked(sanctuary_power_score(
        &Template("Great Warrior".to_string()),
        &skills,
        &inventory,
        100
    )));
}

#[test]
fn sanctuary_exposure_resets_when_protected_or_unlocked() {
    let mut excursions = SanctuaryExcursions(HashMap::new());

    assert_eq!(
        record_sanctuary_exposure(&mut excursions, 1, false, false),
        Some(1)
    );
    assert_eq!(
        record_sanctuary_exposure(&mut excursions, 1, false, false),
        Some(2)
    );
    assert_eq!(
        record_sanctuary_exposure(&mut excursions, 1, true, false),
        None
    );
    assert!(!excursions.contains_key(&1));
    assert_eq!(
        record_sanctuary_exposure(&mut excursions, 1, false, false),
        Some(1)
    );
    assert_eq!(
        record_sanctuary_exposure(&mut excursions, 1, false, true),
        None
    );
    assert!(!excursions.contains_key(&1));
}

#[test]
fn sanctuary_hunter_cadence_and_composition_escalate() {
    assert!(should_spawn_sanctuary_hunters(1));
    assert!(should_spawn_sanctuary_hunters(2));
    assert!(should_spawn_sanctuary_hunters(3));
    assert!(should_spawn_sanctuary_hunters(4));
    assert!(should_spawn_sanctuary_hunters(5));

    assert_eq!(sanctuary_hunter_template_for_slot(0, 1, 0), "Wolf");
    assert_eq!(sanctuary_hunter_template_for_slot(5, 1, 0), "Wolf");
    assert_eq!(sanctuary_hunter_template_for_slot(2, 3, 0), "Spider");
    assert_eq!(sanctuary_hunter_template_for_slot(0, 5, 0), "Wolf Rider");
    assert_eq!(
        sanctuary_hunter_template_for_slot(1, 5, 0),
        "Goblin Pillager"
    );
    assert_eq!(sanctuary_hunter_template_for_slot(0, 1, 150), "Wolf Rider");
    assert_eq!(
        sanctuary_hunter_template_for_slot(1, 1, 150),
        "Goblin Pillager"
    );
}

#[test]
fn sanctuary_hunter_positions_fill_adjacent_open_tiles() {
    let map = flat_land_map();
    let hero_pos = Position { x: 20, y: 20 };
    let monolith_pos = Position { x: 1, y: 1 };
    let all_objs = vec![test_encounter_map_obj(
        MONOLITH_PLAYER_ID,
        monolith_pos,
        CLASS_STRUCTURE,
        &Subclass::Monolith.to_string(),
    )];

    let positions = sanctuary_hunter_adjacent_spawn_positions(hero_pos, &all_objs, &map);
    let expected = Map::ring((hero_pos.x, hero_pos.y), 1)
        .into_iter()
        .map(|(x, y)| Position { x, y })
        .collect::<HashSet<_>>();

    assert_eq!(positions.len(), 6);
    assert_eq!(positions.into_iter().collect::<HashSet<_>>(), expected);
}

#[test]
fn sanctuary_hunter_positions_skip_occupied_adjacent_tiles() {
    let map = flat_land_map();
    let hero_pos = Position { x: 20, y: 20 };
    let adjacent_tiles = Map::ring((hero_pos.x, hero_pos.y), 1)
        .into_iter()
        .map(|(x, y)| Position { x, y })
        .collect::<Vec<_>>();
    let blocked_player_pos = adjacent_tiles[0];
    let blocked_npc_pos = adjacent_tiles[1];
    let all_objs = vec![
        test_encounter_map_obj(
            MONOLITH_PLAYER_ID,
            Position { x: 1, y: 1 },
            CLASS_STRUCTURE,
            &Subclass::Monolith.to_string(),
        ),
        test_encounter_map_obj(
            1,
            blocked_player_pos,
            CLASS_UNIT,
            &Subclass::Villager.to_string(),
        ),
        test_encounter_map_obj(
            NPC_PLAYER_ID,
            blocked_npc_pos,
            CLASS_UNIT,
            &Subclass::Npc.to_string(),
        ),
    ];

    let positions = sanctuary_hunter_adjacent_spawn_positions(hero_pos, &all_objs, &map);
    let position_set = positions.into_iter().collect::<HashSet<_>>();

    assert_eq!(position_set.len(), 4);
    assert!(!position_set.contains(&blocked_player_pos));
    assert!(!position_set.contains(&blocked_npc_pos));
}

#[test]
fn reduce_wildness_at_pos_clamps_at_zero() {
    let mut map = flat_land_map();
    map.wildness = vec![0; (WIDTH * HEIGHT) as usize];
    let pos = Position { x: 10, y: 10 };
    let tile_index = (pos.y * WIDTH + pos.x) as usize;
    map.wildness[tile_index] = 2;

    assert!(reduce_wildness_at_pos(&mut map, pos));
    assert_eq!(map.get_wildness(pos.x, pos.y), 1);
    assert!(reduce_wildness_at_pos(&mut map, pos));
    assert_eq!(map.get_wildness(pos.x, pos.y), 0);
    assert!(!reduce_wildness_at_pos(&mut map, pos));
    assert_eq!(map.get_wildness(pos.x, pos.y), 0);
}

#[test]
fn idle_thirsty_hero_auto_drinks_from_inventory() {
    let mut app = App::new();
    app.add_systems(Update, hero_auto_consume_system);
    app.add_observer(state_change_observer);
    app.insert_resource(GameTick(100));
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(VisibleEvents(Vec::new()));

    let hero = app
        .world_mut()
        .spawn((
            Id(1),
            State::None,
            SubclassHero,
            LastCombatTick(0),
            EventExecuting {
                event_type: String::new(),
                state: EventExecutingState::None,
            },
            Inventory {
                owner: 1,
                items: vec![consumable_item(
                    10,
                    1,
                    "Waterskin (Filled)",
                    DRINK,
                    AttrKey::Thirst,
                    100.0,
                )],
            },
            Thirst::new(HERO_AUTO_CONSUME_THRESHOLD, 0.0),
            Hunger::new(0.0, 0.0),
        ))
        .id();

    app.update();

    assert_eq!(
        *app.world().entity(hero).get::<State>().unwrap(),
        State::Drinking
    );
    assert_eq!(
        app.world()
            .entity(hero)
            .get::<EventExecuting>()
            .unwrap()
            .state,
        EventExecutingState::Executing
    );

    let map_events = app.world().resource::<MapEvents>();
    assert_eq!(map_events.len(), 1);
    let event = map_events.values().next().unwrap();
    assert_eq!(event.obj_id, 1);
    assert_eq!(event.run_tick, 100 + HERO_AUTO_CONSUME_TICKS);
    match &event.event_type {
        VisibleEvent::DrinkEvent { item_id, obj_id } => {
            assert_eq!(*item_id, 10);
            assert_eq!(*obj_id, 1);
        }
        other => panic!("expected drink event, got {:?}", other),
    }
}

#[test]
fn hero_auto_eats_when_hungry_and_idle() {
    let mut app = App::new();
    app.add_systems(Update, hero_auto_consume_system);
    app.add_observer(state_change_observer);
    app.insert_resource(GameTick(200));
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(VisibleEvents(Vec::new()));

    let hero = app
        .world_mut()
        .spawn((
            Id(1),
            State::None,
            SubclassHero,
            LastCombatTick(0),
            EventExecuting {
                event_type: String::new(),
                state: EventExecutingState::None,
            },
            Inventory {
                owner: 1,
                items: vec![consumable_item(
                    20,
                    1,
                    "Salted Meat Strip",
                    FOOD,
                    AttrKey::Feed,
                    100.0,
                )],
            },
            Thirst::new(0.0, 0.0),
            Hunger::new(HERO_AUTO_CONSUME_THRESHOLD, 0.0),
        ))
        .id();

    app.update();

    assert_eq!(
        *app.world().entity(hero).get::<State>().unwrap(),
        State::Eating
    );

    let map_events = app.world().resource::<MapEvents>();
    assert_eq!(map_events.len(), 1);
    let event = map_events.values().next().unwrap();
    match &event.event_type {
        VisibleEvent::EatEvent { item_id, obj_id } => {
            assert_eq!(*item_id, 20);
            assert_eq!(*obj_id, 1);
        }
        other => panic!("expected eat event, got {:?}", other),
    }
}

#[test]
fn hero_auto_consume_skips_busy_combat_locked_and_non_hero_entities() {
    let mut app = App::new();
    app.add_systems(Update, hero_auto_consume_system);
    app.add_observer(state_change_observer);
    app.insert_resource(GameTick(300));
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(VisibleEvents(Vec::new()));

    let drink = consumable_item(30, 1, "Waterskin (Filled)", DRINK, AttrKey::Thirst, 100.0);
    let food = consumable_item(31, 2, "Salted Meat Strip", FOOD, AttrKey::Feed, 100.0);

    let busy_hero = app
        .world_mut()
        .spawn((
            Id(1),
            State::Moving,
            SubclassHero,
            LastCombatTick(0),
            Inventory {
                owner: 1,
                items: vec![drink.clone()],
            },
            Thirst::new(100.0, 0.0),
            Hunger::new(0.0, 0.0),
        ))
        .id();
    let combat_locked_hero = app
        .world_mut()
        .spawn((
            Id(2),
            State::None,
            SubclassHero,
            LastCombatTick(300),
            Inventory {
                owner: 2,
                items: vec![food],
            },
            Thirst::new(0.0, 0.0),
            Hunger::new(100.0, 0.0),
        ))
        .id();
    app.world_mut().spawn((
        Id(3),
        State::None,
        Inventory {
            owner: 3,
            items: vec![drink],
        },
        Thirst::new(100.0, 0.0),
        Hunger::new(0.0, 0.0),
    ));

    app.update();

    assert!(app.world().resource::<MapEvents>().is_empty());
    assert_eq!(
        *app.world().entity(busy_hero).get::<State>().unwrap(),
        State::Moving
    );
    assert_eq!(
        *app.world()
            .entity(combat_locked_hero)
            .get::<State>()
            .unwrap(),
        State::None
    );
}

fn bedroll_item(id: i32, owner: i32) -> Item {
    consumable_item(id, owner, "Bedroll", BEDROLL, AttrKey::Feed, 0.0)
}

#[test]
fn idle_tired_hero_auto_sleeps_with_bedroll() {
    let mut app = App::new();
    app.add_systems(Update, hero_auto_consume_system);
    app.add_observer(state_change_observer);
    app.insert_resource(GameTick(400));
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(VisibleEvents(Vec::new()));

    let hero = app
        .world_mut()
        .spawn((
            Id(1),
            State::None,
            SubclassHero,
            LastCombatTick(0),
            EventExecuting {
                event_type: String::new(),
                state: EventExecutingState::None,
            },
            Inventory {
                owner: 1,
                items: vec![bedroll_item(40, 1)],
            },
            Thirst::new(0.0, 0.0),
            Hunger::new(0.0, 0.0),
            Tired::new(HERO_AUTO_SLEEP_THRESHOLD, 0.0),
        ))
        .id();

    app.update();

    assert_eq!(
        *app.world().entity(hero).get::<State>().unwrap(),
        State::Sleeping
    );
    assert_eq!(
        app.world()
            .entity(hero)
            .get::<EventExecuting>()
            .unwrap()
            .state,
        EventExecutingState::Executing
    );

    let map_events = app.world().resource::<MapEvents>();
    assert_eq!(map_events.len(), 1);
    let event = map_events.values().next().unwrap();
    assert_eq!(event.obj_id, 1);
    assert_eq!(event.run_tick, 400 + HERO_AUTO_CONSUME_TICKS);
    match &event.event_type {
        VisibleEvent::SleepEvent { obj_id } => assert_eq!(*obj_id, 1),
        other => panic!("expected sleep event, got {:?}", other),
    }
}

#[test]
fn idle_tired_hero_without_bedroll_does_not_sleep() {
    let mut app = App::new();
    app.add_systems(Update, hero_auto_consume_system);
    app.add_observer(state_change_observer);
    app.insert_resource(GameTick(400));
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(VisibleEvents(Vec::new()));

    let hero = app
        .world_mut()
        .spawn((
            Id(1),
            State::None,
            SubclassHero,
            LastCombatTick(0),
            EventExecuting {
                event_type: String::new(),
                state: EventExecutingState::None,
            },
            Inventory {
                owner: 1,
                items: Vec::new(),
            },
            Thirst::new(0.0, 0.0),
            Hunger::new(0.0, 0.0),
            Tired::new(HERO_AUTO_SLEEP_THRESHOLD, 0.0),
        ))
        .id();

    app.update();

    assert_eq!(
        *app.world().entity(hero).get::<State>().unwrap(),
        State::None
    );
    assert!(app.world().resource::<MapEvents>().is_empty());
}

#[test]
fn idle_rested_hero_with_bedroll_does_not_sleep() {
    let mut app = App::new();
    app.add_systems(Update, hero_auto_consume_system);
    app.add_observer(state_change_observer);
    app.insert_resource(GameTick(400));
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(VisibleEvents(Vec::new()));

    let hero = app
        .world_mut()
        .spawn((
            Id(1),
            State::None,
            SubclassHero,
            LastCombatTick(0),
            EventExecuting {
                event_type: String::new(),
                state: EventExecutingState::None,
            },
            Inventory {
                owner: 1,
                items: vec![bedroll_item(40, 1)],
            },
            Thirst::new(0.0, 0.0),
            Hunger::new(0.0, 0.0),
            Tired::new(HERO_AUTO_SLEEP_THRESHOLD - 1.0, 0.0),
        ))
        .id();

    app.update();

    assert_eq!(
        *app.world().entity(hero).get::<State>().unwrap(),
        State::None
    );
    assert!(app.world().resource::<MapEvents>().is_empty());
}

#[test]
fn due_consumption_events_fail_closed_without_event_executing() {
    let mut app = App::new();
    app.add_systems(Update, drink_eat_system);
    app.insert_resource(GameTick(11));
    app.insert_resource(Clients::default());
    app.insert_resource(Ids::default());
    app.insert_resource(PlayerWorldPresenceState::default());
    app.insert_resource(EntityObjMap(HashMap::new()));
    app.insert_resource(Templates::from_obj_templates(Vec::new()));
    app.insert_resource(VisibleEvents(Vec::new()));
    app.insert_resource(MapEvents(HashMap::new()));
    app.insert_resource(ActiveInfos(HashMap::new()));

    let drinker = app
        .world_mut()
        .spawn((
            Id(1),
            State::Drinking,
            Inventory {
                owner: 1,
                items: vec![consumable_item(
                    10,
                    1,
                    "Waterskin (Filled)",
                    DRINK,
                    AttrKey::Thirst,
                    40.0,
                )],
            },
            Thirst::new(80.0, 0.0),
            Hunger::new(0.0, 0.0),
            Tired::new(0.0, 0.0),
        ))
        .id();
    let eater = app
        .world_mut()
        .spawn((
            Id(2),
            State::Eating,
            Inventory {
                owner: 2,
                items: vec![consumable_item(
                    20,
                    2,
                    "Salted Meat Strip",
                    FOOD,
                    AttrKey::Feed,
                    40.0,
                )],
            },
            Thirst::new(0.0, 0.0),
            Hunger::new(80.0, 0.0),
            Tired::new(0.0, 0.0),
        ))
        .id();
    let sleeper = app
        .world_mut()
        .spawn((
            Id(3),
            State::Sleeping,
            Inventory {
                owner: 3,
                items: Vec::new(),
            },
            Thirst::new(0.0, 0.0),
            Hunger::new(0.0, 0.0),
            Tired::new(80.0, 0.0),
            Stats {
                hp: 40,
                stamina: Some(5),
                mana: Some(2),
                base_hp: 100,
                base_stamina: Some(20),
                base_mana: Some(10),
                base_def: 0,
                damage_range: None,
                base_damage: None,
                base_speed: None,
                base_vision: None,
            },
        ))
        .id();

    {
        let mut ids = app.world_mut().resource_mut::<Ids>();
        ids.new_obj(1, 1);
        ids.new_obj(2, 1);
        ids.new_obj(3, 1);
    }
    {
        let mut entity_map = app.world_mut().resource_mut::<EntityObjMap>();
        entity_map.new_obj(1, drinker);
        entity_map.new_obj(2, eater);
        entity_map.new_obj(3, sleeper);
    }
    {
        let mut map_events = app.world_mut().resource_mut::<MapEvents>();
        map_events.new(
            1,
            10,
            VisibleEvent::DrinkEvent {
                item_id: 10,
                obj_id: 1,
            },
        );
        map_events.new(
            2,
            10,
            VisibleEvent::EatEvent {
                item_id: 20,
                obj_id: 2,
            },
        );
        map_events.new(3, 10, VisibleEvent::SleepEvent { obj_id: 3 });
    }

    app.update();

    assert!(app.world().resource::<MapEvents>().is_empty());
    assert_eq!(
        app.world().entity(drinker).get::<Thirst>().unwrap().thirst,
        80.0
    );
    let drink_inventory = app.world().entity(drinker).get::<Inventory>().unwrap();
    assert_eq!(drink_inventory.items.len(), 1);
    assert_eq!(drink_inventory.items[0].quantity, 1);
    assert_eq!(
        app.world().entity(eater).get::<Hunger>().unwrap().hunger,
        80.0
    );
    let food_inventory = app.world().entity(eater).get::<Inventory>().unwrap();
    assert_eq!(food_inventory.items.len(), 1);
    assert_eq!(food_inventory.items[0].quantity, 1);
    assert_eq!(
        app.world().entity(sleeper).get::<Tired>().unwrap().tired,
        80.0
    );
    let stats = app.world().entity(sleeper).get::<Stats>().unwrap();
    assert_eq!(stats.hp, 40);
    assert_eq!(stats.stamina, Some(5));
    assert_eq!(stats.mana, Some(2));
}

#[test]
fn due_find_shelter_event_fails_closed_without_event_executing() {
    let mut app = App::new();
    app.add_systems(Update, find_shelter_system);
    app.insert_resource(GameTick(11));
    app.insert_resource(Ids::default());
    app.insert_resource(PlayerWorldPresenceState::default());
    app.insert_resource(EntityObjMap(HashMap::new()));
    app.insert_resource(MapEvents(HashMap::new()));

    let villager = app
        .world_mut()
        .spawn((
            Id(1),
            PlayerId(1),
            Position { x: 0, y: 0 },
            ActiveShelter(NO_SHELTER),
            SubclassVillager,
        ))
        .id();
    let shelter = app
        .world_mut()
        .spawn((
            Id(2),
            PlayerId(1),
            Position { x: 1, y: 0 },
            State::None,
            Shelter {
                max_residents: 2,
                residents: Vec::new(),
            },
        ))
        .id();

    {
        let mut ids = app.world_mut().resource_mut::<Ids>();
        ids.new_obj(1, 1);
        ids.new_obj(2, 1);
    }
    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .new_obj(1, villager);
    app.world_mut().resource_mut::<MapEvents>().new(
        1,
        10,
        VisibleEvent::FindShelterEvent { obj_id: 1 },
    );

    app.update();

    assert!(app.world().resource::<MapEvents>().is_empty());
    assert_eq!(
        app.world()
            .entity(villager)
            .get::<ActiveShelter>()
            .unwrap()
            .0,
        NO_SHELTER
    );
    assert!(app
        .world()
        .entity(shelter)
        .get::<Shelter>()
        .unwrap()
        .residents
        .is_empty());
}

#[test]
fn sleep_heal_scales_with_tiredness() {
    // Fully exhausted sleeper gets the full fraction of max hp...
    assert_eq!(sleep_heal_amount(110, 1.0), 22);
    // ...half-tired gets half...
    assert_eq!(sleep_heal_amount(110, 0.5), 11);
    // ...and a rested sleeper gets nothing — sleep is not a spammable heal.
    assert_eq!(sleep_heal_amount(110, 0.0), 0);
    assert_eq!(sleep_heal_amount(110, -0.5), 0);
    assert_eq!(sleep_heal_amount(110, 2.0), 22);
}

#[test]
fn first_resurrection_uses_flat_affordable_cost() {
    // Flat and below the monolith's 10 starting shards, even with earned XP.
    assert_eq!(resurrection_attempt_cost(1, 0), FIRST_DEATH_SOULSHARD_COST);
    assert_eq!(
        resurrection_attempt_cost(1, 5000),
        FIRST_DEATH_SOULSHARD_COST
    );
}

#[test]
fn later_resurrections_scale_from_second_death() {
    // Second death starts the formula at the base cost...
    assert_eq!(resurrection_attempt_cost(2, 0), 10);
    // ...and each further death applies the 1.2x escalation.
    assert_eq!(resurrection_attempt_cost(3, 0), 12);
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
fn necromancer_activation_adds_scripted_corpse_hunt_brain() {
    let mut app = App::new();
    let old_home = Position { x: 1, y: 1 };
    let home = Position { x: 16, y: 32 };
    let corpse_anchor = Position { x: 5, y: 31 };
    let entity = app
        .world_mut()
        .spawn((
            Home { pos: old_home },
            VisibleTarget::new(999),
            TaskTarget::new(999),
            EventExecuting {
                event_type: "old".to_string(),
                state: EventExecutingState::Executing,
            },
        ))
        .id();

    {
        let mut commands = app.world_mut().commands();
        Encounter::activate_necromancer_hunting_corpse(entity, home, corpse_anchor, &mut commands);
    }
    app.world_mut().flush();

    let entity_ref = app.world().entity(entity);
    assert_eq!(entity_ref.get::<Home>().unwrap().pos, home);
    assert_eq!(
        entity_ref
            .get::<ScriptedCorpseHunt>()
            .unwrap()
            .corpse_anchor,
        corpse_anchor
    );
    assert_eq!(
        entity_ref.get::<EventExecuting>().unwrap().state,
        EventExecutingState::None
    );
    assert_eq!(entity_ref.get::<TaskTarget>().unwrap().target, NO_TARGET);
    assert!(entity_ref.contains::<ThinkerBuilder>());
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
                start_time: 0,
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
fn visible_event_move_packets_keep_source_coordinates() {
    let mut app = App::new();
    app.add_systems(Update, visible_event_system);
    app.insert_resource(EntityObjMap(HashMap::new()));

    let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
    let client_id = Uuid::new_v4();
    app.insert_resource(Clients(Arc::new(Mutex::new(HashMap::from([(
        client_id,
        Client {
            id: client_id,
            player_id: 1,
            sender,
        },
    )])))));

    app.world_mut().spawn((
        Id(1),
        PlayerId(1),
        Position { x: 5, y: 5 },
        Name("Observer".to_string()),
        Template("Human".to_string()),
        Class(CLASS_UNIT.to_string()),
        Subclass::Hero,
        State::None,
        Viewshed { range: 10 },
        Misc::default(),
    ));

    let moved_entity = app
        .world_mut()
        .spawn((
            Id(2),
            PlayerId(2),
            Position { x: 4, y: 4 },
            Name("Moved Unit".to_string()),
            Template("Wolf".to_string()),
            Class(CLASS_UNIT.to_string()),
            Subclass::Npc,
            State::None,
            Misc::default(),
        ))
        .id();

    let updated_entity = app
        .world_mut()
        .spawn((
            Id(3),
            PlayerId(2),
            Position { x: 7, y: 7 },
            Name("Updated Unit".to_string()),
            Template("Wolf".to_string()),
            Class(CLASS_UNIT.to_string()),
            Subclass::Npc,
            State::None,
            Misc::default(),
        ))
        .id();

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .new_obj(2, moved_entity);
    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .new_obj(3, updated_entity);

    app.insert_resource(VisibleEvents(vec![
        MapEvent {
            event_id: Uuid::new_v4(),
            obj_id: 2,
            run_tick: 0,
            event_type: VisibleEvent::MoveEvent {
                src: Position { x: 3, y: 4 },
                dst: Position { x: 4, y: 4 },
            },
        },
        MapEvent {
            event_id: Uuid::new_v4(),
            obj_id: 3,
            run_tick: 0,
            event_type: VisibleEvent::UpdateObjPosEvent {
                src: Position { x: 6, y: 7 },
                dst: Position { x: 7, y: 7 },
            },
        },
    ]));

    app.update();

    let msg = receiver.try_recv().expect("perception change packet");
    let packet: serde_json::Value = serde_json::from_str(&msg).expect("valid json packet");
    assert_eq!(packet["packet"].as_str(), Some("perception_changes"));

    let move_sources: HashMap<i32, (i32, i32)> = packet["events"]
        .as_array()
        .expect("events array")
        .iter()
        .filter(|event| event["event"].as_str() == Some("obj_move"))
        .map(|event| {
            (
                event["obj"]["id"].as_i64().unwrap() as i32,
                (
                    event["src_x"].as_i64().unwrap() as i32,
                    event["src_y"].as_i64().unwrap() as i32,
                ),
            )
        })
        .collect();

    assert_eq!(move_sources.get(&2), Some(&(3, 4)));
    assert_eq!(move_sources.get(&3), Some(&(6, 7)));
}

#[test]
fn queued_move_completion_rejects_dead_actors() {
    assert!(!move_event_actor_is_dead(State::None, false));
    assert!(move_event_actor_is_dead(State::Dead, false));
    assert!(move_event_actor_is_dead(State::Moving, true));
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
fn personal_crisis_is_the_default_survival_director_mode() {
    assert_eq!(
        SurvivalDirectorConfig::default().mode,
        SurvivalDirectorMode::PersonalCrisis
    );
}

fn test_client(id: Uuid, player_id: i32, sender: tokio::sync::mpsc::Sender<String>) -> Client {
    Client {
        id,
        player_id,
        sender,
    }
}

#[test]
fn client_presence_handles_multiple_connections_removals_and_stale_records() {
    let player_id = 42;
    let clients = Clients::default();
    assert!(!clients.is_player_online(player_id));

    let first_id = Uuid::from_u128(1);
    let second_id = Uuid::from_u128(2);
    let (first_sender, _first_receiver) = tokio::sync::mpsc::channel(1);
    let (second_sender, _second_receiver) = tokio::sync::mpsc::channel(1);
    clients
        .lock()
        .unwrap()
        .insert(first_id, test_client(first_id, player_id, first_sender));
    assert!(clients.is_player_online(player_id));

    clients
        .lock()
        .unwrap()
        .insert(second_id, test_client(second_id, player_id, second_sender));
    clients.lock().unwrap().remove(&first_id);
    assert!(
        clients.is_player_online(player_id),
        "one remaining valid client must keep the player online"
    );

    clients.lock().unwrap().remove(&second_id);
    assert!(!clients.is_player_online(player_id));

    let stale_id = Uuid::from_u128(3);
    let (stale_sender, stale_receiver) = tokio::sync::mpsc::channel(1);
    drop(stale_receiver);
    clients
        .lock()
        .unwrap()
        .insert(stale_id, test_client(stale_id, player_id, stale_sender));
    assert!(
        !clients.is_player_online(player_id),
        "a closed sender left in the map is not an active connection"
    );

    let mismatched_key = Uuid::from_u128(4);
    let mismatched_client_id = Uuid::from_u128(5);
    let (mismatched_sender, _mismatched_receiver) = tokio::sync::mpsc::channel(1);
    clients.lock().unwrap().insert(
        mismatched_key,
        test_client(mismatched_client_id, player_id, mismatched_sender),
    );
    assert!(
        !clients.is_player_online(player_id),
        "a malformed stale record is not a valid connection"
    );
}

#[test]
fn goblin_pressure_is_gated_deterministic_and_capped() {
    let developed = GoblinPressureFacts {
        danger_unlocked: true,
        completed_structures: 3,
        living_villagers: 1,
        stored_gold: 100,
        sanctuary_level: 5,
        explore_poi: true,
        choose_expansion: true,
        online_active_ticks: GOBLIN_ONLINE_PRESSURE_TIER_THREE_TICKS,
    };

    let first = calculate_goblin_pressure(&developed);
    let second = calculate_goblin_pressure(&developed);
    let breakdown = calculate_goblin_pressure_breakdown(&developed);
    assert_eq!(first, second);
    assert_eq!(first, GOBLIN_PRESSURE_MAX);
    assert_eq!(breakdown.contributor_sum(), breakdown.raw_total);
    assert_eq!(breakdown.raw_total, 110);
    assert_eq!(breakdown.clamped_total, first);
    assert_eq!(
        calculate_goblin_pressure(&GoblinPressureFacts {
            danger_unlocked: false,
            ..developed
        }),
        0,
        "settlement facts cannot bypass the introduction safety gate"
    );
}

#[test]
fn goblin_pressure_breakdown_uses_every_authoritative_category_and_snapshot_constants() {
    let facts = GoblinPressureFacts {
        danger_unlocked: true,
        completed_structures: 3,
        living_villagers: 1,
        stored_gold: GOBLIN_GOLD_TIER_THREE,
        sanctuary_level: 5,
        explore_poi: true,
        choose_expansion: true,
        online_active_ticks: GOBLIN_ONLINE_PRESSURE_TIER_THREE_TICKS,
    };
    let breakdown = calculate_goblin_pressure_breakdown(&facts);

    assert_eq!(breakdown.danger_unlocked, GOBLIN_DANGER_UNLOCKED_PRESSURE);
    assert_eq!(breakdown.structures, GOBLIN_THREE_STRUCTURES_PRESSURE);
    assert_eq!(breakdown.villagers, GOBLIN_VILLAGER_PRESSURE);
    assert_eq!(breakdown.explore_poi, GOBLIN_EXPLORE_POI_PRESSURE);
    assert_eq!(breakdown.choose_expansion, GOBLIN_CHOOSE_EXPANSION_PRESSURE);
    assert_eq!(breakdown.stored_gold, GOBLIN_GOLD_PRESSURE_PER_TIER * 3);
    assert_eq!(breakdown.sanctuary, GOBLIN_SANCTUARY_PRESSURE_MAX);
    assert_eq!(breakdown.online_time, GOBLIN_ONLINE_PRESSURE_PER_TIER * 3);
    assert_eq!(breakdown.contributor_sum(), breakdown.raw_total);
    assert_eq!(breakdown.clamped_total, calculate_goblin_pressure(&facts));

    let snapshot = goblin_crisis_balance_config_snapshot();
    assert_eq!(snapshot.pressure_max, GOBLIN_PRESSURE_MAX);
    assert_eq!(snapshot.signs_threshold, GOBLIN_SIGNS_PRESSURE);
    assert_eq!(snapshot.pressure_threshold, GOBLIN_PRESSURE_PHASE_PRESSURE);
    assert_eq!(snapshot.preparing_threshold, GOBLIN_PREPARING_PRESSURE);
    assert_eq!(
        snapshot.assault_ready_threshold,
        GOBLIN_ASSAULT_READY_PRESSURE
    );
    assert_eq!(snapshot.game_ticks_per_day, GAME_TICKS_PER_DAY);
    assert_eq!(snapshot.preferred_launch_start_tick, DUSK);
    assert_eq!(snapshot.preferred_launch_wrap_end_tick, FIRST_LIGHT);
    assert_eq!(snapshot.assault_composition, GOBLIN_ASSAULT_COMPOSITION);
}

#[test]
fn goblin_pressure_uses_named_fact_thresholds_without_double_counting() {
    let base = GoblinPressureFacts {
        danger_unlocked: true,
        ..Default::default()
    };
    assert_eq!(calculate_goblin_pressure(&base), 10);
    assert_eq!(
        calculate_goblin_pressure(&GoblinPressureFacts {
            completed_structures: 3,
            ..base
        }),
        30
    );
    assert_eq!(
        calculate_goblin_pressure(&GoblinPressureFacts {
            living_villagers: 1,
            explore_poi: true,
            choose_expansion: true,
            ..base
        }),
        50
    );
    assert_eq!(
        calculate_goblin_pressure(&GoblinPressureFacts {
            stored_gold: 24,
            ..base
        }),
        10
    );
    assert_eq!(
        calculate_goblin_pressure(&GoblinPressureFacts {
            stored_gold: 25,
            ..base
        }),
        15
    );
    assert_eq!(
        calculate_goblin_pressure(&GoblinPressureFacts {
            stored_gold: 50,
            sanctuary_level: 3,
            online_active_ticks: GOBLIN_ONLINE_PRESSURE_TIER_TWO_TICKS,
            ..base
        }),
        36
    );
}

#[test]
fn online_crisis_time_is_idempotent_and_excludes_inactive_intervals() {
    let mut crisis = SettlementCrisis::new(100);

    assert_eq!(advance_online_crisis_time(&mut crisis, 110, true), 10);
    assert_eq!(crisis.online_active_ticks, 10);
    assert_eq!(crisis.phase_online_ticks, 10);

    assert_eq!(advance_online_crisis_time(&mut crisis, 110, true), 0);
    assert_eq!(crisis.online_active_ticks, 10);

    assert_eq!(advance_online_crisis_time(&mut crisis, 200, false), 0);
    assert_eq!(crisis.online_active_ticks, 10);
    assert_eq!(crisis.last_evaluated_tick, 200);

    assert_eq!(advance_online_crisis_time(&mut crisis, 215, true), 15);
    assert_eq!(crisis.online_active_ticks, 25);

    assert_eq!(advance_online_crisis_time(&mut crisis, 205, true), 0);
    assert_eq!(crisis.online_active_ticks, 25);
    assert_eq!(crisis.last_evaluated_tick, 215);

    assert_eq!(advance_online_crisis_time(&mut crisis, 215, true), 0);
    assert_eq!(crisis.online_active_ticks, 25);
    assert_eq!(advance_online_crisis_time(&mut crisis, 220, true), 5);
    assert_eq!(crisis.online_active_ticks, 30);
}

#[test]
fn goblin_phase_transitions_are_ordered_timed_and_stop_at_assault_ready() {
    let mut crisis = SettlementCrisis::new(10);
    crisis.pressure = GOBLIN_PRESSURE_MAX;
    crisis.phase_online_ticks = i32::MAX;

    assert_eq!(
        transition_goblin_crisis(&mut crisis, 20),
        Some((CrisisPhase::Dormant, CrisisPhase::Signs))
    );
    assert_eq!(crisis.phase, CrisisPhase::Signs);
    assert_eq!(crisis.phase_online_ticks, 0);
    assert_eq!(crisis.phase_started_tick, 20);
    assert!(
        transition_goblin_crisis(&mut crisis, 20).is_none(),
        "a developed settlement advances at most one phase per evaluation"
    );

    crisis.phase_online_ticks = GOBLIN_SIGNS_MIN_ONLINE_TICKS - 1;
    assert!(transition_goblin_crisis(&mut crisis, 30).is_none());
    crisis.phase_online_ticks = GOBLIN_SIGNS_MIN_ONLINE_TICKS;
    assert_eq!(
        transition_goblin_crisis(&mut crisis, 40),
        Some((CrisisPhase::Signs, CrisisPhase::Pressure))
    );

    crisis.phase_online_ticks = GOBLIN_PRESSURE_MIN_ONLINE_TICKS - 1;
    assert!(transition_goblin_crisis(&mut crisis, 50).is_none());
    crisis.phase_online_ticks = GOBLIN_PRESSURE_MIN_ONLINE_TICKS;
    assert_eq!(
        transition_goblin_crisis(&mut crisis, 51),
        Some((CrisisPhase::Pressure, CrisisPhase::Preparing))
    );
    assert!(crisis.warning_active);

    crisis.phase_online_ticks = GOBLIN_PREPARING_MIN_ONLINE_TICKS - 1;
    assert!(transition_goblin_crisis(&mut crisis, 60).is_none());
    crisis.phase_online_ticks = GOBLIN_PREPARING_MIN_ONLINE_TICKS;
    assert_eq!(
        transition_goblin_crisis(&mut crisis, 61),
        Some((CrisisPhase::Preparing, CrisisPhase::AssaultReady))
    );
    assert!(crisis.warning_active);
    assert!(transition_goblin_crisis(&mut crisis, 70).is_none());
    assert_eq!(crisis.phase, CrisisPhase::AssaultReady);
}

#[test]
fn goblin_phase_pressure_thresholds_are_enforced() {
    let mut crisis = SettlementCrisis::new(0);
    crisis.pressure = GOBLIN_SIGNS_PRESSURE - 1;
    assert!(next_goblin_crisis_phase(&crisis).is_none());
    crisis.pressure = GOBLIN_SIGNS_PRESSURE;
    assert_eq!(next_goblin_crisis_phase(&crisis), Some(CrisisPhase::Signs));

    crisis.phase = CrisisPhase::Signs;
    crisis.phase_online_ticks = GOBLIN_SIGNS_MIN_ONLINE_TICKS;
    crisis.pressure = GOBLIN_PRESSURE_PHASE_PRESSURE - 1;
    assert!(next_goblin_crisis_phase(&crisis).is_none());
    crisis.pressure = GOBLIN_PRESSURE_PHASE_PRESSURE;
    assert_eq!(
        next_goblin_crisis_phase(&crisis),
        Some(CrisisPhase::Pressure)
    );

    crisis.phase = CrisisPhase::Pressure;
    crisis.phase_online_ticks = GOBLIN_PRESSURE_MIN_ONLINE_TICKS;
    crisis.pressure = GOBLIN_PREPARING_PRESSURE - 1;
    assert!(next_goblin_crisis_phase(&crisis).is_none());
    crisis.pressure = GOBLIN_PREPARING_PRESSURE;
    assert_eq!(
        next_goblin_crisis_phase(&crisis),
        Some(CrisisPhase::Preparing)
    );

    crisis.phase = CrisisPhase::Preparing;
    crisis.phase_online_ticks = GOBLIN_PREPARING_MIN_ONLINE_TICKS;
    crisis.pressure = GOBLIN_ASSAULT_READY_PRESSURE - 1;
    assert!(next_goblin_crisis_phase(&crisis).is_none());
    crisis.pressure = GOBLIN_ASSAULT_READY_PRESSURE;
    assert_eq!(
        next_goblin_crisis_phase(&crisis),
        Some(CrisisPhase::AssaultReady)
    );
}

#[test]
fn goblin_balance_checkpoint2_values_are_exact_and_other_pacing_controls_are_unchanged() {
    let snapshot = goblin_crisis_balance_config_snapshot();

    assert_eq!(snapshot.pressure_max, 100);
    assert_eq!(snapshot.danger_unlocked_pressure, 10);
    assert_eq!(snapshot.three_structures_pressure, 20);
    assert_eq!(snapshot.villager_pressure, 15);
    assert_eq!(snapshot.explore_poi_pressure, 10);
    assert_eq!(snapshot.choose_expansion_pressure, 15);
    assert_eq!(snapshot.gold_tier_thresholds, [25, 50, 100]);
    assert_eq!(snapshot.gold_pressure_per_tier, 5);
    assert_eq!(snapshot.sanctuary_pressure_per_level, 2);
    assert_eq!(snapshot.sanctuary_pressure_max, 10);
    assert_eq!(snapshot.online_pressure_tier_ticks, [600, 1_800, 3_600]);
    assert_eq!(snapshot.online_pressure_per_tier, 5);

    assert_eq!(snapshot.signs_threshold, 20);
    assert_eq!(snapshot.pressure_threshold, 45);
    assert_eq!(snapshot.preparing_threshold, 45);
    assert_eq!(snapshot.assault_ready_threshold, 49);
    assert_eq!(snapshot.signs_min_online_ticks, 600);
    assert_eq!(snapshot.pressure_min_online_ticks, 1_200);
    assert_eq!(snapshot.preparing_min_online_ticks, 1_800);
    assert_eq!(snapshot.assault_ready_grace_ticks, 300);
    assert_eq!(snapshot.assault_max_online_wait_ticks, 1_200);
    assert_eq!(snapshot.preferred_launch_window, "dusk_or_night");
}

#[test]
fn goblin_balance_checkpoint2_growth_path_is_deterministic_ordered_and_cannot_skip_phases() {
    let passive = GoblinPressureFacts {
        danger_unlocked: true,
        online_active_ticks: GOBLIN_ONLINE_PRESSURE_TIER_THREE_TICKS,
        ..Default::default()
    };
    assert_eq!(calculate_goblin_pressure(&passive), 25);

    let developed = GoblinPressureFacts {
        completed_structures: 3,
        sanctuary_level: 2,
        ..passive
    };
    let expected = calculate_goblin_pressure_breakdown(&developed);
    assert_eq!(expected.clamped_total, 49);
    for _ in 0..3 {
        assert_eq!(calculate_goblin_pressure_breakdown(&developed), expected);
    }

    let mut crisis = SettlementCrisis::new(0);
    crisis.pressure = expected.clamped_total;
    crisis.phase_online_ticks = i32::MAX;
    assert_eq!(
        transition_goblin_crisis(&mut crisis, 1),
        Some((CrisisPhase::Dormant, CrisisPhase::Signs))
    );
    assert!(transition_goblin_crisis(&mut crisis, 1).is_none());

    crisis.phase_online_ticks = GOBLIN_SIGNS_MIN_ONLINE_TICKS;
    assert_eq!(
        transition_goblin_crisis(&mut crisis, 2),
        Some((CrisisPhase::Signs, CrisisPhase::Pressure))
    );
    crisis.phase_online_ticks = GOBLIN_PRESSURE_MIN_ONLINE_TICKS;
    assert_eq!(
        transition_goblin_crisis(&mut crisis, 3),
        Some((CrisisPhase::Pressure, CrisisPhase::Preparing))
    );
    crisis.phase_online_ticks = GOBLIN_PREPARING_MIN_ONLINE_TICKS;
    assert_eq!(
        transition_goblin_crisis(&mut crisis, 4),
        Some((CrisisPhase::Preparing, CrisisPhase::AssaultReady))
    );

    let mut below_ready = SettlementCrisis::new(0);
    below_ready.phase = CrisisPhase::Preparing;
    below_ready.phase_online_ticks = GOBLIN_PREPARING_MIN_ONLINE_TICKS;
    below_ready.pressure = GOBLIN_ASSAULT_READY_PRESSURE - 1;
    assert!(transition_goblin_crisis(&mut below_ready, 5).is_none());
}

#[test]
fn global_calendar_tick_does_not_change_goblin_phase_eligibility() {
    let mut early_world = SettlementCrisis::new(0);
    early_world.pressure = GOBLIN_SIGNS_PRESSURE;
    let mut late_world = early_world.clone();

    assert_eq!(
        transition_goblin_crisis(&mut early_world, 100),
        transition_goblin_crisis(&mut late_world, GAME_TICKS_PER_DAY * 100)
    );
    assert_eq!(early_world.phase, late_world.phase);
    assert_eq!(early_world.pressure, late_world.pressure);
}

#[test]
fn assault_launch_policy_requires_online_grace_prefers_darkness_and_has_a_daylight_fallback() {
    let morning = MORNING;
    let dusk = DUSK;
    let night = NIGHT;

    assert!(!assault_launch_allowed(ASSAULT_READY_GRACE_TICKS - 1, dusk));
    assert!(!assault_launch_allowed(ASSAULT_READY_GRACE_TICKS, morning));
    assert!(assault_launch_allowed(ASSAULT_READY_GRACE_TICKS, dusk));
    assert!(assault_launch_allowed(ASSAULT_READY_GRACE_TICKS, night));
    assert!(assault_launch_allowed(
        ASSAULT_MAX_ONLINE_WAIT_TICKS,
        morning
    ));
    assert!(is_assault_preferred_time(FIRST_LIGHT - 1));
    assert!(!is_assault_preferred_time(FIRST_LIGHT));
}

#[test]
fn assault_ids_are_monotonic_and_not_derived_from_the_game_tick() {
    let mut ids = NextCrisisAssaultId::default();
    let first = ids.allocate().unwrap();
    let second = ids.allocate().unwrap();
    assert_eq!(first, 1);
    assert_eq!(second, 2);
    assert_ne!(first, DUSK as u64);
}

#[test]
fn personal_assault_anchor_priority_and_missing_anchor_policy_are_explicit() {
    let player_id = 7;
    let hero = AssaultHeroInfo {
        id: 70,
        pos: Position { x: 10, y: 10 },
        bound_monolith_id: Some(90),
        valid_run: true,
    };
    let spawn_positions = SpawnPositions(HashMap::from([(player_id, Position { x: 9, y: 9 })]));
    let structures = vec![
        AssaultStructureInfo {
            id: 71,
            owner_player_id: player_id,
            pos: Position { x: 8, y: 8 },
            subclass: Subclass::Storage,
        },
        AssaultStructureInfo {
            id: 72,
            owner_player_id: player_id,
            pos: Position { x: 12, y: 12 },
            subclass: Subclass::Campfire,
        },
        AssaultStructureInfo {
            id: 73,
            owner_player_id: player_id + 1,
            pos: Position { x: 9, y: 9 },
            subclass: Subclass::Campfire,
        },
    ];
    let monoliths = HashMap::from([(
        90,
        AssaultMonolithInfo {
            pos: Position { x: 7, y: 7 },
            sanctuary_level: 3,
        },
    )]);

    let bound =
        select_personal_assault_anchor(player_id, hero, &spawn_positions, &structures, &monoliths)
            .unwrap();
    assert_eq!(bound.id, 90);
    assert_eq!(bound.kind, AssaultAnchorKind::BoundMonolith);

    let primary = select_personal_assault_anchor(
        player_id,
        hero,
        &spawn_positions,
        &structures,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(primary.id, 72);
    assert_eq!(primary.kind, AssaultAnchorKind::PrimaryStructure);

    let fallback = select_personal_assault_anchor(
        player_id,
        AssaultHeroInfo {
            bound_monolith_id: None,
            ..hero
        },
        &spawn_positions,
        &[],
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(fallback.id, hero.id);
    assert_eq!(fallback.kind, AssaultAnchorKind::HeroFallback);

    assert!(select_personal_assault_anchor(
        player_id,
        AssaultHeroInfo {
            bound_monolith_id: None,
            ..hero
        },
        &SpawnPositions::default(),
        &[],
        &HashMap::new(),
    )
    .is_none());
}

#[test]
fn personal_assault_spawn_requires_passable_reachable_unoccupied_tiles() {
    let anchor = AssaultAnchor {
        id: 1,
        pos: Position { x: 25, y: 25 },
        kind: AssaultAnchorKind::BuiltStructure,
        sanctuary_level: None,
    };
    let land = flat_land_map();
    let positions = personal_assault_spawn_positions(
        1,
        anchor,
        GOBLIN_ASSAULT_COMPOSITION.len(),
        &HashSet::new(),
        &[],
        &[],
        &land,
    )
    .expect("flat land has valid assault positions");
    assert_eq!(positions.len(), GOBLIN_ASSAULT_COMPOSITION.len());
    assert_eq!(
        positions.iter().copied().collect::<HashSet<_>>().len(),
        GOBLIN_ASSAULT_COMPOSITION.len()
    );
    assert!(positions
        .iter()
        .all(|pos| Map::is_passable(pos.x, pos.y, &land)));

    let occupied = HashSet::from([positions[0]]);
    let neighbour = AssaultStructureInfo {
        id: 2,
        owner_player_id: 2,
        pos: positions[1],
        subclass: Subclass::Storage,
    };
    let constrained = personal_assault_spawn_positions(
        1,
        anchor,
        GOBLIN_ASSAULT_COMPOSITION.len(),
        &occupied,
        &[neighbour],
        &[],
        &land,
    )
    .expect("other valid ring tiles remain available");
    assert!(constrained.iter().all(|pos| !occupied.contains(pos)));
    assert!(constrained
        .iter()
        .all(|pos| Map::dist(*pos, neighbour.pos) >= 3));

    let neighbour_sanctuary = AssaultSanctuaryExclusion {
        owner_player_id: 2,
        pos: positions[0],
    };
    let sanctuary_constrained = personal_assault_spawn_positions(
        1,
        anchor,
        GOBLIN_ASSAULT_COMPOSITION.len(),
        &HashSet::new(),
        &[],
        &[neighbour_sanctuary],
        &land,
    )
    .expect("other valid ring tiles remain outside the neighbouring sanctuary");
    assert!(sanctuary_constrained.iter().all(|pos| {
        Map::dist(*pos, neighbour_sanctuary.pos) >= PERSONAL_ASSAULT_NEIGHBOUR_EXCLUSION_DISTANCE
    }));

    let every_ring_tile = (6..=8)
        .flat_map(|radius| Map::ring((anchor.pos.x, anchor.pos.y), radius))
        .map(|(x, y)| Position { x, y })
        .collect::<HashSet<_>>();
    assert!(personal_assault_spawn_positions(
        1,
        anchor,
        GOBLIN_ASSAULT_COMPOSITION.len(),
        &every_ring_tile,
        &[],
        &[],
        &land,
    )
    .is_none());

    let mut ocean = flat_land_map();
    for tile in &mut ocean.base {
        tile.tile_type = TileType::Ocean;
    }
    assert!(personal_assault_spawn_positions(
        1,
        anchor,
        GOBLIN_ASSAULT_COMPOSITION.len(),
        &HashSet::new(),
        &[],
        &[],
        &ocean,
    )
    .is_none());
}

#[test]
fn first_personal_goblin_composition_uses_only_existing_small_elite_templates() {
    assert_eq!(
        GOBLIN_ASSAULT_COMPOSITION,
        ["Wolf Rider", "Wolf Rider", "Goblin Pillager"]
    );
    let templates = load_obj_templates();
    for name in GOBLIN_ASSAULT_COMPOSITION {
        assert!(templates.iter().any(|template| template.template == name));
    }
    assert!(
        !templates
            .iter()
            .any(|template| template.template == "Goblin"),
        "the repository has no ordinary Goblin template"
    );
}

#[test]
fn checkpoint4_assault_spawn_commits_all_live_attributed_units() {
    fn spawn_once(
        mut commands: Commands,
        mut ids: ResMut<Ids>,
        mut entity_map: ResMut<EntityObjMap>,
        templates: Res<Templates>,
        mut run_spawned_objs: ResMut<RunSpawnedObjs>,
        mut ran: Local<bool>,
    ) {
        if *ran {
            return;
        }
        *ran = true;

        let unit_templates = GOBLIN_ASSAULT_COMPOSITION
            .iter()
            .map(|template| (*template).to_string())
            .collect::<Vec<_>>();
        let positions = vec![
            Position { x: 10, y: 10 },
            Position { x: 11, y: 10 },
            Position { x: 12, y: 10 },
        ];
        let spawned = spawn_goblin_assault(
            7,
            42,
            3,
            &unit_templates,
            &positions,
            &mut commands,
            &mut ids,
            &mut entity_map,
            &templates,
            &mut run_spawned_objs,
        )
        .expect("configured personal assault must spawn atomically");
        assert_eq!(spawned.len(), GOBLIN_ASSAULT_COMPOSITION.len());
    }

    let mut app = App::new();
    app.insert_resource(Ids::default());
    app.insert_resource(EntityObjMap(HashMap::new()));
    app.insert_resource(Templates::from_obj_templates(load_obj_templates()));
    app.insert_resource(RunSpawnedObjs::default());
    app.add_systems(Update, spawn_once);

    app.update();

    let mut query = app
        .world_mut()
        .query::<(&Template, &Position, &State, &CrisisAssaultUnit)>();
    let mut units = query
        .iter(app.world())
        .map(|(template, pos, state, attribution)| (template.0.clone(), *pos, *state, *attribution))
        .collect::<Vec<_>>();
    units.sort_by_key(|unit| (unit.1.x, unit.1.y));

    assert_eq!(units.len(), 3);
    assert_eq!(
        units.iter().map(|unit| unit.0.as_str()).collect::<Vec<_>>(),
        GOBLIN_ASSAULT_COMPOSITION
    );
    assert_eq!(
        units.iter().map(|unit| unit.1).collect::<Vec<_>>(),
        [
            Position { x: 10, y: 10 },
            Position { x: 11, y: 10 },
            Position { x: 12, y: 10 },
        ]
    );
    assert!(units.iter().all(|unit| unit.2 == State::None));
    assert!(units.iter().all(|unit| unit.3
        == CrisisAssaultUnit {
            owner_player_id: 7,
            assault_id: 42,
            spawn_generation: 3,
        }));
    assert_eq!(
        app.world()
            .resource::<RunSpawnedObjs>()
            .get(&7)
            .map(Vec::len),
        Some(3)
    );
}

fn personal_crisis_test_app() -> App {
    let mut app = App::new();
    app.add_systems(Update, personal_crisis_system);
    app.insert_resource(GameTick(100));
    app.insert_resource(Clients::default());
    app.insert_resource(PlayerIntroState::default());
    app.insert_resource(Objectives::default());
    app.insert_resource(SettlementCrisisState::default());
    app.insert_resource(CrisisTelemetryState::default());
    app.insert_resource(CrisisBalanceTelemetryState::default());
    app
}

#[test]
fn crisis_balance_sampler_records_authoritative_preparation_deltas() {
    let player_id = 77;
    let hero_id = 7_700;
    let monolith_id = 7_701;
    let damaged_wall_id = 7_702;
    let foundation_id = 7_703;
    let position = Position { x: 8, y: 8 };
    let stats = |hp, base_hp, base_damage| Stats {
        hp,
        stamina: Some(100),
        mana: None,
        base_hp,
        base_stamina: Some(100),
        base_mana: None,
        base_def: 0,
        damage_range: Some(1),
        base_damage: Some(base_damage),
        base_speed: Some(1),
        base_vision: Some(1),
    };

    let client_id = Uuid::from_u128(77);
    let (sender, _receiver) = tokio::sync::mpsc::channel(1);
    let clients = Clients::default();
    clients
        .lock()
        .unwrap()
        .insert(client_id, test_client(client_id, player_id, sender));

    let mut crises = SettlementCrisisState::default();
    crises.insert(
        player_id,
        SettlementCrisis {
            phase: CrisisPhase::Preparing,
            online_active_ticks: 100,
            ..SettlementCrisis::default()
        },
    );

    let mut app = App::new();
    app.insert_resource(GameTick(100));
    app.insert_resource(clients);
    app.insert_resource(PlayerWorldPresenceState::default());
    app.insert_resource(SpawnPositions(HashMap::from([(player_id, position)])));
    app.insert_resource(crises);
    app.insert_resource(CrisisBalanceTelemetryConfig {
        sample_interval_ticks: Some(1),
    });
    app.insert_resource(CrisisBalanceTelemetryState::default());
    app.insert_resource(CrisisBalanceObservationState::default());
    app.add_systems(Update, crisis_balance_snapshot_system);

    let hero = app
        .world_mut()
        .spawn((
            PlayerId(player_id),
            Id(hero_id),
            position,
            Template("Novice Warrior".to_string()),
            HeroClass::Warrior,
            stats(100, 110, 2),
            Inventory {
                owner: hero_id,
                items: Vec::new(),
            },
            BoundMonolith {
                id: monolith_id,
                pos: position,
            },
            State::None,
            SubclassHero,
        ))
        .id();
    let damaged_wall = app
        .world_mut()
        .spawn((
            PlayerId(player_id),
            Id(damaged_wall_id),
            position,
            Template("Stockade".to_string()),
            Subclass::Wall,
            State::None,
            stats(10, 20, 0),
            Inventory {
                owner: damaged_wall_id,
                items: Vec::new(),
            },
            ClassStructure,
        ))
        .id();
    let foundation = app
        .world_mut()
        .spawn((
            PlayerId(player_id),
            Id(foundation_id),
            position,
            Template("Stockade".to_string()),
            Subclass::Wall,
            State::Founded,
            stats(1, 20, 0),
            Inventory {
                owner: foundation_id,
                items: Vec::new(),
            },
            ClassStructure,
        ))
        .id();
    let monolith = app
        .world_mut()
        .spawn((
            Id(monolith_id),
            position,
            Monolith {
                soulshards: 0,
                sanctuary_level: 1,
            },
            State::None,
        ))
        .id();

    // First sample establishes the authoritative preparation baseline.
    app.update();

    // The second sample observes real ECS deltas. The sampler must not require
    // scenario labels or mutate gameplay state to classify these actions.
    *app.world_mut().get_mut::<State>(foundation).unwrap() = State::None;
    app.world_mut().get_mut::<Stats>(damaged_wall).unwrap().hp = 20;
    {
        let mut inventory = app.world_mut().get_mut::<Inventory>(hero).unwrap();
        inventory.items.push(checkpoint3_guidance_item(
            7_705,
            hero_id,
            "Crude Bandage",
            item::MEDICAL,
            "Bandage",
            false,
            None,
        ));
        inventory.items.push(checkpoint3_guidance_item(
            7_706,
            hero_id,
            "Training Bow",
            WEAPON,
            "Bow",
            true,
            None,
        ));
    }
    app.world_mut()
        .get_mut::<Monolith>(monolith)
        .unwrap()
        .sanctuary_level = 2;
    app.world_mut().spawn((
        PlayerId(player_id),
        Id(7_704),
        State::None,
        stats(100, 100, 1),
        Inventory {
            owner: 7_704,
            items: Vec::new(),
        },
        Assignment {
            structure_id: damaged_wall_id,
            structure_name: "Stockade".to_string(),
            structure_pos: position,
        },
        SubclassVillager,
    ));
    app.world_mut().resource_mut::<GameTick>().0 = 110;
    app.world_mut()
        .resource_mut::<SettlementCrisisState>()
        .get_mut(&player_id)
        .unwrap()
        .online_active_ticks = 110;
    app.update();

    let telemetry = app.world().resource::<CrisisBalanceTelemetryState>();
    let actions = &telemetry.get(&player_id).unwrap().preparation_actions;
    assert_eq!(actions.structures_built, 1);
    assert_eq!(actions.walls_built, 1);
    assert_eq!(actions.structures_repaired, 1);
    assert_eq!(actions.repairs_completed, 1);
    assert_eq!(actions.defensive_structures_completed, 1);
    assert_eq!(actions.equipment_changes, 1);
    assert_eq!(actions.healing_items_acquired, 1);
    assert_eq!(actions.villagers_recruited, 1);
    assert_eq!(actions.villager_assignments_changed, 1);
    assert_eq!(actions.sanctuary_upgrades, 1);
    assert_eq!(actions.first_preparation_action_tick, Some(110));
    assert_eq!(actions.meaningful_preparation_category_count, 6);
    assert_eq!(
        actions.meaningful_preparation_categories,
        [
            "defenses",
            "equipment",
            "healing",
            "repair",
            "sanctuary",
            "villager_support",
        ]
    );
    assert_eq!(actions.online_ticks_near_settlement, 10);
    assert_eq!(actions.online_ticks_away_from_settlement, 0);
    assert!(actions.performed_preparation_action);

    // Re-observing identical authoritative state must not inflate any action
    // or meaningful-category count.
    app.world_mut().resource_mut::<GameTick>().0 = 111;
    app.world_mut()
        .resource_mut::<SettlementCrisisState>()
        .get_mut(&player_id)
        .unwrap()
        .online_active_ticks = 111;
    app.update();
    {
        let telemetry = app.world().resource::<CrisisBalanceTelemetryState>();
        let actions = &telemetry.get(&player_id).unwrap().preparation_actions;
        assert_eq!(actions.structures_repaired, 1);
        assert_eq!(actions.defensive_structures_completed, 1);
        assert_eq!(actions.equipment_changes, 1);
        assert_eq!(actions.healing_items_acquired, 1);
        assert_eq!(actions.villagers_recruited, 1);
        assert_eq!(actions.villager_assignments_changed, 1);
        assert_eq!(actions.sanctuary_upgrades, 1);
        assert_eq!(actions.meaningful_preparation_category_count, 6);
    }

    // Returning after an away Signs warning is observable even before the
    // formal preparation phases. This must not be hidden behind action-count
    // eligibility for Preparing/AssaultReady.
    app.world_mut()
        .resource_mut::<CrisisBalanceTelemetryState>()
        .get_mut(&player_id)
        .unwrap()
        .warnings
        .record(CrisisPhase::Signs, 120, 110, true, false);
    *app.world_mut().get_mut::<Position>(hero).unwrap() = Position { x: 0, y: 0 };
    app.world_mut().resource_mut::<GameTick>().0 = 120;
    {
        let mut crises = app.world_mut().resource_mut::<SettlementCrisisState>();
        let crisis = crises.get_mut(&player_id).unwrap();
        crisis.phase = CrisisPhase::Signs;
        crisis.online_active_ticks = 120;
    }
    app.update();

    *app.world_mut().get_mut::<Position>(hero).unwrap() = position;
    app.world_mut()
        .get_mut::<Inventory>(hero)
        .unwrap()
        .items
        .push(checkpoint3_guidance_item(
            7_707,
            hero_id,
            "Spare Spear",
            WEAPON,
            "Spear",
            true,
            None,
        ));
    app.world_mut().resource_mut::<GameTick>().0 = 130;
    {
        let mut crises = app.world_mut().resource_mut::<SettlementCrisisState>();
        let crisis = crises.get_mut(&player_id).unwrap();
        crisis.phase = CrisisPhase::Pressure;
        crisis.online_active_ticks = 130;
    }
    app.update();
    assert!(
        app.world()
            .resource::<CrisisBalanceTelemetryState>()
            .get(&player_id)
            .unwrap()
            .preparation_actions
            .returned_to_settlement_after_warning
    );
    let telemetry = app.world().resource::<CrisisBalanceTelemetryState>();
    let actions = &telemetry.get(&player_id).unwrap().preparation_actions;
    assert_eq!(
        actions.equipment_changes, 1,
        "equipment changes outside Preparing/AssaultReady must be ignored"
    );
    assert_eq!(actions.meaningful_preparation_category_count, 6);

    // Establish an AssaultReady sample, then mutate equipment on the sample
    // that first observes AssaultActive. Launch readiness is a launch snapshot,
    // but the Active-side mutation must not be backdated into preparation.
    app.world_mut().resource_mut::<GameTick>().0 = 140;
    {
        let mut crises = app.world_mut().resource_mut::<SettlementCrisisState>();
        let crisis = crises.get_mut(&player_id).unwrap();
        crisis.phase = CrisisPhase::AssaultReady;
        crisis.online_active_ticks = 140;
    }
    app.update();

    app.world_mut()
        .get_mut::<Inventory>(hero)
        .unwrap()
        .items
        .push(checkpoint3_guidance_item(
            7_708,
            hero_id,
            "Launch-tick Axe",
            WEAPON,
            "Axe",
            true,
            None,
        ));
    app.world_mut().resource_mut::<GameTick>().0 = 150;
    {
        let mut crises = app.world_mut().resource_mut::<SettlementCrisisState>();
        let crisis = crises.get_mut(&player_id).unwrap();
        crisis.phase = CrisisPhase::AssaultActive;
        crisis.online_active_ticks = 150;
    }
    app.update();

    let telemetry = app.world().resource::<CrisisBalanceTelemetryState>();
    let actions = &telemetry.get(&player_id).unwrap().preparation_actions;
    assert_eq!(
        actions.equipment_changes, 1,
        "an Active-boundary state delta is not a preparation action"
    );
    assert_eq!(actions.healing_items_carried_at_launch, 1);
    assert_eq!(actions.combat_capable_villagers_at_launch, 1);
}

#[test]
fn personal_crisis_initialization_and_timing_require_a_live_online_human_run() {
    let player_id = 7;
    let mut app = personal_crisis_test_app();
    app.world_mut().resource_mut::<PlayerIntroState>().insert(
        player_id,
        PlayerIntroEntry {
            start_tick: 0,
            shipwreck_chain_started: true,
            villager_spawned: true,
            danger_unlocked: false,
        },
    );
    let hero = app
        .world_mut()
        .spawn((PlayerId(player_id), State::None, SubclassHero))
        .id();

    // A hero can remain in the ECS while its owner has no connected client.
    app.update();
    let crisis = app
        .world()
        .resource::<SettlementCrisisState>()
        .get(&player_id)
        .expect("personal crisis should initialize");
    assert_eq!(crisis.phase, CrisisPhase::Dormant);
    assert_eq!(crisis.online_active_ticks, 0);

    let client_id = Uuid::from_u128(7);
    let (sender, _receiver) = tokio::sync::mpsc::channel(1);
    app.world()
        .resource::<Clients>()
        .lock()
        .unwrap()
        .insert(client_id, test_client(client_id, player_id, sender));

    app.world_mut().resource_mut::<GameTick>().0 = 120;
    app.update();
    assert_eq!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .online_active_ticks,
        0,
        "online time must not advance before danger is unlocked"
    );

    app.world_mut()
        .resource_mut::<PlayerIntroState>()
        .get_mut(&player_id)
        .unwrap()
        .danger_unlocked = true;
    app.world_mut().resource_mut::<GameTick>().0 = 130;
    app.update();
    app.update();
    assert_eq!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .online_active_ticks,
        10,
        "repeated evaluation of one GameTick must not double-count"
    );
    {
        let crisis = app
            .world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap();
        let balance = app
            .world()
            .resource::<CrisisBalanceTelemetryState>()
            .get(&player_id)
            .unwrap();
        assert_eq!(balance.latest_pressure.clamped_total, crisis.pressure);
        assert_eq!(
            balance.latest_pressure.raw_total,
            balance.latest_pressure.contributor_sum()
        );
        assert_eq!(
            balance.latest_pressure.clamped_total,
            balance.latest_pressure.raw_total.min(GOBLIN_PRESSURE_MAX)
        );
    }

    {
        let mut crises = app.world_mut().resource_mut::<SettlementCrisisState>();
        let crisis = crises.get_mut(&player_id).unwrap();
        crisis.phase = CrisisPhase::Preparing;
        crisis.warning_active = true;
    }

    app.world()
        .resource::<Clients>()
        .lock()
        .unwrap()
        .remove(&client_id);
    app.world_mut().resource_mut::<GameTick>().0 = 200;
    app.update();
    assert_eq!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .online_active_ticks,
        10
    );
    assert!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .warning_active,
        "disconnect must not clear an active warning"
    );

    let (reconnect_sender, _reconnect_receiver) = tokio::sync::mpsc::channel(1);
    app.world().resource::<Clients>().lock().unwrap().insert(
        client_id,
        test_client(client_id, player_id, reconnect_sender),
    );
    app.world_mut().resource_mut::<GameTick>().0 = 210;
    app.update();
    assert_eq!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .online_active_ticks,
        20,
        "reconnect resumes from the new watermark, not the offline gap"
    );
    assert!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .warning_active,
        "reconnect must retain the Preparing warning"
    );

    app.world_mut().entity_mut(hero).insert(StateDead {
        dead_at: 210,
        killer: "test".to_string(),
    });
    app.world_mut().resource_mut::<GameTick>().0 = 250;
    app.update();
    assert_eq!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .online_active_ticks,
        20,
        "dead heroes do not accumulate crisis time"
    );

    app.world_mut().entity_mut(hero).remove::<StateDead>();
    app.world_mut().resource_mut::<GameTick>().0 = 260;
    app.update();
    assert_eq!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .online_active_ticks,
        30
    );

    app.world_mut().entity_mut(hero).insert(State::Dead);
    app.world_mut().resource_mut::<GameTick>().0 = 270;
    app.update();
    assert_eq!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .online_active_ticks,
        30,
        "logical dead state also blocks crisis time"
    );
    app.world_mut().entity_mut(hero).insert(State::None);
    app.world_mut().resource_mut::<GameTick>().0 = 280;
    app.update();

    app.world_mut()
        .entity_mut(hero)
        .insert(TrueDeath { true_death_at: 280 });
    app.world_mut().resource_mut::<GameTick>().0 = 290;
    app.update();
    assert_eq!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .online_active_ticks,
        40,
        "True Death never accumulates crisis time"
    );
    app.world_mut().entity_mut(hero).remove::<TrueDeath>();
    app.world_mut().resource_mut::<GameTick>().0 = 300;
    app.update();

    app.world_mut().despawn(hero);
    app.world_mut().resource_mut::<GameTick>().0 = 340;
    app.update();
    app.world_mut()
        .spawn((PlayerId(player_id), State::None, SubclassHero));
    app.world_mut().resource_mut::<GameTick>().0 = 350;
    app.update();
    assert_eq!(
        app.world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .unwrap()
            .online_active_ticks,
        60,
        "a missing-hero interval is not backfilled after recreation"
    );
}

#[test]
fn personal_crisis_does_not_initialize_for_npc_heroes() {
    let mut app = personal_crisis_test_app();
    app.world_mut()
        .spawn((PlayerId(NPC_PLAYER_ID), State::None, SubclassHero));
    app.update();
    assert!(app.world().resource::<SettlementCrisisState>().is_empty());
}

#[test]
fn crisis_tier_calculation_empty_state() {
    let crisis = PlayerCrisis::default();
    let tier = crisis_tier(&crisis);
    assert_eq!(tier, 0);
}

#[test]
fn crisis_tier_calculation_all_tiers() {
    let mut crisis = PlayerCrisis::default();
    assert_eq!(crisis_tier(&crisis), 0);

    crisis.rat_spoilage = true;
    assert_eq!(crisis_tier(&crisis), 1);

    crisis.wolf_pack = true;
    assert_eq!(crisis_tier(&crisis), 2);

    crisis.goblin_raid = true;
    assert_eq!(crisis_tier(&crisis), 3);

    crisis.undead_incursion = true;
    assert_eq!(crisis_tier(&crisis), 4);

    crisis.goblin_pillager = true;
    assert_eq!(crisis_tier(&crisis), 5);
}

#[test]
fn crisis_tier_skipped_tiers_reports_highest() {
    // If wolf_pack triggers but rat_spoilage didn't, tier should still be 2
    let mut crisis = PlayerCrisis::default();
    crisis.wolf_pack = true;
    assert_eq!(crisis_tier(&crisis), 2);

    // Undead incursion without goblin raid
    crisis.undead_incursion = true;
    assert_eq!(crisis_tier(&crisis), 4);
}

#[test]
fn crisis_bonus_xp_scales_with_tier() {
    for (tier, expected_bonus) in [(0, 0), (1, 1000), (3, 3000), (5, 5000)] {
        assert_eq!(tier * 1000, expected_bonus);
    }
}

#[test]
fn crisis_state_tracks_per_player() {
    let mut crisis_state = CrisisState::default();

    // Player 1 triggers tier 1 pest crisis
    crisis_state
        .entry(1)
        .or_insert_with(PlayerCrisis::default)
        .rat_spoilage = true;

    // Player 2 triggers wolf crisis
    crisis_state
        .entry(2)
        .or_insert_with(PlayerCrisis::default)
        .wolf_pack = true;

    assert_eq!(crisis_tier(crisis_state.get(&1).unwrap()), 1);
    assert_eq!(crisis_tier(crisis_state.get(&2).unwrap()), 2);
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
    // Novice Warrior (100 HP) vs Thorn Beetle (2 dmg + 2 range = 4 max)
    // Warrior survives at least 100/4 = 25 hits
    let warrior_hp = 100;
    let thorn_beetle_max_dmg = 2 + 2;
    let hits_to_kill_warrior = warrior_hp / thorn_beetle_max_dmg;
    assert!(
        hits_to_kill_warrior >= 10,
        "Warrior should survive 10+ tier 1 enemy hits, got {}",
        hits_to_kill_warrior
    );
}

#[test]
fn hero_can_kill_tier1_creatures_quickly() {
    // Novice Warrior (2 dmg, 2 range) + Copper Axe (+11 dmg)
    // Avg damage: 2 + 1 + 11 = 14. Thorn Beetle HP: 24
    let hero_avg_dmg = 2 + 1 + 11;
    let thorn_beetle_hp = 24;
    let hits_to_kill_tier1_enemy = (thorn_beetle_hp as f64 / hero_avg_dmg as f64).ceil() as i32;
    assert!(
        hits_to_kill_tier1_enemy <= 3,
        "Hero should kill T1 enemies in 3 or fewer hits, got {}",
        hits_to_kill_tier1_enemy
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
    // Tier 4 triggers after 3 player-survival days; Tier 5 after 5.
    let tier4_trigger = DAWN + UNDEAD_INCURSION_SURVIVAL_TICKS;
    let tier5_trigger = DAWN + GOBLIN_PILLAGER_SURVIVAL_TICKS;
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
fn survival_director_starts_after_day_eight_or_objective() {
    // Heavy scaling hordes hold off until day 8 (days 6-7 stay on the gentle ramp),
    // widening the early calm window for banking a food reserve.
    assert!(!survival_director_active(7, None));
    assert!(survival_director_active(8, None));

    let objectives = PlayerObjectives {
        survive_5_nights: true,
        ..Default::default()
    };
    assert!(survival_director_active(4, Some(&objectives)));
}

#[test]
fn player_survival_day_uses_player_join_tick() {
    let join_tick = DAWN + (GAME_TICKS_PER_DAY * 5);
    let mut intro_state = PlayerIntroState(HashMap::new());
    intro_state.insert(
        7,
        PlayerIntroEntry {
            start_tick: join_tick,
            shipwreck_chain_started: false,
            villager_spawned: false,
            danger_unlocked: false,
        },
    );

    assert_eq!(GameTick(join_tick).day(), 6);
    assert_eq!(
        player_survival_day(&GameTick(join_tick), 7, &intro_state),
        1
    );
    assert_eq!(
        player_survival_day(
            &GameTick(join_tick + (GAME_TICKS_PER_DAY * 5)),
            7,
            &intro_state,
        ),
        LEGENDARY_RUMOR_DAY,
    );
    assert_eq!(
        player_days_survived(
            &GameTick(join_tick + (GAME_TICKS_PER_DAY * 5)),
            7,
            &intro_state,
        ),
        5,
    );
}

#[test]
fn timed_crisis_gates_use_player_survival_ticks() {
    let join_tick = DAWN + (GAME_TICKS_PER_DAY * 5);
    let mut intro_state = PlayerIntroState(HashMap::new());
    intro_state.insert(
        7,
        PlayerIntroEntry {
            start_tick: join_tick,
            shipwreck_chain_started: false,
            villager_spawned: false,
            danger_unlocked: false,
        },
    );

    assert_eq!(GameTick(join_tick).day(), 6);
    assert_eq!(
        player_survival_ticks(&GameTick(join_tick), 7, &intro_state),
        0
    );
    assert!(
        player_survival_ticks(
            &GameTick(join_tick + UNDEAD_INCURSION_SURVIVAL_TICKS - 10),
            7,
            &intro_state,
        ) < UNDEAD_INCURSION_SURVIVAL_TICKS
    );
    assert!(
        player_survival_ticks(
            &GameTick(join_tick + UNDEAD_INCURSION_SURVIVAL_TICKS),
            7,
            &intro_state,
        ) >= UNDEAD_INCURSION_SURVIVAL_TICKS
    );
    assert!(
        player_survival_ticks(
            &GameTick(join_tick + GOBLIN_PILLAGER_SURVIVAL_TICKS),
            7,
            &intro_state,
        ) >= GOBLIN_PILLAGER_SURVIVAL_TICKS
    );
}

#[test]
fn atmospheric_messages_use_player_day() {
    assert!(atmospheric_event_message(1, 700).is_some());
    assert_eq!(atmospheric_event_message(6, 700), None);
    assert!(atmospheric_event_message(7, 650).is_some());
}

#[test]
fn rescue_victory_uses_player_survival_day() {
    let join_tick = DAWN + (GAME_TICKS_PER_DAY * 10);
    let mut intro_state = PlayerIntroState(HashMap::new());
    intro_state.insert(
        7,
        PlayerIntroEntry {
            start_tick: join_tick,
            shipwreck_chain_started: false,
            villager_spawned: false,
            danger_unlocked: false,
        },
    );

    let victory = PlayerVictory::default();
    assert_eq!(GameTick(join_tick).day(), 11);
    assert!(!rescue_victory_ready(
        player_survival_day(&GameTick(join_tick), 7, &intro_state),
        &victory,
    ));
    assert!(rescue_victory_ready(
        player_survival_day(
            &GameTick(join_tick + (GAME_TICKS_PER_DAY * 50)),
            7,
            &intro_state,
        ),
        &victory,
    ));

    let already_rescued = PlayerVictory {
        rescue_progress: 1,
        ..Default::default()
    };
    assert!(!rescue_victory_ready(51, &already_rescued));
}

#[test]
fn shipwreck_inspection_triggers_villager_only_after_help_speech() {
    let entry = InitialEncounterEntry {
        rat_ids: vec![1, 2],
        opening_enemy_templates: vec!["Cave Bat".to_string(), "Thorn Beetle".to_string()],
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
        necromancer_id: 0,
        mausoleum_id: 0,
        necro_spawn_anchor: Position { x: 0, y: 0 },
        necro_corpse_anchor: Position { x: 0, y: 0 },
        necro_home: Position { x: 0, y: 0 },
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

fn checkpoint4_crisis(phase: CrisisPhase, pressure: i32) -> SettlementCrisis {
    let mut crisis = SettlementCrisis::new(100);
    crisis.phase = phase;
    crisis.pressure = pressure;
    crisis.warning_active = matches!(
        phase,
        CrisisPhase::Preparing | CrisisPhase::AssaultReady | CrisisPhase::AssaultActive
    );
    crisis
}

fn checkpoint3_guidance_item(
    id: i32,
    owner: i32,
    name: &str,
    class: &str,
    subclass: &str,
    equipped: bool,
    healing: Option<f32>,
) -> Item {
    let mut item = consumable_item(
        id,
        owner,
        name,
        class,
        AttrKey::Healing,
        healing.unwrap_or(0.0),
    );
    item.subclass = subclass.to_string();
    item.equipped = equipped;
    if healing.is_none() {
        item.attrs.clear();
    }
    item
}

fn checkpoint3_guidance_stats(hp: i32, base_hp: i32, base_damage: i32) -> Stats {
    Stats {
        hp,
        stamina: Some(100),
        mana: None,
        base_hp,
        base_stamina: Some(100),
        base_mana: None,
        base_def: 0,
        damage_range: Some(0),
        base_damage: Some(base_damage),
        base_speed: Some(10),
        base_vision: Some(3),
    }
}

#[test]
fn checkpoint3_preparation_options_have_fixed_order_states_and_cap() {
    let facts = CrisisPreparationFacts {
        completed_walls: 2,
        damaged_walls: 1,
        living_villagers: 1,
        combat_capable_villagers: 1,
        live_hero: true,
        hero_idle: true,
        hero_equipped_weapon: Some("Training Bow".to_string()),
        hero_equipped_armor: 1,
        hero_carried_healing: 1,
        ..CrisisPreparationFacts::default()
    };

    let options = derive_crisis_preparation_options(&facts);
    assert_eq!(options.len(), 4);
    assert_eq!(
        options
            .iter()
            .map(|option| option.id.as_str())
            .collect::<Vec<_>>(),
        vec!["defences", "defenders", "equipment", "recovery"]
    );
    assert_eq!(
        options
            .iter()
            .map(|option| option.state.as_str())
            .collect::<Vec<_>>(),
        vec!["needs_attention", "ready", "ready", "ready"]
    );
    assert!(options.iter().all(|option| matches!(
        option.state.as_str(),
        "ready" | "needs_attention" | "unavailable"
    )));

    let buildable = CrisisPreparationFacts {
        live_hero: true,
        hero_idle: true,
        stockade_plan_available: true,
        stockade_log_units_carried: 3,
        can_start_stockade: true,
        ..CrisisPreparationFacts::default()
    };
    let buildable = derive_crisis_preparation_options(&buildable);
    assert_eq!(buildable[0].state, "needs_attention");
    assert!(buildable[0].detail.contains("Stockade plan"));

    let occupied_foundation_tile = CrisisPreparationFacts {
        live_hero: true,
        hero_idle: true,
        current_tile_wall_present: true,
        stockade_plan_available: true,
        stockade_log_units_carried: 3,
        can_start_stockade: false,
        ..CrisisPreparationFacts::default()
    };
    let occupied_foundation_tile = derive_crisis_preparation_options(&occupied_foundation_tile);
    assert_eq!(occupied_foundation_tile[0].state, "unavailable");
    assert!(occupied_foundation_tile[0]
        .action_hint
        .contains("without an existing wall"));
}

#[test]
fn checkpoint3_recovery_uses_actual_usable_item_semantics() {
    let bandage = checkpoint3_guidance_item(
        1,
        10,
        "Crude Bandage",
        item::MEDICAL,
        "Bandage",
        false,
        None,
    );
    let potion = checkpoint3_guidance_item(
        2,
        10,
        "Health Potion",
        item::POTION,
        item::HEALTH,
        false,
        Some(10.0),
    );
    let zero_heal_potion = checkpoint3_guidance_item(
        3,
        10,
        "Empty Potion",
        item::POTION,
        item::HEALTH,
        false,
        Some(0.0),
    );
    let healing_food = checkpoint3_guidance_item(4, 10, "Stew", FOOD, "Stew", false, Some(10.0));

    assert!(is_usable_crisis_healing_item(&bandage));
    assert!(is_usable_crisis_healing_item(&potion));
    assert!(!is_usable_crisis_healing_item(&zero_heal_potion));
    assert!(!is_usable_crisis_healing_item(&healing_food));

    let mut empty_bandage = bandage;
    empty_bandage.quantity = 0;
    assert!(!is_usable_crisis_healing_item(&empty_bandage));
}

#[test]
fn checkpoint3_status_builder_is_phase_gated_read_only_and_structural() {
    let facts = CrisisPreparationFacts {
        live_hero: true,
        hero_carried_healing: 1,
        ..CrisisPreparationFacts::default()
    };
    assert!(
        build_crisis_status_with_preparation(None, Some(&facts))
            .preparation_options
            .is_none(),
        "a no-crisis clear snapshot must never expose preparation rows"
    );
    for phase in [
        CrisisPhase::Dormant,
        CrisisPhase::Signs,
        CrisisPhase::Pressure,
        CrisisPhase::AssaultActive,
        CrisisPhase::Resolved,
    ] {
        let crisis = checkpoint4_crisis(phase, 70);
        assert!(
            build_crisis_status_with_preparation(Some(&crisis), Some(&facts))
                .preparation_options
                .is_none()
        );
    }

    for phase in [CrisisPhase::Preparing, CrisisPhase::AssaultReady] {
        let crisis = checkpoint4_crisis(phase, 70);
        let crisis_before = crisis.clone();
        let facts_before = facts.clone();
        let status = build_crisis_status_with_preparation(Some(&crisis), Some(&facts));
        assert_eq!(status.preparation_options.as_ref().map(Vec::len), Some(4));
        assert_eq!(crisis, crisis_before);
        assert_eq!(facts, facts_before);
    }

    let crisis = checkpoint4_crisis(CrisisPhase::Preparing, 70);
    let baseline = build_crisis_status_with_preparation(Some(&crisis), Some(&facts));
    assert!(
        !crisis_status_changed(&baseline, &baseline),
        "unchanged preparation rows must retain packet deduplication"
    );
    let mut changed_facts = facts;
    changed_facts.hero_carried_healing = 0;
    let changed = build_crisis_status_with_preparation(Some(&crisis), Some(&changed_facts));
    assert!(crisis_status_changed(&baseline, &changed));
}

#[derive(Resource)]
struct Checkpoint3GuidanceOwner(i32);

#[derive(Resource, Default)]
struct Checkpoint3CapturedFacts(Option<CrisisPreparationFacts>);

fn checkpoint3_capture_preparation_facts(
    owner: Res<Checkpoint3GuidanceOwner>,
    collector: CrisisPreparationCollector,
    mut captured: ResMut<Checkpoint3CapturedFacts>,
) {
    captured.0 = Some(collector.collect(owner.0));
}

#[test]
fn checkpoint3_preparation_collector_is_owner_exact_and_non_mutating() {
    let player_id = 7;
    let mut app = App::new();
    app.insert_resource(Checkpoint3GuidanceOwner(player_id));
    app.insert_resource(Checkpoint3CapturedFacts::default());
    let mut ids = Ids::default();
    ids.new_hero(10, player_id);
    ids.new_hero(20, 8);
    app.insert_resource(ids);
    app.add_systems(Update, checkpoint3_capture_preparation_facts);

    let weapon = checkpoint3_guidance_item(10, 10, "Training Bow", WEAPON, "Bow", true, None);
    let bandage = checkpoint3_guidance_item(
        11,
        10,
        "Crude Bandage",
        item::MEDICAL,
        "Bandage",
        false,
        None,
    );
    let healing_food = checkpoint3_guidance_item(12, 10, "Stew", FOOD, "Stew", false, Some(10.0));
    let hero = app
        .world_mut()
        .spawn((
            PlayerId(player_id),
            Id(10),
            Position { x: 5, y: 5 },
            Template("Novice Ranger".to_string()),
            State::None,
            checkpoint3_guidance_stats(100, 100, 0),
            Inventory {
                owner: 10,
                items: vec![weapon, bandage, healing_food],
            },
            SubclassHero,
        ))
        .id();

    app.world_mut().spawn((
        PlayerId(player_id),
        State::None,
        checkpoint3_guidance_stats(500, 500, 1),
        Inventory {
            owner: 11,
            items: vec![],
        },
        SubclassVillager,
    ));
    app.world_mut().spawn((
        PlayerId(player_id),
        Position { x: 6, y: 5 },
        Subclass::Wall,
        State::None,
        checkpoint3_guidance_stats(10, 20, 0),
        Inventory {
            owner: 12,
            items: vec![],
        },
        ClassStructure,
    ));

    // Other-player facts must not leak into the requested owner's guidance.
    app.world_mut().spawn((
        PlayerId(8),
        State::None,
        checkpoint3_guidance_stats(500, 500, 1),
        Inventory {
            owner: 21,
            items: vec![],
        },
        SubclassVillager,
    ));
    app.world_mut().spawn((
        PlayerId(8),
        Position { x: 5, y: 5 },
        Subclass::Wall,
        State::None,
        checkpoint3_guidance_stats(200, 200, 0),
        Inventory {
            owner: 22,
            items: vec![],
        },
        ClassStructure,
    ));
    app.world_mut().spawn((
        PlayerId(8),
        Position { x: 5, y: 5 },
        Subclass::Storage,
        State::None,
        checkpoint3_guidance_stats(100, 100, 0),
        Inventory {
            owner: 23,
            items: vec![checkpoint3_guidance_item(
                20,
                23,
                "Health Potion",
                item::POTION,
                item::HEALTH,
                false,
                Some(50.0),
            )],
        },
        ClassStructure,
    ));

    let before_hp = app.world().entity(hero).get::<Stats>().unwrap().hp;
    let before_items = app
        .world()
        .entity(hero)
        .get::<Inventory>()
        .unwrap()
        .items
        .iter()
        .map(|item| (item.name.clone(), item.quantity, item.equipped))
        .collect::<Vec<_>>();

    app.update();

    let facts = app
        .world()
        .resource::<Checkpoint3CapturedFacts>()
        .0
        .as_ref()
        .unwrap();
    assert_eq!(facts.completed_walls, 1);
    assert_eq!(facts.damaged_walls, 1);
    assert!(
        facts.current_tile_wall_present,
        "the placement blocker must include an unfinished or other-player wall on the hero tile"
    );
    assert_eq!(facts.living_villagers, 1);
    assert_eq!(facts.combat_capable_villagers, 1);
    assert_eq!(facts.hero_equipped_weapon.as_deref(), Some("Training Bow"));
    assert_eq!(facts.hero_carried_healing, 1, "food must not count");
    assert_eq!(
        facts.stored_healing, 0,
        "other-player storage must not leak"
    );

    assert_eq!(
        app.world().entity(hero).get::<Stats>().unwrap().hp,
        before_hp
    );
    assert_eq!(
        app.world()
            .entity(hero)
            .get::<Inventory>()
            .unwrap()
            .items
            .iter()
            .map(|item| (item.name.clone(), item.quantity, item.equipped))
            .collect::<Vec<_>>(),
        before_items
    );
}

#[test]
fn checkpoint4_no_crisis_builds_an_explicit_clear_snapshot() {
    let status = build_crisis_status(None);

    assert_eq!(status.version, 1);
    assert!(!status.exists);
    assert_eq!(status.kind, None);
    assert_eq!(status.phase, None);
    assert_eq!(status.pressure, None);
    assert_eq!(status.pressure_max, None);
    assert!(!status.warning);
    assert!(!status.assault_active);
    assert!(!status.continues_while_disconnected);
}

#[test]
fn checkpoint4_every_phase_has_a_stable_machine_value_and_severity() {
    for (phase, machine_phase, severity) in [
        (CrisisPhase::Dormant, "dormant", "quiet"),
        (CrisisPhase::Signs, "signs", "low"),
        (CrisisPhase::Pressure, "pressure", "medium"),
        (CrisisPhase::Preparing, "preparing", "high"),
        (CrisisPhase::AssaultReady, "assault_ready", "crisis"),
        (CrisisPhase::AssaultActive, "assault_active", "crisis"),
        (CrisisPhase::Resolved, "resolved", "resolved"),
    ] {
        let crisis = checkpoint4_crisis(phase, 73);
        let status = build_crisis_status(Some(&crisis));

        assert!(status.exists);
        assert_eq!(status.kind.as_deref(), Some("goblin"));
        assert_eq!(status.phase.as_deref(), Some(machine_phase));
        assert_eq!(status.severity.as_deref(), Some(severity));
        assert!(status
            .title
            .as_deref()
            .is_some_and(|title| !title.is_empty()));
        assert!(status
            .summary
            .as_deref()
            .is_some_and(|summary| !summary.is_empty()));
        assert!(status
            .action_hint
            .as_deref()
            .is_some_and(|hint| !hint.is_empty()));
    }
}

#[test]
fn checkpoint4_status_mapping_is_read_only_and_uses_server_pressure_max() {
    let crisis = checkpoint4_crisis(CrisisPhase::Pressure, 67);
    let before = crisis.clone();

    let status = build_crisis_status(Some(&crisis));

    assert_eq!(crisis, before);
    assert_eq!(status.pressure, Some(67));
    assert_eq!(status.pressure_max, Some(GOBLIN_PRESSURE_MAX));
}

#[test]
fn checkpoint4_preparing_ready_active_and_resolved_fields_are_authoritative() {
    let preparing = build_crisis_status(Some(&checkpoint4_crisis(CrisisPhase::Preparing, 80)));
    assert!(preparing.warning);

    let mut ready_crisis = checkpoint4_crisis(CrisisPhase::AssaultReady, 92);
    ready_crisis.phase_online_ticks = 70;
    let ready = build_crisis_status(Some(&ready_crisis));
    assert!(ready.assault_ready);
    assert_eq!(ready.preparation_seconds_remaining, Some(23));
    assert_eq!(
        ready.summary.as_deref(),
        Some(
            "The raiders are ready. After the minimum warning, they favor dusk or night but will not wait indefinitely."
        )
    );
    assert_eq!(
        ready.preferred_launch_window.as_deref(),
        Some("dusk_or_night")
    );

    let mut active_crisis = checkpoint4_crisis(CrisisPhase::AssaultActive, 96);
    active_crisis.assault_unit_ids = vec![11, 12, 13];
    active_crisis.assault_defeated_unit_ids = vec![11];
    let active = build_crisis_status(Some(&active_crisis));
    assert!(active.assault_active);
    assert_eq!(active.remaining_attackers, Some(2));
    assert_eq!(active.total_attackers, Some(3));
    assert!(active.continues_while_disconnected);

    let resolved = build_crisis_status(Some(&checkpoint4_crisis(CrisisPhase::Resolved, 96)));
    assert!(resolved.resolved);
    assert!(!resolved.warning);
    assert!(!resolved.continues_while_disconnected);
    assert_eq!(resolved.remaining_attackers, None);
}

#[test]
fn checkpoint4_status_change_policy_throttles_only_pressure_and_countdown() {
    let mut crisis = checkpoint4_crisis(CrisisPhase::Pressure, 50);
    let baseline = build_crisis_status(Some(&crisis));
    assert!(!crisis_status_changed(&baseline, &baseline));

    crisis.pressure = 54;
    assert!(!crisis_status_changed(
        &baseline,
        &build_crisis_status(Some(&crisis))
    ));
    crisis.pressure = 55;
    assert!(crisis_status_changed(
        &baseline,
        &build_crisis_status(Some(&crisis))
    ));

    let mut transitioned = crisis.clone();
    transitioned.phase = CrisisPhase::Preparing;
    transitioned.warning_active = true;
    assert!(crisis_status_changed(
        &baseline,
        &build_crisis_status(Some(&transitioned))
    ));

    let mut ready = checkpoint4_crisis(CrisisPhase::AssaultReady, 90);
    ready.phase_online_ticks = 0;
    let countdown = build_crisis_status(Some(&ready));
    ready.phase_online_ticks = 40;
    assert!(!crisis_status_changed(
        &countdown,
        &build_crisis_status(Some(&ready))
    ));
    ready.phase_online_ticks = 50;
    assert!(crisis_status_changed(
        &countdown,
        &build_crisis_status(Some(&ready))
    ));

    let mut active = checkpoint4_crisis(CrisisPhase::AssaultActive, 90);
    active.assault_unit_ids = vec![1, 2, 3];
    let all_alive = build_crisis_status(Some(&active));
    active.assault_defeated_unit_ids.push(1);
    assert!(crisis_status_changed(
        &all_alive,
        &build_crisis_status(Some(&active))
    ));

    assert!(crisis_status_changed(&baseline, &build_crisis_status(None)));
}

#[test]
fn checkpoint4_delivery_deduplicates_and_resynchronizes_each_connection() {
    let player_id = 7;
    let client_id = Uuid::new_v4();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(16);
    let mut client_map = HashMap::new();
    client_map.insert(
        client_id,
        Client {
            id: client_id,
            player_id,
            sender,
        },
    );

    let mut crisis_state = SettlementCrisisState::default();
    crisis_state.insert(player_id, checkpoint4_crisis(CrisisPhase::Dormant, 10));
    let mut login_sync = CrisisStatusLoginSync::default();
    login_sync.insert(player_id);
    let mut telemetry_state = CrisisTelemetryState::default();
    telemetry_state.insert(player_id, CrisisTelemetry::new(100));

    let clients = Clients(Arc::new(Mutex::new(client_map)));
    let mut app = App::new();
    app.insert_resource(GameTick(100));
    app.insert_resource(clients.clone());
    app.insert_resource(SurvivalDirectorConfig::default());
    app.insert_resource(crisis_state);
    app.insert_resource(login_sync);
    app.insert_resource(CrisisStatusDeliveryState::default());
    app.insert_resource(telemetry_state);
    app.insert_resource(CrisisBalanceTelemetryState::default());
    app.insert_resource(ResumeLoginSyncState::default());
    app.insert_resource(SafeLogoutTelemetryState::default());
    app.add_systems(Update, crisis_status_delivery_system);

    app.update();
    let first = receiver.try_recv().expect("login snapshot");
    let first: ResponsePacket = serde_json::from_str(&first).unwrap();
    assert!(matches!(
        first,
        ResponsePacket::CrisisStatus {
            status: CrisisStatusSnapshot { exists: true, .. }
        }
    ));
    assert!(receiver.try_recv().is_err());

    app.update();
    assert!(
        receiver.try_recv().is_err(),
        "unchanged snapshot must dedupe"
    );

    // A duplicate delayed Login for the same authenticated connection does not
    // force another identical packet.
    app.world_mut()
        .resource_mut::<CrisisStatusLoginSync>()
        .insert(player_id);
    app.update();
    assert!(receiver.try_recv().is_err());

    app.world_mut()
        .resource_mut::<SettlementCrisisState>()
        .get_mut(&player_id)
        .unwrap()
        .pressure = 15;
    app.update();
    assert!(receiver.try_recv().is_ok(), "meaningful pressure sends");

    // Removing the per-run state sends a clear snapshot on the existing
    // connection, as required by True Death and fresh-run cleanup.
    app.world_mut()
        .resource_mut::<SettlementCrisisState>()
        .remove(&player_id);
    app.update();
    let clear = receiver.try_recv().expect("clear snapshot");
    let clear: ResponsePacket = serde_json::from_str(&clear).unwrap();
    assert!(matches!(
        clear,
        ResponsePacket::CrisisStatus {
            status: CrisisStatusSnapshot { exists: false, .. }
        }
    ));

    // A real offline update purges the connection cache. Reusing the same UUID
    // in this deterministic test therefore still receives one reconnect sync.
    clients.lock().unwrap().remove(&client_id);
    app.update();
    let (reconnect_sender, mut reconnect_receiver) = tokio::sync::mpsc::channel(4);
    clients.lock().unwrap().insert(
        client_id,
        Client {
            id: client_id,
            player_id,
            sender: reconnect_sender,
        },
    );
    app.world_mut()
        .resource_mut::<CrisisStatusLoginSync>()
        .insert(player_id);
    app.update();
    assert!(reconnect_receiver.try_recv().is_ok());

    let telemetry = app.world().resource::<CrisisTelemetryState>();
    let telemetry = telemetry.get(&player_id).unwrap();
    assert_eq!(telemetry.status_packets_sent, 4);
    assert_eq!(telemetry.login_snapshots_sent, 2);
}

#[test]
fn checkpoint4_login_sync_bundle_is_atomic_and_connection_exact() {
    let player_id = 70;
    let displaced_id = Uuid::new_v4();
    let replacement_id = Uuid::new_v4();
    let clients = Clients::default();
    let (displaced_sender, mut displaced_receiver) = tokio::sync::mpsc::channel(1);
    clients.activate(Client {
        id: displaced_id,
        player_id,
        sender: displaced_sender,
    });

    assert_eq!(
        clients.try_send_current_bundle(
            player_id,
            displaced_id,
            vec!["explored".to_string(), "world".to_string()],
        ),
        Err(CurrentConnectionSendError::Full),
        "capacity must be reserved for the entire ordered login bundle"
    );
    assert!(
        displaced_receiver.try_recv().is_err(),
        "a partial login bundle must never be queued"
    );

    let (replacement_sender, mut replacement_receiver) = tokio::sync::mpsc::channel(4);
    assert_eq!(
        clients.activate(Client {
            id: replacement_id,
            player_id,
            sender: replacement_sender,
        }),
        vec![displaced_id]
    );
    assert_eq!(
        clients.try_send_current_bundle(player_id, displaced_id, vec!["stale".to_string()]),
        Err(CurrentConnectionSendError::NotCurrent)
    );
    assert!(displaced_receiver.try_recv().is_err());

    assert_eq!(
        clients.try_send_current_bundle(
            player_id,
            replacement_id,
            vec!["explored".to_string(), "world".to_string()],
        ),
        Ok(())
    );
    assert_eq!(replacement_receiver.try_recv().unwrap(), "explored");
    assert_eq!(replacement_receiver.try_recv().unwrap(), "world");
    assert!(replacement_receiver.try_recv().is_err());
}

fn checkpoint4_resume_sync_test_app(
    player_id: i32,
    connection_id: Uuid,
    clients: Clients,
    progress: ResumeLoginSyncProgress,
) -> App {
    let mut record = crate::safe_logout::PlayerPresenceRecord::new(true);
    record.state = crate::safe_logout::PlayerWorldPresence::Online;
    record.resume_in_progress = true;
    record.resume_connection_id = Some(connection_id);
    let mut presence = PlayerWorldPresenceState::default();
    presence.players.insert(player_id, record);

    let mut sync = ResumeLoginSyncState::default();
    sync.insert(player_id, progress);

    let mut app = App::new();
    app.insert_resource(clients);
    app.insert_resource(GameTick(100));
    app.insert_resource(presence);
    app.insert_resource(sync);
    app.insert_resource(SafeLogoutTelemetryState::default());
    app.add_systems(Update, resume_login_sync_completion_system);
    app
}

#[test]
fn checkpoint4_resume_sync_waits_for_crisis_and_perception_delivery() {
    let player_id = 71;
    let connection_id = Uuid::new_v4();
    let clients = Clients::default();
    let (sender, _receiver) = tokio::sync::mpsc::channel(4);
    clients.activate(Client {
        id: connection_id,
        player_id,
        sender,
    });
    let mut app = checkpoint4_resume_sync_test_app(
        player_id,
        connection_id,
        clients,
        ResumeLoginSyncProgress {
            connection_id,
            crisis_status_queued: false,
            perception_queued: true,
        },
    );

    app.update();
    assert!(
        !app.world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&player_id)
            .unwrap()
            .resume_sync_ready
    );

    app.world_mut()
        .resource_mut::<ResumeLoginSyncState>()
        .get_mut(&player_id)
        .unwrap()
        .crisis_status_queued = true;
    app.update();
    assert!(
        app.world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&player_id)
            .unwrap()
            .resume_sync_ready
    );
    assert!(app.world().resource::<ResumeLoginSyncState>().is_empty());
}

#[test]
fn checkpoint4_resume_sync_rejects_authority_replaced_before_release() {
    let player_id = 72;
    let displaced_id = Uuid::new_v4();
    let replacement_id = Uuid::new_v4();
    let clients = Clients::default();
    let (displaced_sender, _displaced_receiver) = tokio::sync::mpsc::channel(4);
    clients.activate(Client {
        id: displaced_id,
        player_id,
        sender: displaced_sender,
    });
    let mut app = checkpoint4_resume_sync_test_app(
        player_id,
        displaced_id,
        clients.clone(),
        ResumeLoginSyncProgress {
            connection_id: displaced_id,
            crisis_status_queued: true,
            perception_queued: true,
        },
    );

    let (replacement_sender, _replacement_receiver) = tokio::sync::mpsc::channel(4);
    clients.activate(Client {
        id: replacement_id,
        player_id,
        sender: replacement_sender,
    });
    app.update();

    assert!(
        !app.world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&player_id)
            .unwrap()
            .resume_sync_ready
    );
    assert!(app.world().resource::<ResumeLoginSyncState>().is_empty());
    assert_eq!(
        app.world()
            .resource::<SafeLogoutTelemetryState>()
            .get(&player_id)
            .unwrap()
            .stale_connection_events_rejected,
        1
    );
}

#[test]
fn checkpoint4_major_transition_notices_emit_once() {
    let player_id = 8;
    let client_id = Uuid::new_v4();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
    let clients = Clients(Arc::new(Mutex::new(HashMap::from([(
        client_id,
        Client {
            id: client_id,
            player_id,
            sender,
        },
    )]))));
    let mut crisis_state = SettlementCrisisState::default();
    crisis_state.insert(player_id, checkpoint4_crisis(CrisisPhase::Pressure, 70));
    let mut login_sync = CrisisStatusLoginSync::default();
    login_sync.insert(player_id);

    let mut app = App::new();
    app.insert_resource(GameTick(100));
    app.insert_resource(clients);
    app.insert_resource(SurvivalDirectorConfig::default());
    app.insert_resource(crisis_state);
    app.insert_resource(login_sync);
    app.insert_resource(CrisisStatusDeliveryState::default());
    app.insert_resource(CrisisTelemetryState::default());
    app.insert_resource(CrisisBalanceTelemetryState::default());
    app.insert_resource(ResumeLoginSyncState::default());
    app.insert_resource(SafeLogoutTelemetryState::default());
    app.add_systems(Update, crisis_status_delivery_system);

    app.update();
    let initial: ResponsePacket =
        serde_json::from_str(&receiver.try_recv().expect("initial status")).unwrap();
    assert!(matches!(initial, ResponsePacket::CrisisStatus { .. }));
    assert!(receiver.try_recv().is_err());

    let transitions = [
        (
            CrisisPhase::Preparing,
            "Goblin raiders are gathering. Prepare your settlement.",
        ),
        (CrisisPhase::AssaultReady, "A goblin raid is imminent."),
        (
            CrisisPhase::AssaultActive,
            "The goblin assault has begun. It will continue if you disconnect.",
        ),
        (
            CrisisPhase::Resolved,
            "The goblin assault has been defeated.",
        ),
    ];

    for (phase, expected_notice) in transitions {
        {
            let mut crises = app.world_mut().resource_mut::<SettlementCrisisState>();
            let crisis = crises.get_mut(&player_id).unwrap();
            crisis.phase = phase;
            crisis.warning_active = matches!(
                phase,
                CrisisPhase::Preparing | CrisisPhase::AssaultReady | CrisisPhase::AssaultActive
            );
            if phase == CrisisPhase::AssaultActive {
                crisis.assault_unit_ids = vec![101, 102, 103];
            } else {
                crisis.assault_unit_ids.clear();
            }
        }

        app.update();
        app.update();

        let mut notices = Vec::new();
        let mut statuses = 0;
        while let Ok(raw) = receiver.try_recv() {
            match serde_json::from_str::<ResponsePacket>(&raw).unwrap() {
                ResponsePacket::Notice { noticemsg, .. } => notices.push(noticemsg),
                ResponsePacket::CrisisStatus { .. } => statuses += 1,
                packet => panic!("unexpected transition packet: {packet:?}"),
            }
        }
        assert_eq!(notices, vec![expected_notice]);
        assert_eq!(statuses, 1, "each transition sends one status snapshot");
    }
}

#[test]
fn checkpoint4_legacy_login_sends_only_a_clear_personal_crisis_status() {
    let player_id = 9;
    let client_id = Uuid::new_v4();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(4);
    let clients = Clients(Arc::new(Mutex::new(HashMap::from([(
        client_id,
        Client {
            id: client_id,
            player_id,
            sender,
        },
    )]))));
    let mut state = SettlementCrisisState::default();
    state.insert(
        player_id,
        checkpoint4_crisis(CrisisPhase::AssaultActive, 100),
    );
    let mut login_sync = CrisisStatusLoginSync::default();
    login_sync.insert(player_id);

    let mut app = App::new();
    app.insert_resource(GameTick(100));
    app.insert_resource(clients);
    app.insert_resource(SurvivalDirectorConfig::new(SurvivalDirectorMode::Legacy));
    app.insert_resource(state);
    app.insert_resource(login_sync);
    app.insert_resource(CrisisStatusDeliveryState::default());
    app.insert_resource(CrisisTelemetryState::default());
    app.insert_resource(CrisisBalanceTelemetryState::default());
    app.insert_resource(ResumeLoginSyncState::default());
    app.insert_resource(SafeLogoutTelemetryState::default());
    app.add_systems(Update, crisis_status_delivery_system);
    app.update();

    let packet = receiver.try_recv().expect("legacy clear snapshot");
    let packet: ResponsePacket = serde_json::from_str(&packet).unwrap();
    assert!(matches!(
        packet,
        ResponsePacket::CrisisStatus {
            status: CrisisStatusSnapshot { exists: false, .. }
        }
    ));
}
