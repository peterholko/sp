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
use crate::headless::{HeroView, ItemView, StructureView, UnitView, WorldView};
use crate::map::{Map, TileType};
use crate::obj::Position;
use crate::PlayerEvent;

const AGGRO_RADIUS: u32 = 3; // only engage near enemies (don't chase into trouble)
const DANGER_RADIUS: u32 = 3; // an enemy this close counts as a threat for retreat
const SLEEP_SAFE_RADIUS: u32 = 3; // no enemy within this -> safe to rest/eat
const LOW_HP: f32 = 0.5; // retreat / heal below this HP fraction
const CRITICAL_HP: f32 = 0.3; // emergency heal below this
// Yield to the game's auto-consume just under its 75.0 trigger so the hero is
// already idle when a need crosses the threshold.
const CONSUME_AT: f32 = 70.0;
// Above this tiredness, surviving (resting) overrides fighting.
const CRITICAL_TIRED: f32 = 85.0;
const MAX_WALLS: usize = 6; // cap palisade walls ringed around the base
const WALL_RING: u32 = 2; // ring radius (leaves the inner tiles free to move)
const JOB_WATCHDOG_TICKS: i32 = 2400; // abandon a stuck build job after ~1 day

// Structure recipes the bot builds (req type -> quantity), matching the
// obj_template.yaml `req` fields. Campfire uses the hero's starting Stick+Resin;
// Stockade walls use Logs pulled from the Burrow / foraged.
const CAMPFIRE_REQS: &[(&str, i32)] = &[("Stick", 1), ("Resin", 1)];
const STOCKADE_REQS: &[(&str, i32)] = &[("Log", 3)];

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

        // The hero acts only while idle; a busy hero still lets us command a villager.
        if hero.is_idle() {
            if let Some(action) = self.hero_action(view, map) {
                return Some(action);
            }
        }

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
            // Food: when out of rations and getting hungry, restock — first from
            // the Burrow's stores, then by foraging (plant-picking yields edible
            // berries & mushrooms) — so the hero doesn't starve once its starting
            // food runs out.
            if hero.hunger >= CONSUME_AT && !has_class(&view.inventory, "Food") {
                if let Some((spos, sid, item_id)) = storage_food(view) {
                    if Map::is_adjacent_including_source(hero.pos, spos) {
                        return Some(PlayerEvent::ItemTransfer {
                            player_id: self.player_id,
                            source_id: sid,
                            target_id: hero.id,
                            item_id,
                        });
                    }
                    if let Some(mv) = self.step_adjacent_to(hero.pos, spos, view, map) {
                        return Some(mv);
                    }
                }
                return Some(PlayerEvent::Gather {
                    player_id: self.player_id,
                });
            }
            let want_drink = hero.thirst >= CONSUME_AT && has_class(&view.inventory, "Drink");
            let want_food = hero.hunger >= CONSUME_AT && has_class(&view.inventory, "Food");
            if want_drink || want_food {
                return None; // idle so hero_auto_consume_system can fire
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

        // 7. Forage: gather on a revealed resource tile, else walk to one.
        if let Some(action) = self.forage(view, map) {
            return Some(action);
        }

        // 8. Explore.
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
        // Keep one idle villager gathering wood near the hero.
        let villager = view.villagers.iter().find(|v| v.idle)?;
        Some(PlayerEvent::OrderGather {
            player_id: self.player_id,
            source_id: villager.id,
            res_type: "Log".to_string(),
        })
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
