//! `coreforge-inspector`
//!
//! Project Inspector (Phase 1).
//!
//! This crate walks a repository tree and infers each module's [`ModuleType`]
//! from well-known marker files (`Cargo.toml`, `CMakeLists.txt`, `package.json`,
//! `go.mod`, ...). It does **not** parse any build-system-specific manifest
//! (`coreforge.toml`) - that is the responsibility of `coreforge-manifest`
//! (Phase 2). It also does not build, schedule, or execute anything; it only
//! discovers *what* exists.

mod detect;
mod error;
mod walk;

pub use detect::detect_module_type;
pub use error::InspectorError;
pub use walk::{InspectConfig, inspect_repository};

use camino::Utf8PathBuf;
use coreforge_core::{Module, ModuleId, ModuleType};

/// A module discovered by the Project Inspector.
///
/// This is a thin wrapper around [`coreforge_core::Module`]; it exists as a
/// distinct type so that future phases (in particular the Manifest parser)
/// can distinguish "auto-discovered" modules from "manifest-declared" ones
/// without changing `coreforge-core`'s public API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredModule {
    /// The module's identifier, derived from its path relative to the repository root.
    pub id: ModuleId,
    /// The module's root directory, relative to the repository root.
    pub root: Utf8PathBuf,
    /// The toolchain inferred from the module's marker file(s).
    pub module_type: ModuleType,
}

impl From<DiscoveredModule> for Module {
    fn from(value: DiscoveredModule) -> Self {
        Module {
            id: value.id,
            root: value.root,
            module_type: value.module_type,
        }
    }
}
