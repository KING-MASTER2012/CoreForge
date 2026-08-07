//! `coreforge-core`
//!
//! Common types shared by every other CoreForge crate: [`Module`], [`ModuleId`],
//! [`ModuleType`], and the shared [`CoreForgeError`] enum.
//!
//! This crate intentionally has no dependency on any other `coreforge-*` crate -
//! it sits at the bottom of the dependency graph.

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// The unique identifier of a build module (e.g. `"engine"`, `"editor"`).
///
/// For modules discovered by the Project Inspector (Phase 1), this is derived
/// from the module's path relative to the repository root. Once the Manifest
/// parser (Phase 2) lands, a module may override its id explicitly via
/// `coreforge.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ModuleId(pub String);

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ModuleId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ModuleId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl ModuleId {
    /// A filesystem-safe representation of this id.
    ///
    /// Workspace ids are namespaced as `repository::module` (see
    /// `coreforge-workspace`), and `::` is not a legal path character on
    /// Windows. Every character that is not an ASCII letter, digit, `-` or
    /// `_` is replaced with `_`. Used wherever an id is turned into a path
    /// segment: managed build output directories and the `dist/` layout.
    #[must_use]
    pub fn sanitized(&self) -> String {
        self.0
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                    character
                } else {
                    '_'
                }
            })
            .collect()
    }
}

/// The toolchain a module is built with.
///
/// This determines which Tool Adapter (Phase 5, `coreforge-toolchain`) is
/// responsible for building the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleType {
    /// A Rust crate or workspace, identified by `Cargo.toml`.
    #[serde(alias = "Cargo")]
    Cargo,
    /// A CMake project, identified by `CMakeLists.txt`.
    #[serde(alias = "CMake")]
    CMake,
    /// A plain Node.js/npm package, identified by `package.json`.
    #[serde(alias = "Npm")]
    Npm,
    /// A Tauri application, identified by `package.json` plus a `src-tauri/` directory.
    #[serde(alias = "Tauri")]
    Tauri,
    /// A Go module, identified by `go.mod`.
    #[serde(alias = "Go")]
    Go,
    /// A SQL migration set. Identified by `supabase/config.toml` (the
    /// Supabase CLI's own project convention), or declared explicitly via
    /// `coreforge.toml` for projects that don't follow that layout.
    #[serde(alias = "Sql")]
    Sql,
    /// A Python package, identified by `pyproject.toml` or `requirements.txt`.
    #[serde(alias = "Python")]
    Python,
}

impl std::fmt::Display for ModuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Cargo => "Cargo",
            Self::CMake => "CMake",
            Self::Npm => "Npm",
            Self::Tauri => "Tauri",
            Self::Go => "Go",
            Self::Sql => "Sql",
            Self::Python => "Python",
        };
        write!(f, "{label}")
    }
}

/// A build module discovered in, or declared for, the target repository.
///
/// Populated by the Project Inspector (Phase 1) from native marker files,
/// then enriched by the Manifest parser (Phase 2) with any `coreforge.toml`
/// overrides and dependency declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    /// The module's unique identifier.
    pub id: ModuleId,
    /// The module's root directory, relative to the repository root.
    pub root: Utf8PathBuf,
    /// The toolchain used to build this module.
    pub module_type: ModuleType,
    /// Ids of the modules this module depends on. Populated from
    /// `coreforge.toml`'s `depends` field; empty for modules with no
    /// manifest (Phase 3's Dependency Resolver treats an empty list as
    /// "no dependencies", not as "unresolved").
    pub depends: Vec<ModuleId>,
}

/// The error type shared across all CoreForge crates.
#[derive(Debug, thiserror::Error)]
pub enum CoreForgeError {
    /// A module with the given id could not be found.
    #[error("module not found: {0}")]
    ModuleNotFound(String),

    /// A manifest file failed to parse or was semantically invalid.
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    /// A path was expected to be valid UTF-8 but was not.
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(String),

    /// A wrapped I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A convenience alias for `Result<T, CoreForgeError>`.
pub type Result<T> = std::result::Result<T, CoreForgeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitized_replaces_illegal_path_characters() {
        assert_eq!(ModuleId::from("engine").sanitized(), "engine");
        assert_eq!(
            ModuleId::from("engine::engine").sanitized(),
            "engine__engine"
        );
        assert_eq!(
            ModuleId::from("coreverse-server").sanitized(),
            "coreverse-server"
        );
    }
}
