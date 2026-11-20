mod affected;
mod build_cmd;
mod cli;
mod discover;
mod list;
mod metadata;
mod package;
mod quota;
mod rate_limit;
mod recipe_edit;
mod sync;
mod upstream;
mod version;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;

/// Environment variable the `snap-repo` wrapper sets to the checkout root.
const ROOT_ENV: &str = "SNAP_REPO_ROOT";

/// A repository root holds the tool crate and the recipes it operates on.
fn is_repo_root(dir: &Path) -> bool {
    dir.join("tool/Cargo.toml").is_file() && dir.join("recipes").is_dir()
}

/// Locate the checkout at run time, never at compile time: the binary must
/// keep working when the repository is renamed, moved or checked out elsewhere.
fn repo_root() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os(ROOT_ENV) {
        let root = PathBuf::from(value);
        if !is_repo_root(&root) {
            bail!(
                "{ROOT_ENV}={} is not a snap-repo repository root \
                 (expected tool/Cargo.toml and recipes/)",
                root.display()
            );
        }
        return Ok(root);
    }

    let exe = std::env::current_exe().context("cannot locate the snap-repo executable")?;
    let exe = exe.canonicalize().unwrap_or(exe);
    for dir in exe.ancestors().skip(1) {
        if is_repo_root(dir) {
            return Ok(dir.to_path_buf());
        }
    }

    bail!(
        "cannot locate the snap-repo repository root from {}: \
         run the ./snap-repo wrapper or set {ROOT_ENV}",
        exe.display()
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let root = repo_root()?;
    match cli.command {
        cli::Command::List(args) => list::run(&root, &args),
        cli::Command::Sync(args) => sync::run(&root, &args).await,
        cli::Command::RateLimit(args) => {
            let config = upstream::HttpConfig::from_env();
            rate_limit::run(&config, args.refresh).await
        }
        cli::Command::Build(args) => build_cmd::run(&root, &args),
        cli::Command::Affected(args) => affected::run(&root, &args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_a_repository_root() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path();
        assert!(!is_repo_root(root), "an empty directory is not a root");

        std::fs::create_dir_all(root.join("tool")).expect("create tool/");
        std::fs::write(root.join("tool/Cargo.toml"), "").expect("write manifest");
        assert!(!is_repo_root(root), "recipes/ is required");

        std::fs::create_dir(root.join("recipes")).expect("create recipes/");
        assert!(is_repo_root(root));
    }

    #[test]
    fn repo_root_resolves_to_the_checkout() {
        let root = repo_root().expect("locate the repository root");
        assert!(is_repo_root(&root), "{} is not a root", root.display());
    }
}
