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

use clap::Parser;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tool/ must have a parent directory")
        .to_path_buf()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let root = repo_root();
    match cli.command {
        cli::Command::List => list::run(&root),
        cli::Command::Sync(args) => sync::run(&root, &args).await,
        cli::Command::RateLimit(args) => {
            let config = upstream::HttpConfig::from_env();
            rate_limit::run(&config, args.refresh).await
        }
        cli::Command::Build(args) => build_cmd::run(&root, &args),
    }
}
