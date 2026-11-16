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
