use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
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
    let mut file = if create {
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

    // Read from the locked handle itself. A second `File::open(path)` here
    // would trip ERROR_LOCK_VIOLATION (os error 33) on Windows because
    // `LockFileEx` is mandatory — even the same process can't open a
    // second handle into the locked range. Unix's `flock` is advisory and
    // permitted the second open, which is why every pre-v0.8.2 release
    // looked fine on macOS and silently failed `a` saves on Windows: the
    // form ran, `apply_form` got `StorageError::ReadFailed`, set a status
    // bar message that got immediately overwritten by the modal-close
    // redraw, and the user only saw an empty sshc.conf.
    file.seek(SeekFrom::Start(0))
        .map_err(StorageError::ReadFailed)?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(StorageError::ReadFailed)?;

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

#[cfg(windows)]
fn set_owner_only_perms(path: &Path) -> Result<(), StorageError> {
    // Replicate Unix `chmod 0600` semantics on Windows by writing an
    // explicit DACL with three ACEs (current owner / SYSTEM / Local
    // Administrators) and disabling inheritance. Windows OpenSSH
    // rejects `~/.ssh/config.d/sshc.conf` with "Bad owner or
    // permissions on …" the moment a broader trustee (Authenticated
    // Users, BUILTIN\Users, Everyone, …) shows up in the DACL —
    // typically via the parent directory's inherited ACEs — so we
    // build a fresh DACL from scratch and mark it `PROTECTED` to keep
    // future parent-directory ACEs from creeping in.
    //
    // The three ACEs we keep:
    //   * the file's existing owner (read+write, full control)
    //   * NT AUTHORITY\SYSTEM
    //   * BUILTIN\Administrators
    //
    // Anything else is removed. v0.8.2 fixed the *save* path; v0.8.3
    // makes the saved file actually usable by `ssh -G` / `ssh` on
    // Windows. Unix behavior is unchanged.
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS, HLOCAL};
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
        GRANT_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::{
        AllocateAndInitializeSid, FreeSid, ACL, DACL_SECURITY_INFORMATION,
        OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
        PSID, SECURITY_NT_AUTHORITY, SID_IDENTIFIER_AUTHORITY,
    };

    // windows-sys 0.59 doesn't re-export several SDK-stable constants
    // under predictable module paths, so we pin the canonical values
    // locally. These come from <sdkddkver.h> / <winnt.h> and have been
    // stable since Win2k.
    const GENERIC_ALL: u32 = 0x1000_0000;
    const NO_INHERITANCE: u32 = 0;
    const SECURITY_LOCAL_SYSTEM_RID: u32 = 0x12;
    const SECURITY_BUILTIN_DOMAIN_RID: u32 = 0x20;
    const DOMAIN_ALIAS_RID_ADMINS: u32 = 0x220;

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // -- Step 1: read the current owner SID off the file. We don't
    //    change the owner; only the DACL. The returned security
    //    descriptor must be freed with LocalFree.
    let mut owner_sid: PSID = ptr::null_mut();
    let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
    let rc = unsafe {
        GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner_sid,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut sd,
        )
    };
    if rc != ERROR_SUCCESS {
        return Err(StorageError::WriteFailed(
            std::io::Error::from_raw_os_error(rc as i32),
        ));
    }

    // -- Step 2: build the two well-known SIDs (SYSTEM, Local
    //    Administrators). Both come back as caller-owned blocks that
    //    must be released with FreeSid. `SECURITY_NT_AUTHORITY` is
    //    already an `SID_IDENTIFIER_AUTHORITY` value in windows-sys
    //    0.59, so we copy it into a mutable local for the &mut
    //    parameter rather than wrap it once more.
    let nt_authority: SID_IDENTIFIER_AUTHORITY = SECURITY_NT_AUTHORITY;
    let mut system_sid: PSID = ptr::null_mut();
    let mut admins_sid: PSID = ptr::null_mut();

    let ok_system = unsafe {
        AllocateAndInitializeSid(
            &nt_authority,
            1,
            SECURITY_LOCAL_SYSTEM_RID,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut system_sid,
        )
    };
    let ok_admins = unsafe {
        AllocateAndInitializeSid(
            &nt_authority,
            2,
            SECURITY_BUILTIN_DOMAIN_RID,
            DOMAIN_ALIAS_RID_ADMINS,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut admins_sid,
        )
    };
    if ok_system == 0 || ok_admins == 0 {
        let err = std::io::Error::last_os_error();
        unsafe {
            if !system_sid.is_null() {
                FreeSid(system_sid);
            }
            if !admins_sid.is_null() {
                FreeSid(admins_sid);
            }
            LocalFree(sd as HLOCAL);
        }
        return Err(StorageError::WriteFailed(err));
    }

    // -- Step 3: assemble three EXPLICIT_ACCESS_W entries. Each grants
    //    GENERIC_ALL with no inheritance so child files don't pick up
    //    anything from us either.
    let make_ea = |sid: PSID| EXPLICIT_ACCESS_W {
        grfAccessPermissions: GENERIC_ALL,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: sid as *mut u16,
        },
    };
    let entries = [make_ea(owner_sid), make_ea(system_sid), make_ea(admins_sid)];

    let mut new_acl: *mut ACL = ptr::null_mut();
    let rc = unsafe {
        SetEntriesInAclW(
            entries.len() as u32,
            entries.as_ptr(),
            ptr::null_mut(),
            &mut new_acl,
        )
    };
    if rc != ERROR_SUCCESS {
        unsafe {
            FreeSid(system_sid);
            FreeSid(admins_sid);
            LocalFree(sd as HLOCAL);
        }
        return Err(StorageError::WriteFailed(
            std::io::Error::from_raw_os_error(rc as i32),
        ));
    }

    // -- Step 4: install the DACL on the file and mark it PROTECTED so
    //    parent directory inheritance can't add Authenticated Users /
    //    Everyone back in.
    let rc = unsafe {
        SetNamedSecurityInfoW(
            wide.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            new_acl,
            ptr::null_mut(),
        )
    };

    unsafe {
        if !new_acl.is_null() {
            LocalFree(new_acl as HLOCAL);
        }
        FreeSid(system_sid);
        FreeSid(admins_sid);
        LocalFree(sd as HLOCAL);
    }

    if rc != ERROR_SUCCESS {
        return Err(StorageError::WriteFailed(
            std::io::Error::from_raw_os_error(rc as i32),
        ));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn set_owner_only_perms(_path: &Path) -> Result<(), StorageError> {
    // Other targets (wasi, etc.) keep the v0.7-era no-op. sshc isn't
    // supported there but we don't want this file to refuse to compile.
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
