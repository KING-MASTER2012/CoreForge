//! Repository-level declarative module definitions from `BUILD.core`.

use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use coreforge_core::{Module, ModuleId, ModuleType};
use serde::Deserialize;

use crate::error::{ManifestError, Result};

/// The repository-level declarative build file name.
pub const BUILD_CORE_FILE_NAME: &str = "BUILD.core";

/// The raw contents of a repository-level `BUILD.core` file.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildCoreFile {
    /// Explicit module targets declared by the repository.
    #[serde(default, rename = "target")]
    pub targets: Vec<TargetDef>,
}

/// A single explicit module target from `BUILD.core`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetDef {
    /// Toolchain kind for the target.
    pub kind: ModuleType,
    /// Unique module id within the repository.
    pub name: String,
    /// Module directory relative to the repository root.
    pub path: Utf8PathBuf,
    /// Module dependencies.
    #[serde(default)]
    pub depends: Vec<String>,
}

/// Explicit modules and the paths they claim from automatic discovery.
pub(crate) struct BuildCoreTargets {
    pub(crate) modules: Vec<Module>,
    pub(crate) claimed_roots: HashSet<Utf8PathBuf>,
}

/// Reads `BUILD.core` if it exists in `root`.
pub fn read_build_core(root: &Utf8Path) -> Result<Option<BuildCoreFile>> {
    let path = root.join(BUILD_CORE_FILE_NAME);
    if !path.is_file() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&path).map_err(|source| ManifestError::Io {
        path: path.to_string(),
        source,
    })?;
    toml::from_str(&contents)
        .map(Some)
        .map_err(|source| ManifestError::BuildCoreParse {
            path: path.to_string(),
            source,
        })
}

/// Resolves explicit `BUILD.core` targets into modules and claimed roots.
pub(crate) fn resolve_build_core_targets(root: &Utf8Path) -> Result<BuildCoreTargets> {
    let Some(file) = read_build_core(root)? else {
        return Ok(BuildCoreTargets {
            modules: Vec::new(),
            claimed_roots: HashSet::new(),
        });
    };

    let canonical_root = canonicalize(root, "BUILD.core", root)?;
    let mut declared = Vec::with_capacity(file.targets.len());

    for target in file.targets {
        if target.name.is_empty() {
            return Err(ManifestError::InvalidBuildTargetPath {
                name: target.name,
                path: target.path.to_string(),
                reason: "target name must not be empty".to_string(),
            });
        }
        if target.path.is_absolute() {
            return Err(ManifestError::InvalidBuildTargetPath {
                name: target.name,
                path: target.path.to_string(),
                reason: "path must be relative to the repository root".to_string(),
            });
        }

        let configured_path = target.path.clone();
        let target_directory = root.join(&configured_path);
        if !target_directory.is_dir() {
            return Err(ManifestError::InvalidBuildTargetPath {
                name: target.name,
                path: configured_path.to_string(),
                reason: "directory does not exist".to_string(),
            });
        }

        let canonical_target = canonicalize(&target_directory, &target.name, &configured_path)?;
        let relative_root = canonical_target
            .strip_prefix(&canonical_root)
            .map_err(|_| ManifestError::InvalidBuildTargetPath {
                name: target.name.clone(),
                path: configured_path.to_string(),
                reason: "path resolves outside the repository root".to_string(),
            })?;
        declared.push((target, relative_root.to_path_buf()));
    }

    for (index, (target, target_root)) in declared.iter().enumerate() {
        for (other, other_root) in declared.iter().skip(index + 1) {
            if target_root.starts_with(other_root) || other_root.starts_with(target_root) {
                return Err(ManifestError::OverlappingBuildTargets {
                    first_name: target.name.clone(),
                    first_path: target.path.to_string(),
                    second_name: other.name.clone(),
                    second_path: other.path.to_string(),
                });
            }
        }
    }

    let claimed_roots = declared
        .iter()
        .map(|(_, root)| root.clone())
        .collect::<HashSet<_>>();
    let modules = declared
        .into_iter()
        .map(|(target, root)| Module {
            id: ModuleId::from(target.name),
            root,
            module_type: target.kind,
            depends: target.depends.into_iter().map(ModuleId::from).collect(),
        })
        .collect();

    Ok(BuildCoreTargets {
        modules,
        claimed_roots,
    })
}

fn canonicalize(
    path: &Utf8Path,
    target_name: &str,
    configured_path: &Utf8Path,
) -> Result<Utf8PathBuf> {
    let canonical = std::fs::canonicalize(path).map_err(|source| ManifestError::Io {
        path: path.to_string(),
        source,
    })?;
    Utf8PathBuf::from_path_buf(canonical).map_err(|path| ManifestError::InvalidBuildTargetPath {
        name: target_name.to_string(),
        path: configured_path.to_string(),
        reason: format!("path is not valid UTF-8: {}", path.to_string_lossy()),
    })
}
