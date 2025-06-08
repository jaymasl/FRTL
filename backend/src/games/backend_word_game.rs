use axum::{
    extract::{State, Extension, Json, Query},
    routing::{get, post},
    Router,
    http::StatusCode,
    http::header::{AUTHORIZATION, CONTENT_TYPE},
    http::{HeaderValue, HeaderName, Method},
};
use std::{env, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use tokio::sync::Mutex;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use tower_http::cors::CorsLayer;
use std::collections::HashMap;
use redis::{Client as RedisClient, AsyncCommands};
use crate::auth::middleware::UserId;
use crate::AppState;
use tracing::{error, info};
use uuid::Uuid;
use sqlx;
use chrono;
use shared::shared_word_game::get_random_word;
use serde::{Serialize, Deserialize};

// Type alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

// Constants for game settings
const GAME_TIMER_SECONDS: u64 = 900; // 15 minutes
const WIN_COOLDOWN_SECONDS: u64 = 82800; // 23 hours
const LOSS_COOLDOWN_SECONDS: u64 = 30; // 30 seconds
const GUESS_COOLDOWN_SECONDS: u64 = 1; // 1 second between guesses

// === Core Structures for the Word Game ===

#[derive(Clone, Debug)]
pub struct WordGame {
    secret_word: String,      // never exposed to the client!
    allowed_guesses: u32,       // equals the word length
    remaining_guesses: u32,     // decrements with each guess
    guesses: Vec<String>,       // guesses submitted by the user
    tiles_history: Vec<Vec<LetterTile>>, // store tile evaluations for each guess
    solved: bool,
}

impl WordGame {
    pub fn new() -> Self {
        let word = get_random_word();
        Self {
            secret_word: word.clone(),
            allowed_guesses: 7,  // Always 7 guesses
            remaining_guesses: 7, // Always 7 guesses
            guesses: Vec::new(),
            tiles_history: Vec::new(),
            solved: false,
        }
    }

    /// Process a guess; returns Ok(is_correct) if guess processed, or Err(reason) if invalid.
    pub fn process_guess(&mut self, guess: String) -> Result<bool, String> {
        let normalized = guess.trim().to_lowercase();
        if normalized.len() != self.secret_word.len() {
            return Err(format!("Guess must be {} letters long", self.secret_word.len()));
        }
        // Enforce alphabetic validation on normalized guess
        if !normalized.chars().all(|c| c.is_alphabetic()) {
            return Err("Guess must contain only alphabetic characters".to_string());
        }
        
        // Check if this guess has already been made
        if self.guesses.contains(&normalized) {
            return Err("You already guessed that".to_string());
        }
        
        // Record the normalized guess and evaluate tiles
        self.guesses.push(normalized.clone());
        
        // Decrement remaining guesses
        self.remaining_guesses = self.remaining_guesses.saturating_sub(1);
        
        // Evaluate tiles for this guess - updated to handle repeated letters correctly
        let mut tiles = Vec::new();
        let secret = self.secret_word.to_lowercase();
        
        // First, mark exact matches (green)
        let secret_chars: Vec<_> = secret.chars().collect();
        let guess_chars: Vec<_> = normalized.chars().collect();
        let mut used_positions = vec![false; secret_chars.len()];
        
        // First pass: mark green matches
        for (i, &ch) in guess_chars.iter().enumerate() {
            if i < secret_chars.len() && ch == secret_chars[i] {
                tiles.push(LetterTile { letter: ch, status: "green".to_string() });
                used_positions[i] = true;
            } else {
                // Placeholder for second pass
                tiles.push(LetterTile { letter: ch, status: "".to_string() });
            }
        }
        
        // Second pass: mark yellow and gray
        for (i, &ch) in guess_chars.iter().enumerate() {
            if tiles[i].status.is_empty() {
                // Look for this character elsewhere in the secret word
                let mut found = false;
                for (j, &secret_ch) in secret_chars.iter().enumerate() {
                    if !used_positions[j] && ch == secret_ch {
                        tiles[i].status = "yellow".to_string();
                        used_positions[j] = true;
                        found = true;
                        break;
                    }
                }
                
                if !found {
                    tiles[i].status = "gray".to_string();
                }
            }
        }
        
        self.tiles_history.push(tiles);
        
        let is_correct = normalized == self.secret_word.to_lowercase();
        if is_correct {
            self.solved = true;
        }
        Ok(is_correct)
    }

    pub fn to_public(&self) -> PublicWordGame {
        // Only expose the solution if the game is over (solved or out of guesses)
        let solution = if self.solved || self.guesses.len() >= self.allowed_guesses as usize {
            Some(self.secret_word.clone())
        } else {
            None
        };
        
        PublicWordGame {
            allowed_guesses: self.allowed_guesses,
            remaining_guesses: self.remaining_guesses,
            guesses: self.guesses.clone(),
            tiles_history: self.tiles_history.clone(),
            solved: self.solved,
            word_length: self.secret_word.len(),
            solution,
            created_at: None, // This will be set by the session
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PublicWordGame {
    pub allowed_guesses: u32,
    pub remaining_guesses: u32,
    pub guesses: Vec<String>,
    pub tiles_history: Vec<Vec<LetterTile>>,
    pub solved: bool,
    pub word_length: usize,
    pub solution: Option<String>,
    pub created_at: Option<u64>,
}

// Game session structure; the secret word is stored in the game.
#[derive(Clone)]
pub struct WordGameSession {
    pub game: WordGame,
    pub created_at: u64,       // seconds since epoch
    pub last_guess_time: u64,  // used for rate limiting
    pub game_session_token: String,
    pub ended: bool,         // flag to indicate if game has ended (e.g., due to timeout)
}

impl WordGameSession {
    const MIN_GUESS_INTERVAL: u64 = GUESS_COOLDOWN_SECONDS;

    fn can_make_guess(&self, current_time: u64) -> bool {
        current_time - self.last_guess_time >= Self::MIN_GUESS_INTERVAL
    }
}

#[derive(Clone)]
pub struct WordGameState {
    // Map of session_id -> session info
    pub sessions: Arc<Mutex<HashMap<String, WordGameSession>>>,
    pub redis: RedisClient,
}

impl WordGameState {
    async fn cleanup_expired_sessions(&self, _app_state: &AppState) {
        let mut sessions = self.sessions.lock().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // We'll just remove expired sessions without trying to update stats
        // The background task for each session will handle updating stats
        sessions.retain(|_, session| {
            now - session.created_at < GAME_TIMER_SECONDS || session.ended
        });
    }

    async fn is_rate_limited(&self, user_id: &Uuid) -> bool {
        let mut redis_conn = match self.redis.get_async_connection().await {
            Ok(conn) => conn,
            Err(e) => {
                error!("Failed to get Redis connection: {:?}", e);
                return true; // Rate limit on Redis failure for safety
            }
        };

        let cooldown_key = format!("word_game:cooldown:{}", user_id);
        
        // First check if user is in cooldown
        let cooldown_ttl: i64 = redis_conn.ttl(&cooldown_key).await.unwrap_or(0);
        if cooldown_ttl > 0 {
            return true; // User is in cooldown period
        }

        // If not in cooldown, check for and clean up any active game
        let active_game_key = format!("word_game:active:{}", user_id);
        match redis_conn.get::<_, Option<String>>(&active_game_key).await {
            Ok(Some(_)) => {
                // Check if the active game key has expired
                let ttl: i64 = redis_conn.ttl(&active_game_key).await.unwrap_or(0);
                if ttl <= 0 {
                    // If expired, delete it and allow new game
                    let _ : () = redis_conn.del(&active_game_key).await.unwrap_or(());
                    false
                } else {
                    // Only consider the game active if we're within the cooldown period
                    ttl <= GAME_TIMER_SECONDS as i64
                }
            },
            Ok(None) => false, // No active game
            Err(e) => {
                error!("Redis error while checking active game: {:?}", e);
                true
            }
        }
    }
}

// Add session expiration constant near other constants
const SESSION_EXPIRY_SECONDS: u64 = 1800;  // 30 minutes expiration

// Compute HMAC signature for a given message using WORD_GAME_SECRET from env.
fn compute_signature(message: &str) -> Result<String, StatusCode> {
    let secret = env::var("WORD_GAME_SECRET").map_err(|_| {
        error!("WORD_GAME_SECRET environment variable not set");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    mac.update(message.as_bytes());
    let result = mac.finalize().into_bytes();
    Ok(hex::encode(result))
}

// === API Response and Request Structures ===

#[derive(Serialize)]
pub struct NewWordGameResponse {
    pub session_id: String,
    pub session_signature: String,
    pub game: PublicWordGame,
}

#[derive(Deserialize)]
pub struct GuessRequest {
    pub session_id: String,
    pub guess: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LetterTile {
    pub letter: char,
    pub status: String, // "green", "yellow", or "gray"
}

#[derive(Serialize)]
pub struct GuessResponse {
    pub correct: bool,
    pub game: PublicWordGame,
    pub message: String,
    pub tiles: Vec<LetterTile>,
    pub new_balance: Option<f64>,
}

#[derive(Deserialize)]
pub struct RefreshQuery {
    pub session_id: String,
}

#[derive(Serialize)]
pub struct RefreshResponse {
    pub game: PublicWordGame,
}

#[derive(Serialize)]
pub struct CooldownStatus {
    pub in_cooldown: bool,
    pub remaining_seconds: Option<i64>,
    pub is_win_cooldown: bool,
    pub requires_membership: bool,
}

// Add this struct for the leaderboard response
#[derive(serde::Serialize)]
pub struct WordLeaderboardEntry {
    pub username: String,
    pub current_streak: i32,
    pub highest_streak: i32,
    pub fastest_time: Option<i32>,
    pub total_words_guessed: i32,
    pub total_games_played: i32,
    pub updated_at: String,
}

// === Endpoint Handlers ===

// Create a new game session
async fn new_game(
    State(state): State<Arc<WordGameState>>,
    Extension(app_state): Extension<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<NewWordGameResponse>, (StatusCode, String)> {
    // Check if user is a member
    match crate::generator::generate_code::is_member(&app_state.pool, user_id.0).await {
        Ok(is_member) => {
            if !is_member {
                return Err((
                    StatusCode::FORBIDDEN,
                    "This feature is only available to members. Please activate a membership code to continue.".to_string()
                ));
            }
        },
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error checking membership: {}", e)
            ));
        }
    }

    // Check if the user is in cooldown
    let mut redis_conn = app_state.redis.get_async_connection().await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis error: {}", e))
    })?;

    // Get username from database first so we can use it in all logs
    let username = match sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&app_state.pool)
    .await {
        Ok(user) => user.username,
        Err(e) => {
            error!("Failed to fetch username: {:?}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()));
        }
    };
    
    // Clean up expired sessions
    state.cleanup_expired_sessions(&app_state).await;

    // Check cooldown using is_rate_limited method
    if state.is_rate_limited(&user_id.0).await {
        return Err((StatusCode::TOO_MANY_REQUESTS, "Game in cooldown period".to_string()));
    }

    // Create game session first
    let session_request = crate::services::claim_service::GameSessionRequest {
        game_type: "word".to_string(),
    };

    let (status, session_token) = match crate::services::claim_service::create_game_session(
        State(app_state.clone()),
        Extension(user_id),
        Json(session_request),
    ).await {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to create game session for user {}: {:?}", username, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
        }
    };

    // Check if game session creation was successful
    if status != StatusCode::OK {
        error!("Game session creation for user {} returned non-OK status: {}", username, status);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to create game session".to_string()));
    }

    info!("📝 Word game session created successfully for user {}", username);

    let mut sessions = state.sessions.lock().await;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let game = WordGame::new();
    let session_id = uuid::Uuid::new_v4().to_string();
    
    // Set the active game key in Redis with proper duration
    let active_game_key = format!("word_game:active:{}", user_id.0);
    let _: () = redis_conn.set_ex(&active_game_key, "active", GAME_TIMER_SECONDS).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Redis error".to_string()))?;
    
    match compute_signature(&format!("session:{}", &session_id)) {
        Ok(session_signature) => {
            let mut public_game = game.to_public();
            public_game.created_at = Some(now);

            sessions.insert(session_id.clone(), WordGameSession {
                game: game.clone(),
                created_at: now,
                last_guess_time: now,
                game_session_token: session_token,
                ended: false,
            });

            // Spawn a background task that automatically ends the game when the timer expires
            {
                use std::time::Duration;
                let state_clone = Arc::clone(&state);
                let session_id_clone = session_id.clone();
                let app_state_clone = app_state.clone();
                let user_id_clone = user_id.0;
                let username_clone = username.clone();
                
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(GAME_TIMER_SECONDS)).await;
                    let mut sessions = state_clone.sessions.lock().await;
                    if let Some(session) = sessions.get_mut(&session_id_clone) {
                        // Only process if the session hasn't already ended
                        if !session.ended {
                            // Fill in remaining guesses
                            while (session.game.guesses.len() as u32) < session.game.allowed_guesses {
                                session.game.guesses.push(String::new());
                                session.game.tiles_history.push(vec![]);
                            }
                            
                            // Mark the session as ended
                            session.ended = true;
                            
                            // Log game completion with loss due to timeout
                            let secret_word = session.game.secret_word.clone();
                            info!("🎮 Word game ended for user {}: LOSS! ⏱️ Timed out after {} seconds. The word was '{}'", 
                                  username_clone, GAME_TIMER_SECONDS, secret_word);
                            
                            // Update word game stats for timeout loss
                            if let Err(e) = update_word_game_stats(
                                &app_state_clone.pool,
                                user_id_clone,
                                false, // loss
                                None
                            ).await {
                                error!("Failed to update word game stats for user {} after timeout in background task: {:?}", 
                                       username_clone, e);
                            }
                            
                            // Clear the active game key and set cooldown
                            if let Ok(mut redis_conn) = state_clone.redis.get_async_connection().await {
                                let active_game_key = format!("word_game:active:{}", user_id_clone);
                                let cooldown_key = format!("word_game:cooldown:{}", user_id_clone);
                                let _ : () = redis_conn.del(&active_game_key).await.unwrap_or(());
                                let _: () = redis::cmd("SETEX")
                                    .arg(&cooldown_key)
                                    .arg(LOSS_COOLDOWN_SECONDS)  // 30 second cooldown
                                    .arg("1")
                                    .query_async(&mut redis_conn)
                                    .await
                                    .unwrap_or(());
                            }
                        }
                    }
                });
            }

            Ok(Json(NewWordGameResponse {
                session_id: session_id.clone(),
                session_signature,
                game: public_game,
            }))
        },
        Err(e) => {
            error!("Failed to compute session signature for user {}: {:?}", username, e);
            Err((e, "Failed to compute session signature".to_string()))
        }
    }
}

// Process a guess
async fn guess(
    State(state): State<Arc<WordGameState>>,
    Extension(app_state): Extension<AppState>,
    Extension(user_id): Extension<UserId>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<GuessRequest>,
) -> Result<Json<GuessResponse>, StatusCode> {
    // Get username for logging
    let username = match sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&app_state.pool)
    .await {
        Ok(user) => user.username,
        Err(e) => {
            error!("Failed to fetch username: {:?}", e);
            user_id.0.to_string() // Fallback to UUID if username fetch fails
        }
    };
    
    let mut sessions = state.sessions.lock().await;
    
    if let Some(session) = sessions.get_mut(&payload.session_id) {
        // If the game has already ended (e.g., due to timeout), immediately return game over
        if session.ended {
            let mut public_game = session.game.to_public();
            public_game.solution = Some(session.game.secret_word.clone());
            
            info!("🎮 Word game already ended for user {}: Game was already marked as ended. The word was '{}'", 
                  username, session.game.secret_word);
            
            return Ok(Json(GuessResponse {
                correct: false,
                game: public_game,
                message: "Times up! Game over.".to_string(),
                tiles: vec![],
                new_balance: None,
            }));
        }
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        // Check if game has expired due to time
        if now - session.created_at >= GAME_TIMER_SECONDS {
            // Mark the game as over by filling the guess history so that further guesses are disallowed
            while (session.game.guesses.len() as u32) < session.game.allowed_guesses {
                session.game.guesses.push(String::new());
                session.game.tiles_history.push(vec![]);
            }
            session.ended = true;
            
            // Clear the active game key and set cooldown
            if let Ok(mut redis_conn) = state.redis.get_async_connection().await {
                let active_game_key = format!("word_game:active:{}", user_id.0);
                let cooldown_key = format!("word_game:cooldown:{}", user_id.0);
                let _ : () = redis_conn.del(&active_game_key).await.unwrap_or(());
                let _: () = redis::cmd("SETEX")
                    .arg(&cooldown_key)
                    .arg(LOSS_COOLDOWN_SECONDS)  // 30 second cooldown
                    .arg("1")
                    .query_async(&mut redis_conn)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
            
            let mut public_game = session.game.to_public();
            public_game.solution = Some(session.game.secret_word.clone());
            
            info!("🎮 Word game ended for user {}: LOSS! ⏱️ Timed out during guess. The word was '{}'", 
                  username, session.game.secret_word);
                  
            // Update word game stats for timeout loss
            if let Err(e) = update_word_game_stats(
                &app_state.pool,
                user_id.0,
                false, // loss
                None
            ).await {
                error!("Failed to update word game stats for user {} after timeout: {:?}", username, e);
            }
            
            return Ok(Json(GuessResponse {
                correct: false,
                game: public_game,
                message: format!("Time's up! The word was '{}'.", session.game.secret_word),
                tiles: vec![],
                new_balance: None,
            }));
        }
        
        // Check if game is already over
        if session.game.solved || (session.game.guesses.len() as u32) >= session.game.allowed_guesses {
            session.ended = true;
            
            // Clear the active game key and set cooldown
            if let Ok(mut redis_conn) = state.redis.get_async_connection().await {
                let active_game_key = format!("word_game:active:{}", user_id.0);
                let cooldown_key = format!("word_game:cooldown:{}", user_id.0);
                let _ : () = redis_conn.del(&active_game_key).await.unwrap_or(());
                let _: () = redis::cmd("SETEX")
                    .arg(&cooldown_key)
                    .arg(LOSS_COOLDOWN_SECONDS)  // 30 second cooldown for losses
                    .arg("1")
                    .query_async(&mut redis_conn)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
            
            let mut public_game = session.game.to_public();
            public_game.solution = Some(session.game.secret_word.clone());
            
            return Ok(Json(GuessResponse {
                correct: false,
                game: public_game,
                message: format!("Game over! The word was '{}'. You can start a new game in {} seconds.", session.game.secret_word, LOSS_COOLDOWN_SECONDS),
                tiles: vec![],
                new_balance: None,
            }));
        }

        // Check rate limiting
        if !session.can_make_guess(now) {
            return Ok(Json(GuessResponse {
                correct: false,
                game: session.game.to_public(),
                message: "Please wait before making another guess".to_string(),
                tiles: vec![],
                new_balance: None,
            }));
        }

        // Update last_guess_time
        session.last_guess_time = now;

        // Verify session signature
        let expected_sig = compute_signature(&format!("session:{}", &payload.session_id))?;
        let session_sig = headers
            .get("X-Session-Signature")
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
            
        if session_sig != expected_sig {
            return Err(StatusCode::FORBIDDEN);
        }

        match session.game.process_guess(payload.guess.clone()) {
            Ok(correct) => {
                let mut new_balance = None;
                
                // Check if game is over (either by winning or running out of guesses)
                if correct {
                    session.ended = true;
                    
                    // Calculate game time in seconds
                    let game_time_seconds = (now - session.created_at) as i32;
                    
                    // Log game completion with win
                    info!("🎮 Word game ended for user {}: WIN! ✅ Word '{}' correctly guessed in {} seconds with {} guesses", 
                          username, session.game.secret_word, game_time_seconds, session.game.guesses.len());
                    
                    // Update word game stats
                    if let Err(e) = update_word_game_stats(
                        &app_state.pool,
                        user_id.0,
                        true, // win
                        Some(game_time_seconds)
                    ).await {
                        error!("Failed to update word game stats for user {}: {:?}", username, e);
                    }
                    
                    // Clear the active game key and set WIN cooldown
                    let mut redis_conn = state.redis.get_async_connection().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let active_game_key = format!("word_game:active:{}", user_id.0);
                    let cooldown_key = format!("word_game:cooldown:{}", user_id.0);
                    let _ : () = redis_conn.del(&active_game_key).await.unwrap_or(());
                    let _: () = redis::cmd("SETEX")
                        .arg(&cooldown_key)
                        .arg(WIN_COOLDOWN_SECONDS)  // 23 hours cooldown for wins
                        .arg("1")
                        .query_async(&mut redis_conn)
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    // Award rewards only if won
                    let reward_request = crate::services::claim_service::GameRewardRequest {
                        session_token: session.game_session_token.clone(),
                        game_type: "word".to_string(),
                        score: 25,  // Increased from 10 to 25 pax
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        milestone_id: None,
                    };

                    if let Ok(reward_resp) = crate::services::claim_service::handle_game_reward(
                        State(app_state.clone()),
                        Extension(user_id),
                        Json(reward_request)
                    ).await {
                        new_balance = Some(reward_resp.new_balance as f64);
                        
                        // Also award 1 scroll
                        let scroll_request = crate::services::claim_service::GameRewardRequest {
                            session_token: session.game_session_token.clone(),
                            game_type: "word".to_string(),
                            score: 1,
                            timestamp: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                            milestone_id: None,
                        };

                        if let Err(e) = crate::services::claim_service::handle_game_scroll_reward(
                            State(app_state),
                            Extension(user_id),
                            Json(scroll_request)
                        ).await {
                            error!("Failed to process scroll reward for user {}: {:?}", username, e);
                        }
                    }
                } else if (session.game.guesses.len() as u32) >= session.game.allowed_guesses || session.game.remaining_guesses == 0 {
                    session.ended = true;
                    
                    // Keep this log message (game ended)
                    info!("🎮 Word game ended for user {}: LOSS! ❌ Ran out of guesses. The word was '{}'", 
                          username, session.game.secret_word);
                    
                    // Update word game stats
                    if let Err(e) = update_word_game_stats(
                        &app_state.pool,
                        user_id.0,
                        false, // loss
                        None
                    ).await {
                        error!("Failed to update word game stats for user {}: {:?}", username, e);
                    }
                    
                    // Clear the active game key and set LOSS cooldown
                    let mut redis_conn = state.redis.get_async_connection().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let active_game_key = format!("word_game:active:{}", user_id.0);
                    let cooldown_key = format!("word_game:cooldown:{}", user_id.0);
                    let _ : () = redis_conn.del(&active_game_key).await.unwrap_or(());
                    let _: () = redis::cmd("SETEX")
                        .arg(&cooldown_key)
                        .arg(LOSS_COOLDOWN_SECONDS)  // 30 second cooldown for losses
                        .arg("1")
                        .query_async(&mut redis_conn)
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }

                // Compute tile evaluations
                let normalized_guess = payload.guess.trim().to_lowercase();
                let secret = session.game.secret_word.to_lowercase();
                let mut tile_results = Vec::new();
                
                for (i, ch) in normalized_guess.chars().enumerate() {
                    let status = if secret.chars().nth(i) == Some(ch) {
                        "green"
                    } else if secret.contains(ch) {
                        "yellow"
                    } else {
                        "gray"
                    };
                    tile_results.push(LetterTile { letter: ch, status: status.to_string() });
                }

                let mut public_game = session.game.to_public();
                if session.ended {
                    public_game.solution = Some(session.game.secret_word.clone());
                }

                let message = if correct {
                    format!("Correct! You've solved the puzzle. You can start a new game in {} minutes.", WIN_COOLDOWN_SECONDS / 60)
                } else if (session.game.guesses.len() as u32) >= session.game.allowed_guesses || session.game.remaining_guesses == 0 {
                    format!("No more guesses left. The word was '{}'. You can start a new game in {} seconds.", session.game.secret_word, LOSS_COOLDOWN_SECONDS)
                } else {
                    "Incorrect guess. Try again.".to_string()
                };

                let guess_resp = GuessResponse {
                    correct,
                    game: public_game,
                    message,
                    tiles: tile_results,
                    new_balance,
                };

                Ok(Json(guess_resp))
            },
            Err(err_msg) => {
                Ok(Json(GuessResponse {
                    correct: false,
                    game: session.game.to_public(),
                    message: err_msg,
                    tiles: vec![],
                    new_balance: None,
                }))
            },
        }
    } else {
        error!("🎮 Session not found for user {} with session_id {}", username, payload.session_id);
        Err(StatusCode::NOT_FOUND)
    }
}

// Refresh game state (returns public state)
async fn refresh(
    State(state): State<Arc<WordGameState>>,
    Extension(app_state): Extension<AppState>,
    Extension(user_id): Extension<UserId>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<RefreshQuery>,
) -> Result<Json<RefreshResponse>, StatusCode> {
    // Get username for logging
    let username = match sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&app_state.pool)
    .await {
        Ok(user) => user.username,
        Err(e) => {
            error!("Failed to fetch username: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let mut sessions = state.sessions.lock().await; // mutable lock to update session if needed
    if let Some(session) = sessions.get_mut(&query.session_id) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        // If the game has already ended (e.g., due to timeout), return game-over state immediately
        if session.ended {
            let mut public_game = session.game.to_public();
            public_game.solution = Some(session.game.secret_word.clone());
            return Ok(Json(RefreshResponse {
                game: public_game
            }));
        }
        
        // If game has expired due to time, mark it as ended immediately
        if now - session.created_at >= GAME_TIMER_SECONDS {
            // Mark game as over by filling guess history if not already full
            while (session.game.guesses.len() as u32) < session.game.allowed_guesses {
                session.game.guesses.push(String::new());
                session.game.tiles_history.push(vec![]);
            }
            session.ended = true;

            // Log game completion with loss due to timeout
            info!("🎮 Word game ended for user {}: LOSS! Timed out after {} seconds. The word was '{}'", 
                  username, now - session.created_at, session.game.secret_word);
            
            // Update word game stats for timeout loss
            if let Err(e) = update_word_game_stats(
                &app_state.pool,
                user_id.0,
                false, // loss
                None
            ).await {
                error!("Failed to update word game stats for user {} after timeout: {:?}", username, e);
            }

            // Clear the active game key and set cooldown
            if let Ok(mut redis_conn) = state.redis.get_async_connection().await {
                let active_game_key = format!("word_game:active:{}", user_id.0);
                let cooldown_key = format!("word_game:cooldown:{}", user_id.0);
                let _ : () = redis_conn.del(&active_game_key).await.unwrap_or(());
                let _: () = redis::cmd("SETEX")
                    .arg(&cooldown_key)
                    .arg(LOSS_COOLDOWN_SECONDS)  // 30 second cooldown
                    .arg("1")
                    .query_async(&mut redis_conn)
                    .await
                    .unwrap_or(());
            } else {
                error!("Failed to get Redis connection for user {} during refresh", username);
            }
            
            let mut public_game = session.game.to_public();
            public_game.solution = Some(session.game.secret_word.clone());
            return Ok(Json(RefreshResponse {
                game: public_game
            }));
        }
        // Also enforce session expiry (30 minutes)
        if now - session.created_at >= SESSION_EXPIRY_SECONDS {
            info!("Session expired for user {}", username);
            return Err(StatusCode::GONE);
        }
        
        let expected_sig = match compute_signature(&format!("session:{}", &query.session_id)) {
            Ok(sig) => sig,
            Err(e) => {
                error!("Failed to compute signature for user {}: {:?}", username, e);
                return Err(e);
            }
        };
        
        let session_sig = headers.get("X-Session-Signature")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                error!("Missing session signature for user {}", username);
                StatusCode::UNAUTHORIZED
            })?;
            
        if session_sig != expected_sig {
            error!("Invalid session signature for user {}", username);
            return Err(StatusCode::FORBIDDEN);
        }
        
        Ok(Json(RefreshResponse {
            game: session.game.to_public()
        }))
    } else {
        error!("Session not found for user {} with session_id {}", username, query.session_id);
        Err(StatusCode::NOT_FOUND)
    }
}

// Add this new endpoint handler before the router setup
async fn get_active_game(
    State(state): State<Arc<WordGameState>>,
    Extension(app_state): Extension<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<NewWordGameResponse>, (StatusCode, String)> {
    // Get username for logging
    let username = match sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&app_state.pool)
    .await {
        Ok(user) => user.username,
        Err(e) => {
            error!("Failed to fetch username: {:?}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()));
        }
    };

    let mut redis_conn = match state.redis.get_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to get Redis connection for user {}: {:?}", username, e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to connect to Redis".to_string()));
        }
    };

    let active_game_key = format!("word_game:active:{}", user_id.0);
    
    // Check if user has an active game
    match redis_conn.get::<_, Option<String>>(&active_game_key).await {
        Ok(Some(_)) => {
            // User has an active game, find it in the sessions
            let sessions = state.sessions.lock().await;
            
            // Find the session belonging to this user
            if let Some((session_id, session)) = sessions.iter().find(|(_, session)| {
                // Find the most recently created session for this user
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - session.created_at
                    < GAME_TIMER_SECONDS
            }) {
                let session_signature = compute_signature(&format!("session:{}", session_id))
                    .map_err(|e| (e, "Failed to compute signature".to_string()))?;
                
                let mut public_game = session.game.to_public();
                public_game.created_at = Some(session.created_at);
                
                return Ok(Json(NewWordGameResponse {
                    session_id: session_id.clone(),
                    session_signature,
                    game: public_game,
                }));
            }
            
            // If we get here, we found an active game in Redis but not in memory
            // This can happen if the server was restarted - clear the Redis key
            let _: Result<(), redis::RedisError> = redis_conn.del(&active_game_key).await;
            Err((StatusCode::NOT_FOUND, "No active game found".to_string()))
        },
        Ok(None) => {
            Err((StatusCode::NOT_FOUND, "No active game found".to_string()))
        },
        Err(e) => {
            error!("Redis error while checking active game for user {}: {:?}", username, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to check active game".to_string()))
        }
    }
}

// Add this new endpoint handler before the router setup
async fn get_cooldown_status(
    State(_state): State<Arc<WordGameState>>,
    Extension(app_state): Extension<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<CooldownStatus>, StatusCode> {
    // Check if user is a member
    match crate::generator::generate_code::is_member(&app_state.pool, user_id.0).await {
        Ok(is_member) => {
            if !is_member {
                return Ok(Json(CooldownStatus {
                    in_cooldown: true,
                    remaining_seconds: None,
                    is_win_cooldown: false,
                    requires_membership: true,
                }));
            }
        },
        Err(e) => {
            error!("Database error checking membership: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Check cooldown in Redis
    let mut redis_conn = app_state.redis.get_async_connection().await.map_err(|e| {
        error!("Redis error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    let cooldown_key = format!("word_game:cooldown:{}", user_id.0);
    
    match redis_conn.ttl::<_, i64>(&cooldown_key).await {
        Ok(ttl) => {
            if ttl > 0 {
                // If TTL is greater than LOSS_COOLDOWN_SECONDS, it must be a win cooldown
                let is_win_cooldown = ttl > LOSS_COOLDOWN_SECONDS as i64;
                Ok(Json(CooldownStatus {
                    in_cooldown: true,
                    remaining_seconds: Some(ttl),
                    is_win_cooldown,
                    requires_membership: false,
                }))
            } else {
                Ok(Json(CooldownStatus {
                    in_cooldown: false,
                    remaining_seconds: None,
                    is_win_cooldown: false,
                    requires_membership: false,
                }))
            }
        },
        Err(e) => {
            error!("Redis error while checking cooldown for user {}: {:?}", user_id.0, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Add this function after the WordGameState implementation
async fn update_word_game_stats(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    is_win: bool,
    game_time_seconds: Option<i32>,
) -> Result<(), sqlx::Error> {
    // Format today's date as YYYY-MM-DD string for SQL
    let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
    
    // Get username for logging
    let username = match sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(pool)
    .await {
        Ok(user) => user.username,
        Err(_) => "unknown".to_string(),
    };
    
    // First, check if the user already has stats
    let existing_stats = sqlx::query!(
        r#"
        SELECT 
            user_id, 
            current_streak, 
            highest_streak, 
            last_played_date,
            fastest_time,
            total_words_guessed,
            total_games_played
        FROM word_game_stats 
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    
    match existing_stats {
        Some(stats) => {
            // User has existing stats, update them
            let last_played_date = stats.last_played_date;
            
            // Calculate new streak
            let (new_current_streak, new_highest_streak) = if is_win {
                // Check if last played date was yesterday or today
                let streak_continued = match last_played_date {
                    Some(date) => {
                        // Get yesterday's date as string
                        let yesterday = chrono::Utc::now()
                            .checked_sub_days(chrono::Days::new(1))
                            .map(|d| d.format("%Y-%m-%d").to_string());
                        
                        let date_str = date.to_string();
                        date_str == today_str || Some(date_str) == yesterday
                    },
                    None => false
                };
                
                if streak_continued {
                    // Continuing streak
                    let new_streak = stats.current_streak + 1;
                    let highest = if new_streak > stats.highest_streak {
                        new_streak
                    } else {
                        stats.highest_streak
                    };
                    (new_streak, highest)
                } else {
                    // Streak broken, start new streak at 1
                    (1, stats.highest_streak)
                }
            } else {
                // Loss doesn't affect streak
                (stats.current_streak, stats.highest_streak)
            };
            
            // Update fastest time ONLY if this is a win and either there's no previous fastest time
            // or this time is faster
            let new_fastest_time = if is_win {
                match (stats.fastest_time, game_time_seconds) {
                    (Some(current), Some(new)) if new < current => {
                        info!("🏆 New fastest time for {}: {} seconds (previous: {} seconds)", 
                              username, new, current);
                        Some(new)
                    },
                    (None, Some(new)) => {
                        info!("🏆 First fastest time for {}: {} seconds", username, new);
                        Some(new)
                    },
                    _ => stats.fastest_time,
                }
            } else {
                // Never update fastest time on a loss
                stats.fastest_time
            };
            
            // Use execute instead of query! to avoid type issues
            let query = format!(
                "UPDATE word_game_stats
                SET 
                    current_streak = $1,
                    highest_streak = $2,
                    last_played_date = $3::date,
                    fastest_time = $4,
                    total_words_guessed = total_words_guessed + $5,
                    total_games_played = total_games_played + 1,
                    updated_at = CURRENT_TIMESTAMP
                WHERE user_id = $6"
            );
            
            let words_guessed = if is_win { 1 } else { 0 };
            
            let _result = sqlx::query(&query)
                .bind(new_current_streak)
                .bind(new_highest_streak)
                .bind(today_str)
                .bind(new_fastest_time)
                .bind(words_guessed)
                .bind(user_id)
                .execute(pool)
                .await?;
                
            if is_win {
                info!("📊 Word game stats updated for {}: Win! Current streak: {}, Highest streak: {}, Words guessed: {}, Games played: {}", 
                      username, new_current_streak, new_highest_streak, stats.total_words_guessed + 1, stats.total_games_played + 1);
            } else {
                info!("📊 Word game stats updated for {}: Loss. Current streak: {}, Highest streak: {}, Games played: {}", 
                      username, new_current_streak, new_highest_streak, stats.total_games_played + 1);
            }
        },
        None => {
            // User has no stats yet, create a new entry
            let query = format!(
                "INSERT INTO word_game_stats (
                    user_id,
                    current_streak,
                    highest_streak,
                    last_played_date,
                    fastest_time,
                    total_words_guessed,
                    total_games_played,
                    created_at,
                    updated_at
                ) VALUES ($1, $2, $3, $4::date, $5, $6, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
            );
            
            let initial_streak = if is_win { 1 } else { 0 };
            let words_guessed = if is_win { 1 } else { 0 };
            
            let _result = sqlx::query(&query)
                .bind(user_id)
                .bind(initial_streak)
                .bind(initial_streak)
                .bind(today_str)
                .bind(if is_win { game_time_seconds } else { None::<i32> }) // Only record time for wins
                .bind(words_guessed)
                .execute(pool)
                .await?;
                
            if is_win {
                info!("📊 First word game stats created for {}: Win! Streak: 1, Words guessed: 1, Games played: 1", username);
            } else {
                info!("📊 First word game stats created for {}: Loss. Streak: 0, Games played: 1", username);
            }
        }
    }
    
    Ok(())
}

// Add this function to get the leaderboard data
async fn get_word_leaderboard(
    Extension(app_state): Extension<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<WordLeaderboardEntry>>, StatusCode> {
    let limit = params.get("limit")
        .and_then(|l| l.parse::<i64>().ok())
        .unwrap_or(10);
    
    // First, let's check if there are any entries in the table at all
    let count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM word_game_stats WHERE total_words_guessed > 0"
    )
    .fetch_one(&app_state.pool)
    .await
    .map_err(|e| {
        error!("Database error when counting word game stats: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .unwrap_or(0); // Handle NULL case, though it should never happen
    
    if count == 0 {
        return Ok(Json(vec![]));
    }
    
    // Fetch leaderboard entries
    let entries = sqlx::query_as!(
        WordLeaderboardEntry,
        r#"
        SELECT 
            u.username,
            wgs.current_streak,
            wgs.highest_streak,
            wgs.fastest_time,
            wgs.total_words_guessed,
            wgs.total_games_played,
            wgs.updated_at::text as "updated_at!"
        FROM 
            word_game_stats wgs
        JOIN 
            users u ON wgs.user_id = u.id
        WHERE 
            wgs.total_words_guessed > 0
        ORDER BY 
            wgs.total_words_guessed DESC,
            wgs.highest_streak DESC,
            wgs.fastest_time ASC NULLS LAST
        LIMIT $1
        "#,
        limit
    )
    .fetch_all(&app_state.pool)
    .await
    .map_err(|e| {
        error!("Database error when fetching word game leaderboard: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    Ok(Json(entries))
}

// Add a debug endpoint to check a specific user's stats
async fn get_my_stats(
    Extension(app_state): Extension<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<Option<WordLeaderboardEntry>>, StatusCode> {
    // Get username for the current user
    let username = match sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&app_state.pool)
    .await {
        Ok(user) => user.username,
        Err(e) => {
            error!("Failed to fetch username: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    info!("📊 Fetching personal word game stats for user: {}", username);
    
    // Fetch the user's stats
    let stats = sqlx::query_as!(
        WordLeaderboardEntry,
        r#"
        SELECT 
            u.username as "username!",
            w.current_streak,
            w.highest_streak,
            w.fastest_time,
            w.total_words_guessed,
            w.total_games_played,
            w.updated_at::TEXT as "updated_at!"
        FROM word_game_stats w
        JOIN users u ON w.user_id = u.id
        WHERE w.user_id = $1
        "#,
        user_id.0
    )
    .fetch_optional(&app_state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch user stats: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    if let Some(stats) = &stats {
        info!("📊 Stats found for {}: Games played: {}, Words guessed: {}, Current streak: {}, Highest streak: {}, Fastest time: {:?}s",
              username, stats.total_games_played, stats.total_words_guessed, 
              stats.current_streak, stats.highest_streak, stats.fastest_time);
    } else {
        info!("📊 No word game stats found for user {} - They haven't played any games yet", username);
    }
    
    Ok(Json(stats))
}

// === Router Setup ===

pub fn create_router() -> Router<Arc<WordGameState>> {
    Router::new()
        .route("/new", post(new_game))
        .route("/guess", post(guess))
        .route("/refresh", get(refresh))
        .route("/active", get(get_active_game))
        .route("/cooldown", get(get_cooldown_status))
        .route("/leaderboard", get(get_word_leaderboard))
        .route("/my-stats", get(get_my_stats))
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