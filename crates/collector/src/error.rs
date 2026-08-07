//! Errors produced while collecting build artifacts into `dist/`.

/// Errors produced by the Artifact Collector.
#[derive(Debug, thiserror::Error)]
pub enum CollectorError {
    /// A filesystem operation failed.
    #[error("I/O error accessing {path}: {source}")]
    Io {
        /// The path that was being read, written, or created.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The `dist-manifest.json` file could not be serialized.
    #[error("failed to serialize dist-manifest.json: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// A convenience alias for collector operations.
pub type Result<T> = std::result::Result<T, CollectorError>;
