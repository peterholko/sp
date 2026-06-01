use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::templates::{EffectTemplates, Templates};

pub const BLEED: &str = "Bleed";
pub const DEEPWOUND: &str = "Deep Wound";
pub const CONCUSSED: &str = "Concussed";
pub const IMPALED: &str = "Impaled";
pub const BACKSTABBED: &str = "Backstabbed";
pub const DAZED: &str = "Dazed";
pub const DISARMED: &str = "Disarmed";
pub const DEMORALIZINGSHOUT: &str = "Demoralizing Shout";
pub const EXPOSEDARMOR: &str = "Exposed Armor";
pub const HAMSTRUNG: &str = "Hamstrung";
pub const FEAR: &str = "Fear";
pub const STUNNED: &str = "Stunned";
pub const SANCTUARY: &str = "Sanctuary";
pub const WEAK_SANCTUARY: &str = "Weak Sanctuary";
pub const FORTIFIED: &str = "Fortified";
pub const BURNING: &str = "Burning";
pub const CAMPFIRE_LIGHT: &str = "Campfire Light";
pub const WATCHTOWER_LIGHT: &str = "Watchtower Light";
pub const FOOD_POISONING: &str = "Food Poisoning";
pub const BRACING: &str = "Bracing";
pub const DODGING: &str = "Dodging";
pub const PARRYING: &str = "Parrying";
pub const SICKNESS: &str = "Sickness";
pub const CURSED: &str = "Cursed";

#[derive(Debug, Reflect, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectInfo {
    pub effect: Effect,
    pub attrs: HashMap<EffectAttr, EffectVal>,
}

#[derive(Debug, Reflect, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectAttr {
    Armor,
    AttackSpeed,
    Health,
    Damage,
    Defense,
    Duration,
    Healing,
    Lifeleech,
    MaxHealth,
    NextAttack,
    Stamina,
    Speed,
    Vision,
    Viewshed,
}

#[derive(Debug, Reflect, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EffectVal {
    Num(f32),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, Reflect, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Effect {
    Bleed,
    DeepWound,
    Concussed,
    Impaled,
    Backstabbed,
    Dazed,
    Disarmed,
    DemoralizingShout,
    ExposedArmor,
    Hamstrung,
    Fear,
    Stunned,
    Sanctuary,
    WeakSanctuary,
    Fortified,
    Burning,
    CampfireLight,
    WatchtowerLight,
    FoodPoisoning,
    Bracing,
    Dodging,
    Parrying,
    Sickness,
    Cursed,
}

impl Effect {
    pub fn to_str(self) -> String {
        match self {
            Effect::Bleed => BLEED.to_string(),
            Effect::DeepWound => DEEPWOUND.to_string(),
            Effect::Concussed => CONCUSSED.to_string(),
            Effect::Impaled => IMPALED.to_string(),
            Effect::Backstabbed => BACKSTABBED.to_string(),
            Effect::Dazed => DAZED.to_string(),
            Effect::Disarmed => DISARMED.to_string(),
            Effect::DemoralizingShout => DEMORALIZINGSHOUT.to_string(),
            Effect::ExposedArmor => EXPOSEDARMOR.to_string(),
            Effect::Hamstrung => HAMSTRUNG.to_string(),
            Effect::Fear => FEAR.to_string(),
            Effect::Stunned => STUNNED.to_string(),
            Effect::Sanctuary => SANCTUARY.to_string(),
            Effect::WeakSanctuary => WEAK_SANCTUARY.to_string(),
            Effect::Fortified => FORTIFIED.to_string(),
            Effect::Burning => BURNING.to_string(),
            Effect::CampfireLight => CAMPFIRE_LIGHT.to_string(),
            Effect::WatchtowerLight => WATCHTOWER_LIGHT.to_string(),
            Effect::FoodPoisoning => FOOD_POISONING.to_string(),
            Effect::Bracing => BRACING.to_string(),
            Effect::Dodging => DODGING.to_string(),
            Effect::Parrying => PARRYING.to_string(),
            Effect::Sickness => SICKNESS.to_string(),
            Effect::Cursed => CURSED.to_string(),
        }
    }

    pub fn from_string(effect_string: &String) -> Self {
        match effect_string.as_str() {
            BLEED => Effect::Bleed,
            DEEPWOUND => Effect::DeepWound,
            CONCUSSED => Effect::Concussed,
            IMPALED => Effect::Impaled,
            BACKSTABBED => Effect::Backstabbed,
            DAZED => Effect::Dazed,
            DISARMED => Effect::Disarmed,
            DEMORALIZINGSHOUT => Effect::DemoralizingShout,
            EXPOSEDARMOR => Effect::ExposedArmor,
            HAMSTRUNG => Effect::Hamstrung,
            FEAR => Effect::Fear,
            STUNNED => Effect::Stunned,
            SANCTUARY => Effect::Sanctuary,
            WEAK_SANCTUARY => Effect::WeakSanctuary,
            FORTIFIED => Effect::Fortified,
            BURNING => Effect::Burning,
            CAMPFIRE_LIGHT => Effect::CampfireLight,
            WATCHTOWER_LIGHT => Effect::WatchtowerLight,
            FOOD_POISONING => Effect::FoodPoisoning,
            BRACING => Effect::Bracing,
            DODGING => Effect::Dodging,
            PARRYING => Effect::Parrying,
            SICKNESS => Effect::Sickness,
            CURSED => Effect::Cursed,
            _ => panic!("Invalid Effect"),
        }
    }
}

type Duration = i32;
type Amplifier = f32;
type Stacks = i32;

#[derive(Debug, Component, Clone)]
pub struct Effects(pub HashMap<Effect, (Duration, Amplifier, Stacks)>);

impl Effects {
    pub fn get_info_list(&self, effect_templates: &EffectTemplates) -> Vec<EffectInfo> {
        let mut effect_info_list = Vec::new();

        for (effect, (duration, amplifier, stacks)) in self.0.iter() {
            let mut effect_attrs = HashMap::new();

            let effect_template = effect_templates
                .get(&effect.clone().to_str())
                .expect("Effect missing from templates");

            if let Some(health) = effect_template.health {
                effect_attrs.insert(EffectAttr::Health, EffectVal::Num(health));
            }
            if let Some(max_hp) = effect_template.max_hp {
                effect_attrs.insert(EffectAttr::MaxHealth, EffectVal::Num(max_hp));
            }
            if let Some(healing) = effect_template.healing {
                effect_attrs.insert(EffectAttr::Healing, EffectVal::Num(healing));
            }
            if let Some(damage) = effect_template.damage {
                effect_attrs.insert(EffectAttr::Damage, EffectVal::Num(damage));
            }
            if let Some(speed) = effect_template.speed {
                effect_attrs.insert(EffectAttr::Speed, EffectVal::Num(speed));
            }
            if let Some(attack_speed) = effect_template.attack_speed {
                effect_attrs.insert(EffectAttr::AttackSpeed, EffectVal::Num(attack_speed));
            }
            if let Some(defense) = effect_template.defense {
                let amplifier = if *amplifier == 0.0 { 1.0 } else { *amplifier };
                effect_attrs.insert(EffectAttr::Defense, EffectVal::Num(defense * amplifier));
            }
            if let Some(armor) = effect_template.armor {
                effect_attrs.insert(EffectAttr::Armor, EffectVal::Num(armor));
            }
            if let Some(lifeleech) = effect_template.lifeleech {
                effect_attrs.insert(EffectAttr::Lifeleech, EffectVal::Num(lifeleech));
            }
            if let Some(viewshed) = effect_template.viewshed {
                effect_attrs.insert(EffectAttr::Viewshed, EffectVal::Num(viewshed as f32));
            }
            if let Some(next_attack) = effect_template.next_attack {
                effect_attrs.insert(EffectAttr::NextAttack, EffectVal::Bool(next_attack));
            }
            if let Some(vision) = effect_template.vision {
                effect_attrs.insert(EffectAttr::Vision, EffectVal::Num(vision));
            }
            effect_attrs.insert(
                EffectAttr::Duration,
                EffectVal::Num(effect_template.duration as f32),
            );

            let effect_info = EffectInfo {
                effect: effect.clone(),
                attrs: effect_attrs,
            };

            effect_info_list.push(effect_info);
        }

        effect_info_list
    }

    pub fn has(&self, effect: Effect) -> bool {
        self.0.contains_key(&effect)
    }

    pub fn get_vision_modifier(&self, templates: &Res<Templates>) -> f32 {
        let mut modifier = 0.0;

        for (effect, (_duration, _amplifier, _stacks)) in self.0.iter() {
            match effect {
                Effect::CampfireLight | Effect::WatchtowerLight => {
                    let effect_template = templates
                        .effect_templates
                        .get(&effect.clone().to_str())
                        .expect("Effect missing from templates");
                    modifier += effect_template.vision.unwrap();
                }
                _ => {}
            }
        }

        return modifier;
    }

    // Value returned is between 0.0 and 1.0
    fn get_damage_effects(self, templates: &Res<Templates>) -> f32 {
        for (effect, (_duration, _amplifier, _stacks)) in self.0.iter() {
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

    pub fn get_speed_effects(&self, templates: &Res<Templates>) -> f32 {
        // Get effects
        for (effect, (_duration, _amplifier, _stackss)) in self.0.iter() {
            let effect_template = templates
                .effect_templates
                .get(&effect.clone().to_str())
                .expect("Effect missing from templates");

            if let Some(effect_speed) = effect_template.speed {
                let modifier = 1.0 + effect_speed;
                return modifier;
            }
        }

        return 1.0;
    }
}
