//! Astro-style frame calibration (dark + flat), registration and stacking of a
//! fingerprint "video" into one larger, cleaner image.

use super::fft::{Fft2, freq, xcorr};
use super::img::{Img, blur, blur_x, median, percentile};

/// Per-pixel sensor calibration: `dark` is the fixed pattern with a finger on the
/// sensor (median of moving-finger frames), `flat` the relative pixel gain.
#[derive(Clone, Debug)]
pub struct Calib {
    pub dark: Img,
    pub flat: Img,
}

impl Calib {
    pub fn apply(&self, f: &Img) -> Img {
        Img::from_fn(f.w, f.h, |x, y| (f.get(x, y) - self.dark.get(x, y)) / self.flat.get(x, y) + 128.0)
    }

    /// Needs a video where the finger moves; returns None with too few finger frames.
    pub fn estimate(proc: &FrameProc, frames: &[Img]) -> Option<Calib> {
        let energy: Vec<f32> = frames.iter().map(|f| FrameProc::energy(&proc.ridges(f, None)).mean()).collect();
        let thr = 0.25 * percentile(&energy, 95.0);
        let fin: Vec<&Img> = frames.iter().zip(&energy).filter(|(_, e)| **e > thr).map(|(f, _)| f).collect();
        if fin.len() < 20 {
            return None;
        }
        let (w, h) = (fin[0].w, fin[0].h);
        let per_pixel = |f: &dyn Fn(usize) -> f32| -> Img {
            let mut out = Img::new(w, h);
            let mut col = vec![0.0f32; fin.len()];
            for i in 0..w * h {
                for (k, v) in col.iter_mut().enumerate() {
                    *v = f(k * w * h + i);
                }
                out.px[i] = median(&col);
            }
            out
        };
        let stack: Vec<f32> = fin.iter().flat_map(|f| f.px.iter().copied()).collect();
        let dark_raw = per_pixel(&|j| stack[j]);
        let resid: Vec<f32> = stack.iter().enumerate().map(|(j, v)| v - dark_raw.px[j % (w * h)]).collect();
        let rmed = per_pixel(&|j| resid[j]);
        let mad = per_pixel(&|j| (resid[j] - rmed.px[j % (w * h)]).abs());
        let dark = blur_x(&dark_raw, 6.0);
        let flat_raw = blur_x(&mad, 6.0);
        let m = median(&flat_raw.px).max(1e-6);
        let flat = flat_raw.map(|v| (v / m).clamp(0.5, 2.0));
        Some(Calib { dark, flat })
    }
}

/// Per-frame processing for one sensor geometry.
pub struct FrameProc {
    pub w: usize,
    pub h: usize,
    fft: Fft2,
    bp: Vec<f32>,
    pub win: Img,
    xfft: Fft2,
}

impl FrameProc {
    /// `kill_rows` removes everything constant along a row (sensor streaks) when no
    /// dark frame is available; with a calibration it is not needed.
    pub fn new(w: usize, h: usize, kill_rows: bool) -> Self {
        let mut bp = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let (fy, fx) = (freq(y, h), freq(x, w));
                let r = fy.hypot(fx);
                let keep = r > 1.0 / 24.0 && r < 1.0 / 3.0 && !(kill_rows && fx.abs() < 1.5 / w as f32);
                bp[y * w + x] = keep as u8 as f32;
            }
        }
        let han = |n: usize, i: usize| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos();
        Self { w, h, fft: Fft2::new(w, h), bp, win: Img::from_fn(w, h, |x, y| han(w, x) * han(h, y)), xfft: Fft2::new(2 * w, 2 * h) }
    }

    /// Ridge-band image: calibration, row-median removal, band-pass.
    pub fn ridges(&self, f: &Img, calib: Option<&Calib>) -> Img {
        let f = match calib {
            Some(c) => c.apply(f),
            None => f.clone(),
        };
        let mut a = f.clone();
        for y in 0..f.h {
            let row = &f.px[y * f.w..(y + 1) * f.w];
            let m = median(row);
            for x in 0..f.w {
                a.px[y * f.w + x] = row[x] - m;
            }
        }
        let mut d = self.fft.of(&a);
        for (v, &k) in d.iter_mut().zip(&self.bp) {
            *v *= k;
        }
        self.fft.inverse(&mut d);
        self.fft.real(&d)
    }

    pub fn energy(ridges: &Img) -> Img {
        blur(&ridges.map(|v| v * v), 6.0)
    }

    pub fn finger_mask(energy: &Img, thr: f32) -> Vec<bool> {
        let raw = energy.map(|e| (e > thr) as u8 as f32);
        blur(&raw, 2.0).px.iter().map(|&v| v > 0.5).collect()
    }

    pub fn xcorr(&self, a: &Img, b: &Img) -> (isize, isize, f32) {
        xcorr(&self.xfft, a, b)
    }
}

pub struct Placed {
    pub pos: (isize, isize),
    pub ridges: Img,
    pub weight: Img,
}

pub struct StitchParams {
    pub min_cover: f32,
    pub min_conf: f32,
    pub max_step: isize,
    pub reacquire_conf: f32,
}

impl Default for StitchParams {
    fn default() -> Self {
        Self { min_cover: 0.35, min_conf: 0.25, max_step: 40, reacquire_conf: 0.45 }
    }
}

pub struct Stitch {
    pub with_finger: usize,
    pub placed: Vec<Placed>,
    /// Energy threshold that separates finger from background in this video.
    pub energy_thr: f32,
}

/// Register a video frame by frame. While the finger stays down each frame follows the
/// previous one; after a lift a new press must clearly overlap one of the recent frames.
pub fn stitch(proc: &FrameProc, frames: &[Img], calib: Option<&Calib>, p: &StitchParams) -> Stitch {
    let prepped: Vec<(Img, Img)> = frames
        .iter()
        .map(|f| {
            let r = proc.ridges(f, calib);
            let e = FrameProc::energy(&r);
            (r, e)
        })
        .collect();
    let all: Vec<f32> = prepped.iter().flat_map(|(_, e)| e.px.iter().copied()).collect();
    let thr = 0.2 * percentile(&all, 98.0);
    let mut items = Vec::new();
    for (k, (r, e)) in prepped.into_iter().enumerate() {
        let mask = FrameProc::finger_mask(&e, thr);
        let cover = mask.iter().filter(|&&m| m).count() as f32 / mask.len() as f32;
        if cover >= p.min_cover {
            items.push((k, r, mask));
        }
    }
    let with_finger = items.len();
    let mut placed: Vec<Placed> = Vec::new();
    let mut last_k = 0usize;
    for (k, ridges, mask) in items {
        let weight = Img::from_fn(proc.w, proc.h, |x, y| mask[y * proc.w + x] as u8 as f32 * proc.win.get(x, y));
        let cand = ridges.zip(&weight, |a, b| a * b);
        let pos = if placed.is_empty() {
            (0, 0)
        } else if k - last_k <= 2 {
            let prev = placed.last().unwrap();
            let (dy, dx, conf) = proc.xcorr(&prev.ridges.zip(&prev.weight, |a, b| a * b), &cand);
            if conf < p.min_conf || dy.abs().max(dx.abs()) > p.max_step {
                continue;
            }
            (prev.pos.0 - dy, prev.pos.1 - dx)
        } else {
            let start = placed.len().saturating_sub(12);
            let best = placed[start..]
                .iter()
                .map(|q| {
                    let (dy, dx, c) = proc.xcorr(&q.ridges.zip(&q.weight, |a, b| a * b), &cand);
                    (c, (q.pos.0 - dy, q.pos.1 - dx))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                .unwrap();
            if best.0 < p.reacquire_conf {
                continue;
            }
            best.1
        };
        placed.push(Placed { pos, ridges, weight });
        last_k = k;
    }
    Stitch { with_finger, placed, energy_thr: thr }
}

#[derive(Clone, Debug)]
pub struct Mosaic {
    pub img: Img,
    pub mask: Vec<bool>,
}

impl Mosaic {
    pub fn area(&self) -> usize {
        self.mask.iter().filter(|&&m| m).count()
    }
}

/// Weighted mean of the registered frames with optional sigma clipping (two passes:
/// mean/std, then drop samples more than `clip` standard deviations away).
pub fn stack(placed: &[Placed], clip: Option<f32>) -> Mosaic {
    let (fw, fh) = (placed[0].ridges.w, placed[0].ridges.h);
    let y0 = placed.iter().map(|p| p.pos.0).min().unwrap();
    let x0 = placed.iter().map(|p| p.pos.1).min().unwrap();
    let h = (placed.iter().map(|p| p.pos.0).max().unwrap() - y0) as usize + fh;
    let w = (placed.iter().map(|p| p.pos.1).max().unwrap() - x0) as usize + fw;
    let accumulate = |lo: Option<(&Img, &Img)>, k: f32| -> (Img, Img, Img) {
        let (mut s, mut s2, mut ws) = (Img::new(w, h), Img::new(w, h), Img::new(w, h));
        for p in placed {
            let (oy, ox) = ((p.pos.0 - y0) as usize, (p.pos.1 - x0) as usize);
            for y in 0..fh {
                for x in 0..fw {
                    let wt = p.weight.get(x, y);
                    if wt <= 0.05 {
                        continue;
                    }
                    let v = p.ridges.get(x, y);
                    let i = (oy + y) * w + ox + x;
                    if let Some((mean, sd)) = lo
                        && (v - mean.px[i]).abs() > k * sd.px[i].max(1e-6)
                    {
                        continue;
                    }
                    s.px[i] += wt * v;
                    s2.px[i] += wt * v * v;
                    ws.px[i] += wt;
                }
            }
        }
        (s, s2, ws)
    };
    let (mut s, s2, mut ws) = accumulate(None, 0.0);
    if let Some(k) = clip {
        let mean = s.zip(&ws, |a, b| if b > 0.0 { a / b } else { 0.0 });
        let var = s2.zip(&ws, |a, b| if b > 0.0 { a / b } else { 0.0 }).zip(&mean, |e2, m| (e2 - m * m).max(0.0));
        let sd = var.map(f32::sqrt);
        let again = accumulate(Some((&mean, &sd)), k);
        s = again.0;
        ws = again.2;
    }
    let mask: Vec<bool> = ws.px.iter().map(|&v| v > 0.05).collect();
    let img = s.zip(&ws, |a, b| if b > 0.05 { a / b } else { 0.0 });
    Mosaic { img, mask }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic whorl print, a 112x112 window sliding along a known path, with row
    /// streaks and noise: positions must come back exactly.
    #[test]
    fn stitch_recovers_known_path() {
        let big = Img::from_fn(300, 300, |x, y| {
            let (dx, dy) = (x as f32 - 140.0, y as f32 - 150.0);
            let th = dy.atan2(dx);
            128.0 + 40.0 * ((dx.hypot(dy)) / 1.4 + 2.5 * (2.0 * th).sin() + 0.03 * x as f32).sin()
        });
        let path: Vec<(usize, usize)> = (0..40).map(|t| ((40.0 + 30.0 * (t as f32 / 6.0).sin()) as usize, 40 + (2.2 * t as f32) as usize)).collect();
        let mut seed = 12345u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 12.0 - 6.0
        };
        let streak: Vec<f32> = (0..112).map(|_| noise() * 1.5).collect();
        let frames: Vec<Img> = path
            .iter()
            .map(|&(y, x)| {
                let mut f = big.crop(x, y, 112, 112);
                for yy in 0..112 {
                    for xx in 0..112 {
                        f.px[yy * 112 + xx] += streak[yy] + noise();
                    }
                }
                f
            })
            .collect();
        let proc = FrameProc::new(112, 112, true);
        let st = stitch(&proc, &frames, None, &StitchParams::default());
        assert_eq!(st.placed.len(), path.len());
        for (p, &(y, x)) in st.placed.iter().zip(&path) {
            assert_eq!(p.pos, (y as isize - path[0].0 as isize, x as isize - path[0].1 as isize));
        }
        let m = stack(&st.placed, Some(2.5));
        assert!(m.area() * 2 > 3 * 112 * 112, "mosaic area {}", m.area());
    }
}
