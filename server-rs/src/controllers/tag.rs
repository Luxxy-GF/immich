use axum::{extract::Path, http::StatusCode, routing::{delete, get, post, put}, Json, Router};
use serde_json::json;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_tags).post(create_tag))
        .route("/:id", get(get_tag).put(update_tag).delete(delete_tag))
        .route("/:id/assets", put(add_tag_assets).delete(remove_tag_assets))
}

async fn get_tags(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn create_tag() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn get_tag(Path(_id): Path<String>) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn update_tag(Path(_id): Path<String>) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn delete_tag(Path(_id): Path<String>) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn add_tag_assets(Path(_id): Path<String>) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn remove_tag_assets(Path(_id): Path<String>) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
