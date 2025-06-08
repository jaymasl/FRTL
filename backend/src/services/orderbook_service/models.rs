use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "order_side_type", rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "buy"),
            OrderSide::Sell => write!(f, "sell"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub side: OrderSide,  // "buy" or "sell"
    pub price: i32,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub side: String,
    pub price: i32,
    pub status: String,
    pub created_at: String,
    pub currency_balance: i32,
    pub scroll_count: i32,
}

#[derive(Debug, Serialize)]
pub struct AggregatedOrderResponse {
    pub side: String,
    pub price: i32,
    pub id: String,
    pub user_id: String,
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct CancelOrderResponse {
    pub currency_balance: i32,
    pub scroll_count: i32,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct FulfillOrderResponse {
    pub id: Uuid,
    pub currency_balance: i32,
    pub scroll_count: i32,
    pub message: String,
} 