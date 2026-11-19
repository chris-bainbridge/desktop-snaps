use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;

pub const DEFAULT_TAG_PATTERN: &str = r"^v?(\d+(?:\.\d+)*)$";

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub snap: String,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub store: Option<Store>,
    pub upstream: Upstream,
}

#[derive(Debug, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub publisher: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    pub provider: Provider,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default, rename = "tag-pattern")]
    pub tag_pattern: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Github,
    Gitlab,
    Sourceforge,
    Launchpad,
    Git,
    Manual,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Gitlab => "gitlab",
            Self::Sourceforge => "sourceforge",
            Self::Launchpad => "launchpad",
            Self::Git => "git",
            Self::Manual => "manual",
        }
    }

    pub fn is_manual(self) -> bool {
        self == Self::Manual
    }
}

impl Upstream {
    pub fn tag_pattern(&self) -> &str {
        self.tag_pattern.as_deref().unwrap_or(DEFAULT_TAG_PATTERN)
    }

    pub fn default_host(&self) -> Option<&'static str> {
        match self.provider {
            Provider::Github => Some("github.com"),
            Provider::Gitlab => Some("gitlab.com"),
            Provider::Sourceforge => Some("sourceforge.net"),
            Provider::Launchpad => Some("launchpad.net"),
            Provider::Git | Provider::Manual => None,
        }
    }

    pub fn host(&self) -> Option<&str> {
        self.host.as_deref().or_else(|| self.default_host())
    }

    /// A compact one-line description for the dry-run table.
    pub fn display(&self) -> String {
        match self.provider {
            Provider::Manual => self.url.clone().unwrap_or_else(|| "manual".to_string()),
            _ => {
                let project = self.project.as_deref().unwrap_or("?");
                let host = match self.provider {
                    Provider::Github | Provider::Sourceforge => None,
                    _ => self.host(),
                };
                match host {
                    Some(host) => format!("{}:{}/{}", self.provider.as_str(), host, project),
                    None => format!("{}:{}", self.provider.as_str(), project),
                }
            }
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.provider.is_manual() {
            if self.url.is_none() || self.reason.is_none() {
                bail!("manual upstream needs both url and reason");
            }
            if self.host.is_some() || self.project.is_some() || self.tag_pattern.is_some() {
                bail!("manual upstream must not set host, project or tag-pattern");
            }
        } else {
            if self.project.is_none() {
                bail!("{} upstream needs a project", self.provider.as_str());
            }
            if self.url.is_some() || self.reason.is_some() {
                bail!(
                    "{} upstream must not set url or reason",
                    self.provider.as_str()
                );
            }
            if matches!(
                self.provider,
                Provider::Gitlab | Provider::Launchpad | Provider::Git
            ) && self.host.is_none()
            {
                bail!("{} upstream needs a host", self.provider.as_str());
            }
        }

        let pattern = Regex::new(self.tag_pattern())
            .with_context(|| format!("invalid tag-pattern {:?}", self.tag_pattern()))?;
        if pattern.captures_len() != 2 {
            bail!(
                "tag-pattern {:?} must have exactly one capture group",
                self.tag_pattern()
            );
        }
        Ok(())
    }
}

impl Metadata {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        let metadata: Self = serde_yaml::from_str(&text)
            .with_context(|| format!("cannot parse {}", path.display()))?;
        metadata
            .upstream
            .validate()
            .with_context(|| format!("invalid upstream in {}", path.display()))?;
        Ok(metadata)
    }
}
