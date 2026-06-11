use std::collections::HashMap;
use std::i32::MAX;

use bevy::prelude::*;
use big_brain::actions::Steps;
use big_brain::prelude::{Highest, Thinker};

use rand::Rng;

use crate::common::{
    AttackTarget, Destination, Drink, Eat, Heat, Hide, Hunger, Idle, MoveTo, SetAttackTarget,
    Sleep, TaskTarget, Thirst, Tired, Transport,
};
use crate::constants::*;
use crate::effect::Effects;
use crate::event::{EventExecuting, EventExecutingState, MapEvents, VisibleEvent};
use crate::game::{
    EncounterMoves, GameTick, Home, HunterBehavior, Minions, SpoilTargetBehavior, WanderingBehavior,
};
use crate::ids::{EntityObjMap, Ids};
use crate::item::{self, Inventory};
use crate::map::{Map, TileType};
use crate::npc::{
    CastSpellTarget, ChaseAndCast, FleeScorer, FleeToHome, ItemsToSteal, MoveToForest,
    NpcMoveNearTarget, NpcMoveTo, NpcMoveToTarget, RaiseDead, RandomWander, RatBlockedWanderScorer,
    ScriptedCorpseHunt, ScriptedCorpseHuntScorer, SetCorpseTarget, SetHome, SetSpoilTarget,
    SetStealTarget, SetTorchTarget, SpoilTarget, SpoilTargetScorer, StealTarget, StealTargetScorer,
    TorchTarget, TorchTargetScorer, VisibleCorpse, VisibleCorpseScorer, VisibleTarget,
    VisibleTargetScorer, WolfBlockedHideScorer,
};
use crate::obj::{
    ActiveShelter, ActiveTask, BaseAttrs, NewObj, Obj, Order, Personality, SubclassVillager,
};
use crate::obj::{
    Class, Id, LastCombatTick, Misc, Name, PlayerId, Position, State, StateAboard, Stats, Subclass,
    SubclassNPC, Template, Viewshed,
};
use crate::skill::Skills;
use crate::tax_collector::{
    AtLanding, Forfeiture, IsAboard, IsTaxCollected, MoveToEmpire, MoveToPos, MoveToTarget,
    NoTaxesToCollect, OverdueTaxScorer, TaxCollector, TaxCollectorTransport, TaxesToCollect,
};
use crate::villager::{
    ArmedRetaliationScorer, CapacityScorer, DrowsyScorer, EnemyDistanceScorer, ExhaustedScorer,
    FightBack, FindDrink, FindFood, FindShelter, GoodMorale, HeatScorer, HungryScorer, IdleScorer,
    LoadItems, Morale, ProcessOrder, SetFleeDestination, SetOrderDestination,
    SetStorageDestination, StructureCapacityScorer, ThirstyScorer, TransferDrink, TransferFood,
    UnloadItems,
};
use crate::villager_util::VillagerUtil;

use crate::templates::{ObjTemplate, Templates};

const RESCUED_VILLAGER_NEED_PER_TICK: f32 = 0.02;
const RESCUED_VILLAGER_STARTING_THIRST: f32 = 62.0;
const RESCUED_VILLAGER_STARTING_HUNGER: f32 = 18.0;

#[derive(Resource, Deref, DerefMut, Reflect, Debug, Default)]
#[reflect(Resource)]
pub struct EncounterProbability(pub HashMap<i32, Vec<(i32, f32)>>);

#[derive(Debug, Clone)]
pub struct Encounter;

#[derive(Debug, Clone)]
pub struct EncounterMapObj {
    pub player_id: i32,
    pub x: i32,
    pub y: i32,
    pub name: String,
    pub class: String,
    pub subclass: String,
    pub template: String,
}

#[derive(Debug, Clone)]
struct Loot {
    item_name: String,
    drop_rate: f32,
    min: i32,
    max: i32,
}

impl Encounter {
    pub fn probability(moves_since_encounter: i32, wildness: i32) -> f32 {
        // No encounter in safe areas
        if wildness == 0 {
            return 0.0;
        }

        let move_base: f32 = 0.99; // 6 moves to reach 1.0
                                   //let move_base: f32 = 0.95175;  // 3 moves to reach 1.0
        let wildness_base: f32 = 0.985;

        let result = 1.0
            - (move_base.powi(moves_since_encounter.pow(3)))
                * (wildness_base.powi(wildness.pow(3)));
        info!("Encounter probability: {:?}", result);
        return result;
    }

    pub fn get_encounter_pos(
        player_id: i32,
        center_x: i32,
        center_y: i32,
        all_obj_pos: Vec<EncounterMapObj>,
        map: &Map,
    ) -> Option<Position> {
        let mut selected_pos;

        // Check for a valid stop within 2 tiles
        let mut neighbours = Map::range((center_x, center_y), 2);
        selected_pos = Self::find_valid_pos(neighbours, player_id, &all_obj_pos, map);

        // If none found, check for a valid spot on the 3rd and 4th ring
        if selected_pos.is_none() {
            neighbours = Map::ring((center_x, center_y), 3);
            selected_pos = Self::find_valid_pos(neighbours, player_id, &all_obj_pos, map);

            if selected_pos.is_none() {
                neighbours = Map::ring((center_x, center_y), 4);
                selected_pos = Self::find_valid_pos(neighbours, player_id, &all_obj_pos, map);
            }
        }

        debug!("Selected Pos (before fallback): {:?}", selected_pos);

        // If no valid tile can be selected return center x,y
        if selected_pos.is_none() {
            selected_pos = Some(Position {
                x: center_x,
                y: center_y,
            });
        }

        return selected_pos;
    }

    pub fn spawn_npc(
        player_id: i32,
        pos: Position,
        template: String,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
    ) -> (Entity, Id, PlayerId, Position) {
        let npc_id = ids.new_obj_id();
        return Self::spawn_npc_with_id(
            npc_id, player_id, pos, template, commands, ids, entity_map, templates,
        );
    }

    pub fn spawn_npc_with_id(
        npc_id: i32,
        player_id: i32,
        pos: Position,
        template: String,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
    ) -> (Entity, Id, PlayerId, Position) {
        let npc_template = templates.obj_templates.get(template);

        let mut npc = Obj {
            id: Id(npc_id),
            player_id: PlayerId(player_id),
            position: pos,
            name: Name(npc_template.template.clone()),
            template: Template(npc_template.template.clone()),
            class: Class(npc_template.class.clone()),
            subclass: Subclass::from_str(&npc_template.subclass),
            state: State::None,
            misc: Misc {
                image: npc_template.image,
                hsl: Vec::new().into(),
                groups: Vec::new().into(),
            },
            stats: Stats {
                hp: npc_template.base_hp.unwrap(),
                base_hp: npc_template.base_hp.unwrap(),
                stamina: npc_template.base_stamina,
                mana: None,
                base_stamina: npc_template.base_stamina,
                base_mana: None,
                base_def: npc_template.base_def.unwrap(),
                base_damage: npc_template.base_dmg,
                damage_range: npc_template.dmg_range,
                base_speed: npc_template.base_speed,
                base_vision: npc_template.base_vision,
            },
            effects: Effects(HashMap::new()),
            inventory: Inventory {
                owner: npc_id,
                items: Vec::new(),
            },
            last_combat_tick: LastCombatTick::default(),
        };

        Encounter::generate_loot(npc_id, ids, &mut npc.inventory, templates);

        let chase_and_attack = Steps::build()
            .label("Chase and Attack")
            .step(SetAttackTarget)
            .step(NpcMoveToTarget)
            .step(AttackTarget);
        let rat_blocked_wander = Steps::build()
            .label("Rat Blocked Wander")
            .step(RandomWander);
        let wolf_forest_hide = Steps::build()
            .label("Wolf Forest Hide")
            .step(MoveToForest)
            .step(Hide);

        let entity = commands
            .spawn((
                npc,
                Viewshed { range: 2 },
                SubclassNPC,
                VisibleTarget::new(NO_TARGET),
                WanderingBehavior { num_moves: 0 }, // Initialize number of sequential wandering moves
                EventExecuting {
                    event_type: "".to_string(),
                    state: EventExecutingState::None,
                },
                Thinker::build()
                    .label("NPC Chase")
                    .picker(Highest)
                    .when(VisibleTargetScorer, chase_and_attack)
                    .when(WolfBlockedHideScorer, wolf_forest_hide)
                    .when(RatBlockedWanderScorer, rat_blocked_wander),
            ))
            .id();

        ids.new_obj(npc_id, player_id);
        entity_map.new_obj(npc_id, entity);

        return (entity, Id(npc_id), PlayerId(player_id), pos);
    }

    pub fn spawn_necromancer(
        player_id: i32,
        pos: Position,
        home_pos: Position,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
    ) -> (Entity, Id, PlayerId, Position) {
        Self::spawn_necromancer_internal(
            player_id, pos, home_pos, None, commands, ids, entity_map, templates,
        )
    }

    pub fn spawn_necromancer_hunting_corpse(
        player_id: i32,
        pos: Position,
        home_pos: Position,
        corpse_anchor: Position,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
    ) -> (Entity, Id, PlayerId, Position) {
        Self::spawn_necromancer_internal(
            player_id,
            pos,
            home_pos,
            Some(corpse_anchor),
            commands,
            ids,
            entity_map,
            templates,
        )
    }

    pub fn spawn_dormant_necromancer(
        player_id: i32,
        pos: Position,
        home_pos: Position,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
    ) -> (Entity, Id, PlayerId, Position) {
        let necro_id = ids.new_obj_id();

        let mut necro_obj = Obj::create_nospawn(
            necro_id,
            player_id,
            "Necromancer".to_string(),
            pos,
            State::Hiding,
            Inventory {
                owner: necro_id,
                items: Vec::new(),
            },
            templates,
        );

        let template = templates.obj_templates.get("Necromancer".to_string());
        Encounter::generate_loot(necro_id, ids, &mut necro_obj.inventory, templates);

        let necro_entity = commands
            .spawn((
                necro_obj.clone(),
                Viewshed {
                    range: template.base_vision.expect("Necromancer has no vision"),
                },
                SubclassNPC,
                Minions { ids: Vec::new() },
                Home { pos: home_pos },
                VisibleTarget::new(NO_TARGET),
                TaskTarget::new(NO_TARGET),
                EventExecuting {
                    event_type: "".to_string(),
                    state: EventExecutingState::None,
                },
            ))
            .id();

        ids.new_obj(necro_obj.id.0, player_id);
        entity_map.new_obj(necro_obj.id.0, necro_entity);

        (necro_entity, necro_obj.id, PlayerId(player_id), pos)
    }

    pub fn activate_necromancer_hunting_corpse(
        entity: Entity,
        home_pos: Position,
        corpse_anchor: Position,
        commands: &mut Commands,
    ) {
        let cast_spell_target = Steps::build()
            .label("Cast Spell Target")
            .step(SetAttackTarget)
            .step(NpcMoveNearTarget)
            .step(CastSpellTarget);

        let raise_dead = Steps::build()
            .label("Raise Dead")
            .step(SetCorpseTarget)
            .step(NpcMoveToTarget)
            .step(RaiseDead);

        let scripted_raise_dead = Steps::build()
            .label("Scripted Corpse Hunt")
            .step(SetCorpseTarget)
            .step(NpcMoveToTarget)
            .step(RaiseDead);

        let flee_and_hide = Steps::build()
            .label("Flee and Hide")
            .step(SetHome)
            .step(NpcMoveTo)
            .step(Hide)
            .step(Idle {
                start_time: 0,
                duration: MAX,
            });

        commands.entity(entity).insert((
            Home { pos: home_pos },
            VisibleTarget::new(NO_TARGET),
            TaskTarget::new(NO_TARGET),
            EventExecuting {
                event_type: "".to_string(),
                state: EventExecutingState::None,
            },
            ScriptedCorpseHunt {
                corpse_anchor,
                search_radius: 5,
            },
            Thinker::build()
                .label("Necromancer")
                .picker(Highest)
                .when(ScriptedCorpseHuntScorer, scripted_raise_dead)
                .when(VisibleTargetScorer, cast_spell_target)
                .when(VisibleCorpseScorer, raise_dead)
                .when(FleeScorer, flee_and_hide),
        ));
    }

    fn spawn_necromancer_internal(
        player_id: i32,
        pos: Position,
        home_pos: Position,
        corpse_anchor: Option<Position>,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
    ) -> (Entity, Id, PlayerId, Position) {
        let necro_id = ids.new_obj_id();

        let mut necro_obj = Obj::create_nospawn(
            necro_id,
            player_id,
            "Necromancer".to_string(),
            //Position { x: 16, y: 33 },
            pos,
            State::None,
            Inventory {
                owner: necro_id,
                items: Vec::new(),
            },
            templates,
        );

        let template = templates.obj_templates.get("Necromancer".to_string());

        Encounter::generate_loot(necro_id, ids, &mut necro_obj.inventory, templates);

        let cast_spell_target = Steps::build()
            .label("Cast Spell Target")
            .step(SetAttackTarget)
            .step(NpcMoveNearTarget)
            .step(CastSpellTarget);

        let raise_dead = Steps::build()
            .label("Raise Dead")
            .step(SetCorpseTarget)
            .step(NpcMoveToTarget)
            .step(RaiseDead);

        let scripted_raise_dead = Steps::build()
            .label("Scripted Corpse Hunt")
            .step(SetCorpseTarget)
            .step(NpcMoveToTarget)
            .step(RaiseDead);

        let flee_and_hide = Steps::build()
            .label("Flee and Hide")
            .step(SetHome)
            .step(NpcMoveTo)
            .step(Hide)
            .step(Idle {
                start_time: 0,
                duration: MAX,
            });

        let necro_thinker = if corpse_anchor.is_some() {
            Thinker::build()
                .label("Necromancer")
                .picker(Highest)
                .when(ScriptedCorpseHuntScorer, scripted_raise_dead)
                .when(VisibleTargetScorer, cast_spell_target)
                .when(VisibleCorpseScorer, raise_dead)
                .when(FleeScorer, flee_and_hide)
        } else {
            Thinker::build()
                .label("Necromancer")
                .picker(Highest)
                .when(VisibleTargetScorer, cast_spell_target)
                .when(VisibleCorpseScorer, raise_dead)
                .when(FleeScorer, flee_and_hide)
        };

        // Spawn Necromancer
        let mut spawned_necro = commands.spawn((
            necro_obj.clone(),
            Viewshed {
                range: template.base_vision.expect("Necromancer has no vision"),
            },
            SubclassNPC,
            Minions { ids: Vec::new() },
            Home { pos: home_pos },
            VisibleTarget::new(NO_TARGET),
            TaskTarget::new(NO_TARGET),
            EventExecuting {
                event_type: "".to_string(),
                state: EventExecutingState::None,
            },
            necro_thinker,
        ));
        let necro_entity = spawned_necro.id();

        if let Some(corpse_anchor) = corpse_anchor {
            spawned_necro.insert(ScriptedCorpseHunt {
                corpse_anchor,
                search_radius: 5,
            });
        }

        ids.new_obj(necro_obj.id.0, player_id);
        entity_map.new_obj(necro_obj.id.0, necro_entity);

        return (necro_entity, necro_obj.id, PlayerId(player_id), pos);
    }

    pub fn spawn_villager(
        player_id: i32,
        pos: Position,
        hsl: Vec<i32>,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
        game_tick: &Res<GameTick>,
    ) -> (Entity, Id) {
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

        let mut villager = Obj {
            id: Id(villager_id),
            player_id: PlayerId(player_id),
            position: pos,
            name: Name(VillagerUtil::generate_name()),
            template: Template("Human Villager".into()),
            class: Class("unit".into()),
            subclass: Subclass::Villager,
            state: State::None,
            misc: Misc {
                image: image,
                hsl: hsl,
                groups: Vec::new(),
            },
            stats: Stats {
                hp: villager_template.base_hp.unwrap(),
                base_hp: villager_template.base_hp.unwrap(),
                stamina: villager_template.base_stamina,
                mana: None,
                base_stamina: villager_template.base_stamina,
                base_mana: None,
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

        let villager_skills =
            VillagerUtil::generate_skills(villager_id, &templates.skill_templates);
        let base_attrs = VillagerUtil::generate_attributes(1);

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
            .step(ProcessOrder);

        let unload_items = Steps::build()
            .label("UnloadItems")
            .step(SetStorageDestination)
            .step(MoveTo)
            .step(UnloadItems);

        let load_items = Steps::build().label("LoadItems").step(LoadItems);

        let villager_inventory = villager.inventory.clone();

        let villager_entity = commands
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
                ActiveTask::None,
                Order::None,
                ActiveShelter(NO_SHELTER),
                VillagerUtil::generate_personality(),
            ))
            .id();

        commands.entity(villager_entity).insert((
            Thirst::new(
                RESCUED_VILLAGER_STARTING_THIRST,
                RESCUED_VILLAGER_NEED_PER_TICK,
            ),
            Hunger::new(
                RESCUED_VILLAGER_STARTING_HUNGER,
                RESCUED_VILLAGER_NEED_PER_TICK,
            ),
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
        entity_map.new_obj(villager_id, villager_entity);

        return (villager_entity, Id(villager_id));
    }

    /// Turn an existing merchant cargo villager (a bare `Obj` + `BaseAttrs` +
    /// `Skills`, owned by the merchant) into a fully-functional player villager,
    /// in place. Used by the hire flow: the same entity (and thus its advertised
    /// attributes/skills) carries over, so a hired villager matches what the hire
    /// menu showed. Inserts the needs + big-brain Thinker bundle that the cargo
    /// entity lacked, and re-homes it to `player_id` at `pos`.
    ///
    /// Keep the inserted component set in sync with `spawn_villager` above.
    pub fn convert_cargo_to_villager(
        commands: &mut Commands,
        entity: Entity,
        pos: Position,
        player_id: i32,
        inventory: &Inventory,
        templates: &Res<Templates>,
        game_tick: &Res<GameTick>,
    ) {
        let villager_template_name = "Human Villager".to_string();

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
            .step(ProcessOrder);

        let unload_items = Steps::build()
            .label("UnloadItems")
            .step(SetStorageDestination)
            .step(MoveTo)
            .step(UnloadItems);

        let load_items = Steps::build().label("LoadItems").step(LoadItems);

        commands.entity(entity).insert((
            PlayerId(player_id),
            pos,
            Viewshed {
                range: Obj::set_viewshed_range(
                    0,
                    villager_template_name,
                    game_tick.0,
                    inventory,
                    templates,
                    0.0,
                ),
            },
            SubclassVillager,
            EncounterMoves(0),
            EventExecuting {
                event_type: "".to_string(),
                state: EventExecutingState::None,
            },
            ActiveTask::None,
            Order::None,
            ActiveShelter(NO_SHELTER),
            VillagerUtil::generate_personality(),
        ));

        commands.entity(entity).insert((
            Thirst::new(
                RESCUED_VILLAGER_STARTING_THIRST,
                RESCUED_VILLAGER_NEED_PER_TICK,
            ),
            Hunger::new(
                RESCUED_VILLAGER_STARTING_HUNGER,
                RESCUED_VILLAGER_NEED_PER_TICK,
            ),
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
    }

    pub fn spawn_tax_collector(
        player_id: i32,
        landing_pos: Position,
        empire_pos: Position,
        target_player: i32,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
        game_tick: &Res<GameTick>,
        map_events: &mut ResMut<MapEvents>,
    ) {
        let tax_collector_ship_id = ids.new_obj_id();
        let tax_collector_ship_obj = Obj::create_nospawn(
            tax_collector_ship_id,
            player_id,
            "Tax Ship".to_string(),
            empire_pos,
            State::None,
            Inventory {
                owner: tax_collector_ship_id,
                items: Vec::new(),
            },
            templates,
        );

        let tax_collector_id = ids.new_obj_id();
        let tax_collector_obj = Obj::create_nospawn(
            tax_collector_id,
            player_id,
            "Tax Collector".to_string(),
            empire_pos,
            State::None,
            Inventory {
                owner: tax_collector_id,
                items: Vec::new(),
            },
            templates,
        );

        let move_to_empire_and_idle = Steps::build()
            .label("MoveToEmpire and Idle")
            .step(MoveToEmpire)
            .step(Idle {
                start_time: 0,
                duration: 100,
            });

        let move_to_landing_and_idle = Steps::build()
            .label("MoveToPos and Idle")
            .step(MoveToPos)
            .step(Idle {
                start_time: 0,
                duration: 100,
            });

        // Spawn Tax Collector Ship
        let tax_collector_ship_entity = commands
            .spawn((
                tax_collector_ship_obj.clone(),
                SubclassNPC,
                Transport {
                    route: Vec::new(),
                    next_stop: 0,
                    hauling: vec![tax_collector_obj.id.0],
                },
                Destination { pos: landing_pos },
                TaxCollectorTransport {
                    tax_collector_id: tax_collector_obj.id.0,
                },
                EventExecuting {
                    event_type: "".to_string(),
                    state: EventExecutingState::None,
                },
                Thinker::build()
                    .label("Tax Collector Ship")
                    .picker(Highest)
                    .when(NoTaxesToCollect, move_to_empire_and_idle)
                    .when(TaxesToCollect, move_to_landing_and_idle),
            ))
            .id();

        ids.new_obj(tax_collector_ship_obj.id.0, player_id);

        entity_map.new_obj(tax_collector_ship_obj.id.0, tax_collector_ship_entity);

        // Create a new object event
        commands.trigger(NewObj {
            entity: tax_collector_ship_entity,
        });

        let target_hero_id = ids
            .get_hero(target_player)
            .expect("Cannot find hero for player");

        let move_to_hero_and_idle = Steps::build()
            .label("MoveToTarget and Idle")
            .step(MoveToTarget {
                target: target_hero_id,
            })
            .step(Idle {
                start_time: 0,
                duration: 100,
            });

        let move_to_ship_and_idle = Steps::build()
            .label("MoveToTarget and Idle")
            /* .step(Talk {
                speech: "Keep working, peasants — I’ll be back when it hurts most.".to_string(),
            })*/
            .step(MoveToTarget {
                target: tax_collector_ship_obj.id.0,
            })
            .step(Idle {
                start_time: 0,
                duration: 100,
            });

        let forfeiture = Steps::build()
            .label("Forfeiture")
            .step(MoveToTarget {
                target: target_hero_id,
            })
            .step(Forfeiture)
            .step(MoveToTarget {
                target: tax_collector_ship_obj.id.0,
            })
            .step(Idle {
                start_time: 0,
                duration: 100,
            });

        // Spawn Tax Collector
        let tax_collector_entity = commands
            .spawn((
                tax_collector_obj.clone(),
                SubclassNPC,
                TaxCollector {
                    target_player: target_player,
                    collection_amount: 0,
                    debt_amount: 0,
                    last_collection_time: game_tick.0 - 1000,
                    landing_pos: landing_pos,
                    transport_id: tax_collector_ship_obj.id.0,
                    last_demand_time: 0,
                },
                StateAboard {
                    transport_id: tax_collector_ship_obj.id.0,
                },
                EventExecuting {
                    event_type: "".to_string(),
                    state: EventExecutingState::None,
                },
                Thinker::build()
                    .label("Tax Collector")
                    .picker(Highest)
                    .when(
                        IsAboard,
                        Idle {
                            start_time: 0,
                            duration: 100,
                        },
                    )
                    .when(AtLanding, move_to_hero_and_idle)
                    .when(IsTaxCollected, move_to_ship_and_idle)
                    .when(OverdueTaxScorer, forfeiture),
            ))
            .id();

        ids.new_obj(tax_collector_obj.id.0, player_id);
        entity_map.new_obj(tax_collector_obj.id.0, tax_collector_entity);

        // Create a new object event
        commands.trigger(NewObj {
            entity: tax_collector_entity,
        });
    }

    pub fn spawn_spoil_crisis(
        npc_id: i32,
        player_id: i32,
        pos: Position,
        template: String,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
        target: i32,
    ) -> (Entity, Id, PlayerId, Position) {
        let npc_template = templates.obj_templates.get(template);

        let mut npc = Obj {
            id: Id(npc_id),
            player_id: PlayerId(player_id),
            position: pos,
            name: Name(npc_template.template.clone()),
            template: Template(npc_template.template.clone()),
            class: Class(npc_template.class.clone()),
            subclass: Subclass::from_str(&npc_template.subclass),
            state: State::None,
            misc: Misc {
                image: npc_template.image,
                hsl: Vec::new().into(),
                groups: Vec::new().into(),
            },
            stats: Stats {
                hp: npc_template.base_hp.unwrap(),
                base_hp: npc_template.base_hp.unwrap(),
                stamina: npc_template.base_stamina,
                mana: None,
                base_stamina: npc_template.base_stamina,
                base_mana: None,
                base_def: npc_template.base_def.unwrap(),
                base_damage: npc_template.base_dmg,
                damage_range: npc_template.dmg_range,
                base_speed: npc_template.base_speed,
                base_vision: npc_template.base_vision,
            },
            effects: Effects(HashMap::new()),
            inventory: Inventory {
                owner: npc_id,
                items: Vec::new(),
            },
            last_combat_tick: LastCombatTick::default(),
        };

        Encounter::generate_loot(npc_id, ids, &mut npc.inventory, templates);

        let spoil_target = Steps::build()
            .label("Spoil Target")
            .step(SetSpoilTarget)
            .step(NpcMoveToTarget)
            .step(SpoilTarget);

        let chase_and_attack = Steps::build()
            .label("Chase and Attack")
            .step(SetAttackTarget)
            .step(NpcMoveToTarget)
            .step(AttackTarget);
        let rat_blocked_wander = Steps::build()
            .label("Rat Blocked Wander")
            .step(RandomWander);
        let wolf_forest_hide = Steps::build()
            .label("Wolf Forest Hide")
            .step(MoveToForest)
            .step(Hide);

        let entity = commands
            .spawn((
                npc,
                Viewshed { range: 2 },
                SubclassNPC,
                VisibleTarget::new(target),
                TaskTarget::new(target),
                EventExecuting {
                    event_type: "".to_string(),
                    state: EventExecutingState::None,
                },
                Thinker::build()
                    .label("Spoil Settlement Crisis")
                    .picker(Highest)
                    .when(SpoilTargetScorer, spoil_target)
                    .when(VisibleTargetScorer, chase_and_attack)
                    .when(WolfBlockedHideScorer, wolf_forest_hide)
                    .when(RatBlockedWanderScorer, rat_blocked_wander), //.when(NoTargetScorer, Wander)
            ))
            .id();

        ids.new_obj(npc_id, player_id);
        entity_map.new_obj(npc_id, entity);

        return (entity, Id(npc_id), PlayerId(player_id), pos);
    }

    pub fn spawn_steal_crisis(
        npc_id: i32,
        player_id: i32,
        pos: Position,
        template: String,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
        target: i32,
    ) -> (Entity, Id, PlayerId, Position) {
        let npc_template = templates.obj_templates.get(template);

        let mut npc = Obj {
            id: Id(npc_id),
            player_id: PlayerId(player_id),
            position: pos,
            name: Name(npc_template.template.clone()),
            template: Template(npc_template.template.clone()),
            class: Class(npc_template.class.clone()),
            subclass: Subclass::from_str(&npc_template.subclass),
            state: State::None,
            misc: Misc {
                image: npc_template.image,
                hsl: Vec::new().into(),
                groups: Vec::new().into(),
            },
            stats: Stats {
                hp: npc_template.base_hp.unwrap(),
                base_hp: npc_template.base_hp.unwrap(),
                stamina: npc_template.base_stamina,
                mana: None,
                base_stamina: npc_template.base_stamina,
                base_mana: None,
                base_def: npc_template.base_def.unwrap(),
                base_damage: npc_template.base_dmg,
                damage_range: npc_template.dmg_range,
                base_speed: npc_template.base_speed,
                base_vision: npc_template.base_vision,
            },
            effects: Effects(HashMap::new()),
            inventory: Inventory {
                owner: npc_id,
                items: Vec::new(),
            },
            last_combat_tick: LastCombatTick::default(),
        };

        Encounter::generate_loot(npc_id, ids, &mut npc.inventory, templates);

        let steal_target = Steps::build()
            .label("Steal Target")
            .step(SetStealTarget)
            .step(NpcMoveToTarget)
            .step(StealTarget);

        let chase_and_attack = Steps::build()
            .label("Chase and Attack")
            .step(SetAttackTarget)
            .step(NpcMoveToTarget)
            .step(AttackTarget);

        let entity = commands
            .spawn((
                npc,
                Viewshed { range: 2 },
                SubclassNPC,
                VisibleTarget::new(target),
                TaskTarget::new(target),
                ItemsToSteal {
                    item_classes: vec![GOLD_COINS.to_string(), WEAPON.to_string()],
                },
                EventExecuting {
                    event_type: "".to_string(),
                    state: EventExecutingState::None,
                },
                Thinker::build()
                    .label("Steal Settlement Crisis")
                    .picker(Highest)
                    .when(StealTargetScorer, steal_target)
                    .when(VisibleTargetScorer, chase_and_attack), //.when(NoTargetScorer, Wander)
            ))
            .id();

        ids.new_obj(npc_id, player_id);
        entity_map.new_obj(npc_id, entity);

        return (entity, Id(npc_id), PlayerId(player_id), pos);
    }

    pub fn spawn_torch_crisis(
        npc_id: i32,
        player_id: i32,
        pos: Position,
        template: String,
        commands: &mut Commands,
        ids: &mut ResMut<Ids>,
        entity_map: &mut ResMut<EntityObjMap>,
        templates: &Res<Templates>,
        target: i32,
    ) -> (Entity, Id, PlayerId, Position) {
        let npc_template = templates.obj_templates.get(template);

        let mut npc = Obj {
            id: Id(npc_id),
            player_id: PlayerId(player_id),
            position: pos,
            name: Name(npc_template.template.clone()),
            template: Template(npc_template.template.clone()),
            class: Class(npc_template.class.clone()),
            subclass: Subclass::from_str(&npc_template.subclass),
            state: State::None,
            misc: Misc {
                image: npc_template.image,
                hsl: Vec::new().into(),
                groups: Vec::new().into(),
            },
            stats: Stats {
                hp: npc_template.base_hp.unwrap(),
                base_hp: npc_template.base_hp.unwrap(),
                stamina: npc_template.base_stamina,
                mana: None,
                base_stamina: npc_template.base_stamina,
                base_mana: None,
                base_def: npc_template.base_def.unwrap(),
                base_damage: npc_template.base_dmg,
                damage_range: npc_template.dmg_range,
                base_speed: npc_template.base_speed,
                base_vision: npc_template.base_vision,
            },
            effects: Effects(HashMap::new()),
            inventory: Inventory {
                owner: npc_id,
                items: Vec::new(),
            },
            last_combat_tick: LastCombatTick::default(),
        };

        Encounter::generate_loot(npc_id, ids, &mut npc.inventory, templates);

        let torch_target = Steps::build()
            .label("Torch Target")
            .step(SetTorchTarget)
            .step(NpcMoveToTarget)
            .step(TorchTarget);

        let chase_and_attack = Steps::build()
            .label("Chase and Attack")
            .step(SetAttackTarget)
            .step(NpcMoveToTarget)
            .step(AttackTarget);

        let entity = commands
            .spawn((
                npc,
                Viewshed { range: 2 },
                SubclassNPC,
                VisibleTarget::new(target),
                TaskTarget::new(target),
                EventExecuting {
                    event_type: "".to_string(),
                    state: EventExecutingState::None,
                },
                Thinker::build()
                    .label("Torch Settlement Crisis")
                    .picker(Highest)
                    .when(TorchTargetScorer, torch_target)
                    .when(VisibleTargetScorer, chase_and_attack), //.when(NoTargetScorer, Wander)
            ))
            .id();

        ids.new_obj(npc_id, player_id);
        entity_map.new_obj(npc_id, entity);

        return (entity, Id(npc_id), PlayerId(player_id), pos);
    }

    pub fn generate_loot(
        npc_id: i32,
        ids: &mut ResMut<Ids>,
        inventory: &mut Inventory,
        templates: &Res<Templates>,
    ) {
        let mut rng = rand::thread_rng();

        let loot_list = Self::loot_list();

        for loot in loot_list.iter() {
            let random_num = rng.gen::<f32>();

            if loot.drop_rate > random_num {
                let item_quantity = rng.gen_range(loot.min..loot.max);

                inventory.create(
                    ids.new_item_id(),
                    npc_id,
                    loot.item_name.clone(),
                    item_quantity,
                    &templates.item_templates,
                );
            }
        }
    }

    pub fn npc_list(tile_type: TileType) -> Vec<&'static str> {
        match tile_type {
            TileType::DeciduousForest => {
                return vec![
                    "Spider",
                    "Wose",
                    "Skeleton",
                    "Windstride Stag",
                    "Swiftstep Hare",
                    "Black Bear",
                    "Cave Bear",
                    "Saberfang Cat",
                ]
            }
            TileType::Rainforest
            | TileType::Jungle
            | TileType::PineForest
            | TileType::PalmForest => {
                return vec![
                    "Spider",
                    "Wose",
                    "Windstride Stag",
                    "Swiftstep Hare",
                    "Mountain Lion",
                    "Black Bear",
                    "Cave Bear",
                    "Saberfang Cat",
                ]
            }
            TileType::Grasslands
            | TileType::HillsGrasslands
            | TileType::Plains
            | TileType::HillsPlains
            | TileType::Savanna => {
                return vec![
                    "Wolf",
                    "Swiftstep Hare",
                    "Windstride Stag",
                    "Mountain Lion",
                    "Saberfang Cat",
                    "Terror Bird",
                ]
            }
            TileType::Snow => return vec!["Wolf", "Yeti", "Frostmane Elk", "Saberfang Cat"],
            TileType::HillsSnow => return vec!["Wolf", "Yeti", "Frostmane Elk", "Saberfang Cat"],
            TileType::FrozenForest => {
                return vec![
                    "Wose",
                    "Yeti",
                    "Spider",
                    "Frostmane Elk",
                    "Black Bear",
                    "Cave Bear",
                    "Saberfang Cat",
                ]
            }
            TileType::Mountain => {
                return vec![
                    "Wolf",
                    "Mountain Lion",
                    "Black Bear",
                    "Cave Bear",
                    "Saberfang Cat",
                ]
            }
            TileType::Desert => return vec!["Scorpion", "Giant Rat", "Skeleton"],
            TileType::HillsDesert => return vec!["Scorpion", "Giant Rat", "Skeleton"],
            //_ => return vec!["Giant Rat", "Wolf", "Skeleton"],
            _ => return vec!["Wolf"],
        }
    }

    fn loot_list() -> Vec<Loot> {
        let copper_dust = Loot {
            item_name: "Valleyrun Copper Dust".to_string(),
            drop_rate: 0.2,
            min: 1,
            max: 5,
        };

        let grape = Loot {
            item_name: "Amitanian Grape".to_string(),
            drop_rate: 0.5,
            min: 1,
            max: 3,
        };

        let training_axe = Loot {
            item_name: "Copper Training Axe".to_string(),
            drop_rate: 0.02,
            min: 1,
            max: 2,
        };

        let berries = Loot {
            item_name: "Honeybell Berries".to_string(),
            drop_rate: 0.99,
            min: 5,
            max: 10,
        };

        let mana = Loot {
            item_name: "Mana".to_string(),
            drop_rate: 0.75,
            min: 1,
            max: 3,
        };

        let coins = Loot {
            item_name: "Gold Coins".to_string(),
            drop_rate: 0.99,
            min: 1,
            max: 10,
        };

        let soulshard = Loot {
            item_name: "Soulshard".to_string(),
            drop_rate: 0.99,
            min: 1,
            max: 2,
        };

        return vec![
            copper_dust,
            grape,
            training_axe,
            berries,
            mana,
            coins,
            soulshard,
        ];
    }

    fn find_valid_pos(
        neighbours: Vec<(i32, i32)>,
        player_id: i32,
        all_obj_pos: &Vec<EncounterMapObj>,
        map: &Map,
    ) -> Option<Position> {
        let valid_neighbours: Vec<(i32, i32)> = neighbours
            .into_iter()
            .filter(|(x, y)| Self::is_valid_pos(*x, *y, player_id, all_obj_pos, map))
            .collect();

        if valid_neighbours.len() > 0 {
            let mut rng = rand::thread_rng();
            let index = rng.gen_range(0..valid_neighbours.len());
            debug!("Random valid pos index: {:?}", index);
            let (pos_x, pos_y) = valid_neighbours[index];

            return Some(Position { x: pos_x, y: pos_y });
        } else {
            return None;
        }
    }

    fn is_valid_pos(
        x: i32,
        y: i32,
        player_id: i32,
        all_obj_pos: &Vec<EncounterMapObj>,
        map: &Map,
    ) -> bool {
        let is_passable = Map::is_passable(x, y, &map);
        let is_valid_pos = Map::is_valid_pos((x, y));
        let is_not_blocked = Self::is_not_blocked(player_id, x, y, &all_obj_pos);
        debug!("is_not_blocked: {:?}", is_not_blocked);

        if is_passable && is_valid_pos && is_not_blocked {
            return true;
        }

        return false;
    }

    fn is_not_blocked(player_id: i32, x: i32, y: i32, all_obj_pos: &Vec<EncounterMapObj>) -> bool {
        debug!(
            "is_not_blocked: {:?} {:?} {:?} {:?}",
            player_id, x, y, all_obj_pos
        );
        // TODO reconsider if player id should be compared
        for obj in all_obj_pos.iter() {
            if x == obj.x && y == obj.y && player_id != obj.player_id {
                // found blocking obj
                return false;
            }
        }

        return true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescued_villager_needs_reach_hungry_and_thirsty_after_one_minute() {
        let mut thirst = Thirst::new(
            RESCUED_VILLAGER_STARTING_THIRST,
            RESCUED_VILLAGER_NEED_PER_TICK,
        );
        let mut hunger = Hunger::new(
            RESCUED_VILLAGER_STARTING_HUNGER,
            RESCUED_VILLAGER_NEED_PER_TICK,
        );

        for _ in 0..(60 * 10) {
            thirst.update_by_tick_amount(1.0);
            hunger.update_by_tick_amount(1.0);
        }

        assert_eq!(thirst.num_to_string(), THIRSTY);
        assert_eq!(hunger.num_to_string(), HUNGRY);
    }
}
