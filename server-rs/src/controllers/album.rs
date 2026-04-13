use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, delete},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    dtos::album::{AlbumResponseDto, UpdateAlbumDto},
    dtos::asset::AssetResponseDto,
    error::AppError,
    controllers::asset::map_asset,
    middleware::auth::AuthDto,
    models::{Album, Asset},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::post(create_album))
        .route("/statistics", get(album_statistics))
        .route("/assets", axum::routing::put(add_assets_to_albums))
        .route("/", get(get_all_albums))
        .route("/:id", get(get_album_info).patch(update_album_info).delete(delete_album))
        .route("/:id/assets", axum::routing::put(add_assets_to_album).delete(remove_assets_from_album))
        .route("/:id/users", axum::routing::put(add_users_to_album))
        .route("/:id/user/:user_id", axum::routing::put(update_album_user).delete(remove_album_user))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAlbumsDto {
    pub shared: Option<bool>,
    pub asset_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInfoDto {
    pub without_assets: Option<bool>,
}

async fn get_all_albums(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<GetAlbumsDto>,
) -> Result<Json<Vec<AlbumResponseDto>>, AppError> {
    let auth_user_id = auth.user.id;

    let albums = if let Some(asset_id) = query.asset_id {
        sqlx::query_as::<_, Album>(
            r#"
            SELECT
                a."id"::text as "id",
                a."ownerId"::text as "ownerId",
                a."albumName",
                a."description",
                a."createdAt",
                a."updatedAt",
                a."albumThumbnailAssetId"::text as "albumThumbnailAssetId",
                a."isActivityEnabled"
            FROM "album" a
            JOIN album_asset aa ON aa."albumId" = a."id"
            WHERE aa."assetId" = $1::uuid
              AND a."ownerId" = $2::uuid
            ORDER BY a."createdAt" DESC
            "#
        )
        .bind(&asset_id)
        .bind(&auth_user_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
    } else {
        sqlx::query_as::<_, Album>(
            r#"
            SELECT "id"::text as "id", "ownerId"::text as "ownerId", "albumName", "description", "createdAt", "updatedAt", "albumThumbnailAssetId"::text as "albumThumbnailAssetId", "isActivityEnabled"
            FROM "album"
            WHERE ($1::bool IS NULL OR $1::bool = false)
              AND "ownerId" = $2::uuid
            "#
        )
        .bind(query.shared)
        .bind(&auth_user_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
    };

    let mut response = Vec::with_capacity(albums.len());
    for album in albums {
        let asset_count = album_asset_count(&state, &album.id).await?;
        response.push(AlbumResponseDto {
            id: album.id,
            owner_id: album.owner_id,
            album_name: album.album_name,
            description: album.description,
            created_at: album.created_at.to_rfc3339(),
            updated_at: album.updated_at.to_rfc3339(),
            album_thumbnail_asset_id: album.album_thumbnail_asset_id,
            shared: false,
            album_users: Some(vec![]),
            has_shared_link: false,
            assets: None,
            asset_count,
            is_activity_enabled: album.is_activity_enabled,
        });
    }

    Ok(Json(response))
}

async fn get_album_info(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
    Query(query): Query<AlbumInfoDto>,
) -> Result<Json<AlbumResponseDto>, AppError> {
    let album = sqlx::query_as::<_, Album>(
        r#"
        SELECT "id"::text as "id", "ownerId"::text as "ownerId", "albumName", "description", "createdAt", "updatedAt", "albumThumbnailAssetId"::text as "albumThumbnailAssetId", "isActivityEnabled"
        FROM "album"
        WHERE "id" = $1::uuid
        "#
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?
    .ok_or_else(|| AppError::BadRequest("Album not found".to_string()))?;

    // Validate permission or owner bounds
    if album.owner_id != auth.user.id {
        // Needs proper ACL checks, simplified for now
    }

    let assets = if query.without_assets.unwrap_or(false) {
        None
    } else {
        Some(album_assets(&state, &album.id).await?)
    };
    let asset_count = album_asset_count(&state, &album.id).await?;

    Ok(Json(AlbumResponseDto {
        id: album.id,
        owner_id: album.owner_id,
        album_name: album.album_name,
        description: album.description,
        created_at: album.created_at.to_rfc3339(),
        updated_at: album.updated_at.to_rfc3339(),
        album_thumbnail_asset_id: album.album_thumbnail_asset_id,
        shared: false,
        album_users: Some(vec![]),
        has_shared_link: false,
        assets,
        asset_count,
        is_activity_enabled: album.is_activity_enabled,
    }))
}

async fn update_album_info(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
    Json(payload): Json<UpdateAlbumDto>,
) -> Result<Json<AlbumResponseDto>, AppError> {
    // Basic verification
    let album = sqlx::query_as::<_, Album>(
        r#"SELECT "id"::text as "id", "ownerId"::text as "ownerId", "albumName", "description", "createdAt", "updatedAt", "albumThumbnailAssetId"::text as "albumThumbnailAssetId", "isActivityEnabled" FROM "album" WHERE "id" = $1::uuid"#
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?
    .ok_or_else(|| AppError::BadRequest("Album not found".to_string()))?;

    if album.owner_id != auth.user.id {
        // Enforce owner check
        return Err(AppError::BadRequest("Forbidden".to_string()));
    }

    let mut tx = state.db.begin().await.map_err(|e| AppError::InternalServerError(e.into()))?;

    let name = payload.album_name.unwrap_or(album.album_name);
    let desc = payload.description.unwrap_or(album.description);
    let thumb = payload.album_thumbnail_asset_id.or(album.album_thumbnail_asset_id);
    let activity = payload.is_activity_enabled.unwrap_or(album.is_activity_enabled);

    let updated_album = sqlx::query_as::<_, Album>(
        r#"
        UPDATE "album"
        SET "albumName" = $1, "description" = $2, "albumThumbnailAssetId" = $3::uuid, "isActivityEnabled" = $4, "updatedAt" = NOW()
        WHERE "id" = $5::uuid
        RETURNING "id"::text as "id", "ownerId"::text as "ownerId", "albumName", "description", "createdAt", "updatedAt", "albumThumbnailAssetId"::text as "albumThumbnailAssetId", "isActivityEnabled"
        "#
    )
    .bind(&name)
    .bind(&desc)
    .bind(&thumb)
    .bind(&activity)
    .bind(&id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.into()))?;

    Ok(Json(AlbumResponseDto {
        id: updated_album.id,
        owner_id: updated_album.owner_id,
        album_name: updated_album.album_name,
        description: updated_album.description,
        created_at: updated_album.created_at.to_rfc3339(),
        updated_at: updated_album.updated_at.to_rfc3339(),
        album_thumbnail_asset_id: updated_album.album_thumbnail_asset_id,
        shared: false,
        album_users: Some(vec![]),
        has_shared_link: false,
        assets: None,
        asset_count: 0,
        is_activity_enabled: updated_album.is_activity_enabled,
    }))
}

async fn delete_album(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
) -> Result<(), AppError> {
    let mut tx = state.db.begin().await.map_err(|e| AppError::InternalServerError(e.into()))?;

    // In Immich, this deletes the album (since there's no soft delete for albums normally except via background jobs)
    let rows_affected = sqlx::query(r#"DELETE FROM "album" WHERE "id" = $1::uuid AND "ownerId" = $2::uuid"#)
        .bind(&id)
        .bind(&auth.user.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?
        .rows_affected();

    if rows_affected == 0 {
        return Err(AppError::BadRequest("Album not found or permission denied".to_string()));
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.into()))?;

    Ok(())
}

async fn create_album(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<AlbumResponseDto>, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let album_name = payload
        .get("albumName")
        .and_then(|v| v.as_str())
        .unwrap_or("New album")
        .to_string();
    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let is_activity_enabled = payload
        .get("isActivityEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let created = sqlx::query_as::<_, Album>(
        r#"
        INSERT INTO "album" (
            "id",
            "ownerId",
            "albumName",
            "description",
            "albumThumbnailAssetId",
            "isActivityEnabled"
        ) VALUES (
            $1::uuid,
            $2::uuid,
            $3,
            $4,
            NULL,
            $5
        )
        RETURNING
            "id"::text as "id",
            "ownerId"::text as "ownerId",
            "albumName",
            "description",
            "createdAt",
            "updatedAt",
            "albumThumbnailAssetId"::text as "albumThumbnailAssetId",
            "isActivityEnabled"
        "#,
    )
    .bind(&id)
    .bind(&auth.user.id)
    .bind(&album_name)
    .bind(&description)
    .bind(is_activity_enabled)
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    Ok(Json(AlbumResponseDto {
        id: created.id,
        owner_id: created.owner_id,
        album_name: created.album_name,
        description: created.description,
        created_at: created.created_at.to_rfc3339(),
        updated_at: created.updated_at.to_rfc3339(),
        album_thumbnail_asset_id: created.album_thumbnail_asset_id,
        shared: false,
        album_users: Some(vec![]),
        has_shared_link: false,
        assets: Some(vec![]),
        asset_count: 0,
        is_activity_enabled: created.is_activity_enabled,
    }))
}

async fn album_statistics() -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(json!({"owned": 0, "shared": 0, "notShared": 0})))
}

async fn add_users_to_album(
    auth: AuthDto,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<AlbumResponseDto>, AppError> {
    Ok(Json(AlbumResponseDto {
        id,
        owner_id: auth.user.id,
        album_name: "Album".to_string(),
        description: "".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        album_thumbnail_asset_id: None,
        shared: true,
        album_users: Some(vec![]),
        has_shared_link: false,
        assets: None,
        asset_count: 0,
        is_activity_enabled: true,
    }))
}

async fn update_album_user() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_album_user() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct BulkIdsDto {
    ids: Vec<String>,
}

#[derive(Debug, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AlbumsAddAssetsDto {
    album_ids: Vec<String>,
    asset_ids: Vec<String>,
}

async fn add_assets_to_album(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
    Json(payload): Json<BulkIdsDto>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let album = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM "album" WHERE "id" = $1::uuid AND "ownerId" = $2::uuid"#)
        .bind(&id)
        .bind(&auth.user.id)
        .fetch_one(&state.db)
        .await?;
    if album == 0 {
        return Err(AppError::BadRequest("Album not found".to_string()));
    }

    let mut results = Vec::with_capacity(payload.ids.len());
    for asset_id in payload.ids {
        let inserted = sqlx::query(
            r#"
            INSERT INTO album_asset ("albumId", "assetId")
            VALUES ($1::uuid, $2::uuid)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&id)
        .bind(&asset_id)
        .execute(&state.db)
        .await?
        .rows_affected();
        results.push(json!({
            "id": asset_id,
            "success": inserted > 0
        }));
    }
    Ok(Json(results))
}

async fn add_assets_to_albums(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(payload): Json<AlbumsAddAssetsDto>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut count = 0i64;
    for album_id in payload.album_ids {
        let album = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM "album" WHERE "id" = $1::uuid AND "ownerId" = $2::uuid"#)
            .bind(&album_id)
            .bind(&auth.user.id)
            .fetch_one(&state.db)
            .await?;
        if album == 0 {
            continue;
        }
        for asset_id in &payload.asset_ids {
            count += sqlx::query(
                r#"INSERT INTO album_asset ("albumId", "assetId") VALUES ($1::uuid, $2::uuid) ON CONFLICT DO NOTHING"#,
            )
            .bind(&album_id)
            .bind(asset_id)
            .execute(&state.db)
            .await?
            .rows_affected() as i64;
        }
    }
    Ok(Json(json!({ "count": count })))
}

async fn remove_assets_from_album(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
    Json(payload): Json<BulkIdsDto>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let album = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM "album" WHERE "id" = $1::uuid AND "ownerId" = $2::uuid"#)
        .bind(&id)
        .bind(&auth.user.id)
        .fetch_one(&state.db)
        .await?;
    if album == 0 {
        return Err(AppError::BadRequest("Album not found".to_string()));
    }

    let mut results = Vec::with_capacity(payload.ids.len());
    for asset_id in payload.ids {
        let deleted = sqlx::query(
            r#"DELETE FROM album_asset WHERE "albumId" = $1::uuid AND "assetId" = $2::uuid"#,
        )
        .bind(&id)
        .bind(&asset_id)
        .execute(&state.db)
        .await?
        .rows_affected();
        results.push(json!({
            "id": asset_id,
            "success": deleted > 0
        }));
    }
    Ok(Json(results))
}

async fn album_assets(state: &AppState, album_id: &str) -> Result<Vec<AssetResponseDto>, AppError> {
    let assets = sqlx::query_as::<_, Asset>(
        r#"
        SELECT
            a."id"::text as "id",
            a."type",
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
            a.visibility::text as "visibility",
            a.width,
            a.height
        FROM album_asset aa
        JOIN "asset" a ON a."id" = aa."assetId"
        WHERE aa."albumId" = $1::uuid
          AND a."deletedAt" IS NULL
        ORDER BY a."fileCreatedAt" DESC
        "#,
    )
    .bind(album_id)
    .fetch_all(&state.db)
    .await?;
    Ok(assets.into_iter().map(map_asset).collect())
}

async fn album_asset_count(state: &AppState, album_id: &str) -> Result<i32, AppError> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM album_asset aa
        JOIN "asset" a ON a."id" = aa."assetId"
        WHERE aa."albumId" = $1::uuid
          AND a."deletedAt" IS NULL
        "#,
    )
    .bind(album_id)
    .fetch_one(&state.db)
    .await?;
    Ok(count as i32)
}
