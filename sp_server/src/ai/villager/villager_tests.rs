use super::*;
use bevy::prelude::App;
use big_brain::prelude::Score;
use big_brain::scorers::spawn_scorer;
use std::collections::HashMap;

use crate::constants::TICKS_PER_SEC;
use crate::obj::ActiveTask;

/// Macro to create a test app with a specific system and standard resources
macro_rules! setup_test_app {
    ($system:expr) => {{
        let mut app = App::new();
        app.add_systems(Update, $system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app
    }};
}

// ==================== Test Utilities ====================

/// Builder for creating test villagers with configurable state
pub struct TestVillagerBuilder {
    thirst: f32,
    hunger: f32,
    tired: f32,
    heat: f32,
    morale: f32,
    active_task: ActiveTask,
    event_state: EventExecutingState,
}

impl Default for TestVillagerBuilder {
    fn default() -> Self {
        Self {
            thirst: 0.0,
            hunger: 0.0,
            tired: 0.0,
            heat: 0.0,
            morale: 50.0,
            active_task: ActiveTask::Idle,
            event_state: EventExecutingState::None,
        }
    }
}

impl TestVillagerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_thirst(mut self, val: f32) -> Self {
        self.thirst = val;
        self
    }

    pub fn with_hunger(mut self, val: f32) -> Self {
        self.hunger = val;
        self
    }

    pub fn with_tired(mut self, val: f32) -> Self {
        self.tired = val;
        self
    }

    pub fn with_heat(mut self, val: f32) -> Self {
        self.heat = val;
        self
    }

    pub fn with_morale(mut self, val: f32) -> Self {
        self.morale = val;
        self
    }

    pub fn with_active_task(mut self, task: ActiveTask) -> Self {
        self.active_task = task;
        self
    }

    pub fn with_event_state(mut self, state: EventExecutingState) -> Self {
        self.event_state = state;
        self
    }

    pub fn spawn(self, world: &mut World) -> Entity {
        world
            .spawn((
                Thirst::new(self.thirst, 0.01),
                Hunger::new(self.hunger, 0.01),
                Tired::new(self.tired, 0.01),
                Heat::new(self.heat),
                Morale::new(self.morale),
                self.active_task,
                EventExecuting {
                    event_type: String::new(),
                    state: self.event_state,
                },
            ))
            .id()
    }
}

// ==================== Thirst Scorer Tests ====================

#[test]
fn thirsty_scorer_returns_low_score_when_hydrated() {
    let mut app = setup_test_app!(thirsty_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_thirst(10.0)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&ThirstyScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    assert!(
        score.get() < 0.2,
        "Expected low score for hydrated villager, got {}",
        score.get()
    );
}

#[test]
fn thirsty_scorer_returns_high_score_when_thirsty() {
    let mut app = setup_test_app!(thirsty_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_thirst(80.0)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&ThirstyScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    assert!(
        score.get() >= 0.8,
        "Expected high score for thirsty villager, got {}",
        score.get()
    );
}

#[test]
fn thirsty_scorer_returns_emergency_score_when_dehydrated() {
    let mut app = setup_test_app!(thirsty_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_thirst(95.0) // Above DEHYDRATED_SCORE (90.0)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&ThirstyScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    assert!(
        score.get() >= 0.99,
        "Expected emergency score for dehydrated villager, got {}",
        score.get()
    );
}

#[test]
fn thirsty_scorer_skips_when_event_completed() {
    let mut app = setup_test_app!(thirsty_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_thirst(80.0)
        .with_event_state(EventExecutingState::Completed)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&ThirstyScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    // Score should remain at default (0.0) since we skip completed events
    assert_eq!(
        score.get(),
        0.0,
        "Expected score to be skipped when event is completed"
    );
}

#[test]
fn thirsty_scorer_boosts_score_when_already_drinking() {
    let mut app = setup_test_app!(thirsty_scorer_system);

    // Villager already getting drink
    let villager_drinking = TestVillagerBuilder::new()
        .with_thirst(50.0)
        .with_active_task(ActiveTask::GettingDrink)
        .spawn(app.world_mut());

    // Villager idle with same thirst
    let villager_idle = TestVillagerBuilder::new()
        .with_thirst(50.0)
        .with_active_task(ActiveTask::Idle)
        .spawn(app.world_mut());

    let scorer_drinking = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&ThirstyScorer, &mut commands, villager_drinking)
    };
    let scorer_idle = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&ThirstyScorer, &mut commands, villager_idle)
    };
    app.world_mut().flush();

    app.update();

    let score_drinking = app
        .world()
        .entity(scorer_drinking)
        .get::<Score>()
        .unwrap()
        .get();
    let score_idle = app
        .world()
        .entity(scorer_idle)
        .get::<Score>()
        .unwrap()
        .get();

    assert!(
        score_drinking > score_idle,
        "Expected drinking villager to have higher score ({}) than idle ({})",
        score_drinking,
        score_idle
    );
}

// ==================== Hunger Scorer Tests ====================

#[test]
fn hungry_scorer_returns_low_score_when_satiated() {
    let mut app = setup_test_app!(hungry_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_hunger(10.0)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&HungryScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    assert!(
        score.get() < 0.2,
        "Expected low score for satiated villager, got {}",
        score.get()
    );
}

#[test]
fn hungry_scorer_returns_high_score_when_hungry() {
    let mut app = setup_test_app!(hungry_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_hunger(80.0)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&HungryScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    assert!(
        score.get() >= 0.8,
        "Expected high score for hungry villager, got {}",
        score.get()
    );
}

#[test]
fn hungry_scorer_returns_emergency_score_when_starving() {
    let mut app = setup_test_app!(hungry_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_hunger(95.0) // Above STARVING_SCORE (90.0)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&HungryScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    assert!(
        score.get() >= 0.99,
        "Expected emergency score for starving villager, got {}",
        score.get()
    );
}

// ==================== Drowsy Scorer Tests ====================

#[test]
fn drowsy_scorer_returns_low_score_when_rested() {
    let mut app = setup_test_app!(drowsy_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_tired(10.0)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&DrowsyScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    assert!(
        score.get() < 0.2,
        "Expected low score for rested villager, got {}",
        score.get()
    );
}

#[test]
fn drowsy_scorer_returns_high_score_when_tired() {
    let mut app = setup_test_app!(drowsy_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_tired(80.0)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&DrowsyScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    assert!(
        score.get() >= 0.8,
        "Expected high score for tired villager, got {}",
        score.get()
    );
}

// ==================== Idle Scorer Tests ====================

#[test]
fn idle_scorer_returns_baseline_score() {
    let mut app = App::new();
    app.add_systems(Update, idle_scorer_system);
    app.world_mut().insert_resource(minimal_templates());

    let villager = app.world_mut().spawn(()).id();

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&IdleScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    assert_eq!(
        score.get(),
        0.1,
        "Expected idle score to be 0.1 (baseline)"
    );
}

// ==================== Heat Scorer Tests ====================
// NOTE: Heat scorer logic is currently commented out in the implementation,
// so these tests verify the current (disabled) behavior.

#[test]
fn heat_scorer_returns_zero_when_logic_disabled() {
    let mut app = setup_test_app!(heat_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_heat(80.0) // Very hot, but logic is disabled
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&HeatScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    // Heat scorer logic is currently commented out, so score remains 0
    assert_eq!(
        score.get(),
        0.0,
        "Heat scorer should return 0 when logic is disabled"
    );
}

// ==================== Morale Scorer Tests ====================
// NOTE: Morale scorer currently returns a fixed 0.6 score regardless of morale value.

#[test]
fn morale_scorer_returns_fixed_score() {
    let mut app = setup_test_app!(morale_scorer_system);

    let villager = TestVillagerBuilder::new()
        .with_morale(60.0)
        .spawn(app.world_mut());

    let scorer_entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&GoodMorale, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
    // Morale scorer currently returns a fixed 0.6 score
    assert_eq!(
        score.get(),
        0.6,
        "Morale scorer should return fixed 0.6 score"
    );
}

// ==================== Priority Tests ====================

#[test]
fn thirst_beats_hunger_at_same_level() {
    let mut app = App::new();
    app.add_systems(Update, (thirsty_scorer_system, hungry_scorer_system));
    app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
    app.world_mut()
        .insert_resource(EntityObjMap(HashMap::new()));

    let villager = TestVillagerBuilder::new()
        .with_thirst(70.0)
        .with_hunger(70.0)
        .spawn(app.world_mut());

    let thirst_scorer = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&ThirstyScorer, &mut commands, villager)
    };
    let hunger_scorer = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&HungryScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let thirst_score = app
        .world()
        .entity(thirst_scorer)
        .get::<Score>()
        .unwrap()
        .get();
    let hunger_score = app
        .world()
        .entity(hunger_scorer)
        .get::<Score>()
        .unwrap()
        .get();

    // At same level, scores should be equal (both use same formula)
    assert!(
        (thirst_score - hunger_score).abs() < 0.01,
        "Expected similar scores at same level: thirst={}, hunger={}",
        thirst_score,
        hunger_score
    );
}

#[test]
fn emergency_needs_override_normal_needs() {
    let mut app = App::new();
    app.add_systems(Update, (thirsty_scorer_system, hungry_scorer_system));
    app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
    app.world_mut()
        .insert_resource(EntityObjMap(HashMap::new()));

    let villager = TestVillagerBuilder::new()
        .with_thirst(95.0) // Emergency level
        .with_hunger(60.0) // Normal level
        .spawn(app.world_mut());

    let thirst_scorer = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&ThirstyScorer, &mut commands, villager)
    };
    let hunger_scorer = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(&HungryScorer, &mut commands, villager)
    };
    app.world_mut().flush();

    app.update();

    let thirst_score = app
        .world()
        .entity(thirst_scorer)
        .get::<Score>()
        .unwrap()
        .get();
    let hunger_score = app
        .world()
        .entity(hunger_scorer)
        .get::<Score>()
        .unwrap()
        .get();

    assert!(
        thirst_score > hunger_score,
        "Expected emergency thirst ({}) to override normal hunger ({})",
        thirst_score,
        hunger_score
    );
}

// ==================== Helper Functions ====================

fn minimal_templates() -> Templates {
    use crate::templates::ObjTemplate;

    let villager_template = ObjTemplate {
        class: "unit".to_string(),
        subclass: "villager".to_string(),
        template: "Villager".to_string(),
        image: "villager".to_string(),
        family: None,
        groups: None,
        base_hp: None,
        base_stamina: None,
        base_dmg: None,
        dmg_range: None,
        base_def: None,
        base_speed: None,
        base_vision: Some(10),
        base_work: None,
        int: None,
        aggression: None,
        kill_xp: None,
        images: None,
        hsl: None,
        waterwalk: None,
        landwalk: None,
        capacity: None,
        max_residents: None,
        campfire: None,
        build_cost: None,
        upgrade_cost: None,
        level: None,
        refine: None,
        req: None,
        upgrade_req: None,
        upgrade_to: None,
        profession: None,
        upkeep: None,
        activity: None,
        workspaces: None,
    };

    Templates::from_obj_templates(vec![villager_template])
}

// ==================== Action State Tests ====================

use big_brain::actions::spawn_action;
use crate::event::{GameEvents, MapEvents};
use crate::ids::Ids;
use crate::item::Item;

/// Builder for creating test villagers with full VillagerQuery components
pub struct ActionTestVillagerBuilder {
    id: i32,
    player_id: i32,
    position: Position,
    thirst: f32,
    hunger: f32,
    tired: f32,
    heat: f32,
    morale: f32,
    active_task: ActiveTask,
    event_state: EventExecutingState,
    inventory_items: Vec<Item>,
}

impl Default for ActionTestVillagerBuilder {
    fn default() -> Self {
        Self {
            id: 1,
            player_id: 1,
            position: Position { x: 0, y: 0 },
            thirst: 0.0,
            hunger: 0.0,
            tired: 0.0,
            heat: 0.0,
            morale: 50.0,
            active_task: ActiveTask::Idle,
            event_state: EventExecutingState::None,
            inventory_items: Vec::new(),
        }
    }
}

impl ActionTestVillagerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_id(mut self, id: i32) -> Self {
        self.id = id;
        self
    }

    pub fn with_thirst(mut self, val: f32) -> Self {
        self.thirst = val;
        self
    }

    pub fn with_hunger(mut self, val: f32) -> Self {
        self.hunger = val;
        self
    }

    pub fn with_tired(mut self, val: f32) -> Self {
        self.tired = val;
        self
    }

    pub fn with_event_state(mut self, state: EventExecutingState) -> Self {
        self.event_state = state;
        self
    }

    pub fn with_drink_item(mut self) -> Self {
        self.inventory_items.push(create_drink_item(self.id));
        self
    }

    pub fn with_food_item(mut self) -> Self {
        self.inventory_items.push(create_food_item(self.id));
        self
    }

    pub fn spawn(self, world: &mut World) -> Entity {
        world
            .spawn((
                Id(self.id),
                PlayerId(self.player_id),
                self.position,
                Class("unit".to_string()),
                State::None,
                Inventory {
                    owner: self.id,
                    items: self.inventory_items,
                },
                self.active_task,
                SubclassVillager,
                Thirst::new(self.thirst, 0.01),
                Hunger::new(self.hunger, 0.01),
                Tired::new(self.tired, 0.01),
                Heat::new(self.heat),
                Morale::new(self.morale),
                EventExecuting {
                    event_type: String::new(),
                    state: self.event_state,
                },
            ))
            .id()
    }
}

/// Creates a drink item for testing
fn create_drink_item(owner: i32) -> Item {
    Item {
        id: 100,
        owner,
        name: "Spring Water".to_string(),
        quantity: 1,
        durability: None,
        class: DRINK.to_string(),
        subclass: "Water".to_string(),
        slot: None,
        image: "spring_water.png".to_string(),
        weight: 1.0,
        equipped: false,
        experiment: None,
        start_time: 0,
        attrs: HashMap::new(),
        produces: Vec::new(),
    }
}

/// Creates a food item for testing
fn create_food_item(owner: i32) -> Item {
    Item {
        id: 101,
        owner,
        name: "Berries".to_string(),
        quantity: 1,
        durability: None,
        class: FOOD.to_string(),
        subclass: "Food".to_string(),
        slot: None,
        image: "berries.png".to_string(),
        weight: 0.5,
        equipped: false,
        experiment: None,
        start_time: 0,
        attrs: HashMap::new(),
        produces: Vec::new(),
    }
}

/// Macro to create a test app with action system and all required resources
macro_rules! setup_action_test_app {
    ($system:expr) => {{
        let mut app = App::new();
        app.add_systems(Update, $system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut()
            .insert_resource(MapEvents(HashMap::new()));
        app.world_mut()
            .insert_resource(GameEvents(HashMap::new()));
        app
    }};
}

/// Helper to spawn an action and set it to Requested state for testing.
/// Big_brain starts actions in Init state, but the thinker system transitions
/// them to Requested. For unit testing without the full thinker, we set it manually.
fn spawn_action_as_requested<T: ActionBuilder + Clone>(
    app: &mut App,
    action: &T,
    actor: Entity,
) -> Entity {
    let action_entity = {
        let mut commands = app.world_mut().commands();
        spawn_action(action, &mut commands, actor)
    };
    app.world_mut().flush();

    // Set action state to Requested
    *app.world_mut()
        .entity_mut(action_entity)
        .get_mut::<ActionState>()
        .unwrap() = ActionState::Requested;

    action_entity
}

// ==================== Drink Action Tests ====================

#[test]
fn drink_action_transitions_to_executing_when_drink_available() {
    let mut app = setup_action_test_app!(drink_action_system);

    let villager = ActionTestVillagerBuilder::new()
        .with_thirst(80.0)
        .with_drink_item()
        .spawn(app.world_mut());

    // Register villager in entity map
    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    let action_entity = spawn_action_as_requested(&mut app, &Drink, villager);

    // Run one tick
    app.update();

    // Check action state transitioned to Executing
    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected action to transition to Executing"
    );

    // Check villager state changed to Drinking
    let villager_state = app.world().entity(villager).get::<State>().unwrap();
    assert_eq!(
        *villager_state,
        State::Drinking,
        "Expected villager state to be Drinking"
    );

    // Check EventExecuting state is Executing
    let event_executing = app
        .world()
        .entity(villager)
        .get::<EventExecuting>()
        .unwrap();
    assert_eq!(
        event_executing.state,
        EventExecutingState::Executing,
        "Expected EventExecuting state to be Executing"
    );
}

#[test]
fn drink_action_fails_when_no_drink_in_inventory() {
    let mut app = setup_action_test_app!(drink_action_system);

    // Create villager without drink item
    let villager = ActionTestVillagerBuilder::new()
        .with_thirst(80.0)
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    let action_entity = spawn_action_as_requested(&mut app, &Drink, villager);

    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Failure,
        "Expected action to fail when no drink available"
    );
}

#[test]
fn drink_action_succeeds_when_event_completes() {
    let mut app = setup_action_test_app!(drink_action_system);

    let villager = ActionTestVillagerBuilder::new()
        .with_thirst(80.0)
        .with_drink_item()
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    let action_entity = spawn_action_as_requested(&mut app, &Drink, villager);

    // First tick: Requested -> Executing
    app.update();

    // Simulate event completion by setting EventExecuting state to Completed
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::Completed;

    // Second tick: Executing -> Success
    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Success,
        "Expected action to succeed when event completes"
    );
}

// ==================== Eat Action Tests ====================

#[test]
fn eat_action_transitions_to_executing_when_food_available() {
    let mut app = setup_action_test_app!(eat_action_system);

    let villager = ActionTestVillagerBuilder::new()
        .with_hunger(80.0)
        .with_food_item()
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    let action_entity = spawn_action_as_requested(&mut app, &Eat, villager);

    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected action to transition to Executing"
    );

    let villager_state = app.world().entity(villager).get::<State>().unwrap();
    assert_eq!(
        *villager_state,
        State::Eating,
        "Expected villager state to be Eating"
    );
}

#[test]
fn eat_action_fails_when_no_food_in_inventory() {
    let mut app = setup_action_test_app!(eat_action_system);

    let villager = ActionTestVillagerBuilder::new()
        .with_hunger(80.0)
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    let action_entity = spawn_action_as_requested(&mut app, &Eat, villager);

    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Failure,
        "Expected action to fail when no food available"
    );
}

#[test]
fn eat_action_succeeds_when_event_completes() {
    let mut app = setup_action_test_app!(eat_action_system);

    let villager = ActionTestVillagerBuilder::new()
        .with_hunger(80.0)
        .with_food_item()
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    let action_entity = spawn_action_as_requested(&mut app, &Eat, villager);

    // First tick: Requested -> Executing
    app.update();

    // Simulate event completion
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::Completed;

    // Second tick: Executing -> Success
    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Success,
        "Expected action to succeed when event completes"
    );
}

// ==================== Sleep Action Tests ====================

#[test]
fn sleep_action_transitions_to_executing() {
    let mut app = setup_action_test_app!(sleep_action_system);

    let villager = ActionTestVillagerBuilder::new()
        .with_tired(80.0)
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    let action_entity = spawn_action_as_requested(&mut app, &Sleep, villager);

    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected action to transition to Executing"
    );

    let villager_state = app.world().entity(villager).get::<State>().unwrap();
    assert_eq!(
        *villager_state,
        State::Sleeping,
        "Expected villager state to be Sleeping"
    );
}

#[test]
fn sleep_action_succeeds_when_event_completes() {
    let mut app = setup_action_test_app!(sleep_action_system);

    let villager = ActionTestVillagerBuilder::new()
        .with_tired(80.0)
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    let action_entity = spawn_action_as_requested(&mut app, &Sleep, villager);

    // First tick: Requested -> Executing
    app.update();

    // Simulate event completion
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::Completed;

    // Second tick: Executing -> Success
    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Success,
        "Expected action to succeed when event completes"
    );

    // Sleep action resets EventExecuting state to None on success
    let event_executing = app
        .world()
        .entity(villager)
        .get::<EventExecuting>()
        .unwrap();
    assert_eq!(
        event_executing.state,
        EventExecutingState::None,
        "Expected EventExecuting state to be reset to None after sleep completes"
    );
}

#[test]
fn sleep_action_cancelled_transitions_to_failure() {
    let mut app = setup_action_test_app!(sleep_action_system);

    let villager = ActionTestVillagerBuilder::new()
        .with_tired(80.0)
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    let action_entity = spawn_action_as_requested(&mut app, &Sleep, villager);

    // First tick: Requested -> Executing
    app.update();

    // Manually set action state to Cancelled
    *app.world_mut()
        .entity_mut(action_entity)
        .get_mut::<ActionState>()
        .unwrap() = ActionState::Cancelled;

    // Second tick: Cancelled -> Failure
    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Failure,
        "Expected cancelled action to transition to Failure"
    );
}

// ==================== Integration Tests: Complete Behavior Cycles ====================

/// Helper macro to set up a multi-scorer test app with all scorer systems
macro_rules! setup_multi_scorer_app {
    () => {{
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                thirsty_scorer_system,
                hungry_scorer_system,
                drowsy_scorer_system,
                idle_scorer_system,
                morale_scorer_system,
                heat_scorer_system,
            ),
        );
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(minimal_templates());
        app
    }};
}

/// Helper to spawn a scorer and return its entity
fn spawn_scorer_for<T: ScorerBuilder + Clone>(
    app: &mut App,
    scorer: &T,
    actor: Entity,
) -> Entity {
    let entity = {
        let mut commands = app.world_mut().commands();
        spawn_scorer(scorer, &mut commands, actor)
    };
    app.world_mut().flush();
    entity
}

/// Helper to get the score value from a scorer entity
fn get_score(app: &App, scorer_entity: Entity) -> f32 {
    app.world()
        .entity(scorer_entity)
        .get::<Score>()
        .unwrap()
        .get()
}

// ---------- Multi-Scorer Priority Decision Tests ----------

#[test]
fn villager_prioritizes_drinking_over_eating_when_very_thirsty() {
    let mut app = setup_multi_scorer_app!();

    let villager = TestVillagerBuilder::new()
        .with_thirst(95.0) // Emergency: above DEHYDRATED_SCORE (90.0)
        .with_hunger(60.0) // Moderate hunger
        .spawn(app.world_mut());

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);

    app.update();

    let thirst_score = get_score(&app, thirst_scorer);
    let hunger_score = get_score(&app, hunger_scorer);

    // Thirst at emergency level (0.99) should dominate moderate hunger (0.60)
    assert!(
        thirst_score > hunger_score,
        "Expected thirst ({}) to beat hunger ({}) when dehydrated",
        thirst_score,
        hunger_score
    );
    assert!(
        thirst_score >= 0.99,
        "Expected emergency thirst score, got {}",
        thirst_score
    );
}

#[test]
fn villager_prioritizes_eating_over_drinking_when_starving() {
    let mut app = setup_multi_scorer_app!();

    let villager = TestVillagerBuilder::new()
        .with_thirst(60.0) // Moderate thirst
        .with_hunger(95.0) // Emergency: above STARVING_SCORE (90.0)
        .spawn(app.world_mut());

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);

    app.update();

    let thirst_score = get_score(&app, thirst_scorer);
    let hunger_score = get_score(&app, hunger_scorer);

    // Hunger at emergency level (0.99) should dominate moderate thirst (0.60)
    assert!(
        hunger_score > thirst_score,
        "Expected hunger ({}) to beat thirst ({}) when starving",
        hunger_score,
        thirst_score
    );
    assert!(
        hunger_score >= 0.99,
        "Expected emergency hunger score, got {}",
        hunger_score
    );
}

#[test]
fn emergency_thirst_overrides_high_tiredness() {
    let mut app = setup_multi_scorer_app!();

    let villager = TestVillagerBuilder::new()
        .with_thirst(95.0) // Emergency
        .with_tired(75.0)  // High but not emergency
        .spawn(app.world_mut());

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let drowsy_scorer = spawn_scorer_for(&mut app, &DrowsyScorer, villager);

    app.update();

    let thirst_score = get_score(&app, thirst_scorer);
    let drowsy_score = get_score(&app, drowsy_scorer);

    assert!(
        thirst_score > drowsy_score,
        "Expected emergency thirst ({}) to override high tiredness ({})",
        thirst_score,
        drowsy_score
    );
}

#[test]
fn emergency_hunger_overrides_high_tiredness() {
    let mut app = setup_multi_scorer_app!();

    let villager = TestVillagerBuilder::new()
        .with_hunger(95.0) // Emergency
        .with_tired(75.0)  // High but not emergency
        .spawn(app.world_mut());

    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);
    let drowsy_scorer = spawn_scorer_for(&mut app, &DrowsyScorer, villager);

    app.update();

    let hunger_score = get_score(&app, hunger_scorer);
    let drowsy_score = get_score(&app, drowsy_scorer);

    assert!(
        hunger_score > drowsy_score,
        "Expected emergency hunger ({}) to override high tiredness ({})",
        hunger_score,
        drowsy_score
    );
}

#[test]
fn all_vital_needs_competing_highest_emergency_wins() {
    let mut app = setup_multi_scorer_app!();

    // Only thirst is at emergency level
    let villager = TestVillagerBuilder::new()
        .with_thirst(95.0) // Emergency
        .with_hunger(75.0) // High routine
        .with_tired(60.0)  // Moderate
        .spawn(app.world_mut());

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);
    let drowsy_scorer = spawn_scorer_for(&mut app, &DrowsyScorer, villager);
    let idle_scorer = spawn_scorer_for(&mut app, &IdleScorer, villager);
    let morale_scorer = spawn_scorer_for(&mut app, &GoodMorale, villager);

    app.update();

    let thirst_score = get_score(&app, thirst_scorer);
    let hunger_score = get_score(&app, hunger_scorer);
    let drowsy_score = get_score(&app, drowsy_scorer);
    let idle_score = get_score(&app, idle_scorer);
    let morale_score = get_score(&app, morale_scorer);

    // Emergency thirst should be highest
    assert!(
        thirst_score > hunger_score,
        "Thirst ({}) should beat hunger ({})",
        thirst_score,
        hunger_score
    );
    assert!(
        thirst_score > drowsy_score,
        "Thirst ({}) should beat drowsy ({})",
        thirst_score,
        drowsy_score
    );
    assert!(
        thirst_score > morale_score,
        "Thirst ({}) should beat morale ({})",
        thirst_score,
        morale_score
    );
    assert!(
        thirst_score > idle_score,
        "Thirst ({}) should beat idle ({})",
        thirst_score,
        idle_score
    );
}

#[test]
fn moderate_thirst_beats_idle_and_morale() {
    let mut app = setup_multi_scorer_app!();

    let villager = TestVillagerBuilder::new()
        .with_thirst(80.0) // High thirst -> score 0.80
        .spawn(app.world_mut());

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let idle_scorer = spawn_scorer_for(&mut app, &IdleScorer, villager);
    let morale_scorer = spawn_scorer_for(&mut app, &GoodMorale, villager);

    app.update();

    let thirst_score = get_score(&app, thirst_scorer);
    let idle_score = get_score(&app, idle_scorer);
    let morale_score = get_score(&app, morale_scorer);

    // Thirst at 0.80 should beat morale (0.6) and idle (0.1)
    assert!(
        thirst_score > morale_score,
        "Thirst ({}) should beat morale ({})",
        thirst_score,
        morale_score
    );
    assert!(
        thirst_score > idle_score,
        "Thirst ({}) should beat idle ({})",
        thirst_score,
        idle_score
    );
}

#[test]
fn morale_beats_idle_when_all_needs_low() {
    let mut app = setup_multi_scorer_app!();

    // All vital needs low, so morale (0.6) should beat idle (0.1)
    let villager = TestVillagerBuilder::new()
        .with_thirst(10.0)
        .with_hunger(10.0)
        .with_tired(10.0)
        .with_morale(50.0)
        .spawn(app.world_mut());

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);
    let drowsy_scorer = spawn_scorer_for(&mut app, &DrowsyScorer, villager);
    let idle_scorer = spawn_scorer_for(&mut app, &IdleScorer, villager);
    let morale_scorer = spawn_scorer_for(&mut app, &GoodMorale, villager);

    app.update();

    let thirst_score = get_score(&app, thirst_scorer);
    let hunger_score = get_score(&app, hunger_scorer);
    let drowsy_score = get_score(&app, drowsy_scorer);
    let idle_score = get_score(&app, idle_scorer);
    let morale_score = get_score(&app, morale_scorer);

    // Morale (0.6) should be highest when vital needs are low
    assert!(
        morale_score > thirst_score,
        "Morale ({}) should beat low thirst ({})",
        morale_score,
        thirst_score
    );
    assert!(
        morale_score > hunger_score,
        "Morale ({}) should beat low hunger ({})",
        morale_score,
        hunger_score
    );
    assert!(
        morale_score > drowsy_score,
        "Morale ({}) should beat low drowsy ({})",
        morale_score,
        drowsy_score
    );
    assert!(
        morale_score > idle_score,
        "Morale ({}) should beat idle ({})",
        morale_score,
        idle_score
    );
}

#[test]
fn dual_emergency_needs_both_score_equally() {
    let mut app = setup_multi_scorer_app!();

    // Both thirst and hunger at emergency level
    let villager = TestVillagerBuilder::new()
        .with_thirst(95.0) // Emergency
        .with_hunger(95.0) // Emergency
        .spawn(app.world_mut());

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);

    app.update();

    let thirst_score = get_score(&app, thirst_scorer);
    let hunger_score = get_score(&app, hunger_scorer);

    // Both should be at emergency score (0.99)
    assert!(
        thirst_score >= 0.99,
        "Expected emergency thirst score, got {}",
        thirst_score
    );
    assert!(
        hunger_score >= 0.99,
        "Expected emergency hunger score, got {}",
        hunger_score
    );
    assert!(
        (thirst_score - hunger_score).abs() < 0.01,
        "Expected equal emergency scores: thirst={}, hunger={}",
        thirst_score,
        hunger_score
    );
}

#[test]
fn active_task_boost_gives_drinking_villager_priority_over_equal_idle_villager() {
    let mut app = setup_multi_scorer_app!();

    // Villager already getting drink gets 1.5x multiplier
    let drinking_villager = TestVillagerBuilder::new()
        .with_thirst(50.0)
        .with_active_task(ActiveTask::GettingDrink)
        .spawn(app.world_mut());

    // Villager idle at same thirst level
    let idle_villager = TestVillagerBuilder::new()
        .with_thirst(50.0)
        .with_active_task(ActiveTask::Idle)
        .spawn(app.world_mut());

    let drinking_thirst = spawn_scorer_for(&mut app, &ThirstyScorer, drinking_villager);
    let idle_thirst = spawn_scorer_for(&mut app, &ThirstyScorer, idle_villager);
    let drinking_morale = spawn_scorer_for(&mut app, &GoodMorale, drinking_villager);

    app.update();

    let drinking_score = get_score(&app, drinking_thirst);
    let idle_score = get_score(&app, idle_thirst);
    let morale_score = get_score(&app, drinking_morale);

    // Drinking villager's boosted score (50*1.5/100=0.75) should beat morale (0.6)
    assert!(
        drinking_score > morale_score,
        "Boosted drinking score ({}) should beat morale ({})",
        drinking_score,
        morale_score
    );
    // Idle villager's unboosted score (50/100=0.50) should lose to morale (0.6)
    assert!(
        idle_score < morale_score,
        "Unboosted thirst score ({}) should lose to morale ({})",
        idle_score,
        morale_score
    );
}

// ---------- Complete Behavior Cycle Tests (Scorer + Action) ----------

/// Helper macro for integration test apps with both scorers and actions
macro_rules! setup_behavior_test_app {
    () => {{
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                thirsty_scorer_system,
                hungry_scorer_system,
                drowsy_scorer_system,
                drink_action_system,
                eat_action_system,
                sleep_action_system,
            ),
        );
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut()
            .insert_resource(MapEvents(HashMap::new()));
        app.world_mut()
            .insert_resource(GameEvents(HashMap::new()));
        app
    }};
}

#[test]
fn thirsty_villager_full_drink_cycle() {
    let mut app = setup_behavior_test_app!();

    let villager = ActionTestVillagerBuilder::new()
        .with_thirst(80.0)
        .with_drink_item()
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    // Verify scorer rates thirst highly
    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let action_entity = spawn_action_as_requested(&mut app, &Drink, villager);

    // Tick 1: scorer evaluates, action transitions to Executing
    app.update();

    let thirst_score = get_score(&app, thirst_scorer);
    assert!(
        thirst_score >= 0.8,
        "Expected high thirst score, got {}",
        thirst_score
    );

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected drink action to be executing"
    );

    // Verify villager state changed to Drinking
    let villager_state = app.world().entity(villager).get::<State>().unwrap();
    assert_eq!(*villager_state, State::Drinking);

    // Simulate event completion (external system marks event done)
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::Completed;

    // Tick 2: action detects completion and transitions to Success
    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Success,
        "Expected drink action to succeed after event completes"
    );
}

#[test]
fn hungry_villager_full_eat_cycle() {
    let mut app = setup_behavior_test_app!();

    let villager = ActionTestVillagerBuilder::new()
        .with_hunger(80.0)
        .with_food_item()
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    // Verify scorer rates hunger highly
    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);
    let action_entity = spawn_action_as_requested(&mut app, &Eat, villager);

    // Tick 1: scorer evaluates, action transitions to Executing
    app.update();

    let hunger_score = get_score(&app, hunger_scorer);
    assert!(
        hunger_score >= 0.8,
        "Expected high hunger score, got {}",
        hunger_score
    );

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected eat action to be executing"
    );

    // Verify villager state changed to Eating
    let villager_state = app.world().entity(villager).get::<State>().unwrap();
    assert_eq!(*villager_state, State::Eating);

    // Simulate event completion
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::Completed;

    // Tick 2: action detects completion and transitions to Success
    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Success,
        "Expected eat action to succeed after event completes"
    );
}

#[test]
fn tired_villager_full_sleep_cycle() {
    let mut app = setup_behavior_test_app!();

    let villager = ActionTestVillagerBuilder::new()
        .with_tired(80.0)
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    // Verify scorer rates tiredness highly
    let drowsy_scorer = spawn_scorer_for(&mut app, &DrowsyScorer, villager);
    let action_entity = spawn_action_as_requested(&mut app, &Sleep, villager);

    // Tick 1: scorer evaluates, action transitions to Executing
    app.update();

    let drowsy_score = get_score(&app, drowsy_scorer);
    assert!(
        drowsy_score >= 0.8,
        "Expected high drowsy score, got {}",
        drowsy_score
    );

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected sleep action to be executing"
    );

    // Verify villager state changed to Sleeping
    let villager_state = app.world().entity(villager).get::<State>().unwrap();
    assert_eq!(*villager_state, State::Sleeping);

    // Simulate event completion
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::Completed;

    // Tick 2: action detects completion and transitions to Success
    app.update();

    let action_state = app
        .world()
        .entity(action_entity)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Success,
        "Expected sleep action to succeed after event completes"
    );

    // Sleep action specifically resets EventExecuting to None on success
    let event_executing = app
        .world()
        .entity(villager)
        .get::<EventExecuting>()
        .unwrap();
    assert_eq!(
        event_executing.state,
        EventExecutingState::None,
        "Expected EventExecuting reset to None after sleep completes"
    );
}

// ---------- Multi-Action Sequential Behavior Tests ----------

#[test]
fn villager_can_drink_then_eat_sequentially() {
    let mut app = setup_behavior_test_app!();

    let villager = ActionTestVillagerBuilder::new()
        .with_thirst(80.0)
        .with_hunger(80.0)
        .with_drink_item()
        .with_food_item()
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    // Phase 1: Start drink action
    let drink_action = spawn_action_as_requested(&mut app, &Drink, villager);

    app.update();

    let action_state = app
        .world()
        .entity(drink_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected drink action to be executing"
    );

    // Complete drink event
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::Completed;

    app.update();

    let action_state = app
        .world()
        .entity(drink_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Success,
        "Expected drink action to succeed"
    );

    // Reset EventExecuting for next action
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::None;

    // Phase 2: Start eat action
    let eat_action = spawn_action_as_requested(&mut app, &Eat, villager);

    app.update();

    let action_state = app
        .world()
        .entity(eat_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected eat action to be executing after drink completed"
    );

    // Complete eat event
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::Completed;

    app.update();

    let action_state = app
        .world()
        .entity(eat_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Success,
        "Expected eat action to succeed"
    );
}

#[test]
fn drink_failure_does_not_block_subsequent_eat_action() {
    let mut app = setup_behavior_test_app!();

    // No drink item, but has food item
    let villager = ActionTestVillagerBuilder::new()
        .with_thirst(80.0)
        .with_hunger(80.0)
        .with_food_item()
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    // Phase 1: Drink fails (no drink in inventory)
    let drink_action = spawn_action_as_requested(&mut app, &Drink, villager);

    app.update();

    let action_state = app
        .world()
        .entity(drink_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Failure,
        "Expected drink action to fail without drink item"
    );

    // Phase 2: Eat should still work
    let eat_action = spawn_action_as_requested(&mut app, &Eat, villager);

    app.update();

    let action_state = app
        .world()
        .entity(eat_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected eat action to work after drink failure"
    );

    let villager_state = app.world().entity(villager).get::<State>().unwrap();
    assert_eq!(
        *villager_state,
        State::Eating,
        "Expected villager to be eating after drink failure"
    );
}

#[test]
fn drink_cancellation_allows_new_action() {
    let mut app = setup_behavior_test_app!();

    let villager = ActionTestVillagerBuilder::new()
        .with_thirst(80.0)
        .with_tired(80.0)
        .with_drink_item()
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    // Phase 1: Start drink action
    let drink_action = spawn_action_as_requested(&mut app, &Drink, villager);

    app.update();

    let action_state = app
        .world()
        .entity(drink_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(*action_state, ActionState::Executing);

    // Phase 2: Cancel drink action (simulating brain selecting higher priority)
    *app.world_mut()
        .entity_mut(drink_action)
        .get_mut::<ActionState>()
        .unwrap() = ActionState::Cancelled;

    app.update();

    let action_state = app
        .world()
        .entity(drink_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Failure,
        "Expected cancelled drink to transition to Failure"
    );

    // Phase 3: Sleep action should work after cancellation
    // Reset EventExecuting since cancellation should have been handled
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::None;

    let sleep_action = spawn_action_as_requested(&mut app, &Sleep, villager);

    app.update();

    let action_state = app
        .world()
        .entity(sleep_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Executing,
        "Expected sleep action to work after drink cancellation"
    );

    let villager_state = app.world().entity(villager).get::<State>().unwrap();
    assert_eq!(
        *villager_state,
        State::Sleeping,
        "Expected villager to be sleeping after drink cancellation"
    );
}

#[test]
fn scorer_reflects_need_changes_between_ticks() {
    let mut app = setup_multi_scorer_app!();

    let villager = TestVillagerBuilder::new()
        .with_thirst(40.0)
        .with_hunger(40.0)
        .spawn(app.world_mut());

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);

    // Tick 1: both at moderate levels
    app.update();

    let thirst_score_1 = get_score(&app, thirst_scorer);
    let hunger_score_1 = get_score(&app, hunger_scorer);

    assert!(
        (thirst_score_1 - hunger_score_1).abs() < 0.01,
        "Expected similar scores at same level: thirst={}, hunger={}",
        thirst_score_1,
        hunger_score_1
    );

    // Simulate thirst increasing drastically (e.g., desert environment)
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<Thirst>()
        .unwrap()
        .thirst = 95.0;

    // Tick 2: thirst now at emergency
    app.update();

    let thirst_score_2 = get_score(&app, thirst_scorer);
    let hunger_score_2 = get_score(&app, hunger_scorer);

    assert!(
        thirst_score_2 > thirst_score_1,
        "Expected thirst score to increase: before={}, after={}",
        thirst_score_1,
        thirst_score_2
    );
    assert!(
        thirst_score_2 > hunger_score_2,
        "Expected emergency thirst ({}) to dominate unchanged hunger ({})",
        thirst_score_2,
        hunger_score_2
    );
}

#[test]
fn completed_event_state_causes_scorer_to_skip() {
    let mut app = setup_multi_scorer_app!();

    // Villager with completed event state - scorers should skip
    let villager = TestVillagerBuilder::new()
        .with_thirst(80.0)
        .with_hunger(80.0)
        .with_tired(80.0)
        .with_event_state(EventExecutingState::Completed)
        .spawn(app.world_mut());

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);
    let drowsy_scorer = spawn_scorer_for(&mut app, &DrowsyScorer, villager);

    app.update();

    // All vital scorers should skip when event state is Completed
    assert_eq!(
        get_score(&app, thirst_scorer),
        0.0,
        "Thirst scorer should skip when event completed"
    );
    assert_eq!(
        get_score(&app, hunger_scorer),
        0.0,
        "Hunger scorer should skip when event completed"
    );
    assert_eq!(
        get_score(&app, drowsy_scorer),
        0.0,
        "Drowsy scorer should skip when event completed"
    );
}

#[test]
fn no_drinks_modifier_reduces_thirst_priority() {
    let mut app = setup_multi_scorer_app!();

    // Two villagers at same thirst, one has NoDrinks marker
    let villager_normal = TestVillagerBuilder::new()
        .with_thirst(60.0)
        .spawn(app.world_mut());

    let villager_no_drinks = TestVillagerBuilder::new()
        .with_thirst(60.0)
        .spawn(app.world_mut());

    // Add NoDrinks component to second villager
    app.world_mut()
        .entity_mut(villager_no_drinks)
        .insert(NoDrinks { at_tick: 0 });

    let scorer_normal = spawn_scorer_for(&mut app, &ThirstyScorer, villager_normal);
    let scorer_no_drinks = spawn_scorer_for(&mut app, &ThirstyScorer, villager_no_drinks);

    app.update();

    let score_normal = get_score(&app, scorer_normal);
    let score_no_drinks = get_score(&app, scorer_no_drinks);

    // NoDrinks modifier subtracts 50 from score before dividing by 100
    // Normal: 60/100 = 0.6, NoDrinks: (60-50)/100 = 0.1
    assert!(
        score_normal > score_no_drinks,
        "Normal villager score ({}) should be higher than no-drinks villager ({})",
        score_normal,
        score_no_drinks
    );
}

#[test]
fn no_food_modifier_reduces_hunger_priority() {
    let mut app = setup_multi_scorer_app!();

    let villager_normal = TestVillagerBuilder::new()
        .with_hunger(60.0)
        .spawn(app.world_mut());

    let villager_no_food = TestVillagerBuilder::new()
        .with_hunger(60.0)
        .spawn(app.world_mut());

    // Add NoFood component to second villager
    app.world_mut()
        .entity_mut(villager_no_food)
        .insert(NoFood { at_tick: 0 });

    let scorer_normal = spawn_scorer_for(&mut app, &HungryScorer, villager_normal);
    let scorer_no_food = spawn_scorer_for(&mut app, &HungryScorer, villager_no_food);

    app.update();

    let score_normal = get_score(&app, scorer_normal);
    let score_no_food = get_score(&app, scorer_no_food);

    // NoFood modifier subtracts 50 from score before dividing by 100
    assert!(
        score_normal > score_no_food,
        "Normal villager score ({}) should be higher than no-food villager ({})",
        score_normal,
        score_no_food
    );
}

#[test]
fn emergency_overrides_no_resource_modifier() {
    let mut app = setup_multi_scorer_app!();

    // Even with NoDrinks modifier, emergency thirst (>=90) should still be emergency
    let villager = TestVillagerBuilder::new()
        .with_thirst(95.0) // Above DEHYDRATED_SCORE
        .spawn(app.world_mut());

    app.world_mut()
        .entity_mut(villager)
        .insert(NoDrinks { at_tick: 0 });

    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);

    app.update();

    let score = get_score(&app, thirst_scorer);

    // Emergency overrides the NoDrinks modifier
    assert!(
        score >= 0.99,
        "Emergency thirst should override NoDrinks modifier, got {}",
        score
    );
}

#[test]
fn full_decision_cycle_thirst_scorer_drives_drink_action_to_completion() {
    let mut app = setup_behavior_test_app!();

    let villager = ActionTestVillagerBuilder::new()
        .with_thirst(95.0) // Emergency thirst
        .with_hunger(40.0) // Moderate hunger
        .with_drink_item()
        .with_food_item()
        .spawn(app.world_mut());

    app.world_mut()
        .resource_mut::<EntityObjMap>()
        .insert(1, villager);

    // Spawn both scorers to verify thirst wins
    let thirst_scorer = spawn_scorer_for(&mut app, &ThirstyScorer, villager);
    let hunger_scorer = spawn_scorer_for(&mut app, &HungryScorer, villager);

    // Spawn drink action (the one thirst scorer would trigger)
    let drink_action = spawn_action_as_requested(&mut app, &Drink, villager);

    // Tick 1: scorers evaluate + action begins
    app.update();

    // Verify thirst wins the decision
    let thirst_score = get_score(&app, thirst_scorer);
    let hunger_score = get_score(&app, hunger_scorer);
    assert!(
        thirst_score > hunger_score,
        "Thirst ({}) should win over hunger ({})",
        thirst_score,
        hunger_score
    );

    // Verify drink action is executing
    let action_state = app
        .world()
        .entity(drink_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(*action_state, ActionState::Executing);

    // Simulate drink event completion
    app.world_mut()
        .entity_mut(villager)
        .get_mut::<EventExecuting>()
        .unwrap()
        .state = EventExecutingState::Completed;

    // Tick 2: action completes
    app.update();

    let action_state = app
        .world()
        .entity(drink_action)
        .get::<ActionState>()
        .unwrap();
    assert_eq!(
        *action_state,
        ActionState::Success,
        "Expected drink action to complete the full cycle"
    );
}
