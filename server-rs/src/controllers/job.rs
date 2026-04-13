use axum::{extract::{Path, State}, http::StatusCode, routing::{get, post, put}, Json, Router};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_queues_legacy).post(create_job))
        .route("/:name", put(run_legacy_queue))
}

async fn get_queues_legacy(State(state): State<AppState>, _auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> {
    let mut result = Vec::new();
    for name in crate::controllers::queue::QUEUE_NAMES {
        let stats = state.job_queue.queue_stats(name).await;
        result.push(serde_json::json!({
            "name": name,
            "active": stats.active,
            "waiting": stats.waiting,
            "completed": stats.completed,
            "failed": stats.failed,
            "delayed": stats.delayed,
            "paused": stats.paused,
            "isPaused": stats.is_paused,
        }));
    }
    Ok(Json(serde_json::json!({ "queues": result })))
}

async fn create_job() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn run_legacy_queue(Path(_name): Path<String>) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::json!({ "success": true })))
}
