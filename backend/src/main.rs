mod download;
mod firebase;
mod openai;

use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use axum::{
    Router,
    http::{HeaderName, HeaderValue, Method},
    middleware::from_fn_with_state,
    routing::{any, get},
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use crate::firebase::FirebaseState;

#[derive(Clone)]
pub struct AppState {
    pub http: reqwest::Client,
    pub firebase: FirebaseState,
    pub openai_api_key: Arc<str>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let openai_api_key =
        env::var("OPENAI_API_KEY").context("OPENAI_API_KEY env var is required")?;
    let allowed_origins =
        env::var("ALLOWED_ORIGINS").unwrap_or_else(|_| "http://localhost:3000".into());
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);

    let state = AppState {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?,
        firebase: FirebaseState::from_env()?,
        openai_api_key: Arc::from(openai_api_key),
    };

    let app = Router::new()
        .route("/v1/{*path}", any(openai::forward))
        .route_layer(from_fn_with_state(state.clone(), firebase::require_google))
        .route(
            "/d/{uid}/{char_id}/character.json",
            get(download::character),
        )
        .route("/d/{uid}/{char_id}/avatar.webp", get(download::avatar))
        .layer(build_cors(&allowed_origins)?)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "dnd-pc-proxy listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_cors(allowed: &str) -> anyhow::Result<CorsLayer> {
    let origins: Vec<HeaderValue> = allowed
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            origin
                .parse::<HeaderValue>()
                .with_context(|| format!("invalid origin {origin}"))
        })
        .collect::<anyhow::Result<_>>()?;

    Ok(CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            HeaderName::from_static("authorization"),
            HeaderName::from_static("content-type"),
        ])
        .max_age(Duration::from_secs(3600)))
}
