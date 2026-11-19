use std::collections::HashMap;
use std::env;
use std::fmt;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use regex::Regex;
use reqwest::header::HeaderMap;
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::metadata::{Provider, Upstream};
use crate::version;

const REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const MAX_COOLDOWN: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct RateLimited {
    pub provider: &'static str,
    pub remaining: Duration,
}

impl fmt::Display for RateLimited {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} rate limited; retry in {}",
            self.provider,
            format_duration(self.remaining)
        )
    }
}

impl std::error::Error for RateLimited {}

pub fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds >= 3600 {
        format!("{}h{}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m{}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

#[derive(Debug, Clone)]
pub struct HttpConfig {
    github_token: Option<String>,
    gitlab_tokens: GitlabTokens,
    pacer: Arc<RequestPacer>,
    github_blocked: Arc<BlockedState>,
    gitlab_blocked: Arc<BlockedState>,
}

#[derive(Debug, Clone, Default)]
pub struct GitlabTokens {
    cache: Arc<StdMutex<HashMap<String, Option<String>>>>,
}

impl GitlabTokens {
    fn new() -> Self {
        Self::default()
    }

    fn get(&self, host: &str) -> Option<String> {
        if let Some(value) = self.cache.lock().unwrap().get(host) {
            return value.clone();
        }
        let token = resolve_gitlab_token(host);
        self.cache
            .lock()
            .unwrap()
            .insert(host.to_string(), token.clone());
        token
    }
}

fn resolve_gitlab_token(host: &str) -> Option<String> {
    let env_key = format!(
        "GITLAB_TOKEN_{}",
        host.to_uppercase()
            .chars()
            .map(|character| if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            })
            .collect::<String>()
    );
    if let Ok(token) = env::var(&env_key)
        && !token.trim().is_empty()
    {
        return Some(token.trim().to_string());
    }
    if host == "gitlab.com"
        && let Ok(token) = env::var("GITLAB_TOKEN")
        && !token.trim().is_empty()
    {
        return Some(token.trim().to_string());
    }

    let output = std::process::Command::new("glab")
        .args(["config", "get", "token", "--host", host, "--global"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if token.is_empty() { None } else { Some(token) }
}

#[derive(Debug)]
struct RequestPacer {
    interval: Duration,
    next: Mutex<Option<Instant>>,
}

impl RequestPacer {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            next: Mutex::new(None),
        }
    }

    async fn wait(&self) {
        let mut next = self.next.lock().await;
        let now = Instant::now();
        if let Some(at) = *next
            && at > now
        {
            sleep(at - now).await;
        }
        *next = Some(Instant::now() + self.interval);
    }
}

#[derive(Debug)]
struct BlockedState {
    until: Mutex<Option<Instant>>,
}

impl BlockedState {
    fn new() -> Self {
        Self {
            until: Mutex::new(None),
        }
    }

    async fn wait(&self, provider: &'static str) -> Result<()> {
        let mut blocked = self.until.lock().await;
        if let Some(until) = *blocked {
            let now = Instant::now();
            if until > now {
                let remaining = until - now;
                if remaining > MAX_COOLDOWN {
                    return Err(RateLimited {
                        provider,
                        remaining,
                    }
                    .into());
                }
                if remaining > Duration::from_secs(1) {
                    eprintln!(
                        "snap-repo: {provider} rate limited; waiting {} before the next request",
                        format_duration(remaining)
                    );
                }
                sleep(remaining).await;
            }
            *blocked = None;
        }
        Ok(())
    }

    async fn remaining(&self) -> Option<Duration> {
        let blocked = self.until.lock().await;
        let now = Instant::now();
        blocked.and_then(|until| (until > now).then(|| until - now))
    }

    async fn cooldown(&self, delay: Duration) {
        if delay.is_zero() {
            return;
        }
        let mut blocked = self.until.lock().await;
        let until = Instant::now() + delay;
        if blocked.is_none_or(|current| current < until) {
            *blocked = Some(until);
        }
    }
}

impl HttpConfig {
    pub fn from_env() -> Self {
        Self {
            github_token: github_token(),
            gitlab_tokens: GitlabTokens::new(),
            pacer: Arc::new(RequestPacer::new(REQUEST_INTERVAL)),
            github_blocked: Arc::new(BlockedState::new()),
            gitlab_blocked: Arc::new(BlockedState::new()),
        }
    }

    pub fn has_github_token(&self) -> bool {
        self.github_token.is_some()
    }

    pub fn github_token(&self) -> Option<&str> {
        self.github_token.as_deref()
    }

    pub fn gitlab_token(&self, host: &str) -> Option<String> {
        self.gitlab_tokens.get(host)
    }

    pub fn has_gitlab_token(&self, host: &str) -> bool {
        self.gitlab_token(host).is_some()
    }

    async fn wait_github(&self) -> Result<()> {
        self.github_blocked.wait("GitHub").await?;
        self.pacer.wait().await;
        Ok(())
    }

    async fn wait_gitlab(&self) -> Result<()> {
        self.gitlab_blocked.wait("GitLab").await?;
        self.pacer.wait().await;
        Ok(())
    }

    async fn note_github(&self, response: &reqwest::Response) {
        note_rate_limit(&self.github_blocked, response, true).await;
    }

    async fn note_gitlab(&self, response: &reqwest::Response) {
        note_rate_limit(&self.gitlab_blocked, response, false).await;
    }

    pub async fn blocked(&self, provider: Provider) -> Option<RateLimited> {
        let (provider, blocked) = match provider {
            Provider::Github => ("GitHub", &self.github_blocked),
            Provider::Gitlab => ("GitLab", &self.gitlab_blocked),
            _ => return None,
        };
        let remaining = blocked.remaining().await?;
        if remaining <= MAX_COOLDOWN {
            return None;
        }
        Some(RateLimited {
            provider,
            remaining,
        })
    }

    pub fn client(&self) -> Result<Client> {
        Client::builder()
            .user_agent(format!("snap-repo/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .context("cannot create HTTP client")
    }
}

async fn note_rate_limit(blocked: &BlockedState, response: &reqwest::Response, github: bool) {
    let status = response.status();
    let headers = response.headers();
    let remaining = header_seconds(
        headers,
        if github {
            "x-ratelimit-remaining"
        } else {
            "ratelimit-remaining"
        },
    );
    let limited = if github {
        status == StatusCode::TOO_MANY_REQUESTS
            || (status == StatusCode::FORBIDDEN
                && (remaining == Some(0) || headers.contains_key("retry-after")))
    } else {
        status == StatusCode::TOO_MANY_REQUESTS
    };
    let exhausted = remaining == Some(0);
    if limited || exhausted {
        let delay = rate_limit_delay(headers).unwrap_or_else(|| Duration::from_secs(60));
        blocked.cooldown(delay).await;
    }
}

fn github_token() -> Option<String> {
    env::var("GITHUB_TOKEN")
        .ok()
        .or_else(|| env::var("GH_TOKEN").ok())
        .or_else(gh_auth_token)
}

fn gh_auth_token() -> Option<String> {
    let output = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if token.is_empty() { None } else { Some(token) }
}

#[derive(Debug, Clone)]
pub struct Latest {
    pub tag: String,
    pub version: String,
}

pub async fn latest_tag(
    client: &Client,
    config: &HttpConfig,
    upstream: &Upstream,
) -> Result<Latest> {
    let pattern = Regex::new(upstream.tag_pattern())
        .with_context(|| format!("invalid tag-pattern {:?}", upstream.tag_pattern()))?;
    // A custom pattern is an explicit stable-tag filter, so prefer tags.
    // The default pattern includes numeric development tags, so releases
    // are the safer source first.
    let prefer_releases = upstream.tag_pattern.is_none();

    match upstream.provider {
        Provider::Github => {
            let project = upstream
                .project
                .as_deref()
                .context("github project missing")?;
            if prefer_releases
                && let Some(tag) = github_latest_release(client, config, project).await?
                && let Some(latest) = select_latest(vec![tag], &pattern)
            {
                return Ok(latest);
            }
            let tags = git_tags_url(&format!("https://github.com/{project}.git")).await?;
            select_latest(tags, &pattern)
                .with_context(|| format!("no tag matches {:?}", upstream.tag_pattern()))
        }
        Provider::Gitlab => {
            if prefer_releases {
                let releases = gitlab_releases(client, config, upstream).await?;
                if let Some(latest) = select_latest(releases, &pattern) {
                    return Ok(latest);
                }
            }
            let tags = gitlab_repository_tags(client, config, upstream).await?;
            select_latest(tags, &pattern)
                .with_context(|| format!("no tag matches {:?}", upstream.tag_pattern()))
        }
        Provider::Git | Provider::Launchpad => {
            let tags = git_tags(upstream).await?;
            select_latest(tags, &pattern)
                .with_context(|| format!("no tag matches {:?}", upstream.tag_pattern()))
        }
        Provider::Sourceforge => bail!("sourceforge fetching is not implemented yet"),
        Provider::Manual => bail!("manual upstream has no release feed"),
    }
}

fn select_latest(tags: Vec<String>, pattern: &Regex) -> Option<Latest> {
    let mut candidates: Vec<Latest> = tags
        .into_iter()
        .filter_map(|tag| {
            let captures = pattern.captures(&tag)?;
            let version = captures.get(1)?.as_str().to_string();
            Some(Latest { tag, version })
        })
        .collect();
    candidates.sort_by(|left, right| {
        version::compare(&left.version, &right.version).then_with(|| left.tag.cmp(&right.tag))
    });
    candidates.pop()
}

async fn github_latest_release(
    client: &Client,
    config: &HttpConfig,
    project: &str,
) -> Result<Option<String>> {
    let url = format!("https://api.github.com/repos/{project}/releases/latest");
    let request = github_request(client.get(&url), config)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    config.wait_github().await?;
    let response = request.send().await.context("GitHub request failed")?;
    config.note_github(&response).await;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let release: GithubRelease = parse_json(response).await?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    Ok(Some(release.tag_name))
}

async fn gitlab_releases(
    client: &Client,
    config: &HttpConfig,
    upstream: &Upstream,
) -> Result<Vec<String>> {
    let host = upstream.host().context("gitlab host missing")?;
    let project = upstream
        .project
        .as_deref()
        .context("gitlab project missing")?;
    let encoded = utf8_percent_encode(project, NON_ALPHANUMERIC).to_string();
    let url = format!("https://{host}/api/v4/projects/{encoded}/releases?per_page=100");
    config.wait_gitlab().await?;
    let response = gitlab_request(client.get(&url), config, host)
        .send()
        .await
        .context("GitLab request failed")?;
    config.note_gitlab(&response).await;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    let releases: Vec<GitlabRelease> = parse_json(response).await?;
    Ok(releases
        .into_iter()
        .map(|release| release.tag_name)
        .collect())
}

async fn gitlab_repository_tags(
    client: &Client,
    config: &HttpConfig,
    upstream: &Upstream,
) -> Result<Vec<String>> {
    let host = upstream.host().context("gitlab host missing")?;
    let project = upstream
        .project
        .as_deref()
        .context("gitlab project missing")?;
    let encoded = utf8_percent_encode(project, NON_ALPHANUMERIC).to_string();
    let url = format!("https://{host}/api/v4/projects/{encoded}/repository/tags?per_page=100");
    config.wait_gitlab().await?;
    let response = gitlab_request(client.get(&url), config, host)
        .send()
        .await?;
    config.note_gitlab(&response).await;
    let tags: Vec<GitlabTag> = parse_json(response).await?;
    Ok(tags.into_iter().map(|tag| tag.name).collect())
}

async fn git_tags(upstream: &Upstream) -> Result<Vec<String>> {
    let host = upstream.host().context("git host missing")?;
    let project = upstream.project.as_deref().context("git project missing")?;
    let url = match upstream.provider {
        Provider::Launchpad => format!("https://{host}/{project}"),
        _ => format!("https://{host}/{project}.git"),
    };
    git_tags_url(&url).await
}

async fn git_tags_url(url: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["ls-remote", "--tags", "--refs", url])
        .output()
        .await
        .with_context(|| format!("cannot run git ls-remote for {url}"))?;
    if !output.status.success() {
        bail!(
            "git ls-remote {url} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let mut tags = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut fields = line.split_whitespace();
        let _sha = fields.next();
        if let Some(tag) = fields
            .next()
            .and_then(|name| name.strip_prefix("refs/tags/"))
        {
            tags.push(tag.to_string());
        }
    }
    Ok(tags)
}

fn github_request(request: RequestBuilder, config: &HttpConfig) -> RequestBuilder {
    match &config.github_token {
        Some(token) => request.bearer_auth(token),
        None => request,
    }
}

fn gitlab_request(request: RequestBuilder, config: &HttpConfig, host: &str) -> RequestBuilder {
    if let Some(token) = config.gitlab_token(host) {
        request.header("PRIVATE-TOKEN", token)
    } else {
        request
    }
}

fn rate_limit_delay(headers: &HeaderMap) -> Option<Duration> {
    if let Some(seconds) = header_seconds(headers, "retry-after") {
        return Some(Duration::from_secs(seconds));
    }
    if let Some(reset) = header_seconds(headers, "x-ratelimit-reset") {
        return Some(until_epoch(reset));
    }
    if let Some(reset) = header_seconds(headers, "ratelimit-reset") {
        return Some(if reset > 1_000_000_000 {
            until_epoch(reset)
        } else {
            Duration::from_secs(reset)
        });
    }
    None
}

fn header_seconds(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers.get(name)?.to_str().ok()?.parse().ok()
}

fn until_epoch(epoch: u64) -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    Duration::from_secs(epoch.saturating_sub(now))
}

async fn parse_json<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    let status = response.status();
    let text = response.text().await.context("cannot read HTTP response")?;
    if !status.is_success() {
        let body: String = text.trim().chars().take(300).collect();
        bail!("HTTP {status}: {body}");
    }
    serde_json::from_str(&text).context("cannot parse JSON response")
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Debug, Deserialize)]
struct GitlabRelease {
    tag_name: String,
}

#[derive(Debug, Deserialize)]
struct GitlabTag {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::{HttpConfig, rate_limit_delay, select_latest};
    use crate::metadata::Provider;
    use regex::Regex;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn provider_rate_limits_are_independent() {
        let config = HttpConfig {
            github_token: None,
            gitlab_tokens: super::GitlabTokens::new(),
            pacer: Arc::new(super::RequestPacer::new(Duration::from_secs(2))),
            github_blocked: Arc::new(super::BlockedState::new()),
            gitlab_blocked: Arc::new(super::BlockedState::new()),
        };
        config
            .gitlab_blocked
            .cooldown(Duration::from_secs(30))
            .await;
        assert!(config.blocked(Provider::Gitlab).await.is_none());
        config
            .gitlab_blocked
            .cooldown(Duration::from_secs(300))
            .await;
        let limit = config.blocked(Provider::Gitlab).await.unwrap();
        assert_eq!(limit.provider, "GitLab");
        assert!(config.blocked(Provider::Github).await.is_none());
    }

    #[tokio::test]
    async fn pacer_spaces_requests() {
        let pacer = super::RequestPacer::new(Duration::from_millis(50));
        let start = std::time::Instant::now();
        pacer.wait().await;
        pacer.wait().await;
        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn parses_retry_after() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("retry-after"),
            HeaderValue::from_static("30"),
        );
        assert_eq!(rate_limit_delay(&headers), Some(Duration::from_secs(30)));
    }

    #[test]
    fn parses_github_reset_epoch() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(now + 120).to_string()).unwrap(),
        );
        let delay = rate_limit_delay(&headers).unwrap().as_secs();
        assert!((118..=120).contains(&delay), "unexpected delay: {delay}");
    }

    #[test]
    fn parses_gitlab_reset_seconds() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("ratelimit-reset"),
            HeaderValue::from_static("45"),
        );
        assert_eq!(rate_limit_delay(&headers), Some(Duration::from_secs(45)));
    }

    #[test]
    fn picks_the_numerically_latest_tag() {
        let pattern = Regex::new(r"^v?(\d+(?:\.\d+)*)$").unwrap();
        let tags = vec![
            "v1.9.0".to_string(),
            "v1.10.0".to_string(),
            "v1.2.0".to_string(),
            "not-a-version".to_string(),
        ];
        let latest = select_latest(tags, &pattern).unwrap();
        assert_eq!(latest.tag, "v1.10.0");
        assert_eq!(latest.version, "1.10.0");
    }

    #[test]
    fn applies_a_decorated_pattern() {
        let pattern = Regex::new(r"^release-(\d+(?:\.\d+)*)$").unwrap();
        let tags = vec![
            "release-4.7.0".to_string(),
            "release-4.7.1".to_string(),
            "v4.7.2".to_string(),
        ];
        let latest = select_latest(tags, &pattern).unwrap();
        assert_eq!(latest.tag, "release-4.7.1");
    }

    #[test]
    fn returns_none_when_nothing_matches() {
        let pattern = Regex::new(r"^v(\d+)$").unwrap();
        assert!(select_latest(vec!["nope".to_string()], &pattern).is_none());
    }
}
