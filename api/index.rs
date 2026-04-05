#[path = "../src/core.rs"]
mod core;

use std::{collections::HashMap, sync::Arc};
use url::form_urlencoded;
use vercel_runtime::{Body, Error, Request, Response, StatusCode, run};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let github = core::GitHubClient::new().map_err(std::io::Error::other)?;
    let state = Arc::new(github);

    run(move |request| {
        let state = Arc::clone(&state);
        async move { handler(request, state).await }
    })
    .await
}

async fn handler(request: Request, state: Arc<core::GitHubClient>) -> Result<Response<Body>, Error> {
    match request.uri().path() {
        "/" => plain_text(StatusCode::OK, core::help_text().to_string()),
        "/rank" => rank_response(request, state).await,
        _ => plain_text(StatusCode::NOT_FOUND, "Not Found".to_string()),
    }
}

async fn rank_response(
    request: Request,
    state: Arc<core::GitHubClient>,
) -> Result<Response<Body>, Error> {
    let query: HashMap<String, String> = form_urlencoded::parse(
        request.uri().query().unwrap_or_default().as_bytes(),
    )
    .into_owned()
    .collect();

    let user = match query.get("user") {
        Some(value) if !value.is_empty() => value,
        _ => {
            return plain_text(
                StatusCode::BAD_REQUEST,
                "missing query param: user".to_string(),
            );
        }
    };

    let username = match core::validate_github_username(user) {
        Ok(name) => name,
        Err(message) => return plain_text(StatusCode::BAD_REQUEST, message),
    };

    let stats = match state.fetch_stats(&username).await {
        Ok(stats) => stats,
        Err(message) => return plain_text(StatusCode::BAD_GATEWAY, message),
    };

    let rank = core::determine_rank(&stats);
    let body = core::format_response(&username, &stats, rank);
    plain_text(StatusCode::OK, body)
}

fn plain_text(status: StatusCode, body: String) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(body.into())?)
}
