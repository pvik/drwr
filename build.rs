//! Embeds the git commit being built, so `drwr -version` can report it.
//!
//! Sets `DRWR_GIT_SHORT_COMMIT` (read with `env!` in `src/cli/mod.rs`) to the
//! short hash of `HEAD`, or `unknown` when building outside a git checkout
//! (e.g. from a source tarball) or without git installed.

use std::process::Command;

fn main() {
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    println!("cargo::rustc-env=DRWR_GIT_SHORT_COMMIT={commit}");

    // Re-run whenever the checked-out commit may have changed. Switching
    // branches (or committing on a detached HEAD) rewrites `HEAD`; committing
    // on a branch rewrites the ref that `HEAD` points at instead.
    //
    // (If that ref is packed, e.g. after `git gc`, there is no loose file to
    // watch and Cargo treats the missing path as always changed, so this
    // script re-runs on every build until the next commit. Redundant, but the
    // hash can never go stale.)
    rerun_if_git_path_changes("HEAD");
    if let Some(branch_ref) = git(&["symbolic-ref", "--quiet", "HEAD"]) {
        rerun_if_git_path_changes(&branch_ref);
    }
}

/// Asks Cargo to re-run this script when `path` inside the git directory
/// changes. `--git-path` is used rather than hardcoding `.git/...` because
/// `.git` is a file, not a directory, in worktrees and submodules.
fn rerun_if_git_path_changes(path: &str) {
    if let Some(path) = git(&["rev-parse", "--git-path", path]) {
        println!("cargo::rerun-if-changed={path}");
    }
}

/// Runs `git` and returns its trimmed stdout, or `None` if git can't be run,
/// exits unsuccessfully, or prints nothing.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let stdout = stdout.trim();
    (!stdout.is_empty()).then(|| stdout.to_owned())
}
