use std::{
    fs,
    path::PathBuf
};
use std::collections::HashMap;
use crate::strings;

pub fn is_file_supported(file_path: &PathBuf) -> Option<bool> {
    if file_path.is_file() && file_path.exists() {
        let extension = file_path.extension()?.to_str()?;

        return Some(strings::SUPPORTED_EXTENSIONS.contains(&extension));
    }

    None
}

// TODO: ordering is.. a bit incorrect
pub fn get_files(image_path: &PathBuf) -> Option<HashMap<usize, PathBuf>> {
    let dir = image_path.parent()?;

    let files: HashMap<usize, PathBuf> = fs::read_dir(dir).ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| is_file_supported(path).unwrap_or_default())
        .enumerate()
        .collect();

    Some(files)
}

pub fn get_file_id(file: &PathBuf, files: &HashMap<usize, PathBuf>) -> Option<usize> {
    for (id, path) in files.iter() {
        if path.eq(file) {
            return Some(*id);
        }
    }

    None
}
