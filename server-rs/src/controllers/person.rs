use axum::{
    extract::{Json as JsonBody, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::{error::AppError, middleware::auth::AuthDto, AppState};

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersonSearchQuery {
    with_hidden: Option<bool>,
    page: Option<i64>,
    size: Option<i64>,
    closest_person_id: Option<String>,
    closest_asset_id: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_people).post(create_person).put(update_people).delete(delete_people))
        .route("/:id", get(get_person).put(update_person).delete(delete_person))
        .route("/:id/statistics", get(get_person_statistics))
        .route("/:id/thumbnail", get(get_person_thumbnail))
        .route("/:id/reassign", put(reassign_faces))
        .route("/:id/merge", post(merge_person))
}

async fn get_people(
    State(state): State<AppState>,
    auth: AuthDto,
    Query(query): Query<PersonSearchQuery>,
) -> Result<Json<Value>, AppError> {
    let size = query.size.unwrap_or(500);
    let page = query.page.unwrap_or(1);
    let rows = sqlx::query(
        r#"
        SELECT
            p.id::text as id,
            p.name,
            p."thumbnailPath",
            p."isHidden",
            p."isFavorite",
            p.color,
            p."birthDate",
            p."updatedAt"
        FROM person p
        WHERE p."ownerId" = $1::uuid
          AND ($2::bool = true OR p."isHidden" = false)
        ORDER BY p."isFavorite" DESC, p.name ASC, p."createdAt" ASC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(&auth.user.id)
    .bind(query.with_hidden.unwrap_or(false))
    .bind(size + 1)
    .bind((page - 1) * size)
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM person WHERE "ownerId" = $1::uuid"#,
    )
    .bind(&auth.user.id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    let hidden: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM person WHERE "ownerId" = $1::uuid AND "isHidden" = true"#,
    )
    .bind(&auth.user.id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let has_next_page = rows.len() as i64 > size;
    let people = rows.into_iter().take(size as usize).map(person_row_to_json).collect::<Vec<_>>();

    Ok(Json(json!({
        "people": people,
        "hasNextPage": has_next_page,
        "total": total,
        "hidden": hidden
    })))
}

async fn create_person(
    State(state): State<AppState>,
    auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Value>, AppError> {
    let id = Uuid::new_v4().to_string();
    let row = sqlx::query(
        r#"
        INSERT INTO person ("id", "ownerId", name, "isHidden", "isFavorite", color, "birthDate")
        VALUES ($1::uuid, $2::uuid, COALESCE($3, ''), COALESCE($4, false), COALESCE($5, false), $6, $7)
        RETURNING id::text as id, name, "thumbnailPath", "isHidden", "isFavorite", color, "birthDate", "updatedAt"
        "#,
    )
    .bind(&id)
    .bind(&auth.user.id)
    .bind(payload.get("name").and_then(|v| v.as_str()))
    .bind(payload.get("isHidden").and_then(|v| v.as_bool()))
    .bind(payload.get("isFavorite").and_then(|v| v.as_bool()))
    .bind(payload.get("color").and_then(|v| v.as_str()))
    .bind(payload.get("birthDate").and_then(|v| v.as_str()).and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()))
    .fetch_one(&state.db)
    .await?;
    Ok(Json(person_row_to_json(row)))
}

async fn update_people(
    State(state): State<AppState>,
    auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Vec<Value>>, AppError> {
    let mut results = Vec::new();
    if let Some(people) = payload.get("people").and_then(|v| v.as_array()) {
        for person in people {
            if let Some(id) = person.get("id").and_then(|v| v.as_str()) {
                let _ = sqlx::query(
                    r#"
                    UPDATE person
                    SET name = COALESCE($1, name),
                        "isHidden" = COALESCE($2, "isHidden"),
                        "isFavorite" = COALESCE($3, "isFavorite"),
                        color = COALESCE($4, color),
                        "birthDate" = COALESCE($5, "birthDate"),
                        "updatedAt" = NOW()
                    WHERE id = $6::uuid AND "ownerId" = $7::uuid
                    "#,
                )
                .bind(person.get("name").and_then(|v| v.as_str()))
                .bind(person.get("isHidden").and_then(|v| v.as_bool()))
                .bind(person.get("isFavorite").and_then(|v| v.as_bool()))
                .bind(person.get("color").and_then(|v| v.as_str()))
                .bind(person.get("birthDate").and_then(|v| v.as_str()).and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()))
                .bind(id)
                .bind(&auth.user.id)
                .execute(&state.db)
                .await?;
                results.push(json!({"id": id, "success": true}));
            }
        }
    }
    Ok(Json(results))
}

async fn delete_people(
    State(state): State<AppState>,
    auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<StatusCode, AppError> {
    if let Some(ids) = payload.get("ids").and_then(|v| v.as_array()) {
        for id in ids.iter().filter_map(|v| v.as_str()) {
            let _ = sqlx::query(r#"DELETE FROM person WHERE id = $1::uuid AND "ownerId" = $2::uuid"#)
                .bind(id)
                .bind(&auth.user.id)
                .execute(&state.db)
                .await?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn get_person(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
) -> Result<Json<Value>, AppError> {
    let row = sqlx::query(
        r#"
        SELECT id::text as id, name, "thumbnailPath", "isHidden", "isFavorite", color, "birthDate", "updatedAt"
        FROM person
        WHERE id = $1::uuid AND "ownerId" = $2::uuid
        "#,
    )
    .bind(&id)
    .bind(&auth.user.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Person not found".to_string()))?;
    Ok(Json(person_row_to_json(row)))
}

async fn update_person(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Value>, AppError> {
    let row = sqlx::query(
        r#"
        UPDATE person
        SET name = COALESCE($1, name),
            "isHidden" = COALESCE($2, "isHidden"),
            "isFavorite" = COALESCE($3, "isFavorite"),
            color = COALESCE($4, color),
            "birthDate" = COALESCE($5, "birthDate"),
            "updatedAt" = NOW()
        WHERE id = $6::uuid AND "ownerId" = $7::uuid
        RETURNING id::text as id, name, "thumbnailPath", "isHidden", "isFavorite", color, "birthDate", "updatedAt"
        "#,
    )
    .bind(payload.get("name").and_then(|v| v.as_str()))
    .bind(payload.get("isHidden").and_then(|v| v.as_bool()))
    .bind(payload.get("isFavorite").and_then(|v| v.as_bool()))
    .bind(payload.get("color").and_then(|v| v.as_str()))
    .bind(payload.get("birthDate").and_then(|v| v.as_str()).and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()))
    .bind(&id)
    .bind(&auth.user.id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(person_row_to_json(row)))
}

async fn delete_person(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
) -> Result<StatusCode, AppError> {
    let _ = sqlx::query(r#"DELETE FROM person WHERE id = $1::uuid AND "ownerId" = $2::uuid"#)
        .bind(&id)
        .bind(&auth.user.id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_person_statistics(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
) -> Result<Json<Value>, AppError> {
    let assets: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM asset_face WHERE "personId" = $1::uuid AND "deletedAt" IS NULL"#,
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    let _ = auth;
    Ok(Json(json!({ "assets": assets })))
}

async fn get_person_thumbnail(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
) -> Result<impl IntoResponse, AppError> {
    let path: String = sqlx::query_scalar::<_, String>(
        r#"SELECT "thumbnailPath" FROM person WHERE id = $1::uuid AND "ownerId" = $2::uuid"#,
    )
    .bind(&id)
    .bind(&auth.user.id)
    .fetch_optional(&state.db)
    .await?
    .filter(|p: &String| !p.is_empty())
    .ok_or_else(|| AppError::BadRequest("Person thumbnail not found".to_string()))?;
    let bytes = tokio::fs::read(&path).await.map_err(|e| AppError::InternalServerError(e.into()))?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/jpeg".parse().unwrap());
    Ok((headers, bytes))
}

async fn reassign_faces(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Vec<Value>>, AppError> {
    let mut results = Vec::new();
    if let Some(data) = payload.get("data").and_then(|v| v.as_array()) {
        for item in data {
            if let (Some(asset_id), Some(person_id)) = (item.get("assetId").and_then(|v| v.as_str()), item.get("personId").and_then(|v| v.as_str())) {
                let _ = sqlx::query(
                    r#"UPDATE asset_face SET "personId" = $1::uuid WHERE "assetId" = $2::uuid AND "deletedAt" IS NULL"#,
                )
                .bind(person_id)
                .bind(asset_id)
                .execute(&state.db)
                .await?;
                results.push(json!({ "id": id, "assetId": asset_id, "personId": person_id }));
            }
        }
    }
    let _ = auth;
    Ok(Json(results))
}

async fn merge_person(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth: AuthDto,
    JsonBody(payload): JsonBody<Value>,
) -> Result<Json<Vec<Value>>, AppError> {
    let mut results = Vec::new();
    if let Some(ids) = payload.get("ids").and_then(|v| v.as_array()) {
        for other_id in ids.iter().filter_map(|v| v.as_str()) {
            let _ = sqlx::query(r#"UPDATE asset_face SET "personId" = $1::uuid WHERE "personId" = $2::uuid AND "deletedAt" IS NULL"#)
                .bind(&id)
                .bind(other_id)
                .execute(&state.db)
                .await?;
            let _ = sqlx::query(r#"DELETE FROM person WHERE id = $1::uuid AND "ownerId" = $2::uuid"#)
                .bind(other_id)
                .bind(&auth.user.id)
                .execute(&state.db)
                .await?;
            results.push(json!({ "id": other_id, "success": true }));
        }
    }
    Ok(Json(results))
}

fn person_row_to_json(row: sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.try_get::<String,_>("id").unwrap_or_default(),
        "name": row.try_get::<String,_>("name").unwrap_or_default(),
        "thumbnailPath": row.try_get::<String,_>("thumbnailPath").unwrap_or_default(),
        "isHidden": row.try_get::<bool,_>("isHidden").unwrap_or(false),
        "isFavorite": row.try_get::<bool,_>("isFavorite").unwrap_or(false),
        "color": row.try_get::<Option<String>,_>("color").unwrap_or(None),
        "birthDate": row.try_get::<Option<chrono::NaiveDate>,_>("birthDate").ok().flatten().map(|d| d.to_string()),
        "updatedAt": row.try_get::<chrono::DateTime<chrono::Utc>,_>("updatedAt").ok().map(|d| d.to_rfc3339()),
    })
}
