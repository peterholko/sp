use core::f32;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use big_brain::prelude::*;
use rand::Rng;

use crate::ai_logging::entity_display;
use crate::combat::{AttackType, Combat, CombatQuery};
use crate::common::{
    AttackTarget, Destination, Hide, Idle, MoveTo, SetAttackTarget, Target, TaskTarget,
};
use crate::crisis_balance::CrisisAttackTelemetryStage;
use crate::effect::Effect;
use crate::effect::Effects;
use crate::event::{EventCompleted, EventExecuting, EventExecutingState, Spell};
use crate::event::{GameEvent, GameEventType, GameEvents, MapEvents, VisibleEvent};
use crate::game::*;
use crate::ids::EntityObjMap;
use crate::ids::Ids;
use crate::item;
use crate::item::*;
use crate::map::{Map, MapPos, TileType};
use crate::network::{send_to_client, ResponsePacket};
use crate::obj::{
    BaseQuery, BaseQueryMutState, Blocker, Class, Id, Obj, ObjStatQuery, PlayerId, Position, State,
    StateChange, StateDead, Stats, Subclass, SubclassNPC, Template, Viewshed,
};
use crate::obj::{BaseQueryEffects, ClassStructure};
use crate::player;
use crate::player::Player;
use crate::safe_logout::{
    is_owner_offline_protected, object_belongs_to_protected_run, protected_player_for_object,
    PlayerWorldPresenceState, SafeLogoutTelemetryState,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimalFallbackKind {
    Wander,
    HideInForest,
    // Wander a few steps first, then retreat to forest cover and hide.
    WanderThenHide,
}

#[derive(Debug, Component, Clone)]
pub struct AnimalFallback {
    pub kind: AnimalFallbackKind,
    pub target_id: i32,
    pub last_seen_pos: Position,
}

#[derive(Clone)]
struct WallTargetCandidate {
    id: i32,
    player_id: i32,
    pos: Position,
    hp: i32,
    distance: u32,
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

#[derive(Debug, Component)]
pub struct ProtectedTargetInvalidated {
    /// The target whose protection caused invalidation. The hold remains until
    /// this exact target is no longer protected, preventing the hostile from
    /// redirecting to another player merely as a fallback.
    target_id: i32,
}

#[derive(SystemParam)]
pub struct NpcProtection<'w, 's> {
    presence: Option<Res<'w, PlayerWorldPresenceState>>,
    actor_ids: Query<'w, 's, &'static Id>,
}

impl NpcProtection<'_, '_> {
    fn owner_is_protected(&self, owner: &PlayerId) -> bool {
        self.presence
            .as_deref()
            .map(|presence| is_owner_offline_protected(owner, presence))
            .unwrap_or(false)
    }

    fn object_is_protected(&self, object_id: i32, ids: &Ids) -> bool {
        self.presence
            .as_deref()
            .map(|presence| object_belongs_to_protected_run(object_id, ids, presence))
            .unwrap_or(false)
    }

    fn object_is_protected_if_ids(&self, object_id: i32, ids: Option<&Ids>) -> bool {
        ids.map(|ids| self.object_is_protected(object_id, ids))
            .unwrap_or(false)
    }

    /// Actor-side protection is based on the NPC's own object id, not its
    /// faction `PlayerId`. Introduction and other run-attributed NPCs commonly
    /// retain a neutral/NPC owner and are associated through `RunSpawnedObjs`.
    fn actor_is_protected(&self, actor: Entity, ids: &Ids) -> bool {
        self.actor_ids
            .get(actor)
            .map(|id| self.object_is_protected(id.0, ids))
            .unwrap_or(false)
    }

    fn actor_is_protected_if_ids(&self, actor: Entity, ids: Option<&Ids>) -> bool {
        ids.map(|ids| self.actor_is_protected(actor, ids))
            .unwrap_or(false)
    }
}

fn queued_event_targets_protected_run(
    event: &VisibleEvent,
    ids: &Ids,
    presence: &PlayerWorldPresenceState,
) -> bool {
    let target_id = match event {
        VisibleEvent::SpellDamageEvent { target_id, .. }
        | VisibleEvent::SpoilEvent { target_id, .. }
        | VisibleEvent::StealEvent { target_id, .. }
        | VisibleEvent::TorchEvent { target_id, .. } => Some(*target_id),
        VisibleEvent::SpellRaiseDeadEvent { corpse_id } => Some(*corpse_id),
        _ => None,
    };

    target_id
        .map(|target_id| object_belongs_to_protected_run(target_id, ids, presence))
        .unwrap_or(false)
}

pub fn protected_target_invalidation_system(
    mut commands: Commands,
    ids: Res<Ids>,
    protection: NpcProtection,
    mut telemetry: Option<ResMut<SafeLogoutTelemetryState>>,
    mut map_events: ResMut<MapEvents>,
    mut npc_query: Query<
        (
            Entity,
            &Id,
            &mut State,
            &mut VisibleTarget,
            Option<&mut TaskTarget>,
            Option<&Target>,
            &mut EventExecuting,
        ),
        With<SubclassNPC>,
    >,
) {
    for (entity, npc_id, mut state, mut visible, task, installed, mut event_executing) in
        &mut npc_query
    {
        if protection.actor_is_protected(entity, &ids) {
            continue;
        }

        let lost_protected_target = [
            visible.target,
            task.as_ref().map(|task| task.target).unwrap_or(NO_TARGET),
            installed.map(|target| target.id).unwrap_or(NO_TARGET),
        ]
        .into_iter()
        .find(|target_id| {
            *target_id != NO_TARGET && protection.object_is_protected(*target_id, &ids)
        });

        let Some(lost_protected_target) = lost_protected_target else {
            continue;
        };

        let protected_player = protection.presence.as_deref().and_then(|presence| {
            protected_player_for_object(lost_protected_target, &ids, presence)
        });
        if let (Some(player_id), Some(telemetry)) = (protected_player, telemetry.as_deref_mut()) {
            telemetry.record_protected_target_rejection(player_id);
        }

        visible.target = NO_TARGET;
        if let Some(mut task) = task {
            task.target = NO_TARGET;
        }
        commands
            .entity(entity)
            .try_remove::<Target>()
            .try_remove::<AnimalFallback>()
            .try_remove::<EventInProgress>()
            .try_insert(ProtectedTargetInvalidated {
                target_id: lost_protected_target,
            });
        event_executing.state = EventExecutingState::None;

        if matches!(*state, State::Moving | State::Casting) {
            *state = State::None;
            commands.trigger(StateChange {
                entity,
                new_state: State::None,
            });
        }

        let event_ids = map_events
            .iter()
            .filter_map(|(event_id, event)| {
                if event.obj_id != npc_id.0 {
                    return None;
                }

                (matches!(&event.event_type, VisibleEvent::MoveEvent { .. })
                    || protection
                        .presence
                        .as_deref()
                        .map(|presence| {
                            queued_event_targets_protected_run(&event.event_type, &ids, presence)
                        })
                        .unwrap_or(false))
                .then_some(*event_id)
            })
            .collect::<Vec<_>>();

        let discarded = event_ids.len();
        for event_id in event_ids {
            map_events.remove(&event_id);
        }
        if let (Some(player_id), Some(telemetry)) = (protected_player, telemetry.as_deref_mut()) {
            telemetry.record_protected_queued_events_discarded(player_id, discarded);
        }
    }
}

pub fn clear_protected_target_invalidation_system(
    mut commands: Commands,
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    invalidated_query: Query<(Entity, &ProtectedTargetInvalidated)>,
) {
    for (entity, invalidated) in &invalidated_query {
        if protection.actor_is_protected_if_ids(entity, ids.as_deref()) {
            continue;
        }

        let still_protected = ids
            .as_deref()
            .map(|ids| protection.object_is_protected(invalidated.target_id, ids))
            .unwrap_or(false);
        if !still_protected {
            commands
                .entity(entity)
                .try_remove::<ProtectedTargetInvalidated>();
        }
    }
}

fn personal_assault_allows_target_owner(
    assault: Option<&CrisisAssaultUnit>,
    target_owner: Option<i32>,
) -> bool {
    assault
        .map(|assault| target_owner == Some(assault.owner_player_id))
        .unwrap_or(true)
}

fn personal_assault_allows_target_entity(
    assault: Option<&CrisisAssaultUnit>,
    target_owner: Option<i32>,
    target_assault: Option<&CrisisAssaultUnit>,
    target_is_dead: bool,
) -> bool {
    assault
        .map(|assault| {
            target_assault
                .map(|target_assault| target_is_dead && target_assault == assault)
                .unwrap_or(target_owner == Some(assault.owner_player_id))
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::App;
    use big_brain::actions::spawn_action;
    use big_brain::prelude::Score;
    use big_brain::scorers::spawn_scorer;
    use big_brain::BigBrainPlugin;
    use std::collections::HashMap;

    use crate::constants::{
        CLASS_CORPSE, CLASS_STRUCTURE, CLASS_UNIT, MONOLITH_PLAYER_ID, NORMAL_SCORE, NPC_PLAYER_ID,
        SUBCLASS_CORPSE, SUBCLASS_NPC, TICKS_PER_SEC, URGENT_SCORE,
    };
    use crate::event::{EventExecuting, EventExecutingState};
    use crate::map::{TileInfo, TileType, HEIGHT, WIDTH};
    use crate::obj::{LastCombatTick, Misc, Name};
    use crate::safe_logout::{PlayerPresenceRecord, PlayerWorldPresence, ProtectedRunKey};
    use crate::templates::ObjTemplate;

    fn test_stats() -> Stats {
        Stats {
            hp: 10,
            stamina: None,
            mana: None,
            base_hp: 10,
            base_stamina: None,
            base_mana: None,
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

    fn test_obj_template(name: &str, int: &str) -> ObjTemplate {
        ObjTemplate {
            class: CLASS_UNIT.to_string(),
            subclass: SUBCLASS_NPC.to_string(),
            template: name.to_string(),
            image: name.to_lowercase(),
            family: None,
            groups: None,
            base_hp: None,
            base_stamina: None,
            base_mana: None,
            base_dmg: None,
            dmg_range: None,
            base_def: None,
            base_speed: None,
            base_vision: Some(10),
            base_work: None,
            int: Some(int.to_string()),
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
        }
    }

    fn minimal_templates() -> Templates {
        Templates::from_obj_templates(vec![
            test_obj_template("Goblin", "cunning"),
            test_obj_template("Zombie", "mindless"),
            test_obj_template("Necromancer", "cunning"),
            test_obj_template("Fire Dragon", "cunning"),
            test_obj_template("Wolf", "animal"),
            test_obj_template("Giant Rat", "animal"),
        ])
    }

    fn flat_test_map() -> Map {
        Map {
            width: WIDTH,
            height: HEIGHT,
            base: vec![
                TileInfo {
                    tile_type: TileType::Grasslands,
                    layers: vec![1],
                };
                (WIDTH * HEIGHT) as usize
            ],
            temperature: Vec::new(),
            moisture: Vec::new(),
            wildness: vec![0; (WIDTH * HEIGHT) as usize],
        }
    }

    fn empty_inventory(owner: i32) -> Inventory {
        Inventory {
            owner,
            items: Vec::new(),
        }
    }

    fn spawn_scripted_corpse_hunt_scorer(
        app: &mut App,
        npc_pos: Position,
        corpse_anchor: Position,
    ) -> (Entity, Entity) {
        let npc_entity = app
            .world_mut()
            .spawn((
                PlayerId(NPC_PLAYER_ID),
                npc_pos,
                TaskTarget::new(NO_TARGET),
                EventExecuting {
                    event_type: String::new(),
                    state: EventExecutingState::None,
                },
                ScriptedCorpseHunt {
                    corpse_anchor,
                    search_radius: 5,
                },
                SubclassNPC,
            ))
            .id();

        let scorer_entity = {
            let mut commands = app.world_mut().commands();
            spawn_scorer(&ScriptedCorpseHuntScorer, &mut commands, npc_entity)
        };
        app.world_mut().flush();

        (npc_entity, scorer_entity)
    }

    fn spawn_corpse_hunt_target(
        app: &mut App,
        id: i32,
        pos: Position,
        class: &str,
        template: &str,
    ) -> Entity {
        let subclass = if class == CLASS_CORPSE {
            SUBCLASS_CORPSE
        } else {
            SUBCLASS_NPC
        };

        app.world_mut()
            .spawn((
                Id(id),
                PlayerId(1),
                pos,
                Name(template.to_string()),
                Template(template.to_string()),
                Class(class.to_string()),
                Subclass::from_str(subclass),
                State::Dead,
                Misc {
                    image: String::new(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
                empty_inventory(id),
            ))
            .id()
    }

    fn setup_scripted_corpse_hunt_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, scripted_corpse_hunt_scorer_system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(flat_test_map());
        app
    }

    fn register_test_obj(app: &mut App, obj_id: i32, player_id: i32, entity: Entity) {
        app.world_mut()
            .resource_mut::<Ids>()
            .new_obj(obj_id, player_id);
        app.world_mut()
            .resource_mut::<EntityObjMap>()
            .new_obj(obj_id, entity);
    }

    fn setup_scripted_necromancer_brain_app() -> App {
        let mut app = App::new();
        app.add_plugins(BigBrainPlugin::new(PreUpdate));
        app.add_systems(
            Update,
            (
                scripted_corpse_hunt_scorer_system.in_set(BigBrainSet::Scorers),
                set_corpse_target_system.in_set(BigBrainSet::Actions),
                move_to_target_system.in_set(BigBrainSet::Actions),
                raise_dead_system.in_set(BigBrainSet::Actions),
            ),
        );
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(flat_test_map());
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut().insert_resource(MapEvents(HashMap::new()));
        app.world_mut().insert_resource(GameEvents(HashMap::new()));
        app.world_mut().insert_resource(minimal_templates());
        app
    }

    fn spawn_scripted_necromancer_brain(
        app: &mut App,
        npc_pos: Position,
        corpse_anchor: Position,
    ) -> Entity {
        let scripted_raise_dead = Steps::build()
            .label("Scripted Corpse Hunt")
            .step(SetCorpseTarget)
            .step(NpcMoveToTarget)
            .step(RaiseDead);

        let npc_entity = app
            .world_mut()
            .spawn((
                Id(28),
                PlayerId(NPC_PLAYER_ID),
                npc_pos,
                Name("Necromancer".to_string()),
                Template("Necromancer".to_string()),
                Class(CLASS_UNIT.to_string()),
                Subclass::Npc,
                State::None,
                Misc {
                    image: "necromancer".to_string(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
                empty_inventory(28),
            ))
            .insert((
                Viewshed { range: 5 },
                SubclassNPC,
                VisibleTarget::new(NO_TARGET),
                TaskTarget::new(NO_TARGET),
                EventExecuting {
                    event_type: String::new(),
                    state: EventExecutingState::None,
                },
                ScriptedCorpseHunt {
                    corpse_anchor,
                    search_radius: 5,
                },
                Thinker::build()
                    .label("Necromancer")
                    .picker(Highest)
                    .when(ScriptedCorpseHuntScorer, scripted_raise_dead),
            ))
            .id();

        register_test_obj(app, 28, NPC_PLAYER_ID, npc_entity);
        npc_entity
    }

    fn spawn_blocking_test_unit(app: &mut App, id: i32, pos: Position) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                Id(id),
                PlayerId(1),
                pos,
                Name("Hero".to_string()),
                Template("Human".to_string()),
                Class(CLASS_UNIT.to_string()),
                Subclass::Hero,
                State::None,
                Misc {
                    image: String::new(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
                empty_inventory(id),
            ))
            .id();
        register_test_obj(app, id, 1, entity);
        entity
    }

    fn wall_stats(hp: i32) -> Stats {
        Stats {
            hp,
            base_hp: hp,
            ..test_stats()
        }
    }

    fn fortified_effects() -> Effects {
        Effects(HashMap::from([(Effect::Fortified, (0, 1.0, 1))]))
    }

    fn spawn_stockade_wall(app: &mut App, id: i32, pos: Position, hp: i32) {
        app.world_mut().spawn((
            Id(id),
            PlayerId(1),
            pos,
            State::None,
            Class(CLASS_STRUCTURE.to_string()),
            Subclass::Wall,
            empty_effects(),
            wall_stats(hp),
            empty_inventory(id),
        ));
    }

    fn spawn_target_scorer(
        app: &mut App,
        npc_template: &str,
        npc_pos: Position,
        viewshed_range: u32,
    ) -> (Entity, Entity) {
        let npc_entity = app
            .world_mut()
            .spawn((
                PlayerId(NPC_PLAYER_ID),
                npc_pos,
                Template(npc_template.to_string()),
                Viewshed {
                    range: viewshed_range,
                },
                VisibleTarget::new(NO_TARGET),
                SubclassNPC,
                test_stats(),
                EventExecuting {
                    event_type: String::new(),
                    state: EventExecutingState::None,
                },
            ))
            .id();

        let scorer_entity = {
            let mut commands = app.world_mut().commands();
            spawn_scorer(&VisibleTargetScorer, &mut commands, npc_entity)
        };
        app.world_mut().flush();

        (npc_entity, scorer_entity)
    }

    fn setup_target_scorer_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, target_scorer_system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(minimal_templates());
        app.world_mut().insert_resource(flat_test_map());
        app
    }

    fn spawn_rat_blocked_wander_scorer(
        app: &mut App,
        npc_state: State,
        visible_target: i32,
    ) -> (Entity, Entity) {
        let npc_entity = app
            .world_mut()
            .spawn((
                AnimalFallback {
                    kind: AnimalFallbackKind::Wander,
                    target_id: 1,
                    last_seen_pos: Position { x: 1, y: 0 },
                },
                VisibleTarget::new(visible_target),
                npc_state,
                SubclassNPC,
            ))
            .id();

        let scorer_entity = {
            let mut commands = app.world_mut().commands();
            spawn_scorer(&RatBlockedWanderScorer, &mut commands, npc_entity)
        };
        app.world_mut().flush();

        (npc_entity, scorer_entity)
    }

    #[test]
    fn rat_blocked_wander_scorer_stays_zero_while_hidden() {
        let mut app = App::new();
        app.add_systems(Update, rat_blocked_wander_scorer_system);
        let (_npc_entity, scorer_entity) =
            spawn_rat_blocked_wander_scorer(&mut app, State::Hiding, NO_TARGET);

        app.update();

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), 0.0);
    }

    #[test]
    fn rat_blocked_wander_scorer_resumes_when_not_hidden_and_no_target() {
        let mut app = App::new();
        app.add_systems(Update, rat_blocked_wander_scorer_system);
        let (_npc_entity, scorer_entity) =
            spawn_rat_blocked_wander_scorer(&mut app, State::None, NO_TARGET);

        app.update();

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), 0.5);
    }

    fn spawn_wander_then_hide_scorers(app: &mut App, num_moves: i32) -> (Entity, Entity) {
        let npc_entity = app
            .world_mut()
            .spawn((
                AnimalFallback {
                    kind: AnimalFallbackKind::WanderThenHide,
                    target_id: 1,
                    last_seen_pos: Position { x: 1, y: 0 },
                },
                VisibleTarget::new(NO_TARGET),
                State::None,
                WanderingBehavior { num_moves },
                SubclassNPC,
            ))
            .id();

        let (wander_scorer, hide_scorer) = {
            let mut commands = app.world_mut().commands();
            (
                spawn_scorer(&RatBlockedWanderScorer, &mut commands, npc_entity),
                spawn_scorer(&WolfBlockedHideScorer, &mut commands, npc_entity),
            )
        };
        app.world_mut().flush();

        (wander_scorer, hide_scorer)
    }

    #[test]
    fn cave_bat_template_wanders_then_hides() {
        assert_eq!(
            animal_fallback_kind_for_template("Cave Bat"),
            Some(AnimalFallbackKind::WanderThenHide)
        );
    }

    #[test]
    fn only_bats_traverse_mountains() {
        assert!(can_traverse_mountains("Cave Bat"));
        assert!(can_traverse_mountains("Frost Bat"));
        assert!(!can_traverse_mountains("Giant Rat"));
        assert!(!can_traverse_mountains("Wolf"));
    }

    #[test]
    fn mountains_and_forests_both_count_as_cover() {
        let mut map = flat_test_map();
        let mountain = Position { x: 2, y: 0 };
        let forest = Position { x: 3, y: 0 };
        map.base[(mountain.y * WIDTH + mountain.x) as usize].tile_type = TileType::Mountain;
        map.base[(forest.y * WIDTH + forest.x) as usize].tile_type = TileType::DeciduousForest;

        assert!(is_cover_position(&map, mountain));
        assert!(is_cover_position(&map, forest));
        assert!(!is_cover_position(&map, Position { x: 0, y: 0 }));
    }

    #[test]
    fn wander_then_hide_wanders_before_threshold() {
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                rat_blocked_wander_scorer_system,
                wolf_blocked_hide_scorer_system,
            ),
        );
        let (wander_scorer, hide_scorer) =
            spawn_wander_then_hide_scorers(&mut app, WANDER_THEN_HIDE_MOVES - 1);

        app.update();

        assert_eq!(
            app.world()
                .entity(wander_scorer)
                .get::<Score>()
                .unwrap()
                .get(),
            0.5
        );
        assert_eq!(
            app.world()
                .entity(hide_scorer)
                .get::<Score>()
                .unwrap()
                .get(),
            0.0
        );
    }

    #[test]
    fn wander_then_hide_hides_at_threshold() {
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                rat_blocked_wander_scorer_system,
                wolf_blocked_hide_scorer_system,
            ),
        );
        let (wander_scorer, hide_scorer) =
            spawn_wander_then_hide_scorers(&mut app, WANDER_THEN_HIDE_MOVES);

        app.update();

        assert_eq!(
            app.world()
                .entity(wander_scorer)
                .get::<Score>()
                .unwrap()
                .get(),
            0.0
        );
        assert_eq!(
            app.world()
                .entity(hide_scorer)
                .get::<Score>()
                .unwrap()
                .get(),
            0.5
        );
    }

    #[test]
    fn target_scorer_picks_nearest_visible_player() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Goblin", Position { x: 0, y: 0 }, 10);

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

    #[test]
    fn checkpoint3_personal_assault_target_scorer_stays_with_the_attributed_owner() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Goblin", Position { x: 0, y: 0 }, 10);
        app.world_mut()
            .entity_mut(npc_entity)
            .insert(CrisisAssaultUnit {
                owner_player_id: 2,
                assault_id: 11,
                spawn_generation: 1,
            });

        // Another player's hero and villager are closer, but explicit assault
        // attribution is authoritative for the attacker's target selection.
        for (id, player_id, x, subclass) in [
            (1, 1, 1, Subclass::Hero),
            (3, 1, 2, Subclass::Villager),
            (2, 2, 3, Subclass::Hero),
        ] {
            app.world_mut().spawn((
                Id(id),
                PlayerId(player_id),
                Position { x, y: 0 },
                State::None,
                Class(CLASS_UNIT.to_string()),
                subclass,
                empty_effects(),
                test_stats(),
            ));
        }

        // Storage and sanctuary objects are never generic-combat targets. A
        // cross-owner wall is also excluded from the attributed wall set.
        for (id, player_id, x, subclass) in [
            (4, 1, 4, Subclass::Wall),
            (5, 1, 5, Subclass::Storage),
            (6, MONOLITH_PLAYER_ID, 6, Subclass::Monolith),
        ] {
            app.world_mut().spawn((
                Id(id),
                PlayerId(player_id),
                Position { x, y: 0 },
                State::None,
                Class(CLASS_STRUCTURE.to_string()),
                subclass,
                empty_effects(),
                test_stats(),
            ));
        }

        app.update();

        assert_eq!(
            app.world()
                .entity(npc_entity)
                .get::<VisibleTarget>()
                .unwrap()
                .target,
            2
        );
        assert_eq!(
            app.world()
                .entity(scorer_entity)
                .get::<Score>()
                .unwrap()
                .get(),
            NORMAL_SCORE / 100.0
        );
    }

    #[test]
    fn checkpoint3_personal_assault_target_scorer_rejects_a_cross_owner_fortifier() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Goblin", Position { x: 0, y: 0 }, 10);
        app.world_mut()
            .entity_mut(npc_entity)
            .insert(CrisisAssaultUnit {
                owner_player_id: 2,
                assault_id: 11,
                spawn_generation: 1,
            });

        let owner_hero = app
            .world_mut()
            .spawn((
                Id(2),
                PlayerId(2),
                Position { x: 2, y: 0 },
                State::None,
                Class(CLASS_UNIT.to_string()),
                Subclass::Hero,
                fortified_effects(),
                test_stats(),
                Fortified { id: 9 },
            ))
            .id();
        let foreign_wall = app
            .world_mut()
            .spawn((
                Id(9),
                PlayerId(1),
                Position { x: 3, y: 0 },
                State::None,
                Class(CLASS_STRUCTURE.to_string()),
                Subclass::Wall,
                empty_effects(),
                wall_stats(50),
            ))
            .id();
        {
            let mut entity_map = app.world_mut().resource_mut::<EntityObjMap>();
            entity_map.new_obj(2, owner_hero);
            entity_map.new_obj(9, foreign_wall);
        }

        app.update();

        assert_eq!(
            app.world()
                .entity(npc_entity)
                .get::<VisibleTarget>()
                .unwrap()
                .target,
            NO_TARGET
        );
        assert_eq!(
            app.world()
                .entity(scorer_entity)
                .get::<Score>()
                .unwrap()
                .get(),
            0.0
        );
    }

    #[test]
    fn checkpoint3_personal_assault_action_owner_check_rejects_cross_player_and_neutral_targets() {
        let assault = CrisisAssaultUnit {
            owner_player_id: 7,
            assault_id: 11,
            spawn_generation: 1,
        };

        assert!(personal_assault_allows_target_owner(
            Some(&assault),
            Some(7)
        ));
        for (_kind, owner) in [
            ("other hero", Some(8)),
            ("other villager", Some(8)),
            ("other wall", Some(8)),
            ("other storage", Some(8)),
            ("neutral sanctuary", Some(MONOLITH_PLAYER_ID)),
            ("missing object", None),
        ] {
            assert!(!personal_assault_allows_target_owner(Some(&assault), owner));
        }
        assert!(personal_assault_allows_target_owner(None, Some(8)));
    }

    #[test]
    fn undead_crisis_target_boundary_allows_only_owner_assets_or_same_assault_dead_units() {
        let assault = CrisisAssaultUnit {
            owner_player_id: 7,
            assault_id: 11,
            spawn_generation: 2,
        };
        let other_assault = CrisisAssaultUnit {
            owner_player_id: 7,
            assault_id: 12,
            spawn_generation: 2,
        };

        assert!(personal_assault_allows_target_entity(
            Some(&assault),
            Some(7),
            None,
            false,
        ));
        assert!(personal_assault_allows_target_entity(
            Some(&assault),
            Some(NPC_PLAYER_ID),
            Some(&assault),
            true,
        ));
        assert!(!personal_assault_allows_target_entity(
            Some(&assault),
            Some(NPC_PLAYER_ID),
            Some(&assault),
            false,
        ));
        assert!(!personal_assault_allows_target_entity(
            Some(&assault),
            Some(NPC_PLAYER_ID),
            Some(&other_assault),
            true,
        ));
        assert!(!personal_assault_allows_target_entity(
            Some(&assault),
            Some(8),
            None,
            false,
        ));
    }

    #[test]
    fn checkpoint3_personal_assault_target_installation_rejects_every_foreign_target_kind() {
        for (kind, owner, class, subclass) in [
            ("hero", 8, CLASS_UNIT, Subclass::Hero),
            ("villager", 8, CLASS_UNIT, Subclass::Villager),
            ("wall", 8, CLASS_STRUCTURE, Subclass::Wall),
            ("storage", 8, CLASS_STRUCTURE, Subclass::Storage),
            ("structure", 8, CLASS_STRUCTURE, Subclass::Campfire),
            (
                "sanctuary",
                MONOLITH_PLAYER_ID,
                CLASS_STRUCTURE,
                Subclass::Monolith,
            ),
        ] {
            let mut app = App::new();
            app.add_systems(Update, set_attack_target_system);
            app.world_mut().insert_resource(GameTick(1));
            app.world_mut().insert_resource(MapEvents::default());
            let mut ids = Ids::default();
            ids.new_obj(42, owner);
            app.world_mut().insert_resource(ids);

            let npc = app
                .world_mut()
                .spawn((
                    Id(100),
                    VisibleTarget::new(42),
                    SubclassNPC,
                    CrisisAssaultUnit {
                        owner_player_id: 7,
                        assault_id: 11,
                        spawn_generation: 1,
                    },
                ))
                .id();
            app.world_mut()
                .spawn((Id(42), PlayerId(owner), Class(class.to_string()), subclass));
            let action = app
                .world_mut()
                .spawn((Actor(npc), ActionState::Requested, SetAttackTarget))
                .id();

            app.update();

            assert_eq!(
                *app.world().entity(action).get::<ActionState>().unwrap(),
                ActionState::Failure,
                "foreign {kind} must fail before target installation"
            );
            assert_eq!(
                app.world()
                    .entity(npc)
                    .get::<VisibleTarget>()
                    .unwrap()
                    .target,
                NO_TARGET,
                "foreign {kind} must be cleared"
            );
            assert!(
                app.world().entity(npc).get::<Target>().is_none(),
                "foreign {kind} must never become an attack target"
            );
        }
    }

    #[test]
    fn animal_target_scorer_skips_wall_structure_targets() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Wolf", Position { x: 0, y: 0 }, 10);

        app.world_mut().spawn((
            Id(10),
            PlayerId(1),
            Position { x: 1, y: 0 },
            State::None,
            Class(CLASS_STRUCTURE.to_string()),
            Subclass::Wall,
            empty_effects(),
            test_stats(),
        ));

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert_eq!(visible_target.target, NO_TARGET);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), 0.0);
    }

    #[test]
    fn giant_rat_target_scorer_marks_wander_for_fortified_target() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Giant Rat", Position { x: 0, y: 0 }, 10);

        app.world_mut().spawn((
            Id(18),
            PlayerId(1),
            Position { x: 1, y: 0 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            fortified_effects(),
            test_stats(),
            Fortified { id: 99 },
        ));

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert_eq!(visible_target.target, NO_TARGET);

        let fallback = app
            .world()
            .entity(npc_entity)
            .get::<AnimalFallback>()
            .unwrap();
        assert_eq!(fallback.kind, AnimalFallbackKind::Wander);
        assert_eq!(fallback.target_id, 18);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), 0.0);
    }

    #[test]
    fn animal_target_scorer_skips_fortified_living_targets() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Wolf", Position { x: 0, y: 0 }, 10);

        app.world_mut().spawn((
            Id(11),
            PlayerId(1),
            Position { x: 1, y: 0 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            fortified_effects(),
            test_stats(),
            Fortified { id: 99 },
        ));

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert_eq!(visible_target.target, NO_TARGET);

        let fallback = app
            .world()
            .entity(npc_entity)
            .get::<AnimalFallback>()
            .unwrap();
        assert_eq!(fallback.kind, AnimalFallbackKind::HideInForest);
        assert_eq!(fallback.target_id, 11);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), 0.0);
    }

    #[test]
    fn animal_target_scorer_selects_reachable_living_target() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Wolf", Position { x: 0, y: 0 }, 10);

        app.world_mut().spawn((
            Id(12),
            PlayerId(1),
            Position { x: 1, y: 0 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            empty_effects(),
            test_stats(),
        ));

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert_eq!(visible_target.target, 12);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), NORMAL_SCORE / 100.0);
    }

    #[test]
    fn animal_target_scorer_skips_living_target_blocked_by_stockades() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Wolf", Position { x: 0, y: 25 }, 10);

        for y in 0..HEIGHT {
            app.world_mut().spawn((
                Id(1000 + y),
                PlayerId(1),
                Position { x: 1, y },
                State::None,
                Class(CLASS_STRUCTURE.to_string()),
                Subclass::Wall,
                empty_inventory(1000 + y),
            ));
        }

        app.world_mut().spawn((
            Id(13),
            PlayerId(1),
            Position { x: 2, y: 25 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            empty_effects(),
            test_stats(),
        ));

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert_eq!(visible_target.target, NO_TARGET);

        let fallback = app
            .world()
            .entity(npc_entity)
            .get::<AnimalFallback>()
            .unwrap();
        assert_eq!(fallback.kind, AnimalFallbackKind::HideInForest);
        assert_eq!(fallback.target_id, 13);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), 0.0);
    }

    #[test]
    fn mindless_target_scorer_selects_first_blocking_stockade() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Zombie", Position { x: 0, y: 25 }, 10);

        for y in 0..HEIGHT {
            spawn_stockade_wall(&mut app, 1000 + y, Position { x: 1, y }, 20);
            spawn_stockade_wall(&mut app, 2000 + y, Position { x: 2, y }, 1);
        }

        app.world_mut().spawn((
            Id(14),
            PlayerId(1),
            Position { x: 3, y: 25 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            empty_effects(),
            test_stats(),
        ));

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert_eq!(visible_target.target, 1025);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), NORMAL_SCORE / 100.0);
    }

    #[test]
    fn cunning_target_scorer_selects_weakest_blocking_stockade() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Goblin", Position { x: 0, y: 25 }, 10);

        for y in 0..HEIGHT {
            spawn_stockade_wall(&mut app, 1000 + y, Position { x: 1, y }, 20);
            spawn_stockade_wall(&mut app, 2000 + y, Position { x: 2, y }, 1);
        }

        app.world_mut().spawn((
            Id(15),
            PlayerId(1),
            Position { x: 3, y: 25 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            empty_effects(),
            test_stats(),
        ));

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert!(
            (2000..2000 + HEIGHT).contains(&visible_target.target),
            "expected a weak second-layer stockade, got {}",
            visible_target.target
        );

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), NORMAL_SCORE / 100.0);
    }

    #[test]
    fn personal_assault_target_scorer_breaches_a_fully_enclosed_owner_target() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Goblin", Position { x: 0, y: 25 }, 40);
        app.world_mut()
            .entity_mut(npc_entity)
            .insert(CrisisAssaultUnit {
                owner_player_id: 1,
                assault_id: 11,
                spawn_generation: 1,
            });

        let hero_position = Position { x: 10, y: 25 };
        app.world_mut().spawn((
            Id(15),
            PlayerId(1),
            hero_position,
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::Hero,
            empty_effects(),
            test_stats(),
        ));
        let wall_ids = Map::ring((hero_position.x, hero_position.y), 1)
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| {
                let id = 3_000 + index as i32;
                spawn_stockade_wall(&mut app, id, Position { x, y }, 20);
                id
            })
            .collect::<Vec<_>>();

        app.update();

        let selected = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .expect("visible target")
            .target;
        assert!(
            wall_ids.contains(&selected),
            "personal attacker should breach the enclosing Stockade ring, selected {selected}"
        );
        assert_eq!(
            app.world()
                .entity(scorer_entity)
                .get::<Score>()
                .expect("target score")
                .get(),
            NORMAL_SCORE / 100.0
        );
    }

    #[test]
    fn cunning_target_scorer_uses_open_route_before_battering_stockade() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Goblin", Position { x: 0, y: 25 }, 10);

        spawn_stockade_wall(&mut app, 3000, Position { x: 1, y: 25 }, 1);

        app.world_mut().spawn((
            Id(16),
            PlayerId(1),
            Position { x: 2, y: 25 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            empty_effects(),
            test_stats(),
        ));

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert_eq!(visible_target.target, 16);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), NORMAL_SCORE / 100.0);
    }

    #[test]
    fn caster_target_scorer_keeps_fortified_living_target() {
        let mut app = setup_target_scorer_app();
        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Necromancer", Position { x: 0, y: 0 }, 10);

        spawn_stockade_wall(&mut app, 99, Position { x: 1, y: 0 }, 10);

        app.world_mut().spawn((
            Id(17),
            PlayerId(1),
            Position { x: 1, y: 0 },
            State::None,
            Class(CLASS_UNIT.to_string()),
            Subclass::from_str("soldier"),
            fortified_effects(),
            test_stats(),
            Fortified { id: 99 },
        ));

        app.update();

        let visible_target = app
            .world()
            .entity(npc_entity)
            .get::<VisibleTarget>()
            .unwrap();
        assert_eq!(visible_target.target, 17);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), NORMAL_SCORE / 100.0);
    }

    #[test]
    fn scripted_corpse_hunt_selects_nearest_shipwreck_human_corpse() {
        let mut app = setup_scripted_corpse_hunt_app();
        let corpse_anchor = Position { x: 12, y: 10 };
        let (npc_entity, scorer_entity) =
            spawn_scripted_corpse_hunt_scorer(&mut app, Position { x: 10, y: 10 }, corpse_anchor);

        spawn_corpse_hunt_target(
            &mut app,
            1,
            Position { x: 13, y: 10 },
            CLASS_CORPSE,
            "Human Corpse",
        );
        spawn_corpse_hunt_target(
            &mut app,
            2,
            Position { x: 11, y: 10 },
            CLASS_CORPSE,
            "Human Corpse",
        );
        spawn_corpse_hunt_target(
            &mut app,
            3,
            Position { x: 10, y: 11 },
            CLASS_UNIT,
            "Human Corpse",
        );
        spawn_corpse_hunt_target(
            &mut app,
            4,
            Position { x: 30, y: 30 },
            CLASS_CORPSE,
            "Human Corpse",
        );

        app.update();

        let target = app.world().entity(npc_entity).get::<TaskTarget>().unwrap();
        assert_eq!(target.target, 2);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), URGENT_SCORE / 100.0);
    }

    #[test]
    fn scripted_corpse_hunt_clears_target_when_no_shipwreck_corpse_exists() {
        let mut app = setup_scripted_corpse_hunt_app();
        let (npc_entity, scorer_entity) = spawn_scripted_corpse_hunt_scorer(
            &mut app,
            Position { x: 10, y: 10 },
            Position { x: 12, y: 10 },
        );

        spawn_corpse_hunt_target(
            &mut app,
            1,
            Position { x: 13, y: 10 },
            CLASS_UNIT,
            "Human Corpse",
        );
        spawn_corpse_hunt_target(
            &mut app,
            2,
            Position { x: 11, y: 10 },
            CLASS_CORPSE,
            "Wolf Corpse",
        );

        app.update();

        let target = app.world().entity(npc_entity).get::<TaskTarget>().unwrap();
        assert_eq!(target.target, NO_TARGET);

        let score = app.world().entity(scorer_entity).get::<Score>().unwrap();
        assert_eq!(score.get(), 0.0);
    }

    #[test]
    fn undead_crisis_corpse_scorer_selects_only_same_assault_normal_death() {
        let mut app = App::new();
        app.add_systems(Update, nearby_corpses_scorer_system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));

        let attribution = CrisisAssaultUnit {
            owner_player_id: 7,
            assault_id: 21,
            spawn_generation: 3,
        };
        let npc_entity = app
            .world_mut()
            .spawn((
                Position { x: 5, y: 5 },
                Viewshed { range: 10 },
                TaskTarget::new(NO_TARGET),
                EventExecuting {
                    event_type: String::new(),
                    state: EventExecutingState::None,
                },
                attribution,
                SubclassNPC,
            ))
            .id();
        let scorer_entity = {
            let mut commands = app.world_mut().commands();
            spawn_scorer(&VisibleCorpseScorer, &mut commands, npc_entity)
        };
        app.world_mut().flush();

        let spawn_target =
            |app: &mut App,
             id: i32,
             pos: Position,
             class: &str,
             target_attribution: Option<CrisisAssaultUnit>| {
                let mut entity = app.world_mut().spawn((
                    Id(id),
                    PlayerId(NPC_PLAYER_ID),
                    pos,
                    Name("corpse".to_string()),
                    Template("Zombie".to_string()),
                    Class(class.to_string()),
                    Subclass::Npc,
                    State::Dead,
                    StateDead {
                        dead_at: 0,
                        killer: "test".to_string(),
                    },
                    Misc {
                        image: String::new(),
                        hsl: Vec::new(),
                        groups: Vec::new(),
                    },
                    SubclassNPC,
                ));
                if let Some(target_attribution) = target_attribution {
                    entity.insert(target_attribution);
                }
            };

        // Both ineligible bodies are closer than the valid source.
        spawn_target(&mut app, 101, Position { x: 5, y: 6 }, CLASS_CORPSE, None);
        spawn_target(
            &mut app,
            102,
            Position { x: 6, y: 5 },
            CLASS_UNIT,
            Some(CrisisAssaultUnit {
                assault_id: 22,
                ..attribution
            }),
        );
        spawn_target(
            &mut app,
            103,
            Position { x: 7, y: 5 },
            CLASS_UNIT,
            Some(attribution),
        );

        app.update();

        assert_eq!(
            app.world()
                .entity(npc_entity)
                .get::<TaskTarget>()
                .unwrap()
                .target,
            103
        );
        assert_eq!(
            app.world()
                .entity(scorer_entity)
                .get::<Score>()
                .unwrap()
                .get(),
            PRIORITY2_SCORE / 100.0
        );
    }

    #[test]
    fn scripted_necromancer_thinker_schedules_move_to_shipwreck_corpse() {
        let mut app = setup_scripted_necromancer_brain_app();
        let npc_entity = spawn_scripted_necromancer_brain(
            &mut app,
            Position { x: 16, y: 32 },
            Position { x: 15, y: 36 },
        );
        let corpse_entity = spawn_corpse_hunt_target(
            &mut app,
            12,
            Position { x: 16, y: 35 },
            CLASS_CORPSE,
            "Human Corpse",
        );
        register_test_obj(&mut app, 12, 999, corpse_entity);

        for _ in 0..20 {
            app.update();
        }

        let target = app.world().entity(npc_entity).get::<TaskTarget>().unwrap();
        assert_eq!(target.target, 12);

        let map_events = app.world().resource::<MapEvents>();
        assert!(
            map_events
                .values()
                .any(|event| matches!(event.event_type, VisibleEvent::MoveEvent { .. })),
            "scripted necromancer did not schedule a move event: {:?}",
            map_events
        );
    }

    #[test]
    fn scripted_necromancer_thinker_routes_around_hero_on_old_spawn_tile() {
        let mut app = setup_scripted_necromancer_brain_app();
        let old_necromancer_pos = Position { x: 4, y: 29 };
        let npc_entity = spawn_scripted_necromancer_brain(
            &mut app,
            Position { x: 5, y: 25 },
            Position { x: 5, y: 31 },
        );
        let corpse_entity = spawn_corpse_hunt_target(
            &mut app,
            12,
            Position { x: 4, y: 30 },
            CLASS_CORPSE,
            "Human Corpse",
        );
        register_test_obj(&mut app, 12, 999, corpse_entity);
        spawn_blocking_test_unit(&mut app, 40, old_necromancer_pos);

        for _ in 0..20 {
            app.update();
        }

        let target = app.world().entity(npc_entity).get::<TaskTarget>().unwrap();
        assert_eq!(target.target, 12);

        let map_events = app.world().resource::<MapEvents>();
        let move_event = map_events
            .values()
            .find_map(|event| match &event.event_type {
                VisibleEvent::MoveEvent { dst, .. } => Some(*dst),
                _ => None,
            })
            .expect("scripted necromancer should schedule a move");
        assert_ne!(move_event, old_necromancer_pos);
    }

    #[test]
    fn checkpoint2_protected_target_is_invalidated_without_retargeting_another_player() {
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                protected_target_invalidation_system,
                target_scorer_system,
                clear_protected_target_invalidation_system,
            )
                .chain(),
        );
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(MapEvents::default());
        app.world_mut()
            .insert_resource(SafeLogoutTelemetryState::default());
        app.world_mut().insert_resource(minimal_templates());
        app.world_mut().insert_resource(flat_test_map());

        let mut protected_record = PlayerPresenceRecord::new(false);
        protected_record.state = PlayerWorldPresence::OfflineProtected;
        let mut presence = PlayerWorldPresenceState::default();
        presence.players.insert(1, protected_record);
        app.world_mut().insert_resource(presence);

        let (npc_entity, scorer_entity) =
            spawn_target_scorer(&mut app, "Goblin", Position { x: 0, y: 0 }, 10);
        app.world_mut().entity_mut(npc_entity).insert((
            Id(100),
            State::Moving,
            TaskTarget::new(1),
            Target { id: 1 },
        ));

        let protected_target = app
            .world_mut()
            .spawn((
                Id(1),
                PlayerId(1),
                Position { x: 1, y: 0 },
                State::None,
                Class(CLASS_UNIT.to_string()),
                Subclass::from_str("soldier"),
                empty_effects(),
                test_stats(),
            ))
            .id();
        let other_player_target = app
            .world_mut()
            .spawn((
                Id(2),
                PlayerId(2),
                Position { x: 2, y: 0 },
                State::None,
                Class(CLASS_UNIT.to_string()),
                Subclass::from_str("soldier"),
                empty_effects(),
                test_stats(),
            ))
            .id();

        {
            let mut ids = app.world_mut().resource_mut::<Ids>();
            ids.new_obj(100, NPC_PLAYER_ID);
            ids.new_obj(1, 1);
            ids.new_obj(2, 2);
        }
        {
            let mut entity_map = app.world_mut().resource_mut::<EntityObjMap>();
            entity_map.new_obj(100, npc_entity);
            entity_map.new_obj(1, protected_target);
            entity_map.new_obj(2, other_player_target);
        }
        app.world_mut().resource_mut::<MapEvents>().new(
            100,
            TICKS_PER_SEC + 10,
            VisibleEvent::MoveEvent {
                src: Position { x: 0, y: 0 },
                dst: Position { x: 1, y: 0 },
            },
        );
        app.world_mut().resource_mut::<MapEvents>().new(
            100,
            TICKS_PER_SEC + 10,
            VisibleEvent::SpellDamageEvent {
                spell: Spell::ShadowBolt,
                target_id: 1,
            },
        );

        for _ in 0..3 {
            app.update();
        }

        let npc = app.world().entity(npc_entity);
        assert_eq!(npc.get::<VisibleTarget>().unwrap().target, NO_TARGET);
        assert_eq!(npc.get::<TaskTarget>().unwrap().target, NO_TARGET);
        assert_eq!(*npc.get::<State>().unwrap(), State::None);
        assert!(npc.get::<Target>().is_none());
        assert_eq!(
            app.world()
                .entity(scorer_entity)
                .get::<Score>()
                .unwrap()
                .get(),
            0.0
        );
        assert!(app.world().resource::<MapEvents>().is_empty());
        assert!(npc.get::<ProtectedTargetInvalidated>().is_some());
        let telemetry = app.world().resource::<SafeLogoutTelemetryState>();
        let protected = telemetry.get(&1).expect("protected owner telemetry");
        assert_eq!(protected.protected_target_rejections, 1);
        assert_eq!(protected.queued_events_discarded, 2);
        assert_eq!(protected.protected_damage_blocks, 0);
        assert_eq!(telemetry.len(), 1);

        app.world_mut()
            .resource_mut::<PlayerWorldPresenceState>()
            .players
            .get_mut(&1)
            .unwrap()
            .state = PlayerWorldPresence::Online;
        app.update();
        assert!(app
            .world()
            .entity(npc_entity)
            .get::<ProtectedTargetInvalidated>()
            .is_none());
        app.update();
        assert_eq!(
            app.world()
                .entity(npc_entity)
                .get::<VisibleTarget>()
                .unwrap()
                .target,
            1
        );
        let telemetry = app.world().resource::<SafeLogoutTelemetryState>();
        let protected = telemetry.get(&1).expect("protected owner telemetry");
        assert_eq!(protected.protected_target_rejections, 1);
        assert_eq!(protected.queued_events_discarded, 2);
    }

    #[test]
    fn safe_logout_checkpoint4_npc_damage_boundary_telemetry_is_owner_scoped_and_idempotent() {
        let mut app = App::new();
        app.add_systems(Update, attack_target_system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(MapEvents::default());
        app.world_mut().insert_resource(GameEvents::default());
        app.world_mut().insert_resource(Clients::default());
        app.world_mut().insert_resource(PlayerStats::default());
        app.world_mut().insert_resource(minimal_templates());
        app.world_mut().insert_resource(flat_test_map());
        app.world_mut()
            .insert_resource(SafeLogoutTelemetryState::default());

        let mut protected_record = PlayerPresenceRecord::new(false);
        protected_record.state = PlayerWorldPresence::OfflineProtected;
        let mut presence = PlayerWorldPresenceState::default();
        presence.players.insert(1, protected_record);
        app.world_mut().insert_resource(presence);

        let npc_entity = app
            .world_mut()
            .spawn((
                Id(100),
                PlayerId(NPC_PLAYER_ID),
                Position { x: 0, y: 0 },
                Class(CLASS_UNIT.to_string()),
                Subclass::Npc,
                Template("Goblin".to_string()),
                State::None,
                Misc {
                    image: "goblin".to_string(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
                empty_inventory(100),
                LastCombatTick(0),
                VisibleTarget::new(1),
                SubclassNPC,
                EventExecuting {
                    event_type: String::new(),
                    state: EventExecutingState::None,
                },
            ))
            .id();
        let target_entity = app
            .world_mut()
            .spawn((
                Id(1),
                PlayerId(1),
                Position { x: 1, y: 0 },
                Class(CLASS_UNIT.to_string()),
                Subclass::Hero,
                Template("Human".to_string()),
                State::None,
                Misc {
                    image: "hero".to_string(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
                empty_inventory(1),
                LastCombatTick(0),
            ))
            .id();
        let attack_action = app
            .world_mut()
            .spawn((Actor(npc_entity), ActionState::Requested, AttackTarget))
            .id();

        {
            let mut ids = app.world_mut().resource_mut::<Ids>();
            ids.new_obj(100, NPC_PLAYER_ID);
            // Exercise the final owner-based safety boundary even if the ID
            // ownership index is stale. Gameplay still trusts the ECS owner.
            ids.new_obj(1, 2);
        }
        {
            let mut entity_map = app.world_mut().resource_mut::<EntityObjMap>();
            entity_map.new_obj(100, npc_entity);
            entity_map.new_obj(1, target_entity);
        }

        let hp_before = app.world().entity(target_entity).get::<Stats>().unwrap().hp;
        app.update();
        app.update();

        assert_eq!(
            app.world().entity(target_entity).get::<Stats>().unwrap().hp,
            hp_before
        );
        assert_eq!(
            app.world()
                .entity(npc_entity)
                .get::<VisibleTarget>()
                .unwrap()
                .target,
            NO_TARGET
        );
        assert_eq!(
            *app.world()
                .entity(attack_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Failure
        );
        assert!(app
            .world()
            .entity(npc_entity)
            .get::<ProtectedTargetInvalidated>()
            .is_some());

        let telemetry = app.world().resource::<SafeLogoutTelemetryState>();
        let protected = telemetry.get(&1).expect("protected owner telemetry");
        assert_eq!(protected.protected_target_rejections, 1);
        assert_eq!(protected.protected_damage_blocks, 1);
        assert_eq!(protected.queued_events_discarded, 0);
        assert_eq!(telemetry.len(), 1);
    }

    fn npc_attack_boundary_fixture(
        npc_state: State,
        target_pos: Position,
    ) -> (App, Entity, Entity, Entity) {
        let mut app = App::new();
        app.add_systems(Update, attack_target_system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(MapEvents::default());
        app.world_mut().insert_resource(GameEvents::default());
        app.world_mut().insert_resource(Clients::default());
        app.world_mut().insert_resource(PlayerStats::default());
        app.world_mut().insert_resource(minimal_templates());
        app.world_mut().insert_resource(flat_test_map());

        let npc_entity = app
            .world_mut()
            .spawn((
                Id(100),
                PlayerId(NPC_PLAYER_ID),
                Position { x: 0, y: 0 },
                Class(CLASS_UNIT.to_string()),
                Subclass::Npc,
                Template("Goblin".to_string()),
                npc_state,
                Misc {
                    image: "goblin".to_string(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
                empty_inventory(100),
                LastCombatTick(0),
                VisibleTarget::new(1),
                SubclassNPC,
                EventExecuting {
                    event_type: String::new(),
                    state: EventExecutingState::None,
                },
            ))
            .id();
        let target_entity = app
            .world_mut()
            .spawn((
                Id(1),
                PlayerId(1),
                target_pos,
                Class(CLASS_UNIT.to_string()),
                Subclass::Hero,
                Template("Human".to_string()),
                State::None,
                Misc {
                    image: "hero".to_string(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
                empty_inventory(1),
                LastCombatTick(0),
            ))
            .id();
        let attack_action = app
            .world_mut()
            .spawn((Actor(npc_entity), ActionState::Requested, AttackTarget))
            .id();

        {
            let mut ids = app.world_mut().resource_mut::<Ids>();
            ids.new_obj(100, NPC_PLAYER_ID);
            ids.new_obj(1, 1);
        }
        {
            let mut entity_map = app.world_mut().resource_mut::<EntityObjMap>();
            entity_map.new_obj(100, npc_entity);
            entity_map.new_obj(1, target_entity);
        }

        if npc_state == State::Dead {
            app.world_mut()
                .entity_mut(npc_entity)
                .insert(crate::obj::StateDead {
                    dead_at: TICKS_PER_SEC,
                    killer: "test".to_string(),
                });
        }

        (app, npc_entity, target_entity, attack_action)
    }

    #[test]
    fn npc_attack_revalidates_adjacency_at_the_damage_boundary() {
        let (mut app, npc_entity, target_entity, attack_action) =
            npc_attack_boundary_fixture(State::None, Position { x: 3, y: 0 });

        let hp_before = app.world().entity(target_entity).get::<Stats>().unwrap().hp;
        app.update();

        assert_eq!(
            app.world().entity(target_entity).get::<Stats>().unwrap().hp,
            hp_before,
            "a stale melee action must not damage a target that moved away"
        );
        assert_eq!(
            *app.world()
                .entity(attack_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            app.world()
                .entity(npc_entity)
                .get::<VisibleTarget>()
                .unwrap()
                .target,
            1,
            "the ordinary chase policy should be allowed to retain the target"
        );
    }

    #[test]
    fn npc_attack_rejects_a_stale_action_from_a_dead_attacker() {
        let (mut app, _npc_entity, target_entity, attack_action) =
            npc_attack_boundary_fixture(State::Dead, Position { x: 1, y: 0 });
        let hp_before = app.world().entity(target_entity).get::<Stats>().unwrap().hp;

        app.update();

        assert_eq!(
            app.world().entity(target_entity).get::<Stats>().unwrap().hp,
            hp_before,
            "a stale action from a dead NPC must not deal damage"
        );
        assert_eq!(
            *app.world()
                .entity(attack_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Failure
        );
    }

    #[test]
    fn npc_attack_cooldown_action_fails_safely_after_actor_despawn() {
        let (mut app, npc_entity, target_entity, attack_action) =
            npc_attack_boundary_fixture(State::None, Position { x: 1, y: 0 });
        *app.world_mut()
            .entity_mut(attack_action)
            .get_mut::<ActionState>()
            .unwrap() = ActionState::Executing;
        assert!(app.world_mut().despawn(npc_entity));
        let hp_before = app.world().entity(target_entity).get::<Stats>().unwrap().hp;

        app.update();

        assert_eq!(
            *app.world()
                .entity(attack_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            app.world().entity(target_entity).get::<Stats>().unwrap().hp,
            hp_before
        );
    }

    #[test]
    fn npc_attack_requested_without_event_boundary_has_no_combat_side_effects() {
        let (mut app, npc_entity, target_entity, attack_action) =
            npc_attack_boundary_fixture(State::None, Position { x: 1, y: 0 });
        app.world_mut()
            .entity_mut(npc_entity)
            .remove::<EventExecuting>();
        let hp_before = app.world().entity(target_entity).get::<Stats>().unwrap().hp;

        app.update();

        assert_eq!(
            *app.world()
                .entity(attack_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            app.world().entity(target_entity).get::<Stats>().unwrap().hp,
            hp_before,
            "an orphaned Requested action must not damage its target"
        );
        assert!(
            app.world().resource::<MapEvents>().is_empty(),
            "an orphaned Requested action must not enqueue damage or cooldown events"
        );
    }

    #[test]
    fn checkpoint2_protected_run_npc_scorer_and_action_pause_while_ambient_npc_continues() {
        let mut app = App::new();
        app.add_systems(Update, (no_target_scorer_system, hide_action_system));
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(MapEvents::default());

        let mut ids = Ids::default();
        for object_id in [100, 101, 200, 201] {
            ids.new_obj(object_id, NPC_PLAYER_ID);
        }
        app.world_mut().insert_resource(ids);

        let mut protected_record = PlayerPresenceRecord::new(false);
        protected_record.state = PlayerWorldPresence::OfflineProtected;
        protected_record.protected_run_key = Some(ProtectedRunKey {
            player_id: 1,
            hero_id: 1,
            start_location_name: "test_start".to_string(),
            bound_monolith_id: 2,
            run_object_ids: vec![100, 101],
        });
        let mut presence = PlayerWorldPresenceState::default();
        presence.players.insert(1, protected_record);
        app.world_mut().insert_resource(presence);

        let protected_scorer_actor = app
            .world_mut()
            .spawn((Id(100), SubclassNPC, VisibleTarget::new(NO_TARGET)))
            .id();
        let protected_action_actor = app.world_mut().spawn((Id(101), SubclassNPC)).id();
        let ambient_scorer_actor = app
            .world_mut()
            .spawn((Id(200), SubclassNPC, VisibleTarget::new(NO_TARGET)))
            .id();
        let ambient_action_actor = app.world_mut().spawn((Id(201), SubclassNPC)).id();

        let protected_scorer = {
            let mut commands = app.world_mut().commands();
            spawn_scorer(&NoTargetScorer, &mut commands, protected_scorer_actor)
        };
        let ambient_scorer = {
            let mut commands = app.world_mut().commands();
            spawn_scorer(&NoTargetScorer, &mut commands, ambient_scorer_actor)
        };
        let protected_action = {
            let mut commands = app.world_mut().commands();
            spawn_action(&Hide, &mut commands, protected_action_actor)
        };
        let ambient_action = {
            let mut commands = app.world_mut().commands();
            spawn_action(&Hide, &mut commands, ambient_action_actor)
        };
        app.world_mut().flush();

        for scorer in [protected_scorer, ambient_scorer] {
            app.world_mut()
                .entity_mut(scorer)
                .get_mut::<Score>()
                .unwrap()
                .set(0.37);
        }
        for action in [protected_action, ambient_action] {
            *app.world_mut()
                .entity_mut(action)
                .get_mut::<ActionState>()
                .unwrap() = ActionState::Requested;
        }

        app.update();

        assert_eq!(
            app.world()
                .entity(protected_scorer)
                .get::<Score>()
                .unwrap()
                .get(),
            0.37
        );
        assert_eq!(
            *app.world()
                .entity(protected_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Requested
        );
        assert_eq!(
            app.world()
                .entity(ambient_scorer)
                .get::<Score>()
                .unwrap()
                .get(),
            0.9
        );
        assert_eq!(
            *app.world()
                .entity(ambient_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Success
        );

        let event_actors = app
            .world()
            .resource::<MapEvents>()
            .iter()
            .map(|(_, event)| event.obj_id)
            .collect::<Vec<_>>();
        assert_eq!(event_actors, vec![201]);
    }

    #[test]
    fn move_to_target_missing_event_executing_has_no_movement_side_effects() {
        let mut app = App::new();
        app.add_systems(Update, move_to_target_system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(flat_test_map());
        app.world_mut().insert_resource(MapEvents::default());
        app.world_mut().insert_resource(GameEvents::default());
        app.world_mut().insert_resource(minimal_templates());

        let npc_pos = Position { x: 0, y: 0 };
        let npc_entity = app
            .world_mut()
            .spawn((
                Id(100),
                PlayerId(NPC_PLAYER_ID),
                npc_pos,
                Name("Goblin".to_string()),
                Template("Goblin".to_string()),
                Class(CLASS_UNIT.to_string()),
                Subclass::Npc,
                State::None,
                Misc {
                    image: "goblin".to_string(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
                Target { id: 1 },
            ))
            .id();
        let target_entity = app
            .world_mut()
            .spawn((
                Id(1),
                PlayerId(1),
                Position { x: 3, y: 0 },
                Name("Hero".to_string()),
                Template("Human".to_string()),
                Class(CLASS_UNIT.to_string()),
                Subclass::Hero,
                State::None,
                Misc {
                    image: "hero".to_string(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
            ))
            .id();
        register_test_obj(&mut app, 100, NPC_PLAYER_ID, npc_entity);
        register_test_obj(&mut app, 1, 1, target_entity);

        let move_action = {
            let mut commands = app.world_mut().commands();
            spawn_action(&NpcMoveToTarget, &mut commands, npc_entity)
        };
        app.world_mut().flush();
        *app.world_mut()
            .entity_mut(move_action)
            .get_mut::<ActionState>()
            .unwrap() = ActionState::Requested;

        app.update();

        assert_eq!(
            *app.world()
                .entity(move_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            *app.world().entity(npc_entity).get::<State>().unwrap(),
            State::None
        );
        assert_eq!(
            *app.world().entity(npc_entity).get::<Position>().unwrap(),
            npc_pos
        );
        assert!(app.world().resource::<MapEvents>().is_empty());
        assert!(app
            .world()
            .entity(npc_entity)
            .get::<EventExecuting>()
            .is_none());
    }

    #[test]
    fn personal_assault_executing_move_rejects_foreign_target_and_cancels_queued_event() {
        fn apply_state_change(state_change: On<StateChange>, mut states: Query<&mut State>) {
            if let Ok(mut state) = states.get_mut(state_change.entity) {
                *state = state_change.new_state;
            }
        }

        let mut app = App::new();
        app.add_observer(apply_state_change);
        app.add_systems(Update, move_to_target_system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(flat_test_map());
        app.world_mut().insert_resource(MapEvents::default());
        app.world_mut().insert_resource(GameEvents::default());
        app.world_mut().insert_resource(minimal_templates());

        let npc_pos = Position { x: 4, y: 4 };
        let attribution = CrisisAssaultUnit {
            owner_player_id: 7,
            assault_id: 21,
            spawn_generation: 3,
        };
        let npc_entity = app
            .world_mut()
            .spawn((
                Id(100),
                PlayerId(NPC_PLAYER_ID),
                npc_pos,
                State::Moving,
                SubclassNPC,
                Target { id: 200 },
                EventExecuting {
                    event_type: "move".to_string(),
                    state: EventExecutingState::Executing,
                },
                attribution,
            ))
            .id();
        let foreign_target = app
            .world_mut()
            .spawn((Id(200), PlayerId(8), Position { x: 8, y: 4 }, State::None))
            .id();
        register_test_obj(&mut app, 100, NPC_PLAYER_ID, npc_entity);
        register_test_obj(&mut app, 200, 8, foreign_target);

        let queued_move = app.world_mut().resource_mut::<MapEvents>().new(
            100,
            TICKS_PER_SEC + 10,
            VisibleEvent::MoveEvent {
                src: npc_pos,
                dst: Position { x: 5, y: 4 },
            },
        );
        let move_action = {
            let mut commands = app.world_mut().commands();
            spawn_action(&NpcMoveToTarget, &mut commands, npc_entity)
        };
        app.world_mut().flush();
        *app.world_mut()
            .entity_mut(move_action)
            .get_mut::<ActionState>()
            .unwrap() = ActionState::Executing;

        app.update();

        assert_eq!(
            *app.world()
                .entity(move_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Failure
        );
        let npc = app.world().entity(npc_entity);
        assert_eq!(*npc.get::<Position>().unwrap(), npc_pos);
        assert_eq!(*npc.get::<State>().unwrap(), State::None);
        assert!(npc.get::<Target>().is_none());
        assert_eq!(
            npc.get::<EventExecuting>().unwrap().state,
            EventExecutingState::Failed
        );
        assert!(app
            .world()
            .resource::<MapEvents>()
            .get(&queued_move.event_id)
            .is_none());
    }

    fn npc_move_to_missing_event_fixture(action_state: ActionState) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_systems(Update, move_to_system);
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(flat_test_map());
        app.world_mut().insert_resource(MapEvents::default());
        app.world_mut().insert_resource(GameEvents::default());

        let npc_pos = Position { x: 0, y: 0 };
        let npc_entity = app
            .world_mut()
            .spawn((
                Id(100),
                PlayerId(NPC_PLAYER_ID),
                npc_pos,
                Class(CLASS_UNIT.to_string()),
                Subclass::Npc,
                State::None,
                test_stats(),
                empty_effects(),
                Destination {
                    pos: Position { x: 3, y: 0 },
                },
            ))
            .id();
        register_test_obj(&mut app, 100, NPC_PLAYER_ID, npc_entity);

        let move_action = {
            let mut commands = app.world_mut().commands();
            spawn_action(&NpcMoveTo, &mut commands, npc_entity)
        };
        app.world_mut().flush();
        *app.world_mut()
            .entity_mut(move_action)
            .get_mut::<ActionState>()
            .unwrap() = action_state;

        (app, npc_entity, move_action)
    }

    #[test]
    fn npc_move_to_requested_without_event_boundary_has_no_movement_side_effects() {
        let (mut app, npc_entity, move_action) =
            npc_move_to_missing_event_fixture(ActionState::Requested);

        app.update();

        assert_eq!(
            *app.world()
                .entity(move_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            *app.world().entity(npc_entity).get::<State>().unwrap(),
            State::None
        );
        assert!(app.world().resource::<MapEvents>().is_empty());
    }

    #[test]
    fn npc_move_to_executing_without_event_boundary_fails_safely() {
        let (mut app, npc_entity, move_action) =
            npc_move_to_missing_event_fixture(ActionState::Executing);

        app.update();

        assert_eq!(
            *app.world()
                .entity(move_action)
                .get::<ActionState>()
                .unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            *app.world().entity(npc_entity).get::<State>().unwrap(),
            State::None
        );
        assert!(app.world().resource::<MapEvents>().is_empty());
    }

    fn npc_fallback_movement_fixture() -> (App, Entity) {
        let mut app = App::new();
        app.world_mut().insert_resource(GameTick(TICKS_PER_SEC));
        app.world_mut().insert_resource(Ids::default());
        app.world_mut()
            .insert_resource(EntityObjMap(HashMap::new()));
        app.world_mut().insert_resource(flat_test_map());
        app.world_mut().insert_resource(MapEvents::default());
        app.world_mut().insert_resource(GameEvents::default());
        app.world_mut().insert_resource(minimal_templates());

        let npc_entity = app
            .world_mut()
            .spawn((
                Id(100),
                PlayerId(NPC_PLAYER_ID),
                Position { x: 10, y: 10 },
                Class(CLASS_UNIT.to_string()),
                Subclass::Npc,
                Template("Goblin".to_string()),
                State::None,
                Misc {
                    image: "goblin".to_string(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                test_stats(),
                empty_effects(),
                WanderingBehavior { num_moves: 0 },
                AnimalFallback {
                    kind: AnimalFallbackKind::HideInForest,
                    target_id: 1,
                    last_seen_pos: Position { x: 12, y: 10 },
                },
                SubclassNPC,
            ))
            .id();
        register_test_obj(&mut app, 100, NPC_PLAYER_ID, npc_entity);

        (app, npc_entity)
    }

    #[test]
    fn random_wander_without_event_boundary_has_no_movement_side_effects() {
        let (mut app, npc_entity) = npc_fallback_movement_fixture();
        app.add_systems(Update, random_wander_action_system);
        let action = {
            let mut commands = app.world_mut().commands();
            spawn_action(&RandomWander, &mut commands, npc_entity)
        };
        app.world_mut().flush();
        *app.world_mut()
            .entity_mut(action)
            .get_mut::<ActionState>()
            .unwrap() = ActionState::Requested;

        app.update();

        assert_eq!(
            *app.world().entity(action).get::<ActionState>().unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            *app.world().entity(npc_entity).get::<State>().unwrap(),
            State::None
        );
        assert_eq!(
            app.world()
                .entity(npc_entity)
                .get::<WanderingBehavior>()
                .unwrap()
                .num_moves,
            0
        );
        assert!(app.world().resource::<MapEvents>().is_empty());
    }

    #[test]
    fn move_to_forest_without_event_boundary_has_no_movement_side_effects() {
        let (mut app, npc_entity) = npc_fallback_movement_fixture();
        app.add_systems(Update, move_to_forest_action_system);
        let action = {
            let mut commands = app.world_mut().commands();
            spawn_action(&MoveToForest, &mut commands, npc_entity)
        };
        app.world_mut().flush();
        *app.world_mut()
            .entity_mut(action)
            .get_mut::<ActionState>()
            .unwrap() = ActionState::Requested;

        app.update();

        assert_eq!(
            *app.world().entity(action).get::<ActionState>().unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            *app.world().entity(npc_entity).get::<State>().unwrap(),
            State::None
        );
        assert!(app.world().resource::<MapEvents>().is_empty());
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

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct ScriptedCorpseHuntScorer;

#[derive(Debug, Clone, Component)]
pub struct ScriptedCorpseHunt {
    pub corpse_anchor: Position,
    pub search_radius: u32,
}

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

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct RatBlockedWanderScorer;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct WolfBlockedHideScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct RandomWander;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct MoveToForest;

pub struct NPCPlugin;

impl Plugin for NPCPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            protected_target_invalidation_system
                .before(BigBrainSet::Scorers)
                .before(BigBrainSet::Actions)
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
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
                random_wander_action_system.in_set(BigBrainSet::Actions),
                move_to_forest_action_system.in_set(BigBrainSet::Actions),
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
                rat_blocked_wander_scorer_system.in_set(BigBrainSet::Scorers),
                wolf_blocked_hide_scorer_system.in_set(BigBrainSet::Scorers),
                no_target_scorer_system.in_set(BigBrainSet::Scorers),
                scripted_corpse_hunt_scorer_system.in_set(BigBrainSet::Scorers),
                nearby_corpses_scorer_system.in_set(BigBrainSet::Scorers),
                flee_scorer_system.in_set(BigBrainSet::Scorers),
                spoil_target_scorer_system.in_set(BigBrainSet::Scorers),
                steal_target_scorer_system.in_set(BigBrainSet::Scorers),
                torch_target_scorer_system.in_set(BigBrainSet::Scorers),
            )
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            clear_protected_target_invalidation_system
                .after(BigBrainSet::Scorers)
                .after(BigBrainSet::Actions)
                .run_if(in_state(AppState::Running)),
        );
    }
}

// SCORER SYSTEMS

pub fn target_scorer_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    ids: Option<Res<Ids>>,
    map: Res<Map>,
    entity_map: Res<EntityObjMap>,
    templates: Res<Templates>,
    protection: NpcProtection,
    mut npc_query: Query<
        (
            &PlayerId,
            &Position,
            &Template,
            &Viewshed,
            &mut VisibleTarget,
            Option<&mut TaskTarget>,
            &Stats,
            &EventExecuting,
            Option<&CrisisAssaultUnit>,
            Option<&ProtectedTargetInvalidated>,
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
    blocking_query: Query<BaseQuery>,
    fortified_query: Query<&Fortified>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<VisibleTargetScorer>>,
) {
    if game_tick.0 % TICKS_PER_SEC != 0 {
        return;
    }

    for (Actor(actor), mut score, span) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        let Ok((
            npc_player_id,
            npc_pos,
            npc_template_name,
            npc_viewshed,
            mut npc_visible_target,
            npc_task_target,
            npc_stats,
            event_executing,
            crisis_assault,
            protected_target_invalidated,
        )) = npc_query.get_mut(*actor)
        else {
            span.span().in_scope(|| {
                npc_warn!(*actor, obj_id, None, "Cannot find npc query");
            });
            continue;
        };

        let protected_target_id = [
            npc_visible_target.target,
            npc_task_target
                .as_ref()
                .map(|target| target.target)
                .unwrap_or(NO_TARGET),
        ]
        .into_iter()
        .find(|target_id| {
            *target_id != NO_TARGET
                && protection.object_is_protected_if_ids(*target_id, ids.as_deref())
        });

        if protected_target_invalidated.is_some() || protected_target_id.is_some() {
            npc_visible_target.target = NO_TARGET;
            if let Some(mut task_target) = npc_task_target {
                task_target.target = NO_TARGET;
            }
            let mut entity_commands = commands.entity(*actor);
            entity_commands.try_remove::<Target>();
            if protected_target_invalidated.is_none() {
                entity_commands.try_insert(ProtectedTargetInvalidated {
                    target_id: protected_target_id.expect("protected target was checked"),
                });
            }
            score.set(0.0);
            continue;
        }

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
        let mut animal_fallback: Option<(u32, AnimalFallback)> = None;

        let npc_template = templates
            .obj_templates
            .get_by_name_template(npc_template_name.0.clone(), npc_template_name.0.clone());
        let int = npc_template.int.unwrap_or("mindless".to_string());
        let aggression = npc_template.aggression.unwrap_or("medium".to_string());
        let animal = is_animal(&int);
        let smart_breach = is_cunning(&int);
        let bypass_fortified_wall = can_bypass_fortified_wall(&npc_template_name.0);

        let visible_walls: Vec<WallTargetCandidate> = target_query
            .iter()
            .filter_map(
                |(
                    target_id,
                    target_player,
                    target_pos,
                    target_state,
                    target_class,
                    target_subclass,
                    _target_effects,
                    target_stats,
                )| {
                    if !player::is_player(target_player.0)
                        || protection.owner_is_protected(target_player)
                        || !personal_assault_allows_target_owner(
                            crisis_assault,
                            Some(target_player.0),
                        )
                        || Obj::is_dead(target_state)
                        || target_class.0 != CLASS_STRUCTURE
                        || *target_subclass != Subclass::Wall
                    {
                        return None;
                    }

                    let distance = Map::dist(*npc_pos, *target_pos);
                    if npc_viewshed.range < distance {
                        return None;
                    }

                    Some(WallTargetCandidate {
                        id: target_id.0,
                        player_id: target_player.0,
                        pos: *target_pos,
                        hp: target_stats.hp,
                        distance,
                    })
                },
            )
            .collect();

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

            if protection.owner_is_protected(target_player) {
                continue;
            }

            // Personal-assault attackers pressure only their owning
            // settlement. Nearby players remain free to attack and assist.
            if !personal_assault_allows_target_owner(crisis_assault, Some(target_player.0)) {
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

            let distance = Map::dist(*npc_pos, *target_pos);

            if animal && target_fortified {
                remember_animal_fallback(
                    &mut animal_fallback,
                    npc_template_name.0.as_str(),
                    target_id.0,
                    *target_pos,
                    distance,
                );
                span.span().in_scope(|| {
                    npc_debug!(
                        *actor,
                        obj_id,
                        Some(npc_template_name.0.as_str()),
                        "Skipping fortified target for animal NPC"
                    );
                });
                continue;
            }

            // Skip if npc is strategic and target is stronger and fortified
            /*if (target_fortified || target_subclass.equals(SUBCLASS_WALL))
                && is_strategic(&aggression)
            {
                continue;
            }*/

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
                let blocking_list = Obj::blocking_list_basequery(npc_player_id.0, &blocking_query);
                // The bounded fast search reports its cap as a terminal path.
                // That is useful for ordinary approximate scoring, but it can
                // falsely describe a fully enclosed personal-assault target as
                // reachable and leave the attacker pressed against a wall
                // forever. Personal assaults use the complete 50x50-map search
                // for this reachability decision before selecting a breach.
                let reachable_without_attacking_blockers = if crisis_assault.is_some() {
                    Map::find_path(
                        *npc_pos,
                        *target_pos,
                        &map,
                        npc_player_id.0,
                        blocking_list.clone(),
                        true,
                        false,
                        false,
                        true,
                        false,
                    )
                } else {
                    Map::find_fast_path(
                        *npc_pos,
                        *target_pos,
                        &map,
                        npc_player_id.0,
                        blocking_list.clone(),
                        true,
                        false,
                        false,
                        true,
                        false,
                    )
                }
                .is_some();

                if !reachable_without_attacking_blockers {
                    if animal {
                        remember_animal_fallback(
                            &mut animal_fallback,
                            npc_template_name.0.as_str(),
                            target_id.0,
                            *target_pos,
                            distance,
                        );
                        span.span().in_scope(|| {
                            npc_debug!(
                                *actor,
                                obj_id,
                                Some(npc_template_name.0.as_str()),
                                "Skipping unreachable target for animal NPC"
                            );
                        });
                        continue;
                    }

                    if should_batter_walls(&int, &npc_template_name.0) {
                        if let Some(wall_target) = select_wall_target_from_blocked_path(
                            *npc_pos,
                            *target_pos,
                            npc_player_id.0,
                            &map,
                            blocking_list,
                            &visible_walls,
                            smart_breach,
                            crisis_assault.is_some(),
                        ) {
                            if distance < selected_target.distance {
                                span.span().in_scope(|| {
                                    npc_debug!(
                                        *actor,
                                        obj_id,
                                        Some(npc_template_name.0.as_str()),
                                        "Target blocked by wall, selecting breach target_id={}",
                                        wall_target.id
                                    );
                                });
                                selected_target = NPCTarget {
                                    distance,
                                    ..wall_target
                                };
                            }
                        }
                    }

                    continue;
                }

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
        if selected_target.fortified && !bypass_fortified_wall {
            commands.entity(*actor).remove::<AnimalFallback>();
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

            let fortifier_owner = entity_map
                .get_entity(fortifier.id)
                .and_then(|entity| target_query.get(entity).ok())
                .map(|(_, player_id, ..)| (player_id.0, protection.owner_is_protected(player_id)));
            if fortifier_owner
                .map(|(_, protected)| protected)
                .unwrap_or(false)
                || !personal_assault_allows_target_owner(
                    crisis_assault,
                    fortifier_owner.map(|(owner, _)| owner),
                )
            {
                span.span().in_scope(|| {
                    npc_warn!(
                        *actor,
                        obj_id,
                        Some(npc_template_name.0.as_str()),
                        "Rejecting personal-assault fortification id={} owner={:?} expected_owner={:?}",
                        fortifier.id,
                        fortifier_owner.map(|(owner, _)| owner),
                        crisis_assault.map(|assault| assault.owner_player_id)
                    );
                });
                npc_visible_target.target = NO_TARGET;
                score.set(0.0);
                continue;
            }

            npc_visible_target.target = fortifier.id;
            score.set(NORMAL_SCORE / 100.0);
        } else if selected_target.id != NO_TARGET {
            commands.entity(*actor).remove::<AnimalFallback>();
            span.span().in_scope(|| {
                npc_info!(
                    *actor,
                    obj_id,
                    Some(npc_template_name.0.as_str()),
                    "Selected target_id={}",
                    selected_target.id
                );
            });
            npc_visible_target.target = selected_target.id;
            score.set(NORMAL_SCORE / 100.0);
        } else {
            if let Some((_distance, fallback)) = animal_fallback {
                commands.entity(*actor).insert(fallback);
            }
            span.span().in_scope(|| {
                npc_debug!(
                    *actor,
                    obj_id,
                    Some(npc_template_name.0.as_str()),
                    "No target found"
                );
            });
            npc_visible_target.target = NO_TARGET;
            score.set(0.0);
        }
    }
}

pub fn spoil_target_scorer_system(
    game_tick: Res<GameTick>,
    ids: Option<Res<Ids>>,
    map: Res<Map>,
    protection: NpcProtection,
    invalidated_query: Query<(), With<ProtectedTargetInvalidated>>,
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
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        let Ok((npc_player_id, npc_pos, npc_inventory, mut npc_task_target)) =
            npc_query.get_mut(*actor)
        else {
            continue;
        };

        if invalidated_query.contains(*actor)
            || (npc_task_target.target != NO_TARGET
                && protection.object_is_protected_if_ids(npc_task_target.target, ids.as_deref()))
        {
            npc_task_target.target = NO_TARGET;
            score.set(0.0);
            continue;
        }

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
            if protection.owner_is_protected(target_player) {
                continue;
            }

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
                true,
            ) {
                let mut blocked = false;
                let mut protected_blocker = false;

                let (path, _c) = path_result;
                let next_pos = &path[1];

                for obj in blocking_list {
                    if obj.pos.x == next_pos.0
                        && obj.pos.y == next_pos.1
                        && obj.id.0 != selected_target.id
                    {
                        if protection.owner_is_protected(&obj.player_id) {
                            protected_blocker = true;
                            break;
                        }
                        info!("Target is blocked by {:?}", obj.id);
                        blocked = true;
                        selected_target.id = obj.id.0;
                    }
                }

                if protected_blocker {
                    npc_task_target.target = NO_TARGET;
                    score.set(0.0);
                    continue;
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
    ids: Option<Res<Ids>>,
    map: Res<Map>,
    protection: NpcProtection,
    invalidated_query: Query<(), With<ProtectedTargetInvalidated>>,
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
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        let Ok((npc_player_id, npc_pos, npc_inventory, mut npc_task_target)) =
            npc_query.get_mut(*actor)
        else {
            continue;
        };

        if invalidated_query.contains(*actor)
            || (npc_task_target.target != NO_TARGET
                && protection.object_is_protected_if_ids(npc_task_target.target, ids.as_deref()))
        {
            npc_task_target.target = NO_TARGET;
            score.set(0.0);
            continue;
        }

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
            if protection.owner_is_protected(target_player) {
                continue;
            }

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
                true,
            ) {
                let mut blocked = false;
                let mut protected_blocker = false;

                let (path, _c) = path_result;
                let next_pos = &path[1];

                for obj in blocking_list {
                    if obj.pos.x == next_pos.0
                        && obj.pos.y == next_pos.1
                        && obj.id.0 != selected_target.id
                    {
                        if protection.owner_is_protected(&obj.player_id) {
                            protected_blocker = true;
                            break;
                        }
                        info!("Target is blocked by {:?}", obj.id);
                        blocked = true;
                        selected_target.id = obj.id.0;
                    }
                }

                if protected_blocker {
                    npc_task_target.target = NO_TARGET;
                    score.set(0.0);
                    continue;
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
    ids: Option<Res<Ids>>,
    map: Res<Map>,
    protection: NpcProtection,
    invalidated_query: Query<(), With<ProtectedTargetInvalidated>>,
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
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        let Ok((npc_player_id, npc_pos, mut npc_task_target)) = npc_query.get_mut(*actor) else {
            continue;
        };

        if invalidated_query.contains(*actor)
            || (npc_task_target.target != NO_TARGET
                && protection.object_is_protected_if_ids(npc_task_target.target, ids.as_deref()))
        {
            npc_task_target.target = NO_TARGET;
            score.set(0.0);
            continue;
        }

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
            if protection.owner_is_protected(target_player) {
                continue;
            }

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
                true,
            ) {
                let mut blocked = false;
                let mut protected_blocker = false;

                let (path, _c) = path_result;
                let next_pos = &path[1];

                for obj in blocking_list {
                    if obj.pos.x == next_pos.0
                        && obj.pos.y == next_pos.1
                        && obj.id.0 != selected_target.id
                    {
                        if protection.owner_is_protected(&obj.player_id) {
                            protected_blocker = true;
                            break;
                        }
                        info!("Target is blocked by {:?}", obj.id);
                        blocked = true;
                        selected_target.id = obj.id.0;
                    }
                }

                if protected_blocker {
                    npc_task_target.target = NO_TARGET;
                    score.set(0.0);
                    continue;
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
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    invalidated_query: Query<(), With<ProtectedTargetInvalidated>>,
    mut npc_query: Query<
        (
            &Position,
            &Viewshed,
            &mut TaskTarget,
            &EventExecuting,
            Option<&CrisisAssaultUnit>,
        ),
        With<SubclassNPC>,
    >,
    target_query: Query<ObjQuery>,
    personal_corpse_query: Query<
        (&CrisisAssaultUnit, &State, Option<&StateDead>),
        With<SubclassNPC>,
    >,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<VisibleCorpseScorer>>,
) {
    if game_tick.0 % TICKS_PER_SEC == 0 {
        for (Actor(actor), mut score, _span) in &mut query {
            if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
                continue;
            }

            let Ok((npc_pos, npc_viewshed, mut npc_task_target, event_executing, npc_assault)) =
                npc_query.get_mut(*actor)
            else {
                error!(
                    "Nearby Corpses Scorer => Cannot find npc query for {:?}",
                    *actor
                );
                continue;
            };

            if invalidated_query.contains(*actor)
                || (npc_task_target.target != NO_TARGET
                    && protection
                        .object_is_protected_if_ids(npc_task_target.target, ids.as_deref()))
            {
                npc_task_target.target = NO_TARGET;
                score.set(0.0);
                continue;
            }

            // Skip if currently executing an event
            if event_executing.state == EventExecutingState::Executing {
                score.set(0.0);
                continue;
            }

            let mut min_distance = u32::MAX;
            let mut corpse_id = NO_TARGET;

            for target in target_query.iter() {
                if protection.owner_is_protected(target.player_id) {
                    continue;
                }

                let eligible_corpse = if let Some(npc_assault) = npc_assault {
                    personal_corpse_query
                        .get(target.entity)
                        .map(|(target_assault, target_state, target_dead)| {
                            target_assault == npc_assault
                                && (*target_state == State::Dead || target_dead.is_some())
                        })
                        .unwrap_or(false)
                } else {
                    target.class.0 == CLASS_CORPSE
                };

                if eligible_corpse {
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

pub fn scripted_corpse_hunt_scorer_system(
    game_tick: Res<GameTick>,
    ids: Option<Res<Ids>>,
    map: Res<Map>,
    protection: NpcProtection,
    invalidated_query: Query<(), With<ProtectedTargetInvalidated>>,
    mut npc_query: Query<
        (
            &PlayerId,
            &Position,
            &mut TaskTarget,
            &EventExecuting,
            &ScriptedCorpseHunt,
        ),
        With<SubclassNPC>,
    >,
    target_query: Query<ObjQuery>,
    blocking_query: Query<BaseQuery>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<ScriptedCorpseHuntScorer>>,
) {
    if game_tick.0 % TICKS_PER_SEC != 0 {
        return;
    }

    for (Actor(actor), mut score, _span) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        let Ok((npc_player_id, npc_pos, mut npc_task_target, event_executing, corpse_hunt)) =
            npc_query.get_mut(*actor)
        else {
            score.set(0.0);
            continue;
        };

        if invalidated_query.contains(*actor)
            || (npc_task_target.target != NO_TARGET
                && protection.object_is_protected_if_ids(npc_task_target.target, ids.as_deref()))
        {
            npc_task_target.target = NO_TARGET;
            score.set(0.0);
            continue;
        }

        if event_executing.state == EventExecutingState::Executing {
            score.set(0.0);
            continue;
        }

        let blocking_list = Obj::blocking_list_basequery(npc_player_id.0, &blocking_query);
        let mut selected_corpse_id = NO_TARGET;
        let mut selected_distance = u32::MAX;

        for target in target_query.iter() {
            if protection.owner_is_protected(target.player_id) {
                continue;
            }

            if target.class.0.as_str() != CLASS_CORPSE
                || target.template.0.as_str() != "Human Corpse"
            {
                continue;
            }

            if Map::dist(corpse_hunt.corpse_anchor, *target.pos) > corpse_hunt.search_radius {
                continue;
            }

            let corpse_distance = Map::dist(*npc_pos, *target.pos);
            if corpse_distance > selected_distance {
                continue;
            }

            if !scripted_corpse_hunt_target_reachable(
                *npc_pos,
                *target.pos,
                npc_player_id.0,
                &map,
                blocking_list.clone(),
            ) {
                continue;
            }

            if corpse_distance < selected_distance || target.id.0 < selected_corpse_id {
                selected_distance = corpse_distance;
                selected_corpse_id = target.id.0;
            }
        }

        if selected_corpse_id != NO_TARGET {
            npc_task_target.target = selected_corpse_id;
            score.set(URGENT_SCORE / 100.0);
        } else {
            npc_task_target.target = NO_TARGET;
            score.set(0.0);
        }
    }
}

fn scripted_corpse_hunt_target_reachable(
    npc_pos: Position,
    corpse_pos: Position,
    npc_player_id: i32,
    map: &Map,
    blocking_list: Vec<Blocker>,
) -> bool {
    if npc_pos == corpse_pos {
        return true;
    }

    Map::find_fast_path(
        npc_pos,
        corpse_pos,
        map,
        npc_player_id,
        blocking_list,
        true,
        false,
        false,
        true,
        false,
    )
    .is_some()
}

pub fn flee_scorer_system(
    game_tick: Res<GameTick>,
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    minions_query: Query<&Minions>,
    state_query: Query<&State>,
    entity_map: Res<EntityObjMap>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<FleeScorer>>,
) {
    if game_tick.0 % (TICKS_PER_SEC * 5) == 0 {
        for (Actor(actor), mut score, _span) in &mut query {
            if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
                continue;
            }

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
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    target_query: Query<&VisibleTarget>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<NoTargetScorer>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        if let Ok(target) = target_query.get(*actor) {
            if target.target == NO_TARGET {
                score.set(0.9);
            } else {
                score.set(0.0);
            }
        }
    }
}

pub fn rat_blocked_wander_scorer_system(
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    npc_query: Query<
        (
            &AnimalFallback,
            &VisibleTarget,
            &State,
            Option<&WanderingBehavior>,
        ),
        With<SubclassNPC>,
    >,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<RatBlockedWanderScorer>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        let Ok((fallback, visible_target, npc_state, wandering)) = npc_query.get(*actor) else {
            score.set(0.0);
            continue;
        };

        if wants_to_wander(fallback.kind, wandering)
            && visible_target.target == NO_TARGET
            && *npc_state != State::Hiding
        {
            score.set(0.5);
        } else {
            score.set(0.0);
        }
    }
}

// `WanderThenHide` animals wander for their first few steps and then switch to
// seeking forest cover. `num_moves` is incremented by `random_wander_action_system`.
fn wants_to_wander(kind: AnimalFallbackKind, wandering: Option<&WanderingBehavior>) -> bool {
    match kind {
        AnimalFallbackKind::Wander => true,
        AnimalFallbackKind::WanderThenHide => {
            let num_moves = wandering.map_or(0, |w| w.num_moves);
            num_moves < WANDER_THEN_HIDE_MOVES
        }
        AnimalFallbackKind::HideInForest => false,
    }
}

fn wants_to_hide(kind: AnimalFallbackKind, wandering: Option<&WanderingBehavior>) -> bool {
    match kind {
        AnimalFallbackKind::HideInForest => true,
        AnimalFallbackKind::WanderThenHide => {
            let num_moves = wandering.map_or(0, |w| w.num_moves);
            num_moves >= WANDER_THEN_HIDE_MOVES
        }
        AnimalFallbackKind::Wander => false,
    }
}

pub fn wolf_blocked_hide_scorer_system(
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    npc_query: Query<
        (
            &AnimalFallback,
            &VisibleTarget,
            &State,
            Option<&WanderingBehavior>,
        ),
        With<SubclassNPC>,
    >,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<WolfBlockedHideScorer>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        let Ok((fallback, visible_target, state, wandering)) = npc_query.get(*actor) else {
            score.set(0.0);
            continue;
        };

        if wants_to_hide(fallback.kind, wandering)
            && visible_target.target == NO_TARGET
            && *state != State::Hiding
        {
            score.set(0.5);
        } else {
            score.set(0.0);
        }
    }
}

// ACTION SYSTEMS

pub fn set_attack_target_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    protection: NpcProtection,
    mut map_events: ResMut<MapEvents>,
    mut visible_target_query: Query<(&mut VisibleTarget, &Id), With<SubclassNPC>>,
    crisis_assault_query: Query<&CrisisAssaultUnit>,
    mut query: Query<(&Actor, &mut ActionState, &SetAttackTarget)>,
    mut alerted_npcs: Local<std::collections::HashSet<Entity>>,
) {
    // Clear alert state for any NPC that has lost its target since last tick
    alerted_npcs.retain(|entity| {
        protection.actor_is_protected(*entity, &ids)
            || visible_target_query
                .get(*entity)
                .map_or(false, |(vt, _)| vt.target != NO_TARGET)
    });

    for (Actor(actor), mut state, _set_attack_destination) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting attack target...");
                let Ok((mut visible_target, npc_id)) = visible_target_query.get_mut(*actor) else {
                    continue;
                };

                if visible_target.target != NO_TARGET
                    && protection.object_is_protected(visible_target.target, &ids)
                {
                    visible_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>();
                    alerted_npcs.remove(actor);
                    *state = ActionState::Failure;
                    continue;
                }

                let assault = crisis_assault_query.get(*actor).ok();
                if !personal_assault_allows_target_owner(
                    assault,
                    ids.get_player(visible_target.target),
                ) {
                    npc_warn!(
                        *actor,
                        Some(npc_id.0),
                        None,
                        "Rejecting personal-assault target id={} owner={:?} expected_owner={:?}",
                        visible_target.target,
                        ids.get_player(visible_target.target),
                        assault.map(|assault| assault.owner_player_id)
                    );
                    visible_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>();
                    alerted_npcs.remove(actor);
                    *state = ActionState::Failure;
                    continue;
                }

                npc_info!(
                    *actor,
                    None,
                    None,
                    "Setting attack target to {:?}",
                    visible_target.target
                );
                commands.entity(*actor).try_insert(Target {
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
                npc_debug!(
                    *actor,
                    None,
                    None,
                    "Set Attack Destination action was cancelled. Considering this a failure."
                );
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_torch_target_system(
    mut commands: Commands,
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    mut task_target_query: Query<&mut TaskTarget>,
    mut query: Query<(&Actor, &mut ActionState, &SetTorchTarget)>,
) {
    for (Actor(actor), mut state, _set_attack_destination) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting torch target...");
                let Ok(mut task_target) = task_target_query.get_mut(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                if task_target.target != NO_TARGET
                    && protection.object_is_protected_if_ids(task_target.target, ids.as_deref())
                {
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>();
                    *state = ActionState::Failure;
                    continue;
                }

                commands.entity(*actor).try_insert(Target {
                    id: task_target.target,
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(
                    *actor,
                    None,
                    None,
                    "Set Torch Target action was cancelled. Considering this a failure."
                );
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_spoil_target_system(
    mut commands: Commands,
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    mut task_target_query: Query<&mut TaskTarget>,
    mut query: Query<(&Actor, &mut ActionState, &SetSpoilTarget)>,
) {
    for (Actor(actor), mut state, _set_spoil_target) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting spoil target...");
                let Ok(mut task_target) = task_target_query.get_mut(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                if task_target.target != NO_TARGET
                    && protection.object_is_protected_if_ids(task_target.target, ids.as_deref())
                {
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>();
                    *state = ActionState::Failure;
                    continue;
                }

                commands.entity(*actor).try_insert(Target {
                    id: task_target.target,
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(
                    *actor,
                    None,
                    None,
                    "Set Spoil Target action was cancelled. Considering this a failure."
                );
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_steal_target_system(
    mut commands: Commands,
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    mut task_target_query: Query<&mut TaskTarget>,
    mut query: Query<(&Actor, &mut ActionState, &SetStealTarget)>,
) {
    for (Actor(actor), mut state, _set_steal_target) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting steal target...");
                let Ok(mut task_target) = task_target_query.get_mut(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                if task_target.target != NO_TARGET
                    && protection.object_is_protected_if_ids(task_target.target, ids.as_deref())
                {
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>();
                    *state = ActionState::Failure;
                    continue;
                }

                commands.entity(*actor).try_insert(Target {
                    id: task_target.target,
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(
                    *actor,
                    None,
                    None,
                    "Set Steal Target action was cancelled. Considering this a failure."
                );
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_corpse_target_system(
    mut commands: Commands,
    ids: Option<Res<Ids>>,
    entity_map: Res<EntityObjMap>,
    protection: NpcProtection,
    mut task_target_query: Query<&mut TaskTarget>,
    crisis_assault_query: Query<&CrisisAssaultUnit>,
    dead_query: Query<(&State, Option<&StateDead>), With<SubclassNPC>>,
    mut query: Query<(&Actor, &mut ActionState, &SetCorpseTarget)>,
) {
    for (Actor(actor), mut state, _set_corpse_target) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

        match *state {
            ActionState::Requested => {
                npc_info!(*actor, None, None, "Setting corpse target...");
                let Ok(mut task_target) = task_target_query.get_mut(*actor) else {
                    npc_error!(*actor, None, None, "Query failed to find entity");
                    *state = ActionState::Failure;
                    continue;
                };

                if task_target.target != NO_TARGET
                    && protection.object_is_protected_if_ids(task_target.target, ids.as_deref())
                {
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>();
                    *state = ActionState::Failure;
                    continue;
                }

                if let Ok(actor_assault) = crisis_assault_query.get(*actor) {
                    let eligible = entity_map
                        .get_entity(task_target.target)
                        .and_then(|target_entity| {
                            let target_assault = crisis_assault_query.get(target_entity).ok()?;
                            let (target_state, target_dead) = dead_query.get(target_entity).ok()?;
                            Some(
                                target_assault == actor_assault
                                    && (*target_state == State::Dead || target_dead.is_some()),
                            )
                        })
                        .unwrap_or(false);
                    if !eligible {
                        task_target.target = NO_TARGET;
                        commands.entity(*actor).try_remove::<Target>();
                        *state = ActionState::Failure;
                        continue;
                    }
                }

                commands.entity(*actor).try_insert(Target {
                    id: task_target.target,
                });
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                npc_debug!(
                    *actor,
                    None,
                    None,
                    "Set Corpse Target action was cancelled. Considering this a failure."
                );
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn set_home_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    mut map_events: ResMut<MapEvents>,
    home_query: Query<(&Id, &Home)>,
    mut query: Query<(&Actor, &mut ActionState, &SetHome)>,
) {
    for (Actor(actor), mut state, _set_home) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

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
                npc_debug!(
                    *actor,
                    None,
                    None,
                    "Set Home action was cancelled. Considering this a failure."
                );
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn random_wander_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    protection: NpcProtection,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    mut obj_query: Query<ObjStatQuery>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut wandering_query: Query<&mut WanderingBehavior>,
    mut query: Query<(&Actor, &mut ActionState, &RandomWander, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _random_wander, span) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                let Some(npc_id) = obj_id else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(npc_player_id) = ids.get_player(npc_id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find player id");
                    *state = ActionState::Failure;
                    continue;
                };

                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);
                let Ok(mut npc) = obj_query.get_mut(*actor) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find npc");
                    *state = ActionState::Failure;
                    continue;
                };

                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                if *npc.state == State::Hiding {
                    npc_debug!(*actor, obj_id, None, "Hidden NPC will not wander");
                    *state = ActionState::Failure;
                    continue;
                }

                let mountainwalk = can_traverse_mountains(&npc.template.0);

                let Some(next_pos) = select_random_adjacent_step(
                    *npc.pos,
                    npc_player_id,
                    &map,
                    &collision_list,
                    mountainwalk,
                ) else {
                    span.span().in_scope(|| {
                        npc_debug!(*actor, obj_id, None, "No random wander step available");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let move_duration =
                    npc_move_duration(npc.stats.base_speed, &npc.effects, &templates, 0.75, 1.25);

                // Validate the event boundary before changing actor state,
                // incrementing the wander cadence, or queuing movement. An
                // orphaned Big Brain action must fail without gameplay effects.
                let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                *npc.state = State::Moving;
                commands.trigger(StateChange {
                    entity: *actor,
                    new_state: State::Moving,
                });

                map_events.new(
                    npc.id.0,
                    game_tick.0 + move_duration,
                    VisibleEvent::MoveEvent {
                        src: *npc.pos,
                        dst: Position {
                            x: next_pos.0,
                            y: next_pos.1,
                        },
                    },
                );

                if let Ok(mut wandering_behavior) = wandering_query.get_mut(*actor) {
                    wandering_behavior.num_moves += 1;
                }

                event_executing.state = EventExecutingState::Executing;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok(event_executing) = event_executing_query.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                if !event_executing.state.is_finished() {
                    continue;
                }

                if event_executing.state.is_failed() {
                    npc_debug!(*actor, obj_id, None, "Random wander move failed");
                    *state = ActionState::Failure;
                } else {
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                let Some(npc_id) = obj_id else {
                    *state = ActionState::Failure;
                    continue;
                };

                cancel_npc_events(npc_id, game_tick.0, &mut ids, &mut game_events);
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn move_to_forest_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    mut ids: ResMut<Ids>,
    protection: NpcProtection,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    mut obj_query: Query<ObjStatQuery>,
    fallback_query: Query<&AnimalFallback>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut query: Query<(&Actor, &mut ActionState, &MoveToForest, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _move_to_forest, span) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

        let obj_id = entity_map.get_obj_by_entity(*actor);
        match *state {
            ActionState::Requested => {
                let Some(npc_id) = obj_id else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(npc_player_id) = ids.get_player(npc_id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find player id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(fallback) = fallback_query.get(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);
                let Ok(mut npc) = obj_query.get_mut(*actor) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find npc");
                    *state = ActionState::Failure;
                    continue;
                };

                if is_cover_position(&map, *npc.pos) {
                    span.span().in_scope(|| {
                        npc_debug!(*actor, obj_id, None, "Reached forest or mountain cover");
                    });
                    *state = ActionState::Success;
                    continue;
                }

                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                let mountainwalk = can_traverse_mountains(&npc.template.0);

                let Some(next_pos) = find_nearest_cover_path(
                    *npc.pos,
                    fallback.last_seen_pos,
                    npc_player_id,
                    &map,
                    &collision_list,
                    mountainwalk,
                )
                .and_then(|(path, _cost)| path.get(1).cloned())
                .or_else(|| {
                    select_random_adjacent_step(
                        *npc.pos,
                        npc_player_id,
                        &map,
                        &collision_list,
                        mountainwalk,
                    )
                }) else {
                    span.span().in_scope(|| {
                        npc_debug!(*actor, obj_id, None, "No forest or fallback move found");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let move_duration =
                    npc_move_duration(npc.stats.base_speed, &npc.effects, &templates, 0.85, 1.15);

                let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                *npc.state = State::Moving;
                commands.trigger(StateChange {
                    entity: *actor,
                    new_state: State::Moving,
                });

                map_events.new(
                    npc.id.0,
                    game_tick.0 + move_duration,
                    VisibleEvent::MoveEvent {
                        src: *npc.pos,
                        dst: Position {
                            x: next_pos.0,
                            y: next_pos.1,
                        },
                    },
                );

                event_executing.state = EventExecutingState::Executing;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                if !event_executing.state.is_finished() {
                    continue;
                }

                if event_executing.state.is_failed() {
                    npc_debug!(*actor, obj_id, None, "Move to forest failed");
                    *state = ActionState::Failure;
                    continue;
                }

                let Some(npc_id) = obj_id else {
                    npc_error!(*actor, None, None, "Cannot find obj id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(npc_player_id) = ids.get_player(npc_id) else {
                    npc_error!(*actor, obj_id, None, "Cannot find player id");
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(fallback) = fallback_query.get(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);
                let Ok(mut npc) = obj_query.get_mut(*actor) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find npc");
                    *state = ActionState::Failure;
                    continue;
                };

                if is_cover_position(&map, *npc.pos) {
                    span.span().in_scope(|| {
                        npc_debug!(*actor, obj_id, None, "Reached forest or mountain cover");
                    });
                    *state = ActionState::Success;
                    continue;
                }

                let mountainwalk = can_traverse_mountains(&npc.template.0);

                let Some(next_pos) = find_nearest_cover_path(
                    *npc.pos,
                    fallback.last_seen_pos,
                    npc_player_id,
                    &map,
                    &collision_list,
                    mountainwalk,
                )
                .and_then(|(path, _cost)| path.get(1).cloned())
                .or_else(|| {
                    select_random_adjacent_step(
                        *npc.pos,
                        npc_player_id,
                        &map,
                        &collision_list,
                        mountainwalk,
                    )
                }) else {
                    span.span().in_scope(|| {
                        npc_debug!(*actor, obj_id, None, "No forest or fallback move found");
                    });
                    *state = ActionState::Failure;
                    continue;
                };

                let move_duration =
                    npc_move_duration(npc.stats.base_speed, &npc.effects, &templates, 0.85, 1.15);

                *npc.state = State::Moving;
                commands.trigger(StateChange {
                    entity: *actor,
                    new_state: State::Moving,
                });

                map_events.new(
                    npc.id.0,
                    game_tick.0 + move_duration,
                    VisibleEvent::MoveEvent {
                        src: *npc.pos,
                        dst: Position {
                            x: next_pos.0,
                            y: next_pos.1,
                        },
                    },
                );

                event_executing.state = EventExecutingState::Executing;
            }
            ActionState::Cancelled => {
                let Some(npc_id) = obj_id else {
                    *state = ActionState::Failure;
                    continue;
                };

                cancel_npc_events(npc_id, game_tick.0, &mut ids, &mut game_events);
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
    protection: NpcProtection,
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
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

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
                        true,
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

                        // Big Brain actions may outlive an actor component
                        // removal in the same deferred-command cycle. Validate
                        // the event boundary before changing state or queuing a
                        // movement event so an orphaned action has no gameplay
                        // side effects.
                        let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
                            *state = ActionState::Failure;
                            continue;
                        };

                        span.span().in_scope(|| {
                            npc_trace!(
                                *actor,
                                obj_id,
                                None,
                                "Next pos=({}, {})",
                                next_pos.0,
                                next_pos.1
                            );
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
                let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                span.span().in_scope(|| {
                    npc_trace!(
                        *actor,
                        obj_id,
                        None,
                        "Event state={:?}",
                        event_executing.state
                    );
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
                            true,
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
                            npc_trace!(
                                *actor,
                                obj_id,
                                None,
                                "Next pos=({}, {})",
                                next_pos.0,
                                next_pos.1
                            );
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
    protection: NpcProtection,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    mut obj_query: Query<ObjStatQuery>,
    target_query: Query<&Target>,
    crisis_assault_query: Query<&CrisisAssaultUnit>,
    dead_target_query: Query<(), With<StateDead>>,
    scripted_corpse_hunt_query: Query<(), With<ScriptedCorpseHunt>>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut query: Query<(&Actor, &mut ActionState, &NpcMoveToTarget, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _move_to_target, span) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

        if let Ok(target) = target_query.get(*actor) {
            if target.id != NO_TARGET && protection.object_is_protected(target.id, &ids) {
                commands.entity(*actor).try_remove::<Target>().try_insert(
                    ProtectedTargetInvalidated {
                        target_id: target.id,
                    },
                );
                *state = ActionState::Failure;
                continue;
            }
            if target.id != NO_TARGET {
                let target_entity = entity_map.get_entity(target.id);
                let actor_assault = crisis_assault_query.get(*actor).ok();
                let target_assault =
                    target_entity.and_then(|entity| crisis_assault_query.get(entity).ok());
                if !personal_assault_allows_target_entity(
                    actor_assault,
                    ids.get_player(target.id),
                    target_assault,
                    target_entity
                        .map(|entity| dead_target_query.contains(entity))
                        .unwrap_or(false),
                ) {
                    if let Some(npc_id) = entity_map.get_obj_by_entity(*actor) {
                        map_events.retain(|_, event| {
                            event.obj_id != npc_id
                                || !matches!(&event.event_type, VisibleEvent::MoveEvent { .. })
                        });
                    }
                    if let Ok(mut event_executing) = event_executing_query.get_mut(*actor) {
                        event_executing.state = EventExecutingState::Failed;
                    }
                    commands
                        .entity(*actor)
                        .try_remove::<Target>()
                        .try_remove::<EventInProgress>();
                    commands.trigger(StateChange {
                        entity: *actor,
                        new_state: State::None,
                    });
                    *state = ActionState::Failure;
                    continue;
                }
            }
        }

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
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find entity for {:?}",
                        target.id
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                npc_debug!(*actor, obj_id, None, "Target Entity: {:?}", target_entity);

                // Have to get the list of collision positions before querying the npc and target
                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);

                let entities = [*actor, target_entity];

                let Ok([mut npc, target]) = obj_query.get_many_mut(entities) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Query failed to find entities {:?}",
                        entities
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                let npc_template = templates
                    .obj_templates
                    .get_by_name_template(npc.template.0.clone(), npc.template.0.clone());
                let npc_int = npc_template.int.unwrap_or("mindless".to_string());
                let allow_attackable_blockers =
                    !scripted_corpse_hunt_query.contains(*actor) && !is_animal(&npc_int);
                let mountainwalk = can_traverse_mountains(&npc.template.0);

                let reached_destination = Map::is_adjacent_including_source(*npc.pos, *target.pos);

                if !reached_destination {
                    let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
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

                    let Some(path_result) = Map::find_fast_path(
                        *npc.pos,
                        *target.pos,
                        &map,
                        npc_player_id,
                        collision_list,
                        true,
                        false,
                        mountainwalk,
                        true, // Allow move onto position with transport
                        allow_attackable_blockers,
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

                    event_executing.state = EventExecutingState::Executing;
                }

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                span.span().in_scope(|| {
                    npc_trace!(*actor, obj_id, None, "MoveToTarget executing");
                });
                let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                span.span().in_scope(|| {
                    npc_trace!(
                        *actor,
                        obj_id,
                        None,
                        "Event state={:?}",
                        event_executing.state
                    );
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
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find entity for {:?}",
                        target.id
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                // Have to get the list of collision positions before querying the npc and target
                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);

                let entities = [*actor, target_entity];

                let Ok([mut npc, target]) = obj_query.get_many_mut(entities) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Query failed to find entities {:?}",
                        entities
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                let npc_template = templates
                    .obj_templates
                    .get_by_name_template(npc.template.0.clone(), npc.template.0.clone());
                let npc_int = npc_template.int.unwrap_or("mindless".to_string());
                let allow_attackable_blockers =
                    !scripted_corpse_hunt_query.contains(*actor) && !is_animal(&npc_int);
                let mountainwalk = can_traverse_mountains(&npc.template.0);

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
                        mountainwalk,
                        true, // Allow move onto position with transport
                        allow_attackable_blockers,
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

                let event_type = GameEventType::CancelAllMapEvents { obj_id: npc_id };

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
    protection: NpcProtection,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    mut game_events: ResMut<GameEvents>,
    templates: Res<Templates>,
    mut obj_query: Query<ObjStatQuery>,
    target_query: Query<&Target>,
    crisis_assault_query: Query<&CrisisAssaultUnit>,
    dead_target_query: Query<(), With<StateDead>>,
    event_completed: Query<&EventCompleted>,
    mut event_executing_query: Query<&mut EventExecuting>,
    mut query: Query<(&Actor, &mut ActionState, &NpcMoveNearTarget)>,
) {
    for (Actor(actor), mut state, move_near_target) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

        if let Ok(target) = target_query.get(*actor) {
            if target.id != NO_TARGET && protection.object_is_protected(target.id, &ids) {
                commands.entity(*actor).try_remove::<Target>().try_insert(
                    ProtectedTargetInvalidated {
                        target_id: target.id,
                    },
                );
                *state = ActionState::Failure;
                continue;
            }
            if target.id != NO_TARGET {
                let target_entity = entity_map.get_entity(target.id);
                let actor_assault = crisis_assault_query.get(*actor).ok();
                let target_assault =
                    target_entity.and_then(|entity| crisis_assault_query.get(entity).ok());
                if !personal_assault_allows_target_entity(
                    actor_assault,
                    ids.get_player(target.id),
                    target_assault,
                    target_entity
                        .map(|entity| dead_target_query.contains(entity))
                        .unwrap_or(false),
                ) {
                    if let Some(npc_id) = entity_map.get_obj_by_entity(*actor) {
                        map_events.retain(|_, event| {
                            event.obj_id != npc_id
                                || !matches!(&event.event_type, VisibleEvent::MoveEvent { .. })
                        });
                    }
                    if let Ok(mut event_executing) = event_executing_query.get_mut(*actor) {
                        event_executing.state = EventExecutingState::Failed;
                    }
                    commands
                        .entity(*actor)
                        .try_remove::<Target>()
                        .try_remove::<EventInProgress>();
                    commands.trigger(StateChange {
                        entity: *actor,
                        new_state: State::None,
                    });
                    *state = ActionState::Failure;
                    continue;
                }
            }
        }

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
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find entity for {:?}",
                        target.id
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                // Have to get the list of collision positions before querying the npc and target
                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);

                let entities = [*actor, target_entity];

                let Ok([mut npc, target]) = obj_query.get_many_mut(entities) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Query failed to find entities {:?}",
                        entities
                    );
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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "NPC {:?} is in range of target {:?}",
                        npc_id,
                        target.id
                    );
                    *state = ActionState::Success;
                } else if target_dist > 2 {
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "NPC {:?} is too far from target {:?}",
                        npc_id,
                        target.id
                    );
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
                        true,
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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "NPC {:?} is too close to target {:?}",
                        npc_id,
                        target.id
                    );
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
                        true,
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
                        npc_trace!(
                            *actor,
                            obj_id,
                            None,
                            "Selected pos list: {:?}",
                            selected_pos_list
                        );

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
                    npc_trace!(
                        *actor,
                        obj_id,
                        None,
                        "Moving event still executing, waiting for completed component"
                    );
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
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find entity for {:?}",
                        target.id
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                // Have to get the list of collision positions before querying the npc and target
                let collision_list = Obj::blocking_list_objstatquery(npc_player_id, &obj_query);

                let entities = [*actor, target_entity];

                let Ok([mut npc, target]) = obj_query.get_many_mut(entities) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Query failed to find entities {:?}",
                        entities
                    );
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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "NPC {:?} is in range of target {:?}",
                        npc_id,
                        target.id
                    );
                    *state = ActionState::Success;
                } else if target_dist > 2 {
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "NPC {:?} is too far from target {:?}",
                        npc_id,
                        target.id
                    );
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
                        true,
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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "NPC {:?} is too close to target {:?}",
                        npc_id,
                        target.id
                    );

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
                        true,
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
                        npc_trace!(
                            *actor,
                            obj_id,
                            None,
                            "Selected pos list: {:?}",
                            selected_pos_list
                        );

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

                npc_debug!(
                    *actor,
                    Some(npc_id),
                    None,
                    "MoveNearTarget action was cancelled. Considering this a failure."
                );

                cancel_npc_events(npc_id, game_tick.0, &mut ids, &mut game_events);

                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

// BB-A: which defensive stance beats a given attack type (telegraph hint).
fn defense_hint_for(attack_type: &AttackType) -> String {
    match attack_type {
        AttackType::Quick => "dodge".to_string(),
        AttackType::Precise => "parry".to_string(),
        AttackType::Fierce => "brace".to_string(),
    }
}

fn melee_target_is_still_adjacent(attacker: Position, target: Position) -> bool {
    Map::is_adjacent_excluding_source(attacker, target)
}

// BB-A: bundled so the attack system stays within Bevy's 16-param limit.
// `next_attacks` remembers each NPC's telegraphed upcoming attack type.
#[derive(SystemParam)]
pub struct TelegraphState<'w, 's> {
    clients: Res<'w, Clients>,
    crisis_assault_units: Query<'w, 's, &'static CrisisAssaultUnit>,
    next_attacks: Local<'s, std::collections::HashMap<i32, AttackType>>,
    protection: NpcProtection<'w, 's>,
    telemetry: Option<ResMut<'w, SafeLogoutTelemetryState>>,
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
    fortified_query: Query<&Fortified>,
    mut query: Query<(&Actor, &mut ActionState, &AttackTarget)>,
    mut telegraph: TelegraphState,
) {
    for (Actor(actor), mut state, _chase_attack) in &mut query {
        if telegraph.protection.actor_is_protected(*actor, &ids) {
            continue;
        }

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

                // A Thinker can already have spawned this action when another
                // system kills the NPC. Reject the stale action at the damage
                // boundary even though lethal combat also removes the thinker.
                if npc.stats.hp <= 0 || Obj::is_dead(&npc.state) {
                    npc_debug!(*actor, obj_id, npc_name, "Dead NPC cannot attack");
                    *state = ActionState::Failure;
                    continue;
                }

                // Validate the action's event boundary before target telemetry,
                // damage, telegraph mutation, or cooldown scheduling. Otherwise
                // a missing component could let an orphaned Requested action
                // deal damage and then become retryable as a failed action.
                let Ok(mut event_executing) = event_executing_query.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(mut visible_target) = visible_target_query.get_mut(*actor) else {
                    continue;
                };

                if visible_target.target != NO_TARGET
                    && telegraph
                        .protection
                        .presence
                        .as_deref()
                        .map(|presence| {
                            object_belongs_to_protected_run(visible_target.target, &ids, presence)
                        })
                        .unwrap_or(false)
                {
                    let protected_target_id = visible_target.target;
                    if let (Some(player_id), Some(telemetry)) = (
                        telegraph
                            .protection
                            .presence
                            .as_deref()
                            .and_then(|presence| {
                                protected_player_for_object(protected_target_id, &ids, presence)
                            }),
                        telegraph.telemetry.as_deref_mut(),
                    ) {
                        telemetry.record_protected_target_rejection(player_id);
                    }
                    visible_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: protected_target_id,
                        },
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                let Some(target_entity) = entity_map.get_entity(visible_target.target) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Cannot find target entity {:?}",
                        visible_target.target
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(mut target) = target_query.get_mut(target_entity) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Cannot find target entity {:?}",
                        target_entity
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                Combat::emit_crisis_attack_telemetry(
                    &mut commands,
                    game_tick.0,
                    CrisisAttackTelemetryStage::Requested,
                    &npc,
                    &target,
                );

                let assault = telegraph.crisis_assault_units.get(*actor).ok();
                if !personal_assault_allows_target_owner(assault, Some(target.player_id.0)) {
                    npc_warn!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Rejecting personal-assault action target id={} owner={} expected_owner={:?}",
                        target.id.0,
                        target.player_id.0,
                        assault.map(|assault| assault.owner_player_id)
                    );
                    visible_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>();
                    *state = ActionState::Failure;
                    continue;
                }

                let npc_template = templates
                    .obj_templates
                    .get_by_name_template(npc.template.0.clone(), npc.template.0.clone());
                let npc_int = npc_template.int.unwrap_or("mindless".to_string());

                if is_animal(&npc_int)
                    && (target.class.0 == CLASS_STRUCTURE || target.effects.has(Effect::Fortified))
                {
                    npc_debug!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Animal NPC cannot attack structures or fortified targets"
                    );
                    visible_target.target = NO_TARGET;
                    *state = ActionState::Failure;
                    continue;
                }

                // Get NPC speed, with a small random jitter so NPCs that
                // spawned on the same tick don't attack in perfect lockstep.
                // Jitter is recomputed each swing, so synced NPCs drift apart.
                let jitter = rand::thread_rng().gen_range(0..=NPC_ATTACK_JITTER_TICKS);
                let npc_speed = npc.stats.base_speed.unwrap_or(1) * TICKS_PER_SEC + jitter;

                // Check if target is fortified
                if target.effects.has(Effect::Fortified) {
                    if let Ok(fortification) = fortified_query.get(target.entity) {
                        if telegraph
                            .protection
                            .presence
                            .as_deref()
                            .map(|presence| {
                                object_belongs_to_protected_run(fortification.id, &ids, presence)
                            })
                            .unwrap_or(false)
                            || !personal_assault_allows_target_owner(
                                assault,
                                ids.get_player(fortification.id),
                            )
                        {
                            npc_warn!(
                                *actor,
                                obj_id,
                                npc_name,
                                "Rejecting personal-assault fortification id={} owner={:?} expected_owner={:?}",
                                fortification.id,
                                ids.get_player(fortification.id),
                                assault.map(|assault| assault.owner_player_id)
                            );
                            visible_target.target = NO_TARGET;
                            commands.entity(*actor).try_remove::<Target>();
                            *state = ActionState::Failure;
                            continue;
                        }
                        npc_debug!(
                            *actor,
                            obj_id,
                            npc_name,
                            "Redirecting melee attack to fortification {:?}",
                            fortification.id
                        );
                        visible_target.target = fortification.id;
                        *state = ActionState::Failure;
                    } else {
                        npc_debug!(*actor, obj_id, npc_name, "Cannot attack fortified obj");
                        *state = ActionState::Success;
                    }
                    continue;
                }

                if let Some(errmsg) =
                    Combat::fortified_outbound_attack_error_from_combat(&npc, &target, false)
                {
                    npc_debug!(*actor, obj_id, npc_name, "{}", errmsg);
                    visible_target.target = NO_TARGET;
                    *state = ActionState::Failure;
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

                // Big-brain selected this action from an earlier world view. The
                // target can move before the action runs, so the damage boundary
                // must validate current positions rather than trusting the stale
                // scorer result. Leave the target selected so the ordinary chase
                // policy can resume on the next decision.
                if !melee_target_is_still_adjacent(*npc.pos, *target.pos) {
                    npc_debug!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Target moved out of melee range before attack acceptance"
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                npc_debug!(*actor, obj_id, npc_name, "Target state={:?}", target.state);

                npc_debug!(
                    *actor,
                    obj_id,
                    npc_name,
                    "Target is adjacent, time to attack"
                );
                // BB-A: use the attack type telegraphed last cycle (Quick on the
                // first swing) so the player had a chance to read and counter it.
                let current_attack = telegraph
                    .next_attacks
                    .get(&npc.id.0)
                    .cloned()
                    .unwrap_or(AttackType::Quick);

                if telegraph
                    .protection
                    .presence
                    .as_deref()
                    .map(|presence| {
                        object_belongs_to_protected_run(target.id.0, &ids, presence)
                            || is_owner_offline_protected(target.player_id, presence)
                    })
                    .unwrap_or(false)
                {
                    if let (Some(player_id), Some(telemetry)) = (
                        telegraph
                            .protection
                            .presence
                            .as_deref()
                            .and_then(|presence| {
                                protected_player_for_object(target.id.0, &ids, presence).or_else(
                                    || {
                                        is_owner_offline_protected(target.player_id, presence)
                                            .then_some(target.player_id.0)
                                    },
                                )
                            }),
                        telegraph.telemetry.as_deref_mut(),
                    ) {
                        telemetry.record_protected_target_rejection(player_id);
                        telemetry.record_protected_damage_block(player_id);
                    }
                    visible_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: target.id.0,
                        },
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                Combat::emit_crisis_attack_telemetry(
                    &mut commands,
                    game_tick.0,
                    CrisisAttackTelemetryStage::Accepted,
                    &npc,
                    &target,
                );

                let (damage, combo, _skill_gain, countered) = Combat::process_attack(
                    current_attack.clone(),
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

                // Add visible damage event to broadcast to everyone nearby.
                // A dodged attack reads as a miss.
                Combat::add_damage_event(
                    game_tick.0,
                    current_attack.clone().to_str(),
                    damage,
                    combo,
                    countered.as_deref() == Some("Dodged"),
                    &npc,
                    &target,
                    &mut map_events,
                );

                // BB-A: choose and telegraph the NEXT attack so the player can
                // read it during the cooldown and set the matching defense.
                let next_attack = match rand::thread_rng().gen_range(0..3) {
                    0 => AttackType::Quick,
                    1 => AttackType::Precise,
                    _ => AttackType::Fierce,
                };
                telegraph.next_attacks.insert(npc.id.0, next_attack.clone());

                if ids.is_hero(target.id.0) {
                    if let Some(label) = &countered {
                        let notice = ResponsePacket::Notice {
                            noticemsg: format!(
                                "{}! You countered {}'s {} strike.",
                                label,
                                npc.template.0,
                                current_attack.clone().to_str()
                            ),
                            expiry: Some(3000),
                        };
                        send_to_client(target.player_id.0, notice, &telegraph.clients);
                    }

                    let telegraph_packet = ResponsePacket::CombatTelegraph {
                        attacker_id: npc.id.0,
                        attacker_name: npc.template.0.clone(),
                        attack_type: next_attack.clone().to_str(),
                        defense_hint: defense_hint_for(&next_attack),
                        strike_in: (npc_speed / TICKS_PER_SEC).max(1),
                    };
                    send_to_client(target.player_id.0, telegraph_packet, &telegraph.clients);
                }

                // Add Cooldown Event
                let cooldown_event = VisibleEvent::CooldownEvent {
                    duration: npc_speed,
                };

                map_events.new(npc.id.0, game_tick.0 + npc_speed, cooldown_event);

                event_executing.state = EventExecutingState::Executing;

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok(event_executing) = event_executing_query.get_mut(*actor) else {
                    // A dead actor may be despawned while its cooldown action is
                    // still executing. This is a normal deferred-command race.
                    *state = ActionState::Failure;
                    continue;
                };

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
                npc_debug!(
                    *actor,
                    Some(npc_id),
                    npc_name,
                    "AttackTarget action was cancelled. Considering this a failure."
                );

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
    protection: NpcProtection,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    mut visible_target_query: Query<(&PlayerId, &mut VisibleTarget), Without<EventInProgress>>,
    mut npc_query: Query<CombatQuery, (With<SubclassNPC>, Without<EventInProgress>)>,
    mut target_query: Query<CombatQuery, Without<SubclassNPC>>,
    mut query: Query<(&Actor, &mut ActionState, &mut ChaseAndCast)>,
) {
    for (Actor(actor), mut state, mut chase_and_cast) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

        let Ok((npc_player_id, mut visible_target)) = visible_target_query.get_mut(*actor) else {
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let target_id = visible_target.target;

                if target_id != NO_TARGET && protection.object_is_protected(target_id, &ids) {
                    visible_target.target = NO_TARGET;
                    commands
                        .entity(*actor)
                        .try_remove::<Target>()
                        .try_remove::<EventInProgress>()
                        .try_insert(ProtectedTargetInvalidated { target_id });
                    *state = ActionState::Failure;
                    continue;
                }

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

                // Get NPC move duration, with a small random jitter so NPCs
                // that spawned on the same tick don't move (and so reach their
                // target to attack) in perfect lockstep.
                let move_duration =
                    npc_move_duration(npc.stats.base_speed, &npc.effects, &templates, 0.85, 1.15);

                if target_id != NO_TARGET {
                    // Get target entity
                    let Some(target_entity) = entity_map.get_entity(target_id) else {
                        continue;
                    };

                    let Ok(target) = target_query.get_mut(target_entity) else {
                        continue;
                    };

                    if protection.owner_is_protected(target.player_id) {
                        visible_target.target = NO_TARGET;
                        commands
                            .entity(*actor)
                            .try_remove::<Target>()
                            .try_remove::<EventInProgress>()
                            .try_insert(ProtectedTargetInvalidated { target_id });
                        *state = ActionState::Failure;
                        continue;
                    }

                    if let Some(errmsg) =
                        Combat::fortified_outbound_attack_error_from_combat(&npc, &target, true)
                    {
                        npc_debug!(
                            *actor,
                            Some(npc.id.0),
                            Some(npc.template.0.as_str()),
                            "{}",
                            errmsg
                        );
                        visible_target.target = NO_TARGET;
                        *state = ActionState::Failure;
                        continue;
                    }

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
                                true,
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
                            true,
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
    protection: NpcProtection,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    target_query: Query<&Target>,
    mut npc_query: Query<BaseQueryMutState, With<SubclassNPC>>,
    obj_query: Query<BaseQuery, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    crisis_assault_query: Query<&CrisisAssaultUnit>,
    personal_corpse_query: Query<
        (&Id, &PlayerId, &Position, &CrisisAssaultUnit),
        (With<SubclassNPC>, With<StateDead>),
    >,
    mut query: Query<(&Actor, &mut ActionState, &mut RaiseDead)>,
) {
    for (Actor(actor), mut state, raise_dead) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

        if let Ok(target) = target_query.get(*actor) {
            if target.id != NO_TARGET && protection.object_is_protected(target.id, &ids) {
                commands.entity(*actor).try_remove::<Target>().try_insert(
                    ProtectedTargetInvalidated {
                        target_id: target.id,
                    },
                );
                *state = ActionState::Failure;
                continue;
            }
        }

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
                    npc_info!(
                        *actor,
                        obj_id,
                        npc_name,
                        "NPC state is not none, skipping execution"
                    );
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, npc_name, "NPC is stunned");
                    continue;
                }

                let Ok(target) = target_query.get(*actor) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Query failed to find target entity"
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                // Get target entity
                npc_info!(*actor, obj_id, npc_name, "Task target: {:?}", target.id);
                let Some(target_entity) = entity_map.get_entity(target.id) else {
                    *state = ActionState::Failure;
                    npc_error!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Cannot find target entity for {:?}",
                        target.id
                    );
                    continue;
                };

                let actor_assault = crisis_assault_query.get(*actor).ok().copied();
                let corpse = if let Some(actor_assault) = actor_assault {
                    personal_corpse_query.get(target_entity).ok().and_then(
                        |(id, player_id, pos, target_assault)| {
                            (*target_assault == actor_assault).then_some((
                                id.0,
                                player_id.clone(),
                                *pos,
                            ))
                        },
                    )
                } else {
                    obj_query
                        .get(target_entity)
                        .ok()
                        .map(|corpse| (corpse.id.0, corpse.player_id.clone(), *corpse.pos))
                };
                let Some((corpse_id, corpse_player_id, corpse_pos)) = corpse else {
                    *state = ActionState::Failure;
                    npc_error!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Cannot find eligible target corpse for {:?}",
                        target.id
                    );
                    continue;
                };

                if protection.owner_is_protected(&corpse_player_id) {
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: corpse_id,
                        },
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                npc_info!(*actor, obj_id, npc_name, "Corpse id: {:?}", corpse_id);

                // Check if target is adjacent to npc, this could happen if the home target scorer changes targets
                if !Map::is_adjacent_including_source(*npc.pos, corpse_pos) {
                    npc_info!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Target is not adjacent to npc, raise dead event failed."
                    );
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
                    VisibleEvent::SpellRaiseDeadEvent { corpse_id },
                );

                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let obj_id = entity_map.get_obj_by_entity(*actor);
                let npc_name = npc_query.get(*actor).ok().map(|n| n.template.0.as_str());

                let Ok(_event) = completed_query.get(*actor) else {
                    npc_info!(
                        *actor,
                        obj_id,
                        npc_name,
                        "RaiseDead action still executing, waiting for completed component"
                    );
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
                npc_debug!(
                    *actor,
                    Some(npc_id),
                    npc_name,
                    "RaiseDead action was cancelled. Considering this a failure."
                );

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
                        true,
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
    ids: Option<Res<Ids>>,
    protection: NpcProtection,
    mut map_events: ResMut<MapEvents>,
    obj_query: Query<&Id>,
    mut query: Query<(&Actor, &mut ActionState, &mut Hide, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _hide, _span) in &mut query {
        if protection.actor_is_protected_if_ids(*actor, ids.as_deref()) {
            continue;
        }

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
    protection: NpcProtection,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    mut task_target_query: Query<&mut TaskTarget>,
    mut npc_query: Query<BaseQueryEffects, With<SubclassNPC>>,
    obj_query: Query<BaseQuery, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    mut query: Query<(&Actor, &mut ActionState, &SpoilTarget)>,
) {
    for (Actor(actor), mut state, _spoil_target_action) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "NPC state is not none, skipping execution"
                    );
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                let Ok(mut task_target) = task_target_query.get_mut(*actor) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find target entity");
                    *state = ActionState::Failure;
                    continue;
                };

                if task_target.target != NO_TARGET
                    && protection.object_is_protected(task_target.target, &ids)
                {
                    let protected_target_id = task_target.target;
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: protected_target_id,
                        },
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                // Get target entity
                npc_info!(
                    *actor,
                    obj_id,
                    None,
                    "Task target: {:?}",
                    task_target.target
                );
                let Some(target_entity) = entity_map.get_entity(task_target.target) else {
                    *state = ActionState::Failure;
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find target entity for {:?}",
                        task_target.target
                    );
                    continue;
                };

                let Ok(target) = obj_query.get(target_entity) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Query failed to find entity {:?} for target {:?}",
                        target_entity,
                        task_target.target
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if target is adjacent to npc, this could happen if the torch target scorer changes targets
                if !Map::is_adjacent_including_source(*npc.pos, *target.pos) {
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "Target is not adjacent to npc, spoil event failed."
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                // Check if target has food or drink items
                let food_item = target.inventory.get_by_class(FOOD.to_owned());
                let drink_item = target.inventory.get_by_class(DRINK.to_owned());

                let Some(item) = food_item.or(drink_item) else {
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "Target does not have food or drink items, spoil event failed."
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                if protection.owner_is_protected(target.player_id) {
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: target.id.0,
                        },
                    );
                    *state = ActionState::Failure;
                    continue;
                }

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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "Spoil target action still executing, waiting for completed component"
                    );
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

                npc_debug!(
                    *actor,
                    Some(npc_id),
                    None,
                    "SpoilTarget action was cancelled. Considering this a failure."
                );

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
    protection: NpcProtection,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    mut task_target_query: Query<&mut TaskTarget>,
    mut npc_query: Query<BaseQueryEffects, With<SubclassNPC>>,
    obj_query: Query<BaseQuery, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    items_to_steal_query: Query<&ItemsToSteal>,
    mut query: Query<(&Actor, &mut ActionState, &StealTarget)>,
) {
    for (Actor(actor), mut state, _steal_target_action) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "NPC state is not none, skipping execution"
                    );
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                let Ok(mut task_target) = task_target_query.get_mut(*actor) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find target entity");
                    *state = ActionState::Failure;
                    continue;
                };

                if task_target.target != NO_TARGET
                    && protection.object_is_protected(task_target.target, &ids)
                {
                    let protected_target_id = task_target.target;
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: protected_target_id,
                        },
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                // Get target entity
                npc_info!(
                    *actor,
                    obj_id,
                    None,
                    "Task target: {:?}",
                    task_target.target
                );
                let Some(target_entity) = entity_map.get_entity(task_target.target) else {
                    *state = ActionState::Failure;
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find target entity for {:?}",
                        task_target.target
                    );
                    continue;
                };

                let Ok(target) = obj_query.get(target_entity) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Query failed to find entity {:?} for target {:?}",
                        target_entity,
                        task_target.target
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if target is adjacent to npc, this could happen if the torch target scorer changes targets
                if !Map::is_adjacent_including_source(*npc.pos, *target.pos) {
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "Target is not adjacent to npc, steal event failed."
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                let Ok(items_to_steal) = items_to_steal_query.get(*actor) else {
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "Target does not have defined items to steal, skipping"
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                if protection.owner_is_protected(target.player_id) {
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: target.id.0,
                        },
                    );
                    *state = ActionState::Failure;
                    continue;
                }

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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "Steal target action still executing, waiting for completed component"
                    );
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

                npc_debug!(
                    *actor,
                    Some(npc_id),
                    None,
                    "StealTarget action was cancelled. Considering this a failure."
                );

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
    protection: NpcProtection,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    mut task_target_query: Query<&mut TaskTarget>,
    mut npc_query: Query<BaseQueryEffects, With<SubclassNPC>>,
    obj_query: Query<BaseQuery, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    mut query: Query<(&Actor, &mut ActionState, &TorchTarget)>,
) {
    for (Actor(actor), mut state, _rat_crisis_action) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "NPC state is not none, skipping execution"
                    );
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, None, "NPC is stunned");
                    continue;
                }

                let Ok(mut task_target) = task_target_query.get_mut(*actor) else {
                    npc_error!(*actor, obj_id, None, "Query failed to find target entity");
                    *state = ActionState::Failure;
                    continue;
                };

                if task_target.target != NO_TARGET
                    && protection.object_is_protected(task_target.target, &ids)
                {
                    let protected_target_id = task_target.target;
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: protected_target_id,
                        },
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                // Get target entity
                npc_info!(
                    *actor,
                    obj_id,
                    None,
                    "Task target: {:?}",
                    task_target.target
                );
                let Some(target_entity) = entity_map.get_entity(task_target.target) else {
                    *state = ActionState::Failure;
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Cannot find target entity for {:?}",
                        task_target.target
                    );
                    continue;
                };

                let Ok(target) = obj_query.get(target_entity) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        None,
                        "Query failed to find entity {:?} for target {:?}",
                        target_entity,
                        task_target.target
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                // Check if target is adjacent to npc, this could happen if the torch target scorer changes targets
                if !Map::is_adjacent_including_source(*npc.pos, *target.pos) {
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "Target is not adjacent to npc, torch event failed."
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                if protection.owner_is_protected(target.player_id) {
                    task_target.target = NO_TARGET;
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: target.id.0,
                        },
                    );
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
                    npc_info!(
                        *actor,
                        obj_id,
                        None,
                        "Torch target action still executing, waiting for completed component"
                    );
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

                npc_debug!(
                    *actor,
                    Some(npc_id),
                    None,
                    "TorchTarget action was cancelled. Considering this a failure."
                );

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
    protection: NpcProtection,
    mut game_events: ResMut<GameEvents>,
    mut map_events: ResMut<MapEvents>,
    completed_query: Query<&EventCompleted>,
    target_query: Query<&Target>,
    mut npc_query: Query<BaseQueryMutState, With<SubclassNPC>>,
    obj_query: Query<BaseQueryEffects, Without<SubclassNPC>>, // Without required to prevent disjointed queries
    crisis_assault_query: Query<&CrisisAssaultUnit>,
    fortified_query: Query<&Fortified>,
    mut query: Query<(&Actor, &mut ActionState, &CastSpellTarget)>,
) {
    for (Actor(actor), mut state, _cast_spell_target) in &mut query {
        if protection.actor_is_protected(*actor, &ids) {
            continue;
        }

        if let Ok(target) = target_query.get(*actor) {
            if target.id != NO_TARGET && protection.object_is_protected(target.id, &ids) {
                commands.entity(*actor).try_remove::<Target>().try_insert(
                    ProtectedTargetInvalidated {
                        target_id: target.id,
                    },
                );
                *state = ActionState::Failure;
                continue;
            }
        }

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
                    npc_info!(
                        *actor,
                        obj_id,
                        npc_name,
                        "NPC state is not none, skipping execution"
                    );
                    continue;
                }

                // NPC is stunned, skip execution
                if npc.effects.has(Effect::Stunned) {
                    npc_debug!(*actor, obj_id, npc_name, "NPC is stunned");
                    continue;
                }

                let Ok(target) = target_query.get(*actor) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Query failed to find target entity"
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                // Get target entity
                npc_info!(*actor, obj_id, npc_name, "Task target: {:?}", target.id);
                let Some(target_entity) = entity_map.get_entity(target.id) else {
                    *state = ActionState::Failure;
                    npc_error!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Cannot find target entity for {:?}",
                        target.id
                    );
                    continue;
                };

                let Ok(target) = obj_query.get(target_entity) else {
                    npc_error!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Query failed to find entity {:?} for target",
                        target_entity
                    );
                    *state = ActionState::Failure;
                    continue;
                };

                let actor_assault = crisis_assault_query.get(*actor).ok();
                if !personal_assault_allows_target_owner(actor_assault, Some(target.player_id.0)) {
                    npc_warn!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Rejecting personal-assault spell target id={} owner={} expected_owner={:?}",
                        target.id.0,
                        target.player_id.0,
                        actor_assault.map(|assault| assault.owner_player_id)
                    );
                    commands.entity(*actor).try_remove::<Target>();
                    *state = ActionState::Failure;
                    continue;
                }

                if let Some(errmsg) = Combat::fortified_outbound_attack_error(
                    npc.effects,
                    fortified_query.get(*actor).ok(),
                    target.effects,
                    fortified_query.get(target_entity).ok(),
                    true,
                ) {
                    npc_debug!(*actor, obj_id, npc_name, "{}", errmsg);
                    commands.entity(*actor).remove::<Target>();
                    *state = ActionState::Failure;
                    continue;
                }

                // Check if target is within range
                if Map::dist(*npc.pos, *target.pos) > 2 {
                    npc_info!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Target is not within range, cast spell failed."
                    );
                    *state = ActionState::Failure;
                    continue;
                }

                if protection.owner_is_protected(target.player_id) {
                    commands.entity(*actor).try_remove::<Target>().try_insert(
                        ProtectedTargetInvalidated {
                            target_id: target.id.0,
                        },
                    );
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
                    npc_info!(
                        *actor,
                        obj_id,
                        npc_name,
                        "Cast spell target action still executing, waiting for completed component"
                    );
                    continue;
                };

                npc_info!(
                    *actor,
                    obj_id,
                    npc_name,
                    "Cast spell target action completed"
                );

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
                npc_debug!(
                    *actor,
                    Some(npc_id),
                    npc_name,
                    "CastSpellTarget action was cancelled. Considering this a failure."
                );

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

const WOLF_FOREST_SEARCH_RADIUS: u32 = 10;

// How many wandering steps a `WanderThenHide` animal takes before it gives up
// and retreats to forest cover. Kept well below the wandering despawn threshold
// (see `despawn_wandering_npc_system`) so these animals hide rather than vanish.
const WANDER_THEN_HIDE_MOVES: i32 = 4;

fn remember_animal_fallback(
    fallback: &mut Option<(u32, AnimalFallback)>,
    template: &str,
    target_id: i32,
    target_pos: Position,
    distance: u32,
) {
    let Some(kind) = animal_fallback_kind_for_template(template) else {
        return;
    };

    if fallback
        .as_ref()
        .map_or(true, |(existing_distance, _)| distance < *existing_distance)
    {
        *fallback = Some((
            distance,
            AnimalFallback {
                kind,
                target_id,
                last_seen_pos: target_pos,
            },
        ));
    }
}

fn animal_fallback_kind_for_template(template: &str) -> Option<AnimalFallbackKind> {
    if template == "Giant Rat" {
        Some(AnimalFallbackKind::Wander)
    } else if template.contains("Wolf") {
        Some(AnimalFallbackKind::HideInForest)
    } else if template == "Cave Bat" {
        Some(AnimalFallbackKind::WanderThenHide)
    } else {
        None
    }
}

// Bats fly, so they can cross mountain terrain that ground-bound NPCs cannot.
fn can_traverse_mountains(template: &str) -> bool {
    template.contains("Bat")
}

fn npc_move_duration(
    base_speed: Option<i32>,
    effects: &Effects,
    templates: &Res<Templates>,
    random_min: f32,
    random_max: f32,
) -> i32 {
    let npc_speed = base_speed.unwrap_or(1).max(1);
    let effect_speed_mod = effects.get_speed_effects(templates);
    let random_factor = rand::thread_rng().gen_range(random_min..random_max);

    (BASE_MOVE_TICKS * (BASE_SPEED / npc_speed as f32) * (1.0 / effect_speed_mod) * random_factor)
        as i32
}

fn select_random_adjacent_step(
    npc_pos: Position,
    npc_player_id: i32,
    map: &Map,
    blocking_list: &Vec<Blocker>,
    mountainwalk: bool,
) -> Option<MapPos> {
    let steps = Map::get_neighbour_tiles(
        npc_pos.x,
        npc_pos.y,
        map,
        npc_player_id,
        blocking_list,
        true,
        false,
        mountainwalk,
        false,
        false,
        MapPos(npc_pos.x, npc_pos.y),
    );

    if steps.is_empty() {
        return None;
    }

    let mut rng = rand::thread_rng();
    steps
        .get(rng.gen_range(0..steps.len()))
        .map(|(map_pos, _cost)| map_pos.clone())
}

fn find_nearest_cover_path(
    npc_pos: Position,
    threat_pos: Position,
    npc_player_id: i32,
    map: &Map,
    blocking_list: &Vec<Blocker>,
    mountainwalk: bool,
) -> Option<(Vec<MapPos>, u32)> {
    let mut best_path: Option<(Vec<MapPos>, u32, u32)> = None;

    for (x, y) in Map::range((npc_pos.x, npc_pos.y), WOLF_FOREST_SEARCH_RADIUS) {
        let cover_pos = Position { x, y };
        if cover_pos == npc_pos || !is_cover_position(map, cover_pos) {
            continue;
        }

        let Some((path, cost)) = Map::find_fast_path(
            npc_pos,
            cover_pos,
            map,
            npc_player_id,
            blocking_list.clone(),
            true,
            false,
            mountainwalk,
            // Allow the goal tile itself to be impassable terrain (e.g. a
            // mountain) so animals can take cover on mountain tiles too.
            true,
            false,
        ) else {
            continue;
        };

        if path.len() < 2 {
            continue;
        }

        let threat_distance = Map::dist(cover_pos, threat_pos);
        let should_replace = best_path
            .as_ref()
            .map_or(true, |(_, best_cost, best_threat)| {
                cost < *best_cost || (cost == *best_cost && threat_distance > *best_threat)
            });

        if should_replace {
            best_path = Some((path, cost, threat_distance));
        }
    }

    best_path.map(|(path, cost, _threat_distance)| (path, cost))
}

// Forest or mountain tiles count as cover for animals retreating from a
// fortified target.
fn is_cover_position(map: &Map, pos: Position) -> bool {
    tile_type_at(map, pos).map_or(false, |tile| tile.is_forest() || tile == TileType::Mountain)
}

fn tile_type_at(map: &Map, pos: Position) -> Option<TileType> {
    if pos.x < 0 || pos.y < 0 || pos.x >= map.width || pos.y >= map.height {
        return None;
    }

    map.base
        .get((pos.y * map.width + pos.x) as usize)
        .map(|tile| tile.tile_type)
}

pub fn is_mindless(int: &String) -> bool {
    return *int == "mindless".to_string();
}

pub fn is_animal(int: &String) -> bool {
    return *int == "animal".to_string();
}

pub fn is_cunning(int: &String) -> bool {
    return *int == "cunning".to_string();
}

fn can_bypass_fortified_wall(template: &str) -> bool {
    template.contains("Necromancer")
        || template.contains("Lich")
        || template.contains("Sorcerer")
        || template.contains("Shaman")
}

fn should_batter_walls(int: &String, _template: &str) -> bool {
    is_mindless(int) || is_cunning(int)
}

fn select_wall_target_from_blocked_path(
    npc_pos: Position,
    target_pos: Position,
    npc_player_id: i32,
    map: &Map,
    blocking_list: Vec<Blocker>,
    visible_walls: &[WallTargetCandidate],
    smart_breach: bool,
    complete_search: bool,
) -> Option<NPCTarget> {
    let path = if complete_search {
        Map::find_path(
            npc_pos,
            target_pos,
            map,
            npc_player_id,
            blocking_list.clone(),
            true,
            false,
            false,
            true,
            true,
        )
    } else {
        Map::find_fast_path(
            npc_pos,
            target_pos,
            map,
            npc_player_id,
            blocking_list.clone(),
            true,
            false,
            false,
            true,
            true,
        )
    }?;
    let (path, _cost) = path;

    let mut path_walls: Vec<(usize, WallTargetCandidate)> = Vec::new();

    for (path_index, map_pos) in path.iter().enumerate().skip(1) {
        let Some(blocker) = blocking_list.iter().find(|blocker| {
            blocker.subclass == Subclass::Wall
                && blocker.pos.x == map_pos.0
                && blocker.pos.y == map_pos.1
        }) else {
            continue;
        };

        if let Some(wall) = visible_walls.iter().find(|wall| wall.id == blocker.id.0) {
            path_walls.push((path_index, wall.clone()));
        }
    }

    if path_walls.is_empty() {
        return None;
    }

    let wall = if smart_breach {
        path_walls
            .iter()
            .map(|(_, wall)| wall)
            .min_by_key(|wall| (wall.hp, wall.distance, wall.id))?
            .clone()
    } else {
        path_walls
            .iter()
            .min_by_key(|(path_index, wall)| (*path_index, wall.distance, wall.id))?
            .1
            .clone()
    };

    Some(NPCTarget {
        id: wall.id,
        player_id: wall.player_id,
        pos: wall.pos,
        distance: wall.distance,
        fortified: false,
    })
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
