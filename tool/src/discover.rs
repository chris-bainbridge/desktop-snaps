use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::package::Package;

pub fn discover(root: &Path) -> Result<Vec<Package>> {
    let recipes = root.join("recipes");
    if !recipes.is_dir() {
        bail!("{} is not a directory", recipes.display());
    }

    let mut dirs: Vec<PathBuf> = fs::read_dir(&recipes)
        .with_context(|| format!("cannot read {}", recipes.display()))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .filter(|dir| dir.join("metadata.yaml").is_file())
        .collect();
    dirs.sort();

    dirs.into_iter().map(|dir| Package::load(&dir)).collect()
}
