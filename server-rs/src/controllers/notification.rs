use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    dtos::notification::NotificationDto,
    error::AppError,
    middleware::auth::AuthDto,
    models::Notification,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_notifications).put(update_notifications).delete(delete_notifications))
        .route("/:id", get(get_notification).put(update_notification).delete(delete_notification))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NotificationSearchQuery {
    unread: Option<bool>,
}

async fn get_notifications(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<NotificationSearchQuery>,
) -> Result<Json<Vec<NotificationDto>>, AppError> {
    let notifications = sqlx::query_as::<_, Notification>(
        r#"
        SELECT
            id::text as id,
            "createdAt",
            "updatedAt",
            "deletedAt",
            "userId"::text as "userId",
            level,
            type,
            data,
            title,
            description,
            "readAt"
        FROM notification
        WHERE ("userId" = $1::uuid OR "userId" IS NULL)
          AND "deletedAt" IS NULL
          AND ($2::bool IS NULL OR ("readAt" IS NULL) = $2::bool)
        ORDER BY "createdAt" DESC
        LIMIT 100
        "#,
    )
    .bind(&auth.user.id)
    .bind(query.unread)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        notifications
            .into_iter()
            .map(|notification| NotificationDto {
                id: notification.id,
                created_at: notification.created_at.to_rfc3339(),
                title: notification.title,
                description: notification.description,
                level: notification.level,
                read_at: notification.read_at.map(|value| value.to_rfc3339()),
                data: notification.data,
                r#type: notification.r#type,
            })
            .collect(),
    ))
}

async fn update_notifications() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn delete_notifications() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
async fn get_notification() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn update_notification() -> Result<Json<serde_json::Value>, AppError> { Ok(Json(json!({}))) }
async fn delete_notification() -> Result<StatusCode, AppError> { Ok(StatusCode::NO_CONTENT) }
