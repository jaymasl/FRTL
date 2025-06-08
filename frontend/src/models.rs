use serde::{Deserialize, Serialize};
use uuid::Uuid;
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Egg {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub summoned_by: Option<Uuid>,
    pub summoned_by_username: Option<String>,
    pub owner_username: Option<String>,
    pub essence: Option<String>,
    pub color: Option<String>,
    pub art_style: Option<String>,
    pub image_path: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub incubation_ends_at: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
pub struct Creature {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub original_egg_id: Option<Uuid>,
    pub original_egg_summoned_by: Option<Uuid>,
    #[serde(default)]
    pub original_egg_created_at: Option<String>,
    #[serde(default)]
    pub hatched_at: Option<String>,
    pub hatched_by: Option<Uuid>,
    pub egg_summoned_by_username: Option<String>,
    pub hatched_by_username: Option<String>,
    pub owner_username: Option<String>,
    pub essence: Option<String>,
    pub color: Option<String>,
    pub art_style: Option<String>,
    pub animal: Option<String>,
    pub rarity: Option<String>,
    pub energy_full: bool,
    pub energy_recharge_complete_at: Option<String>,
    pub streak: i32,
    pub soul: i32,
    pub image_path: Option<String>,
    pub display_name: Option<String>,
    pub prompt: Option<String>,
    pub stats: Option<Value>,
    pub original_egg_image_path: String,
    #[serde(default)]
    pub in_chaos_realm: bool,
    #[serde(default)]
    pub chaos_realm_entry_at: Option<String>,
    #[serde(default)]
    pub chaos_realm_reward_claimed: bool,
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "available".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ChaosRealmResponse {
    pub success: bool,
    pub error: Option<String>,
    pub new_balance: i32,
    #[serde(default)]
    pub reward_amount: i32,
}

#[derive(Debug, Deserialize)]
pub struct ChaosRealmStatusResponse {
    pub in_realm: bool,
    pub reward_claimed: bool,
    pub remaining_seconds: Option<i64>,
    pub investment_amount: i32,
    pub reward_amount: i32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Scroll {
    pub id: uuid::Uuid,
    pub owner_id: Option<uuid::Uuid>,
    pub created_at: String,
    pub display_name: String,
    pub image_path: Option<String>,
    pub description: Option<String>,
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DisplayItem {
    Egg(Egg),
    Creature(Creature),
}

// Add GlobalStats model
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct GlobalStats {
    pub scrolls_count: i64,
    pub eggs_count: i64,
    pub creatures_count: i64,
    pub total_soul: i64,
}

// Add ShowcaseCreatureData model
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ShowcaseCreatureData {
    pub id: Uuid,
    pub display_name: String,
    pub image_path: String,
    pub rarity: String,
    pub owner_username: String,
    pub hatched_at: String, // Assuming String for simplicity in frontend
}