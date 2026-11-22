//! Resolves output destinations and stages replacements on the same filesystem.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::files::same_file;

#[cfg(unix)]
#[path = "output/unix.rs"]
mod access;
#[cfg(windows)]
#[path = "output/windows.rs"]
mod access;

// Live output links have already been resolved. A link here must not be replaced.
fn destination_exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(entry) if entry.is_file() => Ok(true),
        Ok(entry) if entry.is_symlink() => {
            Err(io::Error::other("output path is now a symbolic link"))
        }
        Ok(_) => Err(io::Error::other("output must be a regular file")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// A complete replacement staged on the destination filesystem until it is installed or dropped.
pub(crate) struct StagedOutput {
    file: File,
    paths: StagingPaths,
}

struct StagingPaths {
    name_probe: Option<NameProbe>,
    new_file: Option<PathBuf>,
}

fn temporary_path(destination: &Path, extension: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);

    destination.with_file_name(format!(
        ".urbanflow-{}-{}.{extension}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ))
}

fn staging_error(path: &Path, error: io::Error) -> String {
    format!("{}: staging: {error}", path.display())
}

/// A requested filename inside an otherwise empty directory on the destination filesystem.
struct NameProbe(PathBuf);

impl NameProbe {
    fn new(destination: &Path, other: Option<&Path>) -> Result<Self, String> {
        loop {
            let directory = temporary_path(destination, "tmp");
            let builder = &mut fs::DirBuilder::new();

            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(builder, 0o700);

            match builder.create(&directory) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(staging_error(destination, e)),
            }

            let probe = Self(directory.join(destination.file_name().unwrap()));

            if !same_file(destination, &directory)?
                && !matches!(other, Some(path) if same_file(path, &directory)?)
            {
                return Ok(probe);
            }
        }
    }

    fn contains(&self, path: &Path) -> Result<bool, String> {
        self.0
            .with_file_name(path.file_name().unwrap())
            .try_exists()
            .map_err(|e| staging_error(path, e))
    }
}

impl Drop for NameProbe {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}

impl StagingPaths {
    fn data_path(&self) -> &Path {
        self.new_file
            .as_deref()
            .unwrap_or_else(|| &self.name_probe.as_ref().unwrap().0)
    }

    /// Returns None if a temporary name is already taken or would alias an output.
    fn create_file(
        &mut self,
        destination: &Path,
        other_output: Option<&Path>,
        existing: bool,
    ) -> Result<Option<File>, String> {
        let error = |e| staging_error(destination, e);

        let mut options = File::options();
        options.write(true).create_new(true);

        #[cfg(windows)]
        std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, 0);

        if existing {
            let probe = &self.name_probe.as_ref().unwrap().0;

            #[cfg(target_vendor = "apple")]
            access::private_directory(probe.parent().unwrap()).map_err(error)?;

            return options.open(probe).map(Some).map_err(error);
        }

        // A direct sibling inherits even ACL entries limited to immediate children.
        let extension = if destination
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("new"))
        {
            "tmp"
        } else {
            "new"
        };
        let sibling = temporary_path(destination, extension);

        if let Some(probe) = &self.name_probe {
            let marker = probe.0.with_file_name(sibling.file_name().unwrap());

            drop(options.open(&marker).map_err(error)?);

            for output in std::iter::once(destination).chain(other_output) {
                if probe.contains(output)? {
                    return Ok(None);
                }
            }

            fs::rename(marker, &probe.0).map_err(error)?;
        }

        let file = match options.open(&sibling) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(None),
            Err(e) => return Err(error(e)),
        };

        self.new_file = Some(sibling);
        Ok(Some(file))
    }
}

impl StagedOutput {
    pub(crate) fn new(
        destination: &Path,
        contents: &str,
        other_output: Option<&Path>,
    ) -> Result<Self, String> {
        let error = |e| staging_error(destination, e);
        let same_directory_output = match other_output {
            Some(path) if same_file(destination.parent().unwrap(), path.parent().unwrap())? => {
                Some(path)
            }
            _ => None,
        };

        loop {
            let existing = destination_exists(destination).map_err(error)?;
            let mut paths = StagingPaths {
                name_probe: (existing || same_directory_output.is_some())
                    .then(|| NameProbe::new(destination, other_output))
                    .transpose()?,
                new_file: None,
            };
            let Some(file) = paths.create_file(destination, same_directory_output, existing)?
            else {
                continue;
            };
            let mut staged = Self { file, paths };

            if let Some(other) = same_directory_output
                && staged.has_filename(destination, other)?
            {
                return Err("input, result, and HTML paths must be distinct".to_owned());
            }

            if existing {
                staged.preserve_access(destination)?;
            } else if destination_exists(destination).map_err(error)? {
                // It appeared before writing. Restage with its existing access rules.
                continue;
            }

            staged
                .file
                .write_all(contents.as_bytes())
                .and_then(|()| staged.file.sync_all())
                .map_err(error)?;

            return Ok(staged);
        }
    }

    /// Asks the destination filesystem whether this spelling names the staged file.
    pub(crate) fn has_filename(
        &mut self,
        destination: &Path,
        other: &Path,
    ) -> Result<bool, String> {
        if destination.file_name() == other.file_name() {
            return Ok(true);
        }

        if let Some(probe) = &self.paths.name_probe {
            return probe.contains(other);
        }

        let probe = NameProbe::new(destination, Some(other))?;

        File::create_new(&probe.0).map_err(|e| staging_error(destination, e))?;

        self.paths.name_probe.insert(probe).contains(other)
    }

    fn preserve_access(&self, destination: &Path) -> Result<(), String> {
        #[cfg(any(unix, windows))]
        access::preserve(destination, self.paths.data_path(), &self.file)
            .map_err(|e| format!("{}: access permissions: {e}", destination.display()))?;
        Ok(())
    }

    pub(crate) fn replace(
        mut self,
        destination: &Path,
        before_rename: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        // A destination can appear or have its permissions tightened during staging.
        if destination_exists(destination).map_err(|e| format!("{}: {e}", destination.display()))?
            || self.paths.new_file.is_none()
        {
            self.preserve_access(destination)?;
        }

        before_rename(&mut self)?;
        drop(self.file);

        fs::rename(self.paths.data_path(), destination)
            .map_err(|e| format!("{}: replacement: {e}", destination.display()))?;

        self.paths.new_file = None;
        Ok(())
    }
}

impl Drop for StagingPaths {
    fn drop(&mut self) {
        if let Some(path) = &self.new_file {
            let _ = fs::remove_file(path);
        }
    }
}
