use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Result, bail};
use futures::StreamExt;
use regex::Regex;

use crate::cli::{MAX_JOBS, SyncArgs};
use crate::discover;
use crate::package::{BumpMode, Package};
use crate::upstream::{self, HttpConfig};
use crate::version;

#[derive(Debug)]
struct Row {
    package: String,
    track: String,
    upstream: String,
    tag: String,
    latest: String,
    version: String,
    mode: &'static str,
    action: &'static str,
}

#[derive(Debug)]
struct PackageResult {
    package: Package,
    rows: Vec<Row>,
    error: Option<String>,
}

pub async fn run(root: &Path, args: &SyncArgs) -> Result<()> {
    if !args.dry_run {
        bail!("writes are not implemented yet; run with --dry-run");
    }
    if args.jobs == 0 {
        bail!("--jobs must be at least 1");
    }
    if args.jobs > MAX_JOBS {
        eprintln!("snap-repo: --jobs capped at {MAX_JOBS}");
    }

    let packages = discover::discover(root)?;
    let selected = select(packages, args)?;
    let config = HttpConfig::from_env();
    let client = config.client()?;
    let jobs = args.jobs.min(MAX_JOBS);
    let total = selected.len();
    let name_width = selected
        .iter()
        .map(|package| package.name.len())
        .max()
        .unwrap_or(0);

    let futures = selected.into_iter().map(|package| {
        let client = client.clone();
        let config = config.clone();
        async move { resolve_package(&client, &config, package).await }
    });
    let mut stream = futures::stream::iter(futures).buffer_unordered(jobs);
    let mut results = Vec::with_capacity(total);
    let mut completed = 0;
    while let Some(result) = stream.next().await {
        completed += 1;
        print_progress(completed, total, name_width, &result);
        results.push(result);
    }
    results.sort_by(|left, right| left.package.name.cmp(&right.package.name));

    let rows: Vec<&Row> = results
        .iter()
        .flat_map(|result| result.rows.iter())
        .collect();
    print_table(&rows);
    println!();

    if args.verbose {
        for result in &results {
            for track in &result.package.tracks {
                println!(
                    "{}: dir={} recipe={} app-part={} ({}) tags={} adopt-info={}",
                    result.package.name,
                    result.package.dir.display(),
                    track.path.display(),
                    track.app_part,
                    track.app_part_source.as_str(),
                    track.source_tag_count,
                    track.adopt_info.as_deref().unwrap_or("-"),
                );
            }
        }
        println!();
    }

    for result in &results {
        if let Some(error) = &result.error {
            eprintln!("{}: {error}", result.package.name);
        }
    }

    let manual = results
        .iter()
        .filter(|result| result.package.metadata.upstream.provider.is_manual())
        .count();
    println!(
        "{} packages, {} tracks; {} manual",
        results.len(),
        rows.len(),
        manual
    );
    println!(
        "actions: {} bump, {} up-to-date, {} ahead, {} report-only, {} manual, {} error",
        count(&rows, "bump"),
        count(&rows, "up-to-date"),
        count(&rows, "ahead"),
        count(&rows, "report-only"),
        count(&rows, "manual"),
        count(&rows, "error"),
    );

    let errors = results
        .iter()
        .filter(|result| result.error.is_some())
        .count();
    if errors > 0 {
        bail!("{errors} package(s) failed");
    }
    Ok(())
}

async fn resolve_package(
    client: &reqwest::Client,
    config: &HttpConfig,
    package: Package,
) -> PackageResult {
    let needs_fetch = !package.metadata.upstream.provider.is_manual()
        && package
            .tracks
            .iter()
            .any(|track| track.mode == BumpMode::Tag);
    let latest = if needs_fetch {
        Some(upstream::latest_tag(client, config, &package.metadata.upstream).await)
    } else {
        None
    };

    let mut result = PackageResult {
        package,
        rows: Vec::new(),
        error: None,
    };
    if let Some(Err(error)) = &latest {
        result.error = Some(format!("{error:#}"));
    }

    for track in &result.package.tracks {
        let latest_tag = match &latest {
            Some(Ok(latest)) => latest.tag.clone(),
            _ => "-".to_string(),
        };
        let action = if result.package.metadata.upstream.provider.is_manual() {
            "manual"
        } else if track.mode != BumpMode::Tag {
            "report-only"
        } else {
            match &latest {
                Some(Ok(latest)) => {
                    let current_tag = track.source_tag.as_deref().unwrap_or("");
                    let pattern = Regex::new(result.package.metadata.upstream.tag_pattern())
                        .expect("tag-pattern validated during discovery");
                    let current_version = pattern
                        .captures(current_tag)
                        .and_then(|captures| captures.get(1))
                        .map(|capture| capture.as_str())
                        .unwrap_or(current_tag);
                    match version::compare(&latest.version, current_version) {
                        Ordering::Greater => "bump",
                        Ordering::Equal => "up-to-date",
                        Ordering::Less => "ahead",
                    }
                }
                Some(Err(_)) | None => "error",
            }
        };

        result.rows.push(Row {
            package: result.package.name.clone(),
            track: track.track.clone(),
            upstream: result.package.metadata.upstream.display(),
            tag: track.source_tag.clone().unwrap_or_else(|| "-".to_string()),
            latest: latest_tag,
            version: track.display_version().to_string(),
            mode: track.mode.as_str(),
            action,
        });
    }
    result
}

fn count(rows: &[&Row], action: &str) -> usize {
    rows.iter().filter(|row| row.action == action).count()
}

fn print_progress(completed: usize, total: usize, name_width: usize, result: &PackageResult) {
    eprintln!(
        "[{completed}/{total}] {:<name_width$} {}",
        result.package.name,
        progress_status(result)
    );
}

fn progress_status(result: &PackageResult) -> String {
    if result.error.is_some() {
        return "[error]".to_string();
    }
    if result.package.metadata.upstream.provider.is_manual() {
        return "[manual]".to_string();
    }
    let Some(row) = result.rows.first() else {
        return "[-]".to_string();
    };
    match row.action {
        "bump" => format!("[{} -> {}] *UPDATED*", row.version, row.latest),
        "ahead" => format!("[{} -> {}] *AHEAD*", row.version, row.latest),
        "up-to-date" => format!("[{}]", row.version),
        "report-only" if row.version == "-" => "[report-only]".to_string(),
        "report-only" => format!("[{}] report-only", row.version),
        "error" => "[error]".to_string(),
        _ => format!("[{}]", row.version),
    }
}

fn select(packages: Vec<Package>, args: &SyncArgs) -> Result<Vec<Package>> {
    if args.all && !args.packages.is_empty() {
        bail!("give either package names or --all, not both");
    }
    if !args.all && args.packages.is_empty() {
        bail!("give one or more packages, or --all");
    }
    if args.all {
        return Ok(packages);
    }

    let wanted: HashSet<&str> = args.packages.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    let selected: Vec<_> = packages
        .into_iter()
        .filter(|package| {
            if wanted.contains(package.name.as_str()) {
                seen.insert(package.name.clone());
                true
            } else {
                false
            }
        })
        .collect();

    let unknown: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|name| !seen.contains(*name))
        .collect();
    if !unknown.is_empty() {
        bail!("unknown package(s): {}", unknown.join(", "));
    }
    Ok(selected)
}

fn print_table(rows: &[&Row]) {
    let headers = [
        "PACKAGE", "TRACK", "UPSTREAM", "TAG", "LATEST", "VERSION", "MODE", "ACTION",
    ];
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        let values = row_values(row);
        for (index, value) in values.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    print_row(&headers, &widths);
    for row in rows {
        print_row(&row_values(row), &widths);
    }
}

fn row_values(row: &Row) -> [&str; 8] {
    [
        &row.package,
        &row.track,
        &row.upstream,
        &row.tag,
        &row.latest,
        &row.version,
        row.mode,
        row.action,
    ]
}

fn print_row(values: &[&str; 8], widths: &[usize]) {
    let line: Vec<String> = values
        .iter()
        .enumerate()
        .map(|(index, value)| format!("{:<width$}", value, width = widths[index]))
        .collect();
    println!("{}", line.join("  ").trim_end());
}
