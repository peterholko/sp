//! Explicit safe logout and owner-scoped offline simulation protection.
//!
//! Safe-logout requests enter through authenticated network commands or
//! in-process server code, but `OfflineProtected` remains an authoritative
//! server state: gameplay systems must consult the helpers in this module
//! before they mutate player-owned state.

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::common::{Dehydrated, Exhausted, Idle, Starving};
use crate::constants::TICKS_PER_SEC;
use crate::event::{GameEvent, GameEventType, GameEvents, MapEvents, VisibleEvent};
use crate::farm::{CropStages, Crops};
use crate::game::{
    BoundMonolith, Burning, Client, Clients, CrisisAssaultUnit, CrisisPhase, GameTick,
    InitialEncounterState, IntroEncounterState, LegendaryThreatState, Merchant, Monolith,
    PlayerIntroState, RunScoreState, SanctuaryZones, SettlementCrisisState,
};
use crate::ids::{EntityObjMap, Ids};
use crate::item::Inventory;
use crate::map::Map;
use crate::network::{ResponsePacket, SafeLogoutStatusSnapshot};
use crate::npc::{self, VisibleTarget};
use crate::obj::{
    BuildUpgradeState, Campfire, Id, LastAttacker, LastCombatTick, LastDamageTick, PlayerId,
    Position, State, StateDead, Stats, Subclass, SubclassHero, SubclassNPC, Template, TrueDeath,
};
use crate::player_setup::{AssignedStartLocations, RunSpawnedObjs};
use crate::tax_collector::TaxCollector;
use crate::templates::Templates;
use crate::villager::{NoDrinks, NoFood};
use crate::AppState;

pub const SAFE_LOGOUT_COUNTDOWN_TICKS: i32 = TICKS_PER_SEC * 10;
pub const SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS: i32 = TICKS_PER_SEC * 15;
pub const SAFE_LOGOUT_HOSTILE_RADIUS: u32 = 8;
pub const SAFE_LOGOUT_STATUS_VERSION: u32 = 1;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    InvalidState,
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
            Self::InvalidState => "invalid_state",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Runtime identity for the exact run whose simulation was protected.
///
/// The start-location name distinguishes a recycled slot and the authoritative
/// hero id distinguishes a recreated hero. The bound monolith is included
/// because monolith entities use the shared monolith faction `PlayerId` rather
/// than the settlement owner's `PlayerId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedRunKey {
    pub player_id: i32,
    pub hero_id: i32,
    pub start_location_name: String,
    pub bound_monolith_id: i32,
    /// Snapshot of the existing runtime `RunSpawnedObjs` attribution. These
    /// objects deliberately use neutral/NPC factions, so ordinary `Ids`
    /// ownership cannot associate them with the protected run.
    pub run_object_ids: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerPresenceRecord {
    pub state: PlayerWorldPresence,
    pub safe_logout_requested_tick: Option<i32>,
    pub safe_logout_start_position: Option<Position>,
    /// Global tick at the successful pending -> protected boundary.
    pub protected_since_tick: Option<i32>,
    /// Exact runtime run identity protected by this record.
    pub protected_run_key: Option<ProtectedRunKey>,
    /// Last tick at which a validated protection interval finished.
    pub last_protection_end_tick: Option<i32>,
    /// Successful player-commanded combat from any owned source. The hero's
    /// `LastCombatTick` remains authoritative for entity combat; this aggregate
    /// closes the gap for commands issued through another owned combatant.
    pub last_combat_tick: Option<i32>,
    pub last_damage_tick: Option<i32>,
    pub cancel_reason: Option<SafeLogoutCancelReason>,
    pub rejection_reason: Option<SafeLogoutRejectionReason>,
    pub(crate) last_observed_hp: Option<i32>,
    pub(crate) client_connected: bool,
    pub(crate) safe_logout_connection_ids: Vec<Uuid>,
    /// An exact authenticated Login event requests an ordered resume.
    /// The state remains protected until all owner deadlines are rebased in
    /// PostUpdate, so the reconnect Update cannot run a backlog.
    pub(crate) protection_exit_requested: bool,
    /// Rebase has published connected `Online` presence, but owner simulation
    /// remains suspended until the connection-scoped Login sync finishes.
    pub(crate) resume_in_progress: bool,
    /// The one authoritative connection allowed to finish this resume.
    pub(crate) resume_connection_id: Option<Uuid>,
    /// Set only after the delayed Login world/perception synchronization ran.
    pub(crate) resume_sync_ready: bool,
    /// The exact connection that may receive one successfully queued online
    /// resume announcement. This is a delivery pulse, not gameplay state.
    pub(crate) resume_notice_connection_id: Option<Uuid>,
    /// Deduplicates repeated Login events from the same authoritative socket.
    pub(crate) last_login_connection_id: Option<Uuid>,
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
            protected_since_tick: None,
            protected_run_key: None,
            last_protection_end_tick: None,
            last_combat_tick: None,
            last_damage_tick: None,
            cancel_reason: None,
            rejection_reason: None,
            last_observed_hp: None,
            client_connected: connected,
            safe_logout_connection_ids: Vec::new(),
            protection_exit_requested: false,
            resume_in_progress: false,
            resume_connection_id: None,
            resume_sync_ready: false,
            resume_notice_connection_id: None,
            last_login_connection_id: None,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct PlayerWorldPresenceState {
    pub players: HashMap<i32, PlayerPresenceRecord>,
}

/// Runtime-only, per-run Safe Logout observations. Counters are incremented at
/// authoritative transitions and final rejection boundaries, never per skipped
/// simulation tick and never in the database.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SafeLogoutTelemetry {
    pub requests: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub cancelled: u64,
    pub completed: u64,
    pub protected_sessions_started: u64,
    pub resumed: u64,
    pub protected_ticks_total: u64,
    pub ordinary_disconnects: u64,
    pub active_assault_disconnects: u64,
    pub status_packets_sent: u64,
    pub status_packets_duplicate_suppressed: u64,
    pub protected_input_rejections: u64,
    pub protected_damage_blocks: u64,
    pub protected_target_rejections: u64,
    pub queued_events_discarded: u64,
    pub invariant_recoveries: u64,
    pub run_key_mismatches: u64,
    pub timer_rebases: u64,
    pub stale_connection_events_rejected: u64,
    pub rejection_reasons: HashMap<SafeLogoutRejectionReason, u64>,
    pub cancellation_reasons: HashMap<SafeLogoutCancelReason, u64>,
    pub invariant_reasons: HashMap<String, u64>,
}

impl SafeLogoutTelemetry {
    fn record_rejection(&mut self, reason: SafeLogoutRejectionReason) {
        self.rejected = self.rejected.saturating_add(1);
        let count = self.rejection_reasons.entry(reason).or_default();
        *count = count.saturating_add(1);
    }

    fn record_cancellation(&mut self, reason: SafeLogoutCancelReason) {
        self.cancelled = self.cancelled.saturating_add(1);
        let count = self.cancellation_reasons.entry(reason).or_default();
        *count = count.saturating_add(1);
    }

    fn record_invariant(&mut self, reason: &str) {
        self.invariant_recoveries = self.invariant_recoveries.saturating_add(1);
        let count = self
            .invariant_reasons
            .entry(reason.to_string())
            .or_default();
        *count = count.saturating_add(1);
        if reason.contains("run_key_mismatch") {
            self.run_key_mismatches = self.run_key_mismatches.saturating_add(1);
        }
    }
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct SafeLogoutTelemetryState(pub HashMap<i32, SafeLogoutTelemetry>);

impl SafeLogoutTelemetryState {
    fn player_mut(&mut self, player_id: i32) -> &mut SafeLogoutTelemetry {
        self.entry(player_id).or_default()
    }

    pub fn record_protected_input_rejection(&mut self, player_id: i32) {
        let telemetry = self.player_mut(player_id);
        telemetry.protected_input_rejections =
            telemetry.protected_input_rejections.saturating_add(1);
    }

    pub fn record_stale_connection_event(&mut self, player_id: i32) {
        let telemetry = self.player_mut(player_id);
        telemetry.stale_connection_events_rejected =
            telemetry.stale_connection_events_rejected.saturating_add(1);
    }

    pub fn record_protected_damage_block(&mut self, player_id: i32) {
        let telemetry = self.player_mut(player_id);
        telemetry.protected_damage_blocks = telemetry.protected_damage_blocks.saturating_add(1);
    }

    pub fn record_protected_target_rejection(&mut self, player_id: i32) {
        let telemetry = self.player_mut(player_id);
        telemetry.protected_target_rejections =
            telemetry.protected_target_rejections.saturating_add(1);
    }

    pub fn record_protected_queued_events_discarded(&mut self, player_id: i32, discarded: usize) {
        let telemetry = self.player_mut(player_id);
        telemetry.queued_events_discarded = telemetry
            .queued_events_discarded
            .saturating_add(discarded as u64);
    }
}

#[derive(Debug, Clone)]
struct SentSafeLogoutStatus {
    player_id: i32,
    status: SafeLogoutStatusSnapshot,
    /// Count one suppressed duplicate per stable snapshot period, not once per
    /// game tick, so observability remains low-volume.
    duplicate_suppression_recorded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafeLogoutStatusSendOutcome {
    Sent,
    StaleConnection,
    RegistryUnavailable,
    ChannelFull,
    ChannelClosed,
}

/// Last successfully queued status per authenticated connection. Failed sends
/// are not cached and remain retryable on the following update.
#[derive(Resource, Debug, Default)]
struct SafeLogoutStatusDeliveryState {
    sent: HashMap<Uuid, SentSafeLogoutStatus>,
}

/// The canonical player-level protection predicate used by simulation systems.
pub fn is_player_offline_protected(player_id: i32, presence: &PlayerWorldPresenceState) -> bool {
    presence
        .players
        .get(&player_id)
        .map(|record| {
            record.state == PlayerWorldPresence::OfflineProtected || record.resume_in_progress
        })
        .unwrap_or(false)
}

/// Canonical ownership predicate for ordinary player-owned ECS entities.
pub fn is_owner_offline_protected(owner: &PlayerId, presence: &PlayerWorldPresenceState) -> bool {
    owner.is_human() && is_player_offline_protected(owner.0, presence)
}

/// Canonical object-id predicate. This handles ordinary `Ids` ownership plus
/// the bound-monolith and neutral run-attribution exceptions captured by the
/// protected run key.
pub fn object_belongs_to_protected_run(
    obj_id: i32,
    ids: &Ids,
    presence: &PlayerWorldPresenceState,
) -> bool {
    ids.get_player(obj_id)
        .map(|player_id| is_player_offline_protected(player_id, presence))
        .unwrap_or(false)
        || presence.players.values().any(|record| {
            (record.state == PlayerWorldPresence::OfflineProtected || record.resume_in_progress)
                && record
                    .protected_run_key
                    .as_ref()
                    .map(|key| {
                        key.bound_monolith_id == obj_id || key.run_object_ids.contains(&obj_id)
                    })
                    .unwrap_or(false)
        })
}

/// Resolve the protected player for one object ID. Telemetry uses this at
/// final mutation/target rejection boundaries so defence-in-depth checks do
/// not need to infer ownership from proximity or hostile attribution.
pub fn protected_player_for_object(
    obj_id: i32,
    ids: &Ids,
    presence: &PlayerWorldPresenceState,
) -> Option<i32> {
    ids.get_player(obj_id)
        .filter(|player_id| is_player_offline_protected(*player_id, presence))
        .or_else(|| {
            presence.players.iter().find_map(|(player_id, record)| {
                (is_player_offline_protected(*player_id, presence)
                    && record.protected_run_key.as_ref().is_some_and(|key| {
                        key.bound_monolith_id == obj_id || key.run_object_ids.contains(&obj_id)
                    }))
                .then_some(*player_id)
            })
        })
}

/// Canonical entity-level predicate for systems that already queried both the
/// object id and its ECS owner.
pub fn entity_belongs_to_protected_run(
    id: &Id,
    owner: &PlayerId,
    presence: &PlayerWorldPresenceState,
) -> bool {
    is_owner_offline_protected(owner, presence)
        || presence.players.values().any(|record| {
            (record.state == PlayerWorldPresence::OfflineProtected || record.resume_in_progress)
                && record
                    .protected_run_key
                    .as_ref()
                    .map(|key| key.bound_monolith_id == id.0 || key.run_object_ids.contains(&id.0))
                    .unwrap_or(false)
        })
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestSafeLogout {
    pub player_id: i32,
    pub connection_id: Uuid,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelSafeLogout {
    pub player_id: i32,
    pub connection_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
struct HeroPresenceSnapshot {
    id: i32,
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

/// One read-only evaluation shared by authoritative request handling and
/// player-facing status presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SafeLogoutEligibility {
    eligible: bool,
    reason: Option<SafeLogoutRejectionReason>,
    in_own_sanctuary: bool,
    active_assault: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SafeLogoutCompletionOutcome {
    Completed,
    Cancelled,
    NotPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtectionResumePhase {
    Starting,
    Finalizing,
}

fn protected_resume_fields_are_impossible(record: &PlayerPresenceRecord) -> bool {
    record.state == PlayerWorldPresence::OfflineProtected
        && record.protection_exit_requested
        && record.resume_in_progress
}

fn protection_resume_phase(record: &PlayerPresenceRecord) -> Option<ProtectionResumePhase> {
    if record.state == PlayerWorldPresence::OfflineProtected
        && record.protection_exit_requested
        && !record.resume_in_progress
        && !record.resume_sync_ready
        && record.resume_connection_id.is_some()
    {
        Some(ProtectionResumePhase::Starting)
    } else if record.state == PlayerWorldPresence::Online
        && !record.protection_exit_requested
        && record.resume_in_progress
        && record.resume_sync_ready
        && record.resume_connection_id.is_some()
    {
        Some(ProtectionResumePhase::Finalizing)
    } else {
        None
    }
}

fn clear_protected_fields(record: &mut PlayerPresenceRecord) {
    record.protected_since_tick = None;
    record.protected_run_key = None;
    record.protection_exit_requested = false;
    record.resume_in_progress = false;
    record.resume_connection_id = None;
    record.resume_sync_ready = false;
}

fn clear_pending_fields(record: &mut PlayerPresenceRecord) {
    record.safe_logout_requested_tick = None;
    record.safe_logout_start_position = None;
    record.safe_logout_connection_ids.clear();
}

fn close_protected_interval(
    record: &PlayerPresenceRecord,
    tick: i32,
    telemetry: &mut SafeLogoutTelemetry,
) {
    if let Some(started) = record.protected_since_tick {
        telemetry.protected_ticks_total = telemetry
            .protected_ticks_total
            .saturating_add(tick.saturating_sub(started).max(0) as u64);
    }
}

fn recover_invalid_protection(
    player_id: i32,
    connected: bool,
    tick: i32,
    reason: &str,
    presence: &mut PlayerWorldPresenceState,
    telemetry: &mut SafeLogoutTelemetryState,
) {
    let Some(record) = presence.players.get_mut(&player_id) else {
        return;
    };
    if record.state != PlayerWorldPresence::OfflineProtected && !record.resume_in_progress {
        return;
    }
    close_protected_interval(record, tick, telemetry.player_mut(player_id));
    let previous = record.state;
    record.state = if connected {
        PlayerWorldPresence::Online
    } else {
        PlayerWorldPresence::Disconnected
    };
    record.client_connected = connected;
    clear_pending_fields(record);
    clear_protected_fields(record);
    record.resume_notice_connection_id = None;
    telemetry.player_mut(player_id).record_invariant(reason);
    warn!(
        "safe_logout_protection_invalidated player_id={} previous_presence={} new_presence={} game_tick={} reason={}",
        player_id,
        previous.as_str(),
        record.state.as_str(),
        tick,
        reason
    );
}

/// Fail-safe invariant check runs before gameplay. An active assault wins over
/// corrupt protection, and a stale run key never freezes a recycled run.
fn protected_presence_integrity_system(
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    assigned_runs: Res<AssignedStartLocations>,
    run_spawned: Res<RunSpawnedObjs>,
    zones: Res<SanctuaryZones>,
    crises: Res<SettlementCrisisState>,
    hero_query: HeroPresenceQuery,
    monolith_query: BoundMonolithQuery,
    mut presence: ResMut<PlayerWorldPresenceState>,
    mut telemetry: ResMut<SafeLogoutTelemetryState>,
) {
    let protected_players = presence
        .players
        .iter()
        .filter_map(|(player_id, record)| {
            (record.state == PlayerWorldPresence::OfflineProtected || record.resume_in_progress)
                .then_some(*player_id)
        })
        .collect::<Vec<_>>();

    for player_id in protected_players {
        let resume_fields_are_impossible = presence
            .players
            .get(&player_id)
            .map(protected_resume_fields_are_impossible)
            .unwrap_or(false);
        if resume_fields_are_impossible {
            recover_invalid_protection(
                player_id,
                clients.is_player_online(player_id),
                game_tick.0,
                "invalid_resume_fields",
                &mut presence,
                &mut telemetry,
            );
            continue;
        }

        let protected_since_is_valid = presence
            .players
            .get(&player_id)
            .and_then(|record| record.protected_since_tick)
            .map(|protected_since| protected_since <= game_tick.0)
            .unwrap_or(false);
        if !protected_since_is_valid {
            recover_invalid_protection(
                player_id,
                clients.is_player_online(player_id),
                game_tick.0,
                "invalid_protected_since_tick",
                &mut presence,
                &mut telemetry,
            );
            continue;
        }

        if crisis_is_assault_active(player_id, &crises) {
            recover_invalid_protection(
                player_id,
                clients.is_player_online(player_id),
                game_tick.0,
                "assault_active",
                &mut presence,
                &mut telemetry,
            );
            continue;
        }

        let current_key =
            resolve_hero(player_id, &ids, &entity_map, &hero_query).and_then(|hero| {
                (hero.alive
                    && !hero.true_death
                    && own_sanctuary_status(hero, &zones, &entity_map, &monolith_query)
                        == OwnSanctuaryStatus::Inside)
                    .then(|| protected_run_key(player_id, hero, &assigned_runs, &run_spawned))
                    .flatten()
            });
        let recorded_key = presence
            .players
            .get(&player_id)
            .and_then(|record| record.protected_run_key.clone());
        if current_key.is_none() || current_key != recorded_key {
            recover_invalid_protection(
                player_id,
                clients.is_player_online(player_id),
                game_tick.0,
                "run_key_mismatch",
                &mut presence,
                &mut telemetry,
            );
        }
    }

    let invalid_pending = presence
        .players
        .iter()
        .filter_map(|(player_id, record)| {
            (record.state == PlayerWorldPresence::SafeLogoutPending
                && (record.safe_logout_requested_tick.is_none()
                    || record
                        .safe_logout_requested_tick
                        .is_some_and(|started| started > game_tick.0)
                    || record.safe_logout_start_position.is_none()
                    || record.safe_logout_connection_ids.is_empty()
                    || record.protected_since_tick.is_some()
                    || record.protected_run_key.is_some()
                    || record.protection_exit_requested
                    || record.resume_in_progress
                    || record.resume_connection_id.is_some()
                    || record.resume_sync_ready))
                .then_some(*player_id)
        })
        .collect::<Vec<_>>();
    for player_id in invalid_pending {
        let connected = clients.is_player_online(player_id);
        if cancel_pending(
            player_id,
            SafeLogoutCancelReason::InvalidState,
            connected,
            game_tick.0,
            &mut presence,
            &mut telemetry,
        ) {
            telemetry
                .player_mut(player_id)
                .record_invariant("invalid_pending_fields");
            warn!(
                "safe_logout_invariant_recovered player_id={} game_tick={} reason=invalid_pending_fields",
                player_id, game_tick.0
            );
        }
    }
}

fn visible_event_target(event: &VisibleEvent) -> Option<i32> {
    match event {
        VisibleEvent::DamageEvent { target_id, .. }
        | VisibleEvent::StealEvent { target_id, .. }
        | VisibleEvent::BroadcastStealEvent { target_id, .. }
        | VisibleEvent::SpoilEvent { target_id, .. }
        | VisibleEvent::BroadcastSpoilEvent { target_id, .. }
        | VisibleEvent::TorchEvent { target_id, .. }
        | VisibleEvent::BroadcastTorchEvent { target_id, .. }
        | VisibleEvent::SpellDamageEvent { target_id, .. } => Some(*target_id),
        VisibleEvent::ActivateEvent { structure_id }
        | VisibleEvent::OperateEvent { structure_id }
        | VisibleEvent::RefineEvent { structure_id }
        | VisibleEvent::ExperimentEvent { structure_id }
        | VisibleEvent::PlantEvent { structure_id }
        | VisibleEvent::TendEvent { structure_id }
        | VisibleEvent::HarvestEvent { structure_id }
        | VisibleEvent::RepairEvent { structure_id } => Some(*structure_id),
        VisibleEvent::UseItemEvent { item_owner_id, .. } => Some(*item_owner_id),
        VisibleEvent::DrinkEvent { obj_id, .. }
        | VisibleEvent::EatEvent { obj_id, .. }
        | VisibleEvent::FindDrinkEvent { obj_id }
        | VisibleEvent::FindFoodEvent { obj_id }
        | VisibleEvent::FindShelterEvent { obj_id }
        | VisibleEvent::SleepEvent { obj_id }
        | VisibleEvent::FishingEvent { obj_id } => Some(*obj_id),
        VisibleEvent::InvestigateEvent { target_id } => Some(*target_id),
        VisibleEvent::SpellRaiseDeadEvent { corpse_id } => Some(*corpse_id),
        _ => None,
    }
}

fn is_hostile_destructive_event(event: &VisibleEvent) -> bool {
    matches!(
        event,
        VisibleEvent::DamageEvent { .. }
            | VisibleEvent::SpellDamageEvent { .. }
            | VisibleEvent::StealEvent { .. }
            | VisibleEvent::BroadcastStealEvent { .. }
            | VisibleEvent::SpoilEvent { .. }
            | VisibleEvent::BroadcastSpoilEvent { .. }
            | VisibleEvent::TorchEvent { .. }
            | VisibleEvent::BroadcastTorchEvent { .. }
    )
}

fn target_belongs_to_run(target_id: i32, player_id: i32, key: &ProtectedRunKey, ids: &Ids) -> bool {
    ids.get_player(target_id) == Some(player_id)
        || target_id == key.bound_monolith_id
        || key.run_object_ids.contains(&target_id)
}

/// Remove only unsafe already-queued mutations at the entry boundary. Owner
/// work/action events are retained and later rebased; global queues and other
/// players' events are left alone.
fn purge_unsafe_queued_events(
    player_id: i32,
    key: &ProtectedRunKey,
    ids: &Ids,
    map_events: &mut MapEvents,
) -> usize {
    let before = map_events.len();
    map_events.retain(|_, event| {
        let Some(target_id) = visible_event_target(&event.event_type) else {
            return true;
        };
        if !target_belongs_to_run(target_id, player_id, key, ids) {
            return true;
        }

        let actor_is_owner = ids.get_player(event.obj_id) == Some(player_id);
        !is_hostile_destructive_event(&event.event_type) && actor_is_owner
    });
    before.saturating_sub(map_events.len())
}

#[derive(Debug, Clone)]
struct ProtectionResume {
    player_id: i32,
    duration: i32,
    key: ProtectedRunKey,
    connection_id: Uuid,
    finalizing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumeCommitOutcome {
    AwaitingSync,
    Released,
    AuthorityChanged,
    Stale,
}

fn locked_client_is_active(
    clients: &HashMap<Uuid, Client>,
    client_id: Uuid,
    player_id: i32,
) -> bool {
    clients.get(&client_id).is_some_and(|client| {
        client.id == client_id && client.player_id == player_id && !client.sender.is_closed()
    })
}

fn locked_connection_is_current(
    clients: &HashMap<Uuid, Client>,
    player_id: i32,
    connection_id: Uuid,
) -> bool {
    locked_client_is_active(clients, connection_id, player_id)
        && !clients.iter().any(|(other_id, client)| {
            *other_id != connection_id
                && client.id == *other_id
                && client.player_id == player_id
                && !client.sender.is_closed()
        })
}

/// Commit one already-rebased protection interval. If connection authority
/// changed during the rebase walk, advance the interval boundary to `tick` but
/// keep the run protected so the same duration cannot be applied twice.
fn commit_rebased_resume_presence(
    record: &mut PlayerPresenceRecord,
    resume: &ProtectionResume,
    tick: i32,
    connection_is_current: bool,
    player_connected: bool,
) -> ResumeCommitOutcome {
    let expected_state = if resume.finalizing {
        record.state == PlayerWorldPresence::Online
            && record.resume_in_progress
            && record.resume_sync_ready
    } else {
        record.state == PlayerWorldPresence::OfflineProtected
            && !record.resume_in_progress
            && record.protection_exit_requested
    };
    if !expected_state || record.protected_run_key.as_ref() != Some(&resume.key) {
        return ResumeCommitOutcome::Stale;
    }

    if let Some(value) = record.last_combat_tick.as_mut() {
        rebase_tick(value, resume.duration);
    }
    if let Some(value) = record.last_damage_tick.as_mut() {
        rebase_tick(value, resume.duration);
    }
    record.last_protection_end_tick = Some(tick);
    record.cancel_reason = None;
    record.rejection_reason = None;

    if !connection_is_current {
        record.state = PlayerWorldPresence::OfflineProtected;
        record.client_connected = player_connected;
        record.protected_since_tick = Some(tick);
        record.protected_run_key = Some(resume.key.clone());
        record.protection_exit_requested = false;
        record.resume_in_progress = false;
        record.resume_connection_id = None;
        record.resume_sync_ready = false;
        record.resume_notice_connection_id = None;
        record.last_login_connection_id = None;
        return ResumeCommitOutcome::AuthorityChanged;
    }

    record.client_connected = true;
    if resume.finalizing {
        record.state = PlayerWorldPresence::Online;
        clear_protected_fields(record);
        record.resume_notice_connection_id = Some(resume.connection_id);
        ResumeCommitOutcome::Released
    } else {
        record.state = PlayerWorldPresence::Online;
        record.protected_since_tick = Some(tick);
        record.protected_run_key = Some(resume.key.clone());
        record.protection_exit_requested = false;
        record.resume_in_progress = true;
        record.resume_sync_ready = false;
        ResumeCommitOutcome::AwaitingSync
    }
}

fn rebase_tick(value: &mut i32, duration: i32) {
    *value = value.saturating_add(duration);
}

fn current_run_key_from_world(world: &mut World, player_id: i32) -> Option<ProtectedRunKey> {
    let hero_id = world.resource::<Ids>().get_hero(player_id)?;
    let hero_entity = world.resource::<EntityObjMap>().get_entity(hero_id)?;
    let id = world.get::<Id>(hero_entity)?;
    let owner = world.get::<PlayerId>(hero_entity)?;
    let state = world.get::<State>(hero_entity)?;
    let stats = world.get::<Stats>(hero_entity)?;
    if id.0 != hero_id
        || owner.0 != player_id
        || !owner.is_human()
        || !state.is_alive()
        || stats.hp <= 0
        || world.get::<StateDead>(hero_entity).is_some()
        || world.get::<TrueDeath>(hero_entity).is_some()
    {
        return None;
    }
    let bound_monolith_id = world.get::<BoundMonolith>(hero_entity)?.id;
    let start_location_name = world
        .resource::<AssignedStartLocations>()
        .get(&player_id)?
        .name
        .clone();
    let run_object_ids = protected_run_object_ids(player_id, world.resource::<RunSpawnedObjs>());
    Some(ProtectedRunKey {
        player_id,
        hero_id,
        start_location_name,
        bound_monolith_id,
        run_object_ids,
    })
}

fn protected_run_object_ids(player_id: i32, run_spawned: &RunSpawnedObjs) -> Vec<i32> {
    let mut object_ids = run_spawned.get(&player_id).cloned().unwrap_or_default();
    object_ids.sort_unstable();
    object_ids.dedup();
    object_ids
}

fn run_object_owner_map(world: &World) -> HashMap<i32, i32> {
    let mut owners = HashMap::new();
    for (player_id, object_ids) in world.resource::<RunSpawnedObjs>().iter() {
        for object_id in object_ids {
            owners.insert(*object_id, *player_id);
        }
    }
    owners
}

fn object_run_owner(
    object_id: i32,
    ordinary_owners: &HashMap<i32, i32>,
    run_owners: &HashMap<i32, i32>,
) -> Option<i32> {
    ordinary_owners
        .get(&object_id)
        .copied()
        .filter(|owner| PlayerId(*owner).is_human())
        .or_else(|| run_owners.get(&object_id).copied())
}

fn game_event_belongs_to_player(
    event: &GameEvent,
    player_id: i32,
    ordinary_owners: &HashMap<i32, i32>,
    run_owners: &HashMap<i32, i32>,
    entity_objects: &HashMap<Entity, i32>,
) -> bool {
    let owned = |object_id: i32| {
        object_run_owner(object_id, ordinary_owners, run_owners) == Some(player_id)
    };
    match &event.event_type {
        // Login and notices are connection/presentation work created at the
        // reconnect boundary, not protected-run simulation deadlines.
        GameEventType::Login { .. } | GameEventType::PlayerNotice { .. } => false,
        GameEventType::MerchantArrival {
            player_id: owner, ..
        }
        | GameEventType::MerchantLeavingSoon {
            player_id: owner, ..
        }
        | GameEventType::MerchantDeparture {
            player_id: owner, ..
        }
        | GameEventType::SpawnVillager {
            player_id: owner, ..
        }
        | GameEventType::AddEffectOnTile {
            player_id: owner, ..
        }
        | GameEventType::RemoveEffectOnTile {
            player_id: owner, ..
        } => *owner == player_id,
        GameEventType::ForageEvent { forager_id } => owned(*forager_id),
        GameEventType::GatherEvent { gatherer_id, .. } => owned(*gatherer_id),
        GameEventType::StructureGatherEvent {
            operator_id,
            structure_id,
        }
        | GameEventType::StructureOperateEvent {
            operator_id,
            structure_id,
        } => owned(*operator_id) || owned(*structure_id),
        GameEventType::RefineEvent { refiner_id, .. } => owned(*refiner_id),
        GameEventType::CraftEvent { crafter_id, .. } => owned(*crafter_id),
        GameEventType::StructureRefineEvent {
            refiner_id,
            structure_id,
            ..
        } => owned(*refiner_id) || owned(*structure_id),
        GameEventType::StructureCraftEvent {
            crafter_id,
            structure_id,
            ..
        } => owned(*crafter_id) || owned(*structure_id),
        GameEventType::ExperimentEvent {
            experimenter_id,
            structure_id,
        } => owned(*experimenter_id) || owned(*structure_id),
        GameEventType::UpdatePos { obj_id, .. }
        | GameEventType::DespawnObj { obj_id }
        | GameEventType::CancelRefineEvent { obj_id }
        | GameEventType::CancelAllMapEvents { obj_id }
        | GameEventType::CancelAllowedMapEvents { obj_id } => owned(*obj_id),
        GameEventType::NecroEvent {
            necromancer_id,
            mausoleum_id,
            ..
        } => necromancer_id.map(owned).unwrap_or(false) || mausoleum_id.map(owned).unwrap_or(false),
        GameEventType::RemoveEntity { entity } => entity_objects
            .get(entity)
            .copied()
            .map(owned)
            .unwrap_or(false),
        GameEventType::SpawnNPC { run_owner, .. } => *run_owner == Some(player_id),
        GameEventType::CancelMapEventsById { .. } => false,
    }
}

/// Ordered reconnect barrier. Every reconnect Update still observes
/// `OfflineProtected`; this exclusive PostUpdate system validates the exact run,
/// rebases its absolute deadlines, then publishes Online for the next Update.
fn rebase_and_resume_offline_protection_system(world: &mut World) {
    let tick = world.resource::<GameTick>().0;
    let requested = world
        .resource::<PlayerWorldPresenceState>()
        .players
        .iter()
        .filter_map(|(player_id, record)| {
            let phase = protection_resume_phase(record)?;
            Some((
                *player_id,
                record.protected_since_tick,
                record.protected_run_key.clone(),
                record.resume_connection_id,
                phase == ProtectionResumePhase::Finalizing,
            ))
        })
        .collect::<Vec<_>>();

    let mut resumes = Vec::new();
    for (player_id, protected_since_tick, recorded_key, connection_id, finalizing) in requested {
        let connection_is_current = connection_id.is_some_and(|connection_id| {
            world
                .resource::<Clients>()
                .has_active_connection_from(player_id, &[connection_id])
        });
        if !connection_is_current {
            let connected = world.resource::<Clients>().is_player_online(player_id);
            if let Some(record) = world
                .resource_mut::<PlayerWorldPresenceState>()
                .players
                .get_mut(&player_id)
            {
                record.client_connected = connected;
                record.protection_exit_requested = false;
                if record.resume_in_progress {
                    record.state = PlayerWorldPresence::OfflineProtected;
                    record.resume_in_progress = false;
                    record.resume_connection_id = None;
                    record.resume_sync_ready = false;
                }
            }
            continue;
        }

        let current_key = current_run_key_from_world(world, player_id);
        if current_key.is_none() || current_key != recorded_key {
            let connected = world.resource::<Clients>().is_player_online(player_id);
            world.resource_scope(|world, mut presence: Mut<PlayerWorldPresenceState>| {
                let mut telemetry = world.resource_mut::<SafeLogoutTelemetryState>();
                recover_invalid_protection(
                    player_id,
                    connected,
                    tick,
                    "resume_run_key_mismatch",
                    &mut presence,
                    &mut telemetry,
                );
            });
            continue;
        }
        let Some(key) = recorded_key else {
            continue;
        };
        let Some(connection_id) = connection_id else {
            continue;
        };
        let duration = tick.saturating_sub(protected_since_tick.unwrap_or(tick));
        resumes.push(ProtectionResume {
            player_id,
            duration,
            key,
            connection_id,
            finalizing,
        });
    }
    if resumes.is_empty() {
        return;
    }

    let durations = resumes
        .iter()
        .map(|resume| (resume.player_id, resume.duration))
        .collect::<HashMap<_, _>>();
    let ordinary_owners = world.resource::<Ids>().obj_player_map.clone();
    let run_owners = run_object_owner_map(world);
    let entity_objects = world
        .resource::<EntityObjMap>()
        .iter()
        .map(|(object_id, entity)| (*entity, *object_id))
        .collect::<HashMap<_, _>>();
    let protected_targets = resumes
        .iter()
        .map(|resume| (resume.player_id, resume.key.clone()))
        .collect::<Vec<_>>();
    let collector_owners = {
        let mut query = world.query::<(&Id, &TaxCollector)>();
        query
            .iter(world)
            .map(|(id, collector)| (id.0, collector.target_player))
            .collect::<HashMap<_, _>>()
    };
    let mut rebased_timers = HashMap::<i32, usize>::new();
    let mut discarded_events = HashMap::<i32, usize>::new();
    let mut count = |player_id: i32, amount: usize| {
        *rebased_timers.entry(player_id).or_default() += amount;
    };

    {
        let mut intro = world.resource_mut::<PlayerIntroState>();
        for resume in &resumes {
            if let Some(entry) = intro.get_mut(&resume.player_id) {
                rebase_tick(&mut entry.start_tick, resume.duration);
                count(resume.player_id, 1);
            }
        }
    }
    {
        let mut encounters = world.resource_mut::<InitialEncounterState>();
        for resume in &resumes {
            if let Some(entry) = encounters.get_mut(&resume.player_id) {
                rebase_tick(&mut entry.first_rat_spawn_tick, resume.duration);
                rebase_tick(&mut entry.second_rat_spawn_tick, resume.duration);
                rebase_tick(&mut entry.villager_ready_tick, resume.duration);
                rebase_tick(&mut entry.phase1_unlock_tick, resume.duration);
                rebase_tick(&mut entry.spider_unlock_tick, resume.duration);
                count(resume.player_id, 5);
            }
        }
    }
    {
        let mut crises = world.resource_mut::<SettlementCrisisState>();
        for resume in &resumes {
            if let Some(crisis) = crises.get_mut(&resume.player_id) {
                rebase_tick(&mut crisis.phase_started_tick, resume.duration);
                rebase_tick(&mut crisis.last_evaluated_tick, resume.duration);
                count(resume.player_id, 2);
            }
        }
    }
    {
        let mut scores = world.resource_mut::<RunScoreState>();
        for resume in &resumes {
            if let Some(score) = scores.get_mut(&resume.player_id) {
                rebase_tick(&mut score.start_tick, resume.duration);
                count(resume.player_id, 1);
            }
        }
    }
    {
        let mut legendary = world.resource_mut::<LegendaryThreatState>();
        for resume in &resumes {
            if let Some(threat) = legendary.get_mut(&resume.player_id) {
                if let Some(active_since_tick) = threat.active_since_tick.as_mut() {
                    rebase_tick(active_since_tick, resume.duration);
                    count(resume.player_id, 1);
                }
                if let Some(defeated_at_tick) = threat.defeated_at_tick.as_mut() {
                    // Preserve an already-completed active interval as a pair.
                    // Final True Death scoring subtracts these endpoints.
                    rebase_tick(defeated_at_tick, resume.duration);
                    count(resume.player_id, 1);
                }
                rebase_tick(&mut threat.next_follower_tick, resume.duration);
                count(resume.player_id, 1);
            }
        }
    }
    {
        let mut crops = world.resource_mut::<Crops>();
        for crop in crops.iter_mut().map(|(_, crop)| crop) {
            let Some(owner) = object_run_owner(crop.structure, &ordinary_owners, &run_owners)
            else {
                continue;
            };
            let Some(duration) = durations.get(&owner).copied() else {
                continue;
            };
            rebase_tick(&mut crop.stage_start, duration);
            if crop.stage != CropStages::Dead && crop.stage_end != i32::MAX {
                rebase_tick(&mut crop.stage_end, duration);
            }
            count(owner, 2);
        }
    }
    {
        let mut map_events = world.resource_mut::<MapEvents>();
        let mut remove = Vec::new();
        for (event_id, event) in map_events.iter_mut() {
            let destructive_target_owner =
                visible_event_target(&event.event_type).and_then(|target_id| {
                    protected_targets.iter().find_map(|(player_id, key)| {
                        (ordinary_owners.get(&target_id).copied() == Some(*player_id)
                            || target_id == key.bound_monolith_id
                            || key.run_object_ids.contains(&target_id))
                        .then_some(*player_id)
                    })
                });
            if is_hostile_destructive_event(&event.event_type) {
                if let Some(owner) = destructive_target_owner {
                    remove.push((*event_id, owner));
                    continue;
                }
            }
            let Some(owner) = object_run_owner(event.obj_id, &ordinary_owners, &run_owners)
                .or_else(|| collector_owners.get(&event.obj_id).copied())
            else {
                continue;
            };
            if let Some(duration) = durations.get(&owner).copied() {
                rebase_tick(&mut event.run_tick, duration);
                count(owner, 1);
            }
        }
        for (event_id, owner) in remove {
            map_events.remove(&event_id);
            *discarded_events.entry(owner).or_default() += 1;
        }
    }
    {
        let mut game_events = world.resource_mut::<GameEvents>();
        for event in game_events.values_mut() {
            for resume in &resumes {
                if game_event_belongs_to_player(
                    event,
                    resume.player_id,
                    &ordinary_owners,
                    &run_owners,
                    &entity_objects,
                ) {
                    rebase_tick(&mut event.start_tick, resume.duration);
                    rebase_tick(&mut event.run_tick, resume.duration);
                    count(resume.player_id, 2);
                    break;
                }
            }
        }
    }

    let protected_entities = {
        let mut query = world.query::<(Entity, &Id, &PlayerId)>();
        query
            .iter(world)
            .filter_map(|(entity, id, owner)| {
                let player_id = if owner.is_human() && durations.contains_key(&owner.0) {
                    Some(owner.0)
                } else {
                    resumes
                        .iter()
                        .find(|resume| {
                            resume.key.bound_monolith_id == id.0
                                || resume.key.run_object_ids.contains(&id.0)
                        })
                        .map(|resume| resume.player_id)
                }?;
                Some((entity, player_id))
            })
            .collect::<Vec<_>>()
    };
    for (entity, player_id) in protected_entities {
        let duration = durations[&player_id];
        let mut entity = world.entity_mut(entity);
        if let Some(mut value) = entity.get_mut::<BuildUpgradeState>() {
            if value.start_time != 0 {
                rebase_tick(&mut value.start_time, duration);
                count(player_id, 1);
            }
        }
        if let Some(mut inventory) = entity.get_mut::<Inventory>() {
            for item in &mut inventory.items {
                if item.start_time > 0 {
                    rebase_tick(&mut item.start_time, duration);
                    count(player_id, 1);
                }
            }
        }
        if let Some(mut value) = entity.get_mut::<Campfire>() {
            rebase_tick(&mut value.lit_at, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<Dehydrated>() {
            rebase_tick(&mut value.at_tick, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<Starving>() {
            rebase_tick(&mut value.at_tick, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<Exhausted>() {
            rebase_tick(&mut value.at_tick, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<NoDrinks>() {
            rebase_tick(&mut value.at_tick, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<NoFood>() {
            rebase_tick(&mut value.at_tick, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<Idle>() {
            rebase_tick(&mut value.start_time, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<LastCombatTick>() {
            rebase_tick(&mut value.0, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<LastDamageTick>() {
            rebase_tick(&mut value.0, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<LastAttacker>() {
            rebase_tick(&mut value.tick, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<StateDead>() {
            rebase_tick(&mut value.dead_at, duration);
            count(player_id, 1);
        }
        if let Some(mut value) = entity.get_mut::<Burning>() {
            rebase_tick(&mut value.start_tick, duration);
            count(player_id, 1);
        }
    }

    let protected_collectors = {
        let mut query = world.query::<(Entity, &TaxCollector)>();
        query
            .iter(world)
            .filter_map(|(entity, collector)| {
                durations
                    .contains_key(&collector.target_player)
                    .then_some((entity, collector.target_player))
            })
            .collect::<Vec<_>>()
    };
    for (entity, player_id) in protected_collectors {
        let duration = durations[&player_id];
        let mut entity = world.entity_mut(entity);
        if let Some(mut collector) = entity.get_mut::<TaxCollector>() {
            rebase_tick(&mut collector.last_collection_time, duration);
            rebase_tick(&mut collector.last_demand_time, duration);
            count(player_id, 2);
        }
        if let Some(mut idle) = entity.get_mut::<Idle>() {
            rebase_tick(&mut idle.start_time, duration);
            count(player_id, 1);
        }
    }

    // Hold the same registry mutex used by activation/removal while committing
    // presence. A replacement after this guard is released is ordered after a
    // successful resume; a replacement before it cannot release protection.
    let clients_resource = world.resource::<Clients>().clone();
    let clients_guard = clients_resource
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut telemetry_updates = Vec::new();
    let mut presence = world.resource_mut::<PlayerWorldPresenceState>();
    for resume in resumes {
        let Some(record) = presence.players.get_mut(&resume.player_id) else {
            continue;
        };
        let previous = record.state;
        let timer_count = rebased_timers.get(&resume.player_id).copied().unwrap_or(0);
        let connection_is_current =
            locked_connection_is_current(&clients_guard, resume.player_id, resume.connection_id);
        let player_connected = clients_guard
            .keys()
            .copied()
            .any(|client_id| locked_client_is_active(&clients_guard, client_id, resume.player_id));
        let outcome = commit_rebased_resume_presence(
            record,
            &resume,
            tick,
            connection_is_current,
            player_connected,
        );
        if outcome == ResumeCommitOutcome::Stale {
            continue;
        }
        telemetry_updates.push((
            resume.player_id,
            resume.duration,
            outcome == ResumeCommitOutcome::Released,
            outcome == ResumeCommitOutcome::AuthorityChanged,
        ));
        match outcome {
            ResumeCommitOutcome::Released => info!(
                "safe_logout_protection_resumed player_id={} previous_presence={} new_presence={} game_tick={} protected_duration={} rebased_timers={}",
                resume.player_id,
                previous.as_str(),
                record.state.as_str(),
                tick,
                resume.duration,
                timer_count
            ),
            ResumeCommitOutcome::AwaitingSync => info!(
                "safe_logout_resume_rebased player_id={} previous_presence={} new_presence={} game_tick={} protected_duration={} rebased_timers={} awaiting_sync=true",
                resume.player_id,
                previous.as_str(),
                record.state.as_str(),
                tick,
                resume.duration,
                timer_count
            ),
            ResumeCommitOutcome::AuthorityChanged => info!(
                "safe_logout_resume_authority_changed player_id={} previous_presence={} new_presence={} game_tick={} protected_duration={} rebased_timers={} replacement_connected={}",
                resume.player_id,
                previous.as_str(),
                record.state.as_str(),
                tick,
                resume.duration,
                timer_count,
                player_connected
            ),
            ResumeCommitOutcome::Stale => unreachable!(),
        }
    }
    drop(presence);
    drop(clients_guard);

    let mut telemetry = world.resource_mut::<SafeLogoutTelemetryState>();
    for (player_id, duration, released, authority_changed) in telemetry_updates {
        let telemetry = telemetry.player_mut(player_id);
        telemetry.protected_ticks_total = telemetry
            .protected_ticks_total
            .saturating_add(duration.max(0) as u64);
        if authority_changed {
            telemetry.stale_connection_events_rejected =
                telemetry.stale_connection_events_rejected.saturating_add(1);
        }
        if released {
            // One logical reconnect rebase is reported when its sync barrier
            // releases. The implementation may shift a short follow-up
            // protected interval, but never counts the same resume twice.
            telemetry.timer_rebases = telemetry.timer_rebases.saturating_add(1);
            telemetry.resumed = telemetry.resumed.saturating_add(1);
        }
    }
    for (player_id, discarded) in discarded_events {
        telemetry.record_protected_queued_events_discarded(player_id, discarded);
    }
}

pub struct SafeLogoutPlugin;

impl Plugin for SafeLogoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerWorldPresenceState>()
            .init_resource::<SafeLogoutTelemetryState>()
            .init_resource::<SafeLogoutStatusDeliveryState>()
            .add_message::<RequestSafeLogout>()
            .add_message::<CancelSafeLogout>()
            .add_systems(
                First,
                protected_presence_integrity_system.run_if(in_state(AppState::Running)),
            )
            .add_systems(
                PostUpdate,
                (
                    reconcile_player_world_presence_system,
                    safe_logout_request_system,
                    safe_logout_manual_cancel_system,
                    safe_logout_pending_system,
                    rebase_and_resume_offline_protection_system,
                    safe_logout_status_delivery_system,
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
    telemetry: &mut SafeLogoutTelemetryState,
) {
    let previous = presence.players.get(&player_id).map(|record| record.state);
    let record = PlayerPresenceRecord::new(connected);
    let next = record.state;
    presence.players.insert(player_id, record);
    telemetry.insert(player_id, SafeLogoutTelemetry::default());
    info!(
        "safe_logout_run_initialized player_id={} previous_presence={} new_presence={} game_tick={}",
        player_id,
        previous.map(PlayerWorldPresence::as_str).unwrap_or("none"),
        next.as_str(),
        tick
    );
}

pub fn mark_player_logged_in(
    player_id: i32,
    connection_id: Uuid,
    tick: i32,
    presence: &mut PlayerWorldPresenceState,
    telemetry: &mut SafeLogoutTelemetryState,
) -> bool {
    let Some(snapshot) = presence.players.get(&player_id).cloned() else {
        return false;
    };
    let previous = snapshot.state;
    if snapshot.last_login_connection_id == Some(connection_id)
        && (previous != PlayerWorldPresence::OfflineProtected
            || snapshot.protection_exit_requested
            || snapshot.resume_in_progress)
    {
        return false;
    }
    if previous == PlayerWorldPresence::SafeLogoutPending {
        // Login is the authenticated reconnect boundary. Conservatively cancel
        // an in-flight handoff even if the socket gap occurred between ECS
        // evaluations and was therefore not visible to reconciliation.
        cancel_pending(
            player_id,
            SafeLogoutCancelReason::Disconnected,
            true,
            tick,
            presence,
            telemetry,
        );
        if let Some(record) = presence.players.get_mut(&player_id) {
            record.last_login_connection_id = Some(connection_id);
        }
        return true;
    }
    if previous == PlayerWorldPresence::OfflineProtected {
        let record = presence
            .players
            .get_mut(&player_id)
            .expect("presence snapshot still exists");
        record.client_connected = true;
        let changed_connection = record.resume_connection_id != Some(connection_id);
        record.resume_connection_id = Some(connection_id);
        record.resume_sync_ready = false;
        record.last_login_connection_id = Some(connection_id);
        if !record.protection_exit_requested || changed_connection {
            record.protection_exit_requested = true;
            info!(
                "safe_logout_resume_requested player_id={} presence={} game_tick={} source=login",
                player_id,
                previous.as_str(),
                tick
            );
        }
        return true;
    }
    if snapshot.resume_in_progress {
        let record = presence
            .players
            .get_mut(&player_id)
            .expect("presence snapshot still exists");
        if record.resume_connection_id == Some(connection_id) {
            return false;
        }
        // A replacement during the sync barrier starts a new protected
        // interval at the prior rebase boundary. Already-rebased time is never
        // applied twice; only the new interval is considered on this socket.
        record.state = PlayerWorldPresence::OfflineProtected;
        record.client_connected = true;
        record.resume_in_progress = false;
        record.resume_sync_ready = false;
        record.resume_connection_id = Some(connection_id);
        record.protection_exit_requested = true;
        record.last_login_connection_id = Some(connection_id);
        info!(
            "safe_logout_resume_replaced player_id={} previous_presence={} new_presence={} game_tick={}",
            player_id,
            previous.as_str(),
            record.state.as_str(),
            tick
        );
        return true;
    }
    let record = presence
        .players
        .get_mut(&player_id)
        .expect("presence snapshot still exists");
    if record
        .resume_notice_connection_id
        .is_some_and(|resume_connection_id| resume_connection_id != connection_id)
    {
        record.resume_notice_connection_id = None;
    }
    record.client_connected = true;
    record.state = PlayerWorldPresence::Online;
    record.last_login_connection_id = Some(connection_id);
    if previous != record.state {
        clear_pending_fields(record);
        clear_protected_fields(record);
        record.resume_notice_connection_id = None;
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
    true
}

/// Mark the exact authoritative reconnect's current-run synchronization ready.
/// Final release remains in the ordered PostUpdate rebase barrier.
pub fn mark_player_login_sync_complete(
    player_id: i32,
    connection_id: Uuid,
    tick: i32,
    presence: &mut PlayerWorldPresenceState,
) -> bool {
    let Some(record) = presence.players.get_mut(&player_id) else {
        return false;
    };
    if record.state != PlayerWorldPresence::Online
        || !record.resume_in_progress
        || record.resume_connection_id != Some(connection_id)
        || record.resume_sync_ready
    {
        return false;
    }
    record.resume_sync_ready = true;
    info!(
        "safe_logout_resume_sync_ready player_id={} presence={} game_tick={}",
        player_id,
        record.state.as_str(),
        tick
    );
    true
}

pub fn remove_player_presence_for_run_cleanup(
    player_id: i32,
    tick: i32,
    presence: &mut PlayerWorldPresenceState,
    telemetry: &mut SafeLogoutTelemetryState,
) {
    let Some(record) = presence.players.remove(&player_id) else {
        return;
    };
    if record.state == PlayerWorldPresence::SafeLogoutPending {
        telemetry
            .player_mut(player_id)
            .record_cancellation(SafeLogoutCancelReason::RunEnded);
        info!(
            "safe_logout_countdown_cancelled player_id={} previous_presence={} new_presence=removed game_tick={} reason={}",
            player_id,
            record.state.as_str(),
            tick,
            SafeLogoutCancelReason::RunEnded.as_str()
        );
    }
    if record.state == PlayerWorldPresence::OfflineProtected || record.resume_in_progress {
        close_protected_interval(&record, tick, telemetry.player_mut(player_id));
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
        id: id.0,
        pos: *pos,
        alive: state.is_alive() && state_dead.is_none() && true_death.is_none() && stats.hp > 0,
        true_death: true_death.is_some(),
        hp: stats.hp,
        last_combat_tick: combat_tick.0,
        last_damage_tick: damage_tick.map(|tick| tick.0),
        bound_monolith: bound_monolith.map(|bound| (bound.id, bound.pos)),
    })
}

fn protected_run_key(
    player_id: i32,
    hero: HeroPresenceSnapshot,
    assigned_runs: &AssignedStartLocations,
    run_spawned: &RunSpawnedObjs,
) -> Option<ProtectedRunKey> {
    let start_location = assigned_runs.get(&player_id)?;
    let (bound_monolith_id, _) = hero.bound_monolith?;
    Some(ProtectedRunKey {
        player_id,
        hero_id: hero.id,
        start_location_name: start_location.name.clone(),
        bound_monolith_id,
        run_object_ids: protected_run_object_ids(player_id, run_spawned),
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
                // `State::Hiding` is deliberately absent from every perception
                // path and NPC actions keep it inert until a reveal transition.
                // Treating it as an immediate threat made every fresh run
                // ineligible because the later intro Necromancer is pre-spawned
                // hidden inside this radius. A reveal is observed on the next
                // eligibility/countdown pass and blocks or cancels as normal.
                || !state.is_visible()
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
    telemetry: &mut SafeLogoutTelemetryState,
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
    clear_pending_fields(record);
    clear_protected_fields(record);
    record.resume_notice_connection_id = None;
    record.cancel_reason = Some(reason);
    record.rejection_reason = None;
    telemetry.player_mut(player_id).record_cancellation(reason);
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

fn cancel_pending_with_current_connection(
    player_id: i32,
    reason: SafeLogoutCancelReason,
    clients: &Clients,
    tick: i32,
    presence: &mut PlayerWorldPresenceState,
    telemetry: &mut SafeLogoutTelemetryState,
) -> bool {
    cancel_pending(
        player_id,
        reason,
        clients.is_player_online(player_id),
        tick,
        presence,
        telemetry,
    )
}

/// Complete a pending handoff against two authoritative connection samples.
///
/// The second sample is deliberately taken after the provisional ECS state
/// write. No other ECS system can observe that write while this system owns the
/// resource, so lost request-connection continuity at either sample cancels
/// according to freshly sampled current presence without ever publishing or
/// logging protection. A disconnect after the second sample is ordered after
/// the completion boundary.
fn complete_pending_with_connection_check(
    player_id: i32,
    tick: i32,
    run_key: ProtectedRunKey,
    presence: &mut PlayerWorldPresenceState,
    telemetry: &mut SafeLogoutTelemetryState,
    mut has_request_connection: impl FnMut() -> bool,
    mut is_currently_connected: impl FnMut() -> bool,
) -> SafeLogoutCompletionOutcome {
    if !has_request_connection() {
        cancel_pending(
            player_id,
            SafeLogoutCancelReason::Disconnected,
            is_currently_connected(),
            tick,
            presence,
            telemetry,
        );
        return SafeLogoutCompletionOutcome::Cancelled;
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
    record.protected_since_tick = Some(tick);
    record.protected_run_key = Some(run_key);
    record.protection_exit_requested = false;
    record.safe_logout_requested_tick = None;
    record.safe_logout_start_position = None;
    record.safe_logout_connection_ids.clear();
    record.cancel_reason = None;
    record.rejection_reason = None;
    if !has_request_connection() {
        let connected = is_currently_connected();
        record.state = if connected {
            PlayerWorldPresence::Online
        } else {
            PlayerWorldPresence::Disconnected
        };
        record.client_connected = connected;
        clear_protected_fields(record);
        record.cancel_reason = Some(SafeLogoutCancelReason::Disconnected);
        telemetry
            .player_mut(player_id)
            .record_cancellation(SafeLogoutCancelReason::Disconnected);
        info!(
            "safe_logout_countdown_cancelled player_id={} previous_presence={} new_presence={} game_tick={} reason={}",
            player_id,
            previous.as_str(),
            record.state.as_str(),
            tick,
            SafeLogoutCancelReason::Disconnected.as_str()
        );
        return SafeLogoutCompletionOutcome::Cancelled;
    }

    let telemetry = telemetry.player_mut(player_id);
    telemetry.completed = telemetry.completed.saturating_add(1);
    telemetry.protected_sessions_started = telemetry.protected_sessions_started.saturating_add(1);

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
    crises: Res<SettlementCrisisState>,
    hero_query: HeroPresenceQuery,
    mut presence: ResMut<PlayerWorldPresenceState>,
    mut telemetry: ResMut<SafeLogoutTelemetryState>,
) {
    let mut player_ids = presence.players.keys().copied().collect::<HashSet<_>>();
    player_ids.extend(ids.player_hero_map.keys().copied());
    player_ids.extend(assigned_runs.keys().copied());

    for player_id in player_ids {
        let valid_run = assigned_runs.contains_key(&player_id);
        let hero = resolve_hero(player_id, &ids, &entity_map, &hero_query);
        if !valid_run || hero.is_none() {
            remove_player_presence_for_run_cleanup(
                player_id,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        }
        let hero = hero.expect("checked above");
        let connected = clients.is_player_online(player_id);
        if !presence.players.contains_key(&player_id) {
            initialize_player_presence(
                player_id,
                connected,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
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
                if cancel_pending(
                    player_id,
                    SafeLogoutCancelReason::Disconnected,
                    false,
                    game_tick.0,
                    &mut presence,
                    &mut telemetry,
                ) {
                    let player_telemetry = telemetry.player_mut(player_id);
                    player_telemetry.ordinary_disconnects =
                        player_telemetry.ordinary_disconnects.saturating_add(1);
                    if crisis_is_assault_active(player_id, &crises) {
                        player_telemetry.active_assault_disconnects = player_telemetry
                            .active_assault_disconnects
                            .saturating_add(1);
                    }
                }
            } else if state == Some(PlayerWorldPresence::Online) {
                if let Some(record) = presence.players.get_mut(&player_id) {
                    if record.resume_in_progress {
                        // A socket lost during the synchronization barrier has
                        // never resumed owner simulation. Preserve the run key
                        // and current protected interval for the replacement.
                        let previous = record.state;
                        record.state = PlayerWorldPresence::OfflineProtected;
                        record.client_connected = false;
                        record.protection_exit_requested = false;
                        record.resume_in_progress = false;
                        record.resume_connection_id = None;
                        record.resume_sync_ready = false;
                        record.last_login_connection_id = None;
                        info!(
                            "safe_logout_resume_interrupted player_id={} previous_presence={} new_presence={} game_tick={}",
                            player_id,
                            previous.as_str(),
                            record.state.as_str(),
                            game_tick.0
                        );
                        continue;
                    }
                    let previous = record.state;
                    record.state = PlayerWorldPresence::Disconnected;
                    record.client_connected = false;
                    let player_telemetry = telemetry.player_mut(player_id);
                    player_telemetry.ordinary_disconnects =
                        player_telemetry.ordinary_disconnects.saturating_add(1);
                    if crisis_is_assault_active(player_id, &crises) {
                        player_telemetry.active_assault_disconnects = player_telemetry
                            .active_assault_disconnects
                            .saturating_add(1);
                    }
                    info!(
                        "safe_logout_ordinary_disconnect player_id={} previous_presence={} new_presence={} game_tick={}",
                        player_id,
                        previous.as_str(),
                        record.state.as_str(),
                        game_tick.0
                    );
                }
            } else if let Some(record) = presence.players.get_mut(&player_id) {
                let was_connected = record.client_connected;
                record.client_connected = false;
                if was_connected && record.state == PlayerWorldPresence::OfflineProtected {
                    info!(
                        "safe_logout_protected_disconnect player_id={} presence={} game_tick={}",
                        player_id,
                        record.state.as_str(),
                        game_tick.0
                    );
                }
            }
            continue;
        }

        if let Some(record) = presence.players.get_mut(&player_id) {
            record.client_connected = true;
            // Only the exact authenticated Login event may request release
            // from OfflineProtected. A raw connection edge is insufficient.
            if record.state == PlayerWorldPresence::Disconnected {
                let previous = record.state;
                record.state = PlayerWorldPresence::Online;
                clear_pending_fields(record);
                clear_protected_fields(record);
                record.resume_notice_connection_id = None;
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
    active_connection_ids: &[Uuid],
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
    if record.resume_in_progress {
        return Some(SafeLogoutRejectionReason::AlreadyProtected);
    }
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
    if active_connection_ids.is_empty() {
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

fn safe_logout_eligibility(
    player_id: i32,
    tick: i32,
    active_connection_ids: &[Uuid],
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
) -> SafeLogoutEligibility {
    let active_assault = crisis_is_assault_active(player_id, crises);
    let in_own_sanctuary = resolve_hero(player_id, ids, entity_map, hero_query)
        .map(|hero| {
            own_sanctuary_status(hero, zones, entity_map, monolith_query)
                == OwnSanctuaryStatus::Inside
        })
        .unwrap_or(false);
    let reason = request_rejection(
        player_id,
        tick,
        active_connection_ids,
        ids,
        entity_map,
        assigned_runs,
        zones,
        crises,
        templates,
        hero_query,
        monolith_query,
        hostile_query,
        record,
    );

    SafeLogoutEligibility {
        eligible: reason.is_none(),
        reason,
        in_own_sanctuary,
        active_assault,
    }
}

fn rejection_status(reason: SafeLogoutRejectionReason) -> (&'static str, &'static str) {
    match reason {
        SafeLogoutRejectionReason::NotOnline => (
            "unknown",
            "Safe Logout requires an active connection.",
        ),
        SafeLogoutRejectionReason::InvalidRun => (
            "run_invalid",
            "Safe Logout is unavailable until a valid run is active.",
        ),
        SafeLogoutRejectionReason::MissingHero => (
            "hero_invalid",
            "Safe Logout is unavailable because your hero could not be verified.",
        ),
        SafeLogoutRejectionReason::HeroDied => (
            "hero_dead",
            "Safe Logout is unavailable while your hero is dead.",
        ),
        SafeLogoutRejectionReason::TrueDeath => (
            "true_death",
            "Safe Logout is unavailable after True Death.",
        ),
        SafeLogoutRejectionReason::MissingBoundMonolith
        | SafeLogoutRejectionReason::MissingSanctuaryZone
        | SafeLogoutRejectionReason::SanctuaryInvalid => (
            "sanctuary_invalid",
            "Your sanctuary could not be verified for Safe Logout.",
        ),
        SafeLogoutRejectionReason::OutsideOwnSanctuary => (
            "outside_sanctuary",
            "Return to your own sanctuary to use Safe Logout.",
        ),
        SafeLogoutRejectionReason::AssaultActive => (
            "assault_active",
            "Safe Logout is unavailable during an active assault. Disconnecting will not stop the assault.",
        ),
        SafeLogoutRejectionReason::RecentCombat => (
            "recent_combat",
            "Wait until you have been out of combat.",
        ),
        SafeLogoutRejectionReason::RecentDamage => (
            "recent_damage",
            "Wait until you have stopped taking damage.",
        ),
        SafeLogoutRejectionReason::HostileNearby => (
            "hostile_nearby",
            "Safe Logout is unavailable while enemies are nearby.",
        ),
        SafeLogoutRejectionReason::AlreadyPending => (
            "already_pending",
            "Safe Logout is already in progress.",
        ),
        SafeLogoutRejectionReason::AlreadyProtected => (
            "already_protected",
            "Your settlement is already protected.",
        ),
    }
}

fn cancellation_status(reason: SafeLogoutCancelReason) -> (&'static str, &'static str) {
    match reason {
        SafeLogoutCancelReason::Moved => (
            "moved",
            "Safe Logout was cancelled because you moved.",
        ),
        SafeLogoutCancelReason::EnteredCombat => (
            "entered_combat",
            "Safe Logout was cancelled because you entered combat.",
        ),
        SafeLogoutCancelReason::TookDamage => (
            "took_damage",
            "Safe Logout was cancelled because you took damage.",
        ),
        SafeLogoutCancelReason::HostileNearby => (
            "hostile_nearby",
            "Safe Logout was cancelled because an enemy came nearby.",
        ),
        SafeLogoutCancelReason::LeftSanctuary => (
            "left_sanctuary",
            "Safe Logout was cancelled because you left your sanctuary.",
        ),
        SafeLogoutCancelReason::SanctuaryInvalid => (
            "sanctuary_invalid",
            "Safe Logout was cancelled because your sanctuary could not be verified.",
        ),
        SafeLogoutCancelReason::AssaultStarted => (
            "assault_started",
            "Safe Logout was cancelled because an assault started. Disconnecting will not stop the assault.",
        ),
        SafeLogoutCancelReason::HeroDied => (
            "hero_died",
            "Safe Logout was cancelled because your hero died.",
        ),
        SafeLogoutCancelReason::Disconnected => (
            "disconnected_before_completion",
            "The connection closed before Safe Logout completed. Your settlement is not protected.",
        ),
        SafeLogoutCancelReason::Manual => (
            "manually_cancelled",
            "Safe Logout was cancelled.",
        ),
        SafeLogoutCancelReason::RunEnded => (
            "run_ended",
            "Safe Logout was cancelled because the run ended.",
        ),
        SafeLogoutCancelReason::InvalidState => (
            "invalid_state",
            "Safe Logout was cancelled because its server state could not be verified.",
        ),
    }
}

fn cancellation_matches_rejection(
    cancellation: SafeLogoutCancelReason,
    rejection: SafeLogoutRejectionReason,
) -> bool {
    matches!(
        (cancellation, rejection),
        (
            SafeLogoutCancelReason::EnteredCombat,
            SafeLogoutRejectionReason::RecentCombat
        ) | (
            SafeLogoutCancelReason::TookDamage,
            SafeLogoutRejectionReason::RecentDamage
        ) | (
            SafeLogoutCancelReason::HostileNearby,
            SafeLogoutRejectionReason::HostileNearby
        ) | (
            SafeLogoutCancelReason::LeftSanctuary,
            SafeLogoutRejectionReason::OutsideOwnSanctuary
        ) | (
            SafeLogoutCancelReason::SanctuaryInvalid,
            SafeLogoutRejectionReason::MissingBoundMonolith
                | SafeLogoutRejectionReason::MissingSanctuaryZone
                | SafeLogoutRejectionReason::SanctuaryInvalid
        ) | (
            SafeLogoutCancelReason::AssaultStarted,
            SafeLogoutRejectionReason::AssaultActive
        ) | (
            SafeLogoutCancelReason::HeroDied,
            SafeLogoutRejectionReason::HeroDied | SafeLogoutRejectionReason::TrueDeath
        ) | (
            SafeLogoutCancelReason::Disconnected,
            SafeLogoutRejectionReason::NotOnline
        ) | (
            SafeLogoutCancelReason::RunEnded,
            SafeLogoutRejectionReason::InvalidRun
        )
    )
}

fn build_safe_logout_status(
    record: Option<&PlayerPresenceRecord>,
    tick: i32,
    eligibility: SafeLogoutEligibility,
    connection_id: Option<Uuid>,
) -> SafeLogoutStatusSnapshot {
    let state = record
        .map(|record| record.state)
        .unwrap_or(PlayerWorldPresence::Disconnected);
    let total_seconds = SAFE_LOGOUT_COUNTDOWN_TICKS / TICKS_PER_SEC;
    let (countdown_total_seconds, countdown_remaining_seconds) = match state {
        PlayerWorldPresence::SafeLogoutPending => {
            let elapsed = record
                .and_then(|record| record.safe_logout_requested_tick)
                .map(|started| tick.saturating_sub(started).max(0))
                .unwrap_or(0);
            let remaining_ticks = SAFE_LOGOUT_COUNTDOWN_TICKS.saturating_sub(elapsed).max(0);
            (
                Some(total_seconds),
                Some((remaining_ticks + TICKS_PER_SEC - 1) / TICKS_PER_SEC),
            )
        }
        PlayerWorldPresence::OfflineProtected => (Some(total_seconds), Some(0)),
        PlayerWorldPresence::Online | PlayerWorldPresence::Disconnected => (None, None),
    };

    let display_reason = match state {
        PlayerWorldPresence::Online => match (
            record.and_then(|record| record.cancel_reason),
            eligibility.reason,
        ) {
            (Some(cancellation), None) => Some(cancellation_status(cancellation)),
            (Some(cancellation), Some(rejection))
                if cancellation_matches_rejection(cancellation, rejection) =>
            {
                Some(cancellation_status(cancellation))
            }
            (_, Some(rejection)) => Some(rejection_status(rejection)),
            (None, None) => None,
        },
        PlayerWorldPresence::Disconnected => record
            .and_then(|record| record.cancel_reason)
            .map(cancellation_status)
            .or_else(|| eligibility.reason.map(rejection_status)),
        PlayerWorldPresence::SafeLogoutPending | PlayerWorldPresence::OfflineProtected => None,
    };

    let message = match state {
        PlayerWorldPresence::SafeLogoutPending => {
            "Remain still and avoid combat until Safe Logout completes."
        }
        PlayerWorldPresence::OfflineProtected => {
            "Your settlement is protected. It is now safe to leave."
        }
        PlayerWorldPresence::Online if display_reason.is_none() => {
            "You can safely end your session from this sanctuary."
        }
        PlayerWorldPresence::Disconnected if display_reason.is_none() => {
            "Safe Logout requires an active connection."
        }
        _ => display_reason
            .map(|(_, message)| message)
            .unwrap_or("Safe Logout is unavailable."),
    };

    SafeLogoutStatusSnapshot {
        version: SAFE_LOGOUT_STATUS_VERSION,
        state: match state {
            PlayerWorldPresence::Online => "online",
            PlayerWorldPresence::SafeLogoutPending => "pending",
            PlayerWorldPresence::OfflineProtected => "protected",
            PlayerWorldPresence::Disconnected => "disconnected",
        }
        .to_string(),
        can_request: state == PlayerWorldPresence::Online && eligibility.eligible,
        can_cancel: state == PlayerWorldPresence::SafeLogoutPending,
        countdown_total_seconds,
        countdown_remaining_seconds,
        reason: display_reason.map(|(reason, _)| reason.to_string()),
        message: message.to_string(),
        in_own_sanctuary: eligibility.in_own_sanctuary,
        active_assault: eligibility.active_assault,
        protected: state == PlayerWorldPresence::OfflineProtected,
        resumed_from_protection: record
            .map(|record| {
                connection_id.is_some()
                    && record.resume_notice_connection_id == connection_id
                    && !record.resume_in_progress
            })
            .unwrap_or(false),
    }
}

fn should_send_safe_logout_status(
    previous: Option<&SentSafeLogoutStatus>,
    player_id: i32,
    status: &SafeLogoutStatusSnapshot,
) -> bool {
    previous
        .map(|sent| {
            if sent.player_id != player_id || sent.status == *status {
                return sent.player_id != player_id;
            }
            // The acknowledgement is a one-shot pulse. Clearing only that bit
            // after a successful send must not produce a second announcement
            // packet; any other semantic status change remains deliverable.
            let mut previous_without_resume = sent.status.clone();
            previous_without_resume.resumed_from_protection = false;
            previous_without_resume != *status
        })
        .unwrap_or(true)
}

fn locked_current_connection_id(clients: &HashMap<Uuid, Client>, player_id: i32) -> Option<Uuid> {
    let mut active = clients.iter().filter_map(|(client_id, _client)| {
        locked_client_is_active(clients, *client_id, player_id).then_some(*client_id)
    });
    let connection_id = active.next()?;
    active.next().is_none().then_some(connection_id)
}

/// Queue one status only while `connection_id` is still the player's sole
/// authoritative connection. The registry lock stays held through both the
/// non-blocking enqueue and one-shot resume-pulse consumption, so activation of
/// a replacement is ordered entirely before or entirely after this operation.
fn try_send_safe_logout_status_to_current_connection(
    clients: &Clients,
    player_id: i32,
    connection_id: Uuid,
    serialized: String,
    consumes_resume_notice: bool,
    presence: &mut PlayerWorldPresenceState,
) -> SafeLogoutStatusSendOutcome {
    let Ok(clients) = clients.lock() else {
        return SafeLogoutStatusSendOutcome::RegistryUnavailable;
    };

    if !locked_connection_is_current(&clients, player_id, connection_id) {
        // If replacement won the registry race, preserve the one-shot pulse for
        // that exact new authority. The stale socket neither receives nor
        // consumes it, and the next delivery pass will build a fresh snapshot.
        if consumes_resume_notice {
            if let Some(replacement_connection_id) =
                locked_current_connection_id(&clients, player_id)
            {
                if let Some(record) = presence.players.get_mut(&player_id) {
                    if record.resume_notice_connection_id == Some(connection_id)
                        && !record.resume_in_progress
                    {
                        record.resume_notice_connection_id = Some(replacement_connection_id);
                    }
                }
            }
        }
        return SafeLogoutStatusSendOutcome::StaleConnection;
    }

    let Some(client) = clients.get(&connection_id) else {
        return SafeLogoutStatusSendOutcome::StaleConnection;
    };
    match client.sender.try_send(serialized) {
        Ok(()) => {
            if consumes_resume_notice {
                if let Some(record) = presence.players.get_mut(&player_id) {
                    if record.resume_notice_connection_id == Some(connection_id)
                        && !record.resume_in_progress
                    {
                        record.resume_notice_connection_id = None;
                    }
                }
            }
            SafeLogoutStatusSendOutcome::Sent
        }
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            SafeLogoutStatusSendOutcome::ChannelFull
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            SafeLogoutStatusSendOutcome::ChannelClosed
        }
    }
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
    mut telemetry: ResMut<SafeLogoutTelemetryState>,
) {
    for request in requests.read() {
        let player_telemetry = telemetry.player_mut(request.player_id);
        player_telemetry.requests = player_telemetry.requests.saturating_add(1);
        if !clients.is_current_connection(request.player_id, request.connection_id) {
            player_telemetry.record_rejection(SafeLogoutRejectionReason::NotOnline);
            player_telemetry.stale_connection_events_rejected = player_telemetry
                .stale_connection_events_rejected
                .saturating_add(1);
            info!(
                "safe_logout_request_rejected player_id={} game_tick={} reason={} source=stale_connection",
                request.player_id,
                game_tick.0,
                SafeLogoutRejectionReason::NotOnline.as_str()
            );
            continue;
        }
        let active_connection_ids = vec![request.connection_id];
        let previous = presence
            .players
            .get(&request.player_id)
            .map(|record| record.state);
        let eligibility = safe_logout_eligibility(
            request.player_id,
            game_tick.0,
            &active_connection_ids,
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
        if let Some(reason) = eligibility.reason {
            telemetry
                .player_mut(request.player_id)
                .record_rejection(reason);
            let Some(record) = presence.players.get_mut(&request.player_id) else {
                continue;
            };
            if record.rejection_reason == Some(reason) {
                // Idempotent duplicate packet: do not rewrite state or emit a
                // repeated log. In particular, pending/protected requests can
                // never restart or alter their completed transition.
                continue;
            }
            record.rejection_reason = Some(reason);
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
        let player_telemetry = telemetry.player_mut(request.player_id);
        player_telemetry.accepted = player_telemetry.accepted.saturating_add(1);
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
        record.protected_since_tick = None;
        record.protected_run_key = None;
        record.protection_exit_requested = false;
        record.resume_in_progress = false;
        record.resume_connection_id = None;
        record.resume_sync_ready = false;
        record.resume_notice_connection_id = None;
        record.safe_logout_connection_ids = vec![request.connection_id];
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
    mut telemetry: ResMut<SafeLogoutTelemetryState>,
) {
    for cancellation in cancellations.read() {
        if !clients.is_current_connection(cancellation.player_id, cancellation.connection_id) {
            telemetry.record_stale_connection_event(cancellation.player_id);
            continue;
        }
        cancel_pending_with_current_connection(
            cancellation.player_id,
            SafeLogoutCancelReason::Manual,
            &clients,
            game_tick.0,
            &mut presence,
            &mut telemetry,
        );
    }
}

fn safe_logout_pending_system(
    clients: Res<Clients>,
    game_tick: Res<GameTick>,
    ids: Res<Ids>,
    entity_map: Res<EntityObjMap>,
    assigned_runs: Res<AssignedStartLocations>,
    run_spawned: Res<RunSpawnedObjs>,
    zones: Res<SanctuaryZones>,
    crises: Res<SettlementCrisisState>,
    templates: Res<Templates>,
    hero_query: HeroPresenceQuery,
    monolith_query: BoundMonolithQuery,
    hostile_query: HostileQuery,
    mut map_events: ResMut<MapEvents>,
    mut presence: ResMut<PlayerWorldPresenceState>,
    mut telemetry: ResMut<SafeLogoutTelemetryState>,
) {
    let pending_players = presence
        .players
        .iter()
        .filter_map(|(player_id, record)| {
            (record.state == PlayerWorldPresence::SafeLogoutPending).then_some(*player_id)
        })
        .collect::<Vec<_>>();

    for player_id in pending_players {
        let Some(record_snapshot) = presence.players.get(&player_id).cloned() else {
            continue;
        };
        if !clients
            .has_active_connection_from(player_id, &record_snapshot.safe_logout_connection_ids)
        {
            cancel_pending_with_current_connection(
                player_id,
                SafeLogoutCancelReason::Disconnected,
                &clients,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        }
        if !assigned_runs.contains_key(&player_id) {
            remove_player_presence_for_run_cleanup(
                player_id,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        }
        let Some(hero) = resolve_hero(player_id, &ids, &entity_map, &hero_query) else {
            remove_player_presence_for_run_cleanup(
                player_id,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        };
        if hero.true_death || !hero.alive {
            cancel_pending_with_current_connection(
                player_id,
                SafeLogoutCancelReason::HeroDied,
                &clients,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        }
        if crisis_is_assault_active(player_id, &crises) {
            cancel_pending_with_current_connection(
                player_id,
                SafeLogoutCancelReason::AssaultStarted,
                &clients,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        }

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
            cancel_pending_with_current_connection(
                player_id,
                SafeLogoutCancelReason::TookDamage,
                &clients,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        }
        if record_snapshot
            .last_combat_tick
            .map(|tick| tick >= requested_tick)
            .unwrap_or(false)
            || hero.last_combat_tick >= requested_tick
        {
            cancel_pending_with_current_connection(
                player_id,
                SafeLogoutCancelReason::EnteredCombat,
                &clients,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        }
        if record_snapshot.safe_logout_start_position != Some(hero.pos) {
            cancel_pending_with_current_connection(
                player_id,
                SafeLogoutCancelReason::Moved,
                &clients,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        }
        match own_sanctuary_status(hero, &zones, &entity_map, &monolith_query) {
            OwnSanctuaryStatus::Inside => {}
            OwnSanctuaryStatus::Outside => {
                cancel_pending_with_current_connection(
                    player_id,
                    SafeLogoutCancelReason::LeftSanctuary,
                    &clients,
                    game_tick.0,
                    &mut presence,
                    &mut telemetry,
                );
                continue;
            }
            OwnSanctuaryStatus::MissingBinding
            | OwnSanctuaryStatus::MissingZone
            | OwnSanctuaryStatus::Invalid => {
                cancel_pending_with_current_connection(
                    player_id,
                    SafeLogoutCancelReason::SanctuaryInvalid,
                    &clients,
                    game_tick.0,
                    &mut presence,
                    &mut telemetry,
                );
                continue;
            }
        }
        if hostile_nearby(player_id, hero.pos, &templates, &hostile_query) {
            cancel_pending_with_current_connection(
                player_id,
                SafeLogoutCancelReason::HostileNearby,
                &clients,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        }

        if game_tick.0.saturating_sub(requested_tick) < SAFE_LOGOUT_COUNTDOWN_TICKS {
            continue;
        }
        let Some(run_key) = protected_run_key(player_id, hero, &assigned_runs, &run_spawned) else {
            remove_player_presence_for_run_cleanup(
                player_id,
                game_tick.0,
                &mut presence,
                &mut telemetry,
            );
            continue;
        };
        let completion_connection_ids = record_snapshot.safe_logout_connection_ids.clone();
        let outcome = complete_pending_with_connection_check(
            player_id,
            game_tick.0,
            run_key.clone(),
            &mut presence,
            &mut telemetry,
            || clients.has_active_connection_from(player_id, &completion_connection_ids),
            || clients.is_player_online(player_id),
        );
        if outcome == SafeLogoutCompletionOutcome::Completed {
            let purged = purge_unsafe_queued_events(player_id, &run_key, &ids, &mut map_events);
            let player_telemetry = telemetry.player_mut(player_id);
            player_telemetry.queued_events_discarded = player_telemetry
                .queued_events_discarded
                .saturating_add(purged as u64);
            info!(
                "safe_logout_protection_activated player_id={} game_tick={} hero_id={} start_location={} bound_monolith_id={} purged_events={}",
                player_id,
                game_tick.0,
                run_key.hero_id,
                run_key.start_location_name,
                run_key.bound_monolith_id,
                purged
            );
        }
    }
}

/// Deliver one exact status per authenticated connection, then only meaningful
/// changes. The countdown value is ceil-rounded to whole seconds, so equality
/// deduplication naturally limits pending updates to at most one per second.
/// Completion is observed only after the preceding system has committed
/// `OfflineProtected`.
fn safe_logout_status_delivery_system(
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
    mut delivery: ResMut<SafeLogoutStatusDeliveryState>,
    mut telemetry: ResMut<SafeLogoutTelemetryState>,
) {
    let active_clients = match clients.lock() {
        Ok(clients) => clients
            .iter()
            .filter(|(client_id, client)| {
                **client_id == client.id && client.player_id >= 0 && !client.sender.is_closed()
            })
            .map(|(client_id, client)| (*client_id, client.player_id))
            .collect::<Vec<_>>(),
        Err(_) => return,
    };
    let active_clients = active_clients
        .into_iter()
        .filter(|(client_id, player_id)| clients.is_current_connection(*player_id, *client_id))
        .collect::<Vec<_>>();
    let active_client_players = active_clients
        .iter()
        .map(|(client_id, player_id)| (*client_id, *player_id))
        .collect::<HashMap<_, _>>();
    let mut connection_ids_by_player = HashMap::<i32, Vec<Uuid>>::new();
    for (client_id, player_id) in &active_clients {
        connection_ids_by_player
            .entry(*player_id)
            .or_default()
            .push(*client_id);
    }

    delivery
        .sent
        .retain(|client_id, sent| active_client_players.get(client_id) == Some(&sent.player_id));

    let mut statuses = HashMap::<i32, SafeLogoutStatusSnapshot>::new();
    for player_id in connection_ids_by_player.keys().copied() {
        if presence
            .players
            .get(&player_id)
            .map(|record| record.resume_in_progress)
            .unwrap_or(false)
        {
            // Resume remains simulation-protected until the current-run sync
            // barrier is complete. Do not publish an online snapshot early.
            continue;
        }
        let connection_ids = connection_ids_by_player
            .get(&player_id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if let [current_connection_id] = connection_ids {
            if let Some(record) = presence.players.get_mut(&player_id) {
                if !record.resume_in_progress
                    && record.resume_notice_connection_id.is_some()
                    && record.resume_notice_connection_id != Some(*current_connection_id)
                {
                    // Replacement may have become authoritative before this
                    // delivery pass took its registry snapshot. Preserve the
                    // one-shot acknowledgement for that sole current socket;
                    // the locked send below handles replacements after here.
                    record.resume_notice_connection_id = Some(*current_connection_id);
                }
            }
        }
        let eligibility = safe_logout_eligibility(
            player_id,
            game_tick.0,
            connection_ids,
            &ids,
            &entity_map,
            &assigned_runs,
            &zones,
            &crises,
            &templates,
            &hero_query,
            &monolith_query,
            &hostile_query,
            presence.players.get(&player_id),
        );
        statuses.insert(
            player_id,
            build_safe_logout_status(
                presence.players.get(&player_id),
                game_tick.0,
                eligibility,
                connection_ids.first().copied(),
            ),
        );
    }

    for (client_id, player_id) in active_clients {
        let Some(status) = statuses.get(&player_id) else {
            continue;
        };
        if !should_send_safe_logout_status(delivery.sent.get(&client_id), player_id, status) {
            if let Some(sent) = delivery.sent.get_mut(&client_id) {
                if !sent.duplicate_suppression_recorded {
                    sent.duplicate_suppression_recorded = true;
                    let player_telemetry = telemetry.player_mut(player_id);
                    player_telemetry.status_packets_duplicate_suppressed = player_telemetry
                        .status_packets_duplicate_suppressed
                        .saturating_add(1);
                }
            }
            continue;
        }

        let packet = ResponsePacket::SafeLogoutStatus {
            status: status.clone(),
        };
        let Ok(serialized) = serde_json::to_string(&packet) else {
            error!(
                "safe_logout_status_serialization_failed player_id={}",
                player_id
            );
            continue;
        };
        let outcome = try_send_safe_logout_status_to_current_connection(
            &clients,
            player_id,
            client_id,
            serialized,
            status.resumed_from_protection,
            &mut presence,
        );
        if outcome == SafeLogoutStatusSendOutcome::Sent {
            let player_telemetry = telemetry.player_mut(player_id);
            player_telemetry.status_packets_sent =
                player_telemetry.status_packets_sent.saturating_add(1);
            delivery.sent.insert(
                client_id,
                SentSafeLogoutStatus {
                    player_id,
                    status: status.clone(),
                    duplicate_suppression_recorded: false,
                },
            );
        } else {
            debug!(
                "safe_logout_status_send_deferred player_id={} outcome={:?}",
                player_id, outcome
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_eligibility(eligible: bool) -> SafeLogoutEligibility {
        SafeLogoutEligibility {
            eligible,
            reason: (!eligible).then_some(SafeLogoutRejectionReason::OutsideOwnSanctuary),
            in_own_sanctuary: eligible,
            active_assault: false,
        }
    }

    fn status_delivery_test_app() -> App {
        use crate::templates::TemplatesPlugin;

        let mut app = App::new();
        app.add_plugins(TemplatesPlugin)
            .insert_resource(Clients::default())
            .insert_resource(GameTick::default())
            .insert_resource(Ids::default())
            .insert_resource(EntityObjMap(HashMap::new()))
            .insert_resource(AssignedStartLocations::default())
            .insert_resource(SanctuaryZones::default())
            .insert_resource(SettlementCrisisState::default())
            .insert_resource(PlayerWorldPresenceState::default())
            .insert_resource(SafeLogoutTelemetryState::default())
            .insert_resource(SafeLogoutStatusDeliveryState::default())
            .add_systems(Update, safe_logout_status_delivery_system);
        app
    }

    fn add_status_test_client(
        app: &mut App,
        player_id: i32,
        capacity: usize,
    ) -> (Uuid, tokio::sync::mpsc::Receiver<String>) {
        use crate::game::Client;

        let client_id = Uuid::new_v4();
        let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
        app.world().resource::<Clients>().lock().unwrap().insert(
            client_id,
            Client {
                id: client_id,
                player_id,
                sender,
            },
        );
        (client_id, receiver)
    }

    #[test]
    fn safe_logout_checkpoint3_all_rejection_reasons_have_stable_codes() {
        let cases = [
            (SafeLogoutRejectionReason::NotOnline, "unknown"),
            (SafeLogoutRejectionReason::InvalidRun, "run_invalid"),
            (SafeLogoutRejectionReason::MissingHero, "hero_invalid"),
            (SafeLogoutRejectionReason::HeroDied, "hero_dead"),
            (SafeLogoutRejectionReason::TrueDeath, "true_death"),
            (
                SafeLogoutRejectionReason::MissingBoundMonolith,
                "sanctuary_invalid",
            ),
            (
                SafeLogoutRejectionReason::MissingSanctuaryZone,
                "sanctuary_invalid",
            ),
            (
                SafeLogoutRejectionReason::SanctuaryInvalid,
                "sanctuary_invalid",
            ),
            (
                SafeLogoutRejectionReason::OutsideOwnSanctuary,
                "outside_sanctuary",
            ),
            (SafeLogoutRejectionReason::AssaultActive, "assault_active"),
            (SafeLogoutRejectionReason::RecentCombat, "recent_combat"),
            (SafeLogoutRejectionReason::RecentDamage, "recent_damage"),
            (SafeLogoutRejectionReason::HostileNearby, "hostile_nearby"),
            (SafeLogoutRejectionReason::AlreadyPending, "already_pending"),
            (
                SafeLogoutRejectionReason::AlreadyProtected,
                "already_protected",
            ),
        ];

        for (reason, expected) in cases {
            assert_eq!(rejection_status(reason).0, expected);
            assert!(!rejection_status(reason).1.is_empty());
        }
    }

    #[test]
    fn safe_logout_checkpoint3_all_cancellation_reasons_have_stable_codes() {
        let cases = [
            (SafeLogoutCancelReason::Moved, "moved"),
            (SafeLogoutCancelReason::EnteredCombat, "entered_combat"),
            (SafeLogoutCancelReason::TookDamage, "took_damage"),
            (SafeLogoutCancelReason::HostileNearby, "hostile_nearby"),
            (SafeLogoutCancelReason::LeftSanctuary, "left_sanctuary"),
            (
                SafeLogoutCancelReason::SanctuaryInvalid,
                "sanctuary_invalid",
            ),
            (SafeLogoutCancelReason::AssaultStarted, "assault_started"),
            (SafeLogoutCancelReason::HeroDied, "hero_died"),
            (
                SafeLogoutCancelReason::Disconnected,
                "disconnected_before_completion",
            ),
            (SafeLogoutCancelReason::Manual, "manually_cancelled"),
            (SafeLogoutCancelReason::RunEnded, "run_ended"),
            (SafeLogoutCancelReason::InvalidState, "invalid_state"),
        ];

        for (reason, expected) in cases {
            assert_eq!(cancellation_status(reason).0, expected);
            assert!(!cancellation_status(reason).1.is_empty());
        }
    }

    #[test]
    fn safe_logout_checkpoint3_status_countdown_is_server_rounded_and_protected_is_final_zero() {
        let mut pending = PlayerPresenceRecord::new(true);
        pending.state = PlayerWorldPresence::SafeLogoutPending;
        pending.safe_logout_requested_tick = Some(100);
        let eligibility = test_eligibility(false);

        let at_start = build_safe_logout_status(Some(&pending), 100, eligibility, None);
        let same_second = build_safe_logout_status(Some(&pending), 109, eligibility, None);
        let next_second = build_safe_logout_status(Some(&pending), 110, eligibility, None);
        assert_eq!(at_start.state, "pending");
        assert_eq!(at_start.countdown_total_seconds, Some(10));
        assert_eq!(at_start.countdown_remaining_seconds, Some(10));
        assert_eq!(same_second, at_start);
        assert_eq!(next_second.countdown_remaining_seconds, Some(9));
        assert!(next_second.can_cancel);
        assert!(!next_second.can_request);

        pending.state = PlayerWorldPresence::OfflineProtected;
        let protected = build_safe_logout_status(Some(&pending), 200, eligibility, None);
        assert_eq!(protected.state, "protected");
        assert_eq!(protected.countdown_remaining_seconds, Some(0));
        assert!(protected.protected);
        assert!(!protected.can_cancel);
    }

    #[test]
    fn safe_logout_checkpoint3_status_deduplicates_and_new_connections_resync() {
        let player_id = 77;
        let record = PlayerPresenceRecord::new(true);
        let status = build_safe_logout_status(Some(&record), 10, test_eligibility(true), None);
        assert!(should_send_safe_logout_status(None, player_id, &status));

        let sent = SentSafeLogoutStatus {
            player_id,
            status: status.clone(),
            duplicate_suppression_recorded: false,
        };
        assert!(!should_send_safe_logout_status(
            Some(&sent),
            player_id,
            &status
        ));

        let mut changed = status.clone();
        changed.can_request = false;
        changed.reason = Some("hostile_nearby".to_string());
        assert!(should_send_safe_logout_status(
            Some(&sent),
            player_id,
            &changed
        ));
        assert!(should_send_safe_logout_status(
            Some(&sent),
            player_id + 1,
            &status
        ));
    }

    #[test]
    fn safe_logout_checkpoint3_snapshot_uses_canonical_eligibility_reason() {
        let record = PlayerPresenceRecord::new(true);
        let reasons = [
            SafeLogoutRejectionReason::InvalidRun,
            SafeLogoutRejectionReason::MissingHero,
            SafeLogoutRejectionReason::HeroDied,
            SafeLogoutRejectionReason::TrueDeath,
            SafeLogoutRejectionReason::MissingBoundMonolith,
            SafeLogoutRejectionReason::MissingSanctuaryZone,
            SafeLogoutRejectionReason::SanctuaryInvalid,
            SafeLogoutRejectionReason::OutsideOwnSanctuary,
            SafeLogoutRejectionReason::AssaultActive,
            SafeLogoutRejectionReason::RecentCombat,
            SafeLogoutRejectionReason::RecentDamage,
            SafeLogoutRejectionReason::HostileNearby,
        ];

        for reason in reasons {
            let eligibility = SafeLogoutEligibility {
                eligible: false,
                reason: Some(reason),
                in_own_sanctuary: reason != SafeLogoutRejectionReason::OutsideOwnSanctuary,
                active_assault: reason == SafeLogoutRejectionReason::AssaultActive,
            };
            let status = build_safe_logout_status(Some(&record), 50, eligibility, None);
            assert!(!status.can_request);
            assert_eq!(status.reason.as_deref(), Some(rejection_status(reason).0));
            assert_eq!(status.message, rejection_status(reason).1);
            assert_eq!(status.active_assault, eligibility.active_assault);
        }
    }

    #[test]
    fn safe_logout_checkpoint3_delivery_is_per_connection_deduplicated_and_private() {
        let mut app = status_delivery_test_app();
        let (first_client_id, mut first_receiver) = add_status_test_client(&mut app, 1, 8);
        let (_second_client_id, mut second_receiver) = add_status_test_client(&mut app, 2, 8);

        app.update();
        let first: ResponsePacket =
            serde_json::from_str(&first_receiver.try_recv().unwrap()).unwrap();
        let second: ResponsePacket =
            serde_json::from_str(&second_receiver.try_recv().unwrap()).unwrap();
        assert!(matches!(first, ResponsePacket::SafeLogoutStatus { .. }));
        assert!(matches!(second, ResponsePacket::SafeLogoutStatus { .. }));

        app.update();
        assert!(first_receiver.try_recv().is_err());
        assert!(second_receiver.try_recv().is_err());

        app.world_mut()
            .resource_mut::<PlayerWorldPresenceState>()
            .players
            .insert(1, PlayerPresenceRecord::new(true));
        app.update();
        let first: ResponsePacket =
            serde_json::from_str(&first_receiver.try_recv().unwrap()).unwrap();
        assert!(matches!(
            first,
            ResponsePacket::SafeLogoutStatus { status }
                if status.state == "online" && status.reason.as_deref() == Some("run_invalid")
        ));
        assert!(second_receiver.try_recv().is_err());

        let delivery = app.world().resource::<SafeLogoutStatusDeliveryState>();
        assert_eq!(delivery.sent.get(&first_client_id).unwrap().player_id, 1);
    }

    #[test]
    fn safe_logout_checkpoint3_failed_send_is_retried_and_not_cached() {
        let mut app = status_delivery_test_app();
        let (client_id, mut receiver) = add_status_test_client(&mut app, 3, 1);
        let sender = app
            .world()
            .resource::<Clients>()
            .lock()
            .unwrap()
            .get(&client_id)
            .unwrap()
            .sender
            .clone();
        sender.try_send("occupied".to_string()).unwrap();

        app.update();
        assert!(!app
            .world()
            .resource::<SafeLogoutStatusDeliveryState>()
            .sent
            .contains_key(&client_id));
        assert_eq!(receiver.try_recv().unwrap(), "occupied");

        app.update();
        let packet: ResponsePacket = serde_json::from_str(&receiver.try_recv().unwrap()).unwrap();
        assert!(matches!(packet, ResponsePacket::SafeLogoutStatus { .. }));
        assert!(app
            .world()
            .resource::<SafeLogoutStatusDeliveryState>()
            .sent
            .contains_key(&client_id));
    }

    #[test]
    fn safe_logout_checkpoint3_run_cleanup_and_fresh_run_clear_stale_status() {
        let player_id = 91;
        let mut protected_record = PlayerPresenceRecord::new(false);
        protected_record.state = PlayerWorldPresence::OfflineProtected;
        let protected =
            build_safe_logout_status(Some(&protected_record), 200, test_eligibility(false), None);
        assert!(protected.protected);

        let cleared = build_safe_logout_status(
            None,
            201,
            SafeLogoutEligibility {
                eligible: false,
                reason: Some(SafeLogoutRejectionReason::InvalidRun),
                in_own_sanctuary: false,
                active_assault: false,
            },
            None,
        );
        assert_eq!(cleared.state, "disconnected");
        assert!(!cleared.protected);
        assert!(should_send_safe_logout_status(
            Some(&SentSafeLogoutStatus {
                player_id,
                status: protected,
                duplicate_suppression_recorded: false,
            }),
            player_id,
            &cleared,
        ));

        let fresh_record = PlayerPresenceRecord::new(true);
        let fresh =
            build_safe_logout_status(Some(&fresh_record), 202, test_eligibility(true), None);
        assert_eq!(fresh.state, "online");
        assert!(fresh.can_request);
        assert!(fresh.reason.is_none());
        assert!(!fresh.protected);

        // Exercise the delivery cache across the same presence cleanup and
        // initialization helpers used by True Death and successful new-run
        // setup. A cached protected packet cannot survive either boundary.
        let mut app = status_delivery_test_app();
        let (_client_id, mut receiver) = add_status_test_client(&mut app, player_id, 8);
        app.world_mut()
            .resource_mut::<PlayerWorldPresenceState>()
            .players
            .insert(player_id, protected_record);
        app.update();
        let protected_packet: ResponsePacket =
            serde_json::from_str(&receiver.try_recv().unwrap()).unwrap();
        assert!(matches!(
            protected_packet,
            ResponsePacket::SafeLogoutStatus { status }
                if status.state == "protected" && status.protected
        ));

        app.world_mut()
            .resource_scope(|world, mut presence: Mut<PlayerWorldPresenceState>| {
                let mut telemetry = world.resource_mut::<SafeLogoutTelemetryState>();
                remove_player_presence_for_run_cleanup(
                    player_id,
                    201,
                    &mut presence,
                    &mut telemetry,
                );
            });
        app.update();
        let cleared_packet: ResponsePacket =
            serde_json::from_str(&receiver.try_recv().unwrap()).unwrap();
        assert!(matches!(
            cleared_packet,
            ResponsePacket::SafeLogoutStatus { status }
                if status.state == "disconnected" && !status.protected
        ));

        app.world_mut()
            .resource_scope(|world, mut presence: Mut<PlayerWorldPresenceState>| {
                let mut telemetry = world.resource_mut::<SafeLogoutTelemetryState>();
                initialize_player_presence(player_id, true, 202, &mut presence, &mut telemetry);
            });
        app.update();
        let fresh_packet: ResponsePacket =
            serde_json::from_str(&receiver.try_recv().unwrap()).unwrap();
        assert!(matches!(
            fresh_packet,
            ResponsePacket::SafeLogoutStatus { status }
                if status.state == "online" && !status.protected
        ));
    }

    #[test]
    fn safe_logout_checkpoint3_manual_cancellation_is_online_and_requestable_again() {
        let mut record = PlayerPresenceRecord::new(true);
        record.cancel_reason = Some(SafeLogoutCancelReason::Manual);
        let status = build_safe_logout_status(Some(&record), 20, test_eligibility(true), None);

        assert_eq!(status.state, "online");
        assert_eq!(status.reason.as_deref(), Some("manually_cancelled"));
        assert!(status.can_request);
        assert!(!status.can_cancel);
    }

    #[test]
    fn safe_logout_checkpoint1_pending_cancellation_is_typed_and_idempotent() {
        let player_id = 7;
        let mut presence = PlayerWorldPresenceState::default();
        let mut telemetry = SafeLogoutTelemetryState::default();
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
            &mut telemetry,
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
            &mut telemetry,
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
        let mut telemetry = SafeLogoutTelemetryState::default();
        initialize_player_presence(1, true, 10, &mut presence, &mut telemetry);
        initialize_player_presence(2, false, 10, &mut presence, &mut telemetry);
        {
            let first = presence.players.get_mut(&1).unwrap();
            first.state = PlayerWorldPresence::OfflineProtected;
            first.last_combat_tick = Some(9);
            first.last_damage_tick = Some(8);
        }

        remove_player_presence_for_run_cleanup(1, 11, &mut presence, &mut telemetry);
        remove_player_presence_for_run_cleanup(1, 11, &mut presence, &mut telemetry);
        assert!(!presence.players.contains_key(&1));
        assert_eq!(
            presence.players.get(&2).map(|record| record.state),
            Some(PlayerWorldPresence::Disconnected),
        );

        initialize_player_presence(1, true, 12, &mut presence, &mut telemetry);
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
        let mut telemetry = SafeLogoutTelemetryState::default();
        initialize_player_presence(player_id, true, 1, &mut presence, &mut telemetry);
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
        let mut telemetry = SafeLogoutTelemetryState::default();
        let mut record = PlayerPresenceRecord::new(true);
        record.state = PlayerWorldPresence::SafeLogoutPending;
        record.safe_logout_requested_tick = Some(100);
        record.safe_logout_start_position = Some(Position { x: 4, y: 5 });
        presence.players.insert(player_id, record);

        let samples = Cell::new(0);
        let outcome = complete_pending_with_connection_check(
            player_id,
            200,
            ProtectedRunKey {
                player_id,
                hero_id: 101,
                start_location_name: "test".to_string(),
                bound_monolith_id: 102,
                run_object_ids: Vec::new(),
            },
            &mut presence,
            &mut telemetry,
            || {
                let sample = samples.get();
                samples.set(sample + 1);
                sample == 0
            },
            || false,
        );

        assert_eq!(outcome, SafeLogoutCompletionOutcome::Cancelled);
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

    #[test]
    fn safe_logout_checkpoint1_replacement_connection_rolls_back_online() {
        use std::cell::Cell;

        let player_id = 12;
        let mut presence = PlayerWorldPresenceState::default();
        let mut telemetry = SafeLogoutTelemetryState::default();
        let mut record = PlayerPresenceRecord::new(true);
        record.state = PlayerWorldPresence::SafeLogoutPending;
        record.safe_logout_requested_tick = Some(100);
        record.safe_logout_start_position = Some(Position { x: 4, y: 5 });
        presence.players.insert(player_id, record);

        let samples = Cell::new(0);
        let outcome = complete_pending_with_connection_check(
            player_id,
            200,
            ProtectedRunKey {
                player_id,
                hero_id: 111,
                start_location_name: "test".to_string(),
                bound_monolith_id: 112,
                run_object_ids: Vec::new(),
            },
            &mut presence,
            &mut telemetry,
            || {
                let sample = samples.get();
                samples.set(sample + 1);
                sample == 0
            },
            || true,
        );

        assert_eq!(outcome, SafeLogoutCompletionOutcome::Cancelled);
        assert_eq!(samples.get(), 2);
        let record = presence.players.get(&player_id).unwrap();
        assert_eq!(record.state, PlayerWorldPresence::Online);
        assert_eq!(
            record.cancel_reason,
            Some(SafeLogoutCancelReason::Disconnected)
        );
        assert_eq!(record.safe_logout_requested_tick, None);
        assert_eq!(record.safe_logout_start_position, None);
    }

    #[test]
    fn checkpoint2_canonical_protection_includes_neutral_run_objects_and_bound_monolith() {
        let player_id = 7;
        let hero_id = 70;
        let bound_monolith_id = 71;
        let intro_npc_id = 72;
        let ambient_npc_id = 73;
        let mut ids = Ids::default();
        ids.new_obj(hero_id, player_id);
        ids.new_obj(bound_monolith_id, crate::constants::MONOLITH_PLAYER_ID);
        ids.new_obj(intro_npc_id, crate::constants::NPC_PLAYER_ID);
        ids.new_obj(ambient_npc_id, crate::constants::NPC_PLAYER_ID);

        let mut presence = PlayerWorldPresenceState::default();
        let mut record = PlayerPresenceRecord::new(false);
        record.state = PlayerWorldPresence::OfflineProtected;
        record.protected_since_tick = Some(100);
        record.protected_run_key = Some(ProtectedRunKey {
            player_id,
            hero_id,
            start_location_name: "checkpoint2".to_string(),
            bound_monolith_id,
            run_object_ids: vec![intro_npc_id],
        });
        presence.players.insert(player_id, record);

        assert!(object_belongs_to_protected_run(hero_id, &ids, &presence));
        assert!(object_belongs_to_protected_run(
            bound_monolith_id,
            &ids,
            &presence
        ));
        assert!(object_belongs_to_protected_run(
            intro_npc_id,
            &ids,
            &presence
        ));
        assert!(entity_belongs_to_protected_run(
            &Id(intro_npc_id),
            &PlayerId(crate::constants::NPC_PLAYER_ID),
            &presence
        ));
        assert!(!object_belongs_to_protected_run(
            ambient_npc_id,
            &ids,
            &presence
        ));
    }

    #[test]
    fn safe_logout_checkpoint4_resume_is_connection_scoped_and_duplicate_safe() {
        let player_id = 81;
        let first = Uuid::new_v4();
        let replacement = Uuid::new_v4();
        let mut presence = PlayerWorldPresenceState::default();
        let mut telemetry = SafeLogoutTelemetryState::default();
        let mut record = PlayerPresenceRecord::new(false);
        record.state = PlayerWorldPresence::OfflineProtected;
        record.protected_since_tick = Some(100);
        record.protected_run_key = Some(ProtectedRunKey {
            player_id,
            hero_id: 810,
            start_location_name: "checkpoint4".to_string(),
            bound_monolith_id: 811,
            run_object_ids: vec![812],
        });
        presence.players.insert(player_id, record);

        assert!(mark_player_logged_in(
            player_id,
            first,
            200,
            &mut presence,
            &mut telemetry,
        ));
        assert!(!mark_player_logged_in(
            player_id,
            first,
            200,
            &mut presence,
            &mut telemetry,
        ));
        let record = presence.players.get_mut(&player_id).unwrap();
        assert_eq!(record.resume_connection_id, Some(first));
        assert!(record.protection_exit_requested);

        // Model the first rebase boundary. The exact sync marker may release
        // only this socket; a duplicate or replacement cannot acknowledge it.
        record.state = PlayerWorldPresence::Online;
        record.protection_exit_requested = false;
        record.resume_in_progress = true;
        record.resume_sync_ready = false;
        record.protected_since_tick = Some(200);
        assert!(!mark_player_login_sync_complete(
            player_id,
            replacement,
            204,
            &mut presence,
        ));
        assert!(mark_player_login_sync_complete(
            player_id,
            first,
            204,
            &mut presence,
        ));
        assert!(!mark_player_login_sync_complete(
            player_id,
            first,
            204,
            &mut presence,
        ));

        // A replacement before final release re-enters OfflineProtected and
        // starts a new connection-scoped barrier without losing the run key.
        assert!(mark_player_logged_in(
            player_id,
            replacement,
            205,
            &mut presence,
            &mut telemetry,
        ));
        let record = presence.players.get(&player_id).unwrap();
        assert_eq!(record.state, PlayerWorldPresence::OfflineProtected);
        assert!(record.protection_exit_requested);
        assert!(!record.resume_in_progress);
        assert_eq!(record.resume_connection_id, Some(replacement));
        assert!(record.protected_run_key.is_some());
        assert_eq!(
            telemetry
                .get(&player_id)
                .map(|telemetry| telemetry.resumed)
                .unwrap_or_default(),
            0
        );
    }

    #[test]
    fn safe_logout_checkpoint4_authority_change_during_rebase_keeps_protection() {
        let player_id = 92;
        let displaced = Uuid::new_v4();
        let replacement = Uuid::new_v4();
        let key = ProtectedRunKey {
            player_id,
            hero_id: 920,
            start_location_name: "authority-race".to_string(),
            bound_monolith_id: 921,
            run_object_ids: vec![922],
        };
        let mut record = PlayerPresenceRecord::new(true);
        record.state = PlayerWorldPresence::OfflineProtected;
        record.protected_since_tick = Some(100);
        record.protected_run_key = Some(key.clone());
        record.protection_exit_requested = true;
        record.resume_connection_id = Some(displaced);
        record.last_combat_tick = Some(80);

        let displaced_resume = ProtectionResume {
            player_id,
            duration: 100,
            key: key.clone(),
            connection_id: displaced,
            finalizing: false,
        };
        assert_eq!(
            commit_rebased_resume_presence(&mut record, &displaced_resume, 200, false, true,),
            ResumeCommitOutcome::AuthorityChanged,
        );
        assert_eq!(record.state, PlayerWorldPresence::OfflineProtected);
        assert_eq!(record.protected_since_tick, Some(200));
        assert_eq!(record.protected_run_key.as_ref(), Some(&key));
        assert_eq!(record.last_combat_tick, Some(180));
        assert_eq!(record.resume_connection_id, None);
        assert!(!record.resume_in_progress);

        // Reprocessing the displaced work cannot apply the old duration twice.
        assert_eq!(
            commit_rebased_resume_presence(&mut record, &displaced_resume, 200, false, true,),
            ResumeCommitOutcome::Stale,
        );
        assert_eq!(record.last_combat_tick, Some(180));

        record.protection_exit_requested = true;
        record.resume_connection_id = Some(replacement);
        let replacement_resume = ProtectionResume {
            player_id,
            duration: 5,
            key: key.clone(),
            connection_id: replacement,
            finalizing: false,
        };
        assert_eq!(
            commit_rebased_resume_presence(&mut record, &replacement_resume, 205, true, true,),
            ResumeCommitOutcome::AwaitingSync,
        );
        assert!(record.resume_in_progress);
        assert_eq!(record.last_combat_tick, Some(185));

        record.resume_sync_ready = true;
        let final_release = ProtectionResume {
            player_id,
            duration: 2,
            key,
            connection_id: replacement,
            finalizing: true,
        };
        assert_eq!(
            commit_rebased_resume_presence(&mut record, &final_release, 207, true, true),
            ResumeCommitOutcome::Released,
        );
        assert_eq!(record.state, PlayerWorldPresence::Online);
        assert!(!record.resume_in_progress);
        assert_eq!(record.protected_since_tick, None);
        assert_eq!(record.last_combat_tick, Some(187));
        assert_eq!(record.resume_notice_connection_id, Some(replacement));

        let replacement_status = build_safe_logout_status(
            Some(&record),
            207,
            test_eligibility(true),
            Some(replacement),
        );
        let displaced_status =
            build_safe_logout_status(Some(&record), 207, test_eligibility(true), Some(displaced));
        assert!(replacement_status.resumed_from_protection);
        assert!(!displaced_status.resumed_from_protection);
    }

    #[test]
    fn safe_logout_checkpoint4_completion_and_duration_telemetry_are_exactly_once() {
        let player_id = 82;
        let mut presence = PlayerWorldPresenceState::default();
        let mut telemetry = SafeLogoutTelemetryState::default();
        let mut record = PlayerPresenceRecord::new(true);
        record.state = PlayerWorldPresence::SafeLogoutPending;
        record.safe_logout_requested_tick = Some(100);
        record.safe_logout_start_position = Some(Position { x: 3, y: 4 });
        presence.players.insert(player_id, record);
        let key = ProtectedRunKey {
            player_id,
            hero_id: 820,
            start_location_name: "checkpoint4".to_string(),
            bound_monolith_id: 821,
            run_object_ids: Vec::new(),
        };

        assert_eq!(
            complete_pending_with_connection_check(
                player_id,
                200,
                key.clone(),
                &mut presence,
                &mut telemetry,
                || true,
                || true,
            ),
            SafeLogoutCompletionOutcome::Completed,
        );
        assert_eq!(
            complete_pending_with_connection_check(
                player_id,
                201,
                key,
                &mut presence,
                &mut telemetry,
                || true,
                || true,
            ),
            SafeLogoutCompletionOutcome::NotPending,
        );
        let counters = telemetry.get(&player_id).unwrap();
        assert_eq!(counters.completed, 1);
        assert_eq!(counters.protected_sessions_started, 1);

        remove_player_presence_for_run_cleanup(player_id, 250, &mut presence, &mut telemetry);
        remove_player_presence_for_run_cleanup(player_id, 300, &mut presence, &mut telemetry);
        assert_eq!(telemetry.get(&player_id).unwrap().protected_ticks_total, 50);
    }

    #[test]
    fn safe_logout_checkpoint4_invalid_protection_recovery_is_idempotent() {
        let player_id = 83;
        let mut presence = PlayerWorldPresenceState::default();
        let mut telemetry = SafeLogoutTelemetryState::default();
        let mut record = PlayerPresenceRecord::new(false);
        record.state = PlayerWorldPresence::OfflineProtected;
        record.protected_run_key = Some(ProtectedRunKey {
            player_id,
            hero_id: 830,
            start_location_name: "stale".to_string(),
            bound_monolith_id: 831,
            run_object_ids: Vec::new(),
        });
        presence.players.insert(player_id, record);

        recover_invalid_protection(
            player_id,
            false,
            400,
            "invalid_protected_since_tick",
            &mut presence,
            &mut telemetry,
        );
        recover_invalid_protection(
            player_id,
            false,
            401,
            "invalid_protected_since_tick",
            &mut presence,
            &mut telemetry,
        );

        let record = presence.players.get(&player_id).unwrap();
        assert_eq!(record.state, PlayerWorldPresence::Disconnected);
        assert!(record.protected_run_key.is_none());
        let counters = telemetry.get(&player_id).unwrap();
        assert_eq!(counters.invariant_recoveries, 1);
        assert_eq!(
            counters
                .invariant_reasons
                .get("invalid_protected_since_tick"),
            Some(&1),
        );
    }

    #[test]
    fn safe_logout_checkpoint4_invalid_resume_phase_never_rebases_deadlines() {
        use crate::headless::HeadlessGame;

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "InvalidResumePhaseBot");
        game.prepare_safe_logout_scenario();
        game.complete_valid_safe_logout();

        let player_id = game.player_id();
        let connection_id = game
            .current_connection_id()
            .expect("completed safe logout retains its authenticated client");
        let score_start_tick = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .expect("headless run score")
            .start_tick;
        {
            let world = game.app_mut().world_mut();
            let mut presence = world.resource_mut::<PlayerWorldPresenceState>();
            let record = presence
                .players
                .get_mut(&player_id)
                .expect("protected presence");
            assert_eq!(record.state, PlayerWorldPresence::OfflineProtected);
            record.protection_exit_requested = true;
            record.resume_in_progress = true;
            record.resume_connection_id = Some(connection_id);
            record.resume_sync_ready = false;
        }

        game.tick(1);

        assert_eq!(
            game.world()
                .resource::<RunScoreState>()
                .get(&player_id)
                .expect("run score after recovery")
                .start_tick,
            score_start_tick,
            "an impossible phase must be rejected before the world deadline walk"
        );
        let recovered = game
            .world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&player_id)
            .cloned()
            .expect("recovered presence");
        assert_eq!(recovered.state, PlayerWorldPresence::Online);
        assert!(recovered.protected_since_tick.is_none());
        assert!(recovered.protected_run_key.is_none());
        let counters = game
            .world()
            .resource::<SafeLogoutTelemetryState>()
            .get(&player_id)
            .cloned()
            .expect("safe logout telemetry");
        assert_eq!(counters.timer_rebases, 0);
        assert_eq!(counters.invariant_recoveries, 1);
        assert_eq!(
            counters.invariant_reasons.get("invalid_resume_fields"),
            Some(&1)
        );

        game.tick(1);

        assert_eq!(
            game.world()
                .resource::<RunScoreState>()
                .get(&player_id)
                .expect("run score after duplicate integrity pass")
                .start_tick,
            score_start_tick
        );
        let counters = game
            .world()
            .resource::<SafeLogoutTelemetryState>()
            .get(&player_id)
            .expect("safe logout telemetry after duplicate integrity pass");
        assert_eq!(counters.invariant_recoveries, 1);
        assert_eq!(
            counters.invariant_reasons.get("invalid_resume_fields"),
            Some(&1)
        );
    }

    #[test]
    fn safe_logout_checkpoint4_resume_status_pulse_is_sent_once() {
        let player_id = 84;
        let mut app = status_delivery_test_app();
        let (client_id, mut receiver) = add_status_test_client(&mut app, player_id, 8);
        let mut record = PlayerPresenceRecord::new(true);
        record.resume_notice_connection_id = Some(client_id);
        app.world_mut()
            .resource_mut::<PlayerWorldPresenceState>()
            .players
            .insert(player_id, record);

        app.update();
        let packet: ResponsePacket = serde_json::from_str(&receiver.try_recv().unwrap()).unwrap();
        assert!(matches!(
            packet,
            ResponsePacket::SafeLogoutStatus { status }
                if status.state == "online" && status.resumed_from_protection
        ));
        assert!(app
            .world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&player_id)
            .unwrap()
            .resume_notice_connection_id
            .is_none());

        app.update();
        assert!(receiver.try_recv().is_err());
        let counters = app
            .world()
            .resource::<SafeLogoutTelemetryState>()
            .get(&player_id)
            .unwrap();
        assert_eq!(counters.status_packets_sent, 1);
        assert_eq!(counters.status_packets_duplicate_suppressed, 1);
    }

    #[test]
    fn safe_logout_checkpoint4_replacement_before_status_snapshot_rebinds_resume_pulse() {
        use crate::game::Client;

        let player_id = 86;
        let mut app = status_delivery_test_app();
        let clients = app.world().resource::<Clients>().clone();
        let old_connection_id = Uuid::new_v4();
        let replacement_connection_id = Uuid::new_v4();
        let (old_sender, mut old_receiver) = tokio::sync::mpsc::channel(2);
        let (replacement_sender, mut replacement_receiver) = tokio::sync::mpsc::channel(2);
        clients.activate(Client {
            id: old_connection_id,
            player_id,
            sender: old_sender.clone(),
        });

        let mut record = PlayerPresenceRecord::new(true);
        record.resume_notice_connection_id = Some(old_connection_id);
        app.world_mut()
            .resource_mut::<PlayerWorldPresenceState>()
            .players
            .insert(player_id, record);

        // Replacement is already authoritative before delivery snapshots the
        // registry, so there is no attempted stale send to trigger retargeting.
        assert_eq!(
            clients.activate(Client {
                id: replacement_connection_id,
                player_id,
                sender: replacement_sender,
            }),
            vec![old_connection_id]
        );

        app.update();

        assert!(matches!(
            old_receiver.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
        let delivered: ResponsePacket =
            serde_json::from_str(&replacement_receiver.try_recv().unwrap()).unwrap();
        assert!(matches!(
            delivered,
            ResponsePacket::SafeLogoutStatus { status }
                if status.state == "online" && status.resumed_from_protection
        ));
        assert!(app
            .world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&player_id)
            .unwrap()
            .resume_notice_connection_id
            .is_none());

        app.update();

        assert!(matches!(
            old_receiver.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
        assert!(matches!(
            replacement_receiver.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
        let counters = app
            .world()
            .resource::<SafeLogoutTelemetryState>()
            .get(&player_id)
            .expect("safe logout telemetry");
        assert_eq!(counters.status_packets_sent, 1);
        assert_eq!(counters.status_packets_duplicate_suppressed, 1);
    }

    #[test]
    fn safe_logout_checkpoint4_replacement_at_status_send_preserves_resume_pulse_for_new_authority()
    {
        use crate::game::Client;

        let player_id = 85;
        let clients = Clients::default();
        let old_connection_id = Uuid::new_v4();
        let replacement_connection_id = Uuid::new_v4();
        let (old_sender, mut old_receiver) = tokio::sync::mpsc::channel(2);
        let (replacement_sender, mut replacement_receiver) = tokio::sync::mpsc::channel(2);
        clients.activate(Client {
            id: old_connection_id,
            player_id,
            sender: old_sender,
        });
        let old_snapshot_sender = clients
            .lock()
            .unwrap()
            .get(&old_connection_id)
            .unwrap()
            .sender
            .clone();

        let mut presence = PlayerWorldPresenceState::default();
        let mut record = PlayerPresenceRecord::new(true);
        record.resume_notice_connection_id = Some(old_connection_id);
        presence.players.insert(player_id, record);

        // Model replacement after the delivery system snapshots the old UUID
        // but before it enqueues the status. Atomic activation must win this
        // interleaving completely.
        assert_eq!(
            clients.activate(Client {
                id: replacement_connection_id,
                player_id,
                sender: replacement_sender,
            }),
            vec![old_connection_id]
        );
        let stale_status = build_safe_logout_status(
            presence.players.get(&player_id),
            100,
            test_eligibility(true),
            Some(old_connection_id),
        );
        assert!(stale_status.resumed_from_protection);
        let stale_packet = serde_json::to_string(&ResponsePacket::SafeLogoutStatus {
            status: stale_status,
        })
        .unwrap();
        assert_eq!(
            try_send_safe_logout_status_to_current_connection(
                &clients,
                player_id,
                old_connection_id,
                stale_packet,
                true,
                &mut presence,
            ),
            SafeLogoutStatusSendOutcome::StaleConnection
        );
        assert!(matches!(
            old_receiver.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
        assert_eq!(
            presence
                .players
                .get(&player_id)
                .and_then(|record| record.resume_notice_connection_id),
            Some(replacement_connection_id),
            "the displaced socket must not consume the resume pulse"
        );

        let replacement_status = build_safe_logout_status(
            presence.players.get(&player_id),
            101,
            test_eligibility(true),
            Some(replacement_connection_id),
        );
        assert!(replacement_status.resumed_from_protection);
        let replacement_packet = serde_json::to_string(&ResponsePacket::SafeLogoutStatus {
            status: replacement_status,
        })
        .unwrap();
        assert_eq!(
            try_send_safe_logout_status_to_current_connection(
                &clients,
                player_id,
                replacement_connection_id,
                replacement_packet,
                true,
                &mut presence,
            ),
            SafeLogoutStatusSendOutcome::Sent
        );
        let delivered: ResponsePacket =
            serde_json::from_str(&replacement_receiver.try_recv().unwrap()).unwrap();
        assert!(matches!(
            delivered,
            ResponsePacket::SafeLogoutStatus { status }
                if status.state == "online" && status.resumed_from_protection
        ));
        assert!(presence
            .players
            .get(&player_id)
            .unwrap()
            .resume_notice_connection_id
            .is_none());
        drop(old_snapshot_sender);
    }

    #[test]
    fn safe_logout_checkpoint4_boundary_telemetry_is_owner_scoped() {
        let mut telemetry = SafeLogoutTelemetryState::default();
        telemetry.record_protected_input_rejection(90);
        telemetry.record_protected_damage_block(90);
        telemetry.record_protected_target_rejection(90);
        telemetry.record_protected_queued_events_discarded(90, 3);
        telemetry.record_stale_connection_event(91);

        let protected = telemetry.get(&90).unwrap();
        assert_eq!(protected.protected_input_rejections, 1);
        assert_eq!(protected.protected_damage_blocks, 1);
        assert_eq!(protected.protected_target_rejections, 1);
        assert_eq!(protected.queued_events_discarded, 3);
        assert_eq!(protected.stale_connection_events_rejected, 0);
        assert_eq!(
            telemetry.get(&91).unwrap().stale_connection_events_rejected,
            1
        );
    }
}
