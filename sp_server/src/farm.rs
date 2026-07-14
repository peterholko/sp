use std::collections::HashMap;

use bevy::prelude::*;

use crate::{
    game::{Clients, GameTick},
    ids::Ids,
    network::{self, ResponsePacket},
    player::{self, ActiveInfoType, ActiveInfos},
    safe_logout::{object_belongs_to_protected_run, PlayerWorldPresenceState},
    AppState,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CropStages {
    Seed,
    Sprout,
    Sapling,
    Mature,
    Dead,
}

impl CropStages {
    pub fn to_string(&self) -> String {
        match self {
            CropStages::Seed => "Seed".to_string(),
            CropStages::Sprout => "Sprout".to_string(),
            CropStages::Sapling => "Sapling".to_string(),
            CropStages::Mature => "Mature".to_string(),
            CropStages::Dead => "Dead".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Crop {
    pub structure: i32,
    pub crop_type: String,
    pub quantity: i32,
    pub stage: CropStages,
    pub stage_start: i32,
    pub stage_end: i32,
}

impl Crop {}

#[derive(Resource, Deref, DerefMut, Debug)]
pub struct Crops(HashMap<i32, Crop>);

impl Crops {
    pub fn plant(&mut self, game_tick: i32, structure: i32, seed: String, quantity: i32) {
        info!(
            "Plant Crop seed: {:?} quantity: {:?} structure: {:?}",
            seed, quantity, structure
        );
        if let Some(crop) = self.get_mut(&structure) {
            info!("Found Crop: {:?}", crop);

            // Check crop type
            if crop.stage == CropStages::Seed {
                if crop.crop_type == seed {
                    crop.quantity += quantity;
                    info!("Increase quantity Crop: {:?}", crop);
                }
            } else if crop.stage == CropStages::Dead {
                // Replan crop
                crop.crop_type = seed;
                crop.quantity = quantity;
                crop.stage = CropStages::Seed;
                crop.stage_start = game_tick;
                crop.stage_end = game_tick + 120;
            }

            info!("Updated Crop: {:?}", crop);
        } else {
            info!("Inserting new crop...");
            self.insert(
                structure,
                Crop {
                    structure,
                    crop_type: seed,
                    quantity,
                    stage: CropStages::Seed,
                    stage_start: game_tick,
                    stage_end: game_tick + 120,
                },
            );
            info!("New Crop: {:?}", self);
        }
    }

    pub fn harvest(&mut self, structure: i32, quantity: i32) -> Option<Crop> {
        if let Some(crop) = self.get_mut(&structure) {
            if crop.stage == CropStages::Mature {
                info!("Harvested Crop");
                if crop.quantity > quantity {
                    crop.quantity -= quantity;
                    return Some(crop.clone());
                } else {
                    let cloned_crop = crop.clone();
                    self.remove(&structure);
                    return Some(cloned_crop);
                }
            }
        }

        return None;
    }
}

fn crop_system(
    game_tick: ResMut<GameTick>,
    clients: Res<Clients>,
    ids: Res<Ids>,
    active_infos: Res<ActiveInfos>,
    mut crops: ResMut<Crops>,
    presence: Res<PlayerWorldPresenceState>,
) {
    // Iterate through crops and check if start end is greater or equal to game tick
    for (_structure, crop) in crops.iter_mut() {
        if object_belongs_to_protected_run(crop.structure, &ids, &presence) {
            continue;
        }

        if crop.stage_end <= game_tick.0 {
            info!("Crop {:?} has reached stage end.", crop);
            match crop.stage {
                CropStages::Seed => {
                    crop.stage = CropStages::Sprout;
                    crop.stage_start = game_tick.0;
                    crop.stage_end = game_tick.0 + 200;
                }
                CropStages::Sprout => {
                    crop.stage = CropStages::Sapling;
                    crop.stage_start = game_tick.0;
                    crop.stage_end = game_tick.0 + 300;
                }
                CropStages::Sapling => {
                    crop.stage = CropStages::Mature;
                    crop.stage_start = game_tick.0;
                    crop.stage_end = game_tick.0 + 400;
                }
                CropStages::Mature => {
                    crop.stage = CropStages::Dead;
                }
                CropStages::Dead => {
                    crop.stage_end = i32::MAX;
                }
            }

            let Some(player_id) = ids.get_player(crop.structure) else {
                error!("Cannot resolve crop owner for structure {}", crop.structure);
                continue;
            };

            // Check if crop is being observed
            if let Some(_active_info) =
                active_infos.get(&(crop.structure, ActiveInfoType::Structure))
            {
                let info_crop = ResponsePacket::InfoCrop {
                    id: crop.structure,
                    crop_type: crop.crop_type.clone(),
                    crop_quantity: crop.quantity,
                    crop_stage: crop.stage.to_string(),
                };

                info!("info_crop: {:?}", info_crop);

                network::send_to_client(player_id, info_crop, &clients);
            }
        }
    }
}

pub struct FarmPlugin;

impl Plugin for FarmPlugin {
    fn build(&self, app: &mut App) {
        let crops = Crops(HashMap::new());

        app.insert_resource(crops);

        app.add_systems(Update, crop_system.run_if(in_state(AppState::Running)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safe_logout::{PlayerPresenceRecord, PlayerWorldPresence, PlayerWorldPresenceState};

    #[test]
    fn checkpoint2_protected_crop_freezes_while_other_crop_advances() {
        let protected_player = 1;
        let active_player = 2;
        let protected_structure = 10;
        let active_structure = 20;

        let mut ids = Ids::default();
        ids.new_obj(protected_structure, protected_player);
        ids.new_obj(active_structure, active_player);

        let mut presence = PlayerWorldPresenceState::default();
        let mut record = PlayerPresenceRecord::new(false);
        record.state = PlayerWorldPresence::OfflineProtected;
        presence.players.insert(protected_player, record);

        let crops = Crops(HashMap::from([
            (
                protected_structure,
                Crop {
                    structure: protected_structure,
                    crop_type: "Protected Wheat".to_string(),
                    quantity: 2,
                    stage: CropStages::Seed,
                    stage_start: 0,
                    stage_end: 100,
                },
            ),
            (
                active_structure,
                Crop {
                    structure: active_structure,
                    crop_type: "Active Wheat".to_string(),
                    quantity: 2,
                    stage: CropStages::Seed,
                    stage_start: 0,
                    stage_end: 100,
                },
            ),
        ]));

        let mut app = App::new();
        app.insert_resource(GameTick(200))
            .insert_resource(Clients::default())
            .insert_resource(ids)
            .insert_resource(ActiveInfos(HashMap::new()))
            .insert_resource(crops)
            .insert_resource(presence)
            .add_systems(Update, crop_system);
        app.update();

        let crops = app.world().resource::<Crops>();
        let protected = crops
            .get(&protected_structure)
            .expect("protected crop remains");
        assert_eq!(protected.stage, CropStages::Seed);
        assert_eq!(protected.stage_start, 0);
        assert_eq!(protected.stage_end, 100);

        let active = crops.get(&active_structure).expect("active crop remains");
        assert_eq!(active.stage, CropStages::Sprout);
        assert_eq!(active.stage_start, 200);
        assert_eq!(active.stage_end, 400);
    }
}
