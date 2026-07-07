#[path = "../src/core.rs"]
mod core;

use std::collections::HashMap;
use url::form_urlencoded;
use vercel_runtime::{Error, Request, Response, ResponseBody, run, service_fn};

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenvy::dotenv().ok();
    run(service_fn(handler)).await
}

async fn handler(request: Request) -> Result<Response<ResponseBody>, Error> {
    match request.uri().path() {
        "/" => text_response(200, core::help_text().to_string()),
        "/rank" => rank_response(request).await,
        _ => text_response(404, "Not Found".to_string()),
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
            return text_response(
                400,
                "missing query param: user".to_string(),
            );
        }
    };

    let show_progress = query.get("bar")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);

    let loading_style = query.get("style").map(|s| s.as_str());
    let requested_width = query.get("width").and_then(|v| v.parse::<u32>().ok());
    let requested_height = query.get("height").and_then(|v| v.parse::<u32>().ok());
    let (output_width, output_height) = core::normalize_output_size(requested_width, requested_height);

    let username = match core::validate_github_username(user) {
        Ok(name) => name,
        Err(message) => return text_response(400, message),
    };

    let github = match core::GitHubClient::new() {
        Ok(client) => client,
        Err(message) => {
            return text_response(
                500,
                format!("configuration error: {message}"),
            );
        }
    };

    let stats = match github.fetch_stats(&username).await {
        Ok(stats) => stats,
        Err(message) => return text_response(502, message),
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
    svg_response(200, body)
}

fn text_response(status: u16, body: String) -> Result<Response<ResponseBody>, Error> {
    Ok(Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(body.into())?)
}

fn svg_response(status: u16, body: String) -> Result<Response<ResponseBody>, Error> {
    Ok(Response::builder()
        .status(status)
        .header("content-type", "image/svg+xml; charset=utf-8")
        .body(body.into())?)
}
