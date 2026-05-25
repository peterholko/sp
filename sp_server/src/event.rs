use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::ecs::reflect::ReflectMapEntities;
use bevy::prelude::*;

use uuid::Uuid;

use std::collections::HashMap;

use crate::constants::STATE_NONE;
use crate::effect::Effect;
use crate::obj::Position;

#[derive(Debug, Component)]
pub struct EventCompleted {
    pub event_id: uuid::Uuid,
    pub event_type: String,
    pub at_tick: i32,
    pub success: bool,
}

#[derive(Debug, Component)]
pub struct EventExecuting {
    pub event_type: String,
    pub state: EventExecutingState,
}

#[derive(Debug, Component, PartialEq)]
pub enum EventExecutingState {
    None,
    Executing,
    Completed,
    Failed,
}

impl EventExecutingState {
    pub fn is_finished(&self) -> bool {
        *self == EventExecutingState::Completed
            || *self == EventExecutingState::None
            || *self == EventExecutingState::Failed
    }

    pub fn is_failed(&self) -> bool {
        *self == EventExecutingState::Failed
    }
}

#[derive(Debug, Component)]
pub struct MoveEvent {
    pub event: MapEvent,
    pub is_dst_open: bool,
    pub objs_on_tile: Vec<(i32, i32, String)>,
    pub in_range_sanctuary: Option<(i32, Position)>,
    pub in_range_weak_sanctuary: Option<(i32, Position)>,
    pub is_dst_shelter: Option<i32>,
}

#[derive(Debug, Component)]
pub struct MoveEventPrecheck;

#[derive(Debug, Component)]
pub struct MoveEventUpdate;

#[derive(Debug, Component)]
pub struct MoveEventCompleted;

#[derive(Debug, Component)]
pub struct DrinkEventCompleted {
    pub at_tick: i32,
}

#[derive(Debug, Component)]
pub struct FindEventCompleted {
    pub event: String,
}

#[derive(Debug, Component)]
pub struct EatEventCompleted {
    pub at_tick: i32,
}

#[derive(Debug, Component)]
pub struct SleepEventCompleted {
    pub at_tick: i32,
}

#[derive(Clone, Reflect, Debug)]
pub enum VisibleEvent {
    NewObjEvent,
    RemoveObjEvent {
        pos: Position,
    },
    UpdateObjEvent {
        attrs: Vec<(String, String)>,
    },
    UpdateObjPosEvent {
        src: Position,
        dst: Position,
    },
    UpdateObjVisionEvent {
        range: u32,
    },
    StateChangeEvent {
        new_state: String,
    },
    MoveEvent {
        src: Position,
        dst: Position,
    },
    HideEvent,
    EmbarkEvent {
        transport_id: i32,
    },
    Disembark {
        pos: Position,
    },
    CooldownEvent {
        duration: i32,
    },
    DamageEvent {
        target_id: i32,
        target_pos: Position,
        attack_type: String,
        damage: i32,
        combo: Option<String>,
        state: String,
        missed: bool,
    },
    StealEvent {
        target_id: i32,
        target_pos: Position,
        item_types: Vec<String>,
    },
    BroadcastStealEvent {
        target_id: i32,
        target_pos: Position,
    },
    SpoilEvent {
        target_id: i32,
        target_pos: Position,
        item_type: String,
    },
    BroadcastSpoilEvent {
        target_id: i32,
        target_pos: Position,
        item_type: String,
        item_quantity: i32,
    },
    TorchEvent {
        target_id: i32,
        target_pos: Position,
    },
    BroadcastTorchEvent {
        target_id: i32,
        target_pos: Position,
    },
    EffectExpiredEvent {
        effect: Effect,
    },
    SoundEvent {
        pos: Position,
        sound: String,
        intensity: i32,
    },
    SpeechEvent {
        speech: String,
        intensity: i32,
    },
    ActivateEvent {
        structure_id: i32,
    },
    GatherEvent {
        res_type: String,
    },
    OperateEvent {
        structure_id: i32,
    },
    RefineEvent {
        structure_id: i32,
    },
    CraftEvent {
        structure_id: i32,
        recipe_name: String,
    },
    ExperimentEvent {
        structure_id: i32,
    },
    SurveyEvent,
    ProspectEvent,
    ExploreEvent,
    InvestigateEvent {
        target_id: i32,
    },
    PlantEvent {
        structure_id: i32,
    },
    TendEvent {
        structure_id: i32,
    },
    HarvestEvent {
        structure_id: i32,
    },
    RepairEvent {
        structure_id: i32,
    },
    UseItemEvent {
        item_id: i32,
        item_owner_id: i32,
    },
    FindDrinkEvent {
        // Added Find Events to create a more realistic AI that waits to search again
        obj_id: i32,
    },
    DrinkEvent {
        item_id: i32,
        obj_id: i32,
    },
    FindFoodEvent {
        obj_id: i32,
    },
    EatEvent {
        item_id: i32,
        obj_id: i32,
    },
    FindShelterEvent {
        obj_id: i32,
    },
    SleepEvent {
        obj_id: i32,
    },
    FishingEvent {
        obj_id: i32,
    },
    SpellRaiseDeadEvent {
        corpse_id: i32,
    },
    SpellDamageEvent {
        spell: Spell,
        target_id: i32,
    },
    NoEvent,
}

#[derive(Clone, Reflect, Debug)]
pub struct MapEvent {
    pub event_id: Uuid,
    pub obj_id: i32,
    pub run_tick: i32,
    pub event_type: VisibleEvent,
}

#[derive(Resource, Reflect, Default, Deref, DerefMut, Debug)]
#[reflect(Resource)]
pub struct MapEvents(pub HashMap<Uuid, MapEvent>);

impl MapEvents {
    pub fn new(&mut self, obj_id: i32, game_tick: i32, map_event_type: VisibleEvent) -> MapEvent {
        let map_event_id = Uuid::new_v4();

        let map_state_event = MapEvent {
            event_id: map_event_id,
            obj_id: obj_id,
            run_tick: game_tick,
            event_type: map_event_type,
        };

        self.insert(map_event_id, map_state_event.clone());

        return map_state_event;
    }

    pub fn update_state(&mut self, obj_id: i32, game_tick: i32, state: VisibleEvent) {
        self.new(obj_id, game_tick + 1, state);
    }

    pub fn get_event(&self, event_id: Uuid) -> Option<&MapEvent> {
        self.get(&event_id)
    }

    pub fn remove_event(&mut self, event_id: Uuid) {
        self.remove(&event_id);
    }
}

#[derive(Debug, Resource, Reflect, Deref, DerefMut)]
pub struct VisibleEvents(pub Vec<MapEvent>);

impl VisibleEvents {
    pub fn new(&mut self, obj_id: i32, game_tick: i32, event_type: VisibleEvent) {
        let event_id = Uuid::new_v4();

        let visible_event = MapEvent {
            event_id: event_id,
            obj_id: obj_id,
            run_tick: game_tick,
            event_type: event_type,
        };

        self.push(visible_event.clone());
    }
}

#[derive(Resource, Component, Reflect, Default, Deref, DerefMut, Debug)]
#[reflect(Resource)]
pub struct GameEvents(pub HashMap<i32, GameEvent>);

impl MapEntities for GameEvents {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        for (_index, game_event) in self.iter_mut() {
            match game_event.event_type {
                GameEventType::RemoveEntity { entity } => {
                    game_event.event_type = GameEventType::RemoveEntity {
                        entity: entity_mapper.get_mapped(entity),
                    };
                }
                _ => {}
            }
        }
    }
}

impl GameEvents {
    pub fn get_craft_event(&self, crafter_id: i32) -> Option<GameCraftEvent> {
        for (_, game_event) in self.iter() {
            if let GameEventType::CraftEvent {
                crafter_id: event_crafter_id,
                recipe_name,
            } = &game_event.event_type
            {
                if *event_crafter_id == crafter_id {
                    return Some(GameCraftEvent {
                        event_id: game_event.event_id,
                        start_tick: game_event.start_tick,
                        run_tick: game_event.run_tick,
                        crafter_id: *event_crafter_id,
                        structure_id: None,
                        recipe_name: recipe_name.clone(),
                    });
                }
            }
        }
        None
    }

    pub fn get_structure_craft_event(&self, crafter_id: i32) -> Option<GameCraftEvent> {
        for (_, game_event) in self.iter() {
            if let GameEventType::StructureCraftEvent {
                crafter_id: event_crafter_id,
                structure_id,
                recipe_name,
            } = &game_event.event_type
            {
                if *event_crafter_id == crafter_id {
                    return Some(GameCraftEvent {
                        event_id: game_event.event_id,
                        start_tick: game_event.start_tick,
                        run_tick: game_event.run_tick,
                        crafter_id: *event_crafter_id,
                        structure_id: Some(*structure_id),
                        recipe_name: recipe_name.clone(),
                    });
                }
            }
        }
        None
    }

    pub fn get_refine_event(&self, refiner_id: i32) -> Option<GameRefineEvent> {
        for (_, game_event) in self.iter() {
            if let GameEventType::RefineEvent {
                refiner_id: event_refiner_id,
                item_id,
            } = &game_event.event_type
            {
                if *event_refiner_id == refiner_id {
                    return Some(GameRefineEvent {
                        event_id: game_event.event_id,
                        start_tick: game_event.start_tick,
                        run_tick: game_event.run_tick,
                        refiner_id: *event_refiner_id,
                        structure_id: None,
                        item_id: *item_id,
                    });
                }
            }
        }
        None
    }

    pub fn get_structure_refine_event(&self, refiner_id: i32) -> Option<GameRefineEvent> {
        for (_, game_event) in self.iter() {
            if let GameEventType::StructureRefineEvent {
                refiner_id: event_refiner_id,
                structure_id,
                item_id,
            } = &game_event.event_type
            {
                if *event_refiner_id == refiner_id {
                    return Some(GameRefineEvent {
                        event_id: game_event.event_id,
                        start_tick: game_event.start_tick,
                        run_tick: game_event.run_tick,
                        refiner_id: *event_refiner_id,
                        structure_id: Some(*structure_id),
                        item_id: *item_id,
                    });
                }
            }
        }
        None
    }

    pub fn get_structure_operate_event(&self, operator_id: i32) -> Option<GameOperateEvent> {
        for (_, game_event) in self.iter() {
            if let GameEventType::StructureOperateEvent {
                operator_id: event_operator_id,
                structure_id,
            } = &game_event.event_type
            {
                if *event_operator_id == operator_id {
                    return Some(GameOperateEvent {
                        event_id: game_event.event_id,
                        start_tick: game_event.start_tick,
                        run_tick: game_event.run_tick,
                        operator_id: *event_operator_id,
                        structure_id: Some(*structure_id),
                    });
                }
            }
        }
        None
    }
}

#[derive(Clone, Reflect, Debug)]
pub struct GameEvent {
    pub event_id: i32,
    pub start_tick: i32,
    pub run_tick: i32,
    pub event_type: GameEventType,
}

#[derive(Clone, Reflect, Debug)]

pub enum GameEventType {
    Login {
        player_id: i32,
    },
    PlayerNotice {
        player_id: i32,
        message: String,
        expiry: Option<i32>,
    },
    MerchantArrival {
        merchant_id: i32,
        player_id: i32,
    },
    MerchantLeavingSoon {
        merchant_id: i32,
        player_id: i32,
    },
    MerchantDeparture {
        merchant_id: i32,
        player_id: i32,
    },
    SpawnNPC {
        npc_type: String,
        pos: Position,
        npc_id: Option<i32>,
    },
    ForageEvent {
        forager_id: i32,
    },
    GatherEvent {
        gatherer_id: i32,
        res_type: String,
    },
    StructureGatherEvent {
        operator_id: i32,
        structure_id: i32,
    },
    RefineEvent {
        refiner_id: i32,
        item_id: i32,
    },
    CraftEvent {
        crafter_id: i32,
        recipe_name: String,
    },
    StructureRefineEvent {
        refiner_id: i32,
        structure_id: i32,
        item_id: i32,
    },
    StructureCraftEvent {
        crafter_id: i32,
        structure_id: i32,
        recipe_name: String,
    },
    StructureOperateEvent {
        operator_id: i32,
        structure_id: i32,
    },
    ExperimentEvent {
        experimenter_id: i32,
        structure_id: i32,
    },
    AddEffectOnTile {
        effect: Effect,
        player_id: i32,
        pos: Position,
    },
    RemoveEffectOnTile {
        effect: Effect,
        player_id: i32,
        pos: Position,
    },
    UpdatePos {
        obj_id: i32,
        pos: Position,
    },
    NecroEvent {
        necromancer_id: Option<i32>,
        spawn_anchor: Position,
        corpse_anchor: Position,
        home: Position,
    },
    SpawnVillager {
        pos: Position,
        player_id: i32,
    },
    RemoveEntity {
        entity: Entity,
    },
    CancelRefineEvent {
        obj_id: i32,
    },
    CancelAllMapEvents {
        obj_id: i32,
    },
    CancelAllowedMapEvents {
        obj_id: i32,
    },
    CancelMapEventsById {
        event_ids: Vec<uuid::Uuid>,
    },
}

#[derive(Debug, Clone)]
pub struct GameCraftEvent {
    pub event_id: i32,
    pub start_tick: i32,
    pub run_tick: i32,
    pub crafter_id: i32,
    pub structure_id: Option<i32>,
    pub recipe_name: String,
}

#[derive(Debug, Clone)]
pub struct GameRefineEvent {
    pub event_id: i32,
    pub start_tick: i32,
    pub run_tick: i32,
    pub refiner_id: i32,
    pub structure_id: Option<i32>,
    pub item_id: i32,
}

#[derive(Debug, Clone)]
pub struct GameOperateEvent {
    pub event_id: i32,
    pub start_tick: i32,
    pub run_tick: i32,
    pub operator_id: i32,
    pub structure_id: Option<i32>,
}

#[derive(Clone, Reflect, Debug)]
pub enum Spell {
    ShadowBolt,
    ArcaneBolt,
}

#[derive(Clone, Reflect, Debug)]
pub enum EmbarkAction {
    Embark,
    Disembark,
}
