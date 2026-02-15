use bevy::prelude::*;

use std::collections::HashMap;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use std::fs;

use crate::constants::{BASE_REFINE_TIME, TICKS_PER_SEC};
use crate::item::AttrKey;
use crate::item::AttrVal;

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

    pub fn get_obj_template_by_name(&self, name: String) -> ObjTemplate {
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
    pub fn get_by_structure(structure: String, templates: &Res<Templates>) -> Vec<RecipeTemplate> {
        let mut recipe_templates = Vec::new();

        for recipe_template in templates.recipe_templates.iter() {
            if let Some(structure_req) = &recipe_template.structure_req {
                if structure_req.contains(&structure) {
                    recipe_templates.push(recipe_template.clone());
                }
            }
        }

        return recipe_templates;
    }

    pub fn get_by_name(name: String, templates: &Res<Templates>) -> Option<RecipeTemplate> {
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
