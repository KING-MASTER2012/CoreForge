//! Marker-file based module type detection.
//!
//! A directory is considered a module root as soon as it contains one of the
//! marker files below. When a directory matches, the Project Inspector does
//! **not** descend into it any further (see [`crate::walk::inspect_repository`]),
//! so nested marker files (e.g. a `Cargo.toml` inside a Node.js package's
//! `src-tauri/` directory) do not produce spurious, duplicate modules.

use camino::Utf8Path;
use coreforge_core::ModuleType;

/// Inspects a single directory (non-recursively) and returns the [`ModuleType`]
/// it matches, if any.
///
/// Detection order matters: more specific markers are checked first so that,
/// for example, a Tauri application (which has both `package.json` and a
/// `src-tauri/Cargo.toml`) is classified as [`ModuleType::Tauri`] rather than
/// [`ModuleType::Npm`].
#[must_use]
pub fn detect_module_type(dir: &Utf8Path) -> Option<ModuleType> {
    // Tauri: package.json + src-tauri/ (which itself contains a Cargo.toml).
    if dir.join("package.json").is_file() && dir.join("src-tauri").is_dir() {
        return Some(ModuleType::Tauri);
    }

    // Plain npm/Node.js package.
    if dir.join("package.json").is_file() {
        return Some(ModuleType::Npm);
    }

    // Rust crate or workspace.
    if dir.join("Cargo.toml").is_file() {
        return Some(ModuleType::Cargo);
    }

    // CMake project.
    if dir.join("CMakeLists.txt").is_file() {
        return Some(ModuleType::CMake);
    }

    // Go module.
    if dir.join("go.mod").is_file() {
        return Some(ModuleType::Go);
    }

    // Supabase project (its own de facto marker: a directory named
    // "supabase" containing config.toml). This matches when the walker
    // reaches the supabase/ directory itself, not its parent - repos
    // following the Supabase CLI's own convention are recognized without
    // needing an explicit coreforge.toml.
    if dir.file_name() == Some("supabase") && dir.join("config.toml").is_file() {
        return Some(ModuleType::Sql);
    }

    // Python package.
    if dir.join("pyproject.toml").is_file() || dir.join("requirements.txt").is_file() {
        return Some(ModuleType::Python);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> Utf8PathBuf {
        let dir = std::env::temp_dir().join(format!("coreforge-inspector-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Utf8PathBuf::from_path_buf(dir).unwrap()
    }

    use camino::Utf8PathBuf;

    #[test]
    fn detects_cargo_module() {
        let dir = temp_dir("cargo");
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        assert_eq!(detect_module_type(&dir), Some(ModuleType::Cargo));
    }

    #[test]
    fn detects_cmake_module() {
        let dir = temp_dir("cmake");
        fs::write(dir.join("CMakeLists.txt"), "").unwrap();
        assert_eq!(detect_module_type(&dir), Some(ModuleType::CMake));
    }

    #[test]
    fn detects_npm_module() {
        let dir = temp_dir("npm");
        fs::write(dir.join("package.json"), "{}").unwrap();
        assert_eq!(detect_module_type(&dir), Some(ModuleType::Npm));
    }

    #[test]
    fn detects_tauri_over_npm() {
        let dir = temp_dir("tauri");
        fs::write(dir.join("package.json"), "{}").unwrap();
        fs::create_dir_all(dir.join("src-tauri")).unwrap();
        assert_eq!(detect_module_type(&dir), Some(ModuleType::Tauri));
    }

    #[test]
    fn detects_go_module() {
        let dir = temp_dir("go");
        fs::write(dir.join("go.mod"), "module example.com/x").unwrap();
        assert_eq!(detect_module_type(&dir), Some(ModuleType::Go));
    }

    #[test]
    fn detects_python_module() {
        let dir = temp_dir("python");
        fs::write(dir.join("pyproject.toml"), "").unwrap();
        assert_eq!(detect_module_type(&dir), Some(ModuleType::Python));
    }

    #[test]
    fn detects_supabase_project_as_sql() {
        let dir = temp_dir("supabase");
        let supabase_dir = dir.join("supabase");
        fs::create_dir_all(&supabase_dir).unwrap();
        fs::write(supabase_dir.join("config.toml"), "project_id = \"x\"").unwrap();
        // The "supabase" directory itself is the module root, not its parent.
        assert_eq!(detect_module_type(&supabase_dir), Some(ModuleType::Sql));
        assert_eq!(detect_module_type(&dir), None);
    }

    #[test]
    fn returns_none_for_plain_directory() {
        let dir = temp_dir("plain");
        assert_eq!(detect_module_type(&dir), None);
    }
}
