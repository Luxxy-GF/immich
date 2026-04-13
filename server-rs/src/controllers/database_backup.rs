use axum::{extract::{Multipart, Path}, http::{header, HeaderMap, StatusCode}, response::IntoResponse, routing::{delete, get, post}, Json, Router};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::{error::AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_backups).delete(delete_backups))
        .route("/start-restore", post(start_restore))
        .route("/upload", post(upload_backup))
        .route("/:filename", get(download_backup))
}

fn backup_root() -> PathBuf {
    PathBuf::from("/root/immich/upload/backups")
}

async fn list_backups() -> Result<Json<Value>, AppError> {
    let root = backup_root();
    tokio::fs::create_dir_all(&root).await.ok();
    let mut backups = Vec::new();
    let mut entries = tokio::fs::read_dir(&root).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    while let Some(entry) = entries.next_entry().await.map_err(|e| AppError::InternalServerError(e.into()))? {
        let meta = entry.metadata().await.map_err(|e| AppError::InternalServerError(e.into()))?;
        if !meta.is_file() {
            continue;
        }
        backups.push(json!({
            "filename": entry.file_name().to_string_lossy().to_string(),
            "filesize": meta.len(),
            "timezone": "UTC"
        }));
    }
    backups.sort_by(|a, b| a["filename"].as_str().cmp(&b["filename"].as_str()));
    Ok(Json(json!({ "backups": backups })))
}

async fn delete_backups(Json(payload): Json<Value>) -> Result<StatusCode, AppError> {
    if let Some(backups) = payload.get("backups").and_then(|v| v.as_array()) {
        for filename in backups.iter().filter_map(|v| v.as_str()) {
            let _ = tokio::fs::remove_file(backup_root().join(filename)).await;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn start_restore() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn upload_backup(mut multipart: Multipart) -> Result<StatusCode, AppError> {
    let root = backup_root();
    tokio::fs::create_dir_all(&root).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    while let Some(mut field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
        if field.name() != Some("file") {
            continue;
        }
        let file_name = field.file_name().unwrap_or("backup.sql").to_string();
        let path = root.join(file_name);
        let mut file = tokio::fs::File::create(path).await.map_err(|e| AppError::InternalServerError(e.into()))?;
        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = field.chunk().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
            file.write_all(&chunk).await.map_err(|e| AppError::InternalServerError(e.into()))?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn download_backup(Path(filename): Path<String>) -> Result<impl IntoResponse, AppError> {
    let path = backup_root().join(&filename);
    let bytes = tokio::fs::read(&path).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/octet-stream".parse().unwrap());
    headers.insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"").parse().unwrap());
    Ok((headers, bytes))
}
