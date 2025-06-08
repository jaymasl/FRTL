use axum::{
    extract::State,
    Json,
    http::{HeaderMap, StatusCode, Response},
    body::Body,
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx;
use shared::profanity::ProfanityFilter;
use crate::{AppState, auth::{self, AuthError}, services::user_service};
use lettre::{
    message::header::ContentType,
    transport::smtp::{
        authentication::Credentials,
    },
    Message,
    SmtpTransport,
    Transport,
};
use rand::distributions::{Alphanumeric, DistString};
use time::OffsetDateTime;
use uuid;
use crate::auth::utils::get_frontend_url;

#[derive(Deserialize)]
pub struct ChangeEmailRequest {
    email: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    // No fields needed in a passwordless system
}

#[derive(Deserialize)]
pub struct AccountDeleteRequest {
    // No password needed in passwordless system
}

#[derive(Deserialize)]
pub struct DeleteAccountRequest {
    email: String,
}

#[derive(Deserialize)]
pub struct VerifyDeleteAccountRequest {
    token: String,
}

#[derive(sqlx::FromRow)]
struct User {
    // No password_hash field in passwordless system
}

fn validate_auth_header(headers: &HeaderMap) -> Result<uuid::Uuid, AuthError> {
    headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(AuthError::InvalidToken)
        .and_then(auth::validate_jwt)
}

pub async fn get_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response<Body>, Response<Body>> {
    let user_id = validate_auth_header(&headers).map_err(|e| e.into_response())?;

    tracing::trace!("Fetching profile for user_id: {}", user_id);

    let profile = user_service::get_user_profile(&state.pool, user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch profile: {:?}", e);
            e.into_response()
        })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&profile).unwrap()))
        .unwrap())
}

pub async fn change_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangeEmailRequest>,
) -> Result<Response<Body>, Response<Body>> {
    let user_id = validate_auth_header(&headers).map_err(|e| e.into_response())?;

    if let Err(msg) = ProfanityFilter::validate_email_local_part(&request.email) {
        return Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/json")
            .body(Body::from(format!("{{\"error\":\"{}\"}}", msg)))
            .unwrap());
    }

    if !request.email.contains('@') {
        return Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/json")
            .body(Body::from("{\"error\":\"Invalid email format\"}"))
            .unwrap());
    }

    // Update email directly without password verification
    let result = sqlx::query!(
        "UPDATE users SET email = $1 WHERE id = $2 AND deleted_at IS NULL",
        request.email,
        user_id
    )
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from("{\"message\":\"Email updated successfully\"}"))
                .unwrap())
        },
        Err(e) => {
            if let Some(db_err) = e.as_database_error() {
                if db_err.is_unique_violation() {
                    return Err(Response::builder()
                        .status(StatusCode::CONFLICT)
                        .header("Content-Type", "application/json")
                        .body(Body::from("{\"error\":\"Email already in use\"}"))
                        .unwrap());
                }
            }
            
            Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
                .body(Body::from("{\"error\":\"Failed to update email\"}"))
                .unwrap())
        }
    }
}

pub async fn change_password(
    _state: State<AppState>,
    _headers: HeaderMap,
    _request: Json<ChangePasswordRequest>,
) -> Result<Response<Body>, Response<Body>> {
    // System is now passwordless, so return a message
    Err(Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header("Content-Type", "application/json")
        .body(Body::from("{\"error\":\"This system uses passwordless authentication. Password changes are not supported.\"}"))
        .unwrap())
}

pub async fn delete_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    _request: Json<AccountDeleteRequest>,
) -> Result<Response<Body>, Response<Body>> {
    let user_id = validate_auth_header(&headers).map_err(|e| e.into_response())?;
    
    // No password verification needed in passwordless system
    
    let mut tx = state.pool.begin().await.map_err(|e| AuthError::Database(e).into_response())?;

    // Delete all associated data in the correct order to handle foreign key constraints
    
    // 1. First nullify user references in historical events for transferred items
    sqlx::query!(
        r#"
        UPDATE item_events 
        SET from_user_id = NULL,
            performed_by_user_id = NULL
        WHERE (from_user_id = $1 OR performed_by_user_id = $1)
        AND item_id NOT IN (
            SELECT id FROM scrolls WHERE owner_id = $1
            UNION ALL
            SELECT id FROM eggs WHERE owner_id = $1
            UNION ALL
            SELECT id FROM creatures WHERE owner_id = $1
        )"#,
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 2. Then delete events only for items the user currently owns
    sqlx::query!(
        r#"
        DELETE FROM item_events 
        WHERE item_id IN (
            SELECT id FROM scrolls WHERE owner_id = $1
            UNION ALL
            SELECT id FROM eggs WHERE owner_id = $1
            UNION ALL
            SELECT id FROM creatures WHERE owner_id = $1
        )"#,
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 2. Delete market listings (both as seller and buyer)
    sqlx::query!(
        "DELETE FROM market_listings WHERE seller_id = $1 OR buyer_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 3. Delete scroll orderbook entries
    sqlx::query!(
        "DELETE FROM scroll_orderbook WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 4. Delete scrolls
    sqlx::query!(
        "DELETE FROM scrolls WHERE owner_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 5. Delete creatures
    sqlx::query!(
        "DELETE FROM creatures WHERE owner_id = $1 OR original_egg_summoned_by = $1 OR hatched_by = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 6. Delete eggs
    sqlx::query!(
        "DELETE FROM eggs WHERE owner_id = $1 OR summoned_by = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 7. Delete user preferences
    sqlx::query!(
        "DELETE FROM user_preferences WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 8. Delete user achievements
    sqlx::query!(
        "DELETE FROM user_achievements WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 9. Delete game leaderboard entries
    sqlx::query!(
        "DELETE FROM game_leaderboard WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 10. Delete word game stats
    sqlx::query!(
        "DELETE FROM word_game_stats WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 11. Delete refresh tokens
    sqlx::query!(
        "DELETE FROM refresh_tokens WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 12. Delete magic link tokens
    sqlx::query!(
        "DELETE FROM magic_link_tokens WHERE email = (SELECT email FROM users WHERE id = $1)",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // Finally, delete the user
    sqlx::query!(
        "DELETE FROM users WHERE id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    tx.commit()
        .await
        .map_err(|e| AuthError::Database(e).into_response())?;

    Ok(Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap())
}

pub async fn request_delete_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeleteAccountRequest>,
) -> Result<Response<Body>, Response<Body>> {
    let user_id = match validate_auth_header(&headers) {
        Ok(id) => id,
        Err(e) => return Err(e.into_response()),
    };

    // Verify the email matches the user's email
    let user_email = match sqlx::query_scalar!("SELECT email FROM users WHERE id = $1", user_id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(email)) => email,
        Ok(None) => {
            return Err(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("User not found"))
                .unwrap());
        }
        Err(e) => {
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Database error: {}", e)))
                .unwrap());
        }
    };

    if user_email.to_lowercase() != request.email.to_lowercase() {
        return Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Email does not match account email"))
            .unwrap());
    }

    // Generate a token
    let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let expires_at = OffsetDateTime::now_utc() + time::Duration::hours(24);

    // Store the token in magic_link_tokens table (reusing it for account deletion)
    match sqlx::query!(
        "INSERT INTO magic_link_tokens (email, token, expires_at) VALUES ($1, $2, $3)",
        request.email,
        token,
        expires_at
    )
    .execute(&state.pool)
    .await
    {
        Ok(_) => (),
        Err(e) => {
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Database error: {}", e)))
                .unwrap());
        }
    };

    // Send email with deletion link
    let deletion_link = format!(
        "{}/verify-delete-account?token={}",
        get_frontend_url(),
        token
    );

    match send_deletion_email(&request.email, &deletion_link).await {
        Ok(_) => {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(Body::from("Deletion email sent"))
                .unwrap())
        }
        Err(e) => {
            Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Failed to send email: {}", e)))
                .unwrap())
        }
    }
}

pub async fn verify_delete_account(
    State(state): State<AppState>,
    Json(request): Json<VerifyDeleteAccountRequest>,
) -> Result<Response<Body>, Response<Body>> {
    // Verify the token from the database
    let result = sqlx::query!(
        "SELECT email, expires_at FROM magic_link_tokens WHERE token = $1",
        request.token
    )
    .fetch_optional(&state.pool)
    .await;

    let (email, expires_at) = match result {
        Ok(Some(record)) => {
            (record.email, record.expires_at)
        },
        Ok(None) => {
            tracing::error!("Token not found in database: {}", request.token);
            return Err(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Invalid or expired token"))
                .unwrap());
        }
        Err(e) => {
            tracing::error!("Database error when looking up token: {}", e);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Database error: {}", e)))
                .unwrap());
        }
    };

    // Check if token is expired
    let now = OffsetDateTime::now_utc();
    
    if expires_at < now {
        tracing::error!("Token has expired");
        return Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Token expired"))
            .unwrap());
    }

    // Delete the token
    match sqlx::query!("DELETE FROM magic_link_tokens WHERE token = $1", request.token)
        .execute(&state.pool)
        .await
    {
        Ok(_) => {},
        Err(e) => {
            tracing::error!("Failed to delete token: {}", e);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Database error: {}", e)))
                .unwrap());
        }
    };

    // Get the user ID from the email
    let user_result = match sqlx::query!("SELECT id, username FROM users WHERE email = $1", email)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(user)) => {
            user
        },
        Ok(None) => {
            tracing::error!("User not found for email: {}", email);
            return Err(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("User not found"))
                .unwrap());
        }
        Err(e) => {
            tracing::error!("Database error when looking up user: {}", e);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Database error: {}", e)))
                .unwrap());
        }
    };

    let user_id = user_result.id;
    let username = user_result.username;

    // Start a transaction
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("Failed to start transaction: {}", e);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Database error: {}", e)))
                .unwrap());
        }
    };

    // Delete all associated data in the correct order to handle foreign key constraints
    
    // 1. First nullify user references in historical events for transferred items
    sqlx::query!(
        r#"
        UPDATE item_events 
        SET from_user_id = NULL,
            performed_by_user_id = NULL
        WHERE (from_user_id = $1 OR performed_by_user_id = $1)
        AND item_id NOT IN (
            SELECT id FROM scrolls WHERE owner_id = $1
            UNION ALL
            SELECT id FROM eggs WHERE owner_id = $1
            UNION ALL
            SELECT id FROM creatures WHERE owner_id = $1
        )"#,
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 2. Then delete events only for items the user currently owns
    sqlx::query!(
        r#"
        DELETE FROM item_events 
        WHERE item_id IN (
            SELECT id FROM scrolls WHERE owner_id = $1
            UNION ALL
            SELECT id FROM eggs WHERE owner_id = $1
            UNION ALL
            SELECT id FROM creatures WHERE owner_id = $1
        )"#,
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 2. Delete market listings (both as seller and buyer)
    sqlx::query!(
        "DELETE FROM market_listings WHERE seller_id = $1 OR buyer_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 3. Delete scroll orderbook entries
    sqlx::query!(
        "DELETE FROM scroll_orderbook WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 4. Delete scrolls
    sqlx::query!(
        "DELETE FROM scrolls WHERE owner_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 5. Delete creatures
    sqlx::query!(
        "DELETE FROM creatures WHERE owner_id = $1 OR original_egg_summoned_by = $1 OR hatched_by = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 6. Delete eggs
    sqlx::query!(
        "DELETE FROM eggs WHERE owner_id = $1 OR summoned_by = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 7. Delete user preferences
    sqlx::query!(
        "DELETE FROM user_preferences WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 8. Delete user achievements
    sqlx::query!(
        "DELETE FROM user_achievements WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 9. Delete game leaderboard entries
    sqlx::query!(
        "DELETE FROM game_leaderboard WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 10. Delete word game stats
    sqlx::query!(
        "DELETE FROM word_game_stats WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 11. Delete refresh tokens
    sqlx::query!(
        "DELETE FROM refresh_tokens WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AuthError::Database(e).into_response())?;

    // 12. Delete magic link tokens
    match sqlx::query!(
        "DELETE FROM magic_link_tokens WHERE email = (SELECT email FROM users WHERE id = $1)",
        user_id
    )
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {},
        Err(e) => {
            tracing::error!("Failed to delete magic link tokens: {}", e);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Database error: {}", e)))
                .unwrap());
        }
    };

    // Finally, delete the user
    match sqlx::query!("DELETE FROM users WHERE id = $1", user_id)
        .execute(&mut *tx)
        .await
    {
        Ok(_) => {
            // Commit the transaction
            match tx.commit().await {
                Ok(_) => {
                    tracing::info!("👤 User '{}' account deleted successfully", username);
                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/json")
                        .body(Body::from(format!("{{\"username\":\"{}\"}}", username)))
                        .unwrap())
                },
                Err(e) => {
                    tracing::error!("Failed to commit transaction: {}", e);
                    Err(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from(format!("Database error: {}", e)))
                        .unwrap())
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to delete user with ID: {}, error: {}", user_id, e);
            Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Database error: {}", e)))
                .unwrap())
        }
    }
}

async fn send_deletion_email(email: &str, deletion_link: &str) -> Result<(), AuthError> {
    tracing::info!("Attempting to send account deletion email to: {}", email);
    
    let smtp_username = std::env::var("SMTP_USERNAME").map_err(|e| {
        tracing::error!("Failed to get SMTP_USERNAME: {:?}", e);
        AuthError::Database(sqlx::Error::Configuration(
            "SMTP_USERNAME not set".into(),
        ))
    })?;
    
    let smtp_password = std::env::var("SMTP_PASSWORD").map_err(|e| {
        tracing::error!("Failed to get SMTP_PASSWORD: {:?}", e);
        AuthError::Database(sqlx::Error::Configuration(
            "SMTP_PASSWORD not set".into(),
        ))
    })?;
    
    let smtp_host = std::env::var("SMTP_HOST").map_err(|e| {
        tracing::error!("Failed to get SMTP_HOST: {:?}", e);
        AuthError::Database(sqlx::Error::Configuration(
            "SMTP_HOST not set".into(),
        ))
    })?;

    let email_body = format!(
        r#"
        <html>
        <body style="font-family: Arial, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
            <div style="text-align: center; margin-bottom: 20px;">
                <h1 style="color: #4a5568; margin-bottom: 10px;">Account Deletion Request</h1>
            </div>
            <p>We received a request to delete your FRTL account. If you did not make this request, please ignore this email.</p>
            <p>To confirm account deletion, please click the button below:</p>
            <div style="text-align: center; margin: 30px 0;">
                <a href="{}" style="background-color: #e53e3e; color: white; padding: 16px 32px; text-decoration: none; border-radius: 6px; font-weight: bold; display: inline-block; font-size: 18px; box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1); transition: all 0.3s ease;">Delete My Account</a>
            </div>
            <p style="margin-bottom: 5px;">If the button doesn't work, you can copy and paste this link into your browser:</p>
            <p style="word-break: break-all; background-color: #f3f4f6; padding: 10px; border-radius: 4px; font-size: 14px;">{}</p>
            <p style="color: #718096; font-size: 14px; margin-top: 30px;">This link will expire in 24 hours.</p>
            <hr style="border: none; border-top: 1px solid #e2e8f0; margin: 20px 0;">
            <p style="color: #718096; font-size: 12px; text-align: center;">If you didn't request this account deletion, you can safely ignore this email. FRTL will never request personal information via email.</p>
        </body>
        </html>
        "#,
        deletion_link,
        deletion_link
    );

    let email_message = Message::builder()
        .from("frtl@jaykrown.com".parse().map_err(|e| {
            tracing::error!("Failed to parse from address: {:?}", e);
            AuthError::Database(sqlx::Error::Configuration(
                format!("Failed to parse from address: {}", e).into(),
            ))
        })?)
        .to(format!("<{}>", email).parse().map_err(|e| {
            tracing::error!("Failed to parse to address: {:?}", e);
            AuthError::Database(sqlx::Error::Configuration(
                format!("Failed to parse to address: {}", e).into(),
            ))
        })?)
        .subject("FRTL Account Deletion Request")
        .header(ContentType::TEXT_HTML)
        .body(email_body)
        .map_err(|e| {
            tracing::error!("Failed to build email: {:?}", e);
            AuthError::Database(sqlx::Error::Configuration(
                format!("Failed to build email: {}", e).into(),
            ))
        })?;

    let creds = Credentials::new(smtp_username, smtp_password);

    let mailer = SmtpTransport::relay(&smtp_host)
        .map_err(|e| {
            tracing::error!("Failed to create SMTP transport: {:?}", e);
            AuthError::Database(sqlx::Error::Configuration(
                format!("Failed to create SMTP transport: {}", e).into(),
            ))
        })?
        .credentials(creds)
        .port(465)
        .build();

    match mailer.send(&email_message) {
        Ok(_) => {
            tracing::info!("Account deletion email sent successfully");
            Ok(())
        },
        Err(e) => {
            tracing::error!("Failed to send account deletion email: {:?}", e);
            Err(AuthError::Database(sqlx::Error::Configuration(
                format!("Failed to send email: {}", e).into(),
            )))
        }
    }
}