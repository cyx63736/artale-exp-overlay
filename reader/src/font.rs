use crate::cv::*;
use ocr::img::Bgr;

pub fn digit_box(bgr: &Bgr, left: f64, gap: f64) -> Option<(usize, usize, usize, usize)> {
    let m = digit_fill(bgr);
    let (lab, st) = components(&m, true);
    let (w_, h_) = (m.w, m.h);
    let gray: Vec<i16> = bgr.data.chunks_exact(3).map(|p| p[0].max(p[1]).max(p[2]) as i16).collect();
    let mut comps: Vec<(usize, usize, usize, usize)> = vec![];
    for (i, c) in st.iter().enumerate() {
        let (x, y, w, h, area) = (c.x, c.y, c.w, c.h, c.area);
        if h < 3 || area < 4 || x == 0 || y == 0 || x + w >= w_ || y + h >= h_ || (x as f64 + w as f64 * 0.5) < left {
            continue;
        }
        let pad = 2.max(h / 8);
        let (x0, y0, x1, y1) = (x.saturating_sub(pad), y.saturating_sub(pad), (x + w + pad).min(w_), (y + h + pad).min(h_));
        let mut big = Plane::<u8>::new(x1 - x0, y1 - y0);
        for yy in y..y + h {
            for xx in x..x + w {
                if lab.at(xx, yy) == i as u32 + 1 {
                    big.set(xx - x0, yy - y0, 1);
                }
            }
        }
        let ring = dilate(&big, pad);
        let mut v: Vec<i16> = vec![];
        for yy in 0..big.h {
            for xx in 0..big.w {
                if ring.at(xx, yy) != 0 && big.at(xx, yy) == 0 {
                    v.push(gray[(yy + y0) * w_ + xx + x0]);
                }
            }
        }
        v.sort();
        if !v.is_empty() {
            let n = 1.max(v.len() / 2);
            let mean = v[..n].iter().map(|&a| a as f64).sum::<f64>() / n as f64;
            if mean <= 90.0 {
                comps.push((x, y, w, h));
            }
        }
    }
    let whole: Vec<_> = comps.iter().filter(|c| c.2 as f64 <= c.3 as f64 * 1.2).copied().collect();
    if whole.is_empty() {
        return None;
    }
    let hd = whole.iter().map(|c| c.3).max().unwrap();
    if hd < 8 {
        return None;
    }
    let hd = hd as f64;
    let mut anchor: Option<(usize, usize, usize, usize)> = None;
    for c in whole.iter().filter(|c| c.3 as f64 >= 0.7 * hd) {
        if anchor.map_or(true, |a| c.1 + c.3 > a.1 + a.3) {
            anchor = Some(*c);
        }
    }
    let a = anchor.unwrap();
    let (top, bot) = (a.1 as f64 - 0.2 * hd, (a.1 + a.3) as f64 + 0.2 * hd);
    let row: Vec<_> = comps.iter().filter(|c| c.1 as f64 >= top && (c.1 + c.3) as f64 <= bot).copied().collect();
    let (mut x0, mut x1) = (a.0 as f64, (a.0 + a.2) as f64);
    let mut grown = true;
    while grown {
        grown = false;
        for c in &row {
            let (cx0, cx1) = (c.0 as f64, (c.0 + c.2) as f64);
            if cx0 <= x1 + gap * hd && cx1 >= x0 - gap * hd && (cx0 < x0 || cx1 > x1) {
                x0 = x0.min(cx0);
                x1 = x1.max(cx1);
                grown = true;
            }
        }
    }
    let part: Vec<_> = row.iter().filter(|c| c.0 as f64 >= x0 && (c.0 + c.2) as f64 <= x1).collect();
    let y0 = part.iter().map(|c| c.1).min().unwrap();
    let y1 = part.iter().map(|c| c.1 + c.3).max().unwrap();
    Some((x0 as usize, y0, (x1 - x0) as usize, y1 - y0))
}

pub fn split_digits(fill: &Plane<u8>, bx: (usize, usize, usize, usize), text: &str) -> Option<Vec<(usize, usize)>> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let (x, y, w, h) = bx;
    let prof: Vec<f64> = (0..w).map(|c| (y..y + h).map(|r| fill.at(x + c, r) as f64).sum()).collect();
    let (lo, hi) = (2.max((h as f64 * 0.2) as usize), (h as f64 * 1.1) as usize);
    let inf = f64::INFINITY;
    let mut best = vec![vec![inf; w + 1]; n + 1];
    let mut back = vec![vec![-1i64; w + 1]; n + 1];
    best[0][0] = 0.0;
    for k in 1..=n {
        for c in 1..=w {
            for l in lo..=hi.min(c) {
                let prev = best[k - 1][c - l];
                if prev == inf {
                    continue;
                }
                let dev = (l as f64 - h as f64 * if chars[k - 1] == '1' { 0.35 } else { 0.62 }) / (0.25 * h as f64);
                let cost = prev + if c < w { prof[c - 1] } else { 0.0 } + 3.0 * dev * dev;
                if cost < best[k][c] {
                    best[k][c] = cost;
                    back[k][c] = (c - l) as i64;
                }
            }
        }
    }
    if best[n][w] == inf {
        return None;
    }
    let mut cuts = vec![];
    let mut c = w;
    for k in (1..=n).rev() {
        let b = back[k][c] as usize;
        cuts.push((x + b, x + c));
        c = b;
    }
    cuts.reverse();
    Some(cuts)
}

#[derive(Clone, Debug)]
pub struct Glyph {
    pub ch: char,
    pub t: Plane<f32>,
    pub m: Plane<f32>,
    pub pl: usize,
    pub pr: usize,
}

#[derive(Clone, Debug)]
struct Cand {
    mx: f32,
    px: usize,
    py: usize,
    tw: usize,
    th: usize,
    ch: char,
    pl: usize,
    pr: usize,
    gi: usize,
}

pub struct DigitFont {
    pub glyphs: Vec<Glyph>,
    pub punct: bool,
    pub bad: u32,
    pub reads: u64,
    pub resized: bool,
}

pub const MAX_PER: usize = 4;
pub const MIN_H: usize = 16;

fn pround(v: f64) -> i64 {
    v.round_ties_even() as i64
}

impl DigitFont {
    pub fn new(punct: bool) -> Self {
        DigitFont { glyphs: vec![], punct, bad: 0, reads: 0, resized: false }
    }

    pub fn known(&self) -> String {
        let mut v: Vec<char> = self.glyphs.iter().map(|g| g.ch).collect();
        v.sort();
        v.dedup();
        v.into_iter().collect()
    }

    pub fn learn(&mut self, bgr: &Bgr, text: &str, left: f64) -> String {
        let bx = digit_box(bgr, left, if self.punct { 0.9 } else { 0.45 });
        let n = text.chars().count() as f64;
        let Some(bx) = bx else { return String::new() };
        let ratio = bx.2 as f64 / bx.3 as f64;
        if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) || !(0.25 * n <= ratio && ratio <= 0.95 * n) {
            return String::new();
        }
        if bx.3 < MIN_H {
            return String::new();
        }
        let (x, y, w, h) = bx;
        let (ww, hh) = (bgr.w, bgr.h);
        let g = gray_f32(bgr);
        let (x0, y0, x1, y1) = (x.saturating_sub(3), y.saturating_sub(3), (x + w + 3).min(ww), (y + h + 3).min(hh));
        let sub = crop_bgr(bgr, x0, y0, x1, y1);
        let (lab, st) = components(&digit_fill(&sub), false);
        let keep: Vec<bool> = st
            .iter()
            .map(|c| c.x > 0 && c.y > 0 && c.x + c.w < x1 - x0 && c.y + c.h < y1 - y0 && !(self.punct && (c.h as f64) < 0.6 * h as f64))
            .collect();
        let mut fill = Plane::<u8>::new(ww, hh);
        for yy in 0..lab.h {
            for xx in 0..lab.w {
                let l = lab.at(xx, yy);
                if l > 0 && keep[l as usize - 1] {
                    fill.set(xx + x0, yy + y0, 1);
                }
            }
        }
        let Some(segs) = split_digits(&fill, bx, text) else { return String::new() };
        let pad = 2.max(pround(h as f64 * 0.1)) as usize;
        let d = 1.max(pround(h as f64 * 0.05)) as usize;
        let mut new: Vec<Glyph> = vec![];
        for (&(sx0, sx1), ch) in segs.iter().zip(text.chars()) {
            let (a0, a1, b0, b1) = (sx0.saturating_sub(pad), (sx1 + pad).min(ww), y.saturating_sub(pad), (y + h + pad).min(hh));
            let mut own = Plane::<u8>::new(a1 - a0, b1 - b0);
            for yy in b0..b1 {
                for xx in sx0..sx1 {
                    own.set(xx - a0, yy - b0, fill.at(xx, yy));
                }
            }
            let md = dilate(&own, d);
            let m = Plane { w: md.w, h: md.h, d: md.d.iter().map(|&v| v as f32).collect::<Vec<f32>>() };
            if (m.d.iter().sum::<f32>() as f64) < h as f64 {
                return String::new();
            }
            new.push(Glyph { ch, t: g.crop(a0, b0, a1, b1), m, pl: sx0 - a0, pr: a1 - sx1 });
        }
        let th = new[0].t.h as f64;
        let same_size = |gg: &Glyph| (gg.t.h as f64 - th).abs() <= 0.15 * th;
        let mut added: Vec<char> = vec![];
        for gl in new {
            if self.glyphs.iter().filter(|gg| gg.ch == gl.ch && same_size(gg)).count() >= MAX_PER {
                continue;
            }
            let same: Vec<f32> = self.glyphs.iter().filter(|gg| gg.ch == gl.ch).map(|gg| sim(&gl.t, &gl.m, gg)).collect();
            let other: Vec<f32> = self.glyphs.iter().filter(|gg| gg.ch != gl.ch).map(|gg| sim(&gl.t, &gl.m, gg)).collect();
            let max_same = same.iter().cloned().fold(f32::MIN, f32::max);
            let max_other = other.iter().cloned().fold(f32::MIN, f32::max);
            if (!same.is_empty() && max_same < 0.6) || (!other.is_empty() && max_other > if same.is_empty() { 0.85 } else { max_same }) {
                continue;
            }
            added.push(gl.ch);
            self.glyphs.push(gl);
        }
        let old = self.glyphs.len();
        let mut chars: Vec<char> = self.glyphs.iter().filter(|g| same_size(g)).map(|g| g.ch).collect();
        chars.sort();
        chars.dedup();
        if chars.len() >= 6 {
            self.glyphs.retain(|g| same_size(g));
        }
        self.resized = self.glyphs.len() < old;
        added.sort();
        added.dedup();
        added.into_iter().collect()
    }

    pub fn read(&self, bgr: &Bgr, left: f64) -> Option<String> {
        let thr = 0.8f32;
        let g = gray_f32(bgr);
        let mut cands: Vec<Cand> = vec![];
        for (gi, gl) in self.glyphs.iter().enumerate() {
            if g.h < gl.t.h || g.w < gl.t.w {
                continue;
            }
            let mut r = match_masked(&g, &gl.t, &gl.m);
            let (th, tw) = (gl.t.h, gl.t.w);
            for _ in 0..12 {
                let (mx, px, py) = max_loc(&r);
                if mx < thr {
                    break;
                }
                if px as f64 + tw as f64 * 0.5 >= left {
                    cands.push(Cand { mx, px, py, tw, th, ch: gl.ch, pl: gl.pl, pr: gl.pr, gi });
                }
                for yy in py.saturating_sub(th / 2)..(py + th / 2 + 1).min(r.h) {
                    for xx in px.saturating_sub(tw / 2)..(px + tw / 2 + 1).min(r.w) {
                        r.set(xx, yy, -1.0);
                    }
                }
            }
        }
        if cands.is_empty() {
            return None;
        }
        cands.sort_by(|a, b| b.mx.partial_cmp(&a.mx).unwrap());
        let mut kept: Vec<Cand> = vec![];
        for c in cands {
            let (c0, c1) = ((c.px + c.pl) as f64, (c.px + c.tw - c.pr) as f64);
            if kept.iter().all(|k| {
                let (k0, k1) = ((k.px + k.pl) as f64, (k.px + k.tw - k.pr) as f64);
                c1.min(k1) - c0.max(k0) <= 0.35 * (c1 - c0).min((k.tw - k.pl - k.pr) as f64)
            }) {
                kept.push(c);
            }
        }
        let top = kept[0].clone();
        let mut row: Vec<(usize, Cand)> = kept
            .iter()
            .enumerate()
            .filter(|(_, k)| (k.py as f64 - top.py as f64).abs() <= 0.25 * top.th as f64)
            .map(|(i, k)| (i, k.clone()))
            .collect();
        row.sort_by_key(|(_, k)| k.px);
        let mut runs: Vec<Vec<(usize, Cand)>> = vec![];
        let mut cur = vec![row[0].clone()];
        for pair in row.windows(2) {
            let (a, b) = (&pair[0].1, &pair[1].1);
            if b.px as f64 - (a.px + a.tw) as f64 <= if self.punct { 0.9 } else { 0.2 } * a.th as f64 {
                cur.push(pair[1].clone());
            } else {
                runs.push(cur);
                cur = vec![pair[1].clone()];
            }
        }
        runs.push(cur);
        let key = |r: &Vec<(usize, Cand)>| (r.iter().any(|(i, _)| *i == 0), r.len());
        let mut best = 0;
        for i in 1..runs.len() {
            if key(&runs[i]) > key(&runs[best]) {
                best = i;
            }
        }
        let run: Vec<Cand> = runs[best].iter().map(|(_, c)| c.clone()).collect();
        if !self.complete(bgr, &run, left) {
            return None;
        }
        Some(run.iter().map(|k| k.ch).collect())
    }

    fn complete(&self, bgr: &Bgr, run: &[Cand], left: f64) -> bool {
        let punct = self.punct;
        let y0 = run.iter().map(|k| k.py).min().unwrap() as f64;
        let y1 = run.iter().map(|k| k.py + k.th).max().unwrap() as f64;
        let th = y1 - y0;
        let x0 = run.iter().map(|k| k.px).min().unwrap() as f64;
        let x1 = run.iter().map(|k| k.px + k.tw).max().unwrap() as f64;
        let m = digit_fill(bgr);
        let (lab, st) = components(&m, false);
        let (w_, h_) = (m.w, m.h);
        let mut cover = Plane::<u8>::new(w_, h_);
        for k in run {
            let gm = &self.glyphs[k.gi].m;
            for yy in k.py..(k.py + k.th).min(h_) {
                for xx in (k.px + k.pl)..(k.px + k.tw - k.pr).min(w_) {
                    cover.set(xx, yy, 1);
                }
                for xx in k.px..(k.px + k.tw).min(w_) {
                    if gm.at(xx - k.px, yy - k.py) > 0.0 {
                        cover.set(xx, yy, 1);
                    }
                }
            }
            let mb = Plane { w: gm.w, h: gm.h, d: gm.d.iter().map(|&v| (v > 0.0) as u8).collect::<Vec<u8>>() };
            let body = erode(&mb, 1);
            let (mut on, mut total) = (0usize, 0usize);
            for yy in 0..gm.h {
                for xx in 0..gm.w {
                    if body.at(xx, yy) != 0 {
                        total += 1;
                        if m.at(k.px + xx, k.py + yy) != 0 {
                            on += 1;
                        }
                    }
                }
            }
            if (on as f64) < 0.5 * 1.max(total) as f64 {
                return false;
            }
        }
        let n = st.len();
        let mut inside = vec![0usize; n];
        for i in 0..w_ * h_ {
            let l = lab.d[i];
            if l > 0 && cover.d[i] != 0 {
                inside[l as usize - 1] += 1;
            }
        }
        let mut missed = 0usize;
        for (i, c) in st.iter().enumerate() {
            let (cx, cy, cw, ch, area) = (c.x as f64, c.y as f64, c.w as f64, c.h as f64, c.area);
            if area < 3 {
                continue;
            }
            let edge = c.x == 0 || c.y == 0 || c.x + c.w >= w_ || c.y + c.h >= h_;
            if edge && (ch > 1.5 * th || cw > 3.0 * th) {
                continue;
            }
            if (cy + ch).min(y1) - cy.max(y0) < 0.5 * ch.min(th) {
                continue;
            }
            if cy < y0 - 0.3 * th || cy + ch > y1 + 0.3 * th {
                continue;
            }
            if punct && ch < 0.45 * th {
                continue;
            }
            if (!punct && (cx + cw < x0 - 0.3 * th || cx > x1 + 0.3 * th)) || cx + cw * 0.5 < left {
                continue;
            }
            let big = ch >= 0.35 * th && area as f64 >= 0.02 * th * th;
            if edge && big {
                return false;
            }
            if big && (inside[i] as f64 / area as f64) < 0.8 {
                return false;
            }
            missed += area - inside[i];
        }
        (missed as f64) < 0.08 * th * th
    }
}

pub fn sim(t: &Plane<f32>, m: &Plane<f32>, other: &Glyph) -> f32 {
    let (w, h) = (other.t.w, other.t.h);
    let tt = resize_area(t, w, h);
    let mm0 = resize_area(m, w, h);
    let mm = Plane { w, h, d: mm0.d.iter().map(|&v| (v > 0.5) as u8 as f32).collect::<Vec<f32>>() };
    let r = match_masked(&other.t, &tt, &mm);
    r.d.iter().cloned().fold(f32::MIN, f32::max)
}

use serde_json::{json, Value};

pub fn fonts_to_json(fonts: &[(String, &DigitFont)]) -> Value {
    let mut o = serde_json::Map::new();
    for (k, f) in fonts {
        if f.glyphs.is_empty() {
            continue;
        }
        let gl: Vec<Value> = f
            .glyphs
            .iter()
            .map(|g| {
                json!({"ch": g.ch.to_string(), "w": g.t.w, "h": g.t.h, "pl": g.pl, "pr": g.pr,
                       "t": g.t.d.iter().map(|&v| v as u8).collect::<Vec<u8>>(),
                       "m": g.m.d.iter().map(|&v| v as u8).collect::<Vec<u8>>()})
            })
            .collect();
        o.insert(k.clone(), Value::Array(gl));
    }
    Value::Object(o)
}

pub fn fonts_from_json(v: &Value) -> Vec<(String, DigitFont)> {
    let mut out = vec![];
    let Some(o) = v.as_object() else { return out };
    for (k, arr) in o {
        let mut f = DigitFont::new(k == "meso");
        for g in arr.as_array().into_iter().flatten() {
            let (Some(w), Some(h)) = (g["w"].as_u64(), g["h"].as_u64()) else { continue };
            let (w, h) = (w as usize, h as usize);
            let vals = |key: &str| -> Vec<f32> { g[key].as_array().map_or(vec![], |a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()) };
            let (t, m) = (vals("t"), vals("m"));
            if t.len() != w * h || m.len() != w * h {
                continue;
            }
            f.glyphs.push(Glyph {
                ch: g["ch"].as_str().and_then(|s| s.chars().next()).unwrap_or('?'),
                t: Plane { w, h, d: t },
                m: Plane { w, h, d: m },
                pl: g["pl"].as_u64().unwrap_or(0) as usize,
                pr: g["pr"].as_u64().unwrap_or(0) as usize,
            });
        }
        out.push((k.clone(), f));
    }
    out
}
