use clap::{ArgGroup, Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "snap-repo",
    version,
    about = "Check and bump the snap recipes in this repository"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List all packages and tracks
    List,
    /// Check upstream releases and (later) bump recipes
    Sync(SyncArgs),
    /// Show API quota and reset times
    RateLimit(RateLimitArgs),
    /// Build a snap from a recipe in this repository
    Build(BuildArgs),
}

#[derive(Debug, Args)]
#[command(
    arg_required_else_help = true,
    override_usage = "snap-repo sync [OPTIONS] <PACKAGE>...\n       snap-repo sync --all [--dry-run]",
    after_help = "Environment:\n  GITHUB_TOKEN or GH_TOKEN  GitHub API token; falls back to gh auth token\n  GITLAB_TOKEN_<HOST>       per-host GitLab token; falls back to glab config\n  GITLAB_TOKEN              GitLab.com token (also accepted for gitlab.com)",
    group(
        ArgGroup::new("selection")
            .required(true)
            .multiple(false)
            .args(["packages", "all"])
    )
)]
pub struct SyncArgs {
    /// Packages to check (omit with --all)
    #[arg(value_name = "PACKAGE")]
    pub packages: Vec<String>,

    /// Check every package
    #[arg(long)]
    pub all: bool,

    /// Report only; do not write recipes
    #[arg(long)]
    pub dry_run: bool,

    /// Show the resolved recipe and app part for each track
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Debug, Args)]
pub struct RateLimitArgs {
    /// Ignore cached quota and query every host
    #[arg(long)]
    pub refresh: bool,
}

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct BuildArgs {
    /// Packages to build (omit with --all)
    #[arg(value_name = "PACKAGE")]
    pub packages: Vec<String>,

    /// Build every package (not implemented yet)
    #[arg(long)]
    pub all: bool,

    /// Show the resolved build plan
    #[arg(long)]
    pub dry_run: bool,

    /// Keep the build workspace and the LXD build container
    #[arg(long)]
    pub keep: bool,

    /// Track to build
    #[arg(long, value_name = "TRACK")]
    pub track: Option<String>,

    /// Number of packages to build in parallel
    #[arg(short = 'j', long, default_value_t = 1, value_name = "N")]
    pub jobs: usize,

    /// Arguments after -- are passed to snapcraft
    #[arg(last = true, value_name = "SNAPCRAFT ARGS")]
    pub snapcraft_args: Vec<String>,
}
