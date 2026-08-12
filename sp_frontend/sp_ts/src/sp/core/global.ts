import { ObjectState } from './objectState';
import { TileState } from './tileState';
import { MultiImage } from './multiImage';
import { WeatherState } from './weatherState';
import { ZIndexManager } from './zIndexManager';
import { ProtectedSettlementLookup } from './protectedSettlements';
import { SanctuaryZoneLookup } from './sanctuaryState';
import { SANCTUARY_BORDER_VISIBLE_BY_DEFAULT } from './sanctuaryBorderVisibility';
import { MAP_HEX_SIZE } from './mapGeometry';



export class Global {
    public static network;
    public static game;
    public static gameEmitter; 
    public static uiEmitter;
    public static gameWidth = 666;
    public static gameHeight = 375;
    public static connected = false;

    public static serverOffline = false;
    public static disconnected = false;
    public static networkError = false;

    public static heroDead = false;

    public static tick = 0;

    public static tileWidth = MAP_HEX_SIZE;
    public static tileHeight = MAP_HEX_SIZE;

    public static playerId = '-1';
    public static heroId = '-1';
    public static heroHp = 0;
    public static heroMaxHp = 0;
    public static heroStamina = 0;
    public static heroMaxStamina = 0;
    public static heroMana = 0;
    public static heroMaxMana = 0;
    public static heroClass = "";

    public static drawMapCompleted = false;
    public static effectTextOffsetY = 0;

    public static objectStates : Record<string, ObjectState> = {};
    public static shipwreckSearched = false;
    public static currentObjectiveId = '';
    public static protectedSettlements: ProtectedSettlementLookup = {};
    public static sanctuaryZones: SanctuaryZoneLookup = {};
    public static sanctuaryBorderVisible = SANCTUARY_BORDER_VISIBLE_BY_DEFAULT;
    public static tileStates : Record<string, TileState> = {};
    public static weatherStates : Record<string, WeatherState> = {};

    public static voidTiles = [];
    public static visibleTiles = [];

    public static tileset = {};
    public static imageDefList = {};

    public static selectedKey : any = {};

    public static selectedItemId = -1;
    public static selectedItemOwnerId = -1;
    public static selectedItemName = '';

    public static selectedUpgrade = "";

    public static wantedItemData;
    public static wantedItemName;

    public static infoItemAction = 'inventory';
    public static infoItemTransferAction = 'transfer';

    public static resourceLayerVisible = false;
    
    public static merchantSellTarget;

    public static attacks = [];
    public static combatState = null;

    public static noticeExpiry = 5000;

    public static equipPanelObjId = -1;
    public static showLeftInventoryPanel = true;

    public static isStructureRefining = false;

    public static accountSetupCompleted = false;
    public static accountName = '';

    public static pendingClassSelection: { className: string; heroName: string; portrait: string } | null = null;

    public static zIndexManager: ZIndexManager = new ZIndexManager();
}

// Expose Global to browser console for admin debug commands
declare global {
    interface Window {
        Global: typeof Global;
    }
}

if (typeof window !== 'undefined') {
    window.Global = Global;
}
