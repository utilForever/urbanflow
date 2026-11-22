use std::fs::File;
use std::io;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::ptr::null_mut;
use std::slice;

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    ACL, ACL_SIZE_INFORMATION, AclSizeInformation, DACL_SECURITY_INFORMATION, EqualSid,
    GROUP_SECURITY_INFORMATION, GetAclInformation, GetSecurityDescriptorControl,
    OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    SE_DACL_PROTECTED, SetKernelObjectSecurity, UNPROTECTED_DACL_SECURITY_INFORMATION,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_ENCRYPTED, FILE_READ_ATTRIBUTES, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
};

struct Descriptor {
    raw: PSECURITY_DESCRIPTOR,
    owner: PSID,
    group: PSID,
    dacl: *mut ACL,
}

impl Descriptor {
    fn read(file: &File) -> io::Result<Self> {
        let (mut raw, mut owner, mut group, mut dacl) =
            (null_mut(), null_mut(), null_mut(), null_mut());
        // The descriptor owns the SIDs and ACL returned by GetSecurityInfo.
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                &mut group,
                &mut dacl,
                null_mut(),
                &mut raw,
            )
        };

        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status as i32));
        }

        Ok(Self {
            raw,
            owner,
            group,
            dacl,
        })
    }

    fn entries(&self) -> io::Result<Option<&[u8]>> {
        if self.dacl.is_null() {
            return Ok(None); // A null DACL and an empty DACL have different access rules.
        }

        let mut size = ACL_SIZE_INFORMATION::default();

        if unsafe {
            GetAclInformation(
                self.dacl,
                (&raw mut size).cast(),
                size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }

        // Compare the ordered entries, excluding the ACL header and unused allocation space.
        let bytes = size.AclBytesInUse as usize - size_of::<ACL>();

        Ok(Some(unsafe {
            slice::from_raw_parts(self.dacl.add(1).cast(), bytes)
        }))
    }

    fn protected(&self) -> io::Result<bool> {
        let (mut control, mut revision) = (0, 0);

        if unsafe { GetSecurityDescriptorControl(self.raw, &mut control, &mut revision) } == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(control & SE_DACL_PROTECTED != 0)
    }

    fn changes_from(&self, other: &Self) -> io::Result<u32> {
        let protected = self.protected()?;
        let mut parts = 0;

        if protected != other.protected()? || self.entries()? != other.entries()? {
            parts = DACL_SECURITY_INFORMATION
                | if protected {
                    PROTECTED_DACL_SECURITY_INFORMATION
                } else {
                    UNPROTECTED_DACL_SECURITY_INFORMATION
                };
        }

        let same_sid = |a: PSID, b: PSID| {
            a == b || (!a.is_null() && !b.is_null() && unsafe { EqualSid(a, b) } != 0)
        };

        if !same_sid(self.owner, other.owner) {
            parts |= OWNER_SECURITY_INFORMATION;
        }

        if !same_sid(self.group, other.group) {
            parts |= GROUP_SECURITY_INFORMATION;
        }

        Ok(parts)
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe { LocalFree(self.raw) };
    }
}

pub(super) fn preserve(destination: &Path, staged: &Path, file: &File) -> io::Result<()> {
    let original = File::options()
        .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES)
        .open(destination)?;

    // A DACL copy cannot preserve the EFS recipients or protect plaintext staged bytes.
    if original.metadata()?.file_attributes() & FILE_ATTRIBUTE_ENCRYPTED != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "existing file uses EFS encryption. To save an encrypted result, choose a new name in an EFS-encrypted folder",
        ));
    }

    let original = Descriptor::read(&original)?;
    let parts = original.changes_from(&Descriptor::read(file)?)?;

    if parts == 0 {
        return Ok(());
    }

    let mut rights = 0;

    if parts & DACL_SECURITY_INFORMATION != 0 {
        rights |= WRITE_DAC;
    }

    if parts & (OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION) != 0 {
        rights |= WRITE_OWNER;
    }

    // Only security access is requested, so the staged writer can keep its exclusive data share.
    let editor = File::options().access_mode(rights).open(staged)?;

    if unsafe { SetKernelObjectSecurity(editor.as_raw_handle(), parts, original.raw) } == 0 {
        return Err(io::Error::last_os_error());
    }

    if original.changes_from(&Descriptor::read(file)?)? != 0 {
        return Err(io::Error::other(
            "the staging file did not retain the destination's access rules",
        ));
    }

    Ok(())
}
