use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process;

use nix::errno::Errno;
#[allow(deprecated)]
use nix::fcntl::{flock, FlockArg};

use crate::error::StorageError;

/// Acquire LOCK_EX on the target path, read content, hand to `mutator`,
/// write the new content atomically (tempfile + rename). Drops the
/// lock when this function returns.
///
/// - If `create` is false and the file does not exist, returns Err(ReadFailed).
/// - If `create` is true and the file does not exist, treats existing content as "".
/// - If another process holds the lock, returns Err(LockHeldByOther).
/// - Sets 0600 permissions on the result.
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

    #[allow(deprecated)]
    flock(file.as_raw_fd(), FlockArg::LockExclusiveNonblock).map_err(|e| match e {
        // On all current Unix targets EAGAIN == EWOULDBLOCK, so matching one is enough.
        Errno::EAGAIN => StorageError::LockHeldByOther,
        other => StorageError::LockFailed(std::io::Error::from_raw_os_error(other as i32)),
    })?;

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

    fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))
        .map_err(StorageError::WriteFailed)?;

    fs::rename(&tmp_path, path).map_err(StorageError::RenameFailed)?;

    // file goes out of scope here, releasing the flock.
    drop(file);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_writer_atomic_roundtrip() {
        let temp = assert_fs::TempDir::new().unwrap();
        let path = temp.child("sshs.conf");
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
