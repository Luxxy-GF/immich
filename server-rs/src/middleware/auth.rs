use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts},
};
use std::sync::Arc;

use crate::{
    crypto::hash_sha256,
    error::AppError,
    models::{Session, User},
    AppState,
};

pub struct AuthDto {
    pub user: User,
    pub session: Option<Session>,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthDto
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        let mut token = None;

        // Check x-immich-user-token or x-immich-session-token
        if let Some(h) = parts.headers.get("x-immich-user-token") {
            token = h.to_str().ok().map(String::from);
        }
        if token.is_none() {
            if let Some(h) = parts.headers.get("x-immich-session-token") {
                token = h.to_str().ok().map(String::from);
            }
        }

        // Check Bearer token
        if token.is_none() {
            if let Some(auth_header) = parts.headers.get(header::AUTHORIZATION) {
                if let Ok(auth_str) = auth_header.to_str() {
                    if auth_str.to_lowercase().starts_with("bearer ") {
                        token = Some(auth_str[7..].to_string());
                    }
                }
            }
        }

        // Check Cookies
        if token.is_none() {
            if let Some(cookie_header) = parts.headers.get(header::COOKIE) {
                if let Ok(cookie_str) = cookie_header.to_str() {
                    for cookie in cookie_str.split(';').map(|s| s.trim()) {
                        if let Some(val) = cookie.strip_prefix("immich_access_token=") {
                            token = Some(val.to_string());
                            break;
                        }
                    }
                }
            }
        }

        let token = token.ok_or_else(|| AppError::BadRequest("Authentication required".to_string()))?;

        // Hash token and lookup
        let hashed_token = hash_sha256(&token);

        let session: Session = sqlx::query_as::<_, Session>(
            r#"
            SELECT 
                "id"::text as "id", "createdAt", "updatedAt", "expiresAt", "deviceOS", "deviceType", "appVersion", "pinExpiresAt", "isPendingSyncReset", "userId"::text as "userId", "token"
            FROM "session" 
            WHERE "token" = $1
            "#
        )
        .bind(hashed_token)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::BadRequest("Invalid user token".to_string()))?;

        // Fetch User
        let user: User = sqlx::query_as::<_, User>(
            r#"
            SELECT 
                "id"::text as "id", "name", "email", "avatarColor", "profileImagePath", "profileChangedAt", "storageLabel", "shouldChangePassword", "isAdmin", "createdAt", "updatedAt", "deletedAt", "oauthId", "quotaSizeInBytes", "quotaUsageInBytes", "status", "password", "pinCode"
            FROM "user" 
            WHERE "id" = $1 AND "deletedAt" IS NULL
            "#
        )
        .bind(&session.user_id.parse::<uuid::Uuid>().map_err(|_| AppError::BadRequest("id map error".to_string()))?)
        .fetch_optional(&app_state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .ok_or_else(|| AppError::BadRequest("User not found".to_string()))?;

        Ok(AuthDto {
            user,
            session: Some(session),
        })
    }
}
