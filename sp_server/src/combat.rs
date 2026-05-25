use bevy::ecs::query::{QueryData, WorldQuery};
use bevy::prelude::*;
use big_brain::thinker::ThinkerBuilder;

use rand::Rng;

use crate::constants::TICKS_PER_SEC;
use crate::effect::{Effect, Effects};
use crate::event::{MapEvents, Spell, VisibleEvent};
use crate::game::{Fortified, GameTick};
use crate::ids::Ids;
use crate::item::{self, AttrKey, Inventory, Item};
use crate::map::Map;
use crate::obj::Obj;
use crate::obj::{
    is_peaceful_interruptible_state, CancelEvents, Class, HeroClass, Id, LastAttacker,
    LastCombatTick, Misc, PlayerId, Position, State, StateChange, StateDead, Stats, Subclass,
    Template,
};
use crate::skill::{SkillUpdated, Skills};
use crate::templates::{ComboTemplate, ObjTemplate, Templates};

pub const QUICK: &str = "quick";
pub const PRECISE: &str = "precise";
pub const FIERCE: &str = "fierce";

pub const HAMSTRING: &str = "Hamstring";
pub const GOUGE: &str = "Gouge";

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
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct CombatQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub template: &'static Template,
    pub state: &'static mut State,
    pub misc: &'static mut Misc,
    pub stats: &'static mut Stats,
    pub effects: &'static mut Effects,
    pub fortified: Option<&'static Fortified>,
    pub inventory: &'static mut Inventory,
    pub skills: Option<&'static mut Skills>,
    pub hero_class: Option<&'static HeroClass>,
    pub combo_tracker: Option<&'static mut ComboTracker>,
    pub last_combat_tick: &'static mut LastCombatTick,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct CombatSpellQuery {
    pub entity: Entity,
    pub id: &'static Id,
    pub player_id: &'static PlayerId,
    pub pos: &'static Position,
    pub class: &'static Class,
    pub subclass: &'static Subclass,
    pub template: &'static Template,
    pub state: &'static mut State,
    pub misc: &'static mut Misc,
    pub stats: &'static mut Stats,
    pub effects: &'static mut Effects,
    pub fortified: Option<&'static Fortified>,
    pub last_combat_tick: &'static mut LastCombatTick,
}

#[derive(Debug, Clone)]
pub struct Combat;

impl Combat {
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

    pub fn fortified_outbound_attack_error(
        attacker_effects: &Effects,
        attacker_fortified: Option<&Fortified>,
        target_effects: &Effects,
        target_fortified: Option<&Fortified>,
        ranged_attack: bool,
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

        if !ranged_attack {
            return Some("Only ranged attacks can be used from behind a wall.".to_string());
        }

        None
    }

    pub fn fortified_outbound_attack_error_from_combat(
        attacker: &CombatQueryItem,
        target: &CombatQueryItem,
        ranged_attack: bool,
    ) -> Option<String> {
        Self::fortified_outbound_attack_error(
            &attacker.effects,
            attacker.fortified,
            &target.effects,
            target.fortified,
            ranged_attack,
        )
    }

    pub fn fortified_outbound_attack_error_from_spell(
        attacker: &CombatSpellQueryItem,
        target: &CombatSpellQueryItem,
        ranged_attack: bool,
    ) -> Option<String> {
        Self::fortified_outbound_attack_error(
            &attacker.effects,
            attacker.fortified,
            &target.effects,
            target.fortified,
            ranged_attack,
        )
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
    ) -> (i32, Option<String>, Option<SkillUpdated>) {
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
    ) -> (i32, Option<String>, Option<SkillUpdated>) {
        let mut rng = rand::thread_rng();

        // 1 Get Base Damage, DamageRange, BaseDef and DefHp
        let target_template = templates.obj_templates.get(target.template.0.clone());
        let damage_range = attacker.stats.damage_range.unwrap() as f32;
        let base_damage = attacker.stats.base_damage.unwrap() as f32;
        let base_defense = target.stats.base_def as f32;

        // #3 Get attacker weapons
        let attacker_weapons = attacker.inventory.get_equipped_weapons();
        debug!("Attacker_weapons: {:?}", attacker_weapons);

        // 4 Get damage effects on attacker
        let damage_effects_mod = Self::get_damage_effects(attacker, templates);

        // 5 Get defense effects on defender
        let defense_effects_mod = Self::get_defense_effects(target, templates);

        // 6 Get damage mod from items
        let damage_from_items = attacker
            .inventory
            .get_items_value_by_attr(&item::AttrKey::Damage, true);

        // 6b Get weapon skill damage bonus (+5% per skill level)
        let skill_damage_mod = Self::get_skill_damage_mod(attacker, &attacker_weapons);

        // 7 Get attack type damage from
        let attack_type_damage_mod = Self::attack_type_damage_mod(attack_type.clone());

        // TODO 8 Get damage reduction from Defensive action

        // 8a Get Sanctuary damage reduction
        let sanctuary_defense = Self::get_sanctuary_defense(target, templates);

        // 9 Get armor from defender items
        let defense_from_items = target
            .inventory
            .get_items_value_by_attr(&item::AttrKey::Defense, true);

        // TODO 10 Check if Defender has Defensive Stance

        // 11 & 12 Add attack type to attack list
        Self::add_attack_to_combo_tracker(commands, templates, attack_type, attacker, target);

        // TODO 13 Check if combo is countered

        // TODO 14 Remove Defense Stanc Effect if combo countered

        // 15 Calculate combo damage and apply combo effects
        /*let (combo_quick_damage_mod, combo_precise_damage_mod, combo_fierce_damage_mod) =
            Self::get_combo_damage(combo_template.clone());

        let combo_damage_mod =
            combo_quick_damage_mod * combo_precise_damage_mod * combo_fierce_damage_mod;
        debug!("combo_damage_mod: {:?}", combo_damage_mod);*/

        // TODO 16 Check if target is fortified

        // 17 Roll from base damage
        let roll_damage = rng.gen_range(0.0..damage_range) + base_damage;

        // 18 Calculate total damage
        let total_damage = (roll_damage + damage_from_items + options.damage_bonus as f32)
            * damage_effects_mod
            * attack_type_damage_mod
            * skill_damage_mod;

        // 19 Calculate total defense
        let total_defense = Self::total_defense(
            base_defense,
            defense_from_items,
            defense_effects_mod,
            sanctuary_defense,
        );

        // 20 & 21 Calculate damage defense reduction
        let defense_reduction = total_defense / (total_defense + 50.0);
        let damage_reduction = total_damage * (1.0 - defense_reduction);

        // TODO 22 Get defense stance mod
        let defend_stance_mod = 1.0;

        // 23 Get terrain defense mod
        let terrain_defense_mod = Self::get_terrain_defense(*target.pos, map);

        // TODO 24 Get monolith distance defense mod
        let monolith_distance_defense_mod = 1.0;

        // 25 Calculate final damage
        let final_damage = damage_reduction
            * defend_stance_mod
            * terrain_defense_mod
            * monolith_distance_defense_mod;

        // 26 Update Hp and check if target is dead
        target.stats.hp -= final_damage as i32;

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
        Self::process_weapon_procs(templates, &attacker_weapons, target);

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

        debug!("Total Damage: {:?}", total_damage);

        // Return combo name
        /*let mut combo_name = None;

        if let Some(combo) = combo_template {
            combo_name = Some(combo.name);
        }*/

        return (total_damage as i32, None, skill_updated);
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
        let mut rng = rand::thread_rng();

        // 1 Get Base Damage, DamageRange, BaseDef and DefHp
        let target_template = templates.obj_templates.get(target.template.0.clone());
        let damage_range = attacker.stats.damage_range.unwrap() as f32;
        let base_damage = attacker.stats.base_damage.unwrap() as f32;
        let base_defense = target.stats.base_def as f32;

        // 2 Get attacker & defender items
        let attacker_items = attacker.inventory.get_equipped();
        let defender_items = target.inventory.get_equipped();

        // #3 Get attacker weapons
        let attacker_weapons = attacker.inventory.get_equipped_weapons();
        debug!("Attacker_weapons: {:?}", attacker_weapons);

        // 4 Get damage effects on attacker
        let damage_effects_mod = Self::get_damage_effects(attacker, templates);

        // 5 Get defense effects on defender
        let defense_effects_mod = Self::get_defense_effects(target, templates);

        // 5b Get Sanctuary damage reduction
        let sanctuary_defense = Self::get_sanctuary_defense(target, templates);

        // 6 Get damage mod from items
        let damage_from_items = attacker
            .inventory
            .get_items_value_by_attr(&item::AttrKey::Damage, true);

        // TODO 8 Get damage reduction from Defensive action

        // 9 Get armor from defender items
        let defense_from_items = target
            .inventory
            .get_items_value_by_attr(&item::AttrKey::Defense, true);

        // TODO 10 Check if Defender has Defensive Stance

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

        // 17 Roll from base damage
        let roll_damage = rng.gen_range(0.0..damage_range) + base_damage;

        // 18 Calculate total damage
        let total_damage =
            (roll_damage + damage_from_items) * damage_effects_mod * combo_damage_mod;

        // 19 Calculate total defense
        let total_defense = Self::total_defense(
            base_defense,
            defense_from_items,
            defense_effects_mod,
            sanctuary_defense,
        );

        // 20 & 21 Calculate damage defense reduction
        let defense_reduction = total_defense / (total_defense + 50.0);
        let damage_reduction = total_damage * (1.0 - defense_reduction);

        // TODO 22 Get defense stance mod
        let defend_stance_mod = 1.0;

        // 23 Get terrain defense mod
        let terrain_defense_mod = Self::get_terrain_defense(*target.pos, map);

        // TODO 24 Get monolith distance defense mod
        let monolith_distance_defense_mod = 1.0;

        // 25 Calculate final damage
        let final_damage = damage_reduction
            * defend_stance_mod
            * terrain_defense_mod
            * monolith_distance_defense_mod;

        // 26 Update Hp and check if target is dead
        target.stats.hp -= final_damage as i32;

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
            templates,
            attacker,
            target,
            ids,
            game_tick,
            map_events,
        );

        // 29 Check if any weapons procced
        Self::process_weapon_procs(templates, &attacker_weapons, target);

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

        debug!("Total Damage: {:?}", total_damage);

        // Return combo name
        let mut combo_name = None;

        if let Some(combo) = combo_template {
            combo_name = Some(combo.name);
        }

        return (total_damage as i32, combo_name, skill_updated);
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
        target.stats.hp -= damage;

        if target.stats.hp <= 0 {
            *target.state = State::Dead;
            debug!("Target {:?} is dead", target.entity);
            commands.entity(target.entity).insert(StateDead {
                dead_at: game_tick.0,
                killer: caster.template.0.clone(),
            });
            commands.trigger(StateChange {
                entity: target.entity,
                new_state: State::Dead,
            });
        }

        return damage;
    }

    fn process_weapon_procs(
        templates: &Res<Templates>,
        attacker_weapons: &Vec<Item>,
        target: &mut CombatQueryItem,
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

                        let effects = &mut target.effects.0;
                        effects.insert(effect, (effect_template.duration, 1.0, 1));

                        debug!("effects: {:?}", effects);
                    }
                }
            }
        }
    }

    fn add_attack_to_combo_tracker(
        commands: &mut Commands,
        _templates: &Res<Templates>,
        attack_type: AttackType,
        attacker: &mut CombatQueryItem,
        target: &mut CombatQueryItem,
    ) {
        // Only allow combos for players
        if attacker.player_id.0 < 1000 {
            debug!("check combo_tracker: {:?}", attacker.combo_tracker);

            if let Some(combo_tracker) = &mut attacker.combo_tracker {
                // Add to existing combo tracker only if same target id
                // TODO reconsider if this is a good idea
                if combo_tracker.target_id == target.id.0 {
                    combo_tracker.attacks.push(attack_type);
                } else {
                    combo_tracker.target_id = target.id.0;
                    combo_tracker.attacks = vec![attack_type];
                }
            } else {
                let combo_tracker = ComboTracker {
                    target_id: target.id.0,
                    attacks: vec![attack_type],
                };

                commands.entity(attacker.entity).insert(combo_tracker);
            }

            debug!("post check combo_tracker {:?}", attacker.combo_tracker);
        }
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
        templates: &Res<Templates>,
        _attacker: &mut CombatQueryItem,
        target: &mut CombatQueryItem,
        _ids: &mut ResMut<Ids>,
        game_tick: &Res<GameTick>,
        map_events: &mut ResMut<MapEvents>,
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
                //
                match effect {
                    Effect::Hamstrung => {
                        let hamstrung_event = VisibleEvent::EffectExpiredEvent {
                            effect: effect.clone(),
                        };

                        map_events.new(
                            target.id.0,
                            game_tick.0 + effect_template.duration * TICKS_PER_SEC,
                            hamstrung_event,
                        );
                    }
                    Effect::Stunned => {
                        let stun_event = VisibleEvent::EffectExpiredEvent {
                            effect: effect.clone(),
                        };

                        map_events.new(
                            target.id.0,
                            game_tick.0 + effect_template.duration * TICKS_PER_SEC,
                            stun_event,
                        );
                    }
                    _ => {}
                }

                target
                    .effects
                    .0
                    .insert(effect, (effect_template.duration, 1.0, 1));
            }
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
            if matches!(effect, Effect::Sanctuary | Effect::WeakSanctuary) {
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
        for effect in [Effect::Sanctuary, Effect::WeakSanctuary] {
            if let Some((_duration, amplifier, _stacks)) = effects.0.get(&effect) {
                let effect_template = templates
                    .effect_templates
                    .get(&effect.clone().to_str())
                    .expect("Missing sanctuary template effect");

                return effect_template
                    .defense
                    .expect("Missing defense on sanctuary template effect")
                    * amplifier;
            }
        }

        1.0
    }

    fn total_defense(
        base_defense: f32,
        defense_from_items: f32,
        defense_effects_mod: f32,
        sanctuary_defense: f32,
    ) -> f32 {
        (base_defense + defense_from_items) * defense_effects_mod * sanctuary_defense
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
        templates.effect_templates.load(vec![
            effect_template(&Effect::Sanctuary.to_str(), 5.0),
            effect_template(&Effect::WeakSanctuary.to_str(), 2.0),
        ]);
        templates
    }

    #[test]
    fn sanctuary_defense_uses_full_and_weak_templates() {
        let templates = combat_templates();
        let none = effects(Vec::new());
        let full = effects(vec![Effect::Sanctuary]);
        let weak = effects(vec![Effect::WeakSanctuary]);

        assert_eq!(
            Combat::get_sanctuary_defense_from_effects(&none, &templates),
            1.0
        );
        assert_eq!(
            Combat::get_sanctuary_defense_from_effects(&full, &templates),
            5.0
        );
        assert_eq!(
            Combat::get_sanctuary_defense_from_effects(&weak, &templates),
            2.0
        );
    }

    #[test]
    fn total_defense_adds_base_and_items_before_sanctuary() {
        assert_eq!(Combat::total_defense(4.0, 0.0, 1.0, 5.0), 20.0);
        assert_eq!(Combat::total_defense(4.0, 2.0, 3.0, 2.0), 36.0);
    }

    #[test]
    fn fortified_outbound_attacks_require_range_not_watchtower() {
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
            Some("Only ranged attacks can be used from behind a wall.".to_string())
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
            Some("Only ranged attacks can be used from behind a wall.".to_string())
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
