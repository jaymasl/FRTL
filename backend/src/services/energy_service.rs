use axum::http::StatusCode;
use redis::Client as RedisClient;
use serde::Serialize;
use sqlx::PgPool;
use axum::extract::{Extension, Path};
use axum::Json;
use time::OffsetDateTime;
use sqlx::Acquire;
use uuid::Uuid;
use crate::auth::middleware::UserId;

#[derive(Debug, Serialize)]
pub struct EnergyResponse {
    pub success: bool,
    pub error: Option<String>,
    pub creature_id: Uuid,
    pub energy_full: bool,
    pub pax_balance: i64,
    pub energy_recharge_complete_at: Option<String>,
}

// Energy recharge cost based on rarity
fn get_energy_recharge_cost(rarity: &str) -> i32 {
    match rarity {
        "Rare" => 10,
        "Epic" => 20,
        "Legendary" => 30,
        "Mythical" => 40,
        _ => 5 // Default for Uncommon and others
    }
}

// Energy recharge time in seconds
const ENERGY_RECHARGE_TIME_SECONDS: i64 = 21600; // Increased from 60 seconds to 6 hours (21600 seconds)

#[axum::debug_handler]
pub async fn handle_energy_recharge(
    Path(creature_id): Path<Uuid>,
    Extension(pool): Extension<PgPool>,
    Extension(redis_pool): Extension<RedisClient>,
    Extension(UserId(user_id)): Extension<UserId>,
) -> Result<Json<EnergyResponse>, StatusCode> {
    log::info!("handle_energy_recharge called with creature_id: {} and user_id: {}", creature_id, user_id);

    // Get a connection from the pool
    let mut conn = pool.acquire().await.map_err(|e| {
        log::error!("Failed to get DB connection: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let _redis_conn = redis_pool.get_async_connection().await.map_err(|e| {
        log::error!("Failed to get redis connection: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Start a transaction
    let mut tx = conn.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get the creature and check ownership; cast status to text
    let creature = sqlx::query!(
        r#"
        SELECT c.id, c.owner_id, c.status::text as "status!", c.in_chaos_realm, c.energy_full, 
               c.energy_recharge_complete_at, c.rarity::text as "rarity!"
        FROM creatures c
        WHERE c.id = $1
        "#,
        creature_id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch creature: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    log::info!("Found creature: {:?}", creature);
    log::info!("Creature status from DB: {}", creature.status);

    // Check ownership
    if creature.owner_id != user_id {
        log::error!("Ownership mismatch: creature owner {} != user {}", creature.owner_id, user_id);
        return Err(StatusCode::NOT_FOUND);
    }

    // Check if creature is in chaos realm
    if creature.in_chaos_realm {
        log::error!("Creature {} is in chaos realm", creature_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Ensure creature is available (i.e., not listed/locked)
    if creature.status != "available" {
        log::error!("Creature {} is not available for energy recharge (status: {})", creature_id, creature.status);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if energy is already full
    if creature.energy_full {
        log::error!("Creature {} energy is already full", creature_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if energy is currently recharging
    if let Some(recharge_time) = creature.energy_recharge_complete_at {
        if recharge_time > OffsetDateTime::now_utc() {
            log::error!("Creature {} is still recharging until {}", creature_id, recharge_time);
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Get energy cost based on rarity
    let energy_cost = get_energy_recharge_cost(&creature.rarity);

    // Get user's pax balance (using currency_balance column)
    let user = sqlx::query!(
        r#"
        SELECT currency_balance as pax
        FROM users
        WHERE id = $1
        "#,
        user_id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to fetch user balance: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    log::info!("User {} has {} pax", user_id, user.pax);

    // Check if user has enough pax
    if user.pax < energy_cost {
        return Ok(Json(EnergyResponse {
            success: false,
            error: Some(format!("Not enough currency. Requires {} pax.", energy_cost)),
            creature_id,
            energy_full: creature.energy_full,
            pax_balance: user.pax as i64,
            energy_recharge_complete_at: creature.energy_recharge_complete_at.map(|dt| dt.to_string()),
        }));
    }

    // Set the recharge completion time
    let recharge_complete_at = (OffsetDateTime::now_utc() + time::Duration::seconds(ENERGY_RECHARGE_TIME_SECONDS))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let recharge_complete_at_parsed = OffsetDateTime::now_utc() + time::Duration::seconds(ENERGY_RECHARGE_TIME_SECONDS);

    // Update the creature's energy status and user's pax balance
    let update_result = sqlx::query!(
        r#"UPDATE creatures
        SET energy_full = false,
            energy_recharge_complete_at = $1,
            soul = soul + 1  -- Increment soul by 1 when recharging energy
        WHERE id = $2 AND status = 'available'"#,
        recharge_complete_at_parsed,
        creature_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if update_result.rows_affected() != 1 {
        log::error!("Failed to update creature energy because creature {} is not available (status may be locked)", creature_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    let updated_user = sqlx::query!(
        r#"
        UPDATE users
        SET currency_balance = currency_balance - $1
        WHERE id = $2
        RETURNING currency_balance as "pax!: i32"
        "#,
        energy_cost,
        user_id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        log::error!("Failed to update user balance: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Commit the transaction
    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Clone the pool for the background task
    let pool_clone = pool.clone();
    let creature_id_clone = creature_id;
    
    // Spawn a background task to update creature energy status to full after recharge time expires
    tokio::spawn(async move {
        // Sleep for the recharge time
        tokio::time::sleep(std::time::Duration::from_secs(ENERGY_RECHARGE_TIME_SECONDS as u64)).await;

        // After waiting, update the creature: set energy_full = true and clear the recharge time
        match pool_clone.acquire().await {
            Ok(mut conn) => {
                if let Err(e) = sqlx::query!(
                    r#"
                    UPDATE creatures
                    SET energy_full = true,
                        energy_recharge_complete_at = NULL
                    WHERE id = $1
                    "#,
                    creature_id_clone
                )
                .execute(&mut *conn)
                .await
                {
                    log::error!("Failed to update creature energy to full: {:?}", e);
                } else {
                    log::info!("Creature {} energy updated to full.", creature_id_clone);
                }
            },
            Err(e) => {
                log::error!("Failed to acquire DB connection for post-recharge update: {:?}", e);
            }
        }
    });

    Ok(Json(EnergyResponse {
        success: true,
        error: None,
        creature_id,
        energy_full: false,
        pax_balance: updated_user.pax as i64,
        energy_recharge_complete_at: Some(recharge_complete_at),
    }))
}

/// Checks for creatures with expired energy recharge times and updates them to have full energy.
/// This function is meant to be called periodically to recover from situations where the
/// background task that updates energy status after recharge time expires fails (e.g., due to server restart).
pub async fn check_expired_energy_recharges(pool: &PgPool) -> Result<(), sqlx::Error> {
    log::info!("Checking for creatures with expired energy recharge times...");
    
    let now = OffsetDateTime::now_utc();
    
    // Find creatures with energy_full = false and energy_recharge_complete_at in the past
    let expired_creatures = sqlx::query!(
        r#"
        SELECT id 
        FROM creatures 
        WHERE energy_full = false 
        AND energy_recharge_complete_at IS NOT NULL 
        AND energy_recharge_complete_at < $1
        "#,
        now
    )
    .fetch_all(pool)
    .await?;
    
    let count = expired_creatures.len();
    if count > 0 {
        log::info!("Found {} creatures with expired energy recharge times", count);
        
        // Update all expired creatures to have full energy
        let mut conn = pool.acquire().await?;
        let result = sqlx::query!(
            r#"
            UPDATE creatures
            SET energy_full = true,
                energy_recharge_complete_at = NULL
            WHERE energy_full = false 
            AND energy_recharge_complete_at IS NOT NULL 
            AND energy_recharge_complete_at < $1
            "#,
            now
        )
        .execute(&mut *conn)
        .await?;
        
        log::info!("Updated {} creatures to have full energy", result.rows_affected());
    } else {
        log::info!("No creatures with expired energy recharge times found");
    }
    
    Ok(())
} 