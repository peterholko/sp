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
use crate::game::{sanctuary_upgrade_cost, sanctuary_weak_radius, SANCTUARY_MAX_LEVEL};
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
// Above this tiredness, surviving (resting) overrides fighting.
const CRITICAL_TIRED: f32 = 85.0;
// Above this hunger, eat raw meat now instead of taking time to cook it.
const CRITICAL_HUNGER: f32 = 85.0;
// Feed value that counts as "real" food worth relying on. Foraged berries (~6) and
// mushrooms (~8) fall below it, so the hero hunts + cooks for proper meals.
const GOOD_FEED: f32 = 40.0;
const MAX_WALLS: usize = 6; // cap palisade walls ringed around the base
const WALL_RING: u32 = 2; // ring radius (leaves the inner tiles free to move)
const JOB_WATCHDOG_TICKS: i32 = 2400; // abandon a stuck build job after ~1 day
// Villager economy: hire from the travelling merchant up to the Prosperity goal.
const HIRE_WAGE: i32 = 25; // Gold Coins charged per hire (matches the server)
const TARGET_VILLAGERS: usize = 3; // Prosperity victory wants 3 villagers

// Structure recipes the bot builds (req type -> quantity), matching the
// obj_template.yaml `req` fields. Campfire uses the hero's starting Stick+Resin;
// Stockade walls use Sticks (foraged abundantly), so a refuge is affordable.
const CAMPFIRE_REQS: &[(&str, i32)] = &[("Stick", 1), ("Resin", 1)];
const STOCKADE_REQS: &[(&str, i32)] = &[("Stick", 3)];
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

// A multi-step structure build the bot is currently driving.
struct BuildJob {
    structure_name: String, // "Campfire" / "Stockade"
    subclass: String,       // expected subclass: "campfire" / "wall"
    reqs: Vec<(String, i32)>,
    site: Position,
    structure_id: Option<i32>, // discovered once the foundation is placed
    build_issued: bool,
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
}

impl Bot {
    pub fn new(player_id: i32) -> Self {
        Bot {
            player_id,
            phase: Phase::Bootstrap,
            anchor: None,
            explore_cursor: 0,
            job: None,
            walls_attempted: 0,
            recruit_attempted: false,
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
            let _ = (idle, with_order, gathering, carried, sfood, plant_nodes, mpos, hdist);
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
        } else if view.structures.iter().filter(|s| s.subclass == "wall").count() < MAX_WALLS {
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

        // 2c. Once meat is in hand, see the butcher->cook through: raw meat can't be
        //     safely eaten (food poisoning) and a finished batch of Cooked Meat
        //     (Feed 100) is many days of food, so completing the cook outranks
        //     routine chores. Yield only to a close threat.
        if !threat {
            let has_meat = view
                .inventory
                .iter()
                .any(|i| i.class == "Game Animal" || i.subclass == "Raw Meat");
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

        // 6. Routine needs while safe: refill water, sleep, idle to auto-eat/drink.
        if safe {
            // Water: prospect a spring + refill so dehydration never sets in.
            if let Some(action) = self.water_action(&hero, view, map) {
                return Some(action);
            }
            // Sleep BEFORE foraging: sleep is a short (30-tick) action, while a
            // forage is ~150 ticks — letting a forage pre-empt sleep is how the
            // hero ends up dying of exhaustion mid-gather.
            if hero.tired >= CONSUME_AT {
                return Some(PlayerEvent::Sleep {
                    player_id: self.player_id,
                    structure_id: 0, // ignored by the handler
                });
            }
            // Food pipeline: butcher carcasses -> cook raw meat -> hunt/forage for
            // more. See food_action.
            if let Some(action) = self.food_action(&hero, view, map) {
                return Some(action);
            }
            // Eat / drink EXPLICITLY via Use rather than waiting for the game's
            // auto-consume — auto-consume is blocked by several transient states,
            // so an idle hero can sit on food at hunger 100 and starve.
            if let Some(action) = self.consume_action(&hero, view) {
                return Some(action);
            }
        }

        // 4d. Recruit the first villager (one-time): investigate the Shipwreck POI,
        //     which sets scavenge_shipwreck so the castaway villager arrives ~day 1.
        //     Only when safe, no villager yet, AND all needs have comfortable buffer
        //     — the walk to the wreck must never come at the cost of eat/drink/sleep.
        let needs_comfortable =
            hero.hunger < 45.0 && hero.thirst < 45.0 && hero.tired < 45.0;
        if safe && needs_comfortable && !self.recruit_attempted && view.villagers.is_empty() {
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

        // 4e. Hire more villagers from the travelling merchant, up to the
        //     Prosperity goal. Only when safe + needs comfortable (same as recruit).
        //     The hero pays in Gold Coins, which start in the Burrow, so it first
        //     withdraws gold, then walks to the docked merchant and hires.
        if safe
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
        if safe {
            if let Some(action) = self.loot_soulshards(&hero, view, map) {
                return Some(action);
            }
        }

        // 4g. Empower the Monolith sanctuary when we can afford the next level. This
        //     shrinks random spawns around the base — the primary early-game survival
        //     investment. Only when safe and needs have buffer (it's a short trip to
        //     the nearby Monolith).
        if safe && needs_comfortable {
            if let Some(action) = self.upgrade_sanctuary_action(&hero, view, map) {
                return Some(action);
            }
        }

        // 5. Drive the in-progress build job.
        if self.job.is_some() {
            if let Some(action) = self.advance_job(view, map) {
                return Some(action);
            }
        }

        // 6. Start the next build job (campfire, then walls).
        if let Some(job) = self.next_build_job(view, map) {
            self.job = Some(job);
            if let Some(action) = self.advance_job(view, map) {
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
        if !view.structures.iter().any(|s| s.subclass == "campfire") {
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

        // Then ring the base with walls.
        let wall_count = view.structures.iter().filter(|s| s.subclass == "wall").count();
        if wall_count < MAX_WALLS && self.walls_attempted < MAX_WALLS {
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
            .map(|f| has_all_reqs(&f.inventory, &as_req_slice(&job.reqs)))
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
            if f.founded {
                if hero_pos != f.pos {
                    return self.step_toward(hero_pos, f.pos, view, map);
                }
                if !job.build_issued {
                    job.build_issued = true;
                    return Some(PlayerEvent::Build {
                        player_id: self.player_id,
                        builder_id: hero_id,
                        structure_id: f.id,
                    });
                }
            }
            // Phase F: building -> wait.
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
        if view.monolith.map_or(true, |m| m.level >= SANCTUARY_MAX_LEVEL) {
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
    fn consume_action(&self, hero: &HeroView, view: &WorldView) -> Option<PlayerEvent> {
        if hero.hunger >= CONSUME_AT {
            // Eat the HIGHEST-Feed food first (Cooked Meat 100 over berries ~6) so
            // the hero refills in one bite and isn't stuck eating constantly.
            if let Some(id) = view
                .inventory
                .iter()
                .filter(|i| i.is_edible())
                .max_by(|a, b| a.feed.partial_cmp(&b.feed).unwrap_or(std::cmp::Ordering::Equal))
                .map(|i| i.id)
            {
                return Some(PlayerEvent::Use {
                    player_id: self.player_id,
                    obj_id: hero.id,
                    item_id: id,
                });
            }
        }
        if hero.thirst >= CONSUME_AT {
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
    fn food_action(
        &mut self,
        hero: &HeroView,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
        // 1. Butcher a carcass (a "Felled X", class "Game Animal") into raw meat.
        if let Some(carcass) = view.inventory.iter().find(|i| i.class == "Game Animal") {
            return Some(PlayerEvent::Refine {
                player_id: self.player_id,
                item_id: carcass.id,
            });
        }

        // 2. Cook raw meat into Cooked Meat (the craft). Cooking is fast (20 ticks)
        //    and raw meat is poisonous, so always cook rather than eat it raw.
        let has_raw_meat = view.inventory.iter().any(|i| i.subclass == "Raw Meat");
        if has_raw_meat {
            if let Some(action) = self.cook_action(hero, view, map) {
                return Some(action);
            }
        }

        // 3. Low on GOOD food (high-Feed): restock. Foraged berries/mushrooms have
        //    tiny Feed, so the hero would eat them nonstop and never sleep — the
        //    efficient answer is to hunt and cook a batch of Cooked Meat (Feed 100).
        // 3. Maintain a stock of GOOD food (Cooked Meat, Feed 100). Foraged
        //    berries/mushrooms (~6-8 Feed) only bootstrap the first day — relying on
        //    them means eating nonstop. So once a campfire stands and game is in
        //    reach, hunt + cook proactively to keep a few Cooked Meat on hand. Gated
        //    on calm needs so the hunt/cook doesn't itself trigger a crisis.
        let good_food: i32 = view
            .inventory
            .iter()
            .filter(|i| i.is_edible() && i.feed >= GOOD_FEED)
            .map(|i| i.quantity)
            .sum();
        if good_food < 3
            && hero.tired < CONSUME_AT
            && hero.thirst < CONSUME_AT
            && self.can_hunt_locally(hero, view)
        {
            if let Some(action) = self.hunt_action(hero, view, map) {
                return Some(action);
            }
        }

        // 4. No good food and actually hungry: pull from the Burrow, else forage to
        //    limp along (early game / no campfire / no game nearby).
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
        if !view.inventory.iter().any(|i| i.name == "Firewood" && i.quantity > 0) {
            return false;
        }
        let home = view.home().or(self.anchor).unwrap_or(hero.pos);
        view.resource_tiles
            .iter()
            .any(|t| t.has_game && hex_dist(home, t.pos) <= HUNT_RADIUS)
    }

    // Hunt a Game Animal: equip the Hunting weapon (the starting Sharpened Stick),
    // then gather a revealed game tile (prospect to reveal one — game spawns under
    // grassland/plains hexes near the base). Yields a carcass to butcher in (1).
    fn hunt_action(
        &mut self,
        hero: &HeroView,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
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
    fn cook_action(
        &self,
        hero: &HeroView,
        view: &WorldView,
        map: &Map,
    ) -> Option<PlayerEvent> {
        let campfire = view
            .structures
            .iter()
            .find(|s| s.subclass == "campfire" && s.built)?;

        // Retrieve a finished Cooked Meat from the campfire.
        if let Some(cooked) = campfire.inventory.iter().find(|i| i.subclass == "Cooked Meat") {
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
    fn new(
        name: &str,
        subclass: &str,
        reqs: &[(&str, i32)],
        site: Position,
        tick: i32,
    ) -> Self {
        BuildJob {
            structure_name: name.to_string(),
            subclass: subclass.to_string(),
            reqs: reqs.iter().map(|(t, q)| (t.to_string(), *q)).collect(),
            site,
            structure_id: None,
            build_issued: false,
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
    items.iter().filter(|i| i.name == name).map(|i| i.quantity).sum()
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
    reqs.iter()
        .all(|(t, q)| count_matching(items, t) >= *q)
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
        for s in view.structures.iter().filter(|s| s.subclass == "storage" && s.built) {
            if let Some(item) = s.inventory.iter().find(|i| i.matches_req(req_type)) {
                return Some((s.pos, s.id, item.id));
            }
        }
    }
    None
}

// A Food item sitting in an owned storage (e.g. the Burrow's berries) to pull.
fn storage_food(view: &WorldView) -> Option<(Position, i32, i32)> {
    for s in view.structures.iter().filter(|s| s.subclass == "storage" && s.built) {
        if let Some(item) = s.inventory.iter().find(|i| i.class == "Food" && i.quantity > 0) {
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

// A Gold Coins stack sitting in an owned storage (the Burrow starts with 50) to
// withdraw for hiring. Returns (storage_pos, storage_id, item_id).
fn storage_gold(view: &WorldView) -> Option<(Position, i32, i32)> {
    for s in view.structures.iter().filter(|s| s.subclass == "storage" && s.built) {
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
        for s in view.structures.iter().filter(|s| s.subclass == "storage" && s.built) {
            if s.inventory.iter().any(|i| i.matches_req(req_type)) {
                return Some(s.id);
            }
        }
    }
    None
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
