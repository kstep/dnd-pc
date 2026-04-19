use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};

use crate::{AppState, firebase::FirestoreError};

pub async fn character(
    State(state): State<AppState>,
    Path((uid, char_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = ["users", uid.as_str(), "characters", char_id.as_str()];
    let bearer = headers.get(header::AUTHORIZATION);

    match state.firebase.fetch_doc(&state.http, &path, bearer).await {
        Ok(Some(doc)) => {
            let body = serde_json::to_string(&doc).unwrap_or_else(|_| "{}".into());
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                body,
            )
                .into_response()
        }
        Ok(None) => json_error(StatusCode::NOT_FOUND, r#"{"error":"not_found"}"#),
        Err(err) => firestore_error(err),
    }
}

pub async fn avatar(
    State(state): State<AppState>,
    Path((uid, char_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = ["users", uid.as_str(), "avatars", char_id.as_str()];
    let bearer = headers.get(header::AUTHORIZATION);

    let doc = match state.firebase.fetch_doc(&state.http, &path, bearer).await {
        Ok(Some(doc)) => doc,
        Ok(None) => return json_error(StatusCode::NOT_FOUND, r#"{"error":"not_found"}"#),
        Err(err) => return firestore_error(err),
    };

    let Some(data_uri) = doc.get("data_uri").and_then(|value| value.as_str()) else {
        tracing::error!(uid, char_id, "avatar doc missing data_uri");
        return json_error(StatusCode::NOT_FOUND, r#"{"error":"not_found"}"#);
    };

    let (mime, payload) = match parse_data_uri(data_uri) {
        Some(parts) => parts,
        None => {
            tracing::error!(uid, char_id, "avatar data_uri not base64-encoded");
            return json_error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                r#"{"error":"bad_data_uri"}"#,
            );
        }
    };

    let Ok(bytes) = STANDARD.decode(payload) else {
        tracing::error!(uid, char_id, "avatar base64 decode failed");
        return json_error(StatusCode::BAD_GATEWAY, r#"{"error":"bad_data_uri"}"#);
    };

    (StatusCode::OK, [(header::CONTENT_TYPE, mime)], bytes).into_response()
}

/// Parse `data:<mime>;base64,<payload>` into `(mime, payload)`. Returns
/// `None` if the URI isn't base64-encoded — we don't support other forms.
fn parse_data_uri(uri: &str) -> Option<(&str, &str)> {
    let rest = uri.strip_prefix("data:")?;
    let (header, payload) = rest.split_once(',')?;
    let mime = header.strip_suffix(";base64")?;
    Some((mime, payload))
}

fn firestore_error(err: FirestoreError) -> Response {
    match err {
        FirestoreError::Forbidden => json_error(StatusCode::FORBIDDEN, r#"{"error":"forbidden"}"#),
        FirestoreError::Upstream => {
            json_error(StatusCode::BAD_GATEWAY, r#"{"error":"firestore_error"}"#)
        }
    }
}

fn json_error(status: StatusCode, body: &'static str) -> Response {
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_webp_data_uri() {
        let (mime, payload) = parse_data_uri("data:image/webp;base64,AAAA").unwrap();
        assert_eq!(mime, "image/webp");
        assert_eq!(payload, "AAAA");
    }

    #[test]
    fn rejects_non_base64_uri() {
        assert!(parse_data_uri("data:image/webp,raw").is_none());
        assert!(parse_data_uri("https://example.com/img.webp").is_none());
    }
}
