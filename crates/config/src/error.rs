//! Errors produced while reading `build-system.toml`.

/// Errors produced by the `config` crate.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// `build-system.toml` could not be read.
    #[error("I/O error reading {path}: {source}")]
    Io {
        /// The path that was being read.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// `build-system.toml` exists but is not valid.
    #[error("failed to parse {path}: {source}")]
    Parse {
        /// The path that failed to parse.
        path: String,
        /// The underlying TOML error.
        #[source]
        source: toml::de::Error,
    },
}

/// A convenience alias for `config` operations.
pub type Result<T> = std::result::Result<T, ConfigError>;
