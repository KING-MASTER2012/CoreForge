//! Repository tree walking.

use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use coreforge_core::ModuleId;
use walkdir::WalkDir;

use crate::DiscoveredModule;
use crate::detect::detect_module_type;
use crate::error::{InspectorError, Result};

/// Directory names that are never treated as module roots and are never
/// descended into, regardless of whether they contain a marker file.
///
/// This covers VCS metadata, build output, and third-party/vendored code -
/// none of which should ever be reported as a first-class CoreForge module.
const DEFAULT_IGNORED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "target",
    "build",
    "dist",
    "out",
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
    "third_party",
    "vendor",
    ".idea",
    ".vs",
    ".vscode",
];

/// Configuration for [`inspect_repository`].
#[derive(Debug, Clone)]
pub struct InspectConfig {
    /// Directory names to skip entirely while walking (matched by exact name,
    /// not by path). Defaults to [`DEFAULT_IGNORED_DIRS`].
    pub ignored_dirs: HashSet<String>,
    /// Maximum recursion depth from the repository root. Defaults to `6`,
    /// which comfortably covers realistic monorepo layouts while still
    /// guarding against pathological or accidentally-symlinked trees.
    pub max_depth: usize,
}

impl Default for InspectConfig {
    fn default() -> Self {
        Self {
            ignored_dirs: DEFAULT_IGNORED_DIRS.iter().map(|s| (*s).to_string()).collect(),
            max_depth: 6,
        }
    }
}

/// Walks `root` and returns every discovered module.
///
/// Once a directory is recognized as a module root (i.e. [`detect_module_type`]
/// returns `Some`), the walk does not descend further into that directory -
/// its subdirectories are considered implementation details of that module,
/// not separate modules in their own right.
///
/// # Errors
///
/// Returns [`InspectorError::InvalidRoot`] if `root` does not exist or is not
/// a directory, [`InspectorError::NonUtf8Path`] if a discovered path is not
/// valid UTF-8, and [`InspectorError::Walk`] on directory-read failures
/// (permission errors, broken symlinks, etc.).
pub fn inspect_repository(root: &Utf8Path, config: &InspectConfig) -> Result<Vec<DiscoveredModule>> {
    if !root.is_dir() {
        return Err(InspectorError::InvalidRoot(root.to_string()));
    }

    let mut discovered = Vec::new();
    let mut walker = WalkDir::new(root)
        .max_depth(config.max_depth)
        .into_iter();

    while let Some(entry) = walker.next() {
        let entry = entry?;

        if !entry.file_type().is_dir() {
            continue;
        }

        let path = entry.path();
        let Some(utf8_path) = Utf8Path::from_path(path) else {
            return Err(InspectorError::NonUtf8Path(path.to_string_lossy().into_owned()));
        };

        // The repository root itself is never skipped by the ignore list,
        // even if its name happens to collide with an ignored name.
        if entry.depth() > 0 {
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('.') || config.ignored_dirs.contains(name.as_ref()) {
                walker.skip_current_dir();
                continue;
            }
        }

        if let Some(module_type) = detect_module_type(utf8_path) {
            discovered.push(DiscoveredModule {
                id: module_id_for(root, utf8_path),
                root: relative_to(root, utf8_path),
                module_type,
            });
            // Do not descend further - the module's internals are its own business.
            walker.skip_current_dir();
        }
    }

    // Directory-read order is not guaranteed; sort for deterministic output.
    discovered.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    Ok(discovered)
}

/// Derives a module id from a discovered module's path relative to the
/// repository root, e.g. `applications/launcher` -> `applications-launcher`.
fn module_id_for(root: &Utf8Path, dir: &Utf8Path) -> ModuleId {
    let relative = relative_to(root, dir);
    let id = relative.as_str().replace(['/', '\\'], "-");
    ModuleId::from(if id.is_empty() { ".".to_string() } else { id })
}

fn relative_to(root: &Utf8Path, dir: &Utf8Path) -> Utf8PathBuf {
    dir.strip_prefix(root).map_or_else(|_| dir.to_path_buf(), Utf8Path::to_path_buf)
}
