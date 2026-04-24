use bevy::prelude::*;
use big_brain::prelude::*;

use crate::combat::CombatQuery;
use crate::common::{Destination, Idle, MoveTo, Transport};
use crate::effect::Effect;
use crate::event::{MapEvents, VisibleEvent};
use crate::game::{EventInProgress, GameTick};
use crate::ids::{EntityObjMap, Ids};
use crate::item;
use crate::item::*;
use crate::map::Map;
use crate::obj::{State, *};
use crate::templates::Templates;
use crate::{constants::*, AppState};

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct MerchantScorer;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SailToPort;

#[derive(Debug, Reflect, Component, Default)]
#[reflect(Component)]
pub struct TaxCollector {
    pub target_player: i32,
    pub collection_amount: i32,
    pub debt_amount: i32,
    pub last_collection_time: i32,
    pub landing_pos: Position,
    pub transport_id: i32,
    pub last_demand_time: i32,
}

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct IsAboard;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct IsTaxCollected;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct AtLanding;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct OverdueTaxScorer;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct NoTaxesToCollect;

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct TaxesToCollect;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetDestination;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Talk {
    pub speech: String,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct MoveToTarget {
    pub target: i32,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct MoveToPos;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct MoveToEmpire;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Forfeiture;

#[derive(Debug, Reflect, Component, Default)]
#[reflect(Component)]
pub struct TaxCollectorTransport {
    pub tax_collector_id: i32,
}

#[derive(Debug, Reflect, Component, Default)]
#[reflect(Component)]
pub struct Merchant {
    pub home_port: Position,
    pub target_port: Position,
    pub dest: Position,
    pub in_port_at: i32,
    pub hauling: Vec<i32>,
}

pub struct TaxCollectorPlugin;

impl Plugin for TaxCollectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_tax_collection_system.run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (
                merchant_scorer_system.in_set(BigBrainSet::Scorers),
                idle_action_system.in_set(BigBrainSet::Actions),
                move_to_target_action_system.in_set(BigBrainSet::Actions),
                move_to_pos_action_system.in_set(BigBrainSet::Actions),
                move_to_empire_action_system.in_set(BigBrainSet::Actions),
                forfeiture_action_system.in_set(BigBrainSet::Actions),
                set_destination_action_system.in_set(BigBrainSet::Actions),
                talk_action_system.in_set(BigBrainSet::Actions),
                is_aboard_scorer_system.in_set(BigBrainSet::Scorers),
                at_landing_scorer_system.in_set(BigBrainSet::Scorers),
                is_tax_collected_scorer_system.in_set(BigBrainSet::Scorers),
                no_taxes_to_collect_scorer_system.in_set(BigBrainSet::Scorers),
                taxes_to_collect_scorer_system.in_set(BigBrainSet::Scorers),
                overdue_tax_scorer_system.in_set(BigBrainSet::Scorers),
            )
                .run_if(in_state(AppState::Running)),
        );
    }
}

pub fn merchant_scorer_system(
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<MerchantScorer>>,
) {
    for (Actor(_actor), mut score, _span) in &mut query {
        score.set(1.0);
    }
}

// General system to start a tax collection event
pub fn update_tax_collection_system(
    game_tick: ResMut<GameTick>,
    mut collector_query: Query<&mut TaxCollector>,
) {
    for mut collector in collector_query.iter_mut() {
        let next_tax_collection = collector.last_collection_time + 1000;
        if next_tax_collection <= game_tick.0 {
            info!("Tax collection time for {:?}", collector.target_player);
            collector.collection_amount = 50;
            collector.last_collection_time = game_tick.0;
        }
    }
}

pub fn is_aboard_scorer_system(
    state_query: Query<&State, With<TaxCollector>>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<IsAboard>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if let Ok(state) = state_query.get(*actor) {
            if *state == State::Aboard {
                score.set(0.8);
            } else {
                score.set(0.0);
            }
        }
    }
}

pub fn is_tax_collected_scorer_system(
    tax_collector_query: Query<(&Id, &TaxCollector, &Inventory)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<IsTaxCollected>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if let Ok((id, tax_collector, inventory)) = tax_collector_query.get(*actor) {
            if let Some(gold) = inventory.get_by_class(GOLD.to_string()) {
                if gold.quantity >= tax_collector.collection_amount {
                    score.set(1.0);
                } else {
                    score.set(0.0);
                }
            } else {
                score.set(0.0);
            }
        }
    }
}

pub fn at_landing_scorer_system(
    collector_query: Query<(&Position, &TaxCollector)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<AtLanding>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if let Ok((pos, tax_collector)) = collector_query.get(*actor) {
            if Map::is_adjacent_including_source(*pos, tax_collector.landing_pos) {
                score.set(0.9);
            } else {
                score.set(0.0);
            }
        }
    }
}

pub fn no_taxes_to_collect_scorer_system(
    entity_map: Res<EntityObjMap>,
    transport_query: Query<(&Transport, &TaxCollectorTransport)>,
    collector_query: Query<(&TaxCollector, &Inventory)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<NoTaxesToCollect>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        let Ok((transport, tc_transport)) = transport_query.get(*actor) else {
            continue;
        };

        // Get entity from passenger id
        let Some(collector_entity) = entity_map.get_entity(tc_transport.tax_collector_id) else {
            error!("Cannot find entity for {:?}", tc_transport.tax_collector_id);
            continue;
        };

        if let Ok((collector, inventory)) = collector_query.get(collector_entity) {
            if let Some(gold) = inventory.get_by_class(GOLD.to_string()) {
                if gold.quantity >= collector.collection_amount
                    && transport.hauling.contains(&tc_transport.tax_collector_id)
                {
                    score.set(1.0);
                } else {
                    score.set(0.0);
                }
            } else {
                score.set(0.0);
            }
        }
    }
}

pub fn taxes_to_collect_scorer_system(
    entity_map: Res<EntityObjMap>,
    transport_query: Query<&TaxCollectorTransport>,
    collector_query: Query<(&TaxCollector, &Inventory)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<TaxesToCollect>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        let Ok(tc_transport) = transport_query.get(*actor) else {
            continue;
        };

        // Get entity from passenger id
        let Some(collector_entity) = entity_map.get_entity(tc_transport.tax_collector_id) else {
            error!("Cannot find entity for {:?}", tc_transport.tax_collector_id);
            continue;
        };

        if let Ok((collector, inventory)) = collector_query.get(collector_entity) {
            if let Some(gold) = inventory.get_by_class(GOLD.to_string()) {
                if gold.quantity < collector.collection_amount {
                    score.set(1.0);
                } else {
                    score.set(0.0);
                }
            } else {
                score.set(0.0);
            }
        } else {
            score.set(0.0);
        }
    }
}

pub fn overdue_tax_scorer_system(
    game_tick: Res<GameTick>,
    collector_query: Query<(&Id, &TaxCollector, &Inventory)>,
    mut query: Query<(&Actor, &mut Score, &ScorerSpan), With<OverdueTaxScorer>>,
) {
    for (Actor(actor), mut score, _span) in &mut query {
        if let Ok((_id, collector, inventory)) = collector_query.get(*actor) {
            if collector.last_collection_time + 850 < game_tick.0 {
                if let Some(gold) = inventory.get_by_class(GOLD.to_string()) {
                    if gold.quantity < collector.collection_amount {
                        score.set(1.0);
                    } else {
                        score.set(0.0);
                    }
                }
            }
        }
    }
}

pub fn idle_action_system(
    game_tick: Res<GameTick>,
    mut query: Query<(&Actor, &mut ActionState, &mut Idle, &ActionSpan)>,
) {
    for (Actor(actor), mut state, mut idle, _span) in &mut query {
        match *state {
            ActionState::Requested => {
                info!("Idle action requested by {:?}", actor);
                idle.start_time = game_tick.0;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if game_tick.0 - idle.start_time > idle.duration {
                    info!("Idle action completed for {:?}", actor);
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn move_to_target_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    mut tax_collector_query: Query<(&PlayerId, &mut TaxCollector), Without<EventInProgress>>,
    mut obj_query: Query<ObjStatQuery>,
    mut query: Query<(&Actor, &mut ActionState, &MoveToTarget)>,
) {
    for (Actor(actor), mut state, move_to_target) in &mut query {
        match *state {
            ActionState::Requested => {
                info!("MoveToTarget action requested");
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((tax_collector_player_id, mut tax_collector)) =
                    tax_collector_query.get_mut(*actor)
                else {
                    continue;
                };

                let Some(target_entity) = entity_map.get_entity(move_to_target.target) else {
                    error!("Cannot find entity for {:?}", move_to_target.target);
                    *state = ActionState::Failure;
                    continue;
                };

                // Have to get the list of collision positions before querying the npc and target
                let collision_list =
                    Obj::blocking_list_objstatquery(tax_collector_player_id.0, &obj_query);

                let entities = [*actor, target_entity];

                let Ok([mut npc, target]) = obj_query.get_many_mut(entities) else {
                    error!("Query failed to find entities {:?}", entities);
                    *state = ActionState::Failure;
                    continue;
                };

                // TODO is it possible to stun a transported object?
                /*if obj.effects.0.contains_key(&Effect::Stunned) {
                    debug!("NPC is stunned");
                    continue;
                }*/

                // Get NPC speed
                let mut npc_speed = 1;

                if let Some(npc_base_speed) = npc.stats.base_speed {
                    npc_speed = npc_base_speed;
                }

                let effect_speed_mod = npc.effects.get_speed_effects(&templates);

                let move_duration = (BASE_MOVE_TICKS
                    * (BASE_SPEED / npc_speed as f32)
                    * (1.0 / effect_speed_mod)) as i32;

                let reached_destination;

                if npc.player_id.0 == target.player_id.0 {
                    reached_destination = npc.pos == target.pos;
                } else {
                    reached_destination = Map::is_adjacent_including_source(*npc.pos, *target.pos);
                }

                if reached_destination {
                    if npc.player_id.0 != target.player_id.0 {
                        if tax_collector.last_demand_time + 100 < game_tick.0 {
                            tax_collector.last_demand_time = game_tick.0;

                            let speech_event = VisibleEvent::SpeechEvent {
                                speech: "Tax Time! Pay now or face asset forfeiture!".to_string(),
                                intensity: 2,
                            };

                            map_events.new(npc.id.0, game_tick.0 + 4, speech_event);
                        }
                    }

                    info!("MoveToTarget action success");
                    *state = ActionState::Success;
                } else {
                    info!("Moving to target... {:?}", collision_list);
                    if *npc.state == State::None || *npc.state == State::Aboard {
                        if let Some(path_result) = Map::find_fast_path(
                            *npc.pos,
                            *target.pos,
                            &map,
                            tax_collector_player_id.0,
                            collision_list,
                            true,
                            false,
                            false,
                            true, // Allow move onto position with transport
                        ) {
                            info!("Follower path: {:?}", path_result);

                            let (path, _c) = path_result;
                            let next_pos = &path[1];

                            info!("Next pos: {:?}", next_pos);

                            // Add State Change Event to Moving

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
                            info!("No path found");
                            *state = ActionState::Failure;
                        }
                    } else {
                        info!(
                            "Tax collector can only move in None or Aboard state, {:?}",
                            *npc.state
                        );
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
}

pub fn set_destination_action_system(
    mut dest_query: Query<&mut Destination>,
    mut transport: Query<&mut Transport>,
    mut query: Query<(&Actor, &mut ActionState, &mut SetDestination, &ActionSpan)>,
) {
    for (Actor(actor), mut state, mut _set_destination, _span) in &mut query {
        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                info!("Executing SetDestination action");
                if let Ok(mut dest) = dest_query.get_mut(*actor) {
                    // Get transport entity
                    if let Ok(mut transport) = transport.get_mut(*actor) {
                        dest.pos = transport.route[transport.next_stop as usize];

                        if transport.next_stop + 1 == transport.route.len() as i32 {
                            transport.next_stop -= 1;
                        } else {
                            transport.next_stop += 1;
                        }
                    }

                    *state = ActionState::Success;
                } else {
                    *state = ActionState::Failure;
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn move_to_pos_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    mut npc_query: Query<CombatQuery>,
    dest_query: Query<&Destination>,
    mut query: Query<(&Actor, &mut ActionState, &MoveToPos)>,
) {
    for (Actor(actor), mut state, _move_to_pos) in &mut query {
        match *state {
            ActionState::Requested => {
                info!("MoveToPos action requested");
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok(mut npc) = npc_query.get_mut(*actor) else {
                    error!("Query failed to find entity {:?}", *actor);
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok(dest) = dest_query.get(*actor) else {
                    error!("Query failed to find destination {:?}", *actor);
                    *state = ActionState::Failure;
                    continue;
                };

                if npc.effects.has(Effect::Stunned) {
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

                if *npc.pos == dest.pos {
                    // Arrived at position
                    info!("MoveToPos action success");
                    *state = ActionState::Success;
                } else {
                    if *npc.state == State::None {
                        if let Some(path_result) = Map::find_fast_path(
                            *npc.pos,
                            dest.pos,
                            &map,
                            npc.player_id.0,
                            Vec::new(),
                            false,
                            true, //TODO look up the terrain-walks for the npc
                            false,
                            false,
                        ) {
                            info!("Follower path: {:?}", path_result);

                            let (path, _c) = path_result;
                            let next_pos = &path[1];

                            info!("Next pos: {:?}", next_pos);

                            // Add State Change Event to Moving
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
                            error!("Tax Collector cannot find any available path");
                            *state = ActionState::Failure;
                        }
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
}

pub fn move_to_empire_action_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    map: Res<Map>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    mut npc_query: Query<CombatQuery>,
    mut query: Query<(&Actor, &mut ActionState, &MoveToEmpire)>,
) {
    for (Actor(actor), mut state, _move_to_empire) in &mut query {
        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok(mut npc) = npc_query.get_mut(*actor) else {
                    error!("Query failed to find entity {:?}", *actor);
                    *state = ActionState::Failure;
                    continue;
                };

                if npc.effects.has(Effect::Stunned) {
                    debug!("NPC is stunned");
                    continue;
                }

                // TODO replace this
                let empire_pos = Position { x: 1, y: 37 };

                // Get NPC speed
                let mut npc_speed = 1;

                if let Some(npc_base_speed) = npc.stats.base_speed {
                    npc_speed = npc_base_speed;
                }

                let effect_speed_mod = npc.effects.get_speed_effects(&templates);

                let move_duration = (BASE_MOVE_TICKS
                    * (BASE_SPEED / npc_speed as f32)
                    * (1.0 / effect_speed_mod)) as i32;

                if *npc.pos == empire_pos {
                    *state = ActionState::Success;
                } else {
                    if *npc.state == State::None {
                        if let Some(path_result) = Map::find_fast_path(
                            *npc.pos,
                            empire_pos,
                            &map,
                            npc.player_id.0,
                            Vec::new(),
                            false,
                            true,
                            false,
                            false,
                        ) {
                            info!("Follower path: {:?}", path_result);

                            let (path, _c) = path_result;
                            let next_pos = &path[1];

                            info!("Next pos: {:?}", next_pos);

                            // Add State Change Event to Moving
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
                            // No path found
                            *state = ActionState::Failure;
                        }
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
}

pub fn forfeiture_action_system(
    game_tick: Res<GameTick>,
    entity_map: Res<EntityObjMap>,
    mut ids: ResMut<Ids>,
    mut map_events: ResMut<MapEvents>,
    templates: Res<Templates>,
    mut collector_query: Query<(&Id, &mut TaxCollector)>,
    mut inventory_query: Query<&mut Inventory>,
    mut query: Query<(&Actor, &mut ActionState, &Forfeiture, &ActionSpan)>,
) {
    for (Actor(actor), mut state, _forfeiture, _span) in &mut query {
        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((collector_id, mut collector)) = collector_query.get_mut(*actor) else {
                    continue;
                };

                // Get hero id from player
                let Some(hero_id) = ids.get_hero(collector.target_player) else {
                    error!("Cannot find hero for player {:?}", collector.target_player);
                    *state = ActionState::Failure;
                    continue;
                };

                let Some(hero_entity) = entity_map.get_entity(hero_id) else {
                    error!("Cannot find hero entity for player {:?}", hero_id);
                    *state = ActionState::Failure;
                    continue;
                };

                let Ok([mut hero_inventory, mut collector_inventory]) =
                    inventory_query.get_many_mut([hero_entity, *actor])
                else {
                    error!("Cannot find hero or collector inventory");
                    *state = ActionState::Failure;
                    continue;
                };

                // Get hero items
                let total_gold = hero_inventory.get_total_gold();
                info!(
                    "Total gold: {:?} collection_amount {:?}",
                    total_gold, collector.collection_amount
                );

                let overdue_amount = (collector.collection_amount as f32 * 1.2) as i32;

                if total_gold >= overdue_amount {
                    let mut next_id = ids.new_item_id();
                    Inventory::transfer_gold(
                        &mut hero_inventory,
                        &mut collector_inventory,
                        overdue_amount,
                        &mut next_id,
                        &templates.item_templates,
                    );

                    collector.collection_amount = 0;
                    collector.last_collection_time = game_tick.0;

                    let speech_event = VisibleEvent::SpeechEvent {
                        speech: "Times up! I will take what you owe and 20% extra.".to_string(),
                        intensity: 2,
                    };

                    map_events.new(collector_id.0, game_tick.0 + 4, speech_event);
                } else {
                    let remainder_gold = collector.collection_amount - total_gold;

                    collector.debt_amount = (remainder_gold as f32 * 1.5) as i32;
                    collector.collection_amount = 0;
                    collector.last_collection_time = game_tick.0;

                    let speech_event = VisibleEvent::SpeechEvent {
                        speech: format!(
                            "No gold? Poor rabble, your debt is now {}!",
                            collector.debt_amount
                        ),
                        intensity: 2,
                    };

                    map_events.new(collector_id.0, game_tick.0 + 4, speech_event);
                }

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn talk_action_system(
    game_tick: Res<GameTick>,
    id_query: Query<&Id>,
    mut map_events: ResMut<MapEvents>,
    mut query: Query<(&Actor, &mut ActionState, &mut Talk, &ActionSpan)>,
) {
    for (Actor(actor), mut state, talk, _span) in &mut query {
        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok(id) = id_query.get(*actor) else {
                    error!("Query failed to find entity {:?}", *actor);
                    *state = ActionState::Failure;
                    continue;
                };

                let speech_event = VisibleEvent::SpeechEvent {
                    speech: talk.speech.clone(),
                    intensity: 2,
                };

                map_events.new(id.0, game_tick.0 + 4, speech_event);

                *state = ActionState::Success;
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            _ => {}
        }
    }
}
