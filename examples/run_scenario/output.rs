//! Resolves output destinations and stages replacements on the same filesystem.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::files::same_file;

#[cfg(unix)]
#[path = "output/unix.rs"]
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
    probe: NameProbe,
    replacing: bool,
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
            let probe = NameProbe::new(destination, other_output)?;
            let mut options = File::options();
            options.write(true).create_new(true);
            #[cfg(windows)]
            std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, 0);
            #[cfg(target_vendor = "apple")]
            if existing {
                access::private_directory(probe.0.parent().unwrap()).map_err(error)?;
            }
            let file = options.open(&probe.0).map_err(error)?;
            let mut staged = Self {
                file,
                probe,
                replacing: existing,
            };

            if let Some(other) = same_directory_output
                && staged.probe.contains(other)?
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

    fn preserve_access(&self, destination: &Path) -> Result<(), String> {
        #[cfg(not(unix))]
        fs::metadata(destination).map_err(|e| format!("{}: {e}", destination.display()))?;
        #[cfg(unix)]
        access::preserve(destination, &self.probe.0, &self.file)
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
            || self.replacing
        {
            self.preserve_access(destination)?;
        }

        before_rename(&mut self)?;
        drop(self.file);

        fs::rename(&self.probe.0, destination)
            .map_err(|e| format!("{}: replacement: {e}", destination.display()))?;

        Ok(())
    }
}
