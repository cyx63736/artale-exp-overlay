use crate::dialogs::{Record, Settings};
use crate::files::{self, Paths};
use crate::platform::{self, Hotkey};
use crate::potions::{self, Book, Check, Event, Prompt, Seen, Undo, Watch};
use crate::select::Select;
use crate::worker::{self, Msg, Shared};
use crate::runner::{Kind, Spec};
use egui::{self, Color32, FontFamily, FontId, RichText, Sense, ViewportCommand};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use tracker::util::{comma, comma_f, dur, hms, parse_count, short};
use tracker::{Tracker, Ui as TUi};

pub const BG: Color32 = Color32::from_rgb(0x14, 0x19, 0x22);
pub const OVERLAY_RADIUS: u8 = 8;
pub const WIDGET_RADIUS: u8 = 5;

#[cfg(not(target_os = "macos"))]
const PPP: [f32; 6] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.5];
#[cfg(not(target_os = "macos"))]
const Y_CJK_M: [f32; 6] = [-0.20, -0.18, -0.18, -0.20, -0.135, -0.19];
#[cfg(not(target_os = "macos"))]
const Y_CJK_LBL: [f32; 6] = [-0.19, -0.25, -0.19, -0.22, -0.19, -0.22];
#[cfg(target_os = "macos")]
const PPP: [f32; 1] = [2.0];
#[cfg(target_os = "macos")]
const Y_CJK_M: [f32; 1] = [-0.035];
#[cfg(target_os = "macos")]
const Y_CJK_LBL: [f32; 1] = [0.06];
#[cfg(not(target_os = "macos"))]
const Y_SYM_M_EXTRA: f32 = 0.0;
#[cfg(target_os = "macos")]
const Y_SYM_M_EXTRA: f32 = 0.135;
#[cfg(not(target_os = "macos"))]
const Y_SYM_BTN: f32 = -0.30;
#[cfg(target_os = "macos")]
const Y_SYM_BTN: f32 = 0.075;
#[cfg(not(target_os = "macos"))]
const ICON_DY: f32 = 3.2;
#[cfg(target_os = "macos")]
const ICON_DY: f32 = 0.0;
pub const PANEL: Color32 = Color32::from_rgb(0x1f, 0x27, 0x35);
pub const LINE: Color32 = Color32::from_rgb(0x2e, 0x38, 0x4b);
pub const INK: Color32 = Color32::from_rgb(0xe6, 0xea, 0xf2);
pub const MUTED: Color32 = Color32::from_rgb(0x8b, 0x95, 0xa8);
pub const EXP: Color32 = Color32::from_rgb(0xf0, 0xc0, 0x50);
pub const HP: Color32 = Color32::from_rgb(0xff, 0x6b, 0x61);
pub const MP: Color32 = Color32::from_rgb(0x6e, 0xa2, 0xff);
pub const GOOD: Color32 = Color32::from_rgb(0x4f, 0xcf, 0x8f);
pub const WARN: Color32 = Color32::from_rgb(0xff, 0xa2, 0x5c);

pub fn pt(p: f32) -> f32 {
    p * 4.0 / 3.0
}
pub fn font(p: f32) -> FontId {
    FontId::new(pt(p), FontFamily::Proportional)
}
pub fn bold(p: f32) -> FontId {
    FontId::new(pt(p), FontFamily::Name("bold".into()))
}
fn lbl_font(p: f32) -> FontId {
    FontId::new(pt(p), FontFamily::Name("lbl".into()))
}
pub fn mono(p: f32, b: bool) -> FontId {
    FontId::new(pt(p), FontFamily::Name(if b { "mono_bold" } else { "mono" }.into()))
}

const ICON_BOX: f32 = 18.0;

fn draw_icon(p: &egui::Painter, rect: egui::Rect, kind: &str, counting: bool, col: Color32) {
    use egui::{pos2, Rect, Shape, Stroke};
    let c = rect.center();
    let st = Stroke::new(1.6, col);
    let at = |x: f32, y: f32| pos2(c.x + x, c.y + y);
    match kind {
        "run" if counting => {
            for x in [-4.0, 1.5] {
                p.rect_filled(Rect::from_min_max(at(x, -5.0), at(x + 2.5, 5.0)), 0.8, col);
            }
        }
        "run" => {
            p.add(Shape::convex_polygon(vec![at(-3.0, -5.0), at(5.0, 0.0), at(-3.0, 5.0)], col, Stroke::new(1.0, col)));
        }
        "set" => {
            let ppp = p.ctx().pixels_per_point();
            let (mid, gap) = ((c.y * ppp).round() / ppp, (4.5 * ppp).round() / ppp);
            for k in [-1.0, 0.0, 1.0] {
                let y = mid + k * gap - c.y;
                p.line_segment([at(-5.5, y), at(5.5, y)], st);
            }
        }
        "reset" => {
            let (r, a0, a1) = (5.5, (-10.0f32).to_radians(), 255.0f32.to_radians());
            let pts = (0..=28)
                .map(|i| {
                    let a = a0 + (a1 - a0) * i as f32 / 28.0;
                    at(r * a.cos(), r * a.sin())
                })
                .collect();
            p.add(Shape::line(pts, st));
            let end = at(r * a1.cos(), r * a1.sin());
            let (d, n) = (egui::vec2(-a1.sin(), a1.cos()), egui::vec2(a1.cos(), a1.sin()));
            p.add(Shape::convex_polygon(vec![end + d * 3.4, end + n * 2.9 - d * 0.6, end - n * 2.9 - d * 0.6], col, Stroke::NONE));
        }
        "compact" => {
            p.rect_stroke(Rect::from_min_max(at(-5.5, -4.0), at(5.5, 4.0)), 1.5, st, egui::StrokeKind::Middle);
            p.line_segment([at(-2.5, 0.0), at(2.5, 0.0)], st);
        }
        _ => {
            p.line_segment([at(-4.5, -4.5), at(4.5, 4.5)], st);
            p.line_segment([at(4.5, -4.5), at(-4.5, 4.5)], st);
        }
    }
}

fn btn_hints() -> &'static [(&'static str, String)] {
    static H: std::sync::OnceLock<Vec<(&'static str, String)>> = std::sync::OnceLock::new();
    H.get_or_init(|| {
        let [run, _] = platform::HOTKEY_NAMES;
        let mut v = vec![("run", format!("開始／暫停計時（{run}）"))];
        v.extend(BTN_HINTS_REST.iter().map(|(k, t)| (*k, t.to_string())));
        v
    })
}

const BTN_HINTS_REST: [(&str, &str); 5] = [
    ("set", "設定：藥水價格、框選要自動讀取的位置"),
    ("reset", "結束這一段：成績存進練功紀錄.csv，重新開始算（要連按兩下）"),
    ("compact", "切換精簡模式（只顯示一行）"),
    ("normal", "切換一般模式（顯示詳細內容）"),
    ("close", "關閉浮窗（會先存紀錄）"),
];

pub struct Overlay {
    pub t: Tracker,
    pub paths: Paths,
    pub shared: Arc<Shared>,
    rx: Receiver<Msg>,
    hk: Receiver<Hotkey>,
    hk_fail: Receiver<Vec<&'static str>>,
    quit_rx: Receiver<()>,
    hwnds: Vec<(Kind, isize)>,
    pub quit: bool,
    last_tick: f64,
    last_top: f64,
    last_save: f64,
    hover: Option<&'static str>,
    ocr_fatal: Option<String>,
    hotkey_msg: String,
    pub visible: bool,
    pub settings: Option<Settings>,
    pub record: Option<Record>,
    pub select: Option<Select>,
    size: egui::Vec2,
    last_pos: Option<(i32, i32)>,
    pos_changed_at: f64,
    drag_from: Option<((i32, i32), (i32, i32))>,
    quit_armed: f64,
    pub last_record_fg: Option<isize>,
    pub book: Book,
    pub watch: [Watch; 2],
    pub hint: Option<(u8, f64)>,
    pub auto: Option<crate::autoframe::AutoRun>,
    pub auto_pick: Option<crate::autoframe::AutoPick>,
    scale: f64,
    zoom_seen: f32,
    bad_since: Option<f64>,
    bad_hold: f64,
    screen_warned: Option<String>,
}

pub struct HintGeom {
    pub pos: (i32, i32),
    pub size: (i32, i32),
    pub frame: (i32, i32, i32, i32),
    pub hole: (i32, i32, i32, i32),
    pub tab: (i32, i32, i32, i32),
    pub radius: i32,
    pub text: String,
}

pub fn region_of(cfg: &serde_json::Map<String, serde_json::Value>, kind: &str) -> Option<[i32; 4]> {
    let r: Vec<i32> = cfg.get(&format!("{kind}_region"))?.as_array()?.iter().filter_map(|v| v.as_f64()).map(|v| v as i32).collect();
    r.try_into().ok()
}

pub use tracker::util::{short_name as disp, NAME_MAX};

pub fn screen_key() -> String {
    let (_, _, w, h) = platform::primary_screen();
    format!("{w}x{h}")
}

fn zoom_key() -> String {
    #[cfg(target_os = "macos")]
    {
        let s = ocr::capture::scale();
        if (s - 1.0).abs() > 0.01 {
            return format!("{}@{s}", screen_key());
        }
    }
    screen_key()
}

pub const ZOOM_MAX: f32 = 2.0;

pub fn region_tag(r: [i32; 4]) -> String {
    platform::screen_tag(r[0] + r[2] / 2, r[1] + r[3] / 2)
}

pub const HINT_PICK: u8 = 10;

const HINT_SEC: f64 = 5.0;
pub const HINT: Color32 = Color32::from_rgb(0xff, 0x3b, 0x30);

#[derive(Clone)]
pub enum PromptAct {
    Pick(String),
    Other,
    NotChanged,
    Revert,
    Done,
    AutoPick(usize),
    AutoSkip,
}

pub fn setup_ctx(ctx: &egui::Context) {
    ctx.set_fonts(fonts_for(PPP.len() - 1));
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.interaction.selectable_labels = false;
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = BG;
        style.visuals.extreme_bg_color = PANEL;
        let r = egui::CornerRadius::same(WIDGET_RADIUS);
        for w in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            w.corner_radius = r;
        }
        style.visuals.menu_corner_radius = r;
    });
}

impl Overlay {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        t: Tracker,
        paths: Paths,
        shared: Arc<Shared>,
        rx: Receiver<Msg>,
        hk: Receiver<Hotkey>,
        hk_fail: Receiver<Vec<&'static str>>,
        quit_rx: Receiver<()>,
    ) -> Overlay {
        let book = Book::load(&paths.here);
        Overlay {
            t,
            paths,
            shared,
            rx,
            hk,
            hk_fail,
            quit_rx,
            hwnds: vec![],
            quit: false,
            last_tick: 0.0,
            last_top: 0.0,
            last_save: tracker::sys_now(),
            hover: None,
            ocr_fatal: None,
            hotkey_msg: String::new(),
            visible: true,
            settings: None,
            record: None,
            select: None,
            size: egui::vec2(260.0, 300.0),
            last_pos: None,
            pos_changed_at: 0.0,
            drag_from: None,
            quit_armed: 0.0,
            last_record_fg: None,
            book,
            watch: Default::default(),
            hint: None,
            auto: None,
            auto_pick: None,
            scale: 1.0,
            zoom_seen: 1.0,
            bad_since: None,
            bad_hold: 0.0,
            screen_warned: None,
        }
        .init_book()
    }

    fn init_book(mut self) -> Overlay {
        if !self.names_ok() {
            return self;
        }
        for k in 0..2 {
            let name = self.t.name(k);
            self.book.upsert(k, &name, self.t.price(k));
            if self.paths.tpl[k].exists() && self.book.find(k, &name).map_or(false, |p| p.tpl.is_none()) {
                self.book.store_tpl(k, &name, &self.paths.tpl[k]);
            }
            let r = self.book.find(k, &name).and_then(|p| p.qs.clone());
            self.watch[k].reset(r);
        }
        self.book.save();
        self
    }

    pub fn now(&self) -> f64 {
        self.t.now()
    }

    fn check_screen(&mut self) {
        if self.auto.is_some() || self.auto_pick.is_some() {
            return;
        }
        let old = self.t.cfg.get("region_screen").and_then(|v| v.as_str()).map(|o| o != screen_key());
        let mut moved: Vec<String> = vec![];
        let mut fill = false;
        for k in ["exp", "hp", "mp", "inv", "meso"] {
            let Some(r) = region_of(&self.t.cfg, k) else { continue };
            let now = region_tag(r);
            let key = format!("{k}_screen");
            match self.t.cfg.get(&key).and_then(|v| v.as_str()) {
                Some(t) if t != now => moved.push(now),
                Some(_) => {}
                None if old == Some(true) => moved.push(now),
                None => {
                    self.t.cfg.insert(key, now.into());
                    fill = true;
                }
            }
        }
        if fill {
            self.save_cfg();
        }
        if moved.is_empty() {
            self.screen_warned = None;
            return;
        }
        moved.sort();
        moved.dedup();
        let sig = moved.join(" ");
        if self.screen_warned.as_deref() != Some(sig.as_str()) {
            self.screen_warned = Some(sig);
            let msg = "螢幕解析度變了，之前框的位置可能不準，請打開背包、切到消耗欄，再按設定裡的「自動框選」";
            self.t.say(msg, 15.0);
            self.t.logs.push(msg.into());
        }
    }

    pub fn names_ok(&self) -> bool {
        self.t.cfg.get("names_ok").and_then(|v| v.as_bool()).unwrap_or(true)
    }

    pub fn flush_logs(&mut self) {
        let lines = std::mem::take(&mut self.t.logs);
        files::log_detail(&self.paths.detail, &lines);
    }

    pub fn save_cfg(&mut self) {
        if !files::save_cfg(&self.paths.cfg, &self.t.cfg) {
            files::log_error(&self.paths.err, "存不了 config.json");
        }
        *self.shared.cfg.lock().unwrap() = self.t.cfg.clone();
        self.shared.wake();
    }

    pub fn refresh_tpl(&mut self) {
        self.t.tpl_exists = [self.paths.tpl[0].exists(), self.paths.tpl[1].exists()];
    }

    fn watch_ocr(&mut self, o: &tracker::OcrOut) {
        let now = self.now();
        let bag_open = o.inv.as_ref().map_or(false, |inv| inv.iter().any(|v| v.as_ref().map_or(false, |v| !v.is_empty())));
        if (self.t.has("exp_region") && now - self.t.exp_ok_t > 5.0) || !self.names_ok() || self.settings.is_some() {
            return;
        }
        let odd = |k: usize| -> bool {
            let Some(sig) = &o.qs_sig[k] else { return false };
            let w = &self.watch[k];
            let r = w.check.as_ref().and_then(|c| c.from_sig.as_ref()).or(w.reference.as_ref());
            r.map_or(false, |r| potions::swapped(sig, r))
        };
        let both = odd(0) && odd(1);
        for k in 0..2 {
            let (Some(sig), Some(text)) = (&o.qs_sig[k], &o.qs[k]) else { continue };
            let Some(val) = parse_count(text) else { continue };
            let cells = if bag_open && self.t.tpl_exists[k] { o.inv.as_ref().and_then(|inv| inv[k].as_deref()) } else { None };
            let name = self.t.name(k);
            let others: Vec<(String, Option<Vec<f32>>)> =
                self.book.of(k).iter().filter(|p| p.name != name).map(|p| (p.name.clone(), p.qs.clone())).collect();
            let before = self.watch[k].prompt.clone();
            let seen = Seen { now, sig, val, cells, used: self.t.used(k), effect: both || potions::flooded(sig) };
            match self.watch[k].feed(&seen, &others) {
                Event::Confirmed => {
                    self.watch[k].check = None;
                    self.t.logs.push(format!("{name}：快捷欄確認換好了"));
                    self.t.say(format!("快捷欄已經換成{name}"), 4.0);
                }
                Event::Renamed(from) => self.renamed(k, &from),
                Event::None => {}
            }
            let w = &mut self.watch[k];
            if w.prompt != before {
                match &w.prompt {
                    Some(Prompt::GameChanged { .. }) => self.t.logs.push(format!("{name}：快捷欄的藥水看起來換了，提醒確認")),
                    Some(Prompt::Mismatch { from }) => self.t.logs.push(format!("設定換成{name}，快捷欄看起來還是{from}，提醒確認")),
                    None => {}
                }
            }
            if self.watch[k].dirty {
                self.watch[k].dirty = false;
                let r = self.watch[k].reference.clone();
                if std::mem::take(&mut self.watch[k].claimed) {
                    if let Some(r) = &r {
                        self.book.drop_similar_qs(k, &name, r);
                    }
                }
                self.book.set_qs(k, &name, r);
                self.book.save();
            }
        }
    }

    pub fn switch_potion(&mut self, k: usize, name: &str, price: f64, mark: Option<i64>, verify: bool) {
        let old = self.t.name(k);
        let name = name.trim();
        if name.is_empty() || name == old {
            return;
        }
        let key = |s: &str| format!("{}_{s}", potions::KIND[k]);
        let undo = Undo {
            name: old.clone(),
            price: self.t.cfg.get(&key("price")).cloned().unwrap_or_default(),
            cost_prev: self.t.cost_prev[k],
            used_mark: self.t.used_mark[k],
            prev_price: self.t.prev_price[k],
            session: self.t.session_no,
        };
        self.book.upsert(k, &old, self.t.price(k));
        if self.paths.tpl[k].exists() {
            self.book.store_tpl(k, &old, &self.paths.tpl[k]);
        }
        let from_sig = self.watch[k].reference.clone();
        if from_sig.is_some() {
            self.book.set_qs(k, &old, from_sig.clone());
        }
        let v0 = self.t.qs[k];
        let here = region_of(&self.t.cfg, potions::KIND[k]);
        self.book.keep_slot(k, &old, here);
        self.t.switch_potion(k, name, price, mark);
        self.book.upsert(k, name, price);
        let moved = if verify { self.book.find(k, name).and_then(|p| p.qs_at).filter(|&r| Some(r) != here) } else { None };
        if !verify {
            self.book.set_slot(k, name, here);
        }
        if let Some(r) = moved {
            self.t.cfg.insert(format!("{}_region", potions::KIND[k]), serde_json::json!(r));
            self.t.raw.insert(potions::KIND[k].into(), String::new());
            self.t.logs.push(format!("{name} 放在快捷欄的另一格，改讀它記住的位置"));
        }
        let has_tpl = self.book.restore_tpl(k, name, &self.paths.tpl[k]);
        self.refresh_tpl();
        self.t.inv_msg.clear();
        self.book.save();
        self.save_cfg();
        let r = self.book.find(k, name).and_then(|p| p.qs.clone());
        let now = self.t.now();
        let w = &mut self.watch[k];
        w.reset(r);
        w.prompt = None;
        w.mark_hint = None;
        w.check = if verify && moved.is_none() { Some(Check { t0: now, from_sig, v0, same: 0, changed: 0, undo }) } else { None };
        if !has_tpl && self.t.has("inv_region") {
            self.t.say(format!("已換成{name}，請打開背包按 ☰ 框選一次{name}"), 12.0);
        } else {
            self.t.say(format!("已換成{name}"), 4.0);
        }
        self.show_hint(1 | 1 << (k + 1));
    }

    pub fn show_hint(&mut self, mask: u8) {
        let now = self.now();
        let old = self.hint.filter(|h| now - h.1 < HINT_SEC).map_or(0, |h| h.0);
        self.hint = Some((old | mask, now));
    }

    pub fn hint_geom(&self, i: u8) -> Option<HintGeom> {
        let ([x, y, w, h], text) = if i >= HINT_PICK {
            let r = self.auto_pick.as_ref()?.pairs.get((i - HINT_PICK) as usize)?.qs.rect;
            ([r.0, r.1, r.2, r.3], format!("{}", i - HINT_PICK + 1))
        } else {
            let (mask, t0) = self.hint?;
            if self.now() - t0 >= HINT_SEC || mask & (1 << i) == 0 {
                return None;
            }
            let (key, text) = match i {
                0 => ("inv", "背包".to_string()),
                3 => ("exp", "EXP".to_string()),
                4 => ("meso", "楓幣".to_string()),
                _ => (potions::KIND[i as usize - 1], format!("{} 快捷欄", disp(&self.t.name(i as usize - 1)))),
            };
            (region_of(&self.t.cfg, key)?, text)
        };
        let s = self.scale;
        let px = |p: f64| (p * s).round() as i32;
        let (gap, bw) = (px(3.0), px(3.0).max(2));
        let tw = px(text.chars().map(|c| if c.is_ascii() { 8.0 } else { 14.5 }).sum::<f64>() + 14.0);
        let th = px(20.0);
        let tg = px(3.0);
        let (ox, oy, ow, oh) = (x - gap - bw, y - gap - bw, w + 2 * (gap + bw), h + 2 * (gap + bw));
        let (vx, vy, vw, _) = platform::virtual_screen();
        let (tx, ty) = if i == 0 || i == 3 || i >= HINT_PICK {
            (ox, if oy - th - tg >= vy { oy - th - tg } else { oy + oh + tg })
        } else {
            (if ox - tw - tg >= vx || ox + ow + tg + tw > vx + vw { ox - tw - tg } else { ox + ow + tg }, oy)
        };
        let (left, top) = (ox.min(tx), oy.min(ty));
        let (right, bottom) = ((ox + ow).max(tx + tw), (oy + oh).max(ty + th));
        Some(HintGeom {
            pos: (left, top),
            size: (right - left, bottom - top),
            frame: (ox - left, oy - top, ow, oh),
            hole: (x - gap - left, y - gap - top, w + 2 * gap, h + 2 * gap),
            tab: (tx - left, ty - top, tw, th),
            radius: px(6.0),
            text,
        })
    }

    fn hint_ui(&mut self, ctx: &egui::Context, i: u8) {
        let Some(g) = self.hint_geom(i) else { return };
        let ppp = ctx.pixels_per_point();
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(HINT)).show(ctx, |ui| {
            let (x, y, w, h) = g.tab;
            let r = egui::Rect::from_min_size(egui::pos2(x as f32 / ppp, y as f32 / ppp), egui::vec2(w as f32 / ppp, h as f32 / ppp));
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, &g.text, bold(10.0), Color32::WHITE);
        });
    }

    fn revert_potion(&mut self, k: usize) {
        let Some(c) = self.watch[k].check.take() else { return };
        let u = c.undo;
        let cur = self.t.name(k);
        let key = |s: &str| format!("{}_{s}", potions::KIND[k]);
        self.t.cfg.insert(key("name"), u.name.clone().into());
        self.t.cfg.insert(key("price"), u.price);
        if u.session == self.t.session_no {
            self.t.cost_prev[k] = u.cost_prev;
            self.t.used_mark[k] = u.used_mark;
            self.t.prev_price[k] = u.prev_price;
        } else {
            self.t.cost_prev[k] = 0.0;
            self.t.used_mark[k] = 0;
            self.t.prev_price[k] = None;
        }
        self.t.forget_potion(k);
        self.t.forget_qs(k);
        self.book.restore_tpl(k, &u.name, &self.paths.tpl[k]);
        self.refresh_tpl();
        self.t.inv_msg.clear();
        self.save_cfg();
        self.watch[k].reset(c.from_sig);
        self.watch[k].prompt = None;
        self.t.logs.push(format!("改回{}（換成{cur}之後算的用量都算回{}）", u.name, u.name));
        self.t.say(format!("已改回{}", u.name), 4.0);
    }

    fn renamed(&mut self, k: usize, from: &str) {
        let name = self.t.name(k);
        self.book.move_look(k, from, &name);
        self.book.restore_tpl(k, &name, &self.paths.tpl[k]);
        self.refresh_tpl();
        self.t.inv_msg.clear();
        self.book.save();
        self.t.say(format!("快捷欄一直都是{name}：{from}記的背包格子和快捷欄顏色已經搬到{name}"), 8.0);
        self.t.logs.push(format!("快捷欄跟改名前一樣，是之前名字填錯：{from}記的背包格子和快捷欄顏色搬到{name}"));
    }

    fn prompts(&self) -> Vec<(usize, String, Vec<(String, PromptAct, bool)>)> {
        let mut v = vec![];
        for k in 0..2 {
            let name = self.t.name(k);
            match &self.watch[k].prompt {
                Some(Prompt::GameChanged { suggest, .. }) => {
                    let mut btns: Vec<(String, PromptAct, bool)> = vec![];
                    if let Some(s) = suggest {
                        btns.push((disp(s), PromptAct::Pick(s.clone()), true));
                    }
                    for p in self.book.of(k).iter().filter(|p| p.name != name && Some(&p.name) != suggest.as_ref()).take(5) {
                        btns.push((disp(&p.name), PromptAct::Pick(p.name.clone()), false));
                    }
                    let name = disp(&name);
                    btns.push(("其他…".into(), PromptAct::Other, false));
                    btns.push(("沒換".into(), PromptAct::NotChanged, false));
                    let text = if suggest.is_some() || btns.len() > 2 {
                        format!("快捷欄的{name}好像換了別的藥水？換成：")
                    } else {
                        format!("快捷欄的{name}好像換了別的藥水？")
                    };
                    v.push((k, text, btns));
                }
                Some(Prompt::Mismatch { from }) => {
                    let (name, from) = (disp(&name), disp(from));
                    v.push((
                        k,
                        format!("設定換成{name}，但快捷欄看起來還是{from}"),
                        vec![(format!("改回{from}"), PromptAct::Revert, true), (format!("快捷欄已經是{name}"), PromptAct::Done, false)],
                    ));
                }
                None => {}
            }
        }
        if let Some(p) = &self.auto_pick {
            if let Some(&k) = p.todo.first() {
                let n = p.used.iter().filter(|u| !**u).count();
                let mut btns: Vec<(String, PromptAct, bool)> =
                    (0..p.pairs.len()).filter(|&i| !p.used[i]).map(|i| (format!("{}", i + 1), PromptAct::AutoPick(i), false)).collect();
                btns.push(("都不是".into(), PromptAct::AutoSkip, false));
                v.push((k, format!("快捷欄有 {n} 種藥水，哪一格是{}？（看紅框旁的號碼）", disp(&self.t.name(k))), btns));
            }
        }
        v
    }

    fn prompt_action(&mut self, k: usize, a: PromptAct) {
        match a {
            PromptAct::AutoPick(i) => crate::autoframe::pick(self, i),
            PromptAct::AutoSkip => crate::autoframe::skip(self),
            PromptAct::Pick(name) => {
                let mark = match self.watch[k].prompt {
                    Some(Prompt::GameChanged { mark, .. }) => Some(mark),
                    _ => None,
                };
                if let Some(p) = self.book.find(k, &name).cloned() {
                    self.switch_potion(k, &p.name, p.price, mark, false);
                }
            }
            PromptAct::Other => {
                if let Some(Prompt::GameChanged { mark, .. }) = self.watch[k].prompt {
                    self.watch[k].mark_hint = Some((mark, self.now()));
                }
                self.watch[k].prompt = None;
                self.watch[k].calm();
                self.open_settings();
            }
            PromptAct::NotChanged => {
                self.watch[k].prompt = None;
                self.watch[k].trust_screen();
                self.t.logs.push(format!("{}：按了「沒換」", self.t.name(k)));
            }
            PromptAct::Revert => self.revert_potion(k),
            PromptAct::Done => {
                let name = self.t.name(k);
                let now = self.now();
                let w = &mut self.watch[k];
                let from = match w.prompt.take() {
                    Some(Prompt::Mismatch { from }) => Some(from),
                    _ => None,
                };
                let from_sig = w.check.take().and_then(|c| c.from_sig);
                w.user_done(now, from.zip(from_sig));
                self.t.logs.push(format!("{name}：按了「快捷欄已經是{name}」"));
            }
        }
    }

    pub fn handle_msgs(&mut self, hwnds: &[(Kind, isize)]) {
        self.hwnds = hwnds.to_vec();
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Ready => self.t.ocr_msg = "讀取引擎就緒".into(),
                Msg::Fatal(e) => {
                    files::log_error(&self.paths.err, &e);
                    self.t.ocr_msg = e.clone();
                    self.ocr_fatal = Some(e);
                }
                Msg::Err(e) => {
                    files::log_error(&self.paths.err, &e);
                    self.t.ocr_msg = e;
                }
                Msg::Logs(l) => files::log_detail(&self.paths.detail, &l),
                Msg::Auto(f, img) => crate::autoframe::found(self, *f, *img),
                Msg::Ocr(o) => {
                    self.t.on_ocr(&o);
                    self.watch_ocr(&o);
                }
            }
        }
        while let Ok(bad) = self.hk_fail.try_recv() {
            self.hotkey_msg = format!("{} 被其他程式占用，請直接點浮窗上的按鈕", bad.join("、"));
        }
        while let Ok(k) = self.hk.try_recv() {
            match k {
                Hotkey::Pause => self.t.toggle_run(0.0),
                Hotkey::Toggle => {
                    if self.select.is_some() {
                        continue;
                    }
                    self.visible = !self.visible;
                }
            }
        }
        if self.quit_rx.try_recv().is_ok() {
            if self.t.counting() && self.t.meso_anchor.is_some() && self.now() - self.t.meso_t >= 5.0 {
                self.t.seg_pending = true;
            }
            if !self.t.save_state() {
                if self.t.log_session() == tracker::Saved::Failed {
                    files::log_error(&self.paths.err, "換新浮窗時存不了進度，也寫不進練功紀錄");
                } else {
                    let _ = std::fs::remove_file(&self.paths.state);
                }
            }
            if self.t.running {
                let a = self.t.active();
                self.t.logs.push(format!("換新版浮窗，進度已存（{}）", hms(a)));
            }
            self.flush_logs();
            self.save_cfg();
            std::process::exit(0);
        }
        for u in std::mem::take(&mut self.t.ui) {
            match u {
                TUi::OpenRecord => self.open_record(),
            }
        }
    }

    pub fn open_record(&mut self) {
        if self.select.is_some() {
            return;
        }
        match &mut self.record {
            Some(r) => r.focus = true,
            None => {
                let fg = platform::foreground();
                let mine = self.hwnds.iter().any(|(_, h)| *h == fg);
                self.record = Some(Record::new(&self.t, if mine || fg == 0 { None } else { Some(fg) }));
            }
        }
    }

    pub fn record_closed(&mut self) {
        if let Some(fg) = self.last_record_fg.take() {
            platform::set_foreground(fg);
        }
    }

    pub fn open_settings(&mut self) {
        match &mut self.settings {
            Some(s) => s.focus = true,
            None => {
                let mut s = Settings::new(&self.t);
                s.zoom = self.zoom();
                self.settings = Some(s);
            }
        }
    }

    pub fn quit(&mut self) -> bool {
        let now = self.now();
        let armed = self.quit_armed;
        if !self.t.save_state() && now - armed >= 5.0 {
            if self.t.log_session() == tracker::Saved::Failed {
                self.quit_armed = now;
                self.t.say("存不了進度，也寫不進練功紀錄（Excel 開著嗎？）。5 秒內再按一次 ✕ 就直接關閉", 5.0);
                return false;
            }
            let _ = std::fs::remove_file(&self.paths.state);
        }
        if self.t.running {
            let a = self.t.active();
            self.t.logs.push(format!("關閉浮窗，進度已存（{}）", hms(a)));
        }
        self.flush_logs();
        self.save_cfg();
        self.quit = true;
        true
    }
}

type Base = (egui::FontDefinitions, Option<&'static [u8]>, Option<&'static [u8]>, Option<&'static [u8]>);
fn fonts() -> &'static Base {
    static F: std::sync::OnceLock<Base> = std::sync::OnceLock::new();
    F.get_or_init(load_fonts)
}

fn ppp_slot(ppp: f32) -> usize {
    (0..PPP.len()).min_by(|&a, &b| (PPP[a] - ppp).abs().total_cmp(&(PPP[b] - ppp).abs())).unwrap_or(0)
}

fn fonts_for(i: usize) -> egui::FontDefinitions {
    let (base, cjk, cjk_b, sym) = fonts();
    let mut defs = base.clone();
    let mut put = |name: &str, data: Option<&'static [u8]>, f: f32| {
        if let Some(bytes) = data {
            let fd = egui::FontData::from_static(bytes).tweak(egui::FontTweak { y_offset_factor: f, ..Default::default() });
            defs.font_data.insert(name.to_string(), Arc::new(fd));
        }
    };
    put("cjk_m", *cjk, Y_CJK_M[i]);
    put("cjk_bold_m", *cjk_b, Y_CJK_M[i]);
    put("sym_m", *sym, Y_CJK_M[i] + Y_SYM_M_EXTRA);
    put("cjk_lbl", *cjk, Y_CJK_LBL[i]);
    defs
}

pub fn sync_fonts(ctx: &egui::Context) {
    let i = ppp_slot(ctx.pixels_per_point());
    let id = egui::Id::new("font_ppp_slot");
    if ctx.data(|d| d.get_temp::<usize>(id)) != Some(i) {
        ctx.data_mut(|d| d.insert_temp(id, i));
        ctx.set_fonts(fonts_for(i));
    }
}

fn load_fonts() -> Base {
    let mut defs = egui::FontDefinitions::default();
    let mut add = |name: &str, paths: &[&str], index: u32| -> Option<&'static [u8]> {
        for p in paths {
            if let Ok(bytes) = std::fs::read(p) {
                let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
                let mut fd = egui::FontData::from_static(bytes);
                fd.index = index;
                defs.font_data.insert(name.to_string(), Arc::new(fd));
                return Some(bytes);
            }
        }
        None
    };
    let win = |f: &str| format!(r"C:\Windows\Fonts\{f}");
    let cjk_data = add("cjk", &[&win("msjh.ttc"), "/System/Library/Fonts/STHeiti Light.ttc", "/System/Library/Fonts/STHeiti Medium.ttc", "/System/Library/Fonts/PingFang.ttc"], 0);
    let cjk_b_data = add("cjk_bold", &[&win("msjhbd.ttc"), &win("msjh.ttc"), "/System/Library/Fonts/STHeiti Medium.ttc", "/System/Library/Fonts/PingFang.ttc"], 0);
    let mono = add("mono", &[&win("consola.ttf"), "/System/Library/Fonts/Menlo.ttc"], 0).is_some();
    let mono_b = add("mono_b", &[&win("consolab.ttf"), &win("consola.ttf"), "/System/Library/Fonts/Menlo.ttc"], 0).is_some();
    let sym_data = add("sym", &[&win("seguisym.ttf"), "/System/Library/Fonts/Apple Symbols.ttf"], 0);
    let sym = sym_data.is_some();
    let sym2 = add("sym2", &["/System/Library/Fonts/ZapfDingbats.ttf"], 0).is_some();
    let (cjk, cjk_b) = (cjk_data.is_some(), cjk_b_data.is_some());
    let last = PPP.len() - 1;
    for (name, data, f) in [
        ("cjk_m", cjk_data, Y_CJK_M[last]),
        ("cjk_bold_m", cjk_b_data, Y_CJK_M[last]),
        ("sym_btn", sym_data, Y_SYM_BTN),
        ("cjk_lbl", cjk_data, Y_CJK_LBL[last]),
        ("sym_m", sym_data, Y_CJK_M[last] + Y_SYM_M_EXTRA),
    ] {
        if let Some(bytes) = data {
            let fd = egui::FontData::from_static(bytes).tweak(egui::FontTweak { y_offset_factor: f, ..Default::default() });
            defs.font_data.insert(name.to_string(), Arc::new(fd));
        }
    }
    let fam = |v: &[(&str, bool)]| -> Vec<String> { v.iter().filter(|(_, ok)| *ok).map(|(n, _)| n.to_string()).collect() };
    let mut prop = fam(&[("cjk", cjk), ("sym", sym), ("sym2", sym2)]);
    prop.extend(defs.families[&FontFamily::Proportional].clone());
    let mut boldf = fam(&[("cjk_bold", cjk_b), ("cjk", cjk), ("sym", sym), ("sym2", sym2)]);
    boldf.extend(defs.families[&FontFamily::Proportional].clone());
    let mut monof = fam(&[("mono", mono), ("cjk_m", cjk), ("sym_m", sym), ("sym2", sym2)]);
    monof.extend(defs.families[&FontFamily::Monospace].clone());
    let mut monob = fam(&[("mono_b", mono_b), ("cjk_bold_m", cjk_b), ("sym_m", sym), ("sym2", sym2)]);
    monob.extend(defs.families[&FontFamily::Monospace].clone());
    defs.families.insert(FontFamily::Proportional, prop);
    defs.families.insert(FontFamily::Name("bold".into()), boldf);
    defs.families.insert(FontFamily::Monospace, monof.clone());
    defs.families.insert(FontFamily::Name("mono".into()), monof);
    defs.families.insert(FontFamily::Name("mono_bold".into()), monob);
    for (n, v) in [("btn", [("sym_btn", sym), ("cjk", cjk), ("sym2", sym2)]), ("lbl", [("cjk_lbl", cjk), ("sym", sym), ("sym2", sym2)])] {
        let mut f = fam(&v);
        f.extend(defs.families[&FontFamily::Monospace].clone());
        defs.families.insert(FontFamily::Name(n.into()), f);
    }
    (defs, cjk_data, cjk_b_data, sym_data)
}

type Seg = (String, Color32);

impl Overlay {
    fn status_lines(msg: &str, col: Color32) -> Vec<Vec<Seg>> {
        msg.split('\n')
            .map(|line| {
                if let Some(rest) = line.strip_prefix("自動讀取：") {
                    let mut segs = vec![("自動讀取：".to_string(), MUTED)];
                    for (i, part) in rest.split('、').enumerate() {
                        if i > 0 {
                            segs.push(("、".into(), MUTED));
                        }
                        let bad = part.contains("讀不到") || part.contains("失敗");
                        let c = if bad {
                            HP
                        } else if part.contains('✓') || part == "OK" {
                            GOOD
                        } else {
                            MUTED
                        };
                        segs.push((part.to_string(), c));
                    }
                    segs
                } else {
                    vec![(line.to_string(), col)]
                }
            })
            .collect()
    }

    fn status(&self) -> (String, Color32, bool, bool) {
        let t = &self.t;
        let now = self.now();
        let mut msg = t.ocr_msg.clone();
        let mut col = MUTED;
        let ocr_bad = msg.starts_with("自動讀取：") && (msg.contains("讀不到") || msg.contains("失敗"));
        let mut plain = true;
        if t.ocr_seen > 0.0 && now - t.ocr_seen > 20.0 && ["exp_region", "hp_region", "mp_region", "inv_region"].iter().any(|k| t.has(k)) {
            msg = "自動讀取停住了，按 ☰ 檢查".into();
            col = HP;
            plain = false;
        }
        if let Some(f) = &self.ocr_fatal {
            msg = format!("{f}\n（自動讀取都不能用，詳細內容在 error.log）");
            col = HP;
            plain = false;
        } else if !t.has("exp_region") {
            msg = "① 按 ☰ 改藥水名稱、價格，並「框選」畫面下方的 EXP 數字".into();
            col = WARN;
            plain = false;
            if !t.running {
                msg += &format!("\n② 按 ▶ 開始計時（{}）", platform::HOTKEY_NAMES[0]);
            }
        } else if !t.running {
            msg = format!("按 ▶ 開始計時（{}）\n{msg}", platform::HOTKEY_NAMES[0]);
        }
        for k in 0..2 {
            if t.counting() && t.cap_since[k].map_or(false, |c| now - c > 300.0) {
                msg += &format!(
                    "\n{}快捷欄一直停在 {}，即時看不到用量，\n請打開背包更新",
                    disp(&t.name(k)),
                    t.cfg.get("stack_max").map_or("".into(), |v| v.to_string())
                );
                col = WARN;
                plain = false;
            }
        }
        if t.running && t.has("meso_region") {
            if t.paused && t.seg_pending {
                msg += "\n暫停中：請打開背包確認楓幣";
                col = WARN;
                plain = false;
            } else if t.counting() && t.meso_anchor.is_none() && t.inc_done.is_some() {
                msg += "\n請打開背包確認楓幣";
                col = WARN;
                plain = false;
            }
        }
        if !self.hotkey_msg.is_empty() {
            msg += &format!("\n{}", self.hotkey_msg);
            plain = false;
        }
        if let Some((a, until)) = &t.alert {
            if now < *until {
                msg = a.clone();
                col = if a.starts_with("出錯") { HP } else { GOOD };
                plain = false;
            }
        }
        if let Some(h) = self.hover {
            msg = h.to_string();
            col = MP;
            plain = false;
        }
        (msg, col, plain, ocr_bad)
    }
}

struct View {
    timer: String,
    dot: Color32,
    running: bool,
    counting: bool,
    unit_lbl: String,
    recent_hdr: String,
    avg_hdr: String,
    pairs: Vec<(String, Color32, String, String, Color32)>,
    ones: Vec<(String, Color32, String)>,
    earn_col: Color32,
    compact: Vec<(String, String, Color32)>,
    show_meso: bool,
}

impl Overlay {
    fn view(&mut self) -> View {
        let s = self.t.tick();
        let t = &self.t;
        let a = s.active;
        let unit = t.cfg.get("unit").and_then(|v| v.as_f64()).unwrap_or(600.0);
        let uname = if unit == 600.0 { "每10分" } else { "每小時" };
        let span_name = if unit == 600.0 { "10分" } else { "1小時" };
        let early = a < unit;
        let (recent_hdr, avg_hdr) = if early {
            (format!("實際（{}）", dur(Some(a))), format!("預估{span_name}"))
        } else {
            (format!("近{span_name}"), "平均".to_string())
        };
        let price = |k: &str| t.cfg.get(&format!("{k}_price")).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let (hp_p, mp_p) = (price("hp"), price("mp"));
        let per = |delta: Option<f64>, secs: f64| -> Option<f64> {
            match delta {
                Some(d) if secs >= 60.0 => Some(d / secs * unit),
                _ => None,
            }
        };
        let (cur, base, dt) = t.recent(a, unit);
        let d = |c: Option<f64>, b: Option<f64>| -> Option<f64> {
            match (c, b) {
                (Some(c), Some(b)) => Some(c - b),
                _ => None,
            }
        };
        let base_v = base.map(|b| (Some(b.1 as f64), Some(b.2 as f64), Some(b.3 as f64), b.4.map(|x| x as f64)));
        let (bg, bh, bm, bi) = base_v.unwrap_or((None, None, None, None));
        let _ = bg;
        let r_hp = per(d(Some(cur.2 as f64), bh), dt);
        let r_mp = per(d(Some(cur.3 as f64), bm), dt);
        let r_cost = match (bh, bm) {
            (Some(h), Some(m)) => per(Some(t.span_cost(0, h as i64, t.used(0)) + t.span_cost(1, m as i64, t.used(1))), dt),
            _ => None,
        };
        let _ = (hp_p, mp_p);
        let r_inc = per(d(cur.4.map(|x| x as f64), bi), dt);
        let a_hp = per(Some(t.used(0) as f64), a);
        let a_mp = per(Some(t.used(1) as f64), a);
        let a_cost = per(Some(s.cost), a);
        let a_inc = per(s.income.map(|x| x as f64), t.income_secs(a));
        let sub = |x: Option<f64>, y: Option<f64>| match (x, y) {
            (Some(x), Some(y)) => Some(x - y),
            _ => None,
        };
        let exp_rates = (s.rate_recent.map(|r| r * unit / 3600.0), s.rate.map(|r| r * unit / 3600.0));
        let mut rows: Vec<(&str, Option<f64>, Option<f64>)> = vec![
            ("exp", exp_rates.0, exp_rates.1),
            ("hp", r_hp, a_hp),
            ("mp", r_mp, a_mp),
            ("cost", r_cost, a_cost),
            ("income", r_inc, a_inc),
            ("profit", sub(r_inc, r_cost), sub(a_inc, a_cost)),
        ];
        let rates = rows.clone();
        if early && t.running {
            let actual = [
                Some(t.gain as f64),
                Some(t.used(0) as f64),
                Some(t.used(1) as f64),
                Some(s.cost),
                s.income.map(|x| x as f64),
                s.profit,
            ];
            for (i, r) in rows.iter_mut().enumerate() {
                r.1 = actual[i];
            }
        }
        let waiting = if t.running { "累積中" } else { "–" };
        let need = t.need.filter(|&n| n != 0.0);
        let fmt = |k: &str, col: usize, val: Option<f64>| -> String {
            if col == 0 && early && t.running && (k == "hp" || k == "mp") {
                if let Some(v) = val {
                    return comma_f(v);
                }
            }
            match val {
                None => {
                    if !(k == "income" || k == "profit") || t.income().is_some() {
                        waiting.into()
                    } else if !t.inv_ready(None) {
                        "先框背包".into()
                    } else {
                        "開背包更新".into()
                    }
                }
                Some(v) if k == "exp" => short(Some(v)) + &need.map_or(String::new(), |n| format!(" {:.2}%", v * 100.0 / n)),
                Some(v) if k == "hp" || k == "mp" => {
                    if v < 10.0 {
                        format!("{v:.1}")
                    } else {
                        comma_f(v)
                    }
                }
                Some(v) => short(Some(v)),
            }
        };
        let label = |k: &str| -> (String, Color32) {
            match k {
                "exp" => ("EXP".into(), EXP),
                "hp" => (disp(&t.name(0)), HP),
                "mp" => (disp(&t.name(1)), MP),
                "cost" => ("藥水花費".into(), HP),
                "income" => ("撿錢".into(), EXP),
                _ => ("淨收益".into(), GOOD),
            }
        };
        let show_meso = t.has("meso_region");
        let mut pairs = vec![];
        for (k, rv, av) in &rows {
            if !show_meso && (*k == "income" || *k == "profit") {
                continue;
            }
            let (name, col) = label(k);
            let sign = |v: &Option<f64>| if v.map_or(false, |v| v < 0.0) { HP } else { GOOD };
            let (c0, c1) = if *k == "profit" { (sign(rv), sign(av)) } else { (col, col) };
            pairs.push((name, c0, fmt(k, 0, *rv), fmt(k, 1, *av), c1));
        }
        let extra = if t.levelups > 0 { format!("  升{}級", t.levelups) } else { String::new() };
        let mut ones = vec![("本次獲得".to_string(), INK, format!("{}{extra}", comma(t.gain)))];
        let (earn, earn_col) = if let Some(p) = s.profit {
            (format!("{}（撿到{}、藥水−{}）", short(Some(p)), short(s.income.map(|x| x as f64)), short(Some(s.cost))), if p >= 0.0 { GOOD } else { HP })
        } else if show_meso {
            (format!("藥水花了 {}（開背包讀楓幣後算收益）", short(Some(s.cost))), MUTED)
        } else {
            (format!("藥水花了 {}（☰ 框選背包楓幣就能算收益）", short(Some(s.cost))), MUTED)
        };
        ones.push(("本次收益".into(), earn_col, earn));
        let (mut lv_txt, mut lv_time) = ("–".to_string(), None);
        if let (Some((exp, _)), Some(n)) = (t.exp_last, need) {
            let rate = s.rate_recent.filter(|&r| r != 0.0).or(s.rate);
            lv_time = rate.filter(|&r| r > 0.0).map(|r| (n - exp as f64) / r * 3600.0);
            lv_txt = format!("{}  {:.2}%", dur(lv_time), exp as f64 * 100.0 / n);
        } else if let Some((exp, _)) = t.exp_last {
            lv_txt = comma(exp);
        }
        ones.push(("預估升級".into(), MP, lv_txt));
        for k in 0..2 {
            let hr = if k == 0 { s.hp_hr } else { s.mp_hr };
            let txt = if let Some(left) = t.est_total(k) {
                comma(left) + &hr.filter(|&h| h != 0.0).map_or(String::new(), |h| format!("  預計可用 {}", dur(Some(left as f64 / h * 3600.0))))
            } else if let Some(q) = t.qs[k] {
                format!("快捷欄 {q}（開背包看總數）")
            } else {
                "–".into()
            };
            ones.push((format!("{}剩", disp(&t.name(k))), if k == 0 { HP } else { MP }, txt));
        }
        let mut compact = vec![
            ("累積EXP".to_string(), short(Some(t.gain as f64)), EXP),
            ((if unit == 600.0 { "每十分" } else { "每小時" }).to_string(), short(rates[0].2), EXP),
            ("藥水花費".to_string(), short(rates[3].2), EXP),
        ];
        if let Some(p) = rates[5].2 {
            compact.push(("淨收益".to_string(), short(Some(p)), if p < 0.0 { HP } else { GOOD }));
        }
        compact.push(("預估升級".to_string(), dur(lv_time), EXP));
        View {
            timer: hms(a),
            dot: if t.counting() {
                GOOD
            } else if t.running {
                WARN
            } else {
                MUTED
            },
            running: t.running,
            counting: t.counting(),
            unit_lbl: format!("{uname} ⇄"),
            recent_hdr,
            avg_hdr,
            pairs,
            ones,
            earn_col,
            compact,
            show_meso,
        }
    }
}

impl Overlay {
    pub fn close_req(&mut self, kind: Kind) {
        match kind {
            Kind::Settings => {
                if let Some(s) = &mut self.settings {
                    s.close_req = true;
                }
            }
            Kind::Record => {
                if let Some(r) = &mut self.record {
                    r.close_req = true;
                }
            }
            _ => {}
        }
    }

    pub fn dialog_moved(&mut self, kind: Kind, pos: (i32, i32)) {
        match kind {
            Kind::Settings => {
                if let Some(s) = &mut self.settings {
                    s.pos = Some(pos);
                }
            }
            Kind::Record => {
                if let Some(r) = &mut self.record {
                    r.pos = Some(pos);
                }
            }
            _ => {}
        }
    }

    pub fn hwnd_of(&self, k: Kind) -> Option<isize> {
        self.hwnds.iter().find(|h| h.0 == k).map(|h| h.1)
    }

    pub fn overlay_visible(&self) -> bool {
        self.visible && self.select.is_none() && !self.auto.as_ref().map_or(false, |a| a.hiding())
    }

    pub fn zoom(&self) -> f32 {
        #[cfg(target_os = "macos")]
        let grow = ZOOM_MAX as f64;
        #[cfg(not(target_os = "macos"))]
        let grow = 1.0;
        let auto = || {
            let (_, _, _, sh) = platform::primary_screen();
            (sh as f64 / (1080.0 * self.scale)).min(grow)
        };
        let set = self.t.cfg.get("zoom_by_screen").and_then(|m| m.get(&zoom_key())).and_then(|x| x.as_f64());
        set.unwrap_or_else(auto).clamp(0.6, ZOOM_MAX as f64) as f32
    }

    pub fn dialog_zoom(&self) -> f32 {
        self.zoom().max(1.0)
    }

    pub fn set_zoom(&mut self, z: f32) {
        let mut m = self.t.cfg.get("zoom_by_screen").and_then(|m| m.as_object().cloned()).unwrap_or_default();
        m.insert(zoom_key(), (z as f64).into());
        self.t.cfg.insert("zoom_by_screen".into(), m.into());
        self.t.cfg.remove("zoom");
    }

    pub fn alpha(&self) -> f32 {
        self.t.cfg.get("alpha").and_then(|x| x.as_f64()).unwrap_or(0.88) as f32
    }

    fn home(&mut self) -> (i32, i32) {
        let (mut x, mut y) = (self.t.cfg_f("x") as i32, self.t.cfg_f("y") as i32);
        if !platform::on_screen(x + 30, y + 15) {
            x = 40;
            y = 40;
            self.t.cfg.insert("x".into(), x.into());
            self.t.cfg.insert("y".into(), y.into());
        }
        (x, y)
    }

    pub fn wanted(&mut self, overlay: Option<(i32, i32, i32, i32, f64)>) -> Vec<Spec> {
        let mut v = vec![];
        let (ox, oy) = self.home();
        let scale = overlay.map_or(1.0, |o| o.4);
        self.scale = scale;
        v.push(Spec {
            kind: Kind::Overlay,
            title: "經驗收益計算器",
            pos: (ox, oy),
            size: ((self.size.x as f64 * scale * self.zoom() as f64) as u32, (self.size.y as f64 * scale * self.zoom() as f64) as u32),
            decorations: false,
        });
        if self.select.is_some() {
            if let Some((x, y, w, h)) = crate::select::window_rect(self) {
                v.push(Spec { kind: Kind::Select, title: "框選", pos: (x, y), size: (w as u32, h as u32), decorations: false });
            }
            return v;
        }
        if self.auto.as_ref().map_or(false, |a| a.hiding()) {
            return v;
        }
        let picks = self.auto_pick.as_ref().map_or(0, |p| p.pairs.len() as u8);
        for i in (0..5).chain(HINT_PICK..HINT_PICK + picks).filter(|_| overlay.is_some()) {
            if let Some(g) = self.hint_geom(i) {
                v.push(Spec { kind: Kind::Hint(i), title: "讀取範圍", pos: g.pos, size: (g.size.0 as u32, g.size.1 as u32), decorations: false });
            }
        }
        let orect = overlay.map(|o| (o.0, o.1, o.2, o.3)).unwrap_or((ox, oy, 350, 400));
        let scale = scale * self.dialog_zoom() as f64;
        if let Some(s) = &mut self.settings {
            let size = ((s.size.x as f64 * scale) as i32, (s.size.y as f64 * scale) as i32);
            let pos = *s.pos.get_or_insert_with(|| crate::dialogs::place_near(orect, size, None));
            v.push(Spec { kind: Kind::Settings, title: crate::dialogs::SETTINGS_TITLE, pos, size: (size.0 as u32, size.1 as u32), decorations: true });
        }
        let srect = self.settings.as_ref().and_then(|s| s.pos.map(|p| (p.0, p.1, (s.size.x as f64 * scale) as i32, (s.size.y as f64 * scale) as i32)));
        if let Some(r) = &mut self.record {
            let size = ((r.size.x as f64 * scale) as i32, (r.size.y as f64 * scale) as i32);
            let pos = *r.pos.get_or_insert_with(|| crate::dialogs::place_near(orect, size, srect));
            v.push(Spec { kind: Kind::Record, title: crate::dialogs::RECORD_TITLE, pos, size: (size.0 as u32, size.1 as u32), decorations: true });
        }
        v
    }

    pub fn logic(&mut self, hwnds: &[(Kind, isize)], overlay_pos: Option<(i32, i32)>) {
        self.handle_msgs(hwnds);
        let now = self.now();
        if now - self.last_top > 1.5 && self.select.is_none() {
            self.last_top = now;
            self.check_screen();
            for (k, h) in hwnds {
                if *k != Kind::Overlay || self.visible {
                    platform::keep_top(*h);
                }
            }
        }
        if now - self.last_save > 60.0 {
            self.last_save = now;
            if self.t.running {
                self.t.save_state();
            }
        }
        if now - self.last_tick >= 0.5 && !self.overlay_visible() {
            self.last_tick = now;
            self.t.tick();
        }
        if self.t.reset_armed > 0.0 && now - self.t.reset_armed >= 3.0 {
            self.t.reset_armed = 0.0;
        }
        if let Some(pos) = overlay_pos.filter(|_| self.overlay_visible()) {
            if self.last_pos.map_or(false, |p| p != pos) {
                self.pos_changed_at = now;
            }
            self.last_pos = Some(pos);
            if self.pos_changed_at > 0.0 && now - self.pos_changed_at > 1.0 {
                self.pos_changed_at = 0.0;
                self.t.cfg.insert("x".into(), pos.0.into());
                self.t.cfg.insert("y".into(), pos.1.into());
                self.save_cfg();
            }
        }
        crate::select::step(self);
        crate::autoframe::step(self);
        self.flush_logs();
    }

    pub fn window_ui(&mut self, kind: Kind, ctx: &egui::Context, hwnd: isize) {
        sync_fonts(ctx);
        match kind {
            Kind::Overlay => self.overlay_ui(ctx, hwnd),
            Kind::Settings => crate::dialogs::settings_window(self, ctx, hwnd),
            Kind::Record => crate::dialogs::record_window(self, ctx, hwnd),
            Kind::Select => crate::select::select_window(self, ctx),
            Kind::Hint(i) => self.hint_ui(ctx, i),
        }
    }

    fn overlay_ui(&mut self, ctx: &egui::Context, hwnd: isize) {
        let ctx = ctx.clone();
        self.last_tick = self.now();
        let v = self.view();
        let alpha = self.t.cfg.get("alpha").and_then(|x| x.as_f64()).unwrap_or(0.88) as f32;
        let zoom = self.zoom();
        if (ctx.zoom_factor() - zoom).abs() > 0.001 {
            ctx.set_zoom_factor(zoom);
        }
        let rezoom = (ctx.zoom_factor() - self.zoom_seen).abs() > 0.001;
        self.zoom_seen = ctx.zoom_factor();
        let compact = self.t.cfg.get("compact").and_then(|x| x.as_bool()).unwrap_or(false);
        let (msg, col, plain, ocr_bad) = self.status();
        let now = self.now();
        if ocr_bad {
            self.bad_since.get_or_insert(now);
        } else {
            self.bad_since = None;
        }
        if self.bad_since.map_or(false, |t| now - t >= 3.0) {
            self.bad_hold = now + 3.0;
        }
        let ocr_bad = now < self.bad_hold;

        let mut hover: Option<&'static str> = None;
        let mut act: Option<&str> = None;
        let prompts = self.prompts();
        let mut pact: Option<(usize, PromptAct)> = None;
        let last_size = self.size;
        let area = egui::Area::new(egui::Id::new("main")).fixed_pos(egui::Pos2::ZERO).constrain(false).order(egui::Order::Background);
        let resp = area.show(&ctx, |ui| {
            ui.multiply_opacity(alpha);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            let drag = ui.interact(egui::Rect::from_min_size(egui::Pos2::ZERO, last_size), egui::Id::new("drag"), Sense::click_and_drag());
            if drag.drag_started() {
                self.drag_from = Some((platform::cursor_pos(), platform::window_pos(hwnd)));
            }
            if let Some(((cx, cy), (wx, wy))) = self.drag_from {
                if drag.dragged() {
                    let (x, y) = platform::cursor_pos();
                    platform::move_quiet(hwnd, wx + x - cx, wy + y - cy);
                } else {
                    self.drag_from = None;
                }
            }
            egui::Frame::new()
            .fill(BG)
            .stroke(egui::Stroke::new(1.0, LINE))
            .corner_radius(OVERLAY_RADIUS)
            .inner_margin(egui::Margin { left: 11, right: 11, top: 9, bottom: 9 })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                ui.horizontal(|ui| {
                    let dot_x = ui.cursor().min.x + 3.5;
                    ui.add_space(12.0);
                    let tr = ui.label(RichText::new(&v.timer).font(mono(15.0, true)).color(INK));
                    ui.painter().circle_filled(egui::pos2(dot_x, tr.rect.center().y - ICON_DY), 3.2, v.dot);
                    ui.add_space(10.0);
                    for k in ["run", "set", "reset", "compact", "close"] {
                        let armed = k == "reset" && self.t.reset_armed > 0.0;
                        let id = ui.next_auto_id();
                        let hovered = ui.ctx().read_response(id).map_or(false, |r| r.hovered());
                        let c = if hovered {
                            INK
                        } else if armed {
                            HP
                        } else {
                            MUTED
                        };
                        ui.add_space(3.0);
                        let (r, painter) = ui.allocate_painter(egui::vec2(ICON_BOX, ICON_BOX), Sense::click());
                        let up = r.rect.translate(egui::vec2(0.0, -ICON_DY));
                        draw_icon(&painter.with_clip_rect(r.rect.union(up)), up, k, v.counting, c);
                        ui.add_space(3.0);
                        let hk = if k == "compact" && compact { "normal" } else { k };
                        let hint = btn_hints().iter().find(|h| h.0 == hk).map(|h| h.1.as_str());
                        let (cx, cy) = platform::cursor_pos();
                        let (wx, wy) = platform::window_pos(hwnd);
                        let ppp = ui.ctx().pixels_per_point();
                        let inside = r.rect.contains(egui::pos2((cx - wx) as f32 / ppp, (cy - wy) as f32 / ppp));
                        if r.hovered() || (inside && hint.is_some() && self.hover == hint) {
                            hover = hint;
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if r.clicked() {
                            act = Some(k);
                        }
                    }
                });
                if compact {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (i, (name, val, vc)) in v.compact.iter().enumerate() {
                            if i > 0 {
                                ui.add_space(14.0);
                            }
                            let unit = i == 1;
                            let f = |color| egui::TextFormat { font_id: mono(10.0, true), color, ..Default::default() };
                            let mut job = egui::text::LayoutJob::default();
                            job.append(&if unit { format!("{name} ⇄") } else { name.clone() }, 0.0, f(if unit { MP } else { MUTED }));
                            job.append(&format!(" {val}"), 0.0, f(*vc));
                            let r = ui.add(egui::Label::new(job).sense(if unit { Sense::click() } else { Sense::hover() }));
                            if unit {
                                if r.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if r.clicked() {
                                    act = Some("unit");
                                }
                            }
                        }
                    });
                } else {
                    ui.add_space(6.0);
                    egui::Grid::new("rows").num_columns(3).spacing([14.0, 2.0]).show(ui, |ui| {
                        let h1 = ui.add(egui::Label::new(RichText::new(&v.unit_lbl).font(font(8.0)).color(MP)).sense(Sense::click()));
                        let h2 = ui.add(egui::Label::new(RichText::new(&v.recent_hdr).font(font(8.0)).color(MUTED)).sense(Sense::click()));
                        let h3 = ui.add(egui::Label::new(RichText::new(&v.avg_hdr).font(font(8.0)).color(MUTED)).sense(Sense::click()));
                        if h1.clicked() || h2.clicked() || h3.clicked() {
                            act = Some("unit");
                        }
                        ui.end_row();
                        for (name, c0, rv, av, c1) in &v.pairs {
                            ui.label(RichText::new(name).font(lbl_font(9.0)).color(MUTED));
                            ui.label(RichText::new(rv).font(mono(11.0, true)).color(*c0));
                            ui.label(RichText::new(av).font(mono(11.0, false)).color(*c1));
                            ui.end_row();
                        }
                    });
                    ui.add_space(4.0);
                    let w = ui.min_rect().width().max(200.0);
                    let (r, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), Sense::hover());
                    ui.painter().rect_filled(r, 0.0, LINE);
                    ui.add_space(4.0);
                    egui::Grid::new("ones").num_columns(2).spacing([10.0, 2.0]).show(ui, |ui| {
                        for (name, col, txt) in &v.ones {
                            ui.label(RichText::new(name).font(lbl_font(9.0)).color(MUTED));
                            let c = if name == "本次收益" { v.earn_col } else { *col };
                            ui.label(RichText::new(txt).font(mono(11.0, true)).color(c));
                            ui.end_row();
                        }
                    });
                }
                for (k, text, btns) in &prompts {
                    ui.add_space(6.0);
                    egui::Frame::new().fill(PANEL).stroke(egui::Stroke::new(1.0, WARN)).inner_margin(egui::Margin::symmetric(6, 5)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                        ui.label(RichText::new(text).font(font(9.0)).color(WARN));
                        for row in btns.chunks(4) {
                            ui.horizontal(|ui| {
                                for (lab, a, hi) in row {
                                    let b = egui::Button::new(RichText::new(lab).font(font(9.0)).color(if *hi { BG } else { INK }))
                                        .fill(if *hi { WARN } else { LINE })
                                        .corner_radius(2.0);
                                    if ui.add(b).clicked() {
                                        pact = Some((*k, a.clone()));
                                    }
                                }
                            });
                        }
                    });
                }
                let want = !compact || (!plain && col != MUTED) || ocr_bad;
                if want {
                    ui.add_space(if compact { 4.0 } else { 6.0 });
                    for line in Self::status_lines(&msg, col) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (txt, c) in line {
                                ui.label(RichText::new(txt).font(font(8.0)).color(c));
                            }
                        });
                    }
                }
            });
        });
        let _ = (v.running, v.show_meso);
        let size = resp.response.rect.size() + egui::vec2(1.0, 1.0);
        if rezoom || (size - self.size).length() > 0.5 {
            self.size = size;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
        }
        if hover != self.hover {
            self.hover = hover;
        }
        if let Some((k, a)) = pact {
            self.prompt_action(k, a);
        }
        match act {
            Some("run") => self.t.toggle_run(0.0),
            Some("set") => self.open_settings(),
            Some("reset") => self.t.ask_reset(),
            Some("compact") => {
                let c = !compact;
                self.t.cfg.insert("compact".into(), c.into());
                self.save_cfg();
            }
            Some("close") => {
                self.quit();
            }
            Some("unit") => {
                let u = if self.t.cfg.get("unit").and_then(|x| x.as_f64()).unwrap_or(600.0) == 600.0 { 3600 } else { 600 };
                self.t.cfg.insert("unit".into(), u.into());
                self.save_cfg();
            }
            _ => {}
        }
        self.flush_logs();
    }
}

pub fn spawn_worker(paths: &Paths, shared: &Arc<Shared>, tx: std::sync::mpsc::Sender<Msg>, wake: impl Fn() + Send + 'static) {
    worker::spawn(paths.clone(), shared.clone(), tx, wake);
}
