use axum::{extract::Path, http::StatusCode, routing::{delete, get, post}, Json, Router};

use crate::{error::AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_views).post(create_view)).route("/:id", delete(delete_view))
}

async fn get_views() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn create_view() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(serde_json::json!({}))) }
async fn delete_view(Path(_id): Path<String>) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
