use crate::services::user_service::{self, UserLeaderboardEntry, LeaderboardEntry};
use axum::{
    extract::{State, Path},
    response::{Json},
    http::StatusCode,
};
use tracing::{error, debug};
use crate::AppState;

/// Handler to retrieve the user leaderboard
pub async fn user_leaderboard_handler(
    State(state): State<AppState>
) -> Result<Json<Vec<UserLeaderboardEntry>>, StatusCode> {
    match user_service::get_user_leaderboard(State(state)).await {
        Ok(entries) => Ok(entries),
        Err(e) => {
            error!("Failed to get user leaderboard: {}", e.1);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler to retrieve game leaderboards
pub async fn game_leaderboard_handler(
    State(state): State<AppState>,
    Path(game_type): Path<String>,
) -> Result<Json<Vec<LeaderboardEntry>>, StatusCode> {
    debug!("Entering game_leaderboard_handler for game_type: {}", game_type);
    let limit = 10;
    
    match user_service::get_game_leaderboard(&state.pool, &game_type, limit).await {
        Ok(entries) => {
            debug!("Successfully fetched {} leaderboard entries for game_type: {}", entries.len(), game_type);
            match serde_json::to_string(&entries) {
                 Ok(_) => debug!("Serialization check successful for game_type: {}", game_type),
                 Err(e) => {
                     error!("Serialization failed for game_type {}: {:?}", game_type, e);
                     return Err(StatusCode::INTERNAL_SERVER_ERROR);
                 }
             }
            Ok(Json(entries))
        },
        Err(e) => {
            error!("SQLx error in get_game_leaderboard for game {}: {:?}", game_type, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
} 