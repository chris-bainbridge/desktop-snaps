use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::build_cmd::TRACKS;
use crate::cli::AffectedArgs;
use crate::discover;

/// Print the packages changed by a commit range, for a CI build matrix.
pub fn run(root: &Path, args: &AffectedArgs) -> Result<()> {
    if args.max == 0 {
        bail!("--max must be at least 1");
    }

    let packages = discover::discover(root)?;
    let tracks: BTreeMap<String, Vec<String>> = packages
        .iter()
        .map(|package| {
            (
                package.name.clone(),
                package
                    .tracks
                    .iter()
                    .map(|track| track.track.clone())
                    .collect(),
            )
        })
        .collect();

    let forced = !args.select.is_empty();
    let mut selectors = if forced {
        let selected = validate_selectors(&args.select, &tracks)?;
        if selected.is_empty() {
            bail!("--select did not name any package");
        }
        selected
    } else {
        let paths = changed_paths(root, args)?;
        let mut selected = BTreeSet::new();
        for path in &paths {
            selected.extend(selectors_for_path(path, &tracks));
        }
        selected.into_iter().collect()
    };

    if selectors.is_empty() && !args.fallback.is_empty() {
        selectors = validate_selectors(&args.fallback, &tracks)?;
    }

    if selectors.len() > args.max {
        bail!(
            "{} package(s) affected, more than --max {} (GitHub allows 256 matrix entries)",
            selectors.len(),
            args.max
        );
    }

    let json = serde_json::to_string(&selectors)?;
    if args.json {
        println!("{json}");
    } else {
        for selector in &selectors {
            println!("{selector}");
        }
    }

    if let Some(path) = &args.github_output {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        writeln!(file, "matrix={json}")?;
        writeln!(file, "has_work={}", !selectors.is_empty())?;
    }

    Ok(())
}

/// Map a repository path to the build selectors it affects.
///
/// `recipes/<package>/<track>/...` affects only that track; any other path
/// below the package (metadata.yaml, patches, an icon, ...) affects every
/// track, because it is shared by them.
fn selectors_for_path(path: &str, tracks: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let Some(rest) = path.strip_prefix("recipes/") else {
        return Vec::new();
    };
    let mut components = rest.split('/');
    let Some(name) = components.next() else {
        return Vec::new();
    };
    let Some(available) = tracks.get(name) else {
        return Vec::new();
    };

    if let Some(track) = components.next()
        && TRACKS.contains(&track)
        && available.iter().any(|available| available == track)
    {
        return vec![format_selector(name, track)];
    }

    available
        .iter()
        .map(|track| format_selector(name, track))
        .collect()
}

fn format_selector(name: &str, track: &str) -> String {
    if track == "stable" {
        name.to_string()
    } else {
        format!("{name}:{track}")
    }
}

fn validate_selectors(
    raw: &[String],
    tracks: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<String>> {
    let mut selected = BTreeSet::new();
    for item in raw {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (name, track) = match item.split_once(':') {
            Some((name, track)) if !name.is_empty() && !track.is_empty() => (name, Some(track)),
            Some(_) => bail!("invalid package selector {item:?}"),
            None => (item, None),
        };
        let Some(available) = tracks.get(name) else {
            bail!("unknown package {name:?}");
        };
        let track = track.unwrap_or("stable");
        if !available.iter().any(|available| available == track) {
            let available = available
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            bail!("{name}: no {track:?} track (available: {available})");
        }
        selected.insert(format_selector(name, track));
    }
    Ok(selected.into_iter().collect())
}

fn changed_paths(root: &Path, args: &AffectedArgs) -> Result<Vec<String>> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .arg("diff")
        .arg("--name-only")
        .arg("--no-renames")
        .arg("--diff-filter=ACDMRT");
    if args.worktree {
        command.arg("HEAD");
    } else {
        let base = args.base.as_deref().unwrap_or("HEAD^");
        let head = args.head.as_deref().unwrap_or("HEAD");
        command.arg(format!("{base}...{head}"));
    }

    let output = command.output().context("cannot run git")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git diff failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(name, tracks)| {
                (
                    (*name).to_string(),
                    tracks.iter().map(|track| (*track).to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn maps_flat_package_paths() {
        let tracks = tracks(&[("gnome-logs", &["stable"])]);
        assert_eq!(
            selectors_for_path("recipes/gnome-logs/snapcraft.yaml", &tracks),
            vec!["gnome-logs"]
        );
        assert_eq!(
            selectors_for_path("recipes/gnome-logs/patches/fix.patch", &tracks),
            vec!["gnome-logs"]
        );
        assert_eq!(
            selectors_for_path("recipes/gnome-logs/metadata.yaml", &tracks),
            vec!["gnome-logs"]
        );
    }

    #[test]
    fn maps_nested_track_paths() {
        let tracks = tracks(&[("ardour", &["stable", "edge"])]);
        assert_eq!(
            selectors_for_path("recipes/ardour/edge/snapcraft.yaml", &tracks),
            vec!["ardour:edge"]
        );
        assert_eq!(
            selectors_for_path("recipes/ardour/stable/snapcraft.yaml", &tracks),
            vec!["ardour"]
        );
        assert_eq!(
            selectors_for_path("recipes/ardour/metadata.yaml", &tracks),
            vec!["ardour", "ardour:edge"]
        );
        assert_eq!(
            selectors_for_path("recipes/ardour/patches/fix.patch", &tracks),
            vec!["ardour", "ardour:edge"]
        );
    }

    #[test]
    fn ignores_paths_outside_known_packages() {
        let tracks = tracks(&[("gnome-logs", &["stable"])]);
        assert!(selectors_for_path("README.md", &tracks).is_empty());
        assert!(selectors_for_path("recipes/removed/snapcraft.yaml", &tracks).is_empty());
        assert!(selectors_for_path("tool/src/main.rs", &tracks).is_empty());
    }

    #[test]
    fn formats_and_validates_selectors() {
        let tracks = tracks(&[("gnome-logs", &["stable"]), ("ardour", &["stable", "edge"])]);
        assert_eq!(
            validate_selectors(
                &[
                    " gnome-logs ".to_string(),
                    "ardour:edge".to_string(),
                    "ardour".to_string(),
                    "ardour".to_string(),
                    String::new(),
                ],
                &tracks
            )
            .expect("valid selectors"),
            vec!["ardour", "ardour:edge", "gnome-logs"]
        );

        assert!(validate_selectors(&["missing".to_string()], &tracks).is_err());
        assert!(validate_selectors(&["ardour:beta".to_string()], &tracks).is_err());
        assert!(validate_selectors(&["ardour:".to_string()], &tracks).is_err());
    }
}
