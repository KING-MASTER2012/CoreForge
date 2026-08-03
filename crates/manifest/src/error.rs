//! Error type for the Manifest parser.

/// Errors that can occur while loading or applying `coreforge.toml` manifests.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// A `coreforge.toml` file failed to parse.
    #[error("failed to parse manifest at {path}: {source}")]
    Parse {
        /// The manifest file's path.
        path: String,
        /// The underlying TOML parse error.
        #[source]
        source: toml::de::Error,
    },

    /// A manifest-only module (no native marker file) did not declare `type`.
    #[error(
        "manifest at {path} must declare 'type' - no native marker file was found for this module"
    )]
    MissingType {
        /// The manifest file's path.
        path: String,
    },

    /// A path was expected to be valid UTF-8 but was not.
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(String),

    /// A wrapped I/O error, typically from reading a manifest file.
    #[error("I/O error reading {path}: {source}")]
    Io {
        /// The path being accessed when the error occurred.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A wrapped `walkdir` error encountered while searching for manifest-only modules.
    #[error("failed to walk directory tree: {0}")]
    Walk(#[from] walkdir::Error),

    /// A wrapped error from the Project Inspector (Phase 1).
    #[error(transparent)]
    Inspector(#[from] inspector::InspectorError),
}

/// A convenience alias for `Result<T, ManifestError>`.
pub type Result<T> = std::result::Result<T, ManifestError>;
