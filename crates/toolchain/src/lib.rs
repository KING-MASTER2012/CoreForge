//! Toolchain adapters for CoreForge build modules.
//!
//! [`ToolchainRunner`] implements the scheduler's [`scheduler::JobRunner`]
//! trait and dispatches each module to a concrete builder. Build outputs are
//! isolated under a caller-provided directory so later artifact collection can
//! copy them without inspecting toolchain-specific default locations.

use std::{collections::HashMap, fs, process::Command, sync::Mutex};

use camino::Utf8PathBuf;
use coreforge_core::{Module, ModuleId, ModuleType};
use scheduler::{JobRunner, JobStatus};

/// An artifact emitted by a successful builder invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Module that produced the artifact.
    pub module: ModuleId,
    /// Artifact representation.
    pub kind: ArtifactKind,
    /// Absolute path to the produced artifact.
    pub path: Utf8PathBuf,
    /// The profile this artifact was actually built with. Lets a consumer
    /// (the Artifact Collector) know which of Cargo's `debug`/`release`
    /// subdirectories under `path` holds the real output, instead of
    /// guessing - `path` itself is shared across profiles and may contain
    /// stale output from an earlier build under a different profile.
    pub profile: BuildProfile,
}

/// The representation of a build artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    /// A directory containing one or more toolchain outputs.
    Directory,
}

/// The configuration applied to a build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    /// Development-oriented build output.
    Debug,
    /// Optimized build output.
    Release,
}

/// Physical paths and configuration needed to build one module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildContext {
    /// Absolute module root directory.
    pub module_dir: Utf8PathBuf,
    /// Managed directory where the builder must place its output.
    pub output_dir: Utf8PathBuf,
    /// Requested build profile.
    pub profile: BuildProfile,
}

/// A concrete adapter for a supported build toolchain.
pub trait Builder: Send + Sync {
    /// Returns whether every required external tool is available on `PATH`.
    fn detect(&self) -> bool;

    /// Prepares the module's managed output directory and build configuration.
    fn configure(&self, module: &Module, context: &BuildContext) -> Result<()>;

    /// Builds a module and returns its managed output directory as an artifact.
    fn build(&self, module: &Module, context: &BuildContext) -> Result<Artifact>;

    /// Removes the module's managed build output.
    fn clean(&self, module: &Module, context: &BuildContext) -> Result<()>;
}

/// Scheduler runner that dispatches modules to Cargo, CMake+Ninja, or Go.
pub struct ToolchainRunner {
    contexts: HashMap<ModuleId, BuildContext>,
    artifacts: Mutex<Vec<Artifact>>,
    availability: Mutex<HashMap<ModuleType, bool>>,
    cargo: CargoBuilder,
    cmake: CmakeBuilder,
    go: GoBuilder,
}

impl ToolchainRunner {
    /// Creates a runner using the supplied module contexts.
    #[must_use]
    pub fn new(contexts: HashMap<ModuleId, BuildContext>) -> Self {
        Self {
            contexts,
            artifacts: Mutex::new(Vec::new()),
            availability: Mutex::new(HashMap::new()),
            cargo: CargoBuilder,
            cmake: CmakeBuilder,
            go: GoBuilder,
        }
    }

    /// Returns artifacts produced by successful jobs so far.
    #[must_use]
    pub fn artifacts(&self) -> Vec<Artifact> {
        self.artifacts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Cleans the managed output for one module.
    pub fn clean(&self, module: &Module) -> Result<()> {
        if module.module_type == ModuleType::Sql {
            return Ok(());
        }
        let context = self.context(module)?;
        let builder = self.builder_for(module.module_type)?;
        if !self.tool_available(module.module_type, builder) {
            return Err(ToolchainError::ToolUnavailable(module.module_type));
        }
        builder.clean(module, context)
    }

    fn context(&self, module: &Module) -> Result<&BuildContext> {
        self.contexts
            .get(&module.id)
            .ok_or_else(|| ToolchainError::MissingBuildContext(module.id.clone()))
    }

    fn builder_for(&self, module_type: ModuleType) -> Result<&dyn Builder> {
        match module_type {
            ModuleType::Cargo => Ok(&self.cargo),
            ModuleType::CMake => Ok(&self.cmake),
            ModuleType::Go => Ok(&self.go),
            unsupported => Err(ToolchainError::UnsupportedModuleType(unsupported)),
        }
    }

    fn tool_available(&self, module_type: ModuleType, builder: &dyn Builder) -> bool {
        let mut availability = self
            .availability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(available) = availability.get(&module_type) {
            return *available;
        }

        let available = builder.detect();
        availability.insert(module_type, available);
        available
    }
}

impl JobRunner for ToolchainRunner {
    fn run(&self, module: &Module) -> JobStatus {
        if module.module_type == ModuleType::Sql {
            return JobStatus::Success;
        }

        let result = (|| {
            let builder = self.builder_for(module.module_type)?;
            let context = self.context(module)?;
            if !self.tool_available(module.module_type, builder) {
                return Err(ToolchainError::ToolUnavailable(module.module_type));
            }
            builder.configure(module, context)?;
            let artifact = builder.build(module, context)?;
            self.artifacts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(artifact);
            Ok(())
        })();

        result.map_or_else(
            |error| JobStatus::Failed(error.to_string()),
            |_| JobStatus::Success,
        )
    }
}

/// Cargo adapter.
#[derive(Debug, Clone, Copy)]
pub struct CargoBuilder;

impl Builder for CargoBuilder {
    fn detect(&self) -> bool {
        command_available("cargo")
    }

    fn configure(&self, _module: &Module, context: &BuildContext) -> Result<()> {
        prepare_output_directory(&context.output_dir)
    }

    fn build(&self, module: &Module, context: &BuildContext) -> Result<Artifact> {
        let mut command = Command::new("cargo");
        command
            .current_dir(&context.module_dir)
            .arg("build")
            .arg("--workspace")
            .env("CARGO_TARGET_DIR", context.output_dir.as_std_path());
        if context.profile == BuildProfile::Release {
            command.arg("--release");
        }
        run_command(&mut command, "cargo build", &context.module_dir)?;
        Ok(directory_artifact(module, context))
    }

    fn clean(&self, _module: &Module, context: &BuildContext) -> Result<()> {
        remove_output_directory(&context.output_dir)
    }
}

/// CMake plus Ninja adapter.
#[derive(Debug, Clone, Copy)]
pub struct CmakeBuilder;

impl Builder for CmakeBuilder {
    fn detect(&self) -> bool {
        command_available("cmake") && command_available("ninja")
    }

    fn configure(&self, _module: &Module, context: &BuildContext) -> Result<()> {
        prepare_output_directory(&context.output_dir)?;
        let build_type = match context.profile {
            BuildProfile::Debug => "Debug",
            BuildProfile::Release => "Release",
        };
        let mut command = Command::new("cmake");
        command
            .current_dir(&context.module_dir)
            .arg("-S")
            .arg(&context.module_dir)
            .arg("-B")
            .arg(&context.output_dir)
            .arg("-G")
            .arg("Ninja")
            .arg(format!("-DCMAKE_BUILD_TYPE={build_type}"));
        run_command(&mut command, "cmake configure", &context.module_dir)
    }

    fn build(&self, module: &Module, context: &BuildContext) -> Result<Artifact> {
        let mut command = Command::new("cmake");
        command
            .current_dir(&context.module_dir)
            .arg("--build")
            .arg(&context.output_dir);
        run_command(&mut command, "cmake build", &context.module_dir)?;
        Ok(directory_artifact(module, context))
    }

    fn clean(&self, _module: &Module, context: &BuildContext) -> Result<()> {
        remove_output_directory(&context.output_dir)
    }
}

/// Go adapter.
#[derive(Debug, Clone, Copy)]
pub struct GoBuilder;

impl Builder for GoBuilder {
    fn detect(&self) -> bool {
        command_available("go")
    }

    fn configure(&self, _module: &Module, context: &BuildContext) -> Result<()> {
        prepare_output_directory(&context.output_dir)
    }

    fn build(&self, module: &Module, context: &BuildContext) -> Result<Artifact> {
        let mut command = Command::new("go");
        command
            .current_dir(&context.module_dir)
            .arg("build")
            .arg("-o")
            .arg(&context.output_dir)
            .arg("./...");
        run_command(&mut command, "go build", &context.module_dir)?;
        Ok(directory_artifact(module, context))
    }

    fn clean(&self, _module: &Module, context: &BuildContext) -> Result<()> {
        remove_output_directory(&context.output_dir)
    }
}

fn directory_artifact(module: &Module, context: &BuildContext) -> Artifact {
    Artifact {
        module: module.id.clone(),
        kind: ArtifactKind::Directory,
        path: context.output_dir.clone(),
        profile: context.profile,
    }
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn prepare_output_directory(path: &Utf8PathBuf) -> Result<()> {
    fs::create_dir_all(path).map_err(|source| ToolchainError::Io {
        path: path.to_string(),
        source,
    })
}

fn remove_output_directory(path: &Utf8PathBuf) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|source| ToolchainError::Io {
            path: path.to_string(),
            source,
        })?;
    }
    Ok(())
}

fn run_command(command: &mut Command, label: &str, directory: &Utf8PathBuf) -> Result<()> {
    let output = command
        .output()
        .map_err(|source| ToolchainError::CommandStart {
            command: label.to_string(),
            directory: directory.to_string(),
            source,
        })?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if stderr.is_empty() { stdout } else { stderr };
    Err(ToolchainError::CommandFailed {
        command: label.to_string(),
        directory: directory.to_string(),
        detail,
    })
}

/// Errors produced by toolchain adapters.
#[derive(Debug, thiserror::Error)]
pub enum ToolchainError {
    /// A module did not receive a resolved physical build context.
    #[error("module '{0}' has no build context")]
    MissingBuildContext(ModuleId),

    /// The module type has no builder in this phase.
    #[error("no builder is implemented for module type {0}")]
    UnsupportedModuleType(ModuleType),

    /// A required external build tool is unavailable.
    #[error("required toolchain for module type {0} is not available on PATH")]
    ToolUnavailable(ModuleType),

    /// A process could not be started.
    #[error("failed to start {command} in {directory}: {source}")]
    CommandStart {
        command: String,
        directory: String,
        #[source]
        source: std::io::Error,
    },

    /// A process completed unsuccessfully.
    #[error("{command} failed in {directory}: {detail}")]
    CommandFailed {
        command: String,
        directory: String,
        detail: String,
    },

    /// A filesystem operation failed.
    #[error("I/O error accessing {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// A convenience alias for toolchain operations.
pub type Result<T> = std::result::Result<T, ToolchainError>;

#[cfg(test)]
mod tests {
    use super::*;

    fn module(id: &str, module_type: ModuleType) -> Module {
        Module {
            id: ModuleId::from(id),
            root: Utf8PathBuf::new(),
            module_type,
            depends: Vec::new(),
        }
    }

    #[test]
    fn sql_module_succeeds_without_a_builder_or_context() {
        let runner = ToolchainRunner::new(HashMap::new());
        assert_eq!(
            runner.run(&module("migrations", ModuleType::Sql)),
            JobStatus::Success
        );
        assert!(runner.artifacts().is_empty());
    }

    #[test]
    fn unsupported_module_type_reports_a_failure() {
        let runner = ToolchainRunner::new(HashMap::new());
        assert!(matches!(
            runner.run(&module("web", ModuleType::Npm)),
            JobStatus::Failed(message) if message.contains("no builder is implemented")
        ));
    }
}
