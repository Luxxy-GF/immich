use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde_json::json;
use sqlx::Row;
use std::process::Command;

use crate::dtos::server::{
    ServerAboutResponseDto, ServerConfigDto, ServerFeaturesDto, ServerPingResponse,
    ServerStorageResponseDto, ServerVersionHistoryResponseDto, ServerVersionResponseDto,
};
use crate::error::AppError;
use crate::middleware::auth::AuthDto;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/about", get(about))
        .route("/apk-links", get(apk_links))
        .route("/ping", get(ping))
        .route("/version", get(version))
        .route("/version-history", get(version_history))
        .route("/config", get(config))
        .route("/features", get(features))
        .route("/storage", get(storage))
        .route("/media-types", get(media_types))
        .route("/theme", get(theme))
        .route("/statistics", get(statistics))
        .route("/license", get(get_license).put(set_license).delete(delete_license))
        .route("/version-check", get(version_check))
}

async fn ping() -> Result<Json<ServerPingResponse>, AppError> {
    Ok(Json(ServerPingResponse {
        res: "pong".to_string(),
    }))
}

async fn about(_auth: AuthDto) -> Result<Json<ServerAboutResponseDto>, AppError> {
    let version = immich_version();
    Ok(Json(ServerAboutResponseDto {
        version: format!("v{version}"),
        version_url: format!("https://github.com/immich-app/immich/releases/tag/v{version}"),
        repository: env_or("IMMICH_REPOSITORY", "immich-app/immich"),
        repository_url: env_or("IMMICH_REPOSITORY_URL", "https://github.com/immich-app/immich"),
        source_ref: env_optional("IMMICH_SOURCE_REF"),
        source_commit: env_optional("IMMICH_SOURCE_COMMIT"),
        source_url: env_optional("IMMICH_SOURCE_URL"),
        build: env_optional("IMMICH_BUILD"),
        build_url: env_optional("IMMICH_BUILD_URL"),
        build_image: env_optional("IMMICH_BUILD_IMAGE"),
        build_image_url: env_optional("IMMICH_BUILD_IMAGE_URL"),
        nodejs: None,
        ffmpeg: None,
        imagemagick: None,
        libvips: None,
        exiftool: None,
        licensed: false,
        third_party_source_url: env_optional("IMMICH_THIRD_PARTY_SOURCE_URL"),
        third_party_bug_feature_url: env_optional("IMMICH_THIRD_PARTY_BUG_FEATURE_URL"),
        third_party_documentation_url: env_optional("IMMICH_THIRD_PARTY_DOCUMENTATION_URL"),
        third_party_support_url: env_optional("IMMICH_THIRD_PARTY_SUPPORT_URL"),
    }))
}

async fn apk_links(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> {
    let version = immich_version();
    let base_url = format!("https://github.com/immich-app/immich/releases/download/v{version}");
    Ok(Json(json!({
        "arm64v8a": format!("{base_url}/app-arm64-v8a-release.apk"),
        "armeabiv7a": format!("{base_url}/app-armeabi-v7a-release.apk"),
        "universal": format!("{base_url}/app-release.apk"),
        "x86_64": format!("{base_url}/app-x86_64-release.apk"),
    })))
}

async fn version() -> Result<Json<ServerVersionResponseDto>, AppError> {
    // In Stage 1 we hardcode the version to match existing package.json or system version.
    Ok(Json(ServerVersionResponseDto {
        major: 1,
        minor: 108,
        patch: 0,
    }))
}

async fn version_history(
    State(state): State<AppState>,
) -> Result<Json<Vec<ServerVersionHistoryResponseDto>>, AppError> {
    let history = sqlx::query_as::<_, ServerVersionHistoryResponseDto>(
        r#"
        SELECT "id"::text as "id", "createdAt" as "created_at", version
        FROM version_history
        ORDER BY "createdAt" DESC
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(history))
}

async fn config(State(state): State<AppState>) -> Result<Json<ServerConfigDto>, AppError> {
    let has_admin: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM "user" WHERE "isAdmin" = true AND "deletedAt" IS NULL"#,
    )
    .fetch_one(&state.db)
    .await?;
    let onboarding = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT value FROM system_metadata WHERE key = 'admin-onboarding'"#,
    )
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(ServerConfigDto {
        oauth_button_text: "Login with OAuth".to_string(),
        login_page_message: "".to_string(),
        trash_days: 30,
        user_delete_delay: 7,
        is_initialized: has_admin > 0,
        is_onboarded: onboarding
            .as_ref()
            .and_then(|v| v.get("isOnboarded"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        external_domain: "".to_string(),
        public_users: false,
        map_dark_style_url: "".to_string(),
        map_light_style_url: "".to_string(),
        maintenance_mode: false,
    }))
}

async fn features() -> Result<Json<ServerFeaturesDto>, AppError> {
    // Matching system config features defaults
    Ok(Json(ServerFeaturesDto {
        smart_search: true,
        duplicate_detection: true,
        config_file: false,
        facial_recognition: true,
        map: true,
        trash: true,
        reverse_geocoding: true,
        import_faces: false,
        oauth: false,
        oauth_auto_launch: false,
        password_login: true,
        sidecar: true,
        search: true,
        email: false,
        ocr: true,
    }))
}

async fn storage(_auth: AuthDto) -> Result<Json<ServerStorageResponseDto>, AppError> {
    let output = Command::new("df")
        .args(["-B1", "--output=size,used,avail,pcent", "."])
        .output()
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    let stdout = String::from_utf8(output.stdout).map_err(|e| AppError::InternalServerError(e.into()))?;
    let line = stdout
        .lines()
        .nth(1)
        .ok_or_else(|| AppError::BadRequest("Unable to determine storage".to_string()))?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(AppError::BadRequest("Unable to determine storage".to_string()));
    }

    let disk_size_raw: i64 = parts[0].parse().map_err(|_| AppError::BadRequest("Unable to determine storage".to_string()))?;
    let disk_use_raw: i64 = parts[1].parse().map_err(|_| AppError::BadRequest("Unable to determine storage".to_string()))?;
    let disk_available_raw: i64 = parts[2].parse().map_err(|_| AppError::BadRequest("Unable to determine storage".to_string()))?;
    let disk_usage_percentage: f64 = parts[3]
        .trim_end_matches('%')
        .parse()
        .map_err(|_| AppError::BadRequest("Unable to determine storage".to_string()))?;

    Ok(Json(ServerStorageResponseDto {
        disk_size: human_bytes(disk_size_raw),
        disk_use: human_bytes(disk_use_raw),
        disk_available: human_bytes(disk_available_raw),
        disk_size_raw,
        disk_use_raw,
        disk_available_raw,
        disk_usage_percentage,
    }))
}

async fn statistics(State(state): State<AppState>, _auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT
            u.id::text as "userId",
            u.name as "userName",
            u."quotaSizeInBytes" as "quotaSizeInBytes",
            COALESCE(COUNT(*) FILTER (WHERE a.type = 'IMAGE' AND a.visibility != 'hidden' AND a."deletedAt" IS NULL), 0)::bigint as photos,
            COALESCE(COUNT(*) FILTER (WHERE a.type = 'VIDEO' AND a.visibility != 'hidden' AND a."deletedAt" IS NULL), 0)::bigint as videos,
            COALESCE(SUM(ex."fileSizeInByte") FILTER (WHERE a."deletedAt" IS NULL), 0)::bigint as usage,
            COALESCE(SUM(ex."fileSizeInByte") FILTER (WHERE a.type = 'IMAGE' AND a."deletedAt" IS NULL), 0)::bigint as "usagePhotos",
            COALESCE(SUM(ex."fileSizeInByte") FILTER (WHERE a.type = 'VIDEO' AND a."deletedAt" IS NULL), 0)::bigint as "usageVideos"
        FROM "user" u
        LEFT JOIN "asset" a ON a."ownerId" = u.id
        LEFT JOIN asset_exif ex ON ex."assetId" = a.id
        WHERE u."deletedAt" IS NULL
        GROUP BY u.id, u.name, u."quotaSizeInBytes"
        ORDER BY u.name ASC
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    let mut usage_by_user = Vec::new();
    let mut photos = 0i64;
    let mut videos = 0i64;
    let mut usage = 0i64;
    let mut usage_photos = 0i64;
    let mut usage_videos = 0i64;

    for row in rows {
        let user_photos: i64 = row.try_get("photos").unwrap_or(0);
        let user_videos: i64 = row.try_get("videos").unwrap_or(0);
        let user_usage: i64 = row.try_get("usage").unwrap_or(0);
        let user_usage_photos: i64 = row.try_get("usagePhotos").unwrap_or(0);
        let user_usage_videos: i64 = row.try_get("usageVideos").unwrap_or(0);
        let user_id: String = row.try_get("userId").unwrap_or_default();
        let user_name: String = row.try_get("userName").unwrap_or_default();
        let quota_size_in_bytes: Option<i64> = row.try_get("quotaSizeInBytes").ok();

        photos += user_photos;
        videos += user_videos;
        usage += user_usage;
        usage_photos += user_usage_photos;
        usage_videos += user_usage_videos;

        usage_by_user.push(json!({
            "userId": user_id,
            "userName": user_name,
            "photos": user_photos,
            "videos": user_videos,
            "usage": user_usage,
            "usagePhotos": user_usage_photos,
            "usageVideos": user_usage_videos,
            "quotaSizeInBytes": quota_size_in_bytes,
        }));
    }

    Ok(Json(json!({
        "photos": photos,
        "videos": videos,
        "usage": usage,
        "usagePhotos": usage_photos,
        "usageVideos": usage_videos,
        "usageByUser": usage_by_user,
    })))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerMediaTypesResponseDto {
    pub video: Vec<String>,
    pub image: Vec<String>,
    pub sidecar: Vec<String>,
}

async fn media_types() -> Result<Json<ServerMediaTypesResponseDto>, AppError> {
    Ok(Json(ServerMediaTypesResponseDto {
        video: vec![".mp4".into(), ".webm".into(), ".mov".into(), ".mkv".into()],
        image: vec![".jpg".into(), ".jpeg".into(), ".png".into(), ".heic".into(), ".webp".into(), ".gif".into()],
        sidecar: vec![".xmp".into()],
    }))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerThemeResponseDto {
    pub custom_css: String,
}

async fn theme(
    State(state): State<AppState>,
) -> Result<Json<ServerThemeResponseDto>, AppError> {
    Ok(Json(ServerThemeResponseDto {
        custom_css: load_custom_css(&state).await.unwrap_or_default(),
    }))
}

async fn get_license(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(json!({})))
}

async fn set_license(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(json!({})))
}

async fn delete_license(_auth: AuthDto) -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn version_check(State(state): State<AppState>, _auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> {
    let value = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT value FROM system_metadata WHERE key = 'version-check-state'"#,
    )
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(value.unwrap_or_else(|| json!({"checkedAt": null, "releaseVersion": null}))))
}

fn env_optional(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn env_or(key: &str, fallback: &str) -> Option<String> {
    Some(env_optional(key).unwrap_or_else(|| fallback.to_string()))
}

fn immich_version() -> String {
    serde_json::from_str::<serde_json::Value>(include_str!("../../../server/package.json"))
        .ok()
        .and_then(|package| package.get("version").and_then(|value| value.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "2.7.4".to_string())
}

async fn load_custom_css(state: &AppState) -> Result<String, AppError> {
    let value = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM system_metadata
        WHERE key = 'system-config'
        "#,
    )
    .fetch_optional(&state.db)
    .await?;

    Ok(value
        .as_ref()
        .and_then(|value| value.get("theme"))
        .and_then(|value| value.get("customCss"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string())
}

fn human_bytes(value: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = value as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", value, UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}
