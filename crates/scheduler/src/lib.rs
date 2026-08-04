//! `coreforge-scheduler`
//!
//! Job Scheduler (Phase 4).
//!
//! Consumes a [`coreforge_graph::BuildGraph`]'s parallel levels
//! ([`coreforge_graph::BuildGraph::build_levels`]) and runs each level's
//! modules concurrently through a pluggable [`JobRunner`], bounded by a
//! configurable number of parallel jobs. If a module fails, every module
//! that (transitively) depends on it is automatically skipped rather than
//! attempted.
//!
//! This crate does not know how to actually build anything - that is
//! `coreforge-toolchain`'s job (Phase 5). See [`DryRunRunner`] for a
//! placeholder implementation used until then.

mod engine;
mod error;
mod job;

pub use engine::{SchedulerConfig, SchedulerReport, run_build};
pub use error::{Result, SchedulerError};
pub use job::{DryRunRunner, JobOutcome, JobRunner, JobStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use coreforge_core::{Module, ModuleId, ModuleType};
    use std::collections::HashSet;
    use std::sync::Mutex;

    fn module(id: &str, depends: &[&str]) -> Module {
        Module {
            id: ModuleId::from(id),
            root: camino::Utf8PathBuf::from(id),
            module_type: ModuleType::Cargo,
            depends: depends.iter().map(|d| ModuleId::from(*d)).collect(),
        }
    }

    /// A [`JobRunner`] that fails for a configured set of module ids and
    /// records, in call order, which modules it was actually asked to run
    /// (letting tests assert that skipped modules are never invoked).
    struct RecordingRunner {
        fail_for: HashSet<String>,
        calls: Mutex<Vec<String>>,
    }

    impl RecordingRunner {
        fn new(fail_for: &[&str]) -> Self {
            Self {
                fail_for: fail_for.iter().map(|s| (*s).to_string()).collect(),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl JobRunner for RecordingRunner {
        fn run(&self, module: &Module) -> JobStatus {
            self.calls.lock().unwrap().push(module.id.0.clone());
            if self.fail_for.contains(&module.id.0) {
                JobStatus::Failed("simulated failure".to_string())
            } else {
                JobStatus::Success
            }
        }
    }

    #[test]
    fn dry_run_runner_succeeds_for_every_module() {
        let graph = graph::BuildGraph::from_modules(vec![
            module("engine", &[]),
            module("editor", &["engine"]),
        ])
        .unwrap();

        let report = run_build(&graph, &DryRunRunner, &SchedulerConfig::default()).unwrap();

        assert_eq!(report.outcomes.len(), 2);
        assert!(report.is_success());
        assert_eq!(report.failures().count(), 0);
        assert_eq!(report.skipped().count(), 0);
    }

    #[test]
    fn failure_blocks_only_transitive_dependents() {
        // engine (fails) + independent (no relation) at level 0;
        // editor depends on engine (level 1) -> must be skipped;
        // standalone depends on independent (level 1) -> must still run.
        let graph = graph::BuildGraph::from_modules(vec![
            module("engine", &[]),
            module("independent", &[]),
            module("editor", &["engine"]),
            module("standalone", &["independent"]),
        ])
        .unwrap();

        let runner = RecordingRunner::new(&["engine"]);
        let report = run_build(
            &graph,
            &runner,
            &SchedulerConfig {
                parallel_jobs: 4,
                fail_fast: false,
            },
        )
        .unwrap();

        assert!(!report.is_success());

        let status_of = |id: &str| {
            report
                .outcomes
                .iter()
                .find(|o| o.module.0 == id)
                .map(|o| o.status.clone())
                .unwrap()
        };

        assert_eq!(
            status_of("engine"),
            JobStatus::Failed("simulated failure".to_string())
        );
        assert_eq!(status_of("independent"), JobStatus::Success);
        assert_eq!(status_of("standalone"), JobStatus::Success);
        assert!(status_of("editor").is_skipped());

        // The skipped module must never have actually been invoked.
        let calls = runner.calls.lock().unwrap();
        assert!(!calls.contains(&"editor".to_string()));
        assert!(calls.contains(&"standalone".to_string()));
    }

    #[test]
    fn fail_fast_skips_unstarted_levels_entirely() {
        // engine (fails) + independent (no relation) at level 0;
        // standalone depends on independent (level 1) - not blocked by the
        // failure, but fail-fast should still skip it because level 1 never starts.
        let graph = graph::BuildGraph::from_modules(vec![
            module("engine", &[]),
            module("independent", &[]),
            module("standalone", &["independent"]),
        ])
        .unwrap();

        let runner = RecordingRunner::new(&["engine"]);
        let report = run_build(
            &graph,
            &runner,
            &SchedulerConfig {
                parallel_jobs: 4,
                fail_fast: true,
            },
        )
        .unwrap();

        let status_of = |id: &str| {
            report
                .outcomes
                .iter()
                .find(|o| o.module.0 == id)
                .map(|o| o.status.clone())
                .unwrap()
        };

        assert_eq!(
            status_of("engine"),
            JobStatus::Failed("simulated failure".to_string())
        );
        // Same level as the failure: already dispatched, allowed to finish.
        assert_eq!(status_of("independent"), JobStatus::Success);
        // Next level never started.
        assert!(status_of("standalone").is_skipped());

        let calls = runner.calls.lock().unwrap();
        assert!(!calls.contains(&"standalone".to_string()));
    }

    #[test]
    fn cycle_is_reported_as_scheduler_error() {
        let graph = graph::BuildGraph::from_modules(vec![module("a", &["b"]), module("b", &["a"])])
            .unwrap();

        let result = run_build(&graph, &DryRunRunner, &SchedulerConfig::default());
        assert!(matches!(result, Err(SchedulerError::Graph(_))));
    }
}
