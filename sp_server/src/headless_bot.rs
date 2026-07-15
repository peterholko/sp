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
//   5. Build — drive the current build job (campfire first, then walls), pulling
//      resources from the Burrow and depositing them into the foundation.
//   6. Fortify — once the campfire stands, ring the base with palisade walls.
//   7. Economy — order idle villagers to gather; forage resource tiles.
//   8. Explore — range out to deterministic waypoints when nothing else to do.
//
// Survival model (from the game): there is NO passive HP regen, so survival is
// about avoiding damage (retreat + walls + heal items); hunger/thirst/tiredness
// are auto-managed by the game when the hero is idle. Movement is single-hex-step
// because the server's MoveEvent only accepts a destination adjacent to the mover.

use crate::constants::{WATERSKIN_EMPTY, WATERSKIN_FILLED};
use crate::crisis_balance::CrisisBalanceScenario;
use crate::game::{
    sanctuary_upgrade_cost, sanctuary_weak_radius, CrisisPhase, SANCTUARY_MAX_LEVEL,
};
use crate::headless::{HeroView, ItemView, StructureView, UnitView, WorldView};
use crate::map::{Map, TileType};
use crate::obj::Position;
use crate::PlayerEvent;

// Engage enemies within this range. A lingering enemy keeps re-applying the
// combat lock, which blocks ALL eating/drinking/sleeping — so the hero must
// clear nearby harassers rather than passively ignore them, or it starves while
// standing idle holding food.
const AGGRO_RADIUS: u32 = 3;
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
// obj_template.yaml `req` fields. Campfire uses the hero's starting Stick+Resin;
// Stockade walls use Logs.
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
    structure_name: String, // "Campfire" / "Stockade"
    subclass: String,       // expected subclass: "campfire" / "wall"
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
    recruit_attempted: bool, // investigated the shipwreck to recruit a villager
    upgrade_enabled: bool,   // loot Soulshards + empower the sanctuary (BOT_NO_UPGRADE to disable)
    dbg_last_day: i32,       // last day a FOOD_DEBUG line was emitted
    hunts: u32,              // hunt actions issued (diagnostic)
    balance_policy: BalanceBotPolicy,
    // Dedicated multiplayer helper destination. When set, the hero travels by
    // ordinary Move events until adjacent to the owner's settlement, then holds
    // there and fights nearby enemies through the normal combat event path.
    helper_support_anchor: Option<Position>,
}

impl Bot {
    pub fn new(player_id: i32) -> Self {
        Self::new_with_policy(player_id, BalanceBotPolicy::standard())
    }

    pub fn for_balance_scenario(player_id: i32, scenario: CrisisBalanceScenario) -> Self {
        Self::new_with_policy(player_id, BalanceBotPolicy::for_scenario(scenario))
    }

    pub fn for_helper_support(player_id: i32, owner_settlement_anchor: Position) -> Self {
        let mut bot = Self::new_with_policy(player_id, BalanceBotPolicy::supporting_helper());
        bot.helper_support_anchor = Some(owner_settlement_anchor);
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
            recruit_attempted: false,
            // A/B toggle for measuring the sanctuary loop's contribution.
            upgrade_enabled: balance_policy.upgrade_sanctuary
                && std::env::var("BOT_NO_UPGRADE").is_err(),
            dbg_last_day: -1,
            hunts: 0,
            balance_policy,
            helper_support_anchor: None,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
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
        self.phase = if !view.has_built("campfire") {
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
        let nearest = nearest_enemy(hero.pos, view);
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
        if !threat {
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

        // 5. Fight only near enemies while healthy (don't chase — chasing denies
        //    the rest/eat windows the hero needs to survive).
        if let Some(enemy) = nearest {
            let d = hex_dist(hero.pos, enemy.pos);
            if d <= AGGRO_RADIUS && hero.hp_frac() >= LOW_HP {
                // Make sure the strong combat weapon (axe) is equipped — hunting
                // swaps in the weak Hunting spear, so swap back before a fight.
                if let Some(eq) = self.equip_combat_weapon(view) {
                    return Some(eq);
                }
                if d <= 1 {
                    return Some(PlayerEvent::Attack {
                        player_id: self.player_id,
                        attack_type: "quick".to_string(),
                        source_id: hero.id,
                        target_id: enemy.id,
                    });
                }
                if let Some(mv) = self.step_toward(hero.pos, enemy.pos, view, map) {
                    return Some(mv);
                }
            }
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

        // Needs gating for the expansion steps below. Recruiting the shipwreck
        // villager is a cheap one-time trip right by spawn and that villager then
        // farms food into the larder, so it gets a LOOSE gate (just "not about to
        // need something"). Hiring is a longer trip to the merchant, so it stays
        // moderately gated.
        let needs_ok =
            hero.hunger < CONSUME_AT && hero.thirst < CONSUME_AT && hero.tired < CONSUME_AT;
        let needs_comfortable = hero.hunger < 55.0 && hero.thirst < 55.0 && hero.tired < 55.0;

        // 4c. Recruit the shipwreck villager EARLY — before building. It's the
        //     settlement's first farmer (and one-time), so getting it on day 1 is
        //     worth more than rushing the campfire. The wreck sits next to spawn, so
        //     a loose safe + not-critically-needy gate makes this fire reliably.
        if self.balance_policy.passive {
            return None;
        }

        if self.balance_policy.recruit_shipwreck_villager
            && safe
            && !crisis_hold
            && needs_ok
            && !self.recruit_attempted
            && view.villagers.is_empty()
        {
            if let Some(ship) = view.pois.iter().find(|p| p.template == "Shipwreck") {
                if hex_dist(hero.pos, ship.pos) <= 1 {
                    self.recruit_attempted = true;
                    return Some(PlayerEvent::InvestigatePOI {
                        player_id: self.player_id,
                        target_id: ship.id,
                    });
                }
                if let Some(mv) = self.step_adjacent_to(hero.pos, ship.pos, view, map) {
                    return Some(mv);
                }
            }
        }

        // 5. Build the base. The campfire gates the cook-and-stockpile economy, so
        //    it goes up early (the hero starts with the Stick+Resin) — just after
        //    securing the first villager.
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
        //     The hero pays in Gold Coins, which start in the Burrow, so it first
        //     withdraws gold, then walks to the docked merchant and hires.
        if self.balance_policy.hire_villagers
            && safe
            && !crisis_hold
            && needs_comfortable
            && view.villagers.len() < TARGET_VILLAGERS
            && self.recruit_attempted
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

    // Pick the next structure to build: a campfire if none exists yet, then a
    // ring of palisade walls around home. Returns None when nothing to build.
    fn next_build_job(&mut self, view: &WorldView, map: &Map) -> Option<BuildJob> {
        let hero = view.hero?;
        let home = view.home().or(self.anchor).unwrap_or(hero.pos);

        // Campfire first (also a survival objective). Skip if one already exists.
        if self.balance_policy.build_campfire
            && !view.structures.iter().any(|s| s.subclass == "campfire")
        {
            let site = self.anchor.unwrap_or(hero.pos);
            // Only start if we can actually supply it (hero carries Stick+Resin).
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

        if !foundation_filled && !has_all_reqs(&view.inventory, &as_req_slice(&job.reqs)) {
            // Need more resources in hand — fetch from a storage (the Burrow).
            if let Some((storage_pos, storage_id, item_id)) =
                storage_item_for_missing(view, &job.reqs)
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
    // the hero's pack; the starting gold sits in the Burrow, so the hero withdraws a
    // gold stack first, then walks to the merchant and hires. Returns None when no
    // merchant is docked, nothing is for hire, or there is no gold to be had.
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
        //    COOK FIRST: the campfire carries its own firewood stock (it starts with
        //    10), so cooking usually needs no hero-side fuel at all — only fall back
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
    // is in hand, otherwise withdraw a Log from the Burrow first. Returns the action
    // to take, or None if firewood is fine / no logs are anywhere. This is why the
    // hero stops starving: the Burrow's logs become a long firewood supply instead
    // of the cook stalling once the 10 starting Firewood run out.
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

    // Hunt a Game Animal: equip the Hunting weapon (the starting Sharpened Stick),
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

fn has_all_reqs(items: &[ItemView], reqs: &[(&str, i32)]) -> bool {
    reqs.iter().all(|(t, q)| count_matching(items, t) >= *q)
}

// Find a still-missing requirement that some owned storage holds; return
// (storage_pos, storage_id, item_id) to pull one matching stack to the hero.
fn storage_item_for_missing(
    view: &WorldView,
    reqs: &[(String, i32)],
) -> Option<(Position, i32, i32)> {
    for (req_type, need) in reqs {
        if count_matching(&view.inventory, req_type) >= *need {
            continue;
        }
        for s in view
            .structures
            .iter()
            .filter(|s| s.subclass == "storage" && s.built)
        {
            if let Some(item) = s.inventory.iter().find(|i| i.matches_req(req_type)) {
                return Some((s.pos, s.id, item.id));
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

// A Gold Coins stack sitting in an owned storage (the Burrow starts with 50) to
// withdraw for hiring. Returns (storage_pos, storage_id, item_id).
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
    use crate::headless::HeadlessGame;
    use crate::obj::State;

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

        let mut bot = Bot::for_helper_support(helper_player_id, owner_anchor);
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
        if let Some(item) = inventory.iter().find(|i| i.matches_req(req_type)) {
            return Some(item.id);
        }
    }
    None
}
