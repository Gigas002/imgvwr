use super::ScrollFrame;

#[test]
fn a_wheel_notch_is_one_step() {
    // wlroots and Hyprland send value 15 and value120 120 per notch.
    let down = ScrollFrame {
        value: 15.0,
        value120: Some(120),
    };
    assert_eq!(down.notches(), Some(1.0));
    let up_two = ScrollFrame {
        value: -30.0,
        value120: Some(-240),
    };
    assert_eq!(up_two.notches(), Some(-2.0));
}

#[test]
fn value120_wins_over_a_scaled_axis_value() {
    // A compositor with a scroll factor scales `value` but not value120.
    let frame = ScrollFrame {
        value: 45.0,
        value120: Some(120),
    };
    assert_eq!(frame.notches(), Some(1.0));
}

#[test]
fn high_resolution_wheels_scroll_fractions_of_a_notch() {
    let frame = ScrollFrame {
        value: 3.75,
        value120: Some(30),
    };
    assert_eq!(frame.notches(), Some(0.25));
}

#[test]
fn touchpads_use_fifteen_units_per_notch() {
    let frame = ScrollFrame {
        value: 7.5,
        value120: None,
    };
    assert_eq!(frame.notches(), Some(0.5));
}

#[test]
fn frames_without_vertical_scroll_produce_nothing() {
    assert_eq!(ScrollFrame::default().notches(), None);
}
