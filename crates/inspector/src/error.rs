//! Error type for the Project Inspector.

/// Errors that can occur while inspecting a repository tree.
#[derive(Debug, thiserror::Error)]
pub enum InspectorError {
    /// The given repository root does not exist or is not a directory.
    #[error("repository root does not exist or is not a directory: {0}")]
    InvalidRoot(String),

    /// A directory entry's path is not valid UTF-8.
    ///
    /// CoreForge represents all paths as [`camino::Utf8PathBuf`], so any
    /// non-UTF-8 path encountered while walking the tree is reported here
    /// instead of being silently skipped.
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(String),

    /// A wrapped `walkdir` error (e.g. a broken symlink or a permission error
    /// encountered while walking the directory tree).
    #[error("failed to walk directory tree: {0}")]
    Walk(#[from] walkdir::Error),
}

/// A convenience alias for `Result<T, InspectorError>`.
pub type Result<T> = std::result::Result<T, InspectorError>;
