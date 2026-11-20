use std::collections::{HashSet, VecDeque};
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use crate::cli::BuildArgs;
use crate::metadata::BuildMode;
use crate::package::{Package, Recipe};

const DEFAULT_TRACK: &str = "stable";
pub(crate) const TRACKS: [&str; 4] = ["stable", "candidate", "beta", "edge"];
const FAILURE_TAIL_LINES: usize = 20;

/// Build one or more snaps from recipes in this repository.
pub fn run(root: &Path, args: &BuildArgs) -> Result<()> {
    if args.jobs == 0 {
        bail!("--jobs must be at least 1");
    }

    let plans = resolve_plans(root, args)?;
    if args.dry_run {
        for (index, plan) in plans.iter().enumerate() {
            if index > 0 {
                println!();
            }
            print_plan(root, &plan.package, plan.recipe(), &args.snapcraft_args);
        }
        return Ok(());
    }

    let interrupted = signal_flag()?;
    build_all(root, &plans, args, &interrupted)
}

struct BuildPlan {
    package: Package,
    track_index: usize,
}

impl BuildPlan {
    fn recipe(&self) -> &Recipe {
        &self.package.tracks[self.track_index]
    }
}

fn resolve_plans(root: &Path, args: &BuildArgs) -> Result<Vec<BuildPlan>> {
    if args.all {
        bail!("build --all is not implemented yet");
    }
    if args.packages.is_empty() {
        bail!("give one or more packages, or --all");
    }

    let mut plans = Vec::new();
    let mut seen = HashSet::new();
    for raw in &args.packages {
        let (package, track) = resolve(root, raw, args.track.as_deref())
            .with_context(|| format!("cannot resolve package {raw:?}"))?;
        let track_index = package
            .tracks
            .iter()
            .position(|recipe| recipe.track == track)
            .expect("resolve() returned an existing track");
        if seen.insert((package.name.clone(), track)) {
            plans.push(BuildPlan {
                package,
                track_index,
            });
        }
    }

    for plan in &plans {
        validate_plan(plan)?;
    }
    Ok(plans)
}

fn validate_plan(plan: &BuildPlan) -> Result<()> {
    let package = &plan.package;
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
    Ok(())
}

fn build_all(
    root: &Path,
    plans: &[BuildPlan],
    args: &BuildArgs,
    interrupted: &Arc<AtomicBool>,
) -> Result<()> {
    if plans.is_empty() {
        return Ok(());
    }

    let jobs = args.jobs.min(plans.len()).max(1);
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::<(String, anyhow::Error)>::new());

    thread::scope(|scope| {
        let next = &next;
        let failures = &failures;
        for _ in 0..jobs {
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(plan) = plans.get(index) else {
                        break;
                    };
                    if let Err(error) = build_one(root, plan, args, interrupted.as_ref()) {
                        let label = format!("{} ({})", plan.package.name, plan.recipe().track);
                        failures.lock().expect("failure list").push((label, error));
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().expect("failure list");
    if failures.is_empty() {
        return Ok(());
    }
    for (label, error) in &failures {
        eprintln!("build: {label}: failed");
        if let Some(failure) = error.downcast_ref::<SnapcraftFailure>() {
            eprintln!("  {failure}");
            if !args.verbose && !failure.tail.is_empty() {
                eprintln!("  last {} lines of snapcraft output:", failure.tail.len());
                for line in &failure.tail {
                    eprintln!("  {line}");
                }
            }
            if let Some(log_path) = &failure.log_path
                && (args.verbose || !failure.tail.contains(log_path))
            {
                eprintln!("  {log_path}");
            }
        } else {
            eprintln!("  {error:#}");
        }
        eprintln!();
    }
    bail!("{} package(s) failed", failures.len())
}

fn build_one(
    root: &Path,
    plan: &BuildPlan,
    args: &BuildArgs,
    interrupted: &AtomicBool,
) -> Result<()> {
    let package = &plan.package;
    let recipe = plan.recipe();
    let label = format!("{} ({})", package.name, recipe.track);

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

    let build_result = build(root, recipe, &label, &temp_path, args, interrupted);

    let cleanup_error = if !args.keep && recipe_present(&temp_path) {
        eprintln!("build: {label}: cleaning the LXD build container");
        clean_build_env(&temp_path).err()
    } else {
        None
    };

    if let Some(error) = cleanup_error {
        let path = temp.take().expect("temporary directory").keep();
        eprintln!(
            "build: {label}: error: snapcraft clean failed; workspace kept: {}",
            path.display()
        );
        return Err(error);
    }

    if args.keep {
        let path = temp.take().expect("temporary directory").keep();
        eprintln!(
            "build: {label}: workspace and LXD build container kept: {}",
            path.display()
        );
    }

    match build_result {
        Ok(()) => Ok(()),
        Err(error) => {
            if !args.keep {
                eprintln!(
                    "build: {label}: build failed; temporary workspace removed (use --keep to retain it)"
                );
            }
            Err(error)
        }
    }
}

fn build(
    root: &Path,
    recipe: &Recipe,
    label: &str,
    temp_path: &Path,
    args: &BuildArgs,
    interrupted: &AtomicBool,
) -> Result<()> {
    eprintln!(
        "build: {label}: staging {} in {}",
        relative(root, &recipe.project_dir).display(),
        temp_path.display()
    );
    stage(recipe, temp_path)?;
    check_interrupted(interrupted)?;

    eprintln!("build: {label}: building with snapcraft pack --use-lxd");
    let run = run_snapcraft_pack(
        temp_path,
        &args.snapcraft_args,
        label,
        args.verbose,
        interrupted,
    )?;
    if !run.status.success() {
        return Err(SnapcraftFailure {
            status: run.status,
            tail: run.tail,
            log_path: run.log_path,
        }
        .into());
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
    eprintln!("build: {label}: built {} ({})", final_path.display(), size);
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

struct SnapcraftRun {
    status: ExitStatus,
    tail: Vec<String>,
    log_path: Option<String>,
}

#[derive(Debug)]
struct SnapcraftFailure {
    status: ExitStatus,
    tail: Vec<String>,
    log_path: Option<String>,
}

impl fmt::Display for SnapcraftFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "snapcraft pack exited with {}", self.status)
    }
}

impl std::error::Error for SnapcraftFailure {}

#[derive(Default)]
struct Capture {
    tail: VecDeque<String>,
    log_path: Option<String>,
}

fn run_snapcraft_pack(
    dir: &Path,
    snapcraft_args: &[String],
    label: &str,
    verbose: bool,
    interrupted: &AtomicBool,
) -> Result<SnapcraftRun> {
    let mut child = Command::new("snapcraft")
        .arg("pack")
        .arg("--use-lxd")
        .args(snapcraft_args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("cannot run snapcraft; is it installed?")?;

    let stdout = child
        .stdout
        .take()
        .context("cannot capture snapcraft stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("cannot capture snapcraft stderr")?;
    let capture = Arc::new(Mutex::new(Capture::default()));
    let stdout_thread = capture_stream(stdout, label, verbose, Arc::clone(&capture));
    let stderr_thread = capture_stream(stderr, label, verbose, Arc::clone(&capture));

    let status = loop {
        if let Some(status) = child.try_wait().context("cannot wait for snapcraft")? {
            break status;
        }
        if interrupted.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            bail!("interrupted");
        }
        thread::sleep(Duration::from_millis(100));
    };

    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    let captured = capture.lock().expect("snapcraft capture");
    Ok(SnapcraftRun {
        status,
        tail: captured.tail.iter().cloned().collect(),
        log_path: captured.log_path.clone(),
    })
}

fn capture_stream<R: Read + Send + 'static>(
    reader: R,
    label: &str,
    verbose: bool,
    capture: Arc<Mutex<Capture>>,
) -> thread::JoinHandle<()> {
    let label = label.to_string();
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            let text = String::from_utf8_lossy(&buffer);
            for line in text.split(['\n', '\r']) {
                let line = line.trim_end();
                if line.is_empty() {
                    continue;
                }
                if verbose {
                    eprintln!("[{label}] {line}");
                }
                let mut capture = capture.lock().expect("snapcraft capture");
                if line.contains("Full execution log:") {
                    capture.log_path = Some(line.to_string());
                }
                capture.tail.push_back(line.to_string());
                while capture.tail.len() > FAILURE_TAIL_LINES {
                    capture.tail.pop_front();
                }
            }
        }
    })
}

fn clean_build_env(dir: &Path) -> Result<()> {
    let output = Command::new("snapcraft")
        .arg("clean")
        .arg("--use-lxd")
        .current_dir(dir)
        .output()
        .context("cannot run snapcraft clean; is it installed?")?;
    if output.status.success() {
        return Ok(());
    }

    let mut details = String::new();
    for bytes in [&output.stdout, &output.stderr] {
        let text = String::from_utf8_lossy(bytes);
        let text = text.trim();
        if !text.is_empty() {
            if !details.is_empty() {
                details.push('\n');
            }
            details.push_str(text);
        }
    }
    if details.is_empty() {
        bail!("snapcraft clean exited with {}", output.status);
    }
    bail!("snapcraft clean exited with {}:\n{details}", output.status);
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

    #[test]
    fn captures_only_the_failure_tail() {
        use std::io::Cursor;

        let mut input = String::new();
        for line in 0..30 {
            input.push_str(&format!("line {line}\n"));
        }
        input.push_str("Full execution log: '/tmp/snapcraft.log'\n");

        let capture = Arc::new(Mutex::new(Capture::default()));
        let reader = capture_stream(
            Cursor::new(input.into_bytes()),
            "test (stable)",
            false,
            Arc::clone(&capture),
        );
        reader.join().expect("reader");

        let capture = capture.lock().expect("capture");
        assert_eq!(capture.tail.len(), FAILURE_TAIL_LINES);
        assert_eq!(
            capture.tail.back().map(String::as_str),
            Some("Full execution log: '/tmp/snapcraft.log'")
        );
        assert_eq!(
            capture.log_path.as_deref(),
            Some("Full execution log: '/tmp/snapcraft.log'")
        );
    }
}
