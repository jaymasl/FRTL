use axum::{
    extract::{State, Extension},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use crate::auth::middleware::UserId;
use crate::AppState;
use tracing::info;
use time;

#[derive(Debug, Serialize)]
pub struct MagicButtonResponse {
    success: bool,
    reward_amount: Option<i32>,
    cooldown_remaining: i32,
    last_click: Option<Vec<LastClickInfo>>,
    new_balance: Option<i32>,
    total_clicks: i64,
}

#[derive(Debug, Serialize)]
pub struct LastClickInfo {
    username: String,
    clicked_at: String,
    reward_amount: i32,
}

pub async fn handle_magic_button(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<MagicButtonResponse>, StatusCode> {
    let mut redis_conn = state.redis.get_async_connection().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Check cooldown
    let cooldown_key = format!("magic_button:cooldown:{}", user_id.0);
    let cooldown: Option<i32> = redis::cmd("TTL")
        .arg(&cooldown_key)
        .query_async::<_, Option<i32>>(&mut redis_conn)
        .await
        .unwrap_or(None);

    if let Some(remaining) = cooldown {
        if remaining > 0 {
            // Fetch the last 3 clicks for cooldown response
            let last_clicks = sqlx::query!(
                r#"
                SELECT 
                    u.username,
                    mb.clicked_at,
                    mb.reward_amount
                FROM magic_button_clicks mb
                JOIN users u ON mb.user_id = u.id
                ORDER BY mb.clicked_at DESC
                LIMIT 3
                "#
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            // Get total clicks count
            let total_clicks = sqlx::query!(
                "SELECT COUNT(*) as count FROM magic_button_clicks"
            )
            .fetch_one(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .count
            .unwrap_or(0);

            return Ok(Json(MagicButtonResponse {
                success: false,
                reward_amount: None,
                cooldown_remaining: remaining,
                last_click: Some(last_clicks.into_iter().map(|click| LastClickInfo {
                    username: click.username,
                    clicked_at: format_timestamp(&click.clicked_at),
                    reward_amount: click.reward_amount,
                }).collect()),
                new_balance: None,
                total_clicks,
            }));
        }
    }

    // Generate fixed reward of 50 pax
    let reward = 50;

    // Start transaction
    let mut tx = state.pool.begin().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Record click and update user balance
    let new_balance = sqlx::query!(
        r#"
        WITH click_insert AS (
            INSERT INTO magic_button_clicks (user_id, reward_amount)
            VALUES ($1, $2)
        )
        UPDATE users 
        SET currency_balance = currency_balance + $2
        WHERE id = $1
        RETURNING currency_balance
        "#,
        user_id.0,
        reward
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .currency_balance;

    // Set cooldown (2 minutes)
    redis::cmd("SETEX")
        .arg(&cooldown_key)
        .arg(82800)
        .arg(1)
        .query_async::<_, ()>(&mut redis_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get username for logging
    let username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .username;

    // Log the click
    info!("🎯 {} clicked the magic button and received {} pax!", username, reward);

    // Fetch the last 3 clicks after successful click
    let last_clicks = sqlx::query!(
        r#"
        SELECT 
            u.username,
            mb.clicked_at,
            mb.reward_amount
        FROM magic_button_clicks mb
        JOIN users u ON mb.user_id = u.id
        ORDER BY mb.clicked_at DESC
        LIMIT 3
        "#
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get total clicks count after adding the new click
    let total_clicks = sqlx::query!(
        "SELECT COUNT(*) as count FROM magic_button_clicks"
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .count
    .unwrap_or(0);

    Ok(Json(MagicButtonResponse {
        success: true,
        reward_amount: Some(reward),
        cooldown_remaining: 82800,
        last_click: Some(last_clicks.into_iter().map(|click| LastClickInfo {
            username: click.username,
            clicked_at: format_timestamp(&click.clicked_at),
            reward_amount: click.reward_amount,
        }).collect()),
        new_balance: Some(new_balance),
        total_clicks,
    }))
}

pub async fn get_magic_button_status(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<MagicButtonResponse>, StatusCode> {
    let mut redis_conn = state.redis.get_async_connection().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Check cooldown
    let cooldown_key = format!("magic_button:cooldown:{}", user_id.0);
    let cooldown: Option<i32> = redis::cmd("TTL")
        .arg(&cooldown_key)
        .query_async::<_, Option<i32>>(&mut redis_conn)
        .await
        .unwrap_or(None);

    // Fetch the past 3 clicks
    let last_clicks = sqlx::query!(
        r#"
        SELECT 
            u.username,
            mb.clicked_at,
            mb.reward_amount
        FROM magic_button_clicks mb
        JOIN users u ON mb.user_id = u.id
        ORDER BY mb.clicked_at DESC
        LIMIT 3
        "#
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get total clicks count
    let total_clicks = sqlx::query!(
        "SELECT COUNT(*) as count FROM magic_button_clicks"
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .count
    .unwrap_or(0);

    Ok(Json(MagicButtonResponse {
        success: true,
        reward_amount: None,
        cooldown_remaining: cooldown.unwrap_or(0),
        last_click: Some(last_clicks.into_iter().map(|click| LastClickInfo {
            username: click.username,
            clicked_at: format_timestamp(&click.clicked_at),
            reward_amount: click.reward_amount,
        }).collect()),
        new_balance: None,
        total_clicks,
    }))
}

fn format_timestamp(timestamp: &time::OffsetDateTime) -> String {
    let now = time::OffsetDateTime::now_utc();
    let diff = now - *timestamp;
    
    if diff.whole_minutes() < 1 {
        return "just now".to_string();
    }
    
    // Always use hours format, even for timestamps older than 24 hours
    let total_hours = diff.whole_hours();
    let minutes = diff.whole_minutes() % 60;
    
    if total_hours > 0 {
        return format!("{}h {}m ago", total_hours, minutes);
    } else {
        return format!("{}m ago", minutes);
    }
} 