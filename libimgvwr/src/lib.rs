//! Core engine for the `imgvwr` image viewer.
//!
//! Provides image loading, viewport state management, directory navigation,
//! and a software rendering pipeline. All Wayland and config concerns live in
//! the `imgvwr` binary crate; this library has no knowledge of either.

pub mod keybinds;
pub mod loader;
pub mod navigator;
pub mod renderer;
pub mod viewport;
pub mod wayland;
