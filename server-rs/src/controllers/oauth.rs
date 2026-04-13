use axum::{extract::State, http::{header, HeaderMap, StatusCode}, routing::{get, post}, Json, Router};
use serde_json::json;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/mobile-redirect", get(mobile_redirect))
        .route("/authorize", post(authorize))
        .route("/callback", post(callback))
        .route("/link", post(link_oauth))
        .route("/unlink", post(unlink_oauth))
}

async fn mobile_redirect() -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(json!({"url":"immich://","statusCode":307})))
}

async fn authorize() -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, "immich_oauth_state=test; Path=/; SameSite=Lax".parse().unwrap());
    headers.append(header::SET_COOKIE, "immich_oauth_code_verifier=test; Path=/; SameSite=Lax".parse().unwrap());
    Ok((headers, Json(json!({"url":""}))))
}

async fn callback() -> Result<(HeaderMap, Json<serde_json::Value>), AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, "immich_oauth_state=; Path=/; Max-Age=0; SameSite=Lax".parse().unwrap());
    headers.append(header::SET_COOKIE, "immich_oauth_code_verifier=; Path=/; Max-Age=0; SameSite=Lax".parse().unwrap());
    Ok((headers, Json(json!({"accessToken":"","userId":"","userEmail":"","firstName":"","lastName":"","profileImagePath":"","isAdmin":false,"shouldChangePassword":false}))))
}

async fn link_oauth(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn unlink_oauth(_auth: AuthDto) -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
