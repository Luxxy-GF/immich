use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    crypto::{hash_sha256, random_bytes_as_text},
    error::AppError,
    middleware::auth::AuthDto,
    AppState,
};

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct SessionRow {
    pub(crate) id: String,
    #[sqlx(rename = "deviceOS")]
    pub(crate) device_os: String,
    #[sqlx(rename = "deviceType")]
    pub(crate) device_type: String,
    #[sqlx(rename = "createdAt")]
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "updatedAt")]
    pub(crate) updated_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "expiresAt")]
    pub(crate) expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(rename = "isPendingSyncReset")]
    pub(crate) is_pending_sync_reset: bool,
    #[sqlx(rename = "appVersion")]
    pub(crate) app_version: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SessionCreateDto {
    device_os: Option<String>,
    device_type: Option<String>,
    duration: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SessionUpdateDto {
    is_pending_sync_reset: Option<bool>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_session).get(get_sessions).delete(delete_all_sessions))
        .route("/:id", axum::routing::put(update_session).delete(delete_session))
        .route("/:id/lock", post(lock_session))
}

async fn create_session(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(dto): Json<SessionCreateDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    let parent_id = auth
        .session
        .as_ref()
        .map(|session| session.id.clone())
        .ok_or_else(|| AppError::BadRequest("This endpoint can only be used with a session token".to_string()))?;

    let id = Uuid::new_v4().to_string();
    let token = random_bytes_as_text(32);
    let hashed = hash_sha256(&token);
    let now = Utc::now();
    let expires_at = dto.duration.map(|seconds| now + Duration::seconds(seconds));

    sqlx::query(
        r#"
        INSERT INTO "session" (
            id, token, "createdAt", "updatedAt", "userId", "deviceType", "deviceOS", "expiresAt", "parentId", "isPendingSyncReset", "appVersion"
        ) VALUES (
            $1::uuid, $2, $3, $4, $5::uuid, $6, $7, $8, $9::uuid, false, NULL
        )
        "#,
    )
    .bind(&id)
    .bind(hashed)
    .bind(now)
    .bind(now)
    .bind(&auth.user.id)
    .bind(dto.device_type.unwrap_or_else(|| "Browser".to_string()))
    .bind(dto.device_os.unwrap_or_else(|| "Web".to_string()))
    .bind(expires_at)
    .bind(parent_id)
    .execute(&state.db)
    .await?;

    let row = load_session(&state, &id).await?;
    Ok(Json(map_session(row, None, Some(token))))
}

async fn get_sessions(State(state): State<AppState>, auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let current_session_id = auth.session.as_ref().map(|session| session.id.clone());
    let rows = sqlx::query_as::<_, SessionRow>(
        r#"
        SELECT
            "id"::text as id,
            "deviceOS",
            "deviceType",
            "createdAt",
            "updatedAt",
            "expiresAt",
            "isPendingSyncReset",
            "appVersion"
        FROM "session"
        WHERE "userId" = $1::uuid
          AND ("expiresAt" IS NULL OR "expiresAt" > NOW())
        ORDER BY "updatedAt" DESC, "createdAt" DESC
        "#,
    )
    .bind(&auth.user.id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|row| map_session(row, current_session_id.clone(), None))
            .collect(),
    ))
}

async fn delete_all_sessions(State(state): State<AppState>, auth: AuthDto) -> Result<StatusCode, AppError> {
    let exclude_id = auth.session.as_ref().map(|session| session.id.clone());
    sqlx::query(
        r#"
        DELETE FROM "session"
        WHERE "userId" = $1::uuid
          AND ($2::uuid IS NULL OR id != $2::uuid)
        "#,
    )
    .bind(&auth.user.id)
    .bind(exclude_id)
    .execute(&state.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_session(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
    Json(dto): Json<SessionUpdateDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    if dto.is_pending_sync_reset.is_none() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    let exists = session_belongs_to_user(&state, &auth.user.id, &id).await?;
    if !exists {
        return Err(AppError::BadRequest("Session not found".to_string()));
    }

    sqlx::query(
        r#"
        UPDATE "session"
        SET "isPendingSyncReset" = COALESCE($1, "isPendingSyncReset"), "updatedAt" = NOW()
        WHERE id = $2::uuid
        "#,
    )
    .bind(dto.is_pending_sync_reset)
    .bind(&id)
    .execute(&state.db)
    .await?;

    let row = load_session(&state, &id).await?;
    Ok(Json(map_session(row, None, None)))
}

async fn delete_session(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let exists = session_belongs_to_user(&state, &auth.user.id, &id).await?;
    if !exists {
        return Err(AppError::BadRequest("Session not found".to_string()));
    }

    sqlx::query(r#"DELETE FROM "session" WHERE id = $1::uuid"#)
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn lock_session(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let exists = session_belongs_to_user(&state, &auth.user.id, &id).await?;
    if !exists {
        return Err(AppError::BadRequest("Session not found".to_string()));
    }

    sqlx::query(r#"UPDATE "session" SET "pinExpiresAt" = NULL, "updatedAt" = NOW() WHERE id = $1::uuid"#)
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load_session(state: &AppState, id: &str) -> Result<SessionRow, AppError> {
    sqlx::query_as::<_, SessionRow>(
        r#"
        SELECT
            "id"::text as id,
            "deviceOS",
            "deviceType",
            "createdAt",
            "updatedAt",
            "expiresAt",
            "isPendingSyncReset",
            "appVersion"
        FROM "session"
        WHERE id = $1::uuid
        "#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(Into::into)
}

async fn session_belongs_to_user(state: &AppState, user_id: &str, session_id: &str) -> Result<bool, AppError> {
    let exists = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM "session" WHERE id = $1::uuid AND "userId" = $2::uuid"#,
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(exists > 0)
}

fn map_session(row: SessionRow, current_session_id: Option<String>, token: Option<String>) -> serde_json::Value {
    let mut value = json!({
        "id": row.id,
        "deviceOS": row.device_os,
        "deviceType": row.device_type,
        "createdAt": row.created_at.to_rfc3339(),
        "updatedAt": row.updated_at.to_rfc3339(),
        "expiresAt": row.expires_at.map(|v| v.to_rfc3339()),
        "isPendingSyncReset": row.is_pending_sync_reset,
        "current": current_session_id.as_deref() == Some(&row.id),
        "appVersion": row.app_version,
    });
    if let Some(token) = token {
        value["token"] = json!(token);
    }
    value
}
