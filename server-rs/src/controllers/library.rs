use axum::{extract::{Path, State}, http::StatusCode, routing::{get, post, put}, Json, Router};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{error::AppError, AppState};

#[derive(Debug, sqlx::FromRow)]
struct LibraryRow {
    id: String,
    name: String,
    #[sqlx(rename = "ownerId")]
    owner_id: String,
    #[sqlx(rename = "importPaths")]
    import_paths: Vec<String>,
    #[sqlx(rename = "exclusionPatterns")]
    exclusion_patterns: Vec<String>,
    #[sqlx(rename = "createdAt")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "updatedAt")]
    updated_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "refreshedAt")]
    refreshed_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_libraries).post(create_library))
        .route("/:id", get(get_library).put(update_library).delete(delete_library))
        .route("/:id/validate", post(validate_library))
        .route("/:id/statistics", get(get_library_statistics))
        .route("/:id/scan", post(scan_library))
}

async fn get_libraries(State(state): State<AppState>) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query_as::<_, LibraryRow>(
        r#"
        SELECT
            id::text as id,
            name,
            "ownerId"::text as "ownerId",
            "importPaths",
            "exclusionPatterns",
            "createdAt",
            "updatedAt",
            "refreshedAt"
        FROM library
        WHERE "deletedAt" IS NULL
        ORDER BY "createdAt" DESC
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::new();
    for row in rows {
        let asset_count: i64 = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM asset WHERE "libraryId" = $1::uuid AND "deletedAt" IS NULL"#,
        )
        .bind(&row.id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
        out.push(map_library(row, asset_count as i32));
    }
    Ok(Json(out))
}

async fn create_library(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let id = Uuid::new_v4().to_string();
    let owner_id = payload.get("ownerId").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("ownerId is required".to_string()))?;
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("Library");
    let import_paths = payload.get("importPaths").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let exclusion_patterns = payload.get("exclusionPatterns").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let import_paths: Vec<String> = import_paths.into_iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
    let exclusion_patterns: Vec<String> = exclusion_patterns.into_iter().filter_map(|v| v.as_str().map(str::to_string)).collect();

    sqlx::query(
        r#"
        INSERT INTO library (id, name, "ownerId", "importPaths", "exclusionPatterns")
        VALUES ($1::uuid, $2, $3::uuid, $4, $5)
        "#,
    )
    .bind(&id)
    .bind(name)
    .bind(owner_id)
    .bind(import_paths)
    .bind(exclusion_patterns)
    .execute(&state.db)
    .await?;

    get_library(State(state), Path(id)).await
}

async fn get_library(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let row = load_library(&state, &id).await?;
    let asset_count: i64 = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM asset WHERE "libraryId" = $1::uuid AND "deletedAt" IS NULL"#,
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    Ok(Json(map_library(row, asset_count as i32)))
}

async fn update_library(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let existing = load_library(&state, &id).await?;
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or(&existing.name);
    let import_paths = payload.get("importPaths")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect::<Vec<_>>())
        .unwrap_or(existing.import_paths.clone());
    let exclusion_patterns = payload.get("exclusionPatterns")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect::<Vec<_>>())
        .unwrap_or(existing.exclusion_patterns.clone());

    sqlx::query(
        r#"
        UPDATE library
        SET name = $1, "importPaths" = $2, "exclusionPatterns" = $3, "updatedAt" = NOW()
        WHERE id = $4::uuid
        "#,
    )
    .bind(name)
    .bind(import_paths)
    .bind(exclusion_patterns)
    .bind(&id)
    .execute(&state.db)
    .await?;

    get_library(State(state), Path(id)).await
}

async fn delete_library(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    sqlx::query(r#"UPDATE library SET "deletedAt" = NOW(), "updatedAt" = NOW() WHERE id = $1::uuid"#)
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn validate_library(
    State(_state): State<AppState>,
    Path(_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let import_paths = payload.get("importPaths")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let results: Vec<Value> = import_paths
        .into_iter()
        .filter_map(|v| v.as_str().map(|path| {
            let starts_with_upload = path.starts_with("/root/immich/upload") || path.starts_with("/uploads");
            json!({
                "importPath": path,
                "isValid": !starts_with_upload,
                "message": if starts_with_upload { json!("Cannot use media upload folder for external libraries") } else { Value::Null }
            })
        }))
        .collect();
    Ok(Json(json!({ "importPaths": results })))
}

async fn get_library_statistics(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let total: i64 = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM asset WHERE "libraryId" = $1::uuid AND "deletedAt" IS NULL"#)
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let photos: i64 = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM asset WHERE "libraryId" = $1::uuid AND "deletedAt" IS NULL AND type = 'IMAGE'"#)
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let videos: i64 = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM asset WHERE "libraryId" = $1::uuid AND "deletedAt" IS NULL AND type = 'VIDEO'"#)
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let usage: i64 = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(ex."fileSizeInByte"), 0)
        FROM asset a
        LEFT JOIN asset_exif ex ON ex."assetId" = a.id
        WHERE a."libraryId" = $1::uuid AND a."deletedAt" IS NULL
        "#,
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    Ok(Json(json!({ "photos": photos, "videos": videos, "total": total, "usage": usage })))
}

async fn scan_library(State(state): State<AppState>, Path(id): Path<String>) -> Result<StatusCode, AppError> {
    sqlx::query(r#"UPDATE library SET "refreshedAt" = NOW(), "updatedAt" = NOW() WHERE id = $1::uuid"#)
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load_library(state: &AppState, id: &str) -> Result<LibraryRow, AppError> {
    sqlx::query_as::<_, LibraryRow>(
        r#"
        SELECT
            id::text as id,
            name,
            "ownerId"::text as "ownerId",
            "importPaths",
            "exclusionPatterns",
            "createdAt",
            "updatedAt",
            "refreshedAt"
        FROM library
        WHERE id = $1::uuid AND "deletedAt" IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Library not found".to_string()))
}

fn map_library(row: LibraryRow, asset_count: i32) -> Value {
    json!({
        "id": row.id,
        "name": row.name,
        "ownerId": row.owner_id,
        "importPaths": row.import_paths,
        "exclusionPatterns": row.exclusion_patterns,
        "createdAt": row.created_at.to_rfc3339(),
        "updatedAt": row.updated_at.to_rfc3339(),
        "refreshedAt": row.refreshed_at.map(|v| v.to_rfc3339()),
        "assetCount": asset_count,
    })
}
