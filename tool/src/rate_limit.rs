use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::header::HeaderMap;
use serde::Deserialize;
use tokio::time::sleep;

use crate::quota::{QuotaCache, QuotaEntry, now_epoch};
use crate::upstream::{HttpConfig, format_duration};

const HOSTS: [&str; 4] = [
    "github",
    "gitlab.gnome.org",
    "gitlab.com",
    "gitlab.freedesktop.org",
];

pub async fn run(config: &HttpConfig, refresh: bool) -> Result<()> {
    let mut cache = QuotaCache::load();
    let now = now_epoch();

    println!(
        "{:<24} {:<5} {:<8} {:<10} RESET",
        "HOST", "AUTH", "LIMIT", "REMAINING"
    );

    for host in HOSTS {
        let entry = if !refresh
            && cache
                .hosts
                .get(host)
                .is_some_and(|entry| usable(entry, now))
        {
            cache.hosts.get(host).cloned().unwrap_or_default()
        } else {
            let entry = if host == "github" {
                probe_github(config).await?
            } else {
                probe_gitlab(config, host).await?
            };
            cache.record(host, entry.clone());
            sleep(Duration::from_secs(1)).await;
            entry
        };
        print_entry(host, &entry);
    }

    cache.save()?;
    Ok(())
}

fn usable(entry: &QuotaEntry, now: u64) -> bool {
    entry.checked_at.saturating_add(300) > now && entry.reset.is_none_or(|reset| reset > now)
}

fn print_entry(host: &str, entry: &QuotaEntry) {
    let auth = if entry.authenticated { "yes" } else { "no" };
    let limit = entry
        .limit
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let remaining = entry
        .remaining
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let reset = match entry.blocked_until {
        Some(until) if until > now_epoch() => {
            format!(
                "blocked for {}",
                format_duration(Duration::from_secs(until - now_epoch()))
            )
        }
        _ => match entry.reset {
            Some(reset) if reset > now_epoch() => {
                format!(
                    "in {}",
                    format_duration(Duration::from_secs(reset - now_epoch()))
                )
            }
            Some(_) => "now".to_string(),
            None => "-".to_string(),
        },
    };
    println!(
        "{:<24} {:<5} {:<8} {:<10} {}",
        host, auth, limit, remaining, reset
    );
    if !entry.authenticated {
        let command = if host == "github" {
            "gh auth login".to_string()
        } else {
            format!("glab auth login --hostname {host}")
        };
        println!("  unauthenticated; run `{command}`");
    }
}

async fn probe_github(config: &HttpConfig) -> Result<QuotaEntry> {
    let Some(token) = config.github_token() else {
        return Ok(QuotaEntry {
            checked_at: now_epoch(),
            ..QuotaEntry::default()
        });
    };
    let response = config
        .client()?
        .get("https://api.github.com/rate_limit")
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .context("GitHub rate-limit request failed")?;
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        bail!(
            "GitHub rate-limit request returned {status}: {}",
            text.trim().chars().take(200).collect::<String>()
        );
    }
    let body: GithubRateLimit = serde_json::from_str(&text)?;
    Ok(QuotaEntry {
        authenticated: true,
        limit: Some(body.resources.core.limit),
        remaining: Some(body.resources.core.remaining),
        reset: Some(body.resources.core.reset),
        checked_at: now_epoch(),
        ..QuotaEntry::default()
    })
}

async fn probe_gitlab(config: &HttpConfig, host: &str) -> Result<QuotaEntry> {
    let Some(token) = config.gitlab_token(host) else {
        return Ok(QuotaEntry {
            checked_at: now_epoch(),
            ..QuotaEntry::default()
        });
    };
    let response = config
        .client()?
        .get(format!("https://{host}/api/v4/user"))
        .header("PRIVATE-TOKEN", token)
        .send()
        .await
        .with_context(|| format!("GitLab request to {host} failed"))?;
    let status = response.status();
    let headers = response.headers().clone();
    let text = response.text().await?;
    if status == reqwest::StatusCode::UNAUTHORIZED {
        bail!("{host} rejected the token; run `glab auth login --hostname {host}`");
    }
    if !status.is_success() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
        bail!(
            "{host} returned {status}: {}",
            text.trim().chars().take(200).collect::<String>()
        );
    }

    let reset = header_u64(&headers, "ratelimit-reset");
    let blocked_until = (status == reqwest::StatusCode::TOO_MANY_REQUESTS)
        .then(|| reset.unwrap_or_else(|| now_epoch() + 60));
    Ok(QuotaEntry {
        authenticated: true,
        limit: header_u64(&headers, "ratelimit-limit"),
        remaining: header_u64(&headers, "ratelimit-remaining"),
        observed: header_u64(&headers, "ratelimit-observed"),
        reset,
        checked_at: now_epoch(),
        blocked_until,
    })
}

fn header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers.get(name)?.to_str().ok()?.parse().ok()
}

#[derive(Debug, Deserialize)]
struct GithubRateLimit {
    resources: GithubResources,
}

#[derive(Debug, Deserialize)]
struct GithubResources {
    core: GithubCore,
}

#[derive(Debug, Deserialize)]
struct GithubCore {
    limit: u64,
    remaining: u64,
    reset: u64,
}
