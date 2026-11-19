use std::path::Path;

use anyhow::Result;

use crate::discover;

pub fn run(root: &Path) -> Result<()> {
    let packages = discover::discover(root)?;

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

    println!(
        "{:<name_width$}  {:<track_width$}  {:<base_width$}  PUBLISHER",
        "NAME", "TRACK", "BASE"
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
                "{:<name_width$}  {:<track_width$}  {:<base_width$}  {}",
                package.name, track.track, base, publisher
            );
            println!("{}", line.trim_end());
        }
    }
    Ok(())
}
