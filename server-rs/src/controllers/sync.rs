use axum::{http::StatusCode, routing::{delete, get, post}, Json, Router};
use serde_json::json;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/full-sync", post(full_sync))
        .route("/delta-sync", post(delta_sync))
        .route("/stream", post(sync_stream))
        .route("/ack", get(get_acks).post(set_acks).delete(delete_acks))
}

async fn full_sync(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn delta_sync(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({"upserted":[],"deleted":[],"needsFullSync":false}))) }
async fn sync_stream(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn get_acks(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn set_acks(_auth: AuthDto) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn delete_acks(_auth: AuthDto) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
