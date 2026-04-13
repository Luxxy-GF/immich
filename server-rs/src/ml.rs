use std::time::Duration;

use reqwest::multipart::{Form, Part};
use serde_json::Value;

use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct MlConfig {
    pub enabled: bool,
    pub urls: Vec<String>,
    pub clip_enabled: bool,
    pub clip_model_name: String,
    pub facial_enabled: bool,
    pub facial_model_name: String,
    pub facial_min_score: f32,
    pub ocr_enabled: bool,
    pub ocr_model_name: String,
    pub ocr_min_detection_score: f32,
    pub ocr_min_recognition_score: f32,
    pub ocr_max_resolution: u32,
}

impl Default for MlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            urls: vec![std::env::var("IMMICH_MACHINE_LEARNING_URL").unwrap_or_else(|_| {
                "http://immich-machine-learning:3003".to_string()
            })],
            clip_enabled: true,
            clip_model_name: "ViT-B-32__openai".to_string(),
            facial_enabled: true,
            facial_model_name: "buffalo_l".to_string(),
            facial_min_score: 0.7,
            ocr_enabled: true,
            ocr_model_name: "PP-OCRv5_mobile".to_string(),
            ocr_min_detection_score: 0.5,
            ocr_min_recognition_score: 0.8,
            ocr_max_resolution: 736,
        }
    }
}

pub async fn load_ml_config(state: &AppState) -> Result<MlConfig, AppError> {
    let value = sqlx::query_scalar::<_, Value>(
        r#"SELECT value FROM system_metadata WHERE key = 'system-config'"#,
    )
    .fetch_optional(&state.db)
    .await?;

    let mut config = MlConfig::default();
    if let Some(value) = value {
        if let Some(ml) = value.get("machineLearning") {
            if let Some(enabled) = ml.get("enabled").and_then(|v| v.as_bool()) {
                config.enabled = enabled;
            }
            if let Some(urls) = ml.get("urls").and_then(|v| v.as_array()) {
                config.urls = urls
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
            }
            if let Some(clip) = ml.get("clip") {
                if let Some(enabled) = clip.get("enabled").and_then(|v| v.as_bool()) {
                    config.clip_enabled = enabled;
                }
                if let Some(model) = clip.get("modelName").and_then(|v| v.as_str()) {
                    config.clip_model_name = model.to_string();
                }
            }
            if let Some(face) = ml.get("facialRecognition") {
                if let Some(enabled) = face.get("enabled").and_then(|v| v.as_bool()) {
                    config.facial_enabled = enabled;
                }
                if let Some(model) = face.get("modelName").and_then(|v| v.as_str()) {
                    config.facial_model_name = model.to_string();
                }
                if let Some(score) = face.get("minScore").and_then(|v| v.as_f64()) {
                    config.facial_min_score = score as f32;
                }
            }
            if let Some(ocr) = ml.get("ocr") {
                if let Some(enabled) = ocr.get("enabled").and_then(|v| v.as_bool()) {
                    config.ocr_enabled = enabled;
                }
                if let Some(model) = ocr.get("modelName").and_then(|v| v.as_str()) {
                    config.ocr_model_name = model.to_string();
                }
                if let Some(score) = ocr.get("minDetectionScore").and_then(|v| v.as_f64()) {
                    config.ocr_min_detection_score = score as f32;
                }
                if let Some(score) = ocr.get("minRecognitionScore").and_then(|v| v.as_f64()) {
                    config.ocr_min_recognition_score = score as f32;
                }
                if let Some(max) = ocr.get("maxResolution").and_then(|v| v.as_u64()) {
                    config.ocr_max_resolution = max as u32;
                }
            }
        }
    }

    if config.urls.is_empty() {
        config.urls = MlConfig::default().urls;
    }

    Ok(config)
}

pub async fn predict_image(state: &AppState, entries: Value, image_path: &str) -> Result<Value, AppError> {
    let image_bytes = tokio::fs::read(image_path).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    predict(state, entries.to_string(), Payload::Image(image_bytes)).await
}

pub async fn predict_text(state: &AppState, entries: Value, text: &str) -> Result<Value, AppError> {
    predict(state, entries.to_string(), Payload::Text(text.to_string())).await
}

enum Payload {
    Image(Vec<u8>),
    Text(String),
}

async fn predict(state: &AppState, entries: String, payload: Payload) -> Result<Value, AppError> {
    let config = load_ml_config(state).await?;
    if !config.enabled {
        return Err(AppError::BadRequest("Machine learning disabled".to_string()));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| AppError::InternalServerError(e.into()))?;

    let mut last_error = None;
    for url in &config.urls {
        let endpoint = format!("{}/predict", url.trim_end_matches('/'));
        let mut form = Form::new().text("entries", entries.clone());
        match &payload {
            Payload::Image(bytes) => {
                let part = Part::bytes(bytes.clone()).file_name("image");
                form = form.part("image", part);
            }
            Payload::Text(text) => {
                form = form.text("text", text.clone());
            }
        }
        match client.post(endpoint).multipart(form).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let json = response.json::<Value>().await.map_err(|e| AppError::InternalServerError(e.into()))?;
                    return Ok(json);
                }
                last_error = Some(format!("ML request failed with {}", response.status()));
            }
            Err(err) => last_error = Some(err.to_string()),
        }
    }

    Err(AppError::BadRequest(
        last_error.unwrap_or_else(|| "Machine learning request failed".to_string()),
    ))
}
