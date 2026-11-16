use clap::Parser;

use super::*;

fn resolve(args: &[&str], config: &Config) -> (AppSettings, Vec<String>) {
    let cli = Cli::parse_from(std::iter::once("imgvwr").chain(args.iter().copied()));
    let mut problems = Vec::new();
    let settings = AppSettings::resolve(&cli, config, &mut problems);
    (settings, problems)
}

#[test]
fn defaults_resolve_without_problems() {
    let (s, problems) = resolve(&[], &Config::default());
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!((s.min_scale, s.max_scale), (0.1, 100.0));
    assert_eq!(s.scale_step, 0.08);
    assert_eq!(s.log_level, "warn");
}

#[test]
fn cli_overrides_config() {
    let (s, problems) = resolve(
        &["--min-scale", "0.5", "--scale-step", "0.2"],
        &Config::default(),
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(s.min_scale, 0.5);
    assert_eq!(s.scale_step, 0.2);
}

#[test]
fn inverted_scale_limits_fall_back_instead_of_panicking() {
    // f32::clamp panics when min > max; this used to crash on the first frame.
    let (s, problems) = resolve(
        &["--min-scale", "5", "--max-scale", "1"],
        &Config::default(),
    );
    assert_eq!((s.min_scale, s.max_scale), (0.1, 100.0));
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("min_scale"), "{problems:?}");
}

#[test]
fn non_positive_or_nan_scales_are_rejected() {
    for bad in ["0", "-2", "nan", "inf"] {
        let (s, problems) = resolve(&[&format!("--min-scale={bad}")], &Config::default());
        assert_eq!(s.min_scale, 0.1, "{bad}");
        assert_eq!(problems.len(), 1, "{bad}: {problems:?}");
    }
}

#[test]
fn scale_step_must_be_a_fraction() {
    for bad in ["0", "1", "1.5", "-0.1", "nan"] {
        let (s, problems) = resolve(&[&format!("--scale-step={bad}")], &Config::default());
        assert_eq!(s.scale_step, 0.08, "{bad}");
        assert_eq!(problems.len(), 1, "{bad}: {problems:?}");
    }
}

#[test]
fn log_levels_are_checked() {
    let (s, problems) = resolve(&["--log-level", "DEBUG"], &Config::default());
    assert!(problems.is_empty());
    assert_eq!(s.log_level, "debug");
    let (s, problems) = resolve(&["--log-level", "verbose"], &Config::default());
    assert_eq!(s.log_level, "warn");
    assert_eq!(problems.len(), 1);
}

#[cfg(feature = "keybinds")]
#[test]
fn unknown_key_names_are_reported_and_fall_back() {
    let config = Config {
        keybindings: Some(Keybindings {
            quit: Some("NotAKey".into()),
            rotate_left: Some("[".into()),
            rotate_right: Some("]".into()),
            delete: Some("Delete".into()),
        }),
        ..Config::default()
    };
    let (s, problems) = resolve(&[], &config);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("keybindings.quit"), "{problems:?}");
    let q = keysym_from_str("q").unwrap();
    assert_eq!(
        s.keybind_map.lookup(q),
        Some(libimgvwr::keybinds::Action::Quit)
    );
}
