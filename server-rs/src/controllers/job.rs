use axum::{extract::Path, http::StatusCode, routing::{get, post, put}, Json, Router};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_queues_legacy).post(create_job)).route("/:name", put(run_legacy_queue))
}

async fn get_queues_legacy(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(serde_json::json!({"queues": []}))) }
async fn create_job() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn run_legacy_queue(Path(_name): Path<String>) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(serde_json::json!({}))) }
