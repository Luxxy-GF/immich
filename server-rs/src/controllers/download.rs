use axum::{http::StatusCode, response::IntoResponse, routing::post, Router};

use crate::{error::AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/info", post(download_info)).route("/archive", post(download_archive))
}

async fn download_info() -> Result<impl IntoResponse, AppError> { Ok((StatusCode::OK, axum::Json(serde_json::json!({"archives":[]})))) }
async fn download_archive() -> Result<impl IntoResponse, AppError> { Ok(StatusCode::NOT_IMPLEMENTED) }
