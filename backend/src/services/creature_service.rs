use super::*;
use axum::response::Response;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use tracing::{error, info};
use axum::{
    extract::{State, Path, Json, Extension},
    http::StatusCode,
    response::IntoResponse,
};
use crate::AppState;
use crate::auth::middleware::UserId;
use crate::generator::generate_code::is_member;
use shared::profanity::ProfanityFilter;
use crate::models::ShowcaseCreature;

#[derive(Debug, Serialize, sqlx::FromRow)]
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

#[derive(Debug, Serialize, sqlx::FromRow)]
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
    pub prompt: String,
    pub created_at: String,
    pub incubation_ends_at: String,
}

pub async fn get_user_creatures(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Response<Body>, Response<Body>> {
    let creatures = match sqlx::query_as!(
        Creature,
        r#"
        SELECT 
            c.id,
            c.owner_id,
            c.original_egg_id,
            c.original_egg_summoned_by,
            c.hatched_by,
            u1.username as "egg_summoned_by_username",
            u2.username as "hatched_by_username",
            u3.username as "owner_username",
            c.essence::text as "essence!",
            c.color::text as "color!",
            c.art_style::text as "art_style!",
            c.animal::text as "animal!",
            c.rarity::text as "rarity!",
            c.energy_full,
            c.energy_recharge_complete_at::text as "energy_recharge_complete_at",
            c.streak,
            c.soul,
            c.image_path as "image_path!",
            c.display_name as "display_name!",
            c.prompt,
            c.stats,
            c.original_egg_image_path as "original_egg_image_path!",
            c.hatched_at::text as "hatched_at!",
            c.original_egg_created_at::text as "original_egg_created_at!",
            c.in_chaos_realm,
            c.chaos_realm_entry_at::text as "chaos_realm_entry_at",
            c.chaos_realm_reward_claimed
        FROM creatures c
        LEFT JOIN users u1 ON c.original_egg_summoned_by = u1.id
        LEFT JOIN users u2 ON c.hatched_by = u2.id
        LEFT JOIN users u3 ON c.owner_id = u3.id
        WHERE c.owner_id = $1
        AND c.status = 'available'
        ORDER BY c.hatched_at DESC
        "#,
        user_id.0
    )
    .fetch_all(&state.pool)
    .await {
        Ok(creatures) => creatures,
        Err(e) => {
            error!("Failed to fetch creatures: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to fetch creatures"))
                .unwrap());
        }
    };

    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "http://127.0.0.1:8080")
        .header("Access-Control-Allow-Credentials", "true")
        .body(Body::from(serde_json::to_string(&creatures).unwrap()))
        .unwrap())
}

pub async fn get_user_eggs(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Response<Body>, Response<Body>> {
    match sqlx::query!(
        r#"
        SELECT 
            e.id,
            e.owner_id,
            e.summoned_by,
            u_summoner.username as "summoned_by_username",
            u_owner.username as "owner_username",
            e.essence::text as "essence!",
            e.color::text as "color!",
            e.art_style::text as "art_style!",
            e.image_path as "image_path!",
            e.display_name as "display_name!",
            e.prompt as "prompt!",
            TO_CHAR(e.created_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') as "created_at_str!",
            e.incubation_ends_at,
            TO_CHAR(e.incubation_ends_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') as "incubation_ends_at_str!"
        FROM eggs e
        LEFT JOIN users u_summoner ON e.summoned_by = u_summoner.id
        LEFT JOIN users u_owner ON e.owner_id = u_owner.id
        WHERE e.owner_id = $1
        AND e.status = 'available'
        ORDER BY e.created_at DESC
        "#,
        user_id.0
    )
    .fetch_all(&state.pool)
    .await
    .map(|eggs| {
        eggs.into_iter()
            .map(|egg| Egg {
                id: egg.id,
                owner_id: egg.owner_id,
                summoned_by: Some(egg.summoned_by),
                summoned_by_username: Some(egg.summoned_by_username),
                owner_username: Some(egg.owner_username),
                essence: egg.essence,
                color: egg.color,
                art_style: egg.art_style,
                image_path: egg.image_path,
                display_name: egg.display_name,
                prompt: egg.prompt,
                created_at: egg.created_at_str,
                incubation_ends_at: egg.incubation_ends_at_str,
            })
            .collect::<Vec<_>>()
    }) {
        Ok(eggs) => Ok(Response::builder()
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&eggs).unwrap()))
            .unwrap()),
        Err(e) => {
            error!("Failed to fetch eggs: {}", e);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to fetch eggs"))
                .unwrap())
        }
    }
}

// New struct for rename request
#[derive(Debug, Deserialize)]
pub struct RenameCreatureRequest {
    pub new_name: String,
}

// New handler for renaming creatures
pub async fn rename_creature_handler(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(creature_id): Path<Uuid>,
    Json(payload): Json<RenameCreatureRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Check if user is a member
    let is_member = is_member(&state.pool, user_id.0).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    if !is_member {
        return Err((StatusCode::FORBIDDEN, "This feature requires membership".to_string()));
    }
    
    // Verify creature ownership
    let creature = sqlx::query!(
        "SELECT owner_id FROM creatures WHERE id = $1",
        creature_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    let creature = match creature {
        Some(c) => c,
        None => return Err((StatusCode::NOT_FOUND, "Creature not found".to_string())),
    };
    
    if creature.owner_id != user_id.0 {
        return Err((StatusCode::FORBIDDEN, "You don't own this creature".to_string()));
    }
    
    // Validate new name (not empty, reasonable length)
    let new_name = payload.new_name.trim();
    if new_name.is_empty() || new_name.len() > 20 {
        return Err((StatusCode::BAD_REQUEST, "Invalid name length".to_string()));
    }

    // Check for profanity
    if let Err(msg) = ProfanityFilter::validate_username(new_name) {
        return Err((StatusCode::BAD_REQUEST, msg));
    }
    
    // Start transaction
    let mut tx = state.pool.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Check user's currency balance
    let user = sqlx::query!(
        "SELECT currency_balance FROM users WHERE id = $1 FOR UPDATE",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Check if user has enough currency (100 pax)
    const RENAME_COST: i32 = 100;
    if user.currency_balance < RENAME_COST {
        return Err((StatusCode::BAD_REQUEST, "Insufficient funds".to_string()));
    }
    
    // Deduct currency
    let new_balance = sqlx::query!(
        "UPDATE users SET currency_balance = currency_balance - $1 WHERE id = $2 RETURNING currency_balance",
        RENAME_COST,
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .currency_balance;
    
    // Update creature name
    sqlx::query!(
        "UPDATE creatures SET display_name = $1 WHERE id = $2",
        new_name,
        creature_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Commit transaction
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Get username for logging
    let username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .username;
    
    // Log the rename operation
    info!("🏷️ User {} renamed creature {} to '{}' for {} pax", 
          username, creature_id, new_name, RENAME_COST);
    
    // Return success response with new balance
    Ok((StatusCode::OK, Json(serde_json::json!({
        "success": true,
        "new_balance": new_balance,
        "message": format!("Successfully renamed creature to '{}'", new_name)
    }))))
}

/// Fetches creatures for the public showcase
/// Returns a list of the top N rarest, available creatures.
pub async fn get_public_showcase_creatures(
    State(state): State<AppState>,
) -> Result<Json<Vec<ShowcaseCreature>>, (StatusCode, String)> {
    const SHOWCASE_LIMIT: i64 = 24;

    let creatures = sqlx::query_as!(
        ShowcaseCreature,
        r#"
        SELECT 
            c.id,
            c.display_name,
            c.image_path,
            c.rarity::text as "rarity!",
            u.username as "owner_username!",
            c.hatched_at::text as "hatched_at!"
        FROM creatures c
        JOIN users u ON c.owner_id = u.id
        WHERE c.rarity IN ('Uncommon', 'Rare', 'Epic', 'Legendary', 'Mythical')
        AND c.status = 'available'
        ORDER BY 
            CASE c.rarity
                WHEN 'Mythical' THEN 5
                WHEN 'Legendary' THEN 4
                WHEN 'Epic' THEN 3
                WHEN 'Rare' THEN 2
                WHEN 'Uncommon' THEN 1
                ELSE 0
            END DESC,
            c.hatched_at DESC
        LIMIT $1
        "#,
        SHOWCASE_LIMIT
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch showcase creatures: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch showcase creatures".to_string())
    })?;

    Ok(Json(creatures))
}