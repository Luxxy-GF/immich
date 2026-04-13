use axum::{extract::Path, http::StatusCode, routing::{delete, get, put}, Json, Router};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_partners)).route("/:id", put(create_partner).delete(delete_partner))
}

async fn get_partners(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn create_partner(Path(_id): Path<String>) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(serde_json::json!({}))) }
async fn delete_partner(Path(_id): Path<String>) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
