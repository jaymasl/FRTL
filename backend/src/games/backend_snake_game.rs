use std::time::{Instant, Duration, SystemTime};
use axum::{
    extract::{State, Extension, ws::Message},
    response::IntoResponse,
    routing::get,
    Router,
    http::StatusCode,
    Json,
};
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json;
use std::{
    sync::Arc,
    collections::HashMap,
};
use tokio::sync::{Mutex, mpsc};
use log::{error, warn, info};
// use uuid::Uuid;
use crate::auth::middleware;
use crate::services::claim_service;
use sqlx;
use redis;

mod shared_snake_game {
    include!("../../../shared/src/shared_snake_game.rs");
}
use shared_snake_game::*;

const TICK_RATE: u64 = 100; // milliseconds
const GRID_SIZE: (u32, u32) = (20, 20);
const MIN_UPDATE_INTERVAL: Duration = Duration::from_millis(50); // Minimum time between direction changes
const MAX_MESSAGES_PER_SECOND: u32 = 50; // Increased from 20 to 50
const SESSION_TIMEOUT: Duration = Duration::from_secs(7200); // Increased from 1 hour to 2 hours
const MAX_CONCURRENT_GAMES: usize = 1000;

#[derive(Clone)]
pub struct GameSession {
    pub game: SnakeGame,
    pub last_update: Instant,
    pub ws_sender: mpsc::UnboundedSender<String>,
    pub direction_queue: Vec<Direction>,
    pub created_at: SystemTime,
    pub message_count: u32,
    pub last_message_time: SystemTime,
    pub game_session_token: String,
    pub user_id: uuid::Uuid,
}

#[derive(Clone)]
pub struct SnakeGameState {
    pub sessions: Arc<Mutex<HashMap<String, GameSession>>>,
    pub pool: sqlx::PgPool,
    pub redis: redis::Client,
}

impl SnakeGameState {
    async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.lock().await;
        let now = SystemTime::now();
        sessions.retain(|_, session| {
            if let Ok(duration) = now.duration_since(session.created_at) {
                duration < SESSION_TIMEOUT
            } else {
                false
            }
        });
    }

    async fn is_rate_limited(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            let now = SystemTime::now();
            if let Ok(duration) = now.duration_since(session.last_message_time) {
                if duration.as_secs() >= 1 {
                    // Reset counter for new second
                    session.message_count = 1;
                    session.last_message_time = now;
                    false
                } else {
                    // Increment counter and check limit
                    session.message_count += 1;
                    if session.message_count > MAX_MESSAGES_PER_SECOND {
                        warn!("Rate limit exceeded for session {}: {} messages in less than a second (limit: {})", 
                              session_id, session.message_count, MAX_MESSAGES_PER_SECOND);
                        
                        // Instead of blocking completely, just log and allow some messages through
                        // Only block if significantly over the limit
                        if session.message_count > MAX_MESSAGES_PER_SECOND * 2 {
                            return true;
                        }
                        false
                    } else {
                        false
                    }
                }
            } else {
                error!("SystemTime error in rate limiting for session {}", session_id);
                false // Don't rate limit on time errors
            }
        } else {
            warn!("Rate limit check for non-existent session {}", session_id);
            true
        }
    }
}

pub fn create_router() -> Router<Arc<SnakeGameState>> {
    Router::new()
        .route("/ws", get(ws_handler))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<SnakeGameState>>,
) -> impl IntoResponse {
    // Check if we've hit the maximum number of concurrent games
    let current_sessions = state.sessions.lock().await.len();
    if current_sessions >= MAX_CONCURRENT_GAMES {
        error!("Maximum concurrent games limit reached");
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<SnakeGameState>) {
    // Wait for auth message
    let user_id = match socket.recv().await {
        Some(Ok(Message::Text(token))) if token.starts_with("Bearer ") => {
            let token = token.trim_start_matches("Bearer ").trim();
            info!("Received auth token from WebSocket");
            match crate::auth::validate_jwt(token) {
                Ok(id) => id,
                Err(e) => {
                    error!("Invalid auth token: {:?}", e);
                    return;
                }
            }
        }
        other => {
            error!("Expected auth token as Text message starting with 'Bearer ', but received: {:?}", other);
            return;
        }
    };

    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Generate session ID
    let session_id = uuid::Uuid::new_v4().to_string();
    info!("New WebSocket connection, assigned session id: {}", session_id);
    
    // Create game session token
    let session_request = claim_service::GameSessionRequest {
        game_type: "snake".to_string(),
    };

    let app_state = crate::AppState {
        pool: state.pool.clone(),
        redis: state.redis.clone(),
    };

    let (_, session_token) = match claim_service::create_game_session(
        State(app_state),
        Extension(middleware::UserId(user_id)),
        Json(session_request),
    ).await {
        Ok((_, token)) => (StatusCode::OK, token),
        Err(e) => {
            error!("Failed to create game session: {:?}", e);
            return;
        }
    };
    
    // Create new game session
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(session_id.clone(), GameSession {
            game: SnakeGame::new(GRID_SIZE),
            last_update: Instant::now(),
            ws_sender: tx.clone(),
            direction_queue: Vec::new(),
            created_at: SystemTime::now(),
            message_count: 0,
            last_message_time: SystemTime::now(),
            game_session_token: session_token,
            user_id,
        });
        
        // Get username from database
        let username = match sqlx::query!(
            "SELECT username FROM users WHERE id = $1",
            user_id
        )
        .fetch_one(&state.pool)
        .await {
            Ok(record) => record.username,
            Err(_) => "unknown".to_string()
        };
        
        info!("🐍 Created snake game session with session id {} for user {}", session_id, username);
    }

    // Periodic cleanup of expired sessions
    let state_cleanup = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            state_cleanup.cleanup_expired_sessions().await;
        }
    });

    // Game loop task
    let game_state = state.clone();
    let session_id_clone = session_id.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_RATE));
        loop {
            interval.tick().await;
            let mut sessions = game_state.sessions.lock().await;
            if let Some(session) = sessions.get_mut(&session_id_clone) {
                if !session.game.started {
                    continue;
                }

                let now = std::time::Instant::now();
                if now.duration_since(session.last_update) >= MIN_UPDATE_INTERVAL {
                    if let Some(next_dir) = session.direction_queue.first().cloned() {
                        session.game.direction = next_dir;
                        session.direction_queue.remove(0);
                        session.last_update = now;
                    }
                }

                let food_eaten = session.game.update();

                if food_eaten {
                    match session.game.food.food_type {
                        FoodType::Regular => {
                            // Only award pax if score is divisible by 5
                            if session.game.score % 5 == 0 {
                                // Calculate progressive PAX reward based on score
                                let pax_reward = if session.game.score < 20 {
                                    1  // Base reward until score 15
                                } else if session.game.score < 35 {
                                    2  // Medium reward until score 25
                                } else if session.game.score < 60 {
                                    3  // Higher reward until score 45
                                } else {
                                    4  // Maximum reward for score 45+
                                };
                                
                                // Process regular pax reward
                                let reward_request = claim_service::GameRewardRequest {
                                    session_token: session.game_session_token.clone(),
                                    game_type: "snake".to_string(),
                                    score: pax_reward,  // Use the calculated progressive reward
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                    milestone_id: None,
                                };

                                let user_id = session.user_id;
                                let app_state = crate::AppState {
                                    pool: game_state.pool.clone(),
                                    redis: game_state.redis.clone(),
                                };

                                if let Ok(reward_resp) = claim_service::handle_game_reward(
                                    axum::extract::State(app_state),
                                    axum::extract::Extension(middleware::UserId(user_id)),
                                    axum::Json(reward_request)
                                ).await {
                                    session.game.new_balance = Some(reward_resp.new_balance as f64);
                                } else {
                                    error!("Failed to process snake game pax reward for session {}", session_id_clone);
                                }
                            }
                        },
                        FoodType::Scroll => {
                            // Process scroll reward
                            let reward_request = claim_service::GameRewardRequest {
                                session_token: session.game_session_token.clone(),
                                game_type: "snake".to_string(),
                                score: 1,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                                milestone_id: None,
                            };

                            let app_state = crate::AppState {
                                pool: game_state.pool.clone(),
                                redis: game_state.redis.clone(),
                            };

                            if let Ok(_) = claim_service::handle_game_scroll_reward(
                                axum::extract::State(app_state),
                                axum::extract::Extension(middleware::UserId(session.user_id)),
                                axum::Json(reward_request)
                            ).await {
                                info!("Successfully processed scroll reward for session {}", session_id_clone);
                                if let Ok(msg) = serde_json::to_string(&SnakeMessage::ScrollCollected) {
                                    if session.ws_sender.send(msg).is_err() {
                                        error!("Failed to send scroll collection message");
                                    }
                                }
                            } else {
                                error!("Failed to process snake game scroll reward for session {}", session_id_clone);
                            }
                        }
                    }

                    // Send game state update
                    if let Ok(game_json) = serde_json::to_string(&session.game) {
                        if session.ws_sender.send(game_json).is_err() {
                            error!("Failed to send game state update");
                            break;
                        }
                    }
                } else if let Some(session) = sessions.get(&session_id_clone) {
                    // Regular game update without food eaten
                    if let Ok(game_json) = serde_json::to_string(&session.game) {
                        if session.ws_sender.send(game_json).is_err() {
                            error!("Failed to send game state update");
                            break;
                        }
                    }
                }

                if let Some(session) = sessions.get(&session_id_clone) {
                    if session.game.game_over {
                        if let Ok(msg) = serde_json::to_string(&SnakeMessage::GameOver) {
                            if session.ws_sender.send(msg).is_err() {
                                error!("Failed to send game over message");
                            } else {
                                info!("Game over message sent for session {}, score: {}", session_id_clone, session.game.score);
                            }
                        }
                        // New call: update the user's game score in the leaderboard
                        let _ = crate::services::user_service::update_user_game_score(&game_state.pool, "snake", session.user_id, session.game.score as i32).await;
                        
                        // Don't remove the session immediately - let the client handle the game over state
                        // Instead, mark the session for cleanup after a delay
                        let cleanup_state = game_state.clone();
                        let cleanup_session_id = session_id_clone.clone();
                        tokio::spawn(async move {
                            // Wait 5 seconds before cleaning up the session
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            let mut sessions = cleanup_state.sessions.lock().await;
                            if sessions.remove(&cleanup_session_id).is_some() {
                                info!("Cleaned up game session {} after game over", cleanup_session_id);
                            }
                        });
                        
                        // Break the game loop but don't remove the session yet
                        break;
                    }
                } else {
                    // Session not found, break the loop
                    break;
                }
            } else {
                break;
            }
        }
    });

    // Handle incoming messages
    let state_clone = state.clone();
    let session_id_clone = session_id.clone();
    tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // Check rate limiting
            if state_clone.is_rate_limited(&session_id_clone).await {
                continue;
            }

            if let Ok(text) = msg.into_text() {
                // Validate message size
                if text.len() > 1024 {
                    warn!("Message too large from session {}", session_id_clone);
                    continue;
                }

                if let Ok(snake_msg) = serde_json::from_str::<SnakeMessage>(&text) {
                    let mut sessions = state_clone.sessions.lock().await;
                    if let Some(session) = sessions.get_mut(&session_id_clone) {
                        let now = Instant::now();
                        match snake_msg {
                            SnakeMessage::Start => {
                                session.game = SnakeGame::new(GRID_SIZE);
                                session.last_update = now;
                                // Do not start the game until a valid direction is received
                                // session.game.started remains false
                                if let Ok(game_json) = serde_json::to_string(&session.game) {
                                    if session.ws_sender.send(game_json).is_err() {
                                        error!("Failed to send initial game state");
                                    }
                                }
                            },
                            SnakeMessage::ChangeDirection(dir) => {
                                if !session.game.started {
                                    session.game.started = true;
                                    session.game.direction = dir;  // override the default direction with the first key press
                                    session.last_update = now;
                                } else {
                                    let current_dir = if session.direction_queue.is_empty() {
                                        session.game.direction
                                    } else {
                                        *session.direction_queue.last().unwrap()
                                    };
                                    if session.game.can_change_direction_from(current_dir, dir) {
                                        if session.direction_queue.last() != Some(&dir) {
                                            session.direction_queue.push(dir);
                                        }
                                    }
                                }
                            },
                            _ => {
                                warn!("Unexpected message type from session {}", session_id_clone);
                            }
                        }
                    }
                } else {
                    warn!("Invalid message format from session {}", session_id_clone);
                }
            }
        }
        // Clean up session when WebSocket closes
        let mut sessions = state_clone.sessions.lock().await;
        sessions.remove(&session_id_clone);
        info!("WebSocket closed and session {} removed", session_id_clone);
    });

    // Forward messages to WebSocket
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match sender.send(Message::Text(msg)).await {
                Ok(_) => {},
                Err(e) => {
                    error!("WebSocket send error: {:?} - Connection will be closed", e);
                    break;
                }
            }
        }
        // Attempt to close the connection gracefully
        if let Err(e) = sender.close().await {
            error!("Failed to close WebSocket connection gracefully: {:?}", e);
        }
        info!("WebSocket message forwarding task ended for session");
    });
} 