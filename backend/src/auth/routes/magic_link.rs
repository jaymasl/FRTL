use axum::{
    extract::State,
    http::{Response, StatusCode, HeaderMap},
    Json,
    body::Body,
    response::IntoResponse,
};
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
use crate::{AppState, auth::{self, AuthError, models::{MagicLinkRequest, MagicLinkVerification, AuthResponse}}};
use std::env;
use tracing;
use shared::validation::*;
use crate::auth::utils::get_frontend_url;

// Request a magic link to be sent to the user's email
pub async fn request_magic_link(
    State(state): State<AppState>,
    Json(request): Json<MagicLinkRequest>,
) -> Result<Response<Body>, Response<Body>> {
    // Record the start time to ensure consistent response timing
    let start_time = std::time::Instant::now();
    
    // Validate email format
    if let Err(_) = validate_email(&request.email) {
        return Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Invalid email format"))
            .unwrap());
    }

    // Verify hCaptcha token
    let hcaptcha_secret = env::var("HCAPTCHA_SECRET_KEY").map_err(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Internal server error"))
            .unwrap()
    })?;

    let client = reqwest::Client::new();
    let res = client
        .post("https://hcaptcha.com/siteverify")
        .form(&[
            ("secret", hcaptcha_secret.as_str()),
            ("response", request.captcha_token.as_str()),
        ])
        .send()
        .await
        .map_err(|_| {
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Failed to verify hCaptcha"))
                .unwrap()
        })?;

    if !res.status().is_success() {
        return Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Invalid hCaptcha token"))
            .unwrap());
    }

    let verification: serde_json::Value = res.json().await.map_err(|_| {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Failed to parse hCaptcha response"))
            .unwrap()
    })?;

    if !verification["success"].as_bool().unwrap_or(false) {
        return Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("hCaptcha verification failed"))
            .unwrap());
    }

    // Check if the email exists in the database
    let user_exists = sqlx::query!(
        "SELECT id FROM users WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL",
        request.email
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error checking user existence: {:?}", e);
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Database error"))
            .unwrap()
    })?;

    // Create a future that will be completed regardless of whether the email exists
    let email_future = async {
        if let Some(_) = user_exists {
            // Generate a random token
            let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 64);
            
            // Store the token in the database with a 15-minute expiration
            let expires_at = OffsetDateTime::now_utc() + time::Duration::minutes(15);
            
            let mut tx = state.pool.begin().await.map_err(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Database error"))
                    .unwrap()
            })?;
            
            // Store the token in the database
            let token_data = serde_json::json!({});

            match sqlx::query!(
                "INSERT INTO magic_link_tokens (email, token, expires_at, token_data) VALUES ($1, $2, $3, $4)",
                request.email,
                token,
                expires_at,
                token_data
            )
            .execute(&mut *tx)
            .await
            {
                Ok(_) => {
                    tx.commit().await.map_err(|_| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::from("Database error"))
                            .unwrap()
                    })?;
                },
                Err(e) => {
                    tracing::error!("Failed to store magic link token: {:?}", e);
                    return Err(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Failed to store magic link token"))
                        .unwrap());
                }
            };

            // Generate the magic link URL
            let magic_link = format!(
                "{}/verify-magic-link?token={}",
                get_frontend_url(),
                token
            );
            
            if let Err(e) = send_magic_link_email(&request.email, &magic_link).await {
                tracing::error!("Failed to send magic link email: {:?}", e);
                // Don't reveal that the email exists, just return a generic error
                return Err(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("An error occurred. Please try again later."))
                    .unwrap());
            }
        }
        
        Ok(())
    };
    
    // Execute the email sending future
    let result = email_future.await;
    
    // Calculate how much time has passed
    let elapsed = start_time.elapsed();
    
    // Ensure the function takes at least 1 second to complete, regardless of whether
    // the email exists or not, to prevent timing attacks
    let min_processing_time = std::time::Duration::from_secs(1);
    if elapsed < min_processing_time {
        tokio::time::sleep(min_processing_time - elapsed).await;
    }
    
    // If there was an error in the email sending process, return it
    if let Err(e) = result {
        return Err(e);
    }

    // Always return the same success message regardless of whether the email exists
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from("If an account exists with this email, a magic link has been sent. Please check your inbox and spam folder."))
        .unwrap())
}

// Verify a magic link token and authenticate the user
pub async fn verify_magic_link(
    State(state): State<AppState>,
    mut headers: HeaderMap,
    Json(request): Json<MagicLinkVerification>,
) -> Result<Response<Body>, Response<Body>> {
    let mut tx = state.pool.begin().await.map_err(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Database error"))
            .unwrap()
    })?;
    
    // Verify the token
    let token_record = sqlx::query!(
        "SELECT id, email, expires_at, used_at, token_data FROM magic_link_tokens WHERE token = $1",
        request.token
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Database error"))
            .unwrap()
    })?;
    
    let token_record = match token_record {
        Some(record) => {
            record
        },
        None => {
            tracing::error!("Token not found in database: {}", request.token);
            return Err(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Invalid or expired magic link"))
                .unwrap());
        }
    };
    
    // Check if token is expired
    let now = OffsetDateTime::now_utc();
    
    if token_record.expires_at < now {
        tracing::error!("Token has expired");
        return Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Magic link has expired"))
            .unwrap());
    }
    
    // Check if token has already been used
    if token_record.used_at.is_some() {
        tracing::debug!("Magic link token has already been used");
        return Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Magic link has already been used"))
            .unwrap());
    }
    
    // Mark the token as used
    sqlx::query!(
        "UPDATE magic_link_tokens SET used_at = $1 WHERE id = $2",
        now,
        token_record.id
    )
    .execute(&mut *tx)
    .await
    .map_err(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Database error"))
            .unwrap()
    })?;
    
    // Check if this is a registration token
    let is_registration = if let Some(token_data) = &token_record.token_data {
        token_data.get("is_registration").and_then(|v| v.as_bool()).unwrap_or(false)
    } else {
        false
    };
    
    let user_id = if is_registration {
        // This is a registration token, create the user account
        
        if let Some(token_data) = &token_record.token_data {
            if let Some(username) = token_data.get("username").and_then(|v| v.as_str()) {
                // Create the user account
                let user_id = match sqlx::query!(
                    "INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id",
                    username,
                    token_record.email
                )
                .fetch_one(&mut *tx)
                .await
                {
                    Ok(record) => {
                        tracing::info!("🌱 New user registered - {} ({})", username, token_record.email);
                        record.id
                    },
                    Err(e) => {
                        tracing::error!("Failed to create user account: {}", e);
                        return Err(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::from("Failed to create user account"))
                            .unwrap());
                    }
                };
                
                user_id
            } else {
                tracing::error!("Username not found in token data");
                return Err(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from("Invalid registration data"))
                    .unwrap());
            }
        } else {
            tracing::error!("Token data is missing");
            return Err(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Invalid registration data"))
                .unwrap());
        }
    } else {
        // This is a login token, check if user exists
        
        match sqlx::query!(
            "SELECT id FROM users WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL",
            token_record.email
        )
        .fetch_optional(&mut *tx)
        .await
        {
            Ok(Some(record)) => {
                record.id
            },
            Ok(None) => {
                tracing::error!("User not found for email: {}", token_record.email);
                return Err(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from(serde_json::json!({
                        "error": "User not found",
                        "email": token_record.email,
                        "needs_registration": true
                    }).to_string()))
                    .unwrap());
            },
            Err(e) => {
                tracing::error!("Database error when looking up user: {}", e);
                return Err(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Database error"))
                    .unwrap());
            }
        }
    };
    
    // Authenticate the user
    let (csrf_token, token) = auth::handle_authentication(&mut tx, user_id, &mut headers).await.map_err(|e| {
        e.into_response()
    })?;
    
    // Get user details for response
    let user = sqlx::query!(
        "SELECT username, currency_balance FROM users WHERE id = $1",
        user_id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Database error"))
            .unwrap()
    })?;
    
    tx.commit().await.map_err(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Database error"))
            .unwrap()
    })?;
    
    // Clone the username for logging
    let username = user.username.clone();
    
    let response = AuthResponse {
        csrf_token,
        token,
        requires_captcha: false,
        current_attempts: None,
        currency_balance: user.currency_balance,
        user_id: user_id.to_string(),
        username: user.username,
    };
    
    tracing::info!("👤 User '{}' logged in successfully via magic link", username);
    
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(serde_json::to_string(&response).unwrap_or_default()))
        .unwrap())
}

async fn send_magic_link_email(email: &str, magic_link: &str) -> Result<(), AuthError> {
    let email_body = format!(
        r#"
        <html>
        <body style="font-family: Arial, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
            <div style="text-align: center; margin-bottom: 20px;">
                <h1 style="color: #4a5568; margin-bottom: 10px;">Sign in to FRTL</h1>
            </div>
            <p>Click the button below to sign in to your FRTL account:</p>
            <div style="text-align: center; margin: 30px 0;">
                <a href="{}" style="background-color: #4f46e5; color: white; padding: 16px 32px; text-decoration: none; border-radius: 6px; font-weight: bold; display: inline-block; font-size: 18px; box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1); transition: all 0.3s ease;">Sign in to FRTL</a>
            </div>
            <p style="margin-bottom: 5px;">If the button doesn't work, you can copy and paste this link into your browser:</p>
            <p style="word-break: break-all; background-color: #f3f4f6; padding: 10px; border-radius: 4px; font-size: 14px;">{}</p>
            <p style="color: #718096; font-size: 14px; margin-top: 30px;">This link will expire in 15 minutes.</p>
            <hr style="border: none; border-top: 1px solid #e2e8f0; margin: 20px 0;">
            <p style="color: #718096; font-size: 12px; text-align: center;">If you didn't request this email, you can safely ignore it. FRTL will never request personal information via email.</p>
        </body>
        </html>
        "#,
        magic_link,
        magic_link
    );

    let email = Message::builder()
        .from("frtl@jaykrown.com".parse().map_err(|e| {
            tracing::error!("Failed to parse from address: {:?}", e);
            AuthError::Database(sqlx::Error::PoolTimedOut)
        })?)
        .to(email.parse().map_err(|e| {
            tracing::error!("Failed to parse to address: {:?}", e);
            AuthError::Database(sqlx::Error::PoolTimedOut)
        })?)
        .subject("Sign in to FRTL")
        .header(ContentType::TEXT_HTML)
        .body(email_body)
        .map_err(|e| {
            tracing::error!("Failed to build email: {:?}", e);
            AuthError::Database(sqlx::Error::PoolTimedOut)
        })?;

    let smtp_username = env::var("SMTP_USERNAME").map_err(|e| {
        tracing::error!("Failed to get SMTP_USERNAME: {:?}", e);
        AuthError::Database(sqlx::Error::PoolTimedOut)
    })?;
    
    let smtp_password = env::var("SMTP_PASSWORD").map_err(|e| {
        tracing::error!("Failed to get SMTP_PASSWORD: {:?}", e);
        AuthError::Database(sqlx::Error::PoolTimedOut)
    })?;
    
    let smtp_host = env::var("SMTP_HOST").map_err(|e| {
        tracing::error!("Failed to get SMTP_HOST: {:?}", e);
        AuthError::Database(sqlx::Error::PoolTimedOut)
    })?;

    let creds = Credentials::new(smtp_username, smtp_password);

    // Create a secure SMTP transport with SSL on port 465
    let mailer = SmtpTransport::relay(&smtp_host)
        .map_err(|e| {
            tracing::error!("Failed to create SMTP transport: {:?}", e);
            AuthError::Database(sqlx::Error::PoolTimedOut)
        })?
        .credentials(creds)
        .port(465)
        .build();

    match mailer.send(&email) {
        Ok(_) => Ok(()),
        Err(e) => {
            tracing::error!("Failed to send email: {:?}", e);
            Err(AuthError::Database(sqlx::Error::PoolTimedOut))
        }
    }
} 