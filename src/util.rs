use std::{
    fs,
    path::PathBuf
};
use crate::strings;

pub fn is_file_supported(file_path: &PathBuf) -> Option<bool> {
    match file_path {
        _ if file_path.is_file() && file_path.exists() => {
            let extension = file_path.extension()?.to_str()?;

            Some(strings::SUPPORTED_EXTENSIONS.contains(&extension))
        },
        _ => None
    }
}

pub fn get_files(image_path: &PathBuf) -> Option<Vec<PathBuf>> {
    let dir = image_path.parent()?;

    let mut files: Vec<PathBuf> = fs::read_dir(dir).ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| is_file_supported(path).unwrap_or_default())
        .collect();
    files.sort_by_key(|path| path.file_name().unwrap().to_owned());

    Some(files)
}

pub fn get_file_id(file: &PathBuf, files: &Vec<PathBuf>) -> Option<usize> {
    files.iter().position(|path| path.eq(file))
}
