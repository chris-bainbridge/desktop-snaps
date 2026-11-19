use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_yaml::{Mapping, Value};

use crate::metadata::Metadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpMode {
    Tag,
    Url,
    Script,
    SourceOther,
    None,
}

impl BumpMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tag => "tag",
            Self::Url => "url",
            Self::Script => "script",
            Self::SourceOther => "source-other",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPartSource {
    AdoptInfo,
    Name,
}

impl AppPartSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AdoptInfo => "adopt-info",
            Self::Name => "name",
        }
    }
}

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub dir: PathBuf,
    pub metadata: Metadata,
    pub tracks: Vec<Recipe>,
}

#[derive(Debug)]
pub struct Recipe {
    pub path: PathBuf,
    pub track: String,
    pub app_part: String,
    pub app_part_source: AppPartSource,
    pub adopt_info: Option<String>,
    pub source_tag: Option<String>,
    pub version: Option<String>,
    pub mode: BumpMode,
    pub source_tag_count: usize,
}

impl Package {
    pub fn load(dir: &Path) -> Result<Self> {
        let name = dir
            .file_name()
            .and_then(|name| name.to_str())
            .context("package directory has no valid name")?
            .to_string();
        let metadata = Metadata::load(&dir.join("metadata.yaml"))?;
        if metadata.snap != name {
            bail!(
                "metadata snap {:?} does not match directory {:?}",
                metadata.snap,
                name
            );
        }

        let tracks = load_tracks(dir, &name)?;
        Ok(Self {
            name,
            dir: dir.to_path_buf(),
            metadata,
            tracks,
        })
    }
}

impl Recipe {
    pub fn load(path: &Path, track: &str, expected_snap: &str) -> Result<Self> {
        let text =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        let doc: Value = serde_yaml::from_str(&text)
            .with_context(|| format!("cannot parse {}", path.display()))?;
        let map = doc
            .as_mapping()
            .with_context(|| format!("{} is not a mapping", path.display()))?;
        let snap_name = map_get(map, "name")
            .and_then(value_as_string)
            .with_context(|| format!("{} has no name", path.display()))?;
        if snap_name != expected_snap {
            bail!(
                "{} has name {:?}, expected {:?}",
                path.display(),
                snap_name,
                expected_snap
            );
        }

        let parts = map_get(map, "parts")
            .and_then(Value::as_mapping)
            .with_context(|| format!("{} has no parts mapping", path.display()))?;
        let adopt_info = map_get(map, "adopt-info").and_then(value_as_string);
        let app_part = match adopt_info.as_deref() {
            Some(name) => {
                if map_get(parts, name).is_none() {
                    bail!(
                        "{}: adopt-info points at missing part {:?}",
                        path.display(),
                        name
                    );
                }
                name.to_string()
            }
            None => {
                if map_get(parts, expected_snap).is_none() {
                    bail!(
                        "{}: no adopt-info and no part named {:?}",
                        path.display(),
                        expected_snap
                    );
                }
                expected_snap.to_string()
            }
        };
        let part = map_get(parts, &app_part)
            .and_then(Value::as_mapping)
            .with_context(|| format!("{}: part {:?} is not a mapping", path.display(), app_part))?;

        let source_tag = map_get(part, "source-tag").and_then(value_as_string);
        let has_pull = map_get(part, "override-pull").is_some();
        let source = map_get(part, "source");
        let mode = if source_tag.is_some() {
            BumpMode::Tag
        } else if has_pull {
            BumpMode::Script
        } else if source.is_some_and(value_has_version) {
            BumpMode::Url
        } else if source.is_some() {
            BumpMode::SourceOther
        } else {
            BumpMode::None
        };
        let version = map_get(map, "version").and_then(value_as_string);
        let source_tag_count = parts
            .values()
            .filter(|part| {
                part.as_mapping()
                    .is_some_and(|map| map_get(map, "source-tag").is_some())
            })
            .count();

        Ok(Self {
            path: path.to_path_buf(),
            track: track.to_string(),
            app_part,
            app_part_source: if adopt_info.is_some() {
                AppPartSource::AdoptInfo
            } else {
                AppPartSource::Name
            },
            adopt_info,
            source_tag,
            version,
            mode,
            source_tag_count,
        })
    }

    pub fn display_version(&self) -> &str {
        self.version
            .as_deref()
            .or(self.source_tag.as_deref())
            .unwrap_or("-")
    }
}

fn load_tracks(dir: &Path, snap: &str) -> Result<Vec<Recipe>> {
    let flat = find_recipe(dir);
    if let Some(path) = flat {
        for track in ["stable", "candidate", "beta", "edge"] {
            let nested = dir.join(track);
            if nested.is_dir() && find_recipe(&nested).is_some() {
                bail!(
                    "{} mixes a flat recipe with a nested {:?} recipe",
                    dir.display(),
                    track
                );
            }
        }
        return Ok(vec![Recipe::load(&path, "stable", snap)?]);
    }

    if find_recipe(&dir.join("stable")).is_none() {
        bail!("nested package {} has no stable/ recipe", dir.display());
    }

    let mut tracks = Vec::new();
    for track in ["stable", "candidate", "beta", "edge"] {
        let track_dir = dir.join(track);
        if let Some(path) = find_recipe(&track_dir) {
            tracks.push(Recipe::load(&path, track, snap)?);
        }
    }
    if tracks.is_empty() {
        bail!("package {} has no recipes", dir.display());
    }
    Ok(tracks)
}

fn find_recipe(dir: &Path) -> Option<PathBuf> {
    let root = dir.join("snapcraft.yaml");
    if root.is_file() {
        return Some(root);
    }
    let snap = dir.join("snap/snapcraft.yaml");
    if snap.is_file() {
        return Some(snap);
    }
    None
}

fn map_get<'a>(map: &'a Mapping, key: &str) -> Option<&'a Value> {
    map.get(Value::String(key.to_string()))
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_has_version(value: &Value) -> bool {
    match value {
        Value::String(value) => Regex::new(r"\d+\.\d+")
            .expect("version regex")
            .is_match(value),
        Value::Sequence(values) => values.iter().any(value_has_version),
        Value::Mapping(values) => values.values().any(value_has_version),
        _ => false,
    }
}
