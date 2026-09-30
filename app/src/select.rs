use crate::dialogs::forget_potion;
use crate::platform;
use crate::ui::*;
use egui::{self, Color32};
use ocr::img::Bgr;

pub struct Select {
    kind: String,
    start: f64,
    shot: Option<(Bgr, Option<egui::TextureHandle>)>,
    virt: (i32, i32, i32, i32),
    drag: Option<egui::Pos2>,
    settings_open: bool,
    done: Option<Option<(i32, i32, i32, i32)>>,
}

pub fn begin(o: &mut Overlay, kind: &str) {
    if o.select.is_some() {
        return;
    }
    o.select = Some(Select {
        kind: kind.into(),
        start: o.now(),
        shot: None,
        virt: platform::virtual_screen(),
        drag: None,
        settings_open: o.settings.is_some(),
        done: None,
    });
}

pub fn window_rect(o: &Overlay) -> Option<(i32, i32, i32, i32)> {
    let s = o.select.as_ref()?;
    s.shot.as_ref()?;
    Some(s.virt)
}

fn names(o: &Overlay, kind: &str) -> (String, &'static str) {
    let (hp, mp) = (o.t.name(0), o.t.name(1));
    match kind {
        "exp" => ("畫面下方的 EXP 數字（含 %）".into(), "只框數字那一行就好"),
        "hp" => (format!("快捷欄上{hp}的數量"), "只框數字那一行就好"),
        "mp" => (format!("快捷欄上{mp}的數量"), "只框數字那一行就好"),
        "inv" => ("整個背包的格子區域".into(), "框消耗欄範圍，涵蓋自定義的所有藥水"),
        "hp_tpl" => (format!("背包裡「一格」{hp}"), "從圖示上緣框到數字下緣，左右框滿那一格"),
        "mp_tpl" => (format!("背包裡「一格」{mp}"), "從圖示上緣框到數字下緣，左右框滿那一格"),
        _ => ("背包下方的楓幣數字".into(), "只框數字那一行就好"),
    }
}

pub fn step(o: &mut Overlay) {
    let now = o.now();
    let Some(sel) = o.select.as_mut() else { return };
    if sel.shot.is_none() && now - sel.start >= 0.3 {
        #[cfg(windows)]
        let img = ocr::capture::Grabber::new().and_then(|g| g.grab(sel.virt.0, sel.virt.1, sel.virt.2, sel.virt.3));
        #[cfg(not(windows))]
        let img: Option<Bgr> = None;
        match img {
            Some(img) => sel.shot = Some((img, None)),
            None => sel.done = Some(None),
        }
        if sel.shot.is_none() {
            o.t.say("出錯了：截不到整個畫面", 10.0);
        }
    }
    let sel = o.select.as_mut().unwrap();
    if sel.done.is_none() && now - sel.start > 120.0 {
        sel.done = Some(None);
    }
    let Some(res) = sel.done else { return };
    let s = o.select.take().unwrap();
    let kind = s.kind.clone();
    let (name, _) = names(o, &kind);
    if let (Some((x, y, w, h)), Some((img, _))) = (res, &s.shot) {
        let (vx, vy, _, _) = s.virt;
        if let Some(p) = kind.strip_suffix("_tpl") {
            let i = if p == "hp" { 0 } else { 1 };
            let (cx, cy) = ((x - vx).max(0) as usize, (y - vy).max(0) as usize);
            let (cw, ch) = ((w as usize).min(img.w - cx), (h as usize).min(img.h - cy));
            let crop = reader::cv::crop_bgr(img, cx, cy, cx + cw, cy + ch);
            if image_rgb(&crop).save(&o.paths.tpl[i]).is_ok() {
                o.t.inv_msg.clear();
                forget_potion(&mut o.t, i);
                if o.names_ok() {
                    let (n, pr) = (o.t.name(i), o.t.price(i));
                    o.book.upsert(i, &n, pr);
                    o.book.store_tpl(i, &n, &o.paths.tpl[i]);
                    o.book.save();
                }
                o.refresh_tpl();
                o.t.say(format!("已框選：{name}"), 5.0);
            } else {
                o.t.say("出錯了：存不了框選的結果", 10.0);
            }
        } else {
            o.t.cfg.insert(format!("{kind}_region"), serde_json::json!([x, y, w, h]));
            o.t.raw.insert(kind.clone(), String::new());
            if kind == "exp" {
                o.t.pend_dec = None;
                o.t.reject_n = 0;
                o.t.exp_last = None;
                o.t.need = None;
                o.t.pend_first = None;
                o.t.pre_inc = None;
                o.t.rej_run = None;
            }
            if kind == "hp" || kind == "mp" {
                let i = if kind == "hp" { 0 } else { 1 };
                o.t.forget_qs(i);
                o.book.clear_qs(i);
                o.book.save();
                o.watch[i].reset(None);
                o.watch[i].check = None;
                o.watch[i].prompt = None;
            }
            o.save_cfg();
            o.t.say(format!("已框選：{name}"), 5.0);
        }
    }
    if s.settings_open && o.settings.is_none() {
        o.open_settings();
    }
    o.shared.wake();
}

pub fn select_window(o: &mut Overlay, ctx: &egui::Context) {
    let Some(kind) = o.select.as_ref().map(|s| s.kind.clone()) else { return };
    let (name, hint) = names(o, &kind);
    let sel = o.select.as_mut().unwrap();
    let Some((img, tex)) = sel.shot.as_mut() else { return };
    let tex = tex.get_or_insert_with(|| {
        let rgb: Vec<u8> = img.data.chunks_exact(3).flat_map(|p| [p[2], p[1], p[0]].map(|v| (v as f32 * 0.6) as u8)).collect();
        ctx.load_texture("screen", egui::ColorImage::from_rgb([img.w, img.h], &rgb), egui::TextureOptions::NEAREST)
    });
    let (vx, vy, vw, vh) = sel.virt;
    let ppp = ctx.pixels_per_point();
    let to_px = |p: egui::Pos2| (vx + (p.x * ppp).round() as i32, vy + (p.y * ppp).round() as i32);
    let mut done = None;
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
        let rect = ui.max_rect();
        let painter = ui.painter();
        painter.image(
            tex.id(),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(vw as f32 / ppp, vh as f32 / ppp)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        let (px, py, pw, ph) = platform::primary_screen();
        let pr = egui::Rect::from_min_size(egui::pos2((px - vx) as f32 / ppp, (py - vy) as f32 / ppp), egui::vec2(pw as f32 / ppp, ph as f32 / ppp));
        painter.rect_stroke(pr.shrink(3.0), 0.0, egui::Stroke::new(6.0, WARN), egui::StrokeKind::Inside);
        let size = (ph as f32 / ppp / 90.0).max(14.0) * 4.0 / 3.0;
        let t1 = painter.layout_no_wrap(format!("框選模式：用滑鼠拖曳，框住{name}"), bold(size * 0.75), Color32::WHITE);
        let t2 = painter.layout_no_wrap(format!("{hint}。按右鍵或 Esc 取消"), font(size * 0.75 * 0.7), Color32::from_rgb(0xff, 0xd8, 0xa8));
        let cx = pr.center().x;
        let y1 = pr.min.y + pr.height() / 12.0;
        let w = t1.size().x.max(t2.size().x) + size * 2.0;
        let h = t1.size().y + t2.size().y + size * 1.2;
        let bg = egui::Rect::from_center_size(egui::pos2(cx, y1 + h / 2.0 - size * 0.6), egui::vec2(w, h));
        painter.rect_filled(bg, 0.0, BG);
        painter.rect_stroke(bg, 0.0, egui::Stroke::new(3.0, WARN), egui::StrokeKind::Inside);
        let (s1, s2) = (t1.size(), t2.size());
        painter.galley(egui::pos2(cx - s1.x / 2.0, bg.min.y + size * 0.5), t1, Color32::WHITE);
        painter.galley(egui::pos2(cx - s2.x / 2.0, bg.min.y + size * 0.5 + s1.y + size * 0.2), t2, Color32::WHITE);
        let resp = ui.interact(rect, egui::Id::new("sel"), egui::Sense::click_and_drag());
        ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        if resp.drag_started() {
            sel.drag = ctx.input(|i| i.pointer.press_origin()).or(pointer);
        }
        if let (Some(a), Some(b)) = (sel.drag, pointer) {
            let r = egui::Rect::from_two_pos(a, b);
            painter.rect_stroke(r, 0.0, egui::Stroke::new(3.0, Color32::from_rgb(0xff, 0x3b, 0x30)), egui::StrokeKind::Middle);
            let (x0, y0) = to_px(r.min);
            let (x1, y1) = to_px(r.max);
            painter.text(
                r.min - egui::vec2(0.0, 8.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}×{}", x1 - x0, y1 - y0),
                mono((size / 2.0).max(10.0), true),
                Color32::from_rgb(0xff, 0x8a, 0x80),
            );
        }
        if resp.drag_stopped() {
            if let (Some(a), Some(b)) = (sel.drag.take(), pointer) {
                let r = egui::Rect::from_two_pos(a, b);
                let (x0, y0) = to_px(r.min);
                let (x1, y1) = to_px(r.max);
                if x1 - x0 >= 6 && y1 - y0 >= 6 {
                    done = Some(Some((x0, y0, x1 - x0, y1 - y0)));
                }
            }
        }
        if resp.secondary_clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            done = Some(None);
        }
    });
    if done.is_some() {
        sel.done = done;
    }
}

fn image_rgb(img: &Bgr) -> ::image::RgbImage {
    let mut rgb = ::image::RgbImage::new(img.w as u32, img.h as u32);
    for (d, s) in rgb.pixels_mut().zip(img.data.chunks_exact(3)) {
        *d = ::image::Rgb([s[2], s[1], s[0]]);
    }
    rgb
}
