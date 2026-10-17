use crate::cv::crop_bgr;
use crate::qs_sig;
use ocr::img::{resize_linear, Bgr};
use ocr::{Item, Ocr};

pub type Rect = (i32, i32, i32, i32);

#[derive(Debug, Clone)]
pub struct Cell {
    pub rect: Rect,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct Pair {
    pub qs: Cell,
    pub bag: Cell,
    pub cell: Rect,
    pub sure: bool,
}

#[derive(Debug, Default)]
pub struct Found {
    pub exp: Option<Rect>,
    pub inv: Option<Rect>,
    pub meso: Option<Rect>,
    pub cells: Vec<Cell>,
    pub pairs: Vec<Pair>,
}

fn bounds(it: &Item) -> Option<(f32, f32, f32, f32)> {
    let q = it.quad?;
    let xs = q.iter().map(|p| p[0]);
    let ys = q.iter().map(|p| p[1]);
    Some((
        xs.clone().fold(f32::MAX, f32::min),
        ys.clone().fold(f32::MAX, f32::min),
        xs.fold(f32::MIN, f32::max),
        ys.fold(f32::MIN, f32::max),
    ))
}

fn number(s: &str) -> Option<i64> {
    let t: String = s.chars().filter(|c| !matches!(c, ',' | '，' | '.' | ' ')).collect();
    if t.is_empty() || t.len() > 12 || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

fn is_meso(s: &str) -> bool {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let groups: Vec<&str> = t.split(|c| matches!(c, ',' | '，' | '.')).collect();
    groups.len() >= 2
        && groups.iter().all(|g| !g.is_empty() && g.chars().all(|c| c.is_ascii_digit()))
        && groups[0].len() <= 3
        && groups[1..].iter().all(|g| g.len() == 3)
}

fn digits(s: &str) -> usize {
    s.chars().filter(|c| c.is_ascii_digit()).count()
}

fn clamp_rect(r: (f32, f32, f32, f32), w: usize, h: usize) -> Rect {
    let x0 = r.0.max(0.0).round() as i32;
    let y0 = r.1.max(0.0).round() as i32;
    let x1 = r.2.min(w as f32).round() as i32;
    let y1 = r.3.min(h as f32).round() as i32;
    (x0, y0, (x1 - x0).max(1), (y1 - y0).max(1))
}

fn ocr_zoom(ocr: &mut Ocr, img: &Bgr, r: (usize, usize, usize, usize), scale: f64) -> Vec<(Item, (f32, f32, f32, f32))> {
    let (x0, y0, x1, y1) = (r.0.min(img.w), r.1.min(img.h), r.2.min(img.w), r.3.min(img.h));
    if x1 <= x0 + 4 || y1 <= y0 + 4 {
        return vec![];
    }
    let crop = crop_bgr(img, x0, y0, x1, y1);
    let big = if scale > 1.0 { resize_linear(&crop, (crop.w as f64 * scale) as usize, (crop.h as f64 * scale) as usize) } else { crop };
    let Ok(items) = ocr.run(&big, true, false) else { return vec![] };
    items
        .into_iter()
        .filter_map(|it| {
            let b = bounds(&it)?;
            let s = scale.max(1.0) as f32;
            Some((it, (x0 as f32 + b.0 / s, y0 as f32 + b.1 / s, x0 as f32 + b.2 / s, y0 as f32 + b.3 / s)))
        })
        .collect()
}

pub fn find(img: &Bgr, ocr: &mut Ocr) -> Found {
    find_with(img, ocr, 1)
}

pub fn find_with(img: &Bgr, ocr: &mut Ocr, down: usize) -> Found {
    let mut f = Found::default();
    let all = if down > 1 {
        let k = down as f32;
        let small = resize_linear(img, img.w / down, img.h / down);
        read_all(ocr, &small).into_iter().map(|(it, b)| (it, (b.0 * k, b.1 * k, b.2 * k, b.3 * k))).collect()
    } else {
        read_all(ocr, img)
    };
    f.exp = find_exp(img, &all);
    find_bag(img, ocr, &all, &mut f);
    if !f.cells.is_empty() {
        let exp = f.exp.unwrap_or(((img.w as f32 * 0.45) as i32, (img.h as f32 * 0.97) as i32, 1, 1));
        f.pairs = find_quickslot(img, ocr, exp, &f.cells);
    }
    f
}

fn read_all(ocr: &mut Ocr, img: &Bgr) -> Vec<(Item, (f32, f32, f32, f32))> {
    read_region(ocr, img, (0, 0, img.w, img.h), 1.0)
}

fn read_region(ocr: &mut Ocr, img: &Bgr, r: (usize, usize, usize, usize), scale: f64) -> Vec<(Item, (f32, f32, f32, f32))> {
    let (x0, y0, x1, y1) = (r.0.min(img.w), r.1.min(img.h), r.2.min(img.w), r.3.min(img.h));
    let s = scale.max(1.0);
    let tile = (1900.0 / s) as usize;
    let over = tile / 6;
    if (x1 - x0) <= tile && (y1 - y0) <= tile {
        return ocr_zoom(ocr, img, (x0, y0, x1, y1), scale);
    }
    let starts = |a: usize, b: usize| -> Vec<usize> {
        let mut v = vec![a];
        while v.last().unwrap() + tile < b {
            v.push((v.last().unwrap() + tile - over).min(b.saturating_sub(tile)).max(a));
        }
        v
    };
    let mut all: Vec<(Item, (f32, f32, f32, f32))> = vec![];
    for &y in &starts(y0, y1) {
        for &x in &starts(x0, x1) {
            for (it, b) in ocr_zoom(ocr, img, (x, y, (x + tile).min(x1), (y + tile).min(y1)), scale) {
                let (cx, cy) = ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0);
                let h = b.3 - b.1;
                if all.iter().any(|(o, ob)| o.text == it.text && ((ob.0 + ob.2) / 2.0 - cx).abs() < h && ((ob.1 + ob.3) / 2.0 - cy).abs() < h) {
                    continue;
                }
                let cut = all.iter().position(|(_, ob)| ob.0 < b.2 && b.0 < ob.2 && ob.1 < b.3 && b.1 < ob.3 && (ob.1 + ob.3 - b.1 - b.3).abs() < h);
                match cut {
                    Some(i) if all[i].0.text.chars().count() >= it.text.chars().count() => {}
                    Some(i) => all[i] = (it, b),
                    None => all.push((it, b)),
                }
            }
        }
    }
    all
}

fn find_exp(img: &Bgr, all: &[(Item, (f32, f32, f32, f32))]) -> Option<Rect> {
    let (_, b) = all
        .iter()
        .filter(|(it, _)| it.text.contains('[') && !it.text.contains('/') && digits(&it.text) >= 5)
        .max_by(|a, b| a.1.1.total_cmp(&b.1.1))?;
    let h = b.3 - b.1;
    let (y0, y1) = ((b.1 - h * 0.2).max(0.0), (b.3 + h * 0.15).min(img.h as f32));
    let right = (b.2 + h * 1.2).min(img.w as f32);
    let mut last_black = None;
    let x_end = (b.0 + (b.2 - b.0) * 0.45) as usize;
    for x in ((b.0 - h * 0.3).max(0.0) as usize)..x_end.min(img.w) {
        let n = (y0 as usize..(y1 as usize).min(img.h))
            .filter(|&y| {
                let i = (y * img.w + x) * 3;
                img.data[i].max(img.data[i + 1]).max(img.data[i + 2]) < 30
            })
            .count();
        if n >= 2 {
            last_black = Some(x);
        }
    }
    let left = last_black.map_or(b.0, |x| x as f32 + (h * 0.06).max(1.0));
    Some(clamp_rect((left, y0, right, y1), img.w, img.h))
}

fn find_bag(img: &Bgr, ocr: &mut Ocr, all: &[(Item, (f32, f32, f32, f32))], f: &mut Found) {
    let Some((_, tab)) = all.iter().find(|(it, _)| it.text.contains("消耗")) else { return };
    let th = tab.3 - tab.1;
    let tcy = (tab.1 + tab.3) / 2.0;
    let tw = tab.2 - tab.0;
    let mut same: Vec<&(f32, f32, f32, f32)> = all.iter().map(|(_, b)| b).filter(|b| ((b.1 + b.3) / 2.0 - tcy).abs() < th * 0.6 && (b.0 - tab.0).abs() < tw * 12.0).collect();
    same.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut row: Vec<&(f32, f32, f32, f32)> = same.iter().copied().filter(|b| (b.0 - tab.0).abs() < tw * 5.0).collect();
    let mut r = row.iter().map(|b| b.2).fold(tab.2, f32::max);
    for &b in same.iter().filter(|b| (b.0 - tab.0) >= tw * 5.0) {
        if b.0 - r >= tw * 1.5 {
            break;
        }
        row.push(b);
        r = r.max(b.2);
    }
    let mut l = row.iter().map(|b| b.0).fold(tab.0, f32::min);
    for &b in same.iter().rev().filter(|b| (tab.0 - b.0) >= tw * 5.0) {
        if l - b.2 >= tw * 1.5 {
            break;
        }
        row.push(b);
        l = l.min(b.0);
    }
    let title = all
        .iter()
        .find(|(it, b)| it.text.contains("道具") && b.3 <= tab.1 + th * 0.5 && tab.1 - b.3 < th * 3.0 && (b.0 - tab.0).abs() < (tab.2 - tab.0) * 5.0)
        .map(|(_, b)| b.0);
    let left = row.iter().map(|b| b.0).chain(title).fold(tab.0, f32::min) - th * 0.3;
    let right = row.iter().map(|b| b.2).fold(tab.2, f32::max) + th * 0.3;
    let top = tab.3 + th * 0.2;
    let meso = all
        .iter()
        .filter(|(it, b)| b.1 > top && b.0 >= left - th && b.2 <= right + th * 3.0 && is_meso(&it.text))
        .min_by(|a, b| a.1.1.total_cmp(&b.1.1))
        .map(|(_, b)| *b)
        .or_else(|| {
            let coin = all
                .iter()
                .filter(|(it, b)| b.1 > top && b.0 >= left - th && b.0 <= right + th * 3.0 && (it.text.contains("金幣") || it.text.contains("金币")))
                .min_by(|a, b| a.1.1.total_cmp(&b.1.1))?
                .1;
            let cy = (coin.1 + coin.3) / 2.0;
            all.iter()
                .filter(|(it, b)| ((b.1 + b.3) / 2.0 - cy).abs() < (coin.3 - coin.1) * 0.6 && b.2 <= coin.0 + th && b.0 >= left - th && number(&it.text).is_some())
                .max_by(|a, b| a.1.2.total_cmp(&b.1.2))
                .map(|(_, b)| *b)
        });
    if let Some(m) = meso {
        let mh = m.3 - m.1;
        f.meso = Some(clamp_rect((m.0 - mh * 0.15, m.1 - mh * 0.25, m.2 + mh * 0.15, m.3 + mh * 0.25), img.w, img.h));
    }
    let bottom = meso.map_or(top + (right - left) * 1.4, |m| m.1 - (m.3 - m.1) * 0.5);
    if std::env::var_os("AUTO_DEBUG").is_some() {
        eprintln!("bag tab {tab:?} row {row:?} title {title:?} → ({left:.0},{top:.0})-({right:.0},{bottom:.0}) meso {meso:?}");
        for (it, b) in all.iter().filter(|(_, b)| b.0 < right + th * 6.0 && b.2 > left - th * 3.0 && b.3 > tab.1 - th * 4.0 && b.1 < tab.3 + th * 2.0) {
            eprintln!("  near {:?} @({:.0},{:.0})-({:.0},{:.0})", it.text, b.0, b.1, b.2, b.3);
        }
    }
    if bottom <= top {
        return;
    }
    f.inv = Some(clamp_rect((left, top, right, bottom), img.w, img.h));
    let scale = (40.0 / th as f64).max(1.0).min(3.0);
    let nums: Vec<(i64, (f32, f32, f32, f32))> = read_region(ocr, img, (left as usize, top as usize, (right + th) as usize, bottom as usize), scale)
        .into_iter()
        .filter_map(|(it, b)| number(&it.text).map(|n| (n, b)))
        .collect();
    if nums.len() < 2 {
        return;
    }
    let sx = spacing_min(nums.iter().map(|n| n.1.0).collect(), th * 1.2, th * 2.2);
    let sy = spacing_min(nums.iter().map(|n| n.1.3).collect(), th * 1.2, th * 2.2);
    if std::env::var_os("AUTO_DEBUG").is_some() {
        eprintln!("bag th {th:.1} scale {scale:.2} sx {sx:?} sy {sy:?}");
        for (n, b) in &nums {
            eprintln!("  num {n} @({:.0},{:.0})-({:.0},{:.0})", b.0, b.1, b.2, b.3);
        }
    }
    let (Some(sx), Some(sy)) = (sx, sy) else { return };
    let mut nums = nums;
    let merged: Vec<bool> = nums
        .iter()
        .map(|n| nums.iter().any(|o| o.1.0 > n.1.0 && (o.1.3 - n.1.3).abs() < (n.1.3 - n.1.1) * 0.5 && n.1.2 > o.1.0 + (n.1.3 - n.1.1) * 0.2))
        .collect();
    let mut keep = merged.iter().map(|m| !m);
    nums.retain(|_| keep.next().unwrap_or(true));
    if nums.is_empty() {
        return;
    }
    let nh = {
        let mut v: Vec<f32> = nums.iter().map(|n| n.1.3 - n.1.1).collect();
        v.sort_by(f32::total_cmp);
        v[v.len() / 2]
    };
    let tall = |n: &(i64, (f32, f32, f32, f32))| n.1.3 - n.1.1 >= nh * 0.75;
    let low = nums.iter().filter(|n| tall(n)).map(|n| n.1.3).fold(f32::MIN, f32::max) + sy * 0.5;
    nums.retain(|n| tall(n) || n.1.3 < low);
    let cols = lattice(&nums.iter().map(|n| n.1.0).collect::<Vec<_>>(), sx);
    let rows = lattice(&nums.iter().map(|n| n.1.3).collect::<Vec<_>>(), sy);
    for n in nums.iter_mut() {
        let Some(x0) = cols.iter().copied().find(|c| n.1.0 - c > -sx * 0.2 && n.1.0 - c < sx * 0.6) else { continue };
        if n.1.0 - x0 < nh * 0.8 && tall(n) {
            continue;
        }
        let strip = (x0 - nh * 0.3, n.1.3 - nh * 1.3, x0 + sx * 0.85, n.1.3 + nh * 0.3);
        if let Some(m) = gap_count(ocr, img, strip).filter(|m| *m != n.0 && m.to_string().ends_with(&n.0.to_string())) {
            n.0 = m;
            n.1.0 = x0;
        }
    }
    nums.retain(|n| cols.iter().any(|c| (c - n.1.0).abs() < sx * 0.4) && rows.iter().any(|r| (r - n.1.3).abs() < sy * 0.4));
    for &by in &rows {
        for &x0 in &cols {
            if nums.iter().any(|n| (n.1.0 - x0).abs() < sx * 0.4 && (n.1.3 - by).abs() < sy * 0.4) {
                continue;
            }
            let strip = (x0 - nh * 0.3, by - nh * 1.3, x0 + sx * 0.85, by + nh * 0.3);
            if let Some(n) = gap_count(ocr, img, strip) {
                nums.push((n, (x0, by - nh, x0 + nh * 2.0, by)));
            }
        }
    }
    let snap = |v: f32, at: &[f32], gap: f32| at.iter().copied().find(|a| (a - v).abs() < gap).unwrap_or(v);
    for (n, b) in nums {
        let (x0, by) = (snap(b.0, &cols, sx * 0.4), snap(b.3, &rows, sy * 0.4));
        let r = (x0 - sx * 0.05, by + sy * 0.04 - sy * 0.88, x0 - sx * 0.05 + sx * 0.88, by + sy * 0.04);
        f.cells.push(Cell { rect: clamp_rect(r, img.w, img.h), count: n });
    }
}

fn groups(mut v: Vec<f32>, gap: f32) -> Vec<f32> {
    v.sort_by(f32::total_cmp);
    let mut out: Vec<Vec<f32>> = vec![];
    for x in v {
        match out.last_mut() {
            Some(g) if x - g[0] < gap => g.push(x),
            _ => out.push(vec![x]),
        }
    }
    out.into_iter().map(|g| g[g.len() / 2]).collect()
}

fn lattice(v: &[f32], s: f32) -> Vec<f32> {
    let off = |x: f32, a: f32| {
        let d = (x - a) / s;
        (d - d.round()).abs() * s
    };
    let Some(&a) = v.iter().max_by_key(|&&a| v.iter().filter(|&&x| off(x, a) < s * 0.15).count()) else { return vec![] };
    groups(v.iter().copied().filter(|&x| off(x, a) < s * 0.2).collect(), s * 0.4)
}

fn spacing(v: Vec<f32>, gap: f32) -> Option<f32> {
    spacing_min(v, gap, 0.0)
}

fn spacing_min(mut v: Vec<f32>, gap: f32, min: f32) -> Option<f32> {
    v.sort_by(f32::total_cmp);
    let mut groups: Vec<f32> = vec![];
    for x in v {
        match groups.last() {
            Some(&g) if x - g < gap => {}
            _ => groups.push(x),
        }
    }
    let mut d: Vec<f32> = groups.windows(2).map(|w| w[1] - w[0]).filter(|&d| d > gap).collect();
    if d.is_empty() {
        return None;
    }
    d.sort_by(f32::total_cmp);
    let mut best: Option<(usize, Vec<f32>)> = None;
    for &c in d.iter().filter(|&&c| c >= min) {
        let near = |x: f32, k: f32| (x - c * k).abs() < c * 0.25;
        let ones: Vec<f32> = d.iter().copied().filter(|&x| near(x, 1.0)).collect();
        let support = d.iter().filter(|&&x| near(x, 1.0) || near(x, 2.0) || near(x, 3.0)).count();
        if best.as_ref().map_or(true, |b| support > b.0) {
            best = Some((support, ones));
        }
    }
    let ones = best?.1;
    Some(ones[ones.len() / 2])
}

fn bag_groups(imgs: &[Bgr], sigs: &[Vec<f32>]) -> Vec<usize> {
    let mut g: Vec<usize> = (0..imgs.len()).collect();
    fn root(g: &mut [usize], mut i: usize) -> usize {
        while g[i] != i {
            g[i] = g[g[i]];
            i = g[i];
        }
        i
    }
    for i in 0..imgs.len() {
        for j in i + 1..imgs.len() {
            if same_color(&sigs[i], &sigs[j]) < 0.35 && look(&imgs[i], &imgs[j]) < 0.2 {
                let (a, b) = (root(&mut g, i), root(&mut g, j));
                g[a] = b;
            }
        }
    }
    (0..imgs.len()).map(|i| root(&mut g, i)).collect()
}

fn find_quickslot(img: &Bgr, ocr: &mut Ocr, exp: Rect, cells: &[Cell]) -> Vec<Pair> {
    let debug = std::env::var("AUTO_DEBUG").is_ok();
    let (x0, y1) = ((exp.0 + exp.2) as f32, exp.1 as f32);
    let y0 = (y1 - img.h as f32 * 0.3).max(0.0);
    let counts: Vec<i64> = cells.iter().map(|c| c.count).collect();
    let bag_h = median(cells.iter().map(|c| c.rect.3 as f32).collect());
    let read = |ocr: &mut Ocr, r: (f32, f32, f32, f32), scale: f64| -> Vec<(i64, (f32, f32, f32, f32))> {
        read_region(ocr, img, (r.0.max(0.0) as usize, r.1.max(0.0) as usize, r.2 as usize, r.3 as usize), scale)
            .into_iter()
            .filter_map(|(it, b)| {
                if debug {
                    eprintln!("qs text {:?} {:?}", it.text, b);
                }
                let h = b.3 - b.1;
                if h < bag_h * 0.1 || h > bag_h * 0.36 {
                    return None;
                }
                number(&it.text).filter(|n| counts.contains(n)).map(|n| (n, b))
            })
            .collect()
    };
    let scale = (130.0 / bag_h as f64).max(1.0).min(4.0);
    let mut nums = read(ocr, (x0, y0, img.w as f32, y1), scale);
    if nums.is_empty() {
        return vec![];
    }
    let nh = median(nums.iter().map(|n| n.1.3 - n.1.1).collect());
    let pad = nh * 7.0;
    let area = (
        nums.iter().map(|n| n.1.0).fold(f32::MAX, f32::min) - pad,
        nums.iter().map(|n| n.1.1).fold(f32::MAX, f32::min) - pad,
        (nums.iter().map(|n| n.1.2).fold(f32::MIN, f32::max) + pad).min(img.w as f32),
        (nums.iter().map(|n| n.1.3).fold(f32::MIN, f32::max) + pad).min(y1),
    );
    for n in read(ocr, area, (scale * 1.5).min(4.0)) {
        if !nums.iter().any(|m| m.0 == n.0 && (m.1.0 - n.1.0).abs() < nh && (m.1.1 - n.1.1).abs() < nh) {
            nums.push(n);
        }
    }
    let nh = median(nums.iter().map(|n| n.1.3 - n.1.1).collect());
    let cs = nh * 3.1;
    let (ex, ey) = (median(cells.iter().map(|c| c.rect.2 as f32).collect()) * 0.85, bag_h * 0.76);
    let fit = |p: Option<f32>, e: f32| p.filter(|&p| p > e * 0.8 && p < e * 1.25);
    let px = fit(spacing(nums.iter().map(|n| (n.1.0 + n.1.2) / 2.0).collect(), cs * 0.5), ex).unwrap_or(ex);
    let py = fit(spacing(nums.iter().map(|n| n.1.3).collect(), cs * 0.5), ey).unwrap_or(ey);
    let seeds: Vec<(f32, f32)> = nums.iter().map(|n| ((n.1.0 + n.1.2) / 2.0, n.1.3)).collect();
    let lefts: Vec<f32> = nums.iter().map(|n| n.1.0).collect();
    let pxl = fit(spacing(lefts.clone(), cs * 0.5), ex);
    let pyb = fit(spacing(seeds.iter().map(|s| s.1).collect(), cs * 0.5), ey);
    let (cw, ch) = (pxl.unwrap_or(ex) * 0.87, pyb.unwrap_or(ey) * 0.89);
    let gxp = pxl.unwrap_or(ex);
    let ax = lefts
        .iter()
        .copied()
        .max_by_key(|&a| lefts.iter().filter(|&&x| ((x - a) / gxp - ((x - a) / gxp).round()).abs() < 0.15).count())
        .unwrap_or(0.0);
    let col = |v: f32| {
        let k = (v - ax) / gxp;
        ax + (if (k - k.round()).abs() < 0.25 { k.round() } else { k.ceil() }) * gxp
    };
    let snap = |v: f32, refs: &[f32], pitch: Option<f32>| -> f32 {
        let Some(p) = pitch else { return v };
        let r = refs.iter().copied().min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs())).unwrap_or(v);
        r + ((v - r) / p).round() * p
    };
    let bottoms: Vec<f32> = seeds.iter().map(|s| s.1).collect();
    for &(sx, sy) in &seeds {
        for dy in -1i32..=1 {
            for dx in -4i32..=4 {
                let (cx, by) = (sx + dx as f32 * px, sy + dy as f32 * py);
                if cx - cs / 2.0 < x0 || cx + cs / 2.0 > img.w as f32 || by - cs < y0 || by > y1 {
                    continue;
                }
                if nums.iter().any(|m| ((m.1.0 + m.1.2) / 2.0 - cx).abs() < px * 0.4 && (m.1.3 - by).abs() < py * 0.4) {
                    continue;
                }
                let strip = (cx - cs * 0.55, by - nh * 1.5, cx + cs * 0.55, by + nh * 0.4);
                let read_n = |ocr: &mut Ocr, r: (f32, f32, f32, f32)| {
                    number(&rec_zoom(ocr, img, r, 3.0)).filter(|n| counts.contains(n)).or_else(|| quick(ocr, img, r).filter(|n| counts.contains(n)))
                };
                let got = read_n(ocr, strip);
                if debug {
                    eprintln!("qs guess ({cx:.0},{by:.0}) {got:?}");
                }
                if let Some(n) = got {
                    let step = nh * 0.12;
                    let mut l = strip.0;
                    while l + step < cx && read_n(ocr, (l + step, strip.1, strip.2, strip.3)) == Some(n) {
                        l += step;
                    }
                    let l = col((l - nh * 0.25).max(strip.0));
                    nums.push((n, (l, by - nh, l + n.to_string().len() as f32 * nh * 0.63, by)));
                }
            }
        }
    }
    let imgs: Vec<Bgr> = cells.iter().map(|c| cell_img(img, c.rect)).collect();
    let sigs: Vec<Vec<f32>> = cells.iter().map(|c| sig(img, c.rect)).collect();
    let group = bag_groups(&imgs, &sigs);
    let mut pairs: Vec<Pair> = vec![];
    for (n, b) in nums {
        let (l, bt) = (col(b.0), snap(b.3, &bottoms, pyb) - 1.0);
        let qr = clamp_rect((l, bt - ch, l + cw, bt), img.w, img.h);
        let qc = cell_img(img, qr);
        let qsig = sig(img, qr);
        let dl: Vec<f32> = imgs.iter().map(|c| look(&qc, c)).collect();
        let mut gs: Vec<(usize, f32, f32, bool)> = vec![];
        for (i, &g) in group.iter().enumerate() {
            let h = same_color(&qsig, &sigs[i]);
            match gs.iter_mut().find(|x| x.0 == g) {
                Some(x) => {
                    x.1 = x.1.min(dl[i]);
                    x.2 = x.2.min(h);
                    x.3 |= cells[i].count == n;
                }
                None => gs.push((g, dl[i], h, cells[i].count == n)),
            }
        }
        gs.sort_by(|a, b| a.1.total_cmp(&b.1));
        let cand: Vec<&(usize, f32, f32, bool)> = gs.iter().filter(|x| x.3).collect();
        let pick = cand.first().filter(|g1| {
            let beats = |g2: &&(usize, f32, f32, bool)| g2.1 - g1.1 >= 0.05 || g2.2 - g1.2 >= 0.2;
            g1.1 < 0.28 && cand.iter().skip(1).all(|g2| beats(g2))
                && gs.iter().filter(|g2| g2.0 != g1.0 && g2.1 < g1.1 + 0.015).all(|g2| g2.2 - g1.2 >= 0.2)
        });
        let only = cand.first().filter(|g1| cand.len() == 1 && g1.1 < 0.4 && gs.first().map_or(false, |g| g.0 == g1.0));
        let sure = pick.or(only).is_some();
        if debug {
            eprintln!("qs {n} {:?} groups {:?} pick {:?} sure {sure}", qr, gs.iter().take(4).map(|x| (x.0, (x.1 * 100.0).round() / 100.0, (x.2 * 10.0).round() / 10.0, x.3)).collect::<Vec<_>>(), pick.map(|g| g.0));
        }
        let Some(&&(g1, ..)) = pick.or(only).or(cand.first()) else { continue };
        let Some(c) = (0..cells.len()).filter(|&i| group[i] == g1 && cells[i].count == n).min_by(|&a, &b| dl[a].total_cmp(&dl[b])).map(|i| &cells[i]) else { continue };
        let x0 = b.0 - nh * 0.35;
        let cand = |right: f32, top: f32, bottom: f32| {
            let x1 = if n >= 1000 { b.2 + nh * right } else { (b.2 + nh * right).max(x0 + nh * 2.75) };
            clamp_rect((x0, b.1 - nh * top, x1, b.3 + nh * bottom), img.w, img.h)
        };
        let tries = [(0.15, 0.15, 0.25), (0.3, 0.3, 0.3), (0.05, 0.1, 0.2), (0.5, 0.2, 0.3), (0.15, 0.4, 0.4)];
        let strip = tries
            .iter()
            .map(|&(r, t, bt)| cand(r, t, bt))
            .find(|&r| crate::quick_count(ocr, &crop_bgr(img, r.0 as usize, r.1 as usize, (r.0 + r.2) as usize, (r.1 + r.3) as usize)) == Some(n))
            .unwrap_or_else(|| cand(0.15, 0.15, 0.25));
        let near = |r: Rect| ((r.0 - qr.0).abs() as f32) < cs * 0.5 && ((r.1 - qr.1).abs() as f32) < cs * 0.5;
        let p = Pair { qs: Cell { rect: strip, count: n }, bag: c.clone(), cell: qr, sure };
        match pairs.iter().position(|q| near(q.cell)) {
            None => pairs.push(p),
            Some(j) if sure && !pairs[j].sure => pairs[j] = p,
            Some(_) => {}
        }
    }
    if pairs.len() >= 3 {
        let dx = median(pairs.iter().map(|p| (p.qs.rect.0 - p.cell.0) as f32).collect()) as i32;
        let w = median(pairs.iter().map(|p| p.qs.rect.2 as f32).collect()) as i32;
        for p in pairs.iter_mut() {
            if ((p.qs.rect.0 - p.cell.0 - dx) as f32).abs() > nh * 0.5 {
                if debug {
                    eprintln!("qs {} 數字那一條偏了 {:?} → x {}", p.qs.count, p.qs.rect, p.cell.0 + dx);
                }
                p.qs.rect.0 = p.cell.0 + dx;
                p.qs.rect.2 = w;
            }
        }
    }
    let (Some(first), gx, gy) = (pairs.first().map(|p| p.cell), pxl.unwrap_or(px), pyb.unwrap_or(py)) else { return pairs };
    let off = (pairs.iter().map(|p| p.qs.rect.0 - p.cell.0).min().unwrap_or(0), pairs.iter().map(|p| p.qs.rect.1 - p.cell.1).max().unwrap_or(0));
    let (sw, sh) = (pairs.iter().map(|p| p.qs.rect.2).max().unwrap_or(0), pairs.iter().map(|p| p.qs.rect.3).max().unwrap_or(0));
    let at = |r: Rect| (((r.0 - first.0) as f32 / gx).round() as i32, ((r.1 - first.1) as f32 / gy).round() as i32);
    let idx: Vec<(i32, i32)> = pairs.iter().map(|p| at(p.cell)).collect();
    let (cmin, cmax) = (idx.iter().map(|i| i.0).min().unwrap_or(0), idx.iter().map(|i| i.0).max().unwrap_or(0));
    let (rmin, rmax) = (idx.iter().map(|i| i.1).min().unwrap_or(0), idx.iter().map(|i| i.1).max().unwrap_or(0));
    let cols = if cmax - cmin >= 3 { cmin..=cmax } else { cmax - 3..=cmin + 3 };
    let rows = if rmax > rmin { rmin..=rmax } else { rmin - 1..=rmin + 1 };
    for r in rows {
        for c in cols.clone() {
            {
                if idx.contains(&(c, r)) {
                    continue;
                }
                let (l, t) = (first.0 as f32 + c as f32 * gx, first.1 as f32 + r as f32 * gy);
                if l < x0 || l + first.2 as f32 > img.w as f32 || t < y0 || t + first.3 as f32 > y1 {
                    continue;
                }
                let qr = (l.round() as i32, t.round() as i32, first.2, first.3);
                let qc = cell_img(img, qr);
                let dark = qc.data.chunks_exact(3).filter(|p| p.iter().all(|&c| c < 90)).count() as f32 / (qc.w * qc.h) as f32;
                if !(0.07..0.8).contains(&dark) {
                    continue;
                }
                let qsig = sig(img, qr);
                let dl: Vec<f32> = imgs.iter().map(|c| look(&qc, c)).collect();
                let mut gs: Vec<(usize, f32, f32)> = vec![];
                for (i, &g) in group.iter().enumerate() {
                    let h = same_color(&qsig, &sigs[i]);
                    match gs.iter_mut().find(|x| x.0 == g) {
                        Some(x) => {
                            x.1 = x.1.min(dl[i]);
                            x.2 = x.2.min(h);
                        }
                        None => gs.push((g, dl[i], h)),
                    }
                }
                gs.sort_by(|a, b| a.1.total_cmp(&b.1));
                let Some(&(g1, d1, h1)) = gs.first() else { continue };
                let clear = d1 < 0.22 && gs.iter().skip(1).all(|g2| g2.1 - d1 >= 0.06 || (g2.1 - d1 >= 0.03 && g2.2 - h1 >= 0.3));
                if debug {
                    eprintln!("qs icon {qr:?} groups {:?} clear {clear}", gs.iter().take(3).map(|x| (x.0, (x.1 * 100.0).round() / 100.0, (x.2 * 10.0).round() / 10.0)).collect::<Vec<_>>());
                }
                if !clear {
                    continue;
                }
                let Some(c) = (0..cells.len()).filter(|&i| group[i] == g1).min_by(|&a, &b| dl[a].total_cmp(&dl[b])).map(|i| &cells[i]) else { continue };
                let strip = clamp_rect(((qr.0 + off.0) as f32, (qr.1 + off.1) as f32, (qr.0 + off.0 + sw) as f32, (qr.1 + off.1 + sh) as f32), img.w, img.h);
                pairs.push(Pair { qs: Cell { rect: strip, count: c.count }, bag: c.clone(), cell: qr, sure: true });
            }
        }
    }
    pairs
}

fn rec_zoom(ocr: &mut Ocr, img: &Bgr, r: (f32, f32, f32, f32), scale: f64) -> String {
    let (x0, y0, x1, y1) = (r.0.max(0.0) as usize, r.1.max(0.0) as usize, (r.2 as usize).min(img.w), (r.3 as usize).min(img.h));
    if x1 <= x0 + 2 || y1 <= y0 + 2 {
        return String::new();
    }
    let crop = crop_bgr(img, x0, y0, x1, y1);
    let big = resize_linear(&crop, (crop.w as f64 * scale) as usize, (crop.h as f64 * scale) as usize);
    ocr.run(&big, false, false).ok().and_then(|v| v.into_iter().next()).map_or(String::new(), |it| it.text)
}

fn gap_count(ocr: &mut Ocr, img: &Bgr, strip: (f32, f32, f32, f32)) -> Option<i64> {
    let rec = number(&rec_zoom(ocr, img, strip, 3.0));
    let det: Vec<i64> = read_region(ocr, img, (strip.0.max(0.0) as usize, strip.1.max(0.0) as usize, strip.2 as usize, strip.3 as usize), 3.0)
        .iter()
        .filter_map(|(it, _)| number(&it.text))
        .collect();
    let det = (det.len() == 1).then(|| det[0]);
    let n = match (rec, det) {
        (Some(r), Some(d)) if r != d && r.to_string().starts_with(&d.to_string()) => Some(d),
        _ => rec.or(det),
    };
    let n = n.or_else(|| quick(ocr, img, strip));
    if std::env::var_os("AUTO_DEBUG").is_some() {
        eprintln!("  gap ({:.0},{:.0}) rec {rec:?} det {det:?} -> {n:?}", strip.0, strip.3);
    }
    n
}

fn quick(ocr: &mut Ocr, img: &Bgr, r: (f32, f32, f32, f32)) -> Option<i64> {
    let r = clamp_rect(r, img.w, img.h);
    if r.2 < 4 || r.3 < 4 {
        return None;
    }
    crate::quick_count(ocr, &crop_bgr(img, r.0 as usize, r.1 as usize, (r.0 + r.2) as usize, (r.1 + r.3) as usize))
}

fn median(mut v: Vec<f32>) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

pub fn same_color(a: &[f32], b: &[f32]) -> f32 {
    let (sa, sb): (f32, f32) = (a.iter().sum(), b.iter().sum());
    if sa < 0.01 || sb < 0.01 {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| (x / sa - y / sb).abs()).sum()
}

pub fn look(a: &Bgr, b: &Bgr) -> f32 {
    const N: i32 = 40;
    if a.w < 8 || a.h < 8 || b.w < 8 || b.h < 8 {
        return 1.0;
    }
    let (ia, ib) = (resize_linear(a, N as usize, N as usize), resize_linear(b, N as usize, N as usize));
    let seen = |x: i32, y: i32| (y >= N * 46 / 100 && y < N * 70 / 100) || (x >= N * 78 / 100 && y < N * 70 / 100);
    let mut best = f32::MAX;
    for dy in -5i32..=5 {
        for dx in -5i32..=5 {
            let (mut sum, mut cnt) = (0f32, 0f32);
            for y in 5..N - 5 {
                for x in 5..N - 5 {
                    if !seen(x, y) {
                        continue;
                    }
                    let p = ((y * N + x) * 3) as usize;
                    let q = (((y + dy) * N + x + dx) * 3) as usize;
                    sum += (0..3).map(|k| (ia.data[p + k] as f32 - ib.data[q + k] as f32).abs()).sum::<f32>() / 765.0;
                    cnt += 1.0;
                }
            }
            best = best.min(sum / cnt.max(1.0));
        }
    }
    best
}

pub fn cell_img(img: &Bgr, r: Rect) -> Bgr {
    let (x0, y0) = (r.0.max(0) as usize, r.1.max(0) as usize);
    crop_bgr(img, x0, y0, ((r.0 + r.2) as usize).min(img.w).max(x0 + 1), ((r.1 + r.3) as usize).min(img.h).max(y0 + 1))
}

pub fn sig(img: &Bgr, r: Rect) -> Vec<f32> {
    let (x0, y0) = (r.0.max(0) as usize, r.1.max(0) as usize);
    let (x1, y1) = (((r.0 + r.2) as usize).min(img.w), ((r.1 + r.3 * 6 / 10) as usize).min(img.h));
    if x1 <= x0 || y1 <= y0 {
        return vec![0.0; 12];
    }
    qs_sig(&crop_bgr(img, x0, y0, x1, y1))
}
