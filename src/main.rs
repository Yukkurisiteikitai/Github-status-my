mod core;

use axum::{Router, extract::{Query, State}, http::{StatusCode, HeaderMap}, response::IntoResponse, routing::get};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    github: core::GitHubClient,
}

#[derive(Deserialize)]
struct RankQuery {
    user: String,
    #[serde(default)]
    bar: Option<String>,
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let github = match core::GitHubClient::new() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("failed to initialize GitHub client: {error}");
            std::process::exit(1);
        }
    };

    let state = Arc::new(AppState { github });
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/rank", get(rank_handler))
        .with_state(state);

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind server");

    println!("listening on http://{bind_addr}");
    axum::serve(listener, app).await.expect("server error");
}

fn build_response(status: StatusCode, content_type: &str, body: impl Into<String>) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", content_type.parse().unwrap());
    (status, headers, body.into()).into_response()
}

fn text_response(status: StatusCode, body: impl Into<String>) -> axum::response::Response {
    build_response(status, "text/plain; charset=utf-8", body)
}

async fn index_handler() -> impl IntoResponse {
    text_response(StatusCode::OK, core::help_text())
}

async fn rank_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RankQuery>,
) -> impl IntoResponse {
    let show_progress = query.bar
        .as_ref()
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);

    let loading_style = query.style.as_deref();
    let (output_width, output_height) = core::normalize_output_size(query.width, query.height);

    let username = match core::validate_github_username(&query.user) {
        Ok(name) => name,
        Err(message) => return text_response(StatusCode::BAD_REQUEST, message),
    };

    let stats = match state.github.fetch_stats(&username).await {
        Ok(stats) => stats,
        Err(message) => return text_response(StatusCode::BAD_GATEWAY, message),
    };

    let rank = core::determine_rank(&stats);
    let body = core::format_response(
        &username,
        &stats,
        rank,
        show_progress,
        loading_style,
        output_width,
        output_height,
    );
    build_response(StatusCode::OK, "image/svg+xml; charset=utf-8", body)
}
