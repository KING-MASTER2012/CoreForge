//! Reading and parsing a single `coreforge.toml` file.

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::ManifestError;
use crate::schema::{MANIFEST_FILE_NAME, ManifestFile};

/// Returns the path to `dir`'s `coreforge.toml`, if it exists.
#[must_use]
pub fn find_manifest_path(dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let path = dir.join(MANIFEST_FILE_NAME);
    path.is_file().then_some(path)
}

/// Reads and parses the manifest at `path`.
///
/// # Errors
///
/// Returns [`ManifestError::Io`] if the file cannot be read, or
/// [`ManifestError::Parse`] if its contents are not valid `coreforge.toml`.
pub fn read_manifest(path: &Utf8Path) -> Result<ManifestFile, ManifestError> {
    let contents = std::fs::read_to_string(path).map_err(|source| ManifestError::Io {
        path: path.to_string(),
        source,
    })?;

    toml::from_str(&contents).map_err(|source| ManifestError::Parse {
        path: path.to_string(),
        source,
    })
}
