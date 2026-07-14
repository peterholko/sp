//import {startGame} from './game'
import { Global } from './global'
import { NetworkEvent } from './networkEvent';
import { ObjectState } from './objectState';
import { TileState } from './tileState';
import { GameEvent } from './gameEvent';
import { DEAD, NONE } from "./config";
import { WeatherState } from './weatherState';
import type { CrisisStatusPacket } from './crisisStatus';
import {
  SAFE_LOGOUT_COMPLETION_MESSAGE,
  SafeLogoutCloseGuard,
  SafeLogoutStatusPacket,
  cancelSafeLogoutPacket,
  clearSafeLogoutReconnectSuppression,
  dispatchSafeLogoutStatus,
  rememberSafeLogoutCompletion,
  requestSafeLogoutPacket,
} from './safeLogoutStatus';

export type { CrisisStatusPacket } from './crisisStatus';
export type { SafeLogoutStatusPacket } from './safeLogoutStatus';

export type NetworkPacket =
  | { cmd: 'select_class'; class_name: string; hero_name: string }
  | { cmd: 'recreate_hero' }
  | { cmd: 'get_stats'; id: number }
  | { cmd: 'image_def'; name: string }
  | { cmd: 'move_unit'; x: number; y: number }
  | { cmd: 'attack'; attack_type: string; source_id: number; target_id: number }
  | { cmd: 'ability'; ability_id: string; source_id: number; target_id?: number }
  | { cmd: 'combo'; source_id: number; target_id: number; combo_type: string }
  | { cmd: 'block'; source_id: number }
  | { cmd: 'info_obj'; id: number }
  | { cmd: 'info_skills'; id: number }
  | { cmd: 'info_attrs'; id: number }
  | { cmd: 'info_advance'; source_id: number }
  | { cmd: 'info_upgrade'; structure_id: number }
  | { cmd: 'info_tile'; x: number; y: number }
  | { cmd: 'info_tile_resources'; x: number; y: number }
  | { cmd: 'info_inventory'; id: number }
  | { cmd: 'info_equip'; id: number }
  | { cmd: 'info_item'; obj_id: number; item_id: number; action: string }
  | { cmd: 'info_item_by_name'; name: string }
  | { cmd: 'info_item_transfer'; source_id: number; target_id: number }
  | { cmd: 'info_exit'; id: number; panel_type: string }
  | { cmd: 'info_merchant'; source_id: number; merchant_id: number }
  | { cmd: 'info_hire'; source_id: number }
  | { cmd: 'item_transfer'; item: number; source_id: number; target_id: number }
  | { cmd: 'item_split'; owner_id: number; item: number; quantity: number }
  | { cmd: 'gather' }
  | { cmd: 'operate'; structure_id: number }
  | { cmd: 'plant'; structure_id: number }
  | { cmd: 'tend'; structure_id: number }
  | { cmd: 'harvest'; structure_id: number }
  | { cmd: 'refine'; item_id: number }
  | { cmd: 'structure_refine'; structure_id: number; item_id: number }
  | { cmd: 'craft'; recipe: string }
  | { cmd: 'structure_craft'; structure_id: number; recipe: string }
  | { cmd: 'sleep'; structure_id: number }
  | { cmd: 'order_follow'; source_id: number }
  | { cmd: 'order_gather'; source_id: number; res_type: string }
  | { cmd: 'order_operate'; source_id: number; structure_id: number }
  | { cmd: 'order_refine'; source_id: number; structure_id: number }
  | { cmd: 'order_craft'; source_id: number; structure_id: number }
  | { cmd: 'order_explore'; source_id: number }
  | { cmd: 'order_prospect'; source_id: number }
  | { cmd: 'order_experiment'; source_id: number; structure_id: number }
  | { cmd: 'order_plant'; source_id: number; structure_id: number }
  | { cmd: 'order_tend'; source_id: number; structure_id: number }
  | { cmd: 'order_harvest'; source_id: number; structure_id: number }
  | { cmd: 'order_repair'; source_id: number }
  | { cmd: 'structure_list' }
  | { cmd: 'create_foundation'; source_id: number; structure: string }
  | { cmd: 'build'; source_id: number; structure_id: number }
  | { cmd: 'start_upgrade'; structure_id: number; selected_upgrade: string }
  | { cmd: 'upgrade'; source_id: number; structure_id: number }
  | { cmd: 'experiment'; structure_id: number }
  | { cmd: 'activate'; structure_id: number }
  | { cmd: 'survey'; source_id: number }
  | { cmd: 'prospect' }
  | { cmd: 'explore' }
  | { cmd: 'investigate'; target_id: number }
  | { cmd: 'nearby_resources' }
  | { cmd: 'info_assign'; structure_id: number }
  | { cmd: 'assign'; worker_id: number; structure_id: number }
  | { cmd: 'remove_assign'; worker_id: number; structure_id: number }
  | { cmd: 'equip'; obj_id: number; item: number; status: boolean }
  | { cmd: 'delete_item'; obj_id: number; item_id: number }
  | { cmd: 'info_craft'; crafter_id: number }
  | { cmd: 'info_structure_craft'; structure_id: number }
  | { cmd: 'info_structure_queue'; structure_id: number }
  | { cmd: 'info_work_queue_entry'; structure_id: number; index: number }
  | { cmd: 'add_crafting_entry'; structure_id: number; recipe_name: string }
  | { cmd: 'add_refine_entry'; structure_id: number; refine_item_id: number }
  | { cmd: 'remove_work_entry'; structure_id: number; index: number }
  | { cmd: 'info_refine'; refiner_id: number }
  | { cmd: 'info_structure_refine'; structure_id: number }
  | { cmd: 'info_structure_refine_item'; structure_id: number; item_id: number }
  | { cmd: 'use'; obj_id: number; item_id: number }
  | { cmd: 'delete'; source_id: number }
  | { cmd: 'advance'; source_id: number }
  | { cmd: 'info_experiment'; structure_id: number }
  | { cmd: 'set_exp_item'; structure_id: number; item_id: number }
  | { cmd: 'set_exp_resource'; structure_id: number; item_id: number }
  | { cmd: 'reset_experiment'; structure_id: number }
  | { cmd: 'hire'; source_id: number; target_id: number }
  | { cmd: 'buy_item'; seller_id: number; item_id: number; quantity: number }
  | { cmd: 'sell_item'; item_id: number; target_id: number; quantity: number }
  | { cmd: 'cancel_action' }
  | { cmd: 'request_safe_logout' }
  | { cmd: 'cancel_safe_logout' }
  | { cmd: 'debug_obj'; obj_id: number }
  | { cmd: 'set_log_level'; target: string; level: string }
  | { cmd: 'get_log_levels' };

export interface StructureList {
  result: Structure[];
}

export type ResponsePacket =
  | { packet: 'select_class'; player: number }
  | { packet: 'info_select_class'; result: string }
  | { packet: 'login'; player: number }
  | { packet: 'disconnect'; player: number; client: string }
  | { packet: 'world'; time_of_day: string; day: number }
  | { packet: 'explored_map'; tiles: MapTile[] }
  | { packet: 'init_perception'; data: PerceptionData }
  | { packet: 'new_perception'; data: PerceptionData }
  | { packet: 'new_obj_perception'; new_objs: MapObj[]; new_tiles: MapTile[] }
  | { packet: 'perception_changes'; events: ChangeEvents[] }
  | { packet: 'stats'; data: StatsData }
  | { packet: 'hero_death_state'; phase: string; hero_id: number; hero_name: string; resurrect_cost: number; soulshards_available: number; seconds_remaining: number; message: string }
  | InfoHeroPacket
  | InfoVillagerPacket
  | InfoStructurePacket
  | { packet: 'info_npc'; id: number; name: string; class: string; subclass: string; template: string; state: string; image: string; hsl: number[]; items?: Item[]; effects: string[] }
  | { packet: 'info_monolith'; id: number; name: string; class: string; subclass: string; template: string; image: string; soulshards: number }
  | { packet: 'info_poi'; id: number; name: string; class: string; subclass: string; template: string; image: string; items?: Item[] }
  | { packet: 'info_obj'; id: number; name: string; class: string; subclass: string; template: string; image: string }
  | { packet: 'info_skills'; id: number; skills: Record<string, Skill> }
  | { packet: 'info_attrs'; id: number; attrs: Record<string, number> }
  | { packet: 'info_advance'; id: number; rank: string; next_rank: string; total_xp: number; req_xp: number }
  | { packet: 'info_upgrade'; id: number; upgrade_list: UpgradeTemplate[] }
  | { packet: 'info_tile'; x: number; y: number; name: string; mc: number; def: number; unrevealed: number; sanctuary: string; passable: boolean; wildness: string; survey_status: string; resources: TileResource[]; terrain_features: TileTerrainFeature[] }
  | { packet: 'info_tile_resources'; x: number; y: number; name: string; resources: TileResource[] }
  | { packet: 'info_inventory'; id: number; cap: number; tw: number; items: Item[] }
  | { packet: 'info_inventory_snapshot'; id: number; cap: number; tw: number; items: Item[] }
  | { packet: 'info_equip'; name: string; template: string; id: number; cap: number; tw: number; items: Item[] }
  | { packet: 'info_item'; action?: string; id: number; owner: number; name: string; quantity: number; durability?: number; class: string; subclass: string; image: string; weight: number; equipped: boolean; price?: number; attrs?: Record<string, AttrVal>; produces?: string[] }
  | { packet: 'info_item_transfer'; source_id: number; sourceitems: Inventory; target_id: number; targetitems: Inventory; reqitems: ResReq[] }
  | { packet: 'info_items_update'; id: number; items_updated: Item[]; items_removed: number[] }
  | { packet: 'info_state_update'; id: number; state: string }
  | { packet: 'info_activity_update'; id: number; activity: string }
  | { packet: 'info_needs_update'; id: number; thirst: string; hunger: string; tiredness: string }
  | { packet: 'info_merchant'; source_id: number; inventory: Inventory; merchant_id: number; merchant_inventory: Inventory; merchant_wanted_items: WantedItem[] }
  | { packet: 'info_hire'; data: HireData[] }
  | { packet: 'item_transfer'; result: string; source_id: number; sourceitems: Inventory; target_id: number; targetitems: Inventory; reqitems: ResReq[] }
  | { packet: 'item_split'; result: string; owner: number }
  | { packet: 'info_experiment'; id: number; expitem: Item[]; expresources: Item[]; validresources: Item[]; expstate: string; recipe?: Recipe }
  | { packet: 'info_experiment_state'; id: number; expstate: string }
  | { packet: 'info_crop'; id: number; crop_type: string; crop_quantity: number; crop_stage: string }
  | { packet: 'nearby_resources'; data: TileResourceWithPos[] }
  | { packet: 'structure_list'; result: Structure[] }
  | { packet: 'image_def'; name: string; data: unknown }
  | { packet: 'PlayerMoved'; player_id: number; x: number; y: number }
  | { packet: 'create_foundation'; result: string }
  | { packet: 'start_upgrade'; structure_id: number }
  | { packet: 'work_update'; structure_id: number; work_done: number; total_work: number; work_per_sec: number }
  | { packet: 'upgrade'; upgrade_time: number }
  | { packet: 'craft'; craft_time: number }
  | { packet: 'refine'; refine_time: number }
  | { packet: 'explore'; explore_time: number }
  | { packet: 'survey'; survey_time: number }
  | { packet: 'prospect'; prospect_time: number }
  | { packet: 'investigate'; investigate_time: number }
  | { packet: 'gather'; gather_time: number }
  | { packet: 'attack'; source_id: number; attack_type: string; cooldown: number; stamina_cost: number }
  | { packet: 'ability'; source_id: number; ability_id: string; cooldown: number; stamina_cost?: number; mana_cost?: number }
  | { packet: 'info_assign'; structure_id: number; assignments: Assignment[] }
  | { packet: 'assign'; result: string }
  | { packet: 'equip'; result: string }
  | { packet: 'info_craft'; crafter_id: number; structure_id?: number; items: Item[]; recipes: Recipe[]; crafting_item?: CraftingItem }
  | { packet: 'info_structure_craft'; structure_inventory: Inventory; recipes?: Recipe[]; queue: WorkEntry[]; crafting_item?: CraftingItem }
  | { packet: 'info_structure_queue'; structure_id: number; queue: WorkEntry[] }
  | { packet: 'info_work_queue_entry'; structure_id: number; work_type: string; index: number; worker_id: number; item_name: string; item_image: string; item_quantity: number; work_time: number; progress: number }
  | { packet: 'info_refine'; refiner_id: number; structure_id?: number; refiner_items: Item[]; structure_items?: Item[]; refining_item?: RefiningItem; produced_items: [number, number][] }
  | { packet: 'info_structure_refine'; structure_inventory: Inventory; refining_item?: RefiningItem; produced_items: [number, number][] }
  | { packet: 'info_refine_item'; id: number; name: string; image: string; class: string; subclass: string; quantity: number; produces: ProducedItem[]; refining_skill: string; refining_skill_req: number; refine_time: number; progress: number }
  | { packet: 'xp'; id: number; xp_list: Xp[] }
  | { packet: 'new_items'; action: string; source_id: number; item_name: string; amount: number }
  | { packet: 'buy_item'; source_id: number; inventory: Inventory; merchant_id: number; merchant_inventory: Inventory }
  | { packet: 'sell_item'; source_id: number; inventory: Inventory; merchant_id: number; merchant_inventory: Inventory; merchant_wanted_items: WantedItem[] }
  | { packet: 'gained_effect'; id: number; x: number; y: number; effect: string }
  | { packet: 'lost_effect'; id: number; x: number; y: number; effect: string }
  | { packet: 'reduced_effect'; id: number; x: number; y: number; label: string; effect: string }
  | { packet: 'increased_effect'; id: number; x: number; y: number; label: string; effect: string }
  | { packet: 'Ok' }
  | { packet: 'None' }
  | { packet: 'Pong' }
  | { packet: 'Error'; errmsg: string }
  | { packet: 'Notice'; noticemsg: string; expiry?: number | null }
  | { packet: 'combat_telegraph'; attacker_id: number; attacker_name: string; attack_type: string; defense_hint: string; strike_in: number }
  | { packet: 'info_true_death'; hero_name: string; hero_rank: string; total_xp: number; score_total: number; score_breakdown: ScoreBreakdown; days_survived: number; waves_survived: number; highest_pressure_level: number; legendary_kills: number; hideouts_cleared: number; fate: string; crisis_tier: number }
  | { packet: 'debug_obj'; obj_id: number; enabled: boolean }
  | { packet: 'log_level_set'; target: string; level: string; success: boolean }
  | { packet: 'log_levels'; overrides: Array<[string, string]> }
  | { packet: 'objective_state'; version: number; current_id: string; objectives: ObjectiveProgress[] }
  | { packet: 'threat_state'; version: number; day: number; phase: string; pressure_level: string; next_night_warning: string; known_risks: ThreatRisk[]; legendary_threats: LegendaryThreat[] }
  | CrisisStatusPacket
  | SafeLogoutStatusPacket
  | { packet: 'combat_state'; version: number; target_id: number; enemy_intent: string; attack_history: string[]; matching_combos: ComboHint[]; available_finisher?: string; stamina_costs: StaminaCosts; abilities?: AbilityHint[]; counter_hint: string }
  | { packet: 'discovery_event'; version: number; discovery_type: string; title: string; unlock_source: string; location?: string; result: string };

export interface PerceptionData {
  map: MapTile[];
  observers: MapObj[];
  visible_objs: MapObj[];
  weather: MapWeather[];
}

export interface ObjectiveProgress {
  id: string;
  title: string;
  state: string;
  category: string;
  target?: string;
  action_hint: string;
  lesson: string;
  reward: string;
  progress?: number;
  goal?: number;
}

export interface ThreatRisk {
  id: string;
  label: string;
  severity: string;
  trigger_hint: string;
  counter_hint: string;
  current?: number;
  threshold?: number;
}

export interface LegendaryThreat {
  name: string;
  status: string;
  days_active: number;
  hideout_known: boolean;
  hideout_location?: string;
  next_attack_eta?: number;
  followers_defeated: number;
  captains_defeated: number;
}

export interface ScoreBreakdown {
  survival: number;
  progression: number;
  wealth: number;
  defense: number;
  valor: number;
  legacy: number;
}

export interface ComboHint {
  name: string;
  remaining_attacks: string[];
  effect?: string;
}

export interface StaminaCosts {
  quick: number;
  precise: number;
  fierce: number;
  block: number;
}

export interface AbilityHint {
  id: string;
  label: string;
  cost_type: string;
  cost: number;
  range: number;
  disabled_reason?: string;
  hint: string;
}

export type ChangeEvents =
  | { event: string; obj: MapObj }
  | { event: string; obj_id: number; attr: string; value: string }
  | { event: string; obj: MapObj; src_x: number; src_y: number }
  | { event: string; obj_id: number };

export interface StatsData {
  id: number;
  hp: number;
  base_hp: number;
  stamina: number;
  base_stamina: number;
  mana: number;
  base_mana: number;
  thirst?: string;
  hunger?: string;
  tiredness?: string;
  effects: number[];
}

export type BroadcastEvents =
  | { packet: 'dmg'; source_id: number; target_id: number; attack_type: string; dmg: number; state: string; combo?: string; countered?: string; missed?: boolean }
  | { packet: 'spoil'; source_id: number; target_id: number; itemtype: string; itemquantity: number }
  | { packet: 'steal'; source_id: number; target_id: number }
  | { packet: 'torch'; source_id: number; target_id: number }
  | { packet: 'speech'; source: number; speech: string }
  | { packet: 'sound'; x: number; y: number; sound: string };

export interface MapObj {
  id: number;
  player: number;
  name: string;
  class: string;
  subclass: string;
  template: string;
  image: string;
  x: number;
  y: number;
  state: string;
  vision?: number;
  hsl: number[];
  groups: string[];
  work_done?: number;
  total_work?: number;
  work_per_sec?: number;
}

export interface MapWeather {
  x: number;
  y: number;
  weather: string;
}

export interface MapTile {
  x: number;
  y: number;
  t: unknown;
}

export interface Inventory {
  id: number;
  cap: number;
  tw: number;
  items: Item[];
}

export interface Item {
  id: number;
  name: string;
  quantity: number;
  durability?: number;
  owner: number;
  class: string;
  subclass: string;
  slot?: string;
  image: string;
  weight: number;
  equipped: boolean;
  refineable: boolean;
  attrs?: Record<string, AttrVal>;
}

export interface CraftingItem {
  name: string;
  image: string;
  class: string;
  subclass: string;
  crafting_time: number;
  progress: number;
}

export interface RefiningItem {
  id: number;
  name: string;
  image: string;
  class: string;
  subclass: string;
  quantity: number;
  produces: ProducedItem[];
  refining_skill: string;
  refine_time: number;
  progress: number;
}

export interface ProducedItem {
  name: string;
  image: string;
  class: string;
  subclass: string;
}

export interface Structure {
  name: string;
  image: string;
  class: string;
  subclass: string;
  template: string;
  base_hp: number;
  base_def: number;
  build_time: number;
  req: ResReq[];
  upgrade_req: ResReq[];
}

export interface Assignment {
  id: number;
  name: string;
  image: string;
  structure_id: number;
  structure_name?: string;
}

export interface Recipe {
  name: string;
  image: string;
  class: string;
  subclass: string;
  tier?: number;
  slot?: string;
  damage?: number;
  speed?: number;
  armor?: number;
  stamina_req?: number;
  crafting_time?: number;
  skill_req?: number;
  weight: number;
  amount?: number;
  req: ResReq[];
}

export interface WorkEntry {
  work_type: string;
  villager_id: number;
  recipe_name?: string;
  recipe_image?: string;
  refine_item_id?: number;
  refine_item_image?: string;
  refine_item_class?: string;
  work_time: number;
  progress: number;
}

export interface Skill {
  level: number;
  xp: number;
  next: number;
}

export interface Xp {
  skill: string;
  xp: number;
  levelup?: number;
}

export interface TileResource {
  name: string;
  image: string;
  color: number;
  yield_label: string;
  quantity_label: string;
  properties: Property[];
}

export interface Property {
  name: string;
  value: number;
}

export interface TileTerrainFeature {
  name: string;
  image: string;
  bonus: string;
}

export interface TileResourceWithPos {
  name: string;
  color: number;
  yield_label: string;
  quantity_label: string;
  x: number;
  y: number;
}

export interface HireData {
  id: number;
  name: string;
  image: string;
  wage: number;
  creativity: number;
  dexterity: number;
  endurance: number;
  focus: number;
  intellect: number;
  spirit: number;
  strength: number;
  toughness: number;
  skills: Record<string, number>;
}

export interface UpgradeTemplate {
  name: string;
  template: string;
  req: ResReq[];
  build_time: number;
}

export interface ResReq {
  type: string;
  quantity: number;
  cquantity?: number;
}

export interface WantedItem {
  itemName: string;
  price: number;
  quantity: number;
  class?: string;
  subclass?: string;
  name?: string;
}

export type AttrVal = string | number | boolean | null;

export type EffectInfo = Record<string, unknown>;

export interface InfoHeroPacket {
  packet: 'info_hero';
  id: number;
  name: string;
  class: string;
  subclass: string;
  template: string;
  state: string;
  image: string;
  hsl: number[];
  items?: Item[];
  skills?: Record<string, number>;
  attributes?: Record<string, number>;
  effects: EffectInfo[];
  hp?: number;
  stamina?: number;
  mana?: number;
  thirst: string;
  hunger: string;
  tiredness: string;
  base_hp?: number;
  base_stamina?: number;
  base_mana?: number;
  hero_class?: string;
  base_def?: number;
  base_vision?: number;
  base_speed?: number;
  base_dmg?: number;
  dmg_range?: number;
  total_dmg?: number;
  total_def?: number;
  vision?: number;
}

export interface InfoVillagerPacket {
  packet: 'info_villager';
  id: number;
  name: string;
  class: string;
  subclass: string;
  template: string;
  state: string;
  image: string;
  hsl: number[];
  items?: Item[];
  skills?: Record<string, number>;
  attributes?: Record<string, number>;
  effects?: string[];
  need: string;
  thirst: string;
  hunger: string;
  tiredness: string;
  hp?: number;
  stamina?: number;
  base_hp?: number;
  base_stamina?: number;
  base_def?: number;
  base_vision?: number;
  base_speed?: number;
  base_dmg?: number;
  dmg_range?: number;
  vision?: number;
  structure?: string;
  activity?: string;
  shelter?: string;
  morale?: string;
  order?: string;
  capacity?: number;
  total_weight?: number;
}

export interface InfoStructurePacket {
  packet: 'info_structure';
  id: number;
  name: string;
  class: string;
  subclass: string;
  template: string;
  x: number;
  y: number;
  state: string;
  image: string;
  hsl: number[];
  items?: Item[];
  hp?: number;
  base_hp?: number;
  base_def?: number;
  capacity?: number;
  total_weight?: number;
  workspaces?: number;
  effects?: string[];
  build_cost?: number;
  work_done?: number;
  work_per_sec?: number;
  req?: ResReq[];
  upgrade_req?: ResReq[];
  selected_upgrade?: string;
  crop_type?: string;
  crop_quantity?: number;
  crop_stage?: string;
}

export class Network {

  private websocket;
  private networkErrorTimeoutId: number | null = null;
  private readonly networkErrorGraceMs = 1500;
  private readonly safeLogoutCloseGuard = new SafeLogoutCloseGuard();
  private reloadAfterSafeLogoutClose = false;

  private clearNetworkErrorTimeout() {
    if (this.networkErrorTimeoutId !== null) {
      window.clearTimeout(this.networkErrorTimeoutId);
      this.networkErrorTimeoutId = null;
    }
  }

  private scheduleNetworkError(socket) {
    this.clearNetworkErrorTimeout();

    this.networkErrorTimeoutId = window.setTimeout(() => {
      this.networkErrorTimeoutId = null;

      if (socket !== this.websocket) {
        return;
      }

      if (this.websocket && this.websocket.readyState === WebSocket.OPEN) {
        return;
      }

      if (document.visibilityState !== 'visible') {
        this.scheduleNetworkError(socket);
        return;
      }

      Global.gameEmitter.emit(NetworkEvent.NETWORK_ERROR);
    }, this.networkErrorGraceMs);
  }

  private scheduleServerOffline(socket) {
    this.clearNetworkErrorTimeout();

    this.networkErrorTimeoutId = window.setTimeout(() => {
      this.networkErrorTimeoutId = null;

      if (socket !== this.websocket) {
        return;
      }

      if (this.websocket && this.websocket.readyState === WebSocket.OPEN) {
        return;
      }

      if (document.visibilityState !== 'visible') {
        this.scheduleServerOffline(socket);
        return;
      }

      Global.gameEmitter.emit(NetworkEvent.SERVER_OFFLINE);
    }, this.networkErrorGraceMs);
  }

  public sendMessage(message: String) {
    this.websocket.send(message);
  }

  /*private safeSend(data: any) {
    if (this.websocket.readyState !== WebSocket.OPEN) {
      const error = new Error('WebSocket is not connected');
      console.error('Failed to send message:', error);
      Global.gameEmitter.emit(NetworkEvent.NETWORK_ERROR, { error: 'Connection lost' });
      throw error;
    }
    this.websocket.send(typeof data === 'string' ? data : JSON.stringify(data));
  }*/

  public isConnected() {
    return Boolean(this.websocket && this.websocket.readyState === WebSocket.OPEN);
  }

  public sendPing() {
    this.websocket.sendMessage("0");
  }

  /*public async sendLogin(username: string, password: string) {
    try {
      await this.ready;
      const loginData = { cmd: 'login', username, password };
      this.safeSend(loginData);
      console.log('Login sent:', loginData);
    } catch (error) {
      console.error('Failed to send login:', error);
      Global.gameEmitter.emit(NetworkEvent.SERVER_OFFLINE, { error: 'Failed to send login' });
    }
  }*/

  /*public static sendRegister(username: string, password: string) {
    var m = {
      cmd: "register",
      username: username,
      password: password
    };
    Global.socket.sendMessage(JSON.stringify(m));
  }*/

  public sendSelectedClass(className: string, heroName: string) {
    var m = {
      cmd: "select_class",
      class_name: className,
      hero_name: heroName
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendRecreateHero() {
    var m = {
      cmd: "recreate_hero",
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendImageDef(imageName: string) {
    var m = {
      cmd: 'image_def',
      name: imageName
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendMove(newX: integer, newY: integer) {
    var m = {
      cmd: "move_unit",
      x: newX,
      y: newY
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendItemTransfer(item, sourceId, targetId) {
    var m = {
      cmd: "item_transfer",
      item: item,
      source_id: sourceId,
      target_id: targetId,
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendItemSplit(ownerId, item, quantity) {
    var m = {
      cmd: "item_split",
      owner_id: ownerId,
      item: item,
      quantity: quantity
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoObj(id) {
    var m = {
      cmd: "info_obj",
      id: id
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoItem(obj_id, item_id, action) {
    var m = {
      cmd: "info_item",
      obj_id: obj_id,
      item_id: item_id,
      action: action
    };

    this.sendMessage(JSON.stringify(m));
  }
  public sendInfoItemByName(name) {
    var m = {
      cmd: "info_item_by_name",
      name: name,
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoTile(x, y) {
    var m = {
      cmd: 'info_tile',
      x: x,
      y: y
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoTileResources(x, y) {
    var m = {
      cmd: 'info_tile_resources',
      x: x,
      y: y
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoInventory(id) {
    var m = {
      cmd: "info_inventory",
      id: id
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoEquip(id) {
    var m = {
      cmd: "info_equip",
      id: id
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoItemTransfer(sourceId, targetId) {
    var m = {
      cmd: 'info_item_transfer',
      source_id: sourceId,
      target_id: targetId
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoExperiment(structureId) {
    var m = {
      cmd: 'info_experiment',
      structure_id: structureId
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoAttrs(id) {
    var m = {
      cmd: "info_attrs",
      id: id
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoSkills(id) {
    var m = {
      cmd: "info_skills",
      id: id
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendSurvey(sourceId) {
    var m = {
      cmd: "survey",
      source_id: sourceId
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendProspect() {
    var m = {
      cmd: "prospect"
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendExplore() {
    var m = {
      cmd: "explore"
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendInvestigate(targetId) {
    var m = {
      cmd: "investigate",
      target_id: targetId
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderExplore(sourceId) {
    var m = {
      cmd: "order_explore",
      source_id: sourceId
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderProspect(sourceId) {
    var m = {
      cmd: "order_prospect",
      source_id: sourceId
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendNearbyResources() {
    var m = {
      cmd: "nearby_resources"
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendFollow(id) {
    var m = {
      cmd: "order_follow",
      source_id: id
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderRepair(id) {
    var m = {
      cmd: "order_repair",
      source_id: id
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendCreateFoundation(id, structureName) {
    var m = {
      cmd: "create_foundation",
      source_id: id,
      structure: structureName
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendBuild(id, structureid) {
    var m = {
      cmd: "build",
      source_id: id,
      structure_id: structureid
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendGetStructureList() {
    var m = {
      cmd: "structure_list"
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderGather(id, resourceType) {
    var m = {
      cmd: "order_gather",
      source_id: id,
      res_type: resourceType
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderOperate(sourceId, structureId) {
    var m = {
      cmd: "order_operate",
      source_id: sourceId,
      structure_id: structureId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderCraft(sourceId, structureId) {
    var m = {
      cmd: "order_craft",
      source_id: sourceId,
      structure_id: structureId,
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderRefine(sourceId, structureId) {
    var m = {
      cmd: "order_refine",
      source_id: sourceId,
      structure_id: structureId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderPlant(sourceId, structureId) {
    var m = {
      cmd: "order_plant",
      source_id: sourceId,
      structure_id: structureId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderTend(sourceId, structureId) {
    var m = {
      cmd: "order_tend",
      source_id: sourceId,
      structure_id: structureId,
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderHarvest(sourceId, structureId) {
    var m = {
      cmd: "order_harvest",
      source_id: sourceId,
      structure_id: structureId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendOrderExperiment(sourceId, structureId) {
    var m = {
      cmd: "order_experiment",
      source_id: sourceId,
      structure_id: structureId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendGetStats(id) {
    var m = {
      cmd: "get_stats",
      id: id
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendCombo(sourceId, targetId, comboType) {
    var m = {
      cmd: "combo",
      source_id: sourceId,
      target_id: targetId,
      combo_type: comboType
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendAttack(attackType, sourceId, targetId) {
    var m = {
      cmd: "attack",
      attack_type: attackType,
      source_id: sourceId,
      target_id: targetId
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendAbility(abilityId, sourceId, targetId?) {
    var m = {
      cmd: "ability",
      ability_id: abilityId,
      source_id: sourceId,
      target_id: targetId
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendBlock(sourceId, defense = "brace") {
    var m = {
      cmd: "block",
      source_id: sourceId,
      defense: defense
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendGather() {
    var m = {
      cmd: "gather",
    };

    this.sendMessage(JSON.stringify(m));
  }

  public sendOperate(structureid) {
    var m = {
      cmd: "operate",
      structure_id: structureid
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendRefine(itemid) {
    var m = {
      cmd: "refine",
      item_id: itemid
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendStructureRefine(structureid, itemid) {
    var m = {
      cmd: "structure_refine",
      structure_id: structureid,
      item_id: itemid
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendCraft(recipe) {
    var m = {
      cmd: "craft",
      recipe: recipe
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendStructureCraft(structureid, recipe) {
    var m = {
      cmd: "structure_craft",
      structure_id: structureid,
      recipe: recipe
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendTick() {
    var m = {
      cmd: "tick"
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoAssign(structureId) {
    var m = {
      cmd: "info_assign",
      structure_id: structureId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendAssign(workerId, structureId) {
    var m = {
      cmd: "assign",
      worker_id: workerId,
      structure_id: structureId,
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendRemoveAssign(workerId, structureId) {
    var m = {
      cmd: "remove_assign",
      worker_id: workerId,
      structure_id: structureId,
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoCraft(crafterId) {
    var m = {
      cmd: "info_craft",
      crafter_id: crafterId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoStructureCraft(structureId) {
    var m = {
      cmd: "info_structure_craft",
      structure_id: structureId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoStructureQueue(structureId) {
    var m = {
      cmd: "info_structure_queue",
      structure_id: structureId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoWorkQueueEntry(structureId, index) {
    var m = {
      cmd: "info_work_queue_entry",
      structure_id: structureId,
      index: index
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendAddCraftingEntry(structureId, recipeName) {
    var m = {
      cmd: "add_crafting_entry",
      structure_id: structureId,
      recipe_name: recipeName
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendAddRefineEntry(structureId, refineItemId) {
    var m = {
      cmd: "add_refine_entry",
      structure_id: structureId,
      refine_item_id: refineItemId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendRemoveWorkEntry(structureId, index) {
    var m = {
      cmd: "remove_work_entry",
      structure_id: structureId,
      index: index
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoRefine(refinerId) {
    var m = {
      cmd: "info_refine",
      refiner_id: refinerId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoStructureRefine(structureId) {
    var m = {
      cmd: "info_structure_refine",
      structure_id: structureId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoStructureRefineItem(structureId, itemId) {
    var m = {
      cmd: "info_structure_refine_item",
      structure_id: structureId,
      item_id: itemId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendBuyItem(sellerId, itemId, quantity) {
    var m = {
      cmd: "buy_item",
      seller_id: sellerId,
      item_id: itemId,
      quantity: quantity
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendSellItem(itemId, targetId, quantity) {
    var m = {
      cmd: "sell_item",
      item_id: itemId,
      target_id: targetId,
      quantity: quantity
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoMerchant(sourceId, merchantId) {
    var m = {
      cmd: "info_merchant",
      source_id: sourceId,
      merchant_id: merchantId,
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoHire(sourceId) {
    var m = {
      cmd: "info_hire",
      source_id: sourceId
    }

    this.sendMessage(JSON.stringify(m));
  }

  public sendHire(sourceId, targetId) {
    var m = {
      cmd: "hire",
      source_id: sourceId,
      target_id: targetId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendSetExpItem(structureId, itemId) {
    var m = {
      cmd: "set_exp_item",
      structure_id: structureId,
      item_id: itemId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendSetExpResource(structureId, itemId) {
    var m = {
      cmd: "set_exp_resource",
      structure_id: structureId,
      item_id: itemId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendResetExperiment(structureId) {
    var m = {
      cmd: "reset_experiment",
      structure_id: structureId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoExit(id, paneltype) {
    var m = {
      cmd: "info_exit",
      id: id,
      panel_type: paneltype
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendEquip(objId, itemId, status) {
    var m = {
      cmd: "equip",
      obj_id: objId,
      item: itemId,
      status: status
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendUse(objId, itemId) {
    var m = {
      cmd: "use",
      obj_id: objId,
      item_id: itemId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendDeleteItem(objId, itemId) {
    var m = {
      cmd: "delete_item",
      obj_id: objId,
      item_id: itemId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendSleep(structureId) {
    var m = {
      cmd: "sleep",
      structure_id: structureId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoAdvance(sourceId) {
    var m = {
      cmd: "info_advance",
      source_id: sourceId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendAdvance(sourceId) {
    var m = {
      cmd: "advance",
      source_id: sourceId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendInfoUpgrade(structureId) {
    var m = {
      cmd: "info_upgrade",
      structure_id: structureId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendStartUpgrade(structureId, selectedUpgrade) {
    var m = {
      cmd: "start_upgrade",
      structure_id: structureId,
      selected_upgrade: selectedUpgrade
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendUpgrade(sourceId, structureId) {
    var m = {
      cmd: "upgrade",
      source_id: sourceId,
      structure_id: structureId,
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendExperiment(structureId) {
    var m = {
      cmd: "experiment",
      structure_id: structureId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendActivate(structureId) {
    var m = {
      cmd: "activate",
      structure_id: structureId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendDelete(sourceId) {
    var m = {
      cmd: "delete",
      source_id: sourceId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendPlant(structureId) {
    var m = {
      cmd: "plant",
      structure_id: structureId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendTend(structureId) {
    var m = {
      cmd: "tend",
      structure_id: structureId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendHarvest(structureId) {
    var m = {
      cmd: "harvest",
      structure_id: structureId
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendCancelAction() {

    var m = {
      cmd: "cancel_action"
    }
    this.sendMessage(JSON.stringify(m));
  }

  public sendRequestSafeLogout(): boolean {
    if (!this.isConnected()) {
      return false;
    }

    const packet: NetworkPacket = requestSafeLogoutPacket();
    try {
      this.sendMessage(JSON.stringify(packet));
      return true;
    } catch (error) {
      console.warn('Unable to send Safe Logout request', error);
      return false;
    }
  }

  public sendCancelSafeLogout(): boolean {
    if (!this.isConnected()) {
      return false;
    }

    const packet: NetworkPacket = cancelSafeLogoutPacket();
    try {
      this.sendMessage(JSON.stringify(packet));
      return true;
    } catch (error) {
      console.warn('Unable to send Safe Logout cancellation', error);
      return false;
    }
  }

  /**
   * Send DebugObj command (admin only)
   * Toggles debug logging for a specific object ID
   * @param objId - The object ID to toggle debug logging for
   */
  public sendDebugObj(objId: number) {
    var m = {
      cmd: "debug_obj",
      obj_id: objId
    };
    this.sendMessage(JSON.stringify(m));
    console.log(`[Admin] Sent DebugObj for obj_id: ${objId}`);
  }

  /**
   * Send SetLogLevel command (admin only)
   * Dynamically change log level for a module
   * @param target - Module path (e.g., "siege_perilous::combat")
   * @param level - Log level: "ERROR", "WARN", "INFO", "DEBUG", "TRACE", or "OFF"
   */
  public sendSetLogLevel(target: string, level: string) {
    const validLevels = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE", "OFF"];
    if (!validLevels.includes(level)) {
      console.error(`[Admin] Invalid log level: ${level}. Must be one of: ${validLevels.join(', ')}`);
      return;
    }

    var m = {
      cmd: "set_log_level",
      target: target,
      level: level
    };
    this.sendMessage(JSON.stringify(m));
    console.log(`[Admin] Set log level for '${target}' to ${level}`);
  }

  /**
   * Send GetLogLevels command (admin only)
   * Query current log level overrides from server
   */
  public sendGetLogLevels() {
    var m = {
      cmd: "get_log_levels"
    };
    this.sendMessage(JSON.stringify(m));
    console.log(`[Admin] Requesting current log level overrides`);
  }

  /**
   * Enable DEBUG logging for NPC AI module
   */
  public debugNpcAI() {
    this.sendSetLogLevel("siege_perilous::npc", "DEBUG");
  }

  /**
   * Enable DEBUG logging for combat module
   */
  public debugCombat() {
    this.sendSetLogLevel("siege_perilous::combat", "DEBUG");
  }

  /**
   * Enable DEBUG logging for villager AI module
   */
  public debugVillagerAI() {
    this.sendSetLogLevel("siege_perilous::villager", "DEBUG");
  }

  /**
   * Reset all log overrides (informational - server doesn't support bulk reset)
   */
  public resetAllLogLevels() {
    console.log('[Admin] To reset log levels, restart the server or set each module to "OFF"');
  }

  private completeSafeLogout(packet: SafeLogoutStatusPacket) {
    if (!this.safeLogoutCloseGuard.acceptProtectedStatus(packet)) {
      return;
    }

    this.clearNetworkErrorTimeout();
    Global.connected = false;
    this.reloadAfterSafeLogoutClose = false;

    try {
      rememberSafeLogoutCompletion(window.sessionStorage);
      this.reloadAfterSafeLogoutClose = true;
    } catch (error) {
      console.warn('Unable to persist Safe Logout completion for reload', error);
    }

    Global.gameEmitter.emit(NetworkEvent.SAFE_LOGOUT_COMPLETE, {
      message: SAFE_LOGOUT_COMPLETION_MESSAGE,
    });

    if (this.websocket && this.websocket.readyState === WebSocket.OPEN) {
      this.websocket.close(1000, 'Safe Logout complete');
    }
  }

  constructor() { }

  public connect() {
    const url: string = "wss://" + window.location.hostname + ":8443";

    this.clearNetworkErrorTimeout();
    this.safeLogoutCloseGuard.resetForLogin();
    this.reloadAfterSafeLogoutClose = false;
    try {
      clearSafeLogoutReconnectSuppression(window.sessionStorage);
    } catch (error) {
      console.warn('Unable to clear Safe Logout reconnect suppression', error);
    }
    Global.gameEmitter.emit(NetworkEvent.SAFE_LOGOUT_RESET);
    this.websocket = new WebSocket(url);
    const websocket = this.websocket;

    this.websocket.onopen = (evt) => {
      if (websocket !== this.websocket) {
        return;
      }

      this.clearNetworkErrorTimeout();
      console.log('Opened websocket');
      setInterval(function () {
        console.log('Sending Ping');
        //Network.sendPing()
      }, 50000);
    };

    this.websocket.onclose = (evt) => {
      if (websocket !== this.websocket) {
        return;
      }

      console.log('Websocket Closing...');
      if (this.safeLogoutCloseGuard.suppressConnectionFailure()) {
        this.clearNetworkErrorTimeout();
        Global.connected = false;
        if (this.reloadAfterSafeLogoutClose) {
          window.location.reload();
        }
        return;
      }
      this.scheduleServerOffline(websocket);
    }

    this.websocket.onerror = (evt) => {
      if (websocket !== this.websocket) {
        return;
      }

      console.log('Websocket Error...');
      if (this.safeLogoutCloseGuard.suppressConnectionFailure()) {
        return;
      }
      this.scheduleNetworkError(websocket);
    }

    this.setupMessageHandler();
  }

  private setupMessageHandler() {
    this.websocket.onmessage = (evt) => {
      var jsonData = JSON.parse(evt.data);

      //Check if error message is in the packet
      if (jsonData.hasOwnProperty('errmsg')) {
        console.log('Error received: ' + jsonData.errmsg);
        Global.gameEmitter.emit(NetworkEvent.ERROR, jsonData);
      } else if (jsonData.hasOwnProperty('noticemsg')) {
        Global.gameEmitter.emit(NetworkEvent.NOTICE, jsonData);
      } else if (jsonData.packet == "combat_telegraph") {
        Global.gameEmitter.emit(NetworkEvent.COMBAT_TELEGRAPH, jsonData);
      } else if (jsonData.packet == "select_class") {
        Global.playerId = jsonData.player;
        if (Global.pendingClassSelection) {
          const { className, heroName } = Global.pendingClassSelection;
          Global.pendingClassSelection = null;
          this.sendSelectedClass(className, heroName);
        } else {
          Global.gameEmitter.emit(NetworkEvent.SELECT_CLASS, {});
        }
      } else if (jsonData.packet == "info_select_class") {
        if (jsonData.result == "success") {
          console.log("Class selected, logging in")
          Global.gameEmitter.emit(NetworkEvent.FIRST_LOGIN, {});
        }
      } else if (jsonData.packet == "login") {
        console.log("Login successful")
        Global.playerId = jsonData.player;
        Global.gameEmitter.emit(NetworkEvent.LOGGED_IN, { has_account: jsonData.has_account || false });
      } else if (jsonData.packet == "explored_map") {
        this.processTileStates(jsonData.tiles);
      } else if (jsonData.packet == 'init_perception') {
        console.log('Received Perception');

        this.processTileStates(jsonData.data.map);
        this.processInitObjStates(jsonData.data.visible_objs);
        this.processInitObjStates(jsonData.data.observers); // Add observers after to overwrite if observers end up being visible objects
        this.processInitWeather(jsonData.data.weather);

        //Add small delay to prevent perception event before Scenes are created.
        setTimeout(function () { console.log('Emitting perception event'); Global.gameEmitter.emit(NetworkEvent.PERCEPTION, jsonData); }, 3000);
      } else if (jsonData.packet == 'new_perception') {
        console.log('$$$$$$$$$$ Received New Perception $$$$$$$$$$$$$$$');
        this.processNewPerceptionVisibleObjStates(jsonData.data.visible_objs);
        this.processNewPerceptionObserverStates(jsonData.data.observers);
        this.processTileStates(jsonData.data.map);
        console.log(JSON.stringify(Global.objectStates, null, 2));
        Global.gameEmitter.emit(NetworkEvent.NEW_PERCEPTION, jsonData);
      } else if (jsonData.packet == 'perception_changes') {
        console.log("--- Changes Packet Received ---");
        console.log(jsonData);
        this.processUpdateObjStates(jsonData.events);
        Global.gameEmitter.emit(NetworkEvent.CHANGES, jsonData);
      } else if (jsonData.packet == 'new_obj_perception') {
        console.log("----- OBJ PERCEPTION -----");
        console.log(jsonData);

        this.processObjPerception(jsonData.new_objs);
        this.processTileStates(jsonData.new_tiles);

        Global.gameEmitter.emit(NetworkEvent.OBJ_PERCEPTION, jsonData);
      } else if (jsonData.packet == 'image_def') {
        Global.gameEmitter.emit(NetworkEvent.IMAGE_DEF, jsonData);
      } else if (jsonData.packet == 'stats') {
        this.processGetStats(jsonData.data);
        Global.gameEmitter.emit(NetworkEvent.STATS, jsonData.data);
      } else if (jsonData.packet == 'hero_death_state') {
        Global.gameEmitter.emit(NetworkEvent.HERO_DEATH_STATE, jsonData);
      } else if (jsonData.packet == "info_hero") {
        Global.gameEmitter.emit(NetworkEvent.INFO_HERO, jsonData);
      } else if (jsonData.packet == "info_villager") {
        Global.gameEmitter.emit(NetworkEvent.INFO_VILLAGER, jsonData);
      } else if (jsonData.packet == "info_structure") {
        console.log('info_structure: ' + JSON.stringify(jsonData));
        Global.gameEmitter.emit(NetworkEvent.INFO_STRUCTURE, jsonData);
      } else if (jsonData.packet == "info_npc") {
        Global.gameEmitter.emit(NetworkEvent.INFO_NPC, jsonData);
      } else if (jsonData.packet == "info_monolith") {
        Global.gameEmitter.emit(NetworkEvent.INFO_MONOLITH, jsonData);
      } else if (jsonData.packet == "info_poi") {
        Global.gameEmitter.emit(NetworkEvent.INFO_POI, jsonData);
      } else if (jsonData.packet == "info_obj") {
        Global.gameEmitter.emit(NetworkEvent.INFO_OBJ, jsonData);
      } else if (jsonData.packet == "info_tile") {
        Global.gameEmitter.emit(NetworkEvent.INFO_TILE, jsonData);
      } else if (jsonData.packet == "info_tile_resources") {
        Global.gameEmitter.emit(NetworkEvent.INFO_TILE_RESOURCES, jsonData);
      } else if (jsonData.packet == "info_item") {
        Global.gameEmitter.emit(NetworkEvent.INFO_ITEM, jsonData)
      } else if (jsonData.packet == "info_inventory") {
        Global.gameEmitter.emit(NetworkEvent.INFO_INVENTORY, jsonData);
      } else if (jsonData.packet == "info_inventory_snapshot") {
        Global.gameEmitter.emit(NetworkEvent.INFO_INVENTORY_SNAPSHOT, jsonData);
      } else if (jsonData.packet == "info_equip") {
        Global.gameEmitter.emit(NetworkEvent.INFO_EQUIP, jsonData);
      } else if (jsonData.packet == "info_item_transfer") {
        Global.gameEmitter.emit(NetworkEvent.INFO_ITEM_TRANSFER, jsonData);
      } else if (jsonData.packet == "info_items_update") {
        Global.gameEmitter.emit(NetworkEvent.INFO_ITEMS_UPDATE, jsonData);
      } else if (jsonData.packet == "info_activity_update") {
        Global.gameEmitter.emit(NetworkEvent.INFO_ACTIVITY_UPDATE, jsonData);
      } else if (jsonData.packet == "info_needs_update") {
        Global.gameEmitter.emit(NetworkEvent.INFO_NEEDS_UPDATE, jsonData);
      } else if (jsonData.packet == "info_stamina_update") {
        Global.heroStamina = jsonData.stamina;
        Global.gameEmitter.emit(GameEvent.HERO_STATS_UPDATE, { hp: Global.heroHp, stamina: Global.heroStamina, mana: Global.heroMana });
      } else if (jsonData.packet == "info_mana_update") {
        Global.heroMana = jsonData.mana;
        Global.gameEmitter.emit(GameEvent.HERO_STATS_UPDATE, { hp: Global.heroHp, stamina: Global.heroStamina, mana: Global.heroMana });
      } else if (jsonData.packet == "info_hunger_update") {
        Global.gameEmitter.emit(NetworkEvent.INFO_HUNGER_UPDATE, jsonData);
      } else if (jsonData.packet == "info_thirst_update") {
        Global.gameEmitter.emit(NetworkEvent.INFO_THIRST_UPDATE, jsonData);
      } else if (jsonData.packet == "info_tiredness_update") {
        Global.gameEmitter.emit(NetworkEvent.INFO_TIREDNESS_UPDATE, jsonData);
      } else if (jsonData.packet == "item_transfer") {
        Global.gameEmitter.emit(NetworkEvent.ITEM_TRANSFER, jsonData);
      } else if (jsonData.packet == "item_split") {
        if (jsonData.result == 'success') {
          this.sendInfoInventory(jsonData.owner);
        }
      } else if (jsonData.packet == "info_attrs") {
        Global.gameEmitter.emit(NetworkEvent.INFO_ATTRS, jsonData);
      } else if (jsonData.packet == "info_skills") {
        Global.gameEmitter.emit(NetworkEvent.INFO_SKILLS, jsonData);
      } else if (jsonData.packet == "info_advance") {
        Global.gameEmitter.emit(NetworkEvent.INFO_ADVANCE, jsonData);
      } else if (jsonData.packet == "info_upgrade") {
        Global.gameEmitter.emit(NetworkEvent.INFO_STRUCTURE_UPGRADE, jsonData);
      } else if (jsonData.packet == "info_experiment") {
        Global.gameEmitter.emit(NetworkEvent.INFO_EXPERIMENT, jsonData);
      } else if (jsonData.packet == "info_experiment_state") {
        Global.gameEmitter.emit(NetworkEvent.INFO_EXPERIMENT_STATE, jsonData);
      } else if (jsonData.packet == "info_refine") {
        Global.gameEmitter.emit(NetworkEvent.INFO_REFINE, jsonData);
      } else if (jsonData.packet == "info_structure_refine") {
        Global.gameEmitter.emit(NetworkEvent.INFO_STRUCTURE_REFINE, jsonData);
      } else if (jsonData.packet == "info_refine_item") {
        Global.gameEmitter.emit(NetworkEvent.INFO_REFINE_ITEM, jsonData);
      } else if (jsonData.packet == "info_crop") {
        Global.gameEmitter.emit(NetworkEvent.INFO_CROP, jsonData);
      } else if (jsonData.packet == "info_true_death") {
        Global.gameEmitter.emit(NetworkEvent.INFO_TRUE_DEATH, jsonData);
      } else if (jsonData.packet == "nearby_resources") {
        Global.gameEmitter.emit(NetworkEvent.NEARBY_RESOURCES, jsonData);
      } else if (jsonData.packet == "structure_list") {
        Global.gameEmitter.emit(NetworkEvent.STRUCTURE_LIST, jsonData);
      } else if (jsonData.packet == 'work_update') {
        Global.gameEmitter.emit(NetworkEvent.WORK_UPDATE, jsonData);
      } else if (jsonData.packet == 'start_upgrade') {
        Global.gameEmitter.emit(NetworkEvent.START_UPGRADE, jsonData);
      } else if (jsonData.packet == 'upgrade') {
        Global.gameEmitter.emit(NetworkEvent.UPGRADE, jsonData);
      } else if (jsonData.packet == 'craft') {
        Global.gameEmitter.emit(NetworkEvent.CRAFT, jsonData);
      } else if (jsonData.packet == 'refine') {
        Global.gameEmitter.emit(NetworkEvent.REFINE, jsonData);
      } else if (jsonData.packet == 'explore') {
        Global.gameEmitter.emit(NetworkEvent.EXPLORE, jsonData);
        Global.gameEmitter.emit(NetworkEvent.PROSPECT, jsonData);
      } else if (jsonData.packet == 'survey') {
        Global.gameEmitter.emit(NetworkEvent.SURVEY, jsonData);
      } else if (jsonData.packet == 'prospect') {
        Global.gameEmitter.emit(NetworkEvent.PROSPECT, jsonData);
      } else if (jsonData.packet == 'investigate') {
        Global.gameEmitter.emit(NetworkEvent.INVESTIGATE, jsonData);
      } else if (jsonData.packet == 'gather') {
        Global.gameEmitter.emit(NetworkEvent.GATHER, jsonData);
      } else if (jsonData.packet == 'attack') {
        Global.gameEmitter.emit(NetworkEvent.ATTACK, jsonData);
      } else if (jsonData.packet == 'ability') {
        Global.gameEmitter.emit(NetworkEvent.ABILITY, jsonData);
      } else if (jsonData.packet == 'dmg') {
        this.processDmg(jsonData);
        Global.gameEmitter.emit(NetworkEvent.DMG, jsonData);
      } else if (jsonData.packet == 'spoil') {
        Global.gameEmitter.emit(NetworkEvent.SPOIL, jsonData);
      } else if (jsonData.packet == 'steal') {
        Global.gameEmitter.emit(NetworkEvent.STEAL, jsonData);
      } else if (jsonData.packet == 'torch') {
        Global.gameEmitter.emit(NetworkEvent.TORCH, jsonData);
      } else if (jsonData.packet == "xp") {
        Global.gameEmitter.emit(NetworkEvent.XP, jsonData);
      } else if (jsonData.packet == "gained_effect") {
        Global.gameEmitter.emit(NetworkEvent.GAINED_EFFECT, jsonData);
      } else if (jsonData.packet == "lost_effect") {
        Global.gameEmitter.emit(NetworkEvent.LOST_EFFECT, jsonData);
      } else if (jsonData.packet == "reduced_effect") {
        Global.gameEmitter.emit(NetworkEvent.REDUCED_EFFECT, jsonData);
      } else if (jsonData.packet == "increased_effect") {
        Global.gameEmitter.emit(NetworkEvent.INCREASED_EFFECT, jsonData);
      } else if (jsonData.packet == 'speech') {
        Global.gameEmitter.emit(NetworkEvent.SPEECH, jsonData);
      } else if (jsonData.packet == 'sound') {
        Global.gameEmitter.emit(NetworkEvent.SOUND, jsonData);
      } else if (jsonData.packet == 'info_assign') {
        Global.gameEmitter.emit(NetworkEvent.INFO_ASSIGN, jsonData);
      } else if (jsonData.packet == 'info_craft') {
        Global.gameEmitter.emit(NetworkEvent.INFO_CRAFT, jsonData);
      } else if (jsonData.packet == 'info_structure_craft') {
        Global.gameEmitter.emit(NetworkEvent.INFO_STRUCTURE_CRAFT, jsonData);
      } else if (jsonData.packet == 'info_structure_queue') {
        Global.gameEmitter.emit(NetworkEvent.INFO_STRUCTURE_QUEUE, jsonData);
      } else if (jsonData.packet == 'info_work_queue_entry') {
        Global.gameEmitter.emit(NetworkEvent.INFO_WORK_QUEUE_ENTRY, jsonData);
      } else if (jsonData.packet == 'buy_item') {
        Global.gameEmitter.emit(NetworkEvent.BUY_ITEM, jsonData);
      } else if (jsonData.packet == 'sell_item') {
        Global.gameEmitter.emit(NetworkEvent.SELL_ITEM, jsonData);
      } else if (jsonData.packet == 'info_merchant') {
        Global.gameEmitter.emit(NetworkEvent.INFO_MERCHANT, jsonData);
      } else if (jsonData.packet == 'info_hire') {
        Global.gameEmitter.emit(NetworkEvent.INFO_HIRE, jsonData);
      } else if (jsonData.packet == 'set_exp_item') {
        Global.gameEmitter.emit(NetworkEvent.INFO_EXPERIMENT, jsonData);
      } else if (jsonData.packet == 'set_exp_resource') {
        Global.gameEmitter.emit(NetworkEvent.INFO_EXPERIMENT, jsonData);
      } else if (jsonData.packet == 'advance') {
        Global.gameEmitter.emit(NetworkEvent.ADVANCE, jsonData);
      } else if (jsonData.packet == 'new_items') {
        Global.gameEmitter.emit(NetworkEvent.NEW_ITEMS, jsonData);
      } else if (jsonData.packet == 'world') {
        Global.gameEmitter.emit(NetworkEvent.WORLD, jsonData);
      } else if (jsonData.packet === 'debug_obj') {
        console.log(`[Admin Response] DebugObj for obj ${jsonData.obj_id}: ${jsonData.enabled ? 'ENABLED' : 'DISABLED'}`);
      } else if (jsonData.packet === 'log_level_set') {
        const status = jsonData.success ? '✓ SUCCESS' : '✗ FAILED';
        console.log(`[Admin Response] ${status} - Log level for '${jsonData.target}' set to ${jsonData.level}`);
      } else if (jsonData.packet === 'log_levels') {
        console.log('[Admin Response] Current Log Level Overrides:');
        if (jsonData.overrides.length === 0) {
          console.log('  (none - all modules using default levels)');
        } else {
          jsonData.overrides.forEach(([target, level]) => {
            console.log(`  ${target} = ${level}`);
          });
        }
      } else if (jsonData.packet == 'objectives') {
        Global.gameEmitter.emit(NetworkEvent.OBJECTIVES, jsonData);
      } else if (jsonData.packet == 'objective_state') {
        Global.gameEmitter.emit(NetworkEvent.OBJECTIVE_STATE, jsonData);
      } else if (jsonData.packet == 'threat_state') {
        Global.gameEmitter.emit(NetworkEvent.THREAT_STATE, jsonData);
      } else if (jsonData.packet == 'crisis_status') {
        Global.gameEmitter.emit(NetworkEvent.CRISIS_STATUS, jsonData);
      } else if (jsonData.packet == 'safe_logout_status') {
        const safeLogoutStatus = jsonData as SafeLogoutStatusPacket;
        // UI observers receive the authoritative protected snapshot before the
        // one-shot transport reaction closes the gameplay socket.
        dispatchSafeLogoutStatus(
          safeLogoutStatus,
          (status) => Global.gameEmitter.emit(NetworkEvent.SAFE_LOGOUT_STATUS, status),
          (status) => this.completeSafeLogout(status),
        );
      } else if (jsonData.packet == 'combat_state') {
        Global.combatState = jsonData;
        Global.gameEmitter.emit(NetworkEvent.COMBAT_STATE, jsonData);
      } else if (jsonData.packet == 'discovery_event') {
        Global.gameEmitter.emit(NetworkEvent.DISCOVERY_EVENT, jsonData);
      }
    }
  }

  processInitObjStates(objs) {
    for (var index in objs) {
      var obj = objs[index];
      var objectState: ObjectState = {
        id: obj.id,
        player: obj.player,
        name: obj.name,
        class: obj.class,
        subclass: obj.subclass,
        template: obj.template,
        groups: obj.groups,
        state: obj.state,
        prevstate: obj.state,
        x: obj.x,
        y: obj.y,
        vision: obj.vision,
        image: obj.image,
        hsl: obj.hsl,
        work_done: obj.work_done,
        total_work: obj.total_work,
        work_per_sec: obj.work_per_sec,
        op: 'added'
      };

      console.log(objectState);
      console.log('Global.playerId: ' + Global.playerId);
      if (objectState.player == Global.playerId && objectState.subclass == 'hero') {
        console.log('Setting Hero Id');
        Global.heroId = objectState.id;

        Global.gameEmitter.emit(NetworkEvent.HERO_INIT, Global.heroId);

        this.sendGetStats(Global.heroId);
      }

      Global.objectStates[objectState.id] = objectState;
    }
  }

  processInitWeather(weatherTiles) {
    for (var index in weatherTiles) {
      var weather = weatherTiles[index];

      var weatherState: WeatherState = {
        index: weather.x + '_' + weather.y,
        hexX: weather.x,
        hexY: weather.y,
        weather: weather.weather
      };

      Global.weatherStates[weatherState.index] = weatherState;
    }

    Global.gameEmitter.emit(NetworkEvent.WEATHER_INIT, {});
  }

  processNewPerceptionObserverStates(observers) {
    for (var index in observers) {
      var observer = observers[index];

      if (observer.id in Global.objectStates) {
        // Update all the object state attributes
        Global.objectStates[observer.id].vision = observer.vision;
        Global.objectStates[observer.id].player = observer.player;
        Global.objectStates[observer.id].name = observer.name;
        Global.objectStates[observer.id].class = observer.class;
        Global.objectStates[observer.id].subclass = observer.subclass;
        Global.objectStates[observer.id].template = observer.template;
        Global.objectStates[observer.id].groups = observer.groups;
        Global.objectStates[observer.id].state = observer.state;
        Global.objectStates[observer.id].prevstate = observer.state;
        Global.objectStates[observer.id].x = observer.x;
        Global.objectStates[observer.id].y = observer.y;
        Global.objectStates[observer.id].image = observer.image;
        Global.objectStates[observer.id].hsl = observer.hsl;
        Global.objectStates[observer.id].work_done = observer.work_done;
        Global.objectStates[observer.id].total_work = observer.total_work;
        Global.objectStates[observer.id].work_per_sec = observer.work_per_sec;
        Global.objectStates[observer.id].op = 'updated';
        Global.objectStates[observer.id].updateAttr = undefined;
        Global.objectStates[observer.id].eventType = undefined;

      } else {
        var objectState: ObjectState = {
          id: observer.id,
          player: observer.player,
          name: observer.name,
          class: observer.class,
          subclass: observer.subclass,
          template: observer.template,
          groups: observer.groups,
          state: observer.state,
          prevstate: observer.state,
          x: observer.x,
          y: observer.y,
          vision: observer.vision,
          image: observer.image,
          hsl: observer.hsl,
          work_done: observer.work_done,
          total_work: observer.total_work,
          work_per_sec: observer.work_per_sec,
          op: 'added',
          eventType: undefined
        };
        Global.objectStates[objectState.id] = objectState;
      }
    }
  }

  processNewPerceptionVisibleObjStates(visibleObjs) {
    // Mark all for deleted first
    for (var objectId in Global.objectStates) {
      var objectState = Global.objectStates[objectId];
      objectState.op = 'deleted';
      objectState.eventType = 'perception';
    }

    for (var index in visibleObjs) {
      var visibleObj = visibleObjs[index];

      if (visibleObj.id in Global.objectStates) {
        // Update all the object state attributes
        Global.objectStates[visibleObj.id].vision = visibleObj.vision;
        Global.objectStates[visibleObj.id].player = visibleObj.player;
        Global.objectStates[visibleObj.id].name = visibleObj.name;
        Global.objectStates[visibleObj.id].class = visibleObj.class;
        Global.objectStates[visibleObj.id].subclass = visibleObj.subclass;
        Global.objectStates[visibleObj.id].template = visibleObj.template;
        Global.objectStates[visibleObj.id].groups = visibleObj.groups;
        Global.objectStates[visibleObj.id].state = visibleObj.state;
        Global.objectStates[visibleObj.id].prevstate = visibleObj.state;
        Global.objectStates[visibleObj.id].x = visibleObj.x;
        Global.objectStates[visibleObj.id].y = visibleObj.y;
        Global.objectStates[visibleObj.id].image = visibleObj.image;
        Global.objectStates[visibleObj.id].hsl = visibleObj.hsl;
        Global.objectStates[visibleObj.id].work_done = visibleObj.work_done;
        Global.objectStates[visibleObj.id].total_work = visibleObj.total_work;
        Global.objectStates[visibleObj.id].work_per_sec = visibleObj.work_per_sec;
        Global.objectStates[visibleObj.id].op = 'updated';
        Global.objectStates[visibleObj.id].updateAttr = undefined;
        Global.objectStates[visibleObj.id].eventType = undefined;

      } else {
        var objectState: ObjectState = {
          id: visibleObj.id,
          player: visibleObj.player,
          name: visibleObj.name,
          class: visibleObj.class,
          subclass: visibleObj.subclass,
          template: visibleObj.template,
          groups: visibleObj.groups,
          state: visibleObj.state,
          prevstate: visibleObj.state,
          x: visibleObj.x,
          y: visibleObj.y,
          vision: visibleObj.vision,
          image: visibleObj.image,
          hsl: visibleObj.hsl,
          work_done: visibleObj.work_done,
          total_work: visibleObj.total_work,
          work_per_sec: visibleObj.work_per_sec,
          op: 'added',
          eventType: undefined
        };
        Global.objectStates[objectState.id] = objectState;
      }
    }
  }
  processObjPerception(objs) {
    for (var i = 0; i < objs.length; i++) {
      var obj = objs[i];

      var objectExists = obj.id in Global.objectStates;
      console.log("objectExists: " + objectExists);
      if (!objectExists) {
        Global.objectStates[obj.id] = obj;
        Global.objectStates[obj.id].prevstate = obj.state;
        Global.objectStates[obj.id].op = 'added';

        Global.gameEmitter.emit(GameEvent.OBJ_CREATED, obj.id);
      } else {
        Global.objectStates[obj.id].vision = obj.vision;
        Global.objectStates[obj.id].player = obj.player;
        Global.objectStates[obj.id].name = obj.name;
        Global.objectStates[obj.id].class = obj.class;
        Global.objectStates[obj.id].subclass = obj.subclass;
        Global.objectStates[obj.id].template = obj.template;
        Global.objectStates[obj.id].groups = obj.groups;
        Global.objectStates[obj.id].state = obj.state;
        Global.objectStates[obj.id].prevstate = obj.state;
        Global.objectStates[obj.id].x = obj.x;
        Global.objectStates[obj.id].y = obj.y;
        Global.objectStates[obj.id].image = obj.image;
        Global.objectStates[obj.id].work_done = obj.work_done;
        Global.objectStates[obj.id].total_work = obj.total_work;
        Global.objectStates[obj.id].work_per_sec = obj.work_per_sec;
        Global.objectStates[obj.id].op = 'updated';
        Global.objectStates[obj.id].updateAttr = undefined;
        Global.objectStates[obj.id].eventType = undefined;
      }

    }

    console.log(Global.objectStates);
  }

  processTileStates(tiles) {
    for (var index in tiles) {
      var tile = tiles[index];
      var tileState: TileState = {
        index: tile.x + '_' + tile.y,
        hexX: tile.x,
        hexY: tile.y,
        tiles: tile.t
      };

      Global.tileStates[tileState.index] = tileState;
    }
  }

  processUpdateObjStates(events) {
    //Reset the operation
    /*for (var objectId in Global.objectStates) {
      var objectState = Global.objectStates[objectId] as ObjectState;
      objectState.op = 'none';
    }*/

    for (var i = 0; i < events.length; i++) {
      var eventType = events[i].event;

      if (eventType == "obj_create") {
        var obj = events[i].obj;

        Global.objectStates[obj.id] = obj;
        Global.objectStates[obj.id].prevstate = obj.state;
        Global.objectStates[obj.id].op = 'added';
        Global.objectStates[obj.id].updateAttr = undefined;
        Global.objectStates[obj.id].eventType = 'obj_create';

        Global.gameEmitter.emit(GameEvent.OBJ_CREATED, obj.id);
      } else if (eventType == "obj_update") {
        var obj_id = events[i].obj_id;
        var attrs = events[i].attrs;
        console.log("attrs: " + JSON.stringify(attrs));

        if (!(obj_id in Global.objectStates)) {
          console.warn("Ignoring obj_update for unknown object: " + obj_id);
          continue;
        }

        for (const objAttr of attrs) {

          var attr = objAttr.attr;
          var value = objAttr.value;
          console.log("attr: " + attr + " value: " + value);

          if (attr == 'state') {
            Global.objectStates[obj_id].prevstate = Global.objectStates[obj_id].state || NONE;
            Global.objectStates[obj_id].state = value;
            Global.objectStates[obj_id].updateAttr = 'state';

            // Is hero dead?
            if (obj_id == Global.heroId && value == DEAD) {
              Global.heroDead = true;
              Global.gameEmitter.emit(GameEvent.HERO_DEAD, {});
            }
          } else if (attr == 'template') {
            Global.objectStates[obj_id].template = value;
            //Manually set image because obj_update only supports 1 attr
            // TODO fix this
            Global.objectStates[obj_id].image = 'None';
            Global.objectStates[obj_id].updateAttr = 'template';
          } else if (attr == 'image') {
            Global.objectStates[obj_id].image = value;
            Global.objectStates[obj_id].updateAttr = 'image';
          } else if (attr == 'vision') {
            Global.objectStates[obj_id].vision = parseInt(value);
            Global.objectStates[obj_id].updateAttr = 'vision';
          } else if (attr == 'player_id') {
            Global.objectStates[obj_id].player = value;
            Global.objectStates[obj_id].updateAttr = 'player_id';
          }

          Global.objectStates[obj_id].op = 'updated';
          Global.objectStates[obj_id].eventType = 'obj_update';
          console.log("Emitting obj update: " + obj_id);
          Global.gameEmitter.emit(GameEvent.OBJ_UPDATE, obj_id);
        }
      } else if (eventType == "obj_move") {
        console.log(events[i]);
        var obj = events[i].obj;
        var src_x = events[i].src_x;
        var src_y = events[i].src_y;

        if (obj.id in Global.objectStates) {
          Global.objectStates[obj.id].prevstate = Global.objectStates[obj.id].state;
          Global.objectStates[obj.id].state = obj.state;
          Global.objectStates[obj.id].prevX = Global.objectStates[obj.id].x;
          Global.objectStates[obj.id].prevY = Global.objectStates[obj.id].y;
          Global.objectStates[obj.id].x = obj.x;
          Global.objectStates[obj.id].y = obj.y;
          Global.objectStates[obj.id].op = 'updated';
          Global.objectStates[obj.id].updateAttr = undefined;
          Global.objectStates[obj.id].eventType = 'obj_move';
        } else {
          Global.objectStates[obj.id] = obj;
          Global.objectStates[obj.id].prevstate = obj.state;
          Global.objectStates[obj.id].eventType = 'obj_move';
          Global.objectStates[obj.id].prevX = src_x;
          Global.objectStates[obj.id].prevY = src_y;
          Global.objectStates[obj.id].op = 'added';
        }

        Global.gameEmitter.emit(GameEvent.OBJ_MOVED, obj.id);
      } else if (eventType == "obj_delete") {
        var obj_id = events[i].obj_id;

        if (!(obj_id in Global.objectStates)) {
          console.warn("Ignoring obj_delete for unknown object: " + obj_id);
          continue;
        }

        Global.objectStates[obj_id].op = 'deleted';
        Global.objectStates[obj_id].updateAttr = undefined;
        Global.objectStates[obj_id].eventType = 'obj_delete';
        Global.gameEmitter.emit(GameEvent.OBJ_DELETED, obj_id);
      }
    }
  }

  processGetStats(data) {
    console.log(data);

    Global.heroHp = data.hp;
    Global.heroMaxHp = data.base_hp;
    Global.heroStamina = data.stamina;
    Global.heroMaxStamina = data.base_stamina;
    Global.heroMana = data.mana || 0;
    Global.heroMaxMana = data.base_mana || 0;
    if (Global.heroHp > 0) {
      Global.heroDead = false;
    }
  }


  processDmg(data) {
    //Set object state to dead because an update is not sent to save on messages
    if (!data.missed && data.state == DEAD) {
      if (Global.objectStates[data.target_id]) {
        var targetState = Global.objectStates[data.target_id];
        if (targetState.state != DEAD) {
          targetState.prevstate = targetState.state || NONE;
        }
        targetState.state = DEAD;
        targetState.op = 'updated';
        targetState.updateAttr = 'state';
        targetState.eventType = 'dmg';
        Global.gameEmitter.emit(GameEvent.OBJ_UPDATE, data.target_id);
      } else {
        console.error('ObjectState not found for ' + data.target_id);
      }
    }

    if (data.target_id == Global.heroId) {

      if (!data.missed && (data.state == DEAD || data.dmg >= Global.heroHp)) {
        var wasHeroDead = Global.heroDead;
        Global.heroHp = 0;
        Global.heroDead = true;
        if (!wasHeroDead) {
          Global.gameEmitter.emit(GameEvent.HERO_DEAD, {});
        }
      } else if (!data.missed) {
        Global.heroHp = Math.max(0, Global.heroHp - data.dmg);
      }

      Global.gameEmitter.emit(GameEvent.HERO_STATS_UPDATE, { hp: Global.heroHp, stamina: Global.heroStamina, mana: Global.heroMana });
    } else if (data.source_id == Global.heroId) {

      if (data.missed) {
        return;
      }

      if (data.attack_type == 'combo') {
        Global.attacks.length = 0;
      } else {
        Global.attacks.push(data.attack_type);
      }
    }
  }



}
