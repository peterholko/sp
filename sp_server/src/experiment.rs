use std::collections::HashMap;

use bevy::prelude::*;
use rand::Rng;

use crate::{
    item::{req_matches, Inventory, Item, Items},
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
            if res_req.quantity <= 0 {
                continue;
            }
            let mut req_match = false;

            for reagent in experiment_reagents.iter() {
                if req_matches(
                    &res_req.req_type,
                    &reagent.name,
                    &reagent.class,
                    &reagent.subclass,
                ) {
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

        let mut available = experiment_reagents
            .iter()
            .map(|reagent| reagent.quantity)
            .collect::<Vec<_>>();
        let mut consumed = HashMap::<i32, i32>::new();

        for res_req in experiment.req.iter_mut() {
            debug!("exp res_req: {:?}", res_req);
            for (index, reagent) in experiment_reagents.iter().enumerate() {
                if res_req.quantity <= 0 {
                    break;
                }
                debug!("reagent: {:?}", reagent);
                if req_matches(
                    &res_req.req_type,
                    &reagent.name,
                    &reagent.class,
                    &reagent.subclass,
                ) {
                    let quantity = res_req.quantity.min(available[index]);
                    if quantity > 0 {
                        res_req.quantity -= quantity;
                        available[index] -= quantity;
                        *consumed.entry(reagent.id).or_default() += quantity;
                    }
                }
            }
        }
        for (item_id, quantity) in consumed {
            inventory.remove_quantity(item_id, quantity);
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
        player_id: i32,
        structure_id: i32,
        structure_name: String,
        structure_inventory: &Inventory,
        recipes: &Recipes,
        templates: &Templates,
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

        // Tiered experimentation advances vertically. Untiered families
        // (bows, leather gear, food, and tools) discover peers in the same
        // family instead of being excluded from experimentation entirely.
        let target_recipe_tier = source_recipe.tier.map(|tier| tier + 1);

        // Get source recipe subclass from recipe template or item template
        let source_recipe_subclass = if let Some(source_recipe_subclass) = source_recipe.subclass {
            source_recipe_subclass
        } else {
            let item_template =
                Item::get_template(source_recipe.name.clone(), &templates.item_templates);
            item_template.subclass.clone()
        };

        let player_recipes = recipes.get_by_owner(player_id);
        debug!("player_recipes: {:?}", player_recipes);

        // Find the next tier for tiered recipes, or another member of the
        // untiered family.
        let matching_recipe_templates = Recipes::get_by_subclass_tier(
            structure_name,
            source_recipe_subclass,
            target_recipe_tier,
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
                    if req_matches(
                        &req.req_type,
                        &reagent.name,
                        &reagent.class,
                        &reagent.subclass,
                    ) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ids::Ids, recipe::RecipePlugin, templates::TemplatesPlugin};

    #[test]
    fn discovery_advances_one_tier_and_is_scoped_to_the_player() {
        let mut app = App::new();
        app.add_plugins((TemplatesPlugin, RecipePlugin));

        app.world_mut()
            .resource_scope(|world, templates: Mut<Templates>| {
                let recipe_templates = templates.recipe_templates.to_vec();
                {
                    let mut recipes = world.resource_mut::<Recipes>();
                    recipes.set_templates(recipe_templates);
                    assert!(recipes.create(1, "Copper Short Sword".to_string(), &templates,));
                    assert!(recipes.create(2, "Copper Short Sword".to_string(), &templates,));
                    assert!(recipes.create(2, "Iron Sword".to_string(), &templates));
                }

                let mut ids = Ids::default();
                let mut inventory = Inventory {
                    owner: 10,
                    items: Vec::new(),
                };
                let source = inventory.new(
                    ids.new_item_id(),
                    "Copper Short Sword".to_string(),
                    1,
                    &templates.item_templates,
                );
                let iron = inventory.new(
                    ids.new_item_id(),
                    "Quickforge Iron Ingot".to_string(),
                    1,
                    &templates.item_templates,
                );
                let timber = inventory.new(
                    ids.new_item_id(),
                    "Cragroot Maple Timber".to_string(),
                    1,
                    &templates.item_templates,
                );
                inventory.set_experiment_source(source.id);
                inventory.set_experiment_reagent(iron.id);
                inventory.set_experiment_reagent(timber.id);

                let recipes = world.resource::<Recipes>();
                let player_one_result = Experiment::find_recipe(
                    1,
                    10,
                    "Blacksmith".to_string(),
                    &inventory,
                    &recipes,
                    &templates,
                )
                .expect("player one should discover the next sword tier");
                assert_eq!(player_one_result.name, "Iron Sword");
                assert_eq!(player_one_result.tier, Some(2));

                assert!(Experiment::find_recipe(
                    2,
                    10,
                    "Blacksmith".to_string(),
                    &inventory,
                    &recipes,
                    &templates,
                )
                .is_none());
            });
    }

    #[test]
    fn primitive_spear_is_a_valid_copper_recipe_root() {
        let mut app = App::new();
        app.add_plugins((TemplatesPlugin, RecipePlugin));

        app.world_mut()
            .resource_scope(|world, templates: Mut<Templates>| {
                let recipe_templates = templates.recipe_templates.to_vec();
                world
                    .resource_mut::<Recipes>()
                    .set_templates(recipe_templates);

                let mut ids = Ids::default();
                let mut inventory = Inventory {
                    owner: 10,
                    items: Vec::new(),
                };
                let source = inventory.new(
                    ids.new_item_id(),
                    "Bone-Tipped Spear".to_string(),
                    1,
                    &templates.item_templates,
                );
                let copper = inventory.new(
                    ids.new_item_id(),
                    "Valleyrun Copper Ingot".to_string(),
                    1,
                    &templates.item_templates,
                );
                let timber = inventory.new(
                    ids.new_item_id(),
                    "Cragroot Maple Timber".to_string(),
                    2,
                    &templates.item_templates,
                );
                inventory.set_experiment_source(source.id);
                inventory.set_experiment_reagent(copper.id);
                inventory.set_experiment_reagent(timber.id);

                let result = Experiment::find_recipe(
                    1,
                    10,
                    "Blacksmith".to_string(),
                    &inventory,
                    &world.resource::<Recipes>(),
                    &templates,
                )
                .expect("primitive spear should lead to the copper spear");
                assert_eq!(result.name, "Copper Spear");
                assert_eq!(result.tier, Some(1));
            });
    }

    #[test]
    fn bow_experimentation_advances_one_material_tier_at_a_time() {
        let mut app = App::new();
        app.add_plugins((TemplatesPlugin, RecipePlugin));

        app.world_mut()
            .resource_scope(|world, templates: Mut<Templates>| {
                let recipe_templates = templates.recipe_templates.to_vec();
                world
                    .resource_mut::<Recipes>()
                    .set_templates(recipe_templates);

                let mut ids = Ids::default();
                let mut primitive_inventory = Inventory {
                    owner: 10,
                    items: Vec::new(),
                };
                let training_bow = primitive_inventory.new(
                    ids.new_item_id(),
                    "Training Bow".to_string(),
                    1,
                    &templates.item_templates,
                );
                let timber = primitive_inventory.new(
                    ids.new_item_id(),
                    "Cragroot Maple Timber".to_string(),
                    3,
                    &templates.item_templates,
                );
                let twine = primitive_inventory.new(
                    ids.new_item_id(),
                    "Twine".to_string(),
                    1,
                    &templates.item_templates,
                );
                primitive_inventory.set_experiment_source(training_bow.id);
                primitive_inventory.set_experiment_reagent(timber.id);
                primitive_inventory.set_experiment_reagent(twine.id);

                let hunting_bow = Experiment::find_recipe(
                    1,
                    10,
                    "Workshop".to_string(),
                    &primitive_inventory,
                    &world.resource::<Recipes>(),
                    &templates,
                )
                .expect("training bow should lead to hunting bow");
                assert_eq!(hunting_bow.name, "Hunting Bow");
                assert_eq!(hunting_bow.tier, Some(1));

                let mut iron_inventory = Inventory {
                    owner: 11,
                    items: Vec::new(),
                };
                let hunting_bow = iron_inventory.new(
                    ids.new_item_id(),
                    "Hunting Bow".to_string(),
                    1,
                    &templates.item_templates,
                );
                let timber = iron_inventory.new(
                    ids.new_item_id(),
                    "Cragroot Maple Timber".to_string(),
                    3,
                    &templates.item_templates,
                );
                let iron = iron_inventory.new(
                    ids.new_item_id(),
                    "Quickforge Iron Ingot".to_string(),
                    1,
                    &templates.item_templates,
                );
                let twine = iron_inventory.new(
                    ids.new_item_id(),
                    "Twine".to_string(),
                    1,
                    &templates.item_templates,
                );
                iron_inventory.set_experiment_source(hunting_bow.id);
                iron_inventory.set_experiment_reagent(timber.id);
                iron_inventory.set_experiment_reagent(iron.id);
                iron_inventory.set_experiment_reagent(twine.id);

                let iron_bow = Experiment::find_recipe(
                    1,
                    11,
                    "Workshop".to_string(),
                    &iron_inventory,
                    &world.resource::<Recipes>(),
                    &templates,
                )
                .expect("hunting bow should lead to iron-limbed longbow");
                assert_eq!(iron_bow.name, "Iron-Limbed Longbow");
                assert_eq!(iron_bow.tier, Some(2));
            });
    }
}
