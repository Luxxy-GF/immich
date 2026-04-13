use axum::{extract::{Path, State}, http::StatusCode, routing::{delete, get, post, put}, Json, Router};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use sha2::Digest;
use uuid::Uuid;

use crate::{crypto::random_bytes_as_text, error::AppError, middleware::auth::AuthDto, AppState};

#[derive(Debug, sqlx::FromRow)]
struct ApiKeyRow {
    id: String,
    name: String,
    permissions: Vec<String>,
    #[sqlx(rename = "createdAt")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "updatedAt")]
    updated_at: chrono::DateTime<chrono::Utc>,
    key: Vec<u8>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_api_keys).post(create_api_key))
        .route("/me", get(get_my_api_key))
        .route("/:id", get(get_api_key).put(update_api_key).delete(delete_api_key))
}

async fn create_api_key(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let id = Uuid::new_v4().to_string();
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("API Key");
    let permissions: Vec<String> = payload
        .get("permissions")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let secret = random_bytes_as_text(32);
    let key = sha2::Sha256::digest(secret.as_bytes()).to_vec();

    sqlx::query(
        r#"
        INSERT INTO api_key (id, name, key, "userId", permissions)
        VALUES ($1::uuid, $2, $3, $4::uuid, $5)
        "#,
    )
    .bind(&id)
    .bind(name)
    .bind(key)
    .bind(&auth.user.id)
    .bind(&permissions)
    .execute(&state.db)
    .await?;

    let row = load_api_key(&state, &id, &auth.user.id).await?;
    Ok(Json(json!({
        "apiKey": api_key_json(row),
        "secret": secret
    })))
}

async fn get_api_keys(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows: Vec<ApiKeyRow> = sqlx::query_as::<_, ApiKeyRow>(
        r#"
        SELECT id::text as id, name, permissions, "createdAt", "updatedAt", key
        FROM api_key
        WHERE "userId" = $1::uuid
        ORDER BY "createdAt" DESC
        "#,
    )
    .bind(&auth.user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(api_key_json).collect()))
}

async fn get_my_api_key(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<Value>, AppError> {
    let rows: Vec<ApiKeyRow> = sqlx::query_as::<_, ApiKeyRow>(
        r#"
        SELECT id::text as id, name, permissions, "createdAt", "updatedAt", key
        FROM api_key
        WHERE "userId" = $1::uuid
        ORDER BY "createdAt" DESC
        LIMIT 1
        "#,
    )
    .bind(&auth.user.id)
    .fetch_all(&state.db)
    .await?;
    let row = rows.into_iter().next().ok_or_else(|| AppError::BadRequest("API key not found".to_string()))?;
    Ok(Json(api_key_json(row)))
}

async fn get_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
) -> Result<Json<Value>, AppError> {
    let row = load_api_key(&state, &id, &auth.user.id).await?;
    Ok(Json(api_key_json(row)))
}

async fn update_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let existing = load_api_key(&state, &id, &auth.user.id).await?;
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or(&existing.name);
    let permissions: Vec<String> = payload
        .get("permissions")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or(existing.permissions.clone());
    sqlx::query(
        r#"UPDATE api_key SET name = $1, permissions = $2, "updatedAt" = NOW() WHERE id = $3::uuid AND "userId" = $4::uuid"#,
    )
    .bind(name)
    .bind(permissions)
    .bind(&id)
    .bind(&auth.user.id)
    .execute(&state.db)
    .await?;
    let row = load_api_key(&state, &id, &auth.user.id).await?;
    Ok(Json(api_key_json(row)))
}

async fn delete_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
) -> Result<StatusCode, AppError> {
    sqlx::query(r#"DELETE FROM api_key WHERE id = $1::uuid AND "userId" = $2::uuid"#)
        .bind(&id)
        .bind(&auth.user.id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load_api_key(state: &AppState, id: &str, user_id: &str) -> Result<ApiKeyRow, AppError> {
    sqlx::query_as::<_, ApiKeyRow>(
        r#"
        SELECT id::text as id, name, permissions, "createdAt", "updatedAt", key
        FROM api_key
        WHERE id = $1::uuid AND "userId" = $2::uuid
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("API key not found".to_string()))
}

fn api_key_json(row: ApiKeyRow) -> Value {
    json!({
        "id": row.id,
        "name": row.name,
        "permissions": row.permissions,
        "createdAt": row.created_at.to_rfc3339(),
        "updatedAt": row.updated_at.to_rfc3339(),
        "_debugKeyHash": STANDARD.encode(row.key),
    })
}
