//! `config`
//!
//! Configuration (Phase 8).
//!
//! Reads `build-system.toml`, the file that supplies default values for
//! build behavior (`--release`, `-j`, `--fail-fast`'s cousins) so they don't
//! have to be typed on every `coreforge build` invocation.
//!
//! This is deliberately a different file, and a different crate, from
//! `coreforge-workspace.toml` (`coreforge-workspace`): that file declares
//! *which repositories* make up a workspace, this one declares *how a
//! build should behave*. They answer unrelated questions and are allowed to
//! evolve independently.
//!
//! v1 looks for a single `build-system.toml` at the workspace/repository
//! root only - no per-repository override layer yet. If a real need for
//! that shows up, it can be added without breaking this schema (every field
//! is already optional and merged, not replaced).

mod error;

pub use error::{ConfigError, Result};

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

/// The filename CoreForge looks for at the workspace/repository root.
pub const BUILD_SYSTEM_FILE_NAME: &str = "build-system.toml";

/// The build configuration (optimization profile) requested by
/// `build-system.toml`. Mirrors `toolchain::BuildProfile`; this crate does
/// not depend on `toolchain` so it stays a leaf, but the two are meant to
/// be converted into one another at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Configuration {
    /// Development-oriented build output.
    Debug,
    /// Optimized build output.
    Release,
}

/// The raw, deserialized contents of a `build-system.toml` file.
///
/// Every field is optional: a field left unset here falls back to whatever
/// the CLI's own built-in default is. CoreForge's precedence rule is
/// CLI flag > `build-system.toml` > built-in default; merging the three is
/// the caller's responsibility (typically `coreforge-cli`), since only the
/// caller knows which CLI flags were actually passed on this invocation.
///
/// Unknown fields are rejected rather than silently ignored, since a typo'd
/// key here (unlike a stale `coreforge.toml` field) would otherwise fail
/// silently and just build with the wrong settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSystemConfig {
    /// Default `-j`/`--jobs`: maximum modules to build at once.
    #[serde(default)]
    pub parallel_jobs: Option<usize>,
    /// Default build configuration (`--release` if `Release`).
    #[serde(default)]
    pub configuration: Option<Configuration>,
    /// Whether compiler warnings should be treated as errors by default.
    #[serde(default)]
    pub warnings_as_errors: Option<bool>,
    /// Whether `coreforge build` should also build test targets by default.
    #[serde(default)]
    pub build_tests: Option<bool>,
}

/// Returns `root/build-system.toml` if it exists.
#[must_use]
pub fn discover(root: &Utf8Path) -> Option<Utf8PathBuf> {
    let path = root.join(BUILD_SYSTEM_FILE_NAME);
    path.is_file().then_some(path)
}

/// Reads and parses a `build-system.toml` file at `path`.
///
/// # Errors
///
/// Returns [`ConfigError::Io`] if `path` cannot be read, or
/// [`ConfigError::Parse`] if its contents are not a valid
/// [`BuildSystemConfig`].
pub fn load(path: &Utf8Path) -> Result<BuildSystemConfig> {
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_string(),
        source,
    })?;
    toml::from_str(&contents).map_err(|source| ConfigError::Parse {
        path: path.to_string(),
        source,
    })
}

/// Discovers and loads `build-system.toml` from `root`, if present.
///
/// Returns `Ok(None)` (not an error) when no such file exists - the file is
/// entirely optional, and every field already falls back to a built-in
/// default.
///
/// # Errors
///
/// Returns [`ConfigError::Parse`] if `root/build-system.toml` exists but is
/// not valid.
pub fn load_from_root(root: &Utf8Path) -> Result<Option<BuildSystemConfig>> {
    discover(root).map(|path| load(&path)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn missing_file_is_not_an_error() {
        let dir = tempdir();
        assert!(load_from_root(&dir).unwrap().is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parses_every_field() {
        let dir = tempdir();
        fs::write(
            dir.join(BUILD_SYSTEM_FILE_NAME),
            "parallel_jobs = 16\n\
             configuration = \"Release\"\n\
             warnings_as_errors = true\n\
             build_tests = false\n",
        )
            .unwrap();

        let config = load_from_root(&dir).unwrap().unwrap();
        assert_eq!(config.parallel_jobs, Some(16));
        assert_eq!(config.configuration, Some(Configuration::Release));
        assert_eq!(config.warnings_as_errors, Some(true));
        assert_eq!(config.build_tests, Some(false));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn partial_files_leave_unset_fields_as_none() {
        let dir = tempdir();
        fs::write(dir.join(BUILD_SYSTEM_FILE_NAME), "parallel_jobs = 4\n").unwrap();

        let config = load_from_root(&dir).unwrap().unwrap();
        assert_eq!(config.parallel_jobs, Some(4));
        assert_eq!(config.configuration, None);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_unknown_fields() {
        let dir = tempdir();
        fs::write(dir.join(BUILD_SYSTEM_FILE_NAME), "typo_field = true\n").unwrap();

        assert!(load_from_root(&dir).is_err());

        fs::remove_dir_all(&dir).unwrap();
    }

    fn tempdir() -> Utf8PathBuf {
        let path = Utf8PathBuf::from_path_buf(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "coreforge-config-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
