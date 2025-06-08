use super::*;
use axum::{
    extract::{State, Json, Extension},
    http::StatusCode,
    response::Response,
    body::Body,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::AppState;
use crate::auth::middleware::UserId;
use std::time::{SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use axum::http::header;
use tracing::{info, error};

const DAILY_CLAIM_COOLDOWN: i32 = 82800; // 23 hours
const DAILY_CLAIM_AMOUNT: i32 = 10;
const MAX_REWARDS_PER_MINUTE: u32 = 100;
const REWARD_WINDOW_DURATION: u64 = 300; // 5 minutes in seconds
const MAX_REWARDS_PER_WINDOW: u32 = 200;
const STREAK_RESET_WINDOW: i32 = 169600; // Changed from 86400 (24 hours) to 169600 (47 hours)
const SCROLL_REWARD_DAY: i32 = 7; // Award scroll every 7th day

#[derive(Debug, Deserialize, Clone)]
pub struct GameRewardRequest {
    pub session_token: String,
    pub game_type: String,
    pub score: i32,
    pub timestamp: u64,
    pub milestone_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GameSessionRequest {
    pub game_type: String,
}

#[derive(Serialize)]
pub struct GameRewardResponse {
    pub success: bool,
    pub new_balance: i32,
    pub error: Option<String>,
}

// Validates a game session token and ensures it's within the allowed time window (2 hours)
async fn validate_game_session(
    conn: &mut redis::aio::Connection,
    user_id: Uuid,
    session_token: &str,
    timestamp: u64,
) -> bool {
    let parts: Vec<&str> = session_token.split(':').collect();
    if parts.len() != 2 {
        error!("Invalid session token format for user {}: token does not have two parts", user_id);
        return false;
    }
    let session_id = parts[0];
    let provided_signature = parts[1];

    let key = format!("game_session:{}:{}", user_id, session_id);
    let exists: bool = conn.exists(&key).await.unwrap_or(false);
    if !exists {
        error!("Game session expired or not found for user {}: session_id {}", user_id, session_id);
        return false;
    }

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if current_time - timestamp > 7200 {  // Changed from 5 to 7200 seconds (2 hours)
        error!("Timestamp validation failed for user {}: timestamp too old (diff: {} seconds)", 
               user_id, current_time - timestamp);
        return false;
    }

    let game_secret = std::env::var("GAME_SECRET_KEY").unwrap_or_else(|_| "default_secret_key".to_string());
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(game_secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(session_id.as_bytes());
    let expected_signature = hex::encode(mac.finalize().into_bytes());

    if provided_signature != expected_signature {
        return false;
    }

    true
}

pub async fn create_game_session(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(payload): Json<GameSessionRequest>,
) -> Result<(StatusCode, String), StatusCode> {
    match payload.game_type.as_str() {
        "match" | "snake" | "2048" | "word" => (),
        _ => return Err(StatusCode::BAD_REQUEST),
    }

    // Get username from database
    let username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch username: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?.username;

    // Choose emoji based on game type
    let game_emoji = match payload.game_type.as_str() {
        "match" => "🎴",
        "snake" => "🐍",
        "2048" => "🎮",
        "word" => "📝",
        _ => "🎲",
    };

    info!("{} Received game session request from {} for game_type: '{}'", 
          game_emoji, username, payload.game_type);

    // Get Redis connection
    let mut conn = state.redis.get_async_connection().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // For word game, check if there's already an active game or cooldown
    if payload.game_type == "word" {
        // Check cooldown key first
        let cooldown_key = format!("word_game:cooldown:{}", user_id.0);
        let cooldown_ttl: i64 = conn.ttl(&cooldown_key).await.map_err(|e| {
            error!("Redis error checking cooldown: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        if cooldown_ttl > 0 {
            info!("🚫 User {} attempted to create a word game while in cooldown ({} seconds remaining)", 
                  username, cooldown_ttl);
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        
        // Try to set a lock key using SETNX (SET if Not eXists) - this is atomic
        let lock_key = format!("word_game:session_lock:{}", user_id.0);
        let lock_acquired: bool = conn.set_nx(&lock_key, "1").await.map_err(|e| {
            error!("Redis error setting lock: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        // Set a short expiry on the lock to prevent deadlocks
        let _: () = conn.expire(&lock_key, 30).await.map_err(|e| {
            error!("Redis error setting lock expiry: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        if !lock_acquired {
            info!("🚫 User {} attempted to create a word game while another session was being created", username);
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        
        // Now check if there's an active game
        let active_game_key = format!("word_game:active:{}", user_id.0);
        let active_ttl: i64 = conn.ttl(&active_game_key).await.map_err(|e| {
            error!("Redis error checking active game: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        
        if active_ttl > 0 {
            // Release the lock since we're returning an error
            let _: () = conn.del(&lock_key).await.unwrap_or(());
            
            info!("🚫 User {} attempted to create a word game while another game is active", username);
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    let session_id = Uuid::new_v4().to_string();
    let secret_key = std::env::var("GAME_SECRET_KEY").unwrap_or_else(|_| "default_secret_key".to_string());

    // Compute HMAC of session_id using secret_key
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).expect("HMAC can take key of any size");
    mac.update(session_id.as_bytes());
    let computed_hmac = hex::encode(mac.finalize().into_bytes());
    let session_token = format!("{}:{}", session_id, computed_hmac);

    // Store session with 2 hour expiry
    let _: () = conn.set_ex(
        format!("game_session:{}:{}", user_id.0, session_id),
        &secret_key,
        7200,  // Increased from 600 (10 minutes) to 7200 (2 hours)
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // For word game, release the lock after successful session creation
    if payload.game_type == "word" {
        let lock_key = format!("word_game:session_lock:{}", user_id.0);
        let _: () = conn.del(&lock_key).await.unwrap_or(());
    }

    Ok((StatusCode::OK, session_token))
}

pub async fn handle_game_reward(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(payload): Json<GameRewardRequest>,
) -> Result<Json<GameRewardResponse>, StatusCode> {
    match payload.game_type.as_str() {
        "match" | "snake" | "2048" | "word" => (),
        _ => return Ok(Json(GameRewardResponse {
            success: false,
            new_balance: 0,
            error: Some("Invalid game type".to_string()),
        })),
    }

    if payload.score <= 0 || payload.score > 1000 {
        return Ok(Json(GameRewardResponse {
            success: false,
            new_balance: 0,
            error: Some("Invalid score".to_string()),
        }));
    }

    let mut conn = state.redis.get_async_connection().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if payload.game_type == "2048" {
        let reward_key = format!("game_reward_claimed:{}:{}", user_id.0, payload.session_token);
        let already_claimed: bool = conn.exists(&reward_key).await.unwrap_or(false);
        
        if already_claimed {
            info!("🚫 Prevented duplicate reward claim for user {} with session token {}", user_id.0, payload.session_token);
            return Ok(Json(GameRewardResponse {
                success: false,
                new_balance: 0,
                error: Some("Reward already claimed for this session".to_string()),
            }));
        }

        // Mark this session as having claimed a reward (permanent record)
        let _: () = conn.set(&reward_key, "1").await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    
    if !validate_game_session(
        &mut conn,
        user_id.0,
        &payload.session_token,
        payload.timestamp,
    ).await {
        return Ok(Json(GameRewardResponse {
            success: false,
            new_balance: 0,
            error: Some("Invalid game session".to_string()),
        }));
    }

    // If this is a milestone reward, verify it hasn't been claimed
    if let Some(milestone_id) = &payload.milestone_id {
        let milestone_key = format!("milestone:{}:{}:{}", user_id.0, payload.game_type, milestone_id);
        let already_claimed: bool = conn.exists(&milestone_key).await.unwrap_or(true);
        
        if already_claimed {
            return Ok(Json(GameRewardResponse {
                success: false,
                new_balance: 0,
                error: Some("Milestone already claimed".to_string()),
            }));
        }

        // Mark milestone as claimed
        let _: () = conn.set_ex(&milestone_key, "1", 60).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;  // 1 minute expiry
    }

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // New 5-minute window reward logic using a Lua script for atomic check and update
    let window_key = format!("game_reward_window:{}:{}:{}", user_id.0, payload.game_type, current_time / REWARD_WINDOW_DURATION);
    let lua_script = r#"
local current = redis.call('GET', KEYS[1]) or '0'
current = tonumber(current)
local increment = tonumber(ARGV[1])
if current + increment > tonumber(ARGV[2]) then
    return -1
else
    local new_total = redis.call('INCRBY', KEYS[1], increment)
    redis.call('EXPIRE', KEYS[1], ARGV[3])
    return new_total
end
"#;

    let script_result: i64 = redis::cmd("EVAL")
        .arg(lua_script)
        .arg(1)  // one key
        .arg(&window_key)
        .arg(payload.score)
        .arg(MAX_REWARDS_PER_WINDOW)
        .arg(REWARD_WINDOW_DURATION)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if script_result == -1 {
        return Ok(Json(GameRewardResponse {
            success: false,
            new_balance: 0,
            error: Some("Maximum rewards per 5 minutes exceeded".to_string()),
        }));
    }

    match sqlx::query!(
        "UPDATE users SET currency_balance = currency_balance + $1 WHERE id = $2 RETURNING currency_balance",
        payload.score,
        user_id.0
    )
    .fetch_one(&state.pool)
    .await {
        Ok(record) => Ok(Json(GameRewardResponse {
            success: true,
            new_balance: record.currency_balance,
            error: None,
        })),
        Err(e) => {
            eprintln!("Database update error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn claim_daily_reward(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Response<Body>, Response<Body>> {
    // Check if user is a member
    match crate::generator::generate_code::is_member(&state.pool, user_id.0).await {
        Ok(is_member) => {
            if !is_member {
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::json!({
                        "success": false,
                        "message": "This feature is only available to members. Please activate a membership code to continue.",
                        "requires_membership": true
                    }).to_string()))
                    .unwrap());
            }
        },
        Err(e) => {
            tracing::error!("Database error checking membership: {}", e);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Database error"))
                .unwrap());
        }
    }

    // Verify Redis connection first
    let mut redis_conn = match state.redis.get_async_connection().await {
        Err(redis_err) => {
            tracing::error!("Redis connection error in claim_daily_reward: {}", redis_err);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Redis connection error"))
                .unwrap());
        },
        Ok(conn) => conn
    };

    let mut tx = state.pool.begin().await.map_err(|e| {
        tracing::error!("Failed to start transaction: {}", e);
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Database error"))
            .unwrap()
    })?;

    let user = sqlx::query!(
        r#"
        SELECT 
            currency_balance,
            last_daily_reward,
            claim_streak,
            username,
            COALESCE(
                EXTRACT(EPOCH FROM (NOW() - last_daily_reward))::float,
                CAST($1 AS float)
            ) AS seconds_since_last_claim
        FROM users 
        WHERE id = $2
        FOR UPDATE
        "#,
        DAILY_CLAIM_COOLDOWN as i32,
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Database error"))
            .unwrap()
    })?;

    let seconds = user.seconds_since_last_claim.unwrap_or(DAILY_CLAIM_COOLDOWN as f64);
    
    // Check if the user is still on cooldown
    if seconds < DAILY_CLAIM_COOLDOWN as f64 {
        // User is still on cooldown, return the remaining time
        let remaining_cooldown = DAILY_CLAIM_COOLDOWN as f64 - seconds;
        
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::json!({
                "success": false,
                "message": format!("Please wait {} seconds before claiming again.", remaining_cooldown as i32),
                "remaining_cooldown": remaining_cooldown as i32,
                "claim_streak": user.claim_streak
            }).to_string()))
            .unwrap());
    }
    
    // Calculate the time elapsed after the mandatory cooldown period
    let effective_elapsed = if seconds > DAILY_CLAIM_COOLDOWN as f64 {
        seconds - DAILY_CLAIM_COOLDOWN as f64
    } else {
        0.0
    };

    // If no previous claim or if the effective elapsed time exceeds 24 hours, reset streak to 1; otherwise increment
    let new_claim_streak = if user.last_daily_reward.is_none() || effective_elapsed > STREAK_RESET_WINDOW as f64 {
        1
    } else {
        user.claim_streak + 1
    };

    // Calculate the week number (1-based) from the streak
    let week_number = ((new_claim_streak - 1) / 7) + 1;
    
    // Calculate reward based on week number instead of daily streak
    let reward = DAILY_CLAIM_AMOUNT + (week_number - 1);
    
    let new_balance = user.currency_balance + reward;
    let now = OffsetDateTime::now_utc();
    
    sqlx::query!(
        "UPDATE users SET currency_balance = $1, last_daily_reward = $2, claim_streak = $3 WHERE id = $4",
        new_balance,
        now,
        new_claim_streak,
        user_id.0
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Update reward error: {}", e);
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Failed to update reward"))
            .unwrap()
    })?;

    // Check if the user should receive a scroll (only on 7th day)
    let scroll_awarded = new_claim_streak % SCROLL_REWARD_DAY == 0;
    
    // Award a scroll only on the 7th day
    if scroll_awarded {
        // Award a scroll
        let updated_scroll = sqlx::query!(
            r#"
            UPDATE scrolls
            SET quantity = quantity + 1,
                updated_at = $1
            WHERE owner_id = $2 AND display_name = 'Summoning Scroll'
            RETURNING quantity
            "#,
            now,
            user_id.0
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update scroll: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Database error"))
                .unwrap()
        })?;

        if updated_scroll.is_none() {
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
                tracing::error!("Failed to create scroll: {}", e);
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Database error"))
                    .unwrap()
            })?;
        }
        
        tracing::info!("🧾 {} received a scroll for 7th day claim!", user.username);
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit transaction: {}", e);
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Database error"))
            .unwrap()
    })?;

    // Set the cooldown in Redis with a small buffer (5 seconds) to ensure consistency
    let cooldown_key = format!("user:{}:claim_cooldown", user_id.0);
    let _: () = redis::cmd("SETEX")
        .arg(&cooldown_key)
        .arg(DAILY_CLAIM_COOLDOWN + 5) // Add a small buffer
        .arg(1) // Just a placeholder value
        .query_async(&mut redis_conn)
        .await
        .unwrap_or(());

    tracing::info!("🧾 {} claimed daily reward for {} pax, streak is now {}", user.username, reward, new_claim_streak);

    let message = if scroll_awarded {
        "Congratulations! You received a daily reward and a scroll for your 7th day claim!"
    } else {
        "Successfully claimed your daily reward!"
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::json!({
            "success": true,
            "new_balance": new_balance,
            "remaining_cooldown": DAILY_CLAIM_COOLDOWN, // Return the full cooldown period
            "claim_streak": new_claim_streak,
            "message": message,
            "scroll_reward": scroll_awarded
        }).to_string()))
        .unwrap())
}

pub async fn handle_game_scroll_reward(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(payload): Json<GameRewardRequest>,
) -> Result<Json<GameRewardResponse>, StatusCode> {
    match payload.game_type.as_str() {
        "match" | "snake" | "word" => (),
        _ => return Ok(Json(GameRewardResponse {
            success: false,
            new_balance: 0,
            error: Some("Invalid game type".to_string()),
        })),
    }

    let mut conn = state.redis.get_async_connection().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if !validate_game_session(
        &mut conn,
        user_id.0,
        &payload.session_token,
        payload.timestamp,
    ).await {
        return Ok(Json(GameRewardResponse {
            success: false,
            new_balance: 0,
            error: Some("Invalid game session".to_string()),
        }));
    }

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let minute_key = format!("game_reward_minute:{}:{}:{}", user_id.0, payload.game_type, current_time / 60);
    let rewards_this_minute: u32 = conn.incr(&minute_key, 1u32).await.unwrap_or(1);
    let _: () = conn.expire(&minute_key, 60).await.unwrap_or(());

    if rewards_this_minute > MAX_REWARDS_PER_MINUTE {
        return Ok(Json(GameRewardResponse {
            success: false,
            new_balance: 0,
            error: Some("Maximum rewards per minute exceeded".to_string()),
        }));
    }

    let mut tx = state.pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = sqlx::query!(
        "SELECT currency_balance FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let now = OffsetDateTime::now_utc();
    let updated_scroll = sqlx::query!(
        r#"
        UPDATE scrolls
        SET quantity = quantity + 1,
            updated_at = $1
        WHERE owner_id = $2 AND display_name = 'Summoning Scroll'
        RETURNING quantity
        "#,
        now,
        user_id.0
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if updated_scroll.is_none() {
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
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    tx.commit().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(GameRewardResponse {
        success: true,
        new_balance: user.currency_balance,
        error: None,
    }))
}

pub async fn reset_claim_streak(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Response<Body>, StatusCode> {
    // Verify Redis connection first
    if let Err(redis_err) = state.redis.get_async_connection().await {
        tracing::error!("Redis connection error in reset_claim_streak: {}", redis_err);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // First, fetch the user's last_daily_reward and claim_streak
    let record = sqlx::query!(
        r#"
        SELECT 
            last_daily_reward, 
            claim_streak,
            COALESCE(
                EXTRACT(EPOCH FROM (NOW() - last_daily_reward))::float,
                CAST($1 AS float)
            ) AS seconds_since_last_claim
        FROM users 
        WHERE id = $2
        "#,
        DAILY_CLAIM_COOLDOWN as i32,
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error fetching user: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Check if we have a valid last_daily_reward and if we're past the claim window
    if record.last_daily_reward.is_some() {
        let seconds = record.seconds_since_last_claim.unwrap_or(0.0);
        
        // Only reset if we're past the claim window (> 24 hours)
        if seconds <= STREAK_RESET_WINDOW as f64 {
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .body(Body::from(serde_json::json!({
                    "success": false,
                    "message": "Still within claim window"
                }).to_string()))
                .unwrap());
        }
    }

    // If we're here, either:
    // 1. There's no last_daily_reward (shouldn't happen normally)
    // 2. We're past the claim window (> 24 hours)
    // In either case, we should reset the streak
    match sqlx::query!(
        r#"
        UPDATE users 
        SET claim_streak = 0, 
            last_daily_reward = NULL 
        WHERE id = $1
        RETURNING claim_streak, last_daily_reward
        "#,
        user_id.0
    )
    .fetch_one(&state.pool)
    .await {
        Ok(_record) => {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(Body::from(serde_json::json!({
                    "success": true
                }).to_string()))
                .unwrap())
        },
        Err(e) => {
            tracing::error!("Failed to reset streak for user {}: {}", user_id.0, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_claim_status(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Response<Body>, StatusCode> {
    // Check if user is a member
    match crate::generator::generate_code::is_member(&state.pool, user_id.0).await {
        Ok(is_member) => {
            if !is_member {
                // Create the response with membership requirement
                let json_response = serde_json::json!({
                    "remaining_cooldown": 3600, // Just use the cooldown period
                    "claim_streak": 0,
                    "last_claim_time": null,
                    "requires_membership": true
                });

                // Return the JSON response
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_string(&json_response).unwrap()))
                    .unwrap());
            }
        },
        Err(e) => {
            tracing::error!("Database error checking membership: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Verify Redis connection
    let mut redis_conn = match state.redis.get_async_connection().await {
        Err(redis_err) => {
            tracing::error!("Redis connection error in get_claim_status: {}", redis_err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
        Ok(conn) => conn
    };

    // Get current cooldown from Redis
    let cooldown_key = format!("user:{}:claim_cooldown", user_id.0);
    let ttl: i64 = redis::cmd("TTL")
        .arg(&cooldown_key)
        .query_async(&mut redis_conn)
        .await
        .unwrap_or(-2);

    // Convert negative TTL values to 0 (key doesn't exist or no expiry)
    let mut cooldown_seconds = if ttl > 0 { ttl } else { 0 };

    // Get user's streak and last claim time from the database
    let claim_info = match sqlx::query!(
        r#"
        SELECT 
            claim_streak, 
            last_daily_reward,
            COALESCE(
                EXTRACT(EPOCH FROM (NOW() - last_daily_reward))::float,
                CAST($1 AS float)
            ) AS seconds_since_last_claim
        FROM users 
        WHERE id = $2
        "#,
        DAILY_CLAIM_COOLDOWN as i32,
        user_id.0
    )
    .fetch_one(&state.pool)
    .await {
        Ok(result) => result,
        Err(e) => {
            log::error!("Database error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // If Redis doesn't have a cooldown but we have a recent claim in the database,
    // calculate the cooldown from the database
    if cooldown_seconds == 0 && claim_info.last_daily_reward.is_some() {
        let seconds_since_claim = claim_info.seconds_since_last_claim.unwrap_or(DAILY_CLAIM_COOLDOWN as f64);
        
        // If we're still within the cooldown period, calculate remaining time
        if seconds_since_claim < DAILY_CLAIM_COOLDOWN as f64 {
            cooldown_seconds = (DAILY_CLAIM_COOLDOWN as f64 - seconds_since_claim).ceil() as i64;
            
            // Add a small buffer (5 seconds) to ensure frontend doesn't show claimable too early
            cooldown_seconds += 5;
            
            // Store this in Redis for future requests
            let _: () = redis::cmd("SETEX")
                .arg(&cooldown_key)
                .arg(cooldown_seconds)
                .arg(1) // Just a placeholder value
                .query_async(&mut redis_conn)
                .await
                .unwrap_or(());
        }
    }

    // Create the response
    let json_response = serde_json::json!({
        "remaining_cooldown": cooldown_seconds,
        "claim_streak": claim_info.claim_streak,
        "last_claim_time": claim_info.last_daily_reward.map(|t| t.unix_timestamp()),
    });

    // Return the JSON response
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&json_response).unwrap()))
        .unwrap())
}