use axum::{
    extract::{State, Extension},
    Json,
    routing::{get, post},
    Router,
    debug_handler,
    http::StatusCode,
};
use serde::Deserialize;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use std::env;
use tracing::error;
use crate::auth::middleware::UserId;
use crate::AppState;

// Add constants for rate limiting and session expiration
const MAX_GAMES_PER_MINUTE: u32 = 5;
const SESSION_EXPIRY_SECONDS: u64 = 360;
const MAX_REVEALS_PER_MINUTE: u32 = 30;

use shared::shared_match_game::{
    Card,
    Color,
    MatchGame,
    NewGameResponse,
    RevealRequest,
    RevealResponse,
    RevealOneResponse,
    ColorVariant,
};

#[derive(Clone)]
pub struct GameSession {
    pub game: MatchGame,
    pub created_at: u64,
    pub reveal_count: u32,
    pub last_reveal_time: u64,
    pub game_session_token: String,
}

#[derive(Clone)]
pub struct GameState {
    pub sessions: Arc<Mutex<HashMap<String, GameSession>>>,
}

// Fix: make cleanup_expired_sessions async and properly handle the mutex
impl GameState {
    async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.lock().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        sessions.retain(|_, session| {
            now - session.created_at < SESSION_EXPIRY_SECONDS
        });
    }
}

#[derive(Deserialize)]
pub struct RefreshQuery {
    pub session_id: String,
}

#[derive(Deserialize)]
pub struct RevealOneQuery {
    pub session_id: String,
    pub card_index: usize,
}

async fn index() -> &'static str {
    "Welcome to the Backend Matching Game API (Axum)"
}

#[debug_handler]
async fn new_game(
    State(state): State<Arc<GameState>>,
    Extension(app_state): Extension<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<NewGameResponse>, StatusCode> {
    // Rate limiting check
    let mut sessions = state.sessions.lock().await;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let recent_games = sessions.values()
        .filter(|session| now - session.created_at < 60)
        .count();
    
    if recent_games >= MAX_GAMES_PER_MINUTE as usize {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Create game session first
    let session_request = crate::services::claim_service::GameSessionRequest {
        game_type: "match".to_string(),
    };

    let (_, session_token) = crate::services::claim_service::create_game_session(
        State(app_state.clone()),
        Extension(user_id),
        Json(session_request),
    ).await.map_err(|e| {
        error!("Failed to create game session: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get username from database
    let username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&app_state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch username: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?.username;

    // Create cards before getting the lock
    let mut cards = Vec::new();
    let colors = vec![Color::Red, Color::Blue, Color::Green, Color::Lime, Color::Purple, Color::Orange, Color::Pink, Color::Teal];
    let mut id = 0;
    
    // Determine if this game will have shiny gold cards (5% chance)
    let mut rng = thread_rng();
    let roll = rng.gen::<f32>();
    let is_shiny_game = roll < 0.05;
    
    // Enhanced logging with more visible formatting
    if is_shiny_game {
        tracing::info!("🎲 Shiny Match 🌟 User: {} - Roll: {:.4} - ✅ Shiny", username, roll);
    } else {
        tracing::info!("🎲 Regular Match - User: {} - Roll: {:.4} - ❌ Shiny", username, roll);
    }
    
    if is_shiny_game {
        // Add two shiny gold cards
        cards.push(Card::new(id, Color::Gold, ColorVariant::Shiny));
        id += 1;
        cards.push(Card::new(id, Color::Gold, ColorVariant::Shiny));
        id += 1;
        
        // Add remaining normal cards (7 pairs instead of 8)
        for color in colors.iter().take(7) {
            cards.push(Card::new(id, color.clone(), ColorVariant::Normal));
            id += 1;
            cards.push(Card::new(id, color.clone(), ColorVariant::Normal));
            id += 1;
        }
    } else {
        // Create two cards for each of the eight colors (normal variant)
        for color in colors {
            cards.push(Card::new(id, color.clone(), ColorVariant::Normal));
            id += 1;
            cards.push(Card::new(id, color.clone(), ColorVariant::Normal));
            id += 1;
        }
    }
    
    // Shuffle the cards
    {
        let mut rng = thread_rng();
        cards.shuffle(&mut rng);
    }
    
    let game = MatchGame::new(cards);
    let session_id = Uuid::new_v4().to_string();
    
    // Generate session signature
    let message = format!("session:{}", session_id);
    let session_signature = compute_signature(&message)?;
    
    sessions.insert(session_id.clone(), GameSession {
        game: game.clone(),
        created_at: now,
        reveal_count: 0,
        last_reveal_time: now,
        game_session_token: session_token.clone(),
    });
    
    Ok(Json(NewGameResponse {
        session_id,
        session_signature,
        game: game.to_public(),
    }))
}

fn compute_signature(message: &str) -> Result<String, StatusCode> {
    let secret = env::var("MATCH_GAME_SECRET")
        .map_err(|_| {
            log::error!("MATCH_GAME_SECRET environment variable not set");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    mac.update(message.as_bytes());
    let result = mac.finalize().into_bytes();
    Ok(hex::encode(result))
}

#[debug_handler]
async fn reveal(
    State(game_state): State<Arc<GameState>>,
    Extension(app_state): Extension<AppState>,
    Extension(user_id): Extension<UserId>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<RevealRequest>,
) -> Result<Json<RevealResponse>, StatusCode> {
    // Validate indices first
    if payload.first_index >= 16 || payload.second_index >= 16 || payload.first_index == payload.second_index {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Clean up expired sessions first
    game_state.cleanup_expired_sessions().await;

    let mut sessions = game_state.sessions.lock().await;
    
    if let Some(session) = sessions.get_mut(&payload.session_id) {
        // Verify this is a valid session by checking its signature
        let message = format!("session:{}", payload.session_id);
        let expected_sig = compute_signature(&message)?;
        
        // Get the session signature from the X-Session-Signature header
        let session_sig = headers
            .get("X-Session-Signature")
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
            
        if session_sig != expected_sig {
            return Err(StatusCode::FORBIDDEN);
        }
        
        if SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() - session.created_at >= SESSION_EXPIRY_SECONDS {
            sessions.remove(&payload.session_id);
            return Err(StatusCode::GONE);
        }

        // Apply rate limiting for reveal requests
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        if now - session.last_reveal_time < 60 {
            session.reveal_count += 1;
            if session.reveal_count > MAX_REVEALS_PER_MINUTE {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
        } else {
            session.reveal_count = 1;
            session.last_reveal_time = now;
        }

        let match_found = session.game.reveal_and_check(payload.first_index, payload.second_index, now);
        
        if match_found {
            // Drop the mutex lock before making the reward request
            let game_session_token = session.game_session_token.clone();
            
            // Check if this was a shiny gold match
            let is_shiny_match = session.game.cards[payload.first_index].variant == ColorVariant::Shiny &&
                               session.game.cards[payload.first_index].color == Color::Gold;
            
            drop(sessions);
            
            if is_shiny_match {
                // Award a scroll for matching shiny gold cards
                match crate::services::claim_service::handle_game_scroll_reward(
                    State(app_state.clone()),
                    Extension(user_id),
                    Json(crate::services::claim_service::GameRewardRequest {
                        session_token: game_session_token,
                        game_type: "match".to_string(),
                        score: 1,
                        timestamp: now,
                        milestone_id: None,
                    })
                ).await {
                    Ok(_) => {
                        let sessions = game_state.sessions.lock().await;
                        let session = sessions.get(&payload.session_id).unwrap();
                        Ok(Json(RevealResponse {
                            match_found,
                            score: session.game.score,
                            game: session.game.to_public(),
                            new_balance: None,
                        }))
                    },
                    Err(e) => Err(e)
                }
            } else {
                // Regular pax reward for normal matches
                // Create reward request with proper session token
                let reward_request = crate::services::claim_service::GameRewardRequest {
                    session_token: game_session_token,
                    game_type: "match".to_string(),
                    score: 1,
                    timestamp: now,
                    milestone_id: None,
                };

                // Send reward request
                match crate::services::claim_service::handle_game_reward(
                    State(app_state.clone()),
                    Extension(user_id.clone()),
                    Json(reward_request)
                ).await {
                    Ok(reward_resp) => {
                        let sessions = game_state.sessions.lock().await;
                        let session = sessions.get(&payload.session_id).unwrap();
                        
                        // Check if all pairs are matched after this match
                        let all_matched = session.game.cards.iter().all(|card| card.matched);
                        
                        if all_matched {
                            // Award bonus 2 pax for completing all matches
                            let bonus_request = crate::services::claim_service::GameRewardRequest {
                                session_token: session.game_session_token.clone(),
                                game_type: "match".to_string(),
                                score: 2,  // Bonus 2 pax
                                timestamp: now,
                                milestone_id: None,
                            };
                            
                            // Drop the mutex lock before making the bonus reward request
                            drop(sessions);
                            
                            match crate::services::claim_service::handle_game_reward(
                                State(app_state),
                                Extension(user_id),
                                Json(bonus_request)
                            ).await {
                                Ok(bonus_resp) => {
                                    let sessions = game_state.sessions.lock().await;
                                    let session = sessions.get(&payload.session_id).unwrap();
                                    Ok(Json(RevealResponse {
                                        match_found,
                                        score: session.game.score,
                                        game: session.game.to_public(),
                                        new_balance: Some(bonus_resp.new_balance),
                                    }))
                                },
                                Err(e) => {
                                    error!("Failed to process bonus reward: {:?}", e);
                                    // Return the original reward response if bonus fails
                                    let sessions = game_state.sessions.lock().await;
                                    let session = sessions.get(&payload.session_id).unwrap();
                                    Ok(Json(RevealResponse {
                                        match_found,
                                        score: session.game.score,
                                        game: session.game.to_public(),
                                        new_balance: Some(reward_resp.new_balance),
                                    }))
                                }
                            }
                        } else {
                            Ok(Json(RevealResponse {
                                match_found,
                                score: session.game.score,
                                game: session.game.to_public(),
                                new_balance: Some(reward_resp.new_balance),
                            }))
                        }
                    },
                    Err(e) => {
                        error!("Failed to process match reward: {:?}", e);
                        Err(e)
                    }
                }
            }
        } else {
            // If not a match, spawn a background task to hide the non-matching cards after 1 second
            let state_clone = game_state.clone();
            let session_id_clone = payload.session_id.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let updated_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                let mut sessions = state_clone.sessions.lock().await;
                if let Some(sess) = sessions.get_mut(&session_id_clone) {
                    sess.game.hide_unmatched(updated_time);
                }
            });
            
            Ok(Json(RevealResponse {
                match_found,
                score: session.game.score,
                game: session.game.to_public(),
                new_balance: None,
            }))
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[debug_handler]
async fn refresh(
    State(state): State<Arc<GameState>>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<RefreshQuery>,
) -> Result<Json<RevealOneResponse>, StatusCode> {
    let mut sessions = state.sessions.lock().await;
    if let Some(session) = sessions.get_mut(&query.session_id) {
        let message = format!("session:{}", query.session_id);
        let expected_sig = compute_signature(&message)?;
        let session_sig = headers
            .get("X-Session-Signature")
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        if session_sig != expected_sig {
            return Err(StatusCode::FORBIDDEN);
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        session.game.hide_unmatched(now);
        Ok(Json(RevealOneResponse {
            match_found: false,
            score: session.game.score,
            game: session.game.to_public(),
        }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[debug_handler]
async fn reveal_one(
    State(state): State<Arc<GameState>>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<RevealOneQuery>,
) -> Result<Json<RevealOneResponse>, StatusCode> {
    if query.card_index >= 16 {
         return Err(StatusCode::BAD_REQUEST);
    }
    
    let mut sessions = state.sessions.lock().await;
    
    if let Some(session) = sessions.get_mut(&query.session_id) {
         let message = format!("session:{}", query.session_id);
         let expected_sig = compute_signature(&message)?;
         let session_sig = headers
             .get("X-Session-Signature")
             .and_then(|value| value.to_str().ok())
             .ok_or(StatusCode::UNAUTHORIZED)?;
         if session_sig != expected_sig {
             return Err(StatusCode::FORBIDDEN);
         }
         
         // Reveal the single card
         if query.card_index < session.game.cards.len() {
             session.game.cards[query.card_index].revealed = true;
         }
         
         let mut public_game = session.game.to_public();
         if query.card_index < public_game.cards.len() {
             public_game.cards[query.card_index].revealed = true;
             public_game.cards[query.card_index].color = Some(session.game.cards[query.card_index].color.clone());
         }
         
         Ok(Json(RevealOneResponse {
             match_found: false,
             score: session.game.score,
             game: public_game,
         }))
    } else {
         Err(StatusCode::NOT_FOUND)
    }
}

pub fn create_router() -> Router<Arc<GameState>> {
    Router::new()
        .route("/", get(index))
        .route("/new", post(new_game))
        .route("/reveal", post(reveal))
        .route("/refresh", get(refresh))
        .route("/reveal_one", get(reveal_one))
        .layer(axum::middleware::from_fn(crate::auth::middleware::require_auth))
} 