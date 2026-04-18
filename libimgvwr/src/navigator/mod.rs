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
    /// - If `p` is a **file**: scans its parent directory; sets the cursor to
    ///   `p`'s position in the sorted list.
    /// - If `p` is a **directory**: scans it; cursor starts at index `0`.
    ///
    /// Returns an error if the directory cannot be read or contains no
    /// supported image files.
    pub fn from_path(p: &Path) -> io::Result<Navigator> {
        let (dir, start_file) = if p.is_dir() {
            (p.to_path_buf(), None)
        } else {
            let parent = p.parent().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory")
            })?;
            (parent.to_path_buf(), Some(p.to_path_buf()))
        };

        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| is_supported(p))
            .collect();

        paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        if paths.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no supported images found",
            ));
        }

        let current = start_file
            .and_then(|f| paths.iter().position(|p| p == &f))
            .unwrap_or(0);

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
