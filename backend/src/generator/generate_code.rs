use axum::{
    extract::{Extension, Json},
    response::IntoResponse,
    routing::{post, get},
    Router,
    http::HeaderMap,
    middleware,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use sqlx::PgPool;
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE};
use rand::Rng;
use sqlx::Transaction;
use crate::auth::middleware::UserId;

const INTERNAL_SECRET_HEADER: &str = "x-internal-secret";
const CODE_LENGTH: usize = 24;
const CODE_PREFIX: &str = "FRTL-";

#[derive(Debug, Serialize)]
pub struct MembershipCode {
    pub id: Uuid,
    pub code_hash: String,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub used_at: Option<OffsetDateTime>,
    pub used_by: Option<Uuid>,
    pub is_valid: bool,
    pub duration_minutes: i32,
}

#[derive(Deserialize)]
pub struct GenerateMembershipRequest {
    pub expiration_minutes: Option<i64>,
    pub duration_minutes: Option<i32>,
}

#[derive(Serialize)]
pub struct GenerateMembershipResponse {
    pub membership_code: String,
}

#[derive(Deserialize)]
pub struct RedeemMembershipRequest {
    pub code: String,
}

#[derive(Deserialize)]
pub struct PurchaseTemporaryMembershipRequest {
    pub duration_minutes: Option<i32>,
}

/// Generate a secure membership code with prefix, mixed case, numbers, and safe symbols
fn generate_secure_code() -> String {
    let mut rng = rand::thread_rng();
    let code_bytes: Vec<u8> = (0..CODE_LENGTH)
        .map(|_| rng.gen::<u8>())
        .collect();
    
    let encoded = URL_SAFE.encode(code_bytes);
    format!("{}{}", CODE_PREFIX, &encoded[..CODE_LENGTH])
}

/// Hash a membership code using Argon2
fn hash_membership_code(code: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(code.as_bytes(), &salt)?;
    Ok(password_hash.to_string())
}

/// Verify a membership code against its hash
fn verify_membership_code(code: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    
    Argon2::default()
        .verify_password(code.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Validate a membership code using an existing transaction
pub async fn validate_membership_code_tx<'a>(tx: &mut Transaction<'a, sqlx::Postgres>, code: &str) -> Result<MembershipCode, sqlx::Error> {
    // Get all valid codes and verify each one
    let valid_codes = sqlx::query_as!(MembershipCode,
        "SELECT id, code_hash, created_by, created_at, expires_at, used_at, used_by, is_valid, duration_minutes \
         FROM membership_codes \
         WHERE is_valid = true \
           AND used_at IS NULL \
           AND (expires_at IS NULL OR expires_at > NOW()) \
         FOR UPDATE"
    )
    .fetch_all(&mut **tx)
    .await?;

    // Find the matching code using constant-time comparison
    for valid_code in valid_codes {
        if verify_membership_code(code, &valid_code.code_hash) {
            return Ok(valid_code);
        }
    }
    Err(sqlx::Error::RowNotFound)
}

/// Mark a membership code as used and update user's membership status
pub async fn redeem_membership_code<'a>(tx: &mut Transaction<'a, sqlx::Postgres>, code: &str, user_id: Uuid) -> Result<(), sqlx::Error> {
    // Find and verify the code within the ongoing transaction
    let membership = validate_membership_code_tx(tx, code).await?;

    // Delete the code instead of marking it as used
    let result = sqlx::query!(
        "DELETE FROM membership_codes \
         WHERE id = $1 \
           AND is_valid = true \
           AND used_at IS NULL \
           AND (expires_at IS NULL OR expires_at > NOW()) \
         RETURNING id",
        membership.id
    )
    .fetch_optional(&mut **tx)
    .await?;

    match result {
        Some(_) => {
            // Update user's membership status
            let user = sqlx::query!(
                "SELECT username, is_member, member_until FROM users WHERE id = $1",
                user_id
            )
            .fetch_one(&mut **tx)
            .await?;

            let new_member_until = if user.is_member && user.member_until.is_some() {
                // If already a member, add duration to existing expiration
                user.member_until.unwrap() + time::Duration::minutes(membership.duration_minutes as i64)
            } else {
                // If not a member, set expiration from now
                OffsetDateTime::now_utc() + time::Duration::minutes(membership.duration_minutes as i64)
            };

            // Format the expiration time for logging
            let formatted_time = new_member_until.format(&time::format_description::well_known::Rfc3339).unwrap_or_default();

            // Log the membership activation with emoji and username
            tracing::info!("🎁 User {} became a member until {}", user.username, formatted_time);

            sqlx::query!(
                "UPDATE users SET is_member = true, member_until = $1, membership_source = 'code' WHERE id = $2",
                new_member_until,
                user_id
            )
            .execute(&mut **tx)
            .await?;

            Ok(())
        },
        None => Err(sqlx::Error::RowNotFound),
    }
}

/// Helper function to check if a user is a member
pub async fn is_member(pool: &PgPool, user_id: Uuid) -> Result<bool, sqlx::Error> {
    let user = sqlx::query!(
        "SELECT is_member, member_until FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(pool)
    .await?;

    // Check if membership has expired
    if user.is_member && user.member_until.is_some() {
        let now = OffsetDateTime::now_utc();
        if user.member_until.unwrap() <= now {
            return Ok(false);
        }
    }

    Ok(user.is_member)
}

/// Generate a membership code. This endpoint:
/// 1. Requires the internal secret key in the x-internal-secret header
/// 2. Generates cryptographically secure codes
pub async fn generate_membership_code_handler(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(payload): Json<GenerateMembershipRequest>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    // Since the server is only bound to 127.0.0.1, all direct connections are from localhost
    // We'll still check headers for cases where the server might be behind a proxy
    
    // First check X-Forwarded-For header
    let forwarded_ip = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown");
    
    // Also check X-Real-IP header which might be set by some proxies
    let real_ip = headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown");
    
    // If we have forwarded headers and they're not localhost, reject the request
    // This only applies if we're behind a proxy
    if (forwarded_ip != "unknown" && forwarded_ip != "127.0.0.1" && forwarded_ip != "::1" && forwarded_ip != "localhost") ||
       (real_ip != "unknown" && real_ip != "127.0.0.1" && real_ip != "::1" && real_ip != "localhost") {
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            format!("Membership code generation is restricted to localhost only. Detected IP: {}, Real IP: {}", 
                   forwarded_ip, real_ip),
        ));
    }

    // Verify the internal secret
    let provided_secret = headers
        .get(INTERNAL_SECRET_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| (axum::http::StatusCode::UNAUTHORIZED, "Missing internal secret".to_string()))?;

    let internal_secret = std::env::var("INTERNAL_CODE_GENERATION_SECRET")
        .map_err(|_| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_CODE_GENERATION_SECRET not configured".to_string()))?;

    if provided_secret != internal_secret {
        return Err((axum::http::StatusCode::UNAUTHORIZED, "Invalid internal secret".to_string()));
    }

    // Generate a secure code
    let membership_code = generate_secure_code();
    
    // Hash the code for storage
    let code_hash = hash_membership_code(&membership_code)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Use a system UUID as creator
    let system_uuid = Uuid::new_v4();

    // Calculate expiration time if provided
    let expires_at = payload.expiration_minutes.map(|mins| {
        OffsetDateTime::now_utc() + time::Duration::minutes(mins)
    });

    // Set duration (default to 1 minute if not specified)
    let duration_minutes = payload.duration_minutes.unwrap_or(1);

    // Store the hashed code
    sqlx::query!(
        "INSERT INTO membership_codes (code_hash, created_by, expires_at, duration_minutes) \
         VALUES ($1, $2, $3, $4)",
        code_hash,
        system_uuid,
        expires_at,
        duration_minutes,
    )
    .execute(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(GenerateMembershipResponse {
        membership_code
    }))
}

/// Redeem a membership code to gain member status
pub async fn redeem_membership_code_handler(
    Extension(pool): Extension<PgPool>,
    Extension(user_id): Extension<UserId>,
    Json(payload): Json<RedeemMembershipRequest>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    // Start a transaction
    let mut tx = pool.begin().await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Attempt to redeem the code
    match redeem_membership_code(&mut tx, &payload.code, user_id.0).await {
        Ok(_) => {
            // Commit the transaction
            tx.commit().await
                .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            
            Ok(axum::Json(()))
        },
        Err(_) => Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid or expired membership code".to_string(),
        )),
    }
}

/// Check a user's membership status
pub async fn check_membership_status_handler(
    Extension(pool): Extension<PgPool>,
    Extension(user_id): Extension<UserId>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    // Query the user's membership status
    let user = sqlx::query!(
        "SELECT username, is_member, member_until FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Check if membership has expired
    if user.is_member && user.member_until.is_some() {
        let now = OffsetDateTime::now_utc();
        if user.member_until.unwrap() <= now {
            // Log the membership expiration with username
            tracing::info!("User {} membership expired", user.username);
            
            // Membership has expired, update the database
            sqlx::query!(
                "UPDATE users SET is_member = false, member_until = NULL WHERE id = $1",
                user_id.0
            )
            .execute(&pool)
            .await
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            // Return expired status
            return Ok(axum::Json(serde_json::json!({
                "is_member": false,
                "member_until": null
            })));
        }
    }

    // Return current status with RFC3339 formatted time
    let member_until_str = user.member_until.map(|t| t.format(&time::format_description::well_known::Rfc3339).unwrap_or_default());
    
    Ok(axum::Json(serde_json::json!({
        "is_member": user.is_member,
        "member_until": member_until_str
    })))
}

/// Purchase a temporary membership for currency
pub async fn purchase_temporary_membership_handler(
    Extension(pool): Extension<PgPool>,
    Extension(user_id): Extension<UserId>,
    Json(payload): Json<PurchaseTemporaryMembershipRequest>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    // Default values that can be easily updated
    const DEFAULT_DURATION_MINUTES: i32 = 10080; // 7 days (was 5 minutes)
    const DEFAULT_COST: i32 = 1000; // 1,000 PAX (was 10 PAX)
    
    // Use provided duration or default
    let duration_minutes = payload.duration_minutes.unwrap_or(DEFAULT_DURATION_MINUTES);
    
    // Start a transaction
    let mut tx = pool.begin().await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Get user's current balance and membership status
    let user = sqlx::query!(
        "SELECT username, currency_balance, is_member, member_until FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Check if user has enough balance
    if user.currency_balance < DEFAULT_COST {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            format!("Insufficient balance. Required: {} pax", DEFAULT_COST),
        ));
    }
    
    // Calculate new membership expiration time
    let new_member_until = if user.is_member && user.member_until.is_some() {
        // If already a member, add duration to existing expiration
        user.member_until.unwrap() + time::Duration::minutes(duration_minutes as i64)
    } else {
        // If not a member, set expiration from now
        OffsetDateTime::now_utc() + time::Duration::minutes(duration_minutes as i64)
    };
    
    // Format the expiration time in a more readable way
    let formatted_time = {
        let dt = new_member_until.format(&time::format_description::well_known::Rfc2822).unwrap_or_else(|_| new_member_until.to_string());
        dt
    };
    
    // Deduct currency and update membership status
    sqlx::query!(
        "UPDATE users SET currency_balance = currency_balance - $1, is_member = true, member_until = $2, membership_source = 'purchase' WHERE id = $3",
        DEFAULT_COST,
        new_member_until,
        user_id.0
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Commit the transaction
    tx.commit().await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Log the temporary membership purchase
    tracing::info!(
        "💰 User {} purchased a temporary membership ({} minutes) for {} pax. Active until {}",
        user.username,
        duration_minutes,
        DEFAULT_COST,
        formatted_time
    );
    
    // Return success with new balance
    Ok(axum::Json(serde_json::json!({
        "success": true,
        "new_balance": user.currency_balance - DEFAULT_COST,
        "duration_minutes": duration_minutes,
        "expires_at": formatted_time
    })))
}

/// Returns a router configured with the membership code routes
pub fn membership_code_routes<S>(pool: PgPool) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    // Admin route doesn't need auth middleware
    let admin_routes = Router::new()
        .route("/admin/membership-code", post(generate_membership_code_handler));
    
    // User routes need auth middleware
    let user_routes = Router::new()
        .route("/api/membership/redeem", post(redeem_membership_code_handler))
        .route("/api/membership/status", get(check_membership_status_handler))
        .route("/api/membership/purchase-temporary", post(purchase_temporary_membership_handler))
        .layer(middleware::from_fn(crate::auth::middleware::require_auth));
    
    // Combine the routes
    admin_routes
        .merge(user_routes)
        .layer(Extension(pool))
} 