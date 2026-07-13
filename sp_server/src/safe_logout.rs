//! Checkpoint 1 foundation for explicit safe logout.
//!
//! This module deliberately owns no production protocol surface and applies no
//! simulation protection. Its messages can currently be written only by
//! in-process server code and the headless harness.

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::constants::TICKS_PER_SEC;
use crate::game::{
    BoundMonolith, Clients, CrisisAssaultUnit, CrisisPhase, GameTick, Monolith, SanctuaryZones,
    SettlementCrisisState,
};
use crate::ids::{EntityObjMap, Ids};
use crate::map::Map;
use crate::npc::{self, VisibleTarget};
use crate::obj::{
    Id, LastCombatTick, LastDamageTick, PlayerId, Position, State, StateDead, Stats, Subclass,
    SubclassHero, SubclassNPC, Template, TrueDeath,
};
use crate::player_setup::AssignedStartLocations;
use crate::templates::Templates;
use crate::AppState;

pub const SAFE_LOGOUT_COUNTDOWN_TICKS: i32 = TICKS_PER_SEC * 10;
pub const SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS: i32 = TICKS_PER_SEC * 15;
pub const SAFE_LOGOUT_HOSTILE_RADIUS: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerWorldPresence {
    Online,
    SafeLogoutPending,
    OfflineProtected,
    Disconnected,
}

impl PlayerWorldPresence {
    fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::SafeLogoutPending => "safe_logout_pending",
            Self::OfflineProtected => "offline_protected",
            Self::Disconnected => "disconnected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeLogoutCancelReason {
    Moved,
    EnteredCombat,
    TookDamage,
    HostileNearby,
    LeftSanctuary,
    SanctuaryInvalid,
    AssaultStarted,
    HeroDied,
    Disconnected,
    Manual,
    RunEnded,
}

impl SafeLogoutCancelReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Moved => "moved",
            Self::EnteredCombat => "entered_combat",
            Self::TookDamage => "took_damage",
            Self::HostileNearby => "hostile_nearby",
            Self::LeftSanctuary => "left_sanctuary",
            Self::SanctuaryInvalid => "sanctuary_invalid",
            Self::AssaultStarted => "assault_started",
            Self::HeroDied => "hero_died",
            Self::Disconnected => "disconnected",
            Self::Manual => "manual",
            Self::RunEnded => "run_ended",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeLogoutRejectionReason {
    NotOnline,
    InvalidRun,
    MissingHero,
    HeroDied,
    TrueDeath,
    MissingBoundMonolith,
    MissingSanctuaryZone,
    SanctuaryInvalid,
    OutsideOwnSanctuary,
    AssaultActive,
    RecentCombat,
    RecentDamage,
    HostileNearby,
    AlreadyPending,
    AlreadyProtected,
}

impl SafeLogoutRejectionReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotOnline => "not_online",
            Self::InvalidRun => "invalid_run",
            Self::MissingHero => "missing_hero",
            Self::HeroDied => "hero_died",
            Self::TrueDeath => "true_death",
            Self::MissingBoundMonolith => "missing_bound_monolith",
            Self::MissingSanctuaryZone => "missing_sanctuary_zone",
            Self::SanctuaryInvalid => "sanctuary_invalid",
            Self::OutsideOwnSanctuary => "outside_own_sanctuary",
            Self::AssaultActive => "assault_active",
            Self::RecentCombat => "recent_combat",
            Self::RecentDamage => "recent_damage",
            Self::HostileNearby => "hostile_nearby",
            Self::AlreadyPending => "already_pending",
            Self::AlreadyProtected => "already_protected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerPresenceRecord {
    pub state: PlayerWorldPresence,
    pub safe_logout_requested_tick: Option<i32>,
    pub safe_logout_start_position: Option<Position>,
    /// Successful player-commanded combat from any owned source. The hero's
    /// `LastCombatTick` remains authoritative for entity combat; this aggregate
    /// closes the gap for commands issued through another owned combatant.
    pub last_combat_tick: Option<i32>,
    pub last_damage_tick: Option<i32>,
    pub cancel_reason: Option<SafeLogoutCancelReason>,
    pub rejection_reason: Option<SafeLogoutRejectionReason>,
    pub(crate) last_observed_hp: Option<i32>,
    pub(crate) client_connected: bool,
}

impl PlayerPresenceRecord {
    pub fn new(connected: bool) -> Self {
        Self {
            state: if connected {
                PlayerWorldPresence::Online
            } else {
                PlayerWorldPresence::Disconnected
            },
            safe_logout_requested_tick: None,
            safe_logout_start_position: None,
            last_combat_tick: None,
            last_damage_tick: None,
            cancel_reason: None,
            rejection_reason: None,
            last_observed_hp: None,
            client_connected: connected,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct PlayerWorldPresenceState {
    pub players: HashMap<i32, PlayerPresenceRecord>,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestSafeLogout {
    pub player_id: i32,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelSafeLogout {
    pub player_id: i32,
}

#[derive(Debug, Clone, Copy)]
struct HeroPresenceSnapshot {
    pos: Position,
    alive: bool,
    true_death: bool,
    hp: i32,
    last_combat_tick: i32,
    last_damage_tick: Option<i32>,
    bound_monolith: Option<(i32, Position)>,
}

type HeroPresenceQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Id,
        &'static PlayerId,
        &'static Position,
        &'static State,
        Option<&'static StateDead>,
        Option<&'static TrueDeath>,
        &'static Stats,
        &'static LastCombatTick,
        Option<&'static LastDamageTick>,
        Option<&'static BoundMonolith>,
    ),
    With<SubclassHero>,
>;

type BoundMonolithQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Id,
        &'static Position,
        &'static Monolith,
        &'static State,
        Option<&'static StateDead>,
    ),
    With<Monolith>,
>;

type HostileQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static PlayerId,
        &'static Position,
        &'static Template,
        &'static Subclass,
        &'static State,
        &'static Stats,
        Option<&'static StateDead>,
        Option<&'static CrisisAssaultUnit>,
    ),
    (With<SubclassNPC>, With<VisibleTarget>),
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnSanctuaryStatus {
    Inside,
    Outside,
    MissingBinding,
    MissingZone,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafeLogoutCompletionOutcome {
    Completed,
    Disconnected,
    NotPending,
}

pub struct SafeLogoutPlugin;

impl Plugin for SafeLogoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerWorldPresenceState>()
            .add_message::<RequestSafeLogout>()
            .add_message::<CancelSafeLogout>()
            .add_systems(
                PostUpdate,
                (
                    reconcile_player_world_presence_system,
                    safe_logout_request_system,
                    safe_logout_manual_cancel_system,
                    safe_logout_pending_system,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

pub fn initialize_player_presence(
    player_id: i32,
    connected: bool,
    tick: i32,
    presence: &mut PlayerWorldPresenceState,
) {
    let previous = presence.players.get(&player_id).map(|record| record.state);
    let record = PlayerPresenceRecord::new(connected);
    let next = record.state;
    presence.players.insert(player_id, record);
    info!(
        "safe_logout_run_initialized player_id={} previous_presence={} new_presence={} game_tick={}",
        player_id,
        previous.map(PlayerWorldPresence::as_str).unwrap_or("none"),
        next.as_str(),
        tick
    );
}

pub fn mark_player_logged_in(player_id: i32, tick: i32, presence: &mut PlayerWorldPresenceState) {
    let Some(record) = presence.players.get_mut(&player_id) else {
        return;
    };
    let previous = record.state;
    if previous == PlayerWorldPresence::SafeLogoutPending {
        // Login is the authenticated reconnect boundary. Conservatively cancel
        // an in-flight handoff even if the socket gap occurred between ECS
        // evaluations and was therefore not visible to reconciliation.
        record.client_connected = true;
        record.state = PlayerWorldPresence::Online;
        record.safe_logout_requested_tick = None;
        record.safe_logout_start_position = None;
        record.cancel_reason = Some(SafeLogoutCancelReason::Disconnected);
        record.rejection_reason = None;
        info!(
            "safe_logout_countdown_cancelled player_id={} previous_presence={} new_presence={} game_tick={} reason={}",
            player_id,
            previous.as_str(),
            record.state.as_str(),
            tick,
            SafeLogoutCancelReason::Disconnected.as_str()
        );
        return;
    }
    record.client_connected = true;
    record.state = PlayerWorldPresence::Online;
    if previous != record.state {
        record.safe_logout_requested_tick = None;
        record.safe_logout_start_position = None;
        record.cancel_reason = None;
        record.rejection_reason = None;
        info!(
            "safe_logout_reconnect player_id={} previous_presence={} new_presence={} game_tick={}",
            player_id,
            previous.as_str(),
            record.state.as_str(),
            tick
        );
    }
}

pub fn remove_player_presence_for_run_cleanup(
    player_id: i32,
    tick: i32,
    presence: &mut PlayerWorldPresenceState,
) {
    let Some(record) = presence.players.remove(&player_id) else {
        return;
    };
    if record.state == PlayerWorldPresence::SafeLogoutPending {
        info!(
            "safe_logout_countdown_cancelled player_id={} previous_presence={} new_presence=removed game_tick={} reason={}",
            player_id,
            record.state.as_str(),
            tick,
            SafeLogoutCancelReason::RunEnded.as_str()
        );
    }
    info!(
        "safe_logout_run_cleanup player_id={} previous_presence={} new_presence=removed game_tick={} reason={}",
        player_id,
        record.state.as_str(),
        tick,
        SafeLogoutCancelReason::RunEnded.as_str()
    );
}

pub fn record_player_combat_activity(
    player_id: i32,
    tick: i32,
    presence: &mut PlayerWorldPresenceState,
) {
    let Some(record) = presence.players.get_mut(&player_id) else {
        return;
    };
    record.last_combat_tick = Some(
        record
            .last_combat_tick
            .map(|previous| previous.max(tick))
            .unwrap_or(tick),
    );
}

fn resolve_hero(
    player_id: i32,
    ids: &Ids,
    entity_map: &EntityObjMap,
    hero_query: &HeroPresenceQuery,
) -> Option<HeroPresenceSnapshot> {
    let hero_id = ids.get_hero(player_id)?;
    let hero_entity = entity_map.get_entity(hero_id)?;
    let (
        id,
        owner,
        pos,
        state,
        state_dead,
        true_death,
        stats,
        combat_tick,
        damage_tick,
        bound_monolith,
    ) = hero_query.get(hero_entity).ok()?;
    if id.0 != hero_id || owner.0 != player_id || !owner.is_human() {
        return None;
    }
    Some(HeroPresenceSnapshot {
        pos: *pos,
        alive: state.is_alive() && state_dead.is_none() && true_death.is_none() && stats.hp > 0,
        true_death: true_death.is_some(),
        hp: stats.hp,
        last_combat_tick: combat_tick.0,
        last_damage_tick: damage_tick.map(|tick| tick.0),
        bound_monolith: bound_monolith.map(|bound| (bound.id, bound.pos)),
    })
}

fn own_sanctuary_status(
    hero: HeroPresenceSnapshot,
    zones: &SanctuaryZones,
    entity_map: &EntityObjMap,
    monolith_query: &BoundMonolithQuery,
) -> OwnSanctuaryStatus {
    let Some((monolith_id, bound_position)) = hero.bound_monolith else {
        return OwnSanctuaryStatus::MissingBinding;
    };
    let Some(zone) = zones.get(&monolith_id).copied() else {
        return OwnSanctuaryStatus::MissingZone;
    };
    let Some(monolith_entity) = entity_map.get_entity(monolith_id) else {
        return OwnSanctuaryStatus::Invalid;
    };
    let Ok((id, position, monolith, state, state_dead)) = monolith_query.get(monolith_entity)
    else {
        return OwnSanctuaryStatus::Invalid;
    };
    if id.0 != monolith_id
        || !state.is_alive()
        || state_dead.is_some()
        || *position != zone.pos
        || bound_position != zone.pos
        || monolith.sanctuary_level != zone.level
    {
        return OwnSanctuaryStatus::Invalid;
    }
    if Map::distance((hero.pos.x, hero.pos.y), (zone.pos.x, zone.pos.y)) < zone.full_radius() {
        OwnSanctuaryStatus::Inside
    } else {
        OwnSanctuaryStatus::Outside
    }
}

fn hostile_nearby(
    player_id: i32,
    hero_pos: Position,
    templates: &Templates,
    hostile_query: &HostileQuery,
) -> bool {
    hostile_query.iter().any(
        |(owner, pos, template, subclass, state, stats, state_dead, assault)| {
            if *subclass != Subclass::Npc
                || !owner.is_npc()
                || !state.is_alive()
                || state_dead.is_some()
                || stats.hp <= 0
            {
                return false;
            }

            // Personal-assault units target only their attributed settlement.
            if assault
                .map(|unit| unit.owner_player_id != player_id)
                .unwrap_or(false)
            {
                return false;
            }

            // Missing template/aggression data fails closed as an immediate
            // threat. Passive wildlife follows the existing NPC targeting rule
            // and does not block safe logout.
            let passive = templates
                .obj_templates
                .iter()
                .find(|candidate| candidate.template == template.0)
                .and_then(|candidate| candidate.aggression.as_ref())
                .map(npc::is_passive)
                .unwrap_or(false);
            !passive
                && Map::distance((hero_pos.x, hero_pos.y), (pos.x, pos.y))
                    <= SAFE_LOGOUT_HOSTILE_RADIUS
        },
    )
}

fn crisis_is_assault_active(player_id: i32, crises: &SettlementCrisisState) -> bool {
    crises
        .get(&player_id)
        .map(|crisis| crisis.phase == CrisisPhase::AssaultActive)
        .unwrap_or(false)
}

fn is_recent(current_tick: i32, activity_tick: i32) -> bool {
    current_tick.saturating_sub(activity_tick) < SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS
}

fn cancel_pending(
    player_id: i32,
    reason: SafeLogoutCancelReason,
    connected: bool,
    tick: i32,
    presence: &mut PlayerWorldPresenceState,
) -> bool {
    let Some(record) = presence.players.get_mut(&player_id) else {
        return false;
    };
    if record.state != PlayerWorldPresence::SafeLogoutPending {
        return false;
    }
    let previous = record.state;
    record.state = if connected {
        PlayerWorldPresence::Online
    } else {
        PlayerWorldPresence::Disconnected
    };
    record.client_connected = connected;
    record.safe_logout_requested_tick = None;
    record.safe_logout_start_position = None;
    record.cancel_reason = Some(reason);
    record.rejection_reason = None;
    info!(
        "safe_logout_countdown_cancelled player_id={} previous_presence={} new_presence={} game_tick={} reason={}",
        player_id,
        previous.as_str(),
        record.state.as_str(),
        tick,
        reason.as_str()
    );
    true
}

/// Complete a pending handoff against two authoritative connection samples.
///
/// The second sample is deliberately taken after the provisional ECS state
/// write. No other ECS system can observe that write while this system owns the
/// resource, so a disconnect observed by either sample rolls it back to
/// `Disconnected` without ever publishing or logging protection. A disconnect
/// after the second sample is ordered after the completion boundary.
fn complete_pending_with_connection_check(
    player_id: i32,
    tick: i32,
    presence: &mut PlayerWorldPresenceState,
    mut is_connected: impl FnMut() -> bool,
) -> SafeLogoutCompletionOutcome {
    if !is_connected() {
        cancel_pending(
            player_id,
            SafeLogoutCancelReason::Disconnected,
            false,
            tick,
            presence,
        );
        return SafeLogoutCompletionOutcome::Disconnected;
    }

    let Some(record) = presence.players.get_mut(&player_id) else {
        return SafeLogoutCompletionOutcome::NotPending;
    };
    if record.state != PlayerWorldPresence::SafeLogoutPending {
        return SafeLogoutCompletionOutcome::NotPending;
    }

    let previous = record.state;
    record.state = PlayerWorldPresence::OfflineProtected;
    record.client_connected = true;
    record.safe_logout_requested_tick = None;
    record.safe_logout_start_position = None;
    record.cancel_reason = None;
    record.rejection_reason = None;

    if !is_connected() {
        record.state = PlayerWorldPresence::Disconnected;
        record.client_connected = false;
        record.cancel_reason = Some(SafeLogoutCancelReason::Disconnected);
        info!(
            "safe_logout_countdown_cancelled player_id={} previous_presence={} new_presence={} game_tick={} reason={}",
            player_id,
            previous.as_str(),
            record.state.as_str(),
            tick,
            SafeLogoutCancelReason::Disconnected.as_str()
        );
        return SafeLogoutCompletionOutcome::Disconnected;
    }

    info!(
        "safe_logout_countdown_completed player_id={} previous_presence={} new_presence={} game_tick={}",
        player_id,
        previous.as_str(),
        record.state.as_str(),
        tick
    );
    SafeLogoutCompletionOutcome::Completed
}

fn reconcile_player_world_presence_system(
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    assigned_runs: Res<AssignedStartLocations>,
    hero_query: HeroPresenceQuery,
    mut presence: ResMut<PlayerWorldPresenceState>,
) {
    let mut player_ids = presence.players.keys().copied().collect::<HashSet<_>>();
    player_ids.extend(ids.player_hero_map.keys().copied());
    player_ids.extend(assigned_runs.keys().copied());

    for player_id in player_ids {
        let valid_run = assigned_runs.contains_key(&player_id);
        let hero = resolve_hero(player_id, &ids, &entity_map, &hero_query);
        if !valid_run || hero.is_none() {
            remove_player_presence_for_run_cleanup(player_id, game_tick.0, &mut presence);
            continue;
        }
        let hero = hero.expect("checked above");
        let connected = clients.is_player_online(player_id);
        if !presence.players.contains_key(&player_id) {
            initialize_player_presence(player_id, connected, game_tick.0, &mut presence);
        }

        if let Some(record) = presence.players.get_mut(&player_id) {
            if let Some(damage_tick) = hero.last_damage_tick {
                record.last_damage_tick = Some(
                    record
                        .last_damage_tick
                        .map(|previous| previous.max(damage_tick))
                        .unwrap_or(damage_tick),
                );
            }
            if record
                .last_observed_hp
                .map(|previous| hero.hp < previous)
                .unwrap_or(false)
            {
                record.last_damage_tick = Some(game_tick.0);
            }
            record.last_observed_hp = Some(hero.hp);
        }

        if !connected {
            let state = presence.players.get(&player_id).map(|record| record.state);
            if state == Some(PlayerWorldPresence::SafeLogoutPending) {
                cancel_pending(
                    player_id,
                    SafeLogoutCancelReason::Disconnected,
                    false,
                    game_tick.0,
                    &mut presence,
                );
            } else if state == Some(PlayerWorldPresence::Online) {
                if let Some(record) = presence.players.get_mut(&player_id) {
                    let previous = record.state;
                    record.state = PlayerWorldPresence::Disconnected;
                    record.client_connected = false;
                    info!(
                        "safe_logout_ordinary_disconnect player_id={} previous_presence={} new_presence={} game_tick={}",
                        player_id,
                        previous.as_str(),
                        record.state.as_str(),
                        game_tick.0
                    );
                }
            } else if let Some(record) = presence.players.get_mut(&player_id) {
                record.client_connected = false;
            }
            continue;
        }

        if let Some(record) = presence.players.get_mut(&player_id) {
            let previous_connection = record.client_connected;
            record.client_connected = true;
            let reconnect = record.state == PlayerWorldPresence::Disconnected
                || (record.state == PlayerWorldPresence::OfflineProtected && !previous_connection);
            if reconnect {
                let previous = record.state;
                record.state = PlayerWorldPresence::Online;
                record.safe_logout_requested_tick = None;
                record.safe_logout_start_position = None;
                record.cancel_reason = None;
                record.rejection_reason = None;
                info!(
                    "safe_logout_reconnect player_id={} previous_presence={} new_presence={} game_tick={}",
                    player_id,
                    previous.as_str(),
                    record.state.as_str(),
                    game_tick.0
                );
            }
        }
    }
}

fn request_rejection(
    player_id: i32,
    tick: i32,
    clients: &Clients,
    ids: &Ids,
    entity_map: &EntityObjMap,
    assigned_runs: &AssignedStartLocations,
    zones: &SanctuaryZones,
    crises: &SettlementCrisisState,
    templates: &Templates,
    hero_query: &HeroPresenceQuery,
    monolith_query: &BoundMonolithQuery,
    hostile_query: &HostileQuery,
    record: Option<&PlayerPresenceRecord>,
) -> Option<SafeLogoutRejectionReason> {
    let Some(record) = record else {
        return Some(SafeLogoutRejectionReason::InvalidRun);
    };
    match record.state {
        PlayerWorldPresence::SafeLogoutPending => {
            return Some(SafeLogoutRejectionReason::AlreadyPending);
        }
        PlayerWorldPresence::OfflineProtected => {
            return Some(SafeLogoutRejectionReason::AlreadyProtected);
        }
        PlayerWorldPresence::Disconnected => {
            return Some(SafeLogoutRejectionReason::NotOnline);
        }
        PlayerWorldPresence::Online => {}
    }
    if !clients.is_player_online(player_id) {
        return Some(SafeLogoutRejectionReason::NotOnline);
    }
    if !assigned_runs.contains_key(&player_id) {
        return Some(SafeLogoutRejectionReason::InvalidRun);
    }
    let Some(hero) = resolve_hero(player_id, ids, entity_map, hero_query) else {
        return Some(SafeLogoutRejectionReason::MissingHero);
    };
    if hero.true_death {
        return Some(SafeLogoutRejectionReason::TrueDeath);
    }
    if !hero.alive {
        return Some(SafeLogoutRejectionReason::HeroDied);
    }
    if crisis_is_assault_active(player_id, crises) {
        return Some(SafeLogoutRejectionReason::AssaultActive);
    }
    if record
        .last_damage_tick
        .map(|last_tick| is_recent(tick, last_tick))
        .unwrap_or(false)
        || hero
            .last_damage_tick
            .map(|last_tick| is_recent(tick, last_tick))
            .unwrap_or(false)
    {
        return Some(SafeLogoutRejectionReason::RecentDamage);
    }
    if record
        .last_combat_tick
        .map(|last_tick| is_recent(tick, last_tick))
        .unwrap_or(false)
        || is_recent(tick, hero.last_combat_tick)
    {
        return Some(SafeLogoutRejectionReason::RecentCombat);
    }
    match own_sanctuary_status(hero, zones, entity_map, monolith_query) {
        OwnSanctuaryStatus::Inside => {}
        OwnSanctuaryStatus::Outside => {
            return Some(SafeLogoutRejectionReason::OutsideOwnSanctuary);
        }
        OwnSanctuaryStatus::MissingBinding => {
            return Some(SafeLogoutRejectionReason::MissingBoundMonolith);
        }
        OwnSanctuaryStatus::MissingZone => {
            return Some(SafeLogoutRejectionReason::MissingSanctuaryZone);
        }
        OwnSanctuaryStatus::Invalid => {
            return Some(SafeLogoutRejectionReason::SanctuaryInvalid);
        }
    }
    if hostile_nearby(player_id, hero.pos, templates, hostile_query) {
        return Some(SafeLogoutRejectionReason::HostileNearby);
    }
    None
}

fn safe_logout_request_system(
    mut requests: MessageReader<RequestSafeLogout>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    assigned_runs: Res<AssignedStartLocations>,
    zones: Res<SanctuaryZones>,
    crises: Res<SettlementCrisisState>,
    templates: Res<Templates>,
    hero_query: HeroPresenceQuery,
    monolith_query: BoundMonolithQuery,
    hostile_query: HostileQuery,
    mut presence: ResMut<PlayerWorldPresenceState>,
) {
    for request in requests.read() {
        let previous = presence
            .players
            .get(&request.player_id)
            .map(|record| record.state);
        let rejection = request_rejection(
            request.player_id,
            game_tick.0,
            &clients,
            &ids,
            &entity_map,
            &assigned_runs,
            &zones,
            &crises,
            &templates,
            &hero_query,
            &monolith_query,
            &hostile_query,
            presence.players.get(&request.player_id),
        );
        if let Some(reason) = rejection {
            if let Some(record) = presence.players.get_mut(&request.player_id) {
                record.rejection_reason = Some(reason);
            }
            info!(
                "safe_logout_request_rejected player_id={} previous_presence={} new_presence={} game_tick={} reason={}",
                request.player_id,
                previous.map(PlayerWorldPresence::as_str).unwrap_or("none"),
                previous.map(PlayerWorldPresence::as_str).unwrap_or("none"),
                game_tick.0,
                reason.as_str()
            );
            continue;
        }

        let Some(hero) = resolve_hero(request.player_id, &ids, &entity_map, &hero_query) else {
            continue;
        };
        let Some(record) = presence.players.get_mut(&request.player_id) else {
            continue;
        };
        let previous = record.state;
        info!(
            "safe_logout_requested player_id={} previous_presence={} new_presence={} game_tick={}",
            request.player_id,
            previous.as_str(),
            PlayerWorldPresence::SafeLogoutPending.as_str(),
            game_tick.0
        );
        record.state = PlayerWorldPresence::SafeLogoutPending;
        record.safe_logout_requested_tick = Some(game_tick.0);
        record.safe_logout_start_position = Some(hero.pos);
        record.cancel_reason = None;
        record.rejection_reason = None;
        info!(
            "safe_logout_countdown_started player_id={} previous_presence={} new_presence={} game_tick={} countdown_ticks={}",
            request.player_id,
            previous.as_str(),
            record.state.as_str(),
            game_tick.0,
            SAFE_LOGOUT_COUNTDOWN_TICKS
        );
    }
}

fn safe_logout_manual_cancel_system(
    mut cancellations: MessageReader<CancelSafeLogout>,
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    mut presence: ResMut<PlayerWorldPresenceState>,
) {
    for cancellation in cancellations.read() {
        cancel_pending(
            cancellation.player_id,
            SafeLogoutCancelReason::Manual,
            clients.is_player_online(cancellation.player_id),
            game_tick.0,
            &mut presence,
        );
    }
}

fn safe_logout_pending_system(
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    assigned_runs: Res<AssignedStartLocations>,
    zones: Res<SanctuaryZones>,
    crises: Res<SettlementCrisisState>,
    templates: Res<Templates>,
    hero_query: HeroPresenceQuery,
    monolith_query: BoundMonolithQuery,
    hostile_query: HostileQuery,
    mut presence: ResMut<PlayerWorldPresenceState>,
) {
    let pending_players = presence
        .players
        .iter()
        .filter_map(|(player_id, record)| {
            (record.state == PlayerWorldPresence::SafeLogoutPending).then_some(*player_id)
        })
        .collect::<Vec<_>>();

    for player_id in pending_players {
        let connected = clients.is_player_online(player_id);
        if !connected {
            cancel_pending(
                player_id,
                SafeLogoutCancelReason::Disconnected,
                false,
                game_tick.0,
                &mut presence,
            );
            continue;
        }
        if !assigned_runs.contains_key(&player_id) {
            remove_player_presence_for_run_cleanup(player_id, game_tick.0, &mut presence);
            continue;
        }
        let Some(hero) = resolve_hero(player_id, &ids, &entity_map, &hero_query) else {
            remove_player_presence_for_run_cleanup(player_id, game_tick.0, &mut presence);
            continue;
        };
        if hero.true_death || !hero.alive {
            cancel_pending(
                player_id,
                SafeLogoutCancelReason::HeroDied,
                true,
                game_tick.0,
                &mut presence,
            );
            continue;
        }
        if crisis_is_assault_active(player_id, &crises) {
            cancel_pending(
                player_id,
                SafeLogoutCancelReason::AssaultStarted,
                true,
                game_tick.0,
                &mut presence,
            );
            continue;
        }

        let Some(record_snapshot) = presence.players.get(&player_id).cloned() else {
            continue;
        };
        let requested_tick = record_snapshot
            .safe_logout_requested_tick
            .unwrap_or(game_tick.0);
        if record_snapshot
            .last_damage_tick
            .map(|tick| tick >= requested_tick)
            .unwrap_or(false)
            || hero
                .last_damage_tick
                .map(|tick| tick >= requested_tick)
                .unwrap_or(false)
        {
            cancel_pending(
                player_id,
                SafeLogoutCancelReason::TookDamage,
                true,
                game_tick.0,
                &mut presence,
            );
            continue;
        }
        if record_snapshot
            .last_combat_tick
            .map(|tick| tick >= requested_tick)
            .unwrap_or(false)
            || hero.last_combat_tick >= requested_tick
        {
            cancel_pending(
                player_id,
                SafeLogoutCancelReason::EnteredCombat,
                true,
                game_tick.0,
                &mut presence,
            );
            continue;
        }
        if record_snapshot.safe_logout_start_position != Some(hero.pos) {
            cancel_pending(
                player_id,
                SafeLogoutCancelReason::Moved,
                true,
                game_tick.0,
                &mut presence,
            );
            continue;
        }
        match own_sanctuary_status(hero, &zones, &entity_map, &monolith_query) {
            OwnSanctuaryStatus::Inside => {}
            OwnSanctuaryStatus::Outside => {
                cancel_pending(
                    player_id,
                    SafeLogoutCancelReason::LeftSanctuary,
                    true,
                    game_tick.0,
                    &mut presence,
                );
                continue;
            }
            OwnSanctuaryStatus::MissingBinding
            | OwnSanctuaryStatus::MissingZone
            | OwnSanctuaryStatus::Invalid => {
                cancel_pending(
                    player_id,
                    SafeLogoutCancelReason::SanctuaryInvalid,
                    true,
                    game_tick.0,
                    &mut presence,
                );
                continue;
            }
        }
        if hostile_nearby(player_id, hero.pos, &templates, &hostile_query) {
            cancel_pending(
                player_id,
                SafeLogoutCancelReason::HostileNearby,
                true,
                game_tick.0,
                &mut presence,
            );
            continue;
        }

        if game_tick.0.saturating_sub(requested_tick) < SAFE_LOGOUT_COUNTDOWN_TICKS {
            continue;
        }
        complete_pending_with_connection_check(player_id, game_tick.0, &mut presence, || {
            clients.is_player_online(player_id)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_logout_checkpoint1_pending_cancellation_is_typed_and_idempotent() {
        let player_id = 7;
        let mut presence = PlayerWorldPresenceState::default();
        let mut record = PlayerPresenceRecord::new(true);
        record.state = PlayerWorldPresence::SafeLogoutPending;
        record.safe_logout_requested_tick = Some(100);
        record.safe_logout_start_position = Some(Position { x: 4, y: 5 });
        presence.players.insert(player_id, record);

        assert!(cancel_pending(
            player_id,
            SafeLogoutCancelReason::Moved,
            true,
            101,
            &mut presence,
        ));
        let cancelled = presence.players.get(&player_id).unwrap().clone();
        assert_eq!(cancelled.state, PlayerWorldPresence::Online);
        assert_eq!(cancelled.cancel_reason, Some(SafeLogoutCancelReason::Moved));
        assert_eq!(cancelled.safe_logout_requested_tick, None);
        assert_eq!(cancelled.safe_logout_start_position, None);

        assert!(!cancel_pending(
            player_id,
            SafeLogoutCancelReason::HostileNearby,
            true,
            101,
            &mut presence,
        ));
        assert_eq!(presence.players.get(&player_id), Some(&cancelled));
    }

    #[test]
    fn safe_logout_checkpoint1_activity_cooldown_has_an_exact_boundary() {
        let activity_tick = 1_000;
        assert!(is_recent(
            activity_tick + SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS - 1,
            activity_tick,
        ));
        assert!(!is_recent(
            activity_tick + SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS,
            activity_tick,
        ));
    }

    #[test]
    fn safe_logout_checkpoint1_fresh_run_and_cleanup_are_isolated() {
        let mut presence = PlayerWorldPresenceState::default();
        initialize_player_presence(1, true, 10, &mut presence);
        initialize_player_presence(2, false, 10, &mut presence);
        {
            let first = presence.players.get_mut(&1).unwrap();
            first.state = PlayerWorldPresence::OfflineProtected;
            first.last_combat_tick = Some(9);
            first.last_damage_tick = Some(8);
        }

        remove_player_presence_for_run_cleanup(1, 11, &mut presence);
        remove_player_presence_for_run_cleanup(1, 11, &mut presence);
        assert!(!presence.players.contains_key(&1));
        assert_eq!(
            presence.players.get(&2).map(|record| record.state),
            Some(PlayerWorldPresence::Disconnected),
        );

        initialize_player_presence(1, true, 12, &mut presence);
        let fresh = presence.players.get(&1).unwrap();
        assert_eq!(fresh.state, PlayerWorldPresence::Online);
        assert_eq!(fresh.safe_logout_requested_tick, None);
        assert_eq!(fresh.safe_logout_start_position, None);
        assert_eq!(fresh.last_combat_tick, None);
        assert_eq!(fresh.last_damage_tick, None);
        assert_eq!(fresh.cancel_reason, None);
    }

    #[test]
    fn safe_logout_checkpoint1_player_combat_aggregate_is_monotonic() {
        let player_id = 9;
        let mut presence = PlayerWorldPresenceState::default();
        initialize_player_presence(player_id, true, 1, &mut presence);
        record_player_combat_activity(player_id, 50, &mut presence);
        record_player_combat_activity(player_id, 40, &mut presence);
        assert_eq!(
            presence
                .players
                .get(&player_id)
                .and_then(|record| record.last_combat_tick),
            Some(50),
        );
    }

    #[test]
    fn safe_logout_checkpoint1_disconnect_between_completion_samples_wins() {
        use std::cell::Cell;

        let player_id = 11;
        let mut presence = PlayerWorldPresenceState::default();
        let mut record = PlayerPresenceRecord::new(true);
        record.state = PlayerWorldPresence::SafeLogoutPending;
        record.safe_logout_requested_tick = Some(100);
        record.safe_logout_start_position = Some(Position { x: 4, y: 5 });
        presence.players.insert(player_id, record);

        let samples = Cell::new(0);
        let outcome = complete_pending_with_connection_check(player_id, 200, &mut presence, || {
            let sample = samples.get();
            samples.set(sample + 1);
            sample == 0
        });

        assert_eq!(outcome, SafeLogoutCompletionOutcome::Disconnected);
        assert_eq!(samples.get(), 2);
        let record = presence.players.get(&player_id).unwrap();
        assert_eq!(record.state, PlayerWorldPresence::Disconnected);
        assert_eq!(
            record.cancel_reason,
            Some(SafeLogoutCancelReason::Disconnected)
        );
        assert_eq!(record.safe_logout_requested_tick, None);
        assert_eq!(record.safe_logout_start_position, None);
    }
}
