#[cfg(windows)]
pub mod capture;
#[cfg(target_os = "macos")]
#[path = "capture_mac.rs"]
pub mod capture;
pub mod det;
pub mod img;

use img::Bgr;
use ndarray::{Array4, ArrayViewD, Axis};
use ort::session::Session;
use ort::value::TensorRef;
use std::path::Path;

pub use img::Bgr as Image;

const TEXT_SCORE: f32 = 0.5;
const MIN_HEIGHT: usize = 30;
const WH_RATIO: f32 = 8.0;
const MAX_SIDE: usize = 2000;
const MIN_SIDE: usize = 30;
const DET_LIMIT: usize = 960;
const REC_H: usize = 48;
const REC_W: usize = 320;
const CLS_W: usize = 192;
const CLS_THRESH: f32 = 0.9;
const BATCH: usize = 6;

#[derive(Debug, Clone)]
pub struct Item {
    pub quad: Option<[[f32; 2]; 4]>,
    pub text: String,
    pub score: f32,
}

pub struct Ocr {
    det: Session,
    cls: Session,
    rec: Session,
    keys: Vec<String>,
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn session(p: &Path) -> Result<Session> {
    let e = |e: ort::Error<ort::session::builder::SessionBuilder>| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() };
    Ok(Session::builder()?.with_intra_threads(2).map_err(e)?.with_inter_threads(1).map_err(e)?.commit_from_file(p)?)
}

fn round32(v: usize) -> usize {
    ((v as f64 / 32.0).round_ties_even() * 32.0) as usize
}

fn resize32(img: &Bgr, ratio: f64) -> Result<(Bgr, f64, f64)> {
    let (nh, nw) = (round32((img.h as f64 * ratio) as usize), round32((img.w as f64 * ratio) as usize));
    if nw == 0 || nh == 0 {
        return Err("resize to 0".into());
    }
    Ok((img::resize_linear(img, nw, nh), img.h as f64 / nh as f64, img.w as f64 / nw as f64))
}

impl Ocr {
    pub fn new(dll: &Path, models: &Path) -> Result<Self> {
        let _ = ort::init_from(dll)?.commit();
        let keys_txt = std::fs::read_to_string(models.join("keys.txt"))?;
        let mut keys: Vec<String> = vec!["blank".into()];
        keys.extend(keys_txt.split('\n').map(|s| s.to_string()));
        keys.push(" ".into());
        Ok(Ocr {
            det: session(&models.join("ch_PP-OCRv4_det_infer.onnx"))?,
            cls: session(&models.join("ch_ppocr_mobile_v2.0_cls_infer.onnx"))?,
            rec: session(&models.join("ch_PP-OCRv4_rec_infer.onnx"))?,
            keys,
        })
    }

    pub fn run(&mut self, src: &Bgr, use_det: bool, use_cls: bool) -> Result<Vec<Item>> {
        let (raw_w, raw_h) = (src.w, src.h);
        let mut img = src.clone();
        let (mut rh, mut rw) = (1.0, 1.0);
        if img.w.max(img.h) > MAX_SIDE {
            let r = MAX_SIDE as f64 / img.w.max(img.h) as f64;
            (img, rh, rw) = resize32(&img, r)?;
        }
        if img.w.min(img.h) < MIN_SIDE {
            let r = MIN_SIDE as f64 / img.w.min(img.h) as f64;
            (img, rh, rw) = resize32(&img, r)?;
        }
        let mut quads = None;
        let mut crops = vec![img.clone()];
        if use_det {
            let mut top = 0usize;
            if img.h <= MIN_HEIGHT || img.w as f32 / img.h as f32 > WH_RATIO {
                let new_h = ((img.w as f32 / WH_RATIO) as usize).max(MIN_HEIGHT) * 2;
                top = (new_h as i64 - img.h as i64).unsigned_abs() as usize / 2;
                img = img.pad_tb(top);
            }
            let boxes = self.detect(&img)?;
            if boxes.is_empty() {
                return Ok(vec![]);
            }
            let boxes = det::sort_boxes(boxes);
            crops = boxes.iter().map(|b| crop(&img, b)).collect();
            quads = Some(
                boxes
                    .iter()
                    .map(|b| {
                        let mut q = *b;
                        for p in q.iter_mut() {
                            p[1] -= top as f32;
                            p[0] = (p[0] as f64 * rw) as f32;
                            p[1] = (p[1] as f64 * rh) as f32;
                            p[0] = p[0].max(0.0).min(raw_w as f32);
                            p[1] = p[1].max(0.0).min(raw_h as f32);
                        }
                        q
                    })
                    .collect::<Vec<_>>(),
            );
        }
        if use_cls {
            crops = self.classify(crops)?;
        }
        if let Ok(dir) = std::env::var("OCR_DUMP") {
            for (i, c) in crops.iter().enumerate() {
                let mut rgb = image::RgbImage::new(c.w as u32, c.h as u32);
                for (d, s) in rgb.pixels_mut().zip(c.data.chunks_exact(3)) {
                    *d = image::Rgb([s[2], s[1], s[0]]);
                }
                let _ = rgb.save(std::path::Path::new(&dir).join(format!("crop{i}.png")));
            }
        }
        let rec = self.recognize(&crops)?;
        Ok(match quads {
            None => rec.into_iter().map(|(text, score)| Item { quad: None, text, score }).collect(),
            Some(q) => q
                .into_iter()
                .zip(rec)
                .filter(|(_, (_, s))| *s >= TEXT_SCORE)
                .map(|(q, (text, score))| Item { quad: Some(q), text, score })
                .collect(),
        })
    }

    fn detect(&mut self, img: &Bgr) -> Result<Vec<[[f32; 2]; 4]>> {
        let m = img.w.max(img.h);
        let limit = if m < 960 { DET_LIMIT } else if m < 1500 { 1500 } else { 2000 };
        let ratio = if m > limit { limit as f64 / m as f64 } else { 1.0 };
        let (nh, nw) = (round32((img.h as f64 * ratio) as usize), round32((img.w as f64 * ratio) as usize));
        if nw == 0 || nh == 0 {
            return Ok(vec![]);
        }
        let r = img::resize_linear(img, nw, nh);
        let mut x = Array4::<f32>::zeros((1, 3, nh, nw));
        img::to_chw(&r, nw, x.as_slice_mut().unwrap());
        let out = self.det.run(ort::inputs![TensorRef::from_array_view(&x)?])?;
        let pred: ArrayViewD<f32> = out[0].try_extract_array::<f32>()?;
        let (ph, pw) = (pred.shape()[2], pred.shape()[3]);
        let p: Vec<f32> = pred.iter().copied().collect();
        let boxes = det::boxes_from_pred(&p, pw, ph, img.w, img.h);
        Ok(det::filter_boxes(boxes, img.w, img.h))
    }

    fn classify(&mut self, mut crops: Vec<Bgr>) -> Result<Vec<Bgr>> {
        let order = argsort_ratio(&crops);
        for chunk in order.chunks(BATCH) {
            let mut x = Array4::<f32>::zeros((chunk.len(), 3, REC_H, CLS_W));
            for (b, &i) in chunk.iter().enumerate() {
                let im = &crops[i];
                let rw = ((REC_H as f64 * (im.w as f64 / im.h as f64)).ceil() as usize).min(CLS_W);
                let r = img::resize_linear(im, rw, REC_H);
                let mut slot = x.index_axis_mut(Axis(0), b);
                img::to_chw(&r, CLS_W, slot.as_slice_mut().unwrap());
            }
            let out = self.cls.run(ort::inputs![TensorRef::from_array_view(&x)?])?;
            let prob = out[0].try_extract_array::<f32>()?;
            for (b, &i) in chunk.iter().enumerate() {
                let (p0, p1) = (prob[[b, 0]], prob[[b, 1]]);
                if p1 > p0 && p1 > CLS_THRESH {
                    crops[i] = crops[i].rot180();
                }
            }
        }
        Ok(crops)
    }

    fn recognize(&mut self, crops: &[Bgr]) -> Result<Vec<(String, f32)>> {
        let order = argsort_ratio(crops);
        let mut res = vec![(String::new(), 0.0f32); crops.len()];
        for chunk in order.chunks(BATCH) {
            let mut max_ratio = REC_W as f64 / REC_H as f64;
            for &i in chunk {
                max_ratio = max_ratio.max(crops[i].w as f64 / crops[i].h as f64);
            }
            let width = (REC_H as f64 * max_ratio) as usize;
            let mut x = Array4::<f32>::zeros((chunk.len(), 3, REC_H, width));
            for (b, &i) in chunk.iter().enumerate() {
                let im = &crops[i];
                let rw = ((REC_H as f64 * (im.w as f64 / im.h as f64)).ceil() as usize).min(width);
                let r = img::resize_linear(im, rw, REC_H);
                let mut slot = x.index_axis_mut(Axis(0), b);
                img::to_chw(&r, width, slot.as_slice_mut().unwrap());
            }
            let out = self.rec.run(ort::inputs![TensorRef::from_array_view(&x)?])?;
            let prob = out[0].try_extract_array::<f32>()?;
            for (b, &i) in chunk.iter().enumerate() {
                let seq = prob.index_axis(Axis(0), b);
                let (mut text, mut confs, mut last) = (String::new(), Vec::new(), usize::MAX);
                for t in seq.axis_iter(Axis(0)) {
                    let (mut k, mut best) = (0usize, f32::MIN);
                    for (j, &v) in t.iter().enumerate() {
                        if v > best {
                            best = v;
                            k = j;
                        }
                    }
                    if k != last && k != 0 {
                        text.push_str(&self.keys[k]);
                        confs.push(best);
                    }
                    last = k;
                }
                let score = if confs.is_empty() { 0.0 } else { confs.iter().map(|&c| c as f64).sum::<f64>() / confs.len() as f64 };
                res[i] = (text, score as f32);
            }
        }
        Ok(res)
    }
}

fn argsort_ratio(imgs: &[Bgr]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..imgs.len()).collect();
    idx.sort_by(|&a, &b| {
        let ra = imgs[a].w as f32 / imgs[a].h as f32;
        let rb = imgs[b].w as f32 / imgs[b].h as f32;
        ra.partial_cmp(&rb).unwrap()
    });
    idx
}

fn crop(img: &Bgr, b: &[[f32; 2]; 4]) -> Bgr {
    let d = |a: [f32; 2], c: [f32; 2]| (((a[0] - c[0]) as f64).powi(2) + ((a[1] - c[1]) as f64).powi(2)).sqrt();
    let w = d(b[0], b[1]).max(d(b[2], b[3])) as usize;
    let h = d(b[0], b[3]).max(d(b[1], b[2])) as usize;
    let c = img::warp_crop(img, b, w.max(1), h.max(1));
    if c.h as f32 / c.w as f32 >= 1.5 { c.rot90() } else { c }
}
