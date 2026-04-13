use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPingResponse {
    pub res: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerVersionResponseDto {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ServerVersionHistoryResponseDto {
    pub id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAboutResponseDto {
    pub version: String,
    pub version_url: String,
    pub repository: Option<String>,
    pub repository_url: Option<String>,
    pub source_ref: Option<String>,
    pub source_commit: Option<String>,
    pub source_url: Option<String>,
    pub build: Option<String>,
    pub build_url: Option<String>,
    pub build_image: Option<String>,
    pub build_image_url: Option<String>,
    pub nodejs: Option<String>,
    pub ffmpeg: Option<String>,
    pub imagemagick: Option<String>,
    pub libvips: Option<String>,
    pub exiftool: Option<String>,
    pub licensed: bool,
    pub third_party_source_url: Option<String>,
    pub third_party_bug_feature_url: Option<String>,
    pub third_party_documentation_url: Option<String>,
    pub third_party_support_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigDto {
    pub oauth_button_text: String,
    pub login_page_message: String,
    pub trash_days: u32,
    pub user_delete_delay: u32,
    pub is_initialized: bool,
    pub is_onboarded: bool,
    pub external_domain: String,
    pub public_users: bool,
    pub map_dark_style_url: String,
    pub map_light_style_url: String,
    pub maintenance_mode: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStorageResponseDto {
    pub disk_size: String,
    pub disk_use: String,
    pub disk_available: String,
    pub disk_size_raw: i64,
    pub disk_use_raw: i64,
    pub disk_available_raw: i64,
    pub disk_usage_percentage: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerFeaturesDto {
    pub smart_search: bool,
    pub duplicate_detection: bool,
    pub config_file: bool,
    pub facial_recognition: bool,
    pub map: bool,
    pub trash: bool,
    pub reverse_geocoding: bool,
    pub import_faces: bool,
    pub oauth: bool,
    pub oauth_auto_launch: bool,
    pub password_login: bool,
    pub sidecar: bool,
    pub search: bool,
    pub email: bool,
    pub ocr: bool,
}
