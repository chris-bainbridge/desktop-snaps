use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use regex::Regex;
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::metadata::{Provider, Upstream};
use crate::version;

#[derive(Debug, Clone)]
pub struct HttpConfig {
    github_token: Option<String>,
    gitlab_token: Option<String>,
    limiter: Arc<RequestLimiter>,
}

#[derive(Debug)]
struct RequestLimiter {
    delay: Duration,
    next: Mutex<Option<Instant>>,
}

impl RequestLimiter {
    fn new(delay: Duration) -> Self {
        Self {
            delay,
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
        *next = Some(Instant::now() + self.delay);
    }
}

impl HttpConfig {
    pub fn from_env() -> Self {
        Self {
            github_token: github_token(),
            gitlab_token: env::var("GITLAB_TOKEN").ok(),
            limiter: Arc::new(RequestLimiter::new(Duration::from_millis(250))),
        }
    }

    async fn wait(&self) {
        self.limiter.wait().await;
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
    config.wait().await;
    let response = request.send().await.context("GitHub request failed")?;
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
    config.wait().await;
    let response = gitlab_request(client.get(&url), config, host)
        .send()
        .await
        .context("GitLab request failed")?;
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
    config.wait().await;
    let tags: Vec<GitlabTag> = parse_json(
        gitlab_request(client.get(&url), config, host)
            .send()
            .await?,
    )
    .await?;
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
    use super::select_latest;
    use regex::Regex;

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
