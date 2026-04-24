#[derive(Debug, Clone)]
pub enum DatabaseEvent {
    AddScore {
        player_id: i32,
        hero_name: String,
        hero_rank: String,
        total_xp: i32,
        total_score: i32,
        score_survival: i32,
        score_progression: i32,
        score_wealth: i32,
        score_defense: i32,
        score_valor: i32,
        score_legacy: i32,
        days_survived: i32,
        highest_pressure_level: i32,
        waves_survived: i32,
        legendary_kills: i32,
        hideouts_cleared: i32,
        fate: String,
        crisis_tier: i32,
    },
}
