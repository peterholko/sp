// Configure clippy for Bevy usage
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::enum_glob_use)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]


use bevy::log::LogPlugin;
use bevy::scene::ScenePlugin;
use bevy::state::app::StatesPlugin;
use bevy::{
    app::ScheduleRunnerPlugin,
    prelude::*,
};
use bevy::diagnostic::FrameCountPlugin;
use bevy::app::TaskPoolPlugin;
use core::time::Duration;
use bevy::state::state::States;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::Layer;

use game::{GamePlugin};

pub mod event;
pub mod database;
pub mod game;
pub mod obj;
pub mod constants;
pub mod world;

mod combat;
mod effect;
mod encounter;
mod experiment;
pub mod item;
mod ids;
mod map;
mod network;
mod player;
mod player_setup;
mod recipe;
mod resource;
mod structure;
mod templates;
mod villager_util;
mod terrain_feature;
mod farm;
mod trade;

#[path = "ai/common/common.rs"]
mod common;

#[path = "ai/common/logging.rs"]
pub mod ai_logging;

#[path = "ai/npc/npc.rs"]
mod npc;

#[path = "ai/villager/villager.rs"]
mod villager;

#[path = "ai/tax_collector/tax_collector.rs"]
mod tax_collector;

#[path = "skill/skill.rs"]
mod skill;

#[path = "skill/skill_defs.rs"]
mod skill_defs;

const TIMESTEP_10_PER_SECOND: f64 = 1.0 / 10.0;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[default]
    Loading,
    PreRunning,
    Running,
}

pub fn setup(command: &String) {

    let mut new_game = true;

    if command == "reload" {
        println!("Reloading");
        new_game = false;
    }

    App::new()
        .add_plugins(StatesPlugin)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin::default())
        .add_plugins(TaskPoolPlugin::default())
        .add_plugins(FrameCountPlugin::default())
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            TIMESTEP_10_PER_SECOND,
        )))
        .add_plugins(LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,naga=error,bevy_ecs=warn,big_brain=warn,\
                     siege_perilous::npc=info,siege_perilous::villager=info,\
                     siege_perilous::ai=info,siege_perilous::tax_collector=info,\
                     siege_perilous::game=info,siege_perilous::item=info,\
                     siege_perilous::map=warn,siege_perilous::world=info".into(),
            custom_layer: |_| {
                // Create a file appender for AI debug logs
                let file_appender = tracing_appender::rolling::daily("logs", "ai_debug.log");
                let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

                // Leak the guard to keep it alive for the lifetime of the program
                // This is necessary because the guard must outlive the layer
                Box::leak(Box::new(_guard));

                // Create a filter for AI modules at DEBUG level
                let ai_filter = Targets::new()
                    .with_target("siege_perilous::npc", tracing::Level::DEBUG)
                    .with_target("siege_perilous::villager", tracing::Level::DEBUG)
                    .with_target("siege_perilous::ai", tracing::Level::DEBUG)
                    .with_target("siege_perilous::tax_collector", tracing::Level::DEBUG);

                // Create a formatting layer that writes to the file
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_filter(ai_filter);

                Some(Box::new(file_layer))
            },
            fmt_layer: |_| None,
        })
        .add_plugins(GamePlugin {
            new_game: new_game,
        })
        .init_state::<AppState>()
        .register_type::<combat::Combo>()
        .register_type::<common::Destination>()
        .register_type::<common::Target>()
        .register_type::<common::Transport>()
        .register_type::<effect::Effect>()
        .register_type::<effect::EffectAttr>()
        .register_type::<effect::EffectInfo>()
        .register_type::<effect::EffectVal>()
        .register_type::<encounter::EncounterProbability>()
        .register_type::<event::EmbarkAction>()
        .register_type::<event::GameEvent>()
        .register_type::<event::GameEventType>()
        .register_type::<event::GameEvents>()
        .register_type::<event::MapEvent>()
        .register_type::<event::MapEvents>()
        .register_type::<event::Spell>()
        .register_type::<event::VisibleEvent>()
        .register_type::<event::VisibleEvents>()
        .register_type::<game::DamageRecord>()
        .register_type::<game::ExploredMap>()
        .register_type::<game::Home>()
        .register_type::<game::GameTick>()
        .register_type::<game::Merchant>()
        .register_type::<game::Minions>()
        .register_type::<game::PlayerStat>()
        .register_type::<game::PlayerStats>()
        .register_type::<ids::Ids>()
        .register_type::<item::AttrKey>()
        .register_type::<item::AttrVal>()
        .register_type::<item::ExperimentItemType>()
        .register_type::<item::Inventory>()
        .register_type::<item::Item>()
        .register_type::<item::ItemSubclass>()
        .register_type::<map::Map>()
        .register_type::<map::MoistureType>()
        .register_type::<map::TemperatureType>()
        .register_type::<map::TileInfo>()
        .register_type::<map::TileType>()
        .register_type::<obj::Class>()
        .register_type::<obj::EndRepeatAction>()
        .register_type::<obj::Id>()
        .register_type::<obj::Misc>()
        .register_type::<obj::Name>()
        .register_type::<obj::PlayerId>()
        .register_type::<obj::Position>()
        .register_type::<obj::Sheltered>()
        .register_type::<obj::State>()
        .register_type::<obj::Subclass>()
        .register_type::<obj::Template>()
        .register_type::<obj::Viewshed>()
        .register_type::<skill_defs::Skill>()
        .register_type::<skill::SkillData>()
        .register_type::<skill::Skills>()
        .register_type::<tax_collector::Merchant>()
        .register_type::<tax_collector::TaxCollector>()
        .register_type::<tax_collector::TaxCollectorTransport>()
        .register_type::<templates::ItemAttr>()
        .register_type::<templates::ItemTemplate>()
        .register_type::<trade::Price>()
        .register_type::<trade::Prices>()
        .register_type::<trade::TradePort>()
        .register_type::<trade::TradePorts>()
        .register_type::<trade::WantedItem>()        
        .init_asset::<DynamicScene>()
        .run();
}
