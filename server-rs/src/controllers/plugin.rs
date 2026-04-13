use axum::{extract::Path, routing::get, Json, Router};
use serde_json::json;

use crate::{error::AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/triggers", get(get_plugin_triggers))
        .route("/", get(get_plugins))
        .route("/:id", get(get_plugin))
}

async fn get_plugin_triggers() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn get_plugins() -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn get_plugin(Path(_id): Path<String>) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
