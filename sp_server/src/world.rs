use crate::{
    constants::*,
    effect::Effects,
    event::VisibleEvent,
    game::{Clients, GameTick},
    item::{self, Inventory},
    map::Map,
    network::{send_to_client, MapWeather, ResponsePacket},
    obj::{Id, Obj, PlayerId, SubclassNPC, Template, UpdateObj, Viewshed},
    templates::{ObjTemplate, Templates},
    AppState,
};
use bevy::prelude::*;
use rand::Rng;

#[derive(Debug, Clone)]
pub enum Weather {
    ClearSunny,
    HeavyRain,
    Thunderstorm,
    Moonsoon,
    Hurricane,
    Fog,
    ColdSnap,
    Snow,
    Blizzard,
    PolarVortex,
    Hail,
    Heatwave,
    Drought,
    Duststorm,
    SuperTyphoon,
    FlashFlood,
    IceStorm,
    FireStorm,
    Tornado,
    LightningSuperstorm,
}

impl Weather {
    pub fn to_string(&self) -> String {
        let str = match self {
            Weather::ClearSunny => "Clear and Sunny",
            Weather::HeavyRain => "Heavy Rain",
            Weather::Thunderstorm => "Thunderstorm",
            Weather::Moonsoon => "Monsoon",
            Weather::Hurricane => "Hurricane",
            Weather::Fog => "Fog",
            Weather::ColdSnap => "Cold Snap",
            Weather::Snow => "Snow",
            Weather::Blizzard => "Blizzard",
            Weather::PolarVortex => "Polar Vortex",
            Weather::Hail => "Hail",
            Weather::Heatwave => "Heatwave",
            Weather::Drought => "Drought",
            Weather::Duststorm => "Duststorm",
            Weather::SuperTyphoon => "Super Typhoon",
            Weather::FlashFlood => "Flash Flood",
            Weather::IceStorm => "Ice Storm",
            Weather::FireStorm => "Fire Storm",
            Weather::Tornado => "Tornado",
            Weather::LightningSuperstorm => "Lightning Superstorm",
        };

        return str.to_string();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeOfDay {
    FirstLight,
    Dawn,
    Morning,
    Afternoon,
    Evening,
    Dusk,
    Night,
}

impl TimeOfDay {
    pub fn to_string(&self) -> String {
        match self {
            TimeOfDay::FirstLight => "First Light",
            TimeOfDay::Dawn => "Dawn",
            TimeOfDay::Morning => "Morning",
            TimeOfDay::Afternoon => "Afternoon",
            TimeOfDay::Evening => "Evening",
            TimeOfDay::Dusk => "Dusk",
            TimeOfDay::Night => "Night",
        }
        .to_string()
    }
}

#[derive(Debug, Clone)]
pub struct WeatherArea {
    pub center: (i32, i32),
    pub weather: Weather,
    pub area: Vec<(i32, i32)>,
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct WeatherAreas(Vec<WeatherArea>);

impl WeatherAreas {
    pub fn get_visible_weather_tiles(&self, visible_pos: &Vec<(i32, i32)>) -> Vec<MapWeather> {
        // Get visible weather tiles from weather areas
        let mut visible_weather_tiles = Vec::new();

        for weather_area in self.iter() {
            for pos in visible_pos {
                if weather_area.area.contains(&pos) {
                    let map_weather = MapWeather {
                        x: pos.0,
                        y: pos.1,
                        weather: weather_area.weather.to_string(),
                    };

                    visible_weather_tiles.push(map_weather);
                }
            }
        }

        return visible_weather_tiles;
    }
}

pub fn create_weather_area(center_x: i32, center_y: i32, weather: Weather) -> WeatherArea {
    let radius = rand::thread_rng().gen_range(3..5);
    let area = Map::range((center_x, center_y), radius as u32);

    let weather_area = WeatherArea {
        center: (center_x, center_y),
        weather: weather,
        area: area,
    };

    return weather_area;
}

pub fn get_time_of_day(game_tick: i32) -> TimeOfDay {
    let ticks_in_day = game_tick.rem_euclid(GAME_TICKS_PER_DAY);

    if ticks_in_day < FIRST_LIGHT {
        TimeOfDay::Night
    } else if ticks_in_day < DAWN {
        TimeOfDay::FirstLight
    } else if ticks_in_day < MORNING {
        TimeOfDay::Dawn
    } else if ticks_in_day < AFTERNOON {
        TimeOfDay::Morning
    } else if ticks_in_day < EVENING {
        TimeOfDay::Afternoon
    } else if ticks_in_day < DUSK {
        TimeOfDay::Evening
    } else if ticks_in_day < NIGHT {
        TimeOfDay::Dusk
    } else {
        TimeOfDay::Night
    }
}

pub fn time_of_day_vision_mod(game_ticks: i32) -> f32 {
    let remainder = game_ticks % GAME_TICKS_PER_DAY;

    if remainder >= NIGHT || remainder < FIRST_LIGHT {
        // NIGHT: 2200 → 2400 and 0 → 400
        0.0
    } else if remainder < DAWN {
        // Pre-dawn twilight
        0.5
    } else if remainder < MORNING {
        // Dawn/morning transition
        0.75
    } else if remainder < EVENING {
        // Full daylight
        1.0
    } else if remainder < DUSK {
        // Evening
        0.75
    } else {
        // Dusk (just after sunset)
        0.5
    }
}

pub fn day_system(
    mut commands: Commands,
    game_tick: Res<GameTick>,
    app_state: Option<Res<State<AppState>>>,
    clients: Option<Res<Clients>>,
    templates: Option<Res<Templates>>,
    player_query: Query<&PlayerId>,
    mut viewshed_query: Query<
        (Entity, &Id, &Template, &Inventory, &mut Viewshed, Option<&Effects>),
        Without<SubclassNPC>,
    >,
) {
    if let Some(state) = app_state {
        if *state.get() != AppState::Running {
            return;
        }
    }

    let remainder = game_tick.0 % GAME_TICKS_PER_DAY;

    if remainder == FIRST_LIGHT
        || remainder == DAWN
        || remainder == MORNING
        || remainder == EVENING
        || remainder == DUSK
        || remainder == NIGHT
    {
        let templates_res = templates.as_ref();
        let time_mod = time_of_day_vision_mod(game_tick.0);
        let is_night = remainder >= NIGHT || remainder < FIRST_LIGHT;

        for (entity, id, template, inventory, mut viewshed, effects) in viewshed_query.iter_mut() {
            let template_name = &template.0;
            let base_vision = templates_res
                .and_then(|templates| {
                    templates
                        .obj_templates
                        .iter()
                        .find(|obj_template| obj_template.template == *template_name)
                        .and_then(|obj_template| {
                            obj_template.base_vision.map(|vision| vision as f32)
                        })
                })
                .unwrap_or(viewshed.range as f32);

            let vision_modifier = match (templates_res, effects) {
                (Some(templates), Some(effects)) => effects.get_vision_modifier(templates),
                _ => 0.0,
            };

            let item_vision_mod = if is_night {
                inventory.get_items_value_by_attr(&item::AttrKey::Vision, true)
            } else {
                0.0
            };

            viewshed.range = (base_vision * time_mod + item_vision_mod + vision_modifier)
                .floor()
                .max(0.0) as u32;

            info!("Update vision for obj: {:?}", id.0);

            //Add obj update event
            commands.trigger(UpdateObj {
                entity: entity,
                attrs: vec![(VISION.to_string(), "Pending".to_string())],
            });
        }

        // Make unique list of player ids
        let mut player_ids = Vec::new();
        for player_id in player_query.iter() {
            if !player_ids.contains(&player_id.0) {
                player_ids.push(player_id.0);
            }
        }

        if let Some(clients) = clients {
            for player_id in player_ids {
                let world_packet = ResponsePacket::World {
                    time_of_day: game_tick.time_of_day(),
                    day: game_tick.day(),
                };

                send_to_client(player_id, world_packet, &clients);
            }
        }
    }
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        //let weather_area = create_weather_area(5, 30, Weather::Snow);
        //let weather_areas = WeatherAreas(vec![weather_area]);
        let weather_areas = WeatherAreas(Vec::new());

        app.insert_resource(weather_areas);

        app.add_systems(Update, day_system.run_if(in_state(AppState::Running)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_visible_weather_tiles() {
        let weather_area = WeatherArea {
            center: (0, 0),
            weather: Weather::HeavyRain,
            area: vec![(0, 0), (0, 1), (1, 0), (1, 1)],
        };
        let weather_areas = WeatherAreas(vec![weather_area]);
        let visible_pos = vec![(0, 0), (1, 1), (2, 2)];
        let visible_weather_tiles = weather_areas.get_visible_weather_tiles(&visible_pos);

        assert_eq!(visible_weather_tiles.len(), 2);
        assert_eq!(visible_weather_tiles[0].x, 0);
        assert_eq!(visible_weather_tiles[0].y, 0);
        assert_eq!(
            visible_weather_tiles[0].weather,
            Weather::HeavyRain.to_string()
        );
        assert_eq!(visible_weather_tiles[1].x, 1);
        assert_eq!(visible_weather_tiles[1].y, 1);
        assert_eq!(
            visible_weather_tiles[1].weather,
            Weather::HeavyRain.to_string()
        );
    }

    #[test]
    fn test_get_time_of_day_night_early() {
        // Test early night (0-399)
        assert_eq!(get_time_of_day(0), TimeOfDay::Night);
        assert_eq!(get_time_of_day(200), TimeOfDay::Night);
        assert_eq!(get_time_of_day(399), TimeOfDay::Night);
    }

    #[test]
    fn test_get_time_of_day_first_light() {
        // Test first light (400-499)
        assert_eq!(get_time_of_day(400), TimeOfDay::FirstLight);
        assert_eq!(get_time_of_day(450), TimeOfDay::FirstLight);
        assert_eq!(get_time_of_day(499), TimeOfDay::FirstLight);
    }

    #[test]
    fn test_get_time_of_day_dawn() {
        // Test dawn (500-599)
        assert_eq!(get_time_of_day(500), TimeOfDay::Dawn);
        assert_eq!(get_time_of_day(550), TimeOfDay::Dawn);
        assert_eq!(get_time_of_day(599), TimeOfDay::Dawn);
    }

    #[test]
    fn test_get_time_of_day_morning() {
        // Test morning (600-1199)
        assert_eq!(get_time_of_day(600), TimeOfDay::Morning);
        assert_eq!(get_time_of_day(900), TimeOfDay::Morning);
        assert_eq!(get_time_of_day(1199), TimeOfDay::Morning);
    }

    #[test]
    fn test_get_time_of_day_afternoon() {
        // Test afternoon (1200-1799)
        assert_eq!(get_time_of_day(1200), TimeOfDay::Afternoon);
        assert_eq!(get_time_of_day(1500), TimeOfDay::Afternoon);
        assert_eq!(get_time_of_day(1799), TimeOfDay::Afternoon);
    }

    #[test]
    fn test_get_time_of_day_evening() {
        // Test evening (1800-1999)
        assert_eq!(get_time_of_day(1800), TimeOfDay::Evening);
        assert_eq!(get_time_of_day(1900), TimeOfDay::Evening);
        assert_eq!(get_time_of_day(1999), TimeOfDay::Evening);
    }

    #[test]
    fn test_get_time_of_day_dusk() {
        // Test dusk (2000-2199)
        assert_eq!(get_time_of_day(2000), TimeOfDay::Dusk);
        assert_eq!(get_time_of_day(2100), TimeOfDay::Dusk);
        assert_eq!(get_time_of_day(2199), TimeOfDay::Dusk);
    }

    #[test]
    fn test_get_time_of_day_night_late() {
        // Test late night (2200-2399)
        assert_eq!(get_time_of_day(2200), TimeOfDay::Night);
        assert_eq!(get_time_of_day(2300), TimeOfDay::Night);
        assert_eq!(get_time_of_day(2399), TimeOfDay::Night);
    }

    #[test]
    fn test_get_time_of_day_multiple_days() {
        // Test that the function works correctly across multiple days
        assert_eq!(get_time_of_day(2400), TimeOfDay::Night); // Day 2, tick 0
        assert_eq!(get_time_of_day(2900), TimeOfDay::Dawn); // Day 2, tick 500
        assert_eq!(get_time_of_day(4800), TimeOfDay::Night); // Day 3, tick 0
        assert_eq!(get_time_of_day(7200), TimeOfDay::Night); // Day 4, tick 0
    }

    #[test]
    fn test_get_time_of_day_negative_ticks() {
        // Test that negative ticks are handled correctly with rem_euclid
        assert_eq!(get_time_of_day(-100), TimeOfDay::Night); // Wraps to 2300
        assert_eq!(get_time_of_day(-400), TimeOfDay::Dusk); // Wraps to 2000
    }

    #[test]
    fn test_time_of_day_to_string() {
        assert_eq!(TimeOfDay::FirstLight.to_string(), "First Light");
        assert_eq!(TimeOfDay::Dawn.to_string(), "Dawn");
        assert_eq!(TimeOfDay::Morning.to_string(), "Morning");
        assert_eq!(TimeOfDay::Afternoon.to_string(), "Afternoon");
        assert_eq!(TimeOfDay::Evening.to_string(), "Evening");
        assert_eq!(TimeOfDay::Dusk.to_string(), "Dusk");
        assert_eq!(TimeOfDay::Night.to_string(), "Night");
    }
}
