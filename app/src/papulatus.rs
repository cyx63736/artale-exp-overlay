use crate::zakum::{area, best, half, resize, sums, Tpl, SCALES};
use reader::cv::Plane;
use std::io::Write;

const H: usize = 360;

pub const SAFE: [f32; 10] = [-920.0, -690.5, -446.0, -193.75, 66.0, 333.0, 589.25, 837.0, 1097.0, 1353.0];
const ON_PLATFORM: [f32; 6] = [-920.0, -690.5, -211.0, 40.0, 837.0, 1097.0];
const FLOOR: f32 = 568.0;
pub const ARROW_TOP: f32 = 4.0;
pub const ARROW_H: f32 = 68.0;
const UI_TOP: f32 = 942.0;
const TOP_LEVEL: f32 = 1200.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub x: f32,
    pub y: f32,
    pub u: f32,
}

impl Spot {
    pub fn marks(&self, w: usize, h: usize) -> Option<(Vec<f32>, f32)> {
        let lowest = h as f32 * UI_TOP / 1080.0 - (ARROW_TOP + ARROW_H + 3.0) * self.u;
        if self.top(h) {
            return Some((Vec::new(), lowest));
        }
        let high = self.y + FLOOR * self.u > lowest;
        let floor = (self.y + FLOOR * self.u).min(lowest);
        if floor < 0.0 {
            return None;
        }
        let half_w = 27.0 * self.u;
        let xs: Vec<f32> = (if high { &ON_PLATFORM[..] } else { &SAFE[..] })
            .iter()
            .map(|&d| self.x + d * self.u)
            .filter(|&x| x >= half_w && x <= w as f32 - half_w)
            .collect();
        Some((xs, floor))
    }

    pub fn top(&self, h: usize) -> bool {
        self.y + FLOOR * self.u > h as f32 * TOP_LEVEL / 1080.0
    }

    pub fn spawn(&self, w: usize, h: usize) -> Option<(f32, f32, f32, f32)> {
        let y = self.y + SPAWN_Y * self.u;
        let (y0, y1) = (y - 40.0 * self.u, y + 40.0 * self.u);
        let (lo, hi) = (SPAWN_EDGE * self.u, w as f32 - SPAWN_EDGE * self.u);
        let seen = self.x + SPAWN_CORE.0 * self.u >= lo && self.x + SPAWN_CORE.1 * self.u <= hi;
        (self.top(h) && seen && y0 >= h as f32 * 0.11 && y1 <= h as f32 * UI_TOP / 1080.0).then(|| ((self.x + SPAWN_X.0 * self.u).max(lo), y0, (self.x + SPAWN_X.1 * self.u).min(hi), y1))
    }
}

const SPAWN_X: (f32, f32) = (-150.0, 450.0);
const SPAWN_CORE: (f32, f32) = (20.0, 270.0);
const SPAWN_Y: f32 = -492.0;
const SPAWN_EDGE: f32 = 60.0;

pub struct Clock {
    t3: Tpl,
    t6: Tpl,
    t12: Tpl,
    full: Tpl,
    pub scale: Option<f32>,
    last: Option<(i32, i32)>,
    last_ok: f64,
    log: Option<std::fs::File>,
    pub q: f32,
    spot: Option<Spot>,
    cal: Vec<(f64, f32)>,
    size: (usize, usize),
    resized_at: Option<f64>,
}

const TPL_DX: f32 = 2.0;
const TILE_Q: f32 = 0.55;
const CAL_Q: f32 = 0.7;
const CAL_N: usize = 3;
const CAL_GAP: f64 = 0.5;
const TPL_DY: f32 = 1.0;

impl Clock {
    pub fn new(log: Option<std::fs::File>) -> Clock {
        let t3 = Tpl::new(include_bytes!("papulatus/clock.bin"), 96, 0, 0);
        let t6 = t3.half();
        let t12 = t6.half();
        let full = Tpl::new(include_bytes!("papulatus/clock_full.bin"), 290, 0, 0);
        Clock { t3, t6, t12, full, scale: None, last: None, last_ok: f64::NEG_INFINITY, log, q: 0.0, spot: None, cal: Vec::new(), size: (0, 0), resized_at: None }
    }

    pub fn reset(&mut self) {
        self.scale = None;
        self.last = None;
        self.last_ok = f64::NEG_INFINITY;
        self.spot = None;
        self.cal.clear();
        self.resized_at = None;
    }

    fn logln(&mut self, s: &str) {
        if let Some(f) = self.log.as_mut() {
            let _ = writeln!(f, "{} {s}", tracker::local_iso().chars().take(23).collect::<String>());
        }
    }

    fn prep(img: &ocr::img::Bgr) -> Plane<f32> {
        let sw = ((img.w * H) as f32 / img.h as f32).round().max(1.0) as usize;
        reader::cv::gray_f32(&area(img, sw, H))
    }

    fn calibrate(&mut self, g: &Plane<f32>) -> Option<(f32, i32, i32, f32)> {
        let g6 = half(g);
        let mut cand: Vec<(f32, f32, i32, i32)> = SCALES
            .iter()
            .filter_map(|&s| {
                let r = half(&resize(&g6, (g6.w as f32 / s).round() as usize, (g6.h as f32 / s).round() as usize));
                (r.w > self.t12.w && r.h > self.t12.h).then(|| {
                    let b = best(&r, &sums(&r), &self.t12, 0, 0, r.w as i32, r.h as i32, &[]);
                    (b.0, s, b.1, b.2)
                })
            })
            .collect();
        cand.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut top: Option<(f32, i32, i32, f32)> = None;
        for &(_, s0, x, y) in cand.iter().take(3) {
            for s in [s0 - 0.01, s0, s0 + 0.01] {
                let r = resize(g, (g.w as f32 / s).round() as usize, (g.h as f32 / s).round() as usize);
                let (cx, cy) = ((x as f32 * 4.0 * s0 / s) as i32, (y as f32 * 4.0 * s0 / s) as i32);
                let b = best(&r, &sums(&r), &self.t3, cx - 8, cy - 8, cx + 8, cy + 8, &[]);
                if top.map_or(true, |t| b.0 > t.3) {
                    top = Some((s, b.1, b.2, b.0));
                }
            }
        }
        top
    }

    fn fine_scale(&self, img: &ocr::img::Bgr, s0: f32, x: i32, y: i32) -> f32 {
        let (k, u0) = (s0 * img.h as f32 / H as f32, s0 * img.h as f32 / 1080.0);
        let pad = 24.0 * u0;
        let (x0, y0) = (((x as f32 * k) - pad).max(0.0) as usize, ((y as f32 * k) - pad).max(0.0) as usize);
        let (x1, y1) = (((x as f32 * k) + 290.0 * u0 * 1.06 + pad).min(img.w as f32) as usize, ((y as f32 * k) + 245.0 * u0 * 1.06 + pad).min(img.h as f32) as usize);
        if x1 <= x0 + 10 || y1 <= y0 + 10 {
            return s0;
        }
        let g = reader::cv::gray_f32(&reader::cv::crop_bgr(img, x0, y0, x1, y1));
        let ds: Vec<f32> = (-8..=8).map(|i| i as f32 * 0.005).collect();
        let sc: Vec<f32> = ds
            .iter()
            .map(|d| {
                let u = u0 * (1.0 + d);
                let r = resize(&g, (g.w as f32 / u).round() as usize, (g.h as f32 / u).round() as usize);
                if r.w <= self.full.w || r.h <= self.full.h {
                    return -1.0;
                }
                let (ex, ey) = (((x as f32 * k) - x0 as f32) / u, ((y as f32 * k) - y0 as f32) / u);
                best(&r, &sums(&r), &self.full, ex as i32 - 10, ey as i32 - 10, ex as i32 + 10, ey as i32 + 10, &[]).0
            })
            .collect();
        let i = (0..sc.len()).max_by(|&a, &b| sc[a].total_cmp(&sc[b])).unwrap();
        let mut d = ds[i];
        if i > 0 && i + 1 < sc.len() {
            let c = sc[i - 1] + sc[i + 1] - 2.0 * sc[i];
            if c < -1e-6 {
                d += ((sc[i - 1] - sc[i + 1]) / (2.0 * c)).clamp(-0.5, 0.5) * 0.005;
            }
        }
        s0 * (1.0 + d)
    }

    fn tile_q(&self, r: &Plane<f32>, x: i32, y: i32) -> f32 {
        let t = &self.t3;
        if x < 0 || y < 0 || x as usize + t.w > r.w || y as usize + t.h > r.h {
            return 0.0;
        }
        let mut v: Vec<f32> = (0..9)
            .map(|k| {
                let (ty0, ty1, tx0, tx1) = (k / 3 * t.h / 3, (k / 3 + 1) * t.h / 3, k % 3 * t.w / 3, (k % 3 + 1) * t.w / 3);
                let n = ((ty1 - ty0) * (tx1 - tx0)) as f32;
                let px = |yy: usize, xx: usize| (r.d[(y as usize + yy) * r.w + x as usize + xx], t.raw[yy * t.w + xx]);
                let (mut sa, mut sb) = (0.0f32, 0.0f32);
                for yy in ty0..ty1 {
                    for xx in tx0..tx1 {
                        let (a, b) = px(yy, xx);
                        sa += a;
                        sb += b;
                    }
                }
                let (ma, mb) = (sa / n, sb / n);
                let (mut ab, mut aa, mut bb) = (0.0f32, 0.0f32, 0.0f32);
                for yy in ty0..ty1 {
                    for xx in tx0..tx1 {
                        let (a, b) = px(yy, xx);
                        let (a, b) = (a - ma, b - mb);
                        ab += a * b;
                        aa += a * a;
                        bb += b * b;
                    }
                }
                ab / (aa * bb).sqrt().max(1e-3)
            })
            .collect();
        v.sort_by(|a, b| b.total_cmp(a));
        v[..5].iter().sum::<f32>() / 5.0
    }

    pub fn find(&mut self, img: &ocr::img::Bgr, t: f64) -> Option<Spot> {
        if (img.w, img.h) != self.size {
            if self.scale.is_some() {
                let line = format!("畫面大小變了 {}x{} → {}x{}，重找時鐘", self.size.0, self.size.1, img.w, img.h);
                self.logln(&line);
                self.resized_at = Some(t);
            }
            self.last = None;
            self.spot = None;
            self.cal.clear();
            self.size = (img.w, img.h);
        }
        let g = Clock::prep(img);
        let lost = self.resized_at.is_some_and(|r| self.last_ok < r && t - r > RESIZE_SECS);
        if self.scale.is_some() && (t - self.last_ok > 10.0 || lost) {
            self.resized_at = None;
            self.scale = None;
            self.last = None;
        }
        let shrink = |s: f32| if (s - 1.0).abs() < 0.001 { g.clone() } else { resize(&g, (g.w as f32 / s).round() as usize, (g.h as f32 / s).round() as usize) };
        let (s, r, x, y) = match self.scale {
            None => {
                if self.cal.last().map_or(false, |c| t - c.0 < CAL_GAP) {
                    return None;
                }
                let (s0, x0, y0, _) = self.calibrate(&g)?;
                if (y0 as f32 / H as f32 + 245.0 / 1080.0) * s0 * img.h as f32 > img.h as f32 * UI_TOP / 1080.0 {
                    return None;
                }
                let s1 = self.fine_scale(img, s0, x0, y0);
                let r = shrink(s1);
                let (cx, cy) = ((x0 as f32 * s0 / s1).round() as i32, (y0 as f32 * s0 / s1).round() as i32);
                let (_, x, y) = best(&r, &sums(&r), &self.t3, cx - 3, cy - 3, cx + 3, cy + 3, &[]);
                let q = self.tile_q(&r, x, y);
                self.q = q;
                if q < CAL_Q {
                    return None;
                }
                self.cal.retain(|c| t - c.0 < 10.0);
                self.cal.push((t, s1));
                if self.cal.len() < CAL_N {
                    return None;
                }
                let mut v: Vec<f32> = self.cal.drain(..).map(|c| c.1).collect();
                v.sort_by(f32::total_cmp);
                let s = v[v.len() / 2];
                let r = shrink(s);
                let (cx, cy) = ((x as f32 * s1 / s).round() as i32, (y as f32 * s1 / s).round() as i32);
                let (_, x, y) = best(&r, &sums(&r), &self.t3, cx - 3, cy - 3, cx + 3, cy + 3, &[]);
                let line = format!("時鐘比例 {s:.3}（{}，分數 {q:.2}）畫面 {}x{}", v.iter().map(|s| format!("{s:.3}")).collect::<Vec<_>>().join(" "), img.w, img.h);
                self.logln(&line);
                self.scale = Some(s);
                (s, r, x, y)
            }
            Some(s) => {
                let r = shrink(s);
                let ss = sums(&r);
                let mut b = self.last.map(|(x, y)| best(&r, &ss, &self.t3, x - 14, y - 14, x + 14, y + 14, &[])).map(|b| (self.tile_q(&r, b.1, b.2), b.1, b.2)).filter(|b| b.0 >= TILE_Q);
                if b.is_none() {
                    let r6 = half(&r);
                    let c = best(&r6, &sums(&r6), &self.t6, 0, 0, r6.w as i32, r6.h as i32, &[]);
                    let f = best(&r, &ss, &self.t3, c.1 * 2 - 3, c.2 * 2 - 3, c.1 * 2 + 3, c.2 * 2 + 3, &[]);
                    b = Some((self.tile_q(&r, f.1, f.2), f.1, f.2)).filter(|b| b.0 >= TILE_Q);
                }
                let b = b?;
                self.q = b.0;
                (s, r, b.1, b.2)
            }
        };
        self.last = Some((x, y));
        self.last_ok = t;
        let ss = sums(&r);
        let at = |x: i32, y: i32| best(&r, &ss, &self.t3, x, y, x, y, &[]).0;
        let c = at(x, y);
        let sub = |a: f32, b: f32| {
            let d = a + b - 2.0 * c;
            if d < -1e-6 { ((a - b) / (2.0 * d)).clamp(-0.5, 0.5) } else { 0.0 }
        };
        let fx = x as f32 + sub(at(x - 1, y), at(x + 1, y));
        let fy = y as f32 + sub(at(x, y - 1), at(x, y + 1));
        let k = s * img.h as f32 / H as f32;
        let u = s * img.h as f32 / 1080.0;
        let sp = self.refine(img, 0, 0, Spot { x: fx * k - TPL_DX * u, y: fy * k - TPL_DY * u, u });
        self.spot = Some(sp);
        Some(sp)
    }

    fn refine(&self, img: &ocr::img::Bgr, ox: i32, oy: i32, sp: Spot) -> Spot {
        let u = sp.u;
        let pad = 4.0 * u;
        let (cx, cy) = (sp.x - ox as f32, sp.y - oy as f32);
        let (x0, y0) = ((cx - pad).floor() as i32, (cy - pad).floor() as i32);
        let (x1, y1) = ((cx + 290.0 * u + pad).ceil() as i32, (cy + 245.0 * u + pad).ceil() as i32);
        if x0 < 0 || y0 < 0 || x1 > img.w as i32 || y1 > img.h as i32 {
            return sp;
        }
        let g = reader::cv::gray_f32(&reader::cv::crop_bgr(img, x0 as usize, y0 as usize, x1 as usize, y1 as usize));
        let r = if (u - 1.0).abs() < 0.001 { g } else { resize(&g, ((x1 - x0) as f32 / u).round() as usize, ((y1 - y0) as f32 / u).round() as usize) };
        let (ux, uy) = ((x1 - x0) as f32 / r.w as f32, (y1 - y0) as f32 / r.h as f32);
        let (ex, ey) = (((cx - x0 as f32) / ux).round() as i32, ((cy - y0 as f32) / uy).round() as i32);
        let ss = sums(&r);
        let (q, bx, by) = best(&r, &ss, &self.full, ex - 3, ey - 3, ex + 3, ey + 3, &[]);
        if q < 0.5 {
            return sp;
        }
        let at = |x: i32, y: i32| best(&r, &ss, &self.full, x, y, x, y, &[]).0;
        let sub = |a: f32, b: f32| {
            let d = a + b - 2.0 * q;
            if d < -1e-6 { ((a - b) / (2.0 * d)).clamp(-0.5, 0.5) } else { 0.0 }
        };
        let fx = bx as f32 + sub(at(bx - 1, by), at(bx + 1, by));
        let fy = by as f32 + sub(at(bx, by - 1), at(bx, by + 1));
        Spot { x: ox as f32 + x0 as f32 + fx * ux, y: oy as f32 + y0 as f32 + fy * uy, u }
    }

    pub fn near(&self, w: usize, h: usize) -> Option<(i32, i32, i32, i32)> {
        let sp = self.spot.filter(|_| self.scale.is_some() && self.size == (w, h))?;
        let m = 60.0 * sp.u;
        let (x0, y0) = ((sp.x - m).max(0.0) as i32, (sp.y - m).max(0.0) as i32);
        let (x1, y1) = (((sp.x + 290.0 * sp.u + m) as i32).min(w as i32), ((sp.y + 245.0 * sp.u + m) as i32).min(h as i32));
        (x1 - x0 > 32 && y1 - y0 > 32).then(|| (x0, y0, x1 - x0, y1 - y0))
    }

    pub fn track(&mut self, img: &ocr::img::Bgr, ox: i32, oy: i32, gh: usize, t: f64) -> Option<Spot> {
        let (s, sp) = (self.scale?, self.spot?);
        let k = s * gh as f32 / H as f32;
        let u = s * gh as f32 / 1080.0;
        let (rw, rh) = ((img.w as f32 / k).round() as usize, (img.h as f32 / k).round() as usize);
        if rw <= self.t3.w || rh <= self.t3.h {
            return None;
        }
        let r = reader::cv::gray_f32(&area(img, rw, rh));
        let (ex, ey) = (((sp.x + TPL_DX * u - ox as f32) / k).round() as i32, ((sp.y + TPL_DY * u - oy as f32) / k).round() as i32);
        let r6 = half(&r);
        let m6 = (60.0 * u / k / 2.0).ceil() as i32 + 2;
        let c = best(&r6, &sums(&r6), &self.t6, ex / 2 - m6, ey / 2 - m6, ex / 2 + m6, ey / 2 + m6, &[]);
        let ss = sums(&r);
        let (_, x, y) = best(&r, &ss, &self.t3, c.1 * 2 - 2, c.2 * 2 - 2, c.1 * 2 + 2, c.2 * 2 + 2, &[]);
        let q = self.tile_q(&r, x, y);
        if q < TILE_Q {
            return None;
        }
        self.q = q;
        let (kx, ky) = (img.w as f32 / rw as f32, img.h as f32 / rh as f32);
        let coarse = Spot { x: ox as f32 + x as f32 * kx - TPL_DX * u, y: oy as f32 + y as f32 * ky - TPL_DY * u, u };
        let sp = self.refine(img, ox, oy, coarse);
        self.last = Some((((sp.x + TPL_DX * u) / k).round() as i32, ((sp.y + TPL_DY * u) / k).round() as i32));
        self.last_ok = t;
        self.spot = Some(sp);
        Some(sp)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Band {
    pub y0: usize,
    pub y1: usize,
    pub x0: usize,
    pub x1: usize,
    pub n: usize,
}

fn yellow(p: &[u8]) -> bool {
    let (b, g, r) = (p[0] as i32, p[1] as i32, p[2] as i32);
    r > 140 && g > 140 && (r - g).abs() < 70 && b * 10 < r.min(g) * 6
}

fn yellow_lines(img: &ocr::img::Bgr, mask: &[(i32, i32, i32, i32)]) -> Vec<Band> {
    let (w, h) = (img.w, img.h);
    let (xr, ya, yb) = (w * 65 / 100, h * 60 / 100, h * 96 / 100);
    let xl = w * 22 / 100;
    let rows: Vec<usize> = (ya..yb).map(|y| img.data[y * w * 3..(y * w + xl) * 3].chunks_exact(3).filter(|p| yellow(p)).count()).collect();
    let (lo, hi) = ((h / 180).max(4), (h / 32).max(8));
    let th = (h / 180).max(3);
    let mut out = vec![];
    let mut y = 0;
    while y < rows.len() {
        if rows[y] < th {
            y += 1;
            continue;
        }
        let a = y;
        while y < rows.len() && (rows[y] >= th || (y + 1 < rows.len() && rows[y + 1] >= th)) {
            y += 1;
        }
        let (y0, y1) = (ya + a, ya + y);
        let bh = y1 - y0;
        if bh < lo || bh > hi {
            continue;
        }
        let cols: Vec<usize> = (0..xr).map(|x| (y0..y1).filter(|&yy| yellow(img.px(x, yy))).count()).collect();
        let hidden: Vec<bool> = (0..xr).map(|x| mask.iter().any(|&(mx0, my0, mx1, my1)| (mx0..mx1).contains(&(x as i32)) && my0 < y1 as i32 && my1 > y0 as i32)).collect();
        let mut x = 0;
        while x < xr && x <= w * 15 / 100 {
            if cols[x] == 0 {
                x += 1;
                continue;
            }
            let (x0, mut x1, mut gap, mut n) = (x, x, 0, 0);
            while x < xr {
                if cols[x] > 0 {
                    x1 = x + 1;
                    gap = 0;
                    n += cols[x];
                } else if !hidden[x] {
                    gap += 1;
                    if gap > bh * 2 {
                        break;
                    }
                }
                x += 1;
            }
            let fill = n as f32 / ((x1 - x0) * bh) as f32;
            if x1 - x0 >= w * 6 / 100 && (0.05..0.7).contains(&fill) {
                out.push(Band { y0, y1, x0, x1, n });
                break;
            }
        }
    }
    out
}

pub fn is_foot(s: &str) -> bool {
    s.contains("好好") || "走路要注意腳下".chars().filter(|c| s.contains(*c)).count() >= 3
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Water {
    Now,
    Soon,
}

impl Water {
    pub fn text(&self) -> &'static str {
        match self {
            Water::Now => "黑水！",
            Water::Soon => "黑水快來了",
        }
    }
}

const NOW_SECS: f64 = 3.0;

const SOON_SECS: f64 = 55.0;
const SOON_END: f64 = 120.0;
const MISS_GAP: f64 = 85.0;

struct Seen {
    x0: usize,
    w: usize,
    n: usize,
    foot: bool,
    reads: u8,
    at: f64,
}

const RESIZE_SECS: f64 = 2.0;

pub struct Foot {
    seen: Vec<Seen>,
    bottom: Option<(usize, usize)>,
    old_bottom: Option<usize>,
    was: bool,
    pub last: Option<f64>,
    last_ocr: f64,
    last_seen: f64,
    feet_at: Vec<(usize, f64)>,
    gone_at: Option<f64>,
    hid_at: Option<f64>,
    size: (usize, usize),
    resized_at: Option<f64>,
}

impl Default for Foot {
    fn default() -> Self {
        Foot { seen: vec![], bottom: None, old_bottom: None, was: false, last: None, last_ocr: f64::NEG_INFINITY, last_seen: f64::NEG_INFINITY, feet_at: vec![], gone_at: None, hid_at: None, size: (0, 0), resized_at: None }
    }
}

impl Foot {
    pub fn reset(&mut self) {
        self.bottom = None;
        self.old_bottom = None;
        self.was = false;
        self.last = None;
        self.last_seen = f64::NEG_INFINITY;
        self.feet_at.clear();
        self.gone_at = None;
        self.hid_at = None;
        self.resized_at = None;
    }

    fn find(&self, b: &Band) -> Option<usize> {
        let w = b.x1 - b.x0;
        let same = |s: &Seen| s.x0.abs_diff(b.x0) <= 3 && s.w.abs_diff(w) <= 4 && (s.n.abs_diff(b.n) as f32) <= s.n as f32 * 0.12;
        self.seen.iter().rposition(|s| s.foot && same(s)).or_else(|| self.seen.iter().rposition(same))
    }

    fn known(&self, b: &Band) -> Option<bool> {
        self.find(b).map(|i| self.seen[i].foot)
    }

    pub fn step(&mut self, img: &ocr::img::Bgr, t: f64, mask: &[(i32, i32, i32, i32)], ocr: &mut dyn FnMut(&ocr::img::Bgr) -> String) -> bool {
        let masked;
        let img = if mask.is_empty() {
            img
        } else {
            let mut m = img.clone();
            for &(x0, y0, x1, y1) in mask {
                let (x0, x1) = (x0.clamp(0, m.w as i32) as usize, x1.clamp(0, m.w as i32) as usize);
                for y in y0.clamp(0, m.h as i32) as usize..y1.clamp(0, m.h as i32) as usize {
                    m.data[(y * m.w + x0) * 3..(y * m.w + x1) * 3].fill(0);
                }
            }
            masked = m;
            &masked
        };
        if (img.w, img.h) != self.size {
            if self.size != (0, 0) {
                self.bottom = None;
                self.old_bottom = None;
                self.feet_at.clear();
                self.gone_at = None;
                self.hid_at = None;
                self.seen.clear();
                self.resized_at = Some(t);
            }
            self.size = (img.w, img.h);
        }
        if t - self.last_seen > 20.0 && self.bottom.is_some() {
            self.old_bottom = self.bottom.map(|b| b.0);
            self.bottom = None;
        }
        let tol = (img.h / 100).max(4);
        let mut state: Option<bool> = Some(false);
        let mut lowest_foot = None;
        let mut feet = vec![];
        let mut hid = false;
        let bands = yellow_lines(img, mask);
        for &b in bands.iter().rev() {
            let bh = b.y1 - b.y0;
            let covered = mask.iter().any(|&(mx0, my0, mx1, my1)| my0 < b.y1 as i32 && my1 > b.y0 as i32 && mx0 <= (b.x1 + bh * 2) as i32 && mx1 + bh as i32 * 2 >= b.x0 as i32);
            let b = if covered { Band { y0: b.y1.saturating_sub(img.h * 19 / 1080), ..b } } else { b };
            let at_bottom = self.bottom.map_or(false, |(y, _)| b.y0.abs_diff(y) <= tol);
            if self.bottom.map_or(false, |(y, x)| b.y0 + tol < y || b.x0.abs_diff(x) > 20) {
                if self.known(&b) == Some(true) {
                    feet.push(b.y0);
                }
                continue;
            }
            if at_bottom {
                self.last_seen = t;
            }
            if covered {
                let foot = if t - self.last_ocr >= 0.3 {
                    self.last_ocr = t;
                    let pad = bh / 2 + 2;
                    let (x0, y0) = (b.x0.saturating_sub(pad), b.y0.saturating_sub(pad));
                    let (x1, y1) = ((b.x1 + pad).min(img.w), (b.y1 + pad).min(img.h));
                    let f = is_foot(&ocr(&reader::cv::crop_bgr(img, x0, y0, x1, y1)));
                    if cfg!(test) && std::env::var("PAPU_TRACE").is_ok() {
                        println!("{t:.2} covered {b:?} bottom {at_bottom} foot {f} was {}", self.was);
                    }
                    f.then_some(true)
                } else {
                    None
                };
                if foot == Some(true) {
                    feet.push(b.y0);
                    if lowest_foot.is_none() {
                        lowest_foot = Some((b.y0, b.x0, b.y1 - b.y0));
                    }
                }
                if at_bottom || lowest_foot == Some((b.y0, b.x0, b.y1 - b.y0)) && self.bottom.map_or(true, |(y, _)| b.y0 > y + tol) {
                    state = foot;
                    hid |= foot.is_none();
                }
                continue;
            }
            let idx = self.find(&b);
            let again = idx.map_or(true, |i| at_bottom && !self.seen[i].foot && self.seen[i].reads < 3 && t - self.seen[i].at >= 1.0);
            let foot = if again && t - self.last_ocr >= 0.3 {
                self.last_ocr = t;
                let pad = (b.y1 - b.y0) / 2 + 2;
                let (x0, y0) = (b.x0.saturating_sub(pad), b.y0.saturating_sub(pad));
                let (x1, y1) = ((b.x1 + pad).min(img.w), (b.y1 + pad).min(img.h));
                let f = is_foot(&ocr(&reader::cv::crop_bgr(img, x0, y0, x1, y1)));
                if cfg!(test) && std::env::var("PAPU_TRACE").is_ok() {
                    println!("{t:.2} band {b:?} bottom {at_bottom} foot {f} was {}", self.was);
                }
                match idx {
                    Some(i) => {
                        let s = &mut self.seen[i];
                        s.foot |= f;
                        s.reads += 1;
                        s.at = t;
                    }
                    None => {
                        if self.seen.len() >= 256 {
                            self.seen.remove(0);
                        }
                        self.seen.push(Seen { x0: b.x0, w: b.x1 - b.x0, n: b.n, foot: f, reads: 1, at: t });
                    }
                }
                Some(self.known(&b).unwrap_or(f))
            } else {
                idx.map(|i| self.seen[i].foot)
            };
            if foot == Some(true) {
                feet.push(b.y0);
                if lowest_foot.is_none() {
                    lowest_foot = Some((b.y0, b.x0, b.y1 - b.y0));
                }
            }
            if at_bottom || lowest_foot == Some((b.y0, b.x0, b.y1 - b.y0)) && self.bottom.map_or(true, |(y, _)| b.y0 > y + tol) {
                state = foot;
            }
        }
        let had = |y: usize| self.feet_at.iter().any(|f| f.0.abs_diff(y) <= tol && t - f.1 <= 3.0);
        let fresh = lowest_foot.map_or(false, |(y, _, _)| had(y));
        self.feet_at.retain(|f| t - f.1 <= 3.0 && !feet.iter().any(|&y| y.abs_diff(f.0) <= tol));
        self.feet_at.extend(feet.iter().map(|&y| (y, t)));
        if let Some((y, x, bh)) = lowest_foot {
            if self.bottom.map_or(true, |(b, _)| y > b + tol) {
                let below = bands.iter().any(|b| b.y0 > y + tol) || self.bottom.is_none() && self.old_bottom.map_or(false, |o| y + bh * 3 < o);
                if below {
                    state = Some(self.was);
                } else {
                    self.was = self.bottom.is_none() && (fresh || self.resized_at.is_some_and(|r| t - r <= RESIZE_SECS));
                    self.bottom = Some((y, x));
                    self.last_seen = t;
                }
            }
        }
        if let (Some(false), Some((y, x))) = (state, self.bottom) {
            let bh = (img.h / 50) as i32;
            let (y, x) = (y as i32, x as i32);
            if !bands.iter().any(|b| b.y0.abs_diff(y as usize) <= tol) && mask.iter().any(|&(mx0, my0, mx1, my1)| mx0 <= x + bh * 2 && mx1 >= x && my0 < y + bh && my1 > y) {
                state = None;
                hid = true;
                self.last_seen = t;
            }
        }
        if hid {
            if state.is_none() && t - *self.hid_at.get_or_insert(t) >= 10.0 {
                state = Some(false);
            }
        } else {
            self.hid_at = None;
        }
        let Some(now) = state else { return false };
        let new = now && !self.was && self.last.map_or(true, |l| t - l >= 40.0);
        if cfg!(test) && std::env::var("PAPU_TRACE").is_ok() && now != self.was {
            println!("{t:.2} bottom {:?} {now}", self.bottom);
        }
        if now {
            self.was = true;
            self.gone_at = None;
        } else if self.was && t - *self.gone_at.get_or_insert(t) >= 1.5 {
            self.was = false;
        }
        if new {
            self.last = Some(t);
        }
        new
    }

    pub fn show(&self, t: f64) -> Option<Water> {
        let mut d = t - self.last?;
        if d < NOW_SECS {
            return Some(Water::Now);
        }
        for _ in 0..2 {
            if d >= SOON_END {
                d -= MISS_GAP;
            }
        }
        (SOON_SECS..SOON_END).contains(&d).then_some(Water::Soon)
    }
}

const MOB_LIFE: f64 = 44.0;
const MOB_SOON: f64 = 20.0;
const MOB_WATCH: f64 = 3.0;
const MOB_AWAY: f64 = 2.0;
const MOB_CONFIRM: f64 = 2.0;
pub const MOB_PCT: u32 = 90;

const BADGE: &[u8] = include_bytes!("papulatus/badge.bin");
const BADGE_COARSE: f32 = 0.68;
const BADGE_Q: f32 = 0.8;

struct Badge {
    r: i32,
    pts: Vec<(i32, i32)>,
    tpls: Vec<[(Vec<f32>, f32); 3]>,
}

impl Badge {
    fn new(data: &[u8], r: i32) -> Badge {
        let n = (2 * r + 1) as usize;
        let pts: Vec<(i32, i32)> = (-r..=r).flat_map(|y| (-r..=r).map(move |x| (x, y))).filter(|&(x, y)| x * x + y * y <= r * r && y * 12 <= r * 5).collect();
        let tpls = data
            .chunks(n * n * 3)
            .map(|t| {
                std::array::from_fn(|c| {
                    let v: Vec<f32> = pts.iter().map(|&(x, y)| t[((y + r) as usize * n + (x + r) as usize) * 3 + c] as f32).collect();
                    let m = v.iter().sum::<f32>() / v.len() as f32;
                    let d: Vec<f32> = v.iter().map(|a| a - m).collect();
                    let len = d.iter().map(|a| a * a).sum::<f32>().sqrt().max(1e-6);
                    (d, len)
                })
            })
            .collect();
        Badge { r, pts, tpls }
    }

    fn score(&self, p: &[Plane<f32>; 3], x: i32, y: i32, buf: &mut Vec<f32>) -> f32 {
        let w = p[0].w as i32;
        let n = self.pts.len() as f32;
        let mut sc = vec![0.0f32; self.tpls.len()];
        for c in 0..3 {
            buf.clear();
            buf.extend(self.pts.iter().map(|&(dx, dy)| p[c].d[((y + dy) * w + x + dx) as usize]));
            let s1: f32 = buf.iter().sum();
            let s2: f32 = buf.iter().map(|a| a * a).sum();
            let sd = (s2 - s1 * s1 / n).max(n * 100.0).sqrt();
            for (k, t) in self.tpls.iter().enumerate() {
                let (d, len) = &t[c];
                sc[k] += buf.iter().zip(d).map(|(a, b)| a * b).sum::<f32>() / (sd * len) / 3.0;
            }
        }
        sc.into_iter().fold(-1.0, f32::max)
    }
}

pub struct Bubbles {
    fine: Badge,
    coarse: Badge,
}

impl Default for Bubbles {
    fn default() -> Self {
        let big = 25 * 25 * 3 * 3;
        Bubbles { fine: Badge::new(&BADGE[..big], 12), coarse: Badge::new(&BADGE[big..], 6) }
    }
}

impl Bubbles {
    pub fn find(&self, img: &ocr::img::Bgr, u: f32, zone: Option<(f32, f32, f32, f32)>) -> (Option<f32>, bool) {
        let k = 3.0 * u;
        let (w, h) = ((img.w as f32 / k).round() as usize, (img.h as f32 / k).round() as usize);
        if w < 64 || h < 64 {
            return (None, false);
        }
        let small = area(img, w, h);
        let fine: [Plane<f32>; 3] = std::array::from_fn(|c| Plane { w, h, d: small.data.chunks(3).map(|p| p[c] as f32).collect() });
        let coarse: [Plane<f32>; 3] = std::array::from_fn(|c| half(&fine[c]));
        let (top, bottom) = (h as i32 * 11 / 100, (h as f32 * UI_TOP / 1080.0) as i32);
        let mut buf = Vec::new();
        let r = self.coarse.r;
        let mut cands = vec![];
        for y in (top / 2 + r)..(bottom / 2 - r) {
            for x in r..coarse[0].w as i32 - r {
                let s = self.coarse.score(&coarse, x, y, &mut buf);
                if s >= BADGE_COARSE {
                    cands.push((s, x, y));
                }
            }
        }
        cands.sort_by(|a, b| b.0.total_cmp(&a.0));
        let inside = |&&(_, cx, cy): &&(f32, i32, i32)| zone.is_some_and(|(x0, y0, x1, y1)| (x0..=x1).contains(&(2.0 * cx as f32 * k)) && (y0..=y1).contains(&(2.0 * cy as f32 * k)));
        let near: Vec<(f32, i32, i32)> = cands.iter().filter(inside).take(20).copied().collect();
        cands.truncate(60);
        let r = self.fine.r;
        let mut refine = |list: &[(f32, i32, i32)]| {
            let mut best = -1.0f32;
            for &(_, cx, cy) in list {
                for y in (2 * cy - 2)..=(2 * cy + 3) {
                    for x in (2 * cx - 2)..=(2 * cx + 3) {
                        if y - r >= top && y + r < bottom && x - r >= 0 && x + r < w as i32 {
                            best = best.max(self.fine.score(&fine, x, y, &mut buf));
                        }
                    }
                }
                if best >= BADGE_Q {
                    break;
                }
            }
            best
        };
        let best = refine(&cands);
        let born = refine(&near) >= BADGE_Q;
        ((best >= BADGE_Q).then_some(best), born)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mob {
    Left(u32),
    Soon,
}

pub struct Mobs {
    wave: Option<f64>,
    last: f64,
    view_since: Option<f64>,
    off_since: Option<f64>,
    hits: Vec<f64>,
    born: Vec<f64>,
    pending: Option<f64>,
}

impl Default for Mobs {
    fn default() -> Self {
        Mobs { wave: None, last: f64::NEG_INFINITY, view_since: None, off_since: None, hits: vec![], born: vec![], pending: None }
    }
}

impl Mobs {
    pub fn step(&mut self, t: f64, view: bool, seen: bool, born: bool) -> bool {
        if view {
            self.view_since.get_or_insert(t);
            self.off_since = None;
        } else if t - *self.off_since.get_or_insert(t) >= MOB_AWAY {
            self.view_since = None;
        }
        self.hits.retain(|&h| t - h < 1.0);
        self.born.retain(|&h| t - h < 1.0);
        if seen {
            self.hits.push(t);
        }
        if born && view {
            self.born.push(t);
        }
        let mut new = false;
        if self.pending.is_none() && self.born.len() >= 2 {
            let first = self.born[0];
            let done = self.wave.map_or(true, |w| first - w >= MOB_LIFE + 0.5);
            let watched = self.view_since.is_some_and(|s| first - s >= MOB_WATCH);
            let quiet = first - self.last >= MOB_WATCH || self.wave.is_some_and(|w| self.last <= w + MOB_LIFE + 0.5);
            if done && watched && quiet {
                self.pending = Some(first);
            }
        }
        if self.hits.len() >= 2 {
            self.last = t;
        }
        if let Some(p) = self.pending {
            if self.last >= p + MOB_CONFIRM {
                self.wave = Some(p);
                self.pending = None;
                new = true;
            } else if t - p > MOB_CONFIRM + 1.0 {
                self.pending = None;
            }
        }
        new
    }

    pub fn show(&self, t: f64) -> Option<Mob> {
        let d = t - self.wave?;
        if d < MOB_LIFE {
            Some(Mob::Left((MOB_LIFE - d).ceil() as u32))
        } else {
            (d < MOB_LIFE + MOB_SOON).then_some(Mob::Soon)
        }
    }
}

pub fn papu_pct(name: &str, hp: i64, pct: u32, known: bool) -> Option<u32> {
    let hp = hp as f64 * 100.0;
    let (lo, hi) = (hp / (pct + 1) as f64 * 0.98, if pct == 0 { f64::INFINITY } else { hp / pct as f64 * 1.02 });
    let fit = lo <= 167_000_000.0 && hi >= 155_000_000.0;
    let named = ["拉圖", "拉圓", "拉圆", "拉图", "圖斯", "圓斯", "圆斯", "图斯"].iter().any(|n| name.contains(n));
    (fit && (named || known || pct >= 10)).then_some(pct)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Marks {
    pub left: i32,
    pub floor: i32,
    pub w: i32,
    pub xs: Vec<i32>,
    pub u: f32,
}

impl Marks {
    pub fn area(&self, ox: i32, oy: i32) -> Vec<(i32, i32, i32, i32)> {
        let (hw, pad) = ((27.0 * self.u).ceil() as i32 + 3, (3.0 * self.u).ceil() as i32 + 3);
        let top = self.floor + (ARROW_TOP * self.u).round() as i32 - pad - oy;
        let bot = self.floor + ((ARROW_TOP + ARROW_H) * self.u).round() as i32 + pad - oy;
        self.xs.iter().map(|&x| (self.left + x - hw - ox, top, self.left + x + hw - ox, bot)).collect()
    }
}

pub fn arrow_sprite(u: f32) -> (usize, usize, Vec<u32>) {
    let src = image::load_from_memory_with_format(include_bytes!("papulatus/arrow.png"), image::ImageFormat::Png).map(|i| i.to_rgba8()).ok();
    let Some(src) = src else { return (0, 0, vec![]) };
    let (sw, sh) = (src.width() as usize, src.height() as usize);
    let pm: Vec<[f32; 4]> = src
        .pixels()
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            [p[2] as f32 * a, p[1] as f32 * a, p[0] as f32 * a, p[3] as f32]
        })
        .collect();
    let (w, h) = (((sw as f32 * u).round() as usize).max(4), ((sh as f32 * u).round() as usize).max(5));
    let (kx, ky) = (sw as f32 / w as f32, sh as f32 / h as f32);
    let mut out = vec![0u32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut c = [0f32; 4];
            let n = (kx.ceil().max(1.0) as usize, ky.ceil().max(1.0) as usize);
            for j in 0..n.1 {
                for i in 0..n.0 {
                    let fx = ((x as f32 + (i as f32 + 0.5) / n.0 as f32) * kx - 0.5).clamp(0.0, (sw - 1) as f32);
                    let fy = ((y as f32 + (j as f32 + 0.5) / n.1 as f32) * ky - 0.5).clamp(0.0, (sh - 1) as f32);
                    let (x0, y0) = (fx as usize, fy as usize);
                    let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
                    let (ax, ay) = (fx.fract(), fy.fract());
                    for k in 0..4 {
                        let top = pm[y0 * sw + x0][k] * (1.0 - ax) + pm[y0 * sw + x1][k] * ax;
                        let bot = pm[y1 * sw + x0][k] * (1.0 - ax) + pm[y1 * sw + x1][k] * ax;
                        c[k] += top * (1.0 - ay) + bot * ay;
                    }
                }
            }
            let m = (n.0 * n.1) as f32;
            let px = |v: f32| (v / m).round().clamp(0.0, 255.0) as u32;
            out[y * w + x] = px(c[0]) | px(c[1]) << 8 | px(c[2]) << 16 | px(c[3]) << 24;
        }
    }
    (w, h, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn papu_hp() {
        assert_eq!(papu_pct("拉圖斯", 159_936_693, 99, false), Some(99));
        assert_eq!(papu_pct("拉圓斯", 71_140_848, 44, false), Some(44));
        assert_eq!(papu_pct("拉圖斯", 1_000_000, 0, false), Some(0));
        assert_eq!(papu_pct("殘暴炎魔", 33_000_000, 50, false), None);
        assert_eq!(papu_pct("某某", 1_000_000, 3, false), None);
        assert_eq!(papu_pct("拉圆斯", 11_000_000, 6, false), Some(6));
        assert_eq!(papu_pct("拉图斯", 11_000_000, 6, false), Some(6));
        assert_eq!(papu_pct("拉斯", 11_000_000, 6, false), None);
        assert_eq!(papu_pct("拉斯", 11_000_000, 6, true), Some(6));
        assert_eq!(papu_pct("拉斯", 33_000_000, 6, true), None);
    }

    #[test]
    fn papu_marks_high() {
        let on_floor = Spot { x: 1500.0, y: 100.0, u: 1.0 }.marks(3840, 1080).unwrap();
        assert_eq!(on_floor.0.len(), 10);
        assert_eq!(on_floor.1, 668.0);
        let on_platform = Spot { x: 1500.0, y: 400.0, u: 1.0 }.marks(3840, 1080).unwrap();
        assert_eq!(on_platform.1, 867.0);
        assert_eq!(on_platform.0, vec![580.0, 809.5, 1289.0, 1540.0, 2337.0, 2597.0]);
        let on_top = Spot { x: 1500.0, y: 750.0, u: 1.0 }.marks(3840, 1080).unwrap();
        assert!(on_top.0.is_empty());
    }

    #[test]
    fn foot_cache() {
        let mut f = Foot::default();
        let b = Band { y0: 1824, y1: 1863, x0: 32, x1: 641, n: 9286 };
        f.seen.push(Seen { x0: 32, w: 609, n: 9300, foot: true, reads: 1, at: 0.0 });
        f.seen.push(Seen { x0: 33, w: 607, n: 8400, foot: false, reads: 3, at: 50.0 });
        assert_eq!(f.known(&b), Some(true));
        let other = Band { y0: 1824, y1: 1863, x0: 32, x1: 520, n: 7300 };
        f.seen.push(Seen { x0: 32, w: 488, n: 7300, foot: false, reads: 3, at: 60.0 });
        assert_eq!(f.known(&other), Some(false));
    }

    #[test]
    fn foot_timing() {
        let mut f = Foot::default();
        assert_eq!(f.show(0.0), None);
        f.last = Some(100.0);
        assert_eq!(f.show(102.0), Some(Water::Now));
        assert_eq!(f.show(103.0), None);
        assert_eq!(f.show(120.0), None);
        assert_eq!(f.show(156.0), Some(Water::Soon));
        assert_eq!(f.show(221.0), None);
        assert_eq!(f.show(240.0), Some(Water::Soon));
        assert_eq!(f.show(325.0), Some(Water::Soon));
        assert_eq!(f.show(390.0), None);
    }

    #[test]
    fn foot_text() {
        assert!(is_foot("拉圖斯:走路時要好好注腳下。"));
        assert!(is_foot("拉圓斯:走路時要好好意腳下。"));
        assert!(is_foot("拉固斯;走路时要好好注意胍下。"));
        assert!(is_foot("拉回斯：定路时要好好注息胍卜。"));
        assert!(!is_foot("拉圖斯：我來介紹有趣的朋友給你認識。"));
        assert!(!is_foot("套翠端午的冒险家"));
        assert!(!is_foot("拉圖斯:鬧鐘時間到了!!"));
    }

    fn run_ffmpeg(video: &str, a: f64, b: f64, fps: f64, w: usize, h: usize) -> std::process::Child {
        run_ffmpeg_vf(video, a, b, fps, &format!("scale={w}:{h}"))
    }

    fn run_ffmpeg_vf(video: &str, a: f64, b: f64, fps: f64, vf: &str) -> std::process::Child {
        let mut cmd = std::process::Command::new("ffmpeg");
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
        let pre = std::env::var("PAPU_VF").map(|v| format!("{v},")).unwrap_or_default();
        cmd.args(["-v", "error", "-ss", &a.to_string(), "-t", &(b - a).to_string(), "-i", video, "-vf", &format!("fps={fps},{pre}{vf}"), "-f", "rawvideo", "-pix_fmt", "bgr24", "-"])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    }

    fn sizes(a: f64, b: f64, w: usize, h: usize) -> Vec<(f64, f64, usize, usize, String)> {
        let Ok(v) = std::env::var("PAPU_SIZES") else { return vec![(a, b, w, h, format!("scale={w}:{h}"))] };
        let s: Vec<(f64, usize, usize, String)> = v
            .split(',')
            .map(|p| {
                let (wh, at) = p.split_once('@').unwrap();
                let (sw, sh) = wh.trim_start_matches('c').split_once('x').unwrap();
                let (sw, sh): (usize, usize) = (sw.parse().unwrap(), sh.parse().unwrap());
                let vf = if wh.starts_with('c') { format!("scale={w}:{h},crop={sw}:{sh}") } else { format!("scale={sw}:{sh}") };
                (a + at.parse::<f64>().unwrap(), sw, sh, vf)
            })
            .collect();
        (0..s.len()).map(|i| (s[i].0, s.get(i + 1).map_or(b, |n| n.0), s[i].1, s[i].2, s[i].3.clone())).collect()
    }

    #[test]
    #[ignore]
    fn papu_clock() {
        use std::io::Read;
        let video = std::env::var("PAPU_VIDEO").unwrap();
        let seg = std::env::var("PAPU_SEG").unwrap();
        let (a, b) = seg.split_once('-').unwrap();
        let (a, b): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
        let fps: f64 = std::env::var("PAPU_FPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let (w0, h0) = std::env::var("PAPU_SIZE").ok().and_then(|v| v.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((1920usize, 1080usize));
        let mut c = Clock::new(None);
        let mut i = 0;
        let mut spent = vec![];
        for (sa, sb, w, h, vf) in sizes(a, b, w0, h0) {
            let mut ff = run_ffmpeg_vf(&video, sa, sb, fps, &vf);
            let mut out = ff.stdout.take().unwrap();
            let mut img = ocr::img::Bgr::new(w, h);
            let mut j = 0;
            while out.read_exact(&mut img.data).is_ok() {
                let t = sa + j as f64 / fps;
                j += 1;
                let t0 = std::time::Instant::now();
            let near = c.near(w, h).filter(|_| std::env::var("PAPU_TRACK").is_ok());
            let s = near.and_then(|(x, y, rw, rh)| c.track(&reader::cv::crop_bgr(&img, x as usize, y as usize, (x + rw) as usize, (y + rh) as usize), x, y, h, t)).or_else(|| c.find(&img, t));
            spent.push(t0.elapsed().as_secs_f64() * 1000.0);
            match s {
                Some(s) => println!("{t:.2} {:.2} {:.2} u{:.3} q{:.2}", s.x, s.y, s.u, c.q),
                None => println!("{t:.2} -"),
            }
            i += 1;
        }
        let _ = ff.wait();
        }
        spent.sort_by(f64::total_cmp);
        println!("frames {i} median {:.1} ms max {:.1} ms", spent.get(spent.len() / 2).copied().unwrap_or(0.0), spent.last().copied().unwrap_or(0.0));
    }

    #[test]
    fn papu_mobs_logic() {
        let feed = |m: &mut Mobs, a: f64, b: f64, view: bool, seen: bool, born: bool| {
            let mut t = a;
            while t < b {
                m.step(t, view, seen, born);
                t += 0.25;
            }
        };
        let mut m = Mobs::default();
        feed(&mut m, 0.0, 5.0, true, false, false);
        feed(&mut m, 5.0, 20.0, true, true, true);
        assert_eq!(m.wave, Some(5.0));
        assert_eq!(m.show(20.0), Some(Mob::Left(29)));
        feed(&mut m, 20.0, 30.0, false, false, false);
        assert_eq!(m.show(30.0), Some(Mob::Left(19)));
        feed(&mut m, 30.0, 49.0, true, true, false);
        assert_eq!(m.wave, Some(5.0));
        assert_eq!(m.show(50.0), Some(Mob::Soon));
        feed(&mut m, 49.0, 60.0, false, false, false);
        feed(&mut m, 60.0, 70.0, true, true, false);
        assert_eq!(m.wave, Some(5.0));
        assert_eq!(m.show(200.0), None);
        let mut m = Mobs::default();
        feed(&mut m, 0.0, 10.0, true, true, true);
        assert_eq!(m.wave, None);
        let mut m = Mobs::default();
        feed(&mut m, 0.0, 5.0, true, false, false);
        feed(&mut m, 5.0, 49.0, true, true, true);
        feed(&mut m, 49.0, 51.0, true, false, false);
        feed(&mut m, 51.0, 54.0, true, true, true);
        assert_eq!(m.wave, Some(51.0));
        let mut m = Mobs::default();
        feed(&mut m, 0.0, 5.0, true, false, false);
        feed(&mut m, 5.0, 20.0, true, true, true);
        feed(&mut m, 20.0, 49.5, true, true, false);
        feed(&mut m, 49.5, 50.25, true, true, true);
        feed(&mut m, 50.25, 58.0, true, false, false);
        assert_eq!(m.wave, Some(5.0));
        feed(&mut m, 58.0, 62.0, true, true, true);
        assert_eq!(m.wave, Some(58.0));
        let mut m = Mobs::default();
        feed(&mut m, 0.0, 5.0, true, false, false);
        feed(&mut m, 5.0, 6.5, false, false, false);
        feed(&mut m, 6.5, 8.0, true, false, false);
        feed(&mut m, 8.0, 11.0, true, true, true);
        assert_eq!(m.wave, Some(8.0));
    }

    #[test]
    #[ignore]
    fn papu_mobs() {
        use std::io::Read;
        let video = std::env::var("PAPU_VIDEO").unwrap();
        let seg = std::env::var("PAPU_SEG").unwrap();
        let (a, b) = seg.split_once('-').unwrap();
        let (a, b): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
        let fps: f64 = std::env::var("PAPU_FPS").ok().and_then(|v| v.parse().ok()).unwrap_or(4.0);
        let (w0, h0) = std::env::var("PAPU_SIZE").ok().and_then(|v| v.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((1920usize, 1080usize));
        let debug = std::env::var("PAPU_DEBUG").is_ok();
        let mut c = Clock::new(None);
        let bubbles = Bubbles::default();
        let mut mobs = Mobs::default();
        let (mut i, mut u) = (0, None);
        let mut spent = vec![];
        let mut waves: Vec<f64> = vec![];
        for (sa, sb, w, h, vf) in sizes(a, b, w0, h0) {
        let mut ff = run_ffmpeg_vf(&video, sa, sb, fps, &vf);
        let mut out = ff.stdout.take().unwrap();
        let mut img = ocr::img::Bgr::new(w, h);
        u = None;
        let mut j = 0;
        while out.read_exact(&mut img.data).is_ok() {
            let t = sa + j as f64 / fps;
            j += 1;
            i += 1;
            let near = c.near(w, h);
            let s = near.and_then(|(x, y, rw, rh)| c.track(&reader::cv::crop_bgr(&img, x as usize, y as usize, (x + rw) as usize, (y + rh) as usize), x, y, h, t)).or_else(|| c.find(&img, t));
            if let Some(s) = &s {
                u = Some(s.u);
            }
            let Some(u) = u else { continue };
            let top = s.as_ref().is_some_and(|s| s.top(h));
            let zone = s.as_ref().and_then(|s| s.spawn(w, h));
            let t0 = std::time::Instant::now();
            let (q, born) = if s.as_ref().map_or(true, |s| s.top(h)) { bubbles.find(&img, u, zone) } else { (None, false) };
            spent.push(t0.elapsed().as_secs_f64() * 1000.0);
            if debug {
                println!("{t:.2} top{} view{} {} born{} {}", top as u8, zone.is_some() as u8, q.map_or("-".into(), |q| format!("{q:.2}")), born as u8, s.as_ref().map_or("-".into(), |s| format!("{:.0},{:.0}", s.x, s.y)));
            }
            if mobs.step(t, zone.is_some(), q.is_some(), born) {
                let w = mobs.wave.unwrap();
                println!("{t:.2} 看到出怪 wave {w:.2}");
                waves.push(w);
            }
        }
        let _ = ff.wait();
        }
        spent.sort_by(f64::total_cmp);
        println!("waves {}", waves.iter().map(|w| format!("{w:.1}")).collect::<Vec<_>>().join(" "));
        println!("frames {i} median {:.1} ms max {:.1} ms", spent.get(spent.len() / 2).copied().unwrap_or(0.0), spent.last().copied().unwrap_or(0.0));
    }

    #[test]
    #[ignore]
    fn papu_spawn() {
        use std::io::Read;
        let video = std::env::var("PAPU_VIDEO").unwrap();
        let (a, b) = std::env::var("PAPU_SEG").unwrap().split_once('-').map(|(a, b)| (a.parse::<f64>().unwrap(), b.parse::<f64>().unwrap())).unwrap();
        let fps: f64 = std::env::var("PAPU_FPS").ok().and_then(|v| v.parse().ok()).unwrap_or(4.0);
        let (w, h) = std::env::var("PAPU_SIZE").ok().and_then(|v| v.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((1920usize, 1080usize));
        let mut ff = run_ffmpeg(&video, a, b, fps, w, h);
        let mut out = ff.stdout.take().unwrap();
        let mut c = Clock::new(None);
        let bb = Bubbles::default();
        let mut img = ocr::img::Bgr::new(w, h);
        let mut i = 0;
        while out.read_exact(&mut img.data).is_ok() {
            let t = a + i as f64 / fps;
            i += 1;
            let near = c.near(w, h);
            let Some(s) = near.and_then(|(x, y, rw, rh)| c.track(&reader::cv::crop_bgr(&img, x as usize, y as usize, (x + rw) as usize, (y + rh) as usize), x, y, h, t)).or_else(|| c.find(&img, t)) else { continue };
            if !s.top(h) {
                continue;
            }
            let k = 3.0 * s.u;
            let (sw, sh) = ((w as f32 / k).round() as usize, (h as f32 / k).round() as usize);
            let small = area(&img, sw, sh);
            let fine: [Plane<f32>; 3] = std::array::from_fn(|ch| Plane { w: sw, h: sh, d: small.data.chunks(3).map(|p| p[ch] as f32).collect() });
            let coarse: [Plane<f32>; 3] = std::array::from_fn(|ch| half(&fine[ch]));
            let (top, bottom) = (sh as i32 * 11 / 100, (sh as f32 * UI_TOP / 1080.0) as i32);
            let mut buf = Vec::new();
            let r = bb.coarse.r;
            let mut cands = vec![];
            for y in (top / 2 + r)..(bottom / 2 - r) {
                for x in r..coarse[0].w as i32 - r {
                    if bb.coarse.score(&coarse, x, y, &mut buf) >= BADGE_COARSE {
                        cands.push((x, y));
                    }
                }
            }
            let r = bb.fine.r;
            let mut hits: Vec<(f32, i32, i32)> = vec![];
            for &(cx, cy) in &cands {
                for y in (2 * cy - 2)..=(2 * cy + 3) {
                    for x in (2 * cx - 2)..=(2 * cx + 3) {
                        if y - r >= top && y + r < bottom && x - r >= 0 && x + r < sw as i32 {
                            let q = bb.fine.score(&fine, x, y, &mut buf);
                            if q >= BADGE_Q {
                                hits.push((q, x, y));
                            }
                        }
                    }
                }
            }
            hits.sort_by(|p, q| q.0.total_cmp(&p.0));
            let mut keep: Vec<(f32, i32, i32)> = vec![];
            for hh in hits {
                if keep.iter().all(|k2| (k2.1 - hh.1).abs() > r || (k2.2 - hh.2).abs() > r) {
                    keep.push(hh);
                }
            }
            let pos: Vec<String> = keep.iter().map(|&(q, x, y)| format!("{:.0},{:.0},{q:.2}", (x as f32 * k - s.x) / s.u, (y as f32 * k - s.y) / s.u)).collect();
            println!("{t:.2} u{:.3} {}", s.u, pos.join(" "));
        }
        let _ = ff.wait();
    }

    #[test]
    #[ignore]
    fn papu_alarms() {
        use std::io::Read;
        let video = std::env::var("PAPU_VIDEO").unwrap();
        let (a, b) = std::env::var("PAPU_SEG").unwrap().split_once('-').map(|(a, b)| (a.parse::<f64>().unwrap(), b.parse::<f64>().unwrap())).unwrap();
        let fps: f64 = std::env::var("PAPU_FPS").ok().and_then(|v| v.parse().ok()).unwrap_or(2.0);
        let (w, h) = std::env::var("PAPU_SIZE").ok().and_then(|v| v.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((1920usize, 1080usize));
        let mut ff = run_ffmpeg(&video, a, b, fps, w, h);
        let mut out = ff.stdout.take().unwrap();
        let mut c = Clock::new(None);
        let mut img = ocr::img::Bgr::new(w, h);
        let mut i = 0;
        while out.read_exact(&mut img.data).is_ok() {
            let t = a + i as f64 / fps;
            i += 1;
            let s = c.near(w, h).and_then(|(x, y, rw, rh)| c.track(&reader::cv::crop_bgr(&img, x as usize, y as usize, (x + rw) as usize, (y + rh) as usize), x, y, h, t)).or_else(|| c.find(&img, t));
            let Some(s) = s else { continue };
            let floor = s.y + FLOOR * s.u;
            let (y0, y1) = ((floor - 100.0 * s.u).max(0.0) as usize, (floor - 25.0 * s.u).min(h as f32 - 1.0) as usize);
            if y1 <= y0 + 10 {
                continue;
            }
            let cols: Vec<usize> = (0..w)
                .map(|x| {
                    (y0..y1)
                        .filter(|&y| {
                            let p = img.px(x, y);
                            let (bb, g, r) = (p[0] as i32, p[1] as i32, p[2] as i32);
                            r > 210 && bb > 150 && g < r - 45 && g > 80
                        })
                        .count()
                })
                .collect();
            let need = (12.0 * s.u) as usize;
            let mut found = vec![];
            let mut x = 0;
            while x < w {
                if cols[x] < need {
                    x += 1;
                    continue;
                }
                let x0 = x;
                while x < w && cols[x] >= need / 2 {
                    x += 1;
                }
                let wd = (x - x0) as f32 / s.u;
                if (35.0..140.0).contains(&wd) {
                    found.push(format!("{:.0}:{:.0}", ((x0 + x) as f32 / 2.0 - s.x) / s.u, wd));
                }
            }
            if !found.is_empty() {
                println!("{t:.2} u{:.3} {}", s.u, found.join(" "));
            }
        }
        let _ = ff.wait();
    }

    fn draw(img: &mut ocr::img::Bgr, m: &Marks, bob: i32) {
        let (w, h, px) = arrow_sprite(m.u);
        let top = m.floor + (ARROW_TOP * m.u).round() as i32 + bob;
        for &cx in &m.xs {
            let x0 = m.left + cx - w as i32 / 2;
            for y in 0..h {
                for x in 0..w {
                    let (xx, yy) = (x0 + x as i32, top + y as i32);
                    if xx < 0 || yy < 0 || xx as usize >= img.w || yy as usize >= img.h {
                        continue;
                    }
                    let p = px[y * w + x];
                    let a = (p >> 24) as f32 / 255.0;
                    let i = (yy as usize * img.w + xx as usize) * 3;
                    for k in 0..3 {
                        let c = ((p >> (8 * k)) & 255) as f32;
                        img.data[i + k] = (c + img.data[i + k] as f32 * (1.0 - a)).round().min(255.0) as u8;
                    }
                }
            }
        }
    }

    #[test]
    #[ignore]
    fn papu_preview() {
        let mut img = ocr::img::Bgr::load(std::path::Path::new(&std::env::var("PAPU_IMG").unwrap())).unwrap();
        let mut c = Clock::new(None);
        if std::env::var("PAPU_TRACE").is_ok() {
            println!("calibrate {:?}", c.calibrate(&Clock::prep(&img)));
        }
        let s = (0..8).find_map(|i| c.find(&img, i as f64)).expect("找不到時鐘");
        let (xs, floor) = s.marks(img.w, img.h).expect("地板看不到");
        println!("{s:?} floor {floor:.1} xs {xs:?}");
        let m = Marks { left: 0, floor: floor.round() as i32, w: img.w as i32, xs: xs.iter().map(|v| v.round() as i32).collect(), u: s.u };
        draw(&mut img, &m, 0);
        let out = image::RgbImage::from_fn(img.w as u32, img.h as u32, |x, y| {
            let i = (y as usize * img.w + x as usize) * 3;
            image::Rgb([img.data[i + 2], img.data[i + 1], img.data[i]])
        });
        out.save(std::env::var("PAPU_OUT").unwrap()).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[ignore]
    fn papu_marks_window() {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect, IsWindowVisible};
        crate::platform::marks(Some(Marks { left: 100, floor: 600, w: 800, xs: vec![100, 400, 700], u: 1.0 }));
        std::thread::sleep(std::time::Duration::from_millis(800));
        let cls = crate::platform::wide("ArtaleMarks");
        let h = unsafe { FindWindowW(cls.as_ptr(), std::ptr::null()) };
        assert!(!h.is_null());
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetWindowRect(h, &mut r) };
        println!("visible {} rect {} {} {} {}", unsafe { IsWindowVisible(h) }, r.left, r.top, r.right, r.bottom);
        assert_eq!((r.left, r.right), (100, 900));
        crate::platform::marks(None);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(unsafe { IsWindowVisible(h) }, 0);
    }

    #[test]
    #[ignore]
    fn papu_foot() {
        use std::io::Read;
        let video = std::env::var("PAPU_VIDEO").unwrap();
        let seg = std::env::var("PAPU_SEG").unwrap();
        let (a, b) = seg.split_once('-').unwrap();
        let (a, b): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
        let fps: f64 = std::env::var("PAPU_FPS").ok().and_then(|v| v.parse().ok()).unwrap_or(4.0);
        let (w0, h0) = std::env::var("PAPU_SIZE").ok().and_then(|v| v.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((1920usize, 1080usize));
        let models = std::path::PathBuf::from(std::env::var("BOSS_MODELS").unwrap());
        let mut engine = ocr::Ocr::new(&models.join("onnxruntime.dll"), &models).unwrap();
        let mut f = Foot::default();
        let (mut i, mut reads) = (0, 0);
        let arrows = std::env::var("PAPU_ARROWS").ok();
        let mut clock = Clock::new(None);
        let trace = std::env::var("PAPU_TRACE").is_ok();
        for (sa, sb, w, h, vf) in sizes(a, b, w0, h0) {
        let mut ff = run_ffmpeg_vf(&video, sa, sb, fps, &vf);
        let mut out = ff.stdout.take().unwrap();
        let mut img = ocr::img::Bgr::new(w, h);
        let mut j = 0;
        while out.read_exact(&mut img.data).is_ok() {
            let t = sa + j as f64 / fps;
            j += 1;
            let mut rd = |c: &ocr::img::Bgr| {
                reads += 1;
                let s = engine.run(c, false, false).ok().and_then(|v| v.into_iter().next()).map(|it| it.text).unwrap_or_default();
                if trace {
                    println!("{t:.2} ocr {s}");
                }
                s
            };
            let mut mask = vec![];
            if let Some(a) = arrows.as_deref() {
                let m = clock.find(&img, t).and_then(|s| s.marks(img.w, img.h).map(|(xs, fl)| Marks { left: 0, floor: fl.round() as i32, w: img.w as i32, xs: xs.iter().map(|v| v.round() as i32).collect(), u: s.u }));
                if let Some(m) = &m {
                    draw(&mut img, m, (3.0 * (t * std::f64::consts::TAU / 1.2).sin()).round() as i32);
                }
                if a != "nomask" {
                    mask = m.as_ref().map_or(vec![], |m| m.area(0, 0));
                }
            }
            if f.step(&img, t, &mask, &mut rd) {
                println!("{t:.2} 黑水");
            }
            i += 1;
        }
        let _ = ff.wait();
        }
        println!("frames {i} ocr {reads}");
    }
}
