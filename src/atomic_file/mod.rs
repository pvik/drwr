use std::ffi::OsString;
use std::fs::{self, File, OpenOptions, Permissions};
use std::io;
use std::path::{Path, PathBuf};
use std::process;

/// Replaces the file at `path` with what `contents` writes, by way of a
/// temporary file renamed over it, so that a failed write leaves it whole.
pub fn write(path: &Path, contents: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    // Renaming over a symlink would replace it, not the file it points to.
    let path = match fs::canonicalize(path) {
        Ok(path) => path,
        Err(err) if err.kind() == io::ErrorKind::NotFound => std::path::absolute(path)?,
        Err(err) => return Err(err),
    };
    // Renaming over a read-only file works, so refuse it as writing to it did.
    let permissions = match OpenOptions::new().write(true).open(&path) {
        Ok(file) => Some(file.metadata()?.permissions()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };
    let (temp_path, temp) = create_temp(&path)?;
    let result = fill(temp, permissions, contents).and_then(|()| fs::rename(&temp_path, &path));
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result?;
    // Makes the rename survive a crash. Windows can't open folders as files.
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        File::open(dir)?.sync_all()?;
    }
    Ok(())
}

/// Creates a hidden file beside `path` that no other write is using.
fn create_temp(path: &Path) -> io::Result<(PathBuf, File)> {
    let mut attempt = 0;
    loop {
        let temp_path = temp_path(path, attempt);
        match File::create_new(&temp_path) {
            // Perhaps left by a crash.
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => attempt += 1,
            temp => return temp.map(|temp| (temp_path, temp)),
        }
    }
}

/// The temporary file that writing `path` tries at `attempt`.
fn temp_path(path: &Path, attempt: u32) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(format!(".{}-{attempt}.tmp", process::id()));
    path.with_file_name(name)
}

/// Writes `contents` to `file`, giving it `permissions` first, and waits for
/// them to reach the disk.
fn fill(
    mut file: File,
    permissions: Option<Permissions>,
    contents: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)?;
    }
    contents(&mut file)?;
    file.sync_all()
}

#[cfg(test)]
mod tests;
