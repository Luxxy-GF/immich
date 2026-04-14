use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Datelike, Utc};
use serde::Deserialize;
use serde_json::json;

use crate::{
    dtos::memory::MemoryResponseDto,
    dtos::memory::MemoryOnThisDayDto,
    dtos::asset::AssetResponseDto,
    controllers::asset::map_asset,
    error::AppError,
    middleware::auth::AuthDto,
    models::{Asset, Memory},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_memories).post(create_memory))
        .route("/statistics", get(memory_statistics))
        .route("/:id", get(get_memory).put(update_memory).delete(delete_memory))
        .route("/:id/assets", axum::routing::put(add_memory_assets).delete(remove_memory_assets))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemorySearchQuery {
    #[serde(rename = "for")]
    for_date: Option<DateTime<Utc>>,
    is_saved: Option<bool>,
    is_trashed: Option<bool>,
}

async fn get_memories(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<MemorySearchQuery>,
) -> Result<Json<Vec<MemoryResponseDto>>, AppError> {
    let target_date = query.for_date.unwrap_or_else(Utc::now);
    let memories = sqlx::query_as::<_, Memory>(
        r#"
        SELECT
            id::text as id,
            "createdAt",
            "updatedAt",
            "deletedAt",
            "ownerId"::text as "ownerId",
            type,
            data,
            "isSaved",
            "memoryAt",
            "seenAt",
            "showAt",
            "hideAt"
        FROM memory
        WHERE "ownerId" = $1::uuid
          AND ($2::bool IS NULL OR "isSaved" = $2::bool)
          AND ($3::bool IS NULL OR ("deletedAt" IS NOT NULL) = $3::bool)
          AND "memoryAt" <= $4
          AND ("showAt" IS NULL OR "showAt" <= $4)
          AND ("hideAt" IS NULL OR "hideAt" > $4)
        ORDER BY "memoryAt" DESC
        LIMIT 100
        "#,
    )
    .bind(&auth.user.id)
    .bind(query.is_saved)
    .bind(query.is_trashed)
    .bind(target_date)
    .fetch_all(&state.db)
    .await?;

    let mut response = Vec::with_capacity(memories.len());
    for memory in memories {
        let assets = sqlx::query_as::<_, Asset>(
            r#"
            SELECT
                a.id::text as id,
                a.type,
                a."deviceAssetId",
                a."ownerId"::text as "ownerId",
                a."deviceId",
                a."localDateTime",
                a."fileCreatedAt",
                a."fileModifiedAt",
                a."createdAt",
                a."updatedAt",
                a."originalPath",
                a."originalFileName",
                a."isFavorite",
                a."isOffline",
                a."deletedAt",
                a.checksum,
                a.thumbhash,
                a."livePhotoVideoId"::text as "livePhotoVideoId",
                a.duration,
                a.visibility::text as visibility,
                a.width,
                a.height
            FROM memory_asset ma
            JOIN asset a ON a.id = ma."assetId"
            WHERE ma."memoriesId" = $1::uuid
              AND a."deletedAt" IS NULL
            ORDER BY a."fileCreatedAt" DESC
            "#,
        )
        .bind(&memory.id)
        .fetch_all(&state.db)
        .await?;

        let asset_dtos: Vec<AssetResponseDto> = assets.into_iter().map(map_asset).collect();
        let year = memory
            .data
            .get("year")
            .and_then(|value| value.as_i64())
            .unwrap_or(memory.memory_at.year() as i64) as i32;

        response.push(MemoryResponseDto {
            id: memory.id,
            owner_id: memory.owner_id,
            memory_at: memory.memory_at.to_rfc3339(),
            created_at: memory.created_at.to_rfc3339(),
            updated_at: memory.updated_at.to_rfc3339(),
            is_saved: memory.is_saved,
            data: MemoryOnThisDayDto { year },
            assets: asset_dtos,
            r#type: memory.r#type,
            deleted_at: memory.deleted_at.map(|value| value.to_rfc3339()),
            hide_at: memory.hide_at.map(|value| value.to_rfc3339()),
            seen_at: memory.seen_at.map(|value| value.to_rfc3339()),
            show_at: memory.show_at.map(|value| value.to_rfc3339()),
        });
    }

    Ok(Json(response))
}

async fn create_memory() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn memory_statistics() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({"total": 0}))) }
async fn get_memory() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn update_memory() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn delete_memory() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn add_memory_assets() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn remove_memory_assets() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
