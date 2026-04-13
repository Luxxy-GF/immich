use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/custom.css", get(get_custom_css))
        .route("/.well-known/immich", get(get_well_known))
}

async fn get_well_known() -> impl IntoResponse {
    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "api": {
                "endpoint": "/api"
            }
        })),
    )
}

async fn load_custom_css(state: &AppState) -> Result<String, crate::error::AppError> {
    let value = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM system_metadata
        WHERE key = 'system-config'
        "#,
    )
    .fetch_optional(&state.db)
    .await?;

    Ok(value
        .as_ref()
        .and_then(|value| value.get("theme"))
        .and_then(|value| value.get("customCss"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string())
}

async fn get_custom_css(State(state): State<AppState>) -> impl IntoResponse {
    let css = load_custom_css(&state).await.unwrap_or_default();
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], css)
}
