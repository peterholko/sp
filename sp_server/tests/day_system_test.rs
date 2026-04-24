use std::collections::HashMap;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use siege_perilous::{
    constants::*,
    event::MapEvents,
    game::GameTick,
    item::Inventory,
    obj::{Id, Template, Viewshed},
    world::WorldPlugin,
    AppState,
};

#[test]
fn test_day_system_night() {
    // Setup
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(WorldPlugin);
    app.insert_state(AppState::Running);
    app.insert_resource(GameTick(0));
    app.insert_resource(MapEvents(HashMap::new()));

    // Add a player entity with a viewshed
    let player_id = app
        .world_mut()
        .spawn((
            Id(1),
            Template("player".to_string()),
            Viewshed { range: 10 },
            Inventory {
                owner: 1,
                items: Vec::new(),
            },
        ))
        .id();

    // Test night time
    app.world_mut().resource_mut::<GameTick>().0 = NIGHT;
    app.update();
    assert_eq!(app.world().get::<Viewshed>(player_id).unwrap().range, 0);
}

#[test]
fn test_day_system_first_light() {
    // Setup
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(WorldPlugin);
    app.insert_state(AppState::Running);
    app.insert_resource(GameTick(0));
    app.insert_resource(MapEvents(HashMap::new()));

    // Add a player entity with a viewshed
    let player_id = app
        .world_mut()
        .spawn((
            Id(1),
            Template("player".to_string()),
            Viewshed { range: 10 },
            Inventory {
                owner: 1,
                items: Vec::new(),
            },
        ))
        .id();

    // Test first light
    app.world_mut().resource_mut::<GameTick>().0 = FIRST_LIGHT;
    app.update();
    assert_eq!(app.world().get::<Viewshed>(player_id).unwrap().range, 5);
}

#[test]
fn test_day_system_dawn() {
    // Setup
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(WorldPlugin);
    app.insert_state(AppState::Running);
    app.insert_resource(GameTick(0));
    app.insert_resource(MapEvents(HashMap::new()));

    // Add a player entity with a viewshed
    let player_id = app
        .world_mut()
        .spawn((
            Id(1),
            Template("player".to_string()),
            Viewshed { range: 10 },
            Inventory {
                owner: 1,
                items: Vec::new(),
            },
        ))
        .id();

    // Test dawn
    app.world_mut().resource_mut::<GameTick>().0 = DAWN;
    app.update();
    assert_eq!(app.world().get::<Viewshed>(player_id).unwrap().range, 7);
}

#[test]
fn test_day_system_morning() {
    // Setup
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(WorldPlugin);
    app.insert_state(AppState::Running);
    app.insert_resource(GameTick(0));
    app.insert_resource(MapEvents(HashMap::new()));

    // Add a player entity with a viewshed
    let player_id = app
        .world_mut()
        .spawn((
            Id(1),
            Template("player".to_string()),
            Viewshed { range: 10 },
            Inventory {
                owner: 1,
                items: Vec::new(),
            },
        ))
        .id();

    // Test morning
    app.world_mut().resource_mut::<GameTick>().0 = MORNING;
    app.update();
    assert_eq!(app.world().get::<Viewshed>(player_id).unwrap().range, 10);
}

#[test]
fn test_day_system_evening() {
    // Setup
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(WorldPlugin);
    app.insert_state(AppState::Running);
    app.insert_resource(GameTick(0));
    app.insert_resource(MapEvents(HashMap::new()));

    // Add a player entity with a viewshed
    let player_id = app
        .world_mut()
        .spawn((
            Id(1),
            Template("player".to_string()),
            Viewshed { range: 10 },
            Inventory {
                owner: 1,
                items: Vec::new(),
            },
        ))
        .id();

    // Test evening
    app.world_mut().resource_mut::<GameTick>().0 = EVENING;
    app.update();
    assert_eq!(app.world().get::<Viewshed>(player_id).unwrap().range, 7);
}

#[test]
fn test_day_system_dusk() {
    // Setup
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(WorldPlugin);
    app.insert_state(AppState::Running);
    app.insert_resource(GameTick(0));
    app.insert_resource(MapEvents(HashMap::new()));

    // Add a player entity with a viewshed
    let player_id = app
        .world_mut()
        .spawn((
            Id(1),
            Template("player".to_string()),
            Viewshed { range: 10 },
            Inventory {
                owner: 1,
                items: Vec::new(),
            },
        ))
        .id();

    // Test dusk
    app.world_mut().resource_mut::<GameTick>().0 = DUSK;
    app.update();
    assert_eq!(app.world().get::<Viewshed>(player_id).unwrap().range, 5);
}
