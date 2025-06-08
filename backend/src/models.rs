use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Egg {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub summoned_by: Option<Uuid>,
    pub summoned_by_username: Option<String>,
    pub owner_username: Option<String>,
    pub essence: String,
    pub color: String,
    pub art_style: String,
    pub image_path: String,
    pub display_name: String,
    pub prompt: Option<String>,
    pub created_at: String,
    pub incubation_ends_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Creature {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub original_egg_id: Option<Uuid>,
    pub original_egg_summoned_by: Option<Uuid>,
    pub hatched_by: Uuid,
    pub egg_summoned_by_username: Option<String>,
    pub hatched_by_username: Option<String>,
    pub owner_username: Option<String>,
    pub essence: String,
    pub color: String,
    pub art_style: String,
    pub animal: String,
    pub rarity: String,
    pub energy_full: bool,
    pub energy_recharge_complete_at: Option<String>,
    pub streak: i32,
    pub soul: i32,
    pub image_path: String,
    pub display_name: String,
    pub prompt: Option<String>,
    pub stats: serde_json::Value,
    pub original_egg_image_path: String,
    pub hatched_at: String,
    pub original_egg_created_at: String,
    pub in_chaos_realm: bool,
    pub chaos_realm_entry_at: Option<String>,
    pub chaos_realm_reward_claimed: bool,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ShowcaseCreature {
    pub id: Uuid,
    pub display_name: String,
    pub image_path: String,
    pub rarity: String,
    pub owner_username: String,
    pub hatched_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DisplayItem {
    Egg(Egg),
    Creature(Creature),
}