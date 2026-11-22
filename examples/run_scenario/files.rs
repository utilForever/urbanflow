//! File and directory identities used by both the CLI and desktop when exporting a run.

use std::fs::{self, File};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use same_file::Handle;

use super::scenario;

pub(crate) struct ScenarioFile {
    pub(crate) path: PathBuf,
    resolved_path: PathBuf,
    handle: Handle,
}

fn absolute_file_path(path: &Path) -> io::Result<PathBuf> {
    // Path components can discard a trailing separator or `/.` before lookup.
    let ending = path
        .as_os_str()
        .as_encoded_bytes()
        .rsplit(|&byte| std::path::is_separator(char::from(byte)))
        .next()
        .unwrap_or_default();

    if matches!(ending, b"" | b"." | b"..") || path.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must end in a filename",
        ));
    }

    std::path::absolute(path)
}

impl ScenarioFile {
    pub(crate) fn open(path: &Path) -> Result<(Self, scenario::RailScenario), String> {
        let io_error = |e| format!("{}: I/O: {e}", path.display());
        let chosen = absolute_file_path(path).map_err(io_error)?;
        let resolved_path = fs::canonicalize(&chosen).map_err(io_error)?;
        let reader = File::open(&resolved_path).map_err(io_error)?;
        let json = io::read_to_string(&reader).map_err(io_error)?;
        #[cfg(windows)]
        let reader = pin_for_identity(reader).map_err(io_error)?;
        let handle = Handle::from_file(reader).map_err(io_error)?;
        let rail = scenario::parse(path, &json)
            .and_then(|input| input.into_rail(path))
            .map_err(|e| e.to_string())?;

        Ok((
            Self {
                path: chosen,
                resolved_path,
                handle,
            },
            rail,
        ))
    }

    fn matches_file(&self, existing: &FileId) -> Result<bool, String> {
        #[cfg(unix)]
        let matches_opened = (self.handle.dev(), self.handle.ino()) == *existing;
        #[cfg(not(unix))]
        let matches_opened = self.handle == *existing;

        if matches_opened {
            return Ok(true);
        }

        for name in [&self.path, &self.resolved_path] {
            if file_id(name)?.as_ref() == Some(existing) {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

fn distinct(path: &Path) -> String {
    format!(
        "{}: output must be distinct from the scenario",
        path.display()
    )
}

/// Follows a live output link and only treats a genuinely absent entry as a new file.
pub(crate) fn output_path(path: &Path, inputs: &[&ScenarioFile]) -> Result<PathBuf, String> {
    inspect_output(path, inputs).map(|(resolved, _)| resolved)
}

fn inspect_output(
    path: &Path,
    inputs: &[&ScenarioFile],
) -> Result<(PathBuf, Option<FileId>), String> {
    let io_error = |e| format!("{}: {e}", path.display());
    let chosen = absolute_file_path(path).map_err(io_error)?;

    if inputs
        .iter()
        .any(|input| input.path == chosen || input.resolved_path == chosen)
    {
        return Err(distinct(path));
    }

    let resolved = match fs::symlink_metadata(&chosen) {
        Ok(entry) => {
            let resolved = fs::canonicalize(&chosen).map_err(|e| {
                if entry.is_symlink() {
                    format!(
                        "{}: cannot resolve output symbolic link: {e}",
                        path.display()
                    )
                } else {
                    io_error(e)
                }
            })?;

            if !fs::metadata(&resolved).map_err(io_error)?.is_file() {
                return Err(format!("{}: output must be a regular file", path.display()));
            }

            resolved
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let name = chosen
                .file_name()
                .ok_or_else(|| format!("{}: expected an output filename", path.display()))?;

            fs::canonicalize(chosen.parent().unwrap())
                .map_err(io_error)?
                .join(name)
        }
        Err(e) => return Err(io_error(e)),
    };

    let existing = file_id(&resolved)?;

    for input in inputs {
        if input.resolved_path == resolved
            || matches!(&existing, Some(id) if input.matches_file(id)?)
        {
            return Err(distinct(path));
        }
    }

    Ok((resolved, existing))
}

pub(crate) fn same_file(left: &Path, right: &Path) -> Result<bool, String> {
    if left == right {
        return Ok(true);
    }

    let Some(left_id) = file_id(left)? else {
        return Ok(false);
    };

    Ok(file_id(right)?.as_ref() == Some(&left_id))
}

#[cfg(windows)]
fn pin_for_identity(reader: File) -> io::Result<File> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, ReOpenFile,
    };

    // Reopen the same object, even across a rename, without keeping a read-data share.
    let handle = unsafe {
        ReOpenFile(
            reader.as_raw_handle(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            0,
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    // ReOpenFile transferred ownership of a new handle to us.
    Ok(unsafe { File::from_raw_handle(handle) })
}

#[cfg(unix)]
type FileId = (u64, u64);
#[cfg(not(unix))]
type FileId = Handle;

fn file_id(path: &Path) -> Result<Option<FileId>, String> {
    #[cfg(unix)]
    let id = fs::metadata(path).map(|file| (file.dev(), file.ino()));
    // Zero desired access lets Windows query identity without requesting file data.
    #[cfg(windows)]
    let id = File::options()
        .access_mode(0)
        .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .and_then(Handle::from_file);
    #[cfg(not(any(unix, windows)))]
    let id = Handle::from_path(path);

    match id {
        Ok(id) => Ok(Some(id)),
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        #[cfg(unix)]
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => Ok(None),
        #[cfg(windows)]
        Err(e)
            if e.raw_os_error()
                == Some(windows_sys::Win32::Foundation::ERROR_CANT_RESOLVE_FILENAME as i32) =>
        {
            Ok(None)
        }
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}
