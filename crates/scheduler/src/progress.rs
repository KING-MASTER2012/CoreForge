//! Optional live progress reporting, decoupled from the Scheduler's actual
//! scheduling logic.
//!
//! The Scheduler does not depend on `logging` or `indicatif` - it only
//! offers hooks. `coreforge-cli` is expected to provide the real
//! [`ProgressSink`] implementation that drives a progress bar; anything
//! that doesn't care (tests, `--dry-run`, library consumers) uses
//! [`NoProgress`].

use std::time::Duration;

use coreforge_core::ModuleId;

use crate::job::JobStatus;

/// Receives scheduling progress events as they happen.
///
/// Implementations must be safe to call concurrently: the Scheduler invokes
/// these from whichever worker thread is running a given module, possibly
/// several at once within the same build level.
pub trait ProgressSink: Sync {
    /// Called right before a module's [`crate::JobRunner::run`] is invoked.
    fn job_started(&self, module: &ModuleId) {
        let _ = module;
    }

    /// Called right after a module's job finishes, whether it succeeded,
    /// failed, or - for a module that was never actually run - was skipped.
    fn job_finished(&self, module: &ModuleId, status: &JobStatus, duration: Duration) {
        let _ = (module, status, duration);
    }
}

/// A [`ProgressSink`] that does nothing. The default when no UI is attached.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoProgress;

impl ProgressSink for NoProgress {}
