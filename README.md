<div style="text-align: center;">
  <img src="assets/images/coreforge-emblem.svg" alt="CoreForge Logo" height="250" />
  <h1>CoreForge</h1>
</div>

> **The official build orchestration system for Coreverse Engine.**

<p style="text-align: center;">
  <strong>Build. Orchestrate. Package.</strong><br>
  A modern, workspace-aware build system designed exclusively for developing the Coreverse Engine.
</p>

---

## 🚀 Overview

CoreForge is the official build orchestration tool powering the development of the **Coreverse Engine**.

Unlike general-purpose build systems, CoreForge is **not intended for building games or arbitrary software projects**. It exists solely to coordinate, build, inspect, and package every component that makes up the engine itself.

Its primary goal is to provide a fast, reproducible, and scalable development workflow across multiple repositories, programming languages, and toolchains while presenting a single, unified command-line interface.

> **Current Release:** `v0.1.0`

---

## ✨ Features

### 🏗 Build Orchestration

* Dependency-aware build scheduling
* Automatic build graph generation
* Parallel build execution
* Workspace-wide builds
* Dry-run support
* Fail-fast mode
* Release and Debug configurations

### 📦 Multi-Repository Workspaces

CoreForge can manage multiple repositories as a single logical workspace. Features include:

* Git-based workspace synchronization
* Local repository support
* Workspace lock file
* Automatic module namespacing
* Cross-repository dependencies

### 🔍 Project Inspection

Without compiling anything, CoreForge can:

* Discover modules
* Detect module types
* Resolve dependencies
* Display the complete dependency graph
* Inspect the current workspace

### 📁 Packaging

After a successful build, CoreForge can automatically:

* Collect produced artifacts
* Generate a distribution directory
* Produce a manifest describing packaged outputs

### ⚙️ Configuration

Configuration is fully TOML-based. Current configuration files:

| File                       | Purpose                     |
|:---------------------------|:----------------------------|
| `coreforge.toml`           | Module configuration        |
| `build-system.toml`        | Build defaults              |
| `coreforge-workspace.toml` | Workspace definition        |
| `coreforge-workspace.lock` | Locked repository revisions |

### 🌍 Cross Platform

CoreForge is designed to work across modern desktop operating systems. Supported targets include:

* 🪟 Windows
* 🐧 Linux
* 🍎 macOS

Platform-specific executable and library naming is handled automatically.

---

## 🛠 Supported Toolchains

Current toolchain integrations include:

* 🦀 Cargo (Rust)
* ⚙️ CMake
* 🥷 Ninja
* 🐹 Go

SQL modules are discovered and participate in dependency resolution but are currently not executed during builds.

---

## 📋 Available Commands

| Command          | Description                                              |
|:-----------------|:---------------------------------------------------------|
| `build`          | Build modules or the entire workspace                    |
| `package`        | Build and collect distributable artifacts                |
| `inspect`        | Inspect modules without building                         |
| `graph`          | Display dependency graph and build order                 |
| `clean`          | Remove generated outputs                                 |
| `workspace sync` | Synchronize workspace repositories                       |
| `test`           | Experimental scheduler (native test runners are planned) |

---

## 🧩 Workspace Support

CoreForge supports monolithic and multi-repository engine development.

A workspace may contain repositories such as:

```text
engine/
editor/
launcher/
server/
website/
```

All modules become part of a unified dependency graph, allowing the entire engine ecosystem to be built as a single project.

---

## 📂 Project Status

CoreForge **v0.1.0** represents the first public milestone.

**Implemented:**

* ✅ CLI interface
* ✅ Dependency graph
* ✅ Parallel scheduler
* ✅ Multi-repository workspaces
* ✅ Artifact packaging
* ✅ Build inspection
* ✅ Cargo integration
* ✅ CMake integration
* ✅ Ninja integration
* ✅ Go integration
* ✅ Cross-platform architecture

**Planned for future releases:**

* 🚧 Native test execution
* 🚧 SQL migration execution
* 🚧 Incremental build cache
* 🚧 Plugin system
* 🚧 Additional toolchain integrations

---

## 🎯 Design Goals

CoreForge is designed around a few core principles:

* ⚡ Fast
* 🔄 Reproducible
* 📦 Workspace-aware
* 🧩 Modular
* 🏗 Engine-focused
* 🖥 Cross-platform
* 🔧 Toolchain-agnostic

---

## ❓ Why CoreForge?

Existing build systems solve general software development.

CoreForge solves **Coreverse Engine development**.

It understands the engine's architecture, repositories, modules, and dependencies, allowing developers to build the entire ecosystem through a consistent and unified workflow.

---

## 📜 License

Licensed under the **GNU General Public License v3.0 (GPL-3.0)**.

See the `LICENSE` file for details.

---

<p style="text-align: center;">
  Made with ❤️ for the Coreverse Engine.
</p>
