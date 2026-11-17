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
| `gpu-gles`                   | EGL + OpenGL ES 3.1 driver (vello needs compute shaders)             |
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
| `gpu-vulkan`  | no      | GPU rendering via vello on wgpu/Vulkan           |
| `gpu-gles`    | no      | GPU rendering via vello on wgpu/OpenGL ES        |
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

Open one or more image files or directories:

```sh
imgvwr image.png
imgvwr *.jpg
imgvwr ~/pictures/**/*.webp
imgvwr ~/pictures/
```

A single file opens with the other images of its directory available for
browsing, starting at that file. Several paths are browsed in the given
order; a directory contributes the images it contains. Files that fail to
load are skipped.

Each image is scaled to fit the window, and refitted when the window or
rotation changes, until you zoom or pan.

### Mouse

| Input            | Action                                                |
| ---------------- | ----------------------------------------------------- |
| Wheel            | Zoom by `scale_step` per notch, around the pointer    |
| Left-button drag | Pan                                                   |

### Configuration

Settings are read from `/etc/imgvwr/config.toml`, then
`$XDG_CONFIG_HOME/imgvwr/config.toml` (or `~/.config/imgvwr/config.toml`),
then the file given with `--config`; later files override earlier ones and
command-line options override all of them. See
[`examples/config.toml`](examples/config.toml) for every key with its default.
Unknown keys and invalid values are reported on start-up (and replaced by the
default) instead of being silently ignored.

### CLI options

| Option                             | Description                                                                  |
| ---------------------------------- | ---------------------------------------------------------------------------- |
| `[PATHS]...`                       | Image files or directories to open                                           |
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

| Key      | Action                                                     |
| -------- | ---------------------------------------------------------- |
| `q`      | Quit                                                       |
| `Left`   | Previous image                                             |
| `Right`  | Next image                                                 |
| `[`      | Rotate 90° counter-clockwise                               |
| `]`      | Rotate 90° clockwise                                       |
| `Delete` | Delete the shown file from disk (permanently) and advance  |

`q`, `[`, `]` and `Delete` can be rebound in the `[keybindings]` section of
the config (feature `keybinds`).

---

## Acknowledgements

Thanks to **Harry Jeffery** for creating [imv](https://sr.ht/~exec64/imv/). It is the reference for what a minimal Wayland image viewer should be, and the direct inspiration for this project.
