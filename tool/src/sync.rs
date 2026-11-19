use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use regex::Regex;

use crate::cli::SyncArgs;
use crate::discover;
use crate::package::BumpMode;

#[derive(Debug)]
struct Row {
    package: String,
    track: String,
    upstream: String,
    tag: String,
    version: String,
    mode: &'static str,
    action: &'static str,
}

pub fn run(root: &Path, args: &SyncArgs) -> Result<()> {
    if !args.dry_run {
        bail!("writes are not implemented yet; run with --dry-run");
    }
    if args.jobs == 0 {
        bail!("--jobs must be at least 1");
    }

    let packages = discover::discover(root)?;
    let selected = select(packages, args)?;
    let mut rows = Vec::new();
    let mut manual = 0usize;

    for package in &selected {
        if package.metadata.upstream.provider.is_manual() {
            manual += 1;
        }
        for track in &package.tracks {
            if let Some(tag) = &track.source_tag {
                let pattern = Regex::new(package.metadata.upstream.tag_pattern())
                    .with_context(|| format!("{}: invalid tag-pattern", package.name))?;
                if !pattern.is_match(tag) {
                    bail!(
                        "{}: current tag {:?} does not match tag-pattern {:?}",
                        package.name,
                        tag,
                        package.metadata.upstream.tag_pattern()
                    );
                }
            }

            let action = if package.metadata.upstream.provider.is_manual() {
                "manual"
            } else if track.mode == BumpMode::Tag {
                "check"
            } else {
                "report-only"
            };

            rows.push(Row {
                package: package.name.clone(),
                track: track.track.clone(),
                upstream: package.metadata.upstream.display(),
                tag: track.source_tag.clone().unwrap_or_else(|| "-".to_string()),
                version: track.display_version().to_string(),
                mode: track.mode.as_str(),
                action,
            });
        }
    }

    print_table(&rows);
    println!();

    if args.verbose {
        for package in &selected {
            for track in &package.tracks {
                println!(
                    "{}: dir={} recipe={} app-part={} ({}) tags={} adopt-info={}",
                    package.name,
                    package.dir.display(),
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

    println!(
        "{} packages, {} tracks; {} manual",
        selected.len(),
        rows.len(),
        manual
    );
    let tag = rows.iter().filter(|row| row.mode == "tag").count();
    let script = rows.iter().filter(|row| row.mode == "script").count();
    let url = rows.iter().filter(|row| row.mode == "url").count();
    let source_other = rows.iter().filter(|row| row.mode == "source-other").count();
    let none = rows.iter().filter(|row| row.mode == "none").count();
    println!(
        "modes: {tag} tag, {script} script, {url} url, {source_other} source-other, {none} none"
    );
    println!("upstream fetching is not implemented yet; this is local resolution only.");
    Ok(())
}

fn select(
    packages: Vec<crate::package::Package>,
    args: &SyncArgs,
) -> Result<Vec<crate::package::Package>> {
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

fn print_table(rows: &[Row]) {
    let headers = [
        "PACKAGE", "TRACK", "UPSTREAM", "TAG", "VERSION", "MODE", "ACTION",
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

fn row_values(row: &Row) -> [&str; 7] {
    [
        &row.package,
        &row.track,
        &row.upstream,
        &row.tag,
        &row.version,
        row.mode,
        row.action,
    ]
}

fn print_row(values: &[&str; 7], widths: &[usize]) {
    let line: Vec<String> = values
        .iter()
        .enumerate()
        .map(|(index, value)| format!("{:<width$}", value, width = widths[index]))
        .collect();
    println!("{}", line.join("  ").trim_end());
}
