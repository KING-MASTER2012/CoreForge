//! Error type for the Dependency Resolver.

/// Errors that can occur while resolving a repository into a [`crate::resolve`]d
/// [`coreforge_graph::BuildGraph`].
#[derive(Debug, thiserror::Error)]
pub enum ResolverError {
    /// An error from the Manifest parser (Phase 1 + 2): an invalid
    /// repository root, an unparsable `coreforge.toml`, or a manifest-only
    /// module missing its required `type`.
    #[error(transparent)]
    Manifest(#[from] manifest::ManifestError),

    /// An error from the Build Graph: a duplicate module id, an unknown
    /// dependency, a self-dependency, or a dependency cycle.
    #[error(transparent)]
    Graph(#[from] graph::GraphError),
}

/// A convenience alias for `Result<T, ResolverError>`.
pub type Result<T> = std::result::Result<T, ResolverError>;
