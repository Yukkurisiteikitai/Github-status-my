mod core;

use axum::{Router, extract::{Query, State}, http::StatusCode, response::IntoResponse, routing::get};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    github: core::GitHubClient,
}

#[derive(Deserialize)]
struct RankQuery {
    user: String,
}

#[tokio::main]
async fn main() {
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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind server");

    println!("listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await.expect("server error");
}

async fn index_handler() -> impl IntoResponse {
    (StatusCode::OK, core::help_text())
}

async fn rank_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RankQuery>,
) -> impl IntoResponse {
    let username = match core::validate_github_username(&query.user) {
        Ok(name) => name,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };

    let stats = match state.github.fetch_stats(&username).await {
        Ok(stats) => stats,
        Err(message) => return (StatusCode::BAD_GATEWAY, message).into_response(),
    };

    let rank = core::determine_rank(&stats);
    let body = core::format_response(&username, &stats, rank);
    (StatusCode::OK, body).into_response()
}
