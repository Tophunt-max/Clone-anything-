//! RFC 9457 problem details, the error body of every API response (docs/platform/05-api-spec.md).

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use crate::request_id::{self, RequestId};

pub const CONTENT_TYPE: &str = "application/problem+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Problem {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    /// Stable machine-readable code; clients decide on this, never on `title` or `detail`.
    pub code: String,
    pub detail: String,
    /// The request field a validation error is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl Problem {
    pub fn new(status: StatusCode, code: &str, title: &str, detail: &str) -> Self {
        Problem {
            kind: format!("about:blank#{code}"),
            title: title.to_string(),
            status: status.as_u16(),
            code: code.to_string(),
            detail: detail.to_string(),
            field: None,
            request_id: None,
        }
    }

    pub fn with_request_id(mut self, id: &RequestId) -> Self {
        self.request_id = Some(id.to_string());
        self
    }

    pub fn with_field(mut self, field: &str) -> Self {
        self.field = Some(field.to_string());
        self
    }
}

impl IntoResponse for Problem {
    fn into_response(mut self) -> Response {
        if self.request_id.is_none() {
            self.request_id = request_id::current().map(|id| id.to_string());
        }
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = serde_json::to_vec(&self).unwrap_or_default();
        let mut resp = (status, body).into_response();
        resp.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE));
        resp
    }
}
