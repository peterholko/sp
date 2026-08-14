use bevy::ecs::query::{QueryData, WorldQuery};
use bevy::prelude::*;
use big_brain::thinker::ThinkerBuilder;

use rand::Rng;

use crate::constants::TICKS_PER_SEC;
use crate::crisis_balance::{
    is_live_built_human_core_structure, CrisisAttackTelemetryEvent, CrisisAttackTelemetryStage,
    CrisisCombatTelemetryEvent,
};
use crate::effect::{ControlEffectDiminishingReturns, ControlEffectDrEntry, Effect, Effects};
use crate::event::{MapEvents, Spell, VisibleEvent};
use crate::game::{CrisisAssaultUnit, Fortified, GameTick};
use crate::ids::Ids;
use crate::item::{self, AttrKey, Inventory, Item};
use crate::map::Map;
use crate::obj::Obj;
use crate::obj::{
    is_peaceful_interruptible_state, CancelEvents, Class, ClassStructure, HeroClass, Id,
    LastAttacker, LastCombatTick, LastDamageTick, Misc, PlayerId, Position, State, StateChange,
    StateDead, Stats, Subclass, Template,
};
use crate::skill::{SkillUpdated, Skills};
use crate::templates::{ComboTemplate, ObjTemplate, Templates};

pub const QUICK: &str = "quick";
pub const PRECISE: &str = "precise";
pub const FIERCE: &str = "fierce";

pub const HAMSTRING: &str = "Hamstring";
pub const GOUGE: &str = "Gouge";
pub const COMBO_CHAIN_TIMEOUT_TICKS: i32 = 150;
pub const CONTROL_EFFECT_DR_RESET_TICKS: i32 = 150;
pub const MAX_EFFECT_STACKS: i32 = 5;
pub const FORTIFICATION_REACH_WEAPON_SUBCLASS: &str = "Spear";

#[derive(Event, Debug, Clone, Copy)]
pub struct CombatEffectsChanged {
    pub target_id: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComboAreaProfile {
    pub damage_scale: Option<f32>,
    pub effect: Effect,
    pub effect_duration_scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttackType {
    Quick,
    Precise,
    Fierce,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackOptions {
    pub stamina_cost: i32,
    pub damage_bonus: i32,
}

impl Default for AttackOptions {
    fn default() -> Self {
        Self {
            stamina_cost: 5,
            damage_bonus: 0,
        }
    }
}

impl AttackType {
    pub fn to_str(self) -> String {
        match self {
            AttackType::Quick => QUICK.to_string(),
            AttackType::Precise => PRECISE.to_string(),
            AttackType::Fierce => FIERCE.to_string(),
        }
    }
}

#[derive(Debug, Clone, Reflect)]
pub enum Combo {
    Hamstring,
    Gouge,
    IntimidatingShout,
    ShroudedSlash,
    ShatterCleave,
    MassivePummel,
    NightmareStrike,
}

impl Combo {
    pub fn from_string(combo_string: &String) -> Self {
        match combo_string.as_str() {
            HAMSTRING => Combo::Hamstring,
            GOUGE => Combo::Gouge,
            //TODO finish the other combos
            _ => Combo::Hamstring,
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct ComboTracker {
    pub target_id: i32,
    pub attacks: Vec<AttackType>,
    pub last_attack_tick: i32,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct CombatQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub class: &'static Class,
    pub class_structure: Option<&'static ClassStructure>,
    pub subclass: &'static Subclass,
    pub template: &'static Template,
    pub state: &'static mut State,
    pub misc: &'static mut Misc,
    pub stats: &'static mut Stats,
    pub effects: &'static mut Effects,
    pub control_effect_dr: Option<&'static mut ControlEffectDiminishingReturns>,
    pub fortified: Option<&'static Fortified>,
    pub inventory: &'static mut Inventory,
    pub skills: Option<&'static mut Skills>,
    pub hero_class: Option<&'static HeroClass>,
    pub combo_tracker: Option<&'static mut ComboTracker>,
    pub last_combat_tick: &'static mut LastCombatTick,
    pub crisis_assault: Option<&'static CrisisAssaultUnit>,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct CombatSpellQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub class: &'static Class,
    pub class_structure: Option<&'static ClassStructure>,
    pub subclass: &'static Subclass,
    pub template: &'static Template,
    pub state: &'static mut State,
    pub misc: &'static mut Misc,
    pub stats: &'static mut Stats,
    pub effects: &'static mut Effects,
    pub fortified: Option<&'static Fortified>,
    pub last_combat_tick: &'static mut LastCombatTick,
    pub crisis_assault: Option<&'static CrisisAssaultUnit>,
}

#[derive(Debug, Clone)]
pub struct Combat;

impl Combat {
    pub(crate) fn emit_crisis_attack_telemetry(
        commands: &mut Commands,
        game_tick: i32,
        stage: CrisisAttackTelemetryStage,
        attacker: &CombatQueryItem,
        target: &CombatQueryItem,
    ) {
        let attacker_crisis = attacker.crisis_assault.copied();
        let target_crisis = target.crisis_assault.copied();
        if attacker_crisis.is_none() && target_crisis.is_none() {
            return;
        }
        commands.trigger(CrisisAttackTelemetryEvent {
            entity: target.entity,
            game_tick,
            stage,
            attacker_id: attacker.id.0,
            attacker_player_id: attacker.player_id.0,
            attacker_subclass: *attacker.subclass,
            attacker_crisis,
            target_id: target.id.0,
            target_player_id: target.player_id.0,
            target_subclass: *target.subclass,
            target_is_structure: target.class.is_structure(),
            target_is_core_structure: is_live_built_human_core_structure(
                target.class_structure,
                target.class,
                target.player_id,
                *target.subclass,
                *target.state,
                false,
            ),
            target_crisis,
        });
    }

    pub(crate) fn emit_crisis_combat_telemetry(
        commands: &mut Commands,
        game_tick: i32,
        attacker: &CombatQueryItem,
        target: &CombatQueryItem,
        hp_before: i32,
        target_was_core_structure: bool,
    ) {
        let attacker_crisis = attacker.crisis_assault.copied();
        let target_crisis = target.crisis_assault.copied();
        if attacker_crisis.is_none() && target_crisis.is_none() {
            return;
        }
        let effective_damage = hp_before.max(0).saturating_sub(target.stats.hp.max(0));
        commands.trigger(CrisisCombatTelemetryEvent {
            entity: target.entity,
            game_tick,
            attacker_id: attacker.id.0,
            attacker_player_id: attacker.player_id.0,
            attacker_subclass: *attacker.subclass,
            attacker_crisis,
            target_id: target.id.0,
            target_player_id: target.player_id.0,
            target_subclass: *target.subclass,
            target_is_structure: target.class.is_structure(),
            target_is_core_structure: target_was_core_structure,
            target_crisis,
            effective_damage,
            killed: target.stats.hp <= 0,
        });
    }

    pub(crate) fn emit_crisis_spell_telemetry(
        commands: &mut Commands,
        game_tick: i32,
        attacker: &CombatSpellQueryItem,
        target: &CombatSpellQueryItem,
        hp_before: i32,
        target_was_core_structure: bool,
    ) {
        let attacker_crisis = attacker.crisis_assault.copied();
        let target_crisis = target.crisis_assault.copied();
        if attacker_crisis.is_none() && target_crisis.is_none() {
            return;
        }
        let effective_damage = hp_before.max(0).saturating_sub(target.stats.hp.max(0));
        commands.trigger(CrisisCombatTelemetryEvent {
            entity: target.entity,
            game_tick,
            attacker_id: attacker.id.0,
            attacker_player_id: attacker.player_id.0,
            attacker_subclass: *attacker.subclass,
            attacker_crisis,
            target_id: target.id.0,
            target_player_id: target.player_id.0,
            target_subclass: *target.subclass,
            target_is_structure: target.class.is_structure(),
            target_is_core_structure: target_was_core_structure,
            target_crisis,
            effective_damage,
            killed: target.stats.hp <= 0,
        });
    }

    pub fn class_template_is_attackable(class: &Class, _template: &Template) -> bool {
        !class.is_poi()
    }

    pub fn non_attackable_class_template_error(
        class: &Class,
        template: &Template,
    ) -> Option<String> {
        if template.0 == "Shipwreck" {
            Some("The shipwreck can only be inspected, not attacked.".to_string())
        } else if !Self::class_template_is_attackable(class, template) {
            Some("That cannot be attacked.".to_string())
        } else {
            None
        }
    }

    pub fn target_is_attackable(target: &CombatQueryItem) -> bool {
        Self::class_template_is_attackable(target.class, target.template)
    }

    pub fn non_attackable_target_error(target: &CombatQueryItem) -> Option<String> {
        Self::non_attackable_class_template_error(target.class, target.template)
    }

    pub fn target_is_fortified(target: &CombatQueryItem) -> bool {
        target.effects.has(Effect::Fortified)
    }

    pub fn fortified_target_melee_error(target: &CombatQueryItem) -> Option<String> {
        if Self::target_is_fortified(target) {
            Some("Only ranged attacks can hit a fortified target.".to_string())
        } else {
            None
        }
    }

    // Combat breaks stealth. Returns (reveal_attacker, reveal_target). The
    // attacker always reveals (it never dies from its own swing); the target
    // reveals only if it survived — a slain target is shown as a corpse rather
    // than a revealed unit.
    pub fn combat_reveals(
        attacker_state: State,
        target_state: State,
        target_hp: i32,
    ) -> (bool, bool) {
        (
            attacker_state == State::Hiding,
            target_hp > 0 && target_state == State::Hiding,
        )
    }

    pub fn fortified_outbound_attack_error(
        attacker_effects: &Effects,
        attacker_fortified: Option<&Fortified>,
        target_effects: &Effects,
        target_fortified: Option<&Fortified>,
        can_attack_outbound: bool,
    ) -> Option<String> {
        if !attacker_effects.has(Effect::Fortified) {
            return None;
        }

        if target_effects.has(Effect::Fortified) {
            if let (Some(attacker_fortified), Some(target_fortified)) =
                (attacker_fortified, target_fortified)
            {
                if attacker_fortified.id == target_fortified.id {
                    return None;
                }
            }
        }

        if !can_attack_outbound {
            return Some(
                "Only ranged attacks or attacks with an equipped Spear can be used from behind a wall."
                    .to_string(),
            );
        }

        None
    }

    pub fn equipped_weapon_has_fortification_reach(inventory: &Inventory) -> bool {
        inventory.get_equipped_main_hand().is_some_and(|weapon| {
            weapon.quantity > 0
                && weapon.class == item::WEAPON
                && weapon.subclass == FORTIFICATION_REACH_WEAPON_SUBCLASS
        })
    }

    pub fn fortified_outbound_attack_error_from_combat(
        attacker: &CombatQueryItem,
        target: &CombatQueryItem,
        can_attack_outbound: bool,
    ) -> Option<String> {
        Self::fortified_outbound_attack_error(
            &attacker.effects,
            attacker.fortified,
            &target.effects,
            target.fortified,
            can_attack_outbound,
        )
    }

    pub fn fortified_outbound_attack_error_from_spell(
        attacker: &CombatSpellQueryItem,
        target: &CombatSpellQueryItem,
        can_attack_outbound: bool,
    ) -> Option<String> {
        Self::fortified_outbound_attack_error(
            &attacker.effects,
            attacker.fortified,
            &target.effects,
            target.fortified,
            can_attack_outbound,
        )
    }

    // BB-A: a defensive stance hard-counters one attack type. When the
    // defender's active stance matches the incoming attack, return the design
    // reduction (dodge -100%, parry -75%, brace -50%) plus a counter label.
    fn get_defend_stance_mod(
        attack_type: &AttackType,
        target: &CombatQueryItem,
    ) -> (f32, Option<String>) {
        match attack_type {
            AttackType::Quick if target.effects.has(Effect::Dodging) => {
                (0.0, Some("Dodged".to_string()))
            }
            AttackType::Precise if target.effects.has(Effect::Parrying) => {
                (0.25, Some("Parried".to_string()))
            }
            AttackType::Fierce if target.effects.has(Effect::Bracing) => {
                (0.5, Some("Braced".to_string()))
            }
            _ => (1.0, None),
        }
    }

    pub fn process_attack(
        attack_type: AttackType,
        attacker: &mut CombatQueryItem,
        target: &mut CombatQueryItem,
        commands: &mut Commands,
        templates: &Res<Templates>,
        map: &Res<Map>,
        _ids: &mut ResMut<Ids>,
        game_tick: &Res<GameTick>,
        _map_events: &mut ResMut<MapEvents>,
    ) -> (i32, Option<String>, Option<SkillUpdated>, Option<String>) {
        Self::process_attack_with_options(
            attack_type,
            attacker,
            target,
            commands,
            templates,
            map,
            _ids,
            game_tick,
            _map_events,
            AttackOptions::default(),
        )
    }

    pub fn process_attack_with_options(
        attack_type: AttackType,
        attacker: &mut CombatQueryItem,
        target: &mut CombatQueryItem,
        commands: &mut Commands,
        templates: &Res<Templates>,
        map: &Res<Map>,
        _ids: &mut ResMut<Ids>,
        game_tick: &Res<GameTick>,
        _map_events: &mut ResMut<MapEvents>,
        options: AttackOptions,
    ) -> (i32, Option<String>, Option<SkillUpdated>, Option<String>) {
        let target_template = templates.obj_templates.get(target.template.0.clone());

        // Keep the equipped weapons for proc rolls and kill XP attribution.
        let attacker_weapons = attacker.inventory.get_equipped_weapons();
        debug!("Attacker_weapons: {:?}", attacker_weapons);

        // Basic attacks and finishers share the same damage pipeline below.
        let attack_type_damage_mod = Self::attack_type_damage_mod(attack_type.clone());

        // 10 Check if defender has a matched defensive stance (BB-A).
        let (defend_stance_mod, countered) = Self::get_defend_stance_mod(&attack_type, target);

        // 11 & 12 Add attack type to attack list
        Self::add_attack_to_combo_tracker(
            commands,
            templates,
            attack_type,
            attacker,
            target,
            game_tick.0,
        );

        // 13 & 14 A successful counter interrupts the attacker's combo sequence.
        if countered.is_some() {
            if let Some(combo_tracker) = &mut attacker.combo_tracker {
                combo_tracker.attacks.clear();
                combo_tracker.target_id = -1;
            }
        }

        // 15 Calculate combo damage and apply combo effects
        /*let (combo_quick_damage_mod, combo_precise_damage_mod, combo_fierce_damage_mod) =
            Self::get_combo_damage(combo_template.clone());

        let combo_damage_mod =
            combo_quick_damage_mod * combo_precise_damage_mod * combo_fierce_damage_mod;
        debug!("combo_damage_mod: {:?}", combo_damage_mod);*/

        // TODO 16 Check if target is fortified

        let (total_damage, dealt_damage) = Self::resolve_damage(
            attacker,
            target,
            templates,
            map,
            attack_type_damage_mod,
            options.damage_bonus,
            defend_stance_mod,
        );

        // 26 Update Hp and check if target is dead
        let target_hp_before = target.stats.hp;
        let target_was_core_structure = is_live_built_human_core_structure(
            target.class_structure,
            target.class,
            target.player_id,
            *target.subclass,
            *target.state,
            false,
        );
        target.stats.hp -= dealt_damage;
        if dealt_damage > 0 {
            commands
                .entity(target.entity)
                .try_insert(LastDamageTick(game_tick.0));
        }

        // 27 Update stamina - reduce by 5 per attack
        let attacker_stamina = attacker.stats.stamina.expect("Missing stamina stat");
        attacker.stats.stamina = Some(attacker_stamina - options.stamina_cost);

        // Update last combat tick for both attacker and target (used for stamina regen rate)
        attacker.last_combat_tick.0 = game_tick.0;
        target.last_combat_tick.0 = game_tick.0;
        Self::interrupt_peaceful_work(commands, attacker.entity, &attacker.state);
        Self::interrupt_peaceful_work(commands, target.entity, &target.state);

        if attacker.player_id.0 != target.player_id.0 {
            commands.entity(target.entity).insert(LastAttacker {
                id: attacker.id.0,
                tick: game_tick.0,
            });
        }

        if matches!(target.hero_class, Some(&HeroClass::Warrior))
            && target.effects.0.contains_key(&Effect::Bracing)
        {
            if let (Some(stamina), Some(base_stamina)) =
                (target.stats.stamina, target.stats.base_stamina)
            {
                target.stats.stamina = Some((stamina + 3).min(base_stamina));
            }
        }

        // 28 Apply new effects from this attack
        /*Self::apply_combo_effects(
            combo_template.clone(),
            templates,
            attacker,
            target,
            ids,
            game_tick,
            map_events,
        );*/

        // 29 Check if any weapons procced
        Self::process_weapon_procs(
            commands,
            templates,
            &attacker_weapons,
            target,
            game_tick.0,
            _map_events,
        );

        // 30 & 31 Check if target is dead and update skills
        let mut skill_updated = None;

        debug!("Target HP: {:?}", target.stats.hp);

        if target.stats.hp <= 0 {
            *target.state = State::Dead;

            debug!("Target {:?} is dead", target.entity);
            commands.entity(target.entity).insert(StateDead {
                dead_at: game_tick.0,
                killer: attacker.template.0.clone(),
            });
            commands.trigger(StateChange {
                entity: target.entity,
                new_state: State::Dead,
            });

            commands.entity(target.entity).remove::<ThinkerBuilder>();

            for item in attacker_weapons.iter() {
                skill_updated = Some(SkillUpdated {
                    id: attacker.id.0,
                    xp_type: item.subclass.to_string(),
                    xp: target_template.kill_xp.unwrap_or(0),
                });
            }
        }

        Self::emit_crisis_combat_telemetry(
            commands,
            game_tick.0,
            attacker,
            target,
            target_hp_before,
            target_was_core_structure,
        );

        debug!("Total Damage: {:?}", total_damage);

        // Combat breaks stealth. Reveal the combatants flagged below; the
        // tick they leave Hiding, `reveal_unhidden_system` refreshes nearby
        // players' perception so the now-visible unit re-appears on clients.
        let (reveal_attacker, reveal_target) =
            Self::combat_reveals(*attacker.state, *target.state, target.stats.hp);
        if reveal_attacker {
            commands.trigger(StateChange {
                entity: attacker.entity,
                new_state: State::None,
            });
        }
        if reveal_target {
            commands.trigger(StateChange {
                entity: target.entity,
                new_state: State::None,
            });
        }

        // Return combo name
        /*let mut combo_name = None;

        if let Some(combo) = combo_template {
            combo_name = Some(combo.name);
        }*/

        // Report the damage actually dealt (post armor + matched stance) so the
        // client shows the real number and a successful counter reads as 0/low.
        return (dealt_damage, None, skill_updated, countered);
    }

    pub fn process_combo(
        attacker: &mut CombatQueryItem,
        target: &mut CombatQueryItem,
        commands: &mut Commands,
        templates: &Res<Templates>,
        map: &Res<Map>,
        ids: &mut ResMut<Ids>,
        game_tick: &Res<GameTick>,
        map_events: &mut ResMut<MapEvents>,
    ) -> (i32, Option<String>, Option<SkillUpdated>) {
        let target_template = templates.obj_templates.get(target.template.0.clone());

        // Keep the equipped weapons for proc rolls and kill XP attribution.
        let attacker_weapons = attacker.inventory.get_equipped_weapons();
        debug!("Attacker_weapons: {:?}", attacker_weapons);

        // 11 & 12 Add attack type to attack list and check if combo is completed
        let combo_template = Self::find_combo(commands, templates, attacker, target);
        debug!("process_combo::combo_template: {:?}", combo_template);

        // TODO 13 Check if combo is countered

        // TODO 14 Remove Defense Stanc Effect if combo countered

        // 15 Calculate combo damage and apply combo effects
        let (combo_quick_damage_mod, combo_precise_damage_mod, combo_fierce_damage_mod) =
            Self::get_combo_damage(combo_template.clone());

        let combo_damage_mod =
            combo_quick_damage_mod * combo_precise_damage_mod * combo_fierce_damage_mod;
        debug!("combo_damage_mod: {:?}", combo_damage_mod);

        // TODO 16 Check if target is fortified

        let (total_damage, dealt_damage) =
            Self::resolve_damage(attacker, target, templates, map, combo_damage_mod, 0, 1.0);

        // 26 Update Hp and check if target is dead
        let target_hp_before = target.stats.hp;
        let target_was_core_structure = is_live_built_human_core_structure(
            target.class_structure,
            target.class,
            target.player_id,
            *target.subclass,
            *target.state,
            false,
        );
        target.stats.hp -= dealt_damage;
        if dealt_damage > 0 {
            commands
                .entity(target.entity)
                .try_insert(LastDamageTick(game_tick.0));
        }

        // 27 Update stamina - reduce by 5 per attack
        let attacker_stamina = attacker.stats.stamina.expect("Missing stamina stat");
        attacker.stats.stamina = Some(attacker_stamina - 5);

        // Update last combat tick for both attacker and target (used for stamina regen rate)
        attacker.last_combat_tick.0 = game_tick.0;
        target.last_combat_tick.0 = game_tick.0;
        Self::interrupt_peaceful_work(commands, attacker.entity, &attacker.state);
        Self::interrupt_peaceful_work(commands, target.entity, &target.state);

        if attacker.player_id.0 != target.player_id.0 {
            commands.entity(target.entity).insert(LastAttacker {
                id: attacker.id.0,
                tick: game_tick.0,
            });
        }

        if matches!(target.hero_class, Some(&HeroClass::Warrior))
            && target.effects.0.contains_key(&Effect::Bracing)
        {
            if let (Some(stamina), Some(base_stamina)) =
                (target.stats.stamina, target.stats.base_stamina)
            {
                target.stats.stamina = Some((stamina + 3).min(base_stamina));
            }
        }

        // 28 Apply new effects from this attack
        Self::apply_combo_effects(
            combo_template.clone(),
            commands,
            templates,
            attacker,
            target,
            ids,
            game_tick,
            map_events,
            1.0,
        );

        // 29 Check if any weapons procced
        Self::process_weapon_procs(
            commands,
            templates,
            &attacker_weapons,
            target,
            game_tick.0,
            map_events,
        );

        // 30 & 31 Check if target is dead and update skills
        let mut skill_updated = None;

        debug!("Target HP: {:?}", target.stats.hp);

        if target.stats.hp <= 0 {
            *target.state = State::Dead;

            debug!("Target {:?} is dead", target.entity);
            commands.entity(target.entity).insert(StateDead {
                dead_at: game_tick.0,
                killer: attacker.template.0.clone(),
            });
            commands.trigger(StateChange {
                entity: target.entity,
                new_state: State::Dead,
            });

            commands.entity(target.entity).remove::<ThinkerBuilder>();
            //commands.entity(target.entity).despawn();

            for item in attacker_weapons.iter() {
                skill_updated = Some(SkillUpdated {
                    id: attacker.id.0,
                    xp_type: item.subclass.to_string(),
                    xp: target_template.kill_xp.unwrap_or(0),
                });
            }
        }

        Self::emit_crisis_combat_telemetry(
            commands,
            game_tick.0,
            attacker,
            target,
            target_hp_before,
            target_was_core_structure,
        );

        debug!("Total Damage: {:?}", total_damage);

        // Return combo name
        let mut combo_name = None;

        if let Some(combo) = combo_template {
            combo_name = Some(combo.name);
        }

        return (dealt_damage, combo_name, skill_updated);
    }

    pub fn combo_area_profile(combo_name: &str) -> Option<ComboAreaProfile> {
        match combo_name {
            "Intimidating Shout" => Some(ComboAreaProfile {
                damage_scale: None,
                effect: Effect::Fear,
                effect_duration_scale: 1.0,
            }),
            "Shatter Cleave" => Some(ComboAreaProfile {
                damage_scale: Some(0.5),
                effect: Effect::Bleed,
                effect_duration_scale: 1.0,
            }),
            "Massive Pummel" => Some(ComboAreaProfile {
                damage_scale: None,
                effect: Effect::Concussed,
                effect_duration_scale: 0.5,
            }),
            _ => None,
        }
    }

    pub fn valid_combo_secondary(
        attacker_pos: Position,
        primary_target_id: i32,
        target: &CombatQueryItem,
    ) -> bool {
        Self::valid_combo_secondary_fields(
            attacker_pos,
            primary_target_id,
            target.id.0,
            target.player_id.0,
            target.class,
            *target.state,
            target.stats.hp,
            *target.pos,
            Self::target_is_fortified(target),
        ) && Self::target_is_attackable(target)
    }

    fn valid_combo_secondary_fields(
        attacker_pos: Position,
        primary_target_id: i32,
        target_id: i32,
        target_player_id: i32,
        target_class: &Class,
        target_state: State,
        target_hp: i32,
        target_pos: Position,
        fortified: bool,
    ) -> bool {
        target_id != primary_target_id
            && target_player_id >= 1000
            && target_class.0 == crate::constants::CLASS_UNIT
            && target_state != State::Dead
            && target_hp > 0
            && Map::dist(attacker_pos, target_pos) <= 1
            && !fortified
    }

    pub fn process_combo_secondary(
        attacker: &mut CombatQueryItem,
        target: &mut CombatQueryItem,
        combo_template: &ComboTemplate,
        profile: &ComboAreaProfile,
        commands: &mut Commands,
        templates: &Res<Templates>,
        map: &Res<Map>,
        game_tick: &Res<GameTick>,
        map_events: &mut ResMut<MapEvents>,
    ) -> (i32, Option<SkillUpdated>) {
        let attacker_weapons = attacker.inventory.get_equipped_weapons();
        let target_template = templates.obj_templates.get(target.template.0.clone());
        let dealt_damage = profile.damage_scale.map_or(0, |damage_scale| {
            let (quick, precise, fierce) = Self::get_combo_damage(Some(combo_template.clone()));
            Self::resolve_damage(
                attacker,
                target,
                templates,
                map,
                quick * precise * fierce * damage_scale,
                0,
                1.0,
            )
            .1
        });

        let target_hp_before = target.stats.hp;
        let target_was_core_structure = is_live_built_human_core_structure(
            target.class_structure,
            target.class,
            target.player_id,
            *target.subclass,
            *target.state,
            false,
        );
        target.stats.hp = target.stats.hp.saturating_sub(dealt_damage);
        if dealt_damage > 0 {
            commands
                .entity(target.entity)
                .try_insert(LastDamageTick(game_tick.0));
        }

        attacker.last_combat_tick.0 = game_tick.0;
        target.last_combat_tick.0 = game_tick.0;
        Self::interrupt_peaceful_work(commands, target.entity, &target.state);
        if attacker.player_id.0 != target.player_id.0 {
            commands.entity(target.entity).try_insert(LastAttacker {
                id: attacker.id.0,
                tick: game_tick.0,
            });
        }

        let effect_template = templates
            .effect_templates
            .get(&profile.effect.clone().to_str())
            .expect("AoE combo effect missing from templates");
        Self::apply_timed_combat_effect(
            commands,
            target,
            profile.effect.clone(),
            effect_template,
            game_tick.0,
            map_events,
            profile.effect_duration_scale,
        );

        let mut skill_updated = None;
        if target.stats.hp <= 0 {
            *target.state = State::Dead;
            commands
                .entity(target.entity)
                .try_insert(StateDead {
                    dead_at: game_tick.0,
                    killer: attacker.template.0.clone(),
                })
                .try_remove::<ThinkerBuilder>();
            commands.trigger(StateChange {
                entity: target.entity,
                new_state: State::Dead,
            });
            for item in &attacker_weapons {
                skill_updated = Some(SkillUpdated {
                    id: attacker.id.0,
                    xp_type: item.subclass.to_string(),
                    xp: target_template.kill_xp.unwrap_or(0),
                });
            }
        }

        Self::emit_crisis_combat_telemetry(
            commands,
            game_tick.0,
            attacker,
            target,
            target_hp_before,
            target_was_core_structure,
        );
        let (reveal_attacker, reveal_target) =
            Self::combat_reveals(*attacker.state, *target.state, target.stats.hp);
        if reveal_attacker {
            commands.trigger(StateChange {
                entity: attacker.entity,
                new_state: State::None,
            });
        }
        if reveal_target {
            commands.trigger(StateChange {
                entity: target.entity,
                new_state: State::None,
            });
        }

        (dealt_damage, skill_updated)
    }

    pub fn process_spell_damage(
        commands: &mut Commands,
        game_tick: &Res<GameTick>,
        spell: Spell,
        caster: &CombatSpellQueryItem,
        target: &mut CombatSpellQueryItem,
    ) -> i32 {
        let damage = match spell {
            Spell::ShadowBolt => 1,
            Spell::ArcaneBolt => 12,
        };
        let target_hp_before = target.stats.hp;
        let target_was_core_structure = is_live_built_human_core_structure(
            target.class_structure,
            target.class,
            target.player_id,
            *target.subclass,
            *target.state,
            false,
        );
        target.stats.hp -= damage;
        commands
            .entity(target.entity)
            .try_insert(LastDamageTick(game_tick.0));

        if target.stats.hp <= 0 {
            *target.state = State::Dead;
            debug!("Target {:?} is dead", target.entity);
            commands
                .entity(target.entity)
                .try_insert(StateDead {
                    dead_at: game_tick.0,
                    killer: caster.template.0.clone(),
                })
                .try_remove::<ThinkerBuilder>();
            commands.trigger(StateChange {
                entity: target.entity,
                new_state: State::Dead,
            });
        }

        Self::emit_crisis_spell_telemetry(
            commands,
            game_tick.0,
            caster,
            target,
            target_hp_before,
            target_was_core_structure,
        );

        return damage;
    }

    fn process_weapon_procs(
        commands: &mut Commands,
        templates: &Res<Templates>,
        attacker_weapons: &Vec<Item>,
        target: &mut CombatQueryItem,
        game_tick: i32,
        map_events: &mut ResMut<MapEvents>,
    ) {
        let mut rng = rand::thread_rng();

        for weapon in attacker_weapons.iter() {
            debug!("weapon: {:?}", weapon);

            for proc_attr_key in AttrKey::proc_iter() {
                if let Some(attr_val) = weapon.attrs.get(&proc_attr_key) {
                    debug!("attr_val: {:?}", attr_val);
                    let chance = match attr_val {
                        item::AttrVal::Num(chance) => *chance,
                        _ => panic!("Invalid attr value"),
                    };

                    let roll = rng.gen_range(0.0..1.0);

                    debug!("roll: {:?} chance: {:?}", roll, chance);

                    if roll <= chance {
                        let effect = proc_attr_key.clone().proc_to_effect();
                        debug!("proc effect: {:?}", effect);

                        let effect_string = effect.clone().to_str();

                        let effect_template = templates
                            .effect_templates
                            .get(&effect_string)
                            .expect("Cannot find template for effect");

                        Self::apply_timed_combat_effect(
                            commands,
                            target,
                            effect,
                            effect_template,
                            game_tick,
                            map_events,
                            1.0,
                        );

                        debug!("effects: {:?}", target.effects.0);
                    }
                }
            }
        }
    }

    fn add_attack_to_combo_tracker(
        commands: &mut Commands,
        templates: &Res<Templates>,
        attack_type: AttackType,
        attacker: &mut CombatQueryItem,
        target: &mut CombatQueryItem,
        game_tick: i32,
    ) {
        // Only allow combos for players
        if attacker.player_id.0 < 1000 {
            debug!("check combo_tracker: {:?}", attacker.combo_tracker);

            if let Some(combo_tracker) = &mut attacker.combo_tracker {
                let previous = if combo_tracker.target_id == target.id.0
                    && game_tick.saturating_sub(combo_tracker.last_attack_tick)
                        <= COMBO_CHAIN_TIMEOUT_TICKS
                {
                    combo_tracker.attacks.clone()
                } else {
                    Vec::new()
                };
                combo_tracker.target_id = target.id.0;
                combo_tracker.attacks = Self::next_combo_attacks(&previous, attack_type, templates);
                combo_tracker.last_attack_tick = game_tick;
            } else {
                let combo_tracker = ComboTracker {
                    target_id: target.id.0,
                    attacks: Self::next_combo_attacks(&[], attack_type, templates),
                    last_attack_tick: game_tick,
                };

                commands.entity(attacker.entity).insert(combo_tracker);
            }

            debug!("post check combo_tracker {:?}", attacker.combo_tracker);
        }
    }

    pub(crate) fn next_combo_attacks(
        previous: &[AttackType],
        attack_type: AttackType,
        templates: &Templates,
    ) -> Vec<AttackType> {
        let mut candidate = previous.to_vec();
        candidate.push(attack_type);

        for suffix_start in 0..candidate.len() {
            let suffix = &candidate[suffix_start..];
            let suffix_is_live = templates.combo_templates.iter().any(|(_, combo)| {
                suffix.len() <= combo.attacks.len()
                    && suffix
                        .iter()
                        .zip(combo.attacks.iter())
                        .all(|(attack, expected)| attack.clone().to_str() == *expected)
            });
            if suffix_is_live {
                return suffix.to_vec();
            }
        }

        Vec::new()
    }

    pub(crate) fn live_combo_attacks_before_append(
        tracker: Option<&ComboTracker>,
        target_id: i32,
        game_tick: i32,
    ) -> Vec<AttackType> {
        tracker
            .filter(|tracker| {
                tracker.target_id == target_id
                    && game_tick.saturating_sub(tracker.last_attack_tick)
                        <= COMBO_CHAIN_TIMEOUT_TICKS
            })
            .map(|tracker| tracker.attacks.clone())
            .unwrap_or_default()
    }

    fn find_combo(
        _commands: &mut Commands,
        templates: &Res<Templates>,
        attacker: &mut CombatQueryItem,
        _target: &mut CombatQueryItem,
    ) -> Option<ComboTemplate> {
        let mut combo = None;
        // Only allow combos for players
        if attacker.player_id.0 < 1000 {
            debug!("check combo_tracker: {:?}", attacker.combo_tracker);

            if let Some(combo_tracker) = &mut attacker.combo_tracker {
                let mut attacks_str = Vec::new();

                for attack in combo_tracker.attacks.iter() {
                    attacks_str.push(attack.clone().to_str());
                }

                debug!("attack_str: {:?}", attacks_str);

                for (_combo_name, combo_template) in templates.combo_templates.iter() {
                    debug!("combo_template.attacks: {:?}", combo_template.attacks);
                    if combo_template.attacks == attacks_str {
                        combo = Some(combo_template.clone());
                        break;
                    }
                }
                // Clear attacks even if combo wasn't found
                combo_tracker.attacks.clear();
            }
        }

        return combo;
    }

    fn apply_combo_effects(
        combo: Option<ComboTemplate>,
        commands: &mut Commands,
        templates: &Res<Templates>,
        _attacker: &mut CombatQueryItem,
        target: &mut CombatQueryItem,
        _ids: &mut ResMut<Ids>,
        game_tick: &Res<GameTick>,
        map_events: &mut ResMut<MapEvents>,
        duration_scale: f32,
    ) {
        if let Some(combo_template) = combo {
            for effect_name in combo_template.effects.iter() {
                debug!("combo_template.effect: {:?}", combo_template.effects);

                let effect_template = templates
                    .effect_templates
                    .get(&effect_name.clone())
                    .expect("Effect missing from templates");
                debug!("effect_template: {:?}", effect_template);
                let effect = Effect::from_string(&effect_template.name);

                debug!("Effect applied: {:?}", effect);
                Self::apply_timed_combat_effect(
                    commands,
                    target,
                    effect,
                    effect_template,
                    game_tick.0,
                    map_events,
                    duration_scale,
                );
            }
        }
    }

    fn is_control_effect(effect: &Effect) -> bool {
        matches!(
            effect,
            Effect::Stunned | Effect::Fear | Effect::Concussed | Effect::Hamstrung
        )
    }

    fn control_effect_duration_multiplier(
        dr: &mut ControlEffectDiminishingReturns,
        effect: &Effect,
        game_tick: i32,
    ) -> Option<f32> {
        let entry = dr.0.entry(effect.clone()).or_insert(ControlEffectDrEntry {
            stage: 0,
            last_applied_tick: game_tick,
        });
        if game_tick.saturating_sub(entry.last_applied_tick) >= CONTROL_EFFECT_DR_RESET_TICKS {
            entry.stage = 0;
        }

        let multiplier = match entry.stage {
            0 => Some(1.0),
            1 => Some(0.5),
            2 => Some(0.25),
            _ => None,
        };
        if multiplier.is_some() {
            entry.stage = entry.stage.saturating_add(1).min(3);
            entry.last_applied_tick = game_tick;
        }
        multiplier
    }

    fn apply_timed_combat_effect(
        commands: &mut Commands,
        target: &mut CombatQueryItem,
        effect: Effect,
        effect_template: &crate::templates::EffectTemplate,
        game_tick: i32,
        map_events: &mut ResMut<MapEvents>,
        duration_scale: f32,
    ) -> bool {
        let dr_multiplier = if Self::is_control_effect(&effect) {
            if let Some(dr) = target.control_effect_dr.as_deref_mut() {
                let Some(multiplier) =
                    Self::control_effect_duration_multiplier(dr, &effect, game_tick)
                else {
                    return false;
                };
                multiplier
            } else {
                let mut dr = ControlEffectDiminishingReturns::default();
                let multiplier =
                    Self::control_effect_duration_multiplier(&mut dr, &effect, game_tick)
                        .expect("first control application is never immune");
                commands.entity(target.entity).try_insert(dr);
                multiplier
            }
        } else {
            1.0
        };

        let duration_ticks =
            ((effect_template.duration * TICKS_PER_SEC) as f32 * duration_scale * dr_multiplier)
                .round() as i32;
        if duration_ticks <= 0 {
            return false;
        }

        let expires_at = game_tick.saturating_add(duration_ticks);
        Self::record_timed_effect(
            target.id.0,
            &mut target.effects,
            effect,
            effect_template.stackable.unwrap_or(false),
            expires_at,
            map_events,
        );
        true
    }

    fn record_timed_effect(
        target_id: i32,
        effects: &mut Effects,
        effect: Effect,
        stackable: bool,
        expires_at: i32,
        map_events: &mut MapEvents,
    ) {
        let stacks = Self::next_effect_stack_count(
            effects.0.get(&effect).map(|(_, _, stacks)| *stacks),
            stackable,
        );
        effects.0.insert(effect.clone(), (expires_at, 1.0, stacks));
        map_events.new(
            target_id,
            expires_at,
            VisibleEvent::EffectExpiredEvent { effect },
        );
    }

    fn next_effect_stack_count(current: Option<i32>, stackable: bool) -> i32 {
        if stackable {
            current
                .unwrap_or(0)
                .saturating_add(1)
                .min(MAX_EFFECT_STACKS)
        } else {
            1
        }
    }

    fn get_combo_damage(combo_template: Option<ComboTemplate>) -> (f32, f32, f32) {
        if let Some(combo_template) = combo_template {
            return (
                combo_template.quick_damage,
                combo_template.precise_damage,
                combo_template.fierce_damage,
            );
        } else {
            return (1.0, 1.0, 1.0);
        }
    }

    // Value returned is between 0.0 and 1.0
    fn get_damage_effects(attacker: &mut CombatQueryItem, templates: &Res<Templates>) -> f32 {
        for (effect, (_duration, _amplifier, _stacks)) in attacker.effects.0.iter() {
            let effect_template = templates
                .effect_templates
                .get(&effect.clone().to_str())
                .expect("Effect missing from templates");

            if let Some(effect_damage) = effect_template.damage {
                let modifier = 1.0 + effect_damage; // atk is negative in the template file
                return modifier;
            }
        }

        // No modifier if 1.0 is returned
        return 1.0;
    }

    fn get_defense_effects(target: &mut CombatQueryItem, templates: &Res<Templates>) -> f32 {
        for (effect, (_duration, amplifier, _stacks)) in target.effects.0.iter() {
            if *effect == Effect::Sanctuary {
                continue;
            }

            let effect_template = templates
                .effect_templates
                .get(&effect.clone().to_str())
                .expect("Effect missing from templates");

            if let Some(effect_defense) = effect_template.defense {
                let modifier = 1.0 + (effect_defense * amplifier);
                return modifier;
            }
        }

        // No modifier if 1.0 is returned
        return 1.0;
    }

    fn get_sanctuary_defense(target: &mut CombatQueryItem, templates: &Res<Templates>) -> f32 {
        Self::get_sanctuary_defense_from_effects(&target.effects, templates)
    }

    fn get_sanctuary_defense_from_effects(effects: &Effects, templates: &Templates) -> f32 {
        if let Some((_duration, amplifier, _stacks)) = effects.0.get(&Effect::Sanctuary) {
            let effect_template = templates
                .effect_templates
                .get(&Effect::Sanctuary.to_str())
                .expect("Missing sanctuary template effect");

            return effect_template
                .defense
                .expect("Missing defense on sanctuary template effect")
                * amplifier;
        }

        1.0
    }

    fn get_armor_effects_mod(target: &CombatQueryItem, templates: &Templates) -> f32 {
        Self::get_armor_effects_mod_from_effects(&target.effects, templates)
    }

    fn get_armor_effects_mod_from_effects(effects: &Effects, templates: &Templates) -> f32 {
        let armor_adjustment = effects
            .0
            .iter()
            .filter_map(|(effect, (_expiry_tick, _amplifier, stacks))| {
                templates
                    .effect_templates
                    .get(&effect.clone().to_str())
                    .and_then(|template| template.armor)
                    .map(|armor| armor * *stacks as f32)
            })
            .sum::<f32>();

        (1.0 + armor_adjustment).max(0.0)
    }

    /// The one authoritative physical-damage calculation shared by basic
    /// attacks, primary finishers, and finisher secondary hits.
    fn resolve_damage(
        attacker: &mut CombatQueryItem,
        target: &mut CombatQueryItem,
        templates: &Res<Templates>,
        map: &Res<Map>,
        damage_multiplier: f32,
        flat_damage_bonus: i32,
        defend_stance_mod: f32,
    ) -> (f32, i32) {
        let damage_range = attacker.stats.damage_range.unwrap_or(0).max(0) as f32;
        let damage_roll = if damage_range > 0.0 {
            rand::thread_rng().gen_range(0.0..damage_range)
        } else {
            0.0
        };
        let attacker_weapons = attacker.inventory.get_equipped_weapons();
        let damage_from_items = attacker
            .inventory
            .get_items_value_by_attr(&item::AttrKey::Damage, true);
        let damage_effects_mod = Self::get_damage_effects(attacker, templates);
        let skill_damage_mod = Self::get_skill_damage_mod(attacker, &attacker_weapons);
        let total_damage = Self::outgoing_damage(
            attacker.stats.base_damage.unwrap_or(0) as f32 + damage_roll,
            damage_from_items,
            flat_damage_bonus,
            damage_effects_mod,
            damage_multiplier,
            skill_damage_mod,
        );

        let defense_from_items = target
            .inventory
            .get_items_value_by_attr(&item::AttrKey::Defense, true);
        let total_defense = Self::total_defense(
            target.stats.base_def as f32,
            defense_from_items,
            Self::get_defense_effects(target, templates),
            Self::get_sanctuary_defense(target, templates),
            Self::get_armor_effects_mod(target, templates),
        );
        (
            total_damage,
            Self::post_defense_damage(
                total_damage,
                total_defense,
                defend_stance_mod,
                Self::get_terrain_defense(*target.pos, map),
            ),
        )
    }

    fn outgoing_damage(
        roll_damage: f32,
        damage_from_items: f32,
        flat_damage_bonus: i32,
        damage_effects_mod: f32,
        damage_multiplier: f32,
        skill_damage_mod: f32,
    ) -> f32 {
        (roll_damage + damage_from_items + flat_damage_bonus as f32)
            * damage_effects_mod
            * damage_multiplier
            * skill_damage_mod
    }

    fn post_defense_damage(
        total_damage: f32,
        total_defense: f32,
        defend_stance_mod: f32,
        terrain_defense_mod: f32,
    ) -> i32 {
        let defense_reduction = total_defense / (total_defense + 50.0);
        (total_damage * (1.0 - defense_reduction) * defend_stance_mod * terrain_defense_mod)
            .max(0.0) as i32
    }

    fn total_defense(
        base_defense: f32,
        defense_from_items: f32,
        defense_effects_mod: f32,
        sanctuary_defense: f32,
        armor_effects_mod: f32,
    ) -> f32 {
        (base_defense + defense_from_items)
            * defense_effects_mod
            * sanctuary_defense
            * armor_effects_mod
    }

    fn get_terrain_defense(position: Position, map: &Res<Map>) -> f32 {
        return 1.0 + Map::def_bonus(Map::tile_type(position.x, position.y, &map));
    }

    pub fn add_damage_event(
        game_tick: i32,
        attack_type: String,
        damage: i32,
        combo: Option<String>,
        missed: bool,
        attacker: &CombatQueryItem,
        target: &CombatQueryItem,
        map_events: &mut ResMut<MapEvents>,
    ) {
        let target_state_str = Obj::state_to_str(target.state.clone());

        let damage_event = VisibleEvent::DamageEvent {
            target_id: target.id.0,
            target_pos: target.pos.clone(),
            attack_type: attack_type.clone(),
            damage: damage,
            combo: combo,
            state: target_state_str,
            missed,
        };

        map_events.new(attacker.id.0, game_tick, damage_event);
    }

    fn interrupt_peaceful_work(commands: &mut Commands, entity: Entity, state: &State) {
        if is_peaceful_interruptible_state(state) {
            commands.trigger(CancelEvents { entity });
        }
    }

    fn attack_type_damage_mod(attack_type: AttackType) -> f32 {
        match attack_type {
            AttackType::Quick => 0.5,
            AttackType::Precise => 1.0,
            AttackType::Fierce => 1.5,
        }
    }

    /// Returns a damage multiplier based on the attacker's weapon skill level.
    /// +5% damage per skill level (e.g., Axe level 4 = 1.20x damage).
    fn get_skill_damage_mod(attacker: &CombatQueryItem, weapons: &Vec<Item>) -> f32 {
        if let Some(ref skills) = attacker.skills {
            for weapon in weapons.iter() {
                if let Some(skill) = crate::skill_defs::Skill::from_str(&weapon.subclass) {
                    let level = skills.get_level_by_name(skill);
                    return 1.0 + (level as f32 * 0.05);
                }
            }
        }
        1.0
    }

    pub fn attack_type_to_enum(attack_type: String) -> AttackType {
        match attack_type.as_str() {
            QUICK => AttackType::Quick,
            PRECISE => AttackType::Precise,
            FIERCE => AttackType::Fierce,
            _ => AttackType::Quick,
        }
    }

    /*pub fn combo_to_string(combo: Option<Combo>) -> Option<String> {
        match combo {
            Some(Combo::Hamstring) => Some(HAMSTRING.to_string()),
            Some(Combo::Gouge) => Some(GOUGE.to_string()),
            None => None,
            _ => Some("Unknown Combo".to_string()),
        }
    }*/
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Component)]
    struct TestSpellCaster;

    #[derive(Component)]
    struct TestSpellTarget;

    fn cast_lethal_test_spell(
        mut commands: Commands,
        game_tick: Res<GameTick>,
        mut casters: Query<CombatSpellQuery, (With<TestSpellCaster>, Without<TestSpellTarget>)>,
        mut targets: Query<CombatSpellQuery, (With<TestSpellTarget>, Without<TestSpellCaster>)>,
    ) {
        let caster = casters.single_mut().expect("one spell caster");
        let mut target = targets.single_mut().expect("one spell target");
        Combat::process_spell_damage(
            &mut commands,
            &game_tick,
            Spell::ArcaneBolt,
            &caster,
            &mut target,
        );
    }

    fn spell_test_stats(hp: i32) -> Stats {
        Stats {
            hp,
            base_hp: hp,
            stamina: Some(100),
            mana: Some(100),
            base_stamina: Some(100),
            base_mana: Some(100),
            base_def: 0,
            base_damage: Some(1),
            damage_range: Some(0),
            base_speed: Some(1),
            base_vision: Some(5),
        }
    }

    fn spawn_spell_test_actor(
        world: &mut World,
        id: i32,
        position: Position,
        marker: impl Bundle,
        hp: i32,
    ) -> Entity {
        world
            .spawn((
                marker,
                Id(id),
                PlayerId(id),
                position,
                Class("unit".to_string()),
                Subclass::Hero,
                Template("Human".to_string()),
                State::None,
                Misc {
                    image: String::new(),
                    hsl: Vec::new(),
                    groups: Vec::new(),
                },
                spell_test_stats(hp),
                Effects(HashMap::new()),
                LastCombatTick(0),
            ))
            .id()
    }

    fn effects(values: Vec<Effect>) -> Effects {
        Effects(
            values
                .into_iter()
                .map(|effect| (effect, (0, 1.0, 1)))
                .collect::<HashMap<_, _>>(),
        )
    }

    fn effect_template(name: &str, defense: f32) -> crate::templates::EffectTemplate {
        crate::templates::EffectTemplate {
            name: name.to_string(),
            duration: -1,
            max_hp: None,
            healing: None,
            damage: None,
            damage_over_time: None,
            speed: None,
            attack_speed: None,
            defense: Some(defense),
            stackable: None,
            armor: None,
            lifeleech: None,
            viewshed: None,
            ignore_all_armor: None,
            instant_kill_chance: None,
            next_attack: None,
            vision: None,
            health: None,
            stamina: None,
        }
    }

    fn combat_templates() -> Templates {
        let mut templates = Templates::from_obj_templates(Vec::new());
        templates
            .effect_templates
            .load(vec![effect_template(&Effect::Sanctuary.to_str(), 5.0)]);
        templates
    }

    fn combo_templates() -> Templates {
        let mut templates = Templates::from_obj_templates(Vec::new());
        templates.combo_templates.load(vec![
            ComboTemplate {
                name: "Hamstring".to_string(),
                attacks: vec![QUICK.to_string(), QUICK.to_string()],
                effects: vec!["Hamstrung".to_string()],
                quick_damage: 1.0,
                precise_damage: 1.0,
                fierce_damage: 1.0,
            },
            ComboTemplate {
                name: "Shrouded Slash".to_string(),
                attacks: vec![PRECISE.to_string(), FIERCE.to_string(), QUICK.to_string()],
                effects: vec!["Expose Armor".to_string()],
                quick_damage: 3.0,
                precise_damage: 1.0,
                fierce_damage: 1.0,
            },
            ComboTemplate {
                name: "Shatter Cleave".to_string(),
                attacks: vec![QUICK.to_string(), FIERCE.to_string(), FIERCE.to_string()],
                effects: vec!["Bleed".to_string()],
                quick_damage: 1.0,
                precise_damage: 1.0,
                fierce_damage: 3.5,
            },
            ComboTemplate {
                name: "Nightmare Strike".to_string(),
                attacks: vec![
                    FIERCE.to_string(),
                    PRECISE.to_string(),
                    QUICK.to_string(),
                    FIERCE.to_string(),
                ],
                effects: Vec::new(),
                quick_damage: 1.0,
                precise_damage: 1.0,
                fierce_damage: 8.0,
            },
        ]);
        templates
    }

    #[test]
    fn sanctuary_defense_uses_single_template() {
        let templates = combat_templates();
        let none = effects(Vec::new());
        let sanctuary = effects(vec![Effect::Sanctuary]);

        assert_eq!(
            Combat::get_sanctuary_defense_from_effects(&none, &templates),
            1.0
        );
        assert_eq!(
            Combat::get_sanctuary_defense_from_effects(&sanctuary, &templates),
            5.0
        );
    }

    #[test]
    fn lethal_spell_damage_stops_the_target_thinker() {
        let mut app = App::new();
        app.insert_resource(GameTick(42));
        app.add_systems(Update, cast_lethal_test_spell);
        spawn_spell_test_actor(
            app.world_mut(),
            1,
            Position { x: 0, y: 0 },
            TestSpellCaster,
            20,
        );
        let target = spawn_spell_test_actor(
            app.world_mut(),
            2,
            Position { x: 1, y: 0 },
            TestSpellTarget,
            12,
        );
        app.world_mut()
            .entity_mut(target)
            .insert(ThinkerBuilder::default());

        app.update();

        let target = app.world().entity(target);
        assert_eq!(*target.get::<State>().unwrap(), State::Dead);
        assert_eq!(target.get::<StateDead>().unwrap().dead_at, 42);
        assert!(target.get::<ThinkerBuilder>().is_none());
    }

    #[test]
    fn total_defense_adds_base_and_items_before_sanctuary() {
        assert_eq!(Combat::total_defense(4.0, 0.0, 1.0, 5.0, 1.0), 20.0);
        assert_eq!(Combat::total_defense(4.0, 2.0, 3.0, 2.0, 1.0), 36.0);
        assert_eq!(Combat::total_defense(20.0, 0.0, 1.0, 1.0, 0.75), 15.0);
    }

    #[test]
    fn expose_armor_reduces_total_defense_by_five_percent_per_stack() {
        let mut templates = combat_templates();
        templates
            .effect_templates
            .load(vec![crate::templates::EffectTemplate {
                name: crate::effect::EXPOSEDARMOR.to_string(),
                duration: 20,
                max_hp: None,
                healing: None,
                damage: None,
                damage_over_time: None,
                speed: None,
                attack_speed: None,
                defense: None,
                stackable: Some(true),
                armor: Some(-0.05),
                lifeleech: None,
                viewshed: None,
                ignore_all_armor: None,
                instant_kill_chance: None,
                next_attack: None,
                vision: None,
                health: None,
                stamina: None,
            }]);
        let effects = Effects(HashMap::from([(Effect::ExposedArmor, (200, 1.0, 5))]));

        assert_eq!(
            Combat::get_armor_effects_mod_from_effects(&effects, &templates),
            0.75
        );
        assert_eq!(
            Combat::total_defense(
                20.0,
                0.0,
                1.0,
                1.0,
                Combat::get_armor_effects_mod_from_effects(&effects, &templates),
            ),
            15.0
        );
    }

    #[test]
    fn combo_history_keeps_exact_matches_and_longest_live_suffix() {
        let templates = combo_templates();
        assert_eq!(
            Combat::next_combo_attacks(&[AttackType::Quick], AttackType::Quick, &templates),
            vec![AttackType::Quick, AttackType::Quick]
        );
        assert_eq!(
            Combat::next_combo_attacks(
                &[AttackType::Precise, AttackType::Fierce, AttackType::Quick],
                AttackType::Fierce,
                &templates,
            ),
            vec![AttackType::Quick, AttackType::Fierce]
        );
        assert_eq!(
            Combat::next_combo_attacks(&[], AttackType::Precise, &templates),
            vec![AttackType::Precise]
        );
    }

    #[test]
    fn combo_history_times_out_after_fifteen_seconds() {
        let tracker = ComboTracker {
            target_id: 9,
            attacks: vec![AttackType::Quick],
            last_attack_tick: 100,
        };
        assert_eq!(
            Combat::live_combo_attacks_before_append(Some(&tracker), 9, 250),
            vec![AttackType::Quick]
        );
        assert!(Combat::live_combo_attacks_before_append(Some(&tracker), 9, 251).is_empty());
    }

    #[test]
    fn all_combo_effects_schedule_expiry_and_stackable_effects_cap_at_five() {
        let mut effects = Effects(HashMap::new());
        let mut events = MapEvents::default();
        let combo_effects = [
            Effect::Hamstrung,
            Effect::Stunned,
            Effect::Fear,
            Effect::ExposedArmor,
            Effect::Bleed,
            Effect::Concussed,
        ];
        for (index, effect) in combo_effects.iter().cloned().enumerate() {
            Combat::record_timed_effect(
                index as i32 + 1,
                &mut effects,
                effect.clone(),
                false,
                200 + index as i32,
                &mut events,
            );
            assert!(events.values().any(|event| {
                event.run_tick == 200 + index as i32
                    && matches!(
                        &event.event_type,
                        VisibleEvent::EffectExpiredEvent { effect: scheduled }
                            if *scheduled == effect
                    )
            }));
        }

        for expires_at in 300..308 {
            Combat::record_timed_effect(
                7,
                &mut effects,
                Effect::ExposedArmor,
                true,
                expires_at,
                &mut events,
            );
        }
        assert_eq!(effects.0[&Effect::ExposedArmor], (307, 1.0, 5));
    }

    #[test]
    fn control_effect_dr_uses_full_half_quarter_immune_then_resets() {
        let mut dr = ControlEffectDiminishingReturns::default();
        assert_eq!(
            Combat::control_effect_duration_multiplier(&mut dr, &Effect::Fear, 10),
            Some(1.0)
        );
        assert_eq!(
            Combat::control_effect_duration_multiplier(&mut dr, &Effect::Fear, 20),
            Some(0.5)
        );
        assert_eq!(
            Combat::control_effect_duration_multiplier(&mut dr, &Effect::Fear, 30),
            Some(0.25)
        );
        assert_eq!(
            Combat::control_effect_duration_multiplier(&mut dr, &Effect::Fear, 40),
            None
        );
        assert_eq!(
            Combat::control_effect_duration_multiplier(&mut dr, &Effect::Fear, 100),
            None
        );
        assert_eq!(
            Combat::control_effect_duration_multiplier(&mut dr, &Effect::Fear, 179),
            None
        );
        assert_eq!(
            Combat::control_effect_duration_multiplier(&mut dr, &Effect::Fear, 180),
            Some(1.0)
        );
    }

    #[test]
    fn finisher_damage_uses_skill_scaling_and_reports_post_defense() {
        let pre_defense = Combat::outgoing_damage(10.0, 0.0, 0, 1.0, 2.0, 1.5);
        assert_eq!(pre_defense, 30.0);
        assert_eq!(Combat::post_defense_damage(pre_defense, 50.0, 1.0, 1.0), 15);
    }

    #[test]
    fn aoe_profiles_and_secondary_filters_match_the_milestone() {
        assert_eq!(
            Combat::combo_area_profile("Shatter Cleave")
                .unwrap()
                .damage_scale,
            Some(0.5)
        );
        assert_eq!(
            Combat::combo_area_profile("Massive Pummel")
                .unwrap()
                .effect_duration_scale,
            0.5
        );
        assert!(Combat::combo_area_profile("Hamstring").is_none());

        let attacker = Position { x: 5, y: 5 };
        let unit = Class(crate::constants::CLASS_UNIT.to_string());
        assert!(Combat::valid_combo_secondary_fields(
            attacker,
            10,
            11,
            1000,
            &unit,
            State::None,
            10,
            Position { x: 6, y: 5 },
            false,
        ));
        for invalid in [
            Combat::valid_combo_secondary_fields(
                attacker,
                10,
                11,
                2,
                &unit,
                State::None,
                10,
                Position { x: 6, y: 5 },
                false,
            ),
            Combat::valid_combo_secondary_fields(
                attacker,
                10,
                11,
                1000,
                &unit,
                State::Dead,
                10,
                Position { x: 6, y: 5 },
                false,
            ),
            Combat::valid_combo_secondary_fields(
                attacker,
                10,
                11,
                1000,
                &unit,
                State::None,
                10,
                Position { x: 7, y: 5 },
                false,
            ),
            Combat::valid_combo_secondary_fields(
                attacker,
                10,
                11,
                1000,
                &unit,
                State::None,
                10,
                Position { x: 6, y: 5 },
                true,
            ),
        ] {
            assert!(!invalid);
        }
    }

    #[test]
    fn fortified_outbound_attacks_require_range_or_reach_not_watchtower() {
        let none = effects(Vec::new());
        let fortified = effects(vec![Effect::Fortified]);
        let tower = effects(vec![Effect::Fortified, Effect::WatchtowerLight]);
        let outside = effects(Vec::new());

        assert_eq!(
            Combat::fortified_outbound_attack_error(&none, None, &outside, None, false),
            None
        );
        assert_eq!(
            Combat::fortified_outbound_attack_error(
                &fortified,
                Some(&Fortified { id: 7 }),
                &outside,
                None,
                false,
            ),
            Some(
                "Only ranged attacks or attacks with an equipped Spear can be used from behind a wall."
                    .to_string()
            )
        );
        assert_eq!(
            Combat::fortified_outbound_attack_error(
                &fortified,
                Some(&Fortified { id: 7 }),
                &outside,
                None,
                true,
            ),
            None
        );
        assert_eq!(
            Combat::fortified_outbound_attack_error(
                &tower,
                Some(&Fortified { id: 7 }),
                &outside,
                None,
                false,
            ),
            Some(
                "Only ranged attacks or attacks with an equipped Spear can be used from behind a wall."
                    .to_string()
            )
        );
        assert_eq!(
            Combat::fortified_outbound_attack_error(
                &tower,
                Some(&Fortified { id: 7 }),
                &outside,
                None,
                true,
            ),
            None
        );
    }

    #[test]
    fn combat_reveals_hidden_attacker_and_surviving_hidden_target() {
        // A hidden attacker striking a hidden, surviving target: both reveal.
        assert_eq!(
            Combat::combat_reveals(State::Hiding, State::Hiding, 10),
            (true, true)
        );
        // A slain hidden target is shown as a corpse, not revealed as a unit.
        assert_eq!(
            Combat::combat_reveals(State::Hiding, State::Hiding, 0),
            (true, false)
        );
        // Non-hidden combatants are unaffected.
        assert_eq!(
            Combat::combat_reveals(State::None, State::None, 10),
            (false, false)
        );
        // A hidden defender struck by a visible attacker reveals.
        assert_eq!(
            Combat::combat_reveals(State::None, State::Hiding, 5),
            (false, true)
        );
    }

    #[test]
    fn same_fortification_is_not_an_outbound_attack() {
        let fortified = effects(vec![Effect::Fortified]);

        assert_eq!(
            Combat::fortified_outbound_attack_error(
                &fortified,
                Some(&Fortified { id: 7 }),
                &fortified,
                Some(&Fortified { id: 7 }),
                false,
            ),
            None
        );
    }
}
