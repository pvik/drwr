use std::io::Write as _;

use super::*;
use crate::testing::ScratchDir;

fn write_text(path: &Path, text: &str) -> io::Result<()> {
    write(path, |file| file.write_all(text.as_bytes()))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn replaces_the_file_leaving_nothing_else() {
    let dir = ScratchDir::new("atomic-replace");
    let path = dir.path("plan.graphml");
    fs::write(&path, "the old plan, longer than the new").unwrap();
    write_text(&path, "new").unwrap();
    assert_eq!(read(&path), "new");
    assert_eq!(dir.names(), ["plan.graphml"]);
}

#[test]
fn creates_a_missing_file() {
    let dir = ScratchDir::new("atomic-create");
    let path = dir.path("plan.graphml");
    write_text(&path, "new").unwrap();
    assert_eq!(read(&path), "new");
    assert_eq!(dir.names(), ["plan.graphml"]);
}

#[test]
fn a_failed_write_leaves_the_old_file_whole() {
    let dir = ScratchDir::new("atomic-failed");
    let path = dir.path("plan.graphml");
    fs::write(&path, "old").unwrap();
    let err = write(&path, |file| {
        file.write_all(b"half")?;
        Err(io::Error::other("disk full"))
    })
    .unwrap_err();
    assert_eq!(err.to_string(), "disk full");
    assert_eq!(read(&path), "old");
    assert_eq!(dir.names(), ["plan.graphml"]);
}

#[test]
fn refuses_a_read_only_file() {
    let dir = ScratchDir::new("atomic-read-only");
    let path = dir.path("plan.graphml");
    fs::write(&path, "old").unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    // As root, which may write to it anyway.
    if OpenOptions::new().write(true).open(&path).is_ok() {
        return;
    }
    let err = write_text(&path, "new").unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(read(&path), "old");
    assert_eq!(dir.names(), ["plan.graphml"]);
}

#[cfg(unix)]
#[test]
fn keeps_the_files_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = ScratchDir::new("atomic-mode");
    let path = dir.path("plan.graphml");
    fs::write(&path, "old").unwrap();
    // With execute bits, which no new file gets, whatever the umask.
    fs::set_permissions(&path, Permissions::from_mode(0o751)).unwrap();
    write_text(&path, "new").unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o751);
}

#[cfg(unix)]
#[test]
fn writes_through_a_symlink() {
    let dir = ScratchDir::new("atomic-symlink");
    let path = dir.path("plan.graphml");
    let link = dir.path("link.graphml");
    fs::write(&path, "old").unwrap();
    std::os::unix::fs::symlink(&path, &link).unwrap();
    write_text(&link, "new").unwrap();
    assert!(link.is_symlink());
    assert_eq!(read(&path), "new");
    assert_eq!(dir.names(), ["link.graphml", "plan.graphml"]);
}

#[test]
fn keeps_a_temporary_file_left_by_a_crash() {
    let dir = ScratchDir::new("atomic-leftover");
    let path = dir.path("plan.graphml");
    let leftover = temp_path(&path, 0);
    fs::write(&leftover, "unsaved").unwrap();
    write_text(&path, "new").unwrap();
    assert_eq!(read(&path), "new");
    assert_eq!(read(&leftover), "unsaved");
    assert_eq!(dir.names().len(), 2);
}
