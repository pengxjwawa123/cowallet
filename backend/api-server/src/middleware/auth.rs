use axum::{
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub jti: String, // token ID (for blacklisting)
    pub device_id: String,
    pub exp: usize,
    pub iat: usize,
}

impl Claims {
    pub fn new(user_id: &str, device_id: &str, ttl_secs: u64) -> Self {
        let now = chrono::Utc::now().timestamp() as usize;
        Self {
            sub: user_id.to_string(),
            jti: Uuid::new_v4().to_string(),
            device_id: device_id.to_string(),
            iat: now,
            exp: now + ttl_secs as usize,
        }
    }

    /// Create refresh token claims (longer TTL)
    pub fn new_refresh(user_id: &str, device_id: &str) -> Self {
        Self::new(user_id, device_id, 86400 * 7) // 7 days
    }
}

/// Response for successful authentication with token pair
#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: usize,
    pub token_type: &'static str,
}

fn jwt_secret() -> Vec<u8> {
    std::env::var("JWT_SECRET")
        .expect("JWT_SECRET environment variable must be set")
        .into_bytes()
}

/// Issue a pair of access token (24h) and refresh token (7d)
pub fn issue_token_pair(user_id: &str, device_id: &str) -> Result<TokenPair, jsonwebtoken::errors::Error> {
    let access_claims = Claims::new(user_id, device_id, 86400); // 24h
    let refresh_claims = Claims::new_refresh(user_id, device_id);

    let access_token = encode(
        &Header::default(),
        &access_claims,
        &EncodingKey::from_secret(&jwt_secret()),
    )?;

    let refresh_token = encode(
        &Header::default(),
        &refresh_claims,
        &EncodingKey::from_secret(&jwt_secret()),
    )?;

    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: 86400,
        token_type: "Bearer",
    })
}

/// Verify a JWT token without checking blacklist (for refresh flow)
pub fn verify_token_unchecked(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(&jwt_secret()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

/// Check if a token is blacklisted in the database
pub async fn is_token_blacklisted(db: &PgPool, jti: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM jwt_blacklist WHERE token_id = $1)"
    )
    .bind(Uuid::parse_str(jti).unwrap_or(Uuid::nil()))
    .fetch_one(db)
    .await?;

    Ok(result)
}

/// Add a token to the blacklist (logout/revocation)
pub async fn blacklist_token(
    db: &PgPool,
    jti: &str,
    user_id: &str,
    exp: usize,
    reason: Option<String>,
) -> Result<(), sqlx::Error> {
    let jti_uuid = Uuid::parse_str(jti).unwrap_or_else(|_| Uuid::nil());
    let user_uuid = Uuid::parse_str(user_id).unwrap_or(Uuid::nil());
    let exp_time = chrono::NaiveDateTime::from_timestamp_opt(exp as i64, 0)
        .unwrap_or_else(|| chrono::Utc::now().naive_utc())
        .and_utc();

    sqlx::query(
        "INSERT INTO jwt_blacklist (token_id, user_id, expires_at, reason)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (token_id) DO NOTHING"
    )
    .bind(jti_uuid)
    .bind(user_uuid)
    .bind(exp_time)
    .bind(reason)
    .execute(db)
    .await?;

    Ok(())
}

/// Refresh access token using a valid refresh token
pub async fn refresh_access_token(
    db: &PgPool,
    refresh_token: &str,
    device_id: &str,
) -> Result<TokenPair, StatusCode> {
    // Verify refresh token signature
    let claims = verify_token_unchecked(refresh_token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Check if refresh token is blacklisted
    if is_token_blacklisted(db, &claims.jti).await.unwrap_or(false) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Verify device binding matches
    if claims.device_id != device_id {
        tracing::warn!("Token refresh attempted from different device: expected {}, got {}",
            claims.device_id, device_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Blacklist the old refresh token (one-time use)
    let _ = blacklist_token(
        db,
        &claims.jti,
        &claims.sub,
        claims.exp,
        Some("Token refresh".to_string()),
    ).await;

    // Issue new token pair
    issue_token_pair(&claims.sub, device_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Auth middleware that:
/// 1. Extracts and verifies JWT signature
/// 2. Checks if token is blacklisted (requires DB state)
/// 3. Validates device binding
pub async fn require_auth(mut req: Request, next: Next) -> Result<Response, StatusCode> {
    // Extract AppState first for DB access
    let state = req.extensions()
        .get::<crate::state::AppState>()
        .cloned()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = verify_token_unchecked(token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Check blacklist if DB is available
    if let Some(db) = &state.db {
        if is_token_blacklisted(db, &claims.jti).await.unwrap_or(false) {
            tracing::warn!("Rejected blacklisted token for user {}", claims.sub);
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    // Verify device ID from header matches token claim
    if let Some(device_header) = req.headers().get("X-Device-ID")
        .and_then(|v| v.to_str().ok())
    {
        if device_header != claims.device_id {
            tracing::warn!("Device mismatch for user {}: token={}, header={}",
                claims.sub, claims.device_id, device_header);
            return Err(StatusCode::FORBIDDEN);
        }
    }

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}
