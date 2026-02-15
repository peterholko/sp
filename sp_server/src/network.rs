use bevy::prelude::Res;
use crossbeam_channel::Sender as CBSender;
use serde_with::skip_serializing_none;
use tokio_tungstenite::WebSocketStream;

use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use lazy_static::lazy_static;
use std::net::SocketAddr;
use uuid::Uuid;

use rustls::ServerConfig;
use tokio_rustls::{server::TlsStream, TlsAcceptor};

use deadpool_postgres::{Manager, Pool};
use tokio::net::{TcpListener, TcpStream};
use tokio_postgres::{Config, NoTls};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::{Message, Result};
use tokio_tungstenite::{accept_hdr_async, tungstenite::Error};

use serde::{Deserialize, Serialize};

use chrono::DateTime;
use chrono::Utc;

use crate::constants::{CREATING_HERO, DATABASE_MANAGER_ID, HERO_DEAD, PLAYING};
use crate::database::DatabaseEvent;
use crate::effect;
use crate::game::{DatabaseClient, DatabaseManagers};
use crate::map::MapTile;
use crate::{
    game::{Client, Clients},
    player::PlayerEvent,
};
use crate::{
    game::{ObjQueryItem, ObjQueryMutReadOnlyItem},
    item,
    obj::HeroClassList,
    resource::Property,
    templates::ResReq,
    trade::WantedItem,
};

use std::env;
use std::fs;
use std::path::Path;

use glob::glob;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::fs::File;
use std::io::{self, BufReader};

use std::sync::Arc;
use std::sync::Mutex;

use rustrict::CensorStr;

use dotenvy::dotenv;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "cmd")]
enum NetworkPacket {
    #[serde(rename = "login")]
    Login { account_name: String, password: String },
    #[serde(rename = "register")]
    Register { account_name: String, password: String },
    #[serde(rename = "select_class")]
    SelectedClass { class_name: String, hero_name: String },
    #[serde(rename = "recreate_hero")]
    RecreateHero,
    #[serde(rename = "get_stats")]
    GetStats { id: i32 },
    #[serde(rename = "image_def")]
    ImageDef { name: String },
    #[serde(rename = "move_unit")]
    Move { x: i32, y: i32 },
    #[serde(rename = "attack")]
    Attack {
        attack_type: String,
        source_id: i32,
        target_id: i32,
    },
    #[serde(rename = "combo")]
    Combo {
        source_id: i32,
        target_id: i32,
        combo_type: String,
    },
    #[serde(rename = "info_obj")]
    InfoObj { id: i32 },
    #[serde(rename = "info_skills")]
    InfoSkills { id: i32 },
    #[serde(rename = "info_attrs")]
    InfoAttrs { id: i32 },
    #[serde(rename = "info_advance")]
    InfoAdvance { source_id: i32 },
    #[serde(rename = "info_upgrade")]
    InfoUpgrade { structure_id: i32 },
    #[serde(rename = "info_tile")]
    InfoTile { x: i32, y: i32 },
    #[serde(rename = "info_tile_resources")]
    InfoTileResources { x: i32, y: i32 },
    #[serde(rename = "info_inventory")]
    InfoInventory { id: i32 },
    #[serde(rename = "info_equip")]
    InfoEquip { id: i32 },
    #[serde(rename = "info_item")]
    InfoItem { obj_id: i32, item_id: i32, action: String },
    #[serde(rename = "info_item_by_name")]
    InfoItemByName { name: String },
    #[serde(rename = "info_item_transfer")]
    InfoItemTransfer { source_id: i32, target_id: i32 },
    #[serde(rename = "info_exit")]
    InfoExit { id: i32, panel_type: String },
    #[serde(rename = "info_merchant")]
    InfoMerchant { source_id: i32, merchant_id: i32 },
    #[serde(rename = "info_hire")]
    InfoHire { source_id: i32 },
    #[serde(rename = "item_transfer")]
    ItemTransfer {item: i32, source_id: i32, target_id: i32 },
    #[serde(rename = "item_split")]
    ItemSplit { owner_id: i32, item: i32, quantity: i32 },
    #[serde(rename = "gather")]
    Gather,
    #[serde(rename = "operate")]
    Operate { structure_id: i32 },
    #[serde(rename = "plant")]
    Plant { structure_id: i32 },
    #[serde(rename = "tend")]
    Tend { structure_id: i32 },
    #[serde(rename = "harvest")]
    Harvest { structure_id: i32 },
    #[serde(rename = "refine")]
    Refine { item_id: i32 },
    #[serde(rename = "structure_refine")]
    StructureRefine { structure_id: i32, item_id: i32 },
    #[serde(rename = "craft")]
    Craft { recipe: String },
    #[serde(rename = "structure_craft")]
    StructureCraft { structure_id: i32, recipe: String },
    #[serde(rename = "sleep")]
    Sleep { structure_id: i32 },
    #[serde(rename = "order_follow")]
    OrderFollow { source_id: i32 },
    #[serde(rename = "order_gather")]
    OrderGather { source_id: i32, res_type: String },
    #[serde(rename = "order_operate")]
    OrderOperate { source_id: i32, structure_id: i32 },
    #[serde(rename = "order_refine")]
    OrderRefine { source_id: i32, structure_id: i32 },
    #[serde(rename = "order_craft")]
    OrderCraft { source_id: i32, structure_id: i32 },
    #[serde(rename = "order_explore")]
    OrderExplore { source_id: i32 },
    #[serde(rename = "order_experiment")]
    OrderExperiment { source_id: i32, structure_id: i32 },
    #[serde(rename = "order_plant")]
    OrderPlant { source_id: i32, structure_id: i32 },
    #[serde(rename = "order_tend")]
    OrderTend { source_id: i32, structure_id: i32 },
    #[serde(rename = "order_harvest")]
    OrderHarvest { source_id: i32, structure_id: i32 },
    #[serde(rename = "order_repair")]
    OrderRepair { source_id: i32 },
    #[serde(rename = "structure_list")]
    StructureList {},
    #[serde(rename = "create_foundation")]
    CreateFoundation { source_id: i32, structure: String },
    #[serde(rename = "build")]
    Build { source_id: i32, structure_id: i32 },
    #[serde(rename = "start_upgrade")]
    StartUpgrade {
        structure_id: i32,
        selected_upgrade: String,
    },
    #[serde(rename = "upgrade")]
    Upgrade { source_id: i32, structure_id: i32 },
    #[serde(rename = "experiment")]
    Experiment { structure_id: i32 },
    #[serde(rename = "activate")]
    Activate { structure_id: i32 },
    #[serde(rename = "survey")]
    Survey { source_id: i32 },
    #[serde(rename = "explore")]
    Explore {},
    #[serde(rename = "nearby_resources")]
    NearbyResources {},
    #[serde(rename = "info_assign")]
    InfoAssign { structure_id: i32 },
    #[serde(rename = "assign")]
    Assign { worker_id: i32, structure_id: i32 },
    #[serde(rename = "remove_assign")]
    RemoveAssign { worker_id: i32, structure_id: i32 },
    #[serde(rename = "equip")]
    Equip { obj_id: i32, item: i32, status: bool },
    #[serde(rename = "delete_item")]
    DeleteItem { obj_id: i32, item_id: i32 },
    #[serde(rename = "info_craft")]
    InfoCraft { crafter_id: i32},
    #[serde(rename = "info_structure_craft")]
    InfoStructureCraft { structure_id: i32 },
    #[serde(rename = "info_structure_queue")]
    InfoStructureQueue { structure_id: i32 },
    #[serde(rename = "info_work_queue_entry")]
    InfoWorkQueueEntry { structure_id: i32, index: i32 },
    #[serde(rename = "add_crafting_entry")]
    AddCraftingEntry {
        structure_id: i32,
        recipe_name: String,
    },
    #[serde(rename = "add_refine_entry")]
    AddRefineEntry {
        structure_id: i32,
        refine_item_id: i32,
    }, 
    #[serde(rename = "remove_work_entry")]
    RemoveWorkEntry { structure_id: i32, index: i32 },
    #[serde(rename = "info_refine")]
    InfoRefine { refiner_id: i32},
    #[serde(rename = "info_structure_refine")]
    InfoStructureRefine { structure_id: i32 },
    #[serde(rename = "info_structure_refine_item")]
    InfoStructureRefineItem {
        structure_id: i32,
        item_id: i32,
    },
    #[serde(rename = "use")]
    Use { obj_id: i32, item_id: i32 },
    #[serde(rename = "delete")]
    Remove { source_id: i32 },
    #[serde(rename = "advance")]
    Advance { source_id: i32 },
    #[serde(rename = "info_experiment")]
    InfoExperiment { structure_id: i32 },
    #[serde(rename = "set_exp_item")]
    SetExperimentItem { structure_id: i32, item_id: i32 },
    #[serde(rename = "set_exp_resource")]
    SetExperimentResource { structure_id: i32, item_id: i32 },
    #[serde(rename = "reset_experiment")]
    ResetExperiment { structure_id: i32 },
    #[serde(rename = "hire")]
    Hire { source_id: i32, target_id: i32 },
    #[serde(rename = "buy_item")]
    BuyItem { seller_id: i32, item_id: i32, quantity: i32 },
    #[serde(rename = "sell_item")]
    SellItem {
        item_id: i32,
        target_id: i32,
        quantity: i32,
    },
    #[serde(rename = "cancel_action")]
    CancelAction,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct StructureList {
    pub result: Vec<Structure>,
}

#[skip_serializing_none]
#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "packet")]
pub enum ResponsePacket {
    #[serde(rename = "select_class")]
    SelectClass {
        player: u32,
    },
    #[serde(rename = "info_select_class")]
    InfoSelectClass {
        result: String,
    },
    #[serde(rename = "login")]
    Login {
        player: u32,
    },
    #[serde(rename = "disconnect")]
    Disconnect {
        player: i32,
        client: Uuid,
    },
    #[serde(rename = "world")]
    World {
        time_of_day: String,
        day: i32,
    },
    #[serde(rename = "explored_map")]
    ExploredMap {
        tiles: Vec<MapTile>,
    },
    #[serde(rename = "init_perception")]
    InitPerception {
        data: PerceptionData,
    },
    #[serde(rename = "new_perception")]
    NewPerception {
        data: PerceptionData,
    },
    #[serde(rename = "new_obj_perception")]
    NewObjPerception {
        new_objs: Vec<MapObj>,
        new_tiles: Vec<MapTile>,
    },
    #[serde(rename = "perception_changes")]
    PerceptionChanges {
        events: Vec<ChangeEvents>,
    },
    #[serde(rename = "stats")]
    Stats {
        data: StatsData,
    },
    #[serde(rename = "info_hero")]
    InfoHero {
        id: i32,
        name: String,
        class: String,
        subclass: String,
        template: String,
        state: String,
        image: String,
        hsl: Vec<i32>,
        items: Option<Vec<Item>>,
        skills: Option<HashMap<String, i32>>,
        attributes: Option<HashMap<String, i32>>,
        effects: Vec<effect::EffectInfo>,
        hp: Option<i32>,
        stamina: Option<i32>,
        thirst: String,
        hunger: String,
        tiredness: String,
        base_hp: Option<i32>,
        base_stamina: Option<i32>,
        base_def: Option<i32>,
        base_vision: Option<u32>,
        base_speed: Option<i32>,
        base_dmg: Option<i32>,
        dmg_range: Option<i32>,
        total_dmg: Option<f32>,
        total_def: Option<f32>,
        vision: Option<u32>,
    },
    #[serde(rename = "info_villager")]
    InfoVillager {
        id: i32,
        name: String,
        class: String,
        subclass: String,
        template: String,
        state: String,
        image: String,
        hsl: Vec<i32>,
        items: Option<Vec<Item>>,
        skills: Option<HashMap<String, i32>>,
        attributes: Option<HashMap<String, i32>>,
        effects: Option<Vec<String>>,
        need: String,
        thirst: String,
        hunger: String,
        tiredness: String,
        hp: Option<i32>,
        stamina: Option<i32>,
        base_hp: Option<i32>,
        base_stamina: Option<i32>,
        base_def: Option<i32>,
        base_vision: Option<u32>,
        base_speed: Option<i32>,
        base_dmg: Option<i32>,
        dmg_range: Option<i32>,
        vision: Option<u32>,
        structure: Option<String>,
        activity: Option<String>,
        shelter: Option<String>,
        morale: Option<String>,
        order: Option<String>,
        capacity: Option<i32>,
        total_weight: Option<i32>,
    },
    #[serde(rename = "info_structure")]
    InfoStructure {
        id: i32,
        name: String,
        class: String,
        subclass: String,
        template: String,
        x: i32,
        y: i32,
        state: String,
        image: String,
        hsl: Vec<i32>,
        items: Option<Vec<Item>>,
        hp: Option<i32>,
        base_hp: Option<i32>,
        base_def: Option<i32>,
        capacity: Option<i32>,
        total_weight: Option<i32>,
        workspaces: Option<i32>,
        max_residents: Option<i32>,
        residents: Option<i32>,
        effects: Option<Vec<String>>,
        build_cost: Option<f32>,
        upgrade_cost: Option<f32>,
        work_done: Option<f32>,
        work_per_sec: Option<f32>,
        req: Option<Vec<ResReq>>,
        upgrade_req: Option<Vec<ResReq>>,
        selected_upgrade: Option<String>,
        crop_type: Option<String>,
        crop_quantity: Option<i32>,
        crop_stage: Option<String>,
    },
    #[serde(rename = "info_npc")]
    InfoNPC {
        id: i32,
        name: String,
        class: String,
        subclass: String,
        template: String,
        state: String,
        image: String,
        hsl: Vec<i32>,
        items: Option<Vec<Item>>,
        effects: Vec<String>,
    },
    #[serde(rename = "info_monolith")]
    InfoMonolith {
        id: i32,
        name: String,
        class: String,
        subclass: String,
        template: String,
        image: String,
        soulshards: i32,
    },
    #[serde(rename = "info_poi")]
    InfoPOI {
        id: i32,
        name: String,
        class: String,
        subclass: String,
        template: String,
        image: String,
        items: Option<Vec<Item>>,
    },
    #[serde(rename = "info_obj")]
    InfoObj {
        id: i32,
        name: String,
        class: String,
        subclass: String,
        template: String,
        image: String,
    },
    #[serde(rename = "info_skills")]
    InfoSkills {
        id: i32,
        skills: HashMap<String, Skill>,
    },
    #[serde(rename = "info_attrs")]
    InfoAttrs {
        id: i32,
        attrs: HashMap<String, i32>,
    },
    #[serde(rename = "info_advance")]
    InfoAdvance {
        id: i32,
        rank: String,
        next_rank: String,
        total_xp: i32,
        req_xp: i32,
    },
    #[serde(rename = "info_upgrade")]
    InfoUpgrade {
        id: i32,
        upgrade_list: Vec<UpgradeTemplate>,
    },
    #[serde(rename = "info_tile")]
    InfoTile {
        x: i32,
        y: i32,
        name: String,
        mc: i32,
        def: f32,
        unrevealed: i32,
        sanctuary: String,
        passable: bool,
        wildness: String,
        resources: Vec<TileResource>,
        terrain_features: Vec<TileTerrainFeature>,
    },
    #[serde(rename = "info_tile_resources")]
    InfoTileResources {
        x: i32,
        y: i32,
        name: String,
        resources: Vec<TileResource>,
    },
    #[serde(rename = "info_inventory")]
    InfoInventory {
        id: i32,
        cap: i32,
        tw: i32,
        items: Vec<Item>,
    },
    #[serde(rename = "info_inventory_snapshot")]
    InfoInventorySnapshot {
        id: i32,
        cap: i32,
        tw: i32,
        items: Vec<Item>,
    },
    #[serde(rename = "info_equip")]
    InfoEquip {
        name: String,
        template: String,
        id: i32,
        cap: i32,
        tw: i32,
        items: Vec<Item>,
    },
    #[serde(rename = "info_item")]
    InfoItem {
        id: i32,
        owner: i32,
        name: String,
        quantity: i32,
        durability: Option<i32>,
        class: String,
        subclass: String,
        image: String,
        weight: f32,
        equipped: bool,
        price: Option<i32>,
        attrs: Option<HashMap<item::AttrKey, item::AttrVal>>,
        produces: Option<Vec<String>>,
    },
    #[serde(rename = "info_item_transfer")]
    InfoItemTransfer {
        source_id: i32,
        sourceitems: Inventory,
        target_id: i32,
        targetitems: Inventory,
        reqitems: Vec<ResReq>,
    },
    #[serde(rename = "info_items_update")]
    InfoItemsUpdate {
        id: i32,
        items_updated: Vec<Item>,
        items_removed: Vec<i32>,
    },
    #[serde(rename = "info_state_update")]
    InfoStateUpdate {
        id: i32,
        state: String,
    },
    #[serde(rename = "info_activity_update")]
    InfoActivityUpdate {
        id: i32,
        activity: String,
    },
    #[serde(rename = "info_needs_update")]
    InfoNeedsUpdate {
        id: i32,
        thirst: String,
        hunger: String,
        tiredness: String,
    },
    #[serde(rename = "info_merchant")]
    InfoMerchant {
        source_id: i32,
        inventory: Inventory,
        merchant_id: i32,
        merchant_inventory: Inventory,
        merchant_wanted_items: Vec<WantedItem>,
    },
    #[serde(rename = "info_hire")]
    InfoHire {
        data: Vec<HireData>,
    },
    #[serde(rename = "item_transfer")]
    ItemTransfer {
        result: String,
        source_id: i32,
        sourceitems: Inventory,
        target_id: i32,
        targetitems: Inventory,
        reqitems: Vec<ResReq>,
    },
    #[serde(rename = "item_split")]
    ItemSplit {
        result: String,
        owner: i32,
    },
    #[serde(rename = "info_experiment")]
    InfoExperiment {
        id: i32,
        expitem: Vec<Item>,
        expresources: Vec<Item>,
        validresources: Vec<Item>,
        expstate: String,
        recipe: Option<Recipe>,
    },
    #[serde(rename = "info_experiment_state")]
    InfoExperimentState {
        id: i32,
        expstate: String,
    },
    #[serde(rename = "info_crop")]
    InfoCrop {
        id: i32,
        crop_type: String,
        crop_quantity: i32,
        crop_stage: String,
    },
    #[serde(rename = "nearby_resources")]
    NearbyResources {
        data: Vec<TileResourceWithPos>,
    },
    #[serde(rename = "structure_list")]
    StructureList(StructureList),
    #[serde(rename = "image_def")]
    ImageDef {
        name: String,
        data: serde_json::Value,
    },
    PlayerMoved {
        player_id: i32,
        x: i32,
        y: i32,
    },
    #[serde(rename = "create_foundation")]
    CreateFoundation {
        result: String,
    },
    #[serde(rename = "start_upgrade")]
    StartUpgrade {
        structure_id: i32,
    },
    #[serde(rename = "build")]
    Build {
        build_time: i32,
    },
    #[serde(rename = "upgrade")]
    Upgrade {
        upgrade_time: i32,
    },
    #[serde(rename = "work_update")]
    WorkUpdate {
        structure_id: i32,
        work_done: f32,
        total_work: f32,
        work_per_sec: f32,
    },
    #[serde(rename = "craft")]
    Craft {
        craft_time: i32,
    },
    #[serde(rename = "refine")]
    Refine {
        refine_time: i32,
    },
    #[serde(rename = "explore")]
    Explore {
        explore_time: i32,
    },
    #[serde(rename = "gather")]
    Gather {
        gather_time: i32,
    },
    #[serde(rename = "attack")]
    Attack {
        source_id: i32,
        attack_type: String,
        cooldown: i32,
        stamina_cost: i32,
    },
    #[serde(rename = "info_assign")]
    InfoAssign {
        structure_id: i32,
        assignments: Vec<Assignment>,        
    },
    #[serde(rename = "assign")]
    Assign {
        result: String,
    },
    #[serde(rename = "equip")]
    Equip {
        result: String,
    },
    #[serde(rename = "info_craft")]
    InfoCraft {
        crafter_id: i32,
        structure_id: Option<i32>,
        items: Vec<Item>,
        recipes: Vec<Recipe>,
        crafting_item: Option<CraftingItem>,
    },
    #[serde(rename = "info_structure_craft")]
    InfoStructureCraft {
        structure_inventory: Inventory,
        recipes: Option<Vec<Recipe>>,
        queue: Vec<WorkEntry>,
        crafting_item: Option<CraftingItem>,
    },
    #[serde(rename = "info_structure_queue")]
    InfoStructureQueue {
        structure_id: i32,
        queue: Vec<WorkEntry>,
    },
    #[serde(rename = "info_work_queue_entry")]
    InfoWorkQueueEntry {
        structure_id: i32,
        work_type: String,
        index: i32,
        worker_id: i32,
        item_name: String,
        item_image: String,
        item_quantity: i32,
        work_time: i32,
        progress: i32,
    },
    #[serde(rename = "info_refine")]
    InfoRefine {
        refiner_id: i32,
        structure_id: Option<i32>,
        refiner_items: Vec<Item>,
        structure_items: Option<Vec<Item>>,
        refining_item: Option<RefiningItem>,
        produced_items: Vec<(i32, i32)>,
    },
    #[serde(rename = "info_structure_refine")]
    InfoStructureRefine {
        structure_inventory: Inventory,
        refining_item: Option<RefiningItem>,
        produced_items: Vec<(i32, i32)>,
    },
    #[serde(rename = "info_refine_item")]
    InfoRefineItem {
        id: i32,
        name: String,
        image: String,
        class: String,
        subclass: String,
        quantity: i32,
        produces: Vec<ProducedItem>,
        refining_skill: String,
        refining_skill_req: i32,
        refine_time: i32,
        progress: i32,
    },
    #[serde(rename = "xp")]
    Xp {
        id: i32,
        xp_list: Vec<Xp>,
    },
    #[serde(rename = "new_items")]
    NewItems {
        action: String,
        source_id: i32,
        item_name: String,
        amount: i32,
    },
    #[serde(rename = "buy_item")]
    BuyItem {
        source_id: i32,
        inventory: Inventory,
        merchant_id: i32,
        merchant_inventory: Inventory,
    },
    #[serde(rename = "sell_item")]
    SellItem {
        source_id: i32,
        inventory: Inventory,
        merchant_id: i32,
        merchant_inventory: Inventory,
        merchant_wanted_items: Vec<WantedItem>,
    },
    #[serde(rename = "gained_effect")]
    GainedEffect {
        id: i32,
        x: i32,
        y: i32,
        effect: String,
    },
    #[serde(rename = "lost_effect")]
    LostEffect {
        id: i32,
        x: i32,
        y: i32,
        effect: String,
    },
    #[serde(rename = "reduced_effect")]
    ReducedEffect {
        id: i32,
        x: i32,
        y: i32,
        label: String,
        effect: String,
    },
    #[serde(rename = "increased_effect")]
    IncreasedEffect {
        id: i32,
        x: i32,
        y: i32,
        label: String,
        effect: String,
    },
    Ok,
    None,
    Pong,
    Error {
        errmsg: String,
    },
    Notice {
        noticemsg: String,
        expiry: Option<i32>,
    },
    #[serde(rename = "info_true_death")]
    InfoTrueDeath {
        hero_name: String,
        hero_rank: String,
        total_xp: i32,
        fate: String,
    },
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct PerceptionData {
    pub map: Vec<MapTile>,
    pub observers: Vec<MapObj>,
    pub visible_objs: Vec<MapObj>,
    pub weather: Vec<MapWeather>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ChangeEvents {
    ObjCreate {
        event: String,
        obj: MapObj,
    },
    ObjUpdate {
        event: String,
        obj_id: i32,
        attrs: Vec<ObjAttr>,
    },
    ObjMove {
        event: String,
        obj: MapObj,
        src_x: i32,
        src_y: i32,
    },
    ObjDelete {
        event: String,
        obj_id: i32,
    },
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct StatsData {
    pub id: i32,
    pub hp: i32,
    pub base_hp: i32,
    pub stamina: i32,
    pub base_stamina: i32,
    pub thirst: Option<String>,
    pub hunger: Option<String>,
    pub tiredness: Option<String>,
    pub effects: Vec<i32>,
}

#[skip_serializing_none]
#[derive(Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(tag = "packet")]
pub enum BroadcastEvents {
    #[serde(rename = "dmg")]
    Damage {
        source_id: i32,
        target_id: i32,
        attack_type: String,
        dmg: i32,
        state: String,
        combo: Option<String>,
        countered: Option<String>,
    },
    #[serde(rename = "spoil")]
    Spoil {
        source_id: i32,
        target_id: i32,
        itemtype: String,
        itemquantity: i32,
    },
    #[serde(rename = "steal")]
    Steal { source_id: i32, target_id: i32 },
    #[serde(rename = "torch")]
    Torch { source_id: i32, target_id: i32 },
    #[serde(rename = "speech")]
    Speech { source: i32, speech: String },
    #[serde(rename = "sound")]
    Sound { x: i32, y: i32, sound: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, Hash, PartialEq)]
pub struct MapObj {
    pub id: i32,
    pub player: i32,
    pub name: String,
    pub class: String,
    pub subclass: String,
    pub template: String,
    pub image: String,
    pub x: i32,
    pub y: i32,
    pub state: String,
    pub vision: Option<u32>,
    pub hsl: Vec<i32>,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MapWeather {
    pub x: i32,
    pub y: i32,
    pub weather: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct ObjAttr {
    pub attr: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Inventory {
    pub id: i32,
    pub cap: i32,
    pub tw: i32,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Item {
    pub id: i32,
    pub name: String,
    pub quantity: i32,
    pub durability: Option<i32>,
    pub owner: i32,
    pub class: String,
    pub subclass: String,
    pub slot: Option<String>,
    pub image: String,
    pub weight: f32,
    pub equipped: bool,
    pub refineable: bool,
    pub attrs: Option<HashMap<item::AttrKey, item::AttrVal>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CraftingItem {
    pub name: String,
    pub image: String,
    pub class: String,
    pub subclass: String,
    pub crafting_time: i32,
    pub progress: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct RefiningItem {
    pub id: i32,
    pub name: String,
    pub image: String,
    pub class: String,
    pub subclass: String,
    pub quantity: i32,
    pub produces: Vec<ProducedItem>,
    pub refining_skill: String,
    pub refine_time: i32,
    pub progress: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ProducedItem {
    pub name: String,
    pub image: String,
    pub class: String,
    pub subclass: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Structure {
    pub name: String,
    pub image: String,
    pub class: String,
    pub subclass: String,
    pub template: String,
    pub base_hp: i32,
    pub base_def: i32,
    pub build_time: i32,
    pub req: Vec<ResReq>,
    pub upgrade_req: Vec<ResReq>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Assignment {
    pub id: i32,
    pub name: String,
    pub image: String,
    pub structure_id: i32,
    pub structure_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Recipe {
    pub name: String,
    pub image: String,
    pub class: String,
    pub subclass: String,
    pub tier: Option<i32>,
    pub slot: Option<String>,
    pub damage: Option<i32>,
    pub speed: Option<f32>,
    pub armor: Option<i32>,
    pub stamina_req: Option<i32>,
    pub crafting_time: Option<i32>,
    pub skill_req: Option<i32>,
    pub weight: f32,
    pub amount: Option<i32>,
    pub req: Vec<ResReq>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WorkEntry {
    pub work_type: String,
    pub work_status: String,
    pub villager_id: i32,
    pub recipe_name: Option<String>,
    pub recipe_image: Option<String>,
    pub refine_item_id: Option<i32>,
    pub refine_item_image: Option<String>,
    pub refine_item_class: Option<String>,
    pub work_time: i32,
    pub progress: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Skill {
    pub level: i32,
    pub xp: i32,
    pub next: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Xp {
    pub skill: String,
    pub xp: i32,
    pub levelup: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TileResource {
    pub name: String,
    pub image: String,
    pub color: i32,
    pub yield_label: String,
    pub quantity_label: String,
    pub properties: Vec<Property>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TileTerrainFeature {
    pub name: String,
    pub image: String,
    pub bonus: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TileResourceWithPos {
    pub name: String,
    pub color: i32,
    pub yield_label: String,
    pub quantity_label: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct HireData {
    pub id: i32,
    pub name: String,
    pub image: String,
    pub wage: i32,
    pub creativity: i32,
    pub dexterity: i32,
    pub endurance: i32,
    pub focus: i32,
    pub intellect: i32,
    pub spirit: i32,
    pub strength: i32,
    pub toughness: i32,
    pub skills: HashMap<String, i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct UpgradeTemplate {
    pub name: String,
    pub template: String,
    pub req: Vec<ResReq>,
    pub build_time: i32,
}

#[derive(Debug, Clone)]
pub struct ActiveStream {
    pub player_id: i32,
    pub client_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct Stream {
    pub player_id: i32,
    pub client_id: Uuid,
    pub sender: tokio::sync::mpsc::Sender<String>,
}

#[derive(Debug, Clone)]
pub struct Streams(Arc<Mutex<HashMap<Uuid, Stream>>>);

pub fn send_to_client(player_id: i32, packet: ResponsePacket, clients: &Res<Clients>) {
    for (_client_id, client) in clients.lock().unwrap().iter() {
        if client.player_id == player_id {
            match client
                .sender
                .try_send(serde_json::to_string(&packet).unwrap())
            {
                Ok(_) => (),
                //TODO potentially remove client from client as the client is closed
                Err(e) => println!("Error sending to client: {:?}", e),
            }
        }
    }
}

pub fn send_to_database(database_event: DatabaseEvent, database_managers: &Res<DatabaseManagers>) {
    let binding = database_managers.lock().unwrap();
    let database_client = binding.get(&DATABASE_MANAGER_ID).unwrap();

    match database_client.sender.try_send(database_event) {
        Ok(_) => (),
        Err(e) => println!("Error sending to db: {:?}", e),
    }
}

pub fn create_network_obj(obj: &ObjQueryItem<'_, '_>) -> MapObj {
    let network_obj = MapObj {
        id: obj.id.0,
        player: obj.player_id.0,
        x: obj.pos.x,
        y: obj.pos.y,
        name: obj.name.0.clone(),
        template: obj.template.0.clone(),
        class: obj.class.0.clone(),
        subclass: obj.subclass.to_string(),
        state: obj.state.to_string(),
        vision: None,
        image: obj.misc.image.clone(),
        hsl: obj.misc.hsl.clone(),
        groups: obj.misc.groups.clone(),
    };

    network_obj
}

pub fn network_obj(
    id: i32,
    player_id: i32,
    x: i32,
    y: i32,
    name: String,
    template: String,
    class: String,
    subclass: String,
    state: String,
    image: String,
    hsl: Vec<i32>,
    groups: Vec<String>,
) -> MapObj {
    let network_obj = MapObj {
        id: id,
        player: player_id,
        x: x,
        y: y,
        name: name,
        template: template,
        class: class,
        subclass: subclass,
        state: state,
        vision: None,
        image: image,
        hsl: hsl,
        groups: groups,
    };

    network_obj
}

pub fn to_map_obj(obj: ObjQueryItem<'_, '_>) -> MapObj {
    let network_obj = MapObj {
        id: obj.id.0,
        player: obj.player_id.0,
        x: obj.pos.x,
        y: obj.pos.y,
        name: obj.name.0.clone(),
        template: obj.template.0.clone(),
        class: obj.class.0.clone(),
        subclass: obj.subclass.to_string(),
        state: obj.state.to_string(),
        vision: None,
        image: obj.misc.image.clone(),
        hsl: obj.misc.hsl.clone(),
        groups: obj.misc.groups.clone(),
    };

    network_obj
}

pub fn to_map_without_vision(obj: ObjQueryMutReadOnlyItem<'_, '_>) -> MapObj {
    let network_obj = MapObj {
        id: obj.id.0,
        player: obj.player_id.0,
        x: obj.pos.x,
        y: obj.pos.y,
        name: obj.name.0.clone(),
        template: obj.template.0.clone(),
        class: obj.class.0.clone(),
        subclass: obj.subclass.to_string(),
        state: obj.state.to_string(),
        vision: Some(obj.viewshed.range),
        image: obj.misc.image.clone(),
        hsl: obj.misc.hsl.clone(),
        groups: obj.misc.groups.clone(),
    };

    network_obj
}

lazy_static! {
    static ref TILESET: HashMap<String, serde_json::Value> = {
        println!("Loading tilesets");
        let mut tileset = HashMap::new();

        // Load tilesets
        for entry in glob("./tileset/*.json").expect("Failed to read glob pattern") {
            println!("Entry: {:?}", entry);
          match entry {
              Ok(path) => {
                let path = Path::new(&path);
                let file_stem = path.file_stem();
                let data = fs::read_to_string(&path).expect("Unable to read file");
                let json: serde_json::Value = serde_json::from_str(&data).expect("JSON does not have correct format.");
                let file_stem = file_stem.unwrap().to_str().unwrap().to_string();
                println!("Loading tileset: {:?}", file_stem);
                tileset.insert(file_stem, json);
              },
              Err(e) => println!("{:?}", e),
          }
        }

        tileset
    };
}

fn load_certs(filename: &Path) -> Vec<CertificateDer<'static>> {
    CertificateDer::pem_file_iter(filename)
        .expect("cannot open certificate file")
        .map(|result| result.unwrap())
        .collect()
}

fn load_private_key(filename: &Path) -> PrivateKeyDer<'static> {
    PrivateKeyDer::from_pem_file(filename).expect("cannot read private key file")
}

pub async fn tokio_setup(
    database_to_game_sender: CBSender<DatabaseEvent>,
    database_managers: DatabaseManagers,
    client_to_game_sender: CBSender<PlayerEvent>,
    clients: Clients,
    reset_game: bool,
) {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    let streams = Streams(Arc::new(Mutex::new(HashMap::new())));

    let (stream_to_manager_sender, mut stream_to_manager_receiver) =
        tokio::sync::mpsc::channel::<ActiveStream>(100);

    let streams_clone = streams.clone();
    tokio::spawn(async move {
        while let Some(message) = stream_to_manager_receiver.recv().await {
            println!("Received message from stream: {:?}", message);

            let active_client_id = message.client_id;

            // Terminate other streams for the same player id
            let streams_to_terminate: Vec<_> = {
                let streams_lock = streams_clone.0.lock().unwrap();

                println!("Streams lock: {:?}", streams_lock);
                streams_lock
                    .iter()
                    .filter(|(_client_id, stream)| {
                        stream.player_id == message.player_id
                            && stream.client_id != active_client_id
                    })
                    .map(|(_client_id, stream)| stream.sender.clone())
                    .collect()
            };

            for sender in streams_to_terminate {
                let _ = sender
                    .send("terminate".to_owned())
                    .await
                    .map_err(|e| println!("Error sending terminate to stream: {:?}", e));
            }
        }
    });

    // Configure the connection pool.
    let mut pg_config = Config::new();
    pg_config.host(&env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string()));
    pg_config.user(&env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string()));
    pg_config.password(&env::var("DB_PASSWORD").expect("DB_PASSWORD must be set"));
    pg_config.dbname(&env::var("DB_NAME").unwrap_or_else(|_| "perilous".to_string()));

    let manager = Manager::new(pg_config, NoTls);
    let pool = Pool::builder(manager).max_size(16).build().unwrap();

    println!("Resetting game: {:?}", reset_game);
    if reset_game {
        let client = pool
            .get()
            .await
            .expect("Error getting DB connection from pool");

        let statement = client
            .prepare("UPDATE accounts set player_state = $1")
            .await
            .expect("Error preparing statement");

        client
            .execute(&statement, &[&CREATING_HERO])
            .await
            .expect("Error executing statement");
    }

    let (game_to_database_sender, mut game_to_database_receiver) = tokio::sync::mpsc::channel(100);

    //Store the incremented client id and the game to client sender in the clients hashmap
    database_managers.lock().unwrap().insert(
        DATABASE_MANAGER_ID,
        DatabaseClient {
            sender: game_to_database_sender,
        },
    );

    //Spawn a thread to receive messages from the game to client receiver
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        while let Some(event) = game_to_database_receiver.recv().await {
            println!("GOT = {:?}", event);

            match event {
                DatabaseEvent::AddScore {
                    player_id,
                    hero_name,
                    hero_rank,
                    total_xp,
                    fate,
                } => {
                    let client = pool_clone
                        .get()
                        .await
                        .expect("Error getting DB connection from pool");

                    let statement = client
                        .prepare("INSERT INTO scores (player_id, hero_name, hero_rank, total_xp, fate) VALUES ($1, $2, $3, $4, $5)")
                        .await
                        .expect("Error preparing statement");

                    client
                        .execute(
                            &statement,
                            &[&player_id, &hero_name, &hero_rank, &total_xp, &fate],
                        )
                        .await
                        .expect("Error executing statement");

                    let statement = client
                        .prepare("UPDATE accounts set player_state = $1 where player_id = $2")
                        .await
                        .expect("Error preparing statement");

                    client
                        .execute(&statement, &[&HERO_DEAD, &player_id])
                        .await
                        .expect("Error executing statement");
                }
            }
        }
    });

    // Get address from environment variable
    let addr = env::var("ADDRESS").expect("ADDRESS must be set");

    // Load the certificate and private key from PEM files
    let cert_file = env::var("PUBLIC_CERT_PATH").expect("PUBLIC_CERT_PATH must be set");
    let key_file = env::var("PRIVATE_KEY_PATH").expect("PRIVATE_KEY_PATH must be set");

    let cert_path = Path::new(&cert_file);
    let key_path = Path::new(&key_file);

    let certs = load_certs(cert_path);
    let key = load_private_key(key_path);

    // Create the TLS server configuration
    let tls_config = ServerConfig::builder()
        .with_no_client_auth() // You can change this to use client authentication if required
        .with_single_cert(certs, key)
        .expect("Failed to create ServerConfig");

    let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    println!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        println!("Peer address: {}", peer);

        let tls_acceptor = tls_acceptor.clone();
        let client_to_game_sender = client_to_game_sender.clone();
        let clients = clients.clone();
        let streams = streams.clone();
        let pool = pool.clone();
        let stream_to_manager_sender = stream_to_manager_sender.clone();

        tokio::spawn(async move {
            match tls_acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    accept_connection(
                        peer,
                        tls_stream,
                        client_to_game_sender,
                        clients,
                        streams,
                        pool,
                        stream_to_manager_sender,
                    )
                    .await;
                }
                Err(e) => {
                    eprintln!("Failed to establish TLS connection: {}", e);
                }
            }
        });
    }

    println!("Finished");
}

async fn accept_connection(
    peer: SocketAddr,
    stream: TlsStream<TcpStream>,
    client_to_game_sender: CBSender<PlayerEvent>,
    clients: Clients,
    streams: Streams,
    pool: Pool,
    stream_to_manager_sender: tokio::sync::mpsc::Sender<ActiveStream>,
) {
    if let Err((client_id, e)) = handle_connection(
        peer,
        stream,
        client_to_game_sender,
        clients.clone(),
        streams.clone(),
        pool,
        stream_to_manager_sender,
    )
    .await
    {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8(_) => {
                println!(
                    "Connection closed - removing client: {:?} {:?}",
                    client_id, e
                );

                clients.lock().unwrap().remove(&client_id);
                streams.0.lock().unwrap().remove(&client_id);
            }
            err => {
                println!(
                    "Error processing connection - removing client: {:?} {:?}",
                    client_id, err
                );
                clients.lock().unwrap().remove(&client_id);
                streams.0.lock().unwrap().remove(&client_id);
            }
        }
    }
}

async fn handle_connection(
    peer: SocketAddr,
    stream: TlsStream<TcpStream>,
    client_to_game_sender: CBSender<PlayerEvent>,
    clients: Clients,
    streams: Streams,
    pool: Pool,
    stream_to_manager_sender: tokio::sync::mpsc::Sender<ActiveStream>,
) -> Result<(), (Uuid, Error)> {
    //Get the number of clients for a client id
    //let num_clients = clients.lock().unwrap().keys().len() as i32;

    //Client ID
    let client_id = Uuid::new_v4();

    println!(
        "New client id: {:?} for WebSocket connection: {}",
        client_id, peer
    );

    // Get peer address
    let peer: SocketAddr = stream.get_ref().0.peer_addr().unwrap();
    let peer_ip = peer.ip();

    println!("Peer address: {:?}", peer_ip);

    // Get server address from env
    let env_addr = env::var("ADDRESS").unwrap();
    let server: SocketAddr = env_addr.parse().unwrap();
    let server_ip = server.ip();

    println!("Server address: {:?}", server_ip);

    // Shared session ID state
    let mut session_id: Option<String> = None;
    let mut health_check: bool = false;

    let callback = |req: &Request, response: Response| {
        let headers = req.headers();

        // Look for x-health-check header
        if let Some(_) = headers.get("x-health-check") {
            health_check = true;
            Ok(response)
        } else {
            // Check if cookie is in headers
            let Some(cookie) = headers.get("cookie") else {
                // Return error
                println!("No cookie found");
                let resp = Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(Some("Access denied".into()))
                    .unwrap();
                return Err(resp);
            };

            let cookie_str = cookie.to_str().unwrap();

            // Split the string by ';' to separate the key-value pairs
            let pairs: Vec<&str> = cookie_str.split(";").map(|s| s.trim()).collect();

            let mut parsed: HashMap<&str, &str> = HashMap::new();

            for pair in pairs {
                if let Some((key, value)) = pair.split_once('=') {
                    println!("key: {:?} value: {:?}", key, value);
                    parsed.insert(key, value);
                }
            }

            println!("Parsed: {:?}", parsed);
            // Access values by key
            session_id = parsed.get("session").map(|s| s.to_string());

            Ok(response)
        }
    };

    let ws_stream = match accept_hdr_async(stream, callback).await {
        Ok(ws_stream) => ws_stream,
        Err(e) => {
            println!("WebSocket handshake error for client {}: {}", client_id, e);
            return Err((client_id, e));
        }
    };

    if health_check {
        println!("Server health check");
        let (mut ws_sender, _ws_receiver) = ws_stream.split();
        ws_sender
            .send(Message::Text("Pong".into()))
            .await
            .map_err(|e| (client_id, e))?;

        return Ok(());
    }

    let Some(session_id) = session_id else {
        return Err((client_id, Error::AttackAttempt));
    };

    // Print the session id
    println!("session_id: {:?}", session_id);

    let client = pool
        .get()
        .await
        .expect("Error getting connection from pool");

    let row_session = client
        .query_one(
            "SELECT player_id, created_at FROM sessions WHERE session = $1",
            &[&session_id],
        )
        .await;

    let Ok(row_session) = row_session else {
        println!("Session not found");
        return Err((client_id, Error::AttackAttempt));
    };

    // Check if the session is expired
    let created_at: DateTime<Utc> = row_session.get::<_, DateTime<Utc>>("created_at");
    let now = chrono::Utc::now();

    println!("created_at: {:?}", created_at);
    println!("now: {:?}", now);
    if created_at.signed_duration_since(now) > chrono::Duration::days(1) {
        return Err((client_id, Error::AttackAttempt));
    }

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    //Create a tokio sync channel to for messages from the game to each client
    let (game_to_client_sender, mut game_to_client_receiver) = tokio::sync::mpsc::channel(100);

    // Get the player id from the session
    let player_id: i32 = row_session.get("player_id");

    let (manager_to_stream_sender, mut manager_to_stream_receiver) =
        tokio::sync::mpsc::channel::<String>(100);

    //Store the connection in the connections hashmap
    println!("Inserting stream into streams hashmap");
    streams.0.lock().unwrap().insert(
        client_id,
        Stream {
            client_id: client_id,
            player_id: player_id,
            sender: manager_to_stream_sender,
        },
    );

    println!("Streams: {:?}", streams.0.lock().unwrap());

    // Kill the other streams for the same player id
    match stream_to_manager_sender
        .send(ActiveStream {
            player_id: player_id,
            client_id: client_id,
        })
        .await
    {
        Ok(_) => (),
        Err(_e) => return Err((client_id, Error::ConnectionClosed)),
    }

    // Get the player account_name from the accounts table
    let row_account = client
        .query_one(
            "SELECT account_name, player_state FROM accounts WHERE player_id = $1",
            &[&player_id],
        )
        .await;

    let Ok(row_account) = row_account else {
        return Err((client_id, Error::AttackAttempt));
    };

    let player_username: String = row_account.get::<_, Option<String>>("account_name").unwrap_or_default();
    let player_state: String = row_account.get("player_state");

    let row_score = client.query_one("SELECT hero_name, hero_rank, total_xp, fate FROM scores WHERE player_id = $1 ORDER BY created_at DESC LIMIT 1", &[&player_id]).await;

    let mut hero_name: String = String::new();
    let mut hero_rank: String = String::new();
    let mut total_xp: i32 = 0;
    let mut fate: String = String::new();

    if let Ok(row_score) = row_score {
        hero_name = row_score.get("hero_name");
        hero_rank = row_score.get("hero_rank");
        total_xp = row_score.get("total_xp");
        fate = row_score.get("fate");
    };

    //Store the incremented client id and the game to client sender in the clients hashmap
    println!("Inserting client into clients hashmap");
    clients.lock().unwrap().insert(
        client_id,
        Client {
            id: client_id,
            player_id: player_id,
            sender: game_to_client_sender,
        },
    );

    println!("Clients: {:?}", clients.lock().unwrap());

    println!("player_state: {:?}", player_state);

    let packet = match player_state.as_str() {
        CREATING_HERO => {
            println!("Processing CREATING_HERO");
            ResponsePacket::SelectClass {
                player: player_id as u32,
            }
        }
        PLAYING => {
            println!("Processing PLAYING");
            //Send login to player
            client_to_game_sender
                .send(PlayerEvent::Login {
                    player_id: player_id,
                })
                .expect("Could not send message");

            ResponsePacket::Login {
                player: player_id as u32,
            }
        }
        HERO_DEAD => {
            println!("Processing HERO_DEAD");
            ResponsePacket::InfoTrueDeath {
                hero_name: hero_name.clone(),
                hero_rank: hero_rank.clone(),
                total_xp: total_xp,
                fate: fate.clone(),
            }
        }
        _ => {
            println!("Processing UNKNOWN");
            ResponsePacket::Error {
                errmsg: "Unknown player state".to_owned(),
            }
        }
    };

    let res = serde_json::to_string(&packet).unwrap();

    ws_sender
        .send(Message::Text(res.into()))
        .await
        .map_err(|e| (client_id, e))?;

    //This loop uses the tokio select! macro to receive messages from either the websocket receiver
    //or the game to client receiver
    loop {
        tokio::select! {
            //Receive messages from the websocket
            msg = ws_receiver.next() => {
                match msg {
                    Some(msg) => {
                        let msg = match msg {
                            Ok(msg) => msg,
                            Err(e) => return Err((client_id, e)),
                        };
                        if msg.is_text() || msg.is_binary() {

                            println!("player_id: {:?}", player_id);

                            //Check if the player is authenticated
                            /*if player_id == -1 {
                                //Attempt to login
                                let res_packet: ResponsePacket = match serde_json::from_str(msg.to_text().unwrap()) {
                                    Ok(packet) => {
                                        match packet {
                                            /*NetworkPacket::Register{account_name, password} => {
                                                let (pid, res) = handle_register(pool.clone(), account_name.clone(), password).await;
                                                player_id = pid;
                                                player_username = account_name;


                                                if let Some(client) = clients.lock().unwrap().get_mut(&client_id) {
                                                    (*client).player_id = player_id;
                                                }

                                                res
                                            }*/
                                            NetworkPacket::Login{account_name, password} => {
                                                println!("{:?}", account_name);
                                                //Retrieve player id, note will be set if authenticated
                                                let (pid, res) = handle_login(pool.clone(), account_name.clone(), password, client_to_game_sender.clone()).await;

                                                //Set player_id
                                                player_id = pid;
                                                player_username = account_name;

                                                println!("player_id: {:?} player_username: {:?}", player_id, player_username);

                                                if let Some(client) = clients.lock().unwrap().get_mut(&client_id) {
                                                    (*client).player_id = player_id;
                                                }

                                                //Return packet
                                                res
                                            }
                                            _ => ResponsePacket::Error{errmsg: "Unknown packet".to_owned()}
                                        }
                                    },

                                    Err(_) => ResponsePacket::Error{errmsg: "Unknown packet".to_owned()}
                                };
                                println!("{:?}", res_packet);
                                //TODO send event to game
                                //client_to_game_sender.send(Message::text(res)).expect("Could not send message");

                                //Send response to client
                                let res = serde_json::to_string(&res_packet).unwrap();
                                if let Err(e) = ws_sender.send(Message::Text(res)).await {
                                    return Err((player_id, e));
                                }
                            } else {*/
                                println!("Authenticated packet: {:?}", msg.to_text().unwrap());

                                let res_packet: ResponsePacket = match serde_json::from_str(msg.to_text().unwrap()) {
                                    Ok(packet) => {
                                        match packet {
                                            NetworkPacket::SelectedClass{class_name, hero_name} => {
                                                handle_selected_class(
                                                    pool.clone(),
                                                    player_id,
                                                    class_name,
                                                    hero_name,
                                                    client_to_game_sender.clone()
                                                ).await
                                            }
                                            NetworkPacket::RecreateHero => {
                                                handle_recreate_hero(pool.clone(), player_id).await
                                            }
                                            NetworkPacket::GetStats{id} => {
                                                handle_get_stats(player_id, id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::ImageDef{name} => {
                                                println!("ImageDef name: {:?}", name);
                                                let mut name_stripped = name.clone();
                                                let raw_name = name;

                                                if name_stripped.chars().last().unwrap().is_numeric() {
                                                    name_stripped.pop();
                                                }

                                                ResponsePacket::ImageDef{
                                                    name: raw_name,
                                                    data: TILESET.get(&name_stripped).unwrap().clone()
                                                }
                                            }
                                            NetworkPacket::Move{x, y} => {
                                                handle_move(player_id, x, y, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Attack{attack_type, source_id, target_id} => {
                                                handle_attack(player_id, attack_type, source_id, target_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Combo{source_id, target_id, combo_type} => {
                                                handle_combo(player_id, source_id, target_id, combo_type, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoObj{id} => {
                                                handle_info_obj(player_id, id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoSkills{id} => {
                                                handle_info_skills(player_id, id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoAttrs{id} => {
                                                handle_info_attrs(player_id, id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoAdvance{source_id} => {
                                                handle_info_advance(player_id, source_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoUpgrade{structure_id} => {
                                                handle_info_upgrade(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoTile{x, y} => {
                                                handle_info_tile(player_id, x, y, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoTileResources{x, y} => {
                                                handle_info_tile_resources(player_id, x, y, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoInventory{id} => {
                                                handle_info_inventory(player_id, id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoEquip{id} => {
                                                handle_info_equip(player_id, id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoItem{obj_id, item_id, action} => {
                                                handle_info_item(player_id, obj_id, item_id, action, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoItemByName{name} => {
                                                handle_info_item_by_name(player_id, name, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoItemTransfer{source_id, target_id} => {
                                                handle_info_item_transfer(player_id, source_id, target_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoExit{id, panel_type} => {
                                                handle_info_exit(player_id, id, panel_type, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoHire{source_id} => {
                                                handle_info_hire(player_id, source_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::ItemTransfer{item, source_id, target_id} => {
                                                handle_item_transfer(player_id, item, source_id, target_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::ItemSplit{owner_id, item, quantity} => {
                                                handle_item_split(player_id, owner_id, item, quantity, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Gather => {
                                                handle_gather(player_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Operate{structure_id} => {
                                                handle_operate(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Plant{structure_id} => {
                                                handle_plant(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Tend{structure_id} => {
                                                handle_tend(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Harvest{structure_id} => {
                                                handle_harvest(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Refine{item_id} => {
                                                handle_refine(player_id, item_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::StructureRefine{structure_id, item_id} => {
                                                handle_structure_refine(player_id, structure_id, item_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Craft{recipe} => {
                                                handle_craft(player_id, recipe, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::StructureCraft{structure_id, recipe} => {
                                                handle_structure_craft(player_id, structure_id, recipe, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderFollow{source_id} => {
                                                handle_order_follow(player_id, source_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderGather{source_id, res_type} => {
                                                handle_order_gather(player_id, source_id, res_type, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::StructureList{} => {
                                                handle_structure_list(player_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::CreateFoundation{source_id, structure} => {
                                                handle_create_foundation(player_id, source_id, structure, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Build{source_id, structure_id} => {
                                                handle_build(player_id, source_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::StartUpgrade{structure_id, selected_upgrade} => {
                                                handle_start_upgrade(player_id, structure_id, selected_upgrade, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Upgrade{source_id, structure_id} => {
                                                handle_upgrade(player_id, source_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Experiment{structure_id} => {
                                                handle_experiment(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Activate{structure_id} => {
                                                handle_activate(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Survey{source_id} => {
                                                handle_survey(player_id, source_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::NearbyResources{} => {
                                                handle_nearby_resources(player_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Explore{} => {
                                                handle_explore(player_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoAssign{structure_id} => {
                                                handle_info_assign(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Assign{worker_id, structure_id} => {
                                                handle_assign(player_id, worker_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::RemoveAssign{worker_id, structure_id} => {
                                                handle_remove_assign(player_id, worker_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Equip{obj_id, item, status} => {
                                                handle_equip(player_id, obj_id, item, status, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Sleep{structure_id} => {
                                                handle_sleep(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::DeleteItem{obj_id, item_id} => {
                                                handle_delete_item(player_id, obj_id, item_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoCraft{crafter_id} => {
                                                handle_info_craft(player_id, crafter_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoStructureCraft{structure_id} => {
                                                handle_info_structure_craft(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoStructureQueue{structure_id} => {
                                                handle_info_structure_queue(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoWorkQueueEntry{structure_id, index} => {
                                                handle_info_work_queue_entry(player_id, structure_id, index, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::AddCraftingEntry{structure_id, recipe_name} => {
                                                handle_add_crafting_entry(player_id, structure_id, recipe_name, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::AddRefineEntry{structure_id, refine_item_id} => {
                                                handle_add_refine_entry(player_id, structure_id, refine_item_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::RemoveWorkEntry{structure_id, index} => {
                                                handle_remove_work_entry(player_id, structure_id, index, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoRefine{refiner_id} => {
                                                handle_info_refine(player_id, refiner_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoStructureRefine{structure_id} => {
                                                handle_info_structure_refine(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoStructureRefineItem{structure_id, item_id} => {
                                                handle_info_structure_refine_item(player_id, structure_id, item_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderOperate{source_id, structure_id} => {
                                                handle_order_operate(player_id, source_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderRefine{source_id, structure_id} => {
                                                handle_order_refine(player_id, source_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderCraft{source_id, structure_id} => {
                                                handle_order_craft(player_id, source_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderExplore{source_id} => {
                                                handle_order_explore(player_id, source_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderExperiment{source_id, structure_id} => {
                                                handle_order_experiment(player_id, source_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderPlant{source_id, structure_id} => {
                                                handle_order_plant(player_id, source_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderTend{source_id, structure_id} => {
                                                handle_order_tend(player_id, source_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderHarvest{source_id, structure_id} => {
                                                handle_order_harvest(player_id, source_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::OrderRepair{source_id} => {
                                                handle_order_repair(player_id, source_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Use{obj_id, item_id} => {
                                                handle_use(player_id, obj_id, item_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Remove{source_id} => {
                                                handle_remove(player_id, source_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Advance{source_id} => {
                                                handle_advance(player_id, source_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoExperiment{structure_id} => {
                                                handle_info_experiment(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::SetExperimentItem{structure_id, item_id} => {
                                                //Setting experiment source item, is_resource = false
                                                handle_set_experiment_item(player_id, structure_id, item_id, false, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::SetExperimentResource{structure_id, item_id} => {
                                                //Setting experiment resource item, is_resource = true
                                                handle_set_experiment_item(player_id, structure_id, item_id, true, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::ResetExperiment{structure_id} => {
                                                handle_reset_experiment(player_id, structure_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Hire{source_id, target_id} => {
                                                handle_hire(player_id, source_id, target_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::InfoMerchant{source_id, merchant_id} => {
                                                handle_info_merchant(player_id, source_id, merchant_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::BuyItem{seller_id, item_id, quantity} => {
                                                handle_buy_item(player_id, seller_id, item_id, quantity, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::SellItem{item_id, target_id, quantity} => {
                                                handle_sell_item(player_id, item_id, target_id, quantity, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::CancelAction => {
                                                handle_cancel_action(player_id, client_to_game_sender.clone())
                                            }

                                            _ => ResponsePacket::Ok
                                        }
                                    },
                                    Err(packet) => {
                                        let ping = r#"0"#;

                                        if msg.to_text().unwrap() == ping {
                                            ResponsePacket::Pong
                                        } else {
                                            println!("Error packet: {:?}", packet);
                                            ResponsePacket::Error{errmsg: "Unknown packet".to_owned()}
                                        }
                                    }
                                };
                                if res_packet == ResponsePacket::Pong {
                                    ws_sender.send(Message::Text("1".to_string().into())).await.map_err(|e| (client_id, e))?;
                                }
                                else if res_packet != ResponsePacket::None {
                                    let res = serde_json::to_string(&res_packet).unwrap();
                                    ws_sender.send(Message::Text(res.into())).await.map_err(|e| (client_id, e))?;
                                }
                        } else if msg.is_close() {
                            println!("Message is closed for player: {:?}", player_id);
                            handle_disconnect(client_id, clients.clone());
                            break;
                        } else {
                            println!("Unknown network state: {:?}", msg);
                        }
                    }
                    None => {
                        println!("Message is None");
                        break
                    }
                }
            }
            //Receive messages from the game
            game_msg = game_to_client_receiver.recv() => {
                if let Some(game_msg) = game_msg {

                    match serde_json::from_str(game_msg.as_str()) {
                        Ok(ResponsePacket::Disconnect { player, client }) => {
                            println!("Received disconnect from game, closing websocket for player: {:?} client: {:?}", player, client);
                            println!("Stream: {:?}", ws_sender);
                            ws_sender.send(Message::Close(None)).await.map_err(|e| (client_id, e))?;
                            let removed_client = clients.lock().unwrap().remove(&client);
                            // Don't try to close the sender since it doesn't implement the required trait
                            println!("Removed client: {:?}", removed_client);
                            break;
                        }
                        _ => {
                            ws_sender.send(Message::Text(game_msg.into())).await.map_err(|e| (client_id, e))?;
                        }
                    }

                }
            }

            //Receive messages from the manager
            manager_msg = manager_to_stream_receiver.recv() => {
                if let Some(manager_msg) = manager_msg {
                    println!("Received message from manager: {:?}", manager_msg);
                    ws_sender.send(Message::Close(None)).await.map_err(|e| (client_id, e))?;
                    let removed_stream = streams.0.lock().unwrap().remove(&client_id);
                    println!("Removed stream: {:?}", removed_stream);
                }
            }
        }
    }
    Ok(())
}

fn handle_disconnect(client_id: Uuid, clients: Clients) {
    let mut clients = clients.lock().unwrap();
    clients.remove(&client_id);
}

async fn handle_selected_class(
    pool: Pool,
    player_id: i32,
    class_name: String,
    hero_name: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    println!("handle_selected_class: {:?}", player_id);

    // Check if valid class_name
    let selected_class = match class_name.as_str() {
        "Warrior" => HeroClassList::Warrior,
        "Ranger" => HeroClassList::Ranger,
        "Mage" => HeroClassList::Mage,
        _ => HeroClassList::None,
    };

    if selected_class == HeroClassList::None {
        return ResponsePacket::Error {
            errmsg: "Invalid class".to_owned(),
        };
    }

    if hero_name.is_empty() {
        return ResponsePacket::Error {
            errmsg: "Hero name cannot be empty".to_owned(),
        };
    }

    if hero_name.is_inappropriate() {
        return ResponsePacket::Error {
            errmsg: "Hero name is inappropriate".to_owned(),
        };
    }

    let client = pool
        .get()
        .await
        .expect("Error getting connection from pool");

    let statement = client
        .prepare("UPDATE accounts set player_state = $1 where player_id = $2")
        .await
        .expect("Error preparing statement");

    client
        .execute(&statement, &[&PLAYING, &player_id])
        .await
        .expect("Error executing statement");

    //Send new player event to game
    client_to_game_sender
        .send(PlayerEvent::NewPlayer {
            player_id: player_id,
            hero_name: hero_name.clone(),
            class_name: class_name.clone(),
        })
        .expect("Could not send message");

    ResponsePacket::InfoSelectClass {
        result: "success".to_owned(),
    }
}

async fn handle_recreate_hero(pool: Pool, player_id: i32) -> ResponsePacket {
    println!("handle_recreate_hero: {:?}", player_id);

    let client = pool
        .get()
        .await
        .expect("Error getting connection from pool");

    let statement = client
        .prepare("UPDATE accounts set player_state = $1 where player_id = $2")
        .await
        .expect("Error preparing statement");

    client
        .execute(&statement, &[&CREATING_HERO, &player_id])
        .await
        .expect("Error executing statement");

    // Inform the client to go to select a class state
    ResponsePacket::SelectClass {
        player: player_id as u32,
    }
}

fn handle_get_stats(
    player_id: i32,
    id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::GetStats {
            player_id: player_id,
            id: id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_move(
    player_id: i32,
    x: i32,
    y: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Move {
            player_id: player_id,
            x: x,
            y: y,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_attack(
    player_id: i32,
    attack_type: String,
    source_id: i32,
    target_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Attack {
            player_id: player_id,
            attack_type: attack_type,
            source_id: source_id,
            target_id: target_id,
        })
        .expect("Could not send message");

    ResponsePacket::None
}
fn handle_combo(
    player_id: i32,
    source_id: i32,
    target_id: i32,
    combo_type: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Combo {
            player_id: player_id,
            source_id: source_id,
            target_id: target_id,
            combo_type: combo_type,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_info_obj(
    player_id: i32,
    id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoObj {
            player_id: player_id,
            id: id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_skills(
    player_id: i32,
    id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoSkills {
            player_id: player_id,
            id: id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_attrs(
    player_id: i32,
    id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoAttrs {
            player_id: player_id,
            id: id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_advance(
    player_id: i32,
    source_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoAdvance {
            player_id: player_id,
            id: source_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_upgrade(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoUpgrade {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_tile(
    player_id: i32,
    x: i32,
    y: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoTile {
            player_id: player_id,
            x: x,
            y: y,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_tile_resources(
    player_id: i32,
    x: i32,
    y: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoTileResources {
            player_id: player_id,
            x: x,
            y: y,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_inventory(
    player_id: i32,
    id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoInventory {
            player_id: player_id,
            id: id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_equip(
    player_id: i32,
    id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoEquip {
            player_id: player_id,
            id: id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_item(
    player_id: i32,
    obj_id: i32,
    item_id: i32,
    action: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoItem {
            player_id: player_id,
            obj_id: obj_id,
            item_id: item_id,
            action: action,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_item_by_name(
    player_id: i32,
    name: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoItemByName {
            player_id: player_id,
            name: name,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_item_transfer(
    player_id: i32,
    source_id: i32,
    target_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoItemTransfer {
            player_id: player_id,
            source_id: source_id,
            target_id: target_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_exit(
    player_id: i32,
    id: i32,
    panel_type: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoExit {
            player_id: player_id,
            id: id,
            panel_type: panel_type,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_merchant(
    player_id: i32,
    source_id: i32,
    merchant_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoMerchant {
            player_id: player_id,
            source_id: source_id,
            merchant_id: merchant_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_hire(
    player_id: i32,
    source_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoHire {
            player_id: player_id,
            source_id: source_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_item_transfer(
    player_id: i32,
    item: i32,
    source_id: i32,
    target_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::ItemTransfer {
            player_id: player_id,
            item_id: item,
            source_id: source_id,
            target_id: target_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_item_split(
    player_id: i32,
    owner_id: i32,
    item: i32,
    quantity: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::ItemSplit {
            player_id: player_id,
            owner_id: owner_id,
            item_id: item,
            quantity: quantity,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_gather(player_id: i32, client_to_game_sender: CBSender<PlayerEvent>) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Gather {
            player_id: player_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_plant(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Plant {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_tend(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Tend {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_harvest(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Harvest {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_refine(
    player_id: i32,
    item_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Refine {
            player_id: player_id,
            item_id: item_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_structure_refine(
    player_id: i32,
    structure_id: i32,
    item_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::StructureRefine {
            player_id: player_id,
            structure_id: structure_id,
            item_id: item_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_craft(
    player_id: i32,
    recipe: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Craft {
            player_id: player_id,
            recipe_name: recipe,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_structure_craft(
    player_id: i32,
    structure_id: i32,
    recipe: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::StructureCraft {
            player_id: player_id,
            structure_id: structure_id,
            recipe_name: recipe,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_order_follow(
    player_id: i32,
    source_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderFollow {
            player_id: player_id,
            source_id: source_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_gather(
    player_id: i32,
    source_id: i32,
    res_type: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderGather {
            player_id: player_id,
            source_id: source_id,
            res_type: res_type,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_structure_list(
    player_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::StructureList {
            player_id: player_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_create_foundation(
    player_id: i32,
    source_id: i32,
    structure: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::CreateFoundation {
            player_id: player_id,
            source_id: source_id,
            structure_name: structure,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_build(
    player_id: i32,
    source_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Build {
            player_id: player_id,
            builder_id: source_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_start_upgrade(
    player_id: i32,
    structure_id: i32,
    selected_upgrade: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::StartUpgrade {
            player_id: player_id,
            structure_id: structure_id,
            selected_upgrade: selected_upgrade,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_upgrade(
    player_id: i32,
    source_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Upgrade {
            player_id: player_id,
            builder_id: source_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_experiment(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Experiment {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    ResponsePacket::None
}

fn handle_activate(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Activate {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    ResponsePacket::None
}

fn handle_survey(
    player_id: i32,
    source_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Survey {
            player_id: player_id,
            source_id: source_id,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_explore(player_id: i32, client_to_game_sender: CBSender<PlayerEvent>) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Explore {
            player_id: player_id,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_nearby_resources(
    player_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::NearbyResources {
            player_id: player_id,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_info_assign(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoAssign {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_assign(
    player_id: i32,
    worker_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Assign {
            player_id: player_id,
            worker_id: worker_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_remove_assign(
    player_id: i32,
    worker_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::RemoveAssign {
            player_id: player_id,
            worker_id: worker_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_equip(
    player_id: i32,
    obj_id: i32,
    item: i32,
    status: bool,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Equip {
            player_id: player_id,
            obj_id: obj_id,
            item_id: item,
            status: status,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_sleep(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Sleep {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_delete_item(
    player_id: i32,
    obj_id: i32,
    item_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::DeleteItem {
            player_id: player_id,
            obj_id: obj_id,
            item_id: item_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_craft(
    player_id: i32,
    crafter_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoCraft {
            player_id: player_id,
            crafter_id: crafter_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_info_structure_craft(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoStructureCraft {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_info_structure_queue(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoStructureQueue {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_work_queue_entry(
    player_id: i32,
    structure_id: i32,
    index: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoWorkQueueEntry {
            player_id: player_id,
            structure_id: structure_id,
            index: index,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}
fn handle_add_crafting_entry(
    player_id: i32,
    structure_id: i32,
    recipe_name: String,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::AddCraftingEntry {
            player_id: player_id,
            structure_id: structure_id,
            recipe_name: recipe_name,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_add_refine_entry(
    player_id: i32,
    structure_id: i32,
    refine_item_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::AddRefineEntry {
            player_id: player_id,

            structure_id: structure_id,
            refine_item_id: refine_item_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_remove_work_entry(
    player_id: i32,
    structure_id: i32,
    index: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::RemoveWorkEntry {
            player_id: player_id,
            structure_id: structure_id,
            index: index,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_refine(
    player_id: i32,
    refiner_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoRefine {
            player_id: player_id,
            refiner_id: refiner_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_structure_refine(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoStructureRefine {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_info_structure_refine_item(
    player_id: i32,
    structure_id: i32,
    item_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoStructureRefineItem {
            player_id: player_id,
            structure_id: structure_id,
            item_id: item_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_refine(
    player_id: i32,
    villager_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderRefine {
            player_id: player_id,
            villager_id: villager_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_craft(
    player_id: i32,
    villager_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderCraft {
            player_id: player_id,
            villager_id: villager_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_explore(
    player_id: i32,
    source_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderExplore {
            player_id: player_id,
            villager_id: source_id, // source_id should really be renamed to structure_id in the client
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_experiment(
    player_id: i32,
    villager_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderExperiment {
            player_id: player_id,
            villager_id: villager_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_plant(
    player_id: i32,
    villager_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderPlant {
            player_id: player_id,
            villager_id: villager_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_tend(
    player_id: i32,
    villager_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderTend {
            player_id: player_id,
            villager_id: villager_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_harvest(
    player_id: i32,
    villager_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderHarvest {
            player_id: player_id,
            villager_id: villager_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_repair(
    player_id: i32,
    villager_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderRepair {
            player_id: player_id,
            villager_id: villager_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_use(
    player_id: i32,
    obj_id: i32,
    item_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Use {
            player_id: player_id,
            obj_id: obj_id,
            item_id: item_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_remove(
    player_id: i32,
    source_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Remove {
            player_id: player_id,
            structure_id: source_id, // source_id should really be renamed to structure_id in the client
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_advance(
    player_id: i32,
    source_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Advance {
            player_id: player_id,
            id: source_id, // source_id should really be renamed to structure_id in the client
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_info_experiment(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InfoExperinment {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_set_experiment_item(
    player_id: i32,
    structure_id: i32,
    item_id: i32,
    is_resource: bool,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::SetExperimentItem {
            player_id: player_id,
            structure_id: structure_id,
            item_id: item_id,
            is_resource: is_resource,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_reset_experiment(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::ResetExperiment {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_hire(
    player_id: i32,
    source_id: i32,
    target_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Hire {
            player_id: player_id,
            merchant_id: source_id,
            target_id: target_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_buy_item(
    player_id: i32,
    seller_id: i32,
    item_id: i32,
    quantity: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::BuyItem {
            player_id: player_id,
            seller_id: seller_id,
            item_id: item_id,
            quantity: quantity,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_sell_item(
    player_id: i32,
    item_id: i32,
    target_id: i32,
    quantity: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::SellItem {
            player_id: player_id,
            item_id: item_id,
            target_id: target_id,
            quantity: quantity,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_order_operate(
    player_id: i32,
    villager_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderOperate {
            player_id: player_id,
            villager_id: villager_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_operate(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Operate {
            player_id: player_id,
            structure_id: structure_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_cancel_action(
    player_id: i32,
    client_to_game_sender: CBSender<PlayerEvent>,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::CancelAction {
            player_id: player_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_inappropriate() {
        let hero_name = "Fuck";
        assert!(hero_name.is_inappropriate());
    }
}
