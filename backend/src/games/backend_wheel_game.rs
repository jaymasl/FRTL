use axum::{
    routing::post,
    Router,
    Json,
    extract::{State, Extension},
    debug_handler,
};
use rand::Rng;
use rand::rngs::OsRng;
use shared::shared_wheel_game::*;
use crate::AppState;
use crate::auth::middleware::UserId;
use uuid::Uuid;
use time::OffsetDateTime;
use serde_json;

// Define the cooldown period in seconds
const WHEEL_SPIN_COOLDOWN: u64 = 82800; // 23 hours cooldown

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/spin", post(spin_wheel))
        .route("/cooldown", axum::routing::get(get_wheel_cooldown))
        .layer(axum::middleware::from_fn(crate::auth::middleware::require_auth))
}

#[debug_handler]
async fn spin_wheel(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(_request): Json<WheelSpinRequest>,
) -> Result<Json<WheelSpinResponse>, (axum::http::StatusCode, String)> {
    // Check if user is a member
    match crate::generator::generate_code::is_member(&state.pool, user_id.0).await {
        Ok(is_member) => {
            if !is_member {
                // Get the user's current balance for the response
                let user = sqlx::query!(
                    "SELECT currency_balance FROM users WHERE id = $1",
                    user_id.0
                )
                .fetch_one(&state.pool)
                .await
                .map_err(|e| {
                    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
                })?;
                
                return Ok(Json(WheelSpinResponse {
                    success: false,
                    is_win: false,
                    new_balance: user.currency_balance,
                    message: Some("This feature is only available to members. Please activate a membership code to continue.".to_string()),
                    result_number: None,
                }));
            }
        },
        Err(e) => {
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR, 
                format!("Database error checking membership: {}", e)
            ));
        }
    }

    // Check cooldown in Redis
    let cooldown_key = format!("wheel_spin_cooldown:{}", user_id.0);
    let mut redis_conn = state.redis.get_async_connection().await.map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Redis error: {}", e))
    })?;
    
    // Check if the cooldown key exists
    let cooldown_exists: bool = redis::cmd("EXISTS")
        .arg(&cooldown_key)
        .query_async(&mut redis_conn)
        .await
        .map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Redis error: {}", e))
        })?;
    
    if cooldown_exists {
        // Get the remaining time on the cooldown
        let ttl: i64 = redis::cmd("TTL")
            .arg(&cooldown_key)
            .query_async(&mut redis_conn)
            .await
            .map_err(|e| {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Redis error: {}", e))
            })?;
        
        // Get the user's current balance
        let user = sqlx::query!(
            "SELECT currency_balance FROM users WHERE id = $1",
            user_id.0
        )
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
        })?;
        
        return Ok(Json(WheelSpinResponse {
            success: false,
            is_win: false,
            new_balance: user.currency_balance,
            message: Some(format!("Please wait {} seconds before spinning again.", ttl)),
            result_number: None,
        }));
    }

    // Start a transaction
    let mut tx = state.pool.begin().await.map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    // Get current balance - no need to check for sufficient balance since it's free
    let user = sqlx::query!(
        "SELECT currency_balance FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    // No cost to spin, so we start with the current balance
    let mut new_balance = user.currency_balance;

    // Generate a random number between 0 and 100
    let mut rng = OsRng;
    let result_number = rng.gen_range(0.0..100.0);
    
    // Determine the outcome based on the result number:
    // 0-35: 10 pax (TinyPax) - 35% chance
    // 35-60: 50 pax (SmallPax) - 25% chance
    // 60-85: Win a scroll (Scroll) - 25% chance
    // 85-100: 100 pax (BigPax) - 15% chance
    let is_win = true; // All outcomes are now wins
    let is_tiny_pax_win = result_number < 35.0;
    let is_small_pax_win = result_number >= 35.0 && result_number < 60.0;
    let is_scroll_win = result_number >= 60.0 && result_number < 85.0;
    let is_big_pax_win = result_number >= 85.0;

    // Apply the appropriate reward
    if is_tiny_pax_win {
        // Award 10 pax
        let new_pax_balance = new_balance + 10;
        sqlx::query!(
            "UPDATE users SET currency_balance = $1 WHERE id = $2",
            new_pax_balance,
            user_id.0
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
        })?;
        
        // Update the new balance to reflect the pax reward
        new_balance = new_pax_balance;
    } else if is_small_pax_win {
        // Award 20 pax
        let new_pax_balance = new_balance + 20;
        sqlx::query!(
            "UPDATE users SET currency_balance = $1 WHERE id = $2",
            new_pax_balance,
            user_id.0
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
        })?;
        new_balance = new_pax_balance;
    } else if is_scroll_win {
        // Award a scroll
        let now = OffsetDateTime::now_utc();
        let result = sqlx::query!(
            r#"
            UPDATE scrolls
            SET quantity = quantity + 1,
                updated_at = $1
            WHERE owner_id = $2 AND display_name = 'Summoning Scroll'
            "#,
            now,
            user_id.0
        )
        .execute(&mut *tx)
        .await;

        if result.is_err() || result.unwrap().rows_affected() == 0 {
            // If no rows were affected, insert new scroll record
            sqlx::query!(
                r#"
                INSERT INTO scrolls (
                    id, owner_id, created_at, updated_at, display_name,
                    image_path, description, quantity, item_type
                )
                VALUES (
                    $1, $2, $3, $3, 'Summoning Scroll',
                    '/static/images/scroll-default.avif',
                    'A scroll used to summon an egg',
                    1, 'scroll'
                )
                "#,
                Uuid::new_v4(),
                user_id.0,
                now
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
            })?;
        }
    } else if is_big_pax_win {
        // Award 50 pax
        let new_pax_balance = new_balance + 50;
        sqlx::query!(
            "UPDATE users SET currency_balance = $1 WHERE id = $2",
            new_pax_balance,
            user_id.0
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
        })?;
        new_balance = new_pax_balance;
    }

    // Commit the transaction
    tx.commit().await.map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    // Set the cooldown in Redis
    let _: () = redis::cmd("SETEX")
        .arg(&cooldown_key)
        .arg(WHEEL_SPIN_COOLDOWN)
        .arg(1)
        .query_async(&mut redis_conn)
        .await
        .map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Redis error: {}", e))
        })?;

    // Create appropriate message based on the outcome
    let message = if is_scroll_win {
        Some(format!("Congratulations! You rolled {:.2} and won a scroll! 🎉", result_number))
    } else if is_big_pax_win {
        Some(format!("Congratulations! You rolled {:.2} and won 50 pax! 🎉", result_number))
    } else if is_small_pax_win {
        Some(format!("Congratulations! You rolled {:.2} and won 20 pax! 💰", result_number))
    } else {
        Some(format!("You rolled {:.2} and won 10 pax! 🎉", result_number))
    };

    // Get username for logging
    let username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch username".to_string())
    })?.username;

    // Log the wheel spin result with emojis for better visibility
    if is_scroll_win {
        tracing::info!("🎡 WHEEL SPIN: User {} rolled {:.2} and won a Summoning Scroll! 📜", username, result_number);
    } else if is_big_pax_win {
        tracing::info!("🎡 WHEEL SPIN: User {} rolled {:.2} and won 50 pax! 💰💰💰", username, result_number);
    } else if is_small_pax_win {
        tracing::info!("🎡 WHEEL SPIN: User {} rolled {:.2} and won 20 pax! 💰💰", username, result_number);
    } else {
        tracing::info!("🎡 WHEEL SPIN: User {} rolled {:.2} and won 10 pax! 💰", username, result_number);
    }

    Ok(Json(WheelSpinResponse {
        success: true,
        is_win,
        new_balance,
        message,
        result_number: Some(result_number),
    }))
}

/// New handler to get wheel cooldown
#[debug_handler]
async fn get_wheel_cooldown(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // Check if user is a member
    match crate::generator::generate_code::is_member(&state.pool, user_id.0).await {
        Ok(is_member) => {
            if !is_member {
                return Ok(Json(serde_json::json!({
                    "in_cooldown": true,
                    "requires_membership": true,
                    "message": "This feature is only available to members. Please activate a membership code to continue."
                })));
            }
        },
        Err(e) => {
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR, 
                format!("Database error checking membership: {}", e)
            ));
        }
    }

    // Check cooldown in Redis
    let cooldown_key = format!("wheel_spin_cooldown:{}", user_id.0);
    let mut redis_conn = state.redis.get_async_connection().await.map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Redis error: {}", e))
    })?;

    // Get the TTL from Redis. If key doesn't exist, TTL may be -2.
    let ttl: i64 = redis::cmd("TTL")
        .arg(&cooldown_key)
        .query_async(&mut redis_conn)
        .await
        .map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Redis error: {}", e))
        })?;

    // If TTL is negative, return 0
    let cooldown_seconds = if ttl > 0 { ttl } else { 0 };

    Ok(Json(serde_json::json!({ "cooldown_seconds": cooldown_seconds })))
} 