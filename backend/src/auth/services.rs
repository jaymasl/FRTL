use sqlx::{PgPool, Postgres, Transaction};
use tracing::warn;
use uuid::Uuid;
use axum::http::HeaderMap;
use ipnetwork::IpNetwork;
use std::str::FromStr;
use rand::distributions::{Alphanumeric, DistString};
use time::OffsetDateTime;

use super::AuthError;

pub async fn handle_authentication(
    executor: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    headers: &mut HeaderMap,
) -> Result<(String, String), AuthError> {
    sqlx::query!(
        "UPDATE users SET last_login = NOW() WHERE id = $1",
        user_id
    )
    .execute(&mut **executor)
    .await
    .map_err(AuthError::Database)?;

    // Always store refresh token
    store_refresh_token(executor, user_id).await?;

    let (token, csrf_token) = crate::auth::set_auth_cookies(user_id, headers)?;

    Ok((csrf_token, token))
}

async fn store_refresh_token(
    executor: &mut Transaction<'_, Postgres>,
    user_id: Uuid
) -> Result<(), AuthError> {
    let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 64);
    let expires_at = OffsetDateTime::now_utc() + time::Duration::days(7);

    sqlx::query!(
        "INSERT INTO refresh_tokens (user_id, token, expires_at) VALUES ($1, $2, $3)",
        user_id,
        token,
        expires_at
    )
    .execute(&mut **executor)
    .await
    .map_err(AuthError::Database)?;

    Ok(())
}

pub async fn record_login_attempt(
    pool: &PgPool,
    username: &str,
    ip_address: &str,
    success: bool,
) -> Result<(), AuthError> {
    let ip = IpNetwork::from_str(ip_address).unwrap_or_else(|_| {
        warn!("Invalid IP address: {}", ip_address);
        IpNetwork::from_str("0.0.0.0/0").unwrap()
    });

    sqlx::query!(
        "INSERT INTO login_attempts (username, ip_address, successful) VALUES ($1, $2, $3)",
        username,
        ip,
        success
    )
    .execute(pool)
    .await
    .map_err(AuthError::Database)?;

    Ok(())
}
