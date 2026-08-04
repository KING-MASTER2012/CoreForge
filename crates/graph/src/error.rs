//! Error type for the Build Graph.

use coreforge_core::ModuleId;

/// Errors that can occur while building or querying a [`crate::BuildGraph`].
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    /// Two modules were added with the same id.
    #[error("duplicate module id: {0}")]
    DuplicateModule(ModuleId),

    /// A module's `depends` list references a module id that was never added
    /// to the graph - typically a typo in `coreforge.toml`.
    #[error("module '{module}' depends on unknown module '{dependency}'")]
    UnknownDependency {
        /// The module whose `depends` list contains the unknown id.
        module: ModuleId,
        /// The unresolvable dependency id.
        dependency: ModuleId,
    },

    /// A module declares itself as one of its own dependencies.
    #[error("module '{0}' depends on itself")]
    SelfDependency(ModuleId),

    /// The graph contains a dependency cycle, i.e. it is not a valid DAG.
    /// The listed modules are members of the same cyclic component; the
    /// cycle may involve some or all of them depending on its shape.
    #[error(
        "dependency cycle detected among module(s): {}",
        .0.iter().map(ToString::to_string).collect::<Vec<_>>().join(" -> ")
    )]
    CycleDetected(Vec<ModuleId>),
}

/// A convenience alias for `Result<T, GraphError>`.
pub type Result<T> = std::result::Result<T, GraphError>;
