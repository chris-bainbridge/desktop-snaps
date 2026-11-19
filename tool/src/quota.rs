use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuotaEntry {
    #[serde(default)]
    pub authenticated: bool,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub remaining: Option<u64>,
    #[serde(default)]
    pub observed: Option<u64>,
    #[serde(default)]
    pub reset: Option<u64>,
    #[serde(default)]
    pub checked_at: u64,
    #[serde(default)]
    pub blocked_until: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuotaCache {
    #[serde(default)]
    pub hosts: BTreeMap<String, QuotaEntry>,
}

impl QuotaCache {
    pub fn load() -> Self {
        let Some(path) = cache_path() else {
            return Self::default();
        };
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let Some(path) = cache_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        fs::write(&path, text).with_context(|| format!("cannot write {}", path.display()))?;
        Ok(())
    }

    pub fn blocked(&self, host: &str) -> Option<u64> {
        let until = self.hosts.get(host)?.blocked_until?;
        (until > now_epoch()).then_some(until)
    }

    pub fn record(&mut self, host: &str, entry: QuotaEntry) {
        self.hosts.insert(host.to_string(), entry);
    }

    pub fn record_blocked(&mut self, host: &str, until: u64) {
        let entry = self.hosts.entry(host.to_string()).or_default();
        if entry.blocked_until.is_none_or(|current| current < until) {
            entry.blocked_until = Some(until);
        }
        entry.checked_at = now_epoch();
    }
}

pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn cache_path() -> Option<PathBuf> {
    if let Some(directory) = env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(directory).join("snap-repo/rate-limit.json"));
    }
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache/snap-repo/rate-limit.json"))
}
