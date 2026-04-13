use serde::{Deserialize, Serialize};
use super::asset::AssetResponseDto;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumResponseDto {
    pub id: String,
    pub owner_id: String,
    pub album_name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub album_thumbnail_asset_id: Option<String>,
    pub shared: bool,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_users: Option<Vec<serde_json::Value>>, // Placeholder for users
    
    pub has_shared_link: bool,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Vec<AssetResponseDto>>,
    
    pub asset_count: i32,
    pub is_activity_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAlbumDto {
    pub album_name: Option<String>,
    pub description: Option<String>,
    pub album_thumbnail_asset_id: Option<String>,
    pub is_activity_enabled: Option<bool>,
    pub order: Option<String>,
}
