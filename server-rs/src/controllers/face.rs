use axum::{
    extract::{Json as JsonBody, Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct FaceQuery {
    id: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_face).get(get_faces))
        .route("/:id", put(reassign_face).delete(delete_face))
}

async fn create_face(
    State(state): State<AppState>,
    auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Value>, AppError> {
    let id = Uuid::new_v4().to_string();
    let asset_id = payload.get("assetId").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("assetId is required".to_string()))?;
    let person_id = payload.get("personId").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("personId is required".to_string()))?;
    let image_width = payload.get("imageWidth").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let image_height = payload.get("imageHeight").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let x = payload.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let y = payload.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let width = payload.get("width").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let height = payload.get("height").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    sqlx::query(
        r#"
        INSERT INTO asset_face (
            id, "assetId", "personId", "imageWidth", "imageHeight",
            "boundingBoxX1", "boundingBoxY1", "boundingBoxX2", "boundingBoxY2",
            "sourceType", "isVisible"
        )
        VALUES (
            $1::uuid, $2::uuid, $3::uuid, $4, $5,
            $6, $7, $8, $9,
            'manual', true
        )
        "#,
    )
    .bind(&id)
    .bind(asset_id)
    .bind(person_id)
    .bind(image_width)
    .bind(image_height)
    .bind(x)
    .bind(y)
    .bind(x + width)
    .bind(y + height)
    .execute(&state.db)
    .await?;

    let _ = auth;
    Ok(Json(json!({ "id": id })))
}

async fn get_faces(
    State(state): State<AppState>,
    _auth: AuthDto,
    Query(query): Query<FaceQuery>,
) -> Result<Json<Vec<Value>>, AppError> {
    let asset_id = query.id.ok_or_else(|| AppError::BadRequest("id is required".to_string()))?;
    let rows = sqlx::query(
        r#"
        SELECT
            af.id::text as id,
            af."imageWidth",
            af."imageHeight",
            af."boundingBoxX1",
            af."boundingBoxY1",
            af."boundingBoxX2",
            af."boundingBoxY2",
            af."sourceType",
            p.id::text as person_id,
            p.name as person_name,
            p."thumbnailPath" as person_thumbnail
        FROM asset_face af
        LEFT JOIN person p ON p.id = af."personId"
        WHERE af."assetId" = $1::uuid
          AND af."deletedAt" IS NULL
          AND af."isVisible" = true
        ORDER BY af."boundingBoxX1" ASC
        "#,
    )
    .bind(&asset_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(|row| json!({
        "id": row.try_get::<String,_>("id").unwrap_or_default(),
        "imageWidth": row.try_get::<i32,_>("imageWidth").unwrap_or(0),
        "imageHeight": row.try_get::<i32,_>("imageHeight").unwrap_or(0),
        "boundingBoxX1": row.try_get::<i32,_>("boundingBoxX1").unwrap_or(0),
        "boundingBoxY1": row.try_get::<i32,_>("boundingBoxY1").unwrap_or(0),
        "boundingBoxX2": row.try_get::<i32,_>("boundingBoxX2").unwrap_or(0),
        "boundingBoxY2": row.try_get::<i32,_>("boundingBoxY2").unwrap_or(0),
        "sourceType": row.try_get::<String,_>("sourceType").unwrap_or_else(|_| "manual".to_string()),
        "person": row.try_get::<Option<String>,_>("person_id").unwrap_or(None).map(|id| json!({
            "id": id,
            "name": row.try_get::<Option<String>,_>("person_name").unwrap_or(None).unwrap_or_default(),
            "thumbnailPath": row.try_get::<Option<String>,_>("person_thumbnail").unwrap_or(None).unwrap_or_default(),
            "isHidden": false
        })),
    })).collect()))
}

async fn reassign_face(
    State(state): State<AppState>,
    Path(id): Path<String>,
    _auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Value>, AppError> {
    let face_id = payload.get("id").and_then(|v| v.as_str()).unwrap_or(&id);
    sqlx::query(r#"UPDATE asset_face SET "personId" = $1::uuid, "updatedAt" = NOW() WHERE id = $2::uuid"#)
        .bind(&id)
        .bind(face_id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "id": id })))
}

async fn delete_face(
    State(state): State<AppState>,
    Path(id): Path<String>,
    _auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<StatusCode, AppError> {
    let force = payload.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    if force {
        sqlx::query(r#"DELETE FROM asset_face WHERE id = $1::uuid"#)
            .bind(&id)
            .execute(&state.db)
            .await?;
    } else {
        sqlx::query(r#"UPDATE asset_face SET "deletedAt" = NOW(), "isVisible" = false WHERE id = $1::uuid"#)
            .bind(&id)
            .execute(&state.db)
            .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}
