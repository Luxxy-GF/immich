use axum::{extract::Path, http::StatusCode, routing::{delete, get, put}, Json, Router};
use serde_json::{json, Value};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

const QUEUE_NAMES: &[&str] = &[
    "thumbnailGeneration",
    "metadataExtraction",
    "videoConversion",
    "faceDetection",
    "facialRecognition",
    "smartSearch",
    "duplicateDetection",
    "backgroundTask",
    "storageTemplateMigration",
    "migration",
    "search",
    "sidecar",
    "library",
    "notifications",
    "backupDatabase",
    "ocr",
    "workflow",
    "editor",
];

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_queues))
        .route("/:name", get(get_queue).put(update_queue))
        .route("/:name/jobs", get(get_queue_jobs).delete(empty_queue))
}

async fn get_queues(_auth: AuthDto) -> Result<Json<Vec<Value>>, AppError> {
    Ok(Json(QUEUE_NAMES.iter().map(|name| queue_json(name, false)).collect()))
}

async fn get_queue(Path(name): Path<String>, _auth: AuthDto) -> Result<Json<Value>, AppError> {
    Ok(Json(queue_json(&name, false)))
}

async fn update_queue(Path(name): Path<String>, _auth: AuthDto, Json(payload): Json<Value>) -> Result<Json<Value>, AppError> {
    let is_paused = payload.get("isPaused").and_then(|v| v.as_bool()).unwrap_or(false);
    Ok(Json(queue_json(&name, is_paused)))
}

async fn get_queue_jobs(Path(name): Path<String>, _auth: AuthDto) -> Result<Json<Vec<Value>>, AppError> {
    Ok(Json(vec![
        json!({"id": format!("{name}-demo-1"), "name": "AssetGenerateThumbnails", "timestamp": chrono::Utc::now().timestamp_millis(), "data": {}}),
        json!({"id": format!("{name}-demo-2"), "name": "AssetExtractMetadata", "timestamp": chrono::Utc::now().timestamp_millis(), "data": {}}),
    ]))
}

async fn empty_queue(Path(_name): Path<String>, _auth: AuthDto) -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

fn queue_json(name: &str, is_paused: bool) -> Value {
    json!({
        "name": name,
        "isPaused": is_paused,
        "statistics": {
            "active": 0,
            "completed": 0,
            "delayed": 0,
            "failed": 0,
            "paused": if is_paused { 1 } else { 0 },
            "waiting": 0
        }
    })
}
