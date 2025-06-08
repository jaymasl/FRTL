//! Patreon integration handler
//! 
//! This module handles all Patreon-related functionality including:
//! - Data models for Patreon supporters
//! - Database operations for supporter management
//! - Webhook handling for real-time updates

// Standard library imports

// External crate imports
use axum::{
    extract::Extension,
    http::{StatusCode, HeaderMap},
    response::IntoResponse,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{Error, PgPool};
use tracing::{error, info};
use hex;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use time::OffsetDateTime;

// Internal imports
use crate::AppState;
use crate::services::patreon_link_service::MEMBERSHIP_DURATION_DAYS;

// -----------------
// Data Models
// -----------------

/// Represents a Patreon supporter in our system
#[derive(Debug, Serialize, Deserialize)]
pub struct PatreonSupporter {
    /// Unique identifier from Patreon
    pub id: String,
    /// Supporter's full name
    pub full_name: String,
    /// Supporter's email address
    pub email: String,
    /// Total amount ever paid to the campaign (in cents)
    pub campaign_lifetime_support_cents: i32,
    /// Current pledge amount (in cents)
    pub currently_entitled_amount_cents: i32,
    /// When the last charge was attempted
    pub last_charge_date: Option<OffsetDateTime>,
    /// Status of the last charge attempt
    pub last_charge_status: Option<String>,
    /// When the next charge is scheduled
    pub next_charge_date: Option<OffsetDateTime>,
    /// Current status of the patron (active, declined, former)
    pub patron_status: Option<String>,
}

/// Represents the response from Patreon's API for campaigns
#[derive(Debug, Deserialize)]
pub struct PatreonCampaignsResponse {
    pub data: Vec<PatreonCampaign>,
}

#[derive(Debug, Deserialize)]
pub struct PatreonCampaign {
    pub id: String,
    pub attributes: PatreonCampaignAttributes,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PatreonCampaignAttributes {
    pub created_at: Option<String>,
    pub creation_name: Option<String>,
    pub is_charged_immediately: Option<bool>,
    pub is_monthly: Option<bool>,
    pub is_nsfw: Option<bool>,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub url: Option<String>,
}

/// Represents the incoming webhook data structure from Patreon
#[derive(Debug, Deserialize)]
pub struct PatreonWebhookData {
    pub data: PatreonWebhookUserData,
}

#[derive(Debug, Deserialize)]
pub struct PatreonWebhookUserData {
    pub id: String,
    pub attributes: PatreonWebhookUserAttributes,
}

#[derive(Debug, Deserialize)]
pub struct PatreonWebhookUserAttributes {
    pub full_name: String,
    pub email: String,
    pub campaign_lifetime_support_cents: Option<i32>,
    pub currently_entitled_amount_cents: Option<i32>,
    pub last_charge_status: Option<String>,
    pub patron_status: Option<String>,
}

// Add a new structure for the Patreon API response
/// Represents the response from Patreon's API for campaign members
#[derive(Debug, Deserialize)]
pub struct PatreonMembersResponse {
    pub data: Vec<PatreonMember>,
}

/// Represents an error response from Patreon's API
#[derive(Debug, Deserialize)]
pub struct PatreonErrorResponse {
    pub errors: Vec<PatreonError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PatreonError {
    pub code: Option<serde_json::Value>,
    pub code_name: Option<String>,
    pub detail: Option<String>,
    pub status: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatreonMember {
    pub id: String,
    pub attributes: PatreonMemberAttributes,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PatreonMemberAttributes {
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub patron_status: Option<String>,
}

// -----------------
// OAuth2 Configuration
// -----------------

/// Configuration for Patreon API OAuth2
#[derive(Clone)]
#[allow(dead_code)] // These fields will be used when implementing OAuth2
pub struct PatreonConfig {
    pub client_id: String,
    pub client_secret: String,
    pub access_token: String,
    pub refresh_token: String,
    pub campaign_id: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // This struct will be used when implementing token refresh
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

impl PatreonConfig {
    /// Creates a new PatreonConfig from environment variables
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self {
            client_id: std::env::var("PATREON_CLIENT_ID")?,
            client_secret: std::env::var("PATREON_CLIENT_SECRET")?,
            access_token: std::env::var("PATREON_ACCESS_TOKEN")?,
            refresh_token: std::env::var("PATREON_REFRESH_TOKEN")?,
            campaign_id: std::env::var("PATREON_CAMPAIGN_ID")?,
        })
    }

    /// Refreshes the access token using the refresh token
    #[allow(dead_code)] // This method will be used when implementing token refresh
    pub async fn refresh_token(&mut self, client: &Client) -> Result<(), reqwest::Error> {
        let response = client
            .post("https://www.patreon.com/api/oauth2/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &self.refresh_token),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
            ])
            .send()
            .await?
            .json::<TokenResponse>()
            .await?;

        self.access_token = response.access_token;
        self.refresh_token = response.refresh_token;

        info!("Successfully refreshed Patreon access token");
        Ok(())
    }
}

// -----------------
// Patreon API Client
// -----------------

/// Fetches the current user's campaigns from Patreon's API
pub async fn fetch_user_campaigns(client: &Client, config: &PatreonConfig) -> Result<Vec<PatreonCampaign>, String> {
    let url = "https://www.patreon.com/api/oauth2/v2/campaigns";

    info!("Fetching Patreon campaigns from URL: {}", url);

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", config.access_token))
        .send()
        .await
        .map_err(|e| format!("Failed to send request to Patreon API: {}", e))?;
    
    // Log the response status and body for debugging
    let status = response.status();
    let body = response.text().await
        .map_err(|e| format!("Failed to read response body: {}", e))?;
    
    info!("Patreon API campaigns response status: {}", status);
    info!("Patreon API campaigns response body: {}", body);
    
    // If the response is empty or not valid JSON, return an error
    if body.trim().is_empty() {
        error!("Empty response from Patreon API");
        return Err("Empty response from Patreon API".to_string());
    }
    
    // Check if the response is an error response
    if !status.is_success() {
        // Try to parse as an error response
        match serde_json::from_str::<PatreonErrorResponse>(&body) {
            Ok(error_response) => {
                let error_message = error_response.errors.first()
                    .and_then(|e| e.detail.clone())
                    .unwrap_or_else(|| format!("Patreon API error: {}", status));
                error!("Patreon API error: {}", error_message);
                return Err(error_message);
            },
            Err(e) => {
                error!("Failed to parse Patreon API error response: {:?}", e);
                return Err(format!("Patreon API returned error status {}: {}", status, body));
            }
        }
    }
    
    // Try to parse the response as JSON
    let campaigns_response: PatreonCampaignsResponse = serde_json::from_str(&body)
        .map_err(|e| {
            error!("Failed to parse Patreon API campaigns response: {:?}", e);
            format!("Failed to parse Patreon API campaigns response: {}", e)
        })?;

    Ok(campaigns_response.data)
}

/// Fetches the current list of supporters from Patreon's API
pub async fn fetch_current_supporters(client: &Client, config: &PatreonConfig) -> Result<Vec<PatreonSupporter>, String> {
    let url = format!(
        "https://www.patreon.com/api/oauth2/v2/campaigns/{}/members",
        config.campaign_id
    );

    info!("Fetching Patreon supporters from URL: {}", url);

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.access_token))
        .send()
        .await
        .map_err(|e| format!("Failed to send request to Patreon API: {}", e))?;
    
    // Log the response status and body for debugging
    let status = response.status();
    let body = response.text().await
        .map_err(|e| format!("Failed to read response body: {}", e))?;
    
    info!("Patreon API response status: {}", status);
    info!("Patreon API response body: {}", body);
    
    // If the response is empty or not valid JSON, return an error
    if body.trim().is_empty() {
        error!("Empty response from Patreon API");
        return Err("Empty response from Patreon API".to_string());
    }
    
    // Check if the response is an error response
    if !status.is_success() {
        // Try to parse as an error response
        match serde_json::from_str::<PatreonErrorResponse>(&body) {
            Ok(error_response) => {
                let error_message = error_response.errors.first()
                    .and_then(|e| e.detail.clone())
                    .unwrap_or_else(|| format!("Patreon API error: {}", status));
                error!("Patreon API error: {}", error_message);
                return Err(error_message);
            },
            Err(e) => {
                error!("Failed to parse Patreon API error response: {:?}", e);
                return Err(format!("Patreon API returned error status {}: {}", status, body));
            }
        }
    }
    
    // Try to parse the response as JSON
    let members_response: PatreonMembersResponse = serde_json::from_str(&body)
        .map_err(|e| {
            error!("Failed to parse Patreon API response: {:?}", e);
            format!("Failed to parse Patreon API response: {}", e)
        })?;

    // Convert the response into our PatreonSupporter format
    let supporters = members_response.data.into_iter().map(|member| {
        let attrs = member.attributes;
        PatreonSupporter {
            id: member.id,
            full_name: attrs.full_name.unwrap_or_else(|| "Unknown".to_string()),
            email: attrs.email.unwrap_or_else(|| "unknown@example.com".to_string()),
            campaign_lifetime_support_cents: 0, // Not available in this API call
            currently_entitled_amount_cents: 0, // Not available in this API call
            last_charge_date: None,
            last_charge_status: None, // Not available in this API call
            next_charge_date: None,
            patron_status: attrs.patron_status,
        }
    }).collect();

    Ok(supporters)
}

// -----------------
// Database Operations
// -----------------

/// Inserts or updates a Patreon supporter in the database
pub async fn upsert_supporter(pool: &PgPool, supporter: PatreonSupporter) -> Result<(), Error> {
    sqlx::query!(
        r#"
        INSERT INTO patreon_supporters (
            patreon_id, 
            full_name, 
            email, 
            campaign_lifetime_support_cents, 
            currently_entitled_amount_cents, 
            last_charge_date, 
            last_charge_status, 
            next_charge_date, 
            patron_status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (patreon_id)
        DO UPDATE SET 
            full_name = EXCLUDED.full_name, 
            email = EXCLUDED.email, 
            campaign_lifetime_support_cents = EXCLUDED.campaign_lifetime_support_cents, 
            currently_entitled_amount_cents = EXCLUDED.currently_entitled_amount_cents, 
            last_charge_date = EXCLUDED.last_charge_date, 
            last_charge_status = EXCLUDED.last_charge_status, 
            next_charge_date = EXCLUDED.next_charge_date, 
            patron_status = EXCLUDED.patron_status, 
            updated_at = CURRENT_TIMESTAMP
        "#,
        supporter.id,
        supporter.full_name,
        supporter.email,
        supporter.campaign_lifetime_support_cents,
        supporter.currently_entitled_amount_cents,
        supporter.last_charge_date,
        supporter.last_charge_status,
        supporter.next_charge_date,
        supporter.patron_status
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Retrieves all Patreon supporters from the database
#[allow(dead_code)] // This function will be used when implementing the admin dashboard
pub async fn get_all_supporters(pool: &PgPool) -> Result<Vec<PatreonSupporter>, Error> {
    let records = sqlx::query!(
        r#"
        SELECT 
            patreon_id, 
            full_name, 
            email, 
            campaign_lifetime_support_cents, 
            currently_entitled_amount_cents, 
            last_charge_date, 
            last_charge_status, 
            next_charge_date, 
            patron_status
        FROM patreon_supporters
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    let supporters = records.into_iter().map(|record| PatreonSupporter {
        id: record.patreon_id,
        full_name: record.full_name,
        email: record.email,
        campaign_lifetime_support_cents: record.campaign_lifetime_support_cents,
        currently_entitled_amount_cents: record.currently_entitled_amount_cents,
        last_charge_date: record.last_charge_date,
        last_charge_status: record.last_charge_status,
        next_charge_date: record.next_charge_date,
        patron_status: record.patron_status,
    }).collect();

    Ok(supporters)
}

// -----------------
// Webhook Types
// -----------------

/// Represents the type of webhook event received from Patreon
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatreonWebhookEvent {
    #[serde(rename = "members:create")]
    MemberCreate,
    #[serde(rename = "members:update")]
    MemberUpdate,
    #[serde(rename = "members:delete")]
    MemberDelete,
    #[serde(rename = "members:pledge:create")]
    MemberPledgeCreate,
    #[serde(rename = "members:pledge:update")]
    MemberPledgeUpdate,
    #[serde(rename = "members:pledge:delete")]
    MemberPledgeDelete,
}

// Remove the PatreonEventHeader implementation and replace with a function
fn parse_event_type(event_str: &str) -> Option<PatreonWebhookEvent> {
    match event_str {
        "members:create" => Some(PatreonWebhookEvent::MemberCreate),
        "members:update" => Some(PatreonWebhookEvent::MemberUpdate),
        "members:delete" => Some(PatreonWebhookEvent::MemberDelete),
        "members:pledge:create" => Some(PatreonWebhookEvent::MemberPledgeCreate),
        "members:pledge:update" => Some(PatreonWebhookEvent::MemberPledgeUpdate),
        "members:pledge:delete" => Some(PatreonWebhookEvent::MemberPledgeDelete),
        _ => None,
    }
}

/// Processes incoming webhook events from Patreon
#[axum::debug_handler]
pub async fn patreon_webhook_handler(
    Extension(state): Extension<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl axum::response::IntoResponse {
    // Get webhook secret from environment
    let webhook_secret = std::env::var("PATREON_WEBHOOK_SECRET")
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Webhook secret not configured".to_string()));

    let webhook_secret = match webhook_secret {
        Ok(s) => s,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.1),
    };

    // Verify signature if present
    if let Some(signature) = headers.get("X-Patreon-Signature") {
        let signature = signature.to_str()
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid signature header".to_string()));
        let signature = match signature {
            Ok(s) => s,
            Err(err) => return (StatusCode::BAD_REQUEST, err.1),
        };

        if !verify_webhook_signature(&body, signature, &webhook_secret) {
            return (StatusCode::UNAUTHORIZED, "Invalid webhook signature".to_string());
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing signature header".to_string());
    }

    // Get event type
    let event_type = headers
        .get("X-Patreon-Event")
        .and_then(|h| h.to_str().ok())
        .and_then(parse_event_type);

    let event_type = match event_type {
        Some(et) => et,
        None => return (StatusCode::BAD_REQUEST, "Invalid or missing event type".to_string()),
    };

    // Parse payload
    let payload: Result<PatreonWebhookData, _> = serde_json::from_slice(&body);
    let payload = match payload {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Invalid payload: {}", e)),
    };

    info!("Received Patreon webhook event: {:?} for member {}", event_type, payload.data.id);

    let result = match event_type {
        PatreonWebhookEvent::MemberCreate | PatreonWebhookEvent::MemberPledgeCreate => {
            handle_member_create(&state.pool, payload).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to handle member creation: {}", e)))
        },
        PatreonWebhookEvent::MemberUpdate | PatreonWebhookEvent::MemberPledgeUpdate => {
            handle_member_update(&state.pool, payload).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to handle member update: {}", e)))
        },
        PatreonWebhookEvent::MemberDelete | PatreonWebhookEvent::MemberPledgeDelete => {
            handle_member_delete(&state.pool, &payload.data.id).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to handle member deletion: {}", e)))
        },
    };

    match result {
        Ok(()) => (StatusCode::OK, String::new()),
        Err(err) => err,
    }
}

/// Handles member creation events
async fn handle_member_create(pool: &PgPool, payload: PatreonWebhookData) -> Result<(), Error> {
    let supporter = PatreonSupporter {
        id: payload.data.id.clone(),
        full_name: payload.data.attributes.full_name,
        email: payload.data.attributes.email,
        campaign_lifetime_support_cents: payload.data.attributes.campaign_lifetime_support_cents.unwrap_or(0),
        currently_entitled_amount_cents: payload.data.attributes.currently_entitled_amount_cents.unwrap_or(0),
        last_charge_date: None,
        last_charge_status: payload.data.attributes.last_charge_status,
        next_charge_date: None,
        patron_status: Some("active_patron".to_string()),
    };

    // Insert or update the supporter record
    upsert_supporter(pool, supporter).await?;
    
    // Update any linked user accounts
    if let Err(e) = crate::services::patreon_link_service::update_linked_user_membership(
        pool, 
        &payload.data.id, 
        true // New member is active
    ).await {
        error!("Failed to update linked user membership: {:?}", e);
    }

    Ok(())
}

/// Handles member update events
async fn handle_member_update(pool: &PgPool, payload: PatreonWebhookData) -> Result<(), Error> {
    let supporter = PatreonSupporter {
        id: payload.data.id.clone(),
        full_name: payload.data.attributes.full_name,
        email: payload.data.attributes.email,
        campaign_lifetime_support_cents: payload.data.attributes.campaign_lifetime_support_cents.unwrap_or(0),
        currently_entitled_amount_cents: payload.data.attributes.currently_entitled_amount_cents.unwrap_or(0),
        last_charge_date: None,
        last_charge_status: payload.data.attributes.last_charge_status,
        next_charge_date: None,
        patron_status: payload.data.attributes.patron_status.clone(),
    };

    // Start a transaction
    let mut tx = pool.begin().await?;

    // Insert or update the supporter record
    sqlx::query!(
        r#"
        INSERT INTO patreon_supporters (
            patreon_id, full_name, email, 
            campaign_lifetime_support_cents, currently_entitled_amount_cents,
            patron_status, last_charge_status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (patreon_id) DO UPDATE
        SET full_name = $2,
            email = $3,
            campaign_lifetime_support_cents = $4,
            currently_entitled_amount_cents = $5,
            patron_status = $6,
            last_charge_status = $7,
            updated_at = CURRENT_TIMESTAMP
        "#,
        supporter.id,
        supporter.full_name,
        supporter.email,
        supporter.campaign_lifetime_support_cents,
        supporter.currently_entitled_amount_cents,
        supporter.patron_status,
        supporter.last_charge_status
    )
    .execute(&mut *tx)
    .await?;

    // Get linked users
    let linked_users = sqlx::query!(
        r#"
        SELECT user_id
        FROM user_patreon_links
        WHERE patreon_id = $1
        "#,
        supporter.id
    )
    .fetch_all(&mut *tx)
    .await?;

    let is_active = supporter.patron_status.as_deref() == Some("active_patron");
    
    // Update membership for linked users
    for user in linked_users {
        if is_active {
            // Set or extend membership
            let member_until = time::OffsetDateTime::now_utc() + time::Duration::days(MEMBERSHIP_DURATION_DAYS);
            
            sqlx::query!(
                r#"
                UPDATE users
                SET is_member = true, member_until = $2
                WHERE id = $1
                "#,
                user.user_id,
                member_until
            )
            .execute(&mut *tx)
            .await?;
            
            info!("Extended membership for user {} linked to Patreon ID {}", user.user_id, supporter.id);
        } else {
            // Revoke membership if patron status is not active
            sqlx::query!(
                r#"
                UPDATE users
                SET is_member = false, member_until = NULL
                WHERE id = $1
                "#,
                user.user_id
            )
            .execute(&mut *tx)
            .await?;
            
            info!("Revoked membership for user {} linked to Patreon ID {} (patron_status: {:?})", 
                  user.user_id, supporter.id, supporter.patron_status);
        }
    }

    // Commit the transaction
    tx.commit().await?;
    
    Ok(())
}

/// Handles member deletion events
async fn handle_member_delete(pool: &PgPool, member_id: &str) -> Result<(), Error> {
    // Start a transaction
    let mut tx = pool.begin().await?;

    // Update patron status to former_patron
    sqlx::query!(
        r#"
        UPDATE patreon_supporters 
        SET patron_status = 'former_patron', 
            updated_at = CURRENT_TIMESTAMP 
        WHERE patreon_id = $1
        "#,
        member_id
    )
    .execute(&mut *tx)
    .await?;
    
    // Get linked users
    let linked_users = sqlx::query!(
        r#"
        SELECT user_id
        FROM user_patreon_links
        WHERE patreon_id = $1
        "#,
        member_id
    )
    .fetch_all(&mut *tx)
    .await?;

    // Revoke membership for all linked users
    for user in linked_users {
        sqlx::query!(
            r#"
            UPDATE users
            SET is_member = false, member_until = NULL
            WHERE id = $1
            "#,
            user.user_id
        )
        .execute(&mut *tx)
        .await?;
        
        info!("Revoked membership for user {} due to Patreon member deletion", user.user_id);
    }

    // Commit the transaction
    tx.commit().await?;
    
    Ok(())
}

/// Verifies the webhook signature using HMAC-SHA256
fn verify_webhook_signature(payload: &[u8], signature: &str, secret: &str) -> bool {
    type HmacSha256 = Hmac<Sha256>;
    
    // Create HMAC-SHA256 instance
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    
    // Add message to be authenticated
    mac.update(payload);
    
    // Convert the provided hex signature to bytes
    if let Ok(sig_bytes) = hex::decode(signature) {
        mac.verify_slice(&sig_bytes).is_ok()
    } else {
        false
    }
}

/// Manually fetch current supporters from Patreon's API
#[axum::debug_handler]
pub async fn fetch_supporters_handler(
    Extension(state): Extension<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Verify the internal secret
    let provided_secret = match headers.get("x-internal-secret").and_then(|value| value.to_str().ok()) {
        Some(secret) => secret,
        None => return (StatusCode::UNAUTHORIZED, "Missing internal secret").into_response(),
    };

    let internal_secret = match std::env::var("INTERNAL_CODE_GENERATION_SECRET") {
        Ok(secret) => secret,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_CODE_GENERATION_SECRET not configured").into_response(),
    };

    if provided_secret != internal_secret {
        return (StatusCode::UNAUTHORIZED, "Invalid internal secret").into_response();
    }

    // Load Patreon configuration
    let config = match PatreonConfig::from_env() {
        Ok(config) => config,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load Patreon configuration").into_response(),
    };

    // Create reqwest client
    let client = reqwest::Client::new();

    // First, fetch the user's campaigns to get the correct campaign ID
    match fetch_user_campaigns(&client, &config).await {
        Ok(campaigns) => {
            if campaigns.is_empty() {
                return (StatusCode::NOT_FOUND, "No campaigns found for the current user").into_response();
            }

            // Log the found campaigns
            for (i, campaign) in campaigns.iter().enumerate() {
                info!("Campaign {}: ID={}", i + 1, campaign.id);
                
                // Log all available attributes
                if let Some(ref created_at) = campaign.attributes.created_at {
                    info!("  Created At: {}", created_at);
                }
                if let Some(ref creation_name) = campaign.attributes.creation_name {
                    info!("  Creation Name: {}", creation_name);
                }
                if let Some(is_charged_immediately) = campaign.attributes.is_charged_immediately {
                    info!("  Is Charged Immediately: {}", is_charged_immediately);
                }
                if let Some(is_monthly) = campaign.attributes.is_monthly {
                    info!("  Is Monthly: {}", is_monthly);
                }
                if let Some(is_nsfw) = campaign.attributes.is_nsfw {
                    info!("  Is NSFW: {}", is_nsfw);
                }
                if let Some(ref name) = campaign.attributes.name {
                    info!("  Name: {}", name);
                }
                if let Some(ref summary) = campaign.attributes.summary {
                    info!("  Summary: {}", summary);
                }
                if let Some(ref url) = campaign.attributes.url {
                    info!("  URL: {}", url);
                }
            }

            // Use the first campaign ID for fetching supporters
            let campaign_id = campaigns[0].id.clone();
            info!("Using campaign ID: {} for fetching supporters", campaign_id);

            // Create a new config with the correct campaign ID
            let mut updated_config = config.clone();
            updated_config.campaign_id = campaign_id;

            // Fetch current supporters from Patreon API
            match fetch_current_supporters(&client, &updated_config).await {
                Ok(supporters) => {
                    // Log the number of supporters found
                    info!("Found {} supporters", supporters.len());
                    
                    // Log details of each supporter
                    for (i, supporter) in supporters.iter().enumerate() {
                        info!("Supporter {}: ID={}, Name={}, Email={}", 
                            i + 1, 
                            supporter.id, 
                            supporter.full_name, 
                            supporter.email
                        );
                        if let Some(ref status) = supporter.patron_status {
                            info!("  Status: {}", status);
                        }
                    }
                    
                    // Update supporters in database
                    for supporter in supporters {
                        if let Err(e) = upsert_supporter(&state.pool, supporter).await {
                            error!("Failed to upsert supporter: {:?}", e);
                        }
                    }
                    (StatusCode::OK, "Successfully fetched and updated Patreon supporters").into_response()
                }
                Err(e) => {
                    error!("Failed to fetch supporters: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch supporters from Patreon API: {}", e)).into_response()
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch campaigns: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch campaigns from Patreon API: {}", e)).into_response()
        }
    }
} 