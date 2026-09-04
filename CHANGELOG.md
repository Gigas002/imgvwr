# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

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
