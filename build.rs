//! Embeds the short commit hash (or `unknown`) for `drwr -version` and the
//! About dialog.

use std::process::Command;

fn main() {
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    println!("cargo::rustc-env=DRWR_GIT_SHORT_COMMIT={commit}");

    // Switching branches rewrites HEAD; committing rewrites the branch's ref.
    // If that ref is packed, this re-runs every build until the next commit.
    rerun_if_git_path_changes("HEAD");
    if let Some(branch_ref) = git(&["symbolic-ref", "--quiet", "HEAD"]) {
        rerun_if_git_path_changes(&branch_ref);
    }
}

/// Uses `--git-path` because `.git` is a file in worktrees and submodules.
fn rerun_if_git_path_changes(path: &str) {
    if let Some(path) = git(&["rev-parse", "--git-path", path]) {
        println!("cargo::rerun-if-changed={path}");
    }
}

/// Returns git's trimmed stdout, or `None` if it fails or prints nothing.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let stdout = stdout.trim();
    (!stdout.is_empty()).then(|| stdout.to_owned())
}
