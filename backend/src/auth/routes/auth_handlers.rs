use axum::{
    extract::State,
    http::{Response, StatusCode, HeaderMap},
    Json,
    body::Body,
};
use hyper::header;
use tracing::{info, error};
use serde_json::json;
use std::env;
use cookie::Cookie;
use shared::profanity::ProfanityFilter;
use chrono;
use rand::distributions::{Alphanumeric, DistString};
use lettre::{
    message::header::ContentType,
    transport::smtp::{
        authentication::Credentials,
        client::{TlsParameters, Tls},
    },
    Message,
    SmtpTransport,
    Transport,
};
use reqwest;
use time::OffsetDateTime;

use crate::{AppState, auth::{
    self, AuthError, models::{LoginCredentials, RegisterCredentials, AuthResponse},
    services::{
        record_login_attempt, handle_authentication
    }
}};
use crate::auth::utils::get_frontend_url;

fn create_error_response(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(format!("{{\"error\":\"{}\"}}", message)))
        .unwrap()
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(credentials): Json<LoginCredentials>,
) -> Result<Response<Body>, Response<Body>> {
    info!(
        event = "login_attempt",
        username = %credentials.username,
        timestamp = %chrono::Utc::now().to_rfc3339(),
        "User attempting to login: {}", credentials.username
    );

    // Get client IP address from headers
    let ip_address = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1")
        .split(',')
        .next()
        .unwrap_or("127.0.0.1")
        .trim();

    // Record the login attempt
    if let Err(e) = record_login_attempt(&state.pool, &credentials.username, ip_address, false).await {
        error!("Failed to record login attempt: {:?}", e);
    }

    // In passwordless system, always direct users to use magic links
    let response = json!({
        "error": "This system uses passwordless authentication. Please use magic link to login.",
        "requires_magic_link": true
    });

    Ok(Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&response).unwrap()))
        .unwrap())
}

pub async fn register(
    State(state): State<AppState>,
    Json(credentials): Json<RegisterCredentials>,
) -> Result<Response<Body>, Response<Body>> {
    // Record the start time to ensure consistent response times
    let start_time = std::time::Instant::now();
    
    // Validate hCaptcha token
    let hcaptcha_secret = env::var("HCAPTCHA_SECRET_KEY").map_err(|_| {
        create_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
    })?;

    let client = reqwest::Client::new();
    let res = client
        .post("https://hcaptcha.com/siteverify")
        .form(&[
            ("secret", hcaptcha_secret.as_str()),
            ("response", credentials.captcha_token.as_str()),
        ])
        .send()
        .await
        .map_err(|_| {
            create_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify captcha")
        })?;

    let captcha_response: serde_json::Value = res.json().await.map_err(|_| {
        create_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to parse captcha response")
    })?;

    if !captcha_response["success"].as_bool().unwrap_or(false) {
        return Err(create_error_response(StatusCode::BAD_REQUEST, "Invalid captcha"));
    }

    // Validate profanity
    if let Err(msg) = ProfanityFilter::validate_username(&credentials.username) {
        return Err(create_error_response(StatusCode::BAD_REQUEST, &msg));
    }
    if let Err(msg) = ProfanityFilter::validate_email_local_part(&credentials.email) {
        return Err(create_error_response(StatusCode::BAD_REQUEST, &msg));
    }

    // Check if username or email already exists
    let existing_user = sqlx::query!(
        "SELECT id FROM users WHERE LOWER(username) = LOWER($1) OR LOWER(email) = LOWER($2)",
        credentials.username,
        credentials.email
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        create_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Database error: {}", e),
        )
    })?;

    // Create a future that will be completed regardless of whether the username/email exists
    let registration_future = async {
        if existing_user.is_none() {
            // Only proceed with registration if the user doesn't exist
            
            // Start transaction
            let mut tx = state.pool.begin().await.map_err(|e| {
                create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Database error: {}", e),
                )
            })?;

            // Generate a magic link token
            let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
            let expires_at = OffsetDateTime::now_utc() + time::Duration::hours(24);

            // Store the registration information in the token_data field
            let token_data = serde_json::json!({
                "username": credentials.username,
                "is_registration": true
            });

            match sqlx::query!(
                "INSERT INTO magic_link_tokens (email, token, expires_at, token_data) VALUES ($1, $2, $3, $4)",
                credentials.email,
                token,
                expires_at,
                token_data
            )
            .execute(&mut *tx)
            .await
            {
                Ok(_) => (),
                Err(e) => {
                    return Err(create_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Failed to store token: {}", e),
                    ));
                }
            }

            // Commit transaction
            if let Err(e) = tx.commit().await {
                return Err(create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Failed to commit transaction: {}", e),
                ));
            }

            // Send magic link email
            let magic_link = format!(
                "{}/verify-magic-link?token={}",
                get_frontend_url(),
                token
            );

            match send_magic_link_email(&credentials.email, &magic_link).await {
                Ok(_) => (),
                Err(e) => {
                    return Err(create_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Failed to send email: {}", e),
                    ));
                }
            }
        }
        
        Ok(())
    };

    // Execute the registration process
    let result = registration_future.await;

    // Calculate how much time has passed
    let elapsed = start_time.elapsed();

    // Ensure the function takes at least 5 seconds to complete, regardless of whether
    // the registration succeeds or not, to prevent timing attacks
    let min_processing_time = std::time::Duration::from_secs(5);
    if elapsed < min_processing_time {
        tokio::time::sleep(min_processing_time - elapsed).await;
    }

    // If there was a real error in the process, return it
    if let Err(e) = result {
        return Err(e);
    }

    // Always return the same success message regardless of whether the registration actually happened
    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .body(Body::from(json!({
            "message": "Registration request received. If your email is available, you will receive a confirmation link shortly. Please check your inbox and spam folder."
        }).to_string()))
        .unwrap())
}

async fn send_magic_link_email(email: &str, magic_link: &str) -> Result<(), AuthError> {
    let smtp_username = std::env::var("SMTP_USERNAME").map_err(|_| {
        AuthError::Database(sqlx::Error::Configuration(
            "SMTP_USERNAME not set".into(),
        ))
    })?;
    let smtp_password = std::env::var("SMTP_PASSWORD").map_err(|_| {
        AuthError::Database(sqlx::Error::Configuration(
            "SMTP_PASSWORD not set".into(),
        ))
    })?;
    let smtp_host = std::env::var("SMTP_HOST").map_err(|_| {
        AuthError::Database(sqlx::Error::Configuration(
            "SMTP_HOST not set".into(),
        ))
    })?;

    let email_body = format!(
        r#"
        <html>
        <body style="font-family: Arial, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
            <div style="text-align: center; margin-bottom: 20px;">
                <h1 style="color: #4a5568; margin-bottom: 10px;">Welcome to FRTL!</h1>
            </div>
            <p>Thank you for registering. To complete your registration and log in, please click the button below:</p>
            <div style="text-align: center; margin: 30px 0;">
                <a href="{}" style="background-color: #4f46e5; color: white; padding: 16px 32px; text-decoration: none; border-radius: 6px; font-weight: bold; display: inline-block; font-size: 18px; box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1); transition: all 0.3s ease;">Complete Registration</a>
            </div>
            <p style="margin-bottom: 5px;">If the button doesn't work, you can copy and paste this link into your browser:</p>
            <p style="word-break: break-all; background-color: #f3f4f6; padding: 10px; border-radius: 4px; font-size: 14px;">{}</p>
            <p style="color: #718096; font-size: 14px; margin-top: 30px;">This link will expire in 24 hours.</p>
            <hr style="border: none; border-top: 1px solid #e2e8f0; margin: 20px 0;">
            <p style="color: #718096; font-size: 12px; text-align: center;">If you didn't request this registration, you can safely ignore this email. FRTL will never request personal information via email.</p>
        </body>
        </html>
        "#,
        magic_link,
        magic_link
    );

    let email_message = Message::builder()
        .from("frtl@jaykrown.com".parse().unwrap())
        .to(format!("<{}>", email).parse().unwrap())
        .subject("Complete Your FRTL Registration")
        .header(ContentType::TEXT_HTML)
        .body(email_body)
        .unwrap();

    let creds = Credentials::new(smtp_username, smtp_password);

    let mailer = SmtpTransport::relay(&smtp_host)
        .map_err(|e| {
            AuthError::Database(sqlx::Error::Configuration(
                format!("Failed to create SMTP transport: {}", e).into(),
            ))
        })?
        .credentials(creds)
        .port(587)
        .tls(Tls::Required(TlsParameters::new(smtp_host.clone()).map_err(|e| {
            AuthError::Database(sqlx::Error::Configuration(
                format!("Failed to create TLS parameters: {}", e).into(),
            ))
        })?))
        .build();

    mailer.send(&email_message).map_err(|e| {
        AuthError::Database(sqlx::Error::Configuration(
            format!("Failed to send email: {}", e).into(),
        ))
    })?;

    Ok(())
}

pub async fn refresh_token(
    State(state): State<AppState>,
    cookies: HeaderMap,
) -> Result<Response<Body>, Response<Body>> {
    let refresh_token = match cookies
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookie_str| Cookie::parse(cookie_str).ok())
        .and_then(|cookie| (cookie.name() == "refresh_token").then(|| cookie.value().to_string()))
    {
        Some(token) => token,
        None => {
            return Err(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"error": "Invalid token"}).to_string()))
                .unwrap());
        }
    };

    let user_id = match auth::validate_jwt(&refresh_token) {
        Ok(id) => id,
        Err(_) => {
            return Err(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"error": "Invalid token"}).to_string()))
                .unwrap());
        }
    };
    
    let user = match sqlx::query!(
        r#"SELECT currency_balance, username FROM users WHERE id = $1"#,
        user_id
    )
    .fetch_one(&state.pool)
    .await
    {
        Ok(user) => user,
        Err(e) => {
            error!("Database error: {:?}", e);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"error": "Database error"}).to_string()))
                .unwrap());
        }
    };

    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            error!("Database error: {:?}", e);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"error": "Database error"}).to_string()))
                .unwrap());
        }
    };
    
    let mut response_headers = HeaderMap::new();
    let (csrf_token, token) = match handle_authentication(&mut tx, user_id, &mut response_headers).await {
        Ok(tokens) => tokens,
        Err(e) => {
            error!("Authentication error: {:?}", e);
            return Err(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"error": "Authentication error"}).to_string()))
                .unwrap());
        }
    };
    
    if let Err(e) = tx.commit().await {
        error!("Database error: {:?}", e);
        return Err(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header("Content-Type", "application/json")
            .body(Body::from(json!({"error": "Database error"}).to_string()))
            .unwrap());
    }

    let response = AuthResponse { 
        csrf_token, 
        token, 
        requires_captcha: false, 
        current_attempts: None,
        currency_balance: user.currency_balance,
        user_id: user_id.to_string(),
        username: user.username,
    };
    
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(serde_json::to_string(&response).unwrap_or_default()))
        .unwrap())
}