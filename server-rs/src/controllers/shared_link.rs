use axum::{
    extract::{Json as JsonBody, Path, Query, State},
    http::StatusCode,
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

#[derive(Debug, sqlx::FromRow)]
struct SharedLinkRow {
    id: String,
    description: Option<String>,
    #[sqlx(rename = "userId")]
    user_id: String,
    key: Vec<u8>,
    r#type: String,
    #[sqlx(rename = "createdAt")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "expiresAt")]
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(rename = "allowUpload")]
    allow_upload: bool,
    #[sqlx(rename = "albumId")]
    album_id: Option<String>,
    #[sqlx(rename = "allowDownload")]
    allow_download: bool,
    #[sqlx(rename = "showExif")]
    show_exif: bool,
    password: Option<String>,
    slug: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SharedLinkSearchDto {
    id: Option<String>,
    album_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SharedLinkLoginDto {
    id: String,
    password: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_shared_links).post(create_shared_link))
        .route("/login", post(shared_link_login))
        .route("/me", get(get_my_shared_link))
        .route("/:id", get(get_shared_link).patch(update_shared_link).delete(delete_shared_link))
        .route("/:id/assets", put(add_shared_link_assets).delete(remove_shared_link_assets))
}

async fn list_shared_links(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<SharedLinkSearchDto>,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query_as::<_, SharedLinkRow>(
        r#"
        SELECT
            id::text as id,
            description,
            "userId"::text as "userId",
            key,
            type,
            "createdAt",
            "expiresAt",
            "allowUpload",
            "albumId"::text as "albumId",
            "allowDownload",
            "showExif",
            password,
            slug
        FROM shared_link
        WHERE "userId" = $1::uuid
          AND ($2::uuid IS NULL OR id = $2::uuid)
          AND ($3::uuid IS NULL OR "albumId" = $3::uuid)
        ORDER BY "createdAt" DESC
        "#,
    )
    .bind(&auth.user.id)
    .bind(query.id)
    .bind(query.album_id)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(shared_link_json(&state, row, None).await?);
    }
    Ok(Json(out))
}

async fn create_shared_link(
    State(state): State<AppState>,
    auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Value>, AppError> {
    let id = Uuid::new_v4().to_string();
    let key = Uuid::new_v4().as_bytes().to_vec();
    let link_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("INDIVIDUAL").to_string();
    let description = payload.get("description").and_then(|v| v.as_str()).map(str::to_string);
    let expires_at = payload
        .get("expiresAt")
        .and_then(|v| v.as_str())
        .map(parse_rfc3339)
        .transpose()?;
    let allow_upload = payload.get("allowUpload").and_then(|v| v.as_bool()).unwrap_or(true);
    let allow_download = payload.get("allowDownload").and_then(|v| v.as_bool()).unwrap_or(true);
    let show_exif = payload.get("showMetadata").and_then(|v| v.as_bool()).unwrap_or(true);
    let password = payload.get("password").and_then(|v| v.as_str()).map(str::to_string);
    let slug = payload.get("slug").and_then(|v| v.as_str()).map(str::to_string);
    let album_id = payload.get("albumId").and_then(|v| v.as_str()).map(str::to_string);

    sqlx::query(
        r#"
        INSERT INTO shared_link
            (id, description, "userId", key, type, "expiresAt", "allowUpload", "albumId", "allowDownload", "showExif", password, slug)
        VALUES
            ($1::uuid, $2, $3::uuid, $4, $5, $6, $7, $8::uuid, $9, $10, $11, $12)
        "#,
    )
    .bind(&id)
    .bind(description.clone())
    .bind(&auth.user.id)
    .bind(&key)
    .bind(&link_type)
    .bind(expires_at)
    .bind(allow_upload)
    .bind(album_id.clone())
    .bind(allow_download)
    .bind(show_exif)
    .bind(password.clone())
    .bind(slug.clone())
    .execute(&state.db)
    .await?;

    if let Some(asset_ids) = payload.get("assetIds").and_then(|v| v.as_array()) {
        for asset_id in asset_ids.iter().filter_map(|v| v.as_str()) {
            let _ = sqlx::query(
                r#"INSERT INTO shared_link_asset ("assetId", "sharedLinkId") VALUES ($1::uuid, $2::uuid) ON CONFLICT DO NOTHING"#,
            )
            .bind(asset_id)
            .bind(&id)
            .execute(&state.db)
            .await?;
        }
    }

    let row = load_shared_link(&state, &id).await?;
    Ok(Json(shared_link_json(&state, row, None).await?))
}

async fn get_shared_link(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let row = load_shared_link(&state, &id).await?;
    if row.user_id != auth.user.id {
        return Err(AppError::BadRequest("Shared link not found".to_string()));
    }
    Ok(Json(shared_link_json(&state, row, None).await?))
}

async fn update_shared_link(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Value>, AppError> {
    let existing = load_shared_link(&state, &id).await?;
    if existing.user_id != auth.user.id {
        return Err(AppError::BadRequest("Shared link not found".to_string()));
    }

    let expires_at = if payload.get("changeExpiryTime").and_then(|v| v.as_bool()).unwrap_or(false)
        && payload.get("expiresAt").is_none()
    {
        None
    } else {
        payload
            .get("expiresAt")
            .and_then(|v| v.as_str())
            .map(parse_rfc3339)
            .transpose()?
            .or(existing.expires_at)
    };

    sqlx::query(
        r#"
        UPDATE shared_link
        SET description = COALESCE($1, description),
            password = COALESCE($2, password),
            slug = COALESCE($3, slug),
            "allowUpload" = COALESCE($4, "allowUpload"),
            "allowDownload" = COALESCE($5, "allowDownload"),
            "showExif" = COALESCE($6, "showExif"),
            "expiresAt" = $7
        WHERE id = $8::uuid
        "#,
    )
    .bind(payload.get("description").and_then(|v| v.as_str()).map(str::to_string))
    .bind(payload.get("password").and_then(|v| v.as_str()).map(str::to_string))
    .bind(payload.get("slug").and_then(|v| v.as_str()).map(str::to_string))
    .bind(payload.get("allowUpload").and_then(|v| v.as_bool()))
    .bind(payload.get("allowDownload").and_then(|v| v.as_bool()))
    .bind(payload.get("showMetadata").and_then(|v| v.as_bool()))
    .bind(expires_at)
    .bind(&id)
    .execute(&state.db)
    .await?;

    let row = load_shared_link(&state, &id).await?;
    Ok(Json(shared_link_json(&state, row, None).await?))
}

async fn delete_shared_link(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let row = load_shared_link(&state, &id).await?;
    if row.user_id != auth.user.id {
        return Err(AppError::BadRequest("Shared link not found".to_string()));
    }
    sqlx::query(r#"DELETE FROM shared_link WHERE id = $1::uuid"#)
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_shared_link_assets(
    State(state): State<AppState>,
    _auth: AuthDto,
    Path(id): Path<String>,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Vec<Value>>, AppError> {
    let asset_ids: Vec<String> = payload
        .get("assetIds")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let mut results = Vec::with_capacity(asset_ids.len());
    for asset_id in asset_ids {
        let inserted = sqlx::query(
            r#"INSERT INTO shared_link_asset ("assetId", "sharedLinkId") VALUES ($1::uuid, $2::uuid) ON CONFLICT DO NOTHING"#,
        )
        .bind(&asset_id)
        .bind(&id)
        .execute(&state.db)
        .await?
        .rows_affected();
        results.push(json!({"assetId": asset_id, "success": inserted > 0, "error": if inserted > 0 { Value::Null } else { json!("duplicate") }}));
    }
    Ok(Json(results))
}

async fn remove_shared_link_assets(
    State(state): State<AppState>,
    _auth: AuthDto,
    Path(id): Path<String>,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Vec<Value>>, AppError> {
    let asset_ids: Vec<String> = payload
        .get("assetIds")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let mut results = Vec::with_capacity(asset_ids.len());
    for asset_id in asset_ids {
        let deleted = sqlx::query(
            r#"DELETE FROM shared_link_asset WHERE "sharedLinkId" = $1::uuid AND "assetId" = $2::uuid"#,
        )
        .bind(&id)
        .bind(&asset_id)
        .execute(&state.db)
        .await?
        .rows_affected();
        results.push(json!({"assetId": asset_id, "success": deleted > 0, "error": if deleted > 0 { Value::Null } else { json!("not_found") }}));
    }
    Ok(Json(results))
}

async fn shared_link_login(
    State(state): State<AppState>,
    JsonBody(dto): JsonBody<SharedLinkLoginDto>,
) -> Result<Json<Value>, AppError> {
    let row = load_shared_link(&state, &dto.id).await?;
    let password = row.password.clone().ok_or_else(|| AppError::BadRequest("Shared link is not password protected".to_string()))?;
    if password != dto.password {
        return Err(AppError::BadRequest("Invalid password".to_string()));
    }
    Ok(Json(shared_link_json(&state, row, Some("shared-link-token".to_string())).await?))
}

async fn get_my_shared_link() -> Result<Json<Value>, AppError> {
    Ok(Json(json!({})))
}

async fn load_shared_link(state: &AppState, id: &str) -> Result<SharedLinkRow, AppError> {
    sqlx::query_as::<_, SharedLinkRow>(
        r#"
        SELECT
            id::text as id,
            description,
            "userId"::text as "userId",
            key,
            type,
            "createdAt",
            "expiresAt",
            "allowUpload",
            "albumId"::text as "albumId",
            "allowDownload",
            "showExif",
            password,
            slug
        FROM shared_link
        WHERE id = $1::uuid
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Shared link not found".to_string()))
}

async fn shared_link_json(state: &AppState, row: SharedLinkRow, token: Option<String>) -> Result<Value, AppError> {
    let assets = sqlx::query_scalar::<_, String>(
        r#"
        SELECT a.id::text as id
        FROM shared_link_asset sla
        JOIN asset a ON a.id = sla."assetId"
        WHERE sla."sharedLinkId" = $1::uuid
          AND a."deletedAt" IS NULL
        ORDER BY a."fileCreatedAt" ASC
        "#,
    )
    .bind(&row.id)
    .fetch_all(&state.db)
    .await?;

    Ok(json!({
        "id": row.id,
        "description": row.description,
        "userId": row.user_id,
        "key": URL_SAFE_NO_PAD.encode(row.key),
        "type": row.r#type,
        "createdAt": row.created_at.to_rfc3339(),
        "expiresAt": row.expires_at.map(|v| v.to_rfc3339()),
        "allowUpload": row.allow_upload,
        "allowDownload": row.allow_download,
        "showMetadata": row.show_exif,
        "password": row.password,
        "slug": row.slug,
        "token": token,
        "album": row.album_id.map(|id| json!({"id": id})),
        "assets": assets.into_iter().map(|id| json!({"id": id})).collect::<Vec<_>>(),
    }))
}

fn parse_rfc3339(value: &str) -> Result<chrono::DateTime<chrono::Utc>, AppError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|v| v.with_timezone(&chrono::Utc))
        .map_err(|_| AppError::BadRequest("Invalid date".to_string()))
}
