# Toolchains

`coreforge build` dispatches modules to a builder based on their type.
CoreForge currently provides builders for `cargo`, `cmake` (with Ninja), and
`go`. Missing tools fail with an actionable `PATH` error.

Each build writes to `.coreforge/build/<module-id>/` under the selected
repository or workspace root. This keeps generated outputs separate from the
source repository and prepares a stable input for artifact collection.

SQL modules are intentionally no-ops during normal builds and cleans. Use the
`coreforge db` command family for Supabase operations when it is available.

`test` and `package` retain scheduler dry-run behavior until dedicated adapter
operations are added; they do not invoke a build tool yet.
