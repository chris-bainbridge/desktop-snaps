mod build_cmd;
mod cli;
mod discover;
mod list;
mod metadata;
mod package;
mod sync;

use std::path::{Path, PathBuf};

use clap::Parser;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tool/ must have a parent directory")
        .to_path_buf()
}

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let root = repo_root();
    match cli.command {
        cli::Command::List => list::run(&root),
        cli::Command::Sync(args) => sync::run(&root, &args),
        cli::Command::Build(args) => build_cmd::run(&root, &args),
    }
}
