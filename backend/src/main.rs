use std::net::SocketAddr;
use axum::http::{HeaderValue, Method, Request, Response, StatusCode, header};
use axum::http::header::HeaderName;
use axum::{middleware, Router, extract::State, Extension};
use axum::routing::{post, get, delete};
use axum::response::IntoResponse;
use axum::body::Body;
use cookie::Cookie;
use rand::Rng;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::{SetResponseHeader, SetResponseHeaderLayer};
use tower::Layer;
use tracing::{info, error};
use redis::Client as RedisClient;
use tower_http::services::fs::ServeFileSystemResponseBody;
use shared::rate_limit::{RateLimitType, get_rate_limit_key, API_MAX_REQUESTS, API_WINDOW, RateLimitCheck};
use serde_json::json;
use std::sync::Arc;
use std::collections::HashMap;
use axum::{extract::{Path, Query}, Json};
use serde::Deserialize;
use serde::Serialize;
use crate::services::user_service::{get_game_leaderboard, LeaderboardEntry};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use time;
use crate::services::patreon_link_service;
use crate::handlers::user_leaderboard_handler;
use crate::handlers::game_leaderboard_handler;

use crate::auth::routes::{
    login, register, refresh_token,
    change_email, change_password, delete_account, get_profile,
    request_magic_link, verify_magic_link,
    request_delete_account, verify_delete_account
};
use crate::auth::middleware::require_auth;
use crate::generator::{generate_egg, generate_creature, membership_code_routes};
use crate::services::{
    creature_service::*, 
    claim_service, 
    creature_bind::bind_creature, 
    chaos_realm::*,
    energy_service::{handle_energy_recharge, check_expired_energy_recharges},
    market_service::*,
    scroll_service::*,
    orderbook_service::{create_order, get_orders, cancel_order, fulfill_order},
    magic_button_service,
    patreon_link_service::{link_patreon_account, unlink_patreon_account, get_patreon_status, get_oauth_url, handle_oauth_callback},
};
use crate::games::backend_match_game::{create_router as create_match_game_router, GameState};
use crate::games::backend_snake_game::{create_router as create_snake_game_router, SnakeGameState};
use crate::games::backend_2048_game::{create_router as create_2048_game_router, Game2048State};
use crate::games::backend_wheel_game::create_router as create_wheel_game_router;
use crate::games::backend_word_game::{create_router as create_word_game_router, WordGameState};
use crate::games::backend_hexort_game::create_router as create_hexort_game_router;

mod auth;
mod services;
mod utils;
mod models;
mod handlers;
mod generator;
mod logging;
mod patreon_handler;
mod games;

#[derive(Clone)]
pub struct AppState {
    pool: PgPool,
    redis: RedisClient,
}

// Insert IP rate limiting constants
const IP_MAX_REQUESTS: u32 = 1000;
const IP_WINDOW: u64 = 10;

// Custom error for rate limiting
struct RateLimitError;

impl axum::response::IntoResponse for RateLimitError {
    fn into_response(self) -> axum::response::Response {
        let body = json!({ "error": "Too many requests, please try again later." });
        axum::response::Response::builder()
            .status(axum::http::StatusCode::TOO_MANY_REQUESTS)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap()
    }
}

#[derive(Deserialize)]
pub struct LeaderboardQuery {
    pub limit: Option<i64>,
}

#[axum::debug_handler]
pub async fn leaderboard_handler(
    State(state): State<AppState>,
    Path(game_type): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<LeaderboardEntry>>, StatusCode> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(10);
        
    match get_game_leaderboard(&state.pool, &game_type, limit).await {
        Ok(entries) => Ok(Json(entries)),
        Err(e) => {
            tracing::error!("Failed to get leaderboard for game {}: {}", game_type, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn health_check() -> impl IntoResponse {
    Response::builder().status(200).body(Body::from("OK")).unwrap()
}

fn get_mime_type(response: &Response<ServeFileSystemResponseBody>) -> Option<HeaderValue> {
    let path = response.headers().get("x-path")?;
    let path = path.to_str().ok()?;
    
    match path.split('.').last()? {
        "js" => Some(HeaderValue::from_static("application/javascript")),
        "html" => Some(HeaderValue::from_static("text/html")),
        "css" => Some(HeaderValue::from_static("text/css")),
        "png" => Some(HeaderValue::from_static("image/png")),
        "jpg" | "jpeg" => Some(HeaderValue::from_static("image/jpeg")),
        "avif" => Some(HeaderValue::from_static("image/avif")),
        "svg" => Some(HeaderValue::from_static("image/svg+xml")),
        "wasm" => Some(HeaderValue::from_static("application/wasm")),
        "ico" => Some(HeaderValue::from_static("image/x-icon")),
        "json" => Some(HeaderValue::from_static("application/json")),
        "txt" => Some(HeaderValue::from_static("text/plain")),
        _ => Some(HeaderValue::from_static("application/octet-stream"))
    }
}

async fn api_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response<Body>, StatusCode> {
    let user_id = request
        .extensions()
        .get::<auth::middleware::UserId>()
        .map(|id| id.0.to_string())
        .unwrap_or_else(|| "anonymous".to_string());

    let rate_limit_key = get_rate_limit_key(RateLimitType::Api, &user_id);

    if let Ok(mut conn) = state.redis.get_async_connection().await {
        let attempts: Option<u32> = redis::cmd("GET")
            .arg(&rate_limit_key)
            .query_async(&mut conn)
            .await
            .unwrap_or(None);

        let check = RateLimitCheck::new(attempts.unwrap_or(0), RateLimitType::Api);
        
        if check.is_locked {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }

        let current_attempts = attempts.unwrap_or(0) + 1;
        if current_attempts <= API_MAX_REQUESTS {
            let _: () = redis::cmd("SETEX")
                .arg(&rate_limit_key)
                .arg(API_WINDOW.as_secs())
                .arg(current_attempts)
                .query_async(&mut conn)
                .await
                .unwrap_or(());
        }
    }

    Ok(next.run(request).await)
}

// New IP rate limiting middleware
async fn ip_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response<Body>, Response<Body>> {
    // Extract client IP from headers: prefer "cf-connecting-ip", then "x-forwarded-for", then "x-real-ip", else default to "unknown"
    let client_ip = if let Some(cf_ip) = request.headers().get("cf-connecting-ip") {
        if let Ok(ip_str) = cf_ip.to_str() {
            ip_str.trim().to_string()
        } else {
            "unknown".to_string()
        }
    } else if let Some(forwarded) = request.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            forwarded_str.split(',').next().unwrap_or("unknown").trim().to_string()
        } else {
            "unknown".to_string()
        }
    } else if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(real_ip_str) = real_ip.to_str() {
            real_ip_str.trim().to_string()
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    };

    let key = format!("rate:ip:{}", client_ip);

    if let Ok(mut conn) = state.redis.get_async_connection().await {
        let attempts: Option<u32> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .unwrap_or(None);

        if attempts.unwrap_or(0) >= IP_MAX_REQUESTS {
            return Err(RateLimitError.into_response());
        }

        let current_attempts = attempts.unwrap_or(0) + 1;
        if current_attempts <= IP_MAX_REQUESTS {
            let _: () = redis::cmd("SETEX")
                .arg(&key)
                .arg(IP_WINDOW)
                .arg(current_attempts)
                .query_async(&mut conn)
                .await
                .unwrap_or(());
        }
    }

    Ok(next.run(request).await)
}

// Track which IP addresses have been logged with timestamps
static LOGGED_IPS: Lazy<Mutex<HashMap<String, u64>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

// Time before we log the same IP again (in seconds)
const IP_LOG_EXPIRY: u64 = 3600; // 1 hour

async fn log_visit_middleware(
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response<Body>, StatusCode> {
    // Extract IP address before moving the request
    let ip = if let Some(cf_ip) = request.headers().get("cf-connecting-ip") {
        if let Ok(ip_str) = cf_ip.to_str() {
            ip_str.trim().to_string()
        } else {
            "unknown".to_string()
        }
    } else if let Some(forwarded) = request.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            forwarded_str.split(',').next().unwrap_or("unknown").trim().to_string()
        } else {
            "unknown".to_string()
        }
    } else if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(real_ip_str) = real_ip.to_str() {
            real_ip_str.trim().to_string()
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    };
    
    // Get the request path
    let path = request.uri().path();
    
    // Get the user agent if available
    let user_agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");
    
    // Get current time in seconds
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();
    
    // Check if we should log this IP
    let should_log = {
        let mut logged_ips = LOGGED_IPS.lock().unwrap();
        
        if let Some(last_time) = logged_ips.get(&ip) {
            // Check if enough time has passed since the last log
            if now - last_time > IP_LOG_EXPIRY {
                // Update the timestamp
                logged_ips.insert(ip.clone(), now);
                true
            } else {
                // Always log robots.txt requests regardless of time
                path == "/robots.txt"
            }
        } else {
            // First time seeing this IP
            logged_ips.insert(ip.clone(), now);
            true
        }
    };
    
    // If we should log this IP, do it
    if should_log {
        if path == "/robots.txt" {
            info!("🤖 Robots.txt request: IP={}, User-Agent=\"{}\"", ip, user_agent);
        } else {
            info!("👋 Website visit: {} ✅ Files served successfully", ip);
        }
    }
    
    // Process the request
    let response = next.run(request).await;
    
    Ok(response)
}

// Generic file serving function to reduce redundancy
async fn serve_file(
    relative_path: &str,
    content_type: &'static str,
    path_modifier: Option<impl FnOnce(String) -> String>,
    default_content: Option<&'static str>,
    error_log_prefix: &str
) -> Result<(StatusCode, [(HeaderName, &'static str); 2], Vec<u8>), (StatusCode, &'static str)> {
    // Get the current working directory
    let current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            error!("Failed to get current directory: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Server Error"));
        }
    };
    
    // Construct absolute path to the file
    let mut path = current_dir.join(relative_path);
    
    // Apply path modifier if provided
    if let Some(modifier) = path_modifier {
        let path_str = path.to_string_lossy().to_string();
        let modified_path = modifier(path_str);
        path = std::path::PathBuf::from(modified_path);
    }
    
    match tokio::fs::read(&path).await {
        Ok(data) => {
            Ok((
                StatusCode::OK, 
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                ], 
                data
            ))
        },
        Err(e) => {
            // If default content is provided, use it instead of returning an error
            if let Some(content) = default_content {
                info!("Using default {} content", relative_path);
                return Ok((
                    StatusCode::OK, 
                    [
                        (header::CONTENT_TYPE, content_type),
                        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    ], 
                    content.as_bytes().to_vec()
                ));
            }
            
            // Enhanced error logging with file path and working directory details
            error!("{}: {} (attempted path: {:?}, working dir: {:?})", 
                  error_log_prefix, e, path, current_dir);
            
            // Check if file exists at common alternative locations
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            let alt_paths = [
                format!("./static/frontend/{}", filename),
                format!("./backend/static/frontend/{}", filename),
            ];
            
            for alt_path in &alt_paths {
                if std::path::Path::new(alt_path).exists() {
                    error!("Note: File does exist at alternative path: {}", alt_path);
                }
            }
            
            Err((StatusCode::NOT_FOUND, "Not Found"))
        }
    }
}

async fn serve_favicon() -> Result<(StatusCode, [(HeaderName, &'static str); 2], Vec<u8>), (StatusCode, &'static str)> {
    serve_file(
        "static/images/favicon.ico",
        "image/x-icon",
        None::<fn(String) -> String>,
        None,
        "Error reading favicon"
    ).await
}

async fn serve_robots() -> Result<(StatusCode, [(HeaderName, &'static str); 3], Vec<u8>), (StatusCode, &'static str)> {
    let content = serve_file(
        "static/robots.txt",
        "text/plain",
        None::<fn(String) -> String>,
        Some("User-agent: *\nDisallow: /\n"),
        "Error reading robots.txt"
    ).await?;
    
    // Return with an additional Cache-Control header to prevent caching
    Ok((
        content.0,
        [
            (HeaderName::from_static("content-type"), content.1[0].1),
            (HeaderName::from_static("cache-control"), "no-cache, no-store, must-revalidate"),
            (HeaderName::from_static("pragma"), "no-cache"),
        ],
        content.2
    ))
}

// Add a fallback handler to serve the frontend index.html
async fn serve_frontend_index() -> Result<(StatusCode, [(HeaderName, &'static str); 2], Vec<u8>), (StatusCode, &'static str)> {
    serve_file(
        "../frontend/dist/index.html",
        "text/html",
        None::<fn(String) -> String>,
        None,
        "Error reading index.html"
    ).await
}

// Add routes for frontend assets with the correct patterns to match index.html
async fn serve_frontend_js(Path(js_hash): Path<String>) -> Result<(StatusCode, [(HeaderName, &'static str); 2], Vec<u8>), (StatusCode, &'static str)> {
    let clean_hash = js_hash.trim_end_matches(".js");
    
    serve_file(
        &format!("../frontend/dist/frontend-{}.js", clean_hash),
        "application/javascript; charset=utf-8",
        None::<fn(String) -> String>,
        None,
        "Error reading JavaScript file"
    ).await
}

async fn serve_frontend_wasm(Path(wasm_hash): Path<String>) -> Result<(StatusCode, [(HeaderName, &'static str); 2], Vec<u8>), (StatusCode, &'static str)> {
    serve_file(
        &format!("../frontend/dist/frontend-{}_bg.wasm", wasm_hash),
        "application/wasm",
        None::<fn(String) -> String>,
        None,
        "Error reading WebAssembly file"
    ).await
}

async fn serve_frontend_css(Path(css_hash): Path<String>) -> Result<(StatusCode, [(HeaderName, &'static str); 2], Vec<u8>), (StatusCode, &'static str)> {
    serve_file(
        &format!("../frontend/dist/main-{}.css", css_hash),
        "text/css",
        None::<fn(String) -> String>,
        None,
        "Error reading CSS file"
    ).await
}

#[derive(Serialize)]
pub struct GlobalStats {
    scrolls_count: i64,
    eggs_count: i64,
    creatures_count: i64,
    total_soul: i64,
}

async fn get_global_stats(
    State(state): State<AppState>,
) -> Result<Json<GlobalStats>, StatusCode> {
    // Get total counts from database
    let scrolls_count = sqlx::query_scalar!("SELECT COALESCE(SUM(quantity), 0) FROM scrolls")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            error!("Failed to get scrolls count: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .unwrap_or(0);

    // Count all eggs that haven't been hatched yet, including those on the market
    let eggs_count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM eggs e 
         WHERE NOT EXISTS (
             SELECT 1 FROM creatures c 
             WHERE c.original_egg_id = e.id
         )"
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to get eggs count: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .unwrap_or(0);

    // Count all creatures, including those on the market
    let creatures_count = sqlx::query_scalar!("SELECT COUNT(*) FROM creatures")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            error!("Failed to get creatures count: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .unwrap_or(0);
    
    // Calculate total soul across all creatures
    let total_soul = sqlx::query_scalar!("SELECT COALESCE(SUM(soul), 0) FROM creatures")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            error!("Failed to get total soul: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .unwrap_or(0);

    Ok(Json(GlobalStats {
        scrolls_count,
        eggs_count,
        creatures_count,
        total_soul,
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::setup();
    dotenvy::from_path(".env").ok();

    let state = AppState {
        pool: PgPool::connect_with(
            std::env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set")
                .parse::<sqlx::postgres::PgConnectOptions>()?
                .to_owned()
        )
        .await
        .expect("Failed to create pool"),
        redis: RedisClient::open(std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()))
            .expect("Failed to connect to Redis"),
    };

    // Check for creatures with expired energy recharge times on startup
    info!("Checking for creatures with expired energy recharge times on startup...");
    if let Err(e) = check_expired_energy_recharges(&state.pool).await {
        error!("Error checking expired energy recharges on startup: {:?}", e);
    }

    // Start background task to check expired memberships
    let pool_clone = state.pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Every hour
        loop {
            interval.tick().await;
            if let Err(e) = check_expired_memberships(&pool_clone).await {
                error!("Error checking expired memberships: {:?}", e);
            }
            if let Err(e) = cleanup_expired_membership_codes(&pool_clone).await {
                error!("Error cleaning up expired membership codes: {:?}", e);
            }
            // Add Patreon membership sync
            if let Err(e) = patreon_link_service::sync_patreon_memberships(&pool_clone).await {
                error!("Error syncing Patreon memberships: {:?}", e);
            }
            // Add Patreon token refresh
            if let Err(e) = patreon_link_service::refresh_expired_tokens(&pool_clone).await {
                error!("Error refreshing Patreon tokens: {:?}", e);
            }
            // Check for creatures with expired energy recharge times
            if let Err(e) = check_expired_energy_recharges(&pool_clone).await {
                error!("Error checking expired energy recharges: {:?}", e);
            }
        }
    });

    let auth_routes = Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/refresh", post(refresh_token))
        .route("/magic-link/request", post(request_magic_link))
        .route("/magic-link/verify", post(verify_magic_link));

    let user_routes = Router::new()
        .route("/me", axum::routing::delete(delete_account))
        .route("/me/email", axum::routing::put(change_email))
        .route("/me/password", axum::routing::put(change_password))
        .route("/me/delete-request", axum::routing::post(request_delete_account))
        .route("/me/verify-delete", axum::routing::post(verify_delete_account));

    let protected_routes = Router::new()
        .route("/api/eggs", axum::routing::get(get_user_eggs).post(generate_egg))
        .route("/api/generator/generate-egg", axum::routing::post(generate_egg))
        .route("/api/eggs/:id/generate-creature", post(generate_creature))
        .route("/api/creatures", axum::routing::get(get_user_creatures))
        .route("/api/creatures/:id/bind", post(bind_creature))
        .route("/api/creatures/:id/rename", post(rename_creature_handler))
        .route("/api/creatures/:id/energy_recharge", post(handle_energy_recharge))
        .route("/api/creatures/:id/chaos-realm/enter", post(enter_chaos_realm))
        .route("/api/creatures/:id/chaos-realm/claim", post(claim_chaos_realm_reward))
        .route("/api/creatures/:id/chaos-realm/status", axum::routing::get(get_chaos_realm_status))
        .route("/api/scrolls", axum::routing::get(get_scrolls))
        .route("/api/scrolls/:id", axum::routing::get(get_scroll_by_id))
        .route("/api/profile", axum::routing::get(get_profile))
        .route("/api/daily-claim", post(claim_service::claim_daily_reward))
        .route("/api/daily-claim/reset-streak", post(claim_service::reset_claim_streak))
        .route("/api/daily-claim/status", get(claim_service::get_claim_status))
        .route("/api/game-session", post(claim_service::create_game_session))
        .route("/api/game-reward", post(claim_service::handle_game_reward))
        .route("/api/game-scroll-reward", post(claim_service::handle_game_scroll_reward))
        .route("/api/market/listings", get(get_active_listings))
        .route("/api/market/listings", post(create_listing))
        .route("/api/market/listings/:id/buy", post(purchase_item))
        .route("/api/market/listings/:id", delete(cancel_listing))
        .route("/api/market/listings/:id/item", get(get_listing_item))
        .route("/api/scrolls/orders", get(get_orders))
        .route("/api/scrolls/orders", post(create_order))
        .route("/api/scrolls/orders/:id", delete(cancel_order))
        .route("/api/scrolls/orders/:id/fulfill", post(fulfill_order))
        .route("/api/webhooks/patreon", post(patreon_handler::patreon_webhook_handler))
        .route("/api/magic-button", post(magic_button_service::handle_magic_button))
        .route("/api/magic-button/status", get(magic_button_service::get_magic_button_status))
        .route("/api/settings/patreon/link", post(link_patreon_account))
        .route("/api/settings/patreon/unlink", post(unlink_patreon_account))
        .route("/api/settings/patreon/status", get(get_patreon_status))
        .route("/api/settings/patreon/oauth/url", get(get_oauth_url))
        .route("/api/settings/patreon/oauth/callback", post(handle_oauth_callback))
        .layer(Extension(state.pool.clone()))
        .layer(Extension(state.redis.clone()))
        .layer(middleware::from_fn_with_state(state.clone(), api_rate_limit_middleware))
        .layer(middleware::from_fn(require_auth));

    let cors = CorsLayer::new()
        .allow_origin(vec![
            "http://127.0.0.1:8080".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:3000".parse::<HeaderValue>().unwrap(),
            "https://frtl.dev".parse::<HeaderValue>().unwrap()
        ])
        .allow_methods(vec![Method::GET, Method::POST, Method::PUT, Method::OPTIONS, Method::DELETE])
        .allow_headers(vec![
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("x-requested-with"),
            HeaderName::from_static("x-session-signature")
        ])
        .allow_credentials(true);

    let static_path = if std::path::Path::new("static").exists() {
        std::path::Path::new("static").to_path_buf()
    } else if std::path::Path::new("backend/static").exists() {
        info!("Using alternative static path: backend/static");
        std::path::Path::new("backend/static").to_path_buf()
    } else if std::path::Path::new("../backend/static").exists() {
        info!("Using alternative static path: ../backend/static");
        std::path::Path::new("../backend/static").to_path_buf()
    } else {
        // Default to static even if it doesn't exist
        std::path::Path::new("static").to_path_buf()
    };
    
    let static_service = ServeDir::new(&static_path);
    
    // Create the Cache-Control layer first
    let cache_control_layer = SetResponseHeaderLayer::if_not_present(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache") // Use no-cache to force revalidation
    );
    
    // Apply Cache-Control layer to ServeDir
    let layered_service = cache_control_layer.layer(static_service);
    
    // Now apply the Content-Type setting to the layered service
    let final_static_service = SetResponseHeader::overriding(
        layered_service, 
        header::CONTENT_TYPE,
        get_mime_type
    );

    let game_state = Arc::new(GameState {
        sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new()))
    });

    let snake_game_state = Arc::new(SnakeGameState {
        sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        pool: state.pool.clone(),
        redis: state.redis.clone(),
    });

    let game_2048_state = Arc::new(Game2048State {
        sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    });

    let word_game_state = Arc::new(WordGameState {
        sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        redis: state.redis.clone(),
    });

    let app = Router::new()
        .route("/favicon.svg", axum::routing::get(serve_favicon))
        .route("/robots.txt", axum::routing::get(serve_robots))
        .route("/js-:js_hash", axum::routing::get(serve_frontend_js))
        .route("/wasm-:wasm_hash", axum::routing::get(serve_frontend_wasm))
        .route("/css-:css_hash", axum::routing::get(serve_frontend_css))
        .route("/api/health_check", axum::routing::get(health_check))
        .route("/api/patreon/fetch", post(patreon_handler::fetch_supporters_handler))
        .route("/api/stats/global", axum::routing::get(get_global_stats))
        .route("/api/creatures/showcase", axum::routing::get(get_public_showcase_creatures))
        .layer(Extension(state.clone()))
        .nest("/api/auth", auth_routes)
        .nest("/api/users", user_routes)
        .merge(protected_routes)
        .merge(membership_code_routes(state.pool.clone()))
        .nest_service("/static", final_static_service)
        .route("/api/leaderboard/users", get(user_leaderboard_handler))
        .route("/api/leaderboard/:game_type", get(game_leaderboard_handler))
        .layer(cors.clone())
        .layer(middleware::from_fn_with_state(state.clone(), ip_rate_limit_middleware))
        .layer(middleware::from_fn(csrf_token_middleware))
        .layer(middleware::from_fn(log_visit_middleware))
        .nest("/snake-game", create_snake_game_router()
            .with_state(snake_game_state)
            .layer(Extension(state.clone()))
            .layer(cors.clone()))
        .nest("/match-game", create_match_game_router()
            .with_state(game_state)
            .layer(Extension(state.clone()))
            .layer(cors.clone()))
        .nest("/2048", create_2048_game_router()
            .with_state(game_2048_state)
            .layer(Extension(state.clone()))
            .layer(cors.clone()))
        .nest("/wheel", create_wheel_game_router()
            .with_state(state.clone())
            .layer(cors.clone()))
        .nest("/word-game", create_word_game_router()
            .with_state(word_game_state)
            .layer(Extension(state.clone()))
            .layer(cors.clone()))
        .nest("/hexort", create_hexort_game_router()
            .with_state(state.clone())
            .layer(cors.clone()))
        .route("/", axum::routing::get(serve_frontend_index))
        .fallback(serve_frontend_index)
        .with_state(state.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("listening on {}", addr);
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn csrf_token_middleware(
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response<Body>, StatusCode> {
    if request.method() == Method::GET || request.method() == Method::HEAD || request.method() == Method::OPTIONS {
        return Ok(next.run(request).await);
    }

    if request.uri().path().starts_with("/api/auth") {
        return Ok(next.run(request).await);
    }

    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let mut response = next.run(request).await;

    let mut cookie = Cookie::new("csrf_token", token.clone());
    cookie.set_secure(true);
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(cookie::SameSite::Strict);

    response.headers_mut().insert(
        "Set-Cookie",
        cookie.to_string().parse().unwrap()
    );

    Ok(response)
}

/// Check and update expired memberships
async fn check_expired_memberships(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Find users with expired memberships
    let now = time::OffsetDateTime::now_utc();
    
    // Update users with expired memberships
    sqlx::query!(
        "UPDATE users SET is_member = false, member_until = NULL 
         WHERE is_member = true AND member_until < $1",
        now,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Clean up expired membership codes
async fn cleanup_expired_membership_codes(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Find and delete expired membership codes
    let result = sqlx::query!(
        "DELETE FROM membership_codes WHERE expires_at < $1",
        time::OffsetDateTime::now_utc(),
    )
    .execute(pool)
    .await?;

    tracing::info!("Cleaned up {} expired membership codes", result.rows_affected());

    Ok(())
}