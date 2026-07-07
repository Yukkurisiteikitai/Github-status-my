use reqwest::{
    Client, Response,
    header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT},
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::error::Error as StdError;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const RANK_D_ART: &str = include_str!("../rank/d.md");
const RANK_C_ART: &str = include_str!("../rank/c.md");
const RANK_B_ART: &str = include_str!("../rank/b.md");
const RANK_A_ART: &str = include_str!("../rank/a.md");
const RANK_S_ART: &str = include_str!("../rank/s.md");
const DEFAULT_OUTPUT_WIDTH: u32 = 820;
const DEFAULT_OUTPUT_HEIGHT: u32 = 400;
const MIN_OUTPUT_WIDTH: u32 = 360;
const MIN_OUTPUT_HEIGHT: u32 = 220;
const MAX_OUTPUT_WIDTH: u32 = 2000;
const MAX_OUTPUT_HEIGHT: u32 = 1400;
const DEFAULT_API_BASE: &str = "https://api.github.com";
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_TIMEOUT_SECS: u64 = 20;
const DEFAULT_HTTP_BACKEND: HttpBackend = HttpBackend::NativeTls;
const DEFAULT_FORCE_HTTP1: bool = true;
const DEFAULT_TRACE: bool = false;
const MAX_ATTEMPTS: u32 = 3;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct GitHubClient {
    client: Client,
    config: HttpClientConfig,
}

#[derive(Debug)]
pub struct GitHubStats {
    pub commits: u64,
    pub prs: u64,
    pub stars: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum Rank {
    D,
    C,
    B,
    A,
    S,
}

#[derive(Debug, Clone, Copy)]
pub struct RankRequirements {
    pub commits: u64,
    pub prs: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HttpBackend {
    NativeTls,
    Rustls,
}

#[derive(Clone, Debug)]
struct HttpClientConfig {
    backend: HttpBackend,
    force_http1: bool,
    timeout_secs: u64,
    trace: bool,
    api_base: String,
}

#[derive(Clone, Copy)]
struct RequestContext<'a> {
    id: u64,
    endpoint: &'a str,
    url: &'a str,
}

struct ResponseMeta {
    version: String,
    x_github_request_id: Option<String>,
    x_ratelimit_limit: Option<String>,
    x_ratelimit_remaining: Option<String>,
    x_ratelimit_used: Option<String>,
    x_ratelimit_resource: Option<String>,
    tls_peer_certificate_len: Option<usize>,
}

#[derive(Deserialize)]
struct SearchCount {
    total_count: u64,
}

#[derive(Deserialize)]
struct Repo {
    stargazers_count: u64,
}

impl Rank {
    pub fn label(self) -> &'static str {
        match self {
            Rank::D => "Rank D (Seed)",
            Rank::C => "Rank C (Sprout)",
            Rank::B => "Rank B (Sapling)",
            Rank::A => "Rank A (Tree)",
            Rank::S => "Rank S (World Tree)",
        }
    }

    pub fn art(self) -> &'static str {
        match self {
            Rank::D => RANK_D_ART,
            Rank::C => RANK_C_ART,
            Rank::B => RANK_B_ART,
            Rank::A => RANK_A_ART,
            Rank::S => RANK_S_ART,
        }
    }

    pub fn requirements(self) -> RankRequirements {
        match self {
            Rank::D => RankRequirements { commits: 0, prs: 0 },
            Rank::C => RankRequirements { commits: 50, prs: 5 },
            Rank::B => RankRequirements { commits: 200, prs: 20 },
            Rank::A => RankRequirements { commits: 500, prs: 50 },
            Rank::S => RankRequirements {
                commits: 1000,
                prs: 100,
            },
        }
    }

    pub fn next_rank(self) -> Option<Rank> {
        match self {
            Rank::D => Some(Rank::C),
            Rank::C => Some(Rank::B),
            Rank::B => Some(Rank::A),
            Rank::A => Some(Rank::S),
            Rank::S => None,
        }
    }
}

impl HttpBackend {
    fn from_env() -> Result<Self, String> {
        match std::env::var("GITHUB_HTTP_BACKEND") {
            Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
                "native" | "native-tls" => Ok(Self::NativeTls),
                "rustls" | "rustls-tls" => Ok(Self::Rustls),
                other => Err(format!(
                    "invalid GITHUB_HTTP_BACKEND: {other}. expected native|native-tls|rustls|rustls-tls"
                )),
            },
            Err(_) => Ok(DEFAULT_HTTP_BACKEND),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::NativeTls => "native-tls",
            Self::Rustls => "rustls",
        }
    }
}

impl HttpClientConfig {
    fn from_env() -> Result<Self, String> {
        Ok(Self {
            backend: HttpBackend::from_env()?,
            force_http1: parse_bool_env("GITHUB_HTTP_FORCE_HTTP1", DEFAULT_FORCE_HTTP1)?,
            timeout_secs: parse_u64_env("GITHUB_HTTP_TIMEOUT_SECS", DEFAULT_TIMEOUT_SECS)?,
            trace: parse_bool_env("GITHUB_HTTP_TRACE", DEFAULT_TRACE)?,
            api_base: normalize_api_base(
                std::env::var("GITHUB_API_BASE")
                    .unwrap_or_else(|_| DEFAULT_API_BASE.to_string()),
            )?,
        })
    }

    fn protocol_mode(&self) -> &'static str {
        if self.force_http1 {
            "http1"
        } else {
            "default"
        }
    }
}

impl fmt::Display for HttpClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "backend={} protocol={} timeout_secs={} api_base={} trace={}",
            self.backend.as_str(),
            self.protocol_mode(),
            self.timeout_secs,
            self.api_base,
            self.trace
        )
    }
}

pub fn help_text() -> &'static str {
    "GitHub Rank API\nGET /rank?user=<username>&bar=true&style=bar&width=800&height=400\n"
}

pub fn normalize_output_size(width: Option<u32>, height: Option<u32>) -> (u32, u32) {
    let width = width
        .unwrap_or(DEFAULT_OUTPUT_WIDTH)
        .clamp(MIN_OUTPUT_WIDTH, MAX_OUTPUT_WIDTH);
    let height = height
        .unwrap_or(DEFAULT_OUTPUT_HEIGHT)
        .clamp(MIN_OUTPUT_HEIGHT, MAX_OUTPUT_HEIGHT);
    (width, height)
}

pub fn validate_github_username(raw: &str) -> Result<String, String> {
    let username = raw.trim().trim_start_matches('@');
    if username.is_empty() {
        return Err("user is empty".to_string());
    }
    if username.len() > 39 {
        return Err("user is too long".to_string());
    }
    if username.starts_with('-') || username.ends_with('-') {
        return Err("user cannot start or end with '-'".to_string());
    }
    if username.contains("--") {
        return Err("user cannot contain consecutive '-'".to_string());
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("user contains invalid characters".to_string());
    }
    Ok(username.to_string())
}

pub fn determine_rank(stats: &GitHubStats) -> Rank {
    if stats.commits >= 1000 && stats.prs >= 100 && stats.stars >= 50 {
        Rank::S
    } else if stats.commits >= 500 && stats.prs >= 50 && stats.stars >= 10 {
        Rank::A
    } else if stats.commits >= 200 && stats.prs >= 20 {
        Rank::B
    } else if stats.commits >= 50 && stats.prs >= 5 {
        Rank::C
    } else {
        Rank::D
    }
}

pub fn format_response(
    username: &str,
    stats: &GitHubStats,
    rank: Rank,
    show_progress: bool,
    _loading_style: Option<&str>,
    _output_width: u32,
    _output_height: u32,
) -> String {
    let ascii_art = rank.art();
    let lines = ascii_art.lines().collect::<Vec<_>>();

    let line_height = 10u32;
    let art_height = (lines.len() as u32) * line_height;

    let width = 495u32;
    let base_height = 120u32;
    let progress_extra = if show_progress { 40u32 } else { 0u32 };
    let height = base_height.max(art_height + 85) + progress_extra;

    let mut svg_content = String::new();

    svg_content.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" fill="none">
  <style>
    .header {{ font: 700 16px 'Segoe UI', Ubuntu, Sans-Serif; fill: #000; }}
    .stat-label {{ font: 400 14px 'Segoe UI', Ubuntu, Sans-Serif; fill: #333; }}
    .stat-value {{ font: 700 14px 'Segoe UI', Ubuntu, Sans-Serif; fill: #000; }}
    .rank-text {{ font: 700 20px 'Segoe UI', Ubuntu, Sans-Serif; fill: #000; }}
    .rank-sub {{ font: 400 12px 'Segoe UI', Ubuntu, Sans-Serif; fill: #666; }}
    .ascii-art {{ font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', 'Courier New', monospace; font-size: 8px; fill: #111; line-height: 1; }}
    .progress-label {{ font: 400 12px 'Segoe UI', Ubuntu, Sans-Serif; fill: #666; }}
  </style>
  <rect x="0.5" y="0.5" rx="4.5" width="{}" height="{}" stroke="#111" stroke-width="1.5" fill="#fff"/>
"##,
        width - 1,
        height - 1
    ));

    svg_content.push_str(&format!(
        r##"  <text x="25" y="35" class="header">{}'s GitHub Status</text>
"##,
        escape_svg(username)
    ));

    svg_content.push_str(r##"  <g transform="translate(25, 60)">"##);
    let mut art_y = 0u32;
    for line in lines {
        svg_content.push_str(&format!(
            r##"    <text x="0" y="{art_y}" class="ascii-art" xml:space="preserve" dominant-baseline="hanging">{}</text>
"##,
            escape_svg(line)
        ));
        art_y += line_height;
    }
    svg_content.push_str("  </g>\n");

    let stats_x = 260u32;
    let mut stats_y = 65u32;
    let stats_gap = 22u32;

    let items = vec![
        ("Commits", stats.commits),
        ("PRs", stats.prs),
        ("Stars", stats.stars),
    ];

    for (label, value) in items {
        svg_content.push_str(&format!(
            r##"  <text x="{stats_x}" y="{stats_y}" class="stat-label">{}:</text>
  <text x="{}" y="{stats_y}" class="stat-value">{}</text>
"##,
            label,
            stats_x + 70,
            value
        ));
        stats_y += stats_gap;
    }

    svg_content.push_str(&format!(
        r##"  <g transform="translate({stats_x}, {stats_y})">
    <text x="0" y="8" class="rank-sub">Rank:</text>
    <text x="0" y="32" class="rank-text">{}</text>
  </g>
"##,
        escape_svg(rank.label())
    ));

    if show_progress {
        if let Some(next) = rank.next_rank() {
            let next_reqs = next.requirements();
            let progress_y = height - 25u32;
            let bar_width = 445u32;

            svg_content.push_str(&format!(
                r##"  <g transform="translate(25, {progress_y})">
    <text x="0" y="-10" class="progress-label">Next: {} ({} commits / {} PRs)</text>
"##,
                next.label(),
                next_reqs.commits,
                next_reqs.prs
            ));

            let p_commits = if next_reqs.commits > 0 {
                (stats.commits as f32 / next_reqs.commits as f32).min(1.0)
            } else {
                1.0
            };
            let p_prs = if next_reqs.prs > 0 {
                (stats.prs as f32 / next_reqs.prs as f32).min(1.0)
            } else {
                1.0
            };
            let overall_progress = p_commits * 0.7 + p_prs * 0.3;

            svg_content.push_str(&format!(
                r##"    <rect width="{bar_width}" height="4" rx="2" fill="#eee" />
    <rect width="{}" height="4" rx="2" fill="#000" />
  </g>
"##,
                (overall_progress * bar_width as f32) as u32
            ));
        }
    }

    svg_content.push_str("\n</svg>");
    svg_content
}

fn escape_svg(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

impl GitHubClient {
    pub fn new() -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("github-rank-api"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );

        let token_raw = std::env::var("GITHUB_PAT")
            .or_else(|_| std::env::var("github_pat"))
            .map_err(|_| "GITHUB_PAT is required. set your GitHub Personal Access Token".to_string())?;
        let token = token_raw.trim();
        if token.is_empty() {
            return Err("GITHUB_PAT is empty".to_string());
        }
        let auth_value = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| "invalid GITHUB_PAT header value".to_string())?;
        headers.insert(AUTHORIZATION, auth_value);

        let config = HttpClientConfig::from_env()?;
        let mut builder = Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(config.timeout_secs))
            .tls_info(true);

        builder = match config.backend {
            HttpBackend::NativeTls => builder.use_native_tls(),
            HttpBackend::Rustls => builder.use_rustls_tls(),
        };

        if config.force_http1 {
            builder = builder.http1_only();
        }

        let client = builder
            .build()
            .map_err(|error| format!("failed to build HTTP client ({config}): {error}"))?;

        eprintln!("[github-http] initialized {config}");

        Ok(Self { client, config })
    }

    pub async fn fetch_stats(&self, username: &str) -> Result<GitHubStats, String> {
        let commits_fut = self.fetch_commit_count(username);
        let prs_fut = self.fetch_pr_count(username);
        let stars_fut = self.fetch_total_stars(username);
        let (commits, prs, stars) = tokio::try_join!(commits_fut, prs_fut, stars_fut)
            .map_err(|message| format!("GitHub API error: {message}"))?;

        Ok(GitHubStats {
            commits,
            prs,
            stars,
        })
    }

    async fn fetch_commit_count(&self, username: &str) -> Result<u64, String> {
        let url = format!(
            "{}/search/commits?q=author:{username}&per_page=1",
            self.config.api_base
        );
        let payload: SearchCount = self.get_json("search_commits", &url).await?;
        Ok(payload.total_count)
    }

    async fn fetch_pr_count(&self, username: &str) -> Result<u64, String> {
        let url = format!(
            "{}/search/issues?q=type:pr+author:{username}&per_page=1",
            self.config.api_base
        );
        let payload: SearchCount = self.get_json("search_issues", &url).await?;
        Ok(payload.total_count)
    }

    async fn fetch_total_stars(&self, username: &str) -> Result<u64, String> {
        let mut page = 1;
        let mut total = 0u64;

        loop {
            let url = format!(
                "{}/users/{username}/repos?per_page=100&page={page}",
                self.config.api_base
            );

            let repos: Vec<Repo> = self.get_json("user_repos", &url).await?;
            if repos.is_empty() {
                break;
            }

            total += repos.iter().map(|repo| repo.stargazers_count).sum::<u64>();

            if repos.len() < 100 {
                break;
            }
            page += 1;
        }

        Ok(total)
    }

    async fn get_json<T>(&self, endpoint: &str, url: &str) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let context = RequestContext {
            id: request_id,
            endpoint,
            url,
        };

        let mut last_error = None;

        for attempt in 1..=MAX_ATTEMPTS {
            let started = Instant::now();
            match self.client.get(url).send().await {
                Ok(response) => {
                    let elapsed_ms = started.elapsed().as_millis();
                    let status = response.status();
                    let meta = response_meta(&response);

                    if !status.is_success() {
                        let body = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "<failed to read response body>".to_string());
                        let message = format!(
                            "request failed with status {status} for {url}: {}",
                            body.trim()
                        );
                        self.log_failure(&context, attempt, elapsed_ms, "http_status", &message, Some(&meta));
                        return Err(message);
                    }

                    if self.config.trace {
                        self.log_success(&context, attempt, elapsed_ms, &meta);
                    }

                    return response
                        .json()
                        .await
                        .map_err(|error| {
                            let elapsed_ms = started.elapsed().as_millis();
                            let message = format!("invalid JSON response from {url}: {error}");
                            self.log_failure(
                                &context,
                                attempt,
                                elapsed_ms,
                                "decode",
                                &message,
                                Some(&meta),
                            );
                            message
                        });
                }
                Err(error) => {
                    let elapsed_ms = started.elapsed().as_millis();
                    let class = classify_reqwest_error(&error);
                    let details = format_reqwest_error(&error);
                    let message = format!("attempt {attempt}/{MAX_ATTEMPTS} failed for {url}: {error}");
                    self.log_failure(&context, attempt, elapsed_ms, class, &details, None);
                    last_error = Some(message);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| format!("request failed for {url}")))
    }

    fn log_success(
        &self,
        context: &RequestContext<'_>,
        attempt: u32,
        elapsed_ms: u128,
        meta: &ResponseMeta,
    ) {
        eprintln!(
            "[github-http] request_id={} outcome=success backend={} protocol={} endpoint={} attempt={}/{} elapsed_ms={} url={} status=200 version={} x_github_request_id={} ratelimit_remaining={} tls_peer_certificate_len={}",
            context.id,
            self.config.backend.as_str(),
            self.config.protocol_mode(),
            context.endpoint,
            attempt,
            MAX_ATTEMPTS,
            elapsed_ms,
            context.url,
            meta.version,
            display_option(meta.x_github_request_id.as_deref()),
            display_option(meta.x_ratelimit_remaining.as_deref()),
            display_usize_option(meta.tls_peer_certificate_len),
        );
    }

    fn log_failure(
        &self,
        context: &RequestContext<'_>,
        attempt: u32,
        elapsed_ms: u128,
        class: &str,
        error_details: &str,
        meta: Option<&ResponseMeta>,
    ) {
        eprintln!(
            "[github-http] request_id={} outcome=error backend={} protocol={} endpoint={} attempt={}/{} elapsed_ms={} class={} url={}",
            context.id,
            self.config.backend.as_str(),
            self.config.protocol_mode(),
            context.endpoint,
            attempt,
            MAX_ATTEMPTS,
            elapsed_ms,
            class,
            context.url,
        );
        eprintln!(
            "[github-http-detail]\nrequest_id={}\nbackend={}\nprotocol={}\nendpoint={}\nattempt={}/{}\nelapsed_ms={}\nurl={}\nerror_chain={}\nresponse_version={}\nx_github_request_id={}\nx_ratelimit_limit={}\nx_ratelimit_remaining={}\nx_ratelimit_used={}\nx_ratelimit_resource={}\ntls_peer_certificate_len={}",
            context.id,
            self.config.backend.as_str(),
            self.config.protocol_mode(),
            context.endpoint,
            attempt,
            MAX_ATTEMPTS,
            elapsed_ms,
            context.url,
            error_details,
            meta.map(|m| m.version.as_str()).unwrap_or("-"),
            meta.and_then(|m| m.x_github_request_id.as_deref()).unwrap_or("-"),
            meta.and_then(|m| m.x_ratelimit_limit.as_deref()).unwrap_or("-"),
            meta.and_then(|m| m.x_ratelimit_remaining.as_deref()).unwrap_or("-"),
            meta.and_then(|m| m.x_ratelimit_used.as_deref()).unwrap_or("-"),
            meta.and_then(|m| m.x_ratelimit_resource.as_deref()).unwrap_or("-"),
            meta.and_then(|m| m.tls_peer_certificate_len).map(|v| v.to_string()).as_deref().unwrap_or("-"),
        );
    }
}

fn parse_bool_env(key: &str, default: bool) -> Result<bool, String> {
    match std::env::var(key) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            other => Err(format!("invalid {key}: {other}. expected true|false")),
        },
        Err(_) => Ok(default),
    }
}

fn parse_u64_env(key: &str, default: u64) -> Result<u64, String> {
    match std::env::var(key) {
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("invalid {key}: {value}. expected integer"))
            .and_then(|parsed| {
                if parsed == 0 {
                    Err(format!("invalid {key}: {value}. expected integer > 0"))
                } else {
                    Ok(parsed)
                }
            }),
        Err(_) => Ok(default),
    }
}

fn normalize_api_base(value: String) -> Result<String, String> {
    let trimmed = value.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return Err("GITHUB_API_BASE cannot be empty".to_string());
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("GITHUB_API_BASE must start with http:// or https://".to_string());
    }
    Ok(trimmed)
}

fn classify_reqwest_error(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else if error.is_request() {
        "request"
    } else if error.is_status() {
        "status"
    } else if error.is_decode() {
        "decode"
    } else if error.is_body() {
        "body"
    } else {
        "unknown"
    }
}

fn format_reqwest_error(error: &reqwest::Error) -> String {
    let mut parts = vec![
        format!("display={error}"),
        format!("is_connect={}", error.is_connect()),
        format!("is_timeout={}", error.is_timeout()),
        format!("is_request={}", error.is_request()),
        format!("is_status={}", error.is_status()),
        format!("is_body={}", error.is_body()),
        format!("is_decode={}", error.is_decode()),
    ];

    if let Some(url) = error.url() {
        parts.push(format!("url={url}"));
    }

    if let Some(status) = error.status() {
        parts.push(format!("status={status}"));
    }

    let mut chain = Vec::new();
    let mut current: Option<&dyn StdError> = Some(error);
    let mut depth = 0;
    while let Some(err) = current {
        chain.push(format!("{depth}:{err}"));
        current = err.source();
        depth += 1;
    }
    parts.push(format!("sources={}", chain.join(" | ")));
    parts.join("; ")
}

fn response_meta(response: &Response) -> ResponseMeta {
    ResponseMeta {
        version: format!("{:?}", response.version()),
        x_github_request_id: header_value(response, "x-github-request-id"),
        x_ratelimit_limit: header_value(response, "x-ratelimit-limit"),
        x_ratelimit_remaining: header_value(response, "x-ratelimit-remaining"),
        x_ratelimit_used: header_value(response, "x-ratelimit-used"),
        x_ratelimit_resource: header_value(response, "x-ratelimit-resource"),
        tls_peer_certificate_len: response
            .extensions()
            .get::<reqwest::tls::TlsInfo>()
            .and_then(|info| info.peer_certificate().map(|cert| cert.len())),
    }
}

fn header_value(response: &Response, key: &str) -> Option<String> {
    response
        .headers()
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

fn display_option(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn display_usize_option(value: Option<usize>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_FORCE_HTTP1, HttpBackend, normalize_api_base, parse_bool_env};

    #[test]
    fn backend_aliases_parse() {
        unsafe {
            std::env::set_var("GITHUB_HTTP_BACKEND", "rustls");
        }
        assert_eq!(HttpBackend::from_env().unwrap(), HttpBackend::Rustls);

        unsafe {
            std::env::set_var("GITHUB_HTTP_BACKEND", "native");
        }
        assert_eq!(HttpBackend::from_env().unwrap(), HttpBackend::NativeTls);

        unsafe {
            std::env::remove_var("GITHUB_HTTP_BACKEND");
        }
    }

    #[test]
    fn bool_env_defaults_and_values() {
        unsafe {
            std::env::remove_var("GITHUB_HTTP_TRACE");
        }
        assert_eq!(
            parse_bool_env("GITHUB_HTTP_TRACE", DEFAULT_FORCE_HTTP1).unwrap(),
            DEFAULT_FORCE_HTTP1
        );

        unsafe {
            std::env::set_var("GITHUB_HTTP_TRACE", "false");
        }
        assert!(!parse_bool_env("GITHUB_HTTP_TRACE", true).unwrap());

        unsafe {
            std::env::remove_var("GITHUB_HTTP_TRACE");
        }
    }

    #[test]
    fn api_base_trims_trailing_slash() {
        let normalized = normalize_api_base("https://api.github.com/".to_string()).unwrap();
        assert_eq!(normalized, "https://api.github.com");
    }
}
