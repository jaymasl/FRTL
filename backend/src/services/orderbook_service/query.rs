use axum::{
    extract::{State, Path, Extension},
    http::StatusCode,
    Json,
};
use crate::AppState;
use crate::auth::middleware::UserId;
use crate::services::orderbook_service::models::{AggregatedOrderResponse, CancelOrderResponse, FulfillOrderResponse, ErrorResponse};
use tracing::{info, error};
use uuid::Uuid;

pub async fn get_orders(
    State(state): State<AppState>,
) -> Result<Json<Vec<AggregatedOrderResponse>>, StatusCode> {
    let orders = sqlx::query!(
        r#"
        SELECT 
            o.id::text as "id!",
            o.user_id::text as "user_id!",
            o.side::text as "side!",
            o.price,
            u.username as "username!"
        FROM scroll_orderbook o
        JOIN users u ON o.user_id = u.id
        WHERE o.status = 'active'
        ORDER BY 
            CASE WHEN o.side = 'buy' THEN o.price END DESC,
            CASE WHEN o.side = 'sell' THEN o.price END ASC,
            o.created_at ASC
        "#
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        error!("🚫 Failed to fetch orders: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = orders.into_iter().map(|order| AggregatedOrderResponse {
        side: order.side,
        price: order.price,
        id: order.id,
        user_id: order.user_id,
        username: order.username,
    }).collect::<Vec<_>>();

    Ok(Json(response))
}

pub async fn cancel_order(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(order_id): Path<Uuid>,
) -> Result<Json<CancelOrderResponse>, StatusCode> {
    // Get the username for logging
    let username = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_or_else(
        |e| {
            error!("🚫 Failed to get username for user {}: {}", user_id.0, e);
            "unknown".to_string()
        },
        |record| record.username
    );

    info!("👛 Cancelling order {} for user {}", order_id, username);
    
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("🚫 Failed to begin transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get the order to cancel
    let order = sqlx::query!(
        r#"
        SELECT 
            o.user_id,
            o.side::text as "side!",
            o.status::text as "status!",
            u.username as "owner_username!"
        FROM scroll_orderbook o
        JOIN users u ON o.user_id = u.id
        WHERE o.id = $1
        "#,
        order_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        error!("🚫 Failed to fetch order: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let order = match order {
        Some(o) => o,
        None => {
            error!("🚫 Order not found: {}", order_id);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Verify ownership
    if order.user_id != user_id.0 {
        error!("🚫 User {} attempted to cancel order {} owned by {}", username, order_id, order.owner_username);
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify status
    if order.status != "active" {
        error!("🚫 Cannot cancel order with status: {}", order.status);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Update order status
    sqlx::query!(
        "UPDATE scroll_orderbook SET status = 'cancelled' WHERE id = $1",
        order_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("🚫 Failed to update order status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Return resources based on order type
    if order.side == "buy" {
        // Return currency for buy orders
        let order_price = sqlx::query!(
            "SELECT price FROM scroll_orderbook WHERE id = $1",
            order_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to get order price: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?.price;

        sqlx::query!(
            "UPDATE users SET currency_balance = currency_balance + $1 WHERE id = $2",
            order_price,
            user_id.0
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to return currency: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    } else {
        // Return the scroll to the user
        let scroll_exists = sqlx::query!(
            "SELECT id FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll'",
            user_id.0
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to check user's scrolls: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        match scroll_exists {
            Some(scroll) => {
                sqlx::query!(
                    "UPDATE scrolls SET quantity = quantity + 1 WHERE id = $1",
                    scroll.id
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("🚫 Failed to return scroll: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            }
            None => {
                sqlx::query!(
                    "INSERT INTO scrolls (owner_id, display_name, quantity) VALUES ($1, $2, $3)",
                    user_id.0,
                    "Summoning Scroll",
                    1
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("🚫 Failed to create scroll: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            }
        }
    }

    // Get updated balances
    let record = sqlx::query!(
        "SELECT currency_balance, (SELECT COALESCE(SUM(quantity), 0) FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll') as scroll_count FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        error!("🚫 Failed to get user balance: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let currency_balance = record.currency_balance;
    let scroll_count = record.scroll_count.unwrap_or(0) as i32;

    tx.commit().await.map_err(|e| {
        error!("🚫 Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("📜 Order {} cancelled successfully by {} (order creation fee not refunded)", order_id, username);
    Ok(Json(CancelOrderResponse {
        currency_balance,
        scroll_count,
    }))
}

pub async fn fulfill_order(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Path(order_id): Path<Uuid>,
) -> Result<Json<FulfillOrderResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Get the current user's username for logging
    let current_user = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("🚫 Failed to get current user: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to get current user".to_string() }))
    })?;

    info!("👛 User {} attempting to fulfill order {}", current_user.username, order_id);
    
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("🚫 Failed to begin transaction: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Database transaction error".to_string() }))
    })?;

    // Get the order to fulfill
    let order = sqlx::query!(
        r#"
        SELECT o.id, o.user_id, o.side::text as "side!", o.price, o.status::text as "status!", u.username as seller_username
        FROM scroll_orderbook o
        JOIN users u ON o.user_id = u.id
        WHERE o.id = $1 AND o.status = 'active'
        FOR UPDATE
        "#,
        order_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        error!("🚫 Failed to fetch order: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to fetch order".to_string() }))
    })?;

    let order = match order {
        Some(o) => {
            o
        },
        None => {
            error!("🚫 Order not found or not active: {}", order_id);
            return Err((StatusCode::NOT_FOUND, Json(ErrorResponse { error: "Order not found or already fulfilled".to_string() })));
        }
    };

    // Prevent self-trading
    if order.user_id == user_id.0 {
        error!("🚫 User {} attempted to fulfill their own order {}", current_user.username, order_id);
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Cannot fulfill your own order".to_string() })));
    }

    // Handle based on order side
    if order.side == "sell" {
        // Check if user has enough currency
        let buyer_balance = sqlx::query!(
            "SELECT currency_balance FROM users WHERE id = $1 FOR UPDATE",
            user_id.0
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to fetch buyer balance: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to fetch buyer balance".to_string() }))
        })?.currency_balance;

        if buyer_balance < order.price {
            error!("🚫 Insufficient funds: {} has {} pax, but order costs {} pax", 
                current_user.username, buyer_balance, order.price);
            return Err((StatusCode::PAYMENT_REQUIRED, Json(ErrorResponse { error: "Insufficient funds".to_string() })));
        }

        // Deduct currency from buyer
        sqlx::query!(
            "UPDATE users SET currency_balance = currency_balance - $1 WHERE id = $2",
            order.price,
            user_id.0
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to deduct currency from buyer: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to deduct currency from buyer".to_string() }))
        })?;

        // Add currency to seller
        sqlx::query!(
            "UPDATE users SET currency_balance = currency_balance + $1 WHERE id = $2",
            order.price,
            order.user_id
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to add currency to seller: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to add currency to seller".to_string() }))
        })?;

        // Add scroll to buyer
        let buyer_has_scrolls = sqlx::query!(
            "SELECT id FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll'",
            user_id.0
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to check buyer's scrolls: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to check buyer's scrolls".to_string() }))
        })?;

        match buyer_has_scrolls {
            Some(scroll) => {
                sqlx::query!(
                    "UPDATE scrolls SET quantity = quantity + 1 WHERE id = $1",
                    scroll.id
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("🚫 Failed to update buyer's scrolls: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to update buyer's scrolls".to_string() }))
                })?;
            }
            None => {
                sqlx::query!(
                    "INSERT INTO scrolls (owner_id, display_name, quantity) VALUES ($1, $2, $3)",
                    user_id.0,
                    "Summoning Scroll",
                    1
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("🚫 Failed to create buyer's scrolls: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to create buyer's scrolls".to_string() }))
                })?;
            }
        }

        // Mark order as completed
        sqlx::query!(
            "UPDATE scroll_orderbook SET status = 'completed' WHERE id = $1",
            order_id
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to update order status: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to update order status".to_string() }))
        })?;

        // Get updated balances for response
        let record = sqlx::query!(
            "SELECT currency_balance, (SELECT COALESCE(SUM(quantity), 0) FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll') as scroll_count FROM users WHERE id = $1",
            user_id.0
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to get updated balances: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to get updated balances".to_string() }))
        })?;

        let currency_balance = record.currency_balance;
        let scroll_count = record.scroll_count.unwrap_or(0) as i32;

        // Transaction commit
        tx.commit().await.map_err(|e| {
            error!("🚫 Failed to commit transaction: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to commit transaction".to_string() }))
        })?;

        info!("📜 Order {} fulfilled successfully - {} purchased scroll from {} for {} pax", 
            order_id, current_user.username, order.seller_username, order.price);
            
        Ok(Json(FulfillOrderResponse {
            id: order_id,
            currency_balance,
            scroll_count,
            message: format!("Successfully purchased 1 scroll from {} for {} pax", order.seller_username, order.price),
        }))
    } else {
        // User wants to sell to a buy order
        // Check if user has a scroll to sell
        let seller_scroll = sqlx::query!(
            "SELECT id, quantity FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll'",
            user_id.0
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to check seller's scrolls: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to check seller's scrolls".to_string() }))
        })?;

        let seller_scroll = match seller_scroll {
            Some(s) if s.quantity > 0 => {
                s
            },
            _ => {
                error!("🚫 Seller {} has no scrolls to sell", current_user.username);
                return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "You don't have any scrolls to sell".to_string() })));
            }
        };

        // Deduct scroll from seller
        if seller_scroll.quantity == 1 {
            sqlx::query!(
                "DELETE FROM scrolls WHERE id = $1",
                seller_scroll.id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("🚫 Failed to delete seller's scroll: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to delete seller's scroll".to_string() }))
            })?;
        } else {
            sqlx::query!(
                "UPDATE scrolls SET quantity = quantity - 1 WHERE id = $1",
                seller_scroll.id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("🚫 Failed to update seller's scrolls: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to update seller's scrolls".to_string() }))
            })?;
        }

        // Add currency to seller
        sqlx::query!(
            "UPDATE users SET currency_balance = currency_balance + $1 WHERE id = $2",
            order.price,
            user_id.0
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to add currency to seller: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to add currency to seller".to_string() }))
        })?;

        // Add scroll to buyer
        let buyer_has_scrolls = sqlx::query!(
            "SELECT id FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll'",
            order.user_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to check buyer's scrolls: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to check buyer's scrolls".to_string() }))
        })?;

        match buyer_has_scrolls {
            Some(scroll) => {
                sqlx::query!(
                    "UPDATE scrolls SET quantity = quantity + 1 WHERE id = $1",
                    scroll.id
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("🚫 Failed to update buyer's scrolls: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to update buyer's scrolls".to_string() }))
                })?;
            }
            None => {
                sqlx::query!(
                    "INSERT INTO scrolls (owner_id, display_name, quantity) VALUES ($1, $2, $3)",
                    order.user_id,
                    "Summoning Scroll",
                    1
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("🚫 Failed to create buyer's scrolls: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to create buyer's scrolls".to_string() }))
                })?;
            }
        }

        // Mark order as completed
        sqlx::query!(
            "UPDATE scroll_orderbook SET status = 'completed' WHERE id = $1",
            order_id
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to update order status: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to update order status".to_string() }))
        })?;

        // Get updated balances for response
        let record = sqlx::query!(
            "SELECT currency_balance, (SELECT COALESCE(SUM(quantity), 0) FROM scrolls WHERE owner_id = $1 AND display_name = 'Summoning Scroll') as scroll_count FROM users WHERE id = $1",
            user_id.0
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            error!("🚫 Failed to get updated balances: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to get updated balances".to_string() }))
        })?;

        let currency_balance = record.currency_balance;
        let scroll_count = record.scroll_count.unwrap_or(0) as i32;

        // Transaction commit
        tx.commit().await.map_err(|e| {
            error!("🚫 Failed to commit transaction: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Failed to commit transaction".to_string() }))
        })?;

        info!("📜 Order {} fulfilled successfully - {} sold scroll to {} for {} pax", 
            order_id, current_user.username, order.seller_username, order.price);
            
        Ok(Json(FulfillOrderResponse {
            id: order_id,
            currency_balance,
            scroll_count,
            message: format!("Successfully sold 1 scroll to {} for {} pax", order.seller_username, order.price),
        }))
    }
} 