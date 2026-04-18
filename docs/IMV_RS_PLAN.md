# imv → Rust rewrite plan (`imgvwr`)

This document is both a **human roadmap** and an **agent playbook**: each step is small enough to implement in one focused session, ends in a **verified** state (**build** + **fmt/clippy**; **tests** per §5 once they exist), and defines **how to verify** it. **Reading this plan, implementing accordingly, and updating it** (checkboxes, revision history when policy changes) is the primary execution discipline.

This is **not** a port of the C `imv` codebase. It is a **clean-room Rust rewrite** of a minimal image viewer inspired by imv/mpv UI philosophy. C source under `src/` remains as **read-only reference** until Phase 9.

---

## 1. Goals and constraints

### 1.1 Goals

- **Minimal image viewer** for Wayland: open image file(s), navigate prev/next in directory, zoom, pan, rotate (viewer-only — no file writes).
- **Two-crate workspace**: `libimgvwr` (library — engine, viewport, renderer, Wayland surface) and `imgvwr` (CLI binary — config parsing, arg parsing, event loop wiring).
- **No OpenGL**: use **Wayland SHM** (shared memory buffers) with **software rendering via `image-rs` / `imageops`** for transforms (scale, rotate). This is the default path. Optional GPU acceleration via `wgpu` is gated behind the `gpu` Cargo feature (Phase 8). `wgpu` prefers Vulkan; if Vulkan is absent it falls back to its GL backend (which uses EGL internally) — but no OpenGL or EGL code is written directly in this codebase.
- **image-rs for all decoding**: no format-specific C libraries pulled in by this crate directly. PNG only by default; JPEG/WebP/AVIF/JXL behind optional Cargo features (§4).
- **Wayland-only**: no X11, no XWayland, no platform abstraction layer.
- **No IPC**: no Unix socket, no remote control binary.
- **Config** via TOML (`~/.config/imgvwr/config.toml`, system `/etc/imgvwr/config.toml`); single `--config` CLI override. No other CLI options.
- **Compile-time feature split**: keep the binary minimal by default; optional formats and optional decoration support gated behind features (§4).
- **Rust edition 2024** across the workspace.
- **MIT license** for new Rust code; no strong-copyleft dependencies.

### 1.2 Non-goals

- **X11 / XWayland** support.
- **IPC / remote control** (`imv-msg` equivalent).
- **Any CLI option except `--config`**. File paths are positional arguments, not options.
- **Animated images** (GIF, animated WebP, animated AVIF, animated JXL) — not planned.
- **Image editing / writing**: rotation and zoom are viewer-side only; source files are never modified.
- **Window decorations beyond title**: no custom title bar drawing, no borders, no status overlay bar. Window title is optional and gated behind the `decorations` feature.
- **Compatibility with the C imv** config format, keybind syntax, or command system.
- **Slideshow / timer** mode.
- **Man pages** as a CI deliverable.

### 1.3 Reference map (C → Rust ownership)

| C area            | Role                                   | Rust home                                                               |
| ----------------- | -------------------------------------- | ----------------------------------------------------------------------- |
| `src/imv.c`       | Main loop, viewport state, image state | `libimgvwr::app` state struct + `imgvwr::main` wiring                   |
| `src/backend_*.c` | Per-format decoders                    | Replaced by **`image-rs`** crate features (§3.2)                        |
| `src/viewport.c`  | Pan / zoom / rotate math               | **`libimgvwr::viewport`**                                               |
| `src/navigator.c` | Directory file list                    | **`libimgvwr::navigator`**                                              |
| `src/canvas.c`    | OpenGL + Cairo drawing                 | **Replaced**: `libimgvwr::renderer` does software blit to SHM           |
| `src/wl_window.c` | Wayland surface, EGL, input            | **`libimgvwr::wayland`** (wayland-client, no EGL)                       |
| `src/keyboard.c`  | xkbcommon state                        | `libimgvwr::wayland::keyboard`                                          |
| `src/binds.c`     | Keybind dispatch                       | **`libimgvwr::keybinds`** — simplified (no trie; only single-key binds) |
| `src/log.c`       | Logging                                | `tracing` + `tracing-subscriber`                                        |
| `imv_config`      | Config                                 | **`imgvwr::config`** — all config types, I/O, TOML parse, defaults      |

### 1.4 Crate responsibilities

| Concern           | **`imgvwr`** (CLI / binary)                                                                                                           | **`libimgvwr`** (library / engine)                   |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| **Config**        | All config types (`Config`, `WindowConfig`, `ViewerConfig`, `KeybindConfig`), TOML I/O, path resolution, merge order, `--config` flag | Nothing — no config knowledge                        |
| **CLI**           | Positional args (image paths), `--config`, `clap` parse                                                                               | Nothing — no CLI knowledge                           |
| **Wayland**       | Thin wiring: pass handles into `libimgvwr`                                                                                            | Full Wayland client: surface, SHM, pointer, keyboard |
| **Rendering**     | Nothing                                                                                                                               | Software blit pipeline in `libimgvwr::renderer`      |
| **Image loading** | Nothing                                                                                                                               | `libimgvwr::loader` wraps `image-rs`                 |
| **Viewport math** | Nothing                                                                                                                               | `libimgvwr::viewport`                                |
| **Navigator**     | Nothing                                                                                                                               | `libimgvwr::navigator`                               |

---

## 2. Repository layout (target)

```text
imv/                          ← existing repo root (C tree stays as reference)
  Cargo.toml                  ← [workspace] members = ["libimgvwr", "imgvwr"]
  Cargo.lock                  ← committed (binary workspace)
  libimgvwr/
    Cargo.toml                ← package name = "libimgvwr"; [features] §4.1
    src/
      lib.rs                  ← pub use; no logic, no tests
      loader/
        mod.rs                ← load_image() → DynamicImage, feature gates per format
        tests.rs
      viewport/
        mod.rs                ← ViewportState: scale, offset, rotation
        tests.rs
      navigator/
        mod.rs                ← dir scan, next/prev, current path
        tests.rs
      renderer/
        mod.rs                ← software blit: transform image → SHM pixel buffer
        tests.rs
      wayland/
        mod.rs                ← WaylandState: display, surfaces, seat
        keyboard.rs           ← xkbcommon wrapper, keysym → KeyEvent
        shm.rs                ← SHM pool: fd, mmap, wl_buffer lifecycle
        tests.rs
      keybinds/
        mod.rs                ← KeybindMap: keysym → Action enum; no config dependency
        tests.rs
    tests/                    ← optional integration tests (headless, no compositor)
  imgvwr/
    Cargo.toml                ← package name = "imgvwr"; [[bin]] name = "imgvwr"; features mirror §4.2
    src/
      main.rs                 ← event loop, spawn, wiring; no business logic
      config/
        mod.rs                ← Config/WindowConfig/ViewerConfig/KeybindConfig structs, TOML parse, path resolution, merge, defaults
        tests.rs
      cli/
        mod.rs                ← clap: positional image paths + --config
        tests.rs
    tests/                    ← optional: spawn imgvwr, fixture images
  examples/
    config.toml               ← canonical config reference (TOML, §3.1)
  docs/
    IMV_RS_PLAN.md            ← this file
    TOFI_RUST_MIGRATION_PLAN.md
  src/                        ← C tree (read-only reference; removed in Phase 9)
```

### 2.1 Toolchain and dependency policy

- **Rust edition**: `2024` in `[workspace.package]` and each crate manifest.
- **`rust-version`**: do not pin. Track latest stable; document in README.
- **Version specifiers**: prefer `x.y` (two components) in `Cargo.toml`; exact versions in `Cargo.lock`.
- **Dependency health**: prefer crates with a release or meaningful commit within ~one year; no deprecated crates.
- **License**: prefer MIT / Apache-2.0 / BSD / ISC. Run `cargo deny check licenses` (§9). image-rs is MIT/Apache-2.0.

### 2.2 CI (adopt in Phase 0)

**Reference**: use branch `rust` of https://github.com/Gigas002/tofi as the canonical layout for all CI workflows, `deny.toml`, `.typos.toml`, `dependabot.yml`, `Cargo.toml` metadata, and docs structure. Mirror directly; adapt only names and system deps.

| Workflow         | Role                                                                                                                                                                      |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `build.yml`      | `cargo build --workspace --release`; matrix: default features, `--all-features`, `--no-default-features`. Uses `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`. |
| `fmt-clippy.yml` | `cargo fmt -- --check`; `cargo clippy -D warnings`; matrix on feature sets.                                                                                               |
| `test.yml`       | `cargo test --workspace`; headless tests only (no compositor required).                                                                                                   |
| `typos.yml`      | `typos` spell check.                                                                                                                                                      |
| `deploy.yml`     | Release artifacts; `workflow_dispatch` only until Phase 9.                                                                                                                |

Add `.github/dependabot.yml` with `package-ecosystem: cargo` and `github-actions` in Phase 0.

**System packages** needed for CI (Arch example): `wayland`, `wayland-protocols`, `xkbcommon`, `libavif` (only when `avif` feature is tested).

Remove legacy Meson CI in the same commit that adds Rust CI.

---

## 3. Dependencies strategy

### 3.1 Config: TOML

Use `toml` crate (MIT/Apache-2.0) + `serde` + `serde_derive` for deserializing `config.toml`. All config types and I/O live entirely in `imgvwr::config`. `libimgvwr` has no config module.

**Config file format** (TOML):

```toml
[window]
# Show window title bar. Requires feature "decorations".
decorations = false
antialiasing = true

[viewer]
min_scale = 0.1
max_scale = 100.0
scale_step = 0.08
filter_method = "nearest"   # "nearest" | "linear" | "lanczos3"

[keybindings]
quit        = "q"
rotate_left  = "["
rotate_right = "]"
```

Resolution order: built-in defaults → `/etc/imgvwr/config.toml` → `$XDG_CONFIG_HOME/imgvwr/config.toml` (~`/.config`) → `--config <path>`. Later sources override individual keys.

`filter_method` maps to `libimgvwr::renderer::FilterMethod` enum (an API type, not a config type); `imgvwr::config` parses the string and converts.

### 3.2 Image decoding: image-rs

Crate: `image` (MIT/Apache-2.0). Use `default-features = false` in `libimgvwr/Cargo.toml` and enable formats selectively via features (§4.1).

| Format  | image-rs feature | Plan                                                      |
| ------- | ---------------- | --------------------------------------------------------- |
| PNG     | `png`            | **Default on**                                            |
| JPEG    | `jpeg`           | Optional (`jpeg` feature)                                 |
| WebP    | `webp`           | Optional (`webp` feature)                                 |
| AVIF    | `avif`           | Optional (`avif` feature); requires system `libavif`      |
| JPEG XL | `jxl`            | **Future** (Phase 9); image-rs `jxl` or `jxl-oxide` crate |

image-rs provides `imageops::resize`, `imageops::rotate90/180/270`, and `DynamicImage::to_rgba8()`. These replace all of the C backend decoders and the Cairo-based rendering transformations.

**Animated image note**: image-rs has animation support (e.g. `image::codecs::gif::GifDecoder`), but this project **does not** use it. Load only the first frame; ignore subsequent frames.

### 3.3 Wayland

Use `wayland-client` + `wayland-protocols` (LGPL-2.1 — standard system library exception applies; dynamically linked via system `libwayland-client`).

Protocols needed:

- `xdg-shell` (stable) → window shell, configure/ack cycle, close
- `xdg-decoration-unstable-v1` → optional server-side decorations (only when `decorations` feature is enabled)
- `zwp-pointer-gestures-v1` → optional pinch-to-zoom (future; §9)
- No layer shell (imv is a normal toplevel window, not an overlay)
- No wlr-specific protocols

For pointer gestures: `wayland-protocols-wlr` if needed. For v1, skip; use scroll wheel for zoom only.

### 3.4 Keyboard

Use `xkbcommon` crate (MIT). Map keysyms to `libimgvwr::keybinds::Action` enum. No multi-key sequences (the C trie is overkill for our bind set). One keysym → one action.

### 3.5 SHM rendering pipeline

No GPU stack required. Pipeline per frame:

```
image-rs DynamicImage (full resolution, in memory)
    ↓
viewport: compute src_rect (pan + crop for out-of-bounds drag) + dst_size (scaled to window)
    ↓
imageops::resize(dst_size, FilterMethod → imageops::FilterType)
    ↓
imageops::rotate90 / rotate180 / rotate270 (if rotation != 0°)
    ↓
to_rgba8() → &[u8]   (RGBA, 4 bytes/pixel)
    ↓
convert RGBA → ARGB (Wayland SHM expects wl_shm_format::xrgb8888 / argb8888)
    ↓
memcpy into mmap'd SHM pool buffer
    ↓
wl_surface.attach(wl_buffer) + damage_buffer(full) + commit
```

SHM pool: allocate `width × height × 4` bytes via `memfd_create` (Linux) or `shm_open`; use `rustix` for the syscall. Recreate pool on window resize. Double-buffer if tearing is observed.

For pan/drag: since we blit the full scaled image, panning beyond window bounds is implemented by offsetting the blit destination and filling the remainder with background color. The image can be dragged fully out of the window area.

### 3.6 Logging

Use `tracing` + `tracing-subscriber` (MIT). Gate verbose output behind `RUST_LOG` env (subscriber with `EnvFilter`). No separate feature flag needed.

---

## 4. Cargo features

### 4.1 `libimgvwr` features

```toml
[features]
default = ["png"]
png   = ["image/png"]
jpeg  = ["image/jpeg"]
webp  = ["image/webp"]
avif  = ["image/avif"]
# jxl = ["image/jxl"]       # future; uncomment when image-rs jxl is stable
decorations = []             # enables xdg-decoration protocol wiring + window title
gpu-vulkan  = ["dep:wgpu", "dep:pollster", "wgpu/vulkan", "wgpu/wgsl"]  # GPU via Vulkan (Phase 8)
gpu-gles    = ["dep:wgpu", "dep:pollster", "wgpu/gles",   "wgpu/wgsl"]  # GPU via GLES/EGL (Phase 8)
```

**Rules:**

- `default` = only PNG. End users opt in to additional formats at build time.
- All format features forward to the corresponding `image` crate feature.
- `decorations`: when disabled, window title is never set; `xdg-decoration` negotiation is skipped entirely. No `#[cfg]` spaghetti — use a stub module pattern (see §5.3 testing note).
- `gpu-vulkan` and `gpu-gles` are **independent and mutually exclusive by convention** — enabling both compiles both wgpu backends (larger binary, no other harm). Packagers pick one. Common GPU code is gated on `#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]`.
- Every feature combination must compile: test `--no-default-features`, `--all-features`, and `--features jpeg,webp` in CI.

### 4.2 `imgvwr` features

Mirror every `libimgvwr` feature:

```toml
[dependencies]
libimgvwr = { path = "../libimgvwr", default-features = false }

[features]
default      = ["png"]
png          = ["libimgvwr/png"]
jpeg         = ["libimgvwr/jpeg"]
webp         = ["libimgvwr/webp"]
avif         = ["libimgvwr/avif"]
decorations  = ["libimgvwr/decorations"]
gpu-vulkan   = ["libimgvwr/gpu-vulkan"]
gpu-gles     = ["libimgvwr/gpu-gles"]
```

Packagers use `cargo build -p imgvwr --no-default-features --features "png,jpeg,webp"` etc.

---

## 5. Execution workflow

### 5.1 Workflow per step

1. Read the relevant §6 step carefully.
2. Implement only what the step describes — no scope creep.
3. After each step: `cargo build --workspace` must succeed; `cargo clippy --workspace -- -D warnings` must be clean; `cargo fmt --check` must pass; tests for completed modules must pass.
4. Check the step's checkbox in §6.
5. Update §10 revision history if policy changed.

### 5.2 Agent execution contract

- Only implement what the current step specifies.
- Do not add error handling for impossible paths.
- Do not add comments explaining what the code does — only comments for non-obvious WHY (hidden constraint, workaround, invariant).
- Do not implement features not in the current step's scope, even if "obvious".
- Stub modules with `todo!()` rather than leaving them absent; stubs let CI pass.
- When a step says "write tests", write them in a separate `tests.rs` file — never inline `#[test]` in `mod.rs` or `lib.rs`.

### 5.3 Testing strategy

**Rule**: tests live in `<module>/tests.rs`, not in `mod.rs`. `lib.rs` has no tests.

```rust
// libimgvwr/src/viewport/mod.rs
mod tests;   // always at top; tests.rs is adjacent

// libimgvwr/src/viewport/tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    // ...
}
```

**What to test:**

- `viewport`: pure math — zoom clamp, rotation cycle, pan is additive
- `navigator`: directory scan order, next/prev wrapping
- `loader`: load known test image bytes (embed with `include_bytes!`); assert dimensions
- `keybinds`: `keysym_from_str` + `KeybindMap` lookup round-trip
- `imgvwr::config`: TOML parse from string, default merge, unknown keys ignored

**What not to test (or test manually):**

- Wayland protocol interactions (require compositor)
- SHM buffer lifecycle
- Rendering pixel output (smoke-test manually)

**Test images**: keep tiny fixture PNGs under `libimgvwr/tests/fixtures/` (≤4×4 px). Embed with `include_bytes!` in tests. No large files committed.

---

## 6. Phased steps

### Phase 0 — Workspace bootstrap

All CI config is modelled after the `rust` branch of https://github.com/Gigas002/tofi — adapt names and system deps, do not diverge in structure.

#### 0.1 — Cargo workspace skeleton

- [x] Create root `Cargo.toml` (`[workspace]`, members `["libimgvwr", "imgvwr"]`, `edition = "2024"`).
- [x] Create `libimgvwr/Cargo.toml` (package `name = "libimgvwr"`, empty `src/lib.rs`).
- [x] Create `imgvwr/Cargo.toml` (package `name = "imgvwr"`, `[[bin]] name = "imgvwr"`, empty `src/main.rs`).
- [x] Commit `Cargo.lock` (binary workspace — lock is always committed).

**Verify**: `cargo build --workspace` compiles with zero warnings.

#### 0.2 — CI workflows and repo config ✓

#### 0.2 — CI workflows and repo config

**Reference**: https://github.com/Gigas002/tofi branch `rust`. All workflow files, `deny.toml`, `.typos.toml`, `dependabot.yml`, and `Cargo.toml` metadata (authors, repository, license field, etc.) must be modelled from that branch. Adapt only the following for `imgvwr`:

- **System deps** (replaces Cairo/Pango/Harfbuzz): `pkg-config`, `libwayland-dev`, `libxkbcommon-dev`. Add `libavif-dev` only in steps that test the `avif` feature.
- **`build.yml`**: keep three-matrix (`all-features`, `default`, `minimal`).
- **`fmt-clippy.yml`**: keep two-matrix clippy (`--all-features`, `--no-default-features`).
- **`test.yml`**: coverage targets are `-p libimgvwr` and `-p imgvwr`; Codecov flags `libimgvwr` / `imgvwr`.
- **`deploy.yml`**: binary is `imgvwr`; archive `imgvwr-${VERSION}-x86_64-linux.tar.gz`; publish step targets `libimgvwr` (library only — `imgvwr` binary is not published to crates.io).
- **`deny.yml`**, **`typos.yml`**, **`doc.yml`**, **`dependabot.yml`**: exact copies; only adapt system deps where needed.
- **`.typos.toml`**: mirror tofi's structure; `extend-exclude` must cover `src/**`, `test/**`, `subprojects/**`, `doc/**`, `files/**`, `contrib/**` (legacy C tree present until Phase 10). Drop tofi-specific word exceptions; add back only if real false-positives appear.
- **`deny.toml`**: copy as-is from tofi. Note: `LGPL-2.1` (wayland-client) is covered by the system-library dynamic-link exception — add to `allow` only if `cargo deny` flags it after `Cargo.lock` is populated.

**Verify**: `cargo build --workspace --all-features` green. CI passes on empty crates. `typos` passes. `cargo deny check licenses` passes.

---

### Phase 1 — Config types and TOML parsing

- [x] **1.1** Implement `libimgvwr::renderer::FilterMethod`:
  - `FilterMethod` enum: `Nearest`, `Triangle`, `CatmullRom`, `Gaussian`, `Lanczos3`; `impl From<FilterMethod> for image::imageops::FilterType` (must mirror all image-rs types)
  - This is a renderer API type, not a config type — lives in `libimgvwr`, exported for `imgvwr` to use when calling `renderer::render()`.
  - No tests needed here (tested via renderer tests in Phase 4).

- [x] **1.2** Implement `imgvwr::config`:
  - `FilterMethod` string deserializer: `"nearest"` → `libimgvwr::renderer::FilterMethod::Nearest` etc.
  - `WindowConfig { decorations: bool, antialiasing: bool }` with `impl Default`
  - `ViewerConfig { min_scale: f32, max_scale: f32, scale_step: f32, filter_method: FilterMethod }` with `impl Default`
  - `KeybindConfig { quit: String, rotate_left: String, rotate_right: String }` with `impl Default`
  - `Config { window: WindowConfig, viewer: ViewerConfig, keybindings: KeybindConfig }` with `impl Default`
  - `fn load(path: &Path) -> Result<Config>` — read file → `toml::from_str`; fields not present in TOML retain `Default` values
  - `fn resolve_paths(override_path: Option<&Path>) -> Vec<PathBuf>` — system → user XDG → override
  - `fn merge(base: Config, overlay: Config) -> Config` — overlay wins per-field
  - Write `config/tests.rs`: round-trip from TOML string, missing keys keep defaults, unknown keys ignored.

- [x] **1.3** Implement `imgvwr::cli`:
  - `clap::Parser` struct: positional `paths: Vec<PathBuf>` + `--config <path>: Option<PathBuf>`
  - No other options.
  - Write `cli/tests.rs`: parse no args, parse `--config foo.toml`, parse file paths.

**Verify**: `cargo test -p imgvwr` all pass. `cargo test -p libimgvwr` compiles (no config module to test yet).

---

### Phase 2 — Image loading

- [x] **2.1** Implement `libimgvwr::loader`:
  - `pub fn load(path: &Path) -> Result<DynamicImage, LoadError>` — wraps `image::open()`
  - `LoadError` enum: `Io(std::io::Error)`, `Decode(image::ImageError)`, `UnsupportedFormat`
  - No async; single-threaded blocking load (sufficient for an image viewer — one image at a time).
  - Write `loader/tests.rs`: embed `include_bytes!("../../tests/fixtures/4x4.png")`, call `load()` from `tempfile`, assert width = 4 height = 4; test `UnsupportedFormat` on fake extension.

- [x] **2.2** Add test fixture: `libimgvwr/tests/fixtures/4x4.png` (4×4 RGBA white PNG). Format-specific fixtures (`4x4.jpg`, `4x4.webp`, `4x4.avif`, `4x4.jxl`) are added in Phase 8 alongside their feature steps.

**Verify**: `cargo test -p libimgvwr --features png` passes. `--no-default-features` compiles (loader returns `UnsupportedFormat` for all paths).

---

### Phase 3 — Viewport and navigator

- [x] **3.1** Implement `libimgvwr::viewport`:
  - `ViewportState { scale: f32, offset: (f32, f32), rotation: u16 }` — rotation is 0/90/180/270 only
  - `fn zoom_by(&mut self, delta: f32, min_scale: f32, max_scale: f32)` — clamp to `[min_scale, max_scale]`; caller passes values from `imgvwr::config`
  - `fn rotate_left(&mut self)`, `fn rotate_right(&mut self)` — cycle through 0/270/180/90
  - `fn pan(&mut self, dx: f32, dy: f32)` — unconstrained; allows image to be moved fully out of view (no clamping — user-requested drag-anywhere behavior)
  - `fn reset(&mut self)` — scale = 1.0, offset = (0,0), rotation = 0
  - Write `viewport/tests.rs`: zoom clamp, rotation wraparound, pan is additive.

- [x] **3.2** Implement `libimgvwr::navigator`:
  - `Navigator { paths: Vec<PathBuf>, current: usize }`
  - `fn from_path(p: &Path) -> Result<Navigator>` — if `p` is a file: scan sibling directory for supported image extensions; if `p` is a dir: scan it; sort by filename
  - `fn current(&self) -> &Path`
  - `fn next(&mut self) -> &Path` — wraps around
  - `fn prev(&mut self) -> &Path` — wraps around
  - Extensions to include: png always; jpeg/webp/avif/jxl behind `#[cfg(feature = "...")]`
  - Write `navigator/tests.rs`: use `tempdir`, populate known filenames, assert ordering and wrap.

**Verify**: all unit tests pass; no Wayland or image-rs needed for these modules.

---

### Phase 4 — Software renderer

- [x] **4.1** Implement `libimgvwr::renderer`:
  - `pub fn render(src: &DynamicImage, viewport: &ViewportState, dst_w: u32, dst_h: u32, filter: FilterMethod) -> Vec<u8>`
    - Compute `scaled_w = (src.width() as f32 * viewport.scale) as u32`, `scaled_h` analogously
    - `imageops::resize(src, scaled_w, scaled_h, filter.into())` → `ImageBuffer`
    - Apply rotation: `imageops::rotate90/180/270` (only if rotation != 0)
    - Blit into `dst_w × dst_h` ARGB buffer (zeroed = background, color configurable in future):
      - `blit_x = (dst_w as i32 / 2) - (scaled_w as i32 / 2) + viewport.offset.0 as i32`
      - `blit_y = (dst_h as i32 / 2) - (scaled_h as i32 / 2) + viewport.offset.1 as i32`
      - Copy pixel rows where destination rect and source rect overlap; skip rows/cols out of bounds
    - Convert each RGBA pixel to ARGB (wl_shm_format `ARGB8888`): `[A, R, G, B]` → `[B, G, R, A]` (little-endian `0xAARRGGBB`)
  - Write `renderer/tests.rs`: render 4×4 red image at scale 1.0 into 8×8 buffer, assert center pixels are red (ARGB), corners are black (background).

- [x] **4.2** Background color: hardcode `0x00000000` (transparent/black) for now; make it a `Config` field in Phase 8 polish if desired.

**Verify**: renderer tests pass; no Wayland needed.

---

### Phase 5 — Wayland core

- [x] **5.1** Implement `libimgvwr::wayland::shm`:
  - `ShmPool { fd: OwnedFd, mmap: MmapMut, size: usize }` — created with `rustix::fs::memfd_create`
  - `fn create(size: usize) -> Result<ShmPool>` — create memfd, `ftruncate`, `mmap`
  - `fn resize(&mut self, new_size: usize)` — `ftruncate` + remap
  - `fn as_mut_slice(&mut self) -> &mut [u8]`
  - Expose `fd()` for passing to `wl_shm.create_pool`

- [x] **5.2** Implement `libimgvwr::wayland::keyboard`:
  - Wrap `xkbcommon::xkb::{Context, Keymap, State}` lifecycle
  - `fn update_keymap(fd: RawFd, size: u32) -> Result<KeyboardState>`
  - `fn key_event(state: &mut KeyboardState, key: u32, key_state: wl_keyboard::KeyState) -> Option<KeySym>`
  - Return `xkbcommon::xkb::Keysym` — keybind module will map these

- [x] **5.3** Implement `libimgvwr::wayland` (main `WaylandState`):
  - Connect to display, get registry, bind globals:
    - `wl_compositor`, `wl_shm`, `xdg_wm_base`, `wl_seat`
    - `zxdg_decoration_manager_v1` only when `#[cfg(feature = "decorations")]`
  - Create `wl_surface`, `xdg_surface`, `xdg_toplevel`
  - Handle `xdg_surface::configure` + `xdg_toplevel::configure` (resize, close)
  - Wire `wl_seat` → `wl_keyboard` (attach keyboard handler) + `wl_pointer` (mouse buttons + scroll + motion for zoom/pan)
  - `fn flush(&self)` + `fn dispatch(&mut self, timeout_ms: i32) -> Result<()>` — main loop primitives
  - On `xdg_toplevel::close_requested`: set a shutdown flag
  - No rendering in this module — just surface management and event collection

- [x] **5.4** `libimgvwr::keybinds`:
  - `Action` enum: `Quit`, `RotateLeft`, `RotateRight`
  - `fn keysym_from_str(s: &str) -> Result<Keysym, KeybindError>` — wraps `xkbcommon::xkb::keysym_from_name`; exported so `imgvwr::config` can use it to validate and resolve keybinds at startup
  - `KeybindMap { inner: HashMap<Keysym, Action> }` — constructed by `imgvwr::main` from already-resolved keysyms
  - `fn KeybindMap::new(quit: Keysym, rotate_left: Keysym, rotate_right: Keysym) -> KeybindMap`
  - `fn lookup(&self, sym: Keysym) -> Option<Action>`
  - Write `keybinds/tests.rs`: `keysym_from_str("q")` succeeds; `keysym_from_str("invalid_xyz")` errors; `KeybindMap::new(...)` + lookup round-trip.

**Verify**: `cargo build --workspace` compiles; no integration test for Wayland yet.

---

### Phase 6 — Event loop and integration

- [x] **6.1** Implement `imgvwr::main` event loop:
  - Init: parse CLI, load config, create `Navigator`, load first image, init `ViewportState`, init `WaylandState`
  - Render loop skeleton:
    ```
    loop {
        wayland.dispatch(16)?;   // ~60 fps poll
        if wayland.needs_redraw() || viewport.dirty {
            let pixels = renderer::render(&image, &viewport, w, h, config.viewer.filter_method);
            wayland.commit_frame(&pixels, w, h)?;
            viewport.dirty = false;
        }
        if wayland.closed { break; }
    }
    ```
  - On keyboard event: look up `Action` via `KeybindMap`; execute: `Quit` → break, `RotateLeft/Right` → `viewport.rotate_left/right()` + set dirty flag
  - On scroll event: `viewport.zoom_by(delta * config.viewer.scale_step, config.viewer.min_scale, config.viewer.max_scale)` + dirty
  - On pointer press + motion: `viewport.pan(dx, dy)` + dirty
  - On left/right arrow key (hardcoded keysyms `XK_Left`, `XK_Right`): `navigator.prev()/next()`, load new image, `viewport.reset()`, dirty

- [x] **6.2** `wayland.commit_frame(&pixels, w, h)`:
  - Write `pixels` to `ShmPool` (resize pool if needed)
  - `wl_shm.create_pool` → `pool.create_buffer(w, h, stride, ARGB8888)` → `wl_surface.attach(buffer)` → `damage_buffer(0,0,w,h)` → `wl_surface.commit()`
  - Destroy previous `wl_buffer` after commit (or double-buffer)

- [x] **6.3** When `decorations` feature is enabled and `config.window.decorations = true`:
  - Set window title to `"{filename} — imgvwr"` via `xdg_toplevel.set_title`
  - Request server-side decorations via `zxdg_decoration_manager_v1`
  - When feature is disabled: `set_title` is never called; no `zxdg_decoration_manager_v1` binding

**Verify**: `cargo run -p imgvwr -- path/to/image.png` opens a window showing the image. Manual smoke test: zoom, pan, rotate, next/prev, quit.

---

### Phase 7 — Polish and config completion

- [x] **7.1** Antialiasing: `imgvwr::main` resolves effective `FilterMethod` before calling `renderer::render()` — when `config.window.antialiasing = false`, pass `FilterMethod::Nearest` unconditionally; otherwise pass `config.viewer.filter_method`. No change to `libimgvwr`. Add test in `imgvwr::config` or `imgvwr::main` tests.

- [x] **7.2** `min_scale` / `max_scale` clamping: already wired via scalar params in `viewport.zoom_by(delta, min, max)` from Phase 3.1. Verify values flow from `config.viewer` in `imgvwr::main`.

- [x] **7.3** Initial scale: on image load, compute `fit_to_window` scale — `min(w/img_w, h/img_h)` as f32, clamped to `[min_scale, max_scale]`. Apply as initial `viewport.scale`. Center image.

- [x] **7.4** Window resize: on `xdg_toplevel::configure` with new `(w, h)`: update stored size, mark dirty. Re-render at new size.

- [x] **7.5** Graceful errors: if `loader::load()` fails for current path, log warning via `tracing::warn!` and skip to next image. If all paths fail, exit with error message.

**Verify**: all unit tests pass; manual test with various image sizes, window resizes.

---

### Phase 8 — GPU-accelerated rendering (`gpu` feature)

**Design summary**: Replace the entire render pipeline with a `wgpu`-backed GPU pipeline, gated behind an optional `gpu` Cargo feature. The CPU/SHM path remains the default and is never removed. When `gpu` is compiled in, it is **always used** — there is no per-image or per-filter CPU fallback. If GPU init fails at runtime, the application exits with an error (not a silent fallback). The rendered result is read back to CPU as `Vec<u8>` and written to the existing SHM buffer (no change to the Wayland commit path).

**Technology**: `wgpu` (MIT/Apache-2.0). Backend preference: **Vulkan first, GL (EGL) second**. If Vulkan is unavailable (driver missing, VM, etc.) wgpu falls back to its GL backend, which uses EGL under the hood — this requires zero extra code beyond a one-line backend mask. No OpenGL code is written directly; EGL is only involved if wgpu selects the GL backend. `pollster` (MIT) blocks on async wgpu init without a Tokio runtime.

**New files**:

- `libimgvwr/src/renderer/gpu.rs` — all GPU types and functions (compiled only under `gpu-vulkan` or `gpu-gles`)
- `libimgvwr/src/renderer/shaders/blit.wgsl` — full-screen quad vertex + fragment shader (sampler-based resize)
- `libimgvwr/src/renderer/shaders/lanczos3.wgsl` — compute shader: two-pass separable Lanczos3 convolution
- `libimgvwr/src/renderer/shaders/catmull_rom.wgsl` — compute shader: two-pass separable CatmullRom convolution

Each sub-step below ends in a verified state: `cargo build --workspace --features gpu-vulkan`, `cargo clippy --workspace --features gpu-vulkan -- -D warnings`, and `cargo fmt --check` all pass.

#### 8.1 — Feature scaffold

- [x] Add to `libimgvwr/Cargo.toml`:

  ```toml
  [features]
  gpu-vulkan = ["dep:wgpu", "dep:pollster", "wgpu/vulkan", "wgpu/wgsl"]
  gpu-gles   = ["dep:wgpu", "dep:pollster", "wgpu/gles",   "wgpu/wgsl"]

  [dependencies]
  wgpu     = { version = "29", optional = true, default-features = false }
  pollster = { version = "0.4", optional = true }
  ```

- Add to `imgvwr/Cargo.toml` `[features]`:
  ```toml
  gpu-vulkan = ["libimgvwr/gpu-vulkan"]
  gpu-gles   = ["libimgvwr/gpu-gles"]
  ```
- Create `libimgvwr/src/renderer/gpu.rs` as an empty stub.
- Reference it from `libimgvwr/src/renderer/mod.rs`: `#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))] pub mod gpu;`

**Verify**: `cargo build --workspace`, `cargo build --workspace --features gpu-vulkan`, `cargo build --workspace --features gpu-gles`, and `cargo build --workspace --no-default-features` all compile. ✓

#### 8.2 — GpuContext: device and queue initialization

- [x] Implement `libimgvwr::renderer::gpu::GpuContext`:

```rust
pub struct GpuContext {
    device: wgpu::Device,
    queue:  wgpu::Queue,
}

impl GpuContext {
    /// Returns `Err` if no suitable adapter is found; caller exits the process.
    pub fn new() -> Result<Self, GpuError>
}
```

- Use `pollster::block_on` to drive async init.
- `wgpu::Instance::new` with `backends: Backends::VULKAN | Backends::GL`. wgpu selects Vulkan if available; falls back to the GL backend (EGL-based) automatically — no extra code required.
- `instance.request_adapter` with `PowerPreference::HighPerformance`. Return `Err(GpuError::NoAdapter)` if `None`.
- `adapter.request_device` with default limits and no extra features.
- Log selected backend and adapter name at `tracing::info!` level.

`imgvwr::main`: call `GpuContext::new()` at startup; on `Err`, print the error and exit. No `Option` — when the `gpu` feature is compiled in, the GPU is non-negotiable.

**Verify**: build + clippy clean with `--features gpu-vulkan`, `--features gpu-gles`, and without either.

#### 8.3 — Image upload: DynamicImage → wgpu Texture

- [x] In `libimgvwr::renderer::gpu`, add:

```rust
fn upload_texture(device: &wgpu::Device, queue: &wgpu::Queue, img: &DynamicImage) -> wgpu::Texture
```

- `img.to_rgba8()` → raw bytes.
- Create `wgpu::Texture` (`Rgba8Unorm`, `TextureUsages::TEXTURE_BINDING | COPY_DST`).
- `queue.write_texture(...)` to copy pixel data.
- Return owned texture; caller holds it for the duration of the frame.

No tests (GPU hardware-dependent).

#### 8.4 — GPU resize: sampler-based blit for Nearest / Triangle / Gaussian

- [x] Implement:

```rust
fn resize_blit(
    ctx: &GpuContext,
    src: &wgpu::Texture,
    dst_w: u32, dst_h: u32,
    filter: FilterMethod,
) -> wgpu::Texture
```

- Create output texture at `(dst_w, dst_h)` with `TextureUsages::RENDER_ATTACHMENT | COPY_SRC | TEXTURE_BINDING`.
- Create `wgpu::Sampler`: `FilterMode::Linear` for `Triangle`/`Gaussian`, `FilterMode::Nearest` for `Nearest`.
- Load `blit.wgsl` via `include_str!`; compile render pipeline (full-screen quad, one draw call).
- Run a render pass into the output texture.
- `Lanczos3` and `CatmullRom`: route to sampler linear at this step (overridden in 8.5).

**Verify**: build + clippy clean.

#### 8.5 — High-quality kernels: Lanczos3 and CatmullRom compute shaders

- [x] Write two-pass separable convolution compute shaders:

- `lanczos3.wgsl`: kernel radius 3 (`a=3`); `sinc(x) * sinc(x/a)` weights; horizontal pass → intermediate texture, vertical pass → output texture.
- `catmull_rom.wgsl`: piecewise cubic kernel; same two-pass structure.
- Each shader receives a uniform buffer: `src_size: vec2<u32>`, `dst_size: vec2<u32>`.
- Embed via `include_str!` in `gpu.rs`.
- Dispatch compute pipelines via `wgpu::ComputePass`; output `TextureUsages::STORAGE_BINDING | COPY_SRC`.
- `resize_blit`: when `filter` is `Lanczos3` → dispatch `lanczos3.wgsl`; when `CatmullRom` → dispatch `catmull_rom.wgsl`.

**Verify**: build + clippy clean. Manual smoke test: `Lanczos3` on a large image is visually sharp and noticeably faster than the CPU path.

#### 8.6 — GPU rotation

- [x] Extend the output of 8.4/8.5 to apply rotation:

- Add a uniform `rotation: u32` (0/1/2/3 for 0°/90°/180°/270°) to the blit shader.
- For 90°/270°: swap `dst_w`/`dst_h` when creating the output texture.
- Rotation transform applied in the blit vertex shader via UV coordinate remap (no extra pass needed).
- When `viewport.rotation == 0`: skip rotation uniform update (no-op).

#### 8.7 — Readback: GPU Texture → Vec\<u8\> (ARGB)

- [x] Implement:

```rust
fn readback(ctx: &GpuContext, tex: &wgpu::Texture, w: u32, h: u32) -> Vec<u8>
```

- Create `wgpu::Buffer` (`BufferUsages::COPY_DST | MAP_READ`), size `w * h * 4`.
- Encode `copy_texture_to_buffer`; submit; `device.poll(Maintain::Wait)`.
- Map buffer, read bytes slice, unmap.
- Convert RGBA → ARGB (identical to CPU path; reuse the same byte-swap logic).
- Return `Vec<u8>` directly passable to `wayland.commit_frame()`.

#### 8.8 — Integration: dispatch CPU vs GPU in renderer

- [x] Change `libimgvwr::renderer::render` signature:

```rust
pub fn render(
    src: &DynamicImage,
    viewport: &ViewportState,
    dst_w: u32,
    dst_h: u32,
    filter: FilterMethod,
    #[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))] gpu: &GpuContext,   // required, not Option
) -> Vec<u8>
```

- Without `gpu` feature: existing CPU imageops path, signature unchanged.
- With `gpu` feature: always routes through upload → resize (8.4/8.5 dispatch) → rotate (8.6) → readback (8.7). No CPU imageops call anywhere in this branch.
- `imgvwr::main`: passes `&gpu_context` (initialized once at startup) on every render call.
- Update existing renderer tests: under `#[cfg(not(any(feature = "gpu-vulkan", feature = "gpu-gles")))]` they test the CPU path unchanged; add a separate `#[cfg(any(feature = "gpu-vulkan", feature = "gpu-gles"))]` test block that constructs a `GpuContext` (skipped in CI without GPU via `#[ignore]` or env check).

**Verify**: `cargo test --workspace` passes. `cargo build --workspace --features gpu-vulkan` compiles. Manual test: `cargo run -p imgvwr --features gpu-vulkan -- image.png` is smooth for all filter methods and all image sizes.

#### 8.9 — CI additions

- [x] GPU feature matrix: `--all-features` in `build.yml` and `fmt-clippy.yml` already covers both `gpu-vulkan` and `gpu-gles`; no Mesa packages required at compile time (wgpu uses dlopen).
- Install Mesa Vulkan software rasterizer in CI system deps: `mesa-vulkan-drivers` (Debian/Ubuntu) or `vulkan-swrast` (Arch); install Mesa GLES for the `gpu-gles` entry.
- Set env in the `gpu-vulkan` matrix entry: `WGPU_BACKEND=vulkan`, `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json` (lavapipe).
- `fmt-clippy.yml`: add `--features gpu-vulkan` to clippy matrix.

**Verify**: CI green for all feature combinations including `--features gpu-vulkan` and `--features gpu-gles`.

---

### Phase 9 — Optional format features (non-default)

Each sub-step is independent; do them in any order.

- [x] **9.1** `jpeg` feature: add `libimgvwr/tests/fixtures/4x4.jpg`; verify `cargo build --features jpeg` works; test `loader` with `4x4.jpg` when feature is on.
- [x] **9.2** `webp` feature: add `libimgvwr/tests/fixtures/4x4.webp`; same pattern.
- [x] **9.3** `avif` feature: add `libimgvwr/tests/fixtures/4x4.avif`; test `4x4.avif`. Note: `image/avif` is encode-only; decoding uses `image/avif-native` (pure-Rust `dav1d` — no system `libavif` required). Feature updated in `libimgvwr/Cargo.toml` accordingly.
- [x] **9.3a** `jxl` feature: add `libimgvwr/tests/fixtures/4x4.jxl`; use `jxl` crate 0.4 (the `jxl-rs` pure-Rust decoder from the libjxl project) directly — image-rs has no jxl decoder. Feature `jxl = ["dep:jxl"]` in `libimgvwr`; loader detects `.jxl` extension and routes to a dedicated `load_jxl()` path using the typestate `JxlDecoder` API with `JxlPixelFormat::rgba8(0)` output.
- [x] **9.4** Verify `--no-default-features` compiles (empty format support — `UnsupportedFormat` for all paths).
- [x] **9.5** Verify `--all-features` compiles and tests pass.
- [x] **9.6** animated `gif` playback support. `gif = ["image/gif"]` feature in both crates; `load_gif_frames()` in `libimgvwr::loader` decodes all frames with per-frame `Duration`; `app.rs` uses an `ImageHolder` enum (`Static` / `Animated`) with a `tick()` method that advances frames at their natural delay and returns `true` when a redraw is needed. Single-frame GIFs fall back to `Static`. Fixture: `tests/fixtures/4x4_anim.gif` (2 frames).
- [x] **9.7** animated `avif` playback support. `avif-anim = ["dep:dav1d", "dep:mp4parse", "dep:libc"]` — separate from the static `avif` feature. `load_avif_anim_frames()` parses the ISOBMFF container with `mp4parse` (already in tree via `avif`), builds a per-frame sample table via `mp4parse::unstable::create_sample_table`, prepends the AV1 Sequence Header from the `av1C` box to each frame's OBU data, decodes with `dav1d` (also already in tree), and converts YUV (I400/I420/I422/I444) → RGBA using BT.709 coefficients. `TrackType::Picture` is used (AVIF sequences use `pict`, not `vide`). Static AVIF falls back to `loader::load()`. `GifFrames` renamed `AnimFrames` for both formats. The `dispatch()` poll path and `ImageHolder::Animated` are now gated on `any(feature = "gif", feature = "avif-anim")`. `libc` dep stays optional, enabled by either animation feature. Fixture: `tests/fixtures/4x4_anim.avif` (4 frames, generated via ffmpeg libaom-av1).
- [x] **9.8** animated `jxl` playback support. `jxl-anim = ["jxl"]` feature in both crates; `load_jxl_anim_frames()` in `libimgvwr::loader` parses animation header, decodes all frames with per-frame `Duration` (from `VisibleFrameInfo::duration_ms`), dynamically sets `JxlPixelFormat` with `extra_channel_format: vec![None; num_extra]` to fold extra channels into RGBA output. `app.rs` routes `.jxl` extension through `load_jxl_anim_frames()` first, falling back to static `loader::load()` on error. Fixture: `tests/fixtures/4x4_anim.jxl` (2-frame RGBA, created via `cjxl` from APNG).
- [x] **9.9** animated `webp` playback support. `webp-anim = ["webp"]` feature in both crates; `load_webp_anim_frames()` in `libimgvwr::loader` uses `image::codecs::webp::WebPDecoder` (which implements `AnimationDecoder`) — same pattern as GIF. `app.rs` routes `.webp` through `load_webp_anim_frames()` first, falling back to static `loader::load()` on error. Fixture: `tests/fixtures/4x4_anim.webp` (2-frame RGBA, generated via Pillow).
- [x] **9.10** animated `apng` playback support. `apng = ["png", "dep:libc"]` feature in both crates; `load_apng_frames()` in `libimgvwr::loader` uses `PngDecoder::is_apng()` to gate on animated PNGs, then `PngDecoder::apng()` to obtain an `ApngDecoder` that implements `AnimationDecoder`. `app.rs` routes `.png` through `load_apng_frames()` first, falling back to static `loader::load()` for plain PNGs. Fixture: `tests/fixtures/4x4_anim.png` (2-frame RGBA, generated via Pillow).

---

### Phase 10 — DMA-BUF zero-copy (`dmabuf` feature) ✓

**Implemented.** Eliminates the GPU→CPU PCIe readback by using a wgpu swapchain backed by `VK_KHR_wayland_surface`. The compositor receives swapchain images as DMA-BUFs directly via the Vulkan WSI layer — no application-level `zwp_linux_dmabuf_v1` protocol code required.

**New feature**: `dmabuf` (implies `gpu-vulkan`).

**Design**:
- `WaylandContext::display_ptr()` / `surface_ptr()` — expose raw `wl_display*` / `wl_surface*` for wgpu surface creation (requires `wayland-client/system`).
- `GpuContext::new_with_surface(display_ptr, surface_ptr, w, h)` — creates a `wgpu::Surface<'static>` from the existing Wayland handles; selects a Vulkan adapter compatible with the surface; configures the swapchain.
- `GpuContext::configure_surface(w, h)` — reconfigures the swapchain on window resize.
- `GpuContext::render_and_present(src, viewport, dst_w, dst_h, filter)` — runs the same GPU resize+rotate pipeline as Phase 8 but presents directly to the swapchain via `frame.present()` (no `readback()`, no SHM). Uses `set_viewport` + `set_scissor_rect` to blit the visible region into the correct position on the swapchain texture.
- `imgvwr::app::run()` — when `dmabuf` is active: initialises GPU after `WaylandContext::connect()`, calls `render_and_present` in the render loop, skips `commit_frame` entirely.

---

### Phase 10b — Shell completions (`completions` feature) ✓

**Implemented.** `imgvwr --completions <shell>` prints a completion script to stdout and exits.

**New feature**: `completions` (`clap_complete` + `clap_complete_nushell`, both optional).

Supported shells: `bash`, `zsh`, `fish`, `nushell`.

**Design**:
- `imgvwr/src/completions/mod.rs` — `CompletionShell` enum (clap `ValueEnum`), `generate_completions(shell)`.
- `Cli::completions: Option<CompletionShell>` field under `#[cfg(feature = "completions")]`.
- `main()` handles the early-exit before config/logger init so no Wayland connection is required.

---

### Phase 11 — Legacy C/Meson tree removal

Execute only after v1.0 ships and the Rust implementation (through Phase 9) is complete. The C source tree is kept as reference throughout all prior phases; removing it prematurely would destroy the implementation reference.

Remove in a single commit. Do not touch `.claude/` or `docs/` or `examples/`.

**Delete entirely:**

- `src/`
- `test/`
- `subprojects/`
- `files/`
- `doc/`
- `contrib/`
- `.builds/`
- `meson.build`
- `meson_options.txt`
- `lsan.supp`
- `AUTHORS`
- `CONTRIBUTING`
- `PACKAGERS.md`

**Purge contents (keep file, empty body):**

- `README.md` — leave blank; real content added post-1.0
- `CHANGELOG` → rename to `CHANGELOG.md`, purge contents

**Update `LICENSE`** — keep original Harry Jeffery copyright line, append new line:

```
Copyright Harry Jeffery
Copyright Gigas002 (2026)
```

Full MIT body stays unchanged.

**Update `.gitignore`** — replace C/Meson-oriented content with tofi's Rust-oriented version:

```gitignore
# Prerequisites
*.d

# Object files
*.o
*.ko
*.obj
*.elf

# Linker output
*.ilk
*.map
*.exp

# Precompiled Headers
*.gch
*.pch

# Libraries
*.lib
*.a
*.la
*.lo

# Shared objects (inc. Windows DLLs)
*.dll
*.so
*.so.*
*.dylib

# Executables
*.exe
*.out
*.app
*.i*86
*.x86_64
*.hex

# Debug files
*.dSYM/
*.su
*.idb
*.pdb

# Kernel Module Compile Results
*.mod*
*.cmd
.tmp_versions/
modules.order
Module.symvers
Mkfile.old
dkms.conf

# Vim files
*.swp
*.taghl
tags

# Mac OS files
.DS_Store

# Project specific files
build/
.cache

# Rust (workspace)
/target
```

**Verify**: repo root contains only Rust workspace files + `docs/` + `examples/` + CI config. `typos` still passes. `cargo build --workspace --all-features` still green. `.typos.toml` `extend-exclude` no longer needs to cover `src/**`, `test/**`, etc.

---

## 7. Rendering decision rationale

The C imv uses **OpenGL + Cairo** because:

1. Cairo renders SVG (we drop SVG)
2. Cairo renders overlay text (we have no overlay)
3. OpenGL provides GPU-accelerated zoom/pan texture transforms

For `imgvwr` with image-rs:

- All image decoding is in Rust via image-rs — no per-format C libraries
- Transforms (scale, rotate) are via `imageops` — CPU, but fast enough for non-animated still images
- SHM blit to Wayland compositor is hardware-composited by the compositor itself
- No GPU shader code required

**When is GPU rendering needed?** When large images (>10 MP) cause noticeable lag during zoom, or when high-quality filter methods (Lanczos3, CatmullRom) are too slow on CPU. The optional `gpu` feature (Phase 8) addresses this via `wgpu`-backed GPU resize and rotation.

**No OpenGL, no hand-written EGL**: `wgpu` selects Vulkan first; if unavailable it falls back to its GL backend (which uses EGL internally). No OpenGL or EGL code is written in this codebase — that complexity lives inside `wgpu`.

---

## 8. Risk register

| Risk                                                                           | Likelihood | Impact              | Mitigation                                                                                         |
| ------------------------------------------------------------------------------ | ---------- | ------------------- | -------------------------------------------------------------------------------------------------- |
| image-rs `avif` requires system `libavif`; packagers may not have it           | Medium     | Low (it's optional) | Feature is off by default; CI documents the dep                                                    |
| SHM redraws too slow for large images (>10 MP)                                 | Medium     | Medium              | Cap resize to window size (never upscale beyond 2x); cache last scaled image if viewport unchanged |
| `xdg_decoration` not supported by compositor                                   | Low        | Low                 | Fallback: no decorations (CSD); feature disabled by default                                        |
| image-rs doesn't expose all `FilterType` variants behind `webp`/`avif` feature | Low        | Low                 | `FilterType` is in `imageops`, always available regardless of format features                      |
| Wayland protocol version mismatch (xdg-decoration)                             | Low        | Low                 | Graceful check: skip if global not advertised                                                      |

---

## 9. Definition of done (v1.0)

- [ ] Opens PNG images; shows them in a Wayland window
- [ ] Next/prev navigation within directory works (arrow keys)
- [ ] Zoom via scroll wheel, pan via click-drag; image can be dragged fully off-screen
- [ ] Rotate left/right via configurable keybinds
- [ ] `--config` flag works; TOML config parsed; defaults apply for missing keys
- [ ] `cargo test --workspace` green
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo build --workspace --no-default-features` compiles
- [ ] `cargo build --workspace --all-features` compiles
- [ ] CI green on push

---

## 10. Document maintenance

Update this file when:

- A policy decision in §1–§4 changes (update the section + §10 revision history)
- A phase step is added, removed, or reordered
- A dependency choice changes

### Revision history

| Date       | Change                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-04-17 | Initial plan created                                                                                                                                                                                                                                                                                                                                                                                                                |
| 2026-04-17 | Config struct moved entirely to `imgvwr`; `libimgvwr` has no config module. `FilterMethod` lives in `libimgvwr::renderer` as an API type. `viewport::zoom_by` takes scalar min/max params. `KeybindMap::new` takes resolved keysyms; `keysym_from_str` exported for `imgvwr` to resolve at startup.                                                                                                                                 |
| 2026-04-17 | Phase 0 expanded with full CI detail (7 workflows + dependabot, `.typos.toml`, `deny.toml`). Legacy C/Meson cleanup moved to Phase 10 — must execute last, after v1.0, to preserve C reference tree during implementation.                                                                                                                                                                                                          |
| 2026-04-18 | New Phase 8 inserted: GPU-accelerated rendering via optional `gpu` feature (`wgpu` 29 + `pollster` 0.4). Backend: Vulkan preferred, GL/EGL fallback (one-line mask, no hand-written EGL). When `gpu` is compiled in, GPU is mandatory for all rendering — no per-image CPU fallback. Former Phase 8 (optional formats) → Phase 9; former Phase 9 (future) → Phase 10; former Phase 10 (C removal) → Phase 11. §1.1, §4, §7 updated. |
| 2026-04-19 | Phase 10 implemented: `dmabuf` feature adds wgpu swapchain path (`VK_KHR_wayland_surface`) to eliminate PCIe readback. `wayland-client/system` feature added. `GpuContext::new_with_surface`, `configure_surface`, `render_and_present` added. `gpu_render_inner` extracted as shared helper. |
| 2026-04-19 | Phase 10b implemented: `completions` feature adds `imgvwr --completions <bash\|zsh\|fish\|nushell>` via `clap_complete` + `clap_complete_nushell`. |
