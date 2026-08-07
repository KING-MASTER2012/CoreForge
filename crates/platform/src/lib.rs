//! `platform`
//!
//! Operating-system detection and the filename conventions that follow from
//! it (executable suffix, dynamic/static library prefix and suffix).
//!
//! This crate intentionally has no dependency on any other `coreforge-*`
//! crate - it sits at the bottom of the dependency graph, next to
//! `coreforge-core`. Anything that needs to turn a module id or a build
//! output into a platform-correct filename (the Toolchain adapters, the
//! Artifact Collector) depends on this instead of hardcoding `.exe` checks
//! inline.

/// The operating system family CoreForge is currently running on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    /// Microsoft Windows.
    Windows,
    /// Linux and Linux-derived systems.
    Linux,
    /// macOS.
    MacOs,
    /// Any other target `rustc` supports. Treated like a Unix-like system
    /// for filename conventions, since that is correct for every target
    /// tier 1/2 platform other than Windows.
    Other,
}

/// Returns the OS family CoreForge is currently running on.
#[must_use]
pub const fn current_os() -> Os {
    if cfg!(target_os = "windows") {
        Os::Windows
    } else if cfg!(target_os = "linux") {
        Os::Linux
    } else if cfg!(target_os = "macos") {
        Os::MacOs
    } else {
        Os::Other
    }
}

/// The file extension (including the leading `.`) native executables carry
/// on the current OS. Empty on Unix-like systems.
///
/// ```
/// let name = format!("coreverse-server{}", platform::executable_suffix());
/// ```
#[must_use]
pub const fn executable_suffix() -> &'static str {
    match current_os() {
        Os::Windows => ".exe",
        Os::Linux | Os::MacOs | Os::Other => "",
    }
}

/// The filename prefix native dynamic and static libraries carry on the
/// current OS (`lib` on Unix-like systems, none on Windows).
#[must_use]
pub const fn library_prefix() -> &'static str {
    match current_os() {
        Os::Windows => "",
        Os::Linux | Os::MacOs | Os::Other => "lib",
    }
}

/// The file extension (including the leading `.`) native dynamic libraries
/// carry on the current OS.
#[must_use]
pub const fn dynamic_library_suffix() -> &'static str {
    match current_os() {
        Os::Windows => ".dll",
        Os::MacOs => ".dylib",
        Os::Linux | Os::Other => ".so",
    }
}

/// The file extension (including the leading `.`) native static libraries
/// carry on the current OS.
#[must_use]
pub const fn static_library_suffix() -> &'static str {
    match current_os() {
        Os::Windows => ".lib",
        Os::Linux | Os::MacOs | Os::Other => ".a",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_one_os_is_reported() {
        // Sanity check: whichever OS we're compiled for, the helpers agree
        // with each other and never panic.
        let os = current_os();
        let _ = executable_suffix();
        let _ = library_prefix();
        let _ = dynamic_library_suffix();
        let _ = static_library_suffix();
        match os {
            Os::Windows => assert_eq!(executable_suffix(), ".exe"),
            Os::Linux | Os::MacOs | Os::Other => assert_eq!(executable_suffix(), ""),
        }
    }
}
