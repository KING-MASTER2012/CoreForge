//! Command Parser (Phase 0).
//!
//! This module only reads the command line and turns it into a structured
//! [`Cli`] value. Nothing is built, scanned, or executed here - the Project
//! Inspector (Phase 1) and later phases own that work.

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "coreforge",
    version,
    about = "CoreForge - build orchestrator for CoreVerse Engine",
    long_about = None
)]
pub struct Cli {
    /// Enable verbose logging.
    #[arg(short, long, global = true, conflicts_with = "quiet")]
    pub verbose: bool,

    /// Only report warnings and errors.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Path to the target repository root. Defaults to the current directory.
    #[arg(long, global = true, value_name = "PATH", default_value = ".")]
    pub root: Utf8PathBuf,

    /// Path to a `build-system.toml` file. If unset, it is auto-discovered
    /// as `--root/build-system.toml` (Phase 8). CLI flags always override
    /// whatever this file sets.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<Utf8PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build modules.
    Build(BuildArgs),

    /// Run modules' tests.
    Test(BuildArgs),

    /// Package built artifacts.
    Package(BuildArgs),

    /// Remove build outputs (`build/`).
    Clean {
        /// Only clean the given module. If unset, everything is cleaned.
        #[arg(value_name = "MODULE")]
        module: Option<String>,
    },

    /// Walk the target repository and list discovered modules, without
    /// building anything. Useful for verifying the Project Inspector's
    /// output (Phase 1).
    Inspect,

    /// Resolve the target repository into a Build Graph and print its
    /// linear build order and parallel levels, without building anything.
    /// Useful for verifying the Dependency Resolver's output (Phase 3).
    Graph,

    /// Manage a multi-repository CoreForge workspace.
    Workspace(WorkspaceArgs),
}

#[derive(Debug, clap::Args)]
pub struct WorkspaceArgs {
    #[command(subcommand)]
    pub command: WorkspaceCommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Fetch Git repositories, pin their commits, and update the workspace lock file.
    Sync,
}

#[derive(Debug, clap::Args)]
pub struct BuildArgs {
    /// Only target the given module(s). If unset, the whole module graph is processed.
    #[arg(value_name = "MODULE")]
    pub modules: Vec<String>,

    /// Build with the Release configuration (default: Debug).
    #[arg(long)]
    pub release: bool,

    /// Do not actually run any command; only print the build plan.
    #[arg(long)]
    pub dry_run: bool,

    /// Number of parallel jobs. If unset, falls back to `build-system.toml`
    /// or the number of available CPUs.
    #[arg(short = 'j', long, value_name = "N")]
    pub jobs: Option<usize>,

    /// Stop starting new build levels as soon as any module fails, instead
    /// of continuing to build everything not blocked by the failure.
    #[arg(long)]
    pub fail_fast: bool,
}
