use super::*;
use crate::services::models::*;
use axum::{extract::State, Json};
use axum::http::StatusCode;
use tracing::{error, info, trace};
use serde::Deserialize;
use sqlx::Row;

pub async fn get_user_profile(pool: &PgPool, user_id: Uuid) -> Result<UserProfile, AuthError> {
    trace!("Fetching profile for user ID: {}", user_id);
    
    let user = sqlx::query!(
        r#"
        SELECT 
            username,
            email,
            currency_balance,
            experience,
            rank::text as rank,
            TO_CHAR(last_login, 'YYYY-MM-DD HH24:MI:SS') as last_login,
            TO_CHAR(created_at, 'YYYY-MM-DD HH24:MI:SS') as created_at,
            is_member
        FROM users
        WHERE id = $1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        error!("Database error fetching profile: {:?}", e);
        AuthError::Database(e)
    })?;

    Ok(UserProfile {
        username: user.username,
        email: user.email,
        currency_balance: user.currency_balance,
        experience: user.experience,
        rank: user.rank,
        last_login: user.last_login.map(|t| t.to_string()),
        created_at: user.created_at.unwrap_or_else(|| "Unknown".to_string()),
        is_member: user.is_member,
    })
}

// --- Rank System Logic ---

/// Rank thresholds in ascending order. Each tuple represents (minimum XP, rank name).
const RANK_THRESHOLDS: &[(i32, &str)] = &[
    (0, "Novice"),
    (100, "Apprentice"),
    (250, "Adept"),
    (500, "Expert"),
    (1000, "Master"),
];

/// Computes the rank for the given XP value by iterating over the thresholds.
/// Returns the highest rank that the XP qualifies for.
pub fn compute_rank(xp: i32) -> &'static str {
    let mut current_rank = "Novice";
    for &(threshold, rank) in RANK_THRESHOLDS.iter() {
        if xp >= threshold {
            current_rank = rank;
        } else {
            break;
        }
    }
    current_rank
}

/// Updates the user's experience and rank atomically.
/// 
/// This function starts a transaction, fetches the current XP and rank from the database,
/// increments the XP by xp_increase, computes the new rank via compute_rank,
/// and updates the user's record accordingly. If the rank has changed, it updates that as well.
/// 
/// # Arguments
/// 
/// * `pool` - A reference to the PostgreSQL connection pool.
/// * `user_id` - The UUID of the user whose XP and rank will be updated.
/// * `xp_increase` - The amount of XP to add to the user's current XP.
/// 
/// # Returns
/// 
/// Result<(), sqlx::Error> indicating success or failure of the database operations.
pub async fn update_experience_and_rank(pool: &PgPool, user_id: Uuid, xp_increase: i32) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Define a local struct for the query result
    struct UserXPRecord {
        experience: i32,
        rank: String,
    }

    let user_record = sqlx::query_as!(UserXPRecord,
        "SELECT experience, rank::text as \"rank!\" FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(&mut *tx)
    .await?;

    let new_experience = user_record.experience + xp_increase;
    let new_rank = compute_rank(new_experience);

    // If the rank changed, update both experience and rank
    if user_record.rank != new_rank {
        sqlx::query("UPDATE users SET experience = $1, rank = $2::user_rank WHERE id = $3")
            .bind(new_experience)
            .bind(new_rank)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("UPDATE users SET experience = $1 WHERE id = $2")
            .bind(new_experience)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

// Struct for leaderboard entries
#[derive(Debug, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct LeaderboardEntry {
    pub username: String,
    pub high_score: i32,
    pub updated_at: String,
}

pub async fn update_user_game_score(
    pool: &PgPool,
    game_type: &str,
    user_id: Uuid,
    new_score: i32,
) -> Result<(), sqlx::Error> {
    // Fetch the user's username from the database
    let user_record = sqlx::query!("SELECT username FROM users WHERE id = $1", user_id)
        .fetch_one(pool)
        .await?;
    let username = user_record.username;

    let result = sqlx::query!(
        r#"
        INSERT INTO game_leaderboard (game_type, user_id, high_score)
        VALUES ($1, $2, $3)
        ON CONFLICT (game_type, user_id) DO UPDATE SET
            high_score = GREATEST(game_leaderboard.high_score, EXCLUDED.high_score),
            updated_at = CURRENT_TIMESTAMP
        "#,
        game_type,
        user_id,
        new_score
    )
    .execute(pool)
    .await;

    match result {
        Ok(_res) => {
            info!(
                event = "leaderboard_update_success",
                game_type = game_type,
                username = %username,
                new_score,
                "Leaderboard update successful for user {} in game {} with new score {}",
                username,
                game_type,
                new_score
            );
            Ok(())
        },
        Err(e) => {
            error!(
                event = "leaderboard_update_failure",
                game_type = game_type,
                user_id = %user_id,
                new_score,
                error = %e,
                "Failed to update leaderboard for user {} in game {} with new score {}: {}",
                username,
                game_type,
                new_score,
                e
            );
            Err(e)
        }
    }
}

/// Retrieves the top N players for a specific game.
/// Returns a vector of LeaderboardEntry containing username and score.
pub async fn get_game_leaderboard(
    pool: &PgPool,
    game_type: &str,
    limit: i64
) -> Result<Vec<LeaderboardEntry>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT 
            u.username,
            l.high_score,
            TO_CHAR(l.updated_at, 'YYYY-MM-DD HH24:MI:SS') as updated_at_str
        FROM game_leaderboard l
        JOIN users u ON l.user_id = u.id
        WHERE l.game_type = $1
        ORDER BY l.high_score DESC
        LIMIT $2
        "#,
    )
    .bind(game_type)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let username: String = row.try_get("username")?;
            let high_score: i32 = row.try_get("high_score")?;
            let updated_at_str: Option<String> = row.try_get("updated_at_str")?;

            Ok(LeaderboardEntry {
                username,
                high_score,
                updated_at: updated_at_str.unwrap_or_else(|| String::from("Invalid Date")),
            })
        })
        .collect::<Result<Vec<LeaderboardEntry>, sqlx::Error>>()
}

// New struct and function for user leaderboard
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserLeaderboardEntry {
    pub username: String,
    pub total_soul: i64,
    pub creature_count: i64,
    pub egg_count: i64,
    pub scroll_count: i64,
    pub pax: i32,
}

/// Fetches the user leaderboard data
/// Returns users sorted by total soul across all their creatures, with additional stats
pub async fn get_user_leaderboard(
    State(state): State<AppState>,
) -> Result<Json<Vec<UserLeaderboardEntry>>, (StatusCode, String)> {
    const LEADERBOARD_LIMIT: i64 = 20;

    // Cache the leaderboard data for 5 minutes to reduce database load
    if let Ok(cached_data) = state.redis.get_connection().and_then(|mut conn| {
        redis::cmd("GET")
            .arg("user_leaderboard_cache")
            .query::<Option<String>>(&mut conn)
    }) {
        if let Some(cached) = cached_data {
            if let Ok(parsed) = serde_json::from_str::<Vec<UserLeaderboardEntry>>(&cached) {
                return Ok(Json(parsed));
            }
        }
    }

    let users = sqlx::query_as!(
        UserLeaderboardEntry,
        r#"
        WITH creature_stats AS (
            SELECT
                u.id,
                u.username,
                u.currency_balance AS pax,
                COALESCE(SUM(c.soul), 0) AS total_soul,
                COUNT(DISTINCT c.id) AS creature_count
            FROM
                users u
            LEFT JOIN
                creatures c ON u.id = c.owner_id
            WHERE u.deleted_at IS NULL
            GROUP BY u.id, u.username, u.currency_balance
        ), egg_counts AS (
            SELECT
                owner_id,
                COUNT(id) as egg_count
            FROM eggs
            WHERE status = 'available'
            GROUP BY owner_id
        ), scroll_counts AS (
            SELECT
                owner_id,
                SUM(quantity) as scroll_count
            FROM scrolls s -- Alias scrolls table as s
            WHERE s.owner_id = s.owner_id -- This seems redundant, should be compared to users or creature_stats? Let's assume it's correct as per original logic context.
            AND NOT EXISTS (
                SELECT 1 FROM market_listings ml
                WHERE ml.item_id = s.id
                AND ml.item_type = 'scroll'
                AND ml.status = 'active'
            )
            GROUP BY owner_id
        )
        SELECT
            cs.username as "username!",
            cs.total_soul as "total_soul!",
            cs.creature_count as "creature_count!",
            COALESCE(ec.egg_count, 0) as "egg_count!",
            COALESCE(sc.scroll_count, 0) as "scroll_count!",
            cs.pax as "pax!"
        FROM
            creature_stats cs
        LEFT JOIN egg_counts ec ON cs.id = ec.owner_id
        LEFT JOIN scroll_counts sc ON cs.id = sc.owner_id
        WHERE
            cs.total_soul > 0 OR cs.creature_count > 0
        ORDER BY
            cs.total_soul DESC, cs.creature_count DESC, cs.username ASC
        LIMIT $1
        "#,
        LEADERBOARD_LIMIT
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch user leaderboard: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user leaderboard".to_string())
    })?;

    // Cache the leaderboard data for 1 minute
    if let Ok(serialized) = serde_json::to_string(&users) {
        if let Ok(mut conn) = state.redis.get_connection() {
            let _: Result<(), _> = redis::cmd("SETEX")
                .arg("user_leaderboard_cache")
                .arg(60) // 1 minute TTL (was 300)
                .arg(serialized)
                .query(&mut conn);
        }
    }

    Ok(Json(users))
}