use crate::platform;
use crate::potions;
use crate::ui::*;
use egui::{self, Color32, RichText, ViewportCommand};
use serde_json::Value;
use tracker::util::{bag_count, parse_count, parse_exp, parse_input, parse_meso};
use tracker::Tracker;

pub const SETTINGS_TITLE: &str = "經驗收益計算器設定";
pub const RECORD_TITLE: &str = "記錄目前數值";

const FIELDS: [(&str, &str); 6] = [
    ("hp_name", "紅水名稱"),
    ("hp_price", "紅水單價"),
    ("mp_name", "藍水名稱"),
    ("mp_price", "藍水單價"),
    ("ocr_sec", "自動讀取間隔（秒）"),
    ("auto_pause", "幾分鐘沒經驗就自動暫停（0＝關閉）"),
];

fn val_str(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

pub struct Settings {
    pub vals: Vec<String>,
    pub bad: Vec<bool>,
    pub msg: String,
    pub alpha: f32,
    pub zoom: f32,
    pub focus: bool,
    pub pos: Option<(i32, i32)>,
    pub size: egui::Vec2,
    numeric_focus: bool,
    pub close_req: bool,
    zoom_seen: f32,
}

impl Settings {
    pub fn new(t: &Tracker) -> Settings {
        Settings {
            vals: FIELDS.iter().map(|(k, _)| val_str(t.cfg.get(*k))).collect(),
            bad: vec![false; FIELDS.len()],
            msg: String::new(),
            alpha: t.cfg.get("alpha").and_then(|v| v.as_f64()).unwrap_or(0.88) as f32,
            zoom: 1.0,
            focus: true,
            pos: None,
            size: egui::vec2(520.0, 560.0),
            numeric_focus: false,
            close_req: false,
            zoom_seen: 1.0,
        }
    }
}

pub struct Record {
    pub vals: [String; 4],
    pub bad: [bool; 4],
    pub msg: String,
    pub focus: bool,
    prev_fg: Option<isize>,
    pub pos: Option<(i32, i32)>,
    pub size: egui::Vec2,
    first: bool,
    pub close_req: bool,
    zoom_seen: f32,
}

impl Record {
    pub fn new(_t: &Tracker, prev_fg: Option<isize>) -> Record {
        Record {
            vals: Default::default(),
            bad: [false; 4],
            msg: String::new(),
            focus: true,
            prev_fg,
            pos: None,
            size: egui::vec2(380.0, 260.0),
            first: true,
            zoom_seen: 1.0,
            close_req: false,
        }
    }
}

pub fn place_near(r: (i32, i32, i32, i32), size: (i32, i32), avoid: Option<(i32, i32, i32, i32)>) -> (i32, i32) {
    let (_, _, sw, sh) = platform::primary_screen();
    let sh = sh - 100;
    let frame = 80;
    let (ww, wh) = (size.0 + 32, size.1 + frame);
    let (rx, ry, rw, rh) = r;
    let mut rects = vec![r];
    if let Some(a) = avoid {
        rects.push((a.0, a.1, a.2 + 32, a.3 + frame));
    }
    let side_y = ry.min(sh - wh).max(0);
    let mut spots = vec![(rx, ry + rh + 16), (rx, ry - wh - 16), (rx + rw + 16, side_y), (rx - ww - 16, side_y)];
    for a in &rects[1..] {
        spots.push((a.0 + a.2 + 16, a.1.min(sh - wh).max(0)));
        spots.push((a.0 - ww - 16, a.1.min(sh - wh).max(0)));
    }
    for (x, y) in spots {
        let x = x.min(sw - ww).max(0);
        if y < 0 || y + wh > sh {
            continue;
        }
        if rects.iter().all(|(ax, ay, aw, ah)| x >= ax + aw || x + ww <= *ax || y >= ay + ah || y + wh <= *ay) {
            return (x, y);
        }
    }
    (rx.min(sw - ww).max(0), ry.min(sh - wh).max(0))
}

fn entry(ui: &mut egui::Ui, s: &mut String, width: f32, bad: bool, enabled: bool) -> egui::Response {
    let stroke = if bad { egui::Stroke::new(2.0, HP) } else { egui::Stroke::new(1.0, LINE) };
    egui::Frame::new()
        .stroke(stroke)
        .corner_radius(WIDGET_RADIUS)
        .fill(if enabled { PANEL } else { BG })
        .inner_margin(egui::Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                ui.add_sized(
                    [width, pt(12.0) + 4.0],
                    egui::TextEdit::singleline(s).font(mono(12.0, false)).text_color(INK).frame(egui::Frame::new()),
                )
            })
            .inner
        })
        .inner
}

fn small(text: &str, col: Color32) -> RichText {
    RichText::new(text).font(font(8.0)).color(col)
}

fn button(ui: &mut egui::Ui, text: &str, fill: Color32, fg: Color32, size: f32, pad: f32) -> bool {
    ui.add(egui::Button::new(RichText::new(text).font(bold(size)).color(fg)).fill(fill).corner_radius(WIDGET_RADIUS).min_size(egui::vec2(pad, 0.0)))
        .clicked()
}

fn region_label(o: &Overlay, k: &str) -> (String, bool) {
    let t = &o.t;
    if let Some(p) = k.strip_suffix("_tpl") {
        let i = if p == "hp" { 0 } else { 1 };
        let ok = o.paths.tpl[i].exists();
        return ((if ok { "已記住" } else { "未設定" }).into(), ok);
    }
    if k == "inv" {
        let ok = t.has("inv_region");
        return ((if ok { "已設定" } else { "未設定" }).into(), ok);
    }
    if t.has(&format!("{k}_region")) {
        let raw = t.raw.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
        let ok = raw.is_empty()
            || match k {
                "exp" => parse_exp(&raw).is_some(),
                "meso" => parse_meso(&raw).is_some(),
                _ => parse_count(&raw).is_some(),
            };
        let shown = match k {
            "hp" | "mp" => parse_count(&raw).map(|n| n.to_string()),
            _ => None,
        };
        let txt = if raw.is_empty() {
            "已設定，讀取中…".to_string()
        } else if !ok {
            "讀取失敗，請重新框選".to_string()
        } else {
            format!("讀到：{}", shown.unwrap_or_else(|| raw.chars().take(26).collect()))
        };
        return (txt, ok);
    }
    ("未設定".into(), false)
}

pub fn settings_window(o: &mut Overlay, ctx: &egui::Context, hwnd: isize) {
    if let Some(mut s) = o.settings.take() {
        if settings_ui(o, &mut s, ctx, hwnd) {
            o.settings = Some(s);
        }
    }
}

pub fn record_window(o: &mut Overlay, ctx: &egui::Context, hwnd: isize) {
    if let Some(mut r) = o.record.take() {
        if record_ui(o, &mut r, ctx, hwnd) {
            o.record = Some(r);
        } else {
            o.last_record_fg = r.prev_fg;
        }
    }
}

fn settings_ui(o: &mut Overlay, s: &mut Settings, ctx: &egui::Context, hwnd: isize) -> bool {
    let ctx = ctx.clone();
    let rezoom = dialog_zoom(o, &ctx, &mut s.zoom_seen);
    if s.focus {
        s.focus = false;
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        platform::force_focus(hwnd);
    }
    let close = std::mem::take(&mut s.close_req);
    let mut save = false;
    let mut pick: Option<(usize, String, f64)> = None;
    let mut forget: Option<(usize, String)> = None;
    let mut select: Option<String> = None;
    let mut auto = false;
    let mut clear: Option<String> = None;
    let (_, _, _, sh) = platform::primary_screen();
    let maxh = (sh as f32 / ctx.pixels_per_point() - 260.0).max(300.0);
    let resp = egui::Area::new(egui::Id::new("dlg")).fixed_pos(egui::Pos2::ZERO).constrain(false).show(&ctx, |ui| {
        egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
            ui.style_mut().interaction.selectable_labels = false;
            egui::ScrollArea::vertical().max_height(maxh).auto_shrink([false, true]).show(ui, |ui| {
            let mut numeric_focus = false;
            egui::Grid::new("fields").num_columns(3).spacing([10.0, 4.0]).show(ui, |ui| {
                for (i, (k, lab)) in FIELDS.iter().enumerate() {
                    ui.label(small(lab, MUTED).font(font(9.0)));
                    let r = entry(ui, &mut s.vals[i], 110.0, s.bad[i], true);
                    if k.ends_with("_name") && s.vals[i].chars().count() > NAME_MAX {
                        s.vals[i] = s.vals[i].chars().take(NAME_MAX).collect();
                    }
                    if r.has_focus() && !k.ends_with("_name") {
                        numeric_focus = true;
                    }
                    if k.ends_with("_name") {
                        let kk = if k.starts_with("hp") { 0 } else { 1 };
                        let cur = s.vals[i].trim().to_string();
                        let list: Vec<(String, f64)> = o.book.of(kk).iter().map(|p| (p.name.clone(), p.price)).collect();
                        egui::ComboBox::from_id_salt(k)
                            .selected_text(RichText::new(if cur.is_empty() { "–".to_string() } else { disp(&cur) }).font(font(9.0)).color(INK))
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                for (n, pr) in &list {
                                    let on = *n == cur;
                                    ui.horizontal(|ui| {
                                        if ui.selectable_label(on, RichText::new(n).font(font(9.0))).on_hover_text("換成這瓶").clicked() {
                                            pick = Some((i, n.clone(), *pr));
                                        }
                                        if !on && n.as_str() != o.t.name(kk) {
                                            let x = egui::Button::new(RichText::new("×").font(font(8.0)).color(MUTED)).frame(false);
                                            if ui.add(x).on_hover_text(format!("從清單刪掉{n}")).clicked() {
                                                forget = Some((kk, n.clone()));
                                            }
                                        }
                                    });
                                }
                            });
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
                ui.label(small("浮窗透明度", MUTED).font(font(9.0)));
                let r = ui.add(egui::Slider::new(&mut s.alpha, 0.4..=1.0).step_by(0.02).show_value(false));
                if r.changed() {
                    o.t.cfg.insert("alpha".into(), (s.alpha as f64).into());
                }
                ui.end_row();
                ui.label(small("浮窗大小", MUTED).font(font(9.0)));
                ui.horizontal(|ui| {
                    let r = ui.add(egui::Slider::new(&mut s.zoom, 0.6..=ZOOM_MAX).step_by(0.05).show_value(false));
                    ui.label(RichText::new(format!("{:.0}%", s.zoom * 100.0)).font(mono(9.0, false)).color(MUTED));
                    if r.changed() {
                        o.set_zoom(s.zoom);
                    }
                });
                ui.end_row();
            });
            s.numeric_focus = numeric_focus;
            let (hp, mp) = (o.t.name(0), o.t.name(1));
            let sections: [(&str, Option<&str>, Vec<(&str, String, Color32)>); 3] = [
                ("自動讀取 EXP", None, vec![("exp", "下方 EXP 數字".into(), EXP)]),
                ("藥水即時扣量（快捷欄上的數字）", None, vec![("hp", format!("{hp} 數量"), HP), ("mp", format!("{mp} 數量"), MP)]),
                (
                    "藥水總數校正（打開背包時自動讀取）",
                    Some("請先打開背包、切到消耗欄，再按下面的「框選」"),
                    vec![
                        ("inv", "背包範圍".into(), INK),
                        ("hp_tpl", format!("背包裡一格{hp}"), HP),
                        ("mp_tpl", format!("背包裡一格{mp}"), MP),
                        ("meso", "背包裡的楓幣數字".into(), EXP),
                    ],
                ),
            ];
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if button(ui, "自動框選", MP, Color32::from_rgb(0x0b, 0x14, 0x26), 10.0, 0.0) {
                    auto = true;
                }
                ui.add_space(6.0);
                ui.label(small("一次找出下面全部的位置（請先打開背包切到消耗欄並將藥水放到右下快捷欄）", MUTED).font(font(9.0)));
            });
            for (title, hint, items) in sections {
                ui.add_space(10.0);
                ui.label(RichText::new(title).font(bold(10.0)).color(INK));
                if let Some(h) = hint {
                    ui.label(small(h, WARN).font(font(9.0)));
                }
                egui::Grid::new(title).num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
                    for (k, lab, col) in items {
                        ui.label(small(&lab, col).font(font(9.0)));
                        ui.horizontal(|ui| {
                            if button(ui, "框選", PANEL, INK, 9.0, 0.0) {
                                select = Some(k.to_string());
                            }
                            if button(ui, "清除", PANEL, INK, 9.0, 0.0) {
                                clear = Some(k.to_string());
                            }
                            let (txt, ok) = region_label(o, k);
                            ui.add_space(4.0);
                            ui.label(RichText::new(txt).font(mono(9.0, false)).color(if ok { GOOD } else { HP }));
                        });
                        ui.end_row();
                    }
                });
            }
            if o.t.inv_ready(None) {
                let txt = if o.t.inv_msg.is_empty() {
                    "背包：還沒看到，請打開背包（消耗欄）".to_string()
                } else {
                    format!("背包：{}", o.t.inv_msg.replace('｜', "\n　　　"))
                };
                ui.add_space(4.0);
                ui.label(RichText::new(txt).font(font(9.0)).color(GOOD));
            }
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(&s.msg).font(bold(9.0)).color(HP));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if button(ui, "儲存", MP, Color32::from_rgb(0x0b, 0x14, 0x26), 10.0, 60.0) {
                        save = true;
                    }
                });
            });
        });
    });
    if s.numeric_focus {
        ctx.output_mut(|out| out.ime = None);
    }
    fit(&ctx, &mut s.size, resp.response.rect.size(), rezoom);
    if let Some((i, n, pr)) = pick {
        s.vals[i] = n;
        s.vals[i + 1] = if pr == pr.trunc() { format!("{}", pr as i64) } else { format!("{pr}") };
        s.bad[i + 1] = false;
        apply(o, s, false);
    }
    if let Some((kk, n)) = forget {
        o.book.remove(kk, &n);
        o.book.save();
    }
    if let Some(k) = clear {
        clear_region(o, &k);
    }
    if select.is_some() || auto {
        if !apply(o, s, false) {
            return true;
        }
    }
    if let Some(k) = select {
        crate::select::begin(o, &k);
        return true;
    }
    if auto {
        crate::autoframe::begin(o);
        return true;
    }
    if save || close {
        return !apply(o, s, close) && !close;
    }
    true
}

fn apply(o: &mut Overlay, s: &mut Settings, close: bool) -> bool {
    let mut new: Vec<(String, Value)> = vec![];
    let mut any_bad = false;
    for (i, (k, _)) in FIELDS.iter().enumerate() {
        let text = s.vals[i].trim().to_string();
        if k.ends_with("_name") {
            let d = tracker::default_cfg()[*k].clone();
            new.push((k.to_string(), if text.is_empty() { d } else { Value::String(text) }));
            s.bad[i] = false;
            continue;
        }
        match parse_input(&text) {
            Ok(None) => s.bad[i] = false,
            Ok(Some(n)) if n > 0.0 || (n == 0.0 && *k == "auto_pause") => {
                s.bad[i] = false;
                let v: Value = if n == n.trunc() { (n as i64).into() } else { n.into() };
                new.push((k.to_string(), v));
            }
            _ => {
                s.bad[i] = true;
                any_bad = true;
            }
        }
    }
    if any_bad && !close {
        s.msg = "紅框的數字看不懂（要大於 0，自動暫停可以填 0）。按 Shift 切成英數再打".into();
        return false;
    }
    let newv = |key: &str| new.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).or_else(|| o.t.cfg.get(key).cloned());
    let want: Vec<(String, f64)> = (0..2)
        .map(|kk| {
            let kind = potions::KIND[kk];
            (
                newv(&format!("{kind}_name")).and_then(|v| v.as_str().map(|x| x.to_string())).unwrap_or_default(),
                newv(&format!("{kind}_price")).and_then(|v| v.as_f64()).unwrap_or(0.0),
            )
        })
        .collect();
    let first = !o.names_ok();
    for (kk, (name, price)) in want.into_iter().enumerate() {
        if !first && !name.is_empty() && name != o.t.name(kk) {
            let now = o.now();
            let mark = o.watch[kk].mark_hint.take().filter(|(_, t)| now - t < 900.0).map(|(m, _)| m);
            o.switch_potion(kk, &name, price, mark, true);
        }
    }
    for (k, v) in new {
        o.t.cfg.insert(k, v);
    }
    o.t.cfg.insert("names_ok".into(), true.into());
    for kk in 0..2 {
        let (name, price) = (o.t.name(kk), o.t.price(kk));
        o.book.upsert(kk, &name, price);
        if first {
            if o.paths.tpl[kk].exists() {
                o.book.store_tpl(kk, &name, &o.paths.tpl[kk]);
            }
            let r = o.book.find(kk, &name).and_then(|p| p.qs.clone());
            o.watch[kk].reset(r);
        }
    }
    o.book.save();
    o.t.cfg.insert("alpha".into(), (s.alpha as f64).into());
    o.save_cfg();
    s.msg.clear();
    true
}

fn fit(ctx: &egui::Context, size: &mut egui::Vec2, want: egui::Vec2, force: bool) {
    let want = want.ceil();
    if force || (want - *size).length() > 1.0 {
        *size = want;
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(want));
    }
}

fn dialog_zoom(o: &Overlay, ctx: &egui::Context, seen: &mut f32) -> bool {
    let z = o.dialog_zoom();
    if !ctx.input(|i| i.pointer.any_down()) && (ctx.zoom_factor() - z).abs() > 0.001 {
        ctx.set_zoom_factor(z);
    }
    let re = (ctx.zoom_factor() - *seen).abs() > 0.001;
    *seen = ctx.zoom_factor();
    re
}

fn record_ui(o: &mut Overlay, r: &mut Record, ctx: &egui::Context, hwnd: isize) -> bool {
    let ctx = ctx.clone();
    let rezoom = dialog_zoom(o, &ctx, &mut r.zoom_seen);
    if r.focus {
        r.focus = false;
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        platform::force_focus(hwnd);
    }
    let close = std::mem::take(&mut r.close_req) || ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let mut submit = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let exp_auto = o.t.has("exp_region");
    let inv_ready = o.t.inv_ready(None);
    let rows = [
        ("EXP".to_string(), EXP),
        ("EXP %".to_string(), EXP),
        (format!("{} 總數", o.t.name(0)), HP),
        (format!("{} 總數", o.t.name(1)), MP),
    ];
    let mut first_id: Option<egui::Id> = None;
    let resp = egui::Area::new(egui::Id::new("dlg")).fixed_pos(egui::Pos2::ZERO).constrain(false).show(&ctx, |ui| {
        egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
            ui.style_mut().interaction.selectable_labels = false;
            egui::Grid::new("rec").num_columns(3).spacing([10.0, 6.0]).show(ui, |ui| {
                for (i, (lab, col)) in rows.iter().enumerate() {
                    ui.label(RichText::new(lab).font(bold(10.0)).color(*col));
                    let enabled = !(i < 2 && exp_auto);
                    let resp = entry(ui, &mut r.vals[i], 180.0, r.bad[i], enabled);
                    if enabled && first_id.is_none() {
                        first_id = Some(resp.id);
                    }
                    if !enabled {
                        ui.label(small("自動讀取中", MUTED));
                    } else if i >= 2 && inv_ready {
                        ui.label(small("開背包會自動讀", MUTED));
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
            let mut hint = "背包裡有好幾格就用 + 加起來，例如 3000+3000+1184\n空白的欄位會跳過。Enter 送出、Esc 取消。".to_string();
            if !exp_auto && o.t.exp_last.is_some() && o.t.need.is_none() {
                hint = format!("剛升級過，這次請把 EXP % 也填上\n{hint}");
            }
            ui.add_space(6.0);
            ui.label(small(&hint, MUTED));
            ui.label(RichText::new(&r.msg).font(bold(9.0)).color(HP));
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if button(ui, "記錄", MP, Color32::from_rgb(0x0b, 0x14, 0x26), 10.0, 60.0) {
                    submit = true;
                }
            });
        });
    });
    if r.first {
        r.first = false;
        if let Some(id) = first_id {
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }
    ctx.output_mut(|out| out.ime = None);
    fit(&ctx, &mut r.size, resp.response.rect.size(), rezoom);
    if close {
        return false;
    }
    if !submit {
        return true;
    }
    let mut vals: [Option<f64>; 4] = [None; 4];
    let mut any_bad = false;
    for i in 0..4 {
        if i < 2 && exp_auto {
            continue;
        }
        match parse_input(&r.vals[i]) {
            Ok(v) if i == 1 && v.map_or(false, |p| !(0.0..=100.0).contains(&p)) => {
                r.bad[i] = true;
                any_bad = true;
            }
            Ok(v) => {
                r.bad[i] = false;
                vals[i] = v;
            }
            Err(_) => {
                r.bad[i] = true;
                any_bad = true;
            }
        }
    }
    if any_bad {
        r.msg = "紅框的數字看不懂。輸入法在中文模式的話，按 Shift 切成英數再打".into();
        return true;
    }
    let round = |v: Option<f64>| v.map(|x| x.round_ties_even() as i64);
    match o.t.submit_record(round(vals[0]), vals[1], [round(vals[2]), round(vals[3])]) {
        Ok(()) => false,
        Err(m) => {
            r.bad[1] = true;
            r.msg = m;
            true
        }
    }
}

pub fn forget_potion(t: &mut Tracker, k: usize) {
    t.forget_potion(k);
}

pub fn clear_region(o: &mut Overlay, k: &str) {
    if let Some(p) = k.strip_suffix("_tpl") {
        let i = if p == "hp" { 0 } else { 1 };
        let _ = std::fs::remove_file(&o.paths.tpl[i]);
        forget_potion(&mut o.t, i);
        let name = o.t.name(i);
        o.book.drop_tpl(i, &name);
        o.book.save();
        o.refresh_tpl();
    } else {
        o.t.cfg.insert(format!("{k}_region"), Value::Null);
        o.t.cfg.remove(&format!("{k}_screen"));
        o.t.raw.insert(k.into(), String::new());
        o.save_cfg();
    }
    if k == "hp" || k == "mp" {
        let i = if k == "hp" { 0 } else { 1 };
        o.t.forget_qs(i);
        o.watch[i].reset(None);
        o.watch[i].check = None;
        o.watch[i].prompt = None;
    }
    if k == "exp" {
        o.t.exp_last = None;
        o.t.need = None;
        o.t.pend_first = None;
    }
    if matches!(k, "inv" | "hp_tpl" | "mp_tpl") {
        o.t.inv_msg.clear();
    }
    let _ = bag_count;
}
