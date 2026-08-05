# AGENTS.md

# CoreForge

CoreForge is the official build system, project manager, bootstrapper, and developer toolkit for the Coreverse ecosystem.

Its responsibility is to configure development environments, resolve dependencies, build projects, package artifacts, and provide a unified developer experience across every Coreverse repository.

---

# Mission

When modifying this repository, always preserve these goals:

* Simplicity
* Predictability
* Reproducibility
* Cross-platform compatibility
* High performance
* Zero unnecessary abstraction

CoreForge is **not** a game engine.

CoreForge is **not** an IDE.

CoreForge is **not** a scripting language.

CoreForge exists to build, configure, and manage projects.

---

# Primary Responsibilities

CoreForge is responsible for:

* Workspace management
* Dependency management
* Toolchain discovery
* Environment validation
* Bootstrap
* Project generation
* Building
* Packaging
* Asset pipeline execution
* Build cache management
* Plugin management
* Documentation generation
* Testing
* Continuous Integration support

---

# Supported Languages

CoreForge directly understands projects written in:

* Rust
* C++
* Go
* SQL

Additional languages may be supported through plugins rather than built-in functionality.

---

# Build Philosophy

Every build must be:

* Deterministic
* Incremental
* Parallel
* Reproducible

Never introduce behavior that depends on machine-specific state unless explicitly configured.

---

# Repository Structure

Maintain a clear separation of responsibilities.

Examples include:

* applications/
* crates/
* scripts/
* templates/
* toolchains/
* docs/
* assets/

Do not mix unrelated functionality across directories.

---

# Bootstrap

Bootstrap prepares a machine for development.

Bootstrap may:

* install required toolchains
* verify compiler versions
* configure environment variables
* validate dependencies
* prepare caches

Bootstrap should never silently modify user projects.

---

# Toolchains

Supported toolchains include:

* Rust
* Cargo
* rustup
* CMake
* Ninja
* Clang
* MSVC
* Go

Do not hardcode platform-specific paths.

Always discover tools automatically whenever possible.

---

# Cross Platform

Every feature should work on:

* Windows
* Linux
* macOS

Avoid introducing platform-specific behavior unless absolutely necessary.

---

# Code Style

Prioritize:

* readability
* correctness
* maintainability
* explicit behavior

Avoid:

* unnecessary macros
* unnecessary allocations
* duplicated logic
* hidden side effects

---

# Dependencies

Before adding a dependency, verify that:

* it is actively maintained
* it has a compatible license
* it provides significant value
* existing dependencies cannot already solve the problem

Prefer fewer dependencies.

---

# Error Handling

Errors should:

* explain what failed
* explain why it failed
* provide actionable fixes

Never panic for recoverable user errors.

---

# Logging

Logging should be:

* structured
* concise
* informative

Avoid excessive logging.

---

# Performance

Performance matters.

Avoid:

* unnecessary filesystem scans
* repeated dependency resolution
* duplicate parsing
* unnecessary allocations

Benchmark significant changes.

---

# Security

Never:

* execute arbitrary commands without validation
* trust user input
* expose secrets
* commit credentials
* disable security checks for convenience

Validate every external input.

---

# Testing

New features should include appropriate tests.

Prefer:

* unit tests
* integration tests
* regression tests

Fix failing tests before merging.

---

# Documentation

Documentation is part of the project.

Update documentation whenever:

* commands change
* configuration changes
* behavior changes
* architecture changes

Examples should remain accurate.

---

# Commits

Keep commits:

* focused
* atomic
* descriptive

Avoid unrelated changes in the same commit.

---

# Pull Requests

Pull requests should:

* solve one problem
* include tests when appropriate
* update documentation when necessary
* pass all CI checks

---

# AI Agent Guidelines

AI agents working on this repository should:

* preserve existing architecture
* avoid unnecessary refactoring
* avoid introducing new dependencies without justification
* follow existing naming conventions
* prefer explicit code over clever code
* keep public APIs stable whenever possible
* avoid breaking backward compatibility unless explicitly requested
* never remove functionality without clear justification

If requirements are ambiguous, prefer the smallest safe change that aligns with the project's architecture.

---

# Final Principle

CoreForge should remain a fast, reliable, deterministic, and professional developer tool that scales from small projects to the complete Coreverse ecosystem.
