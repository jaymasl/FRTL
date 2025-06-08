use super::*;
use axum::{
    extract::{Path, State},
    Json,
    http::StatusCode,
    Extension,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::str::FromStr;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs::remove_file;
use tracing::error;
use rand::Rng;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "rarity_type", rename_all = "PascalCase")]
pub enum RarityType {
    Common, Uncommon, Rare, Epic, Legendary, Mythical,
}

impl FromStr for RarityType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Common" => Ok(Self::Common),
            "Uncommon" => Ok(Self::Uncommon),
            "Rare" => Ok(Self::Rare),
            "Epic" => Ok(Self::Epic),
            "Legendary" => Ok(Self::Legendary),
            "Mythical" => Ok(Self::Mythical),
            _ => Err(format!("Invalid rarity: {}", s)),
        }
    }
}

impl std::fmt::Display for RarityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Common => write!(f, "Common"),
            Self::Uncommon => write!(f, "Uncommon"),
            Self::Rare => write!(f, "Rare"),
            Self::Epic => write!(f, "Epic"),
            Self::Legendary => write!(f, "Legendary"),
            Self::Mythical => write!(f, "Mythical"),
        }
    }
}

impl RarityType {
    fn next_tier(&self) -> Option<Self> {
        match self {
            Self::Common => Some(Self::Uncommon),
            Self::Uncommon => Some(Self::Rare),
            Self::Rare => Some(Self::Epic),
            Self::Epic => Some(Self::Legendary),
            Self::Legendary => Some(Self::Mythical),
            Self::Mythical => None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BindRequest {
    pub sacrifice_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct BindResponse {
    pub success: bool,
    pub creature: Option<CreatureResponse>,
    pub error: Option<String>,
    pub new_balance: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct CreatureResponse {
    pub id: Uuid,
    pub rarity: String,
    pub display_name: String,
    pub energy_full: bool,
    pub stats: serde_json::Value,
}

#[axum::debug_handler]
pub async fn bind_creature(
    state: State<AppState>,
    user_id: Extension<UserId>,
    target_id: Path<Uuid>,
    request: Json<BindRequest>,
) -> impl IntoResponse {
    let result = bind_creature_inner(state.0, user_id.0, target_id.0, request.0).await;
    match result {
        Ok(response) => response,
        Err(response) => response,
    }
}

async fn bind_creature_inner(
    state: AppState,
    user_id: UserId,
    target_id: Uuid,
    request: BindRequest,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Transaction error: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?;

    // Check currency balance
    let user = sqlx::query!(
        "SELECT currency_balance FROM users WHERE id = $1 FOR UPDATE",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch user data: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?;

    if user.currency_balance < 55 {
        tx.commit().await.ok();
        return Ok((StatusCode::PAYMENT_REQUIRED, Json(json!({
            "success": false,
            "error": "Not enough Pax (requires 55)",
            "creature": null,
            "new_balance": user.currency_balance
        }))));
    }

    let target = sqlx::query!(
        r#"
        SELECT id, owner_id, rarity::text as "rarity!", essence::text as "essence!", 
               display_name, energy_full, stats, status::text as "status!",
               animal::text as "animal!"
        FROM creatures
        WHERE id = $1 AND owner_id = $2
        FOR UPDATE
        "#,
        target_id,
        user_id.0
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch target: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?;

    let sacrifice = sqlx::query!(
        r#"
        SELECT id, owner_id, rarity::text as "rarity!", essence::text as "essence!",
               image_path as "image_path!", original_egg_image_path as "original_egg_image_path!",
               status::text as "status!"
        FROM creatures
        WHERE id = $1 AND owner_id = $2
        FOR UPDATE
        "#,
        request.sacrifice_id,
        user_id.0
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch sacrifice: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?;

    let (target, sacrifice) = match (target, sacrifice) {
        (Some(t), Some(s)) => (t, s),
        _ => {
            return Ok((StatusCode::OK, Json(json!({
                "success": false,
                "error": "Creatures not found or don't belong to you",
                "creature": null,
                "new_balance": user.currency_balance
            }))))
        }
    };

    // Check if either creature is locked (listed on market)
    if target.status == "locked" || sacrifice.status == "locked" {
        return Ok((StatusCode::OK, Json(json!({
            "success": false,
            "error": "Cannot soul bind while creatures are listed on the market",
            "creature": null,
            "new_balance": user.currency_balance
        }))))
    }

    // Check if target has full energy
    if !target.energy_full {
        return Ok((StatusCode::OK, Json(json!({
            "success": false,
            "error": "Target creature must have full energy to soul bind",
            "creature": null,
            "new_balance": user.currency_balance
        }))))
    }

    let target_rarity = RarityType::from_str(&target.rarity)
        .map_err(|_| {
            (StatusCode::OK, Json(json!({
                "success": false,
                "error": "Invalid target rarity",
                "creature": null,
                "new_balance": user.currency_balance
            })))
        })?;

    let sacrifice_rarity = RarityType::from_str(&sacrifice.rarity)
        .map_err(|_| {
            (StatusCode::OK, Json(json!({
                "success": false,
                "error": "Invalid sacrifice rarity",
                "creature": null,
                "new_balance": user.currency_balance
            })))
        })?;

    if target_rarity != sacrifice_rarity || target.essence != sacrifice.essence {
        return Ok((StatusCode::OK, Json(json!({
            "success": false,
            "error": "Creatures must be of the same rarity and essence type",
            "creature": null,
            "new_balance": user.currency_balance
        }))))
    }

    let new_rarity = match target_rarity.next_tier() {
        Some(r) => r,
        None => {
            return Ok((StatusCode::OK, Json(json!({
                "success": false,
                "error": "Creature is already at maximum rarity",
                "creature": null,
                "new_balance": user.currency_balance
            }))))
        }
    };

    // Delete sacrifice creature's image
    let creature_image_path = PathBuf::from(".")
        .join(sacrifice.image_path.trim_start_matches('/'));
    if let Err(e) = remove_file(&creature_image_path).await {
        error!("Failed to delete creature image: {}", e);
    }

    // Delete original egg image
    let egg_image_path = PathBuf::from(".")
        .join(sacrifice.original_egg_image_path.trim_start_matches('/'));
    if let Err(e) = remove_file(&egg_image_path).await {
        error!("Failed to delete egg image: {}", e);
    }

    // Get current stats and prepare for update
    let mut updated_stats = target.stats.as_object().unwrap_or(&serde_json::Map::new()).clone();
    
    {
        // Only use RNG within this block so it doesn't cross an await boundary
        let mut rng = rand::thread_rng();
        // Randomly increase each stat by 1-5
        for stat in ["health", "attack", "speed"].iter() {
            if let Some(value) = updated_stats.get_mut(*stat) {
                if let Some(current) = value.as_f64() {
                    let increase = rng.gen_range(1..=5) as f64;
                    *value = json!(current + increase);
                }
            }
        }
    } // rng goes out of scope here before any awaits

    // Update the target creature
    sqlx::query!(
        r#"
        UPDATE creatures
        SET rarity = $1::text::rarity_type,
            energy_full = false,
            energy_recharge_complete_at = NULL,
            soul = soul + 5,
            stats = $3
        WHERE id = $2
        "#,
        new_rarity.to_string(),
        target.id,
        serde_json::Value::Object(updated_stats)
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update target: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?;

    // Delete the sacrifice creature
    sqlx::query!(
        "DELETE FROM creatures WHERE id = $1",
        sacrifice.id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to delete sacrifice: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?;

    // Get the updated creature
    let updated = sqlx::query!(
        r#"
        SELECT id, rarity::text as "rarity!", display_name, energy_full, stats, streak, soul
        FROM creatures
        WHERE id = $1
        "#,
        target.id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch updated creature: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?;

    // Only deduct currency after all operations have succeeded
    let new_balance = user.currency_balance - 55;
    sqlx::query!(
        "UPDATE users SET currency_balance = $1 WHERE id = $2",
        new_balance,
        user_id.0
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update balance: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?;

    // Update experience and rank as part of the transaction
    let user_record = sqlx::query!(
        "SELECT experience, rank::text as \"rank!\" FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch user experience: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": "Database error",
                "creature": null,
                "new_balance": null
            })),
        )
    })?;

    let new_experience = user_record.experience + 10;
    let new_rank = user_service::compute_rank(new_experience);

    // Update both experience and rank if rank changed using a runtime query to handle custom type casting.
    if user_record.rank != new_rank {
        let new_rank_str = new_rank.to_string();
        sqlx::query("UPDATE users SET experience = $1, rank = $2::user_rank WHERE id = $3")
            .bind(new_experience)
            .bind(new_rank_str)
            .bind(user_id.0)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to update experience and rank: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": "Database error",
                        "creature": null,
                        "new_balance": null
                    })),
                )
            })?;
    } else {
        sqlx::query!("UPDATE users SET experience = $1 WHERE id = $2", new_experience, user_id.0)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to update experience: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": "Database error",
                        "creature": null,
                        "new_balance": null
                    })),
                )
            })?;
    }

    // Commit the transaction
    tx.commit().await.map_err(|e| {
        error!("Failed to commit: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?;

    // Get usernames for logging
    let username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch username: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "success": false,
            "error": "Database error",
            "creature": null,
            "new_balance": null
        })))
    })?.username;

    info!("🔮 Soul bind successful: {} upgraded a {} {} {} to {} (soul: {})", 
        username, target.rarity, target.essence, target.animal, new_rarity, updated.soul);

    // Final success response
    Ok((StatusCode::OK, Json(json!({
        "success": true,
        "creature": {
            "id": updated.id,
            "rarity": updated.rarity,
            "display_name": updated.display_name,
            "energy_full": updated.energy_full,
            "stats": updated.stats,
            "streak": updated.streak,
            "soul": updated.soul,
        },
        "new_balance": new_balance,
        "error": null
    }))))
}