use axum::{extract::Path, http::StatusCode, routing::{delete, get, post, put}, Json, Router};
use serde_json::json;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(search_stacks).post(create_stack).delete(delete_stacks))
        .route("/:id", get(get_stack).put(update_stack).delete(delete_stack))
        .route("/:id/assets/:asset_id", delete(remove_asset_from_stack))
}

async fn search_stacks(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn create_stack() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn delete_stacks() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn get_stack(Path(_id): Path<String>) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn update_stack(Path(_id): Path<String>) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn delete_stack(Path(_id): Path<String>) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn remove_asset_from_stack(Path((_id, _asset_id)): Path<(String, String)>) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
