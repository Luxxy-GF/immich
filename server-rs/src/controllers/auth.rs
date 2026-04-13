use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::Utc;
use serde_json::json;

use crate::{
    crypto::{compare_bcrypt, hash_sha256, random_bytes_as_text},
    dtos::auth::{AuthStatusResponseDto, LoginCredentialDto, LoginResponseDto, LogoutResponseDto, ValidateAccessTokenResponseDto},
    error::AppError,
    models::User,
    middleware::auth::AuthDto,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/admin-sign-up", post(admin_sign_up))
        .route("/validateToken", post(validate_token))
        .route("/logout", post(logout))
        .route("/status", get(status))
        .route("/change-password", post(change_password))
        .route("/pin-code", post(set_pin_code).put(change_pin_code).delete(reset_pin_code))
        .route("/session/unlock", post(unlock_session))
        .route("/session/lock", post(lock_session))
        .route("/oauth/config", axum::routing::get(oauth_config))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthConfigResponseDto {
    pub enabled: bool,
    pub button_text: String,
    pub auto_launch: bool,
}

async fn oauth_config() -> Result<Json<OAuthConfigResponseDto>, AppError> {
    Ok(Json(OAuthConfigResponseDto {
        enabled: false,
        button_text: "Login with OAuth".to_string(),
        auto_launch: false,
    }))
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginCredentialDto>,
) -> Result<(HeaderMap, Json<LoginResponseDto>), AppError> {
    // Check if password login is disabled (assuming true for now since config is mock)
    // We fetch user from db based on email
    let user: Option<User> = sqlx::query_as::<_, User>(
        r#"
        SELECT 
            "id"::text as "id", "name", "email", "avatarColor", "profileImagePath", "profileChangedAt", "storageLabel", "shouldChangePassword", "isAdmin", "createdAt", "updatedAt", "deletedAt", "oauthId", "quotaSizeInBytes", "quotaUsageInBytes", "status", "password", "pinCode"
        FROM "user" 
        WHERE "email" = $1 AND "deletedAt" IS NULL
        "#
    )
    .bind(&payload.email)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    let valid_user = match user {
        Some(u) => {
            if let Some(ref pw) = u.password {
                if compare_bcrypt(&payload.password, pw) {
                    Some(u)
                } else {
                    None
                }
            } else {
                None
            }
        }
        None => None,
    };

    let user = valid_user.ok_or_else(|| {
        AppError::BadRequest("Incorrect email or password".to_string())
    })?;

    // Create session (as in TS: token = cryptoRepository.randomBytesAsText(32))
    let token_raw = random_bytes_as_text(32);
    let hashed_token = hash_sha256(&token_raw);
    let session_id = uuid::Uuid::new_v4().to_string();

    let user_agent = headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("Unknown");
    
    // Simplistic device parsing matching default behavior (Immich parses User-Agent fully)
    let device_os = "Unknown".to_string(); // In a real rewrite we use ua_parser
    let device_type = "Web".to_string();

    sqlx::query(
        r#"
        INSERT INTO "session" (
            "id", "createdAt", "updatedAt", "deviceOS", "deviceType", "appVersion", "userId", "token"
        ) VALUES (
            $1::uuid, $2, $3, $4, $5, $6, $7::uuid, $8
        )
        "#
    )
    .bind(session_id)
    .bind(Utc::now())
    .bind(Utc::now())
    .bind(device_os)
    .bind(device_type)
    .bind(None::<String>) // appVersion
    .bind(&user.id)
    .bind(hashed_token)
    .execute(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    let mut resp_headers = HeaderMap::new();
    if let Ok(c) = format!("immich_access_token={}; Path=/; HttpOnly; SameSite=Lax", token_raw).parse() {
        resp_headers.append(axum::http::header::SET_COOKIE, c);
    }
    if let Ok(c) = "immich_auth_type=password; Path=/; SameSite=Lax".parse() {
        resp_headers.append(axum::http::header::SET_COOKIE, c);
    }
    if let Ok(c) = "immich_is_authenticated=true; Path=/; SameSite=Lax".parse() {
        resp_headers.append(axum::http::header::SET_COOKIE, c);
    }

    Ok((resp_headers, Json(LoginResponseDto {
        access_token: token_raw,
        user_id: user.id.clone(),
        user_email: user.email.clone(),
        name: user.name.clone(),
        profile_image_path: user.profile_image_path.clone(),
        is_admin: user.is_admin,
        should_change_password: user.should_change_password,
        is_onboarded: user_onboarding(&state, &user.id).await.unwrap_or(false),
    })))
}

async fn admin_sign_up(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let admin_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM "user" WHERE "isAdmin" = true AND "deletedAt" IS NULL"#,
    )
    .fetch_one(&state.db)
    .await?;
    if admin_count > 0 {
        return Err(AppError::BadRequest("The server already has an admin".to_string()));
    }

    let email = payload.get("email").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("email is required".to_string()))?;
    let name = payload.get("name").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("name is required".to_string()))?;
    let password = payload.get("password").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("password is required".to_string()))?;
    let id = uuid::Uuid::new_v4().to_string();
    let password_hash = bcrypt::hash(password, 12).map_err(|e| AppError::InternalServerError(e.into()))?;

    sqlx::query(
        r#"
        INSERT INTO "user" (
            id, email, name, password, "isAdmin", "shouldChangePassword",
            "quotaUsageInBytes", "storageLabel", "avatarColor", status, "profileImagePath", "oauthId"
        ) VALUES (
            $1::uuid, $2, $3, $4, true, false,
            0, 'admin', 'primary', 'active', '', ''
        )
        "#,
    )
    .bind(&id)
    .bind(email)
    .bind(name)
    .bind(password_hash)
    .execute(&state.db)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO user_metadata ("userId", key, value)
        VALUES ($1::uuid, 'onboarding', '{"isOnboarded": true}')
        ON CONFLICT ("userId", key)
        DO UPDATE SET value = EXCLUDED.value
        "#,
    )
    .bind(&id)
    .execute(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": id,
        "email": email,
        "name": name,
        "profileImagePath": "",
        "isAdmin": true,
        "status": "active",
        "storageLabel": "admin",
        "quotaSizeInBytes": null,
        "quotaUsageInBytes": 0,
        "shouldChangePassword": false,
        "avatarColor": "primary",
        "oauthId": "",
        "license": null,
        "createdAt": chrono::Utc::now().to_rfc3339(),
        "updatedAt": chrono::Utc::now().to_rfc3339(),
        "deletedAt": null,
        "profileChangedAt": null
    })))
}

async fn validate_token(_auth: AuthDto) -> Result<Json<ValidateAccessTokenResponseDto>, AppError> {
    Ok(Json(ValidateAccessTokenResponseDto { auth_status: true }))
}

async fn logout(_auth: AuthDto) -> Result<(HeaderMap, Json<LogoutResponseDto>), AppError> {
    let mut headers = HeaderMap::new();
    for cookie in [
        "immich_access_token=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax",
        "immich_auth_type=; Path=/; Max-Age=0; SameSite=Lax",
        "immich_is_authenticated=; Path=/; Max-Age=0; SameSite=Lax",
    ] {
        if let Ok(value) = cookie.parse() {
            headers.append(header::SET_COOKIE, value);
        }
    }

    Ok((headers, Json(LogoutResponseDto {
        successful: true,
        redirect_uri: "/auth/login".to_string(),
    })))
}

async fn status(auth: AuthDto) -> Result<Json<AuthStatusResponseDto>, AppError> {
    Ok(Json(AuthStatusResponseDto {
        pin_code: auth.user.pin_code.is_some(),
        password: auth.user.password.is_some(),
        is_elevated: false,
        expires_at: auth.session.as_ref().and_then(|session| session.expires_at.map(|value| value.to_rfc3339())),
        pin_expires_at: auth.session.as_ref().and_then(|session| session.pin_expires_at.map(|value| value.to_rfc3339())),
    }))
}

async fn change_password() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NOT_IMPLEMENTED)
}

async fn set_pin_code() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn change_pin_code() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn reset_pin_code() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn unlock_session() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn lock_session() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn user_onboarding(state: &AppState, user_id: &str) -> Result<bool, AppError> {
    let value = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT value FROM user_metadata WHERE "userId" = $1::uuid AND key = 'onboarding'"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(value
        .as_ref()
        .and_then(|v| v.get("isOnboarded"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}
