use super::*;

#[test]
fn zoom_clamps_to_min() {
    let mut vp = ViewportState::default();
    vp.zoom_by(-10.0, 0.1, 100.0);
    assert!((vp.scale - 0.1).abs() < f32::EPSILON);
}

#[test]
fn zoom_clamps_to_max() {
    let mut vp = ViewportState::default();
    vp.zoom_by(200.0, 0.1, 100.0);
    assert!((vp.scale - 100.0).abs() < f32::EPSILON);
}

#[test]
fn zoom_applies_delta() {
    let mut vp = ViewportState::default();
    // First step: 1.0 * 1.5 = 1.5
    vp.zoom_by(0.5, 0.1, 100.0);
    assert!((vp.scale - 1.5).abs() < f32::EPSILON);
    // Second step from non-unit base: 1.5 * 1.5 = 2.25 (not 1.5 + 0.5 = 2.0).
    // This would fail with additive zoom, proving the multiplicative property.
    vp.zoom_by(0.5, 0.1, 100.0);
    assert!((vp.scale - 2.25).abs() < f32::EPSILON);
}

#[test]
fn rotate_right_cycles() {
    let mut vp = ViewportState::default();
    vp.rotate_right();
    assert_eq!(vp.rotation, 90);
    vp.rotate_right();
    assert_eq!(vp.rotation, 180);
    vp.rotate_right();
    assert_eq!(vp.rotation, 270);
    vp.rotate_right();
    assert_eq!(vp.rotation, 0);
}

#[test]
fn rotate_left_cycles() {
    let mut vp = ViewportState::default();
    vp.rotate_left();
    assert_eq!(vp.rotation, 270);
    vp.rotate_left();
    assert_eq!(vp.rotation, 180);
    vp.rotate_left();
    assert_eq!(vp.rotation, 90);
    vp.rotate_left();
    assert_eq!(vp.rotation, 0);
}

#[test]
fn pan_is_additive() {
    let mut vp = ViewportState::default();
    vp.pan(10.0, 5.0);
    vp.pan(3.0, -2.0);
    assert!((vp.offset.0 - 13.0).abs() < f32::EPSILON);
    assert!((vp.offset.1 - 3.0).abs() < f32::EPSILON);
}

#[test]
fn reset_restores_defaults() {
    let mut vp = ViewportState::default();
    vp.zoom_by(5.0, 0.1, 100.0);
    vp.pan(20.0, 30.0);
    vp.rotate_right();
    vp.reset();
    assert!((vp.scale - 1.0).abs() < f32::EPSILON);
    assert_eq!(vp.offset, (0.0, 0.0));
    assert_eq!(vp.rotation, 0);
}
