use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::error;
use uuid::Uuid;
use time::OffsetDateTime;

use crate::AppState;
use crate::auth::middleware::UserId;

#[derive(Debug, Serialize, Deserialize)]
pub struct Scroll {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub created_at: OffsetDateTime,
    pub display_name: String,
    pub image_path: Option<String>,
    pub description: Option<String>,
    pub quantity: i32,
}

#[derive(Debug, Serialize)]
pub struct ScrollResponse {
    pub id: Uuid,
    pub display_name: String,
    pub image_path: Option<String>,
    pub description: Option<String>,
    pub quantity: i32,
    pub created_at: String,
}

impl From<Scroll> for ScrollResponse {
    fn from(scroll: Scroll) -> Self {
        ScrollResponse {
            id: scroll.id,
            display_name: scroll.display_name,
            image_path: scroll.image_path,
            description: scroll.description,
            quantity: scroll.quantity,
            created_at: scroll.created_at.to_string(),
        }
    }
}

// Get all scrolls for a user
pub async fn get_scrolls(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<Json<Vec<ScrollResponse>>, StatusCode> {
    let scrolls = sqlx::query_as!(
        Scroll,
        r#"
        SELECT 
            id,
            owner_id,
            created_at,
            display_name,
            image_path,
            description,
            quantity
        FROM scrolls 
        WHERE owner_id = $1 
        ORDER BY created_at DESC
        "#,
        user_id.0
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(scrolls.into_iter().map(ScrollResponse::from).collect()))
}

// Get a specific scroll by ID
pub async fn get_scroll_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ScrollResponse>, StatusCode> {
    let scroll = sqlx::query_as!(
        Scroll,
        r#"
        SELECT 
            id,
            owner_id,
            created_at,
            display_name,
            image_path,
            description,
            quantity
        FROM scrolls 
        WHERE id = $1
        "#,
        id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error: {}", e);
        StatusCode::NOT_FOUND
    })?;

    Ok(Json(scroll.into()))
}

// Check if a user has a specific scroll available
pub async fn check_scroll_availability(
    pool: &PgPool,
    scroll_id: Uuid,
    owner_id: Uuid,
) -> Result<bool, StatusCode> {
    let result = sqlx::query!(
        "SELECT quantity FROM scrolls WHERE id = $1 AND owner_id = $2",
        scroll_id,
        owner_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("Database error checking scroll availability: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(result.map_or(false, |r| r.quantity > 0))
}

// Consume one scroll (internal helper function)
pub async fn consume_scroll(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scroll_id: Uuid,
) -> Result<i32, StatusCode> {
    let scroll = sqlx::query!(
        "SELECT quantity FROM scrolls WHERE id = $1",
        scroll_id
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| {
        error!("Database error: {}", e);
        StatusCode::NOT_FOUND
    })?;

    if scroll.quantity < 1 {
        return Err(StatusCode::PAYMENT_REQUIRED);
    }

    // Either delete the scroll or decrement its quantity
    if scroll.quantity == 1 {
        sqlx::query!("DELETE FROM scrolls WHERE id = $1", scroll_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| {
                error!("Failed to delete scroll: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        Ok(0)
    } else {
        sqlx::query!(
            "UPDATE scrolls SET quantity = quantity - 1 WHERE id = $1",
            scroll_id
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            error!("Failed to update scroll quantity: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        Ok(scroll.quantity - 1)
    }
}