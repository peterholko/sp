// Deterministic "prepare-and-survive" bot for the headless harness.
//
// Reads a `WorldView` snapshot + the `Map` and returns at most one `PlayerEvent`
// per decision step. No RNG / no per-run seed.
//
// Behaviour, in priority order (the hero only accepts a new command while idle,
// i.e. `State::None`; villagers can be ordered even while the hero is busy):
//   1. Emergency heal — drink a healing item when HP is critical.
//   2. Retreat — when wounded and an enemy is close, fall back toward home.
//   3. Heal up — when wounded and safe, use a healing item.
//   4. Fight — attack adjacent enemies / close on nearby ones while healthy.
//   5. Opening — search the run-owned Shipwreck, recover its salvage, and build
//      the normal Burrow foundation from the five salvaged Logs.
//   6. Build — drive later build jobs, pulling resources from the Burrow and
//      depositing them into the foundation.
//   7. Fortify — once the campfire stands, ring the base with palisade walls.
//   8. Economy — order idle villagers to gather; forage resource tiles.
//   9. Explore — range out to deterministic waypoints when nothing else to do.
//
// Survival model (from the game): there is NO passive HP regen, so survival is
// about avoiding damage (retreat + walls + heal items); hunger/thirst/tiredness
// are auto-managed by the game when the hero is idle. Movement is single-hex-step
// because the server's MoveEvent only accepts a destination adjacent to the mover.

use crate::constants::{ATTACK_COOLDOWN_TICKS, WATERSKIN_EMPTY, WATERSKIN_FILLED};
use crate::crisis_balance::CrisisBalanceScenario;
use crate::game::{
    sanctuary_upgrade_cost, sanctuary_weak_radius, CrisisPhase, SANCTUARY_MAX_LEVEL,
};
use crate::headless::{HeroView, ItemView, StructureView, UnitView, WorldView};
use crate::map::{Map, TileType};
use crate::obj::{HeroClass, Position};
use crate::PlayerEvent;

// Engage enemies within this range. A lingering enemy keeps re-applying the
// combat lock, which blocks ALL eating/drinking/sleeping — so the hero must
// clear nearby harassers rather than passively ignore them, or it starves while
// standing idle holding food.
const AGGRO_RADIUS: u32 = 3;
const BASIC_ATTACK_STAMINA_COST: i32 = 5;
const GUARD_BASH_STAMINA_COST: i32 = 10;
const AIMED_SHOT_STAMINA_COST: i32 = 8;
const DISENGAGE_STAMINA_COST: i32 = 8;
const ARCANE_BOLT_MANA_COST: i32 = 20;
const WARD_MANA_COST: i32 = 15;
const CLASS_ABILITY_RANGE: u32 = 3;
// Defensive abilities share the production five-second attack cooldown. Two
// damaging commands between defensive casts keeps each class attacking instead
// of falling into a no-damage Disengage/Ward loop.
const OFFENSIVE_COMMANDS_PER_DEFENSIVE_ABILITY: u8 = 2;
const DANGER_RADIUS: u32 = 3; // an enemy this close counts as a threat for retreat
const SLEEP_SAFE_RADIUS: u32 = 3; // no enemy within this -> clear to rest/eat
const LOW_HP: f32 = 0.5; // retreat / heal below this HP fraction
const CRITICAL_HP: f32 = 0.3; // emergency heal below this
                              // Yield to the game's auto-consume just under its 75.0 trigger so the hero is
                              // already idle when a need crosses the threshold.
const CONSUME_AT: f32 = 70.0;
// Proactively top needs up at every safe window, well before CONSUME_AT, so the
// hero enters each enemy-pressure window with a big buffer. Sustained combat
// denies the idle windows needed to eat/drink/sleep, so banking buffer while calm
// is the main defense against needs-deaths early game.
const PROACTIVE_CONSUME: f32 = 45.0;
// Above this tiredness, surviving (resting) overrides fighting.
const CRITICAL_TIRED: f32 = 85.0;
// Above this hunger, eat raw meat now instead of taking time to cook it.
const CRITICAL_HUNGER: f32 = 85.0;
// Above this thirst, drinking outranks everything but fleeing a close enemy.
const CRITICAL_THIRST: f32 = 85.0;
// Feed value that counts as "real" food worth relying on. Foraged berries (~6) and
// mushrooms (~8) fall below it, so the hero hunts + cooks for proper meals.
const GOOD_FEED: f32 = 40.0;
// Food economy: build a reserve of proper meals (Cooked Meat, Feed 100) during the
// calm early days so the hero can eat through the crisis-heavy days (6+) when
// leaving base to hunt isn't possible. The hero keeps ON_HAND_FOOD meals on its
// person and banks the rest in the Burrow until the reserve hits STOCKPILE_TARGET.
const STOCKPILE_TARGET: i32 = 12; // ~20 hero-days of food banked before crises bite
const ON_HAND_FOOD: i32 = 3; // meals kept on the hero; surplus goes to the Burrow
const LOW_FIREWOOD: i32 = 4; // craft Firewood (1 Log -> 5) below this so cooking never stalls
const MAX_WALLS: usize = 6; // cap palisade walls ringed around the base
const WALL_RING: u32 = 2; // ring radius (leaves the inner tiles free to move)
const JOB_WATCHDOG_TICKS: i32 = 2400; // abandon a stuck build job after ~1 day
                                      // Villager economy: hire from the travelling merchant up to the Prosperity goal.
const HIRE_WAGE: i32 = 25; // Gold Coins charged per hire (matches the server)
const TARGET_VILLAGERS: usize = 3; // Prosperity victory wants 3 villagers

// Structure recipes the bot builds (req type -> quantity), matching the
// obj_template.yaml `req` fields. The revised opening builds a Burrow from five
// recovered Logs; later emergency Campfires use the recovered Stick+Resin, and
// Stockade walls use Logs.
const BURROW_REQS: &[(&str, i32)] = &[("Log", 5)];
const CAMPFIRE_REQS: &[(&str, i32)] = &[("Stick", 1), ("Resin", 1)];
const STOCKADE_REQS: &[(&str, i32)] = &[("Log", 3)];
// Resource type villagers can harvest tool-free (yields berries/grapes -> food).
const PLANT_RES: &str = "Plant";

const EXPLORE_OFFSETS: [(i32, i32); 8] = [
    (6, 0),
    (0, 6),
    (-6, 0),
    (0, -6),
    (10, 8),
    (-10, 8),
    (-10, -8),
    (10, -8),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Bootstrap,
    Build,
    Fortify,
    Survive,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BalanceBotPolicy {
    passive: bool,
    build_campfire: bool,
    max_walls: usize,
    recruit_shipwreck_villager: bool,
    hire_villagers: bool,
    upgrade_sanctuary: bool,
    stay_near_settlement_after_warning: bool,
}

impl BalanceBotPolicy {
    pub const fn for_scenario(scenario: CrisisBalanceScenario) -> Self {
        match scenario {
            CrisisBalanceScenario::Passive => Self {
                passive: true,
                build_campfire: false,
                max_walls: 0,
                recruit_shipwreck_villager: false,
                hire_villagers: false,
                upgrade_sanctuary: false,
                stay_near_settlement_after_warning: false,
            },
            CrisisBalanceScenario::BasicSurvival => Self {
                passive: false,
                build_campfire: true,
                max_walls: 0,
                recruit_shipwreck_villager: false,
                hire_villagers: false,
                upgrade_sanctuary: false,
                stay_near_settlement_after_warning: false,
            },
            CrisisBalanceScenario::PreparedSolo | CrisisBalanceScenario::HelperSupported => {
                Self::prepared_solo()
            }
            CrisisBalanceScenario::FortifiedSolo | CrisisBalanceScenario::NoVillagers => Self {
                passive: false,
                build_campfire: true,
                max_walls: MAX_WALLS,
                recruit_shipwreck_villager: false,
                hire_villagers: false,
                upgrade_sanctuary: true,
                stay_near_settlement_after_warning: true,
            },
            CrisisBalanceScenario::VillagerSupported => Self {
                passive: false,
                build_campfire: true,
                max_walls: MAX_WALLS,
                recruit_shipwreck_villager: true,
                hire_villagers: true,
                upgrade_sanctuary: true,
                stay_near_settlement_after_warning: true,
            },
            CrisisBalanceScenario::OrdinaryDisconnect
            | CrisisBalanceScenario::SafeLogoutBeforeAssault => Self {
                passive: false,
                build_campfire: true,
                max_walls: 3,
                recruit_shipwreck_villager: false,
                hire_villagers: false,
                upgrade_sanctuary: true,
                stay_near_settlement_after_warning: true,
            },
            CrisisBalanceScenario::AdjacentSettlement | CrisisBalanceScenario::Standard => {
                Self::standard()
            }
        }
    }

    const fn prepared_solo() -> Self {
        Self {
            passive: false,
            build_campfire: true,
            max_walls: 3,
            recruit_shipwreck_villager: false,
            hire_villagers: false,
            upgrade_sanctuary: true,
            stay_near_settlement_after_warning: true,
        }
    }

    const fn supporting_helper() -> Self {
        Self {
            passive: false,
            build_campfire: false,
            max_walls: 0,
            recruit_shipwreck_villager: false,
            hire_villagers: false,
            upgrade_sanctuary: false,
            stay_near_settlement_after_warning: false,
        }
    }

    pub const fn standard() -> Self {
        Self {
            passive: false,
            build_campfire: true,
            max_walls: MAX_WALLS,
            recruit_shipwreck_villager: true,
            hire_villagers: true,
            upgrade_sanctuary: true,
            stay_near_settlement_after_warning: false,
        }
    }
}

// A multi-step structure build the bot is currently driving.
struct BuildJob {
    structure_name: String, // "Burrow" / "Campfire" / "Stockade"
    subclass: String,       // expected subclass: "storage" / "campfire" / "wall"
    reqs: Vec<(String, i32)>,
    site: Position,
    structure_id: Option<i32>, // discovered once the foundation is placed
    last_build_issue_tick: Option<i32>,
    started_tick: i32,
}

pub struct Bot {
    player_id: i32,
    phase: Phase,
    anchor: Option<Position>, // spawn/home anchor
    explore_cursor: usize,
    job: Option<BuildJob>,
    walls_attempted: usize,
    shipwreck_search_issued: bool, // sent this run's one opening investigation
    upgrade_enabled: bool, // loot Soulshards + empower the sanctuary (BOT_NO_UPGRADE to disable)
    dbg_last_day: i32,     // last day a FOOD_DEBUG line was emitted
    hunts: u32,            // hunt actions issued (diagnostic)
    balance_policy: BalanceBotPolicy,
    // Dedicated multiplayer helper destination. When set, the hero travels by
    // ordinary Move events until adjacent to the owner's settlement, then holds
    // there and fights nearby enemies through the normal combat event path.
    helper_support_anchor: Option<Position>,
    // The settlement owner whose attributed personal-assault units this helper
    // deliberately supports. This is policy-only targeting context: emitted
    // events remain owned by `player_id` and production validates them normally.
    helper_supported_owner_id: Option<i32>,
    // During the owner's active personal assault, retain one attributed unit
    // until it dies/despawns instead of dropping combat as soon as it leaves the
    // old ambient-enemy aggro radius.
    assault_target_id: Option<i32>,
    // Ranger and Mage tactics intentionally obey the same shared cooldown as
    // production combat. The counters allow a defensive action only after two
    // offensive commands, avoiding both melee-only class simulations and
    // no-damage defensive loops.
    last_tactical_combat_command_tick: Option<i32>,
    ranger_disengaged_target_id: Option<i32>,
    ranger_offensive_commands_since_disengage: u8,
    // Test/audit marker for the most recent ordinary cooldown-reposition event.
    // It has no gameplay effect and is never sent to production systems.
    last_ranger_cooldown_reposition_tick: Option<i32>,
    mage_warded_target_id: Option<i32>,
    mage_offensive_commands_since_ward: u8,
}

impl Bot {
    pub fn new(player_id: i32) -> Self {
        Self::new_with_policy(player_id, BalanceBotPolicy::standard())
    }

    pub fn for_balance_scenario(player_id: i32, scenario: CrisisBalanceScenario) -> Self {
        Self::new_with_policy(player_id, BalanceBotPolicy::for_scenario(scenario))
    }

    pub fn for_helper_support(
        player_id: i32,
        supported_owner_id: i32,
        owner_settlement_anchor: Position,
    ) -> Self {
        let mut bot = Self::new_with_policy(player_id, BalanceBotPolicy::supporting_helper());
        bot.helper_support_anchor = Some(owner_settlement_anchor);
        bot.helper_supported_owner_id = Some(supported_owner_id);
        bot
    }

    fn new_with_policy(player_id: i32, balance_policy: BalanceBotPolicy) -> Self {
        Bot {
            player_id,
            phase: Phase::Bootstrap,
            anchor: None,
            explore_cursor: 0,
            job: None,
            walls_attempted: 0,
            shipwreck_search_issued: false,
            // A/B toggle for measuring the sanctuary loop's contribution.
            upgrade_enabled: balance_policy.upgrade_sanctuary
                && std::env::var("BOT_NO_UPGRADE").is_err(),
            dbg_last_day: -1,
            hunts: 0,
            balance_policy,
            helper_support_anchor: None,
            helper_supported_owner_id: None,
            assault_target_id: None,
            last_tactical_combat_command_tick: None,
            ranger_disengaged_target_id: None,
            ranger_offensive_commands_since_disengage: 0,
            last_ranger_cooldown_reposition_tick: None,
            mage_warded_target_id: None,
            mage_offensive_commands_since_ward: 0,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The owner-attributed assault unit retained by the headless policy. This
    /// exposes policy observation for opt-in telemetry only; callers still send
    /// ordinary production events and the server validates every action.
    pub fn observed_assault_target_id(&self) -> Option<i32> {
        self.assault_target_id
    }

    pub fn step(&mut self, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        let hero = view.hero?;
        if hero.dead || hero.true_death {
            return None;
        }
        if self.anchor.is_none() {
            self.anchor = Some(hero.pos);
        }

        // Abandon a build job that has run too long (stuck / unreachable site).
        if let Some(job) = &self.job {
            if view.game_tick - job.started_tick > JOB_WATCHDOG_TICKS {
                self.job = None;
            }
        }

        // Food economy probe (FOOD_DEBUG): once per day, show hunger, on-hand good
        // food + total edible feed, and the burrow stockpile — to see whether a
        // reserve ever builds.
        if std::env::var("FOOD_DEBUG").is_ok() && view.day != self.dbg_last_day {
            self.dbg_last_day = view.day;
            let good: i32 = view
                .inventory
                .iter()
                .filter(|i| i.is_edible() && i.feed >= GOOD_FEED)
                .map(|i| i.quantity)
                .sum();
            let feed_onhand: i32 = view
                .inventory
                .iter()
                .filter(|i| i.is_edible())
                .map(|i| i.quantity * i.feed as i32)
                .sum();
            let stock: i32 = view
                .structures
                .iter()
                .filter(|s| s.subclass == "storage")
                .flat_map(|s| &s.inventory)
                .filter(|i| i.class == "Food")
                .map(|i| i.quantity)
                .sum();
            let hides = count_matching(&view.inventory, "Hide");
            let has_tent = view
                .structures
                .iter()
                .any(|s| s.subclass == "craft" && s.built);
            let home = view.home().or(self.anchor).unwrap_or(hero.pos);
            let game_near = view
                .resource_tiles
                .iter()
                .filter(|t| t.has_game && hex_dist(home, t.pos) <= 20)
                .count();
            let game_revealed_near = view
                .resource_tiles
                .iter()
                .filter(|t| t.game_revealed && hex_dist(home, t.pos) <= 20)
                .count();
            let can_hunt = self.can_hunt_locally(&hero, view);
            let firewood = firewood_count(&view.inventory);
            let cf_pending = campfire_meat_pending(view);
            eprintln!(
                "[food] day={} hunger={:.0} thirst={:.0} state={:?} meals={} feed={} larder={} cf_meat={} hides={} can_hunt={} firewood={} hunts={}",
                view.day, hero.hunger, hero.thirst, hero.state, good, feed_onhand, stock,
                cf_pending, hides, can_hunt, firewood, self.hunts
            );
            let _ = (game_near, game_revealed_near);
            let _ = has_tent;
        }

        if std::env::var("BOT_DEBUG").is_ok() {
            let idle = view.villagers.iter().filter(|v| v.idle).count();
            let sfood: i32 = view
                .structures
                .iter()
                .filter(|s| s.subclass == "storage")
                .flat_map(|s| &s.inventory)
                .filter(|i| i.class == "Food")
                .map(|i| i.quantity)
                .sum();
            let gathering = view.villagers.iter().filter(|v| v.gathering_now).count();
            let with_order = view.villagers.iter().filter(|v| v.gathering_order).count();
            let carried: i32 = view.villagers.iter().map(|v| v.food_carried).sum();
            let plant_nodes = view
                .resource_tiles
                .iter()
                .filter(|t| t.plant_revealed)
                .count();
            let hgold = hero_gold(&view.inventory);
            let shards = hero_soulshards(&view.inventory);
            let sanc = view.monolith.map(|m| m.level).unwrap_or(-1);
            let corpses = view.corpses.len();
            let hdist = view
                .hero
                .as_ref()
                .zip(view.merchant.as_ref())
                .map(|(h, m)| hex_dist(h.pos, m.pos) as i32)
                .unwrap_or(-1);
            let (mstate, mhire, mpos) = match &view.merchant {
                Some(m) if m.at_landing => ("docked", m.hireable.len(), m.pos),
                Some(m) => ("sailing", m.hireable.len(), m.pos),
                None => ("none", 0, Position { x: -1, y: -1 }),
            };
            eprintln!(
                "[vil] t={} villagers={} gold={} merchant={} hireable={} sanc_lvl={} shards={} corpses={}",
                view.game_tick, view.villagers.len(), hgold, mstate, mhire, sanc, shards, corpses
            );
            let _ = (
                idle,
                with_order,
                gathering,
                carried,
                sfood,
                plant_nodes,
                mpos,
                hdist,
            );
        }

        // The hero acts only while idle; a busy hero still lets us command a villager.
        if hero.is_idle() {
            if let Some(action) = self.hero_action(view, map) {
                return Some(action);
            }
        }

        // The hero is busy or has no move: delegate to an idle villager if one can
        // be put to work on a Plant node the hero is standing on.
        self.villager_action(view)
    }

    pub fn advance_phase(&mut self, view: &WorldView) {
        let Some(hero) = view.hero else {
            self.phase = Phase::Done;
            return;
        };
        if hero.dead || hero.true_death {
            self.phase = Phase::Done;
            return;
        }
        self.phase = if !view.has_built("storage") || !view.has_built("campfire") {
            Phase::Build
        } else if view
            .structures
            .iter()
            .filter(|s| s.subclass == "wall")
            .count()
            < self.balance_policy.max_walls
        {
            Phase::Fortify
        } else {
            Phase::Survive
        };
    }

    // ---- Hero decision (only called while the hero is idle) -----------------

    fn hero_action(&mut self, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        let hero = view.hero?;
        let owned_assault_target = self.owned_assault_target(&hero, view);
        let nearest = owned_assault_target.or_else(|| nearest_enemy(hero.pos, view));
        let pursuing_assault = owned_assault_target.is_some();
        let threat = nearest
            .map(|e| hex_dist(hero.pos, e.pos) <= DANGER_RADIUS)
            .unwrap_or(false);

        let dist_to_enemy = nearest.map(|e| hex_dist(hero.pos, e.pos));
        let safe = dist_to_enemy.map_or(true, |d| d > SLEEP_SAFE_RADIUS);

        if std::env::var("BOT_DEBUG").is_ok() && hero.hunger >= 80.0 && view.game_tick % 200 == 0 {
            eprintln!(
                "[bot] t={} hp={:.0} hunger={:.0} thirst={:.0} tired={:.0} food={} dist_enemy={:?} safe={} state={:?}",
                view.game_tick, hero.hp as f32, hero.hunger, hero.thirst, hero.tired,
                has_class(&view.inventory, "Food"), dist_to_enemy, safe, hero.state,
            );
        }

        // 1. Emergency heal.
        if hero.hp_frac() < CRITICAL_HP {
            if let Some(item_id) = healing_item(view) {
                return Some(PlayerEvent::Use {
                    player_id: self.player_id,
                    obj_id: hero.id,
                    item_id,
                });
            }
        }

        // 2. Survival override: critically tired -> rest now. Break away from any
        //    nearby enemy first (and let the combat lock clear), then sleep. The
        //    hero has no passive recovery and exhaustion is lethal.
        if hero.tired >= CRITICAL_TIRED {
            if threat {
                let home = view.home().or(self.anchor).unwrap_or(hero.pos);
                if let Some(mv) = self.retreat_step(hero.pos, view, map, home) {
                    return Some(mv);
                }
            } else {
                return Some(PlayerEvent::Sleep {
                    player_id: self.player_id,
                    structure_id: 0,
                });
            }
        }

        // 2b. Critical hunger: securing food outranks base-building/exploring.
        //     Retreat from a close enemy first, otherwise run the food pipeline
        //     (butcher/cook/hunt/forage) even if not perfectly safe — otherwise the
        //     hero starves mid-task once its starting rations run out (~day 4).
        if hero.hunger >= CRITICAL_HUNGER && !has_edible(&view.inventory) {
            if threat {
                let home = view.home().or(self.anchor).unwrap_or(hero.pos);
                if let Some(mv) = self.retreat_step(hero.pos, view, map, home) {
                    return Some(mv);
                }
            }
            if let Some(action) = self.food_action(&hero, view, map) {
                return Some(action);
            }
        }

        // 2b-ii. Critical thirst: drink now (break from a close enemy first). There
        //        is no passive recovery and no auto-drink while combat-locked, so an
        //        un-handled thirst spike under pressure is a silent dehydration death.
        if hero.thirst >= CRITICAL_THIRST {
            if threat {
                let home = view.home().or(self.anchor).unwrap_or(hero.pos);
                if let Some(mv) = self.retreat_step(hero.pos, view, map, home) {
                    return Some(mv);
                }
            }
            if let Some(action) = self.consume_action(&hero, view, f32::MAX, CRITICAL_THIRST) {
                return Some(action);
            }
            if let Some(action) = self.water_action(&hero, view, map) {
                return Some(action);
            }
        }

        // 2b-iii. Hungry with food already in the pack: EAT FIRST — a single quick
        //         Use. This must outrank the cook pipeline below; previously the
        //         multi-step cook errands (walk to campfire, stage, craft, retrieve)
        //         preempted eating every decision tick and the hero starved to death
        //         while carrying cooked meals.
        if !threat && hero.hunger >= CONSUME_AT {
            if let Some(action) = self.consume_action(&hero, view, CONSUME_AT, f32::MAX) {
                return Some(action);
            }
        }

        // 2c. Once meat is in the pipeline (carcass/raw meat in hand, or staged /
        //     finishing at the campfire), see the butcher->cook through: raw meat
        //     can't be safely eaten (food poisoning) and a finished batch of Cooked
        //     Meat (Feed 100) is many days of food, so completing the cook outranks
        //     routine chores. Yield only to a close threat.
        if !threat && !pursuing_assault {
            let has_meat = view
                .inventory
                .iter()
                .any(|i| i.class == "Game Animal" || i.subclass == "Raw Meat")
                || campfire_meat_pending(view) > 0;
            if has_meat {
                if let Some(action) = self.food_action(&hero, view, map) {
                    return Some(action);
                }
            }
        }

        // 3. Retreat when wounded and threatened (heal first if possible).
        if hero.hp_frac() < LOW_HP && threat {
            if let Some(item_id) = healing_item(view) {
                return Some(PlayerEvent::Use {
                    player_id: self.player_id,
                    obj_id: hero.id,
                    item_id,
                });
            }
            let home = view.home().or(self.anchor).unwrap_or(hero.pos);
            if let Some(mv) = self.retreat_step(hero.pos, view, map, home) {
                return Some(mv);
            }
            // Cornered: fall through and fight.
        }

        // 4. Heal up while safe.
        if hero.hp_frac() < LOW_HP && !threat {
            if let Some(item_id) = healing_item(view) {
                return Some(PlayerEvent::Use {
                    player_id: self.player_id,
                    obj_id: hero.id,
                    item_id,
                });
            }
        }

        // 5. Ambient enemies retain the conservative short aggro radius. During
        //    AssaultActive, however, retain and chase an owner-attributed assault
        //    unit so the production wave cannot sit beyond radius 3 forever. Class
        //    actions still travel through ordinary Attack/Ability/Move events.
        if let Some(enemy) = nearest {
            let d = hex_dist(hero.pos, enemy.pos);
            if hero.hp_frac() >= LOW_HP && (pursuing_assault || d <= AGGRO_RADIUS) {
                if let Some(action) = self.class_combat_action(&hero, enemy, d, view, map) {
                    return Some(action);
                }
            }
        }
        if pursuing_assault && hero.hp_frac() >= LOW_HP {
            // A temporarily blocked path or depleted combat resource is a hold,
            // not permission to abandon the retained assault target for economy.
            return None;
        }

        // A support hero has no synthetic relocation or damage path: it walks
        // toward the owner's settlement one legal hex at a time, and the combat
        // branch above supplies its ordinary Move/Attack events when enemies are
        // close. Stop adjacent because the anchor is normally an occupied
        // settlement structure tile.
        if let Some(owner_anchor) = self.helper_support_anchor {
            if hex_dist(hero.pos, owner_anchor) > 1 {
                return self.step_toward(hero.pos, owner_anchor, view, map);
            }
            return None;
        }

        // Every active solo policy follows the production opening before it
        // assumes a storage settlement exists. The Shipwreck remains a neutral
        // POI, so the observation explicitly marks the one associated with this
        // run and exposes only that wreck's inventory. Investigation and each
        // transfer still travel through ordinary server-authoritative events.
        if !self.balance_policy.passive && !self.opening_complete(view) {
            return self.opening_action(&hero, view, map);
        }

        let crisis_hold = self.balance_policy.stay_near_settlement_after_warning
            && matches!(
                view.crisis_phase,
                Some(CrisisPhase::Preparing | CrisisPhase::AssaultReady)
            );
        if crisis_hold {
            if let Some(equip) = self.equip_combat_weapon(view) {
                return Some(equip);
            }
            let home = view.home().or(self.anchor).unwrap_or(hero.pos);
            if hex_dist(hero.pos, home) > 1 {
                if let Some(mv) = self.step_toward(hero.pos, home, view, map) {
                    return Some(mv);
                }
            }
        }

        // 6. Routine needs while safe: refill water, sleep, idle to auto-eat/drink.
        if safe {
            // Water: prospect a spring + refill so dehydration never sets in.
            if !crisis_hold {
                if let Some(action) = self.water_action(&hero, view, map) {
                    return Some(action);
                }
            }
            // Sleep BEFORE foraging: sleep is a short (30-tick) action, while a
            // forage is ~150 ticks — letting a forage pre-empt sleep is how the
            // hero ends up dying of exhaustion mid-gather. Rest proactively so the
            // hero banks a tiredness buffer for the next pressure window.
            if hero.tired >= PROACTIVE_CONSUME {
                return Some(PlayerEvent::Sleep {
                    player_id: self.player_id,
                    structure_id: 0, // ignored by the handler
                });
            }
            // Drink proactively (renewable), but only eat when actually hungry (food
            // is scarce). Do this BEFORE the longer food-gathering pipeline so a calm
            // moment isn't spent foraging while a drink/meal is already in the pack.
            if let Some(action) = self.consume_action(&hero, view, CONSUME_AT, PROACTIVE_CONSUME) {
                return Some(action);
            }
            // Food pipeline: butcher carcasses -> cook raw meat -> hunt/forage for
            // more. See food_action.
            if !crisis_hold {
                if let Some(action) = self.food_action(&hero, view, map) {
                    return Some(action);
                }
            }
        }

        // Hiring is a longer expansion trip, so it remains moderately needs-gated.
        let needs_comfortable = hero.hunger < 55.0 && hero.thirst < 55.0 && hero.tired < 55.0;

        // Passive policies deliberately stop before opening/economy actions.
        if self.balance_policy.passive {
            return None;
        }

        // 5. Build the base after the opening Burrow. The normal setup already
        //    supplies a lit Campfire; this fallback can still replace a missing one
        //    from the recovered Stick+Resin before later Stockades.
        if self.job.is_some() {
            if let Some(action) = self.advance_job(view, map) {
                return Some(action);
            }
        }
        if let Some(job) = self.next_build_job(view, map) {
            self.job = Some(job);
            if let Some(action) = self.advance_job(view, map) {
                return Some(action);
            }
        }

        // 4e. Hire more villagers from the travelling merchant, up to the
        //     Prosperity goal. Only when safe + needs comfortable (same as recruit).
        //     The hero pays in Gold Coins recovered from the wreck or later banked
        //     in the Burrow, then walks to the docked merchant and hires.
        if self.balance_policy.hire_villagers
            && safe
            && !crisis_hold
            && needs_comfortable
            && view.villagers.len() < TARGET_VILLAGERS
            && self.shipwreck_search_issued
            && self.balance_policy.recruit_shipwreck_villager
        {
            if let Some(action) = self.hire_action(&hero, view, map) {
                return Some(action);
            }
        }

        // 4f. Loot Soulshards off nearby corpses (fresh kills are usually adjacent),
        //     the currency for empowering the sanctuary. Only when safe.
        if self.upgrade_enabled && safe {
            if let Some(action) = self.loot_soulshards(&hero, view, map) {
                return Some(action);
            }
        }

        // 4g. Empower the Monolith sanctuary when we can afford the next level. This
        //     shrinks random spawns around the base — the primary early-game survival
        //     investment. Only when safe and needs have buffer (it's a short trip to
        //     the nearby Monolith).
        if self.upgrade_enabled && safe && needs_comfortable {
            if let Some(action) = self.upgrade_sanctuary_action(&hero, view, map) {
                return Some(action);
            }
        }

        // 7. Put an idle villager to work — but only opportunistically, never via a
        //    dedicated march (escorting the hero to a far Plant node to delegate
        //    food-gathering cost more survival time than the food was worth). If the
        //    hero happens to already be standing on a revealed Plant node (e.g.
        //    mid-forage) and a villager is idle, hold position one tick so
        //    villager_action can issue the OrderGather. The order then persists, so
        //    the villager keeps harvesting that node and hauling food to the Burrow.
        if safe && view.villagers.iter().any(|v| v.idle) {
            let here = view.resource_tiles.iter().find(|t| t.pos == hero.pos);
            if here.map_or(false, |t| t.plant_revealed) {
                return None;
            }
        }

        if crisis_hold {
            return None;
        }

        // 8. Forage: gather on a revealed resource tile, else walk to one.
        if let Some(action) = self.forage(view, map) {
            return Some(action);
        }

        // 9. Explore.
        self.explore(hero.pos, view, map)
    }

    // ---- Build-job state machine -------------------------------------------

    fn opening_complete(&self, view: &WorldView) -> bool {
        let Some(shipwreck) = view
            .pois
            .iter()
            .find(|poi| poi.template == "Shipwreck" && poi.run_owned)
        else {
            return false;
        };
        self.shipwreck_search_issued && shipwreck.inventory.is_empty() && view.has_built("storage")
    }

    // Drive only the revised new-run opening. Combat and emergency survival
    // decisions remain above this method, so a delayed build never suppresses an
    // immediate threat. Returning None while incomplete deliberately holds the
    // bot out of downstream economy code that assumes a Burrow exists.
    fn opening_action(
        &mut self,
        hero: &HeroView,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
        let (shipwreck_id, shipwreck_pos, salvage_item_id) = view
            .pois
            .iter()
            .find(|poi| poi.template == "Shipwreck" && poi.run_owned)
            .map(|poi| (poi.id, poi.pos, poi.inventory.first().map(|item| item.id)))?;

        if !self.shipwreck_search_issued {
            if Map::is_adjacent_including_source(hero.pos, shipwreck_pos) {
                self.shipwreck_search_issued = true;
                return Some(PlayerEvent::InvestigatePOI {
                    player_id: self.player_id,
                    target_id: shipwreck_id,
                });
            }
            return self.step_adjacent_to(hero.pos, shipwreck_pos, view, map);
        }

        // Transfer one ordinary inventory stack per decision. This is the same
        // manual POI-to-hero operation available to a player; no bot-only item
        // grant or synthetic storage path is used.
        if let Some(item_id) = salvage_item_id {
            if Map::is_adjacent_including_source(hero.pos, shipwreck_pos) {
                return Some(PlayerEvent::ItemTransfer {
                    player_id: self.player_id,
                    source_id: shipwreck_id,
                    target_id: hero.id,
                    item_id,
                });
            }
            return self.step_adjacent_to(hero.pos, shipwreck_pos, view, map);
        }

        // Preserve the Warrior's recovered class protection. Ranger weapon
        // selection remains combat-driven.
        if let Some(helm_id) = view
            .inventory
            .iter()
            .find(|item| item.name == "Copper Helm" && !item.equipped)
            .map(|item| item.id)
        {
            return Some(PlayerEvent::Equip {
                player_id: self.player_id,
                obj_id: hero.id,
                item_id: helm_id,
                status: true,
            });
        }

        // Recovering enough Logs removes the old mandatory gathering step, but
        // the shared stick is still every class's starter weapon and logging
        // fallback. Equip it through the normal player event before building.
        if let Some(stick_id) = view
            .inventory
            .iter()
            .find(|item| item.name == "Sharpened Stick" && !item.equipped)
            .map(|item| item.id)
        {
            return Some(PlayerEvent::Equip {
                player_id: self.player_id,
                obj_id: hero.id,
                item_id: stick_id,
                status: true,
            });
        }

        if !view.has_built("storage") {
            if actual_burrow_log_supply(view) < BURROW_REQS[0].1 {
                return self.logging_action(hero, view, map);
            }

            // A stale later-stage job must never leapfrog the opening shelter.
            if self
                .job
                .as_ref()
                .is_some_and(|job| job.structure_name != "Burrow")
            {
                self.job = None;
            }
            if self.job.is_none() {
                self.job = self.next_build_job(view, map);
            }
            return self.advance_job(view, map);
        }

        None
    }

    // Compatibility fallback for an incomplete salvage supply: equip the
    // starter-only Logging stick, reveal a real Log node through the ordinary
    // Prospect event, then gather until five actual Logs are carried.
    fn logging_action(&self, hero: &HeroView, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        if !view
            .inventory
            .iter()
            .any(|item| item.equipped && item.is_logging)
        {
            let item_id = view
                .inventory
                .iter()
                .find(|item| item.is_logging && item.quantity > 0)
                .map(|item| item.id)?;
            return Some(PlayerEvent::Equip {
                player_id: self.player_id,
                obj_id: hero.id,
                item_id,
                status: true,
            });
        }

        let here = view
            .resource_tiles
            .iter()
            .find(|resource| resource.pos == hero.pos);
        if here.is_some_and(|resource| resource.log_revealed) {
            return Some(PlayerEvent::Gather {
                player_id: self.player_id,
            });
        }
        if here.is_some_and(|resource| resource.has_log) {
            return Some(PlayerEvent::Prospect {
                player_id: self.player_id,
            });
        }

        let target = view
            .resource_tiles
            .iter()
            .filter(|resource| resource.has_log)
            .min_by_key(|resource| hex_dist(hero.pos, resource.pos))?;
        self.step_toward(hero.pos, target.pos, view, map)
    }

    // Pick the next structure to build: the opening Burrow, a replacement
    // Campfire if needed, then a ring of palisade walls around home.
    fn next_build_job(&mut self, view: &WorldView, map: &Map) -> Option<BuildJob> {
        let hero = view.hero?;
        let home = view.home().or(self.anchor).unwrap_or(hero.pos);

        if !view.has_built("storage") {
            if let Some(foundation) = view
                .structures
                .iter()
                .find(|structure| structure.subclass == "storage")
            {
                return Some(BuildJob::new(
                    "Burrow",
                    "storage",
                    BURROW_REQS,
                    foundation.pos,
                    view.game_tick,
                ));
            }
            if actual_log_count(&view.inventory) < BURROW_REQS[0].1 {
                return None;
            }
            let site = self.next_burrow_site(view, home, map)?;
            return Some(BuildJob::new(
                "Burrow",
                "storage",
                BURROW_REQS,
                site,
                view.game_tick,
            ));
        }

        // Campfire first (also a survival objective). Skip if one already exists.
        if self.balance_policy.build_campfire
            && !view.structures.iter().any(|s| s.subclass == "campfire")
        {
            let site = self.anchor.unwrap_or(hero.pos);
            // Only start if we can actually supply it (recovered Stick+Resin).
            if has_all_reqs(&view.inventory, CAMPFIRE_REQS) {
                return Some(BuildJob::new(
                    "Campfire",
                    "campfire",
                    CAMPFIRE_REQS,
                    site,
                    view.game_tick,
                ));
            }
            return None;
        }

        // NOTE: a Crafting Tent build target lived here, but it consumed the Burrow's
        // logs that the cook economy needs for Firewood (the hero has no other log
        // supply yet), which broke cooking and starved the hero. Gear progression is
        // parked (see docs/gear_progression_plan.md) until the hero has a sustainable
        // log supply (wood gathering) so building doesn't starve cooking.

        // Then ring the base with walls.
        let wall_count = view
            .structures
            .iter()
            .filter(|s| s.subclass == "wall")
            .count();
        if wall_count < self.balance_policy.max_walls
            && self.walls_attempted < self.balance_policy.max_walls
        {
            if let Some(site) = self.next_wall_site(view, home, map) {
                // Only commit if logs are reachable (in hand or in a storage).
                if has_all_reqs(&view.inventory, STOCKADE_REQS)
                    || storage_with_req(view, STOCKADE_REQS).is_some()
                {
                    self.walls_attempted += 1;
                    return Some(BuildJob::new(
                        "Stockade",
                        "wall",
                        STOCKADE_REQS,
                        site,
                        view.game_tick,
                    ));
                }
            }
        }

        None
    }

    fn next_burrow_site(&self, view: &WorldView, home: Position, map: &Map) -> Option<Position> {
        let mut adjacent = Map::range((home.x, home.y), 1)
            .into_iter()
            .collect::<Vec<_>>();
        adjacent.sort_unstable();
        adjacent
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .find(|position| {
                *position != home
                    && Map::is_valid_pos((position.x, position.y))
                    && Map::is_passable(position.x, position.y, map)
                    && !view.occupied.contains(&(position.x, position.y))
                    && !view
                        .structures
                        .iter()
                        .any(|structure| structure.pos == *position)
            })
    }

    fn advance_job(&mut self, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        let hero = view.hero?;
        // Take the job out so we can mutate freely, then put it back unless done.
        let mut job = self.job.take()?;

        let action = self.run_job(&mut job, view, map, hero.pos, hero.id);

        // Decide whether the job is finished.
        let done = match job.structure_id {
            Some(sid) => match view.structures.iter().find(|s| s.id == sid) {
                Some(s) => s.built, // complete when the structure finishes building
                None => true,       // structure vanished
            },
            None => false,
        };
        if !done {
            self.job = Some(job);
        }
        action
    }

    // Returns the next action for `job`, or None when waiting / impossible.
    fn run_job(
        &self,
        job: &mut BuildJob,
        view: &WorldView,
        map: &Map,
        hero_pos: Position,
        hero_id: i32,
    ) -> Option<PlayerEvent> {
        // Resolve the foundation if it already exists at the site.
        if job.structure_id.is_none() {
            if let Some(s) = view
                .structures
                .iter()
                .find(|s| s.pos == job.site && s.subclass == job.subclass && !s.built)
            {
                job.structure_id = Some(s.id);
            }
        }

        // Phase A: make sure the hero is carrying the required resources (only
        // needed before the foundation is filled). Pull from a storage if short.
        let foundation = job
            .structure_id
            .and_then(|sid| view.structures.iter().find(|s| s.id == sid));
        let foundation_filled = foundation
            .map(|f| f.building || has_all_reqs(&f.inventory, &as_req_slice(&job.reqs)))
            .unwrap_or(false);
        let carries_remaining_reqs = foundation
            .map(|foundation| {
                has_all_remaining_reqs(
                    &view.inventory,
                    &foundation.inventory,
                    &as_req_slice(&job.reqs),
                )
            })
            .unwrap_or_else(|| has_all_reqs(&view.inventory, &as_req_slice(&job.reqs)));

        if !foundation_filled && !carries_remaining_reqs {
            // Need more resources in hand — fetch from a storage (the Burrow).
            if let Some((storage_pos, storage_id, item_id)) =
                storage_item_for_missing(view, foundation, &job.reqs)
            {
                if Map::is_adjacent_including_source(hero_pos, storage_pos) {
                    return Some(PlayerEvent::ItemTransfer {
                        player_id: self.player_id,
                        source_id: storage_id,
                        target_id: hero_id,
                        item_id,
                    });
                }
                return self.step_adjacent_to(hero_pos, storage_pos, view, map);
            }
            // Can't acquire the resources — abandon by marking the job complete.
            job.structure_id = job.structure_id.or(Some(-1));
            return None;
        }

        // Phase B: get to the build site.
        if job.structure_id.is_none() {
            if hero_pos != job.site {
                return self.step_toward(hero_pos, job.site, view, map);
            }
            // Phase C: place the foundation on the hero's tile.
            return Some(PlayerEvent::CreateFoundation {
                player_id: self.player_id,
                source_id: hero_id,
                structure_name: job.structure_name.clone(),
            });
        }

        // Phase D: deposit required items into the foundation.
        if let Some(f) = foundation {
            if !foundation_filled {
                // Be on the foundation tile to transfer into it.
                if hero_pos != f.pos {
                    return self.step_toward(hero_pos, f.pos, view, map);
                }
                if let Some(item_id) = hero_item_for_missing(&view.inventory, f, &job.reqs) {
                    return Some(PlayerEvent::ItemTransfer {
                        player_id: self.player_id,
                        source_id: hero_id,
                        target_id: f.id,
                        item_id,
                    });
                }
                return None;
            }

            // Phase E: build it (builder must stand on the foundation).
            if f.founded || f.building {
                if hero_pos != f.pos {
                    return self.step_toward(hero_pos, f.pos, view, map);
                }
                // The authoritative handler can reject a build while the hero is
                // still combat-locked, and combat can interrupt an accepted build.
                // Retry after a bounded interval while the structure remains
                // Founded or Building; the production observer resumes a Building
                // structure without consuming its requirements again.
                if job
                    .last_build_issue_tick
                    .map_or(true, |tick| view.game_tick.saturating_sub(tick) >= 20)
                {
                    job.last_build_issue_tick = Some(view.game_tick);
                    return Some(PlayerEvent::Build {
                        player_id: self.player_id,
                        builder_id: hero_id,
                        structure_id: f.id,
                    });
                }
            }
            // Phase F: wait for completion between bounded resume attempts.
        }
        None
    }

    fn next_wall_site(&self, view: &WorldView, home: Position, map: &Map) -> Option<Position> {
        // Walls form a ring at WALL_RING around home, leaving inner tiles free.
        // Deterministic: scan ring tiles in sorted order, pick the first that is
        // valid, passable, unoccupied, and has no wall yet.
        let mut ring: Vec<(i32, i32)> = Map::range((home.x, home.y), WALL_RING)
            .into_iter()
            .filter(|(x, y)| hex_dist(Position { x: *x, y: *y }, home) == WALL_RING)
            .collect();
        ring.sort_unstable();

        for (x, y) in ring {
            if !Map::is_valid_pos((x, y)) || !Map::is_passable(x, y, map) {
                continue;
            }
            if view.occupied.contains(&(x, y)) {
                continue;
            }
            if view.structures.iter().any(|s| s.pos.x == x && s.pos.y == y) {
                continue;
            }
            return Some(Position { x, y });
        }
        None
    }

    // ---- Villager orders ----------------------------------------------------

    fn villager_action(&self, view: &WorldView) -> Option<PlayerEvent> {
        let villager = view.villagers.iter().find(|v| v.idle)?;
        let hero = view.hero?;
        // OrderGather is only valid when the hero stands on a revealed node of that
        // type. Plant needs no tool, so when the hero happens to be on a revealed
        // Plant node, order the idle villager to harvest it. The villager works that
        // spot and (once it has carried ~12 berries) hauls them to the Burrow, which
        // the hero eats from.
        if view
            .resource_tiles
            .iter()
            .any(|t| t.pos == hero.pos && t.plant_revealed)
        {
            return Some(PlayerEvent::OrderGather {
                player_id: self.player_id,
                source_id: villager.id,
                res_type: PLANT_RES.to_string(),
            });
        }
        None
    }

    // Hire a villager from the docked merchant. The wage is paid in Gold Coins from
    // the hero's pack; if later-earned gold was banked in the Burrow, the hero
    // withdraws it first. Returns None when no merchant is docked, nothing is for
    // hire, or there is no gold to be had.
    fn hire_action(&self, hero: &HeroView, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        const HIRE_MAX_DIST: u32 = 22; // don't trek across the map to the merchant
        let merchant = view.merchant.as_ref()?;
        if !merchant.at_landing || merchant.hireable.is_empty() {
            return None;
        }
        // Only bother when the docked merchant is reasonably near — a long escort
        // costs more survival time than a villager is worth.
        if hex_dist(hero.pos, merchant.pos) > HIRE_MAX_DIST {
            return None;
        }

        // Need the wage in hand; if short, pull a Gold Coins stack from the Burrow.
        if hero_gold(&view.inventory) < HIRE_WAGE {
            let (spos, sid, item_id) = storage_gold(view)?;
            if Map::is_adjacent_including_source(hero.pos, spos) {
                return Some(PlayerEvent::ItemTransfer {
                    player_id: self.player_id,
                    source_id: sid,
                    target_id: hero.id,
                    item_id,
                });
            }
            return self.step_adjacent_to(hero.pos, spos, view, map);
        }

        // Gold in hand: get next to the merchant and hire the first villager aboard.
        let target_id = *merchant.hireable.first()?;
        if Map::is_adjacent_including_source(hero.pos, merchant.pos) {
            return Some(PlayerEvent::Hire {
                player_id: self.player_id,
                merchant_id: merchant.id,
                target_id,
            });
        }
        self.step_adjacent_to(hero.pos, merchant.pos, view, map)
    }

    // Walk to the nearest corpse holding a Soulshard and loot it. Kills usually
    // drop adjacent, so this is normally a 0-1 tile detour right after a fight.
    fn loot_soulshards(&self, hero: &HeroView, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        const LOOT_RADIUS: u32 = 6;
        // No point hoarding once the sanctuary is maxed.
        if view
            .monolith
            .map_or(true, |m| m.level >= SANCTUARY_MAX_LEVEL)
        {
            return None;
        }
        let corpse = view
            .corpses
            .iter()
            .filter(|c| hex_dist(hero.pos, c.pos) <= LOOT_RADIUS)
            .min_by_key(|c| hex_dist(hero.pos, c.pos))?;
        if Map::is_adjacent_including_source(hero.pos, corpse.pos) {
            return Some(PlayerEvent::ItemTransfer {
                player_id: self.player_id,
                source_id: corpse.id,
                target_id: hero.id,
                item_id: corpse.soulshard_item,
            });
        }
        self.step_adjacent_to(hero.pos, corpse.pos, view, map)
    }

    // Empower the nearby Monolith when the hero can afford the next level. Walks
    // into the sanctuary's outer ring, then issues UpgradeSanctuary.
    fn upgrade_sanctuary_action(
        &self,
        hero: &HeroView,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
        let mono = view.monolith.as_ref()?;
        if mono.level >= SANCTUARY_MAX_LEVEL {
            return None;
        }
        if hero_soulshards(&view.inventory) < sanctuary_upgrade_cost(mono.level) {
            return None;
        }
        if hex_dist(hero.pos, mono.pos) <= sanctuary_weak_radius(mono.level) {
            return Some(PlayerEvent::UpgradeSanctuary {
                player_id: self.player_id,
                monolith_id: mono.id,
            });
        }
        self.step_adjacent_to(hero.pos, mono.pos, view, map)
    }

    // ---- Water -------------------------------------------------------------

    // Keep waterskins filled. Spring Water sits hidden under nearly every
    // grassland/plains hex, so rather than trekking to a river the hero prospects
    // in place to reveal a spring, then refills empties there (a revealed spring
    // refills infinitely). Returns Use (refill), Prospect (reveal a spring), a
    // step toward a known spring, or None when water is healthy / nothing to do.
    fn water_action(
        &mut self,
        hero: &HeroView,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
        let filled = count_name(&view.inventory, WATERSKIN_FILLED);
        if filled >= 3 {
            return None; // healthy water buffer
        }
        let empty_id = view
            .inventory
            .iter()
            .find(|i| i.name == WATERSKIN_EMPTY)
            .map(|i| i.id)?; // nothing to refill

        let here = view.resource_tiles.iter().find(|r| r.pos == hero.pos);
        let refillable_here = here.map_or(false, |t| t.spring_revealed)
            || Map::are_tile_types_nearby(hero.pos, vec![TileType::River], map);

        // Refill right here.
        if refillable_here {
            return Some(PlayerEvent::Use {
                player_id: self.player_id,
                obj_id: hero.id,
                item_id: empty_id,
            });
        }

        // A spring is under this tile but still hidden -> prospect to reveal it.
        if here.map_or(false, |t| t.has_spring) {
            return Some(PlayerEvent::Prospect {
                player_id: self.player_id,
            });
        }

        // No spring here: walk to the nearest known spring tile (or, if none are
        // known yet, prospect in place — most hexes hide a spring).
        if let Some(t) = view
            .resource_tiles
            .iter()
            .filter(|t| t.has_spring)
            .min_by_key(|t| hex_dist(hero.pos, t.pos))
        {
            if t.pos == hero.pos {
                return Some(PlayerEvent::Prospect {
                    player_id: self.player_id,
                });
            }
            return self.step_toward(hero.pos, t.pos, view, map);
        }

        Some(PlayerEvent::Prospect {
            player_id: self.player_id,
        })
    }

    // Explicitly eat an edible food / drink a waterskin when hungry/thirsty.
    // Eat/drink from the hero's pack, each with its own threshold. Drinking is
    // renewable (springs refill waterskins) so it's tended proactively, but food is
    // scarce — eating proactively just burns the larder faster and starves the hero,
    // so `eat_threshold` is kept high while `drink_threshold` can be low.
    fn consume_action(
        &self,
        hero: &HeroView,
        view: &WorldView,
        eat_threshold: f32,
        drink_threshold: f32,
    ) -> Option<PlayerEvent> {
        if hero.hunger >= eat_threshold {
            // Eat the HIGHEST-Feed food first (Cooked Meat 100 over berries ~6) so
            // the hero refills in one bite and isn't stuck eating constantly.
            if let Some(id) = view
                .inventory
                .iter()
                .filter(|i| i.is_edible())
                .max_by(|a, b| {
                    a.feed
                        .partial_cmp(&b.feed)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|i| i.id)
            {
                return Some(PlayerEvent::Use {
                    player_id: self.player_id,
                    obj_id: hero.id,
                    item_id: id,
                });
            }
        }
        if hero.thirst >= drink_threshold {
            if let Some(id) = view.inventory.iter().find(|i| i.is_drink()).map(|i| i.id) {
                return Some(PlayerEvent::Use {
                    player_id: self.player_id,
                    obj_id: hero.id,
                    item_id: id,
                });
            }
        }
        None
    }

    // ---- Food: butcher -> cook -> hunt -------------------------------------

    // Keep the hero fed. Butcher carcasses into raw meat, cook raw meat into
    // Cooked Meat at the campfire (more Feed), and when out of food hunt game
    // (raw meat) or pull from the Burrow / forage. Raw & cooked meat are both
    // `Food`, auto-eaten by the game when the hero is idle and hungry.
    fn food_action(&mut self, hero: &HeroView, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        // 1. Butcher a carcass (a "Felled X", class "Game Animal") into raw meat.
        if let Some(carcass) = view.inventory.iter().find(|i| i.class == "Game Animal") {
            return Some(PlayerEvent::Refine {
                player_id: self.player_id,
                item_id: carcass.id,
            });
        }

        // 2. Cook raw meat into Cooked Meat (the craft). Cooking is fast (20 ticks)
        //    and raw meat is poisonous, so always cook rather than eat it raw.
        //    The trigger must include meat ALREADY STAGED in the campfire and
        //    finished meals awaiting pickup — cook_action deposits raw meat into the
        //    campfire, so a hero-inventory-only check abandoned the cook the moment
        //    the meat left the pack (staged meat rotted unattended while the bot
        //    wandered off to hunt more).
        //    COOK FIRST: the campfire carries its own 20 Firewood, so cooking
        //    usually needs no hero-side fuel at all — only fall back
        //    to ensure_firewood (split a Burrow Log into 5 Firewood) when the cook
        //    can't proceed for lack of fuel anywhere.
        let has_raw_meat = view.inventory.iter().any(|i| i.subclass == "Raw Meat");
        if has_raw_meat || campfire_meat_pending(view) > 0 {
            if let Some(action) = self.cook_action(hero, view, map) {
                return Some(action);
            }
            if let Some(action) = self.ensure_firewood(hero, view, map) {
                return Some(action);
            }
        }

        let good_food = onhand_good_food(&view.inventory);

        // 2.5 Bank surplus meals into the Burrow larder. The hero keeps ON_HAND_FOOD
        //     on its person and stockpiles the rest, so a reserve actually
        //     accumulates instead of being eaten as fast as it's cooked.
        if good_food > ON_HAND_FOOD {
            if let Some(meal) = view
                .inventory
                .iter()
                .find(|i| i.is_edible() && i.feed >= GOOD_FEED)
            {
                if let Some(s) = view
                    .structures
                    .iter()
                    .find(|s| s.subclass == "storage" && s.built)
                {
                    if Map::is_adjacent_including_source(hero.pos, s.pos) {
                        return Some(PlayerEvent::ItemTransfer {
                            player_id: self.player_id,
                            source_id: hero.id,
                            target_id: s.id,
                            item_id: meal.id,
                        });
                    }
                    return self.step_adjacent_to(hero.pos, s.pos, view, map);
                }
            }
        }

        // 3. Build the food stockpile while calm: hunt + cook proper meals (Cooked
        //    Meat, Feed 100) until the reserve (on hand + banked in the Burrow) hits
        //    STOCKPILE_TARGET. Foraged berries (~6 Feed) only bootstrap the first
        //    day. Keep fuel topped up so the cook never stalls. Gated on calm needs
        //    so the hunt trip doesn't itself trigger a crisis.
        //
        //    Crucially, the reserve also counts UNPROCESSED kills (raw meat in the
        //    pack + meat still inside carcasses). Counting only cooked food made the
        //    gate stay true while the slow cook lagged behind, so the hero hunted
        //    nonstop (60+ hunts), never finished cooking, and starved with kills in
        //    hand. With pending meat counted, a couple of hunts satisfy the target
        //    and the bot switches to butchering + cooking what it already has.
        let pending_meat: i32 = view
            .inventory
            .iter()
            .filter(|i| i.subclass == "Raw Meat")
            .map(|i| i.quantity)
            .sum::<i32>()
            + view
                .inventory
                .iter()
                .filter(|i| i.class == "Game Animal")
                .map(|i| i.quantity)
                .sum::<i32>()
                * 4 // conservative meat per carcass (boar 6, hare 3)
            + campfire_meat_pending(view); // staged/finishing at the campfire
        let reserve = good_food + stored_good_food(view) + pending_meat;
        if reserve < STOCKPILE_TARGET
            && hero.tired < CONSUME_AT
            && hero.thirst < CONSUME_AT
            && self.can_hunt_locally(hero, view)
        {
            if let Some(action) = self.ensure_firewood(hero, view, map) {
                return Some(action);
            }
            if let Some(action) = self.hunt_action(hero, view, map) {
                self.hunts += 1;
                return Some(action);
            }
        }

        // 4. No good food and actually hungry: pull from the Burrow larder, else
        //    forage to limp along (early game / no campfire / no game nearby).
        if hero.hunger >= CONSUME_AT && good_food == 0 {
            if let Some((spos, sid, item_id)) = storage_food(view) {
                if Map::is_adjacent_including_source(hero.pos, spos) {
                    return Some(PlayerEvent::ItemTransfer {
                        player_id: self.player_id,
                        source_id: sid,
                        target_id: hero.id,
                        item_id,
                    });
                }
                return self.step_adjacent_to(hero.pos, spos, view, map);
            }
            return Some(PlayerEvent::Gather {
                player_id: self.player_id,
            });
        }

        None
    }

    // Hunt only when the carcass can be turned into Cooked Meat: a built campfire,
    // firewood on hand, and a game tile within reach of home. The radius is wide —
    // one hunt yields a big batch of meat (many days of food), so an occasional
    // longer trip is worth it.
    fn can_hunt_locally(&self, hero: &HeroView, view: &WorldView) -> bool {
        const HUNT_RADIUS: u32 = 20;
        if !view.has_built("campfire") {
            return false;
        }
        // Need fuel to cook the kill — Firewood on hand or in the campfire, or a
        // real Log (class "Log", not Timber) to split into 5 via ensure_firewood.
        let campfire_fuel = view
            .structures
            .iter()
            .filter(|s| s.subclass == "campfire" && s.built)
            .flat_map(|s| &s.inventory)
            .any(|i| i.name == "Firewood" && i.quantity > 0);
        let has_fuel = campfire_fuel
            || firewood_count(&view.inventory) > 0
            || view.inventory.iter().any(|i| i.class == "Log")
            || storage_log(view).is_some();
        if !has_fuel {
            return false;
        }
        let home = view.home().or(self.anchor).unwrap_or(hero.pos);
        view.resource_tiles
            .iter()
            .any(|t| t.has_game && hex_dist(home, t.pos) <= HUNT_RADIUS)
    }

    // Keep cooking fuel flowing when low: split a Log into Firewood (1 -> 5) if one
    // is in hand, otherwise withdraw a later-banked Log from the Burrow. Returns the
    // action to take, or None if firewood is fine / no logs are anywhere.
    fn ensure_firewood(&self, hero: &HeroView, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        if firewood_count(&view.inventory) >= LOW_FIREWOOD {
            return None;
        }
        if view.inventory.iter().any(|i| i.class == "Log") {
            if std::env::var("FOOD_DEBUG").is_ok() {
                eprintln!("[fuel] t={} crafting Firewood from Log", view.game_tick);
            }
            return Some(PlayerEvent::Craft {
                player_id: self.player_id,
                recipe_name: "Firewood".to_string(),
            });
        }
        // Fetch an ACTUAL Log (class "Log") from storage. Do not use the generic
        // req-matching here: matches_req conflates Timber with "Log" (fine for
        // build reqs), which made this fetch grab the Burrow's Timber stack — the
        // Firewood recipe can't use Timber, and the conflated "already have a Log"
        // count then stalled the fuel chain permanently.
        let (spos, sid, item_id) = storage_log(view)?;
        if std::env::var("FOOD_DEBUG").is_ok() {
            eprintln!("[fuel] t={} fetching Log from storage", view.game_tick);
        }
        if Map::is_adjacent_including_source(hero.pos, spos) {
            return Some(PlayerEvent::ItemTransfer {
                player_id: self.player_id,
                source_id: sid,
                target_id: hero.id,
                item_id,
            });
        }
        self.step_adjacent_to(hero.pos, spos, view, map)
    }

    // Hunt a Game Animal: equip the recovered Hunting weapon (Sharpened Stick),
    // then gather a revealed game tile (prospect to reveal one — game spawns under
    // grassland/plains hexes near the base). Yields a carcass to butcher in (1).
    fn hunt_action(&mut self, hero: &HeroView, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        let equipped_hunting = view.inventory.iter().any(|i| i.equipped && i.is_hunting);
        if !equipped_hunting {
            let id = view.inventory.iter().find(|i| i.is_hunting).map(|i| i.id)?;
            return Some(PlayerEvent::Equip {
                player_id: self.player_id,
                obj_id: hero.id,
                item_id: id,
                status: true,
            });
        }

        let here = view.resource_tiles.iter().find(|t| t.pos == hero.pos);
        if here.map_or(false, |t| t.game_revealed) {
            return Some(PlayerEvent::Gather {
                player_id: self.player_id,
            });
        }
        if here.map_or(false, |t| t.has_game) {
            return Some(PlayerEvent::Prospect {
                player_id: self.player_id,
            });
        }
        if let Some(t) = view
            .resource_tiles
            .iter()
            .filter(|t| t.has_game)
            .min_by_key(|t| hex_dist(hero.pos, t.pos))
        {
            if t.pos == hero.pos {
                return Some(PlayerEvent::Prospect {
                    player_id: self.player_id,
                });
            }
            return self.step_toward(hero.pos, t.pos, view, map);
        }
        Some(PlayerEvent::Prospect {
            player_id: self.player_id,
        })
    }

    // Cook raw meat into Cooked Meat at the campfire: deposit Raw Meat + Firewood
    // into the campfire, StructureCraft, then retrieve the cooked meat. Stateless —
    // each call inspects the campfire's inventory to pick the next step. Returns
    // None if there's no campfire / no firewood (then the raw meat is eaten raw).
    fn cook_action(&self, hero: &HeroView, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        let campfire = view
            .structures
            .iter()
            .find(|s| s.subclass == "campfire" && s.built)?;

        // Retrieve a finished Cooked Meat from the campfire.
        if let Some(cooked) = campfire
            .inventory
            .iter()
            .find(|i| i.subclass == "Cooked Meat")
        {
            if Map::is_adjacent_including_source(hero.pos, campfire.pos) {
                return Some(PlayerEvent::ItemTransfer {
                    player_id: self.player_id,
                    source_id: campfire.id,
                    target_id: hero.id,
                    item_id: cooked.id,
                });
            }
            return self.step_adjacent_to(hero.pos, campfire.pos, view, map);
        }

        let cf_has_meat = campfire.inventory.iter().any(|i| i.subclass == "Raw Meat");
        let cf_has_wood = campfire.inventory.iter().any(|i| i.name == "Firewood");

        // Ingredients are staged -> craft.
        if cf_has_meat && cf_has_wood {
            return Some(PlayerEvent::StructureCraft {
                player_id: self.player_id,
                structure_id: campfire.id,
                recipe_name: "Cooked Meat".to_string(),
            });
        }

        // Otherwise stage the ingredients (must be next to the campfire to transfer).
        if !Map::is_adjacent_including_source(hero.pos, campfire.pos) {
            return self.step_adjacent_to(hero.pos, campfire.pos, view, map);
        }
        if !cf_has_meat {
            let raw = view.inventory.iter().find(|i| i.subclass == "Raw Meat")?;
            return Some(PlayerEvent::ItemTransfer {
                player_id: self.player_id,
                source_id: hero.id,
                target_id: campfire.id,
                item_id: raw.id,
            });
        }
        if !cf_has_wood {
            let wood = view.inventory.iter().find(|i| i.name == "Firewood")?;
            return Some(PlayerEvent::ItemTransfer {
                player_id: self.player_id,
                source_id: hero.id,
                target_id: campfire.id,
                item_id: wood.id,
            });
        }
        None
    }

    /// Return the retained owner-attributed target for an active personal assault.
    /// Once selected, a target remains selected while it is present in the
    /// observation; a deterministic nearest/id tie-break chooses its replacement.
    fn owned_assault_target(&mut self, hero: &HeroView, view: &WorldView) -> Option<UnitView> {
        if self.helper_supported_owner_id.is_none()
            && view.crisis_phase != Some(CrisisPhase::AssaultActive)
        {
            self.assault_target_id = None;
            self.ranger_disengaged_target_id = None;
            self.ranger_offensive_commands_since_disengage = 0;
            self.mage_warded_target_id = None;
            self.mage_offensive_commands_since_ward = 0;
            return None;
        }

        let owner_player_id = self.helper_supported_owner_id.unwrap_or(self.player_id);
        let retained = self.assault_target_id.and_then(|target_id| {
            view.enemies
                .iter()
                .filter(|enemy| enemy.crisis_owner_player_id == Some(owner_player_id))
                .find(|enemy| enemy.id == target_id)
                .copied()
        });
        let target = retained.or_else(|| {
            view.enemies
                .iter()
                .filter(|enemy| enemy.crisis_owner_player_id == Some(owner_player_id))
                .filter(|enemy| hex_dist(hero.pos, enemy.pos) <= hero.vision)
                .min_by_key(|enemy| (hex_dist(hero.pos, enemy.pos), enemy.id))
                .copied()
        });
        let target_id = target.map(|enemy| enemy.id);
        if self.assault_target_id != target_id {
            self.ranger_disengaged_target_id = None;
            self.ranger_offensive_commands_since_disengage = 0;
            self.mage_warded_target_id = None;
            self.mage_offensive_commands_since_ward = 0;
        }
        self.assault_target_id = target_id;
        target
    }

    /// Emit only production-valid class combat commands. The bot does not apply
    /// damage or relocate actors directly; the server remains authoritative for
    /// cooldown, resource, range, accuracy, and hit resolution.
    fn class_combat_action(
        &mut self,
        hero: &HeroView,
        enemy: UnitView,
        distance: u32,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
        match hero.hero_class {
            HeroClass::Warrior => {
                if let Some(equip) = self.equip_combat_weapon(view) {
                    return Some(equip);
                }
                if distance <= 1 {
                    if hero.hp < hero.base_hp && has_resource(hero.stamina, GUARD_BASH_STAMINA_COST)
                    {
                        return Some(PlayerEvent::Ability {
                            player_id: self.player_id,
                            ability_id: "shield_bash".to_string(),
                            source_id: hero.id,
                            target_id: Some(enemy.id),
                        });
                    }
                    return has_resource(hero.stamina, BASIC_ATTACK_STAMINA_COST)
                        .then(|| self.basic_attack(hero.id, enemy.id));
                }
            }
            HeroClass::Ranger => {
                if let Some(equip) = self.equip_training_bow(view) {
                    return Some(equip);
                }
                let bow_range = equipped_training_bow_range(view);
                let tactical_command_ready = self.tactical_combat_command_ready(view.game_tick);
                // Use the production movement path while the shared combat
                // cooldown runs. This preserves the bow's ordinary range-two
                // advantage without resetting the cooldown or manufacturing
                // damage; once the cooldown expires, offense still wins below.
                if distance <= 1 && hero.hp < hero.base_hp && !tactical_command_ready {
                    let reposition =
                        self.ranger_bow_reposition_step(hero.pos, enemy.pos, bow_range, view, map);
                    if reposition.is_some() {
                        self.last_ranger_cooldown_reposition_tick = Some(view.game_tick);
                    }
                    return reposition;
                }
                let disengage_due = self.ranger_disengaged_target_id != Some(enemy.id)
                    || self.ranger_offensive_commands_since_disengage
                        >= OFFENSIVE_COMMANDS_PER_DEFENSIVE_ABILITY;
                if distance <= 1
                    && hero.hp < hero.base_hp
                    && disengage_due
                    && has_resource(hero.stamina, DISENGAGE_STAMINA_COST)
                    && disengage_destination_is_open(hero.pos, enemy.pos, view, map)
                {
                    debug_assert!(tactical_command_ready);
                    self.last_tactical_combat_command_tick = Some(view.game_tick);
                    self.ranger_disengaged_target_id = Some(enemy.id);
                    self.ranger_offensive_commands_since_disengage = 0;
                    return Some(PlayerEvent::Ability {
                        player_id: self.player_id,
                        ability_id: "disengage".to_string(),
                        source_id: hero.id,
                        target_id: Some(enemy.id),
                    });
                }
                if distance <= bow_range && has_resource(hero.stamina, BASIC_ATTACK_STAMINA_COST) {
                    if !tactical_command_ready {
                        return None;
                    }
                    self.last_tactical_combat_command_tick = Some(view.game_tick);
                    self.ranger_offensive_commands_since_disengage = self
                        .ranger_offensive_commands_since_disengage
                        .saturating_add(1);
                    return Some(self.basic_attack(hero.id, enemy.id));
                }
                if bow_range > 0
                    && distance <= CLASS_ABILITY_RANGE
                    && has_resource(hero.stamina, AIMED_SHOT_STAMINA_COST)
                {
                    if !tactical_command_ready {
                        return None;
                    }
                    self.last_tactical_combat_command_tick = Some(view.game_tick);
                    self.ranger_offensive_commands_since_disengage = self
                        .ranger_offensive_commands_since_disengage
                        .saturating_add(1);
                    return Some(PlayerEvent::Ability {
                        player_id: self.player_id,
                        ability_id: "aimed_shot".to_string(),
                        source_id: hero.id,
                        target_id: Some(enemy.id),
                    });
                }
            }
            HeroClass::Mage => {
                let ward_due = self.mage_warded_target_id != Some(enemy.id)
                    || self.mage_offensive_commands_since_ward
                        >= OFFENSIVE_COMMANDS_PER_DEFENSIVE_ABILITY;
                if distance <= 1
                    && hero.hp < hero.base_hp
                    && ward_due
                    && has_resource(hero.mana, WARD_MANA_COST)
                {
                    if !self.tactical_combat_command_ready(view.game_tick) {
                        return None;
                    }
                    self.last_tactical_combat_command_tick = Some(view.game_tick);
                    self.mage_warded_target_id = Some(enemy.id);
                    self.mage_offensive_commands_since_ward = 0;
                    return Some(PlayerEvent::Ability {
                        player_id: self.player_id,
                        ability_id: "ward".to_string(),
                        source_id: hero.id,
                        target_id: None,
                    });
                }
                if distance <= CLASS_ABILITY_RANGE && has_resource(hero.mana, ARCANE_BOLT_MANA_COST)
                {
                    if !self.tactical_combat_command_ready(view.game_tick) {
                        return None;
                    }
                    self.last_tactical_combat_command_tick = Some(view.game_tick);
                    self.mage_offensive_commands_since_ward =
                        self.mage_offensive_commands_since_ward.saturating_add(1);
                    return Some(PlayerEvent::Ability {
                        player_id: self.player_id,
                        ability_id: "arcane_bolt".to_string(),
                        source_id: hero.id,
                        target_id: Some(enemy.id),
                    });
                }
                if let Some(equip) = self.equip_combat_weapon(view) {
                    return Some(equip);
                }
                if distance <= 1 && has_resource(hero.stamina, BASIC_ATTACK_STAMINA_COST) {
                    if !self.tactical_combat_command_ready(view.game_tick) {
                        return None;
                    }
                    self.last_tactical_combat_command_tick = Some(view.game_tick);
                    self.mage_offensive_commands_since_ward =
                        self.mage_offensive_commands_since_ward.saturating_add(1);
                    return Some(self.basic_attack(hero.id, enemy.id));
                }
            }
        }

        self.step_toward(hero.pos, enemy.pos, view, map)
    }

    fn tactical_combat_command_ready(&self, game_tick: i32) -> bool {
        self.last_tactical_combat_command_tick
            .is_none_or(|last_tick| game_tick.saturating_sub(last_tick) >= ATTACK_COOLDOWN_TICKS)
    }

    fn ranger_bow_reposition_step(
        &self,
        from: Position,
        enemy: Position,
        bow_range: u32,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
        if bow_range == 0 {
            return None;
        }
        let current_distance = hex_dist(from, enemy);
        let destination =
            self.best_neighbor(from, view, map, |position| hex_dist(position, enemy) as i32)?;
        let destination_distance = hex_dist(destination, enemy);
        (destination_distance > current_distance && destination_distance <= bow_range)
            .then(|| self.move_to(destination))
    }

    fn basic_attack(&self, source_id: i32, target_id: i32) -> PlayerEvent {
        PlayerEvent::Attack {
            player_id: self.player_id,
            attack_type: "quick".to_string(),
            source_id,
            target_id,
        }
    }

    /// Equip the recovered production bow even though it also carries the Hunting
    /// attribute. The old generic combat selector excluded all hunting weapons and
    /// therefore swapped Rangers away from their only legitimate ranged weapon.
    fn equip_training_bow(&self, view: &WorldView) -> Option<PlayerEvent> {
        let hero = view.hero?;
        if view
            .inventory
            .iter()
            .any(|item| item.name == "Training Bow" && item.equipped && item.quantity > 0)
        {
            return None;
        }
        let item_id = view
            .inventory
            .iter()
            .find(|item| item.name == "Training Bow" && !item.equipped && item.quantity > 0)
            .map(|item| item.id)?;
        Some(PlayerEvent::Equip {
            player_id: self.player_id,
            obj_id: hero.id,
            item_id,
            status: true,
        })
    }

    // Equip the strongest non-hunting weapon (the axe) if it isn't already — used
    // before combat, since hunting swaps in the weak Hunting spear.
    fn equip_combat_weapon(&self, view: &WorldView) -> Option<PlayerEvent> {
        let hero = view.hero?;
        if view
            .inventory
            .iter()
            .any(|i| i.equipped && i.is_weapon && !i.is_hunting)
        {
            return None; // a combat weapon is already equipped
        }
        let id = view
            .inventory
            .iter()
            .find(|i| i.is_weapon && !i.is_hunting && !i.equipped)
            .map(|i| i.id)?;
        Some(PlayerEvent::Equip {
            player_id: self.player_id,
            obj_id: hero.id,
            item_id: id,
            status: true,
        })
    }

    // ---- Economy / movement helpers ----------------------------------------

    fn forage(&self, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        let hero = view.hero?;
        // On a revealed resource tile -> gather.
        if view
            .resource_tiles
            .iter()
            .any(|r| r.revealed && r.pos == hero.pos)
        {
            return Some(PlayerEvent::Gather {
                player_id: self.player_id,
            });
        }
        // Otherwise walk toward the nearest revealed resource tile within reach.
        let target = view
            .resource_tiles
            .iter()
            .filter(|r| r.revealed && hex_dist(hero.pos, r.pos) <= 10)
            .min_by_key(|r| hex_dist(hero.pos, r.pos))
            .map(|r| r.pos)?;
        self.step_toward(hero.pos, target, view, map)
    }

    fn explore(&mut self, from: Position, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        let anchor = self.anchor.unwrap_or(from);
        for _ in 0..EXPLORE_OFFSETS.len() {
            let (ox, oy) = EXPLORE_OFFSETS[self.explore_cursor];
            let wp = clamp_pos(Position {
                x: anchor.x + ox,
                y: anchor.y + oy,
            });
            if hex_dist(from, wp) <= 2 {
                self.explore_cursor = (self.explore_cursor + 1) % EXPLORE_OFFSETS.len();
                continue;
            }
            if let Some(mv) = self.step_toward(from, wp, view, map) {
                return Some(mv);
            }
            self.explore_cursor = (self.explore_cursor + 1) % EXPLORE_OFFSETS.len();
        }
        None
    }

    // Greedy one-hex step toward `target`: the adjacent passable, unoccupied tile
    // that most reduces hex distance. None if nothing gets us strictly closer.
    fn step_toward(
        &self,
        from: Position,
        target: Position,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
        let current = hex_dist(from, target);
        if current == 0 {
            return None;
        }
        let best = self.best_neighbor(from, view, map, |p| {
            // smaller distance-to-target is better -> negate for "score"
            -(hex_dist(p, target) as i32)
        })?;
        if hex_dist(best, target) >= current {
            return None;
        }
        Some(self.move_to(best))
    }

    // Step to a tile ADJACENT to `target` (not onto it) — for transferring with a
    // structure you must not stand on (e.g. the Burrow).
    fn step_adjacent_to(
        &self,
        from: Position,
        target: Position,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
        if Map::is_adjacent_including_source(from, target) {
            return None;
        }
        self.step_toward(from, target, view, map)
    }

    // Retreat: the adjacent passable, unoccupied tile that MAXIMISES distance to
    // the nearest enemy, tie-broken by getting closer to home.
    fn retreat_step(
        &self,
        from: Position,
        view: &WorldView,
        map: &Map,
        home: Position,
    ) -> Option<PlayerEvent> {
        let enemy = nearest_enemy(from, view)?.pos;
        let best = self.best_neighbor(from, view, map, |p| {
            // maximise enemy distance (primary), minimise home distance (secondary)
            (hex_dist(p, enemy) as i32) * 100 - (hex_dist(p, home) as i32)
        })?;
        // Only retreat if it actually increases distance from the enemy.
        if hex_dist(best, enemy) <= hex_dist(from, enemy) {
            return None;
        }
        Some(self.move_to(best))
    }

    // Best adjacent tile by a scoring closure (higher is better); only considers
    // valid, passable, unoccupied neighbours.
    fn best_neighbor(
        &self,
        from: Position,
        view: &WorldView,
        map: &Map,
        score: impl Fn(Position) -> i32,
    ) -> Option<Position> {
        let mut best: Option<(Position, i32)> = None;
        for (nx, ny) in Map::range((from.x, from.y), 1) {
            if nx == from.x && ny == from.y {
                continue;
            }
            if !Map::is_valid_pos((nx, ny)) || !Map::is_passable(nx, ny, map) {
                continue;
            }
            if view.occupied.contains(&(nx, ny)) {
                continue;
            }
            let p = Position { x: nx, y: ny };
            let s = score(p);
            match best {
                Some((_, bs)) if s <= bs => {}
                _ => best = Some((p, s)),
            }
        }
        best.map(|(p, _)| p)
    }

    fn move_to(&self, p: Position) -> PlayerEvent {
        PlayerEvent::Move {
            player_id: self.player_id,
            x: p.x,
            y: p.y,
        }
    }
}

impl BuildJob {
    fn new(name: &str, subclass: &str, reqs: &[(&str, i32)], site: Position, tick: i32) -> Self {
        BuildJob {
            structure_name: name.to_string(),
            subclass: subclass.to_string(),
            reqs: reqs.iter().map(|(t, q)| (t.to_string(), *q)).collect(),
            site,
            structure_id: None,
            last_build_issue_tick: None,
            started_tick: tick,
        }
    }
}

// ---- Free helpers ----------------------------------------------------------

fn hex_dist(a: Position, b: Position) -> u32 {
    Map::distance((a.x, a.y), (b.x, b.y))
}

fn clamp_pos(p: Position) -> Position {
    Position {
        x: p.x.clamp(0, crate::map::WIDTH - 1),
        y: p.y.clamp(0, crate::map::HEIGHT - 1),
    }
}

fn nearest_enemy(from: Position, view: &WorldView) -> Option<UnitView> {
    view.enemies
        .iter()
        .min_by_key(|e| hex_dist(from, e.pos))
        .copied()
}

fn has_resource(resource: Option<i32>, cost: i32) -> bool {
    resource.is_some_and(|available| available >= cost)
}

fn equipped_training_bow_range(view: &WorldView) -> u32 {
    view.inventory
        .iter()
        .filter(|item| item.name == "Training Bow" && item.equipped && item.quantity > 0)
        .map(|item| item.attack_range)
        .max()
        .unwrap_or(0)
}

fn disengage_destination_is_open(
    from: Position,
    target: Position,
    view: &WorldView,
    map: &Map,
) -> bool {
    let Some(destination) = disengage_destination(from, target) else {
        return false;
    };
    Map::is_valid_pos((destination.x, destination.y))
        && Map::is_passable(destination.x, destination.y, map)
        && !view.occupied.contains(&(destination.x, destination.y))
}

fn disengage_destination(from: Position, target: Position) -> Option<Position> {
    let dx = (from.x - target.x).signum();
    let dy = (from.y - target.y).signum();
    if dx == 0 && dy == 0 {
        return None;
    }
    Some(Position {
        x: from.x + dx,
        y: from.y + dy,
    })
}

fn healing_item(view: &WorldView) -> Option<i32> {
    view.inventory.iter().find(|i| i.is_healing).map(|i| i.id)
}

fn has_class(items: &[ItemView], class: &str) -> bool {
    items.iter().any(|i| i.class == class && i.quantity > 0)
}

fn count_name(items: &[ItemView], name: &str) -> i32 {
    items
        .iter()
        .filter(|i| i.name == name)
        .map(|i| i.quantity)
        .sum()
}

fn has_edible(items: &[ItemView]) -> bool {
    items.iter().any(|i| i.is_edible())
}

fn as_req_slice(reqs: &[(String, i32)]) -> Vec<(&str, i32)> {
    reqs.iter().map(|(t, q)| (t.as_str(), *q)).collect()
}

fn count_matching(items: &[ItemView], req_type: &str) -> i32 {
    items
        .iter()
        .filter(|i| i.matches_req(req_type))
        .map(|i| i.quantity)
        .sum()
}

fn actual_log_count(items: &[ItemView]) -> i32 {
    items
        .iter()
        .filter(|item| item.class == "Log")
        .map(|item| item.quantity)
        .sum()
}

fn actual_burrow_log_supply(view: &WorldView) -> i32 {
    actual_log_count(&view.inventory)
        + view
            .structures
            .iter()
            .filter(|structure| structure.subclass == "storage" && !structure.built)
            .map(|structure| actual_log_count(&structure.inventory))
            .sum::<i32>()
}

fn matches_req_exactly(item: &ItemView, req_type: &str) -> bool {
    req_type == item.name || req_type == item.class || req_type == item.subclass
}

// Construction permits Timber as a Log substitute, but the opening should
// consume its five actual Logs first and preserve the single valuable Timber.
fn preferred_item_for_req<'a>(items: &'a [ItemView], req_type: &str) -> Option<&'a ItemView> {
    items
        .iter()
        .find(|item| matches_req_exactly(item, req_type))
        .or_else(|| items.iter().find(|item| item.matches_req(req_type)))
}

fn has_all_reqs(items: &[ItemView], reqs: &[(&str, i32)]) -> bool {
    reqs.iter().all(|(t, q)| count_matching(items, t) >= *q)
}

fn has_all_remaining_reqs(carried: &[ItemView], staged: &[ItemView], reqs: &[(&str, i32)]) -> bool {
    reqs.iter().all(|(req_type, quantity)| {
        let remaining = quantity.saturating_sub(count_matching(staged, req_type));
        count_matching(carried, req_type) >= remaining
    })
}

// Find a still-missing requirement that some owned storage holds; return
// (storage_pos, storage_id, item_id) to pull one matching stack to the hero.
fn storage_item_for_missing(
    view: &WorldView,
    foundation: Option<&StructureView>,
    reqs: &[(String, i32)],
) -> Option<(Position, i32, i32)> {
    for (req_type, need) in reqs {
        let staged = foundation
            .map(|foundation| count_matching(&foundation.inventory, req_type))
            .unwrap_or(0);
        if count_matching(&view.inventory, req_type) + staged >= *need {
            continue;
        }

        // Search every storage for an exact item before accepting a legal
        // substitute from any storage.
        for exact_only in [true, false] {
            for s in view
                .structures
                .iter()
                .filter(|s| s.subclass == "storage" && s.built)
            {
                if let Some(item) = s.inventory.iter().find(|item| {
                    if exact_only {
                        matches_req_exactly(item, req_type)
                    } else {
                        item.matches_req(req_type)
                    }
                }) {
                    return Some((s.pos, s.id, item.id));
                }
            }
        }
    }
    None
}

// A Food item sitting in an owned storage (e.g. the Burrow's berries) to pull.
fn storage_food(view: &WorldView) -> Option<(Position, i32, i32)> {
    for s in view
        .structures
        .iter()
        .filter(|s| s.subclass == "storage" && s.built)
    {
        if let Some(item) = s
            .inventory
            .iter()
            .find(|i| i.class == "Food" && i.quantity > 0)
        {
            return Some((s.pos, s.id, item.id));
        }
    }
    None
}

// Total Gold Coins the hero is carrying.
fn hero_gold(inventory: &[ItemView]) -> i32 {
    inventory
        .iter()
        .filter(|i| i.class == "Gold Coins")
        .map(|i| i.quantity)
        .sum()
}

// Total Soulshards the hero is carrying (currency for sanctuary upgrades).
fn hero_soulshards(inventory: &[ItemView]) -> i32 {
    inventory
        .iter()
        .filter(|i| i.class == "Soulshard")
        .map(|i| i.quantity)
        .sum()
}

// Firewood the hero is carrying (cooking fuel).
fn firewood_count(inventory: &[ItemView]) -> i32 {
    inventory
        .iter()
        .filter(|i| i.name == "Firewood")
        .map(|i| i.quantity)
        .sum()
}

// Proper meals (high-Feed food) the hero is carrying.
fn onhand_good_food(inventory: &[ItemView]) -> i32 {
    inventory
        .iter()
        .filter(|i| i.is_edible() && i.feed >= GOOD_FEED)
        .map(|i| i.quantity)
        .sum()
}

// Proper meals banked in owned storage (the Burrow larder).
fn stored_good_food(view: &WorldView) -> i32 {
    view.structures
        .iter()
        .filter(|s| s.subclass == "storage" && s.built)
        .flat_map(|s| &s.inventory)
        .filter(|i| i.is_edible() && i.feed >= GOOD_FEED)
        .map(|i| i.quantity)
        .sum()
}

// Food mid-pipeline at the campfire: raw meat staged for cooking plus finished
// Cooked Meat awaiting pickup. The cook loop and the hunt gate must both see this
// — meat deposited into the campfire is invisible to hero-inventory checks, which
// previously orphaned the cook and re-opened the hunt gate.
fn campfire_meat_pending(view: &WorldView) -> i32 {
    view.structures
        .iter()
        .filter(|s| s.subclass == "campfire" && s.built)
        .flat_map(|s| &s.inventory)
        .filter(|i| i.subclass == "Raw Meat" || i.subclass == "Cooked Meat")
        .map(|i| i.quantity)
        .sum()
}

// An actual Log stack (class "Log" strictly — Timber doesn't smelt into Firewood)
// sitting in an owned storage. Returns (storage_pos, storage_id, item_id).
fn storage_log(view: &WorldView) -> Option<(Position, i32, i32)> {
    for s in view
        .structures
        .iter()
        .filter(|s| s.subclass == "storage" && s.built)
    {
        if let Some(item) = s
            .inventory
            .iter()
            .find(|i| i.class == "Log" && i.quantity > 0)
        {
            return Some((s.pos, s.id, item.id));
        }
    }
    None
}

// A Gold Coins stack sitting in an owned storage to withdraw for hiring. Returns
// (storage_pos, storage_id, item_id).
fn storage_gold(view: &WorldView) -> Option<(Position, i32, i32)> {
    for s in view
        .structures
        .iter()
        .filter(|s| s.subclass == "storage" && s.built)
    {
        if let Some(item) = s
            .inventory
            .iter()
            .find(|i| i.class == "Gold Coins" && i.quantity > 0)
        {
            return Some((s.pos, s.id, item.id));
        }
    }
    None
}

fn storage_with_req(view: &WorldView, reqs: &[(&str, i32)]) -> Option<i32> {
    for (req_type, _) in reqs {
        for s in view
            .structures
            .iter()
            .filter(|s| s.subclass == "storage" && s.built)
        {
            if s.inventory.iter().any(|i| i.matches_req(req_type)) {
                return Some(s.id);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless::{HeadlessGame, PreparationComparison, PreparationPairLeg};
    use crate::network::ResponsePacket;
    use crate::obj::{HeroClassProfile, State};

    #[test]
    fn helper_supported_primary_policy_matches_prepared_solo() {
        assert_eq!(
            BalanceBotPolicy::for_scenario(CrisisBalanceScenario::HelperSupported),
            BalanceBotPolicy::for_scenario(CrisisBalanceScenario::PreparedSolo)
        );
        assert_ne!(
            BalanceBotPolicy::for_scenario(CrisisBalanceScenario::HelperSupported),
            BalanceBotPolicy::standard(),
            "helper support is an additive second hero, not extra primary preparation"
        );
    }

    #[test]
    fn active_bot_searches_its_run_owned_shipwreck() {
        let mut game = HeadlessGame::new(1_000);
        let player_id = game.spawn_hero("Warrior", "OwnedShipwreckSearchBot");
        let foreign_player_id = game.spawn_connected_scenario_helper("ForeignShipwreckSearchBot");
        let view = game.observe_for_player(player_id);
        let hero = view.hero.expect("opening hero");
        let shipwreck = view
            .pois
            .iter()
            .find(|poi| poi.template == "Shipwreck" && poi.run_owned)
            .expect("run-associated Shipwreck");
        assert!(Map::is_adjacent_including_source(hero.pos, shipwreck.pos));
        let foreign_shipwreck = view
            .pois
            .iter()
            .find(|poi| poi.template == "Shipwreck" && !poi.run_owned)
            .expect("foreign Shipwreck remains observable but opaque");
        assert!(foreign_shipwreck.inventory.is_empty());
        assert_ne!(foreign_player_id, player_id);

        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::InvestigatePOI {
                player_id: event_player_id,
                target_id,
            }) if event_player_id == player_id && target_id == shipwreck.id
        ));
        assert!(bot.shipwreck_search_issued);
    }

    #[test]
    fn searched_opening_uses_manual_transfer_from_owned_shipwreck() {
        let mut game = HeadlessGame::new(1_000);
        let player_id = game.spawn_hero("Warrior", "OwnedShipwreckTransferBot");
        let view = game.observe_for_player(player_id);
        let hero = view.hero.expect("opening hero");
        let shipwreck = view
            .pois
            .iter()
            .find(|poi| poi.template == "Shipwreck" && poi.run_owned)
            .expect("run-associated Shipwreck");
        let first_salvage_id = shipwreck.inventory.first().expect("starter salvage").id;

        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        bot.shipwreck_search_issued = true;
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::ItemTransfer {
                player_id: event_player_id,
                source_id,
                target_id,
                item_id,
            }) if event_player_id == player_id
                && source_id == shipwreck.id
                && target_id == hero.id
                && item_id == first_salvage_id
        ));
    }

    #[test]
    fn burrow_requirement_prefers_actual_log_over_timber_substitute() {
        let mut game = HeadlessGame::new(1_000);
        let player_id = game.spawn_hero("Warrior", "BurrowLogPreferenceBot");
        let view = game.observe_for_player(player_id);
        let mut salvage = view
            .pois
            .iter()
            .find(|poi| poi.template == "Shipwreck" && poi.run_owned)
            .expect("run-associated Shipwreck")
            .inventory
            .clone();
        assert!(salvage.iter().any(|item| item.class == "Timber"));
        salvage.sort_by_key(|item| i32::from(item.class != "Timber"));

        let selected = preferred_item_for_req(&salvage, "Log").expect("build material");
        assert_eq!(selected.class, "Log");
    }

    #[test]
    fn active_bot_completes_revised_opening_through_production_events() {
        const DECISION_TICKS: u32 = 8;
        const MAX_DECISIONS: usize = 1_500;

        let mut game = HeadlessGame::new((DECISION_TICKS as i32) * (MAX_DECISIONS as i32));
        game.restrict_to_preparation_pair_start_location()
            .expect("fixed production-opening start");
        let player_id = game.spawn_hero("Warrior", "ProductionOpeningBot");
        // Intro timing and post-search grace have dedicated integration tests.
        // Keep their randomized combat out of this production-economy proof;
        // only deadlines move, while every salvage/build action remains real.
        game.defer_intro_encounter_deadlines_for_fixture()
            .expect("deferred unrelated intro encounter deadlines");
        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        let mut investigated_owned_wreck = false;
        let mut manual_salvage_transfers = 0;
        let mut production_gathers = 0;
        let mut max_actual_logs = 0;
        let mut normal_burrow_foundation = false;

        for _ in 0..MAX_DECISIONS {
            let view = game.observe_for_player(player_id);
            max_actual_logs = max_actual_logs.max(actual_log_count(&view.inventory));
            if view.has_built("storage") {
                assert!(investigated_owned_wreck);
                assert!(manual_salvage_transfers > 0);
                assert_eq!(
                    production_gathers, 0,
                    "the five salvaged Logs should build the opening Burrow without gathering"
                );
                assert!(max_actual_logs >= BURROW_REQS[0].1);
                assert!(normal_burrow_foundation);
                assert!(view.inventory.iter().any(|item| item.class == "Timber"));
                assert!(view
                    .pois
                    .iter()
                    .find(|poi| poi.template == "Shipwreck" && poi.run_owned)
                    .is_some_and(|poi| poi.inventory.is_empty()));
                assert!(game
                    .protected_intro_snapshot()
                    .opening_enemy_spawned
                    .iter()
                    .all(|spawned| !spawned));
                return;
            }
            assert!(
                view.hero.is_some_and(|hero| !hero.true_death),
                "opening bot suffered True Death before its Burrow"
            );

            if let Some(event) = bot.step(&view, game.map()) {
                match &event {
                    PlayerEvent::InvestigatePOI { target_id, .. } => {
                        investigated_owned_wreck = view
                            .pois
                            .iter()
                            .any(|poi| poi.id == *target_id && poi.run_owned);
                    }
                    PlayerEvent::ItemTransfer { source_id, .. }
                        if view
                            .pois
                            .iter()
                            .any(|poi| poi.id == *source_id && poi.run_owned) =>
                    {
                        manual_salvage_transfers += 1;
                    }
                    PlayerEvent::Gather { .. } => production_gathers += 1,
                    PlayerEvent::CreateFoundation { structure_name, .. }
                        if structure_name == "Burrow" =>
                    {
                        normal_burrow_foundation = true;
                    }
                    _ => {}
                }
                game.inject(event);
            }
            bot.advance_phase(&view);
            game.tick(DECISION_TICKS);
        }

        let view = game.observe_for_player(player_id);
        panic!(
            "opening bot did not finish a normal Burrow in {MAX_DECISIONS} decisions: tick={}, hero={:?}, wreck_items={}, logs={}, structures={}",
            view.game_tick,
            view.hero.map(|hero| (hero.hp, hero.state, hero.true_death)),
            view.pois
                .iter()
                .find(|poi| poi.template == "Shipwreck" && poi.run_owned)
                .map_or(0, |poi| poi.inventory.len()),
            actual_log_count(&view.inventory),
            view.structures.len(),
        );
    }

    #[test]
    fn helper_support_bot_emits_normal_move_and_attack_events() {
        let mut game = HeadlessGame::new(1_000);
        let owner_player_id = game.spawn_hero("Warrior", "SupportOwnerBot");
        let owner_anchor = game
            .observe_for_player(owner_player_id)
            .home()
            .expect("owner settlement anchor");
        let helper_player_id = game.spawn_connected_scenario_helper("SupportHelperBot");
        let mut view = game.observe_for_player(helper_player_id);
        let hero = view.hero.as_mut().expect("connected helper hero");
        hero.state = State::None;
        hero.hp = hero.base_hp;
        hero.hunger = 0.0;
        hero.thirst = 0.0;
        hero.tired = 0.0;
        let helper_hero = *hero;
        assert!(hex_dist(helper_hero.pos, owner_anchor) > 1);

        // Isolate the policy decision from ambient intro enemies and inventory
        // equipment choices. This mutates only the owned observation snapshot,
        // never the authoritative game world.
        view.enemies.clear();
        view.inventory.clear();
        view.occupied.clear();
        view.occupied.insert((helper_hero.pos.x, helper_hero.pos.y));
        view.occupied.insert((owner_anchor.x, owner_anchor.y));

        let mut bot = Bot::for_helper_support(helper_player_id, owner_player_id, owner_anchor);
        let movement = bot
            .step(&view, game.map())
            .expect("helper should travel toward the owner settlement");
        match movement {
            PlayerEvent::Move { player_id, x, y } => {
                let destination = Position { x, y };
                assert_eq!(player_id, helper_player_id);
                assert_eq!(hex_dist(helper_hero.pos, destination), 1);
                assert!(
                    hex_dist(destination, owner_anchor) < hex_dist(helper_hero.pos, owner_anchor)
                );
            }
            _ => panic!("helper travel must use an ordinary Move event"),
        }

        let enemy_pos = Map::range((helper_hero.pos.x, helper_hero.pos.y), 1)
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .find(|position| {
                *position != helper_hero.pos && Map::is_passable(position.x, position.y, game.map())
            })
            .expect("passable adjacent enemy tile");
        const ENEMY_ID: i32 = 9_999_999;
        view.enemies.push(UnitView {
            id: ENEMY_ID,
            player_id: 900_000,
            pos: enemy_pos,
            crisis_owner_player_id: None,
        });

        let combat = bot
            .step(&view, game.map())
            .expect("helper should fight an adjacent enemy");
        match combat {
            PlayerEvent::Attack {
                player_id,
                source_id,
                target_id,
                ..
            } => {
                assert_eq!(player_id, helper_player_id);
                assert_eq!(source_id, helper_hero.id);
                assert_eq!(target_id, ENEMY_ID);
            }
            _ => panic!("helper combat must use an ordinary Attack event"),
        }
    }

    #[test]
    fn helper_support_filters_and_retains_supported_owner_targets_beyond_ambient_radius() {
        let mut game = HeadlessGame::new(1_000);
        let owner_player_id = game.spawn_hero("Warrior", "SupportTargetOwnerBot");
        let owner_anchor = game
            .observe_for_player(owner_player_id)
            .home()
            .expect("owner settlement anchor");
        let helper_player_id = game.spawn_connected_scenario_helper("SupportTargetHelperBot");
        let mut view = game.observe_for_player(helper_player_id);
        let hero = view.hero.as_mut().expect("connected helper hero");
        hero.state = State::None;
        hero.hp = hero.base_hp;
        hero.hunger = 0.0;
        hero.thirst = 0.0;
        hero.tired = 0.0;
        hero.vision = AGGRO_RADIUS + 1;
        let helper_hero = *hero;
        view.inventory.clear();
        view.enemies.clear();
        view.occupied.clear();
        view.occupied.insert((helper_hero.pos.x, helper_hero.pos.y));
        // The helper's own crisis is deliberately inactive. Explicit support
        // context, not the helper's crisis phase, retains the owner's units.
        view.crisis_phase = Some(CrisisPhase::Dormant);

        let supported_pos = passable_at_distance(helper_hero.pos, AGGRO_RADIUS + 1, game.map());
        let other_owner_pos = passable_at_distance(helper_hero.pos, 1, game.map());
        const SUPPORTED_TARGET_ID: i32 = 9_999_910;
        const OTHER_OWNER_TARGET_ID: i32 = 9_999_911;
        view.enemies.push(owner_assault_enemy(
            OTHER_OWNER_TARGET_ID,
            owner_player_id + 100,
            other_owner_pos,
        ));
        view.enemies.push(owner_assault_enemy(
            SUPPORTED_TARGET_ID,
            owner_player_id,
            supported_pos,
        ));
        view.occupied.insert((supported_pos.x, supported_pos.y));
        view.occupied.insert((other_owner_pos.x, other_owner_pos.y));

        let mut bot = Bot::for_helper_support(helper_player_id, owner_player_id, owner_anchor);
        assert_eq!(
            bot.owned_assault_target(&helper_hero, &view)
                .expect("supported-owner target selection")
                .id,
            SUPPORTED_TARGET_ID,
            "a closer unit from another owner must be filtered out"
        );
        view.enemies
            .retain(|enemy| enemy.id != OTHER_OWNER_TARGET_ID);
        view.occupied
            .remove(&(other_owner_pos.x, other_owner_pos.y));
        let event = bot
            .step(&view, game.map())
            .expect("helper should chase the supported owner's distant unit");
        match event {
            PlayerEvent::Move { player_id, x, y } => {
                let destination = Position { x, y };
                assert_eq!(player_id, helper_player_id);
                assert!(
                    hex_dist(destination, supported_pos) < hex_dist(helper_hero.pos, supported_pos),
                    "support movement must close on the supported owner's target"
                );
            }
            event => panic!("distant supported target must produce a normal Move, got {event:?}"),
        }
        assert_eq!(bot.observed_assault_target_id(), Some(SUPPORTED_TARGET_ID));

        let closer_supported_pos = passable_at_distance(helper_hero.pos, 2, game.map());
        view.enemies.push(owner_assault_enemy(
            9_999_912,
            owner_player_id,
            closer_supported_pos,
        ));
        assert_eq!(
            bot.owned_assault_target(&helper_hero, &view)
                .expect("retained supported-owner target")
                .id,
            SUPPORTED_TARGET_ID,
            "a newly closer unit must not replace the retained live target"
        );
    }

    #[test]
    fn helper_support_attack_keeps_event_and_crisis_ownership_separate() {
        let mut game = HeadlessGame::new(1_000);
        let owner_player_id = game.spawn_hero("Warrior", "SupportOwnershipOwnerBot");
        let owner_anchor = game
            .observe_for_player(owner_player_id)
            .home()
            .expect("owner settlement anchor");
        let helper_player_id = game.spawn_connected_scenario_helper("SupportOwnershipHelperBot");
        let mut view = game.observe_for_player(helper_player_id);
        let hero = view.hero.as_mut().expect("connected helper hero");
        hero.state = State::None;
        hero.hp = hero.base_hp;
        hero.hunger = 0.0;
        hero.thirst = 0.0;
        hero.tired = 0.0;
        let helper_hero = *hero;
        view.inventory.clear();
        view.enemies.clear();
        view.occupied.clear();
        view.occupied.insert((helper_hero.pos.x, helper_hero.pos.y));
        view.crisis_phase = Some(CrisisPhase::Dormant);

        let target_pos = passable_at_distance(helper_hero.pos, 1, game.map());
        const TARGET_ID: i32 = 9_999_913;
        view.enemies
            .push(owner_assault_enemy(TARGET_ID, owner_player_id, target_pos));
        view.occupied.insert((target_pos.x, target_pos.y));

        let mut bot = Bot::for_helper_support(helper_player_id, owner_player_id, owner_anchor);
        match bot
            .step(&view, game.map())
            .expect("adjacent supported target should be attacked")
        {
            PlayerEvent::Attack {
                player_id,
                source_id,
                target_id,
                ..
            } => {
                assert_eq!(player_id, helper_player_id);
                assert_eq!(source_id, helper_hero.id);
                assert_eq!(target_id, TARGET_ID);
            }
            event => panic!("helper must emit an ordinary Attack event, got {event:?}"),
        }
        assert_eq!(
            view.enemies[0].crisis_owner_player_id,
            Some(owner_player_id),
            "bot policy must not mutate crisis ownership"
        );
        assert_eq!(bot.helper_supported_owner_id, Some(owner_player_id));
    }

    fn isolated_assault_view(class_name: &str, hero_name: &str) -> (HeadlessGame, i32, WorldView) {
        let mut game = HeadlessGame::new(1_000);
        let player_id = game.spawn_hero(class_name, hero_name);
        let mut view = game.observe_for_player(player_id);
        let recovered_salvage = view
            .pois
            .iter()
            .find(|poi| poi.template == "Shipwreck" && poi.run_owned)
            .expect("class test run-associated Shipwreck")
            .inventory
            .clone();
        view.inventory.extend(recovered_salvage);
        for item in &mut view.inventory {
            item.equipped = match class_name {
                "Ranger" => item.name == "Training Bow" || item.equipped,
                "Warrior" => {
                    item.name == "Sharpened Stick" || item.name == "Copper Helm" || item.equipped
                }
                "Mage" => item.name == "Sharpened Stick" || item.equipped,
                _ => item.equipped,
            };
            if item.is_weapon {
                item.equipped = match class_name {
                    "Ranger" => item.name == "Training Bow",
                    _ => item.name == "Sharpened Stick",
                };
            }
        }
        let hero = view.hero.as_mut().expect("headless hero");
        hero.state = State::None;
        hero.hp = hero.base_hp;
        hero.hunger = 0.0;
        hero.thirst = 0.0;
        hero.tired = 0.0;
        view.enemies.clear();
        view.occupied.clear();
        view.occupied.insert((hero.pos.x, hero.pos.y));
        view.crisis_phase = Some(CrisisPhase::AssaultActive);
        (game, player_id, view)
    }

    fn passable_at_distance(from: Position, distance: u32, map: &Map) -> Position {
        Map::range((from.x, from.y), distance)
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .find(|position| {
                hex_dist(from, *position) == distance
                    && Map::is_passable(position.x, position.y, map)
            })
            .expect("passable combat-test position")
    }

    fn passable_adjacent_with_disengage(from: Position, view: &WorldView, map: &Map) -> Position {
        Map::range((from.x, from.y), 1)
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .find(|position| {
                hex_dist(from, *position) == 1
                    && Map::is_passable(position.x, position.y, map)
                    && disengage_destination_is_open(from, *position, view, map)
            })
            .expect("passable adjacent target and disengage destination")
    }

    fn passable_adjacent_with_blocked_disengage_and_bow_step(
        from: Position,
        view: &WorldView,
        map: &Map,
    ) -> (Position, Position) {
        Map::range((from.x, from.y), 1)
            .into_iter()
            .map(|(x, y)| Position { x, y })
            .find_map(|enemy| {
                if hex_dist(from, enemy) != 1
                    || !Map::is_passable(enemy.x, enemy.y, map)
                    || !disengage_destination_is_open(from, enemy, view, map)
                {
                    return None;
                }
                let retreat = disengage_destination(from, enemy)?;
                let has_alternate_bow_step = Map::range((from.x, from.y), 1)
                    .into_iter()
                    .map(|(x, y)| Position { x, y })
                    .any(|position| {
                        position != from
                            && position != enemy
                            && position != retreat
                            && Map::is_valid_pos((position.x, position.y))
                            && Map::is_passable(position.x, position.y, map)
                            && !view.occupied.contains(&(position.x, position.y))
                            && hex_dist(position, enemy) > 1
                            && hex_dist(position, enemy) <= 2
                    });
                has_alternate_bow_step.then_some((enemy, retreat))
            })
            .expect("adjacent target with blockable retreat and alternate bow step")
    }

    fn owner_assault_enemy(id: i32, owner_player_id: i32, pos: Position) -> UnitView {
        UnitView {
            id,
            player_id: 1_000,
            pos,
            crisis_owner_player_id: Some(owner_player_id),
        }
    }

    #[test]
    fn warrior_uses_normal_melee_attack_for_adjacent_owner_assault_unit() {
        let (game, player_id, mut view) =
            isolated_assault_view("Warrior", "WarriorCombatPolicyBot");
        let hero = view.hero.expect("hero");
        let enemy_pos = passable_adjacent_with_disengage(hero.pos, &view, game.map());
        const ENEMY_ID: i32 = 9_999_901;
        view.enemies
            .push(owner_assault_enemy(ENEMY_ID, player_id, enemy_pos));
        view.occupied.insert((enemy_pos.x, enemy_pos.y));

        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        match bot.step(&view, game.map()).expect("warrior combat action") {
            PlayerEvent::Attack {
                player_id: event_player_id,
                source_id,
                target_id,
                ..
            } => {
                assert_eq!(event_player_id, player_id);
                assert_eq!(source_id, hero.id);
                assert_eq!(target_id, ENEMY_ID);
            }
            event => panic!("Warrior must use an ordinary adjacent Attack, got {event:?}"),
        }
    }

    #[test]
    fn wounded_warrior_uses_guard_bash_defensively_at_adjacency() {
        let (game, player_id, mut view) = isolated_assault_view("Warrior", "WarriorGuardPolicyBot");
        let hero = view.hero.as_mut().expect("hero");
        hero.hp = hero.base_hp - 1;
        assert!(has_resource(hero.stamina, GUARD_BASH_STAMINA_COST));
        let hero = *hero;
        let enemy_pos = passable_at_distance(hero.pos, 1, game.map());
        const ENEMY_ID: i32 = 9_999_906;
        view.enemies
            .push(owner_assault_enemy(ENEMY_ID, player_id, enemy_pos));
        view.occupied.insert((enemy_pos.x, enemy_pos.y));

        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Ability {
                player_id: event_player_id,
                ref ability_id,
                source_id,
                target_id: Some(ENEMY_ID),
            }) if event_player_id == player_id
                && ability_id == "shield_bash"
                && source_id == hero.id
        ));
    }

    #[test]
    fn ranger_equips_and_uses_training_bow_at_its_projected_range() {
        let (game, player_id, mut view) = isolated_assault_view("Ranger", "RangerCombatPolicyBot");
        let hero = view.hero.expect("hero");
        let enemy_pos = passable_at_distance(hero.pos, 2, game.map());
        const ENEMY_ID: i32 = 9_999_902;
        view.enemies
            .push(owner_assault_enemy(ENEMY_ID, player_id, enemy_pos));
        view.occupied.insert((enemy_pos.x, enemy_pos.y));

        let bow_id = view
            .inventory
            .iter()
            .find(|item| item.name == "Training Bow")
            .map(|item| item.id)
            .expect("Ranger recovered Training Bow");
        for item in &mut view.inventory {
            if item.name == "Training Bow" {
                assert_eq!(item.attack_range, 2);
                item.equipped = false;
            } else if item.is_weapon {
                item.equipped = true;
            }
        }

        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Equip {
                player_id: event_player_id,
                item_id,
                status: true,
                ..
            }) if event_player_id == player_id && item_id == bow_id
        ));

        for item in &mut view.inventory {
            item.equipped = item.name == "Training Bow";
        }
        match bot.step(&view, game.map()).expect("Ranger ranged attack") {
            PlayerEvent::Attack {
                player_id: event_player_id,
                source_id,
                target_id,
                ..
            } => {
                assert_eq!(event_player_id, player_id);
                assert_eq!(source_id, hero.id);
                assert_eq!(target_id, ENEMY_ID);
            }
            event => panic!("Ranger must use the Training Bow's normal Attack, got {event:?}"),
        }
    }

    #[test]
    fn wounded_ranger_repositions_during_cooldown_without_starving_bow_offense() {
        let (game, player_id, mut view) =
            isolated_assault_view("Ranger", "RangerDisengagePolicyBot");
        let hero = view.hero.as_mut().expect("hero");
        hero.hp = hero.base_hp - 1;
        assert!(has_resource(hero.stamina, DISENGAGE_STAMINA_COST));
        let hero = *hero;
        let enemy_pos = passable_adjacent_with_disengage(hero.pos, &view, game.map());
        const ENEMY_ID: i32 = 9_999_907;
        view.enemies
            .push(owner_assault_enemy(ENEMY_ID, player_id, enemy_pos));
        view.occupied.insert((enemy_pos.x, enemy_pos.y));
        let first_command_tick = view.game_tick;

        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Ability {
                player_id: event_player_id,
                ref ability_id,
                source_id,
                target_id: Some(ENEMY_ID),
            }) if event_player_id == player_id
                && ability_id == "disengage"
                && source_id == hero.id
        ));

        // Model the retained target catching the Ranger again before the shared
        // combat cooldown expires. The policy uses an ordinary one-hex Move to
        // restore Training Bow range and does not move the cooldown deadline.
        view.game_tick += 1;
        let reposition = bot
            .step(&view, game.map())
            .expect("wounded adjacent Ranger cooldown reposition");
        assert_eq!(
            bot.last_ranger_cooldown_reposition_tick,
            Some(view.game_tick)
        );
        let repositioned = match reposition {
            PlayerEvent::Move {
                player_id: event_player_id,
                x,
                y,
            } => {
                assert_eq!(event_player_id, player_id);
                Position { x, y }
            }
            event => panic!("cooldown reposition must be an ordinary Move, got {event:?}"),
        };
        assert!(Map::is_passable(repositioned.x, repositioned.y, game.map()));
        assert!(!view.occupied.contains(&(repositioned.x, repositioned.y)));
        assert_eq!(hex_dist(hero.pos, repositioned), 1);
        assert_eq!(hex_dist(repositioned, enemy_pos), 2);
        assert!(hex_dist(repositioned, enemy_pos) <= equipped_training_bow_range(&view));

        view.occupied.remove(&(hero.pos.x, hero.pos.y));
        view.occupied.insert((repositioned.x, repositioned.y));
        view.hero.as_mut().expect("hero").pos = repositioned;
        view.game_tick = first_command_tick + ATTACK_COOLDOWN_TICKS;
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Attack {
                player_id: event_player_id,
                source_id,
                target_id: ENEMY_ID,
                ..
            }) if event_player_id == player_id && source_id == hero.id
        ));

        view.game_tick += ATTACK_COOLDOWN_TICKS;
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Attack {
                player_id: event_player_id,
                source_id,
                target_id: ENEMY_ID,
                ..
            }) if event_player_id == player_id && source_id == hero.id
        ));

        // Once two real bow commands have been issued, a later adjacent contact
        // permits the bounded Disengage again rather than kiting forever.
        view.occupied.remove(&(repositioned.x, repositioned.y));
        view.occupied.insert((hero.pos.x, hero.pos.y));
        view.hero.as_mut().expect("hero").pos = hero.pos;
        view.game_tick += ATTACK_COOLDOWN_TICKS;
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Ability {
                ref ability_id,
                target_id: Some(ENEMY_ID),
                ..
            }) if ability_id == "disengage"
        ));

        const REPLACEMENT_ID: i32 = 9_999_908;
        view.enemies.clear();
        view.enemies
            .push(owner_assault_enemy(REPLACEMENT_ID, player_id, enemy_pos));
        view.game_tick += ATTACK_COOLDOWN_TICKS;
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Ability {
                ref ability_id,
                target_id: Some(REPLACEMENT_ID),
                ..
            }) if ability_id == "disengage"
        ));
    }

    #[test]
    fn occupied_disengage_destination_skips_ability_and_preserves_ranger_actions() {
        let (game, player_id, mut view) =
            isolated_assault_view("Ranger", "RangerOccupiedDisengagePolicyBot");
        let hero = view.hero.as_mut().expect("hero");
        hero.hp = hero.base_hp - 1;
        assert!(has_resource(hero.stamina, DISENGAGE_STAMINA_COST));
        let hero = *hero;
        let (enemy_pos, retreat_pos) =
            passable_adjacent_with_blocked_disengage_and_bow_step(hero.pos, &view, game.map());
        const ENEMY_ID: i32 = 9_999_909;
        view.enemies
            .push(owner_assault_enemy(ENEMY_ID, player_id, enemy_pos));
        view.occupied.insert((enemy_pos.x, enemy_pos.y));
        view.occupied.insert((retreat_pos.x, retreat_pos.y));
        for item in &mut view.inventory {
            if item.is_weapon {
                item.equipped = item.name == "Training Bow";
            }
        }

        let first_command_tick = view.game_tick;
        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Attack {
                player_id: event_player_id,
                source_id,
                target_id: ENEMY_ID,
                ..
            }) if event_player_id == player_id && source_id == hero.id
        ));
        assert_eq!(
            bot.last_tactical_combat_command_tick,
            Some(first_command_tick)
        );
        assert_eq!(bot.ranger_disengaged_target_id, None);
        assert_eq!(bot.ranger_offensive_commands_since_disengage, 1);

        view.game_tick += 1;
        let reposition = bot
            .step(&view, game.map())
            .expect("Ranger should keep using a valid cooldown reposition");
        let repositioned = match reposition {
            PlayerEvent::Move {
                player_id: event_player_id,
                x,
                y,
            } => {
                assert_eq!(event_player_id, player_id);
                Position { x, y }
            }
            event => panic!(
                "occupied retreat must skip Disengage and preserve ordinary movement, got {event:?}"
            ),
        };
        assert_ne!(repositioned, retreat_pos);
        assert!(!view.occupied.contains(&(repositioned.x, repositioned.y)));
        assert_eq!(hex_dist(hero.pos, repositioned), 1);
        assert_eq!(hex_dist(repositioned, enemy_pos), 2);
        assert_eq!(
            bot.last_tactical_combat_command_tick,
            Some(first_command_tick),
            "ordinary movement must not manufacture a new combat cooldown"
        );
        assert_eq!(
            bot.last_ranger_cooldown_reposition_tick,
            Some(view.game_tick)
        );
    }

    #[test]
    fn mage_uses_arcane_bolt_with_mana_at_range_three() {
        let (game, player_id, mut view) = isolated_assault_view("Mage", "MageCombatPolicyBot");
        let hero = view.hero.expect("hero");
        assert!(has_resource(hero.mana, ARCANE_BOLT_MANA_COST));
        let enemy_pos = passable_at_distance(hero.pos, 3, game.map());
        const ENEMY_ID: i32 = 9_999_903;
        view.enemies
            .push(owner_assault_enemy(ENEMY_ID, player_id, enemy_pos));
        view.occupied.insert((enemy_pos.x, enemy_pos.y));

        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        match bot.step(&view, game.map()).expect("Mage ranged ability") {
            PlayerEvent::Ability {
                player_id: event_player_id,
                ability_id,
                source_id,
                target_id,
            } => {
                assert_eq!(event_player_id, player_id);
                assert_eq!(ability_id, "arcane_bolt");
                assert_eq!(source_id, hero.id);
                assert_eq!(target_id, Some(ENEMY_ID));
            }
            event => panic!("Mage must use Arcane Bolt at range three, got {event:?}"),
        }
    }

    #[test]
    fn wounded_mage_interleaves_two_arcane_bolts_between_wards() {
        let (game, player_id, mut view) = isolated_assault_view("Mage", "MageWardCadencePolicyBot");
        let hero = view.hero.as_mut().expect("hero");
        hero.hp = hero.base_hp - 1;
        assert!(has_resource(hero.mana, WARD_MANA_COST));
        let hero = *hero;
        let enemy_pos = passable_at_distance(hero.pos, 1, game.map());
        const ENEMY_ID: i32 = 9_999_909;
        view.enemies
            .push(owner_assault_enemy(ENEMY_ID, player_id, enemy_pos));
        view.occupied.insert((enemy_pos.x, enemy_pos.y));

        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Ability {
                player_id: event_player_id,
                ref ability_id,
                source_id,
                target_id: None,
            }) if event_player_id == player_id
                && ability_id == "ward"
                && source_id == hero.id
        ));
        assert!(
            bot.step(&view, game.map()).is_none(),
            "Ward must not be followed by cooldown-starving command spam"
        );

        for _ in 0..OFFENSIVE_COMMANDS_PER_DEFENSIVE_ABILITY {
            view.game_tick += ATTACK_COOLDOWN_TICKS;
            assert!(matches!(
                bot.step(&view, game.map()),
                Some(PlayerEvent::Ability {
                    player_id: event_player_id,
                    ref ability_id,
                    source_id,
                    target_id: Some(ENEMY_ID),
                }) if event_player_id == player_id
                    && ability_id == "arcane_bolt"
                    && source_id == hero.id
            ));
        }

        view.game_tick += ATTACK_COOLDOWN_TICKS;
        assert!(matches!(
            bot.step(&view, game.map()),
            Some(PlayerEvent::Ability {
                ref ability_id,
                target_id: None,
                ..
            }) if ability_id == "ward"
        ));
    }

    #[derive(Debug, Default)]
    struct IntegratedClassCombatEvidence {
        target_acquired: bool,
        normal_move_event: bool,
        normal_attack_event: bool,
        normal_ability_event: bool,
        normal_equip_event: bool,
        ranger_bow_event: bool,
        ranger_cooldown_reposition_event: bool,
        mage_ward_event: bool,
        mage_ward_accepted: bool,
        accepted_attacks: i32,
        damage_dealt: i32,
    }

    fn assert_legitimate_class_setup(class: HeroClass, view: &WorldView) {
        let hero = view.hero.expect("class hero at assault launch");
        assert_eq!(hero.hero_class, class);
        let profile = HeroClassProfile::for_class(class);
        assert_eq!(profile.hero_class, class);
        assert!(
            !profile.ability_ids.is_empty(),
            "{class:?} must use the existing class ability catalogue"
        );
        match class {
            HeroClass::Warrior => assert!(view.inventory.iter().any(|item| {
                item.name == "Sharpened Stick"
                    && item.is_weapon
                    && item.equipped
                    && item.attack_range == 1
            })),
            HeroClass::Ranger => assert!(view.inventory.iter().any(|item| {
                item.name == "Training Bow"
                    && item.is_weapon
                    && item.equipped
                    && item.attack_range == 2
            })),
            HeroClass::Mage => {
                assert!(view.inventory.iter().any(|item| {
                    item.name == "Sharpened Stick"
                        && item.is_weapon
                        && item.equipped
                        && item.attack_range == 1
                }));
                assert!(has_resource(hero.mana, ARCANE_BOLT_MANA_COST));
                assert!(profile.ability_ids.contains(&"arcane_bolt"));
                assert!(profile.ability_ids.contains(&"ward"));
            }
        }
    }

    fn complete_production_opening_for_class_combat(
        game: &mut HeadlessGame,
        player_id: i32,
        bot: &mut Bot,
    ) -> bool {
        const DECISION_TICKS: u32 = 8;
        const MAX_DECISIONS: usize = 1_500;

        for _ in 0..MAX_DECISIONS {
            let view = game.observe_for_player(player_id);
            if view.has_built("storage") {
                assert!(
                    view.pois
                        .iter()
                        .find(|poi| poi.template == "Shipwreck" && poi.run_owned)
                        .is_some_and(|poi| poi.inventory.is_empty()),
                    "production opening must manually recover the run-owned Shipwreck before the Burrow"
                );
                assert!(
                    game.protected_intro_snapshot()
                        .opening_enemy_spawned
                        .iter()
                        .all(|spawned| !spawned),
                    "class-combat fixture setup must finish during the production post-salvage warning grace"
                );
                bot.advance_phase(&view);
                return true;
            }
            if view.hero.is_none_or(|hero| hero.true_death) {
                return false;
            }

            if let Some(event) = bot.step(&view, game.map()) {
                game.inject(event);
            }
            bot.advance_phase(&view);
            game.tick(DECISION_TICKS);
        }

        false
    }

    fn equip_recovered_class_weapon_for_combat(
        class: HeroClass,
        game: &mut HeadlessGame,
        bot: &Bot,
    ) -> bool {
        if class != HeroClass::Ranger {
            return true;
        }

        let view = game.observe();
        if view
            .inventory
            .iter()
            .any(|item| item.name == "Training Bow" && item.equipped)
        {
            return true;
        }
        let Some(event) = bot.equip_training_bow(&view) else {
            return false;
        };
        assert!(matches!(
            event,
            PlayerEvent::Equip {
                player_id,
                status: true,
                ..
            } if player_id == bot.player_id
        ));
        game.inject(event);
        game.tick(2);

        game.observe()
            .inventory
            .iter()
            .any(|item| item.name == "Training Bow" && item.equipped)
    }

    fn integrated_class_combat_evidence(class: HeroClass) -> IntegratedClassCombatEvidence {
        const DECISION_TICKS: u32 = 8;
        const OBSERVATION_TICKS: i32 = 2_000;
        const MAX_ATTEMPTS: usize = 3;

        let mut last_evidence = IntegratedClassCombatEvidence::default();
        for attempt in 0..MAX_ATTEMPTS {
            let class_name = class.to_str();
            let mut game = HeadlessGame::new(20_000);
            game.restrict_to_preparation_pair_start_location()
                .expect("fixed class-combat start");
            let player_id =
                game.spawn_hero(class_name, &format!("Cp4Integrated{class_name}{attempt}"));
            // Intro combat has dedicated production coverage. Keep its random
            // attackers out of this class-assault regression while retaining
            // the complete authoritative Shipwreck -> salvage -> equipment ->
            // Burrow path that establishes legitimate class inventory.
            game.defer_intro_encounter_deadlines_for_fixture()
                .expect("deferred unrelated intro encounter deadlines");
            let mut bot =
                Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
            if !complete_production_opening_for_class_combat(&mut game, player_id, &mut bot)
                || !equip_recovered_class_weapon_for_combat(class, &mut game, &bot)
            {
                continue;
            }
            game.set_crisis_balance_sample_interval(Some(1));
            let launch = game
                .prepare_checkpoint4_preparation_pair_launch(
                    PreparationComparison::EquipmentPrepared,
                    PreparationPairLeg::Control,
                )
                .expect("production active-assault fixture");
            assert_eq!(
                launch.common_fingerprint.hero_class, class_name,
                "fixture must preserve the selected production hero class"
            );
            assert!(!launch.common_fingerprint.assault_units.is_empty());

            let launch_view = game.observe();
            assert_legitimate_class_setup(class, &launch_view);
            let launch_tick = game.game_tick();
            let mut evidence = IntegratedClassCombatEvidence::default();
            game.start_packet_capture();

            while game.game_tick().saturating_sub(launch_tick) < OBSERVATION_TICKS {
                let view = game.observe();
                if view.hero.is_none_or(|hero| hero.true_death)
                    || game.settlement_crisis().map(|crisis| crisis.phase)
                        == Some(CrisisPhase::Resolved)
                {
                    break;
                }

                let event = bot.step(&view, game.map());
                if let Some(target_id) = bot.observed_assault_target_id() {
                    assert!(
                        game.record_observed_crisis_target(target_id),
                        "bot target must belong to the live owner assault"
                    );
                    evidence.target_acquired = true;
                }
                if let Some(event) = event {
                    match &event {
                        PlayerEvent::Move {
                            player_id: event_player_id,
                            x,
                            y,
                        } => {
                            assert_eq!(*event_player_id, player_id);
                            evidence.normal_move_event = true;
                            if class == HeroClass::Ranger
                                && bot.last_ranger_cooldown_reposition_tick == Some(view.game_tick)
                            {
                                let hero = view.hero.expect("Ranger reposition source");
                                let target_id = bot
                                    .observed_assault_target_id()
                                    .expect("retained Ranger assault target");
                                let target = view
                                    .enemies
                                    .iter()
                                    .find(|enemy| enemy.id == target_id)
                                    .expect("retained Ranger target in view");
                                let destination = Position { x: *x, y: *y };
                                let bow_range = equipped_training_bow_range(&view);
                                assert!(hero.hp < hero.base_hp);
                                assert_eq!(hex_dist(hero.pos, target.pos), 1);
                                assert_eq!(hex_dist(hero.pos, destination), 1);
                                assert!(Map::is_passable(destination.x, destination.y, game.map()));
                                assert!(!view.occupied.contains(&(destination.x, destination.y)));
                                assert!(hex_dist(destination, target.pos) > 1);
                                assert!(hex_dist(destination, target.pos) <= bow_range);
                                evidence.ranger_cooldown_reposition_event = true;
                            }
                        }
                        PlayerEvent::Attack {
                            player_id: event_player_id,
                            source_id,
                            target_id,
                            ..
                        } => {
                            assert_eq!(*event_player_id, player_id);
                            assert_eq!(Some(*source_id), view.hero.map(|hero| hero.id));
                            assert!(view.enemies.iter().any(|enemy| {
                                enemy.id == *target_id
                                    && enemy.crisis_owner_player_id == Some(player_id)
                            }));
                            assert!(
                                view.inventory
                                    .iter()
                                    .any(|item| item.equipped && item.is_weapon),
                                "normal Attack must use existing equipped production gear"
                            );
                            evidence.normal_attack_event = true;
                            if class == HeroClass::Ranger {
                                evidence.ranger_bow_event = true;
                            }
                        }
                        PlayerEvent::Ability {
                            player_id: event_player_id,
                            ability_id,
                            source_id,
                            target_id,
                        } => {
                            assert_eq!(*event_player_id, player_id);
                            assert_eq!(Some(*source_id), view.hero.map(|hero| hero.id));
                            if ability_id == "ward" {
                                assert_eq!(*target_id, None, "Ward is production-untargeted");
                                evidence.mage_ward_event = true;
                            } else {
                                assert!(target_id.is_some_and(|target_id| {
                                    view.enemies.iter().any(|enemy| {
                                        enemy.id == target_id
                                            && enemy.crisis_owner_player_id == Some(player_id)
                                    })
                                }));
                            }
                            assert!(
                                HeroClassProfile::for_class(class)
                                    .ability_ids
                                    .contains(&ability_id.as_str()),
                                "bot emitted an ability outside the existing class catalogue"
                            );
                            evidence.normal_ability_event = true;
                            if class == HeroClass::Ranger && ability_id == "aimed_shot" {
                                evidence.ranger_bow_event = true;
                            }
                        }
                        PlayerEvent::Equip {
                            player_id: event_player_id,
                            item_id,
                            status,
                            ..
                        } => {
                            assert_eq!(*event_player_id, player_id);
                            assert!(*status);
                            assert!(view.inventory.iter().any(|item| item.id == *item_id));
                            evidence.normal_equip_event = true;
                        }
                        _ => {}
                    }
                    // `inject` is the same production PlayerEvent ingress used by
                    // every headless runner. The server, not the bot, validates
                    // range/cooldown/resources and applies any resulting damage.
                    game.inject(event);
                }
                bot.advance_phase(&view);
                game.tick(DECISION_TICKS);

                evidence.mage_ward_accepted |=
                    game.finish_packet_capture().into_iter().any(|packet| {
                        matches!(
                            packet,
                            ResponsePacket::Ability { ref ability_id, .. }
                                if ability_id == "ward"
                        )
                    });
                game.start_packet_capture();

                let engagement = game.crisis_balance_telemetry().engagement;
                evidence.accepted_attacks = engagement.hero_attacks_accepted;
                evidence.damage_dealt = engagement.hero_damage_dealt_to_assault;
                if evidence.target_acquired
                    && evidence.accepted_attacks > 0
                    && evidence.damage_dealt > 0
                    && (class != HeroClass::Ranger || evidence.ranger_cooldown_reposition_event)
                    && (class != HeroClass::Mage || evidence.mage_ward_accepted)
                {
                    assert!(engagement.first_hero_target_acquired_tick.is_some());
                    assert!(engagement.first_hero_attack_requested_tick.is_some());
                    assert!(engagement.first_hero_attack_accepted_tick.is_some());
                    assert!(engagement.first_damage_to_attacker_tick.is_some());
                    assert!(
                        evidence.normal_attack_event || evidence.normal_ability_event,
                        "damage must follow an ordinary production combat PlayerEvent"
                    );
                    return evidence;
                }
            }
            last_evidence = evidence;
        }

        panic!(
            "{class:?} did not acquire and damage a production assault attacker in {MAX_ATTEMPTS} bounded attempts; last evidence={last_evidence:?}"
        );
    }

    #[test]
    fn checkpoint4_warrior_bot_acquires_and_damages_through_production_events() {
        let evidence = integrated_class_combat_evidence(HeroClass::Warrior);
        assert!(evidence.target_acquired);
        assert!(evidence.accepted_attacks > 0);
        assert!(evidence.damage_dealt > 0);
    }

    #[test]
    fn checkpoint4_ranger_bot_acquires_and_damages_through_production_events() {
        let evidence = integrated_class_combat_evidence(HeroClass::Ranger);
        assert!(evidence.target_acquired);
        assert!(
            evidence.ranger_bow_event,
            "Ranger damage must follow a Training Bow Attack or Aimed Shot event"
        );
        assert!(evidence.ranger_cooldown_reposition_event);
        assert!(evidence.accepted_attacks > 0);
        assert!(evidence.damage_dealt > 0);
    }

    #[test]
    fn checkpoint4_mage_bot_acquires_and_damages_through_production_events() {
        let evidence = integrated_class_combat_evidence(HeroClass::Mage);
        assert!(evidence.target_acquired);
        assert!(evidence.normal_ability_event);
        assert!(evidence.mage_ward_event);
        assert!(evidence.mage_ward_accepted);
        assert!(evidence.accepted_attacks > 0);
        assert!(evidence.damage_dealt > 0);
    }

    #[test]
    fn active_assault_target_is_retained_and_chased_beyond_old_aggro_radius() {
        let (game, player_id, mut view) =
            isolated_assault_view("Warrior", "AssaultRetentionPolicyBot");
        let mut hero = view.hero.expect("hero");
        hero.vision = AGGRO_RADIUS + 1;
        view.hero = Some(hero);
        let retained_pos = passable_at_distance(hero.pos, AGGRO_RADIUS + 1, game.map());
        const RETAINED_ID: i32 = 9_999_904;
        view.enemies
            .push(owner_assault_enemy(RETAINED_ID, player_id, retained_pos));
        view.occupied.insert((retained_pos.x, retained_pos.y));

        let mut bot = Bot::for_balance_scenario(player_id, CrisisBalanceScenario::BasicSurvival);
        let first = bot
            .step(&view, game.map())
            .expect("active-assault chase action");
        let first_destination = match first {
            PlayerEvent::Move { x, y, .. } => Position { x, y },
            event => panic!("distant assault unit must be chased with Move, got {event:?}"),
        };
        assert!(hex_dist(first_destination, retained_pos) < hex_dist(hero.pos, retained_pos));

        let decoy_pos = passable_at_distance(hero.pos, 1, game.map());
        view.enemies
            .push(owner_assault_enemy(9_999_905, player_id, decoy_pos));
        view.occupied.insert((decoy_pos.x, decoy_pos.y));
        assert_eq!(
            bot.owned_assault_target(&hero, &view)
                .expect("retained owner-assault target")
                .id,
            RETAINED_ID,
            "the closer decoy must not replace the retained assault target"
        );
    }
}

// A hero-held item that satisfies a requirement the foundation still needs.
fn hero_item_for_missing(
    inventory: &[ItemView],
    foundation: &StructureView,
    reqs: &[(String, i32)],
) -> Option<i32> {
    for (req_type, need) in reqs {
        if count_matching(&foundation.inventory, req_type) >= *need {
            continue;
        }
        if let Some(item) = preferred_item_for_req(inventory, req_type) {
            return Some(item.id);
        }
    }
    None
}
