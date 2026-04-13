use serde::Serialize;

use crate::dtos::asset::AssetResponseDto;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryResponseDto {
    pub id: String,
    pub owner_id: String,
    pub memory_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_saved: bool,
    pub data: MemoryOnThisDayDto,
    pub assets: Vec<AssetResponseDto>,
    pub r#type: String,
    pub deleted_at: Option<String>,
    pub hide_at: Option<String>,
    pub seen_at: Option<String>,
    pub show_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOnThisDayDto {
    pub year: i32,
}
