//! The one error type handlers return, mapped onto HTTP status codes.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use sofd_error::{FormatError, OfdError, RenderError};
use uuid::Uuid;

/// A failure with the status code and message the API reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    /// A failure with an explicit status and message.
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    /// A document the route named but the store no longer holds.
    pub fn unknown_document(id: Uuid) -> Self {
        Self::new(StatusCode::NOT_FOUND, format!("no open document {id}"))
    }

    /// The store could not read or write the copy it keeps. That is the
    /// service's own fault, not the caller's, so it is a 500.
    pub fn storage(err: std::io::Error) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("the document store failed: {err}"),
        )
    }
}

/// The body every failure carries: the engine's own message, verbatim.
#[derive(Debug, Serialize)]
struct Failure<'a> {
    error: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(Failure {
                error: &self.message,
            }),
        )
            .into_response()
    }
}

impl From<OfdError> for ApiError {
    fn from(err: OfdError) -> Self {
        let status = status_of(&err);
        Self::new(status, err.to_string())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        Self::storage(err)
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(join: tokio::task::JoinError) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("the worker thread failed: {join}"),
        )
    }
}

/// Classify an engine failure. Damage in the uploaded document is the caller's
/// problem, while failing to turn a sound document into output is ours: a
/// missing font says nothing about the bytes the caller sent.
fn status_of(err: &OfdError) -> StatusCode {
    match err {
        // The page named does not exist, which is a routing question.
        OfdError::Format(FormatError::PageOutOfRange { .. }) => StatusCode::NOT_FOUND,
        // A guard rail stopped a hostile document before it cost us more.
        OfdError::ResourceLimit(_) => StatusCode::PAYLOAD_TOO_LARGE,
        // The caller asked for a picture bigger than the renderer will spend
        // memory on, which is a sizing problem rather than a broken service.
        OfdError::Render(RenderError::BudgetExceeded(_)) => StatusCode::PAYLOAD_TOO_LARGE,
        OfdError::PasswordRequired => StatusCode::LOCKED,
        OfdError::Package(_) | OfdError::Xml(_) | OfdError::Format(_) => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        OfdError::Signature(_) | OfdError::Crypto(_) | OfdError::Compliance(_) => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
