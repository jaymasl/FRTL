use std::path::PathBuf;
use axum::{extract::{State, Extension}, http::StatusCode, response::Json};
use reqwest::Client;
use serde_json::json;
use tracing::error;
use uuid::Uuid;
use time::OffsetDateTime;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use serde::Deserialize;
use tracing::info;

use crate::auth::middleware::UserId;
use crate::AppState;
use super::{Egg, EggResponse};
use super::prompts::{ArtStyle, EssenceType, Color};
use crate::services::scroll_service;

#[derive(Deserialize)]
pub struct GenerateEggRequest {
    scroll_id: Uuid,
}

#[derive(serde::Serialize)]
pub struct GenerateEggResponse {
    egg: EggResponse,
    new_balance: i32,
    remaining_scrolls: i32,
}

// Helper struct to hold image generation results
struct GeneratedImage {
    path: String,
    prompt: String,
    style: ArtStyle,
    essence: EssenceType,
    color: Color,
}

// Helper function to generate the egg image
async fn generate_egg_image() -> Result<GeneratedImage, StatusCode> {
    let api_key = std::env::var("TOGETHER_API_KEY").expect("TOGETHER_API_KEY must be set");
    let client = Client::new();

    let style = rand::random::<ArtStyle>();
    let essence = rand::random::<EssenceType>();
    let color = rand::random::<Color>();

    let prompt = format!(
        "full complete, {} colored egg, {}, {}, small letters 'FRTL' bottom corner, whole pristine unbroken",
        color.description(),
        essence.description(),
        style.description()
    );

    let response = client
        .post("https://api.together.xyz/v1/images/generations")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            "model": "black-forest-labs/FLUX.1-schnell",
            "prompt": prompt.clone(),
            "height": 1024,
            "width": 1024,
            "num_images": 1,
            "steps": 4,
            "seed": rand::random::<u32>()
        }))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to send request to API: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let status = response.status();
    if status.as_u16() == 429 {
        error!("Rate limit exceeded: Too many requests");
        return Err(StatusCode::TOO_MANY_REQUESTS);
    } else if !status.is_success() {
        error!("API returned error status: {}", status);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let json = response.json::<serde_json::Value>().await
        .map_err(|e| {
            error!("Failed to parse API response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if let Some(url) = json.get("data").and_then(|d| d[0]["url"].as_str()) {
        let image_bytes = client.get(url).send().await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .bytes().await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let filename = format!("egg_{}.jpg", Uuid::new_v4());
        let path = PathBuf::from("static/images/eggs").join(&filename);
        
        File::create(&path).await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .write_all(&image_bytes)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(GeneratedImage {
            path: format!("/static/images/eggs/{}", filename),
            prompt,
            style,
            essence,
            color,
        })
    } else {
        error!("No image URL in API response");
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

// Helper function to create the egg record in the database
async fn create_egg_record(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner_id: Uuid,
    image: GeneratedImage,
) -> Result<Egg, StatusCode> {
    let id = Uuid::new_v4();
    let now = OffsetDateTime::now_utc();
    let incubation_ends = now + time::Duration::seconds(82800);

    sqlx::query_as!(
        Egg,
        r#"
        INSERT INTO eggs (
            id, owner_id, summoned_by, essence, color, art_style, 
            created_at, incubation_ends_at, image_path, item_type,
            display_name, status, prompt
        )
        VALUES (
            $1, $2, $2,
            $3::text::essence_type, 
            $4::text::color_type, 
            $5::text::art_style_type, 
            $6, $7, $8, $9, $10,
            'available'::item_status,
            $11
        )
        RETURNING 
            id, 
            owner_id,
            summoned_by,
            essence::text as "essence!",
            color::text as "color!",
            art_style::text as "art_style!",
            image_path as "image_path!",
            display_name,
            prompt as "prompt!",
            incubation_ends_at as "incubation_ends_at!",
            created_at as "created_at!"
        "#,
        id, 
        owner_id, 
        image.essence.to_string(), 
        image.color.to_string(), 
        image.style.to_string(),
        now,
        incubation_ends,
        image.path, 
        "egg", 
        "Magical Egg", 
        image.prompt
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| {
        error!("Database error while inserting egg: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

pub async fn generate_egg(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(request): Json<GenerateEggRequest>,
) -> Result<Json<GenerateEggResponse>, StatusCode> {
    let owner_id = user_id.0;

    // First verify the user has both requirements (without consuming them)
    let user = sqlx::query!(
        "SELECT currency_balance FROM users WHERE id = $1",
        owner_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch user balance: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    if user.currency_balance < 55 {
        return Err(StatusCode::PAYMENT_REQUIRED);
    }

    // Check scroll availability
    let has_scroll = scroll_service::check_scroll_availability(&state.pool, request.scroll_id, owner_id)
        .await?;

    if !has_scroll {
        return Err(StatusCode::NOT_FOUND);
    }

    // Generate the image FIRST, before consuming any resources
    let generated_image = generate_egg_image().await?;

    // Store descriptions before moving generated_image
    let color_desc = generated_image.color.description();
    let essence_desc = generated_image.essence.description();

    // Only after we have the image, start the transaction to consume resources
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Failed to start transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Now consume resources and create egg in a single transaction
    let remaining_scrolls = scroll_service::consume_scroll(&mut tx, request.scroll_id).await?;
    
    let new_balance = user.currency_balance - 55;
    sqlx::query!(
        "UPDATE users SET currency_balance = $1 WHERE id = $2",
        new_balance,
        owner_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update balance: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create the egg record
    let egg = create_egg_record(&mut tx, owner_id, generated_image).await?;

    // Commit the transaction
    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Find the username of the summoner (also idempotent)
    let summoner = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch summoner username: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("🥚 {} successfully summoned a new {} {} egg", summoner.username, color_desc, essence_desc);

    // These operations are idempotent and can safely happen after commit
    crate::services::user_service::update_experience_and_rank(&state.pool, owner_id, 10).await
        .map_err(|e| {
            error!("Failed to update XP and rank: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(GenerateEggResponse {
        egg: egg.into_response(summoner.username),
        new_balance,
        remaining_scrolls,
    }))
}