//! Error type for the Job Scheduler.

/// Errors that can occur while scheduling a build.
///
/// Note that an individual module's build *failing* is not represented here
/// - that is a normal, expected outcome captured as
/// [`crate::JobStatus::Failed`] in the [`crate::SchedulerReport`]. This enum
/// only covers conditions that prevent scheduling from running at all.
#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    /// The Build Graph is not a valid DAG (see [`coreforge_graph::GraphError`]).
    #[error(transparent)]
    Graph(#[from] graph::GraphError),

    /// The dedicated Rayon thread pool for this build could not be created.
    #[error("failed to create thread pool: {0}")]
    ThreadPool(#[from] rayon::ThreadPoolBuildError),
}

/// A convenience alias for `Result<T, SchedulerError>`.
pub type Result<T> = std::result::Result<T, SchedulerError>;
