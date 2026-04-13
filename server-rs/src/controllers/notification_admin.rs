use axum::{extract::Path, http::StatusCode, routing::post, Json, Router};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_notification))
        .route("/test-email", post(test_email))
        .route("/templates/:name", post(render_template))
}

async fn create_notification(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(serde_json::json!({}))) }
async fn test_email(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(serde_json::json!({"messageId": "test"}))) }
async fn render_template(Path(_name): Path<String>, _auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(serde_json::json!({"name": "template", "html": ""}))) }
