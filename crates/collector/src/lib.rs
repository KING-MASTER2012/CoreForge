//! `collector`
//!
//! Artifact Collector (Phase 6).
//!
//! Consumes the [`toolchain::Artifact`]s produced by a build and copies each
//! module's real build output (a single binary or library, not the whole
//! managed build directory) into one central `dist/` tree, alongside a
//! `dist-manifest.json` recording where every module's output ended up.
//!
//! CoreForge does not yet track a module's binary/crate name separately
//! from its [`coreforge_core::ModuleId`] (that would be a Phase 2 manifest
//! extension - `artifact_name` in `coreforge.toml` - for a later phase), so
//! this crate locates the "real" output file inside a module's managed
//! build directory with a best-effort convention search rather than an
//! authoritative lookup. A module whose output cannot be recognized is
//! skipped rather than failing the whole collection; see
//! [`DistManifest::entries`] vs. the artifact count passed to [`collect`]
//! to detect that.

mod error;

pub use error::{CollectorError, Result};

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use coreforge_core::ModuleId;
use serde::{Deserialize, Serialize};
use toolchain::{Artifact, ArtifactKind, BuildProfile};

/// The filename the Artifact Collector writes at the root of `dist/`.
pub const DIST_MANIFEST_FILE_NAME: &str = "dist-manifest.json";

/// One collected artifact's location within `dist/` and its checksum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistEntry {
    /// The module that produced this artifact.
    pub module: ModuleId,
    /// Path to the collected file, relative to `dist/`.
    pub path: Utf8PathBuf,
    /// BLAKE3 checksum of the collected file, hex-encoded.
    pub checksum: String,
}

/// A record of every artifact collected into `dist/`, written as
/// `dist-manifest.json`. Consumed by anything that needs to know which file
/// a module produced without re-deriving the naming convention (e.g. the
/// Launcher deciding which executable to run - that lookup is the
/// Launcher's responsibility, CoreForge only produces the manifest).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistManifest {
    /// Every successfully collected artifact, sorted by module id.
    pub entries: Vec<DistEntry>,
}

impl DistManifest {
    /// Returns the collected entry for a module, if any.
    #[must_use]
    pub fn entry(&self, module: &ModuleId) -> Option<&DistEntry> {
        self.entries.iter().find(|entry| &entry.module == module)
    }
}

/// Copies each artifact's recognizable output file into `dist_root` and
/// returns a manifest of what was collected. Artifacts whose managed build
/// directory doesn't contain a file matching a known naming convention are
/// silently skipped - the caller can compare `manifest.entries.len()`
/// against `artifacts.len()` to report that to the user.
///
/// # Errors
///
/// Returns [`CollectorError::Io`] if `dist_root` (or a module's
/// subdirectory within it) cannot be created, or if a recognized output
/// file cannot be read or copied.
pub fn collect(artifacts: &[Artifact], dist_root: &Utf8Path) -> Result<DistManifest> {
    let mut entries = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        if let Some(entry) = collect_one(artifact, dist_root)? {
            entries.push(entry);
        }
    }
    entries.sort_by(|left, right| left.module.0.cmp(&right.module.0));
    Ok(DistManifest { entries })
}

/// Writes `manifest` as `dist_root/dist-manifest.json`, creating `dist_root`
/// if it doesn't already exist.
///
/// # Errors
///
/// Returns [`CollectorError::Io`] if `dist_root` cannot be created or the
/// manifest cannot be written, or [`CollectorError::Serialize`] if the
/// manifest cannot be serialized (this should not happen for a well-formed
/// [`DistManifest`]).
pub fn write_manifest(manifest: &DistManifest, dist_root: &Utf8Path) -> Result<()> {
    create_dir(dist_root)?;
    let path = dist_root.join(DIST_MANIFEST_FILE_NAME);
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(&path, json).map_err(|source| CollectorError::Io {
        path: path.to_string(),
        source,
    })
}

/// Removes `dist_root` entirely, including every collected artifact and the
/// manifest. Does nothing if `dist_root` does not exist.
///
/// # Errors
///
/// Returns [`CollectorError::Io`] if `dist_root` exists but could not be
/// removed.
pub fn clean(dist_root: &Utf8Path) -> Result<()> {
    if dist_root.exists() {
        fs::remove_dir_all(dist_root).map_err(|source| CollectorError::Io {
            path: dist_root.to_string(),
            source,
        })?;
    }
    Ok(())
}

fn collect_one(artifact: &Artifact, dist_root: &Utf8Path) -> Result<Option<DistEntry>> {
    let ArtifactKind::Directory = artifact.kind;
    let Some(source_file) = locate_output_file(&artifact.module, &artifact.path, artifact.profile)
    else {
        return Ok(None);
    };

    let module_dir = dist_root.join(artifact.module.sanitized());
    create_dir(&module_dir)?;

    let file_name = source_file.file_name().unwrap_or("artifact");
    let dest = module_dir.join(file_name);
    fs::copy(&source_file, &dest).map_err(|source| CollectorError::Io {
        path: dest.to_string(),
        source,
    })?;

    let bytes = fs::read(&dest).map_err(|source| CollectorError::Io {
        path: dest.to_string(),
        source,
    })?;
    let checksum = blake3::hash(&bytes).to_hex().to_string();
    let relative = dest
        .strip_prefix(dist_root)
        .map(Utf8Path::to_path_buf)
        .unwrap_or(dest);

    Ok(Some(DistEntry {
        module: artifact.module.clone(),
        path: relative,
        checksum,
    }))
}

/// Finds the single file inside `output_dir` that represents a module's
/// real build output, trying (in order):
///
/// 1. `{profile}/{candidate}` - Cargo's per-profile subdirectory *for the
///    profile the module was actually built with*. `output_dir` is shared
///    across profiles (a prior build under the other profile may have left
///    its own `debug/`/`release/` subdirectory behind), so only the
///    requested profile's subdirectory is trusted here - falling back to
///    the other profile would silently collect a stale, wrong-profile
///    binary.
/// 2. `{candidate}` directly under `output_dir` - CMake and Go place their
///    output there, with no profile subdirectory at all.
/// 3. If exactly one file exists directly under `output_dir`, use it - a
///    module with a single, unambiguous output regardless of its name.
///
/// Returns `None` if nothing recognizable is found, so the caller can skip
/// this module rather than copying an entire build directory.
fn locate_output_file(
    module: &ModuleId,
    output_dir: &Utf8Path,
    profile: BuildProfile,
) -> Option<Utf8PathBuf> {
    let candidates = candidate_names(module);

    let profile_dir = match profile {
        BuildProfile::Debug => "debug",
        BuildProfile::Release => "release",
    };
    if let Some(found) = find_named(&output_dir.join(profile_dir), &candidates) {
        return Some(found);
    }

    if let Some(found) = find_named(output_dir, &candidates) {
        return Some(found);
    }

    single_file_in(output_dir)
}

fn candidate_names(module: &ModuleId) -> Vec<String> {
    let base = &module.0;
    let sanitized = module.sanitized();
    let mut names = Vec::new();
    for name in [base, &sanitized] {
        names.push(format!("{name}{}", platform::executable_suffix()));
        names.push(format!(
            "{}{name}{}",
            platform::library_prefix(),
            platform::dynamic_library_suffix()
        ));
        names.push(format!("{}{name}.rlib", platform::library_prefix()));
        names.push(format!(
            "{}{name}{}",
            platform::library_prefix(),
            platform::static_library_suffix()
        ));
    }
    names.dedup();
    names
}

fn find_named(dir: &Utf8Path, candidates: &[String]) -> Option<Utf8PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

fn single_file_in(dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut files = fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| Utf8PathBuf::from_path_buf(entry.path()).ok());

    let first = files.next()?;
    if files.next().is_some() {
        return None;
    }
    Some(first)
}

fn create_dir(path: &Utf8Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|source| CollectorError::Io {
        path: path.to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn artifact(module: &str, output_dir: &Utf8Path) -> Artifact {
        artifact_with_profile(module, output_dir, BuildProfile::Debug)
    }

    fn artifact_with_profile(module: &str, output_dir: &Utf8Path, profile: BuildProfile) -> Artifact {
        Artifact {
            module: ModuleId::from(module),
            kind: ArtifactKind::Directory,
            path: output_dir.to_path_buf(),
            profile,
        }
    }

    #[test]
    fn collects_a_cargo_style_release_binary() {
        let dir = tempdir();
        let output_dir = dir.join("engine");
        fs::create_dir_all(output_dir.join("release")).unwrap();
        fs::write(output_dir.join("release").join("engine"), b"pretend binary").unwrap();

        let dist_root = dir.join("dist");
        let manifest = collect(
            &[artifact_with_profile(
                "engine",
                &output_dir,
                BuildProfile::Release,
            )],
            &dist_root,
        )
            .unwrap();

        assert_eq!(manifest.entries.len(), 1);
        let entry = &manifest.entries[0];
        assert_eq!(entry.module, ModuleId::from("engine"));
        assert!(dist_root.join(&entry.path).is_file());
        assert!(!entry.checksum.is_empty());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn collects_a_cargo_style_debug_binary() {
        let dir = tempdir();
        let output_dir = dir.join("engine");
        fs::create_dir_all(output_dir.join("debug")).unwrap();
        fs::write(output_dir.join("debug").join("engine"), b"pretend binary").unwrap();

        let dist_root = dir.join("dist");
        let manifest = collect(
            &[artifact_with_profile(
                "engine",
                &output_dir,
                BuildProfile::Debug,
            )],
            &dist_root,
        )
            .unwrap();

        assert_eq!(manifest.entries.len(), 1);
        assert!(dist_root.join(&manifest.entries[0].path).is_file());

        fs::remove_dir_all(&dir).unwrap();
    }

    /// Regression test for the bug where a Debug rebuild of a module that
    /// had previously been built in Release mode would silently collect
    /// the stale Release binary instead of the fresh Debug one, because
    /// `output_dir` is shared across profiles and the old lookup always
    /// checked `release/` before `debug/` regardless of what was actually
    /// requested.
    #[test]
    fn does_not_fall_back_to_a_stale_binary_from_a_different_profile() {
        let dir = tempdir();
        let output_dir = dir.join("engine");
        fs::create_dir_all(output_dir.join("release")).unwrap();
        fs::write(
            output_dir.join("release").join("engine"),
            b"stale release binary",
        )
            .unwrap();
        fs::create_dir_all(output_dir.join("debug")).unwrap();
        fs::write(
            output_dir.join("debug").join("engine"),
            b"fresh debug binary",
        )
            .unwrap();

        let dist_root = dir.join("dist");
        let manifest = collect(
            &[artifact_with_profile(
                "engine",
                &output_dir,
                BuildProfile::Debug,
            )],
            &dist_root,
        )
            .unwrap();

        assert_eq!(manifest.entries.len(), 1);
        let collected_path = dist_root.join(&manifest.entries[0].path);
        assert_eq!(fs::read(collected_path).unwrap(), b"fresh debug binary");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn falls_back_to_the_single_file_in_the_output_directory() {
        let dir = tempdir();
        let output_dir = dir.join("coreverse-server");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("some-unexpected-name"), b"binary").unwrap();

        let dist_root = dir.join("dist");
        let manifest = collect(&[artifact("coreverse-server", &output_dir)], &dist_root).unwrap();

        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(
            manifest.entries[0].path.file_name(),
            Some("some-unexpected-name")
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn skips_modules_with_no_recognizable_output() {
        let dir = tempdir();
        let output_dir = dir.join("web");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("a"), b"1").unwrap();
        fs::write(output_dir.join("b"), b"2").unwrap();

        let dist_root = dir.join("dist");
        let manifest = collect(&[artifact("web", &output_dir)], &dist_root).unwrap();

        assert!(manifest.entries.is_empty());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn workspace_namespaced_ids_get_a_sanitized_dist_subdirectory() {
        let dir = tempdir();
        let output_dir = dir.join("out");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("engine__engine"), b"1").unwrap();

        let dist_root = dir.join("dist");
        let manifest = collect(&[artifact("engine::engine", &output_dir)], &dist_root).unwrap();

        assert_eq!(manifest.entries.len(), 1);
        assert!(
            manifest.entries[0]
                .path
                .as_str()
                .starts_with("engine__engine"),
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn clean_removes_the_whole_dist_directory() {
        let dir = tempdir();
        let dist_root = dir.join("dist");
        fs::create_dir_all(&dist_root).unwrap();
        fs::write(dist_root.join(DIST_MANIFEST_FILE_NAME), b"{}").unwrap();

        clean(&dist_root).unwrap();

        assert!(!dist_root.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    fn tempdir() -> Utf8PathBuf {
        let path = Utf8PathBuf::from_path_buf(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "coreforge-collector-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
