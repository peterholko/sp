use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::constants::TICKS_PER_SEC;
use crate::game::{Clients, CrisisKind, CrisisPhase, GameTick, SettlementCrisisState};
use crate::obj::{
    ActiveTask, HeroClass, Id, Name, PlayerId, Position, State, Stats, SubclassHero, Template,
};
use crate::safe_logout::{
    PlayerWorldPresence, PlayerWorldPresenceState, SAFE_LOGOUT_COUNTDOWN_TICKS,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminRuntimeSummary {
    pub connected: usize,
    pub safe_logout_pending: usize,
    pub offline_protected: usize,
    pub disconnected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminPositionSnapshot {
    pub x: i32,
    pub y: i32,
}

impl From<Position> for AdminPositionSnapshot {
    fn from(position: Position) -> Self {
        Self {
            x: position.x,
            y: position.y,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminHeroSnapshot {
    pub id: i32,
    pub name: String,
    pub template: String,
    pub class: String,
    pub position: AdminPositionSnapshot,
    pub state: String,
    pub activity: String,
    pub hp: i32,
    pub max_hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminSafeLogoutSnapshot {
    pub requested_tick: Option<i32>,
    pub seconds_remaining: Option<i32>,
    pub start_position: Option<AdminPositionSnapshot>,
    pub protected_since_tick: Option<i32>,
    pub protected_for_seconds: Option<i32>,
    pub last_protection_end_tick: Option<i32>,
    pub cancel_reason: Option<String>,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminCrisisSnapshot {
    pub kind: String,
    pub phase: String,
    pub pressure: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminRuntimePlayerSnapshot {
    pub player_id: i32,
    pub connected: bool,
    pub presence: String,
    pub hero: Option<AdminHeroSnapshot>,
    pub safe_logout: AdminSafeLogoutSnapshot,
    pub crisis: Option<AdminCrisisSnapshot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminRuntimeSnapshot {
    pub generated_at_unix_ms: u64,
    pub game_tick: i32,
    pub game_day: i32,
    pub time_of_day: String,
    pub summary: AdminRuntimeSummary,
    pub players: Vec<AdminRuntimePlayerSnapshot>,
}

/// Read-only status bridge between the Bevy simulation and the asynchronous
/// network runtime. The game thread publishes a small immutable snapshot; an
/// admin request only clones it and never reads or mutates the ECS world.
#[derive(Resource, Clone, Debug, Default)]
pub struct AdminStatusState(Arc<Mutex<AdminRuntimeSnapshot>>);

impl AdminStatusState {
    pub fn publish(&self, snapshot: AdminRuntimeSnapshot) -> bool {
        let Ok(mut current) = self.0.lock() else {
            return false;
        };
        *current = snapshot;
        true
    }

    pub fn snapshot(&self) -> Option<AdminRuntimeSnapshot> {
        self.0.lock().ok().map(|snapshot| snapshot.clone())
    }
}

fn unix_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn crisis_kind_name(kind: CrisisKind) -> &'static str {
    match kind {
        CrisisKind::Goblin => "goblin",
        CrisisKind::Undead => "undead",
    }
}

fn crisis_phase_name(phase: CrisisPhase) -> &'static str {
    match phase {
        CrisisPhase::Dormant => "dormant",
        CrisisPhase::Signs => "signs",
        CrisisPhase::Pressure => "pressure",
        CrisisPhase::Preparing => "preparing",
        CrisisPhase::AssaultReady => "assault_ready",
        CrisisPhase::AssaultActive => "assault_active",
        CrisisPhase::Resolved => "resolved",
    }
}

fn ticks_to_ceil_seconds(ticks: i32) -> i32 {
    ticks
        .max(0)
        .saturating_add(TICKS_PER_SEC - 1)
        .saturating_div(TICKS_PER_SEC)
}

pub(crate) fn refresh_admin_status_system(
    status: Option<Res<AdminStatusState>>,
    clients: Option<Res<Clients>>,
    presence: Option<Res<PlayerWorldPresenceState>>,
    game_tick: Option<Res<GameTick>>,
    crises: Option<Res<SettlementCrisisState>>,
    heroes: Query<
        (
            &Id,
            &PlayerId,
            &Name,
            &Template,
            &HeroClass,
            &Position,
            &State,
            &Stats,
            Option<&ActiveTask>,
        ),
        With<SubclassHero>,
    >,
) {
    let Some(status) = status else {
        return;
    };

    let tick = game_tick.as_ref().map_or(0, |tick| tick.0);
    let mut hero_by_player = HashMap::new();
    for (id, player_id, name, template, class, position, state, stats, activity) in &heroes {
        hero_by_player.insert(
            player_id.0,
            AdminHeroSnapshot {
                id: id.0,
                name: name.0.clone(),
                template: template.0.clone(),
                class: format!("{class:?}"),
                position: (*position).into(),
                state: format!("{state:?}"),
                activity: activity.map_or_else(|| "None".to_string(), ActiveTask::to_string),
                hp: stats.hp,
                max_hp: stats.base_hp,
            },
        );
    }

    let connected_player_ids = clients
        .as_ref()
        .map(|clients| clients.online_player_ids())
        .unwrap_or_default();
    let connected_players = connected_player_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    let mut player_ids = hero_by_player.keys().copied().collect::<BTreeSet<_>>();
    player_ids.extend(connected_player_ids);
    if let Some(presence) = presence.as_ref() {
        player_ids.extend(presence.players.keys().copied());
    }

    let mut players = Vec::with_capacity(player_ids.len());
    for player_id in player_ids {
        let connected = connected_players.contains(&player_id);
        let presence_record = presence
            .as_ref()
            .and_then(|presence| presence.players.get(&player_id));
        let world_presence = presence_record.map_or(
            if connected {
                PlayerWorldPresence::Online
            } else {
                PlayerWorldPresence::Disconnected
            },
            |record| record.state,
        );

        let safe_logout = AdminSafeLogoutSnapshot {
            requested_tick: presence_record.and_then(|record| record.safe_logout_requested_tick),
            seconds_remaining: presence_record
                .and_then(|record| record.safe_logout_requested_tick)
                .map(|requested_tick| {
                    ticks_to_ceil_seconds(
                        requested_tick
                            .saturating_add(SAFE_LOGOUT_COUNTDOWN_TICKS)
                            .saturating_sub(tick),
                    )
                }),
            start_position: presence_record
                .and_then(|record| record.safe_logout_start_position)
                .map(Into::into),
            protected_since_tick: presence_record.and_then(|record| record.protected_since_tick),
            protected_for_seconds: presence_record
                .and_then(|record| record.protected_since_tick)
                .map(|protected_since| {
                    tick.saturating_sub(protected_since)
                        .saturating_div(TICKS_PER_SEC)
                }),
            last_protection_end_tick: presence_record
                .and_then(|record| record.last_protection_end_tick),
            cancel_reason: presence_record
                .and_then(|record| record.cancel_reason)
                .map(|reason| reason.as_str().to_string()),
            rejection_reason: presence_record
                .and_then(|record| record.rejection_reason)
                .map(|reason| reason.as_str().to_string()),
        };

        let crisis = crises
            .as_ref()
            .and_then(|crises| crises.get(&player_id))
            .map(|crisis| AdminCrisisSnapshot {
                kind: crisis_kind_name(crisis.kind).to_string(),
                phase: crisis_phase_name(crisis.phase).to_string(),
                pressure: crisis.pressure,
            });

        players.push(AdminRuntimePlayerSnapshot {
            player_id,
            connected,
            presence: world_presence.as_str().to_string(),
            hero: hero_by_player.remove(&player_id),
            safe_logout,
            crisis,
        });
    }

    let summary = AdminRuntimeSummary {
        connected: players.iter().filter(|player| player.connected).count(),
        safe_logout_pending: players
            .iter()
            .filter(|player| player.presence == "safe_logout_pending")
            .count(),
        offline_protected: players
            .iter()
            .filter(|player| player.presence == "offline_protected")
            .count(),
        disconnected: players
            .iter()
            .filter(|player| player.presence == "disconnected")
            .count(),
    };

    let snapshot = AdminRuntimeSnapshot {
        generated_at_unix_ms: unix_time_millis(),
        game_tick: tick,
        game_day: game_tick.as_ref().map_or(0, |tick| tick.day()),
        time_of_day: game_tick
            .as_ref()
            .map_or_else(|| "Unknown".to_string(), |tick| tick.time_of_day()),
        summary,
        players,
    };

    if !status.publish(snapshot) {
        warn!("admin_status_snapshot_publish_failed reason=poisoned_lock");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::SettlementCrisis;
    use crate::safe_logout::PlayerPresenceRecord;

    #[test]
    fn tick_countdown_rounds_up_to_a_visible_second() {
        assert_eq!(ticks_to_ceil_seconds(0), 0);
        assert_eq!(ticks_to_ceil_seconds(1), 1);
        assert_eq!(ticks_to_ceil_seconds(TICKS_PER_SEC), 1);
        assert_eq!(ticks_to_ceil_seconds(TICKS_PER_SEC + 1), 2);
    }

    #[test]
    fn status_state_publishes_an_immutable_clone() {
        let state = AdminStatusState::default();
        let mut snapshot = AdminRuntimeSnapshot {
            game_tick: 42,
            ..Default::default()
        };
        assert!(state.publish(snapshot.clone()));

        snapshot.game_tick = 99;
        assert_eq!(state.snapshot().expect("published snapshot").game_tick, 42);
    }

    #[test]
    fn snapshot_reports_protected_hero_and_crisis_without_a_live_socket() {
        let mut app = App::new();
        let status = AdminStatusState::default();
        app.insert_resource(status.clone());
        app.insert_resource(GameTick(500));

        let mut presence = PlayerWorldPresenceState::default();
        let mut record = PlayerPresenceRecord::new(false);
        record.state = PlayerWorldPresence::OfflineProtected;
        record.protected_since_tick = Some(400);
        presence.players.insert(7, record);
        app.insert_resource(presence);

        let mut crisis = SettlementCrisis::new(CrisisKind::Goblin, 300);
        crisis.phase = CrisisPhase::Pressure;
        crisis.pressure = 23;
        let mut crises = SettlementCrisisState::default();
        crises.insert(7, crisis);
        app.insert_resource(crises);

        app.world_mut().spawn((
            Id(70),
            PlayerId(7),
            Name("Alden".to_string()),
            Template("Novice Warrior".to_string()),
            HeroClass::Warrior,
            Position { x: 16, y: 36 },
            State::Sleeping,
            Stats {
                hp: 90,
                stamina: Some(40),
                mana: None,
                base_hp: 110,
                base_stamina: Some(100),
                base_mana: None,
                base_def: 0,
                damage_range: Some(1),
                base_damage: Some(1),
                base_speed: Some(10),
                base_vision: Some(3),
            },
            ActiveTask::Sleeping,
            SubclassHero,
        ));
        app.add_systems(Update, refresh_admin_status_system);
        app.update();

        let snapshot = status.snapshot().expect("admin snapshot");
        assert_eq!(snapshot.game_tick, 500);
        assert_eq!(snapshot.summary.connected, 0);
        assert_eq!(snapshot.summary.offline_protected, 1);
        let player = snapshot.players.first().expect("protected player");
        assert_eq!(player.player_id, 7);
        assert_eq!(player.presence, "offline_protected");
        assert_eq!(player.safe_logout.protected_for_seconds, Some(10));
        assert_eq!(player.hero.as_ref().map(|hero| hero.hp), Some(90));
        assert_eq!(
            player.crisis.as_ref().map(|crisis| crisis.pressure),
            Some(23)
        );
    }
}
