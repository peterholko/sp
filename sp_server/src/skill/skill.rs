use bevy::prelude::*;

use std::collections::HashMap;

use crate::skill_defs::Skill;
use crate::templates::{SkillTemplate, SkillTemplates};
use crate::{item, network};

pub const CLASS_GATHERING: &str = "Gathering";
pub const CLASS_CRAFTING: &str = "Crafting";

pub const MINING: &str = "Mining";
pub const LOGGING: &str = "Logging";
pub const STONECUTTING: &str = "Stonecutting";
pub const FORAGING: &str = "Foraging";
pub const FARMING: &str = "Farming";
pub const FISHING: &str = "Fishing";
pub const BUTCHERY: &str = "Butchery";
pub const WOODCUTTING: &str = "Woodcutting";
pub const WEAPONSMITHING: &str = "Weaponsmithing";
pub const ARMORSMITHING: &str = "Armorsmithing";
pub const TOOLMAKING: &str = "Toolmaking";
pub const COOKING: &str = "Cooking";
pub const CONSTRUCTION: &str = "Construction";
pub const CARPENTRY: &str = "Carpentry";
pub const MASONRY: &str = "Masonry";

pub const NOVICE_WARRIOR: &str = "Novice Warrior";
pub const NOVICE_RANGER: &str = "Novice Ranger";
pub const NOVICE_MAGE: &str = "Novice Mage";
pub const SKILLED_WARRIOR: &str = "Skilled Warrior";
pub const SKILLED_RANGER: &str = "Skilled Ranger";
pub const SKILLED_MAGE: &str = "Skilled Mage";
pub const GREAT_WARRIOR: &str = "Great Warrior";
pub const GREAT_RANGER: &str = "Great Ranger";
pub const GREAT_MAGE: &str = "Great Mage";
pub const LEGENDARY_WARRIOR: &str = "Legendary Warrior";
pub const LEGENDARY_RANGER: &str = "Legendary Ranger";
pub const LEGENDARY_MAGE: &str = "Legendary Mage";
pub const MAX_RANK: &str = "Max Rank";

#[derive(Debug, Clone)]
pub struct SkillUpdated {
    pub id: i32,
    pub xp_type: String,
    pub xp: i32,
}

#[derive(Debug, Reflect, Clone)]
pub struct SkillData {
    pub level: i32,
    pub xp: i32,
}

#[derive(Debug, Reflect, Component, Clone)]
#[reflect(Component)]
pub struct Skills(HashMap<Skill, SkillData>);

impl Skills {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn update(
        &mut self,
        skill_name: Skill,
        value: i32,
        skill_templates: &SkillTemplates,
    ) -> Option<i32> {
        let Some(skill_template) = skill_templates.get(skill_name.to_str()) else {
            panic!(
                "Invalid skill name {:?}, does not exist in templates.",
                skill_name.to_str()
            );
        };

        let levelup: Option<i32>;

        if let Some(obj_skill) = self.0.get_mut(&skill_name) {
            levelup = obj_skill.update_xp_level(value, skill_template);
        } else {
            let mut new_skill = SkillData {
                level: 0,
                xp: 0,
            };

            levelup = new_skill.update_xp_level(value, skill_template);

            self.0.insert(skill_name, new_skill);
        }

        return levelup;
    }

    pub fn get_total_xp(&self) -> i32 {
        let mut total_xp = 0;

        for (skill_name, skill) in self.0.iter() {
            total_xp += skill.xp;
        }

        return total_xp;
    }

    pub fn has_skill_level(&self, skill_name: String, skill_level: i32) -> bool {
        // If skill level is 0
        if skill_level == 0 {
            return true;
        }

        if let Some(skill_name_enum) = Skill::from_str(&skill_name) {
            if let Some(obj_skill) = self.0.get(&skill_name_enum) {
                return obj_skill.level >= skill_level;
            }
        }

        return false;
    }

    pub fn get_all(&self) -> HashMap<String, &SkillData> {
        let mut skills_map = HashMap::new();

        for (skill_name, skill) in self.0.iter() {
            skills_map.insert(skill_name.to_str().to_string(), skill);
        }

        return skills_map;
    }

    pub fn get_levels(&self) -> HashMap<String, i32> {
        let mut skills_map = HashMap::new();

        for (skill_name, skill) in self.0.iter() {
            skills_map.insert(skill_name.to_str().to_string(), skill.level);
        }

        return skills_map;
    }

    pub fn get_packet(&self, skill_templates: &SkillTemplates) -> HashMap<String, network::Skill> {
        let mut skills_map = HashMap::new();

        for (skill_name, skill) in self.0.iter() {
            let skill_name_str = skill_name.to_str().to_string();
            let next_xp = Self::get_next(skill_name_str.clone(), skill.level + 1, skill_templates);

            let skill_data = network::Skill {
                level: skill.level,
                xp: skill.xp,
                next: next_xp,
            };

            skills_map.insert(skill_name_str, skill_data);
        }

        return skills_map;
    }

    pub fn get_next(skill_name: String, level: i32, skill_templates: &SkillTemplates) -> i32 {
        let level_usize = level as usize;

        for (_skill_name, skill_template) in skill_templates.iter() {
            if skill_template.name == skill_name {
                if level_usize < skill_template.xp.len() {
                    return skill_template.xp[level_usize];
                } else {
                    return i32::MAX;
                }
            }
        }

        return i32::MAX;
    }

    pub fn get_by_name(&self, skill_name: Skill) -> Option<SkillData> {
        return self.0.get(&skill_name).cloned();
    }

    pub fn get_level_by_name(&self, skill_name: Skill) -> i32 {
        return self.0.get(&skill_name).map(|skill| skill.level).unwrap_or(0);
    }

    pub fn get_templates_by_class(
        class: String,
        skill_templates: &SkillTemplates,
    ) -> Vec<SkillTemplate> {
        let mut skill_template_by_class = Vec::new();

        for (_skill_name, skill_template) in skill_templates.iter() {
            if skill_template.class == class {
                skill_template_by_class.push(skill_template.clone());
            }
        }

        return skill_template_by_class;
    }
}

impl SkillData {

    pub fn update_xp_level(&mut self, value: i32, skill_template: &SkillTemplate) -> Option<i32> {
        let xp_level_list = &skill_template.xp;
        let mut remaining = value;
        let mut levelup = None;

        // Calculate skill level from xp value
        while remaining > 0 {
            if let Ok(xp_index) = usize::try_from(self.level) {
                if xp_index < xp_level_list.len() {
                    if self.xp + remaining < xp_level_list[xp_index] {
                        self.xp += remaining;
                        remaining = 0;
                    } else if self.xp + value == xp_level_list[self.level as usize] {
                        self.xp = 0;
                        self.level += 1;
                        remaining = 0;
                        levelup = Some(self.level);
                    } else {
                        let total_xp = self.xp + remaining;
                        remaining = total_xp - xp_level_list[self.level as usize];

                        self.xp = 0;
                        self.level += 1;
                        levelup = Some(self.level);
                    }
                } else {
                    break;
                }
            }
        }

        return levelup;
    }

    pub fn hero_advance(hero_template: String) -> (String, i32) {
        let (next_template, required_xp) = match hero_template.as_str() {
            NOVICE_WARRIOR => (SKILLED_WARRIOR, 10000),
            NOVICE_RANGER => (SKILLED_RANGER, 10000),
            NOVICE_MAGE => (SKILLED_MAGE, 10000),
            SKILLED_WARRIOR => (GREAT_WARRIOR, 50000),
            SKILLED_RANGER => (GREAT_RANGER, 50000),
            SKILLED_MAGE => (GREAT_MAGE, 50000),
            GREAT_WARRIOR => (LEGENDARY_WARRIOR, 1000000),
            GREAT_RANGER => (LEGENDARY_RANGER, 1000000),
            GREAT_MAGE => (LEGENDARY_MAGE, 1000000),
            LEGENDARY_WARRIOR => (MAX_RANK, -1),
            LEGENDARY_RANGER => (MAX_RANK, -1),
            LEGENDARY_MAGE => (MAX_RANK, -1),
            _ => (MAX_RANK, -1),
        };

        return (next_template.to_string(), required_xp);
    }

    pub fn item_class_to_skill(item_class: &str) -> Option<Skill> {
        match item_class {
            item::WEAPON => Some(Skill::Weaponsmithing),
            item::ARMOR => Some(Skill::Armorsmithing),
            item::GATHERING => Some(Skill::Toolmaking),
            item::TORCH => Some(Skill::Toolmaking),
            item::ITEM_FOOD => Some(Skill::Cooking),
            _ => None,
        }
    }

    pub fn item_subclass_to_skill(item_subclass: &str) -> Option<Skill> {
        match item_subclass {
            item::FIREWOOD => Some(Skill::Woodcutting),
            _ => None,
        }
    }
}
pub struct SkillPlugin;

impl Plugin for SkillPlugin {
    fn build(&self, app: &mut App) {
    }
}
