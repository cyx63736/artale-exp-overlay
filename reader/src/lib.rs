pub mod auto;
pub mod cv;
pub mod font;

use cv::*;
use font::DigitFont;
use ocr::img::{prep, Bgr};
use ocr::{Item, Ocr};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use tracker::util::{bag_count, fix_digits, parse_count, parse_exp, parse_meso};
use tracker::OcrOut;

pub const KS: [&str; 2] = ["hp", "mp"];

pub trait Screen {
    fn grab(&mut self, x: i32, y: i32, w: i32, h: i32) -> Option<Bgr>;
}

pub struct Tpl {
    pub cw: usize,
    pub ch: usize,
    pub top_w: usize,
    pub top_h: usize,
    pub bright: f64,
    pub sig: f64,
    pub blur: Bgr,
    pub hue: Vec<i16>,
    pub colored: Vec<bool>,
    pub a: Vec<f32>,
    pub b: Vec<f32>,
}

impl Tpl {
    pub fn from_cell(cell: &Bgr) -> Tpl {
        let (cw, ch) = (cell.w, cell.h);
        let top_h = 4.max((ch as f64 * 0.55) as usize).min(ch);
        let top = crop_bgr(cell, 0, 0, cw, top_h);
        let sig = (1.0f64).max(cw as f64 / 55.0);
        let blur = gaussian(&top, sig);
        let hsv = hsv(&blur);
        let lab = lab(&blur);
        Tpl {
            cw,
            ch,
            top_w: cw,
            top_h,
            bright: top.data.iter().map(|&v| v as f64).sum::<f64>() / top.data.len() as f64,
            sig,
            hue: hsv.iter().map(|p| p[0] as i16).collect(),
            colored: hsv.iter().map(|p| p[1] > 90 && p[2] > 60).collect(),
            a: lab.iter().map(|p| p[1] - 128.0).collect(),
            b: lab.iter().map(|p| p[2] - 128.0).collect(),
            blur,
        }
    }

    pub fn same_item(&self, patch: &Bgr) -> bool {
        let h = hsv(patch);
        let n_col = self.colored.iter().filter(|&&c| c).count();
        if n_col >= 30 {
            let mut s = 0f64;
            for (i, p) in h.iter().enumerate() {
                if self.colored[i] {
                    let dh = (p[0] as i16 - self.hue[i]).abs();
                    s += dh.min(180 - dh) as f64;
                }
            }
            return s / n_col as f64 <= 5.0;
        }
        let l = lab(patch);
        let (mut s, mut n) = (0f64, 0usize);
        for (i, p) in l.iter().enumerate() {
            let (a, b) = (p[1] - 128.0, p[2] - 128.0);
            let (ta, tb) = (self.a[i], self.b[i]);
            if ta.hypot(tb) > 12.0 || a.hypot(b) > 12.0 {
                s += (a - ta).hypot(b - tb) as f64;
                n += 1;
            }
        }
        n == 0 || s / n as f64 <= 10.0
    }
}

pub fn meso_box(img: &Bgr) -> bool {
    let n = img.w * img.h;
    if n == 0 {
        return false;
    }
    let (mut white, mut dark) = (0usize, 0usize);
    for p in img.data.chunks_exact(3) {
        let mx = *p.iter().max().unwrap() as u32;
        let mn = *p.iter().min().unwrap() as u32;
        if mx > 200 && (mx - mn) * 255 < 40 * mx {
            white += 1;
        } else if mx < 90 {
            dark += 1;
        }
    }
    white * 100 >= n * 55 && dark * 100 >= n * 4 && dark * 100 <= n * 35
}

pub fn qs_sig(img: &Bgr) -> Vec<f32> {
    let mut v = vec![0f32; 12];
    let px = hsv(img);
    for p in &px {
        if p[1] > 90 && p[2] > 60 {
            v[(p[0] as usize / 15).min(11)] += 1.0;
        }
    }
    let n = px.len().max(1) as f32;
    v.iter_mut().for_each(|x| *x /= n);
    v
}

fn join_row(res: &[Item], strict: bool, digits: bool) -> Option<(String, f64)> {
    let mut boxes: Vec<BoxRow> = vec![];
    for it in res {
        let Some(q) = it.quad else { continue };
        let t = if strict { fix_digits(&it.text) } else { it.text.clone() };
        if digits && !DIGITS_ONLY.is_match(&t) {
            continue;
        }
        if strict || HAS_DIGIT.is_match(&t) {
            let ys = q.iter().map(|p| p[1] as f64);
            let (y0, y1) = (ys.clone().fold(f64::MAX, f64::min), ys.fold(f64::MIN, f64::max));
            let x0 = q.iter().map(|p| p[0] as f64).fold(f64::MAX, f64::min);
            boxes.push(((y0 + y1) / 2.0, x0, y0, y1, t, it.score as f64));
        }
    }
    if strict {
        boxes.retain(|b| !b.4.trim().is_empty());
    }
    let best = boxes
        .iter()
        .max_by(|a, b| {
            (a.0, a.1, a.2, a.3)
                .partial_cmp(&(b.0, b.1, b.2, b.3))
                .unwrap()
                .then_with(|| a.4.cmp(&b.4))
                .then_with(|| a.5.partial_cmp(&b.5).unwrap())
        })?
        .clone();
    let (cy, y0, y1) = (best.0, best.2, best.3);
    let tol = (6f64).max((y1 - y0) * 0.5);
    let mut row: Vec<&BoxRow> = boxes.iter().filter(|b| (b.0 - cy).abs() <= tol).collect();
    row.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let maxc = row.iter().map(|b| b.5).fold(f64::MIN, f64::max);
    Some((row.iter().map(|b| b.4.as_str()).collect(), maxc))
}

pub fn quick_count(ocr: &mut Ocr, img: &Bgr) -> Option<i64> {
    let res = ocr.run(&prep(img), true, true).ok()?;
    let t = match join_row(&res, false, false) {
        Some((t, _)) => t,
        None => flatten(&ocr.run(&prep(img), false, false).ok()?),
    };
    parse_count(&t)
}

pub fn flatten(res: &[Item]) -> String {
    let mut boxes: Vec<&Item> = res.iter().filter(|it| it.quad.is_some()).collect();
    if !boxes.is_empty() {
        let minx = |it: &Item| it.quad.unwrap().iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        boxes.sort_by(|a, b| minx(a).partial_cmp(&minx(b)).unwrap());
        return boxes.iter().map(|it| it.text.as_str()).collect::<Vec<_>>().join("|");
    }
    res.iter().map(|it| it.text.as_str()).collect::<Vec<_>>().join(" ")
}

static DIGITS_ONLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\d,.:;'’`\s]+$").unwrap());
static HAS_DIGIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d").unwrap());

pub fn font_name(f: &str) -> &'static str {
    match f {
        "bag" => "背包",
        "qs" => "快捷欄",
        _ => "楓幣",
    }
}

fn pil_gray(img: &Bgr) -> Vec<i16> {
    img.data.chunks_exact(3).map(|p| ((p[2] as u32 * 19595 + p[1] as u32 * 38470 + p[0] as u32 * 7471 + 0x8000) >> 16) as i16).collect()
}

fn region(cfg: &Map<String, Value>, key: &str) -> Option<[i32; 4]> {
    let a = cfg.get(key)?.as_array()?;
    let v: Vec<i32> = a.iter().filter_map(|x| x.as_f64().map(|f| f as i32)).collect();
    if v.len() == 4 { Some([v[0], v[1], v[2], v[3]]) } else { None }
}

type BoxRow = (f64, f64, f64, f64, String, f64);

pub struct Reader {
    pub engine: Ocr,
    pub fonts: HashMap<String, DigitFont>,
    pub fonts_path: Option<PathBuf>,
    pub tpl_paths: [Option<PathBuf>; 2],
    tpl_cache: [Option<(std::time::SystemTime, Tpl)>; 2],
    seen: HashMap<String, (usize, usize, Vec<i16>, Option<String>)>,
    pub bag_id: i64,
    was_open: bool,
    pub font_stat: (u64, u64, f64),
    pub logs: Vec<String>,
    pub fake_now: Option<f64>,
}

impl Reader {
    pub fn new(engine: Ocr) -> Self {
        Reader {
            engine,
            fonts: HashMap::new(),
            fonts_path: None,
            tpl_paths: [None, None],
            tpl_cache: [None, None],
            seen: HashMap::new(),
            bag_id: 0,
            was_open: false,
            font_stat: (0, 0, tracker::sys_now()),
            logs: vec![],
            fake_now: None,
        }
    }

    fn now(&self) -> f64 {
        self.fake_now.unwrap_or_else(tracker::sys_now)
    }

    pub fn load_fonts(&mut self) {
        self.fonts.clear();
        if let Some(p) = &self.fonts_path {
            if let Ok(s) = std::fs::read_to_string(p) {
                if let Ok(v) = serde_json::from_str::<Value>(&s) {
                    self.fonts = font::fonts_from_json(&v).into_iter().collect();
                }
            }
        }
    }

    pub fn save_fonts(&self) {
        let Some(p) = &self.fonts_path else { return };
        let mut keys: Vec<&String> = self.fonts.keys().collect();
        keys.sort();
        let v = font::fonts_to_json(&keys.iter().map(|k| ((*k).clone(), &self.fonts[*k])).collect::<Vec<_>>());
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, v.to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, p);
        }
    }

    fn ocr(&mut self, img: &Bgr, det: bool, cls: bool) -> Vec<Item> {
        self.engine.run(img, det, cls).unwrap_or_default()
    }

    pub fn rec_only(&mut self, img: &Bgr) -> String {
        let p = prep(img);
        let r = self.ocr(&p, false, false);
        flatten(&r)
    }

    fn unchanged(&mut self, key: &str, img: &Bgr) -> Option<String> {
        let g = pil_gray(img);
        if let Some((w, h, old, Some(text))) = self.seen.get(key) {
            if *w == img.w && *h == img.h && old.iter().zip(&g).filter(|(a, b)| (**a - **b).abs() > 40).count() <= 2 {
                return Some(text.clone());
            }
        }
        self.seen.insert(key.to_string(), (img.w, img.h, g, None));
        None
    }

    fn remember(&mut self, key: &str, text: &str) {
        if let Some(e) = self.seen.get_mut(key) {
            e.3 = Some(text.to_string());
        }
    }

    pub fn forget(&mut self, key: &str) {
        self.seen.remove(key);
    }

    pub fn clear_seen(&mut self) {
        self.seen.clear();
    }

    fn font_log(&mut self) {
        let (a, b, t) = self.font_stat;
        let now = self.now();
        if now - t >= 600.0 && a + b > 0 {
            self.logs.push(format!("數字讀取：比對 {a} 次、OCR {b} 次（{}% 用比對）", a * 100 / (a + b)));
            self.font_stat = (0, 0, now);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read_count(&mut self, img: &Bgr, key: Option<&str>, strict: bool, fallback: Option<&Bgr>, digits: bool, font: Option<&str>, left: f64) -> String {
        if let Some(k) = key {
            if let Some(c) = self.unchanged(k, img) {
                return c;
            }
        }
        let mut check: Option<String> = None;
        let mut bgr = img.clone();
        if let Some(fname) = font {
            if fname == "meso" {
                for v in bgr.data.iter_mut() {
                    *v = 255 - *v;
                }
            }
            let t = self.fonts.get(fname).filter(|f| !f.glyphs.is_empty()).and_then(|f| f.read(&bgr, left));
            if let Some(t) = t.filter(|t| !t.is_empty() && (!strict || !bag_count(t).is_empty())) {
                let f = self.fonts.get_mut(fname).unwrap();
                f.reads += 1;
                let complete = "0123456789".chars().all(|c| f.known().contains(c));
                if complete && f.reads % 20 != 0 {
                    self.font_stat.0 += 1;
                    self.font_log();
                    if let Some(k) = key {
                        self.remember(k, &t);
                    }
                    return t;
                }
                check = Some(t);
            }
            self.font_stat.1 += 1;
        }
        let mut text = self.ocr_count(img, strict, fallback, digits);
        if let Some(fname) = font {
            let val = if fname == "meso" {
                parse_meso(&text).map_or(String::new(), |v| v.to_string())
            } else if strict {
                bag_count(&text)
            } else {
                parse_count(&text).map_or(String::new(), |v| v.to_string())
            };
            let punct = fname == "meso";
            self.fonts.entry(fname.to_string()).or_insert_with(|| DigitFont::new(punct));
            let known = self.fonts[fname].known();
            let fresh_digit = val.chars().any(|c| c.is_ascii_digit() && !known.contains(c));
            if let Some(chk) = check.filter(|_| !fresh_digit) {
                if !val.is_empty() && val != chk {
                    let f = self.fonts.get_mut(fname).unwrap();
                    f.bad += 1;
                    let bad = f.bad;
                    self.logs.push(format!("{}數字：比對讀到 {chk}、OCR 讀到 {val}，對不上，這次不採用", font_name(fname)));
                    if bad >= 3 {
                        self.fonts.insert(fname.to_string(), DigitFont::new(punct));
                        self.save_fonts();
                        self.logs.push(format!("{}數字：比對一直跟 OCR 對不上，重新學", font_name(fname)));
                    }
                    text = String::new();
                } else if val.is_empty() {
                    text = chk;
                }
            } else if !val.is_empty() && val.len() <= if punct { 12 } else { 6 } {
                let f = self.fonts.get_mut(fname).unwrap();
                let before = f.known();
                let new = f.learn(&bgr, &val, left);
                let (resized, known) = (f.resized, f.known());
                if resized {
                    self.logs.push(format!("{}數字：字的大小變了（遊戲視窗調過大小？），照新的大小重新學", font_name(fname)));
                }
                if !new.is_empty() || resized {
                    self.save_fonts();
                }
                let fresh: String = new.chars().filter(|c| !before.contains(*c)).collect();
                if !fresh.is_empty() {
                    let missing: String = "0123456789".chars().filter(|c| !known.contains(*c)).collect();
                    let tail = if missing.is_empty() { "（0~9 都學會了）".to_string() } else { format!("（還沒學到 {missing}）") };
                    self.logs.push(format!("{}數字：從 {val} 學到 {fresh}{tail}", font_name(fname)));
                }
            }
        }
        if let Some(k) = key {
            if !(strict || digits) || HAS_DIGIT.is_match(&text) {
                self.remember(k, &text);
            }
        }
        text
    }

    fn ocr_count(&mut self, img: &Bgr, strict: bool, fallback: Option<&Bgr>, digits: bool) -> String {
        let arr = prep(img);
        let res = self.ocr(&arr, true, true);
        if let Some((mut text, maxc)) = join_row(&res, strict, digits) {
            if strict {
                let dotted = text.chars().any(|c| c.is_ascii_digit()) && text.chars().all(|c| c.is_ascii_digit() || c == '.');
                text = bag_count(&text);
                if text.is_empty() && (maxc < 0.8 || dotted) {
                    if let Some(fb) = fallback {
                        let r = self.rec_only(fb);
                        text = bag_count(&fix_digits(&r));
                    }
                }
            }
            return text;
        }
        if strict {
            return match fallback {
                Some(fb) => {
                    let r = self.rec_only(fb);
                    bag_count(&fix_digits(&r))
                }
                None => String::new(),
            };
        }
        if digits {
            return String::new();
        }
        self.rec_only(img)
    }

    pub fn read(&mut self, img: &Bgr, wide: bool) -> String {
        let arr = prep(img);
        let mut modes = [(true, true), (false, false)];
        if !wide {
            modes.reverse();
        }
        let mut last = String::new();
        for (det, cls) in modes {
            let r = self.ocr(&arr, det, cls);
            let text = flatten(&r);
            if !text.is_empty() && parse_exp(&text).is_some() {
                return text;
            }
            if !text.is_empty() {
                last = text;
            }
        }
        last
    }

    fn load_tpls(&mut self) -> [bool; 2] {
        let mut ok = [false; 2];
        for k in 0..2 {
            let Some(p) = self.tpl_paths[k].clone() else {
                self.tpl_cache[k] = None;
                continue;
            };
            let Ok(meta) = std::fs::metadata(&p) else {
                self.tpl_cache[k] = None;
                continue;
            };
            let mt = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            if self.tpl_cache[k].as_ref().map_or(true, |c| c.0 != mt) {
                match Bgr::load(&p) {
                    Ok(cell) => self.tpl_cache[k] = Some((mt, Tpl::from_cell(&cell))),
                    Err(_) => {
                        self.tpl_cache[k] = None;
                        continue;
                    }
                }
            }
            ok[k] = true;
        }
        ok
    }

    pub fn read_inventory(&mut self, img: &Bgr) -> [Option<Vec<Option<i64>>>; 2] {
        let ok = self.load_tpls();
        let (hh, ww) = (img.h, img.w);
        let mut res: [Option<Vec<Option<i64>>>; 2] = [None, None];
        for k in 0..2 {
            if !ok[k] {
                continue;
            }
            let t = &self.tpl_cache[k].as_ref().unwrap().1;
            let (cw, ch, bright, th, tw, sig) = (t.cw, t.ch, t.bright, t.top_h, t.top_w, t.sig);
            if hh < th || ww < tw {
                res[k] = Some(vec![]);
                continue;
            }
            let img_blur = gaussian(img, sig);
            let mut m = match_color(&img_blur, &t.blur);
            let mut hits: Vec<(usize, usize)> = vec![];
            let mut dim = false;
            while hits.len() < 80 {
                let (mx, x, y) = max_loc(&m);
                if mx < 0.6 {
                    break;
                }
                let patch = crop_bgr(img, x, y, x + tw, y + th);
                let patch_bright = patch.data.iter().map(|&v| v as f64).sum::<f64>() / patch.data.len() as f64;
                if !t.same_item(&crop_bgr(&img_blur, x, y, x + tw, y + th)) {
                } else if (patch_bright - bright).abs() <= 20.0 {
                    if mx >= 0.8 {
                        hits.push((x, y));
                    }
                } else if mx >= 0.8 || patch_bright < bright - 20.0 {
                    dim = true;
                }
                for yy in y.saturating_sub(th / 2)..(y + th / 2 + 1).min(m.h) {
                    for xx in x.saturating_sub(tw / 2)..(x + tw / 2 + 1).min(m.w) {
                        m.set(xx, yy, -1.0);
                    }
                }
            }
            if dim {
                res[k] = Some(vec![None]);
                continue;
            }
            hits.sort_by_key(|&(x, y)| (y, x));
            let mut stacks = vec![];
            for (x, y) in hits {
                let (y0, y1) = (y + (ch as f64 * 0.45) as usize, hh.min(y + (ch as f64 * 1.05) as usize));
                let cx0 = x.saturating_sub((cw as f64 * 0.2) as usize);
                let cx1 = ww.min(x + (cw as f64 * 1.08) as usize);
                if y0 >= y1 || cx0 >= cx1 {
                    stacks.push(None);
                    continue;
                }
                let crop = crop_bgr(img, cx0, y0, cx1, y1);
                let (sy0, sy1) = (y + (ch as f64 * 0.5) as usize, hh.min(y + (ch as f64 * 0.95) as usize));
                let sx1 = ww.min(x + cw);
                let strip = if sy0 < sy1 && x < sx1 { Some(crop_bgr(img, x, sy0, sx1, sy1)) } else { None };
                let key = format!("inv_{}_{x}_{y}", KS[k]);
                let text = self.read_count(&crop, Some(&key), true, strip.as_ref(), false, Some("bag"), (x - cx0) as f64);
                stacks.push(parse_count(&text));
            }
            res[k] = Some(stacks);
        }
        res
    }

    pub fn tick(&mut self, cfg: &Map<String, Value>, screen: &mut dyn Screen) -> (Option<OcrOut>, bool) {
        let mut out = OcrOut::default();
        let mut any = false;
        if let Some([x, y, w, h]) = region(cfg, "exp_region") {
            any = true;
            match screen.grab(x, y, w, h) {
                Some(img) => {
                    let text = match self.unchanged("exp", &img) {
                        Some(t) => t,
                        None => {
                            let t = self.read(&img, true);
                            self.remember("exp", &t);
                            t
                        }
                    };
                    out.exp = Some(text);
                }
                None => {
                    out.exp = Some(String::new());
                    out.exp_err = Some("截圖失敗".into());
                }
            }
        }
        for k in 0..2 {
            if let Some([x, y, w, h]) = region(cfg, &format!("{}_region", KS[k])) {
                any = true;
                match screen.grab(x, y, w, h) {
                    Some(img) => {
                        out.qs_sig[k] = Some(qs_sig(&img));
                        out.qs[k] = Some(self.read_count(&img, Some(KS[k]), false, None, false, Some("qs"), 0.0));
                    }
                    None => {
                        out.qs[k] = Some(String::new());
                        out.qs_err[k] = Some("截圖失敗".into());
                    }
                }
            }
        }
        let mut inv_open = false;
        if let Some([x, y, w, h]) = region(cfg, "inv_region") {
            if self.tpl_paths.iter().any(|p| p.as_ref().map_or(false, |p| p.exists())) {
                if let Some(img) = screen.grab(x, y, w, h) {
                    let inv = self.read_inventory(&img);
                    inv_open = inv.iter().any(|v| v.as_ref().map_or(false, |v| !v.is_empty()));
                    out.inv = Some(inv);
                    any = true;
                }
            }
        }
        let meso_img = region(cfg, "meso_region").and_then(|[x, y, w, h]| screen.grab(x, y, w, h));
        let boxed = !inv_open && meso_img.as_ref().map_or(false, meso_box);
        let open = inv_open || boxed;
        if inv_open && !self.was_open {
            self.bag_id += 1;
        }
        self.was_open = inv_open;
        out.bag_id = self.bag_id;
        match meso_img {
            Some(img) if open => {
                out.meso = Some(self.read_count(&img, Some("meso"), false, None, true, Some("meso"), 0.0));
                out.meso_box_only = boxed;
                any = true;
            }
            _ => self.forget("meso"),
        }
        (if any { Some(out) } else { None }, inv_open)
    }
}
