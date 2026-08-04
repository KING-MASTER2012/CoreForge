//! `coreforge-resolver`
//!
//! Dependency Resolver (Phase 3).
//!
//! This crate is the thin glue between the Manifest parser (Phase 1 + 2,
//! `coreforge-manifest`) and the Build Graph (`coreforge-graph`): it takes a
//! repository root, resolves the flat module list, and links it into a
//! [`coreforge_graph::BuildGraph`], surfacing any validation or cycle errors
//! along the way.
//!
//! It does not schedule or execute anything - that is
//! `coreforge-scheduler`/`coreforge-executor`'s job (Phase 4/5).

mod error;

pub use error::{ResolverError, Result};

use camino::Utf8Path;
use graph::BuildGraph;
use inspector::InspectConfig;

/// Runs the full pipeline over `root`: Project Inspector (Phase 1) -> Manifest
/// (Phase 2) -> Build Graph (Phase 3). Returns a [`BuildGraph`] ready for the
/// Scheduler (Phase 4) to consume.
///
/// # Errors
///
/// Returns [`ResolverError::Manifest`] if the repository root is invalid, a
/// `coreforge.toml` fails to parse, or a manifest-only module is missing its
/// required `type`. Returns [`ResolverError::Graph`] if the resolved modules
/// contain a duplicate id, an unknown dependency, a self-dependency, or a
/// dependency cycle.
pub fn resolve(root: &Utf8Path, inspector_config: &InspectConfig) -> Result<BuildGraph> {
    let modules = manifest::resolve_modules(root, inspector_config)?;
    let graph = BuildGraph::from_modules(modules)?;
    // `BuildGraph::from_modules` only links edges; it does not itself check
    // for cycles (that check is deferred to `build_order`/`build_levels` so
    // callers who only need `graph.modules()` don't pay for it). The
    // Dependency Resolver's job explicitly includes cycle detection, so we
    // validate eagerly here and surface the error immediately rather than
    // only when a later phase asks for a build order.
    graph.build_order()?;
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> camino::Utf8PathBuf {
        let dir = std::env::temp_dir().join(format!("coreforge-resolver-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        camino::Utf8PathBuf::from_path_buf(dir).unwrap()
    }

    #[test]
    fn resolves_a_small_repository_end_to_end() {
        let root = temp_dir("end-to-end");

        let engine = root.join("engine");
        fs::create_dir_all(&engine).unwrap();
        fs::write(engine.join("Cargo.toml"), "[workspace]").unwrap();

        let editor = root.join("applications").join("editor");
        fs::create_dir_all(&editor).unwrap();
        fs::write(editor.join("CMakeLists.txt"), "").unwrap();
        fs::write(editor.join("coreforge.toml"), r#"depends = ["engine"]"#).unwrap();

        let graph = resolve(&root, &InspectConfig::default()).unwrap();

        assert_eq!(graph.len(), 2);
        let order = graph.build_order().unwrap();
        let pos = |id: &str| order.iter().position(|m| m.0 == id).unwrap();
        assert!(pos("engine") < pos("applications-editor"));
    }

    #[test]
    fn cycle_across_manifests_is_reported() {
        let root = temp_dir("cycle");

        let a = root.join("a");
        fs::create_dir_all(&a).unwrap();
        fs::write(a.join("Cargo.toml"), "[package]\nname=\"a\"").unwrap();
        fs::write(a.join("coreforge.toml"), r#"depends = ["b"]"#).unwrap();

        let b = root.join("b");
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join("Cargo.toml"), "[package]\nname=\"b\"").unwrap();
        fs::write(b.join("coreforge.toml"), r#"depends = ["a"]"#).unwrap();

        let result = resolve(&root, &InspectConfig::default());
        assert!(matches!(
            result,
            Err(ResolverError::Graph(graph::GraphError::CycleDetected(_)))
        ));
    }
}
