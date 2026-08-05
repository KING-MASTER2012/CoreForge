//! The raw `coreforge.toml` schema.

use coreforge_core::ModuleType;
use serde::Deserialize;

/// The filename CoreForge looks for in each module's root directory.
pub const MANIFEST_FILE_NAME: &str = "coreforge.toml";

/// The raw, deserialized contents of a `coreforge.toml` file.
///
/// Every field is optional for a module that already has a native marker
/// file (`Cargo.toml`, `CMakeLists.txt`, ...) - in that case the manifest
/// only needs to add what the native file can't express (an id override,
/// extra dependencies, ...). For a module with **no** native marker (e.g. a
/// database migration set that doesn't follow the Supabase CLI's own
/// `supabase/config.toml` convention), `type` is required, since there is
/// nothing else to infer it from; see [`crate::discover_manifest_only_modules`].
///
/// Unknown fields are ignored rather than rejected, so that later phases
/// (packaging behavior, etc.) can extend this schema without breaking
/// manifests written against an earlier version of CoreForge.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ManifestFile {
    /// Overrides the module's auto-derived id.
    pub name: Option<String>,

    /// Overrides (or, for manifest-only modules, defines) the module's type.
    #[serde(rename = "type")]
    pub module_type: Option<ModuleType>,

    /// Ids of the modules this module depends on.
    #[serde(default)]
    pub depends: Vec<String>,
}
