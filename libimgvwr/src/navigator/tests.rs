use tempfile::tempdir;

use super::*;

#[cfg(feature = "png")]
fn make_png_files(dir: &std::path::Path, names: &[&str]) {
    for name in names {
        std::fs::File::create(dir.join(name)).unwrap();
    }
}

#[cfg(feature = "png")]
#[test]
fn scan_dir_sorts_by_filename() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["c.png", "a.png", "b.png"]);
    let nav = Navigator::from_path(dir.path()).unwrap();
    assert_eq!(nav.current().file_name().unwrap(), "a.png");
}

#[cfg(feature = "png")]
#[test]
fn scan_file_starts_at_that_file() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "b.png", "c.png"]);
    let nav = Navigator::from_path(&dir.path().join("b.png")).unwrap();
    assert_eq!(nav.current().file_name().unwrap(), "b.png");
}

#[cfg(feature = "png")]
#[test]
fn next_wraps_around() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "b.png"]);
    let mut nav = Navigator::from_path(dir.path()).unwrap();
    nav.next();
    assert_eq!(nav.current().file_name().unwrap(), "b.png");
    nav.next();
    assert_eq!(nav.current().file_name().unwrap(), "a.png");
}

#[cfg(feature = "png")]
#[test]
fn prev_wraps_around() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "b.png"]);
    let mut nav = Navigator::from_path(dir.path()).unwrap();
    nav.prev();
    assert_eq!(nav.current().file_name().unwrap(), "b.png");
    nav.prev();
    assert_eq!(nav.current().file_name().unwrap(), "a.png");
}

#[cfg(feature = "png")]
#[test]
fn remove_current_advances_to_next() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "b.png", "c.png"]);
    let mut nav = Navigator::from_path(dir.path()).unwrap();
    let next = nav.remove_current().unwrap();
    assert_eq!(next.file_name().unwrap(), "b.png");
    assert_eq!(nav.paths.len(), 2);
}

#[cfg(feature = "png")]
#[test]
fn remove_current_at_end_wraps_to_last() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "b.png"]);
    let mut nav = Navigator::from_path(dir.path()).unwrap();
    nav.next();
    let next = nav.remove_current().unwrap();
    assert_eq!(next.file_name().unwrap(), "a.png");
}

#[cfg(feature = "png")]
#[test]
fn remove_current_last_file_returns_none() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png"]);
    let mut nav = Navigator::from_path(dir.path()).unwrap();
    assert!(nav.remove_current().is_none());
}

#[test]
fn empty_dir_returns_error() {
    let dir = tempdir().unwrap();
    let result = Navigator::from_path(dir.path());
    assert!(result.is_err());
}

#[cfg(feature = "png")]
#[test]
fn non_image_files_are_excluded() {
    let dir = tempdir().unwrap();
    std::fs::File::create(dir.path().join("readme.txt")).unwrap();
    std::fs::File::create(dir.path().join("image.png")).unwrap();
    let nav = Navigator::from_path(dir.path()).unwrap();
    assert_eq!(nav.paths.len(), 1);
    assert_eq!(nav.current().file_name().unwrap(), "image.png");
}

#[test]
fn a_bare_file_name_lives_in_the_current_directory() {
    assert_eq!(parent_dir(Path::new("photo.png")), Path::new("."));
    assert_eq!(parent_dir(Path::new("pics/photo.png")), Path::new("pics"));
    assert_eq!(parent_dir(Path::new("/photo.png")), Path::new("/"));
}

#[cfg(feature = "png")]
#[test]
fn an_explicit_file_is_shown_even_with_an_unknown_extension() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "c.png", "b.bmp"]);
    let nav = Navigator::from_path(&dir.path().join("b.bmp")).unwrap();
    assert_eq!(nav.current().file_name().unwrap(), "b.bmp");
    let names: Vec<_> = nav.paths.iter().map(|p| p.file_name().unwrap()).collect();
    assert_eq!(names, ["a.png", "b.bmp", "c.png"]);
}

#[test]
fn an_explicit_file_is_shown_in_a_folder_without_images() {
    let dir = tempdir().unwrap();
    make_files(dir.path(), &["notes.txt", "scan.tiff"]);
    let nav = Navigator::from_path(&dir.path().join("scan.tiff")).unwrap();
    assert_eq!(nav.paths.len(), 1);
    assert_eq!(nav.current().file_name().unwrap(), "scan.tiff");
}

#[cfg(feature = "png")]
#[test]
fn several_paths_are_browsed_in_the_order_given() {
    let dir = tempdir().unwrap();
    make_png_files(dir.path(), &["a.png", "b.png", "c.png", "d.png"]);
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    make_png_files(&sub, &["y.png", "x.png"]);
    let paths = [dir.path().join("c.png"), sub, dir.path().join("a.png")];
    let mut nav = Navigator::from_paths(&paths).unwrap();
    let mut seen = vec![nav.current().file_name().unwrap().to_owned()];
    for _ in 0..4 {
        seen.push(nav.next().file_name().unwrap().to_owned());
    }
    assert_eq!(seen, ["c.png", "x.png", "y.png", "a.png", "c.png"]);
}

#[test]
fn no_paths_is_an_error() {
    assert!(Navigator::from_paths(&[]).is_err());
}

fn make_files(dir: &std::path::Path, names: &[&str]) {
    for name in names {
        std::fs::File::create(dir.join(name)).unwrap();
    }
}
