use reqwest::{
    Client,
    header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT},
};
use serde::Deserialize;

const RANK_D_ART: &str = include_str!("../rank/d.md");
const RANK_C_ART: &str = include_str!("../rank/c.md");
const RANK_B_ART: &str = include_str!("../rank/b.md");
const RANK_A_ART: &str = include_str!("../rank/a.md");
const RANK_S_ART: &str = include_str!("../rank/s.md");

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
    pub stars: u64,
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

    pub fn condition(self) -> &'static str {
        match self {
            Rank::D => "initial state",
            Rank::C => "50 commits + 5 PRs",
            Rank::B => "200 commits + 20 PRs",
            Rank::A => "500 commits + 50 PRs + 10 stars",
            Rank::S => "1000 commits + 100 PRs + 50 stars",
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
            Rank::D => RankRequirements { commits: 0, prs: 0, stars: 0 },
            Rank::C => RankRequirements { commits: 50, prs: 5, stars: 0 },
            Rank::B => RankRequirements { commits: 200, prs: 20, stars: 0 },
            Rank::A => RankRequirements { commits: 500, prs: 50, stars: 10 },
            Rank::S => RankRequirements { commits: 1000, prs: 100, stars: 50 },
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
    "GitHub Rank API\nGET /rank?user=<username>\n"
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

pub fn format_response(username: &str, stats: &GitHubStats, rank: Rank, show_progress: bool, loading_style: Option<&str>) -> String {
    let ascii_art = rank.art();
    let lines = ascii_art.lines().collect::<Vec<_>>();
    
    let mut svg_content = String::new();
    let line_height = 16;
    let width = 1000;
    let height = if show_progress {
        (lines.len() as u32 + 8) * line_height + 150
    } else {
        (lines.len() as u32 + 8) * line_height + 120
    };
    
    svg_content.push_str(&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" style=\"background-color: #ffffff\">"
        , width, height));
    svg_content.push_str("\n  <defs>");
    svg_content.push_str("\n    <style type=\"text/css\"><![CDATA[");
    svg_content.push_str("\n      text { font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', 'Courier New', monospace; font-size: 12px; fill: #000; }");
    svg_content.push_str("\n      .ascii-art { font-size: 11px; white-space: pre; }");
    svg_content.push_str("\n      .info { font-size: 11px; fill: #333; }");
    svg_content.push_str("\n      .progress-label { font-size: 9px; fill: #666; }");
    svg_content.push_str("\n    ]]></style>");
    svg_content.push_str("\n  </defs>");
    svg_content.push_str(&format!("\n  <rect width=\"{}\" height=\"{}\" fill=\"white\" stroke=\"#ddd\" stroke-width=\"1\"/>"
        , width, height));
    
    // Header info
    let info_lines = vec![
        format!("User: {}", username),
        format!("Commits: {}  PRs: {}  Stars: {}", stats.commits, stats.prs, stats.stars),
        format!("Rank: {}  ({})", rank.label(), rank.condition()),
    ];
    
    let mut y = 20;
    for line in &info_lines {
        svg_content.push_str(&format!(
            "\n  <text x=\"980\" y=\"{}\" class=\"info\" text-anchor=\"end\">{}</text>",
            y, escape_svg(line)
        ));
        y += 16;
    }
    
    // Progress bar
    if show_progress {
        if let Some(next) = rank.next_rank() {
            let next_reqs = next.requirements();
            
            y += 10;
            svg_content.push_str(&format!("\n  <text x=\"980\" y=\"{}\" class=\"progress-label\" text-anchor=\"end\">Next: {}</text>", y, next.label()));
            
            let style = loading_style.unwrap_or("bar");
            let row_step: u32 = match style {
                "percentage" => 24,
                "bar" => 22,
                _ => 20,
            };

            y += 14;
            draw_progress_bar(&mut svg_content, 20, 980, y, &format!("Commits: {}/{}", stats.commits, next_reqs.commits), 
                stats.commits, next_reqs.commits, style);
            
            y += row_step;
            draw_progress_bar(&mut svg_content, 20, 980, y, &format!("PRs: {}/{}", stats.prs, next_reqs.prs), 
                stats.prs, next_reqs.prs, style);
            
            if next_reqs.stars > 0 {
                y += row_step;
                draw_progress_bar(&mut svg_content, 20, 980, y, &format!("Stars: {}/{}", stats.stars, next_reqs.stars), 
                    stats.stars, next_reqs.stars, style);
            }
        } else {
            // Rank S has no next tier. Show a one-line cat instead of a progress bar.
            y += 24;
            svg_content.push_str(&format!(
                "\n  <text x=\"980\" y=\"{}\" class=\"progress-label\" text-anchor=\"end\">{}</text>",
                y,
                escape_svg("/\\_/\\ (=^.^=)")
            ));
        }
    }
    
    // ASCII art
    y = 80 + if show_progress { 50 } else { 0 };
    svg_content.push_str("\n  <g>");
    for line in lines {
        svg_content.push_str(&format!(
            "\n    <text x=\"20\" y=\"{}\" class=\"ascii-art\" xml:space=\"preserve\" dominant-baseline=\"hanging\">{}</text>",
            y, escape_svg(line)
        ));
        y += line_height;
    }
    svg_content.push_str("\n  </g>");
    
    svg_content.push_str("\n</svg>");
    svg_content
}

fn draw_progress_bar(svg: &mut String, x: u32, x_end: u32, y: u32, label: &str, current: u64, max: u64, style: &str) {
    let bar_width = 200;
    let bar_height = 10;
    let right_padding = 8;
    let content_right = x_end.saturating_sub(right_padding);
    let progress = if max > 0 {
        ((current as f32 / max as f32) * bar_width as f32).clamp(0.0, bar_width as f32)
    } else {
        bar_width as f32
    };
    
    // ラベルを右に配置
    svg.push_str(&format!("\n  <text x=\"{}\" y=\"{}\" class=\"progress-label\" text-anchor=\"end\">{}</text>", 
        content_right, y, escape_svg(label)));
    
    match style {
        "bar" => {
            // Traditional bar (white fill on black background) - right aligned
            let bar_x = (content_right.saturating_sub(bar_width)).max(x);
            svg.push_str(&format!("\n  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#000\" stroke=\"#333\" stroke-width=\"0.5\"/>", 
                bar_x, y + 5, bar_width, bar_height));
            svg.push_str(&format!("\n  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#fff\" opacity=\"0.9\"/>", 
                bar_x, y + 5, progress as u32, bar_height));
        }
        "blocks" => {
            // ===== ----- style
            let filled = (progress / 10.0) as u32;
            let empty = 20 - filled;
            let block_str = format!("{}{}", 
                "=".repeat(filled as usize),
                "-".repeat(empty as usize)
            );
            svg.push_str(&format!("\n  <text x=\"{}\" y=\"{}\" class=\"progress-label\" font-family=\"monospace\" font-size=\"10px\" text-anchor=\"end\">{}</text>", 
                content_right, y + 12, escape_svg(&block_str)));
        }
        "dots" => {
            // ■■■□□□ style
            let filled = (progress / 10.0) as u32;
            let empty = 20 - filled;
            let dot_str = format!("{}{}", 
                "■".repeat(filled as usize),
                "□".repeat(empty as usize)
            );
            svg.push_str(&format!("\n  <text x=\"{}\" y=\"{}\" class=\"progress-label\" font-family=\"monospace\" font-size=\"10px\" text-anchor=\"end\">{}</text>", 
                content_right, y + 12, escape_svg(&dot_str)));
        }
        "percentage" => {
            // Percentage only
            let percent = if max > 0 { (current as f32 / max as f32 * 100.0) as u32 } else { 0 };
            svg.push_str(&format!("\n  <text x=\"{}\" y=\"{}\" class=\"progress-label\" font-size=\"11px\" text-anchor=\"end\">{:3}%</text>", 
                content_right, y + 14, percent));
        }
        _ => {
            // Default to bar
            let bar_x = (content_right.saturating_sub(bar_width)).max(x);
            svg.push_str(&format!("\n  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#000\" stroke=\"#333\" stroke-width=\"0.5\"/>", 
                bar_x, y + 5, bar_width, bar_height));
            svg.push_str(&format!("\n  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#fff\" opacity=\"0.9\"/>", 
                bar_x, y + 5, progress as u32, bar_height));
        }
    }
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
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| format!("request failed: {error}"))?;

        if !response.status().is_success() {
            return Err(format!("request failed with status {}", response.status()));
        }

        let payload: SearchCount = response
            .json()
            .await
            .map_err(|error| format!("invalid search response: {error}"))?;
        Ok(payload.total_count)
    }

    async fn fetch_total_stars(&self, username: &str) -> Result<u64, String> {
        let mut page = 1;
        let mut total = 0u64;

        loop {
            let url = format!(
                "https://api.github.com/users/{username}/repos?per_page=100&page={page}"
            );

            let response = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|error| format!("request failed: {error}"))?;

            if !response.status().is_success() {
                return Err(format!("request failed with status {}", response.status()));
            }

            let repos: Vec<Repo> = response
                .json()
                .await
                .map_err(|error| format!("invalid repo response: {error}"))?;

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
}
