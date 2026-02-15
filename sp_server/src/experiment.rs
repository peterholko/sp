use std::collections::HashMap;

use bevy::prelude::*;
use rand::Rng;

use crate::{
    item::{Inventory, Item, Items},
    network::{self},
    recipe::Recipes,
    templates::{RecipeTemplate, RecipeTemplates, ResReq, Templates},
};

pub const EXP_STATE_NONE: &str = "Not Started";

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExperimentState {
    None,
    Waiting,
    Progressing,
    Near,
    Discovery,
    TrivialSource,
}

#[derive(Debug, Clone)]
pub struct Experiment {
    pub structure: i32,
    pub recipe: Option<RecipeTemplate>,
    pub state: ExperimentState,
    pub source_item: Option<Item>,
    pub req: Vec<ResReq>,
}

#[derive(Resource, Deref, DerefMut, Debug)]
pub struct Experiments(HashMap<i32, Experiment>);

impl Experiment {
    pub fn create(
        structure_id: i32,
        recipe: Option<RecipeTemplate>,
        state: ExperimentState,
        source_item: Item,
        req: Vec<ResReq>,
        experiments: &mut ResMut<Experiments>,
    ) -> Experiment {
        let experiment = Experiment {
            structure: structure_id,
            recipe: recipe,
            state: state,
            source_item: Some(source_item),
            req: req,
        };

        experiments.insert(structure_id, experiment.clone());

        return experiment.clone(); //Read only
    }

    pub fn set_recipe(recipe: RecipeTemplate, experiment: &mut Experiment) {
        experiment.recipe = Some(recipe.clone());
        experiment.req = recipe.req.clone();
        experiment.state = ExperimentState::Progressing;
    }

    pub fn set_trivial_source(experiment: &mut Experiment) {
        experiment.recipe = None;
        experiment.state = ExperimentState::TrivialSource;
    }

    pub fn update_state(
        structure_id: i32,
        state: ExperimentState,
        experiments: &mut ResMut<Experiments>,
    ) -> Option<Experiment> {
        if let Some(experiment) = experiments.get_mut(&structure_id) {
            experiment.state = state;

            //Returns read only
            return Some(experiment.clone());
        }

        return None;
    }

    pub fn reset(experiment: &mut Experiment) {
        experiment.source_item = None;
        experiment.state = ExperimentState::None;
        experiment.recipe = None;
    }

    pub fn check_reqs(
        structure_id: i32,
        experiment: &mut Experiment,
        structure_inventory: &Inventory,
    ) -> bool {
        // Check source item is set
        if experiment.source_item.is_none() {
            return false;
        }

        // Check reagents of experiment
        let (_experiment_source, experiment_reagents) =
            structure_inventory.get_experiment_source_reagents();
        let mut all_reqs_match = true;

        for res_req in experiment.req.iter() {
            let mut req_match = false;

            for reagent in experiment_reagents.iter() {
                if res_req.req_type == reagent.subclass {
                    req_match = true;
                    break;
                }
            }

            if !req_match {
                all_reqs_match = false;
            }
        }

        return all_reqs_match;
    }

    pub fn check_discovery(
        player_id: i32,
        structure_id: i32,
        experiment: &mut Experiment,
        inventory: &mut Inventory,
        templates: &Res<Templates>,
        recipes: &mut ResMut<Recipes>,
    ) -> ExperimentState {
        let Some(source_item) = &experiment.source_item else {
            debug!("No source item: {:?}", experiment);
            return ExperimentState::None;
        };

        let Some(recipe) = &experiment.recipe else {
            debug!("No recipe: {:?}", experiment);
            return ExperimentState::None;
        };

        let mut res_reqs_reached = true;
        let (_exp_source, experiment_reagents) = inventory.get_experiment_source_reagents();

        for res_req in experiment.req.iter_mut() {
            debug!("exp res_req: {:?}", res_req);
            for reagent in experiment_reagents.iter() {
                debug!("reagent: {:?}", reagent);
                if res_req.req_type == reagent.subclass {
                    if res_req.quantity > 0 {
                        res_req.quantity -= 1;
                    }

                    inventory.remove_quantity(reagent.id, 1);
                }
            }
        }

        // Check if minimum required resources reached
        for res_req in experiment.req.iter() {
            debug!("res_req: {:?}", res_req);
            if res_req.quantity > 0 {
                res_reqs_reached = false;
            }
        }

        debug!("experiment reagent reqs: {:?}", res_reqs_reached);
        if res_reqs_reached {
            let mut rng = rand::thread_rng();
            let chance = rng.gen_range(0..100);

            debug!("experiment chance: {:?}", chance);
            if chance < 99 {
                debug!("Discovered new recipe!");

                // Add new recipe
                recipes.create(player_id, recipe.name.clone(), &templates);

                // Remove source
                inventory.remove_item(source_item.id);

                // Set experiment to discovery
                experiment.source_item = None;
                experiment.state = ExperimentState::Discovery;
            } else {
                experiment.state = ExperimentState::Near;
            }
        }

        return experiment.state.clone();
    }

    pub fn find_recipe(
        structure_id: i32,
        structure_name: String,
        structure_inventory: &Inventory,
        recipes: &ResMut<Recipes>,
        templates: &Res<Templates>,
    ) -> Option<RecipeTemplate> {
        let (experiment_source, experiment_reagents) =
            structure_inventory.get_experiment_source_reagents();

        let Some(experiment_source) = experiment_source else {
            debug!(
                "Experiment source is not set, structure id: {:?}",
                structure_id
            );
            return None;
        };

        let source_recipe = RecipeTemplate::get_by_name(experiment_source.name.clone(), templates);

        let Some(source_recipe) = source_recipe else {
            debug!(
                "Source item recipe cannot be found, experiment source name: {:?}",
                experiment_source.name
            );
            return None;
        };

        let Some(source_recipe_tier) = source_recipe.tier else {
            debug!(
                "Source recipe does not have a tier attribute {:?}",
                source_recipe
            );
            return None;
        };

        // Get source recipe subclass from recipe template or item template
        let source_recipe_subclass = if let Some(source_recipe_subclass) = source_recipe.subclass {
            source_recipe_subclass
        } else {
            let item_template =
                Item::get_template(source_recipe.name.clone(), &templates.item_templates);
            item_template.subclass.clone()
        };

        let player_recipes = recipes.get_by_structure(structure_id);
        debug!("player_recipes: {:?}", player_recipes);

        // Find matching recipes to the source by tier and subclass
        let matching_recipe_templates = Recipes::get_by_subclass_tier(
            structure_name,
            source_recipe_subclass,
            source_recipe_tier,
            templates,
        );
        debug!("matching_recipe_templates: {:?}", matching_recipe_templates);

        let mut undiscovered_recipes = Vec::new();

        // Remove the recipes the player has already discovered
        for recipe_template in matching_recipe_templates.iter() {
            debug!("recipe_template: {:?}", recipe_template);
            let mut discovered = false;

            for player_recipe in player_recipes.iter() {
                debug!(
                    "player_recipe: {:?} recipe_template: {:?}",
                    player_recipe, recipe_template
                );
                if recipe_template.name == player_recipe.name {
                    discovered = true;
                }
            }

            debug!("discovered: {:?}", discovered);
            if !discovered {
                undiscovered_recipes.push(recipe_template.clone());
            }
        }

        debug!("undiscovered_recipes: {:?}", undiscovered_recipes);
        let mut valid_undiscovered_recipes = Vec::new();

        for undiscovered_recipe in undiscovered_recipes.iter() {
            let mut all_matched = true;

            for req in undiscovered_recipe.req.iter() {
                let mut req_matched = false;

                for reagent in experiment_reagents.iter() {
                    if reagent.subclass == req.req_type {
                        req_matched = true;
                    }
                }

                if !req_matched {
                    all_matched = false;
                }
            }

            if all_matched {
                valid_undiscovered_recipes.push(undiscovered_recipe);
            }
        }
        debug!(
            "Valid undiscovered recipes: {:?}",
            valid_undiscovered_recipes
        );

        if valid_undiscovered_recipes.is_empty() {
            return None;
        }

        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..valid_undiscovered_recipes.len());

        let experiement_recipe = valid_undiscovered_recipes[index];

        return Some(experiement_recipe.clone());
    }

    pub fn state_to_string(state: ExperimentState) -> String {
        let state_str = match state {
            ExperimentState::None => "Not Started",
            ExperimentState::Waiting => "Waiting for experimenter",
            ExperimentState::Progressing => "Experimenting",
            ExperimentState::Near => "Near Breakthrough",
            ExperimentState::Discovery => "Eureka!",
            ExperimentState::TrivialSource => "Trivial source item",
        };

        return state_str.to_string();
    }

    pub fn recipe_to_packet(
        experiment: Experiment,
        templates: &Res<Templates>,
    ) -> Option<network::Recipe> {
        let Some(recipe_template) = experiment.recipe else {
            return None;
        };

        if experiment.state == ExperimentState::Discovery {
            let class = if let Some(recipe_template_class) = recipe_template.class {
                recipe_template_class
            } else {
                let item_template =
                    Item::get_template(recipe_template.name.clone(), &templates.item_templates);
                item_template.class.clone()
            };

            let subclass = if let Some(recipe_template_subclass) = recipe_template.subclass {
                recipe_template_subclass
            } else {
                let item_template =
                    Item::get_template(recipe_template.name.clone(), &templates.item_templates);
                item_template.subclass.clone()
            };

            let image = if let Some(recipe_template_image) = recipe_template.image {
                recipe_template_image
            } else {
                let item_template =
                    Item::get_template(recipe_template.name.clone(), &templates.item_templates);
                item_template.image.clone()
            };

            let weight = if let Some(recipe_template_weight) = recipe_template.weight {
                recipe_template_weight
            } else {
                let item_template =
                    Item::get_template(recipe_template.name.clone(), &templates.item_templates);
                item_template.weight.clone()
            };

            let mut slot = None;

            if let Some(recipe_template_slot) = recipe_template.slot {
                slot = Some(recipe_template_slot);
            }

            let recipe = network::Recipe {
                name: recipe_template.name,
                class: class,
                subclass: subclass,
                image: image,
                weight: weight,
                tier: recipe_template.tier,
                slot: slot,
                damage: recipe_template.damage,
                speed: recipe_template.speed,
                armor: recipe_template.armor,
                stamina_req: recipe_template.stamina_req,
                crafting_time: recipe_template.crafting_time,
                skill_req: recipe_template.skill_req,
                amount: recipe_template.amount,
                req: recipe_template.req,
            };

            return Some(recipe);
        } else {
            return None;
        }
    }
}

pub struct ExperimentPlugin;

impl Plugin for ExperimentPlugin {
    fn build(&self, app: &mut App) {
        let experiments = Experiments(HashMap::new());

        app.insert_resource(experiments);
    }
}
