use image::DynamicImage;

use super::*;

fn image(w: u32, h: u32) -> DynamicImage {
    DynamicImage::new_rgba8(w, h)
}

#[test]
fn fit_uses_the_tighter_side() {
    assert_eq!(fit_scale(&image(400, 200), 0, (800, 800), 0.01, 100.0), 2.0);
    assert_eq!(fit_scale(&image(400, 200), 0, (200, 800), 0.01, 100.0), 0.5);
}

#[test]
fn fit_accounts_for_rotation() {
    // A 400×200 landscape image in a 200×400 portrait window: upright it
    // must shrink to half, turned on its side it fits exactly.
    let img = image(400, 200);
    assert_eq!(fit_scale(&img, 0, (200, 400), 0.01, 100.0), 0.5);
    assert_eq!(fit_scale(&img, 180, (200, 400), 0.01, 100.0), 0.5);
    assert_eq!(fit_scale(&img, 90, (200, 400), 0.01, 100.0), 1.0);
    assert_eq!(fit_scale(&img, 270, (200, 400), 0.01, 100.0), 1.0);
}

#[test]
fn fit_is_clamped_to_the_scale_limits() {
    assert_eq!(fit_scale(&image(10, 10), 0, (1000, 1000), 0.1, 5.0), 5.0);
    assert_eq!(fit_scale(&image(10_000, 10), 0, (100, 100), 0.1, 5.0), 0.1);
}

#[test]
fn zoom_and_pan_leave_the_fit_but_rotation_keeps_it() {
    let mut vp = ViewportState::default();
    assert!(on_pointer_motion(3.0, 4.0, &mut vp).unfit);
    assert!(!on_rotate_left(&mut vp).unfit);
    assert!(!on_rotate_right(&mut vp).unfit);
}

#[cfg(feature = "png")]
mod navigation {
    use std::path::Path;

    use super::*;

    /// A folder of PNGs; names starting with `bad` hold a truncated file.
    fn folder(names: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let mut png = Vec::new();
        image(3, 2)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        for name in names {
            let bytes = if name.starts_with("bad") {
                &png[..20]
            } else {
                &png[..]
            };
            std::fs::write(dir.path().join(name), bytes).unwrap();
        }
        dir
    }

    fn name(nav: &Navigator) -> String {
        nav.current()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    fn open(dir: &Path, first: &str) -> (Navigator, ImageHolder, ViewportState) {
        let mut nav = Navigator::from_path(&dir.join(first)).unwrap();
        let mut img = ImageHolder::Static(DynamicImage::new_rgba8(1, 1));
        let mut vp = ViewportState::default();
        assert!(show_loadable(&mut nav, true, true, &mut img, &mut vp));
        (nav, img, vp)
    }

    #[test]
    fn navigation_skips_images_that_fail_to_load() {
        let dir = folder(&["a.png", "bad_b.png", "c.png"]);
        let (mut nav, mut img, mut vp) = open(dir.path(), "a.png");
        assert!(on_navigate(true, &mut nav, &mut img, &mut vp).navigated);
        assert_eq!(name(&nav), "c.png");
        assert!(on_navigate(false, &mut nav, &mut img, &mut vp).navigated);
        assert_eq!(name(&nav), "a.png");
    }

    #[test]
    fn the_cursor_stays_on_screen_when_nothing_else_loads() {
        let dir = folder(&["a.png", "bad_b.png", "bad_c.png"]);
        let (mut nav, mut img, mut vp) = open(dir.path(), "a.png");
        assert!(!on_navigate(true, &mut nav, &mut img, &mut vp).dirty);
        assert_eq!(name(&nav), "a.png");
    }

    #[test]
    fn delete_removes_the_image_on_screen() {
        let dir = folder(&["a.png", "bad_b.png", "c.png"]);
        let (mut nav, mut img, mut vp) = open(dir.path(), "a.png");
        // Right skips the broken b.png; a.png must not be what Delete hits.
        on_navigate(true, &mut nav, &mut img, &mut vp);
        let outcome = on_delete_file(&mut nav, &mut img, &mut vp);
        assert!(!dir.path().join("c.png").exists());
        assert!(dir.path().join("a.png").exists());
        assert!(dir.path().join("bad_b.png").exists());
        // The next loadable image after c.png wraps around to a.png.
        assert!(outcome.navigated);
        assert_eq!(name(&nav), "a.png");
    }

    #[test]
    fn deleting_the_last_loadable_image_quits() {
        let dir = folder(&["a.png", "bad_b.png"]);
        let (mut nav, mut img, mut vp) = open(dir.path(), "a.png");
        assert!(on_delete_file(&mut nav, &mut img, &mut vp).quit);
        assert!(dir.path().join("bad_b.png").exists());
    }

    #[test]
    fn startup_skips_a_broken_first_image() {
        let dir = folder(&["bad_a.png", "b.png"]);
        let (nav, _, _) = open(dir.path(), "bad_a.png");
        assert_eq!(name(&nav), "b.png");
    }
}

#[test]
fn title_names_the_file() {
    assert_eq!(make_title(Path::new("/pics/cat.png")), "cat.png — imgvwr");
}
