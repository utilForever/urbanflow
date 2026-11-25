use std::fs::{self, File, Permissions};
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt, fchown};
use std::path::Path;

pub(super) fn preserve(destination: &Path, _staged: &Path, file: &File) -> io::Result<()> {
    let original = fs::metadata(destination)?;
    let staged = file.metadata()?;
    let owner = (original.uid() != staged.uid()).then_some(original.uid());
    let group = (original.gid() != staged.gid()).then_some(original.gid());

    if owner.is_some() || group.is_some() {
        fchown(file, owner, group)?;
    }

    file.set_permissions(Permissions::from_mode(original.mode() & 0o777))?;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    copy_acl(destination, file)?;
    #[cfg(target_vendor = "apple")]
    apple_acl::apply(Some(destination), file)?;

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn copy_acl(destination: &Path, file: &File) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(destination.as_os_str().as_bytes())?;
    let name = c"system.posix_acl_access".as_ptr();
    let mut acl = vec![0u8; 65536]; // Linux's maximum size for an extended attribute.
    // Both names are NUL-terminated and the buffer is valid for its full length.
    let len = unsafe { libc::getxattr(path.as_ptr(), name, acl.as_mut_ptr().cast(), acl.len()) };

    if len >= 0 {
        let result = unsafe {
            libc::fsetxattr(file.as_raw_fd(), name, acl.as_ptr().cast(), len as usize, 0)
        };

        return if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        };
    }

    let error = io::Error::last_os_error();

    match error.raw_os_error() {
        Some(libc::EOPNOTSUPP) => return Ok(()),
        Some(libc::ENODATA) => {}
        _ => return Err(error),
    }

    // Remove any access ACL inherited by the staging file.
    if unsafe { libc::fremovexattr(file.as_raw_fd(), name) } != 0 {
        let e = io::Error::last_os_error();

        if e.raw_os_error() != Some(libc::ENODATA) {
            return Err(e);
        }
    }

    Ok(())
}

#[cfg(target_vendor = "apple")]
pub(super) fn private_directory(directory: &Path) -> io::Result<()> {
    // An inherited macOS ACL can grant traversal even on a mode-0700 directory.
    apple_acl::apply(None, &File::open(directory)?)
}

#[cfg(target_vendor = "apple")]
mod apple_acl {
    use super::*;
    use std::ffi::{CString, c_char, c_int, c_void};
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    // Darwin's <sys/acl.h>. These calls read security metadata even on a write-only file.
    unsafe extern "C" {
        fn acl_get_file(path: *const c_char, kind: c_int) -> *mut c_void;
        fn acl_init(count: c_int) -> *mut c_void;
        fn acl_set_fd(fd: c_int, acl: *mut c_void) -> c_int;
        fn acl_free(acl: *mut c_void) -> c_int;
    }

    const ACL_TYPE_EXTENDED: c_int = 0x100;

    pub(super) fn apply(source: Option<&Path>, file: &File) -> io::Result<()> {
        let mut acl = std::ptr::null_mut();

        if let Some(source) = source {
            let path = CString::new(source.as_os_str().as_bytes())?;

            acl = unsafe { acl_get_file(path.as_ptr(), ACL_TYPE_EXTENDED) };

            if acl.is_null() {
                let error = io::Error::last_os_error();

                match error.kind() {
                    io::ErrorKind::NotFound => {}
                    io::ErrorKind::Unsupported => return Ok(()),
                    _ => return Err(error),
                }
            }
        }

        let empty = acl.is_null();

        if empty {
            acl = unsafe { acl_init(0) };

            if acl.is_null() {
                return Err(io::Error::last_os_error());
            }
        }

        // An empty source ACL clears any inherited entries.
        let result = unsafe { acl_set_fd(file.as_raw_fd(), acl) };
        let error = (result != 0).then(io::Error::last_os_error);

        unsafe { acl_free(acl) };

        match error {
            Some(e) if empty && e.kind() == io::ErrorKind::Unsupported => Ok(()),
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
