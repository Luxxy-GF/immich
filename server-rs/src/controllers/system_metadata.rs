use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde_json::json;

use crate::{
    dtos::system_metadata::{AdminOnboardingResponseDto, AdminOnboardingUpdateDto},
    error::AppError,
    middleware::auth::AuthDto,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin-onboarding", get(get_admin_onboarding).post(update_admin_onboarding))
        .route("/reverse-geocoding-state", get(get_reverse_geocoding_state))
        .route("/version-check-state", get(get_version_check_state))
}

async fn get_admin_onboarding(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<AdminOnboardingResponseDto>, AppError> {
    ensure_admin(&auth)?;

    let value = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT value
        FROM system_metadata
        WHERE key = 'admin-onboarding'
        "#,
    )
    .fetch_optional(&state.db)
    .await?;

    let is_onboarded = value
        .as_ref()
        .and_then(|value| value.get("isOnboarded"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    Ok(Json(AdminOnboardingResponseDto { is_onboarded }))
}

async fn update_admin_onboarding(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(dto): Json<AdminOnboardingUpdateDto>,
) -> Result<StatusCode, AppError> {
    ensure_admin(&auth)?;

    sqlx::query(
        r#"
        INSERT INTO system_metadata (key, value)
        VALUES ('admin-onboarding', $1)
        ON CONFLICT (key)
        DO UPDATE SET value = EXCLUDED.value
        "#,
    )
    .bind(json!({
        "isOnboarded": dto.is_onboarded,
    }))
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn ensure_admin(auth: &AuthDto) -> Result<(), AppError> {
    if auth.user.is_admin {
        Ok(())
    } else {
        Err(AppError::BadRequest("Admin access required".to_string()))
    }
}

async fn get_reverse_geocoding_state(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<serde_json::Value>, AppError> {
    ensure_admin(&auth)?;
    let value = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT value FROM system_metadata WHERE key = 'reverse-geocoding-state'"#,
    )
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(value.unwrap_or_else(|| json!({"lastUpdate": null, "lastImportFileName": null}))))
}

async fn get_version_check_state(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<serde_json::Value>, AppError> {
    ensure_admin(&auth)?;
    let value = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT value FROM system_metadata WHERE key = 'version-check-state'"#,
    )
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(value.unwrap_or_else(|| json!({"checkedAt": null, "releaseVersion": null}))))
}
