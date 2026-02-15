use std::collections::HashMap;

use bevy::prelude::*;

use crate::obj::{Class, State};
use crate::item::{Inventory, Item};
use crate::{network, obj};
use crate::constants::*;
use crate::templates::{ObjTemplate, ObjTemplates, ResReq, Templates};

pub const RESOURCE: &str = "resource";
pub const CRAFT: &str = "craft";
pub const FARM: &str = "farm";
pub const SHELTER: &str = "shelter";
pub const STORAGE: &str = "storage";

pub const MINE: &str = "Mine";
pub const LUMBERCAMP: &str = "Lumbercamp";
pub const QUARRY: &str = "Quarry";

pub const WALL: &str = "Wall";

#[derive(Debug, Clone)]
pub struct Plan {
    player_id: i32,
    structure: String,
    level: i32,
    tier: i32,
}

#[derive(Resource, Deref, DerefMut, Debug)]
pub struct Plans(Vec<Plan>);

impl Plans {
    pub fn add(
        &mut self,
        player_id: i32,
        structure: String,
        level: i32,
        tier: i32
    ) {
        let plan = Plan {
            player_id: player_id,
            structure: structure,
            level: level,
            tier: tier,
        };

        self.push(plan);
    }
}

pub struct Structure;

impl Structure {

    pub fn available_to_build(
        player_id: i32,
        plans: Vec<Plan>,
        obj_templates: &ObjTemplates,
    ) -> Vec<network::Structure> {
        let mut available_list: Vec<network::Structure> = Vec::new();

        for plan in plans.iter() {
            if player_id == plan.player_id {
                for obj_template in obj_templates.iter() {
                    if plan.structure == obj_template.template {
                        let structure = network::Structure {
                            name: obj_template.template.clone(),
                            image: obj_template.image.clone(),
                            class: obj_template.class.clone(),
                            subclass: obj_template.subclass.clone(),
                            template: obj_template.template.clone(),
                            base_hp: obj_template.base_hp.unwrap_or_default(),
                            base_def: obj_template.base_def.unwrap_or_default(),
                            build_time: obj_template.build_cost.unwrap_or_default(),
                            req: obj_template.req.clone().unwrap_or_default(),
                            upgrade_req: obj_template.upgrade_req.clone().unwrap_or_default(),
                        };

                        available_list.push(structure);
                    }
                }
            }
        }

        return available_list;
    }

    pub fn get_template(template: String, obj_templates: &ObjTemplates) -> Option<ObjTemplate> {
        for obj_template in obj_templates.iter() {
            if obj_template.template == *template {
                return Some(obj_template.clone());
            }
        }

        return None;
    }

    pub fn get_template_by_name(name: String, obj_templates: &ObjTemplates) -> Option<ObjTemplate> {
        for obj_template in obj_templates.iter() {
            if obj_template.template == *name {
                return Some(obj_template.clone());
            }
        }

        return None;
    }

    pub fn process_req_items(
        structure_items: Vec<Item>,
        mut req_items: Vec<ResReq>,
    ) -> Vec<ResReq> {
        // Check current required quantity from structure items
        for req_item in req_items.iter_mut() {
            let mut req_quantity = req_item.quantity;

            for structure_item in structure_items.iter() {
                if req_item.req_type == structure_item.name
                    || req_item.req_type == structure_item.class
                    || req_item.req_type == structure_item.subclass
                {
                    if req_quantity - structure_item.quantity > 0 {
                        req_quantity -= structure_item.quantity;
                    } else {
                        req_quantity = 0;
                    }
                }
            }

            req_item.cquantity = Some(req_quantity);
        }

        return req_items;
    }

    pub fn get_current_req_quantities(
        target_template: String,
        target_class: String,
        target_state: State,
        inventory: &Inventory,
        templates: &Templates,
        selected_upgrade: Option<String>,
    ) -> Vec<ResReq> {
        if target_class == "structure" {
            let target_items = inventory.items.clone();
    
            if target_state == State::Founded {
                let structure_template = templates.obj_templates.get(target_template);
    
                let mut req_items = structure_template
                    .req
                    .expect("Template should have req field.");
    
                // Check current required quantity from structure items
                for req_item in req_items.iter_mut() {
                    let mut req_quantity = req_item.quantity;
    
                    for target_item in target_items.iter() {
                        if req_item.req_type == target_item.name
                            || req_item.req_type == target_item.class
                            || req_item.req_type == target_item.subclass
                        {
                            if req_quantity - target_item.quantity > 0 {
                                req_quantity -= target_item.quantity;
                            } else {
                                req_quantity = 0;
                            }
                        }
                    }
    
                    req_item.cquantity = Some(req_quantity);
                }
    
                return req_items;
            } else if target_state == State::PlanningUpgrade {
                let structure_template = templates.obj_templates.get_by_name_template(
                    selected_upgrade.expect("PlanningUpgrade and Selected Upgrade is None"),
                    target_template,
                );
    
                let mut req_items = structure_template
                    .upgrade_req
                    .expect("Template should have upgrade_req field.");
    
                // Check current required quantity from structure items
                for req_item in req_items.iter_mut() {
                    let mut req_quantity = req_item.quantity;
    
                    for target_item in target_items.iter() {
                        if req_item.req_type == target_item.name
                            || req_item.req_type == target_item.class
                            || req_item.req_type == target_item.subclass
                        {
                            if req_quantity - target_item.quantity > 0 {
                                req_quantity -= target_item.quantity;
                            } else {
                                req_quantity = 0;
                            }
                        }
                    }
    
                    req_item.cquantity = Some(req_quantity);
                }
    
                return req_items;
            }
        }
    
        // Return empty vector
        return Vec::new();
    }

    

    pub fn resource_type(structure_template: String) -> String {
        let resource: String = match structure_template.as_str() {
            MINE => ORE.to_string(),
            LUMBERCAMP => LOG.to_string(),
            QUARRY => STONE.to_string(),
            _ => "unknown".to_string(),
        };

        return resource;
    }

    pub fn is_built(state: State) -> bool {
        let is_built = state != State::Progressing || state != State::Upgrading || state != State::Stalled;

        return is_built;
    }
}

pub struct StructurePlugin;

impl Plugin for StructurePlugin {
    fn build(&self, app: &mut App) {
        let plans = Plans(Vec::new());

        app.insert_resource(plans);
    }
}
