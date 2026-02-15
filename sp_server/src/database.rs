
#[derive(Debug, Clone)]
pub enum DatabaseEvent {
    AddScore {
        player_id: i32,
        hero_name: String,
        hero_rank: String,
        total_xp: i32,
        fate: String,
    },
}
