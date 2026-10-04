//! Minutiae extraction: ridge period, orientation field, Gabor enhancement,
//! binarisation, Zhang-Suen thinning, crossing-number detection, direction by
//! skeleton tracing, and removal of the usual false minutiae.

use std::f32::consts::{PI, TAU};

use super::fft::{Fft2, freq};
use super::img::{Img, blur, erode, percentile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ending,
    Bifurcation,
}

#[derive(Clone, Copy, Debug)]
pub struct Minutia {
    pub x: f32,
    pub y: f32,
    /// Direction in [0, 2pi): away from the ridge for endings, along the stem for
    /// bifurcations (so a ridge/valley polarity flip keeps the direction).
    pub dir: f32,
    pub kind: Kind,
    pub quality: f32,
}

#[derive(Clone, Debug)]
pub struct Features {
    pub w: usize,
    pub h: usize,
    pub minutiae: Vec<Minutia>,
    /// Foreground (usable fingerprint area) after the border margin.
    pub mask: Vec<bool>,
    pub period: f32,
}

#[derive(Clone, Debug)]
pub struct Params {
    /// Orientation smoothing, in ridge periods.
    pub orient_sigma: f32,
    pub min_coherence: f32,
    /// Minutiae closer than this many periods to each other are both dropped.
    pub min_pair_dist: f32,
    /// Border margin, in periods.
    pub margin: f32,
    /// Drop spurs and short ridges found by tracing.
    pub spur_filter: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self { orient_sigma: 1.0, min_coherence: 0.35, min_pair_dist: 0.8, margin: 1.5, spur_filter: false }
    }
}

/// Dominant ridge period (pixels) from the radial power spectrum of the masked image.
pub fn ridge_period(img: &Img, mask: &[bool]) -> f32 {
    let mean = {
        let (s, n) = img.px.iter().zip(mask).filter(|(_, m)| **m).fold((0.0, 0usize), |(s, n), (v, _)| (s + v, n + 1));
        s / n.max(1) as f32
    };
    let a = Img { w: img.w, h: img.h, px: img.px.iter().zip(mask).map(|(&v, &m)| if m { v - mean } else { 0.0 }).collect() };
    let f = Fft2::new(img.w, img.h);
    let d = f.of(&a);
    let bins = 64;
    let (lo, hi) = (1.0 / 25.0f32, 1.0 / 3.5f32);
    let mut acc = vec![0.0f32; bins];
    for y in 0..img.h {
        for x in 0..img.w {
            let r = freq(y, img.h).hypot(freq(x, img.w));
            if r > lo && r < hi {
                let b = (((r - lo) / (hi - lo)) * bins as f32) as usize;
                acc[b.min(bins - 1)] += d[y * img.w + x].norm_sqr();
            }
        }
    }
    // light smoothing of the radial profile, then the peak
    let sm: Vec<f32> = (0..bins).map(|i| (i.saturating_sub(1)..=(i + 1).min(bins - 1)).map(|j| acc[j]).sum()).collect();
    let peak = sm.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i).unwrap_or(bins / 2);
    1.0 / (lo + (peak as f32 + 0.5) / bins as f32 * (hi - lo))
}

/// Ridge orientation (radians in [0, pi), along the ridges) and coherence in [0, 1].
pub fn orientation(img: &Img, sigma: f32) -> (Img, Img) {
    let s = blur(img, 1.0);
    let (w, h) = (img.w, img.h);
    let (mut gxx, mut gxy, mut gm) = (Img::new(w, h), Img::new(w, h), Img::new(w, h));
    for y in 0..h as isize {
        for x in 0..w as isize {
            let p = |dx: isize, dy: isize| s.get_clamped(x + dx, y + dy);
            let gx = (p(1, -1) + 2.0 * p(1, 0) + p(1, 1)) - (p(-1, -1) + 2.0 * p(-1, 0) + p(-1, 1));
            let gy = (p(-1, 1) + 2.0 * p(0, 1) + p(1, 1)) - (p(-1, -1) + 2.0 * p(0, -1) + p(1, -1));
            let i = y as usize * w + x as usize;
            gxx.px[i] = gx * gx - gy * gy;
            gxy.px[i] = 2.0 * gx * gy;
            gm.px[i] = gx * gx + gy * gy;
        }
    }
    let (gxx, gxy, gm) = (blur(&gxx, sigma), blur(&gxy, sigma), blur(&gm, sigma));
    let mut th = Img::new(w, h);
    let mut coh = Img::new(w, h);
    for i in 0..w * h {
        let t = 0.5 * gxy.px[i].atan2(gxx.px[i]) + PI / 2.0;
        th.px[i] = t.rem_euclid(PI);
        coh.px[i] = (gxx.px[i].hypot(gxy.px[i]) / (gm.px[i] + 1e-9)).clamp(0.0, 1.0);
    }
    (th, coh)
}

/// Oriented Gabor filtering with a bank of `n` quantised orientations.
pub fn gabor(img: &Img, theta: &Img, mask: &[bool], period: f32) -> Img {
    let n = 16;
    let (sa, sl) = (0.5 * period, 0.6 * period);
    let r = (3.0 * sa.max(sl)).ceil() as isize;
    let side = (2 * r + 1) as usize;
    let bank: Vec<Vec<f32>> = (0..n)
        .map(|k| {
            let ridge = k as f32 * PI / n as f32;
            let (c, s) = ((ridge + PI / 2.0).cos(), (ridge + PI / 2.0).sin()); // across-ridge axis
            let mut kern = Vec::with_capacity(side * side);
            for dy in -r..=r {
                for dx in -r..=r {
                    let u = dx as f32 * c + dy as f32 * s;
                    let v = -dx as f32 * s + dy as f32 * c;
                    kern.push((-(u * u / (sa * sa) + v * v / (sl * sl)) / 2.0).exp() * (TAU * u / period).cos());
                }
            }
            let m = kern.iter().sum::<f32>() / kern.len() as f32;
            kern.iter_mut().for_each(|v| *v -= m);
            kern
        })
        .collect();
    let mut out = Img::new(img.w, img.h);
    for y in 0..img.h {
        for x in 0..img.w {
            let i = y * img.w + x;
            if !mask[i] {
                continue;
            }
            let k = &bank[((theta.px[i] / PI * n as f32).round() as usize) % n];
            let mut acc = 0.0;
            let mut j = 0;
            for dy in -r..=r {
                for dx in -r..=r {
                    acc += k[j] * img.get_clamped(x as isize + dx, y as isize + dy);
                    j += 1;
                }
            }
            out.px[i] = acc;
        }
    }
    out
}

/// Zhang-Suen thinning of a binary image (in place).
pub fn thin(b: &mut [bool], w: usize, h: usize) {
    let at = |b: &[bool], x: usize, y: usize| b[y * w + x] as u8;
    loop {
        let mut changed = false;
        for step in 0..2 {
            let mut kill = Vec::new();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    if !b[y * w + x] {
                        continue;
                    }
                    let p = [
                        at(b, x, y - 1),
                        at(b, x + 1, y - 1),
                        at(b, x + 1, y),
                        at(b, x + 1, y + 1),
                        at(b, x, y + 1),
                        at(b, x - 1, y + 1),
                        at(b, x - 1, y),
                        at(b, x - 1, y - 1),
                    ];
                    let nb: u8 = p.iter().sum();
                    let trans = (0..8).filter(|&i| p[i] == 0 && p[(i + 1) % 8] == 1).count();
                    let (c1, c2) = if step == 0 { (p[0] * p[2] * p[4], p[2] * p[4] * p[6]) } else { (p[0] * p[2] * p[6], p[0] * p[4] * p[6]) };
                    if (2..=6).contains(&nb) && trans == 1 && c1 == 0 && c2 == 0 {
                        kill.push(y * w + x);
                    }
                }
            }
            changed |= !kill.is_empty();
            for i in kill {
                b[i] = false;
            }
        }
        if !changed {
            break;
        }
    }
}

const NB: [(isize, isize); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

fn crossing(sk: &[bool], w: usize, x: usize, y: usize) -> usize {
    let p: Vec<bool> = NB.iter().map(|(dx, dy)| sk[(y as isize + dy) as usize * w + (x as isize + dx) as usize]).collect();
    (0..8).filter(|&i| p[i] != p[(i + 1) % 8]).count() / 2
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stop {
    Length,
    End,
    Junction,
}

/// Follow the skeleton from (x, y) starting at neighbour `first` for up to `len` steps.
/// Returns the vector to where it stopped, the steps taken and why it stopped.
fn trace(sk: &[bool], w: usize, h: usize, x: usize, y: usize, first: (isize, isize), len: usize) -> ((f32, f32), usize, Stop) {
    let (mut px, mut py) = (x as isize, y as isize);
    let (mut cx, mut cy) = (x as isize + first.0, y as isize + first.1);
    let on = |nx: isize, ny: isize| nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && sk[ny as usize * w + nx as usize];
    let mut steps = 1;
    let mut stop = Stop::Length;
    while steps < len {
        let next: Vec<(isize, isize)> = NB
            .iter()
            .map(|(dx, dy)| (cx + dx, cy + dy))
            .filter(|&(nx, ny)| on(nx, ny) && (nx, ny) != (px, py) && (nx, ny) != (x as isize, y as isize) && (nx - px).abs().max((ny - py).abs()) > 1)
            .collect();
        if next.is_empty() {
            stop = Stop::End;
            break;
        }
        // more than one way on that are not adjacent to each other = a fork
        let distinct = next.iter().filter(|a| !next.iter().any(|b| b != *a && (a.0 - b.0).abs().max((a.1 - b.1).abs()) == 1 && (b.0 - cx).abs() + (b.1 - cy).abs() < (a.0 - cx).abs() + (a.1 - cy).abs())).count();
        if distinct > 1 {
            stop = Stop::Junction;
            break;
        }
        // prefer the 4-connected step
        let (nx, ny) = *next.iter().min_by_key(|(nx, ny)| (nx - cx).abs() + (ny - cy).abs()).unwrap();
        px = cx;
        py = cy;
        cx = nx;
        cy = ny;
        steps += 1;
    }
    (((cx - x as isize) as f32, (cy - y as isize) as f32), steps, stop)
}

/// Extract minutiae from a ridge image; `fg` is the usable fingerprint area.
pub fn extract(img: &Img, fg: &[bool], p: &Params) -> Features {
    let (w, h) = (img.w, img.h);
    let period = ridge_period(img, fg).clamp(5.0, 16.0);
    // local normalisation inside the foreground
    let mean = blur(&Img { w, h, px: img.px.iter().zip(fg).map(|(&v, &m)| if m { v } else { 0.0 }).collect() }, 2.0 * period);
    let cov = blur(&Img { w, h, px: fg.iter().map(|&m| m as u8 as f32).collect() }, 2.0 * period);
    let centred = Img::from_fn(w, h, |x, y| {
        let i = y * w + x;
        if fg[i] { img.px[i] - mean.px[i] / cov.px[i].max(1e-3) } else { 0.0 }
    });
    let var = blur(&centred.map(|v| v * v), 2.0 * period);
    let norm = Img::from_fn(w, h, |x, y| {
        let i = y * w + x;
        centred.px[i] / (var.px[i] / cov.px[i].max(1e-3)).sqrt().max(1e-6)
    });
    let (theta, coh) = orientation(&norm, p.orient_sigma * period);
    let enh = gabor(&norm, &theta, fg, period);
    let mut sk: Vec<bool> = enh.px.iter().zip(fg).map(|(&v, &m)| m && v > 0.0).collect();
    // clear the outermost frame so neighbour lookups never leave the image
    for x in 0..w {
        sk[x] = false;
        sk[(h - 1) * w + x] = false;
    }
    for y in 0..h {
        sk[y * w] = false;
        sk[y * w + w - 1] = false;
    }
    thin(&mut sk, w, h);
    let inner = erode(fg, w, h, (p.margin * period).round().max(2.0) as usize);
    let len = period.round() as usize;
    let mut cands = Vec::new();
    for y in 2..h - 2 {
        for x in 2..w - 2 {
            let i = y * w + x;
            if !sk[i] || !inner[i] || coh.px[i] < p.min_coherence {
                continue;
            }
            let cn = crossing(&sk, w, x, y);
            let kind = match cn {
                1 => Kind::Ending,
                3 => Kind::Bifurcation,
                _ => continue,
            };
            let starts: Vec<(isize, isize)> = NB.iter().copied().filter(|(dx, dy)| sk[(y as isize + dy) as usize * w + (x as isize + dx) as usize]).collect();
            let traced: Vec<((f32, f32), usize, Stop)> = starts.iter().map(|&s| trace(&sk, w, h, x, y, s, 2 * len)).collect();
            // an ending whose ridge stops or forks again within ~1.5 periods is a short
            // ridge or a spur; a bifurcation with a branch that ends right away is a spur
            let short = (1.5 * period) as usize;
            let spurious = match kind {
                Kind::Ending => traced.iter().any(|t| t.2 != Stop::Length && t.1 < short),
                Kind::Bifurcation => traced.iter().any(|t| t.2 == Stop::End && t.1 < short),
            };
            if spurious && p.spur_filter {
                continue;
            }
            let vecs: Vec<(f32, f32)> = traced.iter().map(|t| t.0).collect();
            let raw = match kind {
                Kind::Ending => {
                    let (vx, vy) = vecs.iter().fold((0.0, 0.0), |a, v| (a.0 + v.0, a.1 + v.1));
                    (-vy).atan2(-vx)
                }
                Kind::Bifurcation => {
                    if vecs.len() < 3 {
                        continue;
                    }
                    // the stem is the branch most opposite to the other two
                    let unit: Vec<(f32, f32)> = vecs.iter().map(|&(a, b)| {
                        let n = a.hypot(b).max(1e-6);
                        (a / n, b / n)
                    }).collect();
                    let stem = (0..unit.len())
                        .min_by(|&a, &b| {
                            let s = |k: usize| (0..unit.len()).filter(|&j| j != k).map(|j| unit[k].0 * unit[j].0 + unit[k].1 * unit[j].1).sum::<f32>();
                            s(a).total_cmp(&s(b))
                        })
                        .unwrap();
                    unit[stem].1.atan2(unit[stem].0)
                }
            };
            // snap to the smoothed orientation field, keeping the traced sense
            let t = theta.px[i];
            let d = if (angle_diff(t, raw)).abs() < PI / 2.0 { t } else { t + PI };
            cands.push(Minutia { x: x as f32, y: y as f32, dir: d.rem_euclid(TAU), kind, quality: coh.px[i] });
        }
    }
    // drop clusters: anything with another minutia closer than min_pair_dist periods
    let lim = p.min_pair_dist * period;
    let keep: Vec<bool> = (0..cands.len())
        .map(|a| !(0..cands.len()).any(|b| b != a && (cands[a].x - cands[b].x).hypot(cands[a].y - cands[b].y) < lim))
        .collect();
    let minutiae = cands.into_iter().zip(keep).filter(|(_, k)| *k).map(|(m, _)| m).collect();
    Features { w, h, minutiae, mask: inner, period }
}

/// Signed angle difference a - b wrapped to (-pi, pi].
pub fn angle_diff(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(TAU);
    if d > PI { d - TAU } else { d }
}

/// Foreground mask for a full (dataset) fingerprint image: ridge-band contrast and a
/// clear ridge direction, smoothed, minus half a period.
pub fn foreground(img: &Img, period: f32) -> Vec<bool> {
    let dog = band(img, period);
    let sd = blur(&dog.map(|v| v * v), 1.5 * period).map(f32::sqrt);
    let (_, coh) = orientation(&dog, period);
    let thr = 0.3 * percentile(&sd.px, 95.0);
    let raw = Img { w: img.w, h: img.h, px: sd.px.iter().zip(&coh.px).map(|(&s, &c)| (s > thr && c > 0.4) as u8 as f32).collect() };
    let smooth: Vec<bool> = blur(&raw, period).px.iter().map(|&v| v > 0.5).collect();
    erode(&smooth, img.w, img.h, (period * 0.5) as usize)
}

/// Ridge-band (difference of Gaussians) version of a raw image.
pub fn band(img: &Img, period: f32) -> Img {
    blur(img, period / 6.0).zip(&blur(img, period), |a, b| a - b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_of_synthetic_ridges() {
        let img = Img::from_fn(128, 128, |x, y| (TAU * (x as f32 * 0.8 + y as f32 * 0.6) / 9.0).sin());
        let p = ridge_period(&img, &vec![true; 128 * 128]);
        assert!((p - 9.0).abs() < 1.0, "period {p}");
    }

    #[test]
    fn orientation_of_vertical_ridges() {
        // ridges run vertically (constant along y) -> orientation pi/2
        let img = Img::from_fn(64, 64, |x, _| (TAU * x as f32 / 8.0).sin());
        let (th, coh) = orientation(&img, 6.0);
        let t = th.get(32, 32);
        assert!((t - PI / 2.0).abs() < 0.1, "theta {t}");
        assert!(coh.get(32, 32) > 0.9);
    }

    #[test]
    fn stripes_have_no_minutiae_and_dislocations_are_found() {
        let stripes = Img::from_fn(160, 160, |x, y| (TAU * (x as f32 * 0.9 + y as f32 * 0.44) / 8.0).sin() * 40.0);
        let f = extract(&stripes, &vec![true; 160 * 160], &Params::default());
        assert!(f.minutiae.len() <= 1, "stripes gave {} minutiae", f.minutiae.len());
        // two ridge dislocations (one ridge inserted, one removed) = two minutiae at known spots
        let at = [(70.0f32, 80.0f32), (150.0, 110.0)];
        let img = Img::from_fn(220, 190, |x, y| {
            let (x, y) = (x as f32, y as f32);
            let ph = TAU * x / 9.0 + (y - at[0].1).atan2(x - at[0].0) - (y - at[1].1).atan2(x - at[1].0);
            ph.cos() * 40.0
        });
        let f = extract(&img, &vec![true; 220 * 190], &Params::default());
        for (ax, ay) in at {
            assert!(
                f.minutiae.iter().any(|m| (m.x - ax).hypot(m.y - ay) < 9.0),
                "no minutia near ({ax}, {ay}); found {:?}",
                f.minutiae.iter().map(|m| (m.x as i32, m.y as i32)).collect::<Vec<_>>()
            );
        }
        assert!(f.minutiae.len() <= 4, "too many minutiae: {}", f.minutiae.len());
    }
}
