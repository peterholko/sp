// Deterministic scripted bot for the headless harness.
//
// Reads a `WorldView` snapshot + the `Map` and returns at most one `PlayerEvent`
// per decision step. No RNG and no per-run seed: given the same world state it
// always returns the same action, so two back-to-back runs produce identical
// metrics (the isolation regression guard the runner asserts).
//
// Movement note: the server's MoveEvent only accepts a destination ADJACENT to
// the mover (game.rs `move_event_system`), so the bot walks one hex step at a
// time, greedily reducing hex distance to its target. It only issues a new
// action when the hero is idle (`State::None`); while an action resolves (the
// hero is Moving/Gathering/…) it returns `None` so it never cancels in-flight
// work. Combat always takes priority over the current phase.

use crate::headless::{UnitView, WorldView};
use crate::map::Map;
use crate::obj::Position;
use crate::PlayerEvent;

// Engage any enemy within this hex distance.
const AGGRO_RADIUS: u32 = 6;
// Walk to resource nodes within this hex distance (phase-dependent below).
const GATHER_RADIUS_NEAR: u32 = 6;
const GATHER_RADIUS_FAR: u32 = 12;

// Deterministic exploration offsets from the spawn anchor, walked in order.
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
    Gather,
    Build,
    Fight,
    Explore,
    Done,
}

pub struct Bot {
    player_id: i32,
    phase: Phase,
    anchor: Option<Position>,
    explore_cursor: usize,
}

impl Bot {
    pub fn new(player_id: i32) -> Self {
        Bot {
            player_id,
            phase: Phase::Bootstrap,
            anchor: None,
            explore_cursor: 0,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    // Pick the next action for this decision step. `None` means "do nothing this
    // step" (busy, dead, or no productive move available).
    pub fn step(&mut self, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        let hero = view.hero?;

        if self.anchor.is_none() {
            self.anchor = Some(hero.pos);
        }

        // Only act when idle; otherwise let the current action resolve.
        if !hero.is_idle() {
            return None;
        }

        // 1/2. Combat takes priority in every phase.
        if let Some(enemy) = nearest_enemy(hero.pos, view) {
            let d = hex_dist(hero.pos, enemy.pos);
            if d <= 1 {
                return Some(PlayerEvent::Attack {
                    player_id: self.player_id,
                    attack_type: "quick".to_string(),
                    source_id: hero.id,
                    target_id: enemy.id,
                });
            } else if d <= AGGRO_RADIUS {
                if let Some(mv) = self.step_toward(hero.pos, enemy.pos, view, map) {
                    return Some(mv);
                }
            }
        }

        // 3. Gather nearby resources (skip while in the pure Explore phase so the
        //    hero actually ranges out instead of orbiting the same node).
        if self.phase != Phase::Explore {
            let radius = match self.phase {
                Phase::Bootstrap => GATHER_RADIUS_NEAR,
                _ => GATHER_RADIUS_FAR,
            };
            if let Some(res) = nearest_resource(hero.pos, view, radius) {
                if res == hero.pos {
                    return Some(PlayerEvent::Gather {
                        player_id: self.player_id,
                    });
                } else if let Some(mv) = self.step_toward(hero.pos, res, view, map) {
                    return Some(mv);
                }
            }
        }

        // 4. Explore: walk toward deterministic waypoints around the anchor.
        self.explore(hero.pos, view, map)
    }

    // Update the coarse phase from the survival day. Combat/gather behaviour is
    // unified in `step`; the phase only biases gather radius and exploration.
    pub fn advance_phase(&mut self, view: &WorldView) {
        let Some(hero) = view.hero else {
            self.phase = Phase::Done;
            return;
        };
        if hero.dead || hero.true_death {
            self.phase = Phase::Done;
            return;
        }

        self.phase = match view.day {
            0 | 1 => Phase::Bootstrap,
            2..=3 => Phase::Gather,
            4..=5 => {
                // "Build" days — still gather; placeholder for future build orders.
                Phase::Build
            }
            6..=7 => Phase::Fight,
            _ => Phase::Explore,
        };
    }

    // Greedy one-hex step toward `target`: pick the adjacent passable, unoccupied
    // tile that most reduces hex distance. Returns `None` if no neighbour gets us
    // strictly closer (blocked / already adjacent).
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

        let mut best: Option<(i32, i32, u32)> = None;
        for (nx, ny) in Map::range((from.x, from.y), 1) {
            if nx == from.x && ny == from.y {
                continue;
            }
            if !Map::is_valid_pos((nx, ny)) {
                continue;
            }
            if !Map::is_passable(nx, ny, map) {
                continue;
            }
            if view.occupied.contains(&(nx, ny)) {
                continue;
            }
            let d = hex_dist(Position { x: nx, y: ny }, target);
            match best {
                Some((_, _, bd)) if d >= bd => {}
                _ => best = Some((nx, ny, d)),
            }
        }

        let (bx, by, bd) = best?;
        if bd >= current {
            return None;
        }
        Some(PlayerEvent::Move {
            player_id: self.player_id,
            x: bx,
            y: by,
        })
    }

    fn explore(&mut self, from: Position, view: &WorldView, map: &Map) -> Option<PlayerEvent> {
        let anchor = self.anchor.unwrap_or(from);

        // Try each waypoint until one yields a productive step.
        for _ in 0..EXPLORE_OFFSETS.len() {
            let (ox, oy) = EXPLORE_OFFSETS[self.explore_cursor];
            let wp = clamp_pos(Position {
                x: anchor.x + ox,
                y: anchor.y + oy,
            });

            if hex_dist(from, wp) <= 2 {
                // Reached this waypoint; advance to the next.
                self.explore_cursor = (self.explore_cursor + 1) % EXPLORE_OFFSETS.len();
                continue;
            }

            if let Some(mv) = self.step_toward(from, wp, view, map) {
                return Some(mv);
            }

            // Blocked toward this waypoint; try the next one.
            self.explore_cursor = (self.explore_cursor + 1) % EXPLORE_OFFSETS.len();
        }

        None
    }
}

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

fn nearest_resource(from: Position, view: &WorldView, radius: u32) -> Option<Position> {
    view.resource_tiles
        .iter()
        .filter(|p| hex_dist(from, **p) <= radius)
        .min_by_key(|p| hex_dist(from, **p))
        .copied()
}
