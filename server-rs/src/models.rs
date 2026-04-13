use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    #[sqlx(rename = "avatarColor")]
    pub avatar_color: Option<String>,
    #[sqlx(rename = "profileImagePath")]
    pub profile_image_path: String,
    #[sqlx(rename = "profileChangedAt")]
    pub profile_changed_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "storageLabel")]
    pub storage_label: Option<String>,
    #[sqlx(rename = "shouldChangePassword")]
    pub should_change_password: bool,
    #[sqlx(rename = "isAdmin")]
    pub is_admin: bool,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[sqlx(rename = "deletedAt")]
    pub deleted_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "oauthId")]
    pub oauth_id: String,
    #[sqlx(rename = "quotaSizeInBytes")]
    pub quota_size_in_bytes: Option<i64>,
    #[sqlx(rename = "quotaUsageInBytes")]
    pub quota_usage_in_bytes: i64,
    pub status: String,
    
    // Auth specific fields (hidden from default user columns usually)
    pub password: Option<String>,
    #[sqlx(rename = "pinCode")]
    pub pin_code: Option<String>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct Session {
    pub id: String,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[sqlx(rename = "expiresAt")]
    pub expires_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "deviceOS")]
    pub device_os: String,
    #[sqlx(rename = "deviceType")]
    pub device_type: String,
    #[sqlx(rename = "appVersion")]
    pub app_version: Option<String>,
    #[sqlx(rename = "pinExpiresAt")]
    pub pin_expires_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "isPendingSyncReset")]
    pub is_pending_sync_reset: bool,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    pub token: Vec<u8>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct Album {
    pub id: String,
    #[sqlx(rename = "ownerId")]
    pub owner_id: String,
    #[sqlx(rename = "albumName")]
    pub album_name: String,
    pub description: String,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[sqlx(rename = "albumThumbnailAssetId")]
    pub album_thumbnail_asset_id: Option<String>,
    #[sqlx(rename = "isActivityEnabled")]
    pub is_activity_enabled: bool,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct Asset {
    pub id: String,
    pub r#type: String,
    #[sqlx(rename = "deviceAssetId")]
    pub device_asset_id: String,
    #[sqlx(rename = "ownerId")]
    pub owner_id: String,
    #[sqlx(rename = "deviceId")]
    pub device_id: String,
    #[sqlx(rename = "localDateTime")]
    pub local_date_time: DateTime<Utc>,
    #[sqlx(rename = "fileCreatedAt")]
    pub file_created_at: DateTime<Utc>,
    #[sqlx(rename = "fileModifiedAt")]
    pub file_modified_at: DateTime<Utc>,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[sqlx(rename = "originalPath")]
    pub original_path: String,
    #[sqlx(rename = "originalFileName")]
    pub original_file_name: String,
    #[sqlx(rename = "isFavorite")]
    pub is_favorite: bool,
    #[sqlx(rename = "isOffline")]
    pub is_offline: bool,
    #[sqlx(rename = "deletedAt")]
    pub deleted_at: Option<DateTime<Utc>>,
    pub checksum: Vec<u8>,
    pub thumbhash: Option<Vec<u8>>,
    #[sqlx(rename = "livePhotoVideoId")]
    pub live_photo_video_id: Option<String>,
    pub duration: Option<String>,
    pub visibility: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct Memory {
    pub id: String,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[sqlx(rename = "deletedAt")]
    pub deleted_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "ownerId")]
    pub owner_id: String,
    pub r#type: String,
    pub data: serde_json::Value,
    #[sqlx(rename = "isSaved")]
    pub is_saved: bool,
    #[sqlx(rename = "memoryAt")]
    pub memory_at: DateTime<Utc>,
    #[sqlx(rename = "seenAt")]
    pub seen_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "showAt")]
    pub show_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "hideAt")]
    pub hide_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct Notification {
    pub id: String,
    #[sqlx(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    #[sqlx(rename = "deletedAt")]
    pub deleted_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "userId")]
    pub user_id: Option<String>,
    pub level: String,
    pub r#type: String,
    pub data: Option<serde_json::Value>,
    pub title: String,
    pub description: Option<String>,
    #[sqlx(rename = "readAt")]
    pub read_at: Option<DateTime<Utc>>,
}
