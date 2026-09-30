#[derive(Clone, Debug)]
pub struct Bgr {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl Bgr {
    pub fn new(w: usize, h: usize) -> Self {
        Bgr { w, h, data: vec![0; w * h * 3] }
    }

    pub fn load(path: &std::path::Path) -> image::ImageResult<Self> {
        let rgb = image::open(path)?.to_rgb8();
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        let mut data = rgb.into_raw();
        for p in data.chunks_exact_mut(3) {
            p.swap(0, 2);
        }
        Ok(Bgr { w, h, data })
    }

    #[inline]
    pub fn px(&self, x: usize, y: usize) -> &[u8] {
        let i = (y * self.w + x) * 3;
        &self.data[i..i + 3]
    }

    pub fn pad_tb(&self, pad: usize) -> Bgr {
        let mut out = Bgr::new(self.w, self.h + pad * 2);
        let row = self.w * 3;
        out.data[pad * row..(pad + self.h) * row].copy_from_slice(&self.data);
        out
    }

    pub fn rot180(&self) -> Bgr {
        let mut out = Bgr::new(self.w, self.h);
        let n = self.w * self.h;
        for i in 0..n {
            let j = n - 1 - i;
            out.data[j * 3..j * 3 + 3].copy_from_slice(&self.data[i * 3..i * 3 + 3]);
        }
        out
    }

    pub fn rot90(&self) -> Bgr {
        let mut out = Bgr::new(self.h, self.w);
        for y in 0..self.h {
            for x in 0..self.w {
                let (nx, ny) = (y, self.w - 1 - x);
                let d = (ny * out.w + nx) * 3;
                out.data[d..d + 3].copy_from_slice(self.px(x, y));
            }
        }
        out
    }
}

const COEF_BITS: i32 = 11;
const COEF_SCALE: f32 = (1 << COEF_BITS) as f32;

#[inline]
fn cv_round(v: f32) -> i32 {
    v.round_ties_even() as i32
}

fn linear_tab(src: usize, dst: usize, clamp_pos: bool) -> Vec<(usize, usize, i32, i32)> {
    let scale = src as f64 / dst as f64;
    let last = src as i64 - 1;
    (0..dst)
        .map(|d| {
            let mut f = ((d as f64 + 0.5) * scale - 0.5) as f32;
            let mut s = f.floor() as i64;
            f -= s as f32;
            if clamp_pos {
                if s < 0 {
                    f = 0.0;
                    s = 0;
                }
                if s >= last {
                    f = 0.0;
                    s = last;
                }
            }
            let a0 = cv_round((1.0 - f) * COEF_SCALE).clamp(i16::MIN as i32, i16::MAX as i32);
            let a1 = cv_round(f * COEF_SCALE).clamp(i16::MIN as i32, i16::MAX as i32);
            (s.clamp(0, last) as usize, (s + 1).clamp(0, last) as usize, a0, a1)
        })
        .collect()
}

pub fn resize_linear(img: &Bgr, nw: usize, nh: usize) -> Bgr {
    let xt = linear_tab(img.w, nw, true);
    let yt = linear_tab(img.h, nh, false);
    let hrow = |y: usize| -> Vec<i32> {
        let mut r = vec![0i32; nw * 3];
        for (dx, &(sx, sx1, a0, a1)) in xt.iter().enumerate() {
            for c in 0..3 {
                let p0 = img.data[(y * img.w + sx) * 3 + c] as i32;
                let p1 = img.data[(y * img.w + sx1) * 3 + c] as i32;
                r[dx * 3 + c] = p0 * a0 + if a1 != 0 { p1 * a1 } else { 0 };
            }
        }
        r
    };
    let rows: Vec<Vec<i32>> = (0..img.h).map(hrow).collect();
    let mut out = Bgr::new(nw, nh);
    for (dy, &(sy, sy1, b0, b1)) in yt.iter().enumerate() {
        let r0 = &rows[sy];
        let r1 = &rows[sy1];
        let o = &mut out.data[dy * nw * 3..(dy + 1) * nw * 3];
        for i in 0..nw * 3 {
            let v = (((r0[i] >> 4) * b0) >> 16) + (((r1[i] >> 4) * b1) >> 16);
            o[i] = ((v + 2) >> 2).clamp(0, 255) as u8;
        }
    }
    out
}

const PREC: i32 = 22;

fn lanczos(x: f64) -> f64 {
    let sinc = |x: f64| if x == 0.0 { 1.0 } else { let x = x * std::f64::consts::PI; x.sin() / x };
    if (-3.0..3.0).contains(&x) { sinc(x) * sinc(x / 3.0) } else { 0.0 }
}

fn lanczos_tab(src: usize, dst: usize) -> Vec<(usize, Vec<i32>)> {
    let scale = src as f64 / dst as f64;
    let fscale = scale.max(1.0);
    let support = 3.0 * fscale;
    (0..dst)
        .map(|xx| {
            let center = (xx as f64 + 0.5) * scale;
            let ss = 1.0 / fscale;
            let xmin = ((center - support + 0.5) as i64).max(0) as usize;
            let xmax = ((center + support + 0.5) as i64).min(src as i64) as usize - xmin;
            let mut k: Vec<f64> = (0..xmax).map(|x| lanczos((x as f64 + xmin as f64 - center + 0.5) * ss)).collect();
            let ww: f64 = k.iter().sum();
            if ww != 0.0 {
                for v in k.iter_mut() {
                    *v /= ww;
                }
            }
            let ki = k
                .iter()
                .map(|&v| if v < 0.0 { (-0.5 + v * (1 << PREC) as f64) as i32 } else { (0.5 + v * (1 << PREC) as f64) as i32 })
                .collect();
            (xmin, ki)
        })
        .collect()
}

fn clip8(v: i64) -> u8 {
    (v >> PREC).clamp(0, 255) as u8
}

pub fn resize_lanczos(img: &Bgr, nw: usize, nh: usize) -> Bgr {
    let mut cur = img.clone();
    if nw != img.w {
        let t = lanczos_tab(img.w, nw);
        let mut out = Bgr::new(nw, img.h);
        for y in 0..img.h {
            for (xx, (xmin, k)) in t.iter().enumerate() {
                for c in 0..3 {
                    let mut ss: i64 = 1 << (PREC - 1);
                    for (x, &w) in k.iter().enumerate() {
                        ss += cur.data[(y * cur.w + xmin + x) * 3 + c] as i64 * w as i64;
                    }
                    out.data[(y * nw + xx) * 3 + c] = clip8(ss);
                }
            }
        }
        cur = out;
    }
    if nh != img.h {
        let t = lanczos_tab(img.h, nh);
        let mut out = Bgr::new(cur.w, nh);
        for (yy, (ymin, k)) in t.iter().enumerate() {
            for x in 0..cur.w {
                for c in 0..3 {
                    let mut ss: i64 = 1 << (PREC - 1);
                    for (y, &w) in k.iter().enumerate() {
                        ss += cur.data[((ymin + y) * cur.w + x) * 3 + c] as i64 * w as i64;
                    }
                    out.data[(yy * cur.w + x) * 3 + c] = clip8(ss);
                }
            }
        }
        cur = out;
    }
    cur
}

pub fn prep(img: &Bgr) -> Bgr {
    let scale = if img.h < 30 { 3 } else if img.h < 60 { 2 } else { 1 };
    let im = if scale > 1 { resize_lanczos(img, img.w * scale, img.h * scale) } else { img.clone() };
    let mut out = Bgr::new(im.w + 24, im.h + 24);
    let bg = [im.data[0], im.data[1], im.data[2]];
    for p in out.data.chunks_exact_mut(3) {
        p.copy_from_slice(&bg);
    }
    for y in 0..im.h {
        let d = ((y + 12) * out.w + 12) * 3;
        out.data[d..d + im.w * 3].copy_from_slice(&im.data[y * im.w * 3..(y + 1) * im.w * 3]);
    }
    out
}

pub fn to_chw(img: &Bgr, pad_w: usize, out: &mut [f32]) {
    let plane = img.h * pad_w;
    for y in 0..img.h {
        for x in 0..img.w {
            let p = img.px(x, y);
            for c in 0..3 {
                out[c * plane + y * pad_w + x] = (p[c] as f32 / 255.0 - 0.5) / 0.5;
            }
        }
    }
}

fn cubic_w(x: f32) -> [f32; 4] {
    const A: f32 = -0.75;
    let w0 = ((A * (x + 1.0) - 5.0 * A) * (x + 1.0) + 8.0 * A) * (x + 1.0) - 4.0 * A;
    let w1 = ((A + 2.0) * x - (A + 3.0)) * x * x + 1.0;
    let w2 = ((A + 2.0) * (1.0 - x) - (A + 3.0)) * (1.0 - x) * (1.0 - x) + 1.0;
    [w0, w1, w2, 1.0 - w0 - w1 - w2]
}

pub fn perspective(src: &[[f32; 2]; 4], dst: &[[f32; 2]; 4]) -> [f64; 9] {
    let mut a = [[0f64; 9]; 8];
    for i in 0..4 {
        let (x, y) = (src[i][0] as f64, src[i][1] as f64);
        let (u, v) = (dst[i][0] as f64, dst[i][1] as f64);
        a[i] = [x, y, 1.0, 0.0, 0.0, 0.0, -x * u, -y * u, u];
        a[i + 4] = [0.0, 0.0, 0.0, x, y, 1.0, -x * v, -y * v, v];
    }
    for i in 0..8 {
        let mut k = i;
        for j in i + 1..8 {
            if a[j][i].abs() > a[k][i].abs() {
                k = j;
            }
        }
        if a[k][i].abs() < f64::EPSILON * 100.0 {
            return [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        }
        a.swap(i, k);
        let d = -1.0 / a[i][i];
        for j in i + 1..8 {
            let alpha = a[j][i] * d;
            for c in i + 1..9 {
                a[j][c] += alpha * a[i][c];
            }
        }
    }
    let mut x = [0f64; 8];
    for i in (0..8).rev() {
        let mut s = a[i][8];
        for k in i + 1..8 {
            s -= a[i][k] * x[k];
        }
        x[i] = s / a[i][i];
    }
    let mut m = [0f64; 9];
    m[..8].copy_from_slice(&x);
    m[8] = 1.0;
    m
}

fn inv3(m: &[f64; 9]) -> [f64; 9] {
    let det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6]) + m[2] * (m[3] * m[7] - m[4] * m[6]);
    let d = 1.0 / det;
    [
        (m[4] * m[8] - m[5] * m[7]) * d,
        (m[2] * m[7] - m[1] * m[8]) * d,
        (m[1] * m[5] - m[2] * m[4]) * d,
        (m[5] * m[6] - m[3] * m[8]) * d,
        (m[0] * m[8] - m[2] * m[6]) * d,
        (m[2] * m[3] - m[0] * m[5]) * d,
        (m[3] * m[7] - m[4] * m[6]) * d,
        (m[1] * m[6] - m[0] * m[7]) * d,
        (m[0] * m[4] - m[1] * m[3]) * d,
    ]
}

pub fn warp_crop(img: &Bgr, pts: &[[f32; 2]; 4], w: usize, h: usize) -> Bgr {
    let mut out = Bgr::new(w, h);
    let axis = pts[0][1] == pts[1][1] && pts[2][1] == pts[3][1] && pts[0][0] == pts[3][0] && pts[1][0] == pts[2][0]
        && pts.iter().all(|p| p[0].fract() == 0.0 && p[1].fract() == 0.0)
        && (pts[1][0] - pts[0][0]) as usize == w
        && (pts[3][1] - pts[0][1]) as usize == h;
    let clampx = |x: i64| x.clamp(0, img.w as i64 - 1) as usize;
    let clampy = |y: i64| y.clamp(0, img.h as i64 - 1) as usize;
    if axis {
        let (x0, y0) = (pts[0][0] as i64, pts[0][1] as i64);
        for y in 0..h {
            for x in 0..w {
                let s = img.px(clampx(x0 + x as i64), clampy(y0 + y as i64));
                let d = (y * w + x) * 3;
                out.data[d..d + 3].copy_from_slice(s);
            }
        }
        return out;
    }
    let dst = [[0.0, 0.0], [w as f32, 0.0], [w as f32, h as f32], [0.0, h as f32]];
    let m = inv3(&perspective(pts, &dst));
    let bh0 = 16.min(h);
    let bw0 = (1024 / bh0).min(w);
    for y in 0..h {
        for x in 0..w {
            let bx = x / bw0 * bw0;
            let x1 = (x - bx) as f64;
            let (fb, fy) = (bx as f64, y as f64);
            let x0 = m[0] * fb + m[1] * fy + m[2];
            let y0 = m[3] * fb + m[4] * fy + m[5];
            let w0 = m[6] * fb + m[7] * fy + m[8];
            let z = w0 + m[6] * x1;
            let zz = if z != 0.0 { 1.0 / z } else { 0.0 };
            let (sx, sy) = ((x0 + m[0] * x1) * zz, (y0 + m[3] * x1) * zz);
            let (ix, iy) = (sx.floor() as i64, sy.floor() as i64);
            let wx = cubic_w((sx - ix as f64) as f32);
            let wy = cubic_w((sy - iy as f64) as f32);
            for c in 0..3 {
                let mut acc = 0f32;
                for j in 0..4 {
                    let yy = clampy(iy - 1 + j as i64);
                    let mut row = 0f32;
                    for i in 0..4 {
                        row += img.px(clampx(ix - 1 + i as i64), yy)[c] as f32 * wx[i];
                    }
                    acc += row * wy[j];
                }
                out.data[(y * w + x) * 3 + c] = (acc.round_ties_even() as i64).clamp(0, 255) as u8;
            }
        }
    }
    out
}
