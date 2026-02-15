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

#[derive(Debug, Reflect, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectInfo {
    pub effect: Effect,
    pub attrs: HashMap<EffectAttr, EffectVal>
}

#[derive(Debug, Reflect, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectAttr {
    Health,
    Damage,
    Defense,
    Duration,
    Stamina,
    Vision,
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

            match effect {
                Effect::Sanctuary => { 
                    let defense = *amplifier * effect_template.defense.unwrap();

                    effect_attrs.insert(EffectAttr::Defense, EffectVal::Num(defense));
                    effect_attrs.insert(EffectAttr::Duration, EffectVal::Num(effect_template.duration as f32));
                }
                Effect::WeakSanctuary => { 
                    let defense = *amplifier * effect_template.defense.unwrap();

                    effect_attrs.insert(EffectAttr::Defense, EffectVal::Num(defense));
                    effect_attrs.insert(EffectAttr::Duration, EffectVal::Num(effect_template.duration as f32));
                }
                Effect::Fortified => {
                    effect_attrs.insert(EffectAttr::Defense, EffectVal::Num(effect_template.defense.unwrap()));
                    effect_attrs.insert(EffectAttr::Duration, EffectVal::Num(effect_template.duration as f32));
                }
                Effect::CampfireLight | Effect::WatchtowerLight => {
                    effect_attrs.insert(EffectAttr::Vision, EffectVal::Num(effect_template.vision.unwrap()));
                    effect_attrs.insert(EffectAttr::Duration, EffectVal::Num(effect_template.duration as f32));
                }
                _ => {}
            }

            let effect_info = EffectInfo {
                effect: effect.clone(),
                attrs: effect_attrs
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
                    let effect_template = templates.effect_templates.get(&effect.clone().to_str()).expect("Effect missing from templates");
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
