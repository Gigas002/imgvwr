//! Directory-based image list with prev/next navigation.
//!
//! [`Navigator`] scans a directory for supported image files (determined by
//! enabled Cargo features), sorts them by filename, and tracks the current
//! position. Wrap-around is always enabled.

#[cfg(test)]
mod tests;

use std::{
    io,
    path::{Path, PathBuf},
};

/// An ordered list of image paths in a directory with a current-position
/// cursor.
pub struct Navigator {
    /// Sorted list of supported image paths found in the scanned directory.
    pub paths: Vec<PathBuf>,
    /// Index into `paths` for the currently displayed image.
    pub current: usize,
}

impl Navigator {
    /// Build a [`Navigator`] from a file or directory path.
    ///
    /// - If `p` is a **file**: scans its directory (the current one for a bare
    ///   file name) and starts at `p`. `p` is listed even when its extension
    ///   is not one the scan recognises, since the user asked for it.
    /// - If `p` is a **directory**: scans it; cursor starts at index `0`.
    ///
    /// Returns an error if the directory cannot be read or contains no
    /// supported image files.
    pub fn from_path(p: &Path) -> io::Result<Navigator> {
        if p.is_dir() {
            return Self::from_list(scan_dir(p)?, 0);
        }

        // A missing file is an error, not a cue to show its neighbours.
        std::fs::metadata(p).map_err(|e| with_path(p, e))?;
        let dir = parent_dir(p);
        let mut paths = scan_dir(dir)?;
        let name = p
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
        let current = match paths.iter().position(|q| q.file_name() == Some(name)) {
            Some(i) => i,
            None => {
                let file = dir.join(name);
                let i = paths.partition_point(|q| q.file_name() < Some(name));
                paths.insert(i, file);
                i
            }
        };
        Self::from_list(paths, current)
    }

    /// Build a [`Navigator`] from command-line paths.
    ///
    /// A single path behaves like [`Navigator::from_path`]. Several paths are
    /// browsed in the order given, each directory contributing its supported
    /// images, and the cursor starts at the first.
    pub fn from_paths(paths: &[PathBuf]) -> io::Result<Navigator> {
        match paths {
            [] => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no image paths given",
            )),
            [one] => Self::from_path(one),
            many => {
                let mut list = Vec::new();
                for p in many {
                    if p.is_dir() {
                        list.extend(scan_dir(p)?);
                    } else {
                        std::fs::metadata(p).map_err(|e| with_path(p, e))?;
                        list.push(p.clone());
                    }
                }
                Self::from_list(list, 0)
            }
        }
    }

    fn from_list(paths: Vec<PathBuf>, current: usize) -> io::Result<Navigator> {
        if paths.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no supported images found",
            ));
        }
        Ok(Navigator { paths, current })
    }

    /// Return the path of the currently selected image.
    pub fn current(&self) -> &Path {
        &self.paths[self.current]
    }

    /// Advance to the next image, wrapping around to the first after the last.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> &Path {
        self.current = (self.current + 1) % self.paths.len();
        &self.paths[self.current]
    }

    /// Remove the current entry from the list and advance to the next image.
    /// Returns the new current path, or `None` if the list is now empty.
    pub fn remove_current(&mut self) -> Option<&Path> {
        self.paths.remove(self.current);
        if self.paths.is_empty() {
            return None;
        }
        if self.current >= self.paths.len() {
            self.current = self.paths.len() - 1;
        }
        Some(&self.paths[self.current])
    }

    /// Step back to the previous image, wrapping around to the last before the
    /// first.
    pub fn prev(&mut self) -> &Path {
        self.current = self.current.checked_sub(1).unwrap_or(self.paths.len() - 1);
        &self.paths[self.current]
    }
}

/// `e` with `path` in front, so the message says which file it is about.
fn with_path(path: &Path, e: io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

/// Directory holding `p`; `.` for a bare file name.
fn parent_dir(p: &Path) -> &Path {
    match p.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

/// Supported images in `dir`, sorted by file name.
fn scan_dir(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| with_path(dir, e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_supported(p))
        .collect();
    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(paths)
}

fn is_supported(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        #[cfg(feature = "png")]
        Some("png") => true,
        #[cfg(feature = "jpeg")]
        Some("jpg") | Some("jpeg") => true,
        #[cfg(feature = "webp")]
        Some("webp") => true,
        #[cfg(any(feature = "avif", feature = "avif-anim"))]
        Some("avif") => true,
        #[cfg(feature = "jxl")]
        Some("jxl") => true,
        #[cfg(feature = "gif")]
        Some("gif") => true,
        _ => false,
    }
}
