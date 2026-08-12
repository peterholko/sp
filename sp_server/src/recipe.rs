use bevy::prelude::*;

use crate::item::{Inventory, Item};
use crate::skill::{SkillData, Skills};
use crate::skill_defs::Skill;
use crate::templates::{
    structure_supports_recipe_requirement, ItemAttr, ItemTemplate, RecipeTemplate, ResReq,
    Templates,
};
use crate::{item, network};

/// Pick the first known recipe whose station and ingredient requirements are
/// satisfied.
/// Used by villager auto-operation at food-production structures (Bakery,
/// Smokehouse, Millhouse, Butchery) so an assigned villager can produce without
/// the player picking a specific recipe.
pub fn pick_available_recipe_at(
    owner: i32,
    structure_name: &str,
    inventory: &Inventory,
    recipes: &Recipes,
    skills: &Skills,
    templates: &Templates,
) -> Option<Recipe> {
    for recipe in recipes.get_by_owner(owner) {
        if recipe.supports_structure(structure_name)
            && inventory
                .find_by_craft_reqs(recipe.req.clone(), None)
                .is_some()
            && recipe.skill_requirement_met(skills, templates)
        {
            return Some(recipe);
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct Recipe {
    pub name: String,
    pub class: String,
    pub subclass: String,
    pub image: String,
    pub weight: f32,
    pub durability: Option<i32>,
    pub attrs: Option<Vec<ItemAttr>>,
    pub owner: i32,
    pub tier: Option<i32>,
    pub slot: Option<item::Slot>,
    pub damage: Option<i32>,
    pub speed: Option<f32>,
    pub armor: Option<i32>,
    pub crafting_time: Option<i32>,
    pub structure_req: Option<Vec<String>>,
    pub stamina_req: Option<i32>,
    pub skill_req: Option<i32>,
    pub amount: Option<i32>,
    pub req: Vec<ResReq>,
    pub item_name_from_req: Option<bool>,
}

impl Recipe {
    pub fn requires_structure(&self) -> bool {
        self.structure_req.is_some()
    }

    pub fn supports_structure(&self, structure: &str) -> bool {
        self.structure_req.as_ref().is_some_and(|requirements| {
            requirements
                .iter()
                .any(|requirement| structure_supports_recipe_requirement(structure, requirement))
        })
    }

    pub fn crafting_skill(&self) -> Option<Skill> {
        SkillData::item_class_to_skill(&self.class)
            .or_else(|| SkillData::item_subclass_to_skill(&self.subclass))
    }

    pub fn skill_requirement_met(&self, skills: &Skills, templates: &Templates) -> bool {
        let requirement = self.skill_req.unwrap_or(0);
        if requirement <= 0 {
            return true;
        }

        self.crafting_skill().is_some_and(|skill| {
            skills.has_proficiency_requirement(skill, requirement, &templates.skill_templates)
        })
    }
}

#[derive(Resource, Debug)]
pub struct Recipes {
    recipes: Vec<Recipe>,
    recipe_templates: Vec<RecipeTemplate>,
}

impl Recipes {
    #[cfg(test)]
    pub fn from_recipes(recipes: Vec<Recipe>) -> Self {
        Self {
            recipes,
            recipe_templates: Vec::new(),
        }
    }

    pub fn set_templates(&mut self, recipe_templates: Vec<RecipeTemplate>) {
        self.recipe_templates = recipe_templates;
    }

    pub fn create(&mut self, player: i32, name: String, templates: &Templates) -> bool {
        if self
            .recipes
            .iter()
            .any(|recipe| recipe.owner == player && recipe.name == name)
        {
            return false;
        }

        for recipe_template in self.recipe_templates.iter() {
            if name == recipe_template.name {
                // Assume every recipe template has a equivalent item template
                let item_template =
                    Item::get_template(recipe_template.name.clone(), &templates.item_templates);

                let mut class = item_template.class.clone();
                let mut subclass = item_template.subclass.clone();
                let mut image = item_template.image.clone();
                let mut weight = item_template.weight.clone();
                let mut durability = item_template.durability.clone();
                let mut attrs = None;
                let mut slot = None;
                let mut structure_req = None;

                if let Some(item_template_attrs) = &item_template.attrs {
                    attrs = Some(item_template_attrs.clone());
                }

                if let Some(item_template_slot) = &item_template.slot {
                    slot = Some(item::Slot::str_to_slot(item_template_slot.clone()));
                }

                // Override with recipe template if it exists
                if let Some(recipe_template_class) = &recipe_template.class {
                    class = recipe_template_class.clone();
                }

                if let Some(recipe_template_subclass) = &recipe_template.subclass {
                    subclass = recipe_template_subclass.clone();
                }

                if let Some(recipe_template_image) = &recipe_template.image {
                    image = recipe_template_image.clone();
                }

                if let Some(recipe_template_weight) = &recipe_template.weight {
                    weight = recipe_template_weight.clone();
                }

                if let Some(recipe_template_durability) = &recipe_template.durability {
                    durability = Some(*recipe_template_durability);
                }

                if let Some(recipe_template_attrs) = &recipe_template.attrs {
                    attrs = Some(recipe_template_attrs.clone());
                }

                if let Some(recipe_template_slot) = &recipe_template.slot {
                    slot = Some(item::Slot::str_to_slot(recipe_template_slot.clone()));
                }

                if let Some(recipe_template_structure_req) = &recipe_template.structure_req {
                    structure_req = Some(recipe_template_structure_req.clone());
                }

                let new_recipe = Recipe {
                    name: recipe_template.name.clone(),
                    class: class,
                    subclass: subclass,
                    image: image,
                    weight: weight,
                    durability: durability,
                    attrs: attrs,
                    owner: player,
                    structure_req: structure_req,
                    tier: recipe_template.tier,
                    slot: slot,
                    damage: recipe_template.damage,
                    speed: recipe_template.speed,
                    armor: recipe_template.armor,
                    stamina_req: recipe_template.stamina_req,
                    crafting_time: recipe_template.crafting_time,
                    skill_req: recipe_template.skill_req,
                    amount: recipe_template.amount,
                    req: recipe_template.req.clone(),
                    item_name_from_req: recipe_template.item_name_from_req,
                };

                self.recipes.push(new_recipe);
                debug!("Recipes: {:?}", self.recipes);
                return true;
            }
        }

        false
    }

    pub fn get_by_name(&self, name: String) -> Option<Recipe> {
        for recipe in self.recipes.iter() {
            if recipe.name == *name {
                return Some(recipe.clone());
            }
        }

        return None;
    }

    pub fn get_for_owner_by_name(&self, owner: i32, name: &str) -> Option<Recipe> {
        self.recipes
            .iter()
            .find(|recipe| recipe.owner == owner && recipe.name == name)
            .cloned()
    }

    pub fn get_by_owner(&self, owner: i32) -> Vec<Recipe> {
        let mut owner_recipes: Vec<Recipe> = Vec::new();

        for recipe in self.recipes.iter() {
            if recipe.owner == owner {
                owner_recipes.push(recipe.clone());
            }
        }

        return owner_recipes;
    }

    pub fn get_basic_recipes_packet(&self, owner: i32) -> Vec<network::Recipe> {
        info!("Getting basic recipes");
        let mut basic_recipes: Vec<network::Recipe> = Vec::new();

        for recipe in self.recipes.iter() {
            info!("Recipe: {:?}", recipe);
            if recipe.owner == owner && !recipe.requires_structure() {
                info!("Basic Recipe: {:?}", recipe);
                let recipe_packet = network::Recipe {
                    name: recipe.name.clone(),
                    image: recipe.image.clone(),
                    class: recipe.class.clone(),
                    subclass: recipe.subclass.clone(),
                    tier: recipe.tier.clone(),
                    slot: item::Slot::to_str(recipe.slot.clone()),
                    damage: recipe.damage,
                    speed: recipe.speed,
                    armor: recipe.armor,
                    stamina_req: recipe.stamina_req,
                    crafting_time: recipe.crafting_time,
                    skill_req: recipe.skill_req,
                    weight: recipe.weight,
                    amount: recipe.amount,
                    req: recipe.req.clone(),
                };

                basic_recipes.push(recipe_packet);
            }
        }

        return basic_recipes;
    }

    pub fn get_by_structure_packet(&self, owner: i32, structure: String) -> Vec<network::Recipe> {
        let mut owner_recipes: Vec<network::Recipe> = Vec::new();

        for recipe in self.recipes.iter() {
            // Remove all whitespaces

            info!(
                "Structure Req: {:?} Structure: {:?}",
                recipe.structure_req.clone(),
                structure.clone()
            );

            if recipe.structure_req.is_some() {
                if recipe.owner == owner && recipe.supports_structure(&structure) {
                    let recipe_packet = network::Recipe {
                        name: recipe.name.clone(),
                        image: recipe.image.clone(),
                        class: recipe.class.clone(),
                        subclass: recipe.subclass.clone(),
                        tier: recipe.tier.clone(),
                        slot: item::Slot::to_str(recipe.slot.clone()),
                        damage: recipe.damage,
                        speed: recipe.speed,
                        armor: recipe.armor,
                        stamina_req: recipe.stamina_req,
                        crafting_time: recipe.crafting_time,
                        skill_req: recipe.skill_req,
                        weight: recipe.weight,
                        amount: recipe.amount,
                        req: recipe.req.clone(),
                    };

                    owner_recipes.push(recipe_packet);
                }
            }
        }

        return owner_recipes;
    }

    pub fn get_by_subclass_tier(
        structure: String,
        subclass: String,
        tier: Option<i32>,
        templates: &Templates,
    ) -> Vec<RecipeTemplate> {
        let all_recipes = RecipeTemplate::get_by_structure(structure, templates);

        let mut recipes_by_subclass_tier = Vec::new();

        for recipe in all_recipes.iter() {
            if recipe.tier != tier {
                continue;
            }

            if let Some(recipe_subclass) = &recipe.subclass {
                if *recipe_subclass == subclass {
                    recipes_by_subclass_tier.push(recipe.clone());
                }
            } else {
                // If recipe subclass is not set, get subclass from item template
                let item_template =
                    Item::get_template(recipe.name.clone(), &templates.item_templates);

                if item_template.subclass == subclass {
                    recipes_by_subclass_tier.push(recipe.clone());
                }
            }
        }

        return recipes_by_subclass_tier;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_recipe(name: &str, structure_req: Option<Vec<String>>) -> Recipe {
        Recipe {
            name: name.to_string(),
            class: "Test".to_string(),
            subclass: "Test".to_string(),
            image: "test".to_string(),
            weight: 1.0,
            durability: None,
            attrs: None,
            owner: 1,
            tier: None,
            slot: None,
            damage: None,
            speed: None,
            armor: None,
            crafting_time: Some(10),
            structure_req,
            stamina_req: None,
            skill_req: None,
            amount: Some(1),
            req: Vec::new(),
            item_name_from_req: None,
        }
    }

    #[test]
    fn requires_structure_tracks_structure_requirement() {
        assert!(!test_recipe("Firewood", None).requires_structure());
        assert!(
            test_recipe("Cooked Meat", Some(vec!["Crafting Tent".to_string()]))
                .requires_structure()
        );
    }

    #[test]
    fn shelter_tent_inherits_every_campfire_recipe() {
        let cooked_meat = test_recipe(
            "Cooked Meat",
            Some(vec!["Campfire".to_string(), "Crafting Tent".to_string()]),
        );
        assert!(cooked_meat.supports_structure("Campfire"));
        assert!(cooked_meat.supports_structure("Shelter Tent"));
        assert!(!cooked_meat.supports_structure("Burrow"));

        let recipes = Recipes::from_recipes(vec![cooked_meat]);
        assert_eq!(
            recipes
                .get_by_structure_packet(1, "Shelter Tent".to_string())
                .into_iter()
                .map(|recipe| recipe.name)
                .collect::<Vec<_>>(),
            vec!["Cooked Meat"]
        );
    }

    #[test]
    fn basic_recipe_packet_only_includes_hand_recipes() {
        let mut other_player_recipe = test_recipe("Other Firewood", None);
        other_player_recipe.owner = 2;
        let recipes = Recipes::from_recipes(vec![
            test_recipe("Firewood", None),
            test_recipe("Crude Torch", None),
            test_recipe("Cooked Meat", Some(vec!["Crafting Tent".to_string()])),
            test_recipe(
                "Training Pick Axe",
                Some(vec!["Crafting Tent".to_string(), "Blacksmith".to_string()]),
            ),
            other_player_recipe,
        ]);

        let names: Vec<String> = recipes
            .get_basic_recipes_packet(1)
            .into_iter()
            .map(|recipe| recipe.name)
            .collect();

        assert_eq!(names, vec!["Firewood", "Crude Torch"]);
    }

    #[test]
    fn recipe_lookup_is_scoped_to_owner() {
        let mut player_two_recipe = test_recipe("Firewood", None);
        player_two_recipe.owner = 2;
        let recipes = Recipes::from_recipes(vec![test_recipe("Firewood", None), player_two_recipe]);

        assert_eq!(
            recipes
                .get_for_owner_by_name(1, "Firewood")
                .expect("player one recipe")
                .owner,
            1
        );
        assert_eq!(
            recipes
                .get_for_owner_by_name(2, "Firewood")
                .expect("player two recipe")
                .owner,
            2
        );
        assert!(recipes.get_for_owner_by_name(3, "Firewood").is_none());
    }
}

pub struct RecipePlugin;

impl Plugin for RecipePlugin {
    fn build(&self, app: &mut App) {
        let recipes = Recipes {
            recipes: Vec::new(),
            recipe_templates: Vec::new(),
        };

        app.insert_resource(recipes);
    }
}
