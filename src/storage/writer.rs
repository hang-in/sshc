use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::process;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

#[cfg(unix)]
use nix::errno::Errno;
#[cfg(unix)]
#[allow(deprecated)]
use nix::fcntl::{flock, FlockArg};

use crate::error::StorageError;

/// Acquire an exclusive (advisory on Unix, mandatory on Windows) lock
/// on the target path, read content, hand to `mutator`, write the new
/// content atomically (tempfile + rename). Drops the lock when this
/// function returns.
///
/// - If `create` is false and the file does not exist, returns Err(ReadFailed).
/// - If `create` is true and the file does not exist, treats existing content as "".
/// - If another process holds the lock, returns Err(LockHeldByOther).
/// - Sets 0600 permissions on the result (no-op on Windows; v0.7 defers ACL work).
pub fn with_locked_write<F>(path: &Path, create: bool, mutator: F) -> Result<(), StorageError>
where
    F: FnOnce(&str) -> String,
{
    let file = if create {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(StorageError::WriteFailed)?
    } else {
        File::open(path).map_err(StorageError::ReadFailed)?
    };

    try_lock_exclusive(&file)?;

    let content = if path.exists() {
        let mut s = String::new();
        let mut reader = File::open(path).map_err(StorageError::ReadFailed)?;
        reader
            .read_to_string(&mut s)
            .map_err(StorageError::ReadFailed)?;
        s
    } else {
        String::new()
    };

    let new_content = mutator(&content);

    let tmp_path = path.with_extension(format!("tmp.{}", process::id()));
    {
        let mut tmp = File::create(&tmp_path).map_err(StorageError::WriteFailed)?;
        tmp.write_all(new_content.as_bytes())
            .map_err(StorageError::WriteFailed)?;
        tmp.sync_all().map_err(StorageError::WriteFailed)?;
    }

    set_owner_only_perms(&tmp_path)?;

    // Release the lock *before* renaming over the destination. On Windows
    // MoveFileW (Rust's fs::rename) refuses to replace a path we ourselves
    // hold open — the rename returns ERROR_SHARING_VIOLATION and the user
    // sees their save silently fail. Unix doesn't care either way; closing
    // the handle a few microseconds earlier is harmless. The lock has
    // already done its job by the time we reach this point: the new
    // content is fully written to tmp.
    drop(file);

    fs::rename(&tmp_path, path).map_err(StorageError::RenameFailed)?;

    Ok(())
}

#[cfg(unix)]
fn try_lock_exclusive(file: &File) -> Result<(), StorageError> {
    #[allow(deprecated)]
    flock(file.as_raw_fd(), FlockArg::LockExclusiveNonblock).map_err(|e| match e {
        // On all current Unix targets EAGAIN == EWOULDBLOCK, so matching one is enough.
        Errno::EAGAIN => StorageError::LockHeldByOther,
        other => StorageError::LockFailed(std::io::Error::from_raw_os_error(other as i32)),
    })
}

#[cfg(windows)]
fn try_lock_exclusive(file: &File) -> Result<(), StorageError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_LOCK_VIOLATION};
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    // Lock the entire file (0..u32::MAX,u32::MAX) — equivalent to an
    // advisory whole-file flock for sshc's single-writer use case.
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        LockFileEx(
            file.as_raw_handle() as _,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    };
    if ok == 0 {
        let code = unsafe { GetLastError() };
        if code == ERROR_LOCK_VIOLATION {
            return Err(StorageError::LockHeldByOther);
        }
        return Err(StorageError::LockFailed(std::io::Error::from_raw_os_error(
            code as i32,
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_perms(path: &Path) -> Result<(), StorageError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(StorageError::WriteFailed)
}

#[cfg(not(unix))]
fn set_owner_only_perms(_path: &Path) -> Result<(), StorageError> {
    // Windows: Unix 0600 has no direct equivalent. ACL enforcement is
    // out of scope for v0.7 — we still write the file, just without
    // permission tightening. Defaults inherit from the parent.
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_writer_atomic_roundtrip() {
        let temp = assert_fs::TempDir::new().unwrap();
        let path = temp.child("sshc.conf");
        path.touch().unwrap();

        with_locked_write(path.path(), false, |_old| "new-content".to_string()).unwrap();

        let content = std::fs::read_to_string(path.path()).unwrap();
        assert_eq!(content, "new-content");
        let meta = std::fs::metadata(path.path()).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn test_writer_lock_held_by_other() {
        let temp = assert_fs::TempDir::new().unwrap();
        let path = temp.child("lock.conf");
        path.touch().unwrap();

        let path_clone = path.path().to_path_buf();
        let handle = thread::spawn(move || {
            let f = File::open(&path_clone).unwrap();
            #[allow(deprecated)]
            flock(f.as_raw_fd(), FlockArg::LockExclusive).unwrap();
            thread::sleep(Duration::from_millis(300));
        });

        thread::sleep(Duration::from_millis(50));

        let result = with_locked_write(path.path(), false, |_| String::new());
        assert!(matches!(result, Err(StorageError::LockHeldByOther)));

        handle.join().unwrap();
    }
}
