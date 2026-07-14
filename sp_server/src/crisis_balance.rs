//! Read-only balance instrumentation for the first personal goblin crisis.
//!
//! This module deliberately contains observations, not tuning controls. The
//! authoritative pressure, phase, launch, combat, and Safe Logout systems stay
//! in their existing modules; this layer records what those systems did.

use std::collections::{BTreeSet, HashMap};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::constants::NPC_PLAYER_ID;
use crate::game::{CrisisAssaultUnit, CrisisPhase, SettlementCrisisState};
use crate::obj::Subclass;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CrisisBalanceScenario {
    Passive,
    BasicSurvival,
    PreparedSolo,
    FortifiedSolo,
    NoVillagers,
    VillagerSupported,
    OrdinaryDisconnect,
    SafeLogoutBeforeAssault,
    HelperSupported,
    AdjacentSettlement,
    #[default]
    Standard,
}

impl CrisisBalanceScenario {
    pub const IMPLEMENTED_MATRIX: [Self; 9] = [
        Self::Passive,
        Self::BasicSurvival,
        Self::PreparedSolo,
        Self::FortifiedSolo,
        Self::NoVillagers,
        Self::VillagerSupported,
        Self::OrdinaryDisconnect,
        Self::SafeLogoutBeforeAssault,
        Self::HelperSupported,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Passive => "passive",
            Self::BasicSurvival => "basic_survival",
            Self::PreparedSolo => "prepared_solo",
            Self::FortifiedSolo => "fortified_solo",
            Self::NoVillagers => "no_villagers",
            Self::VillagerSupported => "villager_supported",
            Self::OrdinaryDisconnect => "ordinary_disconnect",
            Self::SafeLogoutBeforeAssault => "safe_logout_before_assault",
            Self::HelperSupported => "helper_supported",
            Self::AdjacentSettlement => "adjacent_settlement",
            Self::Standard => "standard",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::IMPLEMENTED_MATRIX
            .into_iter()
            .chain([Self::AdjacentSettlement, Self::Standard])
            .find(|scenario| scenario.label() == label)
    }

    pub const fn prepared_group(self) -> &'static str {
        match self {
            Self::PreparedSolo
            | Self::FortifiedSolo
            | Self::NoVillagers
            | Self::VillagerSupported
            | Self::OrdinaryDisconnect
            | Self::SafeLogoutBeforeAssault
            | Self::HelperSupported => "prepared",
            _ => "unprepared",
        }
    }

    pub const fn villager_group(self) -> &'static str {
        match self {
            Self::VillagerSupported => "villagers",
            Self::NoVillagers | Self::PreparedSolo | Self::FortifiedSolo => "no_villagers",
            _ => "unspecified",
        }
    }

    pub const fn connection_group(self) -> &'static str {
        match self {
            Self::OrdinaryDisconnect => "disconnected_during_assault",
            Self::SafeLogoutBeforeAssault => "safe_logout_before_assault",
            _ => "connected",
        }
    }

    pub const fn helper_group(self) -> &'static str {
        match self {
            Self::HelperSupported => "helper",
            _ => "solo",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoblinCrisisBalanceConfigSnapshot {
    pub pressure_max: i32,
    pub danger_unlocked_pressure: i32,
    pub three_structures_pressure: i32,
    pub villager_pressure: i32,
    pub explore_poi_pressure: i32,
    pub choose_expansion_pressure: i32,
    pub gold_tier_thresholds: Vec<i32>,
    pub gold_pressure_per_tier: i32,
    pub sanctuary_pressure_per_level: i32,
    pub sanctuary_pressure_max: i32,
    pub online_pressure_tier_ticks: Vec<i32>,
    pub online_pressure_per_tier: i32,
    pub signs_threshold: i32,
    pub pressure_threshold: i32,
    pub preparing_threshold: i32,
    pub assault_ready_threshold: i32,
    pub signs_min_online_ticks: i32,
    pub pressure_min_online_ticks: i32,
    pub preparing_min_online_ticks: i32,
    pub assault_ready_grace_ticks: i32,
    pub assault_max_online_wait_ticks: i32,
    pub preferred_launch_window: String,
    pub game_ticks_per_day: i32,
    pub preferred_launch_start_tick: i32,
    pub preferred_launch_wrap_end_tick: i32,
    pub assault_composition: Vec<String>,
    pub assault_vision: u32,
    pub fallback_spawn_min_distance: i32,
    pub fallback_spawn_max_distance: i32,
    pub sanctuary_spawn_min_offset_from_weak_radius: i32,
    pub sanctuary_spawn_max_offset_from_weak_radius: i32,
    pub neighbouring_structure_exclusion_distance: u32,
    pub spawn_candidate_limit: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CrisisPressureBreakdown {
    pub danger_unlocked: i32,
    pub structures: i32,
    pub villagers: i32,
    pub explore_poi: i32,
    pub choose_expansion: i32,
    pub stored_gold: i32,
    pub sanctuary: i32,
    pub online_time: i32,
    pub raw_total: i32,
    pub clamped_total: i32,
}

impl CrisisPressureBreakdown {
    pub fn contributor_sum(self) -> i32 {
        self.danger_unlocked
            .saturating_add(self.structures)
            .saturating_add(self.villagers)
            .saturating_add(self.explore_poi)
            .saturating_add(self.choose_expansion)
            .saturating_add(self.stored_gold)
            .saturating_add(self.sanctuary)
            .saturating_add(self.online_time)
    }

    pub fn dominant_contributor(self) -> Option<&'static str> {
        let contributors = [
            ("danger_unlocked", self.danger_unlocked),
            ("structures", self.structures),
            ("villagers", self.villagers),
            ("explore_poi", self.explore_poi),
            ("choose_expansion", self.choose_expansion),
            ("stored_gold", self.stored_gold),
            ("sanctuary", self.sanctuary),
            ("online_time", self.online_time),
        ];
        contributors
            .into_iter()
            .filter(|(_, value)| *value > 0)
            .reduce(|best, candidate| {
                if candidate.1 > best.1 {
                    candidate
                } else {
                    best
                }
            })
            .map(|(name, _)| name)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisPressureSnapshot {
    pub game_tick: i32,
    pub online_active_ticks: i32,
    pub phase: String,
    pub breakdown: CrisisPressureBreakdown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisPressureSnapshots {
    pub creation: Option<CrisisPressureSnapshot>,
    pub signs: Option<CrisisPressureSnapshot>,
    pub pressure: Option<CrisisPressureSnapshot>,
    pub preparing: Option<CrisisPressureSnapshot>,
    pub assault_ready: Option<CrisisPressureSnapshot>,
    pub assault_launch: Option<CrisisPressureSnapshot>,
    pub resolution: Option<CrisisPressureSnapshot>,
    pub final_snapshot: Option<CrisisPressureSnapshot>,
    pub periodic: Vec<CrisisPressureSnapshot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisPhaseTimingTelemetry {
    pub crisis_created_tick: Option<i32>,
    pub signs_entered_tick: Option<i32>,
    pub pressure_entered_tick: Option<i32>,
    pub preparing_entered_tick: Option<i32>,
    pub assault_ready_entered_tick: Option<i32>,
    pub assault_active_entered_tick: Option<i32>,
    pub resolved_tick: Option<i32>,
    pub crisis_created_online_tick: Option<i32>,
    pub signs_entered_online_tick: Option<i32>,
    pub pressure_entered_online_tick: Option<i32>,
    pub preparing_entered_online_tick: Option<i32>,
    pub assault_ready_entered_online_tick: Option<i32>,
    pub assault_active_entered_online_tick: Option<i32>,
    pub resolved_online_tick: Option<i32>,
}

impl CrisisPhaseTimingTelemetry {
    pub fn record_phase(&mut self, phase: CrisisPhase, game_tick: i32, online_tick: i32) {
        let (global, online) = match phase {
            CrisisPhase::Dormant => (
                &mut self.crisis_created_tick,
                &mut self.crisis_created_online_tick,
            ),
            CrisisPhase::Signs => (
                &mut self.signs_entered_tick,
                &mut self.signs_entered_online_tick,
            ),
            CrisisPhase::Pressure => (
                &mut self.pressure_entered_tick,
                &mut self.pressure_entered_online_tick,
            ),
            CrisisPhase::Preparing => (
                &mut self.preparing_entered_tick,
                &mut self.preparing_entered_online_tick,
            ),
            CrisisPhase::AssaultReady => (
                &mut self.assault_ready_entered_tick,
                &mut self.assault_ready_entered_online_tick,
            ),
            CrisisPhase::AssaultActive => (
                &mut self.assault_active_entered_tick,
                &mut self.assault_active_entered_online_tick,
            ),
            CrisisPhase::Resolved => (&mut self.resolved_tick, &mut self.resolved_online_tick),
        };
        if global.is_none() {
            *global = Some(game_tick);
            *online = Some(online_tick);
        }
    }

    fn duration(start: Option<i32>, end: Option<i32>) -> Option<i32> {
        Some(end?.saturating_sub(start?).max(0))
    }

    pub fn dormant_duration(&self) -> Option<i32> {
        Self::duration(self.crisis_created_tick, self.signs_entered_tick)
    }

    pub fn signs_duration(&self) -> Option<i32> {
        Self::duration(self.signs_entered_tick, self.pressure_entered_tick)
    }

    pub fn pressure_duration(&self) -> Option<i32> {
        Self::duration(self.pressure_entered_tick, self.preparing_entered_tick)
    }

    pub fn preparing_duration(&self) -> Option<i32> {
        Self::duration(self.preparing_entered_tick, self.assault_ready_entered_tick)
    }

    pub fn assault_ready_duration(&self) -> Option<i32> {
        Self::duration(
            self.assault_ready_entered_tick,
            self.assault_active_entered_tick,
        )
    }

    pub fn assault_duration(&self) -> Option<i32> {
        Self::duration(self.assault_active_entered_tick, self.resolved_tick)
    }

    pub fn total_crisis_duration(&self) -> Option<i32> {
        Self::duration(self.crisis_created_tick, self.resolved_tick)
    }

    pub fn total_online_before_launch(&self) -> Option<i32> {
        Self::duration(
            self.crisis_created_online_tick,
            self.assault_active_entered_online_tick,
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisPreparationSnapshot {
    pub game_tick: i32,
    pub phase: String,
    pub hero_class: String,
    pub hero_template: String,
    pub hero_health: i32,
    pub hero_max_health: i32,
    pub equipped_weapon: Option<String>,
    pub equipped_armor_count: i32,
    pub healing_items: i32,
    pub food_items: i32,
    pub drink_items: i32,
    pub completed_structures: i32,
    pub foundations: i32,
    pub wall_segments: i32,
    pub wall_total_health: i32,
    pub wall_total_max_health: i32,
    pub stockades: i32,
    pub palisades: i32,
    pub watchtowers: i32,
    pub villagers_alive: i32,
    pub villagers_combat_capable: i32,
    pub sanctuary_level: i32,
    pub stored_gold: i32,
    pub stored_food: i32,
    pub stored_resources_total: i32,
    pub hero_near_settlement: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisPreparationSnapshots {
    pub preparing: Option<CrisisPreparationSnapshot>,
    pub assault_ready: Option<CrisisPreparationSnapshot>,
    pub assault_launch: Option<CrisisPreparationSnapshot>,
    pub resolution_or_end: Option<CrisisPreparationSnapshot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisPreparationActions {
    pub structures_built: i32,
    pub walls_built: i32,
    pub structures_repaired: i32,
    pub equipment_changes: i32,
    pub healing_items_acquired: i32,
    pub villagers_recruited: i32,
    pub villager_assignments_changed: i32,
    pub sanctuary_upgrades: i32,
    pub resource_units_acquired: i32,
    pub storage_units_added: i32,
    pub online_ticks_near_settlement: i32,
    pub online_ticks_away_from_settlement: i32,
    pub returned_to_settlement_after_warning: bool,
    pub performed_preparation_action: bool,
}

impl CrisisPreparationActions {
    pub fn mark_action(&mut self) {
        self.performed_preparation_action = true;
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisAssaultOutcomeTelemetry {
    pub assault_launched: bool,
    pub assault_resolved: bool,
    pub assault_unit_count: i32,
    pub assault_units_defeated: i32,
    pub assault_units_remaining: i32,
    pub assault_duration_ticks: Option<i32>,
    pub hero_damage_taken: i32,
    pub hero_deaths_during_assault: i32,
    pub hero_alive_at_resolution: Option<bool>,
    pub villagers_at_launch: i32,
    pub villagers_killed: i32,
    pub structures_at_launch: i32,
    pub structures_damaged: i32,
    pub structures_destroyed: i32,
    pub wall_segments_at_launch: i32,
    pub wall_segments_destroyed: i32,
    pub total_structure_damage: i32,
    pub total_villager_damage: i32,
    pub player_kills: i32,
    pub villager_kills: i32,
    pub helper_kills: i32,
    pub defence_or_other_kills: i32,
    pub ordinary_disconnect_during_assault: bool,
    pub reconnected_during_assault: bool,
    pub resolved_while_owner_offline: bool,
    pub safe_logout_before_assault: bool,
    pub helper_participated: bool,
    pub cross_player_target_violations: i32,
    #[serde(skip)]
    credited_defeated_ids: BTreeSet<i32>,
    #[serde(skip)]
    assault_unit_ids: BTreeSet<i32>,
    #[serde(skip)]
    damaged_structure_ids: BTreeSet<i32>,
    #[serde(skip)]
    destroyed_structure_ids: BTreeSet<i32>,
    #[serde(skip)]
    killed_villager_ids: BTreeSet<i32>,
}

impl CrisisAssaultOutcomeTelemetry {
    pub(crate) fn record_launch_units(&mut self, unit_ids: &[i32]) {
        self.assault_launched = true;
        self.assault_unit_ids = unit_ids.iter().copied().collect();
        self.assault_unit_count = self.assault_unit_ids.len() as i32;
        self.assault_units_remaining = self.assault_unit_count;
    }

    fn tracks_assault_unit(&self, id: i32) -> bool {
        self.assault_unit_ids.contains(&id)
    }

    pub(crate) fn record_hero_lifecycle_transition(
        &mut self,
        previous_phase: Option<CrisisPhase>,
        current_phase: Option<CrisisPhase>,
        was_alive: bool,
        is_alive: bool,
    ) {
        if previous_phase == Some(CrisisPhase::AssaultActive)
            && matches!(
                current_phase,
                Some(CrisisPhase::AssaultActive | CrisisPhase::Resolved)
            )
            && was_alive
            && !is_alive
        {
            self.hero_deaths_during_assault = self.hero_deaths_during_assault.saturating_add(1);
        }
    }

    fn record_defeat(
        &mut self,
        target_id: i32,
        attacker_player_id: i32,
        attacker: Subclass,
        owner: i32,
    ) {
        if !self.credited_defeated_ids.insert(target_id) {
            return;
        }
        self.assault_units_defeated = self.assault_units_defeated.saturating_add(1);
        self.assault_units_remaining = self.assault_units_remaining.saturating_sub(1).max(0);
        match (attacker_player_id == owner, attacker) {
            (true, Subclass::Hero) => self.player_kills = self.player_kills.saturating_add(1),
            (true, Subclass::Villager) => {
                self.villager_kills = self.villager_kills.saturating_add(1)
            }
            (false, _) if attacker_player_id > 0 && attacker_player_id < NPC_PLAYER_ID => {
                self.helper_kills = self.helper_kills.saturating_add(1);
                self.helper_participated = true;
            }
            _ => self.defence_or_other_kills = self.defence_or_other_kills.saturating_add(1),
        }
    }

    fn record_incoming_damage(
        &mut self,
        target_id: i32,
        target_subclass: Subclass,
        target_is_structure: bool,
        amount: i32,
        killed: bool,
    ) {
        let amount = amount.max(0);
        match target_subclass {
            Subclass::Hero => {
                self.hero_damage_taken = self.hero_damage_taken.saturating_add(amount);
            }
            Subclass::Villager => {
                self.total_villager_damage = self.total_villager_damage.saturating_add(amount);
                if killed && self.killed_villager_ids.insert(target_id) {
                    self.villagers_killed = self.villagers_killed.saturating_add(1);
                }
            }
            _ if target_is_structure => {
                self.total_structure_damage = self.total_structure_damage.saturating_add(amount);
                if amount > 0 {
                    self.damaged_structure_ids.insert(target_id);
                    self.structures_damaged = self.damaged_structure_ids.len() as i32;
                }
                if killed && self.destroyed_structure_ids.insert(target_id) {
                    self.structures_destroyed = self.structures_destroyed.saturating_add(1);
                    if target_subclass == Subclass::Wall {
                        self.wall_segments_destroyed =
                            self.wall_segments_destroyed.saturating_add(1);
                    }
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisWarningTelemetry {
    pub signs_delivery_tick: Option<i32>,
    pub preparing_delivery_tick: Option<i32>,
    pub assault_ready_delivery_tick: Option<i32>,
    pub assault_launch_delivery_tick: Option<i32>,
    pub signs_delivery_online_tick: Option<i32>,
    pub preparing_delivery_online_tick: Option<i32>,
    pub assault_ready_delivery_online_tick: Option<i32>,
    pub assault_launch_delivery_online_tick: Option<i32>,
    pub signs_delivered_online: Option<bool>,
    pub preparing_delivered_online: Option<bool>,
    pub assault_ready_delivered_online: Option<bool>,
    pub assault_launch_delivered_online: Option<bool>,
    pub signs_near_settlement: Option<bool>,
    pub preparing_near_settlement: Option<bool>,
    pub assault_ready_near_settlement: Option<bool>,
    pub assault_launch_near_settlement: Option<bool>,
}

impl CrisisWarningTelemetry {
    pub fn record(
        &mut self,
        phase: CrisisPhase,
        game_tick: i32,
        online_tick: i32,
        online: bool,
        near_settlement: bool,
    ) {
        let fields = match phase {
            CrisisPhase::Signs => Some((
                &mut self.signs_delivery_tick,
                &mut self.signs_delivery_online_tick,
                &mut self.signs_delivered_online,
                &mut self.signs_near_settlement,
            )),
            CrisisPhase::Preparing => Some((
                &mut self.preparing_delivery_tick,
                &mut self.preparing_delivery_online_tick,
                &mut self.preparing_delivered_online,
                &mut self.preparing_near_settlement,
            )),
            CrisisPhase::AssaultReady => Some((
                &mut self.assault_ready_delivery_tick,
                &mut self.assault_ready_delivery_online_tick,
                &mut self.assault_ready_delivered_online,
                &mut self.assault_ready_near_settlement,
            )),
            CrisisPhase::AssaultActive => Some((
                &mut self.assault_launch_delivery_tick,
                &mut self.assault_launch_delivery_online_tick,
                &mut self.assault_launch_delivered_online,
                &mut self.assault_launch_near_settlement,
            )),
            _ => None,
        };
        let Some((tick, online_tick_field, online_field, near_field)) = fields else {
            return;
        };
        if tick.is_none() {
            *tick = Some(game_tick);
            *online_tick_field = Some(online_tick);
            *online_field = Some(online);
            *near_field = Some(near_settlement);
        }
    }

    pub fn preparing_to_launch_online_ticks(&self) -> Option<i32> {
        Self::duration(
            self.preparing_delivery_online_tick,
            self.assault_launch_delivery_online_tick,
        )
    }

    pub fn ready_to_launch_online_ticks(&self) -> Option<i32> {
        Self::duration(
            self.assault_ready_delivery_online_tick,
            self.assault_launch_delivery_online_tick,
        )
    }

    pub fn signs_to_launch_global_ticks(&self) -> Option<i32> {
        Self::duration(self.signs_delivery_tick, self.assault_launch_delivery_tick)
    }

    pub fn signs_to_launch_online_ticks(&self) -> Option<i32> {
        Self::duration(
            self.signs_delivery_online_tick,
            self.assault_launch_delivery_online_tick,
        )
    }

    fn duration(start: Option<i32>, end: Option<i32>) -> Option<i32> {
        Some(end?.saturating_sub(start?).max(0))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisBalanceTelemetry {
    pub latest_pressure: CrisisPressureBreakdown,
    pub pressure_snapshots: CrisisPressureSnapshots,
    pub phase_timing: CrisisPhaseTimingTelemetry,
    pub preparation_snapshots: CrisisPreparationSnapshots,
    pub preparation_actions: CrisisPreparationActions,
    pub assault_outcome: CrisisAssaultOutcomeTelemetry,
    pub warnings: CrisisWarningTelemetry,
    pub latest_near_settlement: bool,
    pub latest_online: bool,
    #[serde(skip)]
    pub(crate) latest_hero_alive: Option<bool>,
    #[serde(skip)]
    pub(crate) latest_phase: Option<CrisisPhase>,
}

impl CrisisBalanceTelemetry {
    pub fn record_pressure(
        &mut self,
        phase: CrisisPhase,
        game_tick: i32,
        online_active_ticks: i32,
        breakdown: CrisisPressureBreakdown,
    ) {
        self.latest_pressure = breakdown;
        self.pressure_snapshots.final_snapshot = Some(CrisisPressureSnapshot {
            game_tick,
            online_active_ticks,
            phase: phase_name(phase).to_string(),
            breakdown,
        });
    }

    pub fn record_phase(&mut self, phase: CrisisPhase, game_tick: i32, online_active_ticks: i32) {
        self.phase_timing
            .record_phase(phase, game_tick, online_active_ticks);
        let snapshot = CrisisPressureSnapshot {
            game_tick,
            online_active_ticks,
            phase: phase_name(phase).to_string(),
            breakdown: self.latest_pressure,
        };
        let destination = match phase {
            CrisisPhase::Dormant => &mut self.pressure_snapshots.creation,
            CrisisPhase::Signs => &mut self.pressure_snapshots.signs,
            CrisisPhase::Pressure => &mut self.pressure_snapshots.pressure,
            CrisisPhase::Preparing => &mut self.pressure_snapshots.preparing,
            CrisisPhase::AssaultReady => &mut self.pressure_snapshots.assault_ready,
            CrisisPhase::AssaultActive => &mut self.pressure_snapshots.assault_launch,
            CrisisPhase::Resolved => &mut self.pressure_snapshots.resolution,
        };
        if destination.is_none() {
            *destination = Some(snapshot);
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct CrisisBalanceTelemetryState(pub HashMap<i32, CrisisBalanceTelemetry>);

impl CrisisBalanceTelemetryState {
    pub fn get(&self, player_id: &i32) -> Option<&CrisisBalanceTelemetry> {
        self.0.get(player_id)
    }

    pub fn get_mut(&mut self, player_id: &i32) -> Option<&mut CrisisBalanceTelemetry> {
        self.0.get_mut(player_id)
    }

    pub fn entry(
        &mut self,
        player_id: i32,
    ) -> std::collections::hash_map::Entry<'_, i32, CrisisBalanceTelemetry> {
        self.0.entry(player_id)
    }

    pub fn remove(&mut self, player_id: &i32) -> Option<CrisisBalanceTelemetry> {
        self.0.remove(player_id)
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrisisBalanceTelemetryConfig {
    /// Disabled in production. Headless balance runs may opt into bounded
    /// periodic snapshots in addition to transition snapshots.
    pub sample_interval_ticks: Option<i32>,
}

impl Default for CrisisBalanceTelemetryConfig {
    fn default() -> Self {
        Self {
            sample_interval_ticks: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CrisisBalanceObservation {
    pub tick: i32,
    pub online_active_ticks: i32,
    pub phase: Option<CrisisPhase>,
    pub completed_structure_ids: BTreeSet<i32>,
    pub wall_ids: BTreeSet<i32>,
    pub structure_health: HashMap<i32, i32>,
    pub equipped_weapon: Option<String>,
    pub equipped_armor_count: i32,
    pub healing_items: i32,
    pub villagers: BTreeSet<i32>,
    pub villager_assignments: HashMap<i32, i32>,
    pub sanctuary_level: i32,
    pub total_run_items: i32,
    pub stored_items: i32,
    pub online: bool,
    pub near_settlement: bool,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct CrisisBalanceObservationState(pub HashMap<i32, CrisisBalanceObservation>);

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct CrisisCombatTelemetryEvent {
    pub entity: Entity,
    pub attacker_id: i32,
    pub attacker_player_id: i32,
    pub attacker_subclass: Subclass,
    pub attacker_crisis: Option<CrisisAssaultUnit>,
    pub target_id: i32,
    pub target_player_id: i32,
    pub target_subclass: Subclass,
    pub target_is_structure: bool,
    pub target_crisis: Option<CrisisAssaultUnit>,
    pub effective_damage: i32,
    pub killed: bool,
}

pub(crate) fn crisis_combat_telemetry_observer(
    event: On<CrisisCombatTelemetryEvent>,
    crisis_state: Res<SettlementCrisisState>,
    mut telemetry_state: ResMut<CrisisBalanceTelemetryState>,
) {
    if let Some(source) = event.attacker_crisis {
        let source_metadata_is_current = crisis_state
            .get(&source.owner_player_id)
            .map(|crisis| {
                crisis.assault_id == Some(source.assault_id)
                    && crisis.assault_spawn_generation == source.spawn_generation
                    && matches!(
                        crisis.phase,
                        CrisisPhase::AssaultActive | CrisisPhase::Resolved
                    )
            })
            .unwrap_or(false);
        if source_metadata_is_current {
            if let Some(telemetry) = telemetry_state.get_mut(&source.owner_player_id) {
                if telemetry
                    .assault_outcome
                    .tracks_assault_unit(event.attacker_id)
                {
                    if event.target_player_id != source.owner_player_id {
                        telemetry.assault_outcome.cross_player_target_violations = telemetry
                            .assault_outcome
                            .cross_player_target_violations
                            .saturating_add(1);
                    } else {
                        telemetry.assault_outcome.record_incoming_damage(
                            event.target_id,
                            event.target_subclass,
                            event.target_is_structure,
                            event.effective_damage,
                            event.killed,
                        );
                    }
                }
            }
        }
    }

    if let Some(target) = event.target_crisis {
        let target_is_current = crisis_state
            .get(&target.owner_player_id)
            .map(|crisis| {
                crisis.assault_id == Some(target.assault_id)
                    && crisis.assault_spawn_generation == target.spawn_generation
                    && matches!(
                        crisis.phase,
                        CrisisPhase::AssaultActive | CrisisPhase::Resolved
                    )
            })
            .unwrap_or(false);
        if target_is_current {
            if let Some(telemetry) = telemetry_state.get_mut(&target.owner_player_id) {
                if telemetry
                    .assault_outcome
                    .tracks_assault_unit(event.target_id)
                {
                    if event.effective_damage > 0
                        && event.attacker_player_id > 0
                        && event.attacker_player_id < NPC_PLAYER_ID
                        && event.attacker_player_id != target.owner_player_id
                    {
                        telemetry.assault_outcome.helper_participated = true;
                    }
                    if event.killed {
                        telemetry.assault_outcome.record_defeat(
                            event.target_id,
                            event.attacker_player_id,
                            event.attacker_subclass,
                            target.owner_player_id,
                        );
                    }
                }
            }
        }
    }
}

pub const fn phase_name(phase: CrisisPhase) -> &'static str {
    match phase {
        CrisisPhase::Dormant => "dormant",
        CrisisPhase::Signs => "signs",
        CrisisPhase::Pressure => "pressure",
        CrisisPhase::Preparing => "preparing",
        CrisisPhase::AssaultReady => "assault_ready",
        CrisisPhase::AssaultActive => "assault_active",
        CrisisPhase::Resolved => "resolved",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::goblin_crisis_balance_config_snapshot;

    fn full_pressure_breakdown() -> CrisisPressureBreakdown {
        CrisisPressureBreakdown {
            danger_unlocked: 10,
            structures: 20,
            villagers: 15,
            explore_poi: 10,
            choose_expansion: 15,
            stored_gold: 15,
            sanctuary: 10,
            online_time: 15,
            raw_total: 110,
            clamped_total: 100,
        }
    }

    #[test]
    fn pressure_breakdown_matches_the_read_only_configuration_and_clamps() {
        let config = goblin_crisis_balance_config_snapshot();
        assert_eq!(config.pressure_max, 100);
        assert_eq!(config.danger_unlocked_pressure, 10);
        assert_eq!(config.three_structures_pressure, 20);
        assert_eq!(config.villager_pressure, 15);
        assert_eq!(config.explore_poi_pressure, 10);
        assert_eq!(config.choose_expansion_pressure, 15);
        assert_eq!(config.gold_pressure_per_tier, 5);
        assert_eq!(config.sanctuary_pressure_per_level, 2);
        assert_eq!(config.sanctuary_pressure_max, 10);
        assert_eq!(config.online_pressure_per_tier, 5);

        let breakdown = full_pressure_breakdown();
        assert_eq!(breakdown.contributor_sum(), 110);
        assert_eq!(breakdown.raw_total, breakdown.contributor_sum());
        assert_eq!(
            breakdown.clamped_total,
            breakdown.raw_total.min(config.pressure_max)
        );
    }

    #[test]
    fn pressure_breakdown_uses_saturating_totals_and_identifies_the_dominant_source() {
        let breakdown = CrisisPressureBreakdown {
            danger_unlocked: i32::MAX,
            structures: 20,
            ..CrisisPressureBreakdown::default()
        };
        assert_eq!(breakdown.contributor_sum(), i32::MAX);
        assert_eq!(breakdown.dominant_contributor(), Some("danger_unlocked"));
        assert_eq!(
            CrisisPressureBreakdown::default().dominant_contributor(),
            None
        );
    }

    #[test]
    fn phase_timing_records_first_global_and_online_ticks_idempotently() {
        let mut timing = CrisisPhaseTimingTelemetry::default();
        timing.record_phase(CrisisPhase::Dormant, 100, 0);
        timing.record_phase(CrisisPhase::Signs, 140, 25);
        timing.record_phase(CrisisPhase::Signs, 999, 777);
        timing.record_phase(CrisisPhase::Pressure, 220, 75);
        timing.record_phase(CrisisPhase::Preparing, 400, 150);
        timing.record_phase(CrisisPhase::AssaultReady, 700, 300);
        timing.record_phase(CrisisPhase::AssaultActive, 760, 360);
        timing.record_phase(CrisisPhase::Resolved, 900, 500);

        assert_eq!(timing.signs_entered_tick, Some(140));
        assert_eq!(timing.signs_entered_online_tick, Some(25));
        assert_eq!(timing.dormant_duration(), Some(40));
        assert_eq!(timing.signs_duration(), Some(80));
        assert_eq!(timing.pressure_duration(), Some(180));
        assert_eq!(timing.preparing_duration(), Some(300));
        assert_eq!(timing.assault_ready_duration(), Some(60));
        assert_eq!(timing.assault_duration(), Some(140));
        assert_eq!(timing.total_crisis_duration(), Some(800));
        assert_eq!(timing.total_online_before_launch(), Some(360));
    }

    #[test]
    fn phase_timing_durations_require_both_boundaries_and_never_go_negative() {
        let mut timing = CrisisPhaseTimingTelemetry::default();
        timing.record_phase(CrisisPhase::Signs, 200, 50);
        assert_eq!(timing.dormant_duration(), None);
        assert_eq!(timing.signs_duration(), None);

        timing.record_phase(CrisisPhase::Pressure, 150, 40);
        assert_eq!(timing.signs_duration(), Some(0));
    }

    #[test]
    fn preparation_action_marker_is_idempotent_and_preserves_deltas() {
        let mut actions = CrisisPreparationActions {
            structures_built: 2,
            walls_built: 3,
            healing_items_acquired: 1,
            resource_units_acquired: 12,
            ..CrisisPreparationActions::default()
        };
        assert!(!actions.performed_preparation_action);
        actions.mark_action();
        actions.mark_action();

        assert!(actions.performed_preparation_action);
        assert_eq!(actions.structures_built, 2);
        assert_eq!(actions.walls_built, 3);
        assert_eq!(actions.healing_items_acquired, 1);
        assert_eq!(actions.resource_units_acquired, 12);
    }

    #[test]
    fn warning_timing_records_first_delivery_context_and_online_durations() {
        let mut warnings = CrisisWarningTelemetry::default();
        warnings.record(CrisisPhase::Dormant, 5, 1, true, false);
        assert_eq!(warnings.signs_delivery_tick, None);

        warnings.record(CrisisPhase::Signs, 100, 20, true, false);
        warnings.record(CrisisPhase::Signs, 999, 999, false, true);
        warnings.record(CrisisPhase::Preparing, 300, 100, true, false);
        warnings.record(CrisisPhase::AssaultReady, 500, 180, true, true);
        warnings.record(CrisisPhase::AssaultActive, 650, 250, true, true);

        assert_eq!(warnings.signs_delivery_tick, Some(100));
        assert_eq!(warnings.signs_delivery_online_tick, Some(20));
        assert_eq!(warnings.signs_delivered_online, Some(true));
        assert_eq!(warnings.signs_near_settlement, Some(false));
        assert_eq!(warnings.signs_to_launch_global_ticks(), Some(550));
        assert_eq!(warnings.signs_to_launch_online_ticks(), Some(230));
        assert_eq!(warnings.preparing_to_launch_online_ticks(), Some(150));
        assert_eq!(warnings.ready_to_launch_online_ticks(), Some(70));
    }

    #[test]
    fn warning_durations_require_launch_and_saturate_out_of_order_observations() {
        let mut warnings = CrisisWarningTelemetry::default();
        warnings.record(CrisisPhase::Preparing, 300, 100, true, true);
        assert_eq!(warnings.signs_to_launch_global_ticks(), None);
        assert_eq!(warnings.signs_to_launch_online_ticks(), None);
        assert_eq!(warnings.preparing_to_launch_online_ticks(), None);
        assert_eq!(warnings.ready_to_launch_online_ticks(), None);

        warnings.record(CrisisPhase::Signs, 600, 120, true, true);
        warnings.record(CrisisPhase::AssaultReady, 400, 90, true, true);
        warnings.record(CrisisPhase::AssaultActive, 500, 80, true, true);
        assert_eq!(warnings.signs_to_launch_global_ticks(), Some(0));
        assert_eq!(warnings.signs_to_launch_online_ticks(), Some(0));
        assert_eq!(warnings.preparing_to_launch_online_ticks(), Some(0));
        assert_eq!(warnings.ready_to_launch_online_ticks(), Some(0));
    }

    #[test]
    fn assault_defeat_attribution_is_deduplicated_and_classified() {
        let owner = 7;
        let mut outcome = CrisisAssaultOutcomeTelemetry {
            assault_unit_count: 5,
            assault_units_remaining: 5,
            ..CrisisAssaultOutcomeTelemetry::default()
        };

        outcome.record_defeat(101, owner, Subclass::Hero, owner);
        outcome.record_defeat(101, owner, Subclass::Hero, owner);
        outcome.record_defeat(102, owner, Subclass::Villager, owner);
        outcome.record_defeat(103, 8, Subclass::Hero, owner);
        outcome.record_defeat(104, 1_000, Subclass::Npc, owner);

        assert_eq!(outcome.assault_units_defeated, 4);
        assert_eq!(outcome.assault_units_remaining, 1);
        assert_eq!(outcome.player_kills, 1);
        assert_eq!(outcome.villager_kills, 1);
        assert_eq!(outcome.helper_kills, 1);
        assert_eq!(outcome.defence_or_other_kills, 1);
        assert!(outcome.helper_participated);
    }

    #[test]
    fn incoming_damage_counts_effective_damage_and_deduplicates_destroyed_entities() {
        let mut outcome = CrisisAssaultOutcomeTelemetry::default();
        outcome.record_incoming_damage(1, Subclass::Hero, false, 12, true);
        outcome.record_incoming_damage(1, Subclass::Hero, false, -5, true);
        outcome.record_incoming_damage(2, Subclass::Villager, false, 9, true);
        outcome.record_incoming_damage(2, Subclass::Villager, false, 3, true);
        outcome.record_incoming_damage(3, Subclass::Wall, true, 20, false);
        outcome.record_incoming_damage(3, Subclass::Wall, true, 4, true);
        outcome.record_incoming_damage(3, Subclass::Wall, true, 0, true);
        outcome.record_incoming_damage(4, Subclass::Storage, true, 7, true);
        outcome.record_incoming_damage(5, Subclass::Npc, false, 99, true);

        assert_eq!(outcome.hero_damage_taken, 12);
        assert_eq!(outcome.hero_deaths_during_assault, 0);
        assert_eq!(outcome.total_villager_damage, 12);
        assert_eq!(outcome.villagers_killed, 1);
        assert_eq!(outcome.total_structure_damage, 31);
        assert_eq!(outcome.structures_damaged, 2);
        assert_eq!(outcome.structures_destroyed, 2);
        assert_eq!(outcome.wall_segments_destroyed, 1);
    }

    #[test]
    fn hero_deaths_count_lifecycle_transitions_from_any_cause_once_per_life() {
        let mut outcome = CrisisAssaultOutcomeTelemetry::default();
        outcome.record_hero_lifecycle_transition(
            Some(CrisisPhase::AssaultActive),
            Some(CrisisPhase::AssaultActive),
            true,
            false,
        );
        outcome.record_hero_lifecycle_transition(
            Some(CrisisPhase::AssaultActive),
            Some(CrisisPhase::AssaultActive),
            false,
            false,
        );
        outcome.record_hero_lifecycle_transition(
            Some(CrisisPhase::AssaultActive),
            Some(CrisisPhase::Resolved),
            true,
            false,
        );
        outcome.record_hero_lifecycle_transition(
            Some(CrisisPhase::Preparing),
            Some(CrisisPhase::AssaultActive),
            true,
            false,
        );

        assert_eq!(outcome.hero_deaths_during_assault, 2);
    }

    #[test]
    fn balance_telemetry_keeps_first_transition_snapshot_and_latest_pressure() {
        let mut telemetry = CrisisBalanceTelemetry::default();
        let first = CrisisPressureBreakdown {
            raw_total: 20,
            clamped_total: 20,
            ..CrisisPressureBreakdown::default()
        };
        let later = CrisisPressureBreakdown {
            raw_total: 45,
            clamped_total: 45,
            ..CrisisPressureBreakdown::default()
        };

        telemetry.record_pressure(CrisisPhase::Signs, 100, 20, first);
        telemetry.record_phase(CrisisPhase::Signs, 100, 20);
        telemetry.record_pressure(CrisisPhase::Signs, 200, 80, later);
        telemetry.record_phase(CrisisPhase::Signs, 200, 80);

        let signs = telemetry.pressure_snapshots.signs.as_ref().unwrap();
        assert_eq!(signs.game_tick, 100);
        assert_eq!(signs.breakdown, first);
        assert_eq!(telemetry.latest_pressure, later);
        assert_eq!(
            telemetry
                .pressure_snapshots
                .final_snapshot
                .as_ref()
                .map(|snapshot| snapshot.game_tick),
            Some(200)
        );
    }

    #[test]
    fn combat_observer_accepts_only_tracked_current_assault_attribution() {
        use crate::game::SettlementCrisis;

        let owner = 7;
        let attribution = CrisisAssaultUnit {
            owner_player_id: owner,
            assault_id: 42,
            spawn_generation: 3,
        };
        let mut app = App::new();
        app.init_resource::<SettlementCrisisState>()
            .init_resource::<CrisisBalanceTelemetryState>()
            .add_observer(crisis_combat_telemetry_observer);
        app.world_mut()
            .resource_mut::<SettlementCrisisState>()
            .0
            .insert(
                owner,
                SettlementCrisis {
                    phase: CrisisPhase::AssaultActive,
                    assault_id: Some(attribution.assault_id),
                    assault_spawn_generation: attribution.spawn_generation,
                    ..SettlementCrisis::default()
                },
            );
        app.world_mut()
            .resource_mut::<CrisisBalanceTelemetryState>()
            .entry(owner)
            .or_default()
            .assault_outcome
            .record_launch_units(&[101, 102]);
        let entity = app.world_mut().spawn_empty().id();
        let event = |attacker_id,
                     attacker_player_id,
                     attacker_crisis,
                     target_id,
                     target_player_id,
                     target_crisis,
                     killed| {
            CrisisCombatTelemetryEvent {
                entity,
                attacker_id,
                attacker_player_id,
                attacker_subclass: Subclass::Hero,
                attacker_crisis,
                target_id,
                target_player_id,
                target_subclass: Subclass::Hero,
                target_is_structure: false,
                target_crisis,
                effective_damage: 5,
                killed,
            }
        };

        // Ambient damage and an untracked object with copied attribution do not count.
        app.world_mut()
            .trigger(event(500, owner, None, 1, owner, None, false));
        app.world_mut()
            .trigger(event(999, owner, Some(attribution), 1, owner, None, false));
        // Exact tracked source damage counts; a foreign target is an invariant violation.
        app.world_mut()
            .trigger(event(101, owner, Some(attribution), 1, owner, None, false));
        app.world_mut()
            .trigger(event(101, owner, Some(attribution), 2, 8, None, false));
        // A helper that deals nonlethal damage participates without receiving a kill.
        app.world_mut()
            .trigger(event(800, 8, None, 102, owner, Some(attribution), false));

        // A stale source must not suppress independent current-target kill attribution.
        let stale = CrisisAssaultUnit {
            assault_id: 41,
            ..attribution
        };
        app.world_mut().trigger(event(
            900,
            owner,
            Some(stale),
            102,
            owner,
            Some(attribution),
            true,
        ));
        // Resolution may clear authoritative IDs before the deferred final event; the
        // launch snapshot retains exact tracked IDs for this final attribution fold.
        app.world_mut()
            .resource_mut::<SettlementCrisisState>()
            .get_mut(&owner)
            .unwrap()
            .phase = CrisisPhase::Resolved;
        app.world_mut().trigger(event(
            owner,
            owner,
            None,
            101,
            owner,
            Some(attribution),
            true,
        ));

        let outcome = &app
            .world()
            .resource::<CrisisBalanceTelemetryState>()
            .get(&owner)
            .unwrap()
            .assault_outcome;
        assert_eq!(outcome.hero_damage_taken, 5);
        assert_eq!(outcome.cross_player_target_violations, 1);
        assert_eq!(outcome.assault_units_defeated, 2);
        assert_eq!(outcome.player_kills, 2);
        assert_eq!(outcome.helper_kills, 0);
        assert!(outcome.helper_participated);
    }
}
