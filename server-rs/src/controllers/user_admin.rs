use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{error::AppError, models::User, AppState};

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct UserSearchQuery {
    id: Option<String>,
    with_deleted: Option<bool>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_users).post(create_user))
        .route("/:id", get(get_user).put(update_user).delete(delete_user))
        .route("/:id/preferences", get(get_user_preferences).put(update_user_preferences))
        .route("/:id/restore", post(restore_user))
        .route("/:id/sessions", get(get_user_sessions))
        .route("/:id/statistics", get(get_user_statistics))
}

async fn get_users(
    State(state): State<AppState>,
    Query(query): Query<UserSearchQuery>,
) -> Result<Json<Vec<Value>>, AppError> {
    let users = sqlx::query_as::<_, User>(
        r#"
        SELECT
            "id"::text as "id",
            "name",
            "email",
            "avatarColor",
            "profileImagePath",
            "profileChangedAt",
            "storageLabel",
            "shouldChangePassword",
            "isAdmin",
            "createdAt",
            "updatedAt",
            "deletedAt",
            "oauthId",
            "quotaSizeInBytes",
            "quotaUsageInBytes",
            "status",
            "password",
            "pinCode"
        FROM "user"
        WHERE ($1::uuid IS NULL OR id = $1::uuid)
          AND ($2::bool = true OR "deletedAt" IS NULL)
        ORDER BY "createdAt" DESC
        "#,
    )
    .bind(query.id)
    .bind(query.with_deleted.unwrap_or(false))
    .fetch_all(&state.db)
    .await?;

    let mut response = Vec::new();
    for user in users {
        response.push(map_user_admin(&state, user).await?);
    }
    Ok(Json(response))
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let email = payload.get("email").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("email is required".to_string()))?;
    let name = payload.get("name").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("name is required".to_string()))?;
    let password = payload.get("password").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("password is required".to_string()))?;
    let password_hash = bcrypt::hash(password, 12).map_err(|e| AppError::InternalServerError(e.into()))?;
    let is_admin = payload.get("isAdmin").and_then(|v| v.as_bool()).unwrap_or(false);
    let should_change_password = payload.get("shouldChangePassword").and_then(|v| v.as_bool()).unwrap_or(false);
    let quota_size = payload.get("quotaSizeInBytes").and_then(|v| v.as_i64());
    let storage_label = payload.get("storageLabel").and_then(|v| v.as_str());
    let avatar_color = payload.get("avatarColor").and_then(|v| v.as_str()).unwrap_or("primary");
    let status = "active";

    sqlx::query(
        r#"
        INSERT INTO "user" (
            id, email, name, password, "isAdmin", "shouldChangePassword", "quotaSizeInBytes",
            "quotaUsageInBytes", "storageLabel", "avatarColor", status, "profileImagePath", "oauthId", "createdAt", "updatedAt"
        ) VALUES (
            $1::uuid, $2, $3, $4, $5, $6, $7,
            0, $8, $9, $10, '', '', $11, $12
        )
        "#,
    )
    .bind(&id)
    .bind(email)
    .bind(name)
    .bind(password_hash)
    .bind(is_admin)
    .bind(should_change_password)
    .bind(quota_size)
    .bind(storage_label)
    .bind(avatar_color)
    .bind(status)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    let user = load_user(&state, &id, true).await?;
    Ok(Json(map_user_admin(&state, user).await?))
}

async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let user = load_user(&state, &id, true).await?;
    Ok(Json(map_user_admin(&state, user).await?))
}

async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let existing = load_user(&state, &id, true).await?;
    let email = payload.get("email").and_then(|v| v.as_str()).unwrap_or(&existing.email);
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or(&existing.name);
    let is_admin = payload.get("isAdmin").and_then(|v| v.as_bool()).unwrap_or(existing.is_admin);
    let should_change_password = payload.get("shouldChangePassword").and_then(|v| v.as_bool()).unwrap_or(existing.should_change_password);
    let quota_size = payload.get("quotaSizeInBytes").and_then(|v| v.as_i64()).or(existing.quota_size_in_bytes);
    let storage_label = payload.get("storageLabel").and_then(|v| v.as_str()).or(existing.storage_label.as_deref());
    let avatar_color = payload.get("avatarColor").and_then(|v| v.as_str()).or(existing.avatar_color.as_deref()).unwrap_or("primary");
    let password_hash = if let Some(password) = payload.get("password").and_then(|v| v.as_str()) {
        Some(bcrypt::hash(password, 12).map_err(|e| AppError::InternalServerError(e.into()))?)
    } else {
        existing.password
    };
    let pin_code = if payload.get("pinCode").is_some() {
        payload.get("pinCode").and_then(|v| v.as_str()).map(str::to_string)
    } else {
        existing.pin_code
    };

    sqlx::query(
        r#"
        UPDATE "user"
        SET email = $1,
            name = $2,
            "isAdmin" = $3,
            "shouldChangePassword" = $4,
            "quotaSizeInBytes" = $5,
            "storageLabel" = $6,
            "avatarColor" = $7,
            password = $8,
            "pinCode" = $9,
            "updatedAt" = NOW()
        WHERE id = $10::uuid
        "#,
    )
    .bind(email)
    .bind(name)
    .bind(is_admin)
    .bind(should_change_password)
    .bind(quota_size)
    .bind(storage_label)
    .bind(avatar_color)
    .bind(password_hash)
    .bind(pin_code)
    .bind(&id)
    .execute(&state.db)
    .await?;

    let user = load_user(&state, &id, true).await?;
    Ok(Json(map_user_admin(&state, user).await?))
}

async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let force = payload.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    if force {
        sqlx::query(r#"DELETE FROM "user" WHERE id = $1::uuid"#)
            .bind(&id)
            .execute(&state.db)
            .await?;
        return Ok(Json(json!({"id": id})));
    }

    sqlx::query(r#"UPDATE "user" SET "deletedAt" = NOW(), status = 'deleted', "updatedAt" = NOW() WHERE id = $1::uuid"#)
        .bind(&id)
        .execute(&state.db)
        .await?;
    let user = load_user(&state, &id, true).await?;
    Ok(Json(map_user_admin(&state, user).await?))
}

async fn restore_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    sqlx::query(r#"UPDATE "user" SET "deletedAt" = NULL, status = 'active', "updatedAt" = NOW() WHERE id = $1::uuid"#)
        .bind(&id)
        .execute(&state.db)
        .await?;
    let user = load_user(&state, &id, true).await?;
    Ok(Json(map_user_admin(&state, user).await?))
}

async fn get_user_preferences(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let value = sqlx::query_scalar::<_, Value>(
        r#"SELECT value FROM user_metadata WHERE "userId" = $1::uuid AND key = 'preferences'"#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(value.unwrap_or_else(|| json!({}))))
}

async fn update_user_preferences(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    sqlx::query(
        r#"
        INSERT INTO user_metadata ("userId", key, value)
        VALUES ($1::uuid, 'preferences', $2)
        ON CONFLICT ("userId", key)
        DO UPDATE SET value = EXCLUDED.value
        "#,
    )
    .bind(&id)
    .bind(&payload)
    .execute(&state.db)
    .await?;
    Ok(Json(payload))
}

async fn get_user_sessions(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query_as::<_, crate::controllers::session::SessionRow>(
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
        ORDER BY "updatedAt" DESC, "createdAt" DESC
        "#,
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(|row| json!({
        "id": row.id,
        "deviceOS": row.device_os,
        "deviceType": row.device_type,
        "createdAt": row.created_at.to_rfc3339(),
        "updatedAt": row.updated_at.to_rfc3339(),
        "expiresAt": row.expires_at.map(|v| v.to_rfc3339()),
        "isPendingSyncReset": row.is_pending_sync_reset,
        "appVersion": row.app_version,
        "current": false
    })).collect()))
}

async fn get_user_statistics(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let total = count_assets(&state, &id, None).await?;
    let images = count_assets(&state, &id, Some("IMAGE")).await?;
    let videos = count_assets(&state, &id, Some("VIDEO")).await?;
    Ok(Json(json!({ "total": total, "images": images, "videos": videos })))
}

async fn load_user(state: &AppState, id: &str, with_deleted: bool) -> Result<User, AppError> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT
            "id"::text as "id",
            "name",
            "email",
            "avatarColor",
            "profileImagePath",
            "profileChangedAt",
            "storageLabel",
            "shouldChangePassword",
            "isAdmin",
            "createdAt",
            "updatedAt",
            "deletedAt",
            "oauthId",
            "quotaSizeInBytes",
            "quotaUsageInBytes",
            "status",
            "password",
            "pinCode"
        FROM "user"
        WHERE id = $1::uuid
          AND ($2::bool = true OR "deletedAt" IS NULL)
        "#,
    )
    .bind(id)
    .bind(with_deleted)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("User not found".to_string()))
}

async fn map_user_admin(state: &AppState, user: User) -> Result<Value, AppError> {
    let license = sqlx::query_scalar::<_, Value>(
        r#"SELECT value FROM user_metadata WHERE "userId" = $1::uuid AND key = 'license'"#,
    )
    .bind(&user.id)
    .fetch_optional(&state.db)
    .await?;
    Ok(json!({
        "id": user.id,
        "email": user.email,
        "name": user.name,
        "avatarColor": user.avatar_color.unwrap_or_else(|| "primary".to_string()),
        "profileImagePath": user.profile_image_path,
        "profileChangedAt": user.profile_changed_at.map(|v| v.to_rfc3339()),
        "storageLabel": user.storage_label,
        "shouldChangePassword": user.should_change_password,
        "isAdmin": user.is_admin,
        "createdAt": user.created_at.to_rfc3339(),
        "updatedAt": user.updated_at.to_rfc3339(),
        "deletedAt": user.deleted_at.map(|v| v.to_rfc3339()),
        "oauthId": user.oauth_id,
        "quotaSizeInBytes": user.quota_size_in_bytes,
        "quotaUsageInBytes": user.quota_usage_in_bytes,
        "status": user.status,
        "license": license
    }))
}

async fn count_assets(state: &AppState, user_id: &str, asset_type: Option<&str>) -> Result<i64, AppError> {
    let count = if let Some(asset_type) = asset_type {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM "asset" WHERE "ownerId" = $1::uuid AND "deletedAt" IS NULL AND "type" = $2"#,
        )
        .bind(user_id)
        .bind(asset_type)
        .fetch_one(&state.db)
        .await?
    } else {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM "asset" WHERE "ownerId" = $1::uuid AND "deletedAt" IS NULL"#,
        )
        .bind(user_id)
        .fetch_one(&state.db)
        .await?
    };
    Ok(count)
}
