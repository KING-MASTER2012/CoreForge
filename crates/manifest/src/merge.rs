//! Applying `coreforge.toml` overrides to modules already discovered by the
//! Project Inspector (Phase 1).

use camino::Utf8Path;
use coreforge_core::{Module, ModuleId};

use crate::error::Result;
use crate::load::{find_manifest_path, read_manifest};

/// If `module` has a `coreforge.toml` in its root directory, applies its
/// overrides in place:
///
/// - `name` overrides `module.id`.
/// - `type` overrides `module.module_type`.
/// - `depends` replaces `module.depends`.
///
/// Does nothing if no manifest is present - a module discovered purely from
/// its native marker file is left exactly as the Project Inspector found it.
///
/// # Errors
///
/// Returns [`crate::ManifestError::Parse`] or [`crate::ManifestError::Io`] if
/// a manifest is present but cannot be read or parsed.
pub fn apply_manifest_overrides(root: &Utf8Path, module: &mut Module) -> Result<()> {
    let module_dir = root.join(&module.root);

    let Some(manifest_path) = find_manifest_path(&module_dir) else {
        return Ok(());
    };

    let manifest = read_manifest(&manifest_path)?;

    if let Some(name) = manifest.name {
        module.id = ModuleId::from(name);
    }
    if let Some(module_type) = manifest.module_type {
        module.module_type = module_type;
    }
    module.depends = manifest.depends.into_iter().map(ModuleId::from).collect();

    Ok(())
}
