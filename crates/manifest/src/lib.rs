//! `coreforge-manifest`
//!
//! Manifest parser (Phase 2).
//!
//! Reads each module's `coreforge.toml` and merges it with the modules
//! discovered by the Project Inspector (Phase 1):
//!
//! - For a module with a native marker file (`Cargo.toml`, `CMakeLists.txt`, ...),
//!   the manifest is optional and may override its id/type and declare its
//!   `depends`.
//! - For a module with **no** native marker file (e.g. a SQL migration set),
//!   the manifest is the sole source of truth and must declare `type`.
//!
//! This crate does not build, schedule, or validate the dependency graph -
//! that is [`coreforge_resolver`]'s job (Phase 3). It only produces a flat,
//! deduplicated list of [`coreforge_core::Module`]s with their `depends`
//! fields populated.

mod discover;
mod error;
mod load;
mod merge;
mod schema;

pub use discover::discover_manifest_only_modules;
pub use error::{ManifestError, Result};
pub use load::{find_manifest_path, read_manifest};
pub use merge::apply_manifest_overrides;
pub use schema::{MANIFEST_FILE_NAME, ManifestFile};

use std::collections::HashSet;

use camino::Utf8Path;
use coreforge_core::Module;
use inspector::InspectConfig;

/// Runs the full Phase 1 + Phase 2 pipeline over `root`:
///
/// 1. Discovers modules via native marker files ([`coreforge_inspector::inspect_repository`]).
/// 2. Applies each discovered module's `coreforge.toml` overrides, if present.
/// 3. Discovers additional manifest-only modules (no native marker file).
///
/// Returns a single, id-sorted list of modules. This is the primary entry
/// point later phases (the Dependency Resolver, in particular) should build on.
///
/// # Errors
///
/// Returns an error if the repository root is invalid (via the wrapped
/// [`coreforge_inspector::InspectorError`]), a `coreforge.toml` fails to
/// parse, or a manifest-only module is missing its required `type` field.
pub fn resolve_modules(root: &Utf8Path, inspector_config: &InspectConfig) -> Result<Vec<Module>> {
    let discovered = inspector::inspect_repository(root, inspector_config)?;

    let mut claimed_roots = HashSet::with_capacity(discovered.len());
    let mut modules: Vec<Module> = Vec::with_capacity(discovered.len());

    for discovered_module in discovered {
        let mut module: Module = discovered_module.into();
        apply_manifest_overrides(root, &mut module)?;
        claimed_roots.insert(module.root.clone());
        modules.push(module);
    }

    let manifest_only =
        discover_manifest_only_modules(root, &claimed_roots, inspector_config.max_depth)?;
    modules.extend(manifest_only);

    modules.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    Ok(modules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use coreforge_core::{ModuleId, ModuleType};
    use std::fs;

    fn temp_dir(name: &str) -> Utf8PathBuf {
        let dir = std::env::temp_dir().join(format!("coreforge-manifest-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Utf8PathBuf::from_path_buf(dir).unwrap()
    }

    #[test]
    fn native_marker_module_without_manifest_is_unchanged() {
        let root = temp_dir("no-manifest");
        let engine = root.join("engine");
        fs::create_dir_all(&engine).unwrap();
        fs::write(engine.join("Cargo.toml"), "[workspace]").unwrap();

        let modules = resolve_modules(&root, &InspectConfig::default()).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id.0, "engine");
        assert_eq!(modules[0].module_type, ModuleType::Cargo);
        assert!(modules[0].depends.is_empty());
    }

    #[test]
    fn manifest_overrides_id_type_and_depends() {
        let root = temp_dir("overrides");
        let editor = root.join("editor");
        fs::create_dir_all(&editor).unwrap();
        fs::write(editor.join("CMakeLists.txt"), "").unwrap();
        fs::write(
            editor.join("coreforge.toml"),
            r#"
                name = "editor-override"
                depends = ["engine"]
            "#,
        )
            .unwrap();

        let modules = resolve_modules(&root, &InspectConfig::default()).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].id.0, "editor-override");
        assert_eq!(modules[0].module_type, ModuleType::CMake);
        assert_eq!(modules[0].depends, vec![ModuleId::from("engine")]);
    }

    #[test]
    fn manifest_only_module_requires_type() {
        let root = temp_dir("manifest-only-missing-type");
        let sql = root.join("data").join("migrations");
        fs::create_dir_all(&sql).unwrap();
        fs::write(sql.join("coreforge.toml"), "depends = []").unwrap();

        let result = resolve_modules(&root, &InspectConfig::default());
        assert!(matches!(result, Err(ManifestError::MissingType { .. })));
    }

    #[test]
    fn manifest_only_module_is_discovered_with_type() {
        let root = temp_dir("manifest-only-ok");
        let sql = root.join("data").join("migrations");
        fs::create_dir_all(&sql).unwrap();
        fs::write(sql.join("coreforge.toml"), r#"type = "Sql""#).unwrap();

        let modules = resolve_modules(&root, &InspectConfig::default()).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_type, ModuleType::Sql);
        assert_eq!(modules[0].root, Utf8PathBuf::from("data/migrations"));
    }

    #[test]
    fn manifest_inside_claimed_module_is_not_a_separate_module() {
        // A coreforge.toml nested inside an already-detected Cargo module
        // (e.g. under a sub-crate) must not surface as its own module.
        let root = temp_dir("nested-inside-claimed");
        let engine = root.join("engine");
        fs::create_dir_all(&engine).unwrap();
        fs::write(engine.join("Cargo.toml"), "[workspace]").unwrap();
        let nested = engine.join("sql");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("coreforge.toml"), r#"type = "Sql""#).unwrap();

        let modules = resolve_modules(&root, &InspectConfig::default()).unwrap();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].module_type, ModuleType::Cargo);
    }
}
