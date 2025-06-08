use axum::{
    extract::{State, Extension, Json},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, error};
use time::OffsetDateTime;
use reqwest::Client;

use crate::auth::middleware::UserId;
use crate::AppState;
use crate::patreon_handler::PatreonConfig;

// Duration for membership in days (30 days by default)
pub const MEMBERSHIP_DURATION_DAYS: i64 = 30;

#[derive(Deserialize)]
pub struct PatreonLinkRequest {
    pub email: String,
}

#[derive(Deserialize)]
pub struct PatreonOAuthCallbackRequest {
    pub code: String,
}

#[derive(Serialize)]
pub struct PatreonLinkResponse {
    pub success: bool,
    pub message: String,
    pub is_linked: bool,
    pub is_member: bool,
    pub patron_status: Option<String>,
    pub member_until: Option<String>,
}

#[derive(Serialize)]
pub struct PatreonStatusResponse {
    pub is_linked: bool,
    pub patreon_email: Option<String>,
    pub patron_status: Option<String>,
    pub is_member: bool,
    pub member_until: Option<String>,
}

#[derive(Serialize)]
pub struct PatreonOAuthUrlResponse {
    pub url: String,
}

#[derive(Deserialize, Debug)]
struct PatreonTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    scope: String,
    token_type: String,
}

#[derive(Deserialize)]
struct PatreonIdentityResponse {
    data: PatreonIdentityData,
}

#[derive(Deserialize)]
struct PatreonIdentityData {
    id: String,
    attributes: PatreonIdentityAttributes,
}

#[derive(Deserialize)]
struct PatreonIdentityAttributes {
    email: String,
    full_name: String,
}

/// Generate OAuth URL for Patreon authorization
pub async fn get_oauth_url(
    State(_state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let patreon_config = match PatreonConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load Patreon config: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Get the redirect URI from environment or use a default
    // Make sure this URI is registered in your Patreon developer application
    let redirect_uri = std::env::var("PATREON_REDIRECT_URI")
        .unwrap_or_else(|_| {
            // Default to production URL
            "https://frtl.dev/settings".to_string()
        });
    
    // Generate the OAuth URL
    let oauth_url = format!(
        "https://www.patreon.com/oauth2/authorize?response_type=code&client_id={}&redirect_uri={}&scope=identity",
        patreon_config.client_id,
        redirect_uri
    );
    
    Ok(Json(PatreonOAuthUrlResponse { url: oauth_url }))
}

/// Handle OAuth callback from Patreon
pub async fn handle_oauth_callback(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(payload): Json<PatreonOAuthCallbackRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Load Patreon config first
    let patreon_config = match PatreonConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load Patreon config: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Get the redirect URI from environment or use a default
    let redirect_uri = std::env::var("PATREON_REDIRECT_URI")
        .unwrap_or_else(|_| "https://frtl.dev/settings".to_string());

    // Get user email for logging
    let user = sqlx::query!(
        "SELECT email, username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error fetching user: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("User {} ({}) attempting to link Patreon account via OAuth", 
          user.username, user.email);

    // Check if user already has a linked Patreon account
    let existing_link = sqlx::query!(
        r#"
        SELECT ps.email, ps.patron_status
        FROM user_patreon_links upl
        JOIN patreon_supporters ps ON upl.patreon_id = ps.patreon_id
        WHERE upl.user_id = $1
        "#,
        user_id.0
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error checking existing link: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(link) = existing_link {
        return Ok(Json(PatreonLinkResponse {
            success: false,
            message: format!("Your account is already linked to Patreon account with email: {}", link.email),
            is_linked: true,
            is_member: false,
            patron_status: link.patron_status,
            member_until: None,
        }));
    }

    // Get user identity from Patreon
    let client = Client::new();
    let token_response = match client
        .post("https://www.patreon.com/api/oauth2/token")
        .form(&[
            ("code", payload.code.as_str()),
            ("grant_type", "authorization_code"),
            ("client_id", &patreon_config.client_id),
            ("client_secret", &patreon_config.client_secret),
            ("redirect_uri", &redirect_uri),
        ])
        .send()
        .await
    {
        Ok(response) => {
            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_default();
                error!("Failed to exchange code for token: {}", error_text);
                return Ok(Json(PatreonLinkResponse {
                    success: false,
                    message: "Failed to authenticate with Patreon. Please try again.".to_string(),
                    is_linked: false,
                    is_member: false,
                    patron_status: None,
                    member_until: None,
                }));
            }
            
            match response.json::<PatreonTokenResponse>().await {
                Ok(token) => token,
                Err(e) => {
                    error!("Failed to parse token response: {:?}", e);
                    return Ok(Json(PatreonLinkResponse {
                        success: false,
                        message: "Failed to process Patreon authentication. Please try again.".to_string(),
                        is_linked: false,
                        is_member: false,
                        patron_status: None,
                        member_until: None,
                    }));
                }
            }
        },
        Err(e) => {
            error!("Failed to send token request: {:?}", e);
            return Ok(Json(PatreonLinkResponse {
                success: false,
                message: "Failed to connect to Patreon. Please try again.".to_string(),
                is_linked: false,
                is_member: false,
                patron_status: None,
                member_until: None,
            }));
        }
    };

    // Get user identity from Patreon
    let identity_response = match client
        .get("https://www.patreon.com/api/oauth2/v2/identity")
        .header("Authorization", format!("Bearer {}", token_response.access_token))
        .query(&[("fields[user]", "email,full_name")])
        .send()
        .await
    {
        Ok(response) => {
            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_default();
                error!("Failed to get user identity: {}", error_text);
                return Ok(Json(PatreonLinkResponse {
                    success: false,
                    message: "Failed to retrieve your Patreon account information. Please try again.".to_string(),
                    is_linked: false,
                    is_member: false,
                    patron_status: None,
                    member_until: None,
                }));
            }
            
            match response.json::<PatreonIdentityResponse>().await {
                Ok(identity) => identity,
                Err(e) => {
                    error!("Failed to parse identity response: {:?}", e);
                    return Ok(Json(PatreonLinkResponse {
                        success: false,
                        message: "Failed to process Patreon account information. Please try again.".to_string(),
                        is_linked: false,
                        is_member: false,
                        patron_status: None,
                        member_until: None,
                    }));
                }
            }
        },
        Err(e) => {
            error!("Failed to send identity request: {:?}", e);
            return Ok(Json(PatreonLinkResponse {
                success: false,
                message: "Failed to retrieve Patreon account information. Please try again.".to_string(),
                is_linked: false,
                is_member: false,
                patron_status: None,
                member_until: None,
            }));
        }
    };

    // Check if this Patreon account is already linked to another user
    let existing_link = sqlx::query!(
        r#"
        SELECT u.username
        FROM user_patreon_links upl
        JOIN users u ON upl.user_id = u.id
        WHERE upl.patreon_id = $1
        "#,
        identity_response.data.id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error checking existing Patreon link: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(link) = existing_link {
        return Ok(Json(PatreonLinkResponse {
            success: false,
            message: format!("This Patreon account is already linked to user: {}", link.username),
            is_linked: false,
            is_member: false,
            patron_status: None,
            member_until: None,
        }));
    }

    // Get member status from Patreon
    let patron_status = match client
        .get("https://www.patreon.com/api/oauth2/v2/identity/memberships")
        .header("Authorization", format!("Bearer {}", token_response.access_token))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                Some("active_patron".to_string())
            } else {
                error!("Failed to get member status: error code: {}", response.status());
                None
            }
        },
        Err(e) => {
            error!("Failed to get member status: {:?}", e);
            None
        }
    };

    let is_active_patron = patron_status.as_deref() == Some("active_patron");

    // Calculate token expiration
    let expires_at = OffsetDateTime::now_utc() + time::Duration::seconds(token_response.expires_in as i64);

    // Start a transaction
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Failed to start transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // First, create or update the Patreon supporter record
    let unknown_status = "unknown".to_string();
    let patron_status_ref = patron_status.as_ref().unwrap_or(&unknown_status);
    
    sqlx::query!(
        r#"
        INSERT INTO patreon_supporters (
            patreon_id, full_name, email, 
            campaign_lifetime_support_cents, currently_entitled_amount_cents,
            patron_status
        )
        VALUES ($1, $2, $3, 0, 0, $4)
        ON CONFLICT (patreon_id) DO UPDATE
        SET full_name = $2,
            email = $3,
            patron_status = $4,
            updated_at = CURRENT_TIMESTAMP
        "#,
        identity_response.data.id,
        identity_response.data.attributes.full_name,
        identity_response.data.attributes.email,
        patron_status_ref
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to upsert patreon_supporter: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Then, store the token information
    sqlx::query(
        r#"
        INSERT INTO patreon_tokens 
        (patreon_id, access_token, refresh_token, expires_at, scope, token_type)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (patreon_id) DO UPDATE SET
            access_token = EXCLUDED.access_token,
            refresh_token = EXCLUDED.refresh_token,
            expires_at = EXCLUDED.expires_at,
            scope = EXCLUDED.scope,
            token_type = EXCLUDED.token_type,
            updated_at = CURRENT_TIMESTAMP
        "#
    )
    .bind(&identity_response.data.id)
    .bind(&token_response.access_token)
    .bind(&token_response.refresh_token)
    .bind(expires_at)
    .bind(&token_response.scope)
    .bind(&token_response.token_type)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to store token information: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create the link between user and Patreon account
    sqlx::query!(
        r#"
        INSERT INTO user_patreon_links (user_id, patreon_id)
        VALUES ($1, $2)
        "#,
        user_id.0,
        identity_response.data.id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to insert user_patreon_link: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Update membership status based on patron status
    let member_until = if is_active_patron {
        let member_until = OffsetDateTime::now_utc() + time::Duration::days(MEMBERSHIP_DURATION_DAYS);
        
        sqlx::query!(
            r#"
            UPDATE users
            SET is_member = true, member_until = $2, membership_source = 'patreon'
            WHERE id = $1
            "#,
            user_id.0,
            member_until
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to update user membership: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        Some(member_until.format(&time::format_description::well_known::Rfc3339).unwrap_or_default())
    } else {
        None
    };

    // Commit the transaction
    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(PatreonLinkResponse {
        success: true,
        message: format!("Successfully linked Patreon account for {}", identity_response.data.attributes.email),
        is_linked: true,
        is_member: is_active_patron,
        patron_status,
        member_until,
    }))
}

/// Link a user account to a Patreon account by email
pub async fn link_patreon_account(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
    Json(payload): Json<PatreonLinkRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Get user email for logging
    let user = sqlx::query!(
        "SELECT email, username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error fetching user: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("User {} ({}) attempting to link Patreon account with email: {}", 
          user.username, user.email, payload.email);

    // Check if user already has a linked Patreon account
    let existing_link = sqlx::query!(
        r#"
        SELECT ps.email, ps.patron_status
        FROM user_patreon_links upl
        JOIN patreon_supporters ps ON upl.patreon_id = ps.patreon_id
        WHERE upl.user_id = $1
        "#,
        user_id.0
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error checking existing link: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(link) = existing_link {
        return Ok(Json(PatreonLinkResponse {
            success: false,
            message: format!("Your account is already linked to Patreon account with email: {}", link.email),
            is_linked: true,
            is_member: false, // Will be updated below
            patron_status: link.patron_status,
            member_until: None, // Will be updated below
        }));
    }

    // Find Patreon supporter by email
    let patreon_supporter = sqlx::query!(
        r#"
        SELECT patreon_id, full_name, patron_status, email
        FROM patreon_supporters
        WHERE LOWER(email) = LOWER($1)
        "#,
        payload.email
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error finding Patreon supporter: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let patreon_supporter = match patreon_supporter {
        Some(supporter) => supporter,
        None => {
            return Ok(Json(PatreonLinkResponse {
                success: false,
                message: format!("No Patreon account found with email: {}. Please make sure you're using the same email address as your Patreon account.", payload.email),
                is_linked: false,
                is_member: false,
                patron_status: None,
                member_until: None,
            }));
        }
    };

    // Check if this Patreon account is already linked to another user
    let existing_link = sqlx::query!(
        r#"
        SELECT u.username
        FROM user_patreon_links upl
        JOIN users u ON upl.user_id = u.id
        WHERE upl.patreon_id = $1
        "#,
        patreon_supporter.patreon_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error checking existing Patreon link: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(link) = existing_link {
        return Ok(Json(PatreonLinkResponse {
            success: false,
            message: format!("This Patreon account is already linked to user: {}", link.username),
            is_linked: false,
            is_member: false,
            patron_status: None,
            member_until: None,
        }));
    }

    // Start a transaction
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Failed to start transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create the link
    sqlx::query!(
        r#"
        INSERT INTO user_patreon_links (user_id, patreon_id)
        VALUES ($1, $2)
        "#,
        user_id.0,
        patreon_supporter.patreon_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to insert user_patreon_link: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Check if the Patreon account is active
    let is_active_patron = patreon_supporter.patron_status.as_deref() == Some("active_patron");
    
    // If active, update user membership status
    let member_until = if is_active_patron {
        let member_until = OffsetDateTime::now_utc() + time::Duration::days(MEMBERSHIP_DURATION_DAYS);
        
        sqlx::query!(
            r#"
            UPDATE users
            SET is_member = true, member_until = $2, membership_source = 'patreon'
            WHERE id = $1
            "#,
            user_id.0,
            member_until
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to update user membership: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        Some(member_until.format(&time::format_description::well_known::Rfc3339).unwrap_or_default())
    } else {
        None
    };

    // Commit the transaction
    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("Successfully linked user {} to Patreon account {}", user.username, patreon_supporter.full_name);

    Ok(Json(PatreonLinkResponse {
        success: true,
        message: format!("Successfully linked to Patreon account for: {}", patreon_supporter.full_name),
        is_linked: true,
        is_member: is_active_patron,
        patron_status: patreon_supporter.patron_status,
        member_until,
    }))
}

/// Unlink a user account from a Patreon account
pub async fn unlink_patreon_account(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<impl IntoResponse, StatusCode> {
    // Get user info for logging
    let user = sqlx::query!(
        "SELECT username FROM users WHERE id = $1",
        user_id.0
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error fetching user: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Check if user has a linked Patreon account
    let existing_link = sqlx::query!(
        r#"
        SELECT ps.full_name
        FROM user_patreon_links upl
        JOIN patreon_supporters ps ON upl.patreon_id = ps.patreon_id
        WHERE upl.user_id = $1
        "#,
        user_id.0
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Database error checking existing link: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if existing_link.is_none() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "message": "Your account is not linked to any Patreon account"
        })));
    }

    // Start a transaction
    let mut tx = state.pool.begin().await.map_err(|e| {
        error!("Failed to start transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Delete the link
    sqlx::query!(
        r#"
        DELETE FROM user_patreon_links
        WHERE user_id = $1
        "#,
        user_id.0
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!("Failed to delete user_patreon_link: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Update user membership status if it was from Patreon
    // We don't immediately revoke membership, but we could if that's the desired behavior
    
    // Commit the transaction
    tx.commit().await.map_err(|e| {
        error!("Failed to commit transaction: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("Successfully unlinked user {} from Patreon account", user.username);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Successfully unlinked your account from Patreon"
    })))
}

/// Get the current Patreon link status for a user
pub async fn get_patreon_status(
    State(state): State<AppState>,
    Extension(user_id): Extension<UserId>,
) -> Result<impl IntoResponse, StatusCode> {
    // Get user's Patreon link status
    let link_status = sqlx::query!(
        r#"
        SELECT 
            ps.email as patreon_email,
            ps.patron_status,
            u.is_member,
            u.member_until
        FROM user_patreon_links upl
        JOIN patreon_supporters ps ON upl.patreon_id = ps.patreon_id
        JOIN users u ON upl.user_id = u.id
        WHERE upl.user_id = $1
        "#,
        user_id.0
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch Patreon link status: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let is_linked = link_status.is_some();
    
    let response = PatreonStatusResponse {
        is_linked,
        patreon_email: link_status.as_ref().map(|ls| ls.patreon_email.clone()),
        patron_status: link_status.as_ref().and_then(|ls| ls.patron_status.clone()),
        is_member: link_status.as_ref().map(|ls| ls.is_member).unwrap_or(false),
        member_until: link_status.as_ref()
            .and_then(|ls| ls.member_until)
            .map(|date| date.format(&time::format_description::well_known::Rfc3339).unwrap_or_default()),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Sync all Patreon-linked users' membership status
pub async fn sync_patreon_memberships(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Start a transaction
    let mut tx = pool.begin().await?;
    
    // Get all linked users and their current patron status
    let linked_users = sqlx::query!(
        r#"
        SELECT 
            u.id as user_id,
            u.username,
            ps.patron_status,
            ps.patreon_id,
            u.is_member,
            u.member_until
        FROM users u
        JOIN user_patreon_links upl ON u.id = upl.user_id
        JOIN patreon_supporters ps ON upl.patreon_id = ps.patreon_id
        "#
    )
    .fetch_all(&mut *tx)
    .await?;
    
    let now = time::OffsetDateTime::now_utc();
    
    for user in linked_users {
        let is_active = user.patron_status.as_deref() == Some("active_patron");
        
        match (is_active, user.is_member) {
            (true, true) => {
                // If active patron and already a member, check if we need to extend
                if let Some(member_until) = user.member_until {
                    // If membership expires in less than 15 days, extend it
                    if member_until <= (now + time::Duration::days(15)) {
                        let new_until = now + time::Duration::days(MEMBERSHIP_DURATION_DAYS);
                        sqlx::query!(
                            r#"
                            UPDATE users
                            SET member_until = $2, membership_source = 'patreon'
                            WHERE id = $1
                            "#,
                            user.user_id,
                            new_until
                        )
                        .execute(&mut *tx)
                        .await?;
                    }
                }
            },
            (true, false) => {
                // If active patron but not a member, grant membership
                let member_until = now + time::Duration::days(MEMBERSHIP_DURATION_DAYS);
                sqlx::query!(
                    r#"
                    UPDATE users
                    SET is_member = true, member_until = $2, membership_source = 'patreon'
                    WHERE id = $1
                    "#,
                    user.user_id,
                    member_until
                )
                .execute(&mut *tx)
                .await?;
            },
            (false, true) => {
                // If not an active patron but still a member, only revoke membership if it came from Patreon
                sqlx::query!(
                    r#"
                    UPDATE users
                    SET is_member = false, member_until = NULL
                    WHERE id = $1 AND (membership_source = 'patreon' OR membership_source IS NULL)
                    "#,
                    user.user_id
                )
                .execute(&mut *tx)
                .await?;
            },
            (false, false) => {
                // Both false, nothing to do
            }
        }
    }
    
    // Commit all changes
    tx.commit().await?;
    
    Ok(())
}

/// Update webhook handler to also update linked user memberships
pub async fn update_linked_user_membership(
    pool: &PgPool, 
    patreon_id: &str, 
    is_active: bool
) -> Result<(), sqlx::Error> {
    // Find linked user(s)
    let linked_users = sqlx::query!(
        r#"
        SELECT user_id
        FROM user_patreon_links
        WHERE patreon_id = $1
        "#,
        patreon_id
    )
    .fetch_all(pool)
    .await?;
    
    if linked_users.is_empty() {
        return Ok(());
    }
    
    for user in linked_users {
        if is_active {
            // Set or extend membership
            let member_until = OffsetDateTime::now_utc() + time::Duration::days(MEMBERSHIP_DURATION_DAYS);
            
            sqlx::query!(
                r#"
                UPDATE users
                SET is_member = true, member_until = $2, membership_source = 'patreon'
                WHERE id = $1
                "#,
                user.user_id,
                member_until
            )
            .execute(pool)
            .await?;
        } else {
            // Only revoke membership if it came from Patreon
            sqlx::query!(
                r#"
                UPDATE users
                SET is_member = false, member_until = NULL
                WHERE id = $1 AND (membership_source = 'patreon' OR membership_source IS NULL)
                "#,
                user.user_id
            )
            .execute(pool)
            .await?;
        }
    }
    
    Ok(())
}

/// Refresh expired Patreon tokens
pub async fn refresh_expired_tokens(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Find tokens that expire in the next hour
    let expired_tokens = sqlx::query!(
        r#"
        SELECT patreon_id, refresh_token
        FROM patreon_tokens
        WHERE expires_at <= NOW() + INTERVAL '1 hour'
        "#
    )
    .fetch_all(pool)
    .await?;
    
    if expired_tokens.is_empty() {
        return Ok(());
    }
    
    let patreon_config = match PatreonConfig::from_env() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load Patreon config: {:?}", e);
            return Ok(());
        }
    };
    
    let client = Client::new();
    
    for token in expired_tokens {
        match client
            .post("https://www.patreon.com/api/oauth2/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &token.refresh_token),
                ("client_id", &patreon_config.client_id),
                ("client_secret", &patreon_config.client_secret),
            ])
            .send()
            .await {
                Ok(response) => {
                    match response.json::<PatreonTokenResponse>().await {
                        Ok(new_token) => {
                            // Update token in database
                            match sqlx::query!(
                                r#"
                                UPDATE patreon_tokens
                                SET access_token = $2,
                                    refresh_token = $3,
                                    expires_at = $4,
                                    scope = $5,
                                    token_type = $6,
                                    updated_at = CURRENT_TIMESTAMP
                                WHERE patreon_id = $1
                                "#,
                                token.patreon_id,
                                new_token.access_token,
                                new_token.refresh_token,
                                OffsetDateTime::now_utc() + time::Duration::seconds(new_token.expires_in as i64),
                                new_token.scope,
                                new_token.token_type
                            )
                            .execute(pool)
                            .await {
                                Ok(_) => {},
                                Err(e) => error!("Failed to update token in database for {}: {:?}", token.patreon_id, e)
                            }
                        },
                        Err(e) => error!("Failed to parse token response for {}: {:?}", token.patreon_id, e)
                    }
                },
                Err(e) => error!("Failed to refresh token for {}: {:?}", token.patreon_id, e)
            }
    }
    
    Ok(())
} 