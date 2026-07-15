//! Read-only balance instrumentation for the first personal goblin crisis.
//!
//! This module deliberately contains observations, not tuning controls. The
//! authoritative pressure, phase, launch, combat, and Safe Logout systems stay
//! in their existing modules; this layer records what those systems did.

use std::collections::{BTreeSet, HashMap};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{Target, TaskTarget};
use crate::constants::{NO_TARGET, NPC_PLAYER_ID};
use crate::effect::{Effect, Effects};
use crate::game::{CrisisAssaultUnit, CrisisPhase, GameTick, SettlementCrisisState};
use crate::item::{AttrKey, AttrVal, Inventory};
use crate::map::Map;
use crate::npc::VisibleTarget;
use crate::obj::{
    Class, ClassStructure, HeroClass, Id, LastAttacker, PlayerId, Position, State, StateDead,
    Stats, Subclass, SubclassHero, TrueDeath, Viewshed,
};
use crate::structure::Structure;

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
    pub repairs_started: i32,
    pub repairs_completed: i32,
    pub defensive_structures_started: i32,
    pub defensive_structures_completed: i32,
    pub healing_items_carried_at_launch: i32,
    pub healing_items_used_before_launch: i32,
    pub combat_capable_villagers_at_launch: i32,
    pub first_preparation_action_tick: Option<i32>,
    pub meaningful_preparation_categories: Vec<String>,
    pub meaningful_preparation_category_count: i32,
    #[serde(skip)]
    repair_starts: BTreeSet<i32>,
    #[serde(skip)]
    repair_completions: BTreeSet<i32>,
    #[serde(skip)]
    defensive_structure_starts: BTreeSet<i32>,
    #[serde(skip)]
    defensive_structure_completions: BTreeSet<i32>,
    #[serde(skip)]
    equipment_items: BTreeSet<i32>,
    #[serde(skip)]
    healing_use_events: BTreeSet<Uuid>,
    #[serde(skip)]
    recruited_villagers: BTreeSet<i32>,
    #[serde(skip)]
    reassigned_villagers: BTreeSet<i32>,
    #[serde(skip)]
    meaningful_category_keys: BTreeSet<CrisisPreparationCategory>,
    #[serde(skip)]
    healing_items_high_water: Option<i32>,
    #[serde(skip)]
    total_run_items_high_water: Option<i32>,
    #[serde(skip)]
    stored_items_high_water: Option<i32>,
    #[serde(skip)]
    sanctuary_level_high_water: Option<i32>,
    #[serde(skip)]
    launch_readiness_recorded: bool,
}

impl CrisisPreparationActions {
    pub fn mark_action(&mut self) {
        self.performed_preparation_action = true;
    }

    pub fn mark_action_at(&mut self, game_tick: i32) {
        self.mark_action();
        self.first_preparation_action_tick = Some(
            self.first_preparation_action_tick
                .map_or(game_tick, |first_tick| first_tick.min(game_tick)),
        );
    }

    pub fn record_repair_started(&mut self, structure_id: i32, game_tick: i32) -> bool {
        if !self.repair_starts.insert(structure_id) {
            return false;
        }
        self.repairs_started = self.repairs_started.saturating_add(1);
        self.record_meaningful_action(CrisisPreparationCategory::Repair, game_tick);
        true
    }

    pub fn record_repair_completed(&mut self, structure_id: i32, game_tick: i32) -> bool {
        if !self.repair_completions.insert(structure_id) {
            return false;
        }
        self.repairs_completed = self.repairs_completed.saturating_add(1);
        self.structures_repaired = self.structures_repaired.saturating_add(1);
        self.record_meaningful_action(CrisisPreparationCategory::Repair, game_tick);
        true
    }

    pub fn record_defensive_structure_started(
        &mut self,
        structure_id: i32,
        game_tick: i32,
    ) -> bool {
        if !self.defensive_structure_starts.insert(structure_id) {
            return false;
        }
        self.defensive_structures_started = self.defensive_structures_started.saturating_add(1);
        self.record_meaningful_action(CrisisPreparationCategory::Defenses, game_tick);
        true
    }

    pub fn record_defensive_structure_completed(
        &mut self,
        structure_id: i32,
        is_wall: bool,
        game_tick: i32,
    ) -> bool {
        if !self.defensive_structure_completions.insert(structure_id) {
            return false;
        }
        self.defensive_structures_completed = self.defensive_structures_completed.saturating_add(1);
        self.structures_built = self.structures_built.saturating_add(1);
        if is_wall {
            self.walls_built = self.walls_built.saturating_add(1);
        }
        self.record_meaningful_action(CrisisPreparationCategory::Defenses, game_tick);
        true
    }

    pub fn record_equipment_change(&mut self, item_id: i32, game_tick: i32) -> bool {
        if !self.equipment_items.insert(item_id) {
            return false;
        }
        self.equipment_changes = self.equipment_changes.saturating_add(1);
        self.record_meaningful_action(CrisisPreparationCategory::Equipment, game_tick);
        true
    }

    pub fn observe_healing_items(&mut self, healing_items: i32, game_tick: i32) -> i32 {
        let acquired =
            Self::record_high_water_delta(&mut self.healing_items_high_water, healing_items.max(0));
        if acquired > 0 {
            self.healing_items_acquired = self.healing_items_acquired.saturating_add(acquired);
            self.record_meaningful_action(CrisisPreparationCategory::Healing, game_tick);
        }
        acquired
    }

    pub fn record_healing_item_used_before_launch(
        &mut self,
        use_event_id: Uuid,
        game_tick: i32,
    ) -> bool {
        if !self.healing_use_events.insert(use_event_id) {
            return false;
        }
        self.healing_items_used_before_launch =
            self.healing_items_used_before_launch.saturating_add(1);
        self.record_meaningful_action(CrisisPreparationCategory::Healing, game_tick);
        true
    }

    pub fn record_villager_recruited(&mut self, villager_id: i32, game_tick: i32) -> bool {
        if !self.recruited_villagers.insert(villager_id) {
            return false;
        }
        self.villagers_recruited = self.villagers_recruited.saturating_add(1);
        self.record_meaningful_action(CrisisPreparationCategory::VillagerSupport, game_tick);
        true
    }

    pub fn record_villager_assignment_changed(&mut self, villager_id: i32, game_tick: i32) -> bool {
        if !self.reassigned_villagers.insert(villager_id) {
            return false;
        }
        self.villager_assignments_changed = self.villager_assignments_changed.saturating_add(1);
        self.record_meaningful_action(CrisisPreparationCategory::VillagerSupport, game_tick);
        true
    }

    pub fn observe_sanctuary_level(&mut self, sanctuary_level: i32, game_tick: i32) -> i32 {
        let upgrades = Self::record_high_water_delta(
            &mut self.sanctuary_level_high_water,
            sanctuary_level.max(0),
        );
        if upgrades > 0 {
            self.sanctuary_upgrades = self.sanctuary_upgrades.saturating_add(upgrades);
            self.record_meaningful_action(CrisisPreparationCategory::Sanctuary, game_tick);
        }
        upgrades
    }

    pub fn observe_total_run_items(&mut self, total_items: i32, game_tick: i32) -> i32 {
        let acquired =
            Self::record_high_water_delta(&mut self.total_run_items_high_water, total_items.max(0));
        if acquired > 0 {
            self.resource_units_acquired = self.resource_units_acquired.saturating_add(acquired);
            self.mark_action_at(game_tick);
        }
        acquired
    }

    pub fn observe_stored_items(&mut self, stored_items: i32, game_tick: i32) -> i32 {
        let added =
            Self::record_high_water_delta(&mut self.stored_items_high_water, stored_items.max(0));
        if added > 0 {
            self.storage_units_added = self.storage_units_added.saturating_add(added);
            self.mark_action_at(game_tick);
        }
        added
    }

    pub fn record_launch_readiness(
        &mut self,
        healing_items: i32,
        combat_capable_villager_ids: impl IntoIterator<Item = i32>,
    ) -> bool {
        if self.launch_readiness_recorded {
            return false;
        }
        self.launch_readiness_recorded = true;
        self.healing_items_carried_at_launch = healing_items.max(0);
        self.combat_capable_villagers_at_launch = combat_capable_villager_ids
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len() as i32;
        true
    }

    fn record_meaningful_action(&mut self, category: CrisisPreparationCategory, game_tick: i32) {
        self.mark_action_at(game_tick);
        if self.meaningful_category_keys.insert(category) {
            self.meaningful_preparation_categories = self
                .meaningful_category_keys
                .iter()
                .map(|category| category.label().to_string())
                .collect();
            self.meaningful_preparation_category_count = self.meaningful_category_keys.len() as i32;
        }
    }

    fn record_high_water_delta(high_water: &mut Option<i32>, observed: i32) -> i32 {
        let Some(previous_high_water) = *high_water else {
            *high_water = Some(observed);
            return 0;
        };
        if observed <= previous_high_water {
            return 0;
        }
        *high_water = Some(observed);
        observed.saturating_sub(previous_high_water)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CrisisPreparationCategory {
    Defenses,
    Equipment,
    Healing,
    Repair,
    Sanctuary,
    VillagerSupport,
}

impl CrisisPreparationCategory {
    const fn label(self) -> &'static str {
        match self {
            Self::Defenses => "defenses",
            Self::Equipment => "equipment",
            Self::Healing => "healing",
            Self::Repair => "repair",
            Self::Sanctuary => "sanctuary",
            Self::VillagerSupport => "villager_support",
        }
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

/// Opt-in, read-only instrumentation for the assault engagement pipeline.
///
/// The serialized fields are bounded first-event timestamps and counters. The
/// skipped fields retain just enough prior observation state to make those
/// aggregates idempotent; none of this state is read by gameplay systems.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrisisEngagementTelemetry {
    pub assault_launch_tick: Option<i32>,
    pub first_attacker_visible_tick: Option<i32>,
    pub first_attacker_target_acquired_tick: Option<i32>,
    pub first_hero_target_acquired_tick: Option<i32>,
    pub first_hero_move_toward_attacker_tick: Option<i32>,
    pub first_attacker_move_toward_target_tick: Option<i32>,
    pub first_hero_attack_requested_tick: Option<i32>,
    pub first_hero_attack_accepted_tick: Option<i32>,
    pub first_attacker_attack_requested_tick: Option<i32>,
    pub first_attacker_attack_accepted_tick: Option<i32>,
    pub first_hero_hit_tick: Option<i32>,
    pub first_attacker_hit_tick: Option<i32>,
    pub first_damage_to_attacker_tick: Option<i32>,
    pub first_damage_to_hero_tick: Option<i32>,
    pub first_damage_to_villager_tick: Option<i32>,
    pub first_damage_to_structure_tick: Option<i32>,
    pub minimum_hero_attacker_distance: Option<i32>,
    pub minimum_attacker_settlement_distance: Option<i32>,
    pub hero_attack_attempts: i32,
    pub hero_attacks_accepted: i32,
    pub hero_hits: i32,
    pub hero_damage_dealt_to_assault: i32,
    pub helper_attack_attempts: i32,
    pub helper_attacks_accepted: i32,
    pub helper_hits: i32,
    pub helper_damage_dealt_to_assault: i32,
    pub villager_attack_attempts: i32,
    pub villager_attacks_accepted: i32,
    pub villager_hits: i32,
    pub villager_damage_dealt_to_assault: i32,
    pub healing_items_used_during_assault: i32,
    pub healing_hp_restored_during_assault: i32,
    pub attacker_attack_attempts: i32,
    pub attacker_attacks_accepted: i32,
    pub attacker_hits: i32,
    pub attacker_target_changes: i32,
    pub hero_target_changes: i32,
    pub ticks_without_any_valid_target: i32,
    pub ticks_with_target_but_no_movement: i32,
    pub ticks_in_attack_range_but_no_attack: i32,
    pub engagement_failure_reason: Option<String>,
    pub first_wall_target_tick: Option<i32>,
    pub first_wall_damage_tick: Option<i32>,
    pub first_core_target_tick: Option<i32>,
    pub first_core_damage_tick: Option<i32>,
    pub wall_target_acquisitions: i32,
    pub wall_hits: i32,
    pub wall_damage_absorbed: i32,
    pub attackers_reaching_core: i32,
    pub attackers_bypassing_available_wall: i32,
    pub core_structure_damage: i32,
    pub core_structures_destroyed: i32,
    pub settlement_contact_failure_reason: Option<String>,
    pub hero_defeat_ticks: Vec<i32>,
    pub sanctuary_resurrection_ticks: Vec<i32>,
    pub true_death_tick: Option<i32>,
    pub latest_hero_defeat_cause: Option<String>,
    pub defeat_cause: Option<String>,
    #[serde(skip)]
    previous_hero_position: Option<Position>,
    #[serde(skip)]
    previous_attacker_positions: HashMap<i32, Position>,
    #[serde(skip)]
    previous_attacker_targets: HashMap<i32, i32>,
    #[serde(skip)]
    previous_hero_target: Option<i32>,
    #[serde(skip)]
    previous_hero_dead: bool,
    #[serde(skip)]
    previous_true_death: bool,
    #[serde(skip)]
    last_sample_tick: Option<i32>,
    #[serde(skip)]
    last_hero_attack_accepted_tick: Option<i32>,
    #[serde(skip)]
    last_attacker_attack_accepted_tick: Option<i32>,
    #[serde(skip)]
    pathing_stall_ticks: i32,
    #[serde(skip)]
    known_wall_ids: BTreeSet<i32>,
    #[serde(skip)]
    known_core_structure_ids: BTreeSet<i32>,
    #[serde(skip)]
    attackers_reaching_core_ids: BTreeSet<i32>,
    #[serde(skip)]
    attackers_bypassing_wall_ids: BTreeSet<i32>,
    #[serde(skip)]
    attacker_wall_targets: BTreeSet<(i32, i32)>,
    #[serde(skip)]
    destroyed_core_structure_ids: BTreeSet<i32>,
    #[serde(skip)]
    assault_caused_hero_defeat_ticks: BTreeSet<i32>,
}

impl CrisisEngagementTelemetry {
    fn record_first(field: &mut Option<i32>, game_tick: i32) {
        if field.is_none() {
            *field = Some(game_tick);
        }
    }

    fn record_minimum(field: &mut Option<i32>, distance: i32) {
        *field = Some(field.map_or(distance, |previous| previous.min(distance)));
    }

    pub fn record_launch(&mut self, game_tick: i32) {
        Self::record_first(&mut self.assault_launch_tick, game_tick);
    }

    fn record_hero_movement(&mut self, game_tick: i32) {
        Self::record_first(&mut self.first_hero_move_toward_attacker_tick, game_tick);
    }

    fn record_attacker_movement(&mut self, game_tick: i32) {
        Self::record_first(&mut self.first_attacker_move_toward_target_tick, game_tick);
    }

    fn record_hero_target(&mut self, target_id: i32, game_tick: i32) {
        if target_id == NO_TARGET {
            return;
        }
        Self::record_first(&mut self.first_hero_target_acquired_tick, game_tick);
        if self
            .previous_hero_target
            .is_some_and(|previous| previous != target_id)
        {
            self.hero_target_changes = self.hero_target_changes.saturating_add(1);
        }
        self.previous_hero_target = Some(target_id);
    }

    /// Record a target selected by an opt-in headless policy before that policy
    /// emits a production Move, Attack, or Ability event. Live clients do not
    /// maintain a server-side selected-target component, so this is deliberately
    /// a harness observation rather than a gameplay mutation.
    pub fn record_observed_hero_target(&mut self, target_id: i32, game_tick: i32) {
        self.record_hero_target(target_id, game_tick);
    }

    fn record_attacker_target(
        &mut self,
        attacker_id: i32,
        target_id: i32,
        target_subclass: Subclass,
        target_is_core_structure: bool,
        game_tick: i32,
    ) {
        if target_id == NO_TARGET {
            return;
        }
        Self::record_first(&mut self.first_attacker_target_acquired_tick, game_tick);
        let changed = self
            .previous_attacker_targets
            .insert(attacker_id, target_id)
            .is_some_and(|previous| previous != target_id);
        if changed {
            self.attacker_target_changes = self.attacker_target_changes.saturating_add(1);
        }
        if target_subclass == Subclass::Wall {
            if self.attacker_wall_targets.insert((attacker_id, target_id)) {
                self.wall_target_acquisitions = self.wall_target_acquisitions.saturating_add(1);
            }
            self.known_wall_ids.insert(target_id);
            Self::record_first(&mut self.first_wall_target_tick, game_tick);
        }
        if target_is_core_structure {
            self.known_core_structure_ids.insert(target_id);
            Self::record_first(&mut self.first_core_target_tick, game_tick);
        }
    }

    fn record_hero_attack_stage(
        &mut self,
        target_id: i32,
        game_tick: i32,
        stage: CrisisAttackTelemetryStage,
    ) {
        self.record_hero_target(target_id, game_tick);
        match stage {
            CrisisAttackTelemetryStage::Requested => {
                self.hero_attack_attempts = self.hero_attack_attempts.saturating_add(1);
                Self::record_first(&mut self.first_hero_attack_requested_tick, game_tick);
            }
            CrisisAttackTelemetryStage::Accepted => {
                self.hero_attacks_accepted = self.hero_attacks_accepted.saturating_add(1);
                self.last_hero_attack_accepted_tick = Some(game_tick);
                Self::record_first(&mut self.first_hero_attack_accepted_tick, game_tick);
            }
        }
    }

    fn record_attacker_attack_stage(
        &mut self,
        attacker_id: i32,
        target_id: i32,
        target_subclass: Subclass,
        target_is_core_structure: bool,
        game_tick: i32,
        stage: CrisisAttackTelemetryStage,
    ) {
        self.record_attacker_target(
            attacker_id,
            target_id,
            target_subclass,
            target_is_core_structure,
            game_tick,
        );
        match stage {
            CrisisAttackTelemetryStage::Requested => {
                self.attacker_attack_attempts = self.attacker_attack_attempts.saturating_add(1);
                Self::record_first(&mut self.first_attacker_attack_requested_tick, game_tick);
            }
            CrisisAttackTelemetryStage::Accepted => {
                self.attacker_attacks_accepted = self.attacker_attacks_accepted.saturating_add(1);
                self.last_attacker_attack_accepted_tick = Some(game_tick);
                Self::record_first(&mut self.first_attacker_attack_accepted_tick, game_tick);
            }
        }
    }

    fn record_hero_hit(&mut self, game_tick: i32, effective_damage: i32) {
        self.hero_hits = self.hero_hits.saturating_add(1);
        Self::record_first(&mut self.first_hero_hit_tick, game_tick);
        if effective_damage > 0 {
            Self::record_first(&mut self.first_damage_to_attacker_tick, game_tick);
            self.hero_damage_dealt_to_assault = self
                .hero_damage_dealt_to_assault
                .saturating_add(effective_damage);
        }
    }

    fn record_helper_attack_stage(&mut self, stage: CrisisAttackTelemetryStage) {
        match stage {
            CrisisAttackTelemetryStage::Requested => {
                self.helper_attack_attempts = self.helper_attack_attempts.saturating_add(1);
            }
            CrisisAttackTelemetryStage::Accepted => {
                self.helper_attacks_accepted = self.helper_attacks_accepted.saturating_add(1);
            }
        }
    }

    fn record_helper_hit(&mut self, effective_damage: i32) {
        self.helper_hits = self.helper_hits.saturating_add(1);
        if effective_damage > 0 {
            self.helper_damage_dealt_to_assault = self
                .helper_damage_dealt_to_assault
                .saturating_add(effective_damage);
        }
    }

    fn record_villager_attack_stage(&mut self, stage: CrisisAttackTelemetryStage) {
        match stage {
            CrisisAttackTelemetryStage::Requested => {
                self.villager_attack_attempts = self.villager_attack_attempts.saturating_add(1);
            }
            CrisisAttackTelemetryStage::Accepted => {
                self.villager_attacks_accepted = self.villager_attacks_accepted.saturating_add(1);
            }
        }
    }

    fn record_villager_hit(&mut self, effective_damage: i32) {
        self.villager_hits = self.villager_hits.saturating_add(1);
        if effective_damage > 0 {
            self.villager_damage_dealt_to_assault = self
                .villager_damage_dealt_to_assault
                .saturating_add(effective_damage);
        }
    }

    pub(crate) fn record_healing_use(&mut self, hp_restored: i32) {
        self.healing_items_used_during_assault =
            self.healing_items_used_during_assault.saturating_add(1);
        self.healing_hp_restored_during_assault = self
            .healing_hp_restored_during_assault
            .saturating_add(hp_restored.max(0));
    }

    fn record_attacker_hit(
        &mut self,
        target_id: i32,
        target_subclass: Subclass,
        target_is_structure: bool,
        target_is_core_structure: bool,
        effective_damage: i32,
        killed: bool,
        game_tick: i32,
    ) {
        let damage = effective_damage.max(0);
        self.attacker_hits = self.attacker_hits.saturating_add(1);
        Self::record_first(&mut self.first_attacker_hit_tick, game_tick);
        if damage > 0 {
            match target_subclass {
                Subclass::Hero => {
                    Self::record_first(&mut self.first_damage_to_hero_tick, game_tick);
                    if killed {
                        self.assault_caused_hero_defeat_ticks.insert(game_tick);
                        self.latest_hero_defeat_cause = Some("assault_enemy".to_string());
                    }
                }
                Subclass::Villager => {
                    Self::record_first(&mut self.first_damage_to_villager_tick, game_tick);
                }
                _ if target_is_structure => {
                    Self::record_first(&mut self.first_damage_to_structure_tick, game_tick);
                }
                _ => {}
            }
        }

        if target_subclass == Subclass::Wall {
            self.known_wall_ids.insert(target_id);
            self.wall_hits = self.wall_hits.saturating_add(1);
            self.wall_damage_absorbed = self.wall_damage_absorbed.saturating_add(damage);
            if damage > 0 {
                Self::record_first(&mut self.first_wall_damage_tick, game_tick);
            }
        }
        if target_is_core_structure {
            self.known_core_structure_ids.insert(target_id);
        }
        if target_subclass != Subclass::Wall
            && target_is_structure
            && (target_is_core_structure || self.known_core_structure_ids.contains(&target_id))
        {
            self.core_structure_damage = self.core_structure_damage.saturating_add(damage);
            if damage > 0 {
                Self::record_first(&mut self.first_core_damage_tick, game_tick);
            }
            if killed && self.destroyed_core_structure_ids.insert(target_id) {
                self.core_structures_destroyed = self.core_structures_destroyed.saturating_add(1);
            }
        }
    }

    fn record_hero_defeat(
        &mut self,
        dead: &StateDead,
        last_attacker: Option<&LastAttacker>,
        tracked_assault_units: &BTreeSet<i32>,
    ) {
        if self.hero_defeat_ticks.contains(&dead.dead_at) {
            return;
        }
        self.hero_defeat_ticks.push(dead.dead_at);
        let is_needs = matches!(
            dead.killer.as_str(),
            "Dehydration" | "Starvation" | "Exhaustion" | "Burns"
        );
        let exact_assault_attribution = self
            .assault_caused_hero_defeat_ticks
            .contains(&dead.dead_at)
            || last_attacker.is_some_and(|last| {
                last.tick == dead.dead_at && tracked_assault_units.contains(&last.id)
            });
        self.latest_hero_defeat_cause = Some(
            if is_needs {
                self.engagement_failure_reason = Some("needs_death".to_string());
                "needs"
            } else if exact_assault_attribution {
                self.assault_caused_hero_defeat_ticks.insert(dead.dead_at);
                "assault_enemy"
            } else {
                self.engagement_failure_reason = Some("ambient_death".to_string());
                "ambient_enemy"
            }
            .to_string(),
        );
    }

    fn record_hero_lifecycle(
        &mut self,
        game_tick: i32,
        dead: Option<&StateDead>,
        true_death: bool,
        last_attacker: Option<&LastAttacker>,
        tracked_assault_units: &BTreeSet<i32>,
    ) {
        if let Some(dead) = dead {
            self.record_hero_defeat(dead, last_attacker, tracked_assault_units);
        }

        let currently_dead = dead.is_some();
        if self.previous_hero_dead && !currently_dead {
            self.sanctuary_resurrection_ticks.push(game_tick);
        }
        if true_death && !self.previous_true_death {
            self.true_death_tick = Some(game_tick);
            self.defeat_cause = Some(
                match self.latest_hero_defeat_cause.as_deref() {
                    Some("assault_enemy") => "hero_true_death_from_assault",
                    Some("needs") => "hero_death_from_needs",
                    Some("ambient_enemy") => "hero_death_from_ambient_enemy",
                    _ => "unknown",
                }
                .to_string(),
            );
        }
        self.previous_hero_dead = currently_dead;
        self.previous_true_death = true_death;
    }

    /// Finalize an unresolved headless leg without requiring the telemetry
    /// layer to know the runner's tick cap.
    pub fn record_tick_cap_failure(&mut self) {
        self.defeat_cause = Some("assault_unresolved_at_tick_cap".to_string());
        if self.engagement_failure_reason.is_none() {
            self.engagement_failure_reason = Some(self.stable_failure_reason(true).to_string());
        }
        if self.first_wall_target_tick.is_none() && !self.known_wall_ids.is_empty() {
            self.settlement_contact_failure_reason = Some("no_wall_contact".to_string());
        }
    }

    fn update_settlement_contact_failure_reason(&mut self) {
        self.settlement_contact_failure_reason =
            if self.first_wall_target_tick.is_some() || self.first_wall_damage_tick.is_some() {
                None
            } else if self.known_wall_ids.is_empty() {
                Some("no_wall_present".to_string())
            } else {
                Some("no_wall_contact".to_string())
            };
    }

    pub fn record_no_target_stall(&mut self) {
        self.engagement_failure_reason = Some("no_valid_target".to_string());
    }

    pub fn record_pathing_stall(&mut self) {
        self.engagement_failure_reason = Some("path_unreachable".to_string());
    }

    pub fn stable_failure_reason(&self, tick_cap: bool) -> &'static str {
        if self.first_attacker_visible_tick.is_none() {
            return "no_perception";
        }
        if self.first_attacker_target_acquired_tick.is_none()
            && self.first_hero_target_acquired_tick.is_none()
        {
            return "no_valid_target";
        }
        if self.pathing_stall_ticks > 0
            && self.first_attacker_move_toward_target_tick.is_none()
            && self.first_hero_move_toward_attacker_tick.is_none()
        {
            return "path_unreachable";
        }
        if self.attacker_attack_attempts == 0 && self.ticks_with_target_but_no_movement > 0 {
            return "npc_policy_no_move";
        }
        if self.attacker_attack_attempts == 0 && self.first_attacker_target_acquired_tick.is_some()
        {
            return "npc_policy_no_attack";
        }
        if self.hero_attack_attempts == 0 && self.first_hero_target_acquired_tick.is_some() {
            return "hero_policy_no_attack";
        }
        if self
            .hero_attack_attempts
            .saturating_add(self.attacker_attack_attempts)
            > 0
            && self
                .hero_attacks_accepted
                .saturating_add(self.attacker_attacks_accepted)
                == 0
        {
            return "attack_rejected";
        }
        if self
            .hero_attacks_accepted
            .saturating_add(self.attacker_attacks_accepted)
            > 0
            && self.hero_hits.saturating_add(self.attacker_hits) == 0
        {
            return "all_attacks_missed";
        }
        if tick_cap {
            "tick_cap"
        } else {
            "unknown"
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
    pub engagement: CrisisEngagementTelemetry,
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
        if phase == CrisisPhase::AssaultActive {
            self.engagement.record_launch(game_tick);
        }
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
    pub foundation_ids: BTreeSet<i32>,
    pub defensive_structure_ids: BTreeSet<i32>,
    pub defensive_foundation_ids: BTreeSet<i32>,
    pub wall_ids: BTreeSet<i32>,
    pub structure_health: HashMap<i32, i32>,
    pub equipped_weapon: Option<String>,
    pub equipped_armor_count: i32,
    pub equipped_item_ids: BTreeSet<i32>,
    pub healing_items: i32,
    pub villagers: BTreeSet<i32>,
    pub combat_capable_villagers: BTreeSet<i32>,
    pub villager_assignments: HashMap<i32, i32>,
    pub sanctuary_level: i32,
    pub total_run_items: i32,
    pub stored_items: i32,
    pub online: bool,
    pub near_settlement: bool,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct CrisisBalanceObservationState(pub HashMap<i32, CrisisBalanceObservation>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrisisAttackTelemetryStage {
    Requested,
    Accepted,
}

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct CrisisAttackTelemetryEvent {
    pub entity: Entity,
    pub game_tick: i32,
    pub stage: CrisisAttackTelemetryStage,
    pub attacker_id: i32,
    pub attacker_player_id: i32,
    pub attacker_subclass: Subclass,
    pub attacker_crisis: Option<CrisisAssaultUnit>,
    pub target_id: i32,
    pub target_player_id: i32,
    pub target_subclass: Subclass,
    pub target_is_structure: bool,
    pub target_is_core_structure: bool,
    pub target_crisis: Option<CrisisAssaultUnit>,
}

#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct CrisisCombatTelemetryEvent {
    pub entity: Entity,
    pub game_tick: i32,
    pub attacker_id: i32,
    pub attacker_player_id: i32,
    pub attacker_subclass: Subclass,
    pub attacker_crisis: Option<CrisisAssaultUnit>,
    pub target_id: i32,
    pub target_player_id: i32,
    pub target_subclass: Subclass,
    pub target_is_structure: bool,
    pub target_is_core_structure: bool,
    pub target_crisis: Option<CrisisAssaultUnit>,
    pub effective_damage: i32,
    pub killed: bool,
}

fn attribution_is_current(
    attribution: CrisisAssaultUnit,
    crisis_state: &SettlementCrisisState,
) -> bool {
    crisis_state
        .get(&attribution.owner_player_id)
        .map(|crisis| {
            crisis.assault_id == Some(attribution.assault_id)
                && crisis.assault_spawn_generation == attribution.spawn_generation
                && matches!(
                    crisis.phase,
                    CrisisPhase::AssaultActive | CrisisPhase::Resolved
                )
        })
        .unwrap_or(false)
}

pub(crate) fn crisis_attack_telemetry_observer(
    event: On<CrisisAttackTelemetryEvent>,
    config: Option<Res<CrisisBalanceTelemetryConfig>>,
    crisis_state: Res<SettlementCrisisState>,
    mut telemetry_state: ResMut<CrisisBalanceTelemetryState>,
) {
    if !config
        .as_deref()
        .is_some_and(|config| config.sample_interval_ticks.is_some())
    {
        return;
    }

    if let Some(source) = event.attacker_crisis {
        if attribution_is_current(source, &crisis_state) {
            if let Some(telemetry) = telemetry_state.get_mut(&source.owner_player_id) {
                if telemetry
                    .assault_outcome
                    .tracks_assault_unit(event.attacker_id)
                {
                    if event.target_player_id != source.owner_player_id {
                        if event.stage == CrisisAttackTelemetryStage::Requested {
                            telemetry.assault_outcome.cross_player_target_violations = telemetry
                                .assault_outcome
                                .cross_player_target_violations
                                .saturating_add(1);
                        }
                    } else {
                        telemetry.engagement.record_attacker_attack_stage(
                            event.attacker_id,
                            event.target_id,
                            event.target_subclass,
                            event.target_is_core_structure,
                            event.game_tick,
                            event.stage,
                        );
                    }
                }
            }
        }
    }

    if let Some(target) = event.target_crisis {
        if attribution_is_current(target, &crisis_state) {
            if let Some(telemetry) = telemetry_state.get_mut(&target.owner_player_id) {
                if telemetry
                    .assault_outcome
                    .tracks_assault_unit(event.target_id)
                {
                    if event.attacker_player_id == target.owner_player_id
                        && event.attacker_subclass == Subclass::Hero
                    {
                        telemetry.engagement.record_hero_attack_stage(
                            event.target_id,
                            event.game_tick,
                            event.stage,
                        );
                    } else if event.attacker_player_id == target.owner_player_id
                        && event.attacker_subclass == Subclass::Villager
                    {
                        telemetry
                            .engagement
                            .record_villager_attack_stage(event.stage);
                    } else if event.attacker_player_id > 0
                        && event.attacker_player_id < NPC_PLAYER_ID
                    {
                        telemetry.engagement.record_helper_attack_stage(event.stage);
                    }
                }
            }
        }
    }
}

pub(crate) fn crisis_combat_telemetry_observer(
    event: On<CrisisCombatTelemetryEvent>,
    config: Option<Res<CrisisBalanceTelemetryConfig>>,
    crisis_state: Res<SettlementCrisisState>,
    mut telemetry_state: ResMut<CrisisBalanceTelemetryState>,
) {
    if let Some(source) = event.attacker_crisis {
        let source_metadata_is_current = attribution_is_current(source, &crisis_state);
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
                        if config
                            .as_deref()
                            .is_some_and(|config| config.sample_interval_ticks.is_some())
                        {
                            telemetry.engagement.record_attacker_hit(
                                event.target_id,
                                event.target_subclass,
                                event.target_is_structure,
                                event.target_is_core_structure,
                                event.effective_damage,
                                event.killed,
                                event.game_tick,
                            );
                        }
                    }
                }
            }
        }
    }

    if let Some(target) = event.target_crisis {
        let target_is_current = attribution_is_current(target, &crisis_state);
        if target_is_current {
            if let Some(telemetry) = telemetry_state.get_mut(&target.owner_player_id) {
                if telemetry
                    .assault_outcome
                    .tracks_assault_unit(event.target_id)
                {
                    if config
                        .as_deref()
                        .is_some_and(|config| config.sample_interval_ticks.is_some())
                        && event.attacker_player_id == target.owner_player_id
                        && event.attacker_subclass == Subclass::Hero
                    {
                        telemetry
                            .engagement
                            .record_hero_hit(event.game_tick, event.effective_damage);
                    }
                    if config
                        .as_deref()
                        .is_some_and(|config| config.sample_interval_ticks.is_some())
                        && event.attacker_player_id == target.owner_player_id
                        && event.attacker_subclass == Subclass::Villager
                    {
                        telemetry
                            .engagement
                            .record_villager_hit(event.effective_damage);
                    }
                    if event.effective_damage > 0
                        && event.attacker_player_id > 0
                        && event.attacker_player_id < NPC_PLAYER_ID
                        && event.attacker_player_id != target.owner_player_id
                    {
                        telemetry.assault_outcome.helper_participated = true;
                        if config
                            .as_deref()
                            .is_some_and(|config| config.sample_interval_ticks.is_some())
                        {
                            telemetry
                                .engagement
                                .record_helper_hit(event.effective_damage);
                        }
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

pub(crate) fn is_live_built_human_core_structure(
    class_structure: Option<&ClassStructure>,
    class: &Class,
    player_id: &PlayerId,
    subclass: Subclass,
    state: State,
    dead: bool,
) -> bool {
    class_structure.is_some()
        && class.is_structure()
        && player_id.0 > 0
        && player_id.is_human()
        && subclass != Subclass::Wall
        && Structure::is_built(state)
        && !dead
}

#[derive(Debug, Clone, Copy)]
struct EngagementObjectObservation {
    player_id: i32,
    pos: Position,
    subclass: Subclass,
    live: bool,
    core_structure: bool,
}

fn has_complete_blocking_wall_ring(core: Position, wall_positions: &[Position]) -> bool {
    let adjacent = Map::range((core.x, core.y), 1)
        .into_iter()
        .map(|(x, y)| Position { x, y })
        .filter(|position| Map::dist(core, *position) == 1)
        .collect::<Vec<_>>();
    !adjacent.is_empty()
        && adjacent
            .iter()
            .all(|position| wall_positions.contains(position))
}

/// Captures the terminal transition at the exact `Added<TrueDeath>` boundary so
/// a headless runner that stops immediately after the update cannot outrun the
/// periodic engagement sampler.
pub(crate) fn crisis_true_death_telemetry_system(
    game_tick: Res<GameTick>,
    config: Res<CrisisBalanceTelemetryConfig>,
    crisis_state: Res<SettlementCrisisState>,
    mut telemetry_state: ResMut<CrisisBalanceTelemetryState>,
    hero_query: Query<
        (&PlayerId, &StateDead, Option<&LastAttacker>),
        (With<SubclassHero>, Added<TrueDeath>),
    >,
) {
    if config.sample_interval_ticks.is_none() {
        return;
    }
    for (player_id, dead, last_attacker) in &hero_query {
        let Some(crisis) = crisis_state.get(&player_id.0) else {
            continue;
        };
        if crisis.phase != CrisisPhase::AssaultActive {
            continue;
        }
        let tracked = crisis
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        telemetry_state
            .entry(player_id.0)
            .or_default()
            .engagement
            .record_hero_lifecycle(game_tick.0, Some(dead), true, last_attacker, &tracked);
    }
}

fn supported_hero_attack_range(
    hero_class: Option<&HeroClass>,
    stats: &Stats,
    inventory: &Inventory,
    effects: &Effects,
) -> u32 {
    let mut range = inventory
        .get_equipped_main_hand()
        .and_then(|weapon| match weapon.attrs.get(&AttrKey::AttackRange) {
            Some(AttrVal::Num(value)) if *value > 0.0 => Some(*value as u32),
            _ => None,
        })
        .unwrap_or(1)
        .max(1);
    if range > 1 && effects.has(Effect::WatchtowerLight) {
        range = range.saturating_add(1);
    }

    match hero_class {
        Some(HeroClass::Ranger)
            if stats.stamina.unwrap_or(0) >= 8
                && inventory
                    .get_equipped_main_hand()
                    .is_some_and(|weapon| weapon.subclass == "Bow") =>
        {
            range.max(3)
        }
        Some(HeroClass::Mage) if stats.mana.unwrap_or(0) >= 20 => range.max(3),
        _ => range,
    }
}

/// Opt-in observation of target acquisition, approach, stalls, settlement
/// contact, and hero defeat attribution. It samples authoritative components
/// but never inserts a target, movement request, or gameplay component.
pub(crate) fn crisis_engagement_snapshot_system(
    game_tick: Res<GameTick>,
    config: Res<CrisisBalanceTelemetryConfig>,
    crisis_state: Res<SettlementCrisisState>,
    mut telemetry_state: ResMut<CrisisBalanceTelemetryState>,
    hero_query: Query<
        (
            &PlayerId,
            &Position,
            &Viewshed,
            &State,
            Option<&StateDead>,
            Option<&TrueDeath>,
            Option<&LastAttacker>,
            Option<&HeroClass>,
            &Stats,
            &Inventory,
            &Effects,
        ),
        With<SubclassHero>,
    >,
    assault_query: Query<(
        &Id,
        &Position,
        &State,
        &CrisisAssaultUnit,
        Option<&VisibleTarget>,
        Option<&Target>,
        Option<&TaskTarget>,
        Option<&StateDead>,
    )>,
    object_query: Query<(
        &Id,
        &PlayerId,
        &Position,
        &Class,
        &Subclass,
        &State,
        Option<&ClassStructure>,
        Option<&StateDead>,
    )>,
) {
    let Some(sample_interval) = config.sample_interval_ticks else {
        return;
    };
    let sample_interval = sample_interval.max(1);
    let objects = object_query
        .iter()
        .map(
            |(id, player_id, pos, class, subclass, state, structure, dead)| {
                (
                    id.0,
                    EngagementObjectObservation {
                        player_id: player_id.0,
                        pos: *pos,
                        subclass: *subclass,
                        live: state.is_alive() && dead.is_none(),
                        core_structure: structure.is_some()
                            && is_live_built_human_core_structure(
                                structure,
                                class,
                                player_id,
                                *subclass,
                                *state,
                                dead.is_some(),
                            ),
                    },
                )
            },
        )
        .collect::<HashMap<_, _>>();

    for (owner_player_id, crisis) in crisis_state.iter() {
        if crisis.phase != CrisisPhase::AssaultActive {
            continue;
        }
        let Some(assault_id) = crisis.assault_id else {
            continue;
        };

        let Some((
            _,
            hero_pos,
            hero_viewshed,
            hero_state,
            hero_dead,
            true_death,
            last_attacker,
            hero_class,
            hero_stats,
            hero_inventory,
            hero_effects,
        )) = hero_query
            .iter()
            .find(|(player_id, _, _, _, _, _, _, _, _, _, _)| player_id.0 == *owner_player_id)
        else {
            continue;
        };
        let hero_alive = hero_state.is_alive() && hero_dead.is_none() && true_death.is_none();
        let tracked_assault_units = crisis
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let live_attackers = assault_query
            .iter()
            .filter(|(id, _, state, attribution, _, _, _, dead)| {
                tracked_assault_units.contains(&id.0)
                    && attribution.owner_player_id == *owner_player_id
                    && attribution.assault_id == assault_id
                    && attribution.spawn_generation == crisis.assault_spawn_generation
                    && state.is_alive()
                    && dead.is_none()
            })
            .collect::<Vec<_>>();

        let telemetry = telemetry_state.entry(*owner_player_id).or_default();
        let engagement = &mut telemetry.engagement;
        engagement.record_launch(crisis.assault_started_tick.unwrap_or(game_tick.0));
        let last_sample_tick = engagement.last_sample_tick;
        if last_sample_tick.is_some_and(|last| game_tick.0.saturating_sub(last) < sample_interval) {
            continue;
        }
        let observed_ticks = last_sample_tick
            .map(|last| game_tick.0.saturating_sub(last).max(0))
            .unwrap_or(0);
        engagement.last_sample_tick = Some(game_tick.0);

        engagement.record_hero_lifecycle(
            game_tick.0,
            hero_dead,
            true_death.is_some(),
            last_attacker,
            &tracked_assault_units,
        );

        let owned_structures = objects
            .iter()
            .filter(|(_, object)| {
                object.player_id == *owner_player_id
                    && object.live
                    && (object.core_structure || object.subclass == Subclass::Wall)
            })
            .map(|(id, object)| (*id, *object))
            .collect::<Vec<_>>();
        let core_structures = owned_structures
            .iter()
            .filter(|(_, object)| object.core_structure)
            .copied()
            .collect::<Vec<_>>();
        let walls = owned_structures
            .iter()
            .filter(|(_, object)| object.subclass == Subclass::Wall)
            .copied()
            .collect::<Vec<_>>();
        let wall_positions = walls.iter().map(|(_, wall)| wall.pos).collect::<Vec<_>>();
        engagement
            .known_core_structure_ids
            .extend(core_structures.iter().map(|(id, _)| *id));
        engagement
            .known_wall_ids
            .extend(walls.iter().map(|(id, _)| *id));

        let mut any_attacker_target = false;
        let mut any_attacker_moved_toward_target = false;
        let mut any_in_attack_range = false;
        let mut current_attacker_positions = HashMap::new();
        let mut current_attacker_targets = HashMap::new();

        for (id, pos, state, _attribution, visible, installed, task, _dead) in &live_attackers {
            current_attacker_positions.insert(id.0, **pos);
            let target_id = [
                installed.map(|target| target.id),
                visible.map(|target| target.target),
                task.map(|target| target.target),
            ]
            .into_iter()
            .flatten()
            .find(|target_id| *target_id != NO_TARGET)
            .unwrap_or(NO_TARGET);
            let target = objects.get(&target_id).filter(|target| {
                target_id != NO_TARGET && target.player_id == *owner_player_id && target.live
            });
            if let Some(target) = target {
                any_attacker_target = true;
                current_attacker_targets.insert(id.0, target_id);
                engagement.record_attacker_target(
                    id.0,
                    target_id,
                    target.subclass,
                    target.core_structure,
                    game_tick.0,
                );
                let distance = Map::dist(**pos, target.pos) as i32;
                any_in_attack_range |= distance <= 1;
                let moved_toward = engagement
                    .previous_attacker_positions
                    .get(&id.0)
                    .is_some_and(|previous| Map::dist(*previous, target.pos) > distance as u32);
                if moved_toward {
                    any_attacker_moved_toward_target = true;
                    engagement.record_attacker_movement(game_tick.0);
                }
            }

            if hero_alive {
                let distance = Map::dist(*hero_pos, **pos) as i32;
                CrisisEngagementTelemetry::record_minimum(
                    &mut engagement.minimum_hero_attacker_distance,
                    distance,
                );
                if hero_viewshed.range >= distance as u32 && state.is_visible() {
                    CrisisEngagementTelemetry::record_first(
                        &mut engagement.first_attacker_visible_tick,
                        game_tick.0,
                    );
                }
            }

            if let Some(settlement_distance) = owned_structures
                .iter()
                .map(|(_, structure)| Map::dist(**pos, structure.pos) as i32)
                .min()
            {
                CrisisEngagementTelemetry::record_minimum(
                    &mut engagement.minimum_attacker_settlement_distance,
                    settlement_distance,
                );
            }

            let reached_core = core_structures
                .iter()
                .find(|(_, core)| Map::dist(**pos, core.pos) <= 1);
            if let Some((_, reached_core)) = reached_core {
                if engagement.attackers_reaching_core_ids.insert(id.0) {
                    engagement.attackers_reaching_core =
                        engagement.attackers_reaching_core.saturating_add(1);
                    // "Available wall" is intentionally conservative: count a
                    // bypass only when live owner walls form a complete adjacent
                    // ring around the reached core. A lone wall elsewhere in the
                    // settlement is not evidence that this route was blocked.
                    if has_complete_blocking_wall_ring(reached_core.pos, &wall_positions)
                        && !engagement
                            .attacker_wall_targets
                            .iter()
                            .any(|(attacker_id, _)| *attacker_id == id.0)
                        && engagement.attackers_bypassing_wall_ids.insert(id.0)
                    {
                        engagement.attackers_bypassing_available_wall = engagement
                            .attackers_bypassing_available_wall
                            .saturating_add(1);
                    }
                }
            }
        }

        let nearest_attacker = live_attackers
            .iter()
            .min_by_key(|(_, pos, _, _, _, _, _, _)| Map::dist(*hero_pos, **pos));
        let hero_moved_toward_attacker =
            nearest_attacker.is_some_and(|(_, attacker_pos, _, _, _, _, _, _)| {
                engagement.previous_hero_position.is_some_and(|previous| {
                    Map::dist(previous, **attacker_pos) > Map::dist(*hero_pos, **attacker_pos)
                })
            });
        if hero_moved_toward_attacker {
            engagement.record_hero_movement(game_tick.0);
        }

        let live_attacker_ids = live_attackers
            .iter()
            .map(|(id, _, _, _, _, _, _, _)| id.0)
            .collect::<BTreeSet<_>>();
        let hero_target = engagement
            .previous_hero_target
            .filter(|target| live_attacker_ids.contains(target));
        let hero_has_target = hero_target.is_some();
        if let Some(hero_target) = hero_target {
            let supported_range =
                supported_hero_attack_range(hero_class, hero_stats, hero_inventory, hero_effects);
            any_in_attack_range |= live_attackers
                .iter()
                .find(|(id, _, _, _, _, _, _, _)| id.0 == hero_target)
                .is_some_and(|(_, attacker_pos, _, _, _, _, _, _)| {
                    Map::dist(*hero_pos, **attacker_pos) <= supported_range
                });
        }
        if observed_ticks > 0 {
            if !any_attacker_target && !hero_has_target {
                engagement.ticks_without_any_valid_target = engagement
                    .ticks_without_any_valid_target
                    .saturating_add(observed_ticks);
            } else if !any_attacker_moved_toward_target && !hero_moved_toward_attacker {
                engagement.ticks_with_target_but_no_movement = engagement
                    .ticks_with_target_but_no_movement
                    .saturating_add(observed_ticks);
                if !any_in_attack_range {
                    engagement.pathing_stall_ticks = engagement
                        .pathing_stall_ticks
                        .saturating_add(observed_ticks);
                }
            }

            let accepted_since_last_sample = engagement
                .last_attacker_attack_accepted_tick
                .into_iter()
                .chain(engagement.last_hero_attack_accepted_tick)
                .any(|accepted| last_sample_tick.is_none_or(|last| accepted > last));
            if any_in_attack_range && !accepted_since_last_sample {
                engagement.ticks_in_attack_range_but_no_attack = engagement
                    .ticks_in_attack_range_but_no_attack
                    .saturating_add(observed_ticks);
            }
        }

        engagement.update_settlement_contact_failure_reason();
        engagement.previous_hero_position = Some(*hero_pos);
        engagement.previous_attacker_positions = current_attacker_positions;
        engagement.previous_attacker_targets = current_attacker_targets;
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
    fn preparation_categories_are_distinct_and_stably_ordered() {
        let mut actions = CrisisPreparationActions::default();

        assert!(actions.record_repair_started(100, 500));
        assert!(actions.record_repair_completed(100, 550));
        assert!(actions.record_defensive_structure_started(200, 600));
        assert!(actions.record_defensive_structure_completed(200, true, 650));
        assert!(actions.record_equipment_change(300, 700));
        assert_eq!(actions.observe_healing_items(0, 750), 0);
        assert_eq!(actions.observe_healing_items(2, 800), 2);
        assert!(actions.record_villager_recruited(400, 850));
        assert_eq!(actions.observe_sanctuary_level(0, 875), 0);
        assert_eq!(actions.observe_sanctuary_level(1, 900), 1);

        assert_eq!(actions.meaningful_preparation_category_count, 6);
        assert_eq!(
            actions.meaningful_preparation_categories,
            vec![
                "defenses",
                "equipment",
                "healing",
                "repair",
                "sanctuary",
                "villager_support",
            ]
        );
    }

    #[test]
    fn preparation_first_action_tick_is_the_absolute_earliest_recorded_tick() {
        let mut actions = CrisisPreparationActions::default();
        assert_eq!(actions.first_preparation_action_tick, None);

        assert!(actions.record_defensive_structure_started(10, 700));
        assert_eq!(actions.first_preparation_action_tick, Some(700));
        assert!(actions.record_equipment_change(20, 450));
        assert_eq!(actions.first_preparation_action_tick, Some(450));

        actions.mark_action_at(500);
        actions.mark_action_at(300);
        assert_eq!(actions.first_preparation_action_tick, Some(300));
        assert!(actions.performed_preparation_action);
    }

    #[test]
    fn repeated_preparation_observations_do_not_inflate_meaningful_counts() {
        let mut actions = CrisisPreparationActions::default();

        assert!(actions.record_repair_started(10, 100));
        assert!(!actions.record_repair_started(10, 101));
        assert!(actions.record_repair_completed(10, 110));
        assert!(!actions.record_repair_completed(10, 111));

        assert!(actions.record_defensive_structure_started(20, 120));
        assert!(!actions.record_defensive_structure_started(20, 121));
        assert!(actions.record_defensive_structure_completed(20, true, 130));
        assert!(!actions.record_defensive_structure_completed(20, true, 131));

        assert!(actions.record_equipment_change(30, 140));
        assert!(!actions.record_equipment_change(30, 141));
        assert!(actions.record_equipment_change(31, 142));
        assert!(!actions.record_equipment_change(30, 143));

        assert_eq!(actions.observe_healing_items(1, 150), 0);
        assert_eq!(actions.observe_healing_items(3, 151), 2);
        assert_eq!(actions.observe_healing_items(1, 152), 0);
        assert_eq!(actions.observe_healing_items(3, 153), 0);
        let healing_use_event_id = Uuid::from_u128(40);
        assert!(actions.record_healing_item_used_before_launch(healing_use_event_id, 154));
        assert!(!actions.record_healing_item_used_before_launch(healing_use_event_id, 155));

        assert!(actions.record_villager_recruited(50, 160));
        assert!(!actions.record_villager_recruited(50, 161));
        assert!(actions.record_villager_assignment_changed(50, 162));
        assert!(!actions.record_villager_assignment_changed(50, 163));

        assert_eq!(actions.observe_total_run_items(100, 170), 0);
        assert_eq!(actions.observe_total_run_items(112, 171), 12);
        assert_eq!(actions.observe_total_run_items(80, 172), 0);
        assert_eq!(actions.observe_total_run_items(112, 173), 0);
        assert_eq!(actions.observe_stored_items(50, 180), 0);
        assert_eq!(actions.observe_stored_items(57, 181), 7);
        assert_eq!(actions.observe_stored_items(40, 182), 0);
        assert_eq!(actions.observe_stored_items(57, 183), 0);
        assert_eq!(actions.observe_sanctuary_level(2, 184), 0);
        assert_eq!(actions.observe_sanctuary_level(3, 185), 1);
        assert_eq!(actions.observe_sanctuary_level(2, 186), 0);
        assert_eq!(actions.observe_sanctuary_level(3, 187), 0);

        assert!(actions.record_launch_readiness(3, [50, 50, 51]));
        assert!(!actions.record_launch_readiness(99, [60, 61, 62]));

        assert_eq!(actions.repairs_started, 1);
        assert_eq!(actions.repairs_completed, 1);
        assert_eq!(actions.structures_repaired, 1);
        assert_eq!(actions.defensive_structures_started, 1);
        assert_eq!(actions.defensive_structures_completed, 1);
        assert_eq!(actions.structures_built, 1);
        assert_eq!(actions.walls_built, 1);
        assert_eq!(actions.equipment_changes, 2);
        assert_eq!(actions.healing_items_acquired, 2);
        assert_eq!(actions.healing_items_used_before_launch, 1);
        assert_eq!(actions.villagers_recruited, 1);
        assert_eq!(actions.villager_assignments_changed, 1);
        assert_eq!(actions.resource_units_acquired, 12);
        assert_eq!(actions.storage_units_added, 7);
        assert_eq!(actions.sanctuary_upgrades, 1);
        assert_eq!(actions.healing_items_carried_at_launch, 3);
        assert_eq!(actions.combat_capable_villagers_at_launch, 2);
        assert_eq!(actions.meaningful_preparation_category_count, 6);
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
    fn engagement_first_targets_movements_and_changes_are_idempotent() {
        let mut engagement = CrisisEngagementTelemetry::default();
        engagement.record_hero_target(101, 10);
        engagement.record_hero_target(101, 20);
        engagement.record_hero_target(102, 30);
        engagement.record_attacker_target(201, 1, Subclass::Hero, false, 11);
        engagement.record_attacker_target(201, 1, Subclass::Hero, false, 21);
        engagement.record_attacker_target(201, 2, Subclass::Wall, false, 31);
        engagement.record_attacker_movement(40);
        engagement.record_attacker_movement(50);
        engagement.record_hero_movement(41);
        engagement.record_hero_movement(51);

        assert_eq!(engagement.first_hero_target_acquired_tick, Some(10));
        assert_eq!(engagement.first_attacker_target_acquired_tick, Some(11));
        assert_eq!(engagement.hero_target_changes, 1);
        assert_eq!(engagement.attacker_target_changes, 1);
        assert_eq!(engagement.wall_target_acquisitions, 1);
        assert_eq!(engagement.first_attacker_move_toward_target_tick, Some(40));
        assert_eq!(engagement.first_hero_move_toward_attacker_tick, Some(41));
    }

    #[test]
    fn observed_hero_target_can_precede_attack_and_explain_no_attack_stall() {
        let mut engagement = CrisisEngagementTelemetry {
            first_attacker_visible_tick: Some(5),
            ..Default::default()
        };

        engagement.record_observed_hero_target(101, 10);
        engagement.record_observed_hero_target(101, 20);

        assert_eq!(engagement.first_hero_target_acquired_tick, Some(10));
        assert_eq!(engagement.hero_attack_attempts, 0);
        assert_eq!(
            engagement.stable_failure_reason(true),
            "hero_policy_no_attack"
        );
    }

    #[test]
    fn attack_requests_acceptance_hits_and_damage_remain_distinct() {
        let mut engagement = CrisisEngagementTelemetry::default();
        engagement.record_hero_attack_stage(101, 10, CrisisAttackTelemetryStage::Requested);
        engagement.record_hero_attack_stage(101, 11, CrisisAttackTelemetryStage::Requested);
        engagement.record_hero_attack_stage(101, 12, CrisisAttackTelemetryStage::Accepted);
        engagement.record_attacker_attack_stage(
            201,
            301,
            Subclass::Wall,
            false,
            15,
            CrisisAttackTelemetryStage::Requested,
        );
        engagement.record_attacker_attack_stage(
            201,
            301,
            Subclass::Wall,
            false,
            16,
            CrisisAttackTelemetryStage::Accepted,
        );
        engagement.record_hero_hit(13, 0);
        engagement.record_hero_hit(14, 7);
        engagement.record_helper_attack_stage(CrisisAttackTelemetryStage::Requested);
        engagement.record_helper_attack_stage(CrisisAttackTelemetryStage::Accepted);
        engagement.record_helper_hit(15);
        engagement.record_villager_attack_stage(CrisisAttackTelemetryStage::Requested);
        engagement.record_villager_attack_stage(CrisisAttackTelemetryStage::Accepted);
        engagement.record_villager_hit(9);

        assert_eq!(engagement.hero_attack_attempts, 2);
        assert_eq!(engagement.hero_attacks_accepted, 1);
        assert_eq!(engagement.first_hero_attack_requested_tick, Some(10));
        assert_eq!(engagement.first_hero_attack_accepted_tick, Some(12));
        assert_eq!(engagement.hero_hits, 2);
        assert_eq!(engagement.first_hero_hit_tick, Some(13));
        assert_eq!(engagement.first_damage_to_attacker_tick, Some(14));
        assert_eq!(engagement.hero_damage_dealt_to_assault, 7);
        assert_eq!(engagement.attacker_attack_attempts, 1);
        assert_eq!(engagement.attacker_attacks_accepted, 1);
        assert_eq!(engagement.wall_target_acquisitions, 1);
        assert_eq!(engagement.attacker_target_changes, 0);
        assert_eq!(engagement.helper_attack_attempts, 1);
        assert_eq!(engagement.helper_attacks_accepted, 1);
        assert_eq!(engagement.helper_hits, 1);
        assert_eq!(engagement.helper_damage_dealt_to_assault, 15);
        assert_eq!(engagement.villager_attack_attempts, 1);
        assert_eq!(engagement.villager_attacks_accepted, 1);
        assert_eq!(engagement.villager_hits, 1);
        assert_eq!(engagement.villager_damage_dealt_to_assault, 9);
    }

    #[test]
    fn assault_healing_usage_counts_items_and_restored_hp_separately() {
        let mut engagement = CrisisEngagementTelemetry::default();
        engagement.record_healing_use(10);
        engagement.record_healing_use(0);

        assert_eq!(engagement.healing_items_used_during_assault, 2);
        assert_eq!(engagement.healing_hp_restored_during_assault, 10);
    }

    #[test]
    fn defeat_cause_requires_exact_assault_attribution_and_distinguishes_needs() {
        let tracked = BTreeSet::from([101]);
        let mut assault = CrisisEngagementTelemetry::default();
        assault.record_attacker_hit(1, Subclass::Hero, false, false, 10, true, 100);
        let assault_dead = StateDead {
            dead_at: 100,
            killer: "Goblin Pillager".to_string(),
        };
        assault.record_hero_lifecycle(
            100,
            Some(&assault_dead),
            false,
            Some(&LastAttacker { id: 101, tick: 100 }),
            &tracked,
        );
        assault.record_hero_lifecycle(
            200,
            Some(&assault_dead),
            true,
            Some(&LastAttacker { id: 101, tick: 100 }),
            &tracked,
        );
        assert_eq!(
            assault.defeat_cause.as_deref(),
            Some("hero_true_death_from_assault")
        );

        let mut needs = CrisisEngagementTelemetry::default();
        let needs_dead = StateDead {
            dead_at: 300,
            killer: "Starvation".to_string(),
        };
        needs.record_hero_lifecycle(300, Some(&needs_dead), true, None, &tracked);
        assert_eq!(needs.defeat_cause.as_deref(), Some("hero_death_from_needs"));
        assert_eq!(
            needs.engagement_failure_reason.as_deref(),
            Some("needs_death")
        );

        let mut heat = CrisisEngagementTelemetry::default();
        let heat_dead = StateDead {
            dead_at: 350,
            killer: "Burns".to_string(),
        };
        heat.record_hero_lifecycle(350, Some(&heat_dead), true, None, &tracked);
        assert_eq!(heat.defeat_cause.as_deref(), Some("hero_death_from_needs"));
        assert_eq!(
            heat.engagement_failure_reason.as_deref(),
            Some("needs_death")
        );

        let mut ambient = CrisisEngagementTelemetry::default();
        let ambient_dead = StateDead {
            dead_at: 400,
            killer: "Cave Bat".to_string(),
        };
        ambient.record_hero_lifecycle(
            400,
            Some(&ambient_dead),
            true,
            Some(&LastAttacker { id: 999, tick: 400 }),
            &tracked,
        );
        assert_eq!(
            ambient.defeat_cause.as_deref(),
            Some("hero_death_from_ambient_enemy")
        );
    }

    #[test]
    fn added_true_death_boundary_preserves_exact_assault_cause() {
        use crate::game::SettlementCrisis;

        let owner = 7;
        let mut app = App::new();
        app.insert_resource(GameTick(200))
            .insert_resource(CrisisBalanceTelemetryConfig {
                sample_interval_ticks: Some(1),
            })
            .init_resource::<SettlementCrisisState>()
            .init_resource::<CrisisBalanceTelemetryState>()
            .add_systems(Update, crisis_true_death_telemetry_system);
        app.world_mut()
            .resource_mut::<SettlementCrisisState>()
            .0
            .insert(
                owner,
                SettlementCrisis {
                    phase: CrisisPhase::AssaultActive,
                    assault_id: Some(42),
                    assault_spawn_generation: 3,
                    assault_unit_ids: vec![101],
                    ..SettlementCrisis::default()
                },
            );
        app.world_mut().spawn((
            PlayerId(owner),
            SubclassHero,
            StateDead {
                dead_at: 100,
                killer: "Goblin Pillager".to_string(),
            },
            LastAttacker { id: 101, tick: 100 },
            TrueDeath { true_death_at: 200 },
        ));

        app.update();

        let engagement = &app
            .world()
            .resource::<CrisisBalanceTelemetryState>()
            .get(&owner)
            .unwrap()
            .engagement;
        assert_eq!(engagement.true_death_tick, Some(200));
        assert_eq!(
            engagement.defeat_cause.as_deref(),
            Some("hero_true_death_from_assault")
        );
    }

    #[test]
    fn tick_cap_no_target_and_pathing_failures_use_stable_reasons() {
        let mut no_target = CrisisEngagementTelemetry::default();
        no_target.record_no_target_stall();
        no_target.record_tick_cap_failure();
        assert_eq!(
            no_target.engagement_failure_reason.as_deref(),
            Some("no_valid_target")
        );
        assert_eq!(
            no_target.defeat_cause.as_deref(),
            Some("assault_unresolved_at_tick_cap")
        );

        let mut pathing = CrisisEngagementTelemetry::default();
        pathing.record_pathing_stall();
        pathing.record_tick_cap_failure();
        assert_eq!(
            pathing.engagement_failure_reason.as_deref(),
            Some("path_unreachable")
        );

        let mut plain_cap = CrisisEngagementTelemetry::default();
        plain_cap.first_attacker_visible_tick = Some(1);
        plain_cap.first_attacker_target_acquired_tick = Some(2);
        plain_cap.attacker_attack_attempts = 1;
        plain_cap.attacker_attacks_accepted = 1;
        plain_cap.attacker_hits = 1;
        plain_cap.record_tick_cap_failure();
        assert_eq!(
            plain_cap.engagement_failure_reason.as_deref(),
            Some("tick_cap")
        );
    }

    fn sampled_stall_app(attacker_target: i32) -> App {
        use crate::game::SettlementCrisis;

        let owner = 7;
        let assault_id = 42;
        let attribution = CrisisAssaultUnit {
            owner_player_id: owner,
            assault_id,
            spawn_generation: 1,
        };
        let stats = Stats {
            hp: 100,
            stamina: Some(100),
            mana: Some(100),
            base_hp: 100,
            base_stamina: Some(100),
            base_mana: Some(100),
            base_def: 0,
            damage_range: Some(1),
            base_damage: Some(1),
            base_speed: Some(1),
            base_vision: Some(10),
        };
        let mut app = App::new();
        app.insert_resource(GameTick(0))
            .insert_resource(CrisisBalanceTelemetryConfig {
                sample_interval_ticks: Some(1),
            })
            .init_resource::<SettlementCrisisState>()
            .init_resource::<CrisisBalanceTelemetryState>()
            .add_systems(Update, crisis_engagement_snapshot_system);
        app.world_mut()
            .resource_mut::<SettlementCrisisState>()
            .0
            .insert(
                owner,
                SettlementCrisis {
                    phase: CrisisPhase::AssaultActive,
                    assault_id: Some(assault_id),
                    assault_spawn_generation: 1,
                    assault_started_tick: Some(0),
                    assault_unit_ids: vec![101],
                    ..SettlementCrisis::default()
                },
            );
        app.world_mut().spawn((
            Id(owner),
            PlayerId(owner),
            Position { x: 5, y: 5 },
            Viewshed { range: 10 },
            State::None,
            Class("Character".to_string()),
            Subclass::Hero,
            SubclassHero,
            HeroClass::Warrior,
            stats.clone(),
            Inventory {
                owner,
                items: Vec::new(),
            },
            Effects(std::collections::HashMap::new()),
        ));
        app.world_mut().spawn((
            Id(101),
            PlayerId(NPC_PLAYER_ID),
            Position { x: 10, y: 5 },
            State::None,
            Class("Character".to_string()),
            Subclass::Npc,
            attribution,
            VisibleTarget::new(attacker_target),
        ));
        app
    }

    #[test]
    fn snapshot_pipeline_records_no_target_stall_at_tick_cap() {
        let mut app = sampled_stall_app(NO_TARGET);
        app.update();
        app.world_mut().resource_mut::<GameTick>().0 = 1;
        app.update();

        let mut state = app
            .world_mut()
            .resource_mut::<CrisisBalanceTelemetryState>();
        let engagement = &mut state.get_mut(&7).expect("owner telemetry").engagement;
        assert!(engagement.ticks_without_any_valid_target > 0);
        engagement.record_tick_cap_failure();
        assert_eq!(
            engagement.engagement_failure_reason.as_deref(),
            Some("no_valid_target")
        );
    }

    #[test]
    fn snapshot_pipeline_records_out_of_range_pathing_stall_at_tick_cap() {
        let mut app = sampled_stall_app(7);
        app.update();
        app.world_mut().resource_mut::<GameTick>().0 = 1;
        app.update();

        let mut state = app
            .world_mut()
            .resource_mut::<CrisisBalanceTelemetryState>();
        let engagement = &mut state.get_mut(&7).expect("owner telemetry").engagement;
        assert!(engagement.ticks_with_target_but_no_movement > 0);
        assert!(engagement.pathing_stall_ticks > 0);
        engagement.record_tick_cap_failure();
        assert_eq!(
            engagement.engagement_failure_reason.as_deref(),
            Some("path_unreachable")
        );
    }

    #[test]
    fn wall_and_core_contact_distinguishes_hits_from_damage_and_counts_destroyed_core_once() {
        let mut engagement = CrisisEngagementTelemetry::default();
        engagement.record_attacker_target(101, 10, Subclass::Wall, false, 20);
        engagement.record_attacker_hit(10, Subclass::Wall, true, false, 0, false, 21);
        engagement.record_attacker_hit(10, Subclass::Wall, true, false, 8, false, 22);
        // Core event truth must work before either target acquisition or the
        // periodic sampler has populated the known-core set.
        engagement.record_attacker_hit(11, Subclass::Storage, true, true, 5, true, 24);
        engagement.record_attacker_hit(11, Subclass::Storage, true, true, 0, true, 25);
        // The wall may no longer be in a later live-structure query after this
        // damage. Historical contact remains authoritative.
        engagement.update_settlement_contact_failure_reason();

        assert_eq!(engagement.wall_target_acquisitions, 1);
        assert_eq!(engagement.wall_hits, 2);
        assert_eq!(engagement.wall_damage_absorbed, 8);
        assert_eq!(engagement.first_wall_damage_tick, Some(22));
        assert_eq!(engagement.core_structure_damage, 5);
        assert_eq!(engagement.core_structures_destroyed, 1);
        assert_eq!(engagement.first_core_damage_tick, Some(24));
        assert_eq!(engagement.settlement_contact_failure_reason, None);
    }

    #[test]
    fn available_blocking_wall_requires_a_complete_adjacent_core_ring() {
        let core = Position { x: 10, y: 10 };
        let mut ring = Map::range((core.x, core.y), 1)
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .filter(|position| Map::dist(core, *position) == 1)
            .collect::<Vec<_>>();
        assert_eq!(ring.len(), 6);
        assert!(has_complete_blocking_wall_ring(core, &ring));

        ring.pop();
        assert!(!has_complete_blocking_wall_ring(core, &ring));
        assert!(!has_complete_blocking_wall_ring(
            core,
            &[Position { x: 1, y: 1 }]
        ));
    }

    #[test]
    fn core_structure_classification_requires_marker_human_owner_built_live_and_non_wall() {
        let marker = ClassStructure;
        let class = Class(crate::constants::CLASS_STRUCTURE.to_string());
        let owner = PlayerId(7);

        assert!(is_live_built_human_core_structure(
            Some(&marker),
            &class,
            &owner,
            Subclass::Storage,
            State::None,
            false,
        ));
        assert!(!is_live_built_human_core_structure(
            None,
            &class,
            &owner,
            Subclass::Storage,
            State::None,
            false,
        ));
        assert!(!is_live_built_human_core_structure(
            Some(&marker),
            &class,
            &PlayerId(NPC_PLAYER_ID),
            Subclass::Storage,
            State::None,
            false,
        ));
        assert!(!is_live_built_human_core_structure(
            Some(&marker),
            &class,
            &owner,
            Subclass::Wall,
            State::None,
            false,
        ));
        assert!(!is_live_built_human_core_structure(
            Some(&marker),
            &class,
            &owner,
            Subclass::Storage,
            State::Building,
            false,
        ));
        assert!(!is_live_built_human_core_structure(
            Some(&marker),
            &class,
            &owner,
            Subclass::Storage,
            State::None,
            true,
        ));
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
                game_tick: 500,
                attacker_id,
                attacker_player_id,
                attacker_subclass: Subclass::Hero,
                attacker_crisis,
                target_id,
                target_player_id,
                target_subclass: Subclass::Hero,
                target_is_structure: false,
                target_is_core_structure: false,
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

    #[test]
    fn engagement_observers_exclude_ambient_and_stale_attribution() {
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
            .insert_resource(CrisisBalanceTelemetryConfig {
                sample_interval_ticks: Some(1),
            })
            .add_observer(crisis_attack_telemetry_observer)
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
            .record_launch_units(&[101]);
        let entity = app.world_mut().spawn_empty().id();
        let attack = |stage, attacker_crisis, target_crisis| CrisisAttackTelemetryEvent {
            entity,
            game_tick: 50,
            stage,
            attacker_id: owner,
            attacker_player_id: owner,
            attacker_subclass: Subclass::Hero,
            attacker_crisis,
            target_id: 101,
            target_player_id: NPC_PLAYER_ID,
            target_subclass: Subclass::Npc,
            target_is_structure: false,
            target_is_core_structure: false,
            target_crisis,
        };

        app.world_mut()
            .trigger(attack(CrisisAttackTelemetryStage::Requested, None, None));
        app.world_mut().trigger(attack(
            CrisisAttackTelemetryStage::Requested,
            None,
            Some(attribution),
        ));
        app.world_mut().trigger(attack(
            CrisisAttackTelemetryStage::Accepted,
            None,
            Some(attribution),
        ));
        let stale = CrisisAssaultUnit {
            assault_id: 41,
            ..attribution
        };
        app.world_mut().trigger(attack(
            CrisisAttackTelemetryStage::Accepted,
            None,
            Some(stale),
        ));

        app.world_mut().trigger(CrisisCombatTelemetryEvent {
            entity,
            game_tick: 51,
            attacker_id: owner,
            attacker_player_id: owner,
            attacker_subclass: Subclass::Hero,
            attacker_crisis: None,
            target_id: 101,
            target_player_id: NPC_PLAYER_ID,
            target_subclass: Subclass::Npc,
            target_is_structure: false,
            target_is_core_structure: false,
            target_crisis: Some(attribution),
            effective_damage: 9,
            killed: false,
        });
        app.world_mut().trigger(CrisisCombatTelemetryEvent {
            entity,
            game_tick: 52,
            attacker_id: owner,
            attacker_player_id: owner,
            attacker_subclass: Subclass::Hero,
            attacker_crisis: None,
            target_id: 999,
            target_player_id: NPC_PLAYER_ID,
            target_subclass: Subclass::Npc,
            target_is_structure: false,
            target_is_core_structure: false,
            target_crisis: None,
            effective_damage: 99,
            killed: false,
        });
        // Event-time classification preserves a lethal first hit even when the
        // periodic snapshot has not yet discovered this core structure.
        app.world_mut().trigger(CrisisCombatTelemetryEvent {
            entity,
            game_tick: 53,
            attacker_id: 101,
            attacker_player_id: NPC_PLAYER_ID,
            attacker_subclass: Subclass::Npc,
            attacker_crisis: Some(attribution),
            target_id: 701,
            target_player_id: owner,
            target_subclass: Subclass::Storage,
            target_is_structure: true,
            target_is_core_structure: true,
            target_crisis: None,
            effective_damage: 4,
            killed: true,
        });

        let engagement = &app
            .world()
            .resource::<CrisisBalanceTelemetryState>()
            .get(&owner)
            .unwrap()
            .engagement;
        assert_eq!(engagement.hero_attack_attempts, 1);
        assert_eq!(engagement.hero_attacks_accepted, 1);
        assert_eq!(engagement.hero_hits, 1);
        assert_eq!(engagement.hero_damage_dealt_to_assault, 9);
        assert_eq!(engagement.first_hero_hit_tick, Some(51));
        assert_eq!(engagement.first_core_damage_tick, Some(53));
        assert_eq!(engagement.core_structure_damage, 4);
        assert_eq!(engagement.core_structures_destroyed, 1);
    }
}
