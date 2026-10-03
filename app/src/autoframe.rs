use crate::platform;
use crate::potions::{same_slot, KIND};
use crate::runner::Kind;
use crate::select::{set_region, set_tpl};
use crate::ui::*;
use ocr::img::Bgr;
use reader::auto::{Found, Pair, Rect};
use reader::cv::crop_bgr;

pub struct AutoRun {
    start: f64,
    area: (i32, i32, i32, i32),
    sent: bool,
}

impl AutoRun {
    pub fn hiding(&self) -> bool {
        !self.sent
    }
}

pub struct AutoPick {
    pub pairs: Vec<Pair>,
    pub used: Vec<bool>,
    crops: Vec<Bgr>,
    pub todo: Vec<usize>,
}

pub fn begin(o: &mut Overlay) {
    if o.auto.is_some() || o.select.is_some() {
        return;
    }
    o.auto_pick = None;
    let area = o.hwnd_of(Kind::Overlay).map_or_else(platform::virtual_screen, platform::monitor_of);
    o.auto = Some(AutoRun { start: o.now(), area, sent: false });
}

pub fn step(o: &mut Overlay) {
    let now = o.now();
    let Some(a) = o.auto.as_mut() else { return };
    if !a.sent && now - a.start >= 0.3 {
        let (x, y, w, h) = a.area;
        #[cfg(windows)]
        let img = ocr::capture::Grabber::new().and_then(|g| g.grab(x, y, w, h));
        #[cfg(target_os = "macos")]
        let img = ocr::capture::grab_once(x, y, w, h);
        #[cfg(not(any(windows, target_os = "macos")))]
        let img: Option<Bgr> = {
            let _ = (x, y, w, h);
            None
        };
        a.sent = true;
        match img {
            Some(img) => {
                *o.shared.auto.lock().unwrap() = Some(img);
                o.shared.wake();
                o.t.say("自動框選中，請稍等…", 15.0);
            }
            None => {
                o.auto = None;
                o.t.say("出錯了：截不到畫面", 10.0);
            }
        }
    } else if a.sent && now - a.start > 30.0 {
        o.auto = None;
        o.t.say("自動框選沒有完成，請再按一次，或手動框選", 10.0);
    }
}

pub fn found(o: &mut Overlay, f: Found, img: Bgr) {
    let Some(a) = o.auto.take() else { return };
    let (ax, ay) = (a.area.0, a.area.1);
    let off = |r: Rect| [r.0 + ax, r.1 + ay, r.2, r.3];
    let (mut got, mut miss): (Vec<String>, Vec<&str>) = (vec![], vec![]);
    let mut mask = 0u8;
    for (key, r, bit, name) in [("exp", f.exp, 8u8, "EXP"), ("inv", f.inv, 1, "背包"), ("meso", f.meso, 16, "楓幣")] {
        match r {
            Some(r) => {
                set_region(o, key, off(r));
                got.push(name.into());
                mask |= bit;
            }
            None => miss.push(name),
        }
    }
    let mut free = vec![true; f.pairs.len()];
    let mut found: [Option<usize>; 2] = [None; 2];
    let names = [o.t.name(0), o.t.name(1)];
    let own: Vec<Option<Bgr>> = (0..2).map(|k| Bgr::load(&o.paths.tpl[k]).ok()).collect();
    for k in 0..2 {
        let Some(t) = &own[k] else { continue };
        let best = (0..f.pairs.len()).filter(|&i| free[i]).filter_map(|i| tpl_match(t, &img, f.pairs[i].bag.rect).map(|d| (d, i))).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, i)) = best {
            free[i] = false;
            found[k] = Some(i);
        }
    }
    let others: Vec<(String, Bgr)> = o.book.tpl_files().into_iter().filter_map(|(_, n, p)| Some((n, Bgr::load(&p).ok()?))).collect();
    for k in 0..2 {
        if found[k].is_some() {
            continue;
        }
        let Some(old) = region_of(&o.t.cfg, KIND[k]) else { continue };
        let Some(i) = (0..f.pairs.len()).find(|&i| free[i] && same_slot(old, off(f.pairs[i].qs.rect))) else { continue };
        let bag = f.pairs[i].bag.rect;
        let not_me = own[k].is_some() || others.iter().any(|(n, t)| *n != names[k] && tpl_match(t, &img, bag).is_some());
        if !not_me {
            free[i] = false;
            found[k] = Some(i);
        }
    }
    let mut todo = vec![];
    for k in 0..2 {
        match found[k] {
            Some(i) => {
                let crop = Some(cell(&img, f.pairs[i].bag.rect));
                apply(o, k, off(f.pairs[i].qs.rect), crop.as_ref());
                mask |= 1 << (k + 1);
                got.push(format!("{} 快捷欄", disp(&names[k])));
            }
            None => todo.push(k),
        }
    }
    let mut left: Vec<usize> = (0..f.pairs.len()).filter(|&i| free[i]).collect();
    left.sort_by(|&a, &b| {
        let (ra, rb) = (f.pairs[a].qs.rect, f.pairs[b].qs.rect);
        if (ra.1 - rb.1).abs() * 2 < ra.3.max(rb.3) { ra.0.cmp(&rb.0) } else { ra.1.cmp(&rb.1) }
    });
    if !todo.is_empty() && !left.is_empty() && o.names_ok() {
        o.auto_pick = Some(AutoPick {
            pairs: left
                .iter()
                .map(|&i| {
                    let mut p = f.pairs[i].clone();
                    let r = off(p.qs.rect);
                    p.qs.rect = (r[0], r[1], r[2], r[3]);
                    p
                })
                .collect(),
            used: vec![false; left.len()],
            crops: left.iter().map(|&i| cell(&img, f.pairs[i].bag.rect)).collect(),
            todo: todo.clone(),
        });
    } else if !todo.is_empty() {
        miss.push("快捷欄");
    }
    o.save_cfg();
    o.shared.wake();
    o.show_hint(mask);
    let mut msg = if got.is_empty() { "自動框選：什麼都沒找到".to_string() } else { format!("自動框選：找到 {}", got.join("、")) };
    if f.inv.is_some() && f.cells.is_empty() {
        msg += "\n背包裡看不到藥水的數量：請切到「消耗」分頁再按一次";
    } else {
        let scrolled = f.inv.is_some() && f.pairs.is_empty() && miss.contains(&"快捷欄");
        if scrolled {
            miss.retain(|m| *m != "快捷欄");
        }
        if !miss.is_empty() {
            msg += &format!("\n找不到 {}，請打開背包、切到消耗欄再試一次，或手動框選", miss.join("、"));
        }
        if scrolled {
            msg += "\n找不到快捷欄：背包裡看不到快捷欄上的藥水，請把背包捲到藥水那幾格再試一次";
        }
    }
    if o.auto_pick.is_some() {
        msg += "\n快捷欄的藥水請在上面選";
    }
    o.t.say(msg.clone(), 12.0);
    o.t.logs.push(msg.replace('\n', "；"));
}

pub fn pick(o: &mut Overlay, idx: usize) {
    let Some(p) = o.auto_pick.as_mut() else { return };
    let (Some(&k), Some(pair), Some(crop)) = (p.todo.first(), p.pairs.get(idx).cloned(), p.crops.get(idx).cloned()) else { return };
    p.used[idx] = true;
    p.todo.remove(0);
    if p.todo.is_empty() || p.used.iter().all(|&u| u) {
        o.auto_pick = None;
    }
    let r = pair.qs.rect;
    apply(o, k, [r.0, r.1, r.2, r.3], Some(&crop));
    o.save_cfg();
    o.shared.wake();
    o.show_hint(1 << (k + 1));
    o.t.say(format!("已框選：{} 快捷欄", disp(&o.t.name(k))), 5.0);
}

pub fn skip(o: &mut Overlay) {
    let Some(p) = o.auto_pick.as_mut() else { return };
    if !p.todo.is_empty() {
        p.todo.remove(0);
    }
    if p.todo.is_empty() {
        o.auto_pick = None;
    }
}

fn apply(o: &mut Overlay, k: usize, r: [i32; 4], crop: Option<&Bgr>) {
    if region_of(&o.t.cfg, KIND[k]) != Some(r) {
        set_region(o, KIND[k], r);
    } else {
        o.t.cfg.insert(format!("{}_screen", KIND[k]), crate::ui::region_tag(r).into());
    }
    if let Some(c) = crop {
        set_tpl(o, k, c);
    }
}

fn cell(img: &Bgr, r: Rect) -> Bgr {
    let (x0, y0) = (r.0.max(0) as usize, r.1.max(0) as usize);
    crop_bgr(img, x0, y0, ((r.0 + r.2) as usize).min(img.w), ((r.1 + r.3) as usize).min(img.h))
}

fn tpl_match(tpl: &Bgr, img: &Bgr, r: Rect) -> Option<f32> {
    let c = cell(img, r);
    if c.w < 8 || c.h < 8 || tpl.w < 8 || tpl.h < 8 {
        return None;
    }
    let a = reader::qs_sig(&crop_bgr(&c, 0, 0, c.w, c.h * 6 / 10));
    let b = reader::qs_sig(&crop_bgr(tpl, 0, 0, tpl.w, tpl.h * 6 / 10));
    let d = reader::auto::look(tpl, &c);
    (reader::auto::same_color(&a, &b) < 0.35 && d < 0.22).then_some(d)
}
