//! Keybind resolution: keysym lookup and action dispatch.
//!
//! [`keysym_from_str`] is exported so `imgvwr::config` can resolve key names
//! from the config file at startup. [`KeybindMap`] is constructed from the
//! already-resolved keysyms and used in the event loop.

#[cfg(test)]
mod tests;

use std::collections::HashMap;

pub use xkbcommon::xkb::Keysym;
use xkbcommon::xkb::{self, KEYSYM_NO_FLAGS};

/// Actions that can be triggered by a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    RotateLeft,
    RotateRight,
    DeleteFile,
}

/// Error returned when a key name cannot be resolved to a keysym.
#[derive(Debug)]
pub struct KeybindError(String);

impl std::fmt::Display for KeybindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown key name: {:?}", self.0)
    }
}

impl std::error::Error for KeybindError {}

/// Resolve a key name string (e.g. `"q"`, `"bracketleft"`) to an XKB
/// [`Keysym`].
///
/// Returns [`KeybindError`] if the name is not recognised by xkbcommon.
pub fn keysym_from_str(name: &str) -> Result<Keysym, KeybindError> {
    let sym = xkb::keysym_from_name(name, KEYSYM_NO_FLAGS);
    if sym == Keysym::NoSymbol {
        Err(KeybindError(name.to_owned()))
    } else {
        Ok(sym)
    }
}

/// Maps resolved [`Keysym`]s to [`Action`]s for O(1) lookup in the event loop.
pub struct KeybindMap {
    inner: HashMap<Keysym, Action>,
}

impl KeybindMap {
    /// Build the map from pre-resolved keysyms.
    pub fn new(quit: Keysym, rotate_left: Keysym, rotate_right: Keysym, delete: Keysym) -> Self {
        let mut inner = HashMap::with_capacity(4);
        inner.insert(quit, Action::Quit);
        inner.insert(rotate_left, Action::RotateLeft);
        inner.insert(rotate_right, Action::RotateRight);
        inner.insert(delete, Action::DeleteFile);
        KeybindMap { inner }
    }

    /// Look up the [`Action`] bound to `sym`, if any.
    pub fn lookup(&self, sym: Keysym) -> Option<Action> {
        self.inner.get(&sym).copied()
    }
}
