use axum::{
    extract::{Json as JsonBody, Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ActivitySearchDto {
    album_id: Option<String>,
    asset_id: Option<String>,
    user_id: Option<String>,
    r#type: Option<String>,
    level: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_activities).post(create_activity))
        .route("/statistics", get(get_activity_statistics))
        .route("/:id", delete(delete_activity))
}

async fn get_activities(
    State(state): State<AppState>,
    _auth: AuthDto,
    Query(query): Query<ActivitySearchDto>,
) -> Result<Json<Vec<Value>>, AppError> {
    let album_id = query.album_id.ok_or_else(|| AppError::BadRequest("albumId is required".to_string()))?;
    let asset_id_filter = if query.level.as_deref() == Some("ALBUM") { None } else { query.asset_id.clone() };
    let is_liked = query.r#type.as_deref().map(|v| v == "like");

    let rows = sqlx::query(
        r#"
        SELECT
            a.id::text as id,
            a."createdAt",
            a.comment,
            a."assetId"::text as "assetId",
            a."isLiked",
            u.id::text as user_id,
            u.name as user_name,
            u."profileImagePath" as user_profile_image
        FROM activity a
        JOIN "user" u ON u.id = a."userId" AND u."deletedAt" IS NULL
        LEFT JOIN asset asset_row ON asset_row.id = a."assetId"
        WHERE a."albumId" = $1::uuid
          AND ($2::uuid IS NULL OR a."assetId" = $2::uuid)
          AND ($3::uuid IS NULL OR a."userId" = $3::uuid)
          AND ($4::bool IS NULL OR a."isLiked" = $4::bool)
          AND (asset_row.id IS NULL OR asset_row."deletedAt" IS NULL)
        ORDER BY a."createdAt" ASC
        "#,
    )
    .bind(&album_id)
    .bind(asset_id_filter)
    .bind(query.user_id)
    .bind(is_liked)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(|row| json!({
        "id": row.try_get::<String,_>("id").unwrap_or_default(),
        "createdAt": row.try_get::<chrono::DateTime<chrono::Utc>,_>("createdAt").ok().map(|d| d.to_rfc3339()),
        "comment": row.try_get::<Option<String>,_>("comment").unwrap_or(None),
        "assetId": row.try_get::<Option<String>,_>("assetId").unwrap_or(None),
        "type": if row.try_get::<bool,_>("isLiked").unwrap_or(false) { "like" } else { "comment" },
        "user": {
            "id": row.try_get::<String,_>("user_id").unwrap_or_default(),
            "name": row.try_get::<String,_>("user_name").unwrap_or_default(),
            "profileImagePath": row.try_get::<String,_>("user_profile_image").unwrap_or_default(),
        }
    })).collect()))
}

async fn create_activity(
    State(state): State<AppState>,
    auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let album_id = payload.get("albumId").and_then(|v| v.as_str()).ok_or_else(|| AppError::BadRequest("albumId is required".to_string()))?;
    let asset_id = payload.get("assetId").and_then(|v| v.as_str());
    let is_liked = payload.get("type").and_then(|v| v.as_str()) == Some("like");
    let comment = payload.get("comment").and_then(|v| v.as_str());

    let duplicate = if is_liked {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM activity WHERE "albumId" = $1::uuid AND "userId" = $2::uuid AND COALESCE("assetId"::text, '') = COALESCE($3, '') AND "isLiked" = true"#,
        )
        .bind(&album_id)
        .bind(&auth.user.id)
        .bind(asset_id)
        .fetch_one(&state.db)
        .await?
            > 0
    } else {
        false
    };

    let id = if duplicate {
        sqlx::query_scalar::<_, String>(
            r#"SELECT id::text FROM activity WHERE "albumId" = $1::uuid AND "userId" = $2::uuid AND COALESCE("assetId"::text, '') = COALESCE($3, '') AND "isLiked" = true LIMIT 1"#,
        )
        .bind(&album_id)
        .bind(&auth.user.id)
        .bind(asset_id)
        .fetch_one(&state.db)
        .await?
    } else {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO activity (id, "albumId", "userId", "assetId", comment, "isLiked")
            VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5, $6)
            "#,
        )
        .bind(&id)
        .bind(&album_id)
        .bind(&auth.user.id)
        .bind(asset_id)
        .bind(comment)
        .bind(is_liked)
        .execute(&state.db)
        .await?;
        id
    };

    Ok((
        if duplicate { StatusCode::OK } else { StatusCode::CREATED },
        Json(json!({
            "id": id,
            "createdAt": chrono::Utc::now().to_rfc3339(),
            "comment": comment,
            "assetId": asset_id,
            "type": if is_liked { "like" } else { "comment" },
            "user": {
                "id": auth.user.id,
                "name": auth.user.name,
                "profileImagePath": auth.user.profile_image_path,
            }
        })),
    ))
}

async fn get_activity_statistics(
    State(state): State<AppState>,
    _auth: AuthDto,
    Query(query): Query<ActivitySearchDto>,
) -> Result<Json<Value>, AppError> {
    let album_id = query.album_id.ok_or_else(|| AppError::BadRequest("albumId is required".to_string()))?;
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE "isLiked" = false) as comments,
            COUNT(*) FILTER (WHERE "isLiked" = true) as likes
        FROM activity
        WHERE "albumId" = $1::uuid
          AND ($2::uuid IS NULL OR "assetId" = $2::uuid)
        "#,
    )
    .bind(&album_id)
    .bind(query.asset_id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({
        "comments": row.try_get::<i64,_>("comments").unwrap_or(0),
        "likes": row.try_get::<i64,_>("likes").unwrap_or(0)
    })))
}

async fn delete_activity(
    State(state): State<AppState>,
    Path(id): Path<String>,
    _auth: AuthDto,
) -> Result<StatusCode, AppError> {
    sqlx::query(r#"DELETE FROM activity WHERE id = $1::uuid"#)
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
