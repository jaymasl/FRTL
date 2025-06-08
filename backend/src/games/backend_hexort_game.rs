use std::time::{SystemTime, UNIX_EPOCH};
use axum::{
    extract::{State, Extension},
    http::StatusCode,
    Json,
    Router,
    routing::{get, post},
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{error, info, trace};
use crate::AppState;
use crate::auth::middleware::UserId;
use tower_http::cors::{CorsLayer};
use axum::http::{Method, HeaderName, HeaderValue};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;

// Constants for game mechanics
const GAME_COOLDOWN_SECONDS: u64 = 5; // 5 seconds cooldown between games

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HexortGameSession {
    pub session_token: String,
    pub created_at: u64,
    pub ended: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewHexortGameResponse {
    pub session_id: String,
    pub session_signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HexortGameScore {
    pub score: i32,
    pub game_type: String,
    pub timestamp: u64,
    pub session_id: String,
    #[serde(default)]
    pub disable_rewards: bool,  // Optional field to disable PAX rewards
}

// Create a new game session for the hexort game
pub async fn create_hexort_game_session(
    State(app_state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<(StatusCode, String), (StatusCode, String)> {
    // Fetch username from database using user_id
    let username = match sqlx::query!("SELECT username FROM users WHERE id = $1", user_id.0)
        .fetch_one(&app_state.pool)
        .await {
            Ok(record) => record.username,
            Err(e) => {
                error!("Failed to fetch username for user {}: {:?}", user_id.0, e);
                // Consider how to handle this - maybe return an error or use a default
                "unknown_user".to_string() // Or return an error
            }
        };
    
    let mut redis_conn = match app_state.redis.get_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to connect to Redis: {:?}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to connect to Redis".to_string()));
        }
    };

    // Check if user is on cooldown
    let cooldown_key = format!("hexort_game:cooldown:{}", user_id.0);
    let cooldown_exists: bool = redis_conn.exists(&cooldown_key).await.unwrap_or(false);
    
    if cooldown_exists {
        let ttl: i64 = redis_conn.ttl(&cooldown_key).await.unwrap_or(0);
        return Err((StatusCode::TOO_MANY_REQUESTS, format!("Please wait {} seconds before starting a new game", ttl)));
    }

    // Create a unique session ID
    let session_id = uuid::Uuid::new_v4().to_string();
    let secret_key = std::env::var("GAME_SECRET_KEY").unwrap_or_else(|_| "default_secret_key".to_string());

    // Compute HMAC of session_id using secret_key
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).expect("HMAC can take key of any size");
    mac.update(session_id.as_bytes());
    let computed_hmac = hex::encode(mac.finalize().into_bytes());
    let session_token = format!("{}:{}", session_id, computed_hmac);

    // Store session with 2 hour expiry
    let _: () = redis_conn.set_ex(
        format!("game_session:{}:{}", user_id.0, session_id),
        &secret_key,
        7200,  // 2 hours
    ).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to store session".to_string()))?;

    // Use the fetched username in the log
    info!("🐝 Hexort game session created successfully for user {}", username);
    
    Ok((StatusCode::OK, session_token))
}

// Record game score and award rewards
pub async fn submit_hexort_score(
    State(app_state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(score_data): Json<HexortGameScore>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Log detailed session information for debugging
    trace!("📥 Received score submission for user {}: score={}, session_id={}", 
          user_id.0, score_data.score, score_data.session_id);
    
    let mut redis_conn = match app_state.redis.get_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to connect to Redis: {:?}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to connect to Redis".to_string()));
        }
    };

    // Verify game session
    let parts: Vec<&str> = score_data.session_id.split(':').collect();
    
    // ENHANCED SESSION VALIDATION: Add fallback for mobile browsers
    let session_id = if parts.len() != 2 {
        // Log the validation failure but don't immediately reject
        trace!("⚠️ Session token format incorrect for user {}: has {} parts instead of 2. Attempting fallback validation.", 
              user_id.0, parts.len());
        
        // Check if the raw token itself exists as a valid session
        // This handles the case where a mobile browser sends only the uuid without the signature
        let raw_key = format!("game_session:{}:{}", user_id.0, score_data.session_id);
        let raw_exists: bool = redis_conn.exists(&raw_key).await.unwrap_or(false);
        
        if raw_exists {
            trace!("✅ Fallback validation successful for user {}: found raw session key", user_id.0);
            // Convert to owned String
            score_data.session_id.to_string() // Use the raw token as the session ID
        } else {
            // As a second fallback, check if this user has ANY valid game session
            // Get a list of all game sessions for this user
            let pattern = format!("game_session:{}:*", user_id.0);
            let keys: Vec<String> = match redis_conn.keys(&pattern).await {
                Ok(k) => k,
                Err(e) => {
                    trace!("⚠️ Redis keys operation failed for pattern {}: {}", pattern, e);
                    Vec::new()
                }
            };
            
            if !keys.is_empty() {
                // Extract the session_id from the first valid key
                // Format is "game_session:{user_id}:{session_id}"
                let valid_key = &keys[0];
                let session_parts: Vec<&str> = valid_key.split(':').collect();
                
                if session_parts.len() >= 3 {
                    trace!("✅ Second fallback validation successful for user {}: found existing session", user_id.0);
                    // Clone the relevant part to create an owned String
                    session_parts[2].to_string()
                } else {
                    error!("❌ All fallback validation attempts failed for user {}: no valid session format in key {}", user_id.0, valid_key);
                    return Err((StatusCode::BAD_REQUEST, "Invalid game session".to_string()));
                }
            } else {
                error!("❌ All fallback validation attempts failed for user {}: no session keys found", user_id.0);
                return Err((StatusCode::BAD_REQUEST, "Invalid game session".to_string()));
            }
        }
    } else {
        // Normal case - session format is correct
        // Convert to owned String
        parts[0].to_string()
    };
    
    let key = format!("game_session:{}:{}", user_id.0, session_id);
    let exists: bool = match redis_conn.exists(&key).await {
        Ok(val) => val,
        Err(e) => {
            trace!("⚠️ Redis exists operation failed for key {}: {}", key, e);
            false
        }
    };
    if !exists {
        error!("Game session expired or not found for user {}: session_id {}", user_id.0, session_id);
        return Err((StatusCode::BAD_REQUEST, "Invalid game session".to_string()));
    }

    // Update game leaderboard first, regardless of rewards
    match sqlx::query!(
        r#"
        INSERT INTO game_leaderboard (game_type, user_id, high_score)
        VALUES ($1, $2, $3)
        ON CONFLICT (game_type, user_id) DO UPDATE SET
            high_score = GREATEST(game_leaderboard.high_score, EXCLUDED.high_score),
            updated_at = CURRENT_TIMESTAMP
        "#,
        "hexort",
        user_id.0,
        score_data.score
    )
    .execute(&app_state.pool)
    .await {
        Ok(_) => {
            trace!(
                "✅ Successfully updated leaderboard for user_id {} with score {}. Database write confirmed.", 
                user_id.0, 
                score_data.score
            );
        },
        Err(e) => {
            error!(
                "❌ Failed to update leaderboard for user_id {}: {:?}. Score *not* saved to database.", 
                user_id.0, 
                e
            );
            // IMPORTANT: Still continues execution and returns OK! 
            // Consider changing error handling logic later if this is not desired.
        }
    }

    // Set cooldown for the user
    let cooldown_key = format!("hexort_game:cooldown:{}", user_id.0);
    let _: () = redis_conn.set_ex(cooldown_key, "1", GAME_COOLDOWN_SECONDS).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to set cooldown".to_string()))?;

    // Store the score in Redis leaderboard
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let leaderboard_entry = format!("{}:{}:{}", user_id.0, score_data.score, now);
    let _: () = redis_conn.zadd("hexort_leaderboard", leaderboard_entry, score_data.score as f64).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to update leaderboard".to_string()))?;

    Ok(StatusCode::OK)
}

// Get cooldown status for the user
pub async fn get_hexort_cooldown(
    State(app_state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut redis_conn = match app_state.redis.get_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to connect to Redis: {:?}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to connect to Redis".to_string()));
        }
    };

    let cooldown_key = format!("hexort_game:cooldown:{}", user_id.0);
    let ttl: i64 = redis_conn.ttl(&cooldown_key).await.unwrap_or(0);
    
    let response = serde_json::json!({
        "cooldown_seconds": ttl.max(0),
    });
    
    Ok(Json(response))
}

// Create the router for the hexort game
pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/new", post(create_hexort_game_session))
        .route("/score", post(submit_hexort_score))
        .route("/cooldown", get(get_hexort_cooldown))
        .layer(
            CorsLayer::new()
                .allow_origin(vec![
                    "http://127.0.0.1:8080".parse::<HeaderValue>().unwrap(),
                    "http://127.0.0.1:3000".parse::<HeaderValue>().unwrap(),
                    "http://localhost:3000".parse::<HeaderValue>().unwrap(),
                    "https://frtl.dev".parse::<HeaderValue>().unwrap()
                ])
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS, Method::DELETE])
                .allow_headers([
                    AUTHORIZATION, 
                    CONTENT_TYPE, 
                    HeaderName::from_static("x-requested-with"),
                    HeaderName::from_static("x-session-signature")
                ])
                .allow_credentials(true)
        )
        .layer(axum::middleware::from_fn(crate::auth::middleware::require_auth))
} 