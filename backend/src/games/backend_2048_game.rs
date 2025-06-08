use axum::{
    extract::{State, Extension, Json},
    routing::{get, post},
    Router,
    http::StatusCode,
};
use uuid::Uuid;
use shared::shared_2048_game::{Game2048, PublicGame2048, Direction};
use std::{
    collections::HashMap,
    sync::Arc,
    env,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use tracing::{error, info};
use serde::{Serialize, Deserialize};
use axum::response::IntoResponse;
use tower_http::cors::CorsLayer;
use sqlx;

type HmacSha256 = Hmac<Sha256>;

const MAX_GAMES_PER_MINUTE: usize = 10;
// Rate limiting constants removed as rate limiting is disabled
const SESSION_EXPIRY_SECONDS: u64 = 1800;  // Increased to 30 minutes

#[derive(Clone)]
pub struct Game2048Session {
    pub game: Game2048,
    pub created_at: f64,
    pub last_move_time: f64,
    pub game_session_token: String,
    pub reward_claimed: bool,  // Add flag to track if reward has been claimed
}

#[derive(Clone)]
pub struct Game2048State {
    pub sessions: Arc<Mutex<HashMap<String, Game2048Session>>>,
}

fn compute_signature(message: &str) -> Result<String, StatusCode> {
    // Use environment variable, or default to a development secret
    let secret = env::var("GAME2048_SECRET").unwrap_or_else(|_| "default_2048_secret".to_string());
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    mac.update(message.as_bytes());
    let result = mac.finalize().into_bytes();
    Ok(hex::encode(result))
}

#[derive(Serialize)]
pub struct NewGame2048Response {
    pub session_id: String,
    pub session_signature: String,
    pub game: PublicGame2048,
}

async fn new_game(
    State(state): State<Arc<Game2048State>>,
    Extension(app_state): Extension<crate::AppState>,
    Extension(user_id): Extension<crate::auth::middleware::UserId>,
) -> Result<Json<NewGame2048Response>, StatusCode> {
    let mut sessions = state.sessions.lock().await;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    
    if sessions.values().filter(|s| now - s.created_at < 60.0).count() >= MAX_GAMES_PER_MINUTE {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    
    let session_request = crate::services::claim_service::GameSessionRequest {
        game_type: "2048".to_string(),
    };
    let (_, session_token) = crate::services::claim_service::create_game_session(
        State(app_state.clone()),
        Extension(user_id),
        Json(session_request)
    ).await.map_err(|e| {
        error!("Failed to create game session: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let game = Game2048::new((4, 4));
    let session_id = Uuid::new_v4().to_string();
    let session_signature = compute_signature(&format!("session:{}", session_id))?;
    
    sessions.insert(session_id.clone(), Game2048Session {
        game,
        created_at: now,
        last_move_time: now,
        game_session_token: session_token,
        reward_claimed: false,
    });
    
    Ok(Json(NewGame2048Response {
        session_id: session_id.clone(),
        session_signature,
        game: sessions.get(&session_id).unwrap().game.to_public(),
    }))
}

#[derive(Deserialize)]
pub struct MoveRequest {
    pub session_id: String,
    pub direction: Direction,
}

#[derive(Serialize)]
pub struct MoveResponse {
    pub moved: bool,
    pub game: PublicGame2048,
    pub score: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_balance: Option<i32>,
}

async fn process_move(
    State(state): State<Arc<Game2048State>>,
    Extension(app_state): Extension<crate::AppState>,
    Extension(user_id): Extension<crate::auth::middleware::UserId>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<MoveRequest>,
) -> Result<Json<MoveResponse>, axum::response::Response> {
    let mut sessions = state.sessions.lock().await;
    if let Some(session) = sessions.get_mut(&payload.session_id) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
        
        // Check session expiry
        if now - session.created_at >= SESSION_EXPIRY_SECONDS as f64 {
            sessions.remove(&payload.session_id);
            return Err(StatusCode::GONE.into_response());
        }

        if now - session.last_move_time < 0.2 {
            return Err(StatusCode::TOO_MANY_REQUESTS.into_response());
        }

        let expected_sig = compute_signature(&format!("session:{}", payload.session_id)).map_err(|e| e.into_response())?;
        let session_sig = headers.get("X-Session-Signature")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED.into_response())?;
        if session_sig != expected_sig {
            return Err(StatusCode::FORBIDDEN.into_response());
        }
        
        session.last_move_time = now;
        
        // Check if game is already over and prevent further moves
        if session.game.game_over {
            // Log if this is an attempt to claim reward again
            if session.reward_claimed {
                // Get username from database for better logging
                let username = sqlx::query!(
                    "SELECT username FROM users WHERE id = $1",
                    user_id.0
                )
                .fetch_one(&app_state.pool)
                .await
                .map(|record| record.username)
                .unwrap_or_else(|_| "unknown".to_string());
                
                info!("🚫 Prevented duplicate reward claim attempt for user {} on game session {}", 
                      username, payload.session_id);
            }
            
            return Ok(Json(MoveResponse {
                moved: false,
                game: session.game.to_public(),
                score: session.game.score,
                new_balance: None,
            }));
        }
        
        let moved = session.game.make_move(payload.direction);
        let mut new_balance = None;

        // Only process reward if the game just ended (game_over is true) AND reward hasn't been claimed yet
        if session.game.game_over && !session.reward_claimed {
            // Set the flag to prevent multiple rewards
            session.reward_claimed = true;
            
            // Calculate pax reward (1 pax per 50 score)
            let final_score = session.game.score;
            let pax_reward = (final_score / 50) as i32;
            
            if pax_reward > 0 {
                let reward_request = crate::services::claim_service::GameRewardRequest {
                    session_token: session.game_session_token.clone(),
                    game_type: "2048".to_string(),
                    score: pax_reward,
                    timestamp: now as u64,
                    milestone_id: None,
                };

                match crate::services::claim_service::handle_game_reward(
                    State(app_state.clone()),
                    Extension(user_id),
                    Json(reward_request)
                ).await {
                    Ok(reward_resp) => {
                        // Get username from database
                        let username = sqlx::query!(
                            "SELECT username FROM users WHERE id = $1",
                            user_id.0
                        )
                        .fetch_one(&app_state.pool)
                        .await
                        .map_err(|e| {
                            error!("Failed to fetch username: {:?}", e);
                            StatusCode::INTERNAL_SERVER_ERROR.into_response()
                        })?.username;
                        
                        // Update the user's game score in the leaderboard
                        let _ = crate::services::user_service::update_user_game_score(
                            &app_state.pool, 
                            "2048", 
                            user_id.0, 
                            final_score as i32
                        ).await;
                        
                        info!("🎮 Game over for {}! Final score: {}, awarded {} pax. New balance: {}", 
                            username, final_score, pax_reward, reward_resp.new_balance);
                        new_balance = Some(reward_resp.new_balance);
                    },
                    Err(e) => {
                        error!("Failed to process game over reward: {:?}", e);
                    }
                }
            }
        }
        
        Ok(Json(MoveResponse {
            moved,
            game: session.game.to_public(),
            score: session.game.score,
            new_balance,
        }))
    } else {
        Err(StatusCode::NOT_FOUND.into_response())
    }
}

#[derive(Deserialize)]
pub struct RefreshQuery {
    pub session_id: String,
}

#[derive(Serialize)]
pub struct RefreshResponse {
    pub game: PublicGame2048,
    pub score: u32,
}

async fn refresh(
    State(state): State<Arc<Game2048State>>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<RefreshQuery>,
) -> Result<Json<RefreshResponse>, StatusCode> {
    let sessions = state.sessions.lock().await;
    if let Some(session) = sessions.get(&query.session_id) {
        let expected_sig = compute_signature(&format!("session:{}", query.session_id))?;
        let session_sig = headers.get("X-Session-Signature")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        if session_sig != expected_sig {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(Json(RefreshResponse {
            game: session.game.to_public(),
            score: session.game.score,
        }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub fn create_router() -> Router<Arc<Game2048State>> {
    Router::new()
        .route("/new", post(new_game))
        .route("/move", post(process_move))
        .route("/refresh", get(refresh))
        .layer(CorsLayer::permissive())
        .layer(axum::middleware::from_fn(crate::auth::middleware::require_auth))
}
