use bevy::prelude::*;
use big_brain::prelude::*;

use crate::constants::*;
use crate::obj::Position;

#[derive(Debug, Reflect, Component, Default)]
#[reflect(Component)]
pub struct Destination {
    pub pos: Position,
}

#[derive(Debug, Reflect, Component, Default)]
#[reflect(Component)]
pub struct Target {
    pub id: i32,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct MoveTo;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Hide;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Idle {
    pub start_time: i32,
    pub duration: i32,
}

#[derive(Debug, Reflect, Component, Default)]
#[reflect(Component)]
pub struct Transport {
    pub route: Vec<Position>,
    pub next_stop: i32,
    pub hauling: Vec<i32>,
}

// Tag to indicate extreme drowsinest
#[derive(Debug, Clone, Component)]
pub struct Exhausted {
    pub at_tick: i32,
}

#[derive(Debug, Clone, Component)]
pub struct Dehydrated {
    pub at_tick: i32,
}

// Starving is an tag to indicate extreme hunger
#[derive(Debug, Clone, Component)]
pub struct Starving {
    pub at_tick: i32,
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Drink;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Eat;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Sleep;

#[derive(Component, Debug)]
pub struct Thirst {
    pub per_tick: f32,
    pub thirst: f32,
}

impl Thirst {
    pub fn new(thirst: f32, per_tick: f32) -> Self {
        Self { thirst, per_tick }
    }

    pub fn add(&mut self, value: f32) {
        if self.thirst + value > 100.0 {
            self.thirst = 100.0;
        } else if self.thirst + value < 0.0 {
            self.thirst = 0.0;
        } else {
            self.thirst += value;
        }
    }

    pub fn update_by_tick_amount(&mut self, extra_mod: f32) {
        Self::add(self, self.per_tick * extra_mod)
    }

    pub fn num_to_string(&self) -> String {
        match &self.thirst {
            x if *x < 15.0 => return HYDRATED.to_string(),
            x if *x < 30.0 => return REFRESHED.to_string(),
            x if *x < 60.0 => return SLIGHTLY_THIRSTY.to_string(),
            x if *x < 75.0 => return THIRSTY.to_string(),
            x if *x < 90.0 => return PARCHED.to_string(),
            x if *x <= 100.0 => return DEHYDRATED.to_string(),
            _ => return "Unknown".to_string(),
        }
    }
}

#[derive(Component, Debug)]
pub struct Tired {
    pub tired: f32,
    pub per_tick: f32,
}

impl Tired {
    pub fn new(tired: f32, per_tick: f32) -> Self {
        Self { tired, per_tick }
    }

    pub fn update(&mut self, value: f32) {
        if self.tired + value > 100.0 {
            self.tired = 100.0;
        } else if self.tired + value < 0.0 {
            self.tired = 0.0;
        } else {
            self.tired += value;
        }
    }

    pub fn update_by_tick_amount(&mut self, extra_mod: f32) {
        Self::update(self, self.per_tick * extra_mod)
    }

    pub fn num_to_string(&self) -> String {
        match &self.tired {
            x if *x < 15.0 => return ENERGIZED.to_string(),
            x if *x < 30.0 => return RESTORED.to_string(),
            x if *x < 60.0 => return WEARY.to_string(),
            x if *x < 75.0 => return TIRED.to_string(),
            x if *x < 90.0 => return EXHAUSTED.to_string(),
            x if *x <= 100.0 => return DEPELTED.to_string(),
            _ => return "Unknown".to_string(),
        }
    }
}

#[derive(Component, Debug)]
pub struct Heat {
    pub heat: f32,
}

impl Heat {
    pub fn new(heat: f32) -> Self {
        Self { heat }
    }

    pub fn update(&mut self, value: f32) {
        if self.heat + value > 100.0 {
            self.heat = 100.0;
        } else if self.heat + value < -100.0 {
            self.heat = -100.0;
        } else {
            self.heat += value;
        }
    }

    pub fn update_to_comfortable(&mut self, value: f32) {
        if self.heat > 0.0 {
            self.heat -= value;

            if self.heat < 0.0 {
                self.heat = 0.0;
            }
        } else {
            self.heat += value;

            if self.heat > 0.0 {
                self.heat = 0.0;
            }
        }
    }
}

#[derive(Component, Debug)]
pub struct Hunger {
    pub hunger: f32,
    pub per_tick: f32,
}

impl Hunger {
    pub fn new(hunger: f32, per_tick: f32) -> Self {
        Self { hunger, per_tick }
    }

    pub fn update(&mut self, value: f32) {
        if self.hunger + value > 100.0 {
            self.hunger = 100.0;
        } else if self.hunger + value < 0.0 {
            self.hunger = 0.0;
        } else {
            self.hunger += value;
        }
    }

    pub fn update_by_tick_amount(&mut self, extra_mod: f32) {
        Self::update(self, self.per_tick * extra_mod)
    }

    pub fn num_to_string(&self) -> String {
        match &self.hunger {
            x if *x < 15.0 => return SATIATED.to_string(),
            x if *x < 30.0 => return NOURISHED.to_string(),
            x if *x < 60.0 => return HUNGRY.to_string(),
            x if *x < 75.0 => return PECKISH.to_string(),
            x if *x < 90.0 => return FAMISHED.to_string(),
            x if *x <= 100.0 => return RAVENOUS.to_string(),
            _ => return "Unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct AttackTarget;

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct SetAttackTarget;

#[derive(Debug, Component)]
pub struct TaskTarget {
    pub target: i32,
}

impl TaskTarget {
    pub fn new(target: i32) -> Self {
        Self { target }
    }
}
