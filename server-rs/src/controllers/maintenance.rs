use axum::{http::StatusCode, routing::{get, post}, Json, Router};
use serde_json::{json, Value};

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/detect-install", get(detect_install))
        .route("/login", post(maintenance_login))
        .route("/", post(set_maintenance))
}

async fn get_status() -> Result<Json<Value>, AppError> {
    Ok(Json(json!({
        "action": "end",
        "active": false,
        "progress": null,
        "task": null,
        "error": null
    })))
}

async fn detect_install(_auth: AuthDto) -> Result<Json<Value>, AppError> {
    let storage = ["encoded-video", "library", "upload", "profile", "thumbs", "backups"]
        .into_iter()
        .map(|folder| {
            let path = std::path::Path::new("/root/immich/upload").join(folder);
            let readable = std::fs::read_dir(&path).is_ok();
            let writable = std::fs::create_dir_all(&path).is_ok();
            let files = std::fs::read_dir(&path).map(|iter| iter.count()).unwrap_or(0);
            json!({
                "folder": folder,
                "readable": readable,
                "writable": writable,
                "files": files
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({ "storage": storage })))
}

async fn maintenance_login() -> Result<Json<Value>, AppError> {
    Ok(Json(json!({ "username": "maintenance" })))
}

async fn set_maintenance(_auth: AuthDto) -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}
