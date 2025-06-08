mod models;
mod create;
mod query;

// Re-export only what's needed by external modules
pub use create::create_order;
pub use query::{get_orders, cancel_order, fulfill_order};

// Note: The routes function is not currently used in main.rs, but is kept for future use
// or for documentation purposes. To use it, replace the individual route definitions in main.rs with:
// .merge(orderbook_service::routes())
#[allow(dead_code)]
pub fn routes() -> axum::Router<crate::AppState> {
use axum::{
        routing::{get, post},
        Router,
    };

    Router::new()
        .route("/api/orderbook/orders", get(get_orders))
        .route("/api/orderbook/orders", post(create_order))
        .route("/api/orderbook/orders/:order_id/cancel", post(cancel_order))
        .route("/api/orderbook/orders/:order_id/fulfill", post(fulfill_order))
} 