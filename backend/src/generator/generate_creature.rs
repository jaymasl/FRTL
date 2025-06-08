use axum::{extract::{State, Extension, Path}, http::StatusCode, response::IntoResponse};
use serde_json::json;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use std::path::PathBuf;
use tracing::error;
use uuid::Uuid;
use reqwest::Client;
use super::prompts::{EssenceType, AnimalType, ArtStyle, Color};
use crate::auth::middleware::UserId;
use crate::AppState;
use time::OffsetDateTime;
use axum::Json;
use super::Creature;
use redis;
use tracing::info;

pub async fn generate_creature(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(egg_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let rate_limit_key = format!("hatch_rate_limit:{}", user_id.0);
    
    if let Ok(mut redis_conn) = state.redis.get_async_connection().await {
        let ttl: Option<i64> = redis::cmd("TTL")
            .arg(&rate_limit_key)
            .query_async(&mut redis_conn)
            .await
            .unwrap_or(None);

        if let Some(ttl) = ttl {
            if ttl > 0 {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
        }

        let _: () = redis::cmd("SETEX")
            .arg(&rate_limit_key)
            .arg(5)
            .arg(1)
            .query_async(&mut redis_conn)
            .await
            .unwrap_or(());
    }

    let egg = sqlx::query!(
        r#"
        SELECT 
            id,
            owner_id,
            summoned_by,
            essence::text as "essence!",
            color::text as "color!",
            art_style::text as "art_style!",
            image_path as "image_path!",
            display_name as "display_name!",
            prompt as "prompt!",
            incubation_ends_at,
            created_at
        FROM eggs 
        WHERE id = $1 AND owner_id = $2
        "#,
        egg_id,
        user_id.0
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch egg: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or_else(|| StatusCode::NOT_FOUND)?;

    let now = OffsetDateTime::now_utc();
    if now < egg.incubation_ends_at {
        return Err(StatusCode::BAD_REQUEST);
    }

    let animal: AnimalType = rand::random();
    let essence: EssenceType = egg.essence.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let art_style: ArtStyle = egg.art_style.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let color: Color = egg.color.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let api_key = std::env::var("TOGETHER_API_KEY").expect("TOGETHER_API_KEY must be set");
    let client = Client::new();
    let prompt = format!(
        "{}, {}, one {} color {} animal single, full complete view whole, small letters 'FRTL' bottom corner",
        art_style.description(),
        essence.description(),
        color.description(),
        animal.description(),
    );

    let response = client
        .post("https://api.together.xyz/v1/images/generations")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": "black-forest-labs/FLUX.1-schnell",
            "prompt": prompt,
            "negative_prompt": "human, people, nudity, nsfw, signed signature",
            "steps": 4,
            "width": 1024,
            "height": 1024
        }))
        .send()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if response.status().as_u16() == 429 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let json: serde_json::Value = response.json().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let url = json["data"][0]["url"].as_str()
        .ok_or_else(|| StatusCode::INTERNAL_SERVER_ERROR)?;

    let image_bytes = client.get(url).send().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .bytes().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let filename = format!("creature_{}.jpg", Uuid::new_v4());
    let path = PathBuf::from("static/images/creatures").join(&filename);

    File::create(&path).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .write_all(&image_bytes)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let local_path = format!("/static/images/creatures/{}", filename);
    let display_name = format!("{} {}", essence.to_string(), animal.description());

    let default_stats = json!({
        "health": 1,
        "attack": 1,
        "speed": 1,
    });

    let mut tx = state.pool.begin().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result = sqlx::query!(
        r#"
        INSERT INTO creatures (
            owner_id, original_egg_id, original_egg_summoned_by, hatched_by,
            original_egg_created_at, essence, color, art_style, 
            animal, rarity, energy_full, energy_recharge_complete_at, streak, soul,
            image_path, display_name, prompt, stats,
            original_egg_image_path,
            hatched_at
        )
        VALUES ($1, $2, $3, $4, $5, 
                $6::text::essence_type, $7::text::color_type, $8::text::art_style_type,
                $9::text::animal_type, $10::text::rarity_type, false, $11, $12, 
                $13, $14, $15, $16, $17, $18, $19)
        RETURNING id
        "#,
        user_id.0,          // owner_id
        egg_id,             // original_egg_id
        egg.summoned_by,    // original_egg_summoned_by
        user_id.0,          // hatched_by
        egg.created_at,     // original_egg_created_at
        essence.to_string(),
        egg.color,
        egg.art_style,
        animal.to_string(),
        "Common",
        None::<time::OffsetDateTime>,  // energy_recharge_complete_at
        0,      // Starting streak
        0,      // Starting soul
        local_path,
        display_name,
        prompt,
        serde_json::to_value(&default_stats).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?,
        egg.image_path,
        now
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to insert creature: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let creature_id = result.id;

    sqlx::query!(
        "UPDATE eggs SET status = 'locked'::item_status WHERE id = $1",
        egg_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update egg status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await
        .map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Update user's experience and rank for hatching the creature (+10 XP)
    crate::services::user_service::update_experience_and_rank(&state.pool, user_id.0, 10).await.map_err(|e| {
        error!("Failed to update XP and rank: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let creature = sqlx::query_as!(
        Creature,
        r#"
        SELECT 
            c.id,
            c.owner_id,
            c.original_egg_id,
            c.original_egg_summoned_by,
            c.hatched_by,
            u1.username as "egg_summoned_by_username!",
            u2.username as "hatched_by_username!",
            c.essence::text as "essence!",
            c.color::text as "color!",
            c.art_style::text as "art_style!",
            c.animal::text as "animal!",
            c.rarity::text as "rarity!",
            c.energy_full,
            c.energy_recharge_complete_at::text as "energy_recharge_complete_at",
            c.streak,
            c.soul,
            c.image_path as "image_path!",
            c.display_name as "display_name!",
            c.prompt,
            c.stats,
            c.original_egg_image_path as "original_egg_image_path!",
            c.hatched_at::text as "hatched_at!",
            c.original_egg_created_at::text as "original_egg_created_at!"
        FROM creatures c
        JOIN users u1 ON c.original_egg_summoned_by = u1.id
        JOIN users u2 ON c.hatched_by = u2.id
        WHERE c.id = $1
        "#,
        creature_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch created creature: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get username for logging
    let username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch username: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?.username;

    info!("🐣 {} successfully hatched {} {}", username, creature.essence, creature.animal);

    Ok(Json(creature.into_response()))
}