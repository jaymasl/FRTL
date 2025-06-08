use axum::{
    extract::{State, Json, Extension, Path},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::AppState;
use crate::auth::middleware::UserId;
use tracing::{error, info};
use crate::models::{DisplayItem, Egg, Creature as ModelCreature};

#[derive(Debug, sqlx::FromRow)]
struct OwnerCheck {
    owner_id: Uuid,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateListingRequest {
    pub item_id: Uuid,
    pub item_type: String,
    pub price: i32,
    pub quantity: i32,
}

#[derive(Debug, Serialize)]
pub struct MarketListing {
    pub id: Uuid,
    pub seller_id: Uuid,
    pub seller_username: String,
    pub item_id: Uuid,
    pub item_type: String,
    pub price: i32,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ServiceCreature {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub original_egg_id: Option<Uuid>,
    pub original_egg_summoned_by: Option<Uuid>,
    pub hatched_by: Uuid,
    pub egg_summoned_by_username: Option<String>,
    pub hatched_by_username: Option<String>,
    pub owner_username: Option<String>,
    pub essence: String,
    pub color: String,
    pub art_style: String,
    pub animal: String,
    pub rarity: String,
    pub energy_full: bool,
    pub energy_recharge_complete_at: Option<String>,
    pub streak: i32,
    pub soul: i32,
    pub image_path: String,
    pub display_name: String,
    pub prompt: Option<String>,
    pub stats: serde_json::Value,
    pub original_egg_image_path: String,
    pub hatched_at: String,
    pub original_egg_created_at: String,
    pub in_chaos_realm: bool,
    pub chaos_realm_entry_at: Option<String>,
    pub chaos_realm_reward_claimed: bool,
}

// Create a new market listing
pub async fn create_listing(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(payload): Json<CreateListingRequest>,
) -> Result<Json<ApiResponse<MarketListing>>, StatusCode> {
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Failed to begin transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get seller's username first so we can use it in all logs
    let seller = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch seller username: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("📋 User {} is creating a new listing for {} {} at {} PAX", 
          seller.username, payload.item_type, payload.item_id, payload.price);

    let owner_check = match payload.item_type.as_str() {
        "egg" => sqlx::query_as!(
            OwnerCheck,
            "SELECT owner_id, status::text as status FROM eggs WHERE id = $1",
            payload.item_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to check egg ownership: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?,
        "creature" => sqlx::query_as!(
            OwnerCheck,
            "SELECT owner_id, status::text as status FROM creatures WHERE id = $1",
            payload.item_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to check creature ownership: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?,
        _ => {
            info!("❌ Listing creation failed: Invalid item type '{}' specified by user {}", 
                  payload.item_type, seller.username);
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid item type".to_string()),
            }));
        }
    };

    let owner_check = match owner_check {
        Some(check) => check,
        None => {
            info!("❌ Listing creation failed: Item {} not found for user {}", 
                  payload.item_id, seller.username);
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Item not found".to_string()),
            }));
        }
    };

    if owner_check.owner_id != user_id.0 {
        info!("❌ Listing creation failed: User {} doesn't own {} {}", 
              seller.username, payload.item_type, payload.item_id);
        return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("You don't own this item".to_string()),
        }));
    }

    if owner_check.status.as_deref() != Some("available") {
        info!("❌ Listing creation failed: {} {} is not available for listing (status: {})", 
              payload.item_type, payload.item_id, owner_check.status.unwrap_or_default());
        return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Item is not available for listing".to_string()),
        }));
    }

    // Check if user has enough currency for listing fee
    let listing_fee = 5; // 5 PAX flat fee
    let user = sqlx::query!(
        "SELECT currency_balance FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch user balance: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if user.currency_balance < listing_fee {
        info!("❌ Listing creation failed: User {} has insufficient funds for listing fee (has {} PAX, needs {} PAX)", 
              seller.username, user.currency_balance, listing_fee);
        return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Insufficient funds for listing fee (5 PAX required)".to_string()),
        }));
    }

    // Deduct listing fee
    sqlx::query!(
        "UPDATE users SET currency_balance = currency_balance - $1 WHERE id = $2",
        listing_fee,
        user_id.0
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to deduct listing fee: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let listing = sqlx::query!(
        "INSERT INTO market_listings (seller_id, item_id, item_type, price, quantity, status, type)
        VALUES ($1, $2, $3, $4, $5, 'active'::market_status_type, 'sale'::market_type)
        RETURNING id, seller_id, item_id, item_type, price, created_at",
        user_id.0,
        payload.item_id,
        payload.item_type,
        payload.price,
        payload.quantity
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to create market listing: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match payload.item_type.as_str() {
        "egg" => {
            sqlx::query!(
                "UPDATE eggs SET status = 'locked'::item_status WHERE id = $1",
                payload.item_id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to update egg status: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        "creature" => {
            sqlx::query!(
                "UPDATE creatures SET status = 'locked'::item_status WHERE id = $1",
                payload.item_id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to update creature status: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        _ => unreachable!(),
    };

    sqlx::query!(
        "INSERT INTO item_events (item_id, item_type, event_type, from_user_id, performed_by_user_id)
        VALUES ($1, $2, 'listed_for_sale'::event_type, $3, $3)",
        payload.item_id,
        payload.item_type,
        user_id.0,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to create item event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("✅ Listing created successfully: User {} listed {} {} for {} PAX (listing ID: {})", 
          seller.username, payload.item_type, payload.item_id, payload.price, listing.id);

    Ok(Json(ApiResponse {
        success: true,
        data: Some(MarketListing {
            id: listing.id,
            seller_id: listing.seller_id,
            seller_username: seller.username,
            item_id: listing.item_id,
            item_type: listing.item_type,
            price: listing.price,
            status: "active".to_string(),
            created_at: listing.created_at.to_string(),
        }),
        error: None,
    }))
}

// Get all active listings
pub async fn get_active_listings(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<MarketListing>>>, StatusCode> {
    let listings = sqlx::query!(
        r#"
        SELECT DISTINCT m.id, m.seller_id, u.username as seller_username, m.item_id, m.item_type, m.price, m.created_at
        FROM market_listings m
        JOIN users u ON m.seller_id = u.id
        LEFT JOIN eggs e ON m.item_id = e.id AND m.item_type = 'egg'
        LEFT JOIN creatures c ON m.item_id = c.id AND m.item_type = 'creature'
        WHERE m.status = 'active'::market_status_type 
        AND m.type = 'sale'::market_type
        AND (
            (m.item_type = 'egg' AND e.id IS NOT NULL AND e.status = 'locked'::item_status)
            OR 
            (m.item_type = 'creature' AND c.id IS NOT NULL AND c.status = 'locked'::item_status)
        )
        ORDER BY m.created_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch market listings: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let listings = listings
        .into_iter()
        .map(|l| MarketListing {
            id: l.id,
            seller_id: l.seller_id,
            seller_username: l.seller_username,
            item_id: l.item_id,
            item_type: l.item_type,
            price: l.price,
            status: "active".to_string(),
            created_at: l.created_at.to_string(),
        })
        .collect();

    Ok(Json(ApiResponse {
        success: true,
        data: Some(listings),
        error: None,
    }))
}

// Purchase an item
pub async fn purchase_item(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(listing_id): Path<Uuid>,
) -> Result<Json<ApiResponse<i32>>, StatusCode> {
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Failed to begin transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 1. Get and validate listing with lock
    let listing = sqlx::query!(
        "SELECT seller_id, item_id, item_type, price
         FROM market_listings
         WHERE id = $1 AND status = 'active'::market_status_type
         FOR UPDATE",
        listing_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch listing: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let listing = match listing {
        Some(l) => l,
        None => return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Listing not found or not active".to_string()),
        })),
    };

    if listing.seller_id == user_id.0 {
        return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("You cannot buy your own listing".to_string()),
        }));
    }

    // Get buyer and seller usernames for logging
    let buyer_username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch buyer username: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .username;

    let seller_username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        listing.seller_id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch seller username: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .username;

    info!("🛒 User {} is attempting to purchase item {} from user {} for {} PAX", 
          buyer_username, listing.item_id, seller_username, listing.price);

    // 2. Verify item exists and is still locked
    match listing.item_type.as_str() {
        "egg" => {
            let item = sqlx::query!(
                "SELECT status::text as status 
                 FROM eggs 
                 WHERE id = $1 
                 AND status = 'locked'::item_status 
                 FOR UPDATE",
                listing.item_id
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to fetch egg status: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            if item.is_none() {
                return Ok(Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some("Item is no longer available".to_string()),
                }));
            }
        }
        "creature" => {
            let item = sqlx::query!(
                "SELECT status::text as status 
                 FROM creatures 
                 WHERE id = $1 
                 AND status = 'locked'::item_status 
                 FOR UPDATE",
                listing.item_id
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to fetch creature status: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            if item.is_none() {
                return Ok(Json(ApiResponse {
                    success: false,
                    data: None,
                    error: Some("Item is no longer available".to_string()),
                }));
            }
        }
        _ => return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Invalid item type".to_string()),
        })),
    }

    // 3. Lock and check buyer's balance first
    let buyer = sqlx::query!(
        "SELECT currency_balance 
         FROM users 
         WHERE id = $1 
         FOR UPDATE",
        user_id.0
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch buyer balance: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let buyer = match buyer {
        Some(b) => b,
        None => return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Buyer not found".to_string()),
        })),
    };

    if buyer.currency_balance < listing.price {
        info!("❌ Purchase failed: User {} has insufficient funds to purchase item from {}", 
              buyer_username, seller_username);
        return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Insufficient funds".to_string()),
        }));
    }

    // 4. Update buyer's balance
    let new_balance = sqlx::query!(
        "UPDATE users 
         SET currency_balance = currency_balance - $1 
         WHERE id = $2
         RETURNING currency_balance",
        listing.price,
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update buyer balance: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .currency_balance;

    // 5. Update seller's balance
    sqlx::query!(
        "UPDATE users 
         SET currency_balance = currency_balance + $1 
         WHERE id = $2",
        listing.price,
        listing.seller_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update seller balance: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 6. Transfer item ownership
    match listing.item_type.as_str() {
        "egg" => {
            sqlx::query!(
                "UPDATE eggs 
                 SET owner_id = $1, status = 'available'::item_status 
                 WHERE id = $2",
                user_id.0,
                listing.item_id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to transfer egg ownership: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        "creature" => {
            sqlx::query!(
                "UPDATE creatures 
                 SET owner_id = $1, status = 'available'::item_status 
                 WHERE id = $2",
                user_id.0,
                listing.item_id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to transfer creature ownership: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        _ => unreachable!(),
    }

    // 7. Update listing status
    sqlx::query!(
        "UPDATE market_listings 
         SET status = 'completed'::market_status_type, buyer_id = $1 
         WHERE id = $2",
        user_id.0,
        listing_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update listing status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 8. Record the event
    sqlx::query!(
        "INSERT INTO item_events (item_id, item_type, event_type, from_user_id, to_user_id, performed_by_user_id)
         VALUES ($1, $2, 'sold'::event_type, $3, $4, $4)",
        listing.item_id,
        listing.item_type,
        listing.seller_id,
        user_id.0,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to record item event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("✅ Purchase successful: User {} bought {} {} from {} for {} PAX", 
          buyer_username, listing.item_type, listing.item_id, seller_username, listing.price);

    Ok(Json(ApiResponse {
        success: true,
        data: Some(new_balance),
        error: None,
    }))
}

// Cancel a listing
pub async fn cancel_listing(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(listing_id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Failed to begin transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get user's username for logging
    let username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch username: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .username;

    info!("🔄 User {} is attempting to cancel listing {}", username, listing_id);

    let listing = sqlx::query!(
        "SELECT seller_id, item_id, item_type
         FROM market_listings
         WHERE id = $1 AND status = 'active'::market_status_type",
        listing_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to fetch listing: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let listing = match listing {
        Some(l) => l,
        None => {
            info!("❌ Cancellation failed: Listing {} not found or not active", listing_id);
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Listing not found or not active".to_string()),
            }));
        }
    };

    if listing.seller_id != user_id.0 {
        info!("❌ Cancellation failed: User {} attempted to cancel listing {} they don't own", 
              username, listing_id);
        return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("You can only cancel your own listings".to_string()),
        }));
    }

    match listing.item_type.as_str() {
        "egg" => {
            sqlx::query!(
                "UPDATE eggs SET status = 'available'::item_status WHERE id = $1",
                listing.item_id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to update egg status: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        "creature" => {
            sqlx::query!(
                "UPDATE creatures SET status = 'available'::item_status WHERE id = $1",
                listing.item_id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to update creature status: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
        _ => {
            info!("❌ Cancellation failed: Invalid item type for listing {}", listing_id);
            return Ok(Json(ApiResponse {
                success: false,
                data: None,
                error: Some("Invalid item type".to_string()),
            }));
        }
    }

    sqlx::query!(
        "UPDATE market_listings SET status = 'cancelled'::market_status_type WHERE id = $1",
        listing_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to update listing status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    sqlx::query!(
        "INSERT INTO item_events (item_id, item_type, event_type, from_user_id, performed_by_user_id)
        VALUES ($1, $2, 'sale_cancelled'::event_type, $3, $3)",
        listing.item_id,
        listing.item_type,
        user_id.0,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to record item event: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("✅ Cancellation successful: User {} cancelled listing {} for {} {}", 
          username, listing_id, listing.item_type, listing.item_id);

    Ok(Json(ApiResponse {
        success: true,
        data: None,
        error: None,
    }))
}

// Add this new function
pub async fn get_listing_item(
    State(state): State<AppState>,
    Path(listing_id): Path<Uuid>,
) -> Result<Json<ApiResponse<DisplayItem>>, StatusCode> {
    let listing = sqlx::query!(
        r#"
        SELECT m.item_id, m.item_type, m.status::text as status
        FROM market_listings m
        WHERE m.id = $1 
        AND m.status = 'active'::market_status_type 
        AND m.type = 'sale'::market_type
        "#,
        listing_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch listing: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let listing = match listing {
        Some(l) => l,
        None => return Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Listing not found or not active".to_string()),
        })),
    };

    let item = match listing.item_type.as_str() {
        "egg" => {
            let egg = sqlx::query_as!(
                Egg,
                r#"SELECT 
                    e.id,
                    e.owner_id,
                    e.summoned_by,
                    u_summoner.username as "summoned_by_username",
                    u_owner.username as "owner_username",
                    e.essence::text as "essence!",
                    e.color::text as "color!",
                    e.art_style::text as "art_style!",
                    e.image_path as "image_path!",
                    e.display_name as "display_name!",
                    e.prompt,
                    TO_CHAR(e.created_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') as "created_at!",
                    TO_CHAR(e.incubation_ends_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') as "incubation_ends_at!"
                FROM eggs e
                LEFT JOIN users u_summoner ON e.summoned_by = u_summoner.id
                LEFT JOIN users u_owner ON e.owner_id = u_owner.id
                WHERE e.id = $1 AND e.status = 'locked'::item_status"#,
                listing.item_id
            )
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| {
                error!("Failed to fetch egg: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            egg.map(DisplayItem::Egg)
        }
        "creature" => {
            let creature = sqlx::query_as!(
                ServiceCreature,
                r#"SELECT 
                    c.id,
                    c.owner_id,
                    c.original_egg_id,
                    c.original_egg_summoned_by,
                    c.hatched_by,
                    u1.username as "egg_summoned_by_username",
                    u2.username as "hatched_by_username",
                    u3.username as "owner_username",
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
                    c.original_egg_created_at::text as "original_egg_created_at!",
                    c.in_chaos_realm,
                    c.chaos_realm_entry_at::text as "chaos_realm_entry_at",
                    c.chaos_realm_reward_claimed
                FROM creatures c
                JOIN users u1 ON c.original_egg_summoned_by = u1.id
                JOIN users u2 ON c.hatched_by = u2.id
                JOIN users u3 ON c.owner_id = u3.id
                WHERE c.id = $1 AND c.status = 'locked'::item_status"#,
                listing.item_id
            )
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| {
                error!("Failed to fetch creature: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            creature.map(|c| DisplayItem::Creature(ModelCreature {
                id: c.id,
                owner_id: c.owner_id,
                original_egg_id: c.original_egg_id,
                original_egg_summoned_by: c.original_egg_summoned_by,
                hatched_by: c.hatched_by,
                egg_summoned_by_username: c.egg_summoned_by_username,
                hatched_by_username: c.hatched_by_username,
                owner_username: c.owner_username,
                essence: c.essence,
                color: c.color,
                art_style: c.art_style,
                animal: c.animal,
                rarity: c.rarity,
                energy_full: c.energy_full,
                energy_recharge_complete_at: c.energy_recharge_complete_at,
                streak: c.streak,
                soul: c.soul,
                image_path: c.image_path,
                display_name: c.display_name,
                prompt: c.prompt,
                stats: c.stats,
                original_egg_image_path: c.original_egg_image_path,
                hatched_at: c.hatched_at,
                original_egg_created_at: c.original_egg_created_at,
                in_chaos_realm: c.in_chaos_realm,
                chaos_realm_entry_at: c.chaos_realm_entry_at,
                chaos_realm_reward_claimed: c.chaos_realm_reward_claimed,
            }))
        }
        _ => None,
    };

    match item {
        Some(item) => Ok(Json(ApiResponse {
            success: true,
            data: Some(item),
            error: None,
        })),
        None => Ok(Json(ApiResponse {
            success: false,
            data: None,
            error: Some("Item not found or not available".to_string()),
        })),
    }
}

impl From<ServiceCreature> for DisplayItem {
    fn from(c: ServiceCreature) -> Self {
        DisplayItem::Creature(ModelCreature {
            id: c.id,
            owner_id: c.owner_id,
            original_egg_id: c.original_egg_id,
            original_egg_summoned_by: c.original_egg_summoned_by,
            hatched_by: c.hatched_by,
            egg_summoned_by_username: c.egg_summoned_by_username,
            hatched_by_username: c.hatched_by_username,
            owner_username: c.owner_username,
            essence: c.essence,
            color: c.color,
            art_style: c.art_style,
            animal: c.animal,
            rarity: c.rarity,
            energy_full: c.energy_full,
            energy_recharge_complete_at: c.energy_recharge_complete_at,
            streak: c.streak,
            soul: c.soul,
            image_path: c.image_path,
            display_name: c.display_name,
            prompt: c.prompt,
            stats: c.stats,
            original_egg_image_path: c.original_egg_image_path,
            hatched_at: c.hatched_at,
            original_egg_created_at: c.original_egg_created_at,
            in_chaos_realm: c.in_chaos_realm,
            chaos_realm_entry_at: c.chaos_realm_entry_at,
            chaos_realm_reward_claimed: c.chaos_realm_reward_claimed,
        })
    }
} 