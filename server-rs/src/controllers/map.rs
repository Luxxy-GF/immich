use axum::{routing::get, Json, Router};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/markers", get(get_markers)).route("/reverse-geocode", get(reverse_geocode))
}

async fn get_markers(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
async fn reverse_geocode(_auth: AuthDto) -> Result<Json<Vec<serde_json::Value>>, AppError> { Ok(Json(vec![])) }
