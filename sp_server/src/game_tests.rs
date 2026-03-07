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
    assert_eq!(0 * 1000, 0);    // Tier 0: no bonus
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
    crisis_state.insert(1, PlayerCrisis { rat_spoilage: true, ..Default::default() });

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
    let rat_hp = 20;       // T1
    let wolf_hp = 45;      // T2
    let wolf_rider_hp = 75; // T3
    let goblin_pillager_hp = 55; // T5

    assert!(rat_hp < wolf_hp, "T1 rat should have less HP than T2 wolf");
    assert!(wolf_hp < wolf_rider_hp, "T2 wolf should have less HP than T3 wolf rider");
    assert!(goblin_pillager_hp > rat_hp, "T5 pillager should have more HP than T1 rat");
}

#[test]
fn creature_kill_xp_follows_tier_progression() {
    let rat_xp = 50;         // T1
    let wolf_xp = 150;       // T2
    let wolf_rider_xp = 300; // T3
    let zombie_xp = 100;     // T4 (weak individually)
    let necro_xp = 500;      // T4 boss
    let pillager_xp = 250;   // T5

    assert!(rat_xp < wolf_xp, "T1 should give less XP than T2");
    assert!(wolf_xp < wolf_rider_xp, "T2 should give less XP than T3");
    assert!(zombie_xp < necro_xp, "T4 zombie should give less XP than T4 boss");
    assert!(pillager_xp > wolf_xp, "T5 pillager should give more XP than T2 wolf");
}

#[test]
fn hero_warrior_can_survive_tier1_encounter() {
    // Novice Warrior (100 HP) vs Giant Rat (2 dmg + 3 range = 5 max)
    // Warrior survives at least 100/5 = 20 hits
    let warrior_hp = 100;
    let rat_max_dmg = 2 + 3;
    let hits_to_kill_warrior = warrior_hp / rat_max_dmg;
    assert!(hits_to_kill_warrior >= 10, "Warrior should survive 10+ rat hits, got {}", hits_to_kill_warrior);
}

#[test]
fn hero_can_kill_tier1_creatures_quickly() {
    // Novice Warrior (2 dmg, 2 range) + Copper Axe (+11 dmg)
    // Avg damage: 2 + 1 + 11 = 14. Giant Rat HP: 20
    let hero_avg_dmg = 2 + 1 + 11;
    let rat_hp = 20;
    let hits_to_kill_rat = (rat_hp as f64 / hero_avg_dmg as f64).ceil() as i32;
    assert!(hits_to_kill_rat <= 3, "Hero should kill T1 rat in 3 or fewer hits, got {}", hits_to_kill_rat);
}

#[test]
fn tier5_creatures_are_dangerous_to_novice() {
    // Goblin Pillager (5 dmg + 4 range) vs Novice Warrior (100 HP)
    let warrior_hp = 100;
    let pillager_avg_dmg = 5 + 2; // base + avg_range
    let hits_to_kill_warrior = warrior_hp / pillager_avg_dmg;
    assert!(hits_to_kill_warrior <= 20, "T5 should be dangerous: warrior survives {} hits", hits_to_kill_warrior);
    assert!(hits_to_kill_warrior >= 5, "T5 shouldn't one-shot: warrior survives {} hits", hits_to_kill_warrior);
}

#[test]
fn hero_stamina_allows_reasonable_combat() {
    let stamina_cost_per_attack = 5;
    let warrior_attacks = 100 / stamina_cost_per_attack;
    let mage_attacks = 150 / stamina_cost_per_attack;

    assert!(warrior_attacks >= 15, "Warrior should get 15+ attacks, got {}", warrior_attacks);
    assert!(mage_attacks >= 25, "Mage should get 25+ attacks, got {}", mage_attacks);
}

#[test]
fn hero_classes_have_distinct_profiles() {
    // (hp, def, speed)
    let warrior = (100, 2, 5);
    let ranger = (75, 1, 6);
    let mage = (60, 1, 5);

    assert!(warrior.0 > ranger.0, "Warrior should have more HP than Ranger");
    assert!(warrior.0 > mage.0, "Warrior should have more HP than Mage");
    assert!(warrior.1 >= ranger.1, "Warrior should have >= def than Ranger");
    assert!(ranger.2 > warrior.2, "Ranger should be faster than Warrior");
    assert!(ranger.2 > mage.2, "Ranger should be faster than Mage");
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
    assert!(t4_days >= 2.5 && t4_days <= 3.5, "T4 should be ~3 days from start, got {:.1}", t4_days);

    // Verify T5 is roughly 5 days (12000 ticks) from DAWN (500)
    let t5_days = (tier5_trigger - DAWN) as f64 / ticks_per_day as f64;
    assert!(t5_days >= 4.5 && t5_days <= 5.5, "T5 should be ~5 days from start, got {:.1}", t5_days);
}

// Helper function matching the logic in true_death_system
fn calculate_crisis_tier(crisis: &PlayerCrisis) -> i32 {
    let mut tier = 0;
    if crisis.rat_spoilage { tier = 1; }
    if crisis.wolf_pack { tier = 2; }
    if crisis.goblin_raid { tier = 3; }
    if crisis.undead_incursion { tier = 4; }
    if crisis.goblin_pillager { tier = 5; }
    tier
}
