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
    List(ListArgs),
    /// Check upstream releases and (later) bump recipes
    Sync(SyncArgs),
    /// Show API quota and reset times
    RateLimit(RateLimitArgs),
    /// Build a snap from a recipe in this repository
    Build(BuildArgs),
    /// Print the packages changed by a commit range, for CI matrices
    Affected(AffectedArgs),
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Print a JSON array instead of a table
    #[arg(long)]
    pub json: bool,
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
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Show the resolved recipe and app part for each track
    #[arg(long)]
    pub verbose: bool,

    /// Report package errors but exit successfully, after writing any bumps
    #[arg(long)]
    pub allow_errors: bool,
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
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Stream snapcraft output while building
    #[arg(short = 'v', long)]
    pub verbose: bool,

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

#[derive(Debug, Args)]
#[command(
    after_help = "Examples:\n  snap-repo affected --base origin/main --head HEAD\n  snap-repo affected --worktree --github-output \"$GITHUB_OUTPUT\"\n  snap-repo affected --select gnome-logs,five-or-more --json"
)]
pub struct AffectedArgs {
    /// Base revision (default: HEAD^)
    #[arg(long, value_name = "REV")]
    pub base: Option<String>,

    /// Head revision (default: HEAD)
    #[arg(long, value_name = "REV")]
    pub head: Option<String>,

    /// Detect uncommitted changes instead of comparing two revisions
    #[arg(long, conflicts_with_all = ["base", "head"])]
    pub worktree: bool,

    /// Use exactly these packages instead of inspecting git
    #[arg(
        long,
        value_name = "PACKAGE[,PACKAGE...]",
        value_delimiter = ',',
        conflicts_with_all = ["base", "head", "worktree"]
    )]
    pub select: Vec<String>,

    /// Packages to use when the commit range changes no recipe
    #[arg(long, value_name = "PACKAGE[,PACKAGE...]", value_delimiter = ',')]
    pub fallback: Vec<String>,

    /// Print a compact JSON array instead of one name per line
    #[arg(long)]
    pub json: bool,

    /// Append matrix= and has_work= to this file (for $GITHUB_OUTPUT)
    #[arg(long, value_name = "FILE")]
    pub github_output: Option<std::path::PathBuf>,

    /// Refuse to emit more than this many entries (GitHub's matrix limit)
    #[arg(long, default_value_t = 256, value_name = "N")]
    pub max: usize,
}
