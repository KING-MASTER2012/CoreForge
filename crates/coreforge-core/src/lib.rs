//! `coreforge-core`
//!
//! Common types that all other coreforge crates depend on reside here:
//! `Module`, `ModuleId`, `ModuleType` and the shared `Error` enum. //!
//! NOTE: This crate is still in the skeleton stage (Phase 0). `Module` and `ModuleType`
//! will be populated along with Phase 1 (Project Inspector) and Phase 2 (Manifest).
use serde::{Deserialize, Serialize};

/// The unique identifier of a build module (e.g., "engine-rust", "editor-qt").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleId(pub String);

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Specifies which toolchain a module was compiled with.
/// To be expanded with Phase 5 (Tool Adapter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleType {
    Cargo,
    CMake,
    Npm,
    Tauri,
    Go,
    Sql,
    Python,
}

/// This is a bug type shared across CoreForge.
#[derive(Debug, thiserror::Error)]
pub enum CoreForgeError {
    #[error("Module not found: {0}")]
    ModuleNotFound(String),

    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CoreForgeError>;
