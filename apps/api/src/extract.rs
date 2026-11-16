//! Request extractors whose failures are problem+json like every other error, instead of axum's
//! plain-text rejections.

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts};
use axum::http::StatusCode;

use crate::error::ApiError;
use crate::problem::Problem;

/// A JSON body. Unknown fields, wrong types and missing fields are 422 `bad_input`.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Body<T>(pub T);

/// Path parameters. A malformed ID is 404, the same as an ID the caller can't see.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct Id<T>(pub T);

/// Query parameters. Unknown or malformed ones are 422 `bad_input`.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct Params<T>(pub T);

impl From<JsonRejection> for ApiError {
    fn from(r: JsonRejection) -> Self {
        match r {
            JsonRejection::MissingJsonContentType(_) => ApiError::Problem(Box::new(Problem::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                "JSON required",
                "Send the body as JSON with Content-Type: application/json.",
            ))),
            JsonRejection::BytesRejection(_) => ApiError::Problem(Box::new(Problem::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "Body too large",
                "The request body is larger than this server accepts.",
            ))),
            other => ApiError::Problem(Box::new(Problem::new(StatusCode::UNPROCESSABLE_ENTITY, "bad_input", "Invalid input", &short(&other.body_text())))),
        }
    }
}

impl From<PathRejection> for ApiError {
    fn from(_: PathRejection) -> Self {
        ApiError::not_found()
    }
}

impl From<QueryRejection> for ApiError {
    fn from(r: QueryRejection) -> Self {
        ApiError::Problem(Box::new(Problem::new(StatusCode::UNPROCESSABLE_ENTITY, "bad_input", "Invalid input", &short(&r.body_text()))))
    }
}

/// serde's message names the field and what was wrong; keep it short so it can't echo a whole body.
fn short(s: &str) -> String {
    s.chars().take(300).collect()
}
