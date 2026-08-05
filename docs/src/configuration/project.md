# Project Targets

`BUILD.core` is an optional TOML file at a repository root. It declares
modules whose toolchain, name, path, and dependencies must be explicit.

```toml
[[target]]
kind = "go"
name = "coreverse-server"
path = "./cmd/server"
depends = ["engine::engine"]

[[target]]
kind = "sql"
name = "migrations"
path = "./supabase"
```

Each `path` must exist, be relative to the repository root, and must not
overlap another target path. Declared paths are reserved: CoreForge does not
run native-marker or `coreforge.toml` discovery inside them. Target kinds use
lowercase `ModuleType` names such as `cargo`, `cmake`, `go`, and `sql`.
