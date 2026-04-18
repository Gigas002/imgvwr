# imgvwr

A minimal, fast image viewer for Wayland, written in Rust.

imgvwr is heavily inspired by [imv](https://sr.ht/~exec64/imv/) by Harry Jeffery — a great piece of software that set the bar for what a lightweight Wayland image viewer should feel like. imgvwr is **not** a port, fork, or direct descendant of imv. It is an independent reimplementation that shares the same spirit: stay small, stay fast, stay out of the way. It does not aim to replicate every feature imv has.

---

## Requirements

**Runtime:**

- A Wayland compositor
- `libwayland-client`
- `libxkbcommon`

**Optional runtime dependencies (feature-gated):**

| Feature                      | Runtime requirement                                                  |
| ---------------------------- | -------------------------------------------------------------------- |
| `gpu-vulkan`                 | Vulkan-capable driver (Mesa, NVIDIA, etc.)                           |
| `gpu-gles`                   | EGL + OpenGL ES 2.0 driver                                           |
| `dmabuf`                     | Compositor with `zwp_linux_dmabuf_v1` support (implies `gpu-vulkan`) |
| `decorations`                | Compositor with `zxdg_decoration_manager_v1` support                 |
| `avif` / `avif-anim`         | `libdav1d`                                                           |
| `jxl` / `jxl-anim`           | `libjxl`                                                             |
| `gif` / `webp-anim` / `apng` | `libc` (virtually always present)                                    |

**Build-time:**

- Rust toolchain (edition 2024, stable)
- `pkg-config`
- Wayland protocol headers (`wayland-protocols`)

---

## Building

Clone the repository and build with Cargo:

```sh
git clone https://github.com/Gigas002/imv
cd imv
cargo build --release
```

The resulting binary is at `target/release/imgvwr`.

### Selecting features

By default only PNG support is compiled in. Enable additional formats and backends with `--features`:

```sh
# Common formats
cargo build --release --features jpeg,webp,avif

# Full format set
cargo build --release --features jpeg,webp,avif,avif-anim,jxl,jxl-anim,gif,webp-anim,apng

# GPU-accelerated rendering via Vulkan
cargo build --release --features gpu-vulkan

# GPU via OpenGL ES / EGL
cargo build --release --features gpu-gles

# DMA-BUF zero-copy (requires gpu-vulkan)
cargo build --release --features dmabuf

# Server-side window decorations
cargo build --release --features decorations

# Shell completions (bash, zsh, fish, nushell, elvish, powershell)
cargo build --release --features completions

# Everything
cargo build --release --all-features
```

### Feature reference

| Feature       | Default | Description                                      |
| ------------- | ------- | ------------------------------------------------ |
| `png`         | yes     | PNG decoding                                     |
| `jpeg`        | no      | JPEG decoding                                    |
| `webp`        | no      | WebP (static) decoding                           |
| `webp-anim`   | no      | WebP animation                                   |
| `avif`        | no      | AVIF (static) decoding via dav1d                 |
| `avif-anim`   | no      | AVIF animation via dav1d + mp4parse              |
| `jxl`         | no      | JPEG XL (static) decoding                        |
| `jxl-anim`    | no      | JPEG XL animation                                |
| `gif`         | no      | GIF (animated) decoding                          |
| `apng`        | no      | Animated PNG decoding                            |
| `decorations` | no      | Server-side window decorations                   |
| `gpu-vulkan`  | no      | GPU rendering via wgpu/Vulkan                    |
| `gpu-gles`    | no      | GPU rendering via wgpu/OpenGL ES                 |
| `dmabuf`      | no      | DMA-BUF zero-copy display (implies `gpu-vulkan`) |
| `logging`     | yes     | `RUST_LOG`-driven tracing output                 |
| `config`      | yes     | TOML config file parsing                         |
| `keybinds`    | yes     | Configurable keybindings                         |
| `completions` | no      | Shell completion script generation               |

---

## Usage

```sh
imgvwr [OPTIONS] [PATHS]...
```

Open one or more image files:

```sh
imgvwr image.png
imgvwr *.jpg
imgvwr ~/pictures/**/*.webp
```

### CLI options

| Option                             | Description                                                                  |
| ---------------------------------- | ---------------------------------------------------------------------------- |
| `[PATHS]...`                       | One or more image file paths to open                                         |
| `--config <PATH>`                  | Load an additional config file (layered on top of system/user config)        |
| `-d, --decorations [true\|false]`  | Override window decoration setting                                           |
| `-a, --antialiasing [true\|false]` | Override antialiasing setting                                                |
| `--min-scale <FLOAT>`              | Minimum zoom factor (e.g. `0.1`)                                             |
| `--max-scale <FLOAT>`              | Maximum zoom factor (e.g. `100.0`)                                           |
| `--scale-step <FLOAT>`             | Zoom step per scroll notch (e.g. `0.1`)                                      |
| `--filter-method <METHOD>`         | Scaling filter: `nearest`, `triangle`, `catmull-rom`, `gaussian`, `lanczos3` |
| `--log-level <LEVEL>`              | Log level: `error`, `warn`, `info`, `debug`, `trace`                         |
| `-h, --help`                       | Print help                                                                   |

CLI options override config file values.

### Default keybindings

| Key      | Action                                    |
| -------- | ----------------------------------------- |
| `q`      | Quit                                      |
| `[`      | Rotate 90° counter-clockwise              |
| `]`      | Rotate 90° clockwise                      |
| `Delete` | Delete current file from disk and advance |

---

## Configuration

imgvwr loads config in this order, with later sources overriding earlier ones:

1. Built-in defaults
2. System config: `/etc/imgvwr/config.toml`
3. User config: `$XDG_CONFIG_HOME/imgvwr/config.toml` (falls back to `~/.config/imgvwr/config.toml`)
4. `--config <PATH>` override (if provided)

An example config with all options documented is in [`examples/config.toml`](examples/config.toml).

---

## License

AGPL-3.0-only. See [LICENSE](LICENSE.txt).

---

## Acknowledgements

> Pre-1.0.0 history is preserved on the [`rust` branch of imv fork repo](https://github.com/Gigas002/imv/tree/rust).

> The old [iced](https://github.com/iced-rs/iced)-powered tree lives on the [`iced` branch](https://github.com/Gigas002/imv/tree/iced).

Thanks to **Harry Jeffery** for creating [imv](https://sr.ht/~exec64/imv/). It is the reference for what a minimal Wayland image viewer should be, and the direct inspiration for this project.
