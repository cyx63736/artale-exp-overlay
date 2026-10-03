use crate::platform;
use crate::ui::Overlay;
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId, WindowLevel};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Overlay,
    Settings,
    Record,
    Select,
    Hint(u8),
}

#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    Wake,
}

pub struct Spec {
    pub kind: Kind,
    pub title: &'static str,
    pub pos: (i32, i32),
    pub size: (u32, u32),
    pub decorations: bool,
}

struct Win {
    kind: Kind,
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    ctx: egui::Context,
    state: egui_winit::State,
    rend: EguiSoftwareRender,
    hwnd: isize,
    repaint_at: Option<Instant>,
    shown: bool,
    last_size: PhysicalSize<u32>,
    at: ((i32, i32), (u32, u32)),
}

#[cfg(target_os = "macos")]
impl Drop for Win {
    fn drop(&mut self) {
        platform::restore_class(self.hwnd);
    }
}

pub struct Runner {
    pub app: Overlay,
    proxy: EventLoopProxy<UserEvent>,
    wins: HashMap<WindowId, Win>,
    sb: Option<softbuffer::Context<Rc<Window>>>,
    next_tick: Instant,
    overlay_rect: Option<(i32, i32, i32, i32)>,
}

pub fn run(make: impl FnOnce(EventLoopProxy<UserEvent>) -> Overlay) -> Result<(), String> {
    #[allow(unused_mut)]
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_activate_ignoring_other_apps(false);
        builder.with_activation_policy(winit::platform::macos::ActivationPolicy::Accessory);
    }
    let el = builder.build().map_err(|e| e.to_string())?;
    el.set_control_flow(ControlFlow::Wait);
    let proxy = el.create_proxy();
    let app = make(proxy.clone());
    let mut r = Runner { app, proxy, wins: HashMap::new(), sb: None, next_tick: Instant::now(), overlay_rect: None };
    el.run_app(&mut r).map_err(|e| e.to_string())
}

impl Runner {
    fn find(&self, kind: Kind) -> Option<WindowId> {
        self.wins.iter().find(|(_, w)| w.kind == kind).map(|(id, _)| *id)
    }

    fn create(&mut self, el: &ActiveEventLoop, s: Spec) {
        let mut attr = Window::default_attributes()
            .with_title(s.title)
            .with_decorations(s.decorations)
            .with_resizable(false)
            .with_enabled_buttons(winit::window::WindowButtons::CLOSE | winit::window::WindowButtons::MINIMIZE)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(PhysicalPosition::new(s.pos.0, s.pos.1))
            .with_inner_size(PhysicalSize::new(s.size.0.max(1), s.size.1.max(1)))
            .with_visible(false);
        #[cfg(windows)]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attr = attr.with_drag_and_drop(false);
            if !s.decorations {
                attr = attr.with_skip_taskbar(true);
            }
        }
        if matches!(s.kind, Kind::Overlay | Kind::Hint(_)) {
            attr = attr.with_active(false);
        }
        let window = match el.create_window(attr) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                crate::files::log_error(&self.app.paths.err, &format!("開不了視窗：{e}"));
                return;
            }
        };
        #[cfg(target_os = "macos")]
        if s.decorations {
            window.set_outer_position(PhysicalPosition::new(s.pos.0, s.pos.1));
        }
        if self.sb.is_none() {
            self.sb = softbuffer::Context::new(window.clone()).ok();
        }
        let Some(sb) = &self.sb else { return };
        let Ok(surface) = softbuffer::Surface::new(sb, window.clone()) else { return };
        let ctx = egui::Context::default();
        crate::ui::setup_ctx(&ctx);
        let proxy = self.proxy.clone();
        ctx.set_request_repaint_callback(move |_| {
            let _ = proxy.send_event(UserEvent::Wake);
        });
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, &window, Some(window.scale_factor() as f32), None, None);
        let hwnd = hwnd_of(&window);
        if s.kind == Kind::Overlay {
            platform::no_activate(hwnd);
        }
        if let Kind::Hint(i) = s.kind {
            if let Some(g) = self.app.hint_geom(i) {
                platform::hint_window(hwnd, g.frame, g.hole, g.tab, g.radius);
            }
        }
        #[cfg(target_os = "macos")]
        match s.kind {
            Kind::Select => platform::cover_screen(hwnd),
            Kind::Settings | Kind::Record => platform::key_panel(hwnd),
            Kind::Overlay | Kind::Hint(_) => {}
        }
        let id = window.id();
        self.wins.insert(
            id,
            Win {
                kind: s.kind,
                window,
                surface,
                ctx,
                state,
                rend: EguiSoftwareRender::new(ColorFieldOrder::Bgra),
                hwnd,
                repaint_at: Some(Instant::now()),
                shown: false,
                last_size: PhysicalSize::new(0, 0),
                at: (s.pos, s.size),
            },
        );
    }

    fn reconcile(&mut self, el: &ActiveEventLoop) {
        let overlay_rect = self.find(Kind::Overlay).map(|id| {
            let w = &self.wins[&id].window;
            let p = w.outer_position().unwrap_or_default();
            let s = w.outer_size();
            (p.x, p.y, s.width as i32, s.height as i32, w.scale_factor())
        });
        self.overlay_rect = overlay_rect.map(|(x, y, w, h, _)| (x, y, w, h));
        let wanted = self.app.wanted(overlay_rect);
        let same = |s: &Spec, w: &Win| s.kind == w.kind && (!matches!(s.kind, Kind::Hint(_)) || (s.pos, s.size) == w.at);
        let gone: Vec<WindowId> = self.wins.iter().filter(|(_, w)| !wanted.iter().any(|s| same(s, w))).map(|(id, _)| *id).collect();
        #[cfg(target_os = "macos")]
        let dialog = |k: Kind| matches!(k, Kind::Settings | Kind::Record | Kind::Select);
        #[cfg(target_os = "macos")]
        let closed = gone.iter().any(|id| dialog(self.wins[id].kind));
        for id in gone {
            if let Some(w) = self.wins.remove(&id) {
                if w.kind == Kind::Record {
                    self.app.record_closed();
                }
            }
        }
        #[cfg(target_os = "macos")]
        if closed && !self.wins.values().any(|w| dialog(w.kind)) {
            platform::restore_focus();
        }
        for s in wanted {
            if self.find(s.kind).is_none() {
                self.create(el, s);
            }
        }
        if let Some(id) = self.find(Kind::Overlay) {
            let vis = self.app.overlay_visible();
            let w = self.wins.get_mut(&id).unwrap();
            if w.shown != vis {
                platform::show_quiet(w.hwnd, vis);
                w.shown = vis;
                if vis {
                    platform::keep_top(w.hwnd);
                }
            }
        }
    }

    fn redraw(&mut self, id: WindowId) {
        let Some(w) = self.wins.get_mut(&id) else { return };
        let raw = w.state.take_egui_input(&w.window);
        let kind = w.kind;
        let hwnd = w.hwnd;
        let app = &mut self.app;
        let out = w.ctx.run(raw, |ctx| app.window_ui(kind, ctx, hwnd));
        w.state.handle_platform_output(&w.window, out.platform_output);
        let mut repaint = None;
        #[cfg(target_os = "macos")]
        let top_left = platform::window_pos(hwnd);
        for (_vid, vout) in out.viewport_output {
            let mut info = egui::ViewportInfo::default();
            egui_winit::process_viewport_commands(&w.ctx, &mut info, vout.commands, &w.window, &mut Default::default());
            repaint = Some(vout.repaint_delay);
        }
        #[cfg(target_os = "macos")]
        if platform::window_pos(hwnd) != top_left {
            platform::move_quiet(hwnd, top_left.0, top_left.1);
        }
        if matches!(kind, Kind::Settings | Kind::Record) {
            let sz = w.window.outer_size();
            if sz != w.last_size {
                w.last_size = sz;
                if let Ok(p) = w.window.outer_position() {
                    let (_, _, sw, sh) = platform::primary_screen();
                    let (dw, dh) = (sz.width as i32, sz.height as i32);
                    let off = p.x < 0 || p.y < 0 || p.x + dw > sw || p.y + dh > sh - 90;
                    let hit = self.overlay_rect.map_or(false, |(ox, oy, ow, oh)| p.x < ox + ow && p.x + dw > ox && p.y < oy + oh && p.y + dh > oy);
                    if off || hit {
                        let (nx, ny) = match self.overlay_rect {
                            Some(r) => crate::dialogs::place_near(r, (dw - 32, dh - 80), None),
                            None => (p.x.min(sw - dw).max(0), p.y.min(sh - 90 - dh).max(0)),
                        };
                        if (nx, ny) != (p.x, p.y) {
                            w.window.set_outer_position(PhysicalPosition::new(nx, ny));
                        }
                    }
                }
            }
        }
        if kind == Kind::Overlay {
            let sz = w.window.outer_size();
            if sz != w.last_size {
                w.last_size = sz;
                platform::round_corners(hwnd, sz.width, sz.height, crate::ui::OVERLAY_RADIUS as f32, w.window.scale_factor() * w.ctx.zoom_factor() as f64);
            }
        }
        let ppp = out.pixels_per_point;
        let prims = w.ctx.tessellate(out.shapes, ppp);
        let size = w.window.inner_size();
        if let (Some(ww), Some(hh)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
            if w.surface.resize(ww, hh).is_ok() {
                if let Ok(mut buf) = w.surface.buffer_mut() {
                    buf.fill(0x00141922);
                    let data: &mut [[u8; 4]] = unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut [u8; 4], buf.len()) };
                    let mut b = BufferMutRef::new(data, size.width as usize, size.height as usize);
                    w.rend.render(&mut b, &prims, &out.textures_delta, ppp);
                    let _ = buf.present();
                }
            }
        }
        if !w.shown && matches!(kind, Kind::Hint(_)) {
            platform::show_quiet(hwnd, true);
            w.shown = true;
        } else if !w.shown && kind != Kind::Overlay {
            w.window.set_visible(true);
            w.shown = true;
            platform::force_focus(hwnd);
        } else if !w.shown && kind == Kind::Overlay && self.app.overlay_visible() {
            platform::show_quiet(hwnd, true);
            w.shown = true;
        }
        w.repaint_at = match repaint {
            Some(d) if d < Duration::from_secs(3600) => Some(Instant::now() + d),
            _ => None,
        };
        if kind == Kind::Overlay {
            platform::set_alpha(hwnd, self.app.alpha());
        }
    }

    fn redraw_all(&mut self) {
        let ids: Vec<WindowId> = self.wins.keys().copied().collect();
        for id in ids {
            self.redraw(id);
        }
    }
}

fn hwnd_of(w: &Window) -> isize {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(h) = w.window_handle() {
        #[allow(irrefutable_let_patterns)]
        if let RawWindowHandle::Win32(h) = h.as_raw() {
            return h.hwnd.get();
        }
        #[cfg(target_os = "macos")]
        if let RawWindowHandle::AppKit(h) = h.as_raw() {
            return h.ns_view.as_ptr() as isize;
        }
    }
    0
}

impl ApplicationHandler<UserEvent> for Runner {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.reconcile(el);
        self.redraw_all();
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        if !self.app.quit {
            self.app.quit();
        }
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn _exit(code: i32) -> !;
            }
            unsafe { _exit(0) }
        }
    }

    fn new_events(&mut self, el: &ActiveEventLoop, cause: StartCause) {
        let _ = el;
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            let now = Instant::now();
            if now >= self.next_tick {
                self.next_tick = now + Duration::from_millis(500);
                let hwnds: Vec<(Kind, isize)> = self.wins.values().map(|w| (w.kind, w.hwnd)).collect();
                let pos = self.find(Kind::Overlay).and_then(|id| self.wins[&id].window.outer_position().ok()).map(|p| (p.x, p.y));
                self.app.logic(&hwnds, pos);
                for w in self.wins.values_mut() {
                    w.repaint_at = Some(now);
                }
            }
        }
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, _e: UserEvent) {
        let hwnds: Vec<(Kind, isize)> = self.wins.values().map(|w| (w.kind, w.hwnd)).collect();
        self.app.handle_msgs(&hwnds);
        for w in self.wins.values_mut() {
            w.repaint_at = Some(Instant::now());
        }
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(w) = self.wins.get_mut(&id) else { return };
        let resp = w.state.on_window_event(&w.window, &event);
        match event {
            WindowEvent::RedrawRequested => self.redraw(id),
            WindowEvent::Moved(p) => {
                let k = w.kind;
                self.app.dialog_moved(k, (p.x, p.y));
            }
            WindowEvent::CloseRequested => {
                w.repaint_at = Some(Instant::now());
                let k = w.kind;
                self.app.close_req(k);
            }
            _ => {
                if resp.repaint {
                    w.repaint_at = Some(Instant::now());
                }
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.app.quit {
            el.exit();
            return;
        }
        self.reconcile(el);
        let now = Instant::now();
        let due: Vec<WindowId> = self.wins.iter().filter(|(_, w)| w.repaint_at.map_or(false, |t| t <= now)).map(|(id, _)| *id).collect();
        for id in due {
            self.redraw(id);
        }
        let mut next = self.next_tick;
        for w in self.wins.values() {
            if let Some(t) = w.repaint_at {
                next = next.min(t);
            }
        }
        el.set_control_flow(ControlFlow::WaitUntil(next.max(now + Duration::from_millis(5))));
    }
}
