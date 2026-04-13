use serde::Serialize;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

#[derive(Serialize)]
pub struct NestErrorEnvelope {
    pub message: Vec<String>,
    pub error: String,
    #[serde(rename = "statusCode")]
    pub status_code: u16,
}

pub enum AppError {
    InternalServerError(anyhow::Error),
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, envelope) = match self {
            AppError::InternalServerError(err) => {
                tracing::error!("Internal Server Error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    NestErrorEnvelope {
                        message: vec!["Internal Server Error".to_string()],
                        error: "Internal Server Error".to_string(),
                        status_code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    },
                )
            }
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                NestErrorEnvelope {
                    message: vec![msg],
                    error: "Bad Request".to_string(),
                    status_code: StatusCode::BAD_REQUEST.as_u16(),
                },
            ),
        };

        (status, Json(envelope)).into_response()
    }
}

// Convert from anyhow::Error to AppError automatically
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self::InternalServerError(err.into())
    }
}
