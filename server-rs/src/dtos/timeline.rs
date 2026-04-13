use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeBucketsResponseDto {
    pub count: i32,
    pub time_bucket: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeBucketAssetResponseDto {
    pub id: Vec<String>,
    pub owner_id: Vec<String>,
    pub file_created_at: Vec<String>,
    pub is_favorite: Vec<bool>,
    pub is_image: Vec<bool>,
    pub is_trashed: Vec<bool>,
    pub live_photo_video_id: Vec<Option<String>>,
    pub local_offset_hours: Vec<f64>,
    pub projection_type: Vec<Option<String>>,
    pub ratio: Vec<f64>,
    pub thumbhash: Vec<Option<String>>,
    pub duration: Vec<Option<String>>,
    pub city: Vec<Option<String>>,
    pub country: Vec<Option<String>>,
    pub visibility: Vec<String>,
    pub latitude: Option<Vec<Option<f64>>>,
    pub longitude: Option<Vec<Option<f64>>>,
    pub stack: Option<Vec<Option<Vec<String>>>>,
}
