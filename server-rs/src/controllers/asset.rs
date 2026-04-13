use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, put, delete, post},
    Json, Router,
};
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;
use sha1::{Sha1, Digest};
use std::{collections::HashMap, path::PathBuf, process::Command};

use crate::{
    dtos::asset::{
        AssetBulkDeleteDto, AssetBulkUploadCheckDto, AssetBulkUploadCheckResponseDto,
        AssetBulkUploadCheckResultDto, AssetMediaResponseDto, AssetResponseDto,
        AssetStatsResponseDto, RandomAssetsDto, UpdateAssetDto,
    },
    error::AppError,
    middleware::auth::AuthDto,
    jobs::Job,
    ml,
    models::Asset,
    AppState,
};

#[derive(Debug, Clone)]
struct MediaConfig {
    thumbnail_size: u32,
    thumbnail_format: String,
    preview_size: u32,
    preview_format: String,
    fullsize_enabled: bool,
    fullsize_format: String,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            thumbnail_size: 250,
            thumbnail_format: "webp".to_string(),
            preview_size: 1440,
            preview_format: "jpeg".to_string(),
            fullsize_enabled: false,
            fullsize_format: "jpeg".to_string(),
        }
    }
}

const ASSET_SELECT_COLUMNS: &str = r#"
    "id"::text as "id",
    "type",
    "deviceAssetId",
    "ownerId"::text as "ownerId",
    "deviceId",
    "localDateTime",
    "fileCreatedAt",
    "fileModifiedAt",
    "createdAt",
    "updatedAt",
    "originalPath",
    "originalFileName",
    "isFavorite",
    "isOffline",
    "deletedAt",
    checksum,
    thumbhash,
    "livePhotoVideoId"::text as "livePhotoVideoId",
    duration,
    visibility::text as "visibility",
    width,
    height
"#;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(upload_asset).delete(delete_assets).put(update_assets))
        .route("/bulk-upload-check", post(check_bulk_upload))
        .route("/exist", post(check_existing_assets))
        .route("/jobs", post(run_asset_jobs))
        .route("/copy", put(copy_asset))
        .route("/metadata", put(update_bulk_asset_metadata).delete(delete_bulk_asset_metadata))
        .route("/random", get(get_random_assets))
        .route("/statistics", get(get_asset_statistics))
        .route("/device/:device_id", get(get_assets_by_device_id))
        .route("/:id", get(get_asset_info).put(update_asset))
        .route("/:id/metadata", get(get_asset_metadata).put(update_asset_metadata))
        .route("/:id/metadata/:key", get(get_asset_metadata_key).delete(delete_asset_metadata_key))
        .route("/:id/ocr", get(get_asset_ocr))
        .route("/:id/edits", get(get_asset_edits))
        .route("/:id/original", get(download_asset_original).put(replace_asset_original))
        .route("/:id/thumbnail", get(view_asset_thumbnail))
        .route("/:id/video/playback", get(play_asset_video))
}

async fn upload_asset(
    State(state): State<AppState>,
    _auth: AuthDto,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<AssetMediaResponseDto>), AppError> {
    let mut hasher = Sha1::new();
    let mut fields = HashMap::<String, String>::new();
    let mut original_file_name = None::<String>;

    let asset_id = Uuid::new_v4().to_string();

    while let Some(mut field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
        let field_name = field.name().unwrap_or_default().to_string();
        if field_name == "assetData" {
            original_file_name = field.file_name().map(|name| name.to_string());
            let extension = field
                .file_name()
                .and_then(|name| std::path::Path::new(name).extension().and_then(|ext| ext.to_str()))
                .unwrap_or("bin");
            let file_path = build_upload_path(&state.media_location, &_auth.user.id, &asset_id, extension);
            if let Some(parent) = std::path::Path::new(&file_path).parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| AppError::InternalServerError(e.into()))?;
            }
            let mut file = tokio::fs::File::create(&file_path).await.map_err(|e| AppError::InternalServerError(e.into()))?;

            use tokio::io::AsyncWriteExt;
            while let Some(chunk) = field.chunk().await.map_err(|_| AppError::BadRequest("Chunk error".into()))? {
                hasher.update(&chunk);
                file.write_all(&chunk).await.map_err(|e| AppError::InternalServerError(e.into()))?;
            }
            fields.insert("resolvedOriginalPath".to_string(), file_path);
        } else if field_name == "sidecarData" {
            while field.chunk().await.map_err(|_| AppError::BadRequest("Chunk error".into()))?.is_some() {}
        } else {
            let value = field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?;
            fields.insert(field_name, value);
        }
    }

    let device_asset_id = fields
        .get("deviceAssetId")
        .cloned()
        .ok_or_else(|| AppError::BadRequest("deviceAssetId is required".to_string()))?;
    let device_id = fields
        .get("deviceId")
        .cloned()
        .ok_or_else(|| AppError::BadRequest("deviceId is required".to_string()))?;
    let file_created_at = parse_datetime(fields.get("fileCreatedAt"))?;
    let file_modified_at = parse_datetime(fields.get("fileModifiedAt"))?;
    let duration = fields.get("duration").cloned().filter(|value| !value.is_empty());
    let visibility = fields
        .get("visibility")
        .map(|value| value.to_lowercase())
        .unwrap_or_else(|| "timeline".to_string());
    let is_favorite = fields
        .get("isFavorite")
        .map(|value| value == "true")
        .unwrap_or(false);
    let live_photo_video_id = fields.get("livePhotoVideoId").cloned().filter(|value| !value.is_empty());
    let original_file_name = fields
        .get("filename")
        .cloned()
        .or(original_file_name)
        .unwrap_or_else(|| asset_id.clone());
    let file_path = fields
        .get("resolvedOriginalPath")
        .cloned()
        .ok_or_else(|| AppError::BadRequest("assetData is required".to_string()))?;

    let checksum = hasher.finalize().to_vec();

    let duplicate = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id::text as id
        FROM "asset"
        WHERE "ownerId" = $1::uuid
          AND checksum = $2
        LIMIT 1
        "#,
    )
    .bind(&_auth.user.id)
    .bind(&checksum)
    .fetch_optional(&state.db)
    .await?;

    if let Some(existing_id) = duplicate {
        return Ok((
            StatusCode::OK,
            Json(AssetMediaResponseDto {
                id: existing_id,
                duplicate: true,
                status: "duplicate".to_string(),
            }),
        ));
    }

    let asset_type = infer_asset_type(&original_file_name);
    sqlx::query(
        r#"
        INSERT INTO "asset" (
            id,
            "deviceAssetId",
            "ownerId",
            "deviceId",
            type,
            "originalPath",
            "fileCreatedAt",
            "fileModifiedAt",
            "isFavorite",
            duration,
            checksum,
            "livePhotoVideoId",
            "originalFileName",
            "localDateTime",
            visibility,
            "checksumAlgorithm"
        ) VALUES (
            $1::uuid,
            $2,
            $3::uuid,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            $10,
            $11,
            $12::uuid,
            $13,
            $14,
            $15::asset_visibility_enum,
            'sha1'::asset_checksum_algorithm_enum
        )
        "#,
    )
    .bind(&asset_id)
    .bind(&device_asset_id)
    .bind(&_auth.user.id)
    .bind(&device_id)
    .bind(&asset_type)
    .bind(&file_path)
    .bind(file_created_at)
    .bind(file_modified_at)
    .bind(is_favorite)
    .bind(duration)
    .bind(&checksum)
    .bind(live_photo_video_id)
    .bind(&original_file_name)
    .bind(file_created_at)
    .bind(&visibility)
    .execute(&state.db)
    .await?;

    if let Ok(asset) = load_asset(&state, &_auth.user.id, &asset_id).await {
        if generate_initial_media(&state, &asset).await.is_err() {
            tracing::warn!("eager media generation failed for {}", asset.id);
        }
    }

    let _ = state
        .job_queue
        .enqueue(Job::ExtractMetadata {
            id: Uuid::new_v4().to_string(),
            asset_id: asset_id.clone(),
        })
        .await;

    if let Ok(config) = ml::load_ml_config(&state).await {
        if config.enabled {
            if config.clip_enabled {
                let _ = state
                    .job_queue
                    .enqueue(Job::SmartSearch {
                        id: Uuid::new_v4().to_string(),
                        asset_id: asset_id.clone(),
                    })
                    .await;
            }
            if config.facial_enabled {
                let _ = state
                    .job_queue
                    .enqueue(Job::DetectFaces {
                        id: Uuid::new_v4().to_string(),
                        asset_id: asset_id.clone(),
                    })
                    .await;
            }
            if config.ocr_enabled {
                let _ = state
                    .job_queue
                    .enqueue(Job::Ocr {
                        id: Uuid::new_v4().to_string(),
                        asset_id: asset_id.clone(),
                    })
                    .await;
            }
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(AssetMediaResponseDto {
            id: asset_id,
            duplicate: false,
            status: "created".to_string(),
        }),
    ))
}

pub(crate) fn map_asset(asset: Asset) -> AssetResponseDto {
    AssetResponseDto {
        id: asset.id,
        r#type: asset.r#type,
        thumbhash: asset.thumbhash.map(|b| general_purpose::STANDARD.encode(b)),
        local_date_time: asset.local_date_time.to_rfc3339(),
        duration: asset.duration.unwrap_or_else(|| "0:00:00.00000".to_string()),
        has_metadata: true,
        width: asset.width,
        height: asset.height,
        created_at: asset.created_at.to_rfc3339(),
        device_asset_id: asset.device_asset_id,
        device_id: asset.device_id,
        owner_id: asset.owner_id,
        original_path: asset.original_path,
        original_file_name: asset.original_file_name,
        file_created_at: asset.file_created_at.to_rfc3339(),
        file_modified_at: asset.file_modified_at.to_rfc3339(),
        updated_at: asset.updated_at.to_rfc3339(),
        is_favorite: asset.is_favorite,
        is_archived: asset.visibility == "ARCHIVED",
        is_trashed: asset.deleted_at.is_some(),
        is_offline: asset.is_offline,
        visibility: asset.visibility,
        checksum: general_purpose::STANDARD.encode(asset.checksum),
        is_edited: false,
    }
}

async fn get_random_assets(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<RandomAssetsDto>,
) -> Result<Json<Vec<AssetResponseDto>>, AppError> {
    let limit = query.count.unwrap_or(1) as i64;
    
    // In PostgreSQL, ORDER BY RANDOM() is the simplest way.
    let assets = sqlx::query_as::<_, Asset>(
        &format!(
            r#"
            SELECT {ASSET_SELECT_COLUMNS}
            FROM "asset"
            WHERE "ownerId" = $1::uuid AND "deletedAt" IS NULL
            ORDER BY RANDOM()
            LIMIT $2
            "#
        )
    )
    .bind(&auth.user.id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    let response = assets.into_iter().map(map_asset).collect();
    Ok(Json(response))
}

async fn get_asset_statistics(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<AssetStatsResponseDto>, AppError> {
    let total = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM "asset" WHERE "ownerId" = $1::uuid AND "deletedAt" IS NULL"#
    )
    .bind(&auth.user.id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    let images = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM "asset" WHERE "ownerId" = $1::uuid AND "type" = 'IMAGE' AND "deletedAt" IS NULL"#
    )
    .bind(&auth.user.id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    let videos = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM "asset" WHERE "ownerId" = $1::uuid AND "type" = 'VIDEO' AND "deletedAt" IS NULL"#
    )
    .bind(&auth.user.id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    Ok(Json(AssetStatsResponseDto {
        total: total as i32,
        images: images as i32,
        videos: videos as i32,
    }))
}

async fn check_bulk_upload(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(payload): Json<AssetBulkUploadCheckDto>,
) -> Result<Json<AssetBulkUploadCheckResponseDto>, AppError> {
    let mut results = Vec::with_capacity(payload.assets.len());

    for item in payload.assets {
        let checksum = decode_checksum(&item.checksum)?;
        let duplicate = sqlx::query_as::<_, Asset>(
            &format!(
                r#"
                SELECT {ASSET_SELECT_COLUMNS}
                FROM "asset"
                WHERE "ownerId" = $1::uuid
                  AND checksum = $2
                LIMIT 1
                "#
            ),
        )
        .bind(&auth.user.id)
        .bind(checksum)
        .fetch_optional(&state.db)
        .await?;

        if let Some(asset) = duplicate {
            results.push(AssetBulkUploadCheckResultDto {
                id: item.id,
                action: "reject".to_string(),
                asset_id: Some(asset.id),
                is_trashed: Some(asset.deleted_at.is_some()),
                reason: Some("duplicate".to_string()),
            });
        } else {
            results.push(AssetBulkUploadCheckResultDto {
                id: item.id,
                action: "accept".to_string(),
                asset_id: None,
                is_trashed: None,
                reason: None,
            });
        }
    }

    Ok(Json(AssetBulkUploadCheckResponseDto { results }))
}

async fn get_asset_info(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
) -> Result<Json<AssetResponseDto>, AppError> {
    let asset = sqlx::query_as::<_, Asset>(
        &format!(
            r#"
            SELECT {ASSET_SELECT_COLUMNS}
            FROM "asset"
            WHERE "id" = $1::uuid AND "ownerId" = $2::uuid
            "#
        )
    )
    .bind(&id)
    .bind(&auth.user.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?
    .ok_or_else(|| AppError::BadRequest("Asset not found".to_string()))?;

    Ok(Json(map_asset(asset)))
}

async fn update_asset(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
    Json(payload): Json<UpdateAssetDto>,
) -> Result<Json<AssetResponseDto>, AppError> {
    let asset = sqlx::query_as::<_, Asset>(
        &format!(r#"SELECT {ASSET_SELECT_COLUMNS} FROM "asset" WHERE "id" = $1::uuid"#)
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?
    .ok_or_else(|| AppError::BadRequest("Asset not found".to_string()))?;

    if asset.owner_id != auth.user.id {
        return Err(AppError::BadRequest("Forbidden".to_string()));
    }

    let mut tx = state.db.begin().await.map_err(|e| AppError::InternalServerError(e.into()))?;

    let is_favorite = payload.is_favorite.unwrap_or(asset.is_favorite);
    let vis = if payload.is_archived.unwrap_or(asset.visibility.eq_ignore_ascii_case("archive")) {
        "archive"
    } else {
        "timeline"
    };

    let updated_asset = sqlx::query_as::<_, Asset>(
        &format!(
            r#"
            UPDATE "asset"
            SET "isFavorite" = $1, "visibility" = $2::asset_visibility_enum, "updatedAt" = NOW()
            WHERE "id" = $3::uuid
            RETURNING {ASSET_SELECT_COLUMNS}
            "#
        )
    )
    .bind(&is_favorite)
    .bind(&vis)
    .bind(&id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.into()))?;

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.into()))?;

    Ok(Json(map_asset(updated_asset)))
}

async fn delete_assets(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(payload): Json<AssetBulkDeleteDto>,
) -> Result<(), AppError> {
    if payload.ids.is_empty() {
        return Ok(());
    }

    let mut tx = state.db.begin().await.map_err(|e| AppError::InternalServerError(e.into()))?;

    // Immich soft-deletes assets by setting deletedAt
    for id in &payload.ids {
        sqlx::query(
            r#"UPDATE "asset" SET "deletedAt" = NOW() WHERE "id" = $1::uuid AND "ownerId" = $2::uuid"#
        )
        .bind(id)
        .bind(&auth.user.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.into()))?;
    }

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.into()))?;

    Ok(())
}

async fn update_assets() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn check_existing_assets() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({"existingIds": []}))) }
async fn run_asset_jobs() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn copy_asset() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn update_bulk_asset_metadata() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn delete_bulk_asset_metadata() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn get_assets_by_device_id() -> Result<Json<Vec<String>>, AppError> { Ok(Json(vec![])) }
async fn get_asset_metadata() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn update_asset_metadata() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn get_asset_metadata_key() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn delete_asset_metadata_key() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn get_asset_ocr() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn get_asset_edits() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({"stack": []}))) }
#[derive(Debug, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AssetMediaQuery {
    size: Option<String>,
    edited: Option<bool>,
}

async fn download_asset_original(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let asset = load_asset(&state, &auth.user.id, &id).await?;
    serve_file(asset.original_path, asset.original_file_name, false).await
}
async fn replace_asset_original() -> Result<Json<AssetMediaResponseDto>, AppError> { Ok(Json(AssetMediaResponseDto { id: uuid::Uuid::new_v4().to_string(), duplicate: false, status: "replaced".to_string() })) }
async fn view_asset_thumbnail(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
    Query(query): Query<AssetMediaQuery>,
) -> Result<impl IntoResponse, AppError> {
    let asset = load_asset(&state, &auth.user.id, &id).await?;
    let config = load_media_config(&state).await?;
    let size = query.size.unwrap_or_else(|| "thumbnail".to_string());

    if size == "fullsize" && asset.r#type.eq_ignore_ascii_case("IMAGE") && is_web_supported_image(&asset.original_file_name) && query.edited != Some(true) {
        return serve_file(asset.original_path, asset.original_file_name, false).await;
    }

    let derivative_type = match size.as_str() {
        "preview" => "preview",
        "fullsize" => "fullsize",
        _ => "thumbnail",
    };
    let path = ensure_derivative(&state, &asset, derivative_type, &config).await?;
    serve_known_file(path, guess_derivative_content_type(derivative_type, &config), false).await
}
async fn play_asset_video(
    State(state): State<AppState>,
    auth: AuthDto,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let asset = load_asset(&state, &auth.user.id, &id).await?;
    let config = load_media_config(&state).await?;
    let path = if asset.r#type.eq_ignore_ascii_case("VIDEO") && !is_web_playable_video(&asset.original_file_name) {
        ensure_encoded_video(&state, &asset, &config).await?
    } else {
        get_asset_file_path(&state, &asset.id, "encoded_video", false)
            .await?
            .unwrap_or(asset.original_path.clone())
    };
    serve_known_file(path, "video/mp4", false).await
}

async fn serve_static_placeholder(path: &str, content_type: &str) -> Result<Response, AppError> {
    let bytes = tokio::fs::read(path).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    Ok((headers, bytes).into_response())
}

fn decode_checksum(value: &str) -> Result<Vec<u8>, AppError> {
    if value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        let mut bytes = Vec::with_capacity(20);
        let chars: Vec<char> = value.chars().collect();
        for i in (0..chars.len()).step_by(2) {
            let hex = [chars[i], chars[i + 1]].iter().collect::<String>();
            let byte = u8::from_str_radix(&hex, 16)
                .map_err(|_| AppError::BadRequest("Invalid checksum".to_string()))?;
            bytes.push(byte);
        }
        return Ok(bytes);
    }

    general_purpose::STANDARD
        .decode(value)
        .map_err(|_| AppError::BadRequest("Invalid checksum".to_string()))
}

fn parse_datetime(value: Option<&String>) -> Result<DateTime<Utc>, AppError> {
    let value = value.ok_or_else(|| AppError::BadRequest("Missing datetime field".to_string()))?;
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| AppError::BadRequest("Invalid datetime field".to_string()))
}

fn infer_asset_type(file_name: &str) -> String {
    match file_name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" => "VIDEO".to_string(),
        _ => "IMAGE".to_string(),
    }
}

fn build_upload_path(media_location: &str, owner_id: &str, asset_id: &str, extension: &str) -> String {
    let folder = std::path::Path::new(media_location)
        .join("upload")
        .join(owner_id)
        .join(&asset_id[0..2])
        .join(&asset_id[2..4]);
    folder.join(format!("{asset_id}.{extension}")).to_string_lossy().into_owned()
}

async fn load_asset(state: &AppState, owner_id: &str, asset_id: &str) -> Result<Asset, AppError> {
    sqlx::query_as::<_, Asset>(
        &format!(
            r#"
            SELECT {ASSET_SELECT_COLUMNS}
            FROM "asset"
            WHERE "id" = $1::uuid AND "ownerId" = $2::uuid
            "#
        )
    )
    .bind(asset_id)
    .bind(owner_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Asset not found".to_string()))
}

async fn load_asset_by_id(state: &AppState, asset_id: &str) -> Result<Asset, AppError> {
    sqlx::query_as::<_, Asset>(
        &format!(
            r#"
            SELECT {ASSET_SELECT_COLUMNS}
            FROM "asset"
            WHERE "id" = $1::uuid
            "#
        ),
    )
    .bind(asset_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Asset not found".to_string()))
}

async fn serve_file(path: String, file_name: String, attachment: bool) -> Result<Response, AppError> {
    let bytes = tokio::fs::read(&path).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, guess_content_type(&file_name).parse().unwrap());
    if attachment {
        let value = format!("attachment; filename=\"{file_name}\"");
        headers.insert(header::CONTENT_DISPOSITION, value.parse().unwrap());
    }
    Ok((headers, bytes).into_response())
}

async fn serve_known_file(path: String, content_type: &str, attachment: bool) -> Result<Response, AppError> {
    let bytes = tokio::fs::read(&path).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    if attachment {
        let file_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file");
        let value = format!("attachment; filename=\"{file_name}\"");
        headers.insert(header::CONTENT_DISPOSITION, value.parse().unwrap());
    }
    Ok((headers, bytes).into_response())
}

fn guess_content_type(file_name: &str) -> &'static str {
    match file_name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        _ => "application/octet-stream",
    }
}

async fn ensure_derivative(state: &AppState, asset: &Asset, derivative_type: &str, config: &MediaConfig) -> Result<String, AppError> {
    if let Some(path) = get_asset_file_path(state, &asset.id, derivative_type, false).await? {
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(path);
        }
    }

    let output_path = build_thumbnail_path(&state.media_location, &asset.owner_id, &asset.id, derivative_type, config);
    if let Some(parent) = PathBuf::from(&output_path).parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    }

    generate_derivative(asset, derivative_type, &output_path, config)?;
    upsert_asset_file(state, &asset.id, derivative_type, &output_path, false).await?;

    Ok(output_path)
}

async fn ensure_encoded_video(state: &AppState, asset: &Asset, _config: &MediaConfig) -> Result<String, AppError> {
    if let Some(path) = get_asset_file_path(state, &asset.id, "encoded_video", false).await? {
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(path);
        }
    }

    if asset.r#type.eq_ignore_ascii_case("VIDEO") && is_web_playable_video(&asset.original_file_name) {
        return Ok(asset.original_path.clone());
    }

    let output_path = build_encoded_video_path(&state.media_location, &asset.owner_id, &asset.id);
    if let Some(parent) = PathBuf::from(&output_path).parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    }

    generate_encoded_video(asset, &output_path)?;

    upsert_asset_file(state, &asset.id, "encoded_video", &output_path, false).await?;

    Ok(output_path)
}

fn generate_derivative(asset: &Asset, derivative_type: &str, output_path: &str, config: &MediaConfig) -> Result<(), AppError> {
    let (target, format) = match derivative_type {
        "preview" => (config.preview_size, config.preview_format.as_str()),
        "fullsize" => (config.preview_size.max(2160), config.fullsize_format.as_str()),
        _ => (config.thumbnail_size, config.thumbnail_format.as_str()),
    };

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y");
    if asset.r#type.eq_ignore_ascii_case("VIDEO") {
        cmd.args(["-ss", "00:00:00.000"]);
    }
    cmd.args(["-i", &asset.original_path]);
    cmd.args(["-frames:v", "1"]);
    cmd.args(["-vf", &format!("scale='min({target},iw)':-2")]);
    cmd.args(["-q:v", "3"]);
    if format == "webp" {
        cmd.args(["-vcodec", "libwebp"]);
    }
    cmd.arg(output_path);

    let output = cmd.output().map_err(|e| AppError::InternalServerError(e.into()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(AppError::BadRequest(format!("Thumbnail generation failed: {stderr}")));
    }

    Ok(())
}

fn generate_encoded_video(asset: &Asset, output_path: &str) -> Result<(), AppError> {
    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            &asset.original_path,
            "-movflags",
            "+faststart",
            "-pix_fmt",
            "yuv420p",
            "-vcodec",
            "libx264",
            "-acodec",
            "aac",
            output_path,
        ])
        .output()
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(AppError::BadRequest(format!("Encoded video generation failed: {stderr}")));
    }

    Ok(())
}

fn build_thumbnail_path(media_location: &str, owner_id: &str, asset_id: &str, derivative_type: &str, config: &MediaConfig) -> String {
    let extension = match derivative_type {
        "preview" => &config.preview_format,
        "fullsize" => &config.fullsize_format,
        _ => &config.thumbnail_format,
    };
    let folder = std::path::Path::new(media_location)
        .join("thumbs")
        .join(owner_id)
        .join(&asset_id[0..2])
        .join(&asset_id[2..4]);
    folder
        .join(format!("{asset_id}_{derivative_type}.{extension}"))
        .to_string_lossy()
        .into_owned()
}

fn build_encoded_video_path(media_location: &str, owner_id: &str, asset_id: &str) -> String {
    let folder = std::path::Path::new(media_location)
        .join("encoded-video")
        .join(owner_id)
        .join(&asset_id[0..2])
        .join(&asset_id[2..4]);
    folder
        .join(format!("{asset_id}.mp4"))
        .to_string_lossy()
        .into_owned()
}

fn is_web_supported_image(file_name: &str) -> bool {
    matches!(
        file_name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "avif" | "svg"
    )
}

fn is_web_playable_video(file_name: &str) -> bool {
    matches!(
        file_name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str(),
        "mp4" | "webm" | "m4v"
    )
}

async fn load_media_config(state: &AppState) -> Result<MediaConfig, AppError> {
    let mut config = MediaConfig::default();
    let value = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT value FROM system_metadata WHERE key = 'system-config'"#,
    )
    .fetch_optional(&state.db)
    .await?;

    if let Some(value) = value {
        if let Some(image) = value.get("image") {
            if let Some(thumbnail) = image.get("thumbnail") {
                if let Some(size) = thumbnail.get("size").and_then(|v| v.as_u64()) {
                    config.thumbnail_size = size as u32;
                }
                if let Some(format) = thumbnail.get("format").and_then(|v| v.as_str()) {
                    config.thumbnail_format = format.to_ascii_lowercase();
                }
            }
            if let Some(preview) = image.get("preview") {
                if let Some(size) = preview.get("size").and_then(|v| v.as_u64()) {
                    config.preview_size = size as u32;
                }
                if let Some(format) = preview.get("format").and_then(|v| v.as_str()) {
                    config.preview_format = format.to_ascii_lowercase();
                }
            }
            if let Some(fullsize) = image.get("fullsize") {
                if let Some(enabled) = fullsize.get("enabled").and_then(|v| v.as_bool()) {
                    config.fullsize_enabled = enabled;
                }
                if let Some(format) = fullsize.get("format").and_then(|v| v.as_str()) {
                    config.fullsize_format = format.to_ascii_lowercase();
                }
            }
        }
    }

    Ok(config)
}

async fn generate_initial_media(state: &AppState, asset: &Asset) -> Result<(), AppError> {
    let config = load_media_config(state).await?;
    let _ = ensure_derivative(state, asset, "thumbnail", &config).await?;
    let _ = ensure_derivative(state, asset, "preview", &config).await?;
    if config.fullsize_enabled && !is_web_supported_image(&asset.original_file_name) {
        let _ = ensure_derivative(state, asset, "fullsize", &config).await?;
    }
    if asset.r#type.eq_ignore_ascii_case("VIDEO") {
        let _ = ensure_encoded_video(state, asset, &config).await?;
    }
    Ok(())
}

pub(crate) async fn run_media_job(state: &AppState, job: &Job) -> Result<(), AppError> {
    match job {
        Job::ExtractMetadata { asset_id, .. } => {
            let _asset = load_asset_by_id(state, asset_id).await?;
            sqlx::query(
                r#"
                INSERT INTO asset_job_status ("assetId", "metadataExtractedAt")
                VALUES ($1::uuid, now())
                ON CONFLICT ("assetId")
                DO UPDATE SET "metadataExtractedAt" = EXCLUDED."metadataExtractedAt"
                "#,
            )
            .bind(asset_id)
            .execute(&state.db)
            .await?;
        }
        Job::GenerateThumbnail { asset_id, .. } => {
            let asset = load_asset_by_id(state, asset_id).await?;
            generate_initial_media(state, &asset).await?;
        }
        Job::TranscodeVideo { asset_id, .. } => {
            let asset = load_asset_by_id(state, asset_id).await?;
            let config = load_media_config(state).await?;
            let _ = ensure_encoded_video(state, &asset, &config).await?;
        }
        Job::SmartSearch { asset_id, .. } => {
            let asset = load_asset_by_id(state, asset_id).await?;
            let config = ml::load_ml_config(state).await?;
            if !config.clip_enabled {
                return Ok(());
            }
            let entries = serde_json::json!({
                "clip": {
                    "visual": { "modelName": config.clip_model_name }
                }
            });
            let response = ml::predict_image(state, entries, &asset.original_path).await?;
            let embedding = response
                .get("clip")
                .and_then(|value| value.as_str())
                .ok_or_else(|| AppError::BadRequest("Missing ML embedding".to_string()))?;
            sqlx::query(
                r#"
                INSERT INTO smart_search ("assetId", embedding)
                VALUES ($1::uuid, $2::vectors.vector)
                ON CONFLICT ("assetId")
                DO UPDATE SET embedding = EXCLUDED.embedding
                "#,
            )
            .bind(asset_id)
            .bind(embedding)
            .execute(&state.db)
            .await?;
        }
        Job::DetectFaces { asset_id, .. } => {
            let asset = load_asset_by_id(state, asset_id).await?;
            let config = ml::load_ml_config(state).await?;
            if !config.facial_enabled {
                return Ok(());
            }
            let entries = serde_json::json!({
                "facial-recognition": {
                    "detection": { "modelName": config.facial_model_name, "options": { "minScore": config.facial_min_score } },
                    "recognition": { "modelName": config.facial_model_name }
                }
            });
            let response = ml::predict_image(state, entries, &asset.original_path).await?;
            let faces = response
                .get("facial-recognition")
                .and_then(|value| value.as_array())
                .ok_or_else(|| AppError::BadRequest("Missing face data".to_string()))?;
            let image_width = response.get("imageWidth").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let image_height = response.get("imageHeight").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

            sqlx::query(r#"DELETE FROM asset_face WHERE "assetId" = $1::uuid"#)
                .bind(asset_id)
                .execute(&state.db)
                .await?;

            for face in faces {
                let bbox = face.get("boundingBox").and_then(|v| v.as_object()).ok_or_else(|| {
                    AppError::BadRequest("Missing face bounding box".to_string())
                })?;
                let embedding = face
                    .get("embedding")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| AppError::BadRequest("Missing face embedding".to_string()))?;

                let face_id: String = sqlx::query_scalar(
                    r#"
                    INSERT INTO asset_face ("assetId", "imageWidth", "imageHeight", "boundingBoxX1", "boundingBoxY1", "boundingBoxX2", "boundingBoxY2")
                    VALUES ($1::uuid, $2, $3, $4, $5, $6, $7)
                    RETURNING id::text
                    "#,
                )
                .bind(asset_id)
                .bind(image_width)
                .bind(image_height)
                .bind(bbox.get("x1").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32)
                .bind(bbox.get("y1").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32)
                .bind(bbox.get("x2").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32)
                .bind(bbox.get("y2").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32)
                .fetch_one(&state.db)
                .await?;

                sqlx::query(
                    r#"
                    INSERT INTO face_search ("faceId", embedding)
                    VALUES ($1::uuid, $2::vectors.vector)
                    ON CONFLICT ("faceId")
                    DO UPDATE SET embedding = EXCLUDED.embedding
                    "#,
                )
                .bind(face_id)
                .bind(embedding)
                .execute(&state.db)
                .await?;
            }

            sqlx::query(
                r#"
                INSERT INTO asset_job_status ("assetId", "facesRecognizedAt")
                VALUES ($1::uuid, now())
                ON CONFLICT ("assetId")
                DO UPDATE SET "facesRecognizedAt" = EXCLUDED."facesRecognizedAt"
                "#,
            )
            .bind(asset_id)
            .execute(&state.db)
            .await?;
        }
        Job::Ocr { asset_id, .. } => {
            let asset = load_asset_by_id(state, asset_id).await?;
            let config = ml::load_ml_config(state).await?;
            if !config.ocr_enabled {
                return Ok(());
            }
            let entries = serde_json::json!({
                "ocr": {
                    "detection": { "modelName": config.ocr_model_name, "options": { "minScore": config.ocr_min_detection_score, "maxResolution": config.ocr_max_resolution } },
                    "recognition": { "modelName": config.ocr_model_name, "options": { "minScore": config.ocr_min_recognition_score } }
                }
            });
            let response = ml::predict_image(state, entries, &asset.original_path).await?;
            let ocr = response
                .get("ocr")
                .and_then(|value| value.as_object())
                .ok_or_else(|| AppError::BadRequest("Missing OCR data".to_string()))?;

            let texts = ocr.get("text").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let boxes = ocr.get("box").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let box_scores = ocr.get("boxScore").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let text_scores = ocr.get("textScore").and_then(|v| v.as_array()).cloned().unwrap_or_default();

            sqlx::query(r#"DELETE FROM asset_ocr WHERE "assetId" = $1::uuid"#)
                .bind(asset_id)
                .execute(&state.db)
                .await?;

            let mut box_values = boxes.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>();
            let mut offset = 0usize;
            for (idx, text) in texts.iter().enumerate() {
                if offset + 7 >= box_values.len() {
                    break;
                }
                let text_value = text.as_str().unwrap_or("").to_string();
                let box_score = box_scores.get(idx).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let text_score = text_scores.get(idx).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

                sqlx::query(
                    r#"
                    INSERT INTO asset_ocr ("assetId", x1, y1, x2, y2, x3, y3, x4, y4, "boxScore", "textScore", text)
                    VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    "#,
                )
                .bind(asset_id)
                .bind(box_values[offset] as f32)
                .bind(box_values[offset + 1] as f32)
                .bind(box_values[offset + 2] as f32)
                .bind(box_values[offset + 3] as f32)
                .bind(box_values[offset + 4] as f32)
                .bind(box_values[offset + 5] as f32)
                .bind(box_values[offset + 6] as f32)
                .bind(box_values[offset + 7] as f32)
                .bind(box_score)
                .bind(text_score)
                .bind(text_value)
                .execute(&state.db)
                .await?;

                offset += 8;
            }

            sqlx::query(
                r#"
                INSERT INTO asset_job_status ("assetId", "ocrAt")
                VALUES ($1::uuid, now())
                ON CONFLICT ("assetId")
                DO UPDATE SET "ocrAt" = EXCLUDED."ocrAt"
                "#,
            )
            .bind(asset_id)
            .execute(&state.db)
            .await?;
        }
    }
    Ok(())
}

async fn get_asset_file_path(state: &AppState, asset_id: &str, file_type: &str, is_edited: bool) -> Result<Option<String>, AppError> {
    Ok(sqlx::query_scalar::<_, String>(
        r#"
        SELECT path
        FROM asset_file
        WHERE "assetId" = $1::uuid
          AND type = $2
          AND "isEdited" = $3
        ORDER BY "updatedAt" DESC
        LIMIT 1
        "#,
    )
    .bind(asset_id)
    .bind(file_type)
    .bind(is_edited)
    .fetch_optional(&state.db)
    .await?)
}

async fn upsert_asset_file(state: &AppState, asset_id: &str, file_type: &str, path: &str, is_edited: bool) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO asset_file ("assetId", type, path, "isEdited", "isProgressive", "isTransparent")
        VALUES ($1::uuid, $2, $3, $4, false, false)
        ON CONFLICT ("assetId", type, "isEdited")
        DO UPDATE SET path = EXCLUDED.path
        "#,
    )
    .bind(asset_id)
    .bind(file_type)
    .bind(path)
    .bind(is_edited)
    .execute(&state.db)
    .await?;
    Ok(())
}

fn guess_derivative_content_type(derivative_type: &str, config: &MediaConfig) -> &'static str {
    let format = match derivative_type {
        "preview" => config.preview_format.as_str(),
        "fullsize" => config.fullsize_format.as_str(),
        _ => config.thumbnail_format.as_str(),
    };
    match format {
        "webp" => "image/webp",
        "png" => "image/png",
        _ => "image/jpeg",
    }
}
