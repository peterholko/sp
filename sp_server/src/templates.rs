use bevy::prelude::*;

use std::collections::HashMap;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use std::fs;

use crate::constants::{BASE_REFINE_TIME, TICKS_PER_SEC};
use crate::item::AttrKey;
use crate::item::{AttrVal, LOGS_OR_TIMBER};

pub const SHELTER_TENT_TEMPLATE: &str = "Shelter Tent";
pub const LEGACY_SMALL_TENT_TEMPLATE: &str = "Small Tent";
pub const CAMPFIRE_TEMPLATE: &str = "Campfire";
pub const DROPPED_BAG_TEMPLATE: &str = "Dropped Bag";

pub fn canonical_obj_template_name(name: &str) -> &str {
    match name {
        LEGACY_SMALL_TENT_TEMPLATE => SHELTER_TENT_TEMPLATE,
        _ => name,
    }
}

/// Structure upgrades can retain capabilities from the structure they replace.
/// A Shelter Tent contains the upgraded Campfire, so every recipe that accepts
/// a Campfire must also accept the combined shelter without duplicating that
/// requirement across individual recipe templates.
pub fn structure_supports_recipe_requirement(structure: &str, requirement: &str) -> bool {
    structure == requirement
        || (structure == SHELTER_TENT_TEMPLATE && requirement == CAMPFIRE_TEMPLATE)
}

#[derive(Debug, Resource)]
pub struct Templates {
    pub item_templates: Vec<ItemTemplate>,
    pub res_templates: ResTemplates,
    pub skill_templates: SkillTemplates,
    pub obj_templates: ObjTemplates,
    pub recipe_templates: RecipeTemplates,
    pub effect_templates: EffectTemplates,
    pub combo_templates: ComboTemplates,
    pub res_property_templates: ResPropertyTemplates,
    pub terrain_feature_templates: TerrainFeatureTemplates,
    pub dialogue_templates: DialogueTemplates,
    pub price_templates: PriceTemplates,
}

impl Templates {
    pub fn get_dialogue(&self, name: &str) -> String {
        if let Some(dialogue) = self.dialogue_templates.get(name) {
            return dialogue.text.clone();
        } else {
            return format!("No dialogue found for {:?}", name);
        }
    }

    pub fn get_item_templates_by_class(&self, class: &str) -> Vec<ItemTemplate> {
        let mut item_templates = Vec::new();

        for item_template in self.item_templates.iter() {
            if item_template.class == class {
                item_templates.push(item_template.clone());
            }
        }

        return item_templates;
    }

    pub fn get_item_templates_by_subclass(&self, subclass: &str) -> Vec<ItemTemplate> {
        let mut item_templates = Vec::new();

        for item_template in self.item_templates.iter() {
            if item_template.subclass == subclass {
                item_templates.push(item_template.clone());
            }
        }

        return item_templates;
    }

    /// Validate links that must resolve before a production chain can run.
    ///
    /// Item and recipe execution deliberately use strict template lookups, so
    /// a misspelled station or missing output is a startup error rather than a
    /// late gameplay panic.
    pub fn validate_production_catalog(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut item_names = HashSet::new();
        let mut item_types = HashSet::new();
        let mut structure_names = HashSet::new();

        for item in self.item_templates.iter() {
            if !item_names.insert(item.name.as_str()) {
                errors.push(format!("duplicate item template {:?}", item.name));
            }
            item_types.insert(item.name.as_str());
            item_types.insert(item.class.as_str());
            item_types.insert(item.subclass.as_str());
        }

        for structure in self.obj_templates.iter() {
            if !structure_names.insert(structure.template.as_str()) {
                errors.push(format!(
                    "duplicate object template {:?}",
                    structure.template
                ));
            }

            for requirement in structure
                .req
                .iter()
                .flatten()
                .chain(structure.upgrade_req.iter().flatten())
                .chain(structure.upkeep.iter().flatten())
            {
                if requirement.req_type != LOGS_OR_TIMBER
                    && !item_types.contains(requirement.req_type.as_str())
                {
                    errors.push(format!(
                        "object {:?} requires unknown item type {:?}",
                        structure.template, requirement.req_type
                    ));
                }
            }
        }

        for structure in self.obj_templates.iter() {
            for target in structure.upgrade_to.iter().flatten() {
                if !structure_names.contains(target.as_str()) {
                    errors.push(format!(
                        "object {:?} upgrades to unknown object {:?}",
                        structure.template, target
                    ));
                }
            }
            for refine_type in structure.refine.iter().flatten() {
                if !item_types.contains(refine_type.as_str()) {
                    errors.push(format!(
                        "object {:?} refines unknown item type {:?}",
                        structure.template, refine_type
                    ));
                }
            }
        }

        let mut recipe_names = HashSet::new();
        for recipe in self.recipe_templates.iter() {
            if !recipe_names.insert(recipe.name.as_str()) {
                errors.push(format!("duplicate recipe template {:?}", recipe.name));
            }
            if !item_names.contains(recipe.name.as_str()) {
                errors.push(format!(
                    "recipe {:?} has no matching output item template",
                    recipe.name
                ));
            }
            for requirement in recipe.req.iter() {
                if !item_types.contains(requirement.req_type.as_str()) {
                    errors.push(format!(
                        "recipe {:?} requires unknown item type {:?}",
                        recipe.name, requirement.req_type
                    ));
                }
            }
            for station in recipe.structure_req.iter().flatten() {
                if !structure_names.contains(station.as_str()) {
                    errors.push(format!(
                        "recipe {:?} requires unknown structure {:?}",
                        recipe.name, station
                    ));
                }
            }
        }

        for item in self.item_templates.iter() {
            for output in item.produces.iter().flatten() {
                if !item_names.contains(output.as_str()) {
                    errors.push(format!(
                        "item {:?} refines to unknown item {:?}",
                        item.name, output
                    ));
                }
            }
        }

        for resource in self.res_templates.values() {
            for output in resource.produces.iter().flatten() {
                if !item_names.contains(output.as_str()) {
                    errors.push(format!(
                        "resource {:?} produces unknown item {:?}",
                        resource.name, output
                    ));
                }
            }

            // These resource records are interaction anchors; their dedicated
            // systems select a concrete water or fish item.
            if resource.produces.is_none()
                && resource.res_type != "Spring Water"
                && resource.res_type != "Fish"
                && !item_names.contains(resource.name.as_str())
            {
                errors.push(format!(
                    "resource {:?} has no matching gathered item template",
                    resource.name
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            errors.sort();
            errors.dedup();
            Err(errors)
        }
    }

    pub fn get_obj_template_by_name(&self, name: String) -> ObjTemplate {
        let name = canonical_obj_template_name(&name);
        for obj_template in self.obj_templates.iter() {
            if name == obj_template.template {
                return obj_template.clone();
            }
        }

        // Cannot recover from an invalid obj template
        panic!("Cannot find obj_template: {:?}", name);
    }

    #[cfg(test)]
    pub fn from_obj_templates(obj_templates: Vec<ObjTemplate>) -> Self {
        Self {
            item_templates: vec![],
            res_templates: ResTemplates(HashMap::new()),
            skill_templates: SkillTemplates(HashMap::new()),
            obj_templates: ObjTemplates(obj_templates),
            recipe_templates: RecipeTemplates(vec![]),
            effect_templates: EffectTemplates(HashMap::new()),
            combo_templates: ComboTemplates(HashMap::new()),
            res_property_templates: ResPropertyTemplates(HashMap::new()),
            terrain_feature_templates: TerrainFeatureTemplates(HashMap::new()),
            dialogue_templates: DialogueTemplates(HashMap::new()),
            price_templates: PriceTemplates(HashMap::new()),
        }
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct ObjTemplates(Vec<ObjTemplate>);

impl ObjTemplates {
    pub fn get(&self, template: String) -> ObjTemplate {
        let template = canonical_obj_template_name(&template);
        for obj_template in self.iter() {
            if template == obj_template.template {
                return obj_template.clone();
            }
        }

        // Cannot recover from an invalid obj template
        panic!("Cannot find obj_template: {:?}", template);
    }

    pub fn get_by_name_template(&self, name: String, template: String) -> ObjTemplate {
        // TODO reconsider name vs template

        let name = canonical_obj_template_name(&name);
        let template = canonical_obj_template_name(&template);

        // Check by name first
        for obj_template in self.iter() {
            if name == obj_template.template {
                return obj_template.clone();
            }
        }

        // Check by template name second
        for obj_template in self.iter() {
            if template == obj_template.template {
                return obj_template.clone();
            }
        }

        // Cannot recover from an invalid obj template
        panic!("Cannot find obj_template: {:?}", name);
    }

    pub fn get_capacity(&self, name: String) -> i32 {
        let name = canonical_obj_template_name(&name);
        for obj_template in self.iter() {
            if name == obj_template.template {
                if let Some(capacity) = obj_template.capacity {
                    return capacity;
                } else {
                    return 0;
                }
            }
        }

        // Cannot recover from an invalid obj template
        panic!("Cannot find obj_template: {:?}", name);
    }
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct ResReq {
    #[serde(rename = "type")]
    pub req_type: String,
    pub quantity: i32,
    pub cquantity: Option<i32>, // current quantity
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
// Another way to build the struct...
/*pub struct ObjTemplate {
    pub name: String,
    pub class: String,
    pub subclass: String,
    pub template: String,
    #[serde(flatten)]
    pub attrs: HashMap<String, Value>
}*/
pub struct ObjTemplate {
    pub class: String,
    pub subclass: String,
    pub template: String,
    pub image: String,
    pub family: Option<String>,
    pub groups: Option<Vec<String>>,
    pub base_hp: Option<i32>,
    pub base_stamina: Option<i32>,
    #[serde(default)]
    pub base_mana: Option<i32>,
    pub base_dmg: Option<i32>,
    pub dmg_range: Option<i32>,
    pub base_def: Option<i32>,
    pub base_speed: Option<i32>,
    pub base_vision: Option<u32>,
    pub base_work: Option<i32>,
    pub int: Option<String>,
    pub aggression: Option<String>,
    pub kill_xp: Option<i32>,
    pub images: Option<Vec<String>>,
    pub hsl: Option<Vec<i32>>,
    pub waterwalk: Option<i32>,
    pub landwalk: Option<i32>,
    pub capacity: Option<i32>,
    pub max_residents: Option<i32>,
    pub campfire: Option<bool>,
    pub build_cost: Option<i32>,
    pub upgrade_cost: Option<i32>,
    pub level: Option<i32>,
    pub refine: Option<Vec<String>>,
    pub req: Option<Vec<ResReq>>,
    pub upgrade_req: Option<Vec<ResReq>>,
    pub upgrade_to: Option<Vec<String>>,
    pub profession: Option<String>,
    pub upkeep: Option<Vec<ResReq>>,
    pub activity: Option<String>,
    pub workspaces: Option<i32>,
}

/*#[derive(Debug, Resource, Deref, DerefMut)]
pub struct ItemTemplates(Vec<ItemTemplate>);*/

#[derive(Debug, Reflect, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemAttr {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Reflect, Clone, PartialEq, Serialize, Deserialize)]

pub struct ItemTemplate {
    pub name: String,
    pub class: String,
    pub subclass: String,
    pub image: String,
    pub weight: f32,
    pub durability: Option<i32>,
    pub refine_skill: Option<String>,
    pub refine_skill_req: Option<i32>,
    pub refine_time: Option<i32>,
    pub produces: Option<Vec<String>>,
    pub slot: Option<String>,
    pub duration: Option<i32>,
    pub attrs: Option<Vec<ItemAttr>>,
}

impl ItemTemplate {
    pub fn convert_attrs(&self) -> HashMap<AttrKey, AttrVal> {
        let mut converted_attrs = HashMap::new();

        if let Some(attrs) = &self.attrs {
            for attr in attrs.iter() {
                converted_attrs.insert(
                    AttrKey::str_to_key(attr.name.clone()),
                    AttrVal::Num(attr.value.parse::<f32>().unwrap()),
                );
            }
        }

        return converted_attrs;
    }

    pub fn get_refine_time(&self) -> i32 {
        if let Some(refine_time) = self.refine_time {
            return refine_time * TICKS_PER_SEC;
        } else {
            return BASE_REFINE_TIME;
        }
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct ResTemplates(HashMap<String, ResTemplate>);

#[derive(Debug, Resource, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResTemplate {
    pub name: String,
    #[serde(rename = "type")]
    pub res_type: String,
    pub image: String,
    pub terrain: Vec<String>,
    pub yield_rate: Vec<i32>,
    pub yield_mod: Vec<f32>,
    pub quantity_rate: Vec<i32>,
    pub quantity: Vec<i32>,
    pub skill_req: i32,
    pub level: i32,
    pub quality_rate: Option<Vec<i32>>,
    pub properties: Option<Vec<String>>,
    pub num_properties: Option<i32>,
    pub produces: Option<Vec<String>>,
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct ResPropertyTemplates(HashMap<String, ResPropertyTemplate>);

#[derive(Debug, Resource, Clone, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct ResPropertyTemplate {
    pub name: String,
    pub ranges: Vec<Vec<i32>>,
    pub tag: Vec<String>,
}

impl ResPropertyTemplates {
    pub fn load(&mut self, res_property_templates: Vec<ResPropertyTemplate>) {
        for res_property_template in res_property_templates.iter() {
            debug!("{:?}", res_property_template);
            self.insert(
                res_property_template.name.clone(),
                res_property_template.clone(),
            );
        }
    }

    pub fn get(&self, name: String) -> Vec<ResPropertyTemplate> {
        let mut res_properties = HashSet::new();

        // First try to find by the name value
        for (template_name, res_property_template) in self.iter() {
            if name == *template_name {
                res_properties.insert(res_property_template.clone());
            }

            for tag in res_property_template.tag.iter() {
                if name == *tag {
                    res_properties.insert(res_property_template.clone());
                }
            }
        }

        return res_properties.into_iter().collect();
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct SkillTemplates(HashMap<String, SkillTemplate>);

impl SkillTemplates {
    #[cfg(test)]
    pub fn from_map(skills: HashMap<String, SkillTemplate>) -> Self {
        Self(skills)
    }
}

#[derive(Debug, Resource, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillTemplate {
    pub name: String,
    pub class: String,
    pub xp: Vec<i32>,
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct RecipeTemplates(Vec<RecipeTemplate>);

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct RecipeTemplate {
    pub name: String,
    pub image: Option<String>,
    pub class: Option<String>,
    pub subclass: Option<String>,
    pub weight: Option<f32>,
    pub durability: Option<i32>,
    pub attrs: Option<Vec<ItemAttr>>,
    pub tier: Option<i32>,
    pub slot: Option<String>,
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

impl RecipeTemplate {
    pub fn get_by_structure(structure: String, templates: &Templates) -> Vec<RecipeTemplate> {
        let mut recipe_templates = Vec::new();

        for recipe_template in templates.recipe_templates.iter() {
            if let Some(structure_req) = &recipe_template.structure_req {
                if structure_req.iter().any(|requirement| {
                    structure_supports_recipe_requirement(&structure, requirement)
                }) {
                    recipe_templates.push(recipe_template.clone());
                }
            }
        }

        return recipe_templates;
    }

    pub fn get_by_name(name: String, templates: &Templates) -> Option<RecipeTemplate> {
        for recipe_template in templates.recipe_templates.iter() {
            if name == recipe_template.name {
                return Some(recipe_template.clone());
            }
        }

        return None;
    }
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct EffectTemplate {
    pub name: String,
    pub duration: i32,
    pub max_hp: Option<f32>,
    pub healing: Option<f32>,
    pub damage: Option<f32>,
    #[serde(alias = "dot")]
    pub damage_over_time: Option<f32>,
    pub speed: Option<f32>,
    pub attack_speed: Option<f32>,
    pub defense: Option<f32>,
    pub stackable: Option<bool>,
    pub armor: Option<f32>,
    pub lifeleech: Option<f32>,
    pub viewshed: Option<i32>,
    pub ignore_all_armor: Option<bool>,
    pub instant_kill_chance: Option<f32>,
    pub next_attack: Option<bool>,
    pub vision: Option<f32>,
    pub health: Option<f32>,
    pub stamina: Option<f32>,
}

type EffectName = String;

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct EffectTemplates(HashMap<EffectName, EffectTemplate>);

impl EffectTemplates {
    pub fn load(&mut self, effect_templates: Vec<EffectTemplate>) {
        for effect_template in effect_templates.iter() {
            self.insert(effect_template.name.clone(), effect_template.clone());
        }
    }
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct ComboTemplate {
    pub name: String,
    pub attacks: Vec<String>,
    pub effects: Vec<String>,
    pub quick_damage: f32,
    pub precise_damage: f32,
    pub fierce_damage: f32,
}

type ComboName = String;

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct ComboTemplates(HashMap<ComboName, ComboTemplate>);

impl ComboTemplates {
    pub fn load(&mut self, combo_templates: Vec<ComboTemplate>) {
        for combo_template in combo_templates.iter() {
            self.insert(combo_template.name.clone(), combo_template.clone());
        }
    }
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct TerrainFeatureTemplate {
    pub name: String,
    pub image: String,
    pub description: String,
    pub bonus: String,
    pub terrain: Vec<String>,
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct TerrainFeatureTemplates(HashMap<String, TerrainFeatureTemplate>);

impl TerrainFeatureTemplates {
    pub fn load(&mut self, terrain_feature_templates: Vec<TerrainFeatureTemplate>) {
        for terrain_feature_template in terrain_feature_templates.iter() {
            self.insert(
                terrain_feature_template.name.clone(),
                terrain_feature_template.clone(),
            );
        }
    }
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct DialogueTemplate {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct DialogueTemplates(HashMap<String, DialogueTemplate>);

impl DialogueTemplates {
    pub fn load(&mut self, dialogue_templates: Vec<DialogueTemplate>) {
        for dialogue_template in dialogue_templates.iter() {
            self.insert(dialogue_template.name.clone(), dialogue_template.clone());
        }
    }
}

#[derive(Debug, Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct PriceTemplate {
    pub name: String,
    pub buy_price: i32,
    pub buy_quantity: i32,
    pub sell_price: i32,
    pub sell_quantity: i32,
    pub impact_factor: f32,
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct PriceTemplates(pub HashMap<String, PriceTemplate>);

impl PriceTemplates {
    pub fn load(&mut self, prices_templates: Vec<PriceTemplate>) {
        for price_template in prices_templates.iter() {
            self.insert(price_template.name.clone(), price_template.clone());
        }
    }
}

/// The systems that make structures tick.
pub struct TemplatesPlugin;

impl Plugin for TemplatesPlugin {
    fn build(&self, app: &mut App) {
        // Load skill template data
        let obj_template_file =
            fs::File::open("templates/obj_template.yaml").expect("Could not open file.");
        let obj_templates: Vec<ObjTemplate> =
            serde_yaml::from_reader(obj_template_file).expect("Could not read values.");

        // Load item template data
        let item_template_file =
            fs::File::open("templates/item_template.yaml").expect("Could not open file.");
        let item_templates: Vec<ItemTemplate> =
            serde_yaml::from_reader(item_template_file).expect("Could not read values.");

        // Load res template data
        let res_template_file =
            fs::File::open("templates/res_template.yaml").expect("Could not open file.");
        let res_templates_vec: Vec<ResTemplate> =
            serde_yaml::from_reader(res_template_file).expect("Could not read values.");

        // Convert vector to hashmap for faster access of individual skill
        let res_templates: HashMap<_, _> = res_templates_vec
            .iter()
            .map(|x| (x.name.clone(), x.clone()))
            .collect();

        // Load skill template data
        let skill_template_file =
            fs::File::open("templates/skill_xp_template.yaml").expect("Could not open file.");
        let skill_templates_vec: Vec<SkillTemplate> =
            serde_yaml::from_reader(skill_template_file).expect("Could not read values.");

        // Convert vector to hashmap for faster access of individual skill
        let skill_templates: HashMap<_, _> = skill_templates_vec
            .iter()
            .map(|x| (x.name.clone(), x.clone()))
            .collect();

        // Load skill template data
        let recipe_template_file =
            fs::File::open("templates/recipe_template.yaml").expect("Could not open file.");
        let recipe_templates: Vec<RecipeTemplate> =
            serde_yaml::from_reader(recipe_template_file).expect("Could not read values.");

        // Load effect template data
        let effect_template_file =
            fs::File::open("templates/effect_template.yaml").expect("Could not open file.");

        let effect_template_list: Vec<EffectTemplate> =
            serde_yaml::from_reader(effect_template_file).expect("Could not read values.");

        let mut effect_templates = EffectTemplates(HashMap::new());
        effect_templates.load(effect_template_list);

        // Load combo template data
        let combo_template_file =
            fs::File::open("templates/combo_template.yaml").expect("Could not open file.");

        let combo_template_list: Vec<ComboTemplate> =
            serde_yaml::from_reader(combo_template_file).expect("Could not read values.");

        let mut comobo_templates = ComboTemplates(HashMap::new());
        comobo_templates.load(combo_template_list);

        // Load properties template data
        let res_property_template_file =
            fs::File::open("templates/res_property_template.yaml").expect("Could not open file.");

        let res_property_template_list: Vec<ResPropertyTemplate> =
            serde_yaml::from_reader(res_property_template_file).expect("Could not read values.");

        let mut res_property_templates = ResPropertyTemplates(HashMap::new());
        res_property_templates.load(res_property_template_list);

        // Load terrain features template data
        let terrain_feature_template_file =
            fs::File::open("templates/terrain_feature_template.yaml")
                .expect("Could not open file.");

        let terrain_feature_template_list: Vec<TerrainFeatureTemplate> =
            serde_yaml::from_reader(terrain_feature_template_file).expect("Could not read values.");

        let mut terrain_feature_templates = TerrainFeatureTemplates(HashMap::new());
        terrain_feature_templates.load(terrain_feature_template_list);

        let dialogue_template_file =
            fs::File::open("templates/dialogue_template.yaml").expect("Could not open file.");
        let dialogue_template_list: Vec<DialogueTemplate> =
            serde_yaml::from_reader(dialogue_template_file).expect("Could not read values.");
        let mut dialogue_templates = DialogueTemplates(HashMap::new());
        dialogue_templates.load(dialogue_template_list);

        let price_template_file =
            fs::File::open("templates/price_template.yaml").expect("Could not open file.");
        let price_template_list: Vec<PriceTemplate> =
            serde_yaml::from_reader(price_template_file).expect("Could not read values.");
        let mut price_templates = PriceTemplates(HashMap::new());
        price_templates.load(price_template_list);

        let templates = Templates {
            item_templates: item_templates,
            res_templates: ResTemplates(res_templates),
            skill_templates: SkillTemplates(skill_templates),
            obj_templates: ObjTemplates(obj_templates),
            recipe_templates: RecipeTemplates(recipe_templates),
            effect_templates: effect_templates,
            combo_templates: comobo_templates,
            res_property_templates: res_property_templates,
            terrain_feature_templates: terrain_feature_templates,
            dialogue_templates: dialogue_templates,
            price_templates: price_templates,
        };

        if let Err(errors) = templates.validate_production_catalog() {
            panic!(
                "Invalid production template catalog:\n{}",
                errors
                    .iter()
                    .map(|error| format!("- {error}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }

        // Code gen for skills enum
        /*let skills_file =
            fs::File::open("templates/skills.yaml").expect("Could not open file.");
        let skills_list: Vec<String> = serde_yaml::from_reader(skills_file).expect("Could not read values.");

        let variants: Vec<String> = skills_list
            .iter()
            .map(|name| format!("    {},", name))
            .collect();

        let enum_code = format!(
            "use bevy::prelude::*;\n\
             #[derive(Debug, Reflect, Clone, Hash, PartialEq, Eq)]\n\
             pub enum SkillDef {{\n{}\n}}",
            variants.join("\n")
        );

        fs::write("src/skill/skill_defs.rs", enum_code).unwrap();*/

        app.insert_resource(templates);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::Ids;
    use crate::item::{Inventory, LOG, TIMBER};
    use crate::recipe::{RecipePlugin, Recipes};

    #[test]
    fn production_catalog_references_are_valid() {
        let mut app = App::new();
        app.add_plugins(TemplatesPlugin);

        let templates = app.world().resource::<Templates>();
        assert_eq!(templates.validate_production_catalog(), Ok(()));
    }

    #[test]
    fn structure_wood_requirements_are_explicitly_flexible_or_strict() {
        let mut app = App::new();
        app.add_plugins(TemplatesPlugin);

        let templates = app.world().resource::<Templates>();
        let structure = |name: &str| {
            templates
                .obj_templates
                .iter()
                .find(|template| template.template == name)
                .unwrap_or_else(|| panic!("missing object template {name}"))
        };
        let wood_requirements = |template: &ObjTemplate| {
            template
                .req
                .iter()
                .flatten()
                .chain(template.upgrade_req.iter().flatten())
                .chain(template.upkeep.iter().flatten())
                .filter(|requirement| {
                    matches!(requirement.req_type.as_str(), LOG | TIMBER | LOGS_OR_TIMBER)
                })
                .map(|requirement| (requirement.req_type.clone(), requirement.quantity))
                .collect::<Vec<_>>()
        };

        for name in [
            "Crafting Tent",
            "Blacksmith",
            "Workshop",
            "Mason",
            "Butchery",
            "Smokehouse",
            "Tannery",
            "Millhouse",
            "Textile Mill",
            "Bakery",
            "Tailor",
            "Herbalist",
            "Alchemist",
            "Tavern",
            "Mine",
            "Lumbercamp",
            "Quarry",
            "Trapper",
            "Farm",
            "Shelter Tent",
            "Large Tent",
            "Yurt",
            "Large Yurt",
            "Burrow",
            "Cache",
            "Warehouse",
            "Watchtower",
        ] {
            let requirements = wood_requirements(structure(name));
            assert!(!requirements.is_empty(), "{name} should require wood");
            assert!(
                requirements
                    .iter()
                    .all(|(requirement, _)| requirement == LOGS_OR_TIMBER),
                "{name} should display and accept Logs or Timber: {requirements:?}"
            );
        }

        assert_eq!(
            wood_requirements(structure("Stockade")),
            vec![
                (LOG.to_string(), 15),
                (TIMBER.to_string(), 5),
                (LOG.to_string(), 1)
            ]
        );
        assert_eq!(
            wood_requirements(structure("Palisade")),
            vec![
                (TIMBER.to_string(), 5),
                (TIMBER.to_string(), 3),
                (TIMBER.to_string(), 1)
            ]
        );
        assert_eq!(
            wood_requirements(structure("Fieldstone Walls")),
            vec![(TIMBER.to_string(), 3)]
        );
    }

    #[test]
    fn early_survival_recipes_are_split_between_handcraft_and_crafting_tent() {
        let mut app = App::new();
        app.add_plugins(TemplatesPlugin);

        let templates = app.world().resource::<Templates>();
        let recipe = |name: &str| {
            templates
                .recipe_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing recipe template {name}"))
        };

        for name in [
            "Firewood",
            "Sharpened Stick",
            "Crude Torch",
            "Crude Bandage",
            "Twine",
            "Flint Hatchet",
        ] {
            assert_eq!(
                recipe(name).structure_req,
                None,
                "{name} should remain available through Handcraft"
            );
        }

        for name in [
            "Fishing Rod",
            "Improvised Sling",
            "Stone Knife",
            "Bone Dagger",
            "Stone-Tipped Spear",
            "Bone War Club",
            "Throwing Spear",
        ] {
            assert_eq!(
                recipe(name)
                    .structure_req
                    .as_ref()
                    .map(|requirements| requirements
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()),
                Some(vec!["Crafting Tent"]),
                "{name} should require a Crafting Tent"
            );
        }

        assert_eq!(
            recipe("Crude Bandage")
                .req
                .iter()
                .map(|requirement| (requirement.req_type.as_str(), requirement.quantity))
                .collect::<Vec<_>>(),
            vec![("Plant Fibers", 2)],
            "Crude Bandages should be renewable from gathered fiber"
        );

        let herbal_poultice = recipe("Herbal Poultice");
        assert_eq!(
            herbal_poultice
                .attrs
                .as_ref()
                .and_then(|attrs| attrs.iter().find(|attr| attr.name == "Healing"))
                .map(|attr| attr.value.as_str()),
            Some("20")
        );
        assert_eq!(
            templates
                .item_templates
                .iter()
                .find(|item| item.name == "Herbal Poultice")
                .and_then(|item| item.attrs.as_ref())
                .and_then(|attrs| attrs.iter().find(|attr| attr.name == "Healing"))
                .map(|attr| attr.value.as_str()),
            Some("20")
        );
    }

    #[test]
    fn hunting_resources_are_ground_tiers_with_random_carcass_pools() {
        let mut app = App::new();
        app.add_plugins(TemplatesPlugin);

        let templates = app.world().resource::<Templates>();
        let expected = [
            (
                "Sparse Hunting Grounds",
                "fruitfulhuntinggroundsclearing",
                3,
            ),
            (
                "Fruitful Hunting Grounds",
                "fruitfulhuntinggroundswoodland",
                3,
            ),
            (
                "Bountiful Hunting Grounds",
                "fruitfulhuntinggroundswater",
                4,
            ),
        ];
        let hunting_resources = templates
            .res_templates
            .values()
            .filter(|template| template.res_type == crate::constants::GAME_ANIMAL)
            .collect::<Vec<_>>();

        assert_eq!(hunting_resources.len(), expected.len());
        for (name, image, output_count) in expected {
            let template = templates
                .res_templates
                .get(name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(template.image, image);
            assert_eq!(template.produces.as_ref().map(Vec::len), Some(output_count));
            assert!(
                template.properties.as_ref().is_none_or(Vec::is_empty),
                "{name} must not expose animal properties on the hunting ground"
            );
        }

        for old_species_resource in [
            "Windstride Stag",
            "Bristleback Boar",
            "Swiftstep Hare",
            "Frostmane Elk",
            "Dunehorn Antelope",
            "Stonegrove Ibex",
        ] {
            assert!(!templates.res_templates.contains_key(old_species_resource));
        }
    }

    #[test]
    fn every_harvesting_tool_template_has_durability() {
        let mut app = App::new();
        app.add_plugins(TemplatesPlugin);

        let templates = app.world().resource::<Templates>();
        let gather_attrs = [
            "Mining",
            "Logging",
            "Stonecutting",
            "Fishing",
            "Farming",
            "Foraging",
            "Hunting",
        ];
        let missing = templates
            .item_templates
            .iter()
            .filter(|template| {
                template.attrs.as_ref().is_some_and(|attrs| {
                    attrs
                        .iter()
                        .any(|attr| gather_attrs.contains(&attr.name.as_str()))
                })
            })
            .filter(|template| template.durability.is_none())
            .map(|template| template.name.clone())
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "harvesting tools without durability: {missing:?}"
        );
    }

    #[test]
    fn legacy_small_tent_template_name_resolves_to_shelter_tent() {
        let mut app = App::new();
        app.add_plugins(TemplatesPlugin);

        let templates = app.world().resource::<Templates>();
        let template = templates.obj_templates.get("Small Tent".to_string());

        assert_eq!(template.template, "Shelter Tent");
    }

    fn production_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((TemplatesPlugin, RecipePlugin));
        app.world_mut()
            .resource_scope(|world, templates: Mut<Templates>| {
                let recipe_templates = templates.recipe_templates.to_vec();
                let mut recipes = world.resource_mut::<Recipes>();
                recipes.set_templates(recipe_templates);
            });
        app
    }

    #[test]
    fn copper_and_wood_chain_reaches_a_felling_axe() {
        let mut app = production_test_app();

        app.world_mut()
            .resource_scope(|world, templates: Mut<Templates>| {
                let recipe = {
                    let mut recipes = world.resource_mut::<Recipes>();
                    assert!(recipes.create(1, "Copper Felling Axe".to_string(), &templates));
                    recipes
                        .get_for_owner_by_name(1, "Copper Felling Axe")
                        .expect("created recipe")
                };

                let mut inventory = Inventory {
                    owner: 1,
                    items: Vec::new(),
                };
                let mut ids = Ids::default();
                let log = inventory.new(
                    ids.new_item_id(),
                    "Cragroot Maple Log".to_string(),
                    1,
                    &templates.item_templates,
                );
                let ore = inventory.new(
                    ids.new_item_id(),
                    "Valleyrun Copper Ore".to_string(),
                    1,
                    &templates.item_templates,
                );

                inventory
                    .try_refine(log.id, 1, 100, &templates.item_templates, &mut ids)
                    .expect("log refinement");
                inventory
                    .try_refine(ore.id, 1, 100, &templates.item_templates, &mut ids)
                    .expect("ore refinement");
                inventory
                    .try_craft(
                        ids.new_item_id(),
                        1,
                        "Copper Felling Axe".to_string(),
                        &recipe,
                        None,
                        None,
                        100,
                    )
                    .expect("felling axe craft");

                let felling_axe = inventory
                    .items
                    .iter()
                    .find(|item| item.name == "Copper Felling Axe")
                    .expect("crafted felling axe");
                assert!(matches!(
                    felling_axe.attrs.get(&crate::item::AttrKey::Damage),
                    Some(crate::item::AttrVal::Num(value)) if *value == 6.0
                ));
                assert!(matches!(
                    felling_axe.attrs.get(&crate::item::AttrKey::Logging),
                    Some(crate::item::AttrVal::Num(value)) if *value == 3.0
                ));
                assert_eq!(felling_axe.class, crate::constants::TOOL);
                assert_eq!(felling_axe.durability, Some(75));
                assert!(!inventory
                    .items
                    .iter()
                    .any(|item| item.name == "Valleyrun Copper Ingot"));
                assert!(!inventory
                    .items
                    .iter()
                    .any(|item| item.name == "Cragroot Maple Timber"));
            });
    }

    #[test]
    fn combat_axes_and_logging_tools_have_separate_template_progressions() {
        let app = production_test_app();
        let templates = app.world().resource::<Templates>();

        for name in [
            "Copper Training Axe",
            "Copper Broad Axe",
            "Copper Heavy Axe",
            "Iron War Axe",
            "Mithril War Axe",
        ] {
            let template = templates
                .item_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(template.class, crate::constants::WEAPON);
            assert!(!template
                .convert_attrs()
                .contains_key(&crate::item::AttrKey::Logging));
        }

        for (name, rating, durability) in [
            ("Crude Hatchet", 1.0, 30),
            ("Flint Hatchet", 2.0, 45),
            ("Copper Felling Axe", 3.0, 75),
            ("Iron Felling Axe", 4.0, 120),
            ("Mithril Felling Axe", 4.0, 180),
        ] {
            let template = templates
                .item_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert!(matches!(
                template.convert_attrs().get(&crate::item::AttrKey::Logging),
                Some(crate::item::AttrVal::Num(value)) if *value == rating
            ));
            assert_eq!(template.durability, Some(durability));
        }

        let crude_hatchet = templates
            .item_templates
            .iter()
            .find(|template| template.name == "Crude Hatchet")
            .expect("missing Crude Hatchet");
        assert_eq!(crude_hatchet.class, crate::constants::WEAPON);
        assert_eq!(crude_hatchet.subclass, "Axe");

        for name in [
            "Flint Hatchet",
            "Copper Felling Axe",
            "Iron Felling Axe",
            "Mithril Felling Axe",
        ] {
            let template = templates
                .item_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(template.class, crate::constants::TOOL);
        }
    }

    #[test]
    fn mining_tools_have_a_dedicated_material_progression() {
        let app = production_test_app();
        let templates = app.world().resource::<Templates>();

        for (name, image, rating, durability, tier) in [
            ("Training Pick Axe", "pickaxe", 2.0, 60, 0),
            ("Copper Pick Axe", "copperpickaxe", 3.0, 90, 1),
            ("Iron Pick Axe", "ironpickaxe", 4.0, 140, 2),
            ("Mithril Pick Axe", "mithrilpickaxe", 4.0, 210, 3),
        ] {
            let template = templates
                .item_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(template.class, crate::constants::TOOL);
            assert_eq!(template.subclass, "Pick Axe");
            assert_eq!(template.image, image);
            assert_eq!(template.durability, Some(durability));
            assert!(matches!(
                template.convert_attrs().get(&crate::item::AttrKey::Mining),
                Some(crate::item::AttrVal::Num(value)) if *value == rating
            ));

            let recipe = RecipeTemplate::get_by_name(name.to_string(), templates)
                .unwrap_or_else(|| panic!("missing recipe {name}"));
            assert_eq!(recipe.class.as_deref(), Some(crate::constants::TOOL));
            assert_eq!(recipe.subclass.as_deref(), Some("Pick Axe"));
            assert_eq!(recipe.tier, Some(tier));
        }
    }

    #[test]
    fn stonecutting_tools_have_a_dedicated_material_progression() {
        let app = production_test_app();
        let templates = app.world().resource::<Templates>();

        for (name, image, rating, durability, tier) in [
            (
                "Training Stonecutter Hammer",
                "trainingstonecutterhammer",
                2.0,
                60,
                0,
            ),
            (
                "Copper Stonecutter Hammer",
                "copperstonecutterhammer",
                3.0,
                90,
                1,
            ),
            (
                "Iron Stonecutter Hammer",
                "ironstonecutterhammer",
                4.0,
                140,
                2,
            ),
            (
                "Mithril Stonecutter Hammer",
                "mithrilstonecutterhammer",
                4.0,
                210,
                3,
            ),
        ] {
            let template = templates
                .item_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(template.class, crate::constants::TOOL);
            assert_eq!(template.subclass, "Stonecutter Hammer");
            assert_eq!(template.image, image);
            assert_eq!(template.durability, Some(durability));
            assert!(matches!(
                template
                    .convert_attrs()
                    .get(&crate::item::AttrKey::Stonecutting),
                Some(crate::item::AttrVal::Num(value)) if *value == rating
            ));

            let recipe = RecipeTemplate::get_by_name(name.to_string(), templates)
                .unwrap_or_else(|| panic!("missing recipe {name}"));
            assert_eq!(recipe.class.as_deref(), Some(crate::constants::TOOL));
            assert_eq!(recipe.subclass.as_deref(), Some("Stonecutter Hammer"));
            assert_eq!(recipe.tier, Some(tier));
        }
    }

    #[test]
    fn fishing_rods_have_a_dedicated_material_progression() {
        let app = production_test_app();
        let templates = app.world().resource::<Templates>();

        for (name, image, rating, durability, tier) in [
            ("Fishing Rod", "fishingrod", 1.0, 40, 0),
            ("Copper Fishing Rod", "copperfishingrod", 2.0, 75, 1),
            ("Iron Fishing Rod", "ironfishingrod", 3.0, 120, 2),
            ("Mithril Fishing Rod", "mithrilfishingrod", 4.0, 180, 3),
        ] {
            let template = templates
                .item_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(template.class, crate::constants::TOOL);
            assert_eq!(template.subclass, crate::constants::FISHING_ROD);
            assert_eq!(template.image, image);
            assert_eq!(template.durability, Some(durability));
            assert!(matches!(
                template.convert_attrs().get(&crate::item::AttrKey::Fishing),
                Some(crate::item::AttrVal::Num(value)) if *value == rating
            ));

            let recipe = RecipeTemplate::get_by_name(name.to_string(), templates)
                .unwrap_or_else(|| panic!("missing recipe {name}"));
            assert_eq!(recipe.class.as_deref(), Some(crate::constants::TOOL));
            assert_eq!(
                recipe.subclass.as_deref(),
                Some(crate::constants::FISHING_ROD)
            );
            assert_eq!(recipe.tier, Some(tier));
        }
    }

    #[test]
    fn farming_sickles_have_a_dedicated_material_progression() {
        let app = production_test_app();
        let templates = app.world().resource::<Templates>();

        for (name, image, rating, durability, tier) in [
            ("Sickle", "sickle", 2.0, 60, 0),
            ("Copper Sickle", "coppersickle", 3.0, 90, 1),
            ("Iron Sickle", "ironsickle", 4.0, 140, 2),
            ("Mithril Sickle", "mithrilsickle", 4.0, 210, 3),
        ] {
            let template = templates
                .item_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(template.class, crate::constants::TOOL);
            assert_eq!(template.subclass, "Sickle");
            assert_eq!(template.image, image);
            assert_eq!(template.durability, Some(durability));
            assert!(matches!(
                template.convert_attrs().get(&crate::item::AttrKey::Farming),
                Some(crate::item::AttrVal::Num(value)) if *value == rating
            ));

            let recipe = RecipeTemplate::get_by_name(name.to_string(), templates)
                .unwrap_or_else(|| panic!("missing recipe {name}"));
            assert_eq!(recipe.class.as_deref(), Some(crate::constants::TOOL));
            assert_eq!(recipe.subclass.as_deref(), Some("Sickle"));
            assert_eq!(recipe.tier, Some(tier));
        }
    }

    #[test]
    fn spears_and_bows_form_the_hunting_tool_progression() {
        let app = production_test_app();
        let templates = app.world().resource::<Templates>();

        for name in ["Improvised Sling", "Stone Knife", "Bone Dagger"] {
            let template = templates
                .item_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(template.class, crate::constants::WEAPON);
            assert!(!template
                .convert_attrs()
                .contains_key(&crate::item::AttrKey::Hunting));
        }

        for (name, subclass, rating, durability) in [
            ("Sharpened Stick", "Spear", 1.0, 25),
            ("Stone-Tipped Spear", "Spear", 2.0, 45),
            ("Throwing Spear", "Throwing", 2.0, 40),
            ("Copper Spear", "Spear", 3.0, 75),
            ("Iron Spear", "Spear", 4.0, 120),
            ("Mithril Glaive", "Spear", 4.0, 180),
            ("Training Bow", "Bow", 2.0, 60),
            ("Hunting Bow", "Bow", 3.0, 70),
            ("Iron-Limbed Longbow", "Bow", 4.0, 90),
            ("Mithril Warbow", "Bow", 4.0, 120),
        ] {
            let template = templates
                .item_templates
                .iter()
                .find(|template| template.name == name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(template.class, crate::constants::WEAPON);
            assert_eq!(template.subclass, subclass);
            assert_eq!(template.durability, Some(durability));
            assert!(matches!(
                template.convert_attrs().get(&crate::item::AttrKey::Hunting),
                Some(crate::item::AttrVal::Num(value)) if *value == rating
            ));
        }
    }

    #[test]
    fn early_equipment_stations_connect_tanning_and_specialization() {
        let mut app = App::new();
        app.add_plugins(TemplatesPlugin);

        let templates = app.world().resource::<Templates>();
        let crafting_tent = templates.obj_templates.get("Crafting Tent".to_string());
        let tannery = templates.obj_templates.get("Tannery".to_string());

        assert!(crafting_tent
            .refine
            .as_ref()
            .is_some_and(|types| types.iter().any(|item_type| item_type == "Hide")));
        assert!(crafting_tent
            .upgrade_to
            .as_ref()
            .is_some_and(|upgrades| upgrades.iter().any(|upgrade| upgrade == "Blacksmith")));
        assert!(crafting_tent
            .upgrade_to
            .as_ref()
            .is_some_and(|upgrades| upgrades.iter().any(|upgrade| upgrade == "Workshop")));
        assert!(crafting_tent
            .upgrade_to
            .as_ref()
            .is_some_and(|upgrades| upgrades.iter().any(|upgrade| upgrade == "Tannery")));
        assert!(tannery
            .refine
            .as_ref()
            .is_some_and(|types| types.iter().any(|item_type| item_type == "Hide")));
        assert!(tannery.upgrade_req.is_some());

        for (recipe_name, tier) in [
            ("Stone-Tipped Spear", 0),
            ("Copper Spear", 1),
            ("Iron Spear", 2),
            ("Mithril Glaive", 3),
            ("Training Bow", 0),
            ("Hunting Bow", 1),
            ("Iron-Limbed Longbow", 2),
            ("Mithril Warbow", 3),
            ("Hide Wraps", 0),
            ("Studded Leather Vest", 1),
            ("Iron Chainmail", 2),
            ("Mithril Plate", 3),
        ] {
            let recipe = RecipeTemplate::get_by_name(recipe_name.to_string(), templates)
                .unwrap_or_else(|| panic!("missing progression recipe {recipe_name}"));
            assert_eq!(recipe.tier, Some(tier), "wrong tier for {recipe_name}");
        }
    }

    #[test]
    fn cloth_and_hunted_hide_reach_primitive_armor() {
        let mut app = production_test_app();

        app.world_mut()
            .resource_scope(|world, templates: Mut<Templates>| {
                let (twine_recipe, hide_cap_recipe) = {
                    let mut recipes = world.resource_mut::<Recipes>();
                    assert!(recipes.create(1, "Twine".to_string(), &templates));
                    assert!(recipes.create(1, "Hide Cap".to_string(), &templates));
                    (
                        recipes
                            .get_for_owner_by_name(1, "Twine")
                            .expect("Twine recipe"),
                        recipes
                            .get_for_owner_by_name(1, "Hide Cap")
                            .expect("Hide Cap recipe"),
                    )
                };

                let mut inventory = Inventory {
                    owner: 1,
                    items: Vec::new(),
                };
                let mut ids = Ids::default();
                inventory.new(
                    ids.new_item_id(),
                    "Honeybell Cloth".to_string(),
                    1,
                    &templates.item_templates,
                );
                inventory.new(
                    ids.new_item_id(),
                    "Bristleback Raw Hide".to_string(),
                    1,
                    &templates.item_templates,
                );

                inventory
                    .try_craft(
                        ids.new_item_id(),
                        1,
                        "Twine".to_string(),
                        &twine_recipe,
                        None,
                        None,
                        100,
                    )
                    .expect("Twine craft");
                inventory
                    .try_craft(
                        ids.new_item_id(),
                        1,
                        "Hide Cap".to_string(),
                        &hide_cap_recipe,
                        None,
                        None,
                        100,
                    )
                    .expect("Hide Cap craft");

                let hide_cap = inventory
                    .items
                    .iter()
                    .find(|item| item.name == "Hide Cap")
                    .expect("crafted Hide Cap");
                assert!(matches!(
                    hide_cap.attrs.get(&crate::item::AttrKey::Defense),
                    Some(crate::item::AttrVal::Num(value)) if *value == 1.0
                ));
                assert!(!inventory
                    .items
                    .iter()
                    .any(|item| item.name == "Honeybell Cloth" || item.subclass == "Raw Hide"));
            });
    }

    #[test]
    fn hunted_animal_and_firewood_chain_reaches_cooked_meat() {
        let mut app = production_test_app();

        app.world_mut()
            .resource_scope(|world, templates: Mut<Templates>| {
                let recipe = {
                    let mut recipes = world.resource_mut::<Recipes>();
                    assert!(recipes.create(1, "Cooked Meat".to_string(), &templates));
                    recipes
                        .get_for_owner_by_name(1, "Cooked Meat")
                        .expect("created recipe")
                };

                let mut inventory = Inventory {
                    owner: 1,
                    items: Vec::new(),
                };
                let mut ids = Ids::default();
                let carcass = inventory.new(
                    ids.new_item_id(),
                    "Felled Bristleback Boar".to_string(),
                    1,
                    &templates.item_templates,
                );
                inventory.new(
                    ids.new_item_id(),
                    "Firewood".to_string(),
                    1,
                    &templates.item_templates,
                );

                inventory
                    .try_refine(carcass.id, 1, 100, &templates.item_templates, &mut ids)
                    .expect("carcass refinement");
                inventory
                    .try_craft(
                        ids.new_item_id(),
                        1,
                        "Bristleback Cooked Meat".to_string(),
                        &recipe,
                        None,
                        None,
                        100,
                    )
                    .expect("cooked meat craft");

                let meal = inventory
                    .items
                    .iter()
                    .find(|item| item.name == "Bristleback Cooked Meat")
                    .expect("cooked meat output");
                assert!(meal.attrs.contains_key(&crate::item::AttrKey::Feed));
                assert!(!meal
                    .attrs
                    .contains_key(&crate::item::AttrKey::FoodPoisoning));
            });
    }
}
