use reader::cv::Plane;
use std::collections::VecDeque;
use std::io::Write;
use std::time::Instant;

const W: usize = 640;
const H: usize = 360;

struct Tpl {
    w: usize,
    h: usize,
    d: Vec<f32>,
    raw: Vec<f32>,
    x: i32,
    y: i32,
}

impl Tpl {
    fn new(raw: &[u8], w: usize, x: i32, y: i32) -> Tpl {
        Tpl::of(raw.iter().map(|&v| v as f32).collect(), w, x, y)
    }
    fn of(raw: Vec<f32>, w: usize, x: i32, y: i32) -> Tpl {
        let h = raw.len() / w;
        let m = raw.iter().sum::<f32>() / raw.len() as f32;
        let mut d: Vec<f32> = raw.iter().map(|&v| v - m).collect();
        let n = d.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
        d.iter_mut().for_each(|v| *v /= n);
        Tpl { w, h, d, raw, x, y }
    }
    fn half(&self) -> Tpl {
        let g = half(&Plane { w: self.w, h: self.h, d: self.raw.clone() });
        Tpl::of(g.d, g.w, self.x / 2, self.y / 2)
    }
}

fn half(g: &Plane<f32>) -> Plane<f32> {
    let (w, h) = (g.w / 2, g.h / 2);
    let mut o = Plane::new(w, h);
    for y in 0..h {
        for x in 0..w {
            o.d[y * w + x] = (g.d[2 * y * g.w + 2 * x] + g.d[2 * y * g.w + 2 * x + 1] + g.d[(2 * y + 1) * g.w + 2 * x] + g.d[(2 * y + 1) * g.w + 2 * x + 1]) / 4.0;
        }
    }
    o
}

fn resize(g: &Plane<f32>, nw: usize, nh: usize) -> Plane<f32> {
    let mut o = Plane::new(nw, nh);
    let (sx, sy) = (g.w as f32 / nw as f32, g.h as f32 / nh as f32);
    for y in 0..nh {
        let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (g.h - 1) as f32);
        let (y0, wy) = (fy as usize, fy.fract());
        let y1 = (y0 + 1).min(g.h - 1);
        for x in 0..nw {
            let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (g.w - 1) as f32);
            let (x0, wx) = (fx as usize, fx.fract());
            let x1 = (x0 + 1).min(g.w - 1);
            let a = g.d[y0 * g.w + x0] * (1.0 - wx) + g.d[y0 * g.w + x1] * wx;
            let b = g.d[y1 * g.w + x0] * (1.0 - wx) + g.d[y1 * g.w + x1] * wx;
            o.d[y * nw + x] = a * (1.0 - wy) + b * wy;
        }
    }
    o
}

const SCALES: [f32; 36] = [0.60, 0.62, 0.64, 0.66, 0.68, 0.70, 0.72, 0.74, 0.76, 0.78, 0.80, 0.82, 0.84, 0.86, 0.88, 0.90, 0.92, 0.94, 0.96, 0.98, 1.00, 1.02, 1.04, 1.06, 1.08, 1.10, 1.12, 1.14, 1.16, 1.18, 1.20, 1.22, 1.24, 1.26, 1.28, 1.30];

struct Sums {
    s: Vec<f64>,
    s2: Vec<f64>,
}

fn sums(g: &Plane<f32>) -> Sums {
    let (w, h) = (g.w + 1, g.h + 1);
    let mut s = vec![0.0; w * h];
    let mut s2 = vec![0.0; w * h];
    for y in 0..g.h {
        let (mut r, mut r2) = (0.0, 0.0);
        for x in 0..g.w {
            let v = g.d[y * g.w + x] as f64;
            r += v;
            r2 += v * v;
            s[(y + 1) * w + x + 1] = s[y * w + x + 1] + r;
            s2[(y + 1) * w + x + 1] = s2[y * w + x + 1] + r2;
        }
    }
    Sums { s, s2 }
}

pub type Rect = (i32, i32, i32, i32);

fn best(g: &Plane<f32>, ss: &Sums, t: &Tpl, x0: i32, y0: i32, x1: i32, y1: i32, covers: &[Rect]) -> (f32, i32, i32) {
    let x0 = x0.max(0);
    let y0 = y0.max(0);
    let x1 = x1.min(g.w as i32 - t.w as i32);
    let y1 = y1.min(g.h as i32 - t.h as i32);
    let mut b = (-1.0f32, 0, 0);
    let sw = g.w + 1;
    let n = (t.w * t.h) as f64;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if covers.iter().any(|c| x < c.2 && x + t.w as i32 > c.0 && y < c.3 && y + t.h as i32 > c.1) {
                continue;
            }
            let (xu, yu) = (x as usize, y as usize);
            let (a, bb, c, d) = (yu * sw + xu, yu * sw + xu + t.w, (yu + t.h) * sw + xu, (yu + t.h) * sw + xu + t.w);
            let s = ss.s[d] - ss.s[bb] - ss.s[c] + ss.s[a];
            let s2 = ss.s2[d] - ss.s2[bb] - ss.s2[c] + ss.s2[a];
            let var = (s2 - s * s / n).max(1e-6);
            let mut num = 0.0f32;
            for r in 0..t.h {
                let row = &g.d[(yu + r) * g.w + xu..(yu + r) * g.w + xu + t.w];
                let tr = &t.d[r * t.w..(r + 1) * t.w];
                num += row.iter().zip(tr).map(|(a, b)| a * b).sum::<f32>();
            }
            let v = num / var.sqrt() as f32;
            if v > b.0 {
                b = (v, x, y);
            }
        }
    }
    b
}

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub t: f64,
    pub face: Option<(i32, i32, f32)>,
    pub green: f32,
    pub anchor: Option<usize>,
}

fn view(w: usize, h: usize) -> (usize, usize, usize, usize) {
    if w * 9 > h * 16 {
        let cw = h * 16 / 9;
        ((w - cw) / 2, 0, cw, h)
    } else {
        let ch = w * 9 / 16;
        (0, (h - ch) / 2, w, ch)
    }
}

pub fn prep(img: &ocr::img::Bgr) -> (Plane<f32>, f32) {
    let (x0, y0, cw, ch) = view(img.w, img.h);
    let whole = cw == img.w && ch == img.h;
    let small = if whole { area(img, W, H) } else { area(&reader::cv::crop_bgr(img, x0, y0, x0 + cw, y0 + ch), W, H) };
    let g = reader::cv::gray_f32(&small);
    let full = (!whole).then(|| area(img, W, H));
    let small = full.as_ref().unwrap_or(&small);
    let mut n = 0usize;
    let mut tot = 0usize;
    for y in 32..158 {
        for x in 0..W / 2 {
            let mut c = [0u32; 3];
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let p = small.px(x * 2 + dx, y * 2 + dy);
                for k in 0..3 {
                    c[k] += p[k] as u32;
                }
            }
            let (b, gg, r) = (c[0] as i32 / 4, c[1] as i32 / 4, c[2] as i32 / 4);
            tot += 1;
            if gg > 130 && gg > r + 5 && gg > b + 10 && b > 90 {
                n += 1;
            }
        }
    }
    (g, n as f32 / tot.max(1) as f32)
}

fn area(img: &ocr::img::Bgr, nw: usize, nh: usize) -> ocr::img::Bgr {
    let mut o = ocr::img::Bgr::new(nw, nh);
    for y in 0..nh {
        let (sy0, sy1) = (y * img.h / nh, ((y + 1) * img.h / nh).max(y * img.h / nh + 1));
        for x in 0..nw {
            let (sx0, sx1) = (x * img.w / nw, ((x + 1) * img.w / nw).max(x * img.w / nw + 1));
            let mut c = [0u32; 3];
            for sy in sy0..sy1 {
                let row = &img.data[(sy * img.w + sx0) * 3..(sy * img.w + sx1) * 3];
                for p in row.chunks_exact(3) {
                    c[0] += p[0] as u32;
                    c[1] += p[1] as u32;
                    c[2] += p[2] as u32;
                }
            }
            let n = ((sy1 - sy0) * (sx1 - sx0)) as u32;
            let i = (y * nw + x) * 3;
            o.data[i] = ((c[0] + n / 2) / n) as u8;
            o.data[i + 1] = ((c[1] + n / 2) / n) as u8;
            o.data[i + 2] = ((c[2] + n / 2) / n) as u8;
        }
    }
    o
}

#[derive(Clone, Debug)]
struct Cast {
    t: f64,
    end: f64,
    kinds: String,
    cube: Option<bool>,
}

impl Cast {
    fn tilt(&self) -> bool {
        self.kinds.replace('.', "").contains("TT")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Show {
    Wait,
    Left(usize),
    Next,
    Late,
}

impl Show {
    pub fn text(&self) -> String {
        match self {
            Show::Wait => "偵測中，等雙魔方".into(),
            Show::Left(n) => format!("還有 {n} 招"),
            Show::Next => "下一招黑水！".into(),
            Show::Late => "沒出黑水，下一招黑水".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub show: Show,
    pub phase3: bool,
    pub face: bool,
}

impl Line {
    pub fn text(&self) -> Option<String> {
        (self.show != Show::Wait).then(|| self.show.text())
    }
}

pub struct Zakum {
    tpl: Vec<Tpl>,
    last_anchor: Option<(usize, i32, i32, f64)>,
    frames: VecDeque<Frame>,
    casts: Vec<Cast>,
    pub phase3: bool,
    last_seen: Option<Instant>,
    log: Option<std::fs::File>,
    last_show: Option<Show>,
    anchor_id: Option<usize>,
    covers: Vec<Rect>,
    pub opt: Opt,
    scale: Option<f32>,
    calib: (Vec<f32>, usize),
    last_face: f64,
    known_dx: Option<i32>,
    last_full: f64,
    face3: Tpl,
    face_p3: bool,
    face_diff: f32,
}

const CUBE_GREEN: f32 = 0.015;

impl Zakum {
    pub fn new(log: Option<std::path::PathBuf>) -> Zakum {
        Zakum {
            tpl: vec![
                Tpl::new(include_bytes!("zakum/rwall.bin"), 175, 450, 95),
                Tpl::new(include_bytes!("zakum/chain.bin"), 55, 480, 190),
                Tpl::new(include_bytes!("zakum/left.bin"), 140, 60, 104),
                Tpl::new(include_bytes!("zakum/rlow.bin"), 110, 430, 239),
                Tpl::new(include_bytes!("zakum/llow.bin"), 110, 100, 269),
                Tpl::new(include_bytes!("zakum/pillar.bin"), 68, 79, 287),
                Tpl::new(include_bytes!("zakum/tree.bin"), 36, 355, 162),
                Tpl::new(include_bytes!("zakum/face.bin"), 50, 280, 200),
            ],
            last_anchor: None,
            frames: VecDeque::new(),
            casts: vec![],
            phase3: false,
            last_seen: None,
            log: log.and_then(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).ok()),
            last_show: None,
            anchor_id: None,
            covers: vec![],
            opt: Opt::default(),
            scale: None,
            calib: (vec![0.0; SCALES.len()], 0),
            last_face: 0.0,
            known_dx: None,
            last_full: f64::NEG_INFINITY,
            face3: Tpl::new(include_bytes!("zakum/face3.bin"), 50, 282, 182),
            face_p3: false,
            face_diff: 0.0,
        }
    }

    pub fn face_seen(&self) -> bool {
        self.frames.back().map_or(false, |f| f.t - self.last_face <= 3.0)
    }

    pub fn saw(&mut self, phase3: bool) {
        if self.last_seen.map_or(true, |t| t.elapsed().as_secs_f64() > 60.0) {
            self.reset();
            self.logln("開始");
        }
        self.phase3 = phase3;
        self.last_seen = Some(Instant::now());
    }

    pub fn active(&self) -> bool {
        self.last_seen.map_or(false, |t| t.elapsed().as_secs_f64() < 20.0)
    }

    fn reset(&mut self) {
        self.frames.clear();
        self.casts.clear();
        self.last_anchor = None;
        self.last_show = None;
        self.scale = None;
        self.calib = (vec![0.0; SCALES.len()], 0);
        self.known_dx = None;
        self.face_p3 = false;
        self.face_diff = 0.0;
    }

    fn calibrate(&mut self, g: &Plane<f32>, t: f64) {
        let small: Vec<Tpl> = self.tpl.iter().map(|t| t.half()).collect();
        let nf = small.len() - 1;
        let f3 = self.face3.half();
        for (i, &s) in SCALES.iter().enumerate() {
            let r = half(&resize(g, (W as f32 / s).round() as usize, (H as f32 / s).round() as usize));
            let ss = sums(&r);
            let mut sc: Vec<f32> = (0..nf)
                .map(|k| {
                    let tp = &small[k];
                    best(&r, &ss, tp, tp.x - 40, tp.y - 90, tp.x + 40, tp.y + 90, &[]).0
                })
                .collect();
            sc.sort_by(|a, b| b.total_cmp(a));
            let face = [&small[nf], &f3].iter().map(|f| best(&r, &ss, f, f.x - 40, f.y - 90, f.x + 40, f.y + 90, &[]).0).fold(f32::MIN, f32::max);
            self.calib.0[i] += face + sc[0] + sc[1];
        }
        self.calib.1 += 1;
        if self.calib.1 >= 3 {
            let bi = (0..SCALES.len()).max_by(|&a, &b| self.calib.0[a].total_cmp(&self.calib.0[b])).unwrap();
            if self.calib.0[bi] / 3.0 < 2.2 {
                self.calib = (vec![0.0; SCALES.len()], 0);
                return;
            }
            self.scale = Some(SCALES[bi]);
            self.last_anchor = None;
            self.known_dx = None;
            self.last_face = t;
            let line = format!("畫面比例 {:.2}（分數 {:.2}）", SCALES[bi], self.calib.0[bi] / 3.0);
            self.logln(&line);
            self.calib = (vec![0.0; SCALES.len()], 0);
        }
    }

    fn logln(&mut self, s: &str) {
        if let Some(f) = self.log.as_mut() {
            let _ = writeln!(f, "{} {s}", tracker::local_iso().chars().take(23).collect::<String>());
        }
    }

    pub fn look(&mut self, g: &Plane<f32>, t: f64, covers: &[Rect]) -> Option<(i32, i32, f32)> {
        let ss = sums(g);
        let mut anchor = None;
        let nf = self.tpl.len() - 1;
        let last = self.last_anchor.filter(|a| t - a.3 <= 10.0);
        let kdx = self.known_dx;
        let full = t - self.last_full >= 1.0 || last.map_or(false, |a| t - a.3 <= 1.0);
        if full && last.map_or(true, |a| t - a.3 > 1.0) {
            self.last_full = t;
        }
        let find = |k: usize, near: Option<(i32, i32)>| {
            let tp = &self.tpl[k];
            let (cx, cy, rx, ry) = match (near, kdx) {
                (Some((x, y)), _) => (tp.x + x, tp.y + y, 10, 30),
                (None, Some(dx)) => (tp.x + dx, tp.y, 30, 170),
                (None, None) => (tp.x, tp.y, 80, 170),
            };
            let r = best(g, &ss, tp, cx - rx, cy - ry, cx + rx, cy + ry, covers);
            (r.0, r.1 - tp.x, r.2 - tp.y)
        };
        if let Some((k, x, y, _)) = last {
            let r = find(k, Some((x, y)));
            if r.0 >= 0.9 {
                anchor = Some((k, r.1, r.2));
            }
        }
        if anchor.is_none() {
            let mut tries: Vec<Option<(i32, i32)>> = last.map(|a| Some((a.1, a.2))).into_iter().collect();
            if full {
                tries.push(None);
            }
            for near in tries {
                let rs: Vec<(f32, i32, i32)> = (0..nf).map(|k| find(k, near)).collect();
                let top = (0..nf).max_by(|&a, &b| rs[a].0.total_cmp(&rs[b].0)).unwrap();
                if rs[top].0 >= 0.9 {
                    anchor = Some((top, rs[top].1, rs[top].2));
                } else {
                    let mut ok: Vec<usize> = (0..nf).filter(|&k| rs[k].0 >= 0.65).collect();
                    ok.sort_by(|&a, &b| rs[b].0.total_cmp(&rs[a].0));
                    'pair: for (i, &a) in ok.iter().enumerate() {
                        for &b in &ok[i + 1..] {
                            if (rs[a].1 - rs[b].1).abs() <= 1 && (rs[a].2 - rs[b].2).abs() <= 1 {
                                anchor = Some((a, rs[a].1, rs[a].2));
                                break 'pair;
                            }
                        }
                    }
                }
                if anchor.is_some() {
                    break;
                }
            }
        }
        self.anchor_id = anchor.map(|a| a.0);
        if let Some(a) = anchor {
            self.known_dx = Some(a.1);
        }
        let (ax, ay) = match anchor {
            Some((k, x, y)) => {
                self.last_anchor = Some((k, x, y, t));
                (x, y)
            }
            None => match self.last_anchor.filter(|a| t - a.3 <= 3.0) {
                Some((_, x, y, _)) => (x, y),
                None => return None,
            },
        };
        let near = |f: &Tpl| best(g, &ss, f, f.x + ax - 20, f.y + ay - 35, f.x + ax + 20, f.y + ay + 15, covers);
        let (r1, r3) = (near(&self.tpl[nf]), near(&self.face3));
        if r1.0 > 0.6 || r3.0 > 0.6 {
            self.face_diff = self.face_diff * 0.8 + (r3.0 - r1.0) * 0.2;
        }
        if self.face_p3 != (self.face_diff > if self.face_p3 { -0.03 } else { 0.03 }) {
            self.face_p3 = !self.face_p3;
            self.frames.iter_mut().for_each(|f| f.face = None);
            let line = format!("換臉 {}", if self.face_p3 { "破損" } else { "完整" });
            self.logln(&line);
        }
        let (r, f) = if self.face_p3 { (r3, &self.face3) } else { (r1, &self.tpl[nf]) };
        if r.0 < 0.0 {
            return None;
        }
        Some(((r.1 - f.x - ax) * 2, (r.2 - f.y - ay) * 2, r.0))
    }

    pub fn step(&mut self, img: &ocr::img::Bgr, t: f64, covers: &[Rect]) -> Show {
        let (g, green) = prep(img);
        let (x0, y0, cw, ch) = view(img.w, img.h);
        let (kx, ky) = (W as f64 / cw as f64, H as f64 / ch as f64);
        let sc = |c: &Rect| (((c.0 - x0 as i32) as f64 * kx).floor() as i32, ((c.1 - y0 as i32) as f64 * ky).floor() as i32, ((c.2 - x0 as i32) as f64 * kx).ceil() as i32, ((c.3 - y0 as i32) as f64 * ky).ceil() as i32);
        let small: Vec<Rect> = covers.iter().map(sc).collect();
        if small != self.covers {
            let s = format!("畫面 {}x{} 浮窗 {:?}", img.w, img.h, small);
            self.logln(&s);
            self.covers = small.clone();
        }
        if self.scale.is_some() && t - self.last_face > 10.0 {
            self.scale = None;
        }
        let Some(scale) = self.scale else {
            self.calibrate(&g, t);
            return self.push(Frame { t, face: None, green, anchor: None });
        };
        let (g, small) = if (scale - 1.0).abs() < 0.001 {
            (g, small)
        } else {
            let k = 1.0 / scale;
            let g = resize(&g, (W as f32 * k).round() as usize, (H as f32 * k).round() as usize);
            (g, small.iter().map(|c| ((c.0 as f32 * k).floor() as i32, (c.1 as f32 * k).floor() as i32, (c.2 as f32 * k).ceil() as i32, (c.3 as f32 * k).ceil() as i32)).collect())
        };
        let face = self.look(&g, t, &small);
        if face.map_or(false, |f| f.2 >= 0.75) {
            self.last_face = t;
        }
        let anchor = face.and(self.anchor_id);
        self.push(Frame { t, face, green, anchor })
    }

    pub fn push(&mut self, fr: Frame) -> Show {
        let t = fr.t;
        self.frames.push_back(fr);
        while self.frames.front().map_or(false, |f| f.t < t - 30.0) {
            self.frames.pop_front();
        }
        let mut kind = ' ';
        let p3 = self.face_p3;
        if let Some((dx, dy, _)) = fr.face.filter(|q| q.2 >= if p3 { 0.45 } else { 0.75 }) {
            let pick = |same: bool| -> Vec<(i32, i32)> { self.frames.iter().filter(|f| f.t >= t - 15.0 && (!same || f.anchor == fr.anchor)).filter_map(|f| f.face.filter(|q| q.2 >= 0.8).map(|q| (q.0, q.1))).collect() };
            let recent = match pick(true) {
                v if v.len() > 5 => v,
                _ => pick(false),
            };
            if recent.len() > 5 {
                let med = |mut v: Vec<i32>| {
                    v.sort();
                    let n = v.len();
                    if n % 2 == 1 { v[n / 2] as f32 } else { (v[n / 2 - 1] + v[n / 2]) as f32 / 2.0 }
                };
                let nx = med(recent.iter().map(|p| p.0).collect());
                let ny = med(recent.iter().map(|p| p.1).collect());
                let (rx, ry) = (dx as f32 - nx, dy as f32 - ny);
                let (tilt, mid) = if p3 { (-3.0, -10.0) } else { (-8.0, -21.0) };
                kind = if rx >= 5.0 && ry <= tilt {
                    'T'
                } else if rx.abs() <= 3.0 && ry <= -30.0 {
                    'H'
                } else if rx.abs() <= 3.0 && ry <= mid {
                    'M'
                } else {
                    ' '
                };
            }
        }
        if kind != ' ' {
            match self.casts.last_mut() {
                Some(c) if t - c.end <= 2.0 => {
                    c.end = t;
                    c.kinds.push(kind);
                }
                _ => self.casts.push(Cast { t, end: t, kinds: kind.to_string(), cube: None }),
            }
        } else if let Some(c) = self.casts.last_mut().filter(|c| t - c.end <= 2.0 && !c.kinds.ends_with('.')) {
            c.kinds.push('.');
        }
        self.casts.retain(|c| c.t >= t - 180.0);
        if self.opt.p3green && self.phase3 {
            let n = self.frames.iter().rev().take(4).filter(|f| f.green >= CUBE_GREEN).count();
            let near = self.casts.iter().any(|c| (c.cube != Some(false)) && c.tilt() && (c.t - t).abs() <= 8.0 || c.cube == Some(true) && t - c.t < 20.0);
            if n >= 2 && !near {
                let t0 = self.frames.iter().rev().take(4).filter(|f| f.green >= CUBE_GREEN).map(|f| f.t).fold(t, f64::min);
                let at = self.casts.partition_point(|c| c.t <= t0);
                self.casts.insert(at, Cast { t: t0, end: t0, kinds: "G".into(), cube: Some(true) });
                let s = format!("綠色魔方 {t0:.1}");
                self.logln(&s);
            }
        }
        for i in 0..self.casts.len() {
            if self.casts[i].cube.is_none() && self.casts[i].tilt() {
                let c = self.cube(self.casts[i].t, t);
                self.casts[i].cube = c;
            }
        }
        let show = self.judge(t);
        let line = match fr.face {
            Some((dx, dy, q)) => format!("{t:.1} {dx} {dy} {q:.2} g{:.3} {kind} {:?} a{}", fr.green, show, self.anchor_id.map_or("-".into(), |a| a.to_string())),
            None => format!("{t:.1} - g{:.3} {:?}", fr.green, show),
        };
        self.logln(&line);
        if self.last_show.as_ref() != Some(&show) {
            let s = format!("顯示 {}", show.text());
            self.logln(&s);
            self.last_show = Some(show.clone());
        }
        show
    }

    fn cube(&self, ct: f64, now: f64) -> Option<bool> {
        let w = self.opt.cube_win;
        let n = self.frames.iter().filter(|f| f.t >= ct - 1.0 && f.t <= ct + w && f.green >= CUBE_GREEN).count();
        if n >= 3 {
            Some(true)
        } else if now >= ct + w {
            Some(false)
        } else {
            None
        }
    }

    fn judge(&self, now: f64) -> Show {
        let Some(ci) = self.casts.iter().rposition(|c| c.cube == Some(true)) else { return Show::Wait };
        let tc = self.casts[ci].t;
        if now - tc > 120.0 {
            return Show::Wait;
        }
        let mut n = 0;
        let gap = self.interval();
        let mut prev = tc;
        for (i, c) in self.casts[ci + 1..].iter().enumerate() {
            if c.t < tc + 2.5 {
                continue;
            }
            if c.kinds.contains('T') && !c.tilt() && now - c.end < 1.0 {
                continue;
            }
            if c.tilt() {
                match c.cube {
                    Some(false) => return Show::Wait,
                    _ => {
                        prev = prev.max(c.t);
                        continue;
                    }
                }
            }
            if self.opt.fill && i > 0 && prev > tc && c.t - prev < gap * 0.45 {
                continue;
            }
            if self.opt.fill {
                n += missed(c.t - prev, gap);
            }
            n += 1;
            prev = c.t;
        }
        if self.opt.fill {
            n += missed(now - prev, gap);
        }
        let need: usize = if self.phase3 { 5 } else { 4 };
        let stretch = if self.opt.stretch { (gap / 9.0).clamp(1.0, 1.6) } else { 1.0 };
        if self.opt.timer && n < need && now - tc >= if self.phase3 { 36.0 } else { 37.0 } * stretch {
            n = need;
        }
        if n < need {
            Show::Left(need - n)
        } else if n == need {
            Show::Next
        } else if n == need + 1 {
            Show::Late
        } else {
            Show::Wait
        }
    }

    fn interval(&self) -> f64 {
        let mut v: Vec<f64> = self.casts.windows(2).map(|w| w[1].t - w[0].t).filter(|d| (5.5..=14.0).contains(d)).collect();
        if v.len() < 3 {
            return 9.0;
        }
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    }
}

fn missed(d: f64, gap: f64) -> usize {
    if d < gap * 1.6 {
        return 0;
    }
    (((d / gap) + 0.4).floor() as usize).saturating_sub(1).min(2)
}

#[derive(Clone, Copy, Debug)]
pub struct Opt {
    pub fill: bool,
    pub timer: bool,
    pub p3green: bool,
    pub stretch: bool,
    pub cube_win: f64,
}

impl Default for Opt {
    fn default() -> Self {
        Opt { fill: true, timer: true, p3green: false, stretch: false, cube_win: 3.5 }
    }
}

pub fn zakum_phase(name: &str, hp: i64, pct: u32, prev: bool) -> Option<bool> {
    const HPS: [f64; 3] = [66_000_000.0, 68_000_000.0, 70_000_000.0];
    let hp = hp as f64 * 100.0;
    let (lo, hi) = (hp / (pct + 1) as f64 * 0.98, if pct == 0 { f64::INFINITY } else { hp / pct as f64 * 1.02 });
    let fit: Vec<usize> = (0..3).filter(|&i| HPS[i] >= lo && HPS[i] <= hi).collect();
    if !name.contains("炎魔") && (fit.is_empty() || pct < 10) {
        return None;
    }
    Some(match (fit.contains(&2), fit.iter().any(|&i| i < 2)) {
        (true, false) => true,
        (false, true) => false,
        _ => prev,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_text() {
        let l = |show, face| Line { show, phase3: false, face };
        assert_eq!(l(Show::Wait, true).text(), None);
        assert_eq!(l(Show::Left(2), false).text().as_deref(), Some("還有 2 招"));
        assert_eq!(l(Show::Late, true).text().as_deref(), Some("沒出黑水，下一招黑水"));
    }

    #[test]
    #[ignore]
    fn zakum_video() {
        use std::io::Read;
        let video = std::env::var("ZAKUM_VIDEO").unwrap();
        let seg = std::env::var("ZAKUM_SEG").unwrap();
        let (a, b) = seg.split_once('-').unwrap();
        let (a, b): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
        let p3from: f64 = std::env::var("ZAKUM_P3FROM").ok().and_then(|v| v.parse().ok()).unwrap_or(f64::INFINITY);
        let (w, h, fps) = (1280usize, 720usize, 4.0);
        let mut cmd = std::process::Command::new("ffmpeg");
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
        let mut ff = cmd
            .args(["-v", "error", "-ss", &a.to_string(), "-t", &(b - a).to_string(), "-i", &video, "-vf", &format!("fps={fps},scale={w}:{h}"), "-f", "rawvideo", "-pix_fmt", "bgr24", "-"])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut out = ff.stdout.take().unwrap();
        let mut z = Zakum::new(std::env::var("ZAKUM_LOG").ok().map(Into::into));
        z.opt.fill = std::env::var("ZAKUM_NOFILL").is_err();
        z.opt.timer = std::env::var("ZAKUM_NOTIMER").is_err();
        z.opt.stretch = std::env::var("ZAKUM_STRETCH").is_ok();
        if let Some(w) = std::env::var("ZAKUM_CUBEWIN").ok().and_then(|v| v.parse().ok()) { z.opt.cube_win = w; }
        z.opt.p3green = std::env::var("ZAKUM_P3G").is_ok();
        z.logln(&format!("測試 {seg} 第三階段從 {p3from}"));
        let mut img = ocr::img::Bgr::new(w, h);
        let mut i = 0;
        let mut miss = 0;
        while out.read_exact(&mut img.data).is_ok() {
            let t = a + i as f64 / fps;
            z.phase3 = t >= p3from;
            z.step(&img, t, &[]);
            miss += z.frames.back().unwrap().face.is_none() as usize;
            i += 1;
        }
        let _ = ff.wait();
        println!("seg {seg} frames {i} no face {miss}");
    }

    #[test]
    #[ignore]
    fn zakum_live() {
        use std::io::Read;
        struct Fake<'a>(&'a ocr::img::Bgr);
        impl reader::Screen for Fake<'_> {
            fn grab(&mut self, x: i32, y: i32, w: i32, h: i32) -> Option<ocr::img::Bgr> {
                let img = self.0;
                let (x1, y1) = ((x + w) as usize, (y + h) as usize);
                (x >= 0 && y >= 0 && x1 <= img.w && y1 <= img.h).then(|| reader::cv::crop_bgr(img, x as usize, y as usize, x1, y1))
            }
        }
        let video = std::env::var("ZAKUM_VIDEO").unwrap();
        let seg = std::env::var("ZAKUM_SEG").unwrap();
        let (a, b) = seg.split_once('-').unwrap();
        let (a, b): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
        let models = std::path::PathBuf::from(std::env::var("BOSS_MODELS").unwrap());
        let mut engine = ocr::Ocr::new(&models.join("onnxruntime.dll"), &models).unwrap();
        let (w, h) = std::env::var("ZAKUM_SIZE").ok().and_then(|v| v.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((1280usize, 720usize));
        let fps = 4.0;
        let mut cmd = std::process::Command::new("ffmpeg");
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
        let mut ff = cmd
            .args(["-v", "error", "-ss", &a.to_string(), "-t", &(b - a).to_string(), "-i", &video, "-vf", &format!("fps={fps},scale={w}:{h}"), "-f", "rawvideo", "-pix_fmt", "bgr24", "-"])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut out = ff.stdout.take().unwrap();
        let mut z = Zakum::new(std::env::var("ZAKUM_LOG").ok().map(Into::into));
        z.opt.fill = std::env::var("ZAKUM_NOFILL").is_err();
        z.opt.timer = std::env::var("ZAKUM_NOTIMER").is_err();
        z.opt.stretch = std::env::var("ZAKUM_STRETCH").is_ok();
        if let Some(w) = std::env::var("ZAKUM_CUBEWIN").ok().and_then(|v| v.parse().ok()) { z.opt.cube_win = w; }
        z.opt.p3green = std::env::var("ZAKUM_P3G").is_ok();
        let mut cfg = serde_json::Map::new();
        let mut watcher = crate::boss::Watcher::default();
        let mut img = ocr::img::Bgr::new(w, h);
        let (mut i, mut last_bar, mut seen, mut on) = (0usize, f64::NEG_INFINITY, f64::NEG_INFINITY, false);
        let mut p3 = false;
        while out.read_exact(&mut img.data).is_ok() {
            let t = a + i as f64 / fps;
            i += 1;
            if t - last_bar >= 3.0 {
                last_bar = t;
                if !cfg.contains_key("boss_region") {
                    watcher = crate::boss::Watcher::default();
                }
                let r = match watcher.tick_in(&cfg, &mut engine, &mut Fake(&img), (0, 0, w as i32, h as i32), true) {
                    Some(crate::boss::Out::Found(reg, r)) => {
                        cfg.insert("boss_region".into(), serde_json::json!(reg));
                        Some(r)
                    }
                    Some(crate::boss::Out::Read(r)) => r,
                    None => None,
                };
                if let Some(r) = r {
                    if let Some(ph) = zakum_phase(&r.name, r.hp, r.pct, p3) {
                        if t - seen > 60.0 {
                            z.reset();
                            z.logln(&format!("開始 {t:.1}"));
                        }
                        if ph != p3 || !on {
                            z.logln(&format!("血條 {} {} ({}%) 第三階段 {ph}", r.name, r.hp, r.pct));
                        }
                        seen = t;
                        p3 = ph;
                    }
                }
            }
            on = t - seen < 20.0;
            if on {
                z.phase3 = p3;
                z.step(&img, t, &[]);
            }
        }
        let _ = ff.wait();
        println!("seg {seg} frames {i}");
    }

    #[test]
    #[ignore]
    fn zakum_probe() {
        let im = image::open(std::env::var("ZAKUM_IMG").unwrap()).unwrap().to_rgb8();
        let mut img = ocr::img::Bgr::new(im.width() as usize, im.height() as usize);
        for (i, p) in im.pixels().enumerate() {
            img.data[i * 3..i * 3 + 3].copy_from_slice(&[p[2], p[1], p[0]]);
        }
        let (g0, _) = prep(&img);
        let z = Zakum::new(None);
        if let (Ok(out), Ok(s)) = (std::env::var("ZAKUM_DUMP"), std::env::var("ZAKUM_S")) {
            let s: f32 = s.parse().unwrap();
            let g = resize(&g0, (W as f32 / s).round() as usize, (H as f32 / s).round() as usize);
            let mut b = format!("P5 {} {} 255
", g.w, g.h).into_bytes();
            b.extend(g.d.iter().map(|&v| v.round().clamp(0.0, 255.0) as u8));
            std::fs::write(out, b).unwrap();
        }
        for s in [0.9f32, 0.94, 0.96, 1.0, 1.06, 1.1, 1.12, 1.14, 1.18, 1.22] {
            let g = resize(&g0, (W as f32 / s).round() as usize, (H as f32 / s).round() as usize);
            let ss = sums(&g);
            let r: Vec<String> = z.tpl.iter().map(|tp| {
                let r = best(&g, &ss, tp, -300, -300, 900, 600, &[]);
                format!("{:.2}@{},{}", r.0, r.1 - tp.x, r.2 - tp.y)
            }).collect();
            let f3 = best(&g, &ss, &z.face3, -300, -300, 900, 600, &[]);
            println!("{s} {} f3 {:.2}@{},{}", r.join(" "), f3.0, f3.1 - z.face3.x, f3.2 - z.face3.y);
        }
    }

    #[test]
    #[ignore]
    fn zakum_cost() {
        let g = ocr::capture::Grabber::new().unwrap();
        let (x, y, w, h) = crate::platform::primary_screen();
        let mut z = Zakum::new(None);
        let (mut grab, mut judge) = (vec![], vec![]);
        for i in 0..40 {
            let t0 = Instant::now();
            let img = g.grab(x, y, w, h).unwrap();
            let t1 = Instant::now();
            z.step(&img, i as f64 * 0.25, &[]);
            grab.push((t1 - t0).as_secs_f64() * 1000.0);
            judge.push(t1.elapsed().as_secs_f64() * 1000.0);
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let avg = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        println!("screen {w}x{h} grab {:.1} ms judge {:.1} ms (max {:.1} / {:.1})", avg(&grab), avg(&judge), grab.iter().cloned().fold(0.0, f64::max), judge.iter().cloned().fold(0.0, f64::max));
    }

    fn judged(phase3: bool, casts: &[(f64, &str, Option<bool>)], now: f64) -> Show {
        let mut z = Zakum::new(None);
        z.phase3 = phase3;
        z.casts = casts.iter().map(|&(t, k, cube)| Cast { t, end: t + 0.5, kinds: k.into(), cube }).collect();
        z.judge(now)
    }

    #[test]
    fn count_casts() {
        let cube = (0.0, "TT", Some(true));
        let c = |t: f64| (t, "H", None);
        assert_eq!(judged(false, &[cube], 3.0), Show::Left(4));
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0), c(25.0)], 27.0), Show::Left(1));
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0), c(25.0), c(34.0)], 36.0), Show::Next);
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0), c(25.0), c(34.0), c(43.0)], 45.0), Show::Late);
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0), c(25.0), c(34.0), c(43.0), c(52.0)], 54.0), Show::Wait);
        assert_eq!(judged(true, &[cube, c(7.0), c(16.0), c(25.0), c(34.0)], 35.0), Show::Left(1));
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0), c(25.0), c(34.0), (43.0, "TT", Some(false))], 50.0), Show::Wait);
        assert_eq!(judged(false, &[cube, (1.5, "M", None)], 3.0), Show::Left(4));
    }

    #[test]
    fn missed_and_split() {
        let cube = (0.0, "TT", Some(true));
        let c = |t: f64| (t, "H", None);
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0), c(34.0)], 36.0), Show::Next);
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0), c(25.0)], 41.0), Show::Next);
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0)], 36.0), Show::Left(1));
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0)], 37.5), Show::Next);
        let slow = [(-36.0, "H", None), (-24.0, "H", None), (-12.0, "H", None), cube, c(12.0), c(24.0)];
        assert_eq!(judged(false, &slow, 36.0), Show::Left(2));
        assert_eq!(judged(false, &slow, 37.5), Show::Next);
        assert_eq!(judged(false, &[cube, c(7.0), c(16.0), c(19.0), c(25.0)], 27.0), Show::Left(1));
    }

    #[test]
    #[ignore]
    fn zakum_frames() {
        let dir = std::path::PathBuf::from(std::env::var("ZAKUM_FRAMES").unwrap());
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |e| e == "png" || e == "jpg")).collect();
        files.sort();
        let mut z = Zakum::new(std::env::var("ZAKUM_LOG").ok().map(Into::into));
        z.phase3 = std::env::var("ZAKUM_P3").is_ok();
        z.opt.fill = std::env::var("ZAKUM_NOFILL").is_err();
        z.opt.timer = std::env::var("ZAKUM_NOTIMER").is_err();
        z.opt.stretch = std::env::var("ZAKUM_STRETCH").is_ok();
        if let Some(w) = std::env::var("ZAKUM_CUBEWIN").ok().and_then(|v| v.parse().ok()) { z.opt.cube_win = w; }
        z.opt.p3green = std::env::var("ZAKUM_P3G").is_ok();
        let covers: Vec<Rect> = std::env::var("ZAKUM_COVER").ok().filter(|v| !v.is_empty()).map(|v| {
            let n: Vec<i32> = v.split(',').map(|x| x.parse().unwrap()).collect();
            vec![(n[0], n[1], n[2], n[3])]
        }).unwrap_or_default();
        let mut last = None;
        let mut miss = 0;
        let mut spent = vec![];
        for f in &files {
            let stem = f.file_stem().unwrap().to_string_lossy().to_string();
            let t: f64 = stem.parse::<f64>().unwrap() / if stem.len() >= 6 { 100.0 } else { 10.0 };
            let img = ocr::img::Bgr::load(f).unwrap();
            let t0 = Instant::now();
            let s = z.step(&img, t, &covers);
            spent.push(t0.elapsed().as_secs_f64() * 1000.0);
            let face = z.frames.back().unwrap().face;
            miss += face.is_none() as usize;
            if std::env::var("ZAKUM_TRACE").is_ok() {
                println!("{t:.1} {face:?} a{:?} {:?}", z.anchor_id, s);
            }
            if last.as_ref() != Some(&s) {
                println!("{t:7.1} {}", s.text());
                last = Some(s);
            }
        }
        println!("frames {} no face {miss} judge {:.1} ms/frame (worst {:.1})", files.len(), spent.iter().sum::<f64>() / spent.len().max(1) as f64, spent.iter().cloned().fold(0.0, f64::max));
        for c in &z.casts {
            println!("cast {:.1} {}", c.t, c.kinds);
        }
    }
}
