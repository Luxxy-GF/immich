use axum::{extract::{Path, State}, http::StatusCode, routing::{delete, get, put}, Json, Router};
use serde_json::{json, Value};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub(crate) const QUEUE_NAMES: &[&str] = &[
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

async fn get_queues(State(state): State<AppState>, _auth: AuthDto) -> Result<Json<Vec<Value>>, AppError> {
    let mut queues = Vec::new();
    for name in QUEUE_NAMES {
        let stats = state.job_queue.queue_stats(name).await;
        queues.push(queue_json(name, stats));
    }
    Ok(Json(queues))
}

async fn get_queue(State(state): State<AppState>, Path(name): Path<String>, _auth: AuthDto) -> Result<Json<Value>, AppError> {
    let stats = state.job_queue.queue_stats(&name).await;
    Ok(Json(queue_json(&name, stats)))
}

async fn update_queue(State(state): State<AppState>, Path(name): Path<String>, _auth: AuthDto, Json(payload): Json<Value>) -> Result<Json<Value>, AppError> {
    let is_paused = payload.get("isPaused").and_then(|v| v.as_bool()).unwrap_or(false);
    state.job_queue.set_paused(&name, is_paused).await;
    let stats = state.job_queue.queue_stats(&name).await;
    Ok(Json(queue_json(&name, stats)))
}

async fn get_queue_jobs(State(state): State<AppState>, Path(name): Path<String>, _auth: AuthDto) -> Result<Json<Vec<Value>>, AppError> {
    let jobs = state
        .job_queue
        .jobs_for_queue(&name)
        .await
        .into_iter()
        .map(|job| {
            json!({
                "id": job.id,
                "name": job.name,
                "queueName": job.queue,
                "timestamp": job.created_at,
                "data": job.data,
                "status": job.status,
                "startedAt": job.started_at,
                "completedAt": job.completed_at,
                "failedAt": job.failed_at,
                "error": job.error,
            })
        })
        .collect();
    Ok(Json(jobs))
}

async fn empty_queue(State(state): State<AppState>, Path(name): Path<String>, _auth: AuthDto) -> Result<StatusCode, AppError> {
    state.job_queue.clear_queue(&name).await;
    Ok(StatusCode::NO_CONTENT)
}

fn queue_json(name: &str, stats: crate::jobs::QueueStats) -> Value {
    json!({
        "name": name,
        "isPaused": stats.is_paused,
        "statistics": {
            "active": stats.active,
            "completed": stats.completed,
            "delayed": stats.delayed,
            "failed": stats.failed,
            "paused": stats.paused,
            "waiting": stats.waiting
        }
    })
}
