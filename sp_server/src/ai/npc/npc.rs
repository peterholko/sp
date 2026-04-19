use core::f32;

use bevy::prelude::*;
use big_brain::prelude::*;
use rand::Rng;

use crate::ai_logging::entity_display;
use crate::combat::{AttackType, Combat, CombatQuery};
use crate::common::{
    AttackTarget, Destination, Hide, Idle, MoveTo, SetAttackTarget, Target, TaskTarget,
};
use crate::effect::Effect;
use crate::effect::Effects;
use crate::event::{EventCompleted, EventExecuting, EventExecutingState, Spell};
use crate::event::{GameEvent, GameEventType, GameEvents, MapEvents, VisibleEvent};
use crate::game::*;
use crate::ids::EntityObjMap;
use crate::ids::Ids;
use crate::item;
use crate::item::*;
use crate::map::{Map, MapPos};
use crate::obj::{
    BaseQuery, BaseQueryMutState, Class, Id, Obj, ObjStatQuery, PlayerId, Position, State,
    StateChange, Stats, Subclass, SubclassNPC, Template, Viewshed,
};
use crate::obj::{BaseQueryEffects, ClassStructure};
use crate::player;
use crate::player::Player;
use crate::templates::Templates;
use crate::AppState;
use crate::{constants::*, ids};
use crate::{npc_debug, npc_error, npc_info, npc_trace, npc_warn, with_span};

pub const BASE_MOVE_TICKS: f32 = 100.0;
pub const BASE_SPEED: f32 = 1.0;

pub struct NPCTarget {
    pub id: i32,
    pub player_id: i32,
    pub pos: Position,
    pub distance: u32,
    pub fortified: bool,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct ChaseAndAttack;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetTorchTarget;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetSpoilTarget;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetStealTarget;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetCorpseTarget;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetHome;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct VisibleTargetScorer;

#[derive(Debug, Component)]
pub struct VisibleTarget {
    pub target: i32,
}

impl VisibleTarget {
    pub fn new(target: i32) -> Self {
        Self { target }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::App;
    use big_brain::prelude::Score;
    use big_brain::scorers::spawn_scorer;
    use std::collections::HashMap;

    use crate::constants::{CLASS_UNIT, NORMAL_SCORE, NPC_PLAYER_ID, SUBCLASS_NPC, TICKS_PER_SEC};
    use crate::event::{EventExecuting, EventExecutingState};
    use crate::templates::ObjTemplate;

    fn test_stats() -> Stats {
        Stats {
            hp: 10,
            stamina: None,
            base_hp: 10,
            base_stamina: None,
            base_def: 1,
            damage_range: Some(1),
            base_damage: Some(1),
            base_speed: Some(1),
            base_vision: Some(10),
        }
    }

    fn empty_effects() -> Effects {
        Effects(HashMap::<Effect, (i32, f32, i32)>::new())
    }

    fn minimal_templates() -> Templates {
        let npc_template = ObjTemplate {
            class: CLASS_UNIT.to_string(),
            subclass: SUBCLASS_NPC.to_string(),
            template: "Goblin".to_string(),
            image: "goblin".to_string(),
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
            int: Some("cunning".to_string()),
            aggression: Some("medium".to_string()),
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

        Templates::from_obj_templates(vec![npc_template])
    }

    #[test]
    fn target_scorer_picks_nearest_visible_player() {
        let mut app = App::new();
        app.add_systems(Update, target_scorer_system);

        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(minimal_templates());

        let npc_entity = app
            .world_mut()
            .spawn((
                PlayerId(NPC_PLAYER_ID),
                Position { x: 0, y: 0 },
                Template("Goblin".to_string()),
                Viewshed { range: 10 },
                VisibleTarget::new(NO_TARGET),
                SubclassNPC,
                test_stats(),
                EventExecuting {
                    event_type: String::new(),
                    state: EventExecutingState::None,
                },
            ))
            .id();

        app.world_mut().spawn((
            Id(1),
            PlayerId(1),
            Position { x: 1, y: 0 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            empty_effects(),
            test_stats(),
        ));

        app.world_mut().spawn((
            Id(2),
            PlayerId(2),
            Position { x: 3, y: 0 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            empty_effects(),
            test_stats(),
        ));

        let scorer_entity = {
            let mut commands = app.world_mut().commands();
            spawn_scorer(&VisibleTargetScorer, &mut commands, npc_entity)
        };
        app.world_mut().flush();

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert_eq!(visible_target.target, 1);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), NORMAL_SCORE / 100.0);
    }
}

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct SpoilTargetScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SpoilTarget;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct StealTargetScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct StealTarget;

#[derive(Debug, Clone, Component)]
pub struct ItemsToSteal {
    pub item_classes: Vec<String>,
}

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct NoTargetScorer;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct TorchTargetScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct TorchTarget;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct CastSpellTarget;

// Necromancer
#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Wander;

// Necromancer
#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct ChaseAndCast {
    pub start_time: i32,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct RaiseDead;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct FleeToHome;

// Corpse targets for Necromancer
#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct VisibleCorpseScorer;

// Corpse targets for Necromancer
#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct FleeScorer;

#[derive(Debug, Component)]
pub struct VisibleCorpse {
    pub corpse: i32,
}

impl VisibleCorpse {
    pub fn new(corpse: i32) -> Self {
        Self { corpse }
    }
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct NpcMoveTo;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct NpcMoveToTarget;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct NpcMoveNearTarget;

pub struct NPCPlugin;

impl Plugin for NPCPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                set_attack_target_system.in_set(BigBrainSet::Actions),
                attack_target_system.in_set(BigBrainSet::Actions),
                cast_target_system.in_set(BigBrainSet::Actions),
                set_torch_target_system.in_set(BigBrainSet::Actions),
                set_spoil_target_system.in_set(BigBrainSet::Actions),
                set_steal_target_system.in_set(BigBrainSet::Actions),
                set_corpse_target_system.in_set(BigBrainSet::Actions),
                set_home_system.in_set(BigBrainSet::Actions),
                raise_dead_system.in_set(BigBrainSet::Actions),
                move_to_system.in_set(BigBrainSet::Actions),
                move_to_target_system.in_set(BigBrainSet::Actions),
                move_near_target_system.in_set(BigBrainSet::Actions),
                hide_action_system.in_set(BigBrainSet::Actions),
            )
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                spoil_target_action_system.in_set(BigBrainSet::Actions),
                torch_target_action_system.in_set(BigBrainSet::Actions),
                steal_target_action_system.in_set(BigBrainSet::Actions),
                cast_spell_target_system.in_set(BigBrainSet::Actions),
            )
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                target_scorer_system.in_set(BigBrainSet::Scorers),
                no_target_scorer_system.in_set(BigBrainSet::Scorers),
                nearby_corpses_scorer_system.in_set(BigBrainSet::Scorers),
                flee_scorer_system.in_set(BigBrainSet::Scorers),
                spoil_target_scorer_system.in_set(BigBrainSet::Scorers),
                steal_target_scorer_system.in_set(BigBrainSet::Scorers),
                torch_target_scorer_system.in_set(BigBrainSet::Scorers),
            )
                .run_if(in_state(AppState::Running)),
        );
    }
}

// SCORER SYSTEMS

pub fn target_scorer_system(
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    mut npc_query: Query<
        (
            &PlayerId,
            &Position,
            &Template,
            &Viewshed,
            &mut VisibleTarget,
            Option<&mut TaskTarget>,
            &Stats,
            &EventExecuting
        ),
        With<SubclassNPC>,
    >,
    target_query: Query<(
        &Id,
        &PlayerId,
        &Position,
        &State,
        &Class,
        &Subclass,
        &Effects,
        &Stats,
    )>, // Added April 2025 to prevent targeting NPCs
    fortified_query: Query<&Fortified>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<VisibleTargetScorer>>,
) {
    if game_tick.0 % TICKS_PER_SEC != 0 {
        return;
    }

    for (Actor(actor), mut score, span) in &mut query {
        let obj_id = entity_map.get_obj_by_entity(*actor);
        let Ok((
            _npc_player_id,
            npc_pos,
            npc_template_name,
            npc_viewshed,
            mut npc_visible_target,
            npc_task_target,
            npc_stats,
            event_executing,
        )) = npc_query.get_mut(*actor)
        else {
            span.span().in_scope(|| {
                npc_warn!(*actor, obj_id, None, "Cannot find npc query");
            });
            continue;
        };

        if event_executing.state == EventExecutingState::Executing {
            span.span().in_scope(|| {
                npc_debug!(
                    *actor,
                    obj_id,
                    Some(npc_template_name.0.as_str()),
                    "Currently executing event, skipping target scoring"
                );
            });
            continue;
        }

        let mut selected_target = NPCTarget {
            id: NO_TARGET,
            player_id: -1,
            pos: Position::default(),
            distance: u32::MAX,
            fortified: false,
        };

        let npc_template = templates
            .obj_templates
            .get_by_name_template(npc_template_name.0.clone(), npc_template_name.0.clone());
        let int = npc_template.int.unwrap_or("mindless".to_string());
        let aggression = npc_template.aggression.unwrap_or("medium".to_string());

        // Passive NPCs never target players
        if is_passive(&aggression) {
            score.set(0.0);
            continue;
        }

        for (
            target_id,
            target_player,
            target_pos,
            target_state,
            target_class,
            target_subclass,
            target_effects,
            target_stats,
        ) in target_query.iter()
        {
            let mut target_fortified = false;
            let target_stronger = false;

            span.span().in_scope(|| {
                npc_debug!(
                    *actor,
                    obj_id,
                    Some(npc_template_name.0.as_str()),
                    "Evaluating target player={} id={} class={} subclass={:?} state={:?}",
                    target_player.0,
                    target_id.0,
                    target_class.0,
                    target_subclass,
                    target_state
                );
            });

            if !player::is_player(target_player.0) {
                continue;
            }

            if Obj::is_dead(target_state) {
                continue;
            }

            // Skip POIs
            if target_class.0 == CLASS_POI {
                continue;
            }

            // Skip structures for mindless and animal int, TODO prioritized dangerous targets over structures for cunning
            //if target_class.0 == CLASS_STRUCTURE && (is_mindless(&int) || is_animal(&int)) {
            if target_class.0 == CLASS_STRUCTURE {
                continue;
            }

            span.span().in_scope(|| {
                npc_trace!(
                    *actor,
                    obj_id,
                    Some(npc_template_name.0.as_str()),
                    "npc_strength={} target_strength={}",
                    npc_stats.get_strength(),
                    target_stats.get_strength()
                );
            });
            // Check if target is weaker
            /*if npc_stats.get_strength() < target_stats.get_strength() {
                target_stronger = true;
            }*/

            span.span().in_scope(|| {
                npc_trace!(
                    *actor,
                    obj_id,
                    Some(npc_template_name.0.as_str()),
                    "is_fortified={}",
                    target_effects.has(Effect::Fortified)
                );
            });
            // Check if fortified
            if target_effects.has(Effect::Fortified) {
                target_fortified = true;
            }

            // Skip if npc is strategic and target is stronger and fortified
            /*if (target_fortified || target_subclass.equals(SUBCLASS_WALL))
                && is_strategic(&aggression)
            {
                continue;
            }*/

            let distance = Map::dist(*npc_pos, *target_pos);

            span.span().in_scope(|| {
                npc_trace!(
                    *actor,
                    obj_id,
                    Some(npc_template_name.0.as_str()),
                    "viewshed_range={} distance={} min_distance={}",
                    npc_viewshed.range,
                    distance,
                    selected_target.distance
                );
            });

            if npc_viewshed.range >= distance {
                if distance < selected_target.distance {
                    selected_target = NPCTarget {
                        id: target_id.0,
                        player_id: target_player.0,
                        pos: target_pos.clone(),
                        distance: distance,
                        fortified: target_fortified,
                    };
                }
            }
        }

        span.span().in_scope(|| {
            npc_debug!(
                *actor,
                obj_id,
                Some(npc_template_name.0.as_str()),
                "selected_target_fortified={}",
                selected_target.fortified
            );
        });
        if selected_target.fortified {
            span.span().in_scope(|| {
                npc_debug!(
                    *actor,
                    obj_id,
                    Some(npc_template_name.0.as_str()),
                    "Nearest target is fortified, changing target to fortification"
                );
            });

            let Some(fortified_entity) = entity_map.get_entity(selected_target.id) else {
                span.span().in_scope(|| {
                    npc_error!(
                        *actor,
                        obj_id,
                        Some(npc_template_name.0.as_str()),
                        "Cannot find entity from id={}",
                        selected_target.id
                    );
                });
                continue;
            };

            let Ok(fortifier) = fortified_query.get(fortified_entity) else {
                span.span().in_scope(|| {
                    npc_error!(
                        *actor,
                        obj_id,
                        Some(npc_template_name.0.as_str()),
                        "Cannot find fortified entity {:?}",
                        fortified_entity
                    );
                });
                continue;
            };

            npc_visible_target.target = fortifier.id;
            score.set(NORMAL_SCORE / 100.0);
        } else if selected_target.id != NO_TARGET {
            span.span().in_scope(|| {
                npc_info!(*actor, obj_id, Some(npc_template_name.0.as_str()), "Selected target_id={}", selected_target.id);
            });
            npc_visible_target.target = selected_target.id;
            score.set(NORMAL_SCORE / 100.0);
        } else {
            span.span().in_scope(|| {
                npc_debug!(*actor, obj_id, Some(npc_template_name.0.as_str()), "No target found");
            });
            npc_visible_target.target = NO_TARGET;
            score.set(0.0);
        }
    }
}

pub fn spoil_target_scorer_system(
    game_tick: Res<GameTick>,
    map: Res<Map>,
    mut npc_query: Query<
        (&PlayerId, &Position, &Inventory, &mut TaskTarget),
        (With<SubclassNPC>, Without<EventExecuting>),
    >,
    structure_query: Query<
        (&Id, &PlayerId, &Position, &State, &Effects),
        (With<ClassStructure>, Without<SubclassNPC>),
    >,
    blocking_query: Query<BaseQuery>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<SpoilTargetScorer>>,
) {
    if game_tick.0 % (TICKS_PER_SEC * 5) != 0 {
        return;
    }

    for (Actor(actor), mut score, _span) in &mut query {
        let Ok((npc_player_id, npc_pos, npc_inventory, mut npc_task_target)) =
            npc_query.get_mut(*actor)
        else {
            continue;
        };

        let mut selected_target = NPCTarget {
            id: NO_TARGET,
            player_id: -1,
            pos: Position::default(),
            distance: u32::MAX,
            fortified: false,
        };

        for (target_id, target_player, target_pos, target_state, target_effects) in
            structure_query.iter()
        {
            // Print all target attributes
            info!("Target attributes: {:?}", target_id.0);
            info!("Target player: {:?}", target_player.0);
            info!("Target pos: {:?}", target_pos);
            info!("Target state: {:?}", target_state);
            info!("Target effects: {:?}", target_effects);

            // Skip if target is dead
            if Obj::is_dead(target_state) {
                continue;
            }

            // Check if structure has food or drink items
            let food_item = npc_inventory.get_by_class(FOOD.to_owned());
            let drink_item = npc_inventory.get_by_class(DRINK.to_owned());

            // Skip if structure does not have food or drink items
            if food_item.is_none() && drink_item.is_none() {
                continue;
            }

            let distance = Map::dist(*npc_pos, *target_pos);

            if distance < selected_target.distance {
                selected_target = NPCTarget {
                    id: target_id.0,
                    player_id: target_player.0,
                    pos: target_pos.clone(),
                    distance: distance,
                    fortified: false,
                };
            }
        }

        info!("Selected target: {:?}", selected_target.id);

        if selected_target.id != NO_TARGET {
            let blocking_list = Obj::blocking_list_basequery(npc_player_id.0, &blocking_query);

            if let Some(path_result) = Map::find_path(
                *npc_pos,
                selected_target.pos,
                &map,
                selected_target.player_id,
                blocking_list.clone(),
                true,
                false,
                false,
                true,
            ) {
                let mut blocked = false;

                let (path, _c) = path_result;
                let next_pos = &path[1];

                for obj in blocking_list {
                    if obj.pos.x == next_pos.0
                        && obj.pos.y == next_pos.1
                        && obj.id.0 != selected_target.id
                    {
                        info!("Target is blocked by {:?}", obj.id);
                        blocked = true;
                        selected_target.id = obj.id.0;
                    }
                }

                if !blocked {
                    info!(
                        "Target is not blocked, setting target to {:?}",
                        selected_target.id
                    );
                    // Set blocker to task target to attack
                    npc_task_target.target = selected_target.id;
                    score.set(PRIORITY1_SCORE / 100.0);
                } else {
                    info!(
                        "Target is blocked, setting target to blocked object {:?}",
                        selected_target.id
                    );
                    // Set target to blocked object
                    npc_task_target.target = selected_target.id;
                    score.set(0.0);
                }
            } else {
                info!("No path found to target, setting target to no target");
                npc_task_target.target = NO_TARGET;
                score.set(0.0);
            }
        } else {
            info!("No torch target found, setting target to no target");
            npc_task_target.target = NO_TARGET;
            score.set(0.0);
        }
    }
}

pub fn steal_target_scorer_system(
    game_tick: Res<GameTick>,
    map: Res<Map>,
    mut npc_query: Query<
        (&PlayerId, &Position, &Inventory, &mut TaskTarget),
        (With<SubclassNPC>, Without<EventExecuting>),
    >,
    structure_query: Query<
        (&Id, &PlayerId, &Position, &State, &Effects),
        (With<ClassStructure>, Without<SubclassNPC>),
    >,
    blocking_query: Query<BaseQuery>,
    items_to_steal_query: Query<&ItemsToSteal>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<StealTargetScorer>>,
) {
    if game_tick.0 % (TICKS_PER_SEC * 5) != 0 {
        return;
    }

    for (Actor(actor), mut score, _span) in &mut query {
        let Ok((npc_player_id, npc_pos, npc_inventory, mut npc_task_target)) =
            npc_query.get_mut(*actor)
        else {
            continue;
        };

        let mut selected_target = NPCTarget {
            id: NO_TARGET,
            player_id: -1,
            pos: Position::default(),
            distance: u32::MAX,
            fortified: false,
        };

        for (target_id, target_player, target_pos, target_state, target_effects) in
            structure_query.iter()
        {
            // Print all target attributes
            info!("Target attributes: {:?}", target_id.0);
            info!("Target player: {:?}", target_player.0);
            info!("Target pos: {:?}", target_pos);
            info!("Target state: {:?}", target_state);
            info!("Target effects: {:?}", target_effects);

            // Skip if target is dead
            if Obj::is_dead(target_state) {
                continue;
            }

            // Check if any structures have items to steal
            let Ok(items_to_steal) = items_to_steal_query.get(*actor) else {
                info!("Target does not have defined items to steal, skipping");
                continue;
            };

            for item_class in items_to_steal.item_classes.iter() {
                let item = npc_inventory.get_by_class(item_class.to_owned());
                if let Some(item) = item {
                    info!("Target has item to steal: {:?}", item);

                    let distance = Map::dist(*npc_pos, *target_pos);

                    if distance < selected_target.distance {
                        selected_target = NPCTarget {
                            id: target_id.0,
                            player_id: target_player.0,
                            pos: target_pos.clone(),
                            distance: distance,
                            fortified: false,
                        };
                    }
                }
            }
        }

        info!("Selected target: {:?}", selected_target.id);

        if selected_target.id != NO_TARGET {
            let blocking_list = Obj::blocking_list_basequery(npc_player_id.0, &blocking_query);

            if let Some(path_result) = Map::find_path(
                *npc_pos,
                selected_target.pos,
                &map,
                selected_target.player_id,
                blocking_list.clone(),
                true,
                false,
                false,
                true,
            ) {
                let mut blocked = false;

                let (path, _c) = path_result;
                let next_pos = &path[1];

                for obj in blocking_list {
                    if obj.pos.x == next_pos.0
                        && obj.pos.y == next_pos.1
                        && obj.id.0 != selected_target.id
                    {
                        info!("Target is blocked by {:?}", obj.id);
                        blocked = true;
                        selected_target.id = obj.id.0;
                    }
                }

                if !blocked {
                    info!(
                        "Target is not blocked, setting target to {:?}",
                        selected_target.id
                    );
                    // Set blocker to task target to attack
                    npc_task_target.target = selected_target.id;
                    score.set(PRIORITY1_SCORE / 100.0);
                } else {
                    info!(
                        "Target is blocked, setting target to blocked object {:?}",
                        selected_target.id
                    );
                    // Set target to blocked object
                    npc_task_target.target = selected_target.id;
                    score.set(0.0);
                }
            } else {
                info!("No path found to target, setting target to no target");
                npc_task_target.target = NO_TARGET;
                score.set(0.0);
            }
        } else {
            info!("No torch target found, setting target to no target");
            npc_task_target.target = NO_TARGET;
            score.set(0.0);
        }
    }
}

pub fn torch_target_scorer_system(
    game_tick: Res<GameTick>,
    map: Res<Map>,
    mut npc_query: Query<
        (&PlayerId, &Position, &mut TaskTarget),
        (With<SubclassNPC>, Without<EventExecuting>),
    >,
    structure_query: Query<
        (&Id, &PlayerId, &Position, &State, &Effects),
        (With<ClassStructure>, Without<SubclassNPC>),
    >,
    blocking_query: Query<BaseQuery>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<TorchTargetScorer>>,
) {
    if game_tick.0 % (TICKS_PER_SEC * 5) != 0 {
        return;
    }

    for (Actor(actor), mut score, _span) in &mut query {
        let Ok((npc_player_id, npc_pos, mut npc_task_target)) = npc_query.get_mut(*actor) else {
            continue;
        };

        let mut selected_target = NPCTarget {
            id: NO_TARGET,
            player_id: -1,
            pos: Position::default(),
            distance: u32::MAX,
            fortified: false,
        };

        for (target_id, target_player, target_pos, target_state, target_effects) in
            structure_query.iter()
        {
            // Print all target attributes
            info!("Target attributes: {:?}", target_id.0);
            info!("Target player: {:?}", target_player.0);
            info!("Target pos: {:?}", target_pos);
            info!("Target state: {:?}", target_state);
            info!("Target effects: {:?}", target_effects);

            // Skip if target is dead
            if Obj::is_dead(target_state) {
                continue;
            }

            // Skip if target is already burning
            if target_effects.has(Effect::Burning) {
                info!("Target is burning, skipping");
                continue;
            }

            let distance = Map::dist(*npc_pos, *target_pos);

            if distance < selected_target.distance {
                selected_target = NPCTarget {
                    id: target_id.0,
                    player_id: target_player.0,
                    pos: target_pos.clone(),
                    distance: distance,
                    fortified: false,
                };
            }
        }

        info!("Selected target: {:?}", selected_target.id);

        if selected_target.id != NO_TARGET {
            let blocking_list = Obj::blocking_list_basequery(npc_player_id.0, &blocking_query);

            if let Some(path_result) = Map::find_path(
                *npc_pos,
                selected_target.pos,
                &map,
                selected_target.player_id,
                blocking_list.clone(),
                true,
                false,
                false,
                true,
            ) {
                let mut blocked = false;

                let (path, _c) = path_result;
                let next_pos = &path[1];

                for obj in blocking_list {
                    if obj.pos.x == next_pos.0
                        && obj.pos.y == next_pos.1
                        && obj.id.0 != selected_target.id
                    {
                        info!("Target is blocked by {:?}", obj.id);
                        blocked = true;
                        selected_target.id = obj.id.0;
                    }
                }

                if !blocked {
                    info!(
                        "Target is not blocked, setting target to {:?}",
                        selected_target.id
                    );
                    // Set blocker to task target to attack
                    npc_task_target.target = selected_target.id;
                    score.set(PRIORITY1_SCORE / 100.0);
                } else {
                    info!(
                        "Target is blocked, setting target to blocked object {:?}",
                        selected_target.id
                    );
                    // Set target to blocked object
                    npc_task_target.target = selected_target.id;
                    score.set(0.0);
                }
            } else {
                info!("No path found to target, setting target to no target");
                npc_task_target.target = NO_TARGET;
                score.set(0.0);
            }
        } else {
            info!("No torch target found, setting target to no target");
            npc_task_target.target = NO_TARGET;
            score.set(0.0);
        }
    }
}

pub fn nearby_corpses_scorer_system(
    game_tick: Res<GameTick>,
    mut npc_query: Query<
        (&Position, &Viewshed, &mut TaskTarget, &EventExecuting),
        With<SubclassNPC>,
    >,
    target_query: Query<ObjQuery>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<VisibleCorpseScorer>>,
) {
    if game_tick.0 % TICKS_PER_SEC == 0 {
        for (Actor(actor), mut score, _span) in &mut query {
            let Ok((npc_pos, npc_viewshed, mut npc_task_target, event_executing)) = npc_query.get_mut(*actor) else {
                error!("Nearby Corpses Scorer => Cannot find npc query for {:?}", *actor);
                continue;
            };

            // Skip if currently executing an event
            if event_executing.state == EventExecutingState::Executing {
                score.set(0.0);
                continue;
            }

            let mut min_distance = u32::MAX;
            let mut corpse_id = NO_TARGET;

            for target in target_query.iter() {
                if target.class.0 == CLASS_CORPSE.to_string() {
                    let distance = Map::dist(*npc_pos, *target.pos);

                    if npc_viewshed.range >= distance {
                        if distance < min_distance {
                            min_distance = distance;
                            corpse_id = target.id.0;
                        }
                    }
                }
            }

            if corpse_id != NO_TARGET {
                info!("Setting target to corpse {:?}", corpse_id);
                npc_task_target.target = corpse_id;

                score.set(PRIORITY2_SCORE / 100.0);
            } else {
                score.set(0.0);
            }
        }
    }
}

pub fn flee_scorer_system(
    game_tick: Res<GameTick>,
    minions_query: Query<&Minions>,
    state_query: Query<&State>,
    entity_map: Res<EntityObjMap>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<FleeScorer>>,
) {
    if game_tick.0 % (TICKS_PER_SEC * 5) == 0 {
        for (Actor(actor), mut score, _span) in &mut query {
            if let Ok(minions) = minions_query.get(*actor) {
                let mut minions_dead = true;

                for minion_id in minions.ids.iter() {
                    let Some(minion_entity) = entity_map.get_entity(*minion_id) else {
                        continue;
                    };

                    if let Ok(minion_state) = state_query.get(minion_entity) {
                        if *minion_state != State::Dead {
                            minions_dead = false;
                        }
                    }
                }

                if minions_dead {
                    score.set(PRIORITY1_SCORE / 100.0);
                } else {
                    score.set(0.0);
                }
            }
        }
    }
}

pub fn no_target_scorer_system(
    target_query: Query<&VisibleTarget>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<NoTargetScorer>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if let Ok(target) = target_query.get(*actor) {
            if target.target == NO_TARGET {
                score.set(0.9);
            } else {
                score.set(0.0);
            }
        }
    }
}

// ACTION SYSTEMS

pub fn set_attack_target_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut map_events: ResMut<MapEvents>,
    visible_target_query: Query<(&VisibleTarget, &Id), With<SubclassNPC>>,
    mut query: Query<(&Actor, &mut ActionState, &SetAttackTarget)>,
    mut alerted_npcs: Local<std::collections::HashSet<Entity>>,
) {
    // Clear alert state for any NPC that has lost its target since last tick
    alerted_npcs.retain(|entity| {
        visible_target_query
            .get(*entity)
            .map_or(false, |(vt, _)| vt.target != NO_TARGET)
    });

    for (Actor(actor), mut state, _set_attack_destination) in &mut query {
        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting attack target...");
                let Ok((visible_target, npc_id)) = visible_target_query.get(*actor) else {
                    continue;
                };

                npc_info!(*actor, None, None, "Setting attack target to {:?}", visible_target.target);
                commands.entity(*actor).insert(Target {
                    id: visible_target.target,
                });

                // Emit alert "!" once per engagement
                if !alerted_npcs.contains(actor) {
                    alerted_npcs.insert(*actor);
                    map_events.new(
                        npc_id.0,
                        game_tick.0,
                        VisibleEvent::SpeechEvent {
                            speech: "!".to_string(),
                            intensity: 3,
                        },
                    );
                }

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(*actor, None, None, "Set Attack Destination action was cancelled. Considering this a failure.");
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_torch_target_system(
    mut commands: Commands,
    task_target_query: Query<&mut TaskTarget>,
    mut query: Query<(&Actor, &mut ActionState, &SetTorchTarget)>,
) {
    for (Actor(actor), mut state, _set_attack_destination) in &mut query {
        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting torch target...");
                let Ok(task_target) = task_target_query.get(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                commands.entity(*actor).insert(Target {
                    id: task_target.target,
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(*actor, None, None, "Set Torch Target action was cancelled. Considering this a failure.");
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_spoil_target_system(
    mut commands: Commands,
    task_target_query: Query<&mut TaskTarget>,
    mut query: Query<(&Actor, &mut ActionState, &SetSpoilTarget)>,
) {
    for (Actor(actor), mut state, _set_spoil_target) in &mut query {
        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting spoil target...");
                let Ok(task_target) = task_target_query.get(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                commands.entity(*actor).insert(Target {
                    id: task_target.target,
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(*actor, None, None, "Set Spoil Target action was cancelled. Considering this a failure.");
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_steal_target_system(
    mut commands: Commands,
    task_target_query: Query<&mut TaskTarget>,
    mut query: Query<(&Actor, &mut ActionState, &SetStealTarget)>,
) {
    for (Actor(actor), mut state, _set_steal_target) in &mut query {
        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting steal target...");
                let Ok(task_target) = task_target_query.get(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                commands.entity(*actor).insert(Target {
                    id: task_target.target,
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(*actor, None, None, "Set Steal Target action was cancelled. Considering this a failure.");
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_corpse_target_system(
    mut commands: Commands,
    task_target_query: Query<&mut TaskTarget>,
    mut query: Query<(&Actor, &mut ActionState, &SetCorpseTarget)>,
) {
    for (Actor(actor), mut state, _set_corpse_target) in &mut query {
        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting corpse target...");
                let Ok(task_target) = task_target_query.get(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                commands.entity(*actor).insert(Target {
                    id: task_target.target,
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(*actor, None, None, "Set Corpse Target action was cancelled. Considering this a failure.");
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_home_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut map_events: ResMut<MapEvents>,
    home_query: Query<(&Id, &Home)>,
    mut query: Query<(&Actor, &mut ActionState, &SetHome)>,
) {
    for (Actor(actor), mut state, _set_home) in &mut query {
        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting home destination...");

                let Ok((obj_id, home)) = home_query.get(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                commands
                    .entity(*actor)
                    .insert(Destination { pos: home.pos });

                let speech_event = VisibleEvent::SpeechEvent {
                    speech: "My minions fall, but I will get my revenge!".to_string(),
                    intensity: 2,
                };

                map_events.new(obj_id.0, game_tick.0 + 4, speech_event);

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(*actor, None, None, "Set Home action was cancelled. Considering this a failure.");
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn move_to_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    dest_query: Query<&Destination>,
    obj_query: Query<(&Id, &PlayerId, &Position, &Class, &Subclass, &Stats)>,
    state_query: Query<&mut State>,
    npc_effects_query: Query<&Effects>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut action_query: Query<(&Actor, &mut ActionState, &NpcMoveTo, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _move_to, span) in &mut action_query {
        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    npc_debug!(*actor, obj_id, None, "MoveTo requested");
                });
                let Some(obj_id_val) = obj_id else {
                    span.span().in_scope(|| {
                        npc_error!(*actor, None, None, "Cannot find obj id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(player_id) = ids.get_player(obj_id_val) else {
                    span.span().in_scope(|| {
                        npc_error!(*actor, obj_id, None, "Cannot find player id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let blocking_list = Obj::blocking_list(player_id, actor, &obj_query, &state_query);

                let Ok(destination) = dest_query.get(*actor) else {
                    span.span().in_scope(|| {
                        npc_error!(*actor, obj_id, None, "No Destination component");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                // NPC is stunned, skip execution
                if let Ok(effects) = npc_effects_query.get(*actor) {
                    if effects.has(Effect::Stunned) {
                        continue;
                    }
                }

                let Ok((id, _player_id, pos, _class, _subclass, _stats)) = obj_query.get(*actor)
                else {
                    span.span().in_scope(|| {
                        npc_error!(*actor, obj_id, None, "Cannot get obj query");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                if *pos != destination.pos {
                    if let Some(path_result) = Map::find_fast_path(
                        *pos,
                        destination.pos,
                        &map,
                        player_id,
                        blocking_list,
                        true,
                        false,
                        false,
                        false,
                    ) {
                        span.span().in_scope(|| {
                            npc_trace!(
                                *actor,
                                obj_id,
                                None,
                                "Path found, length={}",
                                path_result.0.len()
                            );
                        });

                        let (path, _c) = path_result;
                        let next_pos = &path[1];

                        span.span().in_scope(|| {
                            npc_trace!(*actor, obj_id, None, "Next pos=({}, {})", next_pos.0, next_pos.1);
                        });

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Moving,
                        });

                        // Add Move Event
                        let move_event = VisibleEvent::MoveEvent {
                            src: *pos,
                            dst: Position {
                                x: next_pos.0,
                                y: next_pos.1,
                            },
                        };

                        map_events.new(
                            id.0,
                            game_tick.0 + 48, // in the future
                            move_event,
                        );

                        let mut event_executing = event_executing_query
                            .get_mut(*actor)
                            .expect("Missing EventExecuting component");
                        event_executing.state = EventExecutingState::Executing;
                    } else {
                        span.span().in_scope(|| {
                            npc_debug!(*actor, obj_id, None, "Cannot find path to destination");
                        });
                        *state = ActionState::Failure
                    }
                }

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                span.span().in_scope(|| {
                    npc_trace!(*actor, obj_id, None, "MoveTo executing");
                });
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                span.span().in_scope(|| {
                    npc_trace!(*actor, obj_id, None, "Event state={:?}", event_executing.state);
                });
                if !event_executing.state.is_finished() {
                    span.span().in_scope(|| {
                        npc_trace!(*actor, obj_id, None, "MoveTo still executing");
                    });
                    continue;
                }

                let Some(obj_id_val) = obj_id else {
                    span.span().in_scope(|| {
                        npc_error!(*actor, None, None, "Cannot find obj id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(player_id) = ids.get_player(obj_id_val) else {
                    span.span().in_scope(|| {
                        npc_error!(*actor, obj_id, None, "Cannot find player id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let blocking_list = Obj::blocking_list(player_id, actor, &obj_query, &state_query);

                if let Ok((id, _player_id, pos, _class, _subclass, _stats)) = obj_query.get(*actor)
                {
                    let Ok(destination) = dest_query.get(*actor) else {
                        span.span().in_scope(|| {
                            npc_error!(*actor, obj_id, None, "No Destination component");
                        });
                        *state = ActionState::Failure;
                        continue;
                    };

                    if *pos != destination.pos {
                        // Check if moving event failed
                        if event_executing.state.is_failed() {
                            span.span().in_scope(|| {
                                npc_warn!(*actor, obj_id, None, "Moving event failed");
                            });
                            *state = ActionState::Failure;
                            continue;
                        }

                        let Some(path_result) = Map::find_fast_path(
                            *pos,
                            destination.pos,
                            &map,
                            player_id,
                            blocking_list,
                            true,
                            false,
                            false,
                            false,
                        ) else {
                            span.span().in_scope(|| {
                                npc_trace!(*actor, obj_id, None, "Cannot find path to destination");
                            });
                            *state = ActionState::Failure;
                            continue;
                        };

                        span.span().in_scope(|| {
                            npc_trace!(
                                *actor,
                                obj_id,
                                None,
                                "Path found, length={}",
                                path_result.0.len()
                            );
                        });

                        let (path, _c) = path_result;
                        let next_pos = &path[1];

                        span.span().in_scope(|| {
                            npc_trace!(*actor, obj_id, None, "Next pos=({}, {})", next_pos.0, next_pos.1);
                        });

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Moving,
                        });

                        // Add Move Event
                        let move_event = VisibleEvent::MoveEvent {
                            src: *pos,
                            dst: Position {
                                x: next_pos.0,
                                y: next_pos.1,
                            },
                        };

                        // Add a random factor to the move duration to prevent all npcs from moving at the same time
                        let random_factor = rand::thread_rng().gen_range(0.85..1.15);
                        let move_duration = (48 as f32 * random_factor) as i32;

                        map_events.new(
                            id.0,
                            game_tick.0 + move_duration, // in the future
                            move_event,
                        );

                        // Set EventExecutingState to Executing
                        event_executing.state = EventExecutingState::Executing;
                    } else {
                        span.span().in_scope(|| {
                            npc_debug!(*actor, obj_id, None, "Adjacent to destination, success");
                        });
                        *state = ActionState::Success;
                    }
                }
            }
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    npc_debug!(*actor, obj_id, None, "Cancelling MoveTo");
                });

                let Some(npc_obj_id) = obj_id else {
                    span.span().in_scope(|| {
                        npc_error!(*actor, None, None, "Cannot find obj id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let event_type = GameEventType::CancelAllMapEvents { obj_id: npc_obj_id };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn move_to_target_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    mut obj_query: Query<ObjStatQuery>,
    target_query: Query<&Target>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut query: Query<(&Actor, &mut ActionState, &NpcMoveToTarget, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _move_to_target, span) in &mut query {
        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                span.span().in_scope(|| {
                    npc_debug!(*actor, obj_id, None, "MoveToTarget action requested");
                });

                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(npc_player_id) = ids.get_player(npc_id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find player id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(target) = target_query.get(*actor) else {
                    npc_error!(*actor, obj_id, None, "Cannot find target");
                    *state = ActionState::Failure;
                    continue;
                };

                npc_debug!(*actor, obj_id, None, "Target: {:?}", target.id);

                let Some(target_entity) = entity_map.get_entity(target.id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find entity for {:?}", target.id);
                    *state = ActionState::Failure;
                    continue;
                };

                npc_debug!(*actor, obj_id, None, "Target Entity: {:?}", target_entity);

                // Have to get the list of collision positions before querying the npc and target
                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);

                let entities = [*actor, target_entity];

                let Ok([mut npc, target]) = obj_query.get_many_mut(entities) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find entities {:?}", entities);
                    *state = ActionState::Failure;
                    continue;
                };

                let reached_destination = Map::is_adjacent_including_source(*npc.pos, *target.pos);

                if !reached_destination {
                    // Check if NPC is stunned and cannot move
                    if npc.effects.has(Effect::Stunned) {
                        npc_debug!(*actor, obj_id, None, "NPC is stunned");
                        continue;
                    }

                    // Get NPC speed
                    let mut npc_speed = 1;

                    if let Some(npc_base_speed) = npc.stats.base_speed {
                        npc_speed = npc_base_speed;
                    }

                    let effect_speed_mod = npc.effects.get_speed_effects(&templates);

                    // Add a random factor to the move duration to prevent all npcs from moving at the same time
                    let random_factor = rand::thread_rng().gen_range(0.85..1.15);
                    let move_duration = (BASE_MOVE_TICKS
                        * (BASE_SPEED / npc_speed as f32)
                        * (1.0 / effect_speed_mod)
                        * random_factor) as i32;

                    let Some(path_result) = Map::find_fast_path(
                        *npc.pos,
                        *target.pos,
                        &map,
                        npc_player_id,
                        collision_list,
                        true,
                        false,
                        false,
                        true, // Allow move onto position with transport
                    ) else {
                        npc_debug!(*actor, obj_id, None, "No path found");
                        *state = ActionState::Failure;
                        continue;
                    };

                    npc_trace!(*actor, obj_id, None, "Follower path: {:?}", path_result);

                    let (path, _c) = path_result;
                    let next_pos = &path[1];

                    npc_trace!(*actor, obj_id, None, "Next pos: {:?}", next_pos);

                    // Add State Change Event to Moving
                    *npc.state = State::Moving;

                    commands.trigger(StateChange {
                        entity: *actor,
                        new_state: State::Moving,
                    });

                    // Add Move Event
                    let move_event = VisibleEvent::MoveEvent {
                        src: *npc.pos,
                        dst: Position {
                            x: next_pos.0,
                            y: next_pos.1,
                        },
                    };

                    map_events.new(npc.id.0, game_tick.0 + move_duration, move_event);

                    let mut event_executing = event_executing_query
                        .get_mut(*actor)
                        .expect("Missing EventExecuting component");
                    event_executing.state = EventExecutingState::Executing;
                }

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                span.span().in_scope(|| {
                    npc_trace!(*actor, obj_id, None, "MoveToTarget executing");
                });
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                span.span().in_scope(|| {
                    npc_trace!(*actor, obj_id, None, "Event state={:?}", event_executing.state);
                });
                if !event_executing.state.is_finished() {
                    span.span().in_scope(|| {
                        npc_trace!(*actor, obj_id, None, "MoveToTarget still executing");
                    });
                    continue;
                }

                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(npc_player_id) = ids.get_player(npc_id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find player id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(target) = target_query.get(*actor) else {
                    npc_error!(*actor, obj_id, None, "Cannot find target");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(target.id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find entity for {:?}", target.id);
                    *state = ActionState::Failure;
                    continue;
                };

                // Have to get the list of collision positions before querying the npc and target
                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);

                let entities = [*actor, target_entity];

                let Ok([mut npc, target]) = obj_query.get_many_mut(entities) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find entities {:?}", entities);
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if NPC is stunned and cannot move
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                // Get NPC speed
                let mut npc_speed = 1;

                if let Some(npc_base_speed) = npc.stats.base_speed {
                    npc_speed = npc_base_speed;
                }

                let effect_speed_mod = npc.effects.get_speed_effects(&templates);

                // Add a random factor to the move duration to prevent all npcs from moving at the same time
                let random_factor = rand::thread_rng().gen_range(0.75..1.25);
                let move_duration = (BASE_MOVE_TICKS
                    * (BASE_SPEED / npc_speed as f32)
                    * (1.0 / effect_speed_mod)
                    * random_factor) as i32;

                let reached_destination = Map::is_adjacent_including_source(*npc.pos, *target.pos);

                if !reached_destination {
                    // Check if moving event failed
                    if event_executing.state.is_failed() {
                        span.span().in_scope(|| {
                            npc_warn!(*actor, obj_id, None, "Moving event failed");
                        });
                        *state = ActionState::Failure;
                        continue;
                    }

                    let Some(path_result) = Map::find_fast_path(
                        *npc.pos,
                        *target.pos,
                        &map,
                        npc_player_id,
                        collision_list,
                        true,
                        false,
                        false,
                        true, // Allow move onto position with transport
                    ) else {
                        npc_debug!(*actor, obj_id, None, "No path found");
                        *state = ActionState::Failure;
                        continue;
                    };

                    npc_trace!(*actor, obj_id, None, "Follower path: {:?}", path_result);

                    let (path, _c) = path_result;
                    let next_pos = &path[1];

                    npc_trace!(*actor, obj_id, None, "Next pos: {:?}", next_pos);

                    // Add State Change Event to Moving
                    *npc.state = State::Moving;

                    commands.trigger(StateChange {
                        entity: *actor,
                        new_state: State::Moving,
                    });

                    // Add Move Event
                    let move_event = VisibleEvent::MoveEvent {
                        src: *npc.pos,
                        dst: Position {
                            x: next_pos.0,
                            y: next_pos.1,
                        },
                    };

                    map_events.new(npc.id.0, game_tick.0 + move_duration, move_event);

                    // Set EventExecutingState to Executing
                    event_executing.state = EventExecutingState::Executing;
                } else {
                    span.span().in_scope(|| {
                        npc_debug!(*actor, obj_id, None, "Adjacent to destination, success");
                    });
                    *state = ActionState::Success;
                }
            }
            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                span.span().in_scope(|| {
                    npc_debug!(*actor, obj_id, None, "Cancelling MoveToTarget");
                });

                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    span.span().in_scope(|| {
                        npc_error!(*actor, None, None, "Cannot find obj id");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let event_type = GameEventType::CancelAllMapEvents {
                    obj_id: npc_id,
                };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type: event_type,
                };

                game_events.insert(event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn move_near_target_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    mut obj_query: Query<ObjStatQuery>,
    target_query: Query<&Target>,
    event_completed: Query<&EventCompleted>,
    mut query: Query<(&Actor, &mut ActionState, &NpcMoveNearTarget)>,
) {
    for (Actor(actor), mut state, move_near_target) in &mut query {
        match *state {
            ActionState::Requested => {
                let obj_id = entity_map.get_obj_by_entity(*actor);
                npc_info!(*actor, obj_id, None, "MoveNearTarget action requested");
                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(npc_player_id) = ids.get_player(npc_id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find player id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(target) = target_query.get(*actor) else {
                    npc_error!(*actor, obj_id, None, "Cannot find target");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(target.id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find entity for {:?}", target.id);
                    *state = ActionState::Failure;
                    continue;
                };

                // Have to get the list of collision positions before querying the npc and target
                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);

                let entities = [*actor, target_entity];

                let Ok([mut npc, target]) = obj_query.get_many_mut(entities) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find entities {:?}", entities);
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if NPC is stunned and cannot move
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                // Get NPC speed
                let mut npc_speed = 1;

                if let Some(npc_base_speed) = npc.stats.base_speed {
                    npc_speed = npc_base_speed;
                }

                let effect_speed_mod = npc.effects.get_speed_effects(&templates);

                // Add a random factor to the move duration to prevent all npcs from moving at the same time
                let random_factor = rand::thread_rng().gen_range(0.85..1.15);
                let move_duration = (BASE_MOVE_TICKS
                    * (BASE_SPEED / npc_speed as f32)
                    * (1.0 / effect_speed_mod)
                    * random_factor) as i32;

                let target_dist = Map::dist(*npc.pos, *target.pos);

                if target_dist == 2 {
                    npc_info!(*actor, obj_id, None, "NPC {:?} is in range of target {:?}", npc_id, target.id);
                    *state = ActionState::Success;
                } else if target_dist > 2 {
                    npc_info!(*actor, obj_id, None, "NPC {:?} is too far from target {:?}", npc_id, target.id);
                    // Check if NPC is stunned and cannot move
                    if npc.effects.has(Effect::Stunned) {
                        npc_debug!(*actor, obj_id, None, "NPC is stunned");
                        continue;
                    }

                    let Some(path_result) = Map::find_fast_path(
                        *npc.pos,
                        *target.pos,
                        &map,
                        npc_player_id,
                        collision_list,
                        true,
                        false,
                        false,
                        true, // Allow move onto position with transport
                    ) else {
                        npc_debug!(*actor, obj_id, None, "No path found");
                        *state = ActionState::Failure;
                        continue;
                    };

                    npc_trace!(*actor, obj_id, None, "Follower path: {:?}", path_result);

                    let (path, _c) = path_result;
                    let next_pos = &path[1];

                    npc_trace!(*actor, obj_id, None, "Next pos: {:?}", next_pos);

                    // Add State Change Event to Moving
                    *npc.state = State::Moving;

                    commands.trigger(StateChange {
                        entity: *actor,
                        new_state: State::Moving,
                    });

                    // Add Move Event
                    let move_event = VisibleEvent::MoveEvent {
                        src: *npc.pos,
                        dst: Position {
                            x: next_pos.0,
                            y: next_pos.1,
                        },
                    };

                    map_events.new(npc.id.0, game_tick.0 + move_duration, move_event);

                    *state = ActionState::Executing;
                } else {
                    npc_info!(*actor, obj_id, None, "NPC {:?} is too close to target {:?}", npc_id, target.id);
                    let neighbour_tiles = Map::get_neighbour_tiles(
                        npc.pos.x,
                        npc.pos.y,
                        &map,
                        npc.player_id.0,
                        &collision_list,
                        true,
                        false,
                        false,
                        false,
                        MapPos(npc.pos.x, npc.pos.y),
                    );

                    let mut selected_pos_list = Vec::new();

                    for (map_pos, _movement_cost) in neighbour_tiles.iter() {
                        let dist = Map::dist(
                            Position {
                                x: map_pos.0,
                                y: map_pos.1,
                            },
                            *target.pos,
                        );

                        if dist == 2 {
                            selected_pos_list.push(map_pos.clone());
                        }
                    }

                    if !selected_pos_list.is_empty() {
                        npc_trace!(*actor, obj_id, None, "Selected pos list: {:?}", selected_pos_list);

                        // Randomly select a pos from list
                        let mut rng = rand::thread_rng();
                        let next_pos =
                            selected_pos_list[rng.gen_range(0..selected_pos_list.len())].clone();

                        // Add State Change Event to Moving
                        *npc.state = State::Moving;

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Moving,
                        });

                        // Add Move Event
                        let move_event = VisibleEvent::MoveEvent {
                            src: *npc.pos,
                            dst: Position {
                                x: next_pos.0,
                                y: next_pos.1,
                            },
                        };

                        map_events.new(npc.id.0, game_tick.0 + move_duration, move_event);

                        *state = ActionState::Executing;
                    } else {
                        npc_debug!(*actor, obj_id, None, "No valid positions found");
                        *state = ActionState::Success;
                    }
                }
            }
            ActionState::Executing => {
                let obj_id = entity_map.get_obj_by_entity(*actor);

                // Check if the moving event is still executing
                let Ok(_event) = event_completed.get(*actor) else {
                    npc_trace!(*actor, obj_id, None, "Moving event still executing, waiting for completed component");
                    continue;
                };

                // Remove EventExecuting & MovingEventCompleted

                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(npc_player_id) = ids.get_player(npc_id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find player id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(target) = target_query.get(*actor) else {
                    npc_error!(*actor, obj_id, None, "Cannot find target");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(target.id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find entity for {:?}", target.id);
                    *state = ActionState::Failure;
                    continue;
                };

                // Have to get the list of collision positions before querying the npc and target
                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);

                let entities = [*actor, target_entity];

                let Ok([mut npc, target]) = obj_query.get_many_mut(entities) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find entities {:?}", entities);
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if NPC is stunned and cannot move
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                // Get NPC speed
                let mut npc_speed = 1;

                if let Some(npc_base_speed) = npc.stats.base_speed {
                    npc_speed = npc_base_speed;
                }

                let effect_speed_mod = npc.effects.get_speed_effects(&templates);

                // Add a random factor to the move duration to prevent all npcs from moving at the same time
                let random_factor = rand::thread_rng().gen_range(0.85..1.15);
                let move_duration = (BASE_MOVE_TICKS
                    * (BASE_SPEED / npc_speed as f32)
                    * (1.0 / effect_speed_mod)
                    * random_factor) as i32;

                let target_dist = Map::dist(*npc.pos, *target.pos);

                if target_dist == 2 {
                    npc_info!(*actor, obj_id, None, "NPC {:?} is in range of target {:?}", npc_id, target.id);
                    *state = ActionState::Success;
                } else if target_dist > 2 {
                    npc_info!(*actor, obj_id, None, "NPC {:?} is too far from target {:?}", npc_id, target.id);
                    let Ok(_event) = event_completed.get(*actor) else {
                        npc_trace!(*actor, obj_id, None, "MovingNearTarget event still executing, waiting for completed component");
                        continue;
                    };

                    // Check if NPC is stunned and cannot move
                    if npc.effects.has(Effect::Stunned) {
                        npc_debug!(*actor, obj_id, None, "NPC is stunned");
                        continue;
                    }

                    let Some(path_result) = Map::find_fast_path(
                        *npc.pos,
                        *target.pos,
                        &map,
                        npc_player_id,
                        collision_list,
                        true,
                        false,
                        false,
                        true, // Allow move onto position with transport
                    ) else {
                        npc_debug!(*actor, obj_id, None, "No path found");
                        *state = ActionState::Failure;
                        continue;
                    };

                    npc_trace!(*actor, obj_id, None, "Follower path: {:?}", path_result);

                    let (path, _c) = path_result;
                    let next_pos = &path[1];

                    npc_trace!(*actor, obj_id, None, "Next pos: {:?}", next_pos);

                    // Add State Change Event to Moving
                    *npc.state = State::Moving;

                    commands.trigger(StateChange {
                        entity: *actor,
                        new_state: State::Moving,
                    });

                    // Add Move Event
                    let move_event = VisibleEvent::MoveEvent {
                        src: *npc.pos,
                        dst: Position {
                            x: next_pos.0,
                            y: next_pos.1,
                        },
                    };

                    map_events.new(npc.id.0, game_tick.0 + move_duration, move_event);

                    *state = ActionState::Executing;
                } else {
                    npc_info!(*actor, obj_id, None, "NPC {:?} is too close to target {:?}", npc_id, target.id);

                    let neighbour_tiles = Map::get_neighbour_tiles(
                        npc.pos.x,
                        npc.pos.y,
                        &map,
                        npc.player_id.0,
                        &collision_list,
                        true,
                        false,
                        false,
                        false,
                        MapPos(npc.pos.x, npc.pos.y),
                    );

                    let mut selected_pos_list = Vec::new();

                    for (map_pos, _movement_cost) in neighbour_tiles.iter() {
                        let dist = Map::dist(
                            Position {
                                x: map_pos.0,
                                y: map_pos.1,
                            },
                            *target.pos,
                        );

                        if dist == 2 {
                            selected_pos_list.push(map_pos.clone());
                        }
                    }

                    if !selected_pos_list.is_empty() {
                        npc_trace!(*actor, obj_id, None, "Selected pos list: {:?}", selected_pos_list);

                        // Randomly select a pos from list
                        let mut rng = rand::thread_rng();
                        let next_pos =
                            selected_pos_list[rng.gen_range(0..selected_pos_list.len())].clone();

                        // Add State Change Event to Moving
                        *npc.state = State::Moving;

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Moving,
                        });

                        // Add Move Event
                        let move_event = VisibleEvent::MoveEvent {
                            src: *npc.pos,
                            dst: Position {
                                x: next_pos.0,
                                y: next_pos.1,
                            },
                        };

                        map_events.new(npc.id.0, game_tick.0 + move_duration, move_event);

                        *state = ActionState::Executing;
                    } else {
                        npc_debug!(*actor, obj_id, None, "No valid positions found");
                        *state = ActionState::Success;
                    }
                }
            }
            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                npc_debug!(*actor, Some(npc_id), None, "MoveNearTarget action was cancelled. Considering this a failure.");

                cancel_npc_events(npc_id, game_tick.0, &mut ids, &mut game_events);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn attack_target_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    mut game_events: ResMut<GameEvents>,
    mut player_stats: ResMut<PlayerStats>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut visible_target_query: Query<&mut VisibleTarget>,
    mut npc_query: Query<CombatQuery, With<SubclassNPC>>,
    mut target_query: Query<CombatQuery, Without<SubclassNPC>>,
    mut query: Query<(&Actor, &mut ActionState, &AttackTarget)>,
) {
    for (Actor(actor), mut state, _chase_attack) in &mut query {
        match *state {
            ActionState::Requested => {
                let Ok(mut npc) = npc_query.get_mut(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                let obj_id = Some(npc.id.0);
                let npc_name = Some(npc.template.0.as_str());

                npc_info!(*actor, obj_id, npc_name, "AttackTarget action requested");

                let Ok(visible_target) = visible_target_query.get_mut(*actor) else {
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(visible_target.target) else {
                    npc_error!(*actor, obj_id, npc_name, "Cannot find target entity {:?}", visible_target.target);
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(mut target) = target_query.get_mut(target_entity) else {
                    npc_error!(*actor, obj_id, npc_name, "Cannot find target entity {:?}", target_entity);
                    *state = ActionState::Failure;
                    continue;
                };

                // Get NPC speed
                let npc_speed = npc.stats.base_speed.unwrap_or(1) * TICKS_PER_SEC;

                // Check if target is fortified
                if target.effects.has(Effect::Fortified) {
                    /* let Ok(fortification) = fortified_query.get(target.entity) else {
                        npc_error!(*actor, obj_id, npc_name, "Query failed to find entity: {:?}", target.entity);
                        continue;
                    };

                    npc_debug!(*actor, obj_id, npc_name, "Updating target to {:?}", fortification.id);
                    visible_target.target = fortification.id;*/
                    npc_debug!(*actor, obj_id, npc_name, "Cannot attack fortified obj");
                    *state = ActionState::Success;
                    continue;
                }

                if target.stats.hp <= 0 || Obj::is_dead(&target.state) {
                    npc_debug!(*actor, obj_id, npc_name, "Target is already dead");
                    if let Ok(mut vt) = visible_target_query.get_mut(*actor) {
                        vt.target = NO_TARGET;
                    }
                    *state = ActionState::Failure;
                    continue;
                }

                npc_debug!(*actor, obj_id, npc_name, "Target state={:?}", target.state);

                npc_debug!(*actor, obj_id, npc_name, "Target is adjacent, time to attack");
                let (damage, combo, _skill_gain) = Combat::process_attack(
                    AttackType::Quick,
                    &mut npc,
                    &mut target,
                    &mut commands,
                    &templates,
                    &map,
                    &mut ids,
                    &game_tick,
                    &mut map_events,
                );

                // Add damage record to target player's damage records
                if ids.is_hero(target.id.0) {
                    let damage_records = &mut player_stats
                        .get_mut(&target.player_id.0)
                        .unwrap()
                        .damage_records;

                    if damage_records.capacity() == damage_records.len() {
                        damage_records.pop_front();
                    }

                    damage_records.push_back(DamageRecord {
                        source: npc.template.0.clone(),
                        target: "Hero".to_string(),
                        amount: damage,
                        damage_type: "attack".to_string(),
                        tick: game_tick.0,
                    });
                }

                // Add visible damage event to broadcast to everyone nearby
                Combat::add_damage_event(
                    game_tick.0,
                    "quick".to_string(),
                    damage,
                    combo,
                    &npc,
                    &target,
                    &mut map_events,
                );

                // Add Cooldown Event
                let cooldown_event = VisibleEvent::CooldownEvent {
                    duration: npc_speed,
                };

                map_events.new(npc.id.0, game_tick.0 + npc_speed, cooldown_event);

                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");
                event_executing.state = EventExecutingState::Executing;

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let mut event_executing = event_executing_query
                    .get_mut(*actor)
                    .expect("Missing EventExecuting component");

                if !event_executing.state.is_finished() {
                    continue;
                }

                // Check if cooldown event failed
                if event_executing.state.is_failed() {
                    let obj_id = entity_map.get_obj_by_entity(*actor);
                    npc_debug!(*actor, obj_id, None, "Cooldown event failed");
                    *state = ActionState::Failure;
                    continue;
                }

                *state = ActionState::Success;
            }
            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let npc_name = npc_query.get(*actor).ok().map(|n| n.template.0.as_str());
                npc_debug!(*actor, Some(npc_id), npc_name, "AttackTarget action was cancelled. Considering this a failure.");

                let event_type = GameEventType::CancelAllMapEvents { obj_id: npc_id };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn cast_target_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    ids: ResMut<Ids>,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    visible_target_query: Query<(&PlayerId, &VisibleTarget), Without<EventInProgress>>,
    mut npc_query: Query<CombatQuery, (With<SubclassNPC>, Without<EventInProgress>)>,
    mut target_query: Query<CombatQuery, Without<SubclassNPC>>,
    mut query: Query<(&Actor, &mut ActionState, &mut ChaseAndCast)>,
) {
    for (Actor(actor), mut state, mut chase_and_cast) in &mut query {
        let Ok((npc_player_id, visible_target)) = visible_target_query.get(*actor) else {
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let target_id = visible_target.target;

                let blockinglist = Obj::blocking_list_combatquery(npc_player_id.0, &target_query);

                let Ok(mut npc) = npc_query.get_mut(*actor) else {
                    continue;
                };

                if game_tick.0 - chase_and_cast.start_time > 30 {
                    info!("Spell completed");
                    *state = ActionState::Success;
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.0.contains_key(&Effect::Stunned) {
                    debug!("NPC is stunned");
                    continue;
                }

                // Get NPC speed
                let mut npc_speed = 1;

                if let Some(npc_base_speed) = npc.stats.base_speed {
                    npc_speed = npc_base_speed;
                }

                let effect_speed_mod = npc.effects.get_speed_effects(&templates);

                let move_duration = (BASE_MOVE_TICKS
                    * (BASE_SPEED / npc_speed as f32)
                    * (1.0 / effect_speed_mod)) as i32;

                if target_id != NO_TARGET {
                    // Get target entity
                    let Some(target_entity) = entity_map.get_entity(target_id) else {
                        continue;
                    };

                    let Ok(target) = target_query.get_mut(target_entity) else {
                        continue;
                    };

                    let target_dist = Map::dist(*npc.pos, *target.pos);

                    if target_dist == 2 {
                        info!("Target is in range, time to cast spell");

                        // Shout spell
                        let speech_event = VisibleEvent::SpeechEvent {
                            speech: "Wis An Ben!".to_string(),
                            intensity: 2,
                        };

                        map_events.new(npc.id.0, game_tick.0 + 4, speech_event);

                        *npc.state = State::Casting;

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Casting,
                        });

                        let spell_damage_event = VisibleEvent::SpellDamageEvent {
                            spell: Spell::ShadowBolt,
                            target_id: target.id.0,
                        };

                        let map_event =
                            map_events.new(npc.id.0, game_tick.0 + 30, spell_damage_event);

                        commands.entity(*actor).insert(EventInProgress {
                            event_id: map_event.event_id,
                        });

                        // Set start time of action
                        chase_and_cast.start_time = game_tick.0;
                    } else if target_dist > 2 {
                        if *npc.state == State::None {
                            if let Some(path_result) = Map::find_fast_path(
                                *npc.pos,
                                *target.pos,
                                &map,
                                npc_player_id.0,
                                blockinglist,
                                true,
                                false,
                                false,
                                false,
                            ) {
                                debug!("Follower path: {:?}", path_result);

                                let (path, _c) = path_result;
                                let next_pos = &path[1];

                                debug!("Next pos: {:?}", next_pos);

                                // Add State Change Event to Moving
                                *npc.state = State::Moving;

                                commands.trigger(StateChange {
                                    entity: *actor,
                                    new_state: State::Moving,
                                });

                                // Add Move Event
                                let move_event = VisibleEvent::MoveEvent {
                                    src: *npc.pos,
                                    dst: Position {
                                        x: next_pos.0,
                                        y: next_pos.1,
                                    },
                                };

                                let move_map_event = map_events.new(
                                    npc.id.0,
                                    game_tick.0 + move_duration,
                                    move_event,
                                );

                                commands.entity(*actor).insert(EventInProgress {
                                    event_id: move_map_event.event_id,
                                });
                            }
                        }
                    } else if target_dist == 1 {
                        let neighbour_tiles = Map::get_neighbour_tiles(
                            npc.pos.x,
                            npc.pos.y,
                            &map,
                            npc_player_id.0,
                            &blockinglist,
                            true,
                            false,
                            false,
                            false,
                            MapPos(npc.pos.x, npc.pos.y),
                        );

                        let mut selected_pos_list = Vec::new();

                        for (map_pos, _movement_cost) in neighbour_tiles.iter() {
                            let dist = Map::dist(
                                Position {
                                    x: map_pos.0,
                                    y: map_pos.1,
                                },
                                *target.pos,
                            );

                            if dist == 2 {
                                selected_pos_list.push(map_pos.clone());
                            }
                        }

                        println!("selected_pos_list: {:?}", selected_pos_list);

                        if selected_pos_list.len() > 0 {
                            // Randomly select a pos from list
                            let mut rng = rand::thread_rng();
                            let next_pos = selected_pos_list
                                [rng.gen_range(0..selected_pos_list.len())]
                            .clone();

                            // Add State Change Event to Moving
                            *npc.state = State::Moving;

                            commands.trigger(StateChange {
                                entity: *actor,
                                new_state: State::Moving,
                            });

                            // Add Move Event
                            let move_event = VisibleEvent::MoveEvent {
                                src: *npc.pos,
                                dst: Position {
                                    x: next_pos.0,
                                    y: next_pos.1,
                                },
                            };

                            let move_map_event =
                                map_events.new(npc.id.0, game_tick.0 + move_duration, move_event);

                            commands.entity(*actor).insert(EventInProgress {
                                event_id: move_map_event.event_id,
                            });
                        } else {
                            // No choice but has to fight

                            // Shout spell
                            let speech_event = VisibleEvent::SpeechEvent {
                                speech: "Wis An Ben!".to_string(),
                                intensity: 2,
                            };

                            map_events.new(npc.id.0, game_tick.0 + 4, speech_event);

                            *npc.state = State::Casting;

                            commands.trigger(StateChange {
                                entity: *actor,
                                new_state: State::Casting,
                            });

                            let spell_damage_event = VisibleEvent::SpellDamageEvent {
                                spell: Spell::ShadowBolt,
                                target_id: target.id.0,
                            };

                            let map_event =
                                map_events.new(npc.id.0, game_tick.0 + 30, spell_damage_event);

                            commands.entity(*actor).insert(EventInProgress {
                                event_id: map_event.event_id,
                            });

                            // Set start time of action
                            chase_and_cast.start_time = game_tick.0;
                        }
                    }

                    *state = ActionState::Success;
                }
            }
            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                debug!("Action was cancelled. Considering this a failure.");
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn raise_dead_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut ids: ResMut<Ids>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    target_query: Query<&Target>,
    mut npc_query: Query<BaseQueryMutState, With<SubclassNPC>>,
    obj_query: Query<BaseQuery, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    mut query: Query<(&Actor, &mut ActionState, &mut RaiseDead)>,
) {
    for (Actor(actor), mut state, raise_dead) in &mut query {
        match *state {
            ActionState::Requested => {
                let Ok(mut npc) = npc_query.get_mut(*actor) else {
                    error!("Query failed to find entity {:?}", *actor);
                    continue;
                };

                let obj_id = Some(npc.id.0);
                let npc_name = Some(npc.template.0.as_str());

                npc_info!(*actor, obj_id, npc_name, "RaiseDead action requested");

                // If NPC state is not none, skip execution
                if *npc.state != State::None {
                    npc_info!(*actor, obj_id, npc_name, "NPC state is not none, skipping execution");
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, npc_name, "NPC is stunned");
                    continue;
                }

                let Ok(target) = target_query.get(*actor) else {
                    npc_error!(*actor, obj_id, npc_name, "Query failed to find target entity");
                    *state = ActionState::Failure;
                    continue;
                };

                // Get target entity
                npc_info!(*actor, obj_id, npc_name, "Task target: {:?}", target.id);
                let Some(target_entity) = entity_map.get_entity(target.id) else {
                    *state = ActionState::Failure;
                    npc_error!(*actor, obj_id, npc_name, "Cannot find target entity for {:?}", target.id);
                    continue;
                };

                let Ok(corpse) = obj_query.get(target_entity) else {
                    *state = ActionState::Failure;
                    npc_error!(*actor, obj_id, npc_name, "Cannot find target obj for {:?}", target.id);
                    continue;
                };

                npc_info!(*actor, obj_id, npc_name, "Corpse: {:?}", corpse);

                // Check if target is adjacent to npc, this could happen if the home target scorer changes targets
                if !Map::is_adjacent_including_source(*npc.pos, *corpse.pos) {
                    npc_info!(*actor, obj_id, npc_name, "Target is not adjacent to npc, raise dead event failed.");
                    *state = ActionState::Failure;
                    continue;
                }

                // Shout spell
                let speech_event = VisibleEvent::SpeechEvent {
                    speech: "Rise from the dead, Uus Corp!".to_string(),
                    intensity: 2,
                };

                map_events.new(npc.id.0, game_tick.0 + 4, speech_event);

                *npc.state = State::Casting;

                commands.trigger(StateChange {
                    entity: *actor,
                    new_state: State::Casting,
                });

                map_events.new(
                    npc.id.0,
                    game_tick.0 + 30,
                    VisibleEvent::SpellRaiseDeadEvent {
                        corpse_id: corpse.id.0,
                    },
                );

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let obj_id = entity_map.get_obj_by_entity(*actor);
                let npc_name = npc_query.get(*actor).ok().map(|n| n.template.0.as_str());

                let Ok(_event) = completed_query.get(*actor) else {
                    npc_info!(*actor, obj_id, npc_name, "RaiseDead action still executing, waiting for completed component");
                    continue;
                };

                npc_info!(*actor, obj_id, npc_name, "RaiseDead action completed");

                *state = ActionState::Success;
            }
            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let npc_name = npc_query.get(*actor).ok().map(|n| n.template.0.as_str());
                npc_debug!(*actor, Some(npc_id), npc_name, "RaiseDead action was cancelled. Considering this a failure.");

                let event_type = GameEventType::CancelAllMapEvents { obj_id: npc_id };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

/*pub fn flee_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    flee_query: Query<(&PlayerId, &Home), Without<EventInProgress>>,
    mut obj_query: Query<ObjStatQuery>,
    mut query: Query<(&Actor, &mut ActionState, &FleeToHome)>,
) {
    for (Actor(actor), mut state, _flee_to_home) in &mut query {
        match *state {
            ActionState::Requested => {
                // This skip the action if the entity has the EventInProgress component
                let Ok((_player_id, _home)) = flee_query.get(*actor) else {
                    continue;
                };

                let Ok(obj) = obj_query.get(*actor) else {
                    continue;
                };

                let sound_event = VisibleEvent::SoundObjEvent {
                    sound: "My minions fall, but I will get my revenge!".to_string(),
                    intensity: 2,
                };

                map_events.new(obj.id.0, game_tick.0 + 4, sound_event);

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((player_id, home)) = flee_query.get(*actor) else {
                    continue;
                };

                let blocking_list = Obj::blocking_list_objstatquery(player_id.0, &mut obj_query);

                let Ok(mut obj) = obj_query.get_mut(*actor) else {
                    continue;
                };

                if *obj.pos == home.pos {
                    commands.entity(*actor).remove::<MoveToInProgress>();
                    *state = ActionState::Success;
                } else {
                    println!("Finding path from {:?} to {:?}", obj.pos, home.pos);

                    if let Some(path_result) = Map::find_fast_path(
                        *obj.pos,
                        home.pos,
                        &map,
                        player_id.0,
                        blocking_list,
                        true,
                        false,
                        false,
                        false,
                    ) {
                        println!("Follower path: {:?}", path_result);

                        let (path, _c) = path_result;
                        let next_pos = &path[1];

                        debug!("Next pos: {:?}", next_pos);

                        // Add State Change Event to Moving
                        *obj.state = State::Moving;

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Moving,
                        });

                        // Add Move Event
                        let move_event = VisibleEvent::MoveEvent {
                            src: *obj.pos,
                            dst: Position {
                                x: next_pos.0,
                                y: next_pos.1,
                            },
                        };

                        let move_map_event = map_events.new(
                            obj.id.0,
                            game_tick.0 + 36, // in the future
                            move_event,
                        );

                        commands.entity(*actor).insert(EventInProgress {
                            event_id: move_map_event.event_id,
                        });

                        commands.entity(*actor).insert(MoveToInProgress);
                    } else {
                        error!("Cannot find path");
                        *state = ActionState::Failure;
                    }
                }
            }
            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                debug!("Action was cancelled. Considering this a failure.");
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}*/

pub fn hide_action_system(
    game_tick: Res<GameTick>,
    mut map_events: ResMut<MapEvents>,
    obj_query: Query<&Id>,
    mut query: Query<(&Actor, &mut ActionState, &mut Hide, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _hide, _span) in &mut query {
        match *state {
            ActionState::Requested => {
                let Ok(obj_id) = obj_query.get(*actor) else {
                    continue;
                };

                map_events.new(
                    obj_id.0,
                    game_tick.0 + 1, // in the future
                    VisibleEvent::HideEvent,
                );

                *state = ActionState::Success;
            }
            ActionState::Executing => {
                // Get Id from actor
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn spoil_target_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut ids: ResMut<Ids>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    task_target_query: Query<&mut TaskTarget>,
    mut npc_query: Query<BaseQueryEffects, With<SubclassNPC>>,
    obj_query: Query<BaseQuery, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    mut query: Query<(&Actor, &mut ActionState, &SpoilTarget)>,
) {
    for (Actor(actor), mut state, _spoil_target_action) in &mut query {
        match *state {
            ActionState::Requested => {
                let Ok(npc) = npc_query.get_mut(*actor) else {
                    error!("Query failed to find entity {:?}", *actor);
                    continue;
                };

                let obj_id = Some(npc.id.0);

                npc_info!(*actor, obj_id, None, "SpoilTarget action requested");

                // If NPC state is not none, skip execution
                if *npc.state != State::None {
                    npc_info!(*actor, obj_id, None, "NPC state is not none, skipping execution");
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                let Ok(task_target) = task_target_query.get(*actor) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find target entity");
                    *state = ActionState::Failure;
                    continue;
                };

                // Get target entity
                npc_info!(*actor, obj_id, None, "Task target: {:?}", task_target.target);
                let Some(target_entity) = entity_map.get_entity(task_target.target) else {
                    *state = ActionState::Failure;
                    npc_error!(*actor, obj_id, None, "Cannot find target entity for {:?}", task_target.target);
                    continue;
                };

                let Ok(target) = obj_query.get(target_entity) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find entity {:?} for target {:?}", target_entity, task_target.target);
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if target is adjacent to npc, this could happen if the torch target scorer changes targets
                if !Map::is_adjacent_including_source(*npc.pos, *target.pos) {
                    npc_info!(*actor, obj_id, None, "Target is not adjacent to npc, spoil event failed.");
                    *state = ActionState::Failure;
                    continue;
                }

                // Check if target has food or drink items
                let food_item = target.inventory.get_by_class(FOOD.to_owned());
                let drink_item = target.inventory.get_by_class(DRINK.to_owned());

                let Some(item) = food_item.or(drink_item) else {
                    npc_info!(*actor, obj_id, None, "Target does not have food or drink items, spoil event failed.");
                    *state = ActionState::Failure;
                    continue;
                };

                let spoil_event = VisibleEvent::SpoilEvent {
                    target_id: target.id.0,
                    target_pos: *target.pos,
                    item_type: item.class.to_string(),
                };

                map_events.new(npc.id.0, game_tick.0 + 20, spoil_event);

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let obj_id = entity_map.get_obj_by_entity(*actor);

                let Ok(_event) = completed_query.get(*actor) else {
                    npc_info!(*actor, obj_id, None, "Spoil target action still executing, waiting for completed component");
                    continue;
                };

                npc_info!(*actor, obj_id, None, "Spoil target action completed");

                *state = ActionState::Success;
            }

            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                npc_debug!(*actor, Some(npc_id), None, "SpoilTarget action was cancelled. Considering this a failure.");

                let event_type = GameEventType::CancelAllMapEvents { obj_id: npc_id };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn steal_target_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut ids: ResMut<Ids>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    task_target_query: Query<&mut TaskTarget>,
    mut npc_query: Query<BaseQueryEffects, With<SubclassNPC>>,
    obj_query: Query<BaseQuery, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    items_to_steal_query: Query<&ItemsToSteal>,
    mut query: Query<(&Actor, &mut ActionState, &StealTarget)>,
) {
    for (Actor(actor), mut state, _steal_target_action) in &mut query {
        match *state {
            ActionState::Requested => {
                let Ok(npc) = npc_query.get_mut(*actor) else {
                    error!("Query failed to find entity {:?}", *actor);
                    continue;
                };

                let obj_id = Some(npc.id.0);

                npc_info!(*actor, obj_id, None, "StealTarget action requested");

                // If NPC state is not none, skip execution
                if *npc.state != State::None {
                    npc_info!(*actor, obj_id, None, "NPC state is not none, skipping execution");
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                let Ok(task_target) = task_target_query.get(*actor) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find target entity");
                    *state = ActionState::Failure;
                    continue;
                };

                // Get target entity
                npc_info!(*actor, obj_id, None, "Task target: {:?}", task_target.target);
                let Some(target_entity) = entity_map.get_entity(task_target.target) else {
                    *state = ActionState::Failure;
                    npc_error!(*actor, obj_id, None, "Cannot find target entity for {:?}", task_target.target);
                    continue;
                };

                let Ok(target) = obj_query.get(target_entity) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find entity {:?} for target {:?}", target_entity, task_target.target);
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if target is adjacent to npc, this could happen if the torch target scorer changes targets
                if !Map::is_adjacent_including_source(*npc.pos, *target.pos) {
                    npc_info!(*actor, obj_id, None, "Target is not adjacent to npc, steal event failed.");
                    *state = ActionState::Failure;
                    continue;
                }

                let Ok(items_to_steal) = items_to_steal_query.get(*actor) else {
                    npc_info!(*actor, obj_id, None, "Target does not have defined items to steal, skipping");
                    *state = ActionState::Failure;
                    continue;
                };

                let steal_event = VisibleEvent::StealEvent {
                    target_id: target.id.0,
                    target_pos: *target.pos,
                    item_types: items_to_steal
                        .item_classes
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                };

                // Add a random factor to the event duration to prevent all npcs from stealing at the same time
                let event_duration = rand::thread_rng().gen_range(20..40);

                map_events.new(npc.id.0, game_tick.0 + event_duration, steal_event);

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let obj_id = entity_map.get_obj_by_entity(*actor);

                let Ok(_event) = completed_query.get(*actor) else {
                    npc_info!(*actor, obj_id, None, "Steal target action still executing, waiting for completed component");
                    continue;
                };

                npc_info!(*actor, obj_id, None, "Steal target action completed");

                *state = ActionState::Success;
            }

            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                npc_debug!(*actor, Some(npc_id), None, "StealTarget action was cancelled. Considering this a failure.");

                let event_type = GameEventType::CancelAllMapEvents { obj_id: npc_id };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn torch_target_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut ids: ResMut<Ids>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    task_target_query: Query<&mut TaskTarget>,
    mut npc_query: Query<BaseQueryEffects, With<SubclassNPC>>,
    obj_query: Query<BaseQuery, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    mut query: Query<(&Actor, &mut ActionState, &TorchTarget)>,
) {
    for (Actor(actor), mut state, _rat_crisis_action) in &mut query {
        match *state {
            ActionState::Requested => {
                let Ok(npc) = npc_query.get_mut(*actor) else {
                    error!("Query failed to find entity {:?}", *actor);
                    continue;
                };

                let obj_id = Some(npc.id.0);

                npc_info!(*actor, obj_id, None, "TorchTarget action requested");

                // If NPC state is not none, skip execution
                if *npc.state != State::None {
                    npc_info!(*actor, obj_id, None, "NPC state is not none, skipping execution");
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                let Ok(task_target) = task_target_query.get(*actor) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find target entity");
                    *state = ActionState::Failure;
                    continue;
                };

                // Get target entity
                npc_info!(*actor, obj_id, None, "Task target: {:?}", task_target.target);
                let Some(target_entity) = entity_map.get_entity(task_target.target) else {
                    *state = ActionState::Failure;
                    npc_error!(*actor, obj_id, None, "Cannot find target entity for {:?}", task_target.target);
                    continue;
                };

                let Ok(target) = obj_query.get(target_entity) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find entity {:?} for target {:?}", target_entity, task_target.target);
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if target is adjacent to npc, this could happen if the torch target scorer changes targets
                if !Map::is_adjacent_including_source(*npc.pos, *target.pos) {
                    npc_info!(*actor, obj_id, None, "Target is not adjacent to npc, torch event failed.");
                    *state = ActionState::Failure;
                    continue;
                }

                let torch_event = VisibleEvent::TorchEvent {
                    target_id: target.id.0,
                    target_pos: *target.pos,
                };

                map_events.new(npc.id.0, game_tick.0 + 20, torch_event);

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let obj_id = entity_map.get_obj_by_entity(*actor);

                let Ok(_event) = completed_query.get(*actor) else {
                    npc_info!(*actor, obj_id, None, "Torch target action still executing, waiting for completed component");
                    continue;
                };

                npc_info!(*actor, obj_id, None, "Torch target action completed");

                *state = ActionState::Success;
            }

            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                npc_debug!(*actor, Some(npc_id), None, "TorchTarget action was cancelled. Considering this a failure.");

                let event_type = GameEventType::CancelAllMapEvents { obj_id: npc_id };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn cast_spell_target_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut ids: ResMut<Ids>,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    target_query: Query<&Target>,
    mut npc_query: Query<BaseQueryMutState, With<SubclassNPC>>,
    obj_query: Query<BaseQuery, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    mut query: Query<(&Actor, &mut ActionState, &CastSpellTarget)>,
) {
    for (Actor(actor), mut state, _cast_spell_target) in &mut query {
        match *state {
            ActionState::Requested => {
                let Ok(mut npc) = npc_query.get_mut(*actor) else {
                    error!("Query failed to find entity {:?}", *actor);
                    continue;
                };

                let obj_id = Some(npc.id.0);
                let npc_name = Some(npc.template.0.as_str());

                npc_info!(*actor, obj_id, npc_name, "CastSpellTarget action requested");

                // If NPC state is not none, skip execution
                if *npc.state != State::None {
                    npc_info!(*actor, obj_id, npc_name, "NPC state is not none, skipping execution");
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, npc_name, "NPC is stunned");
                    continue;
                }

                let Ok(target) = target_query.get(*actor) else {
                    npc_error!(*actor, obj_id, npc_name, "Query failed to find target entity");
                    *state = ActionState::Failure;
                    continue;
                };

                // Get target entity
                npc_info!(*actor, obj_id, npc_name, "Task target: {:?}", target.id);
                let Some(target_entity) = entity_map.get_entity(target.id) else {
                    *state = ActionState::Failure;
                    npc_error!(*actor, obj_id, npc_name, "Cannot find target entity for {:?}", target.id);
                    continue;
                };

                let Ok(target) = obj_query.get(target_entity) else {
                    npc_error!(*actor, obj_id, npc_name, "Query failed to find entity {:?} for target", target_entity);
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if target is within range
                if Map::dist(*npc.pos, *target.pos) > 2 {
                    npc_info!(*actor, obj_id, npc_name, "Target is not within range, cast spell failed.");
                    *state = ActionState::Failure;
                    continue;
                }

                // Shout spell
                let speech_event = VisibleEvent::SpeechEvent {
                    speech: "Wis An Ben!".to_string(),
                    intensity: 2,
                };

                map_events.new(npc.id.0, game_tick.0 + 4, speech_event);

                *npc.state = State::Casting;

                commands.trigger(StateChange {
                    entity: *actor,
                    new_state: State::Casting,
                });

                let spell_damage_event = VisibleEvent::SpellDamageEvent {
                    spell: Spell::ShadowBolt,
                    target_id: target.id.0,
                };

                map_events.new(npc.id.0, game_tick.0 + 50, spell_damage_event);

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let obj_id = entity_map.get_obj_by_entity(*actor);
                let npc_name = npc_query.get(*actor).ok().map(|n| n.template.0.as_str());

                let Ok(_event) = completed_query.get(*actor) else {
                    npc_info!(*actor, obj_id, npc_name, "Cast spell target action still executing, waiting for completed component");
                    continue;
                };

                npc_info!(*actor, obj_id, npc_name, "Cast spell target action completed");

                *state = ActionState::Success;
            }

            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                let Some(npc_id) = entity_map.get_obj_by_entity(*actor) else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let npc_name = npc_query.get(*actor).ok().map(|n| n.template.0.as_str());
                npc_debug!(*actor, Some(npc_id), npc_name, "CastSpellTarget action was cancelled. Considering this a failure.");

                let event_type = GameEventType::CancelAllMapEvents { obj_id: npc_id };

                let event_id = ids.new_map_event_id();

                let event = GameEvent {
                    event_id: event_id,
                    start_tick: game_tick.0,
                    run_tick: game_tick.0 + 1, // Add one game tick
                    event_type,
                };

                game_events.insert(event.event_id, event);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

/*pub fn merchant_scorer_system(
    _game_tick: Res<GameTick>,
    _move_in_progress: Query<&MoveToInProgress>,
    _pos_query: Query<(&Position, &mut Merchant)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<MerchantScorer>>,
) {
    for (Actor(_actor), mut score, _span) in &mut query {
        score.set(1.0);
    }
}*/

pub fn is_mindless(int: &String) -> bool {
    return *int == "mindless".to_string();
}

pub fn is_animal(int: &String) -> bool {
    return *int == "animal".to_string();
}

pub fn is_cunning(int: &String) -> bool {
    return *int == "cunning".to_string();
}

pub fn is_frenzied(aggression: &String) -> bool {
    return *aggression == "frenzied".to_string();
}

pub fn is_strategic(aggression: &String) -> bool {
    return *aggression == "strategic".to_string();
}

pub fn is_passive(aggression: &String) -> bool {
    return *aggression == "passive".to_string();
}

/*pub fn wander_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    player_query: Query<&PlayerId, Without<EventInProgress>>,
    mut obj_query: Query<ObjStatQuery>,
    mut query: Query<(&Actor, &mut ActionState, &Wander)>,
) {
    for (Actor(actor), mut state, _flee_to_home) in &mut query {
        match *state {
            ActionState::Requested => {
                // This skip the action if the entity has the EventInProgress component
                let Ok(_player_id) = player_query.get(*actor) else {
                    continue;
                };

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok(player_id) = player_query.get(*actor) else {
                    continue;
                };

                let blocking_list = Obj::blocking_list_objstatquery(player_id.0, &mut obj_query);

                let Ok(mut obj) = obj_query.get_mut(*actor) else {
                    continue;
                };

                Map::get_neighbour_tiles(obj.pos.x, obj.pos.y, &map, &blocking_list, landwalk, waterwalk, mountainwalk, ignore_goal_terrain_type, goal)

                if *obj.pos == home.pos {
                    commands.entity(*actor).remove::<MoveToInProgress>();
                    *state = ActionState::Success;
                } else {
                    println!("Finding path from {:?} to {:?}", obj.pos, home.pos);

                    if let Some(path_result) = Map::find_path(
                        *obj.pos,
                        home.pos,
                        &map,
                        blocking_list,
                        true,
                        false,
                        false,
                        false,
                    ) {
                        println!("Follower path: {:?}", path_result);

                        let (path, _c) = path_result;
                        let next_pos = &path[1];

                        debug!("Next pos: {:?}", next_pos);

                        // Add State Change Event to Moving
                        *obj.state = State::Moving;

                        commands.trigger(StateChange {
                            entity: *actor,
                            new_state: State::Moving,
                        });

                        // Add Move Event
                        let move_event = VisibleEvent::MoveEvent {
                            src: *obj.pos,
                            dst: Position {
                                x: next_pos.0,
                                y: next_pos.1,
                            },
                        };

                        let move_map_event = map_events.new(
                            obj.id.0,
                            game_tick.0 + 36, // in the future
                            move_event,
                        );

                        commands.entity(*actor).insert(EventInProgress {
                            event_id: move_map_event.event_id,
                        });

                        commands.entity(*actor).insert(MoveToInProgress);
                    } else {
                        error!("Cannot find path");
                        *state = ActionState::Failure;
                    }
                }
            }
            // All Actions should make sure to handle cancellations!
            ActionState::Cancelled => {
                debug!("Action was cancelled. Considering this a failure.");
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}*/

pub fn idle_action_system(mut query: Query<(&Actor, &mut ActionState, &Idle, &ActionSpan)>) {
    for (Actor(actor), mut state, _idle, _span) in &mut query {
        *state = ActionState::Success;
    }
}

fn cancel_npc_events(npc_id: i32, current_tick: i32, ids: &mut Ids, game_events: &mut GameEvents) {
    let event_type = GameEventType::CancelAllMapEvents { obj_id: npc_id };
    let event_id = ids.new_map_event_id();

    let event = GameEvent {
        event_id,
        start_tick: current_tick,
        run_tick: current_tick + 1, // Add one game tick
        event_type,
    };

    game_events.insert(event.event_id, event);
}
