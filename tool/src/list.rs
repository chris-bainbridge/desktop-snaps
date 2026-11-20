use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::cli::ListArgs;
use crate::discover;

#[derive(Serialize)]
struct Row<'a> {
    name: &'a str,
    track: &'a str,
    version: &'a str,
    base: Option<&'a str>,
    publisher: Option<&'a str>,
}

pub fn run(root: &Path, args: &ListArgs) -> Result<()> {
    let packages = discover::discover(root)?;

    if args.json {
        let rows: Vec<Row> = packages
            .iter()
            .flat_map(|package| {
                let base = package.metadata.base.as_deref();
                let publisher = package
                    .metadata
                    .store
                    .as_ref()
                    .and_then(|store| store.publisher.as_deref());
                package.tracks.iter().map(move |track| Row {
                    name: &package.name,
                    track: &track.track,
                    version: track.display_version(),
                    base,
                    publisher,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&rows)?);
        return Ok(());
    }

    let name_width = packages
        .iter()
        .map(|package| package.name.len())
        .max()
        .unwrap_or(0)
        .max("NAME".len());
    let track_width = packages
        .iter()
        .flat_map(|package| &package.tracks)
        .map(|track| track.track.len())
        .max()
        .unwrap_or(0)
        .max("TRACK".len());
    let base_width = packages
        .iter()
        .map(|package| package.metadata.base.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(0)
        .max("BASE".len());
    let version_width = packages
        .iter()
        .flat_map(|package| &package.tracks)
        .map(|track| track.display_version().len())
        .max()
        .unwrap_or(0)
        .max("VERSION".len());

    println!(
        "{:<name_width$}  {:<track_width$}  {:<version_width$}  {:<base_width$}  PUBLISHER",
        "NAME", "TRACK", "VERSION", "BASE"
    );
    for package in &packages {
        let base = package.metadata.base.as_deref().unwrap_or("-");
        let publisher = package
            .metadata
            .store
            .as_ref()
            .and_then(|store| store.publisher.as_deref())
            .unwrap_or("");
        for track in &package.tracks {
            let line = format!(
                "{:<name_width$}  {:<track_width$}  {:<version_width$}  {:<base_width$}  {}",
                package.name,
                track.track,
                track.display_version(),
                base,
                publisher
            );
            println!("{}", line.trim_end());
        }
    }
    Ok(())
}
