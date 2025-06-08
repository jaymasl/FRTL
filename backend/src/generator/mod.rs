pub mod generate_egg;
pub mod generate_creature;
pub mod prompts;
pub mod generate_code;

use serde::Serialize;
use uuid::Uuid;
use time::OffsetDateTime;
use serde_json::Value;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Egg {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub summoned_by: Uuid,
    pub essence: String,
    pub color: String,
    pub art_style: String,
    pub image_path: Option<String>,
    pub display_name: Option<String>,
    pub prompt: Option<String>,
    pub incubation_ends_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Creature {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub original_egg_id: Option<Uuid>,
    pub original_egg_summoned_by: Option<Uuid>,
    pub hatched_by: Uuid,
    pub egg_summoned_by_username: String,
    pub hatched_by_username: String,
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
    pub stats: Value,
    pub original_egg_image_path: String,
    pub hatched_at: String,
    pub original_egg_created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreatureResponse {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub original_egg_id: Option<Uuid>,
    pub original_egg_summoned_by: Option<Uuid>,
    pub hatched_by: Option<Uuid>,
    pub egg_summoned_by_username: String,
    pub hatched_by_username: String,
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
    pub hatched_at: Option<String>,
    pub original_egg_created_at: Option<String>,
}

impl Egg {
    pub fn into_response(self, summoned_by_username: String) -> EggResponse {
        EggResponse {
            id: self.id,
            owner_id: self.owner_id,
            summoned_by: self.summoned_by,
            summoned_by_username,
            essence: self.essence,
            color: self.color,
            art_style: self.art_style,
            image_path: self.image_path.unwrap_or_default(),
            display_name: self.display_name,
            prompt: self.prompt.unwrap_or_default(),
            incubation_ends_at: self.incubation_ends_at.format(&time::format_description::well_known::Rfc3339).unwrap(),
            created_at: self.created_at.format(&time::format_description::well_known::Rfc3339).unwrap(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EggResponse {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub summoned_by: Uuid,
    pub summoned_by_username: String,
    pub essence: String,
    pub color: String,
    pub art_style: String,
    pub image_path: String,
    pub display_name: Option<String>,
    pub prompt: String,
    pub incubation_ends_at: String,
    pub created_at: String,
}

impl Creature {
    pub fn into_response(self) -> CreatureResponse {
        CreatureResponse {
            id: self.id,
            owner_id: self.owner_id,
            original_egg_id: self.original_egg_id,
            original_egg_summoned_by: self.original_egg_summoned_by,
            hatched_by: Some(self.hatched_by),
            egg_summoned_by_username: self.egg_summoned_by_username,
            hatched_by_username: self.hatched_by_username,
            essence: Some(self.essence),
            color: Some(self.color),
            art_style: Some(self.art_style),
            animal: Some(self.animal),
            rarity: Some(self.rarity),
            energy_full: self.energy_full,
            energy_recharge_complete_at: self.energy_recharge_complete_at,
            streak: self.streak,
            soul: self.soul,
            image_path: Some(self.image_path),
            display_name: Some(self.display_name),
            prompt: self.prompt,
            stats: Some(self.stats),
            original_egg_image_path: self.original_egg_image_path,
            hatched_at: Some(self.hatched_at),
            original_egg_created_at: Some(self.original_egg_created_at),
        }
    }
}

pub use generate_egg::generate_egg;
pub use generate_creature::generate_creature;
pub use generate_code::membership_code_routes;