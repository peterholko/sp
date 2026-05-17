use bevy::{math, prelude::*};
use rand::Rng;

use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use tiled::Loader;
//use tiled::{parse, LayerData};

use pathfinding::prelude::astar;

use crate::constants::SANCTUARY_RANGE;
use crate::obj::{Blocker, Position};
use crate::player;
use crate::world::Weather;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        let map = Map::load_map();
        app.insert_resource(map);
    }
}

pub const WIDTH: i32 = 60;
pub const HEIGHT: i32 = 50;

#[derive(Debug, Reflect, Hash, PartialEq, Eq, Clone, Copy)]
pub enum TileType {
    Grasslands,
    Snow,
    River,
    Ocean,
    Plains,
    HillsPlains,
    Desert,
    Oasis,
    HillsDesert,
    HillsGrasslands,
    Swamp,
    HillsSnow,
    DeciduousForest,
    Rainforest,
    Jungle,
    Savanna,
    FrozenForest,
    PineForest,
    PalmForest,
    Mountain,
    Volcano,
    Unknown,
}

impl TileType {
    pub fn is_forest(self) -> bool {
        matches!(
            self,
            TileType::DeciduousForest
                | TileType::Rainforest
                | TileType::Jungle
                | TileType::FrozenForest
                | TileType::PineForest
                | TileType::PalmForest
        )
    }
}

impl fmt::Display for TileType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TileType::Grasslands => write!(f, "Grasslands"),
            TileType::Snow => write!(f, "Snow"),
            TileType::River => write!(f, "River"),
            TileType::Ocean => write!(f, "Ocean"),
            TileType::Plains => write!(f, "Plains"),
            TileType::HillsPlains => write!(f, "Plains Hills"),
            TileType::Desert => write!(f, "Desert"),
            TileType::Oasis => write!(f, "Oasis"),
            TileType::HillsDesert => write!(f, "Desert Hills"),
            TileType::HillsGrasslands => write!(f, "Grassland Hills"),
            TileType::Swamp => write!(f, "Swamp"),
            TileType::HillsSnow => write!(f, "Snow Hills"),
            TileType::DeciduousForest => write!(f, "Deciduous Forest"),
            TileType::Rainforest => write!(f, "Rain Forest"),
            TileType::Jungle => write!(f, "Jungle"),
            TileType::Savanna => write!(f, "Savanna"),
            TileType::FrozenForest => write!(f, "Frozen Forest"),
            TileType::PineForest => write!(f, "Pine Forest"),
            TileType::PalmForest => write!(f, "Palm Forest"),
            TileType::Mountain => write!(f, "Mountain"),
            TileType::Volcano => write!(f, "Volcano"),
            TileType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

#[derive(Debug, Reflect, Clone)]
pub enum TemperatureType {
    Tropical,
    Subtropical,
    WarmTemperate,
    CoolTemperate,
    Boreal,
    Subpolar,
    Polar,
    Unknown,
}

#[derive(Debug, Reflect, Clone)]
pub enum MoistureType {
    SuperHumid,
    Humid,
    SemiHumid,
    Arid,
    SuperArid,
    Unknown,
}

#[derive(Debug, Reflect, Clone)]
pub struct TileInfo {
    pub tile_type: TileType,
    pub layers: Vec<u32>,
}

#[derive(Resource, Reflect, Default, Debug, Clone)]
#[reflect(Resource)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    pub base: Vec<TileInfo>,
    pub temperature: Vec<TemperatureType>,
    pub moisture: Vec<MoistureType>,
    pub wildness: Vec<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MapTile {
    pub x: i32,
    pub y: i32,
    pub t: Vec<u32>,
}

// TODO move to another module
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MapPos(pub i32, pub i32);

impl MapPos {
    fn distance(&self, other: &MapPos) -> u32 {
        // (self.0.abs_diff(other.0) + self.1.abs_diff(other.1)) as u32
        Map::distance((self.0, self.1), (other.0, other.1))
    }

    fn successors(
        &self,
        map: &Map,
        mover_player_id: i32,
        blocking_list: &Vec<Blocker>,
        landwalk: bool,
        waterwalk: bool,
        mountainwalk: bool,
        ignore_goal_terrain_type: bool,
        allow_attackable_blockers: bool,
        goal: MapPos,
    ) -> Vec<(MapPos, u32)> {
        let &MapPos(x, y) = self;
        let s = Map::get_neighbour_tiles(
            x,
            y,
            map,
            mover_player_id,
            blocking_list,
            landwalk,
            waterwalk,
            mountainwalk,
            ignore_goal_terrain_type,
            allow_attackable_blockers,
            goal,
        );
        s
    }
}

impl Map {
    pub fn load_map() -> Map {
        let temp = Map::get_temperature(
            Season::Spring,
            16,
            TemperatureType::Tropical,
            MoistureType::SuperHumid,
            Weather::HeavyRain,
        );
        debug!("temp: {:?}", temp);

        let mut map = Map {
            width: WIDTH,
            height: HEIGHT,
            base: Vec::with_capacity(3000),
            temperature: Vec::with_capacity(3000),
            moisture: Vec::with_capacity(3000),
            wildness: Vec::with_capacity(3000),
        };

        debug!("Loading map...");
        let mut loader = Loader::new();

        let map_path =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("map/test3.tmx");
        let test_map = loader.load_tmx_map(map_path).unwrap();

        for layer in test_map.layers() {
            debug!("layer: {:?}", layer.name);
            if layer.name == "base1" {
                match layer.layer_type() {
                    tiled::LayerType::Tiles(layer) => match layer {
                        tiled::TileLayer::Finite(data) => {
                            for y in 0..HEIGHT {
                                for x in 0..WIDTH {
                                    if let Some(tile) = data.get_tile(x, y) {
                                        let tileset = tile.get_tileset();

                                        if let Some(tileset_tile) = tileset.get_tile(tile.id()) {
                                            if let Some(user_type) = &tileset_tile.user_type {
                                                //debug!("user_type: {:?}", user_type);
                                                let tile_type =
                                                    Map::to_tiletype(user_type.to_string());

                                                if let Some(tile_data) = data.get_tile_data(x, y) {
                                                    let tile_index = tile_data.tileset_index();
                                                    let tile_id = tile_data.id();

                                                    let tile = TileInfo {
                                                        tile_type: tile_type,
                                                        layers: vec![Map::tile_to_gid(
                                                            tile_index, tile_id,
                                                        )],
                                                    };

                                                    map.base.push(tile);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        tiled::TileLayer::Infinite(_data) => {}
                    },
                    _ => {}
                }
            } else if layer.name == "base2" {
                let mut index = 0;

                match layer.layer_type() {
                    tiled::LayerType::Tiles(layer) => match layer {
                        tiled::TileLayer::Finite(data) => {
                            for y in 0..HEIGHT {
                                for x in 0..WIDTH {
                                    if let Some(tile) = data.get_tile(x, y) {
                                        let tileset = tile.get_tileset();

                                        if let Some(tileset_tile) = tileset.get_tile(tile.id()) {
                                            if let Some(user_type) = &tileset_tile.user_type {
                                                let tile_type =
                                                    Map::to_tiletype(user_type.to_string());

                                                if let Some(tile_data) = data.get_tile_data(x, y) {
                                                    let tile_index = tile_data.tileset_index();
                                                    let tile_id = tile_data.id();

                                                    map.base[index].tile_type = tile_type;
                                                    map.base[index].layers.push(Map::tile_to_gid(
                                                        tile_index, tile_id,
                                                    ));
                                                }
                                            }
                                        }
                                    }

                                    index += 1;
                                }
                            }
                        }
                        tiled::TileLayer::Infinite(_data) => {}
                    },
                    _ => {}
                }
            } else if layer.name == "temperature" {
                match layer.layer_type() {
                    tiled::LayerType::Tiles(layer) => match layer {
                        tiled::TileLayer::Finite(data) => {
                            for y in 0..HEIGHT {
                                for x in 0..WIDTH {
                                    if let Some(tile) = data.get_tile(x, y) {
                                        let tileset = tile.get_tileset();
                                        if let Some(tileset_tile) = tileset.get_tile(tile.id()) {
                                            if let Some(user_type) = &tileset_tile.user_type {
                                                let temperature_type =
                                                    Map::to_temperature_type(user_type.to_string());
                                                map.temperature.push(temperature_type);
                                            }
                                        }
                                    } else {
                                        map.temperature.push(TemperatureType::Unknown);
                                    }
                                }
                            }
                        }
                        tiled::TileLayer::Infinite(_data) => {}
                    },
                    _ => {}
                }
            } else if layer.name == "moisture" {
                match layer.layer_type() {
                    tiled::LayerType::Tiles(layer) => match layer {
                        tiled::TileLayer::Finite(data) => {
                            for y in 0..HEIGHT {
                                for x in 0..WIDTH {
                                    if let Some(tile) = data.get_tile(x, y) {
                                        let tileset = tile.get_tileset();
                                        if let Some(tileset_tile) = tileset.get_tile(tile.id()) {
                                            if let Some(user_type) = &tileset_tile.user_type {
                                                let moisture_type =
                                                    Map::to_moisture_type(user_type.to_string());
                                                map.moisture.push(moisture_type);
                                            }
                                        }
                                    } else {
                                        map.moisture.push(MoistureType::Unknown);
                                    }
                                }
                            }
                        }
                        tiled::TileLayer::Infinite(_data) => {}
                    },
                    _ => {}
                }
            }
        }

        // Initialize wildness, potentially based off terrain type
        for i in 0..HEIGHT * WIDTH {
            map.wildness.push(4);
        }

        map
    }

    pub fn pos_to_tuple(pos: Position) -> (i32, i32) {
        (pos.x, pos.y)
    }

    pub fn pos_to_index(x: i32, y: i32) -> usize {
        let tile_index: usize = (y as usize) * (WIDTH as usize) + (x as usize);
        return tile_index;
    }

    pub fn index_to_pos(index: usize) -> MapPos {
        let x = (index as i32) % WIDTH;
        let y = (index as i32) / WIDTH;

        return MapPos(x, y);
    }

    pub fn find_fast_path(
        src_pos: Position,
        dst_pos: Position,
        map: &Map,
        mover_player_id: i32, // TODO This should be expanded to include details about the mover
        mut blocking_list: Vec<Blocker>,
        landwalk: bool,
        waterwalk: bool,
        mountainwalk: bool,
        ignore_goal_terrain_type: bool,
        allow_attackable_blockers: bool,
    ) -> Option<(Vec<MapPos>, u32)> {
        let goal: MapPos = MapPos(dst_pos.x, dst_pos.y);

        if let Some(pos) = blocking_list
            .iter()
            .position(|blocker| blocker.pos.x == goal.0 && blocker.pos.y == goal.1)
        {
            blocking_list.swap_remove(pos);
        }

        let mut counter = 0;
        let mut aborted = false;

        let result = astar(
            &MapPos(src_pos.x, src_pos.y),
            |p| {
                p.successors(
                    &map,
                    mover_player_id,
                    &blocking_list,
                    landwalk,
                    waterwalk,
                    mountainwalk,
                    ignore_goal_terrain_type,
                    allow_attackable_blockers,
                    goal.clone(),
                )
            },
            |p| p.distance(&goal),
            |p| {
                if counter > 200 {
                    aborted = true;
                    return true;
                } else {
                    counter += 1;
                    return *p == goal;
                }
            },
        );

        debug!("find_fast_path aborted: {:?}", aborted);
        debug!("find_fast_path result: {:?}", result);

        return result;
    }

    pub fn find_path(
        src_pos: Position,
        dst_pos: Position,
        map: &Map,
        mover_player_id: i32,
        mut blocking_list: Vec<Blocker>,
        landwalk: bool,
        waterwalk: bool,
        mountainwalk: bool,
        ignore_goal_terrain_type: bool,
        allow_attackable_blockers: bool,
    ) -> Option<(Vec<MapPos>, u32)> {
        let goal: MapPos = MapPos(dst_pos.x, dst_pos.y);

        if let Some(pos) = blocking_list
            .iter()
            .position(|blocker| blocker.pos.x == goal.0 && blocker.pos.y == goal.1)
        {
            blocking_list.swap_remove(pos);
        }

        let mut counter = 0;
        let mut aborted = false;

        let result = astar(
            &MapPos(src_pos.x, src_pos.y),
            |p| {
                p.successors(
                    &map,
                    mover_player_id,
                    &blocking_list,
                    landwalk,
                    waterwalk,
                    mountainwalk,
                    ignore_goal_terrain_type,
                    allow_attackable_blockers,
                    goal.clone(),
                )
            },
            |p| p.distance(&goal),
            |p| {
                if counter > 10000 {
                    aborted = true;
                    return true;
                } else {
                    counter += 1;
                    return *p == goal;
                }
            },
        );

        debug!("find_path aborted: {:?}", aborted);
        debug!("find_path result: {:?}", result);

        return result;
    }

    pub fn get_tiles_by_range(center_x: i32, center_y: i32, r: u32, map: Map) -> Vec<MapTile> {
        let mut tiles = Vec::new();
        let neighbours = Map::range((center_x, center_y), r);

        for neighbour in neighbours {
            let (x, y) = neighbour;

            //Reminder tile_index = y * width + x
            let tile_index: usize = (y as usize) * (WIDTH as usize) + (x as usize);
            let layers = map.base[tile_index].layers.clone();

            let tile = MapTile {
                x: x,
                y: y,
                t: layers,
            };

            tiles.push(tile);
        }

        tiles
    }

    pub fn are_tile_types_nearby(pos: Position, tile_types: Vec<TileType>, map: &Map) -> bool {
        let neighbours = Map::range((pos.x, pos.y), 1);

        for neighbour in neighbours {
            let (x, y) = neighbour;

            if tile_types.contains(&Map::tile_type(x, y, &map)) {
                return true;
            }
        }

        return false;
    }

    pub fn get_nearby_tile_types(
        pos: Position,
        tile_types: Vec<TileType>,
        map: &Map,
    ) -> Vec<TileType> {
        let mut nearby_tile_types = Vec::new();
        let neighbours = Map::range((pos.x, pos.y), 1);

        for neighbour in neighbours {
            let (x, y) = neighbour;

            if tile_types.contains(&Map::tile_type(x, y, &map)) {
                nearby_tile_types.push(Map::tile_type(x, y, &map));
            }
        }

        return nearby_tile_types;
    }

    pub fn get_nearby_tiles_by_types(
        pos: Position,
        tile_types: Vec<TileType>,
        map: &Map,
    ) -> Vec<(Position, TileType)> {
        let mut nearby_tile_types = Vec::new();
        let neighbours = Map::range((pos.x, pos.y), 1);

        for neighbour in neighbours {
            let (x, y) = neighbour;

            if tile_types.contains(&Map::tile_type(x, y, &map)) {
                nearby_tile_types.push((Position { x: x, y: y }, Map::tile_type(x, y, &map)));
            }
        }

        return nearby_tile_types;
    }

    pub fn pos_to_tiles(positions: &Vec<(i32, i32)>, map: &Map) -> Vec<MapTile> {
        let mut tiles = Vec::new();

        for pos in positions.iter() {
            let (x, y) = pos;

            //Reminder tile_index = y * width + x
            let tile_index = *y * WIDTH + *x;
            let tile_index_usize = tile_index as usize;
            let layers = map.base[tile_index_usize].layers.clone();

            let tile = MapTile {
                x: *x,
                y: *y,
                t: layers,
            };

            tiles.push(tile);
        }

        tiles
    }

    pub fn is_passable_by_obj(
        x: i32,
        y: i32,
        landwalk: bool,
        waterwalk: bool,
        mountainwalk: bool,
        map: &Map,
    ) -> bool {
        let tile_index = y * WIDTH + x;
        let tile_index_usize = tile_index as usize;
        let tile_type = map.base[tile_index_usize].tile_type.clone();

        let passable = match (tile_type, landwalk, waterwalk, mountainwalk) {
            (TileType::Ocean, _, true, _) => true,
            (TileType::Ocean, _, false, _) => false,
            (TileType::River, _, true, _) => true,
            (TileType::River, _, false, _) => false,
            (TileType::Mountain, _, _, true) => true,
            (TileType::Mountain, _, _, false) => false,
            (_, false, _, _) => false,
            _ => true,
        };

        return passable;
    }

    pub fn is_passable(x: i32, y: i32, map: &Map) -> bool {
        let tile_index = y * WIDTH + x;
        let tile_index_usize = tile_index as usize;
        //let layers = map.base[tile_index_usize].layers.clone();
        let tile_type = map.base[tile_index_usize].tile_type.clone();

        let passable = match tile_type {
            TileType::Ocean => false,
            TileType::River => false,
            TileType::Mountain => false,
            _ => true,
        };

        return passable;
    }

    pub fn tile_type(x: i32, y: i32, map: &Map) -> TileType {
        let tile_index = y * WIDTH + x;
        let tile_index_usize = tile_index as usize;
        //let layers = map.base[tile_index_usize].layers.clone();
        let tile_type = map.base[tile_index_usize].tile_type.clone();

        return tile_type;
    }

    pub fn tile_name(tile_type: TileType) -> String {
        return tile_type.to_string();
    }

    pub fn movement_cost(tile_type: TileType) -> i32 {
        let movement_cost = match tile_type {
            TileType::Mountain => 5,
            TileType::HillsPlains => 3,
            TileType::HillsGrasslands => 3,
            TileType::HillsSnow => 3,
            TileType::HillsDesert => 3,
            TileType::DeciduousForest => 3,
            TileType::River => 6,
            _ => 1,
        };

        return movement_cost;
    }

    pub fn def_bonus(tile_type: TileType) -> f32 {
        let def_bonus = match tile_type {
            TileType::HillsPlains => 0.33,
            TileType::HillsGrasslands => 0.33,
            TileType::HillsSnow => 0.33,
            TileType::HillsDesert => 0.33,
            TileType::DeciduousForest => 0.5,
            TileType::PineForest => 0.5,
            TileType::FrozenForest => 0.5,
            TileType::Jungle => 0.75,
            TileType::Swamp => 0.66,
            _ => 0.0,
        };

        return def_bonus;
    }

    pub fn tile_temperature(&self, x: i32, y: i32) -> TemperatureType {
        let tile_index = y * WIDTH + x;
        let tile_index_usize = tile_index as usize;
        let temperature_type = self.temperature[tile_index_usize].clone();

        return temperature_type;
    }

    pub fn tile_moisture(&self, x: i32, y: i32) -> MoistureType {
        let tile_index = y * WIDTH + x;
        let tile_index_usize = tile_index as usize;
        let moisture_type = self.moisture[tile_index_usize].clone();

        return moisture_type;
    }

    fn to_tiletype(tile_name: String) -> TileType {
        match tile_name.as_str() {
            "Grasslands" => TileType::Grasslands,
            "Snow" => TileType::Snow,
            "River" => TileType::River,
            "Ocean" => TileType::Ocean,
            "Plains" => TileType::Plains,
            "HillsPlains" => TileType::HillsPlains,
            "Desert" => TileType::Desert,
            "Oasis" => TileType::Oasis,
            "HillsDesert" => TileType::HillsDesert,
            "HillsGrasslands" => TileType::HillsGrasslands,
            "Swamp" => TileType::Swamp,
            "HillsSnow" => TileType::HillsSnow,
            "DeciduousForest" => TileType::DeciduousForest,
            "Rainforest" => TileType::Rainforest,
            "Jungle" => TileType::Jungle,
            "Savanna" => TileType::Savanna,
            "FrozenForest" => TileType::FrozenForest,
            "PineForest" => TileType::PineForest,
            "PalmForest" => TileType::PalmForest,
            "Mountain" => TileType::Mountain,
            "Volcano" => TileType::Volcano,
            _ => TileType::Unknown,
        }
    }

    fn to_temperature_type(temperature_name: String) -> TemperatureType {
        match temperature_name.as_str() {
            "Tropical" => TemperatureType::Tropical,
            "Subtropical" => TemperatureType::Subtropical,
            "WarmTemperate" => TemperatureType::WarmTemperate,
            "CoolTemperate" => TemperatureType::CoolTemperate,
            "Boreal" => TemperatureType::Boreal,
            "Subpolar" => TemperatureType::Subpolar,
            "Polar" => TemperatureType::Polar,
            _ => TemperatureType::Unknown,
        }
    }

    fn to_moisture_type(temperature_name: String) -> MoistureType {
        match temperature_name.as_str() {
            "Super Humid" => MoistureType::SuperHumid,
            "Humid" => MoistureType::Humid,
            "Semi Humid" => MoistureType::SemiHumid,
            "Arid" => MoistureType::Arid,
            "Super Arid" => MoistureType::SuperArid,
            "Unknown" => MoistureType::Unknown,
            _ => MoistureType::Unknown,
        }
    }

    fn gid_to_tiletype(gid: u32) -> TileType {
        match gid {
            1 => TileType::Grasslands,
            2 => TileType::Snow,
            3 => TileType::River,
            4 => TileType::River,
            5 => TileType::Ocean,
            6 => TileType::Plains,
            7 => TileType::HillsPlains,
            8 => TileType::HillsPlains,
            9 => TileType::Plains,
            10 => TileType::Desert,
            11 => TileType::Oasis,
            12 => TileType::HillsDesert,
            13 => TileType::HillsGrasslands,
            14 => TileType::Swamp,
            15 => TileType::Swamp,
            16 => TileType::HillsSnow,
            17 => TileType::Ocean,
            18 => TileType::Swamp,
            19 => TileType::DeciduousForest,
            20 => TileType::Rainforest,
            21 => TileType::Jungle,
            22 => TileType::Savanna,
            23 => TileType::DeciduousForest,
            24 => TileType::DeciduousForest,
            25 => TileType::FrozenForest,
            26 => TileType::FrozenForest,
            27 => TileType::PineForest,
            28 => TileType::FrozenForest,
            29 => TileType::Savanna,
            30 => TileType::PalmForest,
            31 => TileType::Jungle,
            32 => TileType::Mountain,
            33 => TileType::Mountain,
            34 => TileType::Mountain,
            35 => TileType::Mountain,
            36 => TileType::Mountain,
            37 => TileType::Mountain,
            38 => TileType::Mountain,
            39 => TileType::Volcano,
            _ => TileType::Unknown,
        }
    }

    fn tile_to_gid(tile_index: usize, tile_id: u32) -> u32 {
        match (tile_index, tile_id) {
            (0, 0) => 1,
            (0, 1) => 2,
            (0, 2) => 3,
            (0, 3) => 4,
            (0, 4) => 5,
            (0, 5) => 6,
            (0, 6) => 7,
            (0, 7) => 8,
            (0, 8) => 9,
            (0, 9) => 10,
            (0, 10) => 11,
            (0, 11) => 12,
            (0, 12) => 13,
            (0, 13) => 14,
            (0, 14) => 15,
            (0, 15) => 16,
            (0, 16) => 17,
            (1, 0) => 18,
            (2, 0) => 19,
            (2, 1) => 20,
            (2, 2) => 21,
            (2, 3) => 22,
            (2, 4) => 23,
            (2, 5) => 24,
            (2, 6) => 25,
            (2, 7) => 26,
            (2, 8) => 27,
            (2, 9) => 28,
            (2, 10) => 29,
            (2, 11) => 30,
            (2, 12) => 31,
            (3, 0) => 32,
            (3, 1) => 33,
            (3, 2) => 34,
            (3, 3) => 35,
            (3, 4) => 36,
            (3, 5) => 37,
            (3, 6) => 38,
            (3, 7) => 39,
            _ => 0,
        }
    }

    fn odd_q_to_cube((q, r): (i32, i32)) -> (i32, i32, i32) {
        let x = q;
        let z = r - (q - (q & 1)) / 2;
        let y = -x - z;
        (x, y, z)
    }

    fn cube_to_odd_q((x, _y, z): (i32, i32, i32)) -> (i32, i32) {
        let q = x;
        let r = z + (x - (x & 1)) / 2;
        (q, r)
    }

    pub fn distance(src_pos: (i32, i32), dst_pos: (i32, i32)) -> u32 {
        let (sx, sy, sz) = Map::odd_q_to_cube(src_pos);
        let (dx, dy, dz) = Map::odd_q_to_cube(dst_pos);

        let distance = (((sx - dx).abs() + (sy - dy).abs() + (sz - dz).abs()) / 2) as u32;

        distance
    }

    pub fn dist(src_pos: Position, dst_pos: Position) -> u32 {
        let src_pos_tuple = (src_pos.x, src_pos.y);
        let dst_pos_tuple = (dst_pos.x, dst_pos.y);

        let (sx, sy, sz) = Map::odd_q_to_cube(src_pos_tuple);
        let (dx, dy, dz) = Map::odd_q_to_cube(dst_pos_tuple);

        let distance = (((sx - dx).abs() + (sy - dy).abs() + (sz - dz).abs()) / 2) as u32;

        distance
    }

    fn cube_direction(direction: i32) -> (i32, i32, i32) {
        let neighbours_table: Vec<(i32, i32, i32)> = vec![
            (1, -1, 0),
            (1, 0, -1),
            (0, 1, -1),
            (-1, 1, 0),
            (-1, 0, 1),
            (0, -1, 1),
        ];

        return neighbours_table[direction as usize];
    }

    fn cube_neighbour(cube: (i32, i32, i32), direction: i32) -> (i32, i32, i32) {
        return Map::cube_add(cube, Map::cube_direction(direction));
    }

    fn cube_add(cube1: (i32, i32, i32), cube2: (i32, i32, i32)) -> (i32, i32, i32) {
        return (cube1.0 + cube2.0, cube1.1 + cube2.1, cube1.2 + cube2.2);
    }

    fn cube_scale(cube: (i32, i32, i32), factor: i32) -> (i32, i32, i32) {
        return (cube.0 * factor, cube.1 * factor, cube.2 * factor);
    }

    pub fn ring((q, r): (i32, i32), radius: i32) -> Vec<(i32, i32)> {
        let mut results: Vec<(i32, i32)> = Vec::new();

        let center = Map::odd_q_to_cube((q, r));

        let mut hex = Map::cube_add(center, Map::cube_scale(Map::cube_direction(4), radius));

        for i in 0..6 {
            for _j in 0..radius {
                results.push(Map::cube_to_odd_q(hex));
                hex = Map::cube_neighbour(hex, i);
            }
        }

        return results;
    }

    pub fn range((q, r): (i32, i32), num: u32) -> Vec<(i32, i32)> {
        let n = num as i32;

        let mut result: Vec<(i32, i32)> = Vec::new();

        let (cx, cy, cz) = Map::odd_q_to_cube((q, r));

        //TODO could be optimized as per Amit's hex guide
        for sx in -n..=n {
            for sy in -n..=n {
                for sz in -n..=n {
                    if (cx + sx) + (cy + sy) + (cz + sz) == 0 {
                        let pos = Map::cube_to_odd_q(((cx + sx), (cy + sy), (cz + sz)));

                        if Map::is_valid_pos(pos) {
                            result.push(pos);
                        }
                    }
                }
            }
        }

        result
    }

    pub fn get_neighbour_tiles(
        origin_x: i32,
        origin_y: i32,
        map: &Map,
        mover_player_id: i32,
        blocking_list: &Vec<Blocker>,
        landwalk: bool,
        waterwalk: bool,
        mountainwalk: bool,
        ignore_goal_terrain_type: bool,
        allow_attackable_blockers: bool,
        goal: MapPos,
    ) -> Vec<(MapPos, u32)> {
        let neighbours_table: Vec<(i32, i32, i32)> = vec![
            (1, -1, 0),
            (1, 0, -1),
            (0, 1, -1),
            (-1, 1, 0),
            (-1, 0, 1),
            (0, -1, 1),
        ];

        let mut result: Vec<(MapPos, u32)> = Vec::new();

        let (x, y, z) = Map::odd_q_to_cube((origin_x, origin_y));

        for (nx, ny, nz) in neighbours_table {
            let neighbour_cube = (x + nx, y + ny, z + nz);
            let neighbour = Map::cube_to_odd_q(neighbour_cube);
            let neighbour_x = neighbour.0;
            let neighbour_y = neighbour.1;

            // Skip coordinates out of bounds
            if neighbour_x < 0 || neighbour_y < 0 || neighbour_x >= WIDTH || neighbour_y >= HEIGHT {
                continue;
            }

            //Reminder tile_index = y * width + x
            //debug!("neighbour_y: {:?} neighbour_x: {:?}", neighbour_y, neighbour_y);
            let tile_index = neighbour_y * WIDTH + neighbour_x;
            let tile_index_usize = tile_index as usize;
            let tile_type = map.base[tile_index_usize].tile_type.clone();

            let is_valid_pos = Map::is_valid_pos(neighbour);
            let is_passable_by_obj = Map::is_passable_by_obj(
                neighbour_x,
                neighbour_y,
                landwalk,
                waterwalk,
                mountainwalk,
                map,
            );
            let is_not_blocked = Map::is_not_blocked(neighbour, blocking_list);
            let is_blocker_attackable = Map::is_blocker_attackable(
                mover_player_id,
                neighbour,
                blocking_list,
                allow_attackable_blockers,
            );
            //info!("blocking_list: {:?} is_not_blocked: {:?} is_blocker_attackable: {:?}", blocking_list, is_not_blocked, is_blocker_attackable);

            let mut allow_move_to_goal = false;

            if ignore_goal_terrain_type {
                if goal.0 == neighbour_x && goal.1 == neighbour_y {
                    allow_move_to_goal = true;
                }
            }

            if (is_valid_pos && is_passable_by_obj && (is_not_blocked || is_blocker_attackable))
                || allow_move_to_goal
            {
                let movement_cost: u32;

                if is_blocker_attackable {
                    movement_cost = 10;
                } else {
                    movement_cost = Map::movement_cost(tile_type) as u32;
                }

                result.push((MapPos(neighbour_x, neighbour_y), movement_cost));
            }
        }

        return result;
    }

    pub fn get_wander_tiles(
        origin_x: i32,
        origin_y: i32,
        map: &Map,
        mover_player_id: i32,
        blocking_list: &Vec<Blocker>,
        monolith_list: &Vec<MapPos>,
        landwalk: bool,
        waterwalk: bool,
        mountainwalk: bool,
    ) -> Option<MapPos> {
        let neighbours_table: Vec<(i32, i32, i32)> = vec![
            (1, -1, 0),
            (1, 0, -1),
            (0, 1, -1),
            (-1, 1, 0),
            (-1, 0, 1),
            (0, -1, 1),
        ];

        let mut wander_pos_list: Vec<MapPos> = Vec::new();

        let (x, y, z) = Map::odd_q_to_cube((origin_x, origin_y));

        for (nx, ny, nz) in neighbours_table {
            // Skip origin
            if nx == x && ny == y && nz == z {
                continue;
            }

            let neighbour_cube = (x + nx, y + ny, z + nz);
            let neighbour = Map::cube_to_odd_q(neighbour_cube);
            let neighbour_x = neighbour.0;
            let neighbour_y = neighbour.1;

            // Skip coordinates out of bounds
            if neighbour_x < 0 || neighbour_y < 0 || neighbour_x >= WIDTH || neighbour_y >= HEIGHT {
                continue;
            }

            //Reminder tile_index = y * width + x
            //debug!("neighbour_y: {:?} neighbour_x: {:?}", neighbour_y, neighbour_y);
            let tile_index = neighbour_y * WIDTH + neighbour_x;
            let tile_index_usize = tile_index as usize;
            let tile_type = map.base[tile_index_usize].tile_type.clone();

            let is_valid_pos = Map::is_valid_pos(neighbour);
            let is_passable_by_obj = Map::is_passable_by_obj(
                neighbour_x,
                neighbour_y,
                landwalk,
                waterwalk,
                mountainwalk,
                map,
            );
            let is_not_blocked = Map::is_not_blocked(neighbour, blocking_list);
            let is_near_monolith = Map::is_near_monolith(neighbour, monolith_list);

            if is_valid_pos && is_passable_by_obj && is_not_blocked && is_near_monolith {
                wander_pos_list.push(MapPos(neighbour_x, neighbour_y));
            }
        }

        let mut rng = rand::thread_rng();
        let random_index = rng.gen_range(0..wander_pos_list.len());

        if let Some(random_pos) = wander_pos_list.get(random_index) {
            return Some(random_pos.clone());
        }

        return None;
    }

    fn is_not_blocked((x, y): (i32, i32), blocking_list: &Vec<Blocker>) -> bool {
        for blocker in blocking_list {
            if x == blocker.pos.x && y == blocker.pos.y {
                return false;
            }
        }

        return true;
    }

    fn is_blocker_attackable(
        mover_player_id: i32,
        (x, y): (i32, i32),
        blocking_list: &Vec<Blocker>,
        allow_attackable_blockers: bool,
    ) -> bool {
        if !allow_attackable_blockers {
            return false;
        }

        for blocker in blocking_list {
            if x == blocker.pos.x && y == blocker.pos.y {
                // TODO check relations between mover and blocker
                return true;
            }
        }

        return false;
    }

    fn is_near_monolith((x, y): (i32, i32), monolith_list: &Vec<MapPos>) -> bool {
        for monolith_pos in monolith_list {
            if Map::distance((x, y), (monolith_pos.0, monolith_pos.1)) < SANCTUARY_RANGE {
                return false;
            }
        }

        return true;
    }

    fn neighbours((q, r): (i32, i32)) -> Vec<(i32, i32)> {
        let neighbours_table: Vec<(i32, i32, i32)> = vec![
            (1, -1, 0),
            (1, 0, -1),
            (0, 1, -1),
            (-1, 1, 0),
            (-1, 0, 1),
            (0, -1, 1),
        ];

        let mut result: Vec<(i32, i32)> = Vec::new();

        let (x, y, z) = Map::odd_q_to_cube((q, r));

        for (nx, ny, nz) in neighbours_table {
            let neighbour_cube = (x + nx, y + ny, z + nz);
            let neighbour_odd = Map::cube_to_odd_q(neighbour_cube);

            if Map::is_valid_pos(neighbour_odd) {
                result.push(neighbour_odd);
            }
        }

        result
    }

    pub fn is_adjacent_excluding_source(source_pos: Position, target_pos: Position) -> bool {
        let neighbours = Self::neighbours((source_pos.x, source_pos.y));

        for (x, y) in neighbours {
            if x == target_pos.x && y == target_pos.y {
                return true;
            }
        }

        return false;
    }

    pub fn is_adjacent_including_source(source_pos: Position, target_pos: Position) -> bool {
        let neighbours = Self::neighbours((source_pos.x, source_pos.y));

        // Experimenting with treating the source position as adjacent
        if source_pos == target_pos {
            return true;
        }

        for (x, y) in neighbours {
            if x == target_pos.x && y == target_pos.y {
                return true;
            }
        }

        return false;
    }

    pub fn is_valid_pos((q, r): (i32, i32)) -> bool {
        q >= 0 && r >= 0 && q < (WIDTH as i32) && r < (HEIGHT as i32)
    }

    pub fn in_empire(pos: Position) -> bool {
        match pos {
            Position { x: 0, y: 15 } => true,
            _ => false,
        }
    }

    // Climate functions

    pub fn get_temperature(
        season: Season,
        hour: i32,
        temperature_type: TemperatureType,
        moisture_type: MoistureType,
        weather: Weather,
    ) -> f32 {
        let (day_temperature, night_temperature) =
            Map::season_temperatures(season, temperature_type, moisture_type);

        let average_temperature = (day_temperature + night_temperature) / 2.0;
        let temperature_amplitude = (day_temperature - night_temperature) / 2.0;

        // Define sunrise and sunset times
        let sunrise = 4; // Sunrise at 4 AM
        let sunset = 22; // Sunset at 10 PM

        // Calculate the number of daylight hours and nighttime hours
        let daylight_hours = sunset - sunrise;
        let nighttime_hours = 24 - daylight_hours;

        // Adjust the phase of the cosine wave to match sunrise and sunset times
        // Assuming temperature peaks at midday and is lowest just before sunrise
        let midday = sunrise + daylight_hours / 2;
        let base_temperature = average_temperature
            + temperature_amplitude * ((hour - midday) as f32 / 24.0 * 2.0 * PI).cos();

        // Final temperature calculation
        let temperature = base_temperature + Map::get_weather_temp_modifier(weather);

        temperature
    }

    pub fn season_temperatures(
        season: Season,
        temperature_type: TemperatureType,
        moisture_type: MoistureType,
    ) -> (f32, f32) {
        match temperature_type {
            TemperatureType::Tropical => match moisture_type {
                MoistureType::SuperHumid => match season {
                    Season::Spring => (32.0, 24.0),
                    Season::Summer => (34.0, 25.0),
                    Season::Autumn => (32.0, 24.0),
                    Season::Winter => (31.0, 23.0),
                },
                MoistureType::Humid => (30.5, 25.5),
                MoistureType::SemiHumid => (30.5, 25.5),
                MoistureType::Arid => (30.5, 25.5),
                MoistureType::SuperArid => match season {
                    Season::Spring => (30.0, 15.0),
                    Season::Summer => (40.0, 25.0),
                    Season::Autumn => (35.0, 20.0),
                    Season::Winter => (25.0, 10.0),
                },
                MoistureType::Unknown => (30.5, 25.5),
            },
            TemperatureType::Subtropical => match moisture_type {
                MoistureType::SuperHumid => (30.5, 25.5),
                MoistureType::Humid => (30.5, 25.5),
                MoistureType::SemiHumid => (30.5, 25.5),
                MoistureType::Arid => (30.5, 25.5),
                MoistureType::SuperArid => (30.5, 25.5),
                MoistureType::Unknown => (30.5, 25.5),
            },
            TemperatureType::WarmTemperate => match moisture_type {
                MoistureType::SuperHumid => (30.5, 25.5),
                MoistureType::Humid => (30.5, 25.5),
                MoistureType::SemiHumid => (30.5, 25.5),
                MoistureType::Arid => (30.5, 25.5),
                MoistureType::SuperArid => (30.5, 25.5),
                MoistureType::Unknown => (30.5, 25.5),
            },
            TemperatureType::CoolTemperate => match moisture_type {
                MoistureType::SuperHumid => (30.5, 25.5),
                MoistureType::Humid => (30.5, 25.5),
                MoistureType::SemiHumid => (30.5, 25.5),
                MoistureType::Arid => (30.5, 25.5),
                MoistureType::SuperArid => (30.5, 25.5),
                MoistureType::Unknown => (30.5, 25.5),
            },
            TemperatureType::Boreal => match moisture_type {
                MoistureType::SuperHumid => (30.5, 25.5),
                MoistureType::Humid => (30.5, 25.5),
                MoistureType::SemiHumid => (30.5, 25.5),
                MoistureType::Arid => (30.5, 25.5),
                MoistureType::SuperArid => (30.5, 25.5),
                MoistureType::Unknown => (30.5, 25.5),
            },
            TemperatureType::Subpolar => match moisture_type {
                MoistureType::SuperHumid => (30.5, 25.5),
                MoistureType::Humid => (30.5, 25.5),
                MoistureType::SemiHumid => (30.5, 25.5),
                MoistureType::Arid => (30.5, 25.5),
                MoistureType::SuperArid => (30.5, 25.5),
                MoistureType::Unknown => (30.5, 25.5),
            },
            TemperatureType::Polar => match moisture_type {
                MoistureType::Arid => (30.5, 25.5),
                MoistureType::SuperArid => (30.5, 25.5),
                _ => (30.5, 25.5),
            },
            TemperatureType::Unknown => (30.5, 25.5),
        }
    }

    pub fn get_weather_temp_modifier(weather: Weather) -> f32 {
        match weather {
            Weather::ClearSunny => 0.0,
            Weather::HeavyRain => -2.0,
            Weather::Snow => -3.0,
            Weather::Fog => -1.0,
            Weather::Hail => -3.0,
            Weather::Thunderstorm => -3.0,
            Weather::Tornado => -5.0,
            Weather::Hurricane => -5.0,
            Weather::Blizzard => -5.0,
            _ => 0.0,
        }
    }

    /*pub fn initialize_weather_areas(num_areas: i32) -> HashMap<(i32, i32), Vec<(i32, i32)>> {
        let mut weather_areas: HashMap<(i32, i32), Vec<(i32, i32)>> = HashMap::new();
        let mut area_centers: Vec<(i32, i32)> = Vec::new();

        // Randomly select area centers
        for _ in 0..num_areas {
            let x = rand::thread_rng().gen_range(0..WIDTH);
            let y = rand::thread_rng().gen_range(0..HEIGHT);
            area_centers.push((x, y));
            weather_areas.insert((x, y), Vec::new());
        }

        // Hardcoded for testing
        area_centers.push((16, 36));
        weather_areas.insert((16, 36), Vec::new());

        // Iterate through the area centers and randomly pick a radius of tiles to assign to each area
        for center in area_centers {
            let radius = rand::thread_rng().gen_range(3..6);
            let tiles = Map::range(center, radius as u32);

            for tile in tiles {
                weather_areas.get_mut(&center).unwrap().push(tile);
            }
        }

        return weather_areas;
    }*/

    pub fn get_wildness(&self, x: i32, y: i32) -> i32 {
        let tile_index = y * WIDTH + x;
        let tile_index_usize = tile_index as usize;

        return self.wildness[tile_index_usize];
    }

    pub fn get_wildness_string(&self, x: i32, y: i32) -> String {
        let wildness = self.get_wildness(x, y);
        match wildness {
            0 => "Secure".to_string(),
            1 => "Untamed".to_string(),
            2 => "Wild".to_string(),
            3 => "Savage".to_string(),
            4 => "Ferocious".to_string(),
            _ => "Unknown".to_string(),
        }
    }

    pub fn update_wildness(&mut self, x: i32, y: i32, value: i32) {
        let tile_index = y * WIDTH + x;
        let tile_index_usize = tile_index as usize;

        self.wildness[tile_index_usize] += value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::CLASS_STRUCTURE;
    use crate::obj::{Class, Id, PlayerId, State, Subclass};

    fn flat_test_map() -> Map {
        Map {
            width: WIDTH,
            height: HEIGHT,
            base: vec![
                TileInfo {
                    tile_type: TileType::Grasslands,
                    layers: vec![1],
                };
                (WIDTH * HEIGHT) as usize
            ],
            temperature: Vec::new(),
            moisture: Vec::new(),
            wildness: vec![0; (WIDTH * HEIGHT) as usize],
        }
    }

    fn vertical_wall_blockers(x: i32) -> Vec<Blocker> {
        (0..HEIGHT)
            .map(|y| Blocker {
                player_id: PlayerId(1),
                id: Id(1000 + y),
                pos: Position { x, y },
                class: Class(CLASS_STRUCTURE.to_string()),
                subclass: Subclass::Wall,
                state: State::None,
            })
            .collect()
    }

    #[test]
    fn test_load_map() {
        let map: Map = Map::load_map();

        let tile_index: usize = (35 * WIDTH + 17).try_into().unwrap();
        assert_eq!(map.base[tile_index].layers, vec![13]);
        assert_eq!(map.base[tile_index].tile_type, TileType::HillsGrasslands);
    }

    #[test]
    fn test_get_tiles_by_range() {
        let map: Map = Map::load_map();

        let tiles = Map::get_tiles_by_range(16, 36, 2, map);

        println!("{:#?}", tiles);

        let test_tiles = r#"[{"t":[13],"x":17,"y":34},{"t":[1],"x":16,"y":37},{"t":[1],"x":16,"y":35},{"t":[13],"x":18,"y":36},{"t":[5],"x":15,"y":36},{"t":[1],"x":17,"y":37},{"t":[1],"x":15,"y":34},{"t":[5],"x":14,"y":37},{"t":[13],"x":17,"y":35},{"t":[1],"x":16,"y":38},{"t":[1],"x":14,"y":35},{"t":[1],"x":16,"y":36},{"t":[1],"x":18,"y":37},{"t":[13],"x":16,"y":34},{"t":[5],"x":15,"y":37},{"t":[1],"x":18,"y":35},{"t":[13],"x":15,"y":35},{"t":[1],"x":17,"y":36},{"t":[13],"x":14,"y":36}]"#;

        let deserialized_test_tiles: Vec<MapTile> = serde_json::from_str(&test_tiles).unwrap();

        println!("{:#?}", deserialized_test_tiles);

        for tile in tiles {
            assert_eq!(deserialized_test_tiles.contains(&tile), true);
        }
    }

    #[test]
    fn find_fast_path_respects_attackable_blocker_option() {
        let map = flat_test_map();
        let src = Position { x: 0, y: 25 };
        let dst = Position { x: 2, y: 25 };
        let blockers = vertical_wall_blockers(1);

        let blocked_path = Map::find_fast_path(
            src,
            dst,
            &map,
            2,
            blockers.clone(),
            true,
            false,
            false,
            true,
            false,
        );
        assert!(blocked_path.is_none());

        let attackable_path =
            Map::find_fast_path(src, dst, &map, 2, blockers, true, false, false, true, true);
        assert!(attackable_path.is_some());
    }

    #[test]
    fn test_odd_q_to_cube() {
        let odd_q = (2, 1);

        assert_eq!(Map::odd_q_to_cube(odd_q), (2, -2, 0));

        let odd_q = (-1, 2);
        assert_eq!(Map::odd_q_to_cube(odd_q), (-1, -2, 3));
    }

    #[test]
    fn test_cube_to_odd_q() {
        let cube = (2, -2, 0);

        assert_eq!(Map::cube_to_odd_q(cube), (2, 1));

        let cube = (-1, -2, 3);
        assert_eq!(Map::cube_to_odd_q(cube), (-1, 2));
    }

    #[test]
    fn test_distance() {
        let src_pos = (2, 4);
        let dst_pos = (5, 3);

        assert_eq!(Map::distance(src_pos, dst_pos), 3);

        let src_pos = (2, 4);
        let dst_pos = (5, 5);

        assert_eq!(Map::distance(src_pos, dst_pos), 3);

        let src_pos = (2, 4);
        let dst_pos = (5, 6);

        assert_eq!(Map::distance(src_pos, dst_pos), 4);

        let src_pos = (-2, 4);
        let dst_pos = (5, 6);
        assert_eq!(Map::distance(src_pos, dst_pos), 7);
    }

    #[test]
    fn test_range() {
        //[{2,3},{2,2},{3,3},{3,2},{3,1},{4,3},{4,2}]
        let result = vec![(2, 3), (2, 2), (3, 3), (3, 2), (3, 1), (4, 3), (4, 2)];

        assert_eq!(Map::range((3, 2), 1), result);

        //[{0,2},{0,1},{0,0},{1,1},{1,0}]
        let result = vec![(0, 2), (0, 1), (0, 0), (1, 1), (1, 0)];

        assert_eq!(Map::range((0, 1), 1), result);
    }

    #[test]
    fn test_neighbours() {
        //[{1,2},{0,2},{0,1},{1,0},{2,1},{2,2}]
        let result = vec![(2, 2), (2, 1), (1, 0), (0, 1), (0, 2), (1, 2)];

        assert_eq!(Map::neighbours((1, 1)), result);

        //[{0,1},{1,0}]
        let result = vec![(1, 0), (0, 1)];

        assert_eq!(Map::neighbours((0, 0)), result);
    }
}
