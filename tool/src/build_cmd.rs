use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::cli::BuildArgs;

/// Temporary bridge to the bash build driver. The native implementation
/// will replace this once package discovery and track selection are shared.
pub fn run(root: &Path, args: &BuildArgs) -> Result<()> {
    if args.all {
        bail!("build --all is not implemented yet");
    }
    if args.packages.len() != 1 {
        bail!("build takes exactly one package until the native implementation lands");
    }
    if args.jobs != 1 {
        bail!("build --jobs is not implemented yet");
    }

    let mut command = Command::new(root.join("build"));
    command.arg(&args.packages[0]);
    if args.dry_run {
        command.arg("--dry-run");
    }
    if args.keep {
        command.arg("--keep");
    }
    if let Some(track) = &args.track {
        command.arg("--track").arg(track);
    }
    if !args.snapcraft_args.is_empty() {
        command.arg("--").args(&args.snapcraft_args);
    }

    let status = command
        .status()
        .with_context(|| format!("cannot run {}", root.join("build").display()))?;
    if !status.success() {
        bail!("build exited with {status}");
    }
    Ok(())
}
