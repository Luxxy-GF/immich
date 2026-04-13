use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetResponseDto {
    pub id: String,
    pub r#type: String, // AssetType (IMAGE, VIDEO, etc.)
    pub thumbhash: Option<String>,
    pub local_date_time: String,
    pub duration: String,
    pub has_metadata: bool,
    pub width: Option<i32>,
    pub height: Option<i32>,

    pub created_at: String,
    pub device_asset_id: String,
    pub device_id: String,
    pub owner_id: String,
    
    // owner: Option<UserResponseDto> can be added if needed
    pub original_path: String,
    pub original_file_name: String,
    pub file_created_at: String,
    pub file_modified_at: String,
    pub updated_at: String,
    
    pub is_favorite: bool,
    pub is_archived: bool,
    pub is_trashed: bool,
    pub is_offline: bool,
    pub visibility: String, // AssetVisibility
    pub checksum: String,
    
    pub is_edited: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetStatsResponseDto {
    pub total: i32,
    pub images: i32,
    pub videos: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomAssetsDto {
    pub count: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAssetDto {
    pub is_favorite: Option<bool>,
    pub is_archived: Option<bool>,
    pub description: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBulkDeleteDto {
    pub ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBulkUploadCheckItemDto {
    pub id: String,
    pub checksum: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBulkUploadCheckDto {
    pub assets: Vec<AssetBulkUploadCheckItemDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBulkUploadCheckResultDto {
    pub id: String,
    pub action: String,
    pub asset_id: Option<String>,
    pub is_trashed: Option<bool>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBulkUploadCheckResponseDto {
    pub results: Vec<AssetBulkUploadCheckResultDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMediaResponseDto {
    pub id: String,
    pub duplicate: bool,
    pub status: String,
}
