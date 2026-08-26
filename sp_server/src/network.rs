use bevy::prelude::Res;
use crossbeam_channel::Sender as CBSender;
use serde_with::skip_serializing_none;
use tokio_tungstenite::WebSocketStream;

use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use lazy_static::lazy_static;
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

use rustls::ServerConfig;
use tokio_rustls::{server::TlsStream, TlsAcceptor};

use deadpool_postgres::{Manager, Pool};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{self, Instant, MissedTickBehavior};
use tokio_postgres::{Config, NoTls};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::{Message, Result};
use tokio_tungstenite::{accept_hdr_async, tungstenite::Error};

use serde::{Deserialize, Serialize};

use chrono::DateTime;
use chrono::Utc;

use crate::constants::{CREATING_HERO, DATABASE_MANAGER_ID, HERO_DEAD, PLAYING, TICKS_PER_SEC};
use crate::database::DatabaseEvent;
use crate::effect;
use crate::game::{
    AuthoritativeDeliveryFailure, CurrentConnectionSendError, DatabaseClient, DatabaseManagers,
};
use crate::map::{Map, MapTile};
use crate::{
    game::{Client, Clients},
    player::PlayerEvent,
};
use crate::{
    game::{ObjQueryItem, ObjQueryMutReadOnlyItem},
    item,
    obj::{is_valid_hero_portrait, HeroClassList},
    obj::{ActionProgress, ActiveTask, BuildUpgradeState},
    resource::Property,
    templates::ResReq,
    trade::WantedItem,
};

use std::env;
use std::fs;
use std::path::Path;

// Macro for conditional network debug logging controlled by NETWORK_DEBUG env var
macro_rules! net_debug {
    ($($arg:tt)*) => {
        if env::var("NETWORK_DEBUG").is_ok() {
            println!($($arg)*);
        }
    };
}

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

use crate::admin_status::AdminStatusState;
use dotenvy::dotenv;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "cmd")]
enum NetworkPacket {
    #[serde(rename = "login")]
    Login {
        account_name: String,
        password: String,
    },
    #[serde(rename = "register")]
    Register {
        account_name: String,
        password: String,
    },
    #[serde(rename = "select_class")]
    SelectedClass {
        class_name: String,
        hero_name: String,
        portrait: String,
    },
    #[serde(rename = "recreate_hero")]
    RecreateHero,
    #[serde(rename = "request_safe_logout")]
    RequestSafeLogout,
    #[serde(rename = "cancel_safe_logout")]
    CancelSafeLogout,
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
    #[serde(rename = "ability")]
    Ability {
        ability_id: String,
        source_id: i32,
        target_id: Option<i32>,
    },
    #[serde(rename = "combo")]
    Combo {
        source_id: i32,
        target_id: i32,
        combo_type: String,
    },
    #[serde(rename = "block")]
    Block {
        source_id: i32,
        #[serde(default)]
        defense: String,
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
    InfoItem {
        obj_id: i32,
        item_id: i32,
        action: String,
    },
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
    ItemTransfer {
        item: i32,
        source_id: i32,
        target_id: i32,
    },
    #[serde(rename = "loot_all")]
    LootAll { source_id: i32, target_id: i32 },
    #[serde(rename = "item_split")]
    ItemSplit {
        owner_id: i32,
        item: i32,
        quantity: i32,
    },
    #[serde(rename = "gather")]
    Gather { res_type: String },
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
    Craft {
        recipe: String,
        #[serde(default)]
        signature_item_id: Option<i32>,
    },
    #[serde(rename = "structure_craft")]
    StructureCraft {
        structure_id: i32,
        recipe: String,
        #[serde(default)]
        signature_item_id: Option<i32>,
    },
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
    #[serde(rename = "order_prospect")]
    OrderProspect { source_id: i32 },
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
    #[serde(rename = "prospect")]
    Prospect {},
    #[serde(rename = "explore")]
    Explore {},
    #[serde(rename = "investigate")]
    Investigate { target_id: i32 },
    #[serde(rename = "nearby_resources")]
    NearbyResources {},
    #[serde(rename = "info_assign")]
    InfoAssign { structure_id: i32 },
    #[serde(rename = "assign")]
    Assign { worker_id: i32, structure_id: i32 },
    #[serde(rename = "remove_assign")]
    RemoveAssign { worker_id: i32, structure_id: i32 },
    #[serde(rename = "equip")]
    Equip {
        obj_id: i32,
        item: i32,
        status: bool,
    },
    #[serde(rename = "delete_item")]
    DeleteItem { obj_id: i32, item_id: i32 },
    #[serde(rename = "info_craft")]
    InfoCraft { crafter_id: i32 },
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
    InfoRefine { refiner_id: i32 },
    #[serde(rename = "info_structure_refine")]
    InfoStructureRefine { structure_id: i32 },
    #[serde(rename = "info_structure_refine_item")]
    InfoStructureRefineItem { structure_id: i32, item_id: i32 },
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
    BuyItem {
        seller_id: i32,
        item_id: i32,
        quantity: i32,
    },
    #[serde(rename = "sell_item")]
    SellItem {
        item_id: i32,
        target_id: i32,
        quantity: i32,
    },
    #[serde(rename = "cancel_action")]
    CancelAction,
    #[serde(rename = "debug_obj")]
    DebugObj { obj_id: i32 },
    #[serde(rename = "set_log_level")]
    SetLogLevel { target: String, level: String },
    #[serde(rename = "get_log_levels")]
    GetLogLevels,
}

fn decode_network_packet(input: &str) -> serde_json::Result<NetworkPacket> {
    let packet = serde_json::from_str::<NetworkPacket>(input)?;
    if matches!(
        &packet,
        NetworkPacket::RequestSafeLogout | NetworkPacket::CancelSafeLogout
    ) {
        let value = serde_json::from_str::<serde_json::Value>(input)?;
        let exact_command_only = value
            .as_object()
            .map(|object| object.len() == 1 && object.contains_key("cmd"))
            .unwrap_or(false);
        if !exact_command_only {
            return Err(<serde_json::Error as serde::de::Error>::custom(
                "safe-logout commands do not accept client-controlled fields",
            ));
        }
    }
    Ok(packet)
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct StructureList {
    pub result: Vec<Structure>,
}

/// One server-authoritative preparation affordance shown while a personal
/// crisis is gathering or ready to launch. IDs and states are stable protocol
/// values; copy remains presentation data and deliberately exposes no ECS IDs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CrisisPreparationOption {
    pub id: String,
    pub label: String,
    pub state: String,
    pub detail: String,
    pub action_hint: String,
}

/// A readable battlefield job for one living attacker in the personal Goblin
/// assault. This deliberately exposes no entity or target IDs: the role tells
/// the player what is at risk while moment-to-moment targeting remains world
/// state that can change as units move, die, or become unreachable.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CrisisAssaultIntent {
    pub role: String,
    pub label: String,
    pub intent: String,
}

/// Versioned, player-facing personal-crisis state. Gameplay systems remain
/// authoritative; this snapshot deliberately excludes ECS/object IDs,
/// generation bookkeeping, and cleanup/debug state.
#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CrisisStatusSnapshot {
    pub version: u32,
    pub exists: bool,
    pub kind: Option<String>,
    pub phase: Option<String>,
    pub pressure: Option<i32>,
    pub pressure_max: Option<i32>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub action_hint: Option<String>,
    pub severity: Option<String>,
    pub warning: bool,
    pub assault_ready: bool,
    pub assault_active: bool,
    pub resolved: bool,
    pub remaining_attackers: Option<i32>,
    pub total_attackers: Option<i32>,
    pub preparation_seconds_remaining: Option<i32>,
    pub preferred_launch_window: Option<String>,
    /// Additive v1 presentation field. Omitted outside Preparing and
    /// AssaultReady so older clients retain their existing compact payload.
    pub preparation_options: Option<Vec<CrisisPreparationOption>>,
    /// Additive v1 Goblin-assault presentation. Omitted outside an active
    /// role-bearing assault and shrinks as its assigned attackers are defeated.
    pub assault_intents: Option<Vec<CrisisAssaultIntent>>,
    pub continues_while_disconnected: bool,
}

/// Versioned, server-authoritative safe-logout presentation. The payload
/// deliberately contains no player, entity, sanctuary, or protected-run IDs.
#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SafeLogoutStatusSnapshot {
    pub version: u32,
    pub state: String,
    pub can_request: bool,
    pub can_cancel: bool,
    pub countdown_total_seconds: Option<i32>,
    pub countdown_remaining_seconds: Option<i32>,
    pub reason: Option<String>,
    pub message: String,
    pub in_own_sanctuary: bool,
    pub active_assault: bool,
    pub protected: bool,
    pub resumed_from_protection: bool,
}

/// One settlement currently protected by Safe Logout. Coordinates are
/// intentionally omitted: clients anchor presentation to a monolith already
/// learned through ordinary perception instead of receiving hidden map data.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProtectedSettlementSnapshot {
    pub player_id: i32,
    pub monolith_id: i32,
    pub sanctuary_radius: u32,
}

/// The connected player's own live sanctuary boundary. Coordinates are
/// intentionally omitted so the client anchors it to an ordinarily perceived
/// Monolith instead of learning hidden map information.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SanctuaryZoneSnapshot {
    pub monolith_id: i32,
    /// The single sanctuary-protection boundary.
    pub radius: u32,
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
    #[serde(rename = "hero_death_state")]
    HeroDeathState {
        phase: String,
        hero_id: i32,
        hero_name: String,
        resurrect_cost: i32,
        soulshards_available: i32,
        seconds_remaining: i32,
        message: String,
    },
    #[serde(rename = "info_hero")]
    InfoHero {
        id: i32,
        name: String,
        class: String,
        subclass: String,
        template: String,
        state: String,
        activity: Option<String>,
        image: String,
        portrait: Option<String>,
        hsl: Vec<i32>,
        items: Option<Vec<Item>>,
        skills: Option<HashMap<String, i32>>,
        attributes: Option<HashMap<String, i32>>,
        effects: Vec<effect::EffectInfo>,
        hp: Option<i32>,
        stamina: Option<i32>,
        mana: Option<i32>,
        thirst: String,
        hunger: String,
        tiredness: String,
        base_hp: Option<i32>,
        base_stamina: Option<i32>,
        base_mana: Option<i32>,
        hero_class: Option<String>,
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
        portrait: Option<String>,
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
        personality: Option<String>,
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
        total_work: Option<f32>,
        work_per_sec: Option<f32>,
        work_done_milliunits: Option<i64>,
        total_work_milliunits: Option<i64>,
        work_per_sec_milliunits: Option<i64>,
        construction_action_id: Option<i32>,
        construction_updated_at_ms: Option<i64>,
        req: Option<Vec<ResReq>>,
        upgrade_req: Option<Vec<ResReq>>,
        selected_upgrade: Option<String>,
        selected_upgrade_image: Option<String>,
        crop_type: Option<String>,
        crop_quantity: Option<i32>,
        crop_stage: Option<String>,
        upgradeable: bool,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_in: Option<i32>,
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
        survey_status: String,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<String>,
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
        produces: Option<Vec<ProducedItem>>,
    },
    #[serde(rename = "info_item_transfer")]
    InfoItemTransfer {
        source_id: i32,
        sourceitems: Inventory,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_expires_in: Option<i32>,
        target_id: i32,
        targetitems: Inventory,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_expires_in: Option<i32>,
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
    #[serde(rename = "info_stamina_update")]
    InfoStaminaUpdate {
        id: i32,
        stamina: i32,
    },
    #[serde(rename = "info_mana_update")]
    InfoManaUpdate {
        id: i32,
        mana: i32,
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
        data: Vec<ScoutedResourceCategory>,
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
        work_done_milliunits: i64,
        total_work_milliunits: i64,
        work_per_sec_milliunits: i64,
        construction_action_id: i32,
        construction_updated_at_ms: i64,
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
    #[serde(rename = "survey")]
    Survey {
        survey_time: i32,
    },
    #[serde(rename = "prospect")]
    Prospect {
        prospect_time: i32,
    },
    #[serde(rename = "investigate")]
    Investigate {
        investigate_time: i32,
    },
    #[serde(rename = "gather")]
    Gather {
        gather_time: i32,
    },
    #[serde(rename = "attack")]
    Attack {
        source_id: i32,
        attack_type: String,
        cooldown: f32,
        stamina_cost: i32,
    },
    #[serde(rename = "ability")]
    Ability {
        source_id: i32,
        ability_id: String,
        cooldown: i32,
        stamina_cost: Option<i32>,
        mana_cost: Option<i32>,
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
        action_id: Option<i32>,
        action_duration_ms: Option<i32>,
        action_elapsed_ms: Option<i32>,
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
    #[serde(rename = "debug_obj")]
    DebugObj {
        obj_id: i32,
        enabled: bool,
    },
    #[serde(rename = "log_level_set")]
    LogLevelSet {
        target: String,
        level: String,
        success: bool,
    },
    #[serde(rename = "log_levels")]
    LogLevels {
        overrides: Vec<(String, String)>,
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
    #[serde(rename = "combat_telegraph")]
    CombatTelegraph {
        attacker_id: i32,
        attacker_name: String,
        attack_type: String,
        defense_hint: String,
        strike_in: i32,
    },
    #[serde(rename = "info_true_death")]
    InfoTrueDeath {
        hero_name: String,
        hero_rank: String,
        total_xp: i32,
        score_total: i32,
        score_breakdown: ScoreBreakdown,
        days_survived: i32,
        waves_survived: i32,
        highest_pressure_level: i32,
        legendary_kills: i32,
        hideouts_cleared: i32,
        fate: String,
        crisis_tier: i32,
    },
    #[serde(rename = "objectives")]
    Objectives {
        // Legacy wire key: now reports completion of the tutorial's
        // Campfire-to-Shelter-Tent upgrade.
        build_campfire: bool,
        build_3_structures: bool,
        recruit_villager: bool,
        explore_poi: bool,
        survive_5_nights: bool,
        scavenge_shipwreck: bool,
    },
    #[serde(rename = "objective_state")]
    ObjectiveState {
        version: i32,
        current_id: String,
        objectives: Vec<ObjectiveProgress>,
    },
    #[serde(rename = "crisis_status")]
    CrisisStatus {
        #[serde(flatten)]
        status: CrisisStatusSnapshot,
    },
    #[serde(rename = "safe_logout_status")]
    SafeLogoutStatus {
        #[serde(flatten)]
        status: SafeLogoutStatusSnapshot,
    },
    #[serde(rename = "protected_settlements")]
    ProtectedSettlements {
        version: u32,
        settlements: Vec<ProtectedSettlementSnapshot>,
    },
    #[serde(rename = "sanctuary_state")]
    SanctuaryState {
        version: u32,
        zones: Vec<SanctuaryZoneSnapshot>,
    },
    #[serde(rename = "threat_state")]
    ThreatState {
        version: i32,
        day: i32,
        phase: String,
        pressure_level: String,
        next_night_warning: String,
        known_risks: Vec<ThreatRisk>,
        legendary_threats: Vec<LegendaryThreatPacket>,
    },
    #[serde(rename = "combat_state")]
    CombatState {
        version: i32,
        target_id: i32,
        enemy_intent: String,
        attack_history: Vec<String>,
        matching_combos: Vec<ComboHint>,
        available_finisher: Option<String>,
        #[serde(default)]
        finisher_transferable: bool,
        target_effects: Vec<String>,
        stamina_costs: StaminaCosts,
        abilities: Vec<AbilityHint>,
        counter_hint: String,
    },
    #[serde(rename = "discovery_event")]
    DiscoveryEvent {
        version: i32,
        discovery_type: String,
        title: String,
        unlock_source: String,
        location: Option<String>,
        result: String,
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
    pub mana: i32,
    pub base_mana: i32,
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
        missed: Option<bool>,
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

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize, Eq, Hash, PartialEq)]
pub struct MapObj {
    pub id: i32,
    pub player: i32,
    pub name: String,
    pub class: String,
    pub subclass: String,
    pub template: String,
    pub image: String,
    pub portrait: Option<String>,
    pub x: i32,
    pub y: i32,
    pub state: String,
    pub activity: Option<String>,
    pub vision: Option<u32>,
    pub hsl: Vec<i32>,
    pub groups: Vec<String>,
    pub work_done: Option<i32>,
    pub total_work: Option<i32>,
    pub work_per_sec: Option<i32>,
    pub work_done_milliunits: Option<i64>,
    pub total_work_milliunits: Option<i64>,
    pub work_per_sec_milliunits: Option<i64>,
    pub construction_action_id: Option<i32>,
    pub construction_updated_at_ms: Option<i64>,
    pub action_id: Option<i32>,
    pub action_duration_ms: Option<i32>,
    pub action_elapsed_ms: Option<i32>,
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
    pub quantity: i32,
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
    pub placement_resource: Option<String>,
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
    /// Stable identity for the currently executing timed event. Consecutive
    /// cycles in a persistent Operate workspace receive distinct ids.
    pub action_id: Option<i32>,
    /// Authoritative duration of the current action in milliseconds.
    pub action_duration_ms: Option<i32>,
    /// Authoritative elapsed time when this snapshot was produced.
    pub action_elapsed_ms: Option<i32>,
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
pub struct ObjectiveProgress {
    pub id: String,
    pub title: String,
    pub state: String,
    pub category: String,
    pub target: Option<String>,
    pub action_hint: String,
    pub lesson: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocker: Option<String>,
    pub reward: String,
    pub progress: Option<i32>,
    pub goal: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ThreatRisk {
    pub id: String,
    pub label: String,
    pub severity: String,
    pub trigger_hint: String,
    pub counter_hint: String,
    pub current: Option<i32>,
    pub threshold: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct LegendaryThreatPacket {
    pub name: String,
    pub status: String,
    pub days_active: i32,
    pub hideout_known: bool,
    pub hideout_location: Option<String>,
    pub next_attack_eta: Option<i32>,
    pub followers_defeated: i32,
    pub captains_defeated: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct ScoreBreakdown {
    pub survival: i32,
    pub progression: i32,
    pub wealth: i32,
    pub defense: i32,
    pub valor: i32,
    pub legacy: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ComboHint {
    pub name: String,
    pub remaining_attacks: Vec<String>,
    pub effect: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StaminaCosts {
    pub quick: i32,
    pub precise: i32,
    pub fierce: i32,
    pub block: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct AbilityHint {
    pub id: String,
    pub label: String,
    pub cost_type: String,
    pub cost: i32,
    pub range: i32,
    pub disabled_reason: Option<String>,
    pub hint: String,
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
    pub image: String,
    pub color: i32,
    pub yield_label: String,
    pub quantity_label: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScoutedResourceCategory {
    pub category: String,
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
    pub image: String,
    pub req: Vec<ResReq>,
    pub build_time: i32,
}

#[derive(Debug, Clone)]
pub struct ActiveStream {
    pub player_id: i32,
    pub client_id: Uuid,
    pub displaced_client_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct Stream {
    pub player_id: i32,
    pub client_id: Uuid,
    pub sender: tokio::sync::mpsc::Sender<String>,
}

#[derive(Debug, Clone)]
pub struct Streams(Arc<Mutex<HashMap<Uuid, Stream>>>);

/// Connection-scoped producer for the network-to-ECS queue. The current UUID
/// check and nonblocking crossbeam enqueue share the client-registry mutex, so
/// either an input is accepted before replacement activation or the displaced
/// socket is rejected; there is no check/enqueue gap.
#[derive(Clone)]
struct AuthorizedPlayerEventSender {
    sender: CBSender<PlayerEvent>,
    clients: Clients,
    player_id: i32,
    connection_id: Uuid,
}

#[derive(Clone)]
struct AcceptedPlayerEventSender {
    sender: CBSender<PlayerEvent>,
}

impl AuthorizedPlayerEventSender {
    fn new(
        sender: CBSender<PlayerEvent>,
        clients: Clients,
        player_id: i32,
        connection_id: Uuid,
    ) -> Self {
        Self {
            sender,
            clients,
            player_id,
            connection_id,
        }
    }

    fn is_current_locked(&self, clients: &HashMap<Uuid, Client>) -> bool {
        clients
            .get(&self.connection_id)
            .map(|client| {
                client.id == self.connection_id
                    && client.player_id == self.player_id
                    && !client.sender.is_closed()
                    && !client.termination_requested()
                    && !clients.iter().any(|(other_id, other)| {
                        *other_id != self.connection_id
                            && other.id == *other_id
                            && other.player_id == self.player_id
                            && !other.sender.is_closed()
                            && !other.termination_requested()
                    })
            })
            .unwrap_or(false)
    }

    /// Drop stale input without panicking the socket task. Existing handlers
    /// use `expect` because a disconnected crossbeam consumer is exceptional;
    /// displaced authority is an expected rejection instead.
    fn send(&self, event: PlayerEvent) -> Result<(), crossbeam_channel::SendError<PlayerEvent>> {
        let Ok(clients) = self.clients.lock() else {
            info!(
                "stale_connection_event_rejected player_id={} reason=registry_unavailable",
                self.player_id
            );
            return Ok(());
        };
        if !self.is_current_locked(&clients) {
            info!(
                "stale_connection_event_rejected player_id={} reason=authority_replaced",
                self.player_id
            );
            return Ok(());
        }
        self.sender.send(event)
    }

    /// Strict form for commands whose response must distinguish rejection.
    fn send_strict(&self, event: PlayerEvent) -> bool {
        let Ok(clients) = self.clients.lock() else {
            return false;
        };
        self.is_current_locked(&clients) && self.sender.send(event).is_ok()
    }

    /// Linearization token for an accepted operation that performs database
    /// awaits before it can emit its ECS event. Replacement after this point
    /// does not retroactively cancel a request accepted by the old authority;
    /// replacement before it prevents the operation entirely.
    fn begin_operation(&self) -> Option<AcceptedPlayerEventSender> {
        let clients = self.clients.lock().ok()?;
        self.is_current_locked(&clients)
            .then(|| AcceptedPlayerEventSender {
                sender: self.sender.clone(),
            })
    }
}

impl AcceptedPlayerEventSender {
    fn send(&self, event: PlayerEvent) -> Result<(), crossbeam_channel::SendError<PlayerEvent>> {
        self.sender.send(event)
    }
}

pub fn send_to_client(player_id: i32, packet: ResponsePacket, clients: &Res<Clients>) -> bool {
    send_serializable_to_client(player_id, &packet, clients)
}

pub fn send_serializable_to_client<T: Serialize>(
    player_id: i32,
    packet: &T,
    clients: &Res<Clients>,
) -> bool {
    let Some(serialized) = serialize_authoritative_packet(player_id, packet, clients) else {
        return false;
    };

    match clients.try_send_to_player(player_id, serialized) {
        Ok(()) => true,
        Err(CurrentConnectionSendError::RegistryUnavailable) => {
            warn!(
                "authoritative_outbound_registry_unavailable player_id={}",
                player_id
            );
            false
        }
        // Full and Closed already emit the single structured warning while
        // scheduling exact-connection termination. NotCurrent is an ordinary
        // race with disconnect or replacement and requires no extra log spam.
        Err(_) => false,
    }
}

pub fn serialize_authoritative_packet<T: Serialize>(
    player_id: i32,
    packet: &T,
    clients: &Clients,
) -> Option<String> {
    // Serialization can be expensive and may fail for invalid floating-point
    // values. Never perform it while holding the shared client registry lock.
    let serialized = match serde_json::to_string(packet) {
        Ok(serialized) => serialized,
        Err(error) => {
            warn!(
                "authoritative_packet_serialization_failed player_id={} error={:?}",
                player_id, error
            );
            let _ = clients.terminate_current_delivery(
                player_id,
                AuthoritativeDeliveryFailure::SerializationFailed,
            );
            return None;
        }
    };
    Some(serialized)
}

pub fn send_to_database(database_event: DatabaseEvent, database_managers: &Res<DatabaseManagers>) {
    let binding = database_managers.lock().unwrap();
    let database_client = binding.get(&DATABASE_MANAGER_ID).unwrap();

    match database_client.sender.try_send(database_event) {
        Ok(_) => (),
        Err(e) => println!("Error sending to db: {:?}", e),
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BuildProgressFields {
    pub work_done: Option<i32>,
    pub total_work: Option<i32>,
    pub work_per_sec: Option<i32>,
    pub work_done_milliunits: Option<i64>,
    pub total_work_milliunits: Option<i64>,
    pub work_per_sec_milliunits: Option<i64>,
    pub construction_action_id: Option<i32>,
    pub construction_updated_at_ms: Option<i64>,
}

pub fn game_tick_to_millis(game_tick: i32) -> i32 {
    game_tick.saturating_mul(1000) / TICKS_PER_SEC
}

pub fn game_tick_to_timestamp_millis(game_tick: i32) -> i64 {
    i64::from(game_tick).saturating_mul(1000) / i64::from(TICKS_PER_SEC)
}

pub fn work_to_milliunits(value: f32) -> i64 {
    (value * 1000.0).round() as i64
}

pub fn build_progress_fields(build_state: Option<&BuildUpgradeState>) -> BuildProgressFields {
    let Some(build_state) = build_state else {
        return BuildProgressFields::default();
    };

    let has_action = build_state.action_id > 0;

    BuildProgressFields {
        // Retain rounded fields for older clients while the shared timeline
        // consumes the precise fixed-point values below.
        work_done: Some(build_state.work_done.round() as i32),
        total_work: Some(build_state.build_upgrade_cost.round() as i32),
        work_per_sec: Some(build_state.work_per_sec.round() as i32),
        work_done_milliunits: Some(work_to_milliunits(build_state.work_done)),
        total_work_milliunits: Some(work_to_milliunits(build_state.build_upgrade_cost)),
        work_per_sec_milliunits: Some(work_to_milliunits(build_state.work_per_sec)),
        construction_action_id: has_action.then_some(build_state.action_id),
        construction_updated_at_ms: has_action.then_some(game_tick_to_timestamp_millis(
            build_state.progress_updated_at_tick,
        )),
    }
}

pub fn timed_action_progress_fields(
    action_id: i32,
    start_tick: i32,
    end_tick: i32,
    game_tick: i32,
) -> (Option<i32>, Option<i32>, Option<i32>) {
    let duration_ticks = end_tick.saturating_sub(start_tick).max(1);
    let elapsed_ticks = game_tick
        .saturating_sub(start_tick)
        .clamp(0, duration_ticks);
    (
        Some(action_id),
        Some(game_tick_to_millis(duration_ticks)),
        Some(game_tick_to_millis(elapsed_ticks)),
    )
}

pub fn action_progress_fields(
    action: Option<&ActionProgress>,
    game_tick: i32,
) -> (Option<i32>, Option<i32>, Option<i32>) {
    let Some(action) = action else {
        return (None, None, None);
    };

    timed_action_progress_fields(
        action.action_id,
        action.start_tick,
        action.end_tick,
        game_tick,
    )
}

pub fn create_network_obj(obj: &ObjQueryItem<'_, '_>, game_tick: i32) -> MapObj {
    let build_progress = build_progress_fields(obj.build_upgrade_state);
    let (action_id, action_duration_ms, action_elapsed_ms) =
        action_progress_fields(obj.action_progress, game_tick);

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
        activity: obj.active_task.map(ActiveTask::to_string),
        vision: None,
        image: obj.misc.image.clone(),
        portrait: obj.portrait.map(|portrait| portrait.0.clone()),
        hsl: obj.misc.hsl.clone(),
        groups: obj.misc.groups.clone(),
        work_done: build_progress.work_done,
        total_work: build_progress.total_work,
        work_per_sec: build_progress.work_per_sec,
        work_done_milliunits: build_progress.work_done_milliunits,
        total_work_milliunits: build_progress.total_work_milliunits,
        work_per_sec_milliunits: build_progress.work_per_sec_milliunits,
        construction_action_id: build_progress.construction_action_id,
        construction_updated_at_ms: build_progress.construction_updated_at_ms,
        action_id,
        action_duration_ms,
        action_elapsed_ms,
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
        activity: None,
        vision: None,
        image: image,
        portrait: None,
        hsl: hsl,
        groups: groups,
        work_done: None,
        total_work: None,
        work_per_sec: None,
        work_done_milliunits: None,
        total_work_milliunits: None,
        work_per_sec_milliunits: None,
        construction_action_id: None,
        construction_updated_at_ms: None,
        action_id: None,
        action_duration_ms: None,
        action_elapsed_ms: None,
    };

    network_obj
}

pub fn to_map_obj(obj: ObjQueryItem<'_, '_>, game_tick: i32) -> MapObj {
    let build_progress = build_progress_fields(obj.build_upgrade_state);
    let (action_id, action_duration_ms, action_elapsed_ms) =
        action_progress_fields(obj.action_progress, game_tick);

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
        activity: obj.active_task.map(ActiveTask::to_string),
        vision: None,
        image: obj.misc.image.clone(),
        portrait: obj.portrait.map(|portrait| portrait.0.clone()),
        hsl: obj.misc.hsl.clone(),
        groups: obj.misc.groups.clone(),
        work_done: build_progress.work_done,
        total_work: build_progress.total_work,
        work_per_sec: build_progress.work_per_sec,
        work_done_milliunits: build_progress.work_done_milliunits,
        total_work_milliunits: build_progress.total_work_milliunits,
        work_per_sec_milliunits: build_progress.work_per_sec_milliunits,
        construction_action_id: build_progress.construction_action_id,
        construction_updated_at_ms: build_progress.construction_updated_at_ms,
        action_id,
        action_duration_ms,
        action_elapsed_ms,
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
        activity: None,
        vision: Some(obj.viewshed.range),
        image: obj.misc.image.clone(),
        portrait: obj.portrait.map(|portrait| portrait.0.clone()),
        hsl: obj.misc.hsl.clone(),
        groups: obj.misc.groups.clone(),
        work_done: None,
        total_work: None,
        work_per_sec: None,
        work_done_milliunits: None,
        total_work_milliunits: None,
        work_per_sec_milliunits: None,
        construction_action_id: None,
        construction_updated_at_ms: None,
        action_id: None,
        action_duration_ms: None,
        action_elapsed_ms: None,
    };

    network_obj
}

lazy_static! {
    static ref TILESET: HashMap<String, serde_json::Value> = {
        let mut tileset = HashMap::new();

        // Load tilesets
        for entry in glob("./tileset/*.json").expect("Failed to read glob pattern") {
          match entry {
              Ok(path) => {
                let path = Path::new(&path);
                let file_stem = path.file_stem();
                let data = fs::read_to_string(&path).expect("Unable to read file");
                let json: serde_json::Value = serde_json::from_str(&data).expect("JSON does not have correct format.");
                let file_stem = file_stem.unwrap().to_str().unwrap().to_string();
                tileset.insert(file_stem, json);
              },
              Err(e) => eprintln!("Error loading tileset: {:?}", e),
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
    admin_status: AdminStatusState,
    reset_game: bool,
) {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    let streams = Streams(Arc::new(Mutex::new(HashMap::new())));

    let (stream_to_manager_sender, mut stream_to_manager_receiver) =
        tokio::sync::mpsc::channel::<ActiveStream>(100);

    let streams_clone = streams.clone();
    let manager_clients = clients.clone();
    tokio::spawn(async move {
        while let Some(message) = stream_to_manager_receiver.recv().await {
            let active_client_id = message.client_id;

            // A delayed notification may no longer describe the current
            // connection. Its displacement list is still safe to process:
            // `Clients::activate` captured only connections older than this
            // activation, so it can never name a later replacement.
            if !manager_clients.is_current_connection(message.player_id, active_client_id) {
                info!(
                    "connection_manager_stale_activation_cleanup player_id={}",
                    message.player_id
                );
            }

            // Terminate precisely the connections displaced by this atomic
            // activation. Rescanning by player here would introduce a race in
            // which a later replacement could be mistaken for an older stream.
            let streams_to_terminate: Vec<_> = {
                let streams_lock = streams_clone.0.lock().unwrap();
                message
                    .displaced_client_ids
                    .iter()
                    .filter_map(|client_id| {
                        streams_lock.get(client_id).and_then(|stream| {
                            (stream.player_id == message.player_id
                                && stream.client_id == *client_id
                                && stream.client_id != active_client_id)
                                .then(|| stream.sender.clone())
                        })
                    })
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

    if let Ok(client) = pool.get().await {
        let score_migrations = [
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS total_score INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_survival INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_progression INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_wealth INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_defense INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_valor INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS score_legacy INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS days_survived INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS highest_pressure_level INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS waves_survived INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS legendary_kills INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS hideouts_cleared INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE scores ADD COLUMN IF NOT EXISTS crisis_tier INTEGER NOT NULL DEFAULT 0",
            "UPDATE scores SET total_score = total_xp WHERE total_score = 0",
        ];

        for statement in score_migrations {
            if let Err(err) = client.execute(statement, &[]).await {
                println!("Score schema migration failed: {:?}", err);
            }
        }
    }

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
                    total_score,
                    score_survival,
                    score_progression,
                    score_wealth,
                    score_defense,
                    score_valor,
                    score_legacy,
                    days_survived,
                    highest_pressure_level,
                    waves_survived,
                    legendary_kills,
                    hideouts_cleared,
                    fate,
                    crisis_tier,
                } => {
                    let client = pool_clone
                        .get()
                        .await
                        .expect("Error getting DB connection from pool");

                    let statement = client
                        .prepare("INSERT INTO scores (player_id, hero_name, hero_rank, total_xp, total_score, score_survival, score_progression, score_wealth, score_defense, score_valor, score_legacy, days_survived, highest_pressure_level, waves_survived, legendary_kills, hideouts_cleared, fate, crisis_tier) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)")
                        .await
                        .expect("Error preparing statement");

                    client
                        .execute(
                            &statement,
                            &[
                                &player_id,
                                &hero_name,
                                &hero_rank,
                                &total_xp,
                                &total_score,
                                &score_survival,
                                &score_progression,
                                &score_wealth,
                                &score_defense,
                                &score_valor,
                                &score_legacy,
                                &days_survived,
                                &highest_pressure_level,
                                &waves_survived,
                                &legendary_kills,
                                &hideouts_cleared,
                                &fate,
                                &crisis_tier,
                            ],
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
    net_debug!("Listening on: {}", addr);

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(error) => {
                // A transient accept failure (for example EMFILE or an
                // aborted connection) must not permanently stop the server
                // from accepting every later player. Back off to avoid a hot
                // error loop while the operating system recovers.
                warn!("tcp_accept_failed error={:?}; retrying", error);
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        net_debug!("Peer address: {}", peer);

        let tls_acceptor = tls_acceptor.clone();
        let client_to_game_sender = client_to_game_sender.clone();
        let clients = clients.clone();
        let streams = streams.clone();
        let pool = pool.clone();
        let stream_to_manager_sender = stream_to_manager_sender.clone();
        let admin_status = admin_status.clone();

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
                        admin_status,
                    )
                    .await;
                }
                Err(e) => {
                    eprintln!("Failed to establish TLS connection: {}", e);
                }
            }
        });
    }
}

fn cleanup_connection(client_id: Uuid, clients: &Clients, streams: &Streams) {
    clients.remove_if_current(client_id);
    match streams.0.lock() {
        Ok(mut streams) => {
            streams.remove(&client_id);
        }
        Err(error) => {
            warn!(
                "connection_stream_cleanup_failed client_id={} error={:?}",
                client_id, error
            );
        }
    }
}

const WEBSOCKET_PING_INTERVAL: Duration = Duration::from_secs(10);
const WEBSOCKET_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(30);
const WEBSOCKET_WRITE_TIMEOUT: Duration = WEBSOCKET_PING_INTERVAL;

#[derive(Debug, Clone, Copy)]
struct ConnectionLiveness {
    last_seen: Instant,
}

impl ConnectionLiveness {
    fn new(now: Instant) -> Self {
        Self { last_seen: now }
    }

    fn record_activity(&mut self, now: Instant) {
        self.last_seen = now;
    }

    fn deadline(&self) -> Instant {
        self.last_seen + WEBSOCKET_INACTIVITY_TIMEOUT
    }

    fn has_timed_out(&self, now: Instant) -> bool {
        now >= self.deadline()
    }
}

async fn send_websocket_message<S>(sender: &mut S, message: Message) -> Result<()>
where
    S: futures_util::Sink<Message, Error = Error> + Unpin,
{
    match time::timeout(WEBSOCKET_WRITE_TIMEOUT, sender.send(message)).await {
        Ok(result) => result,
        Err(_) => Err(Error::Io(io::Error::new(
            io::ErrorKind::TimedOut,
            "websocket write timed out",
        ))),
    }
}

async fn flush_websocket<S>(sender: &mut S) -> Result<()>
where
    S: futures_util::Sink<Message, Error = Error> + Unpin,
{
    match time::timeout(WEBSOCKET_WRITE_TIMEOUT, sender.flush()).await {
        Ok(result) => result,
        Err(_) => Err(Error::Io(io::Error::new(
            io::ErrorKind::TimedOut,
            "websocket flush timed out",
        ))),
    }
}

fn websocket_text_payload(message: &Message) -> Option<&str> {
    // The game protocol is JSON over WebSocket text frames. In particular,
    // never call `to_text().unwrap()` for an arbitrary binary frame: invalid
    // UTF-8 must close only this connection, not panic its task.
    message.is_text().then(|| message.to_text().ok()).flatten()
}

async fn accept_connection(
    peer: SocketAddr,
    stream: TlsStream<TcpStream>,
    client_to_game_sender: CBSender<PlayerEvent>,
    clients: Clients,
    streams: Streams,
    pool: Pool,
    stream_to_manager_sender: tokio::sync::mpsc::Sender<ActiveStream>,
    admin_status: AdminStatusState,
) {
    if let Err((client_id, e)) = handle_connection(
        peer,
        stream,
        client_to_game_sender,
        clients.clone(),
        streams.clone(),
        pool,
        stream_to_manager_sender,
        admin_status,
    )
    .await
    {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8(_) => {
                info!("connection_closed reason={:?}", e);
                cleanup_connection(client_id, &clients, &streams);
            }
            err => {
                warn!("connection_processing_failed reason={:?}", err);
                cleanup_connection(client_id, &clients, &streams);
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
    admin_status: AdminStatusState,
) -> Result<(), (Uuid, Error)> {
    //Get the number of clients for a client id
    //let num_clients = clients.lock().unwrap().keys().len() as i32;

    //Client ID
    let client_id = Uuid::new_v4();

    net_debug!("New WebSocket connection from {}", peer);

    let peer_ip = peer.ip();

    net_debug!("Peer address: {:?}", peer_ip);

    // Get server address from env
    let env_addr = env::var("ADDRESS").unwrap();
    let server: SocketAddr = env_addr.parse().unwrap();
    let server_ip = server.ip();

    net_debug!("Server address: {:?}", server_ip);

    // Shared session ID state
    let mut session_id: Option<String> = None;
    let mut health_check: bool = false;
    let mut admin_status_request: bool = false;

    let callback = |req: &Request, response: Response| {
        let headers = req.headers();

        // Look for x-health-check header
        if let Some(_) = headers.get("x-health-check") {
            health_check = true;
            Ok(response)
        } else {
            admin_status_request = headers.get("x-admin-status").is_some();

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

            let Ok(cookie_str) = cookie.to_str() else {
                warn!("invalid_cookie_header peer={}", peer);
                let resp = Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Some("Invalid cookie header".into()))
                    .expect("static invalid-cookie response must be valid");
                return Err(resp);
            };

            // Split the string by ';' to separate the key-value pairs
            let pairs: Vec<&str> = cookie_str.split(";").map(|s| s.trim()).collect();

            let mut parsed: HashMap<&str, &str> = HashMap::new();

            for pair in pairs {
                if let Some((key, value)) = pair.split_once('=') {
                    parsed.insert(key, value);
                }
            }
            // Access values by key
            session_id = parsed.get("session").map(|s| s.to_string());

            Ok(response)
        }
    };

    let ws_stream = match accept_hdr_async(stream, callback).await {
        Ok(ws_stream) => ws_stream,
        Err(e) => {
            println!("WebSocket handshake error: {}", e);
            return Err((client_id, e));
        }
    };

    if health_check {
        net_debug!("Server health check");
        let (mut ws_sender, _ws_receiver) = ws_stream.split();
        send_websocket_message(&mut ws_sender, Message::Text("Pong".into()))
            .await
            .map_err(|e| (client_id, e))?;

        return Ok(());
    }

    let Some(session_id) = session_id else {
        return Err((client_id, Error::AttackAttempt));
    };

    let client = pool
        .get()
        .await
        .expect("Error getting connection from pool");

    let row_session = client
        .query_one(
            "SELECT player_id, created_at, last_login FROM sessions WHERE session = $1",
            &[&session_id],
        )
        .await;

    let Ok(row_session) = row_session else {
        println!("Session not found");
        return Err((client_id, Error::AttackAttempt));
    };

    let created_at: DateTime<Utc> = row_session.get::<_, DateTime<Utc>>("created_at");
    let last_login: Option<DateTime<Utc>> = row_session.get("last_login");
    let last_active = last_login.unwrap_or(created_at);
    let now = chrono::Utc::now();

    // Match the web service's seven-day idle expiration policy. Using last_login
    // keeps an active browser session from being rejected by the game socket.
    if now.signed_duration_since(last_active) > chrono::Duration::days(7) {
        println!("Session expired after seven days of inactivity");
        return Err((client_id, Error::AttackAttempt));
    }

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Get the player id from the session
    let player_id: i32 = row_session.get("player_id");

    // Axum already refreshed and authorized admin dashboard requests. Avoid a
    // second database write on every five-second status poll.
    if !admin_status_request {
        if let Err(error) = client
            .execute(
                "UPDATE sessions SET last_login = NOW() WHERE session = $1",
                &[&session_id],
            )
            .await
        {
            println!("Unable to refresh authenticated session: {error}");
            return Err((client_id, Error::AttackAttempt));
        }
    }

    // Get the player account_name from the accounts table
    let row_account = client
        .query_one(
            "SELECT account_name, player_state, is_admin FROM accounts WHERE player_id = $1",
            &[&player_id],
        )
        .await;

    let Ok(row_account) = row_account else {
        // Account not found - this is a data integrity issue (orphaned session)
        println!(
            "DATA INTEGRITY ERROR: Account not found for authenticated player_id {}",
            player_id
        );
        println!("Cleaning up orphaned session...");

        // Clean up the orphaned session
        if let Err(e) = client
            .execute("DELETE FROM sessions WHERE session = $1", &[&session_id])
            .await
        {
            println!("Failed to delete orphaned session: {:?}", e);
        } else {
            println!("Orphaned session deleted successfully");
        }

        return Err((client_id, Error::AttackAttempt));
    };

    let player_username: String = row_account
        .get::<_, Option<String>>("account_name")
        .unwrap_or_default();
    let player_state: String = row_account.get("player_state");
    let is_admin: bool = row_account
        .get::<_, Option<bool>>("is_admin")
        .unwrap_or(false);

    // Operational status requests are authenticated twice: Axum authorizes
    // the page/API first, and the game socket independently verifies the same
    // unexpired admin session. This path deliberately returns before creating
    // a Stream or Client, so polling cannot displace an active game socket.
    if admin_status_request {
        if !is_admin {
            warn!(
                "admin_status_access_denied player_id={} reason=not_admin",
                player_id
            );
            send_websocket_message(
                &mut ws_sender,
                Message::Text(r#"{"error":"insufficient_privileges"}"#.into()),
            )
            .await
            .map_err(|error| (client_id, error))?;
            return Ok(());
        }

        let snapshot = admin_status.snapshot().ok_or_else(|| {
            (
                client_id,
                Error::Io(io::Error::new(
                    io::ErrorKind::Other,
                    "admin status snapshot lock is unavailable",
                )),
            )
        })?;
        let serialized = serde_json::to_string(&snapshot).map_err(|error| {
            (
                client_id,
                Error::Io(io::Error::new(io::ErrorKind::InvalidData, error)),
            )
        })?;
        send_websocket_message(&mut ws_sender, Message::Text(serialized.into()))
            .await
            .map_err(|error| (client_id, error))?;
        let _ = send_websocket_message(&mut ws_sender, Message::Close(None)).await;
        info!("admin_status_snapshot_served player_id={}", player_id);
        return Ok(());
    }

    // Only gameplay sockets receive outbound queues and enter the authoritative
    // connection registries.
    let (game_to_client_sender, mut game_to_client_receiver) = tokio::sync::mpsc::channel(100);
    let (manager_to_stream_sender, mut manager_to_stream_receiver) =
        tokio::sync::mpsc::channel::<String>(100);

    println!("Inserting stream into streams hashmap");
    streams.0.lock().unwrap().insert(
        client_id,
        Stream {
            client_id,
            player_id,
            sender: manager_to_stream_sender,
        },
    );

    let row_score = client.query_one("SELECT hero_name, hero_rank, total_xp, COALESCE(total_score, total_xp) as total_score, COALESCE(score_survival, 0) as score_survival, COALESCE(score_progression, 0) as score_progression, COALESCE(score_wealth, 0) as score_wealth, COALESCE(score_defense, 0) as score_defense, COALESCE(score_valor, 0) as score_valor, COALESCE(score_legacy, 0) as score_legacy, COALESCE(days_survived, 0) as days_survived, COALESCE(highest_pressure_level, 0) as highest_pressure_level, COALESCE(waves_survived, 0) as waves_survived, COALESCE(legendary_kills, 0) as legendary_kills, COALESCE(hideouts_cleared, 0) as hideouts_cleared, fate, COALESCE(crisis_tier, 0) as crisis_tier FROM scores WHERE player_id = $1 ORDER BY created_at DESC LIMIT 1", &[&player_id]).await;

    let mut hero_name: String = String::new();
    let mut hero_rank: String = String::new();
    let mut total_xp: i32 = 0;
    let mut total_score: i32 = 0;
    let mut score_survival: i32 = 0;
    let mut score_progression: i32 = 0;
    let mut score_wealth: i32 = 0;
    let mut score_defense: i32 = 0;
    let mut score_valor: i32 = 0;
    let mut score_legacy: i32 = 0;
    let mut days_survived: i32 = 0;
    let mut highest_pressure_level: i32 = 0;
    let mut waves_survived: i32 = 0;
    let mut legendary_kills: i32 = 0;
    let mut hideouts_cleared: i32 = 0;
    let mut fate: String = String::new();
    let mut crisis_tier: i32 = 0;

    if let Ok(row_score) = row_score {
        hero_name = row_score.get("hero_name");
        hero_rank = row_score.get("hero_rank");
        total_xp = row_score.get("total_xp");
        total_score = row_score.get("total_score");
        score_survival = row_score.get("score_survival");
        score_progression = row_score.get("score_progression");
        score_wealth = row_score.get("score_wealth");
        score_defense = row_score.get("score_defense");
        score_valor = row_score.get("score_valor");
        score_legacy = row_score.get("score_legacy");
        days_survived = row_score.get("days_survived");
        highest_pressure_level = row_score.get("highest_pressure_level");
        waves_survived = row_score.get("waves_survived");
        legendary_kills = row_score.get("legendary_kills");
        hideouts_cleared = row_score.get("hideouts_cleared");
        fate = row_score.get("fate");
        crisis_tier = row_score.get("crisis_tier");
    };

    // Full session/account validation has completed. This atomic activation is
    // the authority boundary: displaced sockets immediately lose command
    // authority even if their asynchronous close has not arrived yet.
    let (network_client, mut authoritative_termination_receiver) =
        Client::with_termination_channel(client_id, player_id, game_to_client_sender);
    let displaced = clients.activate(network_client);
    if !clients.is_current_connection(player_id, client_id) {
        return Err((client_id, Error::ConnectionClosed));
    }
    info!(
        "authenticated_connection_activated player_id={} displaced_connections={}",
        player_id,
        displaced.len()
    );
    let client_to_game_sender = AuthorizedPlayerEventSender::new(
        client_to_game_sender,
        clients.clone(),
        player_id,
        client_id,
    );

    // Notify the stream manager only after atomic activation. It independently
    // verifies this UUID is still current, making delayed messages harmless.
    if stream_to_manager_sender
        .send(ActiveStream {
            player_id,
            client_id,
            displaced_client_ids: displaced,
        })
        .await
        .is_err()
    {
        return Err((client_id, Error::ConnectionClosed));
    }

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
                    player_id,
                    connection_id: client_id,
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
                score_total: total_score,
                score_breakdown: ScoreBreakdown {
                    survival: score_survival,
                    progression: score_progression,
                    wealth: score_wealth,
                    defense: score_defense,
                    valor: score_valor,
                    legacy: score_legacy,
                },
                days_survived,
                waves_survived,
                highest_pressure_level,
                legendary_kills,
                hideouts_cleared,
                fate: fate.clone(),
                crisis_tier: crisis_tier,
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

    if !clients.is_current_connection(player_id, client_id) {
        return Err((client_id, Error::ConnectionClosed));
    }

    send_websocket_message(&mut ws_sender, Message::Text(res.into()))
        .await
        .map_err(|e| (client_id, e))?;

    let now = Instant::now();
    let mut liveness = ConnectionLiveness::new(now);
    let mut ping_interval =
        time::interval_at(now + WEBSOCKET_PING_INTERVAL, WEBSOCKET_PING_INTERVAL);
    ping_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let inactivity_timeout = time::sleep_until(liveness.deadline());
    tokio::pin!(inactivity_timeout);

    // Receive websocket, game, manager, and liveness signals without allowing
    // a closed channel to remain permanently selectable.
    loop {
        tokio::select! {
            biased;

            termination = authoritative_termination_receiver.changed() => {
                match termination {
                    Ok(()) if authoritative_termination_receiver.borrow_and_update().is_some() => {
                        // The shared outbound policy already emitted the sole
                        // structured failure warning. Prioritizing this control
                        // signal prevents stale queued data from being drained
                        // after delivery integrity has been lost.
                        break;
                    }
                    Ok(()) => {}
                    Err(_) => {
                        // Replacement activation or registry teardown dropped
                        // the exact connection's control sender.
                        break;
                    }
                }
            }

            //Receive messages from the websocket
            msg = ws_receiver.next() => {
                match msg {
                    Some(msg) => {
                        let msg = match msg {
                            Ok(msg) => msg,
                            Err(e) => return Err((client_id, e)),
                        };

                        if authenticated_player_for_connection(client_id, &clients)
                            != Some(player_id)
                        {
                            info!(
                                "stale_connection_packet_rejected player_id={} client_id={}",
                                player_id, client_id
                            );
                            break;
                        }

                        let now = Instant::now();
                        liveness.record_activity(now);
                        inactivity_timeout.as_mut().reset(liveness.deadline());

                        if let Some(message_text) = websocket_text_payload(&msg) {

                            println!("player_id: {:?}", player_id);

                            //Check if the player is authenticated
                            /*if player_id == -1 {
                                //Attempt to login
                                let res_packet: ResponsePacket = match decode_network_packet(message_text) {
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
                                net_debug!("{:?}", res_packet);
                                //TODO send event to game
                                //client_to_game_sender.send(Message::text(res)).expect("Could not send message");

                                //Send response to client
                                let res = serde_json::to_string(&res_packet).unwrap();
                                if let Err(e) = ws_sender.send(Message::Text(res)).await {
                                    return Err((player_id, e));
                                }
                            } else {*/
                                println!("Authenticated packet: {:?}", message_text);

                                let res_packet: ResponsePacket = match decode_network_packet(message_text) {
                                    Ok(packet) => {
                                        match packet {
                                            NetworkPacket::SelectedClass{class_name, hero_name, portrait} => {
                                                handle_selected_class(
                                                    pool.clone(),
                                                    player_id,
                                                    class_name,
                                                    hero_name,
                                                    portrait,
                                                    client_to_game_sender.clone()
                                                ).await
                                            }
                                            NetworkPacket::RecreateHero => {
                                                handle_recreate_hero(
                                                    pool.clone(),
                                                    player_id,
                                                    client_to_game_sender.clone(),
                                                ).await
                                            }
                                            NetworkPacket::RequestSafeLogout => {
                                                handle_safe_logout_command(
                                                    client_id,
                                                    &clients,
                                                    client_to_game_sender.clone(),
                                                    false,
                                                )
                                            }
                                            NetworkPacket::CancelSafeLogout => {
                                                handle_safe_logout_command(
                                                    client_id,
                                                    &clients,
                                                    client_to_game_sender.clone(),
                                                    true,
                                                )
                                            }
                                            NetworkPacket::GetStats{id} => {
                                                handle_get_stats(player_id, id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::ImageDef{name} => {
                                                println!("ImageDef name: {:?}", name);
                                                let mut name_stripped = name.clone();
                                                let raw_name = name;

                                                // Guard against empty name; only strip a trailing digit when present
                                                if name_stripped
                                                    .chars()
                                                    .last()
                                                    .map(|c| c.is_numeric())
                                                    .unwrap_or(false)
                                                {
                                                    name_stripped.pop();
                                                }

                                                match TILESET.get(&name_stripped) {
                                                    Some(value) => ResponsePacket::ImageDef {
                                                        name: raw_name,
                                                        data: value.clone(),
                                                    },
                                                    None => {
                                                        eprintln!(
                                                            "ImageDef: missing tileset for '{}' (requested as '{}')",
                                                            name_stripped, raw_name
                                                        );
                                                        ResponsePacket::ImageDef {
                                                            name: raw_name,
                                                            data: serde_json::json!({ "result": "404" }),
                                                        }
                                                    }
                                                }
                                            }
                                            NetworkPacket::Move{x, y} => {
                                                handle_move(player_id, x, y, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Attack{attack_type, source_id, target_id} => {
                                                handle_attack(player_id, attack_type, source_id, target_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Ability{ability_id, source_id, target_id} => {
                                                handle_ability(player_id, ability_id, source_id, target_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Combo{source_id, target_id, combo_type} => {
                                                handle_combo(player_id, source_id, target_id, combo_type, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Block{source_id, defense} => {
                                                handle_block(player_id, source_id, defense, client_to_game_sender.clone())
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
                                            NetworkPacket::LootAll{source_id, target_id} => {
                                                handle_loot_all(player_id, source_id, target_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::ItemSplit{owner_id, item, quantity} => {
                                                handle_item_split(player_id, owner_id, item, quantity, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Gather{res_type} => {
                                                handle_gather(player_id, res_type, client_to_game_sender.clone())
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
                                            NetworkPacket::Craft{recipe, signature_item_id} => {
                                                handle_craft(player_id, recipe, signature_item_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::StructureCraft{structure_id, recipe, signature_item_id} => {
                                                handle_structure_craft(player_id, structure_id, recipe, signature_item_id, client_to_game_sender.clone())
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
                                            NetworkPacket::Prospect{} => {
                                                handle_prospect(player_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::NearbyResources{} => {
                                                handle_nearby_resources(player_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Explore{} => {
                                                handle_explore(player_id, client_to_game_sender.clone())
                                            }
                                            NetworkPacket::Investigate{target_id} => {
                                                handle_investigate(player_id, target_id, client_to_game_sender.clone())
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
                                            NetworkPacket::OrderProspect{source_id} => {
                                                handle_order_prospect(player_id, source_id, client_to_game_sender.clone())
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
                                            NetworkPacket::DebugObj { obj_id } => {
                                                if is_admin {
                                                    handle_debug_obj(player_id, obj_id, client_to_game_sender.clone())
                                                } else {
                                                    ResponsePacket::Error {
                                                        errmsg: "Insufficient privileges".to_owned(),
                                                    }
                                                }
                                            }
                                            NetworkPacket::SetLogLevel { target, level } => {
                                                if is_admin {
                                                    handle_set_log_level(player_id, target, level, client_to_game_sender.clone())
                                                } else {
                                                    ResponsePacket::Error {
                                                        errmsg: "Insufficient privileges".to_owned(),
                                                    }
                                                }
                                            }
                                            NetworkPacket::GetLogLevels => {
                                                if is_admin {
                                                    handle_get_log_levels(player_id, client_to_game_sender.clone())
                                                } else {
                                                    ResponsePacket::Error {
                                                        errmsg: "Insufficient privileges".to_owned(),
                                                    }
                                                }
                                            }

                                            _ => ResponsePacket::Ok
                                        }
                                    },
                                    Err(packet) => {
                                        let ping = r#"0"#;

                                        if message_text == ping {
                                            ResponsePacket::Pong
                                        } else {
                                            println!("Error packet: {:?}", packet);
                                            ResponsePacket::Error{errmsg: "Unknown packet".to_owned()}
                                        }
                                    }
                                };
                                if res_packet == ResponsePacket::Pong {
                                    send_websocket_message(
                                        &mut ws_sender,
                                        Message::Text("1".to_string().into()),
                                    )
                                    .await
                                    .map_err(|e| (client_id, e))?;
                                }
                                else if res_packet != ResponsePacket::None {
                                    let res = serde_json::to_string(&res_packet).unwrap();
                                    send_websocket_message(&mut ws_sender, Message::Text(res.into()))
                                        .await
                                        .map_err(|e| (client_id, e))?;
                                }
                        } else if msg.is_binary() {
                            warn!(
                                "binary_websocket_frame_rejected player_id={} client_id={}",
                                player_id, client_id
                            );
                            // Best-effort close. Even if the peer has already
                            // gone away, falling through the loop performs the
                            // authoritative Clients/Streams cleanup below.
                            let _ = send_websocket_message(&mut ws_sender, Message::Close(None)).await;
                            break;
                        } else if msg.is_close() {
                            println!("Message is closed for player: {:?}", player_id);
                            break;
                        } else if msg.is_ping() {
                            // Tungstenite queues the matching Pong while reading
                            // the Ping. Flush that automatic control response.
                            flush_websocket(&mut ws_sender)
                                .await
                                .map_err(|e| (client_id, e))?;
                        } else if msg.is_pong() {
                            net_debug!(
                                "websocket_pong player_id={} client_id={}",
                                player_id,
                                client_id
                            );
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
                let Some(game_msg) = game_msg else {
                    info!(
                        "connection_outbound_channel_closed player_id={} client_id={}",
                        player_id, client_id
                    );
                    break;
                };

                match serde_json::from_str(game_msg.as_str()) {
                    Ok(ResponsePacket::Disconnect { player, client }) => {
                        info!("server_disconnect_requested player_id={}", player);
                        send_websocket_message(&mut ws_sender, Message::Close(None))
                            .await
                            .map_err(|e| (client_id, e))?;
                        clients.remove_if_current(client);
                        break;
                    }
                    _ => {
                        send_websocket_message(&mut ws_sender, Message::Text(game_msg.into()))
                            .await
                            .map_err(|e| (client_id, e))?;
                    }
                }
            }

            //Receive messages from the manager
            manager_msg = manager_to_stream_receiver.recv() => {
                match manager_msg {
                    Some(_manager_msg) => {
                        send_websocket_message(&mut ws_sender, Message::Close(None))
                            .await
                            .map_err(|e| (client_id, e))?;
                        break;
                    }
                    None => {
                        info!(
                            "connection_manager_channel_closed player_id={} client_id={}",
                            player_id, client_id
                        );
                        break;
                    }
                }
            }

            _ = ping_interval.tick() => {
                if liveness.has_timed_out(Instant::now()) {
                    warn!(
                        "websocket_inactivity_timeout player_id={} client_id={} timeout_seconds={}",
                        player_id,
                        client_id,
                        WEBSOCKET_INACTIVITY_TIMEOUT.as_secs()
                    );
                    break;
                }

                send_websocket_message(&mut ws_sender, Message::Ping(Vec::new().into()))
                    .await
                    .map_err(|e| (client_id, e))?;
            }

            _ = &mut inactivity_timeout => {
                warn!(
                    "websocket_inactivity_timeout player_id={} client_id={} timeout_seconds={}",
                    player_id,
                    client_id,
                    WEBSOCKET_INACTIVITY_TIMEOUT.as_secs()
                );
                break;
            }
        }
    }
    cleanup_connection(client_id, &clients, &streams);
    Ok(())
}

async fn handle_selected_class(
    pool: Pool,
    player_id: i32,
    class_name: String,
    hero_name: String,
    portrait: String,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    println!("handle_selected_class: {:?}", player_id);
    let Some(accepted_sender) = client_to_game_sender.begin_operation() else {
        return ResponsePacket::Error {
            errmsg: "This connection is no longer authoritative.".to_string(),
        };
    };

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

    if !is_valid_hero_portrait(&portrait) {
        return ResponsePacket::Error {
            errmsg: "Invalid hero portrait".to_owned(),
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
    accepted_sender
        .send(PlayerEvent::NewPlayer {
            player_id: player_id,
            hero_name: hero_name.clone(),
            class_name: class_name.clone(),
            portrait: portrait.clone(),
        })
        .expect("Could not send message");

    ResponsePacket::InfoSelectClass {
        result: "success".to_owned(),
    }
}

async fn handle_recreate_hero(
    pool: Pool,
    player_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    println!("handle_recreate_hero: {:?}", player_id);
    let Some(_accepted_operation) = client_to_game_sender.begin_operation() else {
        return ResponsePacket::Error {
            errmsg: "This connection is no longer authoritative.".to_string(),
        };
    };

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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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

/// Resolve the player solely from the currently active authenticated
/// connection. The WebSocket task's local `player_id` is intentionally not an
/// argument: a removed, replaced, malformed, or closed client record must not
/// retain authority to start or cancel safe logout.
fn authenticated_player_for_connection(client_id: Uuid, clients: &Clients) -> Option<i32> {
    let player_id = {
        let clients_guard = clients.lock().ok()?;
        let client = clients_guard.get(&client_id)?;
        (client.id == client_id && client.player_id >= 0).then_some(client.player_id)?
    };

    clients
        .is_current_connection(player_id, client_id)
        .then_some(player_id)
}

fn handle_safe_logout_command(
    client_id: Uuid,
    clients: &Clients,
    client_to_game_sender: AuthorizedPlayerEventSender,
    cancel: bool,
) -> ResponsePacket {
    let Some(player_id) = authenticated_player_for_connection(client_id, clients) else {
        return ResponsePacket::Error {
            errmsg: "Safe Logout requires an authenticated connection.".to_string(),
        };
    };

    let event = if cancel {
        PlayerEvent::CancelSafeLogout {
            player_id,
            connection_id: client_id,
        }
    } else {
        PlayerEvent::RequestSafeLogout {
            player_id,
            connection_id: client_id,
        }
    };

    if !client_to_game_sender.send_strict(event) {
        return ResponsePacket::Error {
            errmsg: "Safe Logout is temporarily unavailable.".to_string(),
        };
    }

    // The authoritative safe-logout status system reports acceptance,
    // rejection, cancellation, countdown, and completion asynchronously.
    ResponsePacket::None
}

fn handle_attack(
    player_id: i32,
    attack_type: String,
    source_id: i32,
    target_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
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

fn handle_ability(
    player_id: i32,
    ability_id: String,
    source_id: i32,
    target_id: Option<i32>,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Ability {
            player_id,
            ability_id,
            source_id,
            target_id,
        })
        .expect("Could not send message");

    ResponsePacket::None
}

fn handle_combo(
    player_id: i32,
    source_id: i32,
    target_id: i32,
    combo_type: String,
    client_to_game_sender: AuthorizedPlayerEventSender,
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

fn handle_block(
    player_id: i32,
    source_id: i32,
    defense: String,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Block {
            player_id,
            source_id,
            defense,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_info_obj(
    player_id: i32,
    id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    if !Map::is_valid_pos((x, y)) {
        warn!(
            "invalid_info_tile_coordinates player_id={} x={} y={}",
            player_id, x, y
        );
        return ResponsePacket::Error {
            errmsg: "Invalid tile coordinates.".to_string(),
        };
    }

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
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    if !Map::is_valid_pos((x, y)) {
        warn!(
            "invalid_info_tile_resources_coordinates player_id={} x={} y={}",
            player_id, x, y
        );
        return ResponsePacket::Error {
            errmsg: "Invalid tile coordinates.".to_string(),
        };
    }

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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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

fn handle_loot_all(
    player_id: i32,
    source_id: i32,
    target_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::LootAll {
            player_id,
            source_id,
            target_id,
        })
        .expect("Could not send message");

    // Response will come from player.rs after the authoritative bulk transfer.
    ResponsePacket::None
}

fn handle_item_split(
    player_id: i32,
    owner_id: i32,
    item: i32,
    quantity: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
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

fn handle_gather(
    player_id: i32,
    res_type: String,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Gather {
            player_id: player_id,
            res_type,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_plant(
    player_id: i32,
    structure_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    signature_item_id: Option<i32>,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Craft {
            player_id: player_id,
            recipe_name: recipe,
            signature_item_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_structure_craft(
    player_id: i32,
    structure_id: i32,
    recipe: String,
    signature_item_id: Option<i32>,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::StructureCraft {
            player_id: player_id,
            structure_id: structure_id,
            recipe_name: recipe,
            signature_item_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::None
}

fn handle_order_follow(
    player_id: i32,
    source_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Survey {
            player_id: player_id,
            source_id: source_id,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_prospect(
    player_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Prospect {
            player_id: player_id,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_explore(
    player_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::Explore {
            player_id: player_id,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_investigate(
    player_id: i32,
    target_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::InvestigatePOI {
            player_id,
            target_id,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_nearby_resources(
    player_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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

fn handle_order_prospect(
    player_id: i32,
    source_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::OrderProspect {
            player_id: player_id,
            villager_id: source_id,
        })
        .expect("Could not send message");

    ResponsePacket::Ok
}

fn handle_order_experiment(
    player_id: i32,
    villager_id: i32,
    structure_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
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
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::CancelAction {
            player_id: player_id,
        })
        .expect("Could not send message");

    // Response will come from game.rs
    ResponsePacket::Ok
}

fn handle_debug_obj(
    player_id: i32,
    obj_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::DebugObj {
            player_id: player_id,
            obj_id: obj_id,
        })
        .expect("Could not send message");

    ResponsePacket::None
}

fn handle_set_log_level(
    player_id: i32,
    target: String,
    level: String,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    // Validate level
    if !["ERROR", "WARN", "INFO", "DEBUG", "TRACE", "OFF"].contains(&level.as_str()) {
        return ResponsePacket::Error {
            errmsg: format!(
                "Invalid log level: {}. Use ERROR, WARN, INFO, DEBUG, TRACE, or OFF",
                level
            ),
        };
    }

    client_to_game_sender
        .send(PlayerEvent::SetLogLevel {
            player_id,
            target,
            level,
        })
        .expect("Could not send message");

    ResponsePacket::None
}

fn handle_get_log_levels(
    player_id: i32,
    client_to_game_sender: AuthorizedPlayerEventSender,
) -> ResponsePacket {
    client_to_game_sender
        .send(PlayerEvent::GetLogLevels { player_id })
        .expect("Could not send message");

    ResponsePacket::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_progress_fields_resume_from_server_elapsed_time() {
        let action = ActionProgress {
            action_id: 41,
            start_tick: 100,
            end_tick: 400,
        };

        assert_eq!(
            action_progress_fields(Some(&action), 220),
            (Some(41), Some(30_000), Some(12_000))
        );
        assert_eq!(
            action_progress_fields(Some(&action), 500),
            (Some(41), Some(30_000), Some(30_000))
        );
        assert_eq!(action_progress_fields(None, 220), (None, None, None));

        assert_eq!(
            timed_action_progress_fields(42, 1_000, 1_150, 1_075),
            (Some(42), Some(15_000), Some(7_500))
        );
    }

    #[test]
    fn construction_progress_fields_preserve_precision_identity_and_timestamp() {
        let build = BuildUpgradeState {
            build_upgrade_cost: 50.0,
            work_done: 12.345,
            work_per_sec: 2.5,
            progress_updated_at_tick: 180,
            start_time: 100,
            action_id: 77,
        };

        let fields = build_progress_fields(Some(&build));
        assert_eq!(fields.work_done, Some(12));
        assert_eq!(fields.work_done_milliunits, Some(12_345));
        assert_eq!(fields.total_work_milliunits, Some(50_000));
        assert_eq!(fields.work_per_sec_milliunits, Some(2_500));
        assert_eq!(fields.construction_action_id, Some(77));
        assert_eq!(fields.construction_updated_at_ms, Some(18_000));

        let value = serde_json::to_value(ResponsePacket::WorkUpdate {
            structure_id: 9,
            work_done: build.work_done,
            total_work: build.build_upgrade_cost,
            work_per_sec: build.work_per_sec,
            work_done_milliunits: work_to_milliunits(build.work_done),
            total_work_milliunits: work_to_milliunits(build.build_upgrade_cost),
            work_per_sec_milliunits: work_to_milliunits(build.work_per_sec),
            construction_action_id: build.action_id,
            construction_updated_at_ms: fields.construction_updated_at_ms.unwrap(),
        })
        .unwrap();

        assert!((value["work_done"].as_f64().unwrap() - 12.345).abs() < 0.000_001);
        assert_eq!(value["work_done_milliunits"], 12_345);
        assert_eq!(value["construction_action_id"], 77);
        assert_eq!(value["construction_updated_at_ms"], 18_000);

        // Construction can remain active in a persistent world well beyond
        // the roughly 25-day range of a 32-bit millisecond counter.
        assert_eq!(
            game_tick_to_timestamp_millis(30 * 24 * 60 * 60 * TICKS_PER_SEC),
            30_i64 * 24 * 60 * 60 * 1000,
        );
    }

    #[test]
    fn work_queue_packet_transmits_authoritative_cycle_timing() {
        let value = serde_json::to_value(ResponsePacket::InfoStructureQueue {
            structure_id: 7,
            queue: vec![WorkEntry {
                work_type: "Operate".to_string(),
                work_status: "In Progress".to_string(),
                villager_id: 12,
                recipe_name: None,
                recipe_image: None,
                refine_item_id: None,
                refine_item_image: None,
                refine_item_class: None,
                work_time: 15,
                progress: 7,
                action_id: Some(42),
                action_duration_ms: Some(15_000),
                action_elapsed_ms: Some(7_500),
            }],
        })
        .unwrap();

        assert_eq!(value["queue"][0]["action_id"], 42);
        assert_eq!(value["queue"][0]["action_duration_ms"], 15_000);
        assert_eq!(value["queue"][0]["action_elapsed_ms"], 7_500);
    }

    #[test]
    fn structure_upgrade_packet_preserves_the_configured_image_key() {
        let value = serde_json::to_value(ResponsePacket::InfoUpgrade {
            id: 7,
            upgrade_list: vec![UpgradeTemplate {
                name: "Shelter Tent".to_string(),
                template: "Shelter Tent".to_string(),
                image: "tent".to_string(),
                req: Vec::new(),
                build_time: 50,
            }],
        })
        .unwrap();

        assert_eq!(value["upgrade_list"][0]["image"], "tent");
    }

    fn no_crisis_status() -> CrisisStatusSnapshot {
        CrisisStatusSnapshot {
            version: 1,
            exists: false,
            kind: None,
            phase: None,
            pressure: None,
            pressure_max: None,
            title: None,
            summary: None,
            action_hint: None,
            severity: None,
            warning: false,
            assault_ready: false,
            assault_active: false,
            resolved: false,
            remaining_attackers: None,
            total_attackers: None,
            preparation_seconds_remaining: None,
            preferred_launch_window: None,
            preparation_options: None,
            assault_intents: None,
            continues_while_disconnected: false,
        }
    }

    fn safe_logout_status(state: &str) -> SafeLogoutStatusSnapshot {
        SafeLogoutStatusSnapshot {
            version: 1,
            state: state.to_string(),
            can_request: state == "online",
            can_cancel: state == "pending",
            countdown_total_seconds: (state == "pending").then_some(10),
            countdown_remaining_seconds: (state == "pending").then_some(7),
            reason: None,
            message: "status".to_string(),
            in_own_sanctuary: true,
            active_assault: false,
            protected: state == "protected",
            resumed_from_protection: false,
        }
    }

    fn authenticated_client(
        player_id: i32,
    ) -> (Clients, Uuid, tokio::sync::mpsc::Receiver<String>) {
        let clients = Clients::default();
        let client_id = Uuid::new_v4();
        let (sender, receiver) = tokio::sync::mpsc::channel(8);
        clients
            .lock()
            .unwrap()
            .insert(client_id, Client::new(client_id, player_id, sender));
        (clients, client_id, receiver)
    }

    #[test]
    fn tile_info_handlers_reject_out_of_bounds_coordinates_without_enqueuing() {
        let player_id = 61;
        let (clients, client_id, _client_receiver) = authenticated_client(player_id);
        let (event_sender, event_receiver) = crossbeam_channel::unbounded();
        let sender = AuthorizedPlayerEventSender::new(event_sender, clients, player_id, client_id);

        for (x, y) in [
            (-1, 0),
            (0, -1),
            (crate::map::WIDTH, 0),
            (0, crate::map::HEIGHT),
            (i32::MIN, 0),
            (i32::MAX, 0),
        ] {
            assert!(matches!(
                handle_info_tile(player_id, x, y, sender.clone()),
                ResponsePacket::Error { errmsg }
                    if errmsg == "Invalid tile coordinates."
            ));
            assert!(matches!(
                handle_info_tile_resources(player_id, x, y, sender.clone()),
                ResponsePacket::Error { errmsg }
                    if errmsg == "Invalid tile coordinates."
            ));
        }

        assert!(event_receiver.try_recv().is_err());
        assert_eq!(
            handle_info_tile(player_id, 0, 0, sender),
            ResponsePacket::None
        );
        assert!(matches!(
            event_receiver.try_recv().unwrap(),
            PlayerEvent::InfoTile { x: 0, y: 0, .. }
        ));
    }

    #[test]
    fn websocket_payload_accepts_text_and_rejects_binary_without_utf8_conversion() {
        let text = Message::Text(r#"{"cmd":"info_tile","x":0,"y":0}"#.into());
        assert_eq!(
            websocket_text_payload(&text),
            Some(r#"{"cmd":"info_tile","x":0,"y":0}"#)
        );

        let invalid_utf8 = Message::Binary(vec![0xff, 0xfe, 0xfd].into());
        assert_eq!(websocket_text_payload(&invalid_utf8), None);
    }

    #[test]
    fn connection_liveness_expires_at_thirty_seconds_and_activity_resets_it() {
        assert_eq!(WEBSOCKET_PING_INTERVAL, Duration::from_secs(10));
        assert_eq!(WEBSOCKET_INACTIVITY_TIMEOUT, Duration::from_secs(30));

        let connected_at = Instant::now();
        let mut liveness = ConnectionLiveness::new(connected_at);
        assert!(!liveness.has_timed_out(connected_at + Duration::from_secs(29)));
        assert!(liveness.has_timed_out(connected_at + Duration::from_secs(30)));

        let incoming_frame_at = connected_at + Duration::from_secs(20);
        liveness.record_activity(incoming_frame_at);
        assert_eq!(liveness.deadline(), connected_at + Duration::from_secs(50));
        assert!(!liveness.has_timed_out(connected_at + Duration::from_secs(49)));
        assert!(liveness.has_timed_out(connected_at + Duration::from_secs(50)));
    }

    #[test]
    fn full_authoritative_queue_signals_independent_exact_connection_termination() {
        let player_id = 64;
        let client_id = Uuid::new_v4();
        let clients = Clients::default();
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        sender.try_send("occupied".to_string()).unwrap();
        let (client, mut termination_receiver) =
            Client::with_termination_channel(client_id, player_id, sender);
        clients.activate(client);

        assert_eq!(
            clients.try_send_to_player(player_id, "authoritative-delta".to_string()),
            Err(CurrentConnectionSendError::Full)
        );
        assert_eq!(receiver.try_recv().unwrap(), "occupied");
        assert!(termination_receiver.has_changed().unwrap());
        assert_eq!(
            *termination_receiver.borrow_and_update(),
            Some(AuthoritativeDeliveryFailure::QueueFull)
        );
        assert!(!clients.is_current_connection(player_id, client_id));

        assert_eq!(
            clients.try_send_to_player(player_id, "second-delta".to_string()),
            Err(CurrentConnectionSendError::Closed)
        );
        assert!(!termination_receiver.has_changed().unwrap());
    }

    #[test]
    fn closed_authoritative_queue_signals_independent_exact_connection_termination() {
        let player_id = 65;
        let client_id = Uuid::new_v4();
        let clients = Clients::default();
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        drop(receiver);
        let (client, mut termination_receiver) =
            Client::with_termination_channel(client_id, player_id, sender);
        clients.activate(client);

        assert_eq!(
            clients.try_send_to_player(player_id, "authoritative-delta".to_string()),
            Err(CurrentConnectionSendError::Closed)
        );
        assert_eq!(
            *termination_receiver.borrow_and_update(),
            Some(AuthoritativeDeliveryFailure::QueueClosed)
        );
        assert!(!clients.is_current_connection(player_id, client_id));
    }

    #[test]
    fn connection_cleanup_removes_both_client_and_stream_entries() {
        let player_id = 62;
        let (clients, client_id, _client_receiver) = authenticated_client(player_id);
        let streams = Streams(Arc::new(Mutex::new(HashMap::new())));
        let (manager_sender, _manager_receiver) = tokio::sync::mpsc::channel(1);
        streams.0.lock().unwrap().insert(
            client_id,
            Stream {
                player_id,
                client_id,
                sender: manager_sender,
            },
        );

        cleanup_connection(client_id, &clients, &streams);

        assert!(clients.active_connection_ids(player_id).is_empty());
        assert!(!streams.0.lock().unwrap().contains_key(&client_id));
    }

    #[test]
    fn stale_connection_cleanup_cannot_remove_its_replacement() {
        let player_id = 63;
        let (clients, stale_id, stale_client_receiver) = authenticated_client(player_id);
        let streams = Streams(Arc::new(Mutex::new(HashMap::new())));
        let (stale_manager_sender, stale_manager_receiver) = tokio::sync::mpsc::channel(1);
        streams.0.lock().unwrap().insert(
            stale_id,
            Stream {
                player_id,
                client_id: stale_id,
                sender: stale_manager_sender,
            },
        );

        let replacement_id = Uuid::new_v4();
        let (replacement_client_sender, replacement_client_receiver) =
            tokio::sync::mpsc::channel(1);
        assert_eq!(
            clients.activate(Client::new(
                replacement_id,
                player_id,
                replacement_client_sender
            )),
            vec![stale_id]
        );
        let (replacement_manager_sender, replacement_manager_receiver) =
            tokio::sync::mpsc::channel(1);
        streams.0.lock().unwrap().insert(
            replacement_id,
            Stream {
                player_id,
                client_id: replacement_id,
                sender: replacement_manager_sender,
            },
        );

        cleanup_connection(stale_id, &clients, &streams);

        assert_eq!(
            clients.current_connection_id(player_id),
            Some(replacement_id)
        );
        let streams = streams.0.lock().unwrap();
        assert!(!streams.contains_key(&stale_id));
        assert!(streams.contains_key(&replacement_id));
        assert!(stale_client_receiver.is_closed());
        assert!(stale_manager_receiver.is_closed());
        assert!(!replacement_client_receiver.is_closed());
        assert!(!replacement_manager_receiver.is_closed());
    }

    #[test]
    fn test_is_inappropriate() {
        let hero_name = "Fuck";
        assert!(hero_name.is_inappropriate());
    }

    #[test]
    fn checkpoint4_crisis_status_uses_stable_tag_and_flat_payload() {
        let mut status = no_crisis_status();
        status.exists = true;
        status.kind = Some("goblin".to_string());
        status.phase = Some("assault_active".to_string());
        status.pressure = Some(91);
        status.pressure_max = Some(100);
        status.title = Some("Settlement Under Attack".to_string());
        status.summary = Some("Goblin raiders are attacking your settlement.".to_string());
        status.action_hint = Some(
            "Defeat the remaining attackers. This assault continues if you disconnect.".to_string(),
        );
        status.severity = Some("crisis".to_string());
        status.warning = true;
        status.assault_active = true;
        status.remaining_attackers = Some(2);
        status.total_attackers = Some(3);
        status.continues_while_disconnected = true;

        let value = serde_json::to_value(ResponsePacket::CrisisStatus { status }).unwrap();

        assert_eq!(value["packet"], "crisis_status");
        assert_eq!(value["version"], 1);
        assert_eq!(value["phase"], "assault_active");
        assert_eq!(value["remaining_attackers"], 2);
        assert_eq!(value["continues_while_disconnected"], true);
        assert!(value.get("status").is_none(), "payload must remain flat");
    }

    #[test]
    fn checkpoint4_no_crisis_status_serializes_as_a_clear_state() {
        let value = serde_json::to_value(ResponsePacket::CrisisStatus {
            status: no_crisis_status(),
        })
        .unwrap();

        assert_eq!(value["packet"], "crisis_status");
        assert_eq!(value["version"], 1);
        assert_eq!(value["exists"], false);
        assert_eq!(value["warning"], false);
        assert!(value.get("kind").is_none());
        assert!(value.get("phase").is_none());
        assert!(value.get("pressure").is_none());
        assert!(value.get("remaining_attackers").is_none());
        assert!(value.get("preparation_options").is_none());
        assert!(value.get("assault_intents").is_none());
    }

    #[test]
    fn goblin_assault_intents_are_additive_flat_v1_fields() {
        let mut status = no_crisis_status();
        status.exists = true;
        status.kind = Some("goblin".to_string());
        status.phase = Some("assault_active".to_string());
        status.assault_active = true;
        status.assault_intents = Some(vec![CrisisAssaultIntent {
            role: "hunter".to_string(),
            label: "Hunter Rider".to_string(),
            intent: "Hunts your hero.".to_string(),
        }]);

        let value = serde_json::to_value(ResponsePacket::CrisisStatus { status }).unwrap();

        assert_eq!(value["packet"], "crisis_status");
        assert_eq!(value["version"], 1);
        assert_eq!(value["assault_intents"][0]["role"], "hunter");
        assert_eq!(value["assault_intents"][0]["label"], "Hunter Rider");
        assert!(value.get("status").is_none(), "payload must remain flat");
    }

    #[test]
    fn checkpoint3_preparation_options_are_additive_flat_v1_fields() {
        let mut status = no_crisis_status();
        status.exists = true;
        status.phase = Some("preparing".to_string());
        status.preparation_options = Some(vec![CrisisPreparationOption {
            id: "defences".to_string(),
            label: "Defences".to_string(),
            state: "needs_attention".to_string(),
            detail: "One completed wall is damaged.".to_string(),
            action_hint: "Order a living villager to repair it.".to_string(),
        }]);

        let encoded = serde_json::to_string(&ResponsePacket::CrisisStatus { status }).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();

        assert_eq!(value["packet"], "crisis_status");
        assert_eq!(value["version"], 1);
        assert_eq!(value["preparation_options"][0]["id"], "defences");
        assert_eq!(value["preparation_options"][0]["state"], "needs_attention");
        assert!(value.get("status").is_none(), "payload must remain flat");

        let decoded: ResponsePacket = serde_json::from_str(&encoded).unwrap();
        assert!(matches!(
            decoded,
            ResponsePacket::CrisisStatus {
                status: CrisisStatusSnapshot {
                    preparation_options: Some(options),
                    ..
                }
            } if options.len() == 1 && options[0].id == "defences"
        ));
    }

    #[test]
    fn safe_logout_checkpoint3_request_commands_use_stable_tags_without_player_ids() {
        assert!(matches!(
            decode_network_packet(r#"{"cmd":"request_safe_logout"}"#).unwrap(),
            NetworkPacket::RequestSafeLogout
        ));
        assert!(matches!(
            decode_network_packet(r#"{"cmd":"cancel_safe_logout"}"#).unwrap(),
            NetworkPacket::CancelSafeLogout
        ));

        let request = serde_json::to_value(NetworkPacket::RequestSafeLogout).unwrap();
        let cancel = serde_json::to_value(NetworkPacket::CancelSafeLogout).unwrap();
        assert_eq!(request, serde_json::json!({"cmd": "request_safe_logout"}));
        assert_eq!(cancel, serde_json::json!({"cmd": "cancel_safe_logout"}));
        assert!(request.get("player_id").is_none());
        assert!(cancel.get("player_id").is_none());
        assert!(decode_network_packet(r#"{"cmd":"request_safe_logout","player_id":999}"#).is_err());
        assert!(decode_network_packet(r#"{"cmd":"cancel_safe_logout","player_id":999}"#).is_err());
    }

    #[test]
    fn loot_all_request_uses_one_source_and_target_packet_without_a_player_id() {
        assert!(matches!(
            decode_network_packet(r#"{"cmd":"loot_all","source_id":41,"target_id":7}"#).unwrap(),
            NetworkPacket::LootAll {
                source_id: 41,
                target_id: 7
            }
        ));

        let value = serde_json::to_value(NetworkPacket::LootAll {
            source_id: 41,
            target_id: 7,
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({"cmd": "loot_all", "source_id": 41, "target_id": 7})
        );
        assert!(value.get("player_id").is_none());
    }

    #[test]
    fn gather_request_carries_the_explicit_resource_category() {
        assert!(matches!(
            decode_network_packet(r#"{"cmd":"gather","res_type":"Forage"}"#).unwrap(),
            NetworkPacket::Gather { res_type } if res_type == "Forage"
        ));

        let value = serde_json::to_value(NetworkPacket::Gather {
            res_type: "Forage".to_string(),
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({"cmd": "gather", "res_type": "Forage"})
        );
        assert!(decode_network_packet(r#"{"cmd":"gather"}"#).is_err());
    }

    #[test]
    fn drop_item_request_is_rejected_while_ground_drops_are_disabled() {
        assert!(decode_network_packet(r#"{"cmd":"drop_item","item_id":41}"#).is_err());
    }

    #[test]
    fn craft_requests_accept_legacy_common_and_explicit_signature_forms() {
        assert!(matches!(
            decode_network_packet(r#"{"cmd":"craft","recipe":"Bone Dagger"}"#).unwrap(),
            NetworkPacket::Craft {
                recipe,
                signature_item_id: None,
            } if recipe == "Bone Dagger"
        ));
        assert!(matches!(
            decode_network_packet(
                r#"{"cmd":"structure_craft","structure_id":41,"recipe":"Copper Spear","signature_item_id":99}"#
            )
            .unwrap(),
            NetworkPacket::StructureCraft {
                structure_id: 41,
                recipe,
                signature_item_id: Some(99),
            } if recipe == "Copper Spear"
        ));
    }

    #[test]
    fn safe_logout_checkpoint3_status_is_flat_versioned_and_omits_optional_fields() {
        let value = serde_json::to_value(ResponsePacket::SafeLogoutStatus {
            status: safe_logout_status("online"),
        })
        .unwrap();

        assert_eq!(value["packet"], "safe_logout_status");
        assert_eq!(value["version"], 1);
        assert_eq!(value["state"], "online");
        assert_eq!(value["can_request"], true);
        assert_eq!(value["protected"], false);
        assert_eq!(value["resumed_from_protection"], false);
        assert!(value.get("status").is_none());
        assert!(value.get("countdown_total_seconds").is_none());
        assert!(value.get("countdown_remaining_seconds").is_none());
        assert!(value.get("reason").is_none());
    }

    #[test]
    fn safe_logout_checkpoint3_all_stable_states_serialize() {
        for state in ["online", "pending", "protected", "disconnected"] {
            let value = serde_json::to_value(ResponsePacket::SafeLogoutStatus {
                status: safe_logout_status(state),
            })
            .unwrap();
            assert_eq!(value["state"], state);
        }
    }

    #[test]
    fn protected_settlements_packet_is_flat_versioned_and_round_trips() {
        let packet = ResponsePacket::ProtectedSettlements {
            version: 1,
            settlements: vec![
                ProtectedSettlementSnapshot {
                    player_id: 7,
                    monolith_id: 701,
                    sanctuary_radius: 6,
                },
                ProtectedSettlementSnapshot {
                    player_id: 12,
                    monolith_id: 1201,
                    sanctuary_radius: 8,
                },
            ],
        };
        let encoded = serde_json::to_string(&packet).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "packet": "protected_settlements",
                "version": 1,
                "settlements": [
                    {
                        "player_id": 7,
                        "monolith_id": 701,
                        "sanctuary_radius": 6
                    },
                    {
                        "player_id": 12,
                        "monolith_id": 1201,
                        "sanctuary_radius": 8
                    }
                ]
            })
        );

        assert_eq!(
            serde_json::from_str::<ResponsePacket>(&encoded).unwrap(),
            packet
        );

        let empty = serde_json::to_value(ResponsePacket::ProtectedSettlements {
            version: 1,
            settlements: Vec::new(),
        })
        .unwrap();
        assert_eq!(empty["settlements"], serde_json::json!([]));
    }

    #[test]
    fn sanctuary_state_packet_is_flat_versioned_and_round_trips() {
        let packet = ResponsePacket::SanctuaryState {
            version: 1,
            zones: vec![SanctuaryZoneSnapshot {
                monolith_id: 701,
                radius: 7,
            }],
        };
        let encoded = serde_json::to_string(&packet).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "packet": "sanctuary_state",
                "version": 1,
                "zones": [{
                    "monolith_id": 701,
                    "radius": 7
                }]
            })
        );
        assert_eq!(
            serde_json::from_str::<ResponsePacket>(&encoded).unwrap(),
            packet
        );

        let empty = serde_json::to_value(ResponsePacket::SanctuaryState {
            version: 1,
            zones: Vec::new(),
        })
        .unwrap();
        assert_eq!(empty["zones"], serde_json::json!([]));
    }

    #[test]
    fn connection_authority_activation_displaces_old_session_atomically() {
        let player_id = 40;
        let clients = Clients::default();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let (first_sender, _first_receiver) = tokio::sync::mpsc::channel(1);
        let (second_sender, _second_receiver) = tokio::sync::mpsc::channel(1);

        assert!(clients
            .activate(Client::new(first_id, player_id, first_sender))
            .is_empty());
        assert!(clients.is_current_connection(player_id, first_id));

        assert_eq!(
            clients.activate(Client::new(second_id, player_id, second_sender)),
            vec![first_id]
        );
        assert!(!clients.is_current_connection(player_id, first_id));
        assert!(clients.is_current_connection(player_id, second_id));
        assert_eq!(
            authenticated_player_for_connection(first_id, &clients),
            None
        );
        assert_eq!(
            authenticated_player_for_connection(second_id, &clients),
            Some(player_id)
        );

        // Cleanup from the displaced socket cannot remove the replacement.
        assert!(clients.remove_if_current(first_id).is_none());
        assert!(clients.is_current_connection(player_id, second_id));
        assert!(clients.remove_if_current(second_id).is_some());
        assert!(!clients.is_player_online(player_id));
    }

    #[test]
    fn connection_authority_duplicate_registry_state_fails_closed() {
        let player_id = 44;
        let clients = Clients::default();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let (first_sender, _first_receiver) = tokio::sync::mpsc::channel(1);
        let (second_sender, _second_receiver) = tokio::sync::mpsc::channel(1);
        clients.lock().unwrap().extend([
            (first_id, Client::new(first_id, player_id, first_sender)),
            (second_id, Client::new(second_id, player_id, second_sender)),
        ]);

        assert!(!clients.is_current_connection(player_id, first_id));
        assert!(!clients.is_current_connection(player_id, second_id));
        assert_eq!(
            authenticated_player_for_connection(first_id, &clients),
            None
        );
        assert_eq!(
            authenticated_player_for_connection(second_id, &clients),
            None
        );
    }

    #[test]
    fn connection_authority_near_simultaneous_replacements_leave_one_controller() {
        let player_id = 45;
        let clients = Clients::default();
        let original_id = Uuid::new_v4();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let (original_sender, _original_receiver) = tokio::sync::mpsc::channel(1);
        let (first_sender, _first_receiver) = tokio::sync::mpsc::channel(1);
        let (second_sender, _second_receiver) = tokio::sync::mpsc::channel(1);
        clients.activate(Client::new(original_id, player_id, original_sender));

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let first_clients = clients.clone();
        let first_barrier = barrier.clone();
        let first = std::thread::spawn(move || {
            first_barrier.wait();
            first_clients.activate(Client::new(first_id, player_id, first_sender))
        });
        let second_clients = clients.clone();
        let second_barrier = barrier.clone();
        let second = std::thread::spawn(move || {
            second_barrier.wait();
            second_clients.activate(Client::new(second_id, player_id, second_sender))
        });

        barrier.wait();
        first.join().unwrap();
        second.join().unwrap();

        let first_is_current = clients.is_current_connection(player_id, first_id);
        let second_is_current = clients.is_current_connection(player_id, second_id);
        assert_ne!(first_is_current, second_is_current);
        assert!(!clients.is_current_connection(player_id, original_id));
        assert_eq!(clients.active_connection_ids(player_id).len(), 1);
    }

    #[test]
    fn connection_authority_event_enqueue_is_atomic_with_replacement() {
        let player_id = 46;
        let displaced_id = Uuid::new_v4();
        let replacement_id = Uuid::new_v4();
        let clients = Clients::default();
        let (network_sender, network_receiver) = crossbeam_channel::unbounded();
        let (displaced_sender, _displaced_receiver) = tokio::sync::mpsc::channel(1);
        clients.activate(Client::new(displaced_id, player_id, displaced_sender));
        let displaced_events = AuthorizedPlayerEventSender::new(
            network_sender.clone(),
            clients.clone(),
            player_id,
            displaced_id,
        );

        let (replacement_sender, _replacement_receiver) = tokio::sync::mpsc::channel(1);
        clients.activate(Client::new(replacement_id, player_id, replacement_sender));
        let replacement_events =
            AuthorizedPlayerEventSender::new(network_sender, clients, player_id, replacement_id);

        displaced_events
            .send(PlayerEvent::Move {
                player_id,
                x: 1,
                y: 2,
            })
            .unwrap();
        assert!(
            network_receiver.try_recv().is_err(),
            "a displaced socket must not enqueue even a connectionless gameplay event"
        );
        assert!(!displaced_events.send_strict(PlayerEvent::Move {
            player_id,
            x: 3,
            y: 4,
        }));

        replacement_events
            .send(PlayerEvent::Move {
                player_id,
                x: 5,
                y: 6,
            })
            .unwrap();
        assert!(matches!(
            network_receiver.try_recv().unwrap(),
            PlayerEvent::Move { x: 5, y: 6, .. }
        ));
    }

    #[test]
    fn connection_authority_async_operation_has_an_explicit_linearization_point() {
        let player_id = 47;
        let accepted_id = Uuid::new_v4();
        let replacement_id = Uuid::new_v4();
        let clients = Clients::default();
        let (network_sender, network_receiver) = crossbeam_channel::unbounded();
        let (accepted_sender, _accepted_receiver) = tokio::sync::mpsc::channel(1);
        clients.activate(Client::new(accepted_id, player_id, accepted_sender));
        let accepted_events = AuthorizedPlayerEventSender::new(
            network_sender,
            clients.clone(),
            player_id,
            accepted_id,
        );
        let operation = accepted_events
            .begin_operation()
            .expect("operation accepted before replacement");

        let (replacement_sender, _replacement_receiver) = tokio::sync::mpsc::channel(1);
        clients.activate(Client::new(replacement_id, player_id, replacement_sender));
        assert!(accepted_events.begin_operation().is_none());

        operation
            .send(PlayerEvent::NewPlayer {
                player_id,
                hero_name: "Linearized".to_string(),
                class_name: "Warrior".to_string(),
                portrait: crate::obj::default_hero_portrait().to_string(),
            })
            .unwrap();
        assert!(matches!(
            network_receiver.try_recv().unwrap(),
            PlayerEvent::NewPlayer { hero_name, .. } if hero_name == "Linearized"
        ));
    }

    #[test]
    fn safe_logout_checkpoint3_authenticated_routing_uses_connection_owner_only() {
        let authenticated_player = 41;
        let (clients, client_id, _client_receiver) = authenticated_client(authenticated_player);
        let (sender, receiver) = crossbeam_channel::unbounded();
        let sender = AuthorizedPlayerEventSender::new(
            sender,
            clients.clone(),
            authenticated_player,
            client_id,
        );

        assert_eq!(
            handle_safe_logout_command(client_id, &clients, sender, false),
            ResponsePacket::None
        );
        assert!(matches!(
            receiver.try_recv().unwrap(),
            PlayerEvent::RequestSafeLogout {
                player_id,
                connection_id,
            } if player_id == authenticated_player && connection_id == client_id
        ));
    }

    #[test]
    fn safe_logout_checkpoint3_cancel_is_owner_scoped() {
        let authenticated_player = 52;
        let (clients, client_id, _client_receiver) = authenticated_client(authenticated_player);
        let (sender, receiver) = crossbeam_channel::unbounded();
        let sender = AuthorizedPlayerEventSender::new(
            sender,
            clients.clone(),
            authenticated_player,
            client_id,
        );

        assert_eq!(
            handle_safe_logout_command(client_id, &clients, sender, true),
            ResponsePacket::None
        );
        assert!(matches!(
            receiver.try_recv().unwrap(),
            PlayerEvent::CancelSafeLogout {
                player_id,
                connection_id,
            } if player_id == authenticated_player && connection_id == client_id
        ));
    }

    #[test]
    fn safe_logout_checkpoint3_unauthenticated_and_stale_mappings_fail_closed() {
        let clients = Clients::default();
        let missing_id = Uuid::new_v4();
        let (sender, receiver) = crossbeam_channel::unbounded();
        let sender = AuthorizedPlayerEventSender::new(sender, clients.clone(), -1, missing_id);
        assert!(matches!(
            handle_safe_logout_command(missing_id, &clients, sender, false),
            ResponsePacket::Error { .. }
        ));
        assert!(receiver.try_recv().is_err());

        let map_key = Uuid::new_v4();
        let mismatched_id = Uuid::new_v4();
        let (client_sender, _client_receiver) = tokio::sync::mpsc::channel(1);
        clients
            .lock()
            .unwrap()
            .insert(map_key, Client::new(mismatched_id, 12, client_sender));
        let (sender, receiver) = crossbeam_channel::unbounded();
        let sender = AuthorizedPlayerEventSender::new(sender, clients.clone(), 12, map_key);
        assert!(matches!(
            handle_safe_logout_command(map_key, &clients, sender, true),
            ResponsePacket::Error { .. }
        ));
        assert!(receiver.try_recv().is_err());

        let closed_id = Uuid::new_v4();
        let (closed_sender, closed_receiver) = tokio::sync::mpsc::channel(1);
        drop(closed_receiver);
        clients
            .lock()
            .unwrap()
            .insert(closed_id, Client::new(closed_id, 13, closed_sender));
        let (sender, receiver) = crossbeam_channel::unbounded();
        let sender = AuthorizedPlayerEventSender::new(sender, clients.clone(), 13, closed_id);
        assert!(matches!(
            handle_safe_logout_command(closed_id, &clients, sender, false),
            ResponsePacket::Error { .. }
        ));
        assert!(receiver.try_recv().is_err());
    }
}
