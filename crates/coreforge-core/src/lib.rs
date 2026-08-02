//! `coreforge-core`
//!
//! Diger tum coreforge crate'lerinin bagimli oldugu ortak tipler burada yasar:
//! `Module`, `ModuleId`, `ModuleType` ve paylasilan `Error` enum'u.
//!
//! NOT: Bu crate henuz iskelet asamasinda (Faz 0). `Module` ve `ModuleType`
//! Faz 1 (Project Inspector) ve Faz 2 (Manifest) ile birlikte doldurulacak.

use serde::{Deserialize, Serialize};

/// Bir build modulunun benzersiz kimligi (ornegin "engine-rust", "editor-qt").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleId(pub String);

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Bir modulun hangi toolchain ile derlendigini belirtir.
/// Faz 5 (Tool Adapter) ile genisletilecek.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleType {
    Cargo,
    CMake,
    Npm,
    Tauri,
    Go,
    Sql,
    Python,
}

/// CoreForge genelinde paylasilan hata tipi.
#[derive(Debug, thiserror::Error)]
pub enum CoreForgeError {
    #[error("modul bulunamadi: {0}")]
    ModuleNotFound(String),

    #[error("gecersiz manifest: {0}")]
    InvalidManifest(String),

    #[error("io hatasi: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CoreForgeError>;
