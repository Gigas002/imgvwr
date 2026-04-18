//! xkbcommon keymap and key-event state wrapper.
//!
//! [`KeyboardState`] is created from the keymap fd delivered by the
//! `wl_keyboard::keymap` event and updated on every key press/release.
//! Only key-press events produce a [`Keysym`]; releases return `None`.

use std::{io, os::fd::OwnedFd};

use wayland_client::protocol::wl_keyboard;
use xkbcommon::xkb::{
    self, CONTEXT_NO_FLAGS, KEYMAP_COMPILE_NO_FLAGS, KEYMAP_FORMAT_TEXT_V1, KeyDirection, Keycode,
    Keysym,
};

/// Live xkbcommon context, keymap, and state for the seat keyboard.
pub struct KeyboardState {
    // Fields kept to satisfy the borrow requirements of xkb::State.
    _context: xkb::Context,
    _keymap: xkb::Keymap,
    state: xkb::State,
}

/// Compile a new [`KeyboardState`] from the keymap `fd` and byte `size`
/// received in the `wl_keyboard::keymap` event.
///
/// Takes ownership of `fd`; the caller should not close it separately.
///
/// # Errors
/// Returns an error if the memory-map or keymap compilation fails.
pub fn update_keymap(fd: OwnedFd, size: u32) -> io::Result<KeyboardState> {
    let context = xkb::Context::new(CONTEXT_NO_FLAGS);
    let keymap = unsafe {
        xkb::Keymap::new_from_fd(
            &context,
            fd,
            size as usize,
            KEYMAP_FORMAT_TEXT_V1,
            KEYMAP_COMPILE_NO_FLAGS,
        )
    }?
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "xkbcommon keymap compilation failed",
        )
    })?;
    let state = xkb::State::new(&keymap);
    Ok(KeyboardState {
        _context: context,
        _keymap: keymap,
        state,
    })
}

/// Feed a key event into `keyboard_state` and return the [`Keysym`] for
/// key-press events, or `None` for releases.
///
/// `key` is the raw Linux evdev scancode from the `wl_keyboard::key` event.
/// XKB keycodes are evdev + 8.
pub fn key_event(
    keyboard_state: &mut KeyboardState,
    key: u32,
    key_state: wl_keyboard::KeyState,
) -> Option<Keysym> {
    let pressed = match key_state {
        wl_keyboard::KeyState::Pressed => true,
        wl_keyboard::KeyState::Released => false,
        _ => return None,
    };
    let keycode = Keycode::new(key + 8);
    let direction = if pressed {
        KeyDirection::Down
    } else {
        KeyDirection::Up
    };
    keyboard_state.state.update_key(keycode, direction);
    if pressed {
        let sym = keyboard_state.state.key_get_one_sym(keycode);
        if sym != Keysym::NoSymbol {
            Some(sym)
        } else {
            None
        }
    } else {
        None
    }
}
