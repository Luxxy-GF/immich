use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/metadata", post(search_metadata))
        .route("/statistics", post(search_statistics))
        .route("/random", post(search_random))
        .route("/large-assets", post(search_large_assets))
        .route("/smart", post(search_smart))
        .route("/explore", get(get_explore))
        .route("/person", get(search_person))
        .route("/places", get(search_places))
        .route("/cities", get(get_assets_by_city))
        .route("/suggestions", get(get_suggestions))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SearchQuery {
    size: Option<i64>,
    page: Option<i64>,
    city: Option<String>,
    country: Option<String>,
    state: Option<String>,
    make: Option<String>,
    model: Option<String>,
    lens_model: Option<String>,
    is_favorite: Option<bool>,
    visibility: Option<String>,
    name: Option<String>,
    r#type: Option<String>,
}

async fn search_metadata(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(query): Json<SearchQuery>,
) -> Result<Json<Value>, AppError> {
    let items = search_assets_internal(&state, &auth.user.id, &query, query.size.unwrap_or(250), query.page.unwrap_or(1)).await?;
    let total = count_assets_internal(&state, &auth.user.id, &query).await?;
    Ok(Json(json!({
        "albums": { "total": 0, "count": 0, "items": [], "facets": [] },
        "assets": {
            "total": total,
            "count": items.len(),
            "items": items,
            "facets": [],
            "nextPage": if total > query.page.unwrap_or(1) * query.size.unwrap_or(250) { json!((query.page.unwrap_or(1) + 1).to_string()) } else { Value::Null }
        }
    })))
}

async fn search_statistics(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(query): Json<SearchQuery>,
) -> Result<Json<Value>, AppError> {
    let total = count_assets_internal(&state, &auth.user.id, &query).await?;
    Ok(Json(json!({ "total": total })))
}

async fn search_random(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(query): Json<SearchQuery>,
) -> Result<Json<Vec<Value>>, AppError> {
    let size = query.size.unwrap_or(250);
    let rows = sqlx::query(
        r#"
        SELECT
            a."id"::text as id,
            a."type",
            a.thumbhash,
            a."localDateTime",
            a.duration,
            a.width,
            a.height,
            a."createdAt",
            a."deviceAssetId",
            a."deviceId",
            a."ownerId"::text as "ownerId",
            a."originalPath",
            a."originalFileName",
            a."fileCreatedAt",
            a."fileModifiedAt",
            a."updatedAt",
            a."isFavorite",
            a."deletedAt",
            a."isOffline",
            a.visibility::text as visibility,
            a.checksum
        FROM asset a
        LEFT JOIN asset_exif ex ON ex."assetId" = a.id
        WHERE a."ownerId" = $1::uuid
          AND a."deletedAt" IS NULL
        ORDER BY random()
        LIMIT $2
        "#,
    )
    .bind(&auth.user.id)
    .bind(size)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(asset_row_to_json).collect()))
}

async fn search_large_assets(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<Value>>, AppError> {
    let size = query.size.unwrap_or(250);
    let rows = sqlx::query(
        r#"
        SELECT
            a."id"::text as id,
            a."type",
            a.thumbhash,
            a."localDateTime",
            a.duration,
            a.width,
            a.height,
            a."createdAt",
            a."deviceAssetId",
            a."deviceId",
            a."ownerId"::text as "ownerId",
            a."originalPath",
            a."originalFileName",
            a."fileCreatedAt",
            a."fileModifiedAt",
            a."updatedAt",
            a."isFavorite",
            a."deletedAt",
            a."isOffline",
            a.visibility::text as visibility,
            a.checksum
        FROM asset a
        JOIN asset_exif ex ON ex."assetId" = a.id
        WHERE a."ownerId" = $1::uuid
          AND a."deletedAt" IS NULL
        ORDER BY ex."fileSizeInByte" DESC NULLS LAST
        LIMIT $2
        "#,
    )
    .bind(&auth.user.id)
    .bind(size)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(asset_row_to_json).collect()))
}

async fn search_smart(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(query): Json<SearchQuery>,
) -> Result<Json<Value>, AppError> {
    search_metadata(State(state), auth, Json(query)).await
}

async fn get_explore(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT ex.city, a."id"::text as asset_id
        FROM asset a
        JOIN asset_exif ex ON ex."assetId" = a.id
        WHERE a."ownerId" = $1::uuid
          AND a."deletedAt" IS NULL
          AND ex.city IS NOT NULL
        ORDER BY a."fileCreatedAt" DESC
        LIMIT 12
        "#,
    )
    .bind(&auth.user.id)
    .fetch_all(&state.db)
    .await?;

    let items = rows.into_iter().filter_map(|row| {
        let city: Option<String> = row.try_get("city").ok();
        let asset_id: Option<String> = row.try_get("asset_id").ok();
        match (city, asset_id) {
            (Some(value), Some(id)) => Some(json!({"fieldName": "city", "items": [{ "value": value, "data": { "id": id } }]})),
            _ => None,
        }
    }).collect();
    Ok(Json(items))
}

async fn search_person(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT id::text as id, name, "thumbnailPath", "isHidden", "isFavorite", color, "birthDate", "updatedAt"
        FROM person
        WHERE "ownerId" = $1::uuid
          AND ($2::text IS NULL OR name ILIKE ('%' || $2 || '%'))
        ORDER BY name ASC
        LIMIT 100
        "#,
    )
    .bind(&auth.user.id)
    .bind(query.name)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(|row| json!({
        "id": row.try_get::<String,_>("id").unwrap_or_default(),
        "name": row.try_get::<String,_>("name").unwrap_or_default(),
        "thumbnailPath": row.try_get::<String,_>("thumbnailPath").unwrap_or_default(),
        "isHidden": row.try_get::<bool,_>("isHidden").unwrap_or(false),
        "isFavorite": row.try_get::<bool,_>("isFavorite").unwrap_or(false),
        "color": row.try_get::<Option<String>,_>("color").unwrap_or(None),
        "birthDate": row.try_get::<Option<chrono::NaiveDate>,_>("birthDate").ok().flatten().map(|d| d.to_string()),
        "updatedAt": row.try_get::<chrono::DateTime<chrono::Utc>,_>("updatedAt").ok().map(|d| d.to_rfc3339()),
    })).collect()))
}

async fn search_places(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT name, latitude, longitude, "admin1Name", "admin2Name"
        FROM geodata_places
        WHERE ($1::text IS NULL OR name ILIKE ($1 || '%'))
        ORDER BY name ASC
        LIMIT 25
        "#,
    )
    .bind(query.name)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(|row| json!({
        "name": row.try_get::<String,_>("name").unwrap_or_default(),
        "latitude": row.try_get::<f64,_>("latitude").unwrap_or(0.0),
        "longitude": row.try_get::<f64,_>("longitude").unwrap_or(0.0),
        "admin1name": row.try_get::<Option<String>,_>("admin1Name").unwrap_or(None),
        "admin2name": row.try_get::<Option<String>,_>("admin2Name").unwrap_or(None),
    })).collect()))
}

async fn get_assets_by_city(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<Vec<Value>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (ex.city)
            a."id"::text as id,
            a."type",
            a.thumbhash,
            a."localDateTime",
            a.duration,
            a.width,
            a.height,
            a."createdAt",
            a."deviceAssetId",
            a."deviceId",
            a."ownerId"::text as "ownerId",
            a."originalPath",
            a."originalFileName",
            a."fileCreatedAt",
            a."fileModifiedAt",
            a."updatedAt",
            a."isFavorite",
            a."deletedAt",
            a."isOffline",
            a.visibility::text as visibility,
            a.checksum
        FROM asset a
        JOIN asset_exif ex ON ex."assetId" = a.id
        WHERE a."ownerId" = $1::uuid
          AND a."deletedAt" IS NULL
          AND ex.city IS NOT NULL
        ORDER BY ex.city, a."fileCreatedAt" DESC
        "#,
    )
    .bind(&auth.user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(asset_row_to_json).collect()))
}

async fn get_suggestions(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<String>>, AppError> {
    let column = match query.r#type.as_deref().unwrap_or("city") {
        "country" => "country",
        "state" => "state",
        "camera-make" => "make",
        "camera-model" => "model",
        "camera-lens-model" => "lensModel",
        _ => "city",
    };
    let sql = format!(
        r#"
        SELECT DISTINCT ex."{column}"::text as value
        FROM asset_exif ex
        JOIN asset a ON a.id = ex."assetId"
        WHERE a."ownerId" = $1::uuid
          AND ex."{column}" IS NOT NULL
        ORDER BY value ASC
        LIMIT 50
        "#
    );
    let rows = sqlx::query(&sql)
        .bind(&auth.user.id)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().filter_map(|row| row.try_get::<String,_>("value").ok()).collect()))
}

async fn search_assets_internal(
    state: &AppState,
    user_id: &str,
    query: &SearchQuery,
    size: i64,
    page: i64,
) -> Result<Vec<Value>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            a."id"::text as id,
            a."type",
            a.thumbhash,
            a."localDateTime",
            a.duration,
            a.width,
            a.height,
            a."createdAt",
            a."deviceAssetId",
            a."deviceId",
            a."ownerId"::text as "ownerId",
            a."originalPath",
            a."originalFileName",
            a."fileCreatedAt",
            a."fileModifiedAt",
            a."updatedAt",
            a."isFavorite",
            a."deletedAt",
            a."isOffline",
            a.visibility::text as visibility,
            a.checksum
        FROM asset a
        LEFT JOIN asset_exif ex ON ex."assetId" = a.id
        WHERE a."ownerId" = $1::uuid
          AND a."deletedAt" IS NULL
          AND ($2::text IS NULL OR ex.city = $2)
          AND ($3::text IS NULL OR ex.country = $3)
          AND ($4::text IS NULL OR ex.state = $4)
          AND ($5::text IS NULL OR ex.make = $5)
          AND ($6::text IS NULL OR ex.model = $6)
          AND ($7::text IS NULL OR ex."lensModel" = $7)
          AND ($8::bool IS NULL OR a."isFavorite" = $8)
          AND ($9::text IS NULL OR a.visibility::text = $9)
          AND ($10::text IS NULL OR a.type = $10)
        ORDER BY a."fileCreatedAt" DESC
        LIMIT $11 OFFSET $12
        "#,
    )
    .bind(user_id)
    .bind(&query.city)
    .bind(&query.country)
    .bind(&query.state)
    .bind(&query.make)
    .bind(&query.model)
    .bind(&query.lens_model)
    .bind(query.is_favorite)
    .bind(&query.visibility)
    .bind(&query.r#type)
    .bind(size)
    .bind((page - 1) * size)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.into_iter().map(asset_row_to_json).collect())
}

async fn count_assets_internal(state: &AppState, user_id: &str, query: &SearchQuery) -> Result<i64, AppError> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM asset a
        LEFT JOIN asset_exif ex ON ex."assetId" = a.id
        WHERE a."ownerId" = $1::uuid
          AND a."deletedAt" IS NULL
          AND ($2::text IS NULL OR ex.city = $2)
          AND ($3::text IS NULL OR ex.country = $3)
          AND ($4::text IS NULL OR ex.state = $4)
          AND ($5::text IS NULL OR ex.make = $5)
          AND ($6::text IS NULL OR ex.model = $6)
          AND ($7::text IS NULL OR ex."lensModel" = $7)
          AND ($8::bool IS NULL OR a."isFavorite" = $8)
          AND ($9::text IS NULL OR a.visibility::text = $9)
          AND ($10::text IS NULL OR a.type = $10)
        "#,
    )
    .bind(user_id)
    .bind(&query.city)
    .bind(&query.country)
    .bind(&query.state)
    .bind(&query.make)
    .bind(&query.model)
    .bind(&query.lens_model)
    .bind(query.is_favorite)
    .bind(&query.visibility)
    .bind(&query.r#type)
    .fetch_one(&state.db)
    .await?;
    Ok(count)
}

fn asset_row_to_json(row: sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.try_get::<String,_>("id").unwrap_or_default(),
        "type": row.try_get::<String,_>("type").unwrap_or_default(),
        "thumbhash": row.try_get::<Option<Vec<u8>>,_>("thumbhash").ok().flatten().map(|b| STANDARD.encode(b)),
        "localDateTime": row.try_get::<chrono::DateTime<chrono::Utc>,_>("localDateTime").ok().map(|d| d.to_rfc3339()),
        "duration": row.try_get::<Option<String>,_>("duration").unwrap_or(None).unwrap_or_else(|| "0:00:00.00000".to_string()),
        "hasMetadata": true,
        "width": row.try_get::<Option<i32>,_>("width").unwrap_or(None),
        "height": row.try_get::<Option<i32>,_>("height").unwrap_or(None),
        "createdAt": row.try_get::<chrono::DateTime<chrono::Utc>,_>("createdAt").ok().map(|d| d.to_rfc3339()),
        "deviceAssetId": row.try_get::<String,_>("deviceAssetId").unwrap_or_default(),
        "deviceId": row.try_get::<String,_>("deviceId").unwrap_or_default(),
        "ownerId": row.try_get::<String,_>("ownerId").unwrap_or_default(),
        "originalPath": row.try_get::<String,_>("originalPath").unwrap_or_default(),
        "originalFileName": row.try_get::<String,_>("originalFileName").unwrap_or_default(),
        "fileCreatedAt": row.try_get::<chrono::DateTime<chrono::Utc>,_>("fileCreatedAt").ok().map(|d| d.to_rfc3339()),
        "fileModifiedAt": row.try_get::<chrono::DateTime<chrono::Utc>,_>("fileModifiedAt").ok().map(|d| d.to_rfc3339()),
        "updatedAt": row.try_get::<chrono::DateTime<chrono::Utc>,_>("updatedAt").ok().map(|d| d.to_rfc3339()),
        "isFavorite": row.try_get::<bool,_>("isFavorite").unwrap_or(false),
        "isArchived": row.try_get::<String,_>("visibility").unwrap_or_default() == "archive",
        "isTrashed": row.try_get::<Option<chrono::DateTime<chrono::Utc>>,_>("deletedAt").unwrap_or(None).is_some(),
        "isOffline": row.try_get::<bool,_>("isOffline").unwrap_or(false),
        "visibility": row.try_get::<String,_>("visibility").unwrap_or_default(),
        "checksum": row.try_get::<Vec<u8>,_>("checksum").ok().map(|b| STANDARD.encode(b)).unwrap_or_default(),
        "isEdited": false
    })
}
