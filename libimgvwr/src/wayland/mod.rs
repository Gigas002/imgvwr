//! Wayland client integration: surface management, SHM buffers, and input.
//!
//! [`WaylandContext`] is the public entry point. It wraps [`WaylandState`]
//! (the dispatch target) together with the [`EventQueue`] so that
//! [`WaylandContext::dispatch`] can borrow both fields without conflict.
//!
//! Submodules:
//! - [`shm`]: SHM pool backed by a `memfd` (Phase 5.1)
//! - [`keyboard`]: xkbcommon keymap and key-event handling (Phase 5.2)

pub mod keyboard;
pub mod shm;

use std::io;

#[cfg(feature = "decorations")]
use tracing::warn;
use tracing::{debug, info};

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    },
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use xkbcommon::xkb::Keysym;

#[cfg(feature = "decorations")]
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};

use crate::wayland::{
    keyboard::{KeyboardState, key_event, update_keymap},
    shm::ShmPool,
};

// ── Input events ────────────────────────────────────────────────────────────

/// Input events collected during dispatch and consumed by the event loop.
pub enum InputEvent {
    /// A key was pressed; carries the resolved XKB keysym.
    Key(Keysym),
    /// Vertical scroll wheel tick. Positive = scroll down.
    /// Carries the pointer position at the time of the event (surface coords).
    Scroll { delta: f32, cursor: (f32, f32) },
    /// Left mouse button pressed (`true`) or released (`false`).
    PointerButton { pressed: bool },
    /// Mouse moved while the button was held; delta in surface pixels.
    PointerMotion { dx: f32, dy: f32 },
}

// ── WaylandState ────────────────────────────────────────────────────────────

/// Wayland protocol objects and collected event state.
///
/// This struct is the dispatch target for [`EventQueue`]; it must not contain
/// the queue itself. See [`WaylandContext`] for the combined public type.
pub struct WaylandState {
    qh: QueueHandle<WaylandState>,

    // Globals bound from the registry
    compositor: Option<wl_compositor::WlCompositor>,
    wl_shm: Option<wl_shm::WlShm>,
    xdg_wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,

    #[cfg(feature = "decorations")]
    decoration_manager: Option<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1>,
    #[cfg(feature = "decorations")]
    toplevel_decoration: Option<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1>,

    // Surface objects created after the first roundtrip
    surface: Option<wl_surface::WlSurface>,
    xdg_surface: Option<xdg_surface::XdgSurface>,
    xdg_toplevel: Option<xdg_toplevel::XdgToplevel>,

    // Input objects and state
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_state: Option<KeyboardState>,
    pointer: Option<wl_pointer::WlPointer>,
    pointer_pos: (f64, f64),
    pointer_pressed: bool,

    /// Set to `true` when the compositor requests the window be closed.
    pub closed: bool,
    /// Set to `true` when a redraw is needed (configure, input, etc.).
    pub needs_redraw: bool,
    /// Current window size in pixels as reported by the compositor.
    pub window_size: (u32, u32),
    /// Events accumulated since the last drain by the event loop.
    pub pending_events: Vec<InputEvent>,
}

impl WaylandState {
    fn new(qh: QueueHandle<WaylandState>, initial_size: (u32, u32)) -> Self {
        WaylandState {
            qh,
            compositor: None,
            wl_shm: None,
            xdg_wm_base: None,
            seat: None,
            #[cfg(feature = "decorations")]
            decoration_manager: None,
            #[cfg(feature = "decorations")]
            toplevel_decoration: None,
            surface: None,
            xdg_surface: None,
            xdg_toplevel: None,
            keyboard: None,
            keyboard_state: None,
            pointer: None,
            pointer_pos: (0.0, 0.0),
            pointer_pressed: false,
            closed: false,
            needs_redraw: false,
            window_size: initial_size,
            pending_events: Vec::new(),
        }
    }

    /// Expose `wl_shm` for Phase 6 buffer creation.
    pub fn wl_shm(&self) -> Option<&wl_shm::WlShm> {
        self.wl_shm.as_ref()
    }

    /// Expose the `wl_surface` for attaching buffers.
    pub fn surface(&self) -> Option<&wl_surface::WlSurface> {
        self.surface.as_ref()
    }

    /// Expose `QueueHandle` for creating objects in Phase 6.
    pub fn qh(&self) -> &QueueHandle<WaylandState> {
        &self.qh
    }
}

// ── WaylandContext ───────────────────────────────────────────────────────────

/// Owns the Wayland connection, state, and event queue.
///
/// The [`EventQueue`] and [`WaylandState`] are separate fields so that
/// `dispatch` can borrow them disjointly without unsafe code.
pub struct WaylandContext {
    conn: Connection,
    /// Public Wayland state — surfaces, globals, pending events, flags.
    pub state: WaylandState,
    event_queue: EventQueue<WaylandState>,
    /// SHM pool backing the pixel buffer. Created on first commit; resized as needed.
    shm_pool: Option<ShmPool>,
    /// The `wl_buffer` attached in the previous frame. Destroyed before each new commit.
    prev_buffer: Option<wl_buffer::WlBuffer>,
}

impl WaylandContext {
    /// Connect to the Wayland compositor and initialise surfaces.
    ///
    /// Performs two roundtrips: one to enumerate globals, one to receive the
    /// initial `xdg_toplevel::configure`.
    pub fn connect(initial_size: (u32, u32), use_decorations: bool) -> io::Result<Self> {
        // Suppress unused-variable lint when the `decorations` feature is off.
        #[cfg(not(feature = "decorations"))]
        let _ = use_decorations;

        let conn = Connection::connect_to_env()
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e))?;
        let mut event_queue = conn.new_event_queue::<WaylandState>();
        let qh = event_queue.handle();

        let mut state = WaylandState::new(qh.clone(), initial_size);

        conn.display().get_registry(&qh, ());
        event_queue
            .roundtrip(&mut state)
            .map_err(io::Error::other)?;

        if state.compositor.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "wl_compositor not found",
            ));
        }
        if state.xdg_wm_base.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "xdg_wm_base not found",
            ));
        }

        let surface = state.compositor.as_ref().unwrap().create_surface(&qh, ());
        let xdg_surf = state
            .xdg_wm_base
            .as_ref()
            .unwrap()
            .get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surf.get_toplevel(&qh, ());
        toplevel.set_app_id("imgvwr".to_string());

        #[cfg(feature = "decorations")]
        if use_decorations {
            if let Some(mgr) = &state.decoration_manager {
                debug!("requesting server-side decorations");
                state.toplevel_decoration = Some(mgr.get_toplevel_decoration(&toplevel, &qh, ()));
            } else {
                warn!(
                    "compositor does not support zxdg_decoration_manager_v1; no server-side decorations"
                );
            }
        }

        surface.commit();

        state.surface = Some(surface);
        state.xdg_surface = Some(xdg_surf);
        state.xdg_toplevel = Some(toplevel);

        event_queue
            .roundtrip(&mut state)
            .map_err(io::Error::other)?;

        let (w, h) = state.window_size;
        info!(width = w, height = h, "connected to Wayland compositor");

        Ok(WaylandContext {
            conn,
            state,
            event_queue,
            shm_pool: None,
            prev_buffer: None,
        })
    }

    /// Write `pixels` (ARGB8888, `w × h × 4` bytes) into a Wayland SHM buffer
    /// and commit it to the surface.
    pub fn commit_frame(&mut self, pixels: &[u8], w: u32, h: u32) -> io::Result<()> {
        let size = (w * h * 4) as usize;

        // Create or grow the SHM backing store.
        match &self.shm_pool {
            None => self.shm_pool = Some(ShmPool::create(size)?),
            Some(p) if p.size < size => self.shm_pool.as_mut().unwrap().resize(size)?,
            _ => {}
        }

        self.shm_pool.as_mut().unwrap().as_mut_slice()[..size].copy_from_slice(pixels);

        // Build a wl_shm_pool + wl_buffer for this frame.
        let stride = w as i32 * 4;
        let wl_pool = {
            let fd = self.shm_pool.as_ref().unwrap().fd();
            let wl_shm = self
                .state
                .wl_shm()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "wl_shm not bound"))?;
            wl_shm.create_pool(fd, size as i32, self.state.qh(), ())
        };
        let buffer = wl_pool.create_buffer(
            0,
            w as i32,
            h as i32,
            stride,
            wl_shm::Format::Argb8888,
            self.state.qh(),
            (),
        );
        wl_pool.destroy();

        // Destroy the previous frame's buffer before the new one takes the surface slot.
        if let Some(prev) = self.prev_buffer.take() {
            prev.destroy();
        }

        let surface = self
            .state
            .surface()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "surface not available"))?;
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.commit();

        self.prev_buffer = Some(buffer);
        self.flush()
    }

    /// Flush the outgoing Wayland socket buffer.
    pub fn flush(&self) -> io::Result<()> {
        self.conn
            .flush()
            .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))
    }

    /// Dispatch pending Wayland events.
    ///
    /// Without any animated-format feature: uses `blocking_dispatch`, which
    /// parks the thread until a compositor event arrives — correct and idle for
    /// static images.
    ///
    /// With an animated-format feature (e.g. `gif`): uses a `poll` timeout so
    /// the animation loop can call `tick()` on a regular cadence even without
    /// user input.
    pub fn dispatch(&mut self, timeout_ms: i32) -> io::Result<()> {
        #[cfg(any(
            feature = "gif",
            feature = "avif-anim",
            feature = "jxl-anim",
            feature = "webp-anim",
            feature = "apng"
        ))]
        {
            use std::os::fd::AsFd;
            use std::os::unix::io::AsRawFd;

            self.flush()?;

            if let Some(guard) = self.event_queue.prepare_read() {
                let mut pfd = libc::pollfd {
                    fd: self.conn.as_fd().as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: &mut pfd is valid for the duration of the call.
                unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
                // Always attempt the read: WouldBlock means no data arrived
                // (spurious wakeup or data already drained), which is fine.
                match guard.read() {
                    Ok(_) => {}
                    Err(wayland_client::backend::WaylandError::Io(e))
                        if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => return Err(io::Error::other(e)),
                }
            }

            self.event_queue
                .dispatch_pending(&mut self.state)
                .map(|_| ())
                .map_err(io::Error::other)
        }

        #[cfg(not(any(
            feature = "gif",
            feature = "avif-anim",
            feature = "jxl-anim",
            feature = "webp-anim",
            feature = "apng"
        )))]
        {
            let _ = timeout_ms;
            self.event_queue
                .blocking_dispatch(&mut self.state)
                .map(|_| ())
                .map_err(io::Error::other)
        }
    }

    /// Raw `wl_display *` pointer for wgpu surface creation.
    ///
    /// SAFETY: The returned pointer is valid for the lifetime of this
    /// `WaylandContext`. Must not be used after `WaylandContext` is dropped.
    #[cfg(feature = "dmabuf")]
    pub fn display_ptr(&self) -> std::ptr::NonNull<std::ffi::c_void> {
        let ptr = self.conn.backend().display_ptr().cast::<std::ffi::c_void>();
        std::ptr::NonNull::new(ptr).expect("wl_display must not be null")
    }

    /// Raw `wl_surface *` pointer for wgpu surface creation.
    ///
    /// SAFETY: The returned pointer is valid for the lifetime of this
    /// `WaylandContext`. Must not be used after `WaylandContext` is dropped.
    #[cfg(feature = "dmabuf")]
    pub fn surface_ptr(&self) -> std::ptr::NonNull<std::ffi::c_void> {
        use wayland_client::Proxy;
        let ptr = self
            .state
            .surface
            .as_ref()
            .expect("wl_surface not yet created")
            .id()
            .as_ptr()
            .cast::<std::ffi::c_void>();
        std::ptr::NonNull::new(ptr).expect("wl_surface must not be null")
    }

    /// Set the XDG toplevel window title.
    ///
    /// Only available when the `decorations` feature is enabled. The title is
    /// buffered and sent on the next Wayland socket flush (i.e. the next
    /// `dispatch` call or `commit_frame`).
    #[cfg(feature = "decorations")]
    pub fn set_title(&mut self, title: &str) {
        if let Some(toplevel) = &self.state.xdg_toplevel {
            debug!(title, "setting window title");
            toplevel.set_title(title.to_string());
        } else {
            warn!("set_title called but xdg_toplevel is not yet initialised");
        }
    }
}

// ── Dispatch implementations ─────────────────────────────────────────────────

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_compositor" => {
                state.compositor = Some(registry.bind(name, version.min(6), qh, ()));
            }
            "wl_shm" => {
                state.wl_shm = Some(registry.bind(name, version.min(1), qh, ()));
            }
            "xdg_wm_base" => {
                state.xdg_wm_base = Some(registry.bind(name, version.min(5), qh, ()));
            }
            "wl_seat" => {
                state.seat = Some(registry.bind(name, version.min(9), qh, ()));
            }
            #[cfg(feature = "decorations")]
            "zxdg_decoration_manager_v1" => {
                info!("compositor supports server-side decorations");
                state.decoration_manager = Some(registry.bind(name, version.min(1), qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for WaylandState {
    fn event(
        _: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
            if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap {
                format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                fd,
                size,
            } => {
                if let Ok(ks) = update_keymap(fd, size) {
                    state.keyboard_state = Some(ks);
                }
            }
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => {
                let sym = if let Some(ks) = state.keyboard_state.as_mut() {
                    if let WEnum::Value(ks_val) = key_state {
                        key_event(ks, key, ks_val)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(sym) = sym {
                    state.pending_events.push(InputEvent::Key(sym));
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Button {
                state: WEnum::Value(btn),
                ..
            } => {
                let pressed = btn == wl_pointer::ButtonState::Pressed;
                state.pointer_pressed = pressed;
                state
                    .pending_events
                    .push(InputEvent::PointerButton { pressed });
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if state.pointer_pressed {
                    let dx = (surface_x - state.pointer_pos.0) as f32;
                    let dy = (surface_y - state.pointer_pos.1) as f32;
                    state
                        .pending_events
                        .push(InputEvent::PointerMotion { dx, dy });
                }
                state.pointer_pos = (surface_x, surface_y);
            }
            wl_pointer::Event::Axis {
                axis: WEnum::Value(wl_pointer::Axis::VerticalScroll),
                value,
                ..
            } => {
                // Normalise: raw axis value is ~10–15 units per wheel notch on most
                // compositors (wlroots/libinput default is 15). Dividing by 10.0 maps
                // one notch to ≈1.0, so `scale_step` in config means "zoom % per notch".
                let cursor = (state.pointer_pos.0 as f32, state.pointer_pos.1 as f32);
                state.pending_events.push(InputEvent::Scroll {
                    delta: -value as f32 / 10.0,
                    cursor,
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for WaylandState {
    fn event(
        state: &mut Self,
        xdg_surf: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surf.ack_configure(serial);
            if let Some(surface) = &state.surface {
                surface.commit();
            }
            state.needs_redraw = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                let w = if width == 0 {
                    state.window_size.0 as i32
                } else {
                    width
                };
                let h = if height == 0 {
                    state.window_size.1 as i32
                } else {
                    height
                };
                let new_size = (w as u32, h as u32);
                if new_size != state.window_size {
                    debug!(width = new_size.0, height = new_size.1, "window resized");
                    state.window_size = new_size;
                }
                state.needs_redraw = true;
            }
            xdg_toplevel::Event::Close => {
                info!("compositor requested window close");
                state.closed = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(feature = "decorations")]
impl Dispatch<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        _: zxdg_decoration_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(feature = "decorations")]
impl Dispatch<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
        _: zxdg_toplevel_decoration_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
