# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

### libimgvwr

#### Changed

- migrated GPU renderer from hand-written wgpu shaders to [vello](https://github.com/linebender/vello) ([@Gigas002](https://github.com/Gigas002))
- `WaylandContext::dispatch` takes a timeout ([`d5b8967`](https://github.com/Gigas002/imgvwr/commit/d5b8967), [@Gigas002](https://github.com/Gigas002))
- `InputEvent::Scroll` reports wheel notches, summed per pointer frame ([`c86796f`](https://github.com/Gigas002/imgvwr/commit/c86796f), [@Gigas002](https://github.com/Gigas002))
- added `Navigator::from_paths` for several files and directories ([`846525d`](https://github.com/Gigas002/imgvwr/commit/846525d), [@Gigas002](https://github.com/Gigas002))
- CPU renderer computes only the visible part of the zoomed image ([`a6ad41f`](https://github.com/Gigas002/imgvwr/commit/a6ad41f), [@Gigas002](https://github.com/Gigas002))
- GPU half-size levels are built 7–11× faster ([`bcaa5de`](https://github.com/Gigas002/imgvwr/commit/bcaa5de), [@Gigas002](https://github.com/Gigas002))
- SHM frames are double-buffered and drawn on the compositor's frame callback ([`84c67ed`](https://github.com/Gigas002/imgvwr/commit/84c67ed), [@Gigas002](https://github.com/Gigas002))
- `set_title` no longer needs the `decorations` feature ([`4dd9454`](https://github.com/Gigas002/imgvwr/commit/4dd9454), [@Gigas002](https://github.com/Gigas002))
- updated vello to 0.12 ([`a83d822`](https://github.com/Gigas002/imgvwr/commit/a83d822), [@Gigas002](https://github.com/Gigas002))

#### Fixed

- animations and JPEG XL images could allocate unbounded memory; animations over 1 GiB show their first frame ([`38e659e`](https://github.com/Gigas002/imgvwr/commit/38e659e), [@Gigas002](https://github.com/Gigas002))
- pointer position was unknown until the first motion; non-left buttons panned ([`ed71226`](https://github.com/Gigas002/imgvwr/commit/ed71226), [@Gigas002](https://github.com/Gigas002))

### imgvwr

#### Changed

- several paths and directories can be opened and browsed in order ([`846525d`](https://github.com/Gigas002/imgvwr/commit/846525d), [@Gigas002](https://github.com/Gigas002))
- config mistakes (unknown keys, bad values, unreadable files) are reported; an unusable `--config` is an error ([`c707535`](https://github.com/Gigas002/imgvwr/commit/c707535), [@Gigas002](https://github.com/Gigas002))
- errors print as `imgvwr: <path>: <message>` with exit status 1 ([`876804a`](https://github.com/Gigas002/imgvwr/commit/876804a), [@Gigas002](https://github.com/Gigas002))
- the event loop sleeps until input or the next animation frame instead of polling ([`5433bdc`](https://github.com/Gigas002/imgvwr/commit/5433bdc), [@Gigas002](https://github.com/Gigas002))
- `RUST_LOG=imgvwr=debug` logs each frame's render time ([`1e1a032`](https://github.com/Gigas002/imgvwr/commit/1e1a032), [@Gigas002](https://github.com/Gigas002))
- added `docs/AUDIT.md`; documented navigation, mouse, directories and config ([`9c4cff1`](https://github.com/Gigas002/imgvwr/commit/9c4cff1), [@Gigas002](https://github.com/Gigas002))

#### Fixed

- Delete could remove a file that failed to load instead of the image on screen ([`818b3db`](https://github.com/Gigas002/imgvwr/commit/818b3db), [@Gigas002](https://github.com/Gigas002))
- the default build showed a black window until input arrived ([`d5b8967`](https://github.com/Gigas002/imgvwr/commit/d5b8967), [@Gigas002](https://github.com/Gigas002))
- `imgvwr file.png` (a bare file name) failed to open ([`846525d`](https://github.com/Gigas002/imgvwr/commit/846525d), [@Gigas002](https://github.com/Gigas002))
- images were fitted to an 800×600 placeholder instead of the window, and never refitted ([`5f8ab18`](https://github.com/Gigas002/imgvwr/commit/5f8ab18), [@Gigas002](https://github.com/Gigas002))
- one wheel notch zoomed 1.5× the configured `scale_step` ([`c86796f`](https://github.com/Gigas002/imgvwr/commit/c86796f), [@Gigas002](https://github.com/Gigas002))
- some scale settings crashed on the first frame ([`c707535`](https://github.com/Gigas002/imgvwr/commit/c707535), [@Gigas002](https://github.com/Gigas002))
- the window had no title without the `decorations` feature ([`4dd9454`](https://github.com/Gigas002/imgvwr/commit/4dd9454), [@Gigas002](https://github.com/Gigas002))
- cargo manifest warning; CI now runs every `cargo deny` check ([`18dcd2c`](https://github.com/Gigas002/imgvwr/commit/18dcd2c), [@Gigas002](https://github.com/Gigas002))

## [1.1.0] - 2026-09-04

### libimgvwr

#### Changed

- major performance improvement for JPEG XL loading, upgrading to `jxl` 0.6 ([#97](https://github.com/Gigas002/imgvwr/pull/97), [@Gigas002](https://github.com/Gigas002))
- migrated GPU renderer to wgpu 30 ([#85](https://github.com/Gigas002/imgvwr/pull/85), [@Gigas002](https://github.com/Gigas002))
- migrated from `libc` to `rustix` ([#77](https://github.com/Gigas002/imgvwr/pull/77), [@Gigas002](https://github.com/Gigas002))

#### Fixed

- incorrect docs metadata in `Cargo.toml` manifests ([#76](https://github.com/Gigas002/imgvwr/pull/76), [@Gigas002](https://github.com/Gigas002))

### imgvwr

#### Changed

- added `docs/ARCHITECTURE.md` ([`8e948d1`](https://github.com/Gigas002/imgvwr/commit/8e948d1), [@Gigas002](https://github.com/Gigas002))
- updated README and links ([`353c03f`](https://github.com/Gigas002/imgvwr/commit/353c03f), [`887e117`](https://github.com/Gigas002/imgvwr/commit/887e117), [@Gigas002](https://github.com/Gigas002))

#### Fixed

- deployed release binary was missing built-in features ([#69](https://github.com/Gigas002/imgvwr/pull/69), [@Gigas002](https://github.com/Gigas002))
- renamed `LICENSE,txt` to `LICENSE.txt` ([#70](https://github.com/Gigas002/imgvwr/pull/70), [@Gigas002](https://github.com/Gigas002))
- clippy lints for Rust 1.98 ([#99](https://github.com/Gigas002/imgvwr/pull/99), [@Gigas002](https://github.com/Gigas002))
- typos CI job ([`3ab9813`](https://github.com/Gigas002/imgvwr/commit/3ab9813), [@Gigas002](https://github.com/Gigas002))

## [1.0.0] - 2026-04-19

[Unreleased]: https://github.com/Gigas002/imgvwr/compare/v1.0.0...HEAD
