//! Resolves a repository or workspace root into a [`Project`]: a build
//! graph plus every module's physical location on disk.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use coreforge_core::ModuleId;
use graph::BuildGraph;

use crate::error::{ExecutorError, Result};

/// A resolved repository or workspace: its dependency graph, plus every
/// module's physical root directory on disk. `BuildGraph` alone only knows
/// ids and dependency edges - actually building a module needs to know
/// where its source lives, which is what this adds.
#[derive(Debug)]
pub struct Project {
    /// The resolved dependency graph.
    pub graph: BuildGraph,
    /// Each module's absolute root directory.
    pub module_dirs: HashMap<ModuleId, Utf8PathBuf>,
}

/// Resolves `root` into a [`Project`].
///
/// Automatically picks workspace mode (multiple repositories declared in
/// `coreforge-workspace.toml`) or single-repository mode, based on whether
/// that file exists at `root`.
///
/// # Errors
///
/// Returns [`ExecutorError::Workspace`] / [`ExecutorError::Resolver`] if
/// resolution fails, or [`ExecutorError::MissingModuleLocation`] if a
/// workspace module has no corresponding physical location (should not
/// happen in practice).
pub fn resolve_project(root: &Utf8Path) -> Result<Project> {
    let inspector_config = inspector::InspectConfig::default();

    if coreforge_workspace::workspace_manifest_exists(root) {
        let workspace = coreforge_workspace::resolve(root, &inspector_config)?;
        let module_dirs = workspace
            .graph
            .modules()
            .map(|module| {
                let location = workspace
                    .module_location(&module.id)
                    .ok_or_else(|| ExecutorError::MissingModuleLocation(module.id.clone()))?;
                Ok((module.id.clone(), location.module_root.clone()))
            })
            .collect::<Result<HashMap<_, _>>>()?;
        Ok(Project {
            graph: workspace.graph,
            module_dirs,
        })
    } else {
        let graph = resolver::resolve(root, &inspector_config)?;
        let module_dirs = graph
            .modules()
            .map(|module| (module.id.clone(), root.join(&module.root)))
            .collect();
        Ok(Project { graph, module_dirs })
    }
}

/// Builds one [`toolchain::BuildContext`] per module: where its source
/// lives, where its build output should be written (namespaced under
/// `root/.coreforge/build/{sanitized module id}`), and which profile to
/// build with.
#[must_use]
pub fn build_contexts(
    module_dirs: &HashMap<ModuleId, Utf8PathBuf>,
    root: &Utf8Path,
    release: bool,
) -> HashMap<ModuleId, toolchain::BuildContext> {
    let profile = if release {
        toolchain::BuildProfile::Release
    } else {
        toolchain::BuildProfile::Debug
    };
    let build_root = root.join(".coreforge").join("build");

    module_dirs
        .iter()
        .map(|(id, module_dir)| {
            (
                id.clone(),
                toolchain::BuildContext {
                    module_dir: module_dir.clone(),
                    output_dir: build_root.join(id.sanitized()),
                    profile,
                },
            )
        })
        .collect()
}

/// Restricts `project.graph` to `modules` and everything they
/// transitively depend on. Returns `None` (meaning "use the whole graph
/// as-is") when `modules` is empty.
///
/// # Errors
///
/// Returns [`ExecutorError::Graph`] if any of `modules` doesn't exist in
/// the graph.
pub fn select_graph(project: &Project, modules: &[String]) -> Result<Option<BuildGraph>> {
    if modules.is_empty() {
        return Ok(None);
    }
    let targets = modules
        .iter()
        .map(|module| ModuleId::from(module.as_str()))
        .collect::<Vec<_>>();
    Ok(Some(project.graph.dependency_closure(&targets)?))
}
