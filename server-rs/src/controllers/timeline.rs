use axum::{extract::{Query, State}, routing::get, Json, Router};
use base64::Engine as _;
use serde::Deserialize;

use crate::{
    dtos::timeline::{TimeBucketAssetResponseDto, TimeBucketsResponseDto},
    error::AppError,
    middleware::auth::AuthDto,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/buckets", get(get_time_buckets))
        .route("/bucket", get(get_time_bucket))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimeBucketQuery {
    time_bucket: Option<String>,
    visibility: Option<String>,
    with_partners: Option<bool>,
}

#[derive(Debug, sqlx::FromRow)]
struct TimeBucketRow {
    #[sqlx(rename = "timeBucket")]
    time_bucket: String,
    count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct TimeBucketAssetRow {
    id: String,
    #[sqlx(rename = "ownerId")]
    owner_id: String,
    #[sqlx(rename = "fileCreatedAt")]
    file_created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "localDateTime")]
    local_date_time: chrono::DateTime<chrono::Utc>,
    #[sqlx(rename = "isFavorite")]
    is_favorite: bool,
    #[sqlx(rename = "deletedAt")]
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(rename = "livePhotoVideoId")]
    live_photo_video_id: Option<String>,
    duration: Option<String>,
    visibility: String,
    thumbhash: Option<Vec<u8>>,
    r#type: String,
    width: Option<i32>,
    height: Option<i32>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    city: Option<String>,
    country: Option<String>,
    #[sqlx(rename = "projectionType")]
    projection_type: Option<String>,
    #[sqlx(rename = "stackId")]
    stack_id: Option<String>,
    stack_count: Option<i64>,
}

async fn get_time_buckets(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<TimeBucketQuery>,
) -> Result<Json<Vec<TimeBucketsResponseDto>>, AppError> {
    let buckets = sqlx::query_as::<_, TimeBucketRow>(
        r#"
        WITH visible_assets AS (
            SELECT a.*
            FROM asset a
            WHERE a."deletedAt" IS NULL
              AND (
                a."ownerId" = $1::uuid
                OR (
                  COALESCE($2::bool, false) = true
                  AND a."ownerId" IN (
                    SELECT p."sharedById"
                    FROM partner p
                    WHERE p."sharedWithId" = $1::uuid
                      AND p."inTimeline" = true
                  )
                )
              )
              AND ($3::text IS NULL OR a.visibility::text = $3::text)
        )
        SELECT
            to_char(date_trunc('month', "localDateTime"), 'YYYY-MM-DD') as "timeBucket",
            COUNT(*) as count
        FROM visible_assets
        GROUP BY 1
        ORDER BY 1 DESC
        "#,
    )
    .bind(&auth.user.id)
    .bind(query.with_partners)
    .bind(query.visibility)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        buckets
            .into_iter()
            .map(|bucket| TimeBucketsResponseDto {
                count: bucket.count as i32,
                time_bucket: bucket.time_bucket,
            })
            .collect(),
    ))
}

async fn get_time_bucket(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<TimeBucketQuery>,
) -> Result<Json<TimeBucketAssetResponseDto>, AppError> {
    let time_bucket = query
        .time_bucket
        .ok_or_else(|| AppError::BadRequest("timeBucket is required".to_string()))?;

    let assets = sqlx::query_as::<_, TimeBucketAssetRow>(
        r#"
        WITH visible_assets AS (
            SELECT a.*
            FROM asset a
            WHERE (
                a."ownerId" = $1::uuid
                OR (
                    COALESCE($2::bool, false) = true
                    AND a."ownerId" IN (
                        SELECT p."sharedById"
                        FROM partner p
                        WHERE p."sharedWithId" = $1::uuid
                          AND p."inTimeline" = true
                    )
                )
            )
              AND ($3::text IS NULL OR a.visibility::text = $3::text)
              AND a."deletedAt" IS NULL
              AND date_trunc('month', a."localDateTime") = date_trunc('month', $4::timestamptz)
        )
        SELECT
            a.id::text as id,
            a."ownerId"::text as "ownerId",
            a."fileCreatedAt",
            a."localDateTime",
            a."isFavorite",
            a."deletedAt",
            a."livePhotoVideoId"::text as "livePhotoVideoId",
            a.duration,
            a.visibility::text as visibility,
            a.thumbhash,
            a.type,
            a.width,
            a.height,
            ex.latitude,
            ex.longitude,
            ex.city,
            ex.country,
            ex."projectionType",
            a."stackId"::text as "stackId",
            stack_counts.stack_count
        FROM visible_assets a
        LEFT JOIN asset_exif ex ON ex."assetId" = a.id
        LEFT JOIN (
            SELECT "stackId", COUNT(*) as stack_count
            FROM asset
            WHERE "stackId" IS NOT NULL
            GROUP BY "stackId"
        ) stack_counts ON stack_counts."stackId" = a."stackId"
        ORDER BY a."localDateTime" DESC, a.id DESC
        "#,
    )
    .bind(&auth.user.id)
    .bind(query.with_partners)
    .bind(query.visibility)
    .bind(&time_bucket)
    .fetch_all(&state.db)
    .await?;

    let mut response = TimeBucketAssetResponseDto {
        id: Vec::with_capacity(assets.len()),
        owner_id: Vec::with_capacity(assets.len()),
        file_created_at: Vec::with_capacity(assets.len()),
        is_favorite: Vec::with_capacity(assets.len()),
        is_image: Vec::with_capacity(assets.len()),
        is_trashed: Vec::with_capacity(assets.len()),
        live_photo_video_id: Vec::with_capacity(assets.len()),
        local_offset_hours: Vec::with_capacity(assets.len()),
        projection_type: Vec::with_capacity(assets.len()),
        ratio: Vec::with_capacity(assets.len()),
        thumbhash: Vec::with_capacity(assets.len()),
        duration: Vec::with_capacity(assets.len()),
        city: Vec::with_capacity(assets.len()),
        country: Vec::with_capacity(assets.len()),
        visibility: Vec::with_capacity(assets.len()),
        latitude: Some(Vec::with_capacity(assets.len())),
        longitude: Some(Vec::with_capacity(assets.len())),
        stack: Some(Vec::with_capacity(assets.len())),
    };

    for asset in assets {
        response.id.push(asset.id);
        response.owner_id.push(asset.owner_id);
        response.file_created_at.push(asset.file_created_at.to_rfc3339());
        response.is_favorite.push(asset.is_favorite);
        response.is_image.push(asset.r#type.eq_ignore_ascii_case("IMAGE"));
        response.is_trashed.push(asset.deleted_at.is_some());
        response.live_photo_video_id.push(asset.live_photo_video_id);
        response.local_offset_hours.push(0.0);
        response.projection_type.push(asset.projection_type);
        response.ratio.push(compute_ratio(asset.width, asset.height));
        response.thumbhash.push(asset.thumbhash.map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)));
        response.duration.push(asset.duration);
        response.city.push(asset.city);
        response.country.push(asset.country);
        response.visibility.push(asset.visibility);
        if let Some(latitude) = &mut response.latitude {
            latitude.push(asset.latitude);
        }
        if let Some(longitude) = &mut response.longitude {
            longitude.push(asset.longitude);
        }
        if let Some(stack) = &mut response.stack {
            stack.push(asset.stack_id.map(|stack_id| vec![stack_id, asset.stack_count.unwrap_or(1).to_string()]));
        }
    }

    Ok(Json(response))
}

fn compute_ratio(width: Option<i32>, height: Option<i32>) -> f64 {
    match (width, height) {
        (Some(w), Some(h)) if h != 0 => w as f64 / h as f64,
        _ => 1.0,
    }
}
