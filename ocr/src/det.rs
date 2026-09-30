use image::{GrayImage, Luma};
use imageproc::contours::find_contours;
use imageproc::point::Point;

pub const THRESH: f32 = 0.3;
pub const BOX_THRESH: f32 = 0.5;
pub const UNCLIP_RATIO: f32 = 1.6;
const MIN_SIZE: f32 = 3.0;
const MAX_CANDIDATES: usize = 1000;

fn sign<T: PartialOrd + Default>(v: T) -> i32 {
    let z = T::default();
    (v > z) as i32 - (v < z) as i32
}

fn sklansky(a: &[[i32; 2]], p: &[usize], start: usize, end: usize, stack: &mut [usize], nsign: i32, sign2: i32) -> usize {
    let incr: i64 = if end > start { 1 } else { -1 };
    let (s, e) = (start as i64, end as i64);
    let (mut pprev, mut pcur, mut pnext) = (s, s + incr, s + 2 * incr);
    let mut stacksize: usize = 3;
    let pt = |i: i64| a[p[i as usize]];
    if start == end || pt(s) == pt(e) {
        stack[0] = start;
        return 1;
    }
    stack[0] = pprev as usize;
    stack[1] = pcur as usize;
    stack[2] = pnext as usize;
    let end2 = e + incr;
    while pnext != end2 {
        let cury = pt(pcur)[1];
        let nexty = pt(pnext)[1];
        let by = nexty - cury;
        if sign(by) != nsign {
            let ax = (pt(pcur)[0] - pt(pprev)[0]) as i64;
            let ay = (cury - pt(pprev)[1]) as i64;
            let bx = (pt(pnext)[0] - pt(pcur)[0]) as i64;
            let convexity = ay * bx - ax * by as i64;
            if sign(convexity) == sign2 && (ax != 0 || ay != 0) {
                pprev = pcur;
                pcur = pnext;
                pnext += incr;
                stack[stacksize] = pnext as usize;
                stacksize += 1;
            } else if pprev == s {
                pcur = pnext;
                stack[1] = pcur as usize;
                pnext += incr;
                stack[2] = pnext as usize;
            } else {
                stack[stacksize - 2] = pnext as usize;
                pcur = pprev;
                pprev = stack[stacksize - 4] as i64;
                stacksize -= 1;
            }
        } else {
            pnext += incr;
            stack[stacksize - 1] = pnext as usize;
        }
    }
    stacksize - 1
}

pub fn convex_hull(data: &[[i32; 2]]) -> Vec<[i32; 2]> {
    let total = data.len();
    if total == 0 {
        return vec![];
    }
    let mut p: Vec<usize> = (0..total).collect();
    p.sort_by(|&i, &j| (data[i][0], data[i][1], i).cmp(&(data[j][0], data[j][1], j)));
    let (mut miny, mut maxy) = (0usize, 0usize);
    for i in 1..total {
        let y = data[p[i]][1];
        if data[p[miny]][1] > y {
            miny = i;
        }
        if data[p[maxy]][1] < y {
            maxy = i;
        }
    }
    let mut hull: Vec<usize> = Vec::new();
    let mut stack = vec![0usize; total + 2];
    if data[p[0]] == data[p[total - 1]] {
        hull.push(0);
    } else {
        let tl_count = sklansky(data, &p, 0, maxy, &mut stack, -1, 1);
        let tl: Vec<usize> = stack[..tl_count].to_vec();
        let tr_count = sklansky(data, &p, total - 1, maxy, &mut stack, -1, -1);
        let tr: Vec<usize> = stack[..tr_count].to_vec();
        let (tl, tr) = (tr, tl);
        for i in 0..tl.len().saturating_sub(1) {
            hull.push(tl[i]);
        }
        for i in (1..tr.len()).rev() {
            hull.push(tr[i]);
        }
        let stop_idx: i64 = if tr.len() > 2 { tr[1] as i64 } else if tl.len() > 2 { tl[tl.len() - 2] as i64 } else { -1 };
        let bl_count = sklansky(data, &p, 0, miny, &mut stack, 1, -1);
        let mut bl: Vec<usize> = stack[..bl_count].to_vec();
        let br_count = sklansky(data, &p, total - 1, miny, &mut stack, 1, 1);
        let mut br: Vec<usize> = stack[..br_count].to_vec();
        if stop_idx >= 0 {
            let check_idx: i64 = if bl.len() > 2 {
                bl[1] as i64
            } else if bl.len() + br.len() > 2 {
                br[2 - bl.len()] as i64
            } else {
                -1
            };
            if check_idx == stop_idx || (check_idx >= 0 && data[p[check_idx as usize]] == data[p[stop_idx as usize]]) {
                bl.truncate(2);
                br.truncate(2);
            }
        }
        for i in 0..bl.len().saturating_sub(1) {
            hull.push(bl[i]);
        }
        for i in (1..br.len()).rev() {
            hull.push(br[i]);
        }
    }
    let mut hb: Vec<usize> = hull.iter().map(|&i| p[i]).collect();
    let nout = hb.len();
    if nout >= 3 {
        let (mut min_idx, mut max_idx, mut lt) = (0usize, 0usize, 0usize);
        for i in 1..nout {
            let idx = hb[i];
            lt += (hb[i - 1] < idx) as usize;
            if lt > 1 && lt as i64 <= i as i64 - 2 {
                break;
            }
            if idx < hb[min_idx] {
                min_idx = i;
            }
            if idx > hb[max_idx] {
                max_idx = i;
            }
        }
        let mmdist = (max_idx as i64 - min_idx as i64).unsigned_abs() as usize;
        if (mmdist == 1 || mmdist == nout - 1) && (lt <= 1 || lt >= nout - 2) {
            let ascending = (max_idx + 1) % nout == min_idx;
            let i0 = if ascending { min_idx } else { max_idx };
            if i0 > 0 {
                let mut tmp = vec![0usize; nout];
                let mut j = i0;
                let mut i = 0;
                while i < nout {
                    let curr = hb[j];
                    tmp[i] = curr;
                    let next_j = if j + 1 < nout { j + 1 } else { 0 };
                    let next = hb[next_j];
                    if i < nout - 1 && (ascending != (curr < next)) {
                        break;
                    }
                    j = next_j;
                    i += 1;
                }
                if i == nout {
                    hb = tmp;
                }
            }
        }
    }
    hb.iter().map(|&i| data[i]).collect()
}

fn rotating_calipers(pts: &[[f32; 2]], orientation: f32) -> [[f32; 2]; 3] {
    let n = pts.len();
    let mut minarea = f32::MAX;
    let mut vect = vec![[0f32; 2]; n];
    let mut inv_len = vec![0f32; n];
    let (mut left, mut bottom, mut right, mut top) = (0usize, 0usize, 0usize, 0usize);
    let mut pt0 = pts[0];
    let (mut left_x, mut right_x, mut top_y, mut bottom_y) = (pt0[0], pt0[0], pt0[1], pt0[1]);
    for i in 0..n {
        if pt0[0] < left_x {
            left_x = pt0[0];
            left = i;
        }
        if pt0[0] > right_x {
            right_x = pt0[0];
            right = i;
        }
        if pt0[1] > top_y {
            top_y = pt0[1];
            top = i;
        }
        if pt0[1] < bottom_y {
            bottom_y = pt0[1];
            bottom = i;
        }
        let pt = pts[if i + 1 < n { i + 1 } else { 0 }];
        let dx = pt[0] as f64 - pt0[0] as f64;
        let dy = pt[1] as f64 - pt0[1] as f64;
        vect[i] = [dx as f32, dy as f32];
        inv_len[i] = (1.0 / (dx * dx + dy * dy).sqrt()) as f32;
        pt0 = pt;
    }
    let _ = orientation;
    let (mut base_a, mut base_b): (f32, f32);
    let mut seq = [bottom, right, top, left];
    let mut buf_i = [0usize; 2];
    let mut buf = [0f32; 5];
    let first_right = |v1: [f32; 2], v2: [f32; 2]| {
        let t = [v1[1], -v1[0]];
        t[0] * v2[0] + t[1] * v2[1] < 0.0
    };
    for _ in 0..n {
        let rot = [
            vect[seq[0]],
            [vect[seq[1]][1], -vect[seq[1]][0]],
            [-vect[seq[2]][0], -vect[seq[2]][1]],
            [-vect[seq[3]][1], vect[seq[3]][0]],
        ];
        let mut main = 0usize;
        for i in 1..4 {
            if first_right(rot[i], rot[main]) {
                main = i;
            }
        }
        let pi = seq[main];
        let lx = vect[pi][0] * inv_len[pi];
        let ly = vect[pi][1] * inv_len[pi];
        (base_a, base_b) = match main {
            0 => (lx, ly),
            1 => (ly, -lx),
            2 => (-lx, -ly),
            _ => (-ly, lx),
        };
        seq[main] = if seq[main] + 1 == n { 0 } else { seq[main] + 1 };
        let dx = pts[seq[1]][0] - pts[seq[3]][0];
        let dy = pts[seq[1]][1] - pts[seq[3]][1];
        let width = dx * base_a + dy * base_b;
        let dx = pts[seq[2]][0] - pts[seq[0]][0];
        let dy = pts[seq[2]][1] - pts[seq[0]][1];
        let height = -dx * base_b + dy * base_a;
        let area = width * height;
        if area <= minarea {
            minarea = area;
            buf_i = [seq[3], seq[0]];
            buf = [base_a, width, base_b, height, area];
        }
    }
    let (a1, b1) = (buf[0], buf[2]);
    let (a2, b2) = (-buf[2], buf[0]);
    let c1 = a1 * pts[buf_i[0]][0] + pts[buf_i[0]][1] * b1;
    let c2 = a2 * pts[buf_i[1]][0] + pts[buf_i[1]][1] * b2;
    let idet = 1.0f32 / (a1 * b2 - a2 * b1);
    let px = (c1 * b2 - c2 * b1) * idet;
    let py = (a1 * c2 - a2 * c1) * idet;
    [[px, py], [a1 * buf[1], b1 * buf[1]], [a2 * buf[3], b2 * buf[3]]]
}

pub fn min_area_rect(pts: &[[i32; 2]]) -> ([[f32; 2]; 4], f32) {
    let hull: Vec<[f32; 2]> = convex_hull(pts).iter().map(|p| [p[0] as f32, p[1] as f32]).collect();
    let n = hull.len();
    let mut angle = -std::f64::consts::PI / 2.0;
    let (mut c, mut w, mut h) = ([0f32; 2], 0f32, 0f32);
    if n > 2 {
        let out = rotating_calipers(&hull, 1.0);
        c = [out[0][0] + (out[1][0] + out[2][0]) * 0.5, out[0][1] + (out[1][1] + out[2][1]) * 0.5];
        w = ((out[2][0] as f64).powi(2) + (out[2][1] as f64).powi(2)).sqrt() as f32;
        h = ((out[1][0] as f64).powi(2) + (out[1][1] as f64).powi(2)).sqrt() as f32;
        if out[1][0] == 0.0 && out[1][1] > 0.0 {
            std::mem::swap(&mut w, &mut h);
        } else {
            angle = -(out[1][0] as f64).atan2(out[1][1] as f64);
        }
    } else if n == 2 {
        c = [(hull[0][0] + hull[1][0]) * 0.5, (hull[0][1] + hull[1][1]) * 0.5];
        let dx = hull[0][0] as f64 - hull[1][0] as f64;
        let dy = hull[0][1] as f64 - hull[1][1] as f64;
        h = (dx * dx + dy * dy).sqrt() as f32;
        if dx == 0.0 {
            std::mem::swap(&mut w, &mut h);
        } else if dy < 0.0 {
            angle = dy.atan2(dx);
            std::mem::swap(&mut w, &mut h);
        } else if dy > 0.0 {
            angle = -dx.atan2(dy);
        }
    } else if n == 1 {
        c = hull[0];
    }
    let deg = (angle * 180.0 / std::f64::consts::PI) as f32;
    let r = deg as f64 * std::f64::consts::PI / 180.0;
    let b = (r.cos() as f32) * 0.5;
    let a = (r.sin() as f32) * 0.5;
    let (ah, aw, bh, bw) = (a * h, a * w, b * h, b * w);
    let p = [
        [c[0] - ah - bw, c[1] + bh - aw],
        [c[0] + ah - bw, c[1] - bh - aw],
        [c[0] + ah + bw, c[1] - bh + aw],
        [c[0] - ah + bw, c[1] + bh + aw],
    ];
    (p, w.min(h))
}

pub fn mini_box(pts: &[[i32; 2]]) -> ([[f32; 2]; 4], f32) {
    let (c, s) = min_area_rect(pts);
    let mut p = c.to_vec();
    p.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
    let (i1, i4) = if p[1][1] > p[0][1] { (0, 1) } else { (1, 0) };
    let (i2, i3) = if p[3][1] > p[2][1] { (2, 3) } else { (3, 2) };
    ([p[i1], p[i2], p[i3], p[i4]], s)
}

fn box_score(pred: &[f32], w: usize, h: usize, b: &[[f32; 2]; 4]) -> f32 {
    let xmin = (b.iter().map(|p| p[0]).fold(f32::MAX, f32::min).floor() as i64).clamp(0, w as i64 - 1);
    let xmax = (b.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil() as i64).clamp(0, w as i64 - 1);
    let ymin = (b.iter().map(|p| p[1]).fold(f32::MAX, f32::min).floor() as i64).clamp(0, h as i64 - 1);
    let ymax = (b.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil() as i64).clamp(0, h as i64 - 1);
    let poly: Vec<[f64; 2]> = b.iter().map(|p| [(p[0] - xmin as f32) as i64 as f64, (p[1] - ymin as f32) as i64 as f64]).collect();
    let (mut sum, mut n) = (0f64, 0usize);
    for y in 0..=(ymax - ymin) {
        for x in 0..=(xmax - xmin) {
            if in_poly(&poly, x as f64, y as f64) {
                sum += pred[(y + ymin) as usize * w + (x + xmin) as usize] as f64;
                n += 1;
            }
        }
    }
    if n == 0 { 0.0 } else { (sum / n as f64) as f32 }
}

fn in_poly(poly: &[[f64; 2]], x: f64, y: f64) -> bool {
    let n = poly.len();
    let (mut pos, mut neg) = (false, false);
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let c = (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
        if c > 1e-9 {
            pos = true;
        }
        if c < -1e-9 {
            neg = true;
        }
    }
    !(pos && neg)
}

fn unclip(b: &[[f32; 2]; 4]) -> Vec<[i32; 2]> {
    let f: Vec<[f64; 2]> = b.iter().map(|q| [q[0] as f64, q[1] as f64]).collect();
    let shoelace = |p: &[[f64; 2]]| -> f64 {
        let n = p.len();
        (0..n).map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1]).sum::<f64>() / 2.0
    };
    let area = shoelace(&f).abs();
    let per: f64 = (0..4).map(|i| ((f[(i + 1) % 4][0] - f[i][0]).powi(2) + (f[(i + 1) % 4][1] - f[i][1]).powi(2)).sqrt()).sum();
    if per == 0.0 {
        return b.iter().map(|p| [p[0] as i32, p[1] as i32]).collect();
    }
    let delta = area * UNCLIP_RATIO as f64 / per;
    let mut src: Vec<[f64; 2]> = Vec::new();
    for q in &f {
        let p = [q[0].trunc(), q[1].trunc()];
        if src.last() != Some(&p) {
            src.push(p);
        }
    }
    while src.len() > 1 && src.first() == src.last() {
        src.pop();
    }
    let len = src.len();
    if len < 3 {
        return src.iter().map(|p| [p[0] as i32, p[1] as i32]).collect();
    }
    if shoelace(&src) < 0.0 {
        src.reverse();
    }
    let cround = |v: f64| -> f64 { if v < 0.0 { (v - 0.5).trunc() } else { (v + 0.5).trunc() } };
    let y = if 0.25 > delta.abs() * 0.25 { delta.abs() * 0.25 } else { 0.25 };
    let mut steps = std::f64::consts::PI / (1.0 - y / delta.abs()).acos();
    if steps > delta.abs() * std::f64::consts::PI {
        steps = delta.abs() * std::f64::consts::PI;
    }
    let (sin, cos) = ((std::f64::consts::TAU / steps).sin(), (std::f64::consts::TAU / steps).cos());
    let steps_per_rad = steps / std::f64::consts::TAU;
    let normals: Vec<[f64; 2]> = (0..len)
        .map(|j| {
            let (a, c) = (src[j], src[(j + 1) % len]);
            let (dx, dy) = (c[0] - a[0], c[1] - a[1]);
            if dx == 0.0 && dy == 0.0 {
                return [0.0, 0.0];
            }
            let l = 1.0 / (dx * dx + dy * dy).sqrt();
            [dy * l, -dx * l]
        })
        .collect();
    let mut out: Vec<[f64; 2]> = Vec::new();
    let mut k = len - 1;
    for j in 0..len {
        let (nk, nj, p) = (normals[k], normals[j], src[j]);
        let mut sin_a = nk[0] * nj[1] - nj[0] * nk[1];
        let pt = |n: [f64; 2]| [cround(p[0] + n[0] * delta), cround(p[1] + n[1] * delta)];
        if (sin_a * delta).abs() < 1.0 {
            let cos_a = nk[0] * nj[0] + nj[1] * nk[1];
            if cos_a > 0.0 {
                out.push(pt(nk));
                k = j;
                continue;
            }
        } else if sin_a > 1.0 {
            sin_a = 1.0;
        } else if sin_a < -1.0 {
            sin_a = -1.0;
        }
        if sin_a * delta < 0.0 {
            out.push(pt(nk));
            out.push(p);
            out.push(pt(nj));
        } else {
            let a = sin_a.atan2(nk[0] * nj[0] + nk[1] * nj[1]);
            let n = (cround(steps_per_rad * a.abs()) as i64).max(1);
            let (mut x, mut yy) = (nk[0], nk[1]);
            for _ in 0..n {
                out.push([cround(p[0] + x * delta), cround(p[1] + yy * delta)]);
                let x2 = x;
                x = x * cos - sin * yy;
                yy = x2 * sin + yy * cos;
            }
            out.push(pt(nj));
        }
        k = j;
    }
    out.iter().map(|p| [p[0] as i32, p[1] as i32]).collect()
}

pub fn boxes_from_pred(pred: &[f32], w: usize, h: usize, src_w: usize, src_h: usize) -> Vec<[[f32; 2]; 4]> {
    let mut bin = GrayImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let mut on = false;
            for dy in 0..2 {
                for dx in 0..2 {
                    let (xx, yy) = (x as i64 - dx, y as i64 - dy);
                    if xx >= 0 && yy >= 0 && pred[yy as usize * w + xx as usize] > THRESH {
                        on = true;
                    }
                }
            }
            if on {
                bin.put_pixel(x as u32, y as u32, Luma([255]));
            }
        }
    }
    let contours = find_contours::<i32>(&bin);
    let mut out = Vec::new();
    for c in contours.iter().take(MAX_CANDIDATES) {
        let pts: Vec<[i32; 2]> = c.points.iter().map(|p: &Point<i32>| [p.x, p.y]).collect();
        let (b, s) = mini_box(&pts);
        if s < MIN_SIZE {
            continue;
        }
        if box_score(pred, w, h, &b) < BOX_THRESH {
            continue;
        }
        let (b, s) = mini_box(&unclip(&b));
        if s < MIN_SIZE + 2.0 {
            continue;
        }
        let mut r = b;
        for p in r.iter_mut() {
            p[0] = ((p[0] / w as f32 * src_w as f32).round_ties_even()).clamp(0.0, src_w as f32);
            p[1] = ((p[1] / h as f32 * src_h as f32).round_ties_even()).clamp(0.0, src_h as f32);
        }
        out.push(r);
    }
    out
}

pub fn filter_boxes(boxes: Vec<[[f32; 2]; 4]>, img_w: usize, img_h: usize) -> Vec<[[f32; 2]; 4]> {
    let mut out = Vec::new();
    for b in boxes {
        let mut xs: Vec<[f32; 2]> = b.to_vec();
        xs.sort_by(|a, c| a[0].partial_cmp(&c[0]).unwrap());
        let mut left = [xs[0], xs[1]];
        let mut right = [xs[2], xs[3]];
        left.sort_by(|a, c| a[1].partial_cmp(&c[1]).unwrap());
        right.sort_by(|a, c| a[1].partial_cmp(&c[1]).unwrap());
        let mut r = [left[0], right[0], right[1], left[1]];
        for p in r.iter_mut() {
            p[0] = (p[0].max(0.0).min(img_w as f32 - 1.0)) as i64 as f32;
            p[1] = (p[1].max(0.0).min(img_h as f32 - 1.0)) as i64 as f32;
        }
        let dist = |a: [f32; 2], c: [f32; 2]| ((a[0] - c[0]).powi(2) + (a[1] - c[1]).powi(2)).sqrt();
        if dist(r[0], r[1]) as i64 <= 3 || dist(r[0], r[3]) as i64 <= 3 {
            continue;
        }
        out.push(r);
    }
    out
}

pub fn sort_boxes(mut b: Vec<[[f32; 2]; 4]>) -> Vec<[[f32; 2]; 4]> {
    b.sort_by(|a, c| (a[0][1], a[0][0]).partial_cmp(&(c[0][1], c[0][0])).unwrap());
    let n = b.len();
    for i in 0..n.saturating_sub(1) {
        let mut j = i as i64;
        while j >= 0 {
            let ju = j as usize;
            if (b[ju + 1][0][1] - b[ju][0][1]).abs() < 10.0 && b[ju + 1][0][0] < b[ju][0][0] {
                b.swap(ju, ju + 1);
                j -= 1;
            } else {
                break;
            }
        }
    }
    b
}
