# Audit

A whole-application audit of imgvwr and libimgvwr (2026-11-17) in six parts:
performance (§2), bugs (§3), security and robustness (§4), UI usability and
design (§5), and code quality, tests and project hygiene (§6). §7 lists what
was left open, and §8 is the checklist to re-run the audit.

Each finding says what was found, how serious it is, and what was done:
**fixed** (with the commit), **accepted** (with the reason), or **OK** (checked,
nothing to do). Every fix is its own commit, with a regression test where one
can be written.

Severity: **high** (data loss, a crash or hang in normal use, a stall the user
notices), **medium** (wrong behaviour in a reachable case, a crash on hostile
input), **low** (hardening, polish, or needs an unusual setup).

## 1. Method

Every source file was read in full. Each finding below was then reproduced
with the release build in a headless sway (wlroots headless backend,
1920×1080 at 144 Hz) rendering on the same GPU that drives a laptop panel,
so nothing touched the desktop session. Input came from a small
`zwlr_virtual_pointer_v1` / `zwp_virtual_keyboard_v1` client fed with scripted
moves, wheel notches, drags and key presses; frames were read back with
`grim`. Timings come from the per-frame debug log added for this audit
(`RUST_LOG=imgvwr=debug` prints `frame ms=… scale=… offset=…`), from
`/proc/<pid>/stat` (CPU time) and `/proc/<pid>/status` (RSS).

Test machine: hybrid laptop, AMD iGPU (drives the panel) plus an NVIDIA
RTX 3060 that runtime-suspends when idle. Test images: a 6000×4000 photo
(24 MP), a 12000×8000 image (96 MP, over vello's 8192 px texture limit),
odd-sized crops, a 20 fps GIF (small and 1920×1080), truncated files, and a
2 MB GIF of forty 8000×8000 frames.

Builds checked: default (CPU renderer, SHM), `gpu-vulkan` without `dmabuf`
(GPU render, SHM read-back), and `--all-features` (dmabuf swapchain, the
AUR package's configuration). Static checks: `cargo clippy` at the three gate
levels, `cargo deny check`, `typos`, `cargo llvm-cov`.

## 2. Performance

The reported problem: "sometimes, only sometimes, dragging a zoomed image
stutters", seen on a desktop with one GPU (RX 9070 XT). It was not reproduced
and its cause is unknown (first row); the other rows are what the audit found
and fixed along the way.

| Area                         | Finding                                                                                                                                                                                                                                                                                                                                  | Status                 |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------- |
| Drag stutter (reported)      | Not reproduced, cause unknown. It was seen on a single-GPU desktop (RX 9070 XT). In the test runs, zoomed drags stayed well inside one 144 Hz frame on every build after the fixes in this table (p95 0.6 ms on the photo). The one stall found is specific to hybrid laptops: there wgpu picks the discrete GPU, which runtime-suspends when idle, and the first frame after it wakes took 713 ms. That cannot explain a single-GPU machine, so imgvwr's adapter choice is unchanged; `WGPU_POWER_PREF=low` avoids that stall on laptops. Every frame's time is now logged at debug level (1e1a032) to diagnose the next occurrence. | Open (§7)              |
| Idle wake-ups                | Builds with any animated format woke every 16 ms even for a still image. The loop now sleeps until input, or until the next animation frame is due (and the animation clock no longer drifts).                                                                                                                                          | Fixed, medium (5433bdc) |
| CPU renderer zoom            | The CPU path resized the whole image to the zoomed size on every frame, pan steps included: 3.5 s and 3.3 GB per frame at 2.27× on the photo, unbounded up to `max_scale`. Only the visible region is now resampled, bit-identical to the old output (tested against the old algorithm across scales, rotations, offsets, filters).        | Fixed, high (a6ad41f)  |
| GPU mip levels               | Zoomed-out views draw from half-size copies built with `imageops::resize` and the configured filter: 200 ms per level on the photo, 1.26 s on the 96 MP image, all before the first frame. A 2×2 box average (nearest keeps a corner pixel) is 7–11× faster with no visible difference.                                                 | Fixed, medium (bcaa5de) |
| SHM frame pacing             | The SHM path drew as fast as input or animation asked, visible or not, and created a `wl_shm_pool` per frame. It now waits for the compositor's frame callback.                                                                                                                                                                          | Fixed, medium (84c67ed) |
| Animated sources (GPU)       | Every frame change uploads a new texture to vello's atlas. Checked for growth: RSS stays at 551 MB over a minute of a 1080p 20 fps GIF; frames take 3 ms.                                                                                                                                                                                 | OK                     |
| Images over 8192 px          | Zoomed in, the view is a crop that becomes a new GPU image on every drag frame. 2.0 ms p50, 3.2 ms p95, 11 ms worst on the 96 MP image: inside a 144 Hz frame except the worst case.                                                                                                                                                      | Accepted               |
| Start-up with a sleeping dGPU | On hybrid laptops imgvwr renders on the discrete GPU, so a launch while it sleeps waits for it to wake: adapter selection takes 1.6 s instead of 0.1 s. | Accepted               |
| Memory                       | RSS on the GPU path is 260 MB for a small image and 436 MB for the photo, mostly the Vulkan driver, vello's pipelines and the decoded image plus its mip chain.                                                                                                                                                                           | Accepted               |
| Image loading                | Images decode on the UI thread; the photo takes ~21 ms, the 96 MP PNG longer. Navigation blocks for that long; there is no prefetch.                                                                                                                                                                                                      | Accepted (§7)          |

### Numbers

All at 1920×1080, release builds; GPU builds were run on the integrated GPU
(`WGPU_POWER_PREF=low`).

| Measurement                                                     | Before                       | After              |
| --------------------------------------------------------------- | ---------------------------- | ------------------ |
| Drag frame p95, zoomed photo (all features)                     | –                            | 0.6 ms             |
| Drag frame p95, zoomed 12000×8000 (all features)                | –                            | 3.2 ms             |
| First frame, 6000×4000 (all features)                           | 260 ms                       | 63 ms              |
| First frame, 12000×8000 (all features)                          | 1028 ms                      | 195 ms             |
| Launch to first frame, small / photo (dGPU awake)               | –                            | 123 / 196 ms       |
| Drag frame, zoomed photo, CPU renderer (default build)          | 3.5 s                        | 26 ms              |
| Peak RSS zooming the photo, CPU renderer                        | 3.26 GB                      | 407 MB             |
| Wake-ups per 10 s, idle still image (animated formats built in) | 621                          | 0                  |
| Wake-ups per 10 s, 20 fps GIF                                   | 750                          | 400                |
| CPU per 10 s, 20 fps GIF on a hidden workspace (SHM path)       | 2340 ms                      | 0 ms               |
| CPU for a scripted zoom + 1000-step drag (SHM read-back path)   | 1140 ms                      | 640 ms             |
| 2 MB GIF bomb (forty 8000×8000 frames)                          | abort (9.5 GiB requested)    | first frame, 1.4 GB peak |

## 3. Bugs

| Area                      | Finding                                                                                                                                                                                                                                         | Status                  |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| Black window              | Without an animated format compiled in (the default build) the loop blocked on input before drawing the first frame. An unmapped window gets no input, so it stayed black forever.                                                               | Fixed, high (d5b8967)   |
| Delete removes wrong file | When the next image failed to load the old one stayed on screen but the cursor had moved to the broken file, so Delete permanently removed a file the user never saw. Navigation now skips unloadable images and the cursor always names the shown one. | Fixed, high (818b3db)   |
| Bare file names           | `imgvwr photo.png` failed with "No such file or directory" (`read_dir("")`).                                                                                                                                                                    | Fixed, high (846525d)   |
| Multiple paths            | Only the first path was used: `imgvwr a.png b.png` browsed a.png's folder. An explicit file with an unknown extension was replaced by another image of its folder.                                                                              | Fixed, medium (846525d) |
| Fit to window             | The first image was fitted to the 800×600 placeholder before the compositor's configure, so it opened at a fraction of a tiled window; resizing or rotating never refitted. The view now follows the window until the user zooms or pans.       | Fixed, medium (5f8ab18) |
| Zoom step                 | Each axis event was divided by 10 though compositors send 15 per notch: one notch zoomed 12 % instead of the configured 8 %. Scrolling is now summed per pointer frame using `axis_value120`.                                                     | Fixed, medium (c86796f) |
| Pointer                   | `wl_pointer.enter` was ignored, so the first zoom after opening centred on (0,0); any button panned, and a press the compositor took away left the image glued to the pointer.                                                                     | Fixed, medium (ed71226) |
| Config crashes            | `min_scale > max_scale` or a NaN scale panicked in `f32::clamp` on the first frame; a non-positive scale or a `scale_step` outside (0, 1) broke zooming. They now fall back to the default with a warning.                                         | Fixed, medium (c707535) |
| Odd mip sizes             | `level_size` floored odd sides while the levels round up, so the transform was off by a pixel and a debug assertion fired.                                                                                                                       | Fixed, low (bcaa5de)    |
| SHM buffer reuse          | Every frame was written into the buffer the compositor might still be reading (the protocol forbids it; tearing on compositors that read late). Two buffers now alternate and are reused only after `wl_buffer.release`.                          | Fixed, medium (84c67ed) |
| Background colour         | The SHM and read-back paths leave the area around the image transparent, the dmabuf swapchain clears it to black. The doc comment said black for all paths; it now says what happens.                                                            | Fixed (doc), accepted (801ba00b6887d215442b62fb4683626b6b4d31dd) |

## 4. Security and robustness

imgvwr reads image files the user opens, which may come from anywhere, and
its own config file. It deletes files only on an explicit key press.

| Area                      | Finding                                                                                                                                                                                                                     | Status                  |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| Decompression bombs       | Animations kept every frame decoded with no limit: a 2 MB GIF requested 9.5 GiB and the process aborted (or pushed the desktop into swap). Frames now stop at 1 GiB and the first frame is shown, with a warning.             | Fixed, medium (38e659e) |
| JPEG XL size              | The JXL loaders allocated `width × height × 4` straight from the header, bypassing the 512 MiB limit `image` applies to other formats. They now refuse larger images before allocating.                                      | Fixed, medium (38e659e) |
| Other formats             | PNG, JPEG, WebP, GIF and APNG go through `image` with its default limits; truncated and corrupt files are reported and skipped (§3).                                                                                        | OK                      |
| Delete                    | `remove_file` on the shown path. It removes a symlink, not its target, and never a file the user did not see (§3). It is permanent; there is no trash.                                                                      | Accepted (§7)           |
| `unsafe`                  | Four blocks: two `mmap`s of our own memfd, the xkbcommon keymap fd from the compositor, and the wgpu surface from the Wayland handles. Each is required by the API and holds its invariant. Test-only `set_var` calls race other tests' environment reads in theory; none has been seen. | OK                      |
| File names in titles/logs | Shown as the window title and in logs; the compositor and terminal treat them as text. Errors name the file (876804a).                                                                                                      | OK                      |
| Config                    | The user's own file. Errors and unknown keys are now reported instead of silently ignored; an explicit `--config` that fails is an error.                                                                                   | Fixed, low (c707535)    |
| Dependencies              | `cargo deny check`: advisories, bans, licences and sources OK. CI ran only the licence check; it now runs all four.                                                                                                         | Fixed (18dcd2c)         |

## 5. UI usability and design

| Area                | Finding                                                                                                                                                                                       | Status            |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------- |
| Window title        | Set only with the `decorations` feature enabled, so most builds showed an untitled window in bars and switchers. Now always `name — imgvwr`.                                                    | Fixed (4dd9454)   |
| Error messages      | Failures printed Rust debug output without the path (`Error: Os { code: 2, … }`). Now `imgvwr: /path/x.png: No such file or directory`, exit status 1.                                          | Fixed (876804a)   |
| Silent config       | Warnings were emitted before the logger existed, so typos, type errors and a missing `--config` file were silently ignored. They are now collected and logged once logging is up.               | Fixed (c707535)   |
| `[` / `]` bindings  | The documented `"["` and `"]"` key names only worked because the fallback happened to be the same key; single characters now resolve.                                                         | Fixed (c707535)   |
| Documentation       | The README did not mention Left/Right, the mouse, directories or the config locations; the example config named the wrong default filter and `scale_step`.                                    | Fixed (9c4cff1)   |
| Key repeat          | Holding Left/Right moves one image; there is no key repeat.                                                                                                                                   | Accepted (§7)     |
| Sort order          | Images are sorted by byte order (`img10` before `img2`), not naturally.                                                                                                                        | Accepted (§7)     |
| Perf diagnostics    | `RUST_LOG=imgvwr=debug` now logs each frame's time, scale and offset, which is what a stutter report needs.                                                                                     | Added (1e1a032)   |

## 6. Code quality, tests and hygiene

| Area             | Finding                                                                                                                                                                 | Status          |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------- |
| Layout           | `app.rs` had no tests and broke the `mod.rs` + `tests.rs` rule of docs/ARCHITECTURE.md; it is now `app/mod.rs` with `app/tests.rs`.                                         | Fixed (5f8ab18) |
| Tests            | 96 → 136 tests. Line coverage (`cargo llvm-cov --workspace --all-features`) 40.6 % → 52.2 %. The rest is mostly the Wayland event loop and GPU set-up, exercised by the headless runs above. | Improved        |
| Manifest warning | `homepage` repeated `repository`, which cargo warns about on every build.                                                                                                | Fixed (18dcd2c) |
| Dependencies     | All direct dependencies at their latest versions; vello 0.11 → 0.12 renders pixel-identical.                                                                             | Fixed (a83d822) |
| Gate             | fmt, typos, `cargo deny check`, clippy at the three feature levels, tests at the three levels and `cargo doc` pass on every commit of the audit.                         | OK              |

## 7. Left open

- **Delete is permanent.** Moving to the freedesktop trash would need a new
  dependency or a trash implementation; worth considering, and the README now
  says "permanently".
- **Loading on the UI thread, no prefetch.** Large images block input while
  they decode. A loader thread plus prefetch of the neighbours would hide it,
  at the cost of memory.
- **Natural sort and key repeat.** Small usability features.
- **The reported drag stutter** (§2) on a single-GPU desktop is unexplained.
  When it happens again, a `RUST_LOG=imgvwr=debug` log shows whether the
  frames around it were slow to render (high `ms=`) or late to start (a gap
  between frames with normal `ms=`, pointing at event delivery or
  presentation).
- **Hybrid laptops** render on the discrete GPU, which wakes slowly from
  runtime suspend: 1.6 s at start-up and 713 ms for the first frame after an
  idle pause. `WGPU_POWER_PREF=low` avoids both.
- **Background colour** differs between the dmabuf path (black) and the SHM
  paths (transparent) (§3). Pick one if consistency matters.

## 8. Re-running the audit

1. Gate: `cargo fmt --all --check`, `typos`, `cargo deny check`, clippy and
   tests at `--no-default-features`, default and `--all-features`,
   `cargo doc --no-deps`.
2. Coverage: `cargo llvm-cov --workspace --all-features --summary-only`.
3. Run a headless compositor (on a hybrid machine, pass `WGPU_POWER_PREF=low`
   so the viewer renders on the same GPU) (`WLR_BACKENDS=headless sway`, pointing
   `WLR_RENDER_DRM_DEVICE` at the integrated GPU) and drive the viewer with a
   virtual pointer and keyboard. For each build (default, `gpu-vulkan`,
   `--all-features`):
   - open a 24 MP photo and a > 8192 px image; zoom 10–15 notches, pause 20 s,
     drag; check `frame ms` p95 stays under one refresh interval and the first
     drag frame is not slow;
   - check the first frame appears without input, fitted to the window;
   - leave a GIF playing on a hidden workspace and check CPU time is ~0;
   - open a truncated image between two good ones, navigate, press Delete and
     check the deleted file is the one shown;
   - open a GIF whose frames decode to several GiB and check the viewer
     stays up.
4. Check RSS.
