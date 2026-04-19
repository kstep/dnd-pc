use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures::TryStreamExt;

use crate::AppState;

const OPENAI_BASE: &str = "https://api.openai.com/v1";

pub async fn forward(
    State(state): State<AppState>,
    method: Method,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let url = format!("{OPENAI_BASE}/{path}");
    let mut req = state
        .http
        .request(method, &url)
        .bearer_auth(state.openai_api_key.as_ref());

    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        req = req.header(header::CONTENT_TYPE, ct);
    }
    if !body.is_empty() {
        req = req.body(body);
    }

    let upstream = match req.send().await {
        Ok(resp) => resp,
        Err(err) => {
            tracing::error!(path, error = %err, "upstream failed");
            return (StatusCode::BAD_GATEWAY, r#"{"error":"upstream_error"}"#).into_response();
        }
    };

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut resp_headers = HeaderMap::new();
    if let Some(ct) = upstream.headers().get(header::CONTENT_TYPE) {
        resp_headers.insert(header::CONTENT_TYPE, ct.clone());
    }

    let stream = upstream
        .bytes_stream()
        .map_err(|err| std::io::Error::other(err.to_string()));
    (status, resp_headers, Body::from_stream(stream)).into_response()
}
