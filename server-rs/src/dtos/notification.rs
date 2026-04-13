use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDto {
    pub id: String,
    pub created_at: String,
    pub title: String,
    pub description: Option<String>,
    pub level: String,
    pub read_at: Option<String>,
    pub data: Option<serde_json::Value>,
    pub r#type: String,
}
