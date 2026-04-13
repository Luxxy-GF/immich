use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{
    dtos::user::{
        OnboardingDto, OnboardingResponseDto, UserAdminResponseDto, UserPreferencesResponseDto,
        UserPreferencesUpdateDto,
    },
    error::AppError,
    middleware::auth::AuthDto,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(search_users))
        .route("/me", get(get_me))
        .route("/me", axum::routing::put(update_me))
        .route("/me/preferences", get(get_my_preferences).put(update_my_preferences))
        .route("/me/license", get(get_my_license).put(set_my_license).delete(delete_my_license))
        .route(
            "/me/onboarding",
            get(get_my_onboarding)
                .put(update_my_onboarding)
                .delete(delete_my_onboarding),
        )
        .route("/:id", get(get_user))
        .route("/profile-image", axum::routing::post(create_profile_image).delete(delete_profile_image))
        .route("/:id/profile-image", get(get_profile_image))
}

async fn search_users(auth: AuthDto) -> Result<Json<Vec<UserAdminResponseDto>>, AppError> {
    Ok(Json(vec![user_admin_from_auth(&auth)]))
}

async fn get_me(auth: AuthDto) -> Result<Json<UserAdminResponseDto>, AppError> {
    Ok(Json(user_admin_from_auth(&auth)))
}

async fn update_me(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<UserAdminResponseDto>, AppError> {
    let email = payload.get("email").and_then(|v| v.as_str()).unwrap_or(&auth.user.email);
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or(&auth.user.name);
    sqlx::query(
        r#"UPDATE "user" SET email = $1, name = $2, "updatedAt" = NOW() WHERE id = $3::uuid"#,
    )
    .bind(email)
    .bind(name)
    .bind(&auth.user.id)
    .execute(&state.db)
    .await?;
    let mut updated = user_admin_from_auth(&auth);
    updated.email = email.to_string();
    updated.name = name.to_string();
    Ok(Json(updated))
}

async fn get_my_preferences(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<UserPreferencesResponseDto>, AppError> {
    let stored = load_preferences_value(&state, &auth.user.id).await?;
    let preferences = build_preferences_response(stored)?;
    Ok(Json(preferences))
}

async fn update_my_preferences(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(dto): Json<UserPreferencesUpdateDto>,
) -> Result<Json<UserPreferencesResponseDto>, AppError> {
    let stored = load_preferences_value(&state, &auth.user.id).await?;
    let mut merged_value = response_to_value(build_preferences_response(stored)?)?;
    deep_merge(&mut merged_value, update_to_value(dto)?);

    let response = value_to_response(merged_value.clone())?;
    let partial = diff_from_defaults(merged_value, response_to_value(UserPreferencesResponseDto::default())?);

    sqlx::query(
        r#"
        INSERT INTO user_metadata ("userId", key, value)
        VALUES ($1::uuid, 'preferences', $2)
        ON CONFLICT ("userId", key)
        DO UPDATE SET value = EXCLUDED.value
        "#,
    )
    .bind(&auth.user.id)
    .bind(partial)
    .execute(&state.db)
    .await?;

    Ok(Json(response))
}

async fn get_my_onboarding(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<OnboardingResponseDto>, AppError> {
    let stored = load_user_metadata_value(&state, &auth.user.id, "onboarding").await?;
    let is_onboarded = stored
        .as_ref()
        .and_then(|value| value.get("isOnboarded"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    Ok(Json(OnboardingResponseDto { is_onboarded }))
}

async fn update_my_onboarding(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(dto): Json<OnboardingDto>,
) -> Result<Json<OnboardingResponseDto>, AppError> {
    sqlx::query(
        r#"
        INSERT INTO user_metadata ("userId", key, value)
        VALUES ($1::uuid, 'onboarding', $2)
        ON CONFLICT ("userId", key)
        DO UPDATE SET value = EXCLUDED.value
        "#,
    )
    .bind(&auth.user.id)
    .bind(serde_json::json!({ "isOnboarded": dto.is_onboarded }))
    .execute(&state.db)
    .await?;

    Ok(Json(OnboardingResponseDto {
        is_onboarded: dto.is_onboarded,
    }))
}

async fn delete_my_onboarding(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<StatusCode, AppError> {
    sqlx::query(
        r#"
        DELETE FROM user_metadata
        WHERE "userId" = $1::uuid
          AND key = 'onboarding'
        "#,
    )
    .bind(&auth.user.id)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn get_my_license(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<Json<serde_json::Value>, AppError> {
    let stored = load_user_metadata_value(&state, &auth.user.id, "license").await?;
    Ok(Json(stored.unwrap_or_else(|| json!({}))))
}

async fn set_my_license(
    State(state): State<AppState>,
    auth: AuthDto,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query(
        r#"INSERT INTO user_metadata ("userId", key, value) VALUES ($1::uuid, 'license', $2) ON CONFLICT ("userId", key) DO UPDATE SET value = EXCLUDED.value"#,
    )
    .bind(&auth.user.id)
    .bind(&payload)
    .execute(&state.db)
    .await?;
    Ok(Json(payload))
}

async fn delete_my_license(
    State(state): State<AppState>,
    auth: AuthDto,
) -> Result<StatusCode, AppError> {
    sqlx::query(r#"DELETE FROM user_metadata WHERE "userId" = $1::uuid AND key = 'license'"#)
        .bind(&auth.user.id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_user(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user = sqlx::query_as::<_, crate::models::User>(
        r#"SELECT "id"::text as "id", "name", "email", "avatarColor", "profileImagePath", "profileChangedAt", "storageLabel", "shouldChangePassword", "isAdmin", "createdAt", "updatedAt", "deletedAt", "oauthId", "quotaSizeInBytes", "quotaUsageInBytes", "status", "password", "pinCode" FROM "user" WHERE id = $1::uuid AND "deletedAt" IS NULL"#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(user.map(|u| json!({
        "id": u.id,
        "email": u.email,
        "name": u.name,
        "profileImagePath": u.profile_image_path,
        "isAdmin": u.is_admin,
        "status": u.status,
    })).unwrap_or_else(|| json!({}))))
}

async fn create_profile_image() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NOT_IMPLEMENTED)
}

async fn delete_profile_image() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NO_CONTENT)
}

async fn get_profile_image() -> Result<StatusCode, AppError> {
    Ok(StatusCode::NOT_FOUND)
}

async fn load_preferences_value(state: &AppState, user_id: &str) -> Result<Option<Value>, AppError> {
    load_user_metadata_value(state, user_id, "preferences").await
}

async fn load_user_metadata_value(
    state: &AppState,
    user_id: &str,
    key: &str,
) -> Result<Option<Value>, AppError> {
    Ok(sqlx::query_scalar::<_, Value>(
        r#"
        SELECT value
        FROM user_metadata
        WHERE "userId" = $1::uuid
          AND key = $2
        "#,
    )
    .bind(user_id)
    .bind(key)
    .fetch_optional(&state.db)
    .await?)
}

fn build_preferences_response(stored: Option<Value>) -> Result<UserPreferencesResponseDto, AppError> {
    let mut defaults = response_to_value(UserPreferencesResponseDto::default())?;
    if let Some(value) = stored {
        deep_merge(&mut defaults, value);
    }

    value_to_response(defaults)
}

fn response_to_value(response: UserPreferencesResponseDto) -> Result<Value, AppError> {
    serde_json::to_value(response).map_err(Into::into)
}

fn update_to_value(update: UserPreferencesUpdateDto) -> Result<Value, AppError> {
    let mut value = serde_json::to_value(update)?;
    strip_nulls(&mut value);
    Ok(value)
}

fn value_to_response(value: Value) -> Result<UserPreferencesResponseDto, AppError> {
    serde_json::from_value(value).map_err(Into::into)
}

fn deep_merge(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target_map), Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                match target_map.get_mut(&key) {
                    Some(existing) => deep_merge(existing, value),
                    None => {
                        target_map.insert(key, value);
                    }
                }
            }
        }
        (target_slot, patch_value) => {
            *target_slot = patch_value;
        }
    }
}

fn diff_from_defaults(current: Value, defaults: Value) -> Value {
    match (current, defaults) {
        (Value::Object(current_map), Value::Object(default_map)) => {
            let mut diff = serde_json::Map::new();

            for (key, current_value) in current_map {
                let maybe_default = default_map.get(&key).cloned().unwrap_or(Value::Null);
                let diff_value = diff_from_defaults(current_value, maybe_default);

                let should_keep = match &diff_value {
                    Value::Null => false,
                    Value::String(value) if value.is_empty() => false,
                    Value::Object(map) if map.is_empty() => false,
                    _ => true,
                };

                if should_keep {
                    diff.insert(key, diff_value);
                }
            }

            Value::Object(diff)
        }
        (current_value, default_value) if current_value == default_value => Value::Null,
        (current_value, _) => current_value,
    }
}

fn strip_nulls(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let keys_to_remove: Vec<String> = map
                .iter_mut()
                .filter_map(|(key, value)| {
                    strip_nulls(value);
                    match value {
                        Value::Null => Some(key.clone()),
                        Value::Object(child) if child.is_empty() => Some(key.clone()),
                        _ => None,
                    }
                })
                .collect();

            for key in keys_to_remove {
                map.remove(&key);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_nulls(item);
            }
        }
        _ => {}
    }
}

fn user_admin_from_auth(auth: &AuthDto) -> UserAdminResponseDto {
    UserAdminResponseDto {
        id: auth.user.id.clone(),
        email: auth.user.email.clone(),
        name: auth.user.name.clone(),
        first_name: String::new(),
        last_name: String::new(),
        profile_image_path: auth.user.profile_image_path.clone(),
        is_admin: auth.user.is_admin,
        should_change_password: auth.user.should_change_password,
        storage_label: auth.user.storage_label.clone(),
        status: auth.user.status.clone(),
        quota_size_in_bytes: auth.user.quota_size_in_bytes,
        quota_usage_in_bytes: auth.user.quota_usage_in_bytes,
    }
}
