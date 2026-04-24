use bevy::prelude::*;
use rand::Rng;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;

use crate::network;
use crate::{
    map::Map,
    obj::Position,
    templates::{Templates, TerrainFeatureTemplate},
};

#[derive(Debug, Clone)]
pub struct TerrainFeature {
    pub name: String,
    pub image: String,
    pub pos: Position,
    pub bonus: String,
    pub reveal: bool,
}

#[derive(Resource, Deref, DerefMut, Debug)]
pub struct TerrainFeatures(HashMap<Position, TerrainFeature>);

impl TerrainFeature {
    pub fn spawn(
        terrain_features: &mut ResMut<TerrainFeatures>,
        templates: &Templates,
        map: &Res<Map>,
    ) {
        let tf_templates = &templates.terrain_feature_templates;

        let mut terrain_list: HashMap<String, Vec<TerrainFeatureTemplate>> = HashMap::new();
        let mut rng = rand::thread_rng();

        for (_name, tf_template) in tf_templates.iter() {
            for terrain in tf_template.terrain.iter() {
                match terrain_list.entry(terrain.to_string()) {
                    Vacant(entry) => {
                        let mut tf_template_list = Vec::new();
                        tf_template_list.push(tf_template.clone());
                        entry.insert(tf_template_list);
                    }
                    Occupied(entry) => {
                        entry.into_mut().push(tf_template.clone());
                    }
                };
            }
        }

        for (index, tile_info) in map.base.iter().enumerate() {
            if let Some(tf_template_list) =
                terrain_list.get(tile_info.tile_type.to_string().as_str())
            {
                for tf_template in tf_template_list.iter() {
                    if rng.gen_range(0..100) > 10 {
                        let map_pos = Map::index_to_pos(index);
                        let position = Position {
                            x: map_pos.0,
                            y: map_pos.1,
                        };

                        let tf = TerrainFeature {
                            name: tf_template.name.clone(),
                            image: tf_template.image.clone(),
                            pos: position,
                            bonus: tf_template.bonus.clone(),
                            reveal: false,
                        };

                        terrain_features.insert(position, tf);
                    }
                }
            }
        }

        //debug!("TerrainFeatures: {:?}", terrain_features);
    }

    pub fn get_by_tile(
        position: Position,
        terrain_features: &TerrainFeatures,
    ) -> Vec<network::TileTerrainFeature> {
        let mut tile_terrain_features = Vec::new();

        if let Some(terrain_features_on_tile) = terrain_features.get(&position) {
            let tile_resource = network::TileTerrainFeature {
                name: terrain_features_on_tile.name.clone(),
                image: terrain_features_on_tile.image.clone(),
                bonus: terrain_features_on_tile.bonus.clone(),
            };

            tile_terrain_features.push(tile_resource);
        }

        return tile_terrain_features;
    }
}

pub struct TerrainFeaturePlugin;

impl Plugin for TerrainFeaturePlugin {
    fn build(&self, app: &mut App) {
        let terrain_features = TerrainFeatures(HashMap::new());

        app.insert_resource(terrain_features);
    }
}
