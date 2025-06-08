use axum::{
    extract::{State, Json, Extension},
    http::StatusCode,
};
use crate::AppState;
use crate::auth::middleware::UserId;
use crate::services::orderbook_service::models::{CreateOrderRequest, OrderResponse, OrderSide, ErrorResponse};
use tracing::error;
use tracing::info;

pub async fn create_order(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(payload): Json<CreateOrderRequest>,
) -> Result<Json<OrderResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("🚫 Failed to begin transaction: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Database transaction error".to_string() }))
    })?;

    // Get username for logging purposes - prefix with underscore to indicate intentional non-use
    let _username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_or_else(
        |e| {
            error!("🚫 Failed to get username for user {}: {}", user_id.0, e);
            "unknown".to_string()
        },
        |record| record.username
    );

    // Validate price
    if payload.price <= 0 || payload.price > 1_000_000_000 {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Price must be between 1 and 1,000,000,000 pax".to_string() })));
    }

    // Define the order creation fee
    const ORDER_CREATION_FEE: i32 = 5;

    // Get user's current currency balance and scroll count
    let record = sqlx::query!(
        "SELECT currency_balance, (SELECT COALESCE(SUM(quantity), 0) FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll') as scroll_count FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("🚫 Failed to get user balance: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to get user balance".to_string() }))
    })?;

    let currency_balance = record.currency_balance;
    let scroll_count = record.scroll_count.unwrap_or(0) as i32;

    // Check if user has enough currency for the order creation fee
    if currency_balance < ORDER_CREATION_FEE {
        return Err((StatusCode::PAYMENT_REQUIRED, Json(ErrorResponse { error: format!("Insufficient funds for order creation fee ({} pax)", ORDER_CREATION_FEE) })));
    }

    // Check if user has enough resources based on order type
    match payload.side {
        OrderSide::Buy => {
            // For buy orders, check if user has enough currency for both the fee and the order amount
            if currency_balance < payload.price + ORDER_CREATION_FEE {
                return Err((StatusCode::PAYMENT_REQUIRED, Json(ErrorResponse { error: format!("Insufficient funds for buy order and fee (need {} pax)", payload.price + ORDER_CREATION_FEE) })));
            }
        },
        OrderSide::Sell => {
            if scroll_count < 1 {
                return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "You don't have any scrolls to sell".to_string() })));
            }
        }
    }

    // Deduct the order creation fee
    sqlx::query!(
        "UPDATE users SET currency_balance = currency_balance - $1 WHERE id = $2",
        ORDER_CREATION_FEE,
        user_id.0
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("🚫 Failed to deduct order creation fee: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to deduct order creation fee".to_string() }))
    })?;

    // Create the order
    let order = sqlx::query!(
        "INSERT INTO scroll_orderbook (user_id, side, price, status) VALUES ($1, $2, $3, 'active') RETURNING id, created_at",
        user_id.0,
        payload.side as _,
        payload.price
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("🚫 Failed to create order: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to create order".to_string() }))
    })?;

    // For buy orders, reserve the currency
    if payload.side == OrderSide::Buy {
        sqlx::query!(
            "UPDATE users SET currency_balance = currency_balance - $1 WHERE id = $2",
            payload.price,
            user_id.0
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to reserve currency: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to reserve currency".to_string() }))
        })?;
    }

    // For sell orders, check for matching buy orders
    if payload.side == OrderSide::Sell {
        // Remove the scroll from inventory
        let scroll_exists = sqlx::query!(
            "SELECT id FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll'",
            user_id.0
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to check scroll existence: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to check scroll existence".to_string() }))
        })?;

        if let Some(scroll) = scroll_exists {
            // Update or delete the scroll
            let count_result = sqlx::query!(
                "SELECT quantity FROM scrolls WHERE id = $1",
                scroll.id
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| {
                error!("🚫 Failed to get scroll quantity: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to get scroll quantity".to_string() }))
            })?;

            if count_result.quantity > 1 {
                sqlx::query!(
                    "UPDATE scrolls SET quantity = quantity - 1 WHERE id = $1",
                    scroll.id
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("🚫 Failed to update scroll quantity: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to update scroll quantity".to_string() }))
                })?;
            } else {
                sqlx::query!(
                    "DELETE FROM scrolls WHERE id = $1",
                    scroll.id
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("🚫 Failed to delete scroll: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to delete scroll".to_string() }))
                })?;
            }
        }

        // Find matching buy orders
        // TODO: Implement matching logic for buy orders
    }

    // Get updated balance and scroll count
    let updated_record = sqlx::query!(
        "SELECT currency_balance, (SELECT COALESCE(SUM(quantity), 0) FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll') as scroll_count FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("🚫 Failed to get updated user balance: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to get updated user balance".to_string() }))
    })?;

    let new_balance = updated_record.currency_balance;
    let new_scroll_count = updated_record.scroll_count.unwrap_or(0) as i32;

    // Commit the transaction
    tx.commit().await.map_err(|e| {
        error!("🚫 Failed to commit transaction: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to commit transaction".to_string() }))
    })?;

    // Convert side to string for response
    let side_str = match payload.side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    };

    // Format created_at for response
    let created_at_str = order.created_at.to_string();

    // Log the successful order creation
    info!("📜 Order {} created successfully - {} placed a {} order for {} pax (fee: {} pax)", 
        order.id, _username, side_str, payload.price, ORDER_CREATION_FEE);

    Ok(Json(OrderResponse {
        id: order.id,
        user_id: user_id.0,
        side: side_str.to_string(),
        price: payload.price,
        status: "active".to_string(),
        created_at: created_at_str,
        currency_balance: new_balance,
        scroll_count: new_scroll_count,
    }))
} 