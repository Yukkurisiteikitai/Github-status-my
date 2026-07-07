use reqwest::{
    Client,
    header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT},
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::time::Duration;

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

#[derive(Clone)]
pub struct GitHubClient {
    client: Client,
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
            Rank::S => RankRequirements { commits: 1000, prs: 100 },
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

#[derive(Deserialize)]
struct SearchCount {
    total_count: u64,
}

#[derive(Deserialize)]
struct Repo {
    stargazers_count: u64,
}

pub fn help_text() -> &'static str {
    "GitHub Rank API\nGET /rank?user=<username>&bar=true&style=bar&width=800&height=400\n"
}

pub fn normalize_output_size(width: Option<u32>, height: Option<u32>) -> (u32, u32) {
    let width = width.unwrap_or(DEFAULT_OUTPUT_WIDTH).clamp(MIN_OUTPUT_WIDTH, MAX_OUTPUT_WIDTH);
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
"##, width - 1, height - 1));

    // Header
    svg_content.push_str(&format!(
        r##"  <text x="25" y="35" class="header">{}'s GitHub Status</text>
"##, escape_svg(username)));

    // Rank Art (Left side)
    svg_content.push_str(r##"  <g transform="translate(25, 60)">"##);
    let mut art_y = 0u32;
    for line in lines {
        svg_content.push_str(&format!(
            r##"    <text x="0" y="{art_y}" class="ascii-art" xml:space="preserve" dominant-baseline="hanging">{}</text>
"##, escape_svg(line)));
        art_y += line_height;
    }
    svg_content.push_str("  </g>\n");

    // Stats (Right side)
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
"##, label, stats_x + 70, value));
        stats_y += stats_gap;
    }

    // Rank details
    svg_content.push_str(&format!(
        r##"  <g transform="translate({stats_x}, {stats_y})">
    <text x="0" y="8" class="rank-sub">Rank:</text>
    <text x="0" y="32" class="rank-text">{}</text>
  </g>
"##, escape_svg(rank.label())));

    // Progress bar
    if show_progress {
        if let Some(next) = rank.next_rank() {
            let next_reqs = next.requirements();
            let progress_y = height - 25u32;
            let bar_width = 445u32;
            
            svg_content.push_str(&format!(
                r##"  <g transform="translate(25, {progress_y})">
    <text x="0" y="-10" class="progress-label">Next: {} ({} commits / {} PRs)</text>
"##, next.label(), next_reqs.commits, next_reqs.prs));

            let p_commits = if next_reqs.commits > 0 { (stats.commits as f32 / next_reqs.commits as f32).min(1.0) } else { 1.0 };
            let p_prs = if next_reqs.prs > 0 { (stats.prs as f32 / next_reqs.prs as f32).min(1.0) } else { 1.0 };
            let overall_progress = p_commits * 0.7 + p_prs * 0.3; 

            svg_content.push_str(&format!(
                r##"    <rect width="{bar_width}" height="4" rx="2" fill="#eee" />
    <rect width="{}" height="4" rx="2" fill="#000" />
  </g>
"##, (overall_progress * bar_width as f32) as u32));
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

        let client = Client::builder()
            .default_headers(headers)
            .http1_only()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {error}"))?;

        Ok(Self { client })
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
            "https://api.github.com/search/commits?q=author:{username}&per_page=1"
        );
        self.fetch_search_total_count(&url).await
    }

    async fn fetch_pr_count(&self, username: &str) -> Result<u64, String> {
        let url = format!("https://api.github.com/search/issues?q=type:pr+author:{username}&per_page=1");
        self.fetch_search_total_count(&url).await
    }

    async fn fetch_search_total_count(&self, url: &str) -> Result<u64, String> {
        let payload: SearchCount = self.get_json(url).await?;
        Ok(payload.total_count)
    }

    async fn fetch_total_stars(&self, username: &str) -> Result<u64, String> {
        let mut page = 1;
        let mut total = 0u64;

        loop {
            let url = format!(
                "https://api.github.com/users/{username}/repos?per_page=100&page={page}"
            );

            let repos: Vec<Repo> = self.get_json(&url).await?;

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

    async fn get_json<T>(&self, url: &str) -> Result<T, String>
    where
        T: DeserializeOwned,
    {
        let mut last_error = None;

        for attempt in 1..=3 {
            match self.client.get(url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if !status.is_success() {
                        let body = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "<failed to read response body>".to_string());
                        return Err(format!(
                            "request failed with status {status} for {url}: {}",
                            body.trim()
                        ));
                    }

                    return response
                        .json()
                        .await
                        .map_err(|error| format!("invalid JSON response from {url}: {error}"));
                }
                Err(error) => {
                    last_error = Some(format!("attempt {attempt}/3 failed for {url}: {error}"));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| format!("request failed for {url}")))
    }
}
