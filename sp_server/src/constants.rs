// Game Constants
pub const TICKS_PER_SEC: i32 = 10;

pub const NO_TARGET: i32 = -1;
pub const ATTACK_COOLDOWN_TICKS: i32 = 50; // 5 seconds at 10 ticks/sec
pub const BASE_MOVE_TICKS: f32 = 100.0;
pub const BASE_SPEED: f32 = 1.0;

pub const MERCHANT_PLAYER_ID: i32 = 2000;
pub const MONOLITH_PLAYER_ID: i32 = 2000;

pub const EMERGENCY_SCORE: f32 = 99.0;
pub const URGENT_SCORE: f32 = 98.0;
pub const MAX_ROUTINE_SCORE: f32 = 97.0;

pub const PRIORITY2_SCORE: f32 = 92.0;
pub const PRIORITY1_SCORE: f32 = 91.0;
pub const NORMAL_SCORE: f32 = 90.0;

// Vitals scores
pub const HYDRATED_SCORE: f32 = 10.0;
pub const REFRESHED_SCORE: f32 = 25.0;
pub const SLIGHTLY_THIRSTY_SCORE: f32 = 60.0;
pub const THIRSTY_SCORE: f32 = 75.0;
pub const PARCHED_SCORE: f32 = 90.0;

pub const DEHYDRATED_WARNING1_AT: i32 = GAME_HOUR * 6;
pub const DEHYDRATED_WARNING2_AT: i32 = GAME_HOUR * 9;
pub const DEHYDRATED_DEATH_AT: i32 = GAME_HOUR * 12;

pub const STARVING_WARNING1_AT: i32 = GAME_HOUR * 6;
pub const STARVING_WARNING2_AT: i32 = GAME_HOUR * 9;
pub const STARVING_DEATH_AT: i32 = GAME_HOUR * 12;

pub const EXHAUSTED_WARNING1_AT: i32 = GAME_HOUR * 6;
pub const EXHAUSTED_WARNING2_AT: i32 = GAME_HOUR * 9;
pub const EXHAUSTED_DEATH_AT: i32 = GAME_HOUR * 12;

pub const DEHYDRATED_SCORE: f32 = 90.0;
pub const STARVING_SCORE: f32 = 90.0;
pub const EXHAUSTED_SCORE: f32 = 90.0;

pub const OVERHEATED_SCORE: f32 = 90.0;
pub const HYPOTHERMIC_SCORE: f32 = -90.0;
pub const COMFORT_TEMPERATURE: f32 = 20.0;

// Game Time Constants
pub const GAME_HOUR: i32 = 100;
pub const GAME_TICKS_PER_DAY: i32 = 2400;

pub const FIRST_LIGHT: i32 = 400;
pub const DAWN: i32 = 500;
pub const MORNING: i32 = 600;
pub const AFTERNOON: i32 = 1200;
pub const EVENING: i32 = 1800;
pub const DUSK: i32 = 2000;
pub const NIGHT: i32 = 2200;

pub const MAX_PRICE: i32 = 1000000;
pub const MAX_BUILD_UPGRADE_COST: i32 = 10000000;

pub const HIGH: &str = "high";
pub const AVERAGE: &str = "average";
pub const LOW: &str = "low";

pub const SANCTUARY_RANGE: u32 = 3;
pub const WEAK_SANCTUARY_RANGE: u32 = 5;

pub const INIT_MONOLITH_SOULSHARDS: i32 = 10;

pub const MAX_PLAYER_ID: i32 = 1000;
pub const NPC_PLAYER_ID: i32 = 1000;

pub const BASE_REFINE_TIME: i32 = 30;
pub const MAX_CRAFTING_QUEUE: usize = 4;

pub const FIND_DRINK_TICKS: i32 = TICKS_PER_SEC * 3;
pub const FIND_FOOD_TICKS: i32 = TICKS_PER_SEC * 3;
pub const FIND_SHELTER_TICKS: i32 = TICKS_PER_SEC * 3;

/// How long a hero gather action takes, in real seconds. Used for both the
/// server-side event schedule (multiplied by TICKS_PER_SEC) and the client-side
/// cooldown countdown (which ticks once per real second), so the two stay in
/// sync — the client previously locked for 40s while the server completed in 4s.
pub const GATHER_TIME_SEC: i32 = 15;

pub const NO_SHELTER: i32 = -1;

// Loot POI (Supply Cache, Washed Ashore Materials) lifetimes. Abandoned caches
// auto-despawn after 5 minutes; once emptied they vanish shortly after, leaving
// a brief beat so the player sees the cache go empty first.
pub const LOOT_POI_DESPAWN_TICKS: i32 = TICKS_PER_SEC * 60 * 5; // 5 minutes
pub const LOOT_POI_EMPTY_DESPAWN_TICKS: i32 = TICKS_PER_SEC * 10; // 10 seconds

pub const IMAGE: &str = "image";
pub const TEMPLATE: &str = "template";
pub const PLAYER_ID: &str = "player_id";
pub const POSITION: &str = "position";
pub const VISION: &str = "vision";

pub const CLASS_STRUCTURE: &str = "structure";
pub const CLASS_UNIT: &str = "unit";
pub const CLASS_CORPSE: &str = "corpse";
pub const CLASS_POI: &str = "poi";

pub const SUBCLASS_HERO: &str = "hero";
pub const SUBCLASS_VILLAGER: &str = "villager";
pub const SUBCLASS_SHELTER: &str = "shelter";
pub const SUBCLASS_WALL: &str = "wall";
pub const SUBCLASS_MONOLITH: &str = "monolith";
pub const SUBCLASS_POI: &str = "poi";
pub const SUBCLASS_MERCHANT: &str = "merchant";
pub const SUBCLASS_TRANSPORT: &str = "transport";
pub const SUBCLASS_CRAFT: &str = "craft";
pub const SUBCLASS_STORAGE: &str = "storage";
pub const SUBCLASS_NPC: &str = "npc";
pub const SUBCLASS_WATCHTOWER: &str = "watchtower";
pub const SUBCLASS_RESOURCE: &str = "resource";
pub const SUBCLASS_FARM: &str = "farm";
pub const SUBCLASS_CORPSE: &str = "corpse";
pub const SUBCLASS_CAMPFIRE: &str = "campfire";

pub const GROUP_TAX_COLLECTOR: &str = "Tax Collector";

// States
pub const STATE_NONE: &str = "none";
pub const STATE_MOVING: &str = "moving";
pub const STATE_ATTACKING: &str = "attacking";
pub const STATE_CASTING: &str = "casting";
pub const STATE_DEAD: &str = "dead";
pub const STATE_FOUNDED: &str = "founded";
pub const STATE_PROGRESSING: &str = "progressing";
pub const STATE_BUILDING: &str = "building";
pub const STATE_PLANNING_UPGRADE: &str = "planning_upgrade";
pub const STATE_UPGRADING: &str = "upgrading";
pub const STATE_STALLED: &str = "stalled";
pub const STATE_GATHERING: &str = "gathering";
pub const STATE_REFINING: &str = "refining";
pub const STATE_OPERATING: &str = "operating";
pub const STATE_LUMBERJACKING: &str = "lumberjacking";
pub const STATE_MINING: &str = "mining";
pub const STATE_CRAFTING: &str = "crafting";
pub const STATE_EXPLORING: &str = "exploring";
pub const STATE_SURVEYING: &str = "surveying";
pub const STATE_PROSPECTING: &str = "prospecting";
pub const STATE_INVESTIGATING: &str = "investigating";
pub const STATE_DRINKING: &str = "drinking";
pub const STATE_EATING: &str = "eating";
pub const STATE_SLEEPING: &str = "sleeping";
pub const STATE_HIDING: &str = "hiding";
pub const STATE_EXPERIMENTING: &str = "experimenting";
pub const STATE_PLANTING: &str = "planting";
pub const STATE_HARVESTING: &str = "harvesting";
pub const STATE_FISHING: &str = "fishing";
pub const STATE_REPAIRING: &str = "repairing";
pub const STATE_ABOARD: &str = "aboard";
pub const STATE_BURNING: &str = "burning";

// Attributes
pub const CREATIVITY: &str = "Creativity";
pub const DEXTERITY: &str = "Dexterity";
pub const ENDURANCE: &str = "Endurance";
pub const FOCUS: &str = "Focus";
pub const INTELLECT: &str = "Intellect";
pub const SPIRIT: &str = "Spirit";
pub const STRENGTH: &str = "Strength";
pub const TOUGHNESS: &str = "Toughness";

// Thirst levels
pub const HYDRATED: &str = "Hydrated";
pub const REFRESHED: &str = "Refreshed";
pub const SLIGHTLY_THIRSTY: &str = "Slightly Thirsty";
pub const THIRSTY: &str = "Thirsty";
pub const PARCHED: &str = "Parched";
pub const DEHYDRATED: &str = "Dehydrated";

// Hunger levels
pub const SATIATED: &str = "Satiated";
pub const NOURISHED: &str = "Nourished";
pub const HUNGRY: &str = "Hungry";
pub const PECKISH: &str = "Peckish";
pub const FAMISHED: &str = "Famished";
pub const RAVENOUS: &str = "Ravenous";

// Tiredness levels
pub const ENERGIZED: &str = "Energized";
pub const RESTORED: &str = "Restored";
pub const WEARY: &str = "Weary";
pub const TIRED: &str = "Tired";
pub const EXHAUSTED: &str = "Exhausted";
pub const DEPELTED: &str = "Depleted";

// Resources
pub const ORE: &str = "Ore";
pub const LOG: &str = "Log";
pub const STONE: &str = "Stone";
//pub const WATER: &str = "Water";
pub const FOOD: &str = "Food";
pub const DRINK: &str = "Drink";
pub const PLANT: &str = "Plant";
pub const GAME_ANIMAL: &str = "Game Animal";
pub const SPRING_WATER: &str = "Spring Water";

pub const INGOT: &str = "Ingot";
pub const TIMBER: &str = "Timber";
pub const BLOCK: &str = "Block";

// Items
pub const FISH: &str = "Fish";

pub const GOLD_COINS: &str = "Gold Coins";
pub const WEAPON: &str = "Weapon";
pub const ARMOR: &str = "Armor";
pub const STICK: &str = "Stick";
pub const PLANT_FIBERS: &str = "Plant Fibers";
pub const PEBBLE: &str = "Pebble";
pub const BERRIES: &str = "Berries";
pub const MUSHROOM: &str = "Mushroom";
pub const PINE_NUTS: &str = "Pine Nuts";
pub const EDIBLE_BARK: &str = "Edible Bark";
pub const RESIN: &str = "Resin";
pub const HONEY: &str = "Honey";

pub const CONTAINER: &str = "Container";
pub const WATERSKIN_SUBCLASS: &str = "Waterskin";
pub const WATERSKIN_FILLED: &str = "Waterskin (Filled)";
pub const WATERSKIN_EMPTY: &str = "Waterskin (Empty)";
pub const TOOL: &str = "Tool";
pub const BUCKET: &str = "Bucket";
pub const WATER_BUCKET: &str = "Bucket of Water";
pub const FISHING_ROD: &str = "Fishing Rod";
pub const TORCH: &str = "Torch";
pub const CARP: &str = "Carp";
pub const LAKE_PERCH: &str = "Lake Perch";
pub const BEDROLL: &str = "Bedroll";

pub const UNLIT_TORCH: &str = "Unlit Torch";
pub const CRUDE_TORCH: &str = "Crude Torch";
pub const RESIN_TORCH: &str = "Resin Torch";
pub const LANTERN_TORCH: &str = "Lantern Torch";

pub const IGNITION_TOOL: &str = "Ignition Tool";

// Skills
pub const FISHING: &str = "Fishing";

// Network
pub const CREATING_HERO: &str = "CREATING_HERO";
pub const PLAYING: &str = "PLAYING";
pub const HERO_DEAD: &str = "HERO_DEAD";

// Database
pub const DATABASE_MANAGER_ID: i32 = 1;
