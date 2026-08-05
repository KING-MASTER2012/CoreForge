//! Discovers modules that have no native marker file and are declared purely
//! via `coreforge.toml` (e.g. a database migration set that doesn't follow
//! the Supabase CLI's own `supabase/config.toml` convention).

use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use coreforge_core::{Module, ModuleId};
use inspector::DEFAULT_IGNORED_DIRS;
use walkdir::WalkDir;

use crate::error::{ManifestError, Result};
use crate::load::{find_manifest_path, read_manifest};

/// Walks `root` looking for directories that contain a `coreforge.toml` but
/// have no native marker file, i.e. modules whose *only* source of truth is
/// the manifest itself.
///
/// `claimed_roots` are the (root-relative) directories of modules already
/// discovered by the Project Inspector. The walk does not descend into them:
/// a `coreforge.toml` nested inside an already-claimed module's directory is
/// treated as that module's internal detail, not as a separate module. This
/// mirrors the Project Inspector's own "stop descending once matched" rule.
///
/// # Errors
///
/// Returns [`ManifestError::Parse`] or [`ManifestError::Io`] if a manifest
/// exists but cannot be read, [`ManifestError::MissingType`] if a
/// manifest-only module does not declare `type`, and [`ManifestError::Walk`]
/// or [`ManifestError::NonUtf8Path`] on directory-walk failures.
pub fn discover_manifest_only_modules(
    root: &Utf8Path,
    claimed_roots: &HashSet<Utf8PathBuf>,
    max_depth: usize,
) -> Result<Vec<Module>> {
    let ignored_dirs: HashSet<&str> = DEFAULT_IGNORED_DIRS.iter().copied().collect();
    let mut discovered = Vec::new();

    let mut walker = WalkDir::new(root).max_depth(max_depth).into_iter();

    while let Some(entry) = walker.next() {
        let entry = entry?;

        if !entry.file_type().is_dir() {
            continue;
        }

        let path = entry.path();
        let Some(utf8_path) = Utf8Path::from_path(path) else {
            return Err(ManifestError::NonUtf8Path(
                path.to_string_lossy().into_owned(),
            ));
        };
        let relative = utf8_path.strip_prefix(root).unwrap_or(utf8_path);

        if entry.depth() > 0 {
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('.') || ignored_dirs.contains(name.as_ref()) {
                walker.skip_current_dir();
                continue;
            }
        }

        if claimed_roots.contains(relative) {
            // Already a module found via a native marker file - its
            // internals are not scanned for further modules.
            walker.skip_current_dir();
            continue;
        }

        let Some(manifest_path) = find_manifest_path(utf8_path) else {
            continue;
        };

        let manifest = read_manifest(&manifest_path)?;
        let Some(module_type) = manifest.module_type else {
            return Err(ManifestError::MissingType {
                path: manifest_path.to_string(),
            });
        };

        let id = manifest
            .name
            .map(ModuleId::from)
            .unwrap_or_else(|| module_id_from_path(relative));

        discovered.push(Module {
            id,
            root: relative.to_path_buf(),
            module_type,
            depends: manifest.depends.into_iter().map(ModuleId::from).collect(),
        });

        // A manifest-only module's directory is not scanned any further either.
        walker.skip_current_dir();
    }

    Ok(discovered)
}

/// Derives a module id from a path relative to the repository root, e.g.
/// `data/migrations` -> `data-migrations`.
fn module_id_from_path(relative: &Utf8Path) -> ModuleId {
    let id = relative.as_str().replace(['/', '\\'], "-");
    ModuleId::from(if id.is_empty() { ".".to_string() } else { id })
}
