//! Multi-repository workspace resolution and synchronization.
//!
//! A workspace is declared by `coreforge-workspace.toml`. Local repositories
//! are used directly, while Git repositories are cloned into `.coreforge` and
//! pinned in `coreforge-workspace.lock`. Repository modules are namespaced
//! before they are linked into one build graph, enabling explicit cross-repo
//! dependencies such as `engine::engine`.

mod error;
mod git;
mod schema;

pub use error::{Result, WorkspaceError};
pub use schema::{RepositoryDef, ResolvedRepository, WorkspaceFile, WorkspaceLockFile};

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use coreforge_core::{Module, ModuleId};
use graph::BuildGraph;
use inspector::InspectConfig;
use schema::RepositorySource;

/// The workspace manifest filename.
pub const WORKSPACE_FILE_NAME: &str = "coreforge-workspace.toml";
/// The workspace lock filename.
pub const WORKSPACE_LOCK_FILE_NAME: &str = "coreforge-workspace.lock";

/// The physical location of a namespaced module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleLocation {
    /// Repository name from `coreforge-workspace.toml`.
    pub repository: String,
    /// Canonical root directory of the repository.
    pub repository_root: Utf8PathBuf,
    /// Canonical root directory of the module.
    pub module_root: Utf8PathBuf,
}

/// A workspace after its repositories have been resolved and linked.
pub struct ResolvedWorkspace {
    /// The single graph containing every namespaced workspace module.
    pub graph: BuildGraph,
    locations: HashMap<ModuleId, ModuleLocation>,
}

impl ResolvedWorkspace {
    /// Returns the physical location of a module in the workspace.
    #[must_use]
    pub fn module_location(&self, id: &ModuleId) -> Option<&ModuleLocation> {
        self.locations.get(id)
    }
}

/// Returns whether `root` declares a CoreForge workspace.
#[must_use]
pub fn workspace_manifest_exists(root: &Utf8Path) -> bool {
    root.join(WORKSPACE_FILE_NAME).is_file()
}

/// Reads and validates `coreforge-workspace.toml` from `root`.
pub fn read_workspace_file(root: &Utf8Path) -> Result<WorkspaceFile> {
    let path = root.join(WORKSPACE_FILE_NAME);
    if !path.is_file() {
        return Err(WorkspaceError::WorkspaceFileMissing(path.to_string()));
    }

    let contents = std::fs::read_to_string(&path).map_err(|source| WorkspaceError::Io {
        path: path.to_string(),
        source,
    })?;
    let workspace: WorkspaceFile =
        toml::from_str(&contents).map_err(|source| WorkspaceError::WorkspaceFileParse {
            path: path.to_string(),
            source,
        })?;
    workspace.validate()?;
    Ok(workspace)
}

/// Reads and validates `coreforge-workspace.lock` from `root`.
pub fn read_workspace_lock(root: &Utf8Path) -> Result<WorkspaceLockFile> {
    let path = root.join(WORKSPACE_LOCK_FILE_NAME);
    if !path.is_file() {
        return Err(WorkspaceError::WorkspaceLockMissing(path.to_string()));
    }

    let contents = std::fs::read_to_string(&path).map_err(|source| WorkspaceError::Io {
        path: path.to_string(),
        source,
    })?;
    let lock: WorkspaceLockFile =
        toml::from_str(&contents).map_err(|source| WorkspaceError::WorkspaceLockParse {
            path: path.to_string(),
            source,
        })?;
    lock.validate()?;
    Ok(lock)
}

/// Synchronizes all Git repositories and writes a lock file containing their
/// resolved commits. Local path repositories are validated but are not locked.
pub fn sync(root: &Utf8Path) -> Result<WorkspaceLockFile> {
    let workspace = read_workspace_file(root)?;
    let mut resolved = Vec::new();

    for repository in &workspace.repository {
        match repository.source()? {
            RepositorySource::Path(path) => {
                validate_local_repository(root, repository, &path)?;
            }
            RepositorySource::Git { git, rev } => {
                let checkout = checkout_path(root, &repository.name);
                let commit = git::sync_repository(&checkout, &git, &rev)?;
                resolved.push(ResolvedRepository {
                    name: repository.name.clone(),
                    git,
                    rev,
                    commit,
                });
            }
        }
    }

    resolved.sort_by(|left, right| left.name.cmp(&right.name));
    let lock = WorkspaceLockFile { resolved };
    write_workspace_lock(root, &lock)?;
    Ok(lock)
}

/// Resolves all workspace repositories using the pinned commits in the lock
/// file, namespaces their modules, and builds a single dependency graph.
pub fn resolve(root: &Utf8Path, inspector_config: &InspectConfig) -> Result<ResolvedWorkspace> {
    let workspace = read_workspace_file(root)?;
    let lock = read_workspace_lock(root)?;
    let locked_repositories = lock.index_for(&workspace)?;

    let mut modules = Vec::new();
    let mut locations = HashMap::new();

    for repository in &workspace.repository {
        let repository_root = match repository.source()? {
            RepositorySource::Path(path) => validate_local_repository(root, repository, &path)?,
            RepositorySource::Git { git, rev } => {
                let locked = locked_repositories.get(&repository.name).ok_or_else(|| {
                    WorkspaceError::RepositoryNotLocked {
                        name: repository.name.clone(),
                    }
                })?;
                if locked.git != git || locked.rev != rev {
                    return Err(WorkspaceError::LockMismatch {
                        name: repository.name.clone(),
                    });
                }

                let checkout = checkout_path(root, &repository.name);
                git::verify_pinned_repository(&checkout, &locked.commit)?;
                canonical_directory(&checkout, &repository.name)?
            }
        };

        let repository_modules = resolver::resolve_modules(&repository_root, inspector_config)?;
        modules.extend(namespace_modules(
            repository,
            &repository_root,
            repository_modules,
            &mut locations,
        )?);
    }

    let graph = BuildGraph::from_modules(modules)?;
    graph.build_order()?;
    Ok(ResolvedWorkspace { graph, locations })
}

fn write_workspace_lock(root: &Utf8Path, lock: &WorkspaceLockFile) -> Result<()> {
    let path = root.join(WORKSPACE_LOCK_FILE_NAME);
    let contents = toml::to_string_pretty(lock)?;
    std::fs::write(&path, contents).map_err(|source| WorkspaceError::Io {
        path: path.to_string(),
        source,
    })?;
    Ok(())
}

fn validate_local_repository(
    workspace_root: &Utf8Path,
    repository: &RepositoryDef,
    path: &Utf8Path,
) -> Result<Utf8PathBuf> {
    let resolved_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };
    canonical_directory(&resolved_path, &repository.name)
}

fn canonical_directory(path: &Utf8Path, repository_name: &str) -> Result<Utf8PathBuf> {
    if !path.is_dir() {
        return Err(WorkspaceError::RepositoryPathInvalid {
            name: repository_name.to_string(),
            path: path.to_string(),
        });
    }

    let mut entries = std::fs::read_dir(path).map_err(|source| WorkspaceError::Io {
        path: path.to_string(),
        source,
    })?;
    if entries.next().is_none() {
        return Err(WorkspaceError::RepositoryPathEmpty {
            name: repository_name.to_string(),
            path: path.to_string(),
        });
    }

    let canonical = std::fs::canonicalize(path).map_err(|source| WorkspaceError::Io {
        path: path.to_string(),
        source,
    })?;
    Utf8PathBuf::from_path_buf(canonical)
        .map_err(|path| WorkspaceError::NonUtf8Path(path.to_string_lossy().into_owned()))
}

fn checkout_path(workspace_root: &Utf8Path, repository_name: &str) -> Utf8PathBuf {
    workspace_root
        .join(".coreforge")
        .join("repos")
        .join(repository_name)
}

fn namespace_modules(
    repository: &RepositoryDef,
    repository_root: &Utf8Path,
    modules: Vec<Module>,
    locations: &mut HashMap<ModuleId, ModuleLocation>,
) -> Result<Vec<Module>> {
    let mut namespaced = Vec::with_capacity(modules.len());

    for mut module in modules {
        if module.id.0.is_empty() || module.id.0.contains("::") {
            return Err(WorkspaceError::InvalidModuleId {
                repository: repository.name.clone(),
                module: module.id.0,
            });
        }

        let module_root = repository_root.join(&module.root);
        let namespaced_id = namespaced_module_id(&repository.name, &module.id);
        module.depends = module
            .depends
            .into_iter()
            .map(|dependency| {
                if dependency.0.contains("::") {
                    dependency
                } else {
                    namespaced_module_id(&repository.name, &dependency)
                }
            })
            .collect();
        module.id = namespaced_id.clone();

        locations.insert(
            namespaced_id,
            ModuleLocation {
                repository: repository.name.clone(),
                repository_root: repository_root.to_path_buf(),
                module_root,
            },
        );
        namespaced.push(module);
    }

    Ok(namespaced)
}

fn namespaced_module_id(repository_name: &str, module_id: &ModuleId) -> ModuleId {
    ModuleId::from(format!("{repository_name}::{module_id}"))
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use super::*;
    use coreforge_core::ModuleType;

    fn temp_dir(name: &str) -> Utf8PathBuf {
        let dir = std::env::temp_dir().join(format!("coreforge-workspace-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Utf8PathBuf::from_path_buf(dir).unwrap()
    }

    fn run_git(directory: &Utf8Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn workspace_requires_exactly_one_repository_source() {
        let root = temp_dir("invalid-source");
        fs::write(
            root.join(WORKSPACE_FILE_NAME),
            r#"
                [[repository]]
                name = "engine"
                path = "../engine"
                git = "https://example.invalid/engine.git"
                rev = "main"
            "#,
        )
            .unwrap();

        let result = read_workspace_file(&root);
        assert!(matches!(
            result,
            Err(WorkspaceError::InvalidRepository { name, .. }) if name == "engine"
        ));
    }

    #[test]
    fn resolves_and_namespaces_cross_repository_dependencies() {
        let base = temp_dir("cross-repository-dependency");
        let workspace_root = base.join("workspace");
        let engine_root = base.join("engine");
        let server_root = base.join("server");
        fs::create_dir_all(&workspace_root).unwrap();
        fs::create_dir_all(&engine_root).unwrap();
        fs::create_dir_all(&server_root).unwrap();
        fs::write(engine_root.join("Cargo.toml"), "[workspace]").unwrap();
        fs::write(engine_root.join("coreforge.toml"), "name = \"engine\"").unwrap();
        fs::write(server_root.join("Cargo.toml"), "[workspace]").unwrap();
        fs::write(
            server_root.join("coreforge.toml"),
            "name = \"server\"\ndepends = [\"engine::engine\"]",
        )
            .unwrap();
        fs::write(
            workspace_root.join(WORKSPACE_FILE_NAME),
            r#"
                [[repository]]
                name = "engine"
                path = "../engine"

                [[repository]]
                name = "server"
                path = "../server"
            "#,
        )
            .unwrap();
        fs::write(workspace_root.join(WORKSPACE_LOCK_FILE_NAME), "").unwrap();

        let workspace = resolve(&workspace_root, &InspectConfig::default()).unwrap();
        assert_eq!(workspace.graph.len(), 2);
        assert_eq!(
            workspace.graph.module(&ModuleId::from("server::server")),
            Some(&Module {
                id: ModuleId::from("server::server"),
                root: Utf8PathBuf::new(),
                module_type: ModuleType::Cargo,
                depends: vec![ModuleId::from("engine::engine")],
            })
        );
        assert!(
            workspace
                .module_location(&ModuleId::from("engine::engine"))
                .is_some()
        );
        assert_eq!(
            workspace.graph.build_order().unwrap(),
            vec![
                ModuleId::from("engine::engine"),
                ModuleId::from("server::server")
            ]
        );
    }

    #[test]
    fn resolve_requires_git_repositories_to_be_locked() {
        let root = temp_dir("missing-lock");
        fs::write(
            root.join(WORKSPACE_FILE_NAME),
            r#"
                [[repository]]
                name = "server"
                git = "https://example.invalid/server.git"
                rev = "main"
            "#,
        )
            .unwrap();

        let result = resolve(&root, &InspectConfig::default());
        assert!(matches!(
            result,
            Err(WorkspaceError::WorkspaceLockMissing(_))
        ));
    }

    /// Regression test: a `checkout` directory that exists but isn't a git
    /// repository at all (e.g. left over from an interrupted sync) must
    /// produce the clear `ManagedCheckoutMissing` error, not a raw `git`
    /// command failure.
    #[test]
    fn sync_reports_a_clear_error_for_a_non_git_checkout_directory() {
        let base = temp_dir("sync-non-git-checkout");
        let workspace_root = base.join("workspace");
        let source_root = base.join("source");
        fs::create_dir_all(&workspace_root).unwrap();
        fs::create_dir_all(&source_root).unwrap();
        fs::write(source_root.join("Cargo.toml"), "[workspace]").unwrap();
        run_git(&source_root, &["init", "--quiet"]);
        run_git(
            &source_root,
            &["config", "user.email", "tests@coreforge.dev"],
        );
        run_git(&source_root, &["config", "user.name", "CoreForge Tests"]);
        run_git(&source_root, &["add", "Cargo.toml"]);
        run_git(&source_root, &["commit", "--quiet", "-m", "initial"]);
        fs::write(
            workspace_root.join(WORKSPACE_FILE_NAME),
            format!(
                "[[repository]]\nname = \"engine\"\ngit = '{}'\nrev = \"HEAD\"\n",
                source_root
            ),
        )
            .unwrap();

        // Simulate a leftover, non-git checkout directory (e.g. from an
        // interrupted sync) at the path `sync` would try to reuse.
        let checkout = checkout_path(&workspace_root, "engine");
        fs::create_dir_all(&checkout).unwrap();
        fs::write(checkout.join("stray-file.txt"), "not a git repo").unwrap();

        let result = sync(&workspace_root);
        assert!(matches!(
            result,
            Err(WorkspaceError::ManagedCheckoutMissing(_))
        ));
    }

    #[test]
    fn sync_pins_git_repositories_and_rejects_dirty_checkouts() {
        let base = temp_dir("sync-git-repository");
        let workspace_root = base.join("workspace");
        let source_root = base.join("source");
        fs::create_dir_all(&workspace_root).unwrap();
        fs::create_dir_all(&source_root).unwrap();
        fs::write(source_root.join("Cargo.toml"), "[workspace]").unwrap();
        run_git(&source_root, &["init", "--quiet"]);
        run_git(
            &source_root,
            &["config", "user.email", "tests@coreforge.dev"],
        );
        run_git(&source_root, &["config", "user.name", "CoreForge Tests"]);
        run_git(&source_root, &["add", "Cargo.toml"]);
        run_git(&source_root, &["commit", "--quiet", "-m", "initial"]);
        fs::write(
            workspace_root.join(WORKSPACE_FILE_NAME),
            format!(
                "[[repository]]\nname = \"engine\"\ngit = '{}'\nrev = \"HEAD\"\n",
                source_root
            ),
        )
        .unwrap();

        let lock = sync(&workspace_root).unwrap();
        assert_eq!(lock.resolved.len(), 1);
        assert_eq!(lock.resolved[0].name, "engine");

        let workspace = resolve(&workspace_root, &InspectConfig::default()).unwrap();
        assert_eq!(workspace.graph.len(), 1);

        let checkout = checkout_path(&workspace_root, "engine");
        fs::write(checkout.join("uncommitted.txt"), "dirty").unwrap();
        let result = sync(&workspace_root);
        assert!(matches!(
            result,
            Err(WorkspaceError::ManagedCheckoutDirty(_))
        ));
    }
}
