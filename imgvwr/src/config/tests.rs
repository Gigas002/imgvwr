use super::*;

#[test]
fn default_has_all_fields_set() {
    let cfg = Config::default();
    let w = cfg.window.unwrap();
    assert_eq!(w.decorations, Some(false));
    assert_eq!(w.antialiasing, Some(false));
    let v = cfg.viewer.unwrap();
    assert!((v.min_scale.unwrap() - 0.1).abs() < f32::EPSILON);
    assert!((v.max_scale.unwrap() - 100.0).abs() < f32::EPSILON);
    assert!((v.scale_step.unwrap() - 0.08).abs() < f32::EPSILON);
    assert_eq!(v.filter_method.unwrap(), FilterMethod::Nearest);
    let k = cfg.keybindings.unwrap();
    assert_eq!(k.quit.unwrap(), "q");
    assert_eq!(k.rotate_left.unwrap(), "[");
    assert_eq!(k.rotate_right.unwrap(), "]");
    assert!(cfg.logging.is_none());
}

#[test]
fn empty_toml_gives_all_none() {
    let cfg: Config = toml::from_str("").unwrap();
    assert!(cfg.window.is_none());
    assert!(cfg.viewer.is_none());
    assert!(cfg.keybindings.is_none());
    assert!(cfg.logging.is_none());
}

#[test]
fn partial_section_leaves_unset_fields_none() {
    let cfg: Config = toml::from_str("[viewer]\nmin_scale = 0.5").unwrap();
    let v = cfg.viewer.unwrap();
    assert!((v.min_scale.unwrap() - 0.5).abs() < f32::EPSILON);
    assert!(v.max_scale.is_none());
    assert!(v.filter_method.is_none());
}

#[test]
fn unknown_keys_are_ignored() {
    let cfg: Config =
        toml::from_str("[window]\ndecorations = true\nunknown = \"ignored\"").unwrap();
    assert_eq!(cfg.window.unwrap().decorations, Some(true));
}

#[test]
fn filter_method_snake_case_variants() {
    let cases = [
        ("nearest", FilterMethod::Nearest),
        ("triangle", FilterMethod::Triangle),
        ("catmull_rom", FilterMethod::CatmullRom),
        ("gaussian", FilterMethod::Gaussian),
        ("lanczos3", FilterMethod::Lanczos3),
    ];
    for (s, expected) in cases {
        let cfg: Config = toml::from_str(&format!("[viewer]\nfilter_method = \"{s}\"")).unwrap();
        assert_eq!(cfg.viewer.unwrap().filter_method.unwrap(), expected);
    }
}

#[test]
fn filter_method_unknown_string_errors() {
    let result: Result<Config, _> = toml::from_str("[viewer]\nfilter_method = \"bicubic\"");
    assert!(result.is_err());
}

#[test]
fn merge_overlay_wins_on_conflict() {
    let base = Config {
        window: Some(Window {
            decorations: Some(false),
            antialiasing: Some(true),
        }),
        viewer: None,
        keybindings: None,
        logging: None,
    };
    let overlay = Config {
        window: Some(Window {
            decorations: Some(true),
            antialiasing: None,
        }),
        viewer: None,
        keybindings: None,
        logging: None,
    };
    let merged = Config::merge(base, overlay);
    let w = merged.window.unwrap();
    assert_eq!(w.decorations, Some(true));
    assert_eq!(w.antialiasing, Some(true));
}

#[test]
fn merge_overlay_none_section_keeps_base() {
    let base = Config::default();
    let overlay = Config {
        window: None,
        viewer: None,
        keybindings: None,
        logging: None,
    };
    let merged = Config::merge(base.clone(), overlay);
    assert!(merged.window.is_some());
    assert!(merged.viewer.is_some());
    assert!(merged.keybindings.is_some());
}

#[test]
fn get_system_path_is_correct() {
    assert_eq!(
        Config::get_system_path(),
        std::path::PathBuf::from("/etc/imgvwr/config.toml")
    );
}

#[test]
fn get_xdg_path_uses_xdg_config_home() {
    unsafe { std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg") };
    let path = Config::get_xdg_path().unwrap();
    assert_eq!(
        path,
        std::path::PathBuf::from("/tmp/xdg/imgvwr/config.toml")
    );
}

#[test]
fn get_home_path_uses_home() {
    unsafe { std::env::set_var("HOME", "/tmp/home") };
    let path = Config::get_home_path().unwrap();
    assert_eq!(
        path,
        std::path::PathBuf::from("/tmp/home/.config/imgvwr/config.toml")
    );
}
