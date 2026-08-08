//! Error type for the executor's orchestration functions.

use coreforge_core::ModuleId;

/// Errors produced by `executor`.
///
/// Wraps every underlying stage's own error type so callers (the CLI, the
/// GUI) get one error surface to match on instead of having to know about
/// `resolver`, `coreforge-workspace`, `scheduler`, `toolchain`,
/// `collector`, `config`, etc. individually. Lower-level errors (the
/// Project Inspector, the Manifest parser) aren't listed separately here -
/// `resolver` and `coreforge-workspace` already wrap them into
/// [`ResolverError`](resolver::ResolverError) /
/// [`WorkspaceError`](coreforge_workspace::WorkspaceError) themselves.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    /// An error from the Dependency Resolver (single-repository mode).
    #[error(transparent)]
    Resolver(#[from] resolver::ResolverError),

    /// An error from the Workspace manager (multi-repository mode).
    #[error(transparent)]
    Workspace(#[from] coreforge_workspace::WorkspaceError),

    /// An error from the Build Graph.
    #[error(transparent)]
    Graph(#[from] graph::GraphError),

    /// An error from the Scheduler.
    #[error(transparent)]
    Scheduler(#[from] scheduler::SchedulerError),

    /// An error from a Toolchain adapter.
    #[error(transparent)]
    Toolchain(#[from] toolchain::ToolchainError),

    /// An error from the Artifact Collector.
    #[error(transparent)]
    Collector(#[from] collector::CollectorError),

    /// A module id given explicitly (`coreforge build engine`, `coreforge
    /// clean engine`, ...) does not exist in the resolved graph.
    #[error("module not found: {0}")]
    ModuleNotFound(String),

    /// A workspace module was resolved into the dependency graph but has
    /// no corresponding physical location on disk. Should not happen
    /// unless `coreforge-workspace` and the graph disagree with each other.
    #[error("workspace module '{0}' has no physical location")]
    MissingModuleLocation(ModuleId),
}

/// A convenience alias for `Result<T, ExecutorError>`.
pub type Result<T> = std::result::Result<T, ExecutorError>;
