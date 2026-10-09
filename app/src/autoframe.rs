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
    verify: Option<Verify>,
}

struct Verify {
    k: usize,
    pair: Pair,
    crop: Option<Bgr>,
}

impl AutoRun {
    pub fn hiding(&self) -> bool {
        !self.sent
    }
}

pub struct AutoPick {
    pub pairs: Vec<Pair>,
    pub used: Vec<bool>,
    crops: Vec<Option<Bgr>>,
    pub todo: Vec<usize>,
    area: (i32, i32, i32, i32),
}

pub fn begin(o: &mut Overlay) {
    if o.auto.is_some() || o.select.is_some() {
        return;
    }
    o.auto_pick = None;
    let area = o.hwnd_of(Kind::Overlay).map_or_else(platform::virtual_screen, platform::monitor_of);
    o.auto = Some(AutoRun { start: o.now(), area, sent: false, verify: None });
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
            None => match o.auto.take().and_then(|a| a.verify) {
                Some(v) => finish_pick(o, v.k, v.pair.qs.rect, v.crop.as_ref()),
                None => o.t.say("出錯了：截不到畫面", 10.0),
            },
        }
    } else if a.sent && now - a.start > 30.0 {
        let a = o.auto.take().unwrap();
        match a.verify {
            Some(v) => finish_pick(o, v.k, v.pair.qs.rect, v.crop.as_ref()),
            None => o.t.say("自動框選沒有完成，請再按一次，或手動框選", 10.0),
        }
    }
}

pub fn found(o: &mut Overlay, f: Found, img: Bgr) {
    let Some(a) = o.auto.take() else { return };
    let (ax, ay) = (a.area.0, a.area.1);
    let off = |r: Rect| [r.0 + ax, r.1 + ay, r.2, r.3];
    if let Some(v) = a.verify {
        let now = f.pairs.iter().find(|p| same_slot(off(p.qs.rect), [v.pair.qs.rect.0, v.pair.qs.rect.1, v.pair.qs.rect.2, v.pair.qs.rect.3]));
        match now {
            Some(p) => {
                let r = off(p.qs.rect);
                let crop = bag_crop(&f, p, &img).or(v.crop);
                finish_pick(o, v.k, (r[0], r[1], r[2], r[3]), crop.as_ref());
            }
            None => finish_pick(o, v.k, v.pair.qs.rect, v.crop.as_ref()),
        }
        return;
    }
    let (mut got, mut miss): (Vec<String>, Vec<&str>) = (vec![], vec![]);
    let mut mask = 0u32;
    for (key, r, bit, name) in [("exp", f.exp, 8u32, "EXP"), ("inv", f.inv, 1, "背包"), ("meso", f.meso, 16, "楓幣")] {
        match r {
            Some(r) => {
                set_region(o, key, off(r));
                got.push(name.into());
                mask |= bit;
            }
            None => miss.push(name),
        }
    }
    let mut free: Vec<bool> = f.pairs.iter().map(|p| p.sure).collect();
    let ks = o.t.ks();
    let mut found: [Option<usize>; tracker::NP] = [None; tracker::NP];
    let names: [String; tracker::NP] = std::array::from_fn(|k| o.t.name(k));
    let own: Vec<Option<Bgr>> = (0..tracker::NP).map(|k| ks.contains(&k).then(|| Bgr::load(&o.paths.tpl[k]).ok().filter(cell_like)).flatten()).collect();
    for &k in &ks {
        let Some(t) = &own[k] else { continue };
        let best = (0..f.pairs.len()).filter(|&i| free[i]).filter_map(|i| tpl_match(t, &img, f.pairs[i].bag.rect).map(|d| (d, i))).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, i)) = best {
            free[i] = false;
            found[k] = Some(i);
        }
    }
    let others: Vec<(String, Bgr)> = o.book.tpl_files().into_iter().filter_map(|(_, n, p)| Some((n, Bgr::load(&p).ok().filter(cell_like)?))).collect();
    for &k in &ks {
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
    for &k in &ks {
        match found[k] {
            Some(i) => {
                let crop = Some(cell(&img, f.pairs[i].bag.rect));
                apply(o, k, off(f.pairs[i].qs.rect), crop.as_ref());
                mask |= pot_bit(k);
                got.push(format!("{} 快捷欄", disp(&names[k])));
            }
            None => todo.push(k),
        }
    }
    let mut left: Vec<usize> = (0..f.pairs.len()).filter(|&i| free[i] || !f.pairs[i].sure).collect();
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
                    let (r, c) = (off(p.qs.rect), off(p.cell));
                    p.qs.rect = (r[0], r[1], r[2], r[3]);
                    p.cell = (c[0], c[1], c[2], c[3]);
                    p
                })
                .collect(),
            used: vec![false; left.len()],
            crops: left.iter().map(|&i| bag_crop(&f, &f.pairs[i], &img)).collect(),
            todo: todo.clone(),
            area: a.area,
        });
    }
    let lost = |k: usize| o.auto_pick.is_none() && todo.contains(&k);
    if lost(0) || lost(1) {
        miss.push("快捷欄");
    }
    let extra_lost: Vec<String> = ks.iter().filter(|&&k| k >= 2 && lost(k)).map(|&k| disp(&names[k])).collect();
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
        if !extra_lost.is_empty() && !scrolled {
            msg += &format!("\n找不到 {} 的快捷欄：有放在快捷欄的話請手動框選；沒放的話打開背包時會算數量", extra_lost.join("、"));
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
    let area = p.area;
    p.used[idx] = true;
    p.todo.remove(0);
    if p.todo.is_empty() || p.used.iter().all(|&u| u) {
        o.auto_pick = None;
    }
    if o.auto.is_none() && o.select.is_none() {
        o.auto = Some(AutoRun { start: o.now(), area, sent: false, verify: Some(Verify { k, pair, crop }) });
    } else {
        finish_pick(o, k, pair.qs.rect, crop.as_ref());
    }
}

fn bag_crop(f: &Found, p: &Pair, img: &Bgr) -> Option<Bgr> {
    (p.sure || f.cells.iter().filter(|c| c.count == p.bag.count).count() == 1).then(|| cell(img, p.bag.rect))
}

fn finish_pick(o: &mut Overlay, k: usize, r: Rect, crop: Option<&Bgr>) {
    apply(o, k, [r.0, r.1, r.2, r.3], crop);
    o.save_cfg();
    o.shared.wake();
    o.show_hint(pot_bit(k));
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
    if let Some(c) = crop.filter(|c| cell_like(c)) {
        set_tpl(o, k, c);
    }
}

fn cell_like(c: &Bgr) -> bool {
    c.w >= 8 && c.h >= 8 && c.w * 2 >= c.h && c.h * 2 >= c.w
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
    let color = reader::auto::same_color(&a, &b);
    let d = reader::auto::look(tpl, &c);
    if color < 0.35 && d < 0.22 {
        return Some(d);
    }
    let (ra, rb) = (tpl.w as f32 / tpl.h as f32, c.w as f32 / c.h as f32);
    let stretched = (ra - rb).abs() > 0.1 * ra.max(rb);
    let colorful = a.iter().sum::<f32>() >= 0.03 && b.iter().sum::<f32>() >= 0.03;
    (stretched && colorful && color < 0.12).then_some(1.0 + color)
}
