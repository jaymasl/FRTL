use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use crate::auth::AuthError;
use axum::{
   body::Body,
};
use crate::auth::middleware::UserId;
use crate::AppState;
use serde_json;

pub mod models {
   use super::*;
   
   #[derive(Debug, Serialize)]
   pub struct UserProfile {
      pub username: String,
      pub email: String,
      pub currency_balance: i32,
      pub experience: i32,
      pub rank: Option<String>,
      pub last_login: Option<String>,
      pub created_at: String,
      pub is_member: bool,
   }
}

pub mod user_service;
pub mod creature_service;
pub mod claim_service;
pub mod creature_bind;
pub mod energy_service;
pub mod chaos_realm;
pub mod market_service;
pub mod scroll_service;
pub mod orderbook_service;
pub mod magic_button_service;
pub mod patreon_link_service;