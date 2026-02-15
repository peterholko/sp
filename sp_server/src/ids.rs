use bevy::prelude::*;

use std::collections::HashMap;

// Indexes for IDs
#[derive(Resource, Clone, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct Ids {
    pub map_event: i32,
    pub player_event: i32,
    pub obj: i32,
    pub item: i32,
    pub player_hero_map: HashMap<i32, i32>,
    pub obj_player_map: HashMap<i32, i32>,
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct EntityObjMap(pub HashMap<i32, Entity>);

impl Ids {
    pub fn new_map_event_id(&mut self) -> i32 {
        self.map_event = self.map_event + 1;
        self.map_event
    }

    pub fn new_obj_id(&mut self) -> i32 {
        self.obj = self.obj + 1;
        self.obj
    }


    pub fn new_item_id(&mut self) -> i32 {
        self.item = self.item + 1;
        self.item
    }

    pub fn get_hero(&self, player_id: i32) -> Option<i32> {
        if let Some(hero_id) = self.player_hero_map.get(&player_id) {
            return Some(*hero_id);
        }

        return None;
    }

    pub fn is_hero(&self, obj_id: i32) -> bool {
        if let Some(player_id) = self.obj_player_map.get(&obj_id) {
            return self.player_hero_map.contains_key(player_id);
        }

        return false;
    }

    /*pub fn get_entity(&self, obj_id: i32) -> Option<Entity> {
        if let Some(entity) = self.obj_entity_map.get(&obj_id) {
            return Some(*entity);
        }

        return None;
    }*/

    pub fn get_player(&self, obj_id: i32) -> Option<i32> {
        if let Some(player) = self.obj_player_map.get(&obj_id) {
            return Some(*player);
        }

        return None;
    }

    pub fn get_all_obj_ids(&self, player_id: i32) -> Vec<i32> {
        
        let mut obj_ids = Vec::new();
        for (obj_id, player) in self.obj_player_map.iter() {
            if *player == player_id {
                obj_ids.push(*obj_id);
            }
        }

        obj_ids
    }

    pub fn new_obj(&mut self, obj_id: i32, player_id: i32) {
        self.obj_player_map.insert(obj_id, player_id);
    }

    pub fn remove_obj(&mut self, obj_id: i32) {
        self.obj_player_map.remove(&obj_id);
    }

    pub fn change_obj_player_id(&mut self, obj_id: i32, new_player_id: i32) {
        if self.obj_player_map.contains_key(&obj_id) {
            self.obj_player_map.remove(&obj_id);
            self.obj_player_map.insert(obj_id, new_player_id);
        } else {
            error!("Cannot find obj_id: {:?} in obj_player_map", obj_id);
        }
    }

    pub fn new_hero(&mut self, hero_id: i32, player_id: i32) {
        self.player_hero_map.insert(player_id, hero_id);
        self.new_obj(hero_id, player_id);
    }

    pub fn remove_hero(&mut self, player_id: i32, hero_id: i32) {
        self.player_hero_map.remove(&player_id);
        self.remove_obj(hero_id);
    }
}

impl EntityObjMap {
    pub fn get_obj_id(&self, entity: Entity) -> Option<i32> {
        for (obj_id, e) in &self.0 {
            if *e == entity {
                return Some(*obj_id);
            }
        }

        return None;
    }

    pub fn get_entity(&self, obj_id: i32) -> Option<Entity> {
        if let Some(entity) = self.0.get(&obj_id) {
            return Some(*entity);
        }

        return None;
    }

    pub fn get_obj_by_entity(&self, entity: Entity) -> Option<i32> {
        for (obj_id, e) in &self.0 {
            if *e == entity {
                return Some(*obj_id);
            }
        }

        return None;
    }

    pub fn new_obj(&mut self, obj_id: i32, entity: Entity) {
        self.0.insert(obj_id, entity);
    }

    pub fn remove_obj(&mut self, obj_id: i32) {
        self.0.remove(&obj_id);
    }
}
