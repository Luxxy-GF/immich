use axum::{extract::Path, http::StatusCode, routing::{delete, get, post, put}, Json, Router};
use serde_json::json;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_workflow).get(get_workflows))
        .route("/:id", get(get_workflow).put(update_workflow).delete(delete_workflow))
}

async fn create_workflow(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn get_workflows(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn get_workflow(Path(_id): Path<String>, _auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn update_workflow(Path(_id): Path<String>, _auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn delete_workflow(Path(_id): Path<String>, _auth: AuthDto) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
