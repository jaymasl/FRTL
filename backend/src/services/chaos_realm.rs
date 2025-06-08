use axum::{
    extract::{Path, State, Extension},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use time::OffsetDateTime;
use tracing::{error, info};
use uuid::Uuid;

use crate::auth::middleware::UserId;
use crate::AppState;

const CHAOS_REALM_DURATION_SECS: i64 = 82800;  // 23 hours (was 10 seconds)

#[derive(Debug, Serialize)]
pub struct ChaosRealmResponse {
    pub success: bool,
    pub error: Option<String>,
    pub new_balance: i32,
    pub reward_amount: i32,
}

#[derive(Debug, Serialize)]
pub struct ChaosRealmStatusResponse {
    pub in_realm: bool,
    pub reward_claimed: bool,
    pub remaining_seconds: Option<i64>,
    pub investment_amount: i32,
    pub reward_amount: i32,
}

// Investment and reward amounts based on rarity
fn get_chaos_realm_amounts(rarity: &str) -> (i32, i32) {
    match rarity {
        "Uncommon" => (0, 8),    // Free entry, +8 pax reward (profit: +3 pax)
        "Rare" => (0, 18),       // Free entry, +18 pax reward (profit: +8 pax, +2 pax better than 2 Uncommons)
        "Epic" => (0, 38),       // Free entry, +38 pax reward (profit: +18 pax, +2 pax better than 2 Rares)
        "Legendary" => (0, 68),  // Free entry, +68 pax reward (profit: +38 pax, +2 pax better than 2 Epics)
        "Mythical" => (0, 118),  // Free entry, +118 pax reward (profit: +78 pax, +2 pax better than 2 Legendaries)
        _ => (0, 0),             // Common and invalid rarities
    }
}

pub async fn enter_chaos_realm(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(creature_id): Path<Uuid>,
) -> Result<Json<ChaosRealmResponse>, StatusCode> {
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Transaction error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get and verify creature state
    let creature = sqlx::query!(
        r#"
        SELECT 
            c.in_chaos_realm,
            c.energy_full,
            c.rarity::text as "rarity!",
            c.status::text as "status!",
            u.currency_balance,
            u.username
        FROM creatures c
        JOIN users u ON c.owner_id = u.id
        WHERE c.id = $1 AND c.owner_id = $2
        FOR UPDATE
        "#,
        creature_id,
        user_id.0
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch creature: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Check if creature is locked (listed on market)
    if creature.status == "locked" {
        info!("🚫 Chaos Realm entry failed for {} - Creature is listed on market", creature.username);
        return Ok(Json(ChaosRealmResponse {
            success: false,
            error: Some("Cannot enter Chaos Realm while creature is listed on the market".to_string()),
            new_balance: creature.currency_balance,
            reward_amount: 0,
        }));
    }

    // Check energy requirement
    if !creature.energy_full {
        info!("🚫 Chaos Realm entry failed for {} - Insufficient energy", creature.username);
        return Ok(Json(ChaosRealmResponse {
            success: false,
            error: Some("Requires full energy to enter Chaos Realm".to_string()),
            new_balance: creature.currency_balance,
            reward_amount: 0,
        }));
    }

    if creature.in_chaos_realm {
        info!("🚫 Chaos Realm entry failed for {} - Already in realm", creature.username);
        return Ok(Json(ChaosRealmResponse {
            success: false,
            error: Some("Creature is already in the Chaos Realm".to_string()),
            new_balance: creature.currency_balance,
            reward_amount: 0,
        }));
    }

    // Update creature state and reset energy
    sqlx::query!(
        r#"
        UPDATE creatures 
        SET 
            in_chaos_realm = true,
            chaos_realm_entry_at = CURRENT_TIMESTAMP,
            chaos_realm_reward_claimed = false,
            energy_full = false,
            energy_recharge_complete_at = NULL,
            soul = soul + 1
        WHERE id = $1
        "#,
        creature_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update creature: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ChaosRealmResponse {
        success: true,
        error: None,
        new_balance: creature.currency_balance,
        reward_amount: 0,
    }))
}

pub async fn claim_chaos_realm_reward(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(creature_id): Path<Uuid>,
) -> Result<Json<ChaosRealmResponse>, StatusCode> {
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Transaction error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get and verify creature state
    let creature = sqlx::query!(
        r#"
        SELECT 
            c.in_chaos_realm,
            c.chaos_realm_entry_at,
            c.chaos_realm_reward_claimed,
            c.rarity::text as "rarity!",
            c.status::text as "status!",
            u.currency_balance,
            u.username
        FROM creatures c
        JOIN users u ON c.owner_id = u.id
        WHERE c.id = $1 AND c.owner_id = $2
        FOR UPDATE
        "#,
        creature_id,
        user_id.0
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch creature: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Check if creature is available (not listed on the market)
    if creature.status != "available" {
        info!("🚫 Chaos Realm claim failed for {} - Creature is listed on market", creature.username);
        return Ok(Json(ChaosRealmResponse {
            success: false,
            error: Some("Cannot claim reward while creature is listed on the market".to_string()),
            new_balance: creature.currency_balance,
            reward_amount: 0,
        }));
    }

    if !creature.in_chaos_realm {
        info!("🚫 Chaos Realm claim failed for {} - Not in realm", creature.username);
        return Ok(Json(ChaosRealmResponse {
            success: false,
            error: Some("Creature is not in the Chaos Realm".to_string()),
            new_balance: creature.currency_balance,
            reward_amount: 0,
        }));
    }

    if creature.chaos_realm_reward_claimed {
        info!("🚫 Chaos Realm claim failed for {} - Already claimed", creature.username);
        return Ok(Json(ChaosRealmResponse {
            success: false,
            error: Some("Reward has already been claimed".to_string()),
            new_balance: creature.currency_balance,
            reward_amount: 0,
        }));
    }

    let entry_time = creature.chaos_realm_entry_at.ok_or_else(|| {
        error!("Chaos realm entry time is null for creature in realm");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let now = OffsetDateTime::now_utc();
    let elapsed = now - entry_time;
    if elapsed.whole_seconds() < CHAOS_REALM_DURATION_SECS {
        info!("🚫 Chaos Realm claim failed for {} - Time remaining: {}s", creature.username, CHAOS_REALM_DURATION_SECS - elapsed.whole_seconds());
        return Ok(Json(ChaosRealmResponse {
            success: false,
            error: Some(format!(
                "Must wait {} more seconds",
                CHAOS_REALM_DURATION_SECS - elapsed.whole_seconds()
            )),
            new_balance: creature.currency_balance,
            reward_amount: 0,
        }));
    }

    let (_, reward) = get_chaos_realm_amounts(&creature.rarity);
    let new_balance = creature.currency_balance + reward;

    // Update creature and user state
    sqlx::query!(
        r#"
        UPDATE creatures 
        SET 
            in_chaos_realm = false,
            chaos_realm_reward_claimed = true,
            soul = soul + 1
        WHERE id = $1
        "#,
        creature_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update creature: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    sqlx::query!(
        "UPDATE users SET currency_balance = $1 WHERE id = $2",
        new_balance,
        user_id.0
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update balance: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ChaosRealmResponse {
        success: true,
        error: None,
        new_balance: new_balance,
        reward_amount: reward,
    }))
}

pub async fn get_chaos_realm_status(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(creature_id): Path<Uuid>,
) -> Result<Json<ChaosRealmStatusResponse>, StatusCode> {
    let status = sqlx::query!(
        r#"
        SELECT 
            c.in_chaos_realm,
            c.chaos_realm_entry_at,
            c.chaos_realm_reward_claimed,
            c.rarity::text as "rarity!"
        FROM creatures c
        WHERE c.id = $1 AND c.owner_id = $2
        "#,
        creature_id,
        user_id.0
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch creature status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let (investment, reward) = get_chaos_realm_amounts(&status.rarity);
    let remaining_time = if status.in_chaos_realm {
        if let Some(entry_time) = status.chaos_realm_entry_at {
            let now = OffsetDateTime::now_utc();
            let elapsed = now - entry_time;
            let remaining = CHAOS_REALM_DURATION_SECS - elapsed.whole_seconds();
            Some(remaining.max(0))
        } else {
            Some(CHAOS_REALM_DURATION_SECS)  // Default to full duration if entry time missing
        }
    } else {
        None
    };

    Ok(Json(ChaosRealmStatusResponse {
        in_realm: status.in_chaos_realm,
        reward_claimed: status.chaos_realm_reward_claimed,
        remaining_seconds: remaining_time,
        investment_amount: investment,
        reward_amount: reward,
    }))
} 