use std::{
    collections::{HashMap, HashSet},
    env,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Context;
use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, jwk::JwkSet};
use serde::Deserialize;
use serde_json::{Map, Value};
use tokio::sync::RwLock;

use crate::AppState;

const FIRESTORE_BASE: &str = "https://firestore.googleapis.com/v1";
const JWKS_URL: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";
const JWKS_TTL: Duration = Duration::from_secs(3600);
const JWKS_REFRESH_DEBOUNCE: Duration = Duration::from_secs(300);
const WHITELIST_TTL: Duration = Duration::from_secs(60);
const WHITELIST_DOC_PATH: &[&str] = &["config", "proxy"];

/// Everything we need to talk to a specific Firebase project: public
/// identifiers, the JWKS cache used to verify ID tokens, and the runtime
/// email whitelist fetched from Firestore.
#[derive(Clone)]
pub struct FirebaseState {
    pub project_id: Arc<str>,
    pub api_key: Arc<str>,
    jwks: JwksCache,
    whitelist: WhitelistCache,
}

impl FirebaseState {
    /// Build from environment: `FIREBASE_PROJECT_ID` and `FIREBASE_API_KEY`.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            project_id: env::var("FIREBASE_PROJECT_ID")
                .context("FIREBASE_PROJECT_ID env var is required")?
                .into(),
            api_key: env::var("FIREBASE_API_KEY")
                .context("FIREBASE_API_KEY env var is required")?
                .into(),
            jwks: JwksCache::default(),
            whitelist: WhitelistCache::default(),
        })
    }

    /// Verify a Firebase ID token and return its claims. Rejects anything
    /// not signed in via `google.com` or whose email isn't whitelisted.
    pub async fn verify_google_bearer(
        &self,
        http: &reqwest::Client,
        token: &str,
    ) -> Result<Claims, AuthError> {
        let kid = jsonwebtoken::decode_header(token)
            .ok()
            .and_then(|h| h.kid)
            .ok_or(AuthError::Unauthorized)?;
        let key = self.jwks.get_key(http, &kid).await?;
        let claims = Claims::decode(token, &key, &self.project_id)?;
        if claims.firebase.sign_in_provider != "google.com" {
            return Err(AuthError::Forbidden);
        }
        self.check_email(http, claims.email.as_deref()).await?;
        Ok(claims)
    }

    /// Reject the request unless the caller's email is in the whitelist
    /// currently stored in Firestore. Default (empty / missing / unreadable
    /// doc) is deny-all.
    async fn check_email(
        &self,
        http: &reqwest::Client,
        email: Option<&str>,
    ) -> Result<(), AuthError> {
        let allowed = self.whitelist.get(self, http).await?;
        let Some(email) = email else {
            return Err(AuthError::EmailNotAllowed);
        };
        if allowed.contains(&email.to_ascii_lowercase()) {
            Ok(())
        } else {
            Err(AuthError::EmailNotAllowed)
        }
    }

    /// Fetch a Firestore document by path. Returns `Ok(None)` for 404,
    /// `Err(FirestoreError)` for transport / upstream failures.
    ///
    /// - `bearer = Some(...)` forwards the caller's ID token (authenticated
    ///   read — Firestore rules see `request.auth`).
    /// - `bearer = None` uses the public Firebase Web API key (unauth read;
    ///   only sees docs with public-read rules).
    pub async fn fetch_doc(
        &self,
        http: &reqwest::Client,
        path: &[&str],
        bearer: Option<&HeaderValue>,
    ) -> Result<Option<Value>, FirestoreError> {
        let joined = path.join("/");
        let url = format!(
            "{FIRESTORE_BASE}/projects/{}/databases/(default)/documents/{joined}",
            self.project_id
        );
        let mut req = http.get(&url);
        match bearer {
            Some(value) => req = req.header(header::AUTHORIZATION, value),
            None => req = req.query(&[("key", self.api_key.as_ref())]),
        }
        let resp = req.send().await.map_err(|err| {
            tracing::error!(error = %err, path = %joined, "firestore fetch failed");
            FirestoreError::Upstream
        })?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(FirestoreError::Forbidden);
        }
        if !status.is_success() {
            tracing::error!(status = %status, path = %joined, "firestore returned non-success");
            return Err(FirestoreError::Upstream);
        }

        let envelope: Value = resp.json().await.map_err(|err| {
            tracing::error!(error = %err, "firestore response parse failed");
            FirestoreError::Upstream
        })?;

        let decoded = envelope
            .get("fields")
            .map(decode_fields)
            .unwrap_or(Value::Object(Map::new()));
        Ok(Some(decoded))
    }
}

// --- Firestore value decoder ---

/// Unwrap a Firestore REST value envelope into plain JSON.
///
/// Firestore wraps every field in a typed object like `{"stringValue": "x"}`
/// or `{"mapValue": {"fields": {...}}}`. We flip this back to ordinary JSON
/// so the result matches the shape the Firebase client SDKs produce and what
/// the frontend `deserialize_character_value` expects.
pub(crate) fn decode_fields(fields: &Value) -> Value {
    let Some(obj) = fields.as_object() else {
        return Value::Object(Map::new());
    };
    let mut out = Map::with_capacity(obj.len());
    for (key, value) in obj {
        out.insert(key.clone(), decode_value(value));
    }
    Value::Object(out)
}

fn decode_value(value: &Value) -> Value {
    let Some(obj) = value.as_object() else {
        return value.clone();
    };
    let Some((kind, inner)) = obj.iter().next() else {
        return Value::Null;
    };
    match kind.as_str() {
        "nullValue" => Value::Null,
        "booleanValue" => inner.clone(),
        "stringValue" | "timestampValue" | "bytesValue" | "referenceValue" | "geoPointValue" => {
            inner.clone()
        }
        "integerValue" => match inner {
            // Firestore encodes ints as strings to preserve i64 precision.
            Value::String(str) => str
                .parse::<i64>()
                .map(Value::from)
                .unwrap_or_else(|_| Value::String(str.clone())),
            other => other.clone(),
        },
        "doubleValue" => inner.clone(),
        "arrayValue" => {
            let values = inner
                .get("values")
                .and_then(Value::as_array)
                .map(|array| array.iter().map(decode_value).collect())
                .unwrap_or_default();
            Value::Array(values)
        }
        "mapValue" => inner
            .get("fields")
            .map(decode_fields)
            .unwrap_or_else(|| Value::Object(Map::new())),
        _ => value.clone(),
    }
}

// --- JWKS cache ---

/// Private JWKS cache. Firebase rotates keys roughly daily; caching avoids
/// a round-trip to Google on every authenticated request.
#[derive(Clone, Default)]
struct JwksCache {
    inner: Arc<RwLock<Jwks>>,
}

#[derive(Default)]
struct Jwks {
    keys: HashMap<String, DecodingKey>,
    refreshed: Option<Instant>,
}

impl JwksCache {
    async fn get_key(&self, http: &reqwest::Client, kid: &str) -> Result<DecodingKey, AuthError> {
        {
            let cache = self.inner.read().await;
            if let Some(key) = cache.keys.get(kid)
                && cache.refreshed.is_some_and(|at| at.elapsed() < JWKS_TTL)
            {
                return Ok(key.clone());
            }
            // Miss: debounce so a bogus kid can't hammer upstream.
            if cache
                .refreshed
                .is_some_and(|at| at.elapsed() < JWKS_REFRESH_DEBOUNCE)
            {
                return Err(AuthError::Unauthorized);
            }
        }
        self.refresh(http).await?;
        let cache = self.inner.read().await;
        cache.keys.get(kid).cloned().ok_or(AuthError::Unauthorized)
    }

    async fn refresh(&self, http: &reqwest::Client) -> Result<(), AuthError> {
        let set: JwkSet = http
            .get(JWKS_URL)
            .send()
            .await
            .map_err(|_| AuthError::Unauthorized)?
            .json()
            .await
            .map_err(|_| AuthError::Unauthorized)?;
        let mut cache = self.inner.write().await;
        cache.keys.clear();
        for jwk in &set.keys {
            let Some(kid) = jwk.common.key_id.clone() else {
                continue;
            };
            if let Ok(key) = DecodingKey::from_jwk(jwk) {
                cache.keys.insert(kid, key);
            }
        }
        cache.refreshed = Some(Instant::now());
        Ok(())
    }
}

// --- Whitelist cache ---

/// Email whitelist cached in memory for `WHITELIST_TTL`. Read from
/// `/config/proxy` in Firestore; default (missing / empty / parse
/// error) is deny-all.
#[derive(Clone, Default)]
struct WhitelistCache {
    inner: Arc<RwLock<Option<WhitelistEntry>>>,
}

struct WhitelistEntry {
    /// Lowercased emails allowed through. Empty set ⇒ deny-all.
    allowed: HashSet<String>,
    fetched: Instant,
}

impl WhitelistCache {
    /// Return the current allowed set, refreshing from Firestore if the
    /// cache entry is stale. On upstream failure with a cached value we
    /// serve the stale set (stale-while-error). No prior cache and upstream
    /// fails → `ConfigUnavailable`.
    async fn get(
        &self,
        firebase: &FirebaseState,
        http: &reqwest::Client,
    ) -> Result<HashSet<String>, AuthError> {
        {
            let guard = self.inner.read().await;
            if let Some(entry) = guard.as_ref()
                && entry.fetched.elapsed() < WHITELIST_TTL
            {
                return Ok(entry.allowed.clone());
            }
        }
        match firebase.fetch_doc(http, WHITELIST_DOC_PATH, None).await {
            Ok(value) => {
                let allowed = value.as_ref().map(parse_whitelist).unwrap_or_default();
                let mut guard = self.inner.write().await;
                *guard = Some(WhitelistEntry {
                    allowed: allowed.clone(),
                    fetched: Instant::now(),
                });
                Ok(allowed)
            }
            Err(err) => {
                let guard = self.inner.read().await;
                if let Some(entry) = guard.as_ref() {
                    tracing::warn!(?err, "whitelist fetch failed, serving stale cache");
                    Ok(entry.allowed.clone())
                } else {
                    tracing::error!(?err, "whitelist fetch failed and no cache");
                    Err(AuthError::ConfigUnavailable)
                }
            }
        }
    }
}

fn parse_whitelist(doc: &Value) -> HashSet<String> {
    let Some(list) = doc.get("allowed_emails").and_then(Value::as_array) else {
        tracing::error!("config/proxy missing or malformed `allowed_emails`; denying all");
        return HashSet::new();
    };
    list.iter()
        .filter_map(Value::as_str)
        .map(str::to_ascii_lowercase)
        .collect()
}

// --- Errors ---

#[derive(Debug, Clone, Copy)]
pub enum AuthError {
    Unauthorized,
    Forbidden,
    EmailNotAllowed,
    ConfigUnavailable,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, r#"{"error":"unauthorized"}"#),
            Self::Forbidden => (StatusCode::FORBIDDEN, r#"{"error":"google_required"}"#),
            Self::EmailNotAllowed => (StatusCode::FORBIDDEN, r#"{"error":"email_not_allowed"}"#),
            Self::ConfigUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"error":"config_unavailable"}"#,
            ),
        };
        (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FirestoreError {
    /// Caller is not allowed to read the doc under Firestore rules.
    Forbidden,
    /// Network / parsing / 5xx from Firestore.
    Upstream,
}

// --- Claims ---

/// Firebase ID token payload. Production code reads `sub`, `email` and
/// `firebase`; the other standard JWT fields are kept so tests can
/// reconstruct and sign a full payload.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub iat: u64,
    pub exp: u64,
    #[serde(default)]
    pub email: Option<String>,
    pub firebase: FirebaseClaims,
}

impl Claims {
    /// Validate signature + iss/aud/exp against a Firebase project and
    /// extract the claims.
    fn decode(token: &str, key: &DecodingKey, project_id: &str) -> Result<Self, AuthError> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[project_id]);
        validation.set_issuer(&[format!("https://securetoken.google.com/{project_id}")]);
        jsonwebtoken::decode::<Self>(token, key, &validation)
            .map(|data| data.claims)
            .map_err(|_| AuthError::Unauthorized)
    }

    #[cfg(test)]
    fn sign(&self, enc: &jsonwebtoken::EncodingKey) -> String {
        use jsonwebtoken::Header;
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(tests::KID.into());
        jsonwebtoken::encode(&header, self, enc).unwrap()
    }
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct FirebaseClaims {
    pub sign_in_provider: String,
}

// --- Middleware ---

pub async fn require_google(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AuthError::Unauthorized)?;

    let claims = state
        .firebase
        .verify_google_bearer(&state.http, token)
        .await?;
    tracing::debug!(uid = %claims.sub, "authenticated");
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use jsonwebtoken::EncodingKey;
    use rand::rngs::OsRng;
    use rsa::{
        RsaPrivateKey,
        pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey},
    };
    use serde_json::json;

    use super::*;

    pub(super) const PROJECT_ID: &str = "test-project";
    pub(super) const KID: &str = "test-kid";

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn make_keys() -> (EncodingKey, DecodingKey) {
        let private = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public = private.to_public_key();
        let priv_pem = private.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF).unwrap();
        let pub_pem = public.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF).unwrap();
        (
            EncodingKey::from_rsa_pem(priv_pem.as_bytes()).unwrap(),
            DecodingKey::from_rsa_pem(pub_pem.as_bytes()).unwrap(),
        )
    }

    fn valid_claims() -> Claims {
        Claims {
            sub: "user-1".into(),
            iss: format!("https://securetoken.google.com/{PROJECT_ID}"),
            aud: PROJECT_ID.into(),
            iat: now() - 10,
            exp: now() + 3600,
            email: Some("alice@example.com".into()),
            firebase: FirebaseClaims {
                sign_in_provider: "google.com".into(),
            },
        }
    }

    async fn key_for(dec: DecodingKey) -> DecodingKey {
        let cache = JwksCache::default();
        let mut guard = cache.inner.write().await;
        guard.keys.insert(KID.into(), dec);
        guard.refreshed = Some(Instant::now());
        drop(guard);
        cache.get_key(&reqwest::Client::new(), KID).await.unwrap()
    }

    // --- JWT tests ---

    #[tokio::test]
    async fn accepts_google_token() {
        let (enc, dec) = make_keys();
        let token = valid_claims().sign(&enc);
        let claims = Claims::decode(&token, &key_for(dec).await, PROJECT_ID).unwrap();
        assert_eq!(claims.sub, "user-1");
    }

    #[tokio::test]
    async fn rejects_wrong_audience() {
        let (enc, dec) = make_keys();
        let token = Claims {
            aud: "other".into(),
            ..valid_claims()
        }
        .sign(&enc);
        assert!(Claims::decode(&token, &key_for(dec).await, PROJECT_ID).is_err());
    }

    #[tokio::test]
    async fn rejects_expired_token() {
        let (enc, dec) = make_keys();
        let token = Claims {
            exp: now() - 600,
            iat: now() - 1200,
            ..valid_claims()
        }
        .sign(&enc);
        assert!(Claims::decode(&token, &key_for(dec).await, PROJECT_ID).is_err());
    }

    #[tokio::test]
    async fn rejects_bad_signature() {
        let (_, dec) = make_keys();
        let (other_enc, _) = make_keys();
        let token = valid_claims().sign(&other_enc);
        assert!(Claims::decode(&token, &key_for(dec).await, PROJECT_ID).is_err());
    }

    // --- Firestore decoder tests ---

    #[test]
    fn decodes_primitive_fields() {
        let envelope = json!({
            "nullField":    { "nullValue": null },
            "boolField":    { "booleanValue": true },
            "stringField":  { "stringValue": "hello" },
            "intField":     { "integerValue": "42" },
            "doubleField":  { "doubleValue": 2.5 },
        });
        assert_eq!(
            decode_fields(&envelope),
            json!({
                "nullField": null,
                "boolField": true,
                "stringField": "hello",
                "intField": 42,
                "doubleField": 2.5,
            })
        );
    }

    #[test]
    fn decodes_nested_map_and_array() {
        let envelope = json!({
            "identity": {
                "mapValue": {
                    "fields": {
                        "name":  { "stringValue": "Kir" },
                        "level": { "integerValue": "5" }
                    }
                }
            },
            "tags": {
                "arrayValue": {
                    "values": [
                        { "stringValue": "a" },
                        { "stringValue": "b" }
                    ]
                }
            }
        });
        assert_eq!(
            decode_fields(&envelope),
            json!({
                "identity": { "name": "Kir", "level": 5 },
                "tags": ["a", "b"]
            })
        );
    }

    #[test]
    fn empty_array_and_map() {
        let envelope = json!({
            "empty_arr": { "arrayValue": {} },
            "empty_map": { "mapValue": {} },
        });
        assert_eq!(
            decode_fields(&envelope),
            json!({
                "empty_arr": [],
                "empty_map": {}
            })
        );
    }

    #[test]
    fn unknown_kind_passes_through() {
        let envelope = json!({
            "weird": { "futureValue": { "x": 1 } }
        });
        assert_eq!(
            decode_fields(&envelope),
            json!({ "weird": { "futureValue": { "x": 1 } } })
        );
    }

    // --- Whitelist parser tests ---

    #[test]
    fn parse_whitelist_lowercases() {
        let doc = json!({ "allowed_emails": ["Alice@Example.com", "BOB@EXAMPLE.COM"] });
        let set = parse_whitelist(&doc);
        assert!(set.contains("alice@example.com"));
        assert!(set.contains("bob@example.com"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn parse_whitelist_missing_field_is_empty() {
        let doc = json!({ "something_else": [] });
        assert!(parse_whitelist(&doc).is_empty());
    }

    // --- Whitelist cache logic ---

    fn firebase_state_stub() -> FirebaseState {
        FirebaseState {
            project_id: "test-project".into(),
            api_key: "test-key".into(),
            jwks: JwksCache::default(),
            whitelist: WhitelistCache::default(),
        }
    }

    async fn seed_whitelist(firebase: &FirebaseState, emails: impl IntoIterator<Item = &str>) {
        let mut guard = firebase.whitelist.inner.write().await;
        *guard = Some(WhitelistEntry {
            allowed: emails.into_iter().map(str::to_ascii_lowercase).collect(),
            fetched: Instant::now(),
        });
    }

    #[tokio::test]
    async fn whitelist_allows_listed_email() {
        let firebase = firebase_state_stub();
        seed_whitelist(&firebase, ["alice@example.com"]).await;
        let http = reqwest::Client::new();
        firebase
            .check_email(&http, Some("alice@example.com"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn whitelist_rejects_unlisted_email() {
        let firebase = firebase_state_stub();
        seed_whitelist(&firebase, ["alice@example.com"]).await;
        let http = reqwest::Client::new();
        let err = firebase
            .check_email(&http, Some("mallory@example.com"))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::EmailNotAllowed));
    }

    #[tokio::test]
    async fn whitelist_rejects_missing_email() {
        let firebase = firebase_state_stub();
        seed_whitelist(&firebase, ["alice@example.com"]).await;
        let http = reqwest::Client::new();
        let err = firebase.check_email(&http, None).await.unwrap_err();
        assert!(matches!(err, AuthError::EmailNotAllowed));
    }

    #[tokio::test]
    async fn whitelist_rejects_when_empty() {
        let firebase = firebase_state_stub();
        seed_whitelist(&firebase, std::iter::empty::<&str>()).await;
        let http = reqwest::Client::new();
        let err = firebase
            .check_email(&http, Some("alice@example.com"))
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::EmailNotAllowed));
    }

    #[tokio::test]
    async fn whitelist_cache_case_insensitive_email() {
        let firebase = firebase_state_stub();
        seed_whitelist(&firebase, ["alice@example.com"]).await;
        let http = reqwest::Client::new();
        firebase
            .check_email(&http, Some("Alice@Example.COM"))
            .await
            .unwrap();
    }
}
