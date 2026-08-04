//! The pluggable job-execution interface.
//!
//! The Scheduler knows nothing about *how* to build a module - that is
//! `coreforge-toolchain`'s job (Phase 5). It only knows how to run whatever
//! implements [`JobRunner`], in dependency-respecting, parallel batches.

use std::time::Duration;

use coreforge_core::{Module, ModuleId};

/// Something that knows how to build a single module.
///
/// Implementations must be safe to call concurrently from multiple threads:
/// [`JobRunner::run`] may be invoked for several modules in the same build
/// level at the same time, from different worker threads.
///
/// This trait is intentionally minimal today. Phase 5's `coreforge-toolchain`
/// crate is expected to provide the real implementation, dispatching to the
/// appropriate Tool Adapter (Cargo, CMake, npm, ...) based on
/// `module.module_type`.
pub trait JobRunner: Send + Sync {
    /// Builds `module`, blocking the calling thread until it's done.
    fn run(&self, module: &Module) -> JobStatus;
}

/// The result of attempting to build a single module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// The module built successfully.
    Success,
    /// The module failed to build. The `String` is a short, human-readable reason.
    Failed(String),
    /// The module was not attempted, because one of its (transitive)
    /// dependencies failed or was itself skipped. The `String` names the
    /// dependency responsible.
    Skipped(String),
}

impl JobStatus {
    /// Whether this status represents a successful build.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Whether this status represents a failed build.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Whether this status represents a skipped module.
    #[must_use]
    pub fn is_skipped(&self) -> bool {
        matches!(self, Self::Skipped(_))
    }
}

/// The outcome of scheduling a single module, including how long its
/// [`JobRunner::run`] call took (zero for a [`JobStatus::Skipped`] module,
/// since it was never run).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutcome {
    /// The module this outcome is for.
    pub module: ModuleId,
    /// What happened.
    pub status: JobStatus,
    /// How long the job took to run.
    pub duration: Duration,
}

/// A [`JobRunner`] that never actually builds anything - it immediately
/// reports every module as successful.
///
/// This exists because Phase 5 (the real Tool Adapters) doesn't exist yet:
/// it lets the Scheduler, and anything built on top of it (e.g. the CLI's
/// `--dry-run` flag), be exercised end-to-end today. `coreforge-toolchain`
/// is expected to provide the real [`JobRunner`] implementation later.
#[derive(Debug, Clone, Copy, Default)]
pub struct DryRunRunner;

impl JobRunner for DryRunRunner {
    fn run(&self, _module: &Module) -> JobStatus {
        JobStatus::Success
    }
}
