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

pub fn format_response(username: &str, stats: &GitHubStats, rank: Rank) -> String {
    format!(
        "GitHub Rank Result\n\nUser: {username}\nCommits: {}\nPRs: {}\nStars: {}\n\nCurrent: {}\nCondition: {}\n\n{}\n",
        stats.commits,
        stats.prs,
        stats.stars,
        rank.label(),
        rank.condition(),
        rank.art()
    )
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
