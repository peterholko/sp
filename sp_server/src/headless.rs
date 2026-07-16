// In-process headless test harness for sp_server.
//
// Builds the Bevy game `App` directly (no TLS / WebSocket / Postgres / real-time
// scheduler), drives it with a deterministic scripted bot, and fast-forwards
// game time by pumping `app.update()`. Run many full games back-to-back to
// collect balance/metrics data. See `headless_bot.rs` for the bot and
// `bin/headless_runner.rs` for the multi-game runner.
//
// Isolation: each `HeadlessGame` owns its own `App` (its own `World`/resources).
// Dropping and recreating a `HeadlessGame` between runs fully isolates them — the
// only process-global statics (`LOG_RELOAD_HANDLE`, `TILESET`) hold no per-game
// mutable state and are not touched on the headless path.

use std::collections::{BTreeMap, HashSet};

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use big_brain::prelude::ThinkerBuilder;
use crossbeam_channel::{unbounded, Sender as CBSender};
use serde::Serialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::common::Transport;
use crate::common::{Heat, Hunger, Target, TaskTarget, Thirst, Tired};
use crate::constants::{
    DATABASE_MANAGER_ID, FOOD, GAME_ANIMAL, GAME_TICKS_PER_DAY, PLANT, SPRING_WATER,
};
use crate::crisis_balance::{
    CrisisBalanceScenario, CrisisBalanceTelemetry, CrisisBalanceTelemetryConfig,
    CrisisBalanceTelemetryState, GoblinCrisisBalanceConfigSnapshot,
};
use crate::database::DatabaseEvent;
use crate::effect::{Effect, Effects};
use crate::encounter::Encounter;
use crate::event::{GameEvent, GameEventType, GameEvents, MapEvents, Spell, VisibleEvent};
use crate::farm::{CropStages, Crops};
use crate::game::{
    crisis_balance_snapshot_system, BoundMonolith, Client, Clients, CrisisAssaultUnit, CrisisPhase,
    CrisisTelemetryState, DatabaseClient, DatabaseManagers, GameTick, LegendaryThreat,
    LegendaryThreatState, Merchant, MerchantSailState, Monolith, NetworkReceiver, Objectives,
    PlayerIntroEncounters, PlayerIntroState, PlayerObjectives, PlayerRunScore, PlayerStats,
    PlayerVictory, RunScoreState, SettlementCrisis, SettlementCrisisState, SurvivalDirectorMode,
    VictoryState,
};
use crate::ids::{EntityObjMap, Ids};
use crate::item::{AttrKey, AttrVal, Inventory, Slot};
use crate::map::Map;
use crate::obj::{
    ActiveTask, Assignment, Assignments, BuildUpgradeState, Class, ClassStructure, HeroClass, Id,
    LastCombatTick, LastDamageTick, Misc, Name, Obj, Order, PlayerId, Position, State,
    StateBuilding, StateDead, Stats, Subclass, SubclassHero, SubclassNPC, SubclassVillager,
    Template, TrueDeath, Viewshed, WorkEntry, WorkQueue, WorkStatus, WorkType,
};
use crate::player_setup::StartLocations;
use crate::resource::Resources;
use crate::safe_logout::{
    is_player_offline_protected, record_player_combat_activity, CancelSafeLogout,
    PlayerPresenceRecord, PlayerWorldPresence, PlayerWorldPresenceState, ProtectedRunKey,
    RequestSafeLogout, SafeLogoutCancelReason, SafeLogoutRejectionReason, SafeLogoutTelemetry,
    SafeLogoutTelemetryState,
};
use crate::skill::Skills;
use crate::structure::Structure;
use crate::templates::Templates;
use crate::{build_headless_app_with_director, AppState, PlayerEvent, ResponsePacket};

// Deterministic player id for the single headless hero. MUST be < MAX_PLAYER_ID
// (1000) so `PlayerId::is_human()` is true and NPC factions (player id 1000+)
// stay distinct.
pub const HEADLESS_PLAYER_ID: i32 = 1;

pub const PREPARATION_PAIR_START_LOCATION: &str = "startpos3";
pub const PREPARATION_STOCKADE_ANCHOR: Position = Position { x: 13, y: 13 };
const PREPARATION_COMMON_STRUCTURE_ANCHOR: Position = Position { x: 16, y: 13 };
const PREPARATION_STOCKADE_HP: i32 = 20;
pub const CHECKPOINT4_BLOCKING_STOCKADE_COUNT: i32 = 6;

/// Checkpoint 3's synthetic preparation comparisons. These labels are part of
/// the versioned runner output; keep them stable for report consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreparationComparison {
    ExistingWalls,
    EquipmentPrepared,
    HealingPrepared,
    CombinedPreparation,
}

impl PreparationComparison {
    pub const ALL: [Self; 4] = [
        Self::ExistingWalls,
        Self::EquipmentPrepared,
        Self::HealingPrepared,
        Self::CombinedPreparation,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ExistingWalls => "existing_walls",
            Self::EquipmentPrepared => "equipment_prepared",
            Self::HealingPrepared => "healing_prepared",
            Self::CombinedPreparation => "combined_preparation",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|comparison| comparison.label() == label)
    }

    pub const fn includes_wall(self) -> bool {
        matches!(self, Self::ExistingWalls | Self::CombinedPreparation)
    }

    pub const fn includes_equipment(self) -> bool {
        matches!(self, Self::EquipmentPrepared | Self::CombinedPreparation)
    }

    pub const fn includes_healing(self) -> bool {
        matches!(self, Self::HealingPrepared | Self::CombinedPreparation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreparationPairLeg {
    Control,
    Treatment,
}

impl PreparationPairLeg {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Treatment => "treatment",
        }
    }

    const fn receives(self, included: bool) -> bool {
        matches!(self, Self::Treatment) && included
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparationAssaultGeometry {
    pub template: String,
    pub template_ordinal: u32,
    pub position: [i32; 2],
}

/// Exact ECS combat-stat projection. Keeping the optional fields distinguishes
/// an absent resource from a real zero, and makes the representation Eq-safe.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PreparationCombatStatsFingerprint {
    pub hp: i32,
    pub stamina: Option<i32>,
    pub mana: Option<i32>,
    pub base_hp: i32,
    pub base_stamina: Option<i32>,
    pub base_mana: Option<i32>,
    pub base_defence: i32,
    pub damage_range: Option<i32>,
    pub base_damage: Option<i32>,
    pub base_speed: Option<i32>,
    pub base_vision: Option<u32>,
}

impl PreparationCombatStatsFingerprint {
    fn from_stats(stats: &Stats) -> Self {
        Self {
            hp: stats.hp,
            stamina: stats.stamina,
            mana: stats.mana,
            base_hp: stats.base_hp,
            base_stamina: stats.base_stamina,
            base_mana: stats.base_mana,
            base_defence: stats.base_def,
            damage_range: stats.damage_range,
            base_damage: stats.base_damage,
            base_speed: stats.base_speed,
            base_vision: stats.base_vision,
        }
    }
}

/// Raw IEEE-754 bits preserve exact need values and rates while allowing Eq.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PreparationNeedsFingerprint {
    pub thirst_bits: u32,
    pub thirst_per_tick_bits: u32,
    pub hunger_bits: u32,
    pub hunger_per_tick_bits: u32,
    pub tired_bits: u32,
    pub tired_per_tick_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparationEffectFingerprint {
    pub effect: String,
    pub duration_or_deadline_tick: i32,
    pub amplifier_bits: u32,
    pub stacks: i32,
}

fn preparation_effect_fingerprints(effects: &Effects) -> Vec<PreparationEffectFingerprint> {
    let mut fingerprints = effects
        .0
        .iter()
        .map(|(effect, (duration_or_deadline_tick, amplifier, stacks))| {
            PreparationEffectFingerprint {
                effect: effect.clone().to_str(),
                duration_or_deadline_tick: *duration_or_deadline_tick,
                amplifier_bits: amplifier.to_bits(),
                stacks: *stacks,
            }
        })
        .collect::<Vec<_>>();
    fingerprints.sort_by(|left, right| {
        left.effect
            .cmp(&right.effect)
            .then(
                left.duration_or_deadline_tick
                    .cmp(&right.duration_or_deadline_tick),
            )
            .then(left.amplifier_bits.cmp(&right.amplifier_bits))
            .then(left.stacks.cmp(&right.stacks))
    });
    fingerprints
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparationAssaultUnitFingerprint {
    pub template: String,
    // Retained for runner-output compatibility; `combat_stats` is authoritative.
    pub hp: i32,
    pub base_hp: i32,
    pub combat_stats: PreparationCombatStatsFingerprint,
    pub effects: Vec<PreparationEffectFingerprint>,
    pub last_combat_tick: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparationInventoryFingerprint {
    pub name: String,
    pub class: String,
    pub subclass: String,
    pub slot: Option<String>,
    pub quantity: i32,
    pub equipped: bool,
}

impl PreparationInventoryFingerprint {
    fn from_item(item: &crate::item::Item) -> Self {
        Self {
            name: item.name.clone(),
            class: item.class.clone(),
            subclass: item.subclass.clone(),
            slot: Slot::to_str(item.slot),
            quantity: item.quantity,
            equipped: item.equipped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparationStructureFingerprint {
    pub template: String,
    pub subclass: String,
    pub position: [i32; 2],
    pub state: String,
    pub hp: i32,
    pub base_hp: i32,
}

/// Selected observed launch fields expected to be identical after normalizing
/// only the declared synthetic treatment. Assault composition and HP are matched,
/// while actual per-leg spawn positions are reported separately as geometry.
/// This is neither a full-ECS nor an RNG fingerprint.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PreparationCommonLaunchFingerprint {
    pub start_location: String,
    pub world_tick: i32,
    pub hero_class: String,
    pub hero_template: String,
    pub hero_position: [i32; 2],
    pub hero_hp: i32,
    pub hero_base_hp: i32,
    pub hero_base_defence: i32,
    pub hero_combat_stats: PreparationCombatStatsFingerprint,
    pub hero_needs: PreparationNeedsFingerprint,
    pub hero_effects: Vec<PreparationEffectFingerprint>,
    /// Accessible combat-lock state. The true player attack cooldown is system-
    /// local production state and cannot be projected from the ECS World.
    pub hero_last_combat_tick: i32,
    pub hero_state: String,
    pub crisis_phase: String,
    pub crisis_pressure: i32,
    pub crisis_online_active_ticks: i32,
    pub crisis_phase_online_ticks: i32,
    pub crisis_assault_started_tick: Option<i32>,
    pub non_crisis_living_hostiles: i32,
    pub normalized_inventory: Vec<PreparationInventoryFingerprint>,
    pub normalized_structures: Vec<PreparationStructureFingerprint>,
    pub assault_units: Vec<PreparationAssaultUnitFingerprint>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PreparationFixtureState {
    pub completed_structures: i32,
    pub completed_wall_segments: i32,
    pub completed_stockades: i32,
    pub declared_anchor_stockades: Vec<PreparationStructureFingerprint>,
    pub hide_wraps: i32,
    pub hide_wraps_equipped: bool,
    pub hide_wraps_items: Vec<PreparationInventoryFingerprint>,
    pub tattered_shirt_items: Vec<PreparationInventoryFingerprint>,
    pub crude_bandages: i32,
    pub crude_bandage_items: Vec<PreparationInventoryFingerprint>,
    pub other_healing_items: i32,
}

fn expected_preparation_stockade() -> PreparationStructureFingerprint {
    PreparationStructureFingerprint {
        template: "Stockade".to_string(),
        subclass: "wall".to_string(),
        position: [PREPARATION_STOCKADE_ANCHOR.x, PREPARATION_STOCKADE_ANCHOR.y],
        state: "none".to_string(),
        hp: PREPARATION_STOCKADE_HP,
        base_hp: PREPARATION_STOCKADE_HP,
    }
}

fn expected_hide_wraps(equipped: bool) -> PreparationInventoryFingerprint {
    PreparationInventoryFingerprint {
        name: "Hide Wraps".to_string(),
        class: "Armor".to_string(),
        subclass: "Chest".to_string(),
        slot: Some("Chest".to_string()),
        quantity: 1,
        equipped,
    }
}

fn expected_crude_bandage() -> PreparationInventoryFingerprint {
    PreparationInventoryFingerprint {
        name: "Crude Bandage".to_string(),
        class: "Medical".to_string(),
        subclass: "Bandage".to_string(),
        slot: None,
        quantity: 1,
        equipped: false,
    }
}

fn expected_tattered_shirt(equipped: bool) -> PreparationInventoryFingerprint {
    PreparationInventoryFingerprint {
        name: "Tattered Shirt".to_string(),
        class: "Clothing".to_string(),
        subclass: "Shirt".to_string(),
        slot: Some("Chest".to_string()),
        quantity: 1,
        equipped,
    }
}

fn normalize_declared_inventory_artifact(
    comparison: PreparationComparison,
    mut item: PreparationInventoryFingerprint,
) -> Option<PreparationInventoryFingerprint> {
    if comparison.includes_healing() && item.name == "Crude Bandage" {
        return None;
    }
    if comparison.includes_equipment() && item.name == "Hide Wraps" {
        item.equipped = false;
    }
    if comparison.includes_equipment() && item.name == "Tattered Shirt" {
        // Equipping Hide Wraps through the production event necessarily
        // displaces this exact starting chest item. Normalize that one known
        // consequence, not the rest of the chest slot.
        item.equipped = true;
    }
    Some(item)
}

fn is_declared_stockade_artifact(
    comparison: PreparationComparison,
    structure: &PreparationStructureFingerprint,
) -> bool {
    comparison.includes_wall()
        && structure.template == "Stockade"
        && structure.position == [PREPARATION_STOCKADE_ANCHOR.x, PREPARATION_STOCKADE_ANCHOR.y]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparationDeclaredDifference {
    pub comparison: String,
    pub completed_stockade_delta: i32,
    pub completed_wall_segment_delta: i32,
    pub hide_wraps_equipped_changes_to_true: bool,
    pub tattered_shirt_equipped_changes_to_false: bool,
    pub crude_bandage_delta: i32,
}

impl PreparationDeclaredDifference {
    pub fn for_comparison(comparison: PreparationComparison) -> Self {
        Self {
            comparison: comparison.label().to_string(),
            completed_stockade_delta: i32::from(comparison.includes_wall()),
            completed_wall_segment_delta: i32::from(comparison.includes_wall()),
            hide_wraps_equipped_changes_to_true: comparison.includes_equipment(),
            tattered_shirt_equipped_changes_to_false: comparison.includes_equipment(),
            crude_bandage_delta: i32::from(comparison.includes_healing()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparationPairLaunch {
    pub leg: PreparationPairLeg,
    pub geometry: Vec<PreparationAssaultGeometry>,
    pub common_fingerprint: PreparationCommonLaunchFingerprint,
    pub fixture: PreparationFixtureState,
}

/// Reject a pair if anything in the normalized launch state differs, or if
/// the controlled fixtures contain any delta other than the declared one.
pub fn validate_preparation_pair_launches(
    comparison: PreparationComparison,
    control: &PreparationPairLaunch,
    treatment: &PreparationPairLaunch,
) -> Result<PreparationDeclaredDifference, String> {
    validate_preparation_pair_launches_with_wall_count(comparison, control, treatment, 1)
}

pub fn validate_checkpoint4_preparation_pair_launches(
    comparison: PreparationComparison,
    control: &PreparationPairLaunch,
    treatment: &PreparationPairLaunch,
) -> Result<PreparationDeclaredDifference, String> {
    validate_preparation_pair_launches_with_wall_count(
        comparison,
        control,
        treatment,
        CHECKPOINT4_BLOCKING_STOCKADE_COUNT,
    )
}

fn validate_preparation_pair_launches_with_wall_count(
    comparison: PreparationComparison,
    control: &PreparationPairLaunch,
    treatment: &PreparationPairLaunch,
    wall_count: i32,
) -> Result<PreparationDeclaredDifference, String> {
    if control.leg != PreparationPairLeg::Control || treatment.leg != PreparationPairLeg::Treatment
    {
        return Err("preparation pair legs must be control then treatment".to_string());
    }
    if control.common_fingerprint != treatment.common_fingerprint {
        return Err(format!(
            "undeclared common launch fingerprint mismatch: control={:?}; treatment={:?}",
            control.common_fingerprint, treatment.common_fingerprint
        ));
    }

    let expected_wall_count = if comparison.includes_wall() {
        wall_count
    } else {
        0
    };
    let mut expected = PreparationDeclaredDifference::for_comparison(comparison);
    expected.completed_stockade_delta = expected_wall_count;
    expected.completed_wall_segment_delta = expected_wall_count;
    let actual_stockade_delta = treatment
        .fixture
        .completed_stockades
        .saturating_sub(control.fixture.completed_stockades);
    let actual_wall_delta = treatment
        .fixture
        .completed_wall_segments
        .saturating_sub(control.fixture.completed_wall_segments);
    let actual_bandage_delta = treatment
        .fixture
        .crude_bandages
        .saturating_sub(control.fixture.crude_bandages);
    let treatment_stockades_valid = treatment.fixture.declared_anchor_stockades.len()
        == expected_wall_count as usize
        && treatment
            .fixture
            .declared_anchor_stockades
            .iter()
            .all(|stockade| {
                stockade.template == "Stockade"
                    && stockade.subclass == "wall"
                    && stockade.state == "none"
                    && stockade.hp == PREPARATION_STOCKADE_HP
                    && stockade.base_hp == PREPARATION_STOCKADE_HP
            })
        && (expected_wall_count != 1
            || treatment.fixture.declared_anchor_stockades
                == vec![expected_preparation_stockade()]);
    let expected_treatment_bandages = if comparison.includes_healing() {
        vec![expected_crude_bandage()]
    } else {
        Vec::new()
    };
    if actual_stockade_delta != expected.completed_stockade_delta
        || actual_wall_delta != expected.completed_wall_segment_delta
        || !control.fixture.declared_anchor_stockades.is_empty()
        || !treatment_stockades_valid
        || control.fixture.hide_wraps != 1
        || treatment.fixture.hide_wraps != 1
        || control.fixture.hide_wraps_equipped
        || treatment.fixture.hide_wraps_equipped != expected.hide_wraps_equipped_changes_to_true
        || control.fixture.hide_wraps_items != vec![expected_hide_wraps(false)]
        || treatment.fixture.hide_wraps_items
            != vec![expected_hide_wraps(comparison.includes_equipment())]
        || control.fixture.tattered_shirt_items != vec![expected_tattered_shirt(true)]
        || treatment.fixture.tattered_shirt_items
            != vec![expected_tattered_shirt(!comparison.includes_equipment())]
        || actual_bandage_delta != expected.crude_bandage_delta
        || control.fixture.crude_bandages != 0
        || !control.fixture.crude_bandage_items.is_empty()
        || treatment.fixture.crude_bandage_items != expected_treatment_bandages
        || control.fixture.other_healing_items != 1
        || treatment.fixture.other_healing_items != 1
        || treatment
            .fixture
            .completed_structures
            .saturating_sub(control.fixture.completed_structures)
            != expected.completed_stockade_delta
    {
        return Err(format!(
            "undeclared preparation fixture mismatch for {}: expected={expected:?}; control={:?}; treatment={:?}",
            comparison.label(), control.fixture, treatment.fixture
        ));
    }
    Ok(expected)
}

/// Bounded result for non-assertive Safe Logout scenario drivers. Production
/// eligibility and cancellation remain authoritative; the harness reports the
/// terminal reason instead of replacing the entire run with panic metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeLogoutCompletionOutcome {
    Completed,
    Rejected(SafeLogoutRejectionReason),
    Cancelled(SafeLogoutCancelReason),
    TimedOut,
    Unexpected(Option<PlayerWorldPresence>),
}

/// Terminal reasons shared by the Checkpoint 4 assault runners. Ordinary hero
/// death is intentionally absent: the production sanctuary resurrection cycle
/// can return the hero to the same still-active assault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssaultObservationStopReason {
    Resolved,
    HeroTrueDeath,
    HeroMissing,
    TickCap,
}

/// Decide whether a launched-assault observation is terminal without using
/// policy inactivity, lack of early damage, or an ordinary hero death as an
/// early-exit condition. This is harness-only; gameplay state remains owned by
/// the production crisis and resurrection systems.
pub fn checkpoint4_assault_observation_stop_reason(
    crisis_phase: Option<CrisisPhase>,
    hero_present: bool,
    hero_true_death: bool,
    observed_assault_ticks: i32,
    cap_ticks: i32,
) -> Option<AssaultObservationStopReason> {
    if crisis_phase == Some(CrisisPhase::Resolved) {
        Some(AssaultObservationStopReason::Resolved)
    } else if !hero_present {
        Some(AssaultObservationStopReason::HeroMissing)
    } else if hero_true_death {
        Some(AssaultObservationStopReason::HeroTrueDeath)
    } else if observed_assault_ticks >= cap_ticks {
        Some(AssaultObservationStopReason::TickCap)
    } else {
        None
    }
}

// Bounded tokio channel capacity for captured client packets. `tick()` drains
// every update so this never has to hold more than one update's worth of output.
const PACKET_CHANNEL_CAP: usize = 16_384;
const DB_CHANNEL_CAP: usize = 1_024;

// Cheap, owned snapshot of the bits of `World` the bot reasons about. Read once
// per decision step via `observe()` so the bot stays pure data-in / action-out.
pub struct WorldView {
    pub hero: Option<HeroView>,
    pub inventory: Vec<ItemView>,
    pub enemies: Vec<UnitView>,
    pub villagers: Vec<VillagerView>,
    pub pois: Vec<PoiView>,
    pub merchant: Option<MerchantView>,
    pub monolith: Option<MonolithView>,
    pub corpses: Vec<CorpseView>,
    pub structures: Vec<StructureView>,
    pub resource_tiles: Vec<ResTileView>,
    pub occupied: HashSet<(i32, i32)>,
    pub game_tick: i32,
    pub day: i32,
    pub crisis_phase: Option<CrisisPhase>,
}

impl WorldView {
    pub fn structures_built(&self) -> i32 {
        self.structures.iter().filter(|s| s.built).count() as i32
    }

    // The hero's "home" anchor for retreat: prefer the campfire, else any owned
    // built structure (the starter Burrow), else None.
    pub fn home(&self) -> Option<Position> {
        self.structures
            .iter()
            .find(|s| s.subclass == "campfire" && s.built)
            .or_else(|| self.structures.iter().find(|s| s.built))
            .map(|s| s.pos)
    }

    pub fn has_built(&self, subclass: &str) -> bool {
        self.structures
            .iter()
            .any(|s| s.subclass == subclass && s.built)
    }
}

#[derive(Clone, Copy)]
pub struct HeroView {
    pub id: i32,
    pub pos: Position,
    pub hero_class: HeroClass,
    pub hp: i32,
    pub base_hp: i32,
    pub stamina: Option<i32>,
    pub mana: Option<i32>,
    pub vision: u32,
    pub state: State,
    pub dead: bool,
    pub true_death: bool,
    // Survival needs (0..=100); the game auto-eats/drinks/sleeps only while idle.
    pub thirst: f32,
    pub hunger: f32,
    pub tired: f32,
}

impl HeroView {
    // Ready to receive a new command: idle (not moving/gathering/etc.) and alive.
    pub fn is_idle(&self) -> bool {
        !self.dead && !self.true_death && self.state == State::None
    }

    pub fn hp_frac(&self) -> f32 {
        if self.base_hp <= 0 {
            1.0
        } else {
            self.hp as f32 / self.base_hp as f32
        }
    }
}

#[derive(Clone, Copy)]
pub struct UnitView {
    pub id: i32,
    pub player_id: i32,
    pub pos: Position,
    /// Owner of the personal assault that spawned this NPC, when applicable.
    /// The balance bot uses this only to retain the owning hero's production
    /// crisis target; ordinary ambient enemies remain un-attributed.
    pub crisis_owner_player_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrisisAssaultUnitView {
    pub obj_id: i32,
    pub template: String,
    pub owner_player_id: i32,
    pub assault_id: u64,
    pub spawn_generation: u32,
    pub hp: i32,
    pub base_hp: i32,
    pub pos: Position,
    pub vision: u32,
    pub has_thinker: bool,
    pub visible_target: Option<i32>,
    pub target: Option<i32>,
    pub task_target: Option<i32>,
    pub dead: bool,
}

#[derive(Clone)]
pub struct ItemView {
    pub id: i32,
    pub name: String,
    pub class: String,
    pub subclass: String,
    pub quantity: i32,
    pub equipped: bool,
    pub is_healing: bool,
    pub is_weapon: bool,
    pub is_hunting: bool, // weapon/tool with a Hunting attr (can take down game)
    pub attack_range: u32, // 1 for ordinary melee/non-weapons
    pub feed: f32,        // Feed value (0 if not edible)
    pub food_poisoning: bool, // eating it risks food poisoning (raw meat)
}

impl ItemView {
    pub fn is_drink(&self) -> bool {
        self.class == "Drink"
    }
    // Safe to eat for hunger: has Feed and won't poison the hero.
    pub fn is_edible(&self) -> bool {
        self.class == "Food" && self.feed > 0.0 && !self.food_poisoning
    }
}

impl ItemView {
    // Mirrors item::req_matches_build: a build requirement of `req_type` is met
    // by an item whose name, class, or subclass equals it (plus Log<-Timber).
    pub fn matches_req(&self, req_type: &str) -> bool {
        req_type == self.name
            || req_type == self.class
            || req_type == self.subclass
            || (req_type == "Log" && self.class == "Timber")
    }
}

#[derive(Clone, Copy)]
pub struct VillagerView {
    pub id: i32,
    pub pos: Position,
    pub idle: bool,
    /// True while the villager has a Gather order (vs. None/other).
    pub gathering_order: bool,
    /// True while the villager is actively in the Gathering state.
    pub gathering_now: bool,
    /// Count of Food-class items the villager is currently carrying.
    pub food_carried: i32,
}

#[derive(Clone)]
pub struct PoiView {
    pub id: i32,
    pub pos: Position,
    pub template: String, // e.g. "Shipwreck"
}

#[derive(Clone)]
pub struct MerchantView {
    pub id: i32,
    pub pos: Position,
    /// True only while the merchant has sailed in and is docked (trade window
    /// open). The bot should only approach/hire when this is true.
    pub at_landing: bool,
    /// Obj ids of the villagers currently aboard the merchant, available to hire.
    pub hireable: Vec<i32>,
}

#[derive(Clone, Copy)]
pub struct MonolithView {
    pub id: i32,
    pub pos: Position,
    /// Current sanctuary level (0 = innate); upgraded with Soulshards.
    pub level: i32,
}

#[derive(Clone, Copy)]
pub struct CorpseView {
    pub id: i32,
    pub pos: Position,
    /// Item id of a Soulshard stack sitting on this corpse, ready to loot.
    pub soulshard_item: i32,
}

#[derive(Clone)]
pub struct StructureView {
    pub id: i32,
    pub pos: Position,
    pub subclass: String,
    pub founded: bool, // placed, not yet built (needs resources + build)
    pub building: bool,
    pub built: bool, // construction complete
    pub inventory: Vec<ItemView>,
}

/// Owner-scoped hero values used by deterministic offline-protection tests.
#[derive(Clone, Debug, PartialEq)]
pub struct ProtectedHeroSnapshot {
    pub hp: i32,
    pub stamina: Option<i32>,
    pub mana: Option<i32>,
    pub thirst: f32,
    pub hunger: f32,
    pub tired: f32,
    pub heat: f32,
    pub effects: Vec<(String, i32, i32)>,
    pub effect_deadlines: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProtectedVillagerSnapshot {
    pub id: i32,
    pub hp: i32,
    pub pos: Position,
    pub thirst: f32,
    pub hunger: f32,
    pub tired: f32,
    pub state: State,
    pub assignment_structure_id: Option<i32>,
    pub inventory_quantity: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProtectedStructureSnapshot {
    pub id: i32,
    pub hp: i32,
    pub work_done: Option<f32>,
    pub work_start_tick: Option<i32>,
    pub queue_entries: usize,
    pub stored_quantity: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtectedWorkDeadline {
    pub event_id: i32,
    pub kind: String,
    pub start_tick: i32,
    pub run_tick: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtectedCropSnapshot {
    pub structure_id: i32,
    pub stage: CropStages,
    pub quantity: i32,
    pub stage_start: i32,
    pub stage_end: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtectedIntroSnapshot {
    pub start_tick: i32,
    pub shipwreck_chain_started: bool,
    pub villager_spawned: bool,
    pub danger_unlocked: bool,
    pub rat_ids: Vec<i32>,
    pub phase1_npc_id: Option<i32>,
    pub first_rat_spawn_tick: i32,
    pub second_rat_spawn_tick: i32,
    pub villager_ready_tick: i32,
    pub phase1_unlock_tick: i32,
    pub spider_unlock_tick: i32,
    pub villager_event_scheduled: bool,
    pub initial_encounter_completed: bool,
    pub spider_encounter_completed: bool,
    pub run_object_ids: Vec<i32>,
}

#[derive(Clone, Copy)]
pub struct ResTileView {
    pub pos: Position,
    pub revealed: bool,        // any resource on the tile is revealed (gatherable)
    pub has_spring: bool,      // a Spring Water resource exists here (maybe hidden)
    pub spring_revealed: bool, // ...and it's revealed -> waterskins refill here
    pub has_game: bool,        // a Game Animal resource exists here (maybe hidden)
    pub game_revealed: bool,   // ...and it's revealed -> huntable with a Hunting tool
    pub has_plant: bool,       // a Plant resource exists here (maybe hidden)
    pub plant_revealed: bool,  // ...and it's revealed -> villagers gather it tool-free
}

fn to_item_view(item: &crate::item::Item) -> ItemView {
    ItemView {
        id: item.id,
        name: item.name.clone(),
        class: item.class.clone(),
        subclass: item.subclass.clone(),
        quantity: item.quantity,
        equipped: item.equipped,
        is_healing: item.attrs.contains_key(&AttrKey::Healing),
        is_weapon: item.class == "Weapon",
        is_hunting: item.attrs.contains_key(&AttrKey::Hunting),
        attack_range: match item.attrs.get(&AttrKey::AttackRange) {
            Some(AttrVal::Num(value)) if *value > 1.0 => *value as u32,
            _ => 1,
        },
        feed: match item.attrs.get(&AttrKey::Feed) {
            Some(AttrVal::Num(v)) => *v,
            _ => 0.0,
        },
        food_poisoning: item.attrs.contains_key(&AttrKey::FoodPoisoning),
    }
}

fn packet_has_tag(packet: &str, expected: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(packet)
        .ok()
        .and_then(|value| value.get("packet")?.as_str().map(str::to_owned))
        .as_deref()
        == Some(expected)
}

const fn crisis_phase_name(phase: CrisisPhase) -> &'static str {
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

const fn safe_logout_rejection_reason_name(reason: SafeLogoutRejectionReason) -> &'static str {
    match reason {
        SafeLogoutRejectionReason::NotOnline => "not_online",
        SafeLogoutRejectionReason::InvalidRun => "invalid_run",
        SafeLogoutRejectionReason::MissingHero => "missing_hero",
        SafeLogoutRejectionReason::HeroDied => "hero_died",
        SafeLogoutRejectionReason::TrueDeath => "true_death",
        SafeLogoutRejectionReason::MissingBoundMonolith => "missing_bound_monolith",
        SafeLogoutRejectionReason::MissingSanctuaryZone => "missing_sanctuary_zone",
        SafeLogoutRejectionReason::SanctuaryInvalid => "sanctuary_invalid",
        SafeLogoutRejectionReason::OutsideOwnSanctuary => "outside_own_sanctuary",
        SafeLogoutRejectionReason::AssaultActive => "assault_active",
        SafeLogoutRejectionReason::RecentCombat => "recent_combat",
        SafeLogoutRejectionReason::RecentDamage => "recent_damage",
        SafeLogoutRejectionReason::HostileNearby => "hostile_nearby",
        SafeLogoutRejectionReason::AlreadyPending => "already_pending",
        SafeLogoutRejectionReason::AlreadyProtected => "already_protected",
    }
}

const fn safe_logout_cancel_reason_name(reason: SafeLogoutCancelReason) -> &'static str {
    match reason {
        SafeLogoutCancelReason::Moved => "moved",
        SafeLogoutCancelReason::EnteredCombat => "entered_combat",
        SafeLogoutCancelReason::TookDamage => "took_damage",
        SafeLogoutCancelReason::HostileNearby => "hostile_nearby",
        SafeLogoutCancelReason::LeftSanctuary => "left_sanctuary",
        SafeLogoutCancelReason::SanctuaryInvalid => "sanctuary_invalid",
        SafeLogoutCancelReason::AssaultStarted => "assault_started",
        SafeLogoutCancelReason::HeroDied => "hero_died",
        SafeLogoutCancelReason::Disconnected => "disconnected",
        SafeLogoutCancelReason::Manual => "manual",
        SafeLogoutCancelReason::RunEnded => "run_ended",
        SafeLogoutCancelReason::InvalidState => "invalid_state",
    }
}

// Per-run metrics emitted by the runner (CSV + JSON). Field names mirror the
// game's own state structs (`PlayerRunScore`, `PlayerObjectives`,
// `PlayerVictory`) so the data lines up with in-game scoring.
#[derive(Debug, Clone, Serialize)]
pub struct RunMetrics {
    pub run_index: u32,
    pub outcome: String,
    pub killer: String, // StateDead.killer at end (e.g. "Starvation", a creature name, or "")
    pub ticks: i32,
    pub days_survived: i32,
    // PlayerRunScore
    pub waves_survived: i32,
    pub enemies_killed: i32,
    pub elites_killed: i32,
    pub captains_killed: i32,
    pub legendary_kills: i32,
    pub hideouts_cleared: i32,
    pub repairs: i32,
    pub highest_pressure_level: i32,
    pub num_deaths: u32,
    // PlayerObjectives (the 10 onboarding/goal flags)
    pub obj_scavenge_shipwreck: bool,
    pub obj_build_campfire: bool,
    pub obj_win_first_fight: bool,
    pub obj_build_3_structures: bool,
    pub obj_recruit_villager: bool,
    pub obj_explore_poi: bool,
    pub obj_choose_expansion: bool,
    pub obj_survive_5_nights: bool,
    pub obj_find_legendary_hideout: bool,
    pub obj_defeat_ashen_warlord: bool,
    // PlayerVictory
    pub victory_rescue_progress: i32,
    pub victory_prosperity: bool,
    pub victory_conquest: bool,
    // Hero end-state
    pub final_hp: i32,
    pub final_skill_total: i32,
    pub final_inventory_count: i32,
    pub structures_built: i32,
    // Personal-crisis runtime telemetry. These are appended to the runner
    // schemas so every pre-Checkpoint-4 field keeps its original name/order.
    pub crisis_highest_phase: String,
    pub crisis_final_phase: String,
    pub crisis_final_pressure: i32,
    pub crisis_signs_tick: Option<i32>,
    pub crisis_pressure_tick: Option<i32>,
    pub crisis_preparing_tick: Option<i32>,
    pub crisis_assault_ready_tick: Option<i32>,
    pub crisis_assault_active_tick: Option<i32>,
    pub crisis_resolved_tick: Option<i32>,
    pub crisis_assaults_launched: i32,
    pub crisis_assaults_resolved: i32,
    pub crisis_units_remaining: i32,
    pub crisis_status_packets_sent: i32,
    pub crisis_login_snapshots_sent: i32,
    pub crisis_duplicate_assaults: i32,
    pub personal_crisis_automatic_dusk_hordes: i32,
    pub crisis_invariants_ok: bool,
    // Safe-logout runtime telemetry. These fields are deliberately appended
    // after the complete pre-Checkpoint-4 schema.
    pub safe_logout_scenario_mode: String,
    pub safe_logout_requests: u64,
    pub safe_logout_accepted: u64,
    pub safe_logout_rejected: u64,
    pub safe_logout_cancelled: u64,
    pub safe_logout_completed: u64,
    pub safe_logout_protected_sessions_started: u64,
    pub safe_logout_resumed: u64,
    pub safe_logout_protected_ticks_total: u64,
    pub safe_logout_ordinary_disconnects: u64,
    pub safe_logout_active_assault_disconnects: u64,
    pub safe_logout_status_packets_sent: u64,
    pub safe_logout_status_packets_duplicate_suppressed: u64,
    pub safe_logout_protected_input_rejections: u64,
    pub safe_logout_protected_damage_blocks: u64,
    pub safe_logout_protected_target_rejections: u64,
    pub safe_logout_queued_events_discarded: u64,
    pub safe_logout_invariant_recoveries: u64,
    pub safe_logout_run_key_mismatches: u64,
    pub safe_logout_timer_rebases: u64,
    pub safe_logout_stale_connection_events_rejected: u64,
    pub safe_logout_rejection_reasons: BTreeMap<String, u64>,
    pub safe_logout_cancellation_reasons: BTreeMap<String, u64>,
    pub safe_logout_invariant_reasons: BTreeMap<String, u64>,
    pub safe_logout_invariants_ok: bool,
    // Milestone 3 balance observations. Existing JSON fields remain flat and
    // unchanged; these additive fields carry the richer nested telemetry.
    pub crisis_balance_scenario: String,
    pub crisis_balance_hero_class: String,
    pub crisis_balance_run_id: String,
    pub crisis_balance_tick_cap: i32,
    pub crisis_balance_tick_cap_reached: bool,
    pub crisis_balance_progression_fixture: bool,
    pub crisis_balance_config: GoblinCrisisBalanceConfigSnapshot,
    pub crisis_balance: CrisisBalanceTelemetry,
    // Additive post-baseline fields. Keep these after the original 189-column
    // balance schema so existing JSON and CSV consumers retain their prefix.
    pub crisis_warning_signs_to_launch_global_ticks: Option<i32>,
    pub crisis_warning_signs_to_launch_online_ticks: Option<i32>,
}

/// Runtime-only observations collected by the in-process harness. Gameplay
/// state remains authoritative; this tracker only samples it after updates and
/// never writes back into the world.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeadlessCrisisTelemetry {
    pub highest_phase: String,
    pub signs_tick: Option<i32>,
    pub pressure_tick: Option<i32>,
    pub preparing_tick: Option<i32>,
    pub assault_ready_tick: Option<i32>,
    pub assault_active_tick: Option<i32>,
    pub resolved_tick: Option<i32>,
    pub assaults_launched: i32,
    pub assaults_resolved: i32,
    pub units_remaining: i32,
    pub status_packets_sent: i32,
    pub login_snapshots_sent: i32,
    pub duplicate_assaults: i32,
}

pub struct HeadlessGame {
    app: App,
    player_id: i32,
    // game-tick at which the hero was spawned; survival measured relative to this
    spawn_tick: i32,
    // run for at most this many game ticks past `spawn_tick`
    max_ticks: i32,

    // crossbeam: harness -> game (player input). Game side is `NetworkReceiver`.
    event_tx: CBSender<PlayerEvent>,
    // shared client map; we hold a clone of the same Arc the resource holds.
    clients: Clients,
    // tokio mpsc: game -> harness (captured client packets / db events).
    packet_tx: mpsc::Sender<String>,
    packet_rx: mpsc::Receiver<String>,
    // Crisis packets are sparse because production delivery is deduplicated, so
    // retain them for deterministic scenarios and per-run packet telemetry.
    crisis_packet_history: Vec<String>,
    // Safe-logout status is likewise sparse and server-deduplicated. Keep a
    // dedicated history so the ten-second production flow can be asserted
    // without retaining the high-volume perception/update stream.
    safe_logout_packet_history: Vec<String>,
    // Full packet capture is opt-in around short scenarios; normal long runner
    // runs do not retain the high-volume perception/update stream.
    capture_packets: bool,
    captured_packets: Vec<String>,
    // kept alive so the dummy DatabaseManager's sender stays open.
    _db_rx: mpsc::Receiver<DatabaseEvent>,

    // number of `app.update()` calls made (debug/info only).
    tick_count: u64,
}

impl HeadlessGame {
    // Build a fresh headless game. `max_ticks` is the number of game ticks the
    // run is allowed to advance past hero spawn before `is_over()` caps it.
    pub fn new(max_ticks: i32) -> Self {
        Self::new_with_director(max_ticks, SurvivalDirectorMode::PersonalCrisis)
    }

    pub fn new_with_director(max_ticks: i32, survival_director_mode: SurvivalDirectorMode) -> Self {
        let mut app = build_headless_app_with_director(survival_director_mode);

        // Player input channel (harness -> game).
        let (event_tx, event_rx) = unbounded::<PlayerEvent>();

        // Output channels (game -> harness). Bounded tokio mpsc; `try_send` /
        // `try_recv` need no running runtime.
        let (packet_tx, packet_rx) = mpsc::channel::<String>(PACKET_CHANNEL_CAP);
        let (db_tx, db_rx) = mpsc::channel::<DatabaseEvent>(DB_CHANNEL_CAP);

        // Insert the network resources the production path would build in
        // `network_init`. We keep a clone of `clients` so `spawn_hero` can add a
        // Client routed to our packet channel.
        let clients = Clients::default();

        let database_managers = DatabaseManagers::default();
        database_managers
            .lock()
            .unwrap()
            .insert(DATABASE_MANAGER_ID, DatabaseClient { sender: db_tx });

        app.insert_resource(NetworkReceiver::new(event_rx));
        app.insert_resource(clients.clone());
        app.insert_resource(database_managers);

        let mut game = HeadlessGame {
            app,
            player_id: 0,
            spawn_tick: 0,
            max_ticks,
            event_tx,
            clients,
            packet_tx,
            packet_rx,
            crisis_packet_history: Vec::new(),
            safe_logout_packet_history: Vec::new(),
            capture_packets: false,
            captured_packets: Vec::new(),
            _db_rx: db_rx,
            tick_count: 0,
        };

        // Pump PreStartup (world_init) -> set Running -> OnEnter(Running) init.
        game.pump_until_running();

        game
    }

    fn pump_until_running(&mut self) {
        for _ in 0..32 {
            self.app.update();
            self.tick_count += 1;
            self.drain_io();
            if self.is_running() {
                break;
            }
        }
        // A couple more so OnEnter(Running) systems (e.g. init_objs) have run and
        // the first Update systems execute under AppState::Running.
        for _ in 0..2 {
            self.app.update();
            self.tick_count += 1;
            self.drain_io();
        }
    }

    fn is_running(&self) -> bool {
        self.app
            .world()
            .get_resource::<bevy::state::state::State<AppState>>()
            .map(|s| *s.get() == AppState::Running)
            .unwrap_or(false)
    }

    // Create the hero. Inserts a Client routed to our packet channel, injects the
    // exact `NewPlayer` event `handle_selected_class` would send, and ticks until
    // the hero has spawned. `class` is "Warrior" | "Ranger" | "Mage".
    pub fn spawn_hero(&mut self, class: &str, name: &str) -> i32 {
        let pid = HEADLESS_PLAYER_ID;
        self.player_id = pid;

        // Deterministic client uuid (no RNG) — the game keys by player_id, the
        // uuid is only the client-map key.
        let client = Client {
            id: Uuid::from_u128(pid as u128),
            player_id: pid,
            sender: self.packet_tx.clone(),
        };
        let displaced = self.clients.activate(client);
        debug_assert!(displaced.is_empty(), "fresh headless hero connection");

        self.inject(PlayerEvent::NewPlayer {
            player_id: pid,
            hero_name: name.to_string(),
            class_name: class.to_string(),
        });

        // Broker drains one event/tick; new_player_system spawns the hero
        // synchronously; the Login game event runs at +4 ticks.
        self.tick(8);
        self.spawn_tick = self.game_tick();

        // Experiment hook: SANCTUARY_LEVEL re-homes the nearest Monolith onto the
        // hero's base and sets its level, so we can A/B "how much does a stronger
        // sanctuary extend survival?" without first wiring the bot to earn/spend
        // Soulshards. Unset = stock behaviour.
        if let Ok(level) = std::env::var("SANCTUARY_LEVEL") {
            if let Ok(level) = level.parse::<i32>() {
                self.set_sanctuary_at_base(level);
            }
        }

        pid
    }

    /// Add a second authenticated hero without changing the primary hero that
    /// `observe`, `is_over`, and `metrics` follow. This is an explicit
    /// multi-player scenario hook for the headless runner; production setup
    /// continues to use the ordinary network/player event path.
    pub fn spawn_connected_scenario_helper(&mut self, name: &str) -> i32 {
        let helper_player_id = self.player_id + 1;
        let helper_client = Client {
            id: Uuid::from_u128(helper_player_id as u128),
            player_id: helper_player_id,
            sender: self.packet_tx.clone(),
        };
        assert!(
            self.clients.activate(helper_client).is_empty(),
            "headless scenario helper must use a fresh player id"
        );
        self.inject(PlayerEvent::NewPlayer {
            player_id: helper_player_id,
            hero_name: name.to_string(),
            class_name: "Warrior".to_string(),
        });
        self.tick(8);
        helper_player_id
    }

    /// Add one ordinary owner villager for a multi-player isolation scenario.
    /// The villager is created through the existing `SpawnVillager` event and
    /// receives no crisis-only statistics, equipment, or behavior.
    pub fn spawn_connected_scenario_villager(&mut self, player_id: i32) -> Result<i32, String> {
        self.spawn_legitimate_villager_near_settlement(player_id, false)
    }

    /// Turn a newly spawned helper into an established neighbouring player for
    /// isolation/support tests. This removes only that helper's scripted intro
    /// chain and ambient intro hostiles; it grants no combat values, resources,
    /// crisis progress, or protection.
    pub fn prepare_established_scenario_helper(&mut self, player_id: i32) {
        use crate::game::{InitialEncounterState, IntroEncounterState};

        if let Some(intro) = self
            .app
            .world_mut()
            .resource_mut::<PlayerIntroState>()
            .get_mut(&player_id)
        {
            intro.shipwreck_chain_started = true;
            intro.villager_spawned = true;
            // Keep the helper's own personal crisis locked for this focused
            // owner-assault scenario.
            intro.danger_unlocked = false;
        }
        self.app
            .world_mut()
            .resource_mut::<InitialEncounterState>()
            .remove(&player_id);
        self.app
            .world_mut()
            .resource_mut::<IntroEncounterState>()
            .insert(
                player_id,
                PlayerIntroEncounters {
                    initial_encounter: true,
                    spider_encounter: true,
                },
            );
        self.normalize_preparation_non_crisis_hostiles();
    }

    /// Arrange bounded, existing-game progression facts for the assault-focused
    /// balance scenarios. This is a headless-only analysis fixture: the bot must
    /// still build walls through normal player events, every crisis phase must
    /// advance through the production systems, and combat is unmodified.
    pub fn prepare_crisis_balance_progression_fixture(&mut self, scenario: CrisisBalanceScenario) {
        if matches!(
            scenario,
            CrisisBalanceScenario::Passive | CrisisBalanceScenario::BasicSurvival
        ) {
            return;
        }

        const FIXTURE_SANCTUARY_LEVEL: i32 = 3;
        const FIXTURE_LOGS: i32 = 18;
        const FIXTURE_GOLD: i32 = 100;

        self.set_sanctuary_at_base(FIXTURE_SANCTUARY_LEVEL);
        let player_id = self.player_id;
        let world = self.app.world_mut();
        if let Some(intro) = world.resource_mut::<PlayerIntroState>().get_mut(&player_id) {
            intro.danger_unlocked = true;
        }
        {
            let mut objectives = world.resource_mut::<Objectives>();
            let objectives = objectives.entry(player_id).or_default();
            objectives.explore_poi = true;
            objectives.choose_expansion = true;
        }

        world.resource_scope(|world, templates: Mut<Templates>| {
            let log_id = world.resource_mut::<Ids>().new_item_id();
            let gold_id = world.resource_mut::<Ids>().new_item_id();
            {
                let mut heroes =
                    world.query_filtered::<(&PlayerId, &mut Inventory), With<SubclassHero>>();
                let (_, mut inventory) = heroes
                    .iter_mut(world)
                    .find(|(owner, _)| owner.0 == player_id)
                    .expect("balance fixture hero inventory");
                inventory.new(
                    log_id,
                    "Springbranch Maple Log".to_string(),
                    FIXTURE_LOGS,
                    &templates.item_templates,
                );
            }
            {
                let mut structures = world.query_filtered::<
                    (&PlayerId, &Subclass, &State, &mut Inventory),
                    With<ClassStructure>,
                >();
                let (_, _, _, mut inventory) = structures
                    .iter_mut(world)
                    .find(|(owner, subclass, state, _)| {
                        owner.0 == player_id
                            && **subclass == Subclass::Storage
                            && Structure::is_built(**state)
                    })
                    .expect("balance fixture completed storage");
                inventory.new(
                    gold_id,
                    "Gold Coins".to_string(),
                    FIXTURE_GOLD,
                    &templates.item_templates,
                );
            }
        });
    }

    /// Restrict the production start-location resource before spawning a hero.
    /// This removes start-position RNG from the matched-observation fixture without
    /// replacing the ordinary `NewPlayer` spawn path.
    pub fn restrict_to_preparation_pair_start_location(&mut self) -> Result<(), String> {
        if self.player_id != 0 {
            return Err(
                "preparation start location must be selected before spawn_hero".to_string(),
            );
        }
        let starts = &mut self.app.world_mut().resource_mut::<StartLocations>().0;
        let Some(selected) = starts
            .iter()
            .find(|start| start.name == PREPARATION_PAIR_START_LOCATION)
            .cloned()
        else {
            return Err(format!(
                "missing required start location {PREPARATION_PAIR_START_LOCATION}"
            ));
        };
        starts.clear();
        starts.push(selected);
        Ok(())
    }

    /// Build one leg of a Checkpoint 3 matched-observation comparison and launch
    /// the existing production assault. Each leg preserves and reports its actual
    /// production launch geometry; the harness never relocates assault actors.
    pub fn prepare_preparation_pair_launch(
        &mut self,
        comparison: PreparationComparison,
        leg: PreparationPairLeg,
    ) -> Result<PreparationPairLaunch, String> {
        self.prepare_preparation_launch_internal(comparison, leg, false, false)
    }

    /// Checkpoint 4 variant of the comparison fixture. Wall-bearing comparisons
    /// put both legs' heroes at the same clear prelaunch defensive position and
    /// represent Existing Walls with six ordinary Stockades around that tile.
    /// This produces a real blocked path without relocating any actor after the
    /// production assault has launched.
    pub fn prepare_checkpoint4_preparation_pair_launch(
        &mut self,
        comparison: PreparationComparison,
        leg: PreparationPairLeg,
    ) -> Result<PreparationPairLaunch, String> {
        self.prepare_preparation_launch_internal(comparison, leg, false, true)
    }

    /// The same direct Checkpoint 4 launch fixture with one ordinary owner
    /// villager equipped with an existing Copper Training Axe before launch.
    /// No villager stats or item attributes are modified.
    pub fn prepare_villager_supported_launch(
        &mut self,
    ) -> Result<(PreparationPairLaunch, i32), String> {
        let launch = self.prepare_preparation_launch_internal(
            PreparationComparison::CombinedPreparation,
            PreparationPairLeg::Treatment,
            true,
            true,
        )?;
        let villager_id = self
            .observe()
            .villagers
            .first()
            .map(|villager| villager.id)
            .ok_or_else(|| "villager-supported launch has no owner villager".to_string())?;
        Ok((launch, villager_id))
    }

    fn prepare_preparation_launch_internal(
        &mut self,
        comparison: PreparationComparison,
        leg: PreparationPairLeg,
        add_legitimate_villager: bool,
        blocking_wall_fixture: bool,
    ) -> Result<PreparationPairLaunch, String> {
        use crate::constants::DUSK;
        use crate::game::{
            CrisisKind, InitialEncounterState, ASSAULT_READY_GRACE_TICKS,
            GOBLIN_ASSAULT_READY_PRESSURE, GOBLIN_PREPARING_PRESSURE,
        };

        if self.player_id == 0 {
            return Err("spawn_hero must run before preparing a pair leg".to_string());
        }
        // Reuse the existing bounded developed-settlement facts so production
        // pressure evaluation continues to produce a legitimate ready value.
        self.prepare_crisis_balance_progression_fixture(CrisisBalanceScenario::PreparedSolo);
        self.normalize_preparation_non_crisis_hostiles();

        let player_id = self.player_id;
        let preparing_tick = self.game_tick();
        {
            let world = self.app.world_mut();
            world
                .resource_mut::<PlayerIntroState>()
                .get_mut(&player_id)
                .ok_or_else(|| "missing player intro state".to_string())?
                .danger_unlocked = true;
            world
                .resource_mut::<InitialEncounterState>()
                .remove(&player_id);
            world.resource_mut::<SettlementCrisisState>().insert(
                player_id,
                SettlementCrisis {
                    kind: CrisisKind::Goblin,
                    phase: CrisisPhase::Preparing,
                    pressure: GOBLIN_PREPARING_PRESSURE,
                    phase_started_tick: preparing_tick,
                    online_active_ticks: 10_000,
                    phase_online_ticks: 0,
                    warning_active: true,
                    last_evaluated_tick: preparing_tick,
                    ..SettlementCrisis::default()
                },
            );
        }

        let hide_wraps_id =
            self.install_preparation_inventory(leg.receives(comparison.includes_healing()))?;
        // Keep the production three-structure pressure contributor identical;
        // otherwise adding the treatment wall would itself add 20 pressure.
        self.spawn_completed_preparation_structure("Cache", PREPARATION_COMMON_STRUCTURE_ANCHOR)?;
        let declared_stockade_positions = if blocking_wall_fixture && comparison.includes_wall() {
            self.prepare_checkpoint4_blocking_wall_positions()?
        } else if comparison.includes_wall() {
            vec![PREPARATION_STOCKADE_ANCHOR]
        } else {
            Vec::new()
        };
        if leg.receives(comparison.includes_wall()) {
            for position in &declared_stockade_positions {
                self.spawn_completed_preparation_structure("Stockade", *position)?;
            }
        }
        if leg.receives(comparison.includes_equipment()) {
            let hero_id = self
                .observe()
                .hero
                .map(|hero| hero.id)
                .ok_or_else(|| "missing hero before equipment preparation".to_string())?;
            self.inject(PlayerEvent::Equip {
                player_id,
                obj_id: hero_id,
                item_id: hide_wraps_id,
                status: true,
            });
        }
        if add_legitimate_villager {
            self.spawn_legitimate_armed_owner_villager_near_settlement()?;
        }

        // Give both legs the same event-processing budget. Only treatment sends
        // Equip, and the ordinary player-event system remains authoritative.
        self.tick(2);
        self.normalize_preparation_non_crisis_hostiles();

        let preferred_tick = self
            .game_tick()
            .div_euclid(GAME_TICKS_PER_DAY)
            .saturating_mul(GAME_TICKS_PER_DAY)
            .saturating_add(DUSK);
        let preferred_tick = if preferred_tick <= self.game_tick() {
            preferred_tick.saturating_add(GAME_TICKS_PER_DAY)
        } else {
            preferred_tick
        };
        let ready_tick = preferred_tick.saturating_sub(ASSAULT_READY_GRACE_TICKS);
        {
            let world = self.app.world_mut();
            world.resource_mut::<GameTick>().0 = ready_tick;
            world.resource_mut::<SettlementCrisisState>().insert(
                player_id,
                SettlementCrisis {
                    kind: CrisisKind::Goblin,
                    phase: CrisisPhase::AssaultReady,
                    pressure: GOBLIN_ASSAULT_READY_PRESSURE,
                    phase_started_tick: ready_tick,
                    online_active_ticks: 10_000,
                    phase_online_ticks: 0,
                    warning_active: true,
                    last_evaluated_tick: ready_tick,
                    ..SettlementCrisis::default()
                },
            );
            world.resource_mut::<GameTick>().0 = preferred_tick.saturating_sub(2);
        }
        self.tick(1);
        if self.settlement_crisis().map(|crisis| crisis.phase) != Some(CrisisPhase::AssaultReady) {
            return Err("production crisis left AssaultReady before launch tick".to_string());
        }
        if !self.crisis_assault_units().is_empty() {
            return Err("production assault launched before the requested dusk tick".to_string());
        }
        self.tick(1);
        if self.settlement_crisis().map(|crisis| crisis.phase) != Some(CrisisPhase::AssaultActive) {
            return Err("production assault did not enter AssaultActive".to_string());
        }

        self.normalize_preparation_non_crisis_hostiles();
        let geometry = self.preparation_assault_geometry()?;
        let fixture = self.preparation_fixture_state(&declared_stockade_positions)?;
        let common_fingerprint =
            self.preparation_common_launch_fingerprint(comparison, &declared_stockade_positions)?;

        Ok(PreparationPairLaunch {
            leg,
            geometry,
            common_fingerprint,
            fixture,
        })
    }

    fn prepare_checkpoint4_blocking_wall_positions(&mut self) -> Result<Vec<Position>, String> {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        let (hero_entity, original_position) = {
            let mut heroes =
                world.query_filtered::<(Entity, &PlayerId, &Position), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner, _)| owner.0 == player_id)
                .map(|(entity, _, position)| (entity, *position))
                .ok_or_else(|| "missing hero for Checkpoint 4 wall fixture".to_string())?
        };
        let occupied = {
            let mut positioned = world.query::<(Entity, &Position)>();
            positioned
                .iter(world)
                .filter(|(entity, _)| *entity != hero_entity)
                .map(|(_, position)| (position.x, position.y))
                .collect::<HashSet<_>>()
        };
        let map = world.resource::<Map>();
        let mut candidates = Map::range((original_position.x, original_position.y), 6)
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|position| {
            (
                Map::dist(original_position, *position),
                position.x,
                position.y,
            )
        });
        let (center, mut ring) = candidates
            .into_iter()
            .find_map(|center| {
                if occupied.contains(&(center.x, center.y))
                    || !Map::is_passable(center.x, center.y, map)
                {
                    return None;
                }
                let ring = Map::ring((center.x, center.y), 1)
                    .into_iter()
                    .map(|(x, y)| Position { x, y })
                    .collect::<Vec<_>>();
                (ring.len() == CHECKPOINT4_BLOCKING_STOCKADE_COUNT as usize
                    && ring.iter().all(|position| {
                        !occupied.contains(&(position.x, position.y))
                            && Map::is_passable(position.x, position.y, map)
                    }))
                .then_some((center, ring))
            })
            .ok_or_else(|| {
                "no clear six-tile prelaunch defensive ring within six tiles".to_string()
            })?;
        ring.sort_by_key(|position| (position.x, position.y));
        *world
            .get_mut::<Position>(hero_entity)
            .ok_or_else(|| "wall fixture hero lost Position".to_string())? = center;
        Ok(ring)
    }

    fn spawn_legitimate_armed_owner_villager_near_settlement(&mut self) -> Result<i32, String> {
        self.spawn_legitimate_villager_near_settlement(self.player_id, true)
    }

    fn spawn_legitimate_villager_near_settlement(
        &mut self,
        player_id: i32,
        equip_training_axe: bool,
    ) -> Result<i32, String> {
        let anchor = self
            .observe_for_player(player_id)
            .home()
            .ok_or_else(|| "villager fixture has no settlement anchor".to_string())?;
        let occupied = {
            let world = self.app.world_mut();
            let mut query = world.query::<(&Position, &State)>();
            query
                .iter(world)
                .filter(|(_, state)| state.is_blocking())
                .map(|(position, _)| (position.x, position.y))
                .collect::<HashSet<_>>()
        };
        let position = (anchor.x - 2..=anchor.x + 2)
            .flat_map(|x| (anchor.y - 2..=anchor.y + 2).map(move |y| Position { x, y }))
            .filter(|position| Map::dist(anchor, *position) <= 2)
            .find(|position| {
                !occupied.contains(&(position.x, position.y))
                    && Map::is_passable_by_obj(
                        position.x,
                        position.y,
                        true,
                        false,
                        false,
                        self.map(),
                    )
            })
            .ok_or_else(|| "no legal settlement tile for villager fixture".to_string())?;
        let existing_ids = {
            let world = self.app.world_mut();
            let mut query = world.query_filtered::<(&Id, &PlayerId), With<SubclassVillager>>();
            query
                .iter(world)
                .filter(|(_, owner)| owner.0 == player_id)
                .map(|(id, _)| id.0)
                .collect::<HashSet<_>>()
        };
        let current_tick = self.game_tick();
        let event_id = self
            .app
            .world_mut()
            .resource_mut::<Ids>()
            .new_map_event_id();
        self.app.world_mut().resource_mut::<GameEvents>().insert(
            event_id,
            GameEvent {
                event_id,
                start_tick: current_tick,
                run_tick: current_tick,
                event_type: GameEventType::SpawnVillager {
                    pos: position,
                    player_id,
                },
            },
        );
        self.tick(3);
        let (entity, villager_id) = {
            let world = self.app.world_mut();
            let mut query =
                world.query_filtered::<(Entity, &Id, &PlayerId), With<SubclassVillager>>();
            query
                .iter(world)
                .find(|(_, id, owner)| owner.0 == player_id && !existing_ids.contains(&id.0))
                .map(|(entity, id, _)| (entity, id.0))
                .ok_or_else(|| "SpawnVillager did not create an owner villager".to_string())?
        };
        let world = self.app.world_mut();
        if equip_training_axe {
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut inventory = world
                .get_mut::<Inventory>(entity)
                .ok_or_else(|| "spawned villager has no inventory".to_string())?;
            inventory.new(
                item_id,
                "Copper Training Axe".to_string(),
                1,
                &item_templates,
            );
            inventory
                .items
                .iter_mut()
                .find(|item| item.id == item_id)
                .ok_or_else(|| "villager weapon was not created".to_string())?
                .equipped = true;
        }
        *world
            .get_mut::<Position>(entity)
            .ok_or_else(|| "spawned villager has no position".to_string())? = position;
        Ok(villager_id)
    }

    /// Return a normal client command for the one treatment bandage only when
    /// the hero is wounded. The runner owns the at-most-once latch.
    pub fn preparation_bandage_use_event(&mut self) -> Option<PlayerEvent> {
        let view = self.observe();
        let hero = view.hero?;
        if hero.dead || hero.true_death || hero.hp >= hero.base_hp {
            return None;
        }
        let bandage = view
            .inventory
            .iter()
            .find(|item| item.name == "Crude Bandage" && item.quantity > 0)?;
        Some(PlayerEvent::Use {
            player_id: self.player_id,
            obj_id: hero.id,
            item_id: bandage.id,
        })
    }

    fn install_preparation_inventory(&mut self, add_bandage: bool) -> Result<i32, String> {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        let templates = world.resource::<Templates>().item_templates.clone();
        let hide_wraps_id = world.resource_mut::<Ids>().new_item_id();
        let bandage_id = add_bandage.then(|| world.resource_mut::<Ids>().new_item_id());
        let mut heroes = world.query_filtered::<(&PlayerId, &mut Inventory), With<SubclassHero>>();
        let (_, mut inventory) = heroes
            .iter_mut(world)
            .find(|(owner, _)| owner.0 == player_id)
            .ok_or_else(|| "missing hero inventory".to_string())?;
        // Preserve the production one-use starting Health Potion identically in
        // both legs. Only remove pre-existing bandages so the declared treatment
        // adds exactly one controlled Crude Bandage.
        inventory
            .items
            .retain(|item| !(item.class == "Medical" && item.subclass == "Bandage"));
        inventory.new(hide_wraps_id, "Hide Wraps".to_string(), 1, &templates);
        if let Some(bandage_id) = bandage_id {
            inventory.new(bandage_id, "Crude Bandage".to_string(), 1, &templates);
        }
        Ok(hide_wraps_id)
    }

    fn spawn_completed_preparation_structure(
        &mut self,
        template_name: &str,
        anchor: Position,
    ) -> Result<(), String> {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        if !Map::is_valid_pos((anchor.x, anchor.y))
            || !Map::is_passable(anchor.x, anchor.y, world.resource::<Map>())
        {
            return Err(format!(
                "preparation {template_name} anchor ({}, {}) is not a valid passable tile",
                anchor.x, anchor.y
            ));
        }
        let occupied = {
            let mut positioned = world.query::<&Position>();
            positioned.iter(world).any(|position| *position == anchor)
        };
        if occupied {
            return Err(format!(
                "preparation {template_name} anchor ({}, {}) is already occupied",
                anchor.x, anchor.y
            ));
        }
        let template = world
            .resource::<Templates>()
            .obj_templates
            .get(template_name.to_string());
        let obj_id = world.resource_mut::<Ids>().new_obj_id();
        let object = Obj {
            id: Id(obj_id),
            player_id: PlayerId(player_id),
            position: anchor,
            name: Name(template_name.to_string()),
            template: Template(template_name.to_string()),
            class: Class(template.class),
            subclass: Subclass::from_str(&template.subclass),
            state: State::None,
            misc: Misc {
                image: template.image,
                hsl: Vec::new(),
                groups: Vec::new(),
            },
            stats: Stats {
                hp: template.base_hp.unwrap_or(20),
                base_hp: template.base_hp.unwrap_or(20),
                stamina: None,
                mana: None,
                base_stamina: None,
                base_mana: None,
                base_def: template.base_def.unwrap_or(0),
                base_damage: None,
                damage_range: None,
                base_speed: None,
                base_vision: None,
            },
            effects: Effects(std::collections::HashMap::new()),
            inventory: Inventory {
                owner: obj_id,
                items: Vec::new(),
            },
            last_combat_tick: LastCombatTick::default(),
        };
        let entity = world
            .spawn((
                object,
                BuildUpgradeState {
                    build_upgrade_cost: template.build_cost.unwrap_or(30) as f32,
                    work_done: template.build_cost.unwrap_or(30) as f32,
                    work_per_sec: 0.0,
                    start_time: 0,
                },
                Assignments(Vec::new()),
                WorkQueue(Vec::new()),
                ClassStructure,
            ))
            .id();
        world.resource_mut::<Ids>().new_obj(obj_id, player_id);
        world.resource_mut::<EntityObjMap>().new_obj(obj_id, entity);
        Ok(())
    }

    fn normalize_preparation_non_crisis_hostiles(&mut self) {
        let world = self.app.world_mut();
        let actors = {
            let mut hostiles = world.query_filtered::<
                (Entity, &Id, &PlayerId, Option<&CrisisAssaultUnit>),
                With<SubclassNPC>,
            >();
            hostiles
                .iter(world)
                .filter(|(_, _, owner, assault)| owner.is_npc() && assault.is_none())
                .map(|(entity, id, ..)| (entity, id.0))
                .collect::<Vec<_>>()
        };
        if actors.is_empty() {
            return;
        }

        let actor_ids = actors
            .iter()
            .map(|(_, object_id)| *object_id)
            .collect::<HashSet<_>>();
        world
            .resource_mut::<MapEvents>()
            .retain(|_, event| !actor_ids.contains(&event.obj_id));
        for (entity, object_id) in actors {
            world.resource_mut::<Ids>().remove_obj(object_id);
            world.resource_mut::<EntityObjMap>().remove_obj(object_id);
            let _ = world.despawn(entity);
        }
    }

    fn preparation_assault_geometry(&mut self) -> Result<Vec<PreparationAssaultGeometry>, String> {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        let mut query = world.query::<(&Id, &Template, &Position, &CrisisAssaultUnit)>();
        let mut units = query
            .iter(world)
            .filter(|(_, _, _, assault)| assault.owner_player_id == player_id)
            .map(|(id, template, position, _)| (template.0.clone(), id.0, *position))
            .collect::<Vec<_>>();
        units.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
        if units.is_empty() {
            return Err("production launch created no attributed assault units".to_string());
        }
        let mut last_template = String::new();
        let mut ordinal = 0_u32;
        Ok(units
            .into_iter()
            .map(|(template, _, position)| {
                if template == last_template {
                    ordinal = ordinal.saturating_add(1);
                } else {
                    last_template = template.clone();
                    ordinal = 0;
                }
                PreparationAssaultGeometry {
                    template,
                    template_ordinal: ordinal,
                    position: [position.x, position.y],
                }
            })
            .collect())
    }

    fn preparation_fixture_state(
        &mut self,
        declared_stockade_positions: &[Position],
    ) -> Result<PreparationFixtureState, String> {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        let (
            completed_structures,
            completed_wall_segments,
            completed_stockades,
            mut declared_anchor_stockades,
        ) = {
            let mut structures = world.query_filtered::<
                (&PlayerId, &Template, &Subclass, &Position, &State, &Stats),
                With<ClassStructure>,
            >();
            let mut totals = (0_i32, 0_i32, 0_i32, Vec::new());
            for (_, template, subclass, position, state, stats) in
                structures.iter(world).filter(|(owner, _, _, _, state, _)| {
                    owner.0 == player_id && Structure::is_built(**state)
                })
            {
                totals.0 = totals.0.saturating_add(1);
                if *subclass == Subclass::Wall {
                    totals.1 = totals.1.saturating_add(1);
                }
                if template.0 == "Stockade" {
                    totals.2 = totals.2.saturating_add(1);
                    if declared_stockade_positions.contains(position) {
                        totals.3.push(PreparationStructureFingerprint {
                            template: template.0.clone(),
                            subclass: subclass.to_string(),
                            position: [position.x, position.y],
                            state: Obj::state_to_str(*state),
                            hp: stats.hp,
                            base_hp: stats.base_hp,
                        });
                    }
                }
            }
            totals
        };
        declared_anchor_stockades.sort_by(|left, right| {
            left.template
                .cmp(&right.template)
                .then(left.position.cmp(&right.position))
                .then(left.state.cmp(&right.state))
                .then(left.hp.cmp(&right.hp))
                .then(left.base_hp.cmp(&right.base_hp))
        });
        let mut heroes = world.query_filtered::<(&PlayerId, &Inventory), With<SubclassHero>>();
        let (_, inventory) = heroes
            .iter(world)
            .find(|(owner, _)| owner.0 == player_id)
            .ok_or_else(|| "missing hero inventory for fixture fingerprint".to_string())?;
        let hide_wraps = inventory
            .items
            .iter()
            .filter(|item| item.name == "Hide Wraps")
            .map(|item| item.quantity)
            .sum();
        let mut hide_wraps_items = inventory
            .items
            .iter()
            .filter(|item| item.name == "Hide Wraps")
            .map(PreparationInventoryFingerprint::from_item)
            .collect::<Vec<_>>();
        hide_wraps_items.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.class.cmp(&right.class))
                .then(left.subclass.cmp(&right.subclass))
                .then(left.slot.cmp(&right.slot))
                .then(left.quantity.cmp(&right.quantity))
                .then(left.equipped.cmp(&right.equipped))
        });
        let mut tattered_shirt_items = inventory
            .items
            .iter()
            .filter(|item| item.name == "Tattered Shirt")
            .map(PreparationInventoryFingerprint::from_item)
            .collect::<Vec<_>>();
        tattered_shirt_items.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.class.cmp(&right.class))
                .then(left.subclass.cmp(&right.subclass))
                .then(left.slot.cmp(&right.slot))
                .then(left.quantity.cmp(&right.quantity))
                .then(left.equipped.cmp(&right.equipped))
        });
        let crude_bandages = inventory
            .items
            .iter()
            .filter(|item| item.name == "Crude Bandage")
            .map(|item| item.quantity)
            .sum();
        let mut crude_bandage_items = inventory
            .items
            .iter()
            .filter(|item| item.name == "Crude Bandage")
            .map(PreparationInventoryFingerprint::from_item)
            .collect::<Vec<_>>();
        crude_bandage_items.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.class.cmp(&right.class))
                .then(left.subclass.cmp(&right.subclass))
                .then(left.slot.cmp(&right.slot))
                .then(left.quantity.cmp(&right.quantity))
                .then(left.equipped.cmp(&right.equipped))
        });
        let other_healing_items = inventory
            .items
            .iter()
            .filter(|item| {
                item.name != "Crude Bandage"
                    && (matches!(item.attrs.get(&AttrKey::Healing), Some(AttrVal::Num(value)) if *value > 0.0)
                        || (item.class == "Medical" && item.subclass == "Bandage"))
            })
            .map(|item| item.quantity)
            .sum();
        Ok(PreparationFixtureState {
            completed_structures,
            completed_wall_segments,
            completed_stockades,
            declared_anchor_stockades,
            hide_wraps,
            hide_wraps_equipped: inventory
                .items
                .iter()
                .any(|item| item.name == "Hide Wraps" && item.equipped),
            hide_wraps_items,
            tattered_shirt_items,
            crude_bandages,
            crude_bandage_items,
            other_healing_items,
        })
    }

    fn preparation_common_launch_fingerprint(
        &mut self,
        comparison: PreparationComparison,
        declared_stockade_positions: &[Position],
    ) -> Result<PreparationCommonLaunchFingerprint, String> {
        let player_id = self.player_id;
        let world_tick = self.game_tick();
        let world = self.app.world_mut();
        let (
            hero_class,
            hero_template,
            hero_position,
            hero_hp,
            hero_base_hp,
            hero_base_defence,
            hero_combat_stats,
            hero_needs,
            hero_effects,
            hero_last_combat_tick,
            hero_state,
            mut normalized_inventory,
        ) = {
            let mut heroes = world.query_filtered::<(
                &PlayerId,
                &HeroClass,
                &Template,
                &Position,
                &Stats,
                &State,
                &Inventory,
                &Thirst,
                &Hunger,
                &Tired,
                &Effects,
                &LastCombatTick,
            ), With<SubclassHero>>();
            let (
                _,
                hero_class,
                template,
                position,
                stats,
                state,
                inventory,
                thirst,
                hunger,
                tired,
                effects,
                last_combat_tick,
            ) = heroes
                .iter(world)
                .find(|(owner, ..)| owner.0 == player_id)
                .ok_or_else(|| "missing hero for common launch fingerprint".to_string())?;
            let inventory = inventory
                .items
                .iter()
                .map(PreparationInventoryFingerprint::from_item)
                // Normalize only the comparison's declared artifact. Unrelated
                // chest-slot and healing inventory remains in the fingerprint.
                .filter_map(|item| normalize_declared_inventory_artifact(comparison, item))
                .collect::<Vec<_>>();
            (
                hero_class.to_str().to_string(),
                template.0.clone(),
                [position.x, position.y],
                stats.hp,
                stats.base_hp,
                stats.base_def,
                PreparationCombatStatsFingerprint::from_stats(stats),
                PreparationNeedsFingerprint {
                    thirst_bits: thirst.thirst.to_bits(),
                    thirst_per_tick_bits: thirst.per_tick.to_bits(),
                    hunger_bits: hunger.hunger.to_bits(),
                    hunger_per_tick_bits: hunger.per_tick.to_bits(),
                    tired_bits: tired.tired.to_bits(),
                    tired_per_tick_bits: tired.per_tick.to_bits(),
                },
                preparation_effect_fingerprints(effects),
                last_combat_tick.0,
                Obj::state_to_str(*state),
                inventory,
            )
        };
        normalized_inventory.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.class.cmp(&right.class))
                .then(left.subclass.cmp(&right.subclass))
        });

        let mut normalized_structures = {
            let mut structures = world.query_filtered::<
                (&PlayerId, &Template, &Subclass, &Position, &State, &Stats),
                With<ClassStructure>,
            >();
            structures
                .iter(world)
                .filter(|(owner, ..)| owner.0 == player_id)
                .map(|(_, template, subclass, position, state, stats)| {
                    PreparationStructureFingerprint {
                        template: template.0.clone(),
                        subclass: subclass.to_string(),
                        position: [position.x, position.y],
                        state: Obj::state_to_str(*state),
                        hp: stats.hp,
                        base_hp: stats.base_hp,
                    }
                })
                // Only the explicitly declared treatment Stockades are
                // normalized away. Every unrelated Stockade remains comparable.
                .filter(|structure| {
                    !(comparison.includes_wall()
                        && structure.template == "Stockade"
                        && declared_stockade_positions
                            .iter()
                            .any(|position| structure.position == [position.x, position.y]))
                })
                .collect::<Vec<_>>()
        };
        normalized_structures.sort_by(|left, right| {
            left.template
                .cmp(&right.template)
                .then(left.position.cmp(&right.position))
        });

        let non_crisis_living_hostiles = {
            let mut hostiles = world.query_filtered::<
                (&PlayerId, &State, Option<&CrisisAssaultUnit>),
                With<SubclassNPC>,
            >();
            hostiles
                .iter(world)
                .filter(|(owner, state, assault)| {
                    owner.is_npc() && state.is_alive() && assault.is_none()
                })
                .count() as i32
        };
        let mut assault_units = {
            let mut units = world.query::<(
                &Template,
                &Stats,
                &State,
                &CrisisAssaultUnit,
                &Effects,
                &LastCombatTick,
            )>();
            units
                .iter(world)
                .filter(|(_, _, state, assault, ..)| {
                    assault.owner_player_id == player_id && state.is_alive()
                })
                .map(|(template, stats, _, _, effects, last_combat_tick)| {
                    PreparationAssaultUnitFingerprint {
                        template: template.0.clone(),
                        hp: stats.hp,
                        base_hp: stats.base_hp,
                        combat_stats: PreparationCombatStatsFingerprint::from_stats(stats),
                        effects: preparation_effect_fingerprints(effects),
                        last_combat_tick: last_combat_tick.0,
                    }
                })
                .collect::<Vec<_>>()
        };
        assault_units.sort_by(|left, right| {
            left.template
                .cmp(&right.template)
                .then(left.hp.cmp(&right.hp))
                .then(left.base_hp.cmp(&right.base_hp))
                .then(
                    left.combat_stats
                        .base_damage
                        .cmp(&right.combat_stats.base_damage),
                )
        });
        let crisis = world
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .cloned()
            .ok_or_else(|| "missing crisis for common launch fingerprint".to_string())?;
        Ok(PreparationCommonLaunchFingerprint {
            start_location: PREPARATION_PAIR_START_LOCATION.to_string(),
            world_tick,
            hero_class,
            hero_template,
            hero_position,
            hero_hp,
            hero_base_hp,
            hero_base_defence,
            hero_combat_stats,
            hero_needs,
            hero_effects,
            hero_last_combat_tick,
            hero_state,
            crisis_phase: crisis_phase_name(crisis.phase).to_string(),
            crisis_pressure: crisis.pressure,
            crisis_online_active_ticks: crisis.online_active_ticks,
            crisis_phase_online_ticks: crisis.phase_online_ticks,
            crisis_assault_started_tick: crisis.assault_started_tick,
            non_crisis_living_hostiles,
            normalized_inventory,
            normalized_structures,
            assault_units,
        })
    }

    pub fn is_player_connected(&self, player_id: i32) -> bool {
        self.clients.current_connection_id(player_id).is_some()
    }

    // Move the nearest Monolith onto the hero's tile and set its sanctuary level.
    // Test/experiment use only (see the SANCTUARY_LEVEL hook in spawn_hero).
    fn set_sanctuary_at_base(&mut self, level: i32) {
        let world = self.app.world_mut();
        let hero_pos = {
            let mut q = world.query_filtered::<&Position, With<SubclassHero>>();
            match q.iter(world).next() {
                Some(p) => *p,
                None => return,
            }
        };
        let mut nearest: Option<(Entity, i32, u32)> = None;
        {
            let mut q = world.query_filtered::<(Entity, &Id, &Position), With<Monolith>>();
            for (e, id, p) in q.iter(world) {
                let d = Map::distance((hero_pos.x, hero_pos.y), (p.x, p.y));
                if nearest.map_or(true, |(_, _, best_distance)| d < best_distance) {
                    nearest = Some((e, id.0, d));
                }
            }
        }
        if let Some((entity, monolith_id, _)) = nearest {
            if let Some(mut pos) = world.get_mut::<Position>(entity) {
                *pos = hero_pos;
            }
            if let Some(mut monolith) = world.get_mut::<Monolith>(entity) {
                monolith.sanctuary_level = level;
            }
            let mut heroes = world.query_filtered::<&mut BoundMonolith, With<SubclassHero>>();
            for mut bound in heroes.iter_mut(world) {
                if bound.id == monolith_id {
                    bound.pos = hero_pos;
                }
            }
        }
    }

    pub fn inject(&mut self, event: PlayerEvent) {
        // Send is effectively infallible (unbounded crossbeam channel); ignore the
        // error if the game side was somehow dropped.
        let _ = self.event_tx.send(event);
    }

    // Advance `n` game ticks by pumping `app.update()`, draining output each step
    // so the bounded packet/db channels never overflow.
    pub fn tick(&mut self, n: u32) {
        for _ in 0..n {
            self.app.update();
            self.tick_count += 1;
            self.drain_io();
        }
    }

    // Keep the bounded output channels empty. Captured packets are debug-only and
    // the bot reads `World` directly, so we discard here.
    fn drain_io(&mut self) {
        while let Ok(packet) = self.packet_rx.try_recv() {
            if packet_has_tag(&packet, "crisis_status") {
                self.crisis_packet_history.push(packet.clone());
            }
            if packet_has_tag(&packet, "safe_logout_status") {
                self.safe_logout_packet_history.push(packet.clone());
            }
            if self.capture_packets {
                self.captured_packets.push(packet);
            }
        }
        while self._db_rx.try_recv().is_ok() {}
    }

    // Drain whatever client packets are currently queued, deserialized. Mainly for
    // debugging/asserts — note `tick()` already drains, so call this between an
    // inject and the next tick if you want to observe a specific response.
    pub fn drain_packets(&mut self) -> Vec<ResponsePacket> {
        let mut out = Vec::new();
        while let Ok(s) = self.packet_rx.try_recv() {
            if let Ok(p) = serde_json::from_str::<ResponsePacket>(&s) {
                out.push(p);
            }
        }
        out
    }

    /// Start bounded, explicit capture of all outgoing packets. Long-running
    /// balance simulations should leave this disabled.
    pub fn start_packet_capture(&mut self) {
        self.captured_packets.clear();
        self.capture_packets = true;
    }

    /// Stop full capture and return every successfully decoded packet observed
    /// since `start_packet_capture`.
    pub fn finish_packet_capture(&mut self) -> Vec<ResponsePacket> {
        self.capture_packets = false;
        std::mem::take(&mut self.captured_packets)
            .into_iter()
            .filter_map(|packet| serde_json::from_str::<ResponsePacket>(&packet).ok())
            .collect()
    }

    /// Return and clear retained structured crisis-status packets. Cumulative
    /// telemetry counters are intentionally unaffected.
    pub fn take_crisis_status_packets(&mut self) -> Vec<ResponsePacket> {
        std::mem::take(&mut self.crisis_packet_history)
            .into_iter()
            .filter_map(|packet| serde_json::from_str::<ResponsePacket>(&packet).ok())
            .collect()
    }

    /// Return and clear retained safe-logout status packets. This follows the
    /// same sparse capture path as crisis status and leaves full capture off.
    pub fn take_safe_logout_status_packets(&mut self) -> Vec<ResponsePacket> {
        std::mem::take(&mut self.safe_logout_packet_history)
            .into_iter()
            .filter_map(|packet| serde_json::from_str::<ResponsePacket>(&packet).ok())
            .collect()
    }

    pub fn world(&self) -> &World {
        self.app.world()
    }

    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    pub fn player_id(&self) -> i32 {
        self.player_id
    }

    pub fn current_connection_id(&self) -> Option<Uuid> {
        self.clients.current_connection_id(self.player_id)
    }

    /// Removes every active test connection for this player while leaving the
    /// hero entity in the ECS, matching production disconnect semantics.
    pub fn disconnect_player(&mut self) {
        if let Some(connection_id) = self.current_connection_id() {
            self.clients.remove_if_current(connection_id);
        }
    }

    pub fn disconnect_scenario_player(&mut self, player_id: i32) {
        if let Some(connection_id) = self.clients.current_connection_id(player_id) {
            self.clients.remove_if_current(connection_id);
        }
    }

    /// Re-adds the harness's deterministic active client without recreating the
    /// hero or resetting run state.
    pub fn reconnect_player(&mut self) {
        let reconnect_id = ((self.player_id as u128) << 64) | (self.tick_count as u128 + 1);
        let client = Client {
            id: Uuid::from_u128(reconnect_id),
            player_id: self.player_id,
            sender: self.packet_tx.clone(),
        };
        self.clients.activate(client);
    }

    /// Reconnect through the production ordering: install the authenticated
    /// client, then enqueue the ordinary Login event that drives resynchronization.
    pub fn reconnect_player_with_login(&mut self) {
        self.reconnect_player();
        let connection_id = self
            .current_connection_id()
            .expect("authoritative headless reconnect");
        // Production authentication emits Login after inserting the client.
        // Reuse that path so reconnect snapshot tests exercise real delivery.
        self.inject(PlayerEvent::Login {
            player_id: self.player_id,
            connection_id,
        });
    }

    /// Enqueue the milestone's internal-only safe-logout request. This helper
    /// deliberately does not pump the app; callers control the exact update on
    /// which the authoritative server systems evaluate the request.
    pub fn request_safe_logout(&mut self) {
        let connection_id = self
            .current_connection_id()
            .expect("safe logout requires an authoritative connection");
        self.app.world_mut().write_message(RequestSafeLogout {
            player_id: self.player_id,
            connection_id,
        });
    }

    /// Enqueue an internal manual cancellation without advancing `GameTick`.
    pub fn cancel_safe_logout(&mut self) {
        let connection_id = self
            .current_connection_id()
            .expect("safe logout cancellation requires an authoritative connection");
        self.app.world_mut().write_message(CancelSafeLogout {
            player_id: self.player_id,
            connection_id,
        });
    }

    /// Enqueue the exact lifecycle event emitted by the authenticated WebSocket
    /// command handler. Unlike `request_safe_logout`, this exercises the
    /// production `PlayerEvent` broker and Bevy-message bridge.
    pub fn request_safe_logout_via_authenticated_ingress(&mut self) {
        let connection_id = self
            .current_connection_id()
            .expect("safe logout ingress requires an authoritative connection");
        self.inject(PlayerEvent::RequestSafeLogout {
            player_id: self.player_id,
            connection_id,
        });
    }

    /// Enqueue the authenticated production cancellation event while retaining
    /// the direct internal helper above for Checkpoint 1/2 regression tests.
    pub fn cancel_safe_logout_via_authenticated_ingress(&mut self) {
        let connection_id = self
            .current_connection_id()
            .expect("safe logout cancellation ingress requires an authoritative connection");
        self.inject(PlayerEvent::CancelSafeLogout {
            player_id: self.player_id,
            connection_id,
        });
    }

    pub fn player_presence_record(&self) -> Option<PlayerPresenceRecord> {
        self.app
            .world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&self.player_id)
            .cloned()
    }

    pub fn player_presence(&self) -> Option<PlayerWorldPresence> {
        self.player_presence_record().map(|record| record.state)
    }

    pub fn player_simulation_is_protected(&self) -> bool {
        is_player_offline_protected(
            self.player_id,
            self.app.world().resource::<PlayerWorldPresenceState>(),
        )
    }

    /// Runtime-only telemetry snapshot for this run. A currently open
    /// protection interval is added at read time, avoiding per-tick writes.
    pub fn safe_logout_telemetry(&self) -> SafeLogoutTelemetry {
        let mut snapshot = self
            .app
            .world()
            .resource::<SafeLogoutTelemetryState>()
            .get(&self.player_id)
            .cloned()
            .unwrap_or_default();
        if let Some(record) = self
            .app
            .world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&self.player_id)
        {
            if record.state == PlayerWorldPresence::OfflineProtected || record.resume_in_progress {
                if let Some(start_tick) = record.protected_since_tick {
                    snapshot.protected_ticks_total = snapshot
                        .protected_ticks_total
                        .saturating_add(self.game_tick().saturating_sub(start_tick).max(0) as u64);
                }
            }
        }
        snapshot
    }

    pub fn crisis_balance_telemetry(&self) -> CrisisBalanceTelemetry {
        self.app
            .world()
            .resource::<CrisisBalanceTelemetryState>()
            .get(&self.player_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Preserve the headless policy's target-acquisition boundary separately
    /// from the later production attack request. The target is accepted only
    /// when it belongs to this player's current live assault generation.
    pub fn record_observed_crisis_target(&mut self, target_id: i32) -> bool {
        let player_id = self.player_id;
        let Some((assault_id, spawn_generation)) = self
            .app
            .world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .and_then(|crisis| {
                (crisis.phase == CrisisPhase::AssaultActive)
                    .then_some((crisis.assault_id?, crisis.assault_spawn_generation))
            })
        else {
            return false;
        };
        let valid = {
            let world = self.app.world_mut();
            let mut query = world.query::<(&Id, &CrisisAssaultUnit, &State)>();
            query.iter(world).any(|(id, attribution, state)| {
                id.0 == target_id
                    && attribution.owner_player_id == player_id
                    && attribution.assault_id == assault_id
                    && attribution.spawn_generation == spawn_generation
                    && *state != State::Dead
            })
        };
        if !valid {
            return false;
        }
        let game_tick = self.game_tick();
        self.app
            .world_mut()
            .resource_mut::<CrisisBalanceTelemetryState>()
            .entry(player_id)
            .or_default()
            .engagement
            .record_observed_hero_target(target_id, game_tick);
        true
    }

    pub fn set_crisis_balance_sample_interval(&mut self, interval_ticks: Option<i32>) {
        self.app
            .world_mut()
            .resource_mut::<CrisisBalanceTelemetryConfig>()
            .sample_interval_ticks = interval_ticks.filter(|value| *value > 0);
    }

    pub fn safe_logout_start_tick(&self) -> Option<i32> {
        self.player_presence_record()
            .and_then(|record| record.safe_logout_requested_tick)
    }

    pub fn safe_logout_cancel_reason(&self) -> Option<SafeLogoutCancelReason> {
        self.player_presence_record()
            .and_then(|record| record.cancel_reason)
    }

    pub fn safe_logout_rejection_reason(&self) -> Option<SafeLogoutRejectionReason> {
        self.player_presence_record()
            .and_then(|record| record.rejection_reason)
    }

    pub fn protected_run_key(&self) -> Option<ProtectedRunKey> {
        self.player_presence_record()
            .and_then(|record| record.protected_run_key)
    }

    /// Drive the real internal Checkpoint 1 request/countdown until the
    /// authoritative Checkpoint 2 state is active.
    pub fn complete_valid_safe_logout(&mut self) {
        self.request_safe_logout();
        self.drive_safe_logout_to_completion();
    }

    /// Production-ingress equivalent of `complete_valid_safe_logout`, used by
    /// the explicit runner scenario rather than by the default balance bot.
    pub fn complete_valid_safe_logout_via_authenticated_ingress(&mut self) {
        self.request_safe_logout_via_authenticated_ingress();
        self.drive_safe_logout_to_completion();
    }

    /// Production-ingress Safe Logout for balance scenarios that must retain a
    /// rejected or cancelled attempt as ordinary telemetry instead of panicking.
    pub fn try_complete_valid_safe_logout_via_authenticated_ingress(
        &mut self,
    ) -> SafeLogoutCompletionOutcome {
        self.request_safe_logout_via_authenticated_ingress();
        self.try_drive_safe_logout_to_completion()
    }

    fn drive_safe_logout_to_completion(&mut self) {
        let outcome = self.try_drive_safe_logout_to_completion();
        assert_eq!(
            outcome,
            SafeLogoutCompletionOutcome::Completed,
            "safe logout did not complete: outcome={outcome:?} record={:?}",
            self.player_presence_record()
        );
    }

    fn try_drive_safe_logout_to_completion(&mut self) -> SafeLogoutCompletionOutcome {
        for _ in 0..=(crate::safe_logout::SAFE_LOGOUT_COUNTDOWN_TICKS + 8) {
            self.tick(1);
            match self.player_presence() {
                Some(PlayerWorldPresence::OfflineProtected) => {
                    return SafeLogoutCompletionOutcome::Completed;
                }
                Some(PlayerWorldPresence::SafeLogoutPending) => {}
                Some(PlayerWorldPresence::Online) => {
                    if let Some(record) = self.player_presence_record() {
                        if let Some(reason) = record.rejection_reason {
                            return SafeLogoutCompletionOutcome::Rejected(reason);
                        }
                        if let Some(reason) = record.cancel_reason {
                            return SafeLogoutCompletionOutcome::Cancelled(reason);
                        }
                    }
                }
                state => return SafeLogoutCompletionOutcome::Unexpected(state),
            }
        }
        SafeLogoutCompletionOutcome::TimedOut
    }

    pub fn disconnect_after_completed_safe_logout(&mut self) {
        assert_eq!(
            self.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        self.disconnect_player();
        self.tick(1);
        assert_eq!(
            self.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
    }

    /// Exercise the authenticated reconnect edge and ordered PostUpdate rebase.
    pub fn reconnect_and_exit_protection(&mut self) {
        self.reconnect_player_with_login();
        for _ in 0..16 {
            self.tick(1);
            if self.player_presence() == Some(PlayerWorldPresence::Online)
                && !self.player_simulation_is_protected()
            {
                return;
            }
        }
        panic!(
            "safe logout reconnect did not clear the synchronization barrier: {:?}",
            self.player_presence_record()
        );
    }

    /// Establish an `AssaultActive` personal crisis through the same
    /// deterministic clock/phase setup used by the focused headless tests.
    /// This is harness-only state arrangement for the runner's disconnect
    /// regression scenario and does not alter production gameplay.
    pub fn prepare_active_assault_disconnect_scenario(&mut self) {
        use crate::constants::DUSK;
        use crate::game::{CrisisKind, InitialEncounterState, ASSAULT_READY_GRACE_TICKS};

        let current_tick = self.game_tick();
        let mut preferred_tick =
            current_tick.div_euclid(GAME_TICKS_PER_DAY) * GAME_TICKS_PER_DAY + DUSK;
        while preferred_tick - ASSAULT_READY_GRACE_TICKS <= current_tick {
            preferred_tick += GAME_TICKS_PER_DAY;
        }
        let ready_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;
        let player_id = self.player_id;
        let world = self.app.world_mut();
        world.resource_mut::<GameTick>().0 = ready_tick;
        world
            .resource_mut::<PlayerIntroState>()
            .get_mut(&player_id)
            .expect("headless player intro state")
            .danger_unlocked = true;
        world
            .resource_mut::<InitialEncounterState>()
            .remove(&player_id);
        world.resource_mut::<SettlementCrisisState>().insert(
            player_id,
            SettlementCrisis {
                kind: CrisisKind::Goblin,
                phase: CrisisPhase::AssaultReady,
                pressure: 100,
                phase_started_tick: ready_tick,
                online_active_ticks: 10_000,
                phase_online_ticks: 0,
                warning_active: true,
                last_evaluated_tick: ready_tick,
                ..SettlementCrisis::default()
            },
        );

        self.app.world_mut().resource_mut::<GameTick>().0 = preferred_tick - 2;
        self.tick(1);
        assert_eq!(
            self.settlement_crisis().map(|crisis| crisis.phase),
            Some(CrisisPhase::AssaultReady)
        );
        assert!(self.crisis_assault_units().is_empty());
        self.tick(1);
        assert_eq!(
            self.settlement_crisis().map(|crisis| crisis.phase),
            Some(CrisisPhase::AssaultActive)
        );
        assert!(!self.crisis_assault_units().is_empty());
    }

    pub fn protected_hero_snapshot(&mut self) -> ProtectedHeroSnapshot {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        let (hero_id, mut snapshot) = {
            let mut query = world.query_filtered::<(
                &Id,
                &PlayerId,
                &Stats,
                &Thirst,
                &Hunger,
                Option<&Tired>,
                Option<&Heat>,
                &Effects,
            ), With<SubclassHero>>();
            let (id, _, stats, thirst, hunger, tired, heat, effects) = query
                .iter(world)
                .find(|(_, owner, ..)| owner.0 == player_id)
                .expect("headless hero");
            let mut effect_values = effects
                .0
                .iter()
                .map(|(effect, (duration, _, stacks))| {
                    (effect.clone().to_str(), *duration, *stacks)
                })
                .collect::<Vec<_>>();
            effect_values.sort_by(|left, right| left.0.cmp(&right.0));
            (
                id.0,
                ProtectedHeroSnapshot {
                    hp: stats.hp,
                    stamina: stats.stamina,
                    mana: stats.mana,
                    thirst: thirst.thirst,
                    hunger: hunger.hunger,
                    tired: tired.map(|value| value.tired).unwrap_or_default(),
                    heat: heat.map(|value| value.heat).unwrap_or_default(),
                    effects: effect_values,
                    effect_deadlines: Vec::new(),
                },
            )
        };
        snapshot.effect_deadlines = world
            .resource::<MapEvents>()
            .values()
            .filter(|event| {
                event.obj_id == hero_id
                    && matches!(event.event_type, VisibleEvent::EffectExpiredEvent { .. })
            })
            .map(|event| event.run_tick)
            .collect();
        snapshot.effect_deadlines.sort_unstable();
        snapshot
    }

    pub fn protected_villager_snapshots(&mut self) -> Vec<ProtectedVillagerSnapshot> {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        let mut query = world.query_filtered::<(
            &Id,
            &PlayerId,
            &Position,
            &Stats,
            &State,
            &Inventory,
            Option<&Thirst>,
            Option<&Hunger>,
            Option<&Tired>,
            Option<&Assignment>,
        ), With<SubclassVillager>>();
        let mut snapshots = query
            .iter(world)
            .filter(|(_, owner, ..)| owner.0 == player_id)
            .map(
                |(id, _, pos, stats, state, inventory, thirst, hunger, tired, assignment)| {
                    ProtectedVillagerSnapshot {
                        id: id.0,
                        hp: stats.hp,
                        pos: *pos,
                        thirst: thirst.map(|value| value.thirst).unwrap_or_default(),
                        hunger: hunger.map(|value| value.hunger).unwrap_or_default(),
                        tired: tired.map(|value| value.tired).unwrap_or_default(),
                        state: *state,
                        assignment_structure_id: assignment.map(|value| value.structure_id),
                        inventory_quantity: inventory.items.iter().map(|item| item.quantity).sum(),
                    }
                },
            )
            .collect::<Vec<_>>();
        snapshots.sort_by_key(|snapshot| snapshot.id);
        snapshots
    }

    pub fn protected_structure_snapshots(&mut self) -> Vec<ProtectedStructureSnapshot> {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        let mut query = world.query_filtered::<(
            &Id,
            &PlayerId,
            &Stats,
            &Inventory,
            Option<&BuildUpgradeState>,
            Option<&WorkQueue>,
        ), With<ClassStructure>>();
        let mut snapshots = query
            .iter(world)
            .filter(|(_, owner, ..)| owner.0 == player_id)
            .map(
                |(id, _, stats, inventory, progress, queue)| ProtectedStructureSnapshot {
                    id: id.0,
                    hp: stats.hp,
                    work_done: progress.map(|value| value.work_done),
                    work_start_tick: progress.map(|value| value.start_time),
                    queue_entries: queue.map(|value| value.0.len()).unwrap_or_default(),
                    stored_quantity: inventory.items.iter().map(|item| item.quantity).sum(),
                },
            )
            .collect::<Vec<_>>();
        snapshots.sort_by_key(|snapshot| snapshot.id);
        snapshots
    }

    pub fn protected_work_deadlines(&self) -> Vec<ProtectedWorkDeadline> {
        let player_id = self.player_id;
        let ids = self.app.world().resource::<crate::ids::Ids>();
        let mut deadlines = self
            .app
            .world()
            .resource::<GameEvents>()
            .values()
            .filter_map(|event| {
                let (object_id, kind) = match &event.event_type {
                    GameEventType::ForageEvent { forager_id } => (*forager_id, "forage"),
                    GameEventType::GatherEvent { gatherer_id, .. } => (*gatherer_id, "gather"),
                    GameEventType::RefineEvent { refiner_id, .. } => (*refiner_id, "refine"),
                    GameEventType::CraftEvent { crafter_id, .. } => (*crafter_id, "craft"),
                    GameEventType::StructureGatherEvent { structure_id, .. } => {
                        (*structure_id, "structure_gather")
                    }
                    GameEventType::StructureRefineEvent { structure_id, .. } => {
                        (*structure_id, "structure_refine")
                    }
                    GameEventType::StructureCraftEvent { structure_id, .. } => {
                        (*structure_id, "structure_craft")
                    }
                    GameEventType::StructureOperateEvent { structure_id, .. } => {
                        (*structure_id, "structure_operate")
                    }
                    GameEventType::ExperimentEvent { structure_id, .. } => {
                        (*structure_id, "experiment")
                    }
                    _ => return None,
                };
                (ids.get_player(object_id) == Some(player_id)).then_some(ProtectedWorkDeadline {
                    event_id: event.event_id,
                    kind: kind.to_string(),
                    start_tick: event.start_tick,
                    run_tick: event.run_tick,
                })
            })
            .collect::<Vec<_>>();
        deadlines.sort_by_key(|event| event.event_id);
        deadlines
    }

    pub fn protected_crop_snapshots(&self) -> Vec<ProtectedCropSnapshot> {
        let player_id = self.player_id;
        let ids = self.app.world().resource::<crate::ids::Ids>();
        let mut snapshots = self
            .app
            .world()
            .resource::<Crops>()
            .values()
            .filter(|crop| ids.get_player(crop.structure) == Some(player_id))
            .map(|crop| ProtectedCropSnapshot {
                structure_id: crop.structure,
                stage: crop.stage.clone(),
                quantity: crop.quantity,
                stage_start: crop.stage_start,
                stage_end: crop.stage_end,
            })
            .collect::<Vec<_>>();
        snapshots.sort_by_key(|crop| crop.structure_id);
        snapshots
    }

    pub fn protected_intro_snapshot(&self) -> ProtectedIntroSnapshot {
        let player_id = self.player_id;
        let world = self.app.world();
        let intro = world
            .resource::<crate::game::PlayerIntroState>()
            .get(&player_id)
            .expect("player intro state");
        let initial = world
            .resource::<crate::game::InitialEncounterState>()
            .get(&player_id)
            .expect("initial encounter state");
        let progress = world
            .resource::<crate::game::IntroEncounterState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        let mut run_object_ids = world
            .resource::<crate::player_setup::RunSpawnedObjs>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        run_object_ids.sort_unstable();
        ProtectedIntroSnapshot {
            start_tick: intro.start_tick,
            shipwreck_chain_started: intro.shipwreck_chain_started,
            villager_spawned: intro.villager_spawned,
            danger_unlocked: intro.danger_unlocked,
            rat_ids: initial.rat_ids.clone(),
            phase1_npc_id: initial.phase1_npc_id,
            first_rat_spawn_tick: initial.first_rat_spawn_tick,
            second_rat_spawn_tick: initial.second_rat_spawn_tick,
            villager_ready_tick: initial.villager_ready_tick,
            phase1_unlock_tick: initial.phase1_unlock_tick,
            spider_unlock_tick: initial.spider_unlock_tick,
            villager_event_scheduled: initial.villager_event_scheduled,
            initial_encounter_completed: progress.initial_encounter,
            spider_encounter_completed: progress.spider_encounter,
            run_object_ids,
        }
    }

    pub fn protected_stored_resource_quantity(&mut self) -> i32 {
        self.protected_structure_snapshots()
            .iter()
            .map(|structure| structure.stored_quantity)
            .sum()
    }

    pub fn queue_hostile_spell_damage_for_test(
        &mut self,
        hostile_id: i32,
        delay_ticks: i32,
    ) -> Uuid {
        let hero = self.observe().hero.expect("headless hero");
        let tick = self.game_tick();
        self.app
            .world_mut()
            .resource_mut::<MapEvents>()
            .new(
                hostile_id,
                tick.saturating_add(delay_ticks),
                VisibleEvent::SpellDamageEvent {
                    spell: Spell::ShadowBolt,
                    target_id: hero.id,
                },
            )
            .event_id
    }

    pub fn attempt_player_mutation_while_protected(&mut self, event: PlayerEvent) {
        self.inject(event);
        self.tick(1);
    }

    pub fn advance_protected_world_ticks(&mut self, ticks: u32) {
        self.tick(ticks);
    }

    /// Deterministically establish the same eligible state used by focused
    /// tests before running the optional Safe Logout runner cycle.
    pub fn prepare_safe_logout_scenario(&mut self) -> Position {
        use crate::npc::VisibleTarget;
        use crate::safe_logout::SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS;

        let sanctuary = self.place_hero_in_own_bound_sanctuary();
        let far = {
            let map = self.map();
            [
                Position { x: 0, y: 0 },
                Position {
                    x: map.width - 1,
                    y: map.height - 1,
                },
                Position {
                    x: 0,
                    y: map.height - 1,
                },
                Position {
                    x: map.width - 1,
                    y: 0,
                },
            ]
            .into_iter()
            .max_by_key(|position| {
                Map::distance((sanctuary.x, sanctuary.y), (position.x, position.y))
            })
            .expect("headless map corner")
        };
        let world = self.app.world_mut();
        let quiet_tick = world
            .resource::<GameTick>()
            .0
            .saturating_sub(SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS)
            .saturating_sub(1);
        let (hero_entity, hero_id) = {
            let mut query = world.query_filtered::<(Entity, &Id, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, _, owner)| owner.0 == self.player_id)
                .map(|(entity, id, _)| (entity, id.0))
                .expect("headless safe-logout hero")
        };
        // The balance bot may have queued a normal move on the last tick before
        // the crisis entered Ready. This fixture deliberately establishes an
        // eligible, stationary Safe Logout probe, so discard only that hero's
        // pending map events and restore its idle state before requesting.
        world
            .resource_mut::<MapEvents>()
            .retain(|_, event| event.obj_id != hero_id);
        if let Some(mut state) = world.get_mut::<State>(hero_entity) {
            if *state != State::Dead {
                *state = State::None;
            }
        }
        world
            .get_mut::<LastCombatTick>(hero_entity)
            .expect("headless hero combat timestamp")
            .0 = quiet_tick;
        if let Some(mut last_damage) = world.get_mut::<LastDamageTick>(hero_entity) {
            last_damage.0 = quiet_tick;
        }
        if let Some(record) = world
            .resource_mut::<PlayerWorldPresenceState>()
            .players
            .get_mut(&self.player_id)
        {
            record.last_combat_tick = Some(quiet_tick);
            record.last_damage_tick = Some(quiet_tick);
        }
        let active_hostiles = {
            let mut query = world.query_filtered::<(
                Entity,
                &Id,
                &PlayerId,
                &Subclass,
                &State,
                &Stats,
                Option<&StateDead>,
            ), (With<SubclassNPC>, With<VisibleTarget>)>();
            query
                .iter(world)
                .filter(|(_, _, owner, subclass, state, stats, dead)| {
                    owner.is_npc()
                        && **subclass == Subclass::Npc
                        && state.is_alive()
                        && dead.is_none()
                        && stats.hp > 0
                })
                .map(|(entity, id, ..)| (entity, id.0))
                .collect::<Vec<_>>()
        };
        let active_hostile_ids = active_hostiles
            .iter()
            .map(|(_, object_id)| *object_id)
            .collect::<HashSet<_>>();
        world
            .resource_mut::<MapEvents>()
            .retain(|_, event| !active_hostile_ids.contains(&event.obj_id));
        for (entity, _) in active_hostiles {
            *world
                .get_mut::<Position>(entity)
                .expect("headless hostile position") = far;
            world
                .get_mut::<VisibleTarget>(entity)
                .expect("headless hostile visible target")
                .target = crate::constants::NO_TARGET;
            world.entity_mut(entity).remove::<Target>();
            world.entity_mut(entity).remove::<TaskTarget>();
        }
        self.tick(1);
        assert_eq!(self.player_presence(), Some(PlayerWorldPresence::Online));
        sanctuary
    }

    /// Put the hero on the Monolith named by its authoritative `BoundMonolith`
    /// component. This never falls back to the nearest arbitrary sanctuary and
    /// does not advance the simulation.
    pub fn place_hero_in_own_bound_sanctuary(&mut self) -> Position {
        use crate::ids::EntityObjMap;

        let player_id = self.player_id;
        let world = self.app.world_mut();
        let (hero_entity, bound_monolith_id) = {
            let mut query =
                world.query_filtered::<(Entity, &PlayerId, &BoundMonolith), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner, _)| owner.0 == player_id)
                .map(|(entity, _, bound)| (entity, bound.id))
                .expect("headless hero with a bound Monolith")
        };
        let monolith_entity = world
            .resource::<EntityObjMap>()
            .get_entity(bound_monolith_id)
            .expect("bound Monolith entity-map entry");
        let monolith_pos = *world
            .get::<Position>(monolith_entity)
            .expect("bound Monolith position");
        *world
            .get_mut::<Position>(hero_entity)
            .expect("headless hero position") = monolith_pos;
        world
            .get_mut::<BoundMonolith>(hero_entity)
            .expect("headless hero bound Monolith")
            .pos = monolith_pos;
        monolith_pos
    }

    /// Move the authoritative hero position directly without pumping the app.
    /// Focused tests use this to make cancellation ordering deterministic.
    pub fn move_hero_for_test(&mut self, position: Position) {
        let player_id = self.player_id;
        let world = self.app.world_mut();
        let hero_entity = {
            let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        *world
            .get_mut::<Position>(hero_entity)
            .expect("headless hero position") = position;
    }

    /// Spawn a live hostile recognized by both production combat and the
    /// safe-logout hostility query. The entity is server-authoritative and
    /// registered in both object-id indexes, but has no behaviour tree so it
    /// remains deterministic until the test moves, kills, or removes it.
    pub fn spawn_safe_logout_test_hostile(&mut self, position: Position) -> i32 {
        use crate::event::{EventExecuting, EventExecutingState};
        use crate::ids::{EntityObjMap, Ids};
        use crate::npc::VisibleTarget;

        let hero_id = {
            let world = self.app.world_mut();
            let mut query = world.query_filtered::<(&Id, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == self.player_id)
                .map(|(id, _)| id.0)
                .expect("headless hero id")
        };
        let world = self.app.world_mut();
        let obj_id = world.resource_mut::<Ids>().new_obj_id();
        let entity = world
            .spawn((
                Id(obj_id),
                PlayerId(crate::constants::NPC_PLAYER_ID),
                position,
                Name("Wolf".to_string()),
                Template("Wolf".to_string()),
                Class("Unit".to_string()),
                Subclass::Npc,
                SubclassNPC,
                State::None,
                Misc::default(),
                Stats {
                    hp: 20,
                    stamina: Some(100),
                    mana: None,
                    base_hp: 20,
                    base_stamina: Some(100),
                    base_mana: None,
                    base_def: 0,
                    damage_range: Some(1),
                    base_damage: Some(1),
                    base_speed: Some(1),
                    base_vision: Some(8),
                },
                Effects(std::collections::HashMap::new()),
                Inventory {
                    owner: obj_id,
                    items: Vec::new(),
                },
                LastCombatTick::default(),
                VisibleTarget::new(hero_id),
            ))
            .id();
        world.entity_mut(entity).insert(EventExecuting {
            event_type: String::new(),
            state: EventExecutingState::None,
        });
        world
            .resource_mut::<Ids>()
            .new_obj(obj_id, crate::constants::NPC_PLAYER_ID);
        world.resource_mut::<EntityObjMap>().new_obj(obj_id, entity);
        obj_id
    }

    pub fn move_safe_logout_test_hostile(&mut self, obj_id: i32, position: Position) {
        use crate::ids::EntityObjMap;

        let world = self.app.world_mut();
        let entity = world
            .resource::<EntityObjMap>()
            .get_entity(obj_id)
            .expect("safe-logout test hostile");
        *world
            .get_mut::<Position>(entity)
            .expect("safe-logout test hostile position") = position;
    }

    pub fn kill_safe_logout_test_hostile(&mut self, obj_id: i32) {
        use crate::ids::EntityObjMap;

        let dead_at = self.game_tick();
        let world = self.app.world_mut();
        let entity = world
            .resource::<EntityObjMap>()
            .get_entity(obj_id)
            .expect("safe-logout test hostile");
        world
            .get_mut::<Stats>(entity)
            .expect("safe-logout test hostile stats")
            .hp = 0;
        world.entity_mut(entity).insert((
            State::Dead,
            StateDead {
                dead_at,
                killer: "Headless safe-logout test".to_string(),
            },
        ));
    }

    pub fn remove_safe_logout_test_hostile(&mut self, obj_id: i32) {
        use crate::ids::{EntityObjMap, Ids};

        let world = self.app.world_mut();
        if let Some(entity) = world.resource::<EntityObjMap>().get_entity(obj_id) {
            let _ = world.despawn(entity);
        }
        world.resource_mut::<EntityObjMap>().remove_obj(obj_id);
        world.resource_mut::<Ids>().remove_obj(obj_id);
    }

    /// Apply authoritative incoming HP loss without advancing the app. The
    /// presence reconciler observes the decrease on the caller's next update.
    pub fn damage_hero_for_test(&mut self, damage: i32) -> i32 {
        let player_id = self.player_id;
        let tick = self.game_tick();
        let world = self.app.world_mut();
        let hero_entity = {
            let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let hp = {
            let mut stats = world
                .get_mut::<Stats>(hero_entity)
                .expect("headless hero stats");
            stats.hp = stats.hp.saturating_sub(damage.max(0));
            stats.hp
        };
        world.entity_mut(hero_entity).insert(LastDamageTick(tick));
        hp
    }

    /// Record the same aggregate activity used by successful server-authorized
    /// combat commands. This helper does not tick or fabricate a client packet.
    pub fn record_player_combat_for_test(&mut self) {
        let tick = self.game_tick();
        let player_id = self.player_id;
        record_player_combat_activity(
            player_id,
            tick,
            &mut self
                .app
                .world_mut()
                .resource_mut::<PlayerWorldPresenceState>(),
        );
    }

    pub fn crisis_telemetry(&self) -> HeadlessCrisisTelemetry {
        let mut snapshot = HeadlessCrisisTelemetry::default();
        if let Some(telemetry) = self
            .app
            .world()
            .get_resource::<CrisisTelemetryState>()
            .and_then(|state| state.get(&self.player_id))
        {
            snapshot.highest_phase = crisis_phase_name(telemetry.highest_phase).to_string();
            snapshot.signs_tick = telemetry.signs_tick;
            snapshot.pressure_tick = telemetry.pressure_tick;
            snapshot.preparing_tick = telemetry.preparing_tick;
            snapshot.assault_ready_tick = telemetry.assault_ready_tick;
            snapshot.assault_active_tick = telemetry.assault_active_tick;
            snapshot.resolved_tick = telemetry.resolved_tick;
            snapshot.assaults_launched = telemetry.assaults_launched;
            snapshot.assaults_resolved = telemetry.assaults_resolved;
            snapshot.status_packets_sent = telemetry.status_packets_sent;
            snapshot.login_snapshots_sent = telemetry.login_snapshots_sent;
            snapshot.duplicate_assaults = telemetry.duplicate_assaults;
        }
        snapshot.units_remaining = self
            .app
            .world()
            .resource::<SettlementCrisisState>()
            .get(&self.player_id)
            .map(|crisis| {
                crisis
                    .assault_unit_ids
                    .len()
                    .saturating_sub(crisis.assault_defeated_unit_ids.len()) as i32
            })
            .unwrap_or(0);
        snapshot
    }

    pub fn settlement_crisis(&self) -> Option<SettlementCrisis> {
        self.app
            .world()
            .resource::<SettlementCrisisState>()
            .get(&self.player_id)
            .cloned()
    }

    /// Resolve the exact Monolith bound to a player's current hero instead of
    /// inferring sanctuary ownership from nearest-distance presentation data.
    pub fn bound_monolith_for_player(&mut self, player_id: i32) -> Option<MonolithView> {
        let world = self.app.world_mut();
        let bound_monolith_id = {
            let mut heroes =
                world.query_filtered::<(&PlayerId, &BoundMonolith), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(owner, _)| owner.0 == player_id)
                .map(|(_, bound)| bound.id)?
        };
        let entity = world
            .resource::<EntityObjMap>()
            .get_entity(bound_monolith_id)?;
        let position = *world.get::<Position>(entity)?;
        let monolith = world.get::<Monolith>(entity)?;
        Some(MonolithView {
            id: bound_monolith_id,
            pos: position,
            level: monolith.sanctuary_level,
        })
    }

    pub fn crisis_assault_units(&mut self) -> Vec<CrisisAssaultUnitView> {
        let world = self.app.world_mut();
        let mut query = world.query::<(
            &Id,
            &Template,
            &CrisisAssaultUnit,
            &Stats,
            &Position,
            &Viewshed,
            Option<&ThinkerBuilder>,
            Option<&StateDead>,
            Option<&crate::npc::VisibleTarget>,
            Option<&Target>,
            Option<&TaskTarget>,
        )>();
        let mut units = query
            .iter(world)
            .map(
                |(
                    id,
                    template,
                    assault,
                    stats,
                    pos,
                    viewshed,
                    thinker,
                    dead,
                    visible_target,
                    target,
                    task_target,
                )| CrisisAssaultUnitView {
                    obj_id: id.0,
                    template: template.0.clone(),
                    owner_player_id: assault.owner_player_id,
                    assault_id: assault.assault_id,
                    spawn_generation: assault.spawn_generation,
                    hp: stats.hp,
                    base_hp: stats.base_hp,
                    pos: *pos,
                    vision: viewshed.range,
                    has_thinker: thinker.is_some(),
                    visible_target: visible_target.map(|target| target.target),
                    target: target.map(|target| target.id),
                    task_target: task_target.map(|target| target.target),
                    dead: dead.is_some(),
                },
            )
            .collect::<Vec<_>>();
        units.sort_by_key(|unit| unit.obj_id);
        units
    }

    pub fn personal_crises_resolved(&self) -> i32 {
        self.app
            .world()
            .resource::<RunScoreState>()
            .get(&self.player_id)
            .map(|score| score.personal_crises_resolved)
            .unwrap_or(0)
    }

    pub fn intro_encounters(&self) -> Option<PlayerIntroEncounters> {
        self.app
            .world()
            .resource::<crate::game::IntroEncounterState>()
            .get(&self.player_id)
            .cloned()
    }

    pub fn game_tick(&self) -> i32 {
        self.app
            .world()
            .get_resource::<GameTick>()
            .map(|t| t.0)
            .unwrap_or(0)
    }

    pub fn ticks_pumped(&self) -> u64 {
        self.tick_count
    }

    // Read the primary player's slice of `World` as owned data. Keep this
    // wrapper for the single-player runner and existing tests.
    pub fn observe(&mut self) -> WorldView {
        let player_id = self.player_id;
        self.observe_for_player(player_id)
    }

    /// Read the slice of `World` a specific connected player's bot needs. Owned
    /// heroes, inventory, villagers, structures, sanctuary selection, and crisis
    /// phase are player-scoped; enemies and environmental/map observations stay
    /// shared exactly as they are in the live world.
    pub fn observe_for_player(&mut self, player_id: i32) -> WorldView {
        let pid = player_id;
        let game_tick = self.game_tick();
        let day = (game_tick / GAME_TICKS_PER_DAY) + 1;

        let world = self.app.world_mut();

        // Hero + its inventory (same entity).
        let (hero, inventory) = {
            let mut q = world.query_filtered::<(
                &Id,
                &PlayerId,
                &Position,
                &Stats,
                &HeroClass,
                &State,
                &Inventory,
                &Thirst,
                &Hunger,
                Option<&Tired>,
                Option<&TrueDeath>,
                &Viewshed,
            ), With<SubclassHero>>();
            match q.iter(world).find(|(_, p, ..)| p.0 == pid) {
                Some((
                    id,
                    _p,
                    pos,
                    stats,
                    hero_class,
                    state,
                    inv,
                    thirst,
                    hunger,
                    tired,
                    td,
                    viewshed,
                )) => (
                    Some(HeroView {
                        id: id.0,
                        pos: *pos,
                        hero_class: *hero_class,
                        hp: stats.hp,
                        base_hp: stats.base_hp,
                        stamina: stats.stamina,
                        mana: stats.mana,
                        vision: viewshed.range,
                        state: *state,
                        dead: *state == State::Dead,
                        true_death: td.is_some(),
                        thirst: thirst.thirst,
                        hunger: hunger.hunger,
                        tired: tired.map(|t| t.tired).unwrap_or(0.0),
                    }),
                    inv.items.iter().map(to_item_view).collect::<Vec<_>>(),
                ),
                None => (None, Vec::new()),
            }
        };

        // Living enemy NPCs.
        let enemies = {
            let mut q = world.query_filtered::<(
                &Id,
                &PlayerId,
                &Position,
                &State,
                Option<&CrisisAssaultUnit>,
            ), With<SubclassNPC>>();
            q.iter(world)
                .filter(|(_, _, _, state, _)| **state != State::Dead)
                .map(|(id, p, pos, _, assault)| UnitView {
                    id: id.0,
                    player_id: p.0,
                    pos: *pos,
                    crisis_owner_player_id: assault.map(|unit| unit.owner_player_id),
                })
                .collect::<Vec<_>>()
        };

        // The player's villagers, with whether they are idle (Order::None),
        // whether they hold a gather order / are actively gathering, and how much
        // food they are carrying (to diagnose the forage->haul->larder loop).
        let villagers = {
            let mut q = world.query_filtered::<(&Id, &PlayerId, &Position, &State, &Order, &Inventory), With<SubclassVillager>>();
            q.iter(world)
                .filter(|(_, p, _, state, _, _)| p.0 == pid && **state != State::Dead)
                .map(|(id, _p, pos, state, order, inv)| VillagerView {
                    id: id.0,
                    pos: *pos,
                    idle: *order == Order::None,
                    gathering_order: matches!(order, Order::Gather { .. }),
                    gathering_now: *state == State::Gathering,
                    food_carried: inv.get_total_weight_by_class(FOOD.to_string()),
                })
                .collect::<Vec<_>>()
        };

        // Points of interest (e.g. the Shipwreck — investigating it recruits the
        // first villager).
        let pois = {
            let mut q = world.query::<(&Id, &Position, &Subclass, &Template)>();
            q.iter(world)
                .filter(|(_, _, subclass, _)| **subclass == Subclass::Poi)
                .map(|(id, pos, _subclass, template)| PoiView {
                    id: id.0,
                    pos: *pos,
                    template: template.0.clone(),
                })
                .collect::<Vec<_>>()
        };

        // The travelling merchant (if any). Its `Transport.hauling` are the
        // villagers available to hire; `sail_state` says whether it's docked.
        let merchant = {
            let mut q = world.query::<(&Id, &Position, &Merchant, &Transport)>();
            q.iter(world)
                .next()
                .map(|(id, pos, merchant, transport)| MerchantView {
                    id: id.0,
                    pos: *pos,
                    at_landing: merchant.sail_state == MerchantSailState::AtLanding,
                    hireable: transport.hauling.clone(),
                })
        };

        // The Monolith nearest the hero — the sanctuary the bot empowers.
        let hero_pos_opt = hero.as_ref().map(|h| h.pos);
        let monolith = {
            let mut q = world.query::<(&Id, &Position, &Monolith)>();
            let mut best: Option<MonolithView> = None;
            let mut best_d = u32::MAX;
            for (id, pos, m) in q.iter(world) {
                let d = hero_pos_opt
                    .map(|hp| Map::distance((hp.x, hp.y), (pos.x, pos.y)))
                    .unwrap_or(0);
                if d < best_d {
                    best_d = d;
                    best = Some(MonolithView {
                        id: id.0,
                        pos: *pos,
                        level: m.sanctuary_level,
                    });
                }
            }
            best
        };

        // Enemy corpses still holding a Soulshard, ready to loot for upgrades.
        let corpses = {
            let mut q = world.query::<(&Id, &PlayerId, &Position, &State, &Inventory)>();
            q.iter(world)
                .filter(|(_, p, _, state, _)| p.0 != pid && **state == State::Dead)
                .filter_map(|(id, _p, pos, _state, inv)| {
                    inv.items
                        .iter()
                        .find(|i| i.class == "Soulshard")
                        .map(|item| CorpseView {
                            id: id.0,
                            pos: *pos,
                            soulshard_item: item.id,
                        })
                })
                .collect::<Vec<_>>()
        };

        // The player's structures (+ inventories so the bot can pull build
        // resources from the Burrow and check foundation contents).
        let structures = {
            let mut q = world.query_filtered::<(&Id, &PlayerId, &Position, &Subclass, &State, &Inventory), With<ClassStructure>>();
            q.iter(world)
                .filter(|(_, p, _, _, _, _)| p.0 == pid)
                .map(|(id, _p, pos, subclass, state, inv)| StructureView {
                    id: id.0,
                    pos: *pos,
                    subclass: subclass.to_string(),
                    founded: *state == State::Founded,
                    building: *state == State::Building || *state == State::Progressing,
                    built: !matches!(
                        *state,
                        State::Founded | State::Building | State::Progressing | State::Stalled
                    ),
                    inventory: inv.items.iter().map(to_item_view).collect(),
                })
                .collect::<Vec<_>>()
        };

        // Tiles occupied by objects that production movement treats as blocking.
        // Dead, founded, progressing, stalled, and hiding objects must not become
        // permanent synthetic obstacles in a bot snapshot.
        let occupied = {
            let mut q = world.query::<(&Position, &State)>();
            q.iter(world)
                .filter(|(_, state)| state.is_blocking())
                .map(|(position, _)| (position.x, position.y))
                .collect::<HashSet<_>>()
        };

        // Resource node tiles (the `Resources` map is keyed by Position). Track
        // both general reveal state and spring-water specifically (the bot
        // prospects a tile to reveal a spring, then refills waterskins there).
        let resource_tiles = world
            .resource::<Resources>()
            .iter()
            .map(|(pos, res_on_tile)| {
                let (has_spring, spring_revealed) = res_on_tile
                    .values()
                    .filter(|r| r.res_type == SPRING_WATER)
                    .fold((false, false), |acc, r| (true, acc.1 || r.reveal));
                let (has_game, game_revealed) = res_on_tile
                    .values()
                    .filter(|r| r.res_type == GAME_ANIMAL)
                    .fold((false, false), |acc, r| (true, acc.1 || r.reveal));
                let (has_plant, plant_revealed) = res_on_tile
                    .values()
                    .filter(|r| r.res_type == PLANT)
                    .fold((false, false), |acc, r| (true, acc.1 || r.reveal));
                ResTileView {
                    pos: *pos,
                    revealed: res_on_tile.values().any(|r| r.reveal),
                    has_spring,
                    spring_revealed,
                    has_game,
                    game_revealed,
                    has_plant,
                    plant_revealed,
                }
            })
            .collect::<Vec<_>>();

        WorldView {
            hero,
            inventory,
            enemies,
            villagers,
            pois,
            merchant,
            monolith,
            corpses,
            structures,
            resource_tiles,
            occupied,
            game_tick,
            day,
            crisis_phase: world
                .resource::<SettlementCrisisState>()
                .get(&pid)
                .map(|crisis| crisis.phase),
        }
    }

    pub fn map(&self) -> &Map {
        self.app.world().resource::<Map>()
    }

    pub fn is_land_passable(&self, position: Position) -> bool {
        Map::is_passable_by_obj(position.x, position.y, true, false, false, self.map())
    }

    // Run is over when capped, the hero permadied (TrueDeath or despawned), or a
    // victory was achieved.
    pub fn is_over(&mut self) -> bool {
        if self.game_tick() - self.spawn_tick >= self.max_ticks {
            return true;
        }

        let pid = self.player_id;
        let world = self.app.world_mut();

        // Victory?
        if let Some(v) = world.resource::<VictoryState>().get(&pid) {
            if v.prosperity || v.conquest || v.rescue_progress > 0 {
                return true;
            }
        }

        // Hero permadead or gone?
        let mut q = world.query_filtered::<(&PlayerId, Option<&TrueDeath>), With<SubclassHero>>();
        match q.iter(world).find(|(p, _)| p.0 == pid) {
            Some((_, td)) => td.is_some(),
            None => true,
        }
    }

    pub fn metrics(&mut self) -> RunMetrics {
        let balance_interval = self
            .app
            .world()
            .resource::<CrisisBalanceTelemetryConfig>()
            .sample_interval_ticks;
        if balance_interval.is_some() {
            self.app
                .world_mut()
                .resource_mut::<CrisisBalanceTelemetryConfig>()
                .sample_interval_ticks = Some(1);
            self.app
                .world_mut()
                .run_system_once(crisis_balance_snapshot_system)
                .expect("final crisis balance snapshot");
            self.app
                .world_mut()
                .resource_mut::<CrisisBalanceTelemetryConfig>()
                .sample_interval_ticks = balance_interval;
        }
        let crisis_telemetry = self.crisis_telemetry();
        let crisis_balance = self.crisis_balance_telemetry();
        let crisis_warning_signs_to_launch_global_ticks =
            crisis_balance.warnings.signs_to_launch_global_ticks();
        let crisis_warning_signs_to_launch_online_ticks =
            crisis_balance.warnings.signs_to_launch_online_ticks();
        let safe_logout_telemetry = self.safe_logout_telemetry();
        let safe_logout_rejection_reasons = safe_logout_telemetry
            .rejection_reasons
            .iter()
            .map(|(reason, count)| {
                (
                    safe_logout_rejection_reason_name(*reason).to_string(),
                    *count,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let safe_logout_cancellation_reasons = safe_logout_telemetry
            .cancellation_reasons
            .iter()
            .map(|(reason, count)| (safe_logout_cancel_reason_name(*reason).to_string(), *count))
            .collect::<BTreeMap<_, _>>();
        let safe_logout_invariant_reasons = safe_logout_telemetry
            .invariant_reasons
            .iter()
            .map(|(reason, count)| (reason.clone(), *count))
            .collect::<BTreeMap<_, _>>();
        let pid = self.player_id;
        let current_tick = self.game_tick();
        let world = self.app.world_mut();

        // Hero end-state (owned primitives so the borrow ends before the next query).
        let (
            final_hp,
            final_skill_total,
            final_inventory_count,
            hero_true_death,
            hero_present,
            killer,
        ) = {
            let mut q = world.query_filtered::<(
                &PlayerId,
                &Stats,
                &Skills,
                &Inventory,
                Option<&TrueDeath>,
                Option<&StateDead>,
            ), With<SubclassHero>>();
            match q.iter(world).find(|(p, ..)| p.0 == pid) {
                Some((_p, stats, skills, inv, td, dead)) => (
                    stats.hp,
                    skills.get_total_xp(),
                    inv.items.len() as i32,
                    td.is_some(),
                    true,
                    dead.map(|d| d.killer.clone()).unwrap_or_default(),
                ),
                None => (0, 0, 0, true, false, String::new()),
            }
        };

        let structures_built = {
            let mut q = world.query_filtered::<&PlayerId, With<ClassStructure>>();
            q.iter(world).filter(|p| p.0 == pid).count() as i32
        };

        let run: PlayerRunScore = world
            .resource::<RunScoreState>()
            .get(&pid)
            .cloned()
            .unwrap_or_default();
        let num_deaths = world
            .resource::<PlayerStats>()
            .get(&pid)
            .map(|s| s.num_deaths)
            .unwrap_or(0);
        let objectives: PlayerObjectives = world
            .resource::<Objectives>()
            .get(&pid)
            .cloned()
            .unwrap_or_default();
        let victory: PlayerVictory = world
            .resource::<VictoryState>()
            .get(&pid)
            .cloned()
            .unwrap_or_default();
        let final_crisis = world.resource::<SettlementCrisisState>().get(&pid).cloned();

        let start_tick = if run.start_tick != 0 {
            run.start_tick
        } else {
            self.spawn_tick
        };
        let ticks = (current_tick - start_tick).max(0);
        let days_survived = ticks / GAME_TICKS_PER_DAY;

        let outcome = if !hero_present || hero_true_death {
            "TrueDeath".to_string()
        } else if victory.prosperity {
            "Victory:Prosperity".to_string()
        } else if victory.conquest {
            "Victory:Conquest".to_string()
        } else if victory.rescue_progress > 0 {
            "Victory:Rescue".to_string()
        } else {
            "MaxTicks".to_string()
        };

        RunMetrics {
            run_index: 0,
            outcome,
            killer,
            ticks,
            days_survived,
            waves_survived: run.waves_survived,
            enemies_killed: run.enemies_killed,
            elites_killed: run.elites_killed,
            captains_killed: run.captains_killed,
            legendary_kills: run.legendary_kills,
            hideouts_cleared: run.hideouts_cleared,
            repairs: run.repairs,
            highest_pressure_level: run.highest_pressure_level,
            num_deaths,
            obj_scavenge_shipwreck: objectives.scavenge_shipwreck,
            obj_build_campfire: objectives.build_campfire,
            obj_win_first_fight: objectives.win_first_fight,
            obj_build_3_structures: objectives.build_3_structures,
            obj_recruit_villager: objectives.recruit_villager,
            obj_explore_poi: objectives.explore_poi,
            obj_choose_expansion: objectives.choose_expansion,
            obj_survive_5_nights: objectives.survive_5_nights,
            obj_find_legendary_hideout: objectives.find_legendary_hideout,
            obj_defeat_ashen_warlord: objectives.defeat_ashen_warlord,
            victory_rescue_progress: victory.rescue_progress,
            victory_prosperity: victory.prosperity,
            victory_conquest: victory.conquest,
            final_hp,
            final_skill_total,
            final_inventory_count,
            structures_built,
            crisis_highest_phase: if crisis_telemetry.highest_phase.is_empty() {
                "none".to_string()
            } else {
                crisis_telemetry.highest_phase.clone()
            },
            crisis_final_phase: final_crisis
                .as_ref()
                .map(|crisis| crisis_phase_name(crisis.phase).to_string())
                .unwrap_or_else(|| "none".to_string()),
            crisis_final_pressure: final_crisis
                .as_ref()
                .map(|crisis| crisis.pressure)
                .unwrap_or(0),
            crisis_signs_tick: crisis_telemetry.signs_tick,
            crisis_pressure_tick: crisis_telemetry.pressure_tick,
            crisis_preparing_tick: crisis_telemetry.preparing_tick,
            crisis_assault_ready_tick: crisis_telemetry.assault_ready_tick,
            crisis_assault_active_tick: crisis_telemetry.assault_active_tick,
            crisis_resolved_tick: crisis_telemetry.resolved_tick,
            crisis_assaults_launched: crisis_telemetry.assaults_launched,
            crisis_assaults_resolved: crisis_telemetry.assaults_resolved,
            crisis_units_remaining: crisis_telemetry.units_remaining,
            crisis_status_packets_sent: crisis_telemetry.status_packets_sent,
            crisis_login_snapshots_sent: crisis_telemetry.login_snapshots_sent,
            crisis_duplicate_assaults: crisis_telemetry.duplicate_assaults,
            personal_crisis_automatic_dusk_hordes: run.waves_survived,
            crisis_invariants_ok: crisis_telemetry.duplicate_assaults == 0
                && crisis_telemetry.assaults_resolved <= crisis_telemetry.assaults_launched,
            safe_logout_scenario_mode: "standard".to_string(),
            safe_logout_requests: safe_logout_telemetry.requests,
            safe_logout_accepted: safe_logout_telemetry.accepted,
            safe_logout_rejected: safe_logout_telemetry.rejected,
            safe_logout_cancelled: safe_logout_telemetry.cancelled,
            safe_logout_completed: safe_logout_telemetry.completed,
            safe_logout_protected_sessions_started: safe_logout_telemetry
                .protected_sessions_started,
            safe_logout_resumed: safe_logout_telemetry.resumed,
            safe_logout_protected_ticks_total: safe_logout_telemetry.protected_ticks_total,
            safe_logout_ordinary_disconnects: safe_logout_telemetry.ordinary_disconnects,
            safe_logout_active_assault_disconnects: safe_logout_telemetry
                .active_assault_disconnects,
            safe_logout_status_packets_sent: safe_logout_telemetry.status_packets_sent,
            safe_logout_status_packets_duplicate_suppressed: safe_logout_telemetry
                .status_packets_duplicate_suppressed,
            safe_logout_protected_input_rejections: safe_logout_telemetry
                .protected_input_rejections,
            safe_logout_protected_damage_blocks: safe_logout_telemetry.protected_damage_blocks,
            safe_logout_protected_target_rejections: safe_logout_telemetry
                .protected_target_rejections,
            safe_logout_queued_events_discarded: safe_logout_telemetry.queued_events_discarded,
            safe_logout_invariant_recoveries: safe_logout_telemetry.invariant_recoveries,
            safe_logout_run_key_mismatches: safe_logout_telemetry.run_key_mismatches,
            safe_logout_timer_rebases: safe_logout_telemetry.timer_rebases,
            safe_logout_stale_connection_events_rejected: safe_logout_telemetry
                .stale_connection_events_rejected,
            safe_logout_rejection_reasons,
            safe_logout_cancellation_reasons,
            safe_logout_invariant_reasons,
            safe_logout_invariants_ok: safe_logout_telemetry.invariant_recoveries == 0
                && safe_logout_telemetry.run_key_mismatches == 0,
            crisis_balance_scenario: "standard".to_string(),
            crisis_balance_hero_class: crisis_balance
                .preparation_snapshots
                .resolution_or_end
                .as_ref()
                .map(|snapshot| snapshot.hero_class.clone())
                .filter(|class| !class.is_empty())
                .unwrap_or_else(|| "unknown".to_string()),
            crisis_balance_run_id: String::new(),
            crisis_balance_tick_cap: self.max_ticks,
            crisis_balance_tick_cap_reached: current_tick.saturating_sub(self.spawn_tick)
                >= self.max_ticks,
            crisis_balance_progression_fixture: false,
            crisis_balance_config: crate::game::goblin_crisis_balance_config_snapshot(),
            crisis_balance,
            crisis_warning_signs_to_launch_global_ticks,
            crisis_warning_signs_to_launch_online_ticks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GOBLIN_ASSAULT_COMPOSITION;
    use crate::network::{CrisisStatusSnapshot, SafeLogoutStatusSnapshot};

    fn crisis_statuses(packets: Vec<ResponsePacket>) -> Vec<CrisisStatusSnapshot> {
        packets
            .into_iter()
            .filter_map(|packet| match packet {
                ResponsePacket::CrisisStatus { status } => Some(status),
                _ => None,
            })
            .collect()
    }

    fn safe_logout_statuses(packets: Vec<ResponsePacket>) -> Vec<SafeLogoutStatusSnapshot> {
        packets
            .into_iter()
            .filter_map(|packet| match packet {
                ResponsePacket::SafeLogoutStatus { status } => Some(status),
                _ => None,
            })
            .collect()
    }

    fn crisis_phase_sequence(statuses: &[CrisisStatusSnapshot]) -> Vec<String> {
        let mut phases = Vec::new();
        for phase in statuses
            .iter()
            .filter(|status| status.exists)
            .filter_map(|status| status.phase.clone())
        {
            if phases.last() != Some(&phase) {
                phases.push(phase);
            }
        }
        phases
    }

    fn prepare_full_crisis_progression_facts(game: &mut HeadlessGame) {
        use crate::game::{InitialEncounterState, PlayerIntroState};

        let player_id = game.player_id();
        game.set_sanctuary_at_base(5);
        let world = game.app.world_mut();
        world
            .resource_mut::<PlayerIntroState>()
            .get_mut(&player_id)
            .expect("player intro state")
            .danger_unlocked = true;
        {
            let mut objectives = world.resource_mut::<Objectives>();
            let objectives = objectives.entry(player_id).or_default();
            objectives.explore_poi = true;
            objectives.choose_expansion = true;
        }
        world
            .resource_mut::<InitialEncounterState>()
            .remove(&player_id);
        for _ in 0..3 {
            world.spawn((PlayerId(player_id), State::None, ClassStructure));
        }
        world.spawn((PlayerId(player_id), State::None, SubclassVillager));
    }

    fn prepare_for_scheduled_dusk(game: &mut HeadlessGame) {
        prepare_for_scheduled_dusk_on_day(game, 3);
    }

    fn prepare_for_scheduled_dusk_on_day(game: &mut HeadlessGame, day_offset: i32) {
        use crate::constants::DUSK;
        use crate::game::PlayerIntroState;

        let dusk = DUSK + (GAME_TICKS_PER_DAY * day_offset);
        let player_id = game.player_id();
        let world = game.app.world_mut();
        {
            let mut intro_state = world.resource_mut::<PlayerIntroState>();
            let intro = intro_state
                .get_mut(&player_id)
                .expect("headless player intro state");
            intro.start_tick = dusk - 4_801;
            intro.danger_unlocked = true;
        }
        world.resource_mut::<GameTick>().0 = dusk - 2;
    }

    fn cross_scheduled_dusk(game: &mut HeadlessGame) -> i32 {
        game.tick(5);
        game.metrics().waves_survived
    }

    fn run_intro_check_at_or_after(game: &mut HeadlessGame, due_tick: i32) {
        let check_tick = ((due_tick + 9) / 10) * 10;
        game.app.world_mut().resource_mut::<GameTick>().0 = check_tick - 2;
        game.tick(15);
    }

    fn mark_obj_ids_dead(game: &mut HeadlessGame, obj_ids: &[i32], dead_at: i32) {
        let entities = {
            let world = game.app.world_mut();
            let mut query = world.query::<(Entity, &Id)>();
            query
                .iter(world)
                .filter(|(_, id)| obj_ids.contains(&id.0))
                .map(|(entity, _)| entity)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            entities.len(),
            obj_ids.len(),
            "every scripted encounter object should exist before being defeated"
        );
        for entity in entities {
            game.app.world_mut().entity_mut(entity).insert((
                State::Dead,
                StateDead {
                    dead_at,
                    killer: "Headless checkpoint test".to_string(),
                },
            ));
        }
    }

    fn next_preferred_assault_tick(current_tick: i32) -> i32 {
        use crate::constants::DUSK;
        use crate::game::ASSAULT_READY_GRACE_TICKS;

        let mut dusk = current_tick.div_euclid(GAME_TICKS_PER_DAY) * GAME_TICKS_PER_DAY + DUSK;
        while dusk - ASSAULT_READY_GRACE_TICKS <= current_tick {
            dusk += GAME_TICKS_PER_DAY;
        }
        dusk
    }

    fn set_personal_assault_ready(game: &mut HeadlessGame) -> i32 {
        use crate::game::{
            CrisisKind, CrisisPhase, InitialEncounterState, PlayerIntroState,
            ASSAULT_READY_GRACE_TICKS,
        };

        let player_id = game.player_id();
        let preferred_tick = next_preferred_assault_tick(game.game_tick());
        let ready_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;
        let world = game.app.world_mut();
        world.resource_mut::<GameTick>().0 = ready_tick;
        world
            .resource_mut::<PlayerIntroState>()
            .get_mut(&player_id)
            .expect("player intro state")
            .danger_unlocked = true;
        world
            .resource_mut::<InitialEncounterState>()
            .remove(&player_id);
        world.resource_mut::<SettlementCrisisState>().insert(
            player_id,
            SettlementCrisis {
                kind: CrisisKind::Goblin,
                phase: CrisisPhase::AssaultReady,
                pressure: 100,
                phase_started_tick: ready_tick,
                online_active_ticks: 10_000,
                phase_online_ticks: 0,
                warning_active: true,
                last_evaluated_tick: ready_tick,
                ..SettlementCrisis::default()
            },
        );
        preferred_tick
    }

    fn advance_ready_clock_to_launch(game: &mut HeadlessGame, preferred_tick: i32) {
        use crate::game::CrisisPhase;

        game.app.world_mut().resource_mut::<GameTick>().0 = preferred_tick - 2;
        game.tick(1);
        let before = game.settlement_crisis().expect("ready crisis");
        assert_eq!(before.phase, CrisisPhase::AssaultReady);
        assert!(game.crisis_assault_units().is_empty());

        game.tick(1);
        assert_eq!(
            game.settlement_crisis().expect("launched crisis").phase,
            CrisisPhase::AssaultActive
        );
    }

    fn resolve_goblin_normally_for_undead_smoke(game: &mut HeadlessGame) -> u64 {
        use crate::constants::TICKS_PER_SEC;
        use crate::game::{
            CrisisKind, CrisisPhase, SettlementCrisisState, ASSAULT_READY_GRACE_TICKS,
            GOBLIN_PREPARING_MIN_ONLINE_TICKS, GOBLIN_PRESSURE_MIN_ONLINE_TICKS,
            GOBLIN_SIGNS_MIN_ONLINE_TICKS,
        };

        prepare_full_crisis_progression_facts(game);
        game.tick(1);
        let signs = game.settlement_crisis().expect("Goblin Signs crisis");
        assert_eq!(signs.kind, CrisisKind::Goblin);
        assert_eq!(signs.phase, CrisisPhase::Signs);

        for (minimum_ticks, expected_phase) in [
            (GOBLIN_SIGNS_MIN_ONLINE_TICKS, CrisisPhase::Pressure),
            (GOBLIN_PRESSURE_MIN_ONLINE_TICKS, CrisisPhase::Preparing),
            (GOBLIN_PREPARING_MIN_ONLINE_TICKS, CrisisPhase::AssaultReady),
        ] {
            game.app.world_mut().resource_mut::<GameTick>().0 += minimum_ticks - 1;
            game.tick(1);
            assert_eq!(
                game.settlement_crisis()
                    .expect("progressing Goblin crisis")
                    .phase,
                expected_phase
            );
        }

        let preferred_tick = next_preferred_assault_tick(game.game_tick());
        {
            let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
            let crisis = crises
                .get_mut(&game.player_id)
                .expect("Goblin Ready crisis");
            crisis.phase_online_ticks = 0;
            crisis.last_evaluated_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;
        }
        advance_ready_clock_to_launch(game, preferred_tick);
        let active = game.settlement_crisis().expect("active Goblin assault");
        let assault_id = active.assault_id.expect("Goblin assault identity");
        let units = game.crisis_assault_units();
        assert_eq!(units.len(), GOBLIN_ASSAULT_COMPOSITION.len());
        for unit in units {
            kill_assault_unit_through_normal_combat(game, unit.obj_id);
        }
        game.tick(2);
        let resolved = game.settlement_crisis().expect("resolved Goblin crisis");
        assert_eq!(resolved.kind, CrisisKind::Goblin);
        assert_eq!(resolved.phase, CrisisPhase::Resolved);
        assert!(resolved.resolution_recorded);
        assert_eq!(game.personal_crises_resolved(), 1);

        // Keep the helper bounded and make its intended online pacing obvious.
        assert!(game.game_tick() < preferred_tick + (60 * TICKS_PER_SEC));
        assault_id
    }

    fn launch_undead_after_completed_goblin(game: &mut HeadlessGame) -> (u64, u32, Vec<i32>) {
        use crate::game::{
            CrisisKind, CrisisPhase, PersonalCrisisHistory, SettlementCrisisState,
            ASSAULT_READY_GRACE_TICKS, NEXT_PERSONAL_CRISIS_DELAY_TICKS,
            UNDEAD_PREPARING_MIN_ONLINE_TICKS, UNDEAD_PRESSURE_MIN_ONLINE_TICKS,
            UNDEAD_SIGNS_MIN_ONLINE_TICKS,
        };

        let player_id = game.player_id();
        resolve_goblin_normally_for_undead_smoke(game);
        let resolved_ticks = game
            .settlement_crisis()
            .expect("resolved Goblin delay holder")
            .phase_online_ticks;
        assert!(resolved_ticks < NEXT_PERSONAL_CRISIS_DELAY_TICKS);
        game.app.world_mut().resource_mut::<GameTick>().0 +=
            NEXT_PERSONAL_CRISIS_DELAY_TICKS - resolved_ticks - 1;
        game.tick(1);

        let dormant = game
            .settlement_crisis()
            .expect("Undead crisis after the completed Goblin delay");
        assert_eq!(dormant.kind, CrisisKind::Undead);
        assert_eq!(dormant.phase, CrisisPhase::Dormant);
        assert!(game
            .world()
            .resource::<PersonalCrisisHistory>()
            .by_player
            .get(&player_id)
            .is_some_and(|history| history.completed.contains(&CrisisKind::Goblin)));

        game.tick(1);
        assert_eq!(game.settlement_crisis().unwrap().phase, CrisisPhase::Signs);
        for (minimum_ticks, expected_phase) in [
            (UNDEAD_SIGNS_MIN_ONLINE_TICKS, CrisisPhase::Pressure),
            (UNDEAD_PRESSURE_MIN_ONLINE_TICKS, CrisisPhase::Preparing),
            (UNDEAD_PREPARING_MIN_ONLINE_TICKS, CrisisPhase::AssaultReady),
        ] {
            game.app.world_mut().resource_mut::<GameTick>().0 += minimum_ticks - 1;
            game.tick(1);
            assert_eq!(
                game.settlement_crisis()
                    .expect("progressing Undead crisis")
                    .phase,
                expected_phase
            );
        }

        let preferred_tick = next_preferred_assault_tick(game.game_tick());
        {
            let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
            let crisis = crises.get_mut(&player_id).expect("Undead Ready crisis");
            crisis.phase_online_ticks = 0;
            crisis.last_evaluated_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;
        }
        advance_ready_clock_to_launch(game, preferred_tick);

        let active = game.settlement_crisis().expect("active Undead assault");
        assert_eq!(active.kind, CrisisKind::Undead);
        assert_eq!(active.phase, CrisisPhase::AssaultActive);
        let assault_id = active.assault_id.expect("Undead assault identity");
        let generation = active.assault_spawn_generation;
        assert_eq!(generation, 1);
        assert_eq!(active.assault_unit_ids.len(), 6);
        (assault_id, generation, active.assault_unit_ids)
    }

    fn assert_fixed_undead_assault(game: &mut HeadlessGame, assault_id: u64, generation: u32) {
        use crate::game::{Home, Minions};
        use crate::ids::EntityObjMap;
        use crate::player_setup::RunSpawnedObjs;

        let player_id = game.player_id();
        let units = game.crisis_assault_units();
        assert_eq!(units.len(), 6);
        let mut templates = units
            .iter()
            .map(|unit| unit.template.as_str())
            .collect::<Vec<_>>();
        templates.sort_unstable();
        assert_eq!(
            templates,
            [
                "Necromancer",
                "Skeleton",
                "Skeleton",
                "Zombie",
                "Zombie",
                "Zombie",
            ]
        );
        assert!(units.iter().all(|unit| {
            unit.owner_player_id == player_id
                && unit.assault_id == assault_id
                && unit.spawn_generation == generation
                && unit.vision == 14
        }));
        assert!(units
            .iter()
            .filter(|unit| unit.template == "Necromancer")
            .all(|unit| unit.has_thinker));

        let necromancer_id = units
            .iter()
            .find(|unit| unit.template == "Necromancer")
            .map(|unit| unit.obj_id)
            .expect("fixed-composition Necromancer");
        let necromancer_entity = game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(necromancer_id)
            .expect("Necromancer entity");
        let home = game
            .world()
            .get::<Home>(necromancer_entity)
            .expect("active Necromancer Home");
        let position = game
            .world()
            .get::<Position>(necromancer_entity)
            .expect("Necromancer position");
        assert_eq!(home.pos, *position);
        assert!(game
            .world()
            .get::<Minions>(necromancer_entity)
            .expect("active Necromancer Minions")
            .ids
            .is_empty());
        let run_ids = game
            .world()
            .resource::<RunSpawnedObjs>()
            .get(&player_id)
            .expect("current-run spawned IDs");
        assert!(units.iter().all(|unit| run_ids.contains(&unit.obj_id)));
    }

    fn trigger_same_assault_raise_dead(game: &mut HeadlessGame) -> (i32, i32, i32) {
        use crate::event::{EventExecuting, EventExecutingState};
        use crate::game::Minions;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;
        use crate::player_setup::RunSpawnedObjs;

        let player_id = game.player_id();
        let active = game.settlement_crisis().expect("active Undead assault");
        let initial_ids = active
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let units = game.crisis_assault_units();
        let necromancer_id = units
            .iter()
            .find(|unit| unit.template == "Necromancer" && !unit.dead)
            .map(|unit| unit.obj_id)
            .expect("live Necromancer");
        let corpse_id = units
            .iter()
            .find(|unit| unit.template == "Zombie" && !unit.dead)
            .map(|unit| unit.obj_id)
            .expect("same-assault Zombie for Raise Dead");

        {
            let world = game.app.world_mut();
            let mut heroes = world.query_filtered::<(&PlayerId, &mut Stats), With<SubclassHero>>();
            let (_, mut stats) = heroes
                .iter_mut(world)
                .find(|(owner, _)| owner.0 == player_id)
                .expect("owner hero");
            stats.hp = 100_000;
            stats.base_hp = 100_000;
        }
        kill_assault_unit_through_normal_combat(game, corpse_id);

        let (corpse_entity, necromancer_entity, corpse_pos) = {
            let world = game.app.world();
            let entity_map = world.resource::<EntityObjMap>();
            let corpse_entity = entity_map
                .get_entity(corpse_id)
                .expect("normal-combat Zombie corpse");
            let necromancer_entity = entity_map
                .get_entity(necromancer_id)
                .expect("active Necromancer");
            let corpse_pos = *world
                .get::<Position>(corpse_entity)
                .expect("same-assault corpse position");
            (corpse_entity, necromancer_entity, corpse_pos)
        };
        {
            let world = game.app.world_mut();
            *world
                .get_mut::<Position>(necromancer_entity)
                .expect("Necromancer position") = corpse_pos;
            *world
                .get_mut::<State>(necromancer_entity)
                .expect("Necromancer state") = State::None;
            world
                .get_mut::<VisibleTarget>(necromancer_entity)
                .expect("Necromancer visible target")
                .target = crate::constants::NO_TARGET;
            world
                .get_mut::<TaskTarget>(necromancer_entity)
                .expect("Necromancer corpse target")
                .target = crate::constants::NO_TARGET;
            world.entity_mut(necromancer_entity).remove::<Target>();
            world
                .get_mut::<EventExecuting>(necromancer_entity)
                .expect("Necromancer event state")
                .state = EventExecutingState::None;
            assert!(world.get::<StateDead>(corpse_entity).is_some());
        }

        let next_scorer_tick = ((game.game_tick() / 10) + 1) * 10;
        game.app.world_mut().resource_mut::<GameTick>().0 = next_scorer_tick - 1;
        let scheduled_raise = (0..60)
            .find_map(|_| {
                game.tick(1);
                game.world()
                    .resource::<MapEvents>()
                    .values()
                    .find(|event| {
                        event.obj_id == necromancer_id
                            && matches!(
                                event.event_type,
                                VisibleEvent::SpellRaiseDeadEvent {
                                    corpse_id: scheduled_corpse_id
                                } if scheduled_corpse_id == corpse_id
                            )
                    })
                    .cloned()
            })
            .expect("existing Necromancer AI must schedule Raise Dead within bound");
        let duplicate_event_id = Uuid::from_u128(7_007_007_007);
        let mut duplicate_raise = scheduled_raise.clone();
        duplicate_raise.event_id = duplicate_event_id;
        game.app
            .world_mut()
            .resource_mut::<MapEvents>()
            .insert(duplicate_event_id, duplicate_raise);

        let next_obj_id_before = game.world().resource::<Ids>().obj;
        let run_spawned_len_before = game
            .world()
            .resource::<RunSpawnedObjs>()
            .get(&player_id)
            .map(Vec::len)
            .unwrap_or_default();
        let minions_len_before = game
            .world()
            .get::<Minions>(necromancer_entity)
            .expect("Necromancer Minions before duplicate Raise Dead")
            .ids
            .len();
        let raised_id = (0..180).find_map(|_| {
            game.tick(1);
            game.crisis_assault_units()
                .into_iter()
                .find(|unit| {
                    !initial_ids.contains(&unit.obj_id) && unit.template == "Zombie" && !unit.dead
                })
                .map(|unit| unit.obj_id)
        });
        let raised_id = raised_id.expect("existing Necromancer AI must Raise Dead within bound");
        assert_eq!(
            game.world().resource::<Ids>().obj,
            next_obj_id_before + 1,
            "duplicate same-corpse events must allocate exactly one raised identity"
        );
        let remaining_events = game.world().resource::<MapEvents>();
        assert!(remaining_events.get(&scheduled_raise.event_id).is_none());
        assert!(remaining_events.get(&duplicate_event_id).is_none());

        let current = game.settlement_crisis().expect("active extended roster");
        assert!(current.assault_unit_ids.contains(&raised_id));
        assert!(current.assault_defeated_unit_ids.contains(&corpse_id));
        assert_eq!(current.assault_unit_ids.len(), initial_ids.len() + 1);
        assert_eq!(
            current
                .assault_unit_ids
                .iter()
                .filter(|id| **id == raised_id)
                .count(),
            1,
            "duplicate same-corpse events must append one roster entry"
        );
        let raised = game
            .crisis_assault_units()
            .into_iter()
            .find(|unit| unit.obj_id == raised_id)
            .expect("raised attributed Zombie");
        assert_eq!(raised.owner_player_id, player_id);
        assert_eq!(raised.assault_id, current.assault_id.unwrap());
        assert_eq!(raised.spawn_generation, current.assault_spawn_generation);
        assert!(!raised.dead);
        let run_spawned = game
            .world()
            .resource::<RunSpawnedObjs>()
            .get(&player_id)
            .expect("current-run spawned IDs after Raise Dead");
        assert_eq!(run_spawned.len(), run_spawned_len_before + 1);
        assert_eq!(
            run_spawned.iter().filter(|id| **id == raised_id).count(),
            1,
            "duplicate same-corpse events must append one RunSpawned entry"
        );
        assert!(game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(corpse_id)
            .is_none());

        let necromancer_entity = game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(necromancer_id)
            .expect("Necromancer remains after raising");
        let minions = game
            .world()
            .get::<Minions>(necromancer_entity)
            .expect("Necromancer Minions");
        assert_eq!(
            minions.ids.iter().filter(|id| **id == raised_id).count(),
            1,
            "raised unit receives one new identity and one Minions entry"
        );
        assert_eq!(minions.ids.len(), minions_len_before + 1);

        (corpse_id, raised_id, necromancer_id)
    }

    fn defeat_remaining_undead_after_necromancer(game: &mut HeadlessGame, necromancer_id: i32) {
        if game
            .crisis_assault_units()
            .iter()
            .any(|unit| unit.obj_id == necromancer_id && !unit.dead)
        {
            kill_assault_unit_through_normal_combat(game, necromancer_id);
        }
        let remaining_ids = game
            .crisis_assault_units()
            .into_iter()
            .filter(|unit| !unit.dead)
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        for unit_id in remaining_ids {
            kill_assault_unit_through_normal_combat(game, unit_id);
        }
        game.tick(2);
    }

    fn kill_assault_unit_through_normal_combat(
        game: &mut HeadlessGame,
        target_id: i32,
    ) -> Vec<(String, i32)> {
        let attacker_player_id = game.player_id();
        kill_assault_unit_through_normal_combat_as(game, attacker_player_id, target_id)
    }

    fn kill_assault_unit_through_normal_combat_as(
        game: &mut HeadlessGame,
        attacker_player_id: i32,
        target_id: i32,
    ) -> Vec<(String, i32)> {
        use crate::constants::ATTACK_COOLDOWN_TICKS;
        use crate::ids::EntityObjMap;

        let (hero_entity, hero_id, hero_pos, target_entity, loot_before) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(Entity, &Id, &PlayerId, &Position), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_pos) = hero_query
                .iter(world)
                .find(|(_, _, owner, _)| owner.0 == attacker_player_id)
                .map(|(entity, id, _, pos)| (entity, id.0, *pos))
                .expect("headless hero");
            let target_entity = world
                .resource::<EntityObjMap>()
                .get_entity(target_id)
                .expect("assault target entity");
            let loot_before = world
                .get::<Inventory>(target_entity)
                .expect("assault inventory")
                .items
                .iter()
                .map(|item| (item.name.clone(), item.quantity))
                .collect::<Vec<_>>();
            (hero_entity, hero_id, hero_pos, target_entity, loot_before)
        };

        {
            let world = game.app.world_mut();
            *world
                .get_mut::<Position>(target_entity)
                .expect("target position") = hero_pos;
            world
                .get_mut::<Stats>(target_entity)
                .expect("target stats")
                .hp = 0;
            *world.get_mut::<State>(hero_entity).expect("hero state") = State::None;
            world
                .get_mut::<Stats>(hero_entity)
                .expect("hero stats")
                .stamina = Some(100);
            world.resource_mut::<GameTick>().0 += ATTACK_COOLDOWN_TICKS + 1;
        }

        game.inject(PlayerEvent::Attack {
            player_id: attacker_player_id,
            attack_type: "quick".to_string(),
            source_id: hero_id,
            target_id,
        });
        game.tick(3);

        let world = game.app.world();
        assert!(
            world.get::<StateDead>(target_entity).is_some(),
            "real PlayerEvent::Attack should produce StateDead"
        );
        assert_eq!(
            world
                .get::<Inventory>(target_entity)
                .expect("normal corpse retains inventory")
                .items
                .iter()
                .map(|item| (item.name.clone(), item.quantity))
                .collect::<Vec<_>>(),
            loot_before,
            "normal combat leaves pre-generated ordinary loot on the corpse"
        );
        loot_before
    }

    fn spawn_connected_helper(game: &mut HeadlessGame, name: &str) -> i32 {
        let helper_player_id = game.player_id() + 1;
        let helper_client = Client {
            id: Uuid::from_u128(helper_player_id as u128),
            player_id: helper_player_id,
            sender: game.packet_tx.clone(),
        };
        assert!(game.clients.activate(helper_client).is_empty());
        game.inject(PlayerEvent::NewPlayer {
            player_id: helper_player_id,
            hero_name: name.to_string(),
            class_name: "Warrior".to_string(),
        });
        game.tick(8);
        helper_player_id
    }

    fn place_player_in_own_bound_sanctuary(game: &mut HeadlessGame, player_id: i32) -> Position {
        use crate::ids::EntityObjMap;

        let world = game.app.world_mut();
        let (hero_entity, bound_monolith_id) = {
            let mut query =
                world.query_filtered::<(Entity, &PlayerId, &BoundMonolith), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner, _)| owner.0 == player_id)
                .map(|(entity, _, bound)| (entity, bound.id))
                .expect("player hero with a bound Monolith")
        };
        let monolith_entity = world
            .resource::<EntityObjMap>()
            .get_entity(bound_monolith_id)
            .expect("bound Monolith entity-map entry");
        let monolith_pos = *world
            .get::<Position>(monolith_entity)
            .expect("bound Monolith position");
        *world
            .get_mut::<Position>(hero_entity)
            .expect("player hero position") = monolith_pos;
        world
            .get_mut::<BoundMonolith>(hero_entity)
            .expect("player hero bound Monolith")
            .pos = monolith_pos;
        monolith_pos
    }

    fn spawn_armed_owner_villager(
        game: &mut HeadlessGame,
        owner_player_id: i32,
        pos: Position,
    ) -> (Entity, i32) {
        use crate::event::{GameEvent, GameEventType, GameEvents};
        use crate::ids::{EntityObjMap, Ids};
        use crate::templates::Templates;

        let existing_villager_ids = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(&Id, &PlayerId), With<SubclassVillager>>();
            query
                .iter(world)
                .filter(|(_, owner)| owner.0 == owner_player_id)
                .map(|(id, _)| id.0)
                .collect::<HashSet<_>>()
        };
        if existing_villager_ids.is_empty() {
            let current_tick = game.game_tick();
            let world = game.app.world_mut();
            let event_id = world.resource_mut::<Ids>().new_map_event_id();
            world.resource_mut::<GameEvents>().insert(
                event_id,
                GameEvent {
                    event_id,
                    start_tick: current_tick,
                    run_tick: current_tick,
                    event_type: GameEventType::SpawnVillager {
                        pos,
                        player_id: owner_player_id,
                    },
                },
            );
            game.tick(3);
        } else {
            game.app
                .world_mut()
                .run_system_once(
                    move |mut commands: Commands,
                          mut ids: ResMut<Ids>,
                          mut entity_map: ResMut<EntityObjMap>,
                          templates: Res<Templates>,
                          game_tick: Res<GameTick>| {
                        Encounter::spawn_villager(
                            owner_player_id,
                            pos,
                            Vec::new(),
                            &mut commands,
                            &mut ids,
                            &mut entity_map,
                            &templates,
                            &game_tick,
                        )
                    },
                )
                .expect("direct test villager spawn");
            game.tick(1);
        }

        let (villager_entity, villager_id) = {
            let world = game.app.world_mut();
            let mut query =
                world.query_filtered::<(Entity, &Id, &PlayerId), With<SubclassVillager>>();
            query
                .iter(world)
                .find(|(_, id, owner)| {
                    owner.0 == owner_player_id && !existing_villager_ids.contains(&id.0)
                })
                .map(|(entity, id, _)| (entity, id.0))
                .expect("owner villager")
        };

        {
            let world = game.app.world_mut();
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut inventory = world
                .get_mut::<Inventory>(villager_entity)
                .expect("villager inventory");
            inventory.new(
                item_id,
                "Copper Training Axe".to_string(),
                1,
                &item_templates,
            );
            let weapon = inventory
                .items
                .iter_mut()
                .find(|item| item.id == item_id)
                .expect("villager test weapon");
            weapon.equipped = true;
            weapon.attrs.insert(AttrKey::Damage, AttrVal::Num(20.0));
            drop(inventory);
            *world
                .get_mut::<Position>(villager_entity)
                .expect("villager position") = pos;
            let mut stats = world
                .get_mut::<Stats>(villager_entity)
                .expect("villager stats");
            stats.hp = stats.hp.max(500);
            stats.base_hp = stats.base_hp.max(500);
            stats.base_def = 0;
        }

        (villager_entity, villager_id)
    }

    fn safe_logout_fixture(name: &str) -> (HeadlessGame, Position) {
        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", name);
        let sanctuary = game.place_hero_in_own_bound_sanctuary();
        move_nearby_headless_hostiles_away(&mut game, sanctuary);
        // Let the ordinary sanctuary sync and presence reconciliation observe
        // the exact bound Monolith before the first request.
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        (game, sanctuary)
    }

    fn prepare_checkpoint2_owner_workload(
        game: &mut HeadlessGame,
        sanctuary: Position,
    ) -> (i32, i32, i32, i32, i32) {
        use crate::event::{GameEvent, GameEvents};
        use crate::game::SettlementCrisisState;
        use crate::ids::Ids;
        use crate::templates::Templates;

        let player_id = game.player_id();
        let (builder_entity, builder_id) = spawn_armed_owner_villager(game, player_id, sanctuary);
        let (refiner_entity, refiner_id) = spawn_armed_owner_villager(game, player_id, sanctuary);
        move_nearby_headless_hostiles_away(game, sanctuary);

        let (hero_id, structure_entity, structure_id, structure_pos, tick) = {
            let world = game.app.world_mut();
            let hero_id = world.resource::<Ids>().get_hero(player_id).unwrap();
            let (structure_entity, structure_id, structure_pos) = {
                let mut structures = world
                    .query_filtered::<(Entity, &Id, &PlayerId, &Position), With<ClassStructure>>();
                structures
                    .iter(world)
                    .find(|(_, _, owner, _)| owner.0 == player_id)
                    .map(|(entity, id, _, pos)| (entity, id.0, *pos))
                    .expect("owned structure")
            };
            (
                hero_id,
                structure_entity,
                structure_id,
                structure_pos,
                world.resource::<GameTick>().0,
            )
        };

        let refiner_item_id = {
            let world = game.app.world_mut();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let hero_entity = world
                .resource::<crate::ids::EntityObjMap>()
                .get_entity(hero_id)
                .expect("hero entity");
            let hero_log_id = world.resource_mut::<Ids>().new_item_id();
            world
                .get_mut::<Inventory>(hero_entity)
                .expect("hero inventory")
                .new(
                    hero_log_id,
                    "Springbranch Maple Log".to_string(),
                    1,
                    &item_templates,
                );
            *world.get_mut::<State>(hero_entity).expect("hero state") = State::Crafting;

            let refiner_item_id = world.resource_mut::<Ids>().new_item_id();
            world
                .get_mut::<Inventory>(structure_entity)
                .expect("structure inventory")
                .new(
                    refiner_item_id,
                    "Felled Swiftstep Hare".to_string(),
                    1,
                    &item_templates,
                );
            *world
                .get_mut::<State>(refiner_entity)
                .expect("refiner state") = State::Refining;
            *world
                .get_mut::<State>(builder_entity)
                .expect("builder state") = State::Building;
            *world
                .get_mut::<Position>(builder_entity)
                .expect("builder position") = structure_pos;
            *world
                .get_mut::<Position>(refiner_entity)
                .expect("refiner position") = structure_pos;
            world.entity_mut(builder_entity).insert((
                Assignment {
                    structure_id,
                    structure_name: "Checkpoint 2 construction".to_string(),
                    structure_pos,
                },
                Order::Build,
                ActiveTask::Building,
            ));
            world.entity_mut(refiner_entity).insert((
                Assignment {
                    structure_id,
                    structure_name: "Checkpoint 2 refinery".to_string(),
                    structure_pos,
                },
                Order::WorkQueue,
                ActiveTask::Refining,
            ));

            world.entity_mut(structure_entity).insert((
                StateBuilding,
                BuildUpgradeState {
                    build_upgrade_cost: 10_000.0,
                    work_done: 17.0,
                    work_per_sec: 1.0,
                    start_time: tick,
                },
            ));
            world
                .get_mut::<Assignments>(structure_entity)
                .expect("structure assignments")
                .0
                .push(builder_id);
            world
                .get_mut::<Assignments>(structure_entity)
                .expect("structure assignments")
                .0
                .push(refiner_id);
            let mut work_queue = world
                .get_mut::<WorkQueue>(structure_entity)
                .expect("structure work queue");
            work_queue.0.push(WorkEntry {
                worker_id: builder_id,
                work_type: WorkType::Build,
                work_status: WorkStatus::InProgress,
                recipe_name: None,
                recipe_image: None,
                refine_item_id: None,
                refine_item_image: None,
                refine_item_class: None,
            });
            work_queue.0.push(WorkEntry {
                worker_id: refiner_id,
                work_type: WorkType::Refine,
                work_status: WorkStatus::InProgress,
                recipe_name: None,
                recipe_image: None,
                refine_item_id: Some(refiner_item_id),
                refine_item_image: Some("hare".to_string()),
                refine_item_class: Some("Game Animal".to_string()),
            });
            drop(work_queue);
            world
                .resource_mut::<Crops>()
                .plant(tick, structure_id, "Wheat".to_string(), 3);
            if let Some(crop) = world.resource_mut::<Crops>().get_mut(&structure_id) {
                crop.stage_end = tick + 500;
            }

            let event_id = world.resource_mut::<Ids>().new_map_event_id();
            world.resource_mut::<GameEvents>().insert(
                event_id,
                GameEvent {
                    event_id,
                    start_tick: tick,
                    run_tick: tick + 5_000,
                    event_type: GameEventType::CraftEvent {
                        crafter_id: hero_id,
                        recipe_name: "Firewood".to_string(),
                    },
                },
            );

            let event_id = world.resource_mut::<Ids>().new_map_event_id();
            world.resource_mut::<GameEvents>().insert(
                event_id,
                GameEvent {
                    event_id,
                    start_tick: tick,
                    run_tick: tick + 5_000,
                    event_type: GameEventType::StructureRefineEvent {
                        refiner_id,
                        structure_id,
                        item_id: refiner_item_id,
                    },
                },
            );

            let mut crises = world.resource_mut::<SettlementCrisisState>();
            let crisis = crises.entry(player_id).or_default();
            crisis.phase = CrisisPhase::Preparing;
            crisis.pressure = 52;
            crisis.phase_started_tick = tick;
            crisis.last_evaluated_tick = tick;
            crisis.phase_online_ticks = 1;
            refiner_item_id
        };

        (
            hero_id,
            builder_id,
            refiner_id,
            structure_id,
            refiner_item_id,
        )
    }

    fn install_checkpoint2_frozen_hero_state(game: &mut HeadlessGame, hero_id: i32) {
        use crate::ids::EntityObjMap;

        let tick = game.game_tick();
        let world = game.app.world_mut();
        let entity = world
            .resource::<EntityObjMap>()
            .get_entity(hero_id)
            .expect("headless hero entity");
        {
            let mut stats = world.get_mut::<Stats>(entity).unwrap();
            stats.hp = stats.base_hp.saturating_sub(9);
            stats.stamina = stats.base_stamina.map(|value| value.saturating_sub(11));
            stats.mana = stats.base_mana.map(|value| value.saturating_sub(7));
        }
        world.get_mut::<Thirst>(entity).unwrap().thirst = 41.0;
        world.get_mut::<Hunger>(entity).unwrap().hunger = 37.0;
        if let Some(mut tired) = world.get_mut::<Tired>(entity) {
            tired.tired = 33.0;
        }
        if let Some(mut heat) = world.get_mut::<Heat>(entity) {
            heat.heat = 22.0;
        }
        world
            .get_mut::<Effects>(entity)
            .unwrap()
            .0
            .insert(Effect::Burning, (40, 1.0, 1));
        world.resource_mut::<MapEvents>().new(
            hero_id,
            tick + 40,
            VisibleEvent::EffectExpiredEvent {
                effect: Effect::Burning,
            },
        );
    }

    fn checkpoint2_player_hero_item_quantity(
        game: &mut HeadlessGame,
        player_id: i32,
        name: &str,
    ) -> i32 {
        let world = game.app.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &Inventory), With<SubclassHero>>();
        query
            .iter(world)
            .find(|(owner, _)| owner.0 == player_id)
            .map(|(_, inventory)| {
                inventory
                    .items
                    .iter()
                    .filter(|item| item.name == name)
                    .map(|item| item.quantity)
                    .sum()
            })
            .unwrap_or_default()
    }

    fn checkpoint2_hero_item_quantity(game: &mut HeadlessGame, name: &str) -> i32 {
        let player_id = game.player_id();
        checkpoint2_player_hero_item_quantity(game, player_id, name)
    }

    fn checkpoint2_player_hero_needs(game: &mut HeadlessGame, player_id: i32) -> (f32, f32, f32) {
        let world = game.app.world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &Thirst, &Hunger, &Tired), With<SubclassHero>>();
        query
            .iter(world)
            .find(|(owner, ..)| owner.0 == player_id)
            .map(|(_, thirst, hunger, tired)| (thirst.thirst, hunger.hunger, tired.tired))
            .expect("player hero needs")
    }

    fn checkpoint2_owned_structure_has_item(game: &mut HeadlessGame, item_id: i32) -> bool {
        let player_id = game.player_id();
        let world = game.app.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &Inventory), With<ClassStructure>>();
        query.iter(world).any(|(owner, inventory)| {
            owner.0 == player_id && inventory.items.iter().any(|item| item.id == item_id)
        })
    }

    fn move_nearby_headless_hostiles_away(game: &mut HeadlessGame, hero_pos: Position) {
        use crate::npc::VisibleTarget;
        use crate::safe_logout::SAFE_LOGOUT_HOSTILE_RADIUS;

        let far = far_map_position(game, hero_pos);
        let world = game.app.world_mut();
        let entities = {
            let mut query = world.query_filtered::<(
                Entity,
                &PlayerId,
                &Position,
                &Subclass,
                &State,
                &Stats,
                Option<&StateDead>,
            ), (With<SubclassNPC>, With<VisibleTarget>)>();
            query
                .iter(world)
                .filter(|(_, owner, pos, subclass, state, stats, dead)| {
                    owner.is_npc()
                        && **subclass == Subclass::Npc
                        && state.is_alive()
                        && dead.is_none()
                        && stats.hp > 0
                        && Map::distance((hero_pos.x, hero_pos.y), (pos.x, pos.y))
                            <= SAFE_LOGOUT_HOSTILE_RADIUS
                })
                .map(|(entity, ..)| entity)
                .collect::<Vec<_>>()
        };
        for entity in entities {
            *world
                .get_mut::<Position>(entity)
                .expect("headless hostile position") = far;
        }
    }

    fn begin_safe_logout(game: &mut HeadlessGame) -> i32 {
        game.request_safe_logout();
        game.tick(1);
        let record = game.player_presence_record();
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending),
            "safe-logout request was not accepted: {record:?}"
        );
        game.safe_logout_start_tick()
            .expect("accepted safe-logout start tick")
    }

    fn move_one_tile(position: Position) -> Position {
        Position {
            x: if position.x < 49 {
                position.x + 1
            } else {
                position.x - 1
            },
            y: position.y,
        }
    }

    fn far_map_position(game: &HeadlessGame, position: Position) -> Position {
        let map = game.map();
        let candidates = [
            Position { x: 0, y: 0 },
            Position {
                x: map.width - 1,
                y: map.height - 1,
            },
            Position {
                x: 0,
                y: map.height - 1,
            },
            Position {
                x: map.width - 1,
                y: 0,
            },
        ];
        candidates
            .into_iter()
            .max_by_key(|candidate| {
                Map::distance((position.x, position.y), (candidate.x, candidate.y))
            })
            .expect("map corner")
    }

    fn passable_unoccupied_adjacent_position(
        game: &mut HeadlessGame,
        origin: Position,
    ) -> Position {
        let occupied = game.observe().occupied;
        Map::range((origin.x, origin.y), 1)
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .find(|position| {
                Map::is_adjacent_excluding_source(*position, origin)
                    && Map::is_passable(position.x, position.y, game.map())
                    && !occupied.contains(&(position.x, position.y))
            })
            .expect("passable unoccupied adjacent test position")
    }

    fn expire_safe_logout_activity_cooldown(game: &mut HeadlessGame) {
        game.app.world_mut().resource_mut::<GameTick>().0 +=
            crate::safe_logout::SAFE_LOGOUT_COMBAT_COOLDOWN_TICKS + 1;
    }

    fn spawn_safe_logout_non_hostile_candidate(
        game: &mut HeadlessGame,
        owner_player_id: i32,
        subclass: Subclass,
        template: &str,
        position: Position,
    ) -> i32 {
        use crate::ids::{EntityObjMap, Ids};
        use crate::npc::VisibleTarget;

        let hero_id = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(&Id, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == game.player_id)
                .map(|(id, _)| id.0)
                .expect("headless hero id")
        };
        let world = game.app.world_mut();
        let obj_id = world.resource_mut::<Ids>().new_obj_id();
        let entity = world
            .spawn((
                Id(obj_id),
                PlayerId(owner_player_id),
                position,
                Template(template.to_string()),
                subclass,
                SubclassNPC,
                State::None,
                Stats {
                    hp: 20,
                    stamina: None,
                    mana: None,
                    base_hp: 20,
                    base_stamina: None,
                    base_mana: None,
                    base_def: 0,
                    damage_range: Some(1),
                    base_damage: Some(1),
                    base_speed: Some(1),
                    base_vision: Some(8),
                },
                VisibleTarget::new(hero_id),
            ))
            .id();
        world.resource_mut::<Ids>().new_obj(obj_id, owner_player_id);
        world.resource_mut::<EntityObjMap>().new_obj(obj_id, entity);
        obj_id
    }

    #[test]
    fn crisis_balance_tick_cap_flag_is_exact_and_spawn_relative() {
        let mut game = HeadlessGame::new(8);
        game.spawn_hero("Warrior", "BalanceCapFlagBot");
        assert!(!game.metrics().crisis_balance_tick_cap_reached);

        game.tick(8);
        assert!(game.metrics().crisis_balance_tick_cap_reached);
    }

    #[test]
    fn safe_logout_checkpoint1_presence_lifecycle_is_explicit_and_idempotent() {
        let (mut game, _) = safe_logout_fixture("SafeLogoutPresenceBot");

        // Requirements 1-6: only an explicit request can leave ordinary online
        // presence; repeated connection edges remain idempotent.
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        assert_ne!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        let disconnected = game.player_presence_record();
        game.disconnect_player();
        game.tick(2);
        assert_eq!(game.player_presence_record(), disconnected);

        game.reconnect_player();
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        let online = game.player_presence_record();
        game.reconnect_player();
        game.tick(2);
        assert_eq!(game.player_presence_record(), online);

        // A socket gap that opens and closes between ECS updates is still
        // observed at the authenticated Login boundary and cannot leave an
        // old countdown running on the replacement connection.
        let (mut gap_game, _) = safe_logout_fixture("SafeLogoutSocketGapBot");
        begin_safe_logout(&mut gap_game);
        gap_game.disconnect_player();
        gap_game.reconnect_player_with_login();
        gap_game.tick(3);
        assert_eq!(
            gap_game.player_presence(),
            Some(PlayerWorldPresence::Online)
        );
        assert_eq!(
            gap_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Disconnected)
        );

        // Connection identity also closes the async gap before the queued
        // Login is processed: a replacement socket cannot inherit a pending
        // countdown from the socket that made the request.
        let (mut delayed_login_game, _) = safe_logout_fixture("SafeLogoutDelayedLoginSocketGapBot");
        begin_safe_logout(&mut delayed_login_game);
        delayed_login_game.disconnect_player();
        delayed_login_game.reconnect_player();
        delayed_login_game.tick(1);
        assert_eq!(
            delayed_login_game.player_presence(),
            Some(PlayerWorldPresence::Online)
        );
        assert_eq!(
            delayed_login_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Disconnected)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_sanctuary_eligibility_is_owned_and_fail_closed() {
        use crate::ids::EntityObjMap;

        let (mut game, own_sanctuary) = safe_logout_fixture("SafeLogoutSanctuaryBot");
        let player_id = game.player_id;

        // Requirement 7: the player's exact bound sanctuary qualifies.
        begin_safe_logout(&mut game);
        game.cancel_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Manual)
        );

        // Requirement 8: being elsewhere does not qualify.
        let outside = far_map_position(&game, own_sanctuary);
        game.move_hero_for_test(outside);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::OutsideOwnSanctuary)
        );

        // Requirement 9: another connected player's bound sanctuary is not a
        // fallback for the owner under test.
        let helper_id = spawn_connected_helper(&mut game, "ForeignSanctuaryBot");
        let foreign_sanctuary = {
            let world = game.app.world_mut();
            let mut heroes =
                world.query_filtered::<(&PlayerId, &BoundMonolith), With<SubclassHero>>();
            let foreign_bound = heroes
                .iter(world)
                .find(|(owner, _)| owner.0 == helper_id)
                .map(|(_, bound)| bound.id)
                .expect("helper bound Monolith");
            let entity = world
                .resource::<EntityObjMap>()
                .get_entity(foreign_bound)
                .expect("helper Monolith entity");
            *world
                .get::<Position>(entity)
                .expect("helper Monolith position")
        };
        game.move_hero_for_test(foreign_sanctuary);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::OutsideOwnSanctuary)
        );

        game.move_hero_for_test(own_sanctuary);
        let (hero_entity, own_monolith_entity) = {
            let world = game.app.world_mut();
            let mut heroes =
                world.query_filtered::<(Entity, &PlayerId, &BoundMonolith), With<SubclassHero>>();
            let (hero_entity, bound_id) = heroes
                .iter(world)
                .find(|(_, owner, _)| owner.0 == player_id)
                .map(|(entity, _, bound)| (entity, bound.id))
                .expect("owner hero binding");
            let monolith_entity = world
                .resource::<EntityObjMap>()
                .get_entity(bound_id)
                .expect("owner Monolith entity");
            (hero_entity, monolith_entity)
        };

        // Requirement 10: a missing binding rejects without a nearest-zone
        // fallback.
        let binding = game
            .app
            .world_mut()
            .entity_mut(hero_entity)
            .take::<BoundMonolith>()
            .expect("owner binding");
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::MissingBoundMonolith)
        );
        game.app.world_mut().entity_mut(hero_entity).insert(binding);

        // Requirement 11: removing the live Monolith also removes its synced
        // zone; the stale entity/id is not accepted.
        let monolith = game
            .app
            .world_mut()
            .entity_mut(own_monolith_entity)
            .take::<Monolith>()
            .expect("owner Monolith component");
        game.tick(1);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::MissingSanctuaryZone)
        );
        game.app
            .world_mut()
            .entity_mut(own_monolith_entity)
            .insert(monolith);
        game.tick(1);

        // A stale bound position and a dead bound Monolith both fail closed.
        game.app
            .world_mut()
            .get_mut::<BoundMonolith>(hero_entity)
            .expect("owner binding")
            .pos = move_one_tile(own_sanctuary);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::SanctuaryInvalid)
        );
        game.place_hero_in_own_bound_sanctuary();
        *game
            .app
            .world_mut()
            .get_mut::<State>(own_monolith_entity)
            .expect("Monolith state") = State::Dead;
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::SanctuaryInvalid)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_dead_and_true_death_never_protect() {
        // Requirements 12-13: ordinary death rejects, and the presence of a
        // fresh True Death marker rejects immediately rather than waiting for
        // the delayed final run cleanup.
        let (mut dead_game, _) = safe_logout_fixture("SafeLogoutDeadBot");
        let dead_entity = {
            let world = dead_game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == dead_game.player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let dead_at = dead_game.game_tick();
        dead_game.app.world_mut().entity_mut(dead_entity).insert((
            State::Dead,
            StateDead {
                dead_at,
                killer: "Eligibility test".to_string(),
            },
        ));
        dead_game
            .app
            .world_mut()
            .get_mut::<Stats>(dead_entity)
            .expect("hero stats")
            .hp = 0;
        dead_game.request_safe_logout();
        dead_game.tick(1);
        assert_eq!(
            dead_game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::HeroDied)
        );
        assert_ne!(
            dead_game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );

        let (mut true_death_game, _) = safe_logout_fixture("SafeLogoutTrueDeathBot");
        let true_death_entity = {
            let world = true_death_game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == true_death_game.player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let true_death_at = true_death_game.game_tick();
        let state_dead = StateDead {
            dead_at: true_death_at,
            killer: "True Death eligibility test".to_string(),
        };
        true_death_game
            .app
            .world_mut()
            .entity_mut(true_death_entity)
            .insert((TrueDeath { true_death_at }, State::Dead, state_dead));
        true_death_game.request_safe_logout();
        true_death_game.tick(1);
        assert_eq!(
            true_death_game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::TrueDeath)
        );
        assert_ne!(
            true_death_game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
    }

    #[test]
    fn safe_logout_fresh_run_ignores_dormant_hidden_intro_enemy() {
        use crate::game::InitialEncounterState;
        use crate::ids::EntityObjMap;
        use crate::safe_logout::SAFE_LOGOUT_HOSTILE_RADIUS;

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "SafeLogoutFreshRunBot");
        let sanctuary = game.place_hero_in_own_bound_sanctuary();
        game.tick(1);

        // Production setup deliberately pre-spawns the future Necromancer in
        // State::Hiding and does not send it through perception. All five map
        // starts put it inside the Safe Logout radius, so exercise the untouched
        // setup rather than the usual fixture that moves nearby NPCs away.
        let (dormant_id, dormant_entity, dormant_state, dormant_pos) = {
            let world = game.app.world();
            let dormant_id = world
                .resource::<InitialEncounterState>()
                .get(&game.player_id)
                .expect("fresh-run initial encounter")
                .necromancer_id;
            let dormant_entity = world
                .resource::<EntityObjMap>()
                .get_entity(dormant_id)
                .expect("fresh-run dormant Necromancer");
            (
                dormant_id,
                dormant_entity,
                *world
                    .get::<State>(dormant_entity)
                    .expect("dormant Necromancer state"),
                *world
                    .get::<Position>(dormant_entity)
                    .expect("dormant Necromancer position"),
            )
        };
        assert_eq!(dormant_state, State::Hiding, "dormant id {dormant_id}");
        assert!(
            Map::dist(sanctuary, dormant_pos) <= SAFE_LOGOUT_HOSTILE_RADIUS,
            "production regression requires the hidden intro enemy inside the safety radius"
        );

        begin_safe_logout(&mut game);

        // Revealing the same live hostile during the pending countdown must
        // immediately restore the ordinary safety rule.
        *game
            .app
            .world_mut()
            .get_mut::<State>(dormant_entity)
            .expect("dormant Necromancer state") = State::None;
        game.tick(1);
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::HostileNearby)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_activity_hostility_and_crisis_eligibility() {
        use crate::game::CrisisKind;
        use crate::ids::EntityObjMap;

        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutEligibilityBot");

        // Requirements 14-15: both authoritative outgoing combat and incoming
        // damage impose the configured cooldown.
        game.record_player_combat_for_test();
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::RecentCombat)
        );
        expire_safe_logout_activity_cooldown(&mut game);
        game.damage_hero_for_test(1);
        game.tick(1);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::RecentDamage)
        );
        expire_safe_logout_activity_cooldown(&mut game);

        // Requirement 16: a live immediate threat blocks the request.
        let hostile = game.spawn_safe_logout_test_hostile(sanctuary);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::HostileNearby)
        );

        // A still-live threat cannot disappear from eligibility merely because
        // object-map cleanup raced ahead of deferred entity cleanup.
        let hostile_entity = game
            .app
            .world()
            .resource::<EntityObjMap>()
            .get_entity(hostile)
            .expect("registered hostile");
        game.app
            .world_mut()
            .resource_mut::<EntityObjMap>()
            .remove_obj(hostile);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::HostileNearby)
        );
        game.app
            .world_mut()
            .resource_mut::<EntityObjMap>()
            .new_obj(hostile, hostile_entity);

        // Requirement 17: the same registered entity no longer blocks once it
        // is a dead corpse.
        game.kill_safe_logout_test_hostile(hostile);
        begin_safe_logout(&mut game);
        game.cancel_safe_logout();
        game.tick(1);

        // Requirements 18-19 and the cross-player friendly-unit rule: fixtures
        // satisfy the hostile query's shape but are excluded by ownership or
        // subclass, just like real villagers and merchants.
        let player_id = game.player_id;
        let villager = spawn_safe_logout_non_hostile_candidate(
            &mut game,
            player_id,
            Subclass::Villager,
            "Villager",
            sanctuary,
        );
        let merchant = spawn_safe_logout_non_hostile_candidate(
            &mut game,
            crate::constants::MERCHANT_PLAYER_ID,
            Subclass::Merchant,
            "Merchant",
            sanctuary,
        );
        let friendly_other = spawn_safe_logout_non_hostile_candidate(
            &mut game,
            player_id + 1,
            Subclass::Npc,
            "Wolf",
            sanctuary,
        );
        begin_safe_logout(&mut game);
        game.cancel_safe_logout();
        game.tick(1);

        // Requirement 21: every explicit pre-assault phase remains eligible;
        // the safe-logout system must not accidentally narrow this list when
        // crisis phases evolve.
        for kind in [CrisisKind::Goblin, CrisisKind::Undead] {
            for phase in [
                CrisisPhase::Dormant,
                CrisisPhase::Signs,
                CrisisPhase::Pressure,
                CrisisPhase::Preparing,
                CrisisPhase::AssaultReady,
            ] {
                let current_tick = game.game_tick();
                let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
                let crisis = crises.get_mut(&game.player_id).expect("personal crisis");
                crisis.kind = kind;
                crisis.phase = phase;
                crisis.phase_online_ticks = 0;
                crisis.last_evaluated_tick = current_tick;
                begin_safe_logout(&mut game);
                game.cancel_safe_logout();
                game.tick(1);
            }
        }

        // Requirement 20: committed assault state always rejects.
        let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
        let crisis = crises.get_mut(&game.player_id).expect("personal crisis");
        crisis.kind = CrisisKind::Undead;
        crisis.phase = CrisisPhase::AssaultActive;
        drop(crises);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::AssaultActive)
        );

        for obj_id in [hostile, villager, merchant, friendly_other] {
            game.remove_safe_logout_test_hostile(obj_id);
        }
    }

    #[test]
    fn safe_logout_checkpoint1_deterministic_move_cancel_then_exact_completion() {
        use crate::safe_logout::SAFE_LOGOUT_COUNTDOWN_TICKS;

        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutCountdownBot");

        // Requirements 22-28 plus the first mandated deterministic scenario.
        let first_start = begin_safe_logout(&mut game);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::AlreadyPending)
        );
        assert_eq!(game.safe_logout_start_tick(), Some(first_start));

        game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS / 2) as u32);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending)
        );
        let evaluated_tick = game.game_tick();
        game.app.world_mut().resource_mut::<GameTick>().0 = evaluated_tick - 1;
        game.tick(1);
        assert_eq!(game.game_tick(), evaluated_tick);
        assert_eq!(game.safe_logout_start_tick(), Some(first_start));
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending),
            "evaluating the same authoritative tick twice must not advance the countdown"
        );
        game.move_hero_for_test(move_one_tile(sanctuary));
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Moved)
        );

        game.place_hero_in_own_bound_sanctuary();
        let second_start = begin_safe_logout(&mut game);
        assert!(second_start > first_start);
        game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending),
            "countdown must not complete one tick early"
        );
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        assert_eq!(game.safe_logout_start_tick(), None);

        // Re-evaluation and a duplicate request cannot produce a second
        // transition or restart pending state.
        game.tick(2);
        game.request_safe_logout();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::AlreadyProtected)
        );

        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected),
            "a socket close after explicit completion preserves the completed handoff"
        );
        game.reconnect_and_exit_protection();
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
    }

    #[test]
    fn safe_logout_checkpoint1_combat_damage_and_hostile_entry_cancel() {
        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutCancelBot");

        // Requirements 30-34: the aggregate server combat hook is shared by
        // attacks, damaging abilities, and combos; incoming damage and a newly
        // arrived hostile have their own cancellation reasons.
        begin_safe_logout(&mut game);
        game.record_player_combat_for_test();
        game.tick(1);
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::EnteredCombat)
        );

        expire_safe_logout_activity_cooldown(&mut game);
        begin_safe_logout(&mut game);
        game.damage_hero_for_test(1);
        game.tick(1);
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::TookDamage)
        );

        expire_safe_logout_activity_cooldown(&mut game);
        let hostile = game.spawn_safe_logout_test_hostile(far_map_position(&game, sanctuary));
        begin_safe_logout(&mut game);
        game.move_safe_logout_test_hostile(hostile, sanctuary);
        game.tick(1);
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::HostileNearby)
        );
        game.remove_safe_logout_test_hostile(hostile);
    }

    #[test]
    fn safe_logout_non_panicking_driver_preserves_cancellation_telemetry() {
        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutDriverCancelBot");
        let hostile = game.spawn_safe_logout_test_hostile(far_map_position(&game, sanctuary));
        game.request_safe_logout_via_authenticated_ingress();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending)
        );

        game.move_safe_logout_test_hostile(hostile, sanctuary);
        assert_eq!(
            game.try_drive_safe_logout_to_completion(),
            SafeLogoutCompletionOutcome::Cancelled(SafeLogoutCancelReason::HostileNearby)
        );
        let telemetry = game.safe_logout_telemetry();
        assert_eq!(telemetry.accepted, 1);
        assert_eq!(telemetry.cancelled, 1);
        assert_eq!(
            telemetry
                .cancellation_reasons
                .get(&SafeLogoutCancelReason::HostileNearby),
            Some(&1)
        );
        game.remove_safe_logout_test_hostile(hostile);
    }

    #[test]
    fn safe_logout_balance_preparation_relocates_approaching_hostiles() {
        use crate::safe_logout::SAFE_LOGOUT_HOSTILE_RADIUS;

        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutBalancePrepBot");
        let just_outside = Map::range((sanctuary.x, sanctuary.y), SAFE_LOGOUT_HOSTILE_RADIUS + 1)
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .find(|position| {
                Map::is_valid_pos((position.x, position.y))
                    && Map::distance((sanctuary.x, sanctuary.y), (position.x, position.y))
                        == SAFE_LOGOUT_HOSTILE_RADIUS + 1
            })
            .expect("valid tile just outside the hostile radius");
        let hostile = game.spawn_safe_logout_test_hostile(just_outside);
        game.queue_hostile_spell_damage_for_test(hostile, 5);
        game.record_player_combat_for_test();
        game.damage_hero_for_test(1);
        game.tick(1);

        let hero = game.observe().hero.expect("safe-logout hero");
        let queued_tick = game.game_tick().saturating_add(5);
        {
            let world = game.app.world_mut();
            let hero_entity = world
                .resource::<EntityObjMap>()
                .get_entity(hero.id)
                .expect("safe-logout hero entity");
            *world
                .get_mut::<State>(hero_entity)
                .expect("safe-logout hero state") = State::Moving;
            world.resource_mut::<MapEvents>().new(
                hero.id,
                queued_tick,
                VisibleEvent::MoveEvent {
                    src: hero.pos,
                    dst: just_outside,
                },
            );
        }

        let prepared_sanctuary = game.prepare_safe_logout_scenario();
        assert_eq!(prepared_sanctuary, sanctuary);
        assert!(game
            .world()
            .resource::<MapEvents>()
            .values()
            .all(|event| event.obj_id != hero.id));
        assert_eq!(
            game.world()
                .get::<State>(
                    game.world()
                        .resource::<EntityObjMap>()
                        .get_entity(hero.id)
                        .expect("prepared hero entity")
                )
                .copied(),
            Some(State::None)
        );
        let moved_hostile = game
            .observe()
            .enemies
            .into_iter()
            .find(|enemy| enemy.id == hostile)
            .expect("prepared hostile remains in the world");
        assert!(
            Map::distance(
                (sanctuary.x, sanctuary.y),
                (moved_hostile.pos.x, moved_hostile.pos.y),
            ) > SAFE_LOGOUT_HOSTILE_RADIUS
        );
        assert_eq!(
            game.try_complete_valid_safe_logout_via_authenticated_ingress(),
            SafeLogoutCompletionOutcome::Completed
        );
        game.remove_safe_logout_test_hostile(hostile);
    }

    #[test]
    fn safe_logout_checkpoint1_production_incoming_damage_cancels() {
        use big_brain::prelude::{ActionState, Actor};

        use crate::common::AttackTarget;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;

        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutProductionDamageBot");
        begin_safe_logout(&mut game);

        let hostile_position = Map::range((sanctuary.x, sanctuary.y), 1)
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .find(|position| {
                Map::is_adjacent_excluding_source(*position, sanctuary)
                    && Map::is_passable(position.x, position.y, game.map())
            })
            .expect("passable melee tile beside the sanctuary");
        let hostile_id = game.spawn_safe_logout_test_hostile(hostile_position);
        let (hero_entity, hero_id, hp_before, hostile_entity) = {
            let world = game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &Id, &Stats), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_stats) =
                heroes.iter(world).next().expect("headless hero");
            let hostile_entity = world
                .resource::<EntityObjMap>()
                .get_entity(hostile_id)
                .expect("production damage attacker");
            (hero_entity, hero_id.0, hero_stats.hp, hostile_entity)
        };
        {
            let world = game.app.world_mut();
            world
                .get_mut::<Stats>(hostile_entity)
                .expect("production damage attacker stats")
                .base_damage = Some(30);
            world
                .get_mut::<VisibleTarget>(hostile_entity)
                .expect("production damage attacker target")
                .target = hero_id;
            world.spawn((Actor(hostile_entity), ActionState::Requested, AttackTarget));
        }

        game.tick(1);

        let hero_stats = game
            .world()
            .get::<Stats>(hero_entity)
            .expect("headless hero stats after production attack");
        assert!(
            hero_stats.hp < hp_before,
            "the production combat system must apply incoming damage"
        );
        assert!(game.world().get::<StateDead>(hero_entity).is_none());
        assert!(
            game.world().get::<LastDamageTick>(hero_entity).is_some(),
            "the production damage writer must stamp the authoritative hero"
        );
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::TookDamage),
            "damage is evaluated before the same attacker's proximity"
        );
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
    }

    #[test]
    fn safe_logout_checkpoint1_real_attack_ability_and_combo_paths_cancel() {
        use crate::combat::{AttackType, ComboTracker};
        use crate::ids::EntityObjMap;
        use crate::player::PlayerEvents;

        fn hero_id(game: &mut HeadlessGame) -> i32 {
            let player_id = game.player_id;
            let world = game.app.world_mut();
            let mut heroes = world.query_filtered::<(&Id, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(id, _)| id.0)
                .expect("headless hero id")
        }

        fn run_event(
            game: &mut HeadlessGame,
            event_id: i32,
            event: PlayerEvent,
        ) -> SafeLogoutCancelReason {
            game.app
                .world_mut()
                .resource_mut::<PlayerEvents>()
                .insert(event_id, event);
            game.tick(1);
            game.safe_logout_cancel_reason()
                .expect("accepted combat event should cancel safe logout")
        }

        // Requirement 30: a successfully accepted ordinary attack records
        // authoritative activity before the PostUpdate completion check.
        let (mut attack_game, sanctuary) = safe_logout_fixture("SafeLogoutRealAttackBot");
        begin_safe_logout(&mut attack_game);
        let attack_source = hero_id(&mut attack_game);
        attack_game
            .app
            .world_mut()
            .resource_mut::<PlayerEvents>()
            .insert(
                -29,
                PlayerEvent::Attack {
                    player_id: HEADLESS_PLAYER_ID,
                    attack_type: "quick".to_string(),
                    source_id: attack_source,
                    target_id: i32::MAX,
                },
            );
        attack_game.tick(1);
        assert_eq!(
            attack_game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending),
            "a rejected target id is not authoritative combat activity"
        );
        assert_eq!(attack_game.safe_logout_cancel_reason(), None);

        let attack_target = attack_game.spawn_safe_logout_test_hostile(sanctuary);
        assert_eq!(
            run_event(
                &mut attack_game,
                -30,
                PlayerEvent::Attack {
                    player_id: HEADLESS_PLAYER_ID,
                    attack_type: "quick".to_string(),
                    source_id: attack_source,
                    target_id: attack_target,
                },
            ),
            SafeLogoutCancelReason::EnteredCombat
        );

        // Requirement 31: a damaging class ability follows the same accepted
        // server path; this uses the Warrior's adjacent Guard Bash.
        let (mut ability_game, sanctuary) = safe_logout_fixture("SafeLogoutRealAbilityBot");
        begin_safe_logout(&mut ability_game);
        let ability_target = ability_game.spawn_safe_logout_test_hostile(sanctuary);
        let ability_source = hero_id(&mut ability_game);
        assert_eq!(
            run_event(
                &mut ability_game,
                -31,
                PlayerEvent::Ability {
                    player_id: HEADLESS_PLAYER_ID,
                    ability_id: "shield_bash".to_string(),
                    source_id: ability_source,
                    target_id: Some(ability_target),
                },
            ),
            SafeLogoutCancelReason::EnteredCombat
        );

        // Requirement 32: a ready damaging combo must also cancel through the
        // real combo event rather than a test-only activity shortcut.
        let (mut combo_game, sanctuary) = safe_logout_fixture("SafeLogoutRealComboBot");
        begin_safe_logout(&mut combo_game);
        let combo_target = combo_game.spawn_safe_logout_test_hostile(sanctuary);
        let combo_source = hero_id(&mut combo_game);
        let combo_hero = combo_game
            .app
            .world()
            .resource::<EntityObjMap>()
            .get_entity(combo_source)
            .expect("headless hero entity");
        combo_game
            .app
            .world_mut()
            .entity_mut(combo_hero)
            .insert(ComboTracker {
                target_id: combo_target,
                attacks: vec![AttackType::Quick, AttackType::Quick],
            });
        assert_eq!(
            run_event(
                &mut combo_game,
                -32,
                PlayerEvent::Combo {
                    player_id: HEADLESS_PLAYER_ID,
                    source_id: combo_source,
                    target_id: combo_target,
                    combo_type: "Hamstring".to_string(),
                },
            ),
            SafeLogoutCancelReason::EnteredCombat
        );
    }

    #[test]
    fn safe_logout_checkpoint1_manual_disconnect_sanctuary_and_death_cancel_once() {
        // Requirements 29, 35-44. Movement is covered by the deterministic
        // scenario; these fixtures exercise the remaining state sources and
        // verify cancellation has no economy/crisis side effects.
        let (mut manual_game, _) = safe_logout_fixture("SafeLogoutManualBot");
        let pressure_before = manual_game
            .settlement_crisis()
            .expect("personal crisis")
            .pressure;
        let rewards_before = manual_game.personal_crises_resolved();
        begin_safe_logout(&mut manual_game);
        manual_game.cancel_safe_logout();
        manual_game.tick(1);
        assert_eq!(
            manual_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Manual)
        );
        let cancelled = manual_game.player_presence_record();
        manual_game.cancel_safe_logout();
        manual_game.tick(1);
        assert_eq!(manual_game.player_presence_record(), cancelled);
        assert_eq!(
            manual_game
                .settlement_crisis()
                .expect("personal crisis")
                .pressure,
            pressure_before
        );
        assert_eq!(manual_game.personal_crises_resolved(), rewards_before);

        let (mut disconnect_game, _) = safe_logout_fixture("SafeLogoutDisconnectBot");
        begin_safe_logout(&mut disconnect_game);
        disconnect_game.disconnect_player();
        disconnect_game.tick(1);
        assert_eq!(
            disconnect_game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        assert_eq!(
            disconnect_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Disconnected)
        );

        let (mut invalid_game, _) = safe_logout_fixture("SafeLogoutInvalidZoneBot");
        let (hero_entity, monolith_entity) = {
            use crate::ids::EntityObjMap;
            let world = invalid_game.app.world_mut();
            let mut heroes =
                world.query_filtered::<(Entity, &PlayerId, &BoundMonolith), With<SubclassHero>>();
            let (hero, monolith_id) = heroes
                .iter(world)
                .find(|(_, owner, _)| owner.0 == invalid_game.player_id)
                .map(|(entity, _, bound)| (entity, bound.id))
                .expect("headless binding");
            let monolith = world
                .resource::<EntityObjMap>()
                .get_entity(monolith_id)
                .expect("bound Monolith");
            (hero, monolith)
        };
        begin_safe_logout(&mut invalid_game);
        invalid_game
            .app
            .world_mut()
            .get_mut::<BoundMonolith>(hero_entity)
            .expect("binding")
            .pos = move_one_tile(
            *invalid_game
                .app
                .world()
                .get::<Position>(monolith_entity)
                .expect("Monolith position"),
        );
        invalid_game.tick(1);
        assert_eq!(
            invalid_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::SanctuaryInvalid)
        );

        let (mut left_game, sanctuary) = safe_logout_fixture("SafeLogoutLeftZoneBot");
        let (left_hero, left_monolith) = {
            use crate::ids::EntityObjMap;
            let world = left_game.app.world_mut();
            let mut heroes =
                world.query_filtered::<(Entity, &PlayerId, &BoundMonolith), With<SubclassHero>>();
            let (hero, bound_id) = heroes
                .iter(world)
                .find(|(_, owner, _)| owner.0 == left_game.player_id)
                .map(|(entity, _, bound)| (entity, bound.id))
                .expect("headless binding");
            let monolith = world
                .resource::<EntityObjMap>()
                .get_entity(bound_id)
                .expect("bound Monolith");
            (hero, monolith)
        };
        begin_safe_logout(&mut left_game);
        let far = far_map_position(&left_game, sanctuary);
        *left_game
            .app
            .world_mut()
            .get_mut::<Position>(left_monolith)
            .expect("Monolith position") = far;
        left_game
            .app
            .world_mut()
            .get_mut::<BoundMonolith>(left_hero)
            .expect("binding")
            .pos = far;
        left_game.tick(1);
        assert_eq!(
            left_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::LeftSanctuary)
        );

        let (mut dead_game, _) = safe_logout_fixture("SafeLogoutPendingDeathBot");
        begin_safe_logout(&mut dead_game);
        let dead_entity = {
            let world = dead_game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == dead_game.player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let dead_at = dead_game.game_tick();
        dead_game.app.world_mut().entity_mut(dead_entity).insert((
            State::Dead,
            StateDead {
                dead_at,
                killer: "Cancellation test".to_string(),
            },
        ));
        dead_game.tick(1);
        assert_eq!(
            dead_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::HeroDied)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_same_tick_danger_wins_over_completion() {
        use crate::safe_logout::SAFE_LOGOUT_COUNTDOWN_TICKS;

        // Requirements 45-47: arrange each danger on the exact update that
        // would otherwise complete the absolute-tick countdown.
        let (mut assault_game, _) = safe_logout_fixture("SafeLogoutRaceAssaultBot");
        let preferred_tick = set_personal_assault_ready(&mut assault_game);
        let pre_request_tick = preferred_tick - SAFE_LOGOUT_COUNTDOWN_TICKS - 1;
        {
            use crate::game::ASSAULT_READY_GRACE_TICKS;

            let world = assault_game.app.world_mut();
            world.resource_mut::<GameTick>().0 = pre_request_tick;
            let mut crises = world.resource_mut::<SettlementCrisisState>();
            let crisis = crises
                .get_mut(&assault_game.player_id)
                .expect("ready personal crisis");
            crisis.phase_online_ticks = ASSAULT_READY_GRACE_TICKS - SAFE_LOGOUT_COUNTDOWN_TICKS - 1;
            crisis.last_evaluated_tick = pre_request_tick;
        }
        let requested_tick = begin_safe_logout(&mut assault_game);
        assert_eq!(requested_tick + SAFE_LOGOUT_COUNTDOWN_TICKS, preferred_tick);
        assault_game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
        assert_eq!(
            assault_game
                .settlement_crisis()
                .expect("ready personal crisis")
                .phase,
            CrisisPhase::AssaultReady
        );
        assert!(assault_game.crisis_assault_units().is_empty());
        assault_game.tick(1);
        assert_eq!(
            assault_game
                .settlement_crisis()
                .expect("launched personal crisis")
                .phase,
            CrisisPhase::AssaultActive
        );
        assert!(!assault_game.crisis_assault_units().is_empty());
        assert_eq!(
            assault_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::AssaultStarted)
        );

        let (mut damage_game, _) = safe_logout_fixture("SafeLogoutRaceDamageBot");
        begin_safe_logout(&mut damage_game);
        damage_game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
        damage_game.damage_hero_for_test(1);
        damage_game.tick(1);
        assert_eq!(
            damage_game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::TookDamage)
        );

        let (mut death_game, _) = safe_logout_fixture("SafeLogoutRaceTrueDeathBot");
        begin_safe_logout(&mut death_game);
        death_game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
        let hero_entity = {
            let world = death_game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == death_game.player_id)
                .map(|(entity, _)| entity)
                .expect("headless hero")
        };
        let true_death_at = death_game.game_tick() - 101;
        let state_dead = StateDead {
            dead_at: true_death_at,
            killer: "True Death ordering test".to_string(),
        };
        death_game.app.world_mut().entity_mut(hero_entity).insert((
            TrueDeath { true_death_at },
            State::Dead,
            state_dead,
        ));
        death_game.tick(1);
        assert_ne!(
            death_game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
    }

    #[test]
    fn safe_logout_checkpoint1_fresh_run_and_cleanup_clear_stale_presence() {
        use crate::safe_logout::remove_player_presence_for_run_cleanup;

        // Requirements 48-49: new-run setup overwrites both stale pending and
        // stale protected records before gameplay begins.
        for stale_state in [
            PlayerWorldPresence::SafeLogoutPending,
            PlayerWorldPresence::OfflineProtected,
        ] {
            let mut game = HeadlessGame::new(20_000);
            let mut stale = PlayerPresenceRecord::new(true);
            stale.state = stale_state;
            stale.safe_logout_requested_tick = Some(123);
            stale.safe_logout_start_position = Some(Position { x: 1, y: 1 });
            game.app
                .world_mut()
                .resource_mut::<PlayerWorldPresenceState>()
                .players
                .insert(HEADLESS_PLAYER_ID, stale);
            game.spawn_hero("Warrior", "SafeLogoutFreshRunBot");
            let fresh = game
                .player_presence_record()
                .expect("fresh run presence record");
            assert_eq!(fresh.state, PlayerWorldPresence::Online);
            assert_eq!(fresh.safe_logout_requested_tick, None);
            assert_eq!(fresh.safe_logout_start_position, None);
        }

        // Requirements 50-51: cleanup is player-scoped and repeatable.
        let (mut game, _) = safe_logout_fixture("SafeLogoutCleanupOwnerBot");
        let helper_id = spawn_connected_helper(&mut game, "SafeLogoutCleanupNeighborBot");
        let neighbor_before = game
            .app
            .world()
            .resource::<PlayerWorldPresenceState>()
            .players
            .get(&helper_id)
            .cloned()
            .expect("neighbor presence");
        let tick = game.game_tick();
        {
            let world = game.app.world_mut();
            world.resource_scope(|world, mut presence: Mut<PlayerWorldPresenceState>| {
                let mut telemetry = world.resource_mut::<SafeLogoutTelemetryState>();
                remove_player_presence_for_run_cleanup(
                    game.player_id,
                    tick,
                    &mut presence,
                    &mut telemetry,
                );
                remove_player_presence_for_run_cleanup(
                    game.player_id,
                    tick,
                    &mut presence,
                    &mut telemetry,
                );
            });
        }
        let presence = game.app.world().resource::<PlayerWorldPresenceState>();
        assert!(!presence.players.contains_key(&game.player_id));
        assert_eq!(presence.players.get(&helper_id), Some(&neighbor_before));
    }

    #[test]
    fn safe_logout_checkpoint1_active_assault_cancels_then_disconnect_continues() {
        let (mut game, _) = safe_logout_fixture("SafeLogoutAssaultContinuityBot");
        let preferred_tick = set_personal_assault_ready(&mut game);
        game.app.world_mut().resource_mut::<GameTick>().0 = preferred_tick - 20;
        begin_safe_logout(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);

        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::AssaultStarted)
        );
        let active = game.settlement_crisis().expect("active assault");
        assert_eq!(active.phase, CrisisPhase::AssaultActive);
        let assault_id = active.assault_id.expect("assault id");
        let generation = active.assault_spawn_generation;
        let unit_ids = game
            .crisis_assault_units()
            .into_iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        assert!(!unit_ids.is_empty());

        // Requirements 52-54 and the second mandated deterministic scenario:
        // ordinary disconnect changes only presence; the committed assault,
        // identity, generation, and live entities remain.
        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        let disconnected = game.settlement_crisis().expect("offline assault");
        assert_eq!(disconnected.phase, CrisisPhase::AssaultActive);
        assert_eq!(disconnected.assault_id, Some(assault_id));
        assert_eq!(disconnected.assault_spawn_generation, generation);
        assert_eq!(
            game.crisis_assault_units()
                .into_iter()
                .map(|unit| unit.obj_id)
                .collect::<Vec<_>>(),
            unit_ids
        );
    }

    #[test]
    fn safe_logout_checkpoint3_scenario_a_production_flow_is_ordered_and_deduplicated() {
        use crate::constants::TICKS_PER_SEC;
        use crate::safe_logout::{SAFE_LOGOUT_COUNTDOWN_TICKS, SAFE_LOGOUT_STATUS_VERSION};

        let (mut game, _) = safe_logout_fixture("SafeLogoutProtocolSuccessBot");

        // A fresh authenticated run receives a current snapshot without first
        // issuing a command. The fixture has already moved the hero into its
        // authoritative sanctuary, so its last initial snapshot is eligible.
        let initial = safe_logout_statuses(game.take_safe_logout_status_packets());
        let initial = initial.last().expect("fresh-run safe-logout snapshot");
        assert_eq!(initial.version, SAFE_LOGOUT_STATUS_VERSION);
        assert_eq!(initial.state, "online");
        assert!(initial.can_request);
        assert!(!initial.can_cancel);
        assert!(initial.in_own_sanctuary);
        assert!(!initial.protected);

        // Exercise the same PlayerEvent ingress and bridge used by an
        // authenticated WebSocket command, not the direct internal test hook.
        game.request_safe_logout_via_authenticated_ingress();
        game.tick(1);
        let requested_tick = game
            .safe_logout_start_tick()
            .expect("authenticated request accepted");
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending)
        );

        let mut statuses = safe_logout_statuses(game.take_safe_logout_status_packets());
        assert_eq!(statuses.len(), 1, "acceptance sends one pending snapshot");
        assert_eq!(statuses[0].state, "pending");
        assert_eq!(
            statuses[0].countdown_total_seconds,
            Some(SAFE_LOGOUT_COUNTDOWN_TICKS / TICKS_PER_SEC)
        );
        assert_eq!(
            statuses[0].countdown_remaining_seconds,
            Some(SAFE_LOGOUT_COUNTDOWN_TICKS / TICKS_PER_SEC)
        );
        assert!(statuses[0].can_cancel);
        assert!(!statuses[0].can_request);
        let mut pending_delivery_ticks = vec![game.game_tick()];

        let elapsed = game.game_tick().saturating_sub(requested_tick);
        assert!(
            elapsed < SAFE_LOGOUT_COUNTDOWN_TICKS,
            "accepted request cannot already be complete"
        );
        let ticks_to_last_pending = SAFE_LOGOUT_COUNTDOWN_TICKS - 1 - elapsed;
        for _ in 0..ticks_to_last_pending {
            game.tick(1);
            let emitted = safe_logout_statuses(game.take_safe_logout_status_packets());
            assert!(
                emitted.len() <= 1,
                "one connection cannot receive multiple pending snapshots in one update"
            );
            if !emitted.is_empty() {
                pending_delivery_ticks.push(game.game_tick());
                statuses.extend(emitted);
            }
        }
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending),
            "the authoritative countdown remains pending one tick before completion"
        );

        let countdown_values = statuses
            .iter()
            .map(|status| {
                assert_eq!(status.state, "pending");
                assert!(status.can_cancel);
                status
                    .countdown_remaining_seconds
                    .expect("pending countdown value")
            })
            .collect::<Vec<_>>();
        let expected_countdown = (1..=SAFE_LOGOUT_COUNTDOWN_TICKS / TICKS_PER_SEC)
            .rev()
            .collect::<Vec<_>>();
        assert_eq!(
            countdown_values, expected_countdown,
            "ceil-rounded delivery emits exactly one restrained packet per countdown second"
        );
        assert!(
            statuses.windows(2).all(|pair| pair[0] != pair[1]),
            "the per-connection cache suppresses duplicate semantic snapshots"
        );
        assert!(
            pending_delivery_ticks
                .windows(2)
                .all(|ticks| ticks[1] - ticks[0] >= TICKS_PER_SEC),
            "pending delivery is limited to at most one update per server second"
        );

        // The final packet is drained only after the update has committed the
        // authoritative transition, proving presentation cannot lead state.
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        let completed = safe_logout_statuses(game.take_safe_logout_status_packets());
        assert_eq!(completed.len(), 1, "completion sends one terminal snapshot");
        assert_eq!(completed[0].state, "protected");
        assert_eq!(completed[0].countdown_remaining_seconds, Some(0));
        assert!(completed[0].protected);
        assert!(!completed[0].can_request);
        assert!(!completed[0].can_cancel);
        assert!(!statuses.iter().any(|status| status.protected));
        let packet_order = statuses
            .iter()
            .chain(completed.iter())
            .map(|status| status.state.as_str())
            .collect::<Vec<_>>();
        let mut expected_order = vec!["pending"; expected_countdown.len()];
        expected_order.push("protected");
        assert_eq!(packet_order, expected_order);

        let protected_before_close = game.protected_hero_snapshot();
        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected),
            "closing after the protected snapshot preserves the completed handoff"
        );
        assert!(game.take_safe_logout_status_packets().is_empty());

        game.advance_protected_world_ticks(25);
        assert_eq!(
            game.protected_hero_snapshot(),
            protected_before_close,
            "Checkpoint 2 protection remains active after the client closes"
        );

        // A later explicit authenticated login gets a fresh per-connection
        // status only after the ordered resume has synchronized and returned
        // the run online. The first connected update remains protected.
        game.reconnect_player_with_login();
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        assert!(game.player_simulation_is_protected());
        assert!(game.take_safe_logout_status_packets().is_empty());
        for _ in 0..16 {
            game.tick(1);
            if !game.player_simulation_is_protected() {
                break;
            }
        }
        assert!(!game.player_simulation_is_protected());
        let resumed = safe_logout_statuses(game.take_safe_logout_status_packets());
        assert_eq!(
            resumed.len(),
            1,
            "new connection receives one fresh snapshot"
        );
        assert_eq!(resumed[0].state, "online");
        assert!(!resumed[0].protected);
        assert!(resumed[0].resumed_from_protection);
        let telemetry = game.safe_logout_telemetry();
        assert_eq!(telemetry.resumed, 1);
        assert_eq!(telemetry.timer_rebases, 1);
        game.tick(3);
        assert_eq!(game.safe_logout_telemetry().timer_rebases, 1);
        assert!(game.take_safe_logout_status_packets().is_empty());
    }

    #[test]
    fn safe_logout_checkpoint3_scenario_b_disconnect_before_completion_stays_unprotected() {
        use crate::safe_logout::SAFE_LOGOUT_COUNTDOWN_TICKS;

        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutProtocolDisconnectBot");
        let _ = game.take_safe_logout_status_packets();
        let hostile_id = game.spawn_safe_logout_test_hostile(far_map_position(&game, sanctuary));

        game.request_safe_logout_via_authenticated_ingress();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending)
        );
        game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS / 3) as u32);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending)
        );

        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Disconnected)
        );
        let record = game
            .player_presence_record()
            .expect("disconnected presence record");
        assert_eq!(record.protected_since_tick, None);
        assert_eq!(record.protected_run_key, None);

        let statuses = safe_logout_statuses(game.take_safe_logout_status_packets());
        assert!(
            statuses.iter().all(|status| !status.protected),
            "a socket close before completion never publishes protected state"
        );

        // Ordinary persistent-world mutation remains live. A hostile spell that
        // Checkpoint 2 would block is processed against the unprotected run.
        let hp_before = game.protected_hero_snapshot().hp;
        game.queue_hostile_spell_damage_for_test(hostile_id, 0);
        for _ in 0..5 {
            game.tick(1);
            if game.protected_hero_snapshot().hp < hp_before {
                break;
            }
        }
        assert!(
            game.protected_hero_snapshot().hp < hp_before,
            "ordinary disconnected simulation must continue without Checkpoint 2 protection"
        );
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        assert!(game.take_safe_logout_status_packets().is_empty());
    }

    #[test]
    fn safe_logout_checkpoint3_scenario_c_cancel_rerequest_and_manual_cancel_are_idempotent() {
        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutProtocolCancelBot");
        let _ = game.take_safe_logout_status_packets();

        game.request_safe_logout_via_authenticated_ingress();
        game.tick(1);
        let first_start = game
            .safe_logout_start_tick()
            .expect("first authenticated request accepted");
        let accepted = safe_logout_statuses(game.take_safe_logout_status_packets());
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].state, "pending");

        game.move_hero_for_test(move_one_tile(sanctuary));
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Moved)
        );
        let moved = safe_logout_statuses(game.take_safe_logout_status_packets());
        assert_eq!(moved.len(), 1, "movement sends one cancellation snapshot");
        assert_eq!(moved[0].state, "online");
        assert_eq!(moved[0].reason.as_deref(), Some("moved"));
        assert!(!moved[0].can_cancel);
        assert!(!moved[0].protected);

        game.tick(3);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        assert!(
            game.take_safe_logout_status_packets().is_empty(),
            "cancellation never schedules an automatic retry"
        );

        game.place_hero_in_own_bound_sanctuary();
        game.tick(1);
        let _ = game.take_safe_logout_status_packets();
        game.request_safe_logout_via_authenticated_ingress();
        game.tick(1);
        let second_start = game
            .safe_logout_start_tick()
            .expect("explicit re-request accepted after conditions recover");
        assert!(second_start > first_start);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::SafeLogoutPending)
        );
        let second_pending = safe_logout_statuses(game.take_safe_logout_status_packets());
        assert_eq!(second_pending.len(), 1);
        assert_eq!(second_pending[0].state, "pending");

        game.cancel_safe_logout_via_authenticated_ingress();
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        assert_eq!(
            game.safe_logout_cancel_reason(),
            Some(SafeLogoutCancelReason::Manual)
        );
        let cancelled_record = game
            .player_presence_record()
            .expect("manual cancellation record");
        let manual = safe_logout_statuses(game.take_safe_logout_status_packets());
        assert_eq!(manual.len(), 1);
        assert_eq!(manual[0].state, "online");
        assert_eq!(manual[0].reason.as_deref(), Some("manually_cancelled"));
        assert!(!manual[0].can_cancel);

        game.cancel_safe_logout_via_authenticated_ingress();
        game.tick(1);
        assert_eq!(
            game.player_presence_record(),
            Some(cancelled_record),
            "duplicate manual cancellation is an idempotent no-op"
        );
        assert!(game.take_safe_logout_status_packets().is_empty());
        assert_ne!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
    }

    #[test]
    fn safe_logout_checkpoint3_scenario_d_active_assault_rejects_and_continues() {
        let (mut game, _) = safe_logout_fixture("SafeLogoutProtocolAssaultBot");
        let _ = game.take_safe_logout_status_packets();
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);

        let active_before = game.settlement_crisis().expect("active personal assault");
        assert_eq!(active_before.phase, CrisisPhase::AssaultActive);
        let assault_id = active_before.assault_id.expect("committed assault id");
        let generation = active_before.assault_spawn_generation;
        let units_before = game.crisis_assault_units();
        assert!(!units_before.is_empty());

        game.request_safe_logout_via_authenticated_ingress();
        game.tick(1);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
        assert_eq!(
            game.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::AssaultActive)
        );
        let statuses = safe_logout_statuses(game.take_safe_logout_status_packets());
        assert_eq!(
            statuses.len(),
            1,
            "assault transition sends one rejection snapshot and the request is deduplicated"
        );
        assert_eq!(statuses[0].state, "online");
        assert_eq!(statuses[0].reason.as_deref(), Some("assault_active"));
        assert!(statuses[0].active_assault);
        assert!(!statuses[0].can_request);
        assert!(!statuses[0].protected);

        let units_after_request = game.crisis_assault_units();
        assert_eq!(
            units_after_request
                .iter()
                .map(|unit| (unit.obj_id, unit.assault_id, unit.spawn_generation, unit.hp))
                .collect::<Vec<_>>(),
            units_before
                .iter()
                .map(|unit| (unit.obj_id, unit.assault_id, unit.spawn_generation, unit.hp))
                .collect::<Vec<_>>()
        );

        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        let disconnected = game
            .settlement_crisis()
            .expect("assault survives ordinary disconnect");
        assert_eq!(disconnected.phase, CrisisPhase::AssaultActive);
        assert_eq!(disconnected.assault_id, Some(assault_id));
        assert_eq!(disconnected.assault_spawn_generation, generation);
        assert_eq!(
            game.crisis_assault_units()
                .iter()
                .map(|unit| (unit.obj_id, unit.assault_id, unit.spawn_generation, unit.hp))
                .collect::<Vec<_>>(),
            units_before
                .iter()
                .map(|unit| (unit.obj_id, unit.assault_id, unit.spawn_generation, unit.hp))
                .collect::<Vec<_>>()
        );

        game.tick(5);
        assert_eq!(
            game.settlement_crisis()
                .expect("offline active assault")
                .phase,
            CrisisPhase::AssaultActive
        );
        assert!(
            game.take_safe_logout_status_packets()
                .into_iter()
                .all(|packet| !matches!(
                    packet,
                    ResponsePacket::SafeLogoutStatus { status } if status.protected
                )),
            "active-assault rejection never publishes protected state"
        );
        assert_ne!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
    }

    #[test]
    fn safe_logout_checkpoint2_long_protection_freezes_owner_and_rebases_on_resume() {
        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutProtectedWorldBot");
        let player_id = game.player_id();
        let (hero_id, builder_id, refiner_id, structure_id, refiner_item_id) =
            prepare_checkpoint2_owner_workload(&mut game, sanctuary);
        let helper_player_id = spawn_connected_helper(&mut game, "SafeLogoutActiveNeighborBot");
        move_nearby_headless_hostiles_away(&mut game, sanctuary);

        game.complete_valid_safe_logout();
        let record = game.player_presence_record().unwrap();
        let protected_since = record.protected_since_tick.expect("protected start tick");
        {
            let world = game.app.world_mut();
            let entity_map = world.resource::<crate::ids::EntityObjMap>();
            let builder_entity = entity_map.get_entity(builder_id).expect("builder entity");
            let refiner_entity = entity_map.get_entity(refiner_id).expect("refiner entity");
            let structure_entity = entity_map
                .get_entity(structure_id)
                .expect("structure entity");
            let structure_pos = *world
                .get::<Position>(structure_entity)
                .expect("structure position");
            world.entity_mut(builder_entity).remove::<ThinkerBuilder>();
            world.entity_mut(refiner_entity).remove::<ThinkerBuilder>();
            world.entity_mut(builder_entity).insert((
                State::Building,
                Position {
                    x: structure_pos.x,
                    y: structure_pos.y,
                },
                Order::Build,
                ActiveTask::Building,
            ));
            world.entity_mut(refiner_entity).insert((
                State::Refining,
                Position {
                    x: structure_pos.x,
                    y: structure_pos.y,
                },
                Order::WorkQueue,
                ActiveTask::Refining,
            ));
            if !world.resource::<GameEvents>().values().any(|event| {
                matches!(
                    event.event_type,
                    GameEventType::StructureRefineEvent {
                        refiner_id: event_refiner,
                        structure_id: event_structure,
                        item_id: event_item,
                    } if event_refiner == refiner_id
                        && event_structure == structure_id
                        && event_item == refiner_item_id
                )
            }) {
                let event_id = world.resource_mut::<crate::ids::Ids>().new_map_event_id();
                world.resource_mut::<GameEvents>().insert(
                    event_id,
                    crate::event::GameEvent {
                        event_id,
                        start_tick: protected_since,
                        run_tick: protected_since + 1,
                        event_type: GameEventType::StructureRefineEvent {
                            refiner_id,
                            structure_id,
                            item_id: refiner_item_id,
                        },
                    },
                );
            }
        }
        for event in game
            .app
            .world_mut()
            .resource_mut::<GameEvents>()
            .values_mut()
        {
            if matches!(
                event.event_type,
                GameEventType::CraftEvent { .. }
                    | GameEventType::RefineEvent { .. }
                    | GameEventType::StructureRefineEvent { .. }
            ) {
                event.start_tick = protected_since;
                event.run_tick = protected_since + 1;
            }
        }
        let key = record.protected_run_key.clone().expect("protected run key");
        game.app
            .world_mut()
            .resource_mut::<LegendaryThreatState>()
            .insert(
                player_id,
                LegendaryThreat {
                    name: "Checkpoint 2 defeated threat".to_string(),
                    hideout_pos: sanctuary,
                    hideout_id: None,
                    boss_id: None,
                    rumor_sent: true,
                    active: false,
                    defeated: true,
                    hideout_revealed: true,
                    active_since_tick: Some(protected_since - 50),
                    defeated_at_tick: Some(protected_since - 20),
                    next_follower_tick: protected_since + 100,
                    waves_sent: 1,
                    follower_waves: Vec::new(),
                    followers_defeated: 0,
                    captains_defeated: 0,
                },
            );
        assert_eq!(key.player_id, game.player_id());
        assert_eq!(key.hero_id, hero_id);
        assert_eq!(
            game.world()
                .resource::<crate::ids::Ids>()
                .get_hero(game.player_id()),
            Some(key.hero_id)
        );

        game.disconnect_after_completed_safe_logout();
        let disconnected_record = game.player_presence_record().unwrap();
        game.disconnect_player();
        game.tick(2);
        assert_eq!(game.player_presence_record(), Some(disconnected_record));

        // Give a connected neighbor deterministic needs, crisis time, and a
        // real craft that becomes due during the protected interval. This
        // distinguishes owner-scoped freezing from a global pause.
        {
            let now = game.game_tick();
            let world = game.app.world_mut();
            let helper_hero_id = world
                .resource::<crate::ids::Ids>()
                .get_hero(helper_player_id)
                .expect("connected helper hero id");
            let helper_entity = world
                .resource::<crate::ids::EntityObjMap>()
                .get_entity(helper_hero_id)
                .expect("connected helper hero entity");
            world
                .get_mut::<Thirst>(helper_entity)
                .expect("helper thirst")
                .thirst = 11.0;
            world
                .get_mut::<Hunger>(helper_entity)
                .expect("helper hunger")
                .hunger = 13.0;
            world
                .get_mut::<Tired>(helper_entity)
                .expect("helper tiredness")
                .tired = 17.0;
            *world.get_mut::<State>(helper_entity).expect("helper state") = State::Crafting;

            let item_templates = world.resource::<Templates>().item_templates.clone();
            let log_id = world.resource_mut::<crate::ids::Ids>().new_item_id();
            world
                .get_mut::<Inventory>(helper_entity)
                .expect("helper inventory")
                .new(
                    log_id,
                    "Springbranch Maple Log".to_string(),
                    1,
                    &item_templates,
                );
            world
                .resource_mut::<PlayerIntroState>()
                .get_mut(&helper_player_id)
                .expect("helper introduction state")
                .danger_unlocked = true;
            let event_id = world.resource_mut::<crate::ids::Ids>().new_map_event_id();
            world.resource_mut::<GameEvents>().insert(
                event_id,
                GameEvent {
                    event_id,
                    start_tick: now,
                    run_tick: now + 50,
                    event_type: GameEventType::CraftEvent {
                        crafter_id: helper_hero_id,
                        recipe_name: "Firewood".to_string(),
                    },
                },
            );
        }

        install_checkpoint2_frozen_hero_state(&mut game, hero_id);
        let hero_before = game.protected_hero_snapshot();
        let villagers_before = game.protected_villager_snapshots();
        let structures_before = game.protected_structure_snapshots();
        let work_before = game.protected_work_deadlines();
        let crops_before = game.protected_crop_snapshots();
        let intro_before = game.protected_intro_snapshot();
        let resources_before = game.protected_stored_resource_quantity();
        let firewood_before = checkpoint2_hero_item_quantity(&mut game, "Firewood");
        let logs_before = checkpoint2_hero_item_quantity(&mut game, "Springbranch Maple Log");
        let crisis_before = game.settlement_crisis().expect("personal crisis");
        let helper_needs_before = checkpoint2_player_hero_needs(&mut game, helper_player_id);
        let helper_firewood_before =
            checkpoint2_player_hero_item_quantity(&mut game, helper_player_id, "Firewood");
        let helper_crisis_before = game
            .world()
            .resource::<SettlementCrisisState>()
            .get(&helper_player_id)
            .expect("helper personal crisis")
            .online_active_ticks;
        assert!(!villagers_before.is_empty());
        assert!(!structures_before.is_empty());
        assert!(!work_before.is_empty());
        assert!(work_before.iter().any(|event| event.kind == "craft"));
        assert!(
            work_before
                .iter()
                .any(|event| event.kind == "structure_refine"),
            "expected retained structure-refine deadline, got {work_before:?}"
        );
        assert!(!crops_before.is_empty());
        assert_eq!(logs_before, 1);
        assert!(checkpoint2_owned_structure_has_item(
            &mut game,
            refiner_item_id
        ));

        let world_tick_before = game.game_tick();
        game.advance_protected_world_ticks(10_000);
        assert!(game.game_tick() >= world_tick_before + 10_000);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        assert_eq!(game.protected_hero_snapshot(), hero_before);
        assert_eq!(game.protected_villager_snapshots(), villagers_before);
        assert_eq!(game.protected_structure_snapshots(), structures_before);
        assert_eq!(game.protected_work_deadlines(), work_before);
        assert_eq!(game.protected_crop_snapshots(), crops_before);
        assert_eq!(game.protected_intro_snapshot(), intro_before);
        assert_eq!(game.protected_stored_resource_quantity(), resources_before);
        assert_eq!(
            checkpoint2_hero_item_quantity(&mut game, "Firewood"),
            firewood_before
        );
        assert_eq!(
            checkpoint2_hero_item_quantity(&mut game, "Springbranch Maple Log"),
            logs_before
        );
        assert!(checkpoint2_owned_structure_has_item(
            &mut game,
            refiner_item_id
        ));
        assert_eq!(game.settlement_crisis(), Some(crisis_before.clone()));
        assert_ne!(
            checkpoint2_player_hero_needs(&mut game, helper_player_id),
            helper_needs_before,
            "the connected neighbor's needs must continue while this owner is protected"
        );
        assert!(
            checkpoint2_player_hero_item_quantity(&mut game, helper_player_id, "Firewood")
                > helper_firewood_before,
            "the connected neighbor's real craft must complete"
        );
        assert!(
            game.world()
                .resource::<SettlementCrisisState>()
                .get(&helper_player_id)
                .expect("continued helper crisis")
                .online_active_ticks
                > helper_crisis_before,
            "the connected neighbor's personal-crisis clock must continue"
        );
        assert_eq!(
            game.world()
                .resource::<PlayerWorldPresenceState>()
                .players
                .get(&helper_player_id)
                .map(|record| record.state),
            Some(PlayerWorldPresence::Online)
        );

        game.reconnect_and_exit_protection();
        let resumed_record = game.player_presence_record().unwrap();
        let resumed_tick = resumed_record
            .last_protection_end_tick
            .expect("resume boundary tick");
        let protected_duration = resumed_tick.saturating_sub(protected_since);
        assert!(protected_duration >= 10_000);
        assert_eq!(resumed_record.protected_since_tick, None);
        assert_eq!(resumed_record.protected_run_key, None);
        let mut expected_intro = intro_before.clone();
        expected_intro.start_tick = expected_intro.start_tick.saturating_add(protected_duration);
        expected_intro.first_rat_spawn_tick = expected_intro
            .first_rat_spawn_tick
            .saturating_add(protected_duration);
        expected_intro.second_rat_spawn_tick = expected_intro
            .second_rat_spawn_tick
            .saturating_add(protected_duration);
        expected_intro.villager_ready_tick = expected_intro
            .villager_ready_tick
            .saturating_add(protected_duration);
        expected_intro.phase1_unlock_tick = expected_intro
            .phase1_unlock_tick
            .saturating_add(protected_duration);
        expected_intro.spider_unlock_tick = expected_intro
            .spider_unlock_tick
            .saturating_add(protected_duration);
        assert_eq!(game.protected_intro_snapshot(), expected_intro);
        let legendary_resumed = game
            .world()
            .resource::<LegendaryThreatState>()
            .get(&player_id)
            .expect("rebased legendary threat");
        assert_eq!(
            legendary_resumed.active_since_tick,
            Some(protected_since - 50 + protected_duration)
        );
        assert_eq!(
            legendary_resumed.defeated_at_tick,
            Some(protected_since - 20 + protected_duration)
        );

        let hero_resumed = game.protected_hero_snapshot();
        let mut expected_hero = hero_before.clone();
        for deadline in &mut expected_hero.effect_deadlines {
            *deadline = deadline.saturating_add(protected_duration);
        }
        assert_eq!(hero_resumed, expected_hero);
        assert_eq!(game.protected_villager_snapshots(), villagers_before);

        let structures_resumed = game.protected_structure_snapshots();
        assert_eq!(structures_resumed.len(), structures_before.len());
        for (before, after) in structures_before.iter().zip(&structures_resumed) {
            assert_eq!(after.id, before.id);
            assert_eq!(after.hp, before.hp);
            assert_eq!(after.work_done, before.work_done);
            assert_eq!(after.queue_entries, before.queue_entries);
            assert_eq!(after.stored_quantity, before.stored_quantity);
            assert_eq!(
                after.work_start_tick,
                before
                    .work_start_tick
                    .map(|tick| tick.saturating_add(protected_duration))
            );
        }

        let work_resumed = game.protected_work_deadlines();
        assert_eq!(work_resumed.len(), work_before.len());
        for (before, after) in work_before.iter().zip(&work_resumed) {
            assert_eq!(after.event_id, before.event_id);
            assert_eq!(after.kind, before.kind);
            assert_eq!(
                after.start_tick,
                before.start_tick.saturating_add(protected_duration)
            );
            assert_eq!(
                after.run_tick,
                before.run_tick.saturating_add(protected_duration)
            );
        }

        let crops_resumed = game.protected_crop_snapshots();
        assert_eq!(crops_resumed.len(), crops_before.len());
        for (before, after) in crops_before.iter().zip(&crops_resumed) {
            assert_eq!(after.structure_id, before.structure_id);
            assert_eq!(after.stage, before.stage);
            assert_eq!(after.quantity, before.quantity);
            assert_eq!(
                after.stage_start,
                before.stage_start.saturating_add(protected_duration)
            );
            assert_eq!(
                after.stage_end,
                before.stage_end.saturating_add(protected_duration)
            );
        }

        let crisis_resumed = game.settlement_crisis().expect("resumed crisis");
        assert_eq!(crisis_resumed.phase, crisis_before.phase);
        assert_eq!(crisis_resumed.pressure, crisis_before.pressure);
        assert_eq!(
            crisis_resumed.online_active_ticks,
            crisis_before.online_active_ticks
        );
        assert_eq!(
            crisis_resumed.phase_online_ticks,
            crisis_before.phase_online_ticks
        );
        assert_eq!(
            crisis_resumed.phase_started_tick,
            crisis_before
                .phase_started_tick
                .saturating_add(protected_duration)
        );
        assert_eq!(
            crisis_resumed.last_evaluated_tick,
            crisis_before
                .last_evaluated_tick
                .saturating_add(protected_duration)
        );
        assert_ne!(crisis_resumed.phase, CrisisPhase::AssaultActive);

        // The next active interval resumes normal time-driven simulation from
        // the preserved values instead of processing the protected backlog.
        game.tick(40);
        assert!(game.protected_hero_snapshot().hp < hero_resumed.hp);
        assert!(
            checkpoint2_hero_item_quantity(&mut game, "Firewood") > firewood_before,
            "the retained real Firewood craft should complete only after resume"
        );
        assert_eq!(
            checkpoint2_hero_item_quantity(&mut game, "Springbranch Maple Log"),
            0
        );
        let refine_source_consumed =
            !checkpoint2_owned_structure_has_item(&mut game, refiner_item_id);
        let remaining_work = game.protected_work_deadlines();
        let villager_states = game.protected_villager_snapshots();
        assert!(
            refine_source_consumed,
            "the retained real structure refine should consume its source item after resume; remaining_work={remaining_work:?} villagers={villager_states:?}"
        );
        let structures_after_active_ticks = game.protected_structure_snapshots();
        assert!(structures_after_active_ticks
            .iter()
            .zip(&structures_resumed)
            .any(|(after, before)| after.work_done > before.work_done));
    }

    #[test]
    fn safe_logout_checkpoint2_queued_hostile_damage_is_purged_or_blocked() {
        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutQueuedDamageBot");
        let hostile_pos = far_map_position(&game, sanctuary);
        let hostile_id = game.spawn_safe_logout_test_hostile(hostile_pos);
        let player_id = game.player_id();
        let (_villager_entity, villager_id) =
            spawn_armed_owner_villager(&mut game, player_id, sanctuary);
        move_nearby_headless_hostiles_away(&mut game, sanctuary);
        let (structure_id, structure_pos) = {
            let player_id = game.player_id();
            let world = game.app.world_mut();
            let mut query =
                world.query_filtered::<(&Id, &PlayerId, &Position), With<ClassStructure>>();
            query
                .iter(world)
                .find(|(_, owner, _)| owner.0 == player_id)
                .map(|(id, _, pos)| (id.0, *pos))
                .expect("owned structure")
        };
        let queued_before_entry = game.queue_hostile_spell_damage_for_test(hostile_id, 300);

        game.complete_valid_safe_logout();
        assert!(!game
            .world()
            .resource::<MapEvents>()
            .contains_key(&queued_before_entry));
        game.disconnect_after_completed_safe_logout();

        let hp_before = game.protected_hero_snapshot().hp;
        let villagers_before = game.protected_villager_snapshots();
        let structures_before = game.protected_structure_snapshots();
        let score_before = {
            let score = game
                .world()
                .resource::<RunScoreState>()
                .get(&game.player_id())
                .expect("run score");
            (
                score.enemies_killed,
                score.elites_killed,
                score.captains_killed,
                score.legendary_kills,
                score.highest_pressure_level,
            )
        };
        let stale_event = game.queue_hostile_spell_damage_for_test(hostile_id, 0);
        let now = game.game_tick();
        let world = game.app.world_mut();
        world.resource_mut::<MapEvents>().new(
            hostile_id,
            now,
            VisibleEvent::SpellDamageEvent {
                spell: Spell::ShadowBolt,
                target_id: villager_id,
            },
        );
        world.resource_mut::<MapEvents>().new(
            hostile_id,
            now,
            VisibleEvent::SpellDamageEvent {
                spell: Spell::ShadowBolt,
                target_id: structure_id,
            },
        );
        world.resource_mut::<MapEvents>().new(
            hostile_id,
            now,
            VisibleEvent::StealEvent {
                target_id: structure_id,
                target_pos: structure_pos,
                item_types: vec!["Resource".to_string(), "Food".to_string()],
            },
        );
        world.resource_mut::<MapEvents>().new(
            hostile_id,
            now,
            VisibleEvent::SpoilEvent {
                target_id: structure_id,
                target_pos: structure_pos,
                item_type: "Resource".to_string(),
            },
        );
        world.resource_mut::<MapEvents>().new(
            hostile_id,
            now,
            VisibleEvent::TorchEvent {
                target_id: structure_id,
                target_pos: structure_pos,
            },
        );
        game.tick(3);
        assert_eq!(game.protected_hero_snapshot().hp, hp_before);
        assert_eq!(game.protected_villager_snapshots(), villagers_before);
        assert_eq!(game.protected_structure_snapshots(), structures_before);
        let score_after = {
            let score = game
                .world()
                .resource::<RunScoreState>()
                .get(&game.player_id())
                .expect("run score");
            (
                score.enemies_killed,
                score.elites_killed,
                score.captains_killed,
                score.legendary_kills,
                score.highest_pressure_level,
            )
        };
        assert_eq!(score_after, score_before);
        assert!(!game
            .world()
            .resource::<MapEvents>()
            .contains_key(&stale_event));
        assert!(game
            .world()
            .resource::<crate::ids::EntityObjMap>()
            .get_entity(hostile_id)
            .is_some());
    }

    #[test]
    fn safe_logout_checkpoint2_protected_inputs_cannot_move_or_attack() {
        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutInputGuardBot");
        let hostile_id = game.spawn_safe_logout_test_hostile(far_map_position(&game, sanctuary));
        let player_id = game.player_id();
        let (_villager_entity, villager_id) =
            spawn_armed_owner_villager(&mut game, player_id, sanctuary);
        move_nearby_headless_hostiles_away(&mut game, sanctuary);
        let structure_id = {
            let player_id = game.player_id();
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(&Id, &PlayerId), With<ClassStructure>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(id, _)| id.0)
                .expect("owned structure")
        };
        game.complete_valid_safe_logout();
        assert!(game
            .world()
            .resource::<Clients>()
            .is_player_online(game.player_id()));

        let hero = game.observe().hero.unwrap();
        let hero_snapshot = game.protected_hero_snapshot();
        let villager_snapshot = game.protected_villager_snapshots();
        let structure_snapshot = game.protected_structure_snapshots();
        let work_snapshot = game.protected_work_deadlines();
        game.attempt_player_mutation_while_protected(PlayerEvent::Move {
            player_id,
            x: move_one_tile(hero.pos).x,
            y: hero.pos.y,
        });
        game.attempt_player_mutation_while_protected(PlayerEvent::Attack {
            player_id,
            attack_type: "melee".to_string(),
            source_id: hero.id,
            target_id: hostile_id,
        });
        game.attempt_player_mutation_while_protected(PlayerEvent::Ability {
            player_id,
            ability_id: "Power Attack".to_string(),
            source_id: hero.id,
            target_id: Some(hostile_id),
        });
        game.attempt_player_mutation_while_protected(PlayerEvent::Combo {
            player_id,
            source_id: hero.id,
            target_id: hostile_id,
            combo_type: "power".to_string(),
        });
        game.attempt_player_mutation_while_protected(PlayerEvent::Craft {
            player_id,
            recipe_name: "Firewood".to_string(),
        });
        game.attempt_player_mutation_while_protected(PlayerEvent::Refine {
            player_id,
            item_id: -1,
        });
        game.attempt_player_mutation_while_protected(PlayerEvent::Assign {
            player_id,
            worker_id: villager_id,
            structure_id,
        });
        game.attempt_player_mutation_while_protected(PlayerEvent::Build {
            player_id,
            builder_id: villager_id,
            structure_id,
        });
        game.attempt_player_mutation_while_protected(PlayerEvent::CreateFoundation {
            player_id,
            source_id: hero.id,
            structure_name: "Campfire".to_string(),
        });
        game.attempt_player_mutation_while_protected(PlayerEvent::ItemTransfer {
            player_id,
            item_id: -1,
            source_id: hero.id,
            target_id: structure_id,
        });

        assert_eq!(game.observe().hero.unwrap().pos, hero.pos);
        assert_eq!(game.protected_hero_snapshot(), hero_snapshot);
        assert_eq!(game.protected_villager_snapshots(), villager_snapshot);
        assert_eq!(game.protected_structure_snapshots(), structure_snapshot);
        assert_eq!(game.protected_work_deadlines(), work_snapshot);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        assert!(!game.world().resource::<MapEvents>().values().any(|event| {
            event.obj_id == hero.id && matches!(event.event_type, VisibleEvent::MoveEvent { .. })
        }));
    }

    #[test]
    fn safe_logout_checkpoint2_global_time_visibility_and_world_packets_continue() {
        use crate::world::{
            create_weather_area, get_time_of_day, TimeOfDay, Weather, WeatherAreas,
        };

        let (mut game, _) = safe_logout_fixture("SafeLogoutEnvironmentBot");
        game.complete_valid_safe_logout();
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        let player_id = game.player_id();

        let mut first_light_tick = game.game_tick().div_euclid(GAME_TICKS_PER_DAY)
            * GAME_TICKS_PER_DAY
            + crate::constants::FIRST_LIGHT;
        if first_light_tick <= game.game_tick() {
            first_light_tick += GAME_TICKS_PER_DAY;
        }
        game.app.world_mut().resource_mut::<GameTick>().0 = first_light_tick - 1;
        let night_range = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(&PlayerId, &Viewshed), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(owner, _)| owner.0 == player_id)
                .map(|(_, viewshed)| viewshed.range)
                .unwrap()
        };
        assert_eq!(get_time_of_day(game.game_tick()), TimeOfDay::Night);

        game.start_packet_capture();
        game.tick(3);
        let packets = game.finish_packet_capture();
        assert_eq!(get_time_of_day(game.game_tick()), TimeOfDay::FirstLight);
        let first_light_range = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(&PlayerId, &Viewshed), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(owner, _)| owner.0 == player_id)
                .map(|(_, viewshed)| viewshed.range)
                .unwrap()
        };
        assert_ne!(first_light_range, night_range);
        assert!(packets.into_iter().any(|packet| matches!(
            packet,
            ResponsePacket::World {
                time_of_day,
                ..
            } if time_of_day == "First Light"
        )));

        game.app
            .world_mut()
            .resource_mut::<WeatherAreas>()
            .push(create_weather_area(-999, -999, Weather::HeavyRain));
        game.app.world_mut().resource_mut::<GameTick>().0 =
            first_light_tick + (crate::constants::DAWN - crate::constants::FIRST_LIGHT) - 1;
        game.tick(3);
        assert!(!game
            .world()
            .resource::<WeatherAreas>()
            .iter()
            .any(|area| area.center == (-999, -999)));
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
    }

    #[test]
    fn safe_logout_checkpoint2_assault_ready_cannot_launch_until_after_reconnect_barrier() {
        use crate::game::ASSAULT_READY_GRACE_TICKS;

        let (mut game, sanctuary) = safe_logout_fixture("SafeLogoutAssaultReadyBot");
        let preferred_tick = set_personal_assault_ready(&mut game);
        let player_id = game.player_id();
        move_nearby_headless_hostiles_away(&mut game, sanctuary);
        game.complete_valid_safe_logout();
        {
            let current_tick = game.game_tick();
            let world = game.app.world_mut();
            let mut crises = world.resource_mut::<SettlementCrisisState>();
            let crisis = crises.get_mut(&player_id).expect("ready crisis");
            crisis.phase_online_ticks = ASSAULT_READY_GRACE_TICKS - 1;
            crisis.last_evaluated_tick = current_tick;
        }

        game.app.world_mut().resource_mut::<GameTick>().0 = preferred_tick - 2;
        game.tick(2);
        assert_eq!(
            game.settlement_crisis()
                .expect("protected ready crisis")
                .phase,
            CrisisPhase::AssaultReady
        );
        assert!(game.crisis_assault_units().is_empty());

        game.reconnect_and_exit_protection();
        assert_eq!(
            game.settlement_crisis()
                .expect("reconnect ready crisis")
                .phase,
            CrisisPhase::AssaultReady,
            "the reconnect Update must still observe OfflineProtected"
        );
        assert!(game.crisis_assault_units().is_empty());

        game.tick(1);
        assert_eq!(
            game.settlement_crisis().expect("post-barrier crisis").phase,
            CrisisPhase::AssaultActive
        );
        assert!(!game.crisis_assault_units().is_empty());
    }

    #[test]
    fn safe_logout_checkpoint2_invalid_active_assault_revokes_protection_without_cleanup() {
        let (mut game, _) = safe_logout_fixture("SafeLogoutInvariantRecoveryBot");
        game.complete_valid_safe_logout();
        game.disconnect_after_completed_safe_logout();
        let player_id = game.player_id();
        {
            let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
            let crisis = crises.entry(player_id).or_default();
            crisis.phase = CrisisPhase::AssaultActive;
            crisis.assault_id = Some(7_777);
            crisis.assault_spawn_generation = 3;
            crisis.assault_unit_ids = vec![91, 92];
        }

        game.app.world_mut().run_schedule(First);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        let crisis = game.settlement_crisis().expect("preserved corrupt assault");
        assert_eq!(crisis.phase, CrisisPhase::AssaultActive);
        assert_eq!(crisis.assault_id, Some(7_777));
        assert_eq!(crisis.assault_spawn_generation, 3);
        assert_eq!(crisis.assault_unit_ids, vec![91, 92]);
    }

    #[test]
    fn safe_logout_checkpoint2_stale_run_key_fails_open_before_gameplay() {
        let (mut game, _) = safe_logout_fixture("SafeLogoutStaleRunKeyBot");
        game.complete_valid_safe_logout();
        game.disconnect_after_completed_safe_logout();
        {
            let player_id = game.player_id();
            let mut presence = game
                .app
                .world_mut()
                .resource_mut::<PlayerWorldPresenceState>();
            let record = presence
                .players
                .get_mut(&player_id)
                .expect("protected presence");
            record
                .protected_run_key
                .as_mut()
                .expect("protected run key")
                .hero_id += 10_000;
        }

        game.app.world_mut().run_schedule(First);
        let record = game.player_presence_record().expect("recovered presence");
        assert_eq!(record.state, PlayerWorldPresence::Disconnected);
        assert_eq!(record.protected_since_tick, None);
        assert_eq!(record.protected_run_key, None);
    }

    #[test]
    fn safe_logout_checkpoint2_bound_monolith_item_expiry_freezes_and_rebases() {
        use crate::ids::{EntityObjMap, Ids};
        use crate::templates::Templates;

        let (mut game, _) = safe_logout_fixture("SafeLogoutBoundInventoryBot");
        game.complete_valid_safe_logout();
        let protected_record = game.player_presence_record().expect("protected record");
        let item_duration = 50;
        let protected_since = protected_record
            .protected_since_tick
            .expect("protected start");
        let bound_monolith_id = protected_record
            .protected_run_key
            .expect("protected key")
            .bound_monolith_id;
        let item_id = {
            let world = game.app.world_mut();
            let monolith_entity = world
                .resource::<EntityObjMap>()
                .get_entity(bound_monolith_id)
                .expect("bound monolith entity");
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut inventory = world
                .get_mut::<Inventory>(monolith_entity)
                .expect("bound monolith inventory");
            inventory.new(item_id, "Crude Torch".to_string(), 1, &item_templates);
            let item = inventory
                .items
                .iter_mut()
                .find(|item| item.id == item_id)
                .expect("expiring test item");
            item.start_time = protected_since;
            item.attrs
                .insert(AttrKey::Duration, AttrVal::Num(item_duration as f32));
            item_id
        };

        game.disconnect_after_completed_safe_logout();
        game.advance_protected_world_ticks(500);
        let item_still_exists = {
            let world = game.world();
            let entity = world
                .resource::<EntityObjMap>()
                .get_entity(bound_monolith_id)
                .unwrap();
            world
                .get::<Inventory>(entity)
                .unwrap()
                .get_by_id(item_id)
                .is_some()
        };
        assert!(item_still_exists);

        game.reconnect_and_exit_protection();
        // Expiry is strict (`start + duration < tick`) and sampled once per
        // second, so include one complete sampling interval beyond duration.
        game.tick((item_duration + crate::constants::TICKS_PER_SEC + 1) as u32);
        let (item_after_active_time, active_tick) = {
            let world = game.world();
            let entity = world
                .resource::<EntityObjMap>()
                .get_entity(bound_monolith_id)
                .unwrap();
            (
                world.get::<Inventory>(entity).unwrap().get_by_id(item_id),
                world.resource::<GameTick>().0,
            )
        };
        assert!(
            item_after_active_time.is_none(),
            "bound-monolith item did not expire after resumed active time: game_tick={active_tick} start_time={:?}",
            item_after_active_time.map(|item| item.start_time)
        );
    }

    // One short capped game end-to-end. Must run with CWD = sp_server/ so the
    // templates/map/tileset files load by relative path.
    #[test]
    fn smoke() {
        let mut game = HeadlessGame::new(1_000);
        let pid = game.spawn_hero("Warrior", "SmokeBot");

        assert!(pid > 0, "player_id should be positive");

        // World built and hero present after spawn.
        let view = game.observe();
        assert!(view.hero.is_some(), "hero should exist after spawn_hero");
        assert!(
            !view.resource_tiles.is_empty(),
            "resource nodes should have spawned (world built)"
        );

        // Fast-forward; game time should advance.
        let before = game.game_tick();
        game.tick(100);
        let after = game.game_tick();
        assert!(after > before, "game tick should advance when pumping");
        assert!(game.ticks_pumped() > 0);

        // Metrics readable.
        let m = game.metrics();
        assert!(m.ticks >= 0);
    }

    #[test]
    fn observations_can_scope_primary_and_connected_helper_players() {
        let mut game = HeadlessGame::new(1_000);
        let primary_player_id = game.spawn_hero("Warrior", "PrimaryObserverBot");
        let helper_player_id = game.spawn_connected_scenario_helper("HelperObserverBot");

        let primary_wrapper = game.observe();
        let primary_scoped = game.observe_for_player(primary_player_id);
        let helper_scoped = game.observe_for_player(helper_player_id);

        assert_eq!(
            primary_wrapper.hero.map(|hero| hero.id),
            primary_scoped.hero.map(|hero| hero.id),
            "the legacy wrapper must remain scoped to the primary player"
        );
        assert_ne!(
            primary_scoped.hero.map(|hero| hero.id),
            helper_scoped.hero.map(|hero| hero.id),
            "each player-scoped observation must select its own hero"
        );
        assert!(!primary_scoped.structures.is_empty());
        assert!(!helper_scoped.structures.is_empty());
        assert!(primary_scoped.structures.iter().all(|primary| {
            helper_scoped
                .structures
                .iter()
                .all(|helper| helper.id != primary.id)
        }));
    }

    #[test]
    fn personal_crisis_mode_does_not_spawn_a_scheduled_dusk_horde() {
        let mut game = HeadlessGame::new(10_000);
        game.spawn_hero("Warrior", "PersonalDuskBot");
        prepare_for_scheduled_dusk(&mut game);

        assert_eq!(cross_scheduled_dusk(&mut game), 0);
    }

    #[test]
    fn legacy_mode_still_runs_the_scheduled_dusk_horde() {
        let mut game = HeadlessGame::new_with_director(10_000, SurvivalDirectorMode::Legacy);
        game.spawn_hero("Warrior", "LegacyDuskBot");

        // Legacy spawn placement samples wilderness outside the sanctuary. Try
        // several distinct dusks so random selection of blocked edge tiles
        // cannot make the director-registration regression flaky.
        let mut waves = 0;
        for day_offset in 3..=10 {
            prepare_for_scheduled_dusk_on_day(&mut game, day_offset);
            waves = cross_scheduled_dusk(&mut game);
            if waves > 0 {
                break;
            }
        }

        assert!(
            waves > 0,
            "legacy nightly_threat_system should schedule its dusk wave"
        );
        assert!(
            game.settlement_crisis().is_none(),
            "legacy mode must not activate the personal crisis state machine"
        );
    }

    #[test]
    fn personal_crisis_mode_preserves_the_introductory_encounter() {
        use crate::event::{GameEventType, GameEvents};
        use crate::game::{InitialEncounterState, IntroEncounterState, PlayerIntroState};

        let mut game = HeadlessGame::new(10_000);
        let player_id = game.spawn_hero("Warrior", "IntroBot");
        let entry = game
            .world()
            .resource::<InitialEncounterState>()
            .get(&player_id)
            .expect("initial encounter state")
            .clone();

        game.app
            .world_mut()
            .resource_mut::<Objectives>()
            .entry(player_id)
            .or_default()
            .scavenge_shipwreck = true;

        run_intro_check_at_or_after(&mut game, entry.second_rat_spawn_tick);

        assert!(
            game.world()
                .resource::<crate::game::PlayerIntroState>()
                .get(&player_id)
                .expect("player intro state")
                .shipwreck_chain_started,
            "the delayed opening encounter should start in personal-crisis mode"
        );
        let opening_deadline = entry.phase1_unlock_tick;
        mark_obj_ids_dead(&mut game, &entry.rat_ids, opening_deadline);
        run_intro_check_at_or_after(&mut game, opening_deadline);

        let phase1_id = game
            .world()
            .resource::<InitialEncounterState>()
            .get(&player_id)
            .and_then(|state| state.phase1_npc_id)
            .expect("boar/crab follow-up should spawn after opening enemies die");
        assert!(
            game.world()
                .resource::<IntroEncounterState>()
                .get(&player_id)
                .expect("separate intro encounter progress")
                .initial_encounter
        );

        mark_obj_ids_dead(&mut game, &[phase1_id], entry.spider_unlock_tick);
        run_intro_check_at_or_after(&mut game, entry.spider_unlock_tick);

        assert!(
            game.world()
                .resource::<IntroEncounterState>()
                .get(&player_id)
                .expect("separate intro encounter progress")
                .spider_encounter
        );
        let spider_exists = {
            let world = game.app.world_mut();
            let mut query = world.query::<(&Template, &Position, Option<&StateDead>)>();
            query.iter(world).any(|(template, pos, dead)| {
                template.0 == "Spider" && *pos == entry.spawn_pos && dead.is_none()
            })
        };
        assert!(
            spider_exists,
            "the Spider follow-up should be alive at the wreck"
        );

        assert!(
            game.world()
                .resource::<PlayerIntroState>()
                .get(&player_id)
                .expect("player intro state")
                .villager_spawned,
            "shipwreck inspection should still rescue the villager"
        );
        let villager_exists = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(&PlayerId, &State), With<SubclassVillager>>();
            query
                .iter(world)
                .any(|(owner, state)| owner.0 == player_id && state.is_alive())
        };
        assert!(villager_exists);
        assert!(game
            .world()
            .resource::<GameEvents>()
            .values()
            .any(|event| matches!(event.event_type, GameEventType::NecroEvent { .. })));

        assert!(matches!(
            entry.phase1_spawn.as_str(),
            "Wild Boar" | "Giant Crab"
        ));
        assert!(entry.phase1_unlock_tick < entry.spider_unlock_tick);
    }

    #[test]
    fn checkpoint4_true_death_clears_status_before_a_fresh_run() {
        use crate::game::{
            CrisisKind, CrisisPhase, InitialEncounterState, IntroEncounterState,
            PlayerIntroEncounters, PlayerIntroState, SettlementCrisis, SettlementCrisisState,
        };

        let mut game = HeadlessGame::new(10_000);
        let player_id = game.spawn_hero("Warrior", "FirstRunBot");
        let neighbor_id = player_id + 1;
        let current_tick = game.game_tick();

        {
            let world = game.app.world_mut();
            let mut intro = world.resource_mut::<IntroEncounterState>();
            intro.insert(
                player_id,
                PlayerIntroEncounters {
                    initial_encounter: true,
                    spider_encounter: true,
                },
            );
            intro.insert(
                neighbor_id,
                PlayerIntroEncounters {
                    initial_encounter: true,
                    spider_encounter: false,
                },
            );
        }
        {
            let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
            crises.insert(
                player_id,
                SettlementCrisis {
                    kind: CrisisKind::Goblin,
                    phase: CrisisPhase::AssaultActive,
                    pressure: 55,
                    phase_started_tick: current_tick,
                    online_active_ticks: 50,
                    phase_online_ticks: 50,
                    warning_active: true,
                    last_evaluated_tick: current_tick,
                    assault_id: Some(99),
                    assault_started_tick: Some(current_tick),
                    assault_unit_ids: vec![999_999],
                    assault_spawn_generation: 1,
                    ..SettlementCrisis::default()
                },
            );
            crises.insert(
                neighbor_id,
                SettlementCrisis {
                    kind: CrisisKind::Goblin,
                    phase: CrisisPhase::Signs,
                    pressure: 40,
                    phase_started_tick: current_tick,
                    online_active_ticks: 50,
                    phase_online_ticks: 50,
                    warning_active: false,
                    last_evaluated_tick: current_tick,
                    ..SettlementCrisis::default()
                },
            );
        }
        game.tick(1);
        let old_statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(
            old_statuses
                .last()
                .and_then(|status| status.phase.as_deref()),
            Some("assault_active")
        );
        assert_eq!(
            old_statuses
                .last()
                .and_then(|status| status.remaining_attackers),
            Some(1)
        );

        let hero = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(entity, _)| entity)
                .expect("first-run hero")
        };
        game.app.world_mut().entity_mut(hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Checkpoint cleanup".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick - (10 * crate::constants::TICKS_PER_SEC) - 1,
            },
        ));

        game.tick(3);
        let cleanup_statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(cleanup_statuses.len(), 1);
        assert!(!cleanup_statuses[0].exists);
        assert!(cleanup_statuses[0].phase.is_none());
        assert!(cleanup_statuses[0].pressure.is_none());
        assert!(cleanup_statuses[0].remaining_attackers.is_none());
        assert!(game
            .world()
            .resource::<IntroEncounterState>()
            .get(&player_id)
            .is_none());
        assert!(game
            .world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .is_none());
        assert!(game
            .world()
            .resource::<PlayerIntroState>()
            .get(&player_id)
            .is_none());
        assert!(game
            .world()
            .resource::<InitialEncounterState>()
            .get(&player_id)
            .is_none());
        assert!(game
            .world()
            .resource::<IntroEncounterState>()
            .contains_key(&neighbor_id));
        assert!(game
            .world()
            .resource::<SettlementCrisisState>()
            .contains_key(&neighbor_id));

        // Model an attributed orphan whose entity-map entry was already removed
        // by an overlapping cleanup. Fresh-run creation must still sweep it.
        let (stale_assault_entity, stale_assault_id) = {
            use crate::ids::Ids;

            let world = game.app.world_mut();
            let stale_id = world.resource_mut::<Ids>().new_obj_id();
            world
                .resource_mut::<Ids>()
                .new_obj(stale_id, crate::constants::NPC_PLAYER_ID);
            let entity = world
                .spawn((
                    Id(stale_id),
                    crate::game::CrisisAssaultUnit {
                        owner_player_id: player_id,
                        assault_id: 99,
                        spawn_generation: 1,
                    },
                ))
                .id();
            (entity, stale_id)
        };

        game.spawn_hero("Warrior", "FreshRunBot");
        let fresh_statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(fresh_statuses.len(), 1);
        assert!(fresh_statuses[0].exists);
        assert_eq!(fresh_statuses[0].phase.as_deref(), Some("dormant"));
        assert_eq!(fresh_statuses[0].pressure, Some(0));
        assert_eq!(fresh_statuses[0].remaining_attackers, None);
        assert!(game.world().get_entity(stale_assault_entity).is_err());
        assert!(!game
            .world()
            .resource::<crate::ids::Ids>()
            .obj_player_map
            .contains_key(&stale_assault_id));
        assert_eq!(
            game.intro_encounters(),
            Some(PlayerIntroEncounters::default())
        );
        let fresh_crisis = game
            .settlement_crisis()
            .expect("fresh run should lazily initialize a personal crisis");
        assert_eq!(fresh_crisis.phase, CrisisPhase::Dormant);
        assert_eq!(fresh_crisis.pressure, 0);
        assert_eq!(fresh_crisis.online_active_ticks, 0);
        assert_eq!(fresh_crisis.assault_id, None);
        assert!(fresh_crisis.assault_unit_ids.is_empty());
        assert!(fresh_crisis.assault_defeated_unit_ids.is_empty());
        assert_eq!(fresh_crisis.assault_spawn_generation, 0);
        assert!(!fresh_crisis.assault_recovery_required);
        assert!(!fresh_crisis.resolution_recorded);
    }

    #[test]
    fn intro_encounter_does_not_queue_spawns_for_a_true_death_owner() {
        use crate::game::InitialEncounterState;

        let mut game = HeadlessGame::new(10_000);
        let player_id = game.spawn_hero("Warrior", "IntroCleanupRaceBot");
        let current_tick = game.game_tick();
        let opening_ids = {
            let mut encounters = game.app.world_mut().resource_mut::<InitialEncounterState>();
            let encounter = encounters
                .get_mut(&player_id)
                .expect("initial encounter state");
            encounter.first_rat_spawn_tick = current_tick;
            encounter.second_rat_spawn_tick = current_tick;
            encounter.phase1_unlock_tick = current_tick;
            encounter.rat_ids.clone()
        };

        let hero = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(entity, _)| entity)
                .expect("hero")
        };
        game.app.world_mut().entity_mut(hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Checkpoint cleanup race".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick,
            },
        ));

        // Cross at least one 10-tick intro evaluation while remaining inside
        // the True Death system's cleanup delay.
        game.tick(20);

        let spawned_opening_enemy = {
            let world = game.app.world_mut();
            let mut query = world.query::<&Id>();
            query.iter(world).any(|id| opening_ids.contains(&id.0))
        };
        assert!(
            !spawned_opening_enemy,
            "deferred intro spawns must not escape a dying run's cleanup"
        );
    }

    #[test]
    fn headless_connection_helpers_leave_the_offline_hero_in_the_world() {
        let mut game = HeadlessGame::new(1_000);
        let player_id = game.spawn_hero("Warrior", "PresenceBot");
        assert!(game
            .world()
            .resource::<Clients>()
            .is_player_online(player_id));

        game.disconnect_player();
        assert!(!game
            .world()
            .resource::<Clients>()
            .is_player_online(player_id));
        assert!(
            game.observe().hero.is_some(),
            "hero existence must not be treated as online presence"
        );

        game.reconnect_player();
        assert!(game
            .world()
            .resource::<Clients>()
            .is_player_online(player_id));
    }

    #[test]
    fn checkpoint2_short_personal_crisis_simulation() {
        use crate::constants::TICKS_PER_SEC;
        use crate::game::{
            CrisisPhase, InitialEncounterState, PlayerIntroState, SettlementCrisisState,
        };

        let mut game = HeadlessGame::new(10_000);
        let player_id = game.spawn_hero("Warrior", "CrisisFoundationBot");
        game.set_sanctuary_at_base(5);

        {
            let world = game.app.world_mut();
            world
                .resource_mut::<PlayerIntroState>()
                .get_mut(&player_id)
                .expect("player intro state")
                .danger_unlocked = true;
            {
                let mut objectives = world.resource_mut::<Objectives>();
                let objective = objectives.entry(player_id).or_default();
                objective.explore_poi = true;
                objective.choose_expansion = true;
            }
            world
                .resource_mut::<InitialEncounterState>()
                .remove(&player_id);

            // Existing-world facts only: completed owned structures and a
            // living recruited villager. These test entities carry exactly the
            // components read by the crisis evaluator.
            for _ in 0..3 {
                world.spawn((PlayerId(player_id), State::None, ClassStructure));
            }
            world.spawn((PlayerId(player_id), State::None, SubclassVillager));
        }

        game.tick(1);
        assert_eq!(game.settlement_crisis().unwrap().phase, CrisisPhase::Signs);

        let next_tick = game.game_tick() + (60 * TICKS_PER_SEC);
        game.app.world_mut().resource_mut::<GameTick>().0 = next_tick;
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Pressure
        );

        let next_tick = game.game_tick() + (120 * TICKS_PER_SEC);
        game.app.world_mut().resource_mut::<GameTick>().0 = next_tick;
        game.tick(1);
        let preparing = game.settlement_crisis().unwrap();
        assert_eq!(preparing.phase, CrisisPhase::Preparing);
        assert!(preparing.warning_active);

        let enemies_before_ready = game
            .observe()
            .enemies
            .into_iter()
            .map(|enemy| enemy.id)
            .collect::<std::collections::HashSet<_>>();
        let next_tick = game.game_tick() + (180 * TICKS_PER_SEC);
        game.app.world_mut().resource_mut::<GameTick>().0 = next_tick;
        game.tick(1);
        let final_crisis = game
            .settlement_crisis()
            .expect("personal crisis after controlled simulation");
        let enemies_after_ready = game
            .observe()
            .enemies
            .into_iter()
            .map(|enemy| enemy.id)
            .collect::<std::collections::HashSet<_>>();
        let automatic_dusk_hordes = game.metrics().waves_survived;

        assert_eq!(final_crisis.phase, CrisisPhase::AssaultReady);
        assert_eq!(enemies_after_ready, enemies_before_ready);
        assert_eq!(automatic_dusk_hordes, 0);
        assert!(game
            .world()
            .resource::<SettlementCrisisState>()
            .contains_key(&player_id));

        println!(
            "checkpoint2_headless highest_phase={:?} online_active_ticks={} final_pressure={} major_assault_entities_spawned=0 automatic_dusk_hordes_spawned={}",
            final_crisis.phase,
            final_crisis.online_active_ticks,
            final_crisis.pressure,
            automatic_dusk_hordes
        );
    }

    #[test]
    fn checkpoint4_normal_packet_progression_and_runtime_telemetry_headless() {
        use crate::constants::TICKS_PER_SEC;

        let mut game = HeadlessGame::new(30_000);
        game.spawn_hero("Warrior", "CrisisPacketProgressionBot");
        prepare_full_crisis_progression_facts(&mut game);

        game.tick(1);
        for seconds in [60, 120, 180] {
            game.app.world_mut().resource_mut::<GameTick>().0 =
                game.game_tick() + seconds * TICKS_PER_SEC;
            game.tick(1);
        }

        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultReady
        );
        let preferred_tick = next_preferred_assault_tick(game.game_tick());
        {
            use crate::game::ASSAULT_READY_GRACE_TICKS;

            let mut crises = game.app.world_mut().resource_mut::<SettlementCrisisState>();
            let crisis = crises.get_mut(&game.player_id).unwrap();
            crisis.phase_online_ticks = 0;
            crisis.last_evaluated_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;
        }
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched_units = game.crisis_assault_units();
        assert_eq!(launched_units.len(), GOBLIN_ASSAULT_COMPOSITION.len());

        for unit in launched_units {
            kill_assault_unit_through_normal_combat(&mut game, unit.obj_id);
        }
        game.tick(2);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );

        let statuses = crisis_statuses(game.take_crisis_status_packets());
        let phases = crisis_phase_sequence(&statuses);
        assert_eq!(
            phases,
            vec![
                "dormant",
                "signs",
                "pressure",
                "preparing",
                "assault_ready",
                "assault_active",
                "resolved",
            ]
        );
        let remaining = statuses
            .iter()
            .filter(|status| status.assault_active)
            .filter_map(|status| status.remaining_attackers)
            .collect::<Vec<_>>();
        assert!(remaining.starts_with(&[GOBLIN_ASSAULT_COMPOSITION.len() as i32]));
        assert!(remaining.windows(2).all(|counts| counts[1] <= counts[0]));
        assert!(remaining
            .iter()
            .any(|remaining| *remaining < GOBLIN_ASSAULT_COMPOSITION.len() as i32));
        assert_eq!(statuses.iter().filter(|status| status.resolved).count(), 1);

        let telemetry = game.crisis_telemetry();
        assert_eq!(telemetry.highest_phase, "resolved");
        assert!(telemetry.signs_tick.is_some());
        assert!(telemetry.pressure_tick.is_some());
        assert!(telemetry.preparing_tick.is_some());
        assert!(telemetry.assault_ready_tick.is_some());
        assert!(telemetry.assault_active_tick.is_some());
        assert!(telemetry.resolved_tick.is_some());
        assert_eq!(telemetry.assaults_launched, 1);
        assert_eq!(telemetry.assaults_resolved, 1);
        assert_eq!(telemetry.units_remaining, 0);
        assert_eq!(telemetry.status_packets_sent as usize, statuses.len());
        assert_eq!(telemetry.login_snapshots_sent, 1);
        assert_eq!(telemetry.duplicate_assaults, 0);

        let metrics = game.metrics();
        assert_eq!(metrics.crisis_highest_phase, "resolved");
        assert_eq!(metrics.crisis_final_phase, "resolved");
        assert_eq!(metrics.crisis_assaults_launched, 1);
        assert_eq!(metrics.crisis_assaults_resolved, 1);
        assert_eq!(metrics.personal_crisis_automatic_dusk_hordes, 0);
        assert!(metrics.crisis_invariants_ok);
        assert_eq!(
            metrics
                .crisis_balance
                .assault_outcome
                .assault_units_defeated,
            GOBLIN_ASSAULT_COMPOSITION.len() as i32,
            "normal Combat damage must reach the crisis attribution observer"
        );
        assert_eq!(
            metrics.crisis_balance.assault_outcome.player_kills,
            GOBLIN_ASSAULT_COMPOSITION.len() as i32
        );
        assert_eq!(metrics.crisis_balance.assault_outcome.helper_kills, 0);

        println!(
            "checkpoint4_normal_packet_progression phases={phases:?} remaining_attackers={remaining:?} status_packets={} login_snapshots={} duplicate_assaults={}",
            telemetry.status_packets_sent,
            telemetry.login_snapshots_sent,
            telemetry.duplicate_assaults,
        );
    }

    #[test]
    fn checkpoint4_active_disconnect_reconnect_sends_one_current_snapshot() {
        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "CrisisReconnectPacketBot");
        game.set_crisis_balance_sample_interval(Some(600));
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let _ = game.take_crisis_status_packets();

        let units_before = game.crisis_assault_units();
        game.disconnect_player();
        game.tick(8);
        let units_offline = game.crisis_assault_units();
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive
        );
        assert_eq!(
            units_offline
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<Vec<_>>(),
            units_before
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<Vec<_>>()
        );

        game.start_packet_capture();
        game.reconnect_player_with_login();
        game.tick(8);
        let reconnect_packets = game.finish_packet_capture();
        let repeated_launch_notice = reconnect_packets.iter().any(|packet| {
            matches!(
                packet,
                ResponsePacket::Notice { noticemsg, .. }
                    if noticemsg == "The goblin assault has begun. It will continue if you disconnect."
            )
        });
        assert!(!repeated_launch_notice);

        let statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.phase.as_deref(), Some("assault_active"));
        assert!(status.assault_active);
        assert!(status.continues_while_disconnected);
        assert_eq!(
            status.remaining_attackers,
            Some(units_offline.iter().filter(|unit| !unit.dead).count() as i32)
        );

        let reconnected = game.settlement_crisis().unwrap();
        let units_reconnected = game.crisis_assault_units();
        assert_eq!(reconnected.assault_id, Some(assault_id));
        assert_eq!(reconnected.assault_spawn_generation, generation);
        assert_eq!(
            units_reconnected
                .iter()
                .map(|unit| (unit.obj_id, unit.hp))
                .collect::<Vec<_>>(),
            units_offline
                .iter()
                .map(|unit| (unit.obj_id, unit.hp))
                .collect::<Vec<_>>()
        );
        assert_eq!(game.crisis_telemetry().login_snapshots_sent, 2);
        assert!(
            game.crisis_balance_telemetry()
                .assault_outcome
                .reconnected_during_assault
        );
    }

    #[test]
    fn checkpoint4_offline_resolution_reconnect_first_snapshot_is_resolved() {
        let mut game = HeadlessGame::new(30_000);
        game.spawn_hero("Warrior", "CrisisOfflineResolutionPacketBot");
        game.set_crisis_balance_sample_interval(Some(600));
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let helper_player_id = spawn_connected_helper(&mut game, "CrisisResolutionHelper");
        let unit_ids = game
            .crisis_assault_units()
            .iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        let _ = game.take_crisis_status_packets();

        game.disconnect_player();
        game.tick(2);
        for unit_id in unit_ids {
            kill_assault_unit_through_normal_combat_as(&mut game, helper_player_id, unit_id);
        }
        game.tick(2);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        assert!(game.take_crisis_status_packets().is_empty());
        let offline_balance = game.crisis_balance_telemetry();
        let resolution_snapshot_tick = offline_balance
            .preparation_snapshots
            .resolution_or_end
            .as_ref()
            .map(|snapshot| snapshot.game_tick)
            .expect("first resolution snapshot");
        let offline_outcome = offline_balance.assault_outcome;
        assert!(offline_outcome.ordinary_disconnect_during_assault);
        assert!(offline_outcome.resolved_while_owner_offline);
        assert_eq!(offline_outcome.hero_alive_at_resolution, Some(true));
        assert_eq!(
            offline_outcome.helper_kills,
            GOBLIN_ASSAULT_COMPOSITION.len() as i32
        );

        game.reconnect_player_with_login();
        game.tick(8);
        let statuses = crisis_statuses(game.take_crisis_status_packets());
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].phase.as_deref(), Some("resolved"));
        assert!(statuses[0].resolved);
        assert!(!statuses[0].assault_active);
        assert_eq!(game.crisis_telemetry().assaults_resolved, 1);
        assert_eq!(
            game.crisis_balance_telemetry()
                .preparation_snapshots
                .resolution_or_end
                .as_ref()
                .map(|snapshot| snapshot.game_tick),
            Some(resolution_snapshot_tick)
        );
        assert!(
            !game
                .crisis_balance_telemetry()
                .assault_outcome
                .reconnected_during_assault
        );
    }

    #[test]
    fn checkpoint3_normal_victory_headless() {
        use crate::game::{CrisisPhase, GOBLIN_ASSAULT_COMPOSITION};
        use crate::ids::Ids;
        use crate::player_setup::RunSpawnedObjs;

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultVictoryBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);

        let launched = game.settlement_crisis().expect("active assault");
        let assault_id = launched.assault_id.expect("logical assault id");
        assert_eq!(launched.assault_spawn_generation, 1);
        assert_eq!(
            launched.assault_unit_ids.len(),
            GOBLIN_ASSAULT_COMPOSITION.len()
        );
        assert!(!launched.resolution_recorded);

        let units = game.crisis_assault_units();
        assert_eq!(units.len(), GOBLIN_ASSAULT_COMPOSITION.len());
        let mut actual_templates = units
            .iter()
            .map(|unit| unit.template.clone())
            .collect::<Vec<_>>();
        let mut expected_templates = GOBLIN_ASSAULT_COMPOSITION
            .iter()
            .map(|template| (*template).to_string())
            .collect::<Vec<_>>();
        actual_templates.sort();
        expected_templates.sort();
        assert_eq!(actual_templates, expected_templates);
        assert!(units.iter().all(|unit| {
            unit.owner_player_id == player_id
                && unit.assault_id == assault_id
                && unit.spawn_generation == 1
                && !unit.dead
        }));
        for unit in &units {
            let entity = game
                .world()
                .resource::<crate::ids::EntityObjMap>()
                .get_entity(unit.obj_id)
                .expect("attributed assault entity");
            assert!(game
                .world()
                .get::<crate::common::TaskTarget>(entity)
                .is_none());
            assert!(game
                .world()
                .get::<crate::npc::ItemsToSteal>(entity)
                .is_none());
        }
        let run_ids = game
            .world()
            .resource::<RunSpawnedObjs>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        assert!(units.iter().all(|unit| run_ids.contains(&unit.obj_id)));

        // Neither an ordinary legacy goblin nor a normally dead unit attributed
        // to another owner can advance this settlement's logical remainder.
        let current_tick = game.game_tick();
        {
            let world = game.app.world_mut();
            let unrelated_id = world.resource_mut::<Ids>().new_obj_id();
            world.spawn((
                Id(unrelated_id),
                Template("Wolf Rider".to_string()),
                State::Dead,
                StateDead {
                    dead_at: current_tick,
                    killer: "Unrelated".to_string(),
                },
            ));
            let other_owner_id = world.resource_mut::<Ids>().new_obj_id();
            world.spawn((
                Id(other_owner_id),
                Template("Goblin Pillager".to_string()),
                State::Dead,
                StateDead {
                    dead_at: current_tick,
                    killer: "Other owner".to_string(),
                },
                CrisisAssaultUnit {
                    owner_player_id: player_id + 1,
                    assault_id: assault_id + 1,
                    spawn_generation: 1,
                },
            ));
        }
        game.tick(1);
        assert_eq!(
            game.settlement_crisis()
                .unwrap()
                .assault_defeated_unit_ids
                .len(),
            0
        );

        // Force two schedule evaluations to observe the exact same GameTick.
        // Active bookkeeping and generation identity must remain idempotent.
        let repeated_tick = game.game_tick() + 1;
        let launched_ids = game
            .crisis_assault_units()
            .iter()
            .filter(|unit| unit.owner_player_id == player_id)
            .map(|unit| unit.obj_id)
            .collect::<HashSet<_>>();
        for _ in 0..2 {
            game.app.world_mut().resource_mut::<GameTick>().0 = repeated_tick - 1;
            game.tick(1);
            assert_eq!(game.game_tick(), repeated_tick);
            assert_eq!(
                game.settlement_crisis().unwrap().assault_spawn_generation,
                1
            );
            assert_eq!(
                game.crisis_assault_units()
                    .iter()
                    .filter(|unit| unit.owner_player_id == player_id)
                    .map(|unit| unit.obj_id)
                    .collect::<HashSet<_>>(),
                launched_ids
            );
        }

        game.tick(5);
        assert_eq!(
            game.crisis_assault_units()
                .iter()
                .filter(|unit| unit.owner_player_id == player_id)
                .count(),
            GOBLIN_ASSAULT_COMPOSITION.len()
        );
        assert_eq!(
            game.settlement_crisis().unwrap().assault_spawn_generation,
            1,
            "active evaluation must not duplicate the generation"
        );

        let unit_ids = units.iter().map(|unit| unit.obj_id).collect::<Vec<_>>();
        kill_assault_unit_through_normal_combat(&mut game, unit_ids[0]);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive,
            "a partial normal defeat cannot resolve the assault"
        );
        let hero_entity = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<Entity, With<SubclassHero>>();
            query.iter(world).next().unwrap()
        };
        game.app
            .world_mut()
            .entity_mut(hero_entity)
            .insert(crate::common::Target {
                id: *unit_ids.last().unwrap(),
            });
        for unit_id in unit_ids.iter().skip(1) {
            kill_assault_unit_through_normal_combat(&mut game, *unit_id);
        }
        game.tick(1);

        let resolved = game.settlement_crisis().expect("resolved crisis state");
        assert_eq!(resolved.phase, CrisisPhase::Resolved);
        assert!(resolved.resolution_recorded);
        assert!(resolved.resolved_at_tick.is_some());
        assert!(!resolved.warning_active);
        assert!(resolved.assault_unit_ids.is_empty());
        assert_eq!(game.personal_crises_resolved(), 1);
        assert!(game
            .world()
            .get::<crate::common::Target>(hero_entity)
            .is_none());
        assert_eq!(game.metrics().waves_survived, 0);

        game.tick(20);
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            game.settlement_crisis().unwrap().assault_spawn_generation,
            1
        );

        println!(
            "checkpoint3_normal_victory assault_id={} generation={} templates={:?} units={} resolution_count={} automatic_dusk_hordes={}",
            assault_id,
            resolved.assault_spawn_generation,
            actual_templates,
            units.len(),
            game.personal_crises_resolved(),
            game.metrics().waves_survived
        );
    }

    #[test]
    fn undead_crisis_checkpoint1_smoke_normal_sequence_stops_at_assault_ready() {
        use crate::game::{
            CrisisKind, CrisisPhase, CrisisTelemetryState, NextCrisisAssaultId,
            PersonalCrisisHistory, SettlementCrisisState, NEXT_PERSONAL_CRISIS_DELAY_TICKS,
            UNDEAD_PREPARING_MIN_ONLINE_TICKS, UNDEAD_PRESSURE_MIN_ONLINE_TICKS,
            UNDEAD_SIGNS_MIN_ONLINE_TICKS,
        };

        let mut game = HeadlessGame::new(60_000);
        let player_id = game.spawn_hero("Warrior", "UndeadSequenceSmokeBot");
        let goblin_assault_id = resolve_goblin_normally_for_undead_smoke(&mut game);
        assert!(game
            .world()
            .resource::<PersonalCrisisHistory>()
            .by_player
            .get(&player_id)
            .is_some_and(|history| history.completed.contains(&CrisisKind::Goblin)));

        let resolved_online_ticks = game
            .settlement_crisis()
            .expect("resolved Goblin clock")
            .phase_online_ticks;
        let ticks_to_boundary = NEXT_PERSONAL_CRISIS_DELAY_TICKS - resolved_online_ticks;
        assert!(ticks_to_boundary >= 2);
        game.app.world_mut().resource_mut::<GameTick>().0 += ticks_to_boundary - 2;
        game.tick(1);
        let before_boundary = game.settlement_crisis().expect("resolved delay holder");
        assert_eq!(before_boundary.kind, CrisisKind::Goblin);
        assert_eq!(before_boundary.phase, CrisisPhase::Resolved);
        assert_eq!(
            before_boundary.phase_online_ticks,
            NEXT_PERSONAL_CRISIS_DELAY_TICKS - 1
        );

        game.tick(1);
        let dormant = game.settlement_crisis().expect("new Undead crisis");
        assert_eq!(dormant.kind, CrisisKind::Undead);
        assert_eq!(dormant.phase, CrisisPhase::Dormant);
        assert_eq!(dormant.pressure, 0);
        assert_eq!(dormant.assault_id, None);
        let next_id_after_goblin = game.world().resource::<NextCrisisAssaultId>().next_value();
        assert_eq!(next_id_after_goblin, goblin_assault_id + 1);
        let goblin_runtime_telemetry = game
            .world()
            .resource::<CrisisTelemetryState>()
            .get(&player_id)
            .cloned()
            .expect("Goblin runtime telemetry");
        let goblin_balance_telemetry = game
            .world()
            .resource::<CrisisBalanceTelemetryState>()
            .get(&player_id)
            .cloned()
            .expect("Goblin balance telemetry");

        game.tick(1);
        assert_eq!(game.settlement_crisis().unwrap().phase, CrisisPhase::Signs);
        for (minimum_ticks, expected_phase) in [
            (UNDEAD_SIGNS_MIN_ONLINE_TICKS, CrisisPhase::Pressure),
            (UNDEAD_PRESSURE_MIN_ONLINE_TICKS, CrisisPhase::Preparing),
            (UNDEAD_PREPARING_MIN_ONLINE_TICKS, CrisisPhase::AssaultReady),
        ] {
            game.app.world_mut().resource_mut::<GameTick>().0 += minimum_ticks - 1;
            game.tick(1);
            assert_eq!(game.settlement_crisis().unwrap().phase, expected_phase);
        }

        let ready = game.settlement_crisis().expect("terminal Undead Ready");
        assert_eq!(ready.kind, CrisisKind::Undead);
        assert_eq!(ready.phase, CrisisPhase::AssaultReady);
        assert_eq!(ready.pressure, 80);
        assert_eq!(ready.assault_id, None);
        assert_eq!(ready.assault_spawn_generation, 0);
        assert!(ready.assault_unit_ids.is_empty());
        assert!(game.crisis_assault_units().is_empty());
        assert_eq!(
            game.personal_crises_resolved(),
            1,
            "Undead Ready must not grant another crisis completion reward"
        );
        assert_eq!(
            game.world().resource::<NextCrisisAssaultId>().next_value(),
            next_id_after_goblin,
            "Undead Ready must not allocate a Goblin assault identity"
        );
        assert_eq!(
            game.world()
                .resource::<CrisisTelemetryState>()
                .get(&player_id)
                .unwrap(),
            &goblin_runtime_telemetry
        );
        assert_eq!(
            game.world()
                .resource::<CrisisBalanceTelemetryState>()
                .get(&player_id)
                .unwrap(),
            &goblin_balance_telemetry
        );

        let statuses = crisis_statuses(game.take_crisis_status_packets());
        let ready_status = statuses
            .iter()
            .find(|status| {
                status.kind.as_deref() == Some("undead")
                    && status.phase.as_deref() == Some("assault_ready")
            })
            .expect("Undead Ready status packet");
        assert_eq!(
            ready_status.title.as_deref(),
            Some("Undead Incursion Imminent")
        );
        assert!(ready_status.preparation_seconds_remaining.is_some());
        assert_eq!(
            ready_status.preferred_launch_window.as_deref(),
            Some("dusk_or_night")
        );
        assert!(!ready_status.continues_while_disconnected);
        assert!(!statuses
            .iter()
            .any(|status| status.kind.as_deref() == Some("undead") && status.assault_active));
    }

    #[test]
    fn undead_crisis_checkpoint1_smoke_safe_logout_freeze_cleanup_and_fresh_run() {
        use crate::game::{
            CrisisKind, CrisisPhase, PersonalCrisisHistory, PlayerCrisisHistory, SettlementCrisis,
            SettlementCrisisState, NEXT_PERSONAL_CRISIS_DELAY_TICKS,
        };

        let mut game = HeadlessGame::new(60_000);
        let player_id = game.spawn_hero("Warrior", "UndeadProtectionSmokeBot");
        resolve_goblin_normally_for_undead_smoke(&mut game);

        let initial_delay_ticks = game.settlement_crisis().unwrap().phase_online_ticks;
        game.app.world_mut().resource_mut::<GameTick>().0 += 99;
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase_online_ticks,
            initial_delay_ticks + 100
        );

        let sanctuary = place_player_in_own_bound_sanctuary(&mut game, player_id);
        move_nearby_headless_hostiles_away(&mut game, sanctuary);
        expire_safe_logout_activity_cooldown(&mut game);
        game.tick(1);
        game.complete_valid_safe_logout();
        let reusable_protection = game
            .player_presence_record()
            .expect("production-created protection record");
        assert_eq!(
            reusable_protection.state,
            PlayerWorldPresence::OfflineProtected
        );
        let frozen = game.settlement_crisis().expect("protected resolved crisis");
        assert_eq!(frozen.kind, CrisisKind::Goblin);
        assert_eq!(frozen.phase, CrisisPhase::Resolved);
        assert!(frozen.phase_online_ticks < NEXT_PERSONAL_CRISIS_DELAY_TICKS);
        let frozen_ticks = frozen.phase_online_ticks;

        game.disconnect_after_completed_safe_logout();
        game.advance_protected_world_ticks(2_000);
        assert_eq!(
            game.settlement_crisis().unwrap().phase_online_ticks,
            frozen_ticks,
            "Offline Protection must freeze the inter-crisis online clock"
        );
        game.reconnect_and_exit_protection();
        assert_eq!(
            game.settlement_crisis().unwrap().phase_online_ticks,
            frozen_ticks,
            "resume rebasing must not backfill protected time"
        );

        let remaining = NEXT_PERSONAL_CRISIS_DELAY_TICKS - frozen_ticks;
        assert!(remaining > 0);
        game.app.world_mut().resource_mut::<GameTick>().0 += remaining - 1;
        game.tick(1);
        let undead = game
            .settlement_crisis()
            .expect("Undead after resumed delay");
        assert_eq!(undead.kind, CrisisKind::Undead);
        assert_eq!(undead.phase, CrisisPhase::Dormant);

        // Reapply the valid protection identity created by the real Safe
        // Logout above to this same run after Undead exists. The shared
        // eligibility test separately exercises every pre-assault phase for
        // both kinds; here the production schedule proves the freeze itself.
        game.tick(1);
        game.disconnect_player();
        let mut undead_protection = reusable_protection;
        undead_protection.protected_since_tick = Some(game.game_tick());
        undead_protection.client_connected = false;
        undead_protection.safe_logout_connection_ids.clear();
        game.app
            .world_mut()
            .resource_mut::<PlayerWorldPresenceState>()
            .players
            .insert(player_id, undead_protection);
        let protected_undead = game
            .settlement_crisis()
            .expect("protected pre-assault Undead")
            .clone();
        assert_eq!(protected_undead.kind, CrisisKind::Undead);
        assert!(protected_undead.phase < CrisisPhase::AssaultActive);
        let protected_history = game
            .world()
            .resource::<PersonalCrisisHistory>()
            .by_player
            .get(&player_id)
            .cloned()
            .expect("protected current-run history");

        game.advance_protected_world_ticks(2_000);
        let still_protected = game
            .settlement_crisis()
            .expect("Undead remains during Offline Protection");
        assert_eq!(still_protected.kind, protected_undead.kind);
        assert_eq!(still_protected.phase, protected_undead.phase);
        assert_eq!(still_protected.pressure, protected_undead.pressure);
        assert_eq!(
            still_protected.online_active_ticks,
            protected_undead.online_active_ticks
        );
        assert_eq!(
            still_protected.phase_online_ticks,
            protected_undead.phase_online_ticks
        );
        assert_eq!(
            game.world()
                .resource::<PersonalCrisisHistory>()
                .by_player
                .get(&player_id),
            Some(&protected_history)
        );
        game.reconnect_and_exit_protection();
        let resumed_undead = game
            .settlement_crisis()
            .expect("resumed pre-assault Undead");
        assert_eq!(resumed_undead.phase, protected_undead.phase);
        assert_eq!(resumed_undead.pressure, protected_undead.pressure);
        assert_eq!(
            resumed_undead.online_active_ticks,
            protected_undead.online_active_ticks
        );
        assert_eq!(
            resumed_undead.phase_online_ticks,
            protected_undead.phase_online_ticks
        );

        let neighbor_id = player_id + 1;
        let mut neighbor_history = PlayerCrisisHistory::default();
        neighbor_history.completed.insert(CrisisKind::Goblin);
        game.app
            .world_mut()
            .resource_mut::<PersonalCrisisHistory>()
            .by_player
            .insert(neighbor_id, neighbor_history.clone());
        let hero = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(_, owner)| owner.0 == player_id)
                .map(|(entity, _)| entity)
                .expect("current hero")
        };
        let current_tick = game.game_tick();
        game.app.world_mut().entity_mut(hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Undead checkpoint cleanup".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick - (10 * crate::constants::TICKS_PER_SEC) - 1,
            },
        ));
        game.tick(3);
        assert!(game
            .world()
            .resource::<SettlementCrisisState>()
            .get(&player_id)
            .is_none());
        let history = game.world().resource::<PersonalCrisisHistory>();
        assert!(!history.by_player.contains_key(&player_id));
        assert_eq!(history.by_player.get(&neighbor_id), Some(&neighbor_history));

        game.disconnect_player();
        let mut stale_history = PlayerCrisisHistory::default();
        stale_history.completed.insert(CrisisKind::Goblin);
        stale_history.completed.insert(CrisisKind::Undead);
        game.app
            .world_mut()
            .resource_mut::<PersonalCrisisHistory>()
            .by_player
            .insert(player_id, stale_history);
        let mut stale_crisis = SettlementCrisis::new(CrisisKind::Undead, game.game_tick());
        stale_crisis.phase = CrisisPhase::AssaultReady;
        game.app
            .world_mut()
            .resource_mut::<SettlementCrisisState>()
            .insert(player_id, stale_crisis);
        game.spawn_hero("Warrior", "UndeadFreshRunSmokeBot");
        let fresh = game.settlement_crisis().expect("fresh-run Goblin crisis");
        assert_eq!(fresh.kind, CrisisKind::Goblin);
        assert_eq!(fresh.phase, CrisisPhase::Dormant);
        assert!(game
            .world()
            .resource::<PersonalCrisisHistory>()
            .by_player
            .get(&player_id)
            .map(|history| history.completed.is_empty())
            .unwrap_or(true));
        assert_eq!(
            game.world()
                .resource::<PersonalCrisisHistory>()
                .by_player
                .get(&neighbor_id),
            Some(&neighbor_history)
        );
    }

    #[test]
    fn undead_crisis_checkpoint2_smoke_full_lifecycle_and_raise_dead() {
        use crate::game::{CrisisKind, CrisisPhase, PersonalCrisisHistory};

        let mut game = HeadlessGame::new(120_000);
        let player_id = game.spawn_hero("Warrior", "UndeadLifecycleSmokeBot");
        let (assault_id, generation, initial_ids) = launch_undead_after_completed_goblin(&mut game);
        assert_fixed_undead_assault(&mut game, assault_id, generation);

        let active_statuses = crisis_statuses(game.take_crisis_status_packets());
        let active_status = active_statuses
            .iter()
            .find(|status| {
                status.kind.as_deref() == Some("undead")
                    && status.phase.as_deref() == Some("assault_active")
            })
            .expect("Undead AssaultActive status");
        assert_eq!(active_status.remaining_attackers, Some(6));
        assert!(active_status.assault_active);
        assert!(active_status.continues_while_disconnected);
        assert_eq!(
            active_status.action_hint.as_deref(),
            Some("Defeat the remaining undead. This assault continues if you disconnect.")
        );

        let (corpse_id, raised_id, _) = trigger_same_assault_raise_dead(&mut game);
        assert!(initial_ids.contains(&corpse_id));
        assert!(!initial_ids.contains(&raised_id));
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive,
            "a raised unit remains part of the unresolved assault"
        );

        for original_id in initial_ids.iter().copied() {
            if game
                .crisis_assault_units()
                .iter()
                .any(|unit| unit.obj_id == original_id && !unit.dead)
            {
                kill_assault_unit_through_normal_combat(&mut game, original_id);
            }
        }
        game.tick(2);
        let active_with_only_raised = game
            .settlement_crisis()
            .expect("raised unit keeps Undead assault active");
        assert_eq!(active_with_only_raised.phase, CrisisPhase::AssaultActive);
        assert!(initial_ids.iter().all(|id| active_with_only_raised
            .assault_defeated_unit_ids
            .contains(id)));
        assert_eq!(
            game.crisis_assault_units()
                .into_iter()
                .filter(|unit| !unit.dead)
                .map(|unit| unit.obj_id)
                .collect::<Vec<_>>(),
            vec![raised_id]
        );
        let one_remaining_status = crisis_statuses(game.take_crisis_status_packets())
            .into_iter()
            .rev()
            .find(|status| {
                status.kind.as_deref() == Some("undead")
                    && status.phase.as_deref() == Some("assault_active")
            })
            .expect("active status while only the raised unit remains");
        assert_eq!(one_remaining_status.remaining_attackers, Some(1));

        kill_assault_unit_through_normal_combat(&mut game, raised_id);
        game.tick(2);
        let resolved = game.settlement_crisis().expect("resolved Undead crisis");
        assert_eq!(resolved.kind, CrisisKind::Undead);
        assert_eq!(resolved.phase, CrisisPhase::Resolved);
        assert!(resolved.resolution_recorded);
        assert_eq!(resolved.assault_id, Some(assault_id));
        assert_eq!(resolved.assault_spawn_generation, generation);
        assert_eq!(game.personal_crises_resolved(), 2);
        let history = game
            .world()
            .resource::<PersonalCrisisHistory>()
            .by_player
            .get(&player_id)
            .cloned()
            .expect("two-kind personal crisis history");
        assert_eq!(history.completed.len(), 2);
        assert!(history.completed.contains(&CrisisKind::Goblin));
        assert!(history.completed.contains(&CrisisKind::Undead));

        let resolved_statuses = crisis_statuses(game.take_crisis_status_packets());
        assert!(resolved_statuses.iter().any(|status| {
            status.kind.as_deref() == Some("undead") && status.phase.as_deref() == Some("resolved")
        }));
        let stable = resolved.clone();
        game.tick(20);
        let after = game.settlement_crisis().expect("terminal Undead holder");
        assert_eq!(after.kind, CrisisKind::Undead);
        assert_eq!(after.phase, CrisisPhase::Resolved);
        assert_eq!(after.resolved_at_tick, stable.resolved_at_tick);
        assert_eq!(after.pressure, stable.pressure);
        assert_eq!(after.phase_online_ticks, stable.phase_online_ticks);
        assert_eq!(game.personal_crises_resolved(), 2);
        assert_eq!(
            game.world().resource::<PersonalCrisisHistory>().by_player[&player_id],
            history
        );
    }

    #[test]
    fn undead_crisis_checkpoint2_smoke_disconnect_reconnect_and_helper() {
        use crate::game::{CrisisKind, CrisisPhase, PersonalCrisisHistory, RunScoreState};

        let mut game = HeadlessGame::new(120_000);
        let owner_player_id = game.spawn_hero("Warrior", "UndeadOfflineOwnerSmokeBot");
        let (assault_id, generation, _) = launch_undead_after_completed_goblin(&mut game);
        let (_, raised_id, necromancer_id) = trigger_same_assault_raise_dead(&mut game);
        let committed_ids = game
            .settlement_crisis()
            .unwrap()
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        assert!(committed_ids.contains(&raised_id));
        let before_disconnect = game
            .crisis_assault_units()
            .into_iter()
            .map(|unit| (unit.obj_id, (unit.hp, unit.dead)))
            .collect::<BTreeMap<_, _>>();

        let helper_player_id = spawn_connected_helper(&mut game, "UndeadHelperSmokeBot");
        game.prepare_established_scenario_helper(helper_player_id);
        let helper_score_before = game
            .world()
            .resource::<RunScoreState>()
            .get(&helper_player_id)
            .cloned()
            .unwrap_or_default();
        game.disconnect_player();
        game.tick(20);

        let offline = game
            .settlement_crisis()
            .expect("offline active Undead assault");
        assert_eq!(offline.kind, CrisisKind::Undead);
        assert_eq!(offline.phase, CrisisPhase::AssaultActive);
        assert_eq!(offline.assault_id, Some(assault_id));
        assert_eq!(offline.assault_spawn_generation, generation);
        assert_eq!(
            offline
                .assault_unit_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            committed_ids
        );
        let after_disconnect = game
            .crisis_assault_units()
            .into_iter()
            .map(|unit| (unit.obj_id, (unit.hp, unit.dead)))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(after_disconnect, before_disconnect);

        kill_assault_unit_through_normal_combat_as(&mut game, helper_player_id, necromancer_id);
        let helper_score_after_kill = game
            .world()
            .resource::<RunScoreState>()
            .get(&helper_player_id)
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            helper_score_after_kill.enemies_killed,
            helper_score_before.enemies_killed + 1
        );
        assert_eq!(helper_score_after_kill.personal_crises_resolved, 0);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive
        );

        game.reconnect_player_with_login();
        game.tick(3);
        let reconnected = game
            .settlement_crisis()
            .expect("reconnected Undead assault");
        assert_eq!(reconnected.assault_id, Some(assault_id));
        assert_eq!(reconnected.assault_spawn_generation, generation);
        assert_eq!(
            reconnected
                .assault_unit_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            committed_ids
        );
        assert!(game
            .crisis_assault_units()
            .iter()
            .all(|unit| unit.owner_player_id == owner_player_id));

        defeat_remaining_undead_after_necromancer(&mut game, necromancer_id);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 2);
        assert!(game
            .world()
            .resource::<PersonalCrisisHistory>()
            .by_player
            .get(&owner_player_id)
            .is_some_and(|history| history.completed.contains(&CrisisKind::Undead)));
        assert_eq!(
            game.world()
                .resource::<RunScoreState>()
                .get(&helper_player_id)
                .map(|score| score.personal_crises_resolved)
                .unwrap_or_default(),
            0,
            "the connected helper can fight but cannot own the crisis reward"
        );
    }

    #[test]
    fn undead_crisis_checkpoint2_smoke_isolation_and_true_death_cleanup() {
        use crate::event::{
            EventExecuting, EventExecutingState, MapEvent, VisibleEvent, VisibleEvents,
        };
        use crate::game::{
            CrisisKind, CrisisPhase, Minions, PersonalCrisisHistory, RunScoreState,
            SettlementCrisisState,
        };
        use crate::ids::{EntityObjMap, Ids};
        use crate::network::ChangeEvents;
        use crate::npc::VisibleTarget;
        use crate::player_setup::RunSpawnedObjs;

        let mut game = HeadlessGame::new(120_000);
        let owner_player_id = game.spawn_hero("Warrior", "UndeadIsolationSmokeBot");
        let (assault_id, generation, _) = launch_undead_after_completed_goblin(&mut game);
        let helper_player_id = spawn_connected_helper(&mut game, "UndeadNeighborSmokeBot");
        game.prepare_established_scenario_helper(helper_player_id);
        let (neighbor_villager_entity, neighbor_villager_id) = game
            .app
            .world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      mut ids: ResMut<Ids>,
                      mut entity_map: ResMut<EntityObjMap>,
                      templates: Res<Templates>,
                      game_tick: Res<GameTick>| {
                    Encounter::spawn_villager(
                        helper_player_id,
                        Position { x: 1, y: 1 },
                        Vec::new(),
                        &mut commands,
                        &mut ids,
                        &mut entity_map,
                        &templates,
                        &game_tick,
                    )
                },
            )
            .expect("ordinary neighboring villager spawn");
        let neighbor_villager_id = neighbor_villager_id.0;
        game.tick(1);

        let units = game.crisis_assault_units();
        let necromancer_id = units
            .iter()
            .find(|unit| unit.template == "Necromancer")
            .map(|unit| unit.obj_id)
            .expect("fixed-composition Necromancer");
        let (necromancer_entity, neighbor_hero_id, neighbor_hero_entity) = {
            let world = game.app.world_mut();
            let entity_map = world.resource::<EntityObjMap>();
            let necromancer_entity = entity_map
                .get_entity(necromancer_id)
                .expect("Necromancer entity");
            let mut heroes = world.query_filtered::<(Entity, &Id, &PlayerId), With<SubclassHero>>();
            let (neighbor_hero_entity, neighbor_hero_id, _) = heroes
                .iter(world)
                .find(|(_, _, owner)| owner.0 == helper_player_id)
                .expect("neighbor hero");
            (necromancer_entity, neighbor_hero_id.0, neighbor_hero_entity)
        };
        let necromancer_pos = *game
            .world()
            .get::<Position>(necromancer_entity)
            .expect("Necromancer position");
        {
            let tick = game.game_tick();
            let world = game.app.world_mut();
            *world
                .get_mut::<Position>(neighbor_villager_entity)
                .expect("neighbor villager position") = necromancer_pos;
            *world
                .get_mut::<State>(neighbor_villager_entity)
                .expect("neighbor villager state") = State::Dead;
            world
                .entity_mut(neighbor_villager_entity)
                .insert(StateDead {
                    dead_at: tick,
                    killer: "Undead isolation fixture".to_string(),
                });
            *world
                .get_mut::<Position>(neighbor_hero_entity)
                .expect("neighbor hero position") = necromancer_pos;
            world
                .get_mut::<VisibleTarget>(necromancer_entity)
                .expect("Necromancer visible target")
                .target = crate::constants::NO_TARGET;
            world
                .get_mut::<TaskTarget>(necromancer_entity)
                .expect("Necromancer task target")
                .target = crate::constants::NO_TARGET;
            world.entity_mut(necromancer_entity).remove::<Target>();
            world
                .get_mut::<EventExecuting>(necromancer_entity)
                .expect("Necromancer event state")
                .state = EventExecutingState::None;
        }

        let next_scorer_tick = ((game.game_tick() / 10) + 1) * 10;
        game.app.world_mut().resource_mut::<GameTick>().0 = next_scorer_tick - 1;
        game.tick(2);
        let necromancer = game
            .crisis_assault_units()
            .into_iter()
            .find(|unit| unit.obj_id == necromancer_id)
            .expect("Necromancer after isolation scoring");
        for selected in [
            necromancer.visible_target,
            necromancer.target,
            necromancer.task_target,
        ] {
            assert_ne!(selected, Some(neighbor_hero_id));
            assert_ne!(selected, Some(neighbor_villager_id));
        }
        assert!(game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(neighbor_villager_id)
            .is_some());

        let (_, raised_id, raised_by_necromancer_id) = trigger_same_assault_raise_dead(&mut game);
        assert_eq!(raised_by_necromancer_id, necromancer_id);
        let necromancer_entity = game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(necromancer_id)
            .expect("Necromancer after same-assault raise");
        let minions = game
            .world()
            .get::<Minions>(necromancer_entity)
            .expect("Necromancer Minions");
        assert!(minions.ids.contains(&raised_id));
        assert!(!minions.ids.contains(&neighbor_villager_id));
        assert!(game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(neighbor_villager_id)
            .is_some());

        let committed = game.settlement_crisis().unwrap();
        assert_eq!(committed.phase, CrisisPhase::AssaultActive);
        assert_eq!(committed.assault_id, Some(assault_id));
        assert_eq!(committed.assault_spawn_generation, generation);
        assert!(committed.assault_unit_ids.contains(&raised_id));
        let committed_ids = committed.assault_unit_ids.clone();
        let score_before_cleanup = game.personal_crises_resolved();
        assert_eq!(score_before_cleanup, 1);

        // Queue a second production Raise Dead for the same update in which
        // the owning run reaches True Death. Cleanup must win that race before
        // the spell can allocate or commit a replacement unit.
        let race_corpse_id = game
            .crisis_assault_units()
            .into_iter()
            .find(|unit| unit.template == "Zombie" && !unit.dead && unit.obj_id != raised_id)
            .map(|unit| unit.obj_id)
            .expect("second same-assault Zombie corpse source");
        kill_assault_unit_through_normal_combat(&mut game, race_corpse_id);
        let (necromancer_entity, race_corpse_pos) = {
            let world = game.app.world();
            let entity_map = world.resource::<EntityObjMap>();
            let necromancer_entity = entity_map
                .get_entity(necromancer_id)
                .expect("Necromancer before cleanup race");
            let race_corpse_entity = entity_map
                .get_entity(race_corpse_id)
                .expect("same-assault corpse before cleanup race");
            let race_corpse_pos = *world
                .get::<Position>(race_corpse_entity)
                .expect("same-assault corpse position");
            (necromancer_entity, race_corpse_pos)
        };
        {
            let world = game.app.world_mut();
            *world
                .get_mut::<Position>(necromancer_entity)
                .expect("Necromancer position") = race_corpse_pos;
            *world
                .get_mut::<Position>(neighbor_hero_entity)
                .expect("neighbor observer position") = race_corpse_pos;
            world
                .get_mut::<Viewshed>(neighbor_hero_entity)
                .expect("neighbor observer viewshed")
                .range = 100;
            *world
                .get_mut::<State>(necromancer_entity)
                .expect("Necromancer state") = State::Casting;
            world
                .get_mut::<EventExecuting>(necromancer_entity)
                .expect("Necromancer event state")
                .state = EventExecutingState::Executing;
        }

        let queued_event_id = Uuid::from_u128(8_008_008_008);
        let current_tick = game.game_tick();
        game.app.world_mut().resource_mut::<MapEvents>().insert(
            queued_event_id,
            MapEvent {
                event_id: queued_event_id,
                obj_id: necromancer_id,
                run_tick: current_tick - 1,
                event_type: VisibleEvent::SpellRaiseDeadEvent {
                    corpse_id: race_corpse_id,
                },
            },
        );
        let (other_generation_entity, other_generation_id) = {
            let world = game.app.world_mut();
            let other_generation_id = world.resource_mut::<Ids>().new_obj_id();
            let other_generation_entity = world
                .spawn((
                    Id(other_generation_id),
                    PlayerId(crate::constants::NPC_PLAYER_ID),
                    Position { x: 40, y: 40 },
                    State::None,
                    CrisisAssaultUnit {
                        owner_player_id,
                        assault_id,
                        spawn_generation: generation + 1,
                    },
                ))
                .id();
            world
                .resource_mut::<Ids>()
                .new_obj(other_generation_id, crate::constants::NPC_PLAYER_ID);
            world
                .resource_mut::<EntityObjMap>()
                .new_obj(other_generation_id, other_generation_entity);
            world
                .resource_mut::<RunSpawnedObjs>()
                .entry(owner_player_id)
                .or_default()
                .push(other_generation_id);
            (other_generation_entity, other_generation_id)
        };
        let next_obj_id_before = game.world().resource::<Ids>().obj;

        let owner_hero = {
            let world = game.app.world_mut();
            let mut heroes = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
            heroes
                .iter(world)
                .find(|(_, owner)| owner.0 == owner_player_id)
                .map(|(entity, _)| entity)
                .expect("owner hero")
        };
        let current_tick = game.game_tick();
        game.app.world_mut().entity_mut(owner_hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Undead cleanup fixture".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick - (10 * crate::constants::TICKS_PER_SEC) - 1,
            },
        ));
        game.start_packet_capture();
        game.tick(1);

        assert_eq!(
            game.world().resource::<Ids>().obj,
            next_obj_id_before,
            "the queued Raise Dead event must not allocate a replacement identity"
        );
        assert!(game
            .world()
            .resource::<MapEvents>()
            .get(&queued_event_id)
            .is_none());
        assert!(game
            .world()
            .resource::<SettlementCrisisState>()
            .get(&owner_player_id)
            .is_none());
        assert!(!game
            .world()
            .resource::<PersonalCrisisHistory>()
            .by_player
            .get(&owner_player_id)
            .is_some_and(|history| history.completed.contains(&CrisisKind::Undead)));
        assert!(game
            .world()
            .resource::<RunScoreState>()
            .get(&owner_player_id)
            .is_none());
        assert!(game
            .world()
            .resource::<RunSpawnedObjs>()
            .get(&owner_player_id)
            .is_none());
        assert!(committed_ids.iter().all(|id| game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(*id)
            .is_none()));
        let matching_units_after_cleanup = {
            let world = game.app.world_mut();
            let mut query = world.query::<&CrisisAssaultUnit>();
            query
                .iter(world)
                .filter(|unit| {
                    unit.owner_player_id == owner_player_id
                        && unit.assault_id == assault_id
                        && unit.spawn_generation == generation
                })
                .count()
        };
        assert_eq!(matching_units_after_cleanup, 0);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert_eq!(
            game.world()
                .resource::<EntityObjMap>()
                .get_entity(other_generation_id),
            Some(other_generation_entity),
            "same-owner attribution from another generation must survive exact assault cleanup"
        );

        let pending_remove_ids = game
            .world()
            .resource::<VisibleEvents>()
            .iter()
            .filter_map(|event| match event.event_type {
                VisibleEvent::RemoveObjEvent { .. } => Some(event.obj_id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        assert!(
            pending_remove_ids.contains(&necromancer_id),
            "same-update cleanup must queue canonical RemoveObj visibility for the Necromancer; pending={pending_remove_ids:?}"
        );
        assert!(
            pending_remove_ids.contains(&race_corpse_id),
            "same-update cleanup must queue canonical RemoveObj visibility for the corpse source; pending={pending_remove_ids:?}"
        );
        assert!(!pending_remove_ids.contains(&other_generation_id));

        // Visibility currently runs before True Death cleanup, so the queued
        // canonical events are delivered on the following production update.
        game.tick(1);
        let packets = game.finish_packet_capture();
        let deleted_ids = packets
            .into_iter()
            .filter_map(|packet| match packet {
                ResponsePacket::PerceptionChanges { events } => Some(events),
                _ => None,
            })
            .flatten()
            .filter_map(|event| match event {
                ChangeEvents::ObjDelete { obj_id, .. } => Some(obj_id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        assert!(
            deleted_ids.contains(&necromancer_id),
            "canonical RemoveObj visibility must announce the cleaned Necromancer; deleted={deleted_ids:?}"
        );
        assert!(
            deleted_ids.contains(&race_corpse_id),
            "canonical RemoveObj visibility must announce the cleaned corpse source; deleted={deleted_ids:?}"
        );
        assert!(game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(neighbor_villager_id)
            .is_some());
        assert!(game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(neighbor_hero_id)
            .is_some());

        game.disconnect_player();
        game.spawn_hero("Warrior", "UndeadFreshRunAfterCleanupSmokeBot");
        let fresh = game.settlement_crisis().expect("fresh-run Goblin crisis");
        assert_eq!(fresh.kind, CrisisKind::Goblin);
        assert_eq!(fresh.phase, CrisisPhase::Dormant);
        assert!(game
            .world()
            .resource::<PersonalCrisisHistory>()
            .by_player
            .get(&owner_player_id)
            .map(|history| history.completed.is_empty())
            .unwrap_or(true));
        assert!(game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(other_generation_id)
            .is_none());
        assert!(
            game.world().get_entity(other_generation_entity).is_err(),
            "the owner-scoped fresh-run orphan sweep must remove a stale generation"
        );
    }

    #[test]
    fn checkpoint3_ready_clock_pauses_offline_and_resumes_on_reconnect() {
        use crate::game::{CrisisPhase, ASSAULT_READY_GRACE_TICKS};

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultPresenceBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        let ready_tick = preferred_tick - ASSAULT_READY_GRACE_TICKS;

        game.app.world_mut().resource_mut::<GameTick>().0 = ready_tick + 9;
        game.tick(1);
        assert_eq!(game.settlement_crisis().unwrap().phase_online_ticks, 10);

        game.disconnect_player();
        game.app.world_mut().resource_mut::<GameTick>().0 += 5_000;
        game.tick(1);
        let offline = game.settlement_crisis().unwrap();
        assert_eq!(offline.phase, CrisisPhase::AssaultReady);
        assert_eq!(offline.phase_online_ticks, 10);
        assert!(game.crisis_assault_units().is_empty());

        let reconnect_preferred = next_preferred_assault_tick(game.game_tick());
        let remaining_online_ticks = ASSAULT_READY_GRACE_TICKS - 10;
        {
            let world = game.app.world_mut();
            world.resource_mut::<GameTick>().0 = reconnect_preferred - 1;
            let mut crises = world.resource_mut::<SettlementCrisisState>();
            crises
                .get_mut(&game.player_id)
                .expect("ready crisis")
                .last_evaluated_tick = reconnect_preferred - remaining_online_ticks;
        }
        game.reconnect_player();
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive
        );
        assert_eq!(
            game.crisis_assault_units().len(),
            GOBLIN_ASSAULT_COMPOSITION.len()
        );
    }

    #[test]
    fn checkpoint3_missing_anchor_stays_ready_without_consuming_an_assault_id() {
        use crate::game::{BoundMonolith, CrisisPhase, SpawnPositions};

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultNoAnchorBot");
        let preferred_tick = set_personal_assault_ready(&mut game);

        {
            let world = game.app.world_mut();
            let hero = {
                let mut query = world.query_filtered::<(Entity, &PlayerId), With<SubclassHero>>();
                query
                    .iter(world)
                    .find(|(_, owner)| owner.0 == player_id)
                    .map(|(entity, _)| entity)
                    .expect("owner hero")
            };
            world.entity_mut(hero).remove::<BoundMonolith>();

            let structures = {
                let mut query = world.query_filtered::<(Entity, &PlayerId), With<ClassStructure>>();
                query
                    .iter(world)
                    .filter(|(_, owner)| owner.0 == player_id)
                    .map(|(entity, _)| entity)
                    .collect::<Vec<_>>()
            };
            for entity in structures {
                *world.get_mut::<State>(entity).expect("structure state") = State::Building;
            }
            world.resource_mut::<SpawnPositions>().remove(&player_id);
            world.resource_mut::<GameTick>().0 = preferred_tick - 2;
        }

        game.tick(2);

        let crisis = game.settlement_crisis().expect("ready crisis remains");
        assert_eq!(crisis.phase, CrisisPhase::AssaultReady);
        assert_eq!(crisis.assault_id, None);
        assert_eq!(crisis.assault_spawn_generation, 0);
        assert!(crisis.assault_unit_ids.is_empty());
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_spawn_failure_stays_ready_without_consuming_a_generation() {
        use crate::game::CrisisPhase;
        use crate::map::TileType;

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultNoSpawnBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);

        {
            let world = game.app.world_mut();
            for tile in &mut world.resource_mut::<Map>().base {
                tile.tile_type = TileType::Ocean;
            }
            world.resource_mut::<GameTick>().0 = preferred_tick - 2;
        }
        game.tick(2);

        let crisis = game.settlement_crisis().expect("ready crisis remains");
        assert_eq!(crisis.phase, CrisisPhase::AssaultReady);
        assert_eq!(crisis.assault_id, None);
        assert_eq!(crisis.assault_spawn_generation, 0);
        assert!(crisis.assault_unit_ids.is_empty());
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_attributed_npc_attack_continues_after_owner_disconnect() {
        use big_brain::prelude::{ActionState, Actor};

        use crate::common::AttackTarget;
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultOfflineDamageBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let unit_ids = launched
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let unit_id = game.crisis_assault_units()[0].obj_id;

        let (hero_entity, hero_id, hero_pos, hp_before, unit_entity) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(Entity, &Id, &Position, &Stats), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_pos, hero_stats) =
                hero_query.iter(world).next().expect("owner hero");
            let unit_entity = world
                .resource::<EntityObjMap>()
                .get_entity(unit_id)
                .expect("assault attacker");
            (
                hero_entity,
                hero_id.0,
                *hero_pos,
                hero_stats.hp,
                unit_entity,
            )
        };
        let attacker_pos = passable_unoccupied_adjacent_position(&mut game, hero_pos);
        let action_entity = {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(unit_entity).unwrap() = attacker_pos;
            world.get_mut::<Stats>(unit_entity).unwrap().base_damage = Some(30);
            world.get_mut::<VisibleTarget>(unit_entity).unwrap().target = hero_id;
            world
                .spawn((Actor(unit_entity), ActionState::Requested, AttackTarget))
                .id()
        };
        let attributed_damage_before = game
            .crisis_balance_telemetry()
            .assault_outcome
            .hero_damage_taken;

        game.disconnect_player();
        game.tick(1);

        let hp_after = game.world().get::<Stats>(hero_entity).unwrap().hp;
        assert!(
            hp_after < hp_before,
            "attributed NPC combat must apply damage while its owner is offline"
        );
        assert_eq!(
            game.crisis_balance_telemetry()
                .assault_outcome
                .hero_damage_taken
                .saturating_sub(attributed_damage_before),
            hp_before.saturating_sub(hp_after),
            "the normal attributed NPC attack hook must record exact effective hero damage"
        );
        assert!(game.world().get::<StateDead>(hero_entity).is_none());
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive
        );
        assert_eq!(
            game.settlement_crisis().unwrap().assault_id,
            Some(assault_id)
        );
        assert_eq!(
            game.settlement_crisis().unwrap().assault_spawn_generation,
            generation
        );
        assert_eq!(
            game.crisis_assault_units()
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<HashSet<_>>(),
            unit_ids
        );
        assert_eq!(
            *game.world().get::<ActionState>(action_entity).unwrap(),
            ActionState::Executing
        );
    }

    #[test]
    fn checkpoint3_stale_cross_owner_action_target_is_rejected_offline() {
        use big_brain::prelude::{ActionState, Actor};

        use crate::common::{AttackTarget, SetAttackTarget};
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;

        let mut game = HeadlessGame::new(20_000);
        let owner_player_id = game.spawn_hero("Warrior", "AssaultTargetOwnerBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let assault_id = game.settlement_crisis().unwrap().assault_id.unwrap();
        let generation = game.settlement_crisis().unwrap().assault_spawn_generation;
        let helper_player_id = spawn_connected_helper(&mut game, "AssaultForeignTargetBot");
        let unit_id = game.crisis_assault_units()[0].obj_id;

        let (unit_entity, helper_entity, helper_id, helper_pos, helper_hp) = {
            let world = game.app.world_mut();
            let unit_entity = world
                .resource::<EntityObjMap>()
                .get_entity(unit_id)
                .unwrap();
            let mut query = world
                .query_filtered::<(Entity, &Id, &PlayerId, &Position, &Stats), With<SubclassHero>>(
                );
            let (helper_entity, helper_id, _, helper_pos, helper_stats) = query
                .iter(world)
                .find(|(_, _, owner, _, _)| owner.0 == helper_player_id)
                .expect("foreign helper hero");
            (
                unit_entity,
                helper_entity,
                helper_id.0,
                *helper_pos,
                helper_stats.hp,
            )
        };

        while (game.game_tick() + 1) % crate::constants::TICKS_PER_SEC == 0 {
            game.tick(1);
        }
        let set_action = {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(unit_entity).unwrap() = helper_pos;
            world.get_mut::<VisibleTarget>(unit_entity).unwrap().target = helper_id;
            world
                .spawn((Actor(unit_entity), ActionState::Requested, SetAttackTarget))
                .id()
        };
        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            *game.world().get::<ActionState>(set_action).unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            game.world()
                .get::<VisibleTarget>(unit_entity)
                .unwrap()
                .target,
            crate::constants::NO_TARGET
        );

        while (game.game_tick() + 1) % crate::constants::TICKS_PER_SEC == 0 {
            game.tick(1);
        }
        let attack_action = {
            let world = game.app.world_mut();
            world.get_mut::<VisibleTarget>(unit_entity).unwrap().target = helper_id;
            world
                .spawn((Actor(unit_entity), ActionState::Requested, AttackTarget))
                .id()
        };
        game.tick(1);

        assert_eq!(
            *game.world().get::<ActionState>(attack_action).unwrap(),
            ActionState::Failure
        );
        assert_eq!(
            game.world().get::<Stats>(helper_entity).unwrap().hp,
            helper_hp
        );
        let crisis = game.settlement_crisis().unwrap();
        assert_eq!(crisis.phase, CrisisPhase::AssaultActive);
        assert_eq!(crisis.assault_id, Some(assault_id));
        assert_eq!(crisis.assault_spawn_generation, generation);
        assert_eq!(owner_player_id, game.player_id());
    }

    #[test]
    fn checkpoint3_disconnect_drops_queued_player_combat_without_progress() {
        use crate::constants::ATTACK_COOLDOWN_TICKS;
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultQueuedCombatBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let unit_ids = launched
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let target_id = game.crisis_assault_units()[0].obj_id;
        let score_before = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();

        let (hero_id, hero_entity, hero_pos, target_entity) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(Entity, &Id, &Position), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_pos) = hero_query.iter(world).next().unwrap();
            let target_entity = world
                .resource::<EntityObjMap>()
                .get_entity(target_id)
                .unwrap();
            (hero_id.0, hero_entity, *hero_pos, target_entity)
        };
        {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(target_entity).unwrap() = hero_pos;
            world.get_mut::<Stats>(target_entity).unwrap().hp = 0;
            *world.get_mut::<State>(hero_entity).unwrap() = State::None;
            world.get_mut::<Stats>(hero_entity).unwrap().stamina = Some(100);
            world.resource_mut::<GameTick>().0 += ATTACK_COOLDOWN_TICKS + 1;
        }
        game.inject(PlayerEvent::Attack {
            player_id,
            attack_type: "quick".to_string(),
            source_id: hero_id,
            target_id,
        });
        game.disconnect_player();
        game.tick(2);

        let crisis = game.settlement_crisis().unwrap();
        let score_after = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        assert_eq!(crisis.phase, CrisisPhase::AssaultActive);
        assert_eq!(crisis.assault_id, Some(assault_id));
        assert_eq!(crisis.assault_spawn_generation, generation);
        assert_eq!(
            crisis
                .assault_unit_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            unit_ids
        );
        assert_eq!(score_after.enemies_killed, score_before.enemies_killed);
        assert_eq!(score_after.elites_killed, score_before.elites_killed);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert!(game.world().get::<StateDead>(target_entity).is_none());
        assert_eq!(
            game.crisis_assault_units().len(),
            GOBLIN_ASSAULT_COMPOSITION.len()
        );
    }

    #[test]
    fn checkpoint3_disconnect_drops_queued_combo_and_block_state() {
        use crate::combat::{AttackType, ComboTracker};
        use crate::effect::{Effect, Effects};
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultQueuedComboBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let unit_ids = launched
            .assault_unit_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let target_id = game.crisis_assault_units()[0].obj_id;
        let score_before = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();

        let (hero_id, hero_entity, hero_pos, target_entity) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(Entity, &Id, &Position), With<SubclassHero>>();
            let (hero_entity, hero_id, hero_pos) = hero_query.iter(world).next().unwrap();
            let target_entity = world
                .resource::<EntityObjMap>()
                .get_entity(target_id)
                .unwrap();
            (hero_id.0, hero_entity, *hero_pos, target_entity)
        };
        {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(target_entity).unwrap() = hero_pos;
            world.get_mut::<Stats>(target_entity).unwrap().hp = 0;
            *world.get_mut::<State>(hero_entity).unwrap() = State::None;
            world.get_mut::<Stats>(hero_entity).unwrap().stamina = Some(100);
            world.entity_mut(hero_entity).insert(ComboTracker {
                target_id,
                attacks: vec![AttackType::Quick, AttackType::Quick],
            });
        }
        game.inject(PlayerEvent::Combo {
            player_id,
            source_id: hero_id,
            target_id,
            combo_type: "Hamstring".to_string(),
        });
        game.inject(PlayerEvent::Block {
            player_id,
            source_id: hero_id,
            defense: "brace".to_string(),
        });
        game.disconnect_player();
        game.tick(2);

        let crisis = game.settlement_crisis().unwrap();
        let effects = game.world().get::<Effects>(hero_entity).unwrap();
        let score_after = game
            .world()
            .resource::<RunScoreState>()
            .get(&player_id)
            .cloned()
            .unwrap_or_default();
        assert_eq!(crisis.phase, CrisisPhase::AssaultActive);
        assert_eq!(crisis.assault_id, Some(assault_id));
        assert_eq!(crisis.assault_spawn_generation, generation);
        assert_eq!(
            crisis
                .assault_unit_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            unit_ids
        );
        assert!(!effects.has(Effect::Bracing));
        assert!(!effects.has(Effect::Dodging));
        assert!(!effects.has(Effect::Parrying));
        assert_eq!(score_after.enemies_killed, score_before.enemies_killed);
        assert_eq!(score_after.elites_killed, score_before.elites_killed);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert_eq!(
            game.crisis_assault_units().len(),
            GOBLIN_ASSAULT_COMPOSITION.len()
        );
    }

    #[test]
    fn checkpoint3_connected_helper_resolves_offline_owner_assault_once() {
        use crate::game::CrisisPhase;

        let mut game = HeadlessGame::new(20_000);
        let owner_player_id = game.spawn_hero("Warrior", "AssaultOfflineOwnerBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let unit_ids = launched.assault_unit_ids.clone();
        let helper_player_id = spawn_connected_helper(&mut game, "AssaultOnlineHelperBot");
        let helper_score_before = game
            .world()
            .resource::<RunScoreState>()
            .get(&helper_player_id)
            .cloned()
            .unwrap_or_default();
        game.disconnect_player();

        kill_assault_unit_through_normal_combat_as(&mut game, helper_player_id, unit_ids[0]);
        let partial = game.settlement_crisis().unwrap();
        assert_eq!(partial.phase, CrisisPhase::AssaultActive);
        assert_eq!(partial.assault_id, Some(assault_id));
        assert_eq!(partial.assault_spawn_generation, generation);
        assert_eq!(partial.assault_defeated_unit_ids.len(), 1);
        assert_eq!(game.personal_crises_resolved(), 0);

        for unit_id in unit_ids.iter().skip(1) {
            kill_assault_unit_through_normal_combat_as(&mut game, helper_player_id, *unit_id);
        }

        let helper_score_after = game
            .world()
            .resource::<RunScoreState>()
            .get(&helper_player_id)
            .cloned()
            .unwrap_or_default();
        let crisis = game.settlement_crisis().unwrap();
        assert_eq!(crisis.phase, CrisisPhase::Resolved);
        assert_eq!(crisis.assault_id, Some(assault_id));
        assert_eq!(crisis.assault_spawn_generation, generation);
        assert_eq!(
            helper_score_after.enemies_killed,
            helper_score_before.enemies_killed + unit_ids.len() as i32
        );
        assert_eq!(
            helper_score_after.elites_killed,
            helper_score_before.elites_killed + unit_ids.len() as i32
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            helper_score_after.personal_crises_resolved, 0,
            "crisis completion remains with the attributed owner"
        );

        game.tick(20);
        assert_eq!(game.personal_crises_resolved(), 1);
        game.reconnect_player();
        game.tick(3);
        let reconnected = game.settlement_crisis().unwrap();
        assert_eq!(reconnected.phase, CrisisPhase::Resolved);
        assert_eq!(reconnected.assault_id, Some(assault_id));
        assert_eq!(reconnected.assault_spawn_generation, generation);
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            game.world()
                .resource::<RunScoreState>()
                .get(&owner_player_id)
                .map(|score| score.personal_crises_resolved),
            Some(1)
        );
    }

    #[test]
    fn checkpoint3_duplicate_new_player_cannot_erase_an_active_assault() {
        use crate::game::CrisisPhase;
        use crate::player_setup::{AssignedStartLocations, StartLocations};

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultDuplicateRunBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let before = game.settlement_crisis().unwrap();
        let assault_id = before.assault_id.unwrap();
        let generation = before.assault_spawn_generation;
        let unit_ids = game
            .crisis_assault_units()
            .iter()
            .filter(|unit| unit.owner_player_id == player_id)
            .map(|unit| unit.obj_id)
            .collect::<HashSet<_>>();
        let starts_before = game.world().resource::<StartLocations>().len();

        game.inject(PlayerEvent::NewPlayer {
            player_id,
            hero_name: "DuplicateRunBot".to_string(),
            class_name: "Warrior".to_string(),
        });
        game.tick(8);

        let after = game.settlement_crisis().unwrap();
        let owned_heroes = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<&PlayerId, With<SubclassHero>>();
            query
                .iter(world)
                .filter(|owner| owner.0 == player_id)
                .count()
        };
        assert_eq!(owned_heroes, 1);
        assert_eq!(after.phase, CrisisPhase::AssaultActive);
        assert_eq!(after.assault_id, Some(assault_id));
        assert_eq!(after.assault_spawn_generation, generation);
        assert_eq!(
            game.world().resource::<StartLocations>().len(),
            starts_before
        );
        assert!(game
            .world()
            .resource::<AssignedStartLocations>()
            .contains_key(&player_id));
        assert_eq!(
            game.crisis_assault_units()
                .iter()
                .filter(|unit| unit.owner_player_id == player_id)
                .map(|unit| unit.obj_id)
                .collect::<HashSet<_>>(),
            unit_ids
        );
    }

    #[test]
    fn checkpoint3_helper_kill_counts_for_owner_without_transferring_crisis() {
        use crate::constants::ATTACK_COOLDOWN_TICKS;
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultOwnerBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let units = game.crisis_assault_units();
        let helper_player_id = player_id + 1;

        // Create a real connected helper run and submit the same normal combat
        // event a nearby human client would send.
        let helper_client = Client {
            id: Uuid::from_u128(helper_player_id as u128),
            player_id: helper_player_id,
            sender: game.packet_tx.clone(),
        };
        assert!(game.clients.activate(helper_client).is_empty());
        game.inject(PlayerEvent::NewPlayer {
            player_id: helper_player_id,
            hero_name: "AssaultHelperBot".to_string(),
            class_name: "Warrior".to_string(),
        });
        game.tick(8);

        let target_id = units[0].obj_id;
        let (helper_id, helper_entity, helper_pos, target_entity) = {
            let world = game.app.world_mut();
            let mut helper_query =
                world.query_filtered::<(Entity, &Id, &PlayerId, &Position), With<SubclassHero>>();
            let (helper_entity, helper_id, _, helper_pos) = helper_query
                .iter(world)
                .find(|(_, _, owner, _)| owner.0 == helper_player_id)
                .map(|(entity, id, owner, pos)| (entity, id.0, owner.0, *pos))
                .expect("connected helper hero");
            let target_entity = world
                .resource::<EntityObjMap>()
                .get_entity(target_id)
                .expect("helper combat target");
            (helper_id, helper_entity, helper_pos, target_entity)
        };
        {
            let world = game.app.world_mut();
            *world.get_mut::<Position>(target_entity).unwrap() = helper_pos;
            world.get_mut::<Stats>(target_entity).unwrap().hp = 0;
            *world.get_mut::<State>(helper_entity).unwrap() = State::None;
            world.get_mut::<Stats>(helper_entity).unwrap().stamina = Some(100);
            world.resource_mut::<GameTick>().0 += ATTACK_COOLDOWN_TICKS + 1;
        }
        game.inject(PlayerEvent::Attack {
            player_id: helper_player_id,
            attack_type: "quick".to_string(),
            source_id: helper_id,
            target_id,
        });
        game.tick(3);
        assert!(game.world().get::<StateDead>(target_entity).is_some());

        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultActive
        );
        assert_eq!(
            game.settlement_crisis()
                .unwrap()
                .assault_defeated_unit_ids
                .len(),
            1
        );
        assert_eq!(game.personal_crises_resolved(), 0);
        assert_eq!(
            game.world()
                .resource::<RunScoreState>()
                .get(&helper_player_id)
                .map(|score| score.enemies_killed),
            Some(1),
            "ordinary kill score follows LastAttacker"
        );

        for unit in units.iter().skip(1) {
            kill_assault_unit_through_normal_combat(&mut game, unit.obj_id);
        }
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            game.world()
                .resource::<RunScoreState>()
                .get(&helper_player_id)
                .map(|score| score.personal_crises_resolved)
                .unwrap_or(0),
            0,
            "crisis completion remains with the attributed owner"
        );
    }

    #[test]
    fn checkpoint3_missing_live_unit_stays_committed_and_requires_recovery() {
        use crate::game::CrisisPhase;
        use crate::ids::{EntityObjMap, Ids};

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultMissingUnitBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let units = game.crisis_assault_units();
        let missing_id = units[0].obj_id;
        let surviving_ids = units
            .iter()
            .skip(1)
            .map(|unit| unit.obj_id)
            .collect::<HashSet<_>>();
        let missing_entity = game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(missing_id)
            .unwrap();
        {
            let world = game.app.world_mut();
            world.resource_mut::<EntityObjMap>().remove_obj(missing_id);
            world.resource_mut::<Ids>().remove_obj(missing_id);
            world.despawn(missing_entity);
        }
        game.tick(1);

        let committed = game.settlement_crisis().unwrap();
        assert_eq!(committed.phase, CrisisPhase::AssaultActive);
        assert_eq!(committed.assault_id, Some(assault_id));
        assert_eq!(committed.assault_spawn_generation, generation);
        assert!(committed.assault_recovery_required);
        assert!(committed.assault_defeated_unit_ids.is_empty());
        assert!(!committed.resolution_recorded);
        assert_eq!(game.personal_crises_resolved(), 0);
        assert_eq!(
            game.crisis_assault_units()
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<HashSet<_>>(),
            surviving_ids
        );
        let outcome = game.crisis_balance_telemetry().assault_outcome;
        assert_eq!(outcome.assault_units_defeated, 0);
        assert_eq!(outcome.player_kills, 0);
        assert_eq!(outcome.villager_kills, 0);
        assert_eq!(outcome.helper_kills, 0);
        assert_eq!(outcome.defence_or_other_kills, 0);
        assert!(!outcome.assault_resolved);
    }

    #[test]
    fn checkpoint3_disconnect_continuation_headless() {
        use big_brain::actions::spawn_action;
        use big_brain::prelude::{ActionState, Actor};

        use crate::common::AttackTarget;
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;
        use crate::obj::LastAttacker;
        use crate::villager::FightBack;

        let mut game = HeadlessGame::new(30_000);
        let player_id = game.spawn_hero("Warrior", "AssaultContinuationBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);

        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let assault_started_tick = launched.assault_started_tick;
        let phase_started_tick = launched.phase_started_tick;
        let units = game.crisis_assault_units();
        let unit_ids_before = units.iter().map(|unit| unit.obj_id).collect::<Vec<_>>();
        let sentinel_id = units[0].obj_id;
        let npc_action_id = units[1].obj_id;
        let helper_target_id = npc_action_id;
        let helper_player_id = spawn_connected_helper(&mut game, "AssaultContinuationHelper");

        let foreign_structure_health_before = {
            let world = game.app.world_mut();
            let mut query =
                world.query_filtered::<(&Id, &PlayerId, &Stats), With<ClassStructure>>();
            let mut structures = query
                .iter(world)
                .filter(|(_, owner, _)| owner.0 == helper_player_id)
                .map(|(id, _, stats)| (id.0, stats.hp))
                .collect::<Vec<_>>();
            structures.sort_by_key(|(id, _)| *id);
            assert!(!structures.is_empty(), "helper run should own structures");
            structures
        };

        let action_tick = game.game_tick();
        let (
            villager_entity,
            villager_id,
            npc_entity,
            sentinel_damaged_hp,
            villager_hp_before,
            npc_hp_before,
        ) = {
            let npc_pos = units[1].pos;
            let villager_pos = passable_unoccupied_adjacent_position(&mut game, npc_pos);
            let (villager_entity, villager_id) =
                spawn_armed_owner_villager(&mut game, player_id, villager_pos);
            let world = game.app.world_mut();
            let entity_map = world.resource::<EntityObjMap>();
            let sentinel_entity = entity_map.get_entity(sentinel_id).unwrap();
            let npc_entity = entity_map.get_entity(npc_action_id).unwrap();
            let sentinel_base_hp = world.get::<Stats>(sentinel_entity).unwrap().base_hp;
            let sentinel_damaged_hp = sentinel_base_hp - 7;
            world.get_mut::<Stats>(sentinel_entity).unwrap().hp = sentinel_damaged_hp;
            *world.get_mut::<Position>(sentinel_entity).unwrap() = Position { x: 49, y: 49 };
            world
                .get_mut::<VisibleTarget>(sentinel_entity)
                .unwrap()
                .target = crate::constants::NO_TARGET;
            *world.get_mut::<Position>(npc_entity).unwrap() = npc_pos;
            world.get_mut::<Stats>(npc_entity).unwrap().base_damage = Some(30);
            world.get_mut::<VisibleTarget>(npc_entity).unwrap().target = villager_id;
            world.entity_mut(villager_entity).insert(LastAttacker {
                id: npc_action_id,
                tick: action_tick,
            });
            let villager_hp_before = world.get::<Stats>(villager_entity).unwrap().hp;
            let npc_hp_before = world.get::<Stats>(npc_entity).unwrap().hp;
            (
                villager_entity,
                villager_id,
                npc_entity,
                sentinel_damaged_hp,
                villager_hp_before,
                npc_hp_before,
            )
        };

        let (npc_action, villager_action) = {
            let world = game.app.world_mut();
            let npc_action = world
                .spawn((Actor(npc_entity), ActionState::Requested, AttackTarget))
                .id();
            let villager_action = {
                let mut commands = world.commands();
                spawn_action(&FightBack, &mut commands, villager_entity)
            };
            world.flush();
            *world
                .entity_mut(villager_action)
                .get_mut::<ActionState>()
                .unwrap() = ActionState::Requested;
            (npc_action, villager_action)
        };

        let units_before = game.crisis_assault_units();
        let unit_health_before = units_before
            .iter()
            .map(|unit| (unit.obj_id, unit.hp))
            .collect::<Vec<_>>();
        assert_eq!(
            units_before
                .iter()
                .find(|unit| unit.obj_id == sentinel_id)
                .unwrap()
                .hp,
            sentinel_damaged_hp
        );
        let crisis_before_disconnect = game.settlement_crisis().unwrap();
        let pressure = crisis_before_disconnect.pressure;
        let warning_active = crisis_before_disconnect.warning_active;

        game.disconnect_player();
        game.tick(1);
        assert_eq!(
            *game.world().get::<ActionState>(npc_action).unwrap(),
            ActionState::Executing,
            "attributed NPC AI must continue while its owner is offline"
        );
        assert_eq!(
            *game.world().get::<ActionState>(villager_action).unwrap(),
            ActionState::Executing,
            "owner villager AI must continue defending while its owner is offline"
        );
        assert!(
            game.world().get::<Stats>(villager_entity).unwrap().hp < villager_hp_before,
            "the attributed NPC must damage a valid owner villager while offline"
        );
        assert!(
            game.world().get::<Stats>(npc_entity).unwrap().hp < npc_hp_before,
            "the owner villager must damage an attributed attacker while offline"
        );
        assert_eq!(
            game.world()
                .get::<VisibleTarget>(npc_entity)
                .unwrap()
                .target,
            villager_id,
            "a still-valid owner-associated target should remain selected"
        );
        game.tick(19);

        kill_assault_unit_through_normal_combat_as(&mut game, helper_player_id, helper_target_id);
        let offline = game.settlement_crisis().unwrap();
        assert_eq!(offline.phase, CrisisPhase::AssaultActive);
        assert_eq!(offline.assault_id, Some(assault_id));
        assert_eq!(offline.assault_spawn_generation, generation);
        assert_eq!(offline.assault_started_tick, assault_started_tick);
        assert_eq!(offline.phase_started_tick, phase_started_tick);
        assert_eq!(offline.pressure, pressure);
        assert_eq!(offline.warning_active, warning_active);
        assert_eq!(offline.assault_defeated_unit_ids.len(), 1);
        assert!(!offline.assault_recovery_required);
        assert_eq!(game.personal_crises_resolved(), 0);

        let units_offline = game.crisis_assault_units();
        let unit_ids_offline = units_offline
            .iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        let unit_health_offline = units_offline
            .iter()
            .map(|unit| (unit.obj_id, unit.hp))
            .collect::<Vec<_>>();
        assert_eq!(unit_ids_offline, unit_ids_before);
        for unit in units_offline.iter().filter(|unit| !unit.dead) {
            let hp_before = unit_health_before
                .iter()
                .find(|(id, _)| *id == unit.obj_id)
                .unwrap()
                .1;
            assert!(unit.hp <= hp_before, "disconnect must never heal attackers");
        }
        assert_eq!(
            units_offline
                .iter()
                .find(|unit| unit.obj_id == sentinel_id)
                .unwrap()
                .hp,
            sentinel_damaged_hp,
            "the isolated damaged attacker must retain its health"
        );

        let foreign_target_ids = {
            let world = game.app.world_mut();
            let mut query = world.query::<(&Id, &PlayerId)>();
            query
                .iter(world)
                .filter(|(_, owner)| owner.0 == helper_player_id)
                .map(|(id, _)| id.0)
                .collect::<HashSet<_>>()
        };
        assert!(units_offline.iter().all(|unit| unit
            .visible_target
            .map(|target| !foreign_target_ids.contains(&target))
            .unwrap_or(true)));

        let foreign_structure_health_offline = {
            let world = game.app.world_mut();
            let mut query =
                world.query_filtered::<(&Id, &PlayerId, &Stats), With<ClassStructure>>();
            let mut structures = query
                .iter(world)
                .filter(|(_, owner, _)| owner.0 == helper_player_id)
                .map(|(id, _, stats)| (id.0, stats.hp))
                .collect::<Vec<_>>();
            structures.sort_by_key(|(id, _)| *id);
            structures
        };
        assert_eq!(
            foreign_structure_health_offline,
            foreign_structure_health_before
        );

        game.reconnect_player();
        let reconnected = game.settlement_crisis().unwrap();
        let units_reconnected = game.crisis_assault_units();
        let unit_ids_reconnected = units_reconnected
            .iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        let unit_health_reconnected = units_reconnected
            .iter()
            .map(|unit| (unit.obj_id, unit.hp))
            .collect::<Vec<_>>();
        assert_eq!(reconnected.phase, CrisisPhase::AssaultActive);
        assert_eq!(reconnected.assault_id, Some(assault_id));
        assert_eq!(reconnected.assault_spawn_generation, generation);
        assert_eq!(reconnected.assault_started_tick, assault_started_tick);
        assert_eq!(reconnected.phase_started_tick, phase_started_tick);
        assert_eq!(unit_ids_reconnected, unit_ids_offline);
        assert_eq!(unit_health_reconnected, unit_health_offline);

        game.tick(1);
        let after_reconnect_update = game.settlement_crisis().unwrap();
        let units_after_reconnect_update = game.crisis_assault_units();
        assert_eq!(after_reconnect_update.phase, CrisisPhase::AssaultActive);
        assert_eq!(after_reconnect_update.assault_id, Some(assault_id));
        assert_eq!(after_reconnect_update.assault_spawn_generation, generation);
        assert_eq!(
            after_reconnect_update.assault_started_tick,
            assault_started_tick
        );
        assert_eq!(
            after_reconnect_update.phase_started_tick,
            phase_started_tick
        );
        assert_eq!(after_reconnect_update.pressure, pressure);
        assert_eq!(after_reconnect_update.warning_active, warning_active);
        assert_eq!(
            units_after_reconnect_update
                .iter()
                .map(|unit| unit.obj_id)
                .collect::<Vec<_>>(),
            unit_ids_reconnected
        );
        for unit in units_after_reconnect_update
            .iter()
            .filter(|unit| !unit.dead)
        {
            let hp_at_reconnect = unit_health_reconnected
                .iter()
                .find(|(id, _)| *id == unit.obj_id)
                .unwrap()
                .1;
            assert!(
                unit.hp <= hp_at_reconnect,
                "the first reconnected update must not heal a surviving attacker"
            );
        }

        for unit in units_after_reconnect_update
            .iter()
            .filter(|unit| !unit.dead)
        {
            kill_assault_unit_through_normal_combat(&mut game, unit.obj_id);
        }
        game.tick(2);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        game.tick(20);
        assert_eq!(game.personal_crises_resolved(), 1);

        println!(
            "checkpoint3_disconnect_continuation assault_id_before={assault_id} assault_id_while_offline={} assault_id_after_reconnect={} generation_before={generation} generation_while_offline={} generation_after_reconnect={} unit_ids_before={unit_ids_before:?} unit_ids_while_offline={unit_ids_offline:?} unit_ids_after_reconnect={unit_ids_reconnected:?} unit_health_before={unit_health_before:?} unit_health_while_offline={unit_health_offline:?} unit_health_after_reconnect={unit_health_reconnected:?} phase_before={:?} phase_while_offline={:?} phase_after_reconnect={:?} phase_final={:?} resolution_count={} other_player_structures_targeted=false",
            offline.assault_id.unwrap(),
            reconnected.assault_id.unwrap(),
            offline.assault_spawn_generation,
            reconnected.assault_spawn_generation,
            launched.phase,
            offline.phase,
            reconnected.phase,
            game.settlement_crisis().unwrap().phase,
            game.personal_crises_resolved(),
        );
    }

    #[test]
    fn checkpoint3_legacy_mode_does_not_run_the_personal_assault_lifecycle() {
        use crate::game::{CrisisPhase, ASSAULT_MAX_ONLINE_WAIT_TICKS};

        let mut game = HeadlessGame::new_with_director(10_000, SurvivalDirectorMode::Legacy);
        game.spawn_hero("Warrior", "LegacyPersonalAssaultIsolationBot");
        let preferred_tick = set_personal_assault_ready(&mut game);
        {
            let world = game.app.world_mut();
            world.resource_mut::<GameTick>().0 = preferred_tick;
            let mut crises = world.resource_mut::<SettlementCrisisState>();
            let crisis = crises.get_mut(&game.player_id).unwrap();
            crisis.phase_online_ticks = ASSAULT_MAX_ONLINE_WAIT_TICKS;
            crisis.last_evaluated_tick = preferred_tick;
        }
        game.tick(3);

        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultReady
        );
        assert!(game.crisis_assault_units().is_empty());
    }

    #[test]
    fn checkpoint3_true_death_cleanup_isolated_and_idempotent() {
        use crate::event::GameEvent;
        use crate::ids::{EntityObjMap, Ids};

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "AssaultTrueDeathBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let own_assault_id = game.settlement_crisis().unwrap().assault_id.unwrap();
        let own_ids = game
            .crisis_assault_units()
            .iter()
            .map(|unit| unit.obj_id)
            .collect::<Vec<_>>();
        let orphaned_entity = {
            let world = game.app.world_mut();
            let entity = world
                .resource::<EntityObjMap>()
                .get_entity(own_ids[0])
                .expect("attributed assault entity");
            world.resource_mut::<EntityObjMap>().remove_obj(own_ids[0]);
            entity
        };

        let (other_entity, other_id, other_crisis, unrelated_entity, unrelated_id) = {
            let world = game.app.world_mut();
            let other_id = world.resource_mut::<Ids>().new_obj_id();
            let other_assault_id = own_assault_id + 1;
            let entity = world
                .spawn((
                    Id(other_id),
                    PlayerId(crate::constants::NPC_PLAYER_ID),
                    Position { x: 0, y: 0 },
                    Template("Wolf Rider".to_string()),
                    State::None,
                    Stats {
                        hp: 75,
                        stamina: None,
                        mana: None,
                        base_hp: 75,
                        base_stamina: None,
                        base_mana: None,
                        base_def: 5,
                        damage_range: Some(1),
                        base_damage: Some(6),
                        base_speed: Some(1),
                        base_vision: Some(4),
                    },
                    CrisisAssaultUnit {
                        owner_player_id: player_id + 1,
                        assault_id: other_assault_id,
                        spawn_generation: 1,
                    },
                ))
                .id();
            world
                .resource_mut::<Ids>()
                .new_obj(other_id, crate::constants::NPC_PLAYER_ID);
            world
                .resource_mut::<EntityObjMap>()
                .new_obj(other_id, entity);
            let other_crisis = SettlementCrisis {
                phase: crate::game::CrisisPhase::AssaultActive,
                pressure: 100,
                warning_active: true,
                assault_id: Some(other_assault_id),
                assault_started_tick: Some(world.resource::<GameTick>().0),
                assault_unit_ids: vec![other_id],
                assault_spawn_generation: 1,
                ..SettlementCrisis::default()
            };
            world
                .resource_mut::<SettlementCrisisState>()
                .insert(player_id + 1, other_crisis.clone());

            // An un-attributed world hostile next to the recycled start is not
            // owned by this run and must not be removed as collateral cleanup.
            let unrelated_id = world.resource_mut::<Ids>().new_obj_id();
            let unrelated_entity = world
                .spawn((
                    Id(unrelated_id),
                    PlayerId(crate::constants::NPC_PLAYER_ID),
                    Position { x: 0, y: 0 },
                    Template("Wolf".to_string()),
                ))
                .id();
            world
                .resource_mut::<Ids>()
                .new_obj(unrelated_id, crate::constants::NPC_PLAYER_ID);
            world
                .resource_mut::<EntityObjMap>()
                .new_obj(unrelated_id, unrelated_entity);
            (
                entity,
                other_id,
                other_crisis,
                unrelated_entity,
                unrelated_id,
            )
        };

        let current_tick = game.game_tick();
        let (queued_villager_spawn, queued_npc_spawn, queued_actor_event, queued_target_event) = {
            let world = game.app.world_mut();
            let queued_villager_spawn = world.resource_mut::<Ids>().new_map_event_id();
            world.resource_mut::<GameEvents>().insert(
                queued_villager_spawn,
                GameEvent {
                    event_id: queued_villager_spawn,
                    start_tick: current_tick,
                    run_tick: current_tick + 100,
                    event_type: GameEventType::SpawnVillager {
                        pos: Position { x: 0, y: 0 },
                        player_id,
                    },
                },
            );
            let queued_npc_spawn = world.resource_mut::<Ids>().new_map_event_id();
            world.resource_mut::<GameEvents>().insert(
                queued_npc_spawn,
                GameEvent {
                    event_id: queued_npc_spawn,
                    start_tick: current_tick,
                    run_tick: current_tick + 100,
                    event_type: GameEventType::SpawnNPC {
                        npc_type: "Wolf".to_string(),
                        pos: Position { x: 0, y: 0 },
                        npc_id: None,
                        run_owner: Some(player_id),
                    },
                },
            );
            let queued_actor_event = world.resource_mut::<MapEvents>().new(
                own_ids[0],
                current_tick + 100,
                VisibleEvent::SpellDamageEvent {
                    spell: Spell::ShadowBolt,
                    target_id: other_id,
                },
            );
            let queued_target_event = world.resource_mut::<MapEvents>().new(
                unrelated_id,
                current_tick + 100,
                VisibleEvent::SpellDamageEvent {
                    spell: Spell::ShadowBolt,
                    target_id: *own_ids.last().unwrap(),
                },
            );
            (
                queued_villager_spawn,
                queued_npc_spawn,
                queued_actor_event.event_id,
                queued_target_event.event_id,
            )
        };
        let hero = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<Entity, With<SubclassHero>>();
            query.iter(world).next().expect("hero")
        };
        game.disconnect_player();
        game.app.world_mut().entity_mut(hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Checkpoint 3 cleanup".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick - (10 * crate::constants::TICKS_PER_SEC) - 1,
            },
        ));
        game.tick(3);

        assert!(game.settlement_crisis().is_none());
        assert_eq!(game.personal_crises_resolved(), 0);
        assert!(!game
            .world()
            .resource::<GameEvents>()
            .contains_key(&queued_villager_spawn));
        assert!(!game
            .world()
            .resource::<GameEvents>()
            .contains_key(&queued_npc_spawn));
        assert!(!game
            .world()
            .resource::<MapEvents>()
            .contains_key(&queued_actor_event));
        assert!(!game
            .world()
            .resource::<MapEvents>()
            .contains_key(&queued_target_event));
        assert!(own_ids.iter().all(|id| game
            .world()
            .resource::<EntityObjMap>()
            .get_entity(*id)
            .is_none()));
        assert!(
            game.world().get_entity(orphaned_entity).is_err(),
            "True Death must despawn an attributed orphan even without its entity-map entry"
        );
        assert!(game.world().get_entity(other_entity).is_ok());
        let other_after_cleanup = game
            .world()
            .resource::<SettlementCrisisState>()
            .get(&(player_id + 1))
            .cloned()
            .expect("another player's active crisis");
        assert_eq!(other_after_cleanup.phase, other_crisis.phase);
        assert_eq!(other_after_cleanup.pressure, other_crisis.pressure);
        assert_eq!(
            other_after_cleanup.warning_active,
            other_crisis.warning_active
        );
        assert_eq!(other_after_cleanup.assault_id, other_crisis.assault_id);
        assert_eq!(
            other_after_cleanup.assault_spawn_generation,
            other_crisis.assault_spawn_generation
        );
        assert_eq!(
            other_after_cleanup.assault_unit_ids,
            other_crisis.assault_unit_ids
        );
        assert_eq!(
            other_after_cleanup.assault_defeated_unit_ids,
            other_crisis.assault_defeated_unit_ids
        );
        assert_eq!(
            other_after_cleanup.resolution_recorded,
            other_crisis.resolution_recorded
        );
        assert_eq!(
            other_after_cleanup.assault_recovery_required,
            other_crisis.assault_recovery_required
        );
        assert_eq!(game.world().get::<Stats>(other_entity).unwrap().hp, 75);
        assert_eq!(
            game.world()
                .get::<CrisisAssaultUnit>(other_entity)
                .unwrap()
                .owner_player_id,
            player_id + 1
        );
        assert_eq!(
            game.world().resource::<EntityObjMap>().get_entity(other_id),
            Some(other_entity)
        );
        assert!(game.world().get_entity(unrelated_entity).is_ok());
        assert_eq!(
            game.world()
                .resource::<EntityObjMap>()
                .get_entity(unrelated_id),
            Some(unrelated_entity)
        );

        game.tick(5);
        assert!(game.world().get_entity(other_entity).is_ok());
        assert!(game.world().get_entity(unrelated_entity).is_ok());
    }

    #[test]
    fn checkpoint3_true_death_while_ready_cancels_without_launch_or_completion() {
        use crate::game::CrisisPhase;

        let mut game = HeadlessGame::new(20_000);
        game.spawn_hero("Warrior", "AssaultReadyTrueDeathBot");
        let _preferred_tick = set_personal_assault_ready(&mut game);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::AssaultReady
        );

        let current_tick = game.game_tick();
        let hero = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<Entity, With<SubclassHero>>();
            query.iter(world).next().expect("hero")
        };
        game.app.world_mut().entity_mut(hero).insert((
            State::Dead,
            StateDead {
                dead_at: current_tick,
                killer: "Checkpoint 3 ready cleanup".to_string(),
            },
            TrueDeath {
                true_death_at: current_tick - (10 * crate::constants::TICKS_PER_SEC) - 1,
            },
        ));
        game.tick(3);

        assert!(game.settlement_crisis().is_none());
        assert!(game.crisis_assault_units().is_empty());
        assert_eq!(game.personal_crises_resolved(), 0);
    }

    #[test]
    fn checkpoint3_repeated_disconnect_reconnect_preserves_one_generation() {
        use crate::game::CrisisPhase;
        use crate::ids::EntityObjMap;
        use crate::npc::VisibleTarget;

        let mut game = HeadlessGame::new(40_000);
        game.spawn_hero("Warrior", "AssaultRepeatBot");
        game.set_sanctuary_at_base(3);
        let preferred_tick = set_personal_assault_ready(&mut game);
        advance_ready_clock_to_launch(&mut game, preferred_tick);
        let launched = game.settlement_crisis().unwrap();
        let assault_id = launched.assault_id.unwrap();
        let generation = launched.assault_spawn_generation;
        let phase_started_tick = launched.phase_started_tick;
        let assault_started_tick = launched.assault_started_tick;
        let units = game.crisis_assault_units();
        let unit_ids = units.iter().map(|unit| unit.obj_id).collect::<Vec<_>>();
        let sentinel_id = unit_ids[0];
        {
            let world = game.app.world_mut();
            let sentinel = world
                .resource::<EntityObjMap>()
                .get_entity(sentinel_id)
                .unwrap();
            let base_hp = world.get::<Stats>(sentinel).unwrap().base_hp;
            world.get_mut::<Stats>(sentinel).unwrap().hp = base_hp - 5;
            *world.get_mut::<Position>(sentinel).unwrap() = Position { x: 49, y: 49 };
            world.get_mut::<VisibleTarget>(sentinel).unwrap().target = crate::constants::NO_TARGET;
        }

        for _ in 0..3 {
            game.disconnect_player();
            game.tick(2);
            let offline = game.settlement_crisis().unwrap();
            let units_offline = game.crisis_assault_units();
            assert_eq!(offline.phase, CrisisPhase::AssaultActive);
            assert_eq!(offline.assault_id, Some(assault_id));
            assert_eq!(offline.assault_spawn_generation, generation);
            assert_eq!(offline.phase_started_tick, phase_started_tick);
            assert_eq!(offline.assault_started_tick, assault_started_tick);
            assert_eq!(
                units_offline
                    .iter()
                    .map(|unit| unit.obj_id)
                    .collect::<Vec<_>>(),
                unit_ids
            );
            let health_offline = units_offline
                .iter()
                .map(|unit| (unit.obj_id, unit.hp))
                .collect::<Vec<_>>();

            game.reconnect_player();
            let reconnected = game.settlement_crisis().unwrap();
            let units_reconnected = game.crisis_assault_units();
            assert_eq!(reconnected.phase, CrisisPhase::AssaultActive);
            assert_eq!(reconnected.assault_id, Some(assault_id));
            assert_eq!(reconnected.assault_spawn_generation, generation);
            assert_eq!(
                units_reconnected
                    .iter()
                    .map(|unit| (unit.obj_id, unit.hp))
                    .collect::<Vec<_>>(),
                health_offline
            );
        }

        let final_units = game.crisis_assault_units();
        for unit in final_units {
            kill_assault_unit_through_normal_combat(&mut game, unit.obj_id);
        }
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().unwrap().phase,
            CrisisPhase::Resolved
        );
        assert_eq!(game.personal_crises_resolved(), 1);
        assert_eq!(
            game.settlement_crisis().unwrap().assault_spawn_generation,
            generation
        );

        println!(
            "checkpoint3_repeated_safety assault_id={} disconnects=3 final_generation={} stale_units=0 completion_count={} duplicate_assault=false panic=false",
            assault_id,
            generation,
            game.personal_crises_resolved()
        );
    }

    // Hand-crafting Firewood from a Log — the cook economy's fuel chain. The bot
    // relies on this to keep cooking past the 10 starting Firewood.
    #[test]
    fn craft_firewood_from_log() {
        use crate::ids::Ids;
        use crate::templates::Templates;

        let mut game = HeadlessGame::new(5_000);
        let pid = game.spawn_hero("Warrior", "FuelBot");
        game.tick(50);

        // Give the hero a Log (same item the Burrow stocks).
        {
            let world = game.app.world_mut();
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut q = world.query_filtered::<&mut Inventory, With<SubclassHero>>();
            let mut inv = q.iter_mut(world).next().expect("hero inventory");
            inv.new(
                item_id,
                "Springbranch Maple Log".to_string(),
                1,
                &item_templates,
            );
        }
        let firewood_before: i32 = game
            .observe()
            .inventory
            .iter()
            .filter(|i| i.name == "Firewood")
            .map(|i| i.quantity)
            .sum();

        game.inject(PlayerEvent::Craft {
            player_id: pid,
            recipe_name: "Firewood".to_string(),
        });
        game.tick(100); // crafting_time is 60 ticks

        let view = game.observe();
        let firewood_after: i32 = view
            .inventory
            .iter()
            .filter(|i| i.name == "Firewood")
            .map(|i| i.quantity)
            .sum();
        let gained = firewood_after - firewood_before;
        // Inventory::craft rolls a random 1..=amount yield (amount=5 for Firewood).
        assert!(
            (1..=5).contains(&gained),
            "crafting should turn 1 Log into 1-5 Firewood, got {gained}"
        );
        assert!(
            !view.inventory.iter().any(|i| i.class == "Log"),
            "the Log should be consumed by the craft"
        );
    }

    // The merchant hire mechanic, end-to-end and isolated from the bot's survival:
    // dock the merchant on the hero's tile, give the hero gold, send Hire, and
    // confirm a player-owned villager appears and the wage is deducted.
    #[test]
    fn hire_from_merchant_adds_villager() {
        use crate::ids::Ids;
        use crate::templates::Templates;

        let mut game = HeadlessGame::new(5_000);
        let pid = game.spawn_hero("Warrior", "HireBot");
        game.tick(50); // let new_player setup spawn the merchant + cargo villagers

        // Hero position (the merchant will dock here so the adjacency check passes).
        let hero_pos = {
            let world = game.app.world_mut();
            let mut q = world.query_filtered::<&Position, With<SubclassHero>>();
            *q.iter(world).next().expect("hero exists")
        };

        // Give the hero 50 Gold Coins to pay the wage from.
        {
            let world = game.app.world_mut();
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut q = world.query_filtered::<&mut Inventory, With<SubclassHero>>();
            let mut hero_inv = q.iter_mut(world).next().expect("hero inventory");
            hero_inv.new(item_id, "Gold Coins".to_string(), 50, &item_templates);
        }
        let gold_before: i32 = game
            .observe()
            .inventory
            .iter()
            .filter(|i| i.class == "Gold Coins")
            .map(|i| i.quantity)
            .sum();
        assert_eq!(gold_before, 50, "hero should be carrying the gold we added");

        // Dock the merchant on the hero's tile and grab a cargo villager to hire.
        let (merchant_id, target_id) = {
            let world = game.app.world_mut();
            let mut q = world.query::<(&Id, &mut Position, &mut Merchant, &Transport)>();
            let (id, mut pos, mut merchant, transport) =
                q.iter_mut(world).next().expect("merchant exists");
            *pos = hero_pos;
            merchant.sail_state = MerchantSailState::AtLanding;
            let target = *transport
                .hauling
                .first()
                .expect("merchant carries cargo villagers");
            (id.0, target)
        };

        assert_eq!(
            game.observe().villagers.len(),
            0,
            "player has no villager before hiring"
        );

        game.inject(PlayerEvent::Hire {
            player_id: pid,
            merchant_id,
            target_id,
        });
        game.tick(20);

        assert_eq!(
            game.observe().villagers.len(),
            1,
            "hiring should give the player one villager"
        );
        let gold_after: i32 = game
            .observe()
            .inventory
            .iter()
            .filter(|i| i.class == "Gold Coins")
            .map(|i| i.quantity)
            .sum();
        assert_eq!(
            gold_after, 25,
            "the wage (25) should be deducted from the hero"
        );
    }

    // Empowering the Monolith sanctuary: pay Soulshards, level rises, and the
    // suppression radius grows with it.
    #[test]
    fn upgrade_sanctuary_raises_level_and_radius() {
        use crate::game::{sanctuary_full_radius, Monolith, SANCTUARY_UPGRADE_COST};
        use crate::ids::Ids;
        use crate::templates::Templates;

        let mut game = HeadlessGame::new(5_000);
        let pid = game.spawn_hero("Warrior", "SancBot");
        game.tick(50);

        // Find the Monolith and confirm it starts at sanctuary level 0.
        let (monolith_id, monolith_pos, level0) = {
            let world = game.app.world_mut();
            let mut q = world.query::<(&Id, &Position, &Monolith)>();
            let (id, pos, m) = q.iter(world).next().expect("a monolith exists");
            (id.0, *pos, m.sanctuary_level)
        };
        assert_eq!(level0, 0, "sanctuary starts at level 0");

        // Stand the hero on the Monolith and give them enough Soulshards.
        {
            let world = game.app.world_mut();
            let item_id = world.resource_mut::<Ids>().new_item_id();
            let item_templates = world.resource::<Templates>().item_templates.clone();
            let mut q =
                world.query_filtered::<(&mut Position, &mut Inventory), With<SubclassHero>>();
            let (mut hpos, mut hinv) = q.iter_mut(world).next().expect("hero");
            *hpos = monolith_pos;
            hinv.new(
                item_id,
                "Soulshard".to_string(),
                SANCTUARY_UPGRADE_COST + 1,
                &item_templates,
            );
        }

        game.inject(PlayerEvent::UpgradeSanctuary {
            player_id: pid,
            monolith_id,
        });
        game.tick(20);

        let level1 = {
            let world = game.app.world_mut();
            let mut q = world.query::<&Monolith>();
            q.iter(world).next().expect("monolith").sanctuary_level
        };
        assert_eq!(level1, 1, "upgrade should raise the sanctuary level");
        assert!(
            sanctuary_full_radius(level1) > sanctuary_full_radius(level0),
            "the suppression radius should grow with level"
        );

        let shards: i32 = game
            .observe()
            .inventory
            .iter()
            .filter(|i| i.class == "Soulshard")
            .map(|i| i.quantity)
            .sum();
        assert_eq!(
            shards, 1,
            "the upgrade cost should be deducted in Soulshards"
        );
    }

    #[test]
    fn safe_logout_checkpoint4_headless_reports_open_and_closed_fifty_thousand_tick_interval() {
        let (mut game, _) = safe_logout_fixture("SafeLogoutTelemetryDurationBot");
        game.complete_valid_safe_logout_via_authenticated_ingress();
        game.disconnect_after_completed_safe_logout();
        let protected_since = game
            .player_presence_record()
            .and_then(|record| record.protected_since_tick)
            .expect("protected interval start");
        let hero_before = game.protected_hero_snapshot();

        // Checkpoint 2 already runs 10,000 consecutive protected updates. This
        // scenario advances the authoritative world clock by 50,000 and then
        // executes the production schedule once, covering large-duration
        // arithmetic without making the focused suite five times slower.
        game.app.world_mut().resource_mut::<GameTick>().0 += 50_000;
        game.tick(1);
        assert_eq!(
            game.player_presence(),
            Some(PlayerWorldPresence::OfflineProtected)
        );
        assert_eq!(game.protected_hero_snapshot(), hero_before);

        let open = game.safe_logout_telemetry();
        let open_duration = game.game_tick().saturating_sub(protected_since) as u64;
        assert!(open_duration >= 50_000);
        assert_eq!(open.requests, 1);
        assert_eq!(open.accepted, 1);
        assert_eq!(open.completed, 1);
        assert_eq!(open.protected_sessions_started, 1);
        assert_eq!(open.protected_ticks_total, open_duration);
        assert_eq!(
            game.metrics().safe_logout_protected_ticks_total,
            open_duration,
            "RunMetrics must include a still-open protection interval"
        );

        game.reconnect_and_exit_protection();
        let closed = game.safe_logout_telemetry();
        assert_eq!(closed.resumed, 1);
        assert!(closed.protected_ticks_total >= open_duration);
        assert!(closed.timer_rebases > 0);
        assert_eq!(closed.invariant_recoveries, 0);
        assert_eq!(closed.run_key_mismatches, 0);
    }

    #[test]
    fn safe_logout_checkpoint4_headless_keeps_multiplayer_presence_and_telemetry_isolated() {
        let (mut game, _) = safe_logout_fixture("SafeLogoutIsolationOwnerBot");
        let owner_id = game.player_id();
        let helper_id = spawn_connected_helper(&mut game, "SafeLogoutIsolationHelperBot");

        game.complete_valid_safe_logout();
        game.disconnect_after_completed_safe_logout();
        assert_eq!(
            game.world()
                .resource::<PlayerWorldPresenceState>()
                .players
                .get(&helper_id)
                .map(|record| record.state),
            Some(PlayerWorldPresence::Online)
        );
        {
            let world = game.app.world_mut();
            world
                .resource_mut::<PlayerIntroState>()
                .get_mut(&helper_id)
                .expect("helper introduction state")
                .danger_unlocked = true;
            world
                .resource_mut::<crate::game::InitialEncounterState>()
                .remove(&helper_id);
        }
        let helper_crisis_before = game
            .world()
            .resource::<SettlementCrisisState>()
            .get(&helper_id)
            .expect("helper crisis")
            .online_active_ticks;
        game.tick(25);
        assert!(
            game.world()
                .resource::<SettlementCrisisState>()
                .get(&helper_id)
                .expect("helper crisis after world progress")
                .online_active_ticks
                > helper_crisis_before,
            "a connected neighbor must continue while only the owner is protected"
        );

        let helper_sanctuary = place_player_in_own_bound_sanctuary(&mut game, helper_id);
        move_nearby_headless_hostiles_away(&mut game, helper_sanctuary);
        game.tick(1);
        let helper_connection = game
            .clients
            .current_connection_id(helper_id)
            .expect("helper authoritative connection");
        game.inject(PlayerEvent::RequestSafeLogout {
            player_id: helper_id,
            connection_id: helper_connection,
        });
        for _ in 0..=(crate::safe_logout::SAFE_LOGOUT_COUNTDOWN_TICKS + 8) {
            game.tick(1);
            let helper_state = game
                .world()
                .resource::<PlayerWorldPresenceState>()
                .players
                .get(&helper_id)
                .map(|record| record.state);
            if helper_state == Some(PlayerWorldPresence::OfflineProtected) {
                break;
            }
        }
        assert_eq!(
            game.world()
                .resource::<PlayerWorldPresenceState>()
                .players
                .get(&helper_id)
                .map(|record| record.state),
            Some(PlayerWorldPresence::OfflineProtected),
            "the second owner should independently complete Safe Logout"
        );
        game.clients.remove_if_current(helper_connection);
        game.tick(1);
        assert!(is_player_offline_protected(
            owner_id,
            game.world().resource::<PlayerWorldPresenceState>()
        ));
        assert!(is_player_offline_protected(
            helper_id,
            game.world().resource::<PlayerWorldPresenceState>()
        ));

        game.reconnect_and_exit_protection();
        assert!(!is_player_offline_protected(
            owner_id,
            game.world().resource::<PlayerWorldPresenceState>()
        ));
        assert!(is_player_offline_protected(
            helper_id,
            game.world().resource::<PlayerWorldPresenceState>()
        ));

        let telemetry = game.world().resource::<SafeLogoutTelemetryState>();
        let owner = telemetry.get(&owner_id).expect("owner telemetry");
        let helper = telemetry.get(&helper_id).expect("helper telemetry");
        assert_eq!(owner.completed, 1);
        assert_eq!(owner.ordinary_disconnects, 0);
        assert_eq!(owner.resumed, 1);
        assert_eq!(helper.requests, 1);
        assert_eq!(helper.completed, 1);
        assert_eq!(helper.resumed, 0);
        assert_eq!(helper.ordinary_disconnects, 0);
        assert_eq!(helper.active_assault_disconnects, 0);
        assert_eq!(game.player_presence(), Some(PlayerWorldPresence::Online));
    }

    #[test]
    fn safe_logout_checkpoint4_headless_distinguishes_ordinary_and_active_assault_disconnects() {
        let (mut ordinary, _) = safe_logout_fixture("SafeLogoutOrdinaryDisconnectBot");
        ordinary.disconnect_player();
        ordinary.tick(1);
        let ordinary_once = ordinary.safe_logout_telemetry();
        assert_eq!(ordinary_once.ordinary_disconnects, 1);
        assert_eq!(ordinary_once.active_assault_disconnects, 0);
        ordinary.tick(5);
        assert_eq!(ordinary.safe_logout_telemetry(), ordinary_once);

        let (mut assault, _) = safe_logout_fixture("SafeLogoutAssaultDisconnectBot");
        let preferred_tick = set_personal_assault_ready(&mut assault);
        advance_ready_clock_to_launch(&mut assault, preferred_tick);
        assert_eq!(
            assault
                .settlement_crisis()
                .expect("active personal assault")
                .phase,
            CrisisPhase::AssaultActive
        );
        assault.request_safe_logout();
        assault.tick(1);
        assert_eq!(
            assault.safe_logout_rejection_reason(),
            Some(SafeLogoutRejectionReason::AssaultActive)
        );
        assault.disconnect_player();
        assault.tick(1);
        let assault_once = assault.safe_logout_telemetry();
        assert_eq!(assault_once.ordinary_disconnects, 1);
        assert_eq!(assault_once.active_assault_disconnects, 1);
        assert_eq!(assault_once.rejected, 1);
        assert_eq!(
            assault_once
                .rejection_reasons
                .get(&SafeLogoutRejectionReason::AssaultActive),
            Some(&1)
        );
        assert_eq!(
            assault
                .settlement_crisis()
                .expect("assault persists after disconnect")
                .phase,
            CrisisPhase::AssaultActive
        );
        assault.tick(5);
        assert_eq!(assault.safe_logout_telemetry(), assault_once);
    }

    #[test]
    fn safe_logout_checkpoint4_headless_final_tick_danger_matrix_cancels_before_protection() {
        use crate::safe_logout::SAFE_LOGOUT_COUNTDOWN_TICKS;

        fn advance_to_final_update(game: &mut HeadlessGame) {
            let requested_tick = begin_safe_logout(game);
            game.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
            assert_eq!(
                game.player_presence(),
                Some(PlayerWorldPresence::SafeLogoutPending)
            );
            assert_eq!(
                game.game_tick(),
                requested_tick + SAFE_LOGOUT_COUNTDOWN_TICKS - 1
            );
        }

        fn assert_cancelled(game: &HeadlessGame, reason: SafeLogoutCancelReason) {
            assert_ne!(
                game.player_presence(),
                Some(PlayerWorldPresence::OfflineProtected)
            );
            assert_eq!(game.safe_logout_cancel_reason(), Some(reason));
            let telemetry = game.safe_logout_telemetry();
            assert_eq!(telemetry.completed, 0);
            assert_eq!(telemetry.cancelled, 1);
            assert_eq!(telemetry.cancellation_reasons.get(&reason), Some(&1));
        }

        let (mut movement, sanctuary) = safe_logout_fixture("SafeLogoutFinalTickMovementBot");
        advance_to_final_update(&mut movement);
        movement.move_hero_for_test(move_one_tile(sanctuary));
        movement.tick(1);
        assert_cancelled(&movement, SafeLogoutCancelReason::Moved);

        let (mut combat, _) = safe_logout_fixture("SafeLogoutFinalTickCombatBot");
        advance_to_final_update(&mut combat);
        combat.record_player_combat_for_test();
        combat.tick(1);
        assert_cancelled(&combat, SafeLogoutCancelReason::EnteredCombat);

        let (mut damage, _) = safe_logout_fixture("SafeLogoutFinalTickDamageBot");
        advance_to_final_update(&mut damage);
        damage.damage_hero_for_test(1);
        damage.tick(1);
        assert_cancelled(&damage, SafeLogoutCancelReason::TookDamage);

        let (mut hostile, sanctuary) = safe_logout_fixture("SafeLogoutFinalTickHostileBot");
        let hostile_id =
            hostile.spawn_safe_logout_test_hostile(far_map_position(&hostile, sanctuary));
        advance_to_final_update(&mut hostile);
        hostile.move_safe_logout_test_hostile(hostile_id, sanctuary);
        hostile.tick(1);
        assert_cancelled(&hostile, SafeLogoutCancelReason::HostileNearby);

        let (mut assault, _) = safe_logout_fixture("SafeLogoutFinalTickAssaultBot");
        let preferred_tick = set_personal_assault_ready(&mut assault);
        let pre_request_tick = preferred_tick - SAFE_LOGOUT_COUNTDOWN_TICKS - 1;
        {
            use crate::game::ASSAULT_READY_GRACE_TICKS;

            let world = assault.app.world_mut();
            world.resource_mut::<GameTick>().0 = pre_request_tick;
            let mut crises = world.resource_mut::<SettlementCrisisState>();
            let crisis = crises
                .get_mut(&assault.player_id)
                .expect("ready personal crisis");
            crisis.phase_online_ticks = ASSAULT_READY_GRACE_TICKS - SAFE_LOGOUT_COUNTDOWN_TICKS - 1;
            crisis.last_evaluated_tick = pre_request_tick;
        }
        let requested_tick = begin_safe_logout(&mut assault);
        assert_eq!(requested_tick + SAFE_LOGOUT_COUNTDOWN_TICKS, preferred_tick);
        assault.tick((SAFE_LOGOUT_COUNTDOWN_TICKS - 1) as u32);
        assert_eq!(
            assault
                .settlement_crisis()
                .expect("ready personal crisis")
                .phase,
            CrisisPhase::AssaultReady
        );
        assault.tick(1);
        assert_eq!(
            assault
                .settlement_crisis()
                .expect("launched personal crisis")
                .phase,
            CrisisPhase::AssaultActive
        );
        assert_cancelled(&assault, SafeLogoutCancelReason::AssaultStarted);

        let (mut disconnected, _) = safe_logout_fixture("SafeLogoutFinalTickDisconnectBot");
        advance_to_final_update(&mut disconnected);
        disconnected.disconnect_player();
        disconnected.tick(1);
        assert_eq!(
            disconnected.player_presence(),
            Some(PlayerWorldPresence::Disconnected)
        );
        assert_cancelled(&disconnected, SafeLogoutCancelReason::Disconnected);
        let disconnect_telemetry = disconnected.safe_logout_telemetry();
        assert_eq!(disconnect_telemetry.ordinary_disconnects, 1);
        assert_eq!(disconnect_telemetry.active_assault_disconnects, 0);
    }

    #[test]
    fn crisis_balance_preparation_snapshot_records_raw_state_without_mutating_inventory() {
        use crate::constants::ARMOR;
        use crate::ids::Ids;
        use crate::structure::Structure;

        fn fixture_stats(hp: i32, base_hp: i32, base_damage: i32) -> Stats {
            Stats {
                hp,
                stamina: Some(100),
                mana: None,
                base_hp,
                base_stamina: Some(100),
                base_mana: None,
                base_def: 0,
                damage_range: Some(1),
                base_damage: Some(base_damage),
                base_speed: Some(1),
                base_vision: Some(1),
            }
        }

        let mut game = HeadlessGame::new(10_000);
        let player_id = game.spawn_hero("Warrior", "CrisisBalanceSnapshotBot");
        game.set_crisis_balance_sample_interval(Some(600));
        game.set_sanctuary_at_base(5);
        game.tick(2);
        assert!(game.settlement_crisis().is_some());

        {
            let world = game.app.world_mut();
            let wall_id = world.resource_mut::<Ids>().new_obj_id();
            let foundation_id = world.resource_mut::<Ids>().new_obj_id();
            let living_villager_id = world.resource_mut::<Ids>().new_obj_id();
            let dead_villager_id = world.resource_mut::<Ids>().new_obj_id();
            world.spawn((
                PlayerId(player_id),
                Id(wall_id),
                Position { x: 3, y: 3 },
                Template("Stockade".to_string()),
                Subclass::Wall,
                State::None,
                fixture_stats(17, 20, 0),
                Inventory {
                    owner: wall_id,
                    items: Vec::new(),
                },
                ClassStructure,
            ));
            world.spawn((
                PlayerId(player_id),
                Id(foundation_id),
                Position { x: 4, y: 3 },
                Template("Storage".to_string()),
                Subclass::Storage,
                State::Founded,
                fixture_stats(1, 100, 0),
                Inventory {
                    owner: foundation_id,
                    items: Vec::new(),
                },
                ClassStructure,
            ));
            world.spawn((
                PlayerId(player_id),
                Id(living_villager_id),
                State::None,
                fixture_stats(80, 100, 1),
                Inventory {
                    owner: living_villager_id,
                    items: Vec::new(),
                },
                SubclassVillager,
            ));
            world.spawn((
                PlayerId(player_id),
                Id(dead_villager_id),
                State::Dead,
                StateDead {
                    dead_at: 0,
                    killer: "fixture".to_string(),
                },
                fixture_stats(0, 100, 1),
                Inventory {
                    owner: dead_villager_id,
                    items: Vec::new(),
                },
                SubclassVillager,
            ));
            world.resource_mut::<GameTick>().0 += 1;
        }

        let (
            expected_inventory_signature,
            expected_armor,
            expected_healing,
            expected_weapon,
            expected_built,
            expected_foundations,
            expected_walls,
            expected_wall_hp,
            expected_wall_max_hp,
            expected_villagers,
            expected_combat_villagers,
            expected_stored,
        ) = {
            let world = game.app.world_mut();
            let mut hero_query =
                world.query_filtered::<(&PlayerId, &Inventory), With<SubclassHero>>();
            let inventory = hero_query
                .iter(world)
                .find(|(owner, _)| owner.0 == player_id)
                .unwrap()
                .1;
            let signature = inventory
                .items
                .iter()
                .map(|item| (item.id, item.quantity, item.equipped))
                .collect::<Vec<_>>();
            let armor = inventory
                .items
                .iter()
                .filter(|item| item.equipped && item.class == ARMOR)
                .count() as i32;
            let healing = inventory
                .items
                .iter()
                .filter(|item| item.attrs.contains_key(&AttrKey::Healing))
                .map(|item| item.quantity)
                .sum::<i32>();
            let weapon = inventory
                .items
                .iter()
                .find(|item| item.equipped && item.class == crate::constants::WEAPON)
                .map(|item| item.name.clone());

            let mut structure_query = world.query_filtered::<
                (&PlayerId, &Subclass, &State, &Stats, &Inventory),
                With<ClassStructure>,
            >();
            let mut built = 0;
            let mut foundations = 0;
            let mut walls = 0;
            let mut wall_hp = 0;
            let mut wall_max_hp = 0;
            let mut stored = 0;
            for (owner, subclass, state, stats, storage) in structure_query.iter(world) {
                if owner.0 != player_id {
                    continue;
                }
                if Structure::is_built(*state) {
                    built += 1;
                    if *subclass == Subclass::Wall {
                        walls += 1;
                        wall_hp += stats.hp.max(0);
                        wall_max_hp += stats.base_hp.max(0);
                    }
                    if *subclass == Subclass::Storage {
                        stored += storage
                            .items
                            .iter()
                            .map(|item| item.quantity.max(0))
                            .sum::<i32>();
                    }
                } else {
                    foundations += 1;
                }
            }

            let mut villager_query = world.query_filtered::<(
                &PlayerId,
                &State,
                &Stats,
                &Inventory,
                Option<&StateDead>,
            ), With<SubclassVillager>>();
            let mut villagers = 0;
            let mut combat_villagers = 0;
            for (owner, state, stats, inventory, dead) in villager_query.iter(world) {
                if owner.0 != player_id || !state.is_alive() || stats.hp <= 0 || dead.is_some() {
                    continue;
                }
                villagers += 1;
                if stats.base_damage.unwrap_or(0) > 0
                    || inventory
                        .items
                        .iter()
                        .any(|item| item.equipped && item.class == crate::constants::WEAPON)
                {
                    combat_villagers += 1;
                }
            }

            (
                signature,
                armor,
                healing,
                weapon,
                built,
                foundations,
                walls,
                wall_hp,
                wall_max_hp,
                villagers,
                combat_villagers,
                stored,
            )
        };

        let metrics = game.metrics();
        let snapshot = metrics
            .crisis_balance
            .preparation_snapshots
            .resolution_or_end
            .expect("forced run-end preparation snapshot");
        assert_eq!(snapshot.hero_class, "Warrior");
        assert_eq!(snapshot.equipped_weapon, expected_weapon);
        assert_eq!(snapshot.equipped_armor_count, expected_armor);
        assert_eq!(snapshot.healing_items, expected_healing);
        assert_eq!(snapshot.completed_structures, expected_built);
        assert_eq!(snapshot.foundations, expected_foundations);
        assert_eq!(snapshot.wall_segments, expected_walls);
        assert_eq!(snapshot.wall_total_health, expected_wall_hp);
        assert_eq!(snapshot.wall_total_max_health, expected_wall_max_hp);
        assert_eq!(snapshot.villagers_alive, expected_villagers);
        assert_eq!(snapshot.villagers_combat_capable, expected_combat_villagers);
        assert_eq!(snapshot.sanctuary_level, 5);
        assert_eq!(snapshot.stored_resources_total, expected_stored);

        let after_inventory_signature = {
            let world = game.app.world_mut();
            let mut query = world.query_filtered::<(&PlayerId, &Inventory), With<SubclassHero>>();
            query
                .iter(world)
                .find(|(owner, _)| owner.0 == player_id)
                .unwrap()
                .1
                .items
                .iter()
                .map(|item| (item.id, item.quantity, item.equipped))
                .collect::<Vec<_>>()
        };
        assert_eq!(after_inventory_signature, expected_inventory_signature);
    }

    #[test]
    fn checkpoint3_prelaunch_sample_captures_ready_action_between_periodic_samples() {
        use crate::game::is_usable_crisis_healing_item;

        let mut game = HeadlessGame::new(20_000);
        let player_id = game.spawn_hero("Warrior", "PrelaunchPreparationSampleBot");
        game.set_sanctuary_at_base(3);
        game.set_crisis_balance_sample_interval(Some(600));

        // Remove any starting heal before the Ready baseline so the one item
        // added below is the only positive preparation delta under test.
        {
            let world = game.app.world_mut();
            let mut heroes =
                world.query_filtered::<(&PlayerId, &mut Inventory), With<SubclassHero>>();
            let (_, mut inventory) = heroes
                .iter_mut(world)
                .find(|(owner, _)| owner.0 == player_id)
                .expect("headless hero inventory");
            inventory
                .items
                .retain(|item| !is_usable_crisis_healing_item(item));
        }

        let preferred_tick = set_personal_assault_ready(&mut game);
        game.app.world_mut().resource_mut::<GameTick>().0 = preferred_tick - 2;
        game.tick(1);
        assert_eq!(
            game.settlement_crisis().expect("ready crisis").phase,
            CrisisPhase::AssaultReady
        );
        assert_eq!(
            game.crisis_balance_telemetry()
                .preparation_actions
                .healing_items_acquired,
            0,
            "the first Ready observation establishes the periodic baseline"
        );

        // This acquisition happens one tick after the prior sample, far short
        // of the 600-tick interval. The successful launch boundary must close
        // the final Ready interval before the phase changes to AssaultActive.
        {
            let world = game.app.world_mut();
            let templates = world.resource::<Templates>().item_templates.clone();
            let bandage_id = world.resource_mut::<Ids>().new_item_id();
            let mut heroes =
                world.query_filtered::<(&PlayerId, &mut Inventory), With<SubclassHero>>();
            let (_, mut inventory) = heroes
                .iter_mut(world)
                .find(|(owner, _)| owner.0 == player_id)
                .expect("headless hero inventory");
            inventory.new(bandage_id, "Crude Bandage".to_string(), 1, &templates);
        }

        game.tick(1);
        let launched = game.settlement_crisis().expect("launched crisis");
        assert_eq!(launched.phase, CrisisPhase::AssaultActive);
        let launch_tick = launched.assault_started_tick.expect("launch tick");
        let actions = game.crisis_balance_telemetry().preparation_actions;
        assert_eq!(actions.healing_items_acquired, 1);
        assert_eq!(actions.healing_items_carried_at_launch, 1);
        assert_eq!(actions.first_preparation_action_tick, Some(launch_tick));
        assert!(actions
            .meaningful_preparation_categories
            .contains(&"healing".to_string()));

        game.tick(1);
        assert_eq!(
            game.crisis_balance_telemetry()
                .preparation_actions
                .healing_items_acquired,
            1,
            "the following Active sample must not recount the Ready action"
        );
    }

    #[test]
    fn preparation_pair_comparison_labels_are_stable() {
        let labels = PreparationComparison::ALL
            .into_iter()
            .map(PreparationComparison::label)
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                "existing_walls",
                "equipment_prepared",
                "healing_prepared",
                "combined_preparation",
            ]
        );
        for comparison in PreparationComparison::ALL {
            assert_eq!(
                PreparationComparison::from_label(comparison.label()),
                Some(comparison)
            );
        }
    }

    fn valid_preparation_test_launches(
        comparison: PreparationComparison,
    ) -> (PreparationPairLaunch, PreparationPairLaunch) {
        let control = PreparationPairLaunch {
            leg: PreparationPairLeg::Control,
            geometry: Vec::new(),
            common_fingerprint: PreparationCommonLaunchFingerprint::default(),
            fixture: PreparationFixtureState {
                completed_structures: 2,
                hide_wraps: 1,
                hide_wraps_items: vec![expected_hide_wraps(false)],
                tattered_shirt_items: vec![expected_tattered_shirt(true)],
                other_healing_items: 1,
                ..PreparationFixtureState::default()
            },
        };
        let treatment = PreparationPairLaunch {
            leg: PreparationPairLeg::Treatment,
            geometry: Vec::new(),
            common_fingerprint: control.common_fingerprint.clone(),
            fixture: PreparationFixtureState {
                completed_structures: 2 + i32::from(comparison.includes_wall()),
                completed_wall_segments: i32::from(comparison.includes_wall()),
                completed_stockades: i32::from(comparison.includes_wall()),
                declared_anchor_stockades: comparison
                    .includes_wall()
                    .then(expected_preparation_stockade)
                    .into_iter()
                    .collect(),
                hide_wraps: 1,
                hide_wraps_equipped: comparison.includes_equipment(),
                hide_wraps_items: vec![expected_hide_wraps(comparison.includes_equipment())],
                tattered_shirt_items: vec![expected_tattered_shirt(
                    !comparison.includes_equipment(),
                )],
                crude_bandages: i32::from(comparison.includes_healing()),
                crude_bandage_items: comparison
                    .includes_healing()
                    .then(expected_crude_bandage)
                    .into_iter()
                    .collect(),
                other_healing_items: 1,
                ..PreparationFixtureState::default()
            },
        };
        (control, treatment)
    }

    #[test]
    fn preparation_fixture_preserves_one_starting_potion_in_both_legs() {
        let fixture_for = |add_bandage: bool, name: &str| {
            let mut game = HeadlessGame::new(100);
            game.spawn_hero("Warrior", name);
            {
                let player_id = game.player_id;
                let world = game.app.world_mut();
                let templates = world.resource::<Templates>().item_templates.clone();
                let bandage_id = world.resource_mut::<Ids>().new_item_id();
                let mut heroes =
                    world.query_filtered::<(&PlayerId, &mut Inventory), With<SubclassHero>>();
                let (_, mut inventory) = heroes
                    .iter_mut(world)
                    .find(|(owner, _)| owner.0 == player_id)
                    .expect("hero inventory");
                inventory.new(bandage_id, "Crude Bandage".to_string(), 2, &templates);
            }
            game.install_preparation_inventory(add_bandage)
                .expect("install preparation inventory");
            let view = game.observe();
            assert_eq!(
                view.inventory
                    .iter()
                    .filter(|item| item.name == "Health Potion" && item.quantity > 0)
                    .map(|item| item.quantity)
                    .sum::<i32>(),
                1
            );
            game.preparation_fixture_state(&[])
                .expect("preparation fixture state")
        };

        let control = fixture_for(false, "PreparationPotionControl");
        let treatment = fixture_for(true, "PreparationPotionTreatment");
        assert_eq!(control.other_healing_items, 1);
        assert_eq!(treatment.other_healing_items, 1);
        assert_eq!(control.crude_bandages, 0);
        assert_eq!(treatment.crude_bandages, 1);
    }

    #[test]
    fn production_bandage_use_heals_consumes_once_and_records_active_assault_usage() {
        let mut game = HeadlessGame::new(20_000);
        game.restrict_to_preparation_pair_start_location()
            .expect("fixed preparation start");
        let player_id = game.spawn_hero("Warrior", "Cp4BandageUse");
        game.set_crisis_balance_sample_interval(Some(1));
        game.prepare_checkpoint4_preparation_pair_launch(
            PreparationComparison::HealingPrepared,
            PreparationPairLeg::Treatment,
        )
        .expect("active assault with one treatment bandage");

        {
            let world = game.app.world_mut();
            let mut heroes = world.query_filtered::<(&PlayerId, &mut Stats), With<SubclassHero>>();
            let (_, mut stats) = heroes
                .iter_mut(world)
                .find(|(owner, _)| owner.0 == player_id)
                .expect("hero stats");
            stats.hp = stats.base_hp - 20;
        }
        let before = game.observe();
        let before_hero = before.hero.expect("hero");
        let bandage = before
            .inventory
            .iter()
            .find(|item| item.name == "Crude Bandage")
            .expect("treatment bandage");
        let use_event = PlayerEvent::Use {
            player_id,
            obj_id: before_hero.id,
            item_id: bandage.id,
        };
        game.inject(use_event.clone());
        game.tick(3);

        let after = game.observe();
        assert_eq!(
            after.hero.expect("hero after bandage").hp,
            before_hero.hp + 10
        );
        assert_eq!(
            after
                .inventory
                .iter()
                .filter(|item| item.id == bandage.id)
                .map(|item| item.quantity)
                .sum::<i32>(),
            0
        );
        let engagement = game.crisis_balance_telemetry().engagement;
        assert_eq!(engagement.healing_items_used_during_assault, 1);
        assert_eq!(engagement.healing_hp_restored_during_assault, 10);

        game.inject(use_event);
        game.tick(3);
        let duplicate = game.crisis_balance_telemetry().engagement;
        assert_eq!(duplicate.healing_items_used_during_assault, 1);
        assert_eq!(duplicate.healing_hp_restored_during_assault, 10);
    }

    #[test]
    fn production_bandage_use_at_full_health_is_a_non_consuming_noop() {
        let mut game = HeadlessGame::new(20_000);
        game.restrict_to_preparation_pair_start_location()
            .expect("fixed preparation start");
        let player_id = game.spawn_hero("Warrior", "Cp4BandageNoop");
        game.set_crisis_balance_sample_interval(Some(1));
        game.prepare_checkpoint4_preparation_pair_launch(
            PreparationComparison::HealingPrepared,
            PreparationPairLeg::Treatment,
        )
        .expect("active assault with one treatment bandage");
        let before = game.observe();
        let hero = before.hero.expect("hero");
        assert_eq!(hero.hp, hero.base_hp);
        let bandage = before
            .inventory
            .iter()
            .find(|item| item.name == "Crude Bandage")
            .expect("treatment bandage");

        game.inject(PlayerEvent::Use {
            player_id,
            obj_id: hero.id,
            item_id: bandage.id,
        });
        game.tick(3);

        let after = game.observe();
        assert_eq!(after.hero.expect("hero after no-op").hp, hero.hp);
        assert_eq!(
            after
                .inventory
                .iter()
                .find(|item| item.id == bandage.id)
                .map(|item| item.quantity),
            Some(1)
        );
        let engagement = game.crisis_balance_telemetry().engagement;
        assert_eq!(engagement.healing_items_used_during_assault, 0);
        assert_eq!(engagement.healing_hp_restored_during_assault, 0);
    }

    #[test]
    fn production_health_potion_use_consumes_exactly_one_and_duplicate_cannot_overconsume() {
        let mut game = HeadlessGame::new(100);
        let player_id = game.spawn_hero("Warrior", "ProductionPotionUse");
        let (hero_id, potion_id, base_hp) = {
            let world = game.app.world_mut();
            let mut heroes = world
                .query_filtered::<(&PlayerId, &mut Stats, &mut Inventory), With<SubclassHero>>();
            let (_, mut stats, mut inventory) = heroes
                .iter_mut(world)
                .find(|(owner, ..)| owner.0 == player_id)
                .expect("hero with starting Health Potion");
            let hero_id = inventory.owner;
            let potion_id = {
                let potion = inventory
                    .items
                    .iter_mut()
                    .find(|item| item.name == "Health Potion")
                    .expect("starting Health Potion");
                assert_eq!(potion.quantity, 1);
                potion.quantity = 2;
                potion.id
            };
            stats.hp = stats.base_hp - 10;
            (hero_id, potion_id, stats.base_hp)
        };
        let use_event = PlayerEvent::Use {
            player_id,
            obj_id: hero_id,
            item_id: potion_id,
        };

        game.start_packet_capture();
        game.inject(use_event.clone());
        game.inject(use_event);
        game.tick(6);
        let packets = game.finish_packet_capture();

        let after = game.observe();
        assert_eq!(after.hero.expect("hero after potion").hp, base_hp);
        assert_eq!(
            after
                .inventory
                .iter()
                .find(|item| item.id == potion_id)
                .map(|item| item.quantity),
            Some(1),
            "the successful use consumes one, while the duplicate at full health is a no-op"
        );
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ResponsePacket::InfoInventory { id, items, .. }
                if *id == hero_id
                    && items
                        .iter()
                        .any(|item| item.id == potion_id && item.quantity == 1)
        )));
    }

    #[test]
    fn production_health_potion_use_at_full_health_without_sickness_is_non_consuming() {
        let mut game = HeadlessGame::new(100);
        let player_id = game.spawn_hero("Warrior", "ProductionPotionNoop");
        let before = game.observe();
        let hero = before.hero.expect("hero");
        let potion = before
            .inventory
            .iter()
            .find(|item| item.name == "Health Potion")
            .expect("starting Health Potion");
        assert_eq!(hero.hp, hero.base_hp);
        {
            let world = game.app.world_mut();
            let mut heroes = world.query_filtered::<(&PlayerId, &Effects), With<SubclassHero>>();
            let (_, effects) = heroes
                .iter(world)
                .find(|(owner, _)| owner.0 == player_id)
                .expect("hero effects");
            assert!(!effects.0.contains_key(&Effect::Sickness));
        }

        game.start_packet_capture();
        game.inject(PlayerEvent::Use {
            player_id,
            obj_id: hero.id,
            item_id: potion.id,
        });
        game.tick(3);
        let packets = game.finish_packet_capture();

        let after = game.observe();
        assert_eq!(after.hero.expect("hero after no-op").hp, hero.hp);
        assert_eq!(
            after
                .inventory
                .iter()
                .find(|item| item.id == potion.id)
                .map(|item| item.quantity),
            Some(1)
        );
        assert!(packets.iter().all(|packet| !matches!(
            packet,
            ResponsePacket::InfoInventory { id, .. } if *id == hero.id
        )));
    }

    #[test]
    fn production_herbal_poultice_cures_sickness_and_consumes_once() {
        let mut game = HeadlessGame::new(100);
        let player_id = game.spawn_hero("Warrior", "ProductionPoulticeUse");
        let (hero_id, poultice_id) = {
            let world = game.app.world_mut();
            let templates = world.resource::<Templates>().item_templates.clone();
            let poultice_id = world.resource_mut::<Ids>().new_item_id();
            let mut heroes = world
                .query_filtered::<(&PlayerId, &mut Inventory, &mut Effects), With<SubclassHero>>();
            let (_, mut inventory, mut effects) = heroes
                .iter_mut(world)
                .find(|(owner, ..)| owner.0 == player_id)
                .expect("hero inventory and effects");
            let poultice = inventory.new(poultice_id, "Herbal Poultice".to_string(), 1, &templates);
            assert_eq!(poultice.id, poultice_id);
            effects.0.insert(Effect::Sickness, (100, 1.0, 1));
            (inventory.owner, poultice_id)
        };

        game.start_packet_capture();
        let use_event = PlayerEvent::Use {
            player_id,
            obj_id: hero_id,
            item_id: poultice_id,
        };
        game.inject(use_event.clone());
        game.tick(3);
        let packets = game.finish_packet_capture();

        let world = game.app.world_mut();
        let mut heroes =
            world.query_filtered::<(&PlayerId, &Inventory, &Effects), With<SubclassHero>>();
        let (_, inventory, effects) = heroes
            .iter(world)
            .find(|(owner, ..)| owner.0 == player_id)
            .expect("hero after poultice");
        assert!(!effects.0.contains_key(&Effect::Sickness));
        assert!(inventory.get_by_id(poultice_id).is_none());
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ResponsePacket::LostEffect { id, effect, .. }
                if *id == hero_id && effect == &Effect::Sickness.to_str()
        )));
        assert!(packets.iter().any(|packet| matches!(
            packet,
            ResponsePacket::InfoInventory { id, items, .. }
                if *id == hero_id && items.iter().all(|item| item.id != poultice_id)
        )));

        drop(heroes);
        game.inject(use_event);
        game.tick(3);
        assert!(game
            .observe()
            .inventory
            .iter()
            .all(|item| item.id != poultice_id));
    }

    #[test]
    fn preparation_structure_anchor_must_be_free_before_fixture_spawn() {
        assert_eq!(PREPARATION_STOCKADE_ANCHOR, Position { x: 13, y: 13 });
        let mut game = HeadlessGame::new(100);
        game.restrict_to_preparation_pair_start_location()
            .expect("fixed preparation start");
        game.spawn_hero("Warrior", "PreparationAnchorBot");
        let hero_position = game.observe().hero.expect("hero").pos;
        let error = game
            .spawn_completed_preparation_structure("Stockade", hero_position)
            .expect_err("occupied hero tile must reject fixture structure");
        assert!(error.contains("already occupied"));

        game.spawn_completed_preparation_structure("Stockade", PREPARATION_STOCKADE_ANCHOR)
            .expect("verified free stockade anchor");
        assert!(game
            .spawn_completed_preparation_structure("Stockade", PREPARATION_STOCKADE_ANCHOR,)
            .unwrap_err()
            .contains("already occupied"));
    }

    #[test]
    fn checkpoint4_existing_walls_fixture_builds_a_matched_blocking_ring() {
        let launch = |leg, name| {
            let mut game = HeadlessGame::new(100);
            game.restrict_to_preparation_pair_start_location()
                .expect("fixed preparation start");
            game.spawn_hero("Warrior", name);
            let launch = game
                .prepare_checkpoint4_preparation_pair_launch(
                    PreparationComparison::ExistingWalls,
                    leg,
                )
                .expect("Checkpoint 4 wall launch");
            let hero_position = game.observe().hero.expect("hero").pos;
            (launch, hero_position)
        };

        let (control, control_hero) = launch(PreparationPairLeg::Control, "Cp4WallControl");
        let (treatment, treatment_hero) = launch(PreparationPairLeg::Treatment, "Cp4WallTreatment");

        assert_eq!(control_hero, treatment_hero);
        assert_eq!(control.fixture.completed_stockades, 0);
        assert_eq!(
            treatment.fixture.completed_stockades,
            CHECKPOINT4_BLOCKING_STOCKADE_COUNT
        );
        assert_eq!(
            treatment.fixture.declared_anchor_stockades.len(),
            CHECKPOINT4_BLOCKING_STOCKADE_COUNT as usize
        );
        assert!(treatment
            .fixture
            .declared_anchor_stockades
            .iter()
            .all(|stockade| Map::dist(
                Position {
                    x: stockade.position[0],
                    y: stockade.position[1],
                },
                treatment_hero,
            ) == 1));

        let difference = validate_checkpoint4_preparation_pair_launches(
            PreparationComparison::ExistingWalls,
            &control,
            &treatment,
        )
        .expect("only the six declared Stockades may differ");
        assert_eq!(
            difference.completed_stockade_delta,
            CHECKPOINT4_BLOCKING_STOCKADE_COUNT
        );
    }

    #[test]
    fn checkpoint4_normal_and_pair_fixtures_match_declared_production_launch_facts() {
        let launch = |checkpoint4_normal_fixture: bool, name: &str| {
            let mut game = HeadlessGame::new(20_000);
            game.restrict_to_preparation_pair_start_location()
                .expect("fixed preparation start");
            game.spawn_hero("Ranger", name);
            if checkpoint4_normal_fixture {
                game.prepare_checkpoint4_preparation_pair_launch(
                    PreparationComparison::EquipmentPrepared,
                    PreparationPairLeg::Treatment,
                )
            } else {
                game.prepare_preparation_pair_launch(
                    PreparationComparison::EquipmentPrepared,
                    PreparationPairLeg::Treatment,
                )
            }
            .expect("production assault launch")
        };

        // Equipment Prepared has no Checkpoint-4-specific wall geometry, so
        // both public runner fixtures are configured identically. Entropy may
        // choose different spawn positions, but every declared production
        // launch fact (class/state, phase/pressure/timing, inventory,
        // structures, composition, and unit HP) must be equivalent.
        let pair_launch = launch(false, "Cp4PairLaunchFacts");
        let normal_launch = launch(true, "Cp4NormalLaunchFacts");
        assert_eq!(
            pair_launch.common_fingerprint,
            normal_launch.common_fingerprint
        );
        assert_eq!(pair_launch.fixture, normal_launch.fixture);
        assert_eq!(
            pair_launch
                .geometry
                .iter()
                .map(|unit| (&unit.template, unit.template_ordinal))
                .collect::<Vec<_>>(),
            normal_launch
                .geometry
                .iter()
                .map(|unit| (&unit.template, unit.template_ordinal))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn checkpoint4_preparation_harness_preserves_production_hero_and_enemy_combat_stats() {
        for class_name in ["Warrior", "Ranger", "Mage"] {
            let mut game = HeadlessGame::new(20_000);
            game.restrict_to_preparation_pair_start_location()
                .expect("fixed preparation start");
            let player_id = game.spawn_hero(
                class_name,
                &format!("Cp4ProductionCombatValues{class_name}"),
            );
            let production_hero_stats = {
                let world = game.app.world_mut();
                let mut heroes = world.query_filtered::<(&PlayerId, &Stats), With<SubclassHero>>();
                let (_, stats) = heroes
                    .iter(world)
                    .find(|(owner, _)| owner.0 == player_id)
                    .expect("production-spawned hero stats");
                PreparationCombatStatsFingerprint::from_stats(stats)
            };

            let launch = game
                .prepare_checkpoint4_preparation_pair_launch(
                    PreparationComparison::EquipmentPrepared,
                    PreparationPairLeg::Control,
                )
                .expect("production assault launch");
            assert_eq!(
                launch.common_fingerprint.hero_combat_stats, production_hero_stats,
                "the preparation harness must not rewrite {class_name} combat stats"
            );
            assert_eq!(
                launch.common_fingerprint.hero_hp,
                launch.common_fingerprint.hero_combat_stats.hp
            );
            assert_eq!(
                launch.common_fingerprint.hero_base_hp,
                launch.common_fingerprint.hero_combat_stats.base_hp
            );
            assert_eq!(
                launch.common_fingerprint.hero_base_defence,
                launch.common_fingerprint.hero_combat_stats.base_defence
            );

            let templates = game.world().resource::<Templates>();
            for unit in &launch.common_fingerprint.assault_units {
                let template = templates.obj_templates.get(unit.template.clone());
                let production_template_stats = PreparationCombatStatsFingerprint {
                    hp: template.base_hp.expect("production NPC base HP"),
                    stamina: template.base_stamina,
                    mana: None,
                    base_hp: template.base_hp.expect("production NPC base HP"),
                    base_stamina: template.base_stamina,
                    base_mana: None,
                    base_defence: template.base_def.expect("production NPC base defence"),
                    damage_range: template.dmg_range,
                    base_damage: template.base_dmg,
                    base_speed: template.base_speed,
                    base_vision: template.base_vision,
                };
                assert_eq!(
                    unit.combat_stats, production_template_stats,
                    "the preparation harness must not rewrite {} combat stats",
                    unit.template
                );
                assert_eq!(unit.hp, unit.combat_stats.hp);
                assert_eq!(unit.base_hp, unit.combat_stats.base_hp);
                assert!(unit.effects.is_empty());
                assert_eq!(unit.last_combat_tick, LastCombatTick::default().0);
            }
        }
    }

    #[test]
    fn checkpoint4_stop_conditions_do_not_end_a_live_assault_before_engagement() {
        let cap_ticks = 15_000;

        // Launch, perception, movement, and combat can legitimately take more
        // than one decision. Neither inactivity nor an ordinary first death is
        // represented in the terminal rule, so both remain observable.
        assert_eq!(
            checkpoint4_assault_observation_stop_reason(
                Some(CrisisPhase::AssaultActive),
                true,
                false,
                0,
                cap_ticks,
            ),
            None
        );
        assert_eq!(
            checkpoint4_assault_observation_stop_reason(
                Some(CrisisPhase::AssaultActive),
                true,
                false,
                cap_ticks - 1,
                cap_ticks,
            ),
            None,
            "a still-live scenario must not stop merely because engagement has not happened yet"
        );

        assert_eq!(
            checkpoint4_assault_observation_stop_reason(
                Some(CrisisPhase::AssaultActive),
                true,
                false,
                cap_ticks,
                cap_ticks,
            ),
            Some(AssaultObservationStopReason::TickCap)
        );
        assert_eq!(
            checkpoint4_assault_observation_stop_reason(
                Some(CrisisPhase::Resolved),
                true,
                false,
                1,
                cap_ticks,
            ),
            Some(AssaultObservationStopReason::Resolved)
        );
        assert_eq!(
            checkpoint4_assault_observation_stop_reason(
                Some(CrisisPhase::AssaultActive),
                true,
                true,
                1,
                cap_ticks,
            ),
            Some(AssaultObservationStopReason::HeroTrueDeath)
        );
        assert_eq!(
            checkpoint4_assault_observation_stop_reason(
                Some(CrisisPhase::AssaultActive),
                false,
                false,
                1,
                cap_ticks,
            ),
            Some(AssaultObservationStopReason::HeroMissing)
        );
    }

    #[test]
    fn preparation_cleanup_despawns_ambient_actor_and_cancels_its_map_events() {
        let mut game = HeadlessGame::new(100);
        game.spawn_hero("Warrior", "PreparationCleanupBot");
        let object_id = {
            let world = game.app.world_mut();
            let object_id = world.resource_mut::<Ids>().new_obj_id();
            let entity = world
                .spawn((
                    Id(object_id),
                    PlayerId(1_000),
                    Position { x: 1, y: 1 },
                    State::Moving,
                    SubclassNPC,
                ))
                .id();
            world.resource_mut::<Ids>().new_obj(object_id, 1_000);
            world
                .resource_mut::<EntityObjMap>()
                .new_obj(object_id, entity);
            world.resource_mut::<MapEvents>().new(
                object_id,
                99,
                VisibleEvent::MoveEvent {
                    src: Position { x: 1, y: 1 },
                    dst: Position { x: 2, y: 1 },
                },
            );
            object_id
        };

        game.normalize_preparation_non_crisis_hostiles();
        let world = game.app.world();
        assert!(world
            .resource::<EntityObjMap>()
            .get_entity(object_id)
            .is_none());
        assert!(world.resource::<Ids>().get_player(object_id).is_none());
        assert!(world
            .resource::<MapEvents>()
            .values()
            .all(|event| event.obj_id != object_id));
    }

    #[test]
    fn preparation_pair_matches_composition_and_hp_but_keeps_actual_geometry() {
        let (mut control, mut treatment) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        control.geometry = vec![PreparationAssaultGeometry {
            template: "Wolf Rider".to_string(),
            template_ordinal: 0,
            position: [3, 10],
        }];
        treatment.geometry = vec![PreparationAssaultGeometry {
            template: "Wolf Rider".to_string(),
            template_ordinal: 0,
            position: [16, 21],
        }];
        let fingerprint = PreparationAssaultUnitFingerprint {
            template: "Wolf Rider".to_string(),
            hp: 40,
            base_hp: 40,
            combat_stats: PreparationCombatStatsFingerprint {
                hp: 40,
                base_hp: 40,
                ..PreparationCombatStatsFingerprint::default()
            },
            effects: Vec::new(),
            last_combat_tick: -1_000,
        };
        control.common_fingerprint.assault_units = vec![fingerprint.clone()];
        treatment.common_fingerprint.assault_units = vec![fingerprint];
        assert!(validate_preparation_pair_launches(
            PreparationComparison::EquipmentPrepared,
            &control,
            &treatment,
        )
        .is_ok());

        treatment.common_fingerprint.assault_units[0].hp -= 1;
        assert!(validate_preparation_pair_launches(
            PreparationComparison::EquipmentPrepared,
            &control,
            &treatment,
        )
        .unwrap_err()
        .contains("common launch fingerprint mismatch"));

        treatment.common_fingerprint.assault_units[0].hp += 1;
        treatment.common_fingerprint.assault_units[0]
            .combat_stats
            .base_damage = Some(99);
        assert!(validate_preparation_pair_launches(
            PreparationComparison::EquipmentPrepared,
            &control,
            &treatment,
        )
        .unwrap_err()
        .contains("common launch fingerprint mismatch"));
    }

    #[test]
    fn preparation_pair_validation_accepts_only_declared_fixture_delta() {
        let (control, treatment) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        assert!(validate_preparation_pair_launches(
            PreparationComparison::EquipmentPrepared,
            &control,
            &treatment,
        )
        .is_ok());

        let mut drifted = treatment.clone();
        drifted.common_fingerprint.world_tick = 1;
        assert!(validate_preparation_pair_launches(
            PreparationComparison::EquipmentPrepared,
            &control,
            &drifted,
        )
        .unwrap_err()
        .contains("common launch fingerprint mismatch"));

        let mut undeclared_bandage = treatment;
        undeclared_bandage.fixture.crude_bandages = 1;
        assert!(validate_preparation_pair_launches(
            PreparationComparison::EquipmentPrepared,
            &control,
            &undeclared_bandage,
        )
        .unwrap_err()
        .contains("fixture mismatch"));

        let (control, mut missing_starting_potion) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        missing_starting_potion.fixture.other_healing_items = 0;
        assert!(validate_preparation_pair_launches(
            PreparationComparison::EquipmentPrepared,
            &control,
            &missing_starting_potion,
        )
        .unwrap_err()
        .contains("fixture mismatch"));
    }

    #[test]
    fn preparation_pair_validation_rejects_resource_needs_effect_and_combat_state_drift() {
        let rejects = |treatment: &PreparationPairLaunch| {
            let (control, _) =
                valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
            assert!(validate_preparation_pair_launches(
                PreparationComparison::EquipmentPrepared,
                &control,
                treatment,
            )
            .unwrap_err()
            .contains("common launch fingerprint mismatch"));
        };

        let (_, mut stamina_drift) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        stamina_drift.common_fingerprint.hero_combat_stats.stamina = Some(1);
        rejects(&stamina_drift);

        let (_, mut mana_drift) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        mana_drift.common_fingerprint.hero_combat_stats.mana = Some(1);
        rejects(&mana_drift);

        let (_, mut needs_drift) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        needs_drift.common_fingerprint.hero_needs.hunger_bits ^= 1;
        rejects(&needs_drift);

        let (_, mut effects_drift) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        effects_drift
            .common_fingerprint
            .hero_effects
            .push(PreparationEffectFingerprint {
                effect: Effect::Bracing.to_str(),
                duration_or_deadline_tick: 10,
                amplifier_bits: 1.0_f32.to_bits(),
                stacks: 1,
            });
        rejects(&effects_drift);

        let (_, mut combat_tick_drift) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        combat_tick_drift.common_fingerprint.hero_last_combat_tick += 1;
        rejects(&combat_tick_drift);
    }

    #[test]
    fn preparation_pair_normalization_erases_only_comparison_artifacts() {
        let other_chest = PreparationInventoryFingerprint {
            name: "Copper Cuirass".to_string(),
            class: "Armor".to_string(),
            subclass: "Chest".to_string(),
            slot: Some("Chest".to_string()),
            quantity: 1,
            equipped: true,
        };
        assert_eq!(
            normalize_declared_inventory_artifact(
                PreparationComparison::EquipmentPrepared,
                other_chest.clone(),
            ),
            Some(other_chest)
        );
        assert_eq!(
            normalize_declared_inventory_artifact(
                PreparationComparison::EquipmentPrepared,
                expected_hide_wraps(true),
            ),
            Some(expected_hide_wraps(false))
        );
        assert_eq!(
            normalize_declared_inventory_artifact(
                PreparationComparison::EquipmentPrepared,
                expected_tattered_shirt(false),
            ),
            Some(expected_tattered_shirt(true))
        );

        let other_healing = PreparationInventoryFingerprint {
            name: "Herbal Poultice".to_string(),
            class: "Potion".to_string(),
            subclass: "Health".to_string(),
            slot: None,
            quantity: 1,
            equipped: false,
        };
        assert_eq!(
            normalize_declared_inventory_artifact(
                PreparationComparison::HealingPrepared,
                other_healing.clone(),
            ),
            Some(other_healing)
        );
        assert_eq!(
            normalize_declared_inventory_artifact(
                PreparationComparison::HealingPrepared,
                expected_crude_bandage(),
            ),
            None
        );

        let stockade = expected_preparation_stockade();
        assert!(is_declared_stockade_artifact(
            PreparationComparison::ExistingWalls,
            &stockade,
        ));
        assert!(!is_declared_stockade_artifact(
            PreparationComparison::EquipmentPrepared,
            &stockade,
        ));
        let mut other_stockade = stockade;
        other_stockade.position[0] -= 1;
        assert!(!is_declared_stockade_artifact(
            PreparationComparison::ExistingWalls,
            &other_stockade,
        ));
    }

    #[test]
    fn preparation_pair_validation_rejects_undeclared_inventory_drift() {
        let (control, mut treatment) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        treatment
            .common_fingerprint
            .normalized_inventory
            .push(PreparationInventoryFingerprint {
                name: "Copper Cuirass".to_string(),
                class: "Armor".to_string(),
                subclass: "Chest".to_string(),
                slot: Some("Chest".to_string()),
                quantity: 1,
                equipped: true,
            });
        assert!(validate_preparation_pair_launches(
            PreparationComparison::EquipmentPrepared,
            &control,
            &treatment,
        )
        .unwrap_err()
        .contains("common launch fingerprint mismatch"));

        let (control, mut treatment) =
            valid_preparation_test_launches(PreparationComparison::EquipmentPrepared);
        treatment.fixture.hide_wraps_items[0].class = "Clothing".to_string();
        assert!(validate_preparation_pair_launches(
            PreparationComparison::EquipmentPrepared,
            &control,
            &treatment,
        )
        .unwrap_err()
        .contains("fixture mismatch"));

        let (control, mut treatment) =
            valid_preparation_test_launches(PreparationComparison::HealingPrepared);
        treatment.fixture.crude_bandage_items[0].subclass = "Health".to_string();
        assert!(validate_preparation_pair_launches(
            PreparationComparison::HealingPrepared,
            &control,
            &treatment,
        )
        .unwrap_err()
        .contains("fixture mismatch"));
    }

    #[test]
    fn preparation_pair_validation_rejects_stockade_anchor_health_state_and_extra_drift() {
        let (control, treatment) =
            valid_preparation_test_launches(PreparationComparison::ExistingWalls);
        assert!(validate_preparation_pair_launches(
            PreparationComparison::ExistingWalls,
            &control,
            &treatment,
        )
        .is_ok());

        let mut wrong_anchor = treatment.clone();
        wrong_anchor.fixture.declared_anchor_stockades[0].position[0] -= 1;
        assert!(validate_preparation_pair_launches(
            PreparationComparison::ExistingWalls,
            &control,
            &wrong_anchor,
        )
        .unwrap_err()
        .contains("fixture mismatch"));

        let mut damaged = treatment.clone();
        damaged.fixture.declared_anchor_stockades[0].hp -= 1;
        assert!(validate_preparation_pair_launches(
            PreparationComparison::ExistingWalls,
            &control,
            &damaged,
        )
        .unwrap_err()
        .contains("fixture mismatch"));

        let mut unfinished = treatment.clone();
        unfinished.fixture.declared_anchor_stockades[0].state = "founded".to_string();
        assert!(validate_preparation_pair_launches(
            PreparationComparison::ExistingWalls,
            &control,
            &unfinished,
        )
        .unwrap_err()
        .contains("fixture mismatch"));

        let mut extra_stockade = treatment;
        let mut undeclared = expected_preparation_stockade();
        undeclared.position[0] -= 1;
        extra_stockade
            .common_fingerprint
            .normalized_structures
            .push(undeclared);
        assert!(validate_preparation_pair_launches(
            PreparationComparison::ExistingWalls,
            &control,
            &extra_stockade,
        )
        .unwrap_err()
        .contains("common launch fingerprint mismatch"));
    }

    #[test]
    fn world_view_occupancy_uses_production_blocking_state_semantics() {
        let mut game = HeadlessGame::new(100);
        game.spawn_hero("Warrior", "OccupancyProjectionBot");
        let before = game.observe();
        let mut open_positions = (0..crate::map::WIDTH)
            .flat_map(|x| (0..crate::map::HEIGHT).map(move |y| Position { x, y }))
            .filter(|position| !before.occupied.contains(&(position.x, position.y)));
        let blocking_position = open_positions.next().expect("first open position");
        let dead_position = open_positions.next().expect("second open position");

        game.app.world_mut().spawn((blocking_position, State::None));
        game.app.world_mut().spawn((dead_position, State::Dead));

        let after = game.observe();
        assert!(after
            .occupied
            .contains(&(blocking_position.x, blocking_position.y)));
        assert!(
            !after.occupied.contains(&(dead_position.x, dead_position.y)),
            "dead positioned entities must not become permanent bot path blockers"
        );
    }
}
