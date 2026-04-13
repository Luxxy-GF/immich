use axum::{extract::Path, http::StatusCode, routing::{delete, get, post}, Json, Router};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_duplicates).delete(delete_duplicates))
        .route("/resolve", post(resolve_duplicates))
        .route("/:id", delete(delete_duplicate))
}

async fn get_duplicates(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn delete_duplicates(_auth: AuthDto) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn resolve_duplicates(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn delete_duplicate(Path(_id): Path<String>, _auth: AuthDto) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
