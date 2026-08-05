use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::error::{Result, WorkspaceError};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceFile {
    #[serde(default)]
    pub repository: Vec<RepositoryDef>,
}

impl WorkspaceFile {
    pub fn validate(&self) -> Result<()> {
        if self.repository.is_empty() {
            return Err(WorkspaceError::NoRepositories);
        }

        let mut names = HashSet::with_capacity(self.repository.len());
        for repository in &self.repository {
            repository.validate_name()?;
            if !names.insert(&repository.name) {
                return Err(WorkspaceError::DuplicateRepository(repository.name.clone()));
            }
            repository.source()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryDef {
    pub name: String,
    pub path: Option<Utf8PathBuf>,
    pub git: Option<String>,
    pub rev: Option<String>,
}

impl RepositoryDef {
    pub(crate) fn source(&self) -> Result<RepositorySource> {
        match (&self.path, &self.git, &self.rev) {
            (Some(path), None, None) if !path.as_str().is_empty() => {
                Ok(RepositorySource::Path(path.clone()))
            }
            (None, Some(git), Some(rev)) if !git.is_empty() && !rev.is_empty() => {
                if git.starts_with('-') || rev.starts_with('-') || git.contains(['\n', '\r']) {
                    return Err(WorkspaceError::InvalidRepository {
                        name: self.name.clone(),
                        reason: "Git URL and revision must be safe command arguments".to_string(),
                    });
                }
                Ok(RepositorySource::Git {
                    git: git.clone(),
                    rev: rev.clone(),
                })
            }
            _ => Err(WorkspaceError::InvalidRepository {
                name: self.name.clone(),
                reason: "specify either 'path' or both 'git' and 'rev'".to_string(),
            }),
        }
    }

    fn validate_name(&self) -> Result<()> {
        let valid = !self.name.is_empty()
            && self.name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            });
        if valid {
            Ok(())
        } else {
            Err(WorkspaceError::InvalidRepository {
                name: self.name.clone(),
                reason: "name may contain only ASCII letters, digits, '-' and '_'".to_string(),
            })
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RepositorySource {
    Path(Utf8PathBuf),
    Git { git: String, rev: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLockFile {
    #[serde(default)]
    pub resolved: Vec<ResolvedRepository>,
}

impl WorkspaceLockFile {
    pub fn validate(&self) -> Result<()> {
        let mut names = HashSet::with_capacity(self.resolved.len());
        for repository in &self.resolved {
            if repository.name.is_empty()
                || repository.git.is_empty()
                || repository.rev.is_empty()
                || repository.commit.is_empty()
            {
                return Err(WorkspaceError::InvalidLockEntry(repository.name.clone()));
            }
            if !names.insert(&repository.name) {
                return Err(WorkspaceError::DuplicateLockEntry(repository.name.clone()));
            }
        }
        Ok(())
    }

    pub(crate) fn index_for<'a>(
        &'a self,
        workspace: &WorkspaceFile,
    ) -> Result<HashMap<String, &'a ResolvedRepository>> {
        self.validate()?;
        let lock_entries = self
            .resolved
            .iter()
            .map(|repository| (repository.name.clone(), repository))
            .collect::<HashMap<_, _>>();

        let git_names = workspace
            .repository
            .iter()
            .filter_map(|repository| match repository.source() {
                Ok(RepositorySource::Git { .. }) => Some(repository.name.clone()),
                Ok(RepositorySource::Path(_)) | Err(_) => None,
            })
            .collect::<HashSet<_>>();

        if lock_entries.keys().any(|name| !git_names.contains(name)) {
            return Err(WorkspaceError::LockContainsUnknownRepository);
        }
        Ok(lock_entries)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedRepository {
    pub name: String,
    pub git: String,
    pub rev: String,
    pub commit: String,
}
