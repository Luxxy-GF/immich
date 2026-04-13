use axum::{extract::Path, http::StatusCode, routing::{delete, get, post}, Json, Router};

use crate::{error::AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_trash))
        .route("/empty", delete(empty_trash))
        .route("/restore", post(restore_trash))
        .route("/:id", delete(delete_trashed_asset))
}

async fn get_trash() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn empty_trash() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn restore_trash() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn delete_trashed_asset(Path(_id): Path<String>) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
