#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace file not found: {0}")]
    WorkspaceFileMissing(String),

    #[error("failed to parse workspace file at {path}: {source}")]
    WorkspaceFileParse {
        path: String,
        #[source]
        source: toml::de::Error,
    },

    #[error("workspace lock file not found: {0}; run 'coreforge workspace sync'")]
    WorkspaceLockMissing(String),

    #[error("failed to parse workspace lock at {path}: {source}")]
    WorkspaceLockParse {
        path: String,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to serialize workspace lock: {0}")]
    WorkspaceLockSerialize(#[from] toml::ser::Error),

    #[error("workspace must declare at least one repository")]
    NoRepositories,

    #[error("invalid workspace repository '{name}': {reason}")]
    InvalidRepository { name: String, reason: String },

    #[error("duplicate workspace repository name: {0}")]
    DuplicateRepository(String),

    #[error("repository '{name}' path is not a directory: {path}")]
    RepositoryPathInvalid { name: String, path: String },

    #[error("repository '{name}' path is empty: {path}")]
    RepositoryPathEmpty { name: String, path: String },

    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(String),

    #[error(
        "repository '{name}' is not present in the workspace lock; run 'coreforge workspace sync'"
    )]
    RepositoryNotLocked { name: String },

    #[error(
        "workspace lock entry does not match repository '{name}'; run 'coreforge workspace sync'"
    )]
    LockMismatch { name: String },

    #[error("workspace lock contains a repository not declared in the manifest")]
    LockContainsUnknownRepository,

    #[error("invalid workspace lock entry for repository '{0}'")]
    InvalidLockEntry(String),

    #[error("duplicate workspace lock entry: {0}")]
    DuplicateLockEntry(String),

    #[error("repository '{repository}' contains an unsupported module id '{module}'")]
    InvalidModuleId { repository: String, module: String },

    #[error(
        "Git is required for workspace synchronization; install it through Bootstrap and ensure it is on PATH"
    )]
    GitUnavailable,

    #[error("Git command failed in {directory}: git {args}: {stderr}")]
    GitCommand {
        directory: String,
        args: String,
        stderr: String,
    },

    #[error("managed repository checkout is missing: {0}; run 'coreforge workspace sync'")]
    ManagedCheckoutMissing(String),

    #[error("managed repository checkout has uncommitted changes: {0}")]
    ManagedCheckoutDirty(String),

    #[error(
        "managed repository checkout at {path} is not pinned to {expected}; run 'coreforge workspace sync'"
    )]
    ManagedCheckoutNotPinned { path: String, expected: String },

    #[error("I/O error accessing {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Resolver(#[from] resolver::ResolverError),

    #[error(transparent)]
    Graph(#[from] graph::GraphError),
}

pub type Result<T> = std::result::Result<T, WorkspaceError>;
