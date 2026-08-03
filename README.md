# imgvwr

A minimal, fast image viewer for Wayland, written in Rust.

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

- Rust toolchain
- `pkg-config`
- Wayland protocol headers (`wayland-protocols`)

---

## Feature reference

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

### Default keybindings

| Key      | Action                                    |
| -------- | ----------------------------------------- |
| `q`      | Quit                                      |
| `[`      | Rotate 90° counter-clockwise              |
| `]`      | Rotate 90° clockwise                      |
| `Delete` | Delete current file from disk and advance |

---

## Acknowledgements

Thanks to **Harry Jeffery** for creating [imv](https://sr.ht/~exec64/imv/). It is the reference for what a minimal Wayland image viewer should be, and the direct inspiration for this project.
