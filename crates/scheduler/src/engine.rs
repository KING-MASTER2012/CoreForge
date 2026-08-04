//! The scheduling engine: runs a [`coreforge_graph::BuildGraph`]'s parallel
//! levels through a [`JobRunner`], skipping modules blocked by a failed or
//! skipped dependency.

use std::collections::HashSet;
use std::time::Instant;

use coreforge_core::ModuleId;
use graph::BuildGraph;
use rayon::prelude::*;

use crate::error::Result;
use crate::job::{JobOutcome, JobRunner, JobStatus};

/// Configuration for [`run_build`].
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Maximum number of modules to build at once, within a single level.
    /// `0` means "use the number of available CPUs"
    /// (`std::thread::available_parallelism`).
    pub parallel_jobs: usize,

    /// If `true`, stop starting new levels as soon as any module in the
    /// current level fails; every module in every not-yet-started level is
    /// reported as [`JobStatus::Skipped`]. Modules already running in the
    /// same level as the failure are allowed to finish - they cannot be
    /// preempted.
    ///
    /// If `false` (the default), scheduling continues for every module that
    /// is not itself blocked by the failure - similar to `make -k`.
    pub fail_fast: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            parallel_jobs: 0,
            fail_fast: false,
        }
    }
}

/// The result of scheduling an entire build: one [`JobOutcome`] per module in
/// the graph, in the order modules were scheduled (level by level).
#[derive(Debug, Clone, Default)]
pub struct SchedulerReport {
    /// Every module's outcome, in scheduling order.
    pub outcomes: Vec<JobOutcome>,
}

impl SchedulerReport {
    /// Whether every module either succeeded, or was skipped for a reason
    /// other than a genuine build failure (this build never had any
    /// [`JobStatus::Failed`] outcomes).
    #[must_use]
    pub fn is_success(&self) -> bool {
        !self.outcomes.iter().any(|o| o.status.is_failed())
    }

    /// The modules that failed to build.
    pub fn failures(&self) -> impl Iterator<Item = &JobOutcome> {
        self.outcomes.iter().filter(|o| o.status.is_failed())
    }

    /// The modules that were skipped (blocked by a failure, or by fail-fast).
    pub fn skipped(&self) -> impl Iterator<Item = &JobOutcome> {
        self.outcomes.iter().filter(|o| o.status.is_skipped())
    }
}

/// Schedules and runs every module in `graph` through `runner`, respecting
/// dependency order and running independent modules in parallel.
///
/// # Errors
///
/// Returns [`crate::SchedulerError::Graph`] if `graph` is not a valid DAG, or
/// [`crate::SchedulerError::ThreadPool`] if the dedicated thread pool for
/// this build could not be created.
pub fn run_build(
    graph: &BuildGraph,
    runner: &dyn JobRunner,
    config: &SchedulerConfig,
) -> Result<SchedulerReport> {
    let levels = graph.build_levels()?;

    let requested_threads = if config.parallel_jobs == 0 {
        std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1)
    } else {
        config.parallel_jobs
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(requested_threads)
        .build()?;

    let mut outcomes = Vec::with_capacity(graph.len());
    let mut blocked: HashSet<ModuleId> = HashSet::new();
    let mut stop_scheduling = false;

    for level in levels {
        if stop_scheduling {
            for id in level {
                outcomes.push(skipped_outcome(
                    id,
                    "fail-fast: an earlier level had a failure",
                ));
            }
            continue;
        }

        // Split the level into modules blocked by an already-failed/skipped
        // dependency (never run) and modules that are actually runnable.
        let mut runnable = Vec::new();
        for id in level {
            let module = graph
                .module(&id)
                .expect("module id came from this graph's own levels");
            if let Some(blocking_dep) = module.depends.iter().find(|d| blocked.contains(*d)) {
                blocked.insert(id.clone());
                outcomes.push(skipped_outcome(
                    id,
                    &format!("dependency '{blocking_dep}' failed or was skipped"),
                ));
            } else {
                runnable.push(module);
            }
        }

        if runnable.is_empty() {
            continue;
        }

        let level_outcomes: Vec<JobOutcome> = pool.install(|| {
            runnable
                .par_iter()
                .map(|module| {
                    let start = Instant::now();
                    let status = runner.run(module);
                    JobOutcome {
                        module: module.id.clone(),
                        status,
                        duration: start.elapsed(),
                    }
                })
                .collect()
        });

        let level_had_failure = level_outcomes.iter().any(|o| o.status.is_failed());
        for outcome in &level_outcomes {
            if outcome.status.is_failed() {
                blocked.insert(outcome.module.clone());
            }
        }
        outcomes.extend(level_outcomes);

        if level_had_failure && config.fail_fast {
            stop_scheduling = true;
        }
    }

    Ok(SchedulerReport { outcomes })
}

fn skipped_outcome(module: ModuleId, reason: &str) -> JobOutcome {
    JobOutcome {
        module,
        status: JobStatus::Skipped(reason.to_string()),
        duration: std::time::Duration::ZERO,
    }
}
