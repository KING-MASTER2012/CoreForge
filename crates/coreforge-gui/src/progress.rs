//! The event types that flow from a background worker thread (running an
//! `executor::` pipeline) back to the UI thread, plus the
//! [`scheduler::ProgressSink`] that turns scheduler callbacks into those
//! events.
//!
//! `egui`/`eframe` is single-threaded: [`crate::app::CoreForgeApp::update`]
//! runs on the UI thread and must never block, so every `executor::`
//! call that might take a while (a real build, a Git sync) runs on a
//! spawned [`std::thread`] instead, reporting back through an
//! `mpsc` channel that `update` drains every frame.

use std::sync::Mutex;
use std::sync::mpsc::Sender;
use std::time::Duration;

use coreforge_core::ModuleId;

/// A message sent from a background worker thread to the UI thread.
pub enum GuiEvent {
    /// A module's build/test job started.
    JobStarted(ModuleId),
    /// A module's build/test job finished.
    JobFinished(ModuleId, JobOutcomeKind, Duration),
    /// `Build`/`Test`/`Package` finished (the whole pipeline, not one job).
    RunFinished(Result<RunSummary, String>),
    /// `Clean` finished, with the number of modules cleaned.
    CleanFinished(Result<usize, String>),
    /// `Workspace Sync` finished, with the number of repositories pinned.
    WorkspaceSyncFinished(Result<usize, String>),
}

/// A UI-friendly copy of [`scheduler::JobStatus`] that owns its data, so it
/// can be sent across the `mpsc` channel without borrowing from the
/// scheduler's own report.
pub enum JobOutcomeKind {
    /// The job succeeded.
    Success,
    /// The job failed, with a human-readable reason.
    Failed(String),
    /// The job was skipped, with a human-readable reason.
    Skipped(String),
}

impl From<&scheduler::JobStatus> for JobOutcomeKind {
    fn from(status: &scheduler::JobStatus) -> Self {
        match status {
            scheduler::JobStatus::Success => Self::Success,
            scheduler::JobStatus::Failed(reason) => Self::Failed(reason.clone()),
            scheduler::JobStatus::Skipped(reason) => Self::Skipped(reason.clone()),
        }
    }
}

/// A short, human-readable summary of a finished `Build`/`Test`/`Package`
/// run, printed as a single log line.
pub struct RunSummary {
    /// The command that produced this summary (`"Build"`, `"Test"`, or
    /// `"Package"`), for the log line.
    pub verb: &'static str,
    /// Modules that succeeded.
    pub succeeded: usize,
    /// Modules that failed.
    pub failed: usize,
    /// Modules that were skipped.
    pub skipped: usize,
    /// For `Package`: `(collected, total)` artifacts. `None` for
    /// `Build`/`Test`, which don't collect anything.
    pub collected: Option<(usize, usize)>,
}

impl std::fmt::Display for RunSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} completed: {} succeeded, {} failed, {} skipped.",
            self.verb, self.succeeded, self.failed, self.skipped
        )?;
        if let Some((collected, total)) = self.collected {
            write!(f, " {collected}/{total} artifacts collected under dist/.")?;
        }
        Ok(())
    }
}

/// Builds a [`RunSummary`] from an [`executor::BuildOutcome`].
#[must_use]
pub fn summarize(
    verb: &'static str,
    outcome: &executor::BuildOutcome,
    collected: Option<usize>,
) -> RunSummary {
    RunSummary {
        verb,
        succeeded: outcome
            .report
            .outcomes
            .iter()
            .filter(|job| job.status.is_success())
            .count(),
        failed: outcome.report.failures().count(),
        skipped: outcome.report.skipped().count(),
        collected: collected.map(|collected| (collected, outcome.artifacts.len())),
    }
}

/// A [`scheduler::ProgressSink`] that forwards every event to the UI
/// thread over an `mpsc` channel.
///
/// The sender is wrapped in a [`Mutex`] purely so this type is
/// unconditionally [`Sync`] (required by [`scheduler::ProgressSink`])
/// regardless of whether `mpsc::Sender` itself is - sending is cheap, so
/// the lock is never held for long.
pub struct ChannelProgress {
    sender: Mutex<Sender<GuiEvent>>,
}

impl ChannelProgress {
    /// Wraps `sender` for use as a [`scheduler::ProgressSink`].
    #[must_use]
    pub fn new(sender: Sender<GuiEvent>) -> Self {
        Self {
            sender: Mutex::new(sender),
        }
    }

    fn send(&self, event: GuiEvent) {
        let sender = self
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // The UI may have already been closed; a dropped receiver just
        // means there's nothing left to report to, not a real error.
        let _ = sender.send(event);
    }
}

impl scheduler::ProgressSink for ChannelProgress {
    fn job_started(&self, module: &ModuleId) {
        self.send(GuiEvent::JobStarted(module.clone()));
    }

    fn job_finished(&self, module: &ModuleId, status: &scheduler::JobStatus, duration: Duration) {
        self.send(GuiEvent::JobFinished(
            module.clone(),
            status.into(),
            duration,
        ));
    }
}
