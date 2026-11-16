use super::*;

#[test]
fn keysym_from_str_known_key() {
    let sym = keysym_from_str("q").unwrap();
    assert_ne!(sym, Keysym::NoSymbol);
}

#[test]
fn keysym_from_str_bracket_keys() {
    keysym_from_str("bracketleft").unwrap();
    keysym_from_str("bracketright").unwrap();
}

#[test]
fn keysym_from_str_unknown_errors() {
    assert!(keysym_from_str("invalid_xyz_key").is_err());
}

#[test]
fn keybind_map_lookup_round_trip() {
    let quit = keysym_from_str("q").unwrap();
    let rl = keysym_from_str("bracketleft").unwrap();
    let rr = keysym_from_str("bracketright").unwrap();

    let del = keysym_from_str("Delete").unwrap();
    let map = KeybindMap::new(quit, rl, rr, del);

    assert_eq!(map.lookup(quit), Some(Action::Quit));
    assert_eq!(map.lookup(rl), Some(Action::RotateLeft));
    assert_eq!(map.lookup(rr), Some(Action::RotateRight));
    assert_eq!(map.lookup(del), Some(Action::DeleteFile));
}

#[test]
fn keybind_map_unknown_sym_returns_none() {
    let quit = keysym_from_str("q").unwrap();
    let rl = keysym_from_str("bracketleft").unwrap();
    let rr = keysym_from_str("bracketright").unwrap();
    let del = keysym_from_str("Delete").unwrap();
    let map = KeybindMap::new(quit, rl, rr, del);

    let unbound = keysym_from_str("z").unwrap();
    assert_eq!(map.lookup(unbound), None);
}

#[test]
fn keysym_from_str_accepts_single_characters() {
    // "[" is not an XKB keysym name, but it is the character on the key.
    assert_eq!(
        keysym_from_str("[").unwrap(),
        keysym_from_str("bracketleft").unwrap()
    );
    assert_eq!(
        keysym_from_str("=").unwrap(),
        keysym_from_str("equal").unwrap()
    );
    assert_eq!(
        keysym_from_str("ä").unwrap(),
        keysym_from_str("adiaeresis").unwrap()
    );
}

#[test]
fn keysym_from_str_rejects_empty_and_long_garbage() {
    assert!(keysym_from_str("").is_err());
    assert!(keysym_from_str("[[").is_err());
}
