use ocr::img::Bgr;
use rustfft::{num_complex::Complex, FftPlanner};

#[derive(Clone, Debug)]
pub struct Plane<T> {
    pub w: usize,
    pub h: usize,
    pub d: Vec<T>,
}

impl<T: Copy + Default> Plane<T> {
    pub fn new(w: usize, h: usize) -> Self {
        Plane { w, h, d: vec![T::default(); w * h] }
    }
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> T {
        self.d[y * self.w + x]
    }
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: T) {
        self.d[y * self.w + x] = v;
    }
    pub fn crop(&self, x0: usize, y0: usize, x1: usize, y1: usize) -> Self {
        let mut o = Plane::new(x1 - x0, y1 - y0);
        for y in y0..y1 {
            o.d[(y - y0) * o.w..(y - y0 + 1) * o.w].copy_from_slice(&self.d[y * self.w + x0..y * self.w + x1]);
        }
        o
    }
}

pub fn crop_bgr(img: &Bgr, x0: usize, y0: usize, x1: usize, y1: usize) -> Bgr {
    let mut o = Bgr::new(x1 - x0, y1 - y0);
    for y in y0..y1 {
        o.data[(y - y0) * o.w * 3..(y - y0 + 1) * o.w * 3].copy_from_slice(&img.data[(y * img.w + x0) * 3..(y * img.w + x1) * 3]);
    }
    o
}

pub fn gray(img: &Bgr) -> Plane<u8> {
    let mut o = Plane::new(img.w, img.h);
    for (d, p) in o.d.iter_mut().zip(img.data.chunks_exact(3)) {
        *d = ((p[0] as u32 * 1868 + p[1] as u32 * 9617 + p[2] as u32 * 4899 + (1 << 13)) >> 14) as u8;
    }
    o
}

pub fn gray_f32(img: &Bgr) -> Plane<f32> {
    let g = gray(img);
    Plane { w: g.w, h: g.h, d: g.d.iter().map(|&v| v as f32).collect() }
}

pub fn digit_fill(img: &Bgr) -> Plane<u8> {
    let mut o = Plane::new(img.w, img.h);
    for (d, p) in o.d.iter_mut().zip(img.data.chunks_exact(3)) {
        let mx = p[0].max(p[1]).max(p[2]) as i16;
        let mn = p[0].min(p[1]).min(p[2]) as i16;
        *d = (mn >= 185 && mx - mn <= 10) as u8;
    }
    o
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comp {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub area: usize,
}

pub fn components(m: &Plane<u8>, conn8: bool) -> (Plane<u32>, Vec<Comp>) {
    let (w, h) = (m.w, m.h);
    let mut parent: Vec<usize> = vec![];
    let mut lab = vec![usize::MAX; w * h];
    fn find(p: &mut Vec<usize>, mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for y in 0..h {
        for x in 0..w {
            if m.d[y * w + x] == 0 {
                continue;
            }
            let mut nb: Vec<usize> = vec![];
            if x > 0 && m.d[y * w + x - 1] != 0 {
                nb.push(lab[y * w + x - 1]);
            }
            if y > 0 {
                if m.d[(y - 1) * w + x] != 0 {
                    nb.push(lab[(y - 1) * w + x]);
                }
                if conn8 {
                    if x > 0 && m.d[(y - 1) * w + x - 1] != 0 {
                        nb.push(lab[(y - 1) * w + x - 1]);
                    }
                    if x + 1 < w && m.d[(y - 1) * w + x + 1] != 0 {
                        nb.push(lab[(y - 1) * w + x + 1]);
                    }
                }
            }
            if nb.is_empty() {
                parent.push(parent.len());
                lab[y * w + x] = parent.len() - 1;
            } else {
                let r = nb.iter().map(|&l| find(&mut parent, l)).min().unwrap();
                for &l in &nb {
                    let rl = find(&mut parent, l);
                    parent[rl] = r;
                }
                lab[y * w + x] = r;
            }
        }
    }
    let n = parent.len();
    let mut key = vec![(usize::MAX, usize::MAX); n];
    let mut bb = vec![(usize::MAX, usize::MAX, 0usize, 0usize, 0usize); n];
    for y in 0..h {
        for x in 0..w {
            let l = lab[y * w + x];
            if l == usize::MAX {
                continue;
            }
            let r = find(&mut parent, l);
            let k = if conn8 { (y / 2, x / 2) } else { (y, x) };
            if k < key[r] {
                key[r] = k;
            }
            let b = &mut bb[r];
            b.0 = b.0.min(x);
            b.1 = b.1.min(y);
            b.2 = b.2.max(x);
            b.3 = b.3.max(y);
            b.4 += 1;
        }
    }
    let mut roots: Vec<usize> = (0..n).filter(|&i| find(&mut parent, i) == i).collect();
    roots.sort_by_key(|&r| key[r]);
    let mut newid = vec![0u32; n];
    for (i, &r) in roots.iter().enumerate() {
        newid[r] = i as u32 + 1;
    }
    let mut out = Plane::<u32>::new(w, h);
    for i in 0..w * h {
        if lab[i] != usize::MAX {
            let r = find(&mut parent, lab[i]);
            out.d[i] = newid[r];
        }
    }
    let comps = roots
        .iter()
        .map(|&r| {
            let b = bb[r];
            Comp { x: b.0, y: b.1, w: b.2 - b.0 + 1, h: b.3 - b.1 + 1, area: b.4 }
        })
        .collect();
    (out, comps)
}

pub fn dilate(m: &Plane<u8>, r: usize) -> Plane<u8> {
    morph(m, r, true)
}

pub fn erode(m: &Plane<u8>, r: usize) -> Plane<u8> {
    morph(m, r, false)
}

fn morph(m: &Plane<u8>, r: usize, max: bool) -> Plane<u8> {
    let (w, h) = (m.w, m.h);
    let mut t = Plane::<u8>::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            let row = &m.d[y * w + a..=y * w + b];
            t.d[y * w + x] = if max { *row.iter().max().unwrap() } else { *row.iter().min().unwrap() };
        }
    }
    let mut o = Plane::<u8>::new(w, h);
    for y in 0..h {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            let mut v = t.d[a * w + x];
            for yy in a..=b {
                let u = t.d[yy * w + x];
                v = if max { v.max(u) } else { v.min(u) };
            }
            o.d[y * w + x] = v;
        }
    }
    o
}

pub fn resize_area(src: &Plane<f32>, nw: usize, nh: usize) -> Plane<f32> {
    let (sw, sh) = (src.w, src.h);
    let (sx, sy) = (sw as f64 / nw as f64, sh as f64 / nh as f64);
    if sx >= 1.0 && sy >= 1.0 {
        let tx = area_tab(sw, nw, sx);
        let ty = area_tab(sh, nh, sy);
        let mut o = Plane::<f32>::new(nw, nh);
        for (dy, wy) in ty.iter().enumerate() {
            for (dx, wx) in tx.iter().enumerate() {
                let mut s = 0f64;
                for &(yy, by) in wy {
                    let mut r = 0f64;
                    for &(xx, ax) in wx {
                        r += src.at(xx, yy) as f64 * ax as f64;
                    }
                    s += r * by as f64;
                }
                o.set(dx, dy, s as f32);
            }
        }
        return o;
    }
    let coef = |src_n: usize, dst_n: usize, scale: f64| -> Vec<(usize, usize, f32, f32)> {
        let inv = dst_n as f64 / src_n as f64;
        (0..dst_n)
            .map(|d| {
                let s = (d as f64 * scale).floor() as i64;
                let mut f = ((d as f64 + 1.0) - (s as f64 + 1.0) * inv) as f32;
                f = if f <= 0.0 { 0.0 } else { f - f.floor() };
                let last = src_n as i64 - 1;
                let (s0, f) = if s >= last { (last, 0.0) } else { (s.max(0), f) };
                (s0 as usize, (s0 + 1).min(last) as usize, 1.0 - f, f)
            })
            .collect()
    };
    let cx = coef(sw, nw, sx);
    let cy = coef(sh, nh, sy);
    let mut o = Plane::<f32>::new(nw, nh);
    for (dy, &(y0, y1, b0, b1)) in cy.iter().enumerate() {
        for (dx, &(x0, x1, a0, a1)) in cx.iter().enumerate() {
            let r0 = src.at(x0, y0) * a0 + src.at(x1, y0) * a1;
            let r1 = src.at(x0, y1) * a0 + src.at(x1, y1) * a1;
            o.set(dx, dy, r0 * b0 + r1 * b1);
        }
    }
    o
}

fn area_tab(src: usize, dst: usize, scale: f64) -> Vec<Vec<(usize, f32)>> {
    (0..dst)
        .map(|dx| {
            let fsx1 = dx as f64 * scale;
            let fsx2 = fsx1 + scale;
            let cell = scale.min(src as f64 - fsx1);
            let mut sx1 = fsx1.ceil() as i64;
            let mut sx2 = fsx2.floor() as i64;
            sx2 = sx2.min(src as i64 - 1);
            sx1 = sx1.min(sx2);
            let mut v = vec![];
            if sx1 as f64 - fsx1 > 1e-3 {
                v.push(((sx1 - 1) as usize, ((sx1 as f64 - fsx1) / cell) as f32));
            }
            for s in sx1..sx2 {
                v.push((s as usize, (1.0 / cell) as f32));
            }
            if fsx2 - sx2 as f64 > 1e-3 {
                v.push((sx2 as usize, ((fsx2 - sx2 as f64).min(1.0).min(cell) / cell) as f32));
            }
            v
        })
        .collect()
}

pub fn match_masked(img: &Plane<f32>, t: &Plane<f32>, m: &Plane<f32>) -> Plane<f32> {
    let (rw, rh) = (img.w + 1 - t.w, img.h + 1 - t.h);
    let msum: f64 = m.d.iter().map(|&v| v as f64).sum();
    let mt: f64 = m.d.iter().zip(&t.d).map(|(&a, &b)| a as f64 * b as f64).sum();
    let mean_t = mt / msum;
    let pts: Vec<(usize, usize, f64)> = (0..t.h)
        .flat_map(|y| (0..t.w).map(move |x| (x, y)))
        .filter(|&(x, y)| m.at(x, y) != 0.0)
        .map(|(x, y)| (x, y, t.at(x, y) as f64 - mean_t))
        .collect();
    let norm_t: f64 = pts.iter().map(|p| p.2 * p.2).sum::<f64>().sqrt();
    let n = pts.len() as f64;
    let mut o = Plane::<f32>::new(rw, rh);
    for y in 0..rh {
        for x in 0..rw {
            let (mut cross, mut si, mut sii) = (0f64, 0f64, 0f64);
            for &(px, py, tv) in &pts {
                let v = img.at(x + px, y + py) as f64;
                cross += v * tv;
                si += v;
                sii += v * v;
            }
            let var = sii - si * si / n;
            let den = norm_t * var.max(0.0).sqrt();
            let r = cross / den;
            o.set(x, y, if r.is_finite() { r as f32 } else { -1.0 });
        }
    }
    o
}

pub fn max_loc(r: &Plane<f32>) -> (f32, usize, usize) {
    let (mut best, mut bx, mut by) = (f32::MIN, 0, 0);
    for y in 0..r.h {
        for x in 0..r.w {
            let v = r.at(x, y);
            if v > best {
                best = v;
                bx = x;
                by = y;
            }
        }
    }
    (best, bx, by)
}

pub fn gaussian(img: &Bgr, sigma: f64) -> Bgr {
    let n = (((sigma * 6.0 + 1.0).round_ties_even()) as usize) | 1;
    let half = (n - 1) / 2;
    let mut k: Vec<f64> = (0..n).map(|i| {
        let x = i as f64 - half as f64;
        (-0.5 * x * x / (sigma * sigma)).exp()
    }).collect();
    let s: f64 = k.iter().sum();
    for v in k.iter_mut() {
        *v /= s;
    }
    let kf: Vec<i64> = k.iter().map(|v| (v * 256.0).round_ties_even() as i64).collect();
    let refl = |i: i64, n: i64| -> usize {
        if n == 1 {
            return 0;
        }
        let mut i = i;
        loop {
            if i < 0 {
                i = -i;
            } else if i >= n {
                i = 2 * n - 2 - i;
            } else {
                return i as usize;
            }
        }
    };
    let (w, h) = (img.w, img.h);
    let mut tmp = vec![0i64; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut acc = 0i64;
                for (i, &kv) in kf.iter().enumerate() {
                    let xx = refl(x as i64 + i as i64 - half as i64, w as i64);
                    acc += img.data[(y * w + xx) * 3 + c] as i64 * kv;
                }
                tmp[(y * w + x) * 3 + c] = acc;
            }
        }
    }
    let mut o = Bgr::new(w, h);
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut acc = 0i64;
                for (i, &kv) in kf.iter().enumerate() {
                    let yy = refl(y as i64 + i as i64 - half as i64, h as i64);
                    acc += tmp[(yy * w + x) * 3 + c] * kv;
                }
                o.data[(y * w + x) * 3 + c] = ((acc + (1 << 15)) >> 16).clamp(0, 255) as u8;
            }
        }
    }
    o
}

pub fn hsv(img: &Bgr) -> Vec<[u8; 3]> {
    const SHIFT: i32 = 12;
    let sdiv = |v: i32| if v == 0 { 0 } else { ((255i64 << SHIFT) as f64 / v as f64).round_ties_even() as i32 };
    let hdiv = |d: i32| if d == 0 { 0 } else { ((180i64 << SHIFT) as f64 / (6.0 * d as f64)).round_ties_even() as i32 };
    img.data
        .chunks_exact(3)
        .map(|p| {
            let (b, g, r) = (p[0] as i32, p[1] as i32, p[2] as i32);
            let v = b.max(g).max(r);
            let vmin = b.min(g).min(r);
            let diff = v - vmin;
            let vr = if v == r { -1 } else { 0 };
            let vg = if v == g { -1 } else { 0 };
            let s = (diff * sdiv(v) + (1 << (SHIFT - 1))) >> SHIFT;
            let mut hh = (vr & (g - b)) + (!vr & ((vg & (b - r + 2 * diff)) + ((!vg) & (r - g + 4 * diff))));
            hh = (hh * hdiv(diff) + (1 << (SHIFT - 1))) >> SHIFT;
            if hh < 0 {
                hh += 180;
            }
            [hh as u8, s as u8, v as u8]
        })
        .collect()
}

pub fn lab(img: &Bgr) -> Vec<[f32; 3]> {
    let lin = |c: u8| {
        let v = c as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    img.data
        .chunks_exact(3)
        .map(|p| {
            let (b, g, r) = (lin(p[0]), lin(p[1]), lin(p[2]));
            let x = (0.412453 * r + 0.357580 * g + 0.180423 * b) / 0.950456;
            let y = 0.212671 * r + 0.715160 * g + 0.072169 * b;
            let z = (0.019334 * r + 0.119193 * g + 0.950227 * b) / 1.088754;
            let f = |t: f64| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
            let l = if y > 0.008856 { 116.0 * y.cbrt() - 16.0 } else { 903.3 * y };
            let a = 500.0 * (f(x) - f(y));
            let bb = 200.0 * (f(y) - f(z));
            [(l * 255.0 / 100.0).round() as f32, (a + 128.0).round() as f32, (bb + 128.0).round() as f32]
        })
        .collect()
}

pub fn match_color(img: &Bgr, t: &Bgr) -> Plane<f32> {
    let (iw, ih, tw, th) = (img.w, img.h, t.w, t.h);
    let (rw, rh) = (iw + 1 - tw, ih + 1 - th);
    let area = (tw * th) as f64;
    let mut planner = FftPlanner::<f64>::new();
    let fr = planner.plan_fft_forward(iw);
    let fc = planner.plan_fft_forward(ih);
    let ir = planner.plan_fft_inverse(iw);
    let ic = planner.plan_fft_inverse(ih);
    let fft2 = |buf: &mut Vec<Complex<f64>>, row: &std::sync::Arc<dyn rustfft::Fft<f64>>, col: &std::sync::Arc<dyn rustfft::Fft<f64>>| {
        for y in 0..ih {
            row.process(&mut buf[y * iw..(y + 1) * iw]);
        }
        let mut colv = vec![Complex::new(0.0, 0.0); ih];
        for x in 0..iw {
            for y in 0..ih {
                colv[y] = buf[y * iw + x];
            }
            col.process(&mut colv);
            for y in 0..ih {
                buf[y * iw + x] = colv[y];
            }
        }
    };
    let mut num = vec![0f64; rw * rh];
    let mut t_mean = [0f64; 3];
    let mut t_var = 0f64;
    for c in 0..3 {
        let m: f64 = (0..tw * th).map(|i| t.data[i * 3 + c] as f64).sum::<f64>() / area;
        t_mean[c] = m;
        t_var += (0..tw * th).map(|i| (t.data[i * 3 + c] as f64 - m).powi(2)).sum::<f64>() / area;
    }
    for c in 0..3 {
        let mut a: Vec<Complex<f64>> = (0..iw * ih).map(|i| Complex::new(img.data[i * 3 + c] as f64, 0.0)).collect();
        let mut b = vec![Complex::new(0.0, 0.0); iw * ih];
        for y in 0..th {
            for x in 0..tw {
                b[y * iw + x] = Complex::new(t.data[(y * tw + x) * 3 + c] as f64, 0.0);
            }
        }
        fft2(&mut a, &fr, &fc);
        fft2(&mut b, &fr, &fc);
        for (p, q) in a.iter_mut().zip(&b) {
            *p *= q.conj();
        }
        let mut colv = vec![Complex::new(0.0, 0.0); ih];
        for x in 0..iw {
            for y in 0..ih {
                colv[y] = a[y * iw + x];
            }
            ic.process(&mut colv);
            for y in 0..ih {
                a[y * iw + x] = colv[y];
            }
        }
        for y in 0..ih {
            ir.process(&mut a[y * iw..(y + 1) * iw]);
        }
        let scale = 1.0 / (iw * ih) as f64;
        for y in 0..rh {
            for x in 0..rw {
                num[y * rw + x] += a[y * iw + x].re * scale;
            }
        }
    }
    let mut s1 = vec![[0f64; 3]; (iw + 1) * (ih + 1)];
    let mut s2 = vec![0f64; (iw + 1) * (ih + 1)];
    for y in 0..ih {
        let mut row1 = [0f64; 3];
        let mut row2 = 0f64;
        for x in 0..iw {
            for c in 0..3 {
                let v = img.data[(y * iw + x) * 3 + c] as f64;
                row1[c] += v;
                row2 += v * v;
            }
            let i = (y + 1) * (iw + 1) + x + 1;
            let up = y * (iw + 1) + x + 1;
            for c in 0..3 {
                s1[i][c] = s1[up][c] + row1[c];
            }
            s2[i] = s2[up] + row2;
        }
    }
    let mut o = Plane::<f32>::new(rw, rh);
    if t_var < f64::EPSILON {
        o.d.iter_mut().for_each(|v| *v = 1.0);
        return o;
    }
    let t_norm = t_var.sqrt() / (1.0 / area).sqrt();
    let at = |x: usize, y: usize| y * (iw + 1) + x;
    for y in 0..rh {
        for x in 0..rw {
            let (a0, a1, a2, a3) = (at(x, y), at(x + tw, y), at(x, y + th), at(x + tw, y + th));
            let mut n = num[y * rw + x];
            let mut mean2 = 0f64;
            for c in 0..3 {
                let s = s1[a3][c] - s1[a1][c] - s1[a2][c] + s1[a0][c];
                mean2 += s * s;
                n -= s * t_mean[c];
            }
            mean2 /= area;
            let sum2 = s2[a3] - s2[a1] - s2[a2] + s2[a0];
            let diff2 = (sum2 - mean2).max(0.0);
            let t = if diff2 <= (0.5f64).min(10.0 * f32::EPSILON as f64 * sum2) { 0.0 } else { diff2.sqrt() * t_norm };
            let r = if n.abs() < t {
                n / t
            } else if n.abs() < t * 1.125 {
                n.signum()
            } else {
                0.0
            };
            o.set(x, y, r as f32);
        }
    }
    o
}
