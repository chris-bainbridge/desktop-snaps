use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use crate::cli::BuildArgs;
use crate::metadata::BuildMode;
use crate::package::{Package, Recipe};

const DEFAULT_TRACK: &str = "stable";
const TRACKS: [&str; 4] = ["stable", "candidate", "beta", "edge"];

/// Build a snap from a recipe in this repository.
pub fn run(root: &Path, args: &BuildArgs) -> Result<()> {
    if args.all {
        bail!("build --all is not implemented yet");
    }
    if args.packages.len() != 1 {
        bail!("build takes exactly one package until --all lands");
    }
    if args.jobs != 1 {
        bail!("build --jobs is not implemented yet");
    }

    let (package, track) = resolve(root, &args.packages[0], args.track.as_deref())?;
    let recipe = package
        .tracks
        .iter()
        .find(|recipe| recipe.track == track)
        .expect("resolve() returned an existing track");

    if package.metadata.mode != BuildMode::Vendored {
        bail!(
            "{}: native build does not support mode: {} yet",
            package.name,
            package.metadata.mode.as_str()
        );
    }
    if package.metadata.prepare.is_some() {
        bail!(
            "{}: native build does not support prepare: yet",
            package.name
        );
    }
    if package.metadata.overlay_repo.is_some()
        || package.metadata.overlay_commit.is_some()
        || package.metadata.overlay_to.is_some()
    {
        bail!(
            "{}: overlay-repo, overlay-commit and overlay-to are not supported yet",
            package.name
        );
    }

    if args.dry_run {
        print_plan(root, &package, recipe, &args.snapcraft_args);
        return Ok(());
    }

    let interrupted = signal_flag()?;
    let mut temp = Some(
        tempfile::Builder::new()
            .prefix(&format!("snap-repo-{}-", package.name))
            .tempdir_in("/tmp")
            .context("cannot create temporary build directory in /tmp")?,
    );
    let temp_path = temp
        .as_ref()
        .expect("temporary directory")
        .path()
        .to_path_buf();

    let build_result = build(root, &package, recipe, &temp_path, args, &interrupted);

    let cleanup_error = if !args.keep && recipe_present(&temp_path) {
        eprintln!("build: cleaning the LXD build container");
        clean_build_env(&temp_path).err()
    } else {
        None
    };

    if let Some(error) = cleanup_error {
        let path = temp.take().expect("temporary directory").keep();
        eprintln!(
            "build: error: snapcraft clean failed; workspace kept: {}",
            path.display()
        );
        return Err(error);
    }

    if args.keep {
        let path = temp.take().expect("temporary directory").keep();
        eprintln!(
            "build: workspace and LXD build container kept: {}",
            path.display()
        );
    }

    match build_result {
        Ok(()) => Ok(()),
        Err(error) => {
            if !args.keep {
                eprintln!(
                    "build: build failed; temporary workspace removed (use --keep to retain it)"
                );
            }
            Err(error)
        }
    }
}

fn build(
    root: &Path,
    package: &Package,
    recipe: &Recipe,
    temp_path: &Path,
    args: &BuildArgs,
    interrupted: &AtomicBool,
) -> Result<()> {
    eprintln!(
        "build: staging {} in {}",
        relative(root, &recipe.project_dir).display(),
        temp_path.display()
    );
    stage(recipe, temp_path)?;
    check_interrupted(interrupted)?;

    eprintln!(
        "build: building {} ({}) with snapcraft pack --use-lxd",
        package.name, recipe.track
    );
    let status = run_snapcraft_pack(temp_path, &args.snapcraft_args, interrupted)?;
    if !status.success() {
        bail!("snapcraft pack exited with {status}");
    }

    let artifact = newest_snap(temp_path)?.with_context(|| {
        format!(
            "snapcraft finished but no .snap was found in {}",
            temp_path.display()
        )
    })?;
    let output_dir = root.join("snaps");
    let final_path = install_artifact(&artifact, &output_dir)?;
    let size = fs::metadata(&final_path)
        .map(|metadata| format_size(metadata.len()))
        .unwrap_or_else(|_| "unknown".to_string());
    eprintln!("build: built {} ({})", final_path.display(), size);
    println!("{}", final_path.display());
    Ok(())
}

/// Resolve a package argument against the repository.
fn resolve(root: &Path, raw: &str, flag_track: Option<&str>) -> Result<(Package, String)> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("empty package name");
    }

    let (name, inline_track) = match raw.split_once(':') {
        Some((name, track)) if !name.is_empty() && !track.is_empty() => {
            (name.to_string(), Some(track.to_string()))
        }
        _ => (raw.to_string(), None),
    };

    let path_track = path_track(root, &name);
    if let Some(inline) = inline_track.as_deref() {
        if let Some(flag) = flag_track {
            bail!("track given twice (--track '{flag}' and ':{inline}')");
        }
        if let Some(path) = path_track.as_deref()
            && inline != path
        {
            bail!("track '{inline}' does not match track '{path}' in '{name}'");
        }
    }
    if let (Some(flag), Some(path)) = (flag_track, path_track.as_deref())
        && flag != path
    {
        bail!("track '{flag}' does not match track '{path}' in '{name}'");
    }

    let package_dir = resolve_package_dir(root, &name)?;
    let package = Package::load(&package_dir)?;

    let track = inline_track
        .or_else(|| flag_track.map(str::to_string))
        .or(path_track)
        .unwrap_or_else(|| DEFAULT_TRACK.to_string());

    if !package.tracks.iter().any(|recipe| recipe.track == track) {
        let available = package
            .tracks
            .iter()
            .map(|recipe| recipe.track.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "{}: no '{track}' track (available: {available})",
            package.name
        );
    }

    Ok((package, track))
}

fn resolve_package_dir(root: &Path, raw: &str) -> Result<PathBuf> {
    let path = Path::new(raw);
    let mut candidates = Vec::new();
    if path.is_absolute() {
        candidates.push(path.to_path_buf());
    } else {
        candidates.push(root.join(path));
        candidates.push(root.join("recipes").join(path));
    }

    for candidate in candidates {
        if candidate.exists() {
            return find_metadata_dir(&candidate)
                .with_context(|| format!("cannot find a package for {raw:?}"));
        }
    }

    bail!("no such package: {raw}; try `snap-repo list`")
}

fn find_metadata_dir(start: &Path) -> Result<PathBuf> {
    let mut current = if start.is_file() {
        start.parent().unwrap_or(start).to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if current.join("metadata.yaml").is_file() {
            return Ok(current);
        }
        if !current.pop() {
            bail!("no metadata.yaml above {}", start.display());
        }
    }
}

/// Return the track implied by a path-shaped argument such as
/// `recipes/ardour/edge`, if there is one.
fn path_track(root: &Path, raw: &str) -> Option<String> {
    let last = Path::new(raw).file_name()?.to_str()?;
    if !TRACKS.contains(&last) {
        return None;
    }
    let path = Path::new(raw);
    let exists = if path.is_absolute() {
        path.exists()
    } else {
        root.join(path).exists() || root.join("recipes").join(path).exists()
    };
    exists.then(|| last.to_string())
}

fn print_plan(root: &Path, package: &Package, recipe: &Recipe, snapcraft_args: &[String]) {
    println!("snap:       {}", package.name);
    println!("package:    {}", relative(root, &package.dir).display());
    let flat = package.dir.join("snapcraft.yaml").is_file()
        || package.dir.join("snap/snapcraft.yaml").is_file();
    println!(
        "track:      {}{}",
        recipe.track,
        if flat { " (flat, implicit)" } else { "" }
    );
    println!(
        "recipe:     {}",
        relative(root, &recipe.project_dir).display()
    );
    println!(
        "base:       {}",
        package.metadata.base.as_deref().unwrap_or("-")
    );
    println!("mode:       {}", package.metadata.mode.as_str());
    println!("workspace:  temporary directory (tempfile)");
    println!("output:     {}", root.join("snaps").display());
    print!("snapcraft:  snapcraft pack --use-lxd");
    for arg in snapcraft_args {
        print!(" {arg}");
    }
    println!();
}

fn stage(recipe: &Recipe, dest: &Path) -> Result<()> {
    let status = Command::new("rsync")
        .arg("-a")
        .arg("--exclude=metadata.yaml")
        .arg("--exclude=.git")
        .arg(format!("{}/", recipe.project_dir.display()))
        .arg(format!("{}/", dest.display()))
        .status()
        .context("cannot run rsync; is it installed?")?;
    if !status.success() {
        bail!("rsync exited with {status}");
    }
    Ok(())
}

fn run_snapcraft_pack(
    dir: &Path,
    snapcraft_args: &[String],
    interrupted: &AtomicBool,
) -> Result<ExitStatus> {
    let mut child = Command::new("snapcraft")
        .arg("pack")
        .arg("--use-lxd")
        .args(snapcraft_args)
        .current_dir(dir)
        .spawn()
        .context("cannot run snapcraft; is it installed?")?;

    loop {
        if let Some(status) = child.try_wait().context("cannot wait for snapcraft")? {
            return Ok(status);
        }
        if interrupted.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("interrupted");
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn clean_build_env(dir: &Path) -> Result<()> {
    let status = Command::new("snapcraft")
        .arg("clean")
        .arg("--use-lxd")
        .current_dir(dir)
        .status()
        .context("cannot run snapcraft clean; is it installed?")?;
    if !status.success() {
        bail!("snapcraft clean exited with {status}");
    }
    Ok(())
}

fn newest_snap(dir: &Path) -> Result<Option<PathBuf>> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension() != Some(OsStr::new("snap")) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        if newest
            .as_ref()
            .is_none_or(|(previous, _)| modified >= *previous)
        {
            newest = Some((modified, path));
        }
    }
    Ok(newest.map(|(_, path)| path))
}

fn install_artifact(artifact: &Path, output_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("cannot create {}", output_dir.display()))?;
    let name = artifact
        .file_name()
        .with_context(|| format!("{} has no file name", artifact.display()))?;
    let destination = output_dir.join(name);

    // tempfile lives on /tmp (usually tmpfs), while the repository is on
    // disk, so rename() often crosses filesystems. Fall back to copy+remove.
    if fs::rename(artifact, &destination).is_err() {
        fs::copy(artifact, &destination).with_context(|| {
            format!(
                "cannot copy {} to {}",
                artifact.display(),
                destination.display()
            )
        })?;
        fs::remove_file(artifact)
            .with_context(|| format!("cannot remove {}", artifact.display()))?;
    }
    Ok(destination)
}

fn recipe_present(dir: &Path) -> bool {
    dir.join("snapcraft.yaml").is_file() || dir.join("snap/snapcraft.yaml").is_file()
}

fn check_interrupted(interrupted: &AtomicBool) -> Result<()> {
    if interrupted.load(Ordering::Relaxed) {
        bail!("interrupted");
    }
    Ok(())
}

#[cfg(unix)]
fn signal_flag() -> Result<Arc<AtomicBool>> {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};

    let flag = Arc::new(AtomicBool::new(false));
    for signal in [SIGINT, SIGTERM, SIGHUP] {
        signal_hook::flag::register(signal, Arc::clone(&flag))
            .with_context(|| format!("cannot install a handler for signal {signal}"))?;
    }
    Ok(flag)
}

#[cfg(not(unix))]
fn signal_flag() -> Result<Arc<AtomicBool>> {
    Ok(Arc::new(AtomicBool::new(false)))
}

fn relative<'a>(root: &'a Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(root).unwrap_or(path)
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes}B")
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(1023), "1023B");
        assert_eq!(format_size(1024), "1.0K");
        assert_eq!(format_size(1536), "1.5K");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0M");
    }

    #[test]
    fn recognizes_track_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        fs::create_dir_all(root.join("recipes/foo/edge")).expect("create dirs");

        assert_eq!(path_track(root, "foo/edge").as_deref(), Some("edge"));
        assert_eq!(
            path_track(root, "recipes/foo/edge").as_deref(),
            Some("edge")
        );
        assert_eq!(path_track(root, "foo/stable"), None);
        assert_eq!(path_track(root, "foo"), None);
    }
}
