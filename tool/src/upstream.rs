use std::env;
use std::sync::Arc;
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

const REQUEST_DELAY: Duration = Duration::from_secs(1);
const MAX_COOLDOWN: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct HttpConfig {
    github_token: Option<String>,
    gitlab_token: Option<String>,
    limiter: Arc<RequestLimiter>,
}

#[derive(Debug)]
struct RequestLimiter {
    next: Mutex<Option<Instant>>,
    blocked_until: Mutex<Option<Instant>>,
}

impl RequestLimiter {
    fn new() -> Self {
        Self {
            next: Mutex::new(None),
            blocked_until: Mutex::new(None),
        }
    }

    async fn wait(&self, delay: Duration) -> Result<()> {
        {
            let mut blocked = self.blocked_until.lock().await;
            if let Some(until) = *blocked {
                let now = Instant::now();
                if until > now {
                    let remaining = until - now;
                    if remaining > MAX_COOLDOWN {
                        bail!("rate limited for another {}s", remaining.as_secs());
                    }
                    if remaining > Duration::from_secs(1) {
                        eprintln!(
                            "snap-repo: rate limited; waiting {}s before the next request",
                            remaining.as_secs()
                        );
                    }
                    sleep(remaining).await;
                }
                *blocked = None;
            }
        }

        let mut next = self.next.lock().await;
        let now = Instant::now();
        if let Some(at) = *next
            && at > now
        {
            sleep(at - now).await;
        }
        *next = Some(Instant::now() + delay);
        Ok(())
    }

    async fn cooldown(&self, delay: Duration) {
        if delay.is_zero() {
            return;
        }
        let mut blocked = self.blocked_until.lock().await;
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
            gitlab_token: env::var("GITLAB_TOKEN").ok(),
            limiter: Arc::new(RequestLimiter::new()),
        }
    }

    async fn wait(&self) -> Result<()> {
        self.limiter.wait(REQUEST_DELAY).await
    }

    async fn note_rate_limit(&self, response: &reqwest::Response) {
        let status = response.status();
        let primary_github_limit = status == StatusCode::FORBIDDEN
            && (response
                .headers()
                .get("x-ratelimit-remaining")
                .and_then(|value| value.to_str().ok())
                == Some("0")
                || response.headers().contains_key("retry-after"));
        if status == StatusCode::TOO_MANY_REQUESTS || primary_github_limit {
            let delay =
                rate_limit_delay(response.headers()).unwrap_or_else(|| Duration::from_secs(60));
            self.limiter.cooldown(delay).await;
        }
    }

    pub fn client(&self) -> Result<Client> {
        Client::builder()
            .user_agent(format!("snap-repo/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .context("cannot create HTTP client")
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

    match upstream.provider {
        Provider::Github => {
            let project = upstream
                .project
                .as_deref()
                .context("github project missing")?;
            if let Some(tag) = github_latest_release(client, config, project).await?
                && let Some(latest) = select_latest(vec![tag], &pattern)
            {
                return Ok(latest);
            }
            let tags = git_tags_url(&format!("https://github.com/{project}.git")).await?;
            select_latest(tags, &pattern)
                .with_context(|| format!("no tag matches {:?}", upstream.tag_pattern()))
        }
        Provider::Gitlab => {
            let releases = gitlab_releases(client, config, upstream).await?;
            if let Some(latest) = select_latest(releases, &pattern) {
                return Ok(latest);
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
    config.wait().await?;
    let response = request.send().await.context("GitHub request failed")?;
    config.note_rate_limit(&response).await;
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
    config.wait().await?;
    let response = gitlab_request(client.get(&url), config, host)
        .send()
        .await
        .context("GitLab request failed")?;
    config.note_rate_limit(&response).await;
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
    config.wait().await?;
    let response = gitlab_request(client.get(&url), config, host)
        .send()
        .await?;
    config.note_rate_limit(&response).await;
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
    if host == "gitlab.com"
        && let Some(token) = &config.gitlab_token
    {
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
    use super::{rate_limit_delay, select_latest};
    use regex::Regex;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
