//! Command Parser
//!
//! This module only reads the command line and translates it into a configured `Cli` value.
//! Nothing is compiled, scanned, or executed yet -
//! Project Inspector (Phase 1) and later will take over this.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "coreforge",
    version,
    about = "CoreForge - CoreVerse Engine icin build orchestrator",
    long_about = None
)]
pub struct Cli {
    /// Detailed (verbose) log output.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Path to the build-system.toml file to be used.
    /// If not specified, the repository will be automatically
    /// searched for (to be implemented in Phase 8).
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// It compiles the modules.
    Build(BuildArgs),

    /// It runs tests for the modules.
    Test(BuildArgs),

    /// The compiled outputs are packaged.
    Package(BuildArgs),

    /// Cleans up build output (build/).
    Clean {
        /// Clear only the specified module. If no module is specified, all will be cleared.
        #[arg(value_name = "MODULE")]
        module: Option<String>,
    },
}

#[derive(Debug, clap::Args)]
pub struct BuildArgs {
    /// Target only the specified module(s). If not specified, the entire module graph will be processed.
    #[arg(value_name = "MODULE")]
    pub modules: Vec<String>,

    /// Compile with Release configuration (default: Debug).
    #[arg(long)]
    pub release: bool,

    /// Don't actually run any commands, just show the build plan.
    #[arg(long)]
    pub dry_run: bool,

    /// Number of parallel jobs. If not specified, the build-system.toml / CPU count will be used.
    #[arg(short = 'j', long, value_name = "N")]
    pub jobs: Option<usize>,
}
