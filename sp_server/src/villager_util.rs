use bevy::prelude::*;



use rand::Rng;
use crate::resource::Resource;
use crate::game::{EventInProgress, VillagerQuery};

use crate::map::MapPos;
use crate::obj::{ActiveTask, BaseAttrs, Obj, Order, State, SubclassNPC};
use crate::skill::{self, SkillData, Skills};
use crate::skill_defs::Skill;
use crate::templates::{SkillTemplates};


#[derive(Debug, Clone)]
pub struct VillagerUtil;

impl VillagerUtil {
    pub fn generate() {}

    pub fn generate_name() -> String {
        let names = vec![
            "Geoffry Holte",
            "Roderich Denholm",
            "Warder Folcey",
            "Andes Bardaye",
        ];

        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..names.len());

        return names[index].to_string();
    }

    pub fn generate_attributes(level: i32) -> BaseAttrs {
        let mut rng = rand::thread_rng();
        let random_range = 10 + level;

        let attrs = BaseAttrs {
            creativity: rng.gen_range(1..random_range),
            dexterity: rng.gen_range(1..random_range),
            endurance: rng.gen_range(1..random_range),
            focus: rng.gen_range(1..random_range),
            intellect: rng.gen_range(1..random_range),
            spirit: rng.gen_range(1..random_range),
            strength: rng.gen_range(1..random_range),
            toughness: rng.gen_range(1..random_range),
        };

        return attrs;
    }

    pub fn generate_skills(
        villager_id: i32,
        skill_templates: &SkillTemplates,
    ) -> Skills {

        let mut skills = Skills::new();

        let mut pool_of_skills = Vec::new();
        let mut gathering_skills =
            Skills::get_templates_by_class(skill::CLASS_GATHERING.to_string(), skill_templates);
        let mut crafting_skills =
            Skills::get_templates_by_class(skill::CLASS_CRAFTING.to_string(), skill_templates);

        pool_of_skills.append(&mut gathering_skills);
        pool_of_skills.append(&mut crafting_skills);

        let mut rng = rand::thread_rng();

        // Generate 3 random skills
        for _i in 0..3 {
            let index = rng.gen_range(0..pool_of_skills.len());
            let selected_skill_name = pool_of_skills.remove(index).name;
            let selected_skill_enum = Skill::from_str(&selected_skill_name).expect(&format!("Invalid skill name: {}", selected_skill_name));
            let random_xp = rng.gen_range(1..2000);

            skills.update(
                selected_skill_enum,
                random_xp,
                skill_templates,
            );
        }

        return skills;
    }

    pub fn order_to_speech(order: &Order) -> String {
        match order {
            Order::Follow { .. } => "On my way!".to_string(),
            Order::Explore { .. } => "Yes sir, exploring this area!".to_string(),
            Order::Gather { .. } => "Yes sir, gathering resources!".to_string(),
            Order::Operate { .. } => "Yes sir, operating this structure!".to_string(),
            Order::Plant { .. } => "Yes sir, off to plant the crops".to_string(),
            Order::Harvest { .. } => "Yes sir, time to harvest".to_string(),
            _ => "I'm speechless for this type of order".to_string(),
        }
    }

    pub fn order_to_activity(order: &Order) -> ActiveTask {
        let activity = match order {
            Order::Follow { .. } => ActiveTask::Following,
            Order::Build => ActiveTask::Building,
            Order::Gather { res_type, .. } => {
                ActiveTask::get_activity_from_res_type(res_type.clone())
            }
            Order::WorkQueue => ActiveTask::Operating,
            Order::Operate { .. } => ActiveTask::Operating,
            Order::Plant { .. } => ActiveTask::Planting,
            Order::Tend { .. } => ActiveTask::Tending,
            Order::Harvest { .. } => ActiveTask::Harvesting,
            Order::Explore { .. } => ActiveTask::Exploring,
            Order::Repair { .. } => ActiveTask::Repairing,
            Order::None => ActiveTask::Idle,
        };

        return activity;
    }

}
