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

    /// The repository-level `BUILD.core` file failed to parse.
    #[error("failed to parse BUILD.core at {path}: {source}")]
    BuildCoreParse {
        /// The path to `BUILD.core`.
        path: String,
        /// The underlying TOML parse error.
        #[source]
        source: toml::de::Error,
    },

    /// A declared `BUILD.core` target has an invalid path.
    #[error("invalid BUILD.core target '{name}' path '{path}': {reason}")]
    InvalidBuildTargetPath {
        /// Target name.
        name: String,
        /// Configured target path.
        path: String,
        /// Reason the path is invalid.
        reason: String,
    },

    /// Two declared `BUILD.core` targets claim overlapping directories.
    #[error("target '{first_name}' ({first_path}) overlaps target '{second_name}' ({second_path})")]
    OverlappingBuildTargets {
        /// First target name.
        first_name: String,
        /// First target path.
        first_path: String,
        /// Second target name.
        second_name: String,
        /// Second target path.
        second_path: String,
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
