#[path = "../src/core.rs"]
mod core;

use std::collections::HashMap;
use url::form_urlencoded;
use vercel_runtime::{Error, Request, Response, ResponseBody, run, service_fn};

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

async fn handler(request: Request) -> Result<Response<ResponseBody>, Error> {
    match request.uri().path() {
        "/" => plain_text(200, core::help_text().to_string()),
        "/rank" => rank_response(request).await,
        _ => plain_text(404, "Not Found".to_string()),
    }
}

async fn rank_response(request: Request) -> Result<Response<ResponseBody>, Error> {
    let query: HashMap<String, String> = form_urlencoded::parse(
        request.uri().query().unwrap_or_default().as_bytes(),
    )
    .into_owned()
    .collect();

    let user = match query.get("user") {
        Some(value) if !value.is_empty() => value,
        _ => {
            return plain_text(
                400,
                "missing query param: user".to_string(),
            );
        }
    };

    let username = match core::validate_github_username(user) {
        Ok(name) => name,
        Err(message) => return plain_text(400, message),
    };

    let github = match core::GitHubClient::new() {
        Ok(client) => client,
        Err(message) => {
            return plain_text(
                500,
                format!("configuration error: {message}"),
            );
        }
    };

    let stats = match github.fetch_stats(&username).await {
        Ok(stats) => stats,
        Err(message) => return plain_text(502, message),
    };

    let rank = core::determine_rank(&stats);
    let body = core::format_response(&username, &stats, rank);
    plain_text(200, body)
}

fn plain_text(status: u16, body: String) -> Result<Response<ResponseBody>, Error> {
    Ok(Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(body.into())?)
}
