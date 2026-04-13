use axum::{http::StatusCode, routing::post, Router};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/unlink-all", post(unlink_all))
}

async fn unlink_all(_auth: AuthDto) -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
