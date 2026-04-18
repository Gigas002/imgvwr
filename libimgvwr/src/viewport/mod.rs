//! Viewer-side transform state: scale, pan offset, and rotation.
//!
//! [`ViewportState`] is pure data with no I/O or rendering. All mutating
//! methods take explicit min/max parameters so the caller (the binary crate)
//! drives policy from config values without `libimgvwr` knowing about config.

#[cfg(test)]
mod tests;

/// The current pan, zoom, and rotation state for the displayed image.
///
/// Rotation is restricted to multiples of 90°, stored as `0`, `90`, `180`,
/// or `270`. Offset is unconstrained — the image may be panned fully
/// off-screen.
pub struct ViewportState {
    /// Current zoom factor. `1.0` means one image pixel per display pixel.
    pub scale: f32,
    /// Pixel offset from the centred position, `(x, y)`.
    pub offset: (f32, f32),
    /// Clockwise rotation in degrees: `0`, `90`, `180`, or `270`.
    pub rotation: u16,
}

impl Default for ViewportState {
    fn default() -> Self {
        ViewportState {
            scale: 1.0,
            offset: (0.0, 0.0),
            rotation: 0,
        }
    }
}

impl ViewportState {
    /// Adjust zoom by `delta`, clamping to `[min_scale, max_scale]`.
    ///
    /// Uses multiplicative scaling (`scale *= 1 + delta`) so that each step is
    /// a constant *percentage* of the current scale, giving perceptually uniform
    /// zoom at any magnification level. `delta = 0.08` always means ±8 %
    /// regardless of whether the image is zoomed in to 10× or out to 0.1×.
    pub fn zoom_by(&mut self, delta: f32, min_scale: f32, max_scale: f32) {
        self.scale = (self.scale * (1.0 + delta)).clamp(min_scale, max_scale);
    }

    /// Zoom around `cursor` (surface coordinates) so the image point under the
    /// pointer stays fixed. `window` is `(width, height)` in surface pixels.
    pub fn zoom_by_at(
        &mut self,
        delta: f32,
        min_scale: f32,
        max_scale: f32,
        cursor: (f32, f32),
        window: (u32, u32),
    ) {
        let old_scale = self.scale;
        self.zoom_by(delta, min_scale, max_scale);
        let ratio = self.scale / old_scale;
        // The image point under the cursor is at (cursor - window_center - offset) in image
        // space. After rescaling, adjust offset so that point stays under the cursor.
        self.offset.0 += (cursor.0 - window.0 as f32 / 2.0 - self.offset.0) * (1.0 - ratio);
        self.offset.1 += (cursor.1 - window.1 as f32 / 2.0 - self.offset.1) * (1.0 - ratio);
    }

    /// Rotate 90° counter-clockwise.
    pub fn rotate_left(&mut self) {
        self.rotation = (self.rotation + 270) % 360;
    }

    /// Rotate 90° clockwise.
    pub fn rotate_right(&mut self) {
        self.rotation = (self.rotation + 90) % 360;
    }

    /// Translate the image by `(dx, dy)` pixels. No clamping — the image can
    /// be dragged fully outside the window area.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.offset.0 += dx;
        self.offset.1 += dy;
    }

    /// Reset to the default state: scale `1.0`, zero offset, no rotation.
    pub fn reset(&mut self) {
        self.scale = 1.0;
        self.offset = (0.0, 0.0);
        self.rotation = 0;
    }
}
