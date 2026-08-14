use std::process::Command;

use camino::Utf8Path;

use crate::error::{Result, WorkspaceError};

pub(crate) fn sync_repository(checkout: &Utf8Path, git_url: &str, rev: &str) -> Result<String> {
    if checkout.exists() {
        ensure_git_checkout(checkout)?;
        ensure_clean(checkout)?;
        let origin = run_git(checkout, &["remote", "get-url", "origin"])?;
        if origin.trim() != git_url {
            return Err(WorkspaceError::GitCommand {
                directory: checkout.to_string(),
                args: "remote get-url origin".to_string(),
                stderr: format!("origin is '{}', expected '{git_url}'", origin.trim()),
            });
        }
    } else {
        let parent = checkout.parent().expect("checkout path has a parent");
        std::fs::create_dir_all(parent).map_err(|source| WorkspaceError::Io {
            path: parent.to_string(),
            source,
        })?;
        let checkout_arg = checkout.as_str();
        run_git(
            parent,
            &["clone", "--no-checkout", "--", git_url, checkout_arg],
        )?;
    }

    run_git(checkout, &["fetch", "--force", "--tags", "origin", rev])?;
    let commit = run_git(checkout, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"])?;
    let commit = commit.trim().to_string();
    run_git(checkout, &["checkout", "--detach", &commit])?;
    Ok(commit)
}

pub(crate) fn verify_pinned_repository(checkout: &Utf8Path, expected_commit: &str) -> Result<()> {
    if !checkout.is_dir() {
        return Err(WorkspaceError::ManagedCheckoutMissing(checkout.to_string()));
    }
    ensure_git_checkout(checkout)?;
    ensure_clean(checkout)?;
    let head = run_git(checkout, &["rev-parse", "HEAD"])?;
    if head.trim() == expected_commit {
        Ok(())
    } else {
        Err(WorkspaceError::ManagedCheckoutNotPinned {
            path: checkout.to_string(),
            expected: expected_commit.to_string(),
        })
    }
}

fn ensure_git_checkout(checkout: &Utf8Path) -> Result<()> {
    // Check for `.git` directly rather than only relying on `git
    // rev-parse`'s exit code: when `checkout` exists but isn't a git
    // repository at all (e.g. a leftover directory from an interrupted
    // sync), `rev-parse --is-inside-work-tree` fails the whole `git`
    // process rather than returning "false", so the `?` below would
    // propagate a raw, confusing `WorkspaceError::GitCommand` instead of
    // this function's intended `ManagedCheckoutMissing`. Checking first
    // avoids ever spawning `git` for that case.
    if !checkout.join(".git").exists() {
        return Err(WorkspaceError::ManagedCheckoutMissing(checkout.to_string()));
    }

    let inside_work_tree = run_git(checkout, &["rev-parse", "--is-inside-work-tree"])?;
    if inside_work_tree.trim() == "true" {
        Ok(())
    } else {
        Err(WorkspaceError::ManagedCheckoutMissing(checkout.to_string()))
    }
}

fn ensure_clean(checkout: &Utf8Path) -> Result<()> {
    let status = run_git(checkout, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        Ok(())
    } else {
        Err(WorkspaceError::ManagedCheckoutDirty(checkout.to_string()))
    }
}

fn run_git(directory: &Utf8Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(args)
        .output()
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                WorkspaceError::GitUnavailable
            } else {
                WorkspaceError::Io {
                    path: directory.to_string(),
                    source,
                }
            }
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(WorkspaceError::GitCommand {
            directory: directory.to_string(),
            args: args.join(" "),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}
