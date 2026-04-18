use super::*;
use clap::Parser;

#[test]
fn no_args_gives_empty_paths_and_no_config() {
    let cli = Cli::parse_from(["imgvwr"]);
    assert!(cli.paths.is_empty());
    assert!(cli.config.is_none());
}

#[test]
fn config_flag_is_parsed() {
    let cli = Cli::parse_from(["imgvwr", "--config", "foo.toml"]);
    assert_eq!(cli.config.unwrap(), std::path::PathBuf::from("foo.toml"));
}

#[test]
fn positional_paths_are_collected() {
    let cli = Cli::parse_from(["imgvwr", "a.png", "b.jpg"]);
    assert_eq!(
        cli.paths,
        vec![
            std::path::PathBuf::from("a.png"),
            std::path::PathBuf::from("b.jpg"),
        ]
    );
}

#[test]
fn paths_and_config_together() {
    let cli = Cli::parse_from(["imgvwr", "--config", "my.toml", "img.png"]);
    assert_eq!(cli.config.unwrap(), std::path::PathBuf::from("my.toml"));
    assert_eq!(cli.paths, vec![std::path::PathBuf::from("img.png")]);
}
