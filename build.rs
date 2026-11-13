//! Embeds the short commit hash (or `unknown`) for `drwr -version` and the
//! About dialog, and the app icon in Windows builds.

use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

/// The icon Explorer and the taskbar show for drwr.exe.
const WINDOWS_ICON: &str = "assets/app_icon/drwr.ico";

fn main() {
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    println!("cargo::rustc-env=DRWR_GIT_SHORT_COMMIT={commit}");

    // Switching branches rewrites HEAD; committing rewrites the branch's ref.
    // If that ref is packed, this re-runs every build until the next commit.
    rerun_if_git_path_changes("HEAD");
    if let Some(branch_ref) = git(&["symbolic-ref", "--quiet", "HEAD"]) {
        rerun_if_git_path_changes(&branch_ref);
    }

    // -gnu builds go without the icon, as GNU ld needs resources converted
    // by windres first.
    let target = |key| env::var(key).unwrap_or_default();
    if target("CARGO_CFG_TARGET_OS") == "windows" && target("CARGO_CFG_TARGET_ENV") == "msvc" {
        println!("cargo::rerun-if-changed={WINDOWS_ICON}");
        let ico = fs::read(WINDOWS_ICON).expect("couldn't read the Windows icon");
        let res = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("drwr.res");
        fs::write(&res, icon_resources(&ico)).expect("couldn't write drwr.res");
        println!("cargo::rustc-link-arg-bins={}", res.display());
    }
}

/// The resource file rc.exe compiles `1 ICON "drwr.ico"` to, for the icon
/// file `ico`: each image as a resource, then a directory of them. MSVC's
/// linker takes it as it is, so building needs no resource compiler.
fn icon_resources(ico: &[u8]) -> Vec<u8> {
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    const EN_US: u16 = 0x0409;
    assert_eq!(ico[..4], [0, 0, 1, 0], "{WINDOWS_ICON} isn't an icon");
    // Every resource file starts with an empty resource.
    let mut res = resource(0, 0, 0, 0, &[]);
    let mut group = ico[..6].to_vec();
    for id in 1..=u16::from_le_bytes([ico[4], ico[5]]) {
        let entry = &ico[6 + 16 * usize::from(id - 1)..][..16];
        let len = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..].try_into().unwrap()) as usize;
        res.extend(resource(RT_ICON, id, 0x1010, EN_US, &ico[offset..][..len]));
        // The image's entry, with its resource id in place of its offset.
        group.extend(&entry[..12]);
        group.extend(id.to_le_bytes());
    }
    res.extend(resource(RT_GROUP_ICON, 1, 0x1030, EN_US, &group));
    res
}

/// A resource with a numbered type and name. `flags` are the 16-bit Windows
/// memory flags rc.exe still writes.
fn resource(kind: u16, id: u16, flags: u16, language: u16, data: &[u8]) -> Vec<u8> {
    let mut res = Vec::new();
    // The data's length, then the header's.
    res.extend((data.len() as u32).to_le_bytes());
    res.extend(32u32.to_le_bytes());
    // 0xffff marks the type and name as numbers. The zeros are the data
    // version, the version and the characteristics.
    for word in [0xffff, kind, 0xffff, id, 0, 0, flags, language, 0, 0, 0, 0] {
        res.extend(word.to_le_bytes());
    }
    res.extend(data);
    // The next resource starts at a multiple of 4 bytes.
    res.resize(res.len().next_multiple_of(4), 0);
    res
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
